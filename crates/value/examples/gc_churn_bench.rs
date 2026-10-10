//! Collection pauses of the mark-sweep collector under graph churn (WP-3.2, WP-5.6).
//!
//!   cargo run --release -p vn-value --example gc_churn_bench
//!
//! For each live-set size, the stable set is built and then rounds of the churn workload (tests/support/churn.rs) run.
//! Every collection the rounds run is timed: the threshold collections at safepoints and the explicit collections at
//! the end of each round. Only the collector call is timed. The model check that follows each collection runs after the
//! timer stops, so a run that disagrees with the model stops with an error and prints no numbers. The numbers are from
//! one run on one machine. Nothing here asserts on a timing.

#[path = "../tests/support/churn.rs"]
mod churn;

use std::time::Instant;

use churn::{Churn, Shape};
use vn_value::Rt;

/// Live-set sizes (objects in the stable set) and the number of churn rounds run at each size.
const RUNS: [(usize, usize); 3] = [(10_000, 40), (100_000, 40), (1_000_000, 30)];

/// Allocations between threshold collections (`Rt::maybe_collect` at each safepoint of the workload).
const THRESHOLD: usize = 2_000;

/// The design's proposed collection-slice budget: p99 at most 2 ms under the reference workload (design.md, section 14).
const P99_BUDGET_MS: f64 = 2.0;

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

fn run(live: usize, rounds: usize) {
    let rt = Rt::new();
    rt.set_gc_threshold(THRESHOLD);
    let mut c = Churn::new(&rt);
    let st = churn::stable(&mut c, live).expect("building the stable set");

    let mut carry = None;
    let mut created = 0;
    let mut automatic = 0;
    let started = Instant::now();
    for round in 0..rounds {
        let stats = churn::round(&mut c, &SHAPE, &mut carry).unwrap_or_else(|e| panic!("round {round} at {live}: {e}"));
        created += stats.created;
        automatic += stats.automatic;
    }
    let wall = started.elapsed().as_secs_f64();
    churn::check_stable(&c, &st).unwrap_or_else(|e| panic!("stable set at {live}: {e}"));

    let pauses = std::mem::take(&mut c.pauses);
    let mut ms: Vec<f64> = pauses.iter().map(|p| p.0).collect();
    let mut freed: Vec<usize> = pauses.iter().map(|p| p.1).collect();
    ms.sort_by(|a, b| a.partial_cmp(b).expect("finite pause"));
    freed.sort_unstable();
    let total_ms: f64 = ms.iter().sum();
    let mean = total_ms / ms.len() as f64;
    let (p50, p95, p99) = (percentile(&ms, 0.50), percentile(&ms, 0.95), percentile(&ms, 0.99));
    let max = *ms.last().expect("at least one collection");
    let stats = rt.heap_stats();

    println!();
    println!("== live set {live} stable objects, {rounds} rounds, threshold {THRESHOLD} allocations ==");
    println!("objects created by the workload: {created}; realm baseline: {} live objects", c.baseline());
    println!(
        "collections: {} (threshold {automatic}, explicit {}); all checked against the model",
        ms.len(),
        ms.len() - automatic
    );
    println!("pause ms: p50 {p50:.3}  p95 {p95:.3}  p99 {p99:.3}  max {max:.3}  mean {mean:.3}");
    let none = freed.iter().filter(|&&f| f == 0).count();
    let mean_freed = freed.iter().sum::<usize>() as f64 / freed.len() as f64;
    println!(
        "objects freed per collection: min {}  p50 {}  p95 {}  max {}  mean {mean_freed:.0}; {none} of {} freed nothing",
        freed[0],
        freed[freed.len() / 2],
        freed[(freed.len() * 95 / 100).min(freed.len() - 1)],
        freed[freed.len() - 1],
        freed.len()
    );
    println!(
        "heap after the last collection: live_objects {}, bytes_estimate {}, total freed {}",
        stats.live_objects, stats.bytes_estimate, stats.freed_total
    );
    println!("time inside collections: {total_ms:.1} ms of {wall:.2} s wall (wall includes workload and model checks)");
    let verdict = if p99 <= P99_BUDGET_MS { "meets" } else { "does not meet" };
    println!("p99 {p99:.3} ms {verdict} the {P99_BUDGET_MS} ms collection-slice budget (design.md section 14)");
}

fn main() {
    println!("gc_churn_bench: vn-value mark-sweep, non-incremental, run once per size");
    println!("machine: {}, {} logical CPUs", cpu_model(), std::thread::available_parallelism().map_or(0, |n| n.get()));
    println!("workload per round: {SHAPE:?}");
    for (live, rounds) in RUNS {
        run(live, rounds);
    }
}
