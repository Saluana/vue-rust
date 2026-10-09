//! The watch base of @vue/reactivity 3.5.43: `watch(source, cb, options)` as a ReactiveEffect whose getter reads the source
//! and whose job runs the callback when the value changed, `onWatcherCleanup`, `getCurrentWatcher` and `traverse` (the deep
//! walk). Behaviour per @vue/reactivity 3.5.43 (production build). Runtime-level watchers (flush timing) are WP-RT1.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use vn_value::{Call, ObjRef, PropKey, Res, Rt, Tracer, Value};

use crate::effect::{effect_pause, effect_resume, effect_run, effect_stop, new_effect, pause_tracking, reset_tracking};
use crate::graph::is_dirty;
use crate::node::{alloc, node, Node, ACTIVE};
use crate::shared::{enumerable_symbol_keys, for_in_keys, has_changed, has_tag, is_object, is_reactive, is_ref, is_shallow, native, truthy};
use crate::state::state;

/// Source shapes of `watch` (the branches of the source test in 3.5.43).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Ref,
    Reactive,
    Array,
    FuncCb,
    FuncNoCb,
    Noop,
}

/// The previous value passed to the callback. `Single(Uninit)` and `MultiInitial` are the INITIAL marker.
#[derive(Clone)]
pub(crate) enum Old {
    Single(Value),
    Multi(Value),
    MultiInitial(usize),
}

/// The state of one watcher: the effect, the source, the callback and the cleanup plumbing.
pub(crate) struct Watch {
    pub(crate) effect: Cell<Option<ObjRef>>,
    pub(crate) source: Value,
    pub(crate) kind: Kind,
    pub(crate) is_multi: bool,
    pub(crate) force_trigger: bool,
    pub(crate) deep: Value,
    pub(crate) depth: f64,
    pub(crate) deep_wrap: bool,
    pub(crate) has_cb: bool,
    pub(crate) cb: RefCell<Value>,
    pub(crate) call: Value,
    pub(crate) scheduler: Value,
    pub(crate) job: RefCell<Value>,
    pub(crate) handle: RefCell<Value>,
    pub(crate) bound_cleanup: RefCell<Value>,
    pub(crate) on_stop: RefCell<Value>,
    pub(crate) old: RefCell<Old>,
    pub(crate) scope: Option<ObjRef>,
}

impl Watch {
    pub(crate) fn trace(&self, t: &mut Tracer) {
        if let Some(e) = self.effect.get() {
            t.mark(e);
        }
        if let Some(s) = self.scope {
            t.mark(s);
        }
        t.mark_value(&self.source);
        t.mark_value(&self.deep);
        t.mark_value(&self.call);
        t.mark_value(&self.scheduler);
        t.mark_value(&self.cb.borrow());
        t.mark_value(&self.job.borrow());
        t.mark_value(&self.handle.borrow());
        t.mark_value(&self.bound_cleanup.borrow());
        t.mark_value(&self.on_stop.borrow());
        match &*self.old.borrow() {
            Old::Single(v) | Old::Multi(v) => t.mark_value(v),
            Old::MultiInitial(_) => {}
        }
    }
}

/// Options of `watch` (the production build reads these; `onWarn` and debug hooks only serve development builds).
#[derive(Clone, Default)]
pub struct WatchOptions {
    pub immediate: Value,
    pub deep: Value,
    pub once: Value,
    pub scheduler: Value,
    pub augment_job: Value,
    pub call: Value,
}

/// The handle returned by `watch`: a function with `pause`, `resume` and `stop` (which is the handle itself).
#[derive(Clone)]
pub struct WatchHandle {
    pub handle: Value,
}

impl WatchHandle {
    pub fn pause(&self, rt: &Rt) -> Res<()> {
        let f = rt.get_str(&self.handle, "pause")?;
        rt.call(&f, &Value::Undefined, &[])?;
        Ok(())
    }

    pub fn resume(&self, rt: &Rt) -> Res<()> {
        let f = rt.get_str(&self.handle, "resume")?;
        rt.call(&f, &Value::Undefined, &[])?;
        Ok(())
    }

    pub fn stop(&self, rt: &Rt) -> Res<()> {
        rt.call(&self.handle, &Value::Undefined, &[])?;
        Ok(())
    }
}

fn num(x: f64) -> Value {
    Value::Number(x)
}

fn watch_of(rt: &Rt, o: ObjRef) -> crate::node::NodeRef {
    node(rt, o)
}

fn data_obj(rt: &Rt, c: &Call<'_>) -> ObjRef {
    rt.fn_data(c.callee)[0].as_object().expect("watch closure data (framework bug)")
}

/// Source shape and whether the callback must always fire (`forceTrigger`).
fn source_kind(rt: &Rt, source: &Value, has_cb: bool) -> Res<(Kind, bool, bool)> {
    if is_ref(rt, source)? {
        return Ok((Kind::Ref, false, is_shallow(rt, source)?));
    }
    if is_reactive(rt, source)? {
        return Ok((Kind::Reactive, false, true));
    }
    if rt.is_array(source) {
        let len = array_len(rt, source)?;
        let mut force = false;
        for i in 0..len {
            let Value::Object(so) = source else { break };
            if rt.has_property(*so, &PropKey::Index(i as u32))? {
                let s = rt.get_v(source, &PropKey::Index(i as u32))?;
                if is_reactive(rt, &s)? || is_shallow(rt, &s)? {
                    force = true;
                    break;
                }
            }
        }
        return Ok((Kind::Array, true, force));
    }
    if rt.is_callable(source) {
        return Ok((if has_cb { Kind::FuncCb } else { Kind::FuncNoCb }, false, false));
    }
    Ok((Kind::Noop, false, false))
}

fn array_len(rt: &Rt, v: &Value) -> Res<usize> {
    let n = rt.to_number(&rt.get_str(v, "length")?)?;
    Ok(if n > 0.0 { n as usize } else { 0 })
}

/// `traverse(value, depth, seen)`: visits the value to `depth` levels, reading refs, arrays, Maps/Sets and plain objects.
pub(crate) fn traverse(rt: &Rt, value: &Value, depth: f64, seen: ObjRef) -> Res<()> {
    if depth <= 0.0 || !is_object(rt, value) {
        return Ok(());
    }
    if truthy(&rt.get_str(value, "__v_skip")?) {
        return Ok(());
    }
    let Value::Object(o) = value else { return Ok(()) };
    let prev = match try_seen_get(rt, seen, *o) {
        Some(p) if !p.is_nan() => p,
        _ => 0.0,
    };
    if prev >= depth {
        return Ok(());
    }
    try_seen_set(rt, seen, *o, depth);
    let depth = depth - 1.0;
    if is_ref(rt, value)? {
        let v = rt.get_str(value, "value")?;
        traverse(rt, &v, depth, seen)?;
    } else if rt.is_array(value) {
        let mut i = 0usize;
        loop {
            let len = rt.to_number(&rt.get_str(value, "length")?)?;
            if (i as f64) >= len {
                break;
            }
            let x = rt.get_v(value, &PropKey::Index(i as u32))?;
            traverse(rt, &x, depth, seen)?;
            i += 1;
        }
    } else if has_tag(rt, value, "Set")? || has_tag(rt, value, "Map")? {
        let for_each = rt.get_str(value, "forEach")?;
        let cb = native(rt, "", 1, traverse_each, vec![Value::Object(seen), num(depth)]);
        rt.call(&for_each, value, &[Value::Object(cb)])?;
    } else if has_tag(rt, value, "Object")? {
        for k in for_in_keys(rt, *o)? {
            let v = rt.get_v(value, &k)?;
            traverse(rt, &v, depth, seen)?;
        }
        for k in enumerable_symbol_keys(rt, *o)? {
            let v = rt.get_v(value, &k)?;
            traverse(rt, &v, depth, seen)?;
        }
    }
    Ok(())
}

fn traverse_each(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let seen = d[0].as_object().expect("seen (framework bug)");
    let depth = match d[1] {
        Value::Number(n) => n,
        _ => 0.0,
    };
    traverse(rt, &c.arg(0), depth, seen)?;
    Ok(Value::Undefined)
}

fn try_seen_get(rt: &Rt, seen: ObjRef, k: ObjRef) -> Option<f64> {
    let sn = node(rt, seen);
    let v = sn.seen().borrow().get(&k).copied();
    v
}

fn try_seen_set(rt: &Rt, seen: ObjRef, k: ObjRef, depth: f64) {
    let sn = node(rt, seen);
    sn.seen().borrow_mut().insert(k, depth);
}

/// A fresh seen-map object.
fn new_seen(rt: &Rt) -> ObjRef {
    alloc(rt, None, Node::Seen(RefCell::new(HashMap::new())))
}

/// `traverse(value, depth)` from a root (depth Infinity by default). Returns the value.
pub(crate) fn traverse_root(rt: &Rt, value: &Value, depth: f64) -> Res<Value> {
    let seen = new_seen(rt);
    traverse(rt, value, depth, seen)?;
    Ok(value.clone())
}

/// `reactiveGetter` of the watch base: a reactive source is walked to the watch's depth.
fn reactive_getter(rt: &Rt, src: &Value, deep: &Value) -> Res<Value> {
    if truthy(deep) {
        return Ok(src.clone());
    }
    let shallow = is_shallow(rt, src)?;
    if shallow || matches!(deep, Value::Bool(false)) || matches!(deep, Value::Number(n) if *n == 0.0) {
        return traverse_root(rt, src, 1.0);
    }
    traverse_root(rt, src, f64::INFINITY)
}

/// The Array source getter: `source.map(...)` with holes kept.
fn map_sources(rt: &Rt, watch: &Watch, source: &Value) -> Res<Value> {
    let Value::Object(so) = source else { return Ok(Value::Undefined) };
    let len = array_len(rt, source)?;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let key = PropKey::Index(i as u32);
        if rt.has_property(*so, &key)? {
            let s = rt.get_v(source, &key)?;
            let v = if is_ref(rt, &s)? {
                rt.get_str(&s, "value")?
            } else if is_reactive(rt, &s)? {
                reactive_getter(rt, &s, &watch.deep)?
            } else if rt.is_callable(&s) {
                if truthy(&watch.call) {
                    rt.call(&watch.call, &Value::Undefined, &[s.clone(), num(2.0)])?
                } else {
                    rt.call(&s, &Value::Undefined, &[])?
                }
            } else {
                Value::Undefined
            };
            out.push(v);
        } else {
            out.push(Value::Hole);
        }
    }
    Ok(Value::Object(rt.new_array(out)))
}

/// The effect's getter: reads the source as the watch's kind says, and walks it when deep.
fn getter_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let w = data_obj(rt, c);
    let wn = watch_of(rt, w);
    let wv = wn.watch();
    let base = match wv.kind {
        Kind::Ref => rt.get_str(&wv.source, "value")?,
        Kind::Reactive => reactive_getter(rt, &wv.source, &wv.deep)?,
        Kind::Array => map_sources(rt, wv, &wv.source)?,
        Kind::FuncCb => {
            if truthy(&wv.call) {
                rt.call(&wv.call, &Value::Undefined, &[wv.source.clone(), num(2.0)])?
            } else if wv.deep_wrap {
                rt.call(&wv.source, &Value::Undefined, &[])?
            } else {
                rt.call(&wv.source, c.this, &[])?
            }
        }
        Kind::FuncNoCb => {
            let cleanup = wv.on_stop.borrow().clone();
            if truthy(&cleanup) {
                pause_tracking(rt);
                let r = rt.call(&cleanup, &Value::Undefined, &[]);
                reset_tracking(rt);
                r?;
            }
            let st = state(rt);
            let current = st.active_watcher.get();
            st.active_watcher.set(wv.effect.get());
            let bc = wv.bound_cleanup.borrow().clone();
            let r = if truthy(&wv.call) {
                let args = Value::Object(rt.new_array(vec![bc]));
                rt.call(&wv.call, &Value::Undefined, &[wv.source.clone(), num(3.0), args])
            } else {
                rt.call(&wv.source, &Value::Undefined, &[bc])
            };
            st.active_watcher.set(current);
            r?
        }
        Kind::Noop => Value::Undefined,
    };
    if wv.deep_wrap {
        traverse_root(rt, &base, wv.depth)
    } else {
        Ok(base)
    }
}

/// The INITIAL-marker test of the old value (`oldValue === INITIAL_WATCHER_VALUE` and its first element).
fn old_at(rt: &Rt, old: &Old, i: usize) -> Res<Value> {
    Ok(match old {
        Old::Single(v) => v.clone(),
        Old::MultiInitial(n) => {
            if i < *n {
                Value::Uninit
            } else {
                Value::Undefined
            }
        }
        Old::Multi(arr) => rt.get_v(arr, &PropKey::Index(i as u32))?,
    })
}

/// `job(immediateFirstRun)`: runs the effect when it is dirty (or on the first run), then calls the callback if the value
/// changed (or always when deep, forced, or immediate).
fn job_impl(rt: &Rt, w: ObjRef, imm: bool) -> Res<Value> {
    let wn = watch_of(rt, w);
    let wv = wn.watch();
    let effect = wv.effect.get().expect("watch effect (framework bug)");
    let active = node(rt, effect).sub().flags.get() & ACTIVE != 0;
    if !active {
        return Ok(Value::Undefined);
    }
    let dirty = is_dirty(rt, effect)?;
    if !dirty && !imm {
        return Ok(Value::Undefined);
    }
    if !wv.has_cb {
        effect_run(rt, effect)?;
        return Ok(Value::Undefined);
    }
    let new_value = effect_run(rt, effect)?;
    let old = wv.old.borrow().clone();
    let changed = if imm || truthy(&wv.deep) || wv.force_trigger {
        true
    } else if wv.is_multi {
        let len = array_len(rt, &new_value)?;
        let mut any = false;
        for i in 0..len {
            let Value::Object(no) = &new_value else { break };
            if rt.has_property(*no, &PropKey::Index(i as u32))? {
                let v = rt.get_v(&new_value, &PropKey::Index(i as u32))?;
                let o = old_at(rt, &old, i)?;
                if has_changed(&v, &o) {
                    any = true;
                    break;
                }
            }
        }
        any
    } else {
        let o = old_at(rt, &old, 0)?;
        has_changed(&new_value, &o)
    };
    if !changed {
        return Ok(Value::Undefined);
    }
    let cleanup = wv.on_stop.borrow().clone();
    if truthy(&cleanup) {
        rt.call(&cleanup, &Value::Undefined, &[])?;
    }
    let st = state(rt);
    let current = st.active_watcher.get();
    st.active_watcher.set(Some(effect));
    let result = (|| -> Res<()> {
        let old_arg = match &old {
            Old::Single(Value::Uninit) => Value::Undefined,
            Old::Single(v) => v.clone(),
            Old::Multi(arr) => {
                let first = rt.get_v(arr, &PropKey::Index(0))?;
                if matches!(first, Value::Uninit) {
                    Value::Object(rt.new_array(Vec::new()))
                } else {
                    arr.clone()
                }
            }
            Old::MultiInitial(_) => Value::Object(rt.new_array(Vec::new())),
        };
        let bc = wv.bound_cleanup.borrow().clone();
        let args = [new_value.clone(), old_arg, bc];
        *wv.old.borrow_mut() = if wv.is_multi { Old::Multi(new_value.clone()) } else { Old::Single(new_value.clone()) };
        let cb = wv.cb.borrow().clone();
        if truthy(&wv.call) {
            let arr = Value::Object(rt.new_array(args.to_vec()));
            rt.call(&wv.call, &Value::Undefined, &[cb, num(3.0), arr])?;
        } else {
            rt.call(&cb, &Value::Undefined, &args)?;
        }
        Ok(())
    })();
    st.active_watcher.set(current);
    result?;
    Ok(Value::Undefined)
}

fn job_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let w = data_obj(rt, c);
    job_impl(rt, w, c.arg(0).to_boolean())?;
    Ok(Value::Undefined)
}

fn job_bound_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let w = data_obj(rt, c);
    job_impl(rt, w, true)?;
    Ok(Value::Undefined)
}

/// The effect's scheduler when a user scheduler is given: `() => scheduler(job, false)`.
fn sched_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let w = data_obj(rt, c);
    let wn = watch_of(rt, w);
    let wv = wn.watch();
    let job = wv.job.borrow().clone();
    rt.call(&wv.scheduler, &Value::Undefined, &[job, Value::Bool(false)])
}

/// `boundCleanup(fn)`: `onWatcherCleanup(fn, false, effect)`.
fn bound_cleanup_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let w = data_obj(rt, c);
    let wn = watch_of(rt, w);
    let effect = wn.watch().effect.get();
    on_watcher_cleanup(rt, c.arg(0), effect);
    Ok(Value::Undefined)
}

/// The cleanup of a watcher (`effect.onStop`): runs and forgets the cleanups registered on the effect.
fn on_stop_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let w = data_obj(rt, c);
    let wn = watch_of(rt, w);
    let wv = wn.watch();
    let Some(effect) = wv.effect.get() else { return Ok(Value::Undefined) };
    let en = node(rt, effect);
    let ef = en.effect();
    if ef.watch_cleanups.borrow().is_none() {
        return Ok(Value::Undefined);
    }
    if truthy(&wv.call) {
        let snapshot = ef.watch_cleanups.borrow().clone().unwrap_or_default();
        let arr = Value::Object(rt.new_array(snapshot));
        rt.call(&wv.call, &Value::Undefined, &[arr, num(4.0)])?;
    } else {
        let mut i = 0usize;
        loop {
            let f = ef.watch_cleanups.borrow().as_ref().and_then(|v| v.get(i).cloned());
            match f {
                Some(f) => {
                    rt.call(&f, &Value::Undefined, &[])?;
                    i += 1;
                }
                None => break,
            }
        }
    }
    *ef.watch_cleanups.borrow_mut() = None;
    Ok(Value::Undefined)
}

/// `onWatcherCleanup(fn, failSilently, owner)`: registers `fn` on the owner effect (no-op without an owner).
pub(crate) fn on_watcher_cleanup(rt: &Rt, f: Value, owner: Option<ObjRef>) {
    let Some(o) = owner else { return };
    let Some(n) = crate::node::try_node(rt, o) else { return };
    if !n.is_effect() {
        return;
    }
    let ef = n.effect();
    ef.watch_cleanups.borrow_mut().get_or_insert_with(Vec::new).push(f);
}

/// The `watchHandle` function: stops the effect and leaves the scope's list.
fn handle_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let w = data_obj(rt, c);
    let wn = watch_of(rt, w);
    let wv = wn.watch();
    let effect = wv.effect.get().expect("watch effect (framework bug)");
    effect_stop(rt, effect)?;
    if let Some(s) = wv.scope {
        let sn = node(rt, s);
        let sc = sn.scope();
        if sc.active.get() {
            let mut list = sc.effects.borrow_mut();
            if let Some(i) = list.iter().position(|x| *x == effect) {
                list.remove(i);
            }
        }
    }
    Ok(Value::Undefined)
}

/// `(...args) => { res = cb(...args); watchHandle(); return res }` for `once`.
fn once_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let orig = d[0].clone();
    let w = d[1].as_object().expect("watch (framework bug)");
    let res = rt.call(&orig, &Value::Undefined, c.args)?;
    let wn = watch_of(rt, w);
    let handle = wn.watch().handle.borrow().clone();
    rt.call(&handle, &Value::Undefined, &[])?;
    Ok(res)
}

fn pause_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = rt.fn_data(c.callee)[0].as_object().expect("effect (framework bug)");
    effect_pause(rt, e);
    Ok(Value::Undefined)
}

fn resume_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = rt.fn_data(c.callee)[0].as_object().expect("effect (framework bug)");
    effect_resume(rt, e)?;
    Ok(Value::Undefined)
}

/// The base `watch` (the reactivity-level watch that the runtime's watchers wrap). Creates the handle, runs the initial
/// evaluation and returns the handle.
pub fn watch_base(rt: &Rt, source: Value, cb: Option<Value>, opts: WatchOptions) -> Res<WatchHandle> {
    let cb = cb.unwrap_or(Value::Undefined);
    let has_cb = truthy(&cb);
    let (kind, is_multi, force) = source_kind(rt, &source, has_cb)?;
    let deep_wrap = has_cb && truthy(&opts.deep);
    let depth = if !deep_wrap {
        0.0
    } else if matches!(opts.deep, Value::Bool(true)) {
        f64::INFINITY
    } else {
        rt.to_number(&opts.deep)?
    };
    let scope = state(rt).active_scope.get();
    let old0 = if is_multi {
        Old::MultiInitial(array_len(rt, &source)?)
    } else {
        Old::Single(Value::Uninit)
    };
    let w = alloc(
        rt,
        None,
        Node::Watch(Watch {
            effect: Cell::new(None),
            source,
            kind,
            is_multi,
            force_trigger: force,
            deep: opts.deep.clone(),
            depth,
            deep_wrap,
            has_cb,
            cb: RefCell::new(cb.clone()),
            call: opts.call.clone(),
            scheduler: opts.scheduler.clone(),
            job: RefCell::new(Value::Undefined),
            handle: RefCell::new(Value::Undefined),
            bound_cleanup: RefCell::new(Value::Undefined),
            on_stop: RefCell::new(Value::Undefined),
            old: RefCell::new(old0),
            scope,
        }),
    );
    let handle = native(rt, "watchHandle", 0, handle_code, vec![Value::Object(w)]);
    *watch_of(rt, w).watch().handle.borrow_mut() = Value::Object(handle);
    if truthy(&opts.once) && has_cb {
        let wrapped = native(rt, "", 0, once_code, vec![cb.clone(), Value::Object(w)]);
        *watch_of(rt, w).watch().cb.borrow_mut() = Value::Object(wrapped);
    }
    let job = native(rt, "job", 1, job_code, vec![Value::Object(w)]);
    if truthy(&opts.augment_job) {
        rt.call(&opts.augment_job, &Value::Undefined, &[Value::Object(job)])?;
    }
    *watch_of(rt, w).watch().job.borrow_mut() = Value::Object(job);
    let getter = native(rt, "getter", 0, getter_code, vec![Value::Object(w)]);
    let effect = new_effect(rt, Value::Object(getter));
    watch_of(rt, w).watch().effect.set(Some(effect));
    let sched = if truthy(&opts.scheduler) {
        Value::Object(native(rt, "scheduler", 0, sched_code, vec![Value::Object(w)]))
    } else {
        Value::Object(job)
    };
    *node(rt, effect).effect().scheduler.borrow_mut() = sched;
    let bc = native(rt, "boundCleanup", 1, bound_cleanup_code, vec![Value::Object(w)]);
    *watch_of(rt, w).watch().bound_cleanup.borrow_mut() = Value::Object(bc);
    let on_stop = native(rt, "onStop", 0, on_stop_code, vec![Value::Object(w)]);
    *watch_of(rt, w).watch().on_stop.borrow_mut() = Value::Object(on_stop);
    *node(rt, effect).effect().on_stop.borrow_mut() = Value::Object(on_stop);
    if has_cb {
        if truthy(&opts.immediate) {
            job_impl(rt, w, true)?;
        } else {
            let first = effect_run(rt, effect)?;
            let wn = watch_of(rt, w);
            *wn.watch().old.borrow_mut() = if is_multi { Old::Multi(first) } else { Old::Single(first) };
        }
    } else if truthy(&opts.scheduler) {
        let bound = native(rt, "bound job", 0, job_bound_code, vec![Value::Object(w)]);
        rt.call(&opts.scheduler, &Value::Undefined, &[Value::Object(bound), Value::Bool(true)])?;
    } else {
        effect_run(rt, effect)?;
    }
    let pause = native(rt, "bound pause", 0, pause_code, vec![Value::Object(effect)]);
    let resume = native(rt, "bound resume", 0, resume_code, vec![Value::Object(effect)]);
    rt.define_data(handle, PropKey::from_str("pause"), Value::Object(pause), vn_value::Flags::ALL)?;
    rt.define_data(handle, PropKey::from_str("resume"), Value::Object(resume), vn_value::Flags::ALL)?;
    rt.define_data(handle, PropKey::from_str("stop"), Value::Object(handle), vn_value::Flags::ALL)?;
    Ok(WatchHandle { handle: Value::Object(handle) })
}

/// JS `watch(source, cb, options = EMPTY_OBJ)`: reads the options in the order of the destructuring.
fn js_watch(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let options = c.arg(2);
    let opts = if options.is_undefined() {
        WatchOptions::default()
    } else {
        WatchOptions {
            immediate: rt.get_str(&options, "immediate")?,
            deep: rt.get_str(&options, "deep")?,
            once: rt.get_str(&options, "once")?,
            scheduler: rt.get_str(&options, "scheduler")?,
            augment_job: rt.get_str(&options, "augmentJob")?,
            call: rt.get_str(&options, "call")?,
        }
    };
    let cb = c.arg(1);
    let handle = watch_base(rt, c.arg(0), Some(cb), opts)?;
    Ok(handle.handle)
}

/// JS `traverse(value, depth = Infinity, seen)`.
fn js_traverse(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let depth = if c.arg(1).is_undefined() { f64::INFINITY } else { rt.to_number(&c.arg(1))? };
    let seen = match c.arg(2) {
        Value::Object(o) => o,
        _ => new_seen(rt),
    };
    let value = c.arg(0);
    traverse(rt, &value, depth, seen)?;
    Ok(value)
}

fn js_on_watcher_cleanup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let owner = if c.arg(2).is_undefined() {
        state(rt).active_watcher.get()
    } else {
        c.arg(2).as_object()
    };
    on_watcher_cleanup(rt, c.arg(0), owner);
    Ok(Value::Undefined)
}

fn js_get_current_watcher(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(state(rt).active_watcher.get().map(Value::Object).unwrap_or(Value::Undefined))
}

/// Exports: `onWatcherCleanup` (public, V16) and `watch`, `traverse`, `getCurrentWatcher` (internal).
pub(crate) fn exported_public(rt: &Rt) -> Vec<(&'static str, Value)> {
    vec![("onWatcherCleanup", Value::Object(native(rt, "onWatcherCleanup", 1, js_on_watcher_cleanup, Vec::new())))]
}

pub(crate) fn exported_internal(rt: &Rt) -> Vec<(&'static str, Value)> {
    vec![
        ("watch", Value::Object(native(rt, "watch", 2, js_watch, Vec::new()))),
        ("traverse", Value::Object(native(rt, "traverse", 1, js_traverse, Vec::new()))),
        ("getCurrentWatcher", Value::Object(native(rt, "getCurrentWatcher", 0, js_get_current_watcher, Vec::new()))),
    ]
}
