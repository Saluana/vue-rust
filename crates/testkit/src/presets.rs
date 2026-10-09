//! Native callback presets of the builtin corpora (docs/dev/builtin-corpus.md). Each preset is a function object
//! whose behaviour is the JavaScript body in tooling/oracle/builtin-corpus.mjs: it appends `[name, ...loggedArgs]`
//! to the shared log before it returns or throws. `tests/presets.rs` checks the bodies against Node's results.

use vn_value::{Call, ErrorKind, Flags, Hint, ObjRef, PropKey, Res, Rt, Value, MAX_DENSE_LEN};

use crate::trace::js_string_of;

/// The presets and their `length`, in the order of the generator's PRESET_ARITY.
pub const PRESETS: [(&str, u32); 15] = [
    ("identity", 1),
    ("double", 1),
    ("isEven", 1),
    ("isTruthy", 1),
    ("index", 2),
    ("pair", 2),
    ("sum", 2),
    ("concat", 2),
    ("cmpNum", 2),
    ("cmpDesc", 2),
    ("cmpStr", 2),
    ("cmpThrow", 2),
    ("throwAt2", 2),
    ("toArr2", 1),
    ("keyOf", 1),
];

/// Creates the presets of one execution (its own `cmpThrow` counter) in `rt`, logging into `log`.
pub fn install_presets(rt: &Rt, log: ObjRef) -> Vec<(&'static str, Value)> {
    let counter = rt.new_cell(Value::Number(0.0));
    PRESETS
        .iter()
        .map(|(name, length)| {
            let data = vec![Value::Object(log), Value::Object(counter), Value::str(name)];
            let f = rt.new_native_function(preset_code, name, *length, data);
            (*name, Value::Object(f))
        })
        .collect()
}

/// `logged(v, index)`: an object is logged as the index it was passed (or -1), a primitive as itself.
fn logged(v: &Value, index: &Value) -> Value {
    match v {
        Value::Object(_) => match index {
            Value::Number(n) if n.is_finite() && n.fract() == 0.0 => Value::Number(*n),
            _ => Value::Number(-1.0),
        },
        other => other.clone(),
    }
}

/// `logged` for the callers that pass no index at all (sort comparators).
fn logged_plain(v: &Value) -> Value {
    logged(v, &Value::Undefined)
}

fn log_push(rt: &Rt, log: ObjRef, entry: Vec<Value>) -> Res<()> {
    let len = match rt.get_str(&Value::Object(log), "length")? {
        Value::Number(n) => n as u32,
        _ => 0,
    };
    let entry = Value::Object(rt.new_array(entry));
    rt.define_data(log, PropKey::Index(len), entry, Flags::ALL)?;
    Ok(())
}

fn preset_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee);
    let (Value::Object(log), Value::Object(counter)) = (&data[0], &data[1]) else {
        return Err(rt.throw_type_error("preset without its log (framework bug)"));
    };
    let (log, counter) = (*log, *counter);
    let Value::String(name) = &data[2] else {
        return Err(rt.throw_type_error("preset without its name (framework bug)"));
    };
    let name = name.to_rust_string_lossy();
    let x = c.arg(0);
    let second = c.arg(1);
    let third = c.arg(2);
    let entry = |vals: Vec<Value>| {
        let mut full = vec![Value::str(&name)];
        full.extend(vals);
        full
    };
    match name.as_str() {
        "identity" => {
            log_push(rt, log, entry(vec![logged(&x, &second)]))?;
            Ok(x)
        }
        "double" => {
            log_push(rt, log, entry(vec![logged(&x, &second)]))?;
            Ok(Value::Number(rt.to_number(&x)? * 2.0))
        }
        "isEven" => {
            log_push(rt, log, entry(vec![logged(&x, &second)]))?;
            let n = rt.to_number(&x)?;
            Ok(Value::Bool(vn_value::number::js_rem(n, 2.0) == 0.0))
        }
        "isTruthy" => {
            log_push(rt, log, entry(vec![logged(&x, &second)]))?;
            Ok(Value::Bool(x.to_boolean()))
        }
        "index" => {
            log_push(rt, log, entry(vec![logged(&x, &second), second.clone()]))?;
            Ok(second)
        }
        "pair" => {
            log_push(rt, log, entry(vec![logged(&x, &second), second.clone()]))?;
            Ok(Value::Object(rt.new_array(vec![x, second])))
        }
        "sum" => {
            log_push(rt, log, entry(vec![logged(&x, &third), logged(&second, &third)]))?;
            js_add(rt, &x, &second)
        }
        "concat" => {
            log_push(rt, log, entry(vec![logged(&x, &third), logged(&second, &third)]))?;
            concat(rt, &[x, second])
        }
        "cmpNum" => {
            log_push(rt, log, entry(vec![logged_plain(&x), logged_plain(&second)]))?;
            let a = rt.to_number(&x)?;
            let b = rt.to_number(&second)?;
            Ok(Value::Number(a - b))
        }
        "cmpDesc" => {
            log_push(rt, log, entry(vec![logged_plain(&x), logged_plain(&second)]))?;
            let b = rt.to_number(&second)?;
            let a = rt.to_number(&x)?;
            Ok(Value::Number(b - a))
        }
        "cmpStr" => {
            log_push(rt, log, entry(vec![logged_plain(&x), logged_plain(&second)]))?;
            // a < b ? -1 : a > b ? 1 : 0, where a > b is IsLessThan(b, a) with the operands converted in a, b order.
            if js_less(rt, &x, &second, true)? == Some(true) {
                Ok(Value::Number(-1.0))
            } else if js_less(rt, &second, &x, false)? == Some(true) {
                Ok(Value::Number(1.0))
            } else {
                Ok(Value::Number(0.0))
            }
        }
        "cmpThrow" => {
            let calls = match rt.cell_get(counter) {
                Value::Number(n) => n + 1.0,
                _ => 1.0,
            };
            rt.cell_set(counter, Value::Number(calls));
            log_push(rt, log, entry(vec![logged_plain(&x), logged_plain(&second)]))?;
            if calls == 2.0 {
                return Err(rt.throw_error(ErrorKind::Error, "cmp"));
            }
            let a = rt.to_number(&x)?;
            let b = rt.to_number(&second)?;
            Ok(Value::Number(a - b))
        }
        "throwAt2" => {
            log_push(rt, log, entry(vec![logged(&x, &second), second.clone()]))?;
            if matches!(second, Value::Number(n) if n == 2.0) {
                return Err(rt.throw_error(ErrorKind::Error, "at2"));
            }
            Ok(x)
        }
        "toArr2" => {
            log_push(rt, log, entry(vec![logged(&x, &second)]))?;
            Ok(Value::Object(rt.new_array(vec![x.clone(), x])))
        }
        "keyOf" => {
            log_push(rt, log, entry(vec![logged(&x, &second)]))?;
            Ok(Value::String(js_string_of(rt, &x)?))
        }
        _ => Err(rt.throw_type_error("unknown preset (framework bug)")),
    }
}

/// The `+` operator: ToPrimitive on both operands, then string concatenation or numeric addition.
fn js_add(rt: &Rt, a: &Value, b: &Value) -> Res<Value> {
    let pa = rt.to_primitive(a, Hint::Default)?;
    let pb = rt.to_primitive(b, Hint::Default)?;
    if matches!(pa, Value::String(_)) || matches!(pb, Value::String(_)) {
        let sa = rt.to_string(&pa)?;
        let sb = rt.to_string(&pb)?;
        return Ok(Value::String(sa.concat(&sb)));
    }
    Ok(Value::Number(rt.to_number(&pa)? + rt.to_number(&pb)?))
}

/// IsLessThan(x, y): `None` when the comparison is undefined (a NaN operand). Primitives are converted in the
/// order the spec gives: x first when `left_first`, else y first.
fn js_less(rt: &Rt, x: &Value, y: &Value, left_first: bool) -> Res<Option<bool>> {
    let (px, py) = if left_first {
        let px = rt.to_primitive(x, Hint::Number)?;
        (px, rt.to_primitive(y, Hint::Number)?)
    } else {
        let py = rt.to_primitive(y, Hint::Number)?;
        (rt.to_primitive(x, Hint::Number)?, py)
    };
    if let (Value::String(sx), Value::String(sy)) = (&px, &py) {
        return Ok(Some(sx.units() < sy.units()));
    }
    let nx = rt.to_number(&px)?;
    let ny = rt.to_number(&py)?;
    if nx.is_nan() || ny.is_nan() {
        return Ok(None);
    }
    Ok(Some(nx < ny))
}

/// `[].concat(...items)`: arrays are spread (holes kept), other values are appended.
fn concat(rt: &Rt, items: &[Value]) -> Res<Value> {
    let mut out: Vec<Value> = Vec::new();
    for item in items {
        match item {
            Value::Object(o) if rt.is_array(item) => {
                let len = match rt.get_str(item, "length")? {
                    Value::Number(n) => n as u32,
                    _ => 0,
                };
                for i in 0..len {
                    let key = PropKey::Index(i);
                    if rt.has_property(*o, &key)? {
                        out.push(rt.get_v(item, &key)?);
                    } else {
                        out.push(Value::Hole);
                    }
                }
            }
            other => out.push(other.clone()),
        }
    }
    if out.len() > MAX_DENSE_LEN {
        return Err(rt.throw_range_error("Invalid array length: concat result exceeds the dense limit"));
    }
    Ok(Value::Object(rt.new_array(out)))
}
