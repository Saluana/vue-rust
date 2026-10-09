//! The canonical element snapshot (docs/dev/ui-scenario-host.md, "Canonical element snapshot"). DocHost::snapshot already
//! writes the element tree in the contract's key order, with two differences the encoder removes: `props` is an object
//! (vn-dom writes it so), and the contract wants it as an array of [key, value] pairs in the order value, checked,
//! selected, disabled. The props themselves (dirty value, checked, selected, disabled) are decided by vn-dom's
//! serializer, which follows the rule of tooling/oracle/ui/snapshot.mjs.

use vn_testkit::json::Json;
use vn_testkit::json_in::parse;
use vn_value::{Flags, JsStr, PropKey, Rt, Value};

/// The contract's props order.
const PROP_ORDER: [&str; 4] = ["value", "checked", "selected", "disabled"];

/// Parses a DocHost snapshot and moves its props into the contract shape.
pub fn contract_of(text: &str) -> Result<Json, String> {
    let json = parse(text).map_err(|e| format!("snapshot is not JSON: {e:?}"))?;
    Ok(contract(&json))
}

/// The contract shape of a parsed snapshot tree.
pub fn contract(j: &Json) -> Json {
    match j {
        Json::Obj(pairs) => Json::Obj(
            pairs
                .iter()
                .map(|(k, v)| {
                    let value = match String::from_utf16_lossy(k).as_str() {
                        "props" => props_pairs(v),
                        _ => contract(v),
                    };
                    (k.clone(), value)
                })
                .collect(),
        ),
        Json::Arr(items) => Json::Arr(items.iter().map(contract).collect()),
        other => other.clone(),
    }
}

/// `{"value":..,"checked":..}` as `[["value",..],["checked",..]]`, in the contract's order. Unknown keys (never written
/// by vn-dom) keep their place after the known ones.
fn props_pairs(v: &Json) -> Json {
    let Json::Obj(pairs) = v else { return v.clone() };
    let rank = |k: &str| PROP_ORDER.iter().position(|p| *p == k).unwrap_or(PROP_ORDER.len());
    let mut keyed: Vec<(usize, String, Json)> = pairs
        .iter()
        .enumerate()
        .map(|(i, (k, val))| {
            let key = String::from_utf16_lossy(k);
            (rank(&key) * 1000 + i, key, val.clone())
        })
        .collect();
    keyed.sort_by_key(|(order, _, _)| *order);
    Json::Arr(keyed.into_iter().map(|(_, key, val)| Json::Arr(vec![Json::text(&key), val])).collect())
}

/// The children of an element snapshot (the `children` array), in document order.
pub fn children_of(element: &Json) -> Vec<Json> {
    element.field("children").and_then(Json::items).map(<[Json]>::to_vec).unwrap_or_default()
}

/// A snapshot tree as JavaScript values: plain objects with the keys in order, and arrays.
pub fn to_js(rt: &Rt, j: &Json) -> Value {
    match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Num(text) => Value::Number(text.parse::<f64>().unwrap_or(f64::NAN)),
        Json::Str(units) => Value::String(JsStr::from_units(units)),
        Json::Arr(items) => {
            let values: Vec<Value> = items.iter().map(|x| to_js(rt, x)).collect();
            Value::Object(rt.new_array(values))
        }
        Json::Obj(pairs) => {
            let obj = rt.new_object();
            for (k, v) in pairs {
                let value = to_js(rt, v);
                let key = PropKey::from_str(&String::from_utf16_lossy(k));
                let _ = rt.define_data(obj, key, value, Flags::ALL);
            }
            Value::Object(obj)
        }
    }
}
