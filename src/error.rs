use std::fmt;
use std::rc::Rc;

#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
    pub stack: Vec<String>,
    pub thrown_value: Option<crate::value::Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    Syntax,
    Reference,
    Type,
    Range,
    Eval,
    Uri,
    User, // thrown JS value
    Internal,
}

impl Error {
    pub fn syntax(msg: impl Into<String>) -> Rc<Error> {
        Rc::new(Error { kind: ErrorKind::Syntax, message: msg.into(), stack: Vec::new(), thrown_value: None })
    }
    pub fn reference(msg: impl Into<String>) -> Rc<Error> {
        Rc::new(Error { kind: ErrorKind::Reference, message: msg.into(), stack: Vec::new(), thrown_value: None })
    }
    pub fn type_err(msg: impl Into<String>) -> Rc<Error> {
        Rc::new(Error { kind: ErrorKind::Type, message: msg.into(), stack: Vec::new(), thrown_value: None })
    }
    pub fn range(msg: impl Into<String>) -> Rc<Error> {
        Rc::new(Error { kind: ErrorKind::Range, message: msg.into(), stack: Vec::new(), thrown_value: None })
    }
    pub fn internal(msg: impl Into<String>) -> Rc<Error> {
        Rc::new(Error { kind: ErrorKind::Internal, message: msg.into(), stack: Vec::new(), thrown_value: None })
    }
    pub fn user(msg: impl Into<String>) -> Rc<Error> {
        Rc::new(Error { kind: ErrorKind::User, message: msg.into(), stack: Vec::new(), thrown_value: None })
    }
    pub fn thrown(v: crate::value::Value) -> Rc<Error> {
        let msg = match &v {
            crate::value::Value::String(s) => s.to_string(),
            crate::value::Value::Undefined => "undefined".to_string(),
            crate::value::Value::Null => "null".to_string(),
            crate::value::Value::Number(n) => crate::value::num_to_string(*n),
            crate::value::Value::Bool(b) => b.to_string(),
            crate::value::Value::Object(o) => {
                let o = o.borrow();
                if let Some(d) = o.props.get("message") {
                    if let crate::value::Value::String(s) = &d.value { s.to_string() } else { format!("{:?}", d.value) }
                } else { "[object Object]".to_string() }
            }
            crate::value::Value::Function(_) => "function".to_string(),
        };
        Rc::new(Error { kind: ErrorKind::User, message: msg, stack: Vec::new(), thrown_value: Some(v) })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.kind {
            ErrorKind::Syntax => "SyntaxError",
            ErrorKind::Reference => "ReferenceError",
            ErrorKind::Type => "TypeError",
            ErrorKind::Range => "RangeError",
            ErrorKind::Eval => "EvalError",
            ErrorKind::Uri => "URIError",
            ErrorKind::User => "Error",
            ErrorKind::Internal => "InternalError",
        };
        write!(f, "{}: {}", name, self.message)
    }
}

pub type Result<T> = std::result::Result<T, Rc<Error>>;

/// Internal control-flow signals (break/continue/return) that are not real errors.
#[derive(Debug)]
pub enum Completion {
    Normal,
    Break(Option<String>),
    Continue(Option<String>),
    Return(crate::value::Value),
}

impl Completion {
    pub fn is_normal(&self) -> bool { matches!(self, Completion::Normal) }
}
