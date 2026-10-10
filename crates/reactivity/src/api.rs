//! The JS surface of the crate: the exports the `vue` module owns for user code (`exports`), the internal helpers that
//! the tests and the runtime use (`internal_exports`), and the prototype setup run once per realm.

use vn_value::{JsStr, Res, Rt, Value};

use crate::computed;
use crate::effect;
use crate::refs;
use crate::scope;
use crate::shared::native;
use crate::state::{state, Protos};
use crate::target;
use crate::watch;

/// Prototypes of the JS-visible reactivity objects (run once per realm, from `state::init`).
pub(crate) fn install_protos(rt: &Rt) -> Protos {
    crate::reactive::install(rt);
    Protos {
        reference: Some(refs::install_ref_proto(rt)),
        custom: Some(refs::install_custom_proto(rt)),
        getter: Some(refs::install_getter_proto(rt)),
        prop: Some(refs::install_prop_proto(rt)),
        computed: Some(computed::install_proto(rt)),
        effect: Some(effect::install_proto(rt)),
        scope: Some(scope::install_proto(rt)),
    }
}

/// The `vue` exports this crate owns for user code: the names the manifest marks P (ref, shallowRef, triggerRef,
/// customRef, toRef, toRefs, toValue, unref, isRef, proxyRefs, computed, effectScope, getCurrentScope, onScopeDispose,
/// EffectScope, onWatcherCleanup, getCurrentWatcher, effect, stop, and the reactive family).
/// Created once per realm and cached, so the functions keep their identity and stay reachable.
pub fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    let cached = state(rt).exports.borrow().clone();
    if let Some(list) = cached {
        return list;
    }
    let proto = state(rt).protos.get().scope.expect("EffectScope prototype (framework bug)");
    let mut list = refs::exported(rt);
    list.push(computed::exported(rt));
    list.extend(scope::exported(rt, proto));
    list.extend(watch::exported_public(rt));
    list.extend(effect::exported_public(rt));
    list.extend(crate::reactive::exported(rt));
    *state(rt).exports.borrow_mut() = Some(list.clone());
    list
}

/// Every name of `exports` (the same function objects) plus the internal helpers: effectWithoutRun, onEffectCleanup, the
/// tracking stack, watch, traverse, track, trigger and the iterate keys. The test glue exposes the whole list under
/// `__internal`; the runtime looks up helpers by name.
pub fn internal_exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    let cached = state(rt).internal.borrow().clone();
    if let Some(list) = cached {
        return list;
    }
    let mut list = exports(rt);
    list.extend(effect::exported_internal(rt));
    list.extend(crate::reactive::internal(rt));
    list.extend(watch::exported_internal(rt));
    list.push(("track", Value::Object(native(rt, "track", 3, js_track, Vec::new()))));
    list.push(("trigger", Value::Object(native(rt, "trigger", 6, js_trigger, Vec::new()))));
    let keys = target::iterate_keys(rt);
    list.push(("ITERATE_KEY", keys[0].clone()));
    list.push(("MAP_KEY_ITERATE_KEY", keys[1].clone()));
    list.push(("ARRAY_ITERATE_KEY", keys[2].clone()));
    *state(rt).internal.borrow_mut() = Some(list.clone());
    list
}

/// `track(target, type, key)`. A primitive target is a TypeError when tracking (WeakMap key), otherwise a no-op.
fn js_track(rt: &Rt, c: &vn_value::Call<'_>) -> Res<Value> {
    let st = state(rt);
    let active = st.should_track.get() && st.active_sub.get().is_some();
    drop(st);
    match c.arg(0) {
        Value::Object(o) => target::track(rt, o, &c.arg(2))?,
        _ if active => return Err(rt.throw_type_error("Invalid value used as weak map key")),
        _ => {}
    }
    Ok(Value::Undefined)
}

/// `trigger(target, type, key, newValue, oldValue, oldTarget)`. An unknown type runs no operation-specific triggers.
fn js_trigger(rt: &Rt, c: &vn_value::Call<'_>) -> Res<Value> {
    let op = match c.arg(1) {
        Value::String(s) if s == JsStr::from_str("set") => target::TriggerOp::Set,
        Value::String(s) if s == JsStr::from_str("add") => target::TriggerOp::Add,
        Value::String(s) if s == JsStr::from_str("delete") => target::TriggerOp::Delete,
        Value::String(s) if s == JsStr::from_str("clear") => target::TriggerOp::Clear,
        _ => target::TriggerOp::Other,
    };
    match c.arg(0) {
        Value::Object(o) => target::trigger(rt, o, op, &c.arg(2), &c.arg(3))?,
        _ => {
            let st = state(rt);
            st.global_version.set(st.global_version.get() + 1);
        }
    }
    Ok(Value::Undefined)
}
