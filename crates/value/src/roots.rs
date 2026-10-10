//! Garbage collection: roots, the `Trace` contract and the mark-sweep driver. Collection runs only at
//! safepoints (call depth 0). Marking is iterative: a worklist of `ObjRef`s, never recursion.

use std::rc::Rc;

use crate::heap::{Heap, HeapStats};
use crate::rt::Rt;
use crate::value::{ObjRef, Throw, Value};

/// Rust-side structures that hold `ObjRef`s implement this and are reachable from a root.
pub trait Trace {
    fn trace(&self, t: &mut Tracer);
}

/// Registered with `Rt::add_root_provider`. Must mark every `ObjRef` it holds.
pub trait RootProvider {
    fn trace_roots(&self, t: &mut Tracer);
}

/// Mark state for one collection: a mark bit per slot and the worklist of marked, unscanned objects.
pub struct Tracer {
    marks: Vec<bool>,
    stack: Vec<ObjRef>,
}

impl Tracer {
    pub(crate) fn new(slots: usize) -> Tracer {
        Tracer { marks: vec![false; slots], stack: Vec::new() }
    }

    /// Marks an object reachable. Marking is idempotent.
    pub fn mark(&mut self, r: ObjRef) {
        match self.marks.get_mut(r.slot as usize) {
            Some(m) => {
                if !*m {
                    *m = true;
                    self.stack.push(r);
                }
            }
            None => panic!("ObjRef slot {} is outside the heap (framework bug)", r.slot),
        }
    }

    pub fn mark_value(&mut self, v: &Value) {
        if let Value::Object(r) = v {
            self.mark(*r);
        }
    }

    /// Scans marked objects until the worklist is empty.
    pub(crate) fn drain(&mut self, heap: &Heap) {
        while let Some(r) = self.stack.pop() {
            heap.get(r).trace(self);
        }
    }

    pub(crate) fn into_marks(self) -> Vec<bool> {
        self.marks
    }
}

impl Trace for Value {
    fn trace(&self, t: &mut Tracer) {
        t.mark_value(self);
    }
}

impl Trace for ObjRef {
    fn trace(&self, t: &mut Tracer) {
        t.mark(*self);
    }
}

impl Trace for Throw {
    fn trace(&self, t: &mut Tracer) {
        t.mark_value(&self.0);
    }
}

impl<T: Trace> Trace for Option<T> {
    fn trace(&self, t: &mut Tracer) {
        if let Some(x) = self {
            x.trace(t);
        }
    }
}

impl<T: Trace> Trace for [T] {
    fn trace(&self, t: &mut Tracer) {
        for x in self {
            x.trace(t);
        }
    }
}

impl<T: Trace> Trace for Vec<T> {
    fn trace(&self, t: &mut Tracer) {
        self.as_slice().trace(t);
    }
}

/// Handle of a persistent root (`Rt::root`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RootId(pub(crate) u32);

/// Handle of a registered root provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProviderId(pub(crate) u32);

#[derive(Default)]
pub(crate) struct RootTable {
    slots: Vec<Option<Value>>,
    free: Vec<u32>,
}

impl RootTable {
    pub(crate) fn add(&mut self, v: Value) -> RootId {
        match self.free.pop() {
            Some(i) => {
                self.slots[i as usize] = Some(v);
                RootId(i)
            }
            None => {
                self.slots.push(Some(v));
                RootId((self.slots.len() - 1) as u32)
            }
        }
    }

    pub(crate) fn remove(&mut self, id: RootId) {
        let slot = self.slots.get_mut(id.0 as usize).expect("unknown RootId (framework bug)");
        assert!(slot.take().is_some(), "RootId unrooted twice (framework bug)");
        self.free.push(id.0);
    }

    pub(crate) fn get(&self, id: RootId) -> Value {
        match self.slots.get(id.0 as usize) {
            Some(Some(v)) => v.clone(),
            _ => panic!("unknown or released RootId (framework bug)"),
        }
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &Value> {
        self.slots.iter().flatten()
    }
}

impl Rt {
    /// Pins a value until `unroot`. Returns the handle to release it with.
    pub fn root(&self, v: &Value) -> RootId {
        self.0.roots.borrow_mut().add(v.clone())
    }

    pub fn unroot(&self, id: RootId) {
        self.0.roots.borrow_mut().remove(id);
    }

    pub fn root_value(&self, id: RootId) -> Value {
        self.0.roots.borrow().get(id)
    }

    pub fn add_root_provider(&self, p: Rc<dyn RootProvider>) -> ProviderId {
        let mut ps = self.0.providers.borrow_mut();
        ps.push(Some(p));
        ProviderId((ps.len() - 1) as u32)
    }

    pub fn remove_root_provider(&self, id: ProviderId) {
        let mut ps = self.0.providers.borrow_mut();
        let slot = ps.get_mut(id.0 as usize).expect("unknown ProviderId (framework bug)");
        assert!(slot.take().is_some(), "ProviderId removed twice (framework bug)");
    }

    /// Whether `obj` still names a live object (false after the collector freed it).
    pub fn is_live(&self, obj: ObjRef) -> bool {
        self.0.heap.borrow().is_live(obj)
    }

    pub fn heap_stats(&self) -> HeapStats {
        self.0.heap.borrow().stats()
    }

    /// Sets how many allocations `maybe_collect` waits for before it collects.
    pub fn set_gc_threshold(&self, allocations: usize) {
        self.0.gc_threshold.set(allocations);
    }

    /// Full mark-sweep. Panics if called while generated code is running (call depth above 0): compiled
    /// code keeps temporaries on the Rust stack, which are not roots.
    pub fn collect(&self) {
        let d = self.call_depth();
        if d != 0 {
            panic!("GC inside running code: collect() at call depth {d} (framework bug)");
        }
        self.gc();
    }

    /// Like `collect`, but returns false without collecting when code is running.
    pub fn try_collect(&self) -> bool {
        if self.call_depth() != 0 {
            return false;
        }
        self.gc();
        true
    }

    /// Collects when the allocation count since the last collection reaches the threshold and no code is
    /// running. Returns whether it collected.
    pub fn maybe_collect(&self) -> bool {
        let st = &self.0;
        if st.depth.get() == 0 && st.heap.borrow().allocs_since_gc >= st.gc_threshold.get() {
            self.gc();
            return true;
        }
        false
    }

    fn gc(&self) {
        let st = &self.0;
        let mut t = Tracer::new(st.heap.borrow().slot_count());
        for r in st.intrinsics.borrow().iter() {
            t.mark(*r);
        }
        for r in st.registry.borrow().values() {
            t.mark(*r);
        }
        for v in st.roots.borrow().values() {
            t.mark_value(v);
        }
        for job in st.jobs.borrow().iter().chain(st.hosts.borrow().iter()) {
            t.mark_value(&job.func);
            for a in &job.args {
                t.mark_value(a);
            }
        }
        for e in st.job_errors.borrow().iter() {
            e.trace(&mut t);
        }
        // Providers run without any borrow held: they only mark, but may still touch their own state.
        let providers: Vec<Rc<dyn RootProvider>> = st.providers.borrow().iter().flatten().cloned().collect();
        for p in &providers {
            p.trace_roots(&mut t);
        }
        {
            let heap = st.heap.borrow();
            t.drain(&heap);
        }
        let marks = t.into_marks();
        st.heap.borrow_mut().sweep(&marks);
    }
}
