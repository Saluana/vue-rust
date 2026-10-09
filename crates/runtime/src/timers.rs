//! Host-driven realm timers. Hosts fire opaque ids; pending JS callbacks are traced by the realm.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use vn_value::{payload_ref, Call, NativeObject, Res, Rt, Tracer, Value};

pub trait TimerDriver {
    fn schedule(&self, id: u32, delay_ms: f64);
    fn cancel(&self, id: u32);
}

struct Timers {
    driver: Rc<dyn TimerDriver>,
    next: Cell<u32>,
    callbacks: RefCell<HashMap<u32, (Value, Vec<Value>)>>,
}

impl NativeObject for Timers {
    fn trace(&self, t: &mut Tracer) {
        for (f, args) in self.callbacks.borrow().values() {
            t.mark_value(f);
            for value in args {
                t.mark_value(value);
            }
        }
    }
}

impl Drop for Timers {
    fn drop(&mut self) {
        for id in self.callbacks.get_mut().keys() {
            self.driver.cancel(*id);
        }
    }
}

const KEY: &str = "vn-runtime.timers";
/// Installs browser-compatible timer globals for a production host.
pub fn install(rt: &Rt, driver: Rc<dyn TimerDriver>) {
    let o = rt.new_native_object(None, Rc::new(Timers {
        driver,
        next: Cell::new(1),
        callbacks: RefCell::new(HashMap::new()),
    }));
    rt.register_builtin(KEY, o);
    for (name, code, len) in [("setTimeout", set_timeout as vn_value::CodeFn, 2), ("clearTimeout", clear_timeout, 1), ("requestAnimationFrame", animation_frame, 1), ("cancelAnimationFrame", clear_timeout, 1)] {
        rt.install_global(name, Value::Object(rt.new_native_function(code, name, len, vec![Value::Object(o)])));
    }
}

fn schedule(rt: &Rt, c: &Call<'_>, frame: bool) -> Res<Value> {
    let callback = c.arg(0);
    if !rt.is_callable(&callback) { return Err(rt.throw_type_error("timer callback must be a function")); }
    let o = rt.fn_data(c.callee)[0].as_object().expect("timer state");
    let payload = rt.native_payload(o).expect("timer state");
    let p = payload_ref::<Timers>(&*payload).expect("timer state type");
    let delay = if frame { 16.0 } else { rt.to_number(&c.arg(1))? };
    let id = p.next.get();
    p.next.set(id.checked_add(1).ok_or_else(|| rt.throw_range_error("timer id limit"))?);
    let args = if frame {
        vec![Value::Number(crate::renderer::renderer(rt)?.host().now_ms() + delay)]
    } else {
        c.args.iter().skip(2).cloned().collect()
    };
    p.callbacks.borrow_mut().insert(id, (callback, args));
    p.driver.schedule(id, if delay.is_finite() { delay.max(0.0) } else { 0.0 });
    Ok(Value::Number(id as f64))
}

fn set_timeout(rt: &Rt, c: &Call<'_>) -> Res<Value> { schedule(rt, c, false) }

fn animation_frame(rt: &Rt, c: &Call<'_>) -> Res<Value> { schedule(rt, c, true) }

fn clear_timeout(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let id = rt.to_number(&c.arg(0))? as u32;
    let payload = rt.native_payload(rt.fn_data(c.callee)[0].as_object().expect("timer state")).expect("timer state");
    let p = payload_ref::<Timers>(&*payload).expect("timer state type");
    p.callbacks.borrow_mut().remove(&id);
    p.driver.cancel(id);
    Ok(Value::Undefined)
}
/// Called by a host when its scheduled timer fires. Removal precedes callback invocation to allow re-entrancy.
pub fn fire(rt: &Rt, id: u32) -> Res<()> {
    let Some(o) = rt.builtin(KEY) else { return Err(rt.throw_type_error("timer driver is not installed")); };
    let payload = rt.native_payload(o).expect("timer state");
    let p = payload_ref::<Timers>(&*payload).expect("timer state type");
    let callback = p.callbacks.borrow_mut().remove(&id);
    if let Some((f, args)) = callback {
        rt.call(&f, &Value::Undefined, &args)?;
    }
    Ok(())
}

pub(crate) fn timeout(rt: &Rt, callback: Value, delay: Value) -> Res<Value> {
    let global = Value::Object(rt.intrinsic(vn_value::Intrinsic::Global));
    let f = rt.get_str(&global, "setTimeout")?;
    if !rt.is_callable(&f) { return Err(rt.throw_type_error("this host has no timer driver")); }
    rt.call(&f, &Value::Undefined, &[callback, delay])
}

pub(crate) fn cancel(rt: &Rt, id: &Value) -> Res<()> {
    if id.is_nullish() { return Ok(()); }
    let global = Value::Object(rt.intrinsic(vn_value::Intrinsic::Global));
    rt.call(&rt.get_str(&global, "clearTimeout")?, &Value::Undefined, &[id.clone()])?;
    Ok(())
}
