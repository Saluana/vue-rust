//! Builtin family: Symbol (WP-B1). Symbol is callable and not constructible (`new Symbol()` throws TypeError).
//! Symbol.for and Symbol.keyFor; the well-known symbols iterator, asyncIterator, toPrimitive, toStringTag and
//! hasInstance (the other well-known symbols are absent). Symbol.prototype: toString, valueOf, description,
//! [Symbol.toPrimitive] and [Symbol.toStringTag].

use crate::function::Call;
use crate::jsstr::JsStr;
use crate::object::{Flags, ObjectKind, Prop};
use crate::rt::{Intrinsic, Rt, WellKnown};
use crate::value::{PropKey, Res, Sym, Value};

/// Symbol(description) (ECMA-262 20.4.1.1): undefined gives no description, anything else is ToString'd.
fn symbol_call(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = c.arg(0);
    let desc = if d.is_undefined() { None } else { Some(rt.to_string(&d)?) };
    Ok(Value::Symbol(rt.new_symbol(desc)))
}

fn symbol_for(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let key = rt.to_string(&c.arg(0))?;
    Ok(Value::Symbol(rt.symbol_for(&key)))
}

/// Symbol.keyFor (ECMA-262 20.4.2.6): the registry key of a registered symbol, undefined for any other symbol.
fn symbol_key_for(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let v = c.arg(0);
    let s = match v {
        Value::Symbol(s) => s,
        other => return Err(rt.throw_type_error(&format!("{} is not a symbol", rt.describe(&other)))),
    };
    let key = {
        let reg = rt.0.symbol_registry.borrow();
        reg.iter().find(|(_, sym)| **sym == s).map(|(k, _)| k.clone())
    };
    Ok(key.map_or(Value::Undefined, Value::String))
}

/// thisSymbolValue (ECMA-262 20.4.3): a symbol or a Symbol wrapper. Anything else is a TypeError.
fn this_symbol(rt: &Rt, this: &Value, method: &str) -> Res<Sym> {
    let found = match this {
        Value::Symbol(s) => Some(*s),
        Value::Object(o) => rt.with_obj(*o, |ob| match &ob.kind {
            ObjectKind::Wrapper(Value::Symbol(s)) => Some(*s),
            _ => None,
        }),
        _ => None,
    };
    found.ok_or_else(|| rt.throw_type_error(&format!("Symbol.prototype.{method} requires that 'this' be a Symbol")))
}

/// SymbolDescriptiveString (ECMA-262 20.4.3.3.1): "Symbol(" followed by the description, then ")".
fn descriptive(rt: &Rt, s: Sym) -> JsStr {
    let d = rt.symbol_description(s).unwrap_or_default();
    JsStr::from_str("Symbol(").concat(&d).concat(&JsStr::from_str(")"))
}

fn sp_to_string(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_symbol(rt, c.this, "toString")?;
    Ok(Value::String(descriptive(rt, s)))
}

fn sp_value_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Symbol(this_symbol(rt, c.this, "valueOf")?))
}

fn sp_description(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let s = this_symbol(rt, c.this, "description")?;
    Ok(rt.symbol_description(s).map_or(Value::Undefined, Value::String))
}

fn sp_to_primitive(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Symbol(this_symbol(rt, c.this, "[Symbol.toPrimitive]")?))
}

pub fn install(rt: &Rt) {
    let sp = rt.intrinsic(Intrinsic::SymbolPrototype);
    let ctor = rt.new_native_function(symbol_call, "Symbol", 0, Vec::new());
    rt.define_builtin(ctor, PropKey::from_str("prototype"), Prop::Data { value: Value::Object(sp), flags: Flags::NONE });
    rt.define_builtin(sp, PropKey::from_str("constructor"), Prop::Data { value: Value::Object(ctor), flags: Flags::HIDDEN });
    rt.define_native_method(ctor, "for", symbol_for, 1);
    rt.define_native_method(ctor, "keyFor", symbol_key_for, 1);

    let well_known: [(&str, WellKnown); 5] = [
        ("asyncIterator", WellKnown::AsyncIterator),
        ("hasInstance", WellKnown::HasInstance),
        ("iterator", WellKnown::Iterator),
        ("toPrimitive", WellKnown::ToPrimitive),
        ("toStringTag", WellKnown::ToStringTag),
    ];
    for (name, w) in well_known {
        let value = Value::Symbol(rt.well_known(w));
        rt.define_builtin(ctor, PropKey::from_str(name), Prop::Data { value, flags: Flags::NONE });
    }

    rt.define_native_method(sp, "toString", sp_to_string, 0);
    rt.define_native_method(sp, "valueOf", sp_value_of, 0);
    rt.define_native_getter(sp, "description", sp_description);
    let to_primitive = rt.new_native_function(sp_to_primitive, "[Symbol.toPrimitive]", 1, Vec::new());
    rt.define_builtin(
        sp,
        PropKey::Sym(rt.well_known(WellKnown::ToPrimitive)),
        Prop::Data { value: Value::Object(to_primitive), flags: Flags::new(false, false, true) },
    );
    rt.define_builtin(
        sp,
        PropKey::Sym(rt.well_known(WellKnown::ToStringTag)),
        Prop::Data { value: Value::str("Symbol"), flags: Flags::new(false, false, true) },
    );

    rt.install_global("Symbol", Value::Object(ctor));
    rt.register_builtin("Symbol", ctor);
}
