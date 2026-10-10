//! Scenario runner: steps 1-5 of the scenario contract (trace-format.md) in a fresh realm, the isolated runner
//! (a large stack, and panics become documents), and `scenario_main`, the entry point of a generated binary.

use std::any::Any;
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

use vn_value::rt::DEFAULT_GC_THRESHOLD;
use std::future::Future;
use std::pin::Pin;

use vn_value::builtins::promise::{async_start, await_value, frame_get, frame_set, new_frame, promise_state, PromiseState};
use vn_value::{ObjRef, PropKey, RootId, Res, Rt, Throw, Value};

use crate::host::install_host;
use crate::json::Json;
use crate::trace::{encode_pass, encode_standalone, Recorder};

/// Stack of the thread a scenario runs on, so deep JavaScript recursion cannot overflow the Rust stack.
const STACK_BYTES: usize = 256 * 1024 * 1024;

/// What a generated entry returns after evaluating the scenario module: its `run` export, or `Undefined`.
pub struct ScenarioEntry {
    pub run: Value,
}

/// A generated scenario entry. It evaluates the module in the given realm; its throw is `result: "throw"`.
pub type ScenarioFn = fn(&Rt) -> Res<ScenarioEntry>;

/// Runs on the fresh realm after the host globals are installed and before the entry evaluates the module. A generated
/// binary uses it to install its native modules (for example vn-runtime's "vue" module, WP-C1c follow-up).
pub type RealmSetup = fn(&Rt);

fn no_setup(_rt: &Rt) {}

/// Runs one scenario on the current thread and returns its document: one line of compact JSON.
pub fn run_scenario(name: &str, entry: ScenarioFn) -> String {
    run_scenario_with_gc_threshold(name, entry, DEFAULT_GC_THRESHOLD)
}

/// Like `run_scenario`, with `threshold` allocations between the collections at the safepoints (the kernel
/// default is 10 000; 1 collects at every safepoint). The document must not depend on the threshold.
pub fn run_scenario_with_gc_threshold(name: &str, entry: ScenarioFn, threshold: usize) -> String {
    let rec = Rc::new(RefCell::new(Recorder::new()));
    scenario_document(name, entry, no_setup, threshold, &rec)
}

/// Runs a scenario on a fresh thread with a 256 MiB stack and joins it. A panic (a framework bug) becomes a
/// document with `result: "panic"` and the panic message, plus the entries recorded before the panic.
pub fn run_scenario_isolated(name: &str, entry: ScenarioFn) -> String {
    run_scenario_isolated_with_setup(name, entry, no_setup)
}

/// Like `run_scenario_isolated`, with `setup` run on the fresh realm first (see RealmSetup).
pub fn run_scenario_isolated_with_setup(name: &str, entry: ScenarioFn, setup: RealmSetup) -> String {
    run_scenario_isolated_with_gc_threshold(name, entry, setup, DEFAULT_GC_THRESHOLD)
}

/// Like `run_scenario_isolated_with_setup`, with `threshold` (see `run_scenario_with_gc_threshold`). The scenario binary
/// takes it from VN_GC_THRESHOLD (task 4.2: a stress run uses 1).
fn run_scenario_isolated_with_gc_threshold(name: &str, entry: ScenarioFn, setup: RealmSetup, threshold: usize) -> String {
    let owned = name.to_string();
    let worker = std::thread::Builder::new().stack_size(STACK_BYTES).spawn(move || {
        let rec = Rc::new(RefCell::new(Recorder::new()));
        match catch_unwind(AssertUnwindSafe(|| scenario_document(&owned, entry, setup, threshold, &rec))) {
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

/// Like `run_scenario_isolated_with_setup`, on the current thread. The wasm32-unknown-unknown target has no threads, so
/// the wasm entry (`crate::wasm`) runs each scenario here, in its own instance. The document is the same. A panic becomes
/// a `panic` document only where panics unwind: that target aborts (panic = "abort"), so a panic there is a trap.
pub fn run_scenario_on_current_thread_with_setup(name: &str, entry: ScenarioFn, setup: RealmSetup) -> String {
    run_scenario_on_current_thread_with_gc_threshold(name, entry, setup, DEFAULT_GC_THRESHOLD)
}

/// Like `run_scenario_on_current_thread_with_setup`, with `threshold` (see `run_scenario_with_gc_threshold`). The wasm
/// entry takes it from the host (task 4.2: a stress run uses 1).
pub fn run_scenario_on_current_thread_with_gc_threshold(
    name: &str,
    entry: ScenarioFn,
    setup: RealmSetup,
    threshold: usize,
) -> String {
    let rec = Rc::new(RefCell::new(Recorder::new()));
    match catch_unwind(AssertUnwindSafe(|| scenario_document(name, entry, setup, threshold, &rec))) {
        Ok(doc) => doc,
        Err(payload) => panic_document(name, &payload_text(payload), &rec),
    }
}

/// The collection threshold of a scenario binary: `VN_GC_THRESHOLD` when set (task 4.2: 1 collects at every safepoint, the
/// stress run of the language scenarios), else the kernel default. A value that is not a number exits 2.
fn gc_threshold_from_env() -> usize {
    let text = match std::env::var("VN_GC_THRESHOLD") {
        Ok(text) => text,
        Err(std::env::VarError::NotPresent) => return DEFAULT_GC_THRESHOLD,
        Err(e) => {
            eprintln!("vn-testkit: VN_GC_THRESHOLD: {e}");
            std::process::exit(2)
        }
    };
    text.trim().parse().unwrap_or_else(|_| {
        eprintln!("vn-testkit: VN_GC_THRESHOLD must be a number of allocations, not {text:?}");
        std::process::exit(2)
    })
}

/// Process entry point of a generated scenario binary. `argv[1]` names the scenario, and `--list` prints the
/// names. The document is printed as one line and the process exits 0, even for `result: "throw"`.
pub fn scenario_main(table: &[(&str, ScenarioFn)]) {
    scenario_main_with_setup(table, no_setup);
}

/// Like `scenario_main`, with `setup` run on the fresh realm of every scenario (see RealmSetup).
pub fn scenario_main_with_setup(table: &[(&str, ScenarioFn)], setup: RealmSetup) {
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
            Some((n, entry)) => {
                let threshold = gc_threshold_from_env();
                println!("{}", run_scenario_isolated_with_gc_threshold(n, *entry, setup, threshold));
            }
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
fn scenario_document(name: &str, entry: ScenarioFn, setup: RealmSetup, threshold: usize, rec: &Rc<RefCell<Recorder>>) -> String {
    match run_steps(name, entry, setup, threshold, rec) {
        Ok(doc) => doc,
        Err(message) => panic_document(name, &message, rec),
    }
}

/// Steps 1-5. An `Err` is a harness failure: the run cannot be reported faithfully, so the caller reports a panic.
fn run_steps(name: &str, entry: ScenarioFn, setup: RealmSetup, threshold: usize, rec: &Rc<RefCell<Recorder>>) -> Result<String, String> {
    // Step 1: a fresh realm with the host globals installed, then the caller's setup (native modules).
    let rt = Rt::new();
    rt.set_gc_threshold(threshold);
    let _clock = install_host(&rt, rec);
    setup(&rt);

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

    // Step 4: call `run`. A sync throw is the result. A returned value is awaited the way the oracle awaits it (`await p`),
    // so a rejection is handled; the outcome is read after the queue drains. A value that never settles is a harness failure.
    if let Some(root) = run_root {
        let run = rt.root_value(root);
        rt.unroot(root); // nothing can collect between this line and the call
        match rt.call(&run, &Value::Undefined, &[]) {
            Ok(returned) => {
                let holder_root = await_returned(&rt, returned);
                drain(&rt)?;
                let holder = rt.root_value(holder_root);
                rt.unroot(holder_root);
                if !slot_is(&rt, &holder, 2) {
                    return Err("the value returned by run never settled before the job queue emptied".to_string());
                }
                if slot_is(&rt, &holder, 1) {
                    let reason = rt.get_v(&holder, &PropKey::Index(0)).expect("the holder is an array");
                    result = "throw";
                    error = encode_thrown(&rt, rec, &reason)?;
                }
            }
            Err(Throw(thrown)) => {
                result = "throw";
                error = encode_thrown(&rt, rec, &thrown)?;
            }
        }
        drain(&rt)?;
    }

    // Step 5: the final drain, then the rejections still unhandled, in rejection order (one encoding pass).
    drain(&rt)?;
    let unhandled = unhandled_reasons(&rt, rec)?;
    let entries = rec.borrow().entries().to_vec();
    Ok(document(name, result, error, entries, unhandled))
}

/// `await v` for the value `run` returned. The await runs in a host async body (`async_start`), so its handler is in
/// place before the queue runs, as the oracle's `await p` is. The body fills a rooted holder `[reason, rejected, finished]`.
fn await_returned(rt: &Rt, returned: Value) -> RootId {
    let holder = Value::Object(rt.new_array(vec![Value::Undefined, Value::Bool(false), Value::Bool(false)]));
    let root = rt.root(&holder);
    let frame = new_frame(rt, 2);
    frame_set(rt, frame, 0, holder);
    frame_set(rt, frame, 1, returned);
    let body: Pin<Box<dyn Future<Output = Res<Value>>>> = Box::pin(await_body(rt.clone(), frame));
    let _ = async_start(rt, frame, body);
    root
}

/// The body of `await_returned`: await the value, then record the outcome in the holder (read from the frame after the await).
async fn await_body(rt: Rt, frame: ObjRef) -> Res<Value> {
    let value = frame_get(&rt, frame, 1);
    let outcome = await_value(&rt, value).await;
    let holder = frame_get(&rt, frame, 0);
    if let Err(Throw(reason)) = outcome {
        rt.set_v_strict(&holder, &PropKey::Index(0), reason)?;
        rt.set_v_strict(&holder, &PropKey::Index(1), Value::Bool(true))?;
    }
    rt.set_v_strict(&holder, &PropKey::Index(2), Value::Bool(true))?;
    Ok(Value::Undefined)
}

fn slot_is(rt: &Rt, holder: &Value, i: u32) -> bool {
    matches!(rt.get_v(holder, &PropKey::Index(i)), Ok(Value::Bool(true)))
}

/// The reasons of the promises still unhandled, in rejection order (`Rt::take_unhandled_rejections`), encoded in one pass.
fn unhandled_reasons(rt: &Rt, rec: &Rc<RefCell<Recorder>>) -> Result<Vec<Json>, String> {
    let reasons: Vec<Value> = rt
        .take_unhandled_rejections()
        .iter()
        .filter_map(|p| match promise_state(rt, p) {
            Some(PromiseState::Rejected(reason)) => Some(reason),
            _ => None,
        })
        .collect();
    encode_pass(rt, rec, &reasons).map_err(|Throw(e)| describe(rt, &e))
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
