//! Value model for the RuJa VM.
//!
//! `Value` is a tagged union. Heap objects live in the GC heap as `HeapObj`
//! and are referenced by `GcIdx`. The GC traces reachable objects from roots
//! and reclaims the rest, including reference cycles.

use crate::ast::FunctionExpr;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// A handle into the GC heap.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GcIdx(pub usize);

/// The value type used throughout the engine.
#[derive(Clone)]
pub enum Value {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(Rc<str>),
    Object(GcIdx),
    Symbol(u32),
}

impl Value {
    pub fn undefined() -> Self {
        Value::Undefined
    }
    pub fn null() -> Self {
        Value::Null
    }
    pub fn from_bool(b: bool) -> Self {
        Value::Bool(b)
    }
    pub fn from_num(n: f64) -> Self {
        Value::Number(n)
    }
    pub fn from_str(s: &str) -> Self {
        Value::String(Rc::from(s))
    }

    pub fn is_undefined(&self) -> bool {
        matches!(self, Value::Undefined)
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
    pub fn is_nullish(&self) -> bool {
        matches!(self, Value::Null | Value::Undefined)
    }
    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Undefined | Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0 && !n.is_nan(),
            Value::String(s) => s.len() > 0,
            Value::Object(_) | Value::Symbol(_) => true,
        }
    }

    pub fn type_of(&self) -> &'static str {
        match self {
            Value::Undefined => "undefined",
            Value::Null => "object",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Object(_) => "object",
            Value::Symbol(_) => "symbol",
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => a == b,
            (Value::Symbol(a), Value::Symbol(b)) => a == b,
            _ => false,
        }
    }
}

/// Quick string conversion for argument handling (not spec-compliant ToString).
pub fn value_to_debug_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_string(),
        Value::Number(n) => num_to_string(*n),
        Value::Bool(b) => b.to_string(),
        Value::Undefined => "undefined".to_string(),
        Value::Null => "null".to_string(),
        _ => format!("{:?}", v),
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Undefined => write!(f, "undefined"),
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{:?}", s),
            Value::Object(_) => write!(f, "[object]"),
            Value::Symbol(_) => write!(f, "[symbol]"),
        }
    }
}

/// A heap-allocated JS object. All heap-resident data is one of these.
pub enum HeapObj {
    Object(ObjectData),
    Array(ArrayData),
    Function(FunctionData),
    Environment(EnvironmentData),
    Map(MapData),
    Set(SetData),
    Promise(PromiseData),
    Generator(GeneratorData),
    Iterator(IteratorData),
}

/// Generic JS object.
pub struct ObjectData {
    pub props: RefCell<HashMap<Rc<str>, PropertyDescriptor>>,
    pub proto: RefCell<Option<Value>>,
    pub extensible: Cell<bool>,
    pub class_name: Option<Rc<str>>,
}

pub struct ArrayData {
    pub items: RefCell<Vec<Value>>,
    pub props: RefCell<HashMap<Rc<str>, PropertyDescriptor>>,
    pub proto: RefCell<Option<Value>>,
}

pub struct FunctionData {
    pub name: Option<Rc<str>>,
    pub kind: FunctionKind,
    pub closure: GcIdx,
    pub prototype: RefCell<Option<Value>>,
    pub props: RefCell<HashMap<Rc<str>, PropertyDescriptor>>,
}

pub enum FunctionKind {
    Native {
        func: crate::vm::NativeFn,
        length: usize,
    },
    Interpreted {
        func: std::rc::Rc<crate::function::FunctionDef>,
    },
    Bound {
        target: GcIdx,
        this_val: Value,
        bound_args: Vec<Value>,
    },
}

pub struct EnvironmentData {
    pub vars: RefCell<HashMap<Rc<str>, Binding>>,
    pub parent: RefCell<Option<GcIdx>>,
    pub is_function_scope: bool,
}

pub struct Binding {
    pub value: RefCell<Value>,
    pub kind: BindingKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BindingKind {
    Var,
    Let,
    Const,
}

pub struct MapData {
    pub entries: RefCell<Vec<(Value, Value)>>,
    pub props: RefCell<HashMap<Rc<str>, PropertyDescriptor>>,
    pub proto: RefCell<Option<Value>>,
}

pub struct SetData {
    pub items: RefCell<Vec<Value>>,
    pub props: RefCell<HashMap<Rc<str>, PropertyDescriptor>>,
    pub proto: RefCell<Option<Value>>,
}

pub struct PromiseData {
    pub state: Cell<PromiseStatus>,
    pub result: RefCell<Value>,
    pub handlers: RefCell<Vec<PromiseHandler>>,
    pub props: RefCell<HashMap<Rc<str>, PropertyDescriptor>>,
    pub proto: RefCell<Option<Value>>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum PromiseStatus {
    Pending,
    Fulfilled,
    Rejected,
}

pub struct PromiseHandler {
    pub on_fulfilled: Value,
    pub on_rejected: Value,
}

pub struct GeneratorData {
    pub function: FunctionExpr,
    pub closure: GcIdx,
    pub state: RefCell<Vec<Value>>,
    pub ip: Cell<usize>,
    pub done: Cell<bool>,
    pub props: RefCell<HashMap<Rc<str>, PropertyDescriptor>>,
    pub proto: RefCell<Option<Value>>,
}

/// Internal iterator state used by `for...of` / `for...in` and the spread operator.
pub struct IteratorData {
    /// Remaining values to yield, in order.
    pub items: RefCell<Vec<Value>>,
    /// Current position into `items`.
    pub index: Cell<usize>,
}

#[derive(Clone)]
pub struct PropertyDescriptor {
    pub value: Value,
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
    pub get: Option<Value>,
    pub set: Option<Value>,
    pub is_accessor: bool,
}

impl Default for PropertyDescriptor {
    fn default() -> Self {
        PropertyDescriptor {
            value: Value::Undefined,
            writable: true,
            enumerable: true,
            configurable: true,
            get: None,
            set: None,
            is_accessor: false,
        }
    }
}

impl PropertyDescriptor {
    pub fn data(value: Value) -> Self {
        PropertyDescriptor {
            value,
            ..Default::default()
        }
    }
}

impl HeapObj {
    /// Is this object callable?
    pub fn is_function(&self) -> bool {
        matches!(self, HeapObj::Function(_))
    }

    /// Common props accessor for any object kind.
    pub fn props(&self) -> &RefCell<HashMap<Rc<str>, PropertyDescriptor>> {
        match self {
            HeapObj::Object(o) => &o.props,
            HeapObj::Array(a) => &a.props,
            HeapObj::Function(f) => &f.props,
            HeapObj::Map(m) => &m.props,
            HeapObj::Set(s) => &s.props,
            HeapObj::Promise(p) => &p.props,
            HeapObj::Generator(g) => &g.props,
            HeapObj::Iterator(_) => panic!("iterator has no props"),
            HeapObj::Environment(_) => panic!("env has no props"),
        }
    }

    /// Common proto accessor.
    pub fn proto(&self) -> &RefCell<Option<Value>> {
        match self {
            HeapObj::Object(o) => &o.proto,
            HeapObj::Array(a) => &a.proto,
            HeapObj::Function(f) => &f.prototype,
            HeapObj::Map(m) => &m.proto,
            HeapObj::Set(s) => &s.proto,
            HeapObj::Promise(p) => &p.proto,
            HeapObj::Generator(g) => &g.proto,
            HeapObj::Environment(_) => panic!("env has no proto"),
            HeapObj::Iterator(_) => panic!("iterator has no proto"),
        }
    }

    /// Class name for `Object.prototype.toString`.
    pub fn class_name(&self) -> &str {
        match self {
            HeapObj::Object(o) => o
                .class_name
                .as_ref()
                .map(|s| s.as_ref())
                .unwrap_or("Object"),
            HeapObj::Array(_) => "Array",
            HeapObj::Function(_) => "Function",
            HeapObj::Map(_) => "Map",
            HeapObj::Set(_) => "Set",
            HeapObj::Promise(_) => "Promise",
            HeapObj::Generator(_) => "Generator",
            HeapObj::Iterator(_) => "Iterator",
            HeapObj::Environment(_) => "Environment",
        }
    }

    pub fn is_array(&self) -> bool {
        matches!(self, HeapObj::Array(_))
    }
    pub fn is_extensible(&self) -> bool {
        match self {
            HeapObj::Object(o) => o.extensible.get(),
            _ => true,
        }
    }
}

/// Render an f64 the way JS `String(n)` would.
pub fn num_to_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n == f64::INFINITY {
        return "Infinity".to_string();
    }
    if n == f64::NEG_INFINITY {
        return "-Infinity".to_string();
    }
    if n == 0.0 {
        return "0".to_string();
    }
    if n.fract() == 0.0 && n.abs() < 1e21 {
        return format!("{}", n as i64);
    }
    let s = format!("{}", n);
    if s.ends_with(".0") {
        s[..s.len() - 2].to_string()
    } else {
        s
    }
}
