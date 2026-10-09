//! Self-test driver (feature `selftest`, never in the shipped build). `call` takes a JSON array of operations and answers with a
//! JSON array of results, one per operation, so the browser tests in tests/platform/web drive the real `Host` trait through
//! the real imports. Each operation is `["method", arg, ...]`; hosts/web/ABI.md documents the argument encoding.
//!
//! Host results are `{"ok": value}` or `{"err": {"kind": "NotFound" | "Unsupported" | "InvalidState", "msg": ...}}`.
//! Harness problems (bad JSON, unknown method, bad argument) are `{"harness": "message"}` and never reach the host.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use vn_runtime::host::{
    EventSink, Host, HostError, HostEvent, HostNode, HostNs, HostResult, HostValue, ListenerId, ListenerOpts,
    ListenerOutcome,
};

use crate::json::{self, Json};
use crate::web_host::{self, WebHost};

/// What a registered test listener does when it runs: queued host operations (re-entrant calls) and an outcome.
#[derive(Clone, Default)]
struct Behaviour {
    actions: Vec<Json>,
    stop: bool,
    stop_immediate: bool,
    prevent: bool,
    throw: bool,
}

thread_local! {
    static BEHAVIOUR: RefCell<HashMap<u32, Behaviour>> = RefCell::new(HashMap::new());
    static LOG: RefCell<Vec<Json>> = const { RefCell::new(Vec::new()) };
    static FIRED: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    static INSTALLED: Cell<bool> = const { Cell::new(false) };
}

/// Records every delivery, then runs the listener's queued operations (the glue's re-entrant calls) and returns its outcome.
struct Recorder;

impl EventSink for Recorder {
    fn dispatch(&self, listener: ListenerId, event: &HostEvent) -> ListenerOutcome {
        LOG.with(|log| log.borrow_mut().push(event_json(listener, event)));
        let plan = BEHAVIOUR.with(|b| b.borrow().get(&listener.0).cloned());
        let Some(plan) = plan else { return ListenerOutcome::default() };
        for action in &plan.actions {
            let result = call_op(action);
            LOG.with(|log| log.borrow_mut().push(obj([("kind", s("action")), ("result", result)])));
        }
        ListenerOutcome {
            stop_propagation: plan.stop,
            stop_immediate: plan.stop_immediate,
            prevent_default: plan.prevent,
            threw: plan.throw,
        }
    }
}

fn ensure_installed() {
    if INSTALLED.with(|flag| flag.replace(true)) {
        return;
    }
    let mut host = WebHost::new();
    host.set_event_sink(Rc::new(Recorder));
    web_host::set_timer_hook(Rc::new(|id| FIRED.with(|f| f.borrow_mut().push(id))));
}

/// Runs a request (a JSON array of operations) and returns the JSON reply as bytes.
pub fn call(request: &[u8]) -> Vec<u8> {
    ensure_installed();
    let reply = match std::str::from_utf8(request).map_err(|e| e.to_string()).and_then(json::parse) {
        Ok(Json::Arr(ops)) => Json::Arr(ops.iter().map(call_op).collect()),
        Ok(_) => harness("the request must be an array of operations"),
        Err(e) => harness(&format!("bad request JSON: {e}")),
    };
    json::write(&reply).into_bytes()
}

fn s(text: &str) -> Json {
    Json::Str(text.to_string())
}

fn obj<const N: usize>(fields: [(&str, Json); N]) -> Json {
    Json::Obj(fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn harness(message: &str) -> Json {
    obj([("harness", s(message))])
}

fn ok(value: Json) -> Json {
    obj([("ok", value)])
}

fn node_json(node: HostNode) -> Json {
    Json::Num(node.0 as f64)
}

fn opt_node_json(node: Option<HostNode>) -> Json {
    node.map_or(Json::Null, node_json)
}

fn opt_str_json(text: Option<String>) -> Json {
    text.map_or(Json::Null, |t| Json::Str(t))
}

fn error_json(e: &HostError) -> Json {
    match e {
        HostError::NotFound => obj([("kind", s("NotFound"))]),
        HostError::Unsupported(m) => obj([("kind", s("Unsupported")), ("msg", s(m))]),
        HostError::InvalidState(m) => obj([("kind", s("InvalidState")), ("msg", s(m))]),
    }
}

/// Turns a host result into `{"ok": ...}` or `{"err": ...}`.
fn reply<T>(result: HostResult<T>, to_json: impl FnOnce(T) -> Json) -> Json {
    match result {
        Ok(v) => obj([("ok", to_json(v))]),
        Err(e) => obj([("err", error_json(&e))]),
    }
}

fn value_json(v: &HostValue) -> Json {
    match v {
        HostValue::Undefined => obj([("k", s("undef"))]),
        HostValue::Null => obj([("k", s("null"))]),
        HostValue::Bool(b) => obj([("k", s("bool")), ("v", Json::Bool(*b))]),
        HostValue::Number(n) => obj([("k", s("num")), ("v", Json::Num(*n))]),
        HostValue::Str(text) => obj([("k", s("str")), ("v", s(text))]),
    }
}

fn event_json(listener: ListenerId, e: &HostEvent) -> Json {
    obj([
        ("kind", s("event")),
        ("listener", Json::Num(listener.0 as f64)),
        ("ty", s(&e.ty)),
        ("target", opt_node_json(e.target)),
        ("current", opt_node_json(e.current_target)),
        ("phase", Json::Num(e.phase as f64)),
        ("bubbles", Json::Bool(e.bubbles)),
        ("cancelable", Json::Bool(e.cancelable)),
        ("composed", Json::Bool(e.composed)),
        ("default_prevented", Json::Bool(e.default_prevented)),
        ("time_stamp", Json::Num(e.time_stamp)),
        ("key", s(&e.key)),
        ("code", s(&e.code)),
        ("ctrl", Json::Bool(e.ctrl_key)),
        ("shift", Json::Bool(e.shift_key)),
        ("alt", Json::Bool(e.alt_key)),
        ("meta", Json::Bool(e.meta_key)),
        ("button", Json::Num(e.button as f64)),
        ("client_x", Json::Num(e.client_x)),
        ("client_y", Json::Num(e.client_y)),
        ("input_data", opt_str_json(e.input_data.clone())),
        ("is_composing", Json::Bool(e.is_composing)),
        ("detail", Json::Num(e.detail as f64)),
    ])
}

/// Argument accessors; every failure is a harness error (a mistake in the test, not a host result).
struct Args<'a>(&'a [Json]);

impl Args<'_> {
    fn get(&self, i: usize) -> Result<&Json, String> {
        self.0.get(i).ok_or_else(|| format!("missing argument {i}"))
    }

    fn num(&self, i: usize) -> Result<f64, String> {
        match self.get(i)? {
            Json::Num(n) => Ok(*n),
            _ => Err(format!("argument {i} must be a number")),
        }
    }

    fn handle(&self, i: usize) -> Result<HostNode, String> {
        let n = self.num(i)?;
        if n.fract() != 0.0 || !(0.0..4294967296.0).contains(&n) {
            return Err(format!("argument {i} is not a handle"));
        }
        Ok(HostNode(n as u32))
    }

    fn opt_handle(&self, i: usize) -> Result<Option<HostNode>, String> {
        match self.get(i)? {
            Json::Null => Ok(None),
            _ => self.handle(i).map(Some),
        }
    }

    fn text(&self, i: usize) -> Result<String, String> {
        match self.get(i)? {
            Json::Str(t) => Ok(t.clone()),
            _ => Err(format!("argument {i} must be a string")),
        }
    }

    fn opt_text(&self, i: usize) -> Result<Option<String>, String> {
        match self.get(i)? {
            Json::Null => Ok(None),
            _ => self.text(i).map(Some),
        }
    }

    fn flag(&self, i: usize) -> Result<bool, String> {
        match self.get(i)? {
            Json::Bool(b) => Ok(*b),
            _ => Err(format!("argument {i} must be a boolean")),
        }
    }

    fn namespace(&self, i: usize) -> Result<HostNs, String> {
        match self.text(i)?.as_str() {
            "html" => Ok(HostNs::Html),
            "svg" => Ok(HostNs::Svg),
            "mathml" => Ok(HostNs::MathMl),
            other => Err(format!("unknown namespace '{other}'")),
        }
    }

    fn value(&self, i: usize) -> Result<HostValue, String> {
        let v = self.get(i)?;
        let kind = v.get("k").and_then(Json::as_str).ok_or_else(|| format!("argument {i} is not a HostValue"))?;
        let payload = v.get("v");
        let number = || match payload {
            Some(Json::Num(n)) => Ok(*n),
            Some(Json::Str(t)) if t == "NaN" => Ok(f64::NAN),
            Some(Json::Str(t)) if t == "Infinity" => Ok(f64::INFINITY),
            Some(Json::Str(t)) if t == "-Infinity" => Ok(f64::NEG_INFINITY),
            _ => Err(format!("argument {i} needs a number")),
        };
        match kind {
            "undef" => Ok(HostValue::Undefined),
            "null" => Ok(HostValue::Null),
            "bool" => match payload {
                Some(Json::Bool(b)) => Ok(HostValue::Bool(*b)),
                _ => Err(format!("argument {i} needs a boolean")),
            },
            "num" => number().map(HostValue::Number),
            "str" => match payload {
                Some(Json::Str(t)) => Ok(HostValue::Str(t.clone())),
                _ => Err(format!("argument {i} needs a string")),
            },
            other => Err(format!("unknown HostValue kind '{other}'")),
        }
    }

    fn listener_opts(&self, i: usize) -> Result<ListenerOpts, String> {
        let o = self.get(i)?;
        let flag = |k: &str| o.get(k).and_then(Json::as_bool).unwrap_or(false);
        Ok(ListenerOpts { capture: flag("capture"), once: flag("once"), passive: flag("passive") })
    }
}

fn call_op(op: &Json) -> Json {
    let Some(items) = op.as_arr() else { return harness("an operation is a JSON array") };
    let Some(name) = items.first().and_then(Json::as_str) else {
        return harness("an operation starts with its method name");
    };
    let mut host = WebHost::new();
    match dispatch(&mut host, name, &Args(&items[1..])) {
        Ok(json) => json,
        Err(message) => harness(&format!("{name}: {message}")),
    }
}

fn dispatch(host: &mut WebHost, name: &str, a: &Args) -> Result<Json, String> {
    Ok(match name {
        "now_ms" => ok(Json::Num(host.now_ms())),
        "create_element" => {
            let (ns, tag, is) = (a.namespace(0)?, a.text(1)?, a.opt_text(2)?);
            reply(host.create_element(ns, &tag, is.as_deref()), node_json)
        }
        "create_text" => ok(node_json(host.create_text(&a.text(0)?))),
        "create_comment" => ok(node_json(host.create_comment(&a.text(0)?))),
        "set_text" => reply(host.set_text(a.handle(0)?, &a.text(1)?), |_| Json::Null),
        "set_element_text" => reply(host.set_element_text(a.handle(0)?, &a.text(1)?), |_| Json::Null),
        "insert" => reply(host.insert(a.handle(0)?, a.handle(1)?, a.opt_handle(2)?), |_| Json::Null),
        "remove" => reply(host.remove(a.handle(0)?), |_| Json::Null),
        "parent_node" => ok(opt_node_json(host.parent_node(a.handle(0)?))),
        "next_sibling" => ok(opt_node_json(host.next_sibling(a.handle(0)?))),
        "query_selector" => reply(host.query_selector(&a.text(0)?), opt_node_json),
        "body" => ok(opt_node_json(host.body())),
        "release" => {
            host.release(a.handle(0)?);
            ok(Json::Null)
        }
        "tag_name" => ok(s(&host.tag_name(a.handle(0)?))),
        "is_element" => ok(Json::Bool(host.is_element(a.handle(0)?))),
        "set_attribute" => reply(host.set_attribute(a.handle(0)?, &a.text(1)?, &a.text(2)?), |_| Json::Null),
        "set_attribute_ns" => {
            let ns = a.opt_text(1)?;
            reply(host.set_attribute_ns(a.handle(0)?, ns.as_deref(), &a.text(2)?, &a.text(3)?), |_| Json::Null)
        }
        "remove_attribute" => reply(host.remove_attribute(a.handle(0)?, &a.text(1)?), |_| Json::Null),
        "remove_attribute_ns" => {
            let ns = a.opt_text(1)?;
            reply(host.remove_attribute_ns(a.handle(0)?, ns.as_deref(), &a.text(2)?), |_| Json::Null)
        }
        "get_attribute" => ok(opt_str_json(host.get_attribute(a.handle(0)?, &a.text(1)?))),
        "has_attribute" => ok(Json::Bool(host.has_attribute(a.handle(0)?, &a.text(1)?))),
        "has_property" => ok(Json::Bool(host.has_property(a.handle(0)?, &a.text(1)?))),
        "get_property" => reply(host.get_property(a.handle(0)?, &a.text(1)?), |v| value_json(&v)),
        "set_property" => reply(host.set_property(a.handle(0)?, &a.text(1)?, a.value(2)?), |_| Json::Null),
        "style_set_property" => {
            let important = a.flag(3)?;
            reply(host.style_set_property(a.handle(0)?, &a.text(1)?, &a.text(2)?, important), |_| Json::Null)
        }
        "style_remove_property" => reply(host.style_remove_property(a.handle(0)?, &a.text(1)?), |_| Json::Null),
        "style_get_property" => ok(s(&host.style_get_property(a.handle(0)?, &a.text(1)?))),
        "style_css_text" => ok(s(&host.style_css_text(a.handle(0)?))),
        "style_set_css_text" => reply(host.style_set_css_text(a.handle(0)?, &a.text(1)?), |_| Json::Null),
        "class_list_add" => reply(host.class_list_add(a.handle(0)?, &a.text(1)?), |_| Json::Null),
        "class_list_remove" => reply(host.class_list_remove(a.handle(0)?, &a.text(1)?), |_| Json::Null),
        "add_event_listener" => {
            let id = ListenerId(a.handle(2)?.0);
            reply(host.add_event_listener(a.handle(0)?, &a.text(1)?, id, a.listener_opts(3)?), |_| Json::Null)
        }
        "remove_event_listener" => {
            let id = ListenerId(a.handle(2)?.0);
            reply(host.remove_event_listener(a.handle(0)?, &a.text(1)?, id, a.flag(3)?), |_| Json::Null)
        }
        "focus" => reply(host.focus(a.handle(0)?), |_| Json::Null),
        "blur" => reply(host.blur(a.handle(0)?), |_| Json::Null),
        "active_element" => ok(opt_node_json(host.active_element())),
        "option_nodes" => ok(Json::Arr(host.option_nodes(a.handle(0)?).into_iter().map(node_json).collect())),
        "outer_html" => ok(opt_str_json(host.outer_html(a.handle(0)?))),
        "inner_html" => ok(opt_str_json(host.inner_html(a.handle(0)?))),
        // ---- harness controls (not Host methods)
        "__behave" => {
            let id = a.handle(0)?.0;
            let spec = a.get(1)?;
            let actions = spec.get("actions").and_then(Json::as_arr).map(<[Json]>::to_vec).unwrap_or_default();
            let flag = |k: &str| spec.get(k).and_then(Json::as_bool).unwrap_or(false);
            let plan = Behaviour {
                actions,
                stop: flag("stop"),
                stop_immediate: flag("stop_immediate"),
                prevent: flag("prevent"),
                throw: flag("throw"),
            };
            BEHAVIOUR.with(|b| b.borrow_mut().insert(id, plan));
            ok(Json::Null)
        }
        "__log" => ok(Json::Arr(LOG.with(|log| std::mem::take(&mut *log.borrow_mut())))),
        "__timer_set" => {
            host.set_timeout(a.handle(0)?.0, a.num(1)?);
            ok(Json::Null)
        }
        "__timer_clear" => {
            host.clear_timeout(a.handle(0)?.0);
            ok(Json::Null)
        }
        "__timers_fired" => ok(Json::Arr(
            FIRED.with(|f| std::mem::take(&mut *f.borrow_mut())).into_iter().map(|id| Json::Num(id as f64)).collect(),
        )),
        "__log_message" => {
            host.log(crate::LogLevel::Info, &a.text(0)?);
            ok(Json::Null)
        }
        "__churn" => {
            churn(host, a.handle(0)?.0)?;
            ok(Json::Null)
        }
        "__panic" => panic!("selftest panic: {}", a.text(0)?),
        "__reset" => {
            BEHAVIOUR.with(|b| b.borrow_mut().clear());
            LOG.with(|l| l.borrow_mut().clear());
            FIRED.with(|f| f.borrow_mut().clear());
            ok(Json::Null)
        }
        other => return Err(format!("unknown method '{other}'")),
    })
}

/// `n` cycles of create, insert, remove and release (the memory-growth check). Each cycle leaves no handle behind.
fn churn(host: &mut WebHost, n: u32) -> Result<(), String> {
    let body = host.body().ok_or("the document has no body")?;
    let fail = |e: HostError| format!("{e:?}");
    for _ in 0..n {
        let el = host.create_element(HostNs::Html, "div", None).map_err(fail)?;
        let text = host.create_text("churn");
        host.insert(text, el, None).map_err(fail)?;
        host.insert(el, body, None).map_err(fail)?;
        host.remove(el).map_err(fail)?;
        host.release(text);
        host.release(el);
    }
    Ok(())
}
