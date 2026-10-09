//! Transition hooks over Host nodes: JS callbacks, appear, cancellation, out-in and CSS classes/timing.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use vn_value::{payload_ref, Call, JsStr, NativeObject, ObjRef, PropKey, Res, Rt, Tracer, Value};
use crate::dom_events::{ctx_of_handle, DomCtx};
use crate::dom_props::{enumerable_keys, include_boolean_attr};
use crate::errors::{call_with_async_error_handling, code, ErrType};
use crate::instance::{current_instance, instance_of, LifeHook};
use crate::lifecycle::inject_hook;
use crate::props::put;
use crate::renderer::{array_len, builtin_fn, component_of, el_of, is_type, renderer, transition_of, ty_of, vnode_at};
use crate::scheduler::queue_post_render_effect;
use crate::vnode::{fields, is_same_vnode_type};

struct State {
    instance: ObjRef,
    mounted: Cell<bool>,
    unmounting: Cell<bool>,
    leaving: Cell<bool>,
    pending_leave: RefCell<Vec<(ObjRef, Value)>>,
    pending_enter: RefCell<Vec<(Value, Value)>>,
}

impl NativeObject for State {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.instance);
        for (vnode, callback) in self.pending_leave.borrow().iter() {
            t.mark(*vnode);
            t.mark_value(callback);
        }
        for (element, callback) in self.pending_enter.borrow().iter() {
            t.mark_value(element);
            t.mark_value(callback);
        }
    }
}

fn payload(rt: &Rt, o: ObjRef) -> Rc<dyn NativeObject> { rt.native_payload(o).expect("transition state") }

fn state_of<'a>(p: &'a Rc<dyn NativeObject>) -> &'a State { payload_ref::<State>(&**p).expect("transition state type") }

fn callback(rt: &Rt, name: &str, code: vn_value::CodeFn, d: Vec<Value>, len: u32) -> Value {
    Value::Object(rt.new_native_function(code, name, len, d))
}

fn setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let inst = current_instance(rt).expect("Transition instance");
    let o = rt.new_native_object(None, Rc::new(State {
        instance: inst,
        mounted: Cell::new(false),
        unmounting: Cell::new(false),
        leaving: Cell::new(false),
        pending_leave: RefCell::new(Vec::new()),
        pending_enter: RefCell::new(Vec::new()),
    }));
    inject_hook(rt, LifeHook::M, &callback(rt, "transitionMounted", mounted, vec![Value::Object(o)], 0), Some(inst), false)?;
    inject_hook(rt, LifeHook::Bum, &callback(rt, "transitionUnmounting", unmounting, vec![Value::Object(o)], 0), Some(inst), false)?;
    let base = rt.fn_data(c.callee).first().is_some_and(Value::to_boolean);
    Ok(callback(rt, "renderTransition", render, vec![Value::Object(o), c.arg(0), Value::Bool(base)], 0))
}

fn mounted(rt: &Rt, c: &Call<'_>) -> Res<Value> { state_of(&payload(rt, rt.fn_data(c.callee)[0].as_object().expect("state"))).mounted.set(true); Ok(Value::Undefined) }

fn unmounting(rt: &Rt, c: &Call<'_>) -> Res<Value> { state_of(&payload(rt, rt.fn_data(c.callee)[0].as_object().expect("state"))).unmounting.set(true); Ok(Value::Undefined) }

fn comment(rt: &Rt) -> Res<Value> { rt.call(&builtin_fn(rt, "vn-runtime.createVNode"), &Value::Undefined, &[crate::renderer::sym(rt, "v-cmt")]) }

fn flatten(rt: &Rt, children: &Value, out: &mut Vec<ObjRef>) -> Res<()> {
    for i in 0..array_len(rt, children)? {
        let v = vnode_at(rt, children, i)?;
        if is_type(rt, &ty_of(rt, v), "v-fgt") { flatten(rt, &crate::renderer::children_of(rt, v), out)?; }
        else { out.push(v); }
    }
    Ok(())
}

fn render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let [state, input_props, base] = &*data else { unreachable!("Transition render captures") };
    let o = state.as_object().expect("state");
    let payload = payload(rt, o);
    let state = state_of(&payload);
    let slots = instance_of(rt, state.instance).slots.borrow().clone();
    let slot = rt.get_str(&slots, "default")?;
    if !rt.is_callable(&slot) { return comment(rt); }
    let children = rt.call(&slot, &Value::Undefined, &[])?;
    let mut children_flat = Vec::new();
    flatten(rt, &children, &mut children_flat)?;
    let child = children_flat.iter().copied().find(|v| !is_type(rt, &ty_of(rt, *v), "v-cmt")).or_else(|| children_flat.first().copied());
    let Some(child) = child else { return comment(rt); };
    if state.leaving.get() { return comment(rt); }
    let props = Value::Object(rt.new_object());
    for key in enumerable_keys(rt, input_props)? {
        put(rt, &props, &key, rt.get_str(input_props, &key)?)?;
    }
    if base.to_boolean() {
        put(rt, &props, "css", Value::Bool(false))?;
    }
    let mode = rt.get_str(&props, "mode")?;
    if mode.to_boolean() && mode != Value::String(JsStr::from_str("default")) && mode != Value::String(JsStr::from_str("out-in")) {
        return Err(rt.throw_type_error("unsupported: Transition mode (supported: default, out-in)"));
    }
    let enter_hooks = hooks(rt, o, child, props.clone(), false)?;
    if !is_type(rt, &ty_of(rt, child), "v-cmt") { set_hooks(rt, child, enter_hooks)?; }
    let old = instance_of(rt, state.instance).sub_tree.borrow().as_object();
    if let Some(old) = old {
        if !is_type(rt, &ty_of(rt, old), "v-cmt") && !is_same_vnode_type(rt, old, child) {
            let out_in = mode == Value::String(JsStr::from_str("out-in")) && !is_type(rt, &ty_of(rt, child), "v-cmt");
            set_hooks(rt, old, hooks(rt, o, old, props, out_in)?)?;
            if out_in { state.leaving.set(true); return comment(rt); }
        }
    }
    Ok(Value::Object(child))
}

fn hooks(rt: &Rt, state: ObjRef, vnode: ObjRef, props: Value, after_leave: bool) -> Res<Value> {
    let hooks = Value::Object(rt.new_object());
    let d = vec![Value::Object(state), Value::Object(vnode), props.clone(), Value::Bool(after_leave)];
    for (name, code, len) in [("beforeEnter", before_enter as vn_value::CodeFn, 1), ("enter", enter, 1), ("leave", leave, 2), ("clone", clone_hooks, 1)] { put(rt, &hooks, name, callback(rt, name, code, d.clone(), len))?; }
    put(rt, &hooks, "persisted", Value::Bool(include_boolean_attr(&rt.get_str(&props, "persisted")?)))?;
    Ok(hooks)
}

fn clone_hooks(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let [state, _, props, after_leave] = &*data else { unreachable!("transition hook captures") };
    hooks(
        rt, state.as_object().expect("state"),
        c.arg(0).as_object().ok_or_else(|| rt.throw_type_error("transition clone requires vnode"))?,
        props.clone(), after_leave.to_boolean(),
    )
}

pub(crate) fn set_hooks(rt: &Rt, vnode: ObjRef, hooks: Value) -> Res<()> {
    if let Some(inst) = component_of(rt, vnode).as_object() {
        if let Some(sub) = instance_of(rt, inst).sub_tree.borrow().as_object() { set_hooks(rt, sub, hooks.clone())?; }
    }
    fields(rt, vnode, |f| f.transition = hooks);
    Ok(())
}

fn appear(rt: &Rt, state: &State, props: &Value) -> Res<Option<bool>> {
    if state.mounted.get() { return Ok(Some(false)); }
    Ok(if include_boolean_attr(&rt.get_str(props, "appear")?) { Some(true) } else { None })
}

fn hook_prop(rt: &Rt, props: &Value, stage: &str, appear: bool) -> Res<Value> {
    let name = if appear { format!("on{}Appear", stage) } else { format!("on{}Enter", stage) };
    let v = rt.get_str(props, &name)?;
    if appear && !v.to_boolean() { rt.get_str(props, &format!("on{}Enter", stage)) } else { Ok(v) }
}

fn call_hook(rt: &Rt, state: &State, f: &Value, args: &[Value]) -> Res<()> {
    if f.to_boolean() { call_with_async_error_handling(rt, f, Some(state.instance), ErrType::Code(code::TRANSITION_HOOK), Some(args))?; }
    Ok(())
}

fn explicit(rt: &Rt, hook: &Value) -> Res<bool> {
    if rt.is_array(hook) {
        for i in 0..array_len(rt, hook)? { if explicit(rt, &rt.get_v(hook, &PropKey::Index(i as u32))?)? { return Ok(true); } }
        Ok(false)
    } else { Ok(hook.to_boolean() && rt.to_number(&rt.get_str(hook, "length")?)? > 1.0) }
}

fn css(rt: &Rt, props: &Value) -> Res<bool> { Ok(rt.get_str(props, "css")? != Value::Bool(false)) }

fn classes(rt: &Rt, props: &Value, phase: &str) -> Res<[String; 3]> {
    let name = rt.get_str(props, "name")?;
    let name = if name.is_nullish() { "v".to_string() } else { rt.to_string(&name)?.to_rust_string_lossy() };
    let mut result = Vec::new();
    for suffix in ["From", "Active", "To"] {
        let v = rt.get_str(props, &format!("{phase}{suffix}Class"))?;
        let fallback = if phase == "appear" { rt.get_str(props, &format!("enter{suffix}Class"))? } else { Value::Undefined };
        let v = if v.is_nullish() { fallback } else { v };
        result.push(if v.is_nullish() { format!("{name}-{}-{}", if phase == "appear" { "enter" } else { phase }, suffix.to_lowercase()) } else { rt.to_string(&v)?.to_rust_string_lossy() });
    }
    Ok(result.try_into().expect("three classes"))
}

fn class(ctx: &DomCtx, el: crate::host::HostNode, name: &str, add: bool) -> Res<()> {
    for token in name.split_whitespace() {
        let result = if add { ctx.host().class_list_add(el, token) } else { ctx.host().class_list_remove(el, token) };
        result.map_err(|e| crate::dom_events::host_error(ctx.rt(), e))?;
    }
    ctx.with_expando(el, |ex| {
        if add { let values = ex.vtc.get_or_insert_with(Vec::new); if !values.iter().any(|v| v == name) { values.push(name.to_string()); } }
        else if let Some(values) = ex.vtc.as_mut() { values.retain(|v| v != name); if values.is_empty() { ex.vtc = None; } }
    });
    Ok(())
}

fn before_enter(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let [state, vnode, props, _] = &*data else { unreachable!("transition hook captures") };
    let payload = payload(rt, state.as_object().expect("state"));
    let state = state_of(&payload);
    let Some(is_appear) = appear(rt, state, props)? else { return Ok(Value::Undefined); };
    let vnode = vnode.as_object().expect("vnode");
    let pending = state.pending_leave.borrow().clone();
    for (old, done) in pending {
        if el_of(rt, old) == crate::dom_events::node_of_handle(rt, &c.arg(0)) {
            rt.call(&done, &Value::Undefined, &[Value::Bool(true)])?;
        } else if is_same_vnode_type(rt, old, vnode) {
            rt.call(&done, &Value::Undefined, &[])?;
        }
    }
    call_hook(rt, state, &hook_prop(rt, props, "Before", is_appear)?, &[c.arg(0)])?;
    if css(rt, props)? {
        let (ctx, el) = ctx_of_handle(rt, &c.arg(0))?;
        let names = classes(rt, props, if is_appear { "appear" } else { "enter" })?;
        class(&ctx, el, &names[0], true)?;
        class(&ctx, el, &names[1], true)?;
    }
    Ok(Value::Undefined)
}

struct Done {
    called: Cell<bool>,
}

impl NativeObject for Done {
    fn trace(&self, _: &mut Tracer) {}
}

fn make_done(rt: &Rt, d: &[Value], el: Value, remove: Value, is_appear: bool, leaving: bool) -> Value {
    let done = rt.new_native_object(None, Rc::new(Done { called: Cell::new(false) }));
    let mut data = d.to_vec();
    data.extend([el, remove, Value::Bool(is_appear), Value::Bool(leaving), Value::Object(done)]);
    callback(rt, "transitionDone", done_code, data, 1)
}

fn enter(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let [state, _, props, _] = &*data else { unreachable!("transition hook captures") };
    let payload = payload(rt, state.as_object().expect("state"));
    let state = state_of(&payload);
    let Some(is_appear) = appear(rt, state, props)? else { return Ok(Value::Undefined); };
    let done = make_done(rt, &data, c.arg(0), Value::Undefined, is_appear, false);
    state.pending_enter.borrow_mut().push((c.arg(0), done.clone()));
    let hook = hook_prop(rt, props, "", is_appear)?;
    call_hook(rt, state, &hook, &[c.arg(0), done.clone()])?;
    if css(rt, props)? {
        next_frame(rt, &data, c.arg(0), done, is_appear, false, explicit(rt, &hook)?)?;
    } else if !explicit(rt, &hook)? {
        rt.call(&done, &Value::Undefined, &[])?;
    }
    Ok(Value::Undefined)
}

fn leave(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let [state, vnode, props, _] = &*data else { unreachable!("transition hook captures") };
    let payload = payload(rt, state.as_object().expect("state"));
    let state = state_of(&payload);
    let el = c.arg(0);
    let pending = state.pending_enter.borrow().clone();
    for (node, done) in pending {
        if vn_value::strict_eq(&node, &el) {
            rt.call(&done, &Value::Undefined, &[Value::Bool(true)])?;
        }
    }
    if state.unmounting.get() {
        rt.call(&c.arg(1), &Value::Undefined, &[])?;
        return Ok(Value::Undefined);
    }
    call_hook(rt, state, &rt.get_str(props, "onBeforeLeave")?, &[el.clone()])?;
    let done = make_done(rt, &data, el.clone(), c.arg(1), false, true);
    state.pending_leave.borrow_mut().push((vnode.as_object().expect("vnode"), done.clone()));
    let hook = rt.get_str(props, "onLeave")?;
    if css(rt, props)? {
        let (ctx, node) = ctx_of_handle(rt, &el)?;
        let names = classes(rt, props, "leave")?;
        class(&ctx, node, &names[0], true)?;
        class(&ctx, node, &names[1], true)?;
    }
    call_hook(rt, state, &hook, &[el.clone(), done.clone()])?;
    if css(rt, props)? {
        next_frame(rt, &data, el, done, false, true, explicit(rt, &hook)?)?;
    } else if !explicit(rt, &hook)? {
        rt.call(&done, &Value::Undefined, &[])?;
    }
    Ok(Value::Undefined)
}

fn done_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let [state, _, props, after_leave, el, remove, is_appear, leaving, completion] = &*data
        else { unreachable!("transition completion captures") };
    let done_payload = rt.native_payload(completion.as_object().expect("done state")).expect("done state");
    let done = payload_ref::<Done>(&*done_payload).expect("done type");
    if done.called.replace(true) {
        return Ok(Value::Undefined);
    }
    let payload = payload(rt, state.as_object().expect("state"));
    let state = state_of(&payload);
    let leaving = leaving.to_boolean();
    let is_appear = is_appear.to_boolean();
    if css(rt, props)? {
        let (ctx, node) = ctx_of_handle(rt, el)?;
        let phase = if leaving { "leave" } else if is_appear { "appear" } else { "enter" };
        for name in classes(rt, props, phase)? {
            class(&ctx, node, &name, false)?;
        }
    }
    let cancelled = c.arg(0).to_boolean();
    if leaving {
        state.pending_leave.borrow_mut().retain(|(_, f)| !vn_value::strict_eq(f, &Value::Object(c.callee)));
        rt.call(remove, &Value::Undefined, &[])?;
        let hook = rt.get_str(props, if cancelled { "onLeaveCancelled" } else { "onAfterLeave" })?;
        call_hook(rt, state, &hook, &[el.clone()])?;
        if after_leave.to_boolean() {
            state.leaving.set(false);
            let rec = instance_of(rt, state.instance);
            if !rec.is_unmounted.get() {
                rt.call(&rec.update.borrow().clone(), &Value::Undefined, &[])?;
            }
        }
    } else {
        state.pending_enter.borrow_mut().retain(|(_, f)| !vn_value::strict_eq(f, &Value::Object(c.callee)));
        let hook = if cancelled {
            hook_prop(rt, props, "Cancelled", is_appear)?
        } else {
            hook_prop(rt, props, "After", is_appear)?
        };
        // Cancelled hook names place the suffix after Enter/Appear.
        let hook = if cancelled {
            rt.get_str(props, if is_appear { "onAppearCancelled" } else { "onEnterCancelled" })?
        } else { hook };
        call_hook(rt, state, &hook, &[el.clone()])?;
    }
    Ok(Value::Undefined)
}

fn next_frame(rt: &Rt, captures: &[Value], el: Value, done: Value, is_appear: bool, leaving: bool, explicit: bool) -> Res<()> {
    let mut data = captures.to_vec();
    data.extend([el, done, Value::Bool(is_appear), Value::Bool(leaving), Value::Bool(explicit), Value::Number(0.0)]);
    raf(rt, callback(rt, "transitionFrame", frame, data, 0))?;
    Ok(())
}

fn raf(rt: &Rt, f: Value) -> Res<Value> {
    let global = Value::Object(rt.intrinsic(vn_value::Intrinsic::Global));
    let raf = rt.get_str(&global, "requestAnimationFrame")?;
    if !rt.is_callable(&raf) { return Err(rt.throw_type_error("this host has no animation frame driver")); }
    rt.call(&raf, &Value::Undefined, &[f])
}

fn frame(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let mut data = rt.fn_data(c.callee).to_vec();
    let [_, _, props, _, el, done, is_appear, leaving, explicit, frame_number] = data.as_slice()
        else { unreachable!("transition frame captures") };
    let done_data = rt.fn_data(done.as_object().expect("done fn"));
    let completion = done_data.last().expect("done state capture");
    let done_payload = rt.native_payload(completion.as_object().expect("done state")).expect("done state");
    if payload_ref::<Done>(&*done_payload).expect("done type").called.get() {
        return Ok(Value::Undefined);
    }
    if *frame_number == Value::Number(0.0) {
        *data.last_mut().expect("frame number capture") = Value::Number(1.0);
        raf(rt, callback(rt, "transitionFrame", frame, data, 0))?;
        return Ok(Value::Undefined);
    }
    let (ctx, node) = ctx_of_handle(rt, el)?;
    let leaving = leaving.to_boolean();
    let phase = if leaving { "leave" } else if is_appear.to_boolean() { "appear" } else { "enter" };
    let names = classes(rt, props, phase)?;
    class(&ctx, node, &names[0], false)?;
    class(&ctx, node, &names[2], true)?;
    if !explicit.to_boolean() {
        let duration = rt.get_str(props, "duration")?;
        let duration = if matches!(duration, Value::Object(_)) {
            rt.get_str(&duration, if leaving { "leave" } else { "enter" })?
        } else { duration };
        let ms = if !duration.is_nullish() {
            rt.to_number(&duration)?
        } else {
            computed_timeout(&ctx, node, &rt.get_str(props, "type")?)?
        };
        if ms > 0.0 {
            crate::timers::timeout(rt, done.clone(), Value::Number(ms))?;
        } else {
            rt.call(done, &Value::Undefined, &[])?;
        }
    }
    Ok(Value::Undefined)
}

fn computed_timeout(ctx: &DomCtx, el: crate::host::HostNode, expected: &Value) -> Res<f64> {
    let kind = if expected.is_nullish() { None } else { Some(ctx.rt().to_string(expected)?.to_rust_string_lossy()) };
    let mut timeout: f64 = 0.0;
    for prefix in ["transition", "animation"] {
        if kind.as_ref().is_some_and(|k| k != prefix) { continue; }
        let duration = ctx.host().computed_style_property(el, &format!("{prefix}-duration")).map_err(|e| crate::dom_events::host_error(ctx.rt(), e))?;
        let delay = ctx.host().computed_style_property(el, &format!("{prefix}-delay")).map_err(|e| crate::dom_events::host_error(ctx.rt(), e))?;
        let durations = times_ms(&duration);
        let delays = times_ms(&delay);
        for (i, duration) in durations.iter().enumerate() {
            timeout = timeout.max(duration + delays[i % delays.len()]);
        }
    }
    Ok(if timeout > 0.0 { timeout + 1.0 } else { 0.0 })
}

fn times_ms(value: &str) -> Vec<f64> {
    value.split(',').map(|time| {
        let time = time.trim();
        if let Some(ms) = time.strip_suffix("ms") {
            ms.parse::<f64>().unwrap_or(0.0)
        } else {
            time.trim_end_matches('s').parse::<f64>().unwrap_or(0.0) * 1000.0
        }
    }).collect()
}

pub(crate) fn invoke(rt: &Rt, vnode: ObjRef, method: &str, extra: &[Value]) -> Res<()> {
    let t = transition_of(rt, vnode);
    if !t.to_boolean() || rt.get_str(&t, "persisted")?.to_boolean() { return Ok(()); }
    let el = renderer(rt)?.ctx.handle(el_of(rt, vnode).ok_or_else(|| rt.throw_type_error("transition element missing"))?);
    let mut args = vec![el]; args.extend_from_slice(extra);
    rt.call(&rt.get_str(&t, method)?, &t, &args)?;
    Ok(())
}

pub(crate) fn queue_enter(rt: &Rt, vnode: ObjRef) -> Res<()> {
    let f = callback(rt, "enterTransition", enter_step, vec![Value::Object(vnode)], 0);
    queue_post_render_effect(rt, &f, &Value::Null)
}

fn enter_step(rt: &Rt, c: &Call<'_>) -> Res<Value> { invoke(rt, rt.fn_data(c.callee)[0].as_object().expect("transition vnode"), "enter", &[])?; Ok(Value::Undefined) }

pub(crate) fn remove(rt: &Rt, vnode: ObjRef) -> Res<()> {
    let done = callback(rt, "removeTransitionNode", remove_node, vec![Value::Object(vnode)], 0);
    invoke(rt, vnode, "leave", &[done])
}

fn remove_node(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let v = rt.fn_data(c.callee)[0].as_object().expect("vnode");
    if let Some(el) = el_of(rt, v) { renderer(rt)?.rm(el)?; }
    Ok(Value::Undefined)
}

pub(crate) fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    let mut out = Vec::new();
    for name in ["Transition", "BaseTransition"] {
        let comp = Value::Object(rt.new_object());
        put(rt, &comp, "name", Value::String(JsStr::from_str(name))).expect("Transition definition");
        put(rt, &comp, "inheritAttrs", Value::Bool(false)).expect("Transition definition");
        let mut names = vec!["mode", "appear", "persisted", "css", "name", "type", "duration", "enterFromClass", "enterActiveClass", "enterToClass", "appearFromClass", "appearActiveClass", "appearToClass", "leaveFromClass", "leaveActiveClass", "leaveToClass"];
        names.extend(["onBeforeEnter", "onEnter", "onAfterEnter", "onEnterCancelled", "onBeforeLeave", "onLeave", "onAfterLeave", "onLeaveCancelled", "onBeforeAppear", "onAppear", "onAfterAppear", "onAppearCancelled"]);
        put(rt, &comp, "props", Value::Object(rt.new_array(names.iter().map(|n| Value::String(JsStr::from_str(n))).collect()))).expect("Transition definition");
        put(rt, &comp, "setup", callback(rt, "setup", setup, vec![Value::Bool(name == "BaseTransition")], 2)).expect("Transition definition");
        out.push((name, comp));
    }
    out
}
