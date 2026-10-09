//! Scenario runner: steps 1-5 of the scenario contract (trace-format.md) in a fresh realm, the isolated runner
//! (a large stack, and panics become documents), and `scenario_main`, the entry point of a generated binary.

use std::any::Any;
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

use vn_value::rt::DEFAULT_GC_THRESHOLD;
use vn_value::{Res, Rt, Throw, Value};

use crate::host::install_host;
use crate::json::Json;
use crate::trace::{encode_standalone, Recorder};

/// Stack of the thread a scenario runs on, so deep JavaScript recursion cannot overflow the Rust stack.
const STACK_BYTES: usize = 256 * 1024 * 1024;

/// What a generated entry returns after evaluating the scenario module: its `run` export, or `Undefined`.
pub struct ScenarioEntry {
    pub run: Value,
}

/// A generated scenario entry. It evaluates the module in the given realm; its throw is `result: "throw"`.
pub type ScenarioFn = fn(&Rt) -> Res<ScenarioEntry>;

/// Runs one scenario on the current thread and returns its document: one line of compact JSON.
pub fn run_scenario(name: &str, entry: ScenarioFn) -> String {
    run_scenario_with_gc_threshold(name, entry, DEFAULT_GC_THRESHOLD)
}

/// Like `run_scenario`, with `threshold` allocations between the collections at the safepoints (the kernel
/// default is 10 000; 1 collects at every safepoint). The document must not depend on the threshold.
pub fn run_scenario_with_gc_threshold(name: &str, entry: ScenarioFn, threshold: usize) -> String {
    let rec = Rc::new(RefCell::new(Recorder::new()));
    scenario_document(name, entry, threshold, &rec)
}

/// Runs a scenario on a fresh thread with a 256 MiB stack and joins it. A panic (a framework bug) becomes a
/// document with `result: "panic"` and the panic message, plus the entries recorded before the panic.
pub fn run_scenario_isolated(name: &str, entry: ScenarioFn) -> String {
    let owned = name.to_string();
    let worker = std::thread::Builder::new().stack_size(STACK_BYTES).spawn(move || {
        let rec = Rc::new(RefCell::new(Recorder::new()));
        match catch_unwind(AssertUnwindSafe(|| scenario_document(&owned, entry, DEFAULT_GC_THRESHOLD, &rec))) {
            Ok(doc) => doc,
            Err(payload) => panic_document(&owned, &payload_text(payload), &rec),
        }
    });
    match worker {
        Err(e) => {
            let empty = Rc::new(RefCell::new(Recorder::new()));
            panic_document(name, &format!("cannot start the scenario thread: {e}"), &empty)
        }
        Ok(handle) => match handle.join() {
            Ok(doc) => doc,
            Err(payload) => {
                let empty = Rc::new(RefCell::new(Recorder::new()));
                panic_document(name, &payload_text(payload), &empty)
            }
        },
    }
}

/// Process entry point of a generated scenario binary. `argv[1]` names the scenario, and `--list` prints the
/// names. The document is printed as one line and the process exits 0, even for `result: "throw"`.
pub fn scenario_main(table: &[(&str, ScenarioFn)]) {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--list") => {
            for (name, _) in table {
                println!("{name}");
            }
        }
        // Hidden: replays a builtin corpus suite and prints the report. Exits 1 when a case fails.
        Some("--replay") => {
            let Some(path) = args.get(2) else {
                eprintln!("usage: <binary> --replay <suite.json>");
                std::process::exit(2);
            };
            let text = match std::fs::read_to_string(path) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("vn-testkit: cannot read {path}: {e}");
                    std::process::exit(2);
                }
            };
            let report = crate::replay::replay_suite(&text, &crate::replay::ReplayOptions::default());
            println!("{}", report.to_json());
            if !report.failures.is_empty() {
                std::process::exit(1);
            }
        }
        Some(name) => match table.iter().find(|(n, _)| *n == name) {
            Some((n, entry)) => println!("{}", run_scenario_isolated(n, *entry)),
            None => {
                eprintln!("vn-testkit: unknown scenario {name:?} (try --list)");
                std::process::exit(2);
            }
        },
        None => {
            eprintln!("usage: <binary> <scenario> | --list");
            std::process::exit(2);
        }
    }
}

/// Steps 1-5 with a recorder owned by the caller, so a panic can still report the entries recorded so far.
fn scenario_document(name: &str, entry: ScenarioFn, threshold: usize, rec: &Rc<RefCell<Recorder>>) -> String {
    match run_steps(name, entry, threshold, rec) {
        Ok(doc) => doc,
        Err(message) => panic_document(name, &message, rec),
    }
}

/// Steps 1-5. An `Err` is a harness failure: the run cannot be reported faithfully, so the caller reports a panic.
fn run_steps(name: &str, entry: ScenarioFn, threshold: usize, rec: &Rc<RefCell<Recorder>>) -> Result<String, String> {
    // Step 1: a fresh realm with the host globals installed.
    let rt = Rt::new();
    rt.set_gc_threshold(threshold);
    let _clock = install_host(&rt, rec);

    // Step 2: evaluate the module. A throw is the result. The `run` export is rooted until it is called,
    // because the drains below may collect garbage.
    let mut result = "ok";
    let mut error = Json::Null;
    let mut run_root = None;
    match entry(&rt) {
        Err(Throw(thrown)) => {
            result = "throw";
            error = encode_thrown(&rt, rec, &thrown)?;
        }
        Ok(ScenarioEntry { run }) => {
            if !matches!(run, Value::Undefined) {
                if !rt.is_callable(&run) {
                    return Err("the scenario export \"run\" is not a function".to_string());
                }
                run_root = Some(rt.root(&run));
            }
        }
    }

    // Step 3: drain the jobs.
    drain(&rt)?;

    // Step 4: call `run`. Its throw is the result. Its return value is ignored until promises exist
    // (TODO(WP-B-promise)): a returned promise would be awaited here.
    if let Some(root) = run_root {
        let run = rt.root_value(root);
        rt.unroot(root); // nothing can collect between this line and the call
        match rt.call(&run, &Value::Undefined, &[]) {
            Ok(_) => {}
            Err(Throw(thrown)) => {
                result = "throw";
                error = encode_thrown(&rt, rec, &thrown)?;
            }
        }
        drain(&rt)?;
    }

    // Step 5: the final drain. unhandledRejections stays empty until promises exist (TODO(WP-B-promise)).
    drain(&rt)?;
    let entries = rec.borrow().entries().to_vec();
    Ok(document(name, result, error, entries, Vec::new()))
}

/// A safepoint between jobs: drain the queue, refuse uncaught job errors (no host job throws), and collect when due.
fn drain(rt: &Rt) -> Result<(), String> {
    rt.run_jobs();
    if !rt.take_uncaught_job_errors().is_empty() {
        return Err("the runtime reported an uncaught job error (framework bug)".to_string());
    }
    rt.maybe_collect();
    Ok(())
}

/// Encodes a thrown value for the document's `error`. A value that cannot be encoded is a harness failure.
fn encode_thrown(rt: &Rt, rec: &Rc<RefCell<Recorder>>, thrown: &Value) -> Result<Json, String> {
    encode_standalone(rt, rec, thrown).map_err(|Throw(e)| describe(rt, &e))
}

/// Best-effort text of a value, for harness failure messages.
fn describe(rt: &Rt, v: &Value) -> String {
    match rt.to_string(v) {
        Ok(s) => s.to_rust_string_lossy(),
        Err(_) => "<unprintable value>".to_string(),
    }
}

/// The one-line document (trace-format.md, "Output document"), with the keys in the oracle's order.
fn document(name: &str, result: &str, error: Json, entries: Vec<Json>, unhandled: Vec<Json>) -> String {
    Json::obj([
        ("scenario", Json::text(name)),
        ("result", Json::text(result)),
        ("error", error),
        ("entries", Json::Arr(entries)),
        ("unhandledRejections", Json::Arr(unhandled)),
    ])
    .to_json()
}

/// A framework-bug document: `error` holds the message as plain text (not an encoded value).
fn panic_document(name: &str, message: &str, rec: &RefCell<Recorder>) -> String {
    let entries = match rec.try_borrow() {
        Ok(r) => r.entries().to_vec(),
        Err(_) => Vec::new(),
    };
    document(name, "panic", Json::text(message), entries, Vec::new())
}

fn payload_text(payload: Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        return (*s).to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    "panic with a non-string payload".to_string()
}
