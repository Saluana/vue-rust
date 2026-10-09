//! vn-runtime::directives: `invokeDirectiveHook` and `withDirectives` of @vue/runtime-core 3.5.43 (production build).

use vn_value::{ObjRef, Res, Value};

use crate::errors::{call_with_error_handling, ErrType};
use crate::normalize::array_get;
use crate::renderer::{array_len, dirs_of, el_of, pause_tracking, reset_tracking, Renderer};

impl Renderer {
    /// `invokeDirectiveHook(vnode, prevVNode, instance, name)`: copies the old binding values, then calls the hook
    /// `name` of every binding with `(el, binding, vnode, prevVNode)` under error handling and tracking paused.
    pub(crate) fn invoke_directive_hook(&self, vnode: ObjRef, prev: Option<ObjRef>, instance: Option<ObjRef>, name: &str) -> Res<()> {
        let rt = self.rt();
        let bindings = dirs_of(rt, vnode);
        let old_bindings = match prev {
            Some(p) => {
                let d = dirs_of(rt, p);
                if d.to_boolean() { Some(d) } else { None }
            }
            None => None,
        };
        let n = array_len(rt, &bindings)?;
        for i in 0..n {
            let binding = array_get(rt, &bindings, i)?;
            if let Some(ob) = &old_bindings {
                let old = array_get(rt, ob, i)?;
                let ov = rt.get_str(&old, "value")?;
                rt.set_str(&binding, "oldValue", ov)?;
            }
            let dir = rt.get_str(&binding, "dir")?;
            let hook = rt.get_str(&dir, name)?;
            if hook.to_boolean() {
                let el = match el_of(rt, vnode) {
                    Some(n) => self.ctx.handle(n),
                    None => Value::Null,
                };
                let args = [el, binding.clone(), Value::Object(vnode), prev.map_or(Value::Null, Value::Object)];
                pause_tracking(rt)?;
                let r = call_with_error_handling(rt, &hook, instance, ErrType::Code(8), Some(&args));
                reset_tracking(rt)?;
                r?;
            }
        }
        Ok(())
    }
}
