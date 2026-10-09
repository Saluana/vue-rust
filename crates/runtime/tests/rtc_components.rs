//! WP-RT-C component differential tests. The scenarios of tests/ui/runtime-c/record-components.mjs are written here with
//! native render and setup functions and the `vue` module exports (createVNode and the lifecycle hooks). Each step mounts
//! the root component with the props of the step and the container's innerHTML must equal what vue@3.5.43 produced in
//! Chrome (tests/ui/runtime-c/components/golden.json).

use vn_runtime::dom_events::DomCtx;
use vn_runtime::host::{Host, HostNode, HostNs};
use vn_runtime::host_doc::DocHost;
use vn_runtime::renderer;
use vn_testkit::json::Json;
use vn_testkit::json_in::parse;
use vn_value::{Call, CodeFn, Flags, JsStr, PropKey, Res, Rt, Value};

/// A realm with the vue exports, a DOM host, the renderer installed and an empty container.
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
        let container = {
            let mut h = host.clone();
            h.create_element(HostNs::Html, "div", None).expect("container")
        };
        Env { rt, ex, host, _ctx: ctx, container }
    }

    fn export(&self, name: &str) -> Value {
        self.ex.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone()).unwrap_or_else(|| panic!("no export {name}"))
    }

    fn func(&self, code: CodeFn, name: &str, len: u32, data: Vec<Value>) -> Value {
        Value::Object(self.rt.new_native_function(code, name, len, data))
    }

    fn obj(&self, pairs: Vec<(&str, Value)>) -> Value {
        ob(&self.rt, &pairs)
    }

    fn arr(&self, items: Vec<Value>) -> Value {
        Value::Object(self.rt.new_array(items))
    }

    /// createVNode(type, props, children).
    fn vnode(&self, ty: Value, props: Value, children: Value) -> Value {
        ok(self.rt.call(&self.export("createVNode"), &Value::Undefined, &[ty, props, children]), "createVNode")
    }

    /// Renders `vnode` (None unmounts) and returns the container's innerHTML.
    fn render(&self, vnode: Option<Value>) -> String {
        let v = match vnode {
            None => None,
            Some(Value::Object(o)) => Some(o),
            Some(_) => panic!("vnode expected"),
        };
        if let Err(Throw(e)) = renderer::render_into(&self.rt, v, self.container) {
            let text = self.rt.to_string(&e).map(|s| s.to_rust_string_lossy()).unwrap_or_else(|_| "?".into());
            panic!("render threw: {text}");
        }
        let mut h = self.host.clone();
        h.inner_html(self.container).expect("container html")
    }

    /// Mounts `root` with the props of each step (None unmounts) and returns the innerHTML after every step.
    fn run(&self, root: &Value, steps: Vec<Option<Value>>) -> Vec<String> {
        steps
            .into_iter()
            .map(|props| {
                let vnode = props.map(|p| self.vnode(root.clone(), p, Value::Null));
                self.render(vnode)
            })
            .collect()
    }
}

use vn_value::Throw;

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

fn get(rt: &Rt, o: &Value, key: &str) -> Value {
    ok(rt.get_str(o, key), "get")
}

fn to_s(rt: &Rt, v: &Value) -> String {
    ok(rt.to_string(v), "to_string").to_rust_string_lossy()
}

/// `a + b + ...` for values of string or number type.
fn concat(rt: &Rt, parts: &[Value]) -> Value {
    let mut out = String::new();
    for p in parts {
        out.push_str(&to_s(rt, p));
    }
    s(&out)
}

fn push(rt: &Rt, arr: &Value, v: Value) {
    let len = get(rt, arr, "length");
    let n = ok(rt.to_number(&len), "length") as u32;
    ok(rt.set_v_strict(arr, &PropKey::Index(n), v), "push");
}

/// `arr.join(sep)`.
fn join(rt: &Rt, arr: &Value, sep: &str) -> String {
    let len = get(rt, arr, "length");
    let n = ok(rt.to_number(&len), "length") as u32;
    let mut parts = Vec::new();
    for i in 0..n {
        let item = ok(rt.get_v(arr, &PropKey::Index(i)), "item");
        parts.push(to_s(rt, &item));
    }
    parts.join(sep)
}

/// The i-th data value of the running native function.
fn data(rt: &Rt, c: &Call<'_>, i: usize) -> Value {
    rt.fn_data(c.callee).get(i).cloned().unwrap_or(Value::Undefined)
}

fn cv(rt: &Rt, c: &Call<'_>, ty: Value, props: Value, children: Value) -> Res<Value> {
    let f = data(rt, c, 0);
    rt.call(&f, &Value::Undefined, &[ty, props, children])
}

fn expected(name: &str) -> Vec<String> {
    let path = format!("{}/../../tests/ui/runtime-c/components/golden.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let Json::Obj(pairs) = parse(&text).expect("golden json") else { panic!("golden is an object") };
    let (_, v) = pairs
        .into_iter()
        .find(|(k, _)| String::from_utf16_lossy(k) == name)
        .unwrap_or_else(|| panic!("no golden for {name}"));
    v.items()
        .expect("steps")
        .iter()
        .map(|st| match st {
            Json::Str(u) => String::from_utf16_lossy(u),
            _ => panic!("step is a string"),
        })
        .collect()
}

fn identity(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

// ----- scenario 1: setup returns a render function -----

fn s1_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let props = ob(rt, &[("class", s("c"))]);
    cv(rt, c, s("p"), props, s("hi"))
}

fn s1_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Object(rt.new_native_function(s1_render, "", 0, vec![data(rt, c, 0)])))
}

// ----- scenario 2: props, attrs fallthrough and class merging -----

fn s2_inner_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let msg = get(rt, c.this, "msg");
    let count = get(rt, c.this, "count");
    let text = concat(rt, &[msg, s(":"), count]);
    let props = ob(rt, &[("class", s("inner"))]);
    cv(rt, c, s("span"), props, text)
}

fn s2_root_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let inner = data(rt, c, 1);
    let props = ob(rt, &[("msg", get(rt, c.this, "msg")), ("count", get(rt, c.this, "n")), ("class", s("outer")), ("id", s("x"))]);
    let f = data(rt, c, 0);
    rt.call(&f, &Value::Undefined, &[inner, props])
}

// ----- scenario 3: boolean casting, kebab-case names and defaults -----

fn s3_factory(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Object(rt.new_array(vec![Value::Number(1.0), Value::Number(2.0)])))
}

fn s3_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let items = get(rt, c.this, "items");
    let items_len = get(rt, &items, "length");
    let text = concat(
        rt,
        &[get(rt, c.this, "flag"), s("|"), get(rt, c.this, "firstName"), s("|"), items_len, s("|"), get(rt, c.this, "n")],
    );
    cv(rt, c, s("i"), Value::Null, text)
}

fn s3_root_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let typed = data(rt, c, 1);
    let props = ob(
        rt,
        &[("flag", get(rt, c.this, "attrsFlag")), ("first-name", get(rt, c.this, "fn")), ("n", get(rt, c.this, "n"))],
    );
    let f = data(rt, c, 0);
    rt.call(&f, &Value::Undefined, &[typed, props])
}

// ----- scenario 4: named and default slots -----

fn s4_slot_header(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(data(rt, c, 0))
}

fn s4_slot_default(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let b = cv(rt, c, s("b"), Value::Null, s("body"))?;
    Ok(Value::Object(rt.new_array(vec![b, s(" tail")])))
}

fn s4_card_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let slots = get(rt, c.this, "$slots");
    let header = get(rt, &slots, "header");
    let default = get(rt, &slots, "default");
    let h_text = if rt.is_callable(&header) { rt.call(&header, &Value::Undefined, &[])? } else { s("none") };
    let d_text = if rt.is_callable(&default) { rt.call(&default, &Value::Undefined, &[])? } else { s("empty") };
    let h2 = cv(rt, c, s("h2"), Value::Null, h_text)?;
    let sec = cv(rt, c, s("section"), Value::Null, d_text)?;
    let children = Value::Object(rt.new_array(vec![h2, sec]));
    cv(rt, c, s("div"), ob(rt, &[("class", s("card"))]), children)
}

fn s4_root_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let card = data(rt, c, 1);
    let header = Value::Object(rt.new_native_function(s4_slot_header, "", 0, vec![get(rt, c.this, "title")]));
    let default = Value::Object(rt.new_native_function(s4_slot_default, "", 0, vec![data(rt, c, 0)]));
    let slots = ob(rt, &[("header", header), ("default", default)]);
    let f = data(rt, c, 0);
    rt.call(&f, &Value::Undefined, &[card, Value::Null, slots])
}

// ----- scenario 5: emits and the lifecycle order -----

fn s5_hook_text(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    push(rt, &data(rt, c, 0), data(rt, c, 1));
    Ok(Value::Undefined)
}

fn s5_mounted(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    push(rt, &data(rt, c, 0), s("m"));
    let v = get(rt, &data(rt, c, 2), "v");
    rt.call(&data(rt, c, 1), &Value::Undefined, &[s("ping"), v])?;
    Ok(Value::Undefined)
}

fn s5_ping(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let x = c.args.first().cloned().unwrap_or(Value::Undefined);
    push(rt, &data(rt, c, 0), concat(rt, &[s("ping:"), x]));
    Ok(Value::Undefined)
}

fn s5_child_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let text = concat(rt, &[s("v="), get(rt, &data(rt, c, 1), "v")]);
    cv(rt, c, s("em"), Value::Null, text)
}

/// `setup(props, { emit })` of the child: registers the hooks in the order of the reference scenario.
fn s5_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let props = c.args.first().cloned().unwrap_or(Value::Undefined);
    let emit = get(rt, c.args.get(1).unwrap_or(&Value::Undefined), "emit");
    let log = data(rt, c, 0);
    let (on_bm, on_m, on_bu, on_u, on_um) = (data(rt, c, 2), data(rt, c, 3), data(rt, c, 4), data(rt, c, 5), data(rt, c, 6));
    let bm = Value::Object(rt.new_native_function(s5_hook_text, "", 0, vec![log.clone(), s("bm")]));
    rt.call(&on_bm, &Value::Undefined, &[bm])?;
    let m = Value::Object(rt.new_native_function(s5_mounted, "", 0, vec![log.clone(), emit, props.clone()]));
    rt.call(&on_m, &Value::Undefined, &[m])?;
    let bu = Value::Object(rt.new_native_function(s5_hook_text, "", 0, vec![log.clone(), s("bu")]));
    rt.call(&on_bu, &Value::Undefined, &[bu])?;
    let u = Value::Object(rt.new_native_function(s5_hook_text, "", 0, vec![log.clone(), s("u")]));
    rt.call(&on_u, &Value::Undefined, &[u])?;
    let um = Value::Object(rt.new_native_function(s5_hook_text, "", 0, vec![log, s("um")]));
    rt.call(&on_um, &Value::Undefined, &[um])?;
    Ok(Value::Object(rt.new_native_function(s5_child_render, "", 0, vec![data(rt, c, 1), props])))
}

fn s5_root_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let log = data(rt, c, 2);
    let p = cv(rt, c, s("p"), Value::Null, s(&join(rt, &log, ",")))?;
    let child = if get(rt, c.this, "show").to_boolean() {
        let on_ping = Value::Object(rt.new_native_function(s5_ping, "", 0, vec![log]));
        let props = ob(rt, &[("v", get(rt, c.this, "v")), ("onPing", on_ping)]);
        cv(rt, c, data(rt, c, 1), props, Value::Null)?
    } else {
        Value::Null
    };
    let children = Value::Object(rt.new_array(vec![p, child]));
    cv(rt, c, s("div"), Value::Null, children)
}

// ----- scenario 6: a render error renders a comment -----

fn s6_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    if get(rt, c.this, "bad").to_boolean() {
        return Err(rt.throw_type_error("boom"));
    }
    cv(rt, c, s("p"), Value::Null, s("ok"))
}

// ----- scenario 7: keyed components -----

fn s7_item_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let text = concat(rt, &[s("item"), get(rt, c.this, "id")]);
    cv(rt, c, s("li"), Value::Null, text)
}

fn s7_root_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let item = data(rt, c, 1);
    let ids = get(rt, c.this, "ids");
    let n = ok(rt.to_number(&get(rt, &ids, "length")), "length") as u32;
    let mut items = Vec::new();
    for i in 0..n {
        let id = ok(rt.get_v(&ids, &PropKey::Index(i)), "id");
        let props = ob(rt, &[("key", id.clone()), ("id", id)]);
        items.push(cv(rt, c, item.clone(), props, Value::Null)?);
    }
    cv(rt, c, s("ul"), Value::Null, Value::Object(rt.new_array(items)))
}

// ----- scenario 8: inheritAttrs false and $attrs -----

fn s8_child_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let attrs = get(rt, c.this, "$attrs");
    let props = ob(rt, &[("title", get(rt, &attrs, "title"))]);
    cv(rt, c, s("span"), props, s("x"))
}

fn s8_root_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let child = data(rt, c, 1);
    let props = ob(rt, &[("title", get(rt, c.this, "t")), ("class", s("k"))]);
    cv(rt, c, child, props, Value::Null)
}

// ----- the scenarios -----

fn scenario_setup_render_fn(env: &Env) -> Vec<String> {
    let setup = env.func(s1_setup, "setup", 0, vec![env.export("createVNode")]);
    let root = env.obj(vec![("setup", setup)]);
    env.run(&root, vec![Some(env.obj(vec![])), None])
}

fn scenario_props_attrs_fallthrough(env: &Env) -> Vec<String> {
    let inner = env.obj(vec![
        ("props", env.arr(vec![s("msg"), s("count")])),
        ("render", env.func(s2_inner_render, "render", 0, vec![env.export("createVNode")])),
    ]);
    let root = env.obj(vec![
        ("props", env.arr(vec![s("msg"), s("n")])),
        ("render", env.func(s2_root_render, "render", 0, vec![env.export("createVNode"), inner])),
    ]);
    env.run(
        &root,
        vec![
            Some(env.obj(vec![("msg", s("a")), ("n", Value::Number(1.0))])),
            Some(env.obj(vec![("msg", s("b")), ("n", Value::Number(1.0))])),
            Some(env.obj(vec![("msg", s("b")), ("n", Value::Number(2.0))])),
        ],
    )
}

fn scenario_props_casting_defaults(env: &Env) -> Vec<String> {
    let typed_props = env.obj(vec![
        ("flag", env.func(identity, "Boolean", 1, vec![])),
        ("first-name", env.obj(vec![("type", env.func(identity, "String", 1, vec![])), ("default", s("anon"))])),
        (
            "items",
            env.obj(vec![
                ("type", env.func(identity, "Array", 1, vec![])),
                ("default", env.func(s3_factory, "", 0, vec![])),
            ]),
        ),
        ("n", env.obj(vec![("type", env.func(identity, "Number", 1, vec![])), ("default", Value::Number(5.0))])),
    ]);
    let typed = env.obj(vec![
        ("props", typed_props),
        ("render", env.func(s3_render, "render", 0, vec![env.export("createVNode")])),
    ]);
    let root = env.obj(vec![
        ("props", env.arr(vec![s("attrsFlag"), s("fn"), s("n")])),
        ("render", env.func(s3_root_render, "render", 0, vec![env.export("createVNode"), typed])),
    ]);
    env.run(
        &root,
        vec![
            Some(env.obj(vec![("attrsFlag", Value::Bool(true)), ("fn", s("Bob")), ("n", Value::Number(3.0))])),
            Some(env.obj(vec![("attrsFlag", s("")), ("fn", Value::Undefined), ("n", Value::Undefined)])),
            Some(env.obj(vec![("attrsFlag", Value::Undefined), ("fn", s("Al")), ("n", s("x"))])),
        ],
    )
}

fn scenario_slots_default_named(env: &Env) -> Vec<String> {
    let card = env.obj(vec![("render", env.func(s4_card_render, "render", 0, vec![env.export("createVNode")]))]);
    let root = env.obj(vec![
        ("props", env.arr(vec![s("title")])),
        ("render", env.func(s4_root_render, "render", 0, vec![env.export("createVNode"), card])),
    ]);
    env.run(&root, vec![Some(env.obj(vec![("title", s("T1"))])), Some(env.obj(vec![("title", s("T2"))]))])
}

fn scenario_emit_and_lifecycle_order(env: &Env) -> Vec<String> {
    let log = env.arr(Vec::new());
    let cvf = env.export("createVNode");
    let child = env.obj(vec![
        ("props", env.arr(vec![s("v")])),
        ("emits", env.arr(vec![s("ping")])),
        (
            "setup",
            env.func(
                s5_setup,
                "setup",
                2,
                vec![
                    log.clone(),
                    cvf.clone(),
                    env.export("onBeforeMount"),
                    env.export("onMounted"),
                    env.export("onBeforeUpdate"),
                    env.export("onUpdated"),
                    env.export("onUnmounted"),
                ],
            ),
        ),
    ]);
    let root = env.obj(vec![
        ("props", env.arr(vec![s("v"), s("show")])),
        ("render", env.func(s5_root_render, "render", 0, vec![cvf, child, log])),
    ]);
    env.run(
        &root,
        vec![
            Some(env.obj(vec![("v", Value::Number(1.0)), ("show", Value::Bool(true))])),
            Some(env.obj(vec![("v", Value::Number(2.0)), ("show", Value::Bool(true))])),
            Some(env.obj(vec![("v", Value::Number(2.0)), ("show", Value::Bool(false))])),
        ],
    )
}

fn scenario_render_error_comment(env: &Env) -> Vec<String> {
    let root = env.obj(vec![
        ("props", env.arr(vec![s("bad")])),
        ("render", env.func(s6_render, "render", 0, vec![env.export("createVNode")])),
    ]);
    env.run(
        &root,
        vec![
            Some(env.obj(vec![("bad", Value::Bool(false))])),
            Some(env.obj(vec![("bad", Value::Bool(true))])),
            Some(env.obj(vec![("bad", Value::Bool(false))])),
        ],
    )
}

fn scenario_keyed_component_list(env: &Env) -> Vec<String> {
    let item = env.obj(vec![
        ("props", env.arr(vec![s("id")])),
        ("render", env.func(s7_item_render, "render", 0, vec![env.export("createVNode")])),
    ]);
    let root = env.obj(vec![
        ("props", env.arr(vec![s("ids")])),
        ("render", env.func(s7_root_render, "render", 0, vec![env.export("createVNode"), item])),
    ]);
    let ids = |v: &[f64]| env.arr(v.iter().map(|n| Value::Number(*n)).collect());
    env.run(
        &root,
        vec![
            Some(env.obj(vec![("ids", ids(&[1.0, 2.0, 3.0]))])),
            Some(env.obj(vec![("ids", ids(&[3.0, 1.0, 2.0]))])),
            Some(env.obj(vec![("ids", ids(&[2.0]))])),
            Some(env.obj(vec![("ids", ids(&[4.0, 2.0, 5.0]))])),
        ],
    )
}

fn scenario_inherit_attrs_false(env: &Env) -> Vec<String> {
    let child = env.obj(vec![
        ("inheritAttrs", Value::Bool(false)),
        ("render", env.func(s8_child_render, "render", 0, vec![env.export("createVNode")])),
    ]);
    let root = env.obj(vec![
        ("props", env.arr(vec![s("t")])),
        ("render", env.func(s8_root_render, "render", 0, vec![env.export("createVNode"), child])),
    ]);
    env.run(&root, vec![Some(env.obj(vec![("t", s("a"))])), Some(env.obj(vec![("t", s("b"))]))])
}

#[test]
fn component_scenarios_match_vue_in_chrome() {
    let scenarios: Vec<(&str, fn(&Env) -> Vec<String>)> = vec![
        ("setup-render-fn", scenario_setup_render_fn),
        ("props-attrs-fallthrough", scenario_props_attrs_fallthrough),
        ("props-casting-defaults", scenario_props_casting_defaults),
        ("slots-default-named", scenario_slots_default_named),
        ("emit-and-lifecycle-order", scenario_emit_and_lifecycle_order),
        ("render-error-comment", scenario_render_error_comment),
        ("keyed-component-list", scenario_keyed_component_list),
        ("inherit-attrs-false", scenario_inherit_attrs_false),
    ];
    let mut failures = Vec::new();
    for (name, build) in &scenarios {
        let env = Env::new();
        let actual = build(&env);
        let want = expected(name);
        if actual.len() != want.len() {
            failures.push(format!("{name}: {} steps, vue has {}", actual.len(), want.len()));
        }
        for (i, (a, e)) in actual.iter().zip(&want).enumerate() {
            if a != e {
                failures.push(format!("{name} step {i}: got {a:?}, vue has {e:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{} mismatching steps:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn negative_control_detects_a_corrupted_component_expectation() {
    let env = Env::new();
    let actual = scenario_props_attrs_fallthrough(&env);
    let mut want = expected("props-attrs-fallthrough");
    assert_eq!(actual, want, "the correct expectation must match");
    want[0] = want[0].replace("a:1", "a:2");
    let mismatches = actual.iter().zip(&want).filter(|(a, e)| a != e).count();
    assert_eq!(mismatches, 1, "the corrupted expectation must be reported");
}

#[test]
fn mount_unmount_1000_components_returns_to_the_live_object_baseline() {
    let env = Env::new();
    let item = env.obj(vec![
        ("props", env.arr(vec![s("id")])),
        ("render", env.func(s7_item_render, "render", 0, vec![env.export("createVNode")])),
    ]);
    let root = env.obj(vec![
        ("props", env.arr(vec![s("ids")])),
        ("render", env.func(s7_root_render, "render", 0, vec![env.export("createVNode"), item])),
    ]);
    // The test holds `root` in a Rust local, which the collector does not see: root it for the whole test.
    let _root_id = env.rt.root(&root);
    let cycle = |env: &Env| {
        let ids = env.arr((0..1000).map(|i| Value::Number(i as f64)).collect());
        let props = env.obj(vec![("ids", ids)]);
        let vnode = env.vnode(root.clone(), props, Value::Null);
        let _ = env.render(Some(vnode));
        let _ = env.render(None);
    };
    cycle(&env);
    env.rt.collect();
    let baseline = env.rt.heap_stats().live_objects;
    for round in 0..3 {
        cycle(&env);
        env.rt.collect();
        let live = env.rt.heap_stats().live_objects;
        assert_eq!(live, baseline, "round {round}: live objects after unmounting 1000 components");
    }
}
