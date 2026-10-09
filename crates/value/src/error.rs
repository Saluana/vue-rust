//! Error objects and the Error hierarchy (ECMA-262 20.5). An error object is `ObjectKind::Error` with own
//! non-enumerable `message` (when given), `cause` (when given in options) and `stack`. `stack` is
//! `"{name}: {message}\n    at <anonymous>"` (no `: ` when the message is empty; see ABI.md).

use crate::function::Call;
use crate::jsstr::JsStr;
use crate::object::{Flags, Obj, ObjectKind, Prop};
use crate::rt::{Intrinsic, Rt};
use crate::value::{ObjRef, PropKey, Res, Throw, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Error,
    Type,
    Range,
    Reference,
    Syntax,
    Eval,
    Uri,
}

const ERROR_KINDS: [ErrorKind; 7] = [
    ErrorKind::Error,
    ErrorKind::Type,
    ErrorKind::Range,
    ErrorKind::Reference,
    ErrorKind::Syntax,
    ErrorKind::Eval,
    ErrorKind::Uri,
];

impl ErrorKind {
    pub fn name(self) -> &'static str {
        match self {
            ErrorKind::Error => "Error",
            ErrorKind::Type => "TypeError",
            ErrorKind::Range => "RangeError",
            ErrorKind::Reference => "ReferenceError",
            ErrorKind::Syntax => "SyntaxError",
            ErrorKind::Eval => "EvalError",
            ErrorKind::Uri => "URIError",
        }
    }

    fn prototype(self) -> Intrinsic {
        match self {
            ErrorKind::Error => Intrinsic::ErrorPrototype,
            ErrorKind::Type => Intrinsic::TypeErrorPrototype,
            ErrorKind::Range => Intrinsic::RangeErrorPrototype,
            ErrorKind::Reference => Intrinsic::ReferenceErrorPrototype,
            ErrorKind::Syntax => Intrinsic::SyntaxErrorPrototype,
            ErrorKind::Eval => Intrinsic::EvalErrorPrototype,
            ErrorKind::Uri => Intrinsic::UriErrorPrototype,
        }
    }

    fn ctor(self) -> Intrinsic {
        match self {
            ErrorKind::Error => Intrinsic::ErrorCtor,
            ErrorKind::Type => Intrinsic::TypeErrorCtor,
            ErrorKind::Range => Intrinsic::RangeErrorCtor,
            ErrorKind::Reference => Intrinsic::ReferenceErrorCtor,
            ErrorKind::Syntax => Intrinsic::SyntaxErrorCtor,
            ErrorKind::Eval => Intrinsic::EvalErrorCtor,
            ErrorKind::Uri => Intrinsic::UriErrorCtor,
        }
    }
}

/// `"{name}: {message}\n    at <anonymous>"`, or `"{name}\n    at <anonymous>"` for an empty message.
fn stack_text(name: &JsStr, message: &JsStr) -> JsStr {
    let head = if message.is_empty() {
        name.clone()
    } else {
        name.concat(&JsStr::from_str(": ")).concat(message)
    };
    head.concat(&JsStr::from_str("\n    at <anonymous>"))
}

impl Rt {
    /// An error value of a builtin kind, for runtime-raised errors (never user-visible constructors).
    pub fn make_error(&self, kind: ErrorKind, msg: &str) -> Value {
        let proto = self.intrinsic(kind.prototype());
        let name = JsStr::from_str(kind.name());
        Value::Object(self.new_error_object(proto, name, Some(JsStr::from_str(msg)), None))
    }

    /// `throw` of an error of `kind`: returns the `Err` to propagate.
    pub fn throw_error(&self, kind: ErrorKind, msg: &str) -> Throw {
        Throw(self.make_error(kind, msg))
    }

    pub fn throw_type_error(&self, msg: &str) -> Throw {
        self.throw_error(ErrorKind::Type, msg)
    }

    pub fn throw_range_error(&self, msg: &str) -> Throw {
        self.throw_error(ErrorKind::Range, msg)
    }

    pub fn throw_reference_error(&self, msg: &str) -> Throw {
        self.throw_error(ErrorKind::Reference, msg)
    }

    pub fn throw_syntax_error(&self, msg: &str) -> Throw {
        self.throw_error(ErrorKind::Syntax, msg)
    }

    fn new_error_object(&self, proto: ObjRef, name: JsStr, message: Option<JsStr>, cause: Option<Value>) -> ObjRef {
        let o = self.alloc(Obj::new(Some(proto), ObjectKind::Error));
        // Own-key order matches V8: stack, message, cause.
        let msg_text = message.clone().unwrap_or_default();
        let stack = stack_text(&name, &msg_text);
        self.define_builtin(o, PropKey::from_str("stack"), Prop::Data { value: Value::String(stack), flags: Flags::HIDDEN });
        if let Some(m) = message {
            self.define_builtin(o, PropKey::from_str("message"), Prop::Data { value: Value::String(m), flags: Flags::HIDDEN });
        }
        if let Some(c) = cause {
            self.define_builtin(o, PropKey::from_str("cause"), Prop::Data { value: c, flags: Flags::HIDDEN });
        }
        o
    }
}

/// Creates the Error constructors and prototypes and installs them on the global object.
pub(crate) fn install(rt: &Rt) {
    let ep = rt.intrinsic(Intrinsic::ErrorPrototype);
    for kind in ERROR_KINDS {
        let proto = rt.intrinsic(kind.prototype());
        let ctor = rt.new_native_constructor(
            error_call,
            error_construct,
            kind.name(),
            1,
            vec![Value::Object(proto)],
        );
        if kind != ErrorKind::Error {
            rt.with_obj_mut(proto, |o| o.proto = Some(ep));
            rt.with_obj_mut(ctor, |o| o.proto = Some(rt.intrinsic(Intrinsic::ErrorCtor)));
        }
        rt.set_intrinsic(kind.ctor(), ctor);
        rt.define_builtin(
            ctor,
            PropKey::from_str("prototype"),
            Prop::Data { value: Value::Object(proto), flags: Flags::NONE },
        );
        rt.define_builtin(
            proto,
            PropKey::from_str("constructor"),
            Prop::Data { value: Value::Object(ctor), flags: Flags::HIDDEN },
        );
        let name = JsStr::from_str(kind.name());
        rt.define_builtin(proto, PropKey::from_str("name"), Prop::Data { value: Value::String(name), flags: Flags::HIDDEN });
        rt.define_builtin(
            proto,
            PropKey::from_str("message"),
            Prop::Data { value: Value::str(""), flags: Flags::HIDDEN },
        );
        rt.install_global(kind.name(), Value::Object(ctor));
    }
    rt.define_native_method(ep, "toString", error_to_string, 0);
}

fn error_call(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    construct_error(rt, c, &Value::Undefined)
}

fn error_construct(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    construct_error(rt, c, c.new_target)
}

/// Shared body of `Error(...)` and `new Error(...)`: the prototype comes from `new.target` when it has one.
fn construct_error(rt: &Rt, c: &Call<'_>, new_target: &Value) -> Res<Value> {
    let default_proto = match rt.fn_data(c.callee).first() {
        Some(Value::Object(p)) => *p,
        _ => panic!("error constructor without a prototype (framework bug)"),
    };
    let mut proto = default_proto;
    if let Value::Object(nt) = new_target {
        if let Value::Object(p) = rt.get(*nt, &PropKey::from_str("prototype"), new_target)? {
            proto = p;
        }
    }
    let msg = c.arg(0);
    let message = if msg.is_undefined() { None } else { Some(rt.to_string(&msg)?) };
    let opts = c.arg(1);
    let mut cause = None;
    if let Value::Object(oo) = &opts {
        let key = PropKey::from_str("cause");
        if rt.has_property(*oo, &key)? {
            cause = Some(rt.get(*oo, &key, &opts)?);
        }
    }
    let name_v = rt.get(proto, &PropKey::from_str("name"), &Value::Object(proto))?;
    let name = if name_v.is_undefined() { JsStr::from_str("Error") } else { rt.to_string(&name_v)? };
    Ok(Value::Object(rt.new_error_object(proto, name, message, cause)))
}

/// Error.prototype.toString (ECMA-262 20.5.3.4).
fn error_to_string(rt: &Rt, c: &Call<'_>) -> Res<Value> {
    let Value::Object(o) = c.this else {
        return Err(rt.throw_type_error("Error.prototype.toString requires that 'this' be an Object"));
    };
    let name_v = rt.get(*o, &PropKey::from_str("name"), c.this)?;
    let name = if name_v.is_undefined() { JsStr::from_str("Error") } else { rt.to_string(&name_v)? };
    let msg_v = rt.get(*o, &PropKey::from_str("message"), c.this)?;
    let msg = if msg_v.is_undefined() { JsStr::empty() } else { rt.to_string(&msg_v)? };
    let out = if name.is_empty() {
        msg
    } else if msg.is_empty() {
        name
    } else {
        name.concat(&JsStr::from_str(": ")).concat(&msg)
    };
    Ok(Value::String(out))
}
