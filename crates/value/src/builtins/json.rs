//! Builtin family: JSON (WP-B6, ECMA-262 25.5). `JSON.parse` and `JSON.stringify` work on UTF-16 code units; V8 is the
//! oracle (`tests/language/builtins/json-*.json`, `crates/value/tests/builtins_b6_*.rs`).
//!
//! Limits and deviations. Each one fails loudly with a RangeError; none is silent:
//! - `JSON.parse` nests at most `MAX_NESTING` (1000) arrays and objects, the WP-B6 limit. V8 has no such limit.
//! - An array longer than `MAX_DENSE_LEN`, or a result longer than `MAX_LEN` code units, throws RangeError.
//! - `JSON.stringify` and the reviver walk keep an explicit stack on the heap, so they have no depth limit and no
//!   Rust recursion (V8 also accepts very deep values).
//! - Boxed Number and String arguments convert with ToNumber and ToString. Until WP-B3 gives Number.prototype and
//!   String.prototype their valueOf and toString, the kernel converts such wrappers through Object.prototype.toString
//!   (ABI.md section 13.4).

use std::collections::HashSet;
use std::rc::Rc;

use crate::function::Call;
use crate::jsstr::{JsStr, MAX_LEN};
use crate::number;
use crate::object::{Flags, ObjectKind, Prop, MAX_DENSE_LEN};
use crate::rt::{Rt, WellKnown};
use crate::value::{ObjRef, PropKey, Res, Throw, Value};

/// Deepest nesting `JSON.parse` accepts, counting arrays and objects together. A deeper text throws RangeError.
pub const MAX_NESTING: usize = 1000;

pub fn install(rt: &Rt) {
    let json = rt.new_object();
    rt.define_native_method(json, "parse", json_parse, 2);
    rt.define_native_method(json, "stringify", json_stringify, 3);
    // ECMA-262 25.5.3: the tag is not writable, not enumerable and configurable.
    let tag = PropKey::Sym(rt.well_known(WellKnown::ToStringTag));
    rt.define_builtin(json, tag, Prop::Data { value: Value::str("JSON"), flags: Flags::CONFIG });
    rt.install_global("JSON", Value::Object(json));
}

fn empty_key() -> PropKey {
    PropKey::Str(JsStr::empty())
}

fn length_key() -> PropKey {
    PropKey::from_str("length")
}

/// The string a property key stands for. Callers pass string keys and array indexes only.
fn key_string(k: &PropKey) -> JsStr {
    match k {
        PropKey::Index(i) => JsStr::from_str(&i.to_string()),
        PropKey::Str(s) => s.clone(),
        PropKey::Sym(_) => JsStr::empty(),
    }
}

/// The primitive held by a Boolean, Number, String or Symbol wrapper.
fn wrapper_value(rt: &Rt, o: ObjRef) -> Option<Value> {
    rt.with_obj(o, |ob| match &ob.kind {
        ObjectKind::Wrapper(v) => Some(v.clone()),
        _ => None,
    })
}

/// EnumerableOwnProperties(o, key) (ECMA-262 7.3.23): the enumerable string keys in own-key order.
fn enumerable_keys(rt: &Rt, o: ObjRef) -> Res<Vec<PropKey>> {
    let mut out = Vec::new();
    for k in rt.own_keys(o)? {
        if matches!(k, PropKey::Sym(_)) {
            continue;
        }
        if rt.get_own_property(o, &k)?.is_some_and(|p| p.flags().enumerable) {
            out.push(k);
        }
    }
    Ok(out)
}

// ----------------------------------------------------------------------------------------------------------------
// JSON.parse (ECMA-262 25.5.1)

fn json_parse(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let text = rt.to_string(&c.arg(0))?;
    let mut p = Parser { rt, src: text.units(), pos: 0 };
    let value = p.text()?;
    let reviver = c.arg(1);
    if !rt.is_callable(&reviver) {
        return Ok(value);
    }
    // Step 7: the walk starts from a wrapper that holds the parsed value under the empty key.
    let root = rt.new_object();
    rt.create_data_property(root, empty_key(), value)?;
    walk(rt, root, &reviver)
}

/// A recursive-descent parser over UTF-16 code units. The grammar is strict JSON: no comments, no trailing commas,
/// no leading zeros, no unescaped control characters. Raw lone surrogates are kept, as V8 keeps them.
struct Parser<'a> {
    rt: &'a Rt,
    src: &'a [u16],
    pos: usize,
}

fn is_digit(u: Option<&u16>) -> bool {
    matches!(u, Some(&(0x30..=0x39)))
}

fn hex_value(u: u16) -> Option<u16> {
    match u {
        0x30..=0x39 => Some(u - 0x30),
        0x41..=0x46 => Some(u - 0x41 + 10),
        0x61..=0x66 => Some(u - 0x61 + 10),
        _ => None,
    }
}

impl Parser<'_> {
    fn text(&mut self) -> Res<Value> {
        let v = self.value(0)?;
        self.ws();
        if self.pos < self.src.len() {
            return Err(self.syntax());
        }
        Ok(v)
    }

    fn ws(&mut self) {
        while matches!(self.src.get(self.pos), Some(&(0x20 | 0x09 | 0x0A | 0x0D))) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, u: u16) -> bool {
        if self.src.get(self.pos) == Some(&u) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn syntax(&self) -> Throw {
        match self.src.get(self.pos) {
            None => self.rt.throw_syntax_error("Unexpected end of JSON input"),
            Some(&u) => self.rt.throw_syntax_error(&format!(
                "Unexpected token '{}' in JSON at position {}",
                String::from_utf16_lossy(&[u]),
                self.pos
            )),
        }
    }

    fn too_deep(&self) -> Throw {
        self.rt.throw_range_error(&format!("JSON nesting exceeds {MAX_NESTING} levels"))
    }

    /// One JSON value. `depth` is the number of arrays and objects that contain it.
    fn value(&mut self, depth: usize) -> Res<Value> {
        self.ws();
        match self.src.get(self.pos).copied() {
            Some(0x7B) => self.object(depth + 1),
            Some(0x5B) => self.array(depth + 1),
            Some(0x22) => Ok(Value::String(self.string()?)),
            Some(0x74) => self.literal("true", Value::Bool(true)),
            Some(0x66) => self.literal("false", Value::Bool(false)),
            Some(0x6E) => self.literal("null", Value::Null),
            Some(0x2D | 0x30..=0x39) => Ok(Value::Number(self.number()?)),
            _ => Err(self.syntax()),
        }
    }

    fn literal(&mut self, word: &str, v: Value) -> Res<Value> {
        for b in word.bytes() {
            if !self.eat(u16::from(b)) {
                return Err(self.syntax());
            }
        }
        Ok(v)
    }

    fn array(&mut self, nest: usize) -> Res<Value> {
        if nest > MAX_NESTING {
            return Err(self.too_deep());
        }
        self.pos += 1;
        let mut elems = Vec::new();
        self.ws();
        if !self.eat(0x5D) {
            loop {
                let v = self.value(nest)?;
                if elems.len() == MAX_DENSE_LEN {
                    return Err(self.rt.throw_range_error("Invalid array length"));
                }
                elems.push(v);
                self.ws();
                if self.eat(0x5D) {
                    break;
                }
                if !self.eat(0x2C) {
                    return Err(self.syntax());
                }
            }
        }
        Ok(Value::Object(self.rt.new_array(elems)))
    }

    fn object(&mut self, nest: usize) -> Res<Value> {
        if nest > MAX_NESTING {
            return Err(self.too_deep());
        }
        self.pos += 1;
        let obj = self.rt.new_object();
        self.ws();
        if self.eat(0x7D) {
            return Ok(Value::Object(obj));
        }
        loop {
            self.ws();
            if self.src.get(self.pos) != Some(&0x22) {
                return Err(self.syntax());
            }
            let key = self.string()?;
            self.ws();
            if !self.eat(0x3A) {
                return Err(self.syntax());
            }
            let v = self.value(nest)?;
            // CreateDataProperty: a repeated key replaces the value in its first position, and "__proto__" is an own key.
            self.rt.create_data_property(obj, PropKey::from_jsstr(key), v)?;
            self.ws();
            if self.eat(0x7D) {
                return Ok(Value::Object(obj));
            }
            if !self.eat(0x2C) {
                return Err(self.syntax());
            }
        }
    }

    /// A string literal; the current unit is the opening quote.
    fn string(&mut self) -> Res<JsStr> {
        self.pos += 1;
        let mut buf = Vec::new();
        loop {
            let Some(u) = self.src.get(self.pos).copied() else {
                return Err(self.rt.throw_syntax_error("Unterminated string in JSON"));
            };
            self.pos += 1;
            match u {
                0x22 => return Ok(JsStr::from_vec(buf)),
                0x5C => buf.push(self.escape()?),
                u if u < 0x20 => {
                    return Err(self.rt.throw_syntax_error(&format!(
                        "Bad control character in string literal in JSON at position {}",
                        self.pos - 1
                    )));
                }
                u => buf.push(u),
            }
        }
    }

    /// The code unit an escape sequence stands for; the backslash is already consumed.
    fn escape(&mut self) -> Res<u16> {
        let Some(e) = self.src.get(self.pos).copied() else {
            return Err(self.syntax());
        };
        self.pos += 1;
        Ok(match e {
            0x22 | 0x5C | 0x2F => e,
            0x62 => 0x08,
            0x66 => 0x0C,
            0x6E => 0x0A,
            0x72 => 0x0D,
            0x74 => 0x09,
            0x75 => {
                let mut v: u32 = 0;
                for _ in 0..4 {
                    let Some(d) = self.src.get(self.pos).copied().and_then(hex_value) else {
                        return Err(self.rt.throw_syntax_error(&format!(
                            "Bad Unicode escape in JSON at position {}",
                            self.pos
                        )));
                    };
                    v = v * 16 + u32::from(d);
                    self.pos += 1;
                }
                v as u16
            }
            _ => return Err(self.rt.throw_syntax_error(&format!("Bad escaped character in JSON at position {}", self.pos - 1))),
        })
    }

    fn digits(&mut self) {
        while is_digit(self.src.get(self.pos)) {
            self.pos += 1;
        }
    }

    /// `-? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE] [+-]? [0-9]+)?`, converted with the correctly rounded parser.
    fn number(&mut self) -> Res<f64> {
        let start = self.pos;
        self.eat(0x2D);
        if !self.eat(0x30) {
            if !is_digit(self.src.get(self.pos)) {
                return Err(self.syntax());
            }
            self.digits();
        }
        if self.eat(0x2E) {
            if !is_digit(self.src.get(self.pos)) {
                return Err(self.syntax());
            }
            self.digits();
        }
        if self.eat(0x65) || self.eat(0x45) {
            if !self.eat(0x2B) {
                self.eat(0x2D);
            }
            if !is_digit(self.src.get(self.pos)) {
                return Err(self.syntax());
            }
            self.digits();
        }
        // Only ASCII units were consumed, so the conversion is exact.
        let text: String = self.src[start..self.pos].iter().map(|&u| char::from(u as u8)).collect();
        text.parse::<f64>().map_err(|_| self.syntax())
    }
}

/// One object or array of the reviver walk: the holder and name it was read from, and the members still to visit.
/// Arrays visit the indexes `0..len`; objects visit `keys`.
struct Node {
    holder: ObjRef,
    name: PropKey,
    val: ObjRef,
    keys: Vec<PropKey>,
    array_len: Option<usize>,
    next: usize,
}

impl Node {
    fn count(&self) -> usize {
        self.array_len.unwrap_or(self.keys.len())
    }

    fn key(&self, i: usize) -> PropKey {
        match self.array_len {
            Some(_) => PropKey::Index(i as u32),
            None => self.keys[i].clone(),
        }
    }
}

/// InternalizeJSONProperty (ECMA-262 25.5.1.1) with an explicit stack. Each object's members are revived before the
/// object itself; an undefined result deletes the member, any other result is defined on its holder.
fn walk(rt: &Rt, root: ObjRef, reviver: &Value) -> Res<Value> {
    let mut stack: Vec<Node> = Vec::new();
    if let Some(v) = open(rt, root, empty_key(), reviver, &mut stack)? {
        return Ok(v);
    }
    loop {
        let Some(top) = stack.last_mut() else {
            return Err(rt.throw_type_error("internal error: empty reviver stack"));
        };
        if top.next < top.count() {
            let key = top.key(top.next);
            top.next += 1;
            let holder = top.val;
            if let Some(v) = open(rt, holder, key.clone(), reviver, &mut stack)? {
                store(rt, holder, key, v)?;
            }
            continue;
        }
        // Every member of the top node is revived: revive the node itself.
        let Some(node) = stack.pop() else {
            return Err(rt.throw_type_error("internal error: empty reviver stack"));
        };
        let args = [key_value(&node.name), Value::Object(node.val)];
        let v = rt.call(reviver, &Value::Object(node.holder), &args)?;
        match stack.last() {
            Some(parent) => store(rt, parent.val, node.name, v)?,
            None => return Ok(v),
        }
    }
}

/// The first half of InternalizeJSONProperty: reads `holder[name]`. A primitive is revived at once and returned. An
/// object gets a node on the stack, and None is returned.
fn open(rt: &Rt, holder: ObjRef, name: PropKey, reviver: &Value, stack: &mut Vec<Node>) -> Res<Option<Value>> {
    let val = rt.get(holder, &name, &Value::Object(holder))?;
    let o = match &val {
        Value::Object(o) => *o,
        _ => {
            let args = [key_value(&name), val];
            return Ok(Some(rt.call(reviver, &Value::Object(holder), &args)?));
        }
    };
    let (keys, array_len) = if rt.is_array(&val) {
        let len = rt.get(o, &length_key(), &val)?;
        (Vec::new(), Some(rt.to_length(&len)? as usize))
    } else {
        (enumerable_keys(rt, o)?, None)
    };
    stack.push(Node { holder, name, val: o, keys, array_len, next: 0 });
    Ok(None)
}

/// CreateDataProperty or Delete on `holder` for a revived member. A refused result is ignored, as in the spec.
fn store(rt: &Rt, holder: ObjRef, key: PropKey, v: Value) -> Res<()> {
    if v.is_undefined() {
        rt.delete(holder, &key)?;
    } else {
        rt.create_data_property(holder, key, v)?;
    }
    Ok(())
}

fn key_value(k: &PropKey) -> Value {
    Value::String(key_string(k))
}

// ----------------------------------------------------------------------------------------------------------------
// JSON.stringify (ECMA-262 25.5.2)

fn json_stringify(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let mut ser = Ser {
        rt,
        replacer: None,
        list: None,
        gap: Vec::new(),
        indent: Vec::new(),
        out: Vec::new(),
        frames: Vec::new(),
        ancestors: HashSet::new(),
        to_json: PropKey::from_str("toJSON"),
    };
    // Steps 2-4: a callable replacer filters every member; an array replacer is the property list.
    let replacer = c.arg(1);
    if let Value::Object(r) = replacer {
        if rt.is_callable(&replacer) {
            ser.replacer = Some(replacer);
        } else if rt.is_array(&replacer) {
            ser.list = Some(property_list(rt, r)?);
        }
    }
    ser.gap = gap_of(rt, &c.arg(2))?;
    // Step 9: the value sits under the empty key of a wrapper, which the replacer sees as `this`.
    let wrapper = rt.new_object();
    rt.create_data_property(wrapper, empty_key(), c.arg(0))?;
    let root = ser.prepare(wrapper, &empty_key())?;
    if !ser.serializable(&root) {
        return Ok(Value::Undefined);
    }
    ser.emit(root)?;
    ser.run()?;
    Ok(Value::String(JsStr::from_vec(ser.out)))
}

/// The property list of an array replacer (step 4.b): string and number items, and boxed String and Number values,
/// as strings, with duplicates dropped.
fn property_list(rt: &Rt, r: ObjRef) -> Res<Rc<[PropKey]>> {
    let rv = Value::Object(r);
    let len = rt.get(r, &length_key(), &rv)?;
    let len = rt.to_length(&len)? as usize;
    let mut seen = HashSet::new();
    let mut list = Vec::new();
    for i in 0..len {
        let v = rt.get(r, &PropKey::Index(i as u32), &rv)?;
        let item = match &v {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(JsStr::from_str(&number::number_to_string(*n))),
            Value::Object(o) => match wrapper_value(rt, *o) {
                Some(Value::String(_) | Value::Number(_)) => Some(rt.to_string(&v)?),
                _ => None,
            },
            _ => None,
        };
        if let Some(s) = item {
            if seen.insert(s.clone()) {
                list.push(PropKey::from_jsstr(s));
            }
        }
    }
    Ok(Rc::from(list))
}

/// The gap string from `space` (steps 5-8): a number gives that many spaces (at most 10), a string its first 10 units.
fn gap_of(rt: &Rt, space: &Value) -> Res<Vec<u16>> {
    let mut space = space.clone();
    if let Value::Object(o) = space {
        match wrapper_value(rt, o) {
            Some(Value::Number(_)) => space = Value::Number(rt.to_number(&space)?),
            Some(Value::String(_)) => space = Value::String(rt.to_string(&space)?),
            _ => {}
        }
    }
    Ok(match space {
        Value::Number(n) => {
            let k = number::to_integer_or_infinity(n).min(10.0);
            if k < 1.0 { Vec::new() } else { vec![0x20; k as usize] }
        }
        Value::String(s) => s.units()[..s.len().min(10)].to_vec(),
        _ => Vec::new(),
    })
}

/// An open object or array of the serializer.
enum Frame {
    /// `keys` are the members to write (the property list, or the enumerable own keys taken at the start). `count` is
    /// the number written, `stepback` the indentation length before this container.
    Obj { o: ObjRef, keys: Rc<[PropKey]>, next: usize, count: usize, stepback: usize },
    Arr { o: ObjRef, len: usize, next: usize, stepback: usize },
}

/// What `run` does next: write the next member, or close the innermost container.
enum Step {
    Close,
    Member { holder: ObjRef, key: PropKey, array: bool, first: bool },
}

/// Serializer state (ECMA-262 25.5.2.1). Output goes to one UTF-16 buffer; `frames` holds the open containers, and
/// `ancestors` holds the same objects for the cycle check.
struct Ser<'a> {
    rt: &'a Rt,
    replacer: Option<Value>,
    list: Option<Rc<[PropKey]>>,
    gap: Vec<u16>,
    indent: Vec<u16>,
    out: Vec<u16>,
    frames: Vec<Frame>,
    ancestors: HashSet<ObjRef>,
    to_json: PropKey,
}

impl Ser<'_> {
    fn put(&mut self, u: u16) -> Res<()> {
        if self.out.len() >= MAX_LEN {
            return Err(self.rt.throw_range_error("Invalid string length"));
        }
        self.out.push(u);
        Ok(())
    }

    fn ascii(&mut self, s: &str) -> Res<()> {
        for b in s.bytes() {
            self.put(u16::from(b))?;
        }
        Ok(())
    }

    /// A line break and `depth` units of the current indentation. Nothing is written without a gap.
    fn newline(&mut self, depth: usize) -> Res<()> {
        if self.gap.is_empty() {
            return Ok(());
        }
        self.put(0x0A)?;
        for i in 0..depth {
            let u = self.indent[i];
            self.put(u)?;
        }
        Ok(())
    }

    /// QuoteJSONString (ECMA-262 25.5.2.4). Other controls become `\u00xx`, and lone surrogates `\udxxx`, in lowercase.
    fn quote(&mut self, s: &[u16]) -> Res<()> {
        self.put(0x22)?;
        let mut i = 0;
        while i < s.len() {
            let u = s[i];
            match u {
                0x08 => self.ascii("\\b")?,
                0x09 => self.ascii("\\t")?,
                0x0A => self.ascii("\\n")?,
                0x0C => self.ascii("\\f")?,
                0x0D => self.ascii("\\r")?,
                0x22 => self.ascii("\\\"")?,
                0x5C => self.ascii("\\\\")?,
                0xD800..=0xDBFF if s.get(i + 1).is_some_and(|&t| (0xDC00..=0xDFFF).contains(&t)) => {
                    self.put(u)?;
                    self.put(s[i + 1])?;
                    i += 1;
                }
                0x00..=0x1F | 0xD800..=0xDFFF => self.ascii(&format!("\\u{u:04x}"))?,
                _ => self.put(u)?,
            }
            i += 1;
        }
        self.put(0x22)
    }

    fn quote_key(&mut self, k: &PropKey) -> Res<()> {
        let s = key_string(k);
        self.quote(s.units())
    }

    /// SerializeJSONProperty steps 1-4 for `holder[key]`: the value after toJSON, the replacer function and the
    /// unwrapping of boxed primitives. Nothing is written here.
    fn prepare(&self, holder: ObjRef, key: &PropKey) -> Res<Value> {
        let rt = self.rt;
        let mut value = rt.get(holder, key, &Value::Object(holder))?;
        if let Value::Object(o) = value {
            let to_json = rt.get(o, &self.to_json, &value)?;
            if rt.is_callable(&to_json) {
                value = rt.call(&to_json, &value, &[Value::String(key_string(key))])?;
            }
        }
        if let Some(f) = &self.replacer {
            let args = [Value::String(key_string(key)), value];
            value = rt.call(f, &Value::Object(holder), &args)?;
        }
        if let Value::Object(o) = value {
            match wrapper_value(rt, o) {
                Some(Value::Number(_)) => value = Value::Number(rt.to_number(&value)?),
                Some(Value::String(_)) => value = Value::String(rt.to_string(&value)?),
                Some(Value::Bool(b)) => value = Value::Bool(b),
                _ => {}
            }
        }
        Ok(value)
    }

    /// False for the values that are left out of an object and written as `null` in an array: undefined, symbols
    /// and callables.
    fn serializable(&self, v: &Value) -> bool {
        match v {
            Value::Null | Value::Bool(_) | Value::String(_) | Value::Number(_) => true,
            Value::Object(_) => !self.rt.is_callable(v),
            Value::Undefined | Value::Symbol(_) | Value::Uninit | Value::Hole => false,
        }
    }

    /// Writes a serializable value: a primitive, or the opening bracket of a container (its members follow in `run`).
    fn emit(&mut self, v: Value) -> Res<()> {
        match v {
            Value::Null => self.ascii("null"),
            Value::Bool(b) => self.ascii(if b { "true" } else { "false" }),
            Value::String(s) => self.quote(s.units()),
            Value::Number(n) if n.is_finite() => self.ascii(&number::number_to_string(n)),
            Value::Number(_) => self.ascii("null"),
            Value::Object(o) => self.open_container(o),
            // Not reached: `serializable` filters these out before every call.
            _ => Ok(()),
        }
    }

    /// Starts an object or array: the cycle check (SerializeJSONObject step 1), the indentation for its members, and
    /// its opening bracket. The member keys (or the array length) are read here, before any member runs.
    fn open_container(&mut self, o: ObjRef) -> Res<()> {
        if self.ancestors.contains(&o) {
            return Err(self.rt.throw_type_error("Converting circular structure to JSON"));
        }
        let stepback = self.indent.len();
        self.indent.extend_from_slice(&self.gap);
        self.ancestors.insert(o);
        if self.rt.is_array(&Value::Object(o)) {
            let len = self.rt.get(o, &length_key(), &Value::Object(o))?;
            let len = self.rt.to_length(&len)? as usize;
            self.put(u16::from(b'['))?;
            self.frames.push(Frame::Arr { o, len, next: 0, stepback });
        } else {
            let keys = match &self.list {
                Some(list) => list.clone(),
                None => Rc::from(enumerable_keys(self.rt, o)?),
            };
            self.put(u16::from(b'{'))?;
            self.frames.push(Frame::Obj { o, keys, next: 0, count: 0, stepback });
        }
        Ok(())
    }

    /// Ends the innermost container: a line break before its closing bracket unless it is empty.
    fn close(&mut self) -> Res<()> {
        let Some(frame) = self.frames.pop() else {
            return Ok(());
        };
        let (o, stepback, bracket, nonempty) = match frame {
            Frame::Obj { o, stepback, count, .. } => (o, stepback, b'}', count > 0),
            Frame::Arr { o, stepback, len, .. } => (o, stepback, b']', len > 0),
        };
        if nonempty {
            self.newline(stepback)?;
        }
        self.put(u16::from(bracket))?;
        self.indent.truncate(stepback);
        self.ancestors.remove(&o);
        Ok(())
    }

    /// Writes the members of the open containers, innermost first. This is the explicit-stack form of
    /// SerializeJSONObject and SerializeJSONArray: each member runs toJSON, the replacer and the getter in the spec's
    /// order, and a nested container is finished before its parent's next member.
    fn run(&mut self) -> Res<()> {
        while let Some(top) = self.frames.len().checked_sub(1) {
            let step = match &mut self.frames[top] {
                Frame::Obj { o, keys, next, count, .. } if *next < keys.len() => {
                    let key = keys[*next].clone();
                    *next += 1;
                    Step::Member { holder: *o, key, array: false, first: *count == 0 }
                }
                Frame::Arr { o, len, next, .. } if *next < *len => {
                    let first = *next == 0;
                    let key = PropKey::Index(*next as u32);
                    *next += 1;
                    Step::Member { holder: *o, key, array: true, first }
                }
                _ => Step::Close,
            };
            match step {
                Step::Close => self.close()?,
                Step::Member { holder, key, array, first } => {
                    let value = self.prepare(holder, &key)?;
                    let live = self.serializable(&value);
                    // An object leaves out a member that is not serializable, before writing anything for it.
                    if !live && !array {
                        continue;
                    }
                    if !first {
                        self.put(u16::from(b','))?;
                    }
                    self.newline(self.indent.len())?;
                    if !array {
                        self.quote_key(&key)?;
                        self.put(u16::from(b':'))?;
                        if !self.gap.is_empty() {
                            self.put(0x20)?;
                        }
                        if let Frame::Obj { count, .. } = &mut self.frames[top] {
                            *count += 1;
                        }
                    }
                    if live {
                        self.emit(value)?;
                    } else {
                        self.ascii("null")?;
                    }
                }
            }
        }
        Ok(())
    }
}
