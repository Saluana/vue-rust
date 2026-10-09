//! The runtime handle `Rt`: realm bootstrap, intrinsics, symbols, the job queue, the watchdog and the
//! ECMAScript coercions. `Rt` is a cheap clone of an `Rc`. Every inner borrow is short and released before
//! user, getter or host code runs (reentrancy is normal).

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use crate::error;
use crate::function::{CodeFn, FunctionData, FnKind, function_prototype_code};
use crate::heap::{Heap, PROP_BYTES};
use crate::native::{Exotic, NativeObject};
use crate::jsstr::JsStr;
use crate::number;
use crate::object::is_length;
use crate::object::{Flags, Obj, ObjectKind, Prop, MAX_DENSE_LEN};
use crate::roots::RootTable;
use crate::roots::RootProvider;
use crate::value::{ObjRef, PropKey, Res, Sym, Throw, Value};

/// Default maximum nesting of `call` and `construct`.
pub const DEFAULT_MAX_CALL_DEPTH: u32 = 1500;
/// Default number of allocations `maybe_collect` waits for.
pub const DEFAULT_GC_THRESHOLD: usize = 10_000;

/// Objects every realm has. Discriminants index the intrinsic table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intrinsic {
    ObjectPrototype = 0,
    FunctionPrototype,
    ArrayPrototype,
    StringPrototype,
    NumberPrototype,
    BooleanPrototype,
    SymbolPrototype,
    ErrorPrototype,
    TypeErrorPrototype,
    RangeErrorPrototype,
    ReferenceErrorPrototype,
    SyntaxErrorPrototype,
    EvalErrorPrototype,
    UriErrorPrototype,
    ErrorCtor,
    TypeErrorCtor,
    RangeErrorCtor,
    ReferenceErrorCtor,
    SyntaxErrorCtor,
    EvalErrorCtor,
    UriErrorCtor,
    Global,
}

pub const INTRINSIC_COUNT: usize = 22;
const _: () = assert!(Intrinsic::Global as usize + 1 == INTRINSIC_COUNT);

/// Well-known symbols. They are the first symbols created, so `Sym(w as u32)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WellKnown {
    Iterator = 0,
    AsyncIterator,
    ToPrimitive,
    ToStringTag,
    HasInstance,
}

const WELL_KNOWN_NAMES: [&str; 5] = [
    "Symbol.iterator",
    "Symbol.asyncIterator",
    "Symbol.toPrimitive",
    "Symbol.toStringTag",
    "Symbol.hasInstance",
];

/// A queued job: `func(...args)` run with `this = undefined`.
pub(crate) struct Job {
    pub(crate) func: Value,
    pub(crate) args: Vec<Value>,
}

pub(crate) struct RtState {
    pub(crate) heap: RefCell<Heap>,
    pub(crate) intrinsics: RefCell<Vec<ObjRef>>,
    /// Symbol table: index is `Sym.0`, the value is the description (immortal entries).
    pub(crate) symbols: RefCell<Vec<Option<JsStr>>>,
    pub(crate) symbol_registry: RefCell<HashMap<JsStr, Sym>>,
    pub(crate) roots: RefCell<RootTable>,
    pub(crate) providers: RefCell<Vec<Option<Rc<dyn RootProvider>>>>,
    pub(crate) jobs: RefCell<VecDeque<Job>>,
    pub(crate) job_errors: RefCell<Vec<Throw>>,
    pub(crate) depth: Cell<u32>,
    pub(crate) max_depth: Cell<u32>,
    pub(crate) heap_limit: Cell<Option<usize>>,
    pub(crate) budget: Cell<Option<u64>>,
    pub(crate) budget_used: Cell<u64>,
    pub(crate) gc_threshold: Cell<usize>,
    /// Named intrinsics of builtin families that are not in `Intrinsic` (iterator prototypes, Map, Promise, ...).
    pub(crate) registry: RefCell<HashMap<&'static str, ObjRef>>,
}

/// How `to_primitive` chooses between `valueOf` and `toString`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hint {
    Default,
    Number,
    String,
}

#[derive(Clone)]
pub struct Rt(pub(crate) Rc<RtState>);

impl Rt {
    /// Creates a realm: global object, the core prototypes, the Error hierarchy and the well-known symbols.
    pub fn new() -> Rt {
        let st = RtState {
            heap: RefCell::new(Heap::default()),
            intrinsics: RefCell::new(Vec::new()),
            symbols: RefCell::new(Vec::new()),
            symbol_registry: RefCell::new(HashMap::new()),
            roots: RefCell::new(RootTable::default()),
            providers: RefCell::new(Vec::new()),
            jobs: RefCell::new(VecDeque::new()),
            job_errors: RefCell::new(Vec::new()),
            depth: Cell::new(0),
            max_depth: Cell::new(DEFAULT_MAX_CALL_DEPTH),
            heap_limit: Cell::new(None),
            budget: Cell::new(None),
            budget_used: Cell::new(0),
            gc_threshold: Cell::new(DEFAULT_GC_THRESHOLD),
            registry: RefCell::new(HashMap::new()),
        };
        let rt = Rt(Rc::new(st));
        for name in WELL_KNOWN_NAMES {
            rt.new_symbol(Some(JsStr::from_str(name)));
        }
        rt.bootstrap();
        crate::builtins::install_all(&rt);
        rt
    }

    fn bootstrap(&self) {
        // Every intrinsic exists before any is wired up, so constructors can reference each other.
        for _ in 0..INTRINSIC_COUNT {
            let r = self.alloc(Obj::new(None, ObjectKind::Ordinary));
            self.0.intrinsics.borrow_mut().push(r);
        }
        let op = self.intrinsic(Intrinsic::ObjectPrototype);
        for i in 1..INTRINSIC_COUNT {
            let r = self.0.intrinsics.borrow()[i];
            self.with_obj_mut(r, |o| o.proto = Some(op));
        }
        let fp = self.intrinsic(Intrinsic::FunctionPrototype);
        self.fill_function(
            fp,
            FunctionData {
                code: function_prototype_code,
                captures: Rc::from(Vec::<ObjRef>::new()),
                data: Rc::from(Vec::<Value>::new()),
                name: JsStr::empty(),
                length: 0,
                kind: FnKind::Normal,
                ctor: None,
            },
        );
        let ap = self.intrinsic(Intrinsic::ArrayPrototype);
        self.with_obj_mut(ap, |o| o.kind = ObjectKind::Array(Vec::new()));
        let global = self.intrinsic(Intrinsic::Global);
        self.install_global("globalThis", Value::Object(global));
        error::install(self);
        crate::coerce::install(self);
    }

    /// Allocates a heap object, accounting its estimated size.
    pub(crate) fn alloc(&self, obj: Obj) -> ObjRef {
        let bytes = obj.estimate_bytes();
        self.0.heap.borrow_mut().alloc(obj, bytes)
    }

    /// Reads a live object. The closure must not call back into the runtime.
    pub(crate) fn with_obj<R>(&self, r: ObjRef, f: impl FnOnce(&Obj) -> R) -> R {
        let h = self.0.heap.borrow();
        f(h.get(r))
    }

    /// Mutates a live object. The closure must not call back into the runtime.
    pub(crate) fn with_obj_mut<R>(&self, r: ObjRef, f: impl FnOnce(&mut Obj) -> R) -> R {
        let mut h = self.0.heap.borrow_mut();
        f(h.get_mut(r))
    }

    pub fn intrinsic(&self, i: Intrinsic) -> ObjRef {
        self.0.intrinsics.borrow()[i as usize]
    }

    /// Registers a named intrinsic of a builtin family (e.g. "IteratorPrototype", "Map.prototype"). It is a GC root.
    pub fn register_builtin(&self, name: &'static str, obj: ObjRef) {
        self.0.registry.borrow_mut().insert(name, obj);
    }

    /// Looks up a named builtin intrinsic registered by `register_builtin`.
    pub fn builtin(&self, name: &str) -> Option<ObjRef> {
        self.0.registry.borrow().get(name).copied()
    }

    pub(crate) fn set_intrinsic(&self, i: Intrinsic, r: ObjRef) {
        self.0.intrinsics.borrow_mut()[i as usize] = r;
    }

    pub fn new_object(&self) -> ObjRef {
        self.new_object_with_proto(Some(self.intrinsic(Intrinsic::ObjectPrototype)))
    }

    pub fn new_object_with_proto(&self, proto: Option<ObjRef>) -> ObjRef {
        self.alloc(Obj::new(proto, ObjectKind::Ordinary))
    }

    /// A dense array with the given elements. Panics above `MAX_DENSE_LEN` (a framework bug: the caller
    /// built the vector).
    pub fn new_array(&self, elems: Vec<Value>) -> ObjRef {
        assert!(elems.len() <= MAX_DENSE_LEN, "new_array beyond the dense limit (framework bug)");
        self.alloc(Obj::new(Some(self.intrinsic(Intrinsic::ArrayPrototype)), ObjectKind::Array(elems)))
    }

    /// Adds or replaces an own property of an ordinary object during setup (no checks: the object must be
    /// extensible and the key must be allowed to change, which holds for builtins).
    pub(crate) fn define_builtin(&self, obj: ObjRef, key: PropKey, prop: Prop) {
        let added = self.with_obj_mut(obj, |o| match o.props.get_mut(&key) {
            Some(p) => {
                *p = prop;
                false
            }
            None => {
                o.props.push(key, prop);
                true
            }
        });
        if added {
            self.0.heap.borrow_mut().add_bytes(obj, PROP_BYTES);
        }
    }

    /// Installs a builtin method (writable, configurable, non-enumerable) on `obj`.
    pub fn define_native_method(&self, obj: ObjRef, name: &str, code: CodeFn, length: u32) {
        let f = self.new_native_function(code, name, length, Vec::new());
        self.define_builtin(obj, PropKey::from_str(name), Prop::Data { value: Value::Object(f), flags: Flags::HIDDEN });
    }

    /// Installs a builtin getter (`get <name>`, configurable, non-enumerable) on `obj`.
    pub fn define_native_getter(&self, obj: ObjRef, name: &str, code: CodeFn) {
        let g = self.new_native_function(code, &format!("get {name}"), 0, Vec::new());
        self.define_builtin(
            obj,
            PropKey::from_str(name),
            Prop::Accessor { get: Some(g), set: None, flags: Flags::new(false, false, true) },
        );
    }

    /// Defines a property of the global object (writable, configurable, non-enumerable).
    pub fn install_global(&self, name: &str, v: Value) {
        let g = self.intrinsic(Intrinsic::Global);
        self.define_builtin(g, PropKey::from_str(name), Prop::Data { value: v, flags: Flags::HIDDEN });
    }

    /// A property key for a static name (already normalised: `"0"` becomes `PropKey::Index(0)`).
    pub fn key(&self, name: &'static str) -> PropKey {
        PropKey::from_str(name)
    }

    // ----- symbols -----

    pub fn well_known(&self, w: WellKnown) -> Sym {
        Sym(w as u32)
    }

    /// A fresh unique symbol.
    pub fn new_symbol(&self, description: Option<JsStr>) -> Sym {
        let mut tab = self.0.symbols.borrow_mut();
        tab.push(description);
        Sym((tab.len() - 1) as u32)
    }

    /// `Symbol.for`: the registered symbol for `key`, created on first use.
    pub fn symbol_for(&self, key: &JsStr) -> Sym {
        let found = self.0.symbol_registry.borrow().get(key).copied();
        if let Some(s) = found {
            return s;
        }
        let s = self.new_symbol(Some(key.clone()));
        self.0.symbol_registry.borrow_mut().insert(key.clone(), s);
        s
    }

    pub fn symbol_description(&self, s: Sym) -> Option<JsStr> {
        self.0.symbols.borrow()[s.0 as usize].clone()
    }

    // ----- jobs and the watchdog -----

    /// Queues a job. Queued values are GC roots until the job runs.
    pub fn enqueue_job(&self, func: Value, args: Vec<Value>) {
        self.0.jobs.borrow_mut().push_back(Job { func, args });
    }

    /// Runs queued jobs until the queue is empty, including jobs queued while draining. A job that throws
    /// does not stop the drain; its error is kept for `take_uncaught_job_errors`. Never collects garbage.
    pub fn run_jobs(&self) -> usize {
        assert_eq!(self.call_depth(), 0, "run_jobs inside running code (framework bug)");
        let mut ran = 0;
        loop {
            let next = self.0.jobs.borrow_mut().pop_front();
            let Some(job) = next else { break };
            self.0.budget_used.set(0);
            ran += 1;
            if let Err(e) = self.call(&job.func, &Value::Undefined, &job.args) {
                self.0.job_errors.borrow_mut().push(e);
            }
        }
        ran
    }

    pub fn take_uncaught_job_errors(&self) -> Vec<Throw> {
        std::mem::take(&mut *self.0.job_errors.borrow_mut())
    }

    /// Soft heap cap: `tick` throws RangeError("heap limit exceeded") while the estimate is above it.
    pub fn set_heap_limit(&self, bytes: usize) {
        self.0.heap_limit.set(Some(bytes));
    }

    pub fn clear_heap_limit(&self) {
        self.0.heap_limit.set(None);
    }

    /// Maximum `tick` calls per job run by `run_jobs` (the counter resets before each job).
    pub fn set_execution_budget(&self, max_ticks: u64) {
        self.0.budget.set(Some(max_ticks));
    }

    pub fn clear_execution_budget(&self) {
        self.0.budget.set(None);
    }

    /// Counts one unit of work and checks the heap limit and the execution budget. Generated code calls it
    /// on function entry and loop back-edges.
    pub fn tick(&self) -> Res<()> {
        let st = &self.0;
        let used = st.budget_used.get() + 1;
        st.budget_used.set(used);
        if let Some(limit) = st.heap_limit.get() {
            if st.heap.borrow().bytes > limit {
                return Err(self.throw_range_error("heap limit exceeded"));
            }
        }
        if let Some(max) = st.budget.get() {
            if used > max {
                return Err(self.throw_range_error("execution budget exceeded"));
            }
        }
        Ok(())
    }

    // ----- coercions -----

    /// Short description of a value for diagnostics (not part of the JavaScript semantics).
    pub(crate) fn describe(&self, v: &Value) -> String {
        match v {
            Value::Undefined => "undefined".to_string(),
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => number::number_to_string(*n),
            Value::String(s) => format!("\"{}\"", s.to_rust_string_lossy()),
            Value::Symbol(s) => {
                let d = self.symbol_description(*s).map(|d| d.to_rust_string_lossy()).unwrap_or_default();
                format!("Symbol({d})")
            }
            Value::Object(_) if self.is_callable(v) => "function".to_string(),
            Value::Object(_) => "#<Object>".to_string(),
            Value::Uninit | Value::Hole => "<internal>".to_string(),
        }
    }

    /// The `typeof` result.
    pub fn type_of(&self, v: &Value) -> &'static str {
        match v {
            Value::Undefined => "undefined",
            Value::Null => "object",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Symbol(_) => "symbol",
            Value::Object(_) if self.is_callable(v) => "function",
            Value::Object(_) => "object",
            Value::Uninit | Value::Hole => panic!("internal marker reached typeof (framework bug)"),
        }
    }

    /// ToPrimitive (ECMA-262 7.1.1), including `Symbol.toPrimitive` and OrdinaryToPrimitive.
    pub fn to_primitive(&self, v: &Value, hint: Hint) -> Res<Value> {
        let Value::Object(o) = v else { return Ok(v.clone()) };
        let exotic = self.get(*o, &PropKey::Sym(self.well_known(WellKnown::ToPrimitive)), v)?;
        if !exotic.is_nullish() {
            if !self.is_callable(&exotic) {
                return Err(self.throw_type_error("Symbol.toPrimitive is not a function"));
            }
            let h = match hint {
                Hint::Default => "default",
                Hint::Number => "number",
                Hint::String => "string",
            };
            let r = self.call(&exotic, v, &[Value::str(h)])?;
            if r.is_object() {
                return Err(self.throw_type_error("Cannot convert object to primitive value"));
            }
            return Ok(r);
        }
        let order = if hint == Hint::String { ["toString", "valueOf"] } else { ["valueOf", "toString"] };
        for name in order {
            let m = self.get(*o, &PropKey::from_str(name), v)?;
            if self.is_callable(&m) {
                let r = self.call(&m, v, &[])?;
                if !r.is_object() {
                    return Ok(r);
                }
            }
        }
        Err(self.throw_type_error("Cannot convert object to primitive value"))
    }

    /// ToNumber (ECMA-262 7.1.4).
    pub fn to_number(&self, v: &Value) -> Res<f64> {
        Ok(match v {
            Value::Undefined => f64::NAN,
            Value::Null => 0.0,
            Value::Bool(b) => f64::from(u8::from(*b)),
            Value::Number(n) => *n,
            Value::String(s) => number::string_to_number(s),
            Value::Symbol(_) => return Err(self.throw_type_error("Cannot convert a Symbol value to a number")),
            Value::Object(_) => {
                let p = self.to_primitive(v, Hint::Number)?;
                return self.to_number(&p);
            }
            Value::Uninit | Value::Hole => panic!("internal marker reached ToNumber (framework bug)"),
        })
    }

    /// ToString (ECMA-262 7.1.17).
    pub fn to_string(&self, v: &Value) -> Res<JsStr> {
        Ok(match v {
            Value::Undefined => JsStr::from_str("undefined"),
            Value::Null => JsStr::from_str("null"),
            Value::Bool(b) => JsStr::from_str(if *b { "true" } else { "false" }),
            Value::Number(n) => JsStr::from_str(&number::number_to_string(*n)),
            Value::String(s) => s.clone(),
            Value::Symbol(_) => return Err(self.throw_type_error("Cannot convert a Symbol value to a string")),
            Value::Object(_) => {
                let p = self.to_primitive(v, Hint::String)?;
                return self.to_string(&p);
            }
            Value::Uninit | Value::Hole => panic!("internal marker reached ToString (framework bug)"),
        })
    }

    /// ToPropertyKey (ECMA-262 7.1.19).
    pub fn to_property_key(&self, v: &Value) -> Res<PropKey> {
        match v {
            Value::Symbol(s) => Ok(PropKey::Sym(*s)),
            Value::String(s) => Ok(PropKey::from_jsstr(s.clone())),
            Value::Object(_) => {
                let p = self.to_primitive(v, Hint::String)?;
                self.to_property_key(&p)
            }
            _ => Ok(PropKey::from_jsstr(self.to_string(v)?)),
        }
    }
}

impl Default for Rt {
    fn default() -> Self {
        Rt::new()
    }
}

impl Rt {
    /// An object whose kind is `ObjectKind::Native(payload)`. The payload must trace every `ObjRef` it holds.
    pub fn new_native_object(&self, proto: Option<ObjRef>, payload: Rc<dyn NativeObject>) -> ObjRef {
        self.alloc(Obj::new(proto, ObjectKind::Native(payload)))
    }

    /// An object whose kind is `ObjectKind::Exotic(hooks)`: proxy-like behaviour (see `Exotic`).
    pub fn new_exotic_object(&self, proto: Option<ObjRef>, hooks: Rc<dyn Exotic>) -> ObjRef {
        self.alloc(Obj::new(proto, ObjectKind::Exotic(hooks)))
    }

    /// The payload of a `Native` object, or `None` for other kinds.
    pub fn native_payload(&self, o: ObjRef) -> Option<Rc<dyn NativeObject>> {
        self.with_obj(o, |ob| match &ob.kind {
            ObjectKind::Native(p) => Some(p.clone()),
            _ => None,
        })
    }

    /// Sets the private extension slot of any object (subsystem data, traced by the collector).
    pub fn set_ext(&self, o: ObjRef, payload: Rc<dyn NativeObject>) {
        self.with_obj_mut(o, |ob| ob.ext = Some(payload));
    }

    pub fn ext(&self, o: ObjRef) -> Option<Rc<dyn NativeObject>> {
        self.with_obj(o, |ob| ob.ext.clone())
    }
}

// ----- value-level property access (wrappers over the object operations in object.rs) -----

impl Rt {
    /// The prototype used for property lookup on a primitive base (None for objects and null/undefined).
    fn proto_for_primitive(&self, base: &Value) -> Option<ObjRef> {
        let i = match base {
            Value::String(_) => Intrinsic::StringPrototype,
            Value::Number(_) => Intrinsic::NumberPrototype,
            Value::Bool(_) => Intrinsic::BooleanPrototype,
            Value::Symbol(_) => Intrinsic::SymbolPrototype,
            _ => return None,
        };
        Some(self.intrinsic(i))
    }

    /// GetValue on any base: objects, strings (`length`, indices), other primitives via their prototypes.
    pub fn get_v(&self, base: &Value, key: &PropKey) -> Res<Value> {
        match base {
            Value::Object(o) => self.get(*o, key, base),
            Value::String(s) => {
                match key {
                    k if is_length(k) => return Ok(Value::Number(s.len() as f64)),
                    PropKey::Index(i) if (*i as usize) < s.len() => {
                        let i = *i as usize;
                        return Ok(Value::String(JsStr::from_units(&s.units()[i..i + 1])));
                    }
                    _ => {}
                }
                self.get(self.intrinsic(Intrinsic::StringPrototype), key, base)
            }
            Value::Undefined | Value::Null => {
                let what = if matches!(base, Value::Null) { "null" } else { "undefined" };
                Err(self.throw_type_error(&format!(
                    "Cannot read properties of {what} (reading '{}')",
                    self.key_text(key)
                )))
            }
            Value::Uninit | Value::Hole => panic!("internal marker used as a value (framework bug)"),
            other => {
                let proto = self.proto_for_primitive(other).expect("primitive has a prototype");
                self.get(proto, key, base)
            }
        }
    }

    /// PutValue on any base. Returns false when refused; `set_v_strict` throws instead.
    pub fn set_v(&self, base: &Value, key: &PropKey, v: Value) -> Res<bool> {
        match base {
            Value::Object(o) => self.set(*o, key, v, base),
            Value::Undefined | Value::Null => {
                let what = if matches!(base, Value::Null) { "null" } else { "undefined" };
                Err(self.throw_type_error(&format!(
                    "Cannot set properties of {what} (setting '{}')",
                    self.key_text(key)
                )))
            }
            Value::Uninit | Value::Hole => panic!("internal marker used as a value (framework bug)"),
            other => {
                let proto = self.proto_for_primitive(other).expect("primitive has a prototype");
                self.set(proto, key, v, other)
            }
        }
    }

    /// The TypeError for a refused assignment (strict mode).
    fn failed_assignment(&self, base: &Value, key: &PropKey) -> Throw {
        let k = self.key_text(key);
        match base {
            Value::Object(o) => {
                let absent = !self.is_extensible(*o) && matches!(self.has_property(*o, key), Ok(false));
                if absent {
                    self.throw_type_error(&format!("Cannot add property {k}, object is not extensible"))
                } else {
                    self.throw_type_error(&format!("Cannot assign to read only property '{k}' of object"))
                }
            }
            Value::String(s) if is_length(key) || matches!(key, PropKey::Index(i) if (*i as usize) < s.len()) => {
                self.throw_type_error(&format!(
                    "Cannot assign to read only property '{k}' of string '{}'",
                    s.to_rust_string_lossy()
                ))
            }
            other => {
                let shown = match other {
                    Value::String(s) => s.to_rust_string_lossy(),
                    v => self.describe(v),
                };
                self.throw_type_error(&format!(
                    "Cannot create property '{k}' on {} '{shown}'",
                    self.type_of(other)
                ))
            }
        }
    }

    /// Strict-mode [[Set]] on an object: a refused assignment throws TypeError.
    pub fn set_strict(&self, o: ObjRef, key: &PropKey, v: Value) -> Res<()> {
        let recv = Value::Object(o);
        if self.set(o, key, v, &recv)? {
            return Ok(());
        }
        Err(self.failed_assignment(&recv, key))
    }

    /// Strict-mode PutValue on any base.
    pub fn set_v_strict(&self, base: &Value, key: &PropKey, v: Value) -> Res<()> {
        if self.set_v(base, key, v)? {
            return Ok(());
        }
        Err(self.failed_assignment(base, key))
    }

    /// `base[name]` with a static-style name.
    pub fn get_str(&self, base: &Value, name: &str) -> Res<Value> {
        self.get_v(base, &PropKey::from_str(name))
    }

    /// `base[name] = v`, returning whether the assignment was accepted.
    pub fn set_str(&self, base: &Value, name: &str, v: Value) -> Res<bool> {
        self.set_v(base, &PropKey::from_str(name), v)
    }
}
