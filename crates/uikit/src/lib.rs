//! vn-uikit: the UI `testHost` of the component scenarios (WP-T2). `ui_setup` is the realm setup of the UI harness
//! (tooling/harness/run-language.mjs --ui): it registers the native module "vue" and installs `testHost.mount`, `find`,
//! `findAll`, the event functions and `container` over a fresh headless `DocHost`. The behaviour mirrors
//! tooling/oracle/ui/page-host.mjs and snapshot.mjs (docs/dev/ui-scenario-host.md).
//!
//! Layers: `selector` (the grammar), `index` (child lists of the headless DOM), `ops` (the DOM operations and their
//! error texts, no JavaScript), `snapshot` (the canonical element tree), and `js` (the testHost objects and functions).

#![forbid(unsafe_code)]

pub mod index;
pub mod ops;
pub mod selector;
pub mod snapshot;

mod js;

pub use js::install;

use vn_runtime::host_doc::DocHost;
use vn_value::Rt;

/// The realm setup of a UI scenario: the "vue" native module, and a fresh document with a body, installed by `install`.
pub fn ui_setup(rt: &Rt) {
    let _ = vn_runtime::exports::register_vue_module(rt);
    let mut host = DocHost::new();
    host.install_body().expect("UI host: the body cannot be created (framework bug)");
    install(rt, host);
}
