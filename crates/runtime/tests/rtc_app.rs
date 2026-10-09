//! WP-RT-C app, function-ref and directive differential tests. The scenarios of tests/ui/runtime-c/record-app.mjs are
//! written here with the vue exports (createApp, withDirectives, createVNode) and native functions. The app mounts into
//! a `#app` element of the document; the outerHTML after each step and the callback log must equal what vue@3.5.43
//! produced in Chrome (tests/ui/runtime-c/app/golden.json).

use vn_runtime::dom_events::DomCtx;
use vn_runtime::host::{Host, HostNode, HostNs};
use vn_runtime::host_doc::DocHost;
use vn_runtime::renderer;
use vn_testkit::json::Json;
use vn_testkit::json_in::parse;
use vn_value::{Call, Flags, JsStr, PropKey, Res, Rt, Value};

struct Env {
    rt: Rt,
    ex: Vec<(&'static str, Value)>,
    host: DocHost,
    _ctx: DomCtx,
    container: HostNode,
}

fn ok<T>(r: Res<T>, what: &str) -> T {
    match r {
        Ok(v) => v,
        Err(_) => panic!("{what}: the runtime threw"),
    }
}

impl Env {
    fn new() -> Env {
        let rt = Rt::new();
        let ex = vn_runtime::exports::all(&rt);
        let host = DocHost::new();
        let ctx = DomCtx::new(&rt, &host);
        renderer::install(&ctx);
        let body = {
            let mut h = host.clone();
            ok(h.install_body().map_err(|_| ()).map_err(|_| vn_value::Throw(Value::Null)), "body")
        };
        let container = {
            let mut h = host.clone();
            let d = h.create_element(HostNs::Html, "div", None).expect("container");
            h.set_attribute(d, "id", "app").expect("id");
            h.insert(d, body, None).expect("insert");
            d
        };
        Env { rt, ex, host, _ctx: ctx, container }
    }

    fn export(&self, name: &str) -> Value {
        self.ex.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone()).unwrap_or_else(|| panic!("no export {name}"))
    }

    fn outer(&self) -> String {
        let mut h = self.host.clone();
        h.outer_html(self.container).expect("outer html")
    }
}

fn s(text: &str) -> Value {
    Value::String(JsStr::from_str(text))
}

fn ob(rt: &Rt, pairs: &[(&str, Value)]) -> Value {
    let o = rt.new_object();
    for (k, v) in pairs {
        ok(rt.define_data(o, PropKey::from_str(k), v.clone(), Flags::ALL), "define");
    }
    Value::Object(o)
}

fn data(rt: &Rt, c: &Call<'_>, i: usize) -> Value {
    rt.fn_data(c.callee).get(i).cloned().unwrap_or(Value::Undefined)
}

fn arg(c: &Call<'_>, i: usize) -> Value {
    c.args.get(i).cloned().unwrap_or(Value::Undefined)
}

fn push_text(rt: &Rt, log: &Value, text: String) {
    let len = ok(rt.get_str(log, "length"), "length");
    let n = ok(rt.to_number(&len), "length") as u32;
    ok(rt.set_v_strict(log, &PropKey::Index(n), Value::String(JsStr::from_str(&text))), "push");
}

fn log_items(rt: &Rt, log: &Value) -> Vec<String> {
    let len = ok(rt.get_str(log, "length"), "length");
    let n = ok(rt.to_number(&len), "length") as u32;
    (0..n)
        .map(|i| {
            let v = ok(rt.get_v(log, &PropKey::Index(i)), "item");
            ok(rt.to_string(&v), "string").to_rust_string_lossy()
        })
        .collect()
}

fn expected(name: &str) -> (Vec<String>, Vec<String>) {
    let path = format!("{}/../../tests/ui/runtime-c/app/golden.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let Json::Obj(pairs) = parse(&text).expect("golden json") else { panic!("golden is an object") };
    let (_, v) = pairs
        .into_iter()
        .find(|(k, _)| String::from_utf16_lossy(k) == name)
        .unwrap_or_else(|| panic!("no golden for {name}"));
    let strings = |j: &Json| -> Vec<String> {
        j.items()
            .expect("array")
            .iter()
            .map(|x| match x {
                Json::Str(u) => String::from_utf16_lossy(u),
                _ => panic!("string expected"),
            })
            .collect()
    };
    let snaps = strings(v.field("snaps").expect("snaps"));
    let log = strings(v.field("log").expect("log"));
    (snaps, log)
}

// ----- scenario 1: mount through a selector, unmount -----

fn scenario_app_mount_unmount(env: &Env) -> (Vec<String>, Vec<String>) {
    let rt = &env.rt;
    let render = Value::Object(rt.new_native_function(render_hello, "render", 0, Vec::new()));
    let root = ob(rt, &[("render", render)]);
    let app = ok(rt.call(&env.export("createApp"), &Value::Undefined, &[root, Value::Null]), "createApp");
    let mut snaps = Vec::new();
    ok(rt.call(&ok(rt.get_str(&app, "mount"), "mount"), &app, &[s("#app")]), "mount");
    snaps.push(env.outer());
    ok(rt.call(&ok(rt.get_str(&app, "unmount"), "unmount"), &app, &[]), "unmount");
    snaps.push(env.outer());
    (snaps, Vec::new())
}

fn render_hello(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let cv = vue_create_vnode(rt, c)?;
    rt.call(&cv, &Value::Undefined, &[s("p"), Value::Null, s("hello")])
}

/// createVNode of the `vue` exports, reached from a render function through the realm's builtin.
fn vue_create_vnode(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Object(rt.builtin("vn-runtime.createVNode").expect("createVNode builtin")))
}

// ----- scenario 2: function refs -----

fn ref_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let log = data(rt, c, 0);
    let el = arg(c, 0);
    let text = if el.is_nullish() {
        "null".to_string()
    } else {
        let tag = rt.get_str(&el, "tagName")?;
        format!("el:{}", rt.to_string(&tag)?.to_rust_string_lossy())
    };
    push_text(rt, &log, text);
    Ok(Value::Undefined)
}

fn render_ref_root(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let log = data(rt, c, 0);
    let cv = vue_create_vnode(rt, c)?;
    let props = ob(rt, &[("ref", Value::Object(rt.new_native_function(ref_fn, "ref", 2, vec![log.clone()])))]);
    let v = rt.get_str(c.this, "v")?;
    let text = rt.to_string(&v)?.to_rust_string_lossy();
    rt.call(&cv, &Value::Undefined, &[s("p"), props, s(&format!("x{text}"))])
}

fn scenario_function_ref_order(env: &Env) -> (Vec<String>, Vec<String>) {
    let rt = &env.rt;
    let log = Value::Object(rt.new_array(Vec::new()));
    let render = Value::Object(rt.new_native_function(render_ref_root, "render", 0, vec![log.clone()]));
    let root = ob(rt, &[("props", Value::Object(rt.new_array(vec![s("v")]))), ("render", render)]);
    let app = ok(rt.call(&env.export("createApp"), &Value::Undefined, &[root, ob(rt, &[("v", Value::Number(7.0))])]), "createApp");
    let mut snaps = Vec::new();
    ok(rt.call(&ok(rt.get_str(&app, "mount"), "mount"), &app, &[s("#app")]), "mount");
    snaps.push(env.outer());
    ok(rt.call(&ok(rt.get_str(&app, "unmount"), "unmount"), &app, &[]), "unmount");
    snaps.push(env.outer());
    (snaps, log_items(rt, &log))
}

// ----- scenario 3: directive hooks -----

fn dir_push(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let text = rt.to_string(&data(rt, c, 1))?.to_rust_string_lossy();
    push_text(rt, &data(rt, c, 0), text);
    Ok(Value::Undefined)
}

fn dir_mounted(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let log = data(rt, c, 0);
    let el = arg(c, 0);
    let binding = arg(c, 1);
    let value = rt.get_str(&binding, "value")?;
    let set_attribute = rt.get_str(&el, "setAttribute")?;
    let v = ok(rt.to_string(&value), "value").to_rust_string_lossy();
    rt.call(&set_attribute, &el, &[s("data-m"), s(&v)])?;
    push_text(rt, &log, format!("mounted:{v}"));
    Ok(Value::Undefined)
}

fn render_directive_root(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let dir = data(rt, c, 0);
    let with = data(rt, c, 1);
    let cv = vue_create_vnode(rt, c)?;
    let vnode = rt.call(&cv, &Value::Undefined, &[s("p"), Value::Null, s("x")])?;
    let entry = Value::Object(rt.new_array(vec![dir, Value::Number(42.0)]));
    let list = Value::Object(rt.new_array(vec![entry]));
    rt.call(&with, &Value::Undefined, &[vnode, list])
}

fn scenario_directive_hooks(env: &Env) -> (Vec<String>, Vec<String>) {
    let rt = &env.rt;
    let log = Value::Object(rt.new_array(Vec::new()));
    let hook = |name: &'static str| Value::Object(rt.new_native_function(dir_push, name, 0, vec![log.clone(), s(name)]));
    let mounted = Value::Object(rt.new_native_function(dir_mounted, "mounted", 2, vec![log.clone()]));
    let dir = ob(
        rt,
        &[
            ("created", hook("created")),
            ("beforeMount", hook("beforeMount")),
            ("mounted", mounted),
            ("beforeUnmount", hook("beforeUnmount")),
            ("unmounted", hook("unmounted")),
        ],
    );
    let render = Value::Object(rt.new_native_function(render_directive_root, "render", 0, vec![dir, env.export("withDirectives")]));
    let root = ob(rt, &[("render", render)]);
    let app = ok(rt.call(&env.export("createApp"), &Value::Undefined, &[root, Value::Null]), "createApp");
    let mut snaps = Vec::new();
    ok(rt.call(&ok(rt.get_str(&app, "mount"), "mount"), &app, &[s("#app")]), "mount");
    snaps.push(env.outer());
    ok(rt.call(&ok(rt.get_str(&app, "unmount"), "unmount"), &app, &[]), "unmount");
    snaps.push(env.outer());
    (snaps, log_items(rt, &log))
}

// ----- scenario 4: a ref changed after mount re-renders through the scheduler -----

fn count_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let count = data(rt, c, 0);
    let v = rt.get_str(&count, "value")?;
    let text = rt.to_string(&v)?.to_rust_string_lossy();
    let cv = vue_create_vnode(rt, c)?;
    rt.call(&cv, &Value::Undefined, &[s("b"), Value::Null, s(&text)])
}

fn count_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let holder = data(rt, c, 0);
    let make_ref = data(rt, c, 1);
    let count = rt.call(&make_ref, &Value::Undefined, &[Value::Number(0.0)])?;
    rt.set_str(&holder, "count", count.clone())?;
    Ok(Value::Object(rt.new_native_function(count_render, "render", 0, vec![count])))
}

fn scenario_reactive_ref_update(env: &Env) -> (Vec<String>, Vec<String>) {
    let rt = &env.rt;
    let holder = ob(rt, &[]);
    let setup = Value::Object(rt.new_native_function(count_setup, "setup", 0, vec![holder.clone(), env.export("ref")]));
    let root = ob(rt, &[("setup", setup)]);
    let app = ok(rt.call(&env.export("createApp"), &Value::Undefined, &[root, Value::Null]), "createApp");
    let mut snaps = Vec::new();
    ok(rt.call(&ok(rt.get_str(&app, "mount"), "mount"), &app, &[s("#app")]), "mount");
    snaps.push(env.outer());
    let count = ok(rt.get_str(&holder, "count"), "count");
    ok(rt.set_str(&count, "value", Value::Number(5.0)), "set");
    rt.run_jobs();
    snaps.push(env.outer());
    ok(rt.set_str(&count, "value", Value::Number(9.0)), "set");
    rt.run_jobs();
    snaps.push(env.outer());
    ok(rt.call(&ok(rt.get_str(&app, "unmount"), "unmount"), &app, &[]), "unmount");
    snaps.push(env.outer());
    (snaps, Vec::new())
}

// ----- scenario 5: scoped styles (__scopeId) and slot scope ids -----

fn cv_call(rt: &Rt, ty: Value, props: Value, children: Value) -> Res<Value> {
    let f = Value::Object(rt.builtin("vn-runtime.createVNode").expect("createVNode builtin"));
    rt.call(&f, &Value::Undefined, &[ty, props, children])
}

fn scoped_child_render(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    cv_call(rt, s("p"), Value::Null, s("c"))
}

fn scoped_card_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let render_slot = data(rt, c, 0);
    let slots = rt.get_str(c.this, "$slots")?;
    let frag = rt.call(&render_slot, &Value::Undefined, &[slots, s("default")])?;
    let list = Value::Object(rt.new_array(vec![frag]));
    cv_call(rt, s("section"), Value::Null, list)
}

fn scoped_slot_b(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    cv_call(rt, s("b"), Value::Null, s("x"))
}

fn scoped_root_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let child = data(rt, c, 0);
    let card = data(rt, c, 1);
    let span = cv_call(rt, s("span"), Value::Null, s("s"))?;
    let child_v = cv_call(rt, child, Value::Null, Value::Null)?;
    let default = Value::Object(rt.new_native_function(scoped_slot_b, "default", 0, Vec::new()));
    let slots = ob(rt, &[("default", default)]);
    let card_v = cv_call(rt, card, Value::Null, slots)?;
    let children = Value::Object(rt.new_array(vec![span, child_v, card_v]));
    cv_call(rt, s("div"), Value::Null, children)
}

fn scenario_scoped_styles(env: &Env) -> (Vec<String>, Vec<String>) {
    let rt = &env.rt;
    let child = ob(rt, &[("__scopeId", s("data-v-child")), ("render", Value::Object(rt.new_native_function(scoped_child_render, "render", 0, Vec::new())))]);
    let card = ob(
        rt,
        &[
            ("__scopeId", s("data-v-card")),
            ("render", Value::Object(rt.new_native_function(scoped_card_render, "render", 0, vec![env.export("renderSlot")]))),
        ],
    );
    let root = ob(
        rt,
        &[
            ("__scopeId", s("data-v-root")),
            ("render", Value::Object(rt.new_native_function(scoped_root_render, "render", 0, vec![child, card]))),
        ],
    );
    let app = ok(rt.call(&env.export("createApp"), &Value::Undefined, &[root, Value::Null]), "createApp");
    let mut snaps = Vec::new();
    ok(rt.call(&ok(rt.get_str(&app, "mount"), "mount"), &app, &[s("#app")]), "mount");
    snaps.push(env.outer());
    ok(rt.call(&ok(rt.get_str(&app, "unmount"), "unmount"), &app, &[]), "unmount");
    snaps.push(env.outer());
    (snaps, Vec::new())
}

// ----- scenario 6: useModel with an emit round trip -----

fn model_child_mounted(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    rt.set_str(&data(rt, c, 0), "value", s("child"))?;
    Ok(Value::Undefined)
}

fn model_child_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let v = rt.get_str(&data(rt, c, 0), "value")?;
    let text = concat_text(rt, "c:", &v)?;
    cv_call(rt, s("i"), Value::Null, text)
}

fn model_child_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let use_model = data(rt, c, 0);
    let on_mounted = data(rt, c, 1);
    let props = arg(c, 0);
    let model = rt.call(&use_model, &Value::Undefined, &[props, s("modelValue")])?;
    let hook = Value::Object(rt.new_native_function(model_child_mounted, "", 0, vec![model.clone()]));
    rt.call(&on_mounted, &Value::Undefined, &[hook])?;
    Ok(Value::Object(rt.new_native_function(model_child_render, "render", 0, vec![model])))
}

fn model_root_update(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    rt.set_str(&data(rt, c, 0), "value", arg(c, 0))?;
    Ok(Value::Undefined)
}

fn model_root_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let v = data(rt, c, 0);
    let child = data(rt, c, 1);
    let value = rt.get_str(&v, "value")?;
    let p_text = concat_text(rt, "p:", &value)?;
    let p = cv_call(rt, s("b"), Value::Null, p_text)?;
    let on_update = Value::Object(rt.new_native_function(model_root_update, "", 1, vec![v.clone()]));
    let props = ob(rt, &[("modelValue", value), ("onUpdate:modelValue", on_update)]);
    let child_v = cv_call(rt, child, props, Value::Null)?;
    cv_call(rt, s("div"), Value::Null, Value::Object(rt.new_array(vec![p, child_v])))
}

fn model_root_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let make_ref = data(rt, c, 0);
    let child = data(rt, c, 1);
    let v = rt.call(&make_ref, &Value::Undefined, &[s("start")])?;
    Ok(Value::Object(rt.new_native_function(model_root_render, "render", 0, vec![v, child])))
}

fn concat_text(rt: &Rt, prefix: &str, v: &Value) -> Res<Value> {
    let text = rt.to_string(v)?.to_rust_string_lossy();
    Ok(s(&format!("{prefix}{text}")))
}

fn scenario_use_model_emit(env: &Env) -> (Vec<String>, Vec<String>) {
    let rt = &env.rt;
    let child = ob(
        rt,
        &[
            ("props", Value::Object(rt.new_array(vec![s("modelValue")]))),
            ("emits", Value::Object(rt.new_array(vec![s("update:modelValue")]))),
            ("setup", Value::Object(rt.new_native_function(model_child_setup, "setup", 1, vec![env.export("useModel"), env.export("onMounted")]))),
        ],
    );
    let root = ob(rt, &[("setup", Value::Object(rt.new_native_function(model_root_setup, "setup", 0, vec![env.export("ref"), child])))]);
    let app = ok(rt.call(&env.export("createApp"), &Value::Undefined, &[root, Value::Null]), "createApp");
    let mut snaps = Vec::new();
    ok(rt.call(&ok(rt.get_str(&app, "mount"), "mount"), &app, &[s("#app")]), "mount");
    snaps.push(env.outer());
    rt.run_jobs();
    snaps.push(env.outer());
    ok(rt.call(&ok(rt.get_str(&app, "unmount"), "unmount"), &app, &[]), "unmount");
    snaps.push(env.outer());
    (snaps, Vec::new())
}

// ----- scenario 7: mergeDefaults, mergeModels and createPropsRestProxy -----

fn helper_md_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let label = rt.get_str(c.this, "label")?;
    cv_call(rt, s("i"), Value::Null, label)
}

fn helper_mm_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let a = rt.to_string(&rt.get_str(c.this, "a")?)?.to_rust_string_lossy();
    let b = rt.to_string(&rt.get_str(c.this, "b")?)?.to_rust_string_lossy();
    cv_call(rt, s("u"), Value::Null, s(&format!("{a}{b}")))
}

fn helper_rp_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let rest = data(rt, c, 0);
    let Value::Object(o) = rest else { return Err(rt.throw_type_error("rest object expected")) };
    let keys: Vec<String> = rt
        .own_keys(o)?
        .into_iter()
        .filter_map(|k| match k {
            PropKey::Str(js) => Some(js.to_rust_string_lossy()),
            _ => None,
        })
        .collect();
    cv_call(rt, s("s"), Value::Null, s(&keys.join(",")))
}

fn helper_rp_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let rest_fn = data(rt, c, 0);
    let rest = rt.call(&rest_fn, &Value::Undefined, &[arg(c, 0), Value::Object(rt.new_array(vec![s("a")]))])?;
    Ok(Value::Object(rt.new_native_function(helper_rp_render, "render", 0, vec![rest])))
}

fn helper_root_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let md = data(rt, c, 0);
    let mm = data(rt, c, 1);
    let rp = data(rt, c, 2);
    let a = cv_call(rt, md, Value::Null, Value::Null)?;
    let mm_props = ob(rt, &[("a", s("1")), ("b", s("2"))]);
    let b = cv_call(rt, mm, mm_props, Value::Null)?;
    let rp_props = ob(rt, &[("a", s("x")), ("b", s("y")), ("c", s("z"))]);
    let c3 = cv_call(rt, rp, rp_props, Value::Null)?;
    cv_call(rt, s("div"), Value::Null, Value::Object(rt.new_array(vec![a, b, c3])))
}

fn scenario_props_helpers(env: &Env) -> (Vec<String>, Vec<String>) {
    let rt = &env.rt;
    let arr = |items: &[&str]| Value::Object(rt.new_array(items.iter().map(|t| s(t)).collect()));
    let md_props = ok(
        rt.call(&env.export("mergeDefaults"), &Value::Undefined, &[arr(&["label"]), ob(rt, &[("label", s("dflt"))])]),
        "mergeDefaults",
    );
    let md = ob(rt, &[("props", md_props), ("render", Value::Object(rt.new_native_function(helper_md_render, "render", 0, Vec::new())))]);
    let mm_props = ok(rt.call(&env.export("mergeModels"), &Value::Undefined, &[arr(&["a"]), arr(&["b"])]), "mergeModels");
    let mm = ob(rt, &[("props", mm_props), ("render", Value::Object(rt.new_native_function(helper_mm_render, "render", 0, Vec::new())))]);
    let rp = ob(
        rt,
        &[
            ("props", arr(&["a", "b", "c"])),
            ("setup", Value::Object(rt.new_native_function(helper_rp_setup, "setup", 1, vec![env.export("createPropsRestProxy")]))),
        ],
    );
    let root = ob(
        rt,
        &[("render", Value::Object(rt.new_native_function(helper_root_render, "render", 0, vec![md, mm, rp])))],
    );
    let app = ok(rt.call(&env.export("createApp"), &Value::Undefined, &[root, Value::Null]), "createApp");
    let mut snaps = Vec::new();
    ok(rt.call(&ok(rt.get_str(&app, "mount"), "mount"), &app, &[s("#app")]), "mount");
    snaps.push(env.outer());
    ok(rt.call(&ok(rt.get_str(&app, "unmount"), "unmount"), &app, &[]), "unmount");
    snaps.push(env.outer());
    (snaps, Vec::new())
}

#[test]
fn app_scenarios_match_vue_in_chrome() {
    let cases: Vec<(&str, fn(&Env) -> (Vec<String>, Vec<String>))> = vec![
        ("app-mount-unmount", scenario_app_mount_unmount),
        ("function-ref-order", scenario_function_ref_order),
        ("directive-hooks", scenario_directive_hooks),
        ("reactive-ref-update", scenario_reactive_ref_update),
        ("scoped-styles", scenario_scoped_styles),
        ("use-model-emit", scenario_use_model_emit),
        ("props-helpers", scenario_props_helpers),
    ];
    let mut failures = Vec::new();
    for (name, build) in cases {
        let env = Env::new();
        let (snaps, log) = build(&env);
        let (want_snaps, want_log) = expected(name);
        if snaps != want_snaps {
            failures.push(format!("{name}: snapshots {snaps:?}, vue has {want_snaps:?}"));
        }
        if log != want_log {
            failures.push(format!("{name}: log {log:?}, vue has {want_log:?}"));
        }
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn negative_control_detects_a_corrupted_app_expectation() {
    let env = Env::new();
    let (snaps, _) = scenario_app_mount_unmount(&env);
    let (want, _) = expected("app-mount-unmount");
    assert_eq!(snaps, want, "the correct expectation must match");
    let mut wrong = want.clone();
    wrong[0] = wrong[0].replace("hello", "HELLO");
    assert_ne!(snaps, wrong, "the corrupted expectation must be reported");
}
