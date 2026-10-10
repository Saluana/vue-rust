//! vn-reactivity: the core of `@vue/reactivity` 3.5.43 (WP-R1a) over the vn-value kernel: the dependency graph, effects,
//! batching, computed values, effect scopes and refs (WP-R1a), and the Proxy-based reactive wrappers (WP-R1b:
//! reactive, readonly, shallow variants, array and collection instrumentation). Behaviour per @vue/reactivity 3.5.43
//! (production build). The watch base is `watch`. Safe Rust only.

#![forbid(unsafe_code)]

mod api;
mod array_inst;
mod collection_inst;
mod computed;
mod diag;
mod effect;
mod graph;
mod handlers;
mod node;
mod reactive;
mod refs;
mod scope;
mod shared;
mod state;
mod target;
pub mod testing;
mod watch;

pub use api::{exports, internal_exports};
pub use diag::{debug_stats, dep_subscriber_count, ReactivityStats};
pub use state::{set_reactive_hooks, ReactiveHooks};
pub use target::{track, trigger, TriggerOp};
pub use watch::{watch_base, WatchHandle, WatchOptions};
pub use testing::register_vue_for_tests;
