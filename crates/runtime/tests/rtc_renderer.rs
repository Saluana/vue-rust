//! WP-RT-C renderer differential tests. The scenarios of tests/ui/runtime-c/renderer (vnode DSL, step by step) are built
//! with the `vue` module exports (createVNode and friends), rendered by the Rust renderer into a DocHost container, and
//! the container's innerHTML after each step is compared with what vue@3.5.43 produced in headless Chrome
//! (tests/ui/runtime-c/record-renderer.mjs writes golden.json).

use vn_runtime::dom_events::DomCtx;
use vn_runtime::host::{Host, HostNs};
use vn_runtime::host_doc::DocHost;
use vn_runtime::renderer;
use vn_testkit::json::Json;
use vn_testkit::json_in::parse;
use vn_value::{Flags, JsStr, PropKey, Res, Rt, Value};

fn fixture(name: &str) -> String {
    let path = format!("{}/../../tests/ui/runtime-c/renderer/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

fn units_text(u: &[u16]) -> String {
    String::from_utf16_lossy(u)
}

/// The vue exports by name (createVNode, createTextVNode, createCommentVNode, Fragment).
struct Exports(Vec<(&'static str, Value)>);

impl Exports {
    fn get(&self, name: &str) -> Value {
        self.0.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone()).unwrap_or_else(|| panic!("no export {name}"))
    }
}

fn call(rt: &Rt, f: &Value, args: &[Value]) -> Res<Value> {
    rt.call(f, &Value::Undefined, args)
}

/// A DSL child: a string, a number, null, or a node object.
fn child(rt: &Rt, ex: &Exports, j: &Json) -> Res<Value> {
    match j {
        Json::Null => Ok(Value::Null),
        Json::Str(u) => Ok(Value::String(JsStr::from_units(u))),
        Json::Num(s) => Ok(Value::Number(s.parse::<f64>().expect("number"))),
        Json::Bool(b) => Ok(Value::Bool(*b)),
        Json::Obj(_) => node(rt, ex, j),
        Json::Arr(items) => {
            let mut v = Vec::new();
            for it in items {
                v.push(child(rt, ex, it)?);
            }
            Ok(Value::Object(rt.new_array(v)))
        }
    }
}

/// One vnode of the DSL: { text } | { comment } | { fragment: [...], key? } | { el, props?, key?, children? }.
fn node(rt: &Rt, ex: &Exports, j: &Json) -> Res<Value> {
    if let Some(t) = j.field("text") {
        let Json::Str(u) = t else { panic!("text must be a string") };
        return call(rt, &ex.get("createTextVNode"), &[Value::String(JsStr::from_units(u))]);
    }
    if let Some(c) = j.field("comment") {
        let Json::Str(u) = c else { panic!("comment must be a string") };
        return call(rt, &ex.get("createCommentVNode"), &[Value::String(JsStr::from_units(u))]);
    }
    let props = props_of(rt, ex, j)?;
    if let Some(Json::Arr(items)) = j.field("fragment") {
        let mut kids = Vec::new();
        for it in items {
            kids.push(child(rt, ex, it)?);
        }
        let kids = Value::Object(rt.new_array(kids));
        return call(rt, &ex.get("createVNode"), &[ex.get("Fragment"), props, kids]);
    }
    let Some(Json::Str(tag)) = j.field("el") else { panic!("node without el/text/comment/fragment") };
    let children = match j.field("children") {
        None | Some(Json::Null) => Value::Null,
        Some(c) => child(rt, ex, c)?,
    };
    call(rt, &ex.get("createVNode"), &[Value::String(JsStr::from_units(tag)), props, children])
}

/// `props` of a node: the `props` object plus `key` (null when there is neither).
fn props_of(rt: &Rt, ex: &Exports, j: &Json) -> Res<Value> {
    let _ = ex;
    let mut pairs: Vec<(Vec<u16>, Json)> = Vec::new();
    if let Some(Json::Obj(p)) = j.field("props") {
        pairs.extend(p.iter().cloned());
    }
    if let Some(k) = j.field("key") {
        pairs.push((units("key"), k.clone()));
    }
    if pairs.is_empty() {
        return Ok(Value::Null);
    }
    let obj = rt.new_object();
    for (k, v) in pairs {
        let val = match v {
            Json::Str(u) => Value::String(JsStr::from_units(&u)),
            Json::Num(s) => Value::Number(s.parse::<f64>().expect("number")),
            Json::Bool(b) => Value::Bool(b),
            other => panic!("unsupported prop value {other:?}"),
        };
        rt.define_data(obj, PropKey::from_str(&units_text(&k)), val, Flags::ALL)?;
    }
    Ok(Value::Object(obj))
}

fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Renders the steps of one scenario; the innerHTML of the container after every step.
fn run_scenario(steps: &[Json]) -> Res<Vec<String>> {
    let rt = Rt::new();
    let ex = Exports(vn_runtime::exports::all(&rt));
    let host = DocHost::new();
    let ctx = DomCtx::new(&rt, &host);
    renderer::install(&ctx);
    let container = {
        let mut h = host.clone();
        h.create_element(HostNs::Html, "div", None).expect("container")
    };
    let mut out = Vec::new();
    for step in steps {
        let render = step.field("render").expect("step.render");
        match render {
            Json::Null => renderer::render_into(&rt, None, container)?,
            other => {
                let v = node(&rt, &ex, other)?;
                let Value::Object(o) = v else { panic!("vnode expected") };
                renderer::render_into(&rt, Some(o), container)?;
            }
        }
        let mut h = host.clone();
        out.push(h.inner_html(container).expect("container html"));
    }
    Ok(out)
}

/// Compares the actual snapshots with the expected ones; returns one line per mismatching step.
fn compare(name: &str, actual: &[String], expected: &[String]) -> Vec<String> {
    let mut bad = Vec::new();
    if actual.len() != expected.len() {
        bad.push(format!("{name}: {} steps, expected {}", actual.len(), expected.len()));
    }
    for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
        if a != e {
            bad.push(format!("{name} step {i}: got {a:?}, vue has {e:?}"));
        }
    }
    bad
}

fn golden_map(text: &str) -> Vec<(String, Vec<String>)> {
    let Json::Obj(pairs) = parse(text).expect("golden json") else { panic!("golden is an object") };
    pairs
        .into_iter()
        .map(|(k, v)| {
            let Some(items) = v.items() else { panic!("golden entry is an array") };
            let steps = items
                .iter()
                .map(|s| match s {
                    Json::Str(u) => units_text(u),
                    _ => panic!("golden step is a string"),
                })
                .collect();
            (units_text(&k), steps)
        })
        .collect()
}

#[test]
fn renderer_scenarios_match_vue_in_chrome() {
    let scen = parse(&fixture("scenarios.json")).expect("scenarios json");
    let golden = golden_map(&fixture("golden.json"));
    let items = scen.items().expect("scenarios is an array").to_vec();
    let mut failures = Vec::new();
    let mut count = 0;
    for sc in &items {
        let Some(Json::Str(name)) = sc.field("name") else { panic!("scenario name") };
        let name = units_text(name);
        let steps = sc.field("steps").and_then(|s| s.items()).expect("steps").to_vec();
        let expected = &golden.iter().find(|(k, _)| *k == name).unwrap_or_else(|| panic!("no golden for {name}")).1;
        let actual = run_scenario(&steps).unwrap_or_else(|e| panic!("{name}: the renderer threw {:?}", e.0));
        failures.extend(compare(&name, &actual, expected));
        count += 1;
    }
    assert!(count >= 20, "expected the full scenario corpus, found {count}");
    assert!(failures.is_empty(), "{} mismatching steps of {count} scenarios:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn negative_control_detects_a_wrong_expectation() {
    let golden = golden_map(&fixture("golden.json"));
    let (name, expected) = golden.iter().find(|(k, _)| k == "element-text").expect("element-text golden");
    let mut wrong = expected.clone();
    wrong[0] = wrong[0].replace("hello", "HELLO");
    let steps = parse(&fixture("scenarios.json")).expect("json");
    let sc = steps.items().unwrap().iter().find(|s| matches!(s.field("name"), Some(Json::Str(u)) if units_text(u) == *name)).expect("scenario");
    let actual = run_scenario(sc.field("steps").and_then(|s| s.items()).expect("steps").to_vec().as_slice()).expect("renders");
    assert!(compare(name, &actual, expected).is_empty(), "the correct expectation must match");
    assert_eq!(compare(name, &actual, &wrong).len(), 1, "the corrupted expectation must be reported");
}

/// The renderer draws through a host behind `Rc<RefCell<dyn Host>>` (the shape a non-re-entrant host plugs in as).
/// The expected markup is the Vue output recorded for `setup-render-fn` (tests/ui/runtime-c/components/golden.json).
#[test]
fn renders_through_a_dyn_host() {
    use std::cell::RefCell;
    use std::rc::Rc;
    use vn_runtime::host::Host;
    use vn_runtime::host::HostNs as Ns;
    let rt = vn_value::Rt::new();
    let host: Rc<RefCell<dyn Host>> = Rc::new(RefCell::new(DocHost::new()));
    let ctx = DomCtx::new(&rt, &host);
    renderer::install(&ctx);
    let container = {
        let mut h = ctx.host();
        h.create_element(Ns::Html, "div", None).expect("container")
    };
    let create = vn_runtime::exports::all(&rt).into_iter().find(|(n, _)| *n == "createVNode").expect("createVNode").1;
    let props = rt.new_object();
    rt.define_data(props, vn_value::PropKey::from_str("class"), vn_value::Value::String(vn_value::JsStr::from_str("c")), vn_value::Flags::ALL)
        .expect("props");
    let vnode = rt
        .call(&create, &vn_value::Value::Undefined, &[vn_value::Value::String(vn_value::JsStr::from_str("p")), vn_value::Value::Object(props), vn_value::Value::String(vn_value::JsStr::from_str("hi"))])
        .expect("createVNode");
    let vn = match vnode {
        vn_value::Value::Object(o) => o,
        _ => panic!("vnode expected"),
    };
    renderer::render_into(&rt, Some(vn), container).expect("render through dyn Host");
    let mut h = ctx.host();
    assert_eq!(h.inner_html(container).expect("container html"), "<p class=\"c\">hi</p>");
}
