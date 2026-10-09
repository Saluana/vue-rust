//! Decoder: the inverse of the encoder (trace-format.md v1.1). It builds runtime values from one encoding pass.
//! It restores aliasing and cycles from `id`/`ref`, the holes of arrays, each object's prototype class, the
//! `integrity` state (applied after the properties), symbols by id, and functions by name and length. Accessor
//! markers, unknown classes (Promise has no kernel representation yet) and malformed input are
//! errors, never guesses.

use std::collections::HashMap;

use vn_value::{Call, Flags, FnInfo, FnKind, Intrinsic, JsStr, ObjRef, Prop, PropKey, Res, Rt, Sym, Value, MAX_DENSE_LEN};

use crate::json::Json;

/// The ids of one encoding pass: objects and symbols by ordinal, and the functions that `fn` entries resolve to.
#[derive(Default)]
pub struct IdTable {
    objects: HashMap<u64, ObjRef>,
    symbols: HashMap<u64, (Sym, Option<JsStr>)>,
    functions: Vec<(JsStr, u32, Value)>,
}

impl IdTable {
    pub fn new() -> IdTable {
        IdTable::default()
    }

    /// The value that `{"$":"fn","name":name,"length":length}` decodes to. Without a registration the entry becomes
    /// a placeholder function with that name and length.
    pub fn register_function(&mut self, name: &str, length: u32, f: Value) {
        self.functions.push((JsStr::from_str(name), length, f));
    }
}

/// Decodes one trace value (the inverse of `encode`).
pub fn decode(rt: &Rt, e: &Json, ids: &mut IdTable) -> Res<Value> {
    match e {
        Json::Null => Ok(Value::Null),
        Json::Bool(b) => Ok(Value::Bool(*b)),
        Json::Num(text) => Ok(Value::Number(parse_number(rt, text)?)),
        Json::Str(units) => Ok(Value::String(JsStr::from_units(units))),
        Json::Arr(_) => Err(invalid(rt, "an array is not a trace value")),
        Json::Obj(_) => decode_tagged(rt, e, ids),
    }
}

/// Decodes one encoding pass (a trace-format list) with one shared id table, in list order.
pub fn decode_pass(rt: &Rt, list: &[Json], ids: &mut IdTable) -> Res<Vec<Value>> {
    list.iter().map(|e| decode(rt, e, ids)).collect()
}

fn invalid(rt: &Rt, message: &str) -> vn_value::Throw {
    rt.throw_type_error(&format!("cannot decode: {message}"))
}

fn tag(e: &Json) -> Option<String> {
    e.field("$").and_then(Json::units).map(|u| String::from_utf16_lossy(u))
}

fn text_field(rt: &Rt, e: &Json, name: &str) -> Res<String> {
    match e.field(name).and_then(Json::units) {
        Some(u) => Ok(String::from_utf16_lossy(u)),
        None => Err(invalid(rt, &format!("missing string field {name}"))),
    }
}

fn integer_field(rt: &Rt, e: &Json, name: &str) -> Res<u64> {
    e.field(name)
        .and_then(Json::num_text)
        .and_then(|t| t.parse::<u64>().ok())
        .ok_or_else(|| invalid(rt, &format!("missing integer field {name}")))
}

fn parse_number(rt: &Rt, text: &str) -> Res<f64> {
    text.parse::<f64>().map_err(|_| invalid(rt, &format!("bad number {text}")))
}

fn decode_tagged(rt: &Rt, e: &Json, ids: &mut IdTable) -> Res<Value> {
    match tag(e).as_deref() {
        Some("undef") => Ok(Value::Undefined),
        Some("num") => decode_number_tag(rt, e),
        Some("sym") => decode_symbol(rt, e, ids),
        Some("fn") => decode_function(rt, e, ids),
        Some("ref") => {
            let id = integer_field(rt, e, "id")?;
            match ids.objects.get(&id) {
                Some(o) => Ok(Value::Object(*o)),
                None => Err(invalid(rt, "a ref to an object that has not been expanded")),
            }
        }
        Some("obj") => decode_object(rt, e, ids),
        Some("accessor") => Err(invalid(rt, "accessor markers are not decodable")),
        Some("hole") => Err(invalid(rt, "a hole is only valid inside elems")),
        _ => Err(invalid(rt, "unsupported trace value")),
    }
}

fn decode_number_tag(rt: &Rt, e: &Json) -> Res<Value> {
    let v = match e.field("v").and_then(Json::units).map(String::from_utf16_lossy).as_deref() {
        Some("-0") => -0.0,
        Some("NaN") => f64::NAN,
        Some("Infinity") => f64::INFINITY,
        Some("-Infinity") => f64::NEG_INFINITY,
        _ => return Err(invalid(rt, "bad number tag")),
    };
    Ok(Value::Number(v))
}

fn decode_symbol(rt: &Rt, e: &Json, ids: &mut IdTable) -> Res<Value> {
    let id = integer_field(rt, e, "id")?;
    let desc = match e.field("desc") {
        Some(Json::Null) => None,
        Some(Json::Str(units)) => Some(JsStr::from_units(units)),
        _ => return Err(invalid(rt, "symbol desc must be a string or null")),
    };
    if let Some((sym, known)) = ids.symbols.get(&id) {
        if *known != desc {
            return Err(invalid(rt, "a symbol id is reused with another description"));
        }
        return Ok(Value::Symbol(*sym));
    }
    let sym = rt.new_symbol(desc.clone());
    ids.symbols.insert(id, (sym, desc));
    Ok(Value::Symbol(sym))
}

fn decode_function(rt: &Rt, e: &Json, ids: &mut IdTable) -> Res<Value> {
    let name = match e.field("name").and_then(Json::units) {
        Some(u) => JsStr::from_units(u),
        None => return Err(invalid(rt, "function name must be a string")),
    };
    let length = integer_field(rt, e, "length")?;
    let length = u32::try_from(length).map_err(|_| invalid(rt, "function length out of range"))?;
    if let Some((_, _, f)) = ids.functions.iter().find(|(n, l, _)| *n == name && *l == length) {
        return Ok(f.clone());
    }
    let f = rt.new_function(placeholder, FnInfo { name, length, kind: FnKind::Normal }, &[]);
    Ok(Value::Object(f))
}

fn placeholder(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

fn decode_object(rt: &Rt, e: &Json, ids: &mut IdTable) -> Res<Value> {
    let id = integer_field(rt, e, "id")?;
    if ids.objects.contains_key(&id) {
        return Err(invalid(rt, "an object id is expanded twice in one pass"));
    }
    let class = text_field(rt, e, "cls")?;
    let proto = text_field(rt, e, "proto")?;
    let o = match class.as_str() {
        "Object" => {
            let o = match proto.as_str() {
                "Object.prototype" => rt.new_object(),
                "null" => rt.new_object_with_proto(None),
                _ => return Err(invalid(rt, "unsupported prototype of a plain object")),
            };
            ids.objects.insert(id, o);
            o
        }
        "Array" => {
            if proto != "other" {
                return Err(invalid(rt, "unsupported prototype of an array"));
            }
            let len = integer_field(rt, e, "len")?;
            let len = usize::try_from(len).ok().filter(|n| *n <= MAX_DENSE_LEN).ok_or_else(|| invalid(rt, "array too long"))?;
            let o = rt.new_array(vec![Value::Hole; len]);
            ids.objects.insert(id, o);
            let elems = e.field("elems").and_then(Json::items).ok_or_else(|| invalid(rt, "array without elems"))?;
            if elems.len() != len {
                return Err(invalid(rt, "array elems do not match its length"));
            }
            for (i, el) in elems.iter().enumerate() {
                if tag(el).as_deref() == Some("hole") {
                    continue;
                }
                let value = decode(rt, el, ids)?;
                rt.define_own(o, &PropKey::Index(i as u32), Prop::Data { value, flags: Flags::ALL })?;
            }
            o
        }
        "Error" => {
            if proto != "other" {
                return Err(invalid(rt, "unsupported prototype of an error"));
            }
            // An own `name` property overrides the prototype's, so the encoded name no longer says which error
            // constructor made the object. Then the base Error constructor is used: the encoding is reproduced.
            let name = text_field(rt, e, "name")?;
            let ctor = error_constructor(rt, if lists_prop(e, "name") { "Error" } else { &name })?;
            let message = match e.field("message").and_then(Json::units) {
                Some(u) => JsStr::from_units(u),
                None => return Err(invalid(rt, "error without a message")),
            };
            let ctor_value = Value::Object(ctor);
            let made = rt.construct(&ctor_value, &[Value::String(message)], &ctor_value)?;
            let o = made.as_object().ok_or_else(|| invalid(rt, "error constructor gave no object"))?;
            ids.objects.insert(id, o);
            o
        }
        "Map" | "Set" => {
            if proto != "other" {
                return Err(invalid(rt, "unsupported prototype of a collection"));
            }
            let is_map = class == "Map";
            let o = if is_map { vn_value::builtins::map_set::new_map(rt) } else { vn_value::builtins::map_set::new_set(rt) };
            ids.objects.insert(id, o);
            if is_map {
                let entries = e.field("entries").and_then(Json::items).ok_or_else(|| invalid(rt, "map without entries"))?;
                for pair in entries {
                    let kv = pair.items().filter(|p| p.len() == 2).ok_or_else(|| invalid(rt, "a map entry is a pair"))?;
                    let k = decode(rt, &kv[0], ids)?;
                    let v = decode(rt, &kv[1], ids)?;
                    vn_value::builtins::map_set::map_put(rt, o, k, v);
                }
            } else {
                let values = e.field("values").and_then(Json::items).ok_or_else(|| invalid(rt, "set without values"))?;
                for item in values {
                    let v = decode(rt, item, ids)?;
                    vn_value::builtins::map_set::set_put(rt, o, v);
                }
            }
            o
        }
        "Promise" => return Err(invalid(rt, "the kernel has no Promise objects yet")),
        _ => return Err(invalid(rt, "unsupported class")),
    };
    for entry in e.field("props").and_then(Json::items).ok_or_else(|| invalid(rt, "object without props"))? {
        let pair = entry.items().filter(|p| p.len() == 2).ok_or_else(|| invalid(rt, "a prop is a [key, value] pair"))?;
        let key = pair[0].units().ok_or_else(|| invalid(rt, "a prop key is a string"))?;
        let value = decode(rt, &pair[1], ids)?;
        rt.define_own(o, &PropKey::from_jsstr(JsStr::from_units(key)), Prop::Data { value, flags: Flags::ALL })?;
    }
    match e.field("integrity").and_then(Json::units).map(String::from_utf16_lossy).as_deref() {
        None => {}
        Some("frozen") => {
            rt.freeze(o)?;
        }
        Some("sealed") => {
            rt.seal(o)?;
        }
        Some("nonextensible") => {
            rt.prevent_extensions(o);
        }
        Some(_) => return Err(invalid(rt, "unknown integrity state")),
    }
    Ok(Value::Object(o))
}

/// Whether the encoded object lists an own property called `name` among its `props`.
fn lists_prop(e: &Json, name: &str) -> bool {
    e.field("props").and_then(Json::items).is_some_and(|props| {
        props.iter().any(|entry| {
            entry.items().and_then(|kv| kv.first()).and_then(Json::units).is_some_and(|k| k.iter().copied().eq(name.encode_utf16()))
        })
    })
}

fn error_constructor(rt: &Rt, name: &str) -> Res<ObjRef> {
    let kind = match name {
        "Error" => Intrinsic::ErrorCtor,
        "TypeError" => Intrinsic::TypeErrorCtor,
        "RangeError" => Intrinsic::RangeErrorCtor,
        "ReferenceError" => Intrinsic::ReferenceErrorCtor,
        "SyntaxError" => Intrinsic::SyntaxErrorCtor,
        "EvalError" => Intrinsic::EvalErrorCtor,
        "URIError" => Intrinsic::UriErrorCtor,
        _ => return Err(invalid(rt, "unknown error name")),
    };
    Ok(rt.intrinsic(kind))
}
