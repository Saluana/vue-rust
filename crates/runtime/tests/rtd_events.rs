//! Event layer tests (WP-RT-D): the collector and the side tables (handlers and invokers survive collections, released
//! nodes take their state with them), the listener registry (replacement keeps one listener), the event object's fields
//! by event kind, and the withModifiers / withKeys helpers. Expectations come from the Vue source (runtime-dom 3.5.43)
//! and the DOM rules, and the corpora cover the same behaviour against Chrome.

use std::cell::RefCell;

use vn_runtime::dom_events::DomCtx;
use vn_runtime::dom_props::patch_prop;
use vn_runtime::exports;
use vn_runtime::host::{Host, HostNs};
use vn_runtime::host_doc::DocHost;
use vn_value::{payload_ref, strict_eq, Call, JsStr, NativeObject, ObjRef, Res, Rt, Tracer, Value};
use vn_dom::{EventInit, EventKind};

/// The log a probe handler writes to (held by the handler's data slot).
#[derive(Default)]
struct Log(RefCell<Vec<String>>);

impl NativeObject for Log {
    fn trace(&self, _t: &mut Tracer) {}
}

fn probe_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let holder = data[0].as_object().expect("log holder");
    let payload = rt.native_payload(holder).expect("log payload");
    let log = payload_ref::<Log>(&*payload).expect("log");
    let e = c.arg(0);
    let mut parts = Vec::new();
    for key in ["key", "button", "type"] {
        let has = rt.in_operator(&Value::String(JsStr::from_str(key)), &e)?;
        parts.push(format!("{key}:{has}"));
    }
    log.0.borrow_mut().push(parts.join(","));
    Ok(Value::Undefined)
}

/// A handler that records which of `key`, `button` and `type` its event object has (`in` semantics).
fn probe(rt: &Rt) -> (Value, ObjRef) {
    let holder = rt.new_native_object(None, std::rc::Rc::new(Log::default()));
    let f = Value::Object(rt.new_native_function(probe_code, "probe", 1, vec![Value::Object(holder)]));
    rt.root(&f);
    (f, holder)
}

fn log_of(rt: &Rt, holder: ObjRef) -> Vec<String> {
    let payload = rt.native_payload(holder).expect("log payload");
    payload_ref::<Log>(&*payload).expect("log").0.borrow().clone()
}

fn setup() -> (Rt, DocHost, DomCtx) {
    let rt = Rt::new();
    let mut host = DocHost::new();
    host.install_body().expect("body");
    let ctx = DomCtx::new(&rt, &host);
    (rt, host, ctx)
}

#[test]
fn handlers_and_invokers_survive_collections_and_keep_running() {
    let (rt, mut host, ctx) = setup();
    let body = host.body().expect("body");
    let btn = host.create_element(HostNs::Html, "button", None).unwrap();
    host.insert(btn, body, None).unwrap();
    let (f, holder) = probe(&rt);
    patch_prop(&ctx, btn, "onClick", &Value::Null, &f, HostNs::Html).unwrap();
    // The invoker and the handler are reachable only through the side table and the listener registry.
    rt.collect();
    host.click(btn).unwrap();
    rt.collect();
    host.click(btn).unwrap();
    let log = log_of(&rt, holder);
    assert_eq!(log, vec!["key:false,button:true,type:true".to_string(), "key:false,button:true,type:true".to_string()]);
}

#[test]
fn released_nodes_take_their_side_table_entries_and_listeners() {
    let (rt, mut host, ctx) = setup();
    let body = host.body().expect("body");
    let div = host.create_element(HostNs::Html, "div", None).unwrap();
    let input = host.create_element(HostNs::Html, "input", None).unwrap();
    host.insert(div, body, None).unwrap();
    host.insert(input, div, None).unwrap();
    let (f, _holder) = probe(&rt);
    patch_prop(&ctx, input, "onInput", &Value::Null, &f, HostNs::Html).unwrap();
    patch_prop(&ctx, input, "value", &Value::Null, &Value::String(JsStr::from_str("v")), HostNs::Html).unwrap();
    patch_prop(&ctx, div, "onClick", &Value::Null, &f, HostNs::Html).unwrap();
    let (expandos, _handles) = ctx.side_table_entries();
    assert_eq!(expandos, 2, "the input and the div carry expandos");
    assert_eq!(ctx.listener_count(), 2);
    host.remove(div).unwrap();
    host.release(div);
    // the subtree is freed: the input's listener and expandos go with the div's
    assert_eq!(ctx.listener_count(), 0, "released listeners are dropped");
    assert_eq!(ctx.side_table_entries().0, 0, "released expandos are dropped");
    assert_eq!(host.tag_name(input), "", "the released child is stale too");
}

#[test]
fn replacing_a_handler_keeps_one_listener_and_runs_the_new_one() {
    let (rt, mut host, ctx) = setup();
    let body = host.body().expect("body");
    let btn = host.create_element(HostNs::Html, "button", None).unwrap();
    host.insert(btn, body, None).unwrap();
    let (first, first_log) = probe(&rt);
    let (second, second_log) = probe(&rt);
    patch_prop(&ctx, btn, "onClick", &Value::Null, &first, HostNs::Html).unwrap();
    patch_prop(&ctx, btn, "onClick", &first, &second, HostNs::Html).unwrap();
    assert_eq!(ctx.listener_count(), 1, "the invoker is updated, not re-added");
    host.click(btn).unwrap();
    assert!(log_of(&rt, first_log).is_empty(), "the replaced handler does not run");
    assert_eq!(log_of(&rt, second_log).len(), 1);
    // removal detaches the listener
    patch_prop(&ctx, btn, "onClick", &second, &Value::Null, HostNs::Html).unwrap();
    assert_eq!(ctx.listener_count(), 0);
    host.click(btn).unwrap();
    assert_eq!(log_of(&rt, second_log).len(), 1);
}

#[test]
fn event_object_fields_follow_the_event_kind() {
    let (rt, mut host, ctx) = setup();
    let body = host.body().expect("body");
    let btn = host.create_element(HostNs::Html, "button", None).unwrap();
    host.insert(btn, body, None).unwrap();
    let (f, holder) = probe(&rt);
    patch_prop(&ctx, btn, "onKeydown", &Value::Null, &f, HostNs::Html).unwrap();
    patch_prop(&ctx, btn, "onClick", &Value::Null, &f, HostNs::Html).unwrap();
    let mut key = EventInit::new("keydown", EventKind::Keyboard);
    key.key = "Enter".into();
    host.dispatch_event(btn, key).unwrap();
    host.dispatch_event(btn, EventInit::new("click", EventKind::Mouse)).unwrap();
    host.dispatch_event(btn, EventInit::new("click", EventKind::Event)).unwrap();
    let log = log_of(&rt, holder);
    assert_eq!(log[0], "key:true,button:false,type:true", "keyboard events have key and no button");
    assert_eq!(log[1], "key:false,button:true,type:true", "mouse events have button and no key");
    assert_eq!(log[2], "key:false,button:false,type:true", "plain events have neither");
}

#[test]
fn with_modifiers_wrapper_is_cached_per_function_and_modifier_list() {
    let (rt, _host, _ctx) = setup();
    let all = exports::all(&rt);
    let get = |name: &str| all.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone()).expect(name);
    let with_modifiers = get("withModifiers");
    let (f, _holder) = probe(&rt);
    let mods = |names: &[&str]| Value::Object(rt.new_array(names.iter().map(|n| Value::String(JsStr::from_str(n))).collect()));
    let a = rt.call(&with_modifiers, &Value::Undefined, &[f.clone(), mods(&["stop"])]).unwrap();
    let b = rt.call(&with_modifiers, &Value::Undefined, &[f.clone(), mods(&["stop"])]).unwrap();
    let c = rt.call(&with_modifiers, &Value::Undefined, &[f.clone(), mods(&["prevent"])]).unwrap();
    assert!(strict_eq(&a, &b), "the same function and modifiers give the same wrapper");
    assert!(!strict_eq(&a, &c), "a different modifier list gives a different wrapper");
    let none = rt.call(&with_modifiers, &Value::Undefined, &[Value::Null, mods(&[])]).unwrap();
    assert!(matches!(none, Value::Null), "withModifiers(null) returns its argument, as runtime-dom does");
}

#[test]
fn with_keys_of_nothing_is_a_type_error() {
    let (rt, _host, _ctx) = setup();
    let all = exports::all(&rt);
    let with_keys = all.iter().find(|(n, _)| *n == "withKeys").map(|(_, v)| v.clone()).expect("withKeys");
    let empty = Value::Object(rt.new_array(Vec::new()));
    let err = rt.call(&with_keys, &Value::Undefined, &[Value::Undefined, empty]).expect_err("withKeys(undefined) throws");
    let message = match rt.get_str(&err.0, "message") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        other => panic!("no message: {other:?}"),
    };
    assert!(message.contains("_withKeys"), "{message}");
}

#[test]
fn with_keys_matches_hyphenated_names_and_aliases() {
    let (rt, mut host, ctx) = setup();
    let body = host.body().expect("body");
    let input = host.create_element(HostNs::Html, "input", None).unwrap();
    host.insert(input, body, None).unwrap();
    let all = exports::all(&rt);
    let with_keys = all.iter().find(|(n, _)| *n == "withKeys").map(|(_, v)| v.clone()).expect("withKeys");
    let (f, holder) = probe(&rt);
    let mods = |names: &[&str]| Value::Object(rt.new_array(names.iter().map(|n| Value::String(JsStr::from_str(n))).collect()));
    let wrapped = rt.call(&with_keys, &Value::Undefined, &[f, mods(&["page-up", "esc"])]).unwrap();
    patch_prop(&ctx, input, "onKeydown", &Value::Null, &wrapped, HostNs::Html).unwrap();
    for key in ["PageUp", "Escape", "PageDown"] {
        let mut init = EventInit::new("keydown", EventKind::Keyboard);
        init.key = key.into();
        host.dispatch_event(input, init).unwrap();
    }
    // three keydowns: PageUp and Escape reach the handler (alias esc), PageDown does not
    assert_eq!(log_of(&rt, holder).len(), 2);
}
