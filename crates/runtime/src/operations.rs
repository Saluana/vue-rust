//! vn-runtime::operations: owned asynchronous operations (design.md section 3.4, "Services started with component
//! ownership"). A host performs the external effect; the runtime owns the JS promise and decides whether a result may be
//! delivered. This is the minimal mechanism. It has no user-facing JS API (that comes with the services task), and no
//! real file or HTTP service uses it yet.
//!
//! Ownership. `start_owned` must run under an active effect scope: a component instance's scope (during setup) or any
//! `effectScope`. The first operation of a scope takes an owner slot in the realm's owner table and registers an
//! `onScopeDispose` cleanup on that scope. The cleanup frees the slot when the scope stops. A token is
//! `(scope, generation, operation)`:
//! - `scope` is the owner slot. Slots are recycled after their scope stops.
//! - `generation` is bumped each time a slot is handed to a new owner. A token from an earlier owner of the same slot
//!   never matches a later owner (ABA guard). The design's stable component ID does not exist in the runtime yet, so the
//!   slot and generation pair stands in for it.
//!
//! Delivery. The host reports a result with `complete`, `fail` or `cancel`. A delivery is discarded, and counted in
//! `Stats`, when it is stale (the generation does not match the slot: `DiscardedStale`), when the owner has stopped
//! (`DiscardedStopped`), or when the operation already settled or was cancelled (`DiscardedSettled`; the first settlement
//! wins). A delivery to a live owner settles the promise through its own resolving functions (`Promise.withResolvers`), so
//! thenables and reactions follow the spec. Reactions run as jobs, never inside the call.
//!
//! Choices (the most conservative reading of design 3.4, 5 and 6; docs/evidence/scope-cancellation.md has the details):
//! - Disposal (the owner scope stops) calls `ServiceHost::cancel` for each pending operation of that owner, in operation
//!   order, drops the resolving functions, and leaves the promise pending forever. Design 3.4 says disposal invalidates
//!   delivery, and a late completion "never resolves user promises, never runs user callbacks". Rejecting on disposal would
//!   run the catch callbacks of a disposed component, so it is not done.
//! - An explicit `cancel` by a still-live owner rejects with the ServiceError `{ code: 'Cancelled', message }` (design 5),
//!   then calls `ServiceHost::cancel`. The external effect may still happen (design 3.4). Its result is discarded.
//! - Starting an operation with no active scope, or in a stopped scope, throws a TypeError before any host call. No
//!   operation is ever left unowned.
//! - Ordinary promises are not touched. Nothing here cancels a promise the scope does not own (design 3.4).
//!
//! Usage. A composable calls `start_owned(rt, label)` in setup, returns `Started::promise` to its caller, and keeps
//! `Started::token` for the host request. The host implements `ServiceHost`: `start` begins the request, `cancel` stops it,
//! and the host reports the outcome with `complete` or `fail` using the token it was given.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use vn_value::{payload_ref, Call, Flags, NativeObject, ObjRef, PropKey, Res, Rt, Tracer, Value};

use crate::instance::{shared, Shared};
use crate::props::reactivity_fn;

/// The ServiceError codes of design section 5.
const SERVICE_ERROR_CODES: [&str; 9] = ["Denied", "Revoked", "Unavailable", "Cancelled", "Timeout", "Quota", "Conflict", "InvalidData", "Io"];

/// Identity of one owned operation: the owner slot, the slot's generation at the owner's first operation, and the operation
/// id (unique within the realm).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeliveryToken {
    pub scope: u32,
    pub generation: u64,
    pub operation: u64,
}

/// The host side of owned operations. The runtime never performs the external effect itself.
pub trait ServiceHost {
    /// An operation started under `token`. The host begins the external work and reports the outcome later.
    fn start(&self, token: DeliveryToken, label: &str);
    /// The owner stopped, or the caller cancelled. The external work may still finish; its result is discarded.
    fn cancel(&self, token: DeliveryToken);
}

/// What `start_owned` returns: the promise for the caller, and the token for the host.
pub struct Started {
    pub promise: Value,
    pub token: DeliveryToken,
}

/// The outcome of a delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// `complete` or `fail` settled the promise.
    Delivered,
    /// An explicit `cancel` rejected the promise with code 'Cancelled'.
    Cancelled,
    /// The owner has stopped, so the result is discarded (`Stats::discarded_stopped`).
    DiscardedStopped,
    /// The token's generation does not match its owner slot, so the result is discarded (`Stats::discarded_stale`).
    DiscardedStale,
    /// The operation already settled or was cancelled, so the result is discarded (`Stats::discarded_settled`).
    DiscardedSettled,
}

/// Counters of the realm's owned operations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub started: u64,
    pub delivered: u64,
    pub cancelled_by_caller: u64,
    pub cancelled_by_disposal: u64,
    pub discarded_stopped: u64,
    pub discarded_stale: u64,
    pub discarded_settled: u64,
}

/// One owner slot. `generation` is the generation of the current or last owner; `live` is false once that owner stopped.
struct Slot {
    generation: u64,
    live: bool,
}

/// A pending operation: its token and the resolving functions of its promise.
struct Pending {
    token: DeliveryToken,
    resolve: Value,
    reject: Value,
}

/// The realm's owned-operation state. It is registered as a builtin, so the collector traces the resolving functions.
struct Table {
    host: RefCell<Option<Rc<dyn ServiceHost>>>,
    slots: RefCell<Vec<Slot>>,
    free: RefCell<Vec<u32>>,
    pending: RefCell<HashMap<u64, Pending>>,
    next_operation: Cell<u64>,
    stats: Cell<Stats>,
    promise_ctor: Value,
    with_resolvers: Value,
    get_current_scope: Value,
    on_scope_dispose: Value,
}

impl NativeObject for Table {
    fn trace(&self, t: &mut Tracer) {
        for p in self.pending.borrow().values() {
            t.mark_value(&p.resolve);
            t.mark_value(&p.reject);
        }
        for v in [&self.promise_ctor, &self.with_resolvers, &self.get_current_scope, &self.on_scope_dispose] {
            t.mark_value(v);
        }
    }
}

/// Stored in the ext slot of an owner scope: the slot and generation that the scope holds.
struct OwnerMeta {
    slot: u32,
    generation: u64,
}

impl NativeObject for OwnerMeta {
    fn trace(&self, _t: &mut Tracer) {}
}

fn table(rt: &Rt) -> Shared<Table> {
    shared(rt, "vn-runtime.operations", new_table)
}

fn new_table(rt: &Rt) -> Table {
    let ctor = rt.builtin("Promise").expect("the Promise builtin is installed by Rt::new (framework bug)");
    let promise_ctor = Value::Object(ctor);
    let with_resolvers = rt.get_str(&promise_ctor, "withResolvers").expect("Promise.withResolvers (framework bug)");
    Table {
        host: RefCell::new(None),
        slots: RefCell::new(Vec::new()),
        free: RefCell::new(Vec::new()),
        pending: RefCell::new(HashMap::new()),
        next_operation: Cell::new(1),
        stats: Cell::new(Stats::default()),
        promise_ctor,
        with_resolvers,
        get_current_scope: reactivity_fn(rt, "getCurrentScope"),
        on_scope_dispose: reactivity_fn(rt, "onScopeDispose"),
    }
}

fn bump(t: &Table, f: impl FnOnce(&mut Stats)) {
    let mut s = t.stats.get();
    f(&mut s);
    t.stats.set(s);
}

/// Installs the service host of the realm (replaces an earlier one).
pub fn install(rt: &Rt, host: Rc<dyn ServiceHost>) {
    *table(rt).host.borrow_mut() = Some(host);
}

/// The counters of the realm's owned operations.
pub fn stats(rt: &Rt) -> Stats {
    table(rt).stats.get()
}

/// Starts an owned operation under the active effect scope. `label` names the request for `ServiceHost::start`. Throws a
/// TypeError, without a host call, when no host is installed, when no scope is active, or when the active scope has stopped.
pub fn start_owned(rt: &Rt, label: &str) -> Res<Started> {
    let st = table(rt);
    let t: &Table = &st;
    let host = t.host.borrow().clone().ok_or_else(|| rt.throw_type_error("owned operation: no service host is installed"))?;
    let scope = match rt.call(&t.get_current_scope, &Value::Undefined, &[])? {
        Value::Object(s) => s,
        _ => return Err(rt.throw_type_error("owned operation started outside an effect scope")),
    };
    if !rt.get_str(&Value::Object(scope), "active")?.to_boolean() {
        return Err(rt.throw_type_error("owned operation started in a stopped effect scope"));
    }
    let (slot, generation) = owner_of(rt, scope)?;
    let r = rt.call(&t.with_resolvers, &t.promise_ctor, &[])?;
    let (promise, resolve, reject) = (rt.get_str(&r, "promise")?, rt.get_str(&r, "resolve")?, rt.get_str(&r, "reject")?);
    let operation = t.next_operation.get();
    t.next_operation.set(operation + 1);
    let token = DeliveryToken { scope: slot, generation, operation };
    t.pending.borrow_mut().insert(operation, Pending { token, resolve, reject });
    bump(t, |s| s.started += 1);
    host.start(token, label);
    Ok(Started { promise, token })
}

/// The owner slot of `scope`. The first operation of a scope takes a slot and registers the disposal cleanup.
fn owner_of(rt: &Rt, scope: ObjRef) -> Res<(u32, u64)> {
    if let Some(p) = rt.ext(scope) {
        if let Some(m) = payload_ref::<OwnerMeta>(&*p) {
            return Ok((m.slot, m.generation));
        }
    }
    let st = table(rt);
    let t: &Table = &st;
    let (slot, generation) = take_slot(t);
    let data = vec![Value::Number(slot as f64), Value::Number(generation as f64)];
    let cleanup = rt.new_native_function(dispose_code, "disposeOperations", 0, data);
    rt.call(&t.on_scope_dispose, &Value::Undefined, &[Value::Object(cleanup)])?;
    rt.set_ext(scope, Rc::new(OwnerMeta { slot, generation }));
    Ok((slot, generation))
}

/// Takes a free slot (its generation moves on) or adds one (generation 1).
fn take_slot(t: &Table) -> (u32, u64) {
    let reused = t.free.borrow_mut().pop();
    let mut slots = t.slots.borrow_mut();
    match reused {
        Some(slot) => {
            let s = &mut slots[slot as usize];
            s.generation += 1;
            s.live = true;
            (slot, s.generation)
        }
        None => {
            slots.push(Slot { generation: 1, live: true });
            ((slots.len() - 1) as u32, 1)
        }
    }
}

fn dispose_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let (Value::Number(slot), Value::Number(generation)) = (&d[0], &d[1]) else {
        panic!("owner cleanup data are numbers (framework bug)");
    };
    dispose(rt, *slot as u32, *generation as u64);
    Ok(Value::Undefined)
}

/// Owner disposal: removes the owner's pending operations, frees the slot, and asks the host to cancel each operation in
/// operation order. The promises are not settled (design 3.4).
fn dispose(rt: &Rt, slot: u32, generation: u64) {
    let st = table(rt);
    let t: &Table = &st;
    let mut ids: Vec<u64> = t
        .pending
        .borrow()
        .iter()
        .filter(|(_, p)| p.token.scope == slot && p.token.generation == generation)
        .map(|(id, _)| *id)
        .collect();
    ids.sort_unstable();
    let tokens: Vec<DeliveryToken> = {
        let mut pending = t.pending.borrow_mut();
        ids.iter().filter_map(|id| pending.remove(id)).map(|p| p.token).collect()
    };
    t.slots.borrow_mut()[slot as usize].live = false;
    t.free.borrow_mut().push(slot);
    bump(t, |s| s.cancelled_by_disposal += tokens.len() as u64);
    let host = t.host.borrow().clone();
    if let Some(host) = host {
        for token in tokens {
            host.cancel(token);
        }
    }
}

/// Classifies a delivery. `Ok(Ok(p))` removes the pending operation, and the caller settles its promise.
fn take_pending(rt: &Rt, t: &Table, token: DeliveryToken) -> Res<Result<Pending, Delivery>> {
    let owner = t.slots.borrow().get(token.scope as usize).map(|s| (s.generation, s.live));
    let Some((generation, live)) = owner else {
        return Err(rt.throw_type_error("owned operation: delivery token names an unknown owner"));
    };
    if generation != token.generation {
        bump(t, |s| s.discarded_stale += 1);
        return Ok(Err(Delivery::DiscardedStale));
    }
    if !live {
        bump(t, |s| s.discarded_stopped += 1);
        return Ok(Err(Delivery::DiscardedStopped));
    }
    let mut pending = t.pending.borrow_mut();
    match pending.get(&token.operation).map(|p| p.token == token) {
        Some(true) => Ok(Ok(pending.remove(&token.operation).expect("pending operation (checked above)"))),
        Some(false) => Err(rt.throw_type_error("owned operation: delivery token does not match its operation")),
        None if token.operation < t.next_operation.get() => {
            bump(t, |s| s.discarded_settled += 1);
            Ok(Err(Delivery::DiscardedSettled))
        }
        None => Err(rt.throw_type_error("owned operation: delivery token names an unknown operation")),
    }
}

/// `complete`: the host's result. Resolves the promise with `value` (the resolution procedure applies).
pub fn complete(rt: &Rt, token: DeliveryToken, value: Value) -> Res<Delivery> {
    let st = table(rt);
    match take_pending(rt, &st, token)? {
        Ok(p) => {
            bump(&st, |s| s.delivered += 1);
            rt.call(&p.resolve, &Value::Undefined, &[value])?;
            Ok(Delivery::Delivered)
        }
        Err(d) => Ok(d),
    }
}

/// `fail`: the host's error, as the ServiceError `{ code, message }`. `code` must be one of the design's codes.
pub fn fail(rt: &Rt, token: DeliveryToken, code: &str, message: &str) -> Res<Delivery> {
    if !SERVICE_ERROR_CODES.contains(&code) {
        return Err(rt.throw_type_error(&format!("owned operation: unknown ServiceError code {code}")));
    }
    let st = table(rt);
    match take_pending(rt, &st, token)? {
        Ok(p) => {
            bump(&st, |s| s.delivered += 1);
            let error = service_error(rt, code, message)?;
            rt.call(&p.reject, &Value::Undefined, &[error])?;
            Ok(Delivery::Delivered)
        }
        Err(d) => Ok(d),
    }
}

/// `cancel`: an explicit cancel by a live owner. Rejects the promise with `{ code: 'Cancelled' }`, then asks the host to stop
/// the work. A cancel after the owner stopped or after settlement changes nothing and reports the discard.
pub fn cancel(rt: &Rt, token: DeliveryToken) -> Res<Delivery> {
    let st = table(rt);
    match take_pending(rt, &st, token)? {
        Ok(p) => {
            bump(&st, |s| s.cancelled_by_caller += 1);
            let error = service_error(rt, "Cancelled", "operation cancelled by its caller")?;
            rt.call(&p.reject, &Value::Undefined, &[error])?;
            let host = st.host.borrow().clone();
            if let Some(host) = host {
                host.cancel(token);
            }
            Ok(Delivery::Cancelled)
        }
        Err(d) => Ok(d),
    }
}

fn service_error(rt: &Rt, code: &str, message: &str) -> Res<Value> {
    let o = rt.new_object();
    rt.define_data(o, PropKey::from_str("code"), Value::str(code), Flags::ALL)?;
    rt.define_data(o, PropKey::from_str("message"), Value::str(message), Flags::ALL)?;
    Ok(Value::Object(o))
}
