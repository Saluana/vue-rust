//! vn-runtime::scheduler: the job queue of runtime-core 3.5.43 (production build): `queueJob`, `queuePostFlushCb`,
//! `flushPreFlushCbs`, `flushPostFlushCbs`, `flushJobs`, `queueFlush`, `findInsertionIndex` and `nextTick`.
//! Behaviour per @vue/runtime-core 3.5.43.
//!
//! A job is a JS callable. Its scheduler state (Vue's `job.flags`, `job.id`, `job.i`) lives in a private `JobMeta` record
//! in the job's ext slot: use `job_meta(rt, job)`. Flag bits are Vue's: QUEUED=1, PRE=2, ALLOW_RECURSE=4, DISPOSED=8.
//!
//! Flushing goes through promises, as in Vue: `queueFlush` runs `resolvedPromise.then(flushJobs)`, where `resolvedPromise`
//! is `Promise.resolve()` created once per realm, and `nextTick` chains `.then(fn)` on `currentFlushPromise ||
//! resolvedPromise`. The reactions are kernel jobs, so they interleave with user promise jobs in V8's order.
//!
//! Deviation: the dev-only `checkRecursiveUpdates` is replaced by a resource watchdog. A job that would run more than
//! RECURSION_LIMIT (100) times in one flush fails that flush with a RangeError. Vue's dev build skips the job with a warning
//! instead, and production Vue has no check at all.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::rc::Rc;

use vn_value::{payload_ref, same_value_zero, Call, ErrorKind, Intrinsic, NativeObject, ObjRef, PropKey, Res, Rt, Tracer, Value};

use crate::errors::{call_with_error_handling, code, ErrType};
use crate::instance::{instance_of, shared, Shared};

pub const QUEUED: u32 = 1;
pub const PRE: u32 = 2;
pub const ALLOW_RECURSE: u32 = 4;
pub const DISPOSED: u32 = 8;
pub const RECURSION_LIMIT: u32 = 100;

/// The scheduler's record of a job: `job.flags`, `job.id` and `job.i` of Vue.
pub struct JobMeta {
    pub flags: Cell<u32>,
    /// `job.id`. `None` is `null`/`undefined`, which `getId` maps to -1 for PRE jobs and to Infinity otherwise.
    pub id: Cell<Option<f64>>,
    /// `job.i`: the owning component instance, if any.
    pub instance: Cell<Option<ObjRef>>,
}

impl NativeObject for JobMeta {
    fn trace(&self, t: &mut Tracer) {
        if let Some(i) = self.instance.get() {
            t.mark(i);
        }
    }
}

/// The meta record of a job (shared with the job's ext slot). Derefs to `JobMeta`.
pub struct Meta(Rc<dyn NativeObject>);

impl Deref for Meta {
    type Target = JobMeta;
    fn deref(&self) -> &JobMeta {
        payload_ref::<JobMeta>(&*self.0).expect("job meta payload (framework bug)")
    }
}

/// The meta record of `job`, created on first use.
pub fn job_meta(rt: &Rt, job: ObjRef) -> Meta {
    if let Some(p) = rt.ext(job) {
        if payload_ref::<JobMeta>(&*p).is_some() {
            return Meta(p);
        }
    }
    let p: Rc<dyn NativeObject> = Rc::new(JobMeta { flags: Cell::new(0), id: Cell::new(None), instance: Cell::new(None) });
    rt.set_ext(job, p.clone());
    Meta(p)
}

/// The realm's queue state.
pub(crate) struct SchedState {
    queue: RefCell<Vec<ObjRef>>,
    flush_index: Cell<isize>,
    pending_post: RefCell<Vec<Value>>,
    active_post: RefCell<Option<Vec<Value>>>,
    post_index: Cell<usize>,
    resolved: Cell<Option<ObjRef>>,
    current_flush: Cell<Option<ObjRef>>,
    flush_fn: Cell<Option<ObjRef>>,
}

impl NativeObject for SchedState {
    fn trace(&self, t: &mut Tracer) {
        for j in self.queue.borrow().iter() {
            t.mark(*j);
        }
        for v in self.pending_post.borrow().iter() {
            t.mark_value(v);
        }
        if let Some(a) = self.active_post.borrow().as_ref() {
            for v in a {
                t.mark_value(v);
            }
        }
        for o in [self.resolved.get(), self.current_flush.get(), self.flush_fn.get()].into_iter().flatten() {
            t.mark(o);
        }
    }
}

fn sched(rt: &Rt) -> Shared<SchedState> {
    shared(rt, "vn-runtime.scheduler", |_| SchedState {
        queue: RefCell::new(Vec::new()),
        flush_index: Cell::new(-1),
        pending_post: RefCell::new(Vec::new()),
        active_post: RefCell::new(None),
        post_index: Cell::new(0),
        resolved: Cell::new(None),
        current_flush: Cell::new(None),
        flush_fn: Cell::new(None),
    })
}

/// `getId(job)`.
fn get_id(rt: &Rt, job: ObjRef) -> f64 {
    let m = job_meta(rt, job);
    match m.id.get() {
        Some(id) => id,
        None if m.flags.get() & PRE != 0 => -1.0,
        None => f64::INFINITY,
    }
}

/// `resolvedPromise = Promise.resolve()`, created on first use.
fn resolved_promise(rt: &Rt) -> Res<ObjRef> {
    let st = sched(rt);
    if let Some(p) = st.resolved.get() {
        return Ok(p);
    }
    let global = Value::Object(rt.intrinsic(Intrinsic::Global));
    let ctor = rt.get_str(&global, "Promise")?;
    let resolve = rt.get_str(&ctor, "resolve")?;
    match rt.call(&resolve, &ctor, &[])? {
        Value::Object(o) => {
            st.resolved.set(Some(o));
            Ok(o)
        }
        _ => Err(rt.throw_type_error("Promise.resolve() did not return an object")),
    }
}

/// The `flushJobs` function that the flush promise reacts with.
fn flush_native(rt: &Rt) -> ObjRef {
    let st = sched(rt);
    if let Some(f) = st.flush_fn.get() {
        return f;
    }
    let f = rt.new_native_function(flush_code, "flushJobs", 1, Vec::new());
    st.flush_fn.set(Some(f));
    f
}

fn flush_code(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    flush_jobs(rt)?;
    Ok(Value::Undefined)
}

/// `queueFlush()`: schedules `flushJobs` once per flush (`currentFlushPromise = resolvedPromise.then(flushJobs)`).
pub fn queue_flush(rt: &Rt) -> Res<()> {
    if sched(rt).current_flush.get().is_some() {
        return Ok(());
    }
    let resolved = Value::Object(resolved_promise(rt)?);
    let then = rt.get_str(&resolved, "then")?;
    let f = flush_native(rt);
    let p = rt.call(&then, &resolved, &[Value::Object(f)])?;
    sched(rt).current_flush.set(p.as_object());
    Ok(())
}

/// `findInsertionIndex(id)`: a binary search after the running job, where PRE jobs with the same id come first.
fn find_insertion_index(rt: &Rt, id: f64) -> usize {
    let st = sched(rt);
    let mut start = (st.flush_index.get() + 1).max(0) as usize;
    let mut end = st.queue.borrow().len();
    while start < end {
        let middle = (start + end) >> 1;
        let job = st.queue.borrow()[middle];
        let mid_id = get_id(rt, job);
        let mid_pre = job_meta(rt, job).flags.get() & PRE != 0;
        if mid_id < id || (mid_id == id && mid_pre) {
            start = middle + 1;
        } else {
            end = middle;
        }
    }
    start
}

/// `queueJob(job)`: queues a job once, in id order (PRE jobs before others with the same id), and schedules a flush.
pub fn queue_job(rt: &Rt, job: ObjRef) -> Res<()> {
    let m = job_meta(rt, job);
    if m.flags.get() & QUEUED != 0 {
        return Ok(());
    }
    let job_id = get_id(rt, job);
    let last = sched(rt).queue.borrow().last().copied();
    let at_tail = match last {
        None => true,
        Some(l) => m.flags.get() & PRE == 0 && job_id >= get_id(rt, l),
    };
    if at_tail {
        sched(rt).queue.borrow_mut().push(job);
    } else {
        let idx = find_insertion_index(rt, job_id);
        let st = sched(rt);
        let mut q = st.queue.borrow_mut();
        let idx = idx.min(q.len());
        q.insert(idx, job);
    }
    m.flags.set(m.flags.get() | QUEUED);
    queue_flush(rt)
}

/// `queuePostFlushCb(cb)`: `cb` is a callable or an array of them. Callables are deduplicated by their QUEUED flag; a
/// callback with id -1 queued during the post flush runs right after the current one.
pub fn queue_post_flush_cb(rt: &Rt, cb: &Value) -> Res<()> {
    if rt.is_array(cb) {
        let len = rt.to_number(&rt.get_str(cb, "length")?)? as u32;
        for i in 0..len {
            let item = rt.get_v(cb, &PropKey::Index(i))?;
            sched(rt).pending_post.borrow_mut().push(item);
        }
    } else {
        let Value::Object(job) = cb else {
            return Err(rt.throw_type_error("post flush callback is not an object"));
        };
        let m = job_meta(rt, *job);
        let st = sched(rt);
        let active = st.active_post.borrow().is_some();
        if active && m.id.get() == Some(-1.0) {
            let pos = st.post_index.get() + 1;
            let mut a = st.active_post.borrow_mut();
            let v = a.as_mut().expect("active post queue (framework bug)");
            let pos = pos.min(v.len());
            v.insert(pos, cb.clone());
        } else if m.flags.get() & QUEUED == 0 {
            st.pending_post.borrow_mut().push(cb.clone());
            m.flags.set(m.flags.get() | QUEUED);
        }
    }
    queue_flush(rt)
}

/// `queuePostRenderEffect(fn, suspense)` (`queueEffectWithSuspense`). Suspense is unsupported: a pending suspense branch
/// fails loudly, and anything else is a plain post flush callback.
pub fn queue_post_render_effect(rt: &Rt, f: &Value, suspense: &Value) -> Res<()> {
    if suspense.to_boolean() && rt.get_str(suspense, "pendingBranch")?.to_boolean() {
        return Err(rt.throw_type_error("unsupported: Suspense pending branch"));
    }
    queue_post_flush_cb(rt, f)
}

/// `flushPreFlushCbs(instance, seen, i = flushIndex + 1)`: runs the PRE jobs at or after `from` (of `inst`'s id, when
/// given) directly, removing them from the queue. Errors propagate, as in Vue.
pub fn flush_pre_flush_cbs(rt: &Rt, inst: Option<ObjRef>, from: Option<isize>) -> Res<()> {
    let st = sched(rt);
    let uid = inst.map(|i| instance_of(rt, i).uid);
    let mut i = from.unwrap_or(st.flush_index.get() + 1).max(0);
    loop {
        let len = st.queue.borrow().len() as isize;
        if i >= len {
            break;
        }
        let cb = st.queue.borrow()[i as usize];
        let m = job_meta(rt, cb);
        if m.flags.get() & PRE != 0 {
            if let Some(u) = uid {
                if m.id.get() != Some(u) {
                    i += 1;
                    continue;
                }
            }
            st.queue.borrow_mut().remove(i as usize);
            i -= 1;
            if m.flags.get() & ALLOW_RECURSE != 0 {
                m.flags.set(m.flags.get() & !QUEUED);
            }
            rt.call(&Value::Object(cb), &Value::Undefined, &[])?;
            if m.flags.get() & ALLOW_RECURSE == 0 {
                m.flags.set(m.flags.get() & !QUEUED);
            }
        }
        i += 1;
    }
    Ok(())
}

fn cmp_ids(a: f64, b: f64) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    let d = a - b;
    if d.is_nan() {
        Equal
    } else if d < 0.0 {
        Less
    } else if d > 0.0 {
        Greater
    } else {
        Equal
    }
}

/// `flushPostFlushCbs(seen)`: runs the pending post callbacks, deduplicated and sorted by id (stable). Callbacks that
/// are queued during the run join the active list. A throw propagates, as in Vue.
pub fn flush_post_flush_cbs(rt: &Rt) -> Res<()> {
    let st = sched(rt);
    let pending: Vec<Value> = st.pending_post.borrow().clone();
    if pending.is_empty() {
        return Ok(());
    }
    let mut deduped: Vec<Value> = Vec::new();
    let mut seen_objs: HashSet<ObjRef> = HashSet::new();
    for v in pending {
        let dup = match &v {
            Value::Object(o) => !seen_objs.insert(*o),
            _ => deduped.iter().any(|d| same_value_zero(d, &v)),
        };
        if !dup {
            deduped.push(v);
        }
    }
    let mut keyed: Vec<(f64, Value)> = Vec::with_capacity(deduped.len());
    for v in deduped {
        let id = match &v {
            Value::Object(o) => get_id(rt, *o),
            _ => return Err(rt.throw_type_error("post flush callback is not an object")),
        };
        keyed.push((id, v));
    }
    keyed.sort_by(|a, b| cmp_ids(a.0, b.0));
    st.pending_post.borrow_mut().clear();
    let sorted: Vec<Value> = keyed.into_iter().map(|(_, v)| v).collect();
    if st.active_post.borrow().is_some() {
        st.active_post.borrow_mut().as_mut().expect("active post queue (framework bug)").extend(sorted);
        return Ok(());
    }
    *st.active_post.borrow_mut() = Some(sorted);
    st.post_index.set(0);
    loop {
        let idx = st.post_index.get();
        let next = match st.active_post.borrow().as_ref() {
            Some(v) if idx < v.len() => Some(v[idx].clone()),
            _ => None,
        };
        let Some(cb) = next else { break };
        let Value::Object(job) = cb else {
            return Err(rt.throw_type_error("post flush callback is not an object"));
        };
        let m = job_meta(rt, job);
        if m.flags.get() & ALLOW_RECURSE != 0 {
            m.flags.set(m.flags.get() & !QUEUED);
        }
        if m.flags.get() & DISPOSED == 0 {
            rt.call(&cb, &Value::Undefined, &[])?;
        }
        m.flags.set(m.flags.get() & !QUEUED);
        st.post_index.set(idx + 1);
    }
    *st.active_post.borrow_mut() = None;
    st.post_index.set(0);
    Ok(())
}

/// `checkRecursiveUpdates` as a watchdog: the (RECURSION_LIMIT + 1)-th run of one job in one flush is an error.
fn check_recursion(rt: &Rt, seen: &mut HashMap<ObjRef, u32>, job: ObjRef) -> Res<()> {
    let c = seen.entry(job).or_insert(0);
    if *c == 0 {
        *c = 1;
    } else if *c > RECURSION_LIMIT {
        return Err(rt.throw_error(
            ErrorKind::Range,
            "Maximum recursive updates exceeded: a job re-queued itself more than 100 times in one flush",
        ));
    } else {
        *c += 1;
    }
    Ok(())
}

/// One pass of the `try` block of `flushJobs`: runs the queue in order, from `flushIndex`.
fn flush_body(rt: &Rt) -> Res<()> {
    let st = sched(rt);
    let mut seen: HashMap<ObjRef, u32> = HashMap::new();
    let mut idx: usize = 0;
    loop {
        st.flush_index.set(idx as isize);
        let job = {
            let q = st.queue.borrow();
            if idx >= q.len() {
                break;
            }
            q[idx]
        };
        let m = job_meta(rt, job);
        if m.flags.get() & DISPOSED == 0 {
            check_recursion(rt, &mut seen, job)?;
            if m.flags.get() & ALLOW_RECURSE != 0 {
                m.flags.set(m.flags.get() & !QUEUED);
            }
            let inst = m.instance.get();
            let ty = if inst.is_some() { code::COMPONENT_UPDATE } else { code::SCHEDULER };
            call_with_error_handling(rt, &Value::Object(job), inst, ErrType::Code(ty), None)?;
            if m.flags.get() & ALLOW_RECURSE == 0 {
                m.flags.set(m.flags.get() & !QUEUED);
            }
        }
        idx += 1;
    }
    Ok(())
}

/// The `finally` block of `flushJobs`. Returns whether jobs or post callbacks were queued meanwhile, which means the
/// recursive `flushJobs()` call of Vue runs next. A throw from the post flush propagates (and leaves `currentFlushPromise`
/// set, as in Vue).
fn flush_finally(rt: &Rt) -> Res<bool> {
    let st = sched(rt);
    let mut i = st.flush_index.get().max(0) as usize;
    loop {
        let job = {
            let q = st.queue.borrow();
            if i >= q.len() {
                break;
            }
            q[i]
        };
        let m = job_meta(rt, job);
        m.flags.set(m.flags.get() & !QUEUED);
        i += 1;
    }
    st.flush_index.set(-1);
    st.queue.borrow_mut().clear();
    flush_post_flush_cbs(rt)?;
    st.current_flush.set(None);
    Ok(!st.queue.borrow().is_empty() || !st.pending_post.borrow().is_empty())
}

/// `flushJobs(seen)`. Vue's recursive call in `finally` becomes a loop, so deep flush chains use no Rust stack. The
/// outcome matches the nested form: a throw from a `finally` wins, and otherwise the innermost body error wins.
pub fn flush_jobs(rt: &Rt) -> Res<()> {
    let mut bodies: Vec<Res<()>> = Vec::new();
    loop {
        bodies.push(flush_body(rt));
        match flush_finally(rt) {
            Err(e) => return Err(e),
            Ok(true) => continue,
            Ok(false) => break,
        }
    }
    let mut result: Res<()> = Ok(());
    for b in bodies.into_iter().rev() {
        if result.is_ok() {
            result = b;
        }
    }
    result
}

/// `nextTick(fn)` with `this` (a truthy `this` binds `fn`, as in Vue): `fn ? p.then(fn) : p`, where
/// `p = currentFlushPromise || resolvedPromise`.
pub fn next_tick(rt: &Rt, this: &Value, f: &Value) -> Res<Value> {
    let p = match sched(rt).current_flush.get() {
        Some(p) => p,
        None => resolved_promise(rt)?,
    };
    let pv = Value::Object(p);
    if !f.to_boolean() {
        return Ok(pv);
    }
    let cb = if this.to_boolean() {
        let bind = rt.get_str(f, "bind")?;
        rt.call(&bind, f, &[this.clone()])?
    } else {
        f.clone()
    };
    let then = rt.get_str(&pv, "then")?;
    rt.call(&then, &pv, &[cb])
}

fn next_tick_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    next_tick(rt, c.this, &c.arg(0))
}

/// The scheduler's exports of the `vue` module.
pub fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    vec![("nextTick", Value::Object(rt.new_native_function(next_tick_code, "nextTick", 1, Vec::new())))]
}
