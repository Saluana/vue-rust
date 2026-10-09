//! Reactive arrays: index and length tracking, the mutating methods, identity-sensitive search, method identity, Array
//! predicates and iteration. Each test is the Rust translation of one snippet in tests/vue/rust/reactive/snippets.mjs.

#[macro_use]
mod r1b_support;
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use common::{js, Env};
use r1b_support::{
    array, bo, check, field_is, field_mod_is, flag, func, iterate, iterate_rt, method, num, object, run_effect, st, sv, undef,
};
use vn_value::{strict_eq, Intrinsic, Value};

/// `r1b-array-index-and-length-tracking`
#[test]
fn array_index_and_length_tracking() {
    let e = Env::new();
    let arr = e.invoke("reactive", &[array(&e, vec![num(1.0), num(2.0), num(3.0)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![arr.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let v = rt.get_str(&d[0], "1")?;
        let len = rt.get_str(&d[0], "length")?;
        log.borrow_mut().push(format!("idx {} {}", js(rt, &v), js(rt, &len)));
        Ok(undef())
    });
    e.set(&arr, "1", num(5.0));
    log_line!(e, "runs", runs.get());
    e.set(&arr, "5", num(9.0));
    log_line!(e, "runs-grow", runs.get(), sv(&e, &e.get(&arr, "length")));
    e.set(&arr, "length", num(2.0));
    log_line!(e, "runs-shrink", runs.get(), sv(&e, &e.get(&arr, "length")));
    check(&e, "r1b-array-index-and-length-tracking");
}

/// `r1b-array-push-effects`
#[test]
fn array_push_effects() {
    let e = Env::new();
    let arr = e.invoke("reactive", &[array(&e, vec![])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![arr.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let len = rt.get_str(&d[0], "length")?;
        log.borrow_mut().push(format!("len {}", js(rt, &len)));
        Ok(undef())
    });
    method(&e, &arr, "push", &[num(1.0)]);
    method(&e, &arr, "push", &[num(2.0), num(3.0)]);
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&arr, "length")));
    check(&e, "r1b-array-push-effects");
}

/// `r1b-array-pop-shift-unshift`
#[test]
fn array_pop_shift_unshift() {
    let e = Env::new();
    let arr = e.invoke("reactive", &[array(&e, vec![num(1.0), num(2.0), num(3.0)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![arr.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let first = rt.get_str(&d[0], "0")?;
        let len = rt.get_str(&d[0], "length")?;
        log.borrow_mut().push(format!("first {} {}", js(rt, &first), js(rt, &len)));
        Ok(undef())
    });
    let popped = method(&e, &arr, "pop", &[]);
    log_line!(e, "pop", sv(&e, &popped), runs.get());
    let shifted = method(&e, &arr, "shift", &[]);
    log_line!(e, "shift", sv(&e, &shifted), runs.get());
    method(&e, &arr, "unshift", &[num(7.0), num(8.0)]);
    let joined = method(&e, &arr, "join", &[st("-")]);
    log_line!(e, "unshift", runs.get(), sv(&e, &joined));
    check(&e, "r1b-array-pop-shift-unshift");
}

/// `r1b-array-splice-effect`
#[test]
fn array_splice_effect() {
    let e = Env::new();
    let arr = e.invoke("reactive", &[array(&e, vec![st("a"), st("b"), st("c"), st("d")])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![arr.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let len = rt.get_str(&d[0], "length")?;
        let one = rt.get_str(&d[0], "1")?;
        log.borrow_mut().push(format!("sp {} {}", js(rt, &len), js(rt, &one)));
        Ok(undef())
    });
    let removed = method(&e, &arr, "splice", &[num(1.0), num(2.0), st("x")]);
    let removed_len = sv(&e, &e.get(&removed, "length"));
    let removed_0 = sv(&e, &e.get(&removed, "0"));
    log_line!(e, "removed", removed_len, removed_0, sv(&e, &e.get(&arr, "length")), runs.get());
    check(&e, "r1b-array-splice-effect");
}

/// `r1b-array-sort-reverse-effect`
#[test]
fn array_sort_reverse_effect() {
    let e = Env::new();
    let arr = e.invoke("reactive", &[array(&e, vec![num(3.0), num(1.0), num(2.0)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![arr.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let first = rt.get_str(&d[0], "0")?;
        log.borrow_mut().push(format!("s {}", js(rt, &first)));
        Ok(undef())
    });
    method(&e, &arr, "sort", &[]);
    log_line!(e, "sorted-runs", runs.get(), sv(&e, &e.get(&arr, "0")));
    method(&e, &arr, "reverse", &[]);
    log_line!(e, "rev-runs", runs.get(), sv(&e, &e.get(&arr, "0")));
    check(&e, "r1b-array-sort-reverse-effect");
}

/// `r1b-array-includes-identity`
#[test]
fn array_includes_identity() {
    let e = Env::new();
    let obj = object(&e, &[]);
    let arr = e.invoke("reactive", &[array(&e, vec![obj.clone(), num(1.0)])]);
    let found = |v: &Value| (method(&e, &arr, "includes", &[v.clone()]), method(&e, &arr, "indexOf", &[v.clone()]), method(&e, &arr, "lastIndexOf", &[v.clone()]));
    let (i1, i2, i3) = found(&obj);
    log_line!(e, "incl-raw", sv(&e, &i1), sv(&e, &i2), sv(&e, &i3));
    let po = e.invoke("reactive", &[obj.clone()]);
    let (p1, p2, p3) = found(&po);
    log_line!(e, "incl-proxy", sv(&e, &p1), sv(&e, &p2), sv(&e, &p3));
    let nan_arr = e.invoke("reactive", &[array(&e, vec![num(f64::NAN)])]);
    let nan_inc = method(&e, &nan_arr, "includes", &[num(f64::NAN)]);
    let nan_idx = method(&e, &nan_arr, "indexOf", &[num(f64::NAN)]);
    log_line!(e, "nan", sv(&e, &nan_inc), sv(&e, &nan_idx));
    let raw_arr = e.invoke("toRaw", &[arr.clone()]);
    let same = method(&e, &arr, "includes", &[raw_arr]);
    log_line!(e, "same-proxy", sv(&e, &same));
    check(&e, "r1b-array-includes-identity");
}

/// `r1b-array-method-identity`
#[test]
fn array_method_identity() {
    let e = Env::new();
    let arr = e.invoke("reactive", &[array(&e, vec![num(1.0)])]);
    let push_a = e.get(&arr, "push");
    let push_b = e.get(&arr, "push");
    let inc_a = e.get(&arr, "includes");
    let inc_b = e.get(&arr, "includes");
    let other = e.invoke("reactive", &[array(&e, vec![num(2.0)])]);
    let fe_other = e.get(&other, "forEach");
    let fe = e.get(&arr, "forEach");
    log_line!(e, "identity", bo(strict_eq(&push_a, &push_b)), bo(strict_eq(&inc_a, &inc_b)), bo(strict_eq(&fe, &fe_other)));
    let has_own = e.get(&arr, "hasOwnProperty");
    let has_own_again = e.get(&arr, "hasOwnProperty");
    let h0 = method(&e, &arr, "hasOwnProperty", &[st("0")]);
    let hlen = method(&e, &arr, "hasOwnProperty", &[st("length")]);
    log_line!(e, "own-hasOwn", bo(strict_eq(&has_own, &has_own_again)), sv(&e, &h0), sv(&e, &hlen));
    check(&e, "r1b-array-method-identity");
}

/// `r1b-array-isarray-and-tostring`
#[test]
fn array_isarray_and_tostring() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![num(1.0), num(2.0)])]);
    let object_proto = Value::Object(e.rt.intrinsic(Intrinsic::ObjectPrototype));
    let to_string = e.get(&object_proto, "toString");
    let tag = e.call(&to_string, &a, &[]);
    log_line!(e, "isArray", bo(e.rt.is_array(&a)), sv(&e, &e.get(&a, "length")), flag(&e, "isReactive", &a), sv(&e, &tag));
    let ro = e.invoke("readonly", &[array(&e, vec![])]);
    let ro_arr = e.invoke("readonly", &[array(&e, vec![num(1.0), num(2.0)])]);
    log_line!(e, "ro", bo(e.rt.is_array(&ro)), sv(&e, &ro_arr));
    check(&e, "r1b-array-isarray-and-tostring");
}

/// `r1b-array-iteration-wraps-items`
#[test]
fn array_iteration_wraps_items() {
    let e = Env::new();
    let item = object(&e, &[("v", num(1.0))]);
    let a = e.invoke("reactive", &[array(&e, vec![item, num(2.0)])]);
    let kinds: Vec<String> = iterate(&e, &a)
        .iter()
        .map(|x| if flag(&e, "isReactive", x) == "true" { "r".to_string() } else { "number".to_string() })
        .collect();
    log_line!(e, "iter", kinds.join(","));
    let entries = method(&e, &a, "entries", &[]);
    for (i, pair) in iterate(&e, &entries).iter().enumerate() {
        let x = e.get(pair, "1");
        log_line!(e, "entry", i, flag(&e, "isReactive", &x));
    }
    let s: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let sink = s.clone();
    let cb = func(&e, "", 3, vec![a.clone()], move |rt, _this, args, d| {
        let item_is_reactive = rt.get_str(&args[0], "__v_isReactive")?.to_boolean();
        let same = strict_eq(&args[2], &d[0]);
        sink.borrow_mut().push_str(&format!("{item_is_reactive}:{same};"));
        Ok(undef())
    });
    method(&e, &a, "forEach", &[cb]);
    log_line!(e, "forEach", s.borrow().clone());
    check(&e, "r1b-array-iteration-wraps-items");
}

/// `r1b-arr-includes-from-index`
#[test]
fn arr_includes_from_index() {
    let e = Env::new();
    let o = object(&e, &[]);
    let arr = e.invoke("reactive", &[array(&e, vec![o.clone(), st("x"), o.clone()])]);
    let inc = method(&e, &arr, "includes", &[o.clone(), num(1.0)]);
    let idx = method(&e, &arr, "indexOf", &[o.clone(), num(1.0)]);
    let last = method(&e, &arr, "lastIndexOf", &[o.clone(), num(1.0)]);
    let neg = method(&e, &arr, "indexOf", &[o.clone(), num(-1.0)]);
    log_line!(e, "from", sv(&e, &inc), sv(&e, &idx), sv(&e, &last), sv(&e, &neg));
    let po = e.invoke("reactive", &[o.clone()]);
    let missing_inc = method(&e, &arr, "includes", &[po.clone(), num(2.0)]);
    let missing_idx = method(&e, &arr, "indexOf", &[po.clone()]);
    let missing_last = method(&e, &arr, "lastIndexOf", &[po.clone()]);
    log_line!(e, "missing", sv(&e, &missing_inc), sv(&e, &missing_idx), sv(&e, &missing_last));
    check(&e, "r1b-arr-includes-from-index");
}

/// `r1b-arr-concat-spreads-wrapped`
#[test]
fn arr_concat_spreads_wrapped() {
    let e = Env::new();
    let item = object(&e, &[("n", num(1.0))]);
    let a = e.invoke("reactive", &[array(&e, vec![item])]);
    let three = e.invoke("reactive", &[array(&e, vec![num(3.0)])]);
    let b = method(&e, &a, "concat", &[array(&e, vec![num(2.0)]), three]);
    let first = e.get(&b, "0");
    log_line!(
        e,
        "concat",
        sv(&e, &e.get(&b, "length")),
        bo(e.rt.is_array(&b)),
        flag(&e, "isReactive", &b),
        flag(&e, "isReactive", &first),
        sv(&e, &e.get(&b, "1")),
        sv(&e, &e.get(&b, "2"))
    );
    check(&e, "r1b-arr-concat-spreads-wrapped");
}

/// `r1b-arr-join-and-tostring-wrapped`
#[test]
fn arr_join_and_tostring_wrapped() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![num(1.0), array(&e, vec![num(2.0), num(3.0)]), st("x")])]);
    let joined = method(&e, &a, "join", &[st("-")]);
    let as_string = sv(&e, &a);
    let proto_to_string = e.get(&Value::Object(e.rt.intrinsic(Intrinsic::ArrayPrototype)), "toString");
    log_line!(e, "join", sv(&e, &joined), as_string, bo(strict_eq(&e.get(&a, "toString"), &proto_to_string)));
    check(&e, "r1b-arr-join-and-tostring-wrapped");
}

/// `r1b-arr-copies-tracking-iterate`
#[test]
fn arr_copies_tracking_iterate() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![num(3.0), num(1.0), num(2.0)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let sorted = rt.call(&rt.get_str(&d[0], "toSorted")?, &d[0], &[])?;
        let joined = rt.call(&rt.get_str(&sorted, "join")?, &sorted, &[st(",")])?;
        log.borrow_mut().push(format!("sorted {}", js(rt, &joined)));
        Ok(undef())
    });
    let rev = method(&e, &a, "toReversed", &[]);
    let rev_joined = method(&e, &rev, "join", &[st(",")]);
    let a_joined = method(&e, &a, "join", &[st(",")]);
    log_line!(e, "rev", sv(&e, &rev_joined), sv(&e, &a_joined), runs.get());
    let spliced = method(&e, &a, "toSpliced", &[num(1.0), num(1.0), st("z")]);
    let spliced_joined = method(&e, &spliced, "join", &[st(",")]);
    log_line!(e, "spliced", sv(&e, &spliced_joined), sv(&e, &e.get(&a, "length")), runs.get());
    method(&e, &a, "push", &[num(4.0)]);
    log_line!(e, "after-push", runs.get());
    check(&e, "r1b-arr-copies-tracking-iterate");
}

/// `r1b-arr-find-family-wraps`
#[test]
fn arr_find_family_wraps() {
    let e = Env::new();
    let o = object(&e, &[("v", num(1.0))]);
    let o2 = object(&e, &[("v", num(2.0))]);
    let a = e.invoke("reactive", &[array(&e, vec![o, o2])]);
    let found = method(&e, &a, "find", &[field_is(&e, "v", 2.0)]);
    let found_v = sv(&e, &e.get(&found, "v"));
    let found_is_reactive = flag(&e, "isReactive", &found);
    let index = method(&e, &a, "findIndex", &[field_is(&e, "v", 2.0)]);
    let last = method(&e, &a, "findLast", &[field_is(&e, "v", 1.0)]);
    let first = e.get(&a, "0");
    let last_same = bo(strict_eq(&last, &first));
    let last_idx = method(&e, &a, "findLastIndex", &[field_is(&e, "v", 9.0)]);
    log_line!(e, "find", found_is_reactive, found_v, sv(&e, &index), last_same, sv(&e, &last_idx));
    check(&e, "r1b-arr-find-family-wraps");
}

/// `r1b-arr-filter-map-wrap-results`
#[test]
fn arr_filter_map_wrap_results() {
    let e = Env::new();
    let items: Vec<Value> = (1..=3).map(|k| object(&e, &[("k", num(k as f64))])).collect();
    let a = e.invoke("reactive", &[array(&e, items)]);
    let odd = method(&e, &a, "filter", &[field_mod_is(&e, "k", 2.0, 1.0)]);
    let odd_first = e.get(&odd, "0");
    let odd_second = e.get(&odd, "1");
    log_line!(e, "filter", sv(&e, &e.get(&odd, "length")), flag(&e, "isReactive", &odd_first), sv(&e, &e.get(&odd_second, "k")));
    let mapper = func(&e, "", 1, vec![], |rt, _this, args, _d| {
        let r = rt.get_str(&args[0], "__v_isReactive")?.to_boolean();
        let k = rt.get_str(&args[0], "k")?;
        Ok(st(&format!("{}:{}", r, js(rt, &k))))
    });
    let ks = method(&e, &a, "map", &[mapper]);
    let joined = method(&e, &ks, "join", &[st(" ")]);
    log_line!(e, "map", sv(&e, &joined));
    check(&e, "r1b-arr-filter-map-wrap-results");
}

/// `r1b-arr-some-every-callback-args`
#[test]
fn arr_some_every_callback_args() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![num(1.0), num(2.0), num(3.0)])]);
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    let some_cb = func(&e, "", 3, vec![a.clone()], move |rt, _this, args, d| {
        sink.borrow_mut().push(js(rt, &args[1]));
        sink.borrow_mut().push(strict_eq(&args[2], &d[0]).to_string());
        Ok(Value::Bool(matches!(args[0], Value::Number(n) if n == 2.0)))
    });
    let r1 = method(&e, &a, "some", &[some_cb]);
    let all_positive = func(&e, "", 1, vec![], |_rt, _this, args, _d| Ok(Value::Bool(matches!(args[0], Value::Number(n) if n > 0.0))));
    let r2 = method(&e, &a, "every", &[all_positive]);
    log_line!(e, "some", sv(&e, &r1), sv(&e, &r2), seen.borrow().join(","));
    check(&e, "r1b-arr-some-every-callback-args");
}

/// `r1b-arr-reduce-accumulator-wrap`
#[test]
fn arr_reduce_accumulator_wrap() {
    let e = Env::new();
    let items: Vec<Value> = [1.0, 2.0].iter().map(|n| object(&e, &[("n", num(*n))])).collect();
    let a = e.invoke("reactive", &[array(&e, items)]);
    let add_n = func(&e, "", 2, vec![], |rt, _this, args, _d| {
        let acc = match args[0] {
            Value::Number(x) => x,
            _ => 0.0,
        };
        let n = match rt.get_str(&args[1], "n")? {
            Value::Number(x) => x,
            _ => 0.0,
        };
        Ok(num(acc + n))
    });
    let total = method(&e, &a, "reduce", &[add_n, num(0.0)]);
    let push_reactive = func(&e, "", 2, vec![], |rt, _this, args, _d| {
        let r = rt.get_str(&args[1], "__v_isReactive")?.to_boolean();
        let push = rt.get_str(&args[0], "push")?;
        rt.call(&push, &args[0], &[Value::Bool(r)])?;
        Ok(args[0].clone())
    });
    let first = method(&e, &a, "reduce", &[push_reactive, array(&e, vec![])]);
    let add_from_right = func(&e, "", 2, vec![], |rt, _this, args, _d| {
        let acc = match args[0] {
            Value::Number(x) => x,
            _ => 0.0,
        };
        let n = match rt.get_str(&args[1], "n")? {
            Value::Number(x) => x,
            _ => 0.0,
        };
        Ok(num(acc + n))
    });
    let rr = method(&e, &a, "reduceRight", &[add_from_right, num(10.0)]);
    let first_joined = method(&e, &first, "join", &[st(",")]);
    log_line!(e, "reduce", sv(&e, &total), sv(&e, &first_joined), sv(&e, &rr));
    check(&e, "r1b-arr-reduce-accumulator-wrap");
}

/// `r1b-arr-reduce-initial-accumulator-wrapped`: without an initial value the first element is the accumulator, wrapped.
#[test]
fn arr_reduce_initial_accumulator_wrapped() {
    let e = Env::new();
    let items: Vec<Value> = [1.0, 2.0].iter().map(|n| object(&e, &[("n", num(*n))])).collect();
    let a = e.invoke("reactive", &[array(&e, items)]);
    let log = e.log.clone();
    let step = func(&e, "", 2, vec![], move |rt, _this, args, _d| {
        let acc = rt.get_str(&args[0], "__v_isReactive")?.to_boolean();
        let x = rt.get_str(&args[1], "__v_isReactive")?.to_boolean();
        log.borrow_mut().push(format!("step {acc} {x}"));
        Ok(args[0].clone())
    });
    let got = method(&e, &a, "reduce", &[step]);
    log_line!(e, "result", flag(&e, "isReactive", &got), sv(&e, &e.get(&got, "n")));
    check(&e, "r1b-arr-reduce-initial-accumulator-wrapped");
}

/// `r1b-arr-reduce-empty-init-not-wrapped`
#[test]
fn arr_reduce_empty_init_not_wrapped() {
    let e = Env::new();
    let o = object(&e, &[("q", num(1.0))]);
    let empty = e.invoke("reactive", &[array(&e, vec![])]);
    let identity = func(&e, "", 1, vec![], |_rt, _this, args, _d| Ok(args[0].clone()));
    let got = method(&e, &empty, "reduce", &[identity, o]);
    log_line!(e, "empty-init", flag(&e, "isReactive", &got));
    let identity2 = func(&e, "", 1, vec![], |_rt, _this, args, _d| Ok(args[0].clone()));
    let msg = match e.rt.call(&e.get(&empty, "reduce"), &empty, &[identity2]) {
        Ok(_) => "none".to_string(),
        Err(t) => {
            let ctor = e.get(&t.0, "constructor");
            sv(&e, &e.get(&ctor, "name"))
        }
    };
    log_line!(e, "empty-noinit", msg);
    check(&e, "r1b-arr-reduce-empty-init-not-wrapped");
}

/// `r1b-arr-readonly-push-silent`
#[test]
fn arr_readonly_push_silent() {
    let e = Env::new();
    let raw = array(&e, vec![num(1.0)]);
    let r = e.invoke("readonly", &[raw.clone()]);
    let n = method(&e, &r, "push", &[num(2.0)]);
    log_line!(e, "ro-push", sv(&e, &n), sv(&e, &e.get(&raw, "length")), sv(&e, &e.get(&r, "length")), sv(&e, &e.get(&r, "1")));
    check(&e, "r1b-arr-readonly-push-silent");
}

/// `r1b-arr-readonly-pop-shift-silent`
#[test]
fn arr_readonly_pop_shift_silent() {
    let e = Env::new();
    let raw = array(&e, vec![num(5.0), num(6.0)]);
    let r = e.invoke("readonly", &[raw.clone()]);
    let popped = method(&e, &r, "pop", &[]);
    let shifted = method(&e, &r, "shift", &[]);
    log_line!(e, "ro-pop", sv(&e, &popped), sv(&e, &shifted), sv(&e, &e.get(&raw, "length")), sv(&e, &e.get(&r, "length")));
    check(&e, "r1b-arr-readonly-pop-shift-silent");
}

/// `r1b-arr-shallow-elements-not-wrapped`
#[test]
fn arr_shallow_elements_not_wrapped() {
    let e = Env::new();
    let inner = object(&e, &[("n", num(1.0))]);
    let a = e.invoke("shallowReactive", &[array(&e, vec![inner.clone()])]);
    let first = e.get(&a, "0");
    log_line!(e, "shallow-el", bo(strict_eq(&first, &inner)), flag(&e, "isReactive", &first), bo(e.rt.is_array(&a)));
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let el = rt.get_str(&d[0], "0")?;
        let n = rt.get_str(&el, "n")?;
        log.borrow_mut().push(format!("first {}", js(rt, &n)));
        Ok(undef())
    });
    e.set(&a, "0", object(&e, &[("n", num(2.0))]));
    log_line!(e, "runs", runs.get());
    check(&e, "r1b-arr-shallow-elements-not-wrapped");
}

/// `r1b-arr-delete-index-hole-tracking`
#[test]
fn arr_delete_index_hole_tracking() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![num(1.0), num(2.0)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let has = rt.has_property(d[0].as_object().expect("proxy"), &vn_value::PropKey::Index(0))?;
        let v = rt.get_str(&d[0], "0")?;
        log.borrow_mut().push(format!("has0 {has} {}", js(rt, &v)));
        Ok(undef())
    });
    let deleted = e.rt.delete(a.as_object().expect("proxy"), &vn_value::PropKey::Index(0)).expect("delete");
    let has_after = e.rt.has_property(a.as_object().expect("proxy"), &vn_value::PropKey::Index(0)).expect("has");
    log_line!(e, "after-delete", runs.get(), bo(has_after), sv(&e, &e.get(&a, "length")));
    let _ = deleted;
    e.set(&a, "0", num(9.0));
    log_line!(e, "after-set", runs.get());
    check(&e, "r1b-arr-delete-index-hole-tracking");
}

/// `r1b-arr-length-set-truncates-effects`
#[test]
fn arr_length_set_truncates_effects() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![num(1.0), num(2.0), num(3.0), num(4.0)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let tail = rt.get_str(&d[0], "3")?;
        let len = rt.get_str(&d[0], "length")?;
        log.borrow_mut().push(format!("tail {} {}", js(rt, &tail), js(rt, &len)));
        Ok(undef())
    });
    e.set(&a, "length", num(2.0));
    log_line!(e, "after", runs.get(), sv(&e, &e.get(&a, "length")), sv(&e, &e.get(&a, "2")));
    check(&e, "r1b-arr-length-set-truncates-effects");
}

/// `r1b-arr-sparse-holes-iteration`
#[test]
fn arr_sparse_holes_iteration() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![num(1.0), Value::Hole, num(3.0)])]);
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    let cb = func(&e, "", 2, vec![], move |rt, _this, args, _d| {
        sink.borrow_mut().push(format!("{}:{}", js(rt, &args[1]), js(rt, &args[0])));
        Ok(undef())
    });
    method(&e, &a, "forEach", &[cb]);
    log_line!(e, "holes", seen.borrow().join(","), bo(e.rt.has_property(a.as_object().expect("proxy"), &vn_value::PropKey::Index(1)).expect("has")), sv(&e, &e.get(&a, "length")));
    check(&e, "r1b-arr-sparse-holes-iteration");
}

/// `r1b-arr-index-string-keys`: `'00'` is not an index, so it is an ordinary key of the array.
#[test]
fn arr_index_string_keys() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![st("a")])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let zero = rt.get_str(&d[0], "0")?;
        let double = rt.get_str(&d[0], "00")?;
        log.borrow_mut().push(format!("k {} {}", js(rt, &zero), js(rt, &double)));
        Ok(undef())
    });
    e.set(&a, "0", st("b"));
    e.set(&a, "00", st("x"));
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&a, "00")));
    check(&e, "r1b-arr-index-string-keys");
}

/// `r1b-arr-ownkeys-tracks-length`: `Object.keys` of an array tracks its length.
#[test]
fn arr_ownkeys_tracks_length() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![num(1.0)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let object_ctor = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), "Object")?;
        let keys = rt.call(&rt.get_str(&object_ctor, "keys")?, &object_ctor, &[d[0].clone()])?;
        let joined = rt.call(&rt.get_str(&keys, "join")?, &keys, &[st(",")])?;
        log.borrow_mut().push(format!("keys {}", js(rt, &joined)));
        Ok(undef())
    });
    method(&e, &a, "push", &[num(2.0)]);
    log_line!(e, "runs", runs.get());
    e.set(&a, "length", num(1.0));
    log_line!(e, "runs2", runs.get());
    check(&e, "r1b-arr-ownkeys-tracks-length");
}

/// `r1b-arr-unshift-push-return-and-tracking`: `a[a.length - 1]` tracks the length and the index `length - 1`.
#[test]
fn arr_unshift_push_return_and_tracking() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let len = match rt.get_str(&d[0], "length")? {
            Value::Number(n) => n,
            _ => f64::NAN,
        };
        let last = rt.get_str(&d[0], &format!("{}", len - 1.0))?;
        log.borrow_mut().push(format!("last {}", js(rt, &last)));
        Ok(undef())
    });
    let pushed = method(&e, &a, "push", &[st("x")]);
    log_line!(e, "push-ret", sv(&e, &pushed), runs.get());
    let unshifted = method(&e, &a, "unshift", &[st("w")]);
    let joined = method(&e, &a, "join", &[st("")]);
    log_line!(e, "unshift-ret", sv(&e, &unshifted), sv(&e, &joined), runs.get());
    check(&e, "r1b-arr-unshift-push-return-and-tracking");
}

/// `r1b-arr-includes-tracks-after-mutation`
#[test]
fn arr_includes_tracks_after_mutation() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![num(1.0)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let found = rt.call(&rt.get_str(&d[0], "includes")?, &d[0], &[num(5.0)])?;
        log.borrow_mut().push(format!("has5 {}", js(rt, &found)));
        Ok(undef())
    });
    method(&e, &a, "push", &[num(5.0)]);
    log_line!(e, "runs", runs.get());
    e.set(&a, "0", num(5.0));
    log_line!(e, "runs2", runs.get());
    check(&e, "r1b-arr-includes-tracks-after-mutation");
}

/// `r1b-arr-nested-array-identity`
#[test]
fn arr_nested_array_identity() {
    let e = Env::new();
    let inner = array(&e, vec![num(1.0)]);
    let a = e.invoke("reactive", &[array(&e, vec![inner.clone()])]);
    let again = e.get(&a, "0");
    let proxy_of_inner = e.invoke("reactive", &[inner.clone()]);
    let via_reactive = e.get(&e.invoke("reactive", &[a.clone()]), "0");
    log_line!(e, "nested-id", bo(strict_eq(&e.get(&a, "0"), &again)), bo(strict_eq(&e.get(&a, "0"), &proxy_of_inner)), bo(strict_eq(&via_reactive, &again)));
    check(&e, "r1b-arr-nested-array-identity");
}

/// `r1b-arr-index-nan-and-string-search`
#[test]
fn arr_index_nan_and_string_search() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![st("1"), num(1.0), num(f64::NAN)])]);
    let idx_one = method(&e, &a, "indexOf", &[num(1.0)]);
    let idx_str = method(&e, &a, "indexOf", &[st("1")]);
    let inc_nan = method(&e, &a, "includes", &[num(f64::NAN)]);
    let last_nan = method(&e, &a, "lastIndexOf", &[num(f64::NAN)]);
    log_line!(e, "idx", sv(&e, &idx_one), sv(&e, &idx_str), sv(&e, &inc_nan), sv(&e, &last_nan));
    check(&e, "r1b-arr-index-nan-and-string-search");
}

/// `r1b-arr-spread-and-destructure`
#[test]
fn arr_spread_and_destructure() {
    let e = Env::new();
    let item = object(&e, &[("x", num(1.0))]);
    let a = e.invoke("reactive", &[array(&e, vec![item, num(2.0)])]);
    let spread = iterate(&e, &a);
    let first = spread[0].clone();
    let second = spread[1].clone();
    let copy = iterate(&e, &a);
    log_line!(e, "destructure", flag(&e, "isReactive", &first), sv(&e, &second), copy.len(), flag(&e, "isReactive", &copy[0]));
    check(&e, "r1b-arr-spread-and-destructure");
}

fn number_of(v: &Value) -> f64 {
    match v {
        Value::Number(n) => *n,
        _ => f64::NAN,
    }
}

/// `r1b-arr-computed-length`
#[test]
fn arr_computed_length() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![num(1.0)])]);
    let getter = func(&e, "", 0, vec![a.clone()], |rt, _this, _args, d| {
        let len = number_of(&rt.get_str(&d[0], "length")?);
        Ok(num(len * 10.0))
    });
    let n = e.invoke("computed", &[getter]);
    log_line!(e, "c1", sv(&e, &e.get(&n, "value")));
    method(&e, &a, "push", &[num(2.0)]);
    log_line!(e, "c2", sv(&e, &e.get(&n, "value")));
    check(&e, "r1b-arr-computed-length");
}

/// `r1b-arr-sort-proxy-elements`
#[test]
fn arr_sort_proxy_elements() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![object(&e, &[("v", num(2.0))]), object(&e, &[("v", num(1.0))])])]);
    let by_v = func(&e, "", 2, vec![], |rt, _this, args, _d| {
        let x = number_of(&rt.get_str(&args[0], "v")?);
        let y = number_of(&rt.get_str(&args[1], "v")?);
        Ok(num(x - y))
    });
    method(&e, &a, "sort", &[by_v]);
    let read_v = func(&e, "", 1, vec![], |rt, _this, args, _d| rt.get_str(&args[0], "v"));
    let values = method(&e, &a, "map", &[read_v]);
    let joined = method(&e, &values, "join", &[st(",")]);
    log_line!(e, "sorted", sv(&e, &joined), flag(&e, "isReactive", &e.get(&a, "0")));
    check(&e, "r1b-arr-sort-proxy-elements");
}

/// `r1b-arr-keys-tracking`
#[test]
fn arr_keys_tracking() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![st("x")])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let keys = rt.call(&rt.get_str(&d[0], "keys")?, &d[0], &[])?;
        let parts: Vec<String> = iterate_rt(rt, &keys)?.iter().map(|k| js(rt, k)).collect();
        log.borrow_mut().push(format!("keys {}", parts.join(",")));
        Ok(undef())
    });
    method(&e, &a, "push", &[st("y")]);
    log_line!(e, "runs", runs.get());
    check(&e, "r1b-arr-keys-tracking");
}

/// `r1b-arr-fill-triggers-per-write`: `fill` is not instrumented, so each element write runs the effect.
#[test]
fn arr_fill_triggers_per_write() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![num(1.0), num(2.0)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let sum = number_of(&rt.get_str(&d[0], "0")?) + number_of(&rt.get_str(&d[0], "1")?);
        log.borrow_mut().push(format!("sum {sum}"));
        Ok(undef())
    });
    method(&e, &a, "fill", &[num(0.0)]);
    let sum = number_of(&e.get(&a, "0")) + number_of(&e.get(&a, "1"));
    log_line!(e, "runs", runs.get(), format!("{sum}"));
    check(&e, "r1b-arr-fill-triggers-per-write");
}

/// `r1b-arr-holes-for-of-undefined`
#[test]
fn arr_holes_for_of_undefined() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![num(1.0), Value::Hole, num(3.0)])]);
    let parts: Vec<String> = iterate(&e, &a).iter().map(|x| sv(&e, x)).collect();
    log_line!(e, "holes-iter", parts.join(","));
    check(&e, "r1b-arr-holes-for-of-undefined");
}

/// `r1b-arr-foreach-push-visits-original`: `forEach` visits the length it started with.
#[test]
fn arr_foreach_push_visits_original() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![num(1.0), num(2.0)])]);
    let visits = Rc::new(Cell::new(0i64));
    let v2 = visits.clone();
    let cb = func(&e, "", 0, vec![a.clone()], move |rt, _this, _args, d| {
        v2.set(v2.get() + 1);
        let push = rt.get_str(&d[0], "push")?;
        rt.call(&push, &d[0], &[num(0.0)])?;
        Ok(undef())
    });
    method(&e, &a, "forEach", &[cb]);
    log_line!(e, "visits", visits.get(), sv(&e, &e.get(&a, "length")));
    check(&e, "r1b-arr-foreach-push-visits-original");
}

/// `r1b-arr-length-after-index-add`
#[test]
fn arr_length_after_index_add() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let len = rt.get_str(&d[0], "length")?;
        log.borrow_mut().push(format!("len {}", js(rt, &len)));
        Ok(undef())
    });
    e.set(&a, "3", num(1.0));
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&a, "length")));
    check(&e, "r1b-arr-length-after-index-add");
}

/// `r1b-shallow-array-push-tracks`
#[test]
fn shallow_array_push_tracks() {
    let e = Env::new();
    let a = e.invoke("shallowReactive", &[array(&e, vec![num(1.0)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let len = rt.get_str(&d[0], "length")?;
        log.borrow_mut().push(format!("len {}", js(rt, &len)));
        Ok(undef())
    });
    method(&e, &a, "push", &[num(2.0)]);
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&a, "length")));
    check(&e, "r1b-shallow-array-push-tracks");
}

/// `r1b-readonly-array-iteration-wrapped`
#[test]
fn readonly_array_iteration_wrapped() {
    let e = Env::new();
    let item = object(&e, &[("v", num(1.0))]);
    let r = e.invoke("readonly", &[array(&e, vec![item])]);
    let flags: Vec<String> = iterate(&e, &r).iter().map(|x| flag(&e, "isReadonly", x)).collect();
    let first = e.get(&r, "0");
    let includes = method(&e, &r, "includes", &[first.clone()]);
    let index = method(&e, &r, "indexOf", &[first.clone()]);
    log_line!(e, "ro-arr-iter", flags.join(","), sv(&e, &includes), sv(&e, &index));
    check(&e, "r1b-readonly-array-iteration-wrapped");
}

/// `r1b-arr-at-tracks-length`
#[test]
fn arr_at_tracks_length() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![num(1.0)])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![a.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let at = rt.call(&rt.get_str(&d[0], "at")?, &d[0], &[num(-1.0)])?;
        log.borrow_mut().push(format!("last {}", js(rt, &at)));
        Ok(undef())
    });
    method(&e, &a, "push", &[num(7.0)]);
    log_line!(e, "runs", runs.get(), sv(&e, &method(&e, &a, "at", &[num(-1.0)])));
    check(&e, "r1b-arr-at-tracks-length");
}

/// `r1b-nested-reactive-arrays-deep-tracking`: a push into an inner array does not run an effect that reads index 1.
#[test]
fn nested_reactive_arrays_deep_tracking() {
    let e = Env::new();
    let inner = array(&e, vec![num(1.0), num(2.0)]);
    let p = e.invoke("reactive", &[object(&e, &[("list", array(&e, vec![inner]))])]);
    let runs = Rc::new(Cell::new(0i64));
    let (log, r2) = (e.log.clone(), runs.clone());
    run_effect(&e, vec![p.clone()], move |rt, d| {
        r2.set(r2.get() + 1);
        let list = rt.get_str(&d[0], "list")?;
        let first = rt.get_str(&list, "0")?;
        let cell = rt.get_str(&first, "1")?;
        log.borrow_mut().push(format!("cell {}", js(rt, &cell)));
        Ok(undef())
    });
    let list = e.get(&p, "list");
    let first = e.get(&list, "0");
    method(&e, &first, "push", &[num(9.0)]);
    log_line!(e, "runs", runs.get());
    e.set(&first, "1", num(7.0));
    log_line!(e, "runs2", runs.get());
    check(&e, "r1b-nested-reactive-arrays-deep-tracking");
}

/// `r1b-push-in-effect-untracked`: a push made inside an effect does not subscribe the effect to the length it reads.
#[test]
fn push_in_effect_untracked() {
    let e = Env::new();
    let a = e.invoke("reactive", &[array(&e, vec![])]);
    let runs = Rc::new(Cell::new(0i64));
    let (r2, a_data) = (runs.clone(), a.clone());
    let body = func(&e, "", 0, vec![a_data], move |rt, _this, _args, d| {
        r2.set(r2.get() + 1);
        let push = rt.get_str(&d[0], "push")?;
        rt.call(&push, &d[0], &[num(r2.get() as f64)])?;
        Ok(undef())
    });
    e.invoke("effect", &[body]);
    log_line!(e, "runs", runs.get(), sv(&e, &e.get(&a, "length")));
    method(&e, &a, "push", &[num(9.0)]);
    log_line!(e, "runs2", runs.get(), sv(&e, &e.get(&a, "length")));
    check(&e, "r1b-push-in-effect-untracked");
}
