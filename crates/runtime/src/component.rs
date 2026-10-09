//! vn-runtime::component: component vnodes of @vue/runtime-core 3.5.43 (production build): mountComponent,
//! setupComponent (setupStatefulComponent, handleSetupResult, finishComponentSetup), setupRenderEffect with its
//! ReactiveEffect, updateComponent, shouldUpdateComponent, updateComponentPreRender, renderComponentRoot (attrs
//! fallthrough included) and unmountComponent.
//!
//! The render effect is made through the reactivity `effect` export. Its first run is deferred: the function it runs
//! does nothing until `RenderEffect::armed` is set. `toggleRecurse` sets ALLOW_RECURSE on the job; the effect's own flag
//! is not reachable from here (deviation: a component does not re-run for a change made by its own render).
//! Unsupported (throw TypeError): async setup, Suspense, KeepAlive, Transition, hydration, the options API (except
//! components and directives), functional components with Transition.

use std::cell::Cell;
use std::rc::Rc;

use vn_value::{payload_ref, Call, Flags, NativeObject, ObjRef, PropKey, Res, Rt, Throw, Tracer, Value};

use crate::app::empty_app_context;
use crate::dom_props::enumerable_keys;
use crate::emits::{emit_fn, is_emit_listener, normalize_emits_options};
use crate::errors::{call_with_error_handling, handle_error, is_promise, ErrType};
use crate::host::{HostNode, HostNs};
use crate::instance::{create_component_instance, enter_instance, instance_of, leave_instance, set_current_rendering_instance, LifeHook};
use crate::normalize::{array_get, for_in_keys, length_of};
use crate::props::{has_prop_value_changed, has_props_changed, init_props, normalize_props_options, reactivity_fn, update_props};
use crate::public_instance::{attrs_proxy, new_public_proxy};
use crate::renderer::{
    children_of, component_of, dirs_of, dyn_props_of, el_of, key_string, patch_of, props_of, renderer, set_el, shape_of,
    transition_of, Renderer, Step,
};
use crate::scheduler::{flush_pre_flush_cbs, job_meta, queue_job, queue_post_render_effect, ALLOW_RECURSE, DISPOSED};
use crate::shapes::{PatchFlags, ShapeFlags};
use crate::slots::{init_slots, update_slots};
use crate::vnode::{self, fields};

/// The data of one component's render effect: the closure variables of `setupRenderEffect`.
pub(crate) struct RenderEffect {
    pub(crate) inst: ObjRef,
    pub(crate) initial: Cell<Option<ObjRef>>,
    pub(crate) container: Cell<Option<HostNode>>,
    pub(crate) anchor: Cell<Option<HostNode>>,
    pub(crate) ns: HostNs,
    pub(crate) optimized: bool,
}

impl NativeObject for RenderEffect {
    fn trace(&self, t: &mut Tracer) {
        t.mark(self.inst);
        if let Some(v) = self.initial.get() {
            t.mark(v);
        }
    }
}

/// ALLOW_RECURSE of a ReactiveEffect (the `flags` bit of @vue/reactivity's node.rs).
const EFFECT_ALLOW_RECURSE: u32 = 32;

const UNSUPPORTED_OPTIONS: &[&str] = &[
    "data",
    "computed",
    "methods",
    "watch",
    "provide",
    "inject",
    "mixins",
    "extends",
    "template",
    "beforeCreate",
    "created",
    "beforeMount",
    "mounted",
    "beforeUpdate",
    "updated",
    "beforeUnmount",
    "unmounted",
    "errorCaptured",
    "renderTracked",
    "renderTriggered",
    "activated",
    "deactivated",
    "serverPrefetch",
];

fn data_obj(rt: &Rt, c: &Call<'_>) -> ObjRef {
    match rt.fn_data(c.callee).first() {
        Some(Value::Object(o)) => *o,
        _ => panic!("component function without its instance (framework bug)"),
    }
}

fn as_obj(rt: &Rt, v: &Value, what: &str) -> Res<ObjRef> {
    match v {
        Value::Object(o) => Ok(*o),
        _ => Err(rt.throw_type_error(&format!("renderer: {what} is not a vnode or instance (framework bug)"))),
    }
}

fn call_method(rt: &Rt, obj: &Value, name: &str) -> Res<Value> {
    let f = rt.get_str(obj, name)?;
    rt.call(&f, obj, &[])
}

/// `createSetupContext(instance)` through the installed renderer (used by useAttrs).
pub(crate) fn create_setup_context_for(rt: &Rt, inst: ObjRef) -> Res<Value> {
    renderer(rt)?.create_setup_context(inst)
}

fn set_component(rt: &Rt, vnode: ObjRef, inst: ObjRef) {
    fields(rt, vnode, |f| f.component = Value::Object(inst));
}

fn hook_list(rt: &Rt, inst: ObjRef, h: LifeHook) -> Vec<Value> {
    instance_of(rt, inst).hooks[h as usize].borrow().clone()
}

fn hook_array(rt: &Rt, list: Vec<Value>) -> Value {
    Value::Object(rt.new_array(list))
}

/// `invokeArrayFns(fns)`: calls every hook in order, with no arguments.
pub(crate) fn invoke_array_fns(rt: &Rt, fns: &[Value]) -> Res<()> {
    for f in fns {
        rt.call(f, &Value::Undefined, &[])?;
    }
    Ok(())
}

fn noop_code(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

fn expose_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let inst = data_obj(rt, c);
    let v = c.args.first().cloned().unwrap_or(Value::Undefined);
    let exposed = if v.to_boolean() { v } else { Value::Object(rt.new_object()) };
    *instance_of(rt, inst).exposed.borrow_mut() = exposed;
    Ok(Value::Undefined)
}

/// The function of the render effect: the component update (`componentUpdateFn`).
fn render_effect_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let pobj = match rt.fn_data(c.callee).first() {
        Some(Value::Object(o)) => *o,
        _ => panic!("render effect without its payload (framework bug)"),
    };
    let p = rt.native_payload(pobj).expect("render effect payload (framework bug)");
    let re = payload_ref::<RenderEffect>(&*p).expect("render effect payload type (framework bug)");
    let r = renderer(rt)?;
    r.component_update(re)?;
    Ok(Value::Undefined)
}

/// `() => queueJob(job)`: the scheduler callback of the render effect, reading `instance.job` when it runs.
fn scheduler_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let inst = data_obj(rt, c);
    let job = instance_of(rt, inst).job.borrow().clone();
    if let Value::Object(j) = job {
        queue_job(rt, j)?;
    }
    Ok(Value::Undefined)
}

/// `runIfDirty` of the job: runs the update when the effect is dirty.
fn job_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let runner = rt.fn_data(c.callee).first().cloned().unwrap_or(Value::Undefined);
    let eff = rt.get_str(&runner, "effect")?;
    let dirty = rt.get_str(&eff, "dirty")?;
    if dirty.to_boolean() {
        rt.call(&runner, &Value::Undefined, &[])?;
    }
    Ok(Value::Undefined)
}

/// `setupStatefulComponent`'s arity test: `setup.length`.
fn arity(rt: &Rt, f: &Value) -> Res<f64> {
    let n = rt.get_str(f, "length")?;
    rt.to_number(&n)
}

fn parent_of_instance(rt: &Rt, inst: ObjRef) -> Option<ObjRef> {
    match &*instance_of(rt, inst).parent.borrow() {
        Value::Object(o) => Some(*o),
        _ => None,
    }
}

/// `applyOptions` for the supported subset: components and directives registries.
fn apply_options(rt: &Rt, inst: ObjRef) -> Res<()> {
    let rec = instance_of(rt, inst);
    let comp = rec.ty.borrow().clone();
    if rt.is_callable(&comp) {
        return Ok(());
    }
    for key in UNSUPPORTED_OPTIONS {
        if rt.get_str(&comp, key)?.to_boolean() {
            return Err(rt.throw_type_error(&format!("unsupported: options API `{key}` on a component")));
        }
    }
    let components = rt.get_str(&comp, "components")?;
    if components.to_boolean() {
        *rec.components.borrow_mut() = components;
    }
    let directives = rt.get_str(&comp, "directives")?;
    if directives.to_boolean() {
        *rec.directives.borrow_mut() = directives;
    }
    Ok(())
}

/// `filterModelListeners(attrs, props)`.
fn filter_model_listeners(rt: &Rt, attrs: &Value, props: &Value) -> Res<Value> {
    let res = rt.new_object();
    for key in for_in_keys(rt, attrs)? {
        let k = key_string(&key);
        let is_model = k.starts_with("onUpdate:");
        let keep = if is_model {
            let stripped = &k[9..];
            let has = match props {
                Value::Object(p) => rt.has_property(*p, &PropKey::from_str(stripped))?,
                _ => false,
            };
            !has
        } else {
            true
        };
        if keep {
            let v = rt.get_v(attrs, &key)?;
            rt.define_data(res, key, v, Flags::ALL)?;
        }
    }
    Ok(Value::Object(res))
}

/// `getFunctionalFallthrough(attrs)`: class, style and listeners of a functional component's attrs.
fn functional_fallthrough(rt: &Rt, attrs: &Value) -> Res<Option<Value>> {
    let mut res: Option<ObjRef> = None;
    for key in for_in_keys(rt, attrs)? {
        let k = key_string(&key);
        if k == "class" || k == "style" || crate::normalize::is_on(&vn_value::JsStr::from_str(&k)) {
            let r = match res {
                Some(r) => r,
                None => {
                    let r = rt.new_object();
                    res = Some(r);
                    r
                }
            };
            let v = rt.get_v(attrs, &key)?;
            rt.define_data(r, key, v, Flags::ALL)?;
        }
    }
    Ok(res.map(Value::Object))
}

impl Renderer {
    /// The vnode of a render result (`normalizeVNode`).
    fn normalize_result(&self, v: &Value) -> Res<ObjRef> {
        let rt = self.rt();
        self.with_vst(|st| vnode::normalize_vnode(rt, st, v))
    }

    /// `processComponent(n1, n2, ...)`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn process_component(
        &self,
        n1: Option<ObjRef>,
        n2: ObjRef,
        container: HostNode,
        anchor: Option<HostNode>,
        parent: Option<ObjRef>,
        ns: HostNs,
        ssi: &Value,
        optimized: bool,
    ) -> Res<()> {
        let rt = self.rt();
        fields(rt, n2, |f| f.slot_scope_ids = ssi.clone());
        match n1 {
            None => {
                if shape_of(rt, n2) & ShapeFlags::COMPONENT_KEPT_ALIVE != 0 {
                    return self.activate_cached(n2, container, anchor, ns, optimized);
                }
                self.mount_component(n2, container, anchor, parent, ns, optimized)
            }
            Some(old) => self.update_component(old, n2, optimized),
        }
    }

    /// `mountComponent(initialVNode, ...)`.
    fn mount_component(
        &self,
        initial: ObjRef,
        container: HostNode,
        anchor: Option<HostNode>,
        parent: Option<ObjRef>,
        ns: HostNs,
        optimized: bool,
    ) -> Res<()> {
        let rt = self.rt();
        let app_ctx = empty_app_context(rt)?;
        let inst = create_component_instance(rt, &Value::Object(initial), parent, &app_ctx)?;
        set_component(rt, initial, inst);
        let rec = instance_of(rt, inst);
        let comp = rec.ty.borrow().clone();
        let po = normalize_props_options(rt, &comp)?;
        *rec.props_options.borrow_mut() = po;
        let eo = normalize_emits_options(rt, &comp)?;
        *rec.emits_options.borrow_mut() = eo;
        *rec.emit.borrow_mut() = emit_fn(rt, inst);
        self.setup_component(inst, optimized)?;
        self.setup_render_effect(inst, initial, container, anchor, ns, optimized)
    }

    /// `setupComponent(instance, false, optimized)`.
    fn setup_component(&self, inst: ObjRef, optimized: bool) -> Res<()> {
        let rt = self.rt();
        let rec = instance_of(rt, inst);
        let vnode = as_obj(rt, &rec.vnode.borrow().clone(), "instance vnode")?;
        let props = props_of(rt, vnode);
        let children = children_of(rt, vnode);
        let is_stateful = shape_of(rt, vnode) & ShapeFlags::STATEFUL_COMPONENT != 0;
        init_props(rt, inst, &props, is_stateful)?;
        init_slots(rt, inst, &children, optimized)?;
        if is_stateful {
            self.setup_stateful(inst)
        } else {
            Ok(())
        }
    }

    /// `setupStatefulComponent(instance)`.
    fn setup_stateful(&self, inst: ObjRef) -> Res<()> {
        let rt = self.rt();
        let rec = instance_of(rt, inst);
        *rec.access_cache.borrow_mut() = Value::Object(rt.new_object_with_proto(None));
        let proxy = new_public_proxy(rt, inst);
        *rec.proxy.borrow_mut() = Value::Object(proxy);
        let comp = rec.ty.borrow().clone();
        let setup = rt.get_str(&comp, "setup")?;
        if !setup.to_boolean() {
            return self.finish_component_setup(inst);
        }
        crate::renderer::pause_tracking(rt)?;
        let setup_ctx = if arity(rt, &setup)? > 1.0 { self.create_setup_context(inst)? } else { Value::Null };
        *rec.setup_context.borrow_mut() = setup_ctx.clone();
        let prev = enter_instance(rt, inst)?;
        let props = rec.props.borrow().clone();
        let result = call_with_error_handling(rt, &setup, Some(inst), ErrType::Code(0), Some(&[props, setup_ctx]));
        let reset_tracking = crate::renderer::reset_tracking(rt);
        let leave = leave_instance(rt, inst, prev);
        let setup_result = result?;
        reset_tracking?;
        leave?;
        if is_promise(rt, &setup_result)? {
            return Err(rt.throw_type_error("unsupported: async setup()"));
        }
        self.handle_setup_result(inst, setup_result)
    }

    /// `createSetupContext(instance)`: `{ attrs, slots, emit, expose }`.
    pub(crate) fn create_setup_context(&self, inst: ObjRef) -> Res<Value> {
        let rt = self.rt();
        let rec = instance_of(rt, inst);
        let attrs = as_obj(rt, &rec.attrs.borrow().clone(), "attrs")?;
        let ctx = rt.new_object();
        let proxy = attrs_proxy(rt, attrs);
        rt.define_data(ctx, PropKey::from_str("attrs"), Value::Object(proxy), Flags::ALL)?;
        let slots = rec.slots.borrow().clone();
        rt.define_data(ctx, PropKey::from_str("slots"), slots, Flags::ALL)?;
        let emit = rec.emit.borrow().clone();
        rt.define_data(ctx, PropKey::from_str("emit"), emit, Flags::ALL)?;
        let expose = rt.new_native_function(expose_code, "expose", 1, vec![Value::Object(inst)]);
        rt.define_data(ctx, PropKey::from_str("expose"), Value::Object(expose), Flags::ALL)?;
        Ok(Value::Object(ctx))
    }

    /// `handleSetupResult(instance, setupResult)`.
    fn handle_setup_result(&self, inst: ObjRef, result: Value) -> Res<()> {
        let rt = self.rt();
        let rec = instance_of(rt, inst);
        if rt.is_callable(&result) {
            *rec.render.borrow_mut() = result;
        } else if let Value::Object(_) = result {
            let proxy_refs = reactivity_fn(rt, "proxyRefs");
            let state = rt.call(&proxy_refs, &Value::Undefined, std::slice::from_ref(&result))?;
            *rec.setup_state.borrow_mut() = state;
        }
        self.finish_component_setup(inst)
    }

    /// `finishComponentSetup(instance)`: the render function (the component's own or a no-op), then applyOptions.
    fn finish_component_setup(&self, inst: ObjRef) -> Res<()> {
        let rt = self.rt();
        let rec = instance_of(rt, inst);
        let has_render = rec.render.borrow().to_boolean();
        if !has_render {
            let comp = rec.ty.borrow().clone();
            let render = rt.get_str(&comp, "render")?;
            let r = if render.to_boolean() {
                render
            } else {
                Value::Object(rt.new_native_function(noop_code, "", 0, Vec::new()))
            };
            *rec.render.borrow_mut() = r;
        }
        apply_options(rt, inst)
    }

    /// `setupRenderEffect(instance, initialVNode, container, anchor, ...)`.
    fn setup_render_effect(
        &self,
        inst: ObjRef,
        initial: ObjRef,
        container: HostNode,
        anchor: Option<HostNode>,
        ns: HostNs,
        optimized: bool,
    ) -> Res<()> {
        let rt = self.rt();
        let re = RenderEffect {
            inst,
            initial: Cell::new(Some(initial)),
            container: Cell::new(Some(container)),
            anchor: Cell::new(anchor),
            ns,
            optimized,
        };
        let payload = rt.new_native_object(None, Rc::new(re));
        let update_fn = rt.new_native_function(render_effect_code, "", 0, vec![Value::Object(payload)]);
        let rec = instance_of(rt, inst);
        let scope = rec.scope.borrow().clone();
        call_method(rt, &scope, "on")?;
        let options = rt.new_object();
        let sched = rt.new_native_function(scheduler_code, "", 0, vec![Value::Object(inst)]);
        rt.define_data(options, PropKey::from_str("scheduler"), Value::Object(sched), Flags::ALL)?;
        rt.define_data(options, PropKey::from_str("lazy"), Value::Bool(true), Flags::ALL)?;
        let effect_fn = reactivity_fn(rt, "effect");
        let runner = rt.call(&effect_fn, &Value::Undefined, &[Value::Object(update_fn), Value::Object(options)]);
        let off = call_method(rt, &scope, "off");
        let runner = runner?;
        off?;
        let effect = rt.get_str(&runner, "effect")?;
        *rec.effect.borrow_mut() = effect;
        *rec.update.borrow_mut() = runner.clone();
        let job = rt.new_native_function(job_code, "", 0, vec![runner.clone()]);
        let meta = job_meta(rt, job);
        meta.instance.set(Some(inst));
        meta.id.set(Some(rec.uid));
        *rec.job.borrow_mut() = Value::Object(job);
        self.toggle_recurse(inst, true)?;
        rt.call(&runner, &Value::Undefined, &[])?;
        Ok(())
    }

    /// `toggleRecurse(instance, allowed)`: ALLOW_RECURSE on the effect (its `flags`) and on the job.
    fn toggle_recurse(&self, inst: ObjRef, allowed: bool) -> Res<()> {
        let rt = self.rt();
        let rec = instance_of(rt, inst);
        let job = rec.job.borrow().clone();
        if let Value::Object(j) = job {
            let meta = job_meta(rt, j);
            let f = meta.flags.get();
            meta.flags.set(if allowed { f | ALLOW_RECURSE } else { f & !ALLOW_RECURSE });
        }
        let effect = rec.effect.borrow().clone();
        if let Value::Object(_) = effect {
            let f = rt.get_str(&effect, "flags")?;
            let n = rt.to_number(&f)? as u32;
            let next = if allowed { n | EFFECT_ALLOW_RECURSE } else { n & !EFFECT_ALLOW_RECURSE };
            rt.set_str(&effect, "flags", Value::Number(next as f64))?;
        }
        Ok(())
    }

    /// The body of `componentUpdateFn`: the first mount, or the update of a mounted component.
    pub(crate) fn component_update(&self, re: &RenderEffect) -> Res<()> {
        let rt = self.rt();
        let inst = re.inst;
        let rec = instance_of(rt, inst);
        if !rec.is_mounted.get() {
            let initial = match re.initial.get() {
                Some(v) => v,
                None => return Err(rt.throw_type_error("renderer: initial vnode missing (framework bug)")),
            };
            let props = props_of(rt, initial);
            let parent = parent_of_instance(rt, inst);
            let bm = hook_list(rt, inst, LifeHook::Bm);
            self.toggle_recurse(inst, false)?;
            if !bm.is_empty() {
                invoke_array_fns(rt, &bm)?;
            }
            if props.to_boolean() {
                let hook = rt.get_str(&props, "onVnodeBeforeMount")?;
                if hook.to_boolean() {
                    self.invoke_vnode_hook(&hook, parent, initial, None)?;
                }
            }
            self.toggle_recurse(inst, true)?;
            let sub = self.render_component_root(inst)?;
            *rec.sub_tree.borrow_mut() = Value::Object(sub);
            let container = match re.container.get() {
                Some(c) => c,
                None => return Err(rt.throw_type_error("renderer: mount container missing (framework bug)")),
            };
            self.patch(None, sub, container, re.anchor.get(), Some(inst), re.ns, &Value::Null, false)?;
            set_el(rt, initial, el_of(rt, sub));
            let m = hook_list(rt, inst, LifeHook::M);
            if !m.is_empty() {
                queue_post_render_effect(rt, &hook_array(rt, m), &Value::Null)?;
            }
            if props.to_boolean() {
                let hook = rt.get_str(&props, "onVnodeMounted")?;
                if hook.to_boolean() {
                    self.queue_steps(vec![Step::Hook(hook, parent, initial, None)])?;
                }
            }
            if shape_of(rt, initial) & ShapeFlags::COMPONENT_SHOULD_KEEP_ALIVE != 0 {
                let a = hook_list(rt, inst, LifeHook::A);
                if !a.is_empty() { queue_post_render_effect(rt, &hook_array(rt, a), &Value::Null)?; }
            }
            rec.is_mounted.set(true);
            re.initial.set(None);
            re.container.set(None);
            re.anchor.set(None);
            return Ok(());
        }
        // update
        let next_v = rec.next.borrow().clone();
        let vnode = as_obj(rt, &rec.vnode.borrow().clone(), "instance vnode")?;
        let parent = parent_of_instance(rt, inst);
        let origin_is_null = matches!(next_v, Value::Null);
        self.toggle_recurse(inst, false)?;
        let next: ObjRef = match next_v {
            Value::Object(n) => {
                set_el(rt, n, el_of(rt, vnode));
                self.update_component_pre_render(inst, n, re.optimized)?;
                n
            }
            _ => vnode,
        };
        let bu = hook_list(rt, inst, LifeHook::Bu);
        if !bu.is_empty() {
            invoke_array_fns(rt, &bu)?;
        }
        let next_props = props_of(rt, next);
        if next_props.to_boolean() {
            let hook = rt.get_str(&next_props, "onVnodeBeforeUpdate")?;
            if hook.to_boolean() {
                self.invoke_vnode_hook(&hook, parent, next, Some(vnode))?;
            }
        }
        self.toggle_recurse(inst, true)?;
        let next_tree = self.render_component_root(inst)?;
        let prev_v = rec.sub_tree.borrow().clone();
        let prev_tree = as_obj(rt, &prev_v, "subtree")?;
        *rec.sub_tree.borrow_mut() = Value::Object(next_tree);
        let prev_el = self.need(el_of(rt, prev_tree), "subtree element")?;
        let container = self.need(self.parent_of(prev_el), "update container")?;
        let anchor = self.next_host_node(prev_tree)?;
        self.patch(Some(prev_tree), next_tree, container, anchor, Some(inst), re.ns, &Value::Null, false)?;
        set_el(rt, next, el_of(rt, next_tree));
        if origin_is_null {
            self.update_hoc_host_el(inst, el_of(rt, next_tree))?;
        }
        let u = hook_list(rt, inst, LifeHook::U);
        if !u.is_empty() {
            queue_post_render_effect(rt, &hook_array(rt, u), &Value::Null)?;
        }
        if next_props.to_boolean() {
            let hook = rt.get_str(&next_props, "onVnodeUpdated")?;
            if hook.to_boolean() {
                self.queue_steps(vec![Step::Hook(hook, parent, next, Some(vnode))])?;
            }
        }
        Ok(())
    }

    /// `updateHOCHostEl(instance, el)`: the root elements of the parents that render this component.
    fn update_hoc_host_el(&self, inst: ObjRef, el: Option<HostNode>) -> Res<()> {
        let rt = self.rt();
        let mut vnode = as_obj(rt, &instance_of(rt, inst).vnode.borrow().clone(), "instance vnode")?;
        let mut parent = parent_of_instance(rt, inst);
        while let Some(p) = parent {
            let prec = instance_of(rt, p);
            let root = as_obj(rt, &prec.sub_tree.borrow().clone(), "subtree")?;
            if root == vnode {
                let pv = as_obj(rt, &prec.vnode.borrow().clone(), "parent vnode")?;
                set_el(rt, pv, el);
                vnode = pv;
                parent = parent_of_instance(rt, p);
            } else {
                break;
            }
        }
        Ok(())
    }

    /// `shouldUpdateComponent(prevVNode, nextVNode, optimized)`.
    fn should_update_component(&self, prev: ObjRef, next: ObjRef, optimized: bool) -> Res<bool> {
        let rt = self.rt();
        let prev_props = props_of(rt, prev);
        let prev_children = children_of(rt, prev);
        let next_props = props_of(rt, next);
        let next_children = children_of(rt, next);
        let patch = patch_of(rt, next);
        let comp = component_of(rt, prev);
        let emits = match comp {
            Value::Object(c) => instance_of(rt, c).emits_options.borrow().clone(),
            _ => Value::Null,
        };
        if dirs_of(rt, next).to_boolean() || transition_of(rt, next).to_boolean() {
            return Ok(true);
        }
        if optimized && patch >= 0 {
            if patch & PatchFlags::DYNAMIC_SLOTS != 0 {
                return Ok(true);
            }
            if patch & PatchFlags::FULL_PROPS != 0 {
                if !prev_props.to_boolean() {
                    return Ok(next_props.to_boolean());
                }
                return has_props_changed(rt, &prev_props, &next_props, &emits);
            } else if patch & PatchFlags::PROPS != 0 {
                let dynamic = dyn_props_of(rt, next);
                for i in 0..length_of(rt, &dynamic)? as usize {
                    let key = rt.to_string(&array_get(rt, &dynamic, i)?)?.to_rust_string_lossy();
                    if has_prop_value_changed(rt, &next_props, &prev_props, &key)? && !is_emit_listener(rt, &emits, &key)? {
                        return Ok(true);
                    }
                }
            }
        } else {
            if prev_children.to_boolean() || next_children.to_boolean() {
                let stable = if next_children.to_boolean() { rt.get_str(&next_children, "$stable")?.to_boolean() } else { false };
                if !stable {
                    return Ok(true);
                }
            }
            if vn_value::strict_eq(&prev_props, &next_props) {
                return Ok(false);
            }
            if !prev_props.to_boolean() {
                return Ok(next_props.to_boolean());
            }
            if !next_props.to_boolean() {
                return Ok(true);
            }
            return has_props_changed(rt, &prev_props, &next_props, &emits);
        }
        Ok(false)
    }

    /// `updateComponent(n1, n2, optimized)`.
    pub(crate) fn update_component(&self, n1: ObjRef, n2: ObjRef, optimized: bool) -> Res<()> {
        let rt = self.rt();
        let comp = component_of(rt, n1);
        let inst = as_obj(rt, &comp, "component of the old vnode")?;
        set_component(rt, n2, inst);
        if self.should_update_component(n1, n2, optimized)? {
            let rec = instance_of(rt, inst);
            *rec.next.borrow_mut() = Value::Object(n2);
            let update = rec.update.borrow().clone();
            rt.call(&update, &Value::Undefined, &[])?;
        } else {
            set_el(rt, n2, el_of(rt, n1));
            *instance_of(rt, inst).vnode.borrow_mut() = Value::Object(n2);
        }
        Ok(())
    }

    /// `updateComponentPreRender(instance, nextVNode, optimized)`.
    fn update_component_pre_render(&self, inst: ObjRef, next: ObjRef, optimized: bool) -> Res<()> {
        let rt = self.rt();
        let rec = instance_of(rt, inst);
        set_component(rt, next, inst);
        let prev_vnode = as_obj(rt, &rec.vnode.borrow().clone(), "instance vnode")?;
        let prev_props = props_of(rt, prev_vnode);
        *rec.vnode.borrow_mut() = Value::Object(next);
        *rec.next.borrow_mut() = Value::Null;
        let next_props = props_of(rt, next);
        update_props(rt, inst, &next_props, &prev_props, optimized)?;
        let children = children_of(rt, next);
        update_slots(rt, inst, &children, optimized)?;
        crate::renderer::pause_tracking(rt)?;
        flush_pre_flush_cbs(rt, Some(inst), None)?;
        crate::renderer::reset_tracking(rt)?;
        Ok(())
    }

    /// `renderComponentRoot(instance)`: runs the render function (or the functional component) and applies the
    /// attrs fallthrough, the directives and the transition of the vnode to the root.
    pub(crate) fn render_component_root(&self, inst: ObjRef) -> Res<ObjRef> {
        let rt = self.rt();
        let rec = instance_of(rt, inst);
        let comp = rec.ty.borrow().clone();
        let vnode = as_obj(rt, &rec.vnode.borrow().clone(), "instance vnode")?;
        let prev = set_current_rendering_instance(rt, Some(inst));
        let computed = self.compute_root(inst, vnode, &comp);
        let (mut result, mut fallthrough) = match computed {
            Ok(v) => v,
            Err(Throw(e)) => {
                handle_error(rt, &e, Some(inst), ErrType::Code(1))?;
                let comment = self.with_vst(|st| {
                    vnode::create_vnode(rt, st, Value::Symbol(rt.symbol_for(&vn_value::JsStr::from_str("v-cmt"))), Value::Null, Value::Null, Value::Number(0.0), Value::Null, false)
                });
                (comment?, None)
            }
        };
        let inherit = rec.inherit_attrs.borrow().clone();
        let inherit_on = !matches!(inherit, Value::Bool(false));
        if let Some(ft) = fallthrough.take() {
            if inherit_on {
                let keys = enumerable_keys(rt, &ft)?;
                let root_shape = shape_of(rt, result);
                if !keys.is_empty() && root_shape & (ShapeFlags::ELEMENT | ShapeFlags::COMPONENT) != 0 {
                    let mut extra = ft;
                    let has_model = keys.iter().any(|k| k.starts_with("onUpdate:"));
                    let options = rec.props_options.borrow().clone();
                    let options0 = if options.to_boolean() { rt.get_v(&options, &PropKey::Index(0))? } else { Value::Undefined };
                    if has_model && options0.to_boolean() {
                        extra = filter_model_listeners(rt, &extra, &options0)?;
                    }
                    result = self.with_vst(|st| vnode::clone_vnode(rt, st, result, &extra, false, true))?;
                }
            }
        }
        if dirs_of(rt, vnode).to_boolean() {
            result = self.with_vst(|st| vnode::clone_vnode(rt, st, result, &Value::Null, false, true))?;
            let own = dirs_of(rt, vnode);
            let merged = if dirs_of(rt, result).to_boolean() {
                let mut items = Vec::new();
                let base = dirs_of(rt, result);
                for i in 0..length_of(rt, &base)? as usize {
                    items.push(array_get(rt, &base, i)?);
                }
                for i in 0..length_of(rt, &own)? as usize {
                    items.push(array_get(rt, &own, i)?);
                }
                Value::Object(rt.new_array(items))
            } else {
                own
            };
            fields(rt, result, |f| f.dirs = merged);
        }
        if transition_of(rt, vnode).to_boolean() {
            crate::transition::set_hooks(rt, result, transition_of(rt, vnode))?;
        }
        set_current_rendering_instance(rt, prev);
        Ok(result)
    }

    /// The render call of `renderComponentRoot`, with the fallthrough attrs that come with the result.
    fn compute_root(&self, inst: ObjRef, vnode: ObjRef, comp: &Value) -> Res<(ObjRef, Option<Value>)> {
        let rt = self.rt();
        let rec = instance_of(rt, inst);
        if shape_of(rt, vnode) & ShapeFlags::STATEFUL_COMPONENT != 0 {
            let proxy = rec.proxy.borrow().clone();
            let render = rec.render.borrow().clone();
            let args = [
                proxy.clone(),
                rec.render_cache.borrow().clone(),
                rec.props.borrow().clone(),
                rec.setup_state.borrow().clone(),
                rec.data.borrow().clone(),
                rec.ctx.borrow().clone(),
            ];
            let rendered = rt.call(&render, &proxy, &args)?;
            let root = self.normalize_result(&rendered)?;
            let attrs = rec.attrs.borrow().clone();
            return Ok((root, Some(attrs)));
        }
        // functional component: render(props, { attrs, slots, emit }) or render(props, null)
        let props = rec.props.borrow().clone();
        let attrs = rec.attrs.borrow().clone();
        let n = arity(rt, comp)?;
        let rendered = if n > 1.0 {
            let ctx = rt.new_object();
            rt.define_data(ctx, PropKey::from_str("attrs"), attrs.clone(), Flags::ALL)?;
            let slots = rec.slots.borrow().clone();
            rt.define_data(ctx, PropKey::from_str("slots"), slots, Flags::ALL)?;
            let emit = rec.emit.borrow().clone();
            rt.define_data(ctx, PropKey::from_str("emit"), emit, Flags::ALL)?;
            rt.call(comp, &Value::Undefined, &[props, Value::Object(ctx)])?
        } else {
            rt.call(comp, &Value::Undefined, &[props, Value::Null])?
        };
        let root = self.normalize_result(&rendered)?;
        let has_props = rt.get_str(comp, "props")?.to_boolean();
        let fallthrough = if has_props { Some(attrs.clone()) } else { functional_fallthrough(rt, &attrs)? };
        Ok((root, fallthrough))
    }

    /// `unmountComponent(instance, parentSuspense, doRemove)`. `instance` is the component value of the vnode.
    pub(crate) fn unmount_component(&self, instance: Value, do_remove: bool) -> Res<()> {
        let rt = self.rt();
        let inst = as_obj(rt, &instance, "component")?;
        let rec = instance_of(rt, inst);
        let m = hook_list(rt, inst, LifeHook::M);
        let a = hook_list(rt, inst, LifeHook::A);
        for h in m.iter().chain(a.iter()) {
            if let Value::Object(f) = h {
                let meta = job_meta(rt, *f);
                meta.flags.set(meta.flags.get() | DISPOSED);
            }
        }
        let bum = hook_list(rt, inst, LifeHook::Bum);
        if !bum.is_empty() {
            invoke_array_fns(rt, &bum)?;
        }
        let scope = rec.scope.borrow().clone();
        call_method(rt, &scope, "stop")?;
        let job = rec.job.borrow().clone();
        let sub = rec.sub_tree.borrow().clone();
        let vnode = as_obj(rt, &rec.vnode.borrow().clone(), "instance vnode")?;
        if let Value::Object(j) = job {
            let meta = job_meta(rt, j);
            meta.flags.set(meta.flags.get() | DISPOSED);
            let sub = as_obj(rt, &sub, "subtree")?;
            self.unmount(sub, Some(inst), do_remove, false)?;
        } else if el_of(rt, vnode).is_some() && sub.to_boolean() {
            let sub = as_obj(rt, &sub, "subtree")?;
            self.unmount(sub, Some(inst), do_remove, false)?;
        }
        let um = hook_list(rt, inst, LifeHook::Um);
        if !um.is_empty() {
            queue_post_render_effect(rt, &hook_array(rt, um), &Value::Null)?;
        }
        self.queue_steps(vec![Step::Unmounted(inst)])
    }
}
