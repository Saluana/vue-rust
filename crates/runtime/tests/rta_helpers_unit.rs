//! Unit tests of toDisplayString for inputs the oracle corpus cannot carry (functions and symbols), written from
//! @vue/shared 3.5.43: a custom toString is used through String(), a symbol is `Symbol(description)`.

use vn_runtime::exports::install_vue_global;
use vn_value::{Call, Flags, Intrinsic, JsStr, PropKey, Res, Rt, Value};

fn realm() -> Rt {
    let rt = Rt::new();
    install_vue_global(&rt);
    rt
}

fn display(rt: &Rt, v: Value) -> Value {
    let vue = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), "Vue").expect("Vue");
    let f = rt.get_str(&vue, "toDisplayString").expect("export");
    rt.call(&f, &Value::Undefined, &[v]).expect("toDisplayString does not throw")
}

fn custom_to_string(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::str("custom"))
}

#[test]
fn custom_to_string_is_used_through_string() {
    let rt = realm();
    let f = rt.new_native_function(custom_to_string, "toString", 0, Vec::new());
    let o = rt.new_object();
    rt.define_data(o, PropKey::from_str("toString"), Value::Object(f), Flags::ALL).unwrap();
    assert_eq!(display(&rt, Value::Object(o)), Value::str("custom"));
}

#[test]
fn symbols_display_as_their_description() {
    let rt = realm();
    let named = rt.new_symbol(Some(JsStr::from_str("x")));
    let unnamed = rt.new_symbol(None);
    assert_eq!(display(&rt, Value::Symbol(named)), Value::str("Symbol(x)"));
    assert_eq!(display(&rt, Value::Symbol(unnamed)), Value::str("Symbol()"));
}

#[test]
fn nullish_and_strings_are_returned_as_described() {
    let rt = realm();
    assert_eq!(display(&rt, Value::Null), Value::str(""));
    assert_eq!(display(&rt, Value::Undefined), Value::str(""));
    assert_eq!(display(&rt, Value::str("kept")), Value::str("kept"));
}
