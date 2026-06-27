//! Environment helpers (EnvironmentData lives in value.rs as a HeapObj variant).

use crate::gc::Heap;
use crate::value::{BindingKind, GcIdx, HeapObj, Value};
use indexmap::IndexMap;
use std::cell::RefCell;

use std::rc::Rc;

pub fn new_env(heap: &Heap, parent: Option<GcIdx>, is_function_scope: bool) -> GcIdx {
    let env = HeapObj::Environment(crate::value::EnvironmentData {
        vars: RefCell::new(IndexMap::new()),
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
                crate::value::Binding {
                    value: RefCell::new(value.clone()),
                    kind,
                },
            );
        }
    });
}

pub fn get(heap: &Heap, env: GcIdx, name: &str) -> Option<Value> {
    let mut cur = Some(env);
    while let Some(e_idx) = cur {
        let (val, parent) = heap.with_obj(e_idx.0, |obj| {
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
        cur = parent;
    }
    None
}

pub fn set(heap: &Heap, env: GcIdx, name: &str, value: Value) -> bool {
    let mut cur = Some(env);
    while let Some(e_idx) = cur {
        let (found, is_const, parent) = heap.with_obj(e_idx.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                if let Some(b) = e.vars.borrow().get(name) {
                    return (true, b.kind == BindingKind::Const, None);
                }
                return (false, false, *e.parent.borrow());
            }
            (false, false, None)
        });
        if found {
            if is_const {
                return false;
            }
            heap.with_obj(e_idx.0, |obj| {
                if let HeapObj::Environment(e) = obj {
                    if let Some(b) = e.vars.borrow().get(name) {
                        *b.value.borrow_mut() = value.clone();
                    } else {
                    }
                }
            });
            return true;
        }
        cur = parent;
    }
    false
}

/// Returns true if `name` is bound as a `const` in the scope chain.
pub fn is_const(heap: &Heap, env: GcIdx, name: &str) -> bool {
    let mut cur = Some(env);
    while let Some(e_idx) = cur {
        let (is_c, parent) = heap.with_obj(e_idx.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                if let Some(b) = e.vars.borrow().get(name) {
                    return (b.kind == BindingKind::Const, None);
                }
                return (false, *e.parent.borrow());
            }
            (false, None)
        });
        if is_c {
            return true;
        }
        cur = parent;
    }
    false
}

pub fn has(heap: &Heap, env: GcIdx, name: &str) -> bool {
    let mut cur = Some(env);
    while let Some(e_idx) = cur {
        let (found, parent) = heap.with_obj(e_idx.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                return (e.vars.borrow().contains_key(name), *e.parent.borrow());
            }
            (false, None)
        });
        if found {
            return true;
        }
        cur = parent;
    }
    false
}

pub fn declare_var(heap: &Heap, env: GcIdx, name: &str, value: Value) {
    let root = function_scope_root(heap, env);
    // Check existence first (drop the borrow) before mutating.
    let exists = heap.with_obj(root.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            e.vars.borrow().contains_key(name)
        } else {
            false
        }
    });
    let _ = exists;
    heap.with_obj(root.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            if e.vars.borrow().contains_key(name) {
                if let Some(b) = e.vars.borrow().get(name) {
                    *b.value.borrow_mut() = value;
                }
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

pub fn function_scope_root(heap: &Heap, env: GcIdx) -> GcIdx {
    let mut cur = env;
    loop {
        let parent = heap.with_obj(cur.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                if e.is_function_scope {
                    return None;
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
