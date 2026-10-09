//! vn-runtime::setup_helpers: the run-time helpers that compiled components call (withCtx, renderSlot, resolveComponent,
//! resolveDynamicComponent, resolveDirective, withDirectives, useSlots, useAttrs, useTemplateRef, defineComponent) of
//! @vue/runtime-core 3.5.43 (production build). Not provided (TypeError when imported is not possible; they are absent
//! from the `vue` module): useModel, mergeModels, mergeDefaults, createPropsRestProxy (declared unsupported).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use vn_value::{payload_ref, same_value, strict_eq, Call, CodeFn, Flags, JsStr, NativeObject, ObjRef, PropKey, Res, Rt, Tracer, Value, WellKnown};

use crate::component::create_setup_context_for;
use crate::instance::{current_instance, get_current_instance, instance_of};
use crate::emits::{emit, model_modifiers};
use crate::instance::empty_obj;
use crate::normalize::{array_get, camelize, capitalize, for_in_keys, length_of};
use crate::props::{camel, hyph, put, reactivity_fn};
use crate::public_instance::public_instance_of;
use crate::renderer::{props_of, scope_id_of};
use crate::shapes::PatchFlags;
use crate::slots::{current_rendering, with_ctx};

/// The i-th data value of the running native function.
fn data(rt: &Rt, c: &Call<'_>, i: usize) -> Value {
    rt.fn_data(c.callee).get(i).cloned().unwrap_or(Value::Undefined)
}

fn arg(c: &Call<'_>, i: usize) -> Value {
    c.args.get(i).cloned().unwrap_or(Value::Undefined)
}

fn func(rt: &Rt, name: &str, len: u32, code: CodeFn) -> Value {
    Value::Object(rt.new_native_function(code, name, len, Vec::new()))
}

/// `withCtx(fn, ctx)` export. `ctx` absent means the current rendering instance.
fn with_ctx_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let f = arg(c, 0);
    let ctx = if c.args.len() < 2 { Value::Undefined } else { arg(c, 1) };
    with_ctx(rt, f, ctx)
}

fn to_key(rt: &Rt, v: &Value) -> Res<String> {
    Ok(rt.to_string(v)?.to_rust_string_lossy())
}

/// `ensureValidVNode(vnodes)`: the vnodes when one of them is a real content, null otherwise.
fn ensure_valid(rt: &Rt, vnodes: &Value) -> Res<Value> {
    let n = length_of(rt, vnodes)? as usize;
    let comment = crate::renderer::sym(rt, "v-cmt");
    let fragment = crate::renderer::sym(rt, "v-fgt");
    for i in 0..n {
        let child = array_get(rt, vnodes, i)?;
        let Some(v) = crate::vnode::as_vnode(rt, &child) else { return Ok(vnodes.clone()) };
        let ty = crate::renderer::ty_of(rt, v);
        if vn_value::strict_eq(&ty, &comment) {
            continue;
        }
        if vn_value::strict_eq(&ty, &fragment) {
            let kids = crate::renderer::children_of(rt, v);
            if ensure_valid(rt, &kids)?.is_nullish() {
                continue;
            }
        }
        return Ok(vnodes.clone());
    }
    Ok(Value::Null)
}

/// `renderSlot(slots, name, props, fallback, noSlotted, branchKey)` (the non-custom-element path).
fn render_slot_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let slots = arg(c, 0);
    let name = to_key(rt, &arg(c, 1))?;
    let props = match arg(c, 2) {
        Value::Object(o) => Value::Object(o),
        _ => Value::Object(rt.new_object()),
    };
    let fallback = arg(c, 3);
    let no_slotted = arg(c, 4).to_boolean();
    let branch_key = arg(c, 5);
    let slot = rt.get_v(&slots, &PropKey::from_str(&name))?;
    if slot.to_boolean() && rt.get_str(&slot, "_c")?.to_boolean() {
        put(rt, &slot, "_d", Value::Bool(false))?;
    }
    let open = crate::renderer::builtin_fn(rt, "vn-runtime.openBlock");
    rt.call(&open, &Value::Undefined, &[])?;
    let valid = if slot.to_boolean() {
        let out = rt.call(&slot, &Value::Undefined, std::slice::from_ref(&props))?;
        ensure_valid(rt, &out)?
    } else {
        slot.clone()
    };
    let result = (|| -> Res<Value> {
        let prop_key = rt.get_str(&props, "key")?;
        let slot_key = if prop_key.to_boolean() {
            prop_key
        } else if branch_key.to_boolean() {
            branch_key.clone()
        } else if valid.to_boolean() {
            rt.get_str(&valid, "key")?
        } else {
            Value::Undefined
        };
        let mut key = if slot_key.to_boolean() && !matches!(slot_key, Value::Symbol(_)) {
            to_key(rt, &slot_key)?
        } else {
            format!("_{name}")
        };
        if !valid.to_boolean() && fallback.to_boolean() {
            key.push_str("_fb");
        }
        let children = if valid.to_boolean() {
            valid.clone()
        } else if fallback.to_boolean() {
            rt.call(&fallback, &Value::Undefined, &[])?
        } else {
            Value::Object(rt.new_array(Vec::new()))
        };
        let slots_flag = rt.get_str(&slots, "_")?;
        let patch = if valid.to_boolean() && matches!(slots_flag, Value::Number(n) if n == 1.0) { PatchFlags::STABLE_FRAGMENT } else { -2 };
        let key_obj = rt.new_object();
        rt.define_data(key_obj, PropKey::from_str("key"), Value::String(JsStr::from_str(&key)), Flags::ALL)?;
        let create = crate::renderer::builtin_fn(rt, "vn-runtime.createBlock");
        let fragment = crate::renderer::sym(rt, "v-fgt");
        rt.call(&create, &Value::Undefined, &[fragment, Value::Object(key_obj), children, Value::Number(patch as f64)])
    })();
    if slot.to_boolean() && rt.get_str(&slot, "_c")?.to_boolean() {
        put(rt, &slot, "_d", Value::Bool(true))?;
    }
    let rendered = result?;
    if !no_slotted {
        if let Value::Object(r) = rendered {
            let sid = scope_id_of(rt, r);
            if sid.to_boolean() {
                let s = to_key(rt, &sid)?;
                let list = rt.new_array(vec![Value::String(JsStr::from_str(&format!("{s}-s")))]);
                crate::vnode::fields(rt, r, |f| f.slot_scope_ids = Value::Object(list));
            }
        }
    }
    Ok(rendered)
}

/// `resolveAsset(type, name, warnMissing, maybeSelfReference)` against the current instance and its app.
fn resolve_asset(rt: &Rt, kind: &str, name: &str, maybe_self: bool) -> Res<Value> {
    let Some(inst) = current_rendering(rt).or_else(|| current_instance(rt)) else { return Ok(Value::Undefined) };
    let rec = instance_of(rt, inst);
    let comp = rec.ty.borrow().clone();
    if kind == "components" {
        let self_name = rt.get_str(&comp, "name")?;
        if let Value::String(s) = &self_name {
            let sn = s.to_rust_string_lossy();
            let cam = camelize(&JsStr::from_str(name)).to_rust_string_lossy();
            let cap = capitalize(&JsStr::from_str(&cam)).to_rust_string_lossy();
            if sn == name || sn == cam || sn == cap {
                return Ok(comp);
            }
        }
    }
    let local = {
        let v = if kind == "components" { rec.components.borrow().clone() } else { rec.directives.borrow().clone() };
        if v.to_boolean() {
            v
        } else {
            rt.get_str(&comp, kind)?
        }
    };
    let res = resolve(rt, &local, name)?;
    if res.to_boolean() {
        return Ok(res);
    }
    let app_ctx = rec.app_context.borrow().clone();
    let global = rt.get_str(&app_ctx, kind)?;
    let res = resolve(rt, &global, name)?;
    if !res.to_boolean() && maybe_self && kind == "components" {
        return Ok(comp);
    }
    Ok(res)
}

/// `resolve(registry, name)`: the name, its camelized form or its capitalized camelized form.
fn resolve(rt: &Rt, registry: &Value, name: &str) -> Res<Value> {
    if !registry.to_boolean() {
        return Ok(Value::Undefined);
    }
    let cam = camel(name);
    let cap = capitalize(&JsStr::from_str(&cam)).to_rust_string_lossy();
    for k in [name.to_string(), cam, cap] {
        let v = rt.get_v(registry, &PropKey::from_str(&k))?;
        if v.to_boolean() {
            return Ok(v);
        }
    }
    Ok(Value::Undefined)
}

fn resolve_component_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let name = to_key(rt, &arg(c, 0))?;
    let res = resolve_asset(rt, "components", &name, arg(c, 1).to_boolean())?;
    Ok(if res.to_boolean() { res } else { Value::String(JsStr::from_str(&name)) })
}

fn resolve_dynamic_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let comp = arg(c, 0);
    if let Value::String(s) = &comp {
        let name = s.to_rust_string_lossy();
        let res = resolve_asset(rt, "components", &name, false)?;
        return Ok(if res.to_boolean() { res } else { comp.clone() });
    }
    if comp.to_boolean() {
        return Ok(comp);
    }
    Ok(Value::Symbol(rt.symbol_for(&JsStr::from_str("v-ndc"))))
}

fn resolve_directive_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let name = to_key(rt, &arg(c, 0))?;
    resolve_asset(rt, "directives", &name, false)
}

/// `withDirectives(vnode, directives)`: appends the bindings of the directives to the vnode.
fn with_directives_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let vnode = arg(c, 0);
    let list = arg(c, 1);
    let Some(current) = current_rendering(rt) else { return Ok(vnode) };
    let Value::Object(v) = vnode else { return Err(rt.throw_type_error("withDirectives: the first argument is not a vnode")) };
    let instance = public_instance_of(rt, Some(current))?;
    let bindings = match crate::renderer::dirs_of(rt, v) {
        Value::Object(b) => b,
        _ => {
            let b = rt.new_array(Vec::new());
            fields_dirs(rt, v, Value::Object(b));
            b
        }
    };
    let n = length_of(rt, &list)? as usize;
    for i in 0..n {
        let entry = array_get(rt, &list, i)?;
        let mut dir = array_get(rt, &entry, 0)?;
        let value = array_get(rt, &entry, 1)?;
        let arg_v = array_get(rt, &entry, 2)?;
        let modifiers = {
            let m = array_get(rt, &entry, 3)?;
            if matches!(m, Value::Undefined) {
                crate::instance::empty_obj(rt)
            } else {
                m
            }
        };
        if !dir.to_boolean() {
            continue;
        }
        if rt.is_callable(&dir) {
            let d = rt.new_object();
            rt.define_data(d, PropKey::from_str("mounted"), dir.clone(), Flags::ALL)?;
            rt.define_data(d, PropKey::from_str("updated"), dir.clone(), Flags::ALL)?;
            dir = Value::Object(d);
        }
        if rt.get_str(&dir, "deep")?.to_boolean() {
            let traverse = reactivity_fn(rt, "traverse");
            rt.call(&traverse, &Value::Undefined, std::slice::from_ref(&value))?;
        }
        let binding = rt.new_object();
        rt.define_data(binding, PropKey::from_str("dir"), dir, Flags::ALL)?;
        rt.define_data(binding, PropKey::from_str("instance"), instance.clone(), Flags::ALL)?;
        rt.define_data(binding, PropKey::from_str("value"), value, Flags::ALL)?;
        rt.define_data(binding, PropKey::from_str("oldValue"), Value::Undefined, Flags::ALL)?;
        rt.define_data(binding, PropKey::from_str("arg"), arg_v, Flags::ALL)?;
        rt.define_data(binding, PropKey::from_str("modifiers"), modifiers, Flags::ALL)?;
        let len = length_of(rt, &Value::Object(bindings))? as u32;
        rt.set_v_strict(&Value::Object(bindings), &PropKey::Index(len), Value::Object(binding))?;
    }
    Ok(Value::Object(v))
}

fn fields_dirs(rt: &Rt, v: ObjRef, dirs: Value) {
    crate::vnode::fields(rt, v, |f| f.dirs = dirs);
}

/// The instance of the current setup, as `getContext` requires.
fn setup_instance(rt: &Rt) -> Res<ObjRef> {
    match get_current_instance(rt) {
        Some(i) => Ok(i),
        None => Err(rt.throw_type_error("unsupported: this helper is only usable inside setup()")),
    }
}

fn use_slots_export(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    let inst = setup_instance(rt)?;
    Ok(instance_of(rt, inst).slots.borrow().clone())
}

fn use_attrs_export(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    let inst = setup_instance(rt)?;
    let existing = instance_of(rt, inst).setup_context.borrow().clone();
    let ctx = if existing.to_boolean() { existing } else { create_setup_context_for(rt, inst)? };
    rt.get_str(&ctx, "attrs")
}

fn ref_getter(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = rt.fn_data(c.callee).first().cloned().unwrap_or(Value::Undefined);
    rt.get_str(&r, "value")
}

fn ref_setter(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = rt.fn_data(c.callee).first().cloned().unwrap_or(Value::Undefined);
    rt.set_str(&r, "value", arg(c, 0))?;
    Ok(Value::Undefined)
}

/// `useTemplateRef(key)`: a shallow ref, exposed through the owner's refs object under `key`.
fn use_template_ref_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let key = to_key(rt, &arg(c, 0))?;
    let shallow = reactivity_fn(rt, "shallowRef");
    let r = rt.call(&shallow, &Value::Undefined, &[Value::Null])?;
    if let Some(inst) = get_current_instance(rt) {
        let rec = instance_of(rt, inst);
        let refs = rec.refs.borrow().clone();
        let refs_obj = match refs {
            Value::Object(o) if o != empty_obj_id(rt) => o,
            _ => {
                let o = rt.new_object();
                *rec.refs.borrow_mut() = Value::Object(o);
                o
            }
        };
        let getter = rt.new_native_function(ref_getter, "get", 0, vec![r.clone()]);
        let setter = rt.new_native_function(ref_setter, "set", 1, vec![r.clone()]);
        rt.define_accessor(refs_obj, PropKey::from_str(&key), Some(getter), Some(setter), Flags::new(true, true, false))?;
    }
    Ok(r)
}

fn empty_obj_id(rt: &Rt) -> ObjRef {
    match crate::instance::empty_obj(rt) {
        Value::Object(o) => o,
        _ => unreachable!("EMPTY_OBJ is an object"),
    }
}

fn define_component_export(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(arg(c, 0))
}

/// `h(type, propsOrChildren, children)`.
fn h_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let create = crate::renderer::builtin_fn(rt, "vn-runtime.createVNode");
    let ty = arg(c, 0);
    let n = c.args.len();
    let plain_object = |v: &Value| matches!(v, Value::Object(_)) && !rt.is_callable(v) && !rt.is_array(v);
    if n == 2 {
        let p = arg(c, 1);
        if plain_object(&p) {
            let is_vnode = crate::vnode::as_vnode(rt, &p).is_some();
            if is_vnode {
                let kids = Value::Object(rt.new_array(vec![p]));
                return rt.call(&create, &Value::Undefined, &[ty, Value::Null, kids]);
            }
            return rt.call(&create, &Value::Undefined, &[ty, p]);
        }
        return rt.call(&create, &Value::Undefined, &[ty, Value::Null, p]);
    }
    let children = if n > 3 {
        Value::Object(rt.new_array(c.args[2..].to_vec()))
    } else if n == 3 && crate::vnode::as_vnode(rt, &arg(c, 2)).is_some() {
        Value::Object(rt.new_array(vec![arg(c, 2)]))
    } else {
        arg(c, 2)
    };
    rt.call(&create, &Value::Undefined, &[ty, arg(c, 1), children])
}

/// The compile-time macros as runtime-core defines them at run time (they return null or nothing).
fn null_macro(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Null)
}

fn no_op(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

/// The helpers of the `vue` module (the names runtime-core exports for compiled code).
pub(crate) fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    vec![
        ("withCtx", func(rt, "withCtx", 3, with_ctx_export)),
        ("renderSlot", func(rt, "renderSlot", 6, render_slot_export)),
        ("resolveComponent", func(rt, "resolveComponent", 2, resolve_component_export)),
        ("resolveDynamicComponent", func(rt, "resolveDynamicComponent", 1, resolve_dynamic_export)),
        ("resolveDirective", func(rt, "resolveDirective", 1, resolve_directive_export)),
        ("withDirectives", func(rt, "withDirectives", 2, with_directives_export)),
        ("useSlots", func(rt, "useSlots", 0, use_slots_export)),
        ("useAttrs", func(rt, "useAttrs", 0, use_attrs_export)),
        ("useTemplateRef", func(rt, "useTemplateRef", 1, use_template_ref_export)),
        ("defineComponent", func(rt, "defineComponent", 1, define_component_export)),
        ("h", func(rt, "h", 3, h_export)),
        ("defineProps", func(rt, "defineProps", 0, null_macro)),
        ("defineEmits", func(rt, "defineEmits", 0, null_macro)),
        ("defineSlots", func(rt, "defineSlots", 0, null_macro)),
        ("withDefaults", func(rt, "withDefaults", 2, null_macro)),
        ("defineExpose", func(rt, "defineExpose", 1, no_op)),
        ("defineModel", func(rt, "defineModel", 0, no_op)),
        ("defineOptions", func(rt, "defineOptions", 1, no_op)),
        ("useModel", func(rt, "useModel", 3, use_model_export)),
        ("mergeModels", func(rt, "mergeModels", 2, merge_models_export)),
        ("mergeDefaults", func(rt, "mergeDefaults", 2, merge_defaults_export)),
        ("createPropsRestProxy", func(rt, "createPropsRestProxy", 2, props_rest_export)),
        ("warn", func(rt, "warn", 0, no_op)),
        ("version", Value::String(JsStr::from_str("3.5.43"))),
    ]
}

// ----- model and props helpers: useModel, mergeModels, mergeDefaults, createPropsRestProxy -----

/// `normalizePropsOrEmits(props)`: an array of names becomes an object with null entries.
fn normalize_props_or_emits(rt: &Rt, v: &Value) -> Res<Value> {
    if !rt.is_array(v) {
        return Ok(v.clone());
    }
    let out = rt.new_object();
    for i in 0..length_of(rt, v)? as usize {
        let name = to_key(rt, &array_get(rt, v, i)?)?;
        rt.define_data(out, PropKey::from_str(&name), Value::Null, Flags::ALL)?;
    }
    Ok(Value::Object(out))
}

/// `mergeDefaults(raw, defaults)`: sets the defaults of declared props (in place, as runtime-core does).
fn merge_defaults_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let props = normalize_props_or_emits(rt, &arg(c, 0))?;
    let defaults = arg(c, 1);
    for key in for_in_keys(rt, &defaults)? {
        let k = crate::renderer::key_string(&key);
        if k.starts_with("__skip") {
            continue;
        }
        let default_value = rt.get_v(&defaults, &key)?;
        let mut opt = rt.get_v(&props, &key)?;
        if opt.to_boolean() {
            if rt.is_array(&opt) || rt.is_callable(&opt) {
                let fresh = plain(rt, &[("type", opt), ("default", default_value)])?;
                put(rt, &props, &k, fresh.clone())?;
                opt = fresh;
            } else {
                put(rt, &opt, "default", default_value)?;
            }
        } else if matches!(opt, Value::Null) {
            let fresh = plain(rt, &[("default", default_value)])?;
            put(rt, &props, &k, fresh.clone())?;
            opt = fresh;
        }
        if opt.to_boolean() && rt.get_str(&defaults, &format!("__skip_{k}"))?.to_boolean() {
            put(rt, &opt, "skipFactory", Value::Bool(true))?;
        }
    }
    Ok(props)
}

/// `mergeModels(a, b)`: two prop lists joined (arrays concatenate, objects are merged).
fn merge_models_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (a, b) = (arg(c, 0), arg(c, 1));
    if !a.to_boolean() {
        return Ok(b);
    }
    if !b.to_boolean() {
        return Ok(a);
    }
    if rt.is_array(&a) && rt.is_array(&b) {
        let mut items = Vec::new();
        for v in [&a, &b] {
            for i in 0..length_of(rt, v)? as usize {
                items.push(array_get(rt, v, i)?);
            }
        }
        return Ok(Value::Object(rt.new_array(items)));
    }
    let out = rt.new_object();
    for src in [normalize_props_or_emits(rt, &a)?, normalize_props_or_emits(rt, &b)?] {
        for key in crate::dom_props::enumerable_keys(rt, &src)? {
            let v = rt.get_str(&src, &key)?;
            rt.define_data(out, PropKey::from_str(&key), v, Flags::ALL)?;
        }
    }
    Ok(Value::Object(out))
}

/// `createPropsRestProxy(props, excludedKeys)`: the props that are not excluded, as enumerable getters.
fn props_rest_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let props = arg(c, 0);
    let excluded = arg(c, 1);
    let ret = rt.new_object();
    for key in for_in_keys(rt, &props)? {
        let k = crate::renderer::key_string(&key);
        let mut skip = false;
        for i in 0..length_of(rt, &excluded)? as usize {
            if rt.to_string(&array_get(rt, &excluded, i)?)?.to_rust_string_lossy() == k {
                skip = true;
                break;
            }
        }
        if skip {
            continue;
        }
        let getter = rt.new_native_function(rest_getter, "get", 0, vec![props.clone(), Value::String(JsStr::from_str(&k))]);
        rt.define_accessor(ret, PropKey::from_str(&k), Some(getter), None, Flags::new(true, true, false))?;
    }
    Ok(Value::Object(ret))
}

fn rest_getter(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let props = data.first().cloned().unwrap_or(Value::Undefined);
    let key = to_key(rt, &data.get(1).cloned().unwrap_or(Value::Undefined))?;
    rt.get_str(&props, &key)
}

/// An object with the given data properties.
fn plain(rt: &Rt, pairs: &[(&str, Value)]) -> Res<Value> {
    let o = rt.new_object();
    for (k, v) in pairs {
        rt.define_data(o, PropKey::from_str(k), v.clone(), Flags::ALL)?;
    }
    Ok(Value::Object(o))
}

/// The state of one `useModel` (the closure variables of its customRef factory and of its synchronous watcher).
pub(crate) struct ModelState {
    instance: ObjRef,
    name: String,
    camel: String,
    hyph: String,
    options: Value,
    props: Value,
    local: RefCell<Value>,
    prev_set: RefCell<Value>,
    prev_emitted: RefCell<Value>,
    track: RefCell<Value>,
    trigger: RefCell<Value>,
    sync_runner: RefCell<Value>,
}

impl NativeObject for ModelState {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.instance);
        t.mark_value(&self.options);
        t.mark_value(&self.props);
        t.mark_value(&self.local.borrow());
        t.mark_value(&self.prev_set.borrow());
        t.mark_value(&self.prev_emitted.borrow());
        t.mark_value(&self.track.borrow());
        t.mark_value(&self.trigger.borrow());
        t.mark_value(&self.sync_runner.borrow());
    }
}

fn model_state(rt: &Rt, data: &Value) -> Res<Rc<dyn NativeObject>> {
    match data {
        Value::Object(o) => rt.native_payload(*o).ok_or_else(|| rt.throw_type_error("useModel state (framework bug)")),
        _ => Err(rt.throw_type_error("useModel state (framework bug)")),
    }
}

/// `hasChanged(a, b)`: not Object.is.
fn changed(a: &Value, b: &Value) -> bool {
    !same_value(a, b)
}

/// `key in obj` for an object value (false for a non-object).
fn has_in(rt: &Rt, obj: &Value, key: &str) -> Res<bool> {
    match obj {
        Value::Object(o) => rt.has_property(*o, &PropKey::from_str(key)),
        _ => Ok(false),
    }
}

/// The customRef factory `(track, trigger) => ({ get, set })` of useModel.
fn model_factory(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let state_v = data(rt, c, 0);
    let p = model_state(rt, &state_v)?;
    let st = payload_ref::<ModelState>(&*p).expect("useModel state type (framework bug)");
    *st.track.borrow_mut() = arg(c, 0);
    *st.trigger.borrow_mut() = arg(c, 1);
    // watchSyncEffect: the body runs now and again, synchronously, whenever the prop changes
    let body = Value::Object(rt.new_native_function(model_sync_body, "", 0, vec![state_v.clone()]));
    let sched = Value::Object(rt.new_native_function(model_sync_sched, "", 0, vec![state_v.clone()]));
    let opts = plain(rt, &[("scheduler", sched)])?;
    let effect = reactivity_fn(rt, "effect");
    let runner = rt.call(&effect, &Value::Undefined, &[body, opts])?;
    *st.sync_runner.borrow_mut() = runner;
    let get = Value::Object(rt.new_native_function(model_get, "get", 0, vec![state_v.clone()]));
    let set = Value::Object(rt.new_native_function(model_set, "set", 1, vec![state_v]));
    plain(rt, &[("get", get), ("set", set)])
}

fn model_sync_body(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = model_state(rt, &data(rt, c, 0))?;
    let st = payload_ref::<ModelState>(&*p).expect("useModel state type (framework bug)");
    let prop = rt.get_str(&st.props, &st.camel)?;
    let local = st.local.borrow().clone();
    if changed(&local, &prop) {
        *st.local.borrow_mut() = prop;
        let trig = st.trigger.borrow().clone();
        rt.call(&trig, &Value::Undefined, &[])?;
    }
    Ok(Value::Undefined)
}

fn model_sync_sched(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = model_state(rt, &data(rt, c, 0))?;
    let st = payload_ref::<ModelState>(&*p).expect("useModel state type (framework bug)");
    let runner = st.sync_runner.borrow().clone();
    rt.call(&runner, &Value::Undefined, &[])?;
    Ok(Value::Undefined)
}

fn model_get(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = model_state(rt, &data(rt, c, 0))?;
    let st = payload_ref::<ModelState>(&*p).expect("useModel state type (framework bug)");
    let track = st.track.borrow().clone();
    rt.call(&track, &Value::Undefined, &[])?;
    let local = st.local.borrow().clone();
    let get = rt.get_str(&st.options, "get")?;
    if get.to_boolean() {
        return rt.call(&get, &Value::Undefined, &[local]);
    }
    Ok(local)
}

fn model_set(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = model_state(rt, &data(rt, c, 0))?;
    let st = payload_ref::<ModelState>(&*p).expect("useModel state type (framework bug)");
    let value = arg(c, 0);
    let set_fn = rt.get_str(&st.options, "set")?;
    let emitted = if set_fn.to_boolean() { rt.call(&set_fn, &Value::Undefined, &[value.clone()])? } else { value.clone() };
    let empty = empty_obj(rt);
    let prev_set = st.prev_set.borrow().clone();
    let local = st.local.borrow().clone();
    let prev_set_is_set = !strict_eq(&prev_set, &empty);
    if !changed(&emitted, &local) && !(prev_set_is_set && changed(&value, &prev_set)) {
        return Ok(Value::Undefined);
    }
    let vnode = match instance_of(rt, st.instance).vnode.borrow().clone() {
        Value::Object(v) => v,
        _ => return Err(rt.throw_type_error("useModel: the instance has no vnode (framework bug)")),
    };
    let raw_props = props_of(rt, vnode);
    let (name, camel, hyph_name) = (st.name.clone(), st.camel.clone(), st.hyph.clone());
    let has_v_model = raw_props.to_boolean()
        && (has_in(rt, &raw_props, &name)? || has_in(rt, &raw_props, &camel)? || has_in(rt, &raw_props, &hyph_name)?)
        && (has_in(rt, &raw_props, &format!("onUpdate:{name}"))?
            || has_in(rt, &raw_props, &format!("onUpdate:{camel}"))?
            || has_in(rt, &raw_props, &format!("onUpdate:{hyph_name}"))?);
    if !has_v_model {
        *st.local.borrow_mut() = value.clone();
        let trig = st.trigger.borrow().clone();
        rt.call(&trig, &Value::Undefined, &[])?;
    }
    emit(rt, st.instance, &Value::String(JsStr::from_str(&format!("update:{name}"))), &[emitted.clone()])?;
    let prev_emitted = st.prev_emitted.borrow().clone();
    let local_now = st.local.borrow().clone();
    if changed(&value, &prev_set)
        && ((changed(&value, &emitted) && !changed(&emitted, &prev_emitted))
            || (has_v_model && prev_set_is_set && !changed(&emitted, &local_now)))
    {
        let trig = st.trigger.borrow().clone();
        rt.call(&trig, &Value::Undefined, &[])?;
    }
    *st.prev_set.borrow_mut() = value;
    *st.prev_emitted.borrow_mut() = emitted;
    Ok(Value::Undefined)
}

/// The state of `res[Symbol.iterator]`: yields the ref, then its modifiers (`const [model, modifiers] = useModel(...)`).
pub(crate) struct ModelIter {
    count: Cell<u32>,
    model: Value,
    modifiers: Value,
}

impl NativeObject for ModelIter {
    fn trace(&self, t: &mut Tracer) {
        t.mark_value(&self.model);
        t.mark_value(&self.modifiers);
    }
}

fn model_iterator(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let it = rt.new_native_object(None, Rc::new(ModelIter { count: Cell::new(0), model: data(rt, c, 0), modifiers: data(rt, c, 1) }));
    let next = Value::Object(rt.new_native_function(model_iter_next, "next", 0, vec![Value::Object(it)]));
    plain(rt, &[("next", next)])
}

fn model_iter_next(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Value::Object(o) = data(rt, c, 0) else { return Err(rt.throw_type_error("useModel iterator (framework bug)")) };
    let p = rt.native_payload(o).expect("useModel iterator state (framework bug)");
    let it = payload_ref::<ModelIter>(&*p).expect("useModel iterator type (framework bug)");
    let n = it.count.get();
    if n >= 2 {
        return plain(rt, &[("value", Value::Undefined), ("done", Value::Bool(true))]);
    }
    it.count.set(n + 1);
    let value = if n == 0 {
        it.model.clone()
    } else if it.modifiers.to_boolean() {
        it.modifiers.clone()
    } else {
        empty_obj(rt)
    };
    plain(rt, &[("value", value), ("done", Value::Bool(false))])
}

/// `useModel(props, name, options)`: a customRef that reads the prop and writes through `update:<name>`.
fn use_model_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let props = arg(c, 0);
    let name = to_key(rt, &arg(c, 1))?;
    let options = match arg(c, 2) {
        Value::Undefined => empty_obj(rt),
        o => o,
    };
    let Some(inst) = get_current_instance(rt) else {
        return Err(rt.throw_type_error("unsupported: useModel outside setup()"));
    };
    let camel_name = camel(&name);
    let hyph_name = hyph(&name);
    let modifiers = model_modifiers(rt, &props, &camel_name)?;
    let state = ModelState {
        instance: inst,
        name: name.clone(),
        camel: camel_name,
        hyph: hyph_name,
        options,
        props,
        local: RefCell::new(Value::Undefined),
        prev_set: RefCell::new(empty_obj(rt)),
        prev_emitted: RefCell::new(Value::Undefined),
        track: RefCell::new(Value::Undefined),
        trigger: RefCell::new(Value::Undefined),
        sync_runner: RefCell::new(Value::Undefined),
    };
    let state_obj = rt.new_native_object(None, Rc::new(state));
    let factory = Value::Object(rt.new_native_function(model_factory, "", 2, vec![Value::Object(state_obj)]));
    let custom_ref = reactivity_fn(rt, "customRef");
    let res = rt.call(&custom_ref, &Value::Undefined, &[factory])?;
    if let Value::Object(r) = res {
        let iter = rt.new_native_function(model_iterator, "", 0, vec![res.clone(), modifiers]);
        let sym = rt.well_known(WellKnown::Iterator);
        rt.define_data(r, PropKey::Sym(sym), Value::Object(iter), Flags::ALL)?;
    }
    Ok(res)
}
