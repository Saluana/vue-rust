//! vn-runtime::public_instance: the public instance proxy (`PublicInstanceProxyHandlers` and `publicPropertiesMap`),
//! the expose proxy of `getComponentPublicInstance`, and the attrs proxy of the setup context, of @vue/runtime-core
//! 3.5.43 (production build). The proxy is an exotic object; its `get`, `set` and `has` hooks answer every key except
//! symbols (answered as undefined). Deviations: `$watch`, `$options` with mixins and the CSS modules lookup are not
//! supported (`$watch` throws TypeError); `defineProperty` on the proxy is not trapped.

use std::rc::Rc;

use vn_value::{Exotic, Hook, ObjRef, PropKey, Res, Rt, Tracer, Value};

use crate::instance::{empty_obj, instance_of};
use crate::props::{has_own, put, reactivity_fn};
use crate::renderer::renderer;

/// Keys of the instance record that `key in instance` reports (the `$`-prefixed assignments the proxy refuses).
const INSTANCE_KEYS: &[&str] = &[
    "uid", "vnode", "type", "parent", "appContext", "root", "next", "subTree", "effect", "update", "job", "scope", "render",
    "proxy", "exposed", "exposeProxy", "withProxy", "provides", "ids", "accessCache", "renderCache", "components",
    "directives", "propsOptions", "emitsOptions", "emit", "emitted", "propsDefaults", "inheritAttrs", "ctx", "data",
    "props", "attrs", "slots", "refs", "setupState", "setupContext", "suspense", "asyncDep", "asyncResolved", "isMounted",
    "isUnmounted", "isDeactivated", "sp",
];

/// `hasSetupBinding(state, key)`: a setup binding of a non-script-setup component.
fn has_setup_binding(rt: &Rt, state: &Value, key: &str) -> Res<bool> {
    if matches!(state, Value::Object(o) if *o == empty_obj_obj(rt)) {
        return Ok(false);
    }
    if rt.get_str(state, "__isScriptSetup")?.to_boolean() {
        return Ok(false);
    }
    has_own(rt, state, key)
}

fn empty_obj_obj(rt: &Rt) -> ObjRef {
    match empty_obj(rt) {
        Value::Object(o) => o,
        _ => unreachable!("EMPTY_OBJ is an object"),
    }
}

/// `getPublicInstance(instance)` for `$parent` and `$root`: the expose proxy when the component exposed something.
pub(crate) fn public_instance_of(rt: &Rt, inst: Option<ObjRef>) -> Res<Value> {
    let Some(i) = inst else { return Ok(Value::Null) };
    let rec = instance_of(rt, i);
    if rec.exposed.borrow().to_boolean() {
        return expose_proxy(rt, i);
    }
    Ok(rec.proxy.borrow().clone())
}

/// `instance.exposeProxy` (created on first use from the exposed object, with `proxyRefs(markRaw(exposed))` as target).
pub(crate) fn expose_proxy(rt: &Rt, inst: ObjRef) -> Res<Value> {
    let rec = instance_of(rt, inst);
    let existing = rec.expose_proxy.borrow().clone();
    if existing.to_boolean() {
        return Ok(existing);
    }
    let exposed = rec.exposed.borrow().clone();
    let mark_raw = reactivity_fn(rt, "markRaw");
    let marked = rt.call(&mark_raw, &Value::Undefined, std::slice::from_ref(&exposed))?;
    let proxy_refs = reactivity_fn(rt, "proxyRefs");
    let target = rt.call(&proxy_refs, &Value::Undefined, std::slice::from_ref(&marked))?;
    let p = rt.new_exotic_object(None, Rc::new(ExposeProxy { inst, target: target.clone() }));
    *rec.expose_proxy.borrow_mut() = Value::Object(p);
    Ok(Value::Object(p))
}

/// `new Proxy(instance.ctx, PublicInstanceProxyHandlers)`.
pub(crate) fn new_public_proxy(rt: &Rt, inst: ObjRef) -> ObjRef {
    rt.new_exotic_object(None, Rc::new(PublicProxy { inst }))
}

/// The handlers of `publicPropertiesMap` that are read through the proxy.
fn public_getter(rt: &Rt, inst: ObjRef, key: &str) -> Res<Option<Value>> {
    let rec = instance_of(rt, inst);
    let v = match key {
        "$" => Value::Object(inst),
        "$el" => {
            let vnode = rec.vnode.borrow().clone();
            let el = match vnode {
                Value::Object(o) => crate::renderer::el_of(rt, o),
                _ => None,
            };
            match el {
                Some(n) => renderer(rt)?.ctx.handle(n),
                None => Value::Null,
            }
        }
        "$data" => rec.data.borrow().clone(),
        "$props" => rec.props.borrow().clone(),
        "$attrs" => {
            let attrs = rec.attrs.borrow().clone();
            if let Value::Object(a) = attrs {
                vn_reactivity::track(rt, a, &Value::String(vn_value::JsStr::from_str("")))?;
            }
            attrs
        }
        "$slots" => rec.slots.borrow().clone(),
        "$refs" => rec.refs.borrow().clone(),
        "$parent" => {
            let p = rec.parent.borrow().clone();
            let po = match p {
                Value::Object(o) => Some(o),
                _ => None,
            };
            public_instance_of(rt, po)?
        }
        "$root" => {
            let r = rec.root.borrow().clone();
            let ro = match r {
                Value::Object(o) => Some(o),
                _ => None,
            };
            public_instance_of(rt, ro)?
        }
        "$host" => Value::Null,
        "$emit" => rec.emit.borrow().clone(),
        "$options" => rec.ty.borrow().clone(),
        "$forceUpdate" => {
            let f = rt.new_native_function(force_update_code, "", 0, vec![Value::Object(inst)]);
            Value::Object(f)
        }
        "$nextTick" => {
            let proxy = rec.proxy.borrow().clone();
            let f = rt.new_native_function(next_tick_code, "", 1, vec![proxy]);
            Value::Object(f)
        }
        "$watch" => return Err(rt.throw_type_error("unsupported: $watch (options API)")),
        _ => return Ok(None),
    };
    Ok(Some(v))
}

fn force_update_code(rt: &Rt, c: &vn_value::Call<'_>) -> Res<Value> {
    let inst = match rt.fn_data(c.callee).first() {
        Some(Value::Object(o)) => *o,
        _ => panic!("$forceUpdate without its instance (framework bug)"),
    };
    let update = instance_of(rt, inst).update.borrow().clone();
    if let Value::Object(u) = update {
        crate::scheduler::queue_job(rt, u)?;
    }
    Ok(Value::Undefined)
}

fn next_tick_code(rt: &Rt, c: &vn_value::Call<'_>) -> Res<Value> {
    let this = rt.fn_data(c.callee).first().cloned().unwrap_or(Value::Undefined);
    let f = c.args.first().cloned().unwrap_or(Value::Undefined);
    crate::scheduler::next_tick(rt, &this, &f)
}

/// `PublicInstanceProxyHandlers.get`.
pub(crate) fn public_get(rt: &Rt, inst: ObjRef, key: &str) -> Res<Value> {
    if key == "__v_skip" {
        return Ok(Value::Bool(true));
    }
    let rec = instance_of(rt, inst);
    let setup_state = rec.setup_state.borrow().clone();
    let data = rec.data.borrow().clone();
    let props = rec.props.borrow().clone();
    let ctx = rec.ctx.borrow().clone();
    let access = rec.access_cache.borrow().clone();
    let data_is_empty = matches!(&data, Value::Object(o) if *o == empty_obj_obj(rt));
    if !key.starts_with('$') {
        let n = rt.get_v(&access, &PropKey::from_str(key))?;
        if !matches!(n, Value::Undefined) {
            match num_of(&n) {
                1 => return rt.get_v(&setup_state, &PropKey::from_str(key)),
                2 => return rt.get_v(&data, &PropKey::from_str(key)),
                4 => return rt.get_v(&ctx, &PropKey::from_str(key)),
                3 => return rt.get_v(&props, &PropKey::from_str(key)),
                _ => {}
            }
        } else if has_setup_binding(rt, &setup_state, key)? {
            set_access(rt, &access, key, 1)?;
            return rt.get_v(&setup_state, &PropKey::from_str(key));
        } else if !data_is_empty && has_own(rt, &data, key)? {
            set_access(rt, &access, key, 2)?;
            return rt.get_v(&data, &PropKey::from_str(key));
        } else if has_own(rt, &props, key)? {
            set_access(rt, &access, key, 3)?;
            return rt.get_v(&props, &PropKey::from_str(key));
        } else if has_own(rt, &ctx, key)? {
            set_access(rt, &access, key, 4)?;
            return rt.get_v(&ctx, &PropKey::from_str(key));
        } else {
            set_access(rt, &access, key, 0)?;
        }
    }
    if let Some(v) = public_getter(rt, inst, key)? {
        return Ok(v);
    }
    if has_own(rt, &ctx, key)? {
        return rt.get_v(&ctx, &PropKey::from_str(key));
    }
    let globals = global_properties(rt, inst)?;
    if has_own(rt, &globals, key)? {
        return rt.get_v(&globals, &PropKey::from_str(key));
    }
    Ok(Value::Undefined)
}

fn num_of(v: &Value) -> i32 {
    match v {
        Value::Number(n) => *n as i32,
        _ => -1,
    }
}

fn set_access(rt: &Rt, access: &Value, key: &str, n: i32) -> Res<()> {
    put(rt, access, key, Value::Number(n as f64))
}

/// `appContext.config.globalProperties` of the instance.
fn global_properties(rt: &Rt, inst: ObjRef) -> Res<Value> {
    let app_ctx = instance_of(rt, inst).app_context.borrow().clone();
    let config = rt.get_str(&app_ctx, "config")?;
    rt.get_str(&config, "globalProperties")
}

/// `PublicInstanceProxyHandlers.set`. Returns false when the write is refused (props and `$` names).
pub(crate) fn public_set(rt: &Rt, inst: ObjRef, key: &str, value: Value) -> Res<bool> {
    let rec = instance_of(rt, inst);
    let setup_state = rec.setup_state.borrow().clone();
    let data = rec.data.borrow().clone();
    let props = rec.props.borrow().clone();
    let ctx = rec.ctx.borrow().clone();
    if has_setup_binding(rt, &setup_state, key)? {
        rt.set_v_strict(&setup_state, &PropKey::from_str(key), value)?;
        return Ok(true);
    }
    let data_is_empty = matches!(&data, Value::Object(o) if *o == empty_obj_obj(rt));
    if !data_is_empty && has_own(rt, &data, key)? {
        rt.set_v_strict(&data, &PropKey::from_str(key), value)?;
        return Ok(true);
    }
    if has_own(rt, &props, key)? {
        return Ok(false);
    }
    if let Some(rest) = key.strip_prefix('$') {
        if INSTANCE_KEYS.contains(&rest) {
            return Ok(false);
        }
    }
    rt.set_v_strict(&ctx, &PropKey::from_str(key), value)?;
    Ok(true)
}

/// `PublicInstanceProxyHandlers.has`.
pub(crate) fn public_has(rt: &Rt, inst: ObjRef, key: &str) -> Res<bool> {
    let rec = instance_of(rt, inst);
    let setup_state = rec.setup_state.borrow().clone();
    let data = rec.data.borrow().clone();
    let props = rec.props.borrow().clone();
    let ctx = rec.ctx.borrow().clone();
    let access = rec.access_cache.borrow().clone();
    if rt.get_v(&access, &PropKey::from_str(key))?.to_boolean() {
        return Ok(true);
    }
    let data_is_empty = matches!(&data, Value::Object(o) if *o == empty_obj_obj(rt));
    if !data_is_empty && !key.starts_with('$') && has_own(rt, &data, key)? {
        return Ok(true);
    }
    if has_setup_binding(rt, &setup_state, key)? || has_own(rt, &props, key)? || has_own(rt, &ctx, key)? {
        return Ok(true);
    }
    if is_public_property(key) {
        return Ok(true);
    }
    let globals = global_properties(rt, inst)?;
    has_own(rt, &globals, key)
}

fn is_public_property(key: &str) -> bool {
    matches!(
        key,
        "$" | "$el" | "$data" | "$props" | "$attrs" | "$slots" | "$refs" | "$parent" | "$root" | "$host" | "$emit" | "$options"
            | "$forceUpdate" | "$nextTick" | "$watch"
    )
}

/// The public instance proxy: an exotic object whose hooks run `PublicInstanceProxyHandlers` on `inst`.
pub(crate) struct PublicProxy {
    pub(crate) inst: ObjRef,
}

fn key_str(key: &PropKey) -> Option<String> {
    match key {
        PropKey::Sym(_) => None,
        other => Some(crate::renderer::key_string(other)),
    }
}

impl Exotic for PublicProxy {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.inst);
    }

    fn get(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, _receiver: &Value) -> Res<Hook<Value>> {
        let Some(k) = key_str(key) else { return Ok(Hook::Handled(Value::Undefined)) };
        Ok(Hook::Handled(public_get(rt, self.inst, &k)?))
    }

    fn set(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, v: &Value, _receiver: &Value) -> Res<Hook<bool>> {
        let Some(k) = key_str(key) else { return Ok(Hook::Handled(false)) };
        Ok(Hook::Handled(public_set(rt, self.inst, &k, v.clone())?))
    }

    fn has(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<bool>> {
        let Some(k) = key_str(key) else { return Ok(Hook::Handled(false)) };
        Ok(Hook::Handled(public_has(rt, self.inst, &k)?))
    }
}

/// The expose proxy: `key in exposed` reads the exposed object, public properties are read from the instance.
pub(crate) struct ExposeProxy {
    pub(crate) inst: ObjRef,
    pub(crate) target: Value,
}

impl Exotic for ExposeProxy {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.inst);
        t.mark_value(&self.target);
    }

    fn get(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, _receiver: &Value) -> Res<Hook<Value>> {
        let Value::Object(target) = self.target else { return Ok(Hook::Handled(Value::Undefined)) };
        if rt.has_property(target, key)? {
            return Ok(Hook::Handled(rt.get_v(&self.target, key)?));
        }
        let Some(k) = key_str(key) else { return Ok(Hook::Handled(Value::Undefined)) };
        if is_public_property(&k) {
            return Ok(Hook::Handled(public_get(rt, self.inst, &k)?));
        }
        Ok(Hook::Handled(Value::Undefined))
    }

    fn has(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<bool>> {
        let Value::Object(target) = self.target else { return Ok(Hook::Handled(false)) };
        if rt.has_property(target, key)? {
            return Ok(Hook::Handled(true));
        }
        Ok(Hook::Handled(key_str(key).is_some_and(|k| is_public_property(&k))))
    }
}

/// The `attrs` seen by `setup(props, { attrs })`: every read tracks the attrs object.
pub(crate) struct AttrsProxy {
    pub(crate) target: ObjRef,
}

impl Exotic for AttrsProxy {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.target);
    }

    fn get(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, receiver: &Value) -> Res<Hook<Value>> {
        vn_reactivity::track(rt, self.target, &Value::String(vn_value::JsStr::from_str("")))?;
        Ok(Hook::Handled(rt.get(self.target, key, receiver)?))
    }
}

/// `new Proxy(attrs, attrsProxyHandlers)`.
pub(crate) fn attrs_proxy(rt: &Rt, attrs: ObjRef) -> ObjRef {
    rt.new_exotic_object(None, Rc::new(AttrsProxy { target: attrs }))
}
