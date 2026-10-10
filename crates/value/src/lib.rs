//! vn-value: the JavaScript value runtime. WP-L1 provides UTF-16 strings (`jsstr`) and ECMAScript number
//! semantics (`number`). WP-L2a adds the kernel: values, heap and collector, object model, functions and
//! cells, errors, property operations, the job queue and the watchdog. The public API is described in
//! ABI.md. Safe Rust only, no dependencies.

#![forbid(unsafe_code)]

pub mod builtins;
pub mod coerce;
pub mod error;
pub(crate) mod fdlibm;
pub mod function;
mod heap;
pub mod jsstr;
pub mod module;
pub mod native;
pub mod number;
pub mod object;
pub mod ops;
pub mod roots;
pub mod rt;
pub mod value;

pub use error::ErrorKind;
pub use function::{Call, CodeFn, FnInfo, FnKind, FunctionData};
pub use heap::HeapStats;
pub use jsstr::JsStr;
pub use module::{ModuleDef, ModuleStatus};
pub use native::{payload_ref, Exotic, Hook, NativeObject};
pub use object::{Flags, ObjClass, ObjectKind, Prop, PropertyMap, MAX_DENSE_LEN};
pub use ops::{same_value_zero, strict_eq};
pub use roots::{ProviderId, RootId, RootProvider, Trace, Tracer};
pub use rt::{Hint, Intrinsic, Rt, WeakRt, WellKnown};
pub use value::{same_value, ObjRef, PropKey, Res, Sym, Throw, Value};
