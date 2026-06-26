use crate::error;
use crate::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum BindingKind {
    Var,
    Let,
    Const,
}

pub struct Binding {
    pub value: RefCell<Value>,
    pub kind: BindingKind,
    pub mutable: bool,
}

#[derive(Default)]
pub struct EnvData {
    pub vars: HashMap<Rc<str>, Rc<Binding>>,
    pub parent: Option<Env>,
}

#[derive(Clone)]
pub struct Env(pub Rc<RefCell<EnvData>>);

impl Env {
    pub fn new() -> Self {
        Env(Rc::new(RefCell::new(EnvData::default())))
    }

    pub fn child(&self) -> Env {
        Env(Rc::new(RefCell::new(EnvData {
            vars: HashMap::new(),
            parent: Some(self.clone()),
        })))
    }

    /// Declare a binding in THIS scope only.
    pub fn declare(&self, name: &str, value: Value, kind: BindingKind) {
        let binding = Rc::new(Binding {
            value: RefCell::new(value),
            mutable: !matches!(kind, BindingKind::Const),
            kind,
        });
        self.0.borrow_mut().vars.insert(Rc::from(name), binding);
    }

    /// Declare or reassign a var binding (hoists to function/global scope).
    pub fn declare_var(&self, name: &str, value: Value) {
        // Walk up to nearest function scope. For simplicity treat global as function scope.
        let mut scope = self.clone();
        loop {
            let parent;
            {
                let data = scope.0.borrow();
                parent = data.parent.clone();
            }
            match parent {
                Some(p) => scope = p,
                None => break,
            }
        }
        let mut data = scope.0.borrow_mut();
        if let Some(b) = data.vars.get(name) {
            *b.value.borrow_mut() = value;
        } else {
            let binding = Rc::new(Binding {
                value: RefCell::new(value),
                mutable: true,
                kind: BindingKind::Var,
            });
            data.vars.insert(Rc::from(name), binding);
        }
    }

    pub fn get(&self, name: &str) -> error::Result<Value> {
        let mut scope = self.clone();
        loop {
            let parent;
            {
                let data = scope.0.borrow();
                if let Some(b) = data.vars.get(name) {
                    return Ok(b.value.borrow().clone());
                }
                parent = data.parent.clone();
            }
            match parent {
                Some(p) => scope = p,
                None => return Err(error::Error::reference(format!("{} is not defined", name))),
            }
        }
    }

    pub fn set(&self, name: &str, value: Value) -> error::Result<()> {
        let mut scope = self.clone();
        loop {
            let parent;
            {
                let data = scope.0.borrow();
                if let Some(b) = data.vars.get(name) {
                    if !b.mutable {
                        return Err(error::Error::type_err("Assignment to constant variable.".to_string()));
                    }
                    *b.value.borrow_mut() = value;
                    return Ok(());
                }
                parent = data.parent.clone();
            }
            match parent {
                Some(p) => scope = p,
                None => return Err(error::Error::reference(format!("{} is not defined", name))),
            }
        }
    }

    pub fn has(&self, name: &str) -> bool {
        let mut scope = self.clone();
        loop {
            let parent;
            {
                let data = scope.0.borrow();
                if data.vars.contains_key(name) {
                    return true;
                }
                parent = data.parent.clone();
            }
            match parent {
                Some(p) => scope = p,
                None => return false,
            }
        }
    }
}
