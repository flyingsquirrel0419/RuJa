//! Environment helpers (EnvironmentData lives in value.rs as a HeapObj variant).

use crate::gc::Heap;
use crate::value::{BindingKind, GcIdx, HeapObj, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Allocate a new environment in the heap.
pub fn new_env(heap: &Heap, parent: Option<GcIdx>, is_function_scope: bool) -> GcIdx {
    let env = HeapObj::Environment(crate::value::EnvironmentData {
        vars: RefCell::new(HashMap::new()),
        parent: RefCell::new(parent),
        is_function_scope,
    });
    GcIdx(heap.allocate(env))
}

pub fn declare(heap: &Heap, env: GcIdx, name: &str, value: Value, kind: BindingKind) {
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            e.vars.borrow_mut().insert(
                Rc::from(name),
                crate::value::Binding { value: RefCell::new(value), kind },
            );
        }
    });
}

pub fn get(heap: &Heap, env: GcIdx, name: &str) -> Option<Value> {
    let (val, parent) = heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            if let Some(b) = e.vars.borrow().get(name) {
                return (Some(b.value.borrow().clone()), None);
            }
            return (None, *e.parent.borrow());
        }
        (None, None)
    });
    if let Some(v) = val {
        return Some(v);
    }
    if let Some(p) = parent {
        return get(heap, p, name);
    }
    None
}

pub fn set(heap: &Heap, env: GcIdx, name: &str, value: Value) -> bool {
    let value_copy = value.clone();
    let (found, parent) = heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            if let Some(b) = e.vars.borrow().get(name) {
                if b.kind == BindingKind::Const {
                    return (false, None); // assignment to const
                }
                *b.value.borrow_mut() = value_copy;
                return (true, None);
            }
            return (false, *e.parent.borrow());
        }
        (false, None)
    });
    if found {
        return true;
    }
    // false could mean "not found" or "const violation"; caller distinguishes
    if let Some(p) = parent {
        return set(heap, p, name, value.clone());
    }
    false
}

pub fn has(heap: &Heap, env: GcIdx, name: &str) -> bool {
    let (found, parent) = heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            return (e.vars.borrow().contains_key(name), *e.parent.borrow());
        }
        (false, None)
    });
    if found {
        return true;
    }
    if let Some(p) = parent {
        return has(heap, p, name);
    }
    false
}

/// Walk up to the nearest function scope (or global) and declare/assign a var.
pub fn declare_var(heap: &Heap, env: GcIdx, name: &str, value: Value) {
    let root = function_scope_root(heap, env);
    heap.with_obj(root.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            if let Some(b) = e.vars.borrow().get(name) {
                *b.value.borrow_mut() = value;
            } else {
                e.vars.borrow_mut().insert(
                    Rc::from(name),
                    crate::value::Binding {
                        value: RefCell::new(value),
                        kind: BindingKind::Var,
                    },
                );
            }
        }
    });
}

/// Find the nearest function-scope ancestor (or global).
pub fn function_scope_root(heap: &Heap, env: GcIdx) -> GcIdx {
    let mut cur = env;
    loop {
        let parent = heap.with_obj(cur.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                if e.is_function_scope {
                    return None; // this IS the function scope
                }
                return *e.parent.borrow();
            }
            None
        });
        match parent {
            Some(p) => cur = p,
            None => return cur,
        }
    }
}
