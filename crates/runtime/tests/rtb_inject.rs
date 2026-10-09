//! WP-RT-B provide/inject and useId tests (crates/runtime/src/inject.rs). Expectations follow runtime-core 3.5.43
//! (production): provide creates Object.create(parentProvides) lazily, inject reads the parent's provides (the vnode's
//! app context at the root), and useId counts with the ids array shared down the tree.

mod rtb_common;

use rtb_common::{object, text_of, Env};
use vn_runtime::inject::{has_injection_context, inject, provide, use_id};
use vn_runtime::instance::{create_component_instance, current_app, set_current_app, with_instance};
use vn_value::{Call, ObjRef, Res, Rt, Value};

fn app(env: &Env, provides: Value, config: Value) -> Value {
    let rt = &env.rt;
    object(rt, vec![("config", config), ("provides", provides)])
}

/// A vnode whose type and appContext are set, as the renderer sets them.
fn vnode(env: &Env, app: &Value) -> Value {
    let rt = &env.rt;
    object(rt, vec![("type", object(rt, vec![])), ("appContext", app.clone())])
}

fn child_of(env: &Env, parent: Option<ObjRef>, app: &Value) -> ObjRef {
    create_component_instance(&env.rt, &vnode(env, app), parent, app).expect("instance")
}

fn key(env: &Env, name: &str) -> Value {
    env.s(name)
}

#[test]
fn inject_finds_the_nearest_provider_and_provide_is_scoped_to_its_instance() {
    let env = Env::new();
    let rt = &env.rt;
    let app = app(&env, Value::Null, object(rt, vec![]));
    let p = child_of(&env, None, &app);
    let c = child_of(&env, Some(p), &app);
    let g = child_of(&env, Some(c), &app);
    with_instance(rt, p, |rt| provide(rt, &key(&env, "k"), &Value::Number(1.0))).unwrap();
    let got = |inst: ObjRef| with_instance(rt, inst, |rt| inject(rt, &key(&env, "k"), None, false)).unwrap();
    assert!(matches!(got(c), Value::Number(n) if n == 1.0), "a child without its own provider sees the parent's value");
    with_instance(rt, c, |rt| provide(rt, &key(&env, "k"), &Value::Number(2.0))).unwrap();
    assert!(matches!(got(g), Value::Number(n) if n == 2.0), "the grandchild sees the nearest provider");
    assert!(matches!(got(c), Value::Number(n) if n == 1.0), "inject reads the parent's provides, never the instance's own");
}

#[test]
fn inject_defaults_factories_and_the_context_check() {
    let env = Env::new();
    let rt = &env.rt;
    let app = app(&env, Value::Null, object(rt, vec![]));
    let p = child_of(&env, None, &app);
    let missing = key(&env, "missing");
    let dflt = Value::Number(5.0);
    assert!(with_instance(rt, p, |rt| inject(rt, &missing, None, false)).unwrap().is_undefined());
    assert!(matches!(with_instance(rt, p, |rt| inject(rt, &missing, Some(&dflt), false)).unwrap(), Value::Number(n) if n == 5.0));
    let factory = Value::Object(rt.new_native_function(seven, "factory", 0, vec![]));
    assert!(matches!(with_instance(rt, p, |rt| inject(rt, &missing, Some(&factory), true)).unwrap(), Value::Number(n) if n == 7.0));
    let fn_default = factory.clone();
    let as_value = with_instance(rt, p, |rt| inject(rt, &missing, Some(&fn_default), false)).unwrap();
    assert!(vn_value::strict_eq(&as_value, &fn_default), "without the factory flag the function itself is the default");
    assert!(inject(rt, &missing, None, false).unwrap().is_undefined(), "no injection context outside an instance");
    assert!(!has_injection_context(rt));
    with_instance(rt, p, |rt| {
        assert!(has_injection_context(rt));
        Ok(())
    })
    .unwrap();
}

fn seven(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Number(7.0))
}

#[test]
fn the_root_reads_app_level_provides_and_a_running_app_provides_without_an_instance() {
    let env = Env::new();
    let rt = &env.rt;
    let prov = object(rt, vec![("k", Value::Number(9.0))]);
    let app_value = app(&env, prov.clone(), object(rt, vec![]));
    let p = child_of(&env, None, &app_value);
    assert!(matches!(with_instance(rt, p, |rt| inject(rt, &key(&env, "k"), None, false)).unwrap(), Value::Number(n) if n == 9.0));
    let running = object(rt, vec![("_context", object(rt, vec![("provides", prov)]))]);
    let prev = set_current_app(rt, running.clone());
    assert!(matches!(inject(rt, &key(&env, "k"), None, false).unwrap(), Value::Number(n) if n == 9.0));
    assert!(has_injection_context(rt));
    set_current_app(rt, prev);
    assert!(current_app(rt).is_nullish());
}

#[test]
fn use_id_counts_down_the_tree_and_uses_the_prefix() {
    let env = Env::new();
    let rt = &env.rt;
    let plain = app(&env, Value::Null, object(rt, vec![]));
    let p = child_of(&env, None, &plain);
    let c = child_of(&env, Some(p), &plain);
    let id = |inst: ObjRef| with_instance(rt, inst, |rt| use_id(rt)).unwrap();
    assert_eq!(text_of(rt, &id(p)), "v-0");
    assert_eq!(text_of(rt, &id(p)), "v-1");
    assert_eq!(text_of(rt, &id(c)), "v-2", "the child shares its parent's ids array");
    assert_eq!(text_of(rt, &use_id(rt).unwrap()), "", "no instance, no id");
    let prefixed = app(&env, Value::Null, object(rt, vec![("idPrefix", env.s("app"))]));
    let q = child_of(&env, None, &prefixed);
    assert_eq!(text_of(rt, &with_instance(rt, q, |rt| use_id(rt)).unwrap()), "app-0");
}
