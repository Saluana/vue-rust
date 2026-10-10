//! The churn workload of crates/value's gc_churn_bench, rebuilt on gc-arena 0.7.0 (WP-3.2 comparison; experiment only).
//!
//!   cd experiments/heap/gc-arena-churn
//!   CARGO_TARGET_DIR=/home/claude/targets/w32-gc-arena cargo run --release
//!
//! Same live-set sizes, rounds, shape and allocation threshold as crates/value/examples/gc_churn_bench.rs, and the same
//! object count per round (11161 at the bench shape). The graph has the same pieces: a list, rings, a wide object with a
//! child, a Map and a Set of objects, closures over cells, native functions over objects, promise chains with their
//! reactions, and one queued job. README.md lists how this differs from the vn-value workload.
//!
//! Two collection modes run the same workload. Only the collector call is timed.
//!
//! - cycle: a collection point runs `Arena::finish_cycle`, one full mark and sweep, comparable with the vn-value
//!   collections. After each round the live count is checked against the expected count.
//! - slice: a collection point runs `Arena::collect_debt` when allocation debt is due. A pause is one incremental slice,
//!   the unit that the design's 2 ms budget names. The live count is checked once, after a final full cycle.

use std::time::Instant;

use gc_arena::arena::CollectionPhase;
use gc_arena::metrics::Pacing;
use gc_arena::{Arena, Collect, Gc, Mutation, RefLock, Rootable};

/// Live-set sizes (stable objects) and the rounds run at each size, as in gc_churn_bench.
const RUNS: [(usize, usize); 3] = [(10_000, 40), (100_000, 40), (1_000_000, 30)];

/// Allocations between collections in cycle mode: the vn-value threshold.
const THRESHOLD: usize = 2_000;

/// The design's collection-slice budget: p99 at most 2 ms under the reference workload (design.md, section 14).
const P99_BUDGET_MS: f64 = 2.0;

/// Stable-set nodes besides the ring objects: the array, the Map, the probe closure and its cell.
const STABLE_EXTRA: usize = 4;

/// The paced slice mode's sleep factor: a cycle starts once allocations since the last cycle reach 5% of the remembered
/// objects, where the default (`Pacing::DEFAULT`) waits for 50%. A tuning experiment, not a recommendation.
const PACED_SLEEP_FACTOR: f64 = 0.05;

/// Sizes of one round, as in tests/support/churn.rs.
#[derive(Clone, Copy, Debug)]
struct Shape {
    list: usize,
    rings: usize,
    ring_size: usize,
    width: usize,
    pairs: usize,
    closures: usize,
    natives: usize,
    promises: usize,
}

const SHAPE: Shape = Shape {
    list: 2_000,
    rings: 100,
    ring_size: 10,
    width: 500,
    pairs: 500,
    closures: 1_000,
    natives: 1_000,
    promises: 300,
};

/// What a node is. A handler is `Wrap` (its reaction returns `{ from: arg }`) or `Pass` (returns its argument).
#[derive(Collect, Clone, Copy, Debug, PartialEq, Eq)]
#[collect(require_static)]
enum Kind {
    Object,
    Array,
    Map,
    Set,
    Cell,
    Closure,
    Native,
    Promise,
    Wrap,
    Pass,
    Job,
}

type Ptr<'gc> = Gc<'gc, RefLock<Node<'gc>>>;

/// One heap object. Every reference is an entry of `refs`, in the order the vn-value model links them. A `None` entry is
/// a holder slot that the round has cleared.
#[derive(Collect)]
#[collect(no_drop)]
struct Node<'gc> {
    kind: Kind,
    /// The numeric `id` property; 0 for an object without one.
    id: f64,
    /// Numeric properties, used by the wide object only.
    nums: Vec<f64>,
    refs: Vec<Option<Ptr<'gc>>>,
}

/// The roots: the stable set (pinned for the whole run), the round's holder, the list head carried into the next
/// round, the job queue, and `extra`, which only a negative control uses.
#[derive(Collect)]
#[collect(no_drop)]
struct Root<'gc> {
    stable: Option<Ptr<'gc>>,
    holder: Option<Ptr<'gc>>,
    carry: Option<Ptr<'gc>>,
    queued: Vec<(Ptr<'gc>, Ptr<'gc>)>,
    extra: Vec<Ptr<'gc>>,
}

type Heap = Arena<Rootable![Root<'_>]>;

/// Allocates nodes inside one `mutate` call and counts them.
struct Make<'a, 'gc> {
    mc: &'a Mutation<'gc>,
    made: usize,
}

impl<'a, 'gc> Make<'a, 'gc> {
    fn new(mc: &'a Mutation<'gc>) -> Self {
        Make { mc, made: 0 }
    }

    fn alloc(&mut self, node: Node<'gc>) -> Ptr<'gc> {
        self.made += 1;
        Gc::new(self.mc, RefLock::new(node))
    }

    fn node(&mut self, kind: Kind, refs: Vec<Option<Ptr<'gc>>>) -> Ptr<'gc> {
        self.alloc(Node { kind, id: 0.0, nums: Vec::new(), refs })
    }

    fn object(&mut self, id: f64) -> Ptr<'gc> {
        self.alloc(Node { kind: Kind::Object, id, nums: Vec::new(), refs: Vec::new() })
    }
}

/// A linked list of `len` objects; each `id` is its position from the head. Returns the head.
fn list<'gc>(m: &mut Make<'_, 'gc>, len: usize) -> Ptr<'gc> {
    let mut head: Option<Ptr<'gc>> = None;
    for id in (0..len).rev() {
        let refs = head.map(|next| vec![Some(next)]).unwrap_or_default();
        head = Some(m.alloc(Node { kind: Kind::Object, id: id as f64, nums: Vec::new(), refs }));
    }
    head.expect("a list has at least one node")
}

/// `count` cycles of `size` objects; each object's `next` is the following one and the last points back to the first.
/// Returns the first object of each cycle.
fn rings<'gc>(m: &mut Make<'_, 'gc>, count: usize, size: usize) -> Vec<Ptr<'gc>> {
    let mut heads = Vec::with_capacity(count);
    for _ in 0..count {
        let mut ring = Vec::with_capacity(size);
        for k in 0..size {
            ring.push(m.object(k as f64));
        }
        for k in 0..size {
            let next = ring[(k + 1) % size];
            ring[k].borrow_mut(m.mc).refs.push(Some(next));
        }
        heads.push(ring[0]);
    }
    heads
}

/// An object with `width` numeric properties and a `child` object. Returns the object.
fn wide<'gc>(m: &mut Make<'_, 'gc>, width: usize) -> Ptr<'gc> {
    let child = m.object(0.0);
    let nums = (0..width).map(|k| k as f64).collect();
    m.alloc(Node { kind: Kind::Object, id: 0.0, nums, refs: vec![Some(child)] })
}

/// A Map from `pairs` new object keys to `pairs` new object values, held as alternating key and value entries.
fn map<'gc>(m: &mut Make<'_, 'gc>, pairs: usize) -> Ptr<'gc> {
    let mut refs = Vec::with_capacity(2 * pairs);
    for k in 0..pairs {
        let key = m.object(k as f64);
        let value = m.object(k as f64);
        refs.push(Some(key));
        refs.push(Some(value));
    }
    m.node(Kind::Map, refs)
}

/// A Set of `members` new objects.
fn set<'gc>(m: &mut Make<'_, 'gc>, members: usize) -> Ptr<'gc> {
    let mut refs = Vec::with_capacity(members);
    for k in 0..members {
        refs.push(Some(m.object(k as f64)));
    }
    m.node(Kind::Set, refs)
}

/// A closure over `obj`: the function holds a cell, and the cell holds the object.
fn closure<'gc>(m: &mut Make<'_, 'gc>, obj: Ptr<'gc>) -> Ptr<'gc> {
    let cell = m.node(Kind::Cell, vec![Some(obj)]);
    m.node(Kind::Closure, vec![Some(cell)])
}

/// The stable live set: `n` objects in a ring, a Map from number to object, a closure over the first object, and the
/// array that holds them all. Pinned for the whole run.
fn stable<'gc>(m: &mut Make<'_, 'gc>, n: usize) -> Ptr<'gc> {
    let mut objs = Vec::with_capacity(n);
    for k in 0..n {
        objs.push(m.object(k as f64));
    }
    for k in 0..n {
        let next = objs[(k + 1) % n];
        objs[k].borrow_mut(m.mc).refs.push(Some(next));
    }
    let map = m.node(Kind::Map, objs.iter().map(|&o| Some(o)).collect());
    let probe = closure(m, objs[0]);
    let mut elems: Vec<Option<Ptr<'gc>>> = objs.iter().map(|&o| Some(o)).collect();
    elems.push(Some(map));
    elems.push(Some(probe));
    m.node(Kind::Array, elems)
}

/// Appends `x` to the holder array and returns its slot.
fn attach<'gc>(mc: &Mutation<'gc>, holder: Ptr<'gc>, x: Ptr<'gc>) -> usize {
    let mut h = holder.borrow_mut(mc);
    h.refs.push(Some(x));
    h.refs.len() - 1
}

/// Clears a holder slot: the structure in it is no longer reachable from the holder.
fn clear_slot<'gc>(mc: &Mutation<'gc>, holder: Ptr<'gc>, slot: usize) {
    holder.borrow_mut(mc).refs[slot] = None;
}

/// The structure held in `slot` of the holder.
fn held<'gc>(holder: Ptr<'gc>, slot: usize) -> Ptr<'gc> {
    holder.borrow().refs[slot].expect("a held structure is attached")
}

fn same<'gc>(a: Option<Ptr<'gc>>, b: Ptr<'gc>) -> bool {
    a.is_some_and(|a| Gc::ptr_eq(a, b))
}

/// Walks the list from `head`: positions are the ids, and the tail links to nothing.
fn check_list<'gc>(head: Ptr<'gc>, len: usize) -> Result<(), String> {
    let mut cur = Some(head);
    for k in 0..len {
        let node = cur.ok_or_else(|| format!("the list ends at {k} of {len} nodes"))?;
        let b = node.borrow();
        if b.id != k as f64 {
            return Err(format!("list position {k} has id {}", b.id));
        }
        cur = b.refs.first().copied().flatten();
    }
    if cur.is_some() {
        return Err(format!("the list continues past {len} nodes"));
    }
    Ok(())
}

/// Reads the stable set back: the ring, the Map and the probe closure must be as `stable` built them.
fn check_stable<'gc>(arr: Ptr<'gc>, n: usize) -> Result<(), String> {
    let a = arr.borrow();
    if a.kind != Kind::Array || a.refs.len() != n + 2 {
        return Err(format!("the stable array has {} entries, expected {}", a.refs.len(), n + 2));
    }
    let mut objs = Vec::with_capacity(n);
    for entry in &a.refs[..n] {
        objs.push((*entry).ok_or("a stable entry is empty")?);
    }
    for (k, &o) in objs.iter().enumerate() {
        let ob = o.borrow();
        if ob.id != k as f64 {
            return Err(format!("stable object {k} has id {}", ob.id));
        }
        if !same(ob.refs.first().copied().flatten(), objs[(k + 1) % n]) {
            return Err(format!("stable object {k} does not link to its successor"));
        }
    }
    let map = a.refs[n].ok_or("the stable Map is missing")?;
    let mb = map.borrow();
    if mb.kind != Kind::Map || mb.refs.len() != n {
        return Err(format!("the stable Map has {} entries, expected {n}", mb.refs.len()));
    }
    for k in 0..n {
        if !same(mb.refs[k], objs[k]) {
            return Err(format!("stable map entry {k} is not stable object {k}"));
        }
    }
    let probe = a.refs[n + 1].ok_or("the stable probe is missing")?;
    let pb = probe.borrow();
    let cell = pb.refs.first().copied().flatten().ok_or("the stable probe has no cell")?;
    let cb = cell.borrow();
    if pb.kind != Kind::Closure || !same(cb.refs.first().copied().flatten(), objs[0]) {
        return Err("the stable closure no longer returns its captured object".into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Every collection point is a full `finish_cycle`.
    Cycle,
    /// Slices at due collection points, with gc-arena's default pacing.
    Slice,
    /// Slices at due collection points, with `PACED_SLEEP_FACTOR`.
    SlicePaced,
}

struct Harness {
    mode: Mode,
    arena: Heap,
    /// Stable ring objects.
    n: usize,
    /// Allocations between collections in cycle mode.
    threshold: usize,
    /// Allocations since the last collection (cycle mode's threshold counter).
    since: usize,
    /// Every node allocated, the stable set included.
    allocated: usize,
    /// Nodes in the stable set: `n + STABLE_EXTRA`.
    stable_nodes: usize,
    /// Cycle mode: collections the threshold triggered, and the explicit ones.
    automatic: usize,
    explicit: usize,
    /// Collection points visited.
    points: usize,
    /// Cycle mode: (ms, objects freed) for each collection.
    pauses: Vec<(f64, usize)>,
    /// Slice mode: ms for each incremental slice.
    slices: Vec<f64>,
}

impl Harness {
    fn new(mode: Mode, n: usize) -> Harness {
        Harness::with_threshold(mode, n, THRESHOLD)
    }

    fn with_threshold(mode: Mode, n: usize, threshold: usize) -> Harness {
        let arena = Heap::new(|_mc| Root { stable: None, holder: None, carry: None, queued: Vec::new(), extra: Vec::new() });
        if mode == Mode::SlicePaced {
            arena.metrics().set_pacing(Pacing { sleep_factor: PACED_SLEEP_FACTOR, ..Pacing::DEFAULT });
        }
        let mut h = Harness {
            mode,
            arena,
            n,
            threshold,
            since: 0,
            allocated: 0,
            stable_nodes: n + STABLE_EXTRA,
            automatic: 0,
            explicit: 0,
            points: 0,
            pauses: Vec::new(),
            slices: Vec::new(),
        };
        let made = h.arena.mutate_root(|mc, root| {
            let mut m = Make::new(mc);
            root.stable = Some(stable(&mut m, n));
            m.made
        });
        h.account(made);
        h
    }

    fn account(&mut self, made: usize) {
        self.since += made;
        self.allocated += made;
    }

    fn created(&self) -> usize {
        self.allocated - self.stable_nodes
    }

    fn live(&self) -> usize {
        self.arena.metrics().total_gc_count()
    }

    /// A collection point. `explicit` marks the three collections a round makes; otherwise the point is the workload's
    /// safepoint after an attach, which collects when the threshold is due.
    fn safepoint(&mut self, explicit: bool) {
        self.points += 1;
        match self.mode {
            Mode::Cycle => {
                if !explicit && self.since < self.threshold {
                    return;
                }
                assert_eq!(self.arena.collection_phase(), CollectionPhase::Sleeping, "a full cycle starts from sleep");
                let before = self.live();
                let started = Instant::now();
                self.arena.finish_cycle();
                let ms = started.elapsed().as_secs_f64() * 1e3;
                self.pauses.push((ms, before - self.live()));
                if explicit {
                    self.explicit += 1;
                } else {
                    self.automatic += 1;
                }
                self.since = 0;
            }
            Mode::Slice | Mode::SlicePaced => {
                if self.arena.metrics().allocation_debt() > 0.0 {
                    let started = Instant::now();
                    self.arena.collect_debt();
                    self.slices.push(started.elapsed().as_secs_f64() * 1e3);
                }
            }
        }
    }

    /// Slice mode: completes the cycle in progress, then runs one full cycle, so the live count is exact. Returns the
    /// time of both calls in milliseconds. These calls are not part of the slice statistics.
    fn finish(&mut self) -> f64 {
        let started = Instant::now();
        self.arena.finish_cycle();
        self.arena.finish_cycle();
        started.elapsed().as_secs_f64() * 1e3
    }

    /// Builds a structure and attaches it to the holder in one `mutate` call, so no collection can run in between.
    /// Returns the holder slot.
    fn stage(&mut self, build: impl for<'gc> FnOnce(&mut Make<'_, 'gc>) -> Ptr<'gc>) -> usize {
        let (made, slot) = self.arena.mutate(|mc, root| {
            let mut m = Make::new(mc);
            let x = build(&mut m);
            let slot = attach(mc, root.holder.expect("the holder is pinned"), x);
            (m.made, slot)
        });
        self.account(made);
        slot
    }

    fn detach(&self, slot: usize) {
        self.arena.mutate(|mc, root| clear_slot(mc, root.holder.expect("the holder is pinned"), slot));
    }

    /// A collection point after a structure is attached, then its slot is cleared.
    fn release(&mut self, slot: usize) {
        self.safepoint(false);
        self.detach(slot);
    }

    /// One churn round, in the order of tests/support/churn.rs. Returns the objects the round created.
    fn round(&mut self, s: &Shape) -> Result<usize, String> {
        let start = self.allocated;

        // The round's holder is pinned, and the list head that the previous round carried is released.
        let made = self.arena.mutate_root(|mc, root| {
            let mut m = Make::new(mc);
            root.holder = Some(m.node(Kind::Array, Vec::new()));
            root.carry = None;
            m.made
        });
        self.account(made);

        // The list is attached to the holder and carried into the next round.
        let (made, slot) = self.arena.mutate_root(|mc, root| {
            let mut m = Make::new(mc);
            let head = list(&mut m, s.list);
            let slot = attach(mc, root.holder.expect("the holder is pinned"), head);
            root.carry = Some(head);
            (m.made, slot)
        });
        self.account(made);
        self.safepoint(false);
        self.arena.mutate(|_, root| check_list(held(root.holder.expect("the holder is pinned"), slot), s.list))?;
        self.detach(slot);

        let slot = self.stage(|m| {
            let heads = rings(m, s.rings, s.ring_size);
            m.node(Kind::Array, heads.iter().map(|&h| Some(h)).collect())
        });
        self.release(slot);

        let slot = self.stage(|m| wide(m, s.width));
        self.release(slot);

        let slot = self.stage(|m| map(m, s.pairs));
        self.release(slot);

        let slot = self.stage(|m| set(m, s.pairs));
        self.release(slot);

        let slot = self.stage(|m| {
            let mut fns = Vec::with_capacity(s.closures);
            for k in 0..s.closures {
                let obj = m.object(k as f64);
                fns.push(Some(closure(m, obj)));
            }
            m.node(Kind::Array, fns)
        });
        self.release(slot);

        let slot = self.stage(|m| {
            let mut fns = Vec::with_capacity(s.natives);
            for k in 0..s.natives {
                let obj = m.object(k as f64);
                fns.push(Some(m.node(Kind::Native, vec![Some(obj)])));
            }
            m.node(Kind::Array, fns)
        });
        self.release(slot);

        let slot = self.stage(|m| {
            // Each promise: its value, the executor that fulfils it, the promise, the handler, and the derived promise.
            let mut jobs = Vec::with_capacity(s.promises);
            for k in 0..s.promises {
                let value = m.object(k as f64);
                let _executor = m.node(Kind::Native, vec![Some(value)]);
                let _promise = m.node(Kind::Promise, vec![Some(value)]);
                let wrap = k % 2 == 0;
                let _handler = m.node(if wrap { Kind::Wrap } else { Kind::Pass }, Vec::new());
                let derived = m.node(Kind::Promise, Vec::new());
                jobs.push((wrap, value, derived));
            }
            // Each reaction runs once: a wrapping handler returns { from: arg }, the other returns its argument.
            for &(wrap, value, derived) in &jobs {
                let result = if wrap { m.node(Kind::Object, vec![Some(value)]) } else { value };
                derived.borrow_mut(m.mc).refs.push(Some(result));
            }
            m.node(Kind::Array, jobs.iter().map(|&(_, _, d)| Some(d)).collect())
        });
        self.release(slot);

        // A queued job holds its argument until the queue drains. Collections run before and after the drain.
        let made = self.arena.mutate_root(|mc, root| {
            let mut m = Make::new(mc);
            let arg = m.object(-1.0);
            let job = m.node(Kind::Job, Vec::new());
            root.queued.push((job, arg));
            m.made
        });
        self.account(made);
        self.safepoint(true);
        let ran = self.arena.mutate_root(|_, root| {
            let jobs = root.queued.len();
            root.queued.clear();
            jobs
        });
        if ran != 1 {
            return Err(format!("one queued job was expected, {ran} ran"));
        }
        self.safepoint(true);

        // The holder is unpinned. The round's garbage is now the list it carried into the next round, plus nothing.
        self.arena.mutate_root(|_, root| root.holder = None);
        self.safepoint(true);

        if self.mode == Mode::Cycle {
            let want = self.stable_nodes + s.list;
            let got = self.live();
            if got != want {
                return Err(format!(
                    "live objects {got}, expected {want} (stable set {} + carried list {})",
                    self.stable_nodes, s.list
                ));
            }
        }
        Ok(self.allocated - start)
    }

    fn verify_stable(&self) -> Result<(), String> {
        let n = self.n;
        self.arena.mutate(|_, root| check_stable(root.stable.expect("the stable set is pinned"), n))
    }

    /// Negative control: keeps one object reachable that the model does not expect.
    #[cfg(test)]
    fn leak(&mut self) {
        let made = self.arena.mutate_root(|mc, root| {
            let mut m = Make::new(mc);
            let x = m.object(0.0);
            root.extra.push(x);
            m.made
        });
        self.account(made);
    }

    /// Negative control: drops the root of the stable set, which the model still expects.
    #[cfg(test)]
    fn drop_stable_root(&mut self) {
        self.arena.mutate_root(|_, root| root.stable = None);
    }
}

/// Nearest-rank percentile of a sorted, non-empty slice.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = (p * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("model name")).map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string()))
        .unwrap_or_else(|| "unknown CPU".to_string())
}

fn print_cycle(h: &Harness, live: usize, rounds: usize, wall: f64) {
    let mut ms: Vec<f64> = h.pauses.iter().map(|p| p.0).collect();
    let mut freed: Vec<usize> = h.pauses.iter().map(|p| p.1).collect();
    ms.sort_by(|a, b| a.partial_cmp(b).expect("finite pause"));
    freed.sort_unstable();
    let total_ms: f64 = ms.iter().sum();
    let mean = total_ms / ms.len() as f64;
    let (p50, p95, p99) = (percentile(&ms, 0.50), percentile(&ms, 0.95), percentile(&ms, 0.99));
    let max = ms[ms.len() - 1];
    let none = freed.iter().filter(|&&f| f == 0).count();
    let mean_freed = freed.iter().sum::<usize>() as f64 / freed.len() as f64;

    println!();
    println!("== cycle mode: live set {live} stable objects, {rounds} rounds, threshold {} allocations ==", h.threshold);
    println!("objects created by the workload: {}; stable set: {} objects", h.created(), h.stable_nodes);
    println!(
        "collections: {} (threshold {}, explicit {}); each round's live count checked against the expected count",
        ms.len(),
        h.automatic,
        h.explicit
    );
    println!("pause ms: p50 {p50:.3}  p95 {p95:.3}  p99 {p99:.3}  max {max:.3}  mean {mean:.3}");
    println!(
        "objects freed per collection: min {}  p50 {}  p95 {}  max {}  mean {mean_freed:.0}; {none} of {} freed nothing",
        freed[0],
        freed[freed.len() / 2],
        freed[(freed.len() * 95 / 100).min(freed.len() - 1)],
        freed[freed.len() - 1],
        freed.len()
    );
    println!("heap after the last collection: live_objects {}", h.live());
    println!("time inside collections: {total_ms:.1} ms of {wall:.2} s wall (wall includes the workload)");
    let verdict = if p99 <= P99_BUDGET_MS { "meets" } else { "does not meet" };
    println!("p99 {p99:.3} ms {verdict} the {P99_BUDGET_MS} ms collection-slice budget (design.md section 14)");
}

fn print_slice(h: &Harness, live: usize, rounds: usize, wall: f64, before_final: usize, final_ms: f64) {
    let (label, sleep) = match h.mode {
        Mode::SlicePaced => ("paced slice", PACED_SLEEP_FACTOR),
        _ => ("slice, default pacing", Pacing::DEFAULT.sleep_factor),
    };
    let mut ms = h.slices.clone();
    ms.sort_by(|a, b| a.partial_cmp(b).expect("finite slice"));
    println!();
    println!("== {label} (sleep_factor {sleep}): live set {live} stable objects, {rounds} rounds, collect_debt when due ==");
    println!("objects created by the workload: {}; stable set: {} objects", h.created(), h.stable_nodes);
    println!("slices: {} (of {} collection points)", ms.len(), h.points);
    if let Some(first) = h.slices.first() {
        println!("first slice: {first:.3} ms (the first collection point pays the debt of the stable set's allocations)");
    }
    if ms.is_empty() {
        println!("no slice ran");
    } else {
        let total_ms: f64 = ms.iter().sum();
        let mean = total_ms / ms.len() as f64;
        let (p50, p95, p99) = (percentile(&ms, 0.50), percentile(&ms, 0.95), percentile(&ms, 0.99));
        let max = ms[ms.len() - 1];
        println!("slice pause ms: p50 {p50:.3}  p95 {p95:.3}  p99 {p99:.3}  max {max:.3}  mean {mean:.3}");
        println!("time inside slices: {total_ms:.1} ms of {wall:.2} s wall (wall includes the workload)");
        let verdict = if p99 <= P99_BUDGET_MS { "meets" } else { "does not meet" };
        println!("slice p99 {p99:.3} ms {verdict} the {P99_BUDGET_MS} ms collection-slice budget (design.md section 14)");
    }
    println!(
        "live objects before the final cycle: {before_final}, of which {} were garbage the slices had not freed",
        before_final - h.live()
    );
    println!("final full cycle (two finish_cycle calls, not in the slice statistics): {final_ms:.3} ms; live_objects {}", h.live());
}

/// Builds the stable set, runs the rounds in one mode, checks the result, and prints the statistics.
fn run(mode: Mode, live: usize, rounds: usize) -> Result<(), String> {
    let mut h = Harness::new(mode, live);
    let started = Instant::now();
    for round in 0..rounds {
        h.round(&SHAPE).map_err(|e| format!("round {round} at {live} ({mode:?}): {e}"))?;
    }
    let wall = started.elapsed().as_secs_f64();
    h.verify_stable().map_err(|e| format!("stable set at {live} ({mode:?}): {e}"))?;
    match mode {
        Mode::Cycle => {
            let freed: usize = h.pauses.iter().map(|p| p.1).sum();
            if freed + h.live() != h.allocated {
                return Err(format!("freed {freed} + live {} != allocated {}", h.live(), h.allocated));
            }
            print_cycle(&h, live, rounds, wall);
        }
        Mode::Slice | Mode::SlicePaced => {
            let before_final = h.live();
            let final_ms = h.finish();
            let want = h.stable_nodes + SHAPE.list;
            if h.live() != want {
                return Err(format!("live objects {} after the final cycle, expected {want}", h.live()));
            }
            print_slice(&h, live, rounds, wall, before_final, final_ms);
        }
    }
    Ok(())
}

fn main() {
    println!("gc_arena_churn: gc-arena 0.7.0 (pinned =0.7.0), the workload of gc_churn_bench, two collection modes");
    println!("machine: {}, {} logical CPUs", cpu_model(), std::thread::available_parallelism().map_or(0, |n| n.get()));
    println!("workload per round: {SHAPE:?}");
    println!("allocation threshold: {THRESHOLD} allocations (cycle mode)");
    println!(
        "slice modes: default pacing (sleep_factor {}) and a paced tuning run (sleep_factor {PACED_SLEEP_FACTOR})",
        Pacing::DEFAULT.sleep_factor
    );
    for (live, rounds) in RUNS {
        for mode in [Mode::Cycle, Mode::Slice, Mode::SlicePaced] {
            if let Err(e) = run(mode, live, rounds) {
                eprintln!("gc_arena_churn: {e}");
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same pieces as SHAPE at a size that runs in a debug build in well under a second.
    const SMALL: Shape = Shape {
        list: 40,
        rings: 3,
        ring_size: 5,
        width: 8,
        pairs: 6,
        closures: 6,
        natives: 6,
        promises: 4,
    };

    #[test]
    fn cycle_rounds_agree_with_the_expected_live_count() {
        let mut h = Harness::with_threshold(Mode::Cycle, 300, 64);
        for _ in 0..4 {
            assert_eq!(h.round(&SMALL).expect("the live count matches after each round"), 136);
        }
        h.verify_stable().expect("the stable set keeps its contents");
        assert!(h.automatic > 0, "the threshold must trigger collections during the rounds");
    }

    #[test]
    fn a_leaked_root_is_reported_by_the_live_count() {
        let mut h = Harness::with_threshold(Mode::Cycle, 300, 64);
        h.round(&SMALL).expect("a clean round");
        h.leak();
        let err = h.round(&SMALL).expect_err("the leaked object must be reported");
        assert!(err.contains("live objects"), "unexpected error: {err}");
    }

    #[test]
    fn a_dropped_stable_root_is_reported_by_the_live_count() {
        let mut h = Harness::with_threshold(Mode::Cycle, 300, 64);
        h.round(&SMALL).expect("a clean round");
        h.drop_stable_root();
        let err = h.round(&SMALL).expect_err("the dropped stable set must be reported");
        assert!(err.contains("live objects"), "unexpected error: {err}");
    }

    #[test]
    fn slice_modes_reach_the_expected_count_after_a_full_cycle() {
        for mode in [Mode::Slice, Mode::SlicePaced] {
            let mut h = Harness::with_threshold(mode, 300, 64);
            for _ in 0..4 {
                h.round(&SMALL).expect("slices do not change the object graph");
            }
            assert!(!h.slices.is_empty(), "{mode:?} must run at least one slice");
            h.finish();
            assert_eq!(h.live(), h.stable_nodes + SMALL.list, "{mode:?}");
            h.verify_stable().expect("the stable set keeps its contents");
        }
    }
}
