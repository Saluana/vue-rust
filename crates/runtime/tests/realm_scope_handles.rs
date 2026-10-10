//! Native handles under an effect scope (WP-5.6, "native handles close on explicit scope disposal").
//!
//! A host handle is a DOM node that a scope owns through `onScopeDispose`. The close callback releases the node through
//! its `DocHost`, and the host's release hook records each release. The expected behaviour is the one of
//! `EffectScope.stop` in @vue/reactivity 3.5.43: effects, then the scope's own cleanups in registration order, then child
//! scopes; a stopped scope never runs again. docs/evidence/gc-latency.md records the Node run that checked these cases
//! against the package in node_modules.

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use vn_runtime::exports;
use vn_runtime::host::{Host, HostNode, HostNs};
use vn_runtime::host_doc::DocHost;
use vn_value::{Call, NativeObject, Res, Rt, Throw, Tracer, Value};

/// What the host released, and what each close callback observed while it ran.
#[derive(Default)]
struct Log {
    released: RefCell<Vec<HostNode>>,
    probes: RefCell<Vec<Probe>>,
}

#[derive(Debug, PartialEq, Eq)]
struct Probe {
    /// Allocating inside the close added exactly one object: the heap was open to JS.
    allocated: bool,
    /// `try_collect` was refused: a collection never runs inside a cleanup.
    collect_refused: bool,
}

/// A native handle: the node, and the host that owns it.
struct Handle {
    host: DocHost,
    node: HostNode,
    log: Rc<Log>,
}

impl NativeObject for Handle {
    fn trace(&self, _: &mut Tracer) {}
}

fn export(rt: &Rt, name: &str) -> Value {
    exports::all(rt).into_iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("the runtime exports {name}")).1
}

fn text(rt: &Rt, v: &Value, name: &str) -> String {
    match rt.get_str(v, name) {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        other => panic!("property {name}: {other:?}"),
    }
}

fn method(rt: &Rt, this: &Value, name: &str, args: &[Value]) -> Res<Value> {
    let f = rt.get_str(this, name)?;
    rt.call(&f, this, args)
}

fn new_scope(rt: &Rt, detached: bool) -> Value {
    let create = export(rt, "effectScope");
    rt.call(&create, &Value::Undefined, &[Value::Bool(detached)]).expect("effectScope(detached)")
}

fn button(host: &mut DocHost) -> HostNode {
    host.create_element(HostNs::Html, "button", None).expect("create a button")
}

fn record_releases(host: &DocHost, log: &Rc<Log>) {
    let log = log.clone();
    host.add_release_hook(Rc::new(move |node: HostNode| log.released.borrow_mut().push(node)));
}

/// The close callback of a handle. It releases the node through the host, and checks on the way that JS can run here:
/// it allocates, and it is refused a collection, because code is running.
fn close_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Value::Object(o) = rt.fn_data(c.callee)[0].clone() else {
        panic!("a close callback holds its handle");
    };
    let payload = rt.native_payload(o).expect("the handle object has a native payload");
    let any: &dyn Any = &*payload;
    let handle = any.downcast_ref::<Handle>().expect("the payload is a Handle");
    let before = rt.heap_stats().live_objects;
    let _fresh = rt.new_object();
    let allocated = rt.heap_stats().live_objects == before + 1;
    handle.log.probes.borrow_mut().push(Probe { allocated, collect_refused: !rt.try_collect() });
    let mut host = handle.host.clone();
    host.release(handle.node);
    Ok(Value::Undefined)
}

/// A close callback for `node`, owned by `host`.
fn close_fn(rt: &Rt, host: &DocHost, log: &Rc<Log>, node: HostNode) -> Value {
    let handle = rt.new_native_object(None, Rc::new(Handle { host: host.clone(), node, log: log.clone() }));
    Value::Object(rt.new_native_function(close_code, "close", 0, vec![Value::Object(handle)]))
}

/// Registers `data[1]` with `data[0]` (`onScopeDispose`). It runs inside `scope.run`, so the active scope owns it.
fn register_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee).to_vec();
    rt.call(&data[0], &Value::Undefined, &[data[1].clone()])
}

/// `scope.run(register)`: the cleanup belongs to `scope` if the scope is live, and is not registered if it is stopped.
fn register_in(rt: &Rt, scope: &Value, cleanup: Value) {
    let on_dispose = export(rt, "onScopeDispose");
    let register = Value::Object(rt.new_native_function(register_code, "register", 0, vec![on_dispose, cleanup]));
    method(rt, scope, "run", &[register]).expect("scope.run");
}

fn stop(rt: &Rt, scope: &Value) {
    method(rt, scope, "stop", &[]).expect("scope.stop");
}

/// Creates a child scope inside the active scope and registers a close callback on it. Returns the child.
fn child_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let data = rt.fn_data(c.callee).to_vec();
    let child = rt.call(&data[0], &Value::Undefined, &[Value::Bool(false)])?;
    register_in(rt, &child, data[1].clone());
    Ok(child)
}

fn throw_code(rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Err(rt.throw_type_error("boom"))
}

/// A cleanup that stops the scope in `data[0]`.
fn stop_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let scope = rt.fn_data(c.callee)[0].clone();
    method(rt, &scope, "stop", &[])
}

#[test]
fn handles_registered_under_a_scope_close_once_when_it_stops() {
    let rt = Rt::new();
    let mut host = DocHost::new();
    let log = Rc::new(Log::default());
    record_releases(&host, &log);
    let (a, b, c) = (button(&mut host), button(&mut host), button(&mut host));

    let scope = new_scope(&rt, false);
    for node in [a, b, c] {
        let close = close_fn(&rt, &host, &log, node);
        register_in(&rt, &scope, close);
    }
    assert!(log.released.borrow().is_empty(), "nothing is closed while the scope is live");

    stop(&rt, &scope);
    assert_eq!(*log.released.borrow(), vec![a, b, c], "each handle closes once, in registration order");
    assert_eq!(log.probes.borrow().len(), 3, "each close callback ran once");

    stop(&rt, &scope);
    assert_eq!(log.released.borrow().len(), 3, "a second stop closes nothing");
    assert_eq!(log.probes.borrow().len(), 3, "a second stop runs no close callback");

    // A callback registered on a stopped scope never runs (`scope.run` returns without calling it), so its node is not
    // closed by the scope.
    let d = button(&mut host);
    let late = close_fn(&rt, &host, &log, d);
    register_in(&rt, &scope, late);
    stop(&rt, &scope);
    assert_eq!(log.released.borrow().len(), 3, "a callback registered after the stop never runs");
    assert!(log.probes.borrow().len() == 3, "exactly three close callbacks ran");
}

#[test]
fn an_open_handle_survives_collections_and_closes_when_its_scope_stops() {
    let rt = Rt::new();
    let mut host = DocHost::new();
    let log = Rc::new(Log::default());
    record_releases(&host, &log);
    let n = button(&mut host);

    // Rust locals are not roots: the scope is pinned for as long as its owner (here, the test) keeps it.
    let scope = new_scope(&rt, false);
    let pin = rt.root(&scope);
    register_in(&rt, &scope, close_fn(&rt, &host, &log, n));
    for _ in 0..3 {
        rt.collect();
        assert!(log.released.borrow().is_empty(), "a collection does not close a handle whose scope is live");
    }
    // The cleanup survived the collections, so stopping the scope still runs it.
    stop(&rt, &scope);
    assert_eq!(*log.released.borrow(), vec![n]);
    assert_eq!(log.probes.borrow().len(), 1, "the close ran exactly once");
    rt.unroot(pin);
}

#[test]
fn collecting_an_unstopped_scope_does_not_close_its_handles() {
    // Characterization of the limit: closing is explicit. An unreferenced scope that nobody stopped is collected with
    // its cleanups, and its handles stay open until the host itself releases them. @vue/reactivity does not run
    // onScopeDispose on collection either.
    let rt = Rt::new();
    let mut host = DocHost::new();
    let log = Rc::new(Log::default());
    record_releases(&host, &log);
    let n = button(&mut host);

    let scope = new_scope(&rt, false);
    let scope_obj = match &scope {
        Value::Object(o) => *o,
        other => panic!("effectScope returns an object: {other:?}"),
    };
    register_in(&rt, &scope, close_fn(&rt, &host, &log, n));
    drop(scope);
    rt.collect();
    assert!(!rt.is_live(scope_obj), "the unreferenced scope is collected");
    assert!(log.released.borrow().is_empty(), "collection does not close the scope's handles");
    assert!(log.probes.borrow().is_empty(), "no cleanup ran");
}

#[test]
fn a_collection_is_allowed_outside_a_cleanup() {
    // Negative control for the probe: the same `try_collect` that the close callback sees as refused succeeds when no
    // code is running, so the refusal inside the cleanup comes from running code and not from a broken probe.
    let rt = Rt::new();
    assert!(rt.try_collect(), "outside JS code a collection runs");
}

#[test]
fn closing_runs_js_with_the_heap_open_and_no_collection_inside() {
    let rt = Rt::new();
    let mut host = DocHost::new();
    let log = Rc::new(Log::default());
    record_releases(&host, &log);
    let n = button(&mut host);

    let scope = new_scope(&rt, false);
    register_in(&rt, &scope, close_fn(&rt, &host, &log, n));
    stop(&rt, &scope);
    assert_eq!(*log.probes.borrow(), vec![Probe { allocated: true, collect_refused: true }]);
    assert_eq!(*log.released.borrow(), vec![n]);
}

#[test]
fn a_child_scopes_handles_close_with_the_parent_and_not_again() {
    let rt = Rt::new();
    let mut host = DocHost::new();
    let log = Rc::new(Log::default());
    record_releases(&host, &log);
    let (parent_node, child_node) = (button(&mut host), button(&mut host));

    let parent = new_scope(&rt, false);
    register_in(&rt, &parent, close_fn(&rt, &host, &log, parent_node));
    let make_child = Value::Object(rt.new_native_function(
        child_code,
        "child",
        0,
        vec![export(&rt, "effectScope"), close_fn(&rt, &host, &log, child_node)],
    ));
    let child = method(&rt, &parent, "run", &[make_child]).expect("the parent runs the child factory");

    stop(&rt, &parent);
    assert_eq!(
        *log.released.borrow(),
        vec![parent_node, child_node],
        "the parent's own cleanup runs first, then the child's"
    );
    assert_eq!(log.probes.borrow().len(), 2, "each of the two close callbacks ran once");
    stop(&rt, &child);
    assert_eq!(log.released.borrow().len(), 2, "stopping the child again closes nothing");
    assert_eq!(log.probes.borrow().len(), 2, "stopping the child again runs no close callback");
}

#[test]
fn a_scope_closes_only_its_own_handles() {
    let rt = Rt::new();
    let mut host = DocHost::new();
    let log = Rc::new(Log::default());
    record_releases(&host, &log);
    let (x, y, z) = (button(&mut host), button(&mut host), button(&mut host));

    let first = new_scope(&rt, false);
    let second = new_scope(&rt, false);
    register_in(&rt, &first, close_fn(&rt, &host, &log, x));
    register_in(&rt, &second, close_fn(&rt, &host, &log, y));
    // Negative control: a cleanup registered with no active scope is a no-op, so no scope can close `z`.
    let on_dispose = export(&rt, "onScopeDispose");
    rt.call(&on_dispose, &Value::Undefined, &[close_fn(&rt, &host, &log, z)])
        .expect("onScopeDispose with no active scope does nothing");

    stop(&rt, &first);
    assert_eq!(*log.released.borrow(), vec![x], "stopping the first scope closes only its handle");
    assert_eq!(log.probes.borrow().len(), 1, "stopping the first scope runs only its close callback");
    stop(&rt, &second);
    assert_eq!(*log.released.borrow(), vec![x, y]);
    assert_eq!(log.probes.borrow().len(), 2, "stopping the second scope runs only its close callback");
    assert!(!log.released.borrow().contains(&z), "a handle registered with no scope is never closed by a scope");
}

#[test]
fn a_throwing_cleanup_aborts_the_stop_and_later_handles_stay_open() {
    // Characterization of the gap: @vue/reactivity 3.5.43 behaves the same way. The stop reports the throw, the cleanups
    // after it do not run, and a second stop is a no-op, so those handles are never closed by this scope.
    let rt = Rt::new();
    let mut host = DocHost::new();
    let log = Rc::new(Log::default());
    record_releases(&host, &log);
    let n = button(&mut host);

    let scope = new_scope(&rt, false);
    let thrower = Value::Object(rt.new_native_function(throw_code, "thrower", 0, vec![]));
    register_in(&rt, &scope, thrower);
    register_in(&rt, &scope, close_fn(&rt, &host, &log, n));
    let Throw(e) = method(&rt, &scope, "stop", &[]).expect_err("the stop reports the throw");
    assert_eq!(text(&rt, &e, "message"), "boom");
    assert!(log.released.borrow().is_empty(), "the stop ended at the throwing cleanup");
    assert!(log.probes.borrow().is_empty(), "the later close callback did not run during the stop");
    stop(&rt, &scope);
    assert!(log.released.borrow().is_empty(), "a second stop does not reach the later handle");
    assert!(log.probes.borrow().is_empty(), "a second stop does not run the later close callback");
}

#[test]
fn a_cleanup_that_stops_its_own_scope_closes_its_handle_once() {
    let rt = Rt::new();
    let mut host = DocHost::new();
    let log = Rc::new(Log::default());
    record_releases(&host, &log);
    let n = button(&mut host);

    let scope = new_scope(&rt, false);
    register_in(&rt, &scope, close_fn(&rt, &host, &log, n));
    let stop_self = Value::Object(rt.new_native_function(stop_code, "stopSelf", 0, vec![scope.clone()]));
    register_in(&rt, &scope, stop_self);
    stop(&rt, &scope);
    assert_eq!(*log.released.borrow(), vec![n], "the nested stop does not close the handle again");
    assert_eq!(log.probes.borrow().len(), 1);
}
