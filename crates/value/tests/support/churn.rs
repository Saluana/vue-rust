//! Graph-churn workload for the collector (WP-3.2, WP-5.6). Shared by tests/gc_churn.rs, tests/gc_exhaustion.rs and
//! examples/gc_churn_bench.rs (through `#[path]`), so the tests and the benchmark run the same shapes.
//!
//! The workload drives the public vn-value API only: objects, arrays, Map and Set, closures over cells, native
//! functions, promises with their reaction jobs, and the job queue. Every object it creates is recorded as a node
//! with the objects it references (`edges`). Reachability is computed here, from the roots the workload holds
//! (`roots`, `queued`), never by the collector, and each collection is checked against it (`Churn::verify`).
//!
//! Rust locals are not roots. An operation attaches each object it creates to a root, or to another attached object,
//! before the next safepoint, and `Churn::safepoint` runs only between operations.

#![allow(dead_code)]

use std::collections::HashMap;
use std::time::Instant;

use vn_value::builtins::map_set;
use vn_value::builtins::promise::{self, PromiseState};
use vn_value::{Call, Flags, FnInfo, FnKind, Intrinsic, ObjRef, PropKey, Res, Rt, RootId, Throw, Value};

/// Sizes of one churn round. Each field is a count of the structures the round builds.
#[derive(Clone, Copy, Debug, Default)]
pub struct Shape {
    /// Nodes in the linked list (its head stays pinned across the round boundary).
    pub list: usize,
    /// Number of cycles.
    pub rings: usize,
    /// Objects per cycle.
    pub ring_size: usize,
    /// Numeric properties of the wide object.
    pub width: usize,
    /// Map entries with object keys and values, and Set members.
    pub pairs: usize,
    /// Closures, each capturing one object through a cell.
    pub closures: usize,
    /// Native functions, each holding one object as its data.
    pub natives: usize,
    /// Fulfilled promises chained with `then`; each reaction job runs once.
    pub promises: usize,
}

/// The stable live set: it is rooted for the whole run and must survive every collection unchanged.
#[derive(Clone, Copy, Debug)]
pub struct Stable {
    pub root: RootId,
    /// Array of the `n` ring objects, then the Map and the probe closure.
    pub arr: usize,
    pub map: usize,
    pub probe: usize,
    pub n: usize,
}

/// Counts reported for one round.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoundStats {
    /// Objects the round created (nodes added to the model).
    pub created: usize,
    /// Collections that ran at a safepoint because the allocation threshold was reached.
    pub automatic: usize,
}

/// A JavaScript throw as text, for test failures.
pub fn js<T>(rt: &Rt, r: Res<T>) -> Result<T, String> {
    r.map_err(|Throw(e)| {
        let text = |k: &str| match rt.get_str(&e, k) {
            Ok(Value::String(s)) => s.to_rust_string_lossy(),
            _ => String::from("?"),
        };
        format!("uncaught {}: {}", text("name"), text("message"))
    })
}

fn identity_code(_rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(c.arg(0))
}

/// A promise handler that allocates a fresh object during the job: `{ from: arg }`.
fn wrap_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let r = rt.new_object();
    rt.define_data(r, PropKey::from_str("from"), c.arg(0), Flags::ALL)?;
    Ok(Value::Object(r))
}

/// Executor of `new Promise(resolve => resolve(data[0]))`.
fn fulfil_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let v = rt.fn_data(c.callee)[0].clone();
    rt.call(&c.arg(0), &Value::Undefined, &[v])?;
    Ok(Value::Undefined)
}

/// Body of a closure: returns the object its cell captured.
fn closure_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let cell = rt.captures(c.callee)[0];
    Ok(rt.cell_get(cell))
}

/// Body of a native function: returns its first data value.
fn data_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    Ok(rt.fn_data(c.callee)[0].clone())
}

/// A queued job: marks its argument.
fn mark_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    rt.set_v_strict(&c.arg(0), &PropKey::from_str("ran"), Value::Bool(true))?;
    Ok(Value::Undefined)
}

/// The model of the heap: every object the workload created, what each one references, and the roots it holds.
pub struct Churn<'a> {
    pub rt: &'a Rt,
    nodes: Vec<ObjRef>,
    edges: Vec<Vec<usize>>,
    index: HashMap<ObjRef, usize>,
    /// Nodes the model believes are alive (the collector agreed at the last check).
    live: Vec<usize>,
    /// Persistent roots held by the workload, with the node each one pins.
    roots: Vec<(RootId, usize)>,
    /// Nodes that queued jobs reference. They are roots until the queue drains.
    queued: Vec<usize>,
    baseline: usize,
    collections: u64,
    automatic: usize,
    /// Every collection that ran through `collect` or `safepoint`: its pause in milliseconds and the objects it freed.
    /// Only the collector call is timed; the model check that follows is not.
    pub pauses: Vec<(f64, usize)>,
}

impl<'a> Churn<'a> {
    /// A model on `rt`, which must hold only the realm's own objects. It collects once and records their count.
    pub fn new(rt: &'a Rt) -> Churn<'a> {
        rt.collect();
        Churn {
            rt,
            nodes: Vec::new(),
            edges: Vec::new(),
            index: HashMap::new(),
            live: Vec::new(),
            roots: Vec::new(),
            queued: Vec::new(),
            baseline: rt.heap_stats().live_objects,
            collections: 0,
            automatic: 0,
            pauses: Vec::new(),
        }
    }

    /// Live objects owned by the realm itself (intrinsics, the global object, the Promise realm, ...).
    pub fn baseline(&self) -> usize {
        self.baseline
    }

    /// Collections checked against the model so far.
    pub fn collections(&self) -> u64 {
        self.collections
    }

    /// Collections that ran at a safepoint because the allocation threshold was reached.
    pub fn automatic(&self) -> usize {
        self.automatic
    }

    /// Objects the workload has created so far (live or freed).
    pub fn created(&self) -> usize {
        self.nodes.len()
    }

    /// Objects the model considers alive.
    pub fn live_nodes(&self) -> usize {
        self.live.len()
    }

    pub fn handle(&self, n: usize) -> ObjRef {
        self.nodes[n]
    }

    pub fn value(&self, n: usize) -> Value {
        Value::Object(self.nodes[n])
    }

    fn track(&mut self, h: ObjRef) -> usize {
        let n = self.nodes.len();
        self.nodes.push(h);
        self.edges.push(Vec::new());
        self.index.insert(h, n);
        self.live.push(n);
        n
    }

    fn link(&mut self, from: usize, to: usize) {
        self.edges[from].push(to);
    }

    pub fn set_num(&mut self, n: usize, key: &str, x: f64) -> Res<()> {
        self.rt.define_data(self.nodes[n], PropKey::from_str(key), Value::Number(x), Flags::ALL)?;
        Ok(())
    }

    fn set_ref(&mut self, from: usize, key: &str, to: usize) -> Res<()> {
        let v = self.value(to);
        self.rt.define_data(self.nodes[from], PropKey::from_str(key), v, Flags::ALL)?;
        self.link(from, to);
        Ok(())
    }

    /// A new object with no properties.
    pub fn object(&mut self) -> usize {
        let h = self.rt.new_object();
        self.track(h)
    }

    /// A new array whose elements are `elems`.
    pub fn array(&mut self, elems: &[usize]) -> usize {
        let vals: Vec<Value> = elems.iter().map(|&e| self.value(e)).collect();
        let h = self.rt.new_array(vals);
        let a = self.track(h);
        for &e in elems {
            self.link(a, e);
        }
        a
    }

    /// A linked list of `len` objects (`next` from head to tail). Each object's `id` is its position from the head.
    /// Returns the head.
    pub fn list(&mut self, len: usize) -> Res<usize> {
        let mut head: Option<usize> = None;
        for id in (0..len).rev() {
            let n = self.object();
            self.set_num(n, "id", id as f64)?;
            if let Some(next) = head {
                self.set_ref(n, "next", next)?;
            }
            head = Some(n);
        }
        Ok(head.expect("a list has at least one node"))
    }

    /// `rings` cycles of `size` objects. Each object's `next` is the following one, and the last points back to the
    /// first. Returns the first object of each cycle.
    pub fn rings(&mut self, rings: usize, size: usize) -> Res<Vec<usize>> {
        let mut heads = Vec::with_capacity(rings);
        for _ in 0..rings {
            let ring: Vec<usize> = (0..size).map(|_| self.object()).collect();
            for k in 0..size {
                self.set_num(ring[k], "id", k as f64)?;
                self.set_ref(ring[k], "next", ring[(k + 1) % size])?;
            }
            heads.push(ring[0]);
        }
        Ok(heads)
    }

    /// An object with `width` numeric properties `p0..` and a `child` object. Returns the object.
    pub fn wide(&mut self, width: usize) -> Res<usize> {
        let n = self.object();
        for k in 0..width {
            self.set_num(n, &format!("p{k}"), k as f64)?;
        }
        let child = self.object();
        self.set_ref(n, "child", child)?;
        Ok(n)
    }

    /// A Map from `pairs` new object keys to `pairs` new object values. Returns the Map and its (key, value) nodes.
    pub fn map(&mut self, pairs: usize) -> Res<(usize, Vec<(usize, usize)>)> {
        let mh = map_set::new_map(self.rt);
        let m = self.track(mh);
        let mut kv = Vec::with_capacity(pairs);
        for k in 0..pairs {
            let key = self.object();
            self.set_num(key, "id", k as f64)?;
            let val = self.object();
            self.set_num(val, "id", k as f64)?;
            let (kh, vh) = (self.nodes[key], self.nodes[val]);
            assert!(map_set::map_put(self.rt, mh, Value::Object(kh), Value::Object(vh)), "map_put on a Map");
            self.link(m, key);
            self.link(m, val);
            kv.push((key, val));
        }
        Ok((m, kv))
    }

    /// A Map from the numbers `0..objs.len()` to `objs`.
    pub fn map_numbers(&mut self, objs: &[usize]) -> usize {
        let mh = map_set::new_map(self.rt);
        let m = self.track(mh);
        for (k, &o) in objs.iter().enumerate() {
            let oh = self.nodes[o];
            assert!(map_set::map_put(self.rt, mh, Value::Number(k as f64), Value::Object(oh)), "map_put on a Map");
            self.link(m, o);
        }
        m
    }

    /// A Set of `members` new objects. Returns the Set and its members.
    pub fn set(&mut self, members: usize) -> Res<(usize, Vec<usize>)> {
        let sh = map_set::new_set(self.rt);
        let s = self.track(sh);
        let mut out = Vec::with_capacity(members);
        for k in 0..members {
            let o = self.object();
            self.set_num(o, "id", k as f64)?;
            let oh = self.nodes[o];
            assert!(map_set::set_put(self.rt, sh, Value::Object(oh)), "set_put on a Set");
            self.link(s, o);
            out.push(o);
        }
        Ok((s, out))
    }

    /// A closure over `obj`: function, then a cell that holds `obj`.
    pub fn closure(&mut self, obj: usize) -> usize {
        let cell_h = self.rt.new_cell(self.value(obj));
        let cell = self.track(cell_h);
        self.link(cell, obj);
        let fh = self.rt.new_function(closure_code, FnInfo::new("closure", 0, FnKind::Normal), &[cell_h]);
        let f = self.track(fh);
        self.link(f, cell);
        f
    }

    /// A native function whose data holds `obj`.
    pub fn native(&mut self, obj: usize) -> usize {
        let fh = self.rt.new_native_function(data_code, "native", 0, vec![self.value(obj)]);
        let f = self.track(fh);
        self.link(f, obj);
        f
    }

    /// A promise handler: `wrap` allocates a result object during the job, otherwise the argument passes through.
    pub fn handler(&mut self, wrap: bool) -> usize {
        let code: vn_value::CodeFn = if wrap { wrap_code } else { identity_code };
        let fh = self.rt.new_native_function(code, "handler", 1, vec![]);
        self.track(fh)
    }

    fn promise_ctor(&self) -> Res<Value> {
        let global = Value::Object(self.rt.intrinsic(Intrinsic::Global));
        self.rt.get_str(&global, "Promise")
    }

    /// `new Promise(resolve => resolve(obj))`: a promise fulfilled with `obj`, which it references.
    pub fn fulfilled(&mut self, obj: usize) -> Res<usize> {
        let exec_h = self.rt.new_native_function(fulfil_code, "executor", 1, vec![self.value(obj)]);
        let exec = self.track(exec_h);
        self.link(exec, obj);
        let ctor = self.promise_ctor()?;
        let p_v = self.rt.construct(&ctor, &[Value::Object(exec_h)], &ctor)?;
        let p_h = p_v.as_object().expect("construct(Promise) returns an object");
        let p = self.track(p_h);
        self.link(p, obj);
        Ok(p)
    }

    /// `p.then(handler)` on a settled promise: queues one reaction job. Returns the derived promise (pending until the
    /// queue drains).
    pub fn then(&mut self, p: usize, handler: usize) -> Res<usize> {
        let pv = self.value(p);
        let method = self.rt.get_str(&pv, "then")?;
        let hv = self.value(handler);
        let q_v = self.rt.call(&method, &pv, &[hv])?;
        let qh = q_v.as_object().expect("then returns an object");
        Ok(self.track(qh))
    }

    /// After the queue drained: `q` is fulfilled with the value its handler returned. A result object the job created is
    /// registered here; `wrap_code` stores its argument as `from`, so it references that argument.
    pub fn settle(&mut self, q: usize) -> Res<()> {
        let result = match promise::promise_state(self.rt, &self.value(q)) {
            Some(PromiseState::Fulfilled(Value::Object(r))) => r,
            other => panic!("promise {q} is not fulfilled with an object: {other:?}"),
        };
        let r_node = match self.index.get(&result).copied() {
            Some(n) => n,
            None => {
                let n = self.track(result);
                if let Value::Object(from) = self.rt.get_str(&Value::Object(result), "from")? {
                    let from_node = self.index[&from];
                    self.link(n, from_node);
                }
                n
            }
        };
        self.link(q, r_node);
        Ok(())
    }

    /// Queues a job `mark(obj)`. The job and `obj` are roots until the queue drains.
    pub fn enqueue_mark(&mut self, obj: usize) {
        let fh = self.rt.new_native_function(mark_code, "mark", 1, vec![]);
        let f = self.track(fh);
        let ov = self.value(obj);
        self.rt.enqueue_job(Value::Object(fh), vec![ov]);
        self.queued.push(f);
        self.queued.push(obj);
    }

    /// Runs the job queue to empty. Returns the number of jobs that ran.
    pub fn drain(&mut self) -> usize {
        let ran = self.rt.run_jobs();
        self.queued.clear();
        ran
    }

    /// Appends `n` to the array `holder` and returns its index. The array is the model's holder: `n` is reachable
    /// while the holder is.
    pub fn attach(&mut self, holder: usize, n: usize) -> Res<usize> {
        let hv = self.value(holder);
        let slot = match self.rt.get_str(&hv, "length")? {
            Value::Number(l) => l as usize,
            _ => panic!("holder is not an array"),
        };
        let nv = self.value(n);
        self.rt.set_v_strict(&hv, &PropKey::Index(slot as u32), nv)?;
        self.link(holder, n);
        Ok(slot)
    }

    /// Clears `holder[slot]`, which held `n`. The model drops the edge too.
    pub fn detach(&mut self, holder: usize, slot: usize, n: usize) -> Res<()> {
        let hv = self.value(holder);
        self.rt.set_v_strict(&hv, &PropKey::Index(slot as u32), Value::Undefined)?;
        self.edges[holder].retain(|&x| x != n);
        Ok(())
    }

    /// Adds a persistent root for `n`.
    pub fn pin(&mut self, n: usize) -> RootId {
        let id = self.rt.root(&self.value(n));
        self.roots.push((id, n));
        id
    }

    /// Removes a persistent root added by `pin`.
    pub fn unpin(&mut self, id: RootId) {
        self.rt.unroot(id);
        self.roots.retain(|&(r, _)| r != id);
    }

    fn reachable(&self) -> Vec<bool> {
        let mut seen = vec![false; self.nodes.len()];
        let mut work: Vec<usize> = Vec::new();
        for &n in self.roots.iter().map(|(_, n)| n).chain(self.queued.iter()) {
            if !seen[n] {
                seen[n] = true;
                work.push(n);
            }
        }
        while let Some(n) = work.pop() {
            for &e in &self.edges[n] {
                if !seen[e] {
                    seen[e] = true;
                    work.push(e);
                }
            }
        }
        seen
    }

    fn record_pause(&mut self, started: Instant, freed_before: u64) {
        let ms = started.elapsed().as_secs_f64() * 1e3;
        let freed = (self.rt.heap_stats().freed_total - freed_before) as usize;
        self.pauses.push((ms, freed));
    }

    /// Checks the last collection against the model. Every model-reachable object must still be live, every other
    /// tracked object must be freed, and the live-object count must be the realm's baseline plus the reachable
    /// objects (so an object the model does not track, such as runtime scaffolding, is reported too). Returns the
    /// number of reachable objects.
    pub fn verify(&mut self) -> Result<usize, String> {
        let reach = self.reachable();
        let mut kept = Vec::with_capacity(self.live.len());
        for &n in &self.live {
            let live = self.rt.is_live(self.nodes[n]);
            match (reach[n], live) {
                (true, true) => kept.push(n),
                (true, false) => {
                    return Err(format!("object {n} is reachable from a root but the collector freed it"));
                }
                (false, true) => {
                    return Err(format!("object {n} is unreachable but survived the collection"));
                }
                (false, false) => {
                    self.index.remove(&self.nodes[n]);
                    self.edges[n] = Vec::new();
                }
            }
        }
        self.live = kept;
        self.collections += 1;
        let got = self.rt.heap_stats().live_objects;
        let want = self.baseline + self.live.len();
        if got != want {
            return Err(format!(
                "live objects {got}, model expects {want} (baseline {} + {} reachable)",
                self.baseline,
                self.live.len()
            ));
        }
        Ok(self.live.len())
    }

    /// Collects now and checks the collection against the model.
    pub fn collect(&mut self) -> Result<usize, String> {
        let freed_before = self.rt.heap_stats().freed_total;
        let started = Instant::now();
        self.rt.collect();
        self.record_pause(started, freed_before);
        self.verify()
    }

    /// A safepoint between operations: collects when the runtime's allocation threshold is due and checks the
    /// collection. Returns whether a collection ran.
    pub fn safepoint(&mut self) -> Result<bool, String> {
        let freed_before = self.rt.heap_stats().freed_total;
        let started = Instant::now();
        if self.rt.maybe_collect() {
            self.record_pause(started, freed_before);
            self.automatic += 1;
            self.verify()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Builds the stable live set: `n` objects in a ring (each with `id` and `next`), a Map from number to object, and a
/// closure over the first object. Pinned for the whole run.
pub fn stable(c: &mut Churn<'_>, n: usize) -> Res<Stable> {
    let mut objs = Vec::with_capacity(n);
    for k in 0..n {
        let o = c.object();
        c.set_num(o, "id", k as f64)?;
        objs.push(o);
    }
    for k in 0..n {
        c.set_ref(objs[k], "next", objs[(k + 1) % n])?;
    }
    let map = c.map_numbers(&objs);
    let probe = c.closure(objs[0]);
    let mut elems = objs.clone();
    elems.push(map);
    elems.push(probe);
    let arr = c.array(&elems);
    let root = c.pin(arr);
    Ok(Stable { root, arr, map, probe, n })
}

/// Reads the stable set back and compares it with the ring, the Map and the closure it was built from.
pub fn check_stable(c: &Churn<'_>, st: &Stable) -> Result<(), String> {
    let rt = c.rt;
    let arr = c.value(st.arr);
    let mut objs = Vec::with_capacity(st.n);
    for k in 0..st.n {
        objs.push(js(rt, rt.get_v(&arr, &PropKey::Index(k as u32)))?);
    }
    for k in 0..st.n {
        let id = js(rt, rt.get_str(&objs[k], "id"))?;
        if id != Value::Number(k as f64) {
            return Err(format!("stable object {k} has id {id:?}"));
        }
        let next = js(rt, rt.get_str(&objs[k], "next"))?;
        if next != objs[(k + 1) % st.n] {
            return Err(format!("stable object {k} does not link to its successor"));
        }
        let mapped = js(rt, get_from_map(rt, &c.value(st.map), Value::Number(k as f64)))?;
        if mapped != objs[k] {
            return Err(format!("stable map entry {k} is not stable object {k}"));
        }
    }
    let probed = js(rt, rt.call(&c.value(st.probe), &Value::Undefined, &[]))?;
    if probed != objs[0] {
        return Err("stable closure no longer returns its captured object".into());
    }
    Ok(())
}

fn get_from_map(rt: &Rt, map: &Value, key: Value) -> Res<Value> {
    let get = rt.get_str(map, "get")?;
    rt.call(&get, map, &[key])
}

/// Walks a list of `len` objects from `head`: each `id` is its position, and the tail links to nothing.
pub fn check_list(c: &Churn<'_>, head: usize, len: usize) -> Result<(), String> {
    let rt = c.rt;
    let mut cur = c.value(head);
    for k in 0..len {
        let id = js(rt, rt.get_str(&cur, "id"))?;
        if id != Value::Number(k as f64) {
            return Err(format!("list position {k} has id {id:?}"));
        }
        cur = js(rt, rt.get_str(&cur, "next"))?;
    }
    if cur != Value::Undefined {
        return Err(format!("list continues past {len} nodes"));
    }
    Ok(())
}

/// Walks each cycle once around and checks that it closes on its first object.
pub fn check_rings(c: &Churn<'_>, heads: &[usize], size: usize) -> Result<(), String> {
    let rt = c.rt;
    for &h in heads {
        let first = c.value(h);
        let mut cur = first.clone();
        for k in 0..size {
            let id = js(rt, rt.get_str(&cur, "id"))?;
            if id != Value::Number(k as f64) {
                return Err(format!("cycle position {k} has id {id:?}"));
            }
            cur = js(rt, rt.get_str(&cur, "next"))?;
        }
        if cur != first {
            return Err("cycle does not close on its first object".into());
        }
    }
    Ok(())
}

pub fn check_wide(c: &Churn<'_>, n: usize, width: usize) -> Result<(), String> {
    let rt = c.rt;
    let v = c.value(n);
    for k in 0..width {
        let p = js(rt, rt.get_str(&v, &format!("p{k}")))?;
        if p != Value::Number(k as f64) {
            return Err(format!("wide property p{k} is {p:?}"));
        }
    }
    let child = js(rt, rt.get_str(&v, "child"))?;
    if !child.is_object() {
        return Err("wide object lost its child".into());
    }
    Ok(())
}

pub fn check_map(c: &Churn<'_>, m: usize, kv: &[(usize, usize)]) -> Result<(), String> {
    let rt = c.rt;
    let mv = c.value(m);
    let size = js(rt, rt.get_str(&mv, "size"))?;
    if size != Value::Number(kv.len() as f64) {
        return Err(format!("map size is {size:?}, expected {}", kv.len()));
    }
    for &(k, v) in kv {
        let got = js(rt, get_from_map(rt, &mv, c.value(k)))?;
        if got != c.value(v) {
            return Err("map entry does not map its key to its value".into());
        }
    }
    Ok(())
}

pub fn check_set(c: &Churn<'_>, s: usize, members: &[usize]) -> Result<(), String> {
    let rt = c.rt;
    let sv = c.value(s);
    let size = js(rt, rt.get_str(&sv, "size"))?;
    if size != Value::Number(members.len() as f64) {
        return Err(format!("set size is {size:?}, expected {}", members.len()));
    }
    let has = js(rt, rt.get_str(&sv, "has"))?;
    for &m in members {
        let found = js(rt, rt.call(&has, &sv, &[c.value(m)]))?;
        if found != Value::Bool(true) {
            return Err("set lost a member".into());
        }
    }
    Ok(())
}

/// Calls each closure: it must return the object it captured.
pub fn check_closures(c: &Churn<'_>, fns: &[usize], objs: &[usize]) -> Result<(), String> {
    let rt = c.rt;
    for (&f, &o) in fns.iter().zip(objs) {
        let got = js(rt, rt.call(&c.value(f), &Value::Undefined, &[]))?;
        if got != c.value(o) {
            return Err("closure does not return the object it captured".into());
        }
    }
    Ok(())
}

pub fn check_natives(c: &Churn<'_>, fns: &[usize], objs: &[usize]) -> Result<(), String> {
    let rt = c.rt;
    for (&f, &o) in fns.iter().zip(objs) {
        let got = js(rt, rt.call(&c.value(f), &Value::Undefined, &[]))?;
        if got != c.value(o) {
            return Err("native function does not return its data".into());
        }
    }
    Ok(())
}

/// Each derived promise is fulfilled with the value its handler returned: the argument itself (pass-through) or a
/// fresh `{ from: argument }` object.
pub fn check_promises(c: &Churn<'_>, results: &[(usize, usize, bool)]) -> Result<(), String> {
    let rt = c.rt;
    for &(q, v, wrap) in results {
        let Some(PromiseState::Fulfilled(r)) = promise::promise_state(rt, &c.value(q)) else {
            return Err("derived promise is not fulfilled".into());
        };
        if wrap {
            let from = js(rt, rt.get_str(&r, "from"))?;
            if from != c.value(v) || r == c.value(v) {
                return Err("wrapped result does not reference its argument".into());
            }
        } else if r != c.value(v) {
            return Err("pass-through result is not the argument".into());
        }
    }
    Ok(())
}

/// One churn round. A holder array (pinned while the round runs) carries each structure to its safepoint and its check,
/// and each structure is dropped right after its check, so every collection finds garbage from the structure before it.
/// The list head is pinned across the round boundary: the previous round's pin (`carry`) is released first. The
/// closure array is dropped as well, and the holder is unpinned at the end. Every collection is checked against the model.
pub fn round(c: &mut Churn<'_>, s: &Shape, carry: &mut Option<RootId>) -> Result<RoundStats, String> {
    let rt = c.rt;
    let created0 = c.created();
    let auto0 = c.automatic();

    let holder = c.array(&[]);
    let hold = c.pin(holder);

    if let Some(old) = carry.take() {
        c.unpin(old);
    }
    let head = js(rt, c.list(s.list))?;
    let slot = js(rt, c.attach(holder, head))?;
    *carry = Some(c.pin(head));
    c.safepoint()?;
    check_list(c, head, s.list)?;
    js(rt, c.detach(holder, slot, head))?;

    let heads = js(rt, c.rings(s.rings, s.ring_size))?;
    let ring_arr = c.array(&heads);
    let slot = js(rt, c.attach(holder, ring_arr))?;
    c.safepoint()?;
    check_rings(c, &heads, s.ring_size)?;
    js(rt, c.detach(holder, slot, ring_arr))?;

    let w = js(rt, c.wide(s.width))?;
    let slot = js(rt, c.attach(holder, w))?;
    c.safepoint()?;
    check_wide(c, w, s.width)?;
    js(rt, c.detach(holder, slot, w))?;

    let (m, kv) = js(rt, c.map(s.pairs))?;
    let slot = js(rt, c.attach(holder, m))?;
    c.safepoint()?;
    check_map(c, m, &kv)?;
    js(rt, c.detach(holder, slot, m))?;

    let (set, members) = js(rt, c.set(s.pairs))?;
    let slot = js(rt, c.attach(holder, set))?;
    c.safepoint()?;
    check_set(c, set, &members)?;
    js(rt, c.detach(holder, slot, set))?;

    let mut objs = Vec::with_capacity(s.closures);
    let mut fns = Vec::with_capacity(s.closures);
    for k in 0..s.closures {
        let o = c.object();
        js(rt, c.set_num(o, "id", k as f64))?;
        objs.push(o);
        fns.push(c.closure(o));
    }
    let closures = c.array(&fns);
    let slot = js(rt, c.attach(holder, closures))?;
    c.safepoint()?;
    check_closures(c, &fns, &objs)?;
    js(rt, c.detach(holder, slot, closures))?;

    let mut nobjs = Vec::with_capacity(s.natives);
    let mut nats = Vec::with_capacity(s.natives);
    for k in 0..s.natives {
        let o = c.object();
        js(rt, c.set_num(o, "id", k as f64))?;
        nobjs.push(o);
        nats.push(c.native(o));
    }
    let natives = c.array(&nats);
    let slot = js(rt, c.attach(holder, natives))?;
    c.safepoint()?;
    check_natives(c, &nats, &nobjs)?;
    js(rt, c.detach(holder, slot, natives))?;

    let mut results = Vec::with_capacity(s.promises);
    for k in 0..s.promises {
        let v = c.object();
        js(rt, c.set_num(v, "id", k as f64))?;
        let p = js(rt, c.fulfilled(v))?;
        let wrap = k % 2 == 0;
        let h = c.handler(wrap);
        let q = js(rt, c.then(p, h))?;
        results.push((q, v, wrap));
    }
    let ran = c.drain();
    if ran != s.promises {
        return Err(format!("{} reaction jobs were expected, {ran} ran", s.promises));
    }
    let mut qs = Vec::with_capacity(s.promises);
    for &(q, _, _) in &results {
        js(rt, c.settle(q))?;
        qs.push(q);
    }
    let promises = c.array(&qs);
    let slot = js(rt, c.attach(holder, promises))?;
    c.safepoint()?;
    check_promises(c, &results)?;
    js(rt, c.detach(holder, slot, promises))?;

    let x = c.object();
    js(rt, c.set_num(x, "id", -1.0))?;
    c.enqueue_mark(x);
    c.collect()?;
    let ran = c.drain();
    if ran != 1 {
        return Err(format!("one queued job was expected, {ran} ran"));
    }
    if js(rt, rt.get_str(&c.value(x), "ran"))? != Value::Bool(true) {
        return Err("queued job did not run on its argument".into());
    }
    c.collect()?;

    c.unpin(hold);
    c.collect()?;

    Ok(RoundStats { created: c.created() - created0, automatic: c.automatic() - auto0 })
}
