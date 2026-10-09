//! `ComputedRefImpl` of @vue/reactivity 3.5.43: a lazy, cached subscriber with its own dep. Behaviour per
//! @vue/reactivity 3.5.43 (production build): `refreshComputed` decides when the getter runs, and a throwing getter
//! bumps the dep version before the error propagates.

use std::cell::{Cell, RefCell};

use vn_value::{Call, ObjRef, Res, Rt, Tracer, Value};

use crate::graph::{cleanup_deps, dep_track, is_dirty, new_dep, prepare_deps};
use crate::node::{alloc, node, this_node, Node, SubFields, DIRTY, EVALUATED, RUNNING, TRACKING};
use crate::shared::{accessor, has_changed, native, truthy};
use crate::state::state;

pub(crate) struct Computed {
    pub(crate) sub: SubFields,
    pub(crate) dep: ObjRef,
    pub(crate) getter: Value,
    pub(crate) setter: Value,
    pub(crate) value: RefCell<Value>,
    pub(crate) global_version: Cell<i64>,
    pub(crate) ssr: bool,
}

impl Computed {
    pub(crate) fn trace(&self, t: &mut Tracer) {
        self.sub.trace(t);
        t.mark(self.dep);
        t.mark_value(&self.getter);
        t.mark_value(&self.setter);
        t.mark_value(&self.value.borrow());
    }
}

/// `new ComputedRefImpl(fn, setter, isSSR)`: starts dirty, its own dep points back at it.
pub(crate) fn new_computed(rt: &Rt, getter: Value, setter: Value, ssr: bool) -> ObjRef {
    let dep = new_dep(rt, None, None);
    let gv = state(rt).global_version.get() - 1;
    let proto = state(rt).protos.get().computed;
    let c = alloc(
        rt,
        proto,
        Node::Computed(Computed {
            sub: SubFields::new(DIRTY),
            dep,
            getter,
            setter,
            value: RefCell::new(Value::Undefined),
            global_version: Cell::new(gv),
            ssr,
        }),
    );
    node(rt, dep).dep().computed.set(Some(c));
    c
}

fn bump_dep(rt: &Rt, dep: ObjRef) {
    let dn = node(rt, dep);
    let d = dn.dep();
    d.version.set(d.version.get() + 1);
}

/// `refreshComputed`: re-runs the getter only when the cached value may be stale.
pub(crate) fn refresh_computed(rt: &Rt, c: ObjRef) -> Res<()> {
    let cn = node(rt, c);
    let cc = cn.computed();
    let st = state(rt);
    if cc.sub.flags.get() & TRACKING != 0 && cc.sub.flags.get() & DIRTY == 0 {
        return Ok(());
    }
    cc.sub.flags.set(cc.sub.flags.get() & !DIRTY);
    if cc.global_version.get() == st.global_version.get() {
        return Ok(());
    }
    cc.global_version.set(st.global_version.get());
    let evaluated = cc.sub.flags.get() & EVALUATED != 0;
    if !cc.ssr && evaluated && (cc.sub.deps.get().is_none() || !is_dirty(rt, c)?) {
        return Ok(());
    }
    cc.sub.flags.set(cc.sub.flags.get() | RUNNING);
    let dep = cc.dep;
    let prev_sub = st.active_sub.get();
    let prev_track = st.should_track.get();
    st.active_sub.set(Some(c));
    st.should_track.set(true);
    let result = (|| -> Res<()> {
        prepare_deps(rt, c);
        let old = cc.value.borrow().clone();
        let value = rt.call(&cc.getter, &Value::Object(c), &[old])?;
        let version = node(rt, dep).dep().version.get();
        let changed = version == 0 || {
            let cur = cc.value.borrow();
            has_changed(&value, &cur)
        };
        if changed {
            cc.sub.flags.set(cc.sub.flags.get() | EVALUATED);
            *cc.value.borrow_mut() = value;
            bump_dep(rt, dep);
        }
        Ok(())
    })();
    if result.is_err() {
        bump_dep(rt, dep);
    }
    st.active_sub.set(prev_sub);
    st.should_track.set(prev_track);
    cleanup_deps(rt, c);
    cc.sub.flags.set(cc.sub.flags.get() & !RUNNING);
    result
}

/// The `value` getter: tracks the computed's dep, refreshes, and records the version the reader saw.
pub(crate) fn computed_get(rt: &Rt, c: ObjRef) -> Res<Value> {
    let dep = node(rt, c).computed().dep;
    let link = dep_track(rt, dep);
    refresh_computed(rt, c)?;
    if let Some(l) = link {
        let version = node(rt, dep).dep().version.get();
        node(rt, l).link().version.set(version);
    }
    let v = node(rt, c).computed().value.borrow().clone();
    Ok(v)
}

/// The `value` setter: calls the user setter, or does nothing for a read-only computed.
pub(crate) fn computed_set(rt: &Rt, c: ObjRef, v: Value) -> Res<()> {
    let setter = node(rt, c).computed().setter.clone();
    if truthy(&setter) {
        rt.call(&setter, &Value::Object(c), &[v])?;
    }
    Ok(())
}

/// `computed(getterOrOptions, debugOptions, isSSR)`. Debug options are development-only and ignored.
fn js_computed(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let arg = c.arg(0);
    let (getter, setter) = if rt.is_callable(&arg) {
        (arg, Value::Undefined)
    } else {
        let g = rt.get_str(&arg, "get")?;
        let s = rt.get_str(&arg, "set")?;
        (g, s)
    };
    let ssr = truthy(&c.arg(2));
    Ok(Value::Object(new_computed(rt, getter, setter, ssr)))
}

pub(crate) fn exported(rt: &Rt) -> (&'static str, Value) {
    ("computed", Value::Object(native(rt, "computed", 2, js_computed, Vec::new())))
}

fn is_computed_node(n: &Node) -> bool {
    n.is_computed()
}

fn g_value(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_node(rt, c.this, is_computed_node)?;
    computed_get(rt, o)
}

fn s_value(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_node(rt, c.this, is_computed_node)?;
    computed_set(rt, o, c.arg(0))?;
    Ok(Value::Undefined)
}

fn g_effect(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_node(rt, c.this, is_computed_node)?;
    Ok(Value::Object(o))
}

fn g_dep(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_node(rt, c.this, is_computed_node)?;
    Ok(Value::Object(node(rt, o).computed().dep))
}

fn g_is_readonly(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_node(rt, c.this, is_computed_node)?;
    let has_setter = truthy(&node(rt, o).computed().setter);
    Ok(Value::Bool(!has_setter))
}

/// The ComputedRefImpl prototype: `value`, `effect`, `dep` and `__v_isReadonly` accessors, `__v_isRef` true.
pub(crate) fn install_proto(rt: &Rt) -> ObjRef {
    let p = rt.new_object();
    crate::shared::hidden(rt, p, "__v_isRef", Value::Bool(true));
    accessor(rt, p, "value", g_value, Some(s_value));
    accessor(rt, p, "effect", g_effect, None);
    accessor(rt, p, "dep", g_dep, None);
    accessor(rt, p, "__v_isReadonly", g_is_readonly, None);
    p
}
