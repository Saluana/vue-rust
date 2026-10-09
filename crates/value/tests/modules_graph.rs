//! Module linking and evaluation (WP-L2c): evaluation order, cycles, error caching, missing modules and
//! exports, native modules, statuses and the framework's refusals. The expected orders, outcomes and
//! thrown-value identities come from Node's own ESM loader (tests/language/corpora/modules.json, written by
//! tests/language/gen/gen-module-cases.mjs). Each test builds the same graph with hand-written ModuleDefs and
//! replays the corpus steps against it (modules_common.rs).

mod support;
#[macro_use]
#[path = "modules_common.rs"]
mod common;

use common::*;
use vn_value::{Call, ModuleStatus, ObjRef, Res, Rt, Throw, Value};

declare_exports!(decl_f, ["f"]);
declare_exports!(decl_g, ["g"]);
declare_exports!(decl_v, ["v"]);
declare_exports!(decl_xy, ["x", "y"]);
declare_exports!(decl_xz, ["x", "z"]);
declare_exports!(decl_yes, ["yes"]);

// ----- single modules, chains and diamonds -----

#[test]
fn single_module_runs_once_and_reloads_without_running() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &[], nothing, nothing, |_rt, _m| {
        log("main");
        Ok(())
    }));
    check_case(&rt, "single");
    let first = rt.load_module("main.mjs").expect("reload");
    assert_eq!(first, rt.load_module("main.mjs").expect("reload"), "one record per module");
    assert!(take_log().is_empty(), "an evaluated module must not run again");
    assert_eq!(rt.module_status(first), ModuleStatus::Evaluated);
    assert_eq!(rt.module_specifier(first), "main.mjs");
}

#[test]
fn chain_evaluates_dependencies_first() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["b.mjs"], nothing, nothing, |_rt, _m| {
        log("main");
        Ok(())
    }));
    rt.register_module_def(def("b.mjs", &["c.mjs"], nothing, nothing, |_rt, _m| {
        log("b");
        Ok(())
    }));
    rt.register_module_def(def("c.mjs", &[], nothing, nothing, |_rt, _m| {
        log("c");
        Ok(())
    }));
    check_case(&rt, "chain");
}

fn register_diamond(rt: &Rt) {
    rt.register_module_def(def("main.mjs", &["a.mjs", "b.mjs"], nothing, nothing, |_rt, _m| {
        log("main");
        Ok(())
    }));
    rt.register_module_def(def("a.mjs", &["d.mjs"], nothing, nothing, |_rt, _m| {
        log("a");
        Ok(())
    }));
    rt.register_module_def(def("b.mjs", &["d.mjs"], nothing, nothing, |_rt, _m| {
        log("b");
        Ok(())
    }));
    rt.register_module_def(def("d.mjs", &[], nothing, nothing, |_rt, _m| {
        log("d");
        Ok(())
    }));
}

#[test]
fn diamond_evaluates_the_shared_dependency_once() {
    let rt = Rt::new();
    register_diamond(&rt);
    check_case(&rt, "diamond");
}

// ----- cycles -----

fn f_code(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::str("f"))
}

/// `export function g() { return 'g' + f(); }`: `f` is the imported cell, captured when `g` is hoisted.
fn g_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let f_cell = rt.captures(c.callee)[0];
    let f = rt.cell_get_checked(f_cell, "f")?;
    let r = rt.call(&f, &Value::Undefined, &[])?;
    Ok(Value::str(&format!("g{}", text(rt, &r)?)))
}

fn bind_main_cycle(rt: &Rt, m: ObjRef) -> Res<()> {
    hoist(rt, m, "f", f_code, &[])
}

fn eval_main_cycle(rt: &Rt, m: ObjRef) -> Res<()> {
    let g = rt.module_import_cell(m, "b.mjs", "g")?;
    let gv = rt.cell_get_checked(g, "g")?;
    let r = rt.call(&gv, &Value::Undefined, &[])?;
    log(format!("main:{}", text(rt, &r)?));
    Ok(())
}

fn bind_b_cycle(rt: &Rt, m: ObjRef) -> Res<()> {
    let f = rt.module_import_cell(m, "main.mjs", "f")?;
    hoist(rt, m, "g", g_code, &[f])
}

fn eval_b_cycle(rt: &Rt, m: ObjRef) -> Res<()> {
    let f = rt.module_import_cell(m, "main.mjs", "f")?;
    let fv = rt.cell_get_checked(f, "f")?;
    let r = rt.call(&fv, &Value::Undefined, &[])?;
    log(format!("b:{}", text(rt, &r)?));
    Ok(())
}

#[test]
fn cycle_function_export_is_usable_before_its_module_evaluates() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["b.mjs"], decl_f, bind_main_cycle, eval_main_cycle));
    rt.register_module_def(def("b.mjs", &["main.mjs"], decl_g, bind_b_cycle, eval_b_cycle));
    check_case(&rt, "cycle-function");
}

fn eval_main_let_uncaught(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "v", Value::Number(1.0))?;
    log("main");
    Ok(())
}

fn bind_b_let(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_import_cell(m, "main.mjs", "v").map(|_| ())
}

fn eval_b_let_uncaught(rt: &Rt, m: ObjRef) -> Res<()> {
    let v = rt.module_import_cell(m, "main.mjs", "v")?;
    let x = rt.cell_get_checked(v, "v")?;
    log(format!("b:{}", text(rt, &x)?));
    Ok(())
}

fn eval_b_let_caught(rt: &Rt, m: ObjRef) -> Res<()> {
    let v = rt.module_import_cell(m, "main.mjs", "v")?;
    match rt.cell_get_checked(v, "v") {
        Ok(x) => log(format!("b:{}", text(rt, &x)?)),
        Err(Throw(e)) => log(format!("b:{}", error_name(rt, &e))),
    }
    Ok(())
}

fn eval_main_let_caught(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "v", Value::Number(1.0))?;
    log(format!("main:{}", text(rt, &Value::Number(1.0))?));
    Ok(())
}

#[test]
fn cycle_let_read_before_initialisation_throws_and_the_error_is_cached() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["b.mjs"], decl_v, nothing, eval_main_let_uncaught));
    rt.register_module_def(def("b.mjs", &["main.mjs"], nothing, bind_b_let, eval_b_let_uncaught));
    check_case(&rt, "cycle-let-uncaught");
    let main = rt.module_record("main.mjs").expect("main has a record");
    let b = rt.module_record("b.mjs").expect("b has a record");
    assert_eq!(rt.module_status(main), ModuleStatus::Errored);
    assert_eq!(rt.module_status(b), ModuleStatus::Errored, "the module that threw is errored too");
}

#[test]
fn cycle_let_read_before_initialisation_can_be_caught_inside_the_module() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["b.mjs"], decl_v, nothing, eval_main_let_caught));
    rt.register_module_def(def("b.mjs", &["main.mjs"], nothing, bind_b_let, eval_b_let_caught));
    check_case(&rt, "cycle-let-caught");
}

// ----- missing modules and exports (link errors) -----

#[test]
fn missing_module_throws_and_no_code_of_the_graph_runs() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["nope.mjs"], nothing, nothing, |_rt, _m| {
        log("main");
        Ok(())
    }));
    check_case(&rt, "missing-module");
}

#[test]
fn missing_request_is_found_before_any_declare_runs() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["ok.mjs", "nope.mjs"], nothing, nothing, |_rt, _m| {
        log("main");
        Ok(())
    }));
    rt.register_module_def(def(
        "ok.mjs",
        &[],
        |_rt, _m| {
            log("declare:ok");
            Ok(())
        },
        nothing,
        nothing,
    ));
    let e = rt.load_module("main.mjs").expect_err("nope.mjs is missing");
    assert_eq!(error_name(&rt, &e.0), "Error");
    assert_eq!(error_message(&rt, &e.0), "Cannot find module 'nope.mjs'");
    assert!(take_log().is_empty(), "no declare, bind or evaluate may run for a graph with a missing request");
    assert!(rt.module_record("ok.mjs").is_none(), "the sibling must not have a record");
    assert!(rt.module_record("main.mjs").is_none());
}

fn bind_main_missing_export(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_import_cell(m, "lib.mjs", "nope").map(|_| ())
}

fn eval_lib_yes(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "yes", Value::Number(1.0))?;
    log("lib");
    Ok(())
}

#[test]
fn missing_export_is_a_link_error_and_the_other_modules_stay_loadable() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["lib.mjs"], nothing, bind_main_missing_export, |_rt, _m| {
        log("main");
        Ok(())
    }));
    rt.register_module_def(def("lib.mjs", &[], decl_yes, nothing, eval_lib_yes));
    check_case(&rt, "missing-export");
    let main = rt.module_record("main.mjs").expect("main has a record");
    assert_eq!(rt.module_status(main), ModuleStatus::Errored);
    let again = rt.load_module("main.mjs").expect_err("the link error is cached");
    assert!(error_message(&rt, &again.0).contains("does not provide an export named 'nope'"));
}

fn bind_a_stars(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_add_star_reexport(m, "b.mjs");
    rt.module_add_star_reexport(m, "c.mjs");
    Ok(())
}

fn bind_main_ambiguous(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_import_cell(m, "a.mjs", "x").map(|_| ())
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
fn ambiguous_star_import_is_a_link_error_and_the_sources_stay_loadable() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["a.mjs"], nothing, bind_main_ambiguous, |_rt, _m| {
        log("main");
        Ok(())
    }));
    rt.register_module_def(def("a.mjs", &["b.mjs", "c.mjs"], nothing, bind_a_stars, nothing));
    rt.register_module_def(def("b.mjs", &[], decl_xy, nothing, eval_b_xy));
    rt.register_module_def(def("c.mjs", &[], decl_xz, nothing, eval_c_xz));
    check_case(&rt, "star-ambiguous-import");
    let e = rt.load_module("main.mjs").expect_err("cached");
    assert!(error_message(&rt, &e.0).contains("conflicting star exports for name 'x'"));
}

// ----- evaluation errors: caching, identity, siblings -----

fn throw_boom(rt: &Rt, _m: ObjRef) -> Res<()> {
    log("bad");
    Err(rt.throw_range_error("boom"))
}

#[test]
fn evaluation_error_is_cached_and_rethrown_with_the_same_identity() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["bad.mjs"], nothing, nothing, |_rt, _m| {
        log("main");
        Ok(())
    }));
    rt.register_module_def(def("bad.mjs", &[], nothing, nothing, throw_boom));
    check_case(&rt, "error-cache");
    let bad = rt.module_record("bad.mjs").expect("bad has a record");
    assert_eq!(rt.module_status(bad), ModuleStatus::Errored);
    let e = rt.load_module("bad.mjs").expect_err("cached");
    assert_eq!(error_message(&rt, &e.0), "boom");
    assert!(take_log().is_empty(), "a module whose evaluation threw must never run again");
}

#[test]
fn sibling_after_a_failed_module_is_not_run_and_loads_later() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["bad.mjs", "other.mjs"], nothing, nothing, |_rt, _m| {
        log("main");
        Ok(())
    }));
    rt.register_module_def(def("bad.mjs", &[], nothing, nothing, throw_boom));
    rt.register_module_def(def("other.mjs", &[], nothing, nothing, |_rt, _m| {
        log("other");
        Ok(())
    }));
    check_case(&rt, "error-sibling");
    let other = rt.module_record("other.mjs").expect("other has a record");
    assert_eq!(rt.module_status(other), ModuleStatus::Evaluated);
}

#[test]
fn dependents_of_a_failed_module_rethrow_the_same_value() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["mid.mjs"], nothing, nothing, |_rt, _m| {
        log("main");
        Ok(())
    }));
    rt.register_module_def(def("mid.mjs", &["bad.mjs"], nothing, nothing, |_rt, _m| {
        log("mid");
        Ok(())
    }));
    rt.register_module_def(def("bad.mjs", &[], nothing, nothing, throw_boom));
    check_case(&rt, "error-chain-cache");
    let mid = rt.module_record("mid.mjs").expect("mid has a record");
    assert_eq!(rt.module_status(mid), ModuleStatus::Errored, "a dependent of a failed module is errored");
}

// ----- native modules and statuses -----

fn bind_main_vue(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_import_cell(m, "vue", "version").map(|_| ())
}

fn eval_main_vue(rt: &Rt, m: ObjRef) -> Res<()> {
    let v = read_import(rt, m, "vue", "version")?;
    log(format!("main:{v}"));
    Ok(())
}

#[test]
fn native_module_is_registered_evaluated_and_imported() {
    let rt = Rt::new();
    let vue = rt.register_native_module("vue", vec![("version", Value::str("3.5.43"))]);
    assert_eq!(rt.module_status(vue), ModuleStatus::Evaluated);
    assert_eq!(rt.load_module("vue").expect("native module loads"), vue);
    rt.register_module_def(def("main.mjs", &["vue"], nothing, bind_main_vue, eval_main_vue));
    check_case(&rt, "native-vue");
}

#[test]
fn module_status_follows_the_lifecycle() {
    fn decl(rt: &Rt, m: ObjRef) -> Res<()> {
        assert_eq!(rt.module_status(m), ModuleStatus::New);
        rt.module_set_locals(m, Vec::new());
        Ok(())
    }
    fn bind(rt: &Rt, m: ObjRef) -> Res<()> {
        assert_eq!(rt.module_status(m), ModuleStatus::Declared);
        Ok(())
    }
    fn eval(rt: &Rt, m: ObjRef) -> Res<()> {
        assert_eq!(rt.module_status(m), ModuleStatus::Evaluating);
        log("life");
        Ok(())
    }
    let rt = Rt::new();
    rt.register_module_def(def("life.mjs", &[], decl, bind, eval));
    let m = rt.load_module("life.mjs").expect("loads");
    assert_eq!(rt.module_status(m), ModuleStatus::Evaluated);
    assert_eq!(take_log(), vec!["life".to_string()]);
}

#[test]
fn loading_a_module_while_modules_are_being_linked_throws() {
    fn bind_refuses(rt: &Rt, _m: ObjRef) -> Res<()> {
        rt.load_module("other.mjs").map(|_| ())
    }
    let rt = Rt::new();
    rt.register_module_def(def("outer.mjs", &[], nothing, bind_refuses, nothing));
    rt.register_module_def(def("other.mjs", &[], nothing, nothing, |_rt, _m| {
        log("other");
        Ok(())
    }));
    let e = rt.load_module("outer.mjs").expect_err("refused while linking");
    assert_eq!(error_name(&rt, &e.0), "Error");
    assert!(error_message(&rt, &e.0).contains("while modules are being linked"));
    assert!(take_log().is_empty(), "the nested module must not run");
    assert!(rt.module_record("other.mjs").is_none(), "the nested module must not be linked");
}

#[test]
fn registering_the_same_definition_twice_is_a_no_op() {
    let rt = Rt::new();
    let d = def("again.mjs", &[], nothing, nothing, |_rt, _m| {
        log("again");
        Ok(())
    });
    rt.register_module_def(d);
    rt.register_module_def(d);
    rt.load_module("again.mjs").expect("loads");
    assert_eq!(take_log(), vec!["again".to_string()]);
}

// ----- framework bugs (panics) -----

#[test]
#[should_panic(expected = "registered twice")]
fn registering_a_different_definition_for_the_same_specifier_panics() {
    let rt = Rt::new();
    rt.register_module_def(def("dup.mjs", &[], nothing, nothing, nothing));
    rt.register_module_def(def("dup.mjs", &[], nothing, nothing, nothing));
}

#[test]
#[should_panic(expected = "already has a record")]
fn module_new_for_an_existing_specifier_panics() {
    let rt = Rt::new();
    let _first = rt.module_new("twice.mjs");
    let _second = rt.module_new("twice.mjs");
}

#[test]
#[should_panic(expected = "GC inside running code")]
fn collection_inside_module_code_panics() {
    let rt = Rt::new();
    rt.register_module_def(def("gc.mjs", &[], nothing, nothing, |rt, _m| {
        rt.collect();
        Ok(())
    }));
    let _ = rt.load_module("gc.mjs");
}

#[test]
#[should_panic(expected = "without listing it in requests")]
fn importing_from_an_unlisted_request_panics() {
    fn bind_unlisted(rt: &Rt, m: ObjRef) -> Res<()> {
        rt.module_import_cell(m, "other.mjs", "x").map(|_| ())
    }
    let rt = Rt::new();
    rt.register_module_def(def("unlisted.mjs", &[], nothing, bind_unlisted, nothing));
    let _ = rt.load_module("unlisted.mjs");
}

// ----- negative control -----

#[test]
fn negative_control_a_reversed_expectation_is_reported() {
    let rt = Rt::new();
    register_diamond(&rt);
    let err = run_case(&rt, "diamond", |v| v.reverse());
    assert!(err.is_err(), "a reversed evaluation order must be reported as a mismatch");
    assert!(err.unwrap_err().contains("step 0"), "the report names the step");

    let rt = Rt::new();
    register_diamond(&rt);
    assert!(run_case(&rt, "diamond", |_| {}).is_ok(), "the true expectation still matches");
}
