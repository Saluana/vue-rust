//! Array.prototype methods (ECMA-262 23.1.3) for the list in WP-B2. Holes follow V8: forEach, map, filter, reduce, some,
//! every, indexOf, lastIndexOf, flat, sort, splice, slice and concat skip absent elements (HasProperty); find,
//! findIndex, findLast, findLastIndex, includes, join, at, toReversed, toSorted and with read them as undefined.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

use super::{
    callable, check_dense, clamp_rel, create_array_iterator, delete_at, get_at, get_present, int_arg, len_of,
    make_array, set_at, set_len, this_obj, IterKind, MAX_SAFE,
};
use crate::function::{Call, CodeFn};
use crate::jsstr::JsStr;
use crate::object::{Flags, Prop, MAX_DENSE_LEN};
use crate::ops::{same_value_zero, strict_eq};
use crate::rt::{Intrinsic, Rt};
use crate::value::{ObjRef, PropKey, Res, Value};

thread_local! {
    /// Arrays whose `join` is running, by realm and handle. A cyclic element joins as the empty string (V8).
    static JOIN_STACK: RefCell<Vec<(usize, ObjRef)>> = const { RefCell::new(Vec::new()) };
}

/// Pops the join stack when a `join` returns, also on an error.
struct JoinGuard;

impl Drop for JoinGuard {
    fn drop(&mut self) {
        JOIN_STACK.with(|s| {
            s.borrow_mut().pop();
        });
    }
}

/// Installs every prototype method of the WP list, `toString` (which keeps %Object.prototype.toString% as read at
/// realm creation), and `values`, which is also `[Symbol.iterator]`.
pub(super) fn install(rt: &Rt, ap: ObjRef) {
    const METHODS: [(&str, CodeFn, u32); 36] = [
        ("at", at, 1),
        ("concat", concat, 1),
        ("entries", entries, 0),
        ("every", every, 1),
        ("fill", fill, 1),
        ("filter", filter, 1),
        ("find", find, 1),
        ("findIndex", find_index, 1),
        ("findLast", find_last, 1),
        ("findLastIndex", find_last_index, 1),
        ("flat", flat, 0),
        ("flatMap", flat_map, 1),
        ("forEach", for_each, 1),
        ("includes", includes, 1),
        ("indexOf", index_of, 1),
        ("join", join, 1),
        ("keys", keys, 0),
        ("lastIndexOf", last_index_of, 1),
        ("map", map, 1),
        ("pop", pop, 0),
        ("push", push, 1),
        ("reduce", reduce, 1),
        ("reduceRight", reduce_right, 1),
        ("reverse", reverse, 0),
        ("shift", shift, 0),
        ("slice", slice, 2),
        ("some", some, 1),
        ("sort", sort_array, 1),
        ("splice", splice, 2),
        ("toReversed", to_reversed, 0),
        ("toSorted", to_sorted, 1),
        ("toSpliced", to_spliced, 2),
        ("unshift", unshift, 1),
        ("with", with, 2),
        ("values", values, 0),
        ("toString", array_to_string, 0),
    ];
    for (name, code, len) in METHODS {
        if name == "toString" {
            let object_to_string = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::ObjectPrototype)), "toString");
            let f = rt.new_native_function(code, name, len, vec![object_to_string.unwrap_or(Value::Undefined)]);
            rt.define_builtin(ap, PropKey::from_str(name), Prop::Data { value: Value::Object(f), flags: Flags::HIDDEN });
        } else {
            rt.define_native_method(ap, name, code, len);
        }
    }
    // Array.prototype[Symbol.iterator] is the same function object as Array.prototype.values (V8, ECMA-262 23.1.3.38).
    let values_fn = rt.get_str(&Value::Object(ap), "values").unwrap_or(Value::Undefined);
    rt.define_builtin(
        ap,
        PropKey::Sym(rt.well_known(crate::rt::WellKnown::Iterator)),
        Prop::Data { value: values_fn, flags: Flags::HIDDEN },
    );
}

/// Pushes an element of a result, refusing a result past the dense limit.
fn push_checked(rt: &Rt, out: &mut Vec<Value>, v: Value) -> Res<()> {
    if out.len() >= MAX_DENSE_LEN {
        return Err(rt.throw_range_error("Invalid array length"));
    }
    out.push(v);
    Ok(())
}

// ----- element access and mutation -----

fn at(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "at")?;
    let len = len_of(rt, o)?;
    let rel = int_arg(rt, c, 0)?;
    let k = if rel >= 0.0 { rel } else { len + rel };
    if k < 0.0 || k >= len {
        return Ok(Value::Undefined);
    }
    get_at(rt, o, k)
}

fn push(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "push")?;
    let mut len = len_of(rt, o)?;
    let n = c.args.len() as f64;
    if len + n > MAX_SAFE {
        return Err(rt.throw_type_error(&format!(
            "Pushing {n} elements on an array-like of length {len} is disallowed, as the total surpasses 2**53-1"
        )));
    }
    for item in c.args {
        set_at(rt, o, len, item.clone())?;
        len += 1.0;
    }
    set_len(rt, o, len)?;
    Ok(Value::Number(len))
}

fn pop(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "pop")?;
    let len = len_of(rt, o)?;
    if len == 0.0 {
        set_len(rt, o, 0.0)?;
        return Ok(Value::Undefined);
    }
    let idx = len - 1.0;
    let v = get_at(rt, o, idx)?;
    delete_at(rt, o, idx)?;
    set_len(rt, o, idx)?;
    Ok(v)
}

fn shift(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "shift")?;
    let len = len_of(rt, o)?;
    if len == 0.0 {
        set_len(rt, o, 0.0)?;
        return Ok(Value::Undefined);
    }
    let first = get_at(rt, o, 0.0)?;
    let mut k = 1.0;
    while k < len {
        match get_present(rt, o, k)? {
            Some(v) => set_at(rt, o, k - 1.0, v)?,
            None => delete_at(rt, o, k - 1.0)?,
        }
        k += 1.0;
        rt.tick()?;
    }
    delete_at(rt, o, len - 1.0)?;
    set_len(rt, o, len - 1.0)?;
    Ok(first)
}

fn unshift(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "unshift")?;
    let len = len_of(rt, o)?;
    let argc = c.args.len() as f64;
    if argc > 0.0 {
        if len + argc > MAX_SAFE {
            return Err(rt.throw_type_error(&format!(
                "Pushing {argc} elements on an array-like of length {len} is disallowed, as the total surpasses 2**53-1"
            )));
        }
        let mut k = len;
        while k > 0.0 {
            let from = k - 1.0;
            let to = k + argc - 1.0;
            match get_present(rt, o, from)? {
                Some(v) => set_at(rt, o, to, v)?,
                None => delete_at(rt, o, to)?,
            }
            k -= 1.0;
            rt.tick()?;
        }
        for (j, item) in c.args.iter().enumerate() {
            set_at(rt, o, j as f64, item.clone())?;
        }
    }
    set_len(rt, o, len + argc)?;
    Ok(Value::Number(len + argc))
}

fn reverse(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "reverse")?;
    let len = len_of(rt, o)?;
    let middle = (len / 2.0).floor();
    let mut lower = 0.0;
    while lower != middle {
        let upper = len - lower - 1.0;
        let lv = get_present(rt, o, lower)?;
        let uv = get_present(rt, o, upper)?;
        match (lv, uv) {
            (Some(l), Some(u)) => {
                set_at(rt, o, lower, u)?;
                set_at(rt, o, upper, l)?;
            }
            (None, Some(u)) => {
                set_at(rt, o, lower, u)?;
                delete_at(rt, o, upper)?;
            }
            (Some(l), None) => {
                delete_at(rt, o, lower)?;
                set_at(rt, o, upper, l)?;
            }
            (None, None) => {}
        }
        lower += 1.0;
        rt.tick()?;
    }
    Ok(Value::Object(o))
}

fn fill(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "fill")?;
    let len = len_of(rt, o)?;
    let mut k = clamp_rel(int_arg(rt, c, 1)?, len);
    let fin = if c.arg(2).is_undefined() { len } else { clamp_rel(int_arg(rt, c, 2)?, len) };
    let v = c.arg(0);
    while k < fin {
        set_at(rt, o, k, v.clone())?;
        k += 1.0;
        rt.tick()?;
    }
    Ok(Value::Object(o))
}

fn splice(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "splice")?;
    let len = len_of(rt, o)?;
    let start = clamp_rel(int_arg(rt, c, 0)?, len);
    let item_count = c.args.len().saturating_sub(2) as f64;
    let del = if c.args.is_empty() {
        0.0
    } else if c.args.len() == 1 {
        len - start
    } else {
        int_arg(rt, c, 1)?.max(0.0).min(len - start)
    };
    if len + item_count - del > MAX_SAFE {
        return Err(rt.throw_type_error("Invalid array length"));
    }
    check_dense(rt, del)?;
    let mut removed = Vec::with_capacity(del as usize);
    let mut k = 0.0;
    while k < del {
        removed.push(get_present(rt, o, start + k)?.unwrap_or(Value::Hole));
        k += 1.0;
        rt.tick()?;
    }
    if item_count < del {
        let mut k = start;
        while k < len - del {
            let from = k + del;
            let to = k + item_count;
            match get_present(rt, o, from)? {
                Some(v) => set_at(rt, o, to, v)?,
                None => delete_at(rt, o, to)?,
            }
            k += 1.0;
            rt.tick()?;
        }
        let mut k = len;
        while k > len - del + item_count {
            delete_at(rt, o, k - 1.0)?;
            k -= 1.0;
            rt.tick()?;
        }
    } else if item_count > del {
        let mut k = len - del;
        while k > start {
            let from = k + del - 1.0;
            let to = k + item_count - 1.0;
            match get_present(rt, o, from)? {
                Some(v) => set_at(rt, o, to, v)?,
                None => delete_at(rt, o, to)?,
            }
            k -= 1.0;
            rt.tick()?;
        }
    }
    for (i, item) in c.args.iter().skip(2).enumerate() {
        set_at(rt, o, start + i as f64, item.clone())?;
    }
    set_len(rt, o, len - del + item_count)?;
    Ok(Value::Object(make_array(rt, removed, None)))
}

// ----- copies and searches -----

fn slice(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "slice")?;
    let len = len_of(rt, o)?;
    let k0 = clamp_rel(int_arg(rt, c, 0)?, len);
    let fin = if c.arg(1).is_undefined() { len } else { clamp_rel(int_arg(rt, c, 1)?, len) };
    let count = (fin - k0).max(0.0);
    check_dense(rt, count)?;
    let mut out = Vec::with_capacity(count as usize);
    let mut k = k0;
    while k < fin {
        out.push(get_present(rt, o, k)?.unwrap_or(Value::Hole));
        k += 1.0;
        rt.tick()?;
    }
    Ok(Value::Object(make_array(rt, out, None)))
}

fn concat(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "concat")?;
    let mut out: Vec<Value> = Vec::new();
    let items = std::iter::once(Value::Object(o)).chain(c.args.iter().cloned());
    for e in items {
        match e.as_object() {
            Some(eo) if rt.is_array(&e) => {
                let elen = len_of(rt, eo)?;
                check_dense(rt, out.len() as f64 + elen)?;
                let mut k = 0.0;
                while k < elen {
                    out.push(get_present(rt, eo, k)?.unwrap_or(Value::Hole));
                    k += 1.0;
                    rt.tick()?;
                }
            }
            _ => push_checked(rt, &mut out, e)?,
        }
    }
    Ok(Value::Object(make_array(rt, out, None)))
}

fn index_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "indexOf")?;
    let len = len_of(rt, o)?;
    if len == 0.0 {
        return Ok(Value::Number(-1.0));
    }
    let n = int_arg(rt, c, 1)?;
    if n == f64::INFINITY {
        return Ok(Value::Number(-1.0));
    }
    let mut k = if n >= 0.0 { n } else { (len + n).max(0.0) };
    let search = c.arg(0);
    while k < len {
        if let Some(v) = get_present(rt, o, k)? {
            if strict_eq(&search, &v) {
                return Ok(Value::Number(k));
            }
        }
        k += 1.0;
        rt.tick()?;
    }
    Ok(Value::Number(-1.0))
}

fn last_index_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "lastIndexOf")?;
    let len = len_of(rt, o)?;
    if len == 0.0 {
        return Ok(Value::Number(-1.0));
    }
    // An explicit fromIndex, even undefined, counts as present (V8).
    let n = if c.args.len() >= 2 { int_arg(rt, c, 1)? } else { len - 1.0 };
    if n == f64::NEG_INFINITY {
        return Ok(Value::Number(-1.0));
    }
    let mut k = if n >= 0.0 { n.min(len - 1.0) } else { len + n };
    let search = c.arg(0);
    while k >= 0.0 {
        if let Some(v) = get_present(rt, o, k)? {
            if strict_eq(&search, &v) {
                return Ok(Value::Number(k));
            }
        }
        k -= 1.0;
        rt.tick()?;
    }
    Ok(Value::Number(-1.0))
}

fn includes(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "includes")?;
    let len = len_of(rt, o)?;
    if len == 0.0 {
        return Ok(Value::Bool(false));
    }
    let n = int_arg(rt, c, 1)?;
    if n == f64::INFINITY {
        return Ok(Value::Bool(false));
    }
    let mut k = if n >= 0.0 { n } else { (len + n).max(0.0) };
    let search = c.arg(0);
    while k < len {
        let v = get_at(rt, o, k)?;
        if same_value_zero(&search, &v) {
            return Ok(Value::Bool(true));
        }
        k += 1.0;
        rt.tick()?;
    }
    Ok(Value::Bool(false))
}

fn to_reversed(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "toReversed")?;
    let len = len_of(rt, o)?;
    check_dense(rt, len)?;
    let mut out = Vec::with_capacity(len as usize);
    let mut k = 0.0;
    while k < len {
        out.push(get_at(rt, o, len - k - 1.0)?);
        k += 1.0;
        rt.tick()?;
    }
    Ok(Value::Object(make_array(rt, out, None)))
}

fn with(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "with")?;
    let len = len_of(rt, o)?;
    let rel = int_arg(rt, c, 0)?;
    let actual = if rel >= 0.0 { rel } else { len + rel };
    if actual >= len || actual < 0.0 {
        return Err(rt.throw_range_error("Invalid index"));
    }
    check_dense(rt, len)?;
    let v = c.arg(1);
    let mut out = Vec::with_capacity(len as usize);
    let mut k = 0.0;
    while k < len {
        out.push(if k == actual { v.clone() } else { get_at(rt, o, k)? });
        k += 1.0;
        rt.tick()?;
    }
    Ok(Value::Object(make_array(rt, out, None)))
}

fn to_spliced(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "toSpliced")?;
    let len = len_of(rt, o)?;
    let start = clamp_rel(int_arg(rt, c, 0)?, len);
    let insert = c.args.len().saturating_sub(2) as f64;
    let skip = if c.args.is_empty() {
        0.0
    } else if c.args.len() == 1 {
        len - start
    } else {
        int_arg(rt, c, 1)?.max(0.0).min(len - start)
    };
    let new_len = len + insert - skip;
    if new_len > MAX_SAFE {
        return Err(rt.throw_type_error("Invalid array length"));
    }
    check_dense(rt, new_len)?;
    let mut out = Vec::with_capacity(new_len as usize);
    let mut i = 0.0;
    while i < start {
        out.push(get_at(rt, o, i)?);
        i += 1.0;
        rt.tick()?;
    }
    out.extend(c.args.iter().skip(2).cloned());
    let mut r = start + skip;
    while (out.len() as f64) < new_len {
        out.push(get_at(rt, o, r)?);
        r += 1.0;
        rt.tick()?;
    }
    Ok(Value::Object(make_array(rt, out, None)))
}

// ----- join and toString -----

fn join(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "join")?;
    let len = len_of(rt, o)?;
    let sep: Vec<u16> = if c.arg(0).is_undefined() {
        vec![u16::from(b',')]
    } else {
        rt.to_string(&c.arg(0))?.units().to_vec()
    };
    join_with(rt, o, len, &sep)
}

fn join_with(rt: &Rt, o: ObjRef, len: f64, sep: &[u16]) -> Res<Value> {
    let key = (Rc::as_ptr(&rt.0) as usize, o);
    if JOIN_STACK.with(|s| s.borrow().contains(&key)) {
        return Ok(Value::String(JsStr::empty()));
    }
    JOIN_STACK.with(|s| s.borrow_mut().push(key));
    let _guard = JoinGuard;
    let mut out: Vec<u16> = Vec::new();
    let mut k = 0.0;
    while k < len {
        if k > 0.0 {
            out.extend_from_slice(sep);
        }
        let el = get_at(rt, o, k)?;
        if !el.is_nullish() {
            out.extend_from_slice(rt.to_string(&el)?.units());
        }
        k += 1.0;
        rt.tick()?;
    }
    Ok(Value::String(JsStr::from_vec(out)))
}

/// Array.prototype.toString (ECMA-262 23.1.3.36): `join` when it is callable, else %Object.prototype.toString%.
fn array_to_string(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "toString")?;
    let join_fn = rt.get(o, &PropKey::from_str("join"), &Value::Object(o))?;
    if rt.is_callable(&join_fn) {
        return rt.call(&join_fn, &Value::Object(o), &[]);
    }
    let object_to_string = rt.fn_data(c.callee)[0].clone();
    rt.call(&object_to_string, &Value::Object(o), &[])
}

// ----- iteration with callbacks -----

fn for_each(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "forEach")?;
    let len = len_of(rt, o)?;
    let f = callable(rt, &c.arg(0))?;
    let t = c.arg(1);
    let mut k = 0.0;
    while k < len {
        if let Some(v) = get_present(rt, o, k)? {
            rt.call(&f, &t, &[v, Value::Number(k), Value::Object(o)])?;
        }
        k += 1.0;
        rt.tick()?;
    }
    Ok(Value::Undefined)
}

fn map(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "map")?;
    let len = len_of(rt, o)?;
    let f = callable(rt, &c.arg(0))?;
    let t = c.arg(1);
    check_dense(rt, len)?;
    let mut out = vec![Value::Hole; len as usize];
    let mut k = 0.0;
    while k < len {
        if let Some(v) = get_present(rt, o, k)? {
            out[k as usize] = rt.call(&f, &t, &[v, Value::Number(k), Value::Object(o)])?;
        }
        k += 1.0;
        rt.tick()?;
    }
    Ok(Value::Object(make_array(rt, out, None)))
}

fn filter(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "filter")?;
    let len = len_of(rt, o)?;
    let f = callable(rt, &c.arg(0))?;
    let t = c.arg(1);
    let mut out: Vec<Value> = Vec::new();
    let mut k = 0.0;
    while k < len {
        if let Some(v) = get_present(rt, o, k)? {
            let kept = rt.call(&f, &t, &[v.clone(), Value::Number(k), Value::Object(o)])?.to_boolean();
            if kept {
                push_checked(rt, &mut out, v)?;
            }
        }
        k += 1.0;
        rt.tick()?;
    }
    Ok(Value::Object(make_array(rt, out, None)))
}

fn some(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "some")?;
    let len = len_of(rt, o)?;
    let f = callable(rt, &c.arg(0))?;
    let t = c.arg(1);
    let mut k = 0.0;
    while k < len {
        if let Some(v) = get_present(rt, o, k)? {
            if rt.call(&f, &t, &[v, Value::Number(k), Value::Object(o)])?.to_boolean() {
                return Ok(Value::Bool(true));
            }
        }
        k += 1.0;
        rt.tick()?;
    }
    Ok(Value::Bool(false))
}

fn every(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "every")?;
    let len = len_of(rt, o)?;
    let f = callable(rt, &c.arg(0))?;
    let t = c.arg(1);
    let mut k = 0.0;
    while k < len {
        if let Some(v) = get_present(rt, o, k)? {
            if !rt.call(&f, &t, &[v, Value::Number(k), Value::Object(o)])?.to_boolean() {
                return Ok(Value::Bool(false));
            }
        }
        k += 1.0;
        rt.tick()?;
    }
    Ok(Value::Bool(true))
}

/// find, findIndex, findLast and findLastIndex: every index is read (holes as undefined), in either direction.
fn find_generic(rt: &Rt, c: &Call<'_>, name: &str, from_end: bool, want_index: bool) -> Res<Value> {
    let o = this_obj(rt, c, name)?;
    let len = len_of(rt, o)?;
    let f = callable(rt, &c.arg(0))?;
    let t = c.arg(1);
    let mut k = if from_end { len - 1.0 } else { 0.0 };
    while (from_end && k >= 0.0) || (!from_end && k < len) {
        let v = get_at(rt, o, k)?;
        if rt.call(&f, &t, &[v.clone(), Value::Number(k), Value::Object(o)])?.to_boolean() {
            return Ok(if want_index { Value::Number(k) } else { v });
        }
        k += if from_end { -1.0 } else { 1.0 };
        rt.tick()?;
    }
    Ok(if want_index { Value::Number(-1.0) } else { Value::Undefined })
}

fn find(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    find_generic(rt, c, "find", false, false)
}

fn find_index(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    find_generic(rt, c, "findIndex", false, true)
}

fn find_last(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    find_generic(rt, c, "findLast", true, false)
}

fn find_last_index(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    find_generic(rt, c, "findLastIndex", true, true)
}

/// reduce (left to right) and reduceRight: the initial value is the second argument when it is passed at all.
fn reduce_generic(rt: &Rt, c: &Call<'_>, name: &str, right: bool) -> Res<Value> {
    let o = this_obj(rt, c, name)?;
    let len = len_of(rt, o)?;
    let f = callable(rt, &c.arg(0))?;
    let pos = |k: f64| if right { len - 1.0 - k } else { k };
    let mut k = 0.0;
    let mut acc = if c.args.len() >= 2 {
        c.arg(1)
    } else {
        let mut found = None;
        while found.is_none() && k < len {
            found = get_present(rt, o, pos(k))?;
            k += 1.0;
        }
        match found {
            Some(v) => v,
            None => return Err(rt.throw_type_error("Reduce of empty array with no initial value")),
        }
    };
    while k < len {
        if let Some(v) = get_present(rt, o, pos(k))? {
            acc = rt.call(&f, &Value::Undefined, &[acc, v, Value::Number(pos(k)), Value::Object(o)])?;
        }
        k += 1.0;
        rt.tick()?;
    }
    Ok(acc)
}

fn reduce(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    reduce_generic(rt, c, "reduce", false)
}

fn reduce_right(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    reduce_generic(rt, c, "reduceRight", true)
}

// ----- flattening -----

fn flat(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "flat")?;
    let len = len_of(rt, o)?;
    let depth = if c.arg(0).is_undefined() { 1.0 } else { int_arg(rt, c, 0)?.max(0.0) };
    let mut out = Vec::new();
    flatten_into(rt, &mut out, o, len, depth, None, 0)?;
    Ok(Value::Object(make_array(rt, out, None)))
}

fn flat_map(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "flatMap")?;
    let len = len_of(rt, o)?;
    let f = callable(rt, &c.arg(0))?;
    let t = c.arg(1);
    let mut out = Vec::new();
    flatten_into(rt, &mut out, o, len, 1.0, Some((&f, &t)), 0)?;
    Ok(Value::Object(make_array(rt, out, None)))
}

/// FlattenIntoArray (ECMA-262 23.1.3.13.1). `level` counts the nesting so that a cyclic array throws the RangeError of
/// a stack overflow (V8) instead of recursing without end.
fn flatten_into(
    rt: &Rt,
    out: &mut Vec<Value>,
    src: ObjRef,
    src_len: f64,
    depth: f64,
    mapper: Option<(&Value, &Value)>,
    level: u32,
) -> Res<()> {
    if rt.call_depth() + level >= rt.max_call_depth() {
        return Err(rt.throw_range_error("Maximum call stack size exceeded"));
    }
    let mut k = 0.0;
    while k < src_len {
        if let Some(v) = get_present(rt, src, k)? {
            let el = match mapper {
                Some((f, t)) => rt.call(f, t, &[v, Value::Number(k), Value::Object(src)])?,
                None => v,
            };
            let nested = if depth > 0.0 { el.as_object().filter(|_| rt.is_array(&el)) } else { None };
            match nested {
                Some(eo) => {
                    let elen = len_of(rt, eo)?;
                    flatten_into(rt, out, eo, elen, depth - 1.0, None, level + 1)?;
                }
                None => push_checked(rt, out, el)?,
            }
        }
        k += 1.0;
        rt.tick()?;
    }
    Ok(())
}

// ----- sorting -----

/// The values of a sort in their final order: the non-undefined values sorted by `cmp` (undefined when the default
/// string comparison applies), then the undefined values. Comparators are never called with undefined.
/// The values of a sort in their final order: the non-undefined values sorted by `cmp` (undefined when the default
/// string comparison applies), then the undefined values. Comparators are never called with undefined. `to_sorted`
/// selects toSorted's small-array path (see sort.rs).
fn sorted_values(rt: &Rt, items: Vec<Value>, cmp: &Value, to_sorted: bool) -> Res<Vec<Value>> {
    let mut vals: Vec<Value> = Vec::with_capacity(items.len());
    let mut undefined = 0usize;
    for v in items {
        if v.is_undefined() {
            undefined += 1;
        } else {
            vals.push(v);
        }
    }
    if cmp.is_undefined() {
        super::sort::sort_values(
            &mut vals,
            &mut |a: &Value, b: &Value| {
                let x = rt.to_string(a)?;
                let y = rt.to_string(b)?;
                Ok(match x.compare(&y) {
                    Ordering::Less => -1.0,
                    Ordering::Equal => 0.0,
                    Ordering::Greater => 1.0,
                })
            },
            to_sorted,
        )?;
    } else {
        super::sort::sort_values(
            &mut vals,
            &mut |a: &Value, b: &Value| {
                let r = rt.call(cmp, &Value::Undefined, &[a.clone(), b.clone()])?;
                let n = rt.to_number(&r)?;
                Ok(if n.is_nan() { 0.0 } else { n })
            },
            to_sorted,
        )?;
    }
    vals.extend((0..undefined).map(|_| Value::Undefined));
    Ok(vals)
}

fn check_comparator(rt: &Rt, cmp: &Value) -> Res<()> {
    if !cmp.is_undefined() && !rt.is_callable(cmp) {
        return Err(rt.throw_type_error("The comparison function must be either a function or undefined"));
    }
    Ok(())
}

/// Array.prototype.sort (ECMA-262 23.1.3.30). The array is written back only after the whole sort succeeds, so a
/// throwing comparator leaves it unchanged. Arrays of fewer than two elements are not written (V8).
fn sort_array(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let cmp = c.arg(0);
    check_comparator(rt, &cmp)?;
    let o = this_obj(rt, c, "sort")?;
    let len = len_of(rt, o)?;
    if len < 2.0 {
        return Ok(Value::Object(o));
    }
    let mut items = Vec::new();
    let mut k = 0.0;
    while k < len {
        if let Some(v) = get_present(rt, o, k)? {
            items.push(v);
        }
        k += 1.0;
        rt.tick()?;
    }
    let sorted = sorted_values(rt, items, &cmp, false)?;
    let n = sorted.len() as f64;
    for (j, v) in sorted.into_iter().enumerate() {
        set_at(rt, o, j as f64, v)?;
    }
    let mut j = n;
    while j < len {
        delete_at(rt, o, j)?;
        j += 1.0;
        rt.tick()?;
    }
    Ok(Value::Object(o))
}

fn to_sorted(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let cmp = c.arg(0);
    check_comparator(rt, &cmp)?;
    let o = this_obj(rt, c, "toSorted")?;
    let len = len_of(rt, o)?;
    check_dense(rt, len)?;
    let mut items = Vec::with_capacity(len as usize);
    let mut k = 0.0;
    while k < len {
        items.push(get_at(rt, o, k)?);
        k += 1.0;
        rt.tick()?;
    }
    let sorted = sorted_values(rt, items, &cmp, true)?;
    Ok(Value::Object(make_array(rt, sorted, None)))
}

// ----- keys, values and entries -----

fn keys(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "keys")?;
    Ok(create_array_iterator(rt, o, IterKind::Keys))
}

fn values(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "values")?;
    Ok(create_array_iterator(rt, o, IterKind::Values))
}

fn entries(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let o = this_obj(rt, c, "entries")?;
    Ok(create_array_iterator(rt, o, IterKind::Entries))
}
