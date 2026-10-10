//! Owned operations (crates/runtime/src/operations.rs, design.md 3.4): scope-owned cancellation, stale-delivery rejection and
//! explicit cancel, driven through the `vue` exports, the renderer and the real promise and scheduler jobs. Expected orders
//! of the watcher, render and unmount steps come from the Node oracle tests/vue/owned-ops/record.mjs (vue@3.5.43, production
//! build), copied by hand. The other expected values follow design.md section 3.4 and section 6, as cited per test.

mod rtb_common;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rtb_common::{append, logger, object, text_of};
use vn_runtime::dom_events::DomCtx;
use vn_runtime::host::{Host, HostNode, HostNs};
use vn_runtime::host_doc::DocHost;
use vn_runtime::operations::{self, Delivery, DeliveryToken, ServiceHost};
use vn_runtime::renderer;
use vn_value::builtins::promise::{promise_state, PromiseState};
use vn_value::{Call, CodeFn, Intrinsic, ObjRef, PropKey, Res, Rt, RootId, Value, WeakRt};

/// A deterministic service host. `start` records the request, the test completes it when it chooses, and `cancel` is recorded
/// and also written to the JS log as "host-cancel <label>", so that its order against the JS cleanups is observable.
struct FakeHost {
    rt: WeakRt,
    log: ObjRef,
    started: RefCell<Vec<(DeliveryToken, String)>>,
    cancelled: RefCell<Vec<DeliveryToken>>,
}

impl ServiceHost for FakeHost {
    fn start(&self, token: DeliveryToken, label: &str) {
        self.started.borrow_mut().push((token, label.to_string()));
    }

    fn cancel(&self, token: DeliveryToken) {
        self.cancelled.borrow_mut().push(token);
        let label = self.label_of(token);
        if let Some(rt) = self.rt.upgrade() {
            append(&rt, &Value::Object(self.log), Value::str(&format!("host-cancel {label}"))).expect("log append");
        }
    }
}

impl FakeHost {
    fn label_of(&self, token: DeliveryToken) -> String {
        self.started.borrow().iter().find(|(t, _)| *t == token).map(|(_, l)| l.clone()).unwrap_or_default()
    }

    /// The token of the n-th started operation.
    fn token(&self, n: usize) -> DeliveryToken {
        self.started.borrow()[n].0
    }

    fn started_count(&self) -> usize {
        self.started.borrow().len()
    }
}

/// A realm with the `vue` exports, a DOM host with the renderer installed, a container, the fake service host, and three JS
/// arrays that the test components push into: `promises` (owned promises), `refs` (component refs) and `resolvers`.
struct Env {
    rt: Rt,
    vue: HashMap<&'static str, Value>,
    _host: DocHost,
    _ctx: DomCtx,
    container: HostNode,
    log: ObjRef,
    promises: ObjRef,
    refs: ObjRef,
    resolvers: ObjRef,
    fake: Rc<FakeHost>,
    _roots: Vec<RootId>,
}

impl Env {
    fn new() -> Env {
        let rt = Rt::new();
        let mut roots = Vec::new();
        let mut vue = HashMap::new();
        for (name, v) in vn_runtime::exports::all(&rt) {
            roots.push(rt.root(&v));
            vue.insert(name, v);
        }
        let log = rt.new_array(Vec::new());
        let promises = rt.new_array(Vec::new());
        let refs = rt.new_array(Vec::new());
        let resolvers = rt.new_array(Vec::new());
        for o in [log, promises, refs, resolvers] {
            roots.push(rt.root(&Value::Object(o)));
        }
        let fake = Rc::new(FakeHost { rt: rt.downgrade(), log, started: RefCell::new(Vec::new()), cancelled: RefCell::new(Vec::new()) });
        operations::install(&rt, fake.clone());
        let host = DocHost::new();
        let ctx = DomCtx::new(&rt, &host);
        renderer::install(&ctx);
        let container = {
            let mut h = host.clone();
            h.create_element(HostNs::Html, "div", None).expect("container")
        };
        Env { rt, vue, _host: host, _ctx: ctx, container, log, promises, refs, resolvers, fake, _roots: roots }
    }

    fn export(&self, name: &str) -> Value {
        self.vue.get(name).cloned().unwrap_or_else(|| panic!("vue export {name} (test bug)"))
    }

    fn native(&self, name: &str, code: CodeFn, data: Vec<Value>) -> Value {
        Value::Object(self.rt.new_native_function(code, name, 0, data))
    }

    fn call(&self, f: &Value, this: &Value, args: &[Value]) -> Value {
        self.rt.call(f, this, args).expect("call")
    }

    /// `obj[name](...args)`.
    fn method(&self, obj: &Value, name: &str, args: &[Value]) -> Value {
        let f = self.rt.get_str(obj, name).expect("method");
        self.call(&f, obj, args)
    }

    fn at(&self, arr: ObjRef, n: usize) -> Value {
        self.rt.get_v(&Value::Object(arr), &PropKey::Index(n as u32)).expect("array item")
    }

    fn promise(&self, n: usize) -> Value {
        self.at(self.promises, n)
    }

    fn set_ref(&self, n: usize, v: Value) {
        self.rt.set_str(&self.at(self.refs, n), "value", v).expect("set ref");
    }

    /// The text of the JS log, one entry per line.
    fn lines(&self) -> Vec<String> {
        let log = Value::Object(self.log);
        let len = self.rt.to_number(&self.rt.get_str(&log, "length").expect("length")).expect("length") as u32;
        (0..len).map(|i| text_of(&self.rt, &self.rt.get_v(&log, &PropKey::Index(i)).expect("log item"))).collect()
    }

    fn mount(&self, def: &Value) {
        let vnode = self.call(&self.export("createVNode"), &Value::Undefined, &[def.clone(), Value::Null, Value::Null]);
        let vnode = vnode.as_object().expect("vnode");
        renderer::render_into(&self.rt, Some(vnode), self.container).expect("mount");
    }

    fn unmount(&self) {
        renderer::render_into(&self.rt, None, self.container).expect("unmount");
    }

    fn fetcher_def(&self) -> Value {
        let data = vec![
            Value::Object(self.log),
            Value::Object(self.promises),
            Value::Object(self.refs),
            self.export("ref"),
            self.export("watch"),
            self.export("createVNode"),
        ];
        object(&self.rt, vec![("setup", self.native("setup", fetcher_setup, data))])
    }
}

fn is_pending(env: &Env, promise: &Value) -> bool {
    matches!(promise_state(&env.rt, promise), Some(PromiseState::Pending))
}

fn say(rt: &Rt, log: &Value, line: &str) -> Res<()> {
    append(rt, log, Value::str(line))
}

/// A watcher callback that logs its prefix (data 1) and the new value. Data: [log, prefix].
fn watch_log(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let line = format!("{}{}", text_of(rt, &d[1]), text_of(rt, &c.arg(0)));
    say(rt, &d[0], &line)?;
    Ok(Value::Undefined)
}

/// Sets the ref in data 0 to the first argument (a promise continuation or a watcher effect).
fn set_value(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    rt.set_str(&d[0], "value", c.arg(0))?;
    Ok(Value::Undefined)
}

fn noop(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

/// Logs "catch <code>" for a rejection reason with a `code` property. Data: [log].
fn catch_log(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let code = text_of(rt, &rt.get_str(&c.arg(0), "code")?);
    say(rt, &d[0], &format!("catch {code}"))?;
    Ok(Value::Undefined)
}

/// The render of the test components: logs "render <value>" and renders the value as text. Data: [log, ref, createVNode].
fn render_value(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let text = text_of(rt, &rt.get_str(&d[1], "value")?);
    say(rt, &d[0], &format!("render {text}"))?;
    rt.call(&d[2], &Value::Undefined, &[Value::str("p"), Value::Null, Value::str(&text)])
}

/// `new Promise` with its resolving functions, through `Promise.withResolvers` (the same resolution procedure).
fn with_resolvers(rt: &Rt) -> Res<(Value, Value)> {
    let global = Value::Object(rt.intrinsic(Intrinsic::Global));
    let ctor = rt.get_str(&global, "Promise")?;
    let r = rt.call(&rt.get_str(&ctor, "withResolvers")?, &ctor, &[])?;
    Ok((rt.get_str(&r, "promise")?, rt.get_str(&r, "resolve")?))
}

/// Setup of `Fetcher`: a ref starts as "idle", a watcher logs its changes, and the component starts one owned operation
/// ("fetch") whose promise writes the result into the ref. Data: [log, promises, refs, ref, watch, createVNode].
fn fetcher_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let state = rt.call(&d[3], &Value::Undefined, &[Value::str("idle")])?;
    append(rt, &d[2], state.clone())?;
    let watcher = Value::Object(rt.new_native_function(watch_log, "watch", 2, vec![d[0].clone(), Value::str("watch ")]));
    rt.call(&d[4], &Value::Undefined, &[state.clone(), watcher, Value::Undefined])?;
    let started = operations::start_owned(rt, "fetch")?;
    append(rt, &d[1], started.promise.clone())?;
    let on_result = Value::Object(rt.new_native_function(set_value, "onResult", 1, vec![state.clone()]));
    let on_error = Value::Object(rt.new_native_function(noop, "onError", 1, Vec::new()));
    rt.call(&rt.get_str(&started.promise, "then")?, &started.promise, &[on_result, on_error])?;
    Ok(Value::Object(rt.new_native_function(render_value, "render", 0, vec![d[0].clone(), state, d[5].clone()])))
}

/// Setup of `Retained`, the ordinary-promise scenario: logs "setup", then makes a promise that the component does not own.
/// Its continuation logs and writes the store, and its resolve function goes into the resolvers array.
/// Data: [log, store, resolvers, createVNode].
fn retained_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    say(rt, &d[0], "setup")?;
    let (promise, resolve) = with_resolvers(rt)?;
    append(rt, &d[2], resolve)?;
    let settle = Value::Object(rt.new_native_function(retained_settle, "settle", 1, vec![d[0].clone(), d[1].clone()]));
    rt.call(&rt.get_str(&promise, "then")?, &promise, &[settle])?;
    Ok(Value::Object(rt.new_native_function(render_value, "render", 0, vec![d[0].clone(), d[1].clone(), d[3].clone()])))
}

/// Data: [log, store].
fn retained_settle(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let v = c.arg(0);
    say(rt, &d[0], &format!("then {}", text_of(rt, &v)))?;
    rt.set_str(&d[1], "value", v)?;
    Ok(Value::Undefined)
}

/// Setup of `Owner`, the Vue-ordering scenario. The order of its steps is the order of the oracle's `unmountOrder`.
/// Data: [log, ref, watch, onBeforeUnmount, onUnmounted, onScopeDispose, onWatcherCleanup, createVNode, refs].
fn owner_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let log = d[0].clone();
    say(rt, &log, "setup")?;
    let state = rt.call(&d[1], &Value::Undefined, &[Value::Number(0.0)])?;
    append(rt, &d[8], state.clone())?;
    let before_unmount = Value::Object(rt.new_native_function(logger, "bum", 0, vec![log.clone(), Value::str("bum")]));
    rt.call(&d[3], &Value::Undefined, &[before_unmount])?;
    let unmounted = Value::Object(rt.new_native_function(logger, "um", 0, vec![log.clone(), Value::str("um")]));
    rt.call(&d[4], &Value::Undefined, &[unmounted])?;
    let watcher = Value::Object(rt.new_native_function(owner_watch, "watch", 1, vec![log.clone(), d[6].clone()]));
    rt.call(&d[2], &Value::Undefined, &[state.clone(), watcher, Value::Undefined])?;
    let user_cleanup = Value::Object(rt.new_native_function(logger, "user-dispose", 0, vec![log.clone(), Value::str("user-dispose")]));
    rt.call(&d[5], &Value::Undefined, &[user_cleanup])?;
    operations::start_owned(rt, "op")?;
    Ok(Value::Object(rt.new_native_function(render_value, "render", 0, vec![log, state, d[7].clone()])))
}

/// Data: [log, onWatcherCleanup]. Registers "watch-cleanup <v>" on the watcher, then logs "watch <v>".
fn owner_watch(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let text = text_of(rt, &c.arg(0));
    let cleanup = Value::Object(rt.new_native_function(logger, "cleanup", 0, vec![d[0].clone(), Value::str(&format!("watch-cleanup {text}"))]));
    rt.call(&d[1], &Value::Undefined, &[cleanup])?;
    say(rt, &d[0], &format!("watch {text}"))?;
    Ok(Value::Undefined)
}

// ----- delivery, cancellation and disposal -----

#[test]
fn completion_while_mounted_resolves_the_promise_and_the_watcher_sees_the_value() {
    let env = Env::new();
    env.mount(&env.fetcher_def());
    assert_eq!(env.lines(), ["render idle"]);
    let token = env.fake.token(0);
    let promise = env.promise(0);
    assert_eq!(operations::complete(&env.rt, token, Value::str("data")).expect("complete"), Delivery::Delivered);
    env.rt.run_jobs();
    assert_eq!(env.lines(), ["render idle", "watch data", "render data"]);
    assert!(matches!(promise_state(&env.rt, &promise), Some(PromiseState::Fulfilled(_))));
    let s = operations::stats(&env.rt);
    assert_eq!((s.started, s.delivered, s.discarded_stopped), (1, 1, 0));
}

#[test]
fn unmount_during_request_discards_the_completion_and_keeps_disposed_effects_stopped() {
    let env = Env::new();
    env.mount(&env.fetcher_def());
    let token = env.fake.token(0);
    let promise = env.promise(0);
    env.unmount();
    env.rt.run_jobs();
    // Disposal asked the host to cancel; the fake host logged it when the component's scope stopped.
    assert_eq!(*env.fake.cancelled.borrow(), vec![token]);
    assert_eq!(env.lines(), ["render idle", "host-cancel fetch"]);
    // The late completion is discarded (design 3.4: never resolves user promises, never runs user callbacks). A write to the
    // disposed component's ref then re-runs neither its watcher nor its render.
    assert_eq!(operations::complete(&env.rt, token, Value::str("data")).expect("late complete"), Delivery::DiscardedStopped);
    env.set_ref(0, Value::str("poke"));
    env.rt.run_jobs();
    assert_eq!(env.lines(), ["render idle", "host-cancel fetch"]);
    assert!(is_pending(&env, &promise), "the promise stays pending: disposal invalidates delivery");
    let s = operations::stats(&env.rt);
    assert_eq!((s.cancelled_by_disposal, s.discarded_stopped, s.delivered), (1, 1, 0));
}

#[test]
fn an_ordinary_promise_keeps_js_behaviour_after_unmount_while_disposed_effects_stay_stopped() {
    // Expected lines copied from `node tests/vue/owned-ops/record.mjs`, scenario ordinary-promise-after-unmount (vue@3.5.43).
    let env = Env::new();
    let store = env.call(&env.export("ref"), &Value::Undefined, &[Value::Number(0.0)]);
    let store_watch = env.native("watch", watch_log, vec![Value::Object(env.log), Value::str("store ")]);
    env.call(&env.export("watch"), &Value::Undefined, &[store.clone(), store_watch, Value::Undefined]);
    let data = vec![Value::Object(env.log), store, Value::Object(env.resolvers), env.export("createVNode")];
    let def = object(&env.rt, vec![("setup", env.native("setup", retained_setup, data))]);
    env.mount(&def);
    env.unmount();
    env.rt.run_jobs();
    let resolve = env.at(env.resolvers, 0);
    env.call(&resolve, &Value::Undefined, &[Value::str("late")]);
    env.rt.run_jobs();
    assert_eq!(env.lines(), ["setup", "render 0", "then late", "store late"]);
}

#[test]
fn a_handle_from_an_earlier_instance_is_rejected_after_remount_and_the_new_instance_is_unaffected() {
    let env = Env::new();
    let def = env.fetcher_def();
    env.mount(&def);
    let old = env.fake.token(0);
    let old_promise = env.promise(0);
    env.unmount();
    env.rt.run_jobs();
    env.mount(&def);
    let new = env.fake.token(1);
    // The owner slot is recycled: the old token names the same slot as the live owner, and only its generation differs. The
    // generation check classifies the old completion as stale. Disposal also removed the old operation's record, so nothing
    // is delivered either way (see docs/evidence/scope-cancellation.md for the mutation runs of this test).
    assert_eq!(new.scope, old.scope);
    assert_eq!(new.generation, old.generation + 1);
    assert_eq!(operations::complete(&env.rt, old, Value::str("stale")).expect("stale complete"), Delivery::DiscardedStale);
    env.rt.run_jobs();
    assert!(is_pending(&env, &old_promise));
    assert_eq!(env.lines(), ["render idle", "host-cancel fetch", "render idle"]);
    assert_eq!(operations::complete(&env.rt, new, Value::str("fresh")).expect("complete"), Delivery::Delivered);
    env.rt.run_jobs();
    assert_eq!(env.lines(), ["render idle", "host-cancel fetch", "render idle", "watch fresh", "render fresh"]);
    assert_eq!(operations::stats(&env.rt).discarded_stale, 1);
}

#[test]
fn an_explicit_cancel_by_a_live_owner_rejects_with_cancelled_and_stops_the_host() {
    let env = Env::new();
    env.mount(&env.fetcher_def());
    let token = env.fake.token(0);
    let promise = env.promise(0);
    let catch = env.native("catch", catch_log, vec![Value::Object(env.log)]);
    env.method(&promise, "then", &[Value::Undefined, catch]);
    assert_eq!(operations::cancel(&env.rt, token).expect("cancel"), Delivery::Cancelled);
    env.rt.run_jobs();
    assert_eq!(env.lines(), ["render idle", "host-cancel fetch", "catch Cancelled"]);
    assert_eq!(operations::complete(&env.rt, token, Value::str("late")).expect("complete"), Delivery::DiscardedSettled);
    env.rt.run_jobs();
    // The owner is still live: a write to its ref still runs its watcher and render.
    env.set_ref(0, Value::str("after"));
    env.rt.run_jobs();
    assert_eq!(env.lines(), ["render idle", "host-cancel fetch", "catch Cancelled", "watch after", "render after"]);
    let s = operations::stats(&env.rt);
    assert_eq!((s.cancelled_by_caller, s.discarded_settled, s.delivered), (1, 1, 0));
}

#[test]
fn cancel_then_completion_in_the_same_tick_rejects_once() {
    let env = Env::new();
    env.mount(&env.fetcher_def());
    let token = env.fake.token(0);
    let catch = env.native("catch", catch_log, vec![Value::Object(env.log)]);
    env.method(&env.promise(0), "then", &[Value::Undefined, catch]);
    assert_eq!(operations::cancel(&env.rt, token).expect("cancel"), Delivery::Cancelled);
    assert_eq!(operations::complete(&env.rt, token, Value::str("x")).expect("complete"), Delivery::DiscardedSettled);
    env.rt.run_jobs();
    assert_eq!(env.lines(), ["render idle", "host-cancel fetch", "catch Cancelled"]);
}

#[test]
fn completion_then_cancel_in_the_same_tick_resolves_once_and_does_not_reach_the_host() {
    let env = Env::new();
    env.mount(&env.fetcher_def());
    let token = env.fake.token(0);
    let catch = env.native("catch", catch_log, vec![Value::Object(env.log)]);
    env.method(&env.promise(0), "then", &[Value::Undefined, catch]);
    assert_eq!(operations::complete(&env.rt, token, Value::str("data")).expect("complete"), Delivery::Delivered);
    assert_eq!(operations::cancel(&env.rt, token).expect("cancel"), Delivery::DiscardedSettled);
    env.rt.run_jobs();
    assert_eq!(env.lines(), ["render idle", "watch data", "render data"]);
    assert!(env.fake.cancelled.borrow().is_empty(), "a settled operation is not cancelled");
}

#[test]
fn a_wrong_generation_is_rejected_while_the_right_one_delivers() {
    let env = Env::new();
    env.mount(&env.fetcher_def());
    let token = env.fake.token(0);
    let promise = env.promise(0);
    let wrong = DeliveryToken { generation: token.generation + 1, ..token };
    assert_eq!(operations::complete(&env.rt, wrong, Value::str("x")).expect("wrong complete"), Delivery::DiscardedStale);
    env.rt.run_jobs();
    assert!(is_pending(&env, &promise));
    assert_eq!(env.lines(), ["render idle"]);
    assert_eq!(operations::complete(&env.rt, token, Value::str("x")).expect("complete"), Delivery::Delivered);
    env.rt.run_jobs();
    assert_eq!(env.lines(), ["render idle", "watch x", "render x"]);
}

#[test]
fn fail_rejects_with_a_service_error_and_unknown_codes_are_refused() {
    let env = Env::new();
    env.mount(&env.fetcher_def());
    let token = env.fake.token(0);
    let catch = env.native("catch", catch_log, vec![Value::Object(env.log)]);
    env.method(&env.promise(0), "then", &[Value::Undefined, catch]);
    assert!(operations::fail(&env.rt, token, "Exploded", "no such code").is_err(), "codes outside design section 5 are refused");
    assert_eq!(operations::fail(&env.rt, token, "Unavailable", "offline").expect("fail"), Delivery::Delivered);
    env.rt.run_jobs();
    assert_eq!(env.lines(), ["render idle", "catch Unavailable"]);
}

#[test]
fn unmount_order_matches_vue_3_5_43() {
    // Expected lines copied from `node tests/vue/owned-ops/record.mjs`, scenario unmount-order (vue@3.5.43, production build).
    // "host-cancel op" is the runtime's cancellation, which the oracle stands in for with an onScopeDispose after the user's.
    const VUE_ORDER: [&str; 9] = ["setup", "render 0", "watch 1", "render 1", "bum", "watch-cleanup 1", "user-dispose", "host-cancel op", "um"];
    let env = Env::new();
    let data = vec![
        Value::Object(env.log),
        env.export("ref"),
        env.export("watch"),
        env.export("onBeforeUnmount"),
        env.export("onUnmounted"),
        env.export("onScopeDispose"),
        env.export("onWatcherCleanup"),
        env.export("createVNode"),
        Value::Object(env.refs),
    ];
    let def = object(&env.rt, vec![("setup", env.native("setup", owner_setup, data))]);
    env.mount(&def);
    env.set_ref(0, Value::Number(1.0));
    env.rt.run_jobs();
    env.unmount();
    env.rt.run_jobs();
    env.set_ref(0, Value::Number(2.0));
    env.rt.run_jobs();
    assert_eq!(env.lines(), VUE_ORDER);
    // Negative control: a mutated expectation (the two scope cleanups swapped) must not match.
    let mut mutated = VUE_ORDER.to_vec();
    mutated.swap(6, 7);
    assert_ne!(env.lines(), mutated);
}

// ----- ownership and scope errors -----

#[test]
fn starting_outside_a_scope_or_in_a_stopped_one_throws_before_any_host_call() {
    let env = Env::new();
    let outside = operations::start_owned(&env.rt, "x").err().expect("no active scope");
    assert!(text_of(&env.rt, &outside.0).contains("outside an effect scope"));
    let scope = env.call(&env.export("effectScope"), &Value::Undefined, &[Value::Bool(true)]);
    env.method(&scope, "stop", &[]);
    env.method(&scope, "on", &[]);
    let stopped = operations::start_owned(&env.rt, "x").err().expect("stopped scope");
    env.method(&scope, "off", &[]);
    assert!(text_of(&env.rt, &stopped.0).contains("stopped effect scope"));
    assert_eq!(env.fake.started_count(), 0, "no host request was started");
    assert_eq!(operations::stats(&env.rt).started, 0);
}

#[test]
fn an_effect_scope_owner_cancels_its_operations_when_it_stops() {
    let env = Env::new();
    let scope = env.call(&env.export("effectScope"), &Value::Undefined, &[Value::Bool(false)]);
    env.method(&scope, "on", &[]);
    let started = operations::start_owned(&env.rt, "op2").expect("start");
    env.method(&scope, "off", &[]);
    env.method(&scope, "stop", &[]);
    assert_eq!(env.lines(), ["host-cancel op2"]);
    assert_eq!(*env.fake.cancelled.borrow(), vec![started.token]);
    assert_eq!(operations::complete(&env.rt, started.token, Value::str("late")).expect("late complete"), Delivery::DiscardedStopped);
    env.rt.run_jobs();
    assert!(is_pending(&env, &started.promise));
    let s = operations::stats(&env.rt);
    assert_eq!((s.cancelled_by_disposal, s.discarded_stopped), (1, 1));
}
