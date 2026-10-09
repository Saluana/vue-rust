//! The prop decision tree of runtime-dom 3.5.43 (`patchProp`, `patchClass`, `patchStyle`, `setStyle`, `patchAttr`,
//! `patchDOMProp`, `shouldSetAsProp`) over the host (WP-RT-D). Behaviour per @vue/runtime-dom 3.5.43 and @vue/shared
//! 3.5.43; the reference is the readable `runtime-dom.cjs.prod.js`.
//!
//! Known deviations (vn-dom's property model, see crates/dom/src/props.rs): `name in el` answers only for the names
//! vn-dom models (so `width`/`height` on `img`, `form`, `list`, event-handler properties are attributes here), `style`
//! names are used as given (no vendor-prefix probing, which Vue does with `in style`), custom elements (`_isVueCE`)
//! are not modelled, and `innerHTML` assignment throws a TypeError because a native host has no HTML parser.

use vn_value::{JsStr, ObjRef, PropKey, Res, Rt, Value};

use crate::dom_events::{array_items, host_error, hyphenate, js_string, to_host_value, DomCtx};
use crate::host::{HostError, HostNode, HostNs, HostValue};

const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

/// shared `isOn`: `on` followed by a character that is not a lowercase letter (`onClick`, `on:click`).
pub(crate) fn is_on(key: &str) -> bool {
    let b = key.as_bytes();
    b.len() > 2 && b[0] == b'o' && b[1] == b'n' && !(b[2].is_ascii_lowercase())
}

/// runtime-dom `isNativeOn`: `on` followed by a lowercase letter.
pub(crate) fn is_native_on(key: &str) -> bool {
    let b = key.as_bytes();
    b.len() > 2 && b[0] == b'o' && b[1] == b'n' && b[2].is_ascii_lowercase()
}

/// shared `isSpecialBooleanAttr`.
pub(crate) fn is_special_boolean_attr(key: &str) -> bool {
    matches!(
        key,
        "itemscope" | "allowfullscreen" | "formnovalidate" | "ismap" | "nomodule" | "novalidate" | "readonly"
    )
}

/// shared `includeBooleanAttr`: `!!value || value === ""`.
pub(crate) fn include_boolean_attr(v: &Value) -> bool {
    v.to_boolean() || matches!(v, Value::String(s) if s.is_empty())
}

/// `for (const key in obj)` over an object: its enumerable own string keys (array indices included).
pub(crate) fn enumerable_keys(rt: &Rt, v: &Value) -> Res<Vec<String>> {
    let Value::Object(o) = v else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for k in rt.own_keys(*o)? {
        let name = match &k {
            PropKey::Index(i) => i.to_string(),
            PropKey::Str(s) => s.to_rust_string_lossy(),
            PropKey::Sym(_) => continue,
        };
        let enumerable = match rt.get_own_property(*o, &k)? {
            Some(p) => p.flags().enumerable,
            None => false,
        };
        if enumerable {
            out.push(name);
        }
    }
    Ok(out)
}

/// JS `parseFloat` on a string: leading whitespace skipped, longest decimal prefix (or `Infinity`), NaN if none.
pub(crate) fn parse_float(s: &str) -> f64 {
    let t = s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    if t[i..].starts_with("Infinity") {
        return if b.first() == Some(&b'-') { f64::NEG_INFINITY } else { f64::INFINITY };
    }
    let digits_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_digits = i - digits_start;
    let mut frac_digits = 0;
    if i < b.len() && b[i] == b'.' {
        let mut j = i + 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        frac_digits = j - (i + 1);
        if int_digits > 0 || frac_digits > 0 {
            i = j;
        }
    }
    if int_digits == 0 && frac_digits == 0 {
        return f64::NAN;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let exp_start = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > exp_start {
            i = j;
        }
    }
    t[..i].parse::<f64>().unwrap_or(f64::NAN)
}

/// shared `looseToNumber`: `parseFloat(val)`, or the value itself when that is NaN.
pub(crate) fn loose_to_number(rt: &Rt, v: &Value) -> Res<Value> {
    let s = js_string(rt, v)?;
    let n = parse_float(&s);
    Ok(if n.is_nan() { v.clone() } else { Value::Number(n) })
}

fn is_plain_object(rt: &Rt, v: &Value) -> bool {
    matches!(v, Value::Object(_)) && !rt.is_callable(v) && !rt.is_array(v)
}

/// `Object.prototype.toString.call(v) === "[object <tag>]"` (the tag test shared uses for Set and Map).
pub(crate) fn has_tag(rt: &Rt, v: &Value, tag: &str) -> Res<bool> {
    let proto = Value::Object(rt.intrinsic(vn_value::Intrinsic::ObjectPrototype));
    let to_string = rt.get_str(&proto, "toString")?;
    let s = rt.call(&to_string, v, &[])?;
    Ok(js_string(rt, &s)? == format!("[object {tag}]"))
}

/// shared `looseEqual` (3.5.43). Dates are not in the realm; Map and Set compare as collections.
pub(crate) fn loose_equal(rt: &Rt, a: &Value, b: &Value) -> Res<bool> {
    loose_equal_at(rt, a, b, 0)
}

fn loose_equal_at(rt: &Rt, a: &Value, b: &Value, depth: u32) -> Res<bool> {
    if vn_value::strict_eq(a, b) {
        return Ok(true);
    }
    if depth > 256 {
        return Ok(false);
    }
    if matches!(a, Value::Symbol(_)) || matches!(b, Value::Symbol(_)) {
        return Ok(false);
    }
    let (a_arr, b_arr) = (rt.is_array(a), rt.is_array(b));
    if a_arr || b_arr {
        if !(a_arr && b_arr) {
            return Ok(false);
        }
        return loose_compare_arrays(rt, a, b, depth);
    }
    let (a_obj, b_obj) = (is_plain_object(rt, a), is_plain_object(rt, b));
    if a_obj || b_obj {
        if !(a_obj && b_obj) {
            return Ok(false);
        }
        let (a_map, b_map) = (has_tag(rt, a, "Map")?, has_tag(rt, b, "Map")?);
        if a_map || b_map {
            return if a_map && b_map { loose_compare_collections(rt, a, b, depth) } else { Ok(false) };
        }
        let (a_set, b_set) = (has_tag(rt, a, "Set")?, has_tag(rt, b, "Set")?);
        if a_set || b_set {
            return if a_set && b_set { loose_compare_collections(rt, a, b, depth) } else { Ok(false) };
        }
        return loose_compare_objects(rt, a, b, depth);
    }
    Ok(js_string(rt, a)? == js_string(rt, b)?)
}

fn loose_compare_arrays(rt: &Rt, a: &Value, b: &Value, depth: u32) -> Res<bool> {
    let (xs, ys) = (array_items(rt, a)?, array_items(rt, b)?);
    if xs.len() != ys.len() {
        return Ok(false);
    }
    for (x, y) in xs.iter().zip(ys.iter()) {
        if !loose_equal_at(rt, x, y, depth + 1)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn loose_compare_objects(rt: &Rt, a: &Value, b: &Value, depth: u32) -> Res<bool> {
    let ka = enumerable_keys(rt, a)?;
    let kb = enumerable_keys(rt, b)?;
    if ka.len() != kb.len() {
        return Ok(false);
    }
    for key in &ka {
        let has_b = kb.iter().any(|k| k == key);
        if !has_b {
            return Ok(false);
        }
        let (x, y) = (rt.get_str(a, key)?, rt.get_str(b, key)?);
        if !loose_equal_at(rt, &x, &y, depth + 1)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// looseCompareCollections: equal sizes and a one-to-one match of the members under looseEqual.
fn loose_compare_collections(rt: &Rt, a: &Value, b: &Value, depth: u32) -> Res<bool> {
    let (sa, sb) = (rt.get_str(a, "size")?, rt.get_str(b, "size")?);
    if !vn_value::strict_eq(&sa, &sb) {
        return Ok(false);
    }
    let candidates = collection_items(rt, b)?;
    let mut matched = vec![false; candidates.len()];
    for item in collection_items(rt, a)? {
        let mut index = None;
        for (i, cand) in candidates.iter().enumerate() {
            if !matched[i] && loose_equal_at(rt, &item, cand, depth + 1)? {
                index = Some(i);
                break;
            }
        }
        match index {
            Some(i) => matched[i] = true,
            None => return Ok(false),
        }
    }
    Ok(true)
}

/// The members of a Set or Map in insertion order (`Array.from(x)` for a Set; keys-and-values are not needed here).
fn collection_items(rt: &Rt, v: &Value) -> Res<Vec<Value>> {
    let arr = vn_value::builtins::array::array_from_iterable(rt, v)?;
    array_items(rt, &Value::Object(arr))
}

/// `arr.findIndex(item => looseEqual(item, val))` (shared `looseIndexOf`).
pub(crate) fn loose_index_of(rt: &Rt, items: &[Value], val: &Value) -> Res<Option<usize>> {
    for (i, item) in items.iter().enumerate() {
        if loose_equal(rt, item, val)? {
            return Ok(Some(i));
        }
    }
    Ok(None)
}

fn tag_of(ctx: &DomCtx, el: HostNode) -> String {
    ctx.host().tag_name(el)
}

fn host_err(rt: &Rt, e: HostError) -> vn_value::Throw {
    host_error(rt, e)
}

/// `typeof el[key]` as the host reports it: undefined for names vn-dom does not model, `object` for object-valued ones.
fn typeof_prop(ctx: &DomCtx, el: HostNode, key: &str) -> &'static str {
    match ctx.host().get_property(el, key) {
        Ok(HostValue::Undefined) => "undefined",
        Ok(HostValue::Null) => "object",
        Ok(HostValue::Bool(_)) => "boolean",
        Ok(HostValue::Number(_)) => "number",
        Ok(HostValue::Str(_)) => "string",
        Err(_) => "object",
    }
}

/// `patchClass`: `className = value`, or `class` for SVG, with the transition classes (`_vtc`) appended.
pub(crate) fn patch_class(ctx: &DomCtx, el: HostNode, value: &Value, is_svg: bool) -> Res<()> {
    let rt = ctx.rt();
    let transition = ctx.with_expando(el, |ex| ex.vtc.clone());
    let joined: Value = match transition {
        Some(tc) => {
            let mut parts: Vec<String> = Vec::new();
            if value.to_boolean() {
                parts.push(js_string(rt, value)?);
            }
            parts.extend(tc);
            Value::String(JsStr::from_str(&parts.join(" ")))
        }
        None => value.clone(),
    };
    let mut host = ctx.host();
    if joined.is_nullish() {
        host.remove_attribute(el, "class").map_err(|e| host_err(rt, e))?;
    } else if is_svg {
        host.set_attribute(el, "class", &js_string(rt, &joined)?).map_err(|e| host_err(rt, e))?;
    } else {
        host.set_property(el, "className", HostValue::Str(js_string(rt, &joined)?)).map_err(|e| host_err(rt, e))?;
    }
    Ok(())
}

/// The `display:` test of runtime-dom (`/(?:^|;)\s*display\s*:/`).
fn display_re(css: &str) -> bool {
    let mut starts = vec![0usize];
    starts.extend(css.match_indices(';').map(|(i, _)| i + 1));
    starts.into_iter().any(|s| {
        let rest = css[s..].trim_start();
        rest.strip_prefix("display").map(|r| r.trim_start().starts_with(':')).unwrap_or(false)
    })
}

/// `importantRE = /\s*!important$/`: the value without the suffix, and whether it had one.
fn strip_important(s: &str) -> (String, bool) {
    match s.strip_suffix("!important") {
        Some(rest) => (rest.trim_end().to_string(), true),
        None => (s.to_string(), false),
    }
}

/// `autoPrefix` without vendor probing: camelCase the name (`font-size` and `fontSize` both give `fontSize`).
fn camelize(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '-' {
            if let Some(&n) = chars.peek() {
                if n.is_alphanumeric() || n == '_' {
                    chars.next();
                    out.extend(n.to_uppercase());
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

/// `setStyle(style, name, val)`: custom properties with `setProperty`; others assigned through their camel name.
/// An empty value removes the declaration, as the CSSOM does for both forms.
fn set_style(ctx: &DomCtx, el: HostNode, name: &str, val: &Value) -> Res<()> {
    let rt = ctx.rt();
    if rt.is_array(val) {
        for v in array_items(rt, val)? {
            set_style(ctx, el, name, &v)?;
        }
        return Ok(());
    }
    let raw = if val.is_nullish() { String::new() } else { js_string(rt, val)? };
    let (text, important) = strip_important(&raw);
    let mut host = ctx.host();
    let property = if name.starts_with("--") { name.to_string() } else { hyphenate(&camelize(name)) };
    let result = if text.is_empty() {
        host.style_remove_property(el, &property)
    } else {
        host.style_set_property(el, &property, &text, important)
    };
    // vn-dom answers Unsupported for a property it does not model (not for an invalid value). The contract requires an
    // unsupported construct to fail loudly, so it is reported as a TypeError instead of being dropped silently.
    result.map_err(|e| host_err(rt, e))
}

/// `patchStyle(el, prev, next)` with the v-show `_vod` / `_vsh` interplay.
pub(crate) fn patch_style(ctx: &DomCtx, el: HostNode, prev: &Value, next: &Value) -> Res<()> {
    let rt = ctx.rt();
    let is_css_string = matches!(next, Value::String(_));
    let mut has_controlled_display = false;
    if next.to_boolean() && !is_css_string {
        if prev.to_boolean() {
            if !matches!(prev, Value::String(_)) {
                for key in enumerable_keys(rt, prev)? {
                    if rt.get_str(next, &key)?.is_nullish() {
                        set_style(ctx, el, &key, &Value::Null)?;
                    }
                }
            } else {
                let prev_text = js_string(rt, prev)?;
                for prev_style in prev_text.split(';') {
                    let end = match prev_style.find(':') {
                        Some(i) => i,
                        // JS: slice(0, -1) when the colon is missing
                        None => prev_style.len().saturating_sub(1),
                    };
                    let key = prev_style.get(..end).unwrap_or("").trim().to_string();
                    if rt.get_str(next, &key)?.is_nullish() {
                        set_style(ctx, el, &key, &Value::Null)?;
                    }
                }
            }
        }
        for key in enumerable_keys(rt, next)? {
            if key == "display" {
                has_controlled_display = true;
            }
            let value = rt.get_str(next, &key)?;
            if !value.is_nullish() {
                let prev_value = if !matches!(prev, Value::String(_)) && prev.to_boolean() {
                    rt.get_str(prev, &key)?
                } else {
                    Value::Undefined
                };
                let preserve = tag_of(ctx, el) == "TEXTAREA"
                    && (key == "width" || key == "height")
                    && matches!(value, Value::String(_))
                    && vn_value::strict_eq(&prev_value, &value);
                if !preserve {
                    set_style(ctx, el, &key, &value)?;
                }
            } else {
                set_style(ctx, el, &key, &Value::String(JsStr::from_str("")))?;
            }
        }
    } else if is_css_string {
        if !vn_value::strict_eq(prev, next) {
            let text = js_string(rt, next)?;
            ctx.host().style_set_css_text(el, &text).map_err(|e| host_err(rt, e))?;
            has_controlled_display = display_re(&text);
        }
    } else if prev.to_boolean() {
        ctx.host().remove_attribute(el, "style").map_err(|e| host_err(rt, e))?;
    }
    let has_vod = ctx.with_expando(el, |ex| ex.vod.is_some());
    if has_vod {
        let mut host = ctx.host();
        let vod = if has_controlled_display { host.style_get_property(el, "display") } else { String::new() };
        let hidden = ctx.with_expando(el, |ex| {
            ex.vod = Some(vod);
            ex.vsh
        });
        if hidden {
            host.style_set_property(el, "display", "none", false).map_err(|e| host_err(rt, e))?;
        }
    }
    Ok(())
}

/// `patchAttr(el, key, value, isSVG, isBoolean)`.
pub(crate) fn patch_attr(ctx: &DomCtx, el: HostNode, key: &str, value: &Value, is_svg: bool, is_boolean: bool) -> Res<()> {
    let rt = ctx.rt();
    let mut host = ctx.host();
    if is_svg && key.starts_with("xlink:") {
        if value.is_nullish() {
            host.remove_attribute_ns(el, Some(XLINK_NS), &key[6..]).map_err(|e| host_err(rt, e))?;
        } else {
            host.set_attribute_ns(el, Some(XLINK_NS), key, &attr_string(rt, value)?).map_err(|e| host_err(rt, e))?;
        }
        return Ok(());
    }
    if value.is_nullish() || (is_boolean && !include_boolean_attr(value)) {
        host.remove_attribute(el, key).map_err(|e| host_err(rt, e))?;
    } else {
        let text = if is_boolean { String::new() } else { attr_string(rt, value)? };
        host.set_attribute(el, key, &text).map_err(|e| host_err(rt, e))?;
    }
    Ok(())
}

/// The string `setAttribute` stores: ToString, with `String(symbol)` for symbols.
fn attr_string(rt: &Rt, v: &Value) -> Res<String> {
    match v {
        Value::Symbol(s) => {
            let desc = rt.symbol_description(*s).map(|d| d.to_rust_string_lossy()).unwrap_or_default();
            Ok(format!("Symbol({desc})"))
        }
        other => js_string(rt, other),
    }
}

/// `patchDOMProp(el, key, value, _, attrName)`: the `value`, `innerHTML` and `textContent` special cases, and the
/// generic property assignment with the boolean and number defaults.
pub(crate) fn patch_dom_prop(ctx: &DomCtx, el: HostNode, key: &str, value: &Value, attr_name: Option<&str>) -> Res<()> {
    let rt = ctx.rt();
    if key == "innerHTML" || key == "textContent" {
        if !value.is_nullish() {
            if key == "innerHTML" {
                return Err(rt.throw_type_error("innerHTML is not supported by this host: a native host has no HTML parser"));
            }
            ctx.host().set_element_text(el, &js_string(rt, value)?).map_err(|e| host_err(rt, e))?;
        }
        return Ok(());
    }
    let tag = tag_of(ctx, el);
    if key == "value" && tag != "PROGRESS" && !tag.contains('-') {
        let mut host = ctx.host();
        let old_value = if tag == "OPTION" {
            host.get_attribute(el, "value").unwrap_or_default()
        } else {
            match host.get_property(el, "value") {
                Ok(HostValue::Str(s)) => s,
                Ok(other) => match other {
                    HostValue::Bool(b) => b.to_string(),
                    HostValue::Number(n) => n.to_string(),
                    _ => String::new(),
                },
                Err(_) => String::new(),
            }
        };
        let new_value = if value.is_nullish() {
            let is_checkbox = matches!(host.get_property(el, "type"), Ok(HostValue::Str(ref s)) if s == "checkbox");
            if is_checkbox { "on".to_string() } else { String::new() }
        } else {
            js_string(rt, value)?
        };
        let has_value = ctx.with_expando(el, |ex| ex.value.is_some());
        if old_value != new_value || !has_value {
            host.set_property(el, "value", HostValue::Str(new_value)).map_err(|e| host_err(rt, e))?;
        }
        if value.is_nullish() {
            host.remove_attribute(el, key).map_err(|e| host_err(rt, e))?;
        }
        ctx.with_expando(el, |ex| ex.value = Some(value.clone()));
        return Ok(());
    }
    let mut value = value.clone();
    let mut need_remove = false;
    let empty = value.is_nullish() || matches!(&value, Value::String(s) if s.is_empty());
    if empty {
        let ty = typeof_prop(ctx, el, key);
        if ty == "boolean" {
            value = Value::Bool(include_boolean_attr(&value));
        } else if value.is_nullish() && ty == "string" {
            value = Value::String(JsStr::from_str(""));
            need_remove = true;
        } else if ty == "number" {
            value = Value::Number(0.0);
            need_remove = true;
        }
    }
    // `try { el[key] = value } catch (e) {}`: a refused assignment is ignored.
    if let Ok(hv) = to_host_value(rt, &value) {
        let _ = ctx.host().set_property(el, key, hv);
    }
    if need_remove {
        let _ = ctx.host().remove_attribute(el, attr_name.unwrap_or(key));
    }
    Ok(())
}

/// `shouldSetAsProp(el, key, value, isSVG)`.
pub(crate) fn should_set_as_prop(ctx: &DomCtx, el: HostNode, key: &str, value: &Value, is_svg: bool) -> bool {
    let host = ctx.host();
    if is_svg {
        if key == "innerHTML" || key == "textContent" {
            return true;
        }
        return host.has_property(el, key) && is_native_on(key) && ctx.rt().is_callable(value);
    }
    let tag = host.tag_name(el);
    match key {
        "spellcheck" | "draggable" | "translate" | "autocorrect" => return false,
        "sandbox" if tag == "IFRAME" => return false,
        "form" => return false,
        "list" if tag == "INPUT" => return false,
        "type" if tag == "TEXTAREA" => return false,
        "width" | "height" if matches!(tag.as_str(), "IMG" | "VIDEO" | "CANVAS" | "SOURCE") => return false,
        _ => {}
    }
    if is_native_on(key) && matches!(value, Value::String(_)) {
        return false;
    }
    host.has_property(el, key)
}

/// `patchProp(el, key, prev, next, namespace)` (runtime-dom 3.5.43), with the element in the host.
pub fn patch_prop(ctx: &DomCtx, el: HostNode, key: &str, prev: &Value, next: &Value, ns: HostNs) -> Res<()> {
    patch_prop_with_owner(ctx, el, key, prev, next, ns, None)
}

/// Component-owned prop patching carries the instance into native event error handling.
pub(crate) fn patch_prop_with_owner(ctx: &DomCtx, el: HostNode, key: &str, prev: &Value, next: &Value, ns: HostNs, owner: Option<ObjRef>) -> Res<()> {
    let is_svg = ns == HostNs::Svg;
    if key == "class" {
        return patch_class(ctx, el, next, is_svg);
    }
    if key == "style" {
        return patch_style(ctx, el, prev, next);
    }
    if is_on(key) {
        if !key.starts_with("onUpdate:") {
            ctx.patch_event(el, key, next, owner)?;
        }
        return Ok(());
    }
    let (name, as_prop) = if let Some(k) = key.strip_prefix('.') {
        (k.to_string(), true)
    } else if let Some(k) = key.strip_prefix('^') {
        (k.to_string(), false)
    } else {
        let p = should_set_as_prop(ctx, el, key, next, is_svg);
        (key.to_string(), p)
    };
    if as_prop {
        patch_dom_prop(ctx, el, &name, next, None)?;
        let tag = tag_of(ctx, el);
        if !tag.contains('-') && (name == "value" || name == "checked" || name == "selected") {
            patch_attr(ctx, el, &name, next, is_svg, name != "value")?;
        }
    } else {
        if name == "true-value" {
            ctx.with_expando(el, |ex| ex.true_value = Some(next.clone()));
        } else if name == "false-value" {
            ctx.with_expando(el, |ex| ex.false_value = Some(next.clone()));
        }
        patch_attr(ctx, el, &name, next, is_svg, is_special_boolean_attr(&name))?;
    }
    Ok(())
}
