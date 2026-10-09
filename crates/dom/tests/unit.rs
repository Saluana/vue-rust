//! Hand-computed unit tests for what the Chrome corpora cannot express: release and stale ids, error values, the
//! snapshot rule and the conversions, and the documented gaps (which must keep answering as documented). Expected
//! values come from the DOM and HTML specifications or are computed by hand, never from the code under test.

use vn_dom::{Document, DomError, EventInit, EventKind, EventState, HostValue, ListenerId, ListenerOptions, Namespace, NodeId};

/// A listener callback for a tree with no listeners.
fn quiet(_: &mut Document, _: ListenerId, _: &mut EventState) -> bool {
    true
}

fn el(doc: &mut Document, tag: &str) -> NodeId {
    doc.create_element(Namespace::Html, tag, None).expect("valid tag")
}

#[test]
fn document_is_node_zero_and_empty() {
    let doc = Document::new();
    assert_eq!(doc.kind(doc.document()), Some(vn_dom::NodeKind::Document));
    assert_eq!(doc.child_nodes(doc.document()), Vec::<NodeId>::new());
    assert_eq!(doc.node_name(doc.document()).as_deref(), Some("#document"));
}

#[test]
fn release_frees_a_detached_subtree_and_its_ids_go_stale() {
    let mut doc = Document::new();
    let p = el(&mut doc, "div");
    let c = el(&mut doc, "span");
    doc.insert_before(p, c, None, &mut quiet).unwrap();
    assert_eq!(doc.release(p), Ok(()));
    assert_eq!(doc.kind(p), None, "a released node has no kind");
    assert_eq!(doc.kind(c), None, "descendants of a released node are released too");
    assert_eq!(doc.get_attribute(c, "x"), Err(DomError::StaleNode));
    let fresh = el(&mut doc, "p");
    assert_ne!(fresh, p, "a released id is never reused");
    assert_ne!(fresh, c);
}

#[test]
fn release_refuses_attached_nodes_and_the_document() {
    let mut doc = Document::new();
    let p = el(&mut doc, "div");
    doc.insert_before(doc.document(), p, None, &mut quiet).unwrap();
    assert_eq!(doc.release(p), Err(DomError::NotDetached));
    assert_eq!(doc.release(doc.document()), Err(DomError::InvalidNodeType));
    assert_eq!(doc.kind(p), Some(vn_dom::NodeKind::Element), "a refused release changes nothing");
}

#[test]
fn stale_ids_are_errors_not_panics() {
    let mut doc = Document::new();
    let p = el(&mut doc, "div");
    doc.release(p).unwrap();
    assert_eq!(doc.insert_before(doc.document(), p, None, &mut quiet), Err(DomError::StaleNode));
    assert_eq!(doc.set_attribute(p, "a", "b"), Err(DomError::StaleNode));
    assert_eq!(doc.dispatch_event(p, EventInit::new("x", EventKind::Event), &mut quiet), Err(DomError::StaleNode));
    assert_eq!(doc.focus(p, &mut quiet), Err(DomError::StaleNode));
    assert_eq!(doc.query_selector(p, "div"), Err(DomError::StaleNode));
    assert!(!doc.has_property(p, "id"));
}

#[test]
fn a_refused_insertion_leaves_the_tree_as_it_was() {
    let mut doc = Document::new();
    let p = el(&mut doc, "div");
    let a = el(&mut doc, "i");
    let q = el(&mut doc, "b");
    let elsewhere = el(&mut doc, "u");
    doc.append_child(p, a, &mut quiet).unwrap();
    assert_eq!(doc.append_child(p, p, &mut quiet), Err(DomError::HierarchyRequestError));
    assert_eq!(doc.insert_before(p, q, Some(elsewhere), &mut quiet), Err(DomError::NotFoundError));
    assert_eq!(doc.child_nodes(p), vec![a]);
    assert_eq!(doc.parent_node(q), None);
}

#[test]
fn text_content_of_a_document_is_absent_and_setting_it_changes_nothing() {
    let mut doc = Document::new();
    assert_eq!(doc.text_content(doc.document()), None);
    doc.set_text_content(doc.document(), "x", &mut quiet).unwrap();
    assert_eq!(doc.text_content(doc.document()), None);
}

#[test]
fn error_names_are_the_dom_exception_names() {
    assert_eq!(DomError::HierarchyRequestError.name(), "HierarchyRequestError");
    assert_eq!(DomError::NotFoundError.name(), "NotFoundError");
    assert_eq!(DomError::InvalidCharacterError.name(), "InvalidCharacterError");
    assert_eq!(DomError::NamespaceError.name(), "NamespaceError");
    assert_eq!(DomError::SyntaxError.name(), "SyntaxError");
    assert_eq!(DomError::ReadOnlyProperty.name(), "TypeError", "strict-mode assignment to a getter-only property");
}

#[test]
fn snapshot_of_a_small_tree_matches_the_documented_rule() {
    let mut doc = Document::new();
    let d = el(&mut doc, "div");
    doc.set_attribute(d, "id", "a").unwrap();
    doc.set_attribute(d, "class", "b c").unwrap();
    let t = doc.create_text("text");
    let c = doc.create_comment("c");
    doc.append_child(d, t, &mut quiet).unwrap();
    doc.append_child(d, c, &mut quiet).unwrap();
    assert_eq!(
        doc.snapshot(d).unwrap(),
        r#"{"$":"el","tag":"div","attrs":[["id","a"],["class","b c"]],"children":[{"$":"text","v":"text"},{"$":"comment","v":"c"}]}"#
    );
}

#[test]
fn snapshot_lists_only_differing_props_and_marks_foreign_namespaces() {
    let mut doc = Document::new();
    let i = el(&mut doc, "input");
    doc.set_attribute(i, "value", "a").unwrap();
    doc.set_property(i, "value", HostValue::Str("b".into()), &mut quiet).unwrap();
    assert_eq!(
        doc.snapshot(i).unwrap(),
        r#"{"$":"el","tag":"input","attrs":[["value","a"]],"props":{"value":"b"},"children":[]}"#
    );
    let svg = doc.create_element(Namespace::Svg, "rect", None).unwrap();
    assert_eq!(doc.snapshot(svg).unwrap(), r#"{"$":"el","tag":"rect","ns":"svg","attrs":[],"children":[]}"#);
}

#[test]
fn snapshot_escapes_strings_like_json_stringify() {
    let mut doc = Document::new();
    let t = doc.create_text("q\"\\\u{1}\n\t\u{e9}");
    assert_eq!(doc.snapshot(t).unwrap(), "{\"$\":\"text\",\"v\":\"q\\\"\\\\\\u0001\\n\\t\u{e9}\"}");
}

#[test]
fn number_and_string_conversions_follow_javascript() {
    assert_eq!(HostValue::Number(2.5).to_dom_string(), "2.5");
    assert_eq!(HostValue::Number(-0.0).to_dom_string(), "0");
    assert_eq!(HostValue::Number(f64::NAN).to_dom_string(), "NaN");
    assert_eq!(HostValue::Number(f64::NEG_INFINITY).to_dom_string(), "-Infinity");
    assert_eq!(HostValue::Null.to_dom_string(), "null");
    assert_eq!(HostValue::Undefined.to_dom_string(), "undefined");
    assert_eq!(HostValue::Undefined.to_nullable_string(), "");
    assert_eq!(HostValue::Null.to_dom_string_null_empty(), "");
    assert_eq!(HostValue::Undefined.to_dom_string_null_empty(), "undefined");
    assert_eq!(HostValue::Str("0x10".into()).to_number(), 16.0);
    assert_eq!(HostValue::Str(" 12 ".into()).to_number(), 12.0);
    assert_eq!(HostValue::Str("1e3".into()).to_number(), 1000.0);
    assert_eq!(HostValue::Str("5.".into()).to_number(), 5.0);
    assert_eq!(HostValue::Str("".into()).to_number(), 0.0);
    assert!(HostValue::Str("1e".into()).to_number().is_nan());
    assert!(HostValue::Str("12px".into()).to_number().is_nan());
    assert_eq!(HostValue::Number(4294967297.0).to_long(), 1);
    assert_eq!(HostValue::Number(-1.0).to_long(), -1);
    assert_eq!(HostValue::Number(2.9).to_long(), 2);
    assert!(HostValue::Str("0".into()).to_boolean() && !HostValue::Str("".into()).to_boolean());
}

#[test]
fn selectors_outside_the_supported_grammar_are_errors_values() {
    let mut doc = Document::new();
    let root = el(&mut doc, "div");
    doc.insert_before(doc.document(), root, None, &mut quiet).unwrap();
    for sel in ["div p", "div > p", "a:hover", "[x^=y]", "a + b", "p::before"] {
        assert_eq!(doc.query_selector(root, sel), Err(DomError::UnsupportedSelector), "{sel}");
    }
    assert_eq!(doc.query_selector(root, ""), Err(DomError::SyntaxError));
    assert_eq!(doc.query_selector(root, "a,,b"), Err(DomError::SyntaxError));
}

#[test]
fn unknown_style_property_names_are_errors_and_invalid_values_are_ignored() {
    let mut doc = Document::new();
    let d = el(&mut doc, "div");
    assert_eq!(doc.style_set_property(d, "not-a-property", "1px", false), Err(DomError::UnsupportedStyleProperty));
    doc.style_set_property(d, "width", "10px", false).unwrap();
    doc.style_set_property(d, "width", "abc", false).unwrap();
    assert_eq!(doc.css_text(d).unwrap(), "width: 10px;", "CSSOM ignores an invalid value");
    assert_eq!(doc.style_get_property(d, "not-a-property").unwrap(), "");
}

#[test]
fn style_attribute_round_trip_is_exact_for_raw_values_until_cssom_changes_it() {
    let mut doc = Document::new();
    let d = el(&mut doc, "div");
    doc.set_attribute(d, "style", "top: 1px ;left:2px").unwrap();
    assert_eq!(doc.get_attribute(d, "style").unwrap().as_deref(), Some("top: 1px ;left:2px"));
    doc.style_set_property(d, "color", "red", false).unwrap();
    assert_eq!(doc.get_attribute(d, "style").unwrap().as_deref(), Some("top: 1px; left: 2px; color: red;"));
}

#[test]
fn documented_gaps_answer_as_documented() {
    let mut doc = Document::new();
    let d = el(&mut doc, "div");
    // Chrome answers true for these; vn-dom does not model them (see props.rs).
    for name in ["innerText", "dataset", "lang", "classList", "onclick", "isContentEditable"] {
        assert!(!doc.has_property(d, name), "{name}");
        assert_eq!(doc.get_property(d, name), Ok(HostValue::Undefined), "{name}");
    }
    // Object-valued properties: `in` is true, but a HostValue cannot hold an object.
    assert!(doc.has_property(d, "style") && doc.has_property(d, "children"));
    assert_eq!(doc.get_property(d, "style"), Err(DomError::UnsupportedProperty));
    // Expandos are not modelled: a set is an error, not a silent expando.
    assert_eq!(doc.set_property(d, "_vei", HostValue::Bool(true), &mut quiet), Err(DomError::UnsupportedProperty));
}

#[test]
fn property_errors_are_values() {
    let mut doc = Document::new();
    let d = el(&mut doc, "div");
    assert_eq!(doc.set_property(d, "tagName", HostValue::Str("x".into()), &mut quiet), Err(DomError::ReadOnlyProperty));
    assert_eq!(doc.set_property(d, "innerHTML", HostValue::Str("<b>".into()), &mut quiet), Err(DomError::NotSupportedError));
    let t = doc.create_text("t");
    assert_eq!(doc.set_attribute(t, "a", "b"), Err(DomError::InvalidNodeType));
    assert_eq!(doc.outer_html(t), None);
    assert_eq!(doc.inner_html(doc.document()), None);
}

#[test]
fn listener_registration_is_keyed_by_type_listener_and_capture() {
    let mut doc = Document::new();
    let d = el(&mut doc, "div");
    let id = ListenerId(7);
    doc.add_event_listener(d, "x", id, ListenerOptions::default()).unwrap();
    doc.add_event_listener(d, "x", id, ListenerOptions { capture: false, once: true, passive: true }).unwrap();
    let mut calls = 0;
    let mut count = |_: &mut Document, _: ListenerId, _: &mut EventState| {
        calls += 1;
        true
    };
    doc.dispatch_event(d, EventInit::new("x", EventKind::Event), &mut count).unwrap();
    doc.dispatch_event(d, EventInit::new("x", EventKind::Event), &mut count).unwrap();
    assert_eq!(calls, 2, "a duplicate add is ignored, so the once option was never applied");
}

#[test]
fn prevent_default_is_ignored_when_not_cancelable() {
    let mut doc = Document::new();
    let d = el(&mut doc, "div");
    doc.add_event_listener(d, "p", ListenerId(1), ListenerOptions::default()).unwrap();
    let mut prevent = |_: &mut Document, _: ListenerId, ev: &mut EventState| {
        ev.prevent_default();
        true
    };
    let r = doc.dispatch_event(d, EventInit::new("p", EventKind::Event), &mut prevent).unwrap();
    assert!(!r.default_prevented);
    let mut cancelable = EventInit::new("p", EventKind::Event);
    cancelable.cancelable = true;
    assert!(doc.dispatch_event(d, cancelable, &mut prevent).unwrap().default_prevented);
}

#[test]
fn focus_on_a_detached_element_does_nothing() {
    let mut doc = Document::new();
    let input = el(&mut doc, "input");
    doc.focus(input, &mut quiet).unwrap();
    assert_eq!(doc.active_element(), None);
}

#[test]
fn query_by_id_takes_the_first_match_in_tree_order() {
    let mut doc = Document::new();
    let main = el(&mut doc, "main");
    doc.insert_before(doc.document(), main, None, &mut quiet).unwrap();
    let a = el(&mut doc, "b");
    let b = el(&mut doc, "i");
    doc.set_attribute(a, "id", "k").unwrap();
    doc.set_attribute(b, "id", "k").unwrap();
    doc.append_child(main, b, &mut quiet).unwrap();
    doc.insert_before(main, a, Some(b), &mut quiet).unwrap();
    assert_eq!(doc.get_element_by_id(doc.document(), "k"), Some(a));
    assert_eq!(doc.get_element_by_id(doc.document(), ""), None);
}

#[test]
fn an_element_created_with_is_has_no_is_attribute() {
    let mut doc = Document::new();
    let b = doc.create_element(Namespace::Html, "button", Some("x-button")).unwrap();
    assert_eq!(doc.get_attribute(b, "is").unwrap(), None);
    assert_eq!(doc.outer_html(b).as_deref(), Some("<button></button>"));
}

#[test]
fn camel_case_setter_converts_like_cssom() {
    let mut doc = Document::new();
    let d = el(&mut doc, "div");
    doc.style_set_camel(d, "backgroundColor", "red", false).unwrap();
    assert_eq!(doc.css_text(d).unwrap(), "background-color: red;");
    assert_eq!(doc.style_get_property(d, "background-color").unwrap(), "red");
    doc.style_set_camel(d, "cssFloat", "left", false).unwrap();
    assert_eq!(doc.style_get_property(d, "float").unwrap(), "left", "cssFloat is the float property");
    doc.style_set_camel(d, "background-color", "blue", false).unwrap();
    assert_eq!(doc.style_get_property(d, "background-color").unwrap(), "blue", "kebab-case names pass through");
}

#[test]
fn camel_case_setter_leaves_custom_properties_and_reports_unknown_names_as_errors() {
    let mut doc = Document::new();
    let d = el(&mut doc, "div");
    doc.style_set_camel(d, "--myVar", "1px", false).unwrap();
    assert_eq!(doc.style_get_property(d, "--myVar").unwrap(), "1px", "custom names keep their case");
    assert_eq!(doc.style_get_property(d, "--myvar").unwrap(), "", "and are case-sensitive");
    assert_eq!(doc.style_set_camel(d, "bogusProp", "1px", false), Err(DomError::UnsupportedStyleProperty));
    assert_eq!(doc.css_text(d).unwrap(), "--myVar: 1px;");
}

#[test]
fn input_and_change_follow_a_connected_checkbox_click_and_not_a_detached_one() {
    let mut doc = Document::new();
    let container = el(&mut doc, "div");
    doc.insert_before(doc.document(), container, None, &mut quiet).unwrap();
    let c = doc.create_element(Namespace::Html, "input", None).unwrap();
    doc.set_attribute(c, "type", "checkbox").unwrap();
    doc.append_child(container, c, &mut quiet).unwrap();
    for (ty, id) in [("click", 1), ("input", 2), ("change", 3)] {
        doc.add_event_listener(c, ty, ListenerId(id), ListenerOptions::default()).unwrap();
    }
    let mut seen: Vec<u32> = Vec::new();
    doc.click(c, &mut |_: &mut Document, id: ListenerId, _: &mut EventState| {
        seen.push(id.0);
        true
    })
    .unwrap();
    assert_eq!(seen, vec![1, 2, 3], "click, then input, then change");
    assert_eq!(doc.get_property(c, "checked"), Ok(HostValue::Bool(true)));

    // Detached: the checkbox toggles, but input and change are not fired (only the click listener runs).
    let detached = el(&mut doc, "input");
    doc.set_attribute(detached, "type", "checkbox").unwrap();
    for (ty, id) in [("click", 4), ("input", 5), ("change", 6)] {
        doc.add_event_listener(detached, ty, ListenerId(id), ListenerOptions::default()).unwrap();
    }
    let mut ran: Vec<u32> = Vec::new();
    doc.click(detached, &mut |_: &mut Document, id: ListenerId, _: &mut EventState| {
        ran.push(id.0);
        true
    })
    .unwrap();
    assert_eq!(ran, vec![4], "detached: click only");
    assert_eq!(doc.get_property(detached, "checked"), Ok(HostValue::Bool(true)));
}
