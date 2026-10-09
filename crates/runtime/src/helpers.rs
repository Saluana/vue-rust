//! vn-runtime::helpers: renderList, createSlots, toHandlers, withMemo and isMemoSame (behaviour per @vue/runtime-core
//! 3.5.43). Like the reference, the sloppy-mode writes (`cache[index] = ret`, `slots[name] = fn`) do not throw when refused.

use vn_value::{Call, JsStr, PropKey, Res, Rt, Value, MAX_DENSE_LEN};

use crate::normalize::{array_get, for_in_keys, is_obj, key_js, length_of, to_handler_key};
use crate::vnode::{as_vnode, current_block_push, has_current_block, memo_of, set_member, tracking_enabled, with_state, State};

/// `cached && cached[i]`: a falsy `cached` is its own value.
fn cached_item(rt: &Rt, cached: &Value, i: usize) -> Res<Value> {
    if cached.to_boolean() {
        rt.get_v(cached, &PropKey::Index(i as u32))
    } else {
        Ok(cached.clone())
    }
}

/// `new Array(n)` as a loop bound: a valid length, within the dense limit.
fn array_len_arg(rt: &Rt, n: f64) -> Res<usize> {
    if !(n >= 0.0 && n.fract() == 0.0 && n <= MAX_DENSE_LEN as f64) {
        return Err(rt.throw_range_error("Invalid array length"));
    }
    Ok(n as usize)
}

/// renderList(source, renderItem, cache, index): arrays and strings call `renderItem(item, i, undefined, cached)`,
/// numbers `renderItem(i + 1, i, ...)`, iterables `renderItem(item, i, ...)`, other objects
/// `renderItem(value, key, i, cached)`. The result is stored in `cache[index]` when a cache is given.
pub(crate) fn render_list(rt: &Rt, source: Value, render: Value, cache: Value, index: Value) -> Res<Value> {
    let cached = if cache.to_boolean() { rt.get_v(&cache, &rt.to_property_key(&index)?)? } else { cache.clone() };
    let mut out: Vec<Value> = Vec::new();
    if rt.is_array(&source) || matches!(source, Value::String(_)) {
        let len = rt.to_number(&rt.get_str(&source, "length")?)?;
        let n = array_len_arg(rt, if len.is_nan() { 0.0 } else { len })?;
        for i in 0..n {
            let item = rt.get_v(&source, &PropKey::Index(i as u32))?;
            let extra = cached_item(rt, &cached, i)?;
            out.push(rt.call(&render, &Value::Undefined, &[item, Value::Number(i as f64), Value::Undefined, extra])?);
        }
    } else if let Value::Number(n) = source {
        let n = array_len_arg(rt, n)?;
        for i in 0..n {
            let extra = cached_item(rt, &cached, i)?;
            out.push(rt.call(&render, &Value::Undefined, &[Value::Number(i as f64 + 1.0), Value::Number(i as f64), Value::Undefined, extra])?);
        }
    } else if is_obj(rt, &source) {
        let iter_method = rt.get_v(&source, &PropKey::Sym(rt.well_known(vn_value::WellKnown::Iterator)))?;
        if iter_method.to_boolean() {
            let mut items = Vec::new();
            vn_value::builtins::iterator::iterate(rt, &source, &mut |v| {
                items.push(v);
                Ok(true)
            })?;
            for (i, item) in items.into_iter().enumerate() {
                let extra = cached_item(rt, &cached, i)?;
                out.push(rt.call(&render, &Value::Undefined, &[item, Value::Number(i as f64), Value::Undefined, extra])?);
            }
        } else {
            let keys: Vec<PropKey> = for_in_keys(rt, &source)?
                .into_iter()
                .filter(|k| {
                    let Value::Object(o) = &source else { return false };
                    rt.get_own_property(*o, k).ok().flatten().is_some_and(|p| p.flags().enumerable)
                })
                .collect();
            for (i, k) in keys.iter().enumerate() {
                let value = rt.get_v(&source, k)?;
                let extra = cached_item(rt, &cached, i)?;
                out.push(rt.call(&render, &Value::Undefined, &[value, Value::String(key_js(k)), Value::Number(i as f64), extra])?);
            }
        }
    }
    let ret = Value::Object(rt.new_array(out));
    if cache.to_boolean() {
        rt.set_v(&cache, &rt.to_property_key(&index)?, ret.clone())?;
    }
    Ok(ret)
}

/// createSlots(slots, dynamicSlots): entries are `{name, fn}` or lists of them; an entry with a key gets a wrapper
/// that writes the key onto the vnode it returns.
pub(crate) fn create_slots(rt: &Rt, slots: Value, dynamic: Value) -> Res<Value> {
    let mut i = 0usize;
    while (i as f64) < length_of(rt, &dynamic)? {
        let slot = array_get(rt, &dynamic, i)?;
        if rt.is_array(&slot) {
            let mut j = 0usize;
            while (j as f64) < length_of(rt, &slot)? {
                let item = array_get(rt, &slot, j)?;
                let name = rt.get_str(&item, "name")?;
                let key = rt.to_property_key(&name)?;
                let f = rt.get_str(&item, "fn")?;
                rt.set_v(&slots, &key, f)?;
                j += 1;
            }
        } else if slot.to_boolean() {
            let name = rt.get_str(&slot, "name")?;
            let key = rt.to_property_key(&name)?;
            let slot_key = rt.get_str(&slot, "key")?;
            let value = if slot_key.to_boolean() {
                Value::Object(rt.new_native_function(slot_with_key_code, "", 0, vec![slot.clone()]))
            } else {
                rt.get_str(&slot, "fn")?
            };
            rt.set_v(&slots, &key, value)?;
        }
        i += 1;
    }
    Ok(slots)
}

/// `(...args) => { const res = slot.fn(...args); if (res) res.key = slot.key; return res; }`
fn slot_with_key_code(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let slot = rt.fn_data(c.callee).first().cloned().unwrap_or(Value::Undefined);
    let f = rt.get_str(&slot, "fn")?;
    let res = rt.call(&f, &Value::Undefined, c.args)?;
    if res.to_boolean() {
        let k = rt.get_str(&slot, "key")?;
        set_member(rt, &res, "key", k)?;
    }
    Ok(res)
}

/// toHandlers(obj, preserveCase): every key of `obj` becomes `on` + capitalized key (or `on:key` when the case is kept
/// and the key has an uppercase letter).
pub(crate) fn to_handlers(rt: &Rt, obj: Value, preserve_case: bool) -> Res<Value> {
    let ret = rt.new_object();
    for k in for_in_keys(rt, &obj)? {
        let name = key_js(&k);
        let has_upper = name.units().iter().any(|c| (0x41..=0x5A).contains(c));
        let out_key = if preserve_case && has_upper {
            JsStr::from_str("on:").concat(&name)
        } else {
            to_handler_key(rt, &Value::String(name))?
        };
        let value = rt.get_v(&obj, &k)?;
        rt.set_v(&Value::Object(ret), &PropKey::from_jsstr(out_key), value)?;
    }
    Ok(Value::Object(ret))
}

/// isMemoSame(cached, memo): the same length and `Object.is` on every entry. A match re-tracks the cached vnode in
/// the current block.
pub(crate) fn is_memo_same(rt: &Rt, st: &State, cached: &Value, memo: &Value) -> Res<bool> {
    let prev = memo_of(rt, cached)?;
    let prev_len = rt.get_str(&prev, "length")?;
    let memo_len = rt.get_str(memo, "length")?;
    if !rt.loose_eq(&prev_len, &memo_len)?.to_boolean() {
        return Ok(false);
    }
    let mut i = 0usize;
    loop {
        let limit = rt.get_str(&prev, "length")?;
        if !rt.lt(&Value::Number(i as f64), &limit)?.to_boolean() {
            break;
        }
        let (a, b) = (rt.get_v(&prev, &PropKey::Index(i as u32))?, rt.get_v(memo, &PropKey::Index(i as u32))?);
        if !vn_value::same_value(&a, &b) {
            return Ok(false);
        }
        i += 1;
    }
    if tracking_enabled(st) && has_current_block(st) {
        let Some(o) = as_vnode(rt, cached) else {
            return Err(rt.throw_type_error("isMemoSame: a memoized value that is not a VNode is not supported"));
        };
        current_block_push(rt, st, o);
    }
    Ok(true)
}

/// withMemo(memo, render, cache, index): the cached entry when `memo` is unchanged, else a fresh render that keeps a
/// copy of `memo` and its index.
pub(crate) fn with_memo(rt: &Rt, st: &State, memo: Value, render: Value, cache: Value, index: Value) -> Res<Value> {
    let key = rt.to_property_key(&index)?;
    let cached = rt.get_v(&cache, &key)?;
    if cached.to_boolean() && is_memo_same(rt, st, &cached, &memo)? {
        return Ok(cached);
    }
    let ret = rt.call(&render, &Value::Undefined, &[])?;
    let slice = rt.get_str(&memo, "slice")?;
    let copy = rt.call(&slice, &memo, &[])?;
    set_member(rt, &ret, "memo", copy)?;
    set_member(rt, &ret, "cacheIndex", index)?;
    rt.set_v(&cache, &key, ret.clone())?;
    Ok(ret)
}

fn n_render_list(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    render_list(rt, c.arg(0), c.arg(1), c.arg(2), c.arg(3))
}

fn n_create_slots(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    create_slots(rt, c.arg(0), c.arg(1))
}

fn n_to_handlers(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    to_handlers(rt, c.arg(0), c.arg(1).to_boolean())
}

fn n_with_memo(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (memo, render, cache, index) = (c.arg(0), c.arg(1), c.arg(2), c.arg(3));
    with_state(rt, c, |st| with_memo(rt, st, memo, render, cache, index))
}

fn n_is_memo_same(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let (cached, memo) = (c.arg(0), c.arg(1));
    with_state(rt, c, |st| Ok(Value::Bool(is_memo_same(rt, st, &cached, &memo)?)))
}

/// The exports of this module (renderList, createSlots, toHandlers, withMemo, isMemoSame), with `state` as data.
pub(crate) fn exports(rt: &Rt, state: vn_value::ObjRef) -> Vec<(&'static str, Value)> {
    let d = [Value::Object(state)];
    vec![
        crate::vnode::func(rt, "renderList", 4, n_render_list, &[]),
        crate::vnode::func(rt, "createSlots", 2, n_create_slots, &[]),
        crate::vnode::func(rt, "toHandlers", 2, n_to_handlers, &[]),
        crate::vnode::func(rt, "withMemo", 4, n_with_memo, &d),
        crate::vnode::func(rt, "isMemoSame", 2, n_is_memo_same, &d),
    ]
}
