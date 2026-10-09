//! WP-RT-B error routing and lifecycle tests (crates/runtime/src/errors.rs, lifecycle.rs, instance.rs). Expectations follow
//! runtime-core 3.5.43 (production): handleError, callWithAsyncErrorHandling, injectHook, registerKeepAliveHook.

mod rtb_common;

use std::cell::RefCell;
use std::rc::Rc;

use rtb_common::{log_and_return, log_args, object, text_of, throw_message, Env};
use vn_runtime::errors::{call_with_async_error_handling, call_with_error_handling, handle_error, log_error, set_error_sink, ErrType, ErrorSink};
use vn_runtime::instance::{create_component_instance, get_current_instance, instance_of, with_instance, LifeHook};
use vn_runtime::lifecycle::inject_hook;
use vn_value::{strict_eq, Call, Res, Rt, Throw, Value};

const INFO_CODE_3: &str = "https://vuejs.org/error-reference/#runtime-3";

/// A vnode whose type is a component object (Vue always has one: `type.inheritAttrs` is read at creation).
fn vnode(env: &Env, ty: Value) -> Value {
    let ty = if ty.is_undefined() { object(&env.rt, vec![]) } else { ty };
    object(&env.rt, vec![("type", ty)])
}

/// An app context with the given config object.
fn app_context(env: &Env, config: Value) -> Value {
    object(&env.rt, vec![("config", config), ("provides", Value::Null)])
}

fn hook(env: &Env, label: &str, result: Value) -> Value {
    Value::Object(env.rt.new_native_function(log_and_return, label, 0, vec![env.log_value(), env.s(label), result]))
}

#[derive(Default)]
struct RecordingSink(RefCell<Vec<String>>);

impl ErrorSink for RecordingSink {
    fn report(&self, rt: &Rt, err: &Value) {
        self.0.borrow_mut().push(text_of(rt, err));
    }
}

#[test]
fn error_captured_hooks_run_from_the_parent_up_and_a_false_result_stops_the_walk() {
    let env = Env::new();
    let rt = &env.rt;
    let app = app_context(&env, object(rt, vec![]));
    let grand = create_component_instance(rt, &vnode(&env, Value::Undefined), None, &app).unwrap();
    let parent = create_component_instance(rt, &vnode(&env, Value::Undefined), Some(grand), &app).unwrap();
    let child = create_component_instance(rt, &vnode(&env, Value::Undefined), Some(parent), &app).unwrap();
    inject_hook(rt, LifeHook::Ec, &hook(&env, "G", Value::Undefined), Some(grand), false).unwrap();
    inject_hook(rt, LifeHook::Ec, &hook(&env, "P", Value::Bool(false)), Some(parent), false).unwrap();
    handle_error(rt, &Value::String(vn_value::JsStr::from_str("boom")), Some(child), ErrType::Code(3)).unwrap();
    assert_eq!(env.log_lines(), ["P"], "the parent's hook returned false, so the grandparent's hook does not run");
}

#[test]
fn error_captured_hooks_without_false_all_run_parent_first() {
    let env = Env::new();
    let rt = &env.rt;
    let app = app_context(&env, object(rt, vec![]));
    let grand = create_component_instance(rt, &vnode(&env, Value::Undefined), None, &app).unwrap();
    let child = create_component_instance(rt, &vnode(&env, Value::Undefined), Some(grand), &app).unwrap();
    inject_hook(rt, LifeHook::Ec, &hook(&env, "G", Value::Undefined), Some(grand), false).unwrap();
    inject_hook(rt, LifeHook::Ec, &hook(&env, "C", Value::Undefined), Some(child), false).unwrap();
    handle_error(rt, &Value::Number(1.0), Some(child), ErrType::Hook(LifeHook::Bm)).unwrap();
    assert_eq!(env.log_lines(), ["G"], "errorCaptured hooks are looked up from the parent, never the instance itself");
}

#[test]
fn the_app_error_handler_gets_the_error_proxy_and_info_when_no_hook_captures() {
    let env = Env::new();
    let rt = &env.rt;
    let handler = Value::Object(rt.new_native_function(log_args, "errorHandler", 3, vec![env.log_value()]));
    let app = app_context(&env, object(rt, vec![("errorHandler", handler)]));
    let child = create_component_instance(rt, &vnode(&env, Value::Undefined), None, &app).unwrap();
    handle_error(rt, &Value::String(vn_value::JsStr::from_str("boom")), Some(child), ErrType::Code(3)).unwrap();
    assert_eq!(env.log_lines(), ["boom", "null", INFO_CODE_3]);
}

#[test]
fn log_error_reports_to_the_sink_and_rethrows_only_when_configured() {
    let env = Env::new();
    let rt = &env.rt;
    let sink = Rc::new(RecordingSink::default());
    set_error_sink(rt, sink.clone());
    handle_error(rt, &Value::String(vn_value::JsStr::from_str("quiet")), None, ErrType::Hook(LifeHook::Bm)).unwrap();
    assert_eq!(sink.0.borrow().as_slice(), ["quiet"]);
    let thrown = log_error(rt, &Value::Number(7.0), &Value::Bool(true));
    assert!(matches!(thrown, Err(Throw(Value::Number(n))) if n == 7.0));
    let config = object(rt, vec![("throwUnhandledErrorInProduction", Value::Bool(true))]);
    let app = app_context(&env, config);
    let inst = create_component_instance(rt, &vnode(&env, Value::Undefined), None, &app).unwrap();
    let rethrown = handle_error(rt, &Value::Number(8.0), Some(inst), ErrType::Code(1));
    assert!(matches!(rethrown, Err(Throw(Value::Number(n))) if n == 8.0));
}

#[test]
fn call_with_error_handling_turns_a_throw_into_undefined_and_reports_it() {
    let env = Env::new();
    let rt = &env.rt;
    let sink = Rc::new(RecordingSink::default());
    set_error_sink(rt, sink.clone());
    let f = Value::Object(rt.new_native_function(throw_message, "f", 0, vec![env.s("bad")]));
    let r = call_with_error_handling(rt, &f, None, ErrType::Code(2), None).unwrap();
    assert!(r.is_undefined());
    assert_eq!(sink.0.borrow().as_slice(), ["TypeError: bad"]);
}

#[test]
fn a_rejected_promise_result_goes_to_the_error_handler_with_the_watch_type() {
    let env = Env::new();
    let rt = &env.rt;
    let handler = Value::Object(rt.new_native_function(log_args, "errorHandler", 3, vec![env.log_value()]));
    let app = app_context(&env, object(rt, vec![("errorHandler", handler)]));
    let inst = create_component_instance(rt, &vnode(&env, Value::Undefined), None, &app).unwrap();
    let promise_ctor = rt.get_str(&Value::Object(rt.intrinsic(vn_value::Intrinsic::Global)), "Promise").unwrap();
    let reject = rt.get_str(&promise_ctor, "reject").unwrap();
    let rejecting = Value::Object(rt.new_native_function(reject_with, "rejecting", 0, vec![reject, env.s("nope")]));
    let res = call_with_async_error_handling(rt, &rejecting, Some(inst), ErrType::Code(3), None).unwrap();
    assert!(vn_runtime::errors::is_promise(rt, &res).unwrap());
    rt.run_jobs();
    assert_eq!(env.log_lines(), ["nope", "null", INFO_CODE_3]);
    assert!(rt.take_unhandled_rejections().is_empty(), "the catch handler handles the rejection");
}

/// Data: [reject function, reason]. Calls `Promise.reject(reason)`.
fn reject_with(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let ctor = rt.get_str(&Value::Object(rt.intrinsic(vn_value::Intrinsic::Global)), "Promise")?;
    rt.call(&d[0], &ctor, &[d[1].clone()])
}

#[test]
fn inject_hook_prepends_and_runs_each_hook_with_its_instance_as_the_current_one() {
    let env = Env::new();
    let rt = &env.rt;
    let app = app_context(&env, object(rt, vec![]));
    let p = create_component_instance(rt, &vnode(&env, Value::Undefined), None, &app).unwrap();
    let who = |label: &str| Value::Object(rt.new_native_function(who_code, label, 0, vec![env.log_value(), env.s(label), Value::Object(p)]));
    let first = inject_hook(rt, LifeHook::Bm, &who("h1"), Some(p), false).unwrap().unwrap();
    let second = inject_hook(rt, LifeHook::Bm, &who("h2"), Some(p), true).unwrap().unwrap();
    let list: Vec<Value> = instance_of(rt, p).hooks[LifeHook::Bm as usize].borrow().clone();
    assert!(strict_eq(&list[0], &second) && strict_eq(&list[1], &first), "prepend puts h2 first");
    for h in list {
        call_with_async_error_handling(rt, &h, Some(p), ErrType::Hook(LifeHook::Bm), Some(&[])).unwrap();
    }
    assert_eq!(env.log_lines(), ["h2=target", "h1=target"]);
    assert!(get_current_instance(rt).is_none(), "the wrapper restores the previous current instance");
}

/// Data: [log, label, target]. Logs whether the current instance is the target.
fn who_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let target = d[2].as_object().expect("target");
    let same = get_current_instance(rt) == Some(target);
    let text = format!("{}={}", text_of(rt, &d[1]), if same { "target" } else { "other" });
    rtb_common::append(rt, &d[0], Value::String(vn_value::JsStr::from_str(&text)))?;
    Ok(Value::Undefined)
}

#[test]
fn a_throwing_lifecycle_hook_is_routed_to_the_handler_and_the_instance_is_restored() {
    let env = Env::new();
    let rt = &env.rt;
    let sink = Rc::new(RecordingSink::default());
    set_error_sink(rt, sink.clone());
    let app = app_context(&env, object(rt, vec![]));
    let p = create_component_instance(rt, &vnode(&env, Value::Undefined), None, &app).unwrap();
    let thrower = Value::Object(rt.new_native_function(throw_message, "thrower", 0, vec![env.s("hook failed")]));
    let w = inject_hook(rt, LifeHook::M, &thrower, Some(p), false).unwrap().unwrap();
    let r = rt.call(&w, &Value::Undefined, &[]).unwrap();
    assert!(r.is_undefined());
    assert_eq!(sink.0.borrow().as_slice(), ["TypeError: hook failed"]);
    assert!(get_current_instance(rt).is_none());
}

#[test]
fn the_wrapper_is_cached_on_the_hook_so_one_function_registers_once_per_target_pair() {
    let env = Env::new();
    let rt = &env.rt;
    let app = app_context(&env, object(rt, vec![]));
    let p = create_component_instance(rt, &vnode(&env, Value::Undefined), None, &app).unwrap();
    let c = create_component_instance(rt, &vnode(&env, Value::Undefined), Some(p), &app).unwrap();
    let h = hook(&env, "H", Value::Undefined);
    let wp = inject_hook(rt, LifeHook::Ec, &h, Some(p), false).unwrap().unwrap();
    let wc = inject_hook(rt, LifeHook::Ec, &h, Some(c), false).unwrap().unwrap();
    assert!(strict_eq(&wp, &wc), "the cached __weh is reused, as in Vue (its target is the first one)");
    assert!(strict_eq(&rt.get_str(&h, "__weh").unwrap(), &wp));
    let prim = inject_hook(rt, LifeHook::Ec, &Value::Number(5.0), Some(p), false);
    assert!(prim.is_err(), "a primitive hook cannot take the __weh property (strict mode)");
}

#[test]
fn on_mounted_forwards_its_arguments_and_registers_on_the_explicit_target() {
    let env = Env::new();
    let rt = &env.rt;
    let app = app_context(&env, object(rt, vec![]));
    let p = create_component_instance(rt, &vnode(&env, Value::Undefined), None, &app).unwrap();
    let hook_fn = Value::Object(rt.new_native_function(log_args, "h", 2, vec![env.log_value()]));
    rt.call(&env.export("onMounted"), &Value::Undefined, &[hook_fn, Value::Object(p)]).unwrap();
    let list: Vec<Value> = instance_of(rt, p).hooks[LifeHook::M as usize].borrow().clone();
    assert_eq!(list.len(), 1);
    rt.call(&list[0], &Value::Undefined, &[Value::Number(1.0), Value::String(vn_value::JsStr::from_str("two"))]).unwrap();
    assert_eq!(env.log_lines(), ["1", "two"]);
}

#[test]
fn on_error_captured_defaults_to_the_current_instance_and_is_a_no_op_outside_one() {
    let env = Env::new();
    let rt = &env.rt;
    let app = app_context(&env, object(rt, vec![]));
    let p = create_component_instance(rt, &vnode(&env, Value::Undefined), None, &app).unwrap();
    let hook_fn = hook(&env, "E", Value::Undefined);
    with_instance(rt, p, |rt| rt.call(&env.export("onErrorCaptured"), &Value::Undefined, &[hook_fn.clone()])).unwrap();
    assert_eq!(instance_of(rt, p).hooks[LifeHook::Ec as usize].borrow().len(), 1);
    rt.call(&env.export("onErrorCaptured"), &Value::Undefined, &[hook_fn]).unwrap();
    assert_eq!(instance_of(rt, p).hooks[LifeHook::Ec as usize].borrow().len(), 1, "no current instance, so nothing is added");
}

#[test]
fn keep_alive_activation_hooks_are_prepended_on_the_keep_alive_root_and_removed_on_unmount() {
    let env = Env::new();
    let rt = &env.rt;
    let app = app_context(&env, object(rt, vec![]));
    let keep_alive_type = object(rt, vec![("__isKeepAlive", Value::Bool(true))]);
    let root = create_component_instance(rt, &vnode(&env, keep_alive_type), None, &app).unwrap();
    let mid = create_component_instance(rt, &vnode(&env, Value::Undefined), Some(root), &app).unwrap();
    let target = create_component_instance(rt, &vnode(&env, Value::Undefined), Some(mid), &app).unwrap();
    let hook_fn = hook(&env, "A", Value::Undefined);
    rt.call(&env.export("onActivated"), &Value::Undefined, &[hook_fn, Value::Object(target)]).unwrap();
    // injectToKeepAliveRoot(hook, type, target, current): `current` is the instance whose parent is the KeepAlive.
    assert_eq!(instance_of(rt, mid).hooks[LifeHook::A as usize].borrow().len(), 1, "registered on the child of KeepAlive");
    assert_eq!(instance_of(rt, root).hooks[LifeHook::A as usize].borrow().len(), 0);
    assert_eq!(instance_of(rt, target).hooks[LifeHook::Um as usize].borrow().len(), 1, "removed again when the target unmounts");
    let remover = instance_of(rt, target).hooks[LifeHook::Um as usize].borrow()[0].clone();
    rt.call(&remover, &Value::Undefined, &[]).unwrap();
    assert_eq!(instance_of(rt, mid).hooks[LifeHook::A as usize].borrow().len(), 0);
}

#[test]
fn a_deactivated_target_skips_its_activated_hook() {
    let env = Env::new();
    let rt = &env.rt;
    let app = app_context(&env, object(rt, vec![]));
    let target = create_component_instance(rt, &vnode(&env, Value::Undefined), None, &app).unwrap();
    let hook_fn = hook(&env, "A", Value::Undefined);
    rt.call(&env.export("onActivated"), &Value::Undefined, &[hook_fn, Value::Object(target)]).unwrap();
    let wrapper = instance_of(rt, target).hooks[LifeHook::A as usize].borrow()[0].clone();
    rt.call(&wrapper, &Value::Undefined, &[]).unwrap();
    assert_eq!(env.log_lines(), ["A"]);
    instance_of(rt, target).is_deactivated.set(true);
    rt.call(&wrapper, &Value::Undefined, &[]).unwrap();
    assert_eq!(env.log_lines(), ["A"], "a deactivated instance skips the hook");
}
