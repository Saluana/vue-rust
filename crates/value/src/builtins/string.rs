//! Builtin family: String (ECMA-262 22.1). Argument order and `this` checks follow the spec; error types and the
//! string length limit follow V8. Not implemented, so absent: localeCompare, normalize, match, matchAll, search,
//! the locale case methods, trimLeft, trimRight, String.raw and the HTML methods.
//! replace, replaceAll and split take string patterns only: a pattern that is not a string is converted with
//! ToString. The @@replace and @@split lookups are skipped, since the runtime has no RegExp and no such symbols.

use std::cell::Cell;
use std::rc::Rc;

use crate::builtins::iterator::{make_iterator_prototype, new_iterator_object, NativeIter};
use crate::function::{Call, CodeFn};
use crate::jsstr::JsStr;
use crate::native::NativeObject;
use crate::number;
use crate::object::{Flags, Obj, ObjectKind, Prop, MAX_DENSE_LEN};
use crate::roots::Tracer;
use crate::rt::{Intrinsic, Rt, WellKnown};
use crate::value::{ObjRef, PropKey, Res, Sym, Throw, Value};

/// V8's longest string, 2^29 - 24 code units. A longer result throws RangeError "Invalid string length" before anything
/// is allocated. This is lower than the kernel's `jsstr::MAX_LEN`, which is what V8 does.
const MAX_LEN: usize = (1 << 29) - 24;

pub fn install(rt: &Rt) {
    let proto = rt.intrinsic(Intrinsic::StringPrototype);
    // String.prototype is itself a String wrapper whose value is "" (ECMA-262 22.1.3). The kernel creates it as an
    // ordinary object, so its kind is set here.
    rt.with_obj_mut(proto, |o| o.kind = ObjectKind::Wrapper(Value::String(JsStr::empty())));
    let ctor = rt.new_native_constructor(string_call, string_construct, "String", 1, Vec::new());
    rt.define_builtin(ctor, PropKey::from_str("prototype"), Prop::Data { value: Value::Object(proto), flags: Flags::NONE });
    rt.define_builtin(proto, PropKey::from_str("constructor"), Prop::Data { value: Value::Object(ctor), flags: Flags::HIDDEN });

    let statics: &[(&str, CodeFn, u32)] = &[("fromCharCode", from_char_code, 1), ("fromCodePoint", from_code_point, 1)];
    for (name, code, len) in statics {
        rt.define_native_method(ctor, name, *code, *len);
    }
    let methods: &[(&str, CodeFn, u32)] = &[
        ("at", at, 1),
        ("charAt", char_at, 1),
        ("charCodeAt", char_code_at, 1),
        ("codePointAt", code_point_at, 1),
        ("concat", concat, 1),
        ("endsWith", ends_with, 1),
        ("includes", includes, 1),
        ("indexOf", index_of, 1),
        ("isWellFormed", is_well_formed, 0),
        ("lastIndexOf", last_index_of, 1),
        ("padEnd", pad_end, 1),
        ("padStart", pad_start, 1),
        ("repeat", repeat, 1),
        ("replace", replace, 2),
        ("replaceAll", replace_all, 2),
        ("slice", slice, 2),
        ("split", split, 2),
        ("startsWith", starts_with, 1),
        ("substr", substr, 2),
        ("substring", substring, 2),
        ("toLowerCase", to_lower_case, 0),
        ("toString", to_string_method, 0),
        ("toUpperCase", to_upper_case, 0),
        ("toWellFormed", to_well_formed, 0),
        ("trim", trim, 0),
        ("trimEnd", trim_end, 0),
        ("trimStart", trim_start, 0),
        ("valueOf", value_of, 0),
    ];
    for (name, code, len) in methods {
        rt.define_native_method(proto, name, *code, *len);
    }
    // %StringIteratorPrototype% (tag "String Iterator") and String.prototype[Symbol.iterator] (ECMA-262 22.1.3.36).
    make_iterator_prototype(rt, "StringIteratorPrototype", "String Iterator");
    let iter_fn = rt.new_native_function(string_iterator, "[Symbol.iterator]", 0, Vec::new());
    let iter_key = PropKey::Sym(rt.well_known(WellKnown::Iterator));
    rt.define_builtin(proto, iter_key, Prop::Data { value: Value::Object(iter_fn), flags: Flags::HIDDEN });
    rt.install_global("String", Value::Object(ctor));
}

// ----- helpers -----

/// RequireObjectCoercible(this), then ToString: the first two steps of most String.prototype methods.
fn this_str(rt: &Rt, c: &Call<'_>, name: &str) -> Res<JsStr> {
    if c.this.is_nullish() {
        return Err(rt.throw_type_error(&format!("String.prototype.{name} called on null or undefined")));
    }
    rt.to_string(c.this)
}

/// thisStringValue (ECMA-262 22.1.3.36): a String primitive or a String wrapper, without any conversion.
fn this_string_value(rt: &Rt, c: &Call<'_>, name: &str) -> Res<JsStr> {
    let found = match c.this {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => rt.with_obj(*o, |ob| match &ob.kind {
            ObjectKind::Wrapper(Value::String(s)) => Some(s.clone()),
            _ => None,
        }),
        _ => None,
    };
    found.ok_or_else(|| rt.throw_type_error(&format!("String.prototype.{name} requires that 'this' be a String")))
}

fn too_long(rt: &Rt) -> Throw {
    rt.throw_range_error("Invalid string length")
}

fn check_len(rt: &Rt, len: usize) -> Res<()> {
    if len > MAX_LEN { Err(too_long(rt)) } else { Ok(()) }
}

/// A relative position as `slice` reads it: a negative value counts back from `len`. Clamped into [0, len].
fn relative(v: f64, len: usize) -> usize {
    let n = len as f64;
    if v < 0.0 { (n + v).max(0.0) as usize } else { v.min(n) as usize }
}

/// An absolute position clamped into [0, len].
fn clamp(v: f64, len: usize) -> usize {
    v.clamp(0.0, len as f64) as usize
}

fn unit_at(s: &JsStr, i: usize) -> JsStr {
    JsStr::from_units(&s.units()[i..i + 1])
}

fn array_of(rt: &Rt, parts: Vec<JsStr>) -> Res<Value> {
    if parts.len() > MAX_DENSE_LEN {
        return Err(rt.throw_range_error("Invalid array length"));
    }
    Ok(Value::Object(rt.new_array(parts.into_iter().map(Value::String).collect())))
}

/// Start positions of the non-overlapping matches of `pattern`, as replaceAll finds them. An empty pattern matches
/// at every code unit boundary.
fn match_positions(s: &JsStr, pattern: &JsStr) -> Vec<usize> {
    let advance = pattern.len().max(1);
    let mut out = Vec::new();
    let mut p = 0;
    while let Some(i) = s.index_of(pattern, p) {
        out.push(i);
        p = i + advance;
        if p > s.len() {
            break;
        }
    }
    out
}

/// GetPrototypeFromConstructor (ECMA-262 10.1.14): `new_target.prototype` when it is an object, else the intrinsic.
fn proto_from(rt: &Rt, new_target: &Value, fallback: Intrinsic) -> Res<ObjRef> {
    match rt.get_str(new_target, "prototype")? {
        Value::Object(p) => Ok(p),
        _ => Ok(rt.intrinsic(fallback)),
    }
}

// ----- iterator -----

/// The state of a String iterator: the string and the code unit where the next code point starts.
struct StringIter {
    s: JsStr,
    pos: Cell<usize>,
}

impl NativeObject for StringIter {
    fn trace(&self, _t: &mut Tracer) {}
}

impl NativeIter for StringIter {
    fn next(&self, _rt: &Rt) -> Res<Option<Value>> {
        let i = self.pos.get();
        let Some((_, n)) = self.s.code_point_at(i) else {
            return Ok(None);
        };
        self.pos.set(i + n);
        Ok(Some(Value::String(JsStr::from_units(&self.s.units()[i..i + n]))))
    }
}

/// String.prototype[Symbol.iterator]: iterates by code point. A lone surrogate is its own item.
fn string_iterator(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    if c.this.is_nullish() {
        return Err(rt.throw_type_error("String.prototype[Symbol.iterator] called on null or undefined"));
    }
    let s = rt.to_string(c.this)?;
    let state = StringIter { s, pos: Cell::new(0) };
    Ok(Value::Object(new_iterator_object(rt, "StringIteratorPrototype", Rc::new(state))))
}

// ----- constructor and statics -----

/// String(value) (ECMA-262 22.1.1.1). A symbol gives its descriptive string instead of a TypeError.
fn string_call(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    if c.args.is_empty() {
        return Ok(Value::String(JsStr::empty()));
    }
    match c.arg(0) {
        Value::Symbol(sym) => Ok(Value::String(symbol_descriptive(rt, sym))),
        v => Ok(Value::String(rt.to_string(&v)?)),
    }
}

/// new String(value): a wrapper object. `new String(sym)` is a TypeError from ToString.
fn string_construct(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = if c.args.is_empty() { JsStr::empty() } else { rt.to_string(&c.args[0])? };
    let proto = proto_from(rt, c.new_target, Intrinsic::StringPrototype)?;
    Ok(Value::Object(rt.alloc(Obj::new(Some(proto), ObjectKind::Wrapper(Value::String(s))))))
}

/// SymbolDescriptiveString (ECMA-262 20.4.3.3.1): "Symbol(" + description + ")".
fn symbol_descriptive(rt: &Rt, sym: Sym) -> JsStr {
    let mut out: Vec<u16> = "Symbol(".encode_utf16().collect();
    if let Some(d) = rt.symbol_description(sym) {
        out.extend_from_slice(d.units());
    }
    out.push(u16::from(b')'));
    JsStr::from_vec(out)
}

/// String.fromCharCode: every argument is ToNumber'd in order, then reduced to a code unit.
fn from_char_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let mut units = Vec::with_capacity(c.args.len());
    for a in c.args {
        units.push(number::to_uint16(rt.to_number(a)?));
    }
    Ok(Value::String(JsStr::from_vec(units)))
}

/// String.fromCodePoint: each argument is converted and range-checked before the next one is converted (V8 order).
fn from_code_point(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let mut units = Vec::new();
    for a in c.args {
        let n = rt.to_number(a)?;
        let cp = if number::is_integer_valued(n) && (0.0..=1_114_111.0).contains(&n) {
            JsStr::from_code_point(n as u32)
        } else {
            None
        };
        let Some(cp) = cp else {
            return Err(rt.throw_range_error(&format!("Invalid code point {}", number::number_to_string(n))));
        };
        units.extend_from_slice(cp.units());
    }
    Ok(Value::String(JsStr::from_vec(units)))
}

// ----- character access -----

fn at(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "at")?;
    let rel = rt.to_integer_or_infinity(&c.arg(0))?;
    let len = s.len() as f64;
    let k = if rel >= 0.0 { rel } else { len + rel };
    if k < 0.0 || k >= len {
        return Ok(Value::Undefined);
    }
    Ok(Value::String(unit_at(&s, k as usize)))
}

fn char_at(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "charAt")?;
    let p = rt.to_integer_or_infinity(&c.arg(0))?;
    if p < 0.0 || p >= s.len() as f64 {
        return Ok(Value::String(JsStr::empty()));
    }
    Ok(Value::String(unit_at(&s, p as usize)))
}

fn char_code_at(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "charCodeAt")?;
    let p = rt.to_integer_or_infinity(&c.arg(0))?;
    let unit = if p < 0.0 { None } else { s.code_unit_at(p as usize) };
    Ok(Value::Number(unit.map_or(f64::NAN, f64::from)))
}

fn code_point_at(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "codePointAt")?;
    let p = rt.to_integer_or_infinity(&c.arg(0))?;
    if p < 0.0 {
        return Ok(Value::Undefined);
    }
    Ok(s.code_point_at(p as usize).map_or(Value::Undefined, |(cp, _)| Value::Number(f64::from(cp))))
}

// ----- searching and comparing -----

fn concat(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "concat")?;
    let mut out = s.units().to_vec();
    for a in c.args {
        let t = rt.to_string(a)?;
        out.extend_from_slice(t.units());
        check_len(rt, out.len())?;
    }
    Ok(Value::String(JsStr::from_vec(out)))
}

fn starts_with(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "startsWith")?;
    let search = rt.to_string(&c.arg(0))?;
    let start = clamp(rt.to_integer_or_infinity(&c.arg(1))?, s.len());
    Ok(Value::Bool(s.starts_with(&search, start)))
}

fn ends_with(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "endsWith")?;
    let search = rt.to_string(&c.arg(0))?;
    let end = if c.arg(1).is_undefined() { s.len() } else { clamp(rt.to_integer_or_infinity(&c.arg(1))?, s.len()) };
    Ok(Value::Bool(s.ends_with(&search, end)))
}

fn includes(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "includes")?;
    let search = rt.to_string(&c.arg(0))?;
    let start = clamp(rt.to_integer_or_infinity(&c.arg(1))?, s.len());
    Ok(Value::Bool(s.includes(&search, start)))
}

fn index_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "indexOf")?;
    let search = rt.to_string(&c.arg(0))?;
    let start = clamp(rt.to_integer_or_infinity(&c.arg(1))?, s.len());
    Ok(Value::Number(s.index_of(&search, start).map_or(-1.0, |i| i as f64)))
}

/// lastIndexOf reads its position with ToNumber, so an undefined or NaN position searches from the end.
fn last_index_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "lastIndexOf")?;
    let search = rt.to_string(&c.arg(0))?;
    let n = rt.to_number(&c.arg(1))?;
    let pos = if n.is_nan() { f64::INFINITY } else { number::to_integer_or_infinity(n) };
    let start = clamp(pos, s.len());
    Ok(Value::Number(s.last_index_of(&search, start).map_or(-1.0, |i| i as f64)))
}

// ----- padding, repetition and slicing -----

fn pad_start(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    pad(rt, c, "padStart", true)
}

fn pad_end(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    pad(rt, c, "padEnd", false)
}

/// padStart and padEnd. The fill string is converted only when padding is needed, and an empty fill returns the
/// string before the length limit is checked (V8 order).
fn pad(rt: &Rt, c: &Call<'_>, name: &str, at_start: bool) -> Res<Value> {
    let s = this_str(rt, c, name)?;
    let max = rt.to_length(&c.arg(0))?;
    if max <= s.len() as f64 {
        return Ok(Value::String(s));
    }
    let fill = if c.arg(1).is_undefined() { JsStr::from_str(" ") } else { rt.to_string(&c.arg(1))? };
    if fill.is_empty() {
        return Ok(Value::String(s));
    }
    if max > MAX_LEN as f64 {
        return Err(too_long(rt));
    }
    let n = max as usize;
    let padded = if at_start { s.pad_start(n, &fill) } else { s.pad_end(n, &fill) };
    padded.map(Value::String).map_err(|_| too_long(rt))
}

/// repeat: the count is checked before the empty-string shortcut, so ''.repeat(Infinity) throws (V8 order).
fn repeat(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "repeat")?;
    let n = rt.to_integer_or_infinity(&c.arg(0))?;
    if n < 0.0 || n == f64::INFINITY {
        return Err(rt.throw_range_error(&format!("Invalid count value: {}", number::number_to_string(n))));
    }
    if n == 0.0 || s.is_empty() {
        return Ok(Value::String(JsStr::empty()));
    }
    if s.len() as f64 * n > MAX_LEN as f64 {
        return Err(too_long(rt));
    }
    s.repeat(n as usize).map(Value::String).map_err(|_| too_long(rt))
}

fn slice(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "slice")?;
    let len = s.len();
    let from = relative(rt.to_integer_or_infinity(&c.arg(0))?, len);
    let to = if c.arg(1).is_undefined() { len } else { relative(rt.to_integer_or_infinity(&c.arg(1))?, len) };
    if from >= to {
        return Ok(Value::String(JsStr::empty()));
    }
    Ok(Value::String(s.substring(from, to)))
}

fn substring(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "substring")?;
    let len = s.len();
    let start = clamp(rt.to_integer_or_infinity(&c.arg(0))?, len);
    let end = if c.arg(1).is_undefined() { len } else { clamp(rt.to_integer_or_infinity(&c.arg(1))?, len) };
    Ok(Value::String(s.substring(start, end)))
}

/// substr (ECMA-262 B.2.2.1): the start is relative as in slice, and the length is clamped to the string.
fn substr(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "substr")?;
    let size = s.len();
    let start = relative(rt.to_integer_or_infinity(&c.arg(0))?, size);
    let length = if c.arg(1).is_undefined() { size as f64 } else { rt.to_integer_or_infinity(&c.arg(1))? };
    let length = clamp(length, size);
    let end = (start + length).min(size);
    Ok(Value::String(s.substring(start, end)))
}

/// split with a string separator. The limit is converted before the separator, and the separator is converted even
/// when the limit is 0 (V8 order). An undefined separator gives the whole string.
fn split(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "split")?;
    let lim = if c.arg(1).is_undefined() { u32::MAX } else { rt.to_uint32(&c.arg(1))? };
    let sep = c.arg(0);
    let pattern = rt.to_string(&sep)?;
    if lim == 0 {
        return array_of(rt, Vec::new());
    }
    if sep.is_undefined() {
        return array_of(rt, vec![s]);
    }
    array_of(rt, s.split_by_string(&pattern, lim))
}

// ----- replacing and case -----

/// The replaceValue argument: a callback, or the string produced by ToString.
enum Rep {
    Fn(Value),
    Str(JsStr),
}

fn replacement(rt: &Rt, v: Value) -> Res<Rep> {
    if rt.is_callable(&v) { Ok(Rep::Fn(v)) } else { Ok(Rep::Str(rt.to_string(&v)?)) }
}

const DOLLAR_SIGN: [u16; 1] = [0x24];

/// GetSubstitution (ECMA-262 22.1.3.19.1) for a string pattern, which has no captures or named groups: `$$`, `$&`,
/// `` $` `` and `$'` expand, and every other `$` stays literal. Checked against the length limit as it grows.
fn substitute(rt: &Rt, out: &mut Vec<u16>, matched: &JsStr, s: &JsStr, pos: usize, rep: &JsStr) -> Res<()> {
    let u = rep.units();
    let mut i = 0;
    while i < u.len() {
        let special = if u[i] == 0x24 { u.get(i + 1).copied() } else { None };
        let (piece, step) = match special {
            Some(0x24) => (&DOLLAR_SIGN[..], 2),
            Some(0x26) => (matched.units(), 2),
            Some(0x60) => (&s.units()[..pos], 2),
            Some(0x27) => (&s.units()[(pos + matched.len()).min(s.len())..], 2),
            _ => (&u[i..i + 1], 1),
        };
        out.extend_from_slice(piece);
        check_len(rt, out.len())?;
        i += step;
    }
    Ok(())
}

/// Appends the replacement for the match at `p` of `pattern` in `s`: GetSubstitution for a string, or the callback
/// (match, position, string) with its result converted by ToString.
fn push_match(rt: &Rt, out: &mut Vec<u16>, s: &JsStr, pattern: &JsStr, p: usize, rep: &Rep) -> Res<()> {
    match rep {
        Rep::Str(r) => substitute(rt, out, pattern, s, p, r),
        Rep::Fn(f) => {
            let args = [Value::String(pattern.clone()), Value::Number(p as f64), Value::String(s.clone())];
            let r = rt.call(f, &Value::Undefined, &args)?;
            let t = rt.to_string(&r)?;
            out.extend_from_slice(t.units());
            check_len(rt, out.len())
        }
    }
}

/// replace with a string pattern: the first match only. The pattern is converted before the replacement (V8 order).
fn replace(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "replace")?;
    let pattern = rt.to_string(&c.arg(0))?;
    let rep = replacement(rt, c.arg(1))?;
    let Some(pos) = s.index_of(&pattern, 0) else {
        return Ok(Value::String(s));
    };
    let mut out = Vec::with_capacity(s.len());
    out.extend_from_slice(&s.units()[..pos]);
    push_match(rt, &mut out, &s, &pattern, pos, &rep)?;
    out.extend_from_slice(&s.units()[pos + pattern.len()..]);
    check_len(rt, out.len())?;
    Ok(Value::String(JsStr::from_vec(out)))
}

/// replaceAll with a string pattern: every non-overlapping match. A callback runs once per match, in order, and each
/// result is converted right after its call.
fn replace_all(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "replaceAll")?;
    let pattern = rt.to_string(&c.arg(0))?;
    let rep = replacement(rt, c.arg(1))?;
    let positions = match_positions(&s, &pattern);
    if positions.is_empty() {
        return Ok(Value::String(s));
    }
    let mut out = Vec::with_capacity(s.len());
    let mut end = 0;
    for p in positions {
        out.extend_from_slice(&s.units()[end..p]);
        push_match(rt, &mut out, &s, &pattern, p, &rep)?;
        end = p + pattern.len();
    }
    out.extend_from_slice(&s.units()[end..]);
    check_len(rt, out.len())?;
    Ok(Value::String(JsStr::from_vec(out)))
}

fn to_lower_case(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "toLowerCase")?;
    let r = s.to_lower_case();
    check_len(rt, r.len())?;
    Ok(Value::String(r))
}

fn to_upper_case(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_str(rt, c, "toUpperCase")?;
    let r = s.to_upper_case();
    check_len(rt, r.len())?;
    Ok(Value::String(r))
}

// ----- trimming, well-formedness and the value methods -----

fn trim(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::String(this_str(rt, c, "trim")?.trim()))
}

fn trim_start(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::String(this_str(rt, c, "trimStart")?.trim_start()))
}

fn trim_end(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::String(this_str(rt, c, "trimEnd")?.trim_end()))
}

fn is_well_formed(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(this_str(rt, c, "isWellFormed")?.is_well_formed()))
}

fn to_well_formed(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::String(this_str(rt, c, "toWellFormed")?.to_well_formed()))
}

fn to_string_method(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::String(this_string_value(rt, c, "toString")?))
}

fn value_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::String(this_string_value(rt, c, "valueOf")?))
}
