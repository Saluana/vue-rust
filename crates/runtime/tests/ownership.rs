//! Task 4.3 acceptance tests: component and effect ownership. Real components are mounted through the runtime renderer
//! on the headless DocHost (`renderer::render_into`, clicks, `inner_html`). Each test checks what happened against two
//! references:
//!  - Vue's behaviour (which hooks and watcher callbacks run and in what order, and the subscriber, effect and
//!    dependency-link counts that Vue exposes) comes from tests/ownership-oracle/record.mjs, run on vue@3.5.43
//!    (production build). The values below are copied by hand from its output:
//!    `NODE_ENV=production node tests/ownership-oracle/record.mjs`.
//!  - The counts of our own resources (live effects, computeds, scopes, watchers, deps, links, subscriber edges, host
//!    handles, listeners and GC objects) are measured, not taken from Vue. A "back to baseline" check takes its
//!    baseline after a warm-up cycle, because realm-level state (exports, the scheduler queue, the renderer tables) is
//!    created on first use and then stays for the life of the realm.
//! The negative controls at the end show that each check reports the fault it is meant to catch.

use vn_reactivity::{debug_stats, dep_subscriber_count};
use vn_runtime::dom_events::DomCtx;
use vn_runtime::host::{Host, HostNode, HostNs};
use vn_runtime::host_doc::DocHost;
use vn_runtime::renderer;
use vn_value::{Call, CodeFn, Flags, JsStr, ObjRef, PropKey, Res, Rt, Throw, Value};

// ----- the oracle values (tests/ownership-oracle/record.mjs, vue@3.5.43) -----

const INDEPENDENCE_MOUNTED: &str = "<div><button>A:0</button><button>B:0</button></div>";
const INDEPENDENCE_CLICKED: &str = "<div><button>A:6</button><button>B:2</button></div>";
const INDEPENDENCE_LOG: &[&str] = &["change A 1", "change A 2", "change A 3", "change B 1"];
/// Effects and dependency links that counter B owns: its render effect, and the links of its render effect and its
/// computed.
const INDEPENDENCE_B_EFFECTS: usize = 1;
const INDEPENDENCE_B_LINKS: usize = 3;
const INDEPENDENCE_AFTER_B_CLICK: &str = "<div><button>A:8</button></div>";
/// The log after B is removed (the log was emptied after the first clicks).
const INDEPENDENCE_LOG_AFTER_B_REMOVED: &[&str] = &["change A 4"];

const COMPUTED_RUNS_AFTER_TWO_READS: usize = 1;
const COMPUTED_VALUES: &[f64] = &[10.0, 10.0, 20.0];
const COMPUTED_RUNS_AFTER_CHANGE: usize = 2;

const KEYED_MOUNTED: &str = "<ul><li>2</li><li>4</li><li>6</li></ul>";
const KEYED_LOG_FIRST_CHANGE: &[&str] = &["watch 1 a", "watch 2 a", "watch 3 a"];
const KEYED_SUBSCRIBERS_BEFORE: usize = 3;
/// The effects and dependency links owned by the removed child (its render effect and its watcher's effect).
const KEYED_CHILD_EFFECTS: usize = 2;
const KEYED_CHILD_LINKS: usize = 3;
const KEYED_LOG_REMOVAL: &[&str] = &["bum 2", "scope disposed 2", "unmounted 2"];
const KEYED_SUBSCRIBERS_AFTER: usize = 2;
const KEYED_LOG_SECOND_CHANGE: &[&str] = &["watch 1 b", "watch 3 b"];
const KEYED_HTML_FINAL: &str = "<ul><li>2</li><li>6</li></ul>";

const ORDER_LOG_BEFORE_UNMOUNT: &[&str] = &["watch 1"];
const ORDER_LOG_DURING_UNMOUNT: &[&str] = &["bum", "scope disposed", "um"];
const ORDER_LOG_AFTER_UNMOUNT: &[&str] = &[];
const ORDER_SUBSCRIBERS_AFTER_UNMOUNT: usize = 0;

/// Keyed children per list in the cycle tests.
const CYCLE_CHILDREN: usize = 20;

// ----- helpers -----

fn ok<T>(rt: &Rt, r: Res<T>, what: &str) -> T {
    match r {
        Ok(v) => v,
        Err(Throw(e)) => {
            let text = rt.to_string(&e).map(|s| s.to_rust_string_lossy()).unwrap_or_else(|_| "?".into());
            panic!("{what}: threw {text}")
        }
    }
}

fn s(text: &str) -> Value {
    Value::String(JsStr::from_str(text))
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

fn text_of(rt: &Rt, v: &Value) -> String {
    ok(rt, rt.to_string(v), "to_string").to_rust_string_lossy()
}

fn get(rt: &Rt, o: &Value, key: &str) -> Value {
    ok(rt, rt.get_str(o, key), key)
}

fn set(rt: &Rt, o: &Value, key: &str, v: Value) {
    ok(rt, rt.set_str(o, key, v), key);
}

fn obj(rt: &Rt, pairs: &[(&str, Value)]) -> Value {
    let o = rt.new_object();
    for (k, v) in pairs {
        ok(rt, rt.define_data(o, PropKey::from_str(k), v.clone(), Flags::ALL), "define");
    }
    Value::Object(o)
}

fn arr(rt: &Rt, items: Vec<Value>) -> Value {
    Value::Object(rt.new_array(items))
}

/// The i-th data value of the running native function.
fn data(rt: &Rt, c: &Call<'_>, i: usize) -> Value {
    rt.fn_data(c.callee).get(i).cloned().unwrap_or(Value::Undefined)
}

fn arg(c: &Call<'_>, i: usize) -> Value {
    c.args.get(i).cloned().unwrap_or(Value::Undefined)
}

/// Appends a line to a log array.
fn push(rt: &Rt, log: &Value, line: String) {
    let n = ok(rt, rt.to_number(&get(rt, log, "length")), "length") as u32;
    ok(rt, rt.set_v_strict(log, &PropKey::Index(n), s(&line)), "push");
}

/// The lines of a log array; the array is emptied, so a long run does not grow it.
fn take_log(rt: &Rt, log: &Value) -> Vec<String> {
    let n = ok(rt, rt.to_number(&get(rt, log, "length")), "length") as u32;
    let mut out = Vec::new();
    for i in 0..n {
        let line = ok(rt, rt.get_v(log, &PropKey::Index(i)), "line");
        out.push(text_of(rt, &line));
    }
    set(rt, log, "length", num(0.0));
    out
}

/// The log must equal the oracle's lines, in order.
fn compare_lines(actual: &[String], expected: &[&str]) -> Result<(), String> {
    let got: Vec<&str> = actual.iter().map(String::as_str).collect();
    if got == expected {
        Ok(())
    } else {
        Err(format!("log {got:?}, vue has {expected:?}"))
    }
}

/// The dep object of a ref, read as `ref.dep` (the JS accessor).
fn dep_of(rt: &Rt, r: &Value) -> ObjRef {
    match get(rt, r, "dep") {
        Value::Object(o) => o,
        other => panic!("ref.dep is not an object: {other:?}"),
    }
}

/// The subscriber count of a ref's dep: the links of its subscriber list.
fn subscribers_of(rt: &Rt, r: &Value) -> usize {
    dep_subscriber_count(rt, dep_of(rt, r)).expect("a ref's dep is a dep")
}

// ----- the realm: real components on the headless DocHost -----

/// A realm with the vue exports, the headless DOM host, the renderer and an empty container. Every export is rooted,
/// because the tests hold them across collections.
struct Env {
    rt: Rt,
    ex: Vec<(&'static str, Value)>,
    host: DocHost,
    ctx: DomCtx,
    container: HostNode,
}

/// Every count the ownership checks compare. Reactivity counts come from `debug_stats`; the rest are the host, the DOM
/// layer's side tables and the GC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Counts {
    effects: usize,
    computeds: usize,
    scopes: usize,
    watchers: usize,
    deps: usize,
    links: usize,
    subscriber_edges: usize,
    host_handles: usize,
    listeners: usize,
    expandos: usize,
    element_handles: usize,
    live_objects: usize,
}

impl Counts {
    /// The counts that differ from `base`, as (name, now - base).
    fn deltas(&self, base: &Counts) -> Vec<(&'static str, i64)> {
        let pairs = [
            ("effects", self.effects, base.effects),
            ("computeds", self.computeds, base.computeds),
            ("scopes", self.scopes, base.scopes),
            ("watchers", self.watchers, base.watchers),
            ("deps", self.deps, base.deps),
            ("links", self.links, base.links),
            ("subscriberEdges", self.subscriber_edges, base.subscriber_edges),
            ("hostHandles", self.host_handles, base.host_handles),
            ("listeners", self.listeners, base.listeners),
            ("expandos", self.expandos, base.expandos),
            ("elementHandles", self.element_handles, base.element_handles),
            ("liveObjects", self.live_objects, base.live_objects),
        ];
        pairs
            .iter()
            .filter(|(_, now, was)| now != was)
            .map(|(name, now, was)| (*name, *now as i64 - *was as i64))
            .collect()
    }
}

fn describe(deltas: &[(&str, i64)]) -> String {
    deltas.iter().map(|(name, d)| format!("{name} {d:+}")).collect::<Vec<_>>().join(", ")
}

/// The change of one named count between two snapshots (0 when it did not change).
fn delta(deltas: &[(&str, i64)], name: &str) -> i64 {
    deltas.iter().find(|(n, _)| *n == name).map(|(_, d)| *d).unwrap_or(0)
}

impl Env {
    fn new() -> Env {
        let rt = Rt::new();
        let ex = vn_runtime::exports::all(&rt);
        for (_, v) in &ex {
            rt.root(v);
        }
        let host = DocHost::new();
        let ctx = DomCtx::new(&rt, &host);
        renderer::install(&ctx);
        let container = {
            let mut h = host.clone();
            h.create_element(HostNs::Html, "div", None).expect("container")
        };
        Env { rt, ex, host, ctx, container }
    }

    /// Roots a value for the rest of the test and returns it.
    fn rooted(&self, v: Value) -> Value {
        self.rt.root(&v);
        v
    }

    fn export(&self, name: &str) -> Value {
        self.ex.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone()).unwrap_or_else(|| panic!("no export {name}"))
    }

    fn func(&self, code: CodeFn, name: &str, data: Vec<Value>) -> Value {
        Value::Object(self.rt.new_native_function(code, name, 0, data))
    }

    /// A setup function with two declared parameters (props and the setup context). The runtime creates a context only
    /// when `setup.length > 1`, as Vue does.
    fn setup_fn(&self, code: CodeFn, data: Vec<Value>) -> Value {
        Value::Object(self.rt.new_native_function(code, "setup", 2, data))
    }

    fn arr(&self, items: Vec<Value>) -> Value {
        arr(&self.rt, items)
    }

    fn call(&self, f: &Value, args: &[Value]) -> Value {
        ok(&self.rt, self.rt.call(f, &Value::Undefined, args), "call")
    }

    /// `createVNode(type, props, children)`.
    fn vnode(&self, ty: Value, props: Value, children: Value) -> Value {
        self.call(&self.export("createVNode"), &[ty, props, children])
    }

    /// `render(vnode, container)`; `None` unmounts. The renderer flushes the pre and post callbacks itself.
    fn render(&self, vnode: Option<Value>) {
        let v = vnode.map(|v| match v {
            Value::Object(o) => o,
            other => panic!("vnode expected, got {other:?}"),
        });
        ok(&self.rt, renderer::render_into(&self.rt, v, self.container), "render");
    }

    /// Runs the queued jobs (component updates, watcher callbacks) and the nextTick callbacks.
    fn flush(&self) {
        self.rt.run_jobs();
        self.ctx.flush_next_tick();
    }

    fn html(&self) -> String {
        let mut h = self.host.clone();
        h.inner_html(self.container).expect("container html")
    }

    /// The rendered elements with a tag, in document order. `tag_name` is uppercase, as the DOM's `tagName` is.
    fn elements(&self, tag: &str) -> Vec<HostNode> {
        let h = self.host.clone();
        h.subtree_nodes(self.container).into_iter().filter(|n| h.tag_name(*n).eq_ignore_ascii_case(tag)).collect()
    }

    fn click(&self, node: HostNode) {
        let mut h = self.host.clone();
        h.click(node).expect("click");
    }

    /// A collection, then every count the ownership checks use.
    fn counts(&self) -> Counts {
        self.rt.collect();
        let r = debug_stats(&self.rt);
        let (expandos, element_handles) = self.ctx.side_table_entries();
        Counts {
            effects: r.effects,
            computeds: r.computeds,
            scopes: r.scopes,
            watchers: r.watchers,
            deps: r.deps,
            links: r.links,
            subscriber_edges: r.subscriber_edges,
            host_handles: self.host.live_handles(),
            listeners: self.ctx.listener_count(),
            expandos,
            element_handles,
            live_objects: self.rt.heap_stats().live_objects,
        }
    }
}

// ----- scenario A: two instances of one component, with props and an emit -----

/// `Counter`: props `label`, emits `change`, and its own ref and computed. The button's click increments its own count
/// and emits `change` with its label and the new count.
fn counter_def(env: &Env) -> Value {
    counter_def_sharing(env, None)
}

/// `Counter`, with every instance using `shared` as its count when one is given (the mutated input of the negative
/// control).
fn counter_def_sharing(env: &Env, shared: Option<&Value>) -> Value {
    let shared = shared.cloned().unwrap_or(Value::Undefined);
    let data = vec![env.export("ref"), env.export("computed"), env.export("createVNode"), shared];
    let setup = env.setup_fn(counter_setup, data);
    obj(&env.rt, &[("props", env.arr(vec![s("label")])), ("emits", env.arr(vec![s("change")])), ("setup", setup)])
}

fn counter_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (ref_fn, computed_fn, cv, shared) = (data(rt, c, 0), data(rt, c, 1), data(rt, c, 2), data(rt, c, 3));
    let props = arg(c, 0);
    let emit = get(rt, &arg(c, 1), "emit");
    let count = match shared {
        Value::Undefined => rt.call(&ref_fn, &Value::Undefined, &[num(0.0)])?,
        shared => shared,
    };
    let getter = Value::Object(rt.new_native_function(counter_doubled, "", 0, vec![count.clone()]));
    let doubled = rt.call(&computed_fn, &Value::Undefined, &[getter])?;
    Ok(Value::Object(rt.new_native_function(counter_render, "", 0, vec![cv, count, doubled, emit, props])))
}

fn counter_doubled(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let n = rt.to_number(&get(rt, &data(rt, c, 0), "value"))?;
    Ok(num(n * 2.0))
}

fn counter_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (cv, count, doubled, props) = (data(rt, c, 0), data(rt, c, 1), data(rt, c, 2), data(rt, c, 4));
    let label = get(rt, &props, "label");
    let text = format!("{}:{}", text_of(rt, &label), text_of(rt, &get(rt, &doubled, "value")));
    let on_click = Value::Object(rt.new_native_function(counter_click, "", 0, vec![count, data(rt, c, 3), props]));
    let button_props = obj(rt, &[("onClick", on_click)]);
    rt.call(&cv, &Value::Undefined, &[s("button"), button_props, s(&text)])
}

fn counter_click(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (count, emit, props) = (data(rt, c, 0), data(rt, c, 1), data(rt, c, 2));
    let next = rt.to_number(&get(rt, &count, "value"))? + 1.0;
    rt.set_str(&count, "value", num(next))?;
    let label = get(rt, &props, "label");
    rt.call(&emit, &Value::Undefined, &[s("change"), label, num(next)])?;
    Ok(Value::Undefined)
}

/// `Pair`: props `showB`; renders counter A, and counter B when `showB` is true. Both emit into the same log.
fn pair_def(env: &Env, counter: &Value, log: &Value) -> Value {
    let setup = env.func(pair_setup, "setup", vec![counter.clone(), env.export("createVNode"), log.clone()]);
    obj(&env.rt, &[("props", env.arr(vec![s("showB")])), ("setup", setup)])
}

fn pair_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Object(rt.new_native_function(pair_render, "", 0, vec![data(rt, c, 0), data(rt, c, 1), data(rt, c, 2)])))
}

fn pair_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (counter, cv, log) = (data(rt, c, 0), data(rt, c, 1), data(rt, c, 2));
    let show_b = get(rt, c.this, "showB").to_boolean();
    let on_change = Value::Object(rt.new_native_function(pair_change, "", 0, vec![log]));
    let a_props = obj(rt, &[("label", s("A")), ("onChange", on_change.clone())]);
    let mut kids = vec![rt.call(&cv, &Value::Undefined, &[counter.clone(), a_props, Value::Null])?];
    if show_b {
        let b_props = obj(rt, &[("label", s("B")), ("onChange", on_change)]);
        kids.push(rt.call(&cv, &Value::Undefined, &[counter, b_props, Value::Null])?);
    }
    let children = arr(rt, kids);
    rt.call(&cv, &Value::Undefined, &[s("div"), Value::Null, children])
}

fn pair_change(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let line = format!("change {} {}", text_of(rt, &arg(c, 0)), text_of(rt, &arg(c, 1)));
    push(rt, &data(rt, c, 0), line);
    Ok(Value::Undefined)
}

// ----- scenarios B and C: a keyed list whose children watch a shared ref -----

/// The keyed child. Its setup creates a computed of its `id` prop, a watcher on the shared `source` ref, and the scope
/// and unmount hooks, which append to `log`. Each child's `li` has a click listener. When `leak` is a scope, the
/// watcher is created inside that scope's `run`, so it outlives its child (the negative control of the ownership
/// checks).
fn item_def(env: &Env, log: &Value, leak: Option<&Value>) -> Value {
    let setup = env.func(
        item_setup,
        "setup",
        vec![
            env.export("computed"),
            env.export("watch"),
            env.export("onScopeDispose"),
            env.export("onBeforeUnmount"),
            env.export("onUnmounted"),
            log.clone(),
            leak.cloned().unwrap_or(Value::Undefined),
            env.export("createVNode"),
        ],
    );
    obj(&env.rt, &[("props", env.arr(vec![s("id"), s("source")])), ("setup", setup)])
}

fn item_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (computed, watch, on_dispose, on_bum, on_um) =
        (data(rt, c, 0), data(rt, c, 1), data(rt, c, 2), data(rt, c, 3), data(rt, c, 4));
    let (log, leak, cv) = (data(rt, c, 5), data(rt, c, 6), data(rt, c, 7));
    let props = arg(c, 0);
    let id = get(rt, &props, "id");
    let id_text = text_of(rt, &id);
    let source = get(rt, &props, "source");
    let getter = Value::Object(rt.new_native_function(item_doubled, "", 0, vec![props.clone()]));
    let doubled = rt.call(&computed, &Value::Undefined, &[getter])?;
    let watch_cb = Value::Object(rt.new_native_function(item_watch_cb, "", 0, vec![log.clone(), id.clone()]));
    match &leak {
        Value::Undefined => {
            rt.call(&watch, &Value::Undefined, &[source.clone(), watch_cb])?;
        }
        scope => {
            let run = get(rt, scope, "run");
            let args = vec![watch, source.clone(), watch_cb];
            let body = Value::Object(rt.new_native_function(item_leaky_watch, "", 0, args));
            rt.call(&run, scope, &[body])?;
        }
    }
    let dispose_text = s(&format!("scope disposed {id_text}"));
    let dispose = Value::Object(rt.new_native_function(log_line, "", 0, vec![log.clone(), dispose_text]));
    rt.call(&on_dispose, &Value::Undefined, &[dispose])?;
    let bum = Value::Object(rt.new_native_function(log_line, "", 0, vec![log.clone(), s(&format!("bum {id_text}"))]));
    rt.call(&on_bum, &Value::Undefined, &[bum])?;
    let um_text = s(&format!("unmounted {id_text}"));
    let um = Value::Object(rt.new_native_function(log_line, "", 0, vec![log.clone(), um_text]));
    rt.call(&on_um, &Value::Undefined, &[um])?;
    let click = Value::Object(rt.new_native_function(item_noop, "", 0, vec![]));
    Ok(Value::Object(rt.new_native_function(item_render, "", 0, vec![cv, doubled, click])))
}

fn item_doubled(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let id = rt.to_number(&get(rt, &data(rt, c, 0), "id"))?;
    Ok(num(id * 2.0))
}

fn item_watch_cb(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let line = format!("watch {} {}", text_of(rt, &data(rt, c, 1)), text_of(rt, &arg(c, 0)));
    push(rt, &data(rt, c, 0), line);
    Ok(Value::Undefined)
}

/// The watcher of a leaky child: `watch(source, cb)`, called inside the leaky scope's `run`.
fn item_leaky_watch(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    rt.call(&data(rt, c, 0), &Value::Undefined, &[data(rt, c, 1), data(rt, c, 2)])
}

fn log_line(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    push(rt, &data(rt, c, 0), text_of(rt, &data(rt, c, 1)));
    Ok(Value::Undefined)
}

fn item_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (cv, doubled, click) = (data(rt, c, 0), data(rt, c, 1), data(rt, c, 2));
    let text = text_of(rt, &get(rt, &doubled, "value"));
    let li_props = obj(rt, &[("onClick", click)]);
    rt.call(&cv, &Value::Undefined, &[s("li"), li_props, s(&text)])
}

fn item_noop(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

/// The keyed list: one `Item` per id, keyed by the id, each with the shared `source` ref.
fn list_def(env: &Env, item: &Value, source: &Value) -> Value {
    let setup = env.func(list_setup, "setup", vec![env.export("createVNode"), item.clone(), source.clone()]);
    obj(&env.rt, &[("props", env.arr(vec![s("ids")])), ("setup", setup)])
}

fn list_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(Value::Object(rt.new_native_function(list_render, "", 0, vec![data(rt, c, 0), data(rt, c, 1), data(rt, c, 2)])))
}

fn list_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (cv, item, source) = (data(rt, c, 0), data(rt, c, 1), data(rt, c, 2));
    let ids = get(rt, c.this, "ids");
    let n = rt.to_number(&get(rt, &ids, "length"))? as u32;
    let mut kids = Vec::new();
    for i in 0..n {
        let id = rt.get_v(&ids, &PropKey::Index(i))?;
        let props = obj(rt, &[("key", id.clone()), ("id", id), ("source", source.clone())]);
        kids.push(rt.call(&cv, &Value::Undefined, &[item.clone(), props, Value::Null])?);
    }
    let children = arr(rt, kids);
    rt.call(&cv, &Value::Undefined, &[s("ul"), Value::Null, children])
}

/// The objects of one keyed-list fixture. Everything is rooted, so the collector only frees what the test unmounts.
struct ListFixture {
    log: Value,
    source: Value,
    list: Value,
    /// The detached scope that leaky children run their watchers in (only for the negative control).
    leak_scope: Option<Value>,
}

fn list_fixture(env: &Env, leaky: bool) -> ListFixture {
    let log = env.rooted(env.arr(vec![]));
    let source = env.rooted(env.call(&env.export("ref"), &[s("x")]));
    let leak_scope = leaky.then(|| env.rooted(env.call(&env.export("effectScope"), &[Value::Bool(true)])));
    let item = env.rooted(item_def(env, &log, leak_scope.as_ref()));
    let list = env.rooted(list_def(env, &item, &source));
    ListFixture { log, source, list, leak_scope }
}

/// Renders the list with the given ids (a re-render with new props, so the keyed children are patched).
fn render_list(env: &Env, fx: &ListFixture, ids: &[f64]) {
    let ids_v = env.arr(ids.iter().map(|n| num(*n)).collect());
    let props = obj(&env.rt, &[("ids", ids_v)]);
    env.render(Some(env.vnode(fx.list.clone(), props, Value::Null)));
    env.flush();
}

fn set_source(env: &Env, fx: &ListFixture, text: &str) {
    set(&env.rt, &fx.source, "value", s(text));
    env.flush();
}

/// What one run of the keyed scenario recorded.
struct KeyedRun {
    html_mounted: String,
    log_first_change: Vec<String>,
    subscribers_before: usize,
    before: Counts,
    log_removal: Vec<String>,
    subscribers_after: usize,
    after: Counts,
    log_second_change: Vec<String>,
    html_final: String,
}

/// Mounts children 1, 2, 3; changes the source; removes child 2; changes the source again.
fn run_keyed(leaky: bool) -> KeyedRun {
    let env = Env::new();
    let fx = list_fixture(&env, leaky);
    render_list(&env, &fx, &[1.0, 2.0, 3.0]);
    let html_mounted = env.html();
    set_source(&env, &fx, "a");
    let log_first_change = take_log(&env.rt, &fx.log);
    let subscribers_before = subscribers_of(&env.rt, &fx.source);
    let before = env.counts();
    render_list(&env, &fx, &[1.0, 3.0]);
    let log_removal = take_log(&env.rt, &fx.log);
    let subscribers_after = subscribers_of(&env.rt, &fx.source);
    let after = env.counts();
    set_source(&env, &fx, "b");
    let log_second_change = take_log(&env.rt, &fx.log);
    let html_final = env.html();
    KeyedRun {
        html_mounted,
        log_first_change,
        subscribers_before,
        before,
        log_removal,
        subscribers_after,
        after,
        log_second_change,
        html_final,
    }
}

// ----- scenario D: a computed read twice -----

/// A computed whose getter counts its runs (`runs` is a JS array, one entry per run) and reads a ref. Returns the runs,
/// the ref, the computed and the getter itself.
fn computed_fixture(env: &Env) -> (Value, Value, Value, Value) {
    let runs = env.rooted(env.arr(vec![]));
    let src = env.rooted(env.call(&env.export("ref"), &[num(1.0)]));
    let getter = env.rooted(env.func(computed_getter, "getter", vec![runs.clone(), src.clone()]));
    let computed = env.rooted(env.call(&env.export("computed"), &[getter.clone()]));
    (runs, src, computed, getter)
}

fn computed_getter(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let runs = data(rt, c, 0);
    let len = rt.to_number(&get(rt, &runs, "length"))? as u32;
    rt.set_v_strict(&runs, &PropKey::Index(len), s("run"))?;
    let n = rt.to_number(&get(rt, &data(rt, c, 1), "value"))?;
    Ok(num(n * 10.0))
}

fn runs_so_far(rt: &Rt, runs: &Value) -> usize {
    rt.to_number(&get(rt, runs, "length")).expect("runs length") as usize
}

/// Reads a computed's `value` `reads` times and returns the number of getter runs that caused.
fn getter_runs_for_reads(env: &Env, runs: &Value, computed: &Value, reads: usize) -> usize {
    let before = runs_so_far(&env.rt, runs);
    for _ in 0..reads {
        get(&env.rt, computed, "value");
    }
    runs_so_far(&env.rt, runs) - before
}

/// The check of the computed-caching behaviour: for two reads made while its dependencies are unchanged, a cached
/// computed runs its getter once (Vue's oracle: `COMPUTED_RUNS_AFTER_TWO_READS`).
fn assert_cached(runs_for_two_reads: usize) -> Result<(), String> {
    if runs_for_two_reads == COMPUTED_RUNS_AFTER_TWO_READS {
        Ok(())
    } else {
        Err(format!("the getter ran {runs_for_two_reads} times for 2 reads, a cached computed runs it once"))
    }
}

// ----- scenario E: unmount hooks and a watcher whose source changes while unmounting -----

/// The child of scenario E: `watch(src)`, a scope-disposal callback, a beforeUnmount hook that changes `src`, and an
/// unmounted hook. Every callback appends to `log`.
fn order_child_def(env: &Env, log: &Value, src: &Value) -> Value {
    let setup = env.func(
        order_setup,
        "setup",
        vec![
            env.export("watch"),
            env.export("onScopeDispose"),
            env.export("onBeforeUnmount"),
            env.export("onUnmounted"),
            env.export("createVNode"),
            log.clone(),
            src.clone(),
        ],
    );
    obj(&env.rt, &[("setup", setup)])
}

fn order_setup(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (watch, on_dispose, on_bum, on_um, cv) =
        (data(rt, c, 0), data(rt, c, 1), data(rt, c, 2), data(rt, c, 3), data(rt, c, 4));
    let (log, src) = (data(rt, c, 5), data(rt, c, 6));
    let watch_cb = Value::Object(rt.new_native_function(order_watch_cb, "", 0, vec![log.clone()]));
    rt.call(&watch, &Value::Undefined, &[src.clone(), watch_cb])?;
    let dispose = Value::Object(rt.new_native_function(log_line, "", 0, vec![log.clone(), s("scope disposed")]));
    rt.call(&on_dispose, &Value::Undefined, &[dispose])?;
    let bum = Value::Object(rt.new_native_function(order_bum, "", 0, vec![log.clone(), src]));
    rt.call(&on_bum, &Value::Undefined, &[bum])?;
    let um = Value::Object(rt.new_native_function(log_line, "", 0, vec![log, s("um")]));
    rt.call(&on_um, &Value::Undefined, &[um])?;
    Ok(Value::Object(rt.new_native_function(order_render, "", 0, vec![cv])))
}

fn order_render(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    rt.call(&data(rt, c, 0), &Value::Undefined, &[s("p"), Value::Null, s("child")])
}

fn order_watch_cb(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    push(rt, &data(rt, c, 0), format!("watch {}", text_of(rt, &arg(c, 0))));
    Ok(Value::Undefined)
}

fn order_bum(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    push(rt, &data(rt, c, 0), "bum".to_string());
    set(rt, &data(rt, c, 1), "value", num(99.0));
    Ok(Value::Undefined)
}

// ----- the tests -----

#[test]
fn props_and_emits_keep_two_instances_of_one_component_independent() {
    let env = Env::new();
    let log = env.rooted(env.arr(vec![]));
    let counter = env.rooted(counter_def(&env));
    let pair = env.rooted(pair_def(&env, &counter, &log));
    let with_b = obj(&env.rt, &[("showB", Value::Bool(true))]);
    env.render(Some(env.vnode(pair.clone(), with_b, Value::Null)));
    env.flush();
    assert_eq!(env.html(), INDEPENDENCE_MOUNTED, "mounted markup");

    let buttons = env.elements("button");
    assert_eq!(buttons.len(), 2, "two counters are rendered");
    let (a, b) = (buttons[0], buttons[1]);
    env.click(a);
    env.click(a);
    env.click(a);
    env.click(b);
    env.flush();
    assert_eq!(env.html(), INDEPENDENCE_CLICKED, "each counter shows its own count");
    compare_lines(&take_log(&env.rt, &log), INDEPENDENCE_LOG).expect("emits carry each counter's label and count");

    // Removing B releases exactly B's effect, computed and scope, and the links B's subscriptions held.
    let before = env.counts();
    let without_b = obj(&env.rt, &[("showB", Value::Bool(false))]);
    env.render(Some(env.vnode(pair, without_b, Value::Null)));
    env.flush();
    let after = env.counts();
    let removal = after.deltas(&before);
    assert_eq!(delta(&removal, "effects"), -(INDEPENDENCE_B_EFFECTS as i64), "effects: {}", describe(&removal));
    assert_eq!(delta(&removal, "computeds"), -1, "computeds: {}", describe(&removal));
    assert_eq!(delta(&removal, "scopes"), -1, "scopes: {}", describe(&removal));
    assert_eq!(delta(&removal, "listeners"), -1, "listeners: {}", describe(&removal));
    let edges = delta(&removal, "subscriberEdges");
    assert_eq!(edges, -(INDEPENDENCE_B_LINKS as i64), "subscriber edges: {}", describe(&removal));

    // A keeps its state and its emits after B is gone.
    let buttons = env.elements("button");
    assert_eq!(buttons.len(), 1, "only A is left");
    env.click(buttons[0]);
    env.flush();
    assert_eq!(env.html(), INDEPENDENCE_AFTER_B_CLICK, "A continues from its own count");
    compare_lines(&take_log(&env.rt, &log), INDEPENDENCE_LOG_AFTER_B_REMOVED).expect("A's emit after B is removed");
}

#[test]
fn a_computed_read_twice_evaluates_its_getter_once() {
    let env = Env::new();
    let (runs, src, computed, _getter) = computed_fixture(&env);
    let first = get(&env.rt, &computed, "value");
    let second = get(&env.rt, &computed, "value");
    // The fixture is fresh, so the runs so far are the runs for the two reads.
    let runs_for_two_reads = runs_so_far(&env.rt, &runs);
    assert_cached(runs_for_two_reads).expect("two reads of a fresh computed");
    set(&env.rt, &src, "value", num(2.0));
    let third = get(&env.rt, &computed, "value");
    assert_eq!(runs_so_far(&env.rt, &runs), COMPUTED_RUNS_AFTER_CHANGE, "one more run after the dependency changed");
    let values = [first, second, third].map(|v| text_of(&env.rt, &v).parse::<f64>().expect("a number"));
    assert_eq!(values, COMPUTED_VALUES, "values of the three reads");
}

#[test]
fn removing_a_keyed_child_stops_its_watcher_and_releases_its_effects() {
    let run = run_keyed(false);
    assert_eq!(run.html_mounted, KEYED_MOUNTED, "mounted markup");
    compare_lines(&run.log_first_change, KEYED_LOG_FIRST_CHANGE).expect("watchers of all three children fire");
    assert_eq!(run.subscribers_before, KEYED_SUBSCRIBERS_BEFORE, "the source's subscribers before the removal");
    compare_lines(&run.log_removal, KEYED_LOG_REMOVAL)
        .expect("removal runs beforeUnmount, scope disposal, then unmounted");
    assert_eq!(run.subscribers_after, KEYED_SUBSCRIBERS_AFTER, "the removed child's watcher left the source");
    compare_lines(&run.log_second_change, KEYED_LOG_SECOND_CHANGE).expect("the removed child's watcher does not fire");
    assert_eq!(run.html_final, KEYED_HTML_FINAL, "final markup");

    // Child 2 owned one render effect and one watcher effect, one computed, one watcher, one scope and one listener.
    let removal = run.after.deltas(&run.before);
    assert_eq!(delta(&removal, "effects"), -(KEYED_CHILD_EFFECTS as i64), "effects: {}", describe(&removal));
    assert_eq!(delta(&removal, "watchers"), -1, "watchers: {}", describe(&removal));
    assert_eq!(delta(&removal, "computeds"), -1, "computeds: {}", describe(&removal));
    assert_eq!(delta(&removal, "scopes"), -1, "scopes: {}", describe(&removal));
    assert_eq!(delta(&removal, "listeners"), -1, "listeners: {}", describe(&removal));
    let edges = delta(&removal, "subscriberEdges");
    assert_eq!(edges, -(KEYED_CHILD_LINKS as i64), "subscriber edges: {}", describe(&removal));
}

#[test]
fn unmount_runs_beforeunmount_then_scope_disposal_then_unmounted_and_the_watcher_stays_stopped() {
    let env = Env::new();
    let log = env.rooted(env.arr(vec![]));
    let src = env.rooted(env.call(&env.export("ref"), &[num(0.0)]));
    let child = env.rooted(order_child_def(&env, &log, &src));
    env.render(Some(env.vnode(child, Value::Null, Value::Null)));
    env.flush();
    set(&env.rt, &src, "value", num(1.0));
    env.flush();
    compare_lines(&take_log(&env.rt, &log), ORDER_LOG_BEFORE_UNMOUNT).expect("watcher fires while mounted");

    env.render(None);
    compare_lines(&take_log(&env.rt, &log), ORDER_LOG_DURING_UNMOUNT).expect("unmount order");
    env.flush();
    set(&env.rt, &src, "value", num(5.0));
    env.flush();
    compare_lines(&take_log(&env.rt, &log), ORDER_LOG_AFTER_UNMOUNT).expect("no watcher callback after unmount");
    assert_eq!(subscribers_of(&env.rt, &src), ORDER_SUBSCRIBERS_AFTER_UNMOUNT, "the watcher left the source");
}

/// Mounts `CYCLE_CHILDREN` keyed children, changes the source, and unmounts them all. The source value changes per
/// cycle, so every watcher runs as it would in an application.
fn mount_and_unmount(env: &Env, fx: &ListFixture, counter: &std::cell::Cell<f64>) {
    let ids: Vec<f64> = (0..CYCLE_CHILDREN).map(|i| i as f64).collect();
    render_list(env, fx, &ids);
    counter.set(counter.get() + 1.0);
    set(&env.rt, &fx.source, "value", num(counter.get()));
    env.flush();
    take_log(&env.rt, &fx.log);
    env.render(None);
    env.flush();
    take_log(&env.rt, &fx.log);
}

#[test]
fn a_mounted_list_owns_the_resources_its_components_describe() {
    let env = Env::new();
    let fx = list_fixture(&env, false);
    let turn = std::cell::Cell::new(0.0);
    mount_and_unmount(&env, &fx, &turn);
    let base = env.counts();
    let ids: Vec<f64> = (0..CYCLE_CHILDREN).map(|i| i as f64).collect();
    render_list(&env, &fx, &ids);
    let peak = env.counts().deltas(&base);
    // The list's render effect, and per child two effects: a render effect and a watcher effect (the oracle records two
    // per child).
    assert_eq!(delta(&peak, "effects"), 1 + 2 * CYCLE_CHILDREN as i64, "effects while mounted: {}", describe(&peak));
    assert_eq!(delta(&peak, "watchers"), CYCLE_CHILDREN as i64, "watchers while mounted: {}", describe(&peak));
    assert_eq!(delta(&peak, "computeds"), CYCLE_CHILDREN as i64, "computeds while mounted: {}", describe(&peak));
    assert_eq!(delta(&peak, "scopes"), 1 + CYCLE_CHILDREN as i64, "scopes while mounted: {}", describe(&peak));
    assert_eq!(delta(&peak, "listeners"), CYCLE_CHILDREN as i64, "listeners while mounted: {}", describe(&peak));
    let (hosts, deps, links) = (delta(&peak, "hostHandles"), delta(&peak, "deps"), delta(&peak, "links"));
    assert!(hosts > 0 && deps > 0 && links > 0, "{}", describe(&peak));
    env.render(None);
    env.flush();
    assert_eq!(env.counts(), base, "unmounting returns every count to the baseline");
}

#[test]
fn fifty_unmount_recreate_cycles_return_every_count_to_the_baseline() {
    let env = Env::new();
    let fx = list_fixture(&env, false);
    let turn = std::cell::Cell::new(0.0);
    let base = check_back_to_baseline(&env, 50, |env| mount_and_unmount(env, &fx, &turn))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(env.counts(), base, "the baseline holds after the last cycle too");
}

/// Runs `cycle` once as a warm-up, takes the baseline, then runs it `cycles` more times. After each run (and a
/// collection) every count must equal the baseline. The error names the counts that differ after the first cycle that
/// leaves some.
fn check_back_to_baseline(env: &Env, cycles: usize, mut cycle: impl FnMut(&Env)) -> Result<Counts, String> {
    cycle(env);
    let base = env.counts();
    for i in 1..=cycles {
        cycle(env);
        let deltas = env.counts().deltas(&base);
        if !deltas.is_empty() {
            return Err(format!("after cycle {i}: {}", describe(&deltas)));
        }
    }
    Ok(base)
}

// ----- negative controls -----

#[test]
fn negative_control_a_leaking_child_is_reported_by_the_baseline_check() {
    let env = Env::new();
    let fx = list_fixture(&env, true);
    let turn = std::cell::Cell::new(0.0);
    let err = check_back_to_baseline(&env, 50, |env| mount_and_unmount(env, &fx, &turn))
        .expect_err("a watcher registered on a global scope must be reported");
    // Each leaking child's watcher keeps its subscription and its effect alive. The watcher's job also holds the child
    // instance (Vue's `doWatch` does the same: `job.i = instance`), so the child's render effect (stopped, but still
    // referenced), its scope and its elements stay live too, and through the instance the list that rendered it. So the
    // first cycle reports 20 watchers, 20 child render effects and 1 list render effect (41 effects), and 20 subscriber
    // edges (the watchers' subscriptions to the source).
    assert!(err.starts_with("after cycle 1:"), "{err}");
    assert!(err.contains(&format!("watchers +{CYCLE_CHILDREN}")), "{err}");
    assert!(err.contains(&format!("effects +{}", 2 * CYCLE_CHILDREN + 1)), "{err}");
    assert!(err.contains(&format!("subscriberEdges +{CYCLE_CHILDREN}")), "{err}");
    assert!(fx.leak_scope.is_some(), "the leaky variant owns a global scope");
}

#[test]
fn negative_control_a_non_cached_getter_is_reported() {
    let env = Env::new();
    let (runs, _src, computed, getter) = computed_fixture(&env);
    // The getter itself, without the computed's cache: each call runs it, so two reads run it twice.
    let uncached_runs = {
        let before = runs_so_far(&env.rt, &runs);
        for _ in 0..2 {
            env.call(&getter, &[]);
        }
        runs_so_far(&env.rt, &runs) - before
    };
    assert_eq!(uncached_runs, 2, "the getter runs on every call when read directly");
    let err = assert_cached(uncached_runs).expect_err("two runs for two reads must be reported");
    assert!(err.contains("2 times"), "{err}");
    // Through the computed itself, the same two reads run the getter once.
    assert_cached(getter_runs_for_reads(&env, &runs, &computed, 2)).expect("the cached read is accepted");
}

#[test]
fn negative_control_a_leaking_keyed_child_fails_the_oracle_comparison() {
    let run = run_keyed(true);
    let err = compare_lines(&run.log_second_change, KEYED_LOG_SECOND_CHANGE)
        .expect_err("a watcher left on a global scope fires for the removed child");
    assert!(err.contains("watch 2 b"), "{err}");
    assert_eq!(run.subscribers_after, KEYED_SUBSCRIBERS_BEFORE, "the leaked watcher keeps its subscription");
    // The leaked watcher's job holds the removed child's instance, so the child's render effect and scope stay live
    // (stopped, but still referenced), and so does the leaked watcher. The removal releases the render effect's link to
    // the computed (a link object), and unsubscribes the computed from the prop (Vue soft-removes those links when a
    // computed loses its last subscriber, and so does this runtime). The watcher's subscription to the source stays, so
    // the subscriber edges fall by 2, not 3.
    let removal = run.after.deltas(&run.before);
    assert_eq!(delta(&removal, "effects"), 0, "effects: {}", describe(&removal));
    assert_eq!(delta(&removal, "watchers"), 0, "watchers: {}", describe(&removal));
    let edges = delta(&removal, "subscriberEdges");
    assert_eq!(edges, -(KEYED_CHILD_LINKS as i64 - 1), "subscriber edges: {}", describe(&removal));
}

/// The markup after the independence clicks when both counters share one count of 4 (hand-derived: its double is 8).
const SHARED_CLICKED: &str = "<div><button>A:8</button><button>B:8</button></div>";

#[test]
fn negative_control_counters_that_share_state_are_reported() {
    let env = Env::new();
    let log = env.rooted(env.arr(vec![]));
    let shared = env.rooted(env.call(&env.export("ref"), &[num(0.0)]));
    let counter = env.rooted(counter_def_sharing(&env, Some(&shared)));
    let pair = env.rooted(pair_def(&env, &counter, &log));
    let with_b = obj(&env.rt, &[("showB", Value::Bool(true))]);
    env.render(Some(env.vnode(pair, with_b, Value::Null)));
    env.flush();
    let buttons = env.elements("button");
    let (a, b) = (buttons[0], buttons[1]);
    env.click(a);
    env.click(a);
    env.click(a);
    env.click(b);
    env.flush();
    // The same clicks as the independence test. With one shared count of 4, both markups show its double, 8.
    assert_eq!(env.html(), SHARED_CLICKED, "shared state: both markups show the shared count");
    let lines = take_log(&env.rt, &log);
    let err = compare_lines(&lines, INDEPENDENCE_LOG).expect_err("shared state must change the emits");
    assert!(err.contains("change B 4"), "{err}");
}

#[test]
fn negative_control_a_corrupted_expectation_is_reported() {
    let mut corrupted: Vec<&str> = INDEPENDENCE_LOG.to_vec();
    corrupted[2] = "change A 4";
    let actual: Vec<String> = INDEPENDENCE_LOG.iter().map(|l| l.to_string()).collect();
    compare_lines(&actual, INDEPENDENCE_LOG).expect("the correct expectation matches");
    let err = compare_lines(&actual, &corrupted).expect_err("the corrupted expectation must be reported");
    assert!(err.contains("change A 4"), "{err}");
}
