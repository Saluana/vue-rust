//! vn-runtime: behaviour of Vue 3.5.43 `runtime-core` + `runtime-dom` (production semantics) over `vn-value`, driving a DOM-like
//! host (`vn-dom::Document` or the browser). One module per concern; each module is owned by one work package (see docs/dev/wp/WP-RT-*.md).
#![forbid(unsafe_code)]

pub mod app;
pub mod async_component;
pub mod component;
pub mod directives;
pub mod dom_events;
pub mod dom_props;
pub mod emits;
pub mod errors;
pub mod exports;
pub mod helpers;
pub mod host;
pub mod host_doc;
pub mod inject;
pub mod instance;
pub mod keep_alive;
pub mod keyed;
pub mod lifecycle;
pub mod normalize;
pub mod props;
pub mod public_instance;
pub mod refs;
pub mod renderer;
pub mod scheduler;
pub mod setup_helpers;
pub mod shapes;
pub mod slots;
pub mod teleport;
pub mod timers;
pub mod transition;
pub mod vmodel;
pub mod vnode;
pub mod vshow;
pub mod watch;
