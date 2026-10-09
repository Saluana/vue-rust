//! vn-runtime::exports: the runtime's `vue` module. `all` collects every module's exports under the names Vue's `vue`
//! package uses; `register_vue_module` registers them as the native module "vue" (ABI.md section 14), and
//! `install_vue_global` installs them as the global `Vue` namespace that the builtin corpora resolve `Vue.<name>` through.

use vn_value::{Flags, ObjRef, PropKey, Rt, Value};

/// Every export of the runtime's `vue` module. Each native export carries the realm's vnode state as its data; the
/// state is shared with the renderer (`renderer::vnode_state`), so every call returns exports of the same realm state.
pub fn all(rt: &Rt) -> Vec<(&'static str, Value)> {
    let state = crate::renderer::vnode_state(rt);
    let mut out = crate::normalize::exports(rt);
    out.extend(crate::vnode::exports(rt, state));
    out.extend(crate::helpers::exports(rt, state));
    out.extend(vn_reactivity::exports(rt));
    out.extend(crate::scheduler::exports(rt));
    out.extend(crate::watch::exports(rt));
    out.extend(crate::lifecycle::exports(rt));
    out.extend(crate::inject::exports(rt));
    out.extend(crate::dom_events::exports(rt));
    out.extend(crate::vmodel::exports(rt));
    out.extend(crate::vshow::exports(rt));
    out.extend(crate::setup_helpers::exports(rt));
    out.extend(crate::app::exports(rt));
    out.extend(crate::teleport::exports(rt));
    out.extend(crate::keep_alive::exports(rt));
    out.extend(crate::async_component::exports(rt));
    out.extend(crate::transition::exports(rt));
    out
}

/// Registers the native module "vue" with `all(rt)`. Panics when the realm already has a "vue" module (framework bug).
pub fn register_vue_module(rt: &Rt) -> ObjRef {
    rt.register_native_module("vue", all(rt))
}

/// Installs the global `Vue` namespace object holding `all(rt)`.
pub fn install_vue_global(rt: &Rt) {
    let ns = rt.new_object();
    for (name, v) in all(rt) {
        rt.define_data(ns, PropKey::from_str(name), v, Flags::ALL).expect("fresh namespace object (framework bug)");
    }
    rt.install_global("Vue", Value::Object(ns));
}
