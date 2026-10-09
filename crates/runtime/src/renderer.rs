//! vn-runtime::renderer: `baseCreateRenderer` of @vue/runtime-core 3.5.43 (production build) over the DOM host. patch,
//! mount and unmount of elements, text, comments and fragments; children (block, unkeyed and keyed in keyed.rs); props
//! through `dom_props::patch_prop`; component dispatch (component.rs); refs (refs.rs); directive hooks (directives.rs);
//! the render entry point with its flush. Behaviour per @vue/runtime-core 3.5.43 (production build).
//! Deviations: Suspense, Static (hoisted) nodes and hydration are unsupported. Advanced components
//! integrate through teleport.rs, transition.rs and keep_alive.rs; see their documented limitations.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;

use vn_value::{payload_ref, strict_eq, Call, JsStr, NativeObject, ObjRef, PropKey, Res, Rt, Tracer, Value};

use crate::dom_events::{host_error, DomCtx};
use crate::dom_props::patch_prop_with_owner;
use crate::errors::{call_with_error_handling, ErrType};
use crate::host::{Host, HostNode, HostNs, HostResult};
use crate::instance::{inst_state, instance_of};
use crate::normalize::{array_get, for_in_keys, key_js, length_of};
use crate::scheduler::queue_post_render_effect;
use crate::shapes::{PatchFlags, ShapeFlags};
use crate::vnode::{self, fields, Block, State};

pub(crate) const RENDERER: &str = "vn-runtime.renderer";
pub(crate) const VNODE_STATE: &str = "vn-runtime.vnode-state";
pub(crate) const SET_BLOCK_TRACKING: &str = "vn-runtime.setBlockTracking";

/// The realm's renderer: the DOM context it draws with, the vnode state it creates vnodes with, and `container._vnode`
/// of every container it rendered into.
pub struct Renderer {
    pub(crate) ctx: DomCtx,
    data: Rc<dyn NativeObject>,
}

impl Deref for Renderer {
    type Target = RendererData;
    fn deref(&self) -> &RendererData {
        payload_ref::<RendererData>(&*self.data).expect("renderer payload (framework bug)")
    }
}

pub struct RendererData {
    host: crate::host::SharedHost,
    state: Rc<RefCell<crate::dom_events::DomState>>,
    pub(crate) st: ObjRef,
    pub(crate) roots: RefCell<HashMap<HostNode, ObjRef>>,
    pub(crate) flushing: Cell<bool>,
    /// `pendingSetRefMap`: the deferred set of each raw ref object, with its job.
    pub(crate) pending_refs: RefCell<crate::refs::PendingRefs>,
}

impl NativeObject for RendererData {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.st);
        for v in self.roots.borrow().values() {
            t.mark(*v);
        }
        self.pending_refs.borrow().trace(t);
    }
}

/// Borrowed view of the installed renderer (kept alive by its payload `Rc`).
pub(crate) struct RendererRef(Renderer);

impl Deref for RendererRef {
    type Target = Renderer;
    fn deref(&self) -> &Renderer {
        &self.0
    }
}

/// The vnode state shared by every export of the realm's `vue` module and by the renderer (block stack, tracking).
pub fn vnode_state(rt: &Rt) -> ObjRef {
    if let Some(o) = rt.builtin(VNODE_STATE) {
        return o;
    }
    let o = vnode::new_state(rt);
    rt.register_builtin(VNODE_STATE, o);
    for (name, v) in vnode::exports(rt, o) {
        let builtin = match name {
            "setBlockTracking" => SET_BLOCK_TRACKING,
            "createVNode" => "vn-runtime.createVNode",
            "openBlock" => "vn-runtime.openBlock",
            "createBlock" => "vn-runtime.createBlock",
            _ => continue,
        };
        if let Value::Object(f) = v {
            rt.register_builtin(builtin, f);
        }
    }
    o
}

/// A function registered by `vnode_state` (`vn-runtime.createVNode`, `vn-runtime.openBlock`, ...).
pub(crate) fn builtin_fn(rt: &Rt, name: &str) -> Value {
    match rt.builtin(name) {
        Some(f) => Value::Object(f),
        None => panic!("{name} is registered with the vnode state (framework bug)"),
    }
}

/// The `setBlockTracking` function of the vnode layer (withCtx and renderSlot turn block tracking off through it).
pub(crate) fn block_tracking_fn(rt: &Rt) -> ObjRef {
    rt.builtin(SET_BLOCK_TRACKING).expect("setBlockTracking is registered with the vnode state (framework bug)")
}

/// Installs the renderer of the realm on `ctx` (replaces an earlier one). Returns the renderer object.
pub fn install(ctx: &DomCtx) -> ObjRef {
    let rt = ctx.rt().clone();
    let st = vnode_state(&rt);
    let r = RendererData {
        host: ctx.host.clone(),
        state: ctx.state.clone(),
        st,
        roots: RefCell::new(HashMap::new()),
        flushing: Cell::new(false),
        pending_refs: RefCell::new(crate::refs::PendingRefs(Vec::new())),
    };
    let o = rt.new_native_object(None, Rc::new(r));
    rt.register_builtin(RENDERER, o);
    o
}

/// The installed renderer of the realm. A TypeError when `install` has not run (no DOM host).
pub(crate) fn renderer(rt: &Rt) -> Res<RendererRef> {
    let Some(o) = rt.builtin(RENDERER) else {
        return Err(rt.throw_type_error("no DOM host is installed for the runtime renderer"));
    };
    let p = rt.native_payload(o).expect("renderer object (framework bug)");
    let data = payload_ref::<RendererData>(&*p).expect("renderer payload (framework bug)");
    let ctx = DomCtx { rt: rt.clone(), host: data.host.clone(), state: data.state.clone() };
    Ok(RendererRef(Renderer { ctx, data: p }))
}

/// `render(vnode, container)` of runtime-dom for a host container. `None` unmounts.
pub fn render_into(rt: &Rt, vnode: Option<ObjRef>, container: HostNode) -> Res<()> {
    renderer(rt)?.render(vnode, container)
}

// ----- vnode field helpers -----

pub(crate) fn num(v: &Value) -> i32 {
    match v {
        Value::Number(n) => *n as i32,
        _ => 0,
    }
}

pub(crate) fn hn(x: Option<u64>) -> Option<HostNode> {
    x.map(|v| HostNode(v as u32))
}

pub(crate) fn hid(n: Option<HostNode>) -> Option<u64> {
    n.map(|h| h.0 as u64)
}

/// `Symbol.for(name)` for the vnode type symbols ("v-fgt", "v-txt", "v-cmt", "v-stc").
pub(crate) fn sym(rt: &Rt, name: &str) -> Value {
    Value::Symbol(rt.symbol_for(&JsStr::from_str(name)))
}

pub(crate) fn ty_of(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.ty.clone())
}
pub(crate) fn shape_of(rt: &Rt, o: ObjRef) -> i32 {
    fields(rt, o, |f| num(&f.shape))
}
pub(crate) fn patch_of(rt: &Rt, o: ObjRef) -> i32 {
    fields(rt, o, |f| num(&f.patch))
}
pub(crate) fn el_of(rt: &Rt, o: ObjRef) -> Option<HostNode> {
    fields(rt, o, |f| hn(f.el))
}
pub(crate) fn set_el(rt: &Rt, o: ObjRef, n: Option<HostNode>) {
    let handle = n.and_then(|n| renderer(rt).ok()?.ctx.handle(n).as_object());
    fields(rt, o, |f| { f.el = hid(n); f.el_handle = handle; });
}
pub(crate) fn anchor_of(rt: &Rt, o: ObjRef) -> Option<HostNode> {
    fields(rt, o, |f| hn(f.anchor))
}
pub(crate) fn set_anchor(rt: &Rt, o: ObjRef, n: Option<HostNode>) {
    let handle = n.and_then(|n| renderer(rt).ok()?.ctx.handle(n).as_object());
    fields(rt, o, |f| { f.anchor = hid(n); f.anchor_handle = handle; });
}
pub(crate) fn props_of(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.props.clone())
}
pub(crate) fn children_of(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.children.clone())
}
pub(crate) fn key_of(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.key.clone())
}
pub(crate) fn ref_of(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.reference.clone())
}
pub(crate) fn dirs_of(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.dirs.clone())
}
pub(crate) fn component_of(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.component.clone())
}
pub(crate) fn dyn_children_of(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.dynamic_children.clone())
}
pub(crate) fn dyn_props_of(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.dynamic_props.clone())
}
pub(crate) fn scope_id_of(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.scope_id.clone())
}
pub(crate) fn slot_scope_ids_of(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.slot_scope_ids.clone())
}
pub(crate) fn transition_of(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.transition.clone())
}
pub(crate) fn memo_of_vnode(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.memo.clone())
}
pub(crate) fn cache_index_of(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.cache_index.clone())
}
pub(crate) fn ctx_of_vnode(rt: &Rt, o: ObjRef) -> Value {
    fields(rt, o, |f| f.ctx.clone())
}

/// `dynamicChildren` is a block object (or null). Its items and its `hasOnce` mark.
pub(crate) fn block_items(rt: &Rt, v: &Value) -> Option<Vec<ObjRef>> {
    match v {
        Value::Object(b) => Some(vnode::block_items(rt, *b)),
        _ => None,
    }
}

pub(crate) fn block_has_once(rt: &Rt, v: &Value) -> bool {
    match v {
        Value::Object(b) => {
            let Some(p) = rt.native_payload(*b) else { return false };
            payload_ref::<Block>(&*p).is_some_and(|blk| blk.has_once.get())
        }
        _ => false,
    }
}

/// A vnode that is a child of an array (a direct read, as runtime-core does after normalization).
pub(crate) fn vnode_at(rt: &Rt, arr: &Value, i: usize) -> Res<ObjRef> {
    let v = array_get(rt, arr, i)?;
    match vnode::as_vnode(rt, &v) {
        Some(o) => Ok(o),
        None => Err(rt.throw_type_error("renderer: child is not a normalized vnode (framework bug)")),
    }
}

pub(crate) fn array_len(rt: &Rt, v: &Value) -> Res<usize> {
    if !v.is_nullish() && matches!(v, Value::Object(_)) {
        Ok(length_of(rt, v)? as usize)
    } else {
        Ok(0)
    }
}

/// isReservedProp of @vue/shared: the empty key and the key, ref, ref_for, ref_key and onVnode* names.
pub(crate) fn is_reserved_prop(key: &str) -> bool {
    matches!(
        key,
        "" | "key"
            | "ref"
            | "ref_for"
            | "ref_key"
            | "onVnodeBeforeMount"
            | "onVnodeMounted"
            | "onVnodeBeforeUpdate"
            | "onVnodeUpdated"
            | "onVnodeBeforeUnmount"
            | "onVnodeUnmounted"
    )
}

pub(crate) fn key_string(k: &PropKey) -> String {
    key_js(k).to_rust_string_lossy()
}

// ----- deferred work: the post-flush queue of Vue's `queuePostRenderEffect` closures -----

/// One closure of Vue's post-flush queue, run in order by `run_steps`.
#[derive(Clone)]
pub(crate) enum Step {
    /// `invokeVNodeHook(hook, instance, vnode, prevVNode)`.
    Hook(Value, Option<ObjRef>, ObjRef, Option<ObjRef>),
    /// `invokeDirectiveHook(vnode, prevVNode, instance, name)`.
    Dir(&'static str, ObjRef, Option<ObjRef>, Option<ObjRef>),
    /// `vnode.el = null` (memo invalidation).
    ClearEl(ObjRef),
    /// `instance.isUnmounted = true`.
    Unmounted(ObjRef),
}

pub(crate) struct Steps(pub(crate) Vec<Step>);

impl NativeObject for Steps {
    fn trace(&self, t: &mut Tracer) {
        for s in &self.0 {
            match s {
                Step::Hook(h, p, v, pv) => {
                    t.mark_value(h);
                    mark_opt(t, *p);
                    t.mark(*v);
                    mark_opt(t, *pv);
                }
                Step::Dir(_, v, pv, p) => {
                    t.mark(*v);
                    mark_opt(t, *pv);
                    mark_opt(t, *p);
                }
                Step::ClearEl(o) | Step::Unmounted(o) => t.mark(*o),
            }
        }
    }
}

fn mark_opt(t: &mut Tracer, o: Option<ObjRef>) {
    if let Some(o) = o {
        t.mark(o);
    }
}

fn opt_value(o: Option<ObjRef>) -> Value {
    match o {
        Some(o) => Value::Object(o),
        None => Value::Null,
    }
}

fn run_step(rt: &Rt, r: &Renderer, s: Step) -> Res<()> {
    match s {
        Step::Hook(h, p, v, pv) => r.invoke_vnode_hook(&h, p, v, pv),
        Step::Dir(name, v, pv, p) => r.invoke_directive_hook(v, pv, p, name),
        Step::ClearEl(v) => {
            set_el(rt, v, None);
            Ok(())
        }
        Step::Unmounted(i) => {
            instance_of(rt, i).is_unmounted.set(true);
            Ok(())
        }
    }
}

fn run_steps_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let Some(Value::Object(o)) = data.first() else {
        panic!("steps function without its payload (framework bug)");
    };
    let p = rt.native_payload(*o).expect("steps payload (framework bug)");
    let steps: Vec<Step> = payload_ref::<Steps>(&*p).expect("steps payload type (framework bug)").0.clone();
    let r = renderer(rt)?;
    for s in steps {
        run_step(rt, &r, s)?;
    }
    Ok(Value::Undefined)
}

// ----- the renderer -----

impl Renderer {
    pub(crate) fn rt(&self) -> &Rt {
        &self.ctx.rt
    }

    pub(crate) fn host(&self) -> Box<dyn Host> {
        self.ctx.host()
    }

    pub(crate) fn hres<T>(&self, r: HostResult<T>) -> Res<T> {
        r.map_err(|e| host_error(&self.ctx.rt, e))
    }

    pub(crate) fn need<T>(&self, o: Option<T>, what: &str) -> Res<T> {
        match o {
            Some(v) => Ok(v),
            None => Err(self.rt().throw_type_error(&format!("renderer: missing host node ({what})"))),
        }
    }

    pub(crate) fn to_s(&self, v: &Value) -> Res<String> {
        Ok(self.rt().to_string(v)?.to_rust_string_lossy())
    }

    pub(crate) fn ins(&self, child: HostNode, parent: HostNode, anchor: Option<HostNode>) -> Res<()> {
        let mut h = self.host();
        let r = h.insert(child, parent, anchor);
        self.hres(r)
    }

    pub(crate) fn rm(&self, child: HostNode) -> Res<()> {
        let mut h = self.host();
        let r = h.remove(child);
        self.hres(r)?;
        self.ctx.retire(child);
        Ok(())
    }

    pub(crate) fn create_text(&self, s: &str) -> HostNode {
        let mut h = self.host();
        h.create_text(s)
    }

    pub(crate) fn create_comment(&self, s: &str) -> HostNode {
        let mut h = self.host();
        h.create_comment(s)
    }

    pub(crate) fn set_text(&self, n: HostNode, s: &str) -> Res<()> {
        let mut h = self.host();
        let r = h.set_text(n, s);
        self.hres(r)
    }

    pub(crate) fn set_element_text(&self, el: HostNode, s: &str) -> Res<()> {
        let mut h = self.host();
        let r = h.set_element_text(el, s);
        self.hres(r)
    }

    pub(crate) fn parent_of(&self, n: HostNode) -> Option<HostNode> {
        self.host().parent_node(n)
    }

    pub(crate) fn next_of(&self, n: HostNode) -> Option<HostNode> {
        self.host().next_sibling(n)
    }

    pub(crate) fn set_attr(&self, el: HostNode, name: &str, value: &str) -> Res<()> {
        let mut h = self.host();
        let r = h.set_attribute(el, name, value);
        self.hres(r)
    }

    /// Runs `f` on the vnode state (`normalize_vnode` and `clone_if_mounted` need it).
    pub(crate) fn with_vst<R>(&self, f: impl FnOnce(&State) -> R) -> R {
        let p = self.rt().native_payload(self.st).expect("vnode state payload (framework bug)");
        let s = payload_ref::<State>(&*p).expect("vnode state type (framework bug)");
        f(s)
    }

    /// `normalizeVNode(child)` or, with `optimized`, `cloneIfMounted(child)`: a vnode for a child value.
    pub(crate) fn norm_value(&self, v: &Value, optimized: bool) -> Res<ObjRef> {
        let rt = self.rt();
        if optimized {
            if let Some(o) = vnode::as_vnode(rt, v) {
                return self.with_vst(|st| vnode::clone_if_mounted(rt, st, o));
            }
        }
        self.with_vst(|st| vnode::normalize_vnode(rt, st, v))
    }

    /// `children[i] = normalize(children[i])`, returning the vnode.
    pub(crate) fn norm_at(&self, arr: &Value, i: usize, optimized: bool) -> Res<ObjRef> {
        let rt = self.rt();
        let raw = array_get(rt, arr, i)?;
        let child = self.norm_value(&raw, optimized)?;
        rt.set_v_strict(arr, &PropKey::Index(i as u32), Value::Object(child))?;
        Ok(child)
    }

    /// Queues `steps` on the post-flush queue (`queuePostRenderEffect(() => ...)`).
    pub(crate) fn queue_steps(&self, steps: Vec<Step>) -> Res<()> {
        if steps.is_empty() {
            return Ok(());
        }
        let rt = self.rt();
        let payload = rt.new_native_object(None, Rc::new(Steps(steps)));
        let f = rt.new_native_function(run_steps_code, "", 0, vec![Value::Object(payload)]);
        queue_post_render_effect(rt, &Value::Object(f), &Value::Null)
    }

    // ----- hooks -----

    /// `invokeVNodeHook(hook, instance, vnode, prevVNode)`: the hook runs with the vnode error handling of runtime-core.
    pub(crate) fn invoke_vnode_hook(&self, hook: &Value, parent: Option<ObjRef>, vnode: ObjRef, prev: Option<ObjRef>) -> Res<()> {
        let args = [Value::Object(vnode), opt_value(prev)];
        call_with_error_handling(self.rt(), hook, parent, ErrType::Code(7), Some(&args)).map(|_| ())
    }

    // ----- patch -----

    /// `patch(n1, n2, container, anchor, parentComponent, namespace, slotScopeIds, optimized)`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn patch(
        &self,
        n1: Option<ObjRef>,
        n2: ObjRef,
        container: HostNode,
        anchor: Option<HostNode>,
        parent: Option<ObjRef>,
        ns: HostNs,
        ssi: &Value,
        optimized: bool,
    ) -> Res<()> {
        let rt = self.rt();
        if n1 == Some(n2) {
            return Ok(());
        }
        let mut anchor = anchor;
        let mut n1 = n1;
        if let Some(old) = n1 {
            if !vnode::is_same_vnode_type(rt, old, n2) {
                anchor = self.next_host_node(old)?;
                self.unmount(old, parent, true, false)?;
                n1 = None;
            }
        }
        let mut optimized = optimized;
        if patch_of(rt, n2) == PatchFlags::BAIL {
            optimized = false;
            fields(rt, n2, |f| f.dynamic_children = Value::Null);
        }
        if let Some(old) = n1 {
            let new_dyn = dyn_children_of(rt, n2);
            if block_items(rt, &new_dyn).is_some() && block_has_once(rt, &dyn_children_of(rt, old)) {
                // `n2.dynamicChildren.hasOnce = true` (EMPTY_ARR is replaced by a fresh block, as Vue does)
                let b = self.fresh_block(true, block_items(rt, &new_dyn).unwrap_or_default());
                fields(rt, n2, |f| f.dynamic_children = Value::Object(b));
            }
        }
        let ty = ty_of(rt, n2);
        let reference = ref_of(rt, n2);
        let shape = shape_of(rt, n2);
        if is_type(rt, &ty, "v-txt") {
            self.process_text(n1, n2, container, anchor)?;
        } else if is_type(rt, &ty, "v-cmt") {
            self.process_comment(n1, n2, container, anchor)?;
        } else if is_type(rt, &ty, "v-stc") {
            return Err(rt.throw_type_error("unsupported: Static (hoisted) vnodes"));
        } else if is_type(rt, &ty, "v-fgt") {
            self.process_fragment(n1, n2, container, anchor, parent, ns, ssi, optimized)?;
        } else if shape & ShapeFlags::ELEMENT != 0 {
            self.process_element(n1, n2, container, anchor, parent, ns, ssi, optimized)?;
        } else if shape & ShapeFlags::COMPONENT != 0 {
            self.process_component(n1, n2, container, anchor, parent, ns, ssi, optimized)?;
        } else if shape & ShapeFlags::TELEPORT != 0 {
            self.process_teleport(n1, n2, container, anchor, parent, ns, ssi)?;
        } else if shape & ShapeFlags::SUSPENSE != 0 {
            return Err(rt.throw_type_error("unsupported: Suspense"));
        }
        // `ref != null && parentComponent` sets the ref; a removed ref is cleared.
        if !reference.is_nullish() && parent.is_some() {
            let old_ref = match n1 {
                Some(o) => Some(ref_of(rt, o)),
                None => None,
            };
            self.set_ref(&reference, old_ref.as_ref(), n2, false, parent)?;
        } else if reference.is_nullish() {
            if let Some(old) = n1 {
                let old_ref = ref_of(rt, old);
                if !old_ref.is_nullish() {
                    self.set_ref(&old_ref, None, old, true, parent)?;
                }
            }
        }
        Ok(())
    }

    /// A fresh block object holding `items`, marked `hasOnce` when asked.
    pub(crate) fn fresh_block(&self, has_once: bool, items: Vec<ObjRef>) -> ObjRef {
        let b = Block { items: RefCell::new(items), has_once: Cell::new(has_once) };
        self.rt().new_native_object(None, Rc::new(b))
    }

    /// `getNextHostNode(vnode)`.
    pub(crate) fn next_host_node(&self, vnode: ObjRef) -> Res<Option<HostNode>> {
        let rt = self.rt();
        let shape = shape_of(rt, vnode);
        if shape & ShapeFlags::COMPONENT != 0 {
            let sub = component_subtree(rt, vnode)?;
            return self.next_host_node(sub);
        }
        if shape & ShapeFlags::SUSPENSE != 0 {
            return Err(rt.throw_type_error("unsupported: Suspense"));
        }
        let from = anchor_of(rt, vnode).or(el_of(rt, vnode));
        Ok(match from {
            Some(n) => self.next_of(n),
            None => None,
        })
    }

    fn process_text(&self, n1: Option<ObjRef>, n2: ObjRef, container: HostNode, anchor: Option<HostNode>) -> Res<()> {
        let rt = self.rt();
        match n1 {
            None => {
                let s = self.to_s(&children_of(rt, n2))?;
                let el = self.create_text(&s);
                set_el(rt, n2, Some(el));
                self.ins(el, container, anchor)
            }
            Some(old) => {
                let el = el_of(rt, old);
                set_el(rt, n2, el);
                let (c2, c1) = (children_of(rt, n2), children_of(rt, old));
                if !strict_eq(&c2, &c1) {
                    let el = self.need(el, "text")?;
                    let s = self.to_s(&c2)?;
                    self.set_text(el, &s)?;
                }
                Ok(())
            }
        }
    }

    fn process_comment(&self, n1: Option<ObjRef>, n2: ObjRef, container: HostNode, anchor: Option<HostNode>) -> Res<()> {
        let rt = self.rt();
        match n1 {
            None => {
                let c = children_of(rt, n2);
                let s = if c.to_boolean() { self.to_s(&c)? } else { String::new() };
                let el = self.create_comment(&s);
                set_el(rt, n2, Some(el));
                self.ins(el, container, anchor)
            }
            Some(old) => {
                set_el(rt, n2, el_of(rt, old));
                Ok(())
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_element(
        &self,
        n1: Option<ObjRef>,
        n2: ObjRef,
        container: HostNode,
        anchor: Option<HostNode>,
        parent: Option<ObjRef>,
        ns: HostNs,
        ssi: &Value,
        optimized: bool,
    ) -> Res<()> {
        let rt = self.rt();
        let mut ns = ns;
        let ty = ty_of(rt, n2);
        if is_str(&ty, "svg") {
            ns = HostNs::Svg;
        } else if is_str(&ty, "math") {
            ns = HostNs::MathMl;
        }
        match n1 {
            None => self.mount_element(n2, container, anchor, parent, ns, ssi, optimized),
            Some(old) => self.patch_element(old, n2, parent, ns, ssi, optimized),
        }
    }

    /// `resolveChildrenNamespace`: foreignObject children are HTML, annotation-xml with an HTML encoding too.
    pub(crate) fn children_ns(&self, vnode: ObjRef, ns: HostNs) -> Res<HostNs> {
        let rt = self.rt();
        let ty = ty_of(rt, vnode);
        if ns == HostNs::Svg && is_str(&ty, "foreignObject") {
            return Ok(HostNs::Html);
        }
        if ns == HostNs::MathMl && is_str(&ty, "annotation-xml") {
            let props = props_of(rt, vnode);
            if props.to_boolean() {
                let enc = rt.get_str(&props, "encoding")?;
                if enc.to_boolean() {
                    let s = self.to_s(&enc)?;
                    if s.contains("html") {
                        return Ok(HostNs::Html);
                    }
                }
            }
        }
        Ok(ns)
    }

    #[allow(clippy::too_many_arguments)]
    fn mount_element(
        &self,
        vnode: ObjRef,
        container: HostNode,
        anchor: Option<HostNode>,
        parent: Option<ObjRef>,
        ns: HostNs,
        ssi: &Value,
        optimized: bool,
    ) -> Res<()> {
        let rt = self.rt();
        let props = props_of(rt, vnode);
        let shape = shape_of(rt, vnode);
        let dirs = dirs_of(rt, vnode);
        let ty = ty_of(rt, vnode);
        let tag = self.to_s(&ty)?;
        let (is, has_props) = if props.to_boolean() {
            let is = rt.get_str(&props, "is")?;
            let is = if is.to_boolean() { Some(self.to_s(&is)?) } else { None };
            (is, true)
        } else {
            (None, false)
        };
        let el = {
            let mut h = self.host();
            let r = h.create_element(ns, &tag, is.as_deref());
            self.hres(r)?
        };
        set_el(rt, vnode, Some(el));
        if tag == "select" && has_props {
            let m = rt.get_str(&props, "multiple")?;
            if !m.is_nullish() {
                let s = self.to_s(&m)?;
                self.set_attr(el, "multiple", &s)?;
            }
        }
        if shape & ShapeFlags::TEXT_CHILDREN != 0 {
            let c = children_of(rt, vnode);
            let s = self.to_s(&c)?;
            self.set_element_text(el, &s)?;
        } else if shape & ShapeFlags::ARRAY_CHILDREN != 0 {
            let c = children_of(rt, vnode);
            let child_ns = self.children_ns(vnode, ns)?;
            self.mount_children(&c, el, None, parent, child_ns, ssi, optimized, 0)?;
        }
        if dirs.to_boolean() {
            self.invoke_directive_hook(vnode, None, parent, "created")?;
        }
        let scope = scope_id_of(rt, vnode);
        self.set_scope_id(el, vnode, &scope, ssi, parent)?;
        if has_props {
            for key in for_in_keys(rt, &props)? {
                let k = key_string(&key);
                if k != "value" && !is_reserved_prop(&k) {
                    let v = rt.get_v(&props, &key)?;
                    patch_prop_with_owner(&self.ctx, el, &k, &Value::Null, &v, ns, parent)?;
                }
            }
            if rt.has_property(props_obj(rt, &props)?, &PropKey::from_str("value"))? {
                let v = rt.get_str(&props, "value")?;
                patch_prop_with_owner(&self.ctx, el, "value", &Value::Null, &v, ns, parent)?;
            }
            let hook = rt.get_str(&props, "onVnodeBeforeMount")?;
            if hook.to_boolean() {
                self.invoke_vnode_hook(&hook, parent, vnode, None)?;
            }
        }
        if dirs.to_boolean() {
            self.invoke_directive_hook(vnode, None, parent, "beforeMount")?;
        }
        if transition_of(rt, vnode).to_boolean() {
            crate::transition::invoke(rt, vnode, "beforeEnter", &[])?;
        }
        self.ins(el, container, anchor)?;
        if transition_of(rt, vnode).to_boolean() { crate::transition::queue_enter(rt, vnode)?; }
        let mounted_hook = if has_props { rt.get_str(&props, "onVnodeMounted")? } else { Value::Undefined };
        if mounted_hook.to_boolean() || dirs.to_boolean() {
            let mut steps = Vec::new();
            if mounted_hook.to_boolean() {
                steps.push(Step::Hook(mounted_hook, parent, vnode, None));
            }
            if dirs.to_boolean() {
                steps.push(Step::Dir("mounted", vnode, None, parent));
            }
            self.queue_steps(steps)?;
        }
        Ok(())
    }

    /// `setScopeId`: the scope attributes of the element and, for a component's root, of its ancestors.
    pub(crate) fn set_scope_id(
        &self,
        el: HostNode,
        vnode: ObjRef,
        scope_id: &Value,
        ssi: &Value,
        parent: Option<ObjRef>,
    ) -> Res<()> {
        let rt = self.rt();
        if scope_id.to_boolean() {
            let s = self.to_s(scope_id)?;
            self.set_attr(el, &s, "")?;
        }
        if let Value::Object(_) = ssi {
            for i in 0..array_len(rt, ssi)? {
                let s = self.to_s(&array_get(rt, ssi, i)?)?;
                self.set_attr(el, &s, "")?;
            }
        }
        if let Some(p) = parent {
            let inst = instance_of(rt, p);
            let sub = inst.sub_tree.borrow().clone();
            if strict_eq(&Value::Object(vnode), &sub) {
                let pv = inst.vnode.borrow().clone();
                let parent_vnode = match vnode::as_vnode(rt, &pv) {
                    Some(v) => v,
                    None => return Err(rt.throw_type_error("renderer: instance without a vnode (framework bug)")),
                };
                let pp = inst.parent.borrow().clone();
                let pid = match pp {
                    Value::Object(o) => Some(o),
                    _ => None,
                };
                let sid = scope_id_of(rt, parent_vnode);
                let sssi = slot_scope_ids_of(rt, parent_vnode);
                self.set_scope_id(el, parent_vnode, &sid, &sssi, pid)?;
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn mount_children(
        &self,
        children: &Value,
        container: HostNode,
        anchor: Option<HostNode>,
        parent: Option<ObjRef>,
        ns: HostNs,
        ssi: &Value,
        optimized: bool,
        start: usize,
    ) -> Res<()> {
        let rt = self.rt();
        let len = array_len(rt, children)?;
        for i in start..len {
            let child = self.norm_at(children, i, optimized)?;
            self.patch(None, child, container, anchor, parent, ns, ssi, optimized)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn patch_element(
        &self,
        n1: ObjRef,
        n2: ObjRef,
        parent: Option<ObjRef>,
        ns: HostNs,
        ssi: &Value,
        optimized: bool,
    ) -> Res<()> {
        let rt = self.rt();
        let el = self.need(el_of(rt, n1), "element")?;
        set_el(rt, n2, Some(el));
        let (mut patch, mut dyn_children, dirs) = fields(rt, n2, |f| (num(&f.patch), f.dynamic_children.clone(), f.dirs.clone()));
        let old_patch = patch_of(rt, n1);
        patch |= old_patch & PatchFlags::FULL_PROPS;
        let old_props = {
            let p = props_of(rt, n1);
            if p.to_boolean() { p } else { crate::instance::empty_obj(rt) }
        };
        let new_props = {
            let p = props_of(rt, n2);
            if p.to_boolean() { p } else { crate::instance::empty_obj(rt) }
        };
        let mut optimized = optimized;
        if let Some(_) = block_items(rt, &dyn_children) {
            if block_items(rt, &dyn_children_of(rt, n1)).map(|v| v.len()) != block_items(rt, &dyn_children).map(|v| v.len()) {
                patch = 0;
                optimized = false;
                dyn_children = Value::Null;
            }
        }
        let hook = rt.get_str(&new_props, "onVnodeBeforeUpdate")?;
        if hook.to_boolean() {
            self.invoke_vnode_hook(&hook, parent, n2, Some(n1))?;
        }
        if dirs.to_boolean() {
            self.invoke_directive_hook(n2, Some(n1), parent, "beforeUpdate")?;
        }
        let inner_html_old = rt.get_str(&old_props, "innerHTML")?;
        let inner_html_new = rt.get_str(&new_props, "innerHTML")?;
        let text_old = rt.get_str(&old_props, "textContent")?;
        let text_new = rt.get_str(&new_props, "textContent")?;
        if (inner_html_old.to_boolean() && inner_html_new.is_nullish()) || (text_old.to_boolean() && text_new.is_nullish()) {
            self.set_element_text(el, "")?;
        }
        let child_ns = self.children_ns(n2, ns)?;
        if let Some(new_items) = block_items(rt, &dyn_children) {
            let old_dyn = dyn_children_of(rt, n1);
            let old_items = block_items(rt, &old_dyn).unwrap_or_default();
            self.patch_block_children(&old_items, &new_items, el, parent, child_ns, ssi)?;
        } else if !optimized {
            self.patch_children(Some(n1), n2, el, None, parent, child_ns, ssi, false)?;
        }
        if patch > 0 {
            if patch & PatchFlags::FULL_PROPS != 0 {
                self.patch_props(el, &old_props, &new_props, parent, ns)?;
            } else {
                if patch & PatchFlags::CLASS != 0 {
                    let (a, b) = (rt.get_str(&old_props, "class")?, rt.get_str(&new_props, "class")?);
                    if !strict_eq(&a, &b) {
                        patch_prop_with_owner(&self.ctx, el, "class", &Value::Null, &b, ns, parent)?;
                    }
                }
                if patch & PatchFlags::STYLE != 0 {
                    let (a, b) = (rt.get_str(&old_props, "style")?, rt.get_str(&new_props, "style")?);
                    patch_prop_with_owner(&self.ctx, el, "style", &a, &b, ns, parent)?;
                }
                if patch & PatchFlags::PROPS != 0 {
                    let to_update = dyn_props_of(rt, n2);
                    for i in 0..array_len(rt, &to_update)? {
                        let key = self.to_s(&array_get(rt, &to_update, i)?)?;
                        let pk = PropKey::from_str(&key);
                        let prev = rt.get_v(&old_props, &pk)?;
                        let next = rt.get_v(&new_props, &pk)?;
                        if !strict_eq(&next, &prev) || key == "value" {
                            patch_prop_with_owner(&self.ctx, el, &key, &prev, &next, ns, parent)?;
                        }
                    }
                }
            }
            if patch & PatchFlags::TEXT != 0 {
                let (a, b) = (children_of(rt, n1), children_of(rt, n2));
                if !strict_eq(&a, &b) {
                    let s = self.to_s(&b)?;
                    self.set_element_text(el, &s)?;
                }
            }
        } else if !optimized && block_items(rt, &dyn_children).is_none() {
            self.patch_props(el, &old_props, &new_props, parent, ns)?;
        }
        let updated_hook = rt.get_str(&new_props, "onVnodeUpdated")?;
        if updated_hook.to_boolean() || dirs.to_boolean() {
            let mut steps = Vec::new();
            if updated_hook.to_boolean() {
                steps.push(Step::Hook(updated_hook, parent, n2, Some(n1)));
            }
            if dirs.to_boolean() {
                steps.push(Step::Dir("updated", n2, Some(n1), parent));
            }
            self.queue_steps(steps)?;
        }
        Ok(())
    }

    /// `patchBlockChildren`: the dynamic children of a block, each with its own container.
    pub(crate) fn patch_block_children(
        &self,
        old_children: &[ObjRef],
        new_children: &[ObjRef],
        fallback: HostNode,
        parent: Option<ObjRef>,
        ns: HostNs,
        ssi: &Value,
    ) -> Res<()> {
        let rt = self.rt();
        for (i, new_vnode) in new_children.iter().enumerate() {
            let Some(old_vnode) = old_children.get(i).copied() else {
                return Err(rt.throw_type_error("unsupported: block children with different lengths"));
            };
            let old_el = el_of(rt, old_vnode);
            let container = match old_el {
                Some(el)
                    if ty_is_fragment(rt, old_vnode)
                        || !vnode::is_same_vnode_type(rt, old_vnode, *new_vnode)
                        || shape_of(rt, old_vnode) & (ShapeFlags::COMPONENT | ShapeFlags::TELEPORT | ShapeFlags::SUSPENSE) != 0 =>
                {
                    self.need(self.parent_of(el), "block parent")?
                }
                _ => fallback,
            };
            self.patch(Some(old_vnode), *new_vnode, container, None, parent, ns, ssi, true)?;
        }
        Ok(())
    }

    pub(crate) fn patch_props(
        &self,
        el: HostNode,
        old_props: &Value,
        new_props: &Value,
        parent: Option<ObjRef>,
        ns: HostNs,
    ) -> Res<()> {
        let rt = self.rt();
        if old_props.to_boolean() && new_props.to_boolean() && !strict_eq(old_props, new_props) {
            // removed keys (old not in new)
            for key in for_in_keys(rt, old_props)? {
                let k = key_string(&key);
                if !is_reserved_prop(&k) && !rt.has_property(props_obj(rt, new_props)?, &key)? {
                    let prev = rt.get_v(old_props, &key)?;
                    patch_prop_with_owner(&self.ctx, el, &k, &prev, &Value::Null, ns, parent)?;
                }
            }
            for key in for_in_keys(rt, new_props)? {
                let k = key_string(&key);
                if is_reserved_prop(&k) {
                    continue;
                }
                let next = rt.get_v(new_props, &key)?;
                let prev = rt.get_v(old_props, &key)?;
                if !strict_eq(&next, &prev) && k != "value" {
                    patch_prop_with_owner(&self.ctx, el, &k, &prev, &next, ns, parent)?;
                }
            }
            if rt.has_property(props_obj(rt, new_props)?, &PropKey::from_str("value"))? {
                let prev = rt.get_str(old_props, "value")?;
                let next = rt.get_str(new_props, "value")?;
                patch_prop_with_owner(&self.ctx, el, "value", &prev, &next, ns, parent)?;
            }
        }
        Ok(())
    }

    /// `patchChildren(n1, n2, container, anchor, ...)`: the children dispatch of the patch flags and shape flags.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn patch_children(
        &self,
        n1: Option<ObjRef>,
        n2: ObjRef,
        container: HostNode,
        anchor: Option<HostNode>,
        parent: Option<ObjRef>,
        ns: HostNs,
        ssi: &Value,
        optimized: bool,
    ) -> Res<()> {
        let rt = self.rt();
        let c1 = match n1 {
            Some(o) => children_of(rt, o),
            None => Value::Null,
        };
        let prev_shape = match n1 {
            Some(o) => shape_of(rt, o),
            None => 0,
        };
        let c2 = children_of(rt, n2);
        let patch = patch_of(rt, n2);
        let shape = shape_of(rt, n2);
        if patch > 0 {
            if patch & PatchFlags::KEYED_FRAGMENT != 0 {
                self.patch_keyed_children(&c1, &c2, container, anchor, parent, ns, ssi, optimized)?;
                return Ok(());
            } else if patch & PatchFlags::UNKEYED_FRAGMENT != 0 {
                self.patch_unkeyed_children(&c1, &c2, container, anchor, parent, ns, ssi, optimized)?;
                return Ok(());
            }
        }
        if shape & ShapeFlags::TEXT_CHILDREN != 0 {
            if prev_shape & ShapeFlags::ARRAY_CHILDREN != 0 {
                self.unmount_children(&c1, parent, true, false, 0)?;
            }
            if !strict_eq(&c2, &c1) {
                let s = self.to_s(&c2)?;
                self.set_element_text(container, &s)?;
            }
        } else if prev_shape & ShapeFlags::ARRAY_CHILDREN != 0 {
            if shape & ShapeFlags::ARRAY_CHILDREN != 0 {
                self.patch_keyed_children(&c1, &c2, container, anchor, parent, ns, ssi, optimized)?;
            } else {
                self.unmount_children(&c1, parent, true, false, 0)?;
            }
        } else {
            if prev_shape & ShapeFlags::TEXT_CHILDREN != 0 {
                self.set_element_text(container, "")?;
            }
            if shape & ShapeFlags::ARRAY_CHILDREN != 0 {
                self.mount_children(&c2, container, anchor, parent, ns, ssi, optimized, 0)?;
            }
        }
        Ok(())
    }

    // ----- fragments and components -----

    #[allow(clippy::too_many_arguments)]
    fn process_fragment(
        &self,
        n1: Option<ObjRef>,
        n2: ObjRef,
        container: HostNode,
        anchor: Option<HostNode>,
        parent: Option<ObjRef>,
        ns: HostNs,
        ssi: &Value,
        optimized: bool,
    ) -> Res<()> {
        let rt = self.rt();
        let (start, end) = match n1 {
            Some(o) => (self.need(el_of(rt, o), "fragment start")?, self.need(anchor_of(rt, o), "fragment end")?),
            None => (self.create_text(""), self.create_text("")),
        };
        set_el(rt, n2, Some(start));
        set_anchor(rt, n2, Some(end));
        let patch = patch_of(rt, n2);
        let dyn_children = dyn_children_of(rt, n2);
        let frag_ssi = fields(rt, n2, |f| f.slot_scope_ids.clone());
        let ssi_owned: Value;
        let ssi = if frag_ssi.to_boolean() {
            ssi_owned = self.concat_scope_ids(ssi, &frag_ssi)?;
            &ssi_owned
        } else {
            ssi
        };
        match n1 {
            None => {
                self.ins(start, container, anchor)?;
                self.ins(end, container, anchor)?;
                let children = children_of(rt, n2);
                let children = if children.to_boolean() { children } else { Value::Object(rt.new_array(Vec::new())) };
                self.mount_children(&children, container, Some(end), parent, ns, ssi, optimized, 0)?;
            }
            Some(old) => {
                let old_dyn = dyn_children_of(rt, old);
                let new_items = block_items(rt, &dyn_children);
                let old_items = block_items(rt, &old_dyn);
                let same_len = match (&new_items, &old_items) {
                    (Some(a), Some(b)) => a.len() == b.len(),
                    _ => false,
                };
                if patch > 0 && patch & PatchFlags::STABLE_FRAGMENT != 0 && same_len {
                    self.patch_block_children(&old_items.unwrap_or_default(), &new_items.unwrap_or_default(), container, parent, ns, ssi)?;
                    if !key_of(rt, n2).is_nullish() || parent.is_some_and(|p| is_subtree_of(rt, p, n2)) {
                        self.traverse_static_children(old, n2, true)?;
                    }
                } else {
                    let end = self.need(anchor_of(rt, n2), "fragment end")?;
                    self.patch_children(Some(old), n2, container, Some(end), parent, ns, ssi, optimized)?;
                }
            }
        }
        Ok(())
    }

    /// `traverseStaticChildren` (hoisted static children only; there are none, so only the element identity moves).
    pub(crate) fn traverse_static_children(&self, n1: ObjRef, n2: ObjRef, _shallow: bool) -> Res<()> {
        let rt = self.rt();
        let c1 = children_of(rt, n1);
        let c2 = children_of(rt, n2);
        let (Some(_), Some(_)) = (vnode::as_vnode(rt, &c1), vnode::as_vnode(rt, &c2)) else {
            return Ok(());
        };
        Ok(())
    }

    /// `[...slotScopeIds, ...fragmentSlotScopeIds]`, or the fragment's list when the parent has none.
    fn concat_scope_ids(&self, ssi: &Value, frag: &Value) -> Res<Value> {
        let rt = self.rt();
        if !ssi.to_boolean() {
            return Ok(frag.clone());
        }
        let mut items = Vec::new();
        for i in 0..array_len(rt, ssi)? {
            items.push(array_get(rt, ssi, i)?);
        }
        for i in 0..array_len(rt, frag)? {
            items.push(array_get(rt, frag, i)?);
        }
        Ok(Value::Object(rt.new_array(items)))
    }

    /// `unmountChildren(children, parent, doRemove, optimized, start)`.
    pub(crate) fn unmount_children(
        &self,
        children: &Value,
        parent: Option<ObjRef>,
        do_remove: bool,
        optimized: bool,
        start: usize,
    ) -> Res<()> {
        let rt = self.rt();
        let len = array_len(rt, children)?;
        for i in start..len {
            let child = vnode_at(rt, children, i)?;
            self.unmount(child, parent, do_remove, optimized)?;
        }
        Ok(())
    }

    /// `move(vnode, container, anchor, moveType)` (moveType 2 = reorder; 0 and 1 need Transition).
    pub(crate) fn move_vnode(&self, vnode: ObjRef, container: HostNode, anchor: Option<HostNode>, move_type: u8) -> Res<()> {
        let rt = self.rt();
        let shape = shape_of(rt, vnode);
        if shape & ShapeFlags::COMPONENT != 0 {
            let sub = component_subtree(rt, vnode)?;
            return self.move_vnode(sub, container, anchor, move_type);
        }
        if shape & ShapeFlags::SUSPENSE != 0 {
            return Err(rt.throw_type_error("unsupported: Suspense"));
        }
        if shape & ShapeFlags::TELEPORT != 0 {
            return self.move_teleport(vnode, container, anchor, 2);
        }
        let ty = ty_of(rt, vnode);
        if is_type(rt, &ty, "v-fgt") {
            let el = self.need(el_of(rt, vnode), "fragment start")?;
            self.ins(el, container, anchor)?;
            let children = children_of(rt, vnode);
            for i in 0..array_len(rt, &children)? {
                let child = vnode_at(rt, &children, i)?;
                self.move_vnode(child, container, anchor, move_type)?;
            }
            let end = self.need(anchor_of(rt, vnode), "fragment end")?;
            return self.ins(end, container, anchor);
        }
        if is_type(rt, &ty, "v-stc") {
            return Err(rt.throw_type_error("unsupported: Static (hoisted) vnodes"));
        }
        let el = self.need(el_of(rt, vnode), "move")?;
        let transition = transition_of(rt, vnode);
        if move_type != 2 && shape & ShapeFlags::ELEMENT != 0 && transition.to_boolean() {
            if move_type == 0 {
                crate::transition::invoke(rt, vnode, "beforeEnter", &[])?;
                self.ins(el, container, anchor)?;
                return crate::transition::queue_enter(rt, vnode);
            }
        }
        self.ins(el, container, anchor)
    }

    /// `unmount(vnode, parentComponent, doRemove, optimized)`.
    pub(crate) fn unmount(&self, vnode: ObjRef, parent: Option<ObjRef>, do_remove: bool, optimized: bool) -> Res<()> {
        let rt = self.rt();
        let mut optimized = optimized;
        let props = props_of(rt, vnode);
        let reference = ref_of(rt, vnode);
        let children = children_of(rt, vnode);
        let dyn_children = dyn_children_of(rt, vnode);
        let shape = shape_of(rt, vnode);
        let patch = patch_of(rt, vnode);
        let dirs = dirs_of(rt, vnode);
        let ty = ty_of(rt, vnode);
        let cache_index = cache_index_of(rt, vnode);
        let memo = memo_of_vnode(rt, vnode);
        if patch == PatchFlags::BAIL || block_has_once(rt, &dyn_children) {
            optimized = false;
        }
        if !reference.is_nullish() {
            pause_tracking(rt)?;
            let r = self.set_ref(&reference, None, vnode, true, parent);
            reset_tracking(rt)?;
            r?;
        }
        if !cache_index.is_nullish() {
            if let Some(p) = parent {
                let vctx = ctx_of_vnode(rt, vnode);
                if vctx.is_nullish() || strict_eq(&vctx, &Value::Object(p)) {
                    let inst = instance_of(rt, p);
                    let cache = inst.render_cache.borrow().clone();
                    let idx = num(&cache_index);
                    rt.set_v_strict(&cache, &PropKey::Index(idx as u32), Value::Undefined)?;
                }
            }
        }
        if shape & ShapeFlags::COMPONENT_SHOULD_KEEP_ALIVE != 0 {
            return self.deactivate_cached(vnode, self.need(parent, "KeepAlive parent")?);
        }
        let should_invoke_dirs = shape & ShapeFlags::ELEMENT != 0 && dirs.to_boolean();
        let before_unmount = if props.to_boolean() { rt.get_str(&props, "onVnodeBeforeUnmount")? } else { Value::Undefined };
        if before_unmount.to_boolean() {
            self.invoke_vnode_hook(&before_unmount, parent, vnode, None)?;
        }
        if shape & ShapeFlags::COMPONENT != 0 {
            self.unmount_component(component_of(rt, vnode), do_remove)?;
        } else {
            if shape & ShapeFlags::SUSPENSE != 0 {
                return Err(rt.throw_type_error("unsupported: Suspense"));
            }
            if should_invoke_dirs {
                self.invoke_directive_hook(vnode, None, parent, "beforeUnmount")?;
            }
            if shape & ShapeFlags::TELEPORT != 0 {
                self.remove_teleport(vnode, parent, do_remove)?;
            } else if let Some(items) = block_items(rt, &dyn_children).filter(|_| !block_has_once(rt, &dyn_children)) {
                if !is_type(rt, &ty, "v-fgt") || (patch > 0 && patch & PatchFlags::STABLE_FRAGMENT != 0) {
                    for child in items {
                        self.unmount(child, parent, false, true)?;
                    }
                } else {
                    self.after_block_check(&ty, patch, optimized, shape, &children, parent, do_remove)?;
                }
            } else {
                self.after_block_check(&ty, patch, optimized, shape, &children, parent, do_remove)?;
            }
            if do_remove && shape & ShapeFlags::TELEPORT == 0 {
                self.remove(vnode)?;
            }
        }
        let after_hook = if props.to_boolean() { rt.get_str(&props, "onVnodeUnmounted")? } else { Value::Undefined };
        let should_invalidate_memo = memo.to_boolean() && cache_index.is_nullish();
        if after_hook.to_boolean() || should_invoke_dirs || should_invalidate_memo {
            let mut steps = Vec::new();
            if after_hook.to_boolean() {
                steps.push(Step::Hook(after_hook, parent, vnode, None));
            }
            if should_invoke_dirs {
                steps.push(Step::Dir("unmounted", vnode, None, parent));
            }
            if should_invalidate_memo {
                steps.push(Step::ClearEl(vnode));
            }
            self.queue_steps(steps)?;
        }
        Ok(())
    }

    /// The `children` branch of `unmount` (Fragment with keyed or unkeyed patch flags, or an array without optimization).
    #[allow(clippy::too_many_arguments)]
    fn after_block_check(
        &self,
        ty: &Value,
        patch: i32,
        optimized: bool,
        shape: i32,
        children: &Value,
        parent: Option<ObjRef>,
        do_remove: bool,
    ) -> Res<()> {
        let _ = do_remove;
        if (is_type(self.rt(), ty, "v-fgt") && patch & (PatchFlags::KEYED_FRAGMENT | PatchFlags::UNKEYED_FRAGMENT) != 0)
            || (!optimized && shape & ShapeFlags::ARRAY_CHILDREN != 0)
        {
            self.unmount_children(children, parent, false, false, 0)?;
        }
        Ok(())
    }

    /// `remove(vnode)`: the host removal of a vnode's nodes (a fragment removes everything up to its end anchor).
    pub(crate) fn remove(&self, vnode: ObjRef) -> Res<()> {
        let rt = self.rt();
        let ty = ty_of(rt, vnode);
        let el = el_of(rt, vnode);
        let anchor = anchor_of(rt, vnode);
        if is_type(rt, &ty, "v-fgt") {
            let mut cur = self.need(el, "fragment start")?;
            let end = self.need(anchor, "fragment end")?;
            while cur != end {
                let next = self.next_of(cur);
                self.rm(cur)?;
                match next {
                    Some(n) => cur = n,
                    None => break,
                }
            }
            return self.rm(end);
        }
        if is_type(rt, &ty, "v-stc") {
            return Err(rt.throw_type_error("unsupported: Static (hoisted) vnodes"));
        }
        if transition_of(rt, vnode).to_boolean() {
            if !rt.get_str(&transition_of(rt, vnode), "persisted")?.to_boolean() {
                return crate::transition::remove(rt, vnode);
            }
        }
        let el = self.need(el, "remove")?;
        self.rm(el)
    }

    /// `render(vnode, container)`: patch or unmount, then flush the pre and post queues (once, not re-entrantly).
    pub fn render(&self, vnode: Option<ObjRef>, container: HostNode) -> Res<()> {
        let rt = self.rt();
        let prev = self.roots.borrow().get(&container).copied();
        let mut instance = None;
        match vnode {
            None => {
                if let Some(old) = prev {
                    self.unmount(old, None, true, false)?;
                    let c = component_of(rt, old);
                    if let Value::Object(i) = c {
                        instance = Some(i);
                    }
                }
            }
            Some(v) => {
                self.patch(prev, v, container, None, None, HostNs::Html, &Value::Null, false)?;
            }
        }
        match vnode {
            Some(v) => {
                self.roots.borrow_mut().insert(container, v);
            }
            None => {
                self.roots.borrow_mut().remove(&container);
            }
        }
        if !self.flushing.get() {
            self.flushing.set(true);
            crate::scheduler::flush_pre_flush_cbs(rt, instance, None)?;
            crate::scheduler::flush_post_flush_cbs(rt)?;
            self.flushing.set(false);
            self.ctx.flush_next_tick();
        }
        Ok(())
    }

    /// The host node of a rendered container's root vnode (tests and `$el`-like reads).
    pub fn root_vnode(&self, container: HostNode) -> Option<ObjRef> {
        self.roots.borrow().get(&container).copied()
    }
}

pub(crate) fn is_type(rt: &Rt, ty: &Value, name: &str) -> bool {
    strict_eq(ty, &sym(rt, name))
}

pub(crate) fn is_str(ty: &Value, name: &str) -> bool {
    matches!(ty, Value::String(s) if s.to_rust_string_lossy() == name)
}

fn ty_is_fragment(rt: &Rt, o: ObjRef) -> bool {
    is_type(rt, &ty_of(rt, o), "v-fgt")
}

/// The object form of a props value (`key in props` needs an object; props are objects when truthy).
fn props_obj(rt: &Rt, v: &Value) -> Res<ObjRef> {
    match v {
        Value::Object(o) => Ok(*o),
        _ => Err(rt.throw_type_error("renderer: props is not an object")),
    }
}

/// `instance.subTree` of a component vnode (its `component` field).
pub(crate) fn component_subtree(rt: &Rt, vnode: ObjRef) -> Res<ObjRef> {
    let c = component_of(rt, vnode);
    let Value::Object(i) = c else {
        return Err(rt.throw_type_error("renderer: component vnode without an instance (framework bug)"));
    };
    let sub = instance_of(rt, i).sub_tree.borrow().clone();
    match sub {
        Value::Object(o) => Ok(o),
        _ => Err(rt.throw_type_error("renderer: component subtree is not a vnode (framework bug)")),
    }
}

/// `parentComponent.subTree === vnode` for the given vnode (used by the fragment's static-children traversal).
fn is_subtree_of(rt: &Rt, parent: ObjRef, vnode: ObjRef) -> bool {
    let sub = instance_of(rt, parent).sub_tree.borrow().clone();
    strict_eq(&sub, &Value::Object(vnode))
}

/// `pauseTracking()` and `resetTracking()` of @vue/reactivity, around the hooks that run during a render.
pub(crate) fn pause_tracking(rt: &Rt) -> Res<()> {
    let st = inst_state(rt);
    rt.call(&st.pause_tracking, &Value::Undefined, &[]).map(|_| ())
}

pub(crate) fn reset_tracking(rt: &Rt) -> Res<()> {
    let st = inst_state(rt);
    rt.call(&st.reset_tracking, &Value::Undefined, &[]).map(|_| ())
}
