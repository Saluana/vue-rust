//! Replays the op language of tests/dom/gen/page-runner.mjs on vn-dom and compares every step with the Chrome
//! recording: the result of the op and the listener log entries it produced. The first divergence is reported with
//! the op index. Handles follow the runner: 0 is the document, and a node gets the next handle when it is first
//! encoded (creation, a returned node, or a log entry).

use std::collections::HashMap;

use vn_dom::{Document, DomError, EventInit, EventKind, EventState, HostValue, ListenerId, ListenerOptions, Namespace, NodeId};

use super::json::Json;

#[derive(Clone, Debug)]
pub enum Action {
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

/// One corpus case: the input ops and listener behaviours, and the expected steps.
#[derive(Clone, Debug)]
pub struct Case {
    pub name: String,
    pub ops: Vec<Json>,
    pub listeners: HashMap<u32, Vec<Action>>,
    pub steps: Vec<Json>,
}

#[derive(Debug)]
pub struct Divergence {
    pub case: String,
    pub op_index: usize,
    pub op: String,
    pub expected: String,
    pub actual: String,
}

enum Fail {
    Dom(DomError),
    Harness(String),
}

impl From<DomError> for Fail {
    fn from(e: DomError) -> Fail {
        Fail::Dom(e)
    }
}

type R = Result<Json, Fail>;

struct Cx {
    handles: Vec<NodeId>,
    log: Vec<Json>,
    listeners: HashMap<u32, Vec<Action>>,
    harness: Option<String>,
}

fn harness<T>(msg: impl Into<String>) -> Result<T, Fail> {
    Err(Fail::Harness(msg.into()))
}

/// The encoding of undefined (what a void op returns in the page).
fn undef() -> Json {
    obj(vec![("$", Json::Str("undefined".into()))])
}

fn obj(kv: Vec<(&str, Json)>) -> Json {
    Json::Obj(kv.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn host_to_json(v: HostValue) -> Json {
    match v {
        HostValue::Undefined => obj(vec![("$", Json::Str("undefined".into()))]),
        HostValue::Null => Json::Null,
        HostValue::Bool(b) => Json::Bool(b),
        HostValue::Number(n) => Json::Num(n),
        HostValue::Str(s) => Json::Str(s),
    }
}

/// A JSON op argument as a HostValue (the page runner's hv: the {"$":"undefined"} marker is undefined).
fn json_to_host(j: &Json) -> HostValue {
    match j {
        Json::Null => HostValue::Null,
        Json::Bool(b) => HostValue::Bool(*b),
        Json::Num(n) => HostValue::Number(*n),
        Json::Str(s) => HostValue::Str(s.clone()),
        Json::Obj(_) if j.get("$").and_then(Json::as_str) == Some("undefined") => HostValue::Undefined,
        _ => HostValue::Undefined,
    }
}

impl Cx {
    /// The handle of a node, registering it at the next index on first use.
    fn handle(&mut self, id: NodeId) -> usize {
        match self.handles.iter().position(|&h| h == id) {
            Some(i) => i,
            None => {
                self.handles.push(id);
                self.handles.len() - 1
            }
        }
    }

    fn node(&mut self, id: NodeId) -> Json {
        let h = self.handle(id);
        obj(vec![("$", Json::Str("node".into())), ("h", Json::Num(h as f64))])
    }

    fn node_or_null(&mut self, id: Option<NodeId>) -> Json {
        match id {
            Some(n) => self.node(n),
            None => Json::Null,
        }
    }

    fn nodes(&mut self, ids: Vec<NodeId>) -> Json {
        Json::Arr(ids.into_iter().map(|n| self.node(n)).collect())
    }

    /// An op argument that names a node by handle.
    fn arg_node(&self, j: &Json) -> Result<NodeId, Fail> {
        let Some(n) = j.as_num() else { return harness(format!("node argument is not a handle: {}", j.text())) };
        match self.handles.get(n as usize) {
            Some(&id) => Ok(id),
            None => harness(format!("no such handle {n}")),
        }
    }

    fn arg_opt_node(&self, j: &Json) -> Result<Option<NodeId>, Fail> {
        match j {
            Json::Null => Ok(None),
            other => self.arg_node(other).map(Some),
        }
    }
}

/// The listener callback for one call: built where it is passed, so `cx` is borrowed only for that call.
macro_rules! inv {
    ($cx:expr) => {
        &mut |d: &mut Document, id: ListenerId, ev: &mut EventState| run_listener(d, $cx, id, ev)
    };
}

fn arg_str<'a>(a: &'a [Json], i: usize) -> Result<&'a str, Fail> {
    match a.get(i).and_then(Json::as_str) {
        Some(s) => Ok(s),
        None => harness(format!("argument {i} must be a string")),
    }
}

fn arg_bool(a: &[Json], i: usize) -> Result<bool, Fail> {
    match a.get(i).and_then(Json::as_bool) {
        Some(b) => Ok(b),
        None => harness(format!("argument {i} must be a boolean")),
    }
}

fn arg<'a>(a: &'a [Json], i: usize) -> Result<&'a Json, Fail> {
    match a.get(i) {
        Some(j) => Ok(j),
        None => harness(format!("missing argument {i}")),
    }
}

/// WebIDL nullable DOMString (textContent, nodeValue): null and undefined become "".
fn nullable_empty(j: &Json) -> String {
    json_to_host(j).to_nullable_string()
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

/// A listener log entry, with the keys in the order the page runner builds them (handles are taken in that order).
fn log_entry(doc: &mut Document, cx: &mut Cx, id: u32, ev: &EventState, action: &Action) -> Result<Json, Fail> {
    Ok(match action {
        Action::LogEv => {
            let cur = match ev.current_target {
                Some(n) => Json::Num(cx.handle(n) as f64),
                None => Json::Null,
            };
            let tgt = Json::Num(cx.handle(ev.target) as f64);
            obj(vec![
                ("id", Json::Num(f64::from(id))),
                ("type", Json::Str(ev.init.ty.clone())),
                ("cur", cur),
                ("tgt", tgt),
                ("phase", Json::Num(f64::from(ev.phase))),
                ("prevented", Json::Bool(ev.default_prevented())),
            ])
        }
        Action::LogKey => obj(vec![
            ("id", Json::Num(f64::from(id))),
            ("key", Json::Str(ev.init.key.clone())),
            ("code", Json::Str(ev.init.code.clone())),
            ("ctrl", Json::Bool(ev.init.ctrl_key)),
            ("shift", Json::Bool(ev.init.shift_key)),
            ("alt", Json::Bool(ev.init.alt_key)),
            ("meta", Json::Bool(ev.init.meta_key)),
        ]),
        Action::LogMouse => obj(vec![
            ("id", Json::Num(f64::from(id))),
            ("button", Json::Num(f64::from(ev.init.button))),
            ("x", Json::Num(f64::from(ev.init.client_x))),
            ("y", Json::Num(f64::from(ev.init.client_y))),
            ("detail", Json::Num(f64::from(ev.init.detail))),
        ]),
        Action::LogInput => obj(vec![
            ("id", Json::Num(f64::from(id))),
            ("data", ev.init.input_data.clone().map_or(Json::Null, Json::Str)),
            ("composing", Json::Bool(ev.init.is_composing)),
        ]),
        Action::LogActive => {
            let active = match doc.active_element() {
                Some(n) => Json::Num(cx.handle(n) as f64),
                None => Json::Null,
            };
            obj(vec![("id", Json::Num(f64::from(id))), ("active", active)])
        }
        Action::LogProp(name) => {
            let target = ev.current_target.unwrap_or(ev.target);
            let value = match doc.get_property(target, name) {
                Ok(v) => host_to_json(v),
                Err(DomError::UnsupportedProperty) => return harness(format!("log prop {name} is unsupported")),
                Err(e) => return Err(Fail::Dom(e)),
            };
            obj(vec![("id", Json::Num(f64::from(id))), ("prop", Json::Str(name.clone())), ("value", value)])
        }
        other => return harness(format!("{other:?} is not a log action")),
    })
}

/// Runs one listener: its actions in order. Returns false when it throws (a Throw action, or a failed nested op).
fn run_listener(doc: &mut Document, cx: &mut Cx, id: ListenerId, ev: &mut EventState) -> bool {
    let actions = cx.listeners.get(&id.0).cloned().unwrap_or_default();
    for action in actions {
        match action {
            Action::Stop => ev.stop_propagation(),
            Action::StopImmediate => ev.stop_immediate_propagation(),
            Action::Prevent => ev.prevent_default(),
            Action::Throw => return false,
            Action::Op(op) => match apply(doc, cx, &op) {
                Ok(_) => {}
                Err(Fail::Dom(_)) => return false,
                Err(Fail::Harness(m)) => {
                    cx.harness.get_or_insert(m);
                    return false;
                }
            },
            log => match log_entry(doc, cx, id.0, ev, &log) {
                Ok(entry) => cx.log.push(entry),
                Err(Fail::Dom(_)) => return false,
                Err(Fail::Harness(m)) => {
                    cx.harness.get_or_insert(m);
                    return false;
                }
            },
        }
    }
    true
}

/// Applies one op. The result is the encoded value, or the DOM error (or a harness error for a malformed op).
fn apply(doc: &mut Document, cx: &mut Cx, op: &Json) -> R {
    let Some(arr) = op.as_arr() else { return harness("op is not an array") };
    let Some(name) = arr.first().and_then(Json::as_str) else { return harness("op has no name") };
    let a = &arr[1..];
    Ok(match name {
        "el" => {
            let ns = match arg_str(a, 0)? {
                "html" => Namespace::Html,
                "svg" => Namespace::Svg,
                "math" => Namespace::MathMl,
                other => return harness(format!("unknown namespace {other}")),
            };
            let id = doc.create_element(ns, arg_str(a, 1)?, None)?;
            cx.node(id)
        }
        "elIs" => {
            let id = doc.create_element(Namespace::Html, arg_str(a, 0)?, Some(arg_str(a, 1)?))?;
            cx.node(id)
        }
        "text" => {
            let id = doc.create_text(arg_str(a, 0)?);
            cx.node(id)
        }
        "comment" => {
            let id = doc.create_comment(arg_str(a, 0)?);
            cx.node(id)
        }
        "insert" => {
            let (p, c) = (cx.arg_node(arg(a, 0)?)?, cx.arg_node(arg(a, 1)?)?);
            let anchor = cx.arg_opt_node(arg(a, 2)?)?;
            doc.insert_before(p, c, anchor, inv!(cx))?;
            cx.node(c)
        }
        "append" => {
            let (p, c) = (cx.arg_node(arg(a, 0)?)?, cx.arg_node(arg(a, 1)?)?);
            doc.append_child(p, c, inv!(cx))?;
            cx.node(c)
        }
        "removeChild" => {
            let (p, c) = (cx.arg_node(arg(a, 0)?)?, cx.arg_node(arg(a, 1)?)?);
            doc.remove_child(p, c, inv!(cx))?;
            cx.node(c)
        }
        "remove" => {
            let c = cx.arg_node(arg(a, 0)?)?;
            doc.detach(c, inv!(cx))?;
            undef()
        }
        "parent" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let p = doc.parent_node(n);
            cx.node_or_null(p)
        }
        "firstChild" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let x = doc.first_child(n);
            cx.node_or_null(x)
        }
        "lastChild" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let x = doc.last_child(n);
            cx.node_or_null(x)
        }
        "nextSibling" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let x = doc.next_sibling(n);
            cx.node_or_null(x)
        }
        "previousSibling" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let x = doc.previous_sibling(n);
            cx.node_or_null(x)
        }
        "childNodes" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let kids = doc.child_nodes(n);
            cx.nodes(kids)
        }
        "contains" => {
            let (x, y) = (cx.arg_node(arg(a, 0)?)?, cx.arg_node(arg(a, 1)?)?);
            Json::Bool(doc.contains(x, y))
        }
        "isConnected" => Json::Bool(doc.is_connected(cx.arg_node(arg(a, 0)?)?)),
        "root" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let r = doc.root_of(n);
            cx.node(r)
        }
        "nodeName" => Json::Str(doc.node_name(cx.arg_node(arg(a, 0)?)?).unwrap_or_default()),
        "nodeType" => Json::Num(match doc.kind(cx.arg_node(arg(a, 0)?)?) {
            Some(vn_dom::NodeKind::Element) => 1.0,
            Some(vn_dom::NodeKind::Text) => 3.0,
            Some(vn_dom::NodeKind::Comment) => 8.0,
            Some(vn_dom::NodeKind::Document) => 9.0,
            None => 0.0,
        }),
        "localName" => match doc.local_name(cx.arg_node(arg(a, 0)?)?) {
            Some(s) => Json::Str(s.to_string()),
            None => Json::Null,
        },
        "tagName" => match doc.tag_name(cx.arg_node(arg(a, 0)?)?) {
            Some(s) => Json::Str(s),
            None => Json::Null,
        },
        "namespaceURI" => match doc.namespace(cx.arg_node(arg(a, 0)?)?) {
            Some(ns) => Json::Str(ns.uri().to_string()),
            None => Json::Null,
        },
        "nodeValue" => match doc.node_value(cx.arg_node(arg(a, 0)?)?) {
            Some(s) => Json::Str(s),
            None => Json::Null,
        },
        "setNodeValue" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            doc.set_node_value(n, &nullable_empty(arg(a, 1)?))?;
            undef()
        }
        "textContent" => match doc.text_content(cx.arg_node(arg(a, 0)?)?) {
            Some(s) => Json::Str(s),
            None => Json::Null,
        },
        "setTextContent" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            doc.set_text_content(n, &nullable_empty(arg(a, 1)?), inv!(cx))?;
            undef()
        }
        "setAttribute" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            doc.set_attribute(n, arg_str(a, 1)?, &json_to_host(arg(a, 2)?).to_dom_string())?;
            undef()
        }
        "setAttributeNS" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let ns = match arg(a, 1)? {
                Json::Null => None,
                other => other.as_str(),
            };
            doc.set_attribute_ns(n, ns, arg_str(a, 2)?, &json_to_host(arg(a, 3)?).to_dom_string())?;
            undef()
        }
        "getAttribute" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            match doc.get_attribute(n, arg_str(a, 1)?)? {
                Some(v) => Json::Str(v),
                None => Json::Null,
            }
        }
        "hasAttribute" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            Json::Bool(doc.has_attribute(n, arg_str(a, 1)?)?)
        }
        "removeAttribute" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            doc.remove_attribute(n, arg_str(a, 1)?)?;
            undef()
        }
        "attributeNames" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            Json::Arr(doc.attribute_names(n)?.into_iter().map(Json::Str).collect())
        }
        "get" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            host_to_json(doc.get_property(n, arg_str(a, 1)?)?)
        }
        "set" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let v = json_to_host(arg(a, 2)?);
            doc.set_property(n, arg_str(a, 1)?, v, inv!(cx))?;
            undef()
        }
        "has" => Json::Bool(doc.has_property(cx.arg_node(arg(a, 0)?)?, arg_str(a, 1)?)),
        "innerHTML" => match doc.inner_html(cx.arg_node(arg(a, 0)?)?) {
            Some(s) => Json::Str(s),
            None => host_to_json(HostValue::Undefined),
        },
        "outerHTML" => match doc.outer_html(cx.arg_node(arg(a, 0)?)?) {
            Some(s) => Json::Str(s),
            None => host_to_json(HostValue::Undefined),
        },
        "snapshot" => Json::Str(doc.snapshot(cx.arg_node(arg(a, 0)?)?)?),
        "styleSet" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            doc.style_set_property(n, arg_str(a, 1)?, arg_str(a, 2)?, arg_bool(a, 3)?)?;
            undef()
        }
        "styleCamel" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            doc.style_set_camel(n, arg_str(a, 1)?, arg_str(a, 2)?, false)?;
            undef()
        }
        "styleRemove" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            Json::Str(doc.style_remove_property(n, arg_str(a, 1)?)?)
        }
        "styleGet" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            Json::Str(doc.style_get_property(n, arg_str(a, 1)?)?)
        }
        "stylePriority" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            Json::Str(doc.style_get_priority(n, arg_str(a, 1)?)?)
        }
        "cssText" => Json::Str(doc.css_text(cx.arg_node(arg(a, 0)?)?)?),
        "setCssText" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            doc.set_css_text(n, arg_str(a, 1)?)?;
            undef()
        }
        "classAdd" | "classRemove" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let mut toks = Vec::new();
            for j in &a[1..] {
                match j.as_str() {
                    Some(t) => toks.push(t),
                    None => return harness("class token must be a string"),
                }
            }
            if name == "classAdd" {
                doc.class_list_add(n, &toks)?;
            } else {
                doc.class_list_remove(n, &toks)?;
            }
            undef()
        }
        "classContains" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            Json::Bool(doc.class_list_contains(n, arg_str(a, 1)?))
        }
        "classToggle" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            Json::Bool(doc.class_list_toggle(n, arg_str(a, 1)?, None)?)
        }
        "classToggleForce" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            Json::Bool(doc.class_list_toggle(n, arg_str(a, 1)?, Some(arg_bool(a, 2)?))?)
        }
        "classList" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            Json::Arr(doc.class_list(n).into_iter().map(Json::Str).collect())
        }
        "listen" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let opts = ListenerOptions { capture: arg_bool(a, 3)?, once: arg_bool(a, 4)?, passive: arg_bool(a, 5)? };
            let id = ListenerId(arg(a, 2)?.as_num().unwrap_or(-1.0) as u32);
            doc.add_event_listener(n, arg_str(a, 1)?, id, opts)?;
            undef()
        }
        "unlisten" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let id = ListenerId(arg(a, 2)?.as_num().unwrap_or(-1.0) as u32);
            doc.remove_event_listener(n, arg_str(a, 1)?, id, arg_bool(a, 3)?)?;
            undef()
        }
        "dispatch" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let init = event_init(arg_str(a, 1)?, arg_str(a, 2)?, a.get(3))?;
            let r = doc.dispatch_event(n, init, inv!(cx))?;
            Json::Bool(!r.default_prevented)
        }
        "click" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            doc.click(n, inv!(cx))?;
            undef()
        }
        "focus" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            doc.focus(n, inv!(cx))?;
            undef()
        }
        "blur" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            doc.blur(n, inv!(cx))?;
            undef()
        }
        "active" => {
            let x = doc.active_element();
            cx.node_or_null(x)
        }
        "byId" => {
            let found = doc.get_element_by_id(doc.document(), arg_str(a, 0)?);
            cx.node_or_null(found)
        }
        "querySelector" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let found = doc.query_selector(n, arg_str(a, 1)?)?;
            cx.node_or_null(found)
        }
        "options" => {
            let n = cx.arg_node(arg(a, 0)?)?;
            let opts = doc.option_ids(n);
            cx.nodes(opts)
        }
        other => return harness(format!("unknown op {other}")),
    })
}

/// Lists the log entries of a step as they are encoded: entries stored in `cx.log` during the op.
fn run_step(doc: &mut Document, cx: &mut Cx, op: &Json) -> Result<(Json, Vec<Json>), String> {
    cx.log.clear();
    let result = match apply(doc, cx, op) {
        Ok(v) => v,
        Err(Fail::Dom(e)) => obj(vec![("$", Json::Str("error".into())), ("name", Json::Str(e.name().into()))]),
        Err(Fail::Harness(m)) => return Err(m),
    };
    Ok((result, std::mem::take(&mut cx.log)))
}

fn parse_action(j: &Json) -> Result<Action, String> {
    let arr = j.as_arr().ok_or("listener action is not an array")?;
    let kind = arr.first().and_then(Json::as_str).ok_or("listener action has no name")?;
    Ok(match kind {
        "log" => match arr.get(1).and_then(Json::as_str) {
            Some("ev") => Action::LogEv,
            Some("key") => Action::LogKey,
            Some("mouse") => Action::LogMouse,
            Some("input") => Action::LogInput,
            Some("active") => Action::LogActive,
            Some("prop") => Action::LogProp(arr.get(2).and_then(Json::as_str).ok_or("prop name")?.to_string()),
            other => return Err(format!("unknown log kind {other:?}")),
        },
        "stop" => Action::Stop,
        "stopImmediate" => Action::StopImmediate,
        "prevent" => Action::Prevent,
        "throw" => Action::Throw,
        "op" => Action::Op(arr.get(1).cloned().ok_or("op action without an op")?),
        other => return Err(format!("unknown listener action {other}")),
    })
}

/// Parses one case of a corpus file.
pub fn parse_case(j: &Json) -> Result<Case, String> {
    let name = j.get("name").and_then(Json::as_str).ok_or("case has no name")?.to_string();
    let ops = j.get("ops").and_then(Json::as_arr).ok_or("case has no ops")?.to_vec();
    let steps = j.get("steps").and_then(Json::as_arr).ok_or("case has no steps")?.to_vec();
    let mut listeners = HashMap::new();
    if let Some(Json::Obj(kv)) = j.get("listeners") {
        for (k, v) in kv {
            let id: u32 = k.parse().map_err(|_| format!("listener key {k} is not a number"))?;
            let actions = v.as_arr().ok_or("listener behaviour is not an array")?;
            listeners.insert(id, actions.iter().map(parse_action).collect::<Result<Vec<_>, _>>()?);
        }
    }
    if ops.len() != steps.len() {
        return Err(format!("{name}: {} ops but {} steps", ops.len(), steps.len()));
    }
    Ok(Case { name, ops, listeners, steps })
}

/// Replays a case on a fresh document. Every step must match its recording; the first mismatch is returned.
pub fn replay(case: &Case) -> Result<(), Divergence> {
    let mut doc = Document::new();
    let mut cx = Cx { handles: vec![doc.document()], log: Vec::new(), listeners: case.listeners.clone(), harness: None };
    for (i, (op, expected)) in case.ops.iter().zip(&case.steps).enumerate() {
        let (result, log) = match run_step(&mut doc, &mut cx, op) {
            Ok(step) => step,
            Err(m) => {
                return Err(Divergence { case: case.name.clone(), op_index: i, op: op.text(), expected: expected.text(), actual: format!("harness error: {m}") })
            }
        };
        let actual = obj(vec![("r", result), ("log", Json::Arr(log))]);
        if !actual.same(expected) {
            return Err(Divergence { case: case.name.clone(), op_index: i, op: op.text(), expected: expected.text(), actual: actual.text() });
        }
    }
    Ok(())
}
