//! Builtin family: Map and Set (ECMA-262 24.1 and 24.2), the ES2025 set methods, Map.groupBy and the Map and Set
//! iterators. A Map or Set is an `ObjectKind::Native` object whose payload is `Collection`.
//!
//! Storage (`Store`): entries in insertion order with tombstones, and a hash index on SameValueZero keys, so lookup is
//! O(1) on average. An iterator or forEach walk is a cursor, an index into the entry list, that every mutation keeps
//! valid: deleted entries are skipped, entries added during the walk are visited, delete then re-add moves the entry to
//! the end, and `clear` resets cursors to 0, so entries added after a clear are visited (V8 14, Node 26; the WP text
//! "clear ends the walk" holds only when nothing is added after it). Compaction drops tombstones and remaps the cursors.
//!
//! Absent by design (not supported in the standard-library manifest): WeakMap, WeakSet, WeakRef, FinalizationRegistry,
//! Symbol.species, Map.prototype.getOrInsert and getOrInsertComputed (V8 14 has the last two; the WP does not list them).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use crate::function::{Call, CodeFn};
use crate::jsstr::JsStr;
use crate::native::{payload_ref, NativeObject};
use crate::number;
use crate::object::{Flags, Prop, MAX_DENSE_LEN};
use crate::roots::Tracer;
use crate::rt::{Rt, WellKnown};
use crate::value::{ObjRef, PropKey, Res, Sym, Value};

use super::iterator::{self, IterRecord, NativeIter};

// ---------------------------------------------------------------- keys

/// Hash key of a SameValueZero value: numbers by bits after -0 and NaN are normalised, strings by content, objects and
/// symbols by identity.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Undefined,
    Null,
    Bool(bool),
    Number(u64),
    String(JsStr),
    Symbol(Sym),
    Object(ObjRef),
}

const CANON_NAN: u64 = f64::NAN.to_bits();

fn key_of(v: &Value) -> Key {
    match v {
        Value::Undefined => Key::Undefined,
        Value::Null => Key::Null,
        Value::Bool(b) => Key::Bool(*b),
        Value::Number(n) if n.is_nan() => Key::Number(CANON_NAN),
        Value::Number(n) if *n == 0.0 => Key::Number(0),
        Value::Number(n) => Key::Number(n.to_bits()),
        Value::String(s) => Key::String(s.clone()),
        Value::Symbol(s) => Key::Symbol(*s),
        Value::Object(o) => Key::Object(*o),
        Value::Uninit | Value::Hole => panic!("internal marker used as a collection key (framework bug)"),
    }
}

/// CanonicalizeKeyedCollectionKey: -0 is stored as +0.
fn canon(v: Value) -> Value {
    match v {
        Value::Number(n) if n == 0.0 => Value::Number(0.0),
        v => v,
    }
}

// ---------------------------------------------------------------- storage

/// One slot of the entry list: a live (key, value) pair, or `None` for a deleted entry (a tombstone). A Set stores
/// `Undefined` as its value.
type Slot = Option<(Value, Value)>;

/// Tombstones are compacted once they are at least this many and outnumber the live entries.
const COMPACT_MIN: usize = 16;

#[derive(Default)]
struct Store {
    entries: Vec<Slot>,
    /// Position in `entries` of each live key. Its size is the collection's size.
    index: HashMap<Key, usize>,
    /// Cursors of iterators and forEach walks. Dead weak references are pruned when the list grows past `prune_at`.
    cursors: Vec<Weak<Cell<usize>>>,
    prune_at: usize,
}

impl Store {
    fn size(&self) -> usize {
        self.index.len()
    }

    fn get(&self, k: &Value) -> Option<&Value> {
        let i = *self.index.get(&key_of(k))?;
        self.entries[i].as_ref().map(|(_, v)| v)
    }

    fn has(&self, k: &Value) -> bool {
        self.index.contains_key(&key_of(k))
    }

    /// Inserts a new entry at the end, or replaces the value of a live one. `k` is already canonical.
    fn set(&mut self, k: Value, v: Value) {
        let key = key_of(&k);
        if let Some(i) = self.index.get(&key).copied() {
            if let Some(slot) = self.entries[i].as_mut() {
                slot.1 = v;
            }
            return;
        }
        self.index.insert(key, self.entries.len());
        self.entries.push(Some((k, v)));
    }

    fn remove(&mut self, k: &Value) -> bool {
        let Some(i) = self.index.remove(&key_of(k)) else {
            return false;
        };
        self.entries[i] = None;
        let dead = self.entries.len() - self.index.len();
        if dead >= COMPACT_MIN && dead > self.index.len() {
            self.compact();
        }
        true
    }

    /// Empties the list. Every cursor moves to 0, so entries added later are visited by walks in progress.
    fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
        self.cursors.retain(|w| match w.upgrade() {
            Some(c) => {
                c.set(0);
                true
            }
            None => false,
        });
    }

    fn new_cursor(&mut self) -> Rc<Cell<usize>> {
        let c = Rc::new(Cell::new(0));
        self.cursors.push(Rc::downgrade(&c));
        if self.cursors.len() > self.prune_at {
            self.cursors.retain(|w| w.strong_count() > 0);
            self.prune_at = (2 * self.cursors.len()).max(COMPACT_MIN);
        }
        c
    }

    /// The next live entry at or after the cursor, which moves past it. `None` at the end of the list.
    fn advance(&self, cur: &Cell<usize>) -> Option<(Value, Value)> {
        let mut pos = cur.get();
        while pos < self.entries.len() {
            let i = pos;
            pos += 1;
            if let Some((k, v)) = &self.entries[i] {
                cur.set(pos);
                return Some((k.clone(), v.clone()));
            }
        }
        cur.set(pos);
        None
    }

    /// Drops the tombstones. A cursor moves to the new position of the first live entry at or after its old one.
    fn compact(&mut self) {
        let old = std::mem::take(&mut self.entries);
        let mut live_before = Vec::with_capacity(old.len() + 1);
        let mut live = 0;
        for slot in &old {
            live_before.push(live);
            if slot.is_some() {
                live += 1;
            }
        }
        live_before.push(live);
        self.cursors.retain(|w| match w.upgrade() {
            Some(c) => {
                c.set(live_before[c.get().min(old.len())]);
                true
            }
            None => false,
        });
        self.entries = old.into_iter().flatten().map(Some).collect();
        for (i, slot) in self.entries.iter().enumerate() {
            if let Some((k, _)) = slot {
                if let Some(pos) = self.index.get_mut(&key_of(k)) {
                    *pos = i;
                }
            }
        }
    }

    /// The live entries in order, copied out so that no borrow is held while user code runs.
    fn live(&self) -> Vec<(Value, Value)> {
        self.entries.iter().flatten().cloned().collect()
    }

    /// A Set holding the live keys of `self` in order (the spec's copy of `[[SetData]]`, without tombstones).
    fn key_copy(&self) -> Store {
        let mut out = Store::default();
        for (k, _) in self.live() {
            out.set(k, Value::Undefined);
        }
        out
    }

    fn trace(&self, t: &mut Tracer) {
        for (k, v) in self.entries.iter().flatten() {
            t.mark_value(k);
            t.mark_value(v);
        }
    }
}

/// The host payload of a Map (`is_set` false) or a Set.
struct Collection {
    is_set: bool,
    store: RefCell<Store>,
}

impl NativeObject for Collection {
    fn trace(&self, t: &mut Tracer) {
        self.store.borrow().trace(t);
    }
}

/// The payload of `this` when it is a Map (`want_set` false) or a Set, else a TypeError.
fn brand(rt: &Rt, this: &Value, want_set: bool, method: &str) -> Res<Rc<dyn NativeObject>> {
    if let Value::Object(o) = this {
        if let Some(p) = rt.native_payload(*o) {
            if payload_ref::<Collection>(&*p).is_some_and(|c| c.is_set == want_set) {
                return Ok(p);
            }
        }
    }
    let kind = if want_set { "Set" } else { "Map" };
    Err(rt.throw_type_error(&format!(
        "Method {kind}.prototype.{method} called on incompatible receiver {}",
        rt.describe(this)
    )))
}

/// The collection of a payload that `brand` returned.
fn coll(p: &Rc<dyn NativeObject>) -> &Collection {
    payload_ref::<Collection>(&**p).expect("payload checked by brand (framework bug)")
}

/// The prototype for a new Map or Set: `new_target.prototype` when it is an object, else the realm's `fallback`.
fn prototype_for(rt: &Rt, new_target: &Value, fallback: &str) -> Res<ObjRef> {
    match rt.get_str(new_target, "prototype")? {
        Value::Object(p) => Ok(p),
        _ => Ok(rt.builtin(fallback).expect("collection prototypes are installed by Rt::new (framework bug)")),
    }
}

fn new_collection(rt: &Rt, proto: ObjRef, is_set: bool, store: Store) -> ObjRef {
    rt.new_native_object(Some(proto), Rc::new(Collection { is_set, store: RefCell::new(store) }))
}

fn new_set_value(rt: &Rt, store: Store) -> Value {
    let proto = rt.builtin("Set.prototype").expect("Set.prototype is installed by Rt::new (framework bug)");
    Value::Object(new_collection(rt, proto, true, store))
}

// ---------------------------------------------------------------- Map and Set constructors

fn map_call(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("Constructor Map requires 'new'"))
}

/// new Map(iterable): `set` is read once, before the iterable is touched. Each entry must be an object (else a
/// TypeError that closes the iterator); its `0` and `1` are read in that order and passed to `set`.
fn map_construct(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let proto = prototype_for(rt, c.new_target, "Map.prototype")?;
    let map = new_collection(rt, proto, false, Store::default());
    let iterable = c.arg(0);
    if iterable.is_nullish() {
        return Ok(Value::Object(map));
    }
    let target = Value::Object(map);
    let adder = rt.get_str(&target, "set")?;
    if !rt.is_callable(&adder) {
        return Err(rt.throw_type_error(&format!("{} is not a function", rt.describe(&adder))));
    }
    iterator::iterate(rt, &iterable, &mut |item| {
        let Some(entry) = item.as_object() else {
            return Err(rt.throw_type_error(&format!("Iterator value {} is not an entry object", rt.describe(&item))));
        };
        let k = rt.get(entry, &PropKey::Index(0), &item)?;
        let v = rt.get(entry, &PropKey::Index(1), &item)?;
        rt.call(&adder, &target, &[k, v])?;
        Ok(true)
    })?;
    Ok(target)
}

fn set_call(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("Constructor Set requires 'new'"))
}

/// new Set(iterable): `add` is read once, before the iterable is touched, and called with each value.
fn set_construct(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let proto = prototype_for(rt, c.new_target, "Set.prototype")?;
    let set = new_collection(rt, proto, true, Store::default());
    let iterable = c.arg(0);
    if iterable.is_nullish() {
        return Ok(Value::Object(set));
    }
    let target = Value::Object(set);
    let adder = rt.get_str(&target, "add")?;
    if !rt.is_callable(&adder) {
        return Err(rt.throw_type_error(&format!("{} is not a function", rt.describe(&adder))));
    }
    iterator::iterate(rt, &iterable, &mut |item| {
        rt.call(&adder, &target, &[item])?;
        Ok(true)
    })?;
    Ok(target)
}

// ---------------------------------------------------------------- Map.prototype

fn map_get(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, false, "get")?;
    let found = coll(&p).store.borrow().get(&c.arg(0)).cloned();
    Ok(found.unwrap_or(Value::Undefined))
}

fn map_set(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, false, "set")?;
    coll(&p).store.borrow_mut().set(canon(c.arg(0)), c.arg(1));
    Ok(c.this.clone())
}

fn map_has(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, false, "has")?;
    let found = coll(&p).store.borrow().has(&c.arg(0));
    Ok(Value::Bool(found))
}

fn map_delete(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, false, "delete")?;
    let removed = coll(&p).store.borrow_mut().remove(&c.arg(0));
    Ok(Value::Bool(removed))
}

fn map_clear(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, false, "clear")?;
    coll(&p).store.borrow_mut().clear();
    Ok(Value::Undefined)
}

fn map_size(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, false, "size")?;
    let n = coll(&p).store.borrow().size();
    Ok(Value::Number(n as f64))
}

fn map_keys(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    make_iterator(rt, c.this, false, Kind::Keys, "keys")
}

fn map_values(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    make_iterator(rt, c.this, false, Kind::Values, "values")
}

fn map_entries_method(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    make_iterator(rt, c.this, false, Kind::Entries, "entries")
}

fn map_for_each(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    for_each(rt, c, false)
}

// ---------------------------------------------------------------- Map.groupBy

/// Map.groupBy(items, callback) (ES2025 24.1.2.1): groups by the callback's key (-0 becomes +0, NaN groups with NaN),
/// called with (value, index). A throw from the callback closes the iterator.
fn map_group_by(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let items = c.arg(0);
    let callback = c.arg(1);
    if items.is_nullish() {
        return Err(rt.throw_type_error("Map.groupBy called on null or undefined"));
    }
    if !rt.is_callable(&callback) {
        return Err(rt.throw_type_error(&format!("{} is not a function", rt.describe(&callback))));
    }
    let mut groups: Vec<(Value, Vec<Value>)> = Vec::new();
    let mut slot: HashMap<Key, usize> = HashMap::new();
    let mut index = 0usize;
    iterator::iterate(rt, &items, &mut |v| {
        let key = canon(rt.call(&callback, &Value::Undefined, &[v.clone(), Value::Number(index as f64)])?);
        let k = key_of(&key);
        let g = match slot.get(&k) {
            Some(&g) => g,
            None => {
                groups.push((key, Vec::new()));
                slot.insert(k, groups.len() - 1);
                groups.len() - 1
            }
        };
        groups[g].1.push(v);
        index += 1;
        Ok(true)
    })?;
    let mut store = Store::default();
    for (key, values) in groups {
        if values.len() > MAX_DENSE_LEN {
            return Err(rt.throw_range_error("Invalid array length"));
        }
        let arr = rt.new_array(values);
        store.set(key, Value::Object(arr));
    }
    let proto = rt.builtin("Map.prototype").expect("Map.prototype is installed by Rt::new (framework bug)");
    Ok(Value::Object(new_collection(rt, proto, false, store)))
}

// ---------------------------------------------------------------- forEach and iterators

/// Map.prototype.forEach(callback, thisArg) and Set.prototype.forEach: each live entry in order, with the entries
/// added during the walk visited. A Map passes (value, key, map); a Set passes (value, value, set).
fn for_each(rt: &Rt, c: &Call<'_>, is_set: bool) -> Res<Value> {
    let p = brand(rt, c.this, is_set, "forEach")?;
    let f = c.arg(0);
    if !rt.is_callable(&f) {
        return Err(rt.throw_type_error(&format!("{} is not a function", rt.describe(&f))));
    }
    let this_arg = c.arg(1);
    let cursor = coll(&p).store.borrow_mut().new_cursor();
    loop {
        let step = coll(&p).store.borrow().advance(&cursor);
        let Some((k, v)) = step else { break };
        let value = if is_set { k.clone() } else { v };
        rt.call(&f, &this_arg, &[value, k, c.this.clone()])?;
    }
    Ok(Value::Undefined)
}

#[derive(Clone, Copy)]
enum Kind {
    Keys,
    Values,
    Entries,
}

/// The state of a Map or Set iterator: the collection (`None` once exhausted, as the spec drops [[IteratedObject]]),
/// the walk's cursor, and what each step yields.
struct CollIter {
    coll: Cell<Option<ObjRef>>,
    cursor: Rc<Cell<usize>>,
    kind: Kind,
    is_set: bool,
}

impl NativeObject for CollIter {
    fn trace(&self, t: &mut Tracer) {
        if let Some(o) = self.coll.get() {
            t.mark(o);
        }
    }
}

impl NativeIter for CollIter {
    fn next(&self, rt: &Rt) -> Res<Option<Value>> {
        let Some(o) = self.coll.get() else {
            return Ok(None);
        };
        let p = rt.native_payload(o).expect("an iterator keeps its collection alive (framework bug)");
        let step = coll(&p).store.borrow().advance(&self.cursor);
        let Some((k, v)) = step else {
            self.coll.set(None);
            return Ok(None);
        };
        let value = match self.kind {
            Kind::Keys => k,
            Kind::Values if self.is_set => k,
            Kind::Values => v,
            Kind::Entries => {
                let second = if self.is_set { k.clone() } else { v };
                Value::Object(rt.new_array(vec![k, second]))
            }
        };
        Ok(Some(value))
    }
}

fn make_iterator(rt: &Rt, this: &Value, is_set: bool, kind: Kind, method: &str) -> Res<Value> {
    let p = brand(rt, this, is_set, method)?;
    let cursor = coll(&p).store.borrow_mut().new_cursor();
    let state = CollIter { coll: Cell::new(this.as_object()), cursor, kind, is_set };
    let proto = if is_set { "SetIteratorPrototype" } else { "MapIteratorPrototype" };
    Ok(Value::Object(iterator::new_iterator_object(rt, proto, Rc::new(state))))
}

// ---------------------------------------------------------------- Set.prototype

fn set_add(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, true, "add")?;
    coll(&p).store.borrow_mut().set(canon(c.arg(0)), Value::Undefined);
    Ok(c.this.clone())
}

fn set_has(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, true, "has")?;
    let found = coll(&p).store.borrow().has(&c.arg(0));
    Ok(Value::Bool(found))
}

fn set_delete(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, true, "delete")?;
    let removed = coll(&p).store.borrow_mut().remove(&c.arg(0));
    Ok(Value::Bool(removed))
}

fn set_clear(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, true, "clear")?;
    coll(&p).store.borrow_mut().clear();
    Ok(Value::Undefined)
}

fn set_size(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, true, "size")?;
    let n = coll(&p).store.borrow().size();
    Ok(Value::Number(n as f64))
}

fn set_for_each(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    for_each(rt, c, true)
}

fn set_values_method(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    make_iterator(rt, c.this, true, Kind::Values, "values")
}

fn set_entries(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    make_iterator(rt, c.this, true, Kind::Entries, "entries")
}

// ---------------------------------------------------------------- ES2025 set methods

/// GetSetRecord (ES2025 24.2.1.2): `size`, `has` and `keys` of the argument, read in that order.
struct SetRec {
    obj: Value,
    /// ToIntegerOrInfinity of `size`: an integer >= 0, possibly +Infinity.
    size: f64,
    has: Value,
    keys: Value,
}

fn set_record(rt: &Rt, other: &Value) -> Res<SetRec> {
    if !other.is_object() {
        return Err(rt.throw_type_error(&format!("{} is not an object", rt.describe(other))));
    }
    let raw = rt.get_str(other, "size")?;
    let num = rt.to_number(&raw)?;
    if num.is_nan() {
        return Err(rt.throw_type_error("The 'size' property of the set-like argument is not a number"));
    }
    let size = number::to_integer_or_infinity(num);
    if size < 0.0 {
        return Err(rt.throw_range_error("The 'size' property of the set-like argument is negative"));
    }
    let has = rt.get_str(other, "has")?;
    if !rt.is_callable(&has) {
        return Err(rt.throw_type_error("The 'has' property of the set-like argument is not callable"));
    }
    let keys = rt.get_str(other, "keys")?;
    if !rt.is_callable(&keys) {
        return Err(rt.throw_type_error("The 'keys' property of the set-like argument is not callable"));
    }
    Ok(SetRec { obj: other.clone(), size, has, keys })
}

/// GetIteratorFromMethod(obj, keys): calls `keys` and reads `next` once. The result must be an object.
fn keys_iter(rt: &Rt, rec: &SetRec) -> Res<IterRecord> {
    let iterator = rt.call(&rec.keys, &rec.obj, &[])?;
    if !iterator.is_object() {
        return Err(rt.throw_type_error(&format!("{} is not an object", rt.describe(&iterator))));
    }
    let next = rt.get_str(&iterator, "next")?;
    Ok(IterRecord { iterator, next, done: false })
}

fn live_size(p: &Rc<dyn NativeObject>) -> f64 {
    coll(p).store.borrow().size() as f64
}

/// Calls `other.has` on each live entry of this set, in order, until a call returns `stop_when`. Returns whether it
/// stopped. The walk is a cursor on this set, so entries deleted by `has` are skipped and entries it adds are visited
/// (the ES2025 `thisSize` re-read).
fn has_walk_stops(rt: &Rt, p: &Rc<dyn NativeObject>, rec: &SetRec, stop_when: bool) -> Res<bool> {
    let cursor = coll(p).store.borrow_mut().new_cursor();
    loop {
        let step = coll(p).store.borrow().advance(&cursor);
        let Some((k, _)) = step else { return Ok(false) };
        if rt.call(&rec.has, &rec.obj, &[k])?.to_boolean() == stop_when {
            return Ok(true);
        }
    }
}

/// Set.prototype.union: `keys` is called before this set is copied.
fn set_union(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, true, "union")?;
    let rec = set_record(rt, &c.arg(0))?;
    let mut keys = keys_iter(rt, &rec)?;
    let mut result = coll(&p).store.borrow().key_copy();
    while let Some(v) = iterator::iter_step(rt, &mut keys)? {
        result.set(canon(v), Value::Undefined);
    }
    Ok(new_set_value(rt, result))
}

/// Set.prototype.intersection: walks this set when it is no larger than `other`, else walks `other`'s keys.
fn set_intersection(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, true, "intersection")?;
    let rec = set_record(rt, &c.arg(0))?;
    let mut result = Store::default();
    if live_size(&p) <= rec.size {
        let cursor = coll(&p).store.borrow_mut().new_cursor();
        loop {
            let step = coll(&p).store.borrow().advance(&cursor);
            let Some((k, _)) = step else { break };
            if rt.call(&rec.has, &rec.obj, &[k.clone()])?.to_boolean() {
                result.set(k, Value::Undefined);
            }
        }
    } else {
        let mut keys = keys_iter(rt, &rec)?;
        while let Some(v) = iterator::iter_step(rt, &mut keys)? {
            let v = canon(v);
            if coll(&p).store.borrow().has(&v) {
                result.set(v, Value::Undefined);
            }
        }
    }
    Ok(new_set_value(rt, result))
}

/// Set.prototype.difference: copies this set first. The walk over `other.has` visits the copy's keys in order, once
/// each (later changes to this set do not affect it); the key walk removes each key `other` yields.
fn set_difference(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, true, "difference")?;
    let rec = set_record(rt, &c.arg(0))?;
    let copy = coll(&p).store.borrow().key_copy();
    let order: Vec<Value> = copy.live().into_iter().map(|(k, _)| k).collect();
    let mut result = copy;
    if live_size(&p) <= rec.size {
        for e in order {
            if rt.call(&rec.has, &rec.obj, &[e.clone()])?.to_boolean() {
                result.remove(&e);
            }
        }
    } else {
        let mut keys = keys_iter(rt, &rec)?;
        while let Some(v) = iterator::iter_step(rt, &mut keys)? {
            result.remove(&canon(v));
        }
    }
    Ok(new_set_value(rt, result))
}

/// Set.prototype.symmetricDifference: `keys` is called before this set is copied; membership of this set is read live.
fn set_symmetric_difference(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, true, "symmetricDifference")?;
    let rec = set_record(rt, &c.arg(0))?;
    let mut keys = keys_iter(rt, &rec)?;
    let mut result = coll(&p).store.borrow().key_copy();
    while let Some(v) = iterator::iter_step(rt, &mut keys)? {
        let v = canon(v);
        let in_this = coll(&p).store.borrow().has(&v);
        let in_result = result.has(&v);
        if in_this {
            if in_result {
                result.remove(&v);
            }
        } else if !in_result {
            result.set(v, Value::Undefined);
        }
    }
    Ok(new_set_value(rt, result))
}

fn set_is_subset_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, true, "isSubsetOf")?;
    let rec = set_record(rt, &c.arg(0))?;
    if live_size(&p) > rec.size {
        return Ok(Value::Bool(false));
    }
    let missing = has_walk_stops(rt, &p, &rec, false)?;
    Ok(Value::Bool(!missing))
}

/// Set.prototype.isSupersetOf: walks `other`'s keys, closing the iterator on the first key missing from this set.
fn set_is_superset_of(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, true, "isSupersetOf")?;
    let rec = set_record(rt, &c.arg(0))?;
    if live_size(&p) < rec.size {
        return Ok(Value::Bool(false));
    }
    let mut keys = keys_iter(rt, &rec)?;
    while let Some(v) = iterator::iter_step(rt, &mut keys)? {
        if !coll(&p).store.borrow().has(&v) {
            iterator::iter_close(rt, &keys, false)?;
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

fn set_is_disjoint_from(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let p = brand(rt, c.this, true, "isDisjointFrom")?;
    let rec = set_record(rt, &c.arg(0))?;
    if live_size(&p) <= rec.size {
        let shared = has_walk_stops(rt, &p, &rec, true)?;
        return Ok(Value::Bool(!shared));
    }
    let mut keys = keys_iter(rt, &rec)?;
    while let Some(v) = iterator::iter_step(rt, &mut keys)? {
        if coll(&p).store.borrow().has(&v) {
            iterator::iter_close(rt, &keys, false)?;
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

// ---------------------------------------------------------------- installation

/// Public helpers for hosts and the test kit (no user code runs in them).
///
/// A new, empty Map with %Map.prototype%.
pub fn new_map(rt: &Rt) -> ObjRef {
    let proto = rt.builtin("Map.prototype").expect("Map.prototype is installed by Rt::new (framework bug)");
    new_collection(rt, proto, false, Store::default())
}

/// A new, empty Set with %Set.prototype%.
pub fn new_set(rt: &Rt) -> ObjRef {
    let proto = rt.builtin("Set.prototype").expect("Set.prototype is installed by Rt::new (framework bug)");
    new_collection(rt, proto, true, Store::default())
}

/// Adds an entry to a Map, replacing the value of an equal key, without calling `set`. False when `obj` is not a Map.
pub fn map_put(rt: &Rt, obj: ObjRef, key: Value, value: Value) -> bool {
    let Some(p) = rt.native_payload(obj) else {
        return false;
    };
    match payload_ref::<Collection>(&*p) {
        Some(c) if !c.is_set => {
            c.store.borrow_mut().set(canon(key), value);
            true
        }
        _ => false,
    }
}

/// Adds a value to a Set without calling `add`. False when `obj` is not a Set.
pub fn set_put(rt: &Rt, obj: ObjRef, value: Value) -> bool {
    let Some(p) = rt.native_payload(obj) else {
        return false;
    };
    match payload_ref::<Collection>(&*p) {
        Some(c) if c.is_set => {
            c.store.borrow_mut().set(canon(value), Value::Undefined);
            true
        }
        _ => false,
    }
}

/// The entries of a Map object in insertion order, or None when `obj` is not a Map.
pub fn map_entries(rt: &Rt, obj: ObjRef) -> Option<Vec<(Value, Value)>> {
    let p = rt.native_payload(obj)?;
    let c = payload_ref::<Collection>(&*p)?;
    if c.is_set {
        return None;
    }
    Some(c.store.borrow().live())
}

/// The values of a Set object in insertion order, or None when `obj` is not a Set.
pub fn set_values(rt: &Rt, obj: ObjRef) -> Option<Vec<Value>> {
    let p = rt.native_payload(obj)?;
    let c = payload_ref::<Collection>(&*p)?;
    if !c.is_set {
        return None;
    }
    Some(c.store.borrow().live().into_iter().map(|(k, _)| k).collect())
}

/// Installs `name` on `obj` as a builtin method (writable, configurable, non-enumerable) and returns its function.
fn method(rt: &Rt, obj: ObjRef, name: &str, code: CodeFn, length: u32) -> ObjRef {
    let f = rt.new_native_function(code, name, length, Vec::new());
    rt.define_builtin(obj, PropKey::from_str(name), Prop::Data { value: Value::Object(f), flags: Flags::HIDDEN });
    f
}

/// Makes `key` refer to an existing function object (an alias, such as `Set.prototype.keys === values`).
fn alias(rt: &Rt, obj: ObjRef, key: PropKey, f: ObjRef) {
    rt.define_builtin(obj, key, Prop::Data { value: Value::Object(f), flags: Flags::HIDDEN });
}

/// `prototype` (non-writable, non-enumerable, non-configurable) and `prototype.constructor`.
fn link_constructor(rt: &Rt, ctor: ObjRef, proto: ObjRef) {
    rt.define_builtin(ctor, PropKey::from_str("prototype"), Prop::Data { value: Value::Object(proto), flags: Flags::NONE });
    rt.define_builtin(proto, PropKey::from_str("constructor"), Prop::Data { value: Value::Object(ctor), flags: Flags::HIDDEN });
}

fn install_map(rt: &Rt) {
    let proto = rt.new_object();
    rt.register_builtin("Map.prototype", proto);
    let ctor = rt.new_native_constructor(map_call, map_construct, "Map", 0, Vec::new());
    link_constructor(rt, ctor, proto);
    method(rt, ctor, "groupBy", map_group_by, 2);
    method(rt, proto, "get", map_get, 1);
    method(rt, proto, "set", map_set, 2);
    method(rt, proto, "has", map_has, 1);
    method(rt, proto, "delete", map_delete, 1);
    method(rt, proto, "clear", map_clear, 0);
    let entries = method(rt, proto, "entries", map_entries_method, 0);
    method(rt, proto, "forEach", map_for_each, 1);
    method(rt, proto, "keys", map_keys, 0);
    rt.define_native_getter(proto, "size", map_size);
    method(rt, proto, "values", map_values, 0);
    rt.define_builtin(
        proto,
        PropKey::Sym(rt.well_known(WellKnown::ToStringTag)),
        Prop::Data { value: Value::str("Map"), flags: Flags::CONFIG },
    );
    alias(rt, proto, PropKey::Sym(rt.well_known(WellKnown::Iterator)), entries);
    rt.install_global("Map", Value::Object(ctor));
    iterator::make_iterator_prototype(rt, "MapIteratorPrototype", "Map Iterator");
}

fn install_set(rt: &Rt) {
    let proto = rt.new_object();
    rt.register_builtin("Set.prototype", proto);
    let ctor = rt.new_native_constructor(set_call, set_construct, "Set", 0, Vec::new());
    link_constructor(rt, ctor, proto);
    method(rt, proto, "has", set_has, 1);
    method(rt, proto, "add", set_add, 1);
    method(rt, proto, "delete", set_delete, 1);
    method(rt, proto, "difference", set_difference, 1);
    method(rt, proto, "clear", set_clear, 0);
    method(rt, proto, "entries", set_entries, 0);
    method(rt, proto, "forEach", set_for_each, 1);
    method(rt, proto, "intersection", set_intersection, 1);
    method(rt, proto, "isSubsetOf", set_is_subset_of, 1);
    method(rt, proto, "isSupersetOf", set_is_superset_of, 1);
    method(rt, proto, "isDisjointFrom", set_is_disjoint_from, 1);
    rt.define_native_getter(proto, "size", set_size);
    method(rt, proto, "symmetricDifference", set_symmetric_difference, 1);
    method(rt, proto, "union", set_union, 1);
    let values = method(rt, proto, "values", set_values_method, 0);
    alias(rt, proto, PropKey::from_str("keys"), values);
    rt.define_builtin(
        proto,
        PropKey::Sym(rt.well_known(WellKnown::ToStringTag)),
        Prop::Data { value: Value::str("Set"), flags: Flags::CONFIG },
    );
    alias(rt, proto, PropKey::Sym(rt.well_known(WellKnown::Iterator)), values);
    rt.install_global("Set", Value::Object(ctor));
    iterator::make_iterator_prototype(rt, "SetIteratorPrototype", "Set Iterator");
}

/// Installs Map, Set and their iterator prototypes. Needs %IteratorPrototype% (the iterator family installs first).
pub fn install(rt: &Rt) {
    install_map(rt);
    install_set(rt);
}
