//! Kernel garbage collection: reachability from every kind of root (global, intrinsics, persistent roots,
//! root providers, closure cells and data, array elements, native payloads, exotic hooks, extension slots),
//! stale-handle detection after slot reuse, the safepoint rule, and a seeded stress test that compares the
//! collector with an independent reachability model. Rust locals are not roots: each test only keeps an
//! `ObjRef` alive through one of the roots it names.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use vn_value::{Call, Exotic, Flags, FnInfo, FnKind, Intrinsic, NativeObject, ObjRef, PropKey, Res, Rt, RootProvider, Tracer, Value};

fn key(s: &str) -> PropKey {
    PropKey::from_str(s)
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

fn ok<T>(r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(_) => panic!("unexpected JS throw"),
    }
}

fn survives(rt: &Rt, o: ObjRef) -> bool {
    rt.is_live(o)
}

fn noop(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

fn collect_from_inside(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    rt.collect();
    Ok(Value::Undefined)
}

fn try_collect_from_inside(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(rt.try_collect()))
}

/// A host payload that holds one object reference and reports it through `trace`.
struct Holder {
    held: Cell<Option<ObjRef>>,
}

impl NativeObject for Holder {
    fn trace(&self, t: &mut Tracer) {
        if let Some(o) = self.held.get() {
            t.mark(o);
        }
    }
}

/// Exotic hooks that hold one reference; no hook overrides ordinary behaviour.
struct Hooks {
    held: Cell<Option<ObjRef>>,
}

impl Exotic for Hooks {
    fn trace(&self, t: &mut Tracer) {
        if let Some(o) = self.held.get() {
            t.mark(o);
        }
    }
}

/// A root provider with a list of roots. `forget` makes it skip tracing: the deliberate bug in the
/// negative controls.
struct Provider {
    list: RefCell<Vec<ObjRef>>,
    forget: bool,
}

impl RootProvider for Provider {
    fn trace_roots(&self, t: &mut Tracer) {
        if !self.forget {
            for o in self.list.borrow().iter() {
                t.mark(*o);
            }
        }
    }
}

// ----- roots -----

#[test]
fn an_unreachable_cycle_is_freed_by_collect() {
    let rt = Rt::new();
    rt.collect();
    let freed0 = rt.heap_stats().freed_total;
    let a = rt.new_object();
    let b = rt.new_object();
    ok(rt.define_data(a, key("other"), Value::Object(b), Flags::ALL));
    ok(rt.define_data(b, key("other"), Value::Object(a), Flags::ALL));
    rt.collect();
    assert!(!survives(&rt, a));
    assert!(!survives(&rt, b));
    assert_eq!(rt.heap_stats().freed_total, freed0 + 2);
}

#[test]
fn a_persistent_root_keeps_its_object_alive() {
    let rt = Rt::new();
    let o = rt.new_object();
    let id = rt.root(&Value::Object(o));
    rt.collect();
    assert!(survives(&rt, o));
    assert_eq!(rt.root_value(id), Value::Object(o));
}

#[test]
fn unrooting_makes_the_object_collectable() {
    let rt = Rt::new();
    let o = rt.new_object();
    let id = rt.root(&Value::Object(o));
    rt.collect();
    assert!(survives(&rt, o));
    rt.unroot(id);
    rt.collect();
    assert!(!survives(&rt, o));
}

#[test]
fn a_global_binding_keeps_its_object_alive() {
    let rt = Rt::new();
    let o = rt.new_object();
    rt.install_global("kept", Value::Object(o));
    rt.collect();
    assert!(survives(&rt, o));
}

#[test]
fn an_intrinsic_prototype_member_is_a_root() {
    let rt = Rt::new();
    let o = rt.new_object();
    let op = rt.intrinsic(Intrinsic::ObjectPrototype);
    ok(rt.define_data(op, key("shared"), Value::Object(o), Flags::ALL));
    rt.collect();
    assert!(survives(&rt, o));
}

#[test]
fn a_property_chain_survives_while_its_head_is_rooted() {
    let rt = Rt::new();
    let a = rt.new_object();
    let b = rt.new_object();
    let c = rt.new_object();
    ok(rt.define_data(a, key("next"), Value::Object(b), Flags::ALL));
    ok(rt.define_data(b, key("next"), Value::Object(c), Flags::ALL));
    let id = rt.root(&Value::Object(a));
    rt.collect();
    assert!(survives(&rt, a) && survives(&rt, b) && survives(&rt, c));
    rt.unroot(id);
    rt.collect();
    assert!(!survives(&rt, a) && !survives(&rt, b) && !survives(&rt, c));
}

#[test]
fn a_captured_cell_keeps_its_value_alive_and_frees_with_the_function() {
    let rt = Rt::new();
    let o = rt.new_object();
    let cell = rt.new_cell(Value::Object(o));
    let f = rt.new_function(noop, FnInfo::new("closure", 0, FnKind::Normal), &[cell]);
    let id = rt.root(&Value::Object(f));
    rt.collect();
    assert!(survives(&rt, cell) && survives(&rt, o));
    rt.unroot(id);
    rt.collect();
    assert!(!survives(&rt, f) && !survives(&rt, cell) && !survives(&rt, o));
}

#[test]
fn native_function_closure_data_keeps_its_objects_alive() {
    let rt = Rt::new();
    let o = rt.new_object();
    let f = rt.new_native_function(noop, "f", 0, vec![Value::Object(o)]);
    let id = rt.root(&Value::Object(f));
    rt.collect();
    assert!(survives(&rt, o));
    rt.unroot(id);
    rt.collect();
    assert!(!survives(&rt, o));
}

#[test]
fn array_elements_keep_their_objects_alive() {
    let rt = Rt::new();
    let o = rt.new_object();
    let arr = rt.new_array(vec![Value::Undefined, Value::Object(o)]);
    let id = rt.root(&Value::Object(arr));
    rt.collect();
    assert!(survives(&rt, o));
    rt.unroot(id);
    rt.collect();
    assert!(!survives(&rt, o) && !survives(&rt, arr));
}

#[test]
fn a_native_payload_trace_keeps_its_objects_alive() {
    let rt = Rt::new();
    let held = rt.new_object();
    let payload: Rc<dyn NativeObject> = Rc::new(Holder { held: Cell::new(Some(held)) });
    let host = rt.new_native_object(None, payload);
    let id = rt.root(&Value::Object(host));
    rt.collect();
    assert!(survives(&rt, held), "the payload's trace must mark `held`");
    rt.unroot(id);
    rt.collect();
    assert!(!survives(&rt, held) && !survives(&rt, host));
}

#[test]
fn a_native_payload_can_be_read_back_and_downcast() {
    let rt = Rt::new();
    let payload: Rc<dyn NativeObject> = Rc::new(Holder { held: Cell::new(None) });
    let host = rt.new_native_object(None, payload);
    let back = rt.native_payload(host).expect("native payload");
    assert!(vn_value::payload_ref::<Holder>(back.as_ref()).is_some());
    let plain = rt.new_object();
    assert!(rt.native_payload(plain).is_none());
}

#[test]
fn exotic_hook_trace_keeps_its_objects_alive() {
    let rt = Rt::new();
    let held = rt.new_object();
    let hooks: Rc<dyn Exotic> = Rc::new(Hooks { held: Cell::new(Some(held)) });
    let proxy = rt.new_exotic_object(None, hooks);
    let id = rt.root(&Value::Object(proxy));
    rt.collect();
    assert!(survives(&rt, held));
    rt.unroot(id);
    rt.collect();
    assert!(!survives(&rt, held));
}

#[test]
fn the_extension_slot_is_traced_and_dies_with_its_object() {
    let rt = Rt::new();
    let held = rt.new_object();
    let host = rt.new_object();
    rt.set_ext(host, Rc::new(Holder { held: Cell::new(Some(held)) }));
    assert!(rt.ext(host).is_some());
    let id = rt.root(&Value::Object(host));
    rt.collect();
    assert!(survives(&rt, held));
    rt.unroot(id);
    rt.collect();
    assert!(!survives(&rt, held) && !survives(&rt, host));
}

#[test]
fn a_root_provider_keeps_objects_alive_until_it_is_removed() {
    let rt = Rt::new();
    let o = rt.new_object();
    let p = Rc::new(Provider { list: RefCell::new(vec![o]), forget: false });
    let pid = rt.add_root_provider(p.clone());
    rt.collect();
    assert!(survives(&rt, o));
    rt.remove_root_provider(pid);
    rt.collect();
    assert!(!survives(&rt, o));
}

#[test]
fn positive_control_a_tracing_provider_keeps_its_objects() {
    let rt = Rt::new();
    let o = rt.new_object();
    let p = Rc::new(Provider { list: RefCell::new(vec![o]), forget: false });
    let _pid = rt.add_root_provider(p);
    rt.collect();
    assert!(survives(&rt, o), "the survival check reports a live object as live");
}

#[test]
fn negative_control_a_provider_that_forgets_to_trace_is_detected() {
    // The provider holds `o` but does not trace it. The collector frees `o`, and the survival check reports
    // that, so a test built on `survives` fails for this bug.
    let rt = Rt::new();
    let o = rt.new_object();
    let p = Rc::new(Provider { list: RefCell::new(vec![o]), forget: true });
    let _pid = rt.add_root_provider(p);
    rt.collect();
    let detected = !survives(&rt, o);
    assert!(detected, "the survival check must report the object the forgetful provider lost");
}

#[test]
fn a_persistent_root_table_keeps_other_roots_intact_when_ids_are_recycled() {
    let rt = Rt::new();
    let a = rt.new_object();
    let b = rt.new_object();
    let c = rt.new_object();
    let ra = rt.root(&Value::Object(a));
    let rb = rt.root(&Value::Object(b));
    rt.unroot(ra);
    let rc = rt.root(&Value::Object(c));
    assert_eq!(rt.root_value(rb), Value::Object(b));
    assert_eq!(rt.root_value(rc), Value::Object(c));
    rt.collect();
    assert!(survives(&rt, b) && survives(&rt, c));
    assert!(!survives(&rt, a));
}

#[test]
#[should_panic(expected = "unrooted twice")]
fn unrooting_the_same_id_twice_is_a_framework_bug() {
    let rt = Rt::new();
    let o = rt.new_object();
    let id = rt.root(&Value::Object(o));
    rt.unroot(id);
    rt.unroot(id);
}

#[test]
fn collect_frees_only_the_unreachable_part_of_a_mixed_graph() {
    let rt = Rt::new();
    let a = rt.new_object();
    let b = rt.new_object();
    let c = rt.new_object();
    let d = rt.new_object();
    let e = rt.new_object();
    let f = rt.new_object();
    let g = rt.new_object();
    ok(rt.define_data(a, key("n"), Value::Object(b), Flags::ALL));
    ok(rt.define_data(b, key("n"), Value::Object(c), Flags::ALL));
    ok(rt.define_data(d, key("back"), Value::Object(a), Flags::ALL));
    ok(rt.define_data(e, key("n"), Value::Object(f), Flags::ALL));
    ok(rt.define_data(f, key("n"), Value::Object(e), Flags::ALL));
    let _ = g;
    let id_a = rt.root(&Value::Object(a));
    let id_g = rt.root(&Value::Object(g));
    rt.collect();
    assert_eq!([survives(&rt, a), survives(&rt, b), survives(&rt, c)], [true, true, true]);
    assert_eq!([survives(&rt, d), survives(&rt, e), survives(&rt, f)], [false, false, false]);
    assert!(survives(&rt, g));
    rt.unroot(id_a);
    rt.unroot(id_g);
}

// ----- stale handles and the safepoint rule -----

#[test]
#[should_panic(expected = "stale ObjRef")]
fn a_stale_handle_panics_instead_of_reading_another_object() {
    let rt = Rt::new();
    let o = rt.new_object();
    rt.collect();
    assert!(!survives(&rt, o));
    let _ = rt.get(o, &key("x"), &Value::Undefined);
}

#[test]
fn a_reused_slot_does_not_revive_the_old_handle() {
    let rt = Rt::new();
    rt.collect();
    let a = rt.new_object();
    rt.collect();
    assert!(!survives(&rt, a));
    let b = rt.new_object();
    assert!(survives(&rt, b));
    assert_ne!(a, b, "the generation changes when a slot is freed");
    assert!(!survives(&rt, a), "the old handle stays dead after the slot is reused");
    ok(rt.define_data(b, key("who"), Value::str("b"), Flags::ALL));
    assert_eq!(ok(rt.get(b, &key("who"), &Value::Object(b))), Value::str("b"));
}

#[test]
#[should_panic(expected = "GC inside running code")]
fn collect_inside_running_code_is_a_framework_bug() {
    let rt = Rt::new();
    let f = rt.new_native_function(collect_from_inside, "c", 0, vec![]);
    let _ = rt.call(&Value::Object(f), &Value::Undefined, &[]);
}

#[test]
fn try_collect_is_refused_while_code_runs_and_allowed_at_depth_zero() {
    let rt = Rt::new();
    let f = rt.new_native_function(try_collect_from_inside, "t", 0, vec![]);
    assert_eq!(ok(rt.call(&Value::Object(f), &Value::Undefined, &[])), Value::Bool(false));
    assert!(rt.try_collect());
}

#[test]
fn maybe_collect_waits_for_the_allocation_threshold() {
    let rt = Rt::new();
    rt.collect();
    rt.set_gc_threshold(10);
    let before = rt.heap_stats().collections;
    for _ in 0..9 {
        rt.new_object();
    }
    assert!(!rt.maybe_collect(), "9 allocations are below the threshold of 10");
    rt.new_object();
    assert!(rt.maybe_collect(), "the 10th allocation reaches the threshold");
    assert_eq!(rt.heap_stats().collections, before + 1);
    assert!(!rt.maybe_collect(), "the counter restarts after a collection");
}

#[test]
fn heap_stats_count_allocations_frees_and_collections() {
    let rt = Rt::new();
    rt.collect();
    let s0 = rt.heap_stats();
    let objs: Vec<ObjRef> = (0..5).map(|_| rt.new_object()).collect();
    let s1 = rt.heap_stats();
    assert_eq!(s1.allocated_total, s0.allocated_total + 5);
    assert_eq!(s1.live_objects, s0.live_objects + 5);
    rt.collect();
    let s2 = rt.heap_stats();
    assert_eq!(s2.freed_total, s0.freed_total + 5);
    assert_eq!(s2.live_objects, s0.live_objects);
    assert_eq!(s2.collections, s0.collections + 1);
    assert!(objs.iter().all(|o| !survives(&rt, *o)));
}

#[test]
fn the_byte_estimate_grows_with_contents_and_returns_to_baseline() {
    let rt = Rt::new();
    rt.collect();
    let base = rt.heap_stats().bytes_estimate;
    let o = rt.new_object();
    let id = rt.root(&Value::Object(o));
    for i in 0..10 {
        ok(rt.define_data(o, key(&format!("p{i}")), num(f64::from(i)), Flags::ALL));
    }
    let grown = rt.heap_stats().bytes_estimate;
    assert!(grown > base, "properties add to the estimate");
    rt.unroot(id);
    rt.collect();
    assert_eq!(rt.heap_stats().bytes_estimate, base);
}

#[test]
fn a_deep_chain_is_marked_without_recursion() {
    // 200 000 links: a recursive marker would overflow the native stack.
    let rt = Rt::new();
    rt.collect();
    let before = rt.heap_stats().live_objects;
    let n = 200_000;
    let mut head = rt.new_object();
    for _ in 1..n {
        let o = rt.new_object();
        ok(rt.define_data(o, key("next"), Value::Object(head), Flags::ALL));
        head = o;
    }
    let id = rt.root(&Value::Object(head));
    rt.collect();
    assert_eq!(rt.heap_stats().live_objects, before + n);
    rt.unroot(id);
    rt.collect();
    assert_eq!(rt.heap_stats().live_objects, before);
}

// ----- randomized stress against an independent model -----

/// xorshift64*: a fixed seed gives a fixed sequence, so every run checks the same graphs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[derive(Clone)]
struct Node {
    handle: ObjRef,
    alive: bool,
    edges: [Option<usize>; 3],
}

/// The model's own view of the graph: nodes, edges and root nodes. Reachability is computed here, not by
/// the runtime.
#[derive(Default)]
struct Model {
    nodes: Vec<Node>,
    roots: Vec<usize>,
}

impl Model {
    fn alive_indices(&self) -> Vec<usize> {
        (0..self.nodes.len()).filter(|&i| self.nodes[i].alive).collect()
    }

    fn reachable(&self) -> Vec<bool> {
        let mut seen = vec![false; self.nodes.len()];
        let mut work = Vec::new();
        for &r in &self.roots {
            if !seen[r] {
                seen[r] = true;
                work.push(r);
            }
        }
        while let Some(n) = work.pop() {
            for e in self.nodes[n].edges.iter().flatten() {
                if !seen[*e] {
                    seen[*e] = true;
                    work.push(*e);
                }
            }
        }
        seen
    }
}

fn set_edge(rt: &Rt, m: &Model, p: usize, k: usize, target: Option<usize>) -> Result<(), String> {
    let v = match target {
        Some(t) => Value::Object(m.nodes[t].handle),
        None => Value::Undefined,
    };
    match rt.define_data(m.nodes[p].handle, key(&format!("e{k}")), v, Flags::ALL) {
        Ok(true) => Ok(()),
        Ok(false) => Err(format!("edge {k} of node {p} was refused")),
        Err(_) => Err(format!("edge {k} of node {p} threw")),
    }
}

/// Runs `steps` random operations. Returns the number of collections checked, or the first disagreement
/// between the runtime and the model. A disagreement stops the run before any freed handle is touched.
fn stress(seed: u64, steps: usize, forget_roots: bool) -> Result<(usize, usize), String> {
    let rt = Rt::new();
    rt.collect();
    let baseline = rt.heap_stats().live_objects;
    let provider = Rc::new(Provider { list: RefCell::new(Vec::new()), forget: forget_roots });
    let _pid = rt.add_root_provider(provider.clone());
    let mut rng = Rng(seed);
    let mut m = Model::default();
    let mut collections = 0usize;
    let mut died = 0usize;
    for _ in 0..steps {
        let pick = rng.below(100);
        let alive = m.alive_indices();
        if pick < 30 || alive.is_empty() {
            let parent = if alive.is_empty() { None } else { Some(alive[rng.below(alive.len())]) };
            let o = rt.new_object();
            let n = m.nodes.len();
            m.nodes.push(Node { handle: o, alive: true, edges: [None; 3] });
            if let Err(e) = rt.define_data(o, key("id"), num(n as f64), Flags::ALL).map(|_| ()).map_err(|_| "id threw".to_string()) {
                return Err(e);
            }
            match parent {
                Some(p) if rng.below(10) < 7 => {
                    let k = rng.below(3);
                    m.nodes[p].edges[k] = Some(n);
                    set_edge(&rt, &m, p, k, Some(n))?;
                }
                _ => {
                    m.roots.push(n);
                    provider.list.borrow_mut().push(o);
                }
            }
        } else if pick < 42 {
            let p = alive[rng.below(alive.len())];
            if !m.roots.contains(&p) {
                m.roots.push(p);
                provider.list.borrow_mut().push(m.nodes[p].handle);
            }
        } else if pick < 52 {
            if !m.roots.is_empty() {
                let i = rng.below(m.roots.len());
                m.roots.remove(i);
                provider.list.borrow_mut().remove(i);
            }
        } else if pick < 82 {
            let p = alive[rng.below(alive.len())];
            let k = rng.below(3);
            let target = if rng.below(10) < 3 { None } else { Some(alive[rng.below(alive.len())]) };
            m.nodes[p].edges[k] = target;
            set_edge(&rt, &m, p, k, target)?;
        } else {
            rt.collect();
            collections += 1;
            let reach = m.reachable();
            for i in 0..m.nodes.len() {
                if m.nodes[i].alive {
                    let live = rt.is_live(m.nodes[i].handle);
                    if live != reach[i] {
                        return Err(format!("collection {collections}: node {i} live={live}, model reachable={}", reach[i]));
                    }
                }
            }
            for i in 0..m.nodes.len() {
                if !reach[i] && m.nodes[i].alive {
                    m.nodes[i].alive = false;
                    died += 1;
                }
            }
            let expected = baseline + reach.iter().filter(|&&r| r).count();
            let got = rt.heap_stats().live_objects;
            if got != expected {
                return Err(format!("collection {collections}: live objects {got}, model expects {expected}"));
            }
            // Sample contents of reachable nodes: identity of `id` and every edge.
            for i in (0..m.nodes.len()).filter(|&i| reach[i]).take(24) {
                let h = m.nodes[i].handle;
                let hv = Value::Object(h);
                let got_id = rt.get(h, &key("id"), &hv).map_err(|_| "id read threw".to_string())?;
                if got_id != num(i as f64) {
                    return Err(format!("node {i} has id {got_id:?}"));
                }
                for k in 0..3 {
                    let want = match m.nodes[i].edges[k] {
                        Some(t) => Value::Object(m.nodes[t].handle),
                        None => Value::Undefined,
                    };
                    let got_e = rt.get(h, &key(&format!("e{k}")), &hv).map_err(|_| "edge read threw".to_string())?;
                    if got_e != want {
                        return Err(format!("node {i} edge {k} differs from the model"));
                    }
                }
            }
        }
    }
    Ok((collections, died))
}

#[test]
fn random_graphs_agree_with_the_reference_model() {
    for seed in [0x5EED_u64, 0xC0FF_EE01, 0x1234_5678_9ABC] {
        let (collections, died) = stress(seed, 3000, false).unwrap_or_else(|e| panic!("seed {seed:#x}: {e}"));
        assert!(collections > 100, "seed {seed:#x} ran only {collections} collections");
        assert!(died > 100, "seed {seed:#x} freed only {died} objects: the graphs did not churn");
    }
}

#[test]
fn negative_control_a_stress_run_with_a_forgetful_root_provider_fails() {
    // Same harness with the root provider not tracing: the first collection must disagree with the model.
    let result = stress(0x5EED, 3000, true);
    assert!(result.is_err(), "the stress harness must detect roots that are not traced");
}
