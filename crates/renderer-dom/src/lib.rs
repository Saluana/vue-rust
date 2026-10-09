//! vn-renderer-dom: the browser host (WP-W1a). `WebHost` implements `vn_runtime::host::Host` over a hand-written wasm32
//! import/export ABI (version 1, documented in hosts/web/ABI.md). The JavaScript glue (hosts/web/vn-host.js) owns every DOM
//! node and applies each operation to the real DOM; Rust only holds opaque `HostNode` handles, the event sink and the
//! timer hook. Nothing here panics across the boundary: the panic hook reports through `host_panic`.
#![deny(unsafe_code)]

pub mod abi;
#[allow(unsafe_code)]
mod ffi;
#[cfg(any(feature = "selftest", test))]
mod json;
mod listeners;
#[cfg(feature = "selftest")]
mod selftest;
mod web_host;

pub use web_host::{LogLevel, WebHost, set_timer_hook};
