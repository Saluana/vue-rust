//! Host payloads (`NativeObject`) and exotic-object hooks (`Exotic`). Both are stored as `Rc<dyn _>`
//! inside heap objects and must report every `ObjRef` they hold through `trace`; forgetting it lets the
//! collector free live objects. Hooks run with no RefCell borrow held, so they may re-enter the runtime.

use std::any::Any;

use crate::object::Prop;
use crate::roots::Tracer;
use crate::rt::Rt;
use crate::value::{ObjRef, PropKey, Res, Value};

/// Host-private payload of an `ObjectKind::Native` object or of an object's extension slot.
pub trait NativeObject: Any {
    fn trace(&self, t: &mut Tracer);
}

/// Outcome of an exotic hook. `Ordinary` runs the default algorithm for this object.
#[derive(Clone, Debug, PartialEq)]
pub enum Hook<T> {
    Ordinary,
    Handled(T),
}

/// Proxy-like behaviour for an object (used by the reactivity port). Every method has an ordinary
/// default: `Ok(Hook::Ordinary)`. Hooks may call back into `rt`; they must not hold borrows across calls.
pub trait Exotic {
    fn trace(&self, t: &mut Tracer);

    fn get(&self, _rt: &Rt, _obj: ObjRef, _key: &PropKey, _receiver: &Value) -> Res<Hook<Value>> {
        Ok(Hook::Ordinary)
    }

    fn set(&self, _rt: &Rt, _obj: ObjRef, _key: &PropKey, _v: &Value, _receiver: &Value) -> Res<Hook<bool>> {
        Ok(Hook::Ordinary)
    }

    fn has(&self, _rt: &Rt, _obj: ObjRef, _key: &PropKey) -> Res<Hook<bool>> {
        Ok(Hook::Ordinary)
    }

    fn delete(&self, _rt: &Rt, _obj: ObjRef, _key: &PropKey) -> Res<Hook<bool>> {
        Ok(Hook::Ordinary)
    }

    fn own_keys(&self, _rt: &Rt, _obj: ObjRef) -> Res<Hook<Vec<PropKey>>> {
        Ok(Hook::Ordinary)
    }

    fn get_own_property(&self, _rt: &Rt, _obj: ObjRef, _key: &PropKey) -> Res<Hook<Option<Prop>>> {
        Ok(Hook::Ordinary)
    }

    fn define(&self, _rt: &Rt, _obj: ObjRef, _key: &PropKey, _prop: &Prop) -> Res<Hook<bool>> {
        Ok(Hook::Ordinary)
    }

    /// `[[PreventExtensions]]` of the object. `Hook::Ordinary` clears the object's own extensible flag.
    fn prevent_extensions(&self, _rt: &Rt, _obj: ObjRef) -> Res<Hook<bool>> {
        Ok(Hook::Ordinary)
    }

    /// `[[IsExtensible]]` of the object. `Hook::Ordinary` reads the object's own extensible flag.
    fn is_extensible(&self, _rt: &Rt, _obj: ObjRef) -> Res<Hook<bool>> {
        Ok(Hook::Ordinary)
    }

    /// `Array.isArray` for the object this hook belongs to: a proxy of an array answers true. Only
    /// `Rt::is_array` reads it; element storage and the array algorithms do not change (ABI.md section 13).
    fn is_array(&self) -> bool {
        false
    }
}

/// Borrows a native payload as its concrete type, or `None` when it is another type.
pub fn payload_ref<T: NativeObject>(n: &dyn NativeObject) -> Option<&T> {
    (n as &dyn Any).downcast_ref::<T>()
}
