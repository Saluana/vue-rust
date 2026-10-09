//! Functions, calls and cells. `CodeFn` is the one code type for compiled and native functions. A function
//! object is a heap object holding a plain fn pointer, captured cells and native closure data (both traced
//! by the collector). No Rust closure may own a `Value` or `ObjRef`: the collector cannot see it.

use std::cell::Cell;
use std::rc::Rc;

use crate::heap::PROP_BYTES;
use crate::jsstr::JsStr;
use crate::object::{Flags, Obj, ObjectKind, Prop};
use crate::rt::{Intrinsic, Rt};
use crate::value::{ObjRef, PropKey, Res, Value};

/// Code of a function. `call.new_target` is `undefined` for a plain call.
pub type CodeFn = fn(&Rt, &Call<'_>) -> Res<Value>;

/// Arguments of one invocation. The slices and references are borrowed for the call only.
pub struct Call<'a> {
    pub this: &'a Value,
    pub args: &'a [Value],
    pub callee: ObjRef,
    pub new_target: &'a Value,
}

impl Call<'_> {
    /// Argument `i`, or `undefined` when fewer were passed.
    pub fn arg(&self, i: usize) -> Value {
        self.args.get(i).cloned().unwrap_or(Value::Undefined)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FnKind {
    Normal,
    Arrow,
    Method,
    Getter,
    Setter,
    Async,
}

/// Name, length and kind of a function being created.
pub struct FnInfo {
    pub name: JsStr,
    pub length: u32,
    pub kind: FnKind,
}

impl FnInfo {
    pub fn new(name: &str, length: u32, kind: FnKind) -> FnInfo {
        FnInfo { name: JsStr::from_str(name), length, kind }
    }
}

/// Payload of a function object. `ctor` is the code used by `construct`; a function without it is not a
/// constructor. Builtin constructors receive `this = undefined` and the new target.
pub struct FunctionData {
    pub code: CodeFn,
    pub captures: Rc<[ObjRef]>,
    pub data: Rc<[Value]>,
    pub name: JsStr,
    pub length: u32,
    pub kind: FnKind,
    pub ctor: Option<CodeFn>,
}

/// Decrements the call depth when a call returns, including by unwinding.
struct DepthGuard<'a>(&'a Cell<u32>);

impl Drop for DepthGuard<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

/// Code of `Function.prototype`: callable, returns undefined.
pub(crate) fn function_prototype_code(_rt: &Rt, _c: &Call<'_>) -> Res<Value> {
    Ok(Value::Undefined)
}

impl Rt {
    /// Turns a fresh object into a function: sets its payload and its own `length` and `name` (both
    /// non-writable, non-enumerable, configurable, in that order as in JavaScript).
    pub(crate) fn fill_function(&self, r: ObjRef, data: FunctionData) {
        let (len, name) = (data.length, data.name.clone());
        self.with_obj_mut(r, |o| {
            o.kind = ObjectKind::Function(data);
            o.props.push(
                PropKey::from_str("length"),
                Prop::Data { value: Value::Number(len as f64), flags: Flags::CONFIG },
            );
            o.props.push(PropKey::from_str("name"), Prop::Data { value: Value::String(name), flags: Flags::CONFIG });
        });
        self.0.heap.borrow_mut().add_bytes(r, 2 * PROP_BYTES);
    }

    fn function_obj(&self, data: FunctionData) -> ObjRef {
        let r = self.alloc(Obj::new(Some(self.intrinsic(Intrinsic::FunctionPrototype)), ObjectKind::Ordinary));
        self.fill_function(r, data);
        r
    }

    /// A user function. `captures` are the cells of its closure variables, in order.
    pub fn new_function(&self, code: CodeFn, info: FnInfo, captures: &[ObjRef]) -> ObjRef {
        self.function_obj(FunctionData {
            code,
            captures: Rc::from(captures),
            data: Rc::from(Vec::<Value>::new()),
            name: info.name,
            length: info.length,
            kind: info.kind,
            ctor: None,
        })
    }

    /// A builtin function. `data` is its closure data, readable through `fn_data`.
    pub fn new_native_function(&self, code: CodeFn, name: &str, length: u32, data: Vec<Value>) -> ObjRef {
        self.function_obj(FunctionData {
            code,
            captures: Rc::from(Vec::<ObjRef>::new()),
            data: Rc::from(data),
            name: JsStr::from_str(name),
            length,
            kind: FnKind::Normal,
            ctor: None,
        })
    }

    /// A builtin constructor: `code` runs for plain calls, `ctor` for `construct`.
    pub fn new_native_constructor(
        &self,
        code: CodeFn,
        ctor: CodeFn,
        name: &str,
        length: u32,
        data: Vec<Value>,
    ) -> ObjRef {
        self.function_obj(FunctionData {
            code,
            captures: Rc::from(Vec::<ObjRef>::new()),
            data: Rc::from(data),
            name: JsStr::from_str(name),
            length,
            kind: FnKind::Normal,
            ctor: Some(ctor),
        })
    }

    /// The cells captured by a function object.
    pub fn captures(&self, callee: ObjRef) -> Rc<[ObjRef]> {
        self.with_obj(callee, |o| match &o.kind {
            ObjectKind::Function(f) => f.captures.clone(),
            _ => panic!("captures() of a non-function object (framework bug)"),
        })
    }

    /// The closure data of a function object.
    pub fn fn_data(&self, callee: ObjRef) -> Rc<[Value]> {
        self.with_obj(callee, |o| match &o.kind {
            ObjectKind::Function(f) => f.data.clone(),
            _ => panic!("fn_data() of a non-function object (framework bug)"),
        })
    }

    fn code_of(&self, f: ObjRef) -> Option<CodeFn> {
        self.with_obj(f, |o| match &o.kind {
            ObjectKind::Function(fd) => Some(fd.code),
            _ => None,
        })
    }

    fn ctor_of(&self, f: ObjRef) -> Option<CodeFn> {
        self.with_obj(f, |o| match &o.kind {
            ObjectKind::Function(fd) => fd.ctor,
            _ => None,
        })
    }

    pub fn is_callable(&self, v: &Value) -> bool {
        match v {
            Value::Object(o) => self.code_of(*o).is_some(),
            _ => false,
        }
    }

    pub fn is_constructor(&self, v: &Value) -> bool {
        match v {
            Value::Object(o) => self.ctor_of(*o).is_some(),
            _ => false,
        }
    }

    /// Current call nesting. Zero when no generated or native code is on the Rust stack.
    pub fn call_depth(&self) -> u32 {
        self.0.depth.get()
    }

    pub fn max_call_depth(&self) -> u32 {
        self.0.max_depth.get()
    }

    pub fn set_max_call_depth(&self, n: u32) {
        self.0.max_depth.set(n);
    }

    fn enter(&self) -> Res<DepthGuard<'_>> {
        let st = &self.0;
        let d = st.depth.get();
        if d >= st.max_depth.get() {
            return Err(self.throw_range_error("Maximum call stack size exceeded"));
        }
        st.depth.set(d + 1);
        Ok(DepthGuard(&st.depth))
    }

    /// Call (ECMA-262 Call). `this` is passed unchanged; the kernel does not coerce it.
    pub fn call(&self, callee: &Value, this: &Value, args: &[Value]) -> Res<Value> {
        let code = match callee {
            Value::Object(f) => self.code_of(*f),
            _ => None,
        };
        let (Some(code), Value::Object(f)) = (code, callee) else {
            return Err(self.throw_type_error(&format!("{} is not a function", self.describe(callee))));
        };
        let _g = self.enter()?;
        let no_target = Value::Undefined;
        let c = Call { this, args, callee: *f, new_target: &no_target };
        code(self, &c)
    }

    /// Construct. Only builtin constructors (functions with a `ctor`) can be constructed; user functions
    /// are not constructors and throw TypeError.
    pub fn construct(&self, callee: &Value, args: &[Value], new_target: &Value) -> Res<Value> {
        let ctor = match callee {
            Value::Object(f) => self.ctor_of(*f),
            _ => None,
        };
        let (Some(ctor), Value::Object(f)) = (ctor, callee) else {
            return Err(self.throw_type_error(&format!("{} is not a constructor", self.describe(callee))));
        };
        if !self.is_constructor(new_target) {
            return Err(self.throw_type_error("new.target is not a constructor"));
        }
        let _g = self.enter()?;
        let this = Value::Undefined;
        let c = Call { this: &this, args, callee: *f, new_target };
        ctor(self, &c)
    }

    /// A closure variable cell. `Value::Uninit` puts it in the temporal dead zone.
    pub fn new_cell(&self, v: Value) -> ObjRef {
        self.alloc(Obj::new(None, ObjectKind::Cell(v)))
    }

    fn cell_value(&self, cell: ObjRef) -> Value {
        self.with_obj(cell, |o| match &o.kind {
            ObjectKind::Cell(v) => v.clone(),
            _ => panic!("cell access on a non-cell object (framework bug)"),
        })
    }

    pub fn cell_get(&self, cell: ObjRef) -> Value {
        self.cell_value(cell)
    }

    pub fn cell_set(&self, cell: ObjRef, v: Value) {
        self.with_obj_mut(cell, |o| match &mut o.kind {
            ObjectKind::Cell(slot) => *slot = v,
            _ => panic!("cell access on a non-cell object (framework bug)"),
        });
    }

    /// Reads a lexical binding: ReferenceError while it is uninitialized.
    pub fn cell_get_checked(&self, cell: ObjRef, name: &str) -> Res<Value> {
        match self.cell_value(cell) {
            Value::Uninit => Err(self.throw_reference_error(&format!("Cannot access '{name}' before initialization"))),
            v => Ok(v),
        }
    }

    /// Assigns a lexical binding: ReferenceError while it is uninitialized.
    pub fn cell_set_checked(&self, cell: ObjRef, name: &str, v: Value) -> Res<()> {
        if matches!(self.cell_value(cell), Value::Uninit) {
            return Err(self.throw_reference_error(&format!("Cannot access '{name}' before initialization")));
        }
        self.cell_set(cell, v);
        Ok(())
    }

    /// Initializes a lexical binding (the declaration). Not checked: the compiler calls it once.
    pub fn cell_init(&self, cell: ObjRef, v: Value) {
        self.cell_set(cell, v);
    }
}
