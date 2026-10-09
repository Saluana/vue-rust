//! WP-B0: the iterator protocol (`vn_value::builtins::iterator`). Expected orders and results for protocol scenarios
//! are copied from V8 (Node v26.5.1) runs of mirrored fixtures; the expectation for each scenario is noted at its test.
//! Rust-level API rules (stepping a done record, the NativeIter wrapper, GC) follow ECMA-262 7.4 as stated in the
//! iterator module's docs. Logs are kept in a host payload that the native functions append to.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use vn_value::builtins::iterator::{
    create_iter_result, get_iterator, iter_close, iter_step, iterate, iterator_prototype, make_iterator_prototype,
    new_iterator_object, NativeIter,
};
use vn_value::{
    payload_ref, CodeFn, Call, ErrorKind, Flags, Intrinsic, NativeObject, ObjRef, Prop, PropKey, Res, Rt, Throw,
    Tracer, Value, WellKnown,
};

// ----- logs and helpers -----

/// Observable events of one scenario, appended to by the native functions of the scenario.
#[derive(Default)]
struct Log(RefCell<Vec<String>>);

impl NativeObject for Log {
    fn trace(&self, _t: &mut Tracer) {}
}

fn new_log(rt: &Rt) -> Value {
    let p: Rc<dyn NativeObject> = Rc::new(Log::default());
    Value::Object(rt.new_native_object(None, p))
}

fn add(rt: &Rt, log: &Value, line: impl Into<String>) {
    let Value::Object(o) = log else { panic!("not a log") };
    let p = rt.native_payload(*o).expect("log payload");
    payload_ref::<Log>(&*p).expect("a Log").0.borrow_mut().push(line.into());
}

fn lines(rt: &Rt, log: &Value) -> Vec<String> {
    let Value::Object(o) = log else { panic!("not a log") };
    let p = rt.native_payload(*o).expect("log payload");
    let v = payload_ref::<Log>(&*p).expect("a Log").0.borrow().clone();
    v
}

fn ok<T>(r: Res<T>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("unexpected JS throw: {e:?}"),
    }
}

fn put(rt: &Rt, o: ObjRef, key: PropKey, v: Value, flags: Flags) {
    ok(rt.define_data(o, key, v, flags));
}

fn obj(v: &Value) -> ObjRef {
    v.as_object().expect("an object")
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_rust_string_lossy(),
        Value::Number(n) => format!("{n}"),
        other => panic!("no text for {other:?}"),
    }
}

fn num(v: &Value) -> f64 {
    match v {
        Value::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

/// The name of the error kind of a thrown value, from its prototype ("other" for anything else).
fn kind(rt: &Rt, e: &Throw) -> &'static str {
    let Value::Object(o) = &e.0 else { return "non-object" };
    let p = rt.get_prototype_of(*o);
    let table = [
        (Intrinsic::TypeErrorPrototype, "TypeError"),
        (Intrinsic::RangeErrorPrototype, "RangeError"),
        (Intrinsic::SyntaxErrorPrototype, "SyntaxError"),
        (Intrinsic::ReferenceErrorPrototype, "ReferenceError"),
    ];
    for (i, name) in table {
        if p == Some(rt.intrinsic(i)) {
            return name;
        }
    }
    "other"
}

/// The kind of the error that `r` must have thrown.
fn err_kind<T: std::fmt::Debug>(rt: &Rt, r: Res<T>) -> &'static str {
    match r {
        Ok(v) => panic!("expected a throw, got {v:?}"),
        Err(e) => kind(rt, &e),
    }
}

fn message(rt: &Rt, e: &Throw) -> String {
    text_of(&ok(rt.get_str(&e.0, "message")))
}

fn err_value(rt: &Rt, kind: ErrorKind, msg: &str) -> Value {
    rt.make_error(kind, msg)
}

// ----- scripted iterators (for the protocol-order tests) -----

/// One scripted result of `next`.
#[derive(Clone)]
enum Step {
    /// `{ value, done }` as plain data properties (value first). `done` may be any value.
    Plain(Value, Value),
    /// `{ value, done }` whose `value` and `done` getters log their names. The protocol reads `done` first.
    Logged(Value, bool),
    /// `{ done: false, value }` where the `value` getter logs and throws a RangeError.
    ValueThrows,
    /// Returned as the result of `next` (a non-object result when it is not an object).
    Raw(Value),
    /// `next` throws this value.
    Throw(Value),
}

/// A `Step::Plain` with a boolean `done`.
fn plain(v: Value, done: bool) -> Step {
    Step::Plain(v, Value::Bool(done))
}

/// State of a scripted iterator: the steps still to run, the number of `next` calls, the log and a prefix for the
/// names logged by `next` (so nested scripts can be told apart).
struct Script {
    steps: RefCell<VecDeque<Step>>,
    calls: Cell<u32>,
    log: Value,
    prefix: &'static str,
}

impl NativeObject for Script {
    fn trace(&self, t: &mut Tracer) {
        t.mark_value(&self.log);
        for s in self.steps.borrow().iter() {
            match s {
                Step::Plain(v, d) => {
                    t.mark_value(v);
                    t.mark_value(d);
                }
                Step::Logged(v, _) | Step::Raw(v) | Step::Throw(v) => t.mark_value(v),
                Step::ValueThrows => {}
            }
        }
    }
}

/// The scripted `next`: data is the script's state object. Logs `<prefix>next` and runs the next step.
fn scripted_next(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let state = rt.fn_data(c.callee)[0].clone();
    let p = rt.native_payload(obj(&state)).expect("script payload");
    let s = payload_ref::<Script>(&*p).expect("a Script");
    s.calls.set(s.calls.get() + 1);
    add(rt, &s.log, format!("{}next", s.prefix));
    let step = s.steps.borrow_mut().pop_front().expect("script exhausted (test bug)");
    match step {
        Step::Plain(value, done) => {
            let o = rt.new_object();
            put(rt, o, PropKey::from_str("value"), value, Flags::ALL);
            put(rt, o, PropKey::from_str("done"), done, Flags::ALL);
            Ok(Value::Object(o))
        }
        Step::Logged(value, done) => {
            let o = rt.new_object();
            let vg = rt.new_native_function(getter_value, "get value", 0, vec![s.log.clone(), value]);
            let dg = rt.new_native_function(getter_done, "get done", 0, vec![s.log.clone(), Value::Bool(done)]);
            ok(rt.define_accessor(o, PropKey::from_str("value"), Some(vg), None, Flags::ALL));
            ok(rt.define_accessor(o, PropKey::from_str("done"), Some(dg), None, Flags::ALL));
            Ok(Value::Object(o))
        }
        Step::ValueThrows => {
            let o = rt.new_object();
            put(rt, o, PropKey::from_str("done"), Value::Bool(false), Flags::ALL);
            let g = rt.new_native_function(getter_throws, "get value", 0, vec![s.log.clone()]);
            ok(rt.define_accessor(o, PropKey::from_str("value"), Some(g), None, Flags::ALL));
            Ok(Value::Object(o))
        }
        Step::Raw(v) => Ok(v),
        Step::Throw(v) => Err(Throw(v)),
    }
}

fn getter_value(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    add(rt, &d[0], "value");
    Ok(d[1].clone())
}

fn getter_done(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    add(rt, &d[0], "done");
    Ok(d[1].clone())
}

fn getter_throws(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    add(rt, &d[0], "value");
    Err(rt.throw_range_error("value getter"))
}

/// An iterator object whose `next` is a scripted function (a data property, read once by GetIterator).
fn script_iterator(rt: &Rt, log: &Value, prefix: &'static str, steps: Vec<Step>) -> Value {
    let s = Script {
        steps: RefCell::new(steps.into_iter().collect()),
        calls: Cell::new(0),
        log: log.clone(),
        prefix,
    };
    let payload: Rc<dyn NativeObject> = Rc::new(s);
    let state = Value::Object(rt.new_native_object(None, payload));
    let next = rt.new_native_function(scripted_next, "next", 0, vec![state]);
    let it = rt.new_object();
    put(rt, it, PropKey::from_str("next"), Value::Object(next), Flags::HIDDEN);
    Value::Object(it)
}

/// Number of `next` calls made on a script iterator.
fn calls_of(rt: &Rt, it: &Value) -> u32 {
    let next = ok(rt.get_str(it, "next"));
    let state = rt.fn_data(obj(&next))[0].clone();
    let p = rt.native_payload(obj(&state)).expect("script payload");
    payload_ref::<Script>(&*p).expect("a Script").calls.get()
}

/// An iterable whose `[Symbol.iterator]` returns `it`. With a log (not `undefined`) the method logs "@@iterator".
fn iterable_of(rt: &Rt, log: &Value, it: &Value) -> Value {
    let m = rt.new_native_function(iter_method, "[Symbol.iterator]", 0, vec![log.clone(), it.clone()]);
    let o = rt.new_object();
    put(rt, o, PropKey::Sym(rt.well_known(WellKnown::Iterator)), Value::Object(m), Flags::HIDDEN);
    Value::Object(o)
}

fn iter_method(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    if !d[0].is_undefined() {
        add(rt, &d[0], "@@iterator");
    }
    Ok(d[1].clone())
}

/// `return` with data [log, label, extra]. Each variant logs the label (and more, where noted).
fn set_return(rt: &Rt, it: &Value, code: CodeFn, log: &Value, label: &str, extra: Value) {
    let f = rt.new_native_function(code, "return", 0, vec![log.clone(), Value::str(label), extra]);
    put(rt, obj(it), PropKey::from_str("return"), Value::Object(f), Flags::HIDDEN);
}

/// Returns `{}`.
fn ret_object(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    add(rt, &d[0], text_of(&d[1]));
    Ok(Value::Object(rt.new_object()))
}

/// Returns 1 (not an object).
fn ret_number(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    add(rt, &d[0], text_of(&d[1]));
    Ok(Value::Number(1.0))
}

/// Throws a RangeError.
fn ret_throws(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    add(rt, &d[0], text_of(&d[1]));
    Err(rt.throw_range_error("return failed"))
}

/// Returns `{}` and logs the argument count and, when `this` is the iterator (data 2), "this=iterator".
fn ret_args(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    add(rt, &d[0], format!("{}{}", text_of(&d[1]), c.args.len()));
    if c.this == &d[2] {
        add(rt, &d[0], "this=iterator");
    }
    Ok(Value::Object(rt.new_object()))
}

/// The `return` getter: logs "getter" and throws a RangeError.
fn return_getter_throws(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    add(rt, &d[0], "getter");
    Err(rt.throw_range_error("return getter"))
}

fn set_return_getter_throws(rt: &Rt, it: &Value, log: &Value) {
    let g = rt.new_native_function(return_getter_throws, "get return", 0, vec![log.clone()]);
    ok(rt.define_accessor(obj(it), PropKey::from_str("return"), Some(g), None, Flags::ALL));
}

fn show(v: &Value) -> String {
    text_of(v)
}

// ----- native iterators (NativeIter) -----

/// A NativeIter over a list of values. It counts its own `next` calls.
struct Items {
    items: Vec<Value>,
    pos: Cell<usize>,
    calls: Cell<u32>,
}

impl Items {
    fn new(items: Vec<Value>) -> Items {
        Items { items, pos: Cell::new(0), calls: Cell::new(0) }
    }
}

impl NativeObject for Items {
    fn trace(&self, t: &mut Tracer) {
        for v in &self.items {
            t.mark_value(v);
        }
    }
}

impl NativeIter for Items {
    fn next(&self, _rt: &Rt) -> Res<Option<Value>> {
        self.calls.set(self.calls.get() + 1);
        let i = self.pos.get();
        if i < self.items.len() {
            self.pos.set(i + 1);
            return Ok(Some(self.items[i].clone()));
        }
        Ok(None)
    }
}

/// A NativeIter that answers from a fixed list (for inner iterators that misbehave or fail).
struct Canned {
    answers: RefCell<VecDeque<Res<Option<Value>>>>,
    calls: Cell<u32>,
}

impl NativeObject for Canned {
    fn trace(&self, t: &mut Tracer) {
        for a in self.answers.borrow().iter() {
            match a {
                Ok(Some(v)) | Err(Throw(v)) => t.mark_value(v),
                Ok(None) => {}
            }
        }
    }
}

impl NativeIter for Canned {
    fn next(&self, _rt: &Rt) -> Res<Option<Value>> {
        self.calls.set(self.calls.get() + 1);
        self.answers.borrow_mut().pop_front().expect("canned answer (test bug)")
    }
}

/// A NativeIter whose first `next` re-enters the same iterator through its `next` method. Later calls answer
/// with the call count. The iterator object is set after creation (`me`).
struct Reenter {
    me: RefCell<Value>,
    calls: Cell<u32>,
}

impl NativeObject for Reenter {
    fn trace(&self, t: &mut Tracer) {
        t.mark_value(&self.me.borrow());
    }
}

impl NativeIter for Reenter {
    fn next(&self, rt: &Rt) -> Res<Option<Value>> {
        let n = self.calls.get() + 1;
        self.calls.set(n);
        if n == 1 {
            let me = self.me.borrow().clone();
            let next = rt.get_str(&me, "next")?;
            let r = rt.call(&next, &me, &[])?;
            let inner = rt.get_str(&r, "value")?;
            return Ok(Some(Value::Number(100.0 + num(&inner))));
        }
        Ok(Some(Value::Number(f64::from(n))))
    }
}

/// A NativeIter whose trace forgets its held object (the deliberate bug of the negative control).
struct Forgetful {
    _held: Value,
}

impl NativeObject for Forgetful {
    fn trace(&self, _t: &mut Tracer) {}
}

impl NativeIter for Forgetful {
    fn next(&self, _rt: &Rt) -> Res<Option<Value>> {
        Ok(None)
    }
}

/// Registers the test family "TestA" (toStringTag "Test Iterator") and returns its prototype.
fn family_a(rt: &Rt) -> ObjRef {
    make_iterator_prototype(rt, "TestA", "Test Iterator")
}

/// An iterable whose `[Symbol.iterator]` returns a fresh `Items` iterator over `values` on every call.
fn items_iterable(rt: &Rt, values: Vec<Value>) -> Value {
    let m = rt.new_native_function(items_method, "[Symbol.iterator]", 0, values);
    let o = rt.new_object();
    put(rt, o, PropKey::Sym(rt.well_known(WellKnown::Iterator)), Value::Object(m), Flags::HIDDEN);
    Value::Object(o)
}

fn items_method(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let values = rt.fn_data(c.callee).to_vec();
    Ok(Value::Object(new_iterator_object(rt, "TestA", Rc::new(Items::new(values)))))
}

// ----- protocol order and reads (V8 oracle: mirror.mjs) -----

/// The trace of a for-of over two logged results and a done one (the scenario of the protocol-order test).
fn order_trace() -> Vec<String> {
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(
        &rt,
        &log,
        "",
        vec![
            Step::Logged(Value::Number(1.0), false),
            Step::Logged(Value::Number(2.0), false),
            Step::Logged(Value::Undefined, true),
        ],
    );
    let iterable = iterable_of(&rt, &log, &it);
    ok(iterate(&rt, &iterable, &mut |x| {
        add(&rt, &log, format!("body {}", show(&x)));
        Ok(true)
    }));
    lines(&rt, &log)
}

#[test]
fn protocol_order_reads_done_before_value_and_skips_value_when_done() {
    // V8: ["@@iterator","next","done","value","body 1","next","done","value","body 2","next","done"]
    assert_eq!(
        order_trace(),
        ["@@iterator", "next", "done", "value", "body 1", "next", "done", "value", "body 2", "next", "done"]
    );
}

#[test]
fn negative_control_a_swapped_read_order_is_reported_as_a_mismatch() {
    // The expectation with `done` and `value` swapped must not match the trace: the comparison above would fail
    // for an implementation that reads `value` first.
    let wrong = ["@@iterator", "next", "value", "done", "body 1", "next", "value", "done", "body 2", "next", "done"];
    assert_ne!(order_trace(), wrong);
}

#[test]
fn next_is_read_once_and_the_record_keeps_that_function() {
    // V8: ["get next","call","v1","call","v2","call"]; the scripted function logs "next" here.
    let rt = Rt::new();
    let log = new_log(&rt);
    let script = script_iterator(
        &rt,
        &log,
        "",
        vec![plain(Value::Number(1.0), false), plain(Value::Number(2.0), false), plain(Value::Number(3.0), true)],
    );
    let next_fn = ok(rt.get_str(&script, "next"));
    let g = rt.new_native_function(get_next, "get next", 0, vec![log.clone(), next_fn]);
    let it = rt.new_object();
    ok(rt.define_accessor(it, PropKey::from_str("next"), Some(g), None, Flags::ALL));
    let iterable = iterable_of(&rt, &Value::Undefined, &Value::Object(it));
    ok(iterate(&rt, &iterable, &mut |x| {
        add(&rt, &log, format!("v{}", show(&x)));
        Ok(true)
    }));
    assert_eq!(lines(&rt, &log), ["get next", "next", "v1", "next", "v2", "next"]);
}

fn get_next(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    add(rt, &d[0], "get next");
    Ok(d[1].clone())
}

#[test]
fn a_result_that_is_not_an_object_is_a_type_error_and_does_not_close() {
    // V8: TypeError, log ["next"] for 1, undefined, null, "x" and true.
    let rt = Rt::new();
    for raw in [Value::Number(1.0), Value::Undefined, Value::Null, Value::str("x"), Value::Bool(true)] {
        let log = new_log(&rt);
        let it = script_iterator(&rt, &log, "", vec![Step::Raw(raw.clone())]);
        set_return(&rt, &it, ret_object, &log, "return", Value::Undefined);
        let iterable = iterable_of(&rt, &Value::Undefined, &it);
        let r = iterate(&rt, &iterable, &mut |_| Ok(true));
        assert_eq!(err_kind(&rt, r), "TypeError", "raw result {raw:?}");
        assert_eq!(lines(&rt, &log), ["next"], "raw result {raw:?}");
    }
}

#[test]
fn done_true_ends_the_loop_without_calling_return() {
    // V8: ["next","body 1","next"], no return.
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(1.0), false), plain(Value::Number(2.0), true)]);
    set_return(&rt, &it, ret_object, &log, "return", Value::Undefined);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    ok(iterate(&rt, &iterable, &mut |x| {
        add(&rt, &log, format!("body {}", show(&x)));
        Ok(true)
    }));
    assert_eq!(lines(&rt, &log), ["next", "body 1", "next"]);
}

#[test]
fn a_throwing_next_marks_the_iterator_done_and_does_not_call_return() {
    // V8: RangeError, log ["next"].
    let rt = Rt::new();
    let log = new_log(&rt);
    let thrown = err_value(&rt, ErrorKind::Range, "n");
    let it = script_iterator(&rt, &log, "", vec![Step::Throw(thrown)]);
    set_return(&rt, &it, ret_object, &log, "return", Value::Undefined);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let r = iterate(&rt, &iterable, &mut |_| Ok(true));
    assert_eq!(err_kind(&rt, r), "RangeError");
    assert_eq!(lines(&rt, &log), ["next"]);
}

#[test]
fn a_throwing_value_getter_does_not_call_return() {
    // V8: RangeError, log ["next","value"].
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![Step::ValueThrows]);
    set_return(&rt, &it, ret_object, &log, "return", Value::Undefined);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let r = iterate(&rt, &iterable, &mut |_| Ok(true));
    assert_eq!(err_kind(&rt, r), "RangeError");
    assert_eq!(lines(&rt, &log), ["next", "value"]);
}

#[test]
fn the_value_is_not_read_when_the_result_is_done() {
    // V8: log ["done"] for the step (the loop reads only done).
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![Step::Logged(Value::Number(1.0), true)]);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    ok(iterate(&rt, &iterable, &mut |_| Ok(true)));
    assert_eq!(lines(&rt, &log), ["next", "done"]);
}

#[test]
fn done_is_coerced_with_to_boolean() {
    // V8: ["a","b"] in both sequences; the third result (done truthy) is not yielded.
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(
        &rt,
        &log,
        "",
        vec![
            Step::Plain(Value::str("a"), Value::Number(0.0)),
            Step::Plain(Value::str("b"), Value::str("")),
            Step::Plain(Value::str("c"), Value::str("x")),
        ],
    );
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let mut got = Vec::new();
    ok(iterate(&rt, &iterable, &mut |x| {
        got.push(show(&x));
        Ok(true)
    }));
    assert_eq!(got, ["a", "b"]);

    let arr = Value::Object(rt.new_array(vec![]));
    let it = script_iterator(
        &rt,
        &log,
        "",
        vec![
            Step::Plain(Value::str("a"), Value::Undefined),
            Step::Plain(Value::str("b"), Value::Null),
            Step::Plain(Value::str("c"), arr),
        ],
    );
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let mut got = Vec::new();
    ok(iterate(&rt, &iterable, &mut |x| {
        got.push(show(&x));
        Ok(true)
    }));
    assert_eq!(got, ["a", "b"]);
}

// ----- early exit, return and errors (V8 oracle: mirror.mjs) -----

#[test]
fn an_early_exit_calls_return_once() {
    // V8: ["next","body 0","return"].
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(0.0), false), plain(Value::Number(1.0), false)]);
    set_return(&rt, &it, ret_object, &log, "return", Value::Undefined);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    ok(iterate(&rt, &iterable, &mut |x| {
        add(&rt, &log, format!("body {}", show(&x)));
        Ok(false)
    }));
    assert_eq!(lines(&rt, &log), ["next", "body 0", "return"]);
    assert_eq!(calls_of(&rt, &it), 1);
}

#[test]
fn return_gets_no_arguments_and_the_iterator_as_this() {
    // V8: ["next","body 0","return0","this=iterator"].
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(0.0), false), plain(Value::Number(1.0), false)]);
    set_return(&rt, &it, ret_args, &log, "return", it.clone());
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    ok(iterate(&rt, &iterable, &mut |x| {
        add(&rt, &log, format!("body {}", show(&x)));
        Ok(false)
    }));
    assert_eq!(lines(&rt, &log), ["next", "body 0", "return0", "this=iterator"]);
}

#[test]
fn a_non_object_return_result_is_a_type_error_on_a_normal_completion() {
    // V8: TypeError, log ["next","body 0","return"].
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(0.0), false), plain(Value::Number(1.0), false)]);
    set_return(&rt, &it, ret_number, &log, "return", Value::Undefined);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let r = iterate(&rt, &iterable, &mut |x| {
        add(&rt, &log, format!("body {}", show(&x)));
        Ok(false)
    });
    assert_eq!(err_kind(&rt, r), "TypeError");
    assert_eq!(lines(&rt, &log), ["next", "body 0", "return"]);
}

#[test]
fn a_throwing_return_propagates_on_a_normal_completion() {
    // V8: RangeError, log ["next","body 0","return"].
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(0.0), false), plain(Value::Number(1.0), false)]);
    set_return(&rt, &it, ret_throws, &log, "return", Value::Undefined);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let r = iterate(&rt, &iterable, &mut |x| {
        add(&rt, &log, format!("body {}", show(&x)));
        Ok(false)
    });
    assert_eq!(err_kind(&rt, r), "RangeError");
    assert_eq!(lines(&rt, &log), ["next", "body 0", "return"]);
}

#[test]
fn a_throwing_return_is_swallowed_on_a_throw_completion() {
    // V8: SyntaxError (the body's), log ["next","body 0","return"].
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(0.0), false), plain(Value::Number(1.0), false)]);
    set_return(&rt, &it, ret_throws, &log, "return", Value::Undefined);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let r = iterate(&rt, &iterable, &mut |x| {
        add(&rt, &log, format!("body {}", show(&x)));
        Err(Throw(err_value(&rt, ErrorKind::Syntax, "body")))
    });
    assert_eq!(err_kind(&rt, r), "SyntaxError");
    assert_eq!(lines(&rt, &log), ["next", "body 0", "return"]);
}

#[test]
fn a_non_object_return_result_is_not_checked_on_a_throw_completion() {
    // V8: SyntaxError (the body's), log ["next","body 0","return"].
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(0.0), false), plain(Value::Number(1.0), false)]);
    set_return(&rt, &it, ret_number, &log, "return", Value::Undefined);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let r = iterate(&rt, &iterable, &mut |x| {
        add(&rt, &log, format!("body {}", show(&x)));
        Err(Throw(err_value(&rt, ErrorKind::Syntax, "body")))
    });
    assert_eq!(err_kind(&rt, r), "SyntaxError");
    assert_eq!(lines(&rt, &log), ["next", "body 0", "return"]);
}

#[test]
fn a_null_or_absent_return_means_no_return_call() {
    // V8: "no error", log ["next","body 0"] for both null and an absent return.
    let rt = Rt::new();
    for null_return in [true, false] {
        let log = new_log(&rt);
        let steps = vec![plain(Value::Number(0.0), false), plain(Value::Number(1.0), false)];
        let it = script_iterator(&rt, &log, "", steps);
        if null_return {
            put(&rt, obj(&it), PropKey::from_str("return"), Value::Null, Flags::HIDDEN);
        }
        let iterable = iterable_of(&rt, &Value::Undefined, &it);
        ok(iterate(&rt, &iterable, &mut |x| {
            add(&rt, &log, format!("body {}", show(&x)));
            Ok(false)
        }));
        assert_eq!(lines(&rt, &log), ["next", "body 0"], "null return: {null_return}");
    }
}

#[test]
fn a_non_callable_return_is_a_type_error_on_a_normal_completion() {
    // V8: TypeError, log ["next","body 0"].
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(0.0), false), plain(Value::Number(1.0), false)]);
    put(&rt, obj(&it), PropKey::from_str("return"), Value::Number(1.0), Flags::HIDDEN);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let r = iterate(&rt, &iterable, &mut |x| {
        add(&rt, &log, format!("body {}", show(&x)));
        Ok(false)
    });
    assert_eq!(err_kind(&rt, r), "TypeError");
    assert_eq!(lines(&rt, &log), ["next", "body 0"]);
}

#[test]
fn a_non_callable_return_is_swallowed_on_a_throw_completion() {
    // V8: SyntaxError (the body's), log ["next","body 0"].
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(0.0), false), plain(Value::Number(1.0), false)]);
    put(&rt, obj(&it), PropKey::from_str("return"), Value::Number(1.0), Flags::HIDDEN);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let r = iterate(&rt, &iterable, &mut |x| {
        add(&rt, &log, format!("body {}", show(&x)));
        Err(Throw(err_value(&rt, ErrorKind::Syntax, "body")))
    });
    assert_eq!(err_kind(&rt, r), "SyntaxError");
    assert_eq!(lines(&rt, &log), ["next", "body 0"]);
}

#[test]
fn a_throwing_return_getter_is_swallowed_on_a_throw_completion() {
    // V8: SyntaxError (the body's), log ["next","body 0","getter"]: the getter runs even on a throw completion.
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(0.0), false), plain(Value::Number(1.0), false)]);
    set_return_getter_throws(&rt, &it, &log);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let r = iterate(&rt, &iterable, &mut |x| {
        add(&rt, &log, format!("body {}", show(&x)));
        Err(Throw(err_value(&rt, ErrorKind::Syntax, "body")))
    });
    assert_eq!(err_kind(&rt, r), "SyntaxError");
    assert_eq!(lines(&rt, &log), ["next", "body 0", "getter"]);
}

#[test]
fn a_throwing_return_getter_propagates_on_a_normal_completion() {
    // V8: RangeError, log ["next","body 0","getter"].
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(0.0), false), plain(Value::Number(1.0), false)]);
    set_return_getter_throws(&rt, &it, &log);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let r = iterate(&rt, &iterable, &mut |x| {
        add(&rt, &log, format!("body {}", show(&x)));
        Ok(false)
    });
    assert_eq!(err_kind(&rt, r), "RangeError");
    assert_eq!(lines(&rt, &log), ["next", "body 0", "getter"]);
}

#[test]
fn a_body_error_closes_the_iterator_and_is_the_error_that_propagates() {
    // V8: SyntaxError, log ["next","body 0","return"].
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(0.0), false), plain(Value::Number(1.0), false)]);
    set_return(&rt, &it, ret_object, &log, "return", Value::Undefined);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let r = iterate(&rt, &iterable, &mut |x| {
        add(&rt, &log, format!("body {}", show(&x)));
        Err(Throw(err_value(&rt, ErrorKind::Syntax, "body")))
    });
    assert_eq!(err_kind(&rt, r), "SyntaxError");
    assert_eq!(lines(&rt, &log), ["next", "body 0", "return"]);
}

#[test]
fn nested_loops_close_only_the_inner_iterator_on_break() {
    // V8: ["O.next","I.next","b0","I.return","O.next","I.next","b0","I.return","O.next"].
    let rt = Rt::new();
    let log = new_log(&rt);
    let outer = script_iterator(
        &rt,
        &log,
        "O.",
        vec![plain(Value::Number(0.0), false), plain(Value::Number(1.0), false), plain(Value::Undefined, true)],
    );
    let outer_iterable = iterable_of(&rt, &Value::Undefined, &outer);
    ok(iterate(&rt, &outer_iterable, &mut |_a| {
        let inner = script_iterator(
            &rt,
            &log,
            "I.",
            vec![plain(Value::Number(0.0), false), plain(Value::Number(1.0), false), plain(Value::Undefined, true)],
        );
        set_return(&rt, &inner, ret_object, &log, "I.return", Value::Undefined);
        let inner_iterable = iterable_of(&rt, &Value::Undefined, &inner);
        ok(iterate(&rt, &inner_iterable, &mut |b| {
            add(&rt, &log, format!("b{}", show(&b)));
            Ok(false)
        }));
        Ok(true)
    }));
    assert_eq!(
        lines(&rt, &log),
        ["O.next", "I.next", "b0", "I.return", "O.next", "I.next", "b0", "I.return", "O.next"]
    );
}

// ----- iter_step and iter_close as direct API -----

#[test]
fn iter_step_yields_values_then_none_and_a_done_record_never_calls_next_again() {
    // ECMA-262 7.4.8: done is read from each result; a record already done is not stepped again.
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(
        &rt,
        &log,
        "",
        vec![plain(Value::Number(1.0), false), plain(Value::Number(2.0), false), plain(Value::Undefined, true)],
    );
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let mut rec = ok(get_iterator(&rt, &iterable));
    assert!(!rec.done);
    assert_eq!(ok(iter_step(&rt, &mut rec)), Some(Value::Number(1.0)));
    assert_eq!(ok(iter_step(&rt, &mut rec)), Some(Value::Number(2.0)));
    assert_eq!(ok(iter_step(&rt, &mut rec)), None);
    assert!(rec.done);
    assert_eq!(ok(iter_step(&rt, &mut rec)), None);
    assert_eq!(calls_of(&rt, &it), 3);
}

#[test]
fn a_throw_in_next_marks_the_record_done_so_it_is_not_stepped_again() {
    let rt = Rt::new();
    let log = new_log(&rt);
    let thrown = err_value(&rt, ErrorKind::Range, "n");
    let it = script_iterator(&rt, &log, "", vec![Step::Throw(thrown), plain(Value::Number(1.0), false)]);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let mut rec = ok(get_iterator(&rt, &iterable));
    let e = iter_step(&rt, &mut rec).unwrap_err();
    assert_eq!(kind(&rt, &e), "RangeError");
    assert!(rec.done);
    assert_eq!(ok(iter_step(&rt, &mut rec)), None);
    assert_eq!(calls_of(&rt, &it), 1);
}

#[test]
fn iter_close_on_a_throw_completion_swallows_return_errors() {
    // ECMA-262 7.4.11: `return` is called; its error is ignored because the completion is a throw.
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(1.0), false)]);
    set_return(&rt, &it, ret_throws, &log, "return", Value::Undefined);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let rec = ok(get_iterator(&rt, &iterable));
    ok(iter_close(&rt, &rec, true));
    assert_eq!(lines(&rt, &log), ["return"]);
}

#[test]
fn iter_close_on_a_normal_completion_propagates_return_errors() {
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(1.0), false)]);
    set_return(&rt, &it, ret_throws, &log, "return", Value::Undefined);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let rec = ok(get_iterator(&rt, &iterable));
    let r = iter_close(&rt, &rec, false);
    assert_eq!(err_kind(&rt, r), "RangeError");
    assert_eq!(lines(&rt, &log), ["return"]);
}

#[test]
fn iter_close_without_a_return_method_does_nothing() {
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(1.0), false)]);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let rec = ok(get_iterator(&rt, &iterable));
    ok(iter_close(&rt, &rec, false));
    assert_eq!(lines(&rt, &log), Vec::<String>::new());
}

#[test]
fn iter_close_on_a_normal_completion_accepts_an_object_result() {
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(1.0), false)]);
    set_return(&rt, &it, ret_object, &log, "return", Value::Undefined);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let rec = ok(get_iterator(&rt, &iterable));
    ok(iter_close(&rt, &rec, false));
    assert_eq!(lines(&rt, &log), ["return"]);
}

#[test]
fn iter_close_on_a_normal_completion_rejects_a_non_object_result() {
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(1.0), false)]);
    set_return(&rt, &it, ret_number, &log, "return", Value::Undefined);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let rec = ok(get_iterator(&rt, &iterable));
    let r = iter_close(&rt, &rec, false);
    assert_eq!(err_kind(&rt, r), "TypeError");
    assert_eq!(lines(&rt, &log), ["return"]);
}

#[test]
fn iter_close_on_a_throw_completion_ignores_a_non_object_result() {
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(1.0), false)]);
    set_return(&rt, &it, ret_number, &log, "return", Value::Undefined);
    let iterable = iterable_of(&rt, &Value::Undefined, &it);
    let rec = ok(get_iterator(&rt, &iterable));
    ok(iter_close(&rt, &rec, true));
    assert_eq!(lines(&rt, &log), ["return"]);
}

// ----- get_iterator (V8 oracle: mirror.mjs) -----

#[test]
fn get_iterator_reads_symbol_iterator_once_and_calls_it_with_no_arguments_and_the_iterable_as_this() {
    // V8: ["get","call0","this=iterable","next"]: one read of @@iterator, one call with no argument, and `this` is
    // the iterable.
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = script_iterator(&rt, &log, "", vec![plain(Value::Number(1.0), true)]);
    let iterable = rt.new_object();
    let data = vec![log.clone(), it.clone(), Value::Object(iterable)];
    let m = rt.new_native_function(counted_iter_method, "[Symbol.iterator]", 0, data);
    let gdata = vec![log.clone(), Value::Object(m)];
    let g = rt.new_native_function(counted_iter_getter, "get [Symbol.iterator]", 0, gdata);
    ok(rt.define_accessor(iterable, PropKey::Sym(rt.well_known(WellKnown::Iterator)), Some(g), None, Flags::ALL));
    ok(iterate(&rt, &Value::Object(iterable), &mut |_| Ok(true)));
    assert_eq!(lines(&rt, &log), ["get", "call0", "this=iterable", "next"]);
}

fn counted_iter_getter(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    add(rt, &d[0], "get");
    Ok(d[1].clone())
}

fn counted_iter_method(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    add(rt, &d[0], format!("call{}", c.args.len()));
    if c.this == &d[2] {
        add(rt, &d[0], "this=iterable");
    }
    Ok(d[1].clone())
}

#[test]
fn a_non_object_from_symbol_iterator_is_a_type_error() {
    // V8: TypeError for 1 and for undefined.
    let rt = Rt::new();
    for v in [Value::Number(1.0), Value::Undefined] {
        let iterable = iterable_of(&rt, &Value::Undefined, &v);
        let r = get_iterator(&rt, &iterable);
        assert_eq!(err_kind(&rt, r), "TypeError", "result {v:?}");
    }
}

#[test]
fn values_that_are_not_iterable_are_type_errors_with_the_is_not_iterable_message() {
    // V8: TypeError for each of these (undefined, null, {}, a non-callable @@iterator, 5, true).
    let rt = Rt::new();
    let bad_iterator = rt.new_object();
    put(&rt, bad_iterator, PropKey::Sym(rt.well_known(WellKnown::Iterator)), Value::Number(1.0), Flags::ALL);
    for v in [
        Value::Undefined,
        Value::Null,
        Value::Object(rt.new_object()),
        Value::Object(bad_iterator),
        Value::Number(5.0),
        Value::Bool(true),
    ] {
        let e = get_iterator(&rt, &v).unwrap_err();
        assert_eq!(kind(&rt, &e), "TypeError", "value {v:?}");
        assert!(message(&rt, &e).ends_with("is not iterable"), "message: {}", message(&rt, &e));
    }
}

#[test]
fn a_missing_next_fails_at_the_first_step_without_calling_return() {
    // V8: TypeError, log [].
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = rt.new_object();
    let f = rt.new_native_function(ret_object, "return", 0, vec![log.clone(), Value::str("return"), Value::Undefined]);
    put(&rt, it, PropKey::from_str("return"), Value::Object(f), Flags::HIDDEN);
    let iterable = iterable_of(&rt, &Value::Undefined, &Value::Object(it));
    let r = iterate(&rt, &iterable, &mut |_| Ok(true));
    assert_eq!(err_kind(&rt, r), "TypeError");
    assert_eq!(lines(&rt, &log), Vec::<String>::new());
}

#[test]
fn a_non_callable_next_is_stored_and_only_fails_when_stepped() {
    // V8: TypeError, log []. get_iterator succeeds; the first step fails.
    let rt = Rt::new();
    let log = new_log(&rt);
    let it = rt.new_object();
    put(&rt, it, PropKey::from_str("next"), Value::Number(1.0), Flags::HIDDEN);
    let f = rt.new_native_function(ret_object, "return", 0, vec![log.clone(), Value::str("return"), Value::Undefined]);
    put(&rt, it, PropKey::from_str("return"), Value::Object(f), Flags::HIDDEN);
    let iterable = iterable_of(&rt, &Value::Undefined, &Value::Object(it));
    let mut rec = ok(get_iterator(&rt, &iterable));
    let e = iter_step(&rt, &mut rec).unwrap_err();
    assert_eq!(kind(&rt, &e), "TypeError");
    assert!(rec.done);
    assert_eq!(lines(&rt, &log), Vec::<String>::new());
}

// ----- %IteratorPrototype% and family prototypes -----

#[test]
fn the_iterator_prototype_method_returns_its_receiver_unchanged() {
    // V8: a primitive receiver comes back as is ("number", true); an object comes back identical.
    let rt = Rt::new();
    let proto = iterator_prototype(&rt);
    let m = ok(rt.get_v(&Value::Object(proto), &PropKey::Sym(rt.well_known(WellKnown::Iterator))));
    assert_eq!(ok(rt.call(&m, &Value::Number(5.0), &[])), Value::Number(5.0));
    let o = Value::Object(rt.new_object());
    assert_eq!(ok(rt.call(&m, &o, &[])), o);
}

#[test]
fn the_iterator_prototype_method_has_the_javascript_attributes() {
    // V8: writable true, enumerable false, configurable true; name "[Symbol.iterator]", length 0; [[Prototype]] is
    // Object.prototype.
    let rt = Rt::new();
    let proto = iterator_prototype(&rt);
    let key = PropKey::Sym(rt.well_known(WellKnown::Iterator));
    let Some(Prop::Data { value, flags }) = ok(rt.get_own_property(proto, &key)) else {
        panic!("@@iterator must be a data property")
    };
    assert_eq!(flags, Flags::HIDDEN);
    assert_eq!(text_of(&ok(rt.get_str(&value, "name"))), "[Symbol.iterator]");
    assert_eq!(ok(rt.get_str(&value, "length")), Value::Number(0.0));
    assert_eq!(rt.get_prototype_of(proto), Some(rt.intrinsic(Intrinsic::ObjectPrototype)));
}

#[test]
fn the_iterator_prototype_is_registered_under_its_name() {
    let rt = Rt::new();
    assert_eq!(rt.builtin("IteratorPrototype"), Some(iterator_prototype(&rt)));
}

#[test]
fn a_family_prototype_has_the_array_iterator_attributes() {
    // V8 (Array Iterator): next is writable, non-enumerable, configurable, length 0, name "next"; @@toStringTag is
    // not writable, not enumerable, configurable, and inherits from %IteratorPrototype%.
    let rt = Rt::new();
    let fam = family_a(&rt);
    let Some(Prop::Data { value, flags }) = ok(rt.get_own_property(fam, &PropKey::from_str("next"))) else {
        panic!("next must be a data property")
    };
    assert_eq!(flags, Flags::HIDDEN);
    assert_eq!(text_of(&ok(rt.get_str(&value, "name"))), "next");
    assert_eq!(ok(rt.get_str(&value, "length")), Value::Number(0.0));
    let Some(Prop::Data { value: tag, flags: tag_flags }) =
        ok(rt.get_own_property(fam, &PropKey::Sym(rt.well_known(WellKnown::ToStringTag))))
    else {
        panic!("@@toStringTag must be a data property")
    };
    assert_eq!(text_of(&tag), "Test Iterator");
    assert_eq!(tag_flags, Flags::CONFIG);
    assert_eq!(rt.get_prototype_of(fam), Some(iterator_prototype(&rt)));
}

#[test]
fn an_iterator_object_prints_its_tag_through_object_prototype_to_string() {
    // V8: "[object Array Iterator]" for an array iterator; the family's tag is used the same way.
    let rt = Rt::new();
    family_a(&rt);
    let it = new_iterator_object(&rt, "TestA", Rc::new(Items::new(vec![])));
    assert_eq!(text_of(&Value::String(ok(rt.to_string(&Value::Object(it))))), "[object Test Iterator]");
}

#[test]
fn a_new_iterator_object_uses_the_prototype_registered_under_its_family() {
    let rt = Rt::new();
    family_a(&rt);
    let it = new_iterator_object(&rt, "TestA", Rc::new(Items::new(vec![])));
    assert_eq!(rt.get_prototype_of(it), rt.builtin("TestA"));
}

#[test]
#[should_panic(expected = "no iterator prototype registered")]
fn a_new_iterator_object_for_an_unknown_family_is_a_framework_bug() {
    let rt = Rt::new();
    let _ = new_iterator_object(&rt, "NoSuchFamily", Rc::new(Items::new(vec![])));
}

// ----- NativeIter behaviour (the wrapper's guarantees) -----

#[test]
fn a_native_iterator_drives_iterate_and_its_own_iterator_method_returns_it() {
    let rt = Rt::new();
    family_a(&rt);
    let items = vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)];
    let it = new_iterator_object(&rt, "TestA", Rc::new(Items::new(items)));
    let rec = ok(get_iterator(&rt, &Value::Object(it)));
    assert_eq!(rec.iterator, Value::Object(it));
    let sum = Cell::new(0.0);
    ok(iterate(&rt, &Value::Object(it), &mut |x| {
        sum.set(sum.get() + num(&x));
        Ok(true)
    }));
    assert_eq!(sum.get(), 6.0);
}

#[test]
fn a_native_iterator_stays_exhausted_without_calling_its_state_again() {
    // The state is asked for three values (two items and the end); later steps never reach it.
    let rt = Rt::new();
    family_a(&rt);
    let state = Rc::new(Items::new(vec![Value::Number(1.0), Value::Number(2.0)]));
    let it = new_iterator_object(&rt, "TestA", state.clone());
    let next = ok(rt.get_str(&Value::Object(it), "next"));
    let mut seen = Vec::new();
    for _ in 0..4 {
        let r = ok(rt.call(&next, &Value::Object(it), &[]));
        let done = ok(rt.get_str(&r, "done")).to_boolean();
        let v = ok(rt.get_str(&r, "value"));
        seen.push((done, if done { None } else { Some(num(&v)) }));
    }
    assert_eq!(seen, [(false, Some(1.0)), (false, Some(2.0)), (true, None), (true, None)]);
    assert_eq!(state.calls.get(), 3);
}

#[test]
fn a_native_iterator_that_returns_none_once_stays_done() {
    // The state answers None and then a value; the wrapper never asks it again after None.
    let rt = Rt::new();
    family_a(&rt);
    let state = Rc::new(Canned {
        answers: RefCell::new(VecDeque::from(vec![Ok(None), Ok(Some(Value::Number(9.0)))])),
        calls: Cell::new(0),
    });
    let it = new_iterator_object(&rt, "TestA", state.clone());
    let next = ok(rt.get_str(&Value::Object(it), "next"));
    for _ in 0..2 {
        let r = ok(rt.call(&next, &Value::Object(it), &[]));
        assert!(ok(rt.get_str(&r, "done")).to_boolean());
        assert_eq!(ok(rt.get_str(&r, "value")), Value::Undefined);
    }
    assert_eq!(state.calls.get(), 1);
}

#[test]
fn a_native_iterator_error_propagates_once_and_finishes_the_iterator() {
    // The state throws once and would then yield 9; the wrapper reports done and never asks again.
    let rt = Rt::new();
    family_a(&rt);
    let thrown = err_value(&rt, ErrorKind::Range, "n");
    let state = Rc::new(Canned {
        answers: RefCell::new(VecDeque::from(vec![Err(Throw(thrown)), Ok(Some(Value::Number(9.0)))])),
        calls: Cell::new(0),
    });
    let it = new_iterator_object(&rt, "TestA", state.clone());
    let next = ok(rt.get_str(&Value::Object(it), "next"));
    let r = rt.call(&next, &Value::Object(it), &[]);
    assert_eq!(err_kind(&rt, r), "RangeError");
    let r2 = ok(rt.call(&next, &Value::Object(it), &[]));
    assert!(ok(rt.get_str(&r2, "done")).to_boolean());
    assert_eq!(state.calls.get(), 1);
}

#[test]
fn native_next_rejects_receivers_that_are_not_iterators_of_its_family() {
    // V8: TypeError for {}, 5, undefined and an iterator of another family (here "TestB").
    let rt = Rt::new();
    let fam_a = family_a(&rt);
    make_iterator_prototype(&rt, "TestB", "Test Iterator B");
    let next_a = ok(rt.get_str(&Value::Object(fam_a), "next"));
    let b_iter = new_iterator_object(&rt, "TestB", Rc::new(Items::new(vec![])));
    let receivers = [
        Value::Object(rt.new_object()),
        Value::Number(5.0),
        Value::Undefined,
        Value::Object(b_iter),
    ];
    for recv in receivers {
        let r = rt.call(&next_a, &recv, &[]);
        assert_eq!(err_kind(&rt, r), "TypeError", "receiver {recv:?}");
    }
    // The positive control: the family's own iterator is accepted.
    let own = new_iterator_object(&rt, "TestA", Rc::new(Items::new(vec![Value::Number(7.0)])));
    let r = ok(rt.call(&next_a, &Value::Object(own), &[]));
    assert_eq!(ok(rt.get_str(&r, "value")), Value::Number(7.0));
}

#[test]
fn a_re_entrant_next_on_a_native_iterator_sees_consistent_state() {
    // V8 (equivalent JS: a first call that calls this.next() once, then returns 100 + the inner value):
    // [102, 3, 4] with 4 calls in total.
    let rt = Rt::new();
    family_a(&rt);
    let state = Rc::new(Reenter { me: RefCell::new(Value::Undefined), calls: Cell::new(0) });
    let it = new_iterator_object(&rt, "TestA", state.clone());
    *state.me.borrow_mut() = Value::Object(it);
    let next = ok(rt.get_str(&Value::Object(it), "next"));
    let mut got = Vec::new();
    for _ in 0..3 {
        let r = ok(rt.call(&next, &Value::Object(it), &[]));
        got.push(num(&ok(rt.get_str(&r, "value"))));
    }
    assert_eq!(got, [102.0, 3.0, 4.0]);
    assert_eq!(state.calls.get(), 4);
}

#[test]
fn nested_iteration_over_the_same_iterable_uses_independent_iterators() {
    // V8: [[1,1],[1,2],[2,1],[2,2]].
    let rt = Rt::new();
    family_a(&rt);
    let iterable = items_iterable(&rt, vec![Value::Number(1.0), Value::Number(2.0)]);
    let pairs = RefCell::new(Vec::new());
    ok(iterate(&rt, &iterable, &mut |a| {
        ok(iterate(&rt, &iterable, &mut |b| {
            pairs.borrow_mut().push((num(&a), num(&b)));
            Ok(true)
        }));
        Ok(true)
    }));
    assert_eq!(pairs.into_inner(), [(1.0, 1.0), (1.0, 2.0), (2.0, 1.0), (2.0, 2.0)]);
}

// ----- create_iter_result -----

#[test]
fn create_iter_result_has_value_then_done_as_plain_data_properties() {
    // V8: Object.keys(result) is ["value","done"], all three attributes true, prototype Object.prototype.
    let rt = Rt::new();
    let r = create_iter_result(&rt, Value::Number(7.0), false);
    let o = obj(&r);
    let keys = ok(rt.own_keys(o));
    assert_eq!(keys, [PropKey::from_str("value"), PropKey::from_str("done")]);
    for k in &keys {
        let Some(Prop::Data { flags, .. }) = ok(rt.get_own_property(o, k)) else { panic!("data property expected") };
        assert_eq!(flags, Flags::ALL, "key {k:?}");
    }
    assert_eq!(rt.get_prototype_of(o), Some(rt.intrinsic(Intrinsic::ObjectPrototype)));
    assert_eq!(ok(rt.get_str(&r, "value")), Value::Number(7.0));
    assert_eq!(ok(rt.get_str(&r, "done")), Value::Bool(false));
    let undef = create_iter_result(&rt, Value::Undefined, true);
    assert!(ok(rt.has_own_property(obj(&undef), &PropKey::from_str("value"))));
}

// ----- garbage collection -----

#[test]
fn objects_held_by_a_native_iterator_survive_collection_between_steps() {
    let rt = Rt::new();
    family_a(&rt);
    let objs: Vec<Value> = (0..3)
        .map(|i| {
            let o = rt.new_object();
            put(&rt, o, PropKey::from_str("k"), Value::Number(f64::from(i)), Flags::ALL);
            Value::Object(o)
        })
        .collect();
    let it = new_iterator_object(&rt, "TestA", Rc::new(Items::new(objs)));
    let root = rt.root(&Value::Object(it));
    let next = ok(rt.get_str(&Value::Object(it), "next"));
    for i in 0..3 {
        rt.collect();
        let r = ok(rt.call(&next, &Value::Object(it), &[]));
        let v = ok(rt.get_str(&r, "value"));
        assert!(rt.is_live(obj(&v)), "item {i} must stay alive");
        assert_eq!(ok(rt.get_str(&v, "k")), Value::Number(f64::from(i)));
    }
    rt.unroot(root);
}

#[test]
fn negative_control_a_payload_that_forgets_to_trace_loses_its_objects() {
    // The iterator holds `held` but does not trace it. The collector frees `held`, so the survival check reports it.
    let rt = Rt::new();
    family_a(&rt);
    let held = rt.new_object();
    let it = new_iterator_object(&rt, "TestA", Rc::new(Forgetful { _held: Value::Object(held) }));
    let root = rt.root(&Value::Object(it));
    rt.collect();
    assert!(rt.is_live(it), "the rooted iterator stays alive");
    assert!(!rt.is_live(held), "a payload that forgets to trace must lose its object");
    rt.unroot(root);
}

#[test]
fn an_unrooted_iterator_and_what_it_holds_are_collected_together() {
    let rt = Rt::new();
    family_a(&rt);
    let held = rt.new_object();
    let it = new_iterator_object(&rt, "TestA", Rc::new(Items::new(vec![Value::Object(held)])));
    let root = rt.root(&Value::Object(it));
    rt.collect();
    assert!(rt.is_live(it) && rt.is_live(held));
    rt.unroot(root);
    rt.collect();
    assert!(!rt.is_live(it), "an unrooted iterator is collectable");
    assert!(!rt.is_live(held), "its traced items die with it");
}
