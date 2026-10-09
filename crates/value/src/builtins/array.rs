//! Builtin family: Array (WP-B2, ECMA-262 23.1; V8 is the oracle where they differ). This file holds the constructor,
//! the statics, the array-like helpers shared with `array/proto.rs`, and the array iterators (`NativeIter` objects of
//! `%ArrayIteratorPrototype%`). Prototype methods are in `array/proto.rs`, the sort in `array/sort.rs`. ABI.md
//! section 16 documents the public helpers and the deviations.

use std::cell::Cell;
use std::rc::Rc;

use crate::builtins::iterator::{
    iter_close, iter_step, iterate, make_iterator_prototype, new_iterator_object, IterRecord, NativeIter,
};
use crate::function::Call;
use crate::jsstr::JsStr;
use crate::native::NativeObject;
use crate::number;
use crate::object::{Flags, Obj, ObjectKind, Prop, MAX_DENSE_LEN};
use crate::roots::Tracer;
use crate::rt::{Intrinsic, Rt, WellKnown};
use crate::value::{ObjRef, PropKey, Res, Value};

mod proto;
mod sort;

/// 2^53 - 1: the largest length of an array-like (ECMA-262 7.1.20).
pub(crate) const MAX_SAFE: f64 = 9_007_199_254_740_991.0;

/// The key of index `n` (an integer in [0, 2^53 - 1]). Indices from 2^32 - 1 on are string keys, not array indices.
pub(crate) fn key_of(n: f64) -> PropKey {
    if n < 4_294_967_295.0 {
        PropKey::Index(n as u32)
    } else {
        PropKey::from_jsstr(JsStr::from_str(&number::number_to_string(n)))
    }
}

/// The element at `k` when it is an own element of a dense array (no user code runs). Holes give `None`.
fn own_elem(rt: &Rt, o: ObjRef, k: f64) -> Option<Value> {
    if k >= 4_294_967_295.0 {
        return None;
    }
    let i = k as usize;
    rt.with_obj(o, |ob| match &ob.kind {
        ObjectKind::Array(v) => match v.get(i) {
            Some(x) if !matches!(x, Value::Hole) => Some(x.clone()),
            _ => None,
        },
        _ => None,
    })
}

/// Get(O, k).
pub(crate) fn get_at(rt: &Rt, o: ObjRef, k: f64) -> Res<Value> {
    if let Some(v) = own_elem(rt, o, k) {
        return Ok(v);
    }
    rt.get(o, &key_of(k), &Value::Object(o))
}

/// HasProperty(O, k) then Get(O, k): `None` when the index is absent (a hole consults the prototype chain).
pub(crate) fn get_present(rt: &Rt, o: ObjRef, k: f64) -> Res<Option<Value>> {
    if let Some(v) = own_elem(rt, o, k) {
        return Ok(Some(v));
    }
    let key = key_of(k);
    if !rt.has_property(o, &key)? {
        return Ok(None);
    }
    Ok(Some(rt.get(o, &key, &Value::Object(o))?))
}

/// Set(O, k, v, true).
pub(crate) fn set_at(rt: &Rt, o: ObjRef, k: f64, v: Value) -> Res<()> {
    rt.set_strict(o, &key_of(k), v)
}

/// CreateDataPropertyOrThrow(O, k, v).
pub(crate) fn create_at(rt: &Rt, o: ObjRef, k: f64, v: Value) -> Res<()> {
    let key = key_of(k);
    if rt.define_own(o, &key, Prop::Data { value: v, flags: Flags::ALL })? {
        return Ok(());
    }
    Err(rt.throw_type_error(&format!("Cannot add property {}, object is not extensible", rt.key_text(&key))))
}

/// DeletePropertyOrThrow(O, k).
pub(crate) fn delete_at(rt: &Rt, o: ObjRef, k: f64) -> Res<()> {
    let key = key_of(k);
    if rt.delete(o, &key)? {
        return Ok(());
    }
    Err(rt.throw_type_error(&format!("Cannot delete property '{}' of [object Array]", rt.key_text(&key))))
}

/// Set(O, "length", n, true).
pub(crate) fn set_len(rt: &Rt, o: ObjRef, n: f64) -> Res<()> {
    rt.set_strict(o, &PropKey::from_str("length"), Value::Number(n))
}

/// LengthOfArrayLike(O): the kernel's length for a real array, else ToLength(Get(O, "length")).
pub(crate) fn len_of(rt: &Rt, o: ObjRef) -> Res<f64> {
    if let Some(n) = rt.with_obj(o, |ob| match &ob.kind {
        ObjectKind::Array(v) => Some(v.len() as f64),
        _ => None,
    }) {
        return Ok(n);
    }
    let v = rt.get(o, &PropKey::from_str("length"), &Value::Object(o))?;
    rt.to_length(&v)
}

/// `ToObject(this)` for method `name`: a TypeError for undefined and null.
pub(crate) fn this_obj(rt: &Rt, c: &Call<'_>, name: &str) -> Res<ObjRef> {
    if c.this.is_nullish() {
        return Err(rt.throw_type_error(&format!("Array.prototype.{name} called on null or undefined")));
    }
    rt.to_object(c.this)
}

/// IsCallable check that returns the value (a TypeError otherwise).
pub(crate) fn callable(rt: &Rt, v: &Value) -> Res<Value> {
    if rt.is_callable(v) {
        return Ok(v.clone());
    }
    Err(rt.throw_type_error(&format!("{} is not a function", rt.describe(v))))
}

/// A relative index clamped to [0, len] (`start`/`end` handling of ECMA-262 23.1.3).
pub(crate) fn clamp_rel(r: f64, len: f64) -> f64 {
    if r < 0.0 { (len + r).max(0.0) } else { r.min(len) }
}

/// ToIntegerOrInfinity of argument `i` (undefined gives 0).
pub(crate) fn int_arg(rt: &Rt, c: &Call<'_>, i: usize) -> Res<f64> {
    rt.to_integer_or_infinity(&c.arg(i))
}

/// A RangeError when a new array of `n` elements would exceed the dense limit (a deliberate deviation: V8 allows
/// sparse arrays up to 2^32 - 1, see ABI.md section 16).
pub(crate) fn check_dense(rt: &Rt, n: f64) -> Res<()> {
    if n > MAX_DENSE_LEN as f64 {
        return Err(rt.throw_range_error("Invalid array length"));
    }
    Ok(())
}

/// A plain array (ArraySpeciesCreate without Symbol.species: results are always plain arrays).
pub(crate) fn make_array(rt: &Rt, elems: Vec<Value>, proto: Option<ObjRef>) -> ObjRef {
    match proto {
        None => rt.new_array(elems),
        Some(p) => rt.alloc(Obj::new(Some(p), ObjectKind::Array(elems))),
    }
}

// ----- constructor and statics -----

fn array_call(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    construct_array(rt, c.args, None)
}

fn array_construct(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let proto = proto_from_ctor(rt, c.new_target)?;
    construct_array(rt, c.args, Some(proto))
}

/// GetPrototypeFromConstructor(newTarget, %Array.prototype%).
fn proto_from_ctor(rt: &Rt, new_target: &Value) -> Res<ObjRef> {
    if new_target.is_object() {
        if let Value::Object(p) = rt.get_str(new_target, "prototype")? {
            return Ok(p);
        }
    }
    Ok(rt.intrinsic(Intrinsic::ArrayPrototype))
}

/// `Array(...items)`: one number makes a holey array of that length, anything else makes the elements.
fn construct_array(rt: &Rt, args: &[Value], proto: Option<ObjRef>) -> Res<Value> {
    let elems = match args {
        [Value::Number(n)] => {
            let len = number::to_uint32(*n);
            if f64::from(len) != *n || len as usize > MAX_DENSE_LEN {
                return Err(rt.throw_range_error("Invalid array length"));
            }
            vec![Value::Hole; len as usize]
        }
        _ => args.to_vec(),
    };
    Ok(Value::Object(make_array(rt, elems, proto)))
}

fn object_or_throw(rt: &Rt, v: Value) -> Res<ObjRef> {
    match v {
        Value::Object(o) => Ok(o),
        _ => Err(rt.throw_type_error("constructor did not return an object")),
    }
}

fn is_array(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Bool(rt.is_array(&c.arg(0))))
}

/// `Array.of(...items)` (ECMA-262 23.1.2.3).
fn array_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let n = c.args.len() as f64;
    let a = if rt.is_constructor(c.this) {
        object_or_throw(rt, rt.construct(c.this, &[Value::Number(n)], c.this)?)?
    } else {
        check_dense(rt, n)?;
        rt.new_array(vec![Value::Hole; n as usize])
    };
    for (k, v) in c.args.iter().enumerate() {
        create_at(rt, a, k as f64, v.clone())?;
    }
    set_len(rt, a, n)?;
    Ok(Value::Object(a))
}

/// `Array.from(items, mapFn, thisArg)` (ECMA-262 23.1.2.1): the iteration protocol when `items` has
/// `Symbol.iterator`, else an array-like read through `length`. Strings iterate by code point through
/// `String.prototype[Symbol.iterator]`.
fn array_from(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let ctor = c.this.clone();
    let items = c.arg(0);
    let mapfn = c.arg(1);
    let this_arg = c.arg(2);
    let mapping = !mapfn.is_undefined();
    if mapping {
        callable(rt, &mapfn)?;
    }
    let iter_method = rt.get_v(&items, &PropKey::Sym(rt.well_known(WellKnown::Iterator)))?;
    if !iter_method.is_nullish() {
        callable(rt, &iter_method)?;
        let a = if rt.is_constructor(&ctor) {
            object_or_throw(rt, rt.construct(&ctor, &[], &ctor)?)?
        } else {
            rt.new_array(Vec::new())
        };
        let iterator = rt.call(&iter_method, &items, &[])?;
        if !iterator.is_object() {
            return Err(rt.throw_type_error("Result of the Symbol.iterator method is not an object"));
        }
        let next = rt.get_str(&iterator, "next")?;
        let mut rec = IterRecord { iterator, next, done: false };
        let mut k = 0.0;
        while let Some(v) = iter_step(rt, &mut rec)? {
            let r = if mapping {
                match rt.call(&mapfn, &this_arg, &[v, Value::Number(k)]) {
                    Ok(x) => x,
                    Err(e) => {
                        let _ = iter_close(rt, &rec, true);
                        return Err(e);
                    }
                }
            } else {
                v
            };
            if let Err(e) = create_at(rt, a, k, r) {
                let _ = iter_close(rt, &rec, true);
                return Err(e);
            }
            k += 1.0;
        }
        set_len(rt, a, k)?;
        return Ok(Value::Object(a));
    }
    let src = rt.to_object(&items)?;
    let len = len_of(rt, src)?;
    let a = if rt.is_constructor(&ctor) {
        object_or_throw(rt, rt.construct(&ctor, &[Value::Number(len)], &ctor)?)?
    } else {
        check_dense(rt, len)?;
        rt.new_array(vec![Value::Hole; len as usize])
    };
    let mut k = 0.0;
    while k < len {
        let v = get_at(rt, src, k)?;
        let r = if mapping { rt.call(&mapfn, &this_arg, &[v, Value::Number(k)])? } else { v };
        create_at(rt, a, k, r)?;
        k += 1.0;
        rt.tick()?;
    }
    set_len(rt, a, len)?;
    Ok(Value::Object(a))
}

/// Collects an iterable into a new array through the iteration protocol (`iterate` of builtins/iterator.rs). Used by
/// the other families and the compiler for spread of an iterable into an array.
pub fn array_from_iterable(rt: &Rt, v: &Value) -> Res<ObjRef> {
    let mut out: Vec<Value> = Vec::new();
    iterate(rt, v, &mut |x| {
        if out.len() >= MAX_DENSE_LEN {
            return Err(rt.throw_range_error("Invalid array length"));
        }
        out.push(x);
        Ok(true)
    })?;
    Ok(rt.new_array(out))
}

// ----- array iterators (%ArrayIteratorPrototype%, ECMA-262 23.1.5) -----

/// What an array iterator yields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IterKind {
    Keys,
    Values,
    Entries,
}

/// The state of an array iterator. `target` becomes `None` once the iterator is exhausted (sticky). The length is
/// read on every step, so the iterator sees elements added or removed during iteration.
struct ArrayIter {
    target: Cell<Option<ObjRef>>,
    kind: IterKind,
    index: Cell<f64>,
}

impl NativeObject for ArrayIter {
    fn trace(&self, t: &mut Tracer) {
        if let Some(o) = self.target.get() {
            t.mark(o);
        }
    }
}

impl NativeIter for ArrayIter {
    fn next(&self, rt: &Rt) -> Res<Option<Value>> {
        let Some(o) = self.target.get() else {
            return Ok(None);
        };
        let len = len_of(rt, o)?;
        let i = self.index.get();
        if i >= len {
            self.target.set(None);
            return Ok(None);
        }
        self.index.set(i + 1.0);
        let v = match self.kind {
            IterKind::Keys => Value::Number(i),
            IterKind::Values => get_at(rt, o, i)?,
            IterKind::Entries => {
                let e = get_at(rt, o, i)?;
                Value::Object(rt.new_array(vec![Value::Number(i), e]))
            }
        };
        Ok(Some(v))
    }
}

/// CreateArrayIterator(O, kind) over the (already converted) object `o`.
pub(crate) fn create_array_iterator(rt: &Rt, o: ObjRef, kind: IterKind) -> Value {
    let it: Rc<dyn NativeIter> = Rc::new(ArrayIter { target: Cell::new(Some(o)), kind, index: Cell::new(0.0) });
    Value::Object(new_iterator_object(rt, "ArrayIteratorPrototype", it))
}

/// Installs `Array`, its statics, `Array.prototype` (through `array/proto.rs`) and %ArrayIteratorPrototype%.
pub fn install(rt: &Rt) {
    let ap = rt.intrinsic(Intrinsic::ArrayPrototype);
    make_iterator_prototype(rt, "ArrayIteratorPrototype", "Array Iterator");
    let ctor = rt.new_native_constructor(array_call, array_construct, "Array", 1, Vec::new());
    rt.define_builtin(ctor, PropKey::from_str("prototype"), Prop::Data { value: Value::Object(ap), flags: Flags::NONE });
    rt.define_builtin(ap, PropKey::from_str("constructor"), Prop::Data { value: Value::Object(ctor), flags: Flags::HIDDEN });
    rt.define_native_method(ctor, "isArray", is_array, 1);
    rt.define_native_method(ctor, "of", array_of, 0);
    rt.define_native_method(ctor, "from", array_from, 1);
    proto::install(rt, ap);
    rt.install_global("Array", Value::Object(ctor));
}
