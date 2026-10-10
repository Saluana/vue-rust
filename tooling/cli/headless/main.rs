// The headless runner of a vue-native app (WP-APP1; source frames and the dev protocol in WP 3.7). `vue-native build
// --target headless` copies this file and state.rs to the generated crate's src/. It mounts the root component into a
// DocHost (the runtime's in-memory document) and either runs an interaction script (one-shot mode) or serves the dev
// protocol (--serve, used by `vue-native dev`).
//
// One-shot: `app <script.json>` runs the script and prints one JSON array with a result per step.
//   Script (JSON): {"steps": [{"op": "click", "selector": "#increment"},
//                             {"op": "input", "selector": "#customer", "value": "Ada"},
//                             {"op": "html", "selector": "#count"},
//                             {"op": "snapshot", "name": "initial"}]}
//   Exit status: 0 ok; 1 the app threw or a step failed (message and source frames on stderr); 2 usage or script error.
//
// --serve: JSON lines. Commands on stdin, events on stdout, one object per line:
//   {"op":"start","restore":<snapshot>|null}  -> {"event":"mounted","html":...,"restored":[...],"skipped":[{"name","reason"}]}
//                                             |  {"event":"error","message":...}
//   {"op":"snapshot"}                         -> {"event":"snapshot","state":{...},"dropped":[...]}
//   {"op":"steps","steps":[...]}              -> {"event":"steps","results":[...]}   (the one-shot step ops)
//   {"op":"quit"}                             -> exit 0
// Errors that reach Vue's error handling are reported as "error" events with their source frames (vn_value SourceFrame).
//
// `--stats` (one-shot only; WP 3.1, the release size and allocation report): the heap is collected at each safepoint (after
// each step), as the web host's vn_flush does, and one JSON line with the vn-value heap counters is printed to stderr after
// the mount ("afterMount") and at the end ("end"). The results printed on stdout do not change.
//
// --timing (one-shot only; task 4.5) also measures the run. It writes one line per measurement to stderr,
// "vn-timing " followed by a JSON object. The stdout result is the same with or without the flag, and without the flag the
// runner prints nothing extra and runs no collection. With the flag, a full collection runs after the mount and after every
// step, outside the timed windows, as the web host runs one at the end of each event. The phases:
//   {"phase":"mounted","main_to_mounted_ns":N,"live_objects":N,"bytes_estimate":N,"allocated_total":N,"vm_rss_kib":N,"vm_hwm_kib":N}
//   {"phase":"step","step":I,"op":"click"|"input","selector":S,"ns":N,"collect_ns":N}   (one per click and input step)
//   {"phase":"exit","steps":N,"main_to_end_ns":N,"live_objects":N,"bytes_estimate":N,"allocated_total":N,"vm_rss_kib":N,"vm_hwm_kib":N}
// main_to_mounted_ns runs from the first statement of main to the drained mount; process creation and loading before main
// are not included. A step's ns runs from the dispatch of the click or input to the end of the scheduler flush (settle ends
// when the job queue is empty); collect_ns is the collection after the step and is not part of ns. live_objects and
// bytes_estimate are read after a full collection; bytes_estimate is the runtime's per-object estimate, not allocator usage.
// vm_rss_kib and vm_hwm_kib are VmRSS and VmHWM (the peak) from /proc/self/status: Linux only, null elsewhere.
#![allow(dead_code)]

mod project;
mod state;
mod vn_async;
mod vn_config;

use std::cell::RefCell;
use std::io::BufRead;
use std::rc::Rc;

use vn_dom::{EventInit, EventKind};
use vn_runtime::dom_events::DomCtx;
use vn_runtime::errors::{set_error_sink, ErrorSink};
use vn_runtime::host::{Host, HostNode, HostNs, HostValue};
use vn_runtime::host_doc::DocHost;
use vn_runtime::renderer;
use vn_testkit::json::Json;
use vn_testkit::json_in::parse;
use vn_value::{JsStr, Res, Rt, SiteKind, SourceFrame, Throw, Value};

enum Failure {
    Script(String),
    App(String),
}

// An error that reached the realm's error sink (Vue's logError): its text and the source frames of its throw.
struct Report {
    text: String,
    frames: Vec<SourceFrame>,
}

struct Sink(Rc<RefCell<Vec<Report>>>);

impl ErrorSink for Sink {
    fn report(&self, rt: &Rt, err: &Value) {
        self.0.borrow_mut().push(Report { text: describe(rt, err), frames: rt.source_frames(err) });
    }
}

// "Name: message" for an error object (as its stack's first line), else the value's text.
fn describe(rt: &Rt, value: &Value) -> String {
    if let Value::Object(_) = value {
        let field = |key: &str| match rt.get_str(value, key) {
            Ok(Value::String(s)) => Some(s.to_rust_string_lossy()),
            _ => None,
        };
        if let (Some(name), Some(message)) = (field("name"), field("message")) {
            return if message.is_empty() { name } else { format!("{name}: {message}") };
        }
    }
    match rt.to_string(value) {
        Ok(s) => s.to_rust_string_lossy(),
        Err(_) => "an uncaught JavaScript exception".to_string(),
    }
}

// One source frame as V8 writes one line of a stack, with the author's position. Generated code without a source map
// is named as such (line 0 in the site table).
fn frame_text(f: &SourceFrame) -> String {
    let kind = if f.kind == SiteKind::Await { "async " } else { "" };
    let name = if f.function.is_empty() { "<anonymous>" } else { f.function };
    if f.line == 0 {
        format!("at {kind}{name} (generated code)")
    } else {
        format!("at {kind}{name} ({}:{}:{})", f.file, f.line, f.column)
    }
}

fn report_text(r: &Report) -> String {
    let mut out = r.text.clone();
    for f in &r.frames {
        out.push_str("\n    ");
        out.push_str(&frame_text(f));
    }
    out
}

// One mounted app: the realm, the document, the container and the root component's public instance.
struct Session {
    rt: Rt,
    ctx: DomCtx,
    host: DocHost,
    container: HostNode,
    reports: Rc<RefCell<Vec<Report>>>,
    root: Value,
}

impl Session {
    // Realm, document and container, before the app is created.
    fn start() -> Result<Session, Failure> {
        let rt = Rt::new();
        let reports = Rc::new(RefCell::new(Vec::new()));
        set_error_sink(&rt, Rc::new(Sink(reports.clone())));
        let host = DocHost::new();
        let ctx = DomCtx::new(&rt, &host);
        renderer::install(&ctx);
        let mut h = host.clone();
        let body = h.install_body().map_err(|e| Failure::App(format!("no document body: {e:?}")))?;
        let container = mount_point(&mut h, body)?;
        Ok(Session { rt, ctx, host, container, reports, root: Value::Undefined })
    }

    // Creates the app from the entry module and mounts it into the container.
    fn mount(&mut self) -> Result<(), Failure> {
        let rt = self.rt.clone();
        let exports = vn_runtime::exports::all(&rt);
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
        // The web host roots the app (projectLibRs in tooling/cli/build.mjs); so does this runner, so a collection keeps it.
        rt.root(&app);
        let mount = js(&rt, rt.get_str(&app, "mount"))?;
        self.root = js(&rt, rt.call(&mount, &app, &[Value::String(JsStr::from_str(vn_config::CONTAINER))]))?;
        settle(&rt, &self.ctx);
        self.check("mount")
    }

    // Fails with the first error that reached the error sink or a listener, with its source frames.
    fn check(&self, at: &str) -> Result<(), Failure> {
        let reports: Vec<Report> = std::mem::take(&mut *self.reports.borrow_mut());
        if let Some(r) = reports.first() {
            return Err(Failure::App(format!("{at}: {}", report_text(r))));
        }
        match self.ctx.take_errors().first() {
            None => Ok(()),
            Some(err) => Err(Failure::App(format!("{at}: {}", message_of(&self.rt, err)))),
        }
    }

    fn run_steps(&mut self, steps: &[Json]) -> Result<Vec<Json>, Failure> {
        self.run_steps_with(steps, |_, _, _, _| Ok(()))
    }

    // Runs the steps; `after` sees each checked step with the time from its dispatch to the drained queue.
    fn run_steps_with(
        &mut self,
        steps: &[Json],
        mut after: impl FnMut(&Rt, usize, &Json, std::time::Duration) -> Result<(), Failure>,
    ) -> Result<Vec<Json>, Failure> {
        let mut h = self.host.clone();
        let mut results = Vec::new();
        for (i, step) in steps.iter().enumerate() {
            settle(&self.rt, &self.ctx);
            let t0 = std::time::Instant::now();
            results.push(run_step(&mut h, self.container, i, step)?);
            settle(&self.rt, &self.ctx);
            let elapsed = t0.elapsed();
            self.check(&format!("step {i}"))?;
            after(&self.rt, i, step, elapsed)?;
        }
        Ok(results)
    }

    fn html(&self) -> String {
        let mut h = self.host.clone();
        h.outer_html(self.container).unwrap_or_default()
    }
}

const TIMING_FLAG: &str = "--timing";

fn main() {
    let started = std::time::Instant::now();
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--serve") {
        serve();
        return;
    }
    let stats = args.iter().skip(1).any(|a| a == "--stats");
    let timing = args.iter().skip(1).any(|a| a.as_str() == TIMING_FLAG);
    let Some(path) = args.iter().skip(1).find(|a| !a.starts_with("--")) else {
        eprintln!("usage: {} [--timing] <script.json> [--stats] | --serve", args.first().map(String::as_str).unwrap_or("app"));
        std::process::exit(2);
    };
    let script = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("cannot read {path}: {err}");
            std::process::exit(2);
        }
    };
    match run(&script, stats, timing.then_some(started)) {
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

fn run(script: &str, stats: bool, started: Option<std::time::Instant>) -> Result<String, Failure> {
    let spec = parse(script).map_err(|e| Failure::Script(format!("not JSON: {e:?}")))?;
    let steps = spec
        .field("steps")
        .and_then(Json::items)
        .ok_or_else(|| Failure::Script("expected {\"steps\": [...]}".to_string()))?;
    let mut session = Session::start()?;
    session.mount()?;
    let after_mount = session.rt.heap_stats();
    if let Some(t0) = started {
        // Read before heap_fields collects, so the figure ends at the drained mount.
        let mounted_ns = t0.elapsed().as_nanos() as u64;
        let mut fields = vec![("phase", Json::text("mounted")), ("main_to_mounted_ns", Json::int(mounted_ns))];
        fields.extend(heap_fields(&session.rt));
        timing_line(fields);
    }
    let results = session.run_steps_with(steps, |rt, i, step, elapsed| {
        if started.is_some() {
            timed_step(rt, i, step, elapsed)?;
        } else if stats {
            rt.collect();
        }
        Ok(())
    })?;
    if stats {
        let line = Json::obj([("afterMount", heap_json(after_mount)), ("end", heap_json(session.rt.heap_stats()))]);
        eprintln!("{}", line.to_json());
    }
    if let Some(t0) = started {
        let main_to_end_ns = t0.elapsed().as_nanos() as u64;
        let mut fields = vec![
            ("phase", Json::text("exit")),
            ("steps", Json::int(steps.len() as u64)),
            ("main_to_end_ns", Json::int(main_to_end_ns)),
        ];
        fields.extend(heap_fields(&session.rt));
        timing_line(fields);
    }
    Ok(Json::Arr(results).to_json())
}

// The heap counters of vn-value as a JSON object (--stats).
fn heap_json(s: vn_value::HeapStats) -> Json {
    Json::obj([
        ("live_objects", Json::int(s.live_objects as u64)),
        ("allocated_total", Json::int(s.allocated_total)),
        ("freed_total", Json::int(s.freed_total)),
        ("collections", Json::int(s.collections)),
        ("bytes_estimate", Json::int(s.bytes_estimate as u64)),
        ("peak_live_objects", Json::int(s.peak_live_objects as u64)),
        ("peak_bytes_estimate", Json::int(s.peak_bytes_estimate as u64)),
    ])
}

// The dev protocol (see the header). A failed start keeps its session, so a snapshot can still be taken.
fn serve() {
    let mut session: Option<Session> = None;
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let cmd = match parse(&line) {
            Ok(cmd) => cmd,
            Err(e) => {
                emit(error_event(&Failure::Script(format!("command is not JSON: {e:?}"))));
                continue;
            }
        };
        match text_field(&cmd, "op").as_deref() {
            Some("quit") => break,
            Some("start") => {
                if session.is_some() {
                    emit(error_event(&Failure::Script("the app is already started".to_string())));
                    continue;
                }
                let mut s = match Session::start() {
                    Ok(s) => s,
                    Err(f) => {
                        emit(error_event(&f));
                        continue;
                    }
                };
                let restore = cmd.field("restore").filter(|j| **j != Json::Null).cloned();
                let outcome = s.mount().and_then(|()| match &restore {
                    Some(state) => restore_state(&s, state),
                    None => Ok(state::Restore::default()),
                });
                match outcome {
                    Ok(info) => emit(Json::obj([
                        ("event", Json::text("mounted")),
                        ("html", Json::text(&s.html())),
                        ("restored", Json::Arr(info.restored.iter().map(|n| Json::text(n)).collect())),
                        ("skipped", Json::Arr(info.skipped.iter().map(|(n, r)| Json::obj([("name", Json::text(n)), ("reason", Json::text(r))])).collect())),
                    ])),
                    Err(f) => emit(error_event(&f)),
                }
                session = Some(s);
            }
            Some("snapshot") => match &session {
                None => emit(error_event(&Failure::Script("the app is not started".to_string()))),
                Some(s) => match state::snapshot(&s.rt, &s.root) {
                    Ok(snap) => emit(Json::obj([
                        ("event", Json::text("snapshot")),
                        ("state", snap.state),
                        ("dropped", Json::Arr(snap.dropped.iter().map(|d| Json::text(d)).collect())),
                    ])),
                    Err(Throw(err)) => emit(error_event(&Failure::App(format!("snapshot: {}", describe(&s.rt, &err))))),
                },
            },
            Some("steps") => match &mut session {
                None => emit(error_event(&Failure::Script("the app is not started".to_string()))),
                Some(s) => {
                    let steps = cmd.field("steps").and_then(Json::items).unwrap_or(&[]).to_vec();
                    match s.run_steps(&steps) {
                        Ok(results) => emit(Json::obj([("event", Json::text("steps")), ("results", Json::Arr(results))])),
                        Err(f) => emit(error_event(&f)),
                    }
                }
            },
            other => emit(error_event(&Failure::Script(format!("unknown op {other:?}")))),
        }
    }
}

// Restores a snapshot into the mounted app and settles it. The errors that the restore causes are the app's errors.
fn restore_state(s: &Session, state: &Json) -> Result<state::Restore, Failure> {
    let info = state::restore(&s.rt, &s.root, state).map_err(|Throw(err)| Failure::App(format!("restore: {}", describe(&s.rt, &err))))?;
    settle(&s.rt, &s.ctx);
    s.check("restore")?;
    Ok(info)
}

fn emit(event: Json) {
    println!("{}", event.to_json());
}

fn error_event(f: &Failure) -> Json {
    let (kind, message) = match f {
        Failure::Script(m) => ("script", m),
        Failure::App(m) => ("app", m),
    };
    Json::obj([("event", Json::text("error")), ("kind", Json::text(kind)), ("message", Json::text(message))])
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

fn message_of(rt: &Rt, value: &Value) -> String {
    match rt.get_str(value, "message") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => "an uncaught JavaScript exception".to_string(),
    }
}

fn js<T>(rt: &Rt, r: Res<T>) -> Result<T, Failure> {
    r.map_err(|Throw(value)| Failure::App(message_of(rt, &value)))
}

// --timing: the timed window of one step has closed after `elapsed`. Checks that no job was left for after the window,
// collects the way the web host does after an event, and prints the step's line for a click or an input.
fn timed_step(rt: &Rt, i: usize, step: &Json, elapsed: std::time::Duration) -> Result<(), Failure> {
    // settle ends with run_jobs, which runs the queue until it is empty; a job run now would be timed nowhere.
    if rt.run_jobs() != 0 {
        return Err(Failure::App(format!("step {i}: jobs ran after the timed window")));
    }
    let collect_started = std::time::Instant::now();
    rt.collect();
    let collect_ns = collect_started.elapsed().as_nanos() as u64;
    let op = text_field(step, "op").unwrap_or_default();
    if op == "click" || op == "input" {
        let selector = text_field(step, "selector").unwrap_or_default();
        timing_line(vec![
            ("phase", Json::text("step")),
            ("step", Json::int(i as u64)),
            ("op", Json::text(&op)),
            ("selector", Json::text(&selector)),
            ("ns", Json::int(elapsed.as_nanos() as u64)),
            ("collect_ns", Json::int(collect_ns)),
        ]);
    }
    Ok(())
}

// One --timing line on stderr: "vn-timing " and a JSON object.
fn timing_line(fields: Vec<(&str, Json)>) {
    eprintln!("vn-timing {}", Json::obj(fields).to_json());
}

// The heap counters after a full collection, and the process memory. The collection runs at a safepoint: no code is running
// between steps, and after the mount the runner uses no Value except the app, which is rooted.
fn heap_fields(rt: &Rt) -> Vec<(&'static str, Json)> {
    rt.collect();
    let heap = rt.heap_stats();
    vec![
        ("live_objects", Json::int(heap.live_objects as u64)),
        ("bytes_estimate", Json::int(heap.bytes_estimate as u64)),
        ("allocated_total", Json::int(heap.allocated_total)),
        ("vm_rss_kib", kib(proc_status_kib("VmRSS:"))),
        ("vm_hwm_kib", kib(proc_status_kib("VmHWM:"))),
    ]
}

fn kib(value: Option<u64>) -> Json {
    value.map_or(Json::Null, Json::int)
}

// A field of /proc/self/status in KiB, e.g. "VmHWM:". None where the file does not exist (not Linux).
fn proc_status_kib(key: &str) -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let rest = status.lines().find_map(|line| line.strip_prefix(key))?;
    rest.trim().strip_suffix("kB")?.trim().parse().ok()
}
