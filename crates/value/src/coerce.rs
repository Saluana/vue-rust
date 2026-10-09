//! Conversions beyond the kernel's ToPrimitive, ToNumber, ToString and ToPropertyKey (those stay in rt.rs):
//! ToNumeric (no BigInt), ToObject, the integer conversions and `typeof`. Behaviour follows ECMA-262 7.1 and
//! 13.5.3. `Symbol.toPrimitive` is honoured by `Rt::to_primitive` (kernel behaviour, see ABI.md section 13).

use crate::function::Call;
use crate::jsstr::JsStr;
use crate::number;
use crate::object::{Obj, ObjectKind};
use crate::rt::{Intrinsic, Rt, WellKnown};
use crate::value::{ObjRef, PropKey, Res, Value};

/// 2^53 - 1, the largest integer a length or an index may hold (Number.MAX_SAFE_INTEGER).
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

impl Rt {
    /// ToNumeric (ECMA-262 7.1.3). There is no BigInt, so the result is always a Number.
    pub fn to_numeric(&self, v: &Value) -> Res<f64> {
        self.to_number(v)
    }

    /// ToObject (ECMA-262 7.1.18). A primitive becomes an `ObjectKind::Wrapper` whose prototype is the matching
    /// intrinsic. undefined and null throw.
    pub fn to_object(&self, v: &Value) -> Res<ObjRef> {
        let proto = match v {
            Value::Object(o) => return Ok(*o),
            Value::Undefined | Value::Null => {
                return Err(self.throw_type_error("Cannot convert undefined or null to object"));
            }
            Value::Bool(_) => Intrinsic::BooleanPrototype,
            Value::Number(_) => Intrinsic::NumberPrototype,
            Value::String(_) => Intrinsic::StringPrototype,
            Value::Symbol(_) => Intrinsic::SymbolPrototype,
            Value::Uninit | Value::Hole => panic!("internal marker converted to an object (framework bug)"),
        };
        Ok(self.alloc(Obj::new(Some(self.intrinsic(proto)), ObjectKind::Wrapper(v.clone()))))
    }

    /// ToIntegerOrInfinity (ECMA-262 7.1.5). NaN gives 0, and the result is never -0.
    pub fn to_integer_or_infinity(&self, v: &Value) -> Res<f64> {
        let n = number::to_integer_or_infinity(self.to_number(v)?);
        Ok(if n == 0.0 { 0.0 } else { n })
    }

    /// ToLength (ECMA-262 7.1.20): an integer in [0, 2^53 - 1].
    pub fn to_length(&self, v: &Value) -> Res<f64> {
        Ok(self.to_integer_or_infinity(v)?.clamp(0.0, MAX_SAFE_INTEGER))
    }

    /// ToIndex (ECMA-262 7.1.22): undefined gives 0; otherwise an integer in [0, 2^53 - 1], or a RangeError.
    pub fn to_index(&self, v: &Value) -> Res<f64> {
        if v.is_undefined() {
            return Ok(0.0);
        }
        let n = self.to_integer_or_infinity(v)?;
        if !(0.0..=MAX_SAFE_INTEGER).contains(&n) {
            return Err(self.throw_range_error("Invalid index"));
        }
        Ok(n)
    }

    /// ToInt32 (ECMA-262 7.1.6).
    pub fn to_int32(&self, v: &Value) -> Res<i32> {
        Ok(number::to_int32(self.to_number(v)?))
    }

    /// ToUint32 (ECMA-262 7.1.7).
    pub fn to_uint32(&self, v: &Value) -> Res<u32> {
        Ok(number::to_uint32(self.to_number(v)?))
    }

    /// The `typeof` result (ECMA-262 13.5.3). Same as `type_of`; the name follows the WP, since `typeof` is a
    /// reserved word in Rust.
    pub fn typeof_(&self, v: &Value) -> &'static str {
        self.type_of(v)
    }
}

/// Installs the one builtin the conversions need before the Object library (B1) exists:
/// `Object.prototype.toString` (ECMA-262 20.1.3.6). B1 may replace it.
pub(crate) fn install(rt: &Rt) {
    rt.define_native_method(rt.intrinsic(Intrinsic::ObjectPrototype), "toString", object_to_string, 0);
}

fn object_to_string(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    if c.this.is_nullish() {
        let tag = if c.this.is_undefined() { "Undefined" } else { "Null" };
        return Ok(Value::str(&format!("[object {tag}]")));
    }
    let o = rt.to_object(c.this)?;
    let tag = match rt.get(o, &PropKey::Sym(rt.well_known(WellKnown::ToStringTag)), &Value::Object(o))? {
        Value::String(s) => s,
        _ => JsStr::from_str(builtin_tag(rt, o)),
    };
    Ok(Value::String(JsStr::from_str("[object ").concat(&tag).concat(&JsStr::from_str("]"))))
}

/// The builtin tag of ECMA-262 20.1.3.6 for the kinds this kernel has (Date and RegExp do not exist yet).
/// A Symbol wrapper is "Object" here: Symbol.prototype has no @@toStringTag until the Symbol library exists.
fn builtin_tag(rt: &Rt, o: ObjRef) -> &'static str {
    if rt.is_array(&Value::Object(o)) {
        return "Array";
    }
    if rt.is_callable(&Value::Object(o)) {
        return "Function";
    }
    rt.with_obj(o, |ob| match &ob.kind {
        ObjectKind::Error => "Error",
        ObjectKind::Wrapper(Value::Bool(_)) => "Boolean",
        ObjectKind::Wrapper(Value::Number(_)) => "Number",
        ObjectKind::Wrapper(Value::String(_)) => "String",
        _ => "Object",
    })
}
