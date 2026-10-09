//! Builtin family: iterator. The iterator protocol of ECMA-262 7.4 for the kernel and for the other builtin families:
//! GetIterator, IteratorStepValue, IteratorClose, the for-of driver, iterator result objects, %IteratorPrototype%,
//! and the native iterator objects (`NativeIter`) that Array, String, Map and Set build. The full protocol runs
//! everywhere; there are no fast paths.

use std::cell::Cell;
use std::rc::Rc;

use crate::function::Call;
use crate::jsstr::JsStr;
use crate::native::{payload_ref, NativeObject};
use crate::object::{Flags, Prop};
use crate::roots::Tracer;
use crate::rt::{Rt, WellKnown};
use crate::value::{ObjRef, PropKey, Res, Throw, Value};

/// An Iterator Record (ECMA-262 7.4.1). `done` is set once the iterator has finished or thrown. The values inside are
/// not GC roots: builtins run at call depth >= 1, where no collection happens, but host code that keeps a record
/// across `collect` must root `iterator` and `next` itself.
#[derive(Clone, Debug)]
pub struct IterRecord {
    pub iterator: Value,
    pub next: Value,
    pub done: bool,
}

/// GetIterator(v, sync) (ECMA-262 7.4.3). `v[Symbol.iterator]` is read once, and `next` is read once and kept in the
/// record. A primitive without an iterator method (for example a string before B3 installs one) is not iterable.
pub fn get_iterator(rt: &Rt, v: &Value) -> Res<IterRecord> {
    if v.is_nullish() {
        return Err(not_iterable(rt, v));
    }
    let method = rt.get_v(v, &PropKey::Sym(rt.well_known(WellKnown::Iterator)))?;
    if method.is_nullish() || !rt.is_callable(&method) {
        return Err(not_iterable(rt, v));
    }
    let iterator = rt.call(&method, v, &[])?;
    if !iterator.is_object() {
        return Err(rt.throw_type_error("Result of the Symbol.iterator method is not an object"));
    }
    let next = rt.get_v(&iterator, &PropKey::from_str("next"))?;
    Ok(IterRecord { iterator, next, done: false })
}

fn not_iterable(rt: &Rt, v: &Value) -> Throw {
    rt.throw_type_error(&format!("{} is not iterable", rt.describe(v)))
}

/// IteratorStepValue (ECMA-262 7.4.8): the next value, or `None` once the iterator is done. Any throw (from `next`,
/// from a non-object result, or from reading `done` or `value`) marks the record done, so it must not be closed.
/// Stepping a record that is already done returns `None` without calling `next`.
pub fn iter_step(rt: &Rt, rec: &mut IterRecord) -> Res<Option<Value>> {
    if rec.done {
        return Ok(None);
    }
    match step_value(rt, rec) {
        Ok(Some(v)) => Ok(Some(v)),
        Ok(None) => {
            rec.done = true;
            Ok(None)
        }
        Err(e) => {
            rec.done = true;
            Err(e)
        }
    }
}

/// IteratorNext, IteratorComplete (`done` is read before `value`) and IteratorValue, in that order.
fn step_value(rt: &Rt, rec: &IterRecord) -> Res<Option<Value>> {
    let result = rt.call(&rec.next, &rec.iterator, &[])?;
    let Some(r) = result.as_object() else {
        return Err(rt.throw_type_error(&format!("Iterator result {} is not an object", rt.describe(&result))));
    };
    if rt.get(r, &PropKey::from_str("done"), &result)?.to_boolean() {
        return Ok(None);
    }
    Ok(Some(rt.get(r, &PropKey::from_str("value"), &result)?))
}

/// IteratorClose (ECMA-262 7.4.11) for a sync iterator. With `completion_is_throw` the caller returns its own error:
/// `return` is still looked up and called, but its errors are swallowed. With a normal completion, errors from `return`
/// propagate and a non-object result is a TypeError. Callers do not close a record that is done.
pub fn iter_close(rt: &Rt, rec: &IterRecord, completion_is_throw: bool) -> Res<()> {
    let inner = call_return(rt, rec);
    if completion_is_throw {
        return Ok(());
    }
    if let Some(result) = inner? {
        if !result.is_object() {
            return Err(rt.throw_type_error(&format!("Iterator result {} is not an object", rt.describe(&result))));
        }
    }
    Ok(())
}

/// GetMethod(iterator, "return") followed by the call. `None` when there is no `return` (undefined or null).
fn call_return(rt: &Rt, rec: &IterRecord) -> Res<Option<Value>> {
    let ret = rt.get_v(&rec.iterator, &PropKey::from_str("return"))?;
    if ret.is_nullish() {
        return Ok(None);
    }
    if !rt.is_callable(&ret) {
        return Err(rt.throw_type_error(&format!("{} is not a function", rt.describe(&ret))));
    }
    rt.call(&ret, &rec.iterator, &[]).map(Some)
}

/// The for-of driver. `f` gets each value and returns `Ok(true)` to continue or `Ok(false)` to stop early; stopping
/// closes the iterator with a normal completion. An `Err` from `f` closes it with a throw completion and propagates
/// that error. Errors from `next` propagate without closing the iterator.
pub fn iterate(rt: &Rt, v: &Value, f: &mut dyn FnMut(Value) -> Res<bool>) -> Res<()> {
    let mut rec = get_iterator(rt, v)?;
    while let Some(x) = iter_step(rt, &mut rec)? {
        match f(x) {
            Ok(true) => {}
            Ok(false) => return iter_close(rt, &rec, false),
            Err(e) => {
                let _ = iter_close(rt, &rec, true);
                return Err(e);
            }
        }
    }
    Ok(())
}

/// CreateIterResultObject (ECMA-262 7.4.14): `{ value, done }`, with the properties in that order.
pub fn create_iter_result(rt: &Rt, value: Value, done: bool) -> Value {
    let o = rt.new_object();
    rt.define_builtin(o, PropKey::from_str("value"), Prop::Data { value, flags: Flags::ALL });
    rt.define_builtin(o, PropKey::from_str("done"), Prop::Data { value: Value::Bool(done), flags: Flags::ALL });
    Value::Object(o)
}

/// Installs %IteratorPrototype% (registered as "IteratorPrototype") with `[Symbol.iterator]() { return this }`.
/// It depends on no other family.
pub fn install(rt: &Rt) {
    let proto = rt.new_object();
    rt.register_builtin("IteratorPrototype", proto);
    let f = rt.new_native_function(iterator_self, "[Symbol.iterator]", 0, Vec::new());
    rt.define_builtin(
        proto,
        PropKey::Sym(rt.well_known(WellKnown::Iterator)),
        Prop::Data { value: Value::Object(f), flags: Flags::HIDDEN },
    );
}

fn iterator_self(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(c.this.clone())
}

/// %IteratorPrototype%. Panics if called before `Rt::new` has installed it (a framework bug).
pub fn iterator_prototype(rt: &Rt) -> ObjRef {
    rt.builtin("IteratorPrototype").expect("%IteratorPrototype% is installed by Rt::new (framework bug)")
}

/// A builtin iterator's state (Array, String, Map and Set iterators). `next` returns the next value, or `None` once the
/// iterator is exhausted. The wrapper never calls `next` again after `None` or after an error. Implementations keep
/// their mutable state in `Cell` or `RefCell` and hold no borrow across a call into user code.
pub trait NativeIter: NativeObject {
    fn next(&self, rt: &Rt) -> Res<Option<Value>>;
}

/// The host payload of an iterator object: its family (the registry name of its prototype), whether it has finished,
/// and the state.
struct IterPayload {
    family: JsStr,
    finished: Cell<bool>,
    inner: Rc<dyn NativeIter>,
}

impl NativeObject for IterPayload {
    fn trace(&self, t: &mut Tracer) {
        self.inner.trace(t);
    }
}

/// Creates the prototype of one builtin iterator family, registered as `registry_name`. Its `[[Prototype]]` is
/// %IteratorPrototype%; it has an own `next` (which accepts only iterators of this family) and `[Symbol.toStringTag]`.
pub fn make_iterator_prototype(rt: &Rt, registry_name: &'static str, tag: &'static str) -> ObjRef {
    let proto = rt.new_object_with_proto(Some(iterator_prototype(rt)));
    let next = rt.new_native_function(native_next, "next", 0, vec![Value::str(registry_name)]);
    let next_prop = Prop::Data { value: Value::Object(next), flags: Flags::HIDDEN };
    rt.define_builtin(proto, PropKey::from_str("next"), next_prop);
    rt.define_builtin(
        proto,
        PropKey::Sym(rt.well_known(WellKnown::ToStringTag)),
        Prop::Data { value: Value::str(tag), flags: Flags::CONFIG },
    );
    rt.register_builtin(registry_name, proto);
    proto
}

/// A builtin iterator object whose prototype is the one registered as `proto_registry_name`. Panics if no such
/// prototype exists (a framework bug).
pub fn new_iterator_object(rt: &Rt, proto_registry_name: &str, payload: Rc<dyn NativeIter>) -> ObjRef {
    let Some(proto) = rt.builtin(proto_registry_name) else {
        panic!("no iterator prototype registered as {proto_registry_name} (framework bug)");
    };
    let state: Rc<dyn NativeObject> = Rc::new(IterPayload {
        family: JsStr::from_str(proto_registry_name),
        finished: Cell::new(false),
        inner: payload,
    });
    rt.new_native_object(Some(proto), state)
}

/// `next` of a builtin iterator prototype. The receiver must be an iterator of this family (its name is the function's
/// closure data); anything else is a TypeError.
fn native_next(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let family = rt.fn_data(c.callee)[0].clone();
    let state = match c.this {
        Value::Object(o) => rt.native_payload(*o),
        _ => None,
    };
    let Some(state) = state else {
        return Err(incompatible(rt, c.this));
    };
    let Some(p) = payload_ref::<IterPayload>(&*state) else {
        return Err(incompatible(rt, c.this));
    };
    if family != Value::String(p.family.clone()) {
        return Err(incompatible(rt, c.this));
    }
    if p.finished.get() {
        return Ok(create_iter_result(rt, Value::Undefined, true));
    }
    match p.inner.next(rt) {
        Ok(Some(v)) => Ok(create_iter_result(rt, v, false)),
        Ok(None) => {
            p.finished.set(true);
            Ok(create_iter_result(rt, Value::Undefined, true))
        }
        Err(e) => {
            p.finished.set(true);
            Err(e)
        }
    }
}

fn incompatible(rt: &Rt, this: &Value) -> Throw {
    rt.throw_type_error(&format!("next method called on incompatible receiver {}", rt.describe(this)))
}
