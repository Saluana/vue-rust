//! The scenario host globals (trace-format.md, "Host globals"): `trace`, and `testHost` with a deterministic
//! fake clock. The objects match the oracle's shape (enumerable properties, names and lengths), so a scenario
//! that inspects them traces the same document.
//!
//! `testHost.tick()` and `testHost.timers.advance(ms)` return Promises (trace-format.md). `tick` resolves at the next
//! quiescence point of the job queue (`quiescence_promise`). `advance` is an async body in the oracle's shape: each step
//! awaits a quiescence point, so the timers fire between drains. `advance_sync` remains the synchronous core for Rust tests.

use std::cell::RefCell;
use std::rc::Rc;

use std::future::Future;
use std::pin::Pin;

use vn_value::builtins::promise::{async_start, await_value, frame_get, frame_set, new_frame, quiescence_promise};
use vn_value::{
    payload_ref, Call, CodeFn, Flags, Intrinsic, NativeObject, ObjRef, PropKey, Res, RootProvider, Rt, Tracer, Value,
};

use crate::trace::{trace_call, Recorder};

struct Timer {
    id: u32,
    due: f64,
    callback: Value,
}

/// Clock and pending timers. `pending` is kept in creation order, so ids ascend along it.
struct TimersState {
    clock: f64,
    next_id: u32,
    pending: Vec<Timer>,
}

impl TimersState {
    fn new() -> TimersState {
        TimersState { clock: 0.0, next_id: 1, pending: Vec::new() }
    }
}

/// Extension payload of every host function: the recorder and the timer table they share.
struct HostState {
    rec: Rc<RefCell<Recorder>>,
    timers: Rc<RefCell<TimersState>>,
}

impl NativeObject for HostState {
    // Holds no ObjRef itself. The callbacks are rooted by `TimerRoots`, and the recorder holds ordinals only.
    fn trace(&self, _t: &mut Tracer) {}
}

/// Keeps every pending callback alive for the whole realm, even after the scenario drops the functions.
struct TimerRoots(Rc<RefCell<TimersState>>);

impl RootProvider for TimerRoots {
    fn trace_roots(&self, t: &mut Tracer) {
        for timer in &self.0.borrow().pending {
            t.mark_value(&timer.callback);
        }
    }
}

/// Handle on the fake clock of an installed realm.
#[derive(Clone)]
pub struct Timers(Rc<RefCell<TimersState>>);

impl Timers {
    /// Fake time in milliseconds (starts at 0).
    pub fn now(&self) -> f64 {
        self.0.borrow().clock
    }

    /// Timers that have neither fired nor been cleared.
    pub fn pending_count(&self) -> usize {
        self.0.borrow().pending.len()
    }

    /// Advances the clock by `ms` (a non-finite or negative delay counts as 0). Due timers fire in (due time,
    /// creation) order. The job queue is drained before the first timer and after each callback, as the format
    /// requires. Call it at call depth 0, between jobs: the drains assert that. A throwing callback ends the
    /// advance with its error, and the clock stays at that timer's due time (as in the oracle).
    pub fn advance_sync(&self, rt: &Rt, ms: f64) -> Res<()> {
        let target = self.now() + delay_of(ms);
        rt.run_jobs();
        loop {
            let fired = {
                let mut st = self.0.borrow_mut();
                match earliest_due(&st.pending, target) {
                    None => None,
                    Some(i) => {
                        let timer = st.pending.remove(i);
                        st.clock = timer.due;
                        Some(timer.callback)
                    }
                }
            };
            let Some(callback) = fired else { break };
            rt.call(&callback, &Value::Undefined, &[])?;
            rt.run_jobs();
        }
        self.0.borrow_mut().clock = target;
        rt.run_jobs();
        Ok(())
    }
}

/// The oracle's `delayOf`: `Number(ms)` when it is finite and positive, else 0.
fn delay_of(ms: f64) -> f64 {
    if ms.is_finite() && ms > 0.0 { ms } else { 0.0 }
}

/// Index of the pending timer with the earliest (due, id) that is due by `limit`.
fn earliest_due(pending: &[Timer], limit: f64) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, t) in pending.iter().enumerate() {
        if t.due > limit {
            continue;
        }
        let better = match best {
            None => true,
            Some(b) => t.due < pending[b].due || (t.due == pending[b].due && t.id < pending[b].id),
        };
        if better {
            best = Some(i);
        }
    }
    best
}

/// Installs `trace` and `testHost` on the realm's global object, wired to `rec`, and returns the clock handle.
/// The globals are enumerable writable properties, as `globalThis.x = ...` makes them in the oracle.
pub fn install_host(rt: &Rt, rec: &Rc<RefCell<Recorder>>) -> Timers {
    let timers = Rc::new(RefCell::new(TimersState::new()));
    rt.add_root_provider(Rc::new(TimerRoots(timers.clone())));
    let state: Rc<dyn NativeObject> = Rc::new(HostState { rec: rec.clone(), timers: timers.clone() });

    let global = rt.intrinsic(Intrinsic::Global);
    let trace = host_fn(rt, &state, trace_native, "trace", 1);
    put(rt, global, "trace", trace);

    let clock = rt.new_object();
    put(rt, clock, "setTimeout", host_fn(rt, &state, set_timeout_native, "setTimeout", 2));
    put(rt, clock, "clearTimeout", host_fn(rt, &state, clear_timeout_native, "clearTimeout", 1));
    put(rt, clock, "now", host_fn(rt, &state, now_native, "now", 0));
    put(rt, clock, "advance", host_fn(rt, &state, advance_native, "advance", 1));

    // The oracle's `tick` is its `drain` function. The name is mirrored, so `testHost.tick.name` matches.
    let tick = host_fn(rt, &state, tick_native, "drain", 0);
    let host = rt.new_object();
    put(rt, host, "timers", Value::Object(clock));
    put(rt, host, "tick", tick);
    put(rt, global, "testHost", Value::Object(host));
    Timers(timers)
}

fn put(rt: &Rt, obj: ObjRef, name: &str, v: Value) {
    let defined = rt.define_data(obj, PropKey::from_str(name), v, Flags::ALL);
    assert!(matches!(defined, Ok(true)), "host install: {name} could not be defined (framework bug)");
}

fn host_fn(rt: &Rt, state: &Rc<dyn NativeObject>, code: CodeFn, name: &str, length: u32) -> Value {
    let f = rt.new_native_function(code, name, length, Vec::new());
    rt.set_ext(f, state.clone());
    Value::Object(f)
}

/// The recorder and timer table of the host function being called.
fn host_state(rt: &Rt, c: &Call<'_>) -> (Rc<RefCell<Recorder>>, Rc<RefCell<TimersState>>) {
    let state = rt.ext(c.callee).expect("host function without host state (framework bug)");
    let h = payload_ref::<HostState>(&*state).expect("host function with foreign state (framework bug)");
    (h.rec.clone(), h.timers.clone())
}

fn trace_native(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Value::String(label) = c.arg(0) else {
        return Err(rt.throw_type_error("trace label must be a string"));
    };
    let (rec, _) = host_state(rt, c);
    trace_call(rt, &rec, &label, c.args.get(1..).unwrap_or(&[]))?;
    Ok(Value::Undefined)
}

fn set_timeout_native(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let callback = c.arg(0);
    if !rt.is_callable(&callback) {
        return Err(rt.throw_type_error("setTimeout callback must be a function"));
    }
    let (_, timers) = host_state(rt, c);
    // The oracle takes the id before it converts the delay, so a throwing delay still consumes an id.
    let id = {
        let mut st = timers.borrow_mut();
        let id = st.next_id;
        st.next_id += 1;
        id
    };
    let delay = rt.to_number(&c.arg(1))?;
    let mut st = timers.borrow_mut();
    let due = st.clock + delay_of(delay);
    st.pending.push(Timer { id, due, callback });
    Ok(Value::Number(f64::from(id)))
}

fn clear_timeout_native(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (_, timers) = host_state(rt, c);
    // Map.delete with SameValueZero: only a number can name a timer.
    if let Value::Number(n) = c.arg(0) {
        timers.borrow_mut().pending.retain(|t| f64::from(t.id) != n);
    }
    Ok(Value::Undefined)
}

fn now_native(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (_, timers) = host_state(rt, c);
    let now = timers.borrow().clock;
    Ok(Value::Number(now))
}

fn advance_native(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (_, timers) = host_state(rt, c);
    // The argument lives in the frame, as compiled code keeps values across awaits.
    let frame = new_frame(rt, 1);
    frame_set(rt, frame, 0, c.arg(0));
    let body: Pin<Box<dyn Future<Output = Res<Value>>>> = Box::pin(advance_body(rt.clone(), timers, frame));
    Ok(async_start(rt, frame, body))
}

/// The oracle's `advance` (an async function): the target is read when the call starts, a conversion error rejects the
/// returned promise, each step waits for the queue to drain, and a throwing callback rejects it too.
async fn advance_body(rt: Rt, timers: Rc<RefCell<TimersState>>, frame: ObjRef) -> Res<Value> {
    let ms = frame_get(&rt, frame, 0);
    let target = timers.borrow().clock + delay_of(rt.to_number(&ms)?);
    drain_step(&rt).await?;
    loop {
        let next = {
            let mut st = timers.borrow_mut();
            match earliest_due(&st.pending, target) {
                None => None,
                Some(i) => {
                    let timer = st.pending.remove(i);
                    st.clock = timer.due;
                    Some(timer.callback)
                }
            }
        };
        let Some(callback) = next else { break };
        rt.call(&callback, &Value::Undefined, &[])?;
        drain_step(&rt).await?;
    }
    timers.borrow_mut().clock = target;
    drain_step(&rt).await?;
    Ok(Value::Undefined)
}

/// `await drain()`: resumes once the job queue is empty.
async fn drain_step(rt: &Rt) -> Res<Value> {
    await_value(rt, Value::Object(quiescence_promise(rt))).await
}

fn tick_native(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Object(quiescence_promise(rt)))
}
