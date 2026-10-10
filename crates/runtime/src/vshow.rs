//! `vShow` (runtime-dom 3.5.43): the display toggle. The original display is kept in `_vod` (the expando state of
//! `dom_events::Expando`) and the hidden flag in `_vsh`; `patchStyle` re-applies `display: none` after a style patch
//! (see dom_props.rs). Transitions come from `vnode.transition` (a JS object with `beforeEnter`, `enter`, `leave`),
//! which the transition WP supplies; when it is absent the display is set directly.

use vn_value::{Call, Res, Rt, Value};

use crate::dom_events::{ctx_of_handle, DomCtx};
use crate::host::HostNode;

/// `el.style.display = value ? el._vod : "none"; el._vsh = !value`.
fn set_display(ctx: &DomCtx, el: HostNode, value: bool) -> Res<()> {
    let vod = ctx.with_expando(el, |ex| ex.vod.clone());
    let mut host = ctx.host();
    if value {
        if let Some(v) = vod {
            if v.is_empty() {
                let _ = host.style_remove_property(el, "display");
            } else {
                let _ = host.style_set_property(el, "display", &v, false);
            }
        }
    } else {
        let _ = host.style_set_property(el, "display", "none", false);
    }
    ctx.with_expando(el, |ex| ex.vsh = !value);
    Ok(())
}

fn transition_call(ctx: &DomCtx, transition: &Value, method: &str, el: HostNode, extra: &[Value]) -> Res<()> {
    let rt = ctx.rt();
    let f = rt.get_str(transition, method)?;
    let mut args = vec![ctx.handle(el)];
    args.extend_from_slice(extra);
    rt.call(&f, transition, &args)?;
    Ok(())
}

/// `beforeMount(el, { value }, { transition })`.
pub fn before_mount(ctx: &DomCtx, el: HostNode, value: &Value, transition: Option<&Value>) -> Res<()> {
    let display = ctx.host().style_get_property(el, "display");
    let vod = if display == "none" { String::new() } else { display };
    ctx.with_expando(el, |ex| ex.vod = Some(vod));
    match transition {
        Some(t) if value.to_boolean() => transition_call(ctx, t, "beforeEnter", el, &[]),
        _ => set_display(ctx, el, value.to_boolean()),
    }
}

/// `mounted(el, { value }, { transition })`.
pub fn mounted(ctx: &DomCtx, el: HostNode, value: &Value, transition: Option<&Value>) -> Res<()> {
    if let Some(t) = transition {
        if value.to_boolean() {
            transition_call(ctx, t, "enter", el, &[])?;
        }
    }
    Ok(())
}

/// `updated(el, { value, oldValue }, { transition })`.
pub fn updated(ctx: &DomCtx, el: HostNode, value: &Value, old_value: &Value, transition: Option<&Value>) -> Res<()> {
    if value.to_boolean() == old_value.to_boolean() {
        return Ok(());
    }
    match transition {
        Some(t) => {
            if value.to_boolean() {
                transition_call(ctx, t, "beforeEnter", el, &[])?;
                set_display(ctx, el, true)?;
                transition_call(ctx, t, "enter", el, &[])
            } else {
                let done = done_callback(ctx, el);
                transition_call(ctx, t, "leave", el, &[done])
            }
        }
        None => set_display(ctx, el, value.to_boolean()),
    }
}

/// `beforeUnmount(el, { value })`.
pub fn before_unmount(ctx: &DomCtx, el: HostNode, value: &Value) -> Res<()> {
    set_display(ctx, el, value.to_boolean())
}

/// The `() => setDisplay(el, false)` callback passed to `transition.leave`.
fn done_callback(ctx: &DomCtx, el: HostNode) -> Value {
    let rt = ctx.rt();
    let f = rt.new_native_function(leave_done_code, "", 0, vec![ctx.handle(el)]);
    Value::Object(f)
}

fn leave_done_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let (ctx, el) = ctx_of_handle(rt, &data[0])?;
    set_display(&ctx, el, false)?;
    Ok(Value::Undefined)
}

fn transition_of(rt: &Rt, vnode: &Value) -> Res<Option<Value>> {
    let t = rt.get_str(vnode, "transition")?;
    Ok(if t.to_boolean() { Some(t) } else { None })
}

fn before_mount_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el) = ctx_of_handle(rt, &c.arg(0))?;
    let value = rt.get_str(&c.arg(1), "value")?;
    let transition = transition_of(rt, &c.arg(2))?;
    before_mount(&ctx, el, &value, transition.as_ref())?;
    Ok(Value::Undefined)
}

fn mounted_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el) = ctx_of_handle(rt, &c.arg(0))?;
    let value = rt.get_str(&c.arg(1), "value")?;
    let transition = transition_of(rt, &c.arg(2))?;
    mounted(&ctx, el, &value, transition.as_ref())?;
    Ok(Value::Undefined)
}

fn updated_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el) = ctx_of_handle(rt, &c.arg(0))?;
    let binding = c.arg(1);
    let value = rt.get_str(&binding, "value")?;
    let old_value = rt.get_str(&binding, "oldValue")?;
    let transition = transition_of(rt, &c.arg(2))?;
    updated(&ctx, el, &value, &old_value, transition.as_ref())?;
    Ok(Value::Undefined)
}

fn before_unmount_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el) = ctx_of_handle(rt, &c.arg(0))?;
    let value = rt.get_str(&c.arg(1), "value")?;
    before_unmount(&ctx, el, &value)?;
    Ok(Value::Undefined)
}

/// The `vShow` directive object of the runtime's `vue` module.
pub fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    let obj = rt.new_object();
    let _ = rt.define_data(obj, vn_value::PropKey::lit("name"), Value::String(vn_value::JsStr::from_str("show")), vn_value::Flags::ALL);
    rt.define_native_method(obj, "beforeMount", before_mount_code, 3);
    rt.define_native_method(obj, "mounted", mounted_code, 3);
    rt.define_native_method(obj, "updated", updated_code, 3);
    rt.define_native_method(obj, "beforeUnmount", before_unmount_code, 3);
    vec![("vShow", Value::Object(obj))]
}
