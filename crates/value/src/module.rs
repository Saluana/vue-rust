//! ECMAScript module records (ECMA-262 16.2.1, restricted to the WP-L2c profile): the declare, bind and
//! evaluate phases come from generated code; the framework owns the graph walk, cycles, live bindings,
//! error caching and namespace objects. The rules are documented in ABI.md ("Modules").
//!
//! Storage: a `Registry` is the payload of the global object's extension slot, so the collector reaches it
//! through a root. Each record is a `Native` heap object whose `Record` payload traces its cells, locals,
//! namespace and thrown value. Module functions run at call depth >= 1, as other compiled code does.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::builtins::promise::{new_promise, reject_promise, resolve_promise};
use crate::error::ErrorKind;
use crate::function::Call;
use crate::jsstr::JsStr;
use crate::native::{Exotic, Hook, NativeObject};
use crate::object::{Flags, Prop};
use crate::roots::Tracer;
use crate::rt::{Intrinsic, Rt, WellKnown};
use crate::value::{same_value, ObjRef, PropKey, Res, Throw, Value};

/// Static description of one compiled module. Generated code supplies one per module.
pub struct ModuleDef {
    /// Canonical id, e.g. "/src/App.vue" or "vue".
    pub specifier: &'static str,
    /// Specifiers of every import and re-export, in source order, duplicates removed.
    pub requests: &'static [&'static str],
    /// Phase 1: create the cells, call `module_set_locals` and `module_define_export`.
    pub declare: fn(&Rt, ObjRef) -> Res<()>,
    /// Phase 2: resolve imports with `module_import_cell`, register re-exports, initialise hoisted functions.
    pub bind: fn(&Rt, ObjRef) -> Res<()>,
    /// Phase 3: top-level code, synchronous (no top-level await in this profile).
    pub evaluate: fn(&Rt, ObjRef) -> Res<()>,
}

/// Lifecycle of a module record. `Errored` keeps the thrown value (see `load_module`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleStatus {
    New,
    Declared,
    Bound,
    Evaluating,
    Evaluated,
    Errored,
}

/// Definitions and records by specifier. Payload of the global object's extension slot.
#[derive(Default)]
struct Registry {
    defs: RefCell<HashMap<String, &'static ModuleDef>>,
    records: RefCell<HashMap<String, ObjRef>>,
    /// Depth of the declare/bind phases; `load_module` refuses to start a graph while it is non-zero.
    linking: Cell<u32>,
}

impl NativeObject for Registry {
    fn trace(&self, t: &mut Tracer) {
        for r in self.records.borrow().values() {
            t.mark(*r);
        }
    }
}

/// A re-export `export { import as export } from request`.
struct Indirect {
    export: JsStr,
    request: String,
    import: JsStr,
}

struct State {
    status: ModuleStatus,
    /// The thrown value of an `Errored` record.
    error: Option<Throw>,
    /// Whether the error happened while linking. Such a record cannot be bound again.
    link_error: bool,
    locals: Option<Vec<ObjRef>>,
    exports: HashMap<JsStr, ObjRef>,
    indirect: Vec<Indirect>,
    stars: Vec<String>,
    namespace: Option<ObjRef>,
}

impl State {
    fn new(status: ModuleStatus) -> State {
        State {
            status,
            error: None,
            link_error: false,
            locals: None,
            exports: HashMap::new(),
            indirect: Vec::new(),
            stars: Vec::new(),
            namespace: None,
        }
    }
}

/// Payload of a module record object.
struct Record {
    spec: String,
    def: Option<&'static ModuleDef>,
    st: RefCell<State>,
}

impl NativeObject for Record {
    fn trace(&self, t: &mut Tracer) {
        let st = self.st.borrow();
        if let Some(e) = &st.error {
            t.mark_value(&e.0);
        }
        for c in st.locals.iter().flatten() {
            t.mark(*c);
        }
        for c in st.exports.values() {
            t.mark(*c);
        }
        if let Some(n) = st.namespace {
            t.mark(n);
        }
    }
}

/// One entry of a namespace object: the export's name (code units) and the cell it reads.
struct NsEntry {
    name: JsStr,
    cell: ObjRef,
}

/// Exotic behaviour of a module namespace object (ECMA-262 10.4.6). The export list is computed on first
/// use, after linking, and then kept.
struct Namespace {
    module: ObjRef,
    names: RefCell<Option<Vec<NsEntry>>>,
}

/// Outcome of ResolveExport (ECMA-262 16.2.1.6.3). A cycle counts as NotFound.
enum Resolved {
    Found(ObjRef),
    NotFound,
    Ambiguous,
}

/// Downcasts a native payload to its concrete type.
fn cast<T: NativeObject>(p: Rc<dyn NativeObject>) -> Option<Rc<T>> {
    let any: Rc<dyn Any> = p;
    any.downcast::<T>().ok()
}

/// Keeps the call depth raised while module code runs.
struct CodeScope<'a>(&'a Cell<u32>);

impl Drop for CodeScope<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

/// Keeps the linking depth raised for the declare and bind phases.
struct LinkScope(Rc<Registry>);

impl Drop for LinkScope {
    fn drop(&mut self) {
        self.0.linking.set(self.0.linking.get() - 1);
    }
}

fn assert_requested(rec: &Record, request: &str) {
    let listed = rec.def.is_some_and(|d| d.requests.iter().any(|r| *r == request));
    assert!(listed, "module '{}' uses '{request}' without listing it in requests (framework bug)", rec.spec);
}

/// The turns still to wait of a host turn of an import, from its argument `i`.
fn number_arg(c: &Call<'_>, i: usize) -> f64 {
    match c.arg(i) {
        Value::Number(n) => n,
        _ => panic!("import() turn without its turn count (framework bug)"),
    }
}

/// One host turn of an import (see `Rt::dynamic_import`). Args are the promise, the specifier and the turns still to
/// wait. A turn that is not the last queues the next one. The last loads and evaluates the module and settles the
/// promise with its namespace, or with the error that loading threw.
fn dynamic_turn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Value::Object(promise) = c.arg(0) else {
        panic!("import() turn without its promise (framework bug)")
    };
    let Value::String(spec) = c.arg(1) else {
        panic!("import() turn without its specifier (framework bug)")
    };
    let remaining = number_arg(c, 2);
    if remaining > 1.0 {
        let args = vec![Value::Object(promise), Value::String(spec), Value::Number(remaining - 1.0)];
        rt.enqueue_host_turn(Value::Object(c.callee), args);
        return Ok(Value::Undefined);
    }
    match rt.load_module(&spec.to_rust_string_lossy()) {
        Ok(m) => resolve_promise(rt, promise, Value::Object(rt.module_namespace(m))),
        Err(Throw(e)) => reject_promise(rt, promise, e),
    }
    Ok(Value::Undefined)
}

impl Rt {
    fn registry(&self) -> Rc<Registry> {
        let g = self.intrinsic(Intrinsic::Global);
        let slot: Rc<dyn NativeObject> = match self.ext(g) {
            Some(p) => p,
            None => {
                let r: Rc<dyn NativeObject> = Rc::new(Registry::default());
                self.set_ext(g, r.clone());
                r
            }
        };
        cast::<Registry>(slot).expect("global object extension slot holds a foreign payload (framework bug)")
    }

    fn record(&self, m: ObjRef) -> Rc<Record> {
        self.native_payload(m).and_then(cast::<Record>).expect("not a module record (framework bug)")
    }

    fn existing_record(&self, spec: &str) -> Option<ObjRef> {
        let reg = self.registry();
        let found = reg.records.borrow().get(spec).copied();
        found
    }

    /// The record of a specifier that is a request of a linked module. Missing means a framework bug.
    fn record_by_spec(&self, spec: &str) -> ObjRef {
        self.existing_record(spec).unwrap_or_else(|| panic!("module '{spec}' has no record (framework bug)"))
    }

    fn new_record(&self, specifier: &str, def: Option<&'static ModuleDef>) -> ObjRef {
        let rec = Rc::new(Record { spec: specifier.to_string(), def, st: RefCell::new(State::new(ModuleStatus::New)) });
        let m = self.new_native_object(None, rec);
        self.registry().records.borrow_mut().insert(specifier.to_string(), m);
        m
    }

    fn cannot_find(&self, spec: &str) -> Throw {
        self.throw_error(ErrorKind::Error, &format!("Cannot find module '{spec}'"))
    }

    /// Runs one module function at call depth >= 1, so no collection can run while its temporaries live.
    fn run(&self, f: fn(&Rt, ObjRef) -> Res<()>, m: ObjRef) -> Res<()> {
        let depth = &self.0.depth;
        depth.set(depth.get() + 1);
        let _scope = CodeScope(depth);
        f(self, m)
    }

    /// Phase 0 of `load_module`: appends the definitions to link, dependencies first. Nothing is created or
    /// run. A record that failed to link makes the whole load fail; linked or evaluated records are skipped.
    fn collect_link(&self, spec: &str, seen: &mut HashSet<String>, out: &mut Vec<&'static ModuleDef>) -> Res<()> {
        if !seen.insert(spec.to_string()) {
            return Ok(());
        }
        let def = match self.existing_record(spec) {
            Some(m) => {
                let rec = self.record(m);
                let (status, link_error, error) = {
                    let st = rec.st.borrow();
                    (st.status, st.link_error, st.error.clone())
                };
                if status == ModuleStatus::Errored && link_error {
                    return Err(error.expect("link error without a thrown value (framework bug)"));
                }
                if status != ModuleStatus::New {
                    return Ok(());
                }
                match rec.def {
                    Some(d) => d,
                    None => return Err(self.cannot_find(spec)),
                }
            }
            None => {
                let found = self.registry().defs.borrow().get(spec).copied();
                match found {
                    Some(d) => d,
                    None => return Err(self.cannot_find(spec)),
                }
            }
        };
        for req in def.requests {
            self.collect_link(req, seen, out)?;
        }
        out.push(def);
        Ok(())
    }

    /// Phases 1 and 2 for `defs` (dependencies first): every declare, then every bind. A failure is cached
    /// on the records that cannot be bound any more (see `link_failed`) and returned.
    fn link(&self, defs: &[&'static ModuleDef]) -> Res<()> {
        let reg = self.registry();
        reg.linking.set(reg.linking.get() + 1);
        let _scope = LinkScope(reg.clone());
        let mut created: Vec<(ObjRef, &'static ModuleDef)> = Vec::new();
        for &def in defs {
            let m = match self.existing_record(def.specifier) {
                Some(m) => m,
                None => self.new_record(def.specifier, Some(def)),
            };
            created.push((m, def));
            if let Err(e) = self.run(def.declare, m) {
                self.link_failed(&created, m, &e);
                return Err(e);
            }
            self.record(m).st.borrow_mut().status = ModuleStatus::Declared;
        }
        for &(m, def) in &created {
            if let Err(e) = self.run(def.bind, m) {
                self.link_failed(&created, m, &e);
                return Err(e);
            }
            self.record(m).st.borrow_mut().status = ModuleStatus::Bound;
        }
        Ok(())
    }

    /// Caches a link error on the failing record, on every record of this load that is not bound, and on
    /// every record that imports one of those. Other bound records stay loadable.
    fn link_failed(&self, created: &[(ObjRef, &'static ModuleDef)], failing: ObjRef, e: &Throw) {
        let mut bad: Vec<ObjRef> = vec![failing];
        loop {
            let mut changed = false;
            for &(m, def) in created {
                if bad.contains(&m) {
                    continue;
                }
                let not_bound = self.record(m).st.borrow().status != ModuleStatus::Bound;
                let needs_bad = def.requests.iter().any(|r| self.existing_record(r).is_some_and(|d| bad.contains(&d)));
                if not_bound || needs_bad {
                    bad.push(m);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for m in bad {
            let rec = self.record(m);
            let mut st = rec.st.borrow_mut();
            st.status = ModuleStatus::Errored;
            st.error = Some(e.clone());
            st.link_error = true;
        }
    }

    /// Phase 3 (ECMA-262 16.2.1.5.3.1, simplified): post-order over the requests. A module already on the
    /// stack is skipped. A throw is cached on the module and on every module waiting for it.
    fn evaluate_graph(&self, m: ObjRef) -> Res<()> {
        let rec = self.record(m);
        let status = rec.st.borrow().status;
        match status {
            ModuleStatus::Evaluated | ModuleStatus::Evaluating => return Ok(()),
            ModuleStatus::Errored => {
                let e = rec.st.borrow().error.clone().expect("errored module without a thrown value (framework bug)");
                return Err(e);
            }
            ModuleStatus::Bound => {}
            other => panic!("module '{}' evaluated in state {other:?} (framework bug)", rec.spec),
        }
        let def = rec.def.expect("bound module without a definition (framework bug)");
        rec.st.borrow_mut().status = ModuleStatus::Evaluating;
        for req in def.requests {
            let dep = self.record_by_spec(req);
            if let Err(e) = self.evaluate_graph(dep) {
                self.evaluation_failed(m, &e);
                return Err(e);
            }
        }
        if let Err(e) = self.run(def.evaluate, m) {
            self.evaluation_failed(m, &e);
            return Err(e);
        }
        rec.st.borrow_mut().status = ModuleStatus::Evaluated;
        Ok(())
    }

    fn evaluation_failed(&self, m: ObjRef, e: &Throw) {
        let rec = self.record(m);
        let mut st = rec.st.borrow_mut();
        st.status = ModuleStatus::Errored;
        st.error = Some(e.clone());
    }

    /// ResolveExport: local exports, then re-exports, then `export *` sources (never `default`). Two star
    /// sources that resolve to different cells make the name ambiguous.
    fn resolve(&self, m: ObjRef, name: &JsStr, set: &mut Vec<(ObjRef, JsStr)>) -> Resolved {
        if set.iter().any(|(x, n)| *x == m && n == name) {
            return Resolved::NotFound;
        }
        set.push((m, name.clone()));
        let rec = self.record(m);
        let (local, indirect, stars) = {
            let st = rec.st.borrow();
            let local = st.exports.get(name).copied();
            let indirect = st.indirect.iter().find(|i| &i.export == name).map(|i| (i.request.clone(), i.import.clone()));
            (local, indirect, st.stars.clone())
        };
        if let Some(c) = local {
            return Resolved::Found(c);
        }
        if let Some((request, import)) = indirect {
            let dep = self.record_by_spec(&request);
            return self.resolve(dep, &import, set);
        }
        if *name == JsStr::from_str("default") {
            return Resolved::NotFound;
        }
        let mut found: Option<ObjRef> = None;
        for request in stars {
            let dep = self.record_by_spec(&request);
            match self.resolve(dep, name, set) {
                Resolved::Ambiguous => return Resolved::Ambiguous,
                Resolved::NotFound => {}
                Resolved::Found(c) => match found {
                    None => found = Some(c),
                    Some(prev) if prev == c => {}
                    Some(_) => return Resolved::Ambiguous,
                },
            }
        }
        match found {
            Some(c) => Resolved::Found(c),
            None => Resolved::NotFound,
        }
    }

    /// GetExportedNames: local and re-exported names, then the names of `export *` sources except `default`.
    fn exported_names(&self, m: ObjRef, star_set: &mut Vec<ObjRef>, out: &mut Vec<JsStr>) {
        if star_set.contains(&m) {
            return;
        }
        star_set.push(m);
        let rec = self.record(m);
        let (own, stars) = {
            let st = rec.st.borrow();
            let own: Vec<JsStr> = st.exports.keys().cloned().chain(st.indirect.iter().map(|i| i.export.clone())).collect();
            (own, st.stars.clone())
        };
        for n in own {
            if !out.contains(&n) {
                out.push(n);
            }
        }
        let default = JsStr::from_str("default");
        for request in stars {
            let dep = self.record_by_spec(&request);
            let mut names = Vec::new();
            self.exported_names(dep, star_set, &mut names);
            for n in names {
                if n != default && !out.contains(&n) {
                    out.push(n);
                }
            }
        }
    }

    /// The namespace entries: every exported name that resolves unambiguously, sorted by code units.
    fn namespace_entries(&self, m: ObjRef) -> Vec<NsEntry> {
        let mut names = Vec::new();
        self.exported_names(m, &mut Vec::new(), &mut names);
        let mut out: Vec<NsEntry> = names
            .into_iter()
            .filter_map(|name| match self.resolve(m, &name, &mut Vec::new()) {
                Resolved::Found(cell) => Some(NsEntry { name, cell }),
                _ => None,
            })
            .collect();
        out.sort_by(|a, b| a.name.compare(&b.name));
        out
    }

    // ----- public API -----

    /// Registers the definition of a module by its specifier. Registering the same definition again is a
    /// no-op; any other duplicate, or a definition for a specifier that already has a record, is a bug.
    pub fn register_module_def(&self, def: &'static ModuleDef) {
        let reg = self.registry();
        let known = reg.defs.borrow().get(def.specifier).copied();
        match known {
            Some(old) if std::ptr::eq(old, def) => {}
            Some(_) => panic!("module '{}' registered twice (framework bug)", def.specifier),
            None => {
                assert!(
                    self.existing_record(def.specifier).is_none(),
                    "module '{}' has a record before its definition (framework bug)",
                    def.specifier
                );
                reg.defs.borrow_mut().insert(def.specifier.to_string(), def);
            }
        }
    }

    /// A record for `specifier` with no code of its own: status `New`, linked by `load_module` only when a
    /// definition is registered for it. Panics if the specifier already has a record.
    pub fn module_new(&self, specifier: &str) -> ObjRef {
        assert!(self.existing_record(specifier).is_none(), "module '{specifier}' already has a record (framework bug)");
        let def = self.registry().defs.borrow().get(specifier).copied();
        self.new_record(specifier, def)
    }

    /// A built-in module: status `Evaluated` at once, each export a constant binding in its own cell.
    pub fn register_native_module(&self, specifier: &str, exports: Vec<(&str, Value)>) -> ObjRef {
        assert!(self.existing_record(specifier).is_none(), "module '{specifier}' already exists (framework bug)");
        assert!(
            self.registry().defs.borrow().get(specifier).is_none(),
            "module '{specifier}' has a definition (framework bug)"
        );
        let m = self.new_record(specifier, None);
        self.record(m).st.borrow_mut().status = ModuleStatus::Evaluated;
        for (name, v) in exports {
            let cell = self.new_cell(v);
            self.module_define_export(m, name, cell);
        }
        m
    }

    /// Links and evaluates the graph rooted at `specifier` and returns its record.
    ///
    /// Idempotent: an evaluated module is returned without running again. A module whose evaluation threw
    /// is never run again: every later load of it, and of anything that imports it, rethrows the same value.
    /// A missing specifier throws `Error` "Cannot find module 'x'" before any code runs. A link error (a
    /// missing or ambiguous import) throws `SyntaxError` and is cached as well. Calling it while modules are
    /// being declared or bound throws.
    pub fn load_module(&self, specifier: &str) -> Res<ObjRef> {
        if let Some(m) = self.existing_record(specifier) {
            let rec = self.record(m);
            let (status, error) = {
                let st = rec.st.borrow();
                (st.status, st.error.clone())
            };
            match status {
                ModuleStatus::Evaluated | ModuleStatus::Evaluating => return Ok(m),
                ModuleStatus::Errored => return Err(error.expect("errored module without a thrown value (framework bug)")),
                _ => {}
            }
        }
        if self.registry().linking.get() > 0 {
            let msg = format!("cannot load module '{specifier}' while modules are being linked");
            return Err(self.throw_error(ErrorKind::Error, &msg));
        }
        let mut defs = Vec::new();
        self.collect_link(specifier, &mut HashSet::new(), &mut defs)?;
        self.link(&defs)?;
        let root = self.record_by_spec(specifier);
        self.evaluate_graph(root)?;
        Ok(root)
    }

    /// `import(specifier)` with a literal specifier (ECMA-262 13.3.10.1, prelinked profile). Returns the promise.
    ///
    /// Nothing runs during the call. The import waits for host turns (`dynamic_turn`), one per round trip that Node's
    /// loader makes with loader hooks (`import_turns`). A turn runs only when the job queue is empty, so the promise
    /// jobs queued before the import all run before its evaluation. The last turn loads and evaluates the module and
    /// settles the promise. A module that threw keeps its error, so a later import rejects with the same value.
    pub fn dynamic_import(&self, specifier: &str) -> ObjRef {
        let promise = new_promise(self);
        let args = vec![
            Value::Object(promise),
            Value::str(specifier),
            Value::Number(f64::from(self.import_turns(specifier))),
        ];
        let turn = self.import_turn_fn();
        self.enqueue_host_turn(Value::Object(turn), args);
        promise
    }

    /// The native function of the import turns, created on first use and registered as a builtin (a GC root).
    fn import_turn_fn(&self) -> ObjRef {
        if let Some(f) = self.builtin("module.import_turn") {
            return f;
        }
        let f = self.new_native_function(dynamic_turn, "", 3, Vec::new());
        self.register_builtin("module.import_turn", f);
        f
    }

    /// Host turns of an import of `specifier`, called now: one to resolve it, and the turns of `load_turns`. Measured
    /// on Node v22.22.0 with loader hooks, the project oracle (tests/language/projects/dyn-order and the probes in
    /// docs/evidence/dynamic-import.md): imports started together settle in the order of their turns. Requests of a
    /// module are counted one after another, which is not calibrated for parallel requests.
    fn import_turns(&self, specifier: &str) -> u32 {
        1 + self.load_turns(specifier, &mut Vec::new())
    }

    /// Turns to load `specifier`, none when it is loaded already: one to load its source, and for each of its requests
    /// one to resolve it plus the turns to load it when it is not loaded. A module already counted on this import's
    /// path is loaded once, so it adds nothing; its requests still cost a resolve each.
    fn load_turns(&self, specifier: &str, counted: &mut Vec<String>) -> u32 {
        if self.is_loaded(specifier) || counted.iter().any(|s| s == specifier) {
            return 0;
        }
        counted.push(specifier.to_string());
        let requests: &[&str] = self.def_of(specifier).map_or(&[], |d| d.requests);
        let mut turns = 1;
        for req in requests {
            turns += 1 + self.load_turns(req, counted);
        }
        turns
    }

    /// Whether `specifier` has a record past `New`: linked, evaluating, evaluated or errored.
    fn is_loaded(&self, specifier: &str) -> bool {
        self.existing_record(specifier).is_some_and(|m| self.module_status(m) != ModuleStatus::New)
    }

    /// The definition of `specifier`: the one its record holds, else the registered one.
    fn def_of(&self, specifier: &str) -> Option<&'static ModuleDef> {
        match self.existing_record(specifier) {
            Some(m) => self.record(m).def,
            None => self.registry().defs.borrow().get(specifier).copied(),
        }
    }

    /// Declares the cells of a module-level scope: every binding (exported or not) in index order. Called
    /// once, by `declare`. Panics if called twice.
    pub fn module_set_locals(&self, m: ObjRef, cells: Vec<ObjRef>) {
        let rec = self.record(m);
        let mut st = rec.st.borrow_mut();
        assert!(st.locals.is_none(), "module '{}': module_set_locals called twice (framework bug)", rec.spec);
        st.locals = Some(cells);
    }

    /// The cell of module-level binding `index`. Panics (framework bug) when out of range or not yet set.
    pub fn module_local(&self, m: ObjRef, index: usize) -> ObjRef {
        let rec = self.record(m);
        let st = rec.st.borrow();
        let locals = st
            .locals
            .as_ref()
            .unwrap_or_else(|| panic!("module '{}': module_local before module_set_locals (framework bug)", rec.spec));
        *locals.get(index).unwrap_or_else(|| panic!("module '{}': local {index} out of range (framework bug)", rec.spec))
    }

    /// Adds the local export `name` bound to `cell`. Panics if the name is already exported.
    pub fn module_define_export(&self, m: ObjRef, name: &str, cell: ObjRef) {
        let rec = self.record(m);
        let key = JsStr::from_str(name);
        let mut st = rec.st.borrow_mut();
        let taken = st.exports.contains_key(&key) || st.indirect.iter().any(|i| i.export == key);
        assert!(!taken, "module '{}': export '{name}' defined twice (framework bug)", rec.spec);
        st.exports.insert(key, cell);
    }

    /// The cell that `name` of module `m` resolves to, following re-exports. `None` when the name is missing,
    /// circular or ambiguous.
    pub fn module_export_cell(&self, m: ObjRef, name: &str) -> Res<Option<ObjRef>> {
        Ok(match self.resolve(m, &JsStr::from_str(name), &mut Vec::new()) {
            Resolved::Found(c) => Some(c),
            _ => None,
        })
    }

    /// The record of a request of `from`, for `import * as ns from request` (then `module_namespace` of it).
    /// `request` must be listed in `from`'s requests; its record exists once `from` is declared.
    pub fn module_request(&self, from: ObjRef, request: &str) -> ObjRef {
        assert_requested(&self.record(from), request);
        self.record_by_spec(request)
    }

    /// The cell an import `name` from `request` binds to, for the importing module `from`. A SyntaxError is
    /// thrown when the export is missing or ambiguous. `request` must be listed in `from`'s requests.
    pub fn module_import_cell(&self, from: ObjRef, request: &str, name: &str) -> Res<ObjRef> {
        assert_requested(&self.record(from), request);
        let dep = self.record_by_spec(request);
        match self.resolve(dep, &JsStr::from_str(name), &mut Vec::new()) {
            Resolved::Found(c) => Ok(c),
            Resolved::NotFound => Err(self.throw_error(
                ErrorKind::Syntax,
                &format!("The requested module '{request}' does not provide an export named '{name}'"),
            )),
            Resolved::Ambiguous => Err(self.throw_error(
                ErrorKind::Syntax,
                &format!("The requested module '{request}' contains conflicting star exports for name '{name}'"),
            )),
        }
    }

    /// `export { import_name as export_name } from request`. Panics on a duplicate export name.
    pub fn module_add_reexport(&self, m: ObjRef, export_name: &str, request: &str, import_name: &str) {
        let rec = self.record(m);
        assert_requested(&rec, request);
        let export = JsStr::from_str(export_name);
        let mut st = rec.st.borrow_mut();
        let taken = st.exports.contains_key(&export) || st.indirect.iter().any(|i| i.export == export);
        assert!(!taken, "module '{}': export '{export_name}' defined twice (framework bug)", rec.spec);
        st.indirect.push(Indirect { export, request: request.to_string(), import: JsStr::from_str(import_name) });
    }

    /// `export * from request`.
    pub fn module_add_star_reexport(&self, m: ObjRef, request: &str) {
        let rec = self.record(m);
        assert_requested(&rec, request);
        rec.st.borrow_mut().stars.push(request.to_string());
    }

    /// The module namespace object (ECMA-262 10.4.6): null prototype, not extensible, `@@toStringTag` "Module".
    /// Created once per module and then reused.
    pub fn module_namespace(&self, m: ObjRef) -> ObjRef {
        let rec = self.record(m);
        let existing = rec.st.borrow().namespace;
        if let Some(ns) = existing {
            return ns;
        }
        let hooks = Rc::new(Namespace { module: m, names: RefCell::new(None) });
        let ns = self.new_exotic_object(None, hooks);
        let tag = PropKey::Sym(self.well_known(WellKnown::ToStringTag));
        let defined = self.define_own(ns, &tag, Prop::Data { value: Value::str("Module"), flags: Flags::NONE });
        assert!(matches!(defined, Ok(true)), "namespace @@toStringTag refused (framework bug)");
        assert!(matches!(self.prevent_extensions(ns), Ok(true)), "namespace preventExtensions (framework bug)");
        rec.st.borrow_mut().namespace = Some(ns);
        ns
    }

    pub fn module_status(&self, m: ObjRef) -> ModuleStatus {
        let rec = self.record(m);
        let status = rec.st.borrow().status;
        status
    }

    /// The specifier a record was created for (diagnostics and tests).
    pub fn module_specifier(&self, m: ObjRef) -> String {
        self.record(m).spec.clone()
    }

    /// The record of `specifier`, if it has one (hosts use it to inspect a module after a failed load).
    pub fn module_record(&self, specifier: &str) -> Option<ObjRef> {
        self.existing_record(specifier)
    }
}

impl Namespace {
    /// Computes the export list on first use. A namespace read while modules are being declared or bound
    /// would see a partial graph, so that is refused instead.
    fn entries(&self, rt: &Rt) -> Res<()> {
        if self.names.borrow().is_some() {
            return Ok(());
        }
        if rt.registry().linking.get() > 0 {
            return Err(rt.throw_error(ErrorKind::Error, "module namespace used before linking completed"));
        }
        let entries = rt.namespace_entries(self.module);
        *self.names.borrow_mut() = Some(entries);
        Ok(())
    }

    /// The export that a string or index key names, with the cell it reads. Symbols name no export.
    fn find(&self, rt: &Rt, key: &PropKey) -> Res<Option<(ObjRef, JsStr)>> {
        let units: Vec<u16> = match key {
            PropKey::Index(i) => i.to_string().encode_utf16().collect(),
            PropKey::Str(s) => s.units().to_vec(),
            PropKey::Sym(_) => return Ok(None),
        };
        self.entries(rt)?;
        let names = self.names.borrow();
        let entries = names.as_ref().expect("namespace entries are computed");
        Ok(entries
            .binary_search_by(|e| e.name.units().cmp(units.as_slice()))
            .ok()
            .map(|i| (entries[i].cell, entries[i].name.clone())))
    }
}

impl Exotic for Namespace {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.module);
        if let Some(entries) = self.names.borrow().as_ref() {
            for e in entries {
                t.mark(e.cell);
            }
        }
    }

    fn get(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, _receiver: &Value) -> Res<Hook<Value>> {
        if let PropKey::Sym(_) = key {
            return Ok(Hook::Ordinary);
        }
        match self.find(rt, key)? {
            Some((cell, name)) => Ok(Hook::Handled(rt.cell_get_checked(cell, &name.to_rust_string_lossy())?)),
            None => Ok(Hook::Handled(Value::Undefined)),
        }
    }

    /// [[Set]] is always refused for a namespace, for every key.
    fn set(&self, _rt: &Rt, _obj: ObjRef, _key: &PropKey, _v: &Value, _receiver: &Value) -> Res<Hook<bool>> {
        Ok(Hook::Handled(false))
    }

    fn has(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<bool>> {
        if let PropKey::Sym(_) = key {
            return Ok(Hook::Ordinary);
        }
        Ok(Hook::Handled(self.find(rt, key)?.is_some()))
    }

    fn delete(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<bool>> {
        if let PropKey::Sym(_) = key {
            return Ok(Hook::Ordinary);
        }
        Ok(Hook::Handled(self.find(rt, key)?.is_none()))
    }

    /// Array-index names first in ascending numeric order, then the other names in code-unit order, then
    /// `@@toStringTag`. This is the order Node/V8 reports (checked by the modules corpus); ECMA-262 only
    /// asks for code-unit order of the names, which V8 does not follow for index-like names.
    fn own_keys(&self, rt: &Rt, _obj: ObjRef) -> Res<Hook<Vec<PropKey>>> {
        self.entries(rt)?;
        let names = self.names.borrow();
        let mut keys: Vec<PropKey> = names
            .as_ref()
            .expect("namespace entries are computed")
            .iter()
            .map(|e| PropKey::from_jsstr(e.name.clone()))
            .collect();
        keys.sort_by_key(|k| match k {
            PropKey::Index(i) => (0u8, *i),
            _ => (1u8, 0),
        });
        keys.push(PropKey::Sym(rt.well_known(WellKnown::ToStringTag)));
        Ok(Hook::Handled(keys))
    }

    fn get_own_property(&self, rt: &Rt, _obj: ObjRef, key: &PropKey) -> Res<Hook<Option<Prop>>> {
        if let PropKey::Sym(_) = key {
            return Ok(Hook::Ordinary);
        }
        let Some((cell, name)) = self.find(rt, key)? else {
            return Ok(Hook::Handled(None));
        };
        let value = rt.cell_get_checked(cell, &name.to_rust_string_lossy())?;
        Ok(Hook::Handled(Some(Prop::Data { value, flags: Flags::new(true, true, false) })))
    }

    /// [[DefineOwnProperty]] accepts only a descriptor that changes nothing: writable, enumerable,
    /// non-configurable, with the same value.
    fn define(&self, rt: &Rt, _obj: ObjRef, key: &PropKey, prop: &Prop) -> Res<Hook<bool>> {
        if let PropKey::Sym(_) = key {
            return Ok(Hook::Ordinary);
        }
        let Some((cell, name)) = self.find(rt, key)? else {
            return Ok(Hook::Handled(false));
        };
        let Prop::Data { value, flags } = prop else {
            return Ok(Hook::Handled(false));
        };
        if flags.configurable || !flags.enumerable || !flags.writable {
            return Ok(Hook::Handled(false));
        }
        let current = rt.cell_get_checked(cell, &name.to_rust_string_lossy())?;
        Ok(Hook::Handled(same_value(value, &current)))
    }
}
