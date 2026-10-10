//! Collector correctness under graph churn (WP-3.2, WP-5.6). The workload in support/churn.rs builds lists, cycles, wide
//! objects, Maps and Sets holding objects, closures over cells, native functions, promise reactions and queued jobs, and
//! drops them. After every collection the runtime must agree with the model: the same reachable objects are live, the
//! rest are freed, and the live-object count is the realm's baseline plus the reachable objects. The stable set and the
//! pinned list keep their contents. No timing is asserted here; examples/gc_churn_bench.rs measures pauses.

#[path = "support/churn.rs"]
mod churn;

use churn::{Churn, Shape};
use vn_value::{Rt, Value};

const SHAPE: Shape = Shape {
    list: 400,
    rings: 20,
    ring_size: 12,
    width: 64,
    pairs: 60,
    closures: 80,
    natives: 80,
    promises: 40,
};

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
fn rounds_of_churn_agree_with_the_model_and_reach_a_steady_heap() {
    let rt = Rt::new();
    rt.set_gc_threshold(256);
    let mut c = Churn::new(&rt);
    let st = churn::stable(&mut c, 500).expect("build the stable set");
    churn::check_stable(&c, &st).expect("the stable set after it is built");
    let mut carry = None;
    let mut steady = None;
    let mut automatic = 0;
    for round in 0..8 {
        let stats = churn::round(&mut c, &SHAPE, &mut carry).unwrap_or_else(|e| panic!("round {round}: {e}"));
        churn::check_stable(&c, &st).unwrap_or_else(|e| panic!("round {round}: stable set: {e}"));
        assert!(stats.automatic > 0, "round {round} ran no collection at a threshold safepoint");
        assert!(stats.created >= SHAPE.list + SHAPE.rings * SHAPE.ring_size + SHAPE.closures + SHAPE.natives);
        automatic += stats.automatic;
        let s = rt.heap_stats();
        let now = (s.live_objects, s.bytes_estimate);
        match steady {
            None => steady = Some(now),
            Some(first) => assert_eq!(now, first, "round {round} left a different heap than the first round"),
        }
    }
    // Each round makes three explicit collections (after the queued job, after it drains, at the end of the round).
    // Every collection that ran, explicit or at a threshold safepoint, was checked against the model.
    assert_eq!(c.collections(), 8 * 3 + automatic as u64);
    assert_eq!(rt.heap_stats().live_objects, c.baseline() + c.live_nodes(), "model and runtime agree after the last round");
}

#[test]
fn a_pinned_list_keeps_its_contents_through_churn_and_is_freed_once_unpinned() {
    let rt = Rt::new();
    rt.set_gc_threshold(128);
    let mut c = Churn::new(&rt);
    let head = c.list(300).expect("build the list");
    let pin = c.pin(head);
    let mut carry = None;
    for _ in 0..3 {
        churn::round(&mut c, &SHAPE, &mut carry).expect("a round");
        churn::check_list(&c, head, 300).expect("the pinned list keeps its contents");
    }
    if let Some(id) = carry.take() {
        c.unpin(id);
    }
    c.unpin(pin);
    c.collect().expect("nothing pins the list any more");
    assert!(!rt.is_live(c.handle(head)), "the unpinned list head is freed");
    assert_eq!(c.live_nodes(), 0);
}

#[test]
fn queued_jobs_keep_their_arguments_until_the_queue_drains() {
    let rt = Rt::new();
    let mut c = Churn::new(&rt);
    let x = c.object();
    c.enqueue_mark(x);
    c.collect().expect("a queued argument is a root");
    assert!(rt.is_live(c.handle(x)));
    assert_eq!(c.drain(), 1, "one job was queued");
    assert_eq!(rt.get_str(&c.value(x), "ran").expect("read the mark"), Value::Bool(true));
    c.collect().expect("a drained argument is garbage");
    assert!(!rt.is_live(c.handle(x)));
}

#[test]
fn each_reaction_runs_once_and_its_result_is_collected_with_the_promises() {
    let rt = Rt::new();
    let mut c = Churn::new(&rt);
    let v = c.object();
    let p = c.fulfilled(v).expect("a fulfilled promise");
    let h1 = c.handler(true);
    let q1 = c.then(p, h1).expect("then");
    let h2 = c.handler(false);
    let q2 = c.then(p, h2).expect("then");
    assert_eq!(c.drain(), 2, "two reactions on a settled promise queue two jobs, each run once");
    c.settle(q1).expect("settle q1");
    c.settle(q2).expect("settle q2");
    churn::check_promises(&c, &[(q1, v, true), (q2, v, false)]).expect("results");
    let reachable = c.collect().expect("nothing holds the promises or their results");
    assert_eq!(reachable, 0);
    assert_eq!(c.live_nodes(), 0);
}

#[test]
fn collecting_at_every_safepoint_agrees_with_the_model() {
    // Threshold 1: every operation of the round ends in a collection that is checked against the model.
    let rt = Rt::new();
    rt.set_gc_threshold(1);
    let mut c = Churn::new(&rt);
    let st = churn::stable(&mut c, 30).expect("stable set");
    let mut carry = None;
    for round in 0..3 {
        let stats = churn::round(&mut c, &SMALL, &mut carry).unwrap_or_else(|e| panic!("round {round}: {e}"));
        // Eight operations allocate and so reach a collection. The detach at the end allocates nothing, so it does not.
        assert_eq!(stats.automatic, 8, "round {round} collected at each allocating safepoint");
        churn::check_stable(&c, &st).unwrap_or_else(|e| panic!("round {round}: stable set: {e}"));
    }
}

#[test]
fn negative_control_a_root_dropped_behind_the_models_back_is_detected() {
    let rt = Rt::new();
    let mut c = Churn::new(&rt);
    let st = churn::stable(&mut c, 50).expect("stable set");
    rt.unroot(st.root); // the model still holds this root
    let err = c.collect().expect_err("the model must report reachable objects that were freed");
    assert!(err.contains("reachable from a root but the collector freed it"), "unexpected report: {err}");
}

#[test]
fn negative_control_an_untracked_root_is_reported_by_the_live_count() {
    let rt = Rt::new();
    let mut c = Churn::new(&rt);
    let _st = churn::stable(&mut c, 50).expect("stable set");
    let stray = rt.new_object();
    let _id = rt.root(&Value::Object(stray)); // the model never hears of this object
    let err = c.collect().expect_err("the live count must disagree with the model");
    assert!(err.contains("model expects"), "unexpected report: {err}");
}
