use std::fmt;
use std::sync::Arc;

use crate::value::Value;

#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
    pub stack: Vec<String>,
    pub thrown_value: Option<Value>,
    pub line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    Syntax,
    Reference,
    Type,
    Range,
    Eval,
    Uri,
    User,
    Internal,
}

impl Error {
    /// Return a copy of this error with the source line attached, unless a
    /// line is already set (the first occurrence wins).
    pub fn with_line(&self, line: Option<usize>) -> Arc<Error> {
        let new_line = match (&self.line, line) {
            (Some(_), _) => self.line,
            (None, Some(l)) => Some(l),
            _ => self.line,
        };
        Arc::new(Error {
            kind: self.kind.clone(),
            message: self.message.clone(),
            stack: self.stack.clone(),
            thrown_value: self.thrown_value.clone(),
            line: new_line,
        })
    }
    pub fn syntax(msg: impl Into<String>) -> Arc<Error> {
        Arc::new(Error {
            kind: ErrorKind::Syntax,
            message: msg.into(),
            stack: Vec::new(),
            thrown_value: None,
            line: None,
        })
    }
    pub fn reference(msg: impl Into<String>) -> Arc<Error> {
        Arc::new(Error {
            kind: ErrorKind::Reference,
            message: msg.into(),
            stack: Vec::new(),
            thrown_value: None,
            line: None,
        })
    }
    pub fn type_err(msg: impl Into<String>) -> Arc<Error> {
        Arc::new(Error {
            kind: ErrorKind::Type,
            message: msg.into(),
            stack: Vec::new(),
            thrown_value: None,
            line: None,
        })
    }
    pub fn range(msg: impl Into<String>) -> Arc<Error> {
        Arc::new(Error {
            kind: ErrorKind::Range,
            message: msg.into(),
            stack: Vec::new(),
            thrown_value: None,
            line: None,
        })
    }
    pub fn internal(msg: impl Into<String>) -> Arc<Error> {
        Arc::new(Error {
            kind: ErrorKind::Internal,
            message: msg.into(),
            stack: Vec::new(),
            thrown_value: None,
            line: None,
        })
    }
    pub fn thrown(v: Value, heap: &crate::gc::Heap) -> Arc<Error> {
        let msg = value_to_message(&v, heap);
        Arc::new(Error {
            kind: ErrorKind::User,
            message: msg,
            stack: Vec::new(),
            thrown_value: Some(v),
            line: None,
        })
    }
}

fn value_to_message(v: &Value, heap: &crate::gc::Heap) -> String {
    match v {
        Value::String(s) => s.to_string(),
        Value::Undefined => "undefined".to_string(),
        Value::Null => "null".to_string(),
        Value::Number(n) => crate::value::num_to_string(*n),
        Value::Bool(b) => b.to_string(),
        Value::BigInt(n) => n.to_string(),
        Value::Object(idx) => heap.with_obj(idx.0, |obj| {
            let props = obj.props();
            if let Some(desc) = props
                .lock()
                .unwrap()
                .get(&crate::value::PropertyKey::from("message"))
            {
                match &desc.value {
                    Value::String(s) => s.to_string(),
                    _ => "[object Error]".to_string(),
                }
            } else {
                "[object Object]".to_string()
            }
        }),
        Value::Symbol(_) => "Symbol".to_string(),
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
        if let Some(line) = self.line {
            write!(f, "{}: {} (at line {})", name, self.message, line)
        } else {
            write!(f, "{}: {}", name, self.message)
        }
    }
}

pub type Result<T> = std::result::Result<T, Arc<Error>>;

/// Internal control-flow signals.
#[derive(Debug)]
pub enum Completion {
    Normal,
    Break(Option<String>),
    Continue(Option<String>),
    Return(Value),
}

impl Completion {
    pub fn is_normal(&self) -> bool {
        matches!(self, Completion::Normal)
    }
}
