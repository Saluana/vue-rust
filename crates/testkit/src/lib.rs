//! vn-testkit: the Rust side of the differential tests (WP-T1).
//!
//! * `trace` encodes runtime values exactly as the Node oracle does (docs/dev/trace-format.md v1.1 and
//!   tooling/oracle/trace.mjs): the same ordinals, refs, key order and JSON text.
//! * `host` installs the scenario globals `trace` and `testHost` (deterministic fake clock).
//! * `runner` runs one scenario in a fresh realm (steps 1-5 of the scenario contract) and provides
//!   `scenario_main`, the process entry point of generated scenario binaries.
//!
//! The crate depends on the kernel (`vn-value`) only.

#![forbid(unsafe_code)]

pub mod decode;
pub mod host;
pub mod json;
pub mod json_in;
pub mod presets;
pub mod replay;
pub mod runner;
pub mod trace;

pub use host::{install_host, Timers};
pub use runner::{
    run_scenario, run_scenario_isolated, run_scenario_isolated_with_setup, run_scenario_with_gc_threshold, scenario_main,
    scenario_main_with_setup, RealmSetup, ScenarioEntry, ScenarioFn,
};
pub use decode::{decode, decode_pass, IdTable};
pub use replay::{replay_suite, Failure, Missing, ReplayOptions, ReplayReport};
pub use trace::{encode, encode_pass, encode_standalone, trace_call, Recorder};
