//! vn-reactivity: the core of `@vue/reactivity` 3.5.43 (WP-R1a) over the vn-value kernel: the dependency graph, effects,
//! batching, computed values, effect scopes and refs. Behaviour per @vue/reactivity 3.5.43 (production build). The
//! Proxy-based reactive wrappers are WP-R1b; the watch base is `watch`. Safe Rust only.

#![forbid(unsafe_code)]

mod api;
mod computed;
mod effect;
mod graph;
mod node;
mod refs;
mod scope;
mod shared;
mod state;
mod target;
pub mod testing;

pub use api::{exports, internal_exports};
pub use state::{set_reactive_hooks, ReactiveHooks};
pub use target::{track, trigger, TriggerOp};
pub use testing::register_vue_for_tests;
