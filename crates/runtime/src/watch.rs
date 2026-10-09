//! vn-runtime::watch: `watch`, `watchEffect`, `watchPostEffect` and `watchSyncEffect` (runtime-core 3.5.43 `doWatch`,
//! production build) over `vn_reactivity::watch_base`. Behaviour per @vue/runtime-core 3.5.43.
//!
//! Scheduling follows `flush`. 'pre' (the default) queues the job with the instance uid as its id and the PRE flag, so
//! PRE jobs of one instance run before its update. 'post' goes through `queuePostRenderEffect`. 'sync' runs inside the
//! write. The callback, getter and cleanups go through `callWithAsyncErrorHandling` of the instance that was current when
//! the watcher was created, with the WATCH_GETTER, WATCH_CALLBACK and WATCH_CLEANUP types.
//!
//! The SSR branch of doWatch is not ported (no SSR). Deviation: `options` must be undefined or an object (null throws
//! TypeError, as the destructuring in Vue does).

use vn_reactivity::{watch_base, WatchOptions};
use vn_value::{Call, JsStr, ObjRef, PropKey, Res, Rt, Value};

use crate::errors::{call_with_async_error_handling, ErrType};
use crate::instance::{current_instance, instance_of};
use crate::scheduler::{job_meta, queue_job, queue_post_render_effect, ALLOW_RECURSE, PRE};

/// The options that `doWatch` reads, with the `flush` mode resolved from the caller.
struct WatchOpts {
    immediate: Value,
    deep: Value,
    flush: Value,
    once: Value,
    scheduler: Value,
}

impl WatchOpts {
    fn none() -> WatchOpts {
        WatchOpts {
            immediate: Value::Undefined,
            deep: Value::Undefined,
            flush: Value::Undefined,
            once: Value::Undefined,
            scheduler: Value::Undefined,
        }
    }

    fn flush(mode: &str) -> WatchOpts {
        WatchOpts { flush: Value::String(JsStr::from_str(mode)), ..WatchOpts::none() }
    }
}

fn read_opts(rt: &Rt, options: &Value) -> Res<WatchOpts> {
    match options {
        Value::Undefined => Ok(WatchOpts::none()),
        Value::Null => Err(rt.throw_type_error("Cannot destructure options: it is null")),
        _ => Ok(WatchOpts {
            immediate: rt.get_str(options, "immediate")?,
            deep: rt.get_str(options, "deep")?,
            flush: rt.get_str(options, "flush")?,
            once: rt.get_str(options, "once")?,
            scheduler: rt.get_str(options, "scheduler")?,
        }),
    }
}

fn is_flush(v: &Value, mode: &str) -> bool {
    matches!(v, Value::String(s) if s.to_rust_string_lossy() == mode)
}

/// `doWatch(source, cb, options)`: builds the reactivity watcher with runtime-core's scheduler, augmentJob and call hooks.
fn do_watch(rt: &Rt, source: &Value, cb: &Value, opts: WatchOpts) -> Res<Value> {
    let inst = current_instance(rt);
    let inst_v = inst.map(Value::Object).unwrap_or(Value::Undefined);
    let is_post = is_flush(&opts.flush, "post");
    let is_sync = is_flush(&opts.flush, "sync");
    let is_pre = !is_post && !is_sync;
    let scheduler = if is_post {
        Value::Object(rt.new_native_function(post_scheduler_code, "", 1, vec![inst_v.clone()]))
    } else if is_sync {
        opts.scheduler.clone()
    } else {
        Value::Object(rt.new_native_function(pre_scheduler_code, "", 2, Vec::new()))
    };
    let augment = rt.new_native_function(
        augment_job_code,
        "",
        1,
        vec![Value::Bool(cb.to_boolean()), Value::Bool(is_pre), inst_v.clone()],
    );
    let call = rt.new_native_function(call_code, "", 3, vec![inst_v]);
    let wopts = WatchOptions {
        immediate: opts.immediate,
        deep: opts.deep,
        once: opts.once,
        scheduler,
        augment_job: Value::Object(augment),
        call: Value::Object(call),
    };
    let handle = watch_base(rt, source.clone(), Some(cb.clone()), wopts)?;
    Ok(handle.handle)
}

fn array_values(rt: &Rt, arr: &Value) -> Res<Vec<Value>> {
    let len = rt.to_number(&rt.get_str(arr, "length")?)? as u32;
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len {
        out.push(rt.get_v(arr, &PropKey::Index(i))?);
    }
    Ok(out)
}

/// `call(fn, type, args)` = `callWithAsyncErrorHandling(fn, instance, type, args)`. The instance is in the function data.
fn call_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let inst: Option<ObjRef> = d[0].as_object();
    let ty = ErrType::from_value(&c.arg(1)).ok_or_else(|| rt.throw_type_error("unsupported error type"))?;
    let args_v = c.arg(2);
    let args: Option<Vec<Value>> = if args_v.is_nullish() { None } else { Some(array_values(rt, &args_v)?) };
    call_with_async_error_handling(rt, &c.arg(0), inst, ty, args.as_deref())
}

/// The 'pre' scheduler: `(job, isFirstRun) => isFirstRun ? job() : queueJob(job)`.
fn pre_scheduler_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let job = c.arg(0);
    if c.arg(1).to_boolean() {
        rt.call(&job, &Value::Undefined, &[])?;
    } else {
        let Value::Object(o) = job else {
            return Err(rt.throw_type_error("queued job is not a function"));
        };
        queue_job(rt, o)?;
    }
    Ok(Value::Undefined)
}

/// The 'post' scheduler: `(job) => queuePostRenderEffect(job, instance && instance.suspense)`.
fn post_scheduler_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let suspense = match d[0].as_object() {
        Some(i) => instance_of(rt, i).suspense.borrow().clone(),
        None => Value::Undefined,
    };
    queue_post_render_effect(rt, &c.arg(0), &suspense)?;
    Ok(Value::Undefined)
}

/// `augmentJob(job)`: ALLOW_RECURSE when there is a callback; for 'pre' jobs PRE, the instance uid as id, and the instance.
fn augment_job_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let Value::Object(job) = c.arg(0) else {
        return Err(rt.throw_type_error("watch job is not an object"));
    };
    let m = job_meta(rt, job);
    if d[0].to_boolean() {
        m.flags.set(m.flags.get() | ALLOW_RECURSE);
    }
    if d[1].to_boolean() {
        m.flags.set(m.flags.get() | PRE);
        if let Some(inst) = d[2].as_object() {
            m.id.set(Some(instance_of(rt, inst).uid));
            m.instance.set(Some(inst));
        }
    }
    Ok(Value::Undefined)
}

fn watch_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let opts = read_opts(rt, &c.arg(2))?;
    do_watch(rt, &c.arg(0), &c.arg(1), opts)
}

fn watch_effect_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let opts = read_opts(rt, &c.arg(1))?;
    do_watch(rt, &c.arg(0), &Value::Null, opts)
}

/// `watchPostEffect(effect)`: `doWatch(effect, null, {flush: 'post'})`, so its options argument is ignored.
fn watch_post_effect_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    do_watch(rt, &c.arg(0), &Value::Null, WatchOpts::flush("post"))
}

/// `watchSyncEffect(effect)`: `doWatch(effect, null, {flush: 'sync'})`.
fn watch_sync_effect_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    do_watch(rt, &c.arg(0), &Value::Null, WatchOpts::flush("sync"))
}

/// The exports of the `vue` module that this module owns.
pub fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    vec![
        ("watch", Value::Object(rt.new_native_function(watch_code, "watch", 3, Vec::new()))),
        ("watchEffect", Value::Object(rt.new_native_function(watch_effect_code, "watchEffect", 2, Vec::new()))),
        (
            "watchPostEffect",
            Value::Object(rt.new_native_function(watch_post_effect_code, "watchPostEffect", 2, Vec::new())),
        ),
        (
            "watchSyncEffect",
            Value::Object(rt.new_native_function(watch_sync_effect_code, "watchSyncEffect", 2, Vec::new())),
        ),
    ]
}
