//! Builtin family: Promise (ECMA-262 27.2) with V8's job ordering (WP-B5). Reactions, thenable adoption and combinator
//! steps are kernel jobs (`Rt::enqueue_job`); nothing runs synchronously except executors and the user code the spec
//! calls synchronously. Rejection tracking follows HostPromiseRejectionTracker: `Rt::take_unhandled_rejections` lists
//! rejected promises that have no handler. The async-function runtime is in `async_rt`.
//!
//! Deviations (conservative): no subclassing and no @@species. SpeciesConstructor reads `constructor` only; a
//! constructor other than %Promise% throws TypeError "unsupported". Promise statics need a Promise `this`.

pub mod async_rt;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub use async_rt::{async_start, await_value, frame_get, frame_set, new_frame, AwaitFuture};

use crate::builtins::iterator::{get_iterator, iter_close, iter_step, iterate, IterRecord};
use crate::error::ErrorKind;
use crate::function::Call;
use crate::jsstr::JsStr;
use crate::native::{payload_ref, NativeObject};
use crate::object::{Flags, Obj, ObjectKind, Prop};
use crate::roots::Tracer;
use crate::rt::{Intrinsic, Rt, WellKnown};
use crate::value::{ObjRef, PropKey, Res, Throw, Value};

/// Settlement of a promise as seen from Rust.
#[derive(Clone, Debug, PartialEq)]
pub enum PromiseState {
    Pending,
    Fulfilled(Value),
    Rejected(Value),
}

/// A reaction handler: a callable, the empty handler (the argument passes through), or an `await` continuation.
#[derive(Clone)]
pub(crate) enum Handler {
    Empty,
    Js(Value),
    Await(ObjRef),
}

/// PromiseCapability: the promise and its resolving functions (JavaScript-visible function objects).
#[derive(Clone)]
pub(crate) struct Capability {
    pub(crate) promise: ObjRef,
    pub(crate) resolve: Value,
    pub(crate) reject: Value,
}

#[derive(Clone)]
pub(crate) struct Reaction {
    pub(crate) on_ok: Handler,
    pub(crate) on_err: Handler,
    pub(crate) cap: Option<Capability>,
}

/// The payload of a promise object.
pub(crate) struct PromiseData {
    pub(crate) state: RefCell<PromiseState>,
    pub(crate) reactions: RefCell<Vec<Reaction>>,
    pub(crate) handled: Cell<bool>,
}

impl PromiseData {
    fn new() -> PromiseData {
        PromiseData { state: RefCell::new(PromiseState::Pending), reactions: RefCell::new(Vec::new()), handled: Cell::new(false) }
    }
}

fn trace_handler(h: &Handler, t: &mut Tracer) {
    match h {
        Handler::Empty => {}
        Handler::Js(v) => t.mark_value(v),
        Handler::Await(task) => t.mark(*task),
    }
}

impl NativeObject for PromiseData {
    fn trace(&self, t: &mut Tracer) {
        match &*self.state.borrow() {
            PromiseState::Pending => {}
            PromiseState::Fulfilled(v) | PromiseState::Rejected(v) => t.mark_value(v),
        }
        for r in self.reactions.borrow().iter() {
            trace_handler(&r.on_ok, t);
            trace_handler(&r.on_err, t);
            if let Some(c) = &r.cap {
                t.mark(c.promise);
                t.mark_value(&c.resolve);
                t.mark_value(&c.reject);
            }
        }
    }
}

/// A one-shot record: the spec's `alreadyResolved` or `alreadyCalled` flag.
struct Flag(Cell<bool>);

impl NativeObject for Flag {
    fn trace(&self, _t: &mut Tracer) {}
}

/// State of one Promise.all / allSettled / any call, shared by its element functions.
struct Collector {
    values: RefCell<Vec<Value>>,
    remaining: Cell<usize>,
    any: bool,
    resolve: Value,
    reject: Value,
}

impl NativeObject for Collector {
    fn trace(&self, t: &mut Tracer) {
        for v in self.values.borrow().iter() {
            t.mark_value(v);
        }
        t.mark_value(&self.resolve);
        t.mark_value(&self.reject);
    }
}

/// The job functions of the realm.
struct Jobs {
    reaction: ObjRef,
    thenable: ObjRef,
    await_resume: ObjRef,
    quiescence: ObjRef,
}

/// Per-realm Promise state. It lives in a native object registered as "promise.realm", so the collector traces it.
struct Realm {
    promise_ctor: ObjRef,
    promise_proto: ObjRef,
    jobs: Jobs,
    /// Rejected promises with no handler, in rejection order.
    unhandled: RefCell<Vec<ObjRef>>,
    /// Promises waiting for the job queue to drain (`quiescence_promise`).
    waiters: RefCell<Vec<ObjRef>>,
    sentinel_queued: Cell<bool>,
    /// Async tasks being polled, innermost last.
    tasks: RefCell<Vec<ObjRef>>,
}

impl NativeObject for Realm {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.promise_ctor);
        t.mark(self.promise_proto);
        t.mark(self.jobs.reaction);
        t.mark(self.jobs.thenable);
        t.mark(self.jobs.await_resume);
        t.mark(self.jobs.quiescence);
        for r in self.unhandled.borrow().iter().chain(self.waiters.borrow().iter()).chain(self.tasks.borrow().iter()) {
            t.mark(*r);
        }
    }
}

fn with_realm<R>(rt: &Rt, f: impl FnOnce(&Realm) -> R) -> R {
    let holder = rt.builtin("promise.realm").expect("the promise realm is installed by Rt::new (framework bug)");
    let payload = rt.native_payload(holder).expect("promise realm holder without payload (framework bug)");
    f(payload_ref::<Realm>(&*payload).expect("promise realm holder with a foreign payload (framework bug)"))
}

fn job(rt: &Rt, pick: fn(&Jobs) -> ObjRef) -> Value {
    Value::Object(with_realm(rt, |r| pick(&r.jobs)))
}

fn promise_data<R>(rt: &Rt, p: ObjRef, f: impl FnOnce(&PromiseData) -> R) -> R {
    let payload = rt.native_payload(p).expect("promise object without payload (framework bug)");
    f(payload_ref::<PromiseData>(&*payload).expect("object is not a promise (framework bug)"))
}

/// The promise behind `v`, when `v` is a Promise object.
fn as_promise(rt: &Rt, v: &Value) -> Option<ObjRef> {
    let Value::Object(o) = v else { return None };
    let payload = rt.native_payload(*o)?;
    let is_promise = payload_ref::<PromiseData>(&*payload).is_some();
    if is_promise { Some(*o) } else { None }
}

fn new_promise_object(rt: &Rt, proto: ObjRef) -> ObjRef {
    rt.new_native_object(Some(proto), Rc::new(PromiseData::new()))
}

/// A new pending promise with the intrinsic prototype (not observable: %Promise% is not subclassed).
pub fn new_promise(rt: &Rt) -> ObjRef {
    let proto = with_realm(rt, |r| r.promise_proto);
    new_promise_object(rt, proto)
}

fn new_flag(rt: &Rt) -> ObjRef {
    rt.new_native_object(None, Rc::new(Flag(Cell::new(false))))
}

/// Claims a flag: true for the first caller only.
fn claim(rt: &Rt, flag: ObjRef) -> bool {
    let payload = rt.native_payload(flag).expect("flag object without payload (framework bug)");
    !payload_ref::<Flag>(&*payload).expect("flag object with a foreign payload (framework bug)").0.replace(true)
}

/// CreateResolvingFunctions (ECMA-262 27.2.1.3): the resolve and reject functions of `p`, sharing one flag.
fn create_resolving_functions(rt: &Rt, p: ObjRef) -> (Value, Value) {
    let data = vec![Value::Object(p), Value::Object(new_flag(rt))];
    let resolve = rt.new_native_function(resolve_function, "", 1, data.clone());
    let reject = rt.new_native_function(reject_function, "", 1, data);
    (Value::Object(resolve), Value::Object(reject))
}

fn resolving_parts(rt: &Rt, c: &Call<'_>) -> (ObjRef, ObjRef) {
    let d = rt.fn_data(c.callee);
    match (&d[0], &d[1]) {
        (Value::Object(p), Value::Object(f)) => (*p, *f),
        _ => panic!("resolving function without its promise (framework bug)"),
    }
}

fn resolve_function(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (p, flag) = resolving_parts(rt, c);
    if claim(rt, flag) {
        resolve_promise(rt, p, c.arg(0));
    }
    Ok(Value::Undefined)
}

fn reject_function(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (p, flag) = resolving_parts(rt, c);
    if claim(rt, flag) {
        reject_promise(rt, p, c.arg(0));
    }
    Ok(Value::Undefined)
}

/// The resolve procedure after the `alreadyResolved` check (ECMA-262 27.2.1.3.2 steps 3-8). Never throws: a throw from
/// the thenable's `then` getter rejects the promise.
pub(crate) fn resolve_promise(rt: &Rt, p: ObjRef, resolution: Value) {
    let Value::Object(r) = resolution else {
        return fulfill_promise(rt, p, resolution);
    };
    if r == p {
        let e = rt.make_error(ErrorKind::Type, "Chaining cycle detected for promise #<Promise>");
        return reject_promise(rt, p, e);
    }
    let then = match rt.get_str(&Value::Object(r), "then") {
        Ok(t) => t,
        Err(Throw(e)) => return reject_promise(rt, p, e),
    };
    if !rt.is_callable(&then) {
        return fulfill_promise(rt, p, Value::Object(r));
    }
    let f = job(rt, |j| j.thenable);
    rt.enqueue_job(f, vec![Value::Object(p), Value::Object(r), then]);
}

pub(crate) fn fulfill_promise(rt: &Rt, p: ObjRef, v: Value) {
    let reactions = promise_data(rt, p, |d| {
        if !matches!(*d.state.borrow(), PromiseState::Pending) {
            return None;
        }
        *d.state.borrow_mut() = PromiseState::Fulfilled(v.clone());
        Some(std::mem::take(&mut *d.reactions.borrow_mut()))
    });
    if let Some(reactions) = reactions {
        trigger(rt, reactions, false, v);
    }
}

/// RejectPromise (ECMA-262 27.2.1.7). A promise without a handler is listed as unhandled (HostPromiseRejectionTracker).
pub(crate) fn reject_promise(rt: &Rt, p: ObjRef, reason: Value) {
    let settled = promise_data(rt, p, |d| {
        if !matches!(*d.state.borrow(), PromiseState::Pending) {
            return None;
        }
        *d.state.borrow_mut() = PromiseState::Rejected(reason.clone());
        Some((std::mem::take(&mut *d.reactions.borrow_mut()), d.handled.get()))
    });
    let Some((reactions, handled)) = settled else { return };
    if !handled {
        with_realm(rt, |r| r.unhandled.borrow_mut().push(p));
    }
    trigger(rt, reactions, true, reason);
}

fn trigger(rt: &Rt, reactions: Vec<Reaction>, is_reject: bool, arg: Value) {
    for r in reactions {
        let h = if is_reject { r.on_err } else { r.on_ok };
        enqueue_reaction(rt, h, r.cap, is_reject, arg.clone());
    }
}

/// Queues the PromiseReactionJob for one reaction (ECMA-262 27.2.2.1).
fn enqueue_reaction(rt: &Rt, h: Handler, cap: Option<Capability>, is_reject: bool, arg: Value) {
    match h {
        Handler::Await(task) => {
            let f = job(rt, |j| j.await_resume);
            rt.enqueue_job(f, vec![Value::Object(task), Value::Bool(is_reject), arg]);
        }
        h => {
            let handler = match h {
                Handler::Js(v) => v,
                _ => Value::Undefined,
            };
            let (resolve, reject) = match cap {
                Some(c) => (c.resolve, c.reject),
                None => (Value::Undefined, Value::Undefined),
            };
            let f = job(rt, |j| j.reaction);
            rt.enqueue_job(f, vec![handler, resolve, reject, Value::Bool(is_reject), arg]);
        }
    }
}

/// Body of the PromiseReactionJob. Arguments: handler (undefined = empty), resolve, reject, is_reject, argument.
fn reaction_job(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (handler, resolve, reject) = (c.arg(0), c.arg(1), c.arg(2));
    let is_reject = c.arg(3).to_boolean();
    let arg = c.arg(4);
    let outcome = if handler.is_undefined() {
        if is_reject { Err(Throw(arg)) } else { Ok(arg) }
    } else {
        rt.call(&handler, &Value::Undefined, &[arg])
    };
    if resolve.is_undefined() {
        return Ok(Value::Undefined);
    }
    match outcome {
        Ok(v) => rt.call(&resolve, &Value::Undefined, &[v])?,
        Err(Throw(e)) => rt.call(&reject, &Value::Undefined, &[e])?,
    };
    Ok(Value::Undefined)
}

/// NewPromiseResolveThenableJob (ECMA-262 27.2.2.2). Arguments: promise, thenable, then.
fn thenable_job(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Value::Object(p) = c.arg(0) else {
        panic!("thenable job without a promise (framework bug)")
    };
    let (resolve, reject) = create_resolving_functions(rt, p);
    if let Err(Throw(e)) = rt.call(&c.arg(2), &c.arg(1), &[resolve, reject.clone()]) {
        rt.call(&reject, &Value::Undefined, &[e])?;
    }
    Ok(Value::Undefined)
}

/// The macrotask boundary (`quiescence_promise`): a job that re-queues itself while other jobs wait, and resolves the
/// waiting promises once it is the only job left. This is the "job queue drained" point of the trace format.
fn quiescence_job(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    if rt.0.jobs.borrow().len() > 0 {
        let f = job(rt, |j| j.quiescence);
        rt.enqueue_job(f, Vec::new());
        return Ok(Value::Undefined);
    }
    let waiters = with_realm(rt, |r| {
        r.sentinel_queued.set(false);
        std::mem::take(&mut *r.waiters.borrow_mut())
    });
    for w in waiters {
        resolve_promise(rt, w, Value::Undefined);
    }
    Ok(Value::Undefined)
}

/// A promise that resolves with undefined the next time the job queue is empty. `testHost.tick` returns it, and
/// `testHost.timers.advance` awaits it between steps (the oracle's `drain`).
pub fn quiescence_promise(rt: &Rt) -> ObjRef {
    let p = new_promise(rt);
    let need_sentinel = with_realm(rt, |r| {
        r.waiters.borrow_mut().push(p);
        !r.sentinel_queued.replace(true)
    });
    if need_sentinel {
        let f = job(rt, |j| j.quiescence);
        rt.enqueue_job(f, Vec::new());
    }
    p
}

/// NewPromiseCapability(%Promise%) (ECMA-262 27.2.1.5).
fn new_promise_capability(rt: &Rt) -> Capability {
    let promise = new_promise(rt);
    let (resolve, reject) = create_resolving_functions(rt, promise);
    Capability { promise, resolve, reject }
}

/// PromiseResolve(%Promise%, x) (ECMA-262 27.2.4.7.1).
pub(crate) fn promise_resolve_intrinsic(rt: &Rt, x: Value) -> Res<ObjRef> {
    let ctor = with_realm(rt, |r| r.promise_ctor);
    if let Some(p) = as_promise(rt, &x) {
        let k = rt.get(p, &PropKey::from_str("constructor"), &Value::Object(p))?;
        if k == Value::Object(ctor) {
            return Ok(p);
        }
    }
    let cap = new_promise_capability(rt);
    rt.call(&cap.resolve, &Value::Undefined, &[x])?;
    Ok(cap.promise)
}

/// PerformPromiseThen (ECMA-262 27.2.5.4.1). `cap` is None for `await`.
pub(crate) fn perform_then(rt: &Rt, p: ObjRef, on_ok: Handler, on_err: Handler, cap: Option<Capability>) {
    let (was_handled, state) = promise_data(rt, p, |d| (d.handled.replace(true), d.state.borrow().clone()));
    match state {
        PromiseState::Pending => promise_data(rt, p, move |d| {
            d.reactions.borrow_mut().push(Reaction { on_ok, on_err, cap });
        }),
        PromiseState::Fulfilled(v) => enqueue_reaction(rt, on_ok, cap, false, v),
        PromiseState::Rejected(v) => {
            if !was_handled {
                with_realm(rt, |r| {
                    let mut u = r.unhandled.borrow_mut();
                    if let Some(i) = u.iter().position(|x| *x == p) {
                        u.remove(i);
                    }
                });
            }
            enqueue_reaction(rt, on_err, cap, true, v);
        }
    }
}

fn handler_of(rt: &Rt, v: Value) -> Handler {
    if rt.is_callable(&v) { Handler::Js(v) } else { Handler::Empty }
}

fn incompatible(rt: &Rt, method: &str, this: &Value) -> Throw {
    rt.throw_type_error(&format!("Method {method} called on incompatible receiver {}", rt.describe(this)))
}

/// SpeciesConstructor(p, %Promise%) without @@species: an undefined or %Promise% `constructor` is the only supported case.
fn species_is_plain(rt: &Rt, p: ObjRef) -> Res<()> {
    let ctor = rt.get(p, &PropKey::from_str("constructor"), &Value::Object(p))?;
    let plain = with_realm(rt, |r| r.promise_ctor);
    match ctor {
        Value::Undefined => Ok(()),
        Value::Object(o) if o == plain => Ok(()),
        Value::Object(_) => Err(rt.throw_type_error("unsupported: Promise subclassing (constructor is not Promise)")),
        _ => Err(rt.throw_type_error("The .constructor property is not an object")),
    }
}

/// Statics need a Promise `this` (subclassing is not supported).
fn require_plain_this(rt: &Rt, this: &Value, method: &str) -> Res<()> {
    let plain = with_realm(rt, |r| r.promise_ctor);
    match this {
        Value::Object(o) if *o == plain => Ok(()),
        Value::Object(_) => Err(rt.throw_type_error(&format!("unsupported: {method} on a constructor other than Promise"))),
        _ => Err(rt.throw_type_error(&format!("{method} called on non-object"))),
    }
}

/// Invoke(V, name, args) (ECMA-262 7.3.21).
fn invoke(rt: &Rt, target: &Value, name: &str, args: &[Value]) -> Res<Value> {
    let f = rt.get_str(target, name)?;
    rt.call(&f, target, args)
}

/// The prototype for an object created by a constructor call: `new_target.prototype` when it is an object.
fn proto_from_new_target(rt: &Rt, new_target: &Value, default: ObjRef) -> Res<ObjRef> {
    if let Value::Object(n) = new_target {
        if let Value::Object(p) = rt.get(*n, &PropKey::from_str("prototype"), new_target)? {
            return Ok(p);
        }
    }
    Ok(default)
}

fn promise_call(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("Promise constructor cannot be invoked without 'new'"))
}

/// `new Promise(executor)` (ECMA-262 27.2.3.1).
fn promise_construct(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let executor = c.arg(0);
    if !rt.is_callable(&executor) {
        return Err(rt.throw_type_error(&format!("Promise resolver {} is not a function", rt.describe(&executor))));
    }
    let default = with_realm(rt, |r| r.promise_proto);
    let proto = proto_from_new_target(rt, c.new_target, default)?;
    let p = new_promise_object(rt, proto);
    let (resolve, reject) = create_resolving_functions(rt, p);
    if let Err(Throw(e)) = rt.call(&executor, &Value::Undefined, &[resolve, reject.clone()]) {
        rt.call(&reject, &Value::Undefined, &[e])?;
    }
    Ok(Value::Object(p))
}

/// `Promise.prototype.then` (ECMA-262 27.2.5.4).
fn proto_then(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Some(p) = as_promise(rt, c.this) else {
        return Err(incompatible(rt, "Promise.prototype.then", c.this));
    };
    species_is_plain(rt, p)?;
    let cap = new_promise_capability(rt);
    let on_ok = handler_of(rt, c.arg(0));
    let on_err = handler_of(rt, c.arg(1));
    let result = Value::Object(cap.promise);
    perform_then(rt, p, on_ok, on_err, Some(cap));
    Ok(result)
}

/// `Promise.prototype.catch` (ECMA-262 27.2.5.1): Invoke(this, "then", undefined, onRejected).
fn proto_catch(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    invoke(rt, c.this, "then", &[Value::Undefined, c.arg(0)])
}

/// `Promise.prototype.finally` (ECMA-262 27.2.5.3).
fn proto_finally(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let this = c.this;
    let Value::Object(p) = this else {
        return Err(rt.throw_type_error("Promise.prototype.finally called on a non-object"));
    };
    species_is_plain(rt, *p)?;
    let on_finally = c.arg(0);
    let (then_finally, catch_finally) = if rt.is_callable(&on_finally) {
        let ctor = Value::Object(with_realm(rt, |r| r.promise_ctor));
        let tf = rt.new_native_function(then_finally_fn, "", 1, vec![on_finally.clone(), ctor.clone()]);
        let cf = rt.new_native_function(catch_finally_fn, "", 1, vec![on_finally, ctor]);
        (Value::Object(tf), Value::Object(cf))
    } else {
        (on_finally.clone(), on_finally)
    };
    invoke(rt, this, "then", &[then_finally, catch_finally])
}

/// thenFinally of ECMA-262 27.2.5.3.1. Data: onFinally, the constructor.
fn then_finally_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let on_finally = rt.fn_data(c.callee)[0].clone();
    let result = rt.call(&on_finally, &Value::Undefined, &[])?;
    let p = promise_resolve_intrinsic(rt, result)?;
    let thunk = rt.new_native_function(value_thunk_fn, "", 0, vec![c.arg(0)]);
    invoke(rt, &Value::Object(p), "then", &[Value::Object(thunk)])
}

/// catchFinally of ECMA-262 27.2.5.3.2. Data: onFinally, the constructor.
fn catch_finally_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let on_finally = rt.fn_data(c.callee)[0].clone();
    let result = rt.call(&on_finally, &Value::Undefined, &[])?;
    let p = promise_resolve_intrinsic(rt, result)?;
    let thrower = rt.new_native_function(thrower_fn, "", 0, vec![c.arg(0)]);
    invoke(rt, &Value::Object(p), "then", &[Value::Object(thrower)])
}

fn value_thunk_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(rt.fn_data(c.callee)[0].clone())
}

fn thrower_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Err(Throw(rt.fn_data(c.callee)[0].clone()))
}

/// `Promise.resolve(x)` (ECMA-262 27.2.4.7).
fn promise_resolve_static(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    require_plain_this(rt, c.this, "Promise.resolve")?;
    Ok(Value::Object(promise_resolve_intrinsic(rt, c.arg(0))?))
}

/// `Promise.reject(r)` (ECMA-262 27.2.4.6).
fn promise_reject_static(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    require_plain_this(rt, c.this, "Promise.reject")?;
    let cap = new_promise_capability(rt);
    rt.call(&cap.reject, &Value::Undefined, &[c.arg(0)])?;
    Ok(Value::Object(cap.promise))
}

/// `Promise.withResolvers()` (ES2024).
fn promise_with_resolvers(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    require_plain_this(rt, c.this, "Promise.withResolvers")?;
    let cap = new_promise_capability(rt);
    let o = rt.new_object();
    rt.define_data(o, PropKey::from_str("promise"), Value::Object(cap.promise), Flags::ALL)?;
    rt.define_data(o, PropKey::from_str("resolve"), cap.resolve, Flags::ALL)?;
    rt.define_data(o, PropKey::from_str("reject"), cap.reject, Flags::ALL)?;
    Ok(Value::Object(o))
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    All,
    AllSettled,
    Any,
    Race,
}

fn promise_all(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    combinator(rt, c, Kind::All, "Promise.all")
}

fn promise_all_settled(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    combinator(rt, c, Kind::AllSettled, "Promise.allSettled")
}

fn promise_any(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    combinator(rt, c, Kind::Any, "Promise.any")
}

fn promise_race(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    combinator(rt, c, Kind::Race, "Promise.race")
}

/// The shared shape of the four combinators (ECMA-262 27.2.4.1 to 27.2.4.5): capability, GetPromiseResolve,
/// GetIterator, then the per-kind loop. Errors reject the returned promise (IfAbruptRejectPromise).
fn combinator(rt: &Rt, c: &Call<'_>, kind: Kind, method: &str) -> Res<Value> {
    require_plain_this(rt, c.this, method)?;
    let this = c.this.clone();
    let cap = new_promise_capability(rt);
    let result = Value::Object(cap.promise);
    let pr = match rt.get_str(&this, "resolve") {
        Ok(f) if rt.is_callable(&f) => f,
        Ok(_) => {
            let e = rt.throw_type_error("Promise resolve or not a function");
            rt.call(&cap.reject, &Value::Undefined, &[e.0])?;
            return Ok(result);
        }
        Err(Throw(e)) => {
            rt.call(&cap.reject, &Value::Undefined, &[e])?;
            return Ok(result);
        }
    };
    let mut rec = match get_iterator(rt, &c.arg(0)) {
        Ok(r) => r,
        Err(Throw(e)) => {
            rt.call(&cap.reject, &Value::Undefined, &[e])?;
            return Ok(result);
        }
    };
    if let Err(Throw(e)) = perform(rt, kind, &mut rec, &this, &cap, &pr) {
        if !rec.done {
            let _ = iter_close(rt, &rec, true);
        }
        rt.call(&cap.reject, &Value::Undefined, &[e])?;
    }
    Ok(result)
}

/// PerformPromiseAll, PerformPromiseAllSettled, PerformPromiseAny and PerformPromiseRace.
fn perform(rt: &Rt, kind: Kind, rec: &mut IterRecord, ctor: &Value, cap: &Capability, pr: &Value) -> Res<()> {
    let col = rt.new_native_object(
        None,
        Rc::new(Collector {
            values: RefCell::new(Vec::new()),
            remaining: Cell::new(1),
            any: kind == Kind::Any,
            resolve: cap.resolve.clone(),
            reject: cap.reject.clone(),
        }),
    );
    loop {
        let Some(next) = iter_step(rt, rec)? else { break };
        if kind == Kind::Race {
            let np = rt.call(pr, ctor, &[next])?;
            invoke(rt, &np, "then", &[cap.resolve.clone(), cap.reject.clone()])?;
            continue;
        }
        let index = {
            let payload = rt.native_payload(col).expect("collector without payload (framework bug)");
            let k = payload_ref::<Collector>(&*payload).expect("collector with a foreign payload (framework bug)");
            let mut v = k.values.borrow_mut();
            v.push(Value::Undefined);
            v.len() - 1
        };
        let np = rt.call(pr, ctor, &[next])?;
        let flag = new_flag(rt);
        let element = |rt: &Rt, code: crate::function::CodeFn, is_reject: bool| {
            let data = vec![
                Value::Object(col),
                Value::Number(index as f64),
                Value::Object(flag),
                Value::Bool(is_reject),
            ];
            Value::Object(rt.new_native_function(code, "", 1, data))
        };
        let (on_ok, on_err) = match kind {
            Kind::All => (element(rt, all_element, false), cap.reject.clone()),
            Kind::AllSettled => (element(rt, settled_element, false), element(rt, settled_element, true)),
            _ => (cap.resolve.clone(), element(rt, any_element, true)),
        };
        {
            let payload = rt.native_payload(col).expect("collector without payload (framework bug)");
            let k = payload_ref::<Collector>(&*payload).expect("collector with a foreign payload (framework bug)");
            k.remaining.set(k.remaining.get() + 1);
        }
        invoke(rt, &np, "then", &[on_ok, on_err])?;
    }
    if kind == Kind::Race {
        return Ok(());
    }
    let payload = rt.native_payload(col).expect("collector without payload (framework bug)");
    let k = payload_ref::<Collector>(&*payload).expect("collector with a foreign payload (framework bug)");
    let left = k.remaining.get() - 1;
    k.remaining.set(left);
    if left == 0 {
        finish(rt, k)?;
    }
    Ok(())
}

/// Data of an element function: the collector, its index, the shared flag, and whether it is a rejection.
fn element_parts(rt: &Rt, c: &Call<'_>) -> (ObjRef, usize, ObjRef, bool) {
    let d = rt.fn_data(c.callee);
    match (&d[0], &d[1], &d[2]) {
        (Value::Object(col), Value::Number(i), Value::Object(flag)) => (*col, *i as usize, *flag, d[3].to_boolean()),
        _ => panic!("promise element function without its state (framework bug)"),
    }
}

/// Promise.all resolve element and Promise.any reject element (ECMA-262 27.2.4.1.2, 27.2.4.2.2 and 27.2.4.3.2).
fn all_element(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (col, index, flag, _) = element_parts(rt, c);
    if !claim(rt, flag) {
        return Ok(Value::Undefined);
    }
    store(rt, col, index, c.arg(0))
}

fn any_element(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    all_element(rt, c)
}

/// Promise.allSettled element functions: the status record for the outcome.
fn settled_element(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (col, index, flag, is_reject) = element_parts(rt, c);
    if !claim(rt, flag) {
        return Ok(Value::Undefined);
    }
    let o = rt.new_object();
    let status = if is_reject { "rejected" } else { "fulfilled" };
    rt.define_data(o, PropKey::from_str("status"), Value::str(status), Flags::ALL)?;
    let key = if is_reject { "reason" } else { "value" };
    rt.define_data(o, PropKey::from_str(key), c.arg(0), Flags::ALL)?;
    store(rt, col, index, Value::Object(o))
}

/// Stores one element result, and settles the combinator's promise when the last element arrives.
fn store(rt: &Rt, col: ObjRef, index: usize, v: Value) -> Res<Value> {
    let payload = rt.native_payload(col).expect("collector without payload (framework bug)");
    let k = payload_ref::<Collector>(&*payload).expect("collector with a foreign payload (framework bug)");
    k.values.borrow_mut()[index] = v;
    let left = k.remaining.get() - 1;
    k.remaining.set(left);
    if left == 0 {
        finish(rt, k)?;
    }
    Ok(Value::Undefined)
}

fn finish(rt: &Rt, k: &Collector) -> Res<()> {
    let items = k.values.borrow().to_vec();
    let arr = rt.new_array(items);
    if k.any {
        let proto = rt.builtin("AggregateError.prototype").expect("AggregateError is installed by Rt::new (framework bug)");
        let err = new_aggregate(rt, proto, arr, Some(JsStr::from_str("All promises were rejected")), None);
        rt.call(&k.reject, &Value::Undefined, &[Value::Object(err)])?;
    } else {
        rt.call(&k.resolve, &Value::Undefined, &[Value::Object(arr)])?;
    }
    Ok(())
}

/// AggregateError objects: an Error object with own `stack`, `message` (when given), `cause` (when given) and `errors`.
fn new_aggregate(rt: &Rt, proto: ObjRef, errors: ObjRef, message: Option<JsStr>, cause: Option<Value>) -> ObjRef {
    let o = rt.alloc(Obj::new(Some(proto), ObjectKind::Error));
    let msg = message.clone().unwrap_or_else(JsStr::empty);
    let head = if msg.is_empty() {
        JsStr::from_str("AggregateError")
    } else {
        JsStr::from_str("AggregateError: ").concat(&msg)
    };
    let stack = head.concat(&JsStr::from_str("\n    at <anonymous>"));
    rt.define_builtin(o, PropKey::from_str("stack"), Prop::Data { value: Value::String(stack), flags: Flags::HIDDEN });
    if let Some(m) = message {
        rt.define_builtin(o, PropKey::from_str("message"), Prop::Data { value: Value::String(m), flags: Flags::HIDDEN });
    }
    if let Some(c) = cause {
        rt.define_builtin(o, PropKey::from_str("cause"), Prop::Data { value: c, flags: Flags::HIDDEN });
    }
    rt.define_builtin(o, PropKey::from_str("errors"), Prop::Data { value: Value::Object(errors), flags: Flags::HIDDEN });
    o
}

/// The `cause` option (InstallErrorCause): read only when the options object has the property.
fn cause_option(rt: &Rt, opts: &Value) -> Res<Option<Value>> {
    if let Value::Object(o) = opts {
        let key = PropKey::from_str("cause");
        if rt.has_property(*o, &key)? {
            return Ok(Some(rt.get(*o, &key, opts)?));
        }
    }
    Ok(None)
}

/// `AggregateError(errors, message, options)` (ECMA-262 20.5.7.1). Called with or without `new`.
fn aggregate_new(rt: &Rt, c: &Call<'_>, new_target: &Value) -> Res<Value> {
    let default = match rt.fn_data(c.callee).first() {
        Some(Value::Object(p)) => *p,
        _ => panic!("AggregateError without its prototype (framework bug)"),
    };
    let proto = proto_from_new_target(rt, new_target, default)?;
    let message = if c.arg(1).is_undefined() { None } else { Some(rt.to_string(&c.arg(1))?) };
    let cause = cause_option(rt, &c.arg(2))?;
    let mut items = Vec::new();
    iterate(rt, &c.arg(0), &mut |v: Value| {
        items.push(v);
        Ok(true)
    })?;
    let errors = rt.new_array(items);
    Ok(Value::Object(new_aggregate(rt, proto, errors, message, cause)))
}

fn aggregate_call(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    aggregate_new(rt, c, &Value::Undefined)
}

fn aggregate_construct(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    aggregate_new(rt, c, c.new_target)
}

fn install_aggregate_error(rt: &Rt) {
    let proto = rt.new_object_with_proto(Some(rt.intrinsic(Intrinsic::ErrorPrototype)));
    let ctor = rt.new_native_constructor(aggregate_call, aggregate_construct, "AggregateError", 2, vec![Value::Object(proto)]);
    rt.with_obj_mut(ctor, |o| o.proto = Some(rt.intrinsic(Intrinsic::ErrorCtor)));
    rt.define_builtin(ctor, PropKey::from_str("prototype"), Prop::Data { value: Value::Object(proto), flags: Flags::NONE });
    rt.define_builtin(proto, PropKey::from_str("constructor"), Prop::Data { value: Value::Object(ctor), flags: Flags::HIDDEN });
    rt.define_builtin(proto, PropKey::from_str("name"), Prop::Data { value: Value::str("AggregateError"), flags: Flags::HIDDEN });
    rt.define_builtin(proto, PropKey::from_str("message"), Prop::Data { value: Value::str(""), flags: Flags::HIDDEN });
    rt.register_builtin("AggregateError.prototype", proto);
    rt.install_global("AggregateError", Value::Object(ctor));
}

/// Registers the Promise family in a fresh realm (called by `builtins::install_all`).
pub fn install(rt: &Rt) {
    let proto = rt.new_object();
    let ctor = rt.new_native_constructor(promise_call, promise_construct, "Promise", 1, Vec::new());
    rt.define_builtin(ctor, PropKey::from_str("prototype"), Prop::Data { value: Value::Object(proto), flags: Flags::NONE });
    rt.define_builtin(proto, PropKey::from_str("constructor"), Prop::Data { value: Value::Object(ctor), flags: Flags::HIDDEN });
    rt.define_builtin(
        proto,
        PropKey::Sym(rt.well_known(WellKnown::ToStringTag)),
        Prop::Data { value: Value::str("Promise"), flags: Flags::CONFIG },
    );
    rt.define_native_method(proto, "then", proto_then, 2);
    rt.define_native_method(proto, "catch", proto_catch, 1);
    rt.define_native_method(proto, "finally", proto_finally, 1);
    rt.define_native_method(ctor, "all", promise_all, 1);
    rt.define_native_method(ctor, "allSettled", promise_all_settled, 1);
    rt.define_native_method(ctor, "any", promise_any, 1);
    rt.define_native_method(ctor, "race", promise_race, 1);
    rt.define_native_method(ctor, "resolve", promise_resolve_static, 1);
    rt.define_native_method(ctor, "reject", promise_reject_static, 1);
    rt.define_native_method(ctor, "withResolvers", promise_with_resolvers, 0);
    let jobs = Jobs {
        reaction: rt.new_native_function(reaction_job, "", 5, Vec::new()),
        thenable: rt.new_native_function(thenable_job, "", 3, Vec::new()),
        await_resume: rt.new_native_function(async_rt::await_resume_job, "", 3, Vec::new()),
        quiescence: rt.new_native_function(quiescence_job, "", 0, Vec::new()),
    };
    let realm = Realm {
        promise_ctor: ctor,
        promise_proto: proto,
        jobs,
        unhandled: RefCell::new(Vec::new()),
        waiters: RefCell::new(Vec::new()),
        sentinel_queued: Cell::new(false),
        tasks: RefCell::new(Vec::new()),
    };
    let holder = rt.new_native_object(None, Rc::new(realm));
    rt.register_builtin("promise.realm", holder);
    rt.register_builtin("Promise", ctor);
    rt.register_builtin("Promise.prototype", proto);
    rt.install_global("Promise", Value::Object(ctor));
    install_aggregate_error(rt);
}

/// The promise state, for host code and tests (None when `v` is not a promise).
pub fn promise_state(rt: &Rt, v: &Value) -> Option<PromiseState> {
    let p = as_promise(rt, v)?;
    Some(promise_data(rt, p, |d| d.state.borrow().clone()))
}

impl Rt {
    /// The promises rejected with no handler at this point, in rejection order. The list is emptied. A handler attached
    /// to one of them later does not bring it back. Use `promise_state` for the reason.
    pub fn take_unhandled_rejections(&self) -> Vec<Value> {
        with_realm(self, |r| std::mem::take(&mut *r.unhandled.borrow_mut()))
            .into_iter()
            .map(Value::Object)
            .collect()
    }
}
