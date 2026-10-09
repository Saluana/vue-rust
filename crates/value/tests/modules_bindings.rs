//! Bindings of ES modules (WP-L2c): live bindings, re-exports, `export *` with ambiguity, `default` and cycles,
//! the module-level locals table (declare, bind and evaluate share cells; closures created in bind capture
//! them), collections between and after the phases, and the framework-bug panics. The corpus cases come from
//! Node (tests/language/corpora/modules.json); the rest follow ECMA-262 16.2.1 and are written by hand.

mod support;
#[macro_use]
#[path = "modules_common.rs"]
mod common;

use common::*;
use vn_value::{Call, ModuleStatus, ObjRef, PropKey, Res, Rt, Value};

declare_exports!(decl_x, ["x"]);
declare_exports!(decl_xy, ["x", "y"]);
declare_exports!(decl_xz, ["x", "z"]);
declare_exports!(decl_default_q, ["default", "q"]);
declare_exports!(decl_a1, ["a1"]);
declare_exports!(decl_b1, ["b1"]);
declare_exports!(decl_count_bump, ["count", "bump"]);
declare_exports!(decl_count_get, ["count", "get"]);

fn module(rt: &Rt, spec: &str) -> ObjRef {
    rt.module_record(spec).unwrap_or_else(|| panic!("{spec} has no record"))
}

// ----- re-exports -----

fn bind_a_reexport_y(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_add_reexport(m, "y", "b.mjs", "x");
    Ok(())
}

fn bind_main_reads_y(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_import_cell(m, "a.mjs", "y").map(|_| ())
}

fn eval_main_reads_y(rt: &Rt, m: ObjRef) -> Res<()> {
    log(format!("main:{}", read_import(rt, m, "a.mjs", "y")?));
    Ok(())
}

fn eval_b_x42(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "x", Value::Number(42.0))?;
    log("b");
    Ok(())
}

#[test]
fn reexport_rename_resolves_to_the_exporters_own_cell() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["a.mjs"], nothing, bind_main_reads_y, eval_main_reads_y));
    rt.register_module_def(def("a.mjs", &["b.mjs"], nothing, bind_a_reexport_y, nothing));
    rt.register_module_def(def("b.mjs", &[], decl_x, nothing, eval_b_x42));
    check_case(&rt, "reexport-rename");
    let via_a = rt.module_export_cell(module(&rt, "a.mjs"), "y").expect("no throw").expect("y resolves");
    let direct = rt.module_export_cell(module(&rt, "b.mjs"), "x").expect("no throw").expect("x resolves");
    assert_eq!(via_a, direct, "a re-export is the exporter's cell, not a copy");
}

fn bind_a_reexport_z(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_add_reexport(m, "z", "b.mjs", "y");
    Ok(())
}

fn bind_b_reexport_y(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_add_reexport(m, "y", "c.mjs", "x");
    Ok(())
}

fn bind_main_reads_z(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_import_cell(m, "a.mjs", "z").map(|_| ())
}

fn eval_main_reads_z(rt: &Rt, m: ObjRef) -> Res<()> {
    log(format!("main:{}", read_import(rt, m, "a.mjs", "z")?));
    Ok(())
}

fn eval_c_cx(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "x", Value::str("cx"))?;
    log("c");
    Ok(())
}

#[test]
fn reexport_chain_resolves_through_every_module() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["a.mjs"], nothing, bind_main_reads_z, eval_main_reads_z));
    rt.register_module_def(def("a.mjs", &["b.mjs"], nothing, bind_a_reexport_z, nothing));
    rt.register_module_def(def("b.mjs", &["c.mjs"], nothing, bind_b_reexport_y, nothing));
    rt.register_module_def(def("c.mjs", &[], decl_x, nothing, eval_c_cx));
    check_case(&rt, "reexport-chain");
}

#[test]
#[should_panic(expected = "defined twice")]
fn duplicate_reexport_name_panics() {
    fn bind_twice(rt: &Rt, m: ObjRef) -> Res<()> {
        rt.module_add_reexport(m, "y", "b.mjs", "x");
        rt.module_add_reexport(m, "y", "b.mjs", "x");
        Ok(())
    }
    let rt = Rt::new();
    rt.register_module_def(def("a.mjs", &["b.mjs"], nothing, bind_twice, nothing));
    rt.register_module_def(def("b.mjs", &[], decl_x, nothing, nothing));
    let _ = rt.load_module("a.mjs");
}

#[test]
#[should_panic(expected = "without listing it in requests")]
fn reexport_from_an_unlisted_request_panics() {
    fn bind_unlisted(rt: &Rt, m: ObjRef) -> Res<()> {
        rt.module_add_reexport(m, "y", "nowhere.mjs", "x");
        Ok(())
    }
    let rt = Rt::new();
    rt.register_module_def(def("a.mjs", &[], nothing, bind_unlisted, nothing));
    let _ = rt.load_module("a.mjs");
}

fn bind_x_from_a(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_add_reexport(m, "x", "a.mjs", "x");
    Ok(())
}

fn bind_a_from_b(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_add_reexport(m, "x", "b.mjs", "x");
    Ok(())
}

fn bind_main_x_from_a(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_import_cell(m, "a.mjs", "x").map(|_| ())
}

#[test]
fn re_export_cycle_terminates_and_the_name_is_missing() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["a.mjs"], nothing, bind_main_x_from_a, nothing));
    rt.register_module_def(def("a.mjs", &["b.mjs"], nothing, bind_a_from_b, nothing));
    rt.register_module_def(def("b.mjs", &["a.mjs"], nothing, bind_x_from_a, nothing));
    let e = rt.load_module("main.mjs").expect_err("x is circular, so missing");
    assert_eq!(error_name(&rt, &e.0), "SyntaxError");
    let a = module(&rt, "a.mjs");
    let b = module(&rt, "b.mjs");
    assert_eq!(rt.module_export_cell(a, "x").expect("no throw"), None, "a circular re-export is not found");
    assert_eq!(rt.module_export_cell(b, "x").expect("no throw"), None);
}

// ----- export * -----

fn bind_a_stars_bc(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_add_star_reexport(m, "b.mjs");
    rt.module_add_star_reexport(m, "c.mjs");
    Ok(())
}

fn bind_stars_d(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_add_star_reexport(m, "d.mjs");
    Ok(())
}

fn bind_main_reads_x_via_a(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_import_cell(m, "a.mjs", "x").map(|_| ())
}

fn eval_main_reads_x_via_a(rt: &Rt, m: ObjRef) -> Res<()> {
    log(format!("main:{}", read_import(rt, m, "a.mjs", "x")?));
    Ok(())
}

fn eval_d_dx(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "x", Value::str("dx"))?;
    log("d");
    Ok(())
}

#[test]
fn star_exports_of_one_binding_through_two_paths_are_not_ambiguous() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["a.mjs"], nothing, bind_main_reads_x_via_a, eval_main_reads_x_via_a));
    rt.register_module_def(def("a.mjs", &["b.mjs", "c.mjs"], nothing, bind_a_stars_bc, nothing));
    rt.register_module_def(def("b.mjs", &["d.mjs"], nothing, bind_stars_d, nothing));
    rt.register_module_def(def("c.mjs", &["d.mjs"], nothing, bind_stars_d, nothing));
    rt.register_module_def(def("d.mjs", &[], decl_x, nothing, eval_d_dx));
    check_case(&rt, "star-same-binding");
}

fn eval_main_keys_and_x_of_a(rt: &Rt, m: ObjRef) -> Res<()> {
    let ns = rt.module_namespace(rt.module_request(m, "a.mjs"));
    log(format!("keys:{}", enumerable_keys(rt, ns)?.join(",")));
    log(format!("x:{}", rt.has_property(ns, &PropKey::from_str("x"))?));
    Ok(())
}

fn eval_b_xy(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "x", Value::Number(1.0))?;
    init_export(rt, m, "y", Value::Number(2.0))?;
    log("b");
    Ok(())
}

fn eval_c_xz(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "x", Value::Number(3.0))?;
    init_export(rt, m, "z", Value::Number(4.0))?;
    log("c");
    Ok(())
}

#[test]
fn ambiguous_star_name_is_missing_from_the_namespace_and_from_export_lookup() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["a.mjs"], nothing, nothing, eval_main_keys_and_x_of_a));
    rt.register_module_def(def("a.mjs", &["b.mjs", "c.mjs"], nothing, bind_a_stars_bc, nothing));
    rt.register_module_def(def("b.mjs", &[], decl_xy, nothing, eval_b_xy));
    rt.register_module_def(def("c.mjs", &[], decl_xz, nothing, eval_c_xz));
    check_case(&rt, "star-ambiguous-namespace");
    let a = module(&rt, "a.mjs");
    assert_eq!(rt.module_export_cell(a, "x").expect("no throw"), None, "ambiguous counts as missing");
    assert!(rt.module_export_cell(a, "y").expect("no throw").is_some());
}

fn eval_main_default_of_a(rt: &Rt, m: ObjRef) -> Res<()> {
    let ns = rt.module_namespace(rt.module_request(m, "a.mjs"));
    log(format!("keys:{}", enumerable_keys(rt, ns)?.join(",")));
    log(format!("default:{}", rt.has_property(ns, &PropKey::from_str("default"))?));
    Ok(())
}

fn eval_b_default_q(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "default", Value::str("bd"))?;
    init_export(rt, m, "q", Value::Number(2.0))
}

#[test]
fn export_star_never_re_exports_default() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["a.mjs"], nothing, nothing, eval_main_default_of_a));
    rt.register_module_def(def("a.mjs", &["b.mjs"], nothing, bind_stars_b, nothing));
    rt.register_module_def(def("b.mjs", &[], decl_default_q, nothing, eval_b_default_q));
    check_case(&rt, "star-default-excluded");
}

fn bind_stars_b(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_add_star_reexport(m, "b.mjs");
    Ok(())
}

fn bind_a_star_a_cycle(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_add_star_reexport(m, "b.mjs");
    Ok(())
}

fn bind_b_star_a_cycle(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_add_star_reexport(m, "a.mjs");
    Ok(())
}

fn eval_main_keys_of_a(rt: &Rt, m: ObjRef) -> Res<()> {
    let ns = rt.module_namespace(rt.module_request(m, "a.mjs"));
    log(format!("keys:{}", enumerable_keys(rt, ns)?.join(",")));
    Ok(())
}

fn eval_a1(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "a1", Value::Number(1.0))
}

fn eval_b1(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "b1", Value::Number(2.0))
}

#[test]
fn export_star_cycle_terminates() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["a.mjs"], nothing, nothing, eval_main_keys_of_a));
    rt.register_module_def(def("a.mjs", &["b.mjs"], decl_a1, bind_a_star_a_cycle, eval_a1));
    rt.register_module_def(def("b.mjs", &["a.mjs"], decl_b1, bind_b_star_a_cycle, eval_b1));
    check_case(&rt, "star-cycle");
}

// ----- live bindings -----

/// `function bump() { count++; }`, where `count` is the module's own cell, captured when `bump` is hoisted.
fn bump_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let cell = rt.captures(c.callee)[0];
    let n = match rt.cell_get(cell) {
        Value::Number(n) => n,
        _ => return Err(rt.throw_type_error("count is not a number")),
    };
    rt.cell_set(cell, Value::Number(n + 1.0));
    Ok(Value::Undefined)
}

fn bind_lib_bump(rt: &Rt, m: ObjRef) -> Res<()> {
    let count = rt.module_local(m, 0);
    hoist(rt, m, "bump", bump_code, &[count])
}

fn eval_lib_count(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "count", Value::Number(0.0))
}

fn bind_main_count(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_import_cell(m, "lib.mjs", "count")?;
    rt.module_import_cell(m, "lib.mjs", "bump").map(|_| ())
}

fn eval_main_count(rt: &Rt, m: ObjRef) -> Res<()> {
    log(format!("before:{}", read_import(rt, m, "lib.mjs", "count")?));
    let bump = rt.module_import_cell(m, "lib.mjs", "bump")?;
    let bump = rt.cell_get_checked(bump, "bump")?;
    rt.call(&bump, &Value::Undefined, &[])?;
    log(format!("after:{}", read_import(rt, m, "lib.mjs", "count")?));
    Ok(())
}

#[test]
fn live_binding_reads_the_value_the_exporter_wrote() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["lib.mjs"], nothing, bind_main_count, eval_main_count));
    rt.register_module_def(def("lib.mjs", &[], decl_count_bump, bind_lib_bump, eval_lib_count));
    check_case(&rt, "live-binding");
}

#[test]
fn module_request_returns_the_record_of_a_listed_request() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["b.mjs"], nothing, nothing, nothing));
    rt.register_module_def(def("b.mjs", &[], nothing, nothing, nothing));
    let main = rt.load_module("main.mjs").expect("loads");
    let b = rt.module_request(main, "b.mjs");
    assert_eq!(Some(b), rt.module_record("b.mjs"));
    assert_eq!(rt.module_specifier(b), "b.mjs");
    assert_eq!(rt.module_status(b), ModuleStatus::Evaluated);
}

#[test]
#[should_panic(expected = "without listing it in requests")]
fn module_request_of_an_unlisted_specifier_panics() {
    let rt = Rt::new();
    let m = rt.module_new("x.mjs");
    let _ = rt.module_request(m, "y.mjs");
}

#[test]
#[should_panic(expected = "defined twice")]
fn duplicate_export_name_panics() {
    fn decl_twice(rt: &Rt, m: ObjRef) -> Res<()> {
        let c = rt.new_cell(Value::Uninit);
        rt.module_define_export(m, "x", c);
        rt.module_define_export(m, "x", c);
        rt.module_set_locals(m, vec![c]);
        Ok(())
    }
    let rt = Rt::new();
    rt.register_module_def(def("dup.mjs", &[], decl_twice, nothing, nothing));
    let _ = rt.load_module("dup.mjs");
}

// ----- the module locals table: declare, bind and evaluate share the cells -----

/// Local 0 is a `let` that is not exported, local 1 is `export let b`, local 2 is a `var`.
fn decl_locals(rt: &Rt, m: ObjRef) -> Res<()> {
    let a = rt.new_cell(Value::Uninit);
    let b = rt.new_cell(Value::Uninit);
    let c = rt.new_cell(Value::Undefined);
    rt.module_set_locals(m, vec![a, b, c]);
    rt.module_define_export(m, "b", b);
    Ok(())
}

fn bind_locals(rt: &Rt, m: ObjRef) -> Res<()> {
    assert!(matches!(rt.cell_get(rt.module_local(m, 0)), Value::Uninit), "a let is in its TDZ before evaluate");
    assert!(matches!(rt.cell_get(rt.module_local(m, 2)), Value::Undefined), "a var starts as undefined");
    Ok(())
}

fn eval_locals(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.cell_init(rt.module_local(m, 0), Value::Number(1.0));
    rt.cell_set(rt.module_local(m, 2), Value::Number(3.0));
    init_export(rt, m, "b", Value::Number(2.0))
}

#[test]
fn locals_are_set_read_and_initialised_across_the_phases() {
    let rt = Rt::new();
    rt.register_module_def(def("locals.mjs", &[], decl_locals, bind_locals, eval_locals));
    let m = rt.load_module("locals.mjs").expect("loads");
    assert_eq!(rt.cell_get(rt.module_local(m, 0)), Value::Number(1.0));
    assert_eq!(rt.cell_get(rt.module_local(m, 1)), Value::Number(2.0));
    assert_eq!(rt.cell_get(rt.module_local(m, 2)), Value::Number(3.0));
    let export = rt.module_export_cell(m, "b").expect("no throw").expect("b is exported");
    assert_eq!(export, rt.module_local(m, 1), "an export is the same cell as its local");
}

/// `export function get() { return count; }`, where `count` is local 0.
fn get_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let cell = rt.captures(c.callee)[0];
    rt.cell_get_checked(cell, "count")
}

fn bind_closure(rt: &Rt, m: ObjRef) -> Res<()> {
    let count = rt.module_local(m, 0);
    hoist(rt, m, "get", get_code, &[count])
}

fn eval_closure(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "count", Value::Number(42.0))
}

fn call_get(rt: &Rt, m: ObjRef) -> Value {
    let get = rt.module_export_cell(m, "get").expect("no throw").expect("get is exported");
    rt.call(&rt.cell_get(get), &Value::Undefined, &[]).expect("get is callable")
}

#[test]
fn closure_created_in_bind_sees_the_value_written_in_evaluate() {
    let rt = Rt::new();
    rt.register_module_def(def("closure.mjs", &[], decl_count_get, bind_closure, eval_closure));
    let m = rt.load_module("closure.mjs").expect("loads");
    assert_eq!(call_get(&rt, m), Value::Number(42.0));
    rt.cell_set(rt.module_local(m, 0), Value::Number(7.0));
    assert_eq!(call_get(&rt, m), Value::Number(7.0), "the closure reads the cell, not a copy");
}

#[test]
fn collections_between_the_phases_keep_records_locals_and_closures() {
    let rt = Rt::new();
    let m = rt.module_new("manual.mjs");
    decl_count_get(&rt, m).expect("declare");
    rt.collect();
    let count = rt.module_local(m, 0);
    assert!(rt.is_live(count), "a declared local survives a collection");
    bind_closure(&rt, m).expect("bind");
    rt.collect();
    eval_closure(&rt, m).expect("evaluate");
    rt.collect();
    assert_eq!(call_get(&rt, m), Value::Number(42.0));
    assert!(rt.is_live(m), "the record survives");
}

#[test]
fn collection_after_load_keeps_bindings_and_frees_garbage() {
    let rt = Rt::new();
    rt.register_module_def(def("closure.mjs", &[], decl_count_get, bind_closure, eval_closure));
    let m = rt.load_module("closure.mjs").expect("loads");
    let garbage = rt.new_object();
    rt.collect();
    assert!(!rt.is_live(garbage), "an unreachable object is collected");
    assert_eq!(call_get(&rt, m), Value::Number(42.0));
    let ns = rt.module_namespace(m);
    assert_eq!(rt.get_str(&Value::Object(ns), "count").expect("readable"), Value::Number(42.0));
}

#[test]
#[should_panic(expected = "called twice")]
fn module_set_locals_twice_panics() {
    let rt = Rt::new();
    let m = rt.module_new("twice.mjs");
    rt.module_set_locals(m, Vec::new());
    rt.module_set_locals(m, Vec::new());
}

#[test]
#[should_panic(expected = "out of range")]
fn module_local_past_the_table_panics() {
    let rt = Rt::new();
    let m = rt.module_new("short.mjs");
    let c = rt.new_cell(Value::Undefined);
    rt.module_set_locals(m, vec![c]);
    let _ = rt.module_local(m, 3);
}

#[test]
#[should_panic(expected = "before module_set_locals")]
fn module_local_before_the_table_is_set_panics() {
    let rt = Rt::new();
    let m = rt.module_new("unset.mjs");
    let _ = rt.module_local(m, 0);
}
