//! vn-dom: a minimal DOM-like document model (WP-U0). It is the in-memory host of the Vue runtime port: the headless
//! test host, the live tree of the native renderer, and the input of the SSR serializer. Behaviour is checked against
//! headless Chrome: tests/dom/gen generates tests/dom/corpora, and crates/dom/tests replays them.
//!
//! Conventions. A NodeId stays valid until `release`. Operations that can run listeners (insert_before, remove_child,
//! detach, set_text_content, set_property, focus, blur, click, dispatch_event) take an `Invoke` callback that the host
//! supplies; pass a callback that returns true when no listener is registered. Every failure is a `DomError` value and
//! nothing panics on user input. Unsupported input is an error or documented, never silently approximated, and the
//! documented gaps are listed at the top of props.rs, style.rs, focus.rs, form.rs and query.rs.

mod attr;
mod class;
mod error;
mod events;
mod focus;
mod form;
mod node;
mod props;
mod query;
mod serialize;
mod style;
mod tree;
mod value;

pub use error::DomError;
pub use events::{DispatchResult, EventInit, EventKind, EventState, Invoke, ListenerId, ListenerOptions};
pub use node::{Document, Namespace, NodeId, NodeKind};
pub use value::HostValue;
