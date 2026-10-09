// The headless runner of a vue-native app (WP-APP1). `vue-native build --target headless` copies this file to the
// generated crate's src/main.rs. It mounts the root component into a DocHost (the runtime's in-memory document), runs the
// interaction script named on the command line and prints one JSON array with a result per step.
// Script (JSON): {"steps": [{"op": "click", "selector": "#increment"},
//                           {"op": "input", "selector": "#customer", "value": "Ada"},
//                           {"op": "html", "selector": "#count"},
//                           {"op": "snapshot", "name": "initial"}]}
// Exit status: 0 ok; 1 the app threw or a step failed (message on stderr); 2 usage or script error.
#![allow(dead_code)]

mod project;
mod vn_async;
mod vn_config;

use vn_dom::{EventInit, EventKind};
use vn_runtime::dom_events::DomCtx;
use vn_runtime::host::{Host, HostNode, HostNs, HostValue};
use vn_runtime::host_doc::DocHost;
use vn_runtime::renderer;
use vn_testkit::json::Json;
use vn_testkit::json_in::parse;
use vn_value::{JsStr, Res, Rt, Throw, Value};

enum Failure {
    Script(String),
    App(String),
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: {} <script.json>", args.first().map(String::as_str).unwrap_or("app"));
        std::process::exit(2);
    };
    let script = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("cannot read {path}: {err}");
            std::process::exit(2);
        }
    };
    match run(&script) {
        Ok(out) => println!("{out}"),
        Err(Failure::Script(message)) => {
            eprintln!("script error: {message}");
            std::process::exit(2);
        }
        Err(Failure::App(message)) => {
            eprintln!("app error: {message}");
            std::process::exit(1);
        }
    }
}

fn run(script: &str) -> Result<String, Failure> {
    let spec = parse(script).map_err(|e| Failure::Script(format!("not JSON: {e:?}")))?;
    let steps = spec
        .field("steps")
        .and_then(Json::items)
        .ok_or_else(|| Failure::Script("expected {\"steps\": [...]}".to_string()))?;

    let rt = Rt::new();
    let exports = vn_runtime::exports::all(&rt);
    let host = DocHost::new();
    let ctx = DomCtx::new(&rt, &host);
    renderer::install(&ctx);
    let mut h = host.clone();
    let body = h.install_body().map_err(|e| Failure::App(format!("no document body: {e:?}")))?;
    let container = mount_point(&mut h, body)?;

    project::natives::register(&rt);
    for def in project::modules::defs() {
        rt.register_module_def(def);
    }
    let module = js(&rt, rt.load_module(project::modules::entry()))?;
    let root = match js(&rt, rt.module_export_cell(module, "default"))? {
        Some(cell) => rt.cell_get(cell),
        None => return Err(Failure::App("the entry module has no default export".to_string())),
    };
    let create_app = exports
        .iter()
        .find(|(name, _)| *name == "createApp")
        .map(|(_, v)| v.clone())
        .ok_or_else(|| Failure::App("the vue module has no createApp".to_string()))?;
    let app = js(&rt, rt.call(&create_app, &Value::Undefined, &[root, Value::Undefined]))?;
    let mount = js(&rt, rt.get_str(&app, "mount"))?;
    js(&rt, rt.call(&mount, &app, &[Value::String(JsStr::from_str(vn_config::CONTAINER))]))?;
    settle(&rt, &ctx);
    check_errors(&rt, &ctx, "mount")?;

    let mut results = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        settle(&rt, &ctx);
        results.push(run_step(&mut h, container, i, step)?);
        settle(&rt, &ctx);
        check_errors(&rt, &ctx, &format!("step {i}"))?;
    }
    Ok(Json::Arr(results).to_json())
}

// The container the page would create: an element with the id of vn_config::CONTAINER, appended to the body.
fn mount_point(h: &mut DocHost, body: HostNode) -> Result<HostNode, Failure> {
    let id = vn_config::CONTAINER
        .strip_prefix('#')
        .ok_or_else(|| Failure::App(format!("container {} is not an id selector", vn_config::CONTAINER)))?;
    let el = h.create_element(HostNs::Html, "div", None).map_err(|e| Failure::App(format!("container: {e:?}")))?;
    h.set_attribute(el, "id", id).map_err(|e| Failure::App(format!("container: {e:?}")))?;
    h.insert(el, body, None).map_err(|e| Failure::App(format!("container: {e:?}")))?;
    Ok(el)
}

fn run_step(h: &mut DocHost, container: HostNode, i: usize, step: &Json) -> Result<Json, Failure> {
    let op = text_field(step, "op").ok_or_else(|| Failure::Script(format!("step {i} has no \"op\"")))?;
    match op.as_str() {
        "click" => {
            let node = select(h, step, i)?;
            h.click(node).map_err(|e| Failure::App(format!("step {i}: click: {e:?}")))?;
            Ok(record(i, &op, Vec::new()))
        }
        "input" => {
            let node = select(h, step, i)?;
            let value = text_field(step, "value")
                .ok_or_else(|| Failure::Script(format!("step {i}: input needs \"value\"")))?;
            h.set_property(node, "value", HostValue::Str(value))
                .map_err(|e| Failure::App(format!("step {i}: set value: {e:?}")))?;
            h.dispatch_event(node, EventInit::new("input", EventKind::Event))
                .map_err(|e| Failure::App(format!("step {i}: input event: {e:?}")))?;
            Ok(record(i, &op, Vec::new()))
        }
        "html" => {
            let node = select(h, step, i)?;
            let html = h.outer_html(node).unwrap_or_default();
            Ok(record(i, &op, vec![("html", Json::text(&html))]))
        }
        "snapshot" => {
            let html = h.outer_html(container).unwrap_or_default();
            let mut extra = vec![("html", Json::text(&html))];
            if let Some(name) = text_field(step, "name") {
                extra.push(("name", Json::text(&name)));
            }
            Ok(record(i, &op, extra))
        }
        other => Err(Failure::Script(format!("step {i}: unknown op \"{other}\""))),
    }
}

fn select(h: &mut DocHost, step: &Json, i: usize) -> Result<HostNode, Failure> {
    let selector = text_field(step, "selector")
        .ok_or_else(|| Failure::Script(format!("step {i}: the op needs \"selector\"")))?;
    match h.query_selector(&selector) {
        Ok(Some(node)) => Ok(node),
        Ok(None) => Err(Failure::App(format!("step {i}: no element matches {selector}"))),
        // A selector outside the supported subset is a mistake in the script, not in the app.
        Err(e) => Err(Failure::Script(format!("step {i}: selector {selector} is not supported: {e:?}"))),
    }
}

fn record(i: usize, op: &str, extra: Vec<(&str, Json)>) -> Json {
    let mut fields = vec![("step", Json::int(i as u64)), ("op", Json::text(op))];
    fields.extend(extra);
    Json::obj(fields)
}

fn text_field(step: &Json, key: &str) -> Option<String> {
    step.field(key).and_then(Json::units).map(String::from_utf16_lossy)
}

// Runs the queued work the page would run between two events: nextTick callbacks, then promise jobs.
fn settle(rt: &Rt, ctx: &DomCtx) {
    for _ in 0..4 {
        ctx.flush_next_tick();
        rt.run_jobs();
    }
}

// A listener or a scheduled job that threw is an app error: nothing is swallowed.
fn check_errors(rt: &Rt, ctx: &DomCtx, at: &str) -> Result<(), Failure> {
    match ctx.take_errors().first() {
        None => Ok(()),
        Some(err) => Err(Failure::App(format!("{at}: {}", message_of(rt, err)))),
    }
}

fn message_of(rt: &Rt, value: &Value) -> String {
    match rt.get_str(value, "message") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => "an uncaught JavaScript exception".to_string(),
    }
}

fn js<T>(rt: &Rt, r: Res<T>) -> Result<T, Failure> {
    r.map_err(|Throw(value)| Failure::App(message_of(rt, &value)))
}
