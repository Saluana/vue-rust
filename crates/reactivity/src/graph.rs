//! The dependency graph of @vue/reactivity 3.5.43: Dep (version, subscriber list, activeLink), Link (one edge from a
//! subscriber to a dep), subscriber bookkeeping (prepareDeps/cleanupDeps), dirtiness and the batching of notifications.
//! Behaviour per @vue/reactivity 3.5.43 (production build).

use std::cell::Cell;
use std::rc::Rc;

use vn_value::{ObjRef, Res, Rt, Throw, Tracer, Value};

use crate::computed::refresh_computed;
use crate::effect::effect_run;
use crate::node::{alloc, mark_opt, node, Node, NodeRef, ACTIVE, ALLOW_RECURSE, DIRTY, NOTIFIED, PAUSED, RUNNING, TRACKING};
use crate::shared::{hidden, truthy};
use crate::state::state;
use crate::target::{Key, TargetMap};

/// A reactive dependency: a version counter and the list of its subscribers (`subs` is the tail; links are doubly
/// linked through `prev_sub`/`next_sub`). `map` is the depsMap entry used for cleanup when the last subscriber leaves.
pub(crate) struct Dep {
    pub(crate) version: Cell<i64>,
    pub(crate) active_link: Cell<Option<ObjRef>>,
    pub(crate) subs: Cell<Option<ObjRef>>,
    pub(crate) computed: Cell<Option<ObjRef>>,
    pub(crate) sc: Cell<i64>,
    pub(crate) map: Option<(Rc<TargetMap>, Key)>,
}

impl Dep {
    pub(crate) fn trace(&self, t: &mut Tracer) {
        mark_opt(t, self.active_link.get());
        mark_opt(t, self.subs.get());
        mark_opt(t, self.computed.get());
    }
}

/// One dependency edge: the subscriber, the dep, the version seen by the subscriber, and the list links.
pub(crate) struct Link {
    pub(crate) sub: ObjRef,
    pub(crate) dep: ObjRef,
    pub(crate) version: Cell<i64>,
    pub(crate) prev_dep: Cell<Option<ObjRef>>,
    pub(crate) next_dep: Cell<Option<ObjRef>>,
    pub(crate) prev_sub: Cell<Option<ObjRef>>,
    pub(crate) next_sub: Cell<Option<ObjRef>>,
    pub(crate) prev_active_link: Cell<Option<ObjRef>>,
}

impl Link {
    pub(crate) fn trace(&self, t: &mut Tracer) {
        t.mark(self.sub);
        t.mark(self.dep);
        mark_opt(t, self.prev_dep.get());
        mark_opt(t, self.next_dep.get());
        mark_opt(t, self.prev_sub.get());
        mark_opt(t, self.next_sub.get());
        mark_opt(t, self.prev_active_link.get());
    }
}

/// The prototype of every dep. Vue's Dep class carries `readonly __v_skip = true` (packages/reactivity/src/dep.ts), so
/// reactive() and readonly() return a dep unchanged and `ref.dep` is never a proxy of one. One shared prototype holds the
/// flag, so a dep costs no property of its own.
pub(crate) fn install_dep_proto(rt: &Rt) -> ObjRef {
    let p = rt.new_object_with_proto(None);
    hidden(rt, p, "__v_skip", Value::Bool(true));
    p
}

/// A new dep. `map` is `Some` for a property dep of a target (its depsMap and key), `computed` for a computed's own dep.
pub(crate) fn new_dep(rt: &Rt, map: Option<(Rc<TargetMap>, Key)>, computed: Option<ObjRef>) -> ObjRef {
    let proto = state(rt).protos.get().dep;
    alloc(
        rt,
        proto,
        Node::Dep(Dep {
            version: Cell::new(0),
            active_link: Cell::new(None),
            subs: Cell::new(None),
            computed: Cell::new(computed),
            sc: Cell::new(0),
            map,
        }),
    )
}

fn new_link(rt: &Rt, sub: ObjRef, dep: ObjRef) -> ObjRef {
    let version = node(rt, dep).dep().version.get();
    alloc(
        rt,
        None,
        Node::Link(Link {
            sub,
            dep,
            version: Cell::new(version),
            prev_dep: Cell::new(None),
            next_dep: Cell::new(None),
            prev_sub: Cell::new(None),
            next_sub: Cell::new(None),
            prev_active_link: Cell::new(None),
        }),
    )
}

/// `addSub`: registers `link` as a subscriber of its dep, re-linking a computed's dependencies on its first subscriber.
pub(crate) fn add_sub(rt: &Rt, link: ObjRef) {
    let ln = node(rt, link);
    let l = ln.link();
    let dn = node(rt, l.dep);
    let d = dn.dep();
    d.sc.set(d.sc.get() + 1);
    if node(rt, l.sub).sub().flags.get() & TRACKING == 0 {
        return;
    }
    if let Some(comp) = d.computed.get() {
        if d.subs.get().is_none() {
            let cn = node(rt, comp);
            let cs = cn.sub();
            cs.flags.set(cs.flags.get() | TRACKING | DIRTY);
            let mut x = cs.deps.get();
            while let Some(lx) = x {
                add_sub(rt, lx);
                x = node(rt, lx).link().next_dep.get();
            }
        }
    }
    let current_tail = d.subs.get();
    if current_tail != Some(link) {
        l.prev_sub.set(current_tail);
        if let Some(t) = current_tail {
            node(rt, t).link().next_sub.set(Some(link));
        }
        let st = state(rt);
        st.live.subscriber_edges.set(st.live.subscriber_edges.get() + 1);
    }
    d.subs.set(Some(link));
}

/// `removeSub`: unlinks `link` from its dep's subscriber list. A soft removal keeps the dep count.
pub(crate) fn remove_sub(rt: &Rt, link: ObjRef, soft: bool) {
    let ln = node(rt, link);
    let l = ln.link();
    let dn = node(rt, l.dep);
    let d = dn.dep();
    let prev = l.prev_sub.get();
    let next = l.next_sub.get();
    // Only a link that is in the subscriber list counts as a dependency subscriber (a soft-removed one is not listed).
    if prev.is_some() || next.is_some() || d.subs.get() == Some(link) {
        let st = state(rt);
        st.live.subscriber_edges.set(st.live.subscriber_edges.get() - 1);
    }
    if let Some(p) = prev {
        node(rt, p).link().next_sub.set(next);
        l.prev_sub.set(None);
    }
    if let Some(n) = next {
        node(rt, n).link().prev_sub.set(prev);
        l.next_sub.set(None);
    }
    if d.subs.get() == Some(link) {
        d.subs.set(prev);
        if prev.is_none() {
            if let Some(comp) = d.computed.get() {
                let cn = node(rt, comp);
                let cs = cn.sub();
                cs.flags.set(cs.flags.get() & !TRACKING);
                let mut x = cs.deps.get();
                while let Some(lx) = x {
                    remove_sub(rt, lx, true);
                    x = node(rt, lx).link().next_dep.get();
                }
            }
        }
    }
    if !soft {
        let v = d.sc.get() - 1;
        d.sc.set(v);
        if v == 0 {
            if let Some((map, key)) = &d.map {
                map.remove(key);
            }
        }
    }
}

/// `removeDep`: unlinks `link` from its subscriber's dependency list (the caller fixes head and tail).
pub(crate) fn remove_dep(rt: &Rt, link: ObjRef) {
    let ln = node(rt, link);
    let l = ln.link();
    let prev = l.prev_dep.get();
    let next = l.next_dep.get();
    if let Some(p) = prev {
        node(rt, p).link().next_dep.set(next);
        l.prev_dep.set(None);
    }
    if let Some(n) = next {
        node(rt, n).link().prev_dep.set(prev);
        l.next_dep.set(None);
    }
}

/// `prepareDeps`: marks every current link stale (version -1) and makes it the active link of its dep.
pub(crate) fn prepare_deps(rt: &Rt, sub: ObjRef) {
    let mut link = node(rt, sub).sub().deps.get();
    while let Some(lo) = link {
        let ln = node(rt, lo);
        let l = ln.link();
        l.version.set(-1);
        let dn = node(rt, l.dep);
        let d = dn.dep();
        l.prev_active_link.set(d.active_link.get());
        d.active_link.set(Some(lo));
        link = l.next_dep.get();
    }
}

/// `cleanupDeps`: drops the links that were not read again in this run, and restores each dep's active link.
pub(crate) fn cleanup_deps(rt: &Rt, sub: ObjRef) {
    let sn = node(rt, sub);
    let s = sn.sub();
    let mut head = None;
    let mut tail = s.deps_tail.get();
    let mut link = tail;
    while let Some(lo) = link {
        let ln = node(rt, lo);
        let l = ln.link();
        let prev = l.prev_dep.get();
        if l.version.get() == -1 {
            if link == tail {
                tail = prev;
            }
            remove_sub(rt, lo, false);
            remove_dep(rt, lo);
        } else {
            head = Some(lo);
        }
        let dn = node(rt, l.dep);
        dn.dep().active_link.set(l.prev_active_link.get());
        l.prev_active_link.set(None);
        link = prev;
    }
    s.deps.set(head);
    s.deps_tail.set(tail);
}

/// `isDirty`: a subscriber is dirty when a dep changed version (refreshing computed deps first).
pub(crate) fn is_dirty(rt: &Rt, sub: ObjRef) -> Res<bool> {
    let mut link = node(rt, sub).sub().deps.get();
    while let Some(lo) = link {
        let ln = node(rt, lo);
        let l = ln.link();
        let dn = node(rt, l.dep);
        let d = dn.dep();
        if d.version.get() != l.version.get() {
            return Ok(true);
        }
        if let Some(comp) = d.computed.get() {
            refresh_computed(rt, comp)?;
            if d.version.get() != l.version.get() {
                return Ok(true);
            }
        }
        link = l.next_dep.get();
    }
    Ok(false)
}

/// `Dep.track`: records the active subscriber as a dependency. Returns its link, if any.
pub(crate) fn dep_track(rt: &Rt, dep: ObjRef) -> Option<ObjRef> {
    let st = state(rt);
    let active = st.active_sub.get()?;
    let dn = node(rt, dep);
    let d = dn.dep();
    if !st.should_track.get() || d.computed.get() == Some(active) {
        return None;
    }
    let existing = d.active_link.get();
    let link = match existing {
        Some(l) if node(rt, l).link().sub == active => l,
        _ => {
            let nl = new_link(rt, active, dep);
            d.active_link.set(Some(nl));
            let sn = node(rt, active);
            let s = sn.sub();
            match s.deps.get() {
                None => {
                    s.deps.set(Some(nl));
                    s.deps_tail.set(Some(nl));
                }
                Some(_) => {
                    let tail = s.deps_tail.get().expect("deps without a tail (framework bug)");
                    node(rt, nl).link().prev_dep.set(Some(tail));
                    node(rt, tail).link().next_dep.set(Some(nl));
                    s.deps_tail.set(Some(nl));
                }
            }
            add_sub(rt, nl);
            return Some(nl);
        }
    };
    let ln = node(rt, link);
    let l = ln.link();
    if l.version.get() == -1 {
        l.version.set(d.version.get());
        if let Some(next) = l.next_dep.get() {
            let prev = l.prev_dep.get();
            node(rt, next).link().prev_dep.set(prev);
            if let Some(p) = prev {
                node(rt, p).link().next_dep.set(Some(next));
            }
            let sn = node(rt, active);
            let s = sn.sub();
            let tail = s.deps_tail.get();
            l.prev_dep.set(tail);
            l.next_dep.set(None);
            if let Some(t) = tail {
                node(rt, t).link().next_dep.set(Some(link));
            }
            s.deps_tail.set(Some(link));
            if s.deps.get() == Some(link) {
                s.deps.set(Some(next));
            }
        }
    }
    Some(link)
}

/// `Dep.notify`: marks the subscribers (last subscriber first) and queues them in a batch.
pub(crate) fn dep_notify(rt: &Rt, dep: ObjRef) -> Res<()> {
    start_batch(rt);
    let mut result = Ok(());
    let dn = node(rt, dep);
    let d = dn.dep();
    let mut link = d.subs.get();
    while let Some(lo) = link {
        let ln = node(rt, lo);
        let l = ln.link();
        if notify_sub(rt, l.sub) {
            let cdep = node(rt, l.sub).computed().dep;
            if let Err(e) = dep_notify(rt, cdep) {
                result = Err(e);
                break;
            }
        }
        link = l.prev_sub.get();
    }
    match end_batch(rt) {
        Err(e) => Err(e),
        Ok(()) => result,
    }
}

/// `Dep.trigger`: a version change. Bumps the global version and notifies the subscribers.
pub(crate) fn dep_trigger(rt: &Rt, dep: ObjRef) -> Res<()> {
    {
        let dn = node(rt, dep);
        let d = dn.dep();
        d.version.set(d.version.get() + 1);
    }
    let st = state(rt);
    st.global_version.set(st.global_version.get() + 1);
    drop(st);
    dep_notify(rt, dep)
}

/// `notify()` of a subscriber. Returns true when a computed was queued and must notify its own subscribers.
fn notify_sub(rt: &Rt, sub: ObjRef) -> bool {
    let sn = node(rt, sub);
    let s = sn.sub();
    if sn.is_computed() {
        s.flags.set(s.flags.get() | DIRTY);
        if s.flags.get() & NOTIFIED == 0 && state(rt).active_sub.get() != Some(sub) {
            batch(rt, sub, true);
            return true;
        }
        return false;
    }
    let f = s.flags.get();
    if f & RUNNING != 0 && f & ALLOW_RECURSE == 0 {
        return false;
    }
    if f & NOTIFIED == 0 {
        batch(rt, sub, false);
    }
    false
}

/// `batch(sub, isComputed)`: pushes a subscriber on the LIFO batch list (computed on its own list).
fn batch(rt: &Rt, sub: ObjRef, is_computed: bool) {
    let st = state(rt);
    let sn = node(rt, sub);
    let s = sn.sub();
    s.flags.set(s.flags.get() | NOTIFIED);
    if is_computed {
        s.next.set(st.batched_computed.get());
        st.batched_computed.set(Some(sub));
    } else {
        s.next.set(st.batched_sub.get());
        st.batched_sub.set(Some(sub));
    }
}

pub(crate) fn start_batch(rt: &Rt) {
    let st = state(rt);
    st.batch_depth.set(st.batch_depth.get() + 1);
}

/// `endBatch`: at depth zero runs the queued computeds (only clearing NOTIFIED) and then the queued effects. Every
/// effect runs even if an earlier one throws; the first error is returned after the batch is drained.
pub(crate) fn end_batch(rt: &Rt) -> Res<()> {
    let st = state(rt);
    let depth = st.batch_depth.get().checked_sub(1).expect("unbalanced endBatch (framework bug)");
    st.batch_depth.set(depth);
    if depth > 0 {
        return Ok(());
    }
    let mut e = st.batched_computed.take();
    while let Some(cur) = e {
        let cn = node(rt, cur);
        let s = cn.sub();
        e = s.next.get();
        s.next.set(None);
        s.flags.set(s.flags.get() & !NOTIFIED);
    }
    let mut error: Option<Throw> = None;
    while let Some(head) = st.batched_sub.take() {
        let mut e = Some(head);
        while let Some(cur) = e {
            let cn = node(rt, cur);
            let s = cn.sub();
            e = s.next.get();
            s.next.set(None);
            s.flags.set(s.flags.get() & !NOTIFIED);
            if s.flags.get() & ACTIVE != 0 {
                if let Err(err) = trigger_effect(rt, cur) {
                    if error.is_none() {
                        error = Some(err);
                    }
                }
            }
        }
    }
    match error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

/// `ReactiveEffect.trigger`: queued while paused; otherwise the scheduler, or a re-run when dirty.
pub(crate) fn trigger_effect(rt: &Rt, e: ObjRef) -> Res<()> {
    let en: NodeRef = node(rt, e);
    let ef = en.effect();
    if ef.sub.flags.get() & PAUSED != 0 {
        ef.queued.set(true);
        return Ok(());
    }
    let sched = ef.scheduler.borrow().clone();
    if truthy(&sched) {
        rt.call(&sched, &Value::Object(e), &[])?;
        return Ok(());
    }
    run_if_dirty(rt, e)
}

/// `ReactiveEffect.runIfDirty`.
pub(crate) fn run_if_dirty(rt: &Rt, e: ObjRef) -> Res<()> {
    if is_dirty(rt, e)? {
        effect_run(rt, e)?;
    }
    Ok(())
}
