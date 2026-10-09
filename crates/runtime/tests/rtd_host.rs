//! `DocHost` conformance and re-entrancy (WP-RT-D). Every expectation is written from the host contract (host.rs) and
//! the vn-dom focus and listener rules (crates/dom/src/focus.rs, events.rs), not from the code under test.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use vn_dom::{EventInit, EventKind};
use vn_runtime::host::{EventSink, Host, HostError, HostEvent, HostNs, HostValue, ListenerId, ListenerOpts, ListenerOutcome};
use vn_runtime::host_doc::DocHost;

type Action = Rc<dyn Fn(&mut DocHost, &HostEvent) -> ListenerOutcome>;

/// Records every delivery as "listener:type@phase" and runs the action registered for the listener, if any.
struct TestSink {
    host: DocHost,
    actions: RefCell<HashMap<u32, Action>>,
    log: RefCell<Vec<String>>,
    serials: RefCell<Vec<u64>>,
}

impl TestSink {
    fn install(host: &DocHost) -> Rc<TestSink> {
        let sink = Rc::new(TestSink { host: host.clone(), actions: RefCell::new(HashMap::new()), log: RefCell::new(Vec::new()), serials: RefCell::new(Vec::new()) });
        let mut h = host.clone();
        h.set_event_sink(sink.clone());
        sink
    }
}

impl EventSink for TestSink {
    fn dispatch(&self, listener: ListenerId, event: &HostEvent) -> ListenerOutcome {
        self.log.borrow_mut().push(format!("{}:{}@{}", listener.0, event.ty, event.phase));
        self.serials.borrow_mut().push(self.host.event_info().serial);
        let action = self.actions.borrow().get(&listener.0).cloned();
        match action {
            Some(f) => {
                let mut h = self.host.clone();
                f(&mut h, event)
            }
            None => ListenerOutcome::default(),
        }
    }
}

fn el(h: &mut DocHost, tag: &str) -> vn_runtime::host::HostNode {
    h.create_element(HostNs::Html, tag, None).expect("element")
}

/// `<html><body><div id="parent"><input id="field"></div></body></html>` with the body returned.
fn fixture() -> (DocHost, vn_runtime::host::HostNode, vn_runtime::host::HostNode, vn_runtime::host::HostNode) {
    let mut host = DocHost::new();
    let body = host.install_body().expect("body");
    let parent = el(&mut host, "div");
    let input = el(&mut host, "input");
    host.set_attribute(parent, "id", "parent").unwrap();
    host.set_attribute(input, "id", "field").unwrap();
    host.insert(parent, body, None).unwrap();
    host.insert(input, parent, None).unwrap();
    (host, body, parent, input)
}

#[test]
fn snapshot_and_query_selector_follow_the_tree() {
    let (mut host, body, parent, input) = fixture();
    assert_eq!(host.parent_node(input), Some(parent));
    assert_eq!(host.query_selector("#field").unwrap(), Some(input));
    assert_eq!(host.body(), Some(body));
    assert_eq!(
        host.snapshot(parent).unwrap(),
        "{\"$\":\"el\",\"tag\":\"div\",\"attrs\":[[\"id\",\"parent\"]],\"children\":[{\"$\":\"el\",\"tag\":\"input\",\"attrs\":[[\"id\",\"field\"]],\"children\":[]}]}"
    );
}

/// The contract's re-entrancy requirement: a listener that runs while `remove` is executing (blur fires before the
/// focused subtree is detached) reads the live tree and writes to it; both effects must be visible afterwards.
#[test]
fn blur_listener_reads_and_writes_the_live_tree_during_remove() {
    let (mut host, _body, parent, input) = fixture();
    let sink = TestSink::install(&host);
    host.add_event_listener(input, "blur", ListenerId(1), ListenerOpts::default()).unwrap();
    host.focus(input).unwrap();
    assert_eq!(host.active_element(), Some(input));
    sink.actions.borrow_mut().insert(
        1,
        Rc::new(move |h: &mut DocHost, _ev: &HostEvent| {
            let during = if h.parent_node(input) == Some(parent) { "attached" } else { "detached" };
            h.set_attribute(input, "data-during", during).unwrap();
            // A nested host operation from inside the listener: it must see and change the same document.
            let extra = h.create_element(HostNs::Html, "span", None).unwrap();
            h.insert(extra, parent, None).unwrap();
            h.set_attribute(extra, "id", "extra").unwrap();
            ListenerOutcome::default()
        }),
    );
    host.remove(input).unwrap();
    assert_eq!(host.parent_node(input), None, "the input is detached after remove");
    assert_eq!(host.get_attribute(input, "data-during").as_deref(), Some("attached"));
    assert_eq!(host.active_element(), None, "focus is cleared by the removal");
    let extra = host.query_selector("#extra").unwrap().expect("nested insert is in the live tree");
    assert_eq!(host.parent_node(extra), Some(parent));
    assert_eq!(sink.log.borrow().as_slice(), &["1:blur@2".to_string()]);
}

#[test]
fn listener_outcome_stops_propagation_and_prevents_default() {
    let (mut host, body, parent, input) = fixture();
    let sink = TestSink::install(&host);
    host.add_event_listener(input, "click", ListenerId(1), ListenerOpts::default()).unwrap();
    host.add_event_listener(parent, "click", ListenerId(2), ListenerOpts::default()).unwrap();
    host.add_event_listener(body, "click", ListenerId(3), ListenerOpts::default()).unwrap();
    sink.actions.borrow_mut().insert(1, Rc::new(|_h: &mut DocHost, _e: &HostEvent| ListenerOutcome { prevent_default: true, stop_propagation: true, ..Default::default() }));
    let mut init = EventInit::new("click", EventKind::Mouse);
    init.bubbles = true;
    init.cancelable = true;
    let prevented = host.dispatch_event(input, init).unwrap();
    assert!(prevented, "a cancelable click that was prevented reports defaultPrevented");
    assert_eq!(sink.log.borrow().as_slice(), &["1:click@2".to_string()], "stopPropagation stops the bubble");
}

#[test]
fn passive_listener_cannot_prevent_default() {
    let (mut host, _body, _parent, input) = fixture();
    let sink = TestSink::install(&host);
    let passive = ListenerOpts { passive: true, ..Default::default() };
    host.add_event_listener(input, "click", ListenerId(1), passive).unwrap();
    sink.actions.borrow_mut().insert(1, Rc::new(|_h: &mut DocHost, _e: &HostEvent| ListenerOutcome { prevent_default: true, ..Default::default() }));
    let mut init = EventInit::new("click", EventKind::Mouse);
    init.cancelable = true;
    assert!(!host.dispatch_event(input, init).unwrap());
}

#[test]
fn each_dispatch_gets_a_new_serial_and_event_kind_is_visible() {
    let (mut host, _body, _parent, input) = fixture();
    let sink = TestSink::install(&host);
    host.add_event_listener(input, "keydown", ListenerId(7), ListenerOpts::default()).unwrap();
    let mut init = EventInit::new("keydown", EventKind::Keyboard);
    init.key = "Enter".to_string();
    host.dispatch_event(input, init.clone()).unwrap();
    host.dispatch_event(input, init).unwrap();
    let serials = sink.serials.borrow().clone();
    assert_eq!(serials.len(), 2);
    assert_ne!(serials[0], serials[1], "two top-level dispatches are distinct events: {serials:?}");
    assert_eq!(host.event_info().kind, EventKind::Keyboard);
}

#[test]
fn released_nodes_are_stale_and_detached_subtrees_only_are_released() {
    let (mut host, _body, parent, input) = fixture();
    host.release(parent); // attached: vn-dom refuses (NotDetached), so the node stays live
    assert_eq!(host.parent_node(input), Some(parent));
    host.remove(parent).unwrap();
    host.release(parent);
    assert_eq!(host.tag_name(parent), "", "a released handle names nothing");
    assert_eq!(host.set_attribute(parent, "id", "x"), Err(HostError::NotFound));
    assert_eq!(host.get_attribute(parent, "id"), None);
    assert_eq!(host.parent_node(input), None, "the child of a released subtree is gone too");
}

#[test]
fn property_round_trip_through_the_dom_model() {
    let (mut host, _body, _parent, input) = fixture();
    host.set_property(input, "value", HostValue::Str("hello".into())).unwrap();
    assert_eq!(host.get_property(input, "value").unwrap(), HostValue::Str("hello".into()));
    assert!(host.has_property(input, "value"));
    assert!(!host.has_property(input, "noSuchProperty"));
    assert_eq!(host.tag_name(input), "INPUT");
    assert!(host.is_element(input));
}

#[test]
fn clock_is_settable_and_serves_time_stamps() {
    let (mut host, _body, _parent, input) = fixture();
    let sink = TestSink::install(&host);
    host.set_now(1234.5);
    assert_eq!(host.now_ms(), 1234.5);
    host.add_event_listener(input, "ping", ListenerId(9), ListenerOpts::default()).unwrap();
    let got = Rc::new(RefCell::new(0.0));
    let g = got.clone();
    sink.actions.borrow_mut().insert(
        9,
        Rc::new(move |_h: &mut DocHost, e: &HostEvent| {
            *g.borrow_mut() = e.time_stamp;
            ListenerOutcome::default()
        }),
    );
    host.dispatch_event(input, EventInit::new("ping", EventKind::Event)).unwrap();
    assert_eq!(*got.borrow(), 1234.5);
}

#[test]
fn unsupported_selector_is_an_error_not_a_panic() {
    let (host, _body, _parent, _input) = fixture();
    assert!(matches!(host.query_selector("div > > p"), Err(HostError::Unsupported(_)) | Err(HostError::InvalidState(_))));
}
