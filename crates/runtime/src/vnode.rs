//! vn-runtime::vnode: VNodes, shape and patch flags, block tracking and the createVNode family (behaviour per
//! @vue/runtime-core 3.5.43, production build). A VNode is an `ObjectKind::Native` heap object whose payload is
//! `VNodeData`; its identity is the object's. Blocks are `Block` payload objects. The block stack and the tracking
//! counter live in one `State` object that every export receives as its first function-data value.
//! Deviations: no rendering instance (`ctx`, `scopeId` stay null, `_ctx` is null), no internal-object prototype
//! (`isInternalObject` is always false), `createStaticVNode` and transition cloning throw TypeError.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use vn_value::{payload_ref, strict_eq, Call, Flags, JsStr, NativeObject, ObjRef, Prop, PropKey, Res, Rt, Sym, Tracer, Value, MAX_DENSE_LEN};

use crate::normalize::{self, array_get, for_in_keys, is_obj, key_js, length_of, normalize_class, normalize_style};
use crate::shapes::{PatchFlags, ShapeFlags};

/// A block: the vnodes tracked while it was open (`currentBlock` of runtime-core).
pub struct Block {
    pub items: RefCell<Vec<ObjRef>>,
    pub has_once: Cell<bool>,
}

impl NativeObject for Block {
    fn trace(&self, t: &mut Tracer) {
        for o in self.items.borrow().iter() {
            t.mark(*o);
        }
    }
}

/// Every field of a vnode (the names of runtime-core's createBaseVNode, plus `memo`, `cacheIndex` and `ce`).
#[derive(Clone)]
pub struct Fields {
    pub ty: Value,
    pub props: Value,
    pub key: Value,
    pub reference: Value,
    pub children: Value,
    pub shape: Value,
    pub patch: Value,
    pub dynamic_props: Value,
    pub dynamic_children: Value,
    pub static_count: Value,
    pub el: Option<u64>,
    pub anchor: Option<u64>,
    pub(crate) el_handle: Option<ObjRef>,
    pub(crate) anchor_handle: Option<ObjRef>,
    pub target: Option<u64>,
    pub target_anchor: Option<u64>,
    pub target_start: Option<u64>,
    pub scope_id: Value,
    pub slot_scope_ids: Value,
    pub component: Value,
    pub suspense: Value,
    pub ss_content: Value,
    pub ss_fallback: Value,
    pub dirs: Value,
    pub transition: Value,
    pub ctx: Value,
    pub app_context: Value,
    pub memo: Value,
    pub cache_index: Value,
    pub ce: Value,
    pub placeholder: Value,
}

impl Fields {
    fn new() -> Fields {
        Fields {
            ty: Value::Null,
            props: Value::Null,
            key: Value::Null,
            reference: Value::Null,
            children: Value::Null,
            shape: Value::Number(0.0),
            patch: Value::Number(0.0),
            dynamic_props: Value::Null,
            dynamic_children: Value::Null,
            static_count: Value::Number(0.0),
            el: None,
            anchor: None,
            el_handle: None,
            anchor_handle: None,
            target: None,
            target_anchor: None,
            target_start: None,
            scope_id: Value::Null,
            slot_scope_ids: Value::Null,
            component: Value::Null,
            suspense: Value::Null,
            ss_content: Value::Null,
            ss_fallback: Value::Null,
            dirs: Value::Null,
            transition: Value::Null,
            ctx: Value::Null,
            app_context: Value::Null,
            memo: Value::Null,
            cache_index: Value::Null,
            ce: Value::Null,
            placeholder: Value::Null,
        }
    }
}

/// The payload of a VNode object.
pub struct VNodeData {
    pub f: RefCell<Fields>,
}

impl NativeObject for VNodeData {
    fn trace(&self, t: &mut Tracer) {
        let f = self.f.borrow();
        for o in [f.el_handle, f.anchor_handle].into_iter().flatten() { t.mark(o); }
        for v in [
            &f.ty, &f.props, &f.key, &f.reference, &f.children, &f.shape, &f.patch, &f.dynamic_props, &f.dynamic_children,
            &f.static_count, &f.scope_id, &f.slot_scope_ids, &f.component, &f.suspense, &f.ss_content, &f.ss_fallback,
            &f.dirs, &f.transition, &f.ctx, &f.app_context, &f.memo, &f.cache_index, &f.ce, &f.placeholder,
        ] {
            t.mark_value(v);
        }
    }
}

/// The per-realm state: the block stack (`None` for a block opened with tracking disabled), the current block,
/// the tracking counter (`isBlockTreeEnabled`) and `EMPTY_ARR`, the empty dynamic-children list.
pub struct State {
    stack: RefCell<Vec<Option<ObjRef>>>,
    current: Cell<Option<ObjRef>>,
    tracking: Cell<f64>,
    empty: ObjRef,
}

impl NativeObject for State {
    fn trace(&self, t: &mut Tracer) {
        for b in self.stack.borrow().iter().flatten() {
            t.mark(*b);
        }
        if let Some(c) = self.current.get() {
            t.mark(c);
        }
        t.mark(self.empty);
    }
}

/// Creates the state object of a realm. Every export of this module carries it as data[0].
pub fn new_state(rt: &Rt) -> ObjRef {
    let empty = new_block(rt, false);
    let st = State { stack: RefCell::new(Vec::new()), current: Cell::new(None), tracking: Cell::new(1.0), empty };
    rt.new_native_object(None, Rc::new(st))
}

/// Runs `f` on the state of the export being called (its function data).
pub(crate) fn with_state<R>(rt: &Rt, c: &Call<'_>, f: impl FnOnce(&State) -> R) -> R {
    let data = rt.fn_data(c.callee);
    let Some(Value::Object(s)) = data.first() else {
        panic!("vnode export without its state (framework bug)");
    };
    let p = rt.native_payload(*s).expect("vnode state payload (framework bug)");
    let st = payload_ref::<State>(&*p).expect("vnode state type (framework bug)");
    f(st)
}

fn new_block(rt: &Rt, has_once: bool) -> ObjRef {
    rt.new_native_object(None, Rc::new(Block { items: RefCell::new(Vec::new()), has_once: Cell::new(has_once) }))
}

fn block_with<R>(rt: &Rt, b: ObjRef, f: impl FnOnce(&Block) -> R) -> R {
    let p = rt.native_payload(b).expect("block payload (framework bug)");
    let blk = payload_ref::<Block>(&*p).expect("block type (framework bug)");
    f(blk)
}

fn block_push(rt: &Rt, b: ObjRef, v: ObjRef) {
    block_with(rt, b, |blk| blk.items.borrow_mut().push(v));
}

/// `currentBlock[currentBlock.indexOf(old)] = new`: a no-op when `old` is not in the block (the JS write lands on
/// a non-index property).
fn block_replace(rt: &Rt, b: ObjRef, old: ObjRef, new: ObjRef) {
    block_with(rt, b, |blk| {
        let mut items = blk.items.borrow_mut();
        if let Some(i) = items.iter().position(|x| *x == old) {
            items[i] = new;
        }
    });
}

/// The vnodes of a block, in order.
pub(crate) fn block_items(rt: &Rt, b: ObjRef) -> Vec<ObjRef> {
    block_with(rt, b, |blk| blk.items.borrow().clone())
}

/// True when `o` is a VNode object.
pub(crate) fn is_native_vnode(rt: &Rt, o: ObjRef) -> bool {
    rt.native_payload(o).is_some_and(|p| payload_ref::<VNodeData>(&*p).is_some())
}

/// The VNode behind a value, when it is one.
pub(crate) fn as_vnode(rt: &Rt, v: &Value) -> Option<ObjRef> {
    match v {
        Value::Object(o) if is_native_vnode(rt, *o) => Some(*o),
        _ => None,
    }
}

/// Reads or edits the fields of a VNode. `f` must not call back into the runtime.
pub(crate) fn fields<R>(rt: &Rt, o: ObjRef, f: impl FnOnce(&mut Fields) -> R) -> R {
    let p = rt.native_payload(o).expect("vnode access on a non-vnode (framework bug)");
    let d = payload_ref::<VNodeData>(&*p).expect("vnode access on a non-vnode (framework bug)");
    let mut g = d.f.borrow_mut();
    f(&mut g)
}

fn alloc(rt: &Rt, f: Fields) -> ObjRef {
    rt.new_native_object(Some(vnode_proto(rt)), Rc::new(VNodeData { f: RefCell::new(f) }))
}

/// The fields that JavaScript code and the instance layer read by property. Native vnodes keep them in the payload;
/// the prototype exposes read-only accessors over the payload. `el` and `anchor` are host nodes and are not exposed.
const VNODE_PROPS: &[&str] = &[
    "type", "props", "key", "ref", "children", "shapeFlag", "patchFlag", "dynamicChildren", "dynamicProps", "appContext",
    "ce", "scopeId", "slotScopeIds", "dirs", "transition", "component", "memo", "cacheIndex", "ctx", "placeholder",
];
const VNODE_PROTO: &str = "vn-runtime.vnode-proto";

fn vnode_proto(rt: &Rt) -> ObjRef {
    if let Some(p) = rt.builtin(VNODE_PROTO) {
        return p;
    }
    let p = rt.new_object();
    for name in VNODE_PROPS {
        let getter = rt.new_native_function(vnode_getter, name, 0, vec![Value::String(JsStr::from_str(name))]);
        rt.define_accessor(p, PropKey::from_str(name), Some(getter), None, Flags::new(false, true, true))
            .expect("vnode prototype accessor (framework bug)");
    }
    rt.register_builtin(VNODE_PROTO, p);
    p
}

/// The getter of one vnode property: the field named by the function's data, of the receiver.
fn vnode_getter(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let name = match rt.fn_data(c.callee).first() {
        Some(Value::String(s)) => s.to_rust_string_lossy(),
        _ => return Ok(Value::Undefined),
    };
    let Some(o) = as_vnode(rt, c.this) else { return Ok(Value::Undefined) };
    Ok(fields(rt, o, |f| match name.as_str() {
        "type" => f.ty.clone(),
        "props" => f.props.clone(),
        "key" => f.key.clone(),
        "ref" => f.reference.clone(),
        "children" => f.children.clone(),
        "shapeFlag" => f.shape.clone(),
        "patchFlag" => f.patch.clone(),
        "dynamicChildren" => f.dynamic_children.clone(),
        "dynamicProps" => f.dynamic_props.clone(),
        "appContext" => f.app_context.clone(),
        "ce" => f.ce.clone(),
        "scopeId" => f.scope_id.clone(),
        "slotScopeIds" => f.slot_scope_ids.clone(),
        "dirs" => f.dirs.clone(),
        "transition" => f.transition.clone(),
        "component" => f.component.clone(),
        "memo" => f.memo.clone(),
        "cacheIndex" => f.cache_index.clone(),
        "ctx" => f.ctx.clone(),
        "placeholder" => f.placeholder.clone(),
        _ => Value::Undefined,
    }))
}

/// `currentRenderingInstance` as a value (null outside a render).
fn current_rendering_value(rt: &Rt) -> Value {
    match crate::instance::inst_state(rt).rendering.get() {
        Some(i) => Value::Object(i),
        None => Value::Null,
    }
}

/// `currentScopeId`: the `__scopeId` of the component being rendered, or null (setCurrentRenderingInstance).
fn current_scope_id(rt: &Rt) -> Res<Value> {
    let Some(inst) = crate::instance::inst_state(rt).rendering.get() else { return Ok(Value::Null) };
    let ty = crate::instance::instance_of(rt, inst).ty.borrow().clone();
    if ty.is_nullish() {
        return Ok(Value::Null);
    }
    let sid = rt.get_str(&ty, "__scopeId")?;
    Ok(if sid.to_boolean() { sid } else { Value::Null })
}

fn sym_value(rt: &Rt, name: &str) -> Value {
    Value::Symbol(rt.symbol_for(&JsStr::from_str(name)))
}

/// Symbol.for("v-fgt"), "v-txt", "v-cmt", "v-stc" and "v-ndc" (NULL_DYNAMIC_COMPONENT), as runtime-core defines them.
pub fn fragment_symbol(rt: &Rt) -> Value {
    sym_value(rt, "v-fgt")
}

pub(crate) fn is_sym(v: &Value, s: Sym) -> bool {
    matches!(v, Value::Symbol(x) if *x == s)
}

fn dflt(v: Value, d: Value) -> Value {
    if v.is_undefined() { d } else { v }
}

// ----- predicates and small helpers -----

/// isProxy: `!!value.__v_raw` for a truthy value.
pub(crate) fn is_proxy(rt: &Rt, v: &Value) -> Res<bool> {
    if !v.to_boolean() {
        return Ok(false);
    }
    Ok(rt.get_v(v, &PropKey::from_str("__v_raw"))?.to_boolean())
}

/// isVNode(value): a VNode object, or an object whose `__v_isVNode` is `true` (the latter is not representable here).
pub(crate) fn is_vnode_value(rt: &Rt, v: &Value) -> Res<bool> {
    if as_vnode(rt, v).is_some() {
        return Ok(true);
    }
    if !v.to_boolean() {
        return Ok(false);
    }
    Ok(matches!(rt.get_v(v, &PropKey::from_str("__v_isVNode"))?, Value::Bool(true)))
}

/// Object.assign({}, src): the own enumerable keys (strings and symbols) copied with [[Set]].
fn extend_copy(rt: &Rt, src: &Value) -> Res<ObjRef> {
    let target = rt.new_object();
    let from = rt.to_object(src)?;
    for k in rt.own_keys(from)? {
        if rt.get_own_property(from, &k)?.is_some_and(|p| p.flags().enumerable) {
            let v = rt.get(from, &k, src)?;
            rt.set_v(&Value::Object(target), &k, v)?;
        }
    }
    Ok(target)
}

/// `[].concat(...items)`: arrays are spread (holes kept), everything else is appended.
pub(crate) fn js_concat(rt: &Rt, items: &[Value]) -> Res<Value> {
    let mut out: Vec<Value> = Vec::new();
    for item in items {
        match item {
            Value::Object(o) if rt.is_array(item) => {
                let len = length_of(rt, item)? as usize;
                for i in 0..len {
                    let key = PropKey::Index(i as u32);
                    if rt.has_property(*o, &key)? {
                        out.push(rt.get_v(item, &key)?);
                    } else {
                        out.push(Value::Hole);
                    }
                }
            }
            other => out.push(other.clone()),
        }
    }
    if out.len() > MAX_DENSE_LEN {
        return Err(rt.throw_range_error("Invalid array length: concat result exceeds the dense limit"));
    }
    Ok(Value::Object(rt.new_array(out)))
}

fn array_includes_same_value_zero(rt: &Rt, arr: &Value, v: &Value) -> Res<bool> {
    let mut i = 0usize;
    while (i as f64) < length_of(rt, arr)? {
        let item = array_get(rt, arr, i)?;
        if vn_value::same_value_zero(&item, v) {
            return Ok(true);
        }
        i += 1;
    }
    Ok(false)
}

// ----- props -----

/// guardReactiveProps: falsy gives null; a proxy is copied (an internal object would be too: not representable).
pub fn guard_reactive_props(rt: &Rt, props: &Value) -> Res<Value> {
    if !props.to_boolean() {
        return Ok(Value::Null);
    }
    if is_proxy(rt, props)? {
        return Ok(Value::Object(extend_copy(rt, props)?));
    }
    Ok(props.clone())
}

/// normalizeKey: `key != null ? key : null`.
fn normalize_key(rt: &Rt, props: &Value) -> Res<Value> {
    let k = rt.get_str(props, "key")?;
    Ok(if k.is_nullish() { Value::Null } else { k })
}

/// normalizeRef: a number becomes a string; a string, a ref or a function becomes `{ i, r, k, f }` (i is null: no
/// rendering instance); any other non-null value is kept as it is.
fn normalize_ref(rt: &Rt, props: &Value) -> Res<Value> {
    let mut r = rt.get_str(props, "ref")?;
    let rk = rt.get_str(props, "ref_key")?;
    let rf = rt.get_str(props, "ref_for")?;
    if let Value::Number(_) = r {
        r = Value::String(rt.to_string(&r)?);
    }
    if r.is_nullish() {
        return Ok(Value::Null);
    }
    if matches!(r, Value::String(_)) || normalize::is_ref(rt, &r)? || rt.is_callable(&r) {
        let o = rt.new_object();
        rt.define_data(o, PropKey::from_str("i"), current_rendering_value(rt), Flags::ALL)?;
        rt.define_data(o, PropKey::from_str("r"), r, Flags::ALL)?;
        rt.define_data(o, PropKey::from_str("k"), rk, Flags::ALL)?;
        rt.define_data(o, PropKey::from_str("f"), Value::Bool(rf.to_boolean()), Flags::ALL)?;
        return Ok(Value::Object(o));
    }
    Ok(r)
}

/// mergeProps(...args): class and style are normalized and joined, on* handlers become arrays, other keys are
/// overwritten by later arguments (empty keys are dropped).
pub fn merge_props(rt: &Rt, args: &[Value]) -> Res<ObjRef> {
    let ret = rt.new_object();
    let rv = Value::Object(ret);
    for to_merge in args {
        for key in for_in_keys(rt, to_merge)? {
            let name = key_js(&key);
            if name.units() == JsStr::from_str("class").units() {
                let (rc, tc) = (rt.get_str(&rv, "class")?, rt.get_str(to_merge, "class")?);
                if !strict_eq(&rc, &tc) {
                    let arr = Value::Object(rt.new_array(vec![rc, tc]));
                    let n = normalize_class(rt, &arr)?;
                    rt.set_v(&rv, &PropKey::from_str("class"), Value::String(n))?;
                }
            } else if name.units() == JsStr::from_str("style").units() {
                let (rs, ts) = (rt.get_str(&rv, "style")?, rt.get_str(to_merge, "style")?);
                let arr = Value::Object(rt.new_array(vec![rs, ts]));
                let n = normalize_style(rt, &arr)?;
                rt.set_v(&rv, &PropKey::from_str("style"), n)?;
            } else if normalize::is_on(&name) {
                let existing = rt.get_v(&rv, &key)?;
                let incoming = rt.get_v(to_merge, &key)?;
                let dup = rt.is_array(&existing) && array_includes_same_value_zero(rt, &existing, &incoming)?;
                if incoming.to_boolean() && !strict_eq(&existing, &incoming) && !dup {
                    let v = if existing.to_boolean() { js_concat(rt, &[existing, incoming])? } else { incoming };
                    normalize::set_key(rt, &rv, &key, v)?;
                } else if incoming.is_nullish() && existing.is_nullish() && !is_model_listener(&name) {
                    normalize::set_key(rt, &rv, &key, incoming)?;
                }
            } else if !name.units().is_empty() {
                let v = rt.get_v(to_merge, &key)?;
                normalize::set_key(rt, &rv, &key, v)?;
            }
        }
    }
    Ok(ret)
}

// ----- children -----

/// normalizeChildren: sets `children` and the SLOTS/ARRAY/TEXT bits of the shape. Element and teleport children that
/// are functions or slot objects are rendered at once (the default slot is called with no arguments).
fn normalize_children(rt: &Rt, st: &State, vnode: ObjRef, children: Value) -> Res<()> {
    let mut ty = 0.0;
    let mut children = children;
    let shape = fields(rt, vnode, |f| f.shape.clone());
    let elem_or_teleport = |rt: &Rt| -> Res<bool> { Ok(rt.bit_and(&shape, &Value::Number(1.0 + 64.0))?.to_boolean()) };
    if children.is_nullish() {
        children = Value::Null;
    } else if rt.is_array(&children) {
        ty = 16.0;
    } else if matches!(children, Value::Object(_)) && !rt.is_callable(&children) {
        if elem_or_teleport(rt)? {
            let slot = rt.get_str(&children, "default")?;
            if slot.to_boolean() {
                if rt.get_str(&slot, "_c")?.to_boolean() {
                    rt.set_str(&slot, "_d", Value::Bool(false))?;
                }
                let rendered = rt.call(&slot, &Value::Undefined, &[])?;
                normalize_children(rt, st, vnode, rendered)?;
                if rt.get_str(&slot, "_c")?.to_boolean() {
                    rt.set_str(&slot, "_d", Value::Bool(true))?;
                }
            }
            return Ok(());
        }
        ty = 32.0;
        let slot_flag = rt.get_str(&children, "_")?;
        if !slot_flag.to_boolean() {
            // isInternalObject is never true here (no internal prototype), so `_ctx` is written.
            rt.set_str(&children, "_ctx", current_rendering_value(rt))?;
        }
    } else if rt.is_callable(&children) {
        if elem_or_teleport(rt)? {
            let holder = rt.new_object();
            rt.define_data(holder, PropKey::from_str("default"), children, Flags::ALL)?;
            return normalize_children(rt, st, vnode, Value::Object(holder));
        }
        let holder = rt.new_object();
        rt.define_data(holder, PropKey::from_str("default"), children, Flags::ALL)?;
        rt.define_data(holder, PropKey::from_str("_ctx"), current_rendering_value(rt), Flags::ALL)?;
        children = Value::Object(holder);
        ty = 32.0;
    } else {
        let text = normalize::string_of(rt, &children)?;
        children = Value::String(text.clone());
        if rt.bit_and(&shape, &Value::Number(64.0))?.to_boolean() {
            ty = 16.0;
            let t = create_text_vnode(rt, st, Value::String(text), Value::Number(0.0))?;
            children = Value::Object(rt.new_array(vec![Value::Object(t)]));
        } else {
            ty = 8.0;
        }
    }
    let cur = fields(rt, vnode, |f| f.shape.clone());
    let next = rt.bit_or(&cur, &Value::Number(ty))?;
    fields(rt, vnode, |f| {
        f.children = children;
        f.shape = next;
    });
    Ok(())
}

// ----- vnode construction -----

/// createBaseVNode (runtime-core): the object, its key and ref, children normalization when asked, and the
/// tracking of the vnode in the current block.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_base_vnode(
    rt: &Rt,
    st: &State,
    ty: Value,
    props: Value,
    children: Value,
    patch: Value,
    dyn_props: Value,
    shape: Value,
    is_block: bool,
    need_full: bool,
) -> Res<ObjRef> {
    let key = if props.to_boolean() { normalize_key(rt, &props)? } else { props.clone() };
    let reference = if props.to_boolean() { normalize_ref(rt, &props)? } else { props.clone() };
    let mut f = Fields::new();
    f.scope_id = current_scope_id(rt)?;
    f.ty = ty.clone();
    f.props = props;
    f.key = key;
    f.reference = reference;
    f.children = children.clone();
    f.shape = shape.clone();
    f.patch = patch;
    f.dynamic_props = dyn_props;
    let vnode = alloc(rt, f);
    if need_full {
        normalize_children(rt, st, vnode, children.clone())?;
        if rt.bit_and(&shape, &Value::Number(f64::from(ShapeFlags::SUSPENSE)))?.to_boolean() {
            let norm = rt.get_v(&ty, &PropKey::from_str("normalize"))?;
            rt.call(&norm, &ty, &[Value::Object(vnode)])?;
        }
    } else if children.to_boolean() {
        let bits = if matches!(children, Value::String(_)) { f64::from(ShapeFlags::TEXT_CHILDREN) } else { f64::from(ShapeFlags::ARRAY_CHILDREN) };
        let cur = fields(rt, vnode, |f| f.shape.clone());
        let next = rt.bit_or(&cur, &Value::Number(bits))?;
        fields(rt, vnode, |f| f.shape = next);
    }
    if st.tracking.get() > 0.0 && !is_block {
        if let Some(cur) = st.current.get() {
            let patch_v = fields(rt, vnode, |f| f.patch.clone());
            let cond = rt.gt(&patch_v, &Value::Number(0.0))?.to_boolean()
                || rt.bit_and(&shape, &Value::Number(f64::from(ShapeFlags::COMPONENT)))?.to_boolean();
            if cond && !strict_eq(&patch_v, &Value::Number(f64::from(PatchFlags::NEED_HYDRATION))) {
                block_push(rt, cur, vnode);
            }
        }
    }
    Ok(vnode)
}

fn is_model_listener(name: &JsStr) -> bool {
    name.starts_with(&JsStr::from_str("onUpdate:"), 0)
}

// ----- createVNode and cloning -----

/// _createVNode (runtime-core): a VNode type is cloned with the props as extra props, other types get their shape
/// from the type (string 1, Suspense 128, Teleport 64, object 4, function 2, else 0).
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_vnode(
    rt: &Rt,
    st: &State,
    ty: Value,
    props: Value,
    children: Value,
    patch: Value,
    dyn_props: Value,
    is_block: bool,
) -> Res<ObjRef> {
    let ndc = rt.symbol_for(&JsStr::from_str("v-ndc"));
    let mut ty = if !ty.to_boolean() || is_sym(&ty, ndc) { sym_value(rt, "v-cmt") } else { ty };
    if is_vnode_value(rt, &ty)? {
        let Some(src) = as_vnode(rt, &ty) else {
            return Err(rt.throw_type_error("createVNode: a VNode-like object that is not a VNode is not supported"));
        };
        let cloned = clone_vnode(rt, st, src, &props, true, false)?;
        if children.to_boolean() {
            normalize_children(rt, st, cloned, children)?;
        }
        if st.tracking.get() > 0.0 && !is_block {
            if let Some(cur) = st.current.get() {
                let shape = fields(rt, cloned, |f| f.shape.clone());
                if rt.bit_and(&shape, &Value::Number(f64::from(ShapeFlags::COMPONENT)))?.to_boolean() {
                    block_replace(rt, cur, src, cloned);
                } else {
                    block_push(rt, cur, cloned);
                }
            }
        }
        fields(rt, cloned, |f| f.patch = Value::Number(-2.0));
        return Ok(cloned);
    }
    if rt.is_callable(&ty) && rt.in_operator(&Value::str("__vccOpts"), &ty)? {
        ty = rt.get_str(&ty, "__vccOpts")?;
    }
    let mut props = props;
    if props.to_boolean() {
        props = guard_reactive_props(rt, &props)?;
        let klass = rt.get_str(&props, "class")?;
        let mut style = rt.get_str(&props, "style")?;
        if klass.to_boolean() && !matches!(klass, Value::String(_)) {
            let n = normalize_class(rt, &klass)?;
            rt.set_str(&props, "class", Value::String(n))?;
        }
        if is_obj(rt, &style) {
            if is_proxy(rt, &style)? && !rt.is_array(&style) {
                style = Value::Object(extend_copy(rt, &style)?);
            }
            let n = normalize_style(rt, &style)?;
            rt.set_str(&props, "style", n)?;
        }
    }
    let kind = if matches!(ty, Value::String(_)) {
        ShapeFlags::ELEMENT
    } else if rt.get_v(&ty, &PropKey::from_str("__isSuspense"))?.to_boolean() {
        ShapeFlags::SUSPENSE
    } else if rt.get_v(&ty, &PropKey::from_str("__isTeleport"))?.to_boolean() {
        ShapeFlags::TELEPORT
    } else if is_obj(rt, &ty) {
        ShapeFlags::STATEFUL_COMPONENT
    } else if rt.is_callable(&ty) {
        ShapeFlags::FUNCTIONAL_COMPONENT
    } else {
        0
    };
    create_base_vnode(rt, st, ty, props, children, patch, dyn_props, Value::Number(f64::from(kind)), is_block, true)
}

/// cloneVNode (runtime-core): the same fields, with extra props merged (and refs merged when asked). The clone has no
/// memo, as in runtime-core.
pub(crate) fn clone_vnode(rt: &Rt, st: &State, src: ObjRef, extra: &Value, merge_ref: bool, clone_transition: bool) -> Res<ObjRef> {
    let s = fields(rt, src, |f| f.clone());
    let merged = if extra.to_boolean() {
        let base = if s.props.to_boolean() { s.props.clone() } else { Value::Object(rt.new_object()) };
        Value::Object(merge_props(rt, &[base, extra.clone()])?)
    } else {
        s.props.clone()
    };
    let key = if merged.to_boolean() { normalize_key(rt, &merged)? } else { merged.clone() };
    let reference = if extra.to_boolean() && rt.get_str(extra, "ref")?.to_boolean() {
        let x = normalize_ref(rt, extra)?;
        if merge_ref && s.reference.to_boolean() {
            if rt.is_array(&s.reference) {
                js_concat(rt, &[s.reference.clone(), x])?
            } else {
                Value::Object(rt.new_array(vec![s.reference.clone(), x]))
            }
        } else {
            x
        }
    } else {
        s.reference.clone()
    };
    if clone_transition && s.transition.to_boolean() {
        return Err(rt.throw_type_error("cloneVNode: transition hooks are not supported"));
    }
    let frag = is_sym(&s.ty, rt.symbol_for(&JsStr::from_str("v-fgt")));
    let patch = if extra.to_boolean() && !frag {
        if strict_eq(&s.patch, &Value::Number(-1.0)) {
            Value::Number(16.0)
        } else {
            rt.bit_or(&s.patch, &Value::Number(16.0))?
        }
    } else {
        s.patch.clone()
    };
    let mut f = s.clone();
    f.props = merged;
    f.key = key;
    f.reference = reference;
    f.patch = patch;
    f.memo = Value::Null;
    f.ss_content = clone_slot_content(rt, st, &s.ss_content)?;
    f.ss_fallback = clone_slot_content(rt, st, &s.ss_fallback)?;
    Ok(alloc(rt, f))
}

fn clone_slot_content(rt: &Rt, st: &State, v: &Value) -> Res<Value> {
    if !v.to_boolean() {
        return Ok(v.clone());
    }
    match as_vnode(rt, v) {
        Some(o) => Ok(Value::Object(clone_vnode(rt, st, o, &Value::Null, false, false)?)),
        None => Err(rt.throw_type_error("cloneVNode: ssContent/ssFallback that is not a VNode is not supported")),
    }
}

/// `child.slice()` of an array: holes are kept as holes.
fn slice_copy(rt: &Rt, arr: &Value) -> Res<ObjRef> {
    let mut out = Vec::new();
    let Value::Object(o) = arr else { return Ok(rt.new_array(out)) };
    let len = length_of(rt, arr)? as usize;
    for i in 0..len {
        let key = PropKey::Index(i as u32);
        if rt.has_property(*o, &key)? {
            out.push(rt.get_v(arr, &key)?);
        } else {
            out.push(Value::Hole);
        }
    }
    Ok(rt.new_array(out))
}

/// normalizeVNode (runtime-core): null and booleans become comments, arrays fragments, VNodes are cloned when mounted,
/// everything else is text of its String() form.
pub fn normalize_vnode(rt: &Rt, st: &State, child: &Value) -> Res<ObjRef> {
    if child.is_nullish() || matches!(child, Value::Bool(_)) {
        return create_vnode(rt, st, sym_value(rt, "v-cmt"), Value::Null, Value::Null, Value::Number(0.0), Value::Null, false);
    }
    if rt.is_array(child) {
        let copy = slice_copy(rt, child)?;
        return create_vnode(rt, st, fragment_symbol(rt), Value::Null, Value::Object(copy), Value::Number(0.0), Value::Null, false);
    }
    if let Some(v) = as_vnode(rt, child) {
        return clone_if_mounted(rt, st, v);
    }
    let text = normalize::string_of(rt, child)?;
    create_vnode(rt, st, sym_value(rt, "v-txt"), Value::Null, Value::String(text), Value::Number(0.0), Value::Null, false)
}

/// cloneIfMounted: a vnode that is not mounted (el null, not hoisted) or memoized is used as it is.
pub fn clone_if_mounted(rt: &Rt, st: &State, child: ObjRef) -> Res<ObjRef> {
    let (el_null, patch, memo) = fields(rt, child, |f| (f.el.is_none(), f.patch.clone(), f.memo.clone()));
    if (el_null && !strict_eq(&patch, &Value::Number(-1.0))) || memo.to_boolean() {
        Ok(child)
    } else {
        clone_vnode(rt, st, child, &Value::Null, false, false)
    }
}

pub(crate) fn create_text_vnode(rt: &Rt, st: &State, text: Value, flag: Value) -> Res<ObjRef> {
    create_vnode(rt, st, sym_value(rt, "v-txt"), Value::Null, text, flag, Value::Null, false)
}

// ----- blocks -----

/// openBlock(disableTracking): pushes a new block (or null when tracking is disabled) and makes it current.
pub(crate) fn open_block(rt: &Rt, st: &State, disable: bool) {
    let b = if disable { None } else { Some(new_block(rt, false)) };
    st.stack.borrow_mut().push(b);
    st.current.set(b);
}

/// closeBlock: pops the stack; the current block becomes the new top (null when empty).
pub(crate) fn close_block(st: &State) {
    st.stack.borrow_mut().pop();
    let top = st.stack.borrow().last().copied().flatten();
    st.current.set(top);
}

/// setupBlock: the vnode gets the current block as its dynamic children (EMPTY when there is none), the block is
/// closed, and the vnode is tracked by the enclosing block.
pub(crate) fn setup_block(rt: &Rt, st: &State, vnode: ObjRef) -> ObjRef {
    let dc = if st.tracking.get() > 0.0 { Value::Object(st.current.get().unwrap_or(st.empty)) } else { Value::Null };
    fields(rt, vnode, |f| f.dynamic_children = dc);
    close_block(st);
    if st.tracking.get() > 0.0 {
        if let Some(cur) = st.current.get() {
            block_push(rt, cur, vnode);
        }
    }
    vnode
}

// ----- memo support (used by helpers) -----

/// The `memo` of a value (a VNode's field, or the property of any other object).
pub(crate) fn memo_of(rt: &Rt, v: &Value) -> Res<Value> {
    match as_vnode(rt, v) {
        Some(o) => Ok(fields(rt, o, |f| f.memo.clone())),
        None => rt.get_v(v, &PropKey::from_str("memo")),
    }
}

/// `v.name = value` for a value that may be a VNode (its field) or any object (a property).
pub(crate) fn set_member(rt: &Rt, v: &Value, name: &str, value: Value) -> Res<()> {
    match as_vnode(rt, v) {
        Some(o) => {
            match name {
                "memo" => fields(rt, o, |f| f.memo = value),
                "cacheIndex" => fields(rt, o, |f| f.cache_index = value),
                "key" => fields(rt, o, |f| f.key = value),
                _ => return Err(rt.throw_type_error(&format!("vnode member {name} is not settable here"))),
            }
            Ok(())
        }
        None => {
            rt.set_v(v, &PropKey::from_str(name), value)?;
            Ok(())
        }
    }
}

/// Pushes a vnode into the current block, when there is one.
pub(crate) fn current_block_push(rt: &Rt, st: &State, v: ObjRef) {
    if let Some(cur) = st.current.get() {
        block_push(rt, cur, v);
    }
}

/// `isBlockTreeEnabled > 0`.
pub(crate) fn tracking_enabled(st: &State) -> bool {
    st.tracking.get() > 0.0
}

/// `currentBlock` is truthy.
pub(crate) fn has_current_block(st: &State) -> bool {
    st.current.get().is_some()
}

// ----- native exports -----

fn n_create_vnode(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ty, props, children) = (c.arg(0), dflt(c.arg(1), Value::Null), dflt(c.arg(2), Value::Null));
    let (patch, dyn_p) = (dflt(c.arg(3), Value::Number(0.0)), dflt(c.arg(4), Value::Null));
    let is_block = c.arg(5).to_boolean();
    with_state(rt, c, |st| Ok(Value::Object(create_vnode(rt, st, ty, props, children, patch, dyn_p, is_block)?)))
}

fn n_create_element_vnode(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let ty = c.arg(0);
    let frag = is_sym(&ty, rt.symbol_for(&JsStr::from_str("v-fgt")));
    let shape = dflt(c.arg(5), Value::Number(if frag { 0.0 } else { 1.0 }));
    let (props, children) = (dflt(c.arg(1), Value::Null), dflt(c.arg(2), Value::Null));
    let (patch, dyn_p) = (dflt(c.arg(3), Value::Number(0.0)), dflt(c.arg(4), Value::Null));
    let (is_block, need_full) = (c.arg(6).to_boolean(), c.arg(7).to_boolean());
    with_state(rt, c, |st| {
        Ok(Value::Object(create_base_vnode(rt, st, ty, props, children, patch, dyn_p, shape, is_block, need_full)?))
    })
}

fn n_create_element_block(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let ty = c.arg(0);
    let frag = is_sym(&ty, rt.symbol_for(&JsStr::from_str("v-fgt")));
    let shape = dflt(c.arg(5), Value::Number(if frag { 0.0 } else { 1.0 }));
    let (props, children) = (dflt(c.arg(1), Value::Null), dflt(c.arg(2), Value::Null));
    let (patch, dyn_p) = (dflt(c.arg(3), Value::Number(0.0)), dflt(c.arg(4), Value::Null));
    with_state(rt, c, |st| {
        let v = create_base_vnode(rt, st, ty, props, children, patch, dyn_p, shape, true, false)?;
        Ok(Value::Object(setup_block(rt, st, v)))
    })
}

fn n_create_block(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ty, props, children) = (c.arg(0), dflt(c.arg(1), Value::Null), dflt(c.arg(2), Value::Null));
    let (patch, dyn_p) = (dflt(c.arg(3), Value::Number(0.0)), dflt(c.arg(4), Value::Null));
    with_state(rt, c, |st| {
        let v = create_vnode(rt, st, ty, props, children, patch, dyn_p, true)?;
        Ok(Value::Object(setup_block(rt, st, v)))
    })
}

fn n_create_text_vnode(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let text = dflt(c.arg(0), Value::str(" "));
    let flag = dflt(c.arg(1), Value::Number(0.0));
    with_state(rt, c, |st| Ok(Value::Object(create_text_vnode(rt, st, text, flag)?)))
}

fn n_create_comment_vnode(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let text = dflt(c.arg(0), Value::str(""));
    let as_block = c.arg(1).to_boolean();
    let comment = sym_value(rt, "v-cmt");
    with_state(rt, c, |st| {
        if as_block {
            open_block(rt, st, false);
            let v = create_vnode(rt, st, comment, Value::Null, text, Value::Number(0.0), Value::Null, true)?;
            Ok(Value::Object(setup_block(rt, st, v)))
        } else {
            Ok(Value::Object(create_vnode(rt, st, comment, Value::Null, text, Value::Number(0.0), Value::Null, false)?))
        }
    })
}

fn n_create_static_vnode(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("createStaticVNode: static HTML strings are not supported (the SFC options disable hoistStatic)"))
}

fn n_open_block(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let disable = c.arg(0).to_boolean();
    with_state(rt, c, |st| open_block(rt, st, disable));
    Ok(Value::Undefined)
}

fn n_set_block_tracking(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Value::Number(delta) = c.arg(0) else {
        return Err(rt.throw_type_error("setBlockTracking: only numeric values are supported"));
    };
    let in_once = c.arg(1).to_boolean();
    with_state(rt, c, |st| {
        st.tracking.set(st.tracking.get() + delta);
        if delta < 0.0 && in_once {
            if let Some(cur) = st.current.get() {
                block_with(rt, cur, |b| b.has_once.set(true));
            }
        }
    });
    Ok(Value::Undefined)
}

fn n_is_vnode(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(is_vnode_value(rt, &c.arg(0))?))
}

fn n_clone_vnode(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Some(src) = as_vnode(rt, &c.arg(0)) else {
        return Err(rt.throw_type_error("cloneVNode: only VNode objects are supported"));
    };
    let (extra, merge_ref, clone_tr) = (c.arg(1), c.arg(2).to_boolean(), c.arg(3).to_boolean());
    with_state(rt, c, |st| Ok(Value::Object(clone_vnode(rt, st, src, &extra, merge_ref, clone_tr)?)))
}

fn n_merge_props(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Object(merge_props(rt, c.args)?))
}

fn n_guard_reactive_props(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    guard_reactive_props(rt, &c.arg(0))
}

/// A native export: `name` and `length` as in Vue (the parameters before the first default).
pub(crate) fn func(rt: &Rt, name: &'static str, len: u32, code: vn_value::CodeFn, data: &[Value]) -> (&'static str, Value) {
    (name, Value::Object(rt.new_native_function(code, name, len, data.to_vec())))
}

/// The exports of this module, each carrying `state` as its function data.
pub(crate) fn exports(rt: &Rt, state: ObjRef) -> Vec<(&'static str, Value)> {
    let d = [Value::Object(state)];
    vec![
        func(rt, "createVNode", 1, n_create_vnode, &d),
        func(rt, "createElementVNode", 1, n_create_element_vnode, &d),
        func(rt, "createElementBlock", 6, n_create_element_block, &d),
        func(rt, "createBlock", 5, n_create_block, &d),
        func(rt, "createTextVNode", 0, n_create_text_vnode, &d),
        func(rt, "createCommentVNode", 0, n_create_comment_vnode, &d),
        func(rt, "createStaticVNode", 2, n_create_static_vnode, &d),
        func(rt, "openBlock", 0, n_open_block, &d),
        func(rt, "setBlockTracking", 1, n_set_block_tracking, &d),
        func(rt, "isVNode", 1, n_is_vnode, &d),
        func(rt, "cloneVNode", 2, n_clone_vnode, &d),
        func(rt, "mergeProps", 0, n_merge_props, &d),
        func(rt, "guardReactiveProps", 1, n_guard_reactive_props, &d),
        ("Fragment", sym_value(rt, "v-fgt")),
        ("Text", sym_value(rt, "v-txt")),
        ("Comment", sym_value(rt, "v-cmt")),
        ("Static", sym_value(rt, "v-stc")),
    ]
}

/// The canonical text of values for the corpora (tests/ui/runtime-a/README.md). It is written in the same way by the
/// Node generator, so the two texts can be compared byte for byte. It reads own data properties only.
pub mod testing {
    use super::*;

    /// Canonical description of `v` (never runs user code).
    pub fn describe(rt: &Rt, v: &Value) -> String {
        let mut out = String::new();
        write_value(rt, v, &mut out, 0);
        out
    }

    /// JSON text of a string (as JSON.stringify writes it for well-formed and lone-surrogate strings).
    pub fn quote(units: &[u16]) -> String {
        let mut out = String::from("\"");
        for r in char::decode_utf16(units.iter().copied()) {
            match r {
                Ok(c) => match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\u{8}' => out.push_str("\\b"),
                    '\u{c}' => out.push_str("\\f"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                    c => out.push(c),
                },
                Err(e) => out.push_str(&format!("\\u{:04x}", e.unpaired_surrogate())),
            }
        }
        out.push('"');
        out
    }

    /// Numbers: -0, NaN and the infinities have tagged forms; other numbers are their String() text.
    pub fn num_text(n: f64) -> String {
        if n == 0.0 && n.is_sign_negative() {
            "{\"$\":\"num\",\"v\":\"-0\"}".to_string()
        } else if n.is_nan() {
            "{\"$\":\"num\",\"v\":\"NaN\"}".to_string()
        } else if n.is_infinite() {
            format!("{{\"$\":\"num\",\"v\":\"{}\"}}", if n > 0.0 { "Infinity" } else { "-Infinity" })
        } else {
            vn_value::number::number_to_string(n)
        }
    }

    /// The type tag of a child in a dynamic-children list: the string itself, `@desc` for a symbol, `fn`, `obj`.
    fn kind_of(rt: &Rt, child: ObjRef) -> String {
        let ty = fields(rt, child, |f| f.ty.clone());
        match ty {
            Value::String(s) => s.to_rust_string_lossy(),
            Value::Symbol(s) => {
                format!("@{}", rt.symbol_description(s).map(|d| d.to_rust_string_lossy()).unwrap_or_else(|| "undefined".to_string()))
            }
            Value::Object(o) if rt.is_callable(&Value::Object(o)) => "fn".to_string(),
            Value::Object(_) => "obj".to_string(),
            _ => "other".to_string(),
        }
    }

    fn write_value(rt: &Rt, v: &Value, out: &mut String, depth: u32) {
        if depth > 200 {
            out.push_str("{\"$\":\"deep\"}");
            return;
        }
        match v {
            Value::Undefined => out.push_str("{\"$\":\"undef\"}"),
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => out.push_str(&num_text(*n)),
            Value::String(s) => out.push_str(&quote(s.units())),
            Value::Symbol(s) => {
                out.push_str("{\"$\":\"sym\",\"desc\":");
                match rt.symbol_description(*s) {
                    Some(d) => out.push_str(&quote(d.units())),
                    None => out.push_str("null"),
                }
                out.push('}');
            }
            Value::Object(o) => write_object(rt, *o, out, depth),
            Value::Uninit | Value::Hole => out.push_str("{\"$\":\"internal\"}"),
        }
    }

    fn own_data(rt: &Rt, o: ObjRef, name: &str) -> Value {
        match rt.get_own_property(o, &PropKey::from_str(name)) {
            Ok(Some(Prop::Data { value, .. })) => value,
            _ => Value::Undefined,
        }
    }

    fn write_object(rt: &Rt, o: ObjRef, out: &mut String, depth: u32) {
        if is_native_vnode(rt, o) {
            write_vnode(rt, o, out, depth);
        } else if rt.is_callable(&Value::Object(o)) {
            out.push_str("{\"$\":\"fn\",\"name\":");
            write_value(rt, &own_data(rt, o, "name"), out, depth + 1);
            out.push_str(",\"length\":");
            write_value(rt, &own_data(rt, o, "length"), out, depth + 1);
            out.push('}');
        } else if rt.is_array(&Value::Object(o)) {
            out.push('[');
            let len = match own_data(rt, o, "length") {
                Value::Number(n) => n as usize,
                _ => 0,
            };
            for i in 0..len {
                if i > 0 {
                    out.push(',');
                }
                match rt.get_own_property(o, &PropKey::Index(i as u32)) {
                    Ok(None) => out.push_str("{\"$\":\"hole\"}"),
                    Ok(Some(Prop::Data { value, .. })) => write_value(rt, &value, out, depth + 1),
                    _ => out.push_str("{\"$\":\"accessor\"}"),
                }
            }
            out.push(']');
        } else if rt.native_payload(o).is_some() {
            out.push_str("{\"$\":\"native\"}");
        } else {
            out.push_str("{\"$\":\"obj\",\"props\":[");
            let mut first = true;
            for k in rt.own_keys(o).unwrap_or_default() {
                let name = match &k {
                    PropKey::Index(i) => i.to_string(),
                    PropKey::Str(s) => s.to_rust_string_lossy(),
                    PropKey::Sym(_) => continue,
                };
                let Ok(Some(p)) = rt.get_own_property(o, &k) else { continue };
                if !p.flags().enumerable {
                    continue;
                }
                if !first {
                    out.push(',');
                }
                first = false;
                out.push('[');
                out.push_str(&quote(&name.encode_utf16().collect::<Vec<u16>>()));
                out.push(',');
                match p {
                    Prop::Data { value, .. } => write_value(rt, &value, out, depth + 1),
                    Prop::Accessor { .. } => out.push_str("{\"$\":\"accessor\"}"),
                }
                out.push(']');
            }
            out.push_str("]}");
        }
    }

    fn write_vnode(rt: &Rt, o: ObjRef, out: &mut String, depth: u32) {
        let f = fields(rt, o, |f| f.clone());
        out.push_str("{\"$\":\"vnode\",\"type\":");
        write_value(rt, &f.ty, out, depth + 1);
        out.push_str(",\"key\":");
        write_value(rt, &f.key, out, depth + 1);
        out.push_str(",\"ref\":");
        write_value(rt, &f.reference, out, depth + 1);
        out.push_str(",\"props\":");
        write_value(rt, &f.props, out, depth + 1);
        out.push_str(",\"children\":");
        write_value(rt, &f.children, out, depth + 1);
        out.push_str(",\"shapeFlag\":");
        write_value(rt, &f.shape, out, depth + 1);
        out.push_str(",\"patchFlag\":");
        write_value(rt, &f.patch, out, depth + 1);
        out.push_str(",\"dynamicProps\":");
        write_value(rt, &f.dynamic_props, out, depth + 1);
        out.push_str(",\"dynamicChildren\":");
        match f.dynamic_children {
            Value::Object(b) => {
                let items = block_items(rt, b);
                out.push_str(&format!("{{\"len\":{},\"kinds\":[", items.len()));
                for (i, c) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&quote(&kind_of(rt, *c).encode_utf16().collect::<Vec<u16>>()));
                }
                out.push_str("]}");
            }
            _ => out.push_str("null"),
        }
        out.push_str(",\"staticCount\":");
        write_value(rt, &f.static_count, out, depth + 1);
        out.push_str(",\"appContext\":");
        write_value(rt, &f.app_context, out, depth + 1);
        out.push('}');
    }
}

/// normalizeVNode with the realm state object of `exports::all` (the renderer and the tests use this entry point).
pub fn normalize_vnode_in(rt: &Rt, state: ObjRef, child: &Value) -> Res<ObjRef> {
    let p = rt.native_payload(state).expect("vnode state object (framework bug)");
    let st = payload_ref::<State>(&*p).expect("vnode state type (framework bug)");
    normalize_vnode(rt, st, child)
}

/// isSameVNodeType (runtime-core): the same type and the same key. Both arguments must be VNodes.
pub fn is_same_vnode_type(rt: &Rt, a: ObjRef, b: ObjRef) -> bool {
    let (ta, ka) = fields(rt, a, |f| (f.ty.clone(), f.key.clone()));
    let (tb, kb) = fields(rt, b, |f| (f.ty.clone(), f.key.clone()));
    strict_eq(&ta, &tb) && strict_eq(&ka, &kb)
}
