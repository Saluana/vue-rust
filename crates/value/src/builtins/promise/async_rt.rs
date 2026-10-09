//! The async-function runtime (WP-B5 part 2). A compiled `async` body is a Rust future that keeps every local and
//! temporary in its heap frame, so the collector (which runs only between jobs) sees them. `async_start` polls the body
//! synchronously up to its first `await`. Every later resumption is a promise reaction job of the awaited value, never an
//! external executor: the waker is a no-op because nothing ever wakes a task except its reaction.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use super::{perform_then, promise_resolve_intrinsic, reject_promise, resolve_promise, with_realm, Handler};
use crate::function::Call;
use crate::native::{payload_ref, NativeObject};
use crate::roots::Tracer;
use crate::rt::Rt;
use crate::value::{ObjRef, Res, Throw, Value};

/// A heap frame: the slots that hold a compiled function's locals and temporaries while it is suspended.
struct Frame {
    slots: RefCell<Vec<Value>>,
}

impl NativeObject for Frame {
    fn trace(&self, t: &mut Tracer) {
        for v in self.slots.borrow().iter() {
            t.mark_value(v);
        }
    }
}

/// A heap object with `slots` Value slots, all `undefined` at first.
pub fn new_frame(rt: &Rt, slots: usize) -> ObjRef {
    rt.new_native_object(None, Rc::new(Frame { slots: RefCell::new(vec![Value::Undefined; slots]) }))
}

fn frame_of<R>(rt: &Rt, f: ObjRef, k: impl FnOnce(&Frame) -> R) -> R {
    let payload = rt.native_payload(f).expect("frame access on an object that is not a frame (framework bug)");
    k(payload_ref::<Frame>(&*payload).expect("frame access on a foreign object (framework bug)"))
}

/// Slot `i` of a frame. Panics for an index past the end (a compiler bug).
pub fn frame_get(rt: &Rt, f: ObjRef, i: usize) -> Value {
    frame_of(rt, f, |fr| fr.slots.borrow()[i].clone())
}

/// Stores `v` in slot `i` of a frame.
pub fn frame_set(rt: &Rt, f: ObjRef, i: usize, v: Value) {
    frame_of(rt, f, |fr| fr.slots.borrow_mut()[i] = v)
}

/// The body of an async function: its result is the function's completion (`Ok` returns, `Err` throws).
type Body = Pin<Box<dyn Future<Output = Res<Value>>>>;

/// A suspended or running async call: its body, its frame, its result promise, and the value its `await` resumes with.
struct Task {
    body: RefCell<Option<Body>>,
    frame: ObjRef,
    result: ObjRef,
    resume: RefCell<Option<Res<Value>>>,
}

impl NativeObject for Task {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.frame);
        t.mark(self.result);
        if let Some(Ok(v) | Err(Throw(v))) = &*self.resume.borrow() {
            t.mark_value(v);
        }
    }
}

/// Starts an async call: runs `body` synchronously up to its first `await` and returns the result promise. The body
/// completes the promise: `Ok(v)` resolves it with `v` (the usual resolution procedure), `Err(t)` rejects it.
pub fn async_start(rt: &Rt, frame: ObjRef, body: Body) -> Value {
    let result = super::new_promise(rt);
    let task = rt.new_native_object(None, Rc::new(Task { body: RefCell::new(Some(body)), frame, result, resume: RefCell::new(None) }));
    poll_task(rt, task);
    Value::Object(result)
}

/// Polls a task once, with the task on top of the task stack, so `await` knows which task it suspends.
fn poll_task(rt: &Rt, task: ObjRef) {
    let payload = rt.native_payload(task).expect("task object without payload (framework bug)");
    let t = payload_ref::<Task>(&*payload).expect("task object with a foreign payload (framework bug)");
    let mut body = t.body.borrow_mut().take().expect("async task polled while it runs (framework bug)");
    with_realm(rt, |r| r.tasks.borrow_mut().push(task));
    let outcome = {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        body.as_mut().poll(&mut cx)
    };
    with_realm(rt, |r| {
        r.tasks.borrow_mut().pop();
    });
    match outcome {
        Poll::Pending => *t.body.borrow_mut() = Some(body),
        Poll::Ready(Ok(v)) => resolve_promise(rt, t.result, v),
        Poll::Ready(Err(Throw(e))) => reject_promise(rt, t.result, e),
    }
}

fn current_task(rt: &Rt) -> Option<ObjRef> {
    with_realm(rt, |r| r.tasks.borrow().last().copied())
}

fn take_resume(rt: &Rt, task: ObjRef) -> Option<Res<Value>> {
    let payload = rt.native_payload(task)?;
    let t = payload_ref::<Task>(&*payload)?;
    let outcome = t.resume.borrow_mut().take();
    outcome
}

/// `await v`: PromiseResolve(%Promise%, v), then a reaction that resumes the current task. The future must be polled
/// inside the body of an async call, which `async_start` arranges.
pub fn await_value(rt: &Rt, v: Value) -> AwaitFuture {
    AwaitFuture { rt: rt.clone(), state: AwaitState::Start(v) }
}

enum AwaitState {
    Start(Value),
    Waiting(ObjRef),
    Done,
}

/// The future of one `await`: pending until the reaction job resumes the task with the outcome.
pub struct AwaitFuture {
    rt: Rt,
    state: AwaitState,
}

impl Future for AwaitFuture {
    type Output = Res<Value>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Res<Value>> {
        let this = self.get_mut();
        match std::mem::replace(&mut this.state, AwaitState::Done) {
            AwaitState::Start(v) => {
                let task = current_task(&this.rt).expect("await outside an async body (framework bug)");
                match promise_resolve_intrinsic(&this.rt, v) {
                    Err(e) => Poll::Ready(Err(e)),
                    Ok(p) => {
                        perform_then(&this.rt, p, Handler::Await(task), Handler::Await(task), None);
                        this.state = AwaitState::Waiting(task);
                        Poll::Pending
                    }
                }
            }
            AwaitState::Waiting(task) => match take_resume(&this.rt, task) {
                Some(outcome) => Poll::Ready(outcome),
                None => {
                    this.state = AwaitState::Waiting(task);
                    Poll::Pending
                }
            },
            AwaitState::Done => panic!("AwaitFuture polled after it completed (framework bug)"),
        }
    }
}

/// The reaction of an awaited promise. Arguments: task, is_reject, value. It resumes the task and polls it.
pub(crate) fn await_resume_job(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Value::Object(task) = c.arg(0) else {
        panic!("await continuation without its task (framework bug)")
    };
    let outcome = if c.arg(1).to_boolean() { Err(Throw(c.arg(2))) } else { Ok(c.arg(2)) };
    {
        let payload = rt.native_payload(task).expect("task object without payload (framework bug)");
        let t = payload_ref::<Task>(&*payload).expect("task object with a foreign payload (framework bug)");
        *t.resume.borrow_mut() = Some(outcome);
    }
    poll_task(rt, task);
    Ok(Value::Undefined)
}
