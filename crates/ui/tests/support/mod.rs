//! Shared test code: the corpus JSON reader and the fixed DOM contexts that the corpora describe. The contexts are
//! built through vn-dom's API (no HTML parser), and they mirror the Chrome pages of tests/ui/style/gen/measure.mjs.

#![allow(dead_code)]

pub mod json;

use vn_dom::{Document, EventState, ListenerId, Namespace, NodeId};

pub use json::Json;

/// A listener callback for DOM calls that may dispatch: no listeners are registered, so nothing runs.
pub fn no_listeners(_: &mut Document, _: ListenerId, _: &mut EventState) -> bool {
    true
}

/// Appends `child` to `parent` (no listeners are involved, so the callback accepts every dispatch).
pub fn append(doc: &mut Document, parent: NodeId, child: NodeId) {
    doc.append_child(parent, child, &mut no_listeners).expect("append");
}

/// An element with the given style attribute (when non-empty) and no children yet.
pub fn element(doc: &mut Document, tag: &str, style: &str) -> NodeId {
    let el = doc.create_element(Namespace::Html, tag, None).expect("create element");
    if !style.is_empty() {
        doc.set_attribute(el, "style", style).expect("style attribute");
    }
    el
}

/// The value-corpus context: html (font-size 16px) > body > div#p (font-size 20px, font-weight `parent_weight` when
/// given) > div#t with the style attribute `t_style`. Returns the document and #t.
pub fn value_context(t_style: &str, parent_weight: Option<&str>) -> (Document, NodeId) {
    let mut doc = Document::new();
    let html = element(&mut doc, "html", "font-size: 16px");
    let body = element(&mut doc, "body", "");
    let p_style = match parent_weight {
        Some(w) => format!("font-size: 20px; font-weight: {w}"),
        None => "font-size: 20px".to_string(),
    };
    let p = element(&mut doc, "div", &p_style);
    let t = element(&mut doc, "div", t_style);
    append(&mut doc, p, t);
    append(&mut doc, body, p);
    append(&mut doc, html, body);
    let root = doc.document();
    append(&mut doc, root, html);
    (doc, t)
}
