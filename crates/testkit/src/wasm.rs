//! The entry of the wasm32-unknown-unknown scenario build (task 4.2). The harness (tooling/harness/build.mjs,
//! writeWasmCrate) writes a cdylib with the scenario modules of the native binary and a table of `(name, entry)`. Its
//! `#[unsafe(no_mangle)]` exports forward to the functions below, which are safe code: this crate keeps forbid(unsafe_code).
//!
//! The host calls one scenario per instance. It reads the result from linear memory: the buffer at `out_ptr()` holds
//! the last name or document, and its length is the value the call returned. The buffer is valid until the next call.

use std::cell::RefCell;

use vn_value::rt::DEFAULT_GC_THRESHOLD;

use crate::runner::{run_scenario_on_current_thread_with_gc_threshold, RealmSetup, ScenarioFn};

/// The result of `scenario_name` and `run_scenario` for an index outside the table.
pub const NO_SUCH_SCENARIO: u32 = u32::MAX;

thread_local! {
    /// The bytes of the last result (a name or a document). Each call rewrites it.
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Number of scenarios in `table`.
pub fn scenario_count(table: &[(&str, ScenarioFn)]) -> u32 {
    table.len() as u32
}

/// Stores the name of scenario `i` in the output buffer and returns its length.
pub fn scenario_name(table: &[(&str, ScenarioFn)], i: u32) -> u32 {
    match table.get(i as usize) {
        Some((name, _)) => store(name.as_bytes()),
        None => NO_SUCH_SCENARIO,
    }
}

/// Runs scenario `i` in a fresh realm (with `setup`) and stores its document in the output buffer: one line of compact
/// JSON, the text `scenario_main` prints natively. Returns the length of the document.
pub fn run_scenario(table: &[(&str, ScenarioFn)], i: u32, setup: RealmSetup) -> u32 {
    run_scenario_with_gc_threshold(table, i, DEFAULT_GC_THRESHOLD as u32, setup)
}

/// Like `run_scenario`, with `threshold` allocations between the collections at the safepoints (1 collects at every
/// safepoint: the stress run of task 4.2). The document must not depend on the threshold.
pub fn run_scenario_with_gc_threshold(table: &[(&str, ScenarioFn)], i: u32, threshold: u32, setup: RealmSetup) -> u32 {
    match table.get(i as usize) {
        Some((name, entry)) => {
            let doc = run_scenario_on_current_thread_with_gc_threshold(name, *entry, setup, threshold as usize);
            store(doc.as_bytes())
        }
        None => NO_SUCH_SCENARIO,
    }
}

/// Address of the output buffer in linear memory. Read it after the call that filled the buffer.
pub fn out_ptr() -> u32 {
    OUT.with(|out| out.borrow().as_ptr() as usize as u32)
}

/// A copy of the output buffer, for native tests (the host reads linear memory instead).
pub fn output() -> Vec<u8> {
    OUT.with(|out| out.borrow().clone())
}

fn store(bytes: &[u8]) -> u32 {
    OUT.with(|out| {
        let mut out = out.borrow_mut();
        out.clear();
        out.extend_from_slice(bytes);
    });
    bytes.len() as u32
}
