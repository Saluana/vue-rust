//! Heap payloads of the reactivity graph. Each Rust-side node is a heap object whose payload is `Node`, so the
//! collector traces every ObjRef it holds and frees unreachable graphs (ABI.md section 11). Mutable fields are `Cell`
//! or `RefCell`; no borrow is held across a call into user code.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;

use vn_value::{payload_ref, NativeObject, ObjRef, Res, Rt, Tracer, Value};

use crate::computed::Computed;
use crate::effect::Effect;
use crate::graph::{Dep, Link};
use crate::refs::{CustomRef, GetterRef, PropRef, RefImpl};
use crate::scope::Scope;
use crate::state::state;
use crate::watch::Watch;

// Subscriber flags, as in @vue/reactivity 3.5.43 (EffectFlags).
pub(crate) const ACTIVE: u32 = 1;
pub(crate) const RUNNING: u32 = 2;
pub(crate) const TRACKING: u32 = 4;
pub(crate) const NOTIFIED: u32 = 8;
pub(crate) const DIRTY: u32 = 16;
pub(crate) const ALLOW_RECURSE: u32 = 32;
pub(crate) const PAUSED: u32 = 64;
pub(crate) const EVALUATED: u32 = 128;

/// Fields shared by every subscriber (ReactiveEffect and ComputedRefImpl).
pub(crate) struct SubFields {
    pub(crate) flags: Cell<u32>,
    pub(crate) deps: Cell<Option<ObjRef>>,
    pub(crate) deps_tail: Cell<Option<ObjRef>>,
    pub(crate) next: Cell<Option<ObjRef>>,
}

impl SubFields {
    pub(crate) fn new(flags: u32) -> SubFields {
        SubFields { flags: Cell::new(flags), deps: Cell::new(None), deps_tail: Cell::new(None), next: Cell::new(None) }
    }

    pub(crate) fn trace(&self, t: &mut Tracer) {
        mark_opt(t, self.deps.get());
        mark_opt(t, self.deps_tail.get());
        mark_opt(t, self.next.get());
    }
}

pub(crate) fn mark_opt(t: &mut Tracer, o: Option<ObjRef>) {
    if let Some(o) = o {
        t.mark(o);
    }
}

/// The payload of every reactivity heap object (dep, link, effect, computed, scope, refs).
pub(crate) enum Node {
    Dep(Dep),
    Link(Link),
    Effect(Effect),
    Computed(Computed),
    Scope(Scope),
    Ref(RefImpl),
    Custom(CustomRef),
    Getter(GetterRef),
    Prop(PropRef),
    Watch(Watch),
    /// The seen-map of `traverse`: object -> depth already walked.
    Seen(RefCell<HashMap<ObjRef, f64>>),
}

impl NativeObject for Node {
    fn trace(&self, t: &mut Tracer) {
        match self {
            Node::Dep(x) => x.trace(t),
            Node::Link(x) => x.trace(t),
            Node::Effect(x) => x.trace(t),
            Node::Computed(x) => x.trace(t),
            Node::Scope(x) => x.trace(t),
            Node::Ref(x) => x.trace(t),
            Node::Custom(x) => x.trace(t),
            Node::Getter(x) => x.trace(t),
            Node::Prop(x) => x.trace(t),
            Node::Watch(x) => x.trace(t),
            Node::Seen(m) => {
                for k in m.borrow().keys() {
                    t.mark(*k);
                }
            }
        }
    }
}

/// Live counts of the reactivity objects of one realm, by kind (read by `debug_stats`). A counted payload adds one to
/// its kind while it exists and takes it away when the collector frees it.
#[derive(Default)]
pub(crate) struct Live {
    pub(crate) effects: Cell<usize>,
    pub(crate) computeds: Cell<usize>,
    pub(crate) scopes: Cell<usize>,
    pub(crate) watchers: Cell<usize>,
    pub(crate) deps: Cell<usize>,
    pub(crate) links: Cell<usize>,
    /// Links in the subscriber list of a dep (kept by `graph::add_sub` and `graph::remove_sub`).
    pub(crate) subscriber_edges: Cell<usize>,
}

/// The payload of every reactivity heap object: the node and the counters of its realm. Freeing the object drops the
/// payload, and the drop only updates a counter (it never calls back into the runtime, see ABI.md).
pub(crate) struct Tracked {
    node: Node,
    live: Rc<Live>,
}

impl Drop for Tracked {
    fn drop(&mut self) {
        if let Some(c) = self.node.counter(&self.live) {
            c.set(c.get() - 1);
        }
    }
}

impl NativeObject for Tracked {
    fn trace(&self, t: &mut Tracer) {
        self.node.trace(t);
    }
}

macro_rules! variant {
    ($name:ident, $ty:ty, $var:ident) => {
        pub(crate) fn $name(&self) -> &$ty {
            match self {
                Node::$var(x) => x,
                _ => panic!("reactivity: unexpected node kind (framework bug)"),
            }
        }
    };
}

impl Node {
    /// The live count of this node's kind, or None for the kinds that are not counted.
    fn counter<'a>(&self, live: &'a Live) -> Option<&'a Cell<usize>> {
        match self {
            Node::Effect(_) => Some(&live.effects),
            Node::Computed(_) => Some(&live.computeds),
            Node::Scope(_) => Some(&live.scopes),
            Node::Watch(_) => Some(&live.watchers),
            Node::Dep(_) => Some(&live.deps),
            Node::Link(_) => Some(&live.links),
            _ => None,
        }
    }

    variant!(dep, Dep, Dep);
    variant!(link, Link, Link);
    variant!(effect, Effect, Effect);
    variant!(computed, Computed, Computed);
    variant!(scope, Scope, Scope);
    variant!(ref_impl, RefImpl, Ref);
    variant!(custom, CustomRef, Custom);
    variant!(getter, GetterRef, Getter);
    variant!(prop, PropRef, Prop);
    variant!(watch, Watch, Watch);

    pub(crate) fn seen(&self) -> &RefCell<HashMap<ObjRef, f64>> {
        match self {
            Node::Seen(m) => m,
            _ => panic!("reactivity: expected a seen-map (framework bug)"),
        }
    }

    /// The subscriber fields of an effect or a computed.
    pub(crate) fn sub(&self) -> &SubFields {
        match self {
            Node::Effect(e) => &e.sub,
            Node::Computed(c) => &c.sub,
            _ => panic!("reactivity: expected a subscriber (framework bug)"),
        }
    }

    pub(crate) fn is_effect(&self) -> bool {
        matches!(self, Node::Effect(_))
    }

    pub(crate) fn is_computed(&self) -> bool {
        matches!(self, Node::Computed(_))
    }

    pub(crate) fn is_scope(&self) -> bool {
        matches!(self, Node::Scope(_))
    }

    pub(crate) fn is_dep(&self) -> bool {
        matches!(self, Node::Dep(_))
    }
}

/// A borrowed view of a node's payload (an `Rc` clone, so no RefCell of the heap is held).
pub(crate) struct NodeRef(Rc<dyn NativeObject>);

impl Deref for NodeRef {
    type Target = Node;
    fn deref(&self) -> &Node {
        &payload_ref::<Tracked>(&*self.0).expect("reactivity node (framework bug)").node
    }
}

/// The node behind `o`; a framework bug when `o` is not a reactivity node.
pub(crate) fn node(rt: &Rt, o: ObjRef) -> NodeRef {
    try_node(rt, o).expect("object is not a reactivity node (framework bug)")
}

/// The node behind `o`, or `None` for other objects and primitives' handles.
pub(crate) fn try_node(rt: &Rt, o: ObjRef) -> Option<NodeRef> {
    let p = rt.native_payload(o)?;
    if payload_ref::<Tracked>(&*p).is_none() {
        return None;
    }
    Some(NodeRef(p))
}

/// Allocates a node object with the given prototype (None for internal nodes). The payload is counted in its realm's
/// live counts until the collector frees it.
pub(crate) fn alloc(rt: &Rt, proto: Option<ObjRef>, n: Node) -> ObjRef {
    let live = Rc::clone(&state(rt).live);
    if let Some(c) = n.counter(&live) {
        c.set(c.get() + 1);
    }
    let payload: Rc<dyn NativeObject> = Rc::new(Tracked { node: n, live });
    rt.new_native_object(proto, payload)
}

/// The receiver of a JS method or accessor: an object whose node satisfies `ok`, else a TypeError.
pub(crate) fn this_node(rt: &Rt, this: &Value, ok: fn(&Node) -> bool) -> Res<ObjRef> {
    if let Value::Object(o) = this {
        if let Some(n) = try_node(rt, *o) {
            if ok(&*n) {
                return Ok(*o);
            }
        }
    }
    Err(rt.throw_type_error("Illegal invocation: receiver is not a reactivity object"))
}
