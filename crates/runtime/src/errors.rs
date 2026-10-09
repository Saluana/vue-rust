//! vn-runtime::errors: `callWithErrorHandling`, `callWithAsyncErrorHandling`, `handleError` and `logError` of runtime-core
//! 3.5.43 (production build). Behaviour per @vue/runtime-core 3.5.43.
//!
//! An error type is `ErrType::Code(n)` (Vue's numeric `ErrorCodes`, e.g. WATCH_GETTER = 2) or `ErrType::Hook(kind)` (a
//! lifecycle list name such as "bm"). The error info string is `https://vuejs.org/error-reference/#runtime-<type>`.
//!
//! `log_error` in production hands the error to the realm's `ErrorSink` (default: stderr) without rethrowing. It rethrows
//! only when the app config sets `throwUnhandledErrorInProduction` (Vue's `logError` does the same).
//! A `false` return from an `errorCaptured` hook stops the propagation. Errors thrown by an `errorCaptured` hook itself are
//! not caught here: the injected wrapper (lifecycle.rs) routes them, as in Vue.

use std::rc::Rc;

use vn_value::{strict_eq, Call, JsStr, NativeObject, ObjRef, PropKey, Res, Rt, Throw, Tracer, Value};

use crate::instance::{as_instance, inst_state, instance_of, shared, LifeHook, Shared};

/// Numeric error codes of runtime-core (`ErrorCodes`).
pub mod code {
    pub const SETUP_FUNCTION: u32 = 0;
    pub const RENDER_FUNCTION: u32 = 1;
    pub const WATCH_GETTER: u32 = 2;
    pub const WATCH_CALLBACK: u32 = 3;
    pub const WATCH_CLEANUP: u32 = 4;
    pub const NATIVE_EVENT_HANDLER: u32 = 5;
    pub const COMPONENT_EVENT_HANDLER: u32 = 6;
    pub const VNODE_HOOK: u32 = 7;
    pub const DIRECTIVE_HOOK: u32 = 8;
    pub const TRANSITION_HOOK: u32 = 9;
    pub const APP_ERROR_HANDLER: u32 = 10;
    pub const APP_WARN_HANDLER: u32 = 11;
    pub const FUNCTION_REF: u32 = 12;
    pub const ASYNC_COMPONENT_LOADER: u32 = 13;
    pub const SCHEDULER: u32 = 14;
    pub const COMPONENT_UPDATE: u32 = 15;
    pub const APP_UNMOUNT_CLEANUP: u32 = 16;
}

/// The description table (`ErrorTypeStrings`) for a numeric code or a hook list name.
pub fn type_name(ty: ErrType) -> &'static str {
    match ty {
        ErrType::Code(c) => match c {
            0 => "setup function",
            1 => "render function",
            2 => "watcher getter",
            3 => "watcher callback",
            4 => "watcher cleanup function",
            5 => "native event handler",
            6 => "component event handler",
            7 => "vnode hook",
            8 => "directive hook",
            9 => "transition hook",
            10 => "app errorHandler",
            11 => "app warnHandler",
            12 => "ref function",
            13 => "async component loader",
            14 => "scheduler flush",
            15 => "component update",
            16 => "app unmount cleanup function",
            _ => "unknown error",
        },
        ErrType::Hook(h) => match h {
            LifeHook::Sp => "serverPrefetch hook",
            LifeHook::Bc => "beforeCreate hook",
            LifeHook::C => "created hook",
            LifeHook::Bm => "beforeMount hook",
            LifeHook::M => "mounted hook",
            LifeHook::Bu => "beforeUpdate hook",
            LifeHook::U => "updated",
            LifeHook::Bum => "beforeUnmount hook",
            LifeHook::Um => "unmounted hook",
            LifeHook::A => "activated hook",
            LifeHook::Da => "deactivated hook",
            LifeHook::Ec => "errorCaptured hook",
            LifeHook::Rtc => "renderTracked hook",
            LifeHook::Rtg => "renderTriggered hook",
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrType {
    Code(u32),
    Hook(LifeHook),
}

impl ErrType {
    /// The `type` argument as Vue passes it: a number for codes, the list name for hooks.
    pub fn to_value(self) -> Value {
        match self {
            ErrType::Code(c) => Value::Number(c as f64),
            ErrType::Hook(h) => Value::String(JsStr::from_str(h.key())),
        }
    }

    /// Reads a `type` argument back. Non-integer or unknown types are `None`.
    pub fn from_value(v: &Value) -> Option<ErrType> {
        match v {
            Value::Number(n) if *n >= 0.0 && n.fract() == 0.0 && *n <= u32::MAX as f64 => Some(ErrType::Code(*n as u32)),
            Value::String(s) => LifeHook::from_key(&s.to_rust_string_lossy()).map(ErrType::Hook),
            _ => None,
        }
    }

    /// `https://vuejs.org/error-reference/#runtime-${type}`.
    fn info(self) -> String {
        match self {
            ErrType::Code(c) => format!("https://vuejs.org/error-reference/#runtime-{c}"),
            ErrType::Hook(h) => format!("https://vuejs.org/error-reference/#runtime-{}", h.key()),
        }
    }
}

/// Receives errors that reach `logError` in production (`console.error` in Vue). Installed per realm.
pub trait ErrorSink {
    fn report(&self, rt: &Rt, err: &Value);
}

/// The default sink: one line on stderr.
pub struct StderrSink;

impl ErrorSink for StderrSink {
    fn report(&self, rt: &Rt, err: &Value) {
        let text = match rt.to_string(err) {
            Ok(s) => s.to_rust_string_lossy(),
            Err(_) => "<unprintable error>".to_string(),
        };
        eprintln!("[vn-runtime] {text}");
    }
}

pub(crate) struct ErrState {
    pub(crate) sink: std::cell::RefCell<Rc<dyn ErrorSink>>,
}

impl NativeObject for ErrState {
    fn trace(&self, _t: &mut Tracer) {}
}

fn err_state(rt: &Rt) -> Shared<ErrState> {
    shared(rt, "vn-runtime.errors", |_| ErrState { sink: std::cell::RefCell::new(Rc::new(StderrSink)) })
}

/// Replaces the realm's error sink. Returns the previous one.
pub fn set_error_sink(rt: &Rt, sink: Rc<dyn ErrorSink>) -> Rc<dyn ErrorSink> {
    let st = err_state(rt);
    st.sink.replace(sink)
}

/// `callWithErrorHandling(fn, instance, type, args)`: calls `f` with `args` (none when `args` is None). A throw goes to
/// `handle_error`, and the call then yields undefined.
pub fn call_with_error_handling(rt: &Rt, f: &Value, inst: Option<ObjRef>, ty: ErrType, args: Option<&[Value]>) -> Res<Value> {
    let r = rt.call(f, &Value::Undefined, args.unwrap_or(&[]));
    match r {
        Ok(v) => Ok(v),
        Err(Throw(e)) => {
            handle_error(rt, &e, inst, ty)?;
            Ok(Value::Undefined)
        }
    }
}

/// `isPromise(val)`: an object or function with callable `then` and `catch`.
pub fn is_promise(rt: &Rt, v: &Value) -> Res<bool> {
    let Value::Object(_) = v else { return Ok(false) };
    let then = rt.get_str(v, "then")?;
    if !rt.is_callable(&then) {
        return Ok(false);
    }
    let catch = rt.get_str(v, "catch")?;
    Ok(rt.is_callable(&catch))
}

fn catch_handler_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let inst = d[0].as_object();
    let ty = ErrType::from_value(&d[1]).expect("error type of a catch handler (framework bug)");
    handle_error(rt, &c.arg(0), inst, ty)?;
    Ok(Value::Undefined)
}

/// `callWithAsyncErrorHandling(fn, instance, type, args)`. A function is called with error handling; a promise result gets
/// `.catch(err => handleError(err, instance, type))`. An array maps over its elements, and anything else yields undefined.
pub fn call_with_async_error_handling(
    rt: &Rt,
    f: &Value,
    inst: Option<ObjRef>,
    ty: ErrType,
    args: Option<&[Value]>,
) -> Res<Value> {
    if rt.is_callable(f) {
        let res = call_with_error_handling(rt, f, inst, ty, args)?;
        if res.to_boolean() && is_promise(rt, &res)? {
            let handler = rt.new_native_function(
                catch_handler_code,
                "",
                1,
                vec![inst.map(Value::Object).unwrap_or(Value::Undefined), ty.to_value()],
            );
            let catch = rt.get_str(&res, "catch")?;
            rt.call(&catch, &res, &[Value::Object(handler)])?;
        }
        return Ok(res);
    }
    if rt.is_array(f) {
        let len = rt.to_number(&rt.get_str(f, "length")?)? as u32;
        let mut out = Vec::with_capacity(len as usize);
        for i in 0..len {
            let item = rt.get_v(f, &PropKey::Index(i))?;
            out.push(call_with_async_error_handling(rt, &item, inst, ty, args)?);
        }
        return Ok(Value::Object(rt.new_array(out)));
    }
    Ok(Value::Undefined)
}

/// `(config.errorHandler, config.throwUnhandledErrorInProduction)` of an instance's app context.
fn app_config(rt: &Rt, inst: ObjRef) -> Res<(Value, Value)> {
    let ac = instance_of(rt, inst).app_context.borrow().clone();
    let config = rt.get_str(&ac, "config")?;
    if !config.to_boolean() {
        return Ok((Value::Undefined, Value::Undefined));
    }
    Ok((rt.get_str(&config, "errorHandler")?, rt.get_str(&config, "throwUnhandledErrorInProduction")?))
}

/// `handleError(err, instance, type)`: the `errorCaptured` hooks of the parent chain first, then the app's `errorHandler`,
/// then `log_error`.
pub fn handle_error(rt: &Rt, err: &Value, inst: Option<ObjRef>, ty: ErrType) -> Res<()> {
    let (error_handler, throw_prod) = match inst {
        Some(i) => app_config(rt, i)?,
        None => (Value::Undefined, Value::Undefined),
    };
    if let Some(i) = inst {
        let rec = instance_of(rt, i);
        let exposed = rec.proxy.borrow().clone();
        let info = Value::String(JsStr::from_str(&ty.info()));
        let mut cur = rec.parent.borrow().as_object();
        while let Some(c) = cur {
            let Some(crec) = as_instance(rt, c) else { break };
            let mut k = 0;
            loop {
                let hook = {
                    let list = crec.hooks[LifeHook::Ec as usize].borrow();
                    if k >= list.len() {
                        break;
                    }
                    list[k].clone()
                };
                let r = rt.call(&hook, &Value::Undefined, &[err.clone(), exposed.clone(), info.clone()])?;
                if strict_eq(&r, &Value::Bool(false)) {
                    return Ok(());
                }
                k += 1;
            }
            cur = crec.parent.borrow().as_object();
        }
        if error_handler.to_boolean() {
            let st = inst_state(rt);
            rt.call(&st.pause_tracking, &Value::Undefined, &[])?;
            call_with_error_handling(
                rt,
                &error_handler,
                None,
                ErrType::Code(code::APP_ERROR_HANDLER),
                Some(&[err.clone(), exposed, info]),
            )?;
            rt.call(&st.reset_tracking, &Value::Undefined, &[])?;
            return Ok(());
        }
    }
    log_error(rt, err, &throw_prod)
}

/// `logError(err, type, contextVNode, throwInDev, throwInProd)` in production: rethrows when `throw_in_prod` is truthy,
/// otherwise reports to the sink.
pub fn log_error(rt: &Rt, err: &Value, throw_in_prod: &Value) -> Res<()> {
    if throw_in_prod.to_boolean() {
        return Err(Throw(err.clone()));
    }
    let sink = err_state(rt).sink.borrow().clone();
    sink.report(rt, err);
    Ok(())
}
