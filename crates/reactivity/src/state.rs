//! Per-realm reactivity state: the globals of @vue/reactivity 3.5.43 (activeSub, activeEffectScope, activeWatcher,
//! batchDepth, batchedSub, batchedComputed, globalVersion, shouldTrack, trackStack), the prototypes of the JS-visible
//! objects and the replaceable reactive hooks. It lives in a `NativeObject` registered as a builtin, so the collector
//! treats it as a root and every ObjRef it holds stays alive.

use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::rc::Rc;

use vn_value::{payload_ref, Intrinsic, JsStr, NativeObject, ObjRef, Prop, PropKey, Res, Rt, Sym, Tracer, Value};

const REGISTRY: &str = "vn-reactivity";

/// Prototypes of the JS-visible reactivity objects (set once per realm by `api::install_protos`).
#[derive(Clone, Copy, Default)]
pub(crate) struct Protos {
    pub(crate) reference: Option<ObjRef>,
    pub(crate) custom: Option<ObjRef>,
    pub(crate) getter: Option<ObjRef>,
    pub(crate) prop: Option<ObjRef>,
    pub(crate) computed: Option<ObjRef>,
    pub(crate) effect: Option<ObjRef>,
    pub(crate) scope: Option<ObjRef>,
    /// The prototype of every dep (the internal Dep objects, which carry `__v_skip`).
    pub(crate) dep: Option<ObjRef>,
}

/// Replaceable `toReactive` / `toReadonly` of @vue/reactivity. Identity until the reactive wrappers register theirs.
#[derive(Clone, Copy)]
pub struct ReactiveHooks {
    pub to_reactive: fn(&Rt, &Value) -> Res<Value>,
    pub to_readonly: fn(&Rt, &Value) -> Res<Value>,
}

fn identity(_rt: &Rt, v: &Value) -> Res<Value> {
    Ok(v.clone())
}

/// Symbols used as dependency keys (ITERATE_KEY, MAP_KEY_ITERATE_KEY, ARRAY_ITERATE_KEY). Immortal, not traced.
#[derive(Clone, Copy)]
pub(crate) struct Syms {
    pub(crate) iterate: Sym,
    pub(crate) map_iterate: Sym,
    pub(crate) array_iterate: Sym,
}

pub(crate) struct State {
    pub(crate) active_sub: Cell<Option<ObjRef>>,
    pub(crate) active_scope: Cell<Option<ObjRef>>,
    pub(crate) active_watcher: Cell<Option<ObjRef>>,
    pub(crate) batch_depth: Cell<u32>,
    pub(crate) batched_sub: Cell<Option<ObjRef>>,
    pub(crate) batched_computed: Cell<Option<ObjRef>>,
    pub(crate) global_version: Cell<i64>,
    pub(crate) should_track: Cell<bool>,
    pub(crate) track_stack: RefCell<Vec<bool>>,
    pub(crate) protos: Cell<Protos>,
    pub(crate) hooks: Cell<ReactiveHooks>,
    pub(crate) syms: Syms,
    /// `Object.prototype.toString` as captured at setup (`toTypeString` of @vue/shared uses it).
    pub(crate) obj_to_string: ObjRef,
    /// Cached JS exports (`api::exports`, `api::internal_exports`); traced.
    pub(crate) exports: RefCell<Option<Vec<(&'static str, Value)>>>,
    pub(crate) internal: RefCell<Option<Vec<(&'static str, Value)>>>,
}

impl State {
    pub(crate) fn object_to_string(&self) -> ObjRef {
        self.obj_to_string
    }
}

impl NativeObject for State {
    fn trace(&self, t: &mut Tracer) {
        for o in [
            self.active_sub.get(),
            self.active_scope.get(),
            self.active_watcher.get(),
            self.batched_sub.get(),
            self.batched_computed.get(),
        ]
        .into_iter()
        .flatten()
        {
            t.mark(o);
        }
        t.mark(self.obj_to_string);
        let p = self.protos.get();
        for o in [p.reference, p.custom, p.getter, p.prop, p.computed, p.effect, p.scope, p.dep].into_iter().flatten() {
            t.mark(o);
        }
        for cache in [&self.exports, &self.internal] {
            if let Some(list) = cache.borrow().as_ref() {
                for (_, v) in list {
                    t.mark_value(v);
                }
            }
        }
    }
}

/// Handle to the realm's state (an `Rc` clone of its payload).
pub(crate) struct StateRef(Rc<dyn NativeObject>);

impl Deref for StateRef {
    type Target = State;
    fn deref(&self) -> &State {
        payload_ref::<State>(&*self.0).expect("reactivity state (framework bug)")
    }
}

/// The reactivity state of `rt`, created on first use.
pub(crate) fn state(rt: &Rt) -> StateRef {
    let obj = match rt.builtin(REGISTRY) {
        Some(o) => o,
        None => init(rt),
    };
    StateRef(rt.native_payload(obj).expect("reactivity state object (framework bug)"))
}

fn init(rt: &Rt) -> ObjRef {
    let syms = Syms {
        iterate: rt.new_symbol(Some(JsStr::empty())),
        map_iterate: rt.new_symbol(Some(JsStr::empty())),
        array_iterate: rt.new_symbol(Some(JsStr::empty())),
    };
    let op = rt.intrinsic(Intrinsic::ObjectPrototype);
    let obj_to_string = match rt.get_own_property(op, &PropKey::lit("toString")) {
        Ok(Some(Prop::Data { value: Value::Object(f), .. })) => f,
        _ => panic!("Object.prototype.toString is missing (framework bug)"),
    };
    let st = State {
        active_sub: Cell::new(None),
        active_scope: Cell::new(None),
        active_watcher: Cell::new(None),
        batch_depth: Cell::new(0),
        batched_sub: Cell::new(None),
        batched_computed: Cell::new(None),
        global_version: Cell::new(0),
        should_track: Cell::new(true),
        track_stack: RefCell::new(Vec::new()),
        protos: Cell::new(Protos::default()),
        hooks: Cell::new(ReactiveHooks { to_reactive: identity, to_readonly: identity }),
        syms,
        obj_to_string,
        exports: RefCell::new(None),
        internal: RefCell::new(None),
    };
    let payload: Rc<dyn NativeObject> = Rc::new(st);
    let obj = rt.new_native_object(None, payload);
    rt.register_builtin(REGISTRY, obj);
    let protos = crate::api::install_protos(rt);
    state(rt).protos.set(protos);
    obj
}

/// Replaces the reactive hooks (`toReactive`, `toReadonly`) used by refs and computed values.
pub fn set_reactive_hooks(rt: &Rt, hooks: ReactiveHooks) {
    state(rt).hooks.set(hooks);
}

/// `toReactive(value)`: through the replaceable hook.
pub(crate) fn to_reactive(rt: &Rt, v: &Value) -> Res<Value> {
    let f = state(rt).hooks.get().to_reactive;
    f(rt, v)
}
