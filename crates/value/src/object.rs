//! Object model: `Obj` records, property storage (`PropertyMap`), attributes (`Flags`, `Prop`) and object
//! kinds. The property algorithms (get, set, define, delete, own keys, integrity levels) are `impl Rt`
//! blocks at the end of this file. Ordinary objects keep every property in the `PropertyMap`; an array
//! keeps its indices in the dense `Vec` and only its named properties in the map.

use std::collections::HashMap;
use std::rc::Rc;

use crate::function::FunctionData;
use crate::jsstr::JsStr;
use crate::heap::{BASE_BYTES, ELEM_BYTES, PROP_BYTES};
use crate::native::{Exotic, Hook, NativeObject};
use crate::rt::Rt;
use crate::number;
use crate::roots::Tracer;
use crate::value::{same_value, ObjRef, PropKey, Res, Value};

/// Largest dense array length (holes included). Growth past it throws a RangeError instead of allocating.
pub const MAX_DENSE_LEN: usize = 1 << 22;

/// Property attributes. For accessors `writable` is always false.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
}

impl Flags {
    pub const NONE: Flags = Flags::new(false, false, false);
    pub const ALL: Flags = Flags::new(true, true, true);
    /// Writable and configurable, not enumerable: the attributes of built-in methods.
    pub const HIDDEN: Flags = Flags::new(true, false, true);
    /// Configurable only: function `name` and `length`.
    pub const CONFIG: Flags = Flags::new(false, false, true);

    pub const fn new(writable: bool, enumerable: bool, configurable: bool) -> Flags {
        Flags { writable, enumerable, configurable }
    }
}

/// A own property as stored: a data property or an accessor pair.
#[derive(Clone, Debug, PartialEq)]
pub enum Prop {
    Data { value: Value, flags: Flags },
    Accessor { get: Option<ObjRef>, set: Option<ObjRef>, flags: Flags },
}

impl Prop {
    pub fn flags(&self) -> Flags {
        match self {
            Prop::Data { flags, .. } | Prop::Accessor { flags, .. } => *flags,
        }
    }

    pub fn is_accessor(&self) -> bool {
        matches!(self, Prop::Accessor { .. })
    }
}

/// Named-property storage in insertion order, with a hash index once it has more than a few entries.
#[derive(Clone, Debug, Default)]
pub struct PropertyMap {
    entries: Vec<(PropKey, Prop)>,
    index: Option<HashMap<PropKey, usize>>,
}

const INDEX_AFTER: usize = 8;

impl PropertyMap {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn find(&self, key: &PropKey) -> Option<usize> {
        match &self.index {
            Some(ix) => ix.get(key).copied(),
            None => self.entries.iter().position(|(k, _)| k == key),
        }
    }

    pub fn get(&self, key: &PropKey) -> Option<&Prop> {
        self.find(key).map(|i| &self.entries[i].1)
    }

    pub fn get_mut(&mut self, key: &PropKey) -> Option<&mut Prop> {
        let i = self.find(key)?;
        Some(&mut self.entries[i].1)
    }

    /// Appends a property. The key must not be present.
    pub fn push(&mut self, key: PropKey, prop: Prop) {
        debug_assert!(self.find(&key).is_none(), "PropertyMap::push on an existing key");
        if let Some(ix) = &mut self.index {
            ix.insert(key.clone(), self.entries.len());
        }
        self.entries.push((key, prop));
        if self.index.is_none() && self.entries.len() > INDEX_AFTER {
            self.rebuild_index();
        }
    }

    /// Removes a property. Removal is O(n) because it keeps insertion order.
    pub fn remove(&mut self, key: &PropKey) -> Option<Prop> {
        let i = self.find(key)?;
        let (_, p) = self.entries.remove(i);
        if self.index.is_some() {
            self.rebuild_index();
        }
        Some(p)
    }

    fn rebuild_index(&mut self) {
        self.index = Some(self.entries.iter().enumerate().map(|(i, (k, _))| (k.clone(), i)).collect());
    }

    pub fn iter(&self) -> impl Iterator<Item = (&PropKey, &Prop)> {
        self.entries.iter().map(|(k, p)| (k, p))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&PropKey, &mut Prop)> {
        self.entries.iter_mut().map(|(k, p)| (&*k, p))
    }
}

/// What an object is. Compiled code and the runtime ports add variants through this enum.
pub enum ObjectKind {
    Ordinary,
    /// Dense elements: `Value::Hole` marks a missing index and the length is `vec.len()`.
    Array(Vec<Value>),
    Function(FunctionData),
    /// Closure variable. `Value::Uninit` marks the temporal dead zone.
    Cell(Value),
    /// Created by the Error constructors; carries own `message`, `stack` and optional `cause`.
    Error,
    /// `ToObject` of a primitive (String, Number, Boolean or Symbol); the value is the wrapped primitive. A String
    /// wrapper also has read-only `length` and index properties (`string_wrapper_own`).
    Wrapper(Value),
    Native(Rc<dyn NativeObject>),
    Exotic(Rc<dyn Exotic>),
}

/// Attributes shared by all dense elements of an array and the attributes of its `length`. `freeze` and
/// `seal` change them uniformly (per-element attributes are not representable; see ABI.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ArrAttrs {
    pub(crate) len_writable: bool,
    pub(crate) elem_writable: bool,
    pub(crate) elem_configurable: bool,
}

impl Default for ArrAttrs {
    fn default() -> Self {
        ArrAttrs { len_writable: true, elem_writable: true, elem_configurable: true }
    }
}

/// A heap object. Only `Heap` and `Rt` construct or mutate these.
pub struct Obj {
    pub(crate) proto: Option<ObjRef>,
    pub(crate) extensible: bool,
    pub(crate) arr: ArrAttrs,
    pub(crate) props: PropertyMap,
    pub(crate) kind: ObjectKind,
    pub(crate) ext: Option<Rc<dyn NativeObject>>,
}

impl Obj {
    pub(crate) fn new(proto: Option<ObjRef>, kind: ObjectKind) -> Obj {
        Obj {
            proto,
            extensible: true,
            arr: ArrAttrs::default(),
            props: PropertyMap::default(),
            kind,
            ext: None,
        }
    }

    /// Estimated bytes for heap accounting.
    pub(crate) fn estimate_bytes(&self) -> usize {
        let elems = match &self.kind {
            ObjectKind::Array(v) => v.len(),
            _ => 0,
        };
        BASE_BYTES + self.props.len() * PROP_BYTES + elems * ELEM_BYTES
    }

    pub(crate) fn exotic(&self) -> Option<Rc<dyn Exotic>> {
        match &self.kind {
            ObjectKind::Exotic(e) => Some(e.clone()),
            _ => None,
        }
    }

    /// Reports every `ObjRef` this object holds. Called by the collector's worklist, never recursively.
    pub(crate) fn trace(&self, t: &mut Tracer) {
        if let Some(p) = self.proto {
            t.mark(p);
        }
        for (_, p) in self.props.iter() {
            match p {
                Prop::Data { value, .. } => t.mark_value(value),
                Prop::Accessor { get, set, .. } => {
                    if let Some(g) = get {
                        t.mark(*g);
                    }
                    if let Some(s) = set {
                        t.mark(*s);
                    }
                }
            }
        }
        match &self.kind {
            ObjectKind::Ordinary | ObjectKind::Error => {}
            ObjectKind::Array(v) => {
                for x in v {
                    t.mark_value(x);
                }
            }
            ObjectKind::Function(f) => {
                for c in f.captures.iter() {
                    t.mark(*c);
                }
                for d in f.data.iter() {
                    t.mark_value(d);
                }
            }
            ObjectKind::Cell(v) => t.mark_value(v),
            ObjectKind::Wrapper(v) => t.mark_value(v),
            ObjectKind::Native(n) => n.trace(t),
            ObjectKind::Exotic(e) => e.trace(t),
        }
        if let Some(e) = &self.ext {
            e.trace(t);
        }
    }
}

// ----- property operations (ECMA-262 10.1 ordinary object internal methods, plus the array exotic) -----

/// "length" as UTF-16 code units.
const LENGTH: &[u16] = &[0x6C, 0x65, 0x6E, 0x67, 0x74, 0x68];

pub(crate) fn is_length(k: &PropKey) -> bool {
    matches!(k, PropKey::Str(s) if s.units() == LENGTH)
}

/// The own properties a String wrapper defines itself: each character index below its length (read-only and
/// enumerable) and `length` (read-only, not enumerable). ECMA-262 10.4.3 (String exotic objects).
fn string_wrapper_own(s: &JsStr, key: &PropKey) -> Option<Prop> {
    match key {
        PropKey::Index(i) if (*i as usize) < s.len() => {
            let i = *i as usize;
            Some(Prop::Data {
                value: Value::String(JsStr::from_units(&s.units()[i..i + 1])),
                flags: Flags::new(false, true, false),
            })
        }
        k if is_length(k) => Some(Prop::Data { value: Value::Number(s.len() as f64), flags: Flags::NONE }),
        _ => None,
    }
}

fn array_len(ob: &Obj) -> usize {
    match &ob.kind {
        ObjectKind::Array(v) => v.len(),
        _ => 0,
    }
}

fn set_elem(ob: &mut Obj, i: usize, v: Value) {
    if let ObjectKind::Array(elems) = &mut ob.kind {
        elems[i] = v;
    }
}

/// ValidateAndApplyPropertyDescriptor for a non-configurable existing property (ECMA-262 10.1.6.3).
/// A configurable property accepts any change.
fn can_replace(cur: &Prop, new: &Prop) -> bool {
    if cur.flags().configurable {
        return true;
    }
    if new.flags().configurable || new.flags().enumerable != cur.flags().enumerable {
        return false;
    }
    match (cur, new) {
        (Prop::Data { value: cv, flags: cf }, Prop::Data { value: nv, flags: nf }) => {
            if cf.writable {
                return true;
            }
            !nf.writable && same_value(cv, nv)
        }
        (Prop::Accessor { get: cg, set: cs, .. }, Prop::Accessor { get: ng, set: ns, .. }) => cg == ng && cs == ns,
        _ => false,
    }
}

impl Rt {
    /// Printable form of a key for diagnostics.
    pub(crate) fn key_text(&self, key: &PropKey) -> String {
        match key {
            PropKey::Index(i) => i.to_string(),
            PropKey::Str(s) => s.to_rust_string_lossy(),
            PropKey::Sym(s) => {
                let d = self.symbol_description(*s).map(|d| d.to_rust_string_lossy()).unwrap_or_default();
                format!("Symbol({d})")
            }
        }
    }

    fn exotic_of(&self, o: ObjRef) -> Option<Rc<dyn Exotic>> {
        self.with_obj(o, |ob| ob.exotic())
    }

    fn is_array_obj(&self, o: ObjRef) -> bool {
        self.with_obj(o, |ob| matches!(ob.kind, ObjectKind::Array(_)))
    }

    /// The own property of a String wrapper that is read-only and non-configurable (see `string_wrapper_own`).
    fn protected_string_prop(&self, o: ObjRef, key: &PropKey) -> Option<Prop> {
        self.with_obj(o, |ob| match &ob.kind {
            ObjectKind::Wrapper(Value::String(s)) => string_wrapper_own(s, key),
            _ => None,
        })
    }

    pub fn get_prototype_of(&self, o: ObjRef) -> Option<ObjRef> {
        self.with_obj(o, |ob| ob.proto)
    }

    pub fn is_extensible(&self, o: ObjRef) -> bool {
        self.with_obj(o, |ob| ob.extensible)
    }

    /// Own property from storage, without exotic hooks: dense elements, the array length, named properties.
    fn own_plain(&self, o: ObjRef, key: &PropKey) -> Option<Prop> {
        self.with_obj(o, |ob| match &ob.kind {
            ObjectKind::Array(elems) => match key {
                PropKey::Index(i) => match elems.get(*i as usize) {
                    Some(v) if !matches!(v, Value::Hole) => Some(Prop::Data {
                        value: v.clone(),
                        flags: Flags::new(ob.arr.elem_writable, true, ob.arr.elem_configurable),
                    }),
                    _ => None,
                },
                k if is_length(k) => Some(Prop::Data {
                    value: Value::Number(elems.len() as f64),
                    flags: Flags::new(ob.arr.len_writable, false, false),
                }),
                k => ob.props.get(k).cloned(),
            },
            ObjectKind::Wrapper(Value::String(s)) => {
                string_wrapper_own(s, key).or_else(|| ob.props.get(key).cloned())
            }
            _ => ob.props.get(key).cloned(),
        })
    }

    /// [[GetOwnProperty]]: the exotic hook first, then storage.
    pub fn get_own_property(&self, o: ObjRef, key: &PropKey) -> Res<Option<Prop>> {
        if let Some(ex) = self.exotic_of(o) {
            if let Hook::Handled(p) = ex.get_own_property(self, o, key)? {
                return Ok(p);
            }
        }
        Ok(self.own_plain(o, key))
    }

    /// [[Get]] (ECMA-262 10.1.8). Getters run with no borrow held. `receiver` is the `this` of a getter.
    pub fn get(&self, obj: ObjRef, key: &PropKey, receiver: &Value) -> Res<Value> {
        let mut cur = obj;
        loop {
            if let Some(ex) = self.exotic_of(cur) {
                if let Hook::Handled(v) = ex.get(self, cur, key, receiver)? {
                    return Ok(v);
                }
            }
            match self.get_own_property(cur, key)? {
                Some(Prop::Data { value, .. }) => return Ok(value),
                Some(Prop::Accessor { get, .. }) => {
                    return match get {
                        Some(g) => self.call(&Value::Object(g), receiver, &[]),
                        None => Ok(Value::Undefined),
                    };
                }
                None => match self.get_prototype_of(cur) {
                    Some(p) => cur = p,
                    None => return Ok(Value::Undefined),
                },
            }
        }
    }

    /// [[Set]] (ECMA-262 10.1.9, OrdinarySet). Returns false when the assignment is refused (read-only,
    /// no setter, non-extensible); the strict wrappers turn that into a TypeError.
    pub fn set(&self, obj: ObjRef, key: &PropKey, v: Value, receiver: &Value) -> Res<bool> {
        let mut cur = obj;
        let found = loop {
            if let Some(ex) = self.exotic_of(cur) {
                if let Hook::Handled(b) = ex.set(self, cur, key, &v, receiver)? {
                    return Ok(b);
                }
            }
            if let Some(p) = self.get_own_property(cur, key)? {
                break Some(p);
            }
            match self.get_prototype_of(cur) {
                Some(p) => cur = p,
                None => break None,
            }
        };
        match found {
            Some(Prop::Accessor { set, .. }) => match set {
                Some(s) => {
                    self.call(&Value::Object(s), receiver, &[v])?;
                    Ok(true)
                }
                None => Ok(false),
            },
            Some(Prop::Data { flags, .. }) if !flags.writable => Ok(false),
            _ => self.set_on_receiver(receiver, key, v),
        }
    }

    /// The last step of OrdinarySet: store `v` as a data property of the receiver.
    fn set_on_receiver(&self, receiver: &Value, key: &PropKey, v: Value) -> Res<bool> {
        let Value::Object(r) = receiver else { return Ok(false) };
        match self.get_own_property(*r, key)? {
            Some(Prop::Accessor { .. }) => Ok(false),
            Some(Prop::Data { flags, .. }) => {
                if !flags.writable {
                    return Ok(false);
                }
                self.define_own(*r, key, Prop::Data { value: v, flags })
            }
            None => self.define_own(*r, key, Prop::Data { value: v, flags: Flags::ALL }),
        }
    }

    /// [[DefineOwnProperty]] with a complete property (not a partial descriptor).
    pub fn define_own(&self, o: ObjRef, key: &PropKey, prop: Prop) -> Res<bool> {
        if let Some(ex) = self.exotic_of(o) {
            if let Hook::Handled(b) = ex.define(self, o, key, &prop)? {
                return Ok(b);
            }
        }
        if let Some(cur) = self.protected_string_prop(o, key) {
            // Read-only and non-configurable: a definition is accepted only when it changes nothing.
            return Ok(cur == prop);
        }
        if self.is_array_obj(o) {
            match key {
                PropKey::Index(i) => return self.array_define_index(o, *i, prop),
                k if is_length(k) => return self.array_define_length(o, prop),
                _ => {}
            }
        }
        Ok(self.ordinary_define(o, key, prop))
    }

    fn ordinary_define(&self, o: ObjRef, key: &PropKey, prop: Prop) -> bool {
        match self.with_obj(o, |ob| ob.props.get(key).cloned()) {
            None => {
                if !self.is_extensible(o) {
                    return false;
                }
                self.with_obj_mut(o, |ob| ob.props.push(key.clone(), prop));
                self.0.heap.borrow_mut().add_bytes(o, PROP_BYTES);
                true
            }
            Some(cur) => {
                if !can_replace(&cur, &prop) {
                    return false;
                }
                self.with_obj_mut(o, |ob| {
                    if let Some(slot) = ob.props.get_mut(key) {
                        *slot = prop;
                    }
                });
                true
            }
        }
    }

    /// Defines an array element. Elements share one attribute set (`ArrAttrs`); a define that would need
    /// different attributes for one element is refused with a TypeError (see ABI.md).
    fn array_define_index(&self, o: ObjRef, i: u32, prop: Prop) -> Res<bool> {
        let idx = i as usize;
        let (len, old, attrs, extensible) = self.with_obj(o, |ob| {
            let (len, old) = match &ob.kind {
                ObjectKind::Array(elems) => {
                    (elems.len(), elems.get(idx).filter(|v| !matches!(v, Value::Hole)).cloned())
                }
                _ => (0, None),
            };
            (len, old, ob.arr, ob.extensible)
        });
        let Prop::Data { value, flags } = prop else {
            return Err(self.throw_type_error("unsupported: accessor property on an array index"));
        };
        if let Some(old) = old {
            let cur = Flags::new(attrs.elem_writable, true, attrs.elem_configurable);
            if flags == cur {
                if attrs.elem_writable {
                    self.with_obj_mut(o, |ob| set_elem(ob, idx, value));
                    return Ok(true);
                }
                return Ok(same_value(&old, &value));
            }
            if !attrs.elem_configurable {
                if flags.configurable || !flags.enumerable {
                    return Ok(false);
                }
                if !attrs.elem_writable {
                    return Ok(false);
                }
            }
            return Err(self.throw_type_error("unsupported: per-element attributes on an array index"));
        }
        if !extensible {
            return Ok(false);
        }
        if flags != Flags::ALL {
            return Err(self.throw_type_error("unsupported: non-default attributes on a new array element"));
        }
        if idx >= len {
            if !attrs.len_writable {
                return Ok(false);
            }
            if idx >= MAX_DENSE_LEN {
                return Err(self.throw_range_error("Invalid array length: index exceeds the dense limit"));
            }
            self.with_obj_mut(o, |ob| {
                if let ObjectKind::Array(elems) = &mut ob.kind {
                    elems.resize(idx + 1, Value::Hole);
                }
            });
            self.0.heap.borrow_mut().add_bytes(o, (idx + 1 - len) * ELEM_BYTES);
        }
        self.with_obj_mut(o, |ob| set_elem(ob, idx, value));
        Ok(true)
    }

    /// ArraySetLength (ECMA-262 10.4.2.4): truncates or extends with holes; a non-configurable element
    /// stops truncation; a false `writable` makes the length read-only.
    fn array_define_length(&self, o: ObjRef, prop: Prop) -> Res<bool> {
        let Prop::Data { value, flags } = prop else { return Ok(false) };
        if flags.configurable || flags.enumerable {
            return Ok(false);
        }
        let num = self.to_number(&value)?;
        let new_len = number::to_uint32(num);
        if f64::from(new_len) != num {
            return Err(self.throw_range_error("Invalid array length"));
        }
        let new = new_len as usize;
        let (len, len_writable, elem_cfg) = self.with_obj(o, |ob| (array_len(ob), ob.arr.len_writable, ob.arr.elem_configurable));
        if !len_writable {
            return Ok(!flags.writable && new == len);
        }
        let mut ok = true;
        let mut final_len = new;
        if new < len {
            let stop = self.with_obj(o, |ob| match &ob.kind {
                ObjectKind::Array(elems) if !elem_cfg => {
                    (new..elems.len()).rev().find(|&i| !matches!(elems[i], Value::Hole))
                }
                _ => None,
            });
            if let Some(k) = stop {
                final_len = k + 1;
                ok = false;
            }
            self.with_obj_mut(o, |ob| {
                if let ObjectKind::Array(elems) = &mut ob.kind {
                    elems.truncate(final_len);
                }
            });
            self.0.heap.borrow_mut().sub_bytes(o, (len - final_len) * ELEM_BYTES);
        } else if new > len {
            if new > MAX_DENSE_LEN {
                return Err(self.throw_range_error("Invalid array length: exceeds the dense limit"));
            }
            self.with_obj_mut(o, |ob| {
                if let ObjectKind::Array(elems) = &mut ob.kind {
                    elems.resize(new, Value::Hole);
                }
            });
            self.0.heap.borrow_mut().add_bytes(o, (new - len) * ELEM_BYTES);
        }
        if !flags.writable {
            self.with_obj_mut(o, |ob| ob.arr.len_writable = false);
        }
        Ok(ok)
    }

    /// Defines a data property (returns false when the definition is refused).
    pub fn define_data(&self, o: ObjRef, key: PropKey, v: Value, flags: Flags) -> Res<bool> {
        self.define_own(o, &key, Prop::Data { value: v, flags })
    }

    /// Defines an accessor property. `writable` is ignored (accessors are never writable).
    pub fn define_accessor(
        &self,
        o: ObjRef,
        key: PropKey,
        get: Option<ObjRef>,
        set: Option<ObjRef>,
        flags: Flags,
    ) -> Res<bool> {
        let flags = Flags::new(false, flags.enumerable, flags.configurable);
        self.define_own(o, &key, Prop::Accessor { get, set, flags })
    }

    /// CreateDataProperty: a new enumerable, writable, configurable data property.
    pub fn create_data_property(&self, o: ObjRef, key: PropKey, v: Value) -> Res<bool> {
        self.define_own(o, &key, Prop::Data { value: v, flags: Flags::ALL })
    }

    /// [[Delete]]. Returns false for a non-configurable property.
    pub fn delete(&self, o: ObjRef, key: &PropKey) -> Res<bool> {
        if let Some(ex) = self.exotic_of(o) {
            if let Hook::Handled(b) = ex.delete(self, o, key)? {
                return Ok(b);
            }
        }
        if self.protected_string_prop(o, key).is_some() {
            return Ok(false);
        }
        if self.is_array_obj(o) {
            if let PropKey::Index(i) = key {
                let idx = *i as usize;
                let (present, cfg) = self.with_obj(o, |ob| {
                    let present = matches!(&ob.kind, ObjectKind::Array(e) if e.get(idx).is_some_and(|v| !matches!(v, Value::Hole)));
                    (present, ob.arr.elem_configurable)
                });
                if !present {
                    return Ok(true);
                }
                if !cfg {
                    return Ok(false);
                }
                self.with_obj_mut(o, |ob| {
                    if let ObjectKind::Array(elems) = &mut ob.kind {
                        elems[idx] = Value::Hole;
                    }
                });
                return Ok(true);
            }
            if is_length(key) {
                return Ok(false);
            }
        }
        let (existed, deleted) = self.with_obj_mut(o, |ob| match ob.props.get(key).map(|p| p.flags().configurable) {
            None => (false, false),
            Some(false) => (true, false),
            Some(true) => {
                ob.props.remove(key);
                (true, true)
            }
        });
        if deleted {
            self.0.heap.borrow_mut().sub_bytes(o, PROP_BYTES);
        }
        Ok(!existed || deleted)
    }

    /// [[HasProperty]]: walks the prototype chain.
    pub fn has_property(&self, o: ObjRef, key: &PropKey) -> Res<bool> {
        let mut cur = o;
        loop {
            if let Some(ex) = self.exotic_of(cur) {
                if let Hook::Handled(b) = ex.has(self, cur, key)? {
                    return Ok(b);
                }
            }
            if self.get_own_property(cur, key)?.is_some() {
                return Ok(true);
            }
            match self.get_prototype_of(cur) {
                Some(p) => cur = p,
                None => return Ok(false),
            }
        }
    }

    pub fn has_own_property(&self, o: ObjRef, key: &PropKey) -> Res<bool> {
        Ok(self.get_own_property(o, key)?.is_some())
    }

    /// [[OwnPropertyKeys]]: integer keys ascending, then strings in insertion order, then symbols in
    /// insertion order. For arrays `length` follows the element indices.
    pub fn own_keys(&self, o: ObjRef) -> Res<Vec<PropKey>> {
        if let Some(ex) = self.exotic_of(o) {
            if let Hook::Handled(keys) = ex.own_keys(self, o)? {
                return Ok(keys);
            }
        }
        Ok(self.with_obj(o, |ob| {
            let mut out = Vec::new();
            if let ObjectKind::Array(elems) = &ob.kind {
                for (i, v) in elems.iter().enumerate() {
                    if !matches!(v, Value::Hole) {
                        out.push(PropKey::Index(i as u32));
                    }
                }
                out.push(PropKey::from_str("length"));
            }
            // A String wrapper lists its characters first, then its other index keys, `length` and the other strings.
            let string_len = match &ob.kind {
                ObjectKind::Wrapper(Value::String(s)) => Some(s.len()),
                _ => None,
            };
            if let Some(len) = string_len {
                out.extend((0..len as u32).map(PropKey::Index));
            }
            let mut idx: Vec<u32> = ob
                .props
                .iter()
                .filter_map(|(k, _)| if let PropKey::Index(i) = k { Some(*i) } else { None })
                .collect();
            idx.sort_unstable();
            out.extend(idx.into_iter().map(PropKey::Index));
            if string_len.is_some() {
                out.push(PropKey::from_str("length"));
            }
            out.extend(ob.props.iter().filter(|(k, _)| matches!(k, PropKey::Str(_))).map(|(k, _)| k.clone()));
            out.extend(ob.props.iter().filter(|(k, _)| matches!(k, PropKey::Sym(_))).map(|(k, _)| k.clone()));
            out
        }))
    }

    /// Sets [[Prototype]]. False when non-extensible or when it would create a cycle.
    pub fn set_prototype_of(&self, o: ObjRef, proto: Option<ObjRef>) -> bool {
        if self.get_prototype_of(o) == proto {
            return true;
        }
        if !self.is_extensible(o) {
            return false;
        }
        let mut p = proto;
        while let Some(x) = p {
            if x == o {
                return false;
            }
            p = self.get_prototype_of(x);
        }
        self.with_obj_mut(o, |ob| ob.proto = proto);
        true
    }

    /// [[PreventExtensions]].
    pub fn prevent_extensions(&self, o: ObjRef) -> bool {
        self.with_obj_mut(o, |ob| ob.extensible = false);
        true
    }

    /// SetIntegrityLevel: `freeze` (frozen) or `seal` (sealed). Arrays change their shared element
    /// attributes and the length attribute uniformly.
    fn set_integrity(&self, o: ObjRef, frozen: bool) -> Res<bool> {
        if self.exotic_of(o).is_some() {
            self.prevent_extensions(o);
            for k in self.own_keys(o)? {
                let Some(p) = self.get_own_property(o, &k)? else { continue };
                let prop = match p {
                    Prop::Data { value, flags } => Prop::Data {
                        value,
                        flags: Flags::new(flags.writable && !frozen, flags.enumerable, false),
                    },
                    Prop::Accessor { get, set, flags } => {
                        Prop::Accessor { get, set, flags: Flags::new(false, flags.enumerable, false) }
                    }
                };
                if !self.define_own(o, &k, prop)? {
                    return Ok(false);
                }
            }
            return Ok(true);
        }
        self.with_obj_mut(o, |ob| {
            ob.extensible = false;
            if let ObjectKind::Array(_) = ob.kind {
                ob.arr.elem_configurable = false;
                if frozen {
                    ob.arr.elem_writable = false;
                    ob.arr.len_writable = false;
                }
            }
            for (_, p) in ob.props.iter_mut() {
                match p {
                    Prop::Data { flags, .. } => {
                        flags.configurable = false;
                        if frozen {
                            flags.writable = false;
                        }
                    }
                    Prop::Accessor { flags, .. } => flags.configurable = false,
                }
            }
        });
        Ok(true)
    }

    pub fn freeze(&self, o: ObjRef) -> Res<bool> {
        self.set_integrity(o, true)
    }

    pub fn seal(&self, o: ObjRef) -> Res<bool> {
        self.set_integrity(o, false)
    }

    /// TestIntegrityLevel: not extensible, and every own property non-configurable (and read-only for
    /// `is_frozen`). For arrays `length` is not consulted, as in V8 (ABI.md section 12): element slots are still
    /// checked through `get_own_property`, and `freeze` still makes `length` read-only.
    fn test_integrity(&self, o: ObjRef, frozen: bool) -> Res<bool> {
        if self.is_extensible(o) {
            return Ok(false);
        }
        let array = self.is_array_obj(o);
        for k in self.own_keys(o)? {
            if array && is_length(&k) {
                continue;
            }
            let Some(p) = self.get_own_property(o, &k)? else { continue };
            if p.flags().configurable {
                return Ok(false);
            }
            if let (true, Prop::Data { flags, .. }) = (frozen, &p) {
                if flags.writable {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    pub fn is_frozen(&self, o: ObjRef) -> Res<bool> {
        self.test_integrity(o, true)
    }

    pub fn is_sealed(&self, o: ObjRef) -> Res<bool> {
        self.test_integrity(o, false)
    }

    /// Array.isArray for values: true for an array, and for an exotic object whose hook says it is one.
    pub fn is_array(&self, v: &Value) -> bool {
        match v {
            Value::Object(o) => self.is_array_obj(*o) || self.exotic_of(*o).is_some_and(|e| e.is_array()),
            _ => false,
        }
    }
}

/// The coarse kind of an object, for diagnostics and the trace encoder (ABI.md section 13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjClass {
    Ordinary,
    Array,
    Function,
    Error,
    Wrapper,
    Native,
    Exotic,
    Cell,
}

impl Rt {
    pub fn class_of(&self, o: ObjRef) -> ObjClass {
        self.with_obj(o, |ob| match &ob.kind {
            ObjectKind::Ordinary => ObjClass::Ordinary,
            ObjectKind::Array(_) => ObjClass::Array,
            ObjectKind::Function(_) => ObjClass::Function,
            ObjectKind::Cell(_) => ObjClass::Cell,
            ObjectKind::Error => ObjClass::Error,
            ObjectKind::Wrapper(_) => ObjClass::Wrapper,
            ObjectKind::Native(_) => ObjClass::Native,
            ObjectKind::Exotic(_) => ObjClass::Exotic,
        })
    }
}
