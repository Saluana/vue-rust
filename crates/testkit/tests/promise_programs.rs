//! Replays the promise-program corpus (WP-B5, tests/language/promise-programs/README.md) on the Rust runtime. Each program
//! runs through the public Promise API (`Promise`, `then`, `catch`, `finally`, the statics) and through async bodies built
//! on `async_start` and `await_value`. The outcome (log, final states, unhandled rejections) must equal the outcome that
//! Node 26 recorded in programs.json. Compiled-style rules: an async body keeps its values in frame slots, never in Rust
//! locals across an `.await`.

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use vn_testkit::json_in::parse;
use vn_testkit::json::Json;
use vn_value::builtins::promise::{async_start, await_value, frame_get, frame_set, new_frame, promise_state, PromiseState};
use vn_value::number::number_to_string;
use vn_value::{Call, Intrinsic, ObjRef, PropKey, Res, Rt, Throw, Value};

const CORPUS: &str = include_str!("../../../tests/language/promise-programs/programs.json");

/// A value written in a program.
#[derive(Clone, Debug)]
enum Item {
    Str(String),
    Num(f64),
    Undef,
    /// Index into the program's id table.
    Ref(usize),
}

#[derive(Clone, Debug)]
enum Handler {
    None,
    Log(String),
    LogArg,
    Return(Item),
    Throw(Item),
}

#[derive(Clone, Debug)]
enum Step {
    Await(Item),
    Log(String),
    LogCaught,
    Return(Item),
    Throw(Item),
    Try { body: Rc<Vec<Step>>, catch: Option<Rc<Vec<Step>>>, finally: Option<Rc<Vec<Step>>> },
}

#[derive(Clone, Debug)]
enum Op {
    New(usize),
    WithResolvers(usize),
    Resolve(usize, Item),
    Reject(usize, Item),
    Thenable(usize, String, Item),
    Then(usize, usize, Handler, Handler),
    Catch(usize, usize, Handler),
    Finally(usize, usize, Handler),
    Static(usize, String, Vec<Item>),
    Log(String),
    Queue(String),
    Async(usize, Rc<Vec<Step>>),
}

/// A parsed program. `names` maps table indexes back to ids (for messages), `order` lists the promise ids in creation order.
struct Program {
    name: String,
    ops: Vec<Op>,
    names: Vec<String>,
    expect: Expect,
}

#[derive(Debug, PartialEq)]
struct Expect {
    log: Vec<String>,
    states: Vec<(String, String)>,
    unhandled: Vec<String>,
}

#[derive(Debug, PartialEq)]
struct Outcome {
    log: Vec<String>,
    states: Vec<(String, String)>,
    unhandled: Vec<String>,
}

fn text(j: &Json) -> String {
    match j {
        Json::Str(units) => String::from_utf16_lossy(units),
        other => panic!("expected a string, found {other:?}"),
    }
}

fn field<'a>(j: &'a Json, key: &str) -> &'a Json {
    j.field(key).unwrap_or_else(|| panic!("program has no field {key:?}"))
}

fn items_of(j: &Json) -> &[Json] {
    j.items().expect("an array")
}

/// Assigns table indexes to ids in order of first mention.
struct Ids {
    names: Vec<String>,
}

impl Ids {
    fn index(&mut self, name: &str) -> usize {
        match self.names.iter().position(|n| n == name) {
            Some(i) => i,
            None => {
                self.names.push(name.to_string());
                self.names.len() - 1
            }
        }
    }

    fn item(&mut self, j: &Json) -> Item {
        match j {
            Json::Str(_) => Item::Str(text(j)),
            Json::Num(n) => Item::Num(n.parse().expect("a number")),
            Json::Obj(_) if field_opt(j, "undef").is_some() => Item::Undef,
            Json::Obj(_) => Item::Ref(self.index(&text(field(j, "ref")))),
            other => panic!("bad item {other:?}"),
        }
    }

    fn handler(&mut self, j: &Json) -> Handler {
        match text(field(j, "h")).as_str() {
            "none" => Handler::None,
            "log" => Handler::Log(text(field(j, "label"))),
            "logArg" => Handler::LogArg,
            "return" => Handler::Return(self.item(field(j, "v"))),
            "throw" => Handler::Throw(self.item(field(j, "v"))),
            other => panic!("unknown handler {other}"),
        }
    }

    fn steps(&mut self, j: &Json) -> Rc<Vec<Step>> {
        Rc::new(
            items_of(j)
                .iter()
                .map(|s| match text(field(s, "s")).as_str() {
                    "await" => Step::Await(self.item(field(s, "v"))),
                    "log" => Step::Log(text(field(s, "label"))),
                    "logCaught" => Step::LogCaught,
                    "return" => Step::Return(self.item(field(s, "v"))),
                    "throw" => Step::Throw(self.item(field(s, "v"))),
                    "try" => {
                        let body = self.steps(field(s, "body"));
                        let catch = match field(s, "catch") {
                            Json::Null => None,
                            c => Some(self.steps(c)),
                        };
                        let finally = match field(s, "finally") {
                            Json::Null => None,
                            f => Some(self.steps(f)),
                        };
                        Step::Try { body, catch, finally }
                    }
                    other => panic!("unknown step {other}"),
                })
                .collect(),
        )
    }

    fn op(&mut self, j: &Json) -> Op {
        let id = |me: &mut Ids, key: &str| me.index(&text(field(j, key)));
        match text(field(j, "op")).as_str() {
            "new" => Op::New(id(self, "id")),
            "withResolvers" => Op::WithResolvers(id(self, "id")),
            "resolve" => Op::Resolve(id(self, "id"), self.item(field(j, "v"))),
            "reject" => Op::Reject(id(self, "id"), self.item(field(j, "v"))),
            "thenable" => Op::Thenable(id(self, "id"), text(field(j, "mode")), self.item(field(j, "v"))),
            "then" => {
                let (i, s) = (id(self, "id"), id(self, "src"));
                Op::Then(i, s, self.handler(field(j, "f")), self.handler(field(j, "r")))
            }
            "catch" => {
                let (i, s) = (id(self, "id"), id(self, "src"));
                Op::Catch(i, s, self.handler(field(j, "r")))
            }
            "finally" => {
                let (i, s) = (id(self, "id"), id(self, "src"));
                Op::Finally(i, s, self.handler(field(j, "f")))
            }
            "static" => {
                let i = id(self, "id");
                let items = items_of(field(j, "items")).iter().map(|x| self.item(x)).collect();
                Op::Static(i, text(field(j, "fn")), items)
            }
            "log" => Op::Log(text(field(j, "label"))),
            "queue" => Op::Queue(text(field(j, "label"))),
            "async" => {
                let i = id(self, "id");
                Op::Async(i, self.steps(field(j, "steps")))
            }
            other => panic!("unknown op {other}"),
        }
    }
}

fn field_opt<'a>(j: &'a Json, key: &str) -> Option<&'a Json> {
    j.field(key)
}

fn parse_program(j: &Json) -> Program {
    let mut ids = Ids { names: Vec::new() };
    let ops = items_of(field(j, "ops")).iter().map(|o| ids.op(o)).collect();
    let expect_j = field(j, "expect");
    let log = items_of(field(expect_j, "log")).iter().map(text).collect();
    let states = items_of(field(expect_j, "states"))
        .iter()
        .map(|pair| {
            let p = items_of(pair);
            (text(&p[0]), text(&p[1]))
        })
        .collect();
    let unhandled = items_of(field(expect_j, "unhandled")).iter().map(text).collect();
    Program {
        name: text(field(j, "name")),
        ops,
        names: ids.names,
        expect: Expect { log, states, unhandled },
    }
}

/// Renders a value as the corpus does: strings as they are, other values as `String(v)`.
fn render(rt: &Rt, v: &Value) -> String {
    match v {
        Value::Undefined => "undefined".to_string(),
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => number_to_string(*n),
        Value::String(s) => s.to_rust_string_lossy(),
        other => match rt.to_string(other) {
            Ok(s) => s.to_rust_string_lossy(),
            Err(_) => "<unprintable>".to_string(),
        },
    }
}

fn global(rt: &Rt) -> Value {
    Value::Object(rt.intrinsic(Intrinsic::Global))
}

fn promise_ctor(rt: &Rt) -> Value {
    rt.get_str(&global(rt), "Promise").expect("Promise is installed")
}

fn push(rt: &Rt, arr: &Value, v: Value) -> Res<()> {
    let len = match rt.get_str(arr, "length")? {
        Value::Number(n) => n as u32,
        _ => 0,
    };
    rt.set_v_strict(arr, &PropKey::Index(len), v)
}

fn item_in(rt: &Rt, table: &Value, item: &Item) -> Res<Value> {
    Ok(match item {
        Item::Str(s) => Value::str(s),
        Item::Num(n) => Value::Number(*n),
        Item::Undef => Value::Undefined,
        Item::Ref(i) => rt.get_v(table, &PropKey::Index(*i as u32))?,
    })
}

/// Native helpers: each takes its state from its closure data (the log, the table, a label or a value).
fn label_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    push(rt, &d[0], d[1].clone())?;
    Ok(Value::Undefined)
}

fn logarg_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let log = rt.fn_data(c.callee)[0].clone();
    let text = render(rt, &c.arg(0));
    push(rt, &log, Value::str(&text))?;
    Ok(Value::Undefined)
}

fn value_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(rt.fn_data(c.callee)[0].clone())
}

fn throw_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Err(Throw(rt.fn_data(c.callee)[0].clone()))
}

/// Executor of `new Promise`: keeps its resolving functions in the holder array (closure data).
fn capture_fn(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let holder = rt.fn_data(c.callee)[0].clone();
    rt.set_v_strict(&holder, &PropKey::Index(0), c.arg(0))?;
    rt.set_v_strict(&holder, &PropKey::Index(1), c.arg(1))?;
    Ok(Value::Undefined)
}

/// The `then` of a thenable: logs `then:ID`, then does its mode (closure data: log, id, mode, value).
fn thenable_then(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let d = rt.fn_data(c.callee);
    let (log, id, mode, v) = (d[0].clone(), render(rt, &d[1]), render(rt, &d[2]), d[3].clone());
    push(rt, &log, Value::str(&format!("then:{id}")))?;
    let (res, rej) = (c.arg(0), c.arg(1));
    match mode.as_str() {
        "resolve" => {
            rt.call(&res, &Value::Undefined, &[v])?;
        }
        "reject" => {
            rt.call(&rej, &Value::Undefined, &[v])?;
        }
        "throw" => return Err(Throw(v)),
        "twice" => {
            rt.call(&res, &Value::Undefined, &[v])?;
            rt.call(&res, &Value::Undefined, &[Value::str("late")])?;
        }
        _ => {}
    }
    Ok(Value::Undefined)
}

/// The program's state during synchronous execution.
struct Interp {
    rt: Rt,
    log: Value,
    table: Value,
    holders: Value,
    names: Vec<String>,
    order: Vec<usize>,
}

impl Interp {
    fn item(&self, item: &Item) -> Res<Value> {
        item_in(&self.rt, &self.table, item)
    }

    fn set(&self, arr: &Value, i: usize, v: Value) {
        self.rt.set_v_strict(arr, &PropKey::Index(i as u32), v).expect("table slot");
    }

    fn native(&self, code: vn_value::CodeFn, name: &str, data: Vec<Value>) -> Value {
        Value::Object(self.rt.new_native_function(code, name, 0, data))
    }

    fn handler(&self, h: &Handler) -> Res<Value> {
        Ok(match h {
            Handler::None => Value::Undefined,
            Handler::Log(label) => self.native(label_fn, "log", vec![self.log.clone(), Value::str(label)]),
            Handler::LogArg => self.native(logarg_fn, "logArg", vec![self.log.clone()]),
            Handler::Return(item) => self.native(value_fn, "return", vec![self.item(item)?]),
            Handler::Throw(item) => self.native(throw_fn, "throw", vec![self.item(item)?]),
        })
    }

    /// `receiver[name](args)`, the way the corpus calls methods.
    fn invoke(&self, receiver: &Value, name: &str, args: &[Value]) -> Res<Value> {
        let f = self.rt.get_str(receiver, name)?;
        self.rt.call(&f, receiver, args)
    }

    fn table_get(&self, i: usize) -> Value {
        self.rt.get_v(&self.table, &PropKey::Index(i as u32)).expect("table entry")
    }

    fn run(&mut self, op: &Op) -> Res<()> {
        let rt = self.rt.clone();
        match op {
            Op::New(id) => {
                let holder = Value::Object(rt.new_array(vec![Value::Undefined, Value::Undefined]));
                let exec = self.native(capture_fn, "executor", vec![holder.clone()]);
                let ctor = promise_ctor(&rt);
                let p = rt.construct(&ctor, &[exec], &ctor)?;
                self.set(&self.table, *id, p);
                self.set(&self.holders, *id, holder);
                self.order.push(*id);
            }
            Op::WithResolvers(id) => {
                let obj = self.invoke(&promise_ctor(&rt), "withResolvers", &[])?;
                let promise = rt.get_str(&obj, "promise")?;
                let resolve = rt.get_str(&obj, "resolve")?;
                let reject = rt.get_str(&obj, "reject")?;
                let holder = Value::Object(rt.new_array(vec![resolve, reject]));
                self.set(&self.table, *id, promise);
                self.set(&self.holders, *id, holder);
                self.order.push(*id);
            }
            Op::Resolve(id, v) => {
                let holder = rt.get_v(&self.holders, &PropKey::Index(*id as u32))?;
                let resolve = rt.get_v(&holder, &PropKey::Index(0))?;
                rt.call(&resolve, &Value::Undefined, &[self.item(v)?])?;
            }
            Op::Reject(id, v) => {
                let holder = rt.get_v(&self.holders, &PropKey::Index(*id as u32))?;
                let reject = rt.get_v(&holder, &PropKey::Index(1))?;
                rt.call(&reject, &Value::Undefined, &[self.item(v)?])?;
            }
            Op::Thenable(id, mode, v) => {
                let obj = rt.new_object();
                let data = vec![
                    self.log.clone(),
                    Value::str(&self.names[*id]),
                    Value::str(mode),
                    self.item(v)?,
                ];
                let then = self.native(thenable_then, "then", data);
                rt.define_data(obj, PropKey::from_str("then"), then, vn_value::Flags::ALL)?;
                self.set(&self.table, *id, Value::Object(obj));
            }
            Op::Then(id, src, f, r) => {
                let p = self.table_get(*src);
                let (f, r) = (self.handler(f)?, self.handler(r)?);
                let q = self.invoke(&p, "then", &[f, r])?;
                self.set(&self.table, *id, q);
                self.order.push(*id);
            }
            Op::Catch(id, src, r) => {
                let p = self.table_get(*src);
                let r = self.handler(r)?;
                let q = self.invoke(&p, "catch", &[r])?;
                self.set(&self.table, *id, q);
                self.order.push(*id);
            }
            Op::Finally(id, src, f) => {
                let p = self.table_get(*src);
                let f = self.handler(f)?;
                let q = self.invoke(&p, "finally", &[f])?;
                self.set(&self.table, *id, q);
                self.order.push(*id);
            }
            Op::Static(id, name, items) => {
                let ctor = promise_ctor(&rt);
                let args = if name == "resolve" || name == "reject" {
                    vec![self.item(&items[0])?]
                } else {
                    let values = items.iter().map(|x| self.item(x)).collect::<Res<Vec<_>>>()?;
                    vec![Value::Object(rt.new_array(values))]
                };
                let q = self.invoke(&ctor, name, &args)?;
                self.set(&self.table, *id, q);
                self.order.push(*id);
            }
            Op::Log(label) => push(&rt, &self.log, Value::str(label))?,
            Op::Queue(label) => {
                let p = self.invoke(&promise_ctor(&rt), "resolve", &[Value::Undefined])?;
                let f = self.native(label_fn, "queued", vec![self.log.clone(), Value::str(label)]);
                self.invoke(&p, "then", &[f])?;
            }
            Op::Async(id, steps) => {
                let frame = new_frame(&rt, FRAME_SLOTS);
                frame_set(&rt, frame, 0, self.log.clone());
                frame_set(&rt, frame, 1, self.table.clone());
                let body: Body = Box::pin(run_body(rt.clone(), frame, steps.clone()));
                let result = async_start(&rt, frame, body);
                self.set(&self.table, *id, result);
                self.order.push(*id);
            }
        }
        Ok(())
    }
}

fn run_program(prog: &Program) -> Outcome {
    let rt = Rt::new();
    let n = prog.names.len();
    let log = Value::Object(rt.new_array(Vec::new()));
    let table = Value::Object(rt.new_array(vec![Value::Undefined; n]));
    let holders = Value::Object(rt.new_array(vec![Value::Undefined; n]));
    rt.root(&log);
    rt.root(&table);
    rt.root(&holders);
    let mut interp = Interp { rt: rt.clone(), log: log.clone(), table: table.clone(), holders, names: prog.names.clone(), order: Vec::new() };
    for op in &prog.ops {
        interp.run(op).unwrap_or_else(|Throw(v)| panic!("{}: an op threw {}", prog.name, render(&rt, &v)));
    }
    rt.run_jobs();
    assert!(rt.take_uncaught_job_errors().is_empty(), "{}: a job threw", prog.name);
    let unhandled = rt
        .take_unhandled_rejections()
        .iter()
        .map(|p| match promise_state(&rt, p) {
            Some(PromiseState::Rejected(v)) => render(&rt, &v),
            other => panic!("{}: an unhandled promise is not rejected: {other:?}", prog.name),
        })
        .collect();
    let states = interp
        .order
        .iter()
        .map(|&id| {
            let p = interp.table_get(id);
            let state = match promise_state(&rt, &p) {
                Some(PromiseState::Pending) => "pending".to_string(),
                Some(PromiseState::Fulfilled(v)) => format!("fulfilled:{}", render(&rt, &v)),
                Some(PromiseState::Rejected(v)) => format!("rejected:{}", render(&rt, &v)),
                None => panic!("{}: id {} is not a promise", prog.name, prog.names[id]),
            };
            (prog.names[id].clone(), state)
        })
        .collect();
    let entries = {
        let len = match rt.get_str(&log, "length").expect("length") {
            Value::Number(n) => n as u32,
            _ => 0,
        };
        (0..len).map(|i| render(&rt, &rt.get_v(&log, &PropKey::Index(i)).expect("entry"))).collect()
    };
    Outcome { log: entries, states, unhandled }
}

// ----- async bodies: the same steps, with values kept in frame slots -----

/// Frame layout: 0 = log array, 1 = table, 4 = the last caught value, 5 + 2d = completion kind of a try at depth d (0
/// normal, 1 return, 2 throw), 6 + 2d = its value.
const FRAME_SLOTS: usize = 24;

type Body = Pin<Box<dyn Future<Output = Res<Value>>>>;

enum Flow {
    Normal,
    Return(Value),
}

fn frame_log(rt: &Rt, frame: ObjRef, label: &str) -> Res<()> {
    let log = frame_get(rt, frame, 0);
    push(rt, &log, Value::str(label))
}

fn frame_item(rt: &Rt, frame: ObjRef, item: &Item) -> Res<Value> {
    let table = frame_get(rt, frame, 1);
    item_in(rt, &table, item)
}

fn frame_kind(rt: &Rt, frame: ObjRef, slot: usize) -> f64 {
    match frame_get(rt, frame, slot) {
        Value::Number(n) => n,
        _ => 0.0,
    }
}

/// Parks a completion in the frame, so the value is not held in a Rust local across the next `.await`.
fn store_completion(rt: &Rt, frame: ObjRef, slot: usize, completion: Res<Flow>) {
    let (kind, value) = match completion {
        Ok(Flow::Normal) => (0.0, Value::Undefined),
        Ok(Flow::Return(v)) => (1.0, v),
        Err(Throw(e)) => (2.0, e),
    };
    frame_set(rt, frame, slot, Value::Number(kind));
    frame_set(rt, frame, slot + 1, value);
}

async fn run_body(rt: Rt, frame: ObjRef, steps: Rc<Vec<Step>>) -> Res<Value> {
    match exec(rt, frame, steps, 0).await? {
        Flow::Return(v) => Ok(v),
        Flow::Normal => Ok(Value::Undefined),
    }
}

fn exec(rt: Rt, frame: ObjRef, steps: Rc<Vec<Step>>, depth: usize) -> Pin<Box<dyn Future<Output = Res<Flow>>>> {
    Box::pin(async move {
        for step in steps.iter() {
            match step {
                Step::Await(item) => {
                    let v = frame_item(&rt, frame, item)?;
                    await_value(&rt, v).await?;
                }
                Step::Log(label) => frame_log(&rt, frame, label)?,
                Step::LogCaught => {
                    let caught = frame_get(&rt, frame, 4);
                    let text = format!("caught:{}", render(&rt, &caught));
                    frame_log(&rt, frame, &text)?;
                }
                Step::Return(item) => return Ok(Flow::Return(frame_item(&rt, frame, item)?)),
                Step::Throw(item) => return Err(Throw(frame_item(&rt, frame, item)?)),
                Step::Try { body, catch, finally } => {
                    if let Flow::Return(v) = try_step(rt.clone(), frame, body.clone(), catch.clone(), finally.clone(), depth).await? {
                        return Ok(Flow::Return(v));
                    }
                }
            }
        }
        Ok(Flow::Normal)
    })
}

/// try / catch / finally. A finally block's own return is ignored, and only its throw replaces the pending completion
/// (the same as the generator in gen-b5.mjs).
fn try_step(
    rt: Rt,
    frame: ObjRef,
    body: Rc<Vec<Step>>,
    catch: Option<Rc<Vec<Step>>>,
    finally: Option<Rc<Vec<Step>>>,
    depth: usize,
) -> Pin<Box<dyn Future<Output = Res<Flow>>>> {
    Box::pin(async move {
        let kind_slot = 5 + 2 * depth;
        let first = exec(rt.clone(), frame, body, depth + 1).await;
        store_completion(&rt, frame, kind_slot, first);
        if let Some(c) = catch {
            if frame_kind(&rt, frame, kind_slot) == 2.0 {
                let thrown = frame_get(&rt, frame, kind_slot + 1);
                frame_set(&rt, frame, 4, thrown);
                let second = exec(rt.clone(), frame, c, depth + 1).await;
                store_completion(&rt, frame, kind_slot, second);
            }
        }
        if let Some(f) = finally {
            if let Err(e) = exec(rt.clone(), frame, f, depth + 1).await {
                return Err(e);
            }
        }
        match frame_kind(&rt, frame, kind_slot) {
            k if k == 1.0 => Ok(Flow::Return(frame_get(&rt, frame, kind_slot + 1))),
            k if k == 2.0 => Err(Throw(frame_get(&rt, frame, kind_slot + 1))),
            _ => Ok(Flow::Normal),
        }
    })
}

// ----- the replay -----

fn programs() -> Vec<Program> {
    let doc = parse(CORPUS).expect("programs.json is valid JSON");
    items_of(&doc).iter().map(parse_program).collect()
}

fn describe_mismatch(name: &str, want: &Expect, got: &Outcome) -> String {
    let mut out = format!("{name}:\n");
    if want.log != got.log {
        out.push_str(&format!("  log   want {:?}\n        got  {:?}\n", want.log, got.log));
    }
    if want.states != got.states {
        out.push_str(&format!("  states want {:?}\n         got  {:?}\n", want.states, got.states));
    }
    if want.unhandled != got.unhandled {
        out.push_str(&format!("  unhandled want {:?}\n            got  {:?}\n", want.unhandled, got.unhandled));
    }
    out
}

fn matches(want: &Expect, got: &Outcome) -> bool {
    want.log == got.log && want.states == got.states && want.unhandled == got.unhandled
}

#[test]
fn the_corpus_has_at_least_400_programs_with_documented_hand_designed_ones() {
    let all = programs();
    assert!(all.len() >= 400, "found {} programs", all.len());
    let doc = parse(CORPUS).expect("valid JSON");
    let hand = items_of(&doc).iter().take_while(|p| !text(field(p, "name")).starts_with("random-")).count();
    assert!(hand >= 100, "hand-designed programs: {hand}");
    for p in items_of(&doc) {
        assert!(!text(field(p, "doc")).is_empty(), "every program has a doc");
    }
}

#[test]
fn every_program_matches_node_v8() {
    let all = programs();
    let mut failures = Vec::new();
    for prog in &all {
        let got = run_program(prog);
        if !matches(&prog.expect, &got) {
            failures.push(describe_mismatch(&prog.name, &prog.expect, &got));
        }
    }
    for f in failures.iter().take(10) {
        eprintln!("{f}");
    }
    assert!(failures.is_empty(), "{} of {} programs differ from V8", failures.len(), all.len());
}

#[test]
fn a_corrupted_expectation_is_reported_as_a_mismatch() {
    // Negative control: the same program with one expected entry changed must not match.
    let all = programs();
    let prog = all.iter().find(|p| p.name == "then-resolved-queues-at-once").expect("the program exists");
    let got = run_program(prog);
    assert!(matches(&prog.expect, &got), "the unmodified program matches");
    let mut corrupted = Expect { log: prog.expect.log.clone(), states: prog.expect.states.clone(), unhandled: prog.expect.unhandled.clone() };
    corrupted.log[0] = "async".to_string();
    assert!(!matches(&corrupted, &got), "a changed log entry is detected");
    let mut corrupted = Expect { log: prog.expect.log.clone(), states: prog.expect.states.clone(), unhandled: prog.expect.unhandled.clone() };
    corrupted.states[0].1 = "rejected:x".to_string();
    assert!(!matches(&corrupted, &got), "a changed state is detected");
}
