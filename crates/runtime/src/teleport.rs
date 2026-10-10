//! Teleport's local and target anchors, target changes, disabled moves and deferred mounts.
use vn_value::{Call, Flags, JsStr, PropKey, ObjRef, Res, Rt, Value};
use crate::dom_events::node_of_handle;
use crate::dom_props::include_boolean_attr;
use crate::host::{HostNode, HostNs};
use crate::renderer::{anchor_of, array_len, children_of, el_of, hid, hn, props_of, renderer, set_anchor, set_el, vnode_at, Renderer};
use crate::scheduler::queue_post_render_effect;
use crate::vnode::fields;

fn disabled(rt: &Rt, v: ObjRef) -> Res<bool> {
    Ok(include_boolean_attr(&rt.get_str(&props_of(rt, v), "disabled")?))
}

fn target(r: &Renderer, v: ObjRef) -> Res<HostNode> {
    let to = r.rt().get_str(&props_of(r.rt(), v), "to")?;
    let node = match &to {
        Value::String(s) => r.hres(r.host().query_selector(&s.to_rust_string_lossy()))?,
        _ => node_of_handle(r.rt(), &to),
    };
    r.need(node, "Teleport target")
}

fn deferred_mount(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let [vnode, parent, namespace, slot_scope_ids] = &*data else { unreachable!("Teleport mount captures") };
    let vnode = vnode.as_object().expect("Teleport vnode");
    if fields(rt, vnode, |f| f.teleport_unmounted) {
        return Ok(Value::Undefined);
    }
    let r = renderer(rt)?;
    let namespace = match namespace {
        Value::Number(1.0) => HostNs::Svg,
        Value::Number(2.0) => HostNs::MathMl,
        _ => HostNs::Html,
    };
    let container = r.need(el_of(rt, vnode).and_then(|n| r.parent_of(n)), "Teleport container")?;
    r.mount_teleport_target(vnode, container, parent.as_object(), namespace, slot_scope_ids)?;
    Ok(Value::Undefined)
}

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn process_teleport(&self, old: Option<ObjRef>, v: ObjRef, container: HostNode, anchor: Option<HostNode>, parent: Option<ObjRef>, ns: HostNs, ssi: &Value) -> Res<()> {
        let rt = self.rt();
        if let Some(old) = old {
            set_el(rt, v, el_of(rt, old));
            set_anchor(rt, v, anchor_of(rt, old));
            let (target_id, target_start, target_anchor, target_handles) = fields(rt, old, |f| {
                (f.target, f.target_start, f.target_anchor, f.target_handles.clone())
            });
            fields(rt, v, |f| {
                f.target = target_id;
                f.target_start = target_start;
                f.target_anchor = target_anchor;
                f.target_handles = target_handles;
            });
            let was_disabled = disabled(rt, old)?;
            let old_target = hn(target_id);
            let old_anchor = if was_disabled { anchor_of(rt, old) } else { hn(target_anchor) };
            let current = if was_disabled { container } else { self.need(old_target, "Teleport target")? };
            self.patch_children(Some(old), v, current, old_anchor, parent, ns, ssi, false)?;
            if disabled(rt, v)? {
                if !was_disabled { self.move_teleport(v, container, anchor_of(rt, v), 1)?; }
            } else {
                let next_target = target(self, v)?;
                if old_target != Some(next_target) {
                    let handles = [Some(next_target), hn(target_start), hn(target_anchor)].into_iter().flatten()
                        .filter_map(|n| self.ctx.handle(n).as_object()).collect();
                    fields(rt, v, |f| { f.target = hid(Some(next_target)); f.target_handles = handles; });
                    if let Some(start) = hn(target_start) { self.ins(start, next_target, None)?; }
                    self.move_teleport(v, next_target, None, 0)?;
                } else if was_disabled {
                    self.move_teleport(v, next_target, hn(target_anchor), 1)?;
                }
            }
        } else {
            let start = self.create_text("");
            let end = self.create_text("");
            set_el(rt, v, Some(start));
            set_anchor(rt, v, Some(end));
            self.ins(start, container, anchor)?;
            self.ins(end, container, anchor)?;
            if include_boolean_attr(&rt.get_str(&props_of(rt, v), "defer")?) {
                let ns = match ns { HostNs::Html => 0.0, HostNs::Svg => 1.0, HostNs::MathMl => 2.0 };
                let f = Value::Object(rt.new_native_function(deferred_mount, "mountTeleport", 0,
                    vec![Value::Object(v), parent.map(Value::Object).unwrap_or(Value::Null), Value::Number(ns), ssi.clone()]));
                queue_post_render_effect(rt, &f, &Value::Null)?;
            } else { self.mount_teleport_target(v, container, parent, ns, ssi)?; }
        }
        Ok(())
    }
    fn mount_teleport_target(&self, v: ObjRef, container: HostNode, parent: Option<ObjRef>, ns: HostNs, ssi: &Value) -> Res<()> {
        let rt = self.rt();
        let target = target(self, v)?;
        let start = self.create_text("");
        let end = self.create_text("");
        self.ins(start, target, None)?;
        self.ins(end, target, None)?;
        let handles = [target, start, end].iter().filter_map(|n| self.ctx.handle(*n).as_object()).collect();
        fields(rt, v, |f| {
            f.target = hid(Some(target));
            f.target_start = hid(Some(start));
            f.target_anchor = hid(Some(end));
            f.target_handles = handles;
        });
        let off = disabled(rt, v)?;
        self.mount_children(&children_of(rt, v), if off { container } else { target },
            if off { anchor_of(rt, v) } else { Some(end) }, parent, ns, ssi, false, 0)
    }
    pub(crate) fn move_teleport(&self, v: ObjRef, container: HostNode, anchor: Option<HostNode>, kind: u8) -> Res<()> {
        let rt = self.rt();
        if kind == 0 {
            if let Some(end) = fields(rt, v, |f| hn(f.target_anchor)) { self.ins(end, container, anchor)?; }
        }
        if kind == 2 { self.ins(self.need(el_of(rt, v), "Teleport start")?, container, anchor)?; }
        if kind != 2 || disabled(rt, v)? {
            let children = children_of(rt, v);
            for i in 0..array_len(rt, &children)? { self.move_vnode(vnode_at(rt, &children, i)?, container, anchor, 2)?; }
        }
        if kind == 2 { self.ins(self.need(anchor_of(rt, v), "Teleport end")?, container, anchor)?; }
        Ok(())
    }
    pub(crate) fn remove_teleport(&self, v: ObjRef, parent: Option<ObjRef>, do_remove: bool) -> Res<()> {
        let rt = self.rt();
        fields(rt, v, |f| f.teleport_unmounted = true);
        for n in fields(rt, v, |f| [hn(f.target_start), hn(f.target_anchor)]).into_iter().flatten() { self.rm(n)?; }
        if do_remove {
            for n in [el_of(rt, v), anchor_of(rt, v)].into_iter().flatten() { self.rm(n)?; }
        }
        self.unmount_children(&children_of(rt, v), parent, do_remove || !disabled(rt, v)?, false, 0)
    }
}

pub(crate) fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    let o = rt.new_object();
    rt.define_data(o, PropKey::lit("__isTeleport"), Value::Bool(true), Flags::ALL).expect("fresh Teleport");
    rt.define_data(o, PropKey::lit("name"), Value::String(JsStr::from_str("Teleport")), Flags::ALL).expect("fresh Teleport");
    vec![("Teleport", Value::Object(o))]
}
