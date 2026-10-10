//! vn-runtime::hydrate: `createHydrationFunctions` of @vue/runtime-core 3.5.43 (production build) over the DOM host.
//! `hydrate` adopts the markup already in a container: `hydrate_node` binds one vnode to one host node (text, comment,
//! fragment, element, component), `hydrate_element` attaches props and listeners, `hydrate_children` walks the child
//! nodes (splitting adjacent text as hydrateChildren does), `hydrate_fragment` expects the "[" ... "]" comment pair,
//! `handle_mismatch` removes a node that does not fit and mounts the vnode in its place, and `locate_closing_anchor`
//! finds the matching "]". The DOM is read through `Host::first_child`, `next_sibling`, `parent_node`, `node_type` and
//! `node_value`. Each recovered mismatch is also written to the realm's log (`hydration_mismatches`); runtime-core keeps
//! no such log (production logs one console error).
//! Not in the subset (TypeError, never a silent approximation): Teleport, Suspense, Static (hoisted) vnodes, Transition,
//! KeepAlive, async components, scoped slot ids, and any host that does not implement `first_child`/`node_type`/`node_value`.
//! Not ported, and silent in this port: the template-node exemptions of runtime-core (`isTemplateNode`, prod:1466-1470 and
//! 1517: DocHost has no template content), `data-allow-mismatch`, and the console error of production (the log is kept).

use vn_value::{ObjRef, Res, Rt, Value};

use crate::dom_props::{is_on, patch_prop_with_owner};
use crate::host::{HostNode, HostNs};
use crate::instance::instance_of;
use crate::normalize::{array_get, for_in_keys};
use crate::renderer::{
    array_len, children_of, dirs_of, dyn_children_of, dyn_props_of, el_of, is_reserved_prop, is_type, key_string, patch_of,
    props_of, ref_of, set_anchor, set_el, shape_of, slot_scope_ids_of, ty_of, vnode_at, Renderer, Step,
};
use crate::shapes::{PatchFlags, ShapeFlags};
use crate::vnode::fields;

const DOM_ELEMENT: u8 = 1;
const DOM_TEXT: u8 = 3;
const DOM_COMMENT: u8 = 8;

/// `text.slice(from)` in UTF-16 code units, as hydrateChildren splits a text node.
fn utf16_tail(s: &str, from: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();
    String::from_utf16_lossy(&units[from.min(units.len())..])
}

fn truthy_prop(rt: &Rt, props: &Value, key: &str) -> Res<bool> {
    Ok(props.to_boolean() && rt.get_str(props, key)?.to_boolean())
}

/// `isReactive` of @vue/reactivity: a readonly proxy answers for its raw target, otherwise `__v_isReactive` decides. The
/// reactivity crate keeps its helper crate-private, so the same protocol is read here.
fn is_reactive_value(rt: &Rt, v: &Value) -> Res<bool> {
    if !matches!(v, Value::Object(_)) {
        return Ok(false);
    }
    if rt.get_str(v, "__v_isReadonly")?.to_boolean() {
        let raw = rt.get_str(v, "__v_raw")?;
        return is_reactive_value(rt, &raw);
    }
    Ok(rt.get_str(v, "__v_isReactive")?.to_boolean())
}

impl Renderer {
    /// `hydrate(vnode, container)`: adopts the markup of `container` for `vnode`, mounting it when the container is empty.
    pub(crate) fn hydrate(&self, vnode: ObjRef, container: HostNode) -> Res<()> {
        let rt = self.rt();
        let first = self.hres(self.host().first_child(container))?;
        match first {
            None => self.patch(None, vnode, container, None, None, HostNs::Html, &Value::Null, false)?,
            Some(node) => {
                let prev = self.hydrating.replace(true);
                let r = self.hydrate_node(node, vnode, None, &Value::Null, HostNs::Html, false);
                self.hydrating.set(prev);
                r?;
            }
        }
        crate::scheduler::flush_post_flush_cbs(rt)?;
        self.roots.borrow_mut().insert(container, vnode);
        Ok(())
    }

    /// `hydrateNode(node, vnode, parentComponent, parentSuspense, slotScopeIds, optimized)`: binds `vnode` to `node` and
    /// returns the DOM node after the one that was adopted (or recovered).
    pub(crate) fn hydrate_node(
        &self,
        node: HostNode,
        vnode: ObjRef,
        parent: Option<ObjRef>,
        ssi: &Value,
        ns: HostNs,
        optimized: bool,
    ) -> Res<Option<HostNode>> {
        let rt = self.rt();
        let mut optimized = optimized || dyn_children_of(rt, vnode).to_boolean();
        let dom_type = self.dom_type(node)?;
        let is_fragment_start = dom_type == DOM_COMMENT && self.node_text(node)? == "[";
        set_el(rt, vnode, Some(node));
        if patch_of(rt, vnode) == PatchFlags::BAIL {
            optimized = false;
            fields(rt, vnode, |f| f.dynamic_children = Value::Null);
        }
        let ty = ty_of(rt, vnode);
        let shape = shape_of(rt, vnode);
        let reference = ref_of(rt, vnode);
        let next: Option<HostNode> = if is_type(rt, &ty, "v-txt") {
            if dom_type != DOM_TEXT {
                if matches!(children_of(rt, vnode), Value::String(ref t) if t.is_empty()) {
                    // `vnode.children === ""` (empty text): a fresh empty text node in front of the mismatched node
                    let t = self.create_text("");
                    set_el(rt, vnode, Some(t));
                    let container = self.need(self.parent_of(node), "text container")?;
                    self.ins(t, container, Some(node))?;
                    Some(node)
                } else {
                    self.handle_mismatch(node, vnode, parent, ssi, ns, false, optimized)?
                }
            } else {
                let text = self.node_text(node)?;
                let expected = self.to_s(&children_of(rt, vnode))?;
                if text != expected {
                    self.log_mismatch(format!("text: server {text:?}, client {expected:?}"));
                    self.set_text(node, &expected)?;
                }
                self.next_of(node)
            }
        } else if is_type(rt, &ty, "v-cmt") {
            // Comment text is not compared (runtime-core does not compare it either).
            if dom_type != DOM_COMMENT || is_fragment_start {
                self.handle_mismatch(node, vnode, parent, ssi, ns, false, optimized)?
            } else {
                self.next_of(node)
            }
        } else if is_type(rt, &ty, "v-stc") {
            return Err(rt.throw_type_error("unsupported: Static (hoisted) vnode in hydration"));
        } else if is_type(rt, &ty, "v-fgt") {
            if !is_fragment_start {
                self.handle_mismatch(node, vnode, parent, ssi, ns, false, optimized)?
            } else {
                self.hydrate_fragment(node, vnode, parent, ssi, ns, optimized)?
            }
        } else if shape & ShapeFlags::ELEMENT != 0 {
            let tag = self.to_s(&ty)?;
            let tag_ok = dom_type == DOM_ELEMENT && tag.to_ascii_lowercase() == self.host().tag_name(node).to_ascii_lowercase();
            if !tag_ok {
                self.handle_mismatch(node, vnode, parent, ssi, ns, false, optimized)?
            } else {
                self.hydrate_element(node, vnode, parent, ssi, ns, optimized)?
            }
        } else if shape & ShapeFlags::COMPONENT != 0 {
            self.reject_builtin(vnode, "hydration")?;
            if slot_scope_ids_of(rt, vnode).to_boolean() {
                return Err(rt.throw_type_error("unsupported: slot scope ids in hydration"));
            }
            let container = self.need(self.parent_of(node), "component container")?;
            let after = if is_fragment_start { self.locate_closing_anchor(node)? } else { self.next_of(node) };
            self.mount_component(vnode, container, None, parent, ns, optimized)?;
            after
        } else if shape & ShapeFlags::TELEPORT != 0 {
            return Err(rt.throw_type_error("unsupported: Teleport in hydration"));
        } else if shape & ShapeFlags::SUSPENSE != 0 {
            return Err(rt.throw_type_error("unsupported: Suspense in hydration"));
        } else {
            return Err(rt.throw_type_error("unsupported: vnode type in hydration"));
        };
        if reference.to_boolean() {
            self.set_ref(&reference, None, vnode, false, parent)?;
        }
        Ok(next)
    }

    /// `hydrateElement(el, vnode, ...)`: children, text content, props and listeners of an adopted element.
    fn hydrate_element(
        &self,
        el: HostNode,
        vnode: ObjRef,
        parent: Option<ObjRef>,
        ssi: &Value,
        ns: HostNs,
        optimized: bool,
    ) -> Res<Option<HostNode>> {
        let rt = self.rt();
        let optimized = optimized || dyn_children_of(rt, vnode).to_boolean();
        let props = props_of(rt, vnode);
        let patch = patch_of(rt, vnode);
        let shape = shape_of(rt, vnode);
        let dirs = dirs_of(rt, vnode);
        let ty = self.to_s(&ty_of(rt, vnode))?;
        let force_patch = ty == "input" || ty == "option";
        // `if (forcePatch || hasDynamicProps || patchFlag !== -1)` in runtime-core's hydrateElement: a cached (hoisted static)
        // element with no dynamic props is adopted as it is; its children, props and hooks are not compared or run.
        if !(force_patch || dyn_props_of(rt, vnode).to_boolean() || patch != PatchFlags::CACHED) {
            return Ok(self.next_of(el));
        }
        if dirs.to_boolean() {
            self.invoke_directive_hook(vnode, None, parent, "created")?;
        }
        let overridden = truthy_prop(rt, &props, "innerHTML")? || truthy_prop(rt, &props, "textContent")?;
        if shape & ShapeFlags::ARRAY_CHILDREN != 0 && !overridden {
            let first = self.hres(self.host().first_child(el))?;
            let mut next = self.hydrate_children(first, vnode, el, parent, ssi, ns, optimized)?;
            // server nodes after the last client child are extra markup: logged and removed, as runtime-core does
            if next.is_some() {
                self.log_mismatch("children: the server markup has nodes after the last client child".to_string());
            }
            while let Some(cur) = next {
                next = self.next_of(cur);
                self.rm(cur)?;
            }
        } else if shape & ShapeFlags::TEXT_CHILDREN != 0 {
            let children = children_of(rt, vnode);
            let mut client = self.to_s(&children)?;
            if (ty == "pre" || ty == "textarea") && client.starts_with('\n') {
                client.remove(0);
            }
            let text = self.text_content_of(el)?;
            if text != client && text != client.replace("\r\n", "\n").replace('\r', "\n") {
                self.log_mismatch(format!("text content of <{ty}>: server {text:?}, client {client:?}"));
                let full = self.to_s(&children)?;
                self.set_element_text(el, &full)?;
            }
        }
        if props.to_boolean() {
            let is_custom = ty.contains('-');
            let dyn_props = dyn_props_of(rt, vnode);
            let has_dyn = dyn_props.to_boolean();
            if force_patch || has_dyn || !optimized || patch & (PatchFlags::FULL_PROPS | PatchFlags::NEED_HYDRATION) != 0 {
                let dyn_names = if has_dyn { self.string_list(&dyn_props)? } else { Vec::new() };
                for key in for_in_keys(rt, &props)? {
                    let k = key_string(&key);
                    let wanted = (force_patch && (k.ends_with("value") || k == "indeterminate"))
                        || (is_on(&k) && !is_reserved_prop(&k))
                        || k.starts_with('.')
                        || (is_custom && !is_reserved_prop(&k))
                        || dyn_names.contains(&k);
                    if !wanted {
                        continue;
                    }
                    let v = rt.get_v(&props, &key)?;
                    if self.unchanged_resource_prop(el, &k, &v)? {
                        continue;
                    }
                    patch_prop_with_owner(&self.ctx, el, &k, &Value::Null, &v, ns, parent)?;
                }
            } else {
                let on_click = rt.get_str(&props, "onClick")?;
                if on_click.to_boolean() {
                    patch_prop_with_owner(&self.ctx, el, "onClick", &Value::Null, &on_click, ns, parent)?;
                } else if patch & PatchFlags::STYLE != 0 {
                    // `patchFlag & 4 && isReactive(props.style)`: every key of a reactive style is read, so the render effect
                    // depends on them as it does after a patch (runtime-core 3.5.43, prod:1683-1684)
                    let style = rt.get_str(&props, "style")?;
                    if is_reactive_value(rt, &style)? {
                        for key in for_in_keys(rt, &style)? {
                            rt.get_v(&style, &key)?;
                        }
                    }
                }
            }
        }
        let before = if props.to_boolean() { rt.get_str(&props, "onVnodeBeforeMount")? } else { Value::Null };
        if before.to_boolean() {
            self.invoke_vnode_hook(&before, parent, vnode, None)?;
        }
        if dirs.to_boolean() {
            self.invoke_directive_hook(vnode, None, parent, "beforeMount")?;
        }
        let mounted_hook = if props.to_boolean() { rt.get_str(&props, "onVnodeMounted")? } else { Value::Null };
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
        Ok(self.next_of(el))
    }

    /// `hydrateChildren(node, parentVNode, container, ...)`: returns the DOM node left after the children.
    #[allow(clippy::too_many_arguments)]
    fn hydrate_children(
        &self,
        node: Option<HostNode>,
        parent_vnode: ObjRef,
        container: HostNode,
        parent: Option<ObjRef>,
        ssi: &Value,
        ns: HostNs,
        optimized: bool,
    ) -> Res<Option<HostNode>> {
        let rt = self.rt();
        let optimized = optimized || dyn_children_of(rt, parent_vnode).to_boolean();
        let children = children_of(rt, parent_vnode);
        let len = array_len(rt, &children)?;
        let mut node = node;
        let mut checked = false;
        for i in 0..len {
            let vnode = if optimized { vnode_at(rt, &children, i)? } else { self.norm_at(&children, i, false)? };
            let is_text = is_type(rt, &ty_of(rt, vnode), "v-txt");
            match node {
                Some(n) => {
                    if is_text && !optimized && i + 1 < len {
                        let raw_next = array_get(rt, &children, i + 1)?;
                        let next_vnode = self.norm_value(&raw_next, false)?;
                        if is_type(rt, &ty_of(rt, next_vnode), "v-txt") {
                            // two client text nodes, one server text node: split it at the first one's length
                            let data = self.node_text(n)?;
                            let mine = self.to_s(&children_of(rt, vnode))?;
                            let rest = utf16_tail(&data, mine.encode_utf16().count());
                            let t = self.create_text(&rest);
                            let after = self.next_of(n);
                            self.ins(t, container, after)?;
                            self.set_text(n, &mine)?;
                        }
                    }
                    node = self.hydrate_node(n, vnode, parent, ssi, ns, optimized)?;
                }
                None => {
                    if is_text && !children_of(rt, vnode).to_boolean() {
                        let t = self.create_text("");
                        set_el(rt, vnode, Some(t));
                        self.ins(t, container, None)?;
                    } else {
                        if !checked {
                            checked = true;
                            self.log_mismatch("children: the client expects more nodes than the server rendered".to_string());
                        }
                        self.patch(None, vnode, container, None, parent, ns, ssi, optimized)?;
                    }
                }
            }
        }
        Ok(node)
    }

    /// `hydrateFragment(node, vnode, ...)`: the children up to the "]" comment, which becomes `vnode.anchor`.
    fn hydrate_fragment(
        &self,
        node: HostNode,
        vnode: ObjRef,
        parent: Option<ObjRef>,
        ssi: &Value,
        ns: HostNs,
        optimized: bool,
    ) -> Res<Option<HostNode>> {
        let rt = self.rt();
        if slot_scope_ids_of(rt, vnode).to_boolean() {
            return Err(rt.throw_type_error("unsupported: slot scope ids in hydration"));
        }
        let container = self.need(self.parent_of(node), "fragment container")?;
        let first = self.next_of(node);
        let next = self.hydrate_children(first, vnode, container, parent, ssi, ns, optimized)?;
        if let Some(n) = next {
            if self.dom_type(n)? == DOM_COMMENT && self.node_text(n)? == "]" {
                set_anchor(rt, vnode, Some(n));
                return Ok(self.next_of(n));
            }
        }
        self.log_mismatch("fragment: the server markup has no \"]\" anchor".to_string());
        let close = self.create_comment("]");
        set_anchor(rt, vnode, Some(close));
        self.ins(close, container, next)?;
        Ok(next)
    }

    /// `handleMismatch(node, vnode, ...)`: removes the node (and the fragment content up to "]") and mounts `vnode` in
    /// its place, before the node that followed it. The vnode gets a new host node; the old one is gone.
    #[allow(clippy::too_many_arguments)]
    fn handle_mismatch(
        &self,
        node: HostNode,
        vnode: ObjRef,
        parent: Option<ObjRef>,
        ssi: &Value,
        ns: HostNs,
        is_fragment: bool,
        optimized: bool,
    ) -> Res<Option<HostNode>> {
        let rt = self.rt();
        let server = self.describe_node(node)?;
        let client = self.describe_vnode(vnode)?;
        self.log_mismatch(format!("node: server {server}, client {client}"));
        set_el(rt, vnode, None);
        if is_fragment {
            let end = self.locate_closing_anchor(node)?;
            loop {
                let after = self.next_of(node);
                match after {
                    Some(n) if Some(n) != end => self.rm(n)?,
                    _ => break,
                }
            }
        }
        let next = self.next_of(node);
        let container = self.need(self.parent_of(node), "mismatch container")?;
        self.rm(node)?;
        self.patch(None, vnode, container, next, parent, ns, ssi, optimized)?;
        if let Some(p) = parent {
            let el = el_of(rt, vnode);
            let inst_vnode = instance_of(rt, p).vnode.borrow().clone();
            if let Value::Object(v) = inst_vnode {
                set_el(rt, v, el);
            }
            self.update_hoc_host_el(p, el)?;
        }
        Ok(next)
    }

    /// `locateClosingAnchor(node, "[", "]")`: the node after the "]" that matches the fragment start, or None.
    fn locate_closing_anchor(&self, node: HostNode) -> Res<Option<HostNode>> {
        let mut depth = 0usize;
        let mut cur = Some(node);
        loop {
            let Some(c) = cur else { return Ok(None) };
            cur = self.next_of(c);
            let Some(n) = cur else { return Ok(None) };
            if self.dom_type(n)? == DOM_COMMENT {
                let data = self.node_text(n)?;
                if data == "[" {
                    depth += 1;
                } else if data == "]" {
                    if depth == 0 {
                        return Ok(self.next_of(n));
                    }
                    depth -= 1;
                }
            }
        }
    }

    /// Components outside the subset (Transition, KeepAlive and async components): a TypeError that names `phase`
    /// ("hydration" or "SSR"), so an unsupported tree fails loudly instead of being approximated.
    pub(crate) fn reject_builtin(&self, vnode: ObjRef, phase: &str) -> Res<()> {
        let rt = self.rt();
        let ty = ty_of(rt, vnode);
        if !matches!(ty, Value::Object(_)) {
            return Ok(());
        }
        if rt.get_str(&ty, "__isKeepAlive")?.to_boolean() {
            return Err(rt.throw_type_error(&format!("unsupported: KeepAlive in {phase}")));
        }
        let name = rt.get_str(&ty, "name")?;
        if name.to_boolean() {
            let n = self.to_s(&name)?;
            if n == "Transition" || n == "BaseTransition" {
                return Err(rt.throw_type_error(&format!("unsupported: Transition in {phase}")));
            }
        }
        if rt.get_str(&ty, "__asyncLoader")?.to_boolean() {
            return Err(rt.throw_type_error(&format!("unsupported: async component in {phase}")));
        }
        Ok(())
    }

    /// `props.value` style checks for resource attributes: `isUnchangedResourceProp`.
    fn unchanged_resource_prop(&self, el: HostNode, key: &str, client: &Value) -> Res<bool> {
        if !matches!(key, "src" | "srcset" | "href" | "poster") {
            return Ok(false);
        }
        let current = self.host().get_attribute(el, key);
        let expected = if client.is_nullish() { None } else { Some(self.to_s(client)?) };
        Ok(current == expected)
    }

    /// `textContent` of an element: the text nodes below it, in document order.
    fn text_content_of(&self, el: HostNode) -> Res<String> {
        let mut out = String::new();
        let mut cur = self.hres(self.host().first_child(el))?;
        while let Some(n) = cur {
            match self.dom_type(n)? {
                DOM_TEXT => out.push_str(&self.node_text(n)?),
                DOM_ELEMENT => out.push_str(&self.text_content_of(n)?),
                _ => {}
            }
            cur = self.next_of(n);
        }
        Ok(out)
    }

    fn string_list(&self, arr: &Value) -> Res<Vec<String>> {
        let rt = self.rt();
        let n = array_len(rt, arr)?;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            out.push(self.to_s(&array_get(rt, arr, i)?)?);
        }
        Ok(out)
    }

    fn dom_type(&self, n: HostNode) -> Res<u8> {
        self.hres(self.host().node_type(n))
    }

    fn node_text(&self, n: HostNode) -> Res<String> {
        self.hres(self.host().node_value(n))
    }

    fn describe_node(&self, n: HostNode) -> Res<String> {
        Ok(match self.dom_type(n)? {
            DOM_TEXT => "text".to_string(),
            DOM_COMMENT => format!("comment {:?}", self.node_text(n)?),
            _ => format!("<{}>", self.host().tag_name(n).to_ascii_lowercase()),
        })
    }

    fn describe_vnode(&self, vnode: ObjRef) -> Res<String> {
        let rt = self.rt();
        let ty = ty_of(rt, vnode);
        Ok(if is_type(rt, &ty, "v-txt") {
            "text".to_string()
        } else if is_type(rt, &ty, "v-cmt") {
            "comment".to_string()
        } else if is_type(rt, &ty, "v-fgt") {
            "fragment".to_string()
        } else if shape_of(rt, vnode) & ShapeFlags::ELEMENT != 0 {
            format!("<{}>", self.to_s(&ty)?)
        } else {
            "component".to_string()
        })
    }

    fn log_mismatch(&self, line: String) {
        self.mismatches.borrow_mut().push(line);
    }
}
