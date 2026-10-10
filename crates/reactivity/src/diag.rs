//! Diagnostics for tests and tooling. The runtime never reads these. The counts describe the objects that still exist:
//! an object that nothing references counts until the collector frees it, so call `Rt::collect()` before reading them.

use vn_value::{ObjRef, Rt};

use crate::node::{node, try_node};
use crate::state::state;

/// Live reactivity objects of one realm (see the module comment).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReactivityStats {
    /// ReactiveEffect objects: render effects, watcher effects and `effect()` runners.
    pub effects: usize,
    /// ComputedRefImpl objects.
    pub computeds: usize,
    /// EffectScope objects.
    pub scopes: usize,
    /// Watcher objects (`watch` base); each owns one effect.
    pub watchers: usize,
    /// Dep objects: one per tracked property key of a target, per ref, and per computed.
    pub deps: usize,
    /// Link objects: one per (subscriber, dep) edge still referenced by a live subscriber or dep.
    pub links: usize,
    /// Links in the subscriber list of a dep, summed over the realm: the dependency-subscriber count.
    pub subscriber_edges: usize,
}

/// The live reactivity counts of `rt`.
pub fn debug_stats(rt: &Rt) -> ReactivityStats {
    let st = state(rt);
    let l = &st.live;
    ReactivityStats {
        effects: l.effects.get(),
        computeds: l.computeds.get(),
        scopes: l.scopes.get(),
        watchers: l.watchers.get(),
        deps: l.deps.get(),
        links: l.links.get(),
        subscriber_edges: l.subscriber_edges.get(),
    }
}

/// The number of subscribers of `dep` (a dep object, such as a ref's `dep`), counted along its subscriber list. None
/// when `dep` is not a dep.
pub fn dep_subscriber_count(rt: &Rt, dep: ObjRef) -> Option<usize> {
    let dn = try_node(rt, dep)?;
    if !dn.is_dep() {
        return None;
    }
    let mut count = 0;
    let mut link = dn.dep().subs.get();
    while let Some(l) = link {
        count += 1;
        link = node(rt, l).link().prev_sub.get();
    }
    Some(count)
}
