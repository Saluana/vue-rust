//! `EffectScope` of @vue/reactivity 3.5.43: ownership of effects and child scopes, stop cascade (effects, then the
//! scope's own cleanups, then child scopes), pause/resume, `on`/`off`, `run`, `onScopeDispose`. Behaviour per
//! @vue/reactivity 3.5.43 (production build).

use std::cell::{Cell, RefCell};

use vn_value::{Call, Flags, ObjRef, PropKey, Res, Rt, Tracer, Value};

use crate::effect::{effect_pause, effect_resume, effect_stop};
use crate::node::{alloc, node, this_node, Node};
use crate::shared::{accessor, hidden, native, truthy};
use crate::state::state;

pub(crate) struct Scope {
    pub(crate) detached: bool,
    pub(crate) active: Cell<bool>,
    pub(crate) on: Cell<u32>,
    pub(crate) paused: Cell<bool>,
    pub(crate) effects: RefCell<Vec<ObjRef>>,
    pub(crate) cleanups: RefCell<Vec<Value>>,
    pub(crate) scopes: RefCell<Vec<ObjRef>>,
    pub(crate) parent: Cell<Option<ObjRef>>,
    pub(crate) index: Cell<usize>,
    pub(crate) prev_scope: Cell<Option<ObjRef>>,
}

impl Scope {
    pub(crate) fn trace(&self, t: &mut Tracer) {
        for e in self.effects.borrow().iter() {
            t.mark(*e);
        }
        for s in self.scopes.borrow().iter() {
            t.mark(*s);
        }
        for c in self.cleanups.borrow().iter() {
            t.mark_value(c);
        }
        for o in [self.parent.get(), self.prev_scope.get()].into_iter().flatten() {
            t.mark(o);
        }
    }
}

/// `new EffectScope(detached)`. A non-detached scope joins the active scope as its child; inside a stopped scope it
/// is created inactive.
pub(crate) fn new_scope(rt: &Rt, detached: bool) -> ObjRef {
    let proto = state(rt).protos.get().scope;
    let s = Scope {
        detached,
        active: Cell::new(true),
        on: Cell::new(0),
        paused: Cell::new(false),
        effects: RefCell::new(Vec::new()),
        cleanups: RefCell::new(Vec::new()),
        scopes: RefCell::new(Vec::new()),
        parent: Cell::new(None),
        index: Cell::new(0),
        prev_scope: Cell::new(None),
    };
    let o = alloc(rt, proto, Node::Scope(s));
    let active = state(rt).active_scope.get();
    if !detached {
        if let Some(p) = active {
            let pn = node(rt, p);
            let ps = pn.scope();
            let sn = node(rt, o);
            let me = sn.scope();
            if ps.active.get() {
                me.parent.set(Some(p));
                ps.scopes.borrow_mut().push(o);
                me.index.set(ps.scopes.borrow().len() - 1);
            } else {
                me.active.set(false);
            }
        }
    }
    o
}

/// `scope.pause()`: child scopes first, then the effects; a stopped scope does nothing.
pub(crate) fn scope_pause(rt: &Rt, s: ObjRef) {
    let sn = node(rt, s);
    let sc = sn.scope();
    if !sc.active.get() {
        return;
    }
    sc.paused.set(true);
    let children: Vec<ObjRef> = sc.scopes.borrow().clone();
    for c in children {
        scope_pause(rt, c);
    }
    let effects: Vec<ObjRef> = sc.effects.borrow().clone();
    for e in effects {
        effect_pause(rt, e);
    }
}

/// `scope.resume()`: resumes a paused scope; queued triggers of its effects run here.
pub(crate) fn scope_resume(rt: &Rt, s: ObjRef) -> Res<()> {
    let sn = node(rt, s);
    let sc = sn.scope();
    if !sc.active.get() || !sc.paused.get() {
        return Ok(());
    }
    sc.paused.set(false);
    let children: Vec<ObjRef> = sc.scopes.borrow().clone();
    for c in children {
        scope_resume(rt, c)?;
    }
    let effects: Vec<ObjRef> = sc.effects.borrow().clone();
    for e in effects {
        effect_resume(rt, e)?;
    }
    Ok(())
}

/// `scope.run(fn)`: runs `fn` with this scope as the active one; a stopped scope returns undefined.
pub(crate) fn scope_run(rt: &Rt, s: ObjRef, f: &Value) -> Res<Value> {
    let sn = node(rt, s);
    if !sn.scope().active.get() {
        return Ok(Value::Undefined);
    }
    let st = state(rt);
    let prev = st.active_scope.get();
    st.active_scope.set(Some(s));
    let r = rt.call(f, &Value::Undefined, &[]);
    st.active_scope.set(prev);
    r
}

/// `scope.on()`: makes the scope active without running a function (counted, so calls nest).
pub(crate) fn scope_on(rt: &Rt, s: ObjRef) {
    let sn = node(rt, s);
    let sc = sn.scope();
    let n = sc.on.get() + 1;
    sc.on.set(n);
    if n == 1 {
        let st = state(rt);
        sc.prev_scope.set(st.active_scope.get());
        st.active_scope.set(Some(s));
    }
}

/// `scope.off()`: undoes the matching `on()`, restoring the previously active scope.
pub(crate) fn scope_off(rt: &Rt, s: ObjRef) {
    let sn = node(rt, s);
    let sc = sn.scope();
    if sc.on.get() == 0 {
        return;
    }
    let n = sc.on.get() - 1;
    sc.on.set(n);
    if n != 0 {
        return;
    }
    let st = state(rt);
    if st.active_scope.get() == Some(s) {
        st.active_scope.set(sc.prev_scope.get());
    } else {
        let mut cur = st.active_scope.get();
        while let Some(c) = cur {
            let cn = node(rt, c);
            let cs = cn.scope();
            if cs.prev_scope.get() == Some(s) {
                cs.prev_scope.set(sc.prev_scope.get());
                break;
            }
            cur = cs.prev_scope.get();
        }
    }
    sc.prev_scope.set(None);
}

/// `scope.stop(fromParent)`: effects first, then the scope's cleanups, then child scopes; removes itself from its
/// parent unless the parent is stopping it.
pub(crate) fn scope_stop(rt: &Rt, s: ObjRef, from_parent: bool) -> Res<()> {
    let sn = node(rt, s);
    let sc = sn.scope();
    if !sc.active.get() {
        return Ok(());
    }
    sc.active.set(false);
    let n_effects = sc.effects.borrow().len();
    for i in 0..n_effects {
        let e = sc.effects.borrow().get(i).copied();
        if let Some(e) = e {
            effect_stop(rt, e)?;
        }
    }
    sc.effects.borrow_mut().clear();
    let n_cleanups = sc.cleanups.borrow().len();
    for i in 0..n_cleanups {
        let f = sc.cleanups.borrow().get(i).cloned();
        if let Some(f) = f {
            rt.call(&f, &Value::Undefined, &[])?;
        }
    }
    sc.cleanups.borrow_mut().clear();
    let children: Vec<ObjRef> = sc.scopes.borrow().clone();
    for c in children {
        scope_stop(rt, c, true)?;
    }
    sc.scopes.borrow_mut().clear();
    if !sc.detached && !from_parent {
        if let Some(p) = sc.parent.get() {
            let pn = node(rt, p);
            let ps = pn.scope();
            let last = ps.scopes.borrow_mut().pop();
            if let Some(last) = last {
                if last != s {
                    let idx = sc.index.get();
                    ps.scopes.borrow_mut()[idx] = last;
                    node(rt, last).scope().index.set(idx);
                }
            }
        }
    }
    sc.parent.set(None);
    Ok(())
}

/// `onScopeDispose(fn)`: registers `fn` on the active scope; with no active scope it does nothing.
pub(crate) fn on_scope_dispose(rt: &Rt, f: Value) {
    if let Some(s) = state(rt).active_scope.get() {
        node(rt, s).scope().cleanups.borrow_mut().push(f);
    }
}

fn is_scope_node(n: &Node) -> bool {
    n.is_scope()
}

fn js_effect_scope(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Object(new_scope(rt, truthy(&c.arg(0)))))
}

fn js_get_current_scope(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(state(rt).active_scope.get().map(Value::Object).unwrap_or(Value::Undefined))
}

fn js_on_scope_dispose(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    on_scope_dispose(rt, c.arg(0));
    Ok(Value::Undefined)
}

/// `EffectScope` as a constructor: `new EffectScope(detached)`. A plain call is a TypeError, as for a class.
fn scope_ctor_call(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("Class constructor EffectScope cannot be invoked without 'new'"))
}

fn scope_ctor(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Object(new_scope(rt, truthy(&c.arg(0)))))
}

pub(crate) fn exported(rt: &Rt, proto: ObjRef) -> Vec<(&'static str, Value)> {
    let ctor = rt.new_native_constructor(scope_ctor_call, scope_ctor, "EffectScope", 0, Vec::new());
    rt.define_data(ctor, PropKey::lit("prototype"), Value::Object(proto), Flags::NONE)
        .expect("EffectScope.prototype (framework bug)");
    hidden(rt, proto, "constructor", Value::Object(ctor));
    vec![
        ("effectScope", Value::Object(native(rt, "effectScope", 1, js_effect_scope, Vec::new()))),
        ("getCurrentScope", Value::Object(native(rt, "getCurrentScope", 0, js_get_current_scope, Vec::new()))),
        ("onScopeDispose", Value::Object(native(rt, "onScopeDispose", 1, js_on_scope_dispose, Vec::new()))),
        ("EffectScope", Value::Object(ctor)),
    ]
}

// ----- JS methods of EffectScope objects -----

fn this_scope(rt: &Rt, c: &Call<'_>) -> Res<ObjRef> {
    this_node(rt, c.this, is_scope_node)
}

fn m_run(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_scope(rt, c)?;
    scope_run(rt, s, &c.arg(0))
}

fn m_stop(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_scope(rt, c)?;
    scope_stop(rt, s, truthy(&c.arg(0)))?;
    Ok(Value::Undefined)
}

fn m_pause(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_scope(rt, c)?;
    scope_pause(rt, s);
    Ok(Value::Undefined)
}

fn m_resume(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_scope(rt, c)?;
    scope_resume(rt, s)?;
    Ok(Value::Undefined)
}

fn m_on(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_scope(rt, c)?;
    scope_on(rt, s);
    Ok(Value::Undefined)
}

fn m_off(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_scope(rt, c)?;
    scope_off(rt, s);
    Ok(Value::Undefined)
}

fn g_active(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_scope(rt, c)?;
    Ok(Value::Bool(node(rt, s).scope().active.get()))
}

/// The EffectScope prototype: `run`, `stop`, `pause`, `resume`, `on`, `off` and the `active` accessor.
pub(crate) fn install_proto(rt: &Rt) -> ObjRef {
    let p = rt.new_object();
    // EffectScope carries `readonly __v_skip = true` (packages/reactivity/src/effectScope.ts), so reactive() and readonly()
    // return a scope unchanged: `reactive({ scope }).scope` is the scope itself, and its methods get the scope as `this`.
    hidden(rt, p, "__v_skip", Value::Bool(true));
    rt.define_native_method(p, "run", m_run, 1);
    rt.define_native_method(p, "stop", m_stop, 1);
    rt.define_native_method(p, "pause", m_pause, 0);
    rt.define_native_method(p, "resume", m_resume, 0);
    rt.define_native_method(p, "on", m_on, 0);
    rt.define_native_method(p, "off", m_off, 0);
    accessor(rt, p, "active", g_active, None);
    p
}
