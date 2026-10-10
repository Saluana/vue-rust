//! v-model directives (runtime-dom 3.5.43): `vModelText`, `vModelCheckbox`, `vModelRadio`, `vModelSelect` and
//! `vModelDynamic`, with their `created` / `mounted` / `beforeUpdate` / `updated` hooks.
//!
//! The hooks take `(el, binding, vnode, prevVNode)` with `binding = { value, oldValue, modifiers }` and
//! `vnode = { props }`, as the runtime passes them. Element state Vue keeps as expandos (`_assign`, `_modelValue`,
//! `_initialValue`, `_pendingValue`, `_trueValue`, `composing`, ...) lives in `dom_events::Expando`. Listeners are
//! native closures registered on the element, so they run through the same event sink as user handlers.
//! `nextTick` (the clear of `_pendingValue` after a select change) is queued and run by `DomCtx::flush_next_tick`,
//! which the scheduler calls.

use vn_value::{Call, CodeFn, Intrinsic, JsStr, PropKey, Res, Rt, Value};

use crate::dom_events::{array_items, ctx_of_handle, js_string, to_host_value, DomCtx};
use crate::dom_props::{has_tag, loose_equal, loose_index_of, loose_to_number};
use crate::host::{HostNode, ListenerOpts};

/// The modifiers and values of a v-model hook's binding argument.
struct Binding {
    value: Value,
    old_value: Value,
    lazy: bool,
    trim: bool,
    number: bool,
}

fn binding_of(rt: &Rt, b: &Value) -> Res<Binding> {
    let value = rt.get_str(b, "value")?;
    let old_value = rt.get_str(b, "oldValue")?;
    let mods = rt.get_str(b, "modifiers")?;
    let flag = |name: &str| -> Res<bool> {
        if mods.is_nullish() {
            return Ok(false);
        }
        Ok(rt.get_str(&mods, name)?.to_boolean())
    };
    Ok(Binding { value, old_value, lazy: flag("lazy")?, trim: flag("trim")?, number: flag("number")? })
}

fn props_of(rt: &Rt, vnode: &Value) -> Res<Value> {
    rt.get_str(vnode, "props")
}

/// `getModelAssigner(vnode)`: `vnode.props["onUpdate:modelValue"] || false`.
fn model_assigner(rt: &Rt, vnode: &Value) -> Res<Value> {
    let props = props_of(rt, vnode)?;
    let f = rt.get_str(&props, "onUpdate:modelValue")?;
    Ok(if f.to_boolean() { f } else { Value::Bool(false) })
}

/// `el[assignKey](value)`: the callback is called with `el` as `this`; an array of callbacks is called in order.
fn call_assign(ctx: &DomCtx, el: HostNode, assign: &Value, value: Value) -> Res<()> {
    let rt = ctx.rt();
    if rt.is_array(assign) {
        for f in array_items(rt, assign)? {
            rt.call(&f, &Value::Undefined, std::slice::from_ref(&value))?;
        }
        return Ok(());
    }
    let this = ctx.handle(el);
    rt.call(assign, &this, &[value])?;
    Ok(())
}

fn assign_of(ctx: &DomCtx, el: HostNode) -> Value {
    ctx.with_expando(el, |ex| ex.assign.clone()).unwrap_or(Value::Bool(false))
}

/// `castValue(value, trim, number)`.
fn cast_value(rt: &Rt, value: Value, trim: bool, number: bool) -> Res<Value> {
    let mut v = value;
    if trim {
        let s = js_string(rt, &v)?;
        v = Value::String(JsStr::from_str(s.trim()));
    }
    if number {
        v = loose_to_number(rt, &v)?;
    }
    Ok(v)
}

fn host_prop(ctx: &DomCtx, el: HostNode, name: &str) -> Value {
    match ctx.host().get_property(el, name) {
        Ok(hv) => crate::dom_events::host_to_js(hv),
        Err(_) => Value::Undefined,
    }
}

fn set_host_prop(ctx: &DomCtx, el: HostNode, name: &str, v: &Value) -> Res<()> {
    let rt = ctx.rt();
    let hv = to_host_value(rt, v)?;
    ctx.host().set_property(el, name, hv).map_err(|e| crate::dom_events::host_error(rt, e))
}

fn element_value(ctx: &DomCtx, el: HostNode) -> Value {
    host_prop(ctx, el, "value")
}

fn is_set(rt: &Rt, v: &Value) -> Res<bool> {
    if !matches!(v, Value::Object(_)) {
        return Ok(false);
    }
    has_tag(rt, v, "Set")
}

/// `new Set(items)`.
fn new_set(rt: &Rt, items: Value) -> Res<Value> {
    let global = Value::Object(rt.intrinsic(Intrinsic::Global));
    let ctor = rt.get_str(&global, "Set")?;
    rt.construct(&ctor, &[items], &ctor)
}

fn method(rt: &Rt, obj: &Value, name: &str, args: &[Value]) -> Res<Value> {
    let f = rt.get_str(obj, name)?;
    rt.call(&f, obj, args)
}

/// `_value` if the element has it (set by patchDOMProp), else the element's `value` (`getValue`).
fn get_value(ctx: &DomCtx, el: HostNode) -> Value {
    match ctx.with_expando(el, |ex| ex.value.clone()) {
        Some(v) => v,
        None => element_value(ctx, el),
    }
}

/// `getCheckboxValue(el, checked)`: `_trueValue` / `_falseValue` when set, else the boolean itself.
fn get_checkbox_value(ctx: &DomCtx, el: HostNode, checked: bool) -> Value {
    let v = ctx.with_expando(el, |ex| if checked { ex.true_value.clone() } else { ex.false_value.clone() });
    v.unwrap_or(Value::Bool(checked))
}

/// The root of `el` in the host: the document node when the element is connected.
fn is_connected_root(ctx: &DomCtx, el: HostNode) -> bool {
    let host = ctx.host();
    let mut cur = el;
    while let Some(p) = host.parent_node(cur) {
        cur = p;
    }
    host.document_node() == Some(cur)
}

// ---- vModelText ----

fn text_listener_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let (ctx, el) = ctx_of_handle(rt, &data[0])?;
    let trim = data[1].to_boolean();
    let cast = data[2].to_boolean();
    let e = c.arg(0);
    let target = rt.get_str(&e, "target")?;
    let (tctx, tnode) = ctx_of_handle(rt, &target)?;
    if tctx.with_expando(tnode, |ex| ex.composing) {
        return Ok(Value::Undefined);
    }
    let raw = element_value(&ctx, el);
    let v = cast_value(rt, raw, trim, cast)?;
    let assign = assign_of(&ctx, el);
    call_assign(&ctx, el, &assign, v)?;
    Ok(Value::Undefined)
}

fn text_change_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let (ctx, el) = ctx_of_handle(rt, &data[0])?;
    let trim = data[1].to_boolean();
    let cast = data[2].to_boolean();
    let raw = element_value(&ctx, el);
    let v = cast_value(rt, raw, trim, cast)?;
    set_host_prop(&ctx, el, "value", &v)?;
    Ok(Value::Undefined)
}

fn composition_start_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = c.arg(0);
    let target = rt.get_str(&e, "target")?;
    let (ctx, node) = ctx_of_handle(rt, &target)?;
    ctx.with_expando(node, |ex| ex.composing = true);
    Ok(Value::Undefined)
}

/// `onCompositionEnd(e)`: when a composition was in progress, dispatch an `input` event on the target.
fn composition_end_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let e = c.arg(0);
    let target = rt.get_str(&e, "target")?;
    let (ctx, node) = ctx_of_handle(rt, &target)?;
    let composing = ctx.with_expando(node, |ex| ex.composing);
    if composing {
        ctx.with_expando(node, |ex| ex.composing = false);
        let mut host = ctx.host();
        host.dispatch_event(node, vn_dom::EventInit::new("input", vn_dom::EventKind::Event))
            .map_err(|e| crate::dom_events::host_error(rt, e))?;
    }
    Ok(Value::Undefined)
}

fn add_listener(ctx: &DomCtx, el: HostNode, ty: &str, code: vn_value::CodeFn, data: Vec<Value>) -> Res<()> {
    let rt = ctx.rt();
    let f = rt.new_native_function(code, "listener", 1, data);
    ctx.add_listener(el, ty, f, ListenerOpts::default())?;
    Ok(())
}

fn text_created(ctx: &DomCtx, el: HostNode, b: &Binding, vnode: &Value) -> Res<()> {
    let rt = ctx.rt();
    let has_parent = ctx.host().parent_node(el).is_some();
    if has_parent {
        let ty = js_string(rt, &host_prop(ctx, el, "type"))?;
        if ty == "text" {
            let d = js_string(rt, &host_prop(ctx, el, "defaultValue"))?;
            ctx.with_expando(el, |ex| ex.initial_value = Some(d.replace(['\r', '\n'], "")));
        } else if ty == "textarea" {
            let d = js_string(rt, &host_prop(ctx, el, "defaultValue"))?;
            ctx.with_expando(el, |ex| ex.initial_value = Some(d.replace("\r\n", "\n").replace('\r', "\n")));
        }
    }
    let assign = model_assigner(rt, vnode)?;
    ctx.with_expando(el, |ex| ex.assign = Some(assign));
    let cast = b.number || {
        let props = props_of(rt, vnode)?;
        props.to_boolean() && js_string(rt, &rt.get_str(&props, "type")?)? == "number"
    };
    let handle = ctx.handle(el);
    let data = vec![handle.clone(), Value::Bool(b.trim), Value::Bool(cast)];
    let input_ty = if b.lazy { "change" } else { "input" };
    add_listener(ctx, el, input_ty, text_listener_code, data.clone())?;
    if b.trim || cast {
        add_listener(ctx, el, "change", text_change_code, data)?;
    }
    if !b.lazy {
        add_listener(ctx, el, "compositionstart", composition_start_code, Vec::new())?;
        add_listener(ctx, el, "compositionend", composition_end_code, Vec::new())?;
        add_listener(ctx, el, "change", composition_end_code, Vec::new())?;
    }
    Ok(())
}

fn text_mounted(ctx: &DomCtx, el: HostNode, b: &Binding) -> Res<()> {
    let rt = ctx.rt();
    let new_value = if b.value.is_nullish() { Value::String(JsStr::from_str("")) } else { b.value.clone() };
    let initial = ctx.with_expando(el, |ex| ex.initial_value.take());
    let ty = js_string(rt, &host_prop(ctx, el, "type"))?;
    match initial {
        Some(init) if (ty == "text" || ty == "textarea") && js_string(rt, &element_value(ctx, el))? != init => {
            let raw = element_value(ctx, el);
            let v = cast_value(rt, raw, b.trim, b.number)?;
            let assign = assign_of(ctx, el);
            call_assign(ctx, el, &assign, v)?;
        }
        _ => set_host_prop(ctx, el, "value", &new_value)?,
    }
    Ok(())
}

fn text_before_update(ctx: &DomCtx, el: HostNode, b: &Binding, vnode: &Value) -> Res<()> {
    let rt = ctx.rt();
    let assign = model_assigner(rt, vnode)?;
    ctx.with_expando(el, |ex| ex.assign = Some(assign));
    if ctx.with_expando(el, |ex| ex.composing) {
        return Ok(());
    }
    let raw = element_value(ctx, el);
    let ty = js_string(rt, &host_prop(ctx, el, "type"))?;
    let raw_text = js_string(rt, &raw)?;
    // /^0\d/.test(el.value)
    let starts_zero_digit = raw_text.len() >= 2 && raw_text.starts_with('0') && raw_text.as_bytes()[1].is_ascii_digit();
    let el_value = if (b.number || ty == "number") && !starts_zero_digit { loose_to_number(rt, &raw)? } else { raw.clone() };
    let new_value = if b.value.is_nullish() { Value::String(JsStr::from_str("")) } else { b.value.clone() };
    if vn_value::strict_eq(&el_value, &new_value) {
        return Ok(());
    }
    let focused = ctx.host().active_element() == Some(el);
    if is_connected_root(ctx, el) && focused && ty != "range" {
        if b.lazy && vn_value::strict_eq(&b.value, &b.old_value) {
            return Ok(());
        }
        if b.trim {
            let trimmed = js_string(rt, &raw)?;
            if vn_value::strict_eq(&Value::String(JsStr::from_str(trimmed.trim())), &new_value) {
                return Ok(());
            }
        }
    }
    set_host_prop(ctx, el, "value", &new_value)
}

// ---- vModelCheckbox ----

fn checkbox_change_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let (ctx, el) = ctx_of_handle(rt, &data[0])?;
    let model_value = ctx.with_expando(el, |ex| ex.model_value.clone()).unwrap_or(Value::Undefined);
    let element_value = get_value(&ctx, el);
    let checked = host_prop(&ctx, el, "checked").to_boolean();
    let assign = assign_of(&ctx, el);
    if rt.is_array(&model_value) {
        let items = array_items(rt, &model_value)?;
        let index = loose_index_of(rt, &items, &element_value)?;
        let found = index.is_some();
        if checked && !found {
            let next = method(rt, &model_value, "concat", std::slice::from_ref(&element_value))?;
            call_assign(&ctx, el, &assign, next)?;
        } else if !checked && found {
            let idx = index.unwrap_or(0);
            let filtered: Vec<Value> = items.into_iter().enumerate().filter(|(i, _)| *i != idx).map(|(_, v)| v).collect();
            call_assign(&ctx, el, &assign, Value::Object(rt.new_array(filtered)))?;
        }
    } else if is_set(rt, &model_value)? {
        let cloned = new_set(rt, model_value.clone())?;
        if checked {
            method(rt, &cloned, "add", std::slice::from_ref(&element_value))?;
        } else {
            method(rt, &cloned, "delete", std::slice::from_ref(&element_value))?;
        }
        call_assign(&ctx, el, &assign, cloned)?;
    } else {
        let v = get_checkbox_value(&ctx, el, checked);
        call_assign(&ctx, el, &assign, v)?;
    }
    Ok(Value::Undefined)
}

/// `setChecked(el, { value, oldValue }, vnode)`.
fn set_checked(ctx: &DomCtx, el: HostNode, b: &Binding, vnode: &Value) -> Res<()> {
    let rt = ctx.rt();
    ctx.with_expando(el, |ex| ex.model_value = Some(b.value.clone()));
    let checked = if rt.is_array(&b.value) {
        let items = array_items(rt, &b.value)?;
        let props = props_of(rt, vnode)?;
        let wanted = rt.get_str(&props, "value")?;
        loose_index_of(rt, &items, &wanted)?.is_some()
    } else if is_set(rt, &b.value)? {
        let props = props_of(rt, vnode)?;
        let wanted = rt.get_str(&props, "value")?;
        method(rt, &b.value, "has", std::slice::from_ref(&wanted))?.to_boolean()
    } else {
        if vn_value::strict_eq(&b.value, &b.old_value) {
            return Ok(());
        }
        let v = get_checkbox_value(ctx, el, true);
        loose_equal(rt, &b.value, &v)?
    };
    if host_prop(ctx, el, "checked").to_boolean() != checked {
        ctx.host().set_property(el, "checked", crate::host::HostValue::Bool(checked)).map_err(|e| crate::dom_events::host_error(rt, e))?;
    }
    Ok(())
}

// ---- vModelRadio ----

fn radio_change_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let (ctx, el) = ctx_of_handle(rt, &data[0])?;
    let v = get_value(&ctx, el);
    let assign = assign_of(&ctx, el);
    call_assign(&ctx, el, &assign, v)?;
    Ok(Value::Undefined)
}

fn set_checked_loose(ctx: &DomCtx, el: HostNode, value: &Value, vnode: &Value) -> Res<()> {
    let rt = ctx.rt();
    let props = props_of(rt, vnode)?;
    let wanted = rt.get_str(&props, "value")?;
    let checked = loose_equal(rt, value, &wanted)?;
    if host_prop(ctx, el, "checked").to_boolean() != checked {
        ctx.host().set_property(el, "checked", crate::host::HostValue::Bool(checked)).map_err(|e| crate::dom_events::host_error(rt, e))?;
    }
    Ok(())
}

// ---- vModelSelect ----

/// The `change` listener of a select: the selected values become the model value.
fn select_change_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let (ctx, el) = ctx_of_handle(rt, &data[0])?;
    let number = data[1].to_boolean();
    let host = ctx.host();
    let mut selected: Vec<Value> = Vec::new();
    for opt in host.option_nodes(el) {
        let is_selected = matches!(ctx.host().get_property(opt, "selected"), Ok(crate::host::HostValue::Bool(true)));
        if is_selected {
            let v = get_value(&ctx, opt);
            selected.push(if number { loose_to_number(rt, &v)? } else { v });
        }
    }
    let multiple = host_prop(&ctx, el, "multiple").to_boolean();
    let model_value = ctx.with_expando(el, |ex| ex.model_value.clone()).unwrap_or(Value::Undefined);
    let assigned: Value = if multiple {
        if is_set(rt, &model_value)? {
            new_set(rt, Value::Object(rt.new_array(selected.clone())))?
        } else {
            Value::Object(rt.new_array(selected.clone()))
        }
    } else {
        selected.first().cloned().unwrap_or(Value::Undefined)
    };
    let pending_value = if multiple {
        Value::Object(rt.new_array(selected.clone()))
    } else {
        assigned.clone()
    };
    let token = ctx.next_token();
    ctx.with_expando(el, |ex| {
        ex.pending = Some(crate::dom_events::PendingSelect { token, multiple, value: pending_value });
    });
    ctx.queue_pending_clear(el, token);
    let assign = assign_of(&ctx, el);
    call_assign(&ctx, el, &assign, assigned)?;
    Ok(Value::Undefined)
}

/// `setSelected(el, value)`.
fn set_selected(ctx: &DomCtx, el: HostNode, value: &Value) -> Res<()> {
    let rt = ctx.rt();
    let is_multiple = host_prop(ctx, el, "multiple").to_boolean();
    let is_array_value = rt.is_array(value);
    let value_is_set = is_set(rt, value)?;
    if is_multiple && !is_array_value && !value_is_set {
        return Ok(());
    }
    let host = ctx.host();
    let options = host.option_nodes(el);
    for (i, opt) in options.iter().enumerate() {
        let option_value = get_value(ctx, *opt);
        if is_multiple {
            let selected = if is_array_value {
                let items = array_items(rt, value)?;
                if matches!(option_value, Value::String(_) | Value::Number(_)) {
                    let target = js_string(rt, &option_value)?;
                    let mut any = false;
                    for v in &items {
                        if js_string(rt, v)? == target {
                            any = true;
                            break;
                        }
                    }
                    any
                } else {
                    loose_index_of(rt, &items, &option_value)?.is_some()
                }
            } else {
                method(rt, value, "has", std::slice::from_ref(&option_value))?.to_boolean()
            };
            set_host_prop_bool(ctx, *opt, "selected", selected)?;
        } else {
            let v = get_value(ctx, *opt);
            if loose_equal(rt, &v, value)? {
                let current = ctx.host().get_property(el, "selectedIndex");
                if !matches!(current, Ok(crate::host::HostValue::Number(n)) if n == i as f64) {
                    set_host_prop(ctx, el, "selectedIndex", &Value::Number(i as f64))?;
                }
                return Ok(());
            }
        }
    }
    if !is_multiple {
        let current = ctx.host().get_property(el, "selectedIndex");
        if !matches!(current, Ok(crate::host::HostValue::Number(n)) if n == -1.0) {
            set_host_prop(ctx, el, "selectedIndex", &Value::Number(-1.0))?;
        }
    }
    Ok(())
}

fn set_host_prop_bool(ctx: &DomCtx, el: HostNode, name: &str, v: bool) -> Res<()> {
    set_host_prop(ctx, el, name, &Value::Bool(v))
}

/// `isSameSelectValue(value, assignedValue, multiple)`; `assigned` is the pending value (`pending[1]`).
fn is_same_select_value(rt: &Rt, value: &Value, assigned: &Value, multiple: bool) -> Res<bool> {
    if !multiple {
        return loose_equal(rt, value, assigned);
    }
    if rt.is_array(value) {
        return loose_equal(rt, value, assigned);
    }
    if is_set(rt, value)? {
        let assigned_items = array_items(rt, assigned)?;
        let size = rt.get_str(value, "size")?;
        if !vn_value::strict_eq(&size, &Value::Number(assigned_items.len() as f64)) {
            return Ok(false);
        }
        for item in &assigned_items {
            if !method(rt, value, "has", std::slice::from_ref(item))?.to_boolean() {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    Ok(false)
}

// ---- directive hooks (Rust entry points) ----

/// `vModelText.created`.
pub fn text_created_hook(ctx: &DomCtx, el: HostNode, binding: &Value, vnode: &Value) -> Res<()> {
    let b = binding_of(ctx.rt(), binding)?;
    text_created(ctx, el, &b, vnode)
}

/// `vModelText.mounted`.
pub fn text_mounted_hook(ctx: &DomCtx, el: HostNode, binding: &Value) -> Res<()> {
    let b = binding_of(ctx.rt(), binding)?;
    text_mounted(ctx, el, &b)
}

/// `vModelText.beforeUpdate`.
pub fn text_before_update_hook(ctx: &DomCtx, el: HostNode, binding: &Value, vnode: &Value) -> Res<()> {
    let b = binding_of(ctx.rt(), binding)?;
    text_before_update(ctx, el, &b, vnode)
}

/// `vModelCheckbox.created`: the change listener and the assigner; the initial checkedness waits for `mounted`.
pub fn checkbox_created_hook(ctx: &DomCtx, el: HostNode, vnode: &Value) -> Res<()> {
    let rt = ctx.rt();
    let assign = model_assigner(rt, vnode)?;
    ctx.with_expando(el, |ex| ex.assign = Some(assign));
    add_listener(ctx, el, "change", checkbox_change_code, vec![ctx.handle(el)])
}

/// `vModelCheckbox.mounted` (`setChecked`).
pub fn checkbox_mounted_hook(ctx: &DomCtx, el: HostNode, binding: &Value, vnode: &Value) -> Res<()> {
    let b = binding_of(ctx.rt(), binding)?;
    set_checked(ctx, el, &b, vnode)
}

/// `vModelCheckbox.beforeUpdate`.
pub fn checkbox_before_update_hook(ctx: &DomCtx, el: HostNode, binding: &Value, vnode: &Value) -> Res<()> {
    let rt = ctx.rt();
    let assign = model_assigner(rt, vnode)?;
    ctx.with_expando(el, |ex| ex.assign = Some(assign));
    let b = binding_of(rt, binding)?;
    set_checked(ctx, el, &b, vnode)
}

/// `vModelRadio.created`.
pub fn radio_created_hook(ctx: &DomCtx, el: HostNode, binding: &Value, vnode: &Value) -> Res<()> {
    let rt = ctx.rt();
    let b = binding_of(rt, binding)?;
    set_checked_loose(ctx, el, &b.value, vnode)?;
    let assign = model_assigner(rt, vnode)?;
    ctx.with_expando(el, |ex| ex.assign = Some(assign));
    add_listener(ctx, el, "change", radio_change_code, vec![ctx.handle(el)])
}

/// `vModelRadio.beforeUpdate`.
pub fn radio_before_update_hook(ctx: &DomCtx, el: HostNode, binding: &Value, vnode: &Value) -> Res<()> {
    let rt = ctx.rt();
    let assign = model_assigner(rt, vnode)?;
    ctx.with_expando(el, |ex| ex.assign = Some(assign));
    let b = binding_of(rt, binding)?;
    if !vn_value::strict_eq(&b.value, &b.old_value) {
        set_checked_loose(ctx, el, &b.value, vnode)?;
    }
    Ok(())
}

/// `vModelSelect.created`.
pub fn select_created_hook(ctx: &DomCtx, el: HostNode, binding: &Value, vnode: &Value) -> Res<()> {
    let rt = ctx.rt();
    let b = binding_of(rt, binding)?;
    ctx.with_expando(el, |ex| ex.model_value = Some(b.value.clone()));
    add_listener(ctx, el, "change", select_change_code, vec![ctx.handle(el), Value::Bool(b.number)])?;
    let assign = model_assigner(rt, vnode)?;
    ctx.with_expando(el, |ex| ex.assign = Some(assign));
    Ok(())
}

/// `vModelSelect.mounted`.
pub fn select_mounted_hook(ctx: &DomCtx, el: HostNode, binding: &Value) -> Res<()> {
    let b = binding_of(ctx.rt(), binding)?;
    set_selected(ctx, el, &b.value)
}

/// `vModelSelect.beforeUpdate`.
pub fn select_before_update_hook(ctx: &DomCtx, el: HostNode, binding: &Value, vnode: &Value) -> Res<()> {
    let rt = ctx.rt();
    let b = binding_of(rt, binding)?;
    ctx.with_expando(el, |ex| ex.model_value = Some(b.value.clone()));
    let assign = model_assigner(rt, vnode)?;
    ctx.with_expando(el, |ex| ex.assign = Some(assign));
    Ok(())
}

/// `vModelSelect.updated`.
pub fn select_updated_hook(ctx: &DomCtx, el: HostNode, binding: &Value) -> Res<()> {
    let rt = ctx.rt();
    let b = binding_of(rt, binding)?;
    let pending = ctx.with_expando(el, |ex| ex.pending.take());
    let multiple_now = host_prop(ctx, el, "multiple").to_boolean();
    let same = match &pending {
        Some(p) if p.multiple == multiple_now => is_same_select_value(rt, &b.value, &p.value, p.multiple)?,
        _ => false,
    };
    if !same {
        set_selected(ctx, el, &b.value)?;
    }
    Ok(())
}

/// `resolveDynamicModel(tagName, type)`: which directive a dynamic v-model uses.
fn resolve_dynamic(tag: &str, ty: &str) -> &'static str {
    match tag {
        "SELECT" => "select",
        "TEXTAREA" => "text",
        _ => match ty {
            "checkbox" => "checkbox",
            "radio" => "radio",
            _ => "text",
        },
    }
}

/// `callModelHook(el, binding, vnode, prevVNode, hook)` of `vModelDynamic`.
pub fn dynamic_hook(ctx: &DomCtx, el: HostNode, hook: &str, binding: &Value, vnode: &Value) -> Res<()> {
    let rt = ctx.rt();
    let tag = ctx.host().tag_name(el);
    let props_type = {
        let props = props_of(rt, vnode)?;
        if props.to_boolean() {
            js_string(rt, &rt.get_str(&props, "type")?)?
        } else {
            String::new()
        }
    };
    let directive = resolve_dynamic(&tag, &props_type);
    match (directive, hook) {
        ("text", "created") => text_created_hook(ctx, el, binding, vnode),
        ("text", "mounted") => text_mounted_hook(ctx, el, binding),
        ("text", "beforeUpdate") => text_before_update_hook(ctx, el, binding, vnode),
        ("checkbox", "created") => checkbox_created_hook(ctx, el, vnode),
        ("checkbox", "mounted") => checkbox_mounted_hook(ctx, el, binding, vnode),
        ("checkbox", "beforeUpdate") => checkbox_before_update_hook(ctx, el, binding, vnode),
        ("radio", "created") => radio_created_hook(ctx, el, binding, vnode),
        ("radio", "beforeUpdate") => radio_before_update_hook(ctx, el, binding, vnode),
        ("select", "created") => select_created_hook(ctx, el, binding, vnode),
        ("select", "mounted") => select_mounted_hook(ctx, el, binding),
        ("select", "beforeUpdate") => select_before_update_hook(ctx, el, binding, vnode),
        ("select", "updated") => select_updated_hook(ctx, el, binding),
        _ => Ok(()),
    }
}

// ---- JS-visible hooks: (el, binding, vnode, prevVNode) ----

fn el_and_binding(rt: &Rt, c: &Call<'_>) -> Res<(DomCtx, HostNode, Value, Value)> {
    let (ctx, el) = ctx_of_handle(rt, &c.arg(0))?;
    Ok((ctx, el, c.arg(1), c.arg(2)))
}

fn text_created_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el, binding, vnode) = el_and_binding(rt, c)?;
    text_created_hook(&ctx, el, &binding, &vnode)?;
    Ok(Value::Undefined)
}

fn text_mounted_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el, binding, _vnode) = el_and_binding(rt, c)?;
    text_mounted_hook(&ctx, el, &binding)?;
    Ok(Value::Undefined)
}

/// `vModelText.getSSRProps({ value })`: the server-rendered input carries the model value as its `value` attribute
/// (`initVModelForSSR` in runtime-dom 3.5.43, runtime-dom.cjs.js:1784).
fn text_ssr_props_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let value = rt.get_str(&c.arg(0), "value")?;
    let out = rt.new_object();
    rt.define_data(out, PropKey::lit("value"), value, vn_value::Flags::ALL)?;
    Ok(Value::Object(out))
}

fn text_before_update_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el, binding, vnode) = el_and_binding(rt, c)?;
    text_before_update_hook(&ctx, el, &binding, &vnode)?;
    Ok(Value::Undefined)
}

fn checkbox_created_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el, _binding, vnode) = el_and_binding(rt, c)?;
    checkbox_created_hook(&ctx, el, &vnode)?;
    Ok(Value::Undefined)
}

fn checkbox_mounted_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el, binding, vnode) = el_and_binding(rt, c)?;
    checkbox_mounted_hook(&ctx, el, &binding, &vnode)?;
    Ok(Value::Undefined)
}

fn checkbox_before_update_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el, binding, vnode) = el_and_binding(rt, c)?;
    checkbox_before_update_hook(&ctx, el, &binding, &vnode)?;
    Ok(Value::Undefined)
}

fn radio_created_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el, binding, vnode) = el_and_binding(rt, c)?;
    radio_created_hook(&ctx, el, &binding, &vnode)?;
    Ok(Value::Undefined)
}

fn radio_before_update_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el, binding, vnode) = el_and_binding(rt, c)?;
    radio_before_update_hook(&ctx, el, &binding, &vnode)?;
    Ok(Value::Undefined)
}

fn select_created_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el, binding, vnode) = el_and_binding(rt, c)?;
    select_created_hook(&ctx, el, &binding, &vnode)?;
    Ok(Value::Undefined)
}

fn select_mounted_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el, binding, _vnode) = el_and_binding(rt, c)?;
    select_mounted_hook(&ctx, el, &binding)?;
    Ok(Value::Undefined)
}

fn select_before_update_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el, binding, vnode) = el_and_binding(rt, c)?;
    select_before_update_hook(&ctx, el, &binding, &vnode)?;
    Ok(Value::Undefined)
}

fn select_updated_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ctx, el, binding, _vnode) = el_and_binding(rt, c)?;
    select_updated_hook(&ctx, el, &binding)?;
    Ok(Value::Undefined)
}

fn dynamic_js(rt: &Rt, c: &Call<'_>, hook: &str) -> Res<Value> {
    let (ctx, el, binding, vnode) = el_and_binding(rt, c)?;
    dynamic_hook(&ctx, el, hook, &binding, &vnode)?;
    Ok(Value::Undefined)
}

fn dynamic_created_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    dynamic_js(rt, c, "created")
}

fn dynamic_mounted_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    dynamic_js(rt, c, "mounted")
}

fn dynamic_before_update_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    dynamic_js(rt, c, "beforeUpdate")
}

fn dynamic_updated_js(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    dynamic_js(rt, c, "updated")
}

/// A directive object: a plain object with the hook natives (and `deep` where runtime-dom sets it).
fn directive(rt: &Rt, hooks: &[(&str, CodeFn)], deep: bool) -> Value {
    let obj = rt.new_object();
    if deep {
        let _ = rt.define_data(obj, PropKey::lit("deep"), Value::Bool(true), vn_value::Flags::ALL);
    }
    for (hook, code) in hooks {
        rt.define_native_method(obj, hook, *code, 4);
    }
    Value::Object(obj)
}

/// The v-model directive objects of the runtime's `vue` module.
pub fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    vec![
        (
            "vModelText",
            directive(
                rt,
                &[("created", text_created_js), ("mounted", text_mounted_js), ("beforeUpdate", text_before_update_js), ("getSSRProps", text_ssr_props_js)],
                false,
            ),
        ),
        (
            "vModelCheckbox",
            directive(rt, &[("created", checkbox_created_js), ("mounted", checkbox_mounted_js), ("beforeUpdate", checkbox_before_update_js)], true),
        ),
        ("vModelRadio", directive(rt, &[("created", radio_created_js), ("beforeUpdate", radio_before_update_js)], false)),
        (
            "vModelSelect",
            directive(
                rt,
                &[("created", select_created_js), ("mounted", select_mounted_js), ("beforeUpdate", select_before_update_js), ("updated", select_updated_js)],
                true,
            ),
        ),
        (
            "vModelDynamic",
            directive(
                rt,
                &[("created", dynamic_created_js), ("mounted", dynamic_mounted_js), ("beforeUpdate", dynamic_before_update_js), ("updated", dynamic_updated_js)],
                false,
            ),
        ),
    ]
}
