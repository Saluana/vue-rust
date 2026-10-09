//! Operators on `Value` (ECMA-262 13.5 to 13.15 and the comparison algorithms of 7.2) without BigInt. Each
//! operator converts its operands in the order the spec gives (left before right; IsLessThan runs its two
//! ToPrimitive calls in the order of its LeftFirst flag) and lets conversion throws propagate. Number/number
//! and string/string fast paths come first. `instance_of` does not consult `Symbol.hasInstance` (ABI.md).

use std::cmp::Ordering;

use crate::jsstr::{self, JsStr};
use crate::number;
use crate::rt::{Hint, Rt};
use crate::value::{ObjRef, PropKey, Res, Value};

/// IsStrictlyEqual, the `===` operator: no conversions. NaN differs from everything and
/// +0 equals -0.
pub fn strict_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Undefined, Value::Undefined) | (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Symbol(x), Value::Symbol(y)) => x == y,
        (Value::Object(x), Value::Object(y)) => x == y,
        _ => false,
    }
}

/// SameValueZero: like `===`, except that NaN equals NaN.
pub fn same_value_zero(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => number::same_value_zero(*x, *y),
        _ => strict_eq(a, b),
    }
}

/// Number comparison for IsLessThan: `None` is the spec's `undefined` (a NaN was involved).
fn less_numbers(a: f64, b: f64) -> Option<bool> {
    if a.is_nan() || b.is_nan() { None } else { Some(a < b) }
}

/// A value as V8 prints it inside an error message. No conversion runs: strings print bare.
fn msg_text(rt: &Rt, v: &Value) -> String {
    match v {
        Value::String(s) => s.to_rust_string_lossy(),
        other => rt.describe(other),
    }
}

impl Rt {
    // ----- arithmetic (ECMA-262 13.15.3) -----

    /// `a + b`. Strings concatenate when either primitive is a string; otherwise both are ToNumeric-ed and added.
    /// Objects use the default hint.
    pub fn add(&self, a: &Value, b: &Value) -> Res<Value> {
        match (a, b) {
            (Value::Number(x), Value::Number(y)) => return Ok(Value::Number(*x + *y)),
            (Value::String(x), Value::String(y)) => return self.concat_str(x, y),
            _ => {}
        }
        let pa = self.to_primitive(a, Hint::Default)?;
        let pb = self.to_primitive(b, Hint::Default)?;
        if matches!(pa, Value::String(_)) || matches!(pb, Value::String(_)) {
            let sa = self.to_string(&pa)?;
            let sb = self.to_string(&pb)?;
            return self.concat_str(&sa, &sb);
        }
        let x = self.to_numeric(&pa)?;
        let y = self.to_numeric(&pb)?;
        Ok(Value::Number(x + y))
    }

    fn concat_str(&self, x: &JsStr, y: &JsStr) -> Res<Value> {
        if x.len() + y.len() > jsstr::MAX_LEN {
            return Err(self.throw_range_error("Invalid string length"));
        }
        Ok(Value::String(x.concat(y)))
    }

    /// ToNumeric of both operands, left first: the order every numeric and bitwise operator uses.
    fn numeric_operands(&self, a: &Value, b: &Value) -> Res<(f64, f64)> {
        if let (Value::Number(x), Value::Number(y)) = (a, b) {
            return Ok((*x, *y));
        }
        let x = self.to_numeric(a)?;
        let y = self.to_numeric(b)?;
        Ok((x, y))
    }

    /// `a - b`.
    pub fn sub(&self, a: &Value, b: &Value) -> Res<Value> {
        let (x, y) = self.numeric_operands(a, b)?;
        Ok(Value::Number(x - y))
    }

    /// `a * b`.
    pub fn mul(&self, a: &Value, b: &Value) -> Res<Value> {
        let (x, y) = self.numeric_operands(a, b)?;
        Ok(Value::Number(x * y))
    }

    /// `a / b`.
    pub fn div(&self, a: &Value, b: &Value) -> Res<Value> {
        let (x, y) = self.numeric_operands(a, b)?;
        Ok(Value::Number(x / y))
    }

    /// `a % b` (remainder with the sign of the dividend, as fmod).
    pub fn rem(&self, a: &Value, b: &Value) -> Res<Value> {
        let (x, y) = self.numeric_operands(a, b)?;
        Ok(Value::Number(number::js_rem(x, y)))
    }

    /// `a ** b` (Number::exponentiate).
    pub fn pow(&self, a: &Value, b: &Value) -> Res<Value> {
        let (x, y) = self.numeric_operands(a, b)?;
        Ok(Value::Number(number::js_pow(x, y)))
    }

    /// Unary `-a`: ToNumeric, then negation.
    pub fn neg(&self, a: &Value) -> Res<Value> {
        Ok(Value::Number(-self.to_numeric(a)?))
    }

    /// Unary `+a`: ToNumber, which throws for a Symbol.
    pub fn pos(&self, a: &Value) -> Res<Value> {
        Ok(Value::Number(self.to_number(a)?))
    }

    /// `++a` as a value: ToNumeric plus one. A postfix caller keeps the old `to_numeric(a)` itself.
    pub fn inc(&self, a: &Value) -> Res<Value> {
        Ok(Value::Number(self.to_numeric(a)? + 1.0))
    }

    /// `--a` as a value: ToNumeric minus one.
    pub fn dec(&self, a: &Value) -> Res<Value> {
        Ok(Value::Number(self.to_numeric(a)? - 1.0))
    }

    // ----- bitwise (ECMA-262 13.15.3, 6.1.6.1) -----

    /// `~a`: ToNumeric, ToInt32, bitwise not.
    pub fn bit_not(&self, a: &Value) -> Res<Value> {
        Ok(Value::Number(f64::from(!self.to_int32(a)?)))
    }

    /// `a & b`.
    pub fn bit_and(&self, a: &Value, b: &Value) -> Res<Value> {
        let (x, y) = self.numeric_operands(a, b)?;
        Ok(Value::Number(f64::from(number::to_int32(x) & number::to_int32(y))))
    }

    /// `a | b`.
    pub fn bit_or(&self, a: &Value, b: &Value) -> Res<Value> {
        let (x, y) = self.numeric_operands(a, b)?;
        Ok(Value::Number(f64::from(number::to_int32(x) | number::to_int32(y))))
    }

    /// `a ^ b`.
    pub fn bit_xor(&self, a: &Value, b: &Value) -> Res<Value> {
        let (x, y) = self.numeric_operands(a, b)?;
        Ok(Value::Number(f64::from(number::to_int32(x) ^ number::to_int32(y))))
    }

    /// `a << b`: the shift count is ToUint32(b) & 31.
    pub fn shl(&self, a: &Value, b: &Value) -> Res<Value> {
        let (x, y) = self.numeric_operands(a, b)?;
        Ok(Value::Number(f64::from(number::to_int32(x).wrapping_shl(number::to_uint32(y) & 31))))
    }

    /// `a >> b` (sign-propagating).
    pub fn shr(&self, a: &Value, b: &Value) -> Res<Value> {
        let (x, y) = self.numeric_operands(a, b)?;
        Ok(Value::Number(f64::from(number::to_int32(x).wrapping_shr(number::to_uint32(y) & 31))))
    }

    /// `a >>> b` (zero-filling, on the uint32 value of `a`).
    pub fn ushr(&self, a: &Value, b: &Value) -> Res<Value> {
        let (x, y) = self.numeric_operands(a, b)?;
        Ok(Value::Number(f64::from(number::to_uint32(x).wrapping_shr(number::to_uint32(y) & 31))))
    }

    // ----- logical and relational (LogicalNOT, IsLessThan, IsLooselyEqual) -----

    /// `!a`: the negation of ToBoolean. Never throws.
    pub fn not_(&self, a: &Value) -> bool {
        !a.to_boolean()
    }

    /// IsLessThan (ECMA-262 7.2.13) without BigInt. `None` is the spec's `undefined`. `left_first` picks the
    /// order of the two ToPrimitive calls: `x` first when true, `y` first when false.
    fn is_less_than(&self, x: &Value, y: &Value, left_first: bool) -> Res<Option<bool>> {
        if let (Value::Number(a), Value::Number(b)) = (x, y) {
            return Ok(less_numbers(*a, *b));
        }
        if let (Value::String(a), Value::String(b)) = (x, y) {
            return Ok(Some(a.compare(b) == Ordering::Less));
        }
        let (px, py) = if left_first {
            let px = self.to_primitive(x, Hint::Number)?;
            let py = self.to_primitive(y, Hint::Number)?;
            (px, py)
        } else {
            let py = self.to_primitive(y, Hint::Number)?;
            let px = self.to_primitive(x, Hint::Number)?;
            (px, py)
        };
        if let (Value::String(a), Value::String(b)) = (&px, &py) {
            return Ok(Some(a.compare(b) == Ordering::Less));
        }
        let nx = self.to_numeric(&px)?;
        let ny = self.to_numeric(&py)?;
        Ok(less_numbers(nx, ny))
    }

    /// `a < b`.
    pub fn lt(&self, a: &Value, b: &Value) -> Res<Value> {
        Ok(Value::Bool(self.is_less_than(a, b, true)? == Some(true)))
    }

    /// `a > b`: IsLessThan(b, a) with LeftFirst false, so `a` is still the first operand converted.
    pub fn gt(&self, a: &Value, b: &Value) -> Res<Value> {
        Ok(Value::Bool(self.is_less_than(b, a, false)? == Some(true)))
    }

    /// `a <= b`: the negation of `b < a`, where `undefined` counts as false.
    pub fn le(&self, a: &Value, b: &Value) -> Res<Value> {
        Ok(Value::Bool(self.is_less_than(b, a, false)? == Some(false)))
    }

    /// `a >= b`: the negation of `a < b`, where `undefined` counts as false.
    pub fn ge(&self, a: &Value, b: &Value) -> Res<Value> {
        Ok(Value::Bool(self.is_less_than(a, b, true)? == Some(false)))
    }

    /// `a == b` (IsLooselyEqual, ECMA-262 7.2.14) without BigInt. `!=` is the negation of this.
    pub fn loose_eq(&self, a: &Value, b: &Value) -> Res<Value> {
        Ok(Value::Bool(self.is_loosely_equal(a, b)?))
    }

    fn is_loosely_equal(&self, x: &Value, y: &Value) -> Res<bool> {
        match (x, y) {
            (Value::Number(a), Value::Number(b)) => return Ok(a == b),
            (Value::String(a), Value::String(b)) => return Ok(a == b),
            _ => {}
        }
        if std::mem::discriminant(x) == std::mem::discriminant(y) {
            return Ok(strict_eq(x, y));
        }
        Ok(match (x, y) {
            (Value::Undefined | Value::Null, Value::Undefined | Value::Null) => true,
            (Value::Number(a), Value::String(s)) => *a == number::string_to_number(s),
            (Value::String(s), Value::Number(b)) => number::string_to_number(s) == *b,
            (Value::Bool(b), _) => return self.is_loosely_equal(&Value::Number(f64::from(u8::from(*b))), y),
            (_, Value::Bool(b)) => return self.is_loosely_equal(x, &Value::Number(f64::from(u8::from(*b)))),
            (Value::Object(_), Value::Number(_) | Value::String(_) | Value::Symbol(_)) => {
                let px = self.to_primitive(x, Hint::Default)?;
                return self.is_loosely_equal(&px, y);
            }
            (Value::Number(_) | Value::String(_) | Value::Symbol(_), Value::Object(_)) => {
                let py = self.to_primitive(y, Hint::Default)?;
                return self.is_loosely_equal(x, &py);
            }
            _ => false,
        })
    }

    // ----- object relations (instanceof and in) -----

    /// `a instanceof b`: `b` must be callable; the prototype chain of `a` is searched for `b.prototype`.
    /// `Symbol.hasInstance` is not consulted (ABI.md section 13).
    pub fn instance_of(&self, a: &Value, b: &Value) -> Res<bool> {
        let Value::Object(c) = b else {
            return Err(self.throw_type_error("Right-hand side of 'instanceof' is not an object"));
        };
        if !self.is_callable(b) {
            return Err(self.throw_type_error("Right-hand side of 'instanceof' is not callable"));
        }
        self.ordinary_has_instance(*c, a)
    }

    /// OrdinaryHasInstance, for a function that is not a bound function.
    fn ordinary_has_instance(&self, c: ObjRef, v: &Value) -> Res<bool> {
        let Value::Object(o) = v else { return Ok(false) };
        let proto = self.get(c, &PropKey::from_str("prototype"), &Value::Object(c))?;
        let Value::Object(p) = proto else {
            return Err(self.throw_type_error(&format!(
                "Function has non-object prototype '{}' in instanceof check",
                msg_text(self, &proto)
            )));
        };
        let mut cur = self.get_prototype_of(*o);
        while let Some(x) = cur {
            if x == p {
                return Ok(true);
            }
            cur = self.get_prototype_of(x);
        }
        Ok(false)
    }

    /// `key in obj`. A non-object right side throws before the key is converted.
    pub fn in_operator(&self, key: &Value, obj: &Value) -> Res<bool> {
        let Value::Object(o) = obj else {
            return Err(self.throw_type_error(&format!(
                "Cannot use 'in' operator to search for '{}' in {}",
                msg_text(self, key),
                msg_text(self, obj)
            )));
        };
        let k = self.to_property_key(key)?;
        self.has_property(*o, &k)
    }
}
