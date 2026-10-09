//! vn-css: the documented CSS subset for vue-native (WP-U1a, design.md section 9.2). Pure Rust, standard
//! library only, no `unsafe`. Modules: `token` (CSS Syntax 3 tokenizer), `parse` (stylesheet, rules,
//! declarations and recovery diagnostics), `selector`, `matching` (generic over the caller's element trait),
//! `media`, `cascade` (cascade, custom properties and var()), `support` (the allowlist as data).
//! Behaviour is checked against headless Chrome by the corpora under tests/css (see crates/css/tests).

pub mod ast;
pub mod cascade;
pub mod custom;
pub mod diag;
pub mod matching;
pub mod media;
pub mod parse;
pub mod selector;
pub mod support;
pub mod token;

pub use ast::{Declaration, MediaRule, Rule, StyleRule, Stylesheet, TokenList};
pub use cascade::{cascade, CascadedDeclarations, Candidate, Origin};
pub use custom::{computed_custom_properties, resolve_custom_property, CustomProperties, Invalid};
pub use diag::Diagnostic;
pub use matching::{matches, matches_list, Element, ElementState};
pub use media::{MediaEnv, MediaQueryList};
pub use parse::{parse_declarations, parse_stylesheet, ParseOptions, ParseResult};
pub use selector::{SelectorList, Specificity};
