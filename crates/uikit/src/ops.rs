//! The DOM operations behind the testHost functions, over the headless DocHost. They follow tooling/oracle/ui/page-host.mjs
//! function by function, with the oracle's error texts. Errors are values here (no JavaScript); js.rs turns them into throws.

use vn_dom::{EventInit, EventKind};
use vn_runtime::host::{Host, HostError, HostNode, HostNs, HostValue};
use vn_runtime::host_doc::DocHost;
use vn_testkit::json::Json;

use crate::index::Index;
use crate::selector::Selector;
use crate::snapshot;

/// A failed operation: a TypeError (wrong argument) or an Error (the host refused the operation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiError {
    Type(String),
    Error(String),
}

pub type UiResult<T> = Result<T, UiError>;

fn type_err<T>(msg: &str) -> UiResult<T> {
    Err(UiError::Type(msg.to_string()))
}

pub fn host_err(e: HostError) -> UiError {
    UiError::Error(match e {
        HostError::Unsupported(m) => format!("unsupported: {m}"),
        HostError::InvalidState(m) => format!("invalid DOM state: {m}"),
        HostError::NotFound => "The object can not be found here.".to_string(),
    })
}

/// The init members that dispatch accepts (page-host.mjs INIT_KEYS).
pub const INIT_KEYS: [&str; 12] = [
    "bubbles", "cancelable", "key", "code", "ctrlKey", "shiftKey", "altKey", "metaKey", "button", "detail", "data", "isComposing",
];

/// The init of one dispatched event, after the JavaScript side has converted each member. `None` means the member was
/// absent or nullish, so the browser default applies.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EventSpec {
    pub bubbles: Option<bool>,
    pub cancelable: Option<bool>,
    pub key: Option<String>,
    pub code: Option<String>,
    pub ctrl_key: bool,
    pub shift_key: bool,
    pub alt_key: bool,
    pub meta_key: bool,
    pub button: Option<i32>,
    pub detail: Option<i32>,
    pub data: Option<String>,
    pub is_composing: bool,
}

// Browser defaults for the events the host creates (page-host.mjs NO_BUBBLE, NO_CANCEL and the interface sets).
const NO_BUBBLE: [&str; 7] = ["focus", "blur", "mouseenter", "mouseleave", "load", "error", "scroll"];
const NO_CANCEL: [&str; 14] = [
    "focus", "blur", "focusin", "focusout", "mouseenter", "mouseleave", "input", "change", "compositionstart",
    "compositionupdate", "compositionend", "load", "error", "scroll",
];
const MOUSE: [&str; 11] = [
    "click", "dblclick", "mousedown", "mouseup", "mousemove", "mouseover", "mouseout", "mouseenter", "mouseleave",
    "contextmenu", "auxclick",
];
const KEY: [&str; 3] = ["keydown", "keyup", "keypress"];
const INPUT: [&str; 2] = ["input", "beforeinput"];

/// The event object the oracle's makeEvent builds for `ty` and `spec`, as vn-dom's init.
pub fn event_init(ty: &str, spec: &EventSpec) -> EventInit {
    let kind = if MOUSE.contains(&ty) {
        EventKind::Mouse
    } else if KEY.contains(&ty) {
        EventKind::Keyboard
    } else if INPUT.contains(&ty) {
        EventKind::Input
    } else {
        EventKind::Event
    };
    let mut init = EventInit::new(ty, kind);
    init.bubbles = spec.bubbles.unwrap_or(!NO_BUBBLE.contains(&ty));
    init.cancelable = spec.cancelable.unwrap_or(!NO_CANCEL.contains(&ty));
    init.composed = kind != EventKind::Event;
    init.key = spec.key.clone().unwrap_or_default();
    init.code = spec.code.clone().unwrap_or_default();
    init.ctrl_key = spec.ctrl_key;
    init.shift_key = spec.shift_key;
    init.alt_key = spec.alt_key;
    init.meta_key = spec.meta_key;
    init.button = spec.button.unwrap_or(0);
    init.detail = spec.detail.unwrap_or(if ty == "click" { 1 } else { 0 });
    init.input_data = spec.data.clone();
    init.is_composing = spec.is_composing;
    init
}

/// The tag name of an element, lowercase (DocHost returns upper case for HTML elements).
fn tag_of(h: &DocHost, el: HostNode) -> String {
    h.tag_name(el).to_ascii_lowercase()
}

fn prop_str(h: &mut DocHost, el: HostNode, name: &str) -> UiResult<String> {
    match h.get_property(el, name).map_err(host_err)? {
        HostValue::Str(s) => Ok(s),
        _ => Ok(String::new()),
    }
}

fn prop_bool(h: &mut DocHost, el: HostNode, name: &str) -> UiResult<bool> {
    Ok(matches!(h.get_property(el, name).map_err(host_err)?, HostValue::Bool(true)))
}

pub fn is_select(h: &DocHost, el: HostNode) -> bool {
    tag_of(h, el) == "select"
}

/// A text input (type text or range) or a textarea: the controls `input()` accepts.
fn text_like(h: &mut DocHost, el: HostNode) -> UiResult<bool> {
    let tag = tag_of(h, el);
    if tag == "textarea" {
        return Ok(true);
    }
    if tag != "input" {
        return Ok(false);
    }
    let ty = prop_str(h, el, "type")?;
    Ok(ty == "text" || ty == "range")
}

fn checkable(h: &mut DocHost, el: HostNode) -> UiResult<bool> {
    if tag_of(h, el) != "input" {
        return Ok(false);
    }
    let ty = prop_str(h, el, "type")?;
    Ok(ty == "checkbox" || ty == "radio")
}

/// `element.dispatchEvent(event)` for an event made by `event_init`, at fake time `now` (ms).
pub fn dispatch(h: &mut DocHost, el: HostNode, ty: &str, spec: &EventSpec, now: f64) -> UiResult<()> {
    h.set_now(now);
    h.dispatch_event(el, event_init(ty, spec)).map(|_| ()).map_err(host_err)
}

pub fn click(h: &mut DocHost, el: HostNode, now: f64) -> UiResult<()> {
    dispatch(h, el, "click", &EventSpec::default(), now)
}

/// `input(node, value)`: sets `value`, then dispatches a bubbling `input` event with the value as its data.
pub fn input(h: &mut DocHost, el: HostNode, value: Option<&str>, now: f64) -> UiResult<()> {
    if !text_like(h, el)? {
        return type_err("input: the target must be a text input, a range or a textarea");
    }
    let Some(value) = value else { return type_err("input: value must be a string") };
    set_value(h, el, value)?;
    dispatch(h, el, "input", &EventSpec { data: Some(value.to_string()), ..EventSpec::default() }, now)
}

/// `change(node, value)` for a text input, a range or a textarea.
pub fn change_text(h: &mut DocHost, el: HostNode, value: Option<&str>, now: f64) -> UiResult<()> {
    if !text_like(h, el)? {
        return type_err("change: the target must be a text input, a range, a textarea or a select");
    }
    let Some(value) = value else { return type_err("change: value must be a string") };
    set_value(h, el, value)?;
    dispatch(h, el, "change", &EventSpec::default(), now)
}

/// `select(node, value)`: `value` is a string for a single select, or the selected values of a multiple select.
pub fn select(h: &mut DocHost, el: HostNode, value: SelectValue<'_>, now: f64) -> UiResult<()> {
    if !is_select(h, el) {
        return type_err("select: the target must be a select element");
    }
    if prop_bool(h, el, "multiple")? {
        let SelectValue::Many(values) = value else {
            return type_err("select: a multiple select takes an array of values");
        };
        for opt in h.option_nodes(el) {
            let v = prop_str(h, opt, "value")?;
            let selected = values.iter().any(|x| *x == v);
            h.set_property(opt, "selected", HostValue::Bool(selected)).map_err(host_err)?;
        }
    } else {
        let SelectValue::One(v) = value else {
            return type_err("select: value must be a string");
        };
        set_value(h, el, v)?;
    }
    dispatch(h, el, "change", &EventSpec::default(), now)
}

/// The value argument of `select`, as the JavaScript side reads it: a string, an array of its string items, or any
/// other value (which every select refuses).
pub enum SelectValue<'a> {
    One(&'a str),
    Many(&'a [String]),
    Other,
}

fn set_value(h: &mut DocHost, el: HostNode, value: &str) -> UiResult<()> {
    h.set_property(el, "value", HostValue::Str(value.to_string())).map_err(host_err)
}

/// `check(node, checked)`: a checkbox or radio, changed through its activation behaviour (`el.click()`).
pub fn check(h: &mut DocHost, el: HostNode, checked: Option<bool>, now: f64) -> UiResult<()> {
    if !checkable(h, el)? {
        return type_err("check: the target must be a checkbox or a radio button");
    }
    let Some(want) = checked else {
        return type_err("check: the second argument must be a boolean");
    };
    let is_radio = prop_str(h, el, "type")? == "radio";
    if prop_bool(h, el, "checked")? == want {
        return Ok(());
    }
    if is_radio && !want {
        return type_err("check: a radio button cannot be unchecked by user activation");
    }
    h.set_now(now);
    h.click(el).map_err(host_err)?;
    if prop_bool(h, el, "checked")? != want {
        return Err(UiError::Error("check: the control did not change (is it disabled?)".to_string()));
    }
    Ok(())
}

pub fn focus(h: &mut DocHost, el: HostNode) -> UiResult<()> {
    h.focus(el).map_err(host_err)
}

/// `container(id)`: the element with that id, created under body when missing.
pub fn container(h: &mut DocHost, id: &str) -> UiResult<HostNode> {
    if let Some(found) = h.query_selector(&format!("#{id}")).map_err(host_err)? {
        return Ok(found);
    }
    let body = h.body().ok_or_else(|| UiError::Error("the document has no body".to_string()))?;
    let el = h.create_element(HostNs::Html, "div", None).map_err(host_err)?;
    h.set_attribute(el, "id", id).map_err(host_err)?;
    h.insert(el, body, None).map_err(host_err)?;
    Ok(el)
}

/// The container of mount `n` (`div#app-N`): a new element appended to body, even when the id is taken (as the oracle).
pub fn mount_container(h: &mut DocHost, n: u32) -> UiResult<HostNode> {
    let body = h.body().ok_or_else(|| UiError::Error("the document has no body".to_string()))?;
    let el = h.create_element(HostNs::Html, "div", None).map_err(host_err)?;
    h.set_attribute(el, "id", &format!("app-{n}")).map_err(host_err)?;
    h.insert(el, body, None).map_err(host_err)?;
    Ok(el)
}

/// Every element under `root` (not root itself) that matches `sel`, in document order.
pub fn find_all(h: &DocHost, root: HostNode, sel: &Selector) -> UiResult<Vec<HostNode>> {
    let idx = Index::build(h).map_err(UiError::Error)?;
    let mut out = Vec::new();
    let mut stack: Vec<HostNode> = idx.children(root).iter().rev().copied().collect();
    while let Some(n) = stack.pop() {
        stack.extend(idx.children(n).iter().rev().copied());
        if h.is_element(n) && matches(h, &idx, n, sel) {
            out.push(n);
        }
    }
    Ok(out)
}

fn matches(h: &DocHost, idx: &Index, el: HostNode, sel: &Selector) -> bool {
    let tag = h.tag_name(el);
    let attr = |name: &str| h.get_attribute(el, name);
    let child_index = || {
        let parent = h.parent_node(el)?;
        let mut elements = idx.children(parent).iter().filter(|c| h.is_element(**c));
        elements.position(|c| *c == el).map(|i| i as u64 + 1)
    };
    sel.matches(&tag, &attr, &child_index)
}

/// The canonical snapshot of an element (`Element.snapshot`), as a contract tree.
pub fn snapshot_of(h: &DocHost, el: HostNode) -> UiResult<Json> {
    let mut d = h.clone();
    let text = d.snapshot(el).map_err(host_err)?;
    snapshot::contract_of(&text).map_err(UiError::Error)
}

/// The children of a container in the contract shape (`Mount.snapshot`).
pub fn children_snapshot(h: &DocHost, el: HostNode) -> UiResult<Vec<Json>> {
    Ok(snapshot::children_of(&snapshot_of(h, el)?))
}

pub fn inner_html(h: &DocHost, el: HostNode) -> String {
    let mut d = h.clone();
    d.inner_html(el).unwrap_or_default()
}

pub fn text_content(h: &mut DocHost, el: HostNode) -> UiResult<String> {
    prop_str(h, el, "textContent")
}

pub fn is_connected(h: &mut DocHost, el: HostNode) -> UiResult<bool> {
    prop_bool(h, el, "isConnected")
}

pub fn attr(h: &DocHost, el: HostNode, name: &str) -> Option<String> {
    h.get_attribute(el, name)
}

pub fn prop(h: &mut DocHost, el: HostNode, name: &str) -> UiResult<HostValue> {
    h.get_property(el, name).map_err(host_err)
}
