//! Promise (WP-B5 part 1): job ordering checked against V8 (the outputs of the Node probes recorded in each test),
//! constructor and static error cases, unhandled rejection bookkeeping.

use vn_value::builtins::promise::{self, PromiseState};
use vn_value::{Call, Flags, Intrinsic, PropKey, Res, Rt, Throw, Value};

/// A JavaScript-visible log array plus the native helpers that append to it.
struct Env {
    rt: Rt,
    log: Value,
}

fn push(rt: &Rt, arr: &Value, v: Value) -> Res<Value> {
    let len = match rt.get_str(arr, "length")? {
        Value::Number(n) => n as u32,
        _ => 0,
    };
    rt.set_v_strict(arr, &PropKey::Index(len), v)?;
    Ok(Value::Undefined)
}

/// Appends its closure label to the log.
fn label_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    push(rt, &d[0], d[1].clone())
}

/// Appends its first argument to the log.
fn arg_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    push(rt, &d[0], c.arg(0))
}

/// Throws its closure value.
fn noop_fn(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

fn throw_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Err(Throw(rt.fn_data(c.callee)[0].clone()))
}

/// Executor of `new Promise(res => res(data))`.
fn adopt_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let inner = rt.fn_data(c.callee)[0].clone();
    rt.call(&c.arg(0), &Value::Undefined, &[inner])
}

/// `then(resolve)` of a thenable: logs "then" (the log is its closure data) and resolves with "x".
fn thenable_then(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let log = rt.fn_data(c.callee)[0].clone();
    push(rt, &log, Value::str("then"))?;
    rt.call(&c.arg(0), &Value::Undefined, &[Value::str("x")])
}

impl Env {
    fn new() -> Env {
        let rt = Rt::new();
        let log = Value::Object(rt.new_array(Vec::new()));
        Env { rt, log }
    }

    fn global(&self) -> Value {
        Value::Object(self.rt.intrinsic(Intrinsic::Global))
    }

    fn promise(&self) -> Value {
        self.rt.get_str(&self.global(), "Promise").expect("Promise is installed")
    }

    fn static_call(&self, name: &str, args: &[Value]) -> Res<Value> {
        let ctor = self.promise();
        let f = self.rt.get_str(&ctor, name)?;
        self.rt.call(&f, &ctor, args)
    }

    fn method(&self, this: &Value, name: &str, args: &[Value]) -> Res<Value> {
        let f = self.rt.get_str(this, name)?;
        self.rt.call(&f, this, args)
    }

    /// Logs `label` right now (synchronous code of a program).
    fn sync(&self, label: &str) {
        push(&self.rt, &self.log, Value::str(label)).expect("log");
    }

    /// A native function that logs `label` when it runs.
    fn label(&self, label: &str) -> Value {
        Value::Object(self.rt.new_native_function(label_fn, "label", 0, vec![self.log.clone(), Value::str(label)]))
    }

    /// A native function that logs its first argument.
    fn logging(&self) -> Value {
        Value::Object(self.rt.new_native_function(arg_fn, "logArg", 1, vec![self.log.clone()]))
    }

    fn noop(&self) -> Value {
        Value::Object(self.rt.new_native_function(noop_fn, "noop", 0, Vec::new()))
    }

    fn thrower(&self, v: Value) -> Value {
        Value::Object(self.rt.new_native_function(throw_fn, "thrower", 0, vec![v]))
    }

    fn resolved(&self, v: Value) -> Value {
        self.static_call("resolve", &[v]).expect("Promise.resolve")
    }

    /// The log entries as strings.
    fn entries(&self) -> Vec<String> {
        let n = match self.rt.get_str(&self.log, "length").expect("length") {
            Value::Number(n) => n as u32,
            _ => 0,
        };
        (0..n)
            .map(|i| match self.rt.get_v(&self.log, &PropKey::Index(i)).expect("entry") {
                Value::String(s) => s.to_rust_string_lossy(),
                Value::Number(x) => format!("{x}"),
                other => format!("{other:?}"),
            })
            .collect()
    }

    fn drain(&self) {
        self.rt.run_jobs();
        assert!(self.rt.take_uncaught_job_errors().is_empty(), "no job may throw");
    }
}

fn s(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|p| p.to_string()).collect()
}

#[test]
fn reactions_run_as_jobs_after_the_synchronous_code() {
    let env = Env::new();
    let p = env.resolved(Value::str("start"));
    env.method(&p, "then", &[env.label("reaction")]).expect("then");
    env.sync("sync");
    assert_eq!(env.entries(), s(&["sync"]), "the reaction waits for the job queue");
    env.drain();
    assert_eq!(env.entries(), s(&["sync", "reaction"]));
}

#[test]
fn then_chains_interleave_in_registration_order_like_v8() {
    // Node 26: sync, a1, b1, a2, b2 (the program of tests/language/cases/promises/then-chain-order.js).
    let env = Env::new();
    let p = env.resolved(Value::str("start"));
    let a1 = env.method(&p, "then", &[env.label("a1")]).expect("then");
    env.method(&a1, "then", &[env.label("a2")]).expect("then");
    let b1 = env.method(&p, "then", &[env.label("b1")]).expect("then");
    env.method(&b1, "then", &[env.label("b2")]).expect("then");
    env.sync("sync");
    env.drain();
    assert_eq!(env.entries(), s(&["sync", "a1", "b1", "a2", "b2"]));
}

#[test]
fn resolving_with_a_native_promise_takes_two_extra_ticks_like_v8() {
    // Node 26: `p.then(t1).then(t2)` and `new Promise(r => r(Promise.resolve(2))).then(q)` print sync, t1, t2, q.
    let env = Env::new();
    let p = env.resolved(Value::Number(1.0));
    let t1 = env.method(&p, "then", &[env.label("t1")]).expect("then");
    env.method(&t1, "then", &[env.label("t2")]).expect("then");
    let inner = env.resolved(Value::Number(2.0));
    let executor = Value::Object(env.rt.new_native_function(adopt_fn, "executor", 2, vec![inner]));
    let ctor = env.promise();
    let q = env.rt.construct(&ctor, &[executor], &ctor).expect("new Promise");
    env.method(&q, "then", &[env.label("q")]).expect("then");
    env.sync("sync");
    env.drain();
    assert_eq!(env.entries(), s(&["sync", "t1", "t2", "q"]));
}

#[test]
fn a_thenable_is_adopted_by_a_job_that_calls_its_then() {
    // Node 26: `Promise.resolve(th).then(T)` against a four-step chain prints sync, then, a, T, b, c, d.
    let env = Env::new();
    let thenable = {
        let o = env.rt.new_object();
        let then = env.rt.new_native_function(thenable_then, "then", 2, vec![env.log.clone()]);
        env.rt.define_data(o, PropKey::from_str("then"), Value::Object(then), Flags::ALL).expect("define");
        Value::Object(o)
    };
    let p = env.resolved(thenable);
    env.method(&p, "then", &[env.label("T")]).expect("then");
    let a = env.resolved(Value::Undefined);
    let a1 = env.method(&a, "then", &[env.label("a")]).expect("then");
    let b1 = env.method(&a1, "then", &[env.label("b")]).expect("then");
    let c1 = env.method(&b1, "then", &[env.label("c")]).expect("then");
    env.method(&c1, "then", &[env.label("d")]).expect("then");
    env.sync("sync");
    env.drain();
    assert_eq!(env.entries(), s(&["sync", "then", "a", "T", "b", "c", "d"]));
}

#[test]
fn unhandled_rejections_are_listed_in_order_until_a_handler_is_attached() {
    let env = Env::new();
    let first = env.static_call("reject", &[Value::str("first")]).expect("reject");
    let second = env.static_call("reject", &[Value::str("second")]).expect("reject");
    let late = env.static_call("reject", &[Value::str("late")]).expect("reject");
    env.method(&late, "catch", &[env.label("handled")]).expect("catch");
    env.drain();
    let listed = env.rt.take_unhandled_rejections();
    assert_eq!(listed, vec![first.clone(), second.clone()], "the handled promise is not listed");
    assert_eq!(promise::promise_state(&env.rt, &first), Some(PromiseState::Rejected(Value::str("first"))));
    assert!(env.rt.take_unhandled_rejections().is_empty(), "taking empties the list");
    assert_eq!(env.entries(), s(&["handled"]));
}

#[test]
fn a_rejection_handled_before_the_drain_is_never_listed() {
    let env = Env::new();
    let p = env.static_call("reject", &[Value::str("x")]).expect("reject");
    env.method(&p, "then", &[Value::Undefined, env.logging()]).expect("then");
    env.drain();
    assert!(env.rt.take_unhandled_rejections().is_empty());
    assert_eq!(env.entries(), s(&["x"]));
}

#[test]
fn a_rejection_from_a_derived_promise_is_listed() {
    let env = Env::new();
    let p = env.resolved(Value::Number(1.0));
    let q = env.method(&p, "then", &[env.thrower(Value::str("boom"))]).expect("then");
    env.drain();
    assert_eq!(env.rt.take_unhandled_rejections(), vec![q.clone()]);
    assert_eq!(promise::promise_state(&env.rt, &q), Some(PromiseState::Rejected(Value::str("boom"))));
}

#[test]
fn promise_without_new_is_a_type_error_and_the_executor_must_be_callable() {
    let env = Env::new();
    let ctor = env.promise();
    let err = env.rt.call(&ctor, &Value::Undefined, &[Value::Undefined]).expect_err("Promise() throws");
    assert_eq!(error_name(&env, &err), "TypeError");
    let err = env.rt.construct(&ctor, &[Value::Number(1.0)], &ctor).expect_err("non-callable executor");
    assert_eq!(error_name(&env, &err), "TypeError");
}

fn error_name(env: &Env, e: &Throw) -> String {
    match env.rt.get_str(&e.0, "name") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => String::from("<none>"),
    }
}

/// Renders a value for log entries: arrays as [a,b], plain objects as {k=v}, primitives as String(v).
fn show(rt: &Rt, v: &Value) -> String {
    match v {
        Value::Object(o) if rt.is_array(v) => {
            let n = match rt.get_str(v, "length").expect("length") {
                Value::Number(n) => n as u32,
                _ => 0,
            };
            let items: Vec<String> = (0..n).map(|i| show(rt, &rt.get_v(v, &PropKey::Index(i)).expect("index"))).collect();
            let _ = o;
            format!("[{}]", items.join(","))
        }
        Value::Object(o) => {
            let keys = rt.own_keys(*o).expect("keys");
            let parts: Vec<String> = keys
                .iter()
                .map(|k| {
                    let name = match k {
                        PropKey::Str(s) => s.to_rust_string_lossy(),
                        PropKey::Index(i) => i.to_string(),
                        PropKey::Sym(_) => String::from("@@"),
                    };
                    let val = rt.get(*o, k, v).expect("get");
                    format!("{name}={}", show(rt, &val))
                })
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::String(s) => s.to_rust_string_lossy(),
        Value::Number(n) => format!("{n}"),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::from("null"),
        Value::Undefined => String::from("undefined"),
        other => format!("{other:?}"),
    }
}

fn array(env: &Env, items: Vec<Value>) -> Value {
    Value::Object(env.rt.new_array(items))
}

/// Logs the argument of a reaction, rendered with `show`.
fn logging_rendered(env: &Env) -> Value {
    Value::Object(env.rt.new_native_function(render_arg_fn, "render", 1, vec![env.log.clone()]))
}

fn render_arg_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let log = rt.fn_data(c.callee)[0].clone();
    let text = show(rt, &c.arg(0));
    push(rt, &log, Value::str(&text))
}

/// Reads a property of a rejection/fulfilment reason as text.
fn text_of(env: &Env, v: &Value, name: &str) -> String {
    show(&env.rt, &env.rt.get_str(v, name).expect("property"))
}

#[test]
fn all_resolves_with_the_values_in_input_order() {
    // Node 26: all([ok(1), 2, ok('three')]) -> [1,2,"three"].
    let env = Env::new();
    let input = array(&env, vec![env.resolved(Value::Number(1.0)), Value::Number(2.0), env.resolved(Value::str("three"))]);
    let p = env.static_call("all", &[input]).expect("Promise.all");
    env.method(&p, "then", &[logging_rendered(&env)]).expect("then");
    env.drain();
    assert_eq!(env.entries(), s(&["[1,2,three]"]));
}

#[test]
fn all_rejects_with_the_first_rejection_in_time() {
    // Node 26: all([ok(1), bad('boom'), bad('second')]).catch(e => e) -> "boom".
    let env = Env::new();
    let bad_a = env.static_call("reject", &[Value::str("boom")]).expect("reject");
    let bad_b = env.static_call("reject", &[Value::str("second")]).expect("reject");
    let input = array(&env, vec![env.resolved(Value::Number(1.0)), bad_a, bad_b]);
    let p = env.static_call("all", &[input]).expect("Promise.all");
    env.method(&p, "catch", &[env.logging()]).expect("catch");
    env.drain();
    assert_eq!(env.entries(), s(&["boom"]));
}

#[test]
fn all_settled_reports_a_status_record_per_input() {
    // Node 26: allSettled([ok('x'), bad('nope')]) -> [{"status":"fulfilled","value":"x"},{"status":"rejected","reason":"nope"}].
    let env = Env::new();
    let bad = env.static_call("reject", &[Value::str("nope")]).expect("reject");
    let input = array(&env, vec![env.resolved(Value::str("x")), bad]);
    let p = env.static_call("allSettled", &[input]).expect("Promise.allSettled");
    env.method(&p, "then", &[logging_rendered(&env)]).expect("then");
    env.drain();
    assert_eq!(env.entries(), s(&["[{status=fulfilled,value=x},{status=rejected,reason=nope}]"]));
}

#[test]
fn race_settles_with_the_first_settled_input() {
    let env = Env::new();
    let input = array(&env, vec![env.resolved(Value::str("fast")), env.resolved(Value::str("slow"))]);
    let p = env.static_call("race", &[input]).expect("Promise.race");
    env.method(&p, "then", &[env.logging()]).expect("then");
    env.drain();
    assert_eq!(env.entries(), s(&["fast"]));
}

#[test]
fn any_takes_the_first_fulfilment_and_otherwise_an_aggregate_error() {
    let env = Env::new();
    let bad = env.static_call("reject", &[Value::str("a")]).expect("reject");
    let input = array(&env, vec![bad, env.resolved(Value::str("b"))]);
    let p = env.static_call("any", &[input]).expect("Promise.any");
    env.method(&p, "then", &[env.logging()]).expect("then");
    env.drain();
    assert_eq!(env.entries(), s(&["b"]));

    let env = Env::new();
    let a = env.static_call("reject", &[Value::str("a")]).expect("reject");
    let b = env.static_call("reject", &[Value::str("b")]).expect("reject");
    let input = array(&env, vec![a, b]);
    let p = env.static_call("any", &[input]).expect("Promise.any");
    env.drain();
    let Some(PromiseState::Rejected(err)) = promise::promise_state(&env.rt, &p) else { panic!("rejected") };
    assert_eq!(text_of(&env, &err, "name"), "AggregateError");
    assert_eq!(text_of(&env, &err, "message"), "All promises were rejected");
    assert_eq!(text_of(&env, &err, "errors"), "[a,b]");
}

#[test]
fn any_of_nothing_rejects_with_an_empty_aggregate_error_and_all_of_nothing_fulfils() {
    let env = Env::new();
    let p = env.static_call("any", &[array(&env, Vec::new())]).expect("Promise.any");
    let q = env.static_call("all", &[array(&env, Vec::new())]).expect("Promise.all");
    let r = env.static_call("race", &[array(&env, Vec::new())]).expect("Promise.race");
    env.drain();
    let Some(PromiseState::Rejected(err)) = promise::promise_state(&env.rt, &p) else { panic!("rejected") };
    assert_eq!(text_of(&env, &err, "errors"), "[]");
    match promise::promise_state(&env.rt, &q) {
        Some(PromiseState::Fulfilled(v)) => assert_eq!(show(&env.rt, &v), "[]"),
        other => panic!("all of nothing fulfils, got {other:?}"),
    }
    assert_eq!(promise::promise_state(&env.rt, &r), Some(PromiseState::Pending), "race of nothing never settles");
    let _ = env.rt.take_unhandled_rejections();
}

#[test]
fn with_resolvers_hands_out_a_promise_and_its_resolver() {
    let env = Env::new();
    let obj = env.static_call("withResolvers", &[]).expect("withResolvers");
    let p = env.rt.get_str(&obj, "promise").expect("promise");
    let resolve = env.rt.get_str(&obj, "resolve").expect("resolve");
    env.method(&p, "then", &[env.logging()]).expect("then");
    env.rt.call(&resolve, &Value::Undefined, &[Value::str("ok")]).expect("resolve");
    env.sync("sync");
    env.drain();
    assert_eq!(env.entries(), s(&["sync", "ok"]));
}

#[test]
fn finally_passes_the_outcome_through_and_a_throwing_callback_replaces_it() {
    // Node 26: "finally-ok", then "value".
    let env = Env::new();
    let p = env.resolved(Value::str("value"));
    let q = env.method(&p, "finally", &[env.label("finally-ok")]).expect("finally");
    env.method(&q, "then", &[env.logging()]).expect("then");
    env.drain();
    assert_eq!(env.entries(), s(&["finally-ok", "value"]));

    let env = Env::new();
    let rejected = env.static_call("reject", &[Value::str("original")]).expect("reject");
    let q = env.method(&rejected, "finally", &[env.thrower(Value::str("replaced"))]).expect("finally");
    env.method(&q, "catch", &[env.logging()]).expect("catch");
    env.drain();
    assert_eq!(env.entries(), s(&["replaced"]));
}

#[test]
fn finally_with_a_non_callable_passes_through() {
    let env = Env::new();
    let p = env.resolved(Value::Number(3.0));
    let q = env.method(&p, "finally", &[Value::Number(5.0)]).expect("finally");
    env.method(&q, "then", &[env.logging()]).expect("then");
    env.drain();
    assert_eq!(env.entries(), s(&["3"]));
}

#[test]
fn finally_takes_two_extra_ticks_like_v8() {
    // Node 26: a b c F d e.
    let env = Env::new();
    let p = env.resolved(Value::Number(1.0));
    let f = env.method(&p, "finally", &[env.noop()]).expect("finally");
    env.method(&f, "then", &[env.label("F")]).expect("then");
    let mut chain = env.resolved(Value::Undefined);
    for step in ["a", "b", "c", "d", "e"] {
        chain = env.method(&chain, "then", &[env.label(step)]).expect("then");
    }
    env.drain();
    assert_eq!(env.entries(), s(&["a", "b", "c", "F", "d", "e"]));
}

#[test]
fn catch_and_finally_need_an_object_receiver() {
    let env = Env::new();
    let proto = env.rt.get_str(&env.promise(), "prototype").expect("prototype");
    let catch = env.rt.get_str(&proto, "catch").expect("catch");
    let err = env.rt.call(&catch, &Value::Undefined, &[Value::Undefined]).expect_err("catch on undefined");
    assert_eq!(error_name(&env, &err), "TypeError");
    let finally = env.rt.get_str(&proto, "finally").expect("finally");
    let err = env.rt.call(&finally, &Value::Number(1.0), &[Value::Undefined]).expect_err("finally on a number");
    assert_eq!(error_name(&env, &err), "TypeError");
}

#[test]
fn aggregate_error_has_the_v8_shape() {
    // Node 26: own names ["stack","message","cause","errors"], AggregateError.length 2, prototype chain to Error.
    let env = Env::new();
    let global = env.global();
    let ctor = env.rt.get_str(&global, "AggregateError").expect("AggregateError");
    let errors = array(&env, vec![Value::Number(1.0), Value::Number(2.0)]);
    let opts = env.rt.new_object();
    env.rt.define_data(opts, PropKey::from_str("cause"), Value::str("c"), Flags::ALL).expect("cause");
    let err = env.rt.construct(&ctor, &[errors, Value::str("msg"), Value::Object(opts)], &ctor).expect("new AggregateError");
    let Value::Object(e) = err.clone() else { panic!("object") };
    let names: Vec<String> = env
        .rt
        .own_keys(e)
        .expect("keys")
        .iter()
        .map(|k| match k {
            PropKey::Str(s) => s.to_rust_string_lossy(),
            _ => String::from("?"),
        })
        .collect();
    assert_eq!(names, s(&["stack", "message", "cause", "errors"]));
    assert_eq!(text_of(&env, &err, "message"), "msg");
    assert_eq!(text_of(&env, &err, "errors"), "[1,2]");
    assert_eq!(text_of(&env, &err, "cause"), "c");
    let len = env.rt.get_str(&ctor, "length").expect("length");
    assert_eq!(len, Value::Number(2.0));
    let error_ctor = env.rt.get_str(&global, "Error").expect("Error");
    let proto_of_ctor = env.rt.get_prototype_of(ctor.as_object().expect("ctor"));
    assert_eq!(proto_of_ctor, error_ctor.as_object());
}

#[test]
fn promise_surface_matches_v8_lengths_and_names() {
    // Node 26: length 1, name "Promise"; then 2, catch 1, finally 1, statics 1 (withResolvers 0).
    let env = Env::new();
    let ctor = env.promise();
    assert_eq!(env.rt.get_str(&ctor, "length").expect("length"), Value::Number(1.0));
    assert_eq!(text_of(&env, &ctor, "name"), "Promise");
    let proto = env.rt.get_str(&ctor, "prototype").expect("prototype");
    for (obj, name, len) in [
        (&proto, "then", 2.0),
        (&proto, "catch", 1.0),
        (&proto, "finally", 1.0),
        (&ctor, "all", 1.0),
        (&ctor, "allSettled", 1.0),
        (&ctor, "any", 1.0),
        (&ctor, "race", 1.0),
        (&ctor, "resolve", 1.0),
        (&ctor, "reject", 1.0),
        (&ctor, "withResolvers", 0.0),
    ] {
        let f = env.rt.get_str(obj, name).expect(name);
        assert_eq!(env.rt.get_str(&f, "length").expect("length"), Value::Number(len), "{name}");
        assert_eq!(text_of(&env, &f, "name"), name, "{name}");
    }
    assert!(env.rt.get_str(&ctor, "try").expect("try").is_undefined(), "Promise.try is not in this profile");
    let species = env.rt.get_v(&ctor, &PropKey::Sym(env.rt.well_known(vn_value::WellKnown::ToStringTag))).expect("tag");
    assert!(species.is_undefined() || species == Value::str("Promise"), "Promise has no @@species");
}

#[test]
fn combinator_resolution_takes_the_ticks_v8_takes() {
    // Node 26: each observer and a chain of a, b, c, d (three steps for race) print the orders below.
    let cases: [(&str, &str, usize, &[&str]); 4] = [
        ("all", "ALL", 4, &["a", "ALL", "b", "c", "d"]),
        ("allSettled", "S", 4, &["a", "S", "b", "c", "d"]),
        ("race", "R", 3, &["a", "R", "b", "c"]),
        ("any", "Y", 4, &["a", "Y", "b", "c", "d"]),
    ];
    for (name, label, steps, expected) in cases {
        let env = Env::new();
        let inputs = match name {
            "all" | "allSettled" | "race" => array(&env, vec![env.resolved(Value::Number(1.0)), env.resolved(Value::Number(2.0))]),
            _ => {
                let bad = env.static_call("reject", &[Value::Number(1.0)]).expect("reject");
                array(&env, vec![bad, env.resolved(Value::Number(2.0))])
            }
        };
        let observed = env.static_call(name, &[inputs]).expect("combinator");
        env.method(&observed, "then", &[env.label(label)]).expect("then");
        let mut chain = env.resolved(Value::Undefined);
        for step in ["a", "b", "c", "d"].iter().take(steps) {
            chain = env.method(&chain, "then", &[env.label(step)]).expect("then");
        }
        env.drain();
        assert_eq!(env.entries(), s(expected), "{name}");
    }
}
