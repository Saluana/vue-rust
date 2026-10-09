//! vn-runtime::instance: the component instance record (Vue's `ComponentInternalInstance`, built by `createComponentInstance`
//! of runtime-core 3.5.43, production build) and the realm's current-instance state (`currentInstance`,
//! `currentRenderingInstance`, `currentApp`, the uid counter). Behaviour per @vue/runtime-core 3.5.43.
//!
//! Who writes what. Every ObjRef a field holds is traced by `Instance::trace`.
//! - `create_component_instance` sets uid, vnode, ty (Vue `type`), parent, appContext, root, scope (a detached EffectScope),
//!   provides, ids, renderCache, inheritAttrs, ctx (`{_: instance}`), suspense (always null: Suspense is unsupported),
//!   suspenseId, and the hook lists (empty). It calls `vnode.ce(instance)` when the vnode has one.
//! - Left at their defaults for other WPs: propsOptions and emitsOptions (props/emits WP), emit (emits WP), and every
//!   renderer field (next, subTree, effect, update, job, render, proxy, exposed, exposeProxy, withProxy, accessCache,
//!   components, directives, emitted, propsDefaults, data, props, attrs, slots, refs, setupState, setupContext, isMounted,
//!   isUnmounted, isDeactivated, asyncDep, asyncResolved). They start at createComponentInstance's defaults.
//! - `lifecycle.rs` appends to the hook lists. `errors.rs` reads `ec` along the parent chain.
//!
//! Deviations: the instance handle is the Native object holding the record; JS code cannot read its fields. Vue's
//! `getCurrentInstance()` returns the internal instance; ours returns the handle (null when there is none).

use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;

use vn_value::{payload_ref, Flags, JsStr, NativeObject, ObjRef, PropKey, Res, Rt, Tracer, Value};

/// The lifecycle hook lists of an instance (`bc c bm m bu u um bum da a rtg rtc ec sp` in Vue).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifeHook {
    Bc,
    C,
    Bm,
    M,
    Bu,
    U,
    Um,
    Bum,
    Da,
    A,
    Rtg,
    Rtc,
    Ec,
    Sp,
}

impl LifeHook {
    pub const ALL: [LifeHook; 14] = [
        LifeHook::Bc,
        LifeHook::C,
        LifeHook::Bm,
        LifeHook::M,
        LifeHook::Bu,
        LifeHook::U,
        LifeHook::Um,
        LifeHook::Bum,
        LifeHook::Da,
        LifeHook::A,
        LifeHook::Rtg,
        LifeHook::Rtc,
        LifeHook::Ec,
        LifeHook::Sp,
    ];

    /// The list's property name in Vue, which is also the `type` string that `injectHook` receives.
    pub fn key(self) -> &'static str {
        match self {
            LifeHook::Bc => "bc",
            LifeHook::C => "c",
            LifeHook::Bm => "bm",
            LifeHook::M => "m",
            LifeHook::Bu => "bu",
            LifeHook::U => "u",
            LifeHook::Um => "um",
            LifeHook::Bum => "bum",
            LifeHook::Da => "da",
            LifeHook::A => "a",
            LifeHook::Rtg => "rtg",
            LifeHook::Rtc => "rtc",
            LifeHook::Ec => "ec",
            LifeHook::Sp => "sp",
        }
    }

    pub fn from_key(s: &str) -> Option<LifeHook> {
        LifeHook::ALL.iter().copied().find(|h| h.key() == s)
    }
}

// Every `Value` field of the record, in Vue's createComponentInstance order. Each starts as `null`; the constructor
// below overrides the ones that Vue sets. `ty` is Vue's `type` (a reserved word here).
macro_rules! instance_record {
    ($($f:ident),* $(,)?) => {
        /// The instance record (Vue's `ComponentInternalInstance`). Fields are interior-mutable: the renderer and the
        /// runtime-b modules update them through shared references.
        pub struct Instance {
            pub uid: f64,
            $(pub $f: RefCell<Value>,)*
            pub suspense_id: f64,
            pub async_resolved: Cell<bool>,
            pub is_mounted: Cell<bool>,
            pub is_unmounted: Cell<bool>,
            pub is_deactivated: Cell<bool>,
            /// Lifecycle hook lists indexed by `LifeHook as usize` (order of `LifeHook::ALL`).
            pub hooks: [RefCell<Vec<Value>>; 14],
        }

        impl Instance {
            fn blank(uid: f64) -> Instance {
                Instance {
                    uid,
                    $($f: RefCell::new(Value::Null),)*
                    suspense_id: 0.0,
                    async_resolved: Cell::new(false),
                    is_mounted: Cell::new(false),
                    is_unmounted: Cell::new(false),
                    is_deactivated: Cell::new(false),
                    hooks: std::array::from_fn(|_| RefCell::new(Vec::new())),
                }
            }

            fn trace_values(&self, t: &mut Tracer) {
                $(t.mark_value(&self.$f.borrow());)*
                for h in &self.hooks {
                    for v in h.borrow().iter() {
                        t.mark_value(v);
                    }
                }
            }
        }
    };
}

instance_record!(
    vnode,
    ty,
    parent,
    app_context,
    root,
    next,
    sub_tree,
    effect,
    update,
    job,
    scope,
    render,
    proxy,
    exposed,
    expose_proxy,
    with_proxy,
    provides,
    ids,
    access_cache,
    render_cache,
    components,
    directives,
    props_options,
    emits_options,
    emit,
    emitted,
    props_defaults,
    inherit_attrs,
    ctx,
    data,
    props,
    attrs,
    slots,
    refs,
    setup_state,
    setup_context,
    suspense,
    async_dep,
);

impl NativeObject for Instance {
    fn trace(&self, t: &mut Tracer) {
        self.trace_values(t);
    }
}

/// Borrowed view of an instance record, kept alive by its payload `Rc`.
pub struct InstRef(Rc<dyn NativeObject>);

impl Deref for InstRef {
    type Target = Instance;
    fn deref(&self) -> &Instance {
        payload_ref::<Instance>(&*self.0).expect("component instance payload (framework bug)")
    }
}

/// The record behind `obj`, or `None` when `obj` is not a component instance handle.
pub fn as_instance(rt: &Rt, obj: ObjRef) -> Option<InstRef> {
    let p = rt.native_payload(obj)?;
    if payload_ref::<Instance>(&*p).is_some() { Some(InstRef(p)) } else { None }
}

/// The record behind an instance handle that the runtime created (a framework bug when `obj` is not one).
pub fn instance_of(rt: &Rt, obj: ObjRef) -> InstRef {
    as_instance(rt, obj).expect("component instance handle (framework bug)")
}

/// A realm-wide state object: a Native payload registered under `name` as a builtin (so it is a GC root), created by
/// `make` on first use. Derefs to the payload type `T`.
pub(crate) struct Shared<T: NativeObject>(Rc<dyn NativeObject>, PhantomData<T>);

impl<T: NativeObject> Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        payload_ref::<T>(&*self.0).expect("runtime-b realm state (framework bug)")
    }
}

pub(crate) fn shared<T: NativeObject>(rt: &Rt, name: &'static str, make: impl FnOnce(&Rt) -> T) -> Shared<T> {
    let obj = match rt.builtin(name) {
        Some(o) => o,
        None => {
            let o = rt.new_native_object(None, Rc::new(make(rt)));
            rt.register_builtin(name, o);
            o
        }
    };
    Shared(rt.native_payload(obj).expect("runtime-b realm state object (framework bug)"), PhantomData)
}

/// The realm's current-instance state.
pub(crate) struct InstState {
    /// `currentInstance`.
    pub(crate) current: Cell<Option<ObjRef>>,
    /// `currentRenderingInstance`.
    pub(crate) rendering: Cell<Option<ObjRef>>,
    /// `currentApp` (`null` when no app is running `runWithContext`).
    pub(crate) app: RefCell<Value>,
    /// The module-level `uid` counter of `createComponentInstance`.
    pub(crate) uid: Cell<f64>,
    /// The shared `EMPTY_OBJ` that Vue uses as the default for props, attrs, slots and similar fields.
    pub(crate) empty_obj: Value,
    /// Reactivity's `effectScope` function (`new EffectScope(detached)`).
    pub(crate) effect_scope: Value,
    /// Reactivity's `pauseTracking` and `resetTracking`.
    pub(crate) pause_tracking: Value,
    pub(crate) reset_tracking: Value,
}

impl NativeObject for InstState {
    fn trace(&self, t: &mut Tracer) {
        if let Some(o) = self.current.get() {
            t.mark(o);
        }
        if let Some(o) = self.rendering.get() {
            t.mark(o);
        }
        t.mark_value(&self.app.borrow());
        t.mark_value(&self.empty_obj);
        t.mark_value(&self.effect_scope);
        t.mark_value(&self.pause_tracking);
        t.mark_value(&self.reset_tracking);
    }
}

fn named(list: &[(&'static str, Value)], name: &str) -> Value {
    list.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone()).expect("reactivity export (framework bug)")
}

fn new_inst_state(rt: &Rt) -> InstState {
    let exports = vn_reactivity::exports(rt);
    let internal = vn_reactivity::internal_exports(rt);
    InstState {
        current: Cell::new(None),
        rendering: Cell::new(None),
        app: RefCell::new(Value::Null),
        uid: Cell::new(0.0),
        empty_obj: Value::Object(rt.new_object()),
        effect_scope: named(&exports, "effectScope"),
        pause_tracking: named(&internal, "pauseTracking"),
        reset_tracking: named(&internal, "resetTracking"),
    }
}

pub(crate) fn inst_state(rt: &Rt) -> Shared<InstState> {
    shared(rt, "vn-runtime.instance", new_inst_state)
}

/// `EMPTY_OBJ`: the realm's shared empty object.
pub fn empty_obj(rt: &Rt) -> Value {
    inst_state(rt).empty_obj.clone()
}

/// `Object.create(proto)` for a provides or prototype value (an object or null, as in Vue).
pub(crate) fn object_create(rt: &Rt, proto: &Value) -> Res<ObjRef> {
    match proto {
        Value::Object(o) => Ok(rt.new_object_with_proto(Some(*o))),
        Value::Null => Ok(rt.new_object_with_proto(None)),
        _ => Err(rt.throw_type_error("Object prototype may only be an Object or null")),
    }
}

/// `createComponentInstance(vnode, parent)` for the production build. `empty_app_context` is the app module's
/// `emptyAppContext`, used when there is neither a parent nor a `vnode.appContext`. `propsOptions` and `emitsOptions`
/// stay undefined: the props and emits WPs normalize them.
pub fn create_component_instance(rt: &Rt, vnode: &Value, parent: Option<ObjRef>, empty_app_context: &Value) -> Res<ObjRef> {
    let st = inst_state(rt);
    let uid = st.uid.get();
    st.uid.set(uid + 1.0);
    let parent_rec = match parent {
        Some(p) => Some(as_instance(rt, p).ok_or_else(|| rt.throw_type_error("parent is not a component instance"))?),
        None => None,
    };
    let ty = rt.get_str(vnode, "type")?;
    let app_context = match &parent_rec {
        Some(p) => p.app_context.borrow().clone(),
        None => rt.get_str(vnode, "appContext")?,
    };
    let app_context = if app_context.to_boolean() { app_context } else { empty_app_context.clone() };
    let scope = rt.call(&st.effect_scope, &Value::Undefined, &[Value::Bool(true)])?;
    let provides = match &parent_rec {
        Some(p) => p.provides.borrow().clone(),
        None => {
            let ap = rt.get_str(&app_context, "provides")?;
            Value::Object(object_create(rt, &ap)?)
        }
    };
    let ids = match &parent_rec {
        Some(p) => p.ids.borrow().clone(),
        None => Value::Object(rt.new_array(vec![
            Value::String(JsStr::empty()),
            Value::Number(0.0),
            Value::Number(0.0),
        ])),
    };
    let inherit_attrs = rt.get_str(&ty, "inheritAttrs")?;
    let ce = rt.get_str(vnode, "ce")?;
    let empty = st.empty_obj.clone();
    let render_cache = Value::Object(rt.new_array(Vec::new()));
    let rec = Instance::blank(uid);
    rec.vnode.replace(vnode.clone());
    rec.ty.replace(ty);
    rec.parent.replace(parent.map(Value::Object).unwrap_or(Value::Null));
    rec.app_context.replace(app_context);
    rec.scope.replace(scope);
    rec.provides.replace(provides);
    rec.ids.replace(ids);
    rec.render_cache.replace(render_cache);
    rec.inherit_attrs.replace(inherit_attrs);
    rec.props_defaults.replace(empty.clone());
    rec.data.replace(empty.clone());
    rec.props.replace(empty.clone());
    rec.attrs.replace(empty.clone());
    rec.slots.replace(empty.clone());
    rec.refs.replace(empty.clone());
    rec.setup_state.replace(empty);
    let obj = rt.new_native_object(None, Rc::new(rec));
    let this = instance_of(rt, obj);
    let root = match &parent_rec {
        Some(p) => p.root.borrow().clone(),
        None => Value::Object(obj),
    };
    this.root.replace(root);
    let ctx = rt.new_object();
    rt.define_data(ctx, PropKey::lit("_"), Value::Object(obj), Flags::ALL)?;
    this.ctx.replace(Value::Object(ctx));
    if ce.to_boolean() {
        rt.call(&ce, vnode, &[Value::Object(obj)])?;
    }
    Ok(obj)
}

pub fn current_instance(rt: &Rt) -> Option<ObjRef> {
    inst_state(rt).current.get()
}

/// `getCurrentInstance()`: `currentInstance || currentRenderingInstance`.
pub fn get_current_instance(rt: &Rt) -> Option<ObjRef> {
    let st = inst_state(rt);
    st.current.get().or(st.rendering.get())
}

/// `setCurrentRenderingInstance(instance)`: returns the previous value.
pub fn set_current_rendering_instance(rt: &Rt, inst: Option<ObjRef>) -> Option<ObjRef> {
    inst_state(rt).rendering.replace(inst)
}

pub fn current_app(rt: &Rt) -> Value {
    inst_state(rt).app.borrow().clone()
}

/// Sets `currentApp` (the app module's `runWithContext`) and returns the previous value.
pub fn set_current_app(rt: &Rt, app: Value) -> Value {
    inst_state(rt).app.replace(app)
}

fn call_method(rt: &Rt, obj: &Value, name: &str) -> Res<Value> {
    let f = rt.get_str(obj, name)?;
    rt.call(&f, obj, &[])
}

/// `setCurrentInstance(instance)`: makes `inst` the current instance and turns its scope on. Returns the previous
/// current instance, which `leave_instance` restores.
pub fn enter_instance(rt: &Rt, inst: ObjRef) -> Res<Option<ObjRef>> {
    let st = inst_state(rt);
    let prev = st.current.replace(Some(inst));
    let scope = instance_of(rt, inst).scope.borrow().clone();
    if let Err(e) = call_method(rt, &scope, "on") {
        st.current.set(prev);
        return Err(e);
    }
    Ok(prev)
}

/// The reset closure of `setCurrentInstance`: `instance.scope.off()`, then the previous instance is restored. The
/// restore also runs when `off` throws (a deliberate deviation: Vue would leave the state wrong on that path).
pub fn leave_instance(rt: &Rt, inst: ObjRef, prev: Option<ObjRef>) -> Res<()> {
    let st = inst_state(rt);
    let scope = instance_of(rt, inst).scope.borrow().clone();
    let r = call_method(rt, &scope, "off").map(|_| ());
    st.current.set(prev);
    r
}

/// `unsetCurrentInstance()`: `currentInstance && currentInstance.scope.off()`, then no current instance.
pub fn unset_current_instance(rt: &Rt) -> Res<()> {
    let st = inst_state(rt);
    match st.current.get() {
        Some(c) => {
            let scope = instance_of(rt, c).scope.borrow().clone();
            let r = call_method(rt, &scope, "off").map(|_| ());
            st.current.set(None);
            r
        }
        None => Ok(()),
    }
}

/// Runs `f` with `inst` as the current instance and restores the previous one afterwards, on the error path too.
pub fn with_instance<T>(rt: &Rt, inst: ObjRef, f: impl FnOnce(&Rt) -> Res<T>) -> Res<T> {
    let prev = enter_instance(rt, inst)?;
    let r = f(rt);
    let l = leave_instance(rt, inst, prev);
    match (r, l) {
        (Err(e), _) => Err(e),
        (Ok(_), Err(e)) => Err(e),
        (Ok(v), Ok(())) => Ok(v),
    }
}

/// `markAsyncBoundary(instance)`: `instance.ids = [ids[0] + ids[2]++ + "-", 0, 0]`.
pub fn mark_async_boundary(rt: &Rt, inst: ObjRef) -> Res<()> {
    let rec = instance_of(rt, inst);
    let ids = rec.ids.borrow().clone();
    let s0 = rt.get_v(&ids, &PropKey::Index(0))?;
    let n2 = rt.to_number(&rt.get_v(&ids, &PropKey::Index(2))?)?;
    rt.set_v_strict(&ids, &PropKey::Index(2), Value::Number(n2 + 1.0))?;
    let left = rt.to_string(&s0)?;
    let mid = rt.to_string(&Value::Number(n2))?;
    let s = left.concat(&mid).concat(&JsStr::from_str("-"));
    let arr = rt.new_array(vec![Value::String(s), Value::Number(0.0), Value::Number(0.0)]);
    rec.ids.replace(Value::Object(arr));
    Ok(())
}
