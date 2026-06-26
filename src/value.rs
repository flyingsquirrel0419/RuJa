use crate::ast::FunctionExpr;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

pub type NativeFn = fn(&mut crate::interpreter::Interpreter, &[Value], Option<Value>) -> Result<Value, Rc<crate::error::Error>>;

#[derive(Clone)]
pub enum Value {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(Rc<str>),
    Object(Rc<RefCell<Obj>>),
    Function(Rc<FunctionValue>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => Rc::ptr_eq(a, b),
            (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

pub struct FunctionValue {
    pub name: Option<Rc<str>>,
    pub kind: FunctionKind,
    pub closure: crate::environment::Env,
    pub prototype: Option<Value>, // .prototype for constructors
    pub properties: RefCell<HashMap<Rc<str>, PropertyDescriptor>>,
}

pub enum FunctionKind {
    Native {
        func: NativeFn,
        length: usize,
    },
    Interpreted {
        func: FunctionExpr,
    },
    Bound {
        target: Rc<FunctionValue>,
        this_val: Value,
        bound_args: Vec<Value>,
    },
}

pub struct Obj {
    pub props: HashMap<Rc<str>, PropertyDescriptor>,
    pub proto: Option<Value>,
    pub class_name: Option<Rc<str>>, // "[object Array]" etc
    pub extensible: bool,
    // internal data for arrays / primitives wrappers
    pub internal: InternalData,
}

pub enum InternalData {
    None,
    Array(Vec<Value>),
    String(Rc<str>),
    Number(f64),
    Boolean(bool),
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
        PropertyDescriptor { value, ..Default::default() }
    }
}

/// Render an f64 the way JavaScript `String(n)` would.
pub fn num_to_string(n: f64) -> String {
    if n.is_nan() { return "NaN".to_string(); }
    if n == f64::INFINITY { return "Infinity".to_string(); }
    if n == f64::NEG_INFINITY { return "-Infinity".to_string(); }
    if n == 0.0 { return "0".to_string(); }
    if n.fract() == 0.0 && n.abs() < 1e21 {
        return format!("{}", n as i64);
    }
    // Rust formats floats with a trailing .0; JS does not.
    let s = format!("{}", n);
    if s.ends_with(".0") { s[..s.len()-2].to_string() } else { s }
}

impl Value {
    pub fn undefined() -> Self { Value::Undefined }
    pub fn null() -> Self { Value::Null }
    pub fn from_bool(b: bool) -> Self { Value::Bool(b) }
    pub fn from_num(n: f64) -> Self { Value::Number(n) }
    pub fn from_str(s: &str) -> Self { Value::String(Rc::from(s)) }

    pub fn is_undefined(&self) -> bool { matches!(self, Value::Undefined) }
    pub fn is_null(&self) -> bool { matches!(self, Value::Null) }
    pub fn is_nullish(&self) -> bool { matches!(self, Value::Null | Value::Undefined) }
    pub fn is_object(&self) -> bool { matches!(self, Value::Object(_)) }
    pub fn is_function(&self) -> bool { matches!(self, Value::Function(_)) }
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Undefined | Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0 && !n.is_nan(),
            Value::String(s) => s.len() > 0,
            Value::Object(_) | Value::Function(_) => true,
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
            Value::Function(_) => "function",
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Undefined => write!(f, "undefined"),
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Object(_) => write!(f, "[object Object]"),
            Value::Function(_) => write!(f, "[Function]"),
        }
    }
}

impl Obj {
    pub fn new() -> Self {
        Obj {
            props: HashMap::new(),
            proto: None,
            class_name: None,
            extensible: true,
            internal: InternalData::None,
        }
    }

    pub fn new_array() -> Self {
        Obj {
            props: HashMap::new(),
            proto: None,
            class_name: Some(Rc::from("[object Array]")),
            extensible: true,
            internal: InternalData::Array(Vec::new()),
        }
    }

    pub fn new_string(s: Rc<str>) -> Self {
        Obj {
            props: HashMap::new(),
            proto: None,
            class_name: Some(Rc::from("[object String]")),
            extensible: false,
            internal: InternalData::String(s),
        }
    }

    pub fn is_array(&self) -> bool { matches!(self.internal, InternalData::Array(_)) }
    pub fn is_string_wrapper(&self) -> bool { matches!(self.internal, InternalData::String(_)) }

    pub fn get_own(&self, key: &str) -> Option<&PropertyDescriptor> {
        self.props.get(key)
    }
}
