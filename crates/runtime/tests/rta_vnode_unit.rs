//! Unit tests of the vnode layer (WP-RT-A): flag values, shape computation, block tracking, merging, cloning, GC
//! reachability and the canonical description. Expected values are written from the Vue 3.5.43 reference
//! (node_modules/@vue/runtime-core/dist/runtime-core.cjs.prod.js) and checked against the corpora in tests/ui/runtime-a.

use vn_runtime::exports::install_vue_global;
use vn_runtime::shapes::{PatchFlags, ShapeFlags, SlotFlags};
use vn_runtime::vnode::testing::describe;
use vn_value::{Flags, Intrinsic, PropKey, Rt, Value};

fn realm() -> Rt {
    let rt = Rt::new();
    install_vue_global(&rt);
    rt
}

fn export(rt: &Rt, name: &str) -> Value {
    let vue = rt.get_str(&Value::Object(rt.intrinsic(Intrinsic::Global)), "Vue").expect("global Vue");
    rt.get_str(&vue, name).expect("export")
}

fn call(rt: &Rt, name: &str, args: &[Value]) -> Value {
    let f = export(rt, name);
    rt.call(&f, &Value::Undefined, args).unwrap_or_else(|_| panic!("{name} threw"))
}

fn throws(rt: &Rt, name: &str, args: &[Value]) -> String {
    let f = export(rt, name);
    match rt.call(&f, &Value::Undefined, args) {
        Ok(_) => panic!("{name} did not throw"),
        Err(e) => rt.get_str(&e.0, "name").map(|v| rt.to_string(&v).unwrap().to_rust_string_lossy()).unwrap(),
    }
}

fn obj(rt: &Rt, pairs: &[(&str, Value)]) -> Value {
    let o = rt.new_object();
    for (k, v) in pairs {
        rt.define_data(o, PropKey::from_str(k), v.clone(), Flags::ALL).unwrap();
    }
    Value::Object(o)
}

fn s(x: &str) -> Value {
    Value::str(x)
}

#[test]
fn flag_values_match_vue_shared() {
    // @vue/shared 3.5.43: PatchFlags, ShapeFlags and SlotFlags (node -p "require('@vue/shared')").
    assert_eq!(
        [PatchFlags::TEXT, PatchFlags::CLASS, PatchFlags::STYLE, PatchFlags::PROPS, PatchFlags::FULL_PROPS, PatchFlags::NEED_HYDRATION],
        [1, 2, 4, 8, 16, 32]
    );
    assert_eq!(
        [PatchFlags::STABLE_FRAGMENT, PatchFlags::KEYED_FRAGMENT, PatchFlags::UNKEYED_FRAGMENT, PatchFlags::NEED_PATCH, PatchFlags::DYNAMIC_SLOTS, PatchFlags::DEV_ROOT_FRAGMENT],
        [64, 128, 256, 512, 1024, 2048]
    );
    assert_eq!((PatchFlags::CACHED, PatchFlags::BAIL), (-1, -2));
    assert_eq!(
        [ShapeFlags::ELEMENT, ShapeFlags::FUNCTIONAL_COMPONENT, ShapeFlags::STATEFUL_COMPONENT, ShapeFlags::TEXT_CHILDREN, ShapeFlags::ARRAY_CHILDREN, ShapeFlags::SLOTS_CHILDREN],
        [1, 2, 4, 8, 16, 32]
    );
    assert_eq!(
        [ShapeFlags::TELEPORT, ShapeFlags::SUSPENSE, ShapeFlags::COMPONENT_SHOULD_KEEP_ALIVE, ShapeFlags::COMPONENT_KEPT_ALIVE, ShapeFlags::COMPONENT],
        [64, 128, 256, 512, 6]
    );
    assert_eq!([SlotFlags::STABLE, SlotFlags::DYNAMIC, SlotFlags::FORWARDED], [1, 2, 3]);
}

#[test]
fn element_shape_counts_text_children() {
    let rt = realm();
    let v = call(&rt, "createElementVNode", &[s("div"), Value::Null, s("hi")]);
    let text = describe(&rt, &v);
    assert!(text.contains("\"shapeFlag\":9"), "{text}");
    assert!(text.contains("\"type\":\"div\""), "{text}");
    assert!(text.contains("\"children\":\"hi\""), "{text}");
}

#[test]
fn create_vnode_normalizes_class_and_style() {
    let rt = realm();
    let style = Value::Object(rt.new_array(vec![obj(&rt, &[("color", s("red"))]), s("top: 1px; left:2px")]));
    let props = obj(&rt, &[("class", Value::Object(rt.new_array(vec![s("a"), obj(&rt, &[("b", Value::Bool(true)), ("c", Value::Number(0.0))])]))), ("style", style)]);
    let v = call(&rt, "createVNode", &[s("p"), props]);
    let text = describe(&rt, &v);
    assert!(text.contains("[\"class\",\"a b\"]"), "{text}");
    assert!(text.contains("[\"style\",{\"$\":\"obj\",\"props\":[[\"color\",\"red\"],[\"top\",\"1px\"],[\"left\",\"2px\"]]}]"), "{text}");
    assert!(text.contains("\"shapeFlag\":1,\"patchFlag\":0"), "{text}");
}

#[test]
fn fragment_and_component_shapes() {
    let rt = realm();
    let frag = call(&rt, "createVNode", &[export(&rt, "Fragment"), Value::Null, Value::Null, Value::Number(64.0)]);
    assert!(describe(&rt, &frag).contains("\"shapeFlag\":0,\"patchFlag\":64"));
    let comp = obj(&rt, &[("setup", Value::Null)]);
    let v = call(&rt, "createVNode", &[comp, Value::Null]);
    assert!(describe(&rt, &v).contains("\"shapeFlag\":4"));
    let f = export(&rt, "isVNode");
    let fv = call(&rt, "createVNode", &[f, Value::Null]);
    assert!(describe(&rt, &fv).contains("\"shapeFlag\":2"));
}

#[test]
fn blocks_track_dynamic_children() {
    let rt = realm();
    call(&rt, "openBlock", &[]);
    let span = call(&rt, "createElementVNode", &[s("span"), Value::Null, Value::Null, Value::Number(1.0)]);
    let children = Value::Object(rt.new_array(vec![span]));
    let block = call(&rt, "createElementBlock", &[s("div"), Value::Null, children, Value::Number(0.0)]);
    let text = describe(&rt, &block);
    assert!(text.contains("\"dynamicChildren\":{\"len\":1,\"kinds\":[\"span\"]}"), "{text}");
}

#[test]
fn close_without_open_block_is_not_an_error() {
    let rt = realm();
    let block = call(&rt, "createBlock", &[s("i"), Value::Null, Value::Null]);
    assert!(describe(&rt, &block).contains("\"dynamicChildren\":{\"len\":0,\"kinds\":[]}"));
}

#[test]
fn disabled_tracking_gives_null_dynamic_children() {
    let rt = realm();
    call(&rt, "setBlockTracking", &[Value::Number(-1.0)]);
    let block = call(&rt, "createBlock", &[s("i"), Value::Null, Value::Null]);
    assert!(describe(&rt, &block).contains("\"dynamicChildren\":null"));
    call(&rt, "setBlockTracking", &[Value::Number(1.0)]);
}

#[test]
fn static_vnodes_are_unsupported() {
    let rt = realm();
    assert!(throws(&rt, "createStaticVNode", &[s("<b>x</b>"), Value::Number(1.0)]).contains("TypeError"));
}

#[test]
fn merge_props_joins_handlers_and_classes() {
    let rt = realm();
    let f = export(&rt, "isVNode");
    let g = export(&rt, "openBlock");
    let a = obj(&rt, &[("class", s("x")), ("onClick", f.clone())]);
    let b = obj(&rt, &[("class", s("y")), ("onClick", g.clone()), ("id", s("k"))]);
    let merged = call(&rt, "mergeProps", &[a, b]);
    let text = describe(&rt, &merged);
    assert!(text.contains("[\"class\",\"x y\"]"), "{text}");
    assert!(text.contains("[\"onClick\",[{\"$\":\"fn\",\"name\":\"isVNode\",\"length\":1},{\"$\":\"fn\",\"name\":\"openBlock\",\"length\":0}]]"), "{text}");
    assert!(text.contains("[\"id\",\"k\"]"), "{text}");
}

#[test]
fn clone_merges_extra_props_and_drops_memo() {
    let rt = realm();
    let base = call(&rt, "createElementVNode", &[s("a"), obj(&rt, &[("href", s("/x"))]), s("t"), Value::Number(8.0)]);
    let extra = obj(&rt, &[("title", s("T"))]);
    let cloned = call(&rt, "cloneVNode", &[base, extra]);
    let text = describe(&rt, &cloned);
    assert!(text.contains("[\"href\",\"/x\"]") && text.contains("[\"title\",\"T\"]"), "{text}");
    assert!(text.contains("\"patchFlag\":24"), "{text}");
}

#[test]
fn vnodes_reachable_only_through_children_survive_collection() {
    let rt = realm();
    let inner = call(&rt, "createElementVNode", &[s("em"), Value::Null, s("x")]);
    rt.root(&inner);
    let outer_children = Value::Object(rt.new_array(vec![inner]));
    let outer = call(&rt, "createElementVNode", &[s("div"), Value::Null, outer_children]);
    let id = rt.root(&outer);
    let before = describe(&rt, &outer);
    rt.collect();
    assert_eq!(describe(&rt, &outer), before);
    assert!(before.contains("\"type\":\"em\""));
    rt.unroot(id);
}

#[test]
fn is_vnode_and_text_helpers() {
    let rt = realm();
    let v = call(&rt, "createTextVNode", &[s("t")]);
    assert_eq!(call(&rt, "isVNode", &[v.clone()]), Value::Bool(true));
    assert_eq!(call(&rt, "isVNode", &[s("t")]), Value::Bool(false));
    assert!(describe(&rt, &v).contains("\"type\":{\"$\":\"sym\",\"desc\":\"v-txt\"}"));
}
