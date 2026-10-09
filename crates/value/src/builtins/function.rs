//! Builtin family: Function.prototype (WP-B1): call, apply, bind, toString and [Symbol.hasInstance]. The Function
//! constructor is absent (manifest X, L20). A bound function is a native function whose record (target, bound this,
//! bound arguments) sits in its extension slot; its [[Call]] and [[Construct]] forward to the target.

use std::rc::Rc;

use crate::function::Call;
use crate::jsstr::JsStr;
use crate::native::{payload_ref, NativeObject};
use crate::object::{Flags, ObjectKind, Prop, MAX_DENSE_LEN};
use crate::roots::Tracer;
use crate::rt::{Intrinsic, Rt, WellKnown};
use crate::value::{ObjRef, PropKey, Res, Value};

/// The record of a bound function (ECMA-262 10.4.1.3 BoundFunctionCreate).
struct BoundFn {
    target: ObjRef,
    this: Value,
    args: Vec<Value>,
}

impl NativeObject for BoundFn {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.target);
        t.mark_value(&self.this);
        for a in &self.args {
            t.mark_value(a);
        }
    }
}

/// Target, bound this and bound arguments of `f`, or None when `f` is not a bound function.
fn bound_parts(rt: &Rt, f: ObjRef) -> Option<(ObjRef, Value, Vec<Value>)> {
    let ext = rt.ext(f)?;
    let b = payload_ref::<BoundFn>(&*ext)?;
    Some((b.target, b.this.clone(), b.args.clone()))
}

fn not_a_function(rt: &Rt, what: &str) -> crate::value::Throw {
    rt.throw_type_error(&format!("{what} called on a non-function"))
}

/// CreateListFromArrayLike (ECMA-262 7.3.18). A length above the dense limit is a RangeError, not an allocation
/// of that size; V8 also refuses such argument lists.
pub(crate) fn list_from_array_like(rt: &Rt, v: &Value) -> Res<Vec<Value>> {
    let Value::Object(o) = v else {
        return Err(rt.throw_type_error("CreateListFromArrayLike called on non-object"));
    };
    let len_v = rt.get(*o, &PropKey::from_str("length"), v)?;
    let len = rt.to_length(&len_v)?;
    if len > MAX_DENSE_LEN as f64 {
        return Err(rt.throw_range_error("Maximum call stack size exceeded"));
    }
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len as u32 {
        out.push(rt.get(*o, &PropKey::Index(i), v)?);
    }
    Ok(out)
}

/// InstanceofOperator (ECMA-262 13.10.2): `Symbol.hasInstance` of the target when it has one, else
/// OrdinaryHasInstance. The kernel operator `Rt::instance_of` does not consult the symbol (ABI.md section 13.4).
pub(crate) fn instance_of(rt: &Rt, v: &Value, target: &Value) -> Res<bool> {
    if !matches!(target, Value::Object(_)) {
        return Err(rt.throw_type_error("Right-hand side of 'instanceof' is not an object"));
    }
    let h = rt.get_v(target, &PropKey::Sym(rt.well_known(WellKnown::HasInstance)))?;
    if !h.is_nullish() {
        if !rt.is_callable(&h) {
            return Err(rt.throw_type_error("Symbol.hasInstance is not a function"));
        }
        return Ok(rt.call(&h, target, std::slice::from_ref(v))?.to_boolean());
    }
    if !rt.is_callable(target) {
        return Err(rt.throw_type_error("Right-hand side of 'instanceof' is not callable"));
    }
    ordinary_has_instance(rt, target, v)
}

/// OrdinaryHasInstance (ECMA-262 7.3.22). A bound function answers for its target.
pub(crate) fn ordinary_has_instance(rt: &Rt, ctor: &Value, v: &Value) -> Res<bool> {
    let Value::Object(f) = ctor else { return Ok(false) };
    if !rt.is_callable(ctor) {
        return Ok(false);
    }
    if let Some((target, _, _)) = bound_parts(rt, *f) {
        return instance_of(rt, v, &Value::Object(target));
    }
    let Value::Object(o) = v else { return Ok(false) };
    let proto = rt.get(*f, &PropKey::from_str("prototype"), ctor)?;
    let p = match proto {
        Value::Object(p) => p,
        other => {
            return Err(rt.throw_type_error(&format!(
                "Function has non-object prototype '{}' in instanceof check",
                rt.describe(&other)
            )))
        }
    };
    let mut cur = rt.get_prototype_of(*o);
    while let Some(x) = cur {
        if x == p {
            return Ok(true);
        }
        cur = rt.get_prototype_of(x);
    }
    Ok(false)
}

fn fp_call(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    if !rt.is_callable(c.this) {
        return Err(not_a_function(rt, "Function.prototype.call"));
    }
    let this_arg = c.arg(0);
    rt.call(c.this, &this_arg, c.args.get(1..).unwrap_or(&[]))
}

fn fp_apply(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    if !rt.is_callable(c.this) {
        return Err(not_a_function(rt, "Function.prototype.apply"));
    }
    let arr = c.arg(1);
    let args = if arr.is_nullish() { Vec::new() } else { list_from_array_like(rt, &arr)? };
    rt.call(c.this, &c.arg(0), &args)
}

/// [[Call]] of a bound function: the target with the bound this and the bound arguments first.
fn bound_call(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (target, this, mut args) = bound_parts(rt, c.callee).expect("bound function without its record (framework bug)");
    args.extend_from_slice(c.args);
    rt.call(&Value::Object(target), &this, &args)
}

/// [[Construct]] of a bound function (ECMA-262 10.4.1.2): NewTarget becomes the target when it is the bound function.
fn bound_construct(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (target, _, mut args) = bound_parts(rt, c.callee).expect("bound function without its record (framework bug)");
    args.extend_from_slice(c.args);
    let nt = match c.new_target {
        Value::Object(n) if *n == c.callee => Value::Object(target),
        other => other.clone(),
    };
    rt.construct(&Value::Object(target), &args, &nt)
}

/// Function.prototype.bind (ECMA-262 20.2.3.2). The bound function is a constructor exactly when the target is.
/// Its `length` is max(0, targetLength - boundArgs) and its name is "bound " followed by the target's name.
fn fp_bind(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    if !rt.is_callable(c.this) {
        return Err(rt.throw_type_error("Bind must be called on a function"));
    }
    let Value::Object(target) = *c.this else {
        return Err(rt.throw_type_error("Bind must be called on a function"));
    };
    let tv = Value::Object(target);
    let this_arg = c.arg(0);
    let bound_args: Vec<Value> = c.args.get(1..).unwrap_or(&[]).to_vec();
    let argc = bound_args.len() as f64;
    let f = if rt.is_constructor(&tv) {
        rt.new_native_constructor(bound_call, bound_construct, "", 0, Vec::new())
    } else {
        rt.new_native_function(bound_call, "", 0, Vec::new())
    };
    let proto = rt.get_prototype_of(target);
    rt.with_obj_mut(f, |o| o.proto = proto);
    rt.set_ext(f, Rc::new(BoundFn { target, this: this_arg, args: bound_args }));

    let key_length = PropKey::from_str("length");
    let mut len = 0.0;
    if rt.has_own_property(target, &key_length)? {
        if let Value::Number(n) = rt.get(target, &key_length, &tv)? {
            // ToIntegerOrInfinity maps NaN to 0; +Infinity stays infinite, -Infinity clamps to 0 below.
            len = (rt.to_integer_or_infinity(&Value::Number(n))? - argc).max(0.0);
        }
    }
    rt.define_builtin(f, key_length, Prop::Data { value: Value::Number(len), flags: Flags::CONFIG });

    let base = match rt.get(target, &PropKey::from_str("name"), &tv)? {
        Value::String(s) => s,
        _ => JsStr::empty(),
    };
    let full = JsStr::from_str("bound ").concat(&base);
    rt.define_builtin(f, PropKey::from_str("name"), Prop::Data { value: Value::String(full), flags: Flags::CONFIG });
    Ok(Value::Object(f))
}

/// Function.prototype.toString (ECMA-262 20.2.3.5) in the native shape "function name() { [native code] }". Compiled
/// functions have no source text in the kernel, so they print the same shape. A bound function has an empty name
/// here and prints "function () { [native code] }", as V8 does.
fn fp_to_string(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Value::Object(f) = *c.this else {
        return Err(rt.throw_type_error("Function.prototype.toString requires that 'this' be a Function"));
    };
    if !rt.is_callable(c.this) {
        return Err(rt.throw_type_error("Function.prototype.toString requires that 'this' be a Function"));
    }
    let name = rt.with_obj(f, |o| match &o.kind {
        ObjectKind::Function(fd) => fd.name.clone(),
        _ => JsStr::empty(),
    });
    let text = JsStr::from_str("function ").concat(&name).concat(&JsStr::from_str("() { [native code] }"));
    Ok(Value::String(text))
}

/// Function.prototype[@@hasInstance] (ECMA-262 20.2.3.6): OrdinaryHasInstance(this, V).
fn fp_has_instance(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(ordinary_has_instance(rt, c.this, &c.arg(0))?))
}

pub fn install(rt: &Rt) {
    let fp = rt.intrinsic(Intrinsic::FunctionPrototype);
    rt.define_native_method(fp, "apply", fp_apply, 2);
    rt.define_native_method(fp, "bind", fp_bind, 1);
    rt.define_native_method(fp, "call", fp_call, 1);
    rt.define_native_method(fp, "toString", fp_to_string, 0);
    let has_instance = rt.new_native_function(fp_has_instance, "[Symbol.hasInstance]", 1, Vec::new());
    rt.define_builtin(
        fp,
        PropKey::Sym(rt.well_known(WellKnown::HasInstance)),
        Prop::Data { value: Value::Object(has_instance), flags: Flags::NONE },
    );
}
