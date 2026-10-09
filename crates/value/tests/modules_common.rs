//! Helpers shared by the modules_*.rs test files (WP-L2c). Each of them includes this file with
//! `#[macro_use] #[path = "modules_common.rs"] mod common;` and `mod support;`. It holds the evaluation log,
//! module definitions built like generated statics, hoisting and initialisation helpers, and the replay of
//! the Node corpus (tests/language/corpora/modules.json): each case's steps are loaded in order and checked
//! against Node's outcome, evaluation log and thrown-value identity.

#![allow(dead_code, unused_macros)]

use std::cell::RefCell;

#[path = "support/mod.rs"]
mod support;

use support::Json;
use vn_value::{same_value, CodeFn, FnInfo, FnKind, ModuleDef, ObjRef, Res, Rt, Throw, Value};

const CORPUS: &str = include_str!("../../../tests/language/corpora/modules.json");

thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub fn log(s: impl Into<String>) {
    LOG.with(|l| l.borrow_mut().push(s.into()));
}

pub fn take_log() -> Vec<String> {
    LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

/// A definition leaked to `'static`, like the statics generated code provides.
pub fn def(
    specifier: &'static str,
    requests: &'static [&'static str],
    declare: fn(&Rt, ObjRef) -> Res<()>,
    bind: fn(&Rt, ObjRef) -> Res<()>,
    evaluate: fn(&Rt, ObjRef) -> Res<()>,
) -> &'static ModuleDef {
    Box::leak(Box::new(ModuleDef { specifier, requests, declare, bind, evaluate }))
}

pub fn nothing(_rt: &Rt, _m: ObjRef) -> Res<()> {
    Ok(())
}

pub fn text(rt: &Rt, v: &Value) -> Res<String> {
    Ok(rt.to_string(v)?.to_rust_string_lossy())
}

/// The `name` of a thrown error object.
pub fn error_name(rt: &Rt, v: &Value) -> String {
    let name = rt.get_str(v, "name").and_then(|n| rt.to_string(&n));
    name.expect("thrown value has a name").to_rust_string_lossy()
}

pub fn error_message(rt: &Rt, v: &Value) -> String {
    let msg = rt.get_str(v, "message").expect("message readable");
    text(rt, &msg).expect("message is a string")
}

/// Declares one exported binding per name: a TDZ cell each, local `i` being the i-th name.
macro_rules! declare_exports {
    ($f:ident, [$($name:literal),* $(,)?]) => {
        fn $f(rt: &Rt, m: ObjRef) -> Res<()> {
            let names: &[&str] = &[$($name),*];
            let cells: Vec<ObjRef> = names.iter().map(|_| rt.new_cell(Value::Uninit)).collect();
            for (name, cell) in names.iter().zip(&cells) {
                rt.module_define_export(m, name, *cell);
            }
            rt.module_set_locals(m, cells);
            Ok(())
        }
    };
}

/// Hoists a function declaration (bind phase): creates the function and initialises its exported cell.
pub fn hoist(rt: &Rt, m: ObjRef, name: &str, code: CodeFn, captures: &[ObjRef]) -> Res<()> {
    let f = rt.new_function(code, FnInfo::new(name, 0, FnKind::Normal), captures);
    let cell = rt.module_export_cell(m, name)?.expect("hoisted function is exported");
    rt.cell_init(cell, Value::Object(f));
    Ok(())
}

/// Initialises an exported `let`/`const` binding (its declaration, run by evaluate).
pub fn init_export(rt: &Rt, m: ObjRef, name: &str, v: Value) -> Res<()> {
    let cell = rt.module_export_cell(m, name)?.expect("export is declared");
    rt.cell_init(cell, v);
    Ok(())
}

/// Reads an imported binding through its cell, with the TDZ check, as text.
pub fn read_import(rt: &Rt, m: ObjRef, request: &str, name: &str) -> Res<String> {
    let cell = rt.module_import_cell(m, request, name)?;
    text(rt, &rt.cell_get_checked(cell, name)?)
}

pub fn corpus_case(name: &str) -> Json {
    let doc = Json::parse(CORPUS);
    let found = doc.get("cases").arr().iter().find(|c| c.get("name").str() == name).cloned();
    found.unwrap_or_else(|| panic!("corpus has no case {name:?}"))
}

/// Replays the Node steps of corpus case `name` against `rt`. Returns the first mismatch in outcome,
/// thrown-value identity or evaluation log. `mutate` edits the expected logs (used by a negative control).
pub fn run_case(rt: &Rt, name: &str, mutate: impl Fn(&mut Vec<String>)) -> Result<(), String> {
    let case = corpus_case(name);
    let mut thrown: Vec<Value> = Vec::new();
    for (i, step) in case.get("steps").arr().iter().enumerate() {
        let file = step.get("load").str();
        let mut want_log: Vec<String> = step.get("log").arr().iter().map(|s| s.str().to_string()).collect();
        mutate(&mut want_log);
        let want_outcome = step.get("outcome").str();
        take_log();
        let result = rt.load_module(file);
        let got_log = take_log();
        let (outcome, same) = match result {
            Ok(_) => ("ok".to_string(), None),
            Err(Throw(v)) => {
                let same = thrown.iter().any(|t| same_value(t, &v));
                let ename = error_name(rt, &v);
                thrown.push(v);
                (ename, Some(same))
            }
        };
        let want_same = step.get("sameThrow");
        let same_ok = match (same, want_same) {
            (None, w) => w.is_null(),
            (Some(s), Json::Bool(w)) => *w == s,
            (Some(_), _) => false,
        };
        if outcome != want_outcome || !same_ok || got_log != want_log {
            return Err(format!(
                "{name} step {i} ({file}): got {outcome}, same={same:?}, log={got_log:?}; \
                 node says {want_outcome}, same={want_same:?}, log={want_log:?}"
            ));
        }
    }
    Ok(())
}

pub fn check_case(rt: &Rt, name: &str) {
    if let Err(e) = run_case(rt, name, |_| {}) {
        panic!("{e}");
    }
}

/// The enumerable own string keys, as `Object.keys` reports them.
pub fn enumerable_keys(rt: &Rt, o: ObjRef) -> Res<Vec<String>> {
    let mut out = Vec::new();
    for k in rt.own_keys(o)? {
        let name = match &k {
            vn_value::PropKey::Index(i) => i.to_string(),
            vn_value::PropKey::Str(s) => s.to_rust_string_lossy(),
            vn_value::PropKey::Sym(_) => continue,
        };
        if rt.get_own_property(o, &k)?.is_some_and(|p| p.flags().enumerable) {
            out.push(name);
        }
    }
    Ok(out)
}
