use std::fmt;
use std::sync::Arc;

use crate::value::{HeapObj, PropertyKey, Value};

#[derive(Debug, Clone)]
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
    /// Execution-fuel exhaustion: a host-level abort that is NOT catchable by
    /// user `try/catch`, so untrusted code cannot swallow it and keep running.
    Fuel,
}

impl Error {
    /// Whether a thrown error can be caught by a JS `try/catch`. Most errors
    /// (TypeError, user-thrown, ...) are catchable; a fuel exhaustion is a
    /// host-level abort that must propagate out so untrusted code cannot
    /// swallow it and keep running.
    pub fn catchable(&self) -> bool {
        !matches!(self.kind, ErrorKind::Fuel)
    }

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
    pub fn uri(msg: impl Into<String>) -> Arc<Error> {
        Arc::new(Error {
            kind: ErrorKind::Uri,
            message: msg.into(),
            stack: Vec::new(),
            thrown_value: None,
            line: None,
        })
    }
    /// A non-catchable fuel-exhaustion abort (displayed as RangeError).
    pub fn fuel(msg: impl Into<String>) -> Arc<Error> {
        Arc::new(Error {
            kind: ErrorKind::Fuel,
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
        let kind = error_kind_from_value(&v, heap).unwrap_or(ErrorKind::User);
        Arc::new(Error {
            kind,
            message: msg,
            stack: Vec::new(),
            thrown_value: Some(v),
            line: None,
        })
    }
}

fn error_kind_from_value(v: &Value, heap: &crate::gc::Heap) -> Option<ErrorKind> {
    let Value::Object(idx) = v else {
        return None;
    };
    heap.with_obj(idx.0, |obj| {
        let name = obj
            .props()
            .lock()
            .get(&PropertyKey::from("name"))
            .and_then(|desc| match &desc.value {
                Value::String(s) => Some(s.as_ref().to_string()),
                _ => None,
            })
            .unwrap_or_else(|| obj.class_name().to_string());
        match name.as_str() {
            "SyntaxError" => Some(ErrorKind::Syntax),
            "ReferenceError" => Some(ErrorKind::Reference),
            "TypeError" => Some(ErrorKind::Type),
            "RangeError" => Some(ErrorKind::Range),
            "EvalError" => Some(ErrorKind::Eval),
            "URIError" => Some(ErrorKind::Uri),
            "Error" => Some(ErrorKind::User),
            _ => None,
        }
    })
}

fn value_to_message(v: &Value, heap: &crate::gc::Heap) -> String {
    match v {
        Value::String(s) => s.to_string(),
        Value::Undefined => "undefined".to_string(),
        Value::Null => "null".to_string(),
        Value::Number(n) => crate::value::num_to_string(*n),
        Value::Bool(b) => b.to_string(),
        Value::BigInt(n) => n.to_string(),
        Value::Object(idx) => {
            let (message, class_name, proto) = heap.with_obj(idx.0, |obj| {
                (
                    own_string_property(obj, "message"),
                    obj.class_name().to_string(),
                    obj.proto().lock().clone(),
                )
            });
            if class_name == "Object" {
                if let Some(name) = constructor_name_from_prototype(proto, heap) {
                    if name != "Object" {
                        return format!("{}: {}", name, message.unwrap_or_default());
                    }
                }
            }
            message.unwrap_or_else(|| "[object Object]".to_string())
        }
        Value::Symbol(_) => "Symbol".to_string(),
        Value::PrivateName(key) => format!("[private #{}]", key.description),
        Value::Reference(_) => "[reference]".to_string(),
    }
}

fn own_string_property(obj: &HeapObj, key: &str) -> Option<String> {
    obj.props()
        .lock()
        .get(&PropertyKey::from(key))
        .and_then(|desc| match &desc.value {
            Value::String(s) => Some(s.to_string()),
            _ => None,
        })
}

fn constructor_name_from_prototype(proto: Option<Value>, heap: &crate::gc::Heap) -> Option<String> {
    let Value::Object(proto_idx) = proto? else {
        return None;
    };
    let ctor = heap.with_obj(proto_idx.0, |obj| {
        obj.props()
            .lock()
            .get(&PropertyKey::from("constructor"))
            .map(|desc| desc.value.clone())
    })?;
    let Value::Object(ctor_idx) = ctor else {
        return None;
    };
    heap.with_obj(ctor_idx.0, |obj| match obj {
        HeapObj::Function(f) => f.name.as_ref().map(|s| s.to_string()),
        _ => own_string_property(obj, "name"),
    })
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
            ErrorKind::Fuel => "RangeError",
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
