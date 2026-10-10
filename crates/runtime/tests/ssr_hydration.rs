//! The SSR and hydration probe of task 3.6 (tickets/w36-ssr): `renderToString(app)` (`ssr::render_app_to_string`) and
//! `createSSRApp(...).mount("#app")` (`renderer::hydrate_into`) over the subset of tests/ssr-oracle/probe.mjs.
//!
//! The expected HTML is the output of `node tests/ssr-oracle/probe.mjs` (part (a): vue@3.5.43 with @vue/server-renderer),
//! copied into `ORACLE` from that output. Every byte of the Rust output must equal it (`compare_html`). The hydration tests
//! build that server markup as DOM nodes (`parse_markup`, a parser for the markup the server renderer writes) and check what
//! the client does with those nodes: node identity (the same handles), host node creations, listeners, patches, the value
//! of an input, and the mismatch log. Each realm (`Rt`) has its own renderer, DOM host and log; no state is shared.

use vn_runtime::dom_events::DomCtx;
use vn_runtime::host::{Host, HostNode, HostNs, HostValue};
use vn_runtime::host_doc::DocHost;
use vn_runtime::{renderer, ssr};
use vn_value::{Call, CodeFn, Flags, JsStr, PropKey, Res, Rt, Throw, Value};

/// `node tests/ssr-oracle/probe.mjs`, part (a): one line per probe, `name<TAB>html`. Copied from that output.
const ORACLE: &[(&str, &str)] = &[
    ("static-tree", r#"<div id="root" class="a b" style="color: red; margin: 0"><p>Hello &amp; &lt;world&gt; &quot;q&quot; &#39;s</p><img alt="x&quot;y" src="a.png"><br><span title="it&#39;s">end</span></div>"#),
    ("attrs-style", r#"<div class="x y" style="color:blue;font-size:12px;opacity:0.5;" data-n="3" disabled tabindex="-1">attrs</div>"#),
    ("interpolation", r#"<p data-msg="&lt;b&gt;&quot;x&quot; &amp; &#39;y&#39;&lt;/b&gt;">Count: 0 &amp; &lt;b&gt;&quot;x&quot; &amp; &#39;y&#39;&lt;/b&gt;</p>"#),
    ("cond-false", r#"<div><!----></div>"#),
    ("cond-true", r#"<div><p>shown 2</p></div>"#),
    ("keyed-list", r#"<ul><!--[--><li>a</li><li>b</li><li>c &amp; d</li><!--]--></ul>"#),
    ("keyed-empty", r#"<ul><!--[--><!--]--></ul>"#),
    ("child-props-emit", r#"<button class="child" data-x="v">ping L</button>"#),
    ("input-v-model", r#"<input type="text" value="Hello">"#),
    ("click-button", r#"<button>Count: 0</button>"#),
    ("app", r#"<div class="app"><h1 title="Hi">Hi &amp; 1</h1><button class="child" data-x="v">ping Hi</button><p>shown</p><ul><!--[--><li>a</li><li>b</li><li>c &amp; d</li><!--]--></ul><input type="text" value="Hi"><button>Count: 1</button></div>"#),
    ("input-typed", r#"<div><input type="text" value="Hello"><p>Typed: Hello</p></div>"#),
    ("attr-edge", r#"<div hidden title id="0" data-e data-f="false" required class="">edge</div>"#),
    ("comment-escape", r#"<div><!--a  b  c  d --><!-- lead ->> x--></div>"#),
    ("module-counter-first", r#"<i>1</i>"#),
    ("module-counter-second", r#"<i>2</i>"#),
    ("comment-with-text", r#"<!--v-if-->"#),
];

/// The probes that hydrate their own server markup (the module counter is checked by the SSR tests only).
const HYDRATED_PROBES: &[&str] = &[
    "static-tree",
    "attrs-style",
    "interpolation",
    "cond-false",
    "cond-true",
    "keyed-list",
    "keyed-empty",
    "child-props-emit",
    "input-v-model",
    "click-button",
    "app",
    "input-typed",
    "comment-with-text",
];

/// The constructs outside the subset. Each one must throw a TypeError whose message names the phase and the construct.
/// Static (hoisted) vnodes are not listed: `createStaticVNode` refuses to create one (see `static_vnodes_cannot_be_created`).
const UNSUPPORTED: &[&str] = &["Teleport", "Transition", "KeepAlive", "Suspense", "async component"];

/// The list of the `app` probe and of the keyed-list probes: `[id, label]`.
const ITEMS: &[(f64, &str)] = &[(1.0, "a"), (2.0, "b"), (3.0, "c & d")];

fn oracle(name: &str) -> &'static str {
    ORACLE.iter().find(|(n, _)| *n == name).map(|(_, html)| *html).unwrap_or_else(|| panic!("no oracle line {name}"))
}

/// The byte comparison of the probes: `Ok` only when the two strings are identical; otherwise the first byte that differs.
fn compare_html(actual: &str, expected: &str) -> Result<(), String> {
    if actual == expected {
        return Ok(());
    }
    let at = actual.bytes().zip(expected.bytes()).position(|(a, e)| a != e).unwrap_or(actual.len().min(expected.len()));
    Err(format!("HTML differs at byte {at}\n  actual   {actual:?}\n  expected {expected:?}"))
}

// ----- realm helpers -----

/// A realm: an `Rt` with the vue exports, a DOM host with the renderer installed, `#app` inside the body, and the bag the
/// probe components use (the vnode helpers, `reactive`, the child component and this realm's module-level counter).
struct Realm {
    rt: Rt,
    ex: Vec<(&'static str, Value)>,
    host: DocHost,
    _ctx: DomCtx,
    container: HostNode,
    bag: Value,
}

impl Realm {
    fn new() -> Realm {
        let rt = Rt::new();
        let ex = vn_runtime::exports::all(&rt);
        let host = DocHost::new();
        let ctx = DomCtx::new(&rt, &host);
        renderer::install(&ctx);
        let mut h = host.clone();
        let body = h.install_body().expect("body");
        let container = h.create_element(HostNs::Html, "div", None).expect("container");
        h.set_attribute(container, "id", "app").expect("id");
        h.insert(container, body, None).expect("insert the container");
        let bag = make_bag(&rt, &ex);
        Realm { rt, ex, host, _ctx: ctx, container, bag }
    }

    fn export(&self, name: &str) -> Value {
        self.ex.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone()).unwrap_or_else(|| panic!("no export {name}"))
    }

    fn handle(&self) -> DocHost {
        self.host.clone()
    }
}

fn make_bag(rt: &Rt, ex: &[(&'static str, Value)]) -> Value {
    let find = |name: &str| ex.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone()).unwrap_or_else(|| panic!("no export {name}"));
    let reactive = find("reactive");
    let module_state = call(rt, &reactive, &Value::Undefined, &[ob(rt, &[("hits", num(0.0))])]);
    // `setup(props, { emit })` has two parameters, so the setup context is created (Vue: setup.length > 1).
    let setup = Value::Object(rt.new_native_function(child_setup, "setup", 2, vec![]));
    let child = ob(rt, &[("props", arr(rt, vec![s("label")])), ("emits", arr(rt, vec![s("ping")])), ("setup", setup)]);
    ob(
        rt,
        &[
            ("createCommentVNode", find("createCommentVNode")),
            ("Fragment", find("Fragment")),
            ("withDirectives", find("withDirectives")),
            ("vModelText", find("vModelText")),
            ("reactive", reactive),
            ("Child", child),
            ("moduleState", module_state),
        ],
    )
}

fn ok<T>(rt: &Rt, r: Res<T>, what: &str) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("{what} threw {}", describe(rt, &e)),
    }
}

/// `name: message` of a thrown value.
fn describe(rt: &Rt, e: &Throw) -> String {
    let (name, message) = error_of(rt, e);
    format!("{name}: {message}")
}

/// The `name` and `message` of a thrown error object.
fn error_of(rt: &Rt, e: &Throw) -> (String, String) {
    let name = text(rt, &rt.get_str(&e.0, "name").expect("error name"));
    let message = text(rt, &rt.get_str(&e.0, "message").expect("error message"));
    (name, message)
}

fn call(rt: &Rt, f: &Value, this: &Value, args: &[Value]) -> Value {
    ok(rt, rt.call(f, this, args), "call")
}

fn get(rt: &Rt, base: &Value, key: &str) -> Value {
    ok(rt, rt.get_str(base, key), key)
}

fn set(rt: &Rt, base: &Value, key: &str, v: Value) {
    ok(rt, rt.set_v_strict(base, &PropKey::from_str(key), v), key);
}

fn text(rt: &Rt, v: &Value) -> String {
    ok(rt, rt.to_string(v), "string").to_rust_string_lossy()
}

fn s(text: &str) -> Value {
    Value::String(JsStr::from_str(text))
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

fn ob(rt: &Rt, pairs: &[(&str, Value)]) -> Value {
    let o = rt.new_object();
    for (k, v) in pairs {
        ok(rt, rt.define_data(o, PropKey::from_str(k), v.clone(), Flags::ALL), "define");
    }
    Value::Object(o)
}

fn arr(rt: &Rt, items: Vec<Value>) -> Value {
    Value::Object(rt.new_array(items))
}

fn func(rt: &Rt, code: CodeFn, name: &str, data: Vec<Value>) -> Value {
    Value::Object(rt.new_native_function(code, name, 0, data))
}

fn data(rt: &Rt, c: &Call<'_>, i: usize) -> Value {
    rt.fn_data(c.callee).get(i).cloned().unwrap_or(Value::Undefined)
}

fn arg(c: &Call<'_>, i: usize) -> Value {
    c.args.get(i).cloned().unwrap_or(Value::Undefined)
}

/// `createVNode(type, props, children)` of the realm.
fn vn(rt: &Rt, ty: Value, props: Value, children: Value) -> Value {
    let create = rt.builtin("vn-runtime.createVNode").expect("createVNode builtin");
    call(rt, &Value::Object(create), &Value::Undefined, &[ty, props, children])
}

/// `createCommentVNode(text)` of the vue exports of the realm.
fn comment(rt: &Rt, bag: &Value, text_: &str) -> Value {
    let f = get(rt, bag, "createCommentVNode");
    call(rt, &f, &Value::Undefined, &[s(text_)])
}

fn items_value(rt: &Rt, rows: &[(f64, &str)]) -> Value {
    let list = rows.iter().map(|(id, label)| ob(rt, &[("id", num(*id)), ("label", s(label))])).collect();
    arr(rt, list)
}

fn elements(rt: &Rt, list: &Value) -> Vec<Value> {
    let n = ok(rt, rt.to_number(&get(rt, list, "length")), "length") as u32;
    (0..n).map(|i| ok(rt, rt.get_v(list, &PropKey::Index(i)), "item")).collect()
}

// ----- the probe components of probe.mjs -----

/// `Child`: `defineComponent({ props: ['label'], emits: ['ping'], setup(props, { emit }) { ... } })`.
fn child_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(func(rt, child_render, "render", vec![arg(c, 0), arg(c, 1)]))
}

fn child_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let props = data(rt, c, 0);
    let ctx = data(rt, c, 1);
    let label = rt.get_str(&props, "label")?;
    let on_click = func(rt, child_click, "onClick", vec![ctx, label.clone()]);
    let attrs = ob(rt, &[("class", s("child")), ("onClick", on_click)]);
    let label_text = format!("ping {}", text(rt, &label));
    Ok(vn(rt, s("button"), attrs, s(&label_text)))
}

fn child_click(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let emit = rt.get_str(&data(rt, c, 0), "emit")?;
    rt.call(&emit, &Value::Undefined, &[s("ping"), data(rt, c, 1)])?;
    Ok(Value::Undefined)
}

fn noop(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

/// `() => { s.count++; }` and `onPing: () => { s.count++; }` over the reactive state in data[0].
fn inc_count(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let st = data(rt, c, 0);
    let count = rt.to_number(&rt.get_str(&st, "count")?)?;
    rt.set_v_strict(&st, &PropKey::from_str("count"), num(count + 1.0))?;
    Ok(Value::Undefined)
}

/// `(v) => { s.msg = v; }`.
fn set_msg(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    rt.set_v_strict(&data(rt, c, 0), &PropKey::from_str("msg"), arg(c, 0))?;
    Ok(Value::Undefined)
}

/// `withDirectives(h('input', { type: 'text', 'onUpdate:modelValue': ... }), [[vModelText, s.msg]])`.
fn typed_input(rt: &Rt, bag: &Value, st: &Value) -> Value {
    let on_update = func(rt, set_msg, "onUpdate:modelValue", vec![st.clone()]);
    let input = vn(rt, s("input"), ob(rt, &[("type", s("text")), ("onUpdate:modelValue", on_update)]), Value::Undefined);
    let binding = arr(rt, vec![get(rt, bag, "vModelText"), get(rt, st, "msg")]);
    let dirs = arr(rt, vec![binding]);
    call(rt, &get(rt, bag, "withDirectives"), &Value::Undefined, &[input, dirs])
}

/// `h(Fragment, null, s.items.map((it) => h('li', { key: it.id }, it.label)))` inside a `ul`.
fn keyed_list(rt: &Rt, bag: &Value, items: &Value) -> Value {
    let fragment = get(rt, bag, "Fragment");
    let rows = elements(rt, items)
        .iter()
        .map(|it| {
            let key = get(rt, it, "id");
            vn(rt, s("li"), ob(rt, &[("key", key)]), get(rt, it, "label"))
        })
        .collect();
    let frag = vn(rt, fragment, Value::Null, arr(rt, rows));
    vn(rt, s("ul"), Value::Null, arr(rt, vec![frag]))
}

/// `h(Child, { label, 'data-x': 'v', onPing })`.
fn child_vnode(rt: &Rt, bag: &Value, label: Value, on_ping: Value) -> Value {
    let props = ob(rt, &[("label", label), ("data-x", s("v")), ("onPing", on_ping)]);
    vn(rt, get(rt, bag, "Child"), props, Value::Undefined)
}

/// The `app` probe: the title, the child, the v-if, the keyed list, the typed input and the counter button.
fn app_tree(rt: &Rt, bag: &Value, st: &Value) -> Value {
    let msg = get(rt, st, "msg");
    let count = get(rt, st, "count");
    let title_text = format!("{} & {}", text(rt, &msg), text(rt, &count));
    let title = vn(rt, s("h1"), ob(rt, &[("title", msg.clone())]), s(&title_text));
    let child_el = child_vnode(rt, bag, msg.clone(), func(rt, inc_count, "onPing", vec![st.clone()]));
    let show = if get(rt, st, "show").to_boolean() { vn(rt, s("p"), Value::Null, s("shown")) } else { comment(rt, bag, "") };
    let list = keyed_list(rt, bag, &get(rt, st, "items"));
    let input = typed_input(rt, bag, st);
    let button_text = format!("Count: {}", text(rt, &count));
    let button = vn(rt, s("button"), ob(rt, &[("onClick", func(rt, inc_count, "onClick", vec![st.clone()]))]), s(&button_text));
    vn(rt, s("div"), ob(rt, &[("class", s("app"))]), arr(rt, vec![title, child_el, show, list, input, button]))
}

/// The render function of a probe over its reactive state (`render(state)` in probe.mjs).
fn probe_tree(rt: &Rt, bag: &Value, name: &str, st: &Value) -> Value {
    match name {
        "static-tree" => {
            let div_props = ob(rt, &[("id", s("root")), ("class", s("a b")), ("style", s("color: red; margin: 0"))]);
            let children = vec![
                vn(rt, s("p"), Value::Null, s("Hello & <world> \"q\" 's")),
                vn(rt, s("img"), ob(rt, &[("alt", s("x\"y")), ("src", s("a.png"))]), Value::Undefined),
                vn(rt, s("br"), Value::Undefined, Value::Undefined),
                vn(rt, s("span"), ob(rt, &[("title", s("it's"))]), s("end")),
            ];
            vn(rt, s("div"), div_props, arr(rt, children))
        }
        "attrs-style" => {
            let class = arr(rt, vec![s("x"), ob(rt, &[("y", Value::Bool(true)), ("z", Value::Bool(false))]), Value::Null]);
            let style = ob(rt, &[("color", s("blue")), ("fontSize", s("12px")), ("opacity", num(0.5)), ("width", Value::Null)]);
            let props = ob(
                rt,
                &[
                    ("class", class),
                    ("style", style),
                    ("data-n", num(3.0)),
                    ("hidden", Value::Bool(false)),
                    ("disabled", Value::Bool(true)),
                    ("title", Value::Null),
                    ("tabindex", s("-1")),
                ],
            );
            vn(rt, s("div"), props, s("attrs"))
        }
        "interpolation" => {
            let msg = get(rt, st, "msg");
            let p_text = format!("Count: {} & {}", text(rt, &get(rt, st, "count")), text(rt, &msg));
            vn(rt, s("p"), ob(rt, &[("data-msg", msg)]), s(&p_text))
        }
        "cond-false" | "cond-true" => {
            let child = if get(rt, st, "show").to_boolean() {
                let p_text = format!("shown {}", text(rt, &get(rt, st, "count")));
                vn(rt, s("p"), Value::Null, s(&p_text))
            } else {
                comment(rt, bag, "")
            };
            vn(rt, s("div"), Value::Null, arr(rt, vec![child]))
        }
        "keyed-list" | "keyed-empty" => keyed_list(rt, bag, &get(rt, st, "items")),
        "child-props-emit" => child_vnode(rt, bag, get(rt, st, "label"), func(rt, noop, "onPing", vec![])),
        "input-v-model" => typed_input(rt, bag, st),
        "input-typed" => {
            let p_text = format!("Typed: {}", text(rt, &get(rt, st, "msg")));
            let p = vn(rt, s("p"), Value::Null, s(&p_text));
            vn(rt, s("div"), Value::Null, arr(rt, vec![typed_input(rt, bag, st), p]))
        }
        "click-button" => {
            let button_text = format!("Count: {}", text(rt, &get(rt, st, "count")));
            vn(rt, s("button"), ob(rt, &[("onClick", func(rt, inc_count, "onClick", vec![st.clone()]))]), s(&button_text))
        }
        "app" => app_tree(rt, bag, st),
        "attr-edge" => {
            let props = ob(
                rt,
                &[
                    ("hidden", Value::Bool(true)),
                    ("title", s("")),
                    ("id", num(0.0)),
                    ("data-e", s("")),
                    ("data-f", Value::Bool(false)),
                    ("required", s("")),
                    ("class", s("")),
                    ("data-g", Value::Null),
                    ("data-h", Value::Undefined),
                ],
            );
            vn(rt, s("div"), props, s("edge"))
        }
        "comment-escape" => {
            let children = vec![comment(rt, bag, "a --> b <!-- c --!> d <!-"), comment(rt, bag, "-> lead ->> x")];
            vn(rt, s("div"), Value::Null, arr(rt, children))
        }
        other => panic!("no probe {other}"),
    }
}

/// `makeState()` of each probe (a plain object; `probe_setup` makes it reactive).
fn probe_state(rt: &Rt, name: &str) -> Value {
    match name {
        "static-tree" | "attrs-style" => ob(rt, &[]),
        "interpolation" => ob(rt, &[("count", num(0.0)), ("msg", s("<b>\"x\" & 'y'</b>"))]),
        "cond-false" => ob(rt, &[("show", Value::Bool(false)), ("count", num(2.0))]),
        "cond-true" => ob(rt, &[("show", Value::Bool(true)), ("count", num(2.0))]),
        "keyed-list" => ob(rt, &[("items", items_value(rt, ITEMS))]),
        "keyed-empty" => ob(rt, &[("items", items_value(rt, &[]))]),
        "child-props-emit" => ob(rt, &[("label", s("L"))]),
        "input-v-model" | "input-typed" => ob(rt, &[("msg", s("Hello"))]),
        "click-button" => ob(rt, &[("count", num(0.0))]),
        "app" => ob(rt, &[("msg", s("Hi")), ("count", num(1.0)), ("show", Value::Bool(true)), ("items", items_value(rt, ITEMS))]),
        "attr-edge" | "comment-escape" => ob(rt, &[]),
        other => panic!("no probe {other}"),
    }
}

/// `setup()` of a probe component: `reactive(raw)`, then the render function over that state.
/// data: [realm bag, probe name, raw state or undefined for `makeState()`].
fn probe_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let bag = data(rt, c, 0);
    let name = data(rt, c, 1);
    let mut raw = data(rt, c, 2);
    if raw.is_nullish() {
        raw = probe_state(rt, &text(rt, &name));
    }
    let state = rt.call(&rt.get_str(&bag, "reactive")?, &Value::Undefined, &[raw])?;
    Ok(func(rt, probe_render, "render", vec![bag, state, name]))
}

fn probe_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let bag = data(rt, c, 0);
    let st = data(rt, c, 1);
    let name = text(rt, &data(rt, c, 2));
    Ok(probe_tree(rt, &bag, &name, &st))
}

/// The root component of a probe: `{ setup() { ... } }` with the state given, or made by `makeState()`.
fn probe_root_with(realm: &Realm, name: &str, raw: Value) -> Value {
    let setup = func(&realm.rt, probe_setup, "setup", vec![realm.bag.clone(), s(name), raw]);
    ob(&realm.rt, &[("setup", setup)])
}

fn probe_root(realm: &Realm, name: &str) -> Value {
    probe_root_with(realm, name, Value::Undefined)
}

/// `Counter` of probe.mjs: `setup() { moduleState.hits++; return () => h('i', null, String(moduleState.hits)); }`.
fn counter_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let bag = data(rt, c, 0);
    let module_state = rt.get_str(&bag, "moduleState")?;
    let hits = rt.to_number(&rt.get_str(&module_state, "hits")?)? + 1.0;
    rt.set_v_strict(&module_state, &PropKey::from_str("hits"), num(hits))?;
    Ok(func(rt, counter_render, "render", vec![bag]))
}

fn counter_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let module_state = rt.get_str(&data(rt, c, 0), "moduleState")?;
    let hits = text(rt, &rt.get_str(&module_state, "hits")?);
    Ok(vn(rt, s("i"), Value::Null, s(&hits)))
}

fn counter_root(realm: &Realm) -> Value {
    let setup = func(&realm.rt, counter_setup, "setup", vec![realm.bag.clone()]);
    ob(&realm.rt, &[("setup", setup)])
}

/// `{ render: () => createCommentVNode('v-if') }`.
fn comment_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(comment(rt, &data(rt, c, 0), "v-if"))
}

fn component_root(realm: &Realm, code: CodeFn) -> Value {
    ob(&realm.rt, &[("render", func(&realm.rt, code, "render", vec![realm.bag.clone()]))])
}

/// `{ render: () => h('div', null, [child]) }` with the child vnode in data[0].
fn render_div_child(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let child = data(rt, c, 0);
    Ok(vn(rt, s("div"), Value::Null, arr(rt, vec![child])))
}

fn div_root(realm: &Realm, child: Value) -> Value {
    ob(&realm.rt, &[("render", func(&realm.rt, render_div_child, "render", vec![child]))])
}

/// `h('div', null, [h('span', null, 'a'), 'Client text'])`.
fn render_text_mismatch(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    let span = vn(rt, s("span"), Value::Null, s("a"));
    Ok(vn(rt, s("div"), Value::Null, arr(rt, vec![span, s("Client text")])))
}

/// `h('div', null, [h('p', null, 'Client')])`.
fn render_element_text_mismatch(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    let p = vn(rt, s("p"), Value::Null, s("Client"));
    Ok(vn(rt, s("div"), Value::Null, arr(rt, vec![p])))
}

/// `h('div', null, [h('p', null, 'x')])`.
fn render_tag_mismatch(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    let p = vn(rt, s("p"), Value::Null, s("x"));
    Ok(vn(rt, s("div"), Value::Null, arr(rt, vec![p])))
}

fn loader_noop(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

/// A vnode of a construct outside the subset, built in `realm`.
fn unsupported_child(realm: &Realm, case: &str) -> Value {
    let rt = &realm.rt;
    match case {
        "Teleport" => {
            let props = ob(rt, &[("to", s("body"))]);
            vn(rt, realm.export("Teleport"), props, arr(rt, vec![vn(rt, s("span"), Value::Null, s("x"))]))
        }
        "Transition" => vn(rt, realm.export("Transition"), Value::Null, arr(rt, vec![vn(rt, s("p"), Value::Null, s("x"))])),
        "KeepAlive" => vn(rt, realm.export("KeepAlive"), Value::Null, arr(rt, vec![vn(rt, s("p"), Value::Null, s("x"))])),
        "Suspense" => {
            // Vue's Suspense object: `__isSuspense`, and `normalize(vnode)`, which createVNode calls for the slots.
            let normalize = func(rt, noop, "normalize", vec![]);
            let ty = ob(rt, &[("__isSuspense", Value::Bool(true)), ("name", s("Suspense")), ("normalize", normalize)]);
            vn(rt, ty, Value::Null, arr(rt, vec![vn(rt, s("p"), Value::Null, s("x"))]))
        }
        "async component" => {
            let loader = func(rt, loader_noop, "loader", vec![]);
            let ty = call(rt, &realm.export("defineAsyncComponent"), &Value::Undefined, &[loader]);
            vn(rt, ty, Value::Null, Value::Undefined)
        }
        other => panic!("no case {other}"),
    }
}

// ----- the server side and the client side -----

/// `renderToString(app)` of the probe (or of the module counter and the comment component), in `realm`.
fn render_probe(realm: &Realm, name: &str) -> String {
    let root = match name {
        "comment-with-text" => ob(&realm.rt, &[("render", func(&realm.rt, comment_render, "render", vec![realm.bag.clone()]))]),
        "module-counter-first" | "module-counter-second" => counter_root(realm),
        _ => probe_root(realm, name),
    };
    server_html(realm, root)
}

/// The root of a probe for the client (the same root the server rendered).
fn root_for(realm: &Realm, name: &str) -> Value {
    match name {
        "comment-with-text" => component_root(realm, comment_render),
        "module-counter-first" | "module-counter-second" => counter_root(realm),
        _ => probe_root(realm, name),
    }
}

fn create_ssr_app(realm: &Realm, root: Value) -> Value {
    call(&realm.rt, &realm.export("createSSRApp"), &Value::Undefined, &[root, Value::Null])
}

fn server_html(realm: &Realm, root: Value) -> String {
    let app = create_ssr_app(realm, root);
    ok(&realm.rt, ssr::render_app_to_string(&realm.rt, &app), "renderToString")
}

/// `createSSRApp(root).mount("#app")` in the client realm: the hydration, or the error it threw.
fn hydrate_app(realm: &Realm, root: Value) -> Res<Value> {
    let app = create_ssr_app(realm, root);
    let mount = get(&realm.rt, &app, "mount");
    realm.rt.call(&mount, &app, &[s("#app")])
}

// ----- markup and DOM helpers -----

fn is_void(tag: &str) -> bool {
    matches!(tag, "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input" | "link" | "meta" | "param" | "source" | "track" | "wbr")
}

/// The five escapes of @vue/shared, decoded (an unknown escape is a test failure).
fn decode(src: &str) -> String {
    const ESCAPES: [(&str, char); 5] = [("&amp;", '&'), ("&lt;", '<'), ("&gt;", '>'), ("&quot;", '"'), ("&#39;", '\'')];
    let mut out = String::new();
    let mut rest = src;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let at = &rest[i..];
        let (escape, ch) = ESCAPES.iter().find(|(e, _)| at.starts_with(e)).unwrap_or_else(|| panic!("unknown escape in {at:?}"));
        out.push(*ch);
        rest = &at[escape.len()..];
    }
    out.push_str(rest);
    out
}

/// Builds server markup as the children of `parent`. The parser covers what the server renderer writes for the subset:
/// start and end tags, quoted and bare attributes, void elements, comments and the escapes. Text between two tags is one
/// text node, as a browser parser makes it.
fn parse_markup(host: &mut DocHost, parent: HostNode, html: &str) {
    let mut open = vec![parent];
    let mut rest = html;
    while !rest.is_empty() {
        let top = *open.last().expect("an open element");
        if let Some(after) = rest.strip_prefix("<!--") {
            let end = after.find("-->").expect("comment end");
            let node = host.create_comment(&after[..end]);
            host.insert(node, top, None).expect("insert comment");
            rest = &after[end + 3..];
        } else if let Some(after) = rest.strip_prefix("</") {
            let end = after.find('>').expect("end tag end");
            open.pop();
            rest = &after[end + 1..];
        } else if let Some(after) = rest.strip_prefix('<') {
            let name_end = after.find(|c: char| c.is_whitespace() || c == '>').expect("tag end");
            let name = &after[..name_end];
            let mut tail = &after[name_end..];
            let mut attrs: Vec<(String, String)> = Vec::new();
            loop {
                tail = tail.trim_start();
                if let Some(t) = tail.strip_prefix('>') {
                    tail = t;
                    break;
                }
                let attr_end = tail.find(|c: char| c == '=' || c == '>' || c.is_whitespace()).unwrap_or(tail.len());
                assert!(attr_end > 0, "attribute name expected in {tail:?}");
                let attr_name = &tail[..attr_end];
                tail = &tail[attr_end..];
                if let Some(t) = tail.strip_prefix('=') {
                    let t = t.strip_prefix('"').expect("quoted attribute value");
                    let end = t.find('"').expect("attribute value end");
                    attrs.push((attr_name.to_string(), decode(&t[..end])));
                    tail = &t[end + 1..];
                } else {
                    attrs.push((attr_name.to_string(), String::new()));
                }
            }
            let el = host.create_element(HostNs::Html, name, None).expect("create element");
            for (k, v) in &attrs {
                host.set_attribute(el, k, v).expect("set attribute");
            }
            host.insert(el, top, None).expect("insert element");
            if !is_void(name) {
                open.push(el);
            }
            rest = tail;
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            let node = host.create_text(&decode(&rest[..end]));
            host.insert(node, top, None).expect("insert text");
            rest = &rest[end..];
        }
    }
}

fn child_nodes(host: &DocHost, node: HostNode) -> Vec<HostNode> {
    let mut out = Vec::new();
    let mut cur = host.first_child(node).expect("first child");
    while let Some(c) = cur {
        out.push(c);
        cur = host.next_sibling(c);
    }
    out
}

/// `textContent`: the text below a node, in document order.
fn text_content(host: &DocHost, node: HostNode) -> String {
    match host.node_type(node).expect("node type") {
        3 => host.node_value(node).expect("text value"),
        1 | 9 => child_nodes(host, node).into_iter().map(|c| text_content(host, c)).collect(),
        _ => String::new(),
    }
}

/// The first element below `root` (not `root` itself) with the tag and, when given, the `textContent`.
fn find_element(host: &DocHost, root: HostNode, tag: &str, with_text: Option<&str>) -> HostNode {
    host.subtree_nodes(root)
        .into_iter()
        .skip(1)
        .find(|n| {
            host.is_element(*n) && host.tag_name(*n).eq_ignore_ascii_case(tag) && with_text.is_none_or(|t| text_content(host, *n) == t)
        })
        .unwrap_or_else(|| panic!("no <{tag}> with text {with_text:?}"))
}

fn inner_html(host: &mut DocHost, node: HostNode) -> String {
    host.inner_html(node).expect("serialization")
}

fn property(host: &mut DocHost, node: HostNode, name: &str) -> String {
    match host.get_property(node, name).expect("property") {
        HostValue::Str(v) => v,
        other => panic!("property {name} is {other:?}, not a string"),
    }
}

/// The identity check after hydration: the same nodes in the same places, and no node created or removed.
fn same_nodes(before: &[HostNode], after: &[HostNode]) -> Result<(), String> {
    if before == after {
        return Ok(());
    }
    let lost: Vec<&HostNode> = before.iter().filter(|n| !after.contains(n)).collect();
    let added: Vec<&HostNode> = after.iter().filter(|n| !before.contains(n)).collect();
    Err(format!("the node list changed: {} nodes before, {} after; lost {lost:?}, added {added:?}", before.len(), after.len()))
}

/// The identity check after a state change: every element of `before` is still in the tree. The only new nodes allowed are
/// text nodes, because runtime-dom's `textContent` setter replaces an element's text with one new text node. Returns them.
fn kept_elements(host: &DocHost, before: &[HostNode], after: &[HostNode]) -> Result<Vec<HostNode>, String> {
    for n in before.iter().filter(|n| host.is_element(**n)) {
        if !after.contains(n) {
            return Err(format!("element {n:?} was replaced"));
        }
    }
    let mut added = Vec::new();
    for n in after.iter().filter(|n| !before.contains(n)) {
        let ty = host.node_type(*n).expect("node type");
        if ty != 3 {
            return Err(format!("node {n:?} of type {ty} was created"));
        }
        added.push(*n);
    }
    Ok(added)
}

// ----- tests: the SSR byte comparison -----

#[test]
fn ssr_probes_are_byte_identical_to_the_oracle() {
    let counter_realm = Realm::new();
    let mut failures = Vec::new();
    for (name, expected) in ORACLE {
        let html = if name.starts_with("module-counter") {
            render_probe(&counter_realm, name)
        } else {
            render_probe(&Realm::new(), name)
        };
        if let Err(e) = compare_html(&html, expected) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{} of {} probes differ:\n{}", failures.len(), ORACLE.len(), failures.join("\n"));
    assert_eq!(ORACLE.len(), 17, "the oracle has 17 lines: 14 probes, the module counter pair and the comment");
}

#[test]
fn comparator_rejects_mutated_expectations() {
    let actual = render_probe(&Realm::new(), "static-tree");
    let expected = oracle("static-tree");
    compare_html(&actual, expected).expect("the oracle line must match");
    let mutants = [
        expected.replace("&#39;", "'"),
        expected.replace("&quot;", "\""),
        expected.replace("&amp;", "&"),
        expected.replacen("<br>", "<br/>", 1),
        format!("{expected}\n"),
    ];
    for m in &mutants {
        assert_ne!(m, expected, "the mutation must change the expectation");
        assert!(compare_html(&actual, m).is_err(), "a mutated expectation passed the comparator: {m:?}");
    }
    let actual = render_probe(&Realm::new(), "keyed-list");
    let expected = oracle("keyed-list");
    compare_html(&actual, expected).expect("the oracle line must match");
    assert!(compare_html(&actual, &expected.replacen("<!--]-->", "", 1)).is_err(), "a missing fragment end passed");
    assert!(compare_html(&actual, &expected.replacen("<!--[-->", "<!---->", 1)).is_err(), "a swapped fragment start passed");
}

#[test]
fn two_realms_render_interleaved_without_sharing_state() {
    let a = Realm::new();
    let b = Realm::new();
    // The same component in two requests with different state: a shows the paragraph, b shows the empty comment.
    let a_state = ob(&a.rt, &[("show", Value::Bool(true)), ("count", num(2.0))]);
    let b_state = ob(&b.rt, &[("show", Value::Bool(false)), ("count", num(2.0))]);
    let a1 = server_html(&a, probe_root_with(&a, "cond-true", a_state.clone()));
    let b1 = server_html(&b, probe_root_with(&b, "cond-false", b_state.clone()));
    // The module-level counter of each realm starts from that realm's own state.
    let a_counter1 = render_probe(&a, "module-counter-first");
    let b_counter1 = render_probe(&b, "module-counter-first");
    // Request a changes its state; request b keeps its own.
    set(&a.rt, &a_state, "show", Value::Bool(false));
    let a2 = server_html(&a, probe_root_with(&a, "cond-true", a_state.clone()));
    set(&b.rt, &b_state, "show", Value::Bool(true));
    let b2 = server_html(&b, probe_root_with(&b, "cond-false", b_state.clone()));
    let a_counter2 = render_probe(&a, "module-counter-second");
    let b_counter2 = render_probe(&b, "module-counter-second");

    compare_html(&a1, oracle("cond-true")).expect("a, request 1");
    compare_html(&b1, oracle("cond-false")).expect("b, request 1");
    compare_html(&a2, oracle("cond-false")).expect("a, request 2 (its own state changed)");
    compare_html(&b2, oracle("cond-true")).expect("b, request 2 (its own state changed)");
    compare_html(&a_counter1, oracle("module-counter-first")).expect("realm a counter, first");
    compare_html(&b_counter1, oracle("module-counter-first")).expect("realm b counter starts at its own 1");
    compare_html(&a_counter2, oracle("module-counter-second")).expect("realm a counter, second");
    compare_html(&b_counter2, oracle("module-counter-second")).expect("realm b counter, second");
}

#[test]
fn ssr_rejects_constructs_outside_the_subset_with_a_type_error() {
    for case in UNSUPPORTED {
        let realm = Realm::new();
        let child = unsupported_child(&realm, case);
        let app = create_ssr_app(&realm, div_root(&realm, child));
        match ssr::render_app_to_string(&realm.rt, &app) {
            Ok(html) => panic!("{case}: rendered {html:?} instead of throwing"),
            Err(e) => {
                let (name, message) = error_of(&realm.rt, &e);
                assert_eq!(name, "TypeError", "{case}");
                assert_eq!(message, format!("unsupported: {case} in SSR"), "{case}");
            }
        }
    }
}

// ----- tests: hydration -----

#[test]
fn hydration_adopts_every_server_probe_without_creating_or_moving_nodes() {
    for name in HYDRATED_PROBES {
        let server = Realm::new();
        let html = render_probe(&server, name);
        compare_html(&html, oracle(name)).unwrap_or_else(|e| panic!("{name}: {e}"));

        let client = Realm::new();
        let mut h = client.handle();
        parse_markup(&mut h, client.container, &html);
        let before = h.subtree_nodes(client.container);
        let created = h.created_nodes();
        let markup = inner_html(&mut h, client.container);
        ok(&client.rt, hydrate_app(&client, root_for(&client, name)), name);

        let after = h.subtree_nodes(client.container);
        same_nodes(&before, &after).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(h.created_nodes(), created, "{name}: hydration created host nodes");
        assert_eq!(inner_html(&mut h, client.container), markup, "{name}: hydration changed the markup");
        assert!(renderer::hydration_mismatches(&client.rt).expect("log").is_empty(), "{name}: unexpected mismatch");
    }
}

#[test]
fn hydrated_app_keeps_its_nodes_and_patches_them_on_events() {
    let server = Realm::new();
    let html = render_probe(&server, "app");
    compare_html(&html, oracle("app")).expect("server side");

    let client = Realm::new();
    let mut h = client.handle();
    parse_markup(&mut h, client.container, &html);
    let before = h.subtree_nodes(client.container);
    let created = h.created_nodes();
    let h1 = find_element(&h, client.container, "h1", None);
    let child_button = find_element(&h, client.container, "button", Some("ping Hi"));
    let counter_button = find_element(&h, client.container, "button", Some("Count: 1"));
    ok(&client.rt, hydrate_app(&client, probe_root(&client, "app")), "hydrate app");
    let after = h.subtree_nodes(client.container);
    same_nodes(&before, &after).expect("hydration keeps the server nodes");
    assert_eq!(h.created_nodes(), created, "hydration created host nodes");

    // The child's button emits 'ping'; the parent's onPing adds one to count. Both adopted buttons were given listeners.
    h.click(child_button).expect("click the child");
    client.rt.run_jobs();
    assert_eq!(text_content(&h, h1), "Hi & 2", "the adopted title is patched");
    assert_eq!(text_content(&h, counter_button), "Count: 2", "the adopted counter is patched");
    let mid = h.subtree_nodes(client.container);
    let added = kept_elements(&h, &before, &mid).expect("no element was replaced");
    assert_eq!(h.created_nodes() - created, added.len() as u64, "only text nodes were created by the patch");

    // The adopted counter button has its own listener.
    h.click(counter_button).expect("click the counter");
    client.rt.run_jobs();
    assert_eq!(text_content(&h, counter_button), "Count: 3");
    assert_eq!(text_content(&h, h1), "Hi & 3");
    let end = h.subtree_nodes(client.container);
    kept_elements(&h, &before, &end).expect("the adopted elements are still the same elements");
}

#[test]
fn hydration_keeps_an_input_edited_before_hydration_and_updates_the_model() {
    let server = Realm::new();
    let html = render_probe(&server, "input-typed");
    compare_html(&html, oracle("input-typed")).expect("server side");

    let client = Realm::new();
    let mut h = client.handle();
    parse_markup(&mut h, client.container, &html);
    let input = find_element(&h, client.container, "input", None);
    let p = find_element(&h, client.container, "p", None);
    // The user types before the script runs: the value property changes, the value attribute does not.
    h.set_property(input, "value", HostValue::Str("Hi there".to_string())).expect("type");
    let before = h.subtree_nodes(client.container);
    let created = h.created_nodes();

    ok(&client.rt, hydrate_app(&client, probe_root(&client, "input-typed")), "hydrate");
    // Right after the mount: the edit is still in the input; vModelText.mounted assigned it to the model; the paragraph
    // still shows the server text until the queued update runs.
    assert_eq!(property(&mut h, input, "value"), "Hi there", "the edit survives hydration");
    assert_eq!(text_content(&h, p), "Typed: Hello", "the update is queued, not run yet");
    client.rt.run_jobs();
    assert_eq!(property(&mut h, input, "value"), "Hi there", "the update does not overwrite the edit");
    assert_eq!(text_content(&h, p), "Typed: Hi there", "the model holds the edit and the paragraph shows it");
    assert_eq!(h.get_attribute(input, "value").as_deref(), Some("Hello"), "the server attribute is unchanged");
    let after = h.subtree_nodes(client.container);
    let added = kept_elements(&h, &before, &after).expect("the input and the paragraph are kept; only text nodes are new");
    assert_eq!(h.created_nodes() - created, added.len() as u64, "only the paragraph's text was replaced");
}

#[test]
fn hydration_takes_the_model_value_of_an_input_that_was_not_edited() {
    let server = Realm::new();
    let html = render_probe(&server, "input-typed");

    let client = Realm::new();
    let mut h = client.handle();
    parse_markup(&mut h, client.container, &html);
    let input = find_element(&h, client.container, "input", None);
    let before = h.subtree_nodes(client.container);
    let created = h.created_nodes();
    ok(&client.rt, hydrate_app(&client, probe_root(&client, "input-typed")), "hydrate");
    client.rt.run_jobs();
    assert_eq!(property(&mut h, input, "value"), "Hello");
    assert_eq!(text_content(&h, find_element(&h, client.container, "p", None)), "Typed: Hello");
    let after = h.subtree_nodes(client.container);
    same_nodes(&before, &after).expect("nothing changed");
    assert_eq!(h.created_nodes(), created);
}

#[test]
fn hydration_fixes_a_text_mismatch_in_place_and_logs_it() {
    let client = Realm::new();
    let mut h = client.handle();
    parse_markup(&mut h, client.container, "<div><span>a</span>Server text</div>");
    let div = find_element(&h, client.container, "div", None);
    let text_node = child_nodes(&h, div)[1];
    let before = h.subtree_nodes(client.container);
    let created = h.created_nodes();

    ok(&client.rt, hydrate_app(&client, component_root(&client, render_text_mismatch)), "hydrate");
    let after = h.subtree_nodes(client.container);
    same_nodes(&before, &after).expect("the text node is fixed in place");
    assert_eq!(h.created_nodes(), created, "no node was created");
    assert_eq!(h.node_value(text_node).expect("text"), "Client text");
    assert_eq!(
        renderer::hydration_mismatches(&client.rt).expect("log"),
        vec!["text: server \"Server text\", client \"Client text\"".to_string()]
    );
}

#[test]
fn hydration_resets_element_text_that_differs_and_logs_it() {
    let client = Realm::new();
    let mut h = client.handle();
    parse_markup(&mut h, client.container, "<div><p>Server</p></div>");
    let p = find_element(&h, client.container, "p", None);
    let old_text = child_nodes(&h, p)[0];
    let created = h.created_nodes();

    ok(&client.rt, hydrate_app(&client, component_root(&client, render_element_text_mismatch)), "hydrate");
    assert_eq!(find_element(&h, client.container, "p", Some("Client")), p, "the element is adopted");
    let new_text = child_nodes(&h, p);
    assert_eq!(new_text.len(), 1);
    assert_ne!(new_text[0], old_text, "textContent replaces the text node, as runtime-dom does");
    assert_eq!(h.created_nodes(), created + 1, "one text node is created");
    assert_eq!(
        renderer::hydration_mismatches(&client.rt).expect("log"),
        vec!["text content of <p>: server \"Server\", client \"Client\"".to_string()]
    );
}

#[test]
fn hydration_replaces_an_element_of_another_tag_and_logs_it() {
    let client = Realm::new();
    let mut h = client.handle();
    parse_markup(&mut h, client.container, "<div><span>x</span></div>");
    let span = find_element(&h, client.container, "span", None);
    let created = h.created_nodes();

    ok(&client.rt, hydrate_app(&client, component_root(&client, render_tag_mismatch)), "hydrate");
    assert!(!h.subtree_nodes(client.container).contains(&span), "the server span was removed");
    let p = find_element(&h, client.container, "p", Some("x"));
    assert_ne!(p, span);
    assert_eq!(inner_html(&mut h, client.container), "<div><p>x</p></div>");
    assert_eq!(h.created_nodes(), created + 2, "the new p and its text are created");
    assert_eq!(
        renderer::hydration_mismatches(&client.rt).expect("log"),
        vec!["node: server <span>, client <p>".to_string()]
    );
}

#[test]
fn identity_check_fails_when_mounting_recreates_the_server_nodes() {
    let server = Realm::new();
    let html = render_probe(&server, "app");

    let client = Realm::new();
    let mut h = client.handle();
    parse_markup(&mut h, client.container, &html);
    let before = h.subtree_nodes(client.container);
    let created = h.created_nodes();
    // createApp(...).mount("#app") clears the container and creates the tree again. The identity check must reject it.
    let app = call(&client.rt, &client.export("createApp"), &Value::Undefined, &[probe_root(&client, "app"), Value::Null]);
    let mount = get(&client.rt, &app, "mount");
    call(&client.rt, &mount, &app, &[s("#app")]);
    let after = h.subtree_nodes(client.container);
    assert!(same_nodes(&before, &after).is_err(), "the identity check accepted recreated nodes");
    assert!(kept_elements(&h, &before, &after).is_err(), "the element check accepted recreated elements");
    assert!(h.created_nodes() > created, "the mount created nodes");
}

#[test]
fn hydration_into_an_empty_container_mounts_the_app() {
    let client = Realm::new();
    let h = client.handle();
    let created = h.created_nodes();
    ok(&client.rt, hydrate_app(&client, probe_root(&client, "app")), "mount");
    let mut h = client.handle();
    assert!(h.created_nodes() > created, "the app was mounted");
    let h1 = find_element(&h, client.container, "h1", None);
    assert_eq!(text_content(&h, h1), "Hi & 1");
    let child_button = find_element(&h, client.container, "button", Some("ping Hi"));
    h.click(child_button).expect("click");
    client.rt.run_jobs();
    assert_eq!(text_content(&h, h1), "Hi & 2");
}

#[test]
fn hydration_rejects_constructs_outside_the_subset_with_a_type_error() {
    for case in UNSUPPORTED {
        let client = Realm::new();
        let mut h = client.handle();
        parse_markup(&mut h, client.container, "<div><span>x</span></div>");
        let child = unsupported_child(&client, case);
        match hydrate_app(&client, div_root(&client, child)) {
            Ok(_) => panic!("{case}: hydrated instead of throwing"),
            Err(e) => {
                let (name, message) = error_of(&client.rt, &e);
                assert_eq!(name, "TypeError", "{case}");
                assert_eq!(message, format!("unsupported: {case} in hydration"), "{case}");
            }
        }
    }
}

#[test]
fn static_vnodes_cannot_be_created_so_hydration_never_meets_one() {
    let realm = Realm::new();
    let create = realm.export("createStaticVNode");
    match realm.rt.call(&create, &Value::Undefined, &[s("<p>x</p>"), num(1.0)]) {
        Ok(_) => panic!("createStaticVNode created a vnode"),
        Err(e) => {
            let (name, message) = error_of(&realm.rt, &e);
            assert_eq!(name, "TypeError");
            assert_eq!(message, "createStaticVNode: static HTML strings are not supported (the SFC options disable hoistStatic)");
        }
    }
}

/// `h('div', null, ['A', 'B'])` over the server text "AB".
fn render_two_texts(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(vn(rt, s("div"), Value::Null, arr(rt, vec![s("A"), s("B")])))
}

/// `h('div', null, [h('span', null, 'a')])`.
fn render_first_span(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    let span = vn(rt, s("span"), Value::Null, s("a"));
    Ok(vn(rt, s("div"), Value::Null, arr(rt, vec![span])))
}

/// `h('div', null, [h('span', null, 'a'), h('p', null, 'b')])`.
fn render_span_then_p(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    let span = vn(rt, s("span"), Value::Null, s("a"));
    let p = vn(rt, s("p"), Value::Null, s("b"));
    Ok(vn(rt, s("div"), Value::Null, arr(rt, vec![span, p])))
}

/// `createVNode('div', null, [h('p', null, 'client')], -1)`: a hoisted static element (patchFlag CACHED).
fn render_hoisted(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    let p = vn(rt, s("p"), Value::Null, s("client"));
    let create = rt.builtin("vn-runtime.createVNode").expect("createVNode builtin");
    rt.call(&Value::Object(create), &Value::Undefined, &[s("div"), Value::Null, arr(rt, vec![p]), num(-1.0)])
}

#[test]
fn hydration_splits_adjacent_text_nodes_and_keeps_the_first_one() {
    let client = Realm::new();
    let mut h = client.handle();
    parse_markup(&mut h, client.container, "<div>AB</div>");
    let div = find_element(&h, client.container, "div", None);
    let first = child_nodes(&h, div)[0];
    let created = h.created_nodes();
    ok(&client.rt, hydrate_app(&client, component_root(&client, render_two_texts)), "hydrate");
    let kids = child_nodes(&h, div);
    assert_eq!(kids.len(), 2, "the text was split in two");
    assert_eq!(kids[0], first, "the first text node is kept");
    assert_eq!(h.node_value(kids[0]).expect("text"), "A");
    assert_eq!(h.node_value(kids[1]).expect("text"), "B");
    assert_eq!(h.created_nodes(), created + 1, "one text node is created for the rest");
    assert!(renderer::hydration_mismatches(&client.rt).expect("log").is_empty(), "a split is not a mismatch");
}

#[test]
fn hydration_removes_server_nodes_the_client_does_not_render_and_logs_them() {
    let client = Realm::new();
    let mut h = client.handle();
    parse_markup(&mut h, client.container, "<div><span>a</span><b>extra</b></div>");
    let b = find_element(&h, client.container, "b", None);
    ok(&client.rt, hydrate_app(&client, component_root(&client, render_first_span)), "hydrate");
    assert!(!h.subtree_nodes(client.container).contains(&b), "the extra server element was removed");
    assert_eq!(inner_html(&mut h, client.container), "<div><span>a</span></div>");
    assert_eq!(
        renderer::hydration_mismatches(&client.rt).expect("log"),
        vec!["children: the server markup has nodes after the last client child".to_string()]
    );
}

#[test]
fn hydration_mounts_client_children_the_server_did_not_render_and_logs_it() {
    let client = Realm::new();
    let mut h = client.handle();
    parse_markup(&mut h, client.container, "<div><span>a</span></div>");
    let created = h.created_nodes();
    ok(&client.rt, hydrate_app(&client, component_root(&client, render_span_then_p)), "hydrate");
    assert_eq!(inner_html(&mut h, client.container), "<div><span>a</span><p>b</p></div>");
    assert_eq!(h.created_nodes(), created + 2, "the p and its text are created");
    assert_eq!(
        renderer::hydration_mismatches(&client.rt).expect("log"),
        vec!["children: the client expects more nodes than the server rendered".to_string()]
    );
}

#[test]
fn hydration_adopts_a_hoisted_element_without_comparing_its_children() {
    let client = Realm::new();
    let mut h = client.handle();
    parse_markup(&mut h, client.container, "<div><span>x</span></div>");
    let before = h.subtree_nodes(client.container);
    let created = h.created_nodes();
    ok(&client.rt, hydrate_app(&client, component_root(&client, render_hoisted)), "hydrate");
    same_nodes(&before, &h.subtree_nodes(client.container)).expect("the static element is adopted as it is");
    assert_eq!(h.created_nodes(), created);
    assert_eq!(inner_html(&mut h, client.container), "<div><span>x</span></div>", "the server markup is kept");
    assert!(renderer::hydration_mismatches(&client.rt).expect("log").is_empty());
}

#[test]
fn hydration_logs_stay_in_their_realm() {
    let a = Realm::new();
    let b = Realm::new();
    let mut hb = b.handle();
    parse_markup(&mut hb, b.container, "<div><span>a</span>Server text</div>");
    ok(&b.rt, hydrate_app(&b, component_root(&b, render_text_mismatch)), "hydrate b");
    assert!(renderer::hydration_mismatches(&a.rt).expect("log a").is_empty(), "realm a has no mismatch");
    assert_eq!(renderer::hydration_mismatches(&b.rt).expect("log b").len(), 1, "realm b recorded its mismatch");
}

/// `openBlock()` then `createElementBlock('div', { style }, 'x', flag)` with the reactive style of the bag: the block form a
/// compiled template takes, so hydration is optimized and the style proxy stays in props.style (createVNode would copy it).
/// data[0] is the bag: `style`, `flag`, `renders` (the runs of this render) and the block exports.
fn render_block_with_style(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let bag = data(rt, c, 0);
    let renders = rt.to_number(&rt.get_str(&bag, "renders")?)? + 1.0;
    rt.set_v_strict(&bag, &PropKey::from_str("renders"), num(renders))?;
    let open = rt.get_str(&bag, "openBlock")?;
    rt.call(&open, &Value::Undefined, &[])?;
    let props = ob(rt, &[("style", rt.get_str(&bag, "style")?)]);
    let block = rt.get_str(&bag, "createElementBlock")?;
    rt.call(&block, &Value::Undefined, &[s("div"), props, s("x"), rt.get_str(&bag, "flag")?])
}

#[test]
fn hydration_reads_the_keys_of_a_reactive_style_only_when_the_flag_has_style() {
    // runtime-core's hydrateElement: `patchFlag & 4 && isReactive(props.style)` reads each key of the style, so the render
    // effect depends on them as it does after a patch (prod:1683-1684). With STYLE (4), a later change to one key runs the
    // render again. Without it (0), nothing read the key during hydration and the render does not run.
    for (flag, runs_after_change) in [(4.0, 2.0), (0.0, 1.0)] {
        let client = Realm::new();
        let mut h = client.handle();
        parse_markup(&mut h, client.container, "<div style=\"color: red\">x</div>");
        let before = h.subtree_nodes(client.container);
        let style = call(&client.rt, &client.export("reactive"), &Value::Undefined, &[ob(&client.rt, &[("color", s("red"))])]);
        let bag = ob(
            &client.rt,
            &[
                ("style", style.clone()),
                ("flag", num(flag)),
                ("renders", num(0.0)),
                ("openBlock", client.export("openBlock")),
                ("createElementBlock", client.export("createElementBlock")),
            ],
        );
        let root = ob(&client.rt, &[("render", func(&client.rt, render_block_with_style, "render", vec![bag.clone()]))]);
        ok(&client.rt, hydrate_app(&client, root), "hydrate");
        same_nodes(&before, &h.subtree_nodes(client.container)).expect("the server element is adopted");
        assert!(renderer::hydration_mismatches(&client.rt).expect("log").is_empty(), "flag {flag}: no mismatch");
        let renders = |rt: &Rt| rt.to_number(&get(rt, &bag, "renders")).expect("a number");
        assert_eq!(renders(&client.rt), 1.0, "flag {flag}: the hydration rendered once");

        set(&client.rt, &style, "color", s("blue"));
        client.rt.run_jobs();
        assert_eq!(renders(&client.rt), runs_after_change, "flag {flag}: runs after a change to one style key");
    }
}
