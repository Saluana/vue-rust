//! The only unsafe code of the crate: the `vn_host` imports, the exports of ABI 1, and every read or write of linear memory
//! they imply (hosts/web/ABI.md states the invariants). Off wasm32 the imports are panicking stubs, so `cargo test` covers
//! the pure parts of the crate only. Every `unsafe` block has a SAFETY note.

use std::alloc::{self, Layout};
#[cfg(feature = "selftest")]
use std::ptr;

use crate::abi::{ABI_VERSION, STATUS_OK};

/// Alignment of every buffer that crosses the boundary (the glue writes u32 arrays that Rust reads in place).
const ALIGN: usize = 8;

macro_rules! vn_imports {
    ($($name:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)?;)*) => {
        #[cfg(target_arch = "wasm32")]
        #[link(wasm_import_module = "vn_host")]
        unsafe extern "C" {
            $(pub fn $name($($arg: $ty),*) $(-> $ret)?;)*
        }

        $(
            #[cfg(not(target_arch = "wasm32"))]
            #[allow(unused_variables)]
            pub unsafe fn $name($($arg: $ty),*) $(-> $ret)? {
                super::native_only(stringify!($name))
            }
        )*
    };
}

/// The import table of ABI 1: one line per operation (hosts/web/ABI.md gives the meaning of every argument).
#[allow(dead_code)]
mod raw {
    vn_imports! {
        host_now() -> f64;
        host_set_timeout(id: u32, ms: f64);
        host_clear_timeout(id: u32);
        host_log(level: u32, ptr: u32, len: u32);
        host_panic(ptr: u32, len: u32);
        host_error_message(out: u32);
        create_element(ns: u32, tag_ptr: u32, tag_len: u32, is_ptr: u32, is_len: u32, has_is: u32, out: u32) -> i32;
        create_text(ptr: u32, len: u32) -> u32;
        create_comment(ptr: u32, len: u32) -> u32;
        set_text(node: u32, ptr: u32, len: u32) -> i32;
        set_element_text(el: u32, ptr: u32, len: u32) -> i32;
        insert(child: u32, parent: u32, anchor: u32) -> i32;
        remove(child: u32) -> i32;
        parent_node(node: u32) -> u32;
        next_sibling(node: u32) -> u32;
        query_selector(ptr: u32, len: u32, out: u32) -> i32;
        body() -> u32;
        release(node: u32);
        tag_name(el: u32, out: u32) -> i32;
        is_element(node: u32) -> i32;
        set_attribute(el: u32, name_ptr: u32, name_len: u32, value_ptr: u32, value_len: u32) -> i32;
        set_attribute_ns(el: u32, has_ns: u32, ns_ptr: u32, ns_len: u32, name_ptr: u32, name_len: u32, value_ptr: u32, value_len: u32) -> i32;
        remove_attribute(el: u32, name_ptr: u32, name_len: u32) -> i32;
        remove_attribute_ns(el: u32, has_ns: u32, ns_ptr: u32, ns_len: u32, local_ptr: u32, local_len: u32) -> i32;
        get_attribute(el: u32, name_ptr: u32, name_len: u32, out: u32) -> i32;
        has_attribute(el: u32, name_ptr: u32, name_len: u32) -> i32;
        has_property(node: u32, name_ptr: u32, name_len: u32) -> i32;
        get_property(node: u32, name_ptr: u32, name_len: u32, out: u32) -> i32;
        set_property(node: u32, name_ptr: u32, name_len: u32, kind: u32, num: f64, str_ptr: u32, str_len: u32) -> i32;
        style_set_property(el: u32, name_ptr: u32, name_len: u32, value_ptr: u32, value_len: u32, important: u32) -> i32;
        style_remove_property(el: u32, name_ptr: u32, name_len: u32) -> i32;
        style_get_property(el: u32, name_ptr: u32, name_len: u32, out: u32) -> i32;
        style_css_text(el: u32, out: u32) -> i32;
        style_set_css_text(el: u32, text_ptr: u32, text_len: u32) -> i32;
        class_list_add(el: u32, token_ptr: u32, token_len: u32) -> i32;
        class_list_remove(el: u32, token_ptr: u32, token_len: u32) -> i32;
        add_event_listener(el: u32, type_ptr: u32, type_len: u32, listener: u32, flags: u32) -> i32;
        remove_event_listener(el: u32, type_ptr: u32, type_len: u32, listener: u32, capture: u32) -> i32;
        focus(el: u32) -> i32;
        blur(el: u32) -> i32;
        active_element() -> u32;
        option_nodes(select: u32, out: u32) -> i32;
        outer_html(node: u32, out: u32) -> i32;
        inner_html(node: u32, out: u32) -> i32;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn native_only(name: &str) -> ! {
    panic!("vn_host import `{name}` exists only on wasm32")
}

// ---- helpers for pointers that cross the boundary

fn addr(s: &str) -> u32 {
    s.as_ptr() as usize as u32
}

fn len(s: &str) -> u32 {
    s.len() as u32
}

fn at<T>(p: *mut T) -> u32 {
    p as usize as u32
}

/// Takes ownership of a string the glue allocated with `vn_alloc`: copies the bytes out and frees the buffer.
fn take_string(p: u32, n: u32) -> String {
    if n == 0 {
        return String::new();
    }
    // SAFETY: the glue wrote exactly n bytes at p after `vn_alloc(n)` and passed ownership to us; nothing else refers to them.
    let bytes = unsafe { std::slice::from_raw_parts(p as usize as *const u8, n as usize) }.to_vec();
    free(p, n);
    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    }
}

/// Frees a buffer obtained from `vn_alloc(n)` (zero-length buffers were never allocated).
fn free(p: u32, n: u32) {
    if n == 0 || p == 0 {
        return;
    }
    if let Ok(layout) = Layout::from_size_align(n as usize, ALIGN) {
        // SAFETY: (p, n) names a live allocation made by `vn_alloc(n)` with this same layout, freed exactly once by the caller.
        unsafe { alloc::dealloc(p as usize as *mut u8, layout) }
    }
}

/// Runs a call that writes a string into a two-word out slot; the string is taken only when the call reports success.
fn string_out(call: impl FnOnce(u32) -> i32) -> (i32, String) {
    let mut out = [0u32; 2];
    let st = call(at(out.as_mut_ptr()));
    if st == STATUS_OK { (st, take_string(out[0], out[1])) } else { (st, String::new()) }
}

/// Like `string_out` for imports that answer 1 (present, string written) or 0 (absent).
fn optional_string_out(call: impl FnOnce(u32) -> i32) -> Option<String> {
    let mut out = [0u32; 2];
    if call(at(out.as_mut_ptr())) == 1 { Some(take_string(out[0], out[1])) } else { None }
}

// ---- wrappers: one per import, with the argument conventions of hosts/web/ABI.md

pub fn now() -> f64 {
    // SAFETY: no pointer arguments.
    unsafe { raw::host_now() }
}

pub fn set_timeout(id: u32, ms: f64) {
    // SAFETY: scalar arguments only.
    unsafe { raw::host_set_timeout(id, ms) }
}

pub fn clear_timeout(id: u32) {
    // SAFETY: scalar argument only.
    unsafe { raw::host_clear_timeout(id) }
}

pub fn log(level: u32, msg: &str) {
    // SAFETY: the glue reads len bytes at addr during the call only; msg outlives the call.
    unsafe { raw::host_log(level, addr(msg), len(msg)) }
}

pub fn panic_report(msg: &str) {
    // SAFETY: as for `log`; called from the panic hook, which runs before the trap.
    unsafe { raw::host_panic(addr(msg), len(msg)) }
}

pub fn error_message() -> String {
    let mut out = [0u32; 2];
    // SAFETY: the glue writes two u32 words into `out` during the call (it is the only writer while the call runs).
    unsafe { raw::host_error_message(at(out.as_mut_ptr())) };
    take_string(out[0], out[1])
}

pub fn create_element(ns: u32, tag: &str, is: Option<&str>) -> (i32, u32) {
    let mut out = 0u32;
    let (is_ptr, is_len) = is.map_or((0, 0), |s| (addr(s), len(s)));
    // SAFETY: tag and is stay borrowed for the call; `out` receives one u32 during the call.
    let st = unsafe {
        raw::create_element(ns, addr(tag), len(tag), is_ptr, is_len, is.is_some() as u32, at(&mut out as *mut u32))
    };
    (st, out)
}

pub fn create_text(text: &str) -> u32 {
    // SAFETY: text is borrowed for the call.
    unsafe { raw::create_text(addr(text), len(text)) }
}

pub fn create_comment(text: &str) -> u32 {
    // SAFETY: text is borrowed for the call.
    unsafe { raw::create_comment(addr(text), len(text)) }
}

pub fn set_text(node: u32, text: &str) -> i32 {
    // SAFETY: text is borrowed for the call.
    unsafe { raw::set_text(node, addr(text), len(text)) }
}

pub fn set_element_text(el: u32, text: &str) -> i32 {
    // SAFETY: text is borrowed for the call.
    unsafe { raw::set_element_text(el, addr(text), len(text)) }
}

pub fn insert(child: u32, parent: u32, anchor: u32) -> i32 {
    // SAFETY: scalar arguments only.
    unsafe { raw::insert(child, parent, anchor) }
}

pub fn remove(child: u32) -> i32 {
    // SAFETY: scalar argument only.
    unsafe { raw::remove(child) }
}

pub fn parent_node(node: u32) -> u32 {
    // SAFETY: scalar argument only.
    unsafe { raw::parent_node(node) }
}

pub fn next_sibling(node: u32) -> u32 {
    // SAFETY: scalar argument only.
    unsafe { raw::next_sibling(node) }
}

pub fn query_selector(selector: &str) -> (i32, u32) {
    let mut out = 0u32;
    // SAFETY: selector is borrowed for the call; `out` receives one u32 during the call.
    let st = unsafe { raw::query_selector(addr(selector), len(selector), at(&mut out as *mut u32)) };
    (st, out)
}

pub fn body() -> u32 {
    // SAFETY: no arguments.
    unsafe { raw::body() }
}

pub fn release(node: u32) {
    // SAFETY: scalar argument only.
    unsafe { raw::release(node) }
}

pub fn tag_name(el: u32) -> (i32, String) {
    // SAFETY: the out slot is local to string_out and written only during the call.
    string_out(|out| unsafe { raw::tag_name(el, out) })
}

pub fn is_element(node: u32) -> i32 {
    // SAFETY: scalar argument only.
    unsafe { raw::is_element(node) }
}

pub fn set_attribute(el: u32, name: &str, value: &str) -> i32 {
    // SAFETY: name and value are borrowed for the call.
    unsafe { raw::set_attribute(el, addr(name), len(name), addr(value), len(value)) }
}

pub fn set_attribute_ns(el: u32, ns: Option<&str>, qualified: &str, value: &str) -> i32 {
    let (ns_ptr, ns_len) = ns.map_or((0, 0), |s| (addr(s), len(s)));
    // SAFETY: all strings are borrowed for the call; a null namespace is flagged by has_ns = 0 and never read.
    unsafe {
        raw::set_attribute_ns(
            el,
            ns.is_some() as u32,
            ns_ptr,
            ns_len,
            addr(qualified),
            len(qualified),
            addr(value),
            len(value),
        )
    }
}

pub fn remove_attribute(el: u32, name: &str) -> i32 {
    // SAFETY: name is borrowed for the call.
    unsafe { raw::remove_attribute(el, addr(name), len(name)) }
}

pub fn remove_attribute_ns(el: u32, ns: Option<&str>, local: &str) -> i32 {
    let (ns_ptr, ns_len) = ns.map_or((0, 0), |s| (addr(s), len(s)));
    // SAFETY: as for `set_attribute_ns`.
    unsafe { raw::remove_attribute_ns(el, ns.is_some() as u32, ns_ptr, ns_len, addr(local), len(local)) }
}

pub fn get_attribute(el: u32, name: &str) -> Option<String> {
    // SAFETY: name is borrowed for the call; the out slot is local to optional_string_out.
    optional_string_out(|out| unsafe { raw::get_attribute(el, addr(name), len(name), out) })
}

pub fn has_attribute(el: u32, name: &str) -> i32 {
    // SAFETY: name is borrowed for the call.
    unsafe { raw::has_attribute(el, addr(name), len(name)) }
}

pub fn has_property(node: u32, name: &str) -> i32 {
    // SAFETY: name is borrowed for the call.
    unsafe { raw::has_property(node, addr(name), len(name)) }
}

/// A HostValue as the glue writes it: 24 bytes, `kind` at 0, `flag` at 4, `num` at 8, text (ptr, len) at 16 and 20.
#[repr(C, align(8))]
struct RawValue {
    kind: u32,
    flag: u32,
    num: f64,
    ptr: u32,
    len: u32,
}

/// The value of a property: `kind` is one of the VALUE_* codes; `text` is set for strings only.
#[derive(Default)]
pub struct PropValue {
    pub kind: u32,
    pub flag: u32,
    pub num: f64,
    pub text: String,
}

pub fn get_property(node: u32, name: &str) -> (i32, PropValue) {
    let mut raw = RawValue { kind: 0, flag: 0, num: 0.0, ptr: 0, len: 0 };
    // SAFETY: name is borrowed for the call; the glue writes the 24-byte struct at `raw` during the call and nothing else.
    let st = unsafe { raw::get_property(node, addr(name), len(name), at(&mut raw as *mut RawValue)) };
    if st != STATUS_OK {
        return (st, PropValue::default());
    }
    let text = if raw.kind == crate::abi::VALUE_STRING { take_string(raw.ptr, raw.len) } else { String::new() };
    (st, PropValue { kind: raw.kind, flag: raw.flag, num: raw.num, text })
}

pub fn set_property(node: u32, name: &str, kind: u32, num: f64, text: &str) -> i32 {
    // SAFETY: name and text are borrowed for the call; num is a plain f64 argument.
    unsafe { raw::set_property(node, addr(name), len(name), kind, num, addr(text), len(text)) }
}

pub fn style_set_property(el: u32, name: &str, value: &str, important: bool) -> i32 {
    // SAFETY: name and value are borrowed for the call.
    unsafe { raw::style_set_property(el, addr(name), len(name), addr(value), len(value), important as u32) }
}

pub fn style_remove_property(el: u32, name: &str) -> i32 {
    // SAFETY: name is borrowed for the call.
    unsafe { raw::style_remove_property(el, addr(name), len(name)) }
}

pub fn style_get_property(el: u32, name: &str) -> (i32, String) {
    // SAFETY: name is borrowed for the call; the out slot is local to string_out.
    string_out(|out| unsafe { raw::style_get_property(el, addr(name), len(name), out) })
}

pub fn style_css_text(el: u32) -> (i32, String) {
    // SAFETY: the out slot is local to string_out.
    string_out(|out| unsafe { raw::style_css_text(el, out) })
}

pub fn style_set_css_text(el: u32, text: &str) -> i32 {
    // SAFETY: text is borrowed for the call.
    unsafe { raw::style_set_css_text(el, addr(text), len(text)) }
}

pub fn class_list_add(el: u32, token: &str) -> i32 {
    // SAFETY: token is borrowed for the call.
    unsafe { raw::class_list_add(el, addr(token), len(token)) }
}

pub fn class_list_remove(el: u32, token: &str) -> i32 {
    // SAFETY: token is borrowed for the call.
    unsafe { raw::class_list_remove(el, addr(token), len(token)) }
}

pub fn add_event_listener(el: u32, ty: &str, listener: u32, flags: u32) -> i32 {
    // SAFETY: ty is borrowed for the call.
    unsafe { raw::add_event_listener(el, addr(ty), len(ty), listener, flags) }
}

pub fn remove_event_listener(el: u32, ty: &str, listener: u32, capture: bool) -> i32 {
    // SAFETY: ty is borrowed for the call.
    unsafe { raw::remove_event_listener(el, addr(ty), len(ty), listener, capture as u32) }
}

pub fn focus(el: u32) -> i32 {
    // SAFETY: scalar argument only.
    unsafe { raw::focus(el) }
}

pub fn blur(el: u32) -> i32 {
    // SAFETY: scalar argument only.
    unsafe { raw::blur(el) }
}

pub fn active_element() -> u32 {
    // SAFETY: no arguments.
    unsafe { raw::active_element() }
}

/// The option handles of a select. The glue allocates (ptr, count) with `vn_alloc(4 * count)`; Rust copies and frees it.
pub fn option_nodes(select: u32) -> (i32, Vec<u32>) {
    let mut out = [0u32; 2];
    // SAFETY: the out slot is local and written only during the call.
    let st = unsafe { raw::option_nodes(select, at(out.as_mut_ptr())) };
    if st != STATUS_OK {
        return (st, Vec::new());
    }
    let [p, n] = out;
    if n == 0 {
        free(p, 0);
        return (st, Vec::new());
    }
    // SAFETY: the glue wrote n aligned u32 handles at p with vn_alloc(4 * n) and passed ownership to us.
    let handles = unsafe { std::slice::from_raw_parts(p as usize as *const u32, n as usize) }.to_vec();
    free(p, n.saturating_mul(4));
    (st, handles)
}

pub fn outer_html(node: u32) -> Option<String> {
    // SAFETY: the out slot is local to optional_string_out.
    optional_string_out(|out| unsafe { raw::outer_html(node, out) })
}

pub fn inner_html(node: u32) -> Option<String> {
    // SAFETY: the out slot is local to optional_string_out.
    optional_string_out(|out| unsafe { raw::inner_html(node, out) })
}

/// Takes the address of every import, so the linker keeps the complete `vn_host` table in the module (a cdylib keeps only the
/// code its exports reach). Returns the number of imports; `vn_host_probe` exposes it. It calls nothing.
pub fn linked_imports() -> u32 {
    let table: [*const (); 44] = [
        raw::host_now as *const (),
        raw::host_set_timeout as *const (),
        raw::host_clear_timeout as *const (),
        raw::host_log as *const (),
        raw::host_panic as *const (),
        raw::host_error_message as *const (),
        raw::create_element as *const (),
        raw::create_text as *const (),
        raw::create_comment as *const (),
        raw::set_text as *const (),
        raw::set_element_text as *const (),
        raw::insert as *const (),
        raw::remove as *const (),
        raw::parent_node as *const (),
        raw::next_sibling as *const (),
        raw::query_selector as *const (),
        raw::body as *const (),
        raw::release as *const (),
        raw::tag_name as *const (),
        raw::is_element as *const (),
        raw::set_attribute as *const (),
        raw::set_attribute_ns as *const (),
        raw::remove_attribute as *const (),
        raw::remove_attribute_ns as *const (),
        raw::get_attribute as *const (),
        raw::has_attribute as *const (),
        raw::has_property as *const (),
        raw::get_property as *const (),
        raw::set_property as *const (),
        raw::style_set_property as *const (),
        raw::style_remove_property as *const (),
        raw::style_get_property as *const (),
        raw::style_css_text as *const (),
        raw::style_set_css_text as *const (),
        raw::class_list_add as *const (),
        raw::class_list_remove as *const (),
        raw::add_event_listener as *const (),
        raw::remove_event_listener as *const (),
        raw::focus as *const (),
        raw::blur as *const (),
        raw::active_element as *const (),
        raw::option_nodes as *const (),
        raw::outer_html as *const (),
        raw::inner_html as *const (),
    ];
    std::hint::black_box(&table);
    table.len() as u32
}

// ---- exports (ABI 1)

/// Links every import (see `linked_imports`) and returns their number: 44 in ABI 1. The glue does not call it.
#[unsafe(no_mangle)]
pub extern "C" fn vn_host_probe() -> u32 {
    linked_imports()
}

/// The ABI version this build implements; the glue refuses any other value.
#[unsafe(no_mangle)]
pub extern "C" fn vn_abi_version() -> u32 {
    ABI_VERSION
}

/// Installs the panic hook. The glue calls it once after instantiation.
#[unsafe(no_mangle)]
pub extern "C" fn vn_init() {
    crate::web_host::install_panic_hook();
}

/// Allocates `len` bytes aligned to 8 for the glue; returns 0 when the allocation fails. Zero-length requests return a
/// non-null aligned address that must never be read.
#[unsafe(no_mangle)]
pub extern "C" fn vn_alloc(len: u32) -> u32 {
    if len == 0 {
        return ALIGN as u32;
    }
    let Ok(layout) = Layout::from_size_align(len as usize, ALIGN) else {
        return 0;
    };
    // SAFETY: the layout has a non-zero size.
    let p = unsafe { alloc::alloc(layout) };
    p as usize as u32
}

/// Frees a buffer returned by `vn_alloc` (or by an export that documents the same convention).
#[unsafe(no_mangle)]
pub extern "C" fn vn_free(ptr: u32, len: u32) {
    free(ptr, len)
}

/// Delivers one event to listener `listener`. The payload at [ptr, ptr + len) is read during the call only; the returned
/// bits are the listener outcome (abi::OUTCOME_*).
#[unsafe(no_mangle)]
pub extern "C" fn vn_event(listener: u32, ptr: u32, len: u32) -> u32 {
    // SAFETY: the glue allocated the payload with vn_alloc, filled it before the call and frees it after the call returns.
    let payload = unsafe { bytes_at(ptr, len) };
    crate::web_host::dispatch_event(listener, payload)
}

/// Fires timer `id` previously scheduled with `host_set_timeout`. Returns 1 when a timer hook is installed, else 0.
#[unsafe(no_mangle)]
pub extern "C" fn vn_timer(id: u32) -> u32 {
    crate::web_host::timer_fired(id)
}

/// Self-test entry point: the request is UTF-8 JSON at [ptr, ptr + len); the reply is a vn_alloc'd buffer holding a
/// little-endian u32 byte count followed by the JSON reply. Free it with `vn_free(reply, 4 + count)`.
#[cfg(feature = "selftest")]
#[unsafe(no_mangle)]
pub extern "C" fn selftest_call(ptr: u32, len: u32) -> u32 {
    // SAFETY: as for `vn_event`: the glue owns the request buffer for the duration of the call.
    let request = unsafe { bytes_at(ptr, len) };
    let reply = crate::selftest::call(request);
    export_prefixed(&reply)
}

/// Views [p, p + n) of linear memory. Only called with buffers the glue owns for the duration of the call.
unsafe fn bytes_at<'a>(p: u32, n: u32) -> &'a [u8] {
    if n == 0 {
        return &[];
    }
    // SAFETY: the caller guarantees that n readable bytes start at p for 'a (see the callers' SAFETY notes).
    unsafe { std::slice::from_raw_parts(p as usize as *const u8, n as usize) }
}

#[cfg(feature = "selftest")]
fn export_prefixed(bytes: &[u8]) -> u32 {
    let total = bytes.len() + 4;
    let p = vn_alloc(total as u32);
    if p == 0 {
        return 0;
    }
    let dst = p as usize as *mut u8;
    // SAFETY: p was allocated above with `total` bytes; the two copies write exactly 4 + bytes.len() of them.
    unsafe {
        ptr::copy_nonoverlapping((bytes.len() as u32).to_le_bytes().as_ptr(), dst, 4);
        ptr::copy_nonoverlapping(bytes.as_ptr(), dst.add(4), bytes.len());
    }
    p
}
