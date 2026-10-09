// Realm setup of scenario binaries (WP-C1c follow-up): the setup runs on the fresh realm before the entry evaluates the
// module (so a native module can be registered in time), and a run without setup does not see it (negative control).
use std::sync::atomic::{AtomicBool, Ordering};

use vn_testkit::{run_scenario_isolated, run_scenario_isolated_with_setup, ScenarioEntry};
use vn_value::{Res, Rt, Throw, Value};

static WITH_SETUP: AtomicBool = AtomicBool::new(false);
static WITHOUT_SETUP: AtomicBool = AtomicBool::new(false);

// Entries succeed only when their own flag has been set (WITHOUT_SETUP is never set: no setup is passed to that run).
fn entry_with_setup(_rt: &Rt) -> Res<ScenarioEntry> {
    if WITH_SETUP.load(Ordering::SeqCst) {
        Ok(ScenarioEntry { run: Value::Undefined })
    } else {
        Err(Throw(Value::str("setup did not run before evaluation")))
    }
}

fn entry_without_setup(_rt: &Rt) -> Res<ScenarioEntry> {
    if WITHOUT_SETUP.load(Ordering::SeqCst) {
        Ok(ScenarioEntry { run: Value::Undefined })
    } else {
        Err(Throw(Value::str("setup did not run before evaluation")))
    }
}

fn set_with_setup(_rt: &Rt) {
    WITH_SETUP.store(true, Ordering::SeqCst);
}

#[test]
fn setup_runs_before_module_evaluation() {
    let doc = run_scenario_isolated_with_setup("setup-probe", entry_with_setup, set_with_setup);
    assert!(doc.contains("\"result\":\"ok\""), "expected ok, got {doc}");
}

#[test]
fn negative_control_without_setup_throws() {
    let doc = run_scenario_isolated("no-setup-probe", entry_without_setup);
    assert!(doc.contains("\"result\":\"throw\""), "expected throw, got {doc}");
    assert!(doc.contains("setup did not run"), "expected the probe message, got {doc}");
}
