//! Module namespace objects (WP-L2c, ECMA-262 10.4.6): contents and key order, live reads, the TDZ, refusals,
//! identity and the refusal to read a namespace while modules are still being linked. The corpus cases
//! namespace-contents and namespace-tdz are probed by the module code itself, with the same texts that Node's
//! probes log (tests/language/corpora/modules.json). The other checks are written by hand from the spec.

mod support;
#[macro_use]
#[path = "modules_common.rs"]
mod common;

use common::*;
use vn_value::{Call, Flags, ModuleStatus, ObjRef, Prop, PropKey, Res, Rt, Throw, Value, WellKnown};

fn key(s: &str) -> PropKey {
    PropKey::from_str(s)
}

fn module(rt: &Rt, spec: &str) -> ObjRef {
    rt.module_record(spec).unwrap_or_else(|| panic!("{spec} has no record"))
}

/// `Object.getOwnPropertyDescriptor` of a data property, as "value,writable,enumerable,configurable".
fn data_line(rt: &Rt, p: Option<Prop>) -> String {
    match p {
        Some(Prop::Data { value, flags }) => format!(
            "{},{},{},{}",
            text(rt, &value).expect("text"),
            flags.writable,
            flags.enumerable,
            flags.configurable
        ),
        other => panic!("expected a data property, found {other:?}"),
    }
}

// ----- corpus: namespace-contents (lib.mjs exports, main.mjs probes) -----

fn f_fn_code(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::str("f"))
}

/// `export function bump() { counter++; }`, where `counter` is captured when `bump` is hoisted.
fn bump_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let cell = rt.captures(c.callee)[0];
    let n = match rt.cell_get(cell) {
        Value::Number(n) => n,
        _ => return Err(rt.throw_type_error("counter is not a number")),
    };
    rt.cell_set(cell, Value::Number(n + 1.0));
    Ok(Value::Undefined)
}

/// Locals: 0 a, 1 b, 2 counter, 3 bump, 4 f, 5 hidden, 6 the default export. Exports: a, b, counter, bump, f,
/// zz (= hidden), "10" (= a), "2" (= b) and default.
fn decl_lib(rt: &Rt, m: ObjRef) -> Res<()> {
    let cells: Vec<ObjRef> = (0..7).map(|_| rt.new_cell(Value::Uninit)).collect();
    rt.module_set_locals(m, cells.clone());
    rt.module_define_export(m, "a", cells[0]);
    rt.module_define_export(m, "b", cells[1]);
    rt.module_define_export(m, "counter", cells[2]);
    rt.module_define_export(m, "bump", cells[3]);
    rt.module_define_export(m, "f", cells[4]);
    rt.module_define_export(m, "zz", cells[5]);
    rt.module_define_export(m, "10", cells[0]);
    rt.module_define_export(m, "2", cells[1]);
    rt.module_define_export(m, "default", cells[6]);
    Ok(())
}

fn bind_lib(rt: &Rt, m: ObjRef) -> Res<()> {
    let counter = rt.module_local(m, 2);
    hoist(rt, m, "bump", bump_code, &[counter])?;
    hoist(rt, m, "f", f_fn_code, &[])
}

fn eval_lib(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "a", Value::Number(1.0))?;
    init_export(rt, m, "b", Value::Number(2.0))?;
    init_export(rt, m, "counter", Value::Number(0.0))?;
    init_export(rt, m, "zz", Value::Number(3.0))?;
    init_export(rt, m, "default", Value::str("dflt"))
}

fn bind_main_contents(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_import_cell(m, "lib.mjs", "bump").map(|_| ())
}

/// The probes of main.mjs in namespace-contents, one log line each, in the same order and with the same text.
fn probe_contents(rt: &Rt, m: ObjRef) -> Res<()> {
    let lib = rt.module_request(m, "lib.mjs");
    let ns = rt.module_namespace(lib);
    let ns2 = rt.module_namespace(lib);
    let nsv = Value::Object(ns);
    let bump = rt.module_import_cell(m, "lib.mjs", "bump")?;
    let bump = rt.cell_get_checked(bump, "bump")?;

    log(format!("keys:{}", enumerable_keys(rt, ns)?.join(",")));

    let mut own = Vec::new();
    for k in rt.own_keys(ns)? {
        own.push(match &k {
            PropKey::Index(i) => i.to_string(),
            PropKey::Str(s) => s.to_rust_string_lossy(),
            PropKey::Sym(s) => format!("sym:{}", rt.symbol_description(*s).map(|d| d.to_rust_string_lossy()).unwrap_or_default()),
        });
    }
    log(format!("own:{}", own.join(",")));
    log(format!("proto:{}", rt.get_prototype_of(ns).is_none()));
    log(format!("ext:{}", rt.is_extensible(ns)));
    let tag = rt.get_v(&nsv, &PropKey::Sym(rt.well_known(WellKnown::ToStringTag)))?;
    log(format!("tag:{}", text(rt, &tag)?));

    let mut vals = Vec::new();
    for name in ["10", "2", "a", "b", "zz", "default"] {
        vals.push(text(rt, &rt.get_str(&nsv, name)?)?);
    }
    let f = rt.get_str(&nsv, "f")?;
    vals.push(text(rt, &rt.call(&f, &Value::Undefined, &[])?)?);
    vals.push(text(rt, &rt.get_str(&nsv, "counter")?)?);
    vals.push(text(rt, &rt.get_str(&nsv, "missing")?)?);
    log(format!("vals:{}", vals.join(",")));

    log(format!("desc:{}", data_line(rt, rt.get_own_property(ns, &key("a"))?)));
    let tag_prop = rt.get_own_property(ns, &PropKey::Sym(rt.well_known(WellKnown::ToStringTag)))?;
    log(format!("desctag:{}", data_line(rt, tag_prop)));

    log(format!("set:{}", rt.set(ns, &key("a"), Value::Number(5.0), &nsv)?));
    // Reflect.defineProperty(ns, 'a', { value: 9 }) is refused; so is the complete descriptor with another value.
    let define = Prop::Data { value: Value::Number(9.0), flags: Flags::new(true, true, false) };
    log(format!("define:{}", rt.define_own(ns, &key("a"), define)?));
    log(format!("delete:{}", rt.delete(ns, &key("a"))?));
    log(format!("delete2:{}", rt.delete(ns, &key("nope"))?));
    log(format!(
        "has:{},{},{}",
        rt.has_property(ns, &key("a"))?,
        rt.has_property(ns, &key("nope"))?,
        rt.has_property(ns, &key("zz"))?
    ));
    log(format!("same:{}", ns == ns2));
    match rt.set_v_strict(&nsv, &key("a"), Value::Number(2.0)) {
        Ok(()) => log("assign:none"),
        Err(Throw(e)) => log(format!("assign:{}", error_name(rt, &e))),
    }
    // Object.seal(ns) returns ns when the kernel's seal succeeds.
    log(format!("seal:{}", rt.seal(ns)?));
    // Object.freeze throws when the kernel refuses the integrity level (here: the read-only define).
    log(if rt.freeze(ns)? { "freeze:none" } else { "freeze:TypeError" });
    rt.call(&bump, &Value::Undefined, &[])?;
    log(format!("live:{}", text(rt, &rt.get_str(&nsv, "counter")?)?));
    Ok(())
}

#[test]
fn namespace_contents_and_probes_match_node() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["lib.mjs"], nothing, bind_main_contents, probe_contents));
    rt.register_module_def(def("lib.mjs", &[], decl_lib, bind_lib, eval_lib));
    check_case(&rt, "namespace-contents");
}

#[test]
fn negative_control_reversed_namespace_probes_are_reported() {
    // The same probes with the expected lines reversed must not match; the report names the case.
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["lib.mjs"], nothing, bind_main_contents, probe_contents));
    rt.register_module_def(def("lib.mjs", &[], decl_lib, bind_lib, eval_lib));
    let err = run_case(&rt, "namespace-contents", |v| v.reverse()).expect_err("reversed lines are a mismatch");
    assert!(err.starts_with("namespace-contents"), "{err}");
}

// ----- corpus: namespace-tdz (a namespace read while its module is still uninitialised) -----

fn probe_tdz(rt: &Rt, m: ObjRef) -> Res<()> {
    let ns = rt.module_namespace(rt.module_request(m, "lib.mjs"));
    log(format!("in:{}", rt.has_property(ns, &key("v"))?));
    match enumerable_keys(rt, ns) {
        Ok(_) => log("keys:ok"),
        Err(Throw(e)) => log(format!("keys:{}", error_name(rt, &e))),
    }
    match rt.get_str(&Value::Object(ns), "v") {
        Ok(v) => log(format!("read:{}", text(rt, &v)?)),
        Err(Throw(e)) => log(format!("tdz:{}", error_name(rt, &e))),
    }
    Ok(())
}

fn bind_probe(rt: &Rt, m: ObjRef) -> Res<()> {
    rt.module_import_cell(m, "lib.mjs", "v").map(|_| ())
}

fn eval_main_tdz(_rt: &Rt, _m: ObjRef) -> Res<()> {
    log("main");
    Ok(())
}

fn eval_lib_v(rt: &Rt, m: ObjRef) -> Res<()> {
    init_export(rt, m, "v", Value::Number(1.0))?;
    log("lib");
    Ok(())
}

declare_exports!(decl_v, ["v"]);

#[test]
fn namespace_read_before_the_export_is_initialised_is_a_reference_error() {
    let rt = Rt::new();
    rt.register_module_def(def("main.mjs", &["lib.mjs"], nothing, nothing, eval_main_tdz));
    rt.register_module_def(def("lib.mjs", &["probe.mjs"], decl_v, nothing, eval_lib_v));
    rt.register_module_def(def("probe.mjs", &["lib.mjs"], nothing, bind_probe, probe_tdz));
    check_case(&rt, "namespace-tdz");
}

// ----- direct checks -----

fn load_lib_namespace(rt: &Rt) -> ObjRef {
    rt.register_module_def(def("main.mjs", &["lib.mjs"], nothing, bind_main_contents, nothing));
    rt.register_module_def(def("lib.mjs", &[], decl_lib, bind_lib, eval_lib));
    let main = rt.load_module("main.mjs").expect("loads");
    assert_eq!(rt.module_status(main), ModuleStatus::Evaluated);
    rt.module_namespace(module(rt, "lib.mjs"))
}

#[test]
fn namespace_is_created_once_and_has_null_prototype_and_no_extensions() {
    let rt = Rt::new();
    let ns = load_lib_namespace(&rt);
    assert_eq!(rt.module_namespace(module(&rt, "lib.mjs")), ns, "one namespace per module");
    assert_eq!(rt.get_prototype_of(ns), None);
    assert!(!rt.is_extensible(ns));
    assert!(rt.set_prototype_of(ns, None), "keeping the null prototype is allowed");
    assert!(!rt.set_prototype_of(ns, Some(rt.new_object())), "no other prototype can be set");
}

#[test]
fn namespace_to_string_tag_is_module_and_read_only() {
    let rt = Rt::new();
    let ns = load_lib_namespace(&rt);
    let tag = rt.get_own_property(ns, &PropKey::Sym(rt.well_known(WellKnown::ToStringTag))).expect("no throw");
    assert_eq!(data_line(&rt, tag), "Module,false,false,false");
    let v = rt.get(ns, &PropKey::Sym(rt.well_known(WellKnown::ToStringTag)), &Value::Object(ns)).expect("no throw");
    assert_eq!(v, Value::str("Module"));
}

#[test]
fn namespace_refuses_writes_defines_and_deletes_of_exports() {
    let rt = Rt::new();
    let ns = load_lib_namespace(&rt);
    let nsv = Value::Object(ns);
    assert_eq!(rt.set(ns, &key("a"), Value::Number(9.0), &nsv), Ok(false));
    assert_eq!(rt.delete(ns, &key("a")), Ok(false));
    // A definition that changes nothing is accepted; a changed value, or any other attribute, is refused.
    let same = Prop::Data { value: Value::Number(1.0), flags: Flags::new(true, true, false) };
    assert_eq!(rt.define_own(ns, &key("a"), same), Ok(true));
    let changed = Prop::Data { value: Value::Number(9.0), flags: Flags::new(true, true, false) };
    assert_eq!(rt.define_own(ns, &key("a"), changed), Ok(false));
    let configurable = Prop::Data { value: Value::Number(1.0), flags: Flags::new(true, true, true) };
    assert_eq!(rt.define_own(ns, &key("a"), configurable), Ok(false));
    assert_eq!(rt.get_str(&nsv, "a"), Ok(Value::Number(1.0)), "the export is unchanged");
}

#[test]
fn namespace_keys_list_index_names_first_then_the_rest_in_code_unit_order() {
    let rt = Rt::new();
    let ns = load_lib_namespace(&rt);
    let names: Vec<String> = rt
        .own_keys(ns)
        .expect("no throw")
        .iter()
        .map(|k| match k {
            PropKey::Index(i) => i.to_string(),
            PropKey::Str(s) => s.to_rust_string_lossy(),
            PropKey::Sym(_) => "@@toStringTag".to_string(),
        })
        .collect();
    let want = ["2", "10", "a", "b", "bump", "counter", "default", "f", "zz", "@@toStringTag"];
    assert_eq!(names, want, "the order Node reports");
    // Negative control: plain code-unit order (which ECMA-262 names) puts "10" before "2"; it must differ.
    let code_unit: Vec<&str> = vec!["10", "2", "a", "b", "bump", "counter", "default", "f", "zz", "@@toStringTag"];
    assert_ne!(names.iter().map(String::as_str).collect::<Vec<_>>(), code_unit);
}

#[test]
fn namespace_read_of_a_missing_name_is_undefined_and_absent() {
    let rt = Rt::new();
    let ns = load_lib_namespace(&rt);
    let nsv = Value::Object(ns);
    assert_eq!(rt.get_str(&nsv, "nope"), Ok(Value::Undefined));
    assert_eq!(rt.has_property(ns, &key("nope")), Ok(false));
    assert_eq!(rt.get_own_property(ns, &key("nope")), Ok(None));
}

#[test]
fn namespace_read_while_modules_are_being_linked_is_refused() {
    fn decl_reader(rt: &Rt, m: ObjRef) -> Res<()> {
        let ns = rt.module_namespace(rt.module_request(m, "b.mjs"));
        rt.get_str(&Value::Object(ns), "x").map(|_| ())
    }
    let rt = Rt::new();
    rt.register_module_def(def("a.mjs", &["b.mjs"], decl_reader, nothing, nothing));
    rt.register_module_def(def("b.mjs", &[], decl_x, nothing, nothing));
    let e = rt.load_module("a.mjs").expect_err("refused while linking");
    assert!(error_message(&rt, &e.0).contains("before linking completed"));
}

declare_exports!(decl_x, ["x"]);
