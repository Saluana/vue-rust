//! Test glue. `register_vue_for_tests` registers the native module `"vue"` on a realm: the owned exports as module
//! exports, plus `__internal` (an object holding the internal helpers). The harness calls it once per realm.

use vn_value::{Flags, ObjRef, PropKey, Rt, Value};

pub fn register_vue_for_tests(rt: &Rt) -> ObjRef {
    let mut list = crate::exports(rt);
    let internal = crate::internal_exports(rt);
    let obj = rt.new_object();
    for (name, v) in internal {
        rt.define_data(obj, PropKey::from_str(name), v, Flags::ALL).expect("__internal export (framework bug)");
    }
    list.push(("__internal", Value::Object(obj)));
    rt.register_native_module("vue", list)
}
