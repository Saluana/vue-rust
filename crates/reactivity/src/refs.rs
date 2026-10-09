//! Refs of @vue/reactivity 3.5.43: `RefImpl` (ref, shallowRef), `CustomRefImpl`, `GetterRefImpl` (toRef of a getter),
//! `ObjectRefImpl` (toRef and toRefs of a property), triggerRef, toValue, unref, isRef and proxyRefs. Behaviour per
//! @vue/reactivity 3.5.43 (production build). The JS-visible objects are node objects whose prototypes carry the
//! `value` accessors; the flags (`__v_isRef`, `__v_isShallow`, `__v_isReadonly`) are read as properties, as in Vue.

use std::cell::RefCell;
use std::rc::Rc;

use vn_value::{Call, Exotic, Hook, ObjRef, Prop, PropKey, Res, Rt, Tracer, Value};

use crate::graph::{dep_track, dep_trigger, new_dep};
use crate::node::{alloc, node, this_node, try_node, Node};
use crate::shared::{
    accessor, for_in_keys, has_changed, hidden, is_object, is_proxy, is_ref, is_reactive, is_readonly, is_shallow,
    is_integer_key, key_value, native, to_raw, truthy, wrap_reactive,
};
use crate::state::state;
use crate::target::dep_of;

/// `RefImpl`: `_rawValue`, `_value`, the dep and the shallow flag.
pub(crate) struct RefImpl {
    pub(crate) dep: ObjRef,
    pub(crate) raw: RefCell<Value>,
    pub(crate) value: RefCell<Value>,
    pub(crate) shallow: bool,
}

impl RefImpl {
    pub(crate) fn trace(&self, t: &mut Tracer) {
        t.mark(self.dep);
        t.mark_value(&self.raw.borrow());
        t.mark_value(&self.value.borrow());
    }
}

/// `CustomRefImpl`: the value last returned by `get`, and the user's get/set (bound to the ref when called).
pub(crate) struct CustomRef {
    pub(crate) dep: ObjRef,
    pub(crate) value: RefCell<Value>,
    pub(crate) get: RefCell<Value>,
    pub(crate) set: RefCell<Value>,
}

impl CustomRef {
    pub(crate) fn trace(&self, t: &mut Tracer) {
        t.mark(self.dep);
        t.mark_value(&self.value.borrow());
        t.mark_value(&self.get.borrow());
        t.mark_value(&self.set.borrow());
    }
}

/// `GetterRefImpl`: a read-only ref over a getter.
pub(crate) struct GetterRef {
    pub(crate) getter: Value,
    pub(crate) value: RefCell<Value>,
}

impl GetterRef {
    pub(crate) fn trace(&self, t: &mut Tracer) {
        t.mark_value(&self.getter);
        t.mark_value(&self.value.borrow());
    }
}

/// `ObjectRefImpl`: a property of an object as a ref (`toRef(obj, key)`).
pub(crate) struct PropRef {
    pub(crate) object: Value,
    pub(crate) key: PropKey,
    pub(crate) default: Value,
    pub(crate) raw: Value,
    pub(crate) shallow: bool,
    pub(crate) value: RefCell<Value>,
}

impl PropRef {
    pub(crate) fn trace(&self, t: &mut Tracer) {
        t.mark_value(&self.object);
        t.mark_value(&self.default);
        t.mark_value(&self.raw);
        t.mark_value(&self.value.borrow());
    }
}

/// `createRef(value, shallow)`: returns an existing ref unchanged.
pub(crate) fn create_ref(rt: &Rt, v: Value, shallow: bool) -> Res<Value> {
    if is_ref(rt, &v)? {
        return Ok(v);
    }
    let dep = new_dep(rt, None, None);
    let raw = if shallow { v.clone() } else { to_raw(rt, &v)? };
    let value = if shallow { v.clone() } else { wrap_reactive(rt, &v)? };
    let proto = state(rt).protos.get().reference;
    let r = alloc(
        rt,
        proto,
        Node::Ref(RefImpl { dep, raw: RefCell::new(raw), value: RefCell::new(value), shallow }),
    );
    Ok(Value::Object(r))
}

/// `customRef(factory)`: the factory receives `track` and `trigger` bound to the ref's dep.
pub(crate) fn custom_ref(rt: &Rt, factory: Value) -> Res<Value> {
    let dep = new_dep(rt, None, None);
    let proto = state(rt).protos.get().custom;
    let obj = alloc(
        rt,
        proto,
        Node::Custom(CustomRef {
            dep,
            value: RefCell::new(Value::Undefined),
            get: RefCell::new(Value::Undefined),
            set: RefCell::new(Value::Undefined),
        }),
    );
    let track_fn = native(rt, "bound track", 0, custom_track, vec![Value::Object(dep)]);
    let trigger_fn = native(rt, "bound trigger", 0, custom_trigger, vec![Value::Object(dep)]);
    let res = rt.call(&factory, &Value::Undefined, &[Value::Object(track_fn), Value::Object(trigger_fn)])?;
    if res.is_nullish() {
        return Err(rt.throw_type_error("Cannot destructure a nullish customRef factory result"));
    }
    let get = rt.get_str(&res, "get")?;
    let set = rt.get_str(&res, "set")?;
    let cn = node(rt, obj);
    let cr = cn.custom();
    *cr.get.borrow_mut() = get;
    *cr.set.borrow_mut() = set;
    Ok(Value::Object(obj))
}

fn custom_track(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let dep = dep_datum(rt, c);
    Ok(match dep_track(rt, dep) {
        Some(l) => Value::Object(l),
        None => Value::Undefined,
    })
}

fn custom_trigger(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    dep_trigger(rt, dep_datum(rt, c))?;
    Ok(Value::Undefined)
}

fn dep_datum(rt: &Rt, c: &Call<'_>) -> ObjRef {
    rt.fn_data(c.callee)[0].as_object().expect("dep datum (framework bug)")
}

/// `GetterRefImpl` over a getter function.
pub(crate) fn getter_ref(rt: &Rt, getter: Value) -> ObjRef {
    let proto = state(rt).protos.get().getter;
    alloc(rt, proto, Node::Getter(GetterRef { getter, value: RefCell::new(Value::Undefined) }))
}

/// The `ObjectRefImpl` constructor: `_raw = toRaw(object)`, and `_shallow` as in 3.5.43 (a property of a non-array
/// or a non-index key is shallow unless a proxy's raw target is reached through a deep layer).
pub(crate) fn prop_ref(rt: &Rt, object: Value, key: PropKey, default: Value) -> Res<ObjRef> {
    let raw = to_raw(rt, &object)?;
    let key_v = key_value(&key);
    let mut shallow = true;
    let is_symbol = matches!(key, PropKey::Sym(_));
    if !rt.is_array(&object) || is_symbol || !is_integer_key(rt, &key_v) {
        let mut obj = object.clone();
        loop {
            shallow = !is_proxy(rt, &obj)? || is_shallow(rt, &obj)?;
            if !shallow {
                break;
            }
            let next = rt.get_str(&obj, "__v_raw")?;
            if !truthy(&next) {
                break;
            }
            obj = next;
        }
    }
    let proto = state(rt).protos.get().prop;
    Ok(alloc(
        rt,
        proto,
        Node::Prop(PropRef { object, key, default, raw, shallow, value: RefCell::new(Value::Undefined) }),
    ))
}

/// `unref(r)`: `isRef(r) ? r.value : r`.
pub(crate) fn unref(rt: &Rt, v: &Value) -> Res<Value> {
    if is_ref(rt, v)? {
        rt.get_str(v, "value")
    } else {
        Ok(v.clone())
    }
}

/// A property key from a JS value: symbols stay symbols; anything else is `String(key)`.
pub(crate) fn to_prop_key(rt: &Rt, v: &Value) -> Res<PropKey> {
    match v {
        Value::Symbol(s) => Ok(PropKey::Sym(*s)),
        _ => rt.to_property_key(v),
    }
}

// ----- RefImpl -----

fn is_ref_impl(n: &Node) -> bool {
    matches!(n, Node::Ref(_))
}

fn g_ref_value(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = this_node(rt, c.this, is_ref_impl)?;
    let dep = node(rt, r).ref_impl().dep;
    dep_track(rt, dep);
    let v = node(rt, r).ref_impl().value.borrow().clone();
    Ok(v)
}

fn s_ref_value(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = this_node(rt, c.this, is_ref_impl)?;
    let new_v = c.arg(0);
    let (dep, shallow, old) = {
        let rn = node(rt, r);
        let ri = rn.ref_impl();
        (ri.dep, ri.shallow, ri.raw.borrow().clone())
    };
    let direct = shallow || is_shallow(rt, &new_v)? || is_readonly(rt, &new_v)?;
    let nv = if direct { new_v } else { to_raw(rt, &new_v)? };
    if has_changed(&nv, &old) {
        let wrapped = if direct { nv.clone() } else { wrap_reactive(rt, &nv)? };
        {
            let rn = node(rt, r);
            let ri = rn.ref_impl();
            *ri.raw.borrow_mut() = nv;
            *ri.value.borrow_mut() = wrapped;
        }
        dep_trigger(rt, dep)?;
    }
    Ok(Value::Undefined)
}

fn g_ref_shallow(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = this_node(rt, c.this, is_ref_impl)?;
    Ok(Value::Bool(node(rt, r).ref_impl().shallow))
}

fn g_ref_dep(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = this_node(rt, c.this, is_ref_impl)?;
    Ok(Value::Object(node(rt, r).ref_impl().dep))
}

// ----- CustomRefImpl -----

fn is_custom(n: &Node) -> bool {
    matches!(n, Node::Custom(_))
}

fn g_custom_value(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = this_node(rt, c.this, is_custom)?;
    let f = node(rt, r).custom().get.borrow().clone();
    let v = rt.call(&f, &Value::Object(r), &[])?;
    *node(rt, r).custom().value.borrow_mut() = v.clone();
    Ok(v)
}

fn s_custom_value(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = this_node(rt, c.this, is_custom)?;
    let f = node(rt, r).custom().set.borrow().clone();
    rt.call(&f, &Value::Object(r), &[c.arg(0)])?;
    Ok(Value::Undefined)
}

fn g_custom_dep(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = this_node(rt, c.this, is_custom)?;
    Ok(Value::Object(node(rt, r).custom().dep))
}

// ----- GetterRefImpl -----

fn is_getter(n: &Node) -> bool {
    matches!(n, Node::Getter(_))
}

fn g_getter_value(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = this_node(rt, c.this, is_getter)?;
    let f = node(rt, r).getter().getter.clone();
    let v = rt.call(&f, &Value::Object(r), &[])?;
    *node(rt, r).getter().value.borrow_mut() = v.clone();
    Ok(v)
}

// ----- ObjectRefImpl -----

fn is_prop(n: &Node) -> bool {
    matches!(n, Node::Prop(_))
}

fn g_prop_value(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = this_node(rt, c.this, is_prop)?;
    let (object, key, shallow, default) = {
        let rn = node(rt, r);
        let p = rn.prop();
        (p.object.clone(), p.key.clone(), p.shallow, p.default.clone())
    };
    let mut val = rt.get_v(&object, &key)?;
    if shallow {
        val = unref(rt, &val)?;
    }
    let v = if val.is_undefined() { default } else { val };
    *node(rt, r).prop().value.borrow_mut() = v.clone();
    Ok(v)
}

fn s_prop_value(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = this_node(rt, c.this, is_prop)?;
    let (object, key, raw, shallow) = {
        let rn = node(rt, r);
        let p = rn.prop();
        (p.object.clone(), p.key.clone(), p.raw.clone(), p.shallow)
    };
    let new_v = c.arg(0);
    if shallow {
        let raw_ref = rt.get_v(&raw, &key)?;
        if is_ref(rt, &raw_ref)? {
            let nested = rt.get_v(&object, &key)?;
            if is_ref(rt, &nested)? {
                rt.set_v_strict(&nested, &PropKey::lit("value"), new_v)?;
                return Ok(Value::Undefined);
            }
        }
    }
    rt.set_v_strict(&object, &key, new_v)?;
    Ok(Value::Undefined)
}

fn g_prop_dep(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = this_node(rt, c.this, is_prop)?;
    let (raw, key) = {
        let rn = node(rt, r);
        let p = rn.prop();
        (p.raw.clone(), key_value(&p.key))
    };
    Ok(match dep_of(rt, &raw, &key) {
        Some(d) => Value::Object(d),
        None => Value::Undefined,
    })
}

// ----- prototypes -----

pub(crate) fn install_ref_proto(rt: &Rt) -> ObjRef {
    let p = rt.new_object();
    hidden(rt, p, "__v_isRef", Value::Bool(true));
    accessor(rt, p, "value", g_ref_value, Some(s_ref_value));
    accessor(rt, p, "__v_isShallow", g_ref_shallow, None);
    accessor(rt, p, "dep", g_ref_dep, None);
    p
}

pub(crate) fn install_custom_proto(rt: &Rt) -> ObjRef {
    let p = rt.new_object();
    hidden(rt, p, "__v_isRef", Value::Bool(true));
    accessor(rt, p, "value", g_custom_value, Some(s_custom_value));
    accessor(rt, p, "dep", g_custom_dep, None);
    p
}

pub(crate) fn install_getter_proto(rt: &Rt) -> ObjRef {
    let p = rt.new_object();
    hidden(rt, p, "__v_isRef", Value::Bool(true));
    hidden(rt, p, "__v_isReadonly", Value::Bool(true));
    accessor(rt, p, "value", g_getter_value, None);
    p
}

pub(crate) fn install_prop_proto(rt: &Rt) -> ObjRef {
    let p = rt.new_object();
    hidden(rt, p, "__v_isRef", Value::Bool(true));
    accessor(rt, p, "value", g_prop_value, Some(s_prop_value));
    accessor(rt, p, "dep", g_prop_dep, None);
    p
}

// ----- JS surface -----

fn js_ref(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    create_ref(rt, c.arg(0), false)
}

fn js_shallow_ref(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    create_ref(rt, c.arg(0), true)
}

fn js_trigger_ref(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = c.arg(0);
    let dep = rt.get_str(&r, "dep")?;
    if truthy(&dep) {
        match &dep {
            Value::Object(d) if try_node(rt, *d).is_some_and(|n| n.is_dep()) => dep_trigger(rt, *d)?,
            _ => {
                let t = rt.get_str(&dep, "trigger")?;
                rt.call(&t, &dep, &[])?;
            }
        }
    }
    Ok(Value::Undefined)
}

fn js_custom_ref(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    custom_ref(rt, c.arg(0))
}

fn js_to_ref(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let source = c.arg(0);
    if is_ref(rt, &source)? {
        return Ok(source);
    }
    if rt.is_callable(&source) {
        return Ok(Value::Object(getter_ref(rt, source)));
    }
    if is_object(rt, &source) && c.args.len() > 1 {
        let key = to_prop_key(rt, &c.arg(1))?;
        return Ok(Value::Object(prop_ref(rt, source, key, c.arg(2))?));
    }
    create_ref(rt, source, false)
}

fn js_to_refs(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let object = c.arg(0);
    let ret = if rt.is_array(&object) {
        let len = rt.get_str(&object, "length")?;
        let n = rt.to_number(&len)? as usize;
        rt.new_array(vec![Value::Hole; n])
    } else {
        rt.new_object()
    };
    if let Value::Object(o) = &object {
        for k in for_in_keys(rt, *o)? {
            let r = prop_ref(rt, object.clone(), k.clone(), Value::Undefined)?;
            rt.create_data_property(ret, k, Value::Object(r))?;
        }
    }
    Ok(Value::Object(ret))
}

fn js_to_value(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = c.arg(0);
    if rt.is_callable(&s) {
        rt.call(&s, &Value::Undefined, &[])
    } else {
        unref(rt, &s)
    }
}

fn js_unref(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    unref(rt, &c.arg(0))
}

fn js_is_ref(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(is_ref(rt, &c.arg(0))?))
}

/// The `shallowUnwrapHandlers` of `proxyRefs`, as exotic hooks over `target`.
struct ProxyRefs {
    target: ObjRef,
    is_arr: bool,
}

impl Exotic for ProxyRefs {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.target);
    }

    fn get(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, receiver: &Value) -> Res<Hook<Value>> {
        if *key == PropKey::lit("__v_raw") {
            return Ok(Hook::Handled(Value::Object(self.target)));
        }
        let v = rt.get(self.target, key, receiver)?;
        Ok(Hook::Handled(unref(rt, &v)?))
    }

    fn set(&self, rt: &Rt, obj: ObjRef, key: &PropKey, v: &Value, receiver: &Value) -> Res<Hook<bool>> {
        let old = rt.get(self.target, key, &Value::Object(self.target))?;
        if is_ref(rt, &old)? && !is_ref(rt, v)? {
            rt.set_v_strict(&old, &PropKey::lit("value"), v.clone())?;
            return Ok(Hook::Handled(true));
        }
        // A write through the proxy lands on the target (the proxy has no defineProperty trap of its own).
        let recv = match receiver {
            Value::Object(o) if *o == obj => Value::Object(self.target),
            other => other.clone(),
        };
        Ok(Hook::Handled(rt.set(self.target, key, v.clone(), &recv)?))
    }

    fn has(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.has_property(self.target, key)?))
    }

    fn delete(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.delete(self.target, key)?))
    }

    fn own_keys(&self, rt: &Rt, _obj: ObjRef) -> Res<Hook<Vec<PropKey>>> {
        Ok(Hook::Handled(rt.own_keys(self.target)?))
    }

    fn get_own_property(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<Option<Prop>>> {
        Ok(Hook::Handled(rt.get_own_property(self.target, key)?))
    }

    fn define(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, prop: &Prop) -> Res<Hook<bool>> {
        Ok(Hook::Handled(rt.define_own(self.target, key, prop.clone())?))
    }

    fn is_array(&self) -> bool {
        self.is_arr
    }
}

/// `proxyRefs(obj)`: an object whose reads unwrap refs and whose writes assign through them. Reactive objects are
/// returned unchanged. The proxy is an exotic object over `obj` (no JS `Proxy` exists in this runtime).
pub(crate) fn proxy_refs(rt: &Rt, obj: &Value) -> Res<Value> {
    let Value::Object(o) = obj else {
        return Err(rt.throw_type_error("Cannot create proxy with a non-object as target or handler"));
    };
    if is_reactive(rt, obj)? {
        return Ok(obj.clone());
    }
    let proto = rt.get_prototype_of(*o);
    let is_arr = rt.is_array(obj);
    let hooks: Rc<dyn Exotic> = Rc::new(ProxyRefs { target: *o, is_arr });
    Ok(Value::Object(rt.new_exotic_object(proto, hooks)))
}

fn js_proxy_refs(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    proxy_refs(rt, &c.arg(0))
}

/// The public exports this module owns (`ref`, `shallowRef`, `triggerRef`, `customRef`, `toRef`, `toRefs`, `toValue`,
/// `unref`, `isRef`, `proxyRefs`).
pub(crate) fn exported(rt: &Rt) -> Vec<(&'static str, Value)> {
    vec![
        ("ref", Value::Object(native(rt, "ref", 1, js_ref, Vec::new()))),
        ("shallowRef", Value::Object(native(rt, "shallowRef", 1, js_shallow_ref, Vec::new()))),
        ("triggerRef", Value::Object(native(rt, "triggerRef", 1, js_trigger_ref, Vec::new()))),
        ("customRef", Value::Object(native(rt, "customRef", 1, js_custom_ref, Vec::new()))),
        ("toRef", Value::Object(native(rt, "toRef", 3, js_to_ref, Vec::new()))),
        ("toRefs", Value::Object(native(rt, "toRefs", 1, js_to_refs, Vec::new()))),
        ("toValue", Value::Object(native(rt, "toValue", 1, js_to_value, Vec::new()))),
        ("unref", Value::Object(native(rt, "unref", 1, js_unref, Vec::new()))),
        ("isRef", Value::Object(native(rt, "isRef", 1, js_is_ref, Vec::new()))),
        ("proxyRefs", Value::Object(native(rt, "proxyRefs", 1, js_proxy_refs, Vec::new()))),
    ]
}
