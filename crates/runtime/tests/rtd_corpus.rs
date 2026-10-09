//! The vn-dom corpora (tests/dom/corpora, recorded from headless Chrome) replayed through the `Host` trait on `DocHost`
//! (WP-RT-D). Each op of the corpus language maps onto a host method; listener behaviours run inside the event sink,
//! so nested operations and log actions re-enter the host while the dispatch is in progress. A case is replayed only
//! when every op it uses has a `Host` method; the others are counted and listed, never silently passed.

mod rtd_common;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rtd_common::{parse_json, Json};
use vn_dom::{EventInit, EventKind};
use vn_runtime::host::{EventSink, Host, HostError, HostEvent, HostNode, HostNs, HostValue, ListenerId, ListenerOpts, ListenerOutcome};
use vn_runtime::host_doc::DocHost;

const GROUPS: [&str; 11] = ["tree", "attr", "prop", "form", "style", "class", "event", "focus", "serial", "query", "activation"];

/// The ops the `Host` trait can express. Anything else (childNodes, firstChild, classList, byId, ...) is not replayed.
const MAPPED_OPS: &[&str] = &[
    "el", "elIs", "text", "comment", "insert", "append", "removeChild", "remove", "parent", "nextSibling", "tagName", "setNodeValue",
    "setTextContent", "setAttribute", "setAttributeNS", "getAttribute", "hasAttribute", "removeAttribute", "get", "set", "has",
    "innerHTML", "outerHTML", "snapshot", "styleSet", "styleRemove", "styleGet", "cssText", "setCssText", "classAdd", "classRemove",
    "listen", "unlisten", "dispatch", "click", "focus", "blur", "active", "options", "querySelector",
];

#[derive(Clone)]
enum Action {
    LogEv,
    LogKey,
    LogMouse,
    LogInput,
    LogActive,
    LogProp(String),
    Stop,
    StopImmediate,
    Prevent,
    Throw,
    Op(Json),
}

struct Case {
    name: String,
    ops: Vec<Json>,
    listeners: HashMap<u32, Vec<Action>>,
    steps: Vec<Json>,
}

fn parse_action(j: &Json) -> Action {
    let arr = j.as_arr().expect("listener action");
    match arr.first().and_then(Json::as_str).expect("action name") {
        "log" => match arr.get(1).and_then(Json::as_str) {
            Some("ev") => Action::LogEv,
            Some("key") => Action::LogKey,
            Some("mouse") => Action::LogMouse,
            Some("input") => Action::LogInput,
            Some("active") => Action::LogActive,
            Some("prop") => Action::LogProp(arr.get(2).and_then(Json::as_str).expect("prop name").to_string()),
            other => panic!("unknown log kind {other:?}"),
        },
        "stop" => Action::Stop,
        "stopImmediate" => Action::StopImmediate,
        "prevent" => Action::Prevent,
        "throw" => Action::Throw,
        "op" => Action::Op(arr.get(1).cloned().expect("op action")),
        other => panic!("unknown listener action {other}"),
    }
}

fn parse_case(j: &Json) -> Case {
    let name = j.get("name").and_then(Json::as_str).expect("case name").to_string();
    let ops = j.get("ops").and_then(Json::as_arr).expect("ops").clone();
    let steps = j.get("steps").and_then(Json::as_arr).expect("steps").clone();
    let mut listeners = HashMap::new();
    if let Some(Json::Obj(kv)) = j.get("listeners") {
        for (k, v) in kv {
            let id: u32 = k.parse().expect("listener id");
            listeners.insert(id, v.as_arr().expect("behaviour").iter().map(parse_action).collect());
        }
    }
    assert_eq!(ops.len(), steps.len(), "{name}: ops and steps differ");
    Case { name, ops, listeners, steps }
}

/// A failing op: a DOM error (its name is the result), or a harness problem (the replay cannot continue).
enum Fail {
    Host(HostError),
    Harness(String),
}

type R = Result<Json, Fail>;

fn harness<T>(msg: impl Into<String>) -> Result<T, Fail> {
    Err(Fail::Harness(msg.into()))
}

fn obj(kv: Vec<(&str, Json)>) -> Json {
    Json::Obj(kv.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn undef() -> Json {
    obj(vec![("$", Json::Str("undefined".into()))])
}

/// The dom name of an error (StaleNode for a released node; the names vn-dom reports).
fn error_name(e: &HostError) -> String {
    match e {
        HostError::NotFound => "StaleNode".to_string(),
        HostError::InvalidState(n) | HostError::Unsupported(n) => n.clone(),
    }
}

fn host_to_json(v: HostValue) -> Json {
    match v {
        HostValue::Undefined => undef(),
        HostValue::Null => Json::Null,
        HostValue::Bool(b) => Json::Bool(b),
        HostValue::Number(n) => Json::Num(n),
        HostValue::Str(s) => Json::Str(s),
    }
}

/// A JSON op argument as a HostValue ({"$":"undefined"} is undefined).
fn json_to_host(j: &Json) -> HostValue {
    match j {
        Json::Null => HostValue::Null,
        Json::Bool(b) => HostValue::Bool(*b),
        Json::Num(n) => HostValue::Number(*n),
        Json::Str(s) => HostValue::Str(s.clone()),
        _ => HostValue::Undefined,
    }
}

/// ToString of a host value (DOMString arguments: undefined becomes "undefined").
fn to_dom_string(v: &HostValue) -> String {
    match v {
        HostValue::Undefined => "undefined".to_string(),
        HostValue::Null => "null".to_string(),
        HostValue::Bool(b) => b.to_string(),
        HostValue::Number(n) => {
            if *n == 0.0 {
                "0".to_string()
            } else if n.fract() == 0.0 && n.abs() < 1e21 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        HostValue::Str(s) => s.clone(),
    }
}

/// WebIDL nullable DOMString (textContent, nodeValue): null and undefined become "".
fn nullable_empty(j: &Json) -> String {
    match json_to_host(j) {
        HostValue::Undefined | HostValue::Null => String::new(),
        other => to_dom_string(&other),
    }
}

/// Order-insensitive structural equality (the object keys of a step are compared as a set).
fn same(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Obj(x), Json::Obj(y)) => x.len() == y.len() && x.iter().all(|(k, v)| b.get(k).is_some_and(|w| same(v, w))),
        (Json::Arr(x), Json::Arr(y)) => x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| same(p, q)),
        _ => a == b,
    }
}

fn event_init(kind: &str, ty: &str, opts: Option<&Json>) -> Result<EventInit, Fail> {
    let kind = match kind {
        "Event" => EventKind::Event,
        "Mouse" => EventKind::Mouse,
        "Keyboard" => EventKind::Keyboard,
        "Input" => EventKind::Input,
        other => return harness(format!("unknown event kind {other}")),
    };
    let mut init = EventInit::new(ty, kind);
    let Some(o) = opts else { return Ok(init) };
    let flag = |k: &str| o.get(k).and_then(Json::as_bool).unwrap_or(false);
    let num = |k: &str| o.get(k).and_then(Json::as_num).unwrap_or(0.0) as i32;
    let text = |k: &str| o.get(k).and_then(Json::as_str).unwrap_or("").to_string();
    init.bubbles = flag("bubbles");
    init.cancelable = flag("cancelable");
    init.composed = flag("composed");
    init.key = text("key");
    init.code = text("code");
    init.ctrl_key = flag("ctrlKey");
    init.shift_key = flag("shiftKey");
    init.alt_key = flag("altKey");
    init.meta_key = flag("metaKey");
    init.button = num("button");
    init.client_x = num("clientX");
    init.client_y = num("clientY");
    init.detail = num("detail");
    init.input_data = o.get("data").and_then(Json::as_str).map(str::to_string);
    init.is_composing = flag("isComposing");
    Ok(init)
}

/// Replay state shared by the ops and the event sink: the host, the handle table and the step's log.
struct State {
    host: DocHost,
    handles: RefCell<Vec<HostNode>>,
    log: RefCell<Vec<Json>>,
    listeners: HashMap<u32, Vec<Action>>,
    passive: RefCell<HashMap<u32, bool>>,
    harness: RefCell<Option<String>>,
}

impl State {
    /// The handle of a node: its index in first-use order (the document is handle 0).
    fn handle(&self, n: HostNode) -> usize {
        let mut hs = self.handles.borrow_mut();
        match hs.iter().position(|&h| h == n) {
            Some(i) => i,
            None => {
                hs.push(n);
                hs.len() - 1
            }
        }
    }

    fn node(&self, n: HostNode) -> Json {
        obj(vec![("$", Json::Str("node".into())), ("h", Json::Num(self.handle(n) as f64))])
    }

    fn node_or_null(&self, n: Option<HostNode>) -> Json {
        match n {
            Some(n) => self.node(n),
            None => Json::Null,
        }
    }

    fn nodes(&self, ns: Vec<HostNode>) -> Json {
        Json::Arr(ns.into_iter().map(|n| self.node(n)).collect())
    }

    fn arg_node(&self, j: &Json) -> Result<HostNode, Fail> {
        match j.as_num() {
            Some(h) => self.handles.borrow().get(h as usize).copied().map_or_else(|| harness(format!("no such handle {h}")), Ok),
            None => harness("node argument must be a handle"),
        }
    }

    fn arg_opt_node(&self, j: &Json) -> Result<Option<HostNode>, Fail> {
        match j {
            Json::Null => Ok(None),
            other => self.arg_node(other).map(Some),
        }
    }
}

fn arg<'a>(a: &'a [Json], i: usize) -> Result<&'a Json, Fail> {
    a.get(i).map_or_else(|| harness(format!("missing argument {i}")), Ok)
}

fn arg_str<'a>(a: &'a [Json], i: usize) -> Result<&'a str, Fail> {
    a.get(i).and_then(Json::as_str).map_or_else(|| harness(format!("argument {i} must be a string")), Ok)
}

fn arg_bool(a: &[Json], i: usize) -> Result<bool, Fail> {
    a.get(i).and_then(Json::as_bool).map_or_else(|| harness(format!("argument {i} must be a boolean")), Ok)
}

fn host_err(e: HostError) -> Fail {
    Fail::Host(e)
}

/// The log entry of one listener action (the field set and the values of the page runner).
fn log_entry(st: &State, id: u32, ev: &HostEvent, action: &Action, prevented: bool) -> Result<Json, Fail> {
    Ok(match action {
        Action::LogEv => {
            let cur = match ev.current_target {
                Some(n) => Json::Num(st.handle(n) as f64),
                None => Json::Null,
            };
            let tgt = match ev.target {
                Some(n) => Json::Num(st.handle(n) as f64),
                None => Json::Null,
            };
            obj(vec![
                ("id", Json::Num(f64::from(id))),
                ("type", Json::Str(ev.ty.clone())),
                ("cur", cur),
                ("tgt", tgt),
                ("phase", Json::Num(f64::from(ev.phase))),
                ("prevented", Json::Bool(prevented)),
            ])
        }
        Action::LogKey => obj(vec![
            ("id", Json::Num(f64::from(id))),
            ("key", Json::Str(ev.key.clone())),
            ("code", Json::Str(ev.code.clone())),
            ("ctrl", Json::Bool(ev.ctrl_key)),
            ("shift", Json::Bool(ev.shift_key)),
            ("alt", Json::Bool(ev.alt_key)),
            ("meta", Json::Bool(ev.meta_key)),
        ]),
        Action::LogMouse => obj(vec![
            ("id", Json::Num(f64::from(id))),
            ("button", Json::Num(f64::from(ev.button))),
            ("x", Json::Num(ev.client_x)),
            ("y", Json::Num(ev.client_y)),
            ("detail", Json::Num(f64::from(ev.detail))),
        ]),
        Action::LogInput => obj(vec![
            ("id", Json::Num(f64::from(id))),
            ("data", ev.input_data.clone().map_or(Json::Null, Json::Str)),
            ("composing", Json::Bool(ev.is_composing)),
        ]),
        Action::LogActive => {
            let active = st.host.clone().active_element();
            obj(vec![("id", Json::Num(f64::from(id))), ("active", st.node_or_null(active))])
        }
        Action::LogProp(name) => {
            let target = ev.current_target.or(ev.target).ok_or_else(|| Fail::Harness("no target".into()))?;
            let value = st.host.clone().get_property(target, name).map_err(|e| match e {
                HostError::Unsupported(_) => Fail::Harness(format!("log prop {name} is unsupported")),
                other => Fail::Host(other),
            })?;
            obj(vec![("id", Json::Num(f64::from(id))), ("prop", Json::Str(name.clone())), ("value", host_to_json(value))])
        }
        other => return harness(format!("{} is not a log action", match other {
            Action::Stop => "stop",
            Action::StopImmediate => "stopImmediate",
            Action::Prevent => "prevent",
            Action::Throw => "throw",
            Action::Op(_) => "op",
            _ => "log",
        })),
    })
}

/// Runs one listener: its actions in order. A throw (or a failed nested op) stops the actions and reports `threw`.
fn run_listener(st: &State, id: ListenerId, ev: &HostEvent) -> ListenerOutcome {
    let actions = st.listeners.get(&id.0).cloned().unwrap_or_default();
    let passive = st.passive.borrow().get(&id.0).copied().unwrap_or(false);
    let mut out = ListenerOutcome::default();
    // default_prevented as this listener sees it: the dispatch's state plus the prevent actions run so far
    let mut prevented = ev.default_prevented;
    for action in actions {
        match action {
            Action::Stop => out.stop_propagation = true,
            Action::StopImmediate => {
                out.stop_immediate = true;
                out.stop_propagation = true;
            }
            Action::Prevent => {
                if ev.cancelable && !passive {
                    prevented = true;
                    out.prevent_default = true;
                }
            }
            Action::Throw => {
                out.threw = true;
                return out;
            }
            Action::Op(op) => {
                if apply(st, &op).is_err() {
                    out.threw = true;
                    return out;
                }
            }
            log => match log_entry(st, id.0, ev, &log, prevented) {
                Ok(entry) => st.log.borrow_mut().push(entry),
                Err(Fail::Harness(m)) => {
                    st.harness.borrow_mut().get_or_insert(m);
                    out.threw = true;
                    return out;
                }
                Err(Fail::Host(_)) => {
                    out.threw = true;
                    return out;
                }
            },
        }
    }
    out
}

struct Sink(Rc<State>);

impl EventSink for Sink {
    fn dispatch(&self, listener: ListenerId, event: &HostEvent) -> ListenerOutcome {
        run_listener(&self.0, listener, event)
    }
}

/// Applies one op (page-run / replay.rs `apply`), through the host.
fn apply(st: &State, op: &Json) -> R {
    let arr = op.as_arr().map_or_else(|| harness("op is not an array"), Ok)?;
    let name = arr.first().and_then(Json::as_str).map_or_else(|| harness("op has no name"), Ok)?;
    let a = &arr[1..];
    let mut host = st.host.clone();
    Ok(match name {
        "el" => {
            let ns = match arg_str(a, 0)? {
                "html" => HostNs::Html,
                "svg" => HostNs::Svg,
                "math" => HostNs::MathMl,
                other => return harness(format!("unknown namespace {other}")),
            };
            let n = host.create_element(ns, arg_str(a, 1)?, None).map_err(host_err)?;
            st.node(n)
        }
        "elIs" => {
            let n = host.create_element(HostNs::Html, arg_str(a, 0)?, Some(arg_str(a, 1)?)).map_err(host_err)?;
            st.node(n)
        }
        "text" => {
            let n = host.create_text(arg_str(a, 0)?);
            st.node(n)
        }
        "comment" => {
            let n = host.create_comment(arg_str(a, 0)?);
            st.node(n)
        }
        "insert" => {
            let p = st.arg_node(arg(a, 0)?)?;
            let c = st.arg_node(arg(a, 1)?)?;
            let anchor = st.arg_opt_node(arg(a, 2)?)?;
            host.insert(c, p, anchor).map_err(host_err)?;
            st.node(c)
        }
        "append" => {
            let p = st.arg_node(arg(a, 0)?)?;
            let c = st.arg_node(arg(a, 1)?)?;
            host.insert(c, p, None).map_err(host_err)?;
            st.node(c)
        }
        "removeChild" => {
            let p = st.arg_node(arg(a, 0)?)?;
            let c = st.arg_node(arg(a, 1)?)?;
            // removeChild names a child of its parent: anything else is NotFoundError (Host::remove is detach)
            if host.parent_node(c) != Some(p) {
                return Err(Fail::Host(HostError::InvalidState("NotFoundError".into())));
            }
            host.remove(c).map_err(host_err)?;
            st.node(c)
        }
        "remove" => {
            let c = st.arg_node(arg(a, 0)?)?;
            host.remove(c).map_err(host_err)?;
            undef()
        }
        "parent" => {
            let n = st.arg_node(arg(a, 0)?)?;
            let p = host.parent_node(n);
            st.node_or_null(p)
        }
        "nextSibling" => {
            let n = st.arg_node(arg(a, 0)?)?;
            let s = host.next_sibling(n);
            st.node_or_null(s)
        }
        "tagName" => {
            let n = st.arg_node(arg(a, 0)?)?;
            if host.is_element(n) {
                Json::Str(host.tag_name(n))
            } else {
                Json::Null
            }
        }
        "setNodeValue" => {
            let n = st.arg_node(arg(a, 0)?)?;
            host.set_text(n, &nullable_empty(arg(a, 1)?)).map_err(host_err)?;
            undef()
        }
        "setTextContent" => {
            let n = st.arg_node(arg(a, 0)?)?;
            host.set_element_text(n, &nullable_empty(arg(a, 1)?)).map_err(host_err)?;
            undef()
        }
        "setAttribute" => {
            let n = st.arg_node(arg(a, 0)?)?;
            host.set_attribute(n, arg_str(a, 1)?, &to_dom_string(&json_to_host(arg(a, 2)?))).map_err(host_err)?;
            undef()
        }
        "setAttributeNS" => {
            let n = st.arg_node(arg(a, 0)?)?;
            let ns = match arg(a, 1)? {
                Json::Null => None,
                other => other.as_str(),
            };
            host.set_attribute_ns(n, ns, arg_str(a, 2)?, &to_dom_string(&json_to_host(arg(a, 3)?))).map_err(host_err)?;
            undef()
        }
        "getAttribute" => {
            let n = st.arg_node(arg(a, 0)?)?;
            match host.get_attribute(n, arg_str(a, 1)?) {
                Some(v) => Json::Str(v),
                None => Json::Null,
            }
        }
        "hasAttribute" => {
            let n = st.arg_node(arg(a, 0)?)?;
            Json::Bool(host.has_attribute(n, arg_str(a, 1)?))
        }
        "removeAttribute" => {
            let n = st.arg_node(arg(a, 0)?)?;
            host.remove_attribute(n, arg_str(a, 1)?).map_err(host_err)?;
            undef()
        }
        "get" => {
            let n = st.arg_node(arg(a, 0)?)?;
            host_to_json(host.get_property(n, arg_str(a, 1)?).map_err(host_err)?)
        }
        "set" => {
            let n = st.arg_node(arg(a, 0)?)?;
            host.set_property(n, arg_str(a, 1)?, json_to_host(arg(a, 2)?)).map_err(host_err)?;
            undef()
        }
        "has" => {
            let n = st.arg_node(arg(a, 0)?)?;
            Json::Bool(host.has_property(n, arg_str(a, 1)?))
        }
        "innerHTML" => {
            let n = st.arg_node(arg(a, 0)?)?;
            match host.inner_html(n) {
                Some(s) => Json::Str(s),
                None => undef(),
            }
        }
        "outerHTML" => {
            let n = st.arg_node(arg(a, 0)?)?;
            match host.outer_html(n) {
                Some(s) => Json::Str(s),
                None => undef(),
            }
        }
        "snapshot" => {
            let n = st.arg_node(arg(a, 0)?)?;
            Json::Str(host.snapshot(n).map_err(host_err)?)
        }
        "styleSet" => {
            let n = st.arg_node(arg(a, 0)?)?;
            host.style_set_property(n, arg_str(a, 1)?, arg_str(a, 2)?, arg_bool(a, 3)?).map_err(host_err)?;
            undef()
        }
        "styleRemove" => {
            let n = st.arg_node(arg(a, 0)?)?;
            let name = arg_str(a, 1)?;
            let old = host.style_get_property(n, name);
            host.style_remove_property(n, name).map_err(host_err)?;
            Json::Str(old)
        }
        "styleGet" => {
            let n = st.arg_node(arg(a, 0)?)?;
            Json::Str(host.style_get_property(n, arg_str(a, 1)?))
        }
        "cssText" => {
            let n = st.arg_node(arg(a, 0)?)?;
            Json::Str(host.style_css_text(n))
        }
        "setCssText" => {
            let n = st.arg_node(arg(a, 0)?)?;
            host.style_set_css_text(n, arg_str(a, 1)?).map_err(host_err)?;
            undef()
        }
        "classAdd" | "classRemove" => {
            let n = st.arg_node(arg(a, 0)?)?;
            for j in &a[1..] {
                let Some(t) = j.as_str() else { return harness("class token must be a string") };
                if name == "classAdd" {
                    host.class_list_add(n, t).map_err(host_err)?;
                } else {
                    host.class_list_remove(n, t).map_err(host_err)?;
                }
            }
            undef()
        }
        "listen" => {
            let n = st.arg_node(arg(a, 0)?)?;
            let id = arg(a, 2)?.as_num().unwrap_or(-1.0) as u32;
            let opts = ListenerOpts { capture: arg_bool(a, 3)?, once: arg_bool(a, 4)?, passive: arg_bool(a, 5)? };
            st.passive.borrow_mut().insert(id, opts.passive);
            host.add_event_listener(n, arg_str(a, 1)?, ListenerId(id), opts).map_err(host_err)?;
            undef()
        }
        "unlisten" => {
            let n = st.arg_node(arg(a, 0)?)?;
            let id = arg(a, 2)?.as_num().unwrap_or(-1.0) as u32;
            host.remove_event_listener(n, arg_str(a, 1)?, ListenerId(id), arg_bool(a, 3)?).map_err(host_err)?;
            undef()
        }
        "dispatch" => {
            let n = st.arg_node(arg(a, 0)?)?;
            let init = event_init(arg_str(a, 1)?, arg_str(a, 2)?, a.get(3))?;
            let prevented = host.dispatch_event(n, init).map_err(host_err)?;
            Json::Bool(!prevented)
        }
        "click" => {
            let n = st.arg_node(arg(a, 0)?)?;
            host.click(n).map_err(host_err)?;
            undef()
        }
        "focus" => {
            let n = st.arg_node(arg(a, 0)?)?;
            host.focus(n).map_err(host_err)?;
            undef()
        }
        "blur" => {
            let n = st.arg_node(arg(a, 0)?)?;
            host.blur(n).map_err(host_err)?;
            undef()
        }
        "active" => {
            let x = host.active_element();
            st.node_or_null(x)
        }
        "querySelector" => {
            // the Host query is document-scoped: only the document root is replayed
            let root = st.arg_node(arg(a, 0)?)?;
            if root != HostNode(0) {
                return harness("querySelector below the document is not a Host operation");
            }
            let found = host.query_selector(arg_str(a, 1)?).map_err(host_err)?;
            st.node_or_null(found)
        }
        "options" => {
            let n = st.arg_node(arg(a, 0)?)?;
            let opts = host.option_nodes(n);
            st.nodes(opts)
        }
        other => return harness(format!("unknown op {other}")),
    })
}

/// Cases that use a call the Host trait cannot express, by name. `class/add-no-tokens-creates-attribute` calls
/// `classList.add()` with no tokens, which creates the class attribute in Chrome and in vn-dom. `Host::class_list_add`
/// takes one token, and Vue never calls it with zero tokens, so the case is excluded and reported with the others.
const NOT_HOST_EXPRESSIBLE: &[&str] = &["class/add-no-tokens-creates-attribute"];

/// Whether every op of the case has a Host method (and the document-scoped querySelector form).
fn replayable(case: &Case) -> Result<(), String> {
    if NOT_HOST_EXPRESSIBLE.contains(&case.name.as_str()) {
        return Err("classList.add() with no tokens".to_string());
    }
    for op in &case.ops {
        let name = op.as_arr().and_then(|a| a.first()).and_then(Json::as_str).unwrap_or("");
        if !MAPPED_OPS.contains(&name) {
            return Err(name.to_string());
        }
        if name == "querySelector" && op.as_arr().and_then(|a| a.get(1)).and_then(Json::as_num) != Some(0.0) {
            return Err("querySelector below the document".to_string());
        }
    }
    Ok(())
}

/// Replays one case on a fresh DocHost. Returns the first step that differs from the recording.
fn replay(case: &Case) -> Result<(), String> {
    let host = DocHost::new();
    let st = Rc::new(State {
        host: host.clone(),
        handles: RefCell::new(vec![HostNode(0)]),
        log: RefCell::new(Vec::new()),
        listeners: case.listeners.clone(),
        passive: RefCell::new(HashMap::new()),
        harness: RefCell::new(None),
    });
    let mut h = host.clone();
    h.set_event_sink(Rc::new(Sink(Rc::clone(&st))));
    for (i, (op, expected)) in case.ops.iter().zip(case.steps.iter()).enumerate() {
        let describe = |actual: &str| format!("{} op {i} {}: expected {} got {}", case.name, op.render(), expected.render(), actual);
        st.log.borrow_mut().clear();
        let result = match apply(&st, op) {
            Ok(v) => v,
            Err(Fail::Host(e)) => obj(vec![("$", Json::Str("error".into())), ("name", Json::Str(error_name(&e)))]),
            Err(Fail::Harness(m)) => return Err(describe(&format!("harness error: {m}"))),
        };
        if let Some(m) = st.harness.borrow_mut().take() {
            return Err(describe(&format!("harness error: {m}")));
        }
        let log = std::mem::take(&mut *st.log.borrow_mut());
        let actual = obj(vec![("r", result), ("log", Json::Arr(log))]);
        if !same(&actual, expected) {
            return Err(describe(&actual.render()));
        }
    }
    Ok(())
}

fn load_group(group: &str) -> Vec<Case> {
    let path = format!("{}/../../tests/dom/corpora/{group}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let root = parse_json(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    root.as_arr().expect("corpus array").iter().map(parse_case).collect()
}

#[test]
fn dom_corpora_replay_identically_through_the_host_trait() {
    let mut replayed = 0;
    let mut total = 0;
    let mut unmapped: HashMap<String, usize> = HashMap::new();
    let mut failures = Vec::new();
    for group in GROUPS {
        for case in load_group(group) {
            total += 1;
            match replayable(&case) {
                Err(op) => *unmapped.entry(op).or_insert(0) += 1,
                Ok(()) => {
                    replayed += 1;
                    if let Err(msg) = replay(&case) {
                        failures.push(msg);
                    }
                }
            }
        }
    }
    let mut listing: Vec<String> = unmapped.iter().map(|(k, v)| format!("{k}:{v}")).collect();
    listing.sort();
    eprintln!("dom corpora through Host: {replayed} of {total} cases replayed; not replayable (ops without a Host method): {}", listing.join(" "));
    assert!(failures.is_empty(), "{} cases diverge through the Host trait. First: {}", failures.len(), failures.iter().take(3).cloned().collect::<Vec<_>>().join("\n"));
    assert!(replayed >= 180, "only {replayed} cases are expressible through Host");
}

fn op_name(op: &Json) -> &str {
    op.as_arr().and_then(|a| a.first()).and_then(Json::as_str).unwrap_or("")
}

/// Negative control: a doctored recording (a property read) must be reported at exactly that op. The value written
/// into the recording is an object, which no recorded result can be.
#[test]
fn negative_control_doctored_dom_recording_is_reported() {
    let mut case = GROUPS
        .iter()
        .flat_map(|g| load_group(g))
        .find(|c| replayable(c).is_ok() && c.ops.iter().any(|o| op_name(o) == "get"))
        .expect("a replayable case with a property read");
    let i = case.ops.iter().position(|o| op_name(o) == "get").expect("get op");
    case.steps[i] = obj(vec![("r", obj(vec![("x", Json::Num(1.0))])), ("log", Json::Arr(Vec::new()))]);
    let err = replay(&case).expect_err("a doctored recording must not replay cleanly");
    assert!(err.contains(&format!("op {i} ")), "{err}");
}

/// Negative control: a listener behaviour changed in the case changes its log at the dispatch.
#[test]
fn negative_control_changed_listener_behaviour_is_reported() {
    let mut case = load_group("event").into_iter().find(|c| c.name == "event/path-capture-target-bubble").expect("fixture");
    case.listeners.insert(3, vec![Action::Stop]);
    assert!(replay(&case).is_err(), "a changed listener must change the event log");
}
