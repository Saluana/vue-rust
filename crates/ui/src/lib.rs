//! vn-ui: typed style values for the documented CSS subset (WP-U1b). `values` and `props` define what a declaration
//! means; `shorthand` expands shorthands into longhands; `compute` runs the cascade of vn-css over a vn-dom document
//! and produces a `ComputedStyle` per element; `ua` is the built-in user-agent stylesheet. Chrome is the oracle
//! (tests/ui/style/corpora). Layout and painting consume the output in later work packages.

pub mod calc;
pub mod color;
pub mod compute;
pub mod diag;
pub mod dom;
pub mod font;
pub mod grid;
pub mod length;
pub mod num;
pub mod parse;
pub mod props;
pub mod serialize;
pub mod shorthand;
pub mod style;
pub mod transform;
pub mod tree;
pub mod ua;
pub mod value;

pub use compute::{compute_styles, ElementStyle, Sheet, StyleTree};
pub use diag::StyleDiagnostic;
pub use dom::{NoStates, StateProvider};
pub use style::ComputedStyle;
pub use value::Value;
