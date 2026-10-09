// The helpers every generated module carries (WP-C1b, "generated prelude"). They are small Rust functions over the
// kernel API (ABI.md §0 and §13-14). Nothing here is a semantic shortcut: each helper states the spec step it performs.
export const PRELUDE = `#![allow(unused_mut, unused_variables, unused_assignments, dead_code, unreachable_code, non_snake_case, unused_labels, unused_parens, unused_braces, unused_imports)]
use vn_value::*;
use vn_value::builtins::iterator::{get_iterator, iter_close, iter_step, IterRecord};

/// Completion of a statement that crosses a try/finally or a for-of body (closures return Res<Flow>).
pub enum Flow { Normal, Return(Value), Break(u32), Continue(u32) }

fn vn_tdz(rt: &Rt, name: &str) -> Throw {
    rt.throw_reference_error(&format!("Cannot access '{name}' before initialization"))
}

/// Reads a Rust-local lexical binding: ReferenceError in the temporal dead zone.
fn vn_lget(rt: &Rt, v: &Value, name: &str) -> Res<Value> {
    if let Value::Uninit = v {
        return Err(vn_tdz(rt, name));
    }
    Ok(v.clone())
}

/// Assigns a Rust-local lexical binding (checked: ReferenceError before the declaration).
fn vn_lset(rt: &Rt, slot: &mut Value, v: Value, name: &str) -> Res<()> {
    if let Value::Uninit = slot {
        return Err(vn_tdz(rt, name));
    }
    *slot = v;
    Ok(())
}

fn vn_const_error(rt: &Rt) -> Throw {
    rt.throw_type_error("Assignment to constant variable.")
}

fn vn_global_key(rt: &Rt, name: &str) -> (ObjRef, PropKey) {
    (rt.intrinsic(Intrinsic::Global), PropKey::from_str(name))
}

/// An unresolved identifier read: ReferenceError when the global object has no such property.
fn vn_gget(rt: &Rt, name: &str) -> Res<Value> {
    let (g, key) = vn_global_key(rt, name);
    if !rt.has_property(g, &key)? {
        return Err(rt.throw_reference_error(&format!("{name} is not defined")));
    }
    rt.get(g, &key, &Value::Object(g))
}

/// typeof of an unresolved identifier: no error for a missing global.
fn vn_gtypeof(rt: &Rt, name: &str) -> Res<Value> {
    let (g, key) = vn_global_key(rt, name);
    if !rt.has_property(g, &key)? {
        return Ok(Value::Undefined);
    }
    rt.get(g, &key, &Value::Object(g))
}

/// A call whose callee is named in the message (so a missing builtin is visible in the failure text).
fn vn_call(rt: &Rt, f: &Value, this: &Value, args: &[Value], what: &str) -> Res<Value> {
    if !rt.is_callable(f) {
        return Err(rt.throw_type_error(&format!("{what} is not a function")));
    }
    rt.call(f, this, args)
}

/// Assignment to an unresolved identifier (strict mode): ReferenceError when the global is missing.
fn vn_gset(rt: &Rt, name: &str, v: Value) -> Res<()> {
    let (g, key) = vn_global_key(rt, name);
    if !rt.has_property(g, &key)? {
        return Err(rt.throw_reference_error(&format!("{name} is not defined")));
    }
    rt.set_strict(g, &key, v)
}

fn vn_nullish_read(rt: &Rt, base: &Value) -> Throw {
    let what = if matches!(base, Value::Null) { "null" } else { "undefined" };
    rt.throw_type_error(&format!("Cannot read properties of {what}"))
}

/// base[key] (GetValue): a nullish base throws before the key is converted.
fn vn_get(rt: &Rt, base: &Value, key: &Value) -> Res<Value> {
    if base.is_nullish() {
        return Err(vn_nullish_read(rt, base));
    }
    let k = rt.to_property_key(key)?;
    rt.get_v(base, &k)
}

/// base[key] = v (PutValue, strict): the key is converted after the right-hand side was evaluated.
fn vn_put(rt: &Rt, base: &Value, key: &Value, v: Value) -> Res<()> {
    if base.is_nullish() {
        let what = if matches!(base, Value::Null) { "null" } else { "undefined" };
        return Err(rt.throw_type_error(&format!("Cannot set properties of {what}")));
    }
    let k = rt.to_property_key(key)?;
    rt.set_v_strict(base, &k, v)
}

/// Converts a computed key once (used where the spec converts before the operation).
fn vn_key(rt: &Rt, key: &Value) -> Res<PropKey> {
    rt.to_property_key(key)
}

fn vn_bool(b: bool) -> Value {
    Value::Bool(b)
}

/// The Boolean value of a comparison result (loose_eq returns Value::Bool).
fn vn_truth(v: &Value) -> bool {
    v.to_boolean()
}

fn vn_not_bool(v: &Value) -> Value {
    Value::Bool(!v.to_boolean())
}

/// A key as a string value (for-in names and function names).
fn vn_key_value(k: &PropKey) -> Value {
    match k {
        PropKey::Index(i) => Value::String(JsStr::from_str(&i.to_string())),
        PropKey::Str(s) => Value::String(s.clone()),
        PropKey::Sym(s) => Value::Symbol(*s),
    }
}

/// Builds the array of an array literal (checked against the dense limit).
fn vn_array(rt: &Rt, elems: Vec<Value>) -> Res<Value> {
    if elems.len() > MAX_DENSE_LEN {
        return Err(rt.throw_range_error(&format!("Invalid array length: {}", elems.len())));
    }
    Ok(Value::Object(rt.new_array(elems)))
}

/// Appends the values of an iterable (spread in arrays and calls).
fn vn_spread(rt: &Rt, out: &mut Vec<Value>, v: &Value) -> Res<()> {
    let mut it = get_iterator(rt, v)?;
    while let Some(x) = iter_step(rt, &mut it)? {
        out.push(x);
    }
    Ok(())
}

/// Rest parameter: the arguments from index \`from\` on.
fn vn_rest(rt: &Rt, args: &[Value], from: usize) -> Res<Value> {
    let rest: Vec<Value> = args.iter().skip(from).cloned().collect();
    vn_array(rt, rest)
}

/// RequireObjectCoercible for object destructuring: null and undefined throw before any property is read.
fn vn_require_obj(rt: &Rt, v: &Value) -> Res<()> {
    if v.is_nullish() {
        return Err(vn_nullish_read(rt, v));
    }
    Ok(())
}

/// Object literal accessor: merges with the other half that the literal already defined.
fn vn_define_accessor(rt: &Rt, obj: ObjRef, key: PropKey, get: Option<Value>, set: Option<Value>) -> Res<()> {
    let (mut g, mut s) = match rt.get_own_property(obj, &key)? {
        Some(Prop::Accessor { get, set, .. }) => (get, set),
        _ => (None, None),
    };
    if let Some(Value::Object(f)) = get {
        g = Some(f);
    }
    if let Some(Value::Object(f)) = set {
        s = Some(f);
    }
    rt.define_accessor(obj, key, g, s, Flags::ALL)?;
    Ok(())
}

/// SetFunctionName for a computed key: the name is the key (symbols as "[description]") with an optional prefix.
fn vn_set_name(rt: &Rt, f: &Value, k: &PropKey, prefix: &str) -> Res<()> {
    let base = match k {
        PropKey::Sym(s) => match rt.symbol_description(*s) {
            Some(d) => format!("[{}]", d.to_rust_string_lossy()),
            None => String::new(),
        },
        PropKey::Str(s) => s.to_rust_string_lossy(),
        PropKey::Index(i) => i.to_string(),
    };
    if let Value::Object(o) = f {
        let name = JsStr::from_str(&format!("{prefix}{base}"));
        rt.define_own(*o, &PropKey::from_str("name"), Prop::Data { value: Value::String(name), flags: Flags::CONFIG })?;
    }
    Ok(())
}

/// CopyDataProperties (object spread and object rest): own enumerable keys, symbols included, in own-key order.
fn vn_copy_props(rt: &Rt, target: ObjRef, src: &Value, excluded: &[PropKey]) -> Res<()> {
    if src.is_nullish() {
        return Ok(());
    }
    let from = rt.to_object(src)?;
    for key in rt.own_keys(from)? {
        if excluded.contains(&key) {
            continue;
        }
        if let Some(p) = rt.get_own_property(from, &key)? {
            if p.flags().enumerable {
                let v = rt.get(from, &key, &Value::Object(from))?;
                rt.create_data_property(target, key, v)?;
            }
        }
    }
    Ok(())
}

/// delete base[key] in strict mode: a refused delete throws.
fn vn_delete(rt: &Rt, base: &Value, key: &Value) -> Res<Value> {
    if base.is_nullish() {
        return Err(rt.throw_type_error("Cannot convert undefined or null to object"));
    }
    let k = rt.to_property_key(key)?;
    let o = rt.to_object(base)?;
    if !rt.delete(o, &k)? {
        return Err(rt.throw_type_error("Cannot delete property in strict mode"));
    }
    Ok(Value::Bool(true))
}

/// The keys of a for-in loop: enumerable string keys of the object and its prototypes, own keys first, a key shadowed by
/// a nearer (even non-enumerable) property is skipped. The loop re-checks each key before visiting it.
fn vn_for_in_keys(rt: &Rt, v: &Value) -> Res<Vec<Value>> {
    if v.is_nullish() {
        return Ok(Vec::new());
    }
    let obj = rt.to_object(v)?;
    let mut seen: std::collections::HashSet<PropKey> = std::collections::HashSet::new();
    let mut out = Vec::new();
    let mut cur = Some(obj);
    while let Some(o) = cur {
        for key in rt.own_keys(o)? {
            if let PropKey::Sym(_) = key {
                continue;
            }
            if seen.insert(key.clone()) {
                if let Some(p) = rt.get_own_property(o, &key)? {
                    if p.flags().enumerable {
                        out.push(vn_key_value(&key));
                    }
                }
            }
        }
        cur = rt.get_prototype_of(o);
    }
    Ok(out)
}

/// Whether a for-in key is still present on the object when its turn comes (a key deleted earlier is skipped).
fn vn_for_in_present(rt: &Rt, obj: ObjRef, key: &Value) -> Res<bool> {
    let k = rt.to_property_key(key)?;
    rt.has_property(obj, &k)
}

/// IteratorClose with a normal completion (break, return, continue to an outer label): its errors propagate.
fn vn_iter_close_normal(rt: &Rt, it: &IterRecord) -> Res<()> {
    iter_close(rt, it, false)
}

/// IteratorClose with a throw completion: the original error wins, so the return() result is ignored.
fn vn_iter_close_throw(rt: &Rt, it: &IterRecord) {
    let _ = iter_close(rt, it, true);
}
`;
