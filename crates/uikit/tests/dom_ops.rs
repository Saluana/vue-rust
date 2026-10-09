// The DOM operations of the UI testHost over hand-built trees (no JavaScript): find, snapshot, events, forms. Expected
// values are written from docs/dev/ui-scenario-host.md and the oracle's page-host.mjs, not computed by the code under test.
use std::cell::RefCell;
use std::rc::Rc;

use vn_dom::EventKind;
use vn_runtime::host::{EventSink, Host, HostEvent, HostNode, HostNs, HostValue, ListenerId, ListenerOpts, ListenerOutcome};
use vn_runtime::host_doc::DocHost;
use vn_uikit::index::Index;
use vn_uikit::ops::{self, EventSpec, SelectValue, UiError};
use vn_uikit::selector::parse;
use vn_uikit::snapshot;

fn el(h: &mut DocHost, parent: HostNode, tag: &str, attrs: &[(&str, &str)]) -> HostNode {
    let e = h.create_element(HostNs::Html, tag, None).expect("element");
    for (k, v) in attrs {
        h.set_attribute(e, k, v).expect("attribute");
    }
    h.insert(e, parent, None).expect("insert");
    e
}

fn text(h: &mut DocHost, parent: HostNode, s: &str) -> HostNode {
    let t = h.create_text(s);
    h.insert(t, parent, None).expect("insert text");
    t
}

fn comment(h: &mut DocHost, parent: HostNode, s: &str) {
    let c = h.create_comment(s);
    h.insert(c, parent, None).expect("insert comment");
}

/// A document with body > div#root, and a list of ids (as strings) for the matches.
fn ids(h: &DocHost, nodes: &[HostNode]) -> Vec<String> {
    nodes.iter().map(|n| h.get_attribute(*n, "id").unwrap_or_else(|| format!("#{}", n.0))).collect()
}

fn find(h: &DocHost, root: HostNode, sel: &str) -> Vec<HostNode> {
    ops::find_all(h, root, &parse(sel).expect("selector")).expect("find")
}

/// body > div#root > [span.a#s1, "t", p > span.a#s2, <!--c-->, span.b#s3]
fn sample() -> (DocHost, HostNode) {
    let mut h = DocHost::new();
    let body = h.install_body().expect("body");
    let root = el(&mut h, body, "div", &[("id", "root")]);
    el(&mut h, root, "span", &[("id", "s1"), ("class", "a")]);
    text(&mut h, root, "t");
    let p = el(&mut h, root, "p", &[("id", "p")]);
    el(&mut h, p, "span", &[("id", "s2"), ("class", "x  a\tc")]);
    comment(&mut h, root, "c");
    el(&mut h, root, "span", &[("id", "s3"), ("class", "b"), ("data-k", "v w")]);
    (h, root)
}

#[test]
fn find_returns_matches_in_document_order_below_the_scope_only() {
    let (h, root) = sample();
    assert_eq!(ids(&h, &find(&h, root, "span")), ["s1", "s2", "s3"]);
    assert_eq!(ids(&h, &find(&h, root, "div")), Vec::<String>::new(), "the scope itself is not a match");
    assert_eq!(ids(&h, &find(&h, root, "#s2")), ["s2"]);
    assert_eq!(find(&h, root, "#nope"), Vec::<HostNode>::new());
}

#[test]
fn class_matches_whole_tokens_and_attributes_match_exactly() {
    let (h, root) = sample();
    assert_eq!(ids(&h, &find(&h, root, ".a")), ["s1", "s2"], "a token in a whitespace-separated list");
    assert!(find(&h, root, ".ab").is_empty(), "a substring is not a class");
    assert_eq!(ids(&h, &find(&h, root, "[data-k=\"v w\"]")), ["s3"]);
    assert_eq!(ids(&h, &find(&h, root, "[data-k='v w']")), ["s3"]);
    assert!(find(&h, root, "[data-k=v]").is_empty(), "the value must match exactly");
    assert_eq!(ids(&h, &find(&h, root, "[data-k]")), ["s3"]);
    assert!(find(&h, root, "[data-missing]").is_empty());
}

#[test]
fn nth_child_counts_element_siblings_only() {
    let (h, root) = sample();
    // Children of root: span#s1 (1), text, p (2), comment, span#s3 (3). Text and comments do not count.
    // span#s2 is the first element child of its parent p, so it matches as well (any parent counts).
    assert_eq!(ids(&h, &find(&h, root, "span:nth-child(1)")), ["s1", "s2"]);
    assert_eq!(ids(&h, &find(&h, root, "span:nth-child(3)")), ["s3"]);
    assert_eq!(ids(&h, &find(&h, root, ":nth-child(2)")), ["p"]);
    assert!(find(&h, root, "span:nth-child(2)").is_empty());
    assert!(find(&h, root, "span:nth-child(0)").is_empty(), "positions start at 1");
}

#[test]
fn child_lists_follow_moves_and_removals() {
    let mut h = DocHost::new();
    let body = h.install_body().expect("body");
    let a = el(&mut h, body, "i", &[("id", "a")]);
    let b = el(&mut h, body, "i", &[("id", "b")]);
    let c = el(&mut h, body, "i", &[("id", "c")]);
    h.insert(a, body, None).expect("move a to the end");
    assert_eq!(Index::build(&h).expect("index").children(body), &[b, c, a]);
    h.remove(b).expect("remove b");
    assert_eq!(Index::build(&h).expect("index").children(body), &[c, a]);
    h.insert(b, body, Some(a)).expect("insert b before a");
    assert_eq!(Index::build(&h).expect("index").children(body), &[c, b, a]);
}

#[test]
fn snapshot_has_contract_shape_and_keeps_text_nodes_apart() {
    let mut h = DocHost::new();
    let body = h.install_body().expect("body");
    let div = el(&mut h, body, "div", &[("id", "a"), ("class", "b")]);
    text(&mut h, div, "x");
    text(&mut h, div, "y");
    comment(&mut h, div, "c");
    let snap = ops::snapshot_of(&h, div).expect("snapshot").to_json();
    assert_eq!(
        snap,
        r#"{"$":"el","tag":"div","attrs":[["id","a"],["class","b"]],"children":[{"$":"text","v":"x"},{"$":"text","v":"y"},{"$":"comment","v":"c"}]}"#
    );
}

#[test]
fn snapshot_props_are_the_dirty_value_checked_selected_and_disabled_in_that_order() {
    let mut h = DocHost::new();
    let body = h.install_body().expect("body");
    // Attribute value and a different property: props value only.
    let input = el(&mut h, body, "input", &[("type", "text"), ("value", "a")]);
    h.set_property(input, "value", HostValue::Str("b".into())).expect("set value");
    let snap = ops::snapshot_of(&h, input).expect("snapshot").to_json();
    assert_eq!(snap, r#"{"$":"el","tag":"input","attrs":[["type","text"],["value","a"]],"props":[["value","b"]],"children":[]}"#);
    // The property equal to the attribute-derived default: no props at all.
    let plain = el(&mut h, body, "input", &[("type", "text"), ("value", "a")]);
    assert_eq!(snapshot::contract_of(&h.snapshot(plain).expect("raw")).expect("contract").to_json().contains("props"), false);
    // A checkbox checked by property (no checked attribute): props checked true.
    let cb = el(&mut h, body, "input", &[("type", "checkbox")]);
    h.set_property(cb, "checked", HostValue::Bool(true)).expect("checked");
    assert!(ops::snapshot_of(&h, cb).expect("snapshot").to_json().contains(r#""props":[["checked",true]]"#));
    // A disabled attribute reflects into the property: no props. Option selected by property: props selected.
    let btn = el(&mut h, body, "button", &[("disabled", "")]);
    assert!(!ops::snapshot_of(&h, btn).expect("snapshot").to_json().contains("props"));
    let sel = el(&mut h, body, "select", &[]);
    let opt = el(&mut h, sel, "option", &[("value", "1")]);
    h.set_property(opt, "selected", HostValue::Bool(true)).expect("selected");
    assert!(ops::snapshot_of(&h, sel).expect("snapshot").to_json().contains(r#"["selected",true]"#));
}

#[test]
fn snapshot_props_pairs_come_in_contract_order() {
    let mut h = DocHost::new();
    let body = h.install_body().expect("body");
    let field = el(&mut h, body, "input", &[("type", "text"), ("value", "x")]);
    h.set_property(field, "checked", HostValue::Bool(true)).expect("checked");
    h.set_property(field, "value", HostValue::Str("y".into())).expect("value");
    let snap = ops::snapshot_of(&h, field).expect("snapshot").to_json();
    assert!(snap.contains(r#""props":[["value","y"],["checked",true]]"#), "{snap}");
}

/// Records every listener call as "listener:type:data".
struct Log(Rc<RefCell<Vec<String>>>);

impl EventSink for Log {
    fn dispatch(&self, listener: ListenerId, event: &HostEvent) -> ListenerOutcome {
        self.0.borrow_mut().push(format!("{}:{}:{}", listener.0, event.ty, event.input_data.clone().unwrap_or_default()));
        ListenerOutcome::default()
    }
}

fn logged(h: &mut DocHost) -> Rc<RefCell<Vec<String>>> {
    let log = Rc::new(RefCell::new(Vec::new()));
    h.set_event_sink(Rc::new(Log(log.clone())));
    log
}

#[test]
fn input_sets_the_value_and_dispatches_a_bubbling_input_event() {
    let mut h = DocHost::new();
    let body = h.install_body().expect("body");
    let wrap = el(&mut h, body, "div", &[]);
    let field = el(&mut h, wrap, "input", &[("type", "text")]);
    let log = logged(&mut h);
    h.add_event_listener(wrap, "input", ListenerId(1), ListenerOpts::default()).expect("listener");
    ops::input(&mut h, field, Some("hi"), 3.0).expect("input");
    assert_eq!(h.get_property(field, "value").expect("value"), HostValue::Str("hi".into()));
    assert_eq!(*log.borrow(), ["1:input:hi"], "the ancestor sees the bubbling input event with its data");
    assert_eq!(ops::input(&mut h, field, None, 3.0), Err(UiError::Type("input: value must be a string".into())));
    let cb = el(&mut h, body, "input", &[("type", "checkbox")]);
    assert_eq!(
        ops::input(&mut h, cb, Some("x"), 3.0),
        Err(UiError::Type("input: the target must be a text input, a range or a textarea".into())),
        "a checkbox is not a text input"
    );
}

#[test]
fn check_toggles_through_activation_and_refuses_disabled_and_radio_cases() {
    let mut h = DocHost::new();
    let body = h.install_body().expect("body");
    let cb = el(&mut h, body, "input", &[("type", "checkbox")]);
    let log = logged(&mut h);
    h.add_event_listener(body, "change", ListenerId(7), ListenerOpts::default()).expect("listener");
    ops::check(&mut h, cb, Some(true), 0.0).expect("check");
    assert_eq!(h.get_property(cb, "checked").expect("checked"), HostValue::Bool(true));
    assert!(log.borrow().iter().any(|e| e.starts_with("7:change")), "a change event reaches the ancestor: {:?}", log.borrow());
    ops::check(&mut h, cb, Some(true), 0.0).expect("already checked: no event");
    let disabled = el(&mut h, body, "input", &[("type", "checkbox"), ("disabled", "")]);
    assert_eq!(
        ops::check(&mut h, disabled, Some(true), 0.0),
        Err(UiError::Error("check: the control did not change (is it disabled?)".into()))
    );
    let radio = el(&mut h, body, "input", &[("type", "radio"), ("checked", "")]);
    assert_eq!(
        ops::check(&mut h, radio, Some(false), 0.0),
        Err(UiError::Type("check: a radio button cannot be unchecked by user activation".into()))
    );
    assert_eq!(ops::check(&mut h, cb, None, 0.0), Err(UiError::Type("check: the second argument must be a boolean".into())));
    let text_field = el(&mut h, body, "input", &[("type", "text")]);
    assert_eq!(ops::check(&mut h, text_field, Some(true), 0.0), Err(UiError::Type("check: the target must be a checkbox or a radio button".into())));
}

#[test]
fn click_bubbles_and_uses_the_mouse_defaults() {
    let mut h = DocHost::new();
    let body = h.install_body().expect("body");
    let outer = el(&mut h, body, "div", &[]);
    let inner = el(&mut h, outer, "button", &[]);
    let log = logged(&mut h);
    h.add_event_listener(outer, "click", ListenerId(3), ListenerOpts::default()).expect("listener");
    ops::click(&mut h, inner, 9.0).expect("click");
    assert_eq!(*log.borrow(), ["3:click:"]);
    let init = ops::event_init("click", &EventSpec::default());
    assert_eq!((init.kind, init.bubbles, init.cancelable, init.detail, init.composed), (EventKind::Mouse, true, true, 1, true));
    let focus = ops::event_init("focus", &EventSpec::default());
    assert_eq!((focus.kind, focus.bubbles, focus.cancelable), (EventKind::Event, false, false));
    let input = ops::event_init("input", &EventSpec::default());
    assert_eq!((input.kind, input.bubbles, input.cancelable), (EventKind::Input, true, false));
    let custom = ops::event_init("custom", &EventSpec::default());
    assert_eq!((custom.bubbles, custom.cancelable), (true, true), "an event type the oracle does not list bubbles and cancels");
    let key = ops::event_init("keydown", &EventSpec { key: Some("Enter".into()), ..EventSpec::default() });
    assert_eq!((key.kind, key.key.as_str(), key.bubbles), (EventKind::Keyboard, "Enter", true));
}

#[test]
fn select_takes_an_array_for_a_multiple_select_and_a_string_otherwise() {
    let mut h = DocHost::new();
    let body = h.install_body().expect("body");
    let multi = el(&mut h, body, "select", &[("multiple", "")]);
    let opts: Vec<HostNode> = ["a", "b", "c"].iter().map(|v| el(&mut h, multi, "option", &[("value", *v)])).collect();
    ops::select(&mut h, multi, SelectValue::Many(&["b".to_string(), "c".to_string()]), 0.0).expect("select");
    let selected: Vec<HostValue> = opts.iter().map(|o| h.get_property(*o, "selected").expect("selected")).collect();
    assert_eq!(selected, [HostValue::Bool(false), HostValue::Bool(true), HostValue::Bool(true)]);
    assert_eq!(
        ops::select(&mut h, multi, SelectValue::One("a"), 0.0),
        Err(UiError::Type("select: a multiple select takes an array of values".into()))
    );
    let single = el(&mut h, body, "select", &[]);
    el(&mut h, single, "option", &[("value", "x")]);
    el(&mut h, single, "option", &[("value", "y")]);
    assert_eq!(ops::select(&mut h, single, SelectValue::Many(&[]), 0.0), Err(UiError::Type("select: value must be a string".into())));
    ops::select(&mut h, single, SelectValue::One("y"), 0.0).expect("single");
    assert_eq!(h.get_property(single, "value").expect("value"), HostValue::Str("y".into()));
    assert_eq!(ops::select(&mut h, body, SelectValue::One("y"), 0.0), Err(UiError::Type("select: the target must be a select element".into())));
}

#[test]
fn container_reuses_an_id_but_each_mount_gets_a_new_element() {
    let mut h = DocHost::new();
    let body = h.install_body().expect("body");
    let first = ops::container(&mut h, "box").expect("container");
    assert_eq!(ops::container(&mut h, "box").expect("again"), first);
    assert_eq!(h.parent_node(first), Some(body));
    let app = ops::mount_container(&mut h, 1).expect("mount");
    let again = ops::mount_container(&mut h, 1).expect("second mount with the same number");
    assert_ne!(app, again, "a mount always appends a new container");
    assert_eq!(h.get_attribute(app, "id").as_deref(), Some("app-1"));
}
