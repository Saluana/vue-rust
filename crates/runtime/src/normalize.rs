//! vn-runtime::normalize: the pure helpers of @vue/shared 3.5.43 (class and style normalization, toDisplayString, name
//! transforms, loose comparison). Behaviour per @vue/shared 3.5.43. Each function takes the runtime because JavaScript
//! values may run user code (getters, toString, JSON.stringify). Native wrappers are installed by `exports.rs`.

use vn_value::builtins::iterator;
use vn_value::{number, Intrinsic, JsStr, ObjRef, PropKey, Res, Rt, Value};

/// `isObject` of @vue/shared: a non-null object that is not callable.
pub(crate) fn is_obj(rt: &Rt, v: &Value) -> bool {
    matches!(v, Value::Object(_)) && !rt.is_callable(v)
}

/// `length` of an array-like value, as a number (NaN when absent).
pub(crate) fn length_of(rt: &Rt, v: &Value) -> Res<f64> {
    let l = rt.get_str(v, "length")?;
    rt.to_number(&l)
}

pub(crate) fn array_get(rt: &Rt, v: &Value, i: usize) -> Res<Value> {
    rt.get_v(v, &PropKey::Index(i as u32))
}

/// A property key as its string form (array indices in decimal). Symbols give the empty string (callers skip them).
pub(crate) fn key_js(k: &PropKey) -> JsStr {
    match k {
        PropKey::Index(i) => JsStr::from_str(&i.to_string()),
        PropKey::Str(s) => s.clone(),
        PropKey::Sym(_) => JsStr::empty(),
    }
}

/// The keys of `for (const k in v)`: enumerable string keys of the object and its prototypes, own keys first,
/// shadowed names skipped (a non-enumerable own key hides an enumerable inherited one).
pub(crate) fn for_in_keys(rt: &Rt, v: &Value) -> Res<Vec<PropKey>> {
    if v.is_nullish() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut seen: std::collections::HashSet<PropKey> = std::collections::HashSet::new();
    let mut cur = Some(rt.to_object(v)?);
    while let Some(obj) = cur {
        for k in rt.own_keys(obj)? {
            if matches!(k, PropKey::Sym(_)) || !seen.insert(k.clone()) {
                continue;
            }
            if let Some(p) = rt.get_own_property(obj, &k)? {
                if p.flags().enumerable {
                    out.push(k);
                }
            }
        }
        cur = rt.get_prototype_of(obj);
    }
    Ok(out)
}

fn is_word(c: u16) -> bool {
    matches!(c, 0x30..=0x39 | 0x41..=0x5A | 0x61..=0x7A | 0x5F)
}

fn ascii_upper(c: u16) -> u16 {
    if (0x61..=0x7A).contains(&c) { c - 32 } else { c }
}

// ----- class and style -----

/// normalizeClass: strings as they are, arrays joined with spaces, objects by their truthy keys; trimmed.
pub fn normalize_class(rt: &Rt, v: &Value) -> Res<JsStr> {
    let mut res: Vec<u16> = Vec::new();
    if let Value::String(s) = v {
        res.extend_from_slice(s.units());
    } else if rt.is_array(v) {
        let mut i = 0usize;
        while (i as f64) < length_of(rt, v)? {
            let n = normalize_class(rt, &array_get(rt, v, i)?)?;
            if !n.is_empty() {
                res.extend_from_slice(n.units());
                res.push(32);
            }
            i += 1;
        }
    } else if is_obj(rt, v) {
        for k in for_in_keys(rt, v)? {
            if rt.get_v(v, &k)?.to_boolean() {
                res.extend_from_slice(key_js(&k).units());
                res.push(32);
            }
        }
    }
    Ok(JsStr::from_vec(res).trim())
}

fn comment_end(u: &[u16], i: usize) -> Option<usize> {
    // `/\*[^]*?\*\/` starting at i: the first "*/" at or after i + 2.
    let mut j = i + 2;
    while j + 1 < u.len() {
        if u[j] == 0x2A && u[j + 1] == 0x2F {
            return Some(j + 2);
        }
        j += 1;
    }
    None
}

fn quoted_end(u: &[u16], i: usize) -> Option<usize> {
    // `"(?:[^"\\]|\\[^])*"` (or the single-quoted form) starting at i.
    let q = u[i];
    let mut j = i + 1;
    while j < u.len() {
        if u[j] == q {
            return Some(j + 1);
        }
        if u[j] == 0x5C {
            if j + 1 >= u.len() {
                return None;
            }
            j += 2;
        } else {
            j += 1;
        }
    }
    None
}

/// `cssText.replace(styleCommentRE, m => m.startsWith('/*') ? '' : m)` with
/// `styleCommentRE = /"…"|'…'|\\[^]|\/\*[^]*?\*\//g`: quoted strings and escapes are kept, comments are removed.
fn strip_style_comments(u: &[u16]) -> Vec<u16> {
    let mut out = Vec::with_capacity(u.len());
    let mut i = 0;
    while i < u.len() {
        let c = u[i];
        if (c == 0x22 || c == 0x27) && quoted_end(u, i).is_some() {
            let end = quoted_end(u, i).unwrap_or(i + 1);
            out.extend_from_slice(&u[i..end]);
            i = end;
            continue;
        }
        if c == 0x5C && i + 1 < u.len() {
            out.extend_from_slice(&u[i..i + 2]);
            i += 2;
            continue;
        }
        if c == 0x2F && i + 1 < u.len() && u[i + 1] == 0x2A {
            if let Some(end) = comment_end(u, i) {
                i = end;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// True when the `;` at `p` is inside parentheses: `(?![^(]*\))` fails, i.e. a `)` comes before any `(` after it.
fn semicolon_protected(u: &[u16], p: usize) -> bool {
    for &c in &u[p + 1..] {
        if c == 0x28 {
            return false;
        }
        if c == 0x29 {
            return true;
        }
    }
    false
}

fn trim_units(u: &[u16]) -> JsStr {
    JsStr::from_units(u).trim()
}

/// parseStringStyle: declarations split at unprotected `;`, each at its first `:` (with a character after it), keys and
/// values trimmed. Quoted strings and comments are handled as in `strip_style_comments`.
pub fn parse_string_style(rt: &Rt, s: &JsStr) -> Res<ObjRef> {
    let ret = rt.new_object();
    let u = strip_style_comments(s.units());
    let mut items: Vec<&[u16]> = Vec::new();
    let mut start = 0;
    for (p, &c) in u.iter().enumerate() {
        if c == 0x3B && !semicolon_protected(&u, p) {
            items.push(&u[start..p]);
            start = p + 1;
        }
    }
    items.push(&u[start..]);
    for item in items {
        if item.is_empty() {
            continue;
        }
        if let Some(colon) = item.iter().position(|&c| c == 0x3A) {
            if colon + 1 < item.len() {
                let key = trim_units(&item[..colon]);
                let val = trim_units(&item[colon + 1..]);
                set_key(rt, &Value::Object(ret), &PropKey::from_jsstr(key), Value::String(val))?;
            }
        }
    }
    Ok(ret)
}

/// normalizeStyle: arrays merge their items (strings are parsed), objects and strings are returned as they are,
/// anything else gives undefined.
pub fn normalize_style(rt: &Rt, v: &Value) -> Res<Value> {
    if rt.is_array(v) {
        let res = rt.new_object();
        let mut i = 0usize;
        while (i as f64) < length_of(rt, v)? {
            let item = array_get(rt, v, i)?;
            let normalized = match &item {
                Value::String(s) => Value::Object(parse_string_style(rt, s)?),
                _ => normalize_style(rt, &item)?,
            };
            if normalized.to_boolean() {
                for k in for_in_keys(rt, &normalized)? {
                    let val = rt.get_v(&normalized, &k)?;
                    set_key(rt, &Value::Object(res), &k, val)?;
                }
            }
            i += 1;
        }
        return Ok(Value::Object(res));
    }
    if matches!(v, Value::String(_)) || is_obj(rt, v) {
        return Ok(v.clone());
    }
    Ok(Value::Undefined)
}

/// stringifyStyle: a string is returned as it is; objects give `name:value;` for string and number values.
pub fn stringify_style(rt: &Rt, v: &Value) -> Res<JsStr> {
    if !v.to_boolean() {
        return Ok(JsStr::empty());
    }
    if let Value::String(s) = v {
        return Ok(s.clone());
    }
    let mut out: Vec<u16> = Vec::new();
    for k in for_in_keys(rt, v)? {
        let val = rt.get_v(v, &k)?;
        if matches!(val, Value::String(_) | Value::Number(_)) {
            let name = key_js(&k);
            let name = if name.units().starts_with(&[0x2D, 0x2D]) { name } else { hyphenate(&name) };
            out.extend_from_slice(name.units());
            out.push(0x3A);
            out.extend_from_slice(rt.to_string(&val)?.units());
            out.push(0x3B);
        }
    }
    Ok(JsStr::from_vec(out))
}

/// normalizeProps: class and style normalized in place; a falsy argument gives null.
pub fn normalize_props(rt: &Rt, props: &Value) -> Res<Value> {
    if !props.to_boolean() {
        return Ok(Value::Null);
    }
    let klass = rt.get_str(props, "class")?;
    let style = rt.get_str(props, "style")?;
    if klass.to_boolean() && !matches!(klass, Value::String(_)) {
        rt.set_v(props, &PropKey::lit("class"), Value::String(normalize_class(rt, &klass)?))?;
    }
    if style.to_boolean() {
        let n = normalize_style(rt, &style)?;
        rt.set_v(props, &PropKey::lit("style"), n)?;
    }
    Ok(props.clone())
}

// ----- names -----

/// hyphenate: `str.replace(/\B([A-Z])/g, '-$1').toLowerCase()`.
pub fn hyphenate(s: &JsStr) -> JsStr {
    let u = s.units();
    let mut out: Vec<u16> = Vec::with_capacity(u.len() + 4);
    for (i, &c) in u.iter().enumerate() {
        if (0x41..=0x5A).contains(&c) && i > 0 && is_word(u[i - 1]) {
            out.push(0x2D);
        }
        out.push(c);
    }
    JsStr::from_vec(out).to_lower_case()
}

/// camelize: `str.replace(/-\w/g, c => c.slice(1).toUpperCase())`.
pub fn camelize(s: &JsStr) -> JsStr {
    let u = s.units();
    let mut out: Vec<u16> = Vec::with_capacity(u.len());
    let mut i = 0;
    while i < u.len() {
        if u[i] == 0x2D && i + 1 < u.len() && is_word(u[i + 1]) {
            out.push(ascii_upper(u[i + 1]));
            i += 2;
        } else {
            out.push(u[i]);
            i += 1;
        }
    }
    JsStr::from_vec(out)
}

/// capitalize: `str.charAt(0).toUpperCase() + str.slice(1)`.
pub fn capitalize(s: &JsStr) -> JsStr {
    let u = s.units();
    if u.is_empty() {
        return JsStr::empty();
    }
    JsStr::from_units(&u[..1]).to_upper_case().concat(&JsStr::from_units(&u[1..]))
}

/// toHandlerKey: `str ? 'on' + capitalize(str) : ''`. A truthy non-string throws, as `str.charAt` does.
pub fn to_handler_key(rt: &Rt, v: &Value) -> Res<JsStr> {
    if !v.to_boolean() {
        return Ok(JsStr::empty());
    }
    match v {
        Value::String(s) => Ok(JsStr::from_str("on").concat(&capitalize(s))),
        _ => Err(rt.throw_type_error("str.charAt is not a function")),
    }
}

/// camelize / capitalize of a non-string throw a TypeError (`str.replace` / `str.charAt` is not a function).
pub fn expect_str<'a>(rt: &Rt, v: &'a Value, what: &str) -> Res<&'a JsStr> {
    match v {
        Value::String(s) => Ok(s),
        _ => Err(rt.throw_type_error(&format!("str.{what} is not a function"))),
    }
}

/// isOn: `key` starts with "on" and the third character is not a lowercase letter.
pub fn is_on(s: &JsStr) -> bool {
    let u = s.units();
    u.len() >= 3 && u[0] == 0x6F && u[1] == 0x6E && (u[2] > 0x7A || u[2] < 0x61)
}

// ----- loose conversions -----

/// looseToNumber: parseFloat of the value; the value itself when that is NaN.
pub fn loose_to_number(rt: &Rt, v: &Value) -> Res<Value> {
    let n = number::parse_float(&rt.to_string(v)?);
    Ok(if n.is_nan() { v.clone() } else { Value::Number(n) })
}

/// toNumber: Number(value) for strings; the value itself when that is NaN (or the value is not a string).
pub fn to_number_shared(v: &Value) -> Value {
    let n = match v {
        Value::String(s) => number::string_to_number(s),
        _ => f64::NAN,
    };
    if n.is_nan() { v.clone() } else { Value::Number(n) }
}

/// looseIndexOf: the first index whose element is loosely equal to `val`, or -1.
pub fn loose_index_of(rt: &Rt, arr: &Value, val: &Value) -> Res<f64> {
    let mut i = 0usize;
    while (i as f64) < length_of(rt, arr)? {
        let item = array_get(rt, arr, i)?;
        if loose_equal(rt, &item, val)? {
            return Ok(i as f64);
        }
        i += 1;
    }
    Ok(-1.0)
}

/// The realm's `Object.prototype.toString` applied to `v` (`toTypeString`), as a string.
fn to_type_string(rt: &Rt, v: &Value) -> Res<JsStr> {
    let f = rt.get_v(&Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype)), &PropKey::lit("toString"))?;
    let s = rt.call(&f, v, &[])?;
    rt.to_string(&s)
}

fn tag_is(rt: &Rt, v: &Value, tag: &str) -> Res<bool> {
    Ok(to_type_string(rt, v)?.units() == JsStr::from_str(&format!("[object {tag}]")).units())
}

/// The keys of `Object.keys(v)` for an object (symbols excluded).
fn own_enum_keys(rt: &Rt, v: &Value) -> Res<Vec<PropKey>> {
    let Value::Object(o) = v else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for k in rt.own_keys(*o)? {
        if matches!(k, PropKey::Sym(_)) {
            continue;
        }
        if rt.get_own_property(*o, &k)?.is_some_and(|p| p.flags().enumerable) {
            out.push(k);
        }
    }
    Ok(out)
}

/// `Array.from(v)` for an iterable (a Map, a Set or an iterator-backed value).
fn collect_iter(rt: &Rt, v: &Value) -> Res<Vec<Value>> {
    let mut out = Vec::new();
    iterator::iterate(rt, v, &mut |x| {
        out.push(x);
        Ok(true)
    })?;
    Ok(out)
}

/// looseEqual (@vue/shared 3.5.43). `seen` holds the pairs under comparison, one list per side (the two Maps of the
/// reference): a pair that meets itself again is equal only when both sides still point at each other.
pub fn loose_equal(rt: &Rt, a: &Value, b: &Value) -> Res<bool> {
    loose_equal_in(rt, a, b, &mut Seen::default())
}

#[derive(Default)]
struct Seen {
    a: Vec<(Value, Value)>,
    b: Vec<(Value, Value)>,
}

fn same(x: &Value, y: &Value) -> bool {
    vn_value::strict_eq(x, y)
}

fn loose_equal_in(rt: &Rt, a: &Value, b: &Value, seen: &mut Seen) -> Res<bool> {
    if same(a, b) {
        return Ok(true);
    }
    let (ad, bd) = (tag_is(rt, a, "Date")?, tag_is(rt, b, "Date")?);
    if ad || bd {
        if !(ad && bd) {
            return Ok(false);
        }
        let ga = rt.get_str(a, "getTime")?;
        let ta = rt.call(&ga, a, &[])?;
        let gb = rt.get_str(b, "getTime")?;
        let tb = rt.call(&gb, b, &[])?;
        return Ok(same(&ta, &tb));
    }
    if matches!(a, Value::Symbol(_)) || matches!(b, Value::Symbol(_)) {
        return Ok(same(a, b));
    }
    let (aa, ba) = (rt.is_array(a), rt.is_array(b));
    if aa || ba {
        return if aa && ba { nested(rt, a, b, seen, false) } else { Ok(false) };
    }
    let (ao, bo) = (is_obj(rt, a), is_obj(rt, b));
    if ao || bo {
        return if ao && bo { nested(rt, a, b, seen, true) } else { Ok(false) };
    }
    let (sa, sb) = (rt.to_string(a)?, rt.to_string(b)?);
    Ok(sa.units() == sb.units())
}

fn nested(rt: &Rt, a: &Value, b: &Value, seen: &mut Seen, objects: bool) -> Res<bool> {
    if seen.a.iter().any(|(k, _)| same(k, a)) || seen.b.iter().any(|(k, _)| same(k, b)) {
        let ga = seen.a.iter().find(|(k, _)| same(k, a)).map_or(Value::Undefined, |(_, v)| v.clone());
        let gb = seen.b.iter().find(|(k, _)| same(k, b)).map_or(Value::Undefined, |(_, v)| v.clone());
        return Ok(same(&ga, b) && same(&gb, a));
    }
    seen.a.push((a.clone(), b.clone()));
    seen.b.push((b.clone(), a.clone()));
    let r = if objects { compare_objects(rt, a, b, seen) } else { compare_arrays(rt, a, b, seen) };
    if let Some(p) = seen.a.iter().position(|(k, _)| same(k, a)) {
        seen.a.remove(p);
    }
    if let Some(p) = seen.b.iter().position(|(k, _)| same(k, b)) {
        seen.b.remove(p);
    }
    r
}

fn compare_arrays(rt: &Rt, a: &Value, b: &Value, seen: &mut Seen) -> Res<bool> {
    if length_of(rt, a)? != length_of(rt, b)? {
        return Ok(false);
    }
    let mut i = 0usize;
    while (i as f64) < length_of(rt, a)? {
        if !loose_equal_in(rt, &array_get(rt, a, i)?, &array_get(rt, b, i)?, seen)? {
            return Ok(false);
        }
        i += 1;
    }
    Ok(true)
}

fn compare_objects(rt: &Rt, a: &Value, b: &Value, seen: &mut Seen) -> Res<bool> {
    for tag in ["Map", "Set"] {
        let (x, y) = (tag_is(rt, a, tag)?, tag_is(rt, b, tag)?);
        if x || y {
            return Ok(x && y && compare_collections(rt, a, b, seen)?);
        }
    }
    if own_enum_keys(rt, a)?.len() != own_enum_keys(rt, b)?.len() {
        return Ok(false);
    }
    for k in for_in_keys(rt, a)? {
        let ah = matches!(a, Value::Object(o) if rt.has_own_property(*o, &k)?);
        let bh = matches!(b, Value::Object(o) if rt.has_own_property(*o, &k)?);
        if (ah && !bh) || (!ah && bh) {
            return Ok(false);
        }
        let (va, vb) = (rt.get_v(a, &k)?, rt.get_v(b, &k)?);
        if !loose_equal_in(rt, &va, &vb, seen)? {
            return Ok(false);
        }
    }
    let (sa, sb) = (rt.to_string(a)?, rt.to_string(b)?);
    Ok(sa.units() == sb.units())
}

fn compare_collections(rt: &Rt, a: &Value, b: &Value, seen: &mut Seen) -> Res<bool> {
    let (sa, sb) = (rt.get_str(a, "size")?, rt.get_str(b, "size")?);
    if !same(&sa, &sb) {
        return Ok(false);
    }
    let cand = collect_iter(rt, b)?;
    let mut matched = vec![false; cand.len()];
    for item in collect_iter(rt, a)? {
        let mut index = None;
        for (i, c) in cand.iter().enumerate() {
            if !matched[i] && loose_equal_in(rt, &item, c, seen)? {
                index = Some(i);
                break;
            }
        }
        match index {
            None => return Ok(false),
            Some(i) => matched[i] = true,
        }
    }
    Ok(true)
}

// ----- toDisplayString -----

/// The realm's `Object.prototype.toString` applied to `v` (`toTypeString`).
fn is_tagged(rt: &Rt, v: &Value, tag: &str) -> Res<bool> {
    tag_is(rt, v, tag)
}

pub(crate) fn is_ref(rt: &Rt, v: &Value) -> Res<bool> {
    if !v.to_boolean() {
        return Ok(false);
    }
    Ok(matches!(rt.get_v(v, &PropKey::lit("__v_isRef"))?, Value::Bool(true)))
}

/// `stringifySymbol(v, i)`: a symbol becomes `Symbol(description)` (or `Symbol(i)` when it has none); others pass.
fn stringify_symbol(rt: &Rt, v: &Value, i: &Value) -> Value {
    match v {
        Value::Symbol(s) => {
            let mut units: Vec<u16> = "Symbol(".encode_utf16().collect();
            match rt.symbol_description(*s) {
                Some(d) => units.extend_from_slice(d.units()),
                None => {
                    let t = match i {
                        Value::String(t) => t.clone(),
                        Value::Number(n) => JsStr::from_str(&number::number_to_string(*n)),
                        _ => JsStr::empty(),
                    };
                    units.extend_from_slice(t.units());
                }
            }
            units.push(0x29);
            Value::String(JsStr::from_vec(units))
        }
        other => other.clone(),
    }
}

/// `String(v)` (symbols give `Symbol(desc)`).
pub fn string_of(rt: &Rt, v: &Value) -> Res<JsStr> {
    match v {
        Value::Symbol(_) => match stringify_symbol(rt, v, &Value::Undefined) {
            Value::String(s) => Ok(s),
            _ => Ok(JsStr::empty()),
        },
        _ => rt.to_string(v),
    }
}

fn replacer_value(rt: &Rt, val: Value) -> Res<Value> {
    if is_ref(rt, &val)? {
        let inner = rt.get_str(&val, "value")?;
        return replacer_value(rt, inner);
    }
    if is_tagged(rt, &val, "Map")? {
        let size = rt.get_str(&val, "size")?;
        let entries = rt.call(&rt.get_str(&val, "entries")?, &val, &[])?;
        let collected = rt.new_object();
        let mut i = 0usize;
        let mut pending: Vec<Value> = Vec::new();
        iterator::iterate(rt, &entries, &mut |e| {
            pending.push(e);
            Ok(true)
        })?;
        for e in pending {
            let key = array_get(rt, &e, 0)?;
            let val2 = array_get(rt, &e, 1)?;
            let shown = stringify_symbol(rt, &key, &Value::Number(i as f64));
            let k = rt.add(&shown, &Value::str(" =>"))?;
            rt.set_v(&Value::Object(collected), &rt.to_property_key(&k)?, val2)?;
            i += 1;
        }
        let label = JsStr::from_str("Map(").concat(&rt.to_string(&size)?).concat(&JsStr::from_str(")"));
        let out = rt.new_object();
        rt.set_v(&Value::Object(out), &PropKey::from_jsstr(label), Value::Object(collected))?;
        return Ok(Value::Object(out));
    }
    if is_tagged(rt, &val, "Set")? {
        let size = rt.get_str(&val, "size")?;
        let values = rt.call(&rt.get_str(&val, "values")?, &val, &[])?;
        let mut items = Vec::new();
        iterator::iterate(rt, &values, &mut |v| {
            items.push(v);
            Ok(true)
        })?;
        let mapped: Vec<Value> = items.iter().map(|v| stringify_symbol(rt, v, &Value::str(""))).collect();
        let label = JsStr::from_str("Set(").concat(&rt.to_string(&size)?).concat(&JsStr::from_str(")"));
        let out = rt.new_object();
        rt.set_v(&Value::Object(out), &PropKey::from_jsstr(label), Value::Object(rt.new_array(mapped)))?;
        return Ok(Value::Object(out));
    }
    if let Value::Symbol(_) = val {
        return Ok(stringify_symbol(rt, &val, &Value::str("")));
    }
    if is_obj(rt, &val) && !rt.is_array(&val) && !is_tagged(rt, &val, "Object")? {
        return Ok(Value::String(rt.to_string(&val)?));
    }
    Ok(val)
}

/// The `replacer` of @vue/shared (its `_key` argument is unused).
pub(crate) fn display_replacer_code(rt: &Rt, c: &vn_value::Call<'_>) -> Res<Value> {
    replacer_value(rt, c.arg(1))
}

/// toDisplayString: strings as they are, null and undefined as "", arrays and plain objects through JSON.stringify
/// with the display replacer (indent 2), other objects through String().
pub fn to_display_string(rt: &Rt, v: &Value) -> Res<Value> {
    if let Value::String(_) = v {
        return Ok(v.clone());
    }
    if v.is_nullish() {
        return Ok(Value::str(""));
    }
    let object_tostring = rt.get_v(&Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype)), &PropKey::lit("toString"))?;
    let plain_candidate = if rt.is_array(v) {
        true
    } else if is_obj(rt, v) {
        let ts = rt.get_str(v, "toString")?;
        vn_value::strict_eq(&ts, &object_tostring) || !rt.is_callable(&ts)
    } else {
        false
    };
    if plain_candidate {
        if is_ref(rt, v)? {
            let inner = rt.get_str(v, "value")?;
            return to_display_string(rt, &inner);
        }
        let json = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), "JSON")?;
        let stringify = rt.get_str(&json, "stringify")?;
        let replacer = rt.new_native_function(display_replacer_code, "", 2, Vec::new());
        return rt.call(&stringify, &json, &[v.clone(), Value::Object(replacer), Value::Number(2.0)]);
    }
    Ok(Value::String(string_of(rt, v)?))
}


// ----- native exports -----

fn n_normalize_class(rt: &Rt, c: &vn_value::Call<'_>) -> Res<Value> {
    Ok(Value::String(normalize_class(rt, &c.arg(0))?))
}

fn n_normalize_style(rt: &Rt, c: &vn_value::Call<'_>) -> Res<Value> {
    normalize_style(rt, &c.arg(0))
}

fn n_normalize_props(rt: &Rt, c: &vn_value::Call<'_>) -> Res<Value> {
    normalize_props(rt, &c.arg(0))
}

fn n_to_display_string(rt: &Rt, c: &vn_value::Call<'_>) -> Res<Value> {
    to_display_string(rt, &c.arg(0))
}

fn n_to_handler_key(rt: &Rt, c: &vn_value::Call<'_>) -> Res<Value> {
    Ok(Value::String(to_handler_key(rt, &c.arg(0))?))
}

fn n_camelize(rt: &Rt, c: &vn_value::Call<'_>) -> Res<Value> {
    let v = c.arg(0);
    Ok(Value::String(camelize(expect_str(rt, &v, "replace")?)))
}

fn n_capitalize(rt: &Rt, c: &vn_value::Call<'_>) -> Res<Value> {
    let v = c.arg(0);
    Ok(Value::String(capitalize(expect_str(rt, &v, "charAt")?)))
}

/// The exports of this module (no state needed).
pub(crate) fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    use crate::vnode::func;
    vec![
        func(rt, "normalizeClass", 1, n_normalize_class, &[]),
        func(rt, "normalizeStyle", 1, n_normalize_style, &[]),
        func(rt, "normalizeProps", 1, n_normalize_props, &[]),
        func(rt, "toDisplayString", 1, n_to_display_string, &[]),
        func(rt, "toHandlerKey", 1, n_to_handler_key, &[]),
        func(rt, "camelize", 1, n_camelize, &[]),
        func(rt, "capitalize", 1, n_capitalize, &[]),
    ]
}

/// `obj[key] = value` as JavaScript writes it. A key of "__proto__" goes through Object.prototype.__proto__, which
/// ignores primitive values and sets the prototype for objects and null (the kernel has no such accessor yet).
pub(crate) fn set_key(rt: &Rt, obj: &Value, key: &PropKey, value: Value) -> Res<()> {
    if let (PropKey::Str(s), Value::Object(o)) = (key, obj) {
        if s.units() == "__proto__".encode_utf16().collect::<Vec<u16>>().as_slice() {
            return match value {
                Value::Object(p) if rt.set_prototype_of(*o, Some(p)) => Ok(()),
                Value::Null if rt.set_prototype_of(*o, None) => Ok(()),
                Value::Object(_) | Value::Null => Err(rt.throw_type_error("Object.prototype.__proto__ setter: cyclic prototype")),
                _ => Ok(()),
            };
        }
    }
    rt.set_v(obj, key, value)?;
    Ok(())
}
