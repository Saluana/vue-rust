//! Replay of builtin-call corpora (docs/dev/builtin-corpus.md). Each case runs in a fresh realm: its `pre`
//! encoding is decoded, the target is called through the runtime, and the result is encoded and compared with
//! `post` (structurally; exceptions compare their `name` only; `approx` numbers may differ by one ulp).
//! A target that the realm does not have is reported as missing, which is not a failure. Nothing here runs
//! the collector: a case is short-lived and every value it holds stays on the Rust stack.

use std::cell::RefCell;
use std::collections::HashSet;

use vn_value::{Intrinsic, ObjRef, PropKey, Rt, Throw, Value};

use crate::decode::{decode_pass, IdTable};
use crate::json::Json;
use crate::json_in;
use crate::presets::{install_presets, PRESETS};
use crate::trace::{encode_pass, Recorder};

/// Options of a replay. `setup` runs in every fresh realm before the case: it defines native targets (the
/// builtin WPs install theirs there, and tests define their own).
#[derive(Default)]
pub struct ReplayOptions {
    pub setup: Option<fn(&Rt)>,
}

/// A case that replayed differently. `expected` and `actual` are the JSON texts of the first differing parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub id: String,
    pub reason: String,
    pub expected: String,
    pub actual: String,
}

/// A case whose target the realm does not define.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missing {
    pub id: String,
    pub target: String,
}

/// The outcome of one suite. `total = passed + failures + missing` for every suite that could be read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReplayReport {
    pub suite: String,
    pub total: usize,
    pub passed: usize,
    pub failures: Vec<Failure>,
    pub missing: Vec<Missing>,
}

impl ReplayReport {
    /// Every case passed and none is missing.
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty() && self.missing.is_empty() && self.passed == self.total
    }

    /// The report as one line of JSON (the `--replay` output).
    pub fn to_json(&self) -> String {
        Json::obj([
            ("suite", Json::text(&self.suite)),
            ("total", Json::int(self.total as u64)),
            ("passed", Json::int(self.passed as u64)),
            (
                "failures",
                Json::Arr(
                    self.failures
                        .iter()
                        .map(|f| {
                            Json::obj([
                                ("id", Json::text(&f.id)),
                                ("reason", Json::text(&f.reason)),
                                ("expected", Json::text(&f.expected)),
                                ("actual", Json::text(&f.actual)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "missing",
                Json::Arr(
                    self.missing
                        .iter()
                        .map(|m| Json::obj([("id", Json::text(&m.id)), ("target", Json::text(&m.target))]))
                        .collect(),
                ),
            ),
        ])
        .to_json()
    }
}

/// Replays every case of a suite file (its text) with the default tolerance: `approx` numbers may differ by one ulp.
pub fn replay_suite(json_text: &str, opts: &ReplayOptions) -> ReplayReport {
    replay_suite_approx_ulps(json_text, opts, 1)
}

/// Replays every case of a suite file (its text). A malformed suite is a single failure with `total` 0. `approx_ulps` is
/// the number of ulps by which `approx` numbers may differ from the oracle (0 compares them exactly).
pub fn replay_suite_approx_ulps(json_text: &str, opts: &ReplayOptions, approx_ulps: u32) -> ReplayReport {
    let parsed = match json_in::parse(json_text) {
        Ok(v) => v,
        Err(e) => return fatal(format!("not JSON: {e}")),
    };
    let suite = match parsed.field("suite").and_then(Json::units) {
        Some(u) => String::from_utf16_lossy(u),
        None => return fatal("the suite name is missing".to_string()),
    };
    let Some(cases) = parsed.field("cases").and_then(Json::items) else {
        return fatal("cases must be an array".to_string());
    };
    let mut report = ReplayReport { suite, ..ReplayReport::default() };
    let mut seen: HashSet<String> = HashSet::new();
    for case in cases {
        report.total += 1;
        let id = case.field("id").and_then(Json::units).map(String::from_utf16_lossy).unwrap_or_else(|| "<no id>".to_string());
        if !seen.insert(id.clone()) {
            report.failures.push(Failure { id, reason: "duplicate case id".to_string(), expected: String::new(), actual: String::new() });
            continue;
        }
        match replay_case(case, opts, approx_ulps) {
            Outcome::Passed => report.passed += 1,
            Outcome::Failed { reason, expected, actual } => {
                report.failures.push(Failure { id, reason, expected, actual });
            }
            Outcome::Missing(target) => report.missing.push(Missing { id, target }),
        }
    }
    report
}

fn fatal(reason: String) -> ReplayReport {
    ReplayReport {
        suite: "<suite>".to_string(),
        failures: vec![Failure { id: "<suite>".to_string(), reason, expected: String::new(), actual: String::new() }],
        ..ReplayReport::default()
    }
}

enum Outcome {
    Passed,
    Failed { reason: String, expected: String, actual: String },
    Missing(String),
}

fn failed(reason: impl Into<String>) -> Outcome {
    Outcome::Failed { reason: reason.into(), expected: String::new(), actual: String::new() }
}

/// One case, read from its JSON (`docs/dev/builtin-corpus.md`, with the README conventions).
struct Spec {
    target: String,
    path: String,
    is_new: bool,
    is_static: bool,
    is_get: bool,
    approx: bool,
    cbs: Option<Vec<Option<String>>>,
    pre: Vec<Json>,
    post: Json,
}

impl Spec {
    fn read(case: &Json) -> Result<Spec, String> {
        let target = match case.field("target").and_then(Json::units) {
            Some(u) => String::from_utf16_lossy(u),
            None => return Err("the case has no target".to_string()),
        };
        let is_new = target.starts_with("new:");
        let path = if is_new { target["new:".len()..].to_string() } else { target.clone() };
        let is_static = case.field("static") == Some(&Json::Bool(true));
        let is_get = case.field("get").and_then(Json::num_text) == Some("1");
        let approx = case.field("approx").and_then(Json::num_text) == Some("1");
        if is_new && (is_static || is_get) {
            return Err("new: targets take no static or get flag".to_string());
        }
        if is_get && is_static {
            return Err("get targets take no static flag".to_string());
        }
        let cbs = match case.field("cbs") {
            None | Some(Json::Null) => None,
            Some(Json::Arr(items)) => {
                let mut names = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        Json::Null => names.push(None),
                        Json::Str(u) => {
                            let name = String::from_utf16_lossy(u);
                            if !PRESETS.iter().any(|(n, _)| *n == name) {
                                return Err(format!("unknown callback preset {name}"));
                            }
                            names.push(Some(name));
                        }
                        _ => return Err("cbs entries are preset names or null".to_string()),
                    }
                }
                Some(names)
            }
            Some(_) => return Err("cbs must be an array".to_string()),
        };
        let pre = match case.field("pre").and_then(Json::items) {
            Some(items) => items.to_vec(),
            None => return Err("pre must be an array".to_string()),
        };
        let post = match case.field("post") {
            Some(p @ Json::Obj(_)) => p.clone(),
            _ => return Err("post is missing".to_string()),
        };
        Ok(Spec { target, path, is_new, is_static, is_get, approx, cbs, pre, post })
    }
}

/// The value at `base[name]` for an own property, or `None` (the target path must be own properties).
fn own_value(rt: &Rt, base: &Value, name: &str) -> Option<Value> {
    let Value::Object(o) = base else {
        return None;
    };
    let key = PropKey::from_str(name);
    if !rt.has_own_property(*o, &key).ok()? {
        return None;
    }
    rt.get_v(base, &key).ok()
}

/// Resolves a dotted path from the global object: (owner of the last segment, its value).
fn resolve(rt: &Rt, path: &str) -> Option<(Value, Value)> {
    let segs: Vec<&str> = path.split('.').collect();
    let mut owner = Value::Object(rt.intrinsic(Intrinsic::Global));
    for seg in &segs[..segs.len() - 1] {
        owner = own_value(rt, &owner, seg)?;
    }
    let value = own_value(rt, &owner, segs[segs.len() - 1])?;
    Some((owner, value))
}

/// `nameOf(e)` of the generator: the thrown object's string `name`, else null.
fn name_json(rt: &Rt, e: &Value) -> Json {
    if matches!(e, Value::Object(_)) {
        if let Ok(Value::String(s)) = rt.get_str(e, "name") {
            return Json::Str(s.units().to_vec());
        }
    }
    Json::Null
}

fn message_of(rt: &Rt, e: &Throw) -> String {
    match rt.get_str(&e.0, "message") {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        _ => "<error>".to_string(),
    }
}

fn preset_named(presets: &[(&'static str, Value)], name: &str) -> Option<Value> {
    presets.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone())
}

fn is_same_object(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::Object(x), Value::Object(y)) if x == y)
}

fn replay_case(case: &Json, opts: &ReplayOptions, approx_ulps: u32) -> Outcome {
    let spec = match Spec::read(case) {
        Ok(s) => s,
        Err(reason) => return failed(reason),
    };
    let rt = Rt::new();
    if let Some(setup) = opts.setup {
        setup(&rt);
    }
    let Some((owner, target)) = resolve(&rt, &spec.path) else {
        return Outcome::Missing(spec.target.clone());
    };
    let shape_ok = if spec.is_get {
        matches!(target, Value::Number(_))
    } else if spec.is_new {
        rt.is_constructor(&target)
    } else {
        rt.is_callable(&target)
    };
    if !shape_ok {
        let want = if spec.is_get { "a number" } else if spec.is_new { "a constructor" } else { "a function" };
        return failed(format!("{} is not {want}", spec.path));
    }

    // Every callback in pre must be a known preset with its arity (the generator's rule).
    for item in &spec.pre {
        if let Some(reason) = check_callbacks(item) {
            return failed(reason);
        }
    }

    let log: ObjRef = rt.new_array(Vec::new());
    let presets = install_presets(&rt, log);
    let mut ids = IdTable::new();
    for (name, f) in &presets {
        let length = PRESETS.iter().find(|(n, _)| n == name).map(|(_, l)| *l).unwrap_or(0);
        ids.register_function(name, length, f.clone());
    }
    let values = match decode_pass(&rt, &spec.pre, &mut ids) {
        Ok(v) => v,
        Err(Throw(e)) => return failed(format!("pre cannot be decoded: {}", message_of(&rt, &Throw(e)))),
    };

    // The decoded inputs must encode back to the same pre (one pass, the same ordinals).
    let pre_rec = RefCell::new(Recorder::new());
    let round_trip = match encode_pass(&rt, &pre_rec, &values) {
        Ok(list) => list,
        Err(Throw(e)) => return failed(format!("pre does not re-encode: {}", message_of(&rt, &Throw(e)))),
    };
    if let Some((path, e, a)) = json_diff("pre", &Json::Arr(spec.pre.clone()), &Json::Arr(round_trip), 0) {
        return Outcome::Failed { reason: format!("pre does not round-trip at {path}"), expected: e, actual: a };
    }

    if spec.is_get && !values.is_empty() {
        return failed("get case has arguments");
    }
    if let Some(cbs) = &spec.cbs {
        for (j, name) in cbs.iter().enumerate() {
            if let Some(name) = name {
                let wanted = preset_named(&presets, name);
                let found = values.get(j);
                if !matches!((found, &wanted), (Some(v), Some(w)) if is_same_object(v, w)) {
                    return failed(format!("position {j} is not callback {name}"));
                }
            }
        }
    }

    // The call. `this` is the first decoded value for a method, and the owner is `this` for a static target.
    let has_this = !spec.is_static && !spec.is_new && !spec.is_get;
    let this = if has_this { values.first().cloned().unwrap_or(Value::Undefined) } else { Value::Undefined };
    let args: &[Value] = if has_this { values.get(1..).unwrap_or(&[]) } else { &values };
    let outcome = if spec.is_get {
        Ok(target.clone())
    } else if spec.is_new {
        rt.construct(&target, args, &target)
    } else if spec.is_static {
        rt.call(&target, &owner, args)
    } else {
        rt.call(&target, &this, args)
    };
    let (ret, thrown) = match outcome {
        Ok(v) => (v, Json::Null),
        Err(Throw(e)) => (Value::Null, Json::obj([("name", name_json(&rt, &e))])),
    };

    // The post state: one pass over [ret, this?, ...args, log].
    let mut list = vec![ret];
    if has_this {
        list.push(this);
    }
    list.extend(args.iter().cloned());
    list.push(Value::Object(log));
    let post_rec = RefCell::new(Recorder::new());
    let enc = match encode_pass(&rt, &post_rec, &list) {
        Ok(enc) => enc,
        Err(Throw(e)) => return failed(format!("post cannot be encoded: {}", message_of(&rt, &Throw(e)))),
    };
    let actual = Json::obj([
        ("ret", enc[0].clone()),
        ("throw", thrown),
        ("state", Json::Arr(enc[1..].to_vec())),
    ]);
    let expected = Json::obj([
        ("ret", spec.post.field("ret").cloned().unwrap_or(Json::Null)),
        ("throw", spec.post.field("throw").cloned().unwrap_or(Json::Null)),
        ("state", spec.post.field("state").cloned().unwrap_or(Json::Null)),
    ]);
    match json_diff("post", &expected, &actual, if spec.approx { approx_ulps } else { 0 }) {
        None => Outcome::Passed,
        Some((path, e, a)) => Outcome::Failed { reason: format!("first difference at {path}"), expected: e, actual: a },
    }
}

/// The callbacks inside one pre item: `fn` entries must name a preset with its arity.
fn check_callbacks(item: &Json) -> Option<String> {
    match item {
        Json::Obj(pairs) => {
            if item.field("$").and_then(Json::units).map(String::from_utf16_lossy).as_deref() == Some("fn") {
                let name = item.field("name").and_then(Json::units).map(String::from_utf16_lossy).unwrap_or_default();
                let length = item.field("length").and_then(Json::num_text).and_then(|t| t.parse::<u32>().ok());
                let known = PRESETS.iter().find(|(n, _)| *n == name).map(|(_, l)| *l);
                if known.is_none() || known != length {
                    return Some(format!("unknown callback {name}"));
                }
                return None;
            }
            pairs.iter().find_map(|(_, v)| check_callbacks(v))
        }
        Json::Arr(items) => items.iter().find_map(check_callbacks),
        _ => None,
    }
}

/// A structural difference: (path, expected text, actual text). Object keys are compared as sets, as the
/// oracle's compareDocs does. With a non-zero `approx`, numbers may differ by up to that many ulps, except the
/// structural `id`, `len` and `length` fields.
fn json_diff(path: &str, e: &Json, a: &Json, approx: u32) -> Option<(String, String, String)> {
    match (e, a) {
        (Json::Num(x), Json::Num(y)) => {
            if num_equal(x, y, approx) {
                None
            } else {
                Some((path.to_string(), x.clone(), y.clone()))
            }
        }
        (Json::Arr(xs), Json::Arr(ys)) => {
            if xs.len() != ys.len() {
                return Some((format!("{path}.length"), xs.len().to_string(), ys.len().to_string()));
            }
            xs.iter().zip(ys).enumerate().find_map(|(i, (x, y))| json_diff(&format!("{path}[{i}]"), x, y, approx))
        }
        (Json::Obj(xs), Json::Obj(ys)) => {
            for (k, xv) in xs {
                let name = String::from_utf16_lossy(k);
                let child = format!("{path}.{name}");
                let structural = matches!(name.as_str(), "id" | "len" | "length");
                match ys.iter().find(|(yk, _)| yk == k) {
                    Some((_, yv)) => {
                        if let Some(d) = json_diff(&child, xv, yv, if structural { 0 } else { approx }) {
                            return Some(d);
                        }
                    }
                    None => return Some((child, xv.to_json(), "<missing>".to_string())),
                }
            }
            for (k, yv) in ys {
                if !xs.iter().any(|(xk, _)| xk == k) {
                    let child = format!("{path}.{}", String::from_utf16_lossy(k));
                    return Some((child, "<missing>".to_string(), yv.to_json()));
                }
            }
            None
        }
        _ => {
            if e == a {
                None
            } else {
                Some((path.to_string(), e.to_json(), a.to_json()))
            }
        }
    }
}

fn num_equal(x: &str, y: &str, ulps: u32) -> bool {
    let (Ok(p), Ok(q)) = (x.parse::<f64>(), y.parse::<f64>()) else {
        return false;
    };
    if p == q {
        return true;
    }
    let (mut up, mut down) = (p, p);
    for _ in 0..ulps {
        up = up.next_up();
        down = down.next_down();
        if q == up || q == down {
            return true;
        }
    }
    false
}
