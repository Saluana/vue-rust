//! Heap exhaustion during churn (WP-5.6). Exceeding `set_heap_limit` surfaces as a catchable JavaScript RangeError from
//! generated-style calls, never as a Rust panic, and the runtime is usable again once the refused garbage is collected.
//! The function bodies below follow ABI.md section 0: `rt.tick()?` on function entry and on every loop back-edge, and
//! calls through `rt.call`. `try_code` is the `try { return f() } catch (e) { return e }` that generated code wraps
//! around a call. No JavaScript source is run; the generated code is written by hand against the kernel ABI.

#[path = "support/churn.rs"]
mod churn;

use churn::{Churn, Shape};
use vn_value::{Call, Flags, PropKey, Res, Rt, Throw, Value};

/// Objects stored per call of `block_code`.
const BLOCK: usize = 64;

/// Estimated bytes that one stored object adds (ABI.md section 12): 96 for the object, 48 for its one property and 24
/// for its slot in the holder array. The heap limit is compared with this estimate.
const BYTES_PER_STORED: usize = 96 + 48 + 24;

const SHAPE: Shape = Shape {
    list: 120,
    rings: 6,
    ring_size: 8,
    width: 16,
    pairs: 12,
    closures: 16,
    natives: 16,
    promises: 8,
};

fn length_of(rt: &Rt, arr: &Value) -> usize {
    match rt.get_str(arr, "length") {
        Ok(Value::Number(n)) => n as usize,
        other => panic!("array length: {other:?}"),
    }
}

/// Stores `BLOCK` objects in the holder (`data[0]`), with a tick on entry and on every loop back-edge.
fn block_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    rt.tick()?;
    let holder = rt.fn_data(c.callee)[0].clone();
    for i in 0..BLOCK {
        rt.tick()?;
        let o = rt.new_object();
        rt.define_data(o, PropKey::from_str("i"), Value::Number(i as f64), Flags::ALL)?;
        let slot = length_of(rt, &holder) as u32;
        rt.set_v_strict(&holder, &PropKey::Index(slot), Value::Object(o))?;
    }
    Ok(Value::Undefined)
}

/// Calls `block` (`data[0]`) `count` times (`data[1]`), with a tick on each back-edge.
fn churn_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    rt.tick()?;
    let data = rt.fn_data(c.callee);
    let block = data[0].clone();
    let count = match data[1] {
        Value::Number(n) => n as usize,
        _ => 0,
    };
    for _ in 0..count {
        rt.tick()?;
        rt.call(&block, &Value::Undefined, &[])?;
    }
    Ok(Value::Undefined)
}

/// `try { return f() } catch (e) { return e }`: a thrown value comes back as the result.
fn try_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    match rt.call(&c.arg(0), &Value::Undefined, &[]) {
        Ok(_) => Ok(Value::Undefined),
        Err(Throw(e)) => Ok(e),
    }
}

fn text(rt: &Rt, v: &Value, name: &str) -> String {
    match rt.get_str(v, name) {
        Ok(Value::String(s)) => s.to_rust_string_lossy(),
        other => panic!("property {name}: {other:?}"),
    }
}

/// A holder array (rooted by the caller), the block and churn functions over it, and the try wrapper.
struct Program {
    holder: vn_value::ObjRef,
    churn: Value,
    try_fn: Value,
}

fn program(rt: &Rt, count: usize) -> Program {
    let holder = rt.new_array(vec![]);
    let block = rt.new_native_function(block_code, "block", 0, vec![Value::Object(holder)]);
    let churn = rt.new_native_function(churn_code, "churn", 0, vec![Value::Object(block), Value::Number(count as f64)]);
    let try_fn = rt.new_native_function(try_code, "try", 1, vec![]);
    Program { holder, churn: Value::Object(churn), try_fn: Value::Object(try_fn) }
}

#[test]
fn exceeding_the_heap_limit_during_churn_is_a_catchable_range_error() {
    let rt = Rt::new();
    let mut c = Churn::new(&rt);
    let st = churn::stable(&mut c, 200).expect("stable set");
    let p = program(&rt, 10_000);
    let hid = rt.root(&Value::Object(p.holder));
    let before = rt.heap_stats().bytes_estimate;
    let limit = before + 64 * 1024;
    rt.set_heap_limit(limit);

    let caught = rt.call(&p.try_fn, &Value::Undefined, std::slice::from_ref(&p.churn)).expect("try does not throw");
    assert_eq!(text(&rt, &caught, "name"), "RangeError");
    assert_eq!(text(&rt, &caught, "message"), "heap limit exceeded");

    // The refusal is a tick inside the loop: the first tick that sees more than `limit` bytes stops the churn, after
    // exactly (limit - before) / BYTES_PER_STORED + 1 objects were stored.
    let stored = length_of(&rt, &Value::Object(p.holder));
    assert_eq!(stored, (limit - before) / BYTES_PER_STORED + 1, "objects stored before the refusal");

    // The refused garbage: once the holder is dropped and collected, the model and the runtime agree again.
    rt.unroot(hid);
    c.collect().expect("the model agrees after the refused churn is collected");
    assert!(rt.heap_stats().bytes_estimate < limit, "the collection brought the estimate under the limit");
    rt.tick().expect("ticks succeed again below the limit");

    // The runtime is still usable under the same limit: a smaller churn runs to completion.
    let q = program(&rt, 2);
    let qid = rt.root(&Value::Object(q.holder));
    let r = rt.call(&q.try_fn, &Value::Undefined, std::slice::from_ref(&q.churn)).expect("try does not throw");
    assert_eq!(r, Value::Undefined, "the second churn finished without a throw");
    assert_eq!(length_of(&rt, &Value::Object(q.holder)), 2 * BLOCK);
    rt.unroot(qid);
    c.collect().expect("the model agrees after the second churn");
    churn::check_stable(&c, &st).expect("the stable set is intact after the refusal and the recovery");

    rt.clear_heap_limit();
    let mut carry = None;
    churn::round(&mut c, &SHAPE, &mut carry).expect("a full churn round verifies after the recovery");
}

#[test]
fn garbage_stays_counted_against_the_limit_until_it_is_collected() {
    let rt = Rt::new();
    rt.collect();
    let p = program(&rt, 10_000);
    let hid = rt.root(&Value::Object(p.holder));
    let before = rt.heap_stats().bytes_estimate;
    let limit = before + 64 * 1024;
    rt.set_heap_limit(limit);
    let caught = rt.call(&p.try_fn, &Value::Undefined, std::slice::from_ref(&p.churn)).expect("try does not throw");
    assert_eq!(text(&rt, &caught, "message"), "heap limit exceeded");

    // Dropping the root does not lower the estimate: the refused objects are still allocated.
    rt.unroot(hid);
    assert!(rt.tick().is_err(), "unrooted but uncollected garbage still counts against the limit");
    rt.collect();
    rt.tick().expect("after the collection the estimate is below the limit");
}

#[test]
fn a_zero_limit_refuses_the_first_tick_and_stores_nothing() {
    let rt = Rt::new();
    let p = program(&rt, 10_000);
    rt.set_heap_limit(0);
    let caught = rt.call(&p.try_fn, &Value::Undefined, std::slice::from_ref(&p.churn)).expect("try does not throw");
    assert_eq!(text(&rt, &caught, "name"), "RangeError");
    assert_eq!(text(&rt, &caught, "message"), "heap limit exceeded");
    assert_eq!(length_of(&rt, &Value::Object(p.holder)), 0, "the refusal came before any object was stored");
}
