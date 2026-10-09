//! Builtin family: Boolean (WP-B1). Boolean is a function and a constructor; `new Boolean(v)` is a Boolean wrapper
//! (ObjectKind::Wrapper). Boolean.prototype is itself a Boolean wrapper holding false (ECMA-262 20.3.3). Methods:
//! toString and valueOf.

use crate::function::Call;
use crate::object::{Flags, Obj, ObjectKind, Prop};
use crate::builtins::object::proto_from_ctor;
use crate::rt::{Intrinsic, Rt};
use crate::value::{PropKey, Res, Value};

fn boolean_call(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(c.arg(0).to_boolean()))
}

fn boolean_construct(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let b = c.arg(0).to_boolean();
    let proto = proto_from_ctor(rt, c.new_target, Intrinsic::BooleanPrototype)?;
    Ok(Value::Object(rt.alloc(Obj::new(Some(proto), ObjectKind::Wrapper(Value::Bool(b))))))
}

/// thisBooleanValue (ECMA-262 20.3.3): a boolean or a Boolean wrapper. Anything else is a TypeError.
fn this_boolean(rt: &Rt, this: &Value, method: &str) -> Res<bool> {
    let found = match this {
        Value::Bool(b) => Some(*b),
        Value::Object(o) => rt.with_obj(*o, |ob| match &ob.kind {
            ObjectKind::Wrapper(Value::Bool(b)) => Some(*b),
            _ => None,
        }),
        _ => None,
    };
    found.ok_or_else(|| rt.throw_type_error(&format!("Boolean.prototype.{method} requires that 'this' be a Boolean")))
}

fn bp_to_string(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let b = this_boolean(rt, c.this, "toString")?;
    Ok(Value::str(if b { "true" } else { "false" }))
}

fn bp_value_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(this_boolean(rt, c.this, "valueOf")?))
}

pub fn install(rt: &Rt) {
    let bp = rt.intrinsic(Intrinsic::BooleanPrototype);
    rt.with_obj_mut(bp, |o| o.kind = ObjectKind::Wrapper(Value::Bool(false)));
    let ctor = rt.new_native_constructor(boolean_call, boolean_construct, "Boolean", 1, Vec::new());
    rt.define_builtin(ctor, PropKey::from_str("prototype"), Prop::Data { value: Value::Object(bp), flags: Flags::NONE });
    rt.define_builtin(bp, PropKey::from_str("constructor"), Prop::Data { value: Value::Object(ctor), flags: Flags::HIDDEN });
    rt.define_native_method(bp, "toString", bp_to_string, 0);
    rt.define_native_method(bp, "valueOf", bp_value_of, 0);
    rt.install_global("Boolean", Value::Object(ctor));
    rt.register_builtin("Boolean", ctor);
}
