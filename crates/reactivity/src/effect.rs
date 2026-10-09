//! `ReactiveEffect` of @vue/reactivity 3.5.43 (run, stop, pause, resume), `effect()`, `onEffectCleanup`, the tracking
//! stack and the JS surface of effects. Behaviour per @vue/reactivity 3.5.43 (production build).

use std::cell::{Cell, RefCell};

use vn_value::{Call, Flags, ObjRef, PropKey, Res, Rt, Tracer, Value};

use crate::graph::{cleanup_deps, is_dirty, prepare_deps, remove_sub, trigger_effect};
use crate::node::{alloc, node, try_node, Node, SubFields, ACTIVE, PAUSED, RUNNING, TRACKING};
use crate::shared::{accessor, native, truthy};
use crate::state::state;

pub(crate) struct Effect {
    pub(crate) sub: SubFields,
    pub(crate) fn_: Value,
    pub(crate) cleanup: RefCell<Value>,
    pub(crate) scheduler: RefCell<Value>,
    pub(crate) on_stop: RefCell<Value>,
    /// Cleanups registered by watchers (`cleanupMap` of the watch base); `None` once they have run.
    pub(crate) watch_cleanups: RefCell<Option<Vec<Value>>>,
    /// Membership of `pausedQueueEffects`: triggered while paused, to run on resume.
    pub(crate) queued: Cell<bool>,
}

impl Effect {
    pub(crate) fn trace(&self, t: &mut Tracer) {
        self.sub.trace(t);
        t.mark_value(&self.fn_);
        t.mark_value(&self.cleanup.borrow());
        t.mark_value(&self.scheduler.borrow());
        t.mark_value(&self.on_stop.borrow());
        if let Some(list) = self.watch_cleanups.borrow().as_ref() {
            for v in list {
                t.mark_value(v);
            }
        }
    }
}

/// A new ReactiveEffect. It joins the active scope, or is created inactive when that scope is stopped.
pub(crate) fn new_effect(rt: &Rt, fn_: Value) -> ObjRef {
    let proto = state(rt).protos.get().effect;
    let scope = state(rt).active_scope.get();
    let mut flags = ACTIVE | TRACKING;
    if let Some(sc) = scope {
        if !node(rt, sc).scope().active.get() {
            flags &= !ACTIVE;
        }
    }
    let e = alloc(
        rt,
        proto,
        Node::Effect(Effect {
            sub: SubFields::new(flags),
            fn_,
            cleanup: RefCell::new(Value::Undefined),
            scheduler: RefCell::new(Value::Undefined),
            on_stop: RefCell::new(Value::Undefined),
            watch_cleanups: RefCell::new(None),
            queued: Cell::new(false),
        }),
    );
    if let Some(sc) = scope {
        let sn = node(rt, sc);
        let s = sn.scope();
        if s.active.get() {
            s.effects.borrow_mut().push(e);
        }
    }
    e
}

/// `ReactiveEffect.run`: runs the function with this effect as the active subscriber (no tracking when stopped).
pub(crate) fn effect_run(rt: &Rt, e: ObjRef) -> Res<Value> {
    let en = node(rt, e);
    let ef = en.effect();
    let fn_ = ef.fn_.clone();
    if ef.sub.flags.get() & ACTIVE == 0 {
        return rt.call(&fn_, &Value::Object(e), &[]);
    }
    ef.sub.flags.set(ef.sub.flags.get() | RUNNING);
    cleanup_effect(rt, e)?;
    prepare_deps(rt, e);
    let st = state(rt);
    let prev_sub = st.active_sub.get();
    let prev_track = st.should_track.get();
    st.active_sub.set(Some(e));
    st.should_track.set(true);
    let result = rt.call(&fn_, &Value::Object(e), &[]);
    cleanup_deps(rt, e);
    st.active_sub.set(prev_sub);
    st.should_track.set(prev_track);
    ef.sub.flags.set(ef.sub.flags.get() & !RUNNING);
    result
}

/// `cleanupEffect`: runs and forgets the cleanup registered by the previous run (with no active subscriber).
pub(crate) fn cleanup_effect(rt: &Rt, e: ObjRef) -> Res<()> {
    let cleanup = {
        let en = node(rt, e);
        let ef = en.effect();
        ef.cleanup.replace(Value::Undefined)
    };
    if !truthy(&cleanup) {
        return Ok(());
    }
    let st = state(rt);
    let prev = st.active_sub.get();
    st.active_sub.set(None);
    let r = rt.call(&cleanup, &Value::Undefined, &[]);
    st.active_sub.set(prev);
    r.map(|_| ())
}

/// `ReactiveEffect.stop`: unsubscribes from every dep, runs the cleanup and `onStop`, and deactivates.
pub(crate) fn effect_stop(rt: &Rt, e: ObjRef) -> Res<()> {
    let en = node(rt, e);
    let ef = en.effect();
    if ef.sub.flags.get() & ACTIVE == 0 {
        return Ok(());
    }
    let mut link = ef.sub.deps.get();
    while let Some(lo) = link {
        remove_sub(rt, lo, false);
        link = node(rt, lo).link().next_dep.get();
    }
    ef.sub.deps.set(None);
    ef.sub.deps_tail.set(None);
    cleanup_effect(rt, e)?;
    let on_stop = ef.on_stop.borrow().clone();
    if truthy(&on_stop) {
        rt.call(&on_stop, &Value::Object(e), &[])?;
    }
    ef.sub.flags.set(ef.sub.flags.get() & !ACTIVE);
    Ok(())
}

pub(crate) fn effect_pause(rt: &Rt, e: ObjRef) {
    let en = node(rt, e);
    let s = en.effect().sub.flags.get();
    en.effect().sub.flags.set(s | PAUSED);
}

/// `ReactiveEffect.resume`: unpauses and runs the trigger that arrived while paused, if any.
pub(crate) fn effect_resume(rt: &Rt, e: ObjRef) -> Res<()> {
    let queued = {
        let en = node(rt, e);
        let ef = en.effect();
        if ef.sub.flags.get() & PAUSED == 0 {
            return Ok(());
        }
        ef.sub.flags.set(ef.sub.flags.get() & !PAUSED);
        let q = ef.queued.get();
        ef.queued.set(false);
        q
    };
    if queued {
        trigger_effect(rt, e)?;
    }
    Ok(())
}

/// `onEffectCleanup(fn)`: stores the cleanup on the active effect (a non-effect subscriber ignores it).
pub(crate) fn on_effect_cleanup(rt: &Rt, f: Value) {
    if let Some(a) = state(rt).active_sub.get() {
        let an = node(rt, a);
        if an.is_effect() {
            *an.effect().cleanup.borrow_mut() = f;
        }
    }
}

pub(crate) fn pause_tracking(rt: &Rt) {
    let st = state(rt);
    let cur = st.should_track.get();
    st.track_stack.borrow_mut().push(cur);
    st.should_track.set(false);
}

pub(crate) fn enable_tracking(rt: &Rt) {
    let st = state(rt);
    let cur = st.should_track.get();
    st.track_stack.borrow_mut().push(cur);
    st.should_track.set(true);
}

pub(crate) fn reset_tracking(rt: &Rt) {
    let st = state(rt);
    let last = st.track_stack.borrow_mut().pop();
    st.should_track.set(last.unwrap_or(true));
}

/// The effect behind a JS receiver, or a TypeError.
fn effect_obj(rt: &Rt, v: &Value) -> Res<ObjRef> {
    if let Value::Object(o) = v {
        if let Some(n) = try_node(rt, *o) {
            if n.is_effect() {
                return Ok(*o);
            }
        }
    }
    Err(rt.throw_type_error("not a reactive effect"))
}

fn js_effect(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let mut f = c.arg(0);
    // `if (fn.effect instanceof ReactiveEffect) fn = fn.effect.fn`: a runner's effect is reused for its function.
    let inner = rt.get_str(&f, "effect")?;
    if let Value::Object(io) = inner {
        if let Some(n) = try_node(rt, io) {
            if n.is_effect() {
                f = n.effect().fn_.clone();
            }
        }
    }
    let e = new_effect(rt, f);
    let options = c.arg(1);
    let mut lazy = false;
    if truthy(&options) {
        let sched = rt.get_str(&options, "scheduler")?;
        let on_stop = rt.get_str(&options, "onStop")?;
        lazy = rt.get_str(&options, "lazy")?.to_boolean();
        let en = node(rt, e);
        *en.effect().scheduler.borrow_mut() = sched;
        *en.effect().on_stop.borrow_mut() = on_stop;
    }
    // `lazy: true` creates the effect without its first run (runtime-core's ReactiveEffect: the caller runs it).
    if !lazy {
        if let Err(err) = effect_run(rt, e) {
            return match effect_stop(rt, e) {
                Err(e2) => Err(e2),
                Ok(()) => Err(err),
            };
        }
    }
    let runner = native(rt, "bound run", 0, runner_code, vec![Value::Object(e)]);
    rt.define_data(runner, PropKey::from_str("effect"), Value::Object(e), Flags::ALL)?;
    Ok(Value::Object(runner))
}

fn runner_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = rt.fn_data(c.callee)[0].as_object().expect("effect runner data (framework bug)");
    effect_run(rt, e)
}

fn js_stop(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let eff = rt.get_str(&c.arg(0), "effect")?;
    let e = effect_obj(rt, &eff)?;
    effect_stop(rt, e)?;
    Ok(Value::Undefined)
}

fn js_on_effect_cleanup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    on_effect_cleanup(rt, c.arg(0));
    Ok(Value::Undefined)
}

fn js_pause_tracking(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    pause_tracking(rt);
    Ok(Value::Undefined)
}

fn js_enable_tracking(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    enable_tracking(rt);
    Ok(Value::Undefined)
}

fn js_reset_tracking(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    reset_tracking(rt);
    Ok(Value::Undefined)
}

pub(crate) fn exported(rt: &Rt) -> Vec<(&'static str, Value)> {
    vec![
        ("effect", Value::Object(rt.new_native_function(js_effect, "effect", 2, Vec::new()))),
        ("stop", Value::Object(rt.new_native_function(js_stop, "stop", 1, Vec::new()))),
        ("onEffectCleanup", Value::Object(rt.new_native_function(js_on_effect_cleanup, "onEffectCleanup", 1, Vec::new()))),
        ("pauseTracking", Value::Object(rt.new_native_function(js_pause_tracking, "pauseTracking", 0, Vec::new()))),
        ("enableTracking", Value::Object(rt.new_native_function(js_enable_tracking, "enableTracking", 0, Vec::new()))),
        ("resetTracking", Value::Object(rt.new_native_function(js_reset_tracking, "resetTracking", 0, Vec::new()))),
    ]
}

// ----- JS methods and accessors of ReactiveEffect objects -----

fn this_effect(rt: &Rt, c: &Call<'_>) -> Res<ObjRef> {
    effect_obj(rt, c.this)
}

fn m_run(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = this_effect(rt, c)?;
    effect_run(rt, e)
}

fn m_stop(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = this_effect(rt, c)?;
    effect_stop(rt, e)?;
    Ok(Value::Undefined)
}

fn m_pause(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = this_effect(rt, c)?;
    effect_pause(rt, e);
    Ok(Value::Undefined)
}

fn m_resume(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = this_effect(rt, c)?;
    effect_resume(rt, e)?;
    Ok(Value::Undefined)
}

fn g_dirty(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = this_effect(rt, c)?;
    Ok(Value::Bool(is_dirty(rt, e)?))
}

fn g_flags(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = this_effect(rt, c)?;
    Ok(Value::Number(node(rt, e).effect().sub.flags.get() as f64))
}

fn s_flags(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = this_effect(rt, c)?;
    let v = rt.to_uint32(&c.arg(0))?;
    node(rt, e).effect().sub.flags.set(v);
    Ok(Value::Undefined)
}

fn g_scheduler(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = this_effect(rt, c)?;
    let v = node(rt, e).effect().scheduler.borrow().clone();
    Ok(v)
}

fn s_scheduler(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = this_effect(rt, c)?;
    *node(rt, e).effect().scheduler.borrow_mut() = c.arg(0);
    Ok(Value::Undefined)
}

fn g_on_stop(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = this_effect(rt, c)?;
    let v = node(rt, e).effect().on_stop.borrow().clone();
    Ok(v)
}

fn s_on_stop(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = this_effect(rt, c)?;
    *node(rt, e).effect().on_stop.borrow_mut() = c.arg(0);
    Ok(Value::Undefined)
}

fn g_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = this_effect(rt, c)?;
    let v = node(rt, e).effect().fn_.clone();
    Ok(v)
}

/// The ReactiveEffect prototype (its methods and accessors; `Object.prototype` above it).
pub(crate) fn install_proto(rt: &Rt) -> ObjRef {
    let p = rt.new_object();
    rt.define_native_method(p, "run", m_run, 0);
    rt.define_native_method(p, "stop", m_stop, 0);
    rt.define_native_method(p, "pause", m_pause, 0);
    rt.define_native_method(p, "resume", m_resume, 0);
    accessor(rt, p, "dirty", g_dirty, None);
    accessor(rt, p, "flags", g_flags, Some(s_flags));
    accessor(rt, p, "scheduler", g_scheduler, Some(s_scheduler));
    accessor(rt, p, "onStop", g_on_stop, Some(s_on_stop));
    accessor(rt, p, "fn", g_fn, None);
    p
}
