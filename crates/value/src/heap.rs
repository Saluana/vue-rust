//! Object storage: a non-moving slot arena. A slot's generation is bumped when it is freed, so an
//! `ObjRef` that outlived its object is detected on every access. A stale access panics: the collector
//! only frees objects that no root reaches, so a stale handle is a framework bug, never a JS value.

use crate::object::Obj;
use crate::value::ObjRef;

/// Estimated bytes of an object with no properties and no elements.
pub(crate) const BASE_BYTES: usize = 96;
/// Estimated bytes per own property entry.
pub(crate) const PROP_BYTES: usize = 48;
/// Estimated bytes per dense array element.
pub(crate) const ELEM_BYTES: usize = 24;

pub(crate) struct Slot {
    pub(crate) ver: u32,
    pub(crate) obj: Option<Obj>,
    pub(crate) bytes: usize,
}

/// Counters reported by `Rt::heap_stats`. The peaks are high-water marks since the realm was created: they rise on
/// allocation and property growth, and a collection does not lower them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeapStats {
    pub live_objects: usize,
    pub allocated_total: u64,
    pub collections: u64,
    pub freed_total: u64,
    pub bytes_estimate: usize,
    pub peak_live_objects: usize,
    pub peak_bytes_estimate: usize,
}

#[derive(Default)]
pub(crate) struct Heap {
    slots: Vec<Slot>,
    free: Vec<u32>,
    pub(crate) live: usize,
    pub(crate) allocated_total: u64,
    pub(crate) freed_total: u64,
    pub(crate) collections: u64,
    pub(crate) bytes: usize,
    pub(crate) peak_live: usize,
    pub(crate) peak_bytes: usize,
    pub(crate) allocs_since_gc: usize,
}

impl Heap {
    pub(crate) fn alloc(&mut self, obj: Obj, bytes: usize) -> ObjRef {
        let slot = match self.free.pop() {
            Some(s) => s,
            None => {
                let s = self.slots.len();
                assert!(s < u32::MAX as usize, "object heap slot space exhausted (framework bug)");
                self.slots.push(Slot { ver: 0, obj: None, bytes: 0 });
                s as u32
            }
        };
        let entry = &mut self.slots[slot as usize];
        debug_assert!(entry.obj.is_none(), "allocated into a live slot (framework bug)");
        entry.obj = Some(obj);
        entry.bytes = bytes;
        let ver = entry.ver;
        self.bytes += bytes;
        self.live += 1;
        self.allocated_total += 1;
        self.allocs_since_gc += 1;
        self.peak_live = self.peak_live.max(self.live);
        self.peak_bytes = self.peak_bytes.max(self.bytes);
        ObjRef { slot, r#gen: ver }
    }

    fn stale(r: ObjRef) -> ! {
        panic!(
            "stale ObjRef (slot {}, ver {}): the object was freed by the collector (framework bug)",
            r.slot, r.r#gen
        )
    }

    pub(crate) fn get(&self, r: ObjRef) -> &Obj {
        match self.slots.get(r.slot as usize) {
            Some(Slot { ver, obj: Some(o), .. }) if *ver == r.r#gen => o,
            _ => Self::stale(r),
        }
    }

    pub(crate) fn get_mut(&mut self, r: ObjRef) -> &mut Obj {
        match self.slots.get_mut(r.slot as usize) {
            Some(Slot { ver, obj: Some(o), .. }) if *ver == r.r#gen => o,
            _ => Self::stale(r),
        }
    }

    pub(crate) fn is_live(&self, r: ObjRef) -> bool {
        matches!(self.slots.get(r.slot as usize), Some(Slot { ver, obj: Some(_), .. }) if *ver == r.r#gen)
    }

    fn slot_mut(&mut self, r: ObjRef) -> &mut Slot {
        match self.slots.get_mut(r.slot as usize) {
            Some(s) if s.ver == r.r#gen && s.obj.is_some() => s,
            _ => Self::stale(r),
        }
    }

    /// Adds `n` estimated bytes to a live object (a property or elements were added).
    pub(crate) fn add_bytes(&mut self, r: ObjRef, n: usize) {
        self.slot_mut(r).bytes += n;
        self.bytes += n;
        self.peak_bytes = self.peak_bytes.max(self.bytes);
    }

    /// Removes `n` estimated bytes from a live object (a property or elements were removed).
    pub(crate) fn sub_bytes(&mut self, r: ObjRef, n: usize) {
        self.slot_mut(r).bytes -= n;
        self.bytes -= n;
    }

    pub(crate) fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Frees every slot whose mark is false and returns how many were freed. Dropping a payload
    /// never calls back into the runtime (see ABI.md).
    pub(crate) fn sweep(&mut self, marks: &[bool]) -> usize {
        assert_eq!(marks.len(), self.slots.len(), "mark table does not match the heap (framework bug)");
        let mut freed = 0;
        for (i, s) in self.slots.iter_mut().enumerate() {
            if s.obj.is_some() && !marks[i] {
                s.obj = None;
                self.bytes -= s.bytes;
                s.bytes = 0;
                s.ver = s.ver.wrapping_add(1);
                self.free.push(i as u32);
                freed += 1;
            }
        }
        self.live -= freed;
        self.freed_total += freed as u64;
        self.allocs_since_gc = 0;
        self.collections += 1;
        freed
    }

    pub(crate) fn stats(&self) -> HeapStats {
        HeapStats {
            live_objects: self.live,
            allocated_total: self.allocated_total,
            collections: self.collections,
            freed_total: self.freed_total,
            bytes_estimate: self.bytes,
            peak_live_objects: self.peak_live,
            peak_bytes_estimate: self.peak_bytes,
        }
    }
}
