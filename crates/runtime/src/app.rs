//! vn-runtime::app: the app context of @vue/runtime-core 3.5.43 (`createAppContext`). Checkpoint 2 provides the
//! context that components use when no app is mounted (`emptyAppContext`); the app object follows in a later checkpoint.

use vn_value::{payload_ref, Call, Flags, ObjRef, PropKey, Res, Rt, Value};

/// The `i`-th argument of a call (undefined past the end).
fn arg(c: &Call<'_>, i: usize) -> Value {
    c.args.get(i).cloned().unwrap_or(Value::Undefined)
}

/// The realm builtin that holds `emptyAppContext`.
pub(crate) const EMPTY_APP_CONTEXT: &str = "vn-runtime.empty-app-context";

/// `createAppContext()`: the config, the global registries and the caches of an app.
pub(crate) fn create_app_context(rt: &Rt) -> Res<ObjRef> {
    let ctx = rt.new_object();
    let config = rt.new_object();
    let global_props = rt.new_object();
    let merge = rt.new_object();
    let compiler = rt.new_object();
    let components = rt.new_object();
    let directives = rt.new_object();
    let provides = rt.new_object_with_proto(None);
    let cache_a = rt.new_object();
    let cache_b = rt.new_object();
    let cache_c = rt.new_object();
    let mixins = rt.new_array(Vec::new());
    let set = |o: ObjRef, k: &str, v: Value| rt.define_data(o, PropKey::from_str(k), v, Flags::ALL);
    set(config, "performance", Value::Bool(false))?;
    set(config, "globalProperties", Value::Object(global_props))?;
    set(config, "optionMergeStrategies", Value::Object(merge))?;
    set(config, "errorHandler", Value::Undefined)?;
    set(config, "warnHandler", Value::Undefined)?;
    set(config, "compilerOptions", Value::Object(compiler))?;
    set(ctx, "app", Value::Null)?;
    set(ctx, "config", Value::Object(config))?;
    set(ctx, "mixins", Value::Object(mixins))?;
    set(ctx, "components", Value::Object(components))?;
    set(ctx, "directives", Value::Object(directives))?;
    set(ctx, "provides", Value::Object(provides))?;
    set(ctx, "optionsCache", Value::Object(cache_a))?;
    set(ctx, "propsCache", Value::Object(cache_b))?;
    set(ctx, "emitsCache", Value::Object(cache_c))?;
    Ok(ctx)
}

/// `emptyAppContext`: the context of components created without an app (one per realm).
pub(crate) fn empty_app_context(rt: &Rt) -> Res<Value> {
    if let Some(o) = rt.builtin(EMPTY_APP_CONTEXT) {
        return Ok(Value::Object(o));
    }
    let o = create_app_context(rt)?;
    rt.register_builtin(EMPTY_APP_CONTEXT, o);
    Ok(Value::Object(o))
}

/// The state of one app object (`createApp`'s closure variables): its root, its mount and its plugins.
pub(crate) struct AppState {
    pub(crate) context: ObjRef,
    pub(crate) root: Value,
    pub(crate) root_props: Value,
    pub(crate) mounted: std::cell::Cell<bool>,
    pub(crate) container: std::cell::Cell<Option<crate::host::HostNode>>,
    pub(crate) installed: std::cell::RefCell<Vec<Value>>,
    pub(crate) cleanup: std::cell::RefCell<Vec<Value>>,
    /// `createSSRApp`: `mount` adopts the markup already in the container and does not clear it.
    pub(crate) hydrate: bool,
}

impl vn_value::NativeObject for AppState {
    fn trace(&self, t: &mut vn_value::Tracer) {
        t.mark(self.context);
        t.mark_value(&self.root);
        t.mark_value(&self.root_props);
        for v in self.installed.borrow().iter().chain(self.cleanup.borrow().iter()) {
            t.mark_value(v);
        }
    }
}

fn app_state(rt: &Rt, c: &Call<'_>) -> Res<std::rc::Rc<dyn vn_value::NativeObject>> {
    let data = rt.fn_data(c.callee);
    match data.get(1) {
        Some(Value::Object(o)) => rt.native_payload(*o).ok_or_else(|| rt.throw_type_error("app state (framework bug)")),
        _ => Err(rt.throw_type_error("app method without its state (framework bug)")),
    }
}

fn app_of(rt: &Rt, c: &Call<'_>) -> Value {
    rt.fn_data(c.callee).first().cloned().unwrap_or(Value::Undefined)
}

/// `createApp(rootComponent, rootProps)`.
pub(crate) fn create_app(rt: &Rt, root: Value, props: Value) -> Res<Value> {
    create_app_with(rt, root, props, false)
}

/// `createApp` (`hydrate` false) or `createSSRApp` (`hydrate` true): the same app object; only `mount` differs.
fn create_app_with(rt: &Rt, root: Value, props: Value, hydrate: bool) -> Res<Value> {
    let root = if rt.is_callable(&root) {
        root
    } else {
        let copy = rt.new_object();
        for k in crate::dom_props::enumerable_keys(rt, &root)? {
            let v = rt.get_str(&root, &k)?;
            rt.define_data(copy, PropKey::from_str(&k), v, Flags::ALL)?;
        }
        Value::Object(copy)
    };
    let props = match &props {
        Value::Object(_) => props,
        _ => Value::Null,
    };
    let context = create_app_context(rt)?;
    let state = AppState {
        context,
        root: root.clone(),
        root_props: props.clone(),
        mounted: std::cell::Cell::new(false),
        container: std::cell::Cell::new(None),
        installed: std::cell::RefCell::new(Vec::new()),
        cleanup: std::cell::RefCell::new(Vec::new()),
        hydrate,
    };
    let payload = rt.new_native_object(None, std::rc::Rc::new(state));
    let app = rt.new_object();
    let set = |k: &str, v: Value| rt.define_data(app, PropKey::from_str(k), v, Flags::ALL);
    set("_uid", Value::Number(0.0))?;
    set("_component", root)?;
    set("_props", props)?;
    set("_container", Value::Null)?;
    set("_context", Value::Object(context))?;
    set("_instance", Value::Null)?;
    set("version", Value::String(vn_value::JsStr::from_str("3.5.43")))?;
    let config = rt.get_str(&Value::Object(context), "config")?;
    set("config", config)?;
    rt.define_data(context, PropKey::lit("app"), Value::Object(app), Flags::ALL)?;
    let methods: [(&str, vn_value::CodeFn, u32); 9] = [
        ("mount", app_mount, 1),
        ("unmount", app_unmount, 0),
        ("use", app_use, 1),
        ("component", app_component, 2),
        ("directive", app_directive, 2),
        ("provide", app_provide, 2),
        ("runWithContext", app_run_with_context, 1),
        ("onUnmount", app_on_unmount, 1),
        ("mixin", app_mixin, 1),
    ];
    for (name, code, len) in methods {
        let f = rt.new_native_function(code, name, len, vec![Value::Object(app), Value::Object(payload)]);
        rt.define_data(app, PropKey::from_str(name), Value::Object(f), Flags::ALL)?;
    }
    Ok(Value::Object(app))
}

/// `normalizeContainer(container)`: an element handle, or the first element matching a selector.
fn container_node(rt: &Rt, v: &Value) -> Res<Option<crate::host::HostNode>> {
    match v {
        Value::String(s) => {
            let sel = s.to_rust_string_lossy();
            let host = crate::renderer::renderer(rt)?.ctx.host();
            let r = {
                host.query_selector(&sel)
            };
            match r {
                Ok(n) => Ok(n),
                Err(e) => Err(crate::dom_events::host_error(rt, e)),
            }
        }
        v if v.is_nullish() => Ok(None),
        v => {
            let (_, node) = crate::dom_events::ctx_of_handle(rt, v)?;
            Ok(Some(node))
        }
    }
}

fn app_mount(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let app = app_of(rt, c);
    let p = app_state(rt, c)?;
    let state = payload_ref::<AppState>(&*p).expect("app state type (framework bug)");
    let Some(node) = container_node(rt, &arg(c, 0))? else {
        return Ok(Value::Undefined);
    };
    let r = crate::renderer::renderer(rt)?;
    let host = r.ctx.host();
    let is_element = {
        host.is_element(node)
    };
    // runtime-dom's mount clears the container; createSSRApp's mount keeps the server markup for hydration.
    if is_element && !state.hydrate {
        let mut h = r.ctx.host();
        h.set_element_text(node, "").map_err(|e| crate::dom_events::host_error(rt, e))?;
    }
    if state.mounted.get() {
        return Ok(Value::Undefined);
    }
    let vnode = rt.call(&crate::renderer::builtin_fn(rt, "vn-runtime.createVNode"), &Value::Undefined, &[state.root.clone(), state.root_props.clone()])?;
    let Value::Object(vn) = vnode else { return Err(rt.throw_type_error("createVNode returned no vnode (framework bug)")) };
    crate::vnode::fields(rt, vn, |f| f.app_context = Value::Object(state.context));
    if state.hydrate {
        crate::renderer::hydrate_into(rt, vn, node)?;
    } else {
        crate::renderer::render_into(rt, Some(vn), node)?;
    }
    state.mounted.set(true);
    state.container.set(Some(node));
    let inst = match crate::renderer::component_of(rt, vn) {
        Value::Object(i) => Some(i),
        _ => None,
    };
    let _ = app;
    if is_element {
        let mut h = r.ctx.host();
        let _ = h.remove_attribute(node, "v-cloak");
        let _ = h.set_attribute(node, "data-v-app", "");
    }
    crate::public_instance::public_instance_of(rt, inst)
}

fn app_unmount(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let app = app_of(rt, c);
    let p = app_state(rt, c)?;
    let state = payload_ref::<AppState>(&*p).expect("app state type (framework bug)");
    if state.mounted.get() {
        let fns = state.cleanup.borrow().clone();
        for f in fns {
            rt.call(&f, &Value::Undefined, std::slice::from_ref(&app))?;
        }
        if let Some(node) = state.container.get() {
            crate::renderer::render_into(rt, None, node)?;
        }
        state.mounted.set(false);
        state.container.set(None);
    }
    Ok(Value::Undefined)
}

fn app_use(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let app = app_of(rt, c);
    let p = app_state(rt, c)?;
    let state = payload_ref::<AppState>(&*p).expect("app state type (framework bug)");
    let plugin = arg(c, 0);
    let already = state.installed.borrow().iter().any(|v| vn_value::same_value_zero(v, &plugin));
    if !already {
        let install = if plugin.to_boolean() { rt.get_str(&plugin, "install")? } else { Value::Undefined };
        let mut args = vec![app.clone()];
        args.extend(c.args.iter().skip(1).cloned());
        if rt.is_callable(&install) {
            state.installed.borrow_mut().push(plugin.clone());
            rt.call(&install, &plugin, &args)?;
        } else if rt.is_callable(&plugin) {
            state.installed.borrow_mut().push(plugin.clone());
            rt.call(&plugin, &Value::Undefined, &args)?;
        }
    }
    Ok(app)
}

/// `app.component(name, def)`: a registration, or the lookup when `def` is absent.
fn register(rt: &Rt, c: &Call<'_>, which: &str) -> Res<Value> {
    let app = app_of(rt, c);
    let p = app_state(rt, c)?;
    let state = payload_ref::<AppState>(&*p).expect("app state type (framework bug)");
    let registry = rt.get_str(&Value::Object(state.context), which)?;
    let name = arg(c, 0);
    let def = arg(c, 1);
    if !def.to_boolean() {
        return rt.get_v(&registry, &PropKey::from_str(&rt.to_string(&name)?.to_rust_string_lossy()));
    }
    rt.set_v_strict(&registry, &PropKey::from_str(&rt.to_string(&name)?.to_rust_string_lossy()), def)?;
    Ok(app)
}

fn app_component(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    register(rt, c, "components")
}

fn app_directive(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    register(rt, c, "directives")
}

fn app_provide(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let app = app_of(rt, c);
    let p = app_state(rt, c)?;
    let state = payload_ref::<AppState>(&*p).expect("app state type (framework bug)");
    let provides = rt.get_str(&Value::Object(state.context), "provides")?;
    rt.set_v_strict(&provides, &rt.to_property_key(&arg(c, 0))?, arg(c, 1))?;
    Ok(app)
}

fn app_run_with_context(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let app = app_of(rt, c);
    let prev = crate::instance::set_current_app(rt, app);
    let r = rt.call(&arg(c, 0), &Value::Undefined, &[]);
    crate::instance::set_current_app(rt, prev);
    r
}

fn app_on_unmount(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let app = app_of(rt, c);
    let p = app_state(rt, c)?;
    let state = payload_ref::<AppState>(&*p).expect("app state type (framework bug)");
    state.cleanup.borrow_mut().push(arg(c, 0));
    Ok(app)
}

fn app_mixin(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("unsupported: app.mixin (options API)"))
}

/// `createApp(rootComponent, rootProps)` export.
fn create_app_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    create_app(rt, arg(c, 0), arg(c, 1))
}

/// `createSSRApp(rootComponent, rootProps)` export.
fn create_ssr_app_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    create_app_with(rt, arg(c, 0), arg(c, 1), true)
}

/// `render(vnode, container)` export (runtime-dom's render, for a host container).
fn render_export(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let vnode = match arg(c, 0) {
        Value::Object(o) => Some(o),
        _ => None,
    };
    let Some(node) = container_node(rt, &arg(c, 1))? else {
        return Err(rt.throw_type_error("render: the container is not an element"));
    };
    crate::renderer::render_into(rt, vnode, node)?;
    Ok(Value::Undefined)
}

/// The app exports of the `vue` module.
pub(crate) fn exports(rt: &Rt) -> Vec<(&'static str, Value)> {
    vec![
        ("createApp", Value::Object(rt.new_native_function(create_app_export, "createApp", 2, Vec::new()))),
        ("createSSRApp", Value::Object(rt.new_native_function(create_ssr_app_export, "createSSRApp", 2, Vec::new()))),
        ("render", Value::Object(rt.new_native_function(render_export, "render", 3, Vec::new()))),
    ]
}
