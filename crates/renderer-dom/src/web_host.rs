//! `WebHost`: the `Host` trait over the `vn_host` imports. It keeps no DOM state (the glue owns every node); a `WebHost` is a
//! handle to the single browser host of the wasm instance. Every method maps to one import; failures come back as the
//! `HostError` named by the status code, with the message fetched from the glue.

use std::cell::RefCell;
use std::rc::Rc;

use vn_runtime::host::{
    EventSink, Host, HostError, HostNode, HostNs, HostResult, HostValue, ListenerId, ListenerOpts,
};

use crate::abi::{self, STATUS_OK};
use crate::{ffi, listeners};

/// Severity of `WebHost::log`, shown by the glue's `onLog` (0 debug, 1 info, 2 warn, 3 error).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

/// Handle to the browser host of this wasm instance (zero-sized: all state lives in the glue or in thread-local cells).
#[derive(Debug, Default)]
pub struct WebHost {
    _private: (),
}

thread_local! {
    static TIMER_HOOK: RefCell<Option<Rc<dyn Fn(u32)>>> = const { RefCell::new(None) };
}

/// Installs the function that runs when a timer scheduled with `WebHost::set_timeout` fires (`vn_timer`).
pub fn set_timer_hook(hook: Rc<dyn Fn(u32)>) {
    TIMER_HOOK.with(|slot| *slot.borrow_mut() = Some(hook));
}

pub(crate) fn timer_fired(id: u32) -> u32 {
    let hook = TIMER_HOOK.with(|slot| slot.borrow().clone());
    match hook {
        Some(hook) => {
            hook(id);
            1
        }
        None => 0,
    }
}

pub(crate) fn dispatch_event(listener: u32, payload: &[u8]) -> u32 {
    match abi::decode_event(payload) {
        Ok(event) => abi::outcome_bits(listeners::dispatch(ListenerId(listener), &event)),
        Err(why) => {
            ffi::log(abi::LOG_ERROR, &format!("malformed event payload: {why}"));
            0
        }
    }
}

pub(crate) fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| ffi::panic_report(&info.to_string())));
}

impl WebHost {
    pub fn new() -> Self {
        WebHost { _private: () }
    }

    /// Writes a message through the glue's `onLog`.
    pub fn log(&self, level: LogLevel, message: &str) {
        ffi::log(level as u32, message);
    }

    /// Schedules `vn_timer(id)` after `ms` milliseconds; a second call with the same id replaces the first.
    pub fn set_timeout(&self, id: u32, ms: f64) {
        ffi::set_timeout(id, ms);
    }

    pub fn clear_timeout(&self, id: u32) {
        ffi::clear_timeout(id);
    }
}

/// Maps a status code to the trait's result. Failures carry the message the glue recorded for them.
fn check(status: i32) -> HostResult<()> {
    match status {
        STATUS_OK => Ok(()),
        abi::STATUS_NOT_FOUND => Err(HostError::NotFound),
        abi::STATUS_UNSUPPORTED => Err(HostError::Unsupported(ffi::error_message())),
        abi::STATUS_INVALID_STATE => Err(HostError::InvalidState(ffi::error_message())),
        other => Err(HostError::InvalidState(format!("vn_host returned unknown status {other}"))),
    }
}

fn opt(handle: u32) -> Option<HostNode> {
    if handle == 0 { None } else { Some(HostNode(handle)) }
}

/// Creation cannot report an error through the infallible trait methods, so exhaustion of the glue's handle table stops
/// the program through the panic hook (reported as a panic by the glue).
fn created(handle: u32, what: &str) -> HostNode {
    if handle == 0 {
        panic!("vn_host: no node handle left for a new {what}");
    }
    HostNode(handle)
}

impl Host for WebHost {
    fn set_event_sink(&mut self, sink: Rc<dyn EventSink>) {
        listeners::install(sink);
    }

    fn now_ms(&self) -> f64 {
        ffi::now()
    }

    fn create_element(&mut self, ns: HostNs, tag: &str, is: Option<&str>) -> HostResult<HostNode> {
        let ns = match ns {
            HostNs::Html => abi::NS_HTML,
            HostNs::Svg => abi::NS_SVG,
            HostNs::MathMl => abi::NS_MATHML,
        };
        let (status, handle) = ffi::create_element(ns, tag, is);
        check(status)?;
        Ok(HostNode(handle))
    }

    fn create_text(&mut self, text: &str) -> HostNode {
        created(ffi::create_text(text), "text node")
    }

    fn create_comment(&mut self, text: &str) -> HostNode {
        created(ffi::create_comment(text), "comment node")
    }

    fn set_text(&mut self, node: HostNode, text: &str) -> HostResult<()> {
        check(ffi::set_text(node.0, text))
    }

    fn set_element_text(&mut self, el: HostNode, text: &str) -> HostResult<()> {
        check(ffi::set_element_text(el.0, text))
    }

    fn insert(&mut self, child: HostNode, parent: HostNode, anchor: Option<HostNode>) -> HostResult<()> {
        // Handle 0 means "no anchor" on the wire, so an explicit HostNode(0) anchor is refused here.
        if anchor == Some(HostNode(0)) {
            return Err(HostError::NotFound);
        }
        check(ffi::insert(child.0, parent.0, anchor.map_or(0, |a| a.0)))
    }

    fn remove(&mut self, child: HostNode) -> HostResult<()> {
        check(ffi::remove(child.0))
    }

    fn parent_node(&self, node: HostNode) -> Option<HostNode> {
        opt(ffi::parent_node(node.0))
    }

    fn next_sibling(&self, node: HostNode) -> Option<HostNode> {
        opt(ffi::next_sibling(node.0))
    }

    fn query_selector(&self, selector: &str) -> HostResult<Option<HostNode>> {
        let (status, handle) = ffi::query_selector(selector);
        check(status)?;
        Ok(opt(handle))
    }

    fn body(&self) -> Option<HostNode> {
        opt(ffi::body())
    }

    fn release(&mut self, node: HostNode) {
        ffi::release(node.0);
    }

    fn tag_name(&self, el: HostNode) -> String {
        ffi::tag_name(el.0).1
    }

    fn is_element(&self, node: HostNode) -> bool {
        ffi::is_element(node.0) == 1
    }

    fn set_attribute(&mut self, el: HostNode, name: &str, value: &str) -> HostResult<()> {
        check(ffi::set_attribute(el.0, name, value))
    }

    fn set_attribute_ns(&mut self, el: HostNode, ns: Option<&str>, qualified: &str, value: &str) -> HostResult<()> {
        check(ffi::set_attribute_ns(el.0, ns, qualified, value))
    }

    fn remove_attribute(&mut self, el: HostNode, name: &str) -> HostResult<()> {
        check(ffi::remove_attribute(el.0, name))
    }

    fn remove_attribute_ns(&mut self, el: HostNode, ns: Option<&str>, local: &str) -> HostResult<()> {
        check(ffi::remove_attribute_ns(el.0, ns, local))
    }

    fn get_attribute(&self, el: HostNode, name: &str) -> Option<String> {
        ffi::get_attribute(el.0, name)
    }

    fn has_attribute(&self, el: HostNode, name: &str) -> bool {
        ffi::has_attribute(el.0, name) == 1
    }

    fn has_property(&self, el: HostNode, name: &str) -> bool {
        ffi::has_property(el.0, name) == 1
    }

    fn get_property(&mut self, el: HostNode, name: &str) -> HostResult<HostValue> {
        let (status, value) = ffi::get_property(el.0, name);
        check(status)?;
        Ok(match value.kind {
            abi::VALUE_UNDEFINED => HostValue::Undefined,
            abi::VALUE_NULL => HostValue::Null,
            abi::VALUE_BOOL => HostValue::Bool(value.flag != 0),
            abi::VALUE_NUMBER => HostValue::Number(value.num),
            abi::VALUE_STRING => HostValue::Str(value.text),
            other => return Err(HostError::InvalidState(format!("vn_host returned value kind {other}"))),
        })
    }

    fn set_property(&mut self, el: HostNode, name: &str, value: HostValue) -> HostResult<()> {
        let (kind, num, text) = match &value {
            HostValue::Undefined => (abi::VALUE_UNDEFINED, 0.0, ""),
            HostValue::Null => (abi::VALUE_NULL, 0.0, ""),
            HostValue::Bool(b) => (abi::VALUE_BOOL, if *b { 1.0 } else { 0.0 }, ""),
            HostValue::Number(n) => (abi::VALUE_NUMBER, *n, ""),
            HostValue::Str(s) => (abi::VALUE_STRING, 0.0, s.as_str()),
        };
        check(ffi::set_property(el.0, name, kind, num, text))
    }

    fn style_set_property(&mut self, el: HostNode, name: &str, value: &str, important: bool) -> HostResult<()> {
        check(ffi::style_set_property(el.0, name, value, important))
    }

    fn style_remove_property(&mut self, el: HostNode, name: &str) -> HostResult<()> {
        check(ffi::style_remove_property(el.0, name))
    }

    fn style_get_property(&self, el: HostNode, name: &str) -> String {
        ffi::style_get_property(el.0, name).1
    }

    fn style_css_text(&self, el: HostNode) -> String {
        ffi::style_css_text(el.0).1
    }

    fn style_set_css_text(&mut self, el: HostNode, text: &str) -> HostResult<()> {
        check(ffi::style_set_css_text(el.0, text))
    }

    fn class_list_add(&mut self, el: HostNode, token: &str) -> HostResult<()> {
        check(ffi::class_list_add(el.0, token))
    }

    fn class_list_remove(&mut self, el: HostNode, token: &str) -> HostResult<()> {
        check(ffi::class_list_remove(el.0, token))
    }

    fn add_event_listener(&mut self, el: HostNode, ty: &str, id: ListenerId, opts: ListenerOpts) -> HostResult<()> {
        let mut flags = 0;
        if opts.capture {
            flags |= abi::LISTENER_CAPTURE;
        }
        if opts.once {
            flags |= abi::LISTENER_ONCE;
        }
        if opts.passive {
            flags |= abi::LISTENER_PASSIVE;
        }
        check(ffi::add_event_listener(el.0, ty, id.0, flags))
    }

    fn remove_event_listener(&mut self, el: HostNode, ty: &str, id: ListenerId, capture: bool) -> HostResult<()> {
        check(ffi::remove_event_listener(el.0, ty, id.0, capture))
    }

    fn focus(&mut self, el: HostNode) -> HostResult<()> {
        check(ffi::focus(el.0))
    }

    fn blur(&mut self, el: HostNode) -> HostResult<()> {
        check(ffi::blur(el.0))
    }

    fn active_element(&self) -> Option<HostNode> {
        opt(ffi::active_element())
    }

    fn option_nodes(&self, select: HostNode) -> Vec<HostNode> {
        let (status, handles) = ffi::option_nodes(select.0);
        if status != STATUS_OK {
            return Vec::new();
        }
        handles.into_iter().map(HostNode).collect()
    }

    fn outer_html(&mut self, node: HostNode) -> Option<String> {
        ffi::outer_html(node.0)
    }

    fn inner_html(&mut self, node: HostNode) -> Option<String> {
        ffi::inner_html(node.0)
    }
}
