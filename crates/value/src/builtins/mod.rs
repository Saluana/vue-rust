//! Builtin families (Object, Function, Symbol, Array, String, Number, Math, Map/Set, JSON, ...). One file per family,
//! each exposing `pub fn install(rt: &Rt)`. `install_all` runs at the end of `Rt::new()`; the order matters where a family
//! looks up another's intrinsics (`Rt::builtin(name)`): iterator first, then Object/Function/Symbol/Boolean/globals,
//! Number/String/Math, Array, Map/Set, JSON. See docs/dev/wp/BUILTINS-COMMON.md.

use crate::rt::Rt;

pub mod array;
pub mod boolean;
pub mod function;
pub mod global;
pub mod iterator;
pub mod json;
pub mod map_set;
pub mod math;
pub mod number;
pub mod object;
pub mod promise;
pub mod string;
pub mod symbol;

/// Installs every builtin family into a fresh realm.
pub(crate) fn install_all(rt: &Rt) {
    iterator::install(rt);
    object::install(rt);
    function::install(rt);
    symbol::install(rt);
    boolean::install(rt);
    global::install(rt);
    number::install(rt);
    string::install(rt);
    math::install(rt);
    array::install(rt);
    map_set::install(rt);
    json::install(rt);
    promise::install(rt);
}
