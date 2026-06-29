//! Environment helpers (EnvironmentData lives in value.rs as a HeapObj variant).

use crate::gc::Heap;
use crate::value::{BindingKind, GcIdx, HeapObj, Value};
use indexmap::IndexMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

use std::sync::Arc;

pub fn new_env(heap: &Heap, parent: Option<GcIdx>, is_function_scope: bool) -> GcIdx {
    let env = HeapObj::Environment(crate::value::EnvironmentData {
        vars: Mutex::new(IndexMap::new()),
        parent: Mutex::new(parent),
        is_function_scope,
        with_object: Mutex::new(None),
    });
    GcIdx(heap.allocate(env))
}

/// Create a per-iteration child environment for a `for (let ...)` loop: copy
/// the current lexical (`let`/`const`) bindings of `env` into a fresh child
/// environment whose parent is `env`. This gives each iteration its own
/// binding so closures created in the body capture distinct values (the
/// classic `for (let i...) out.push(()=>i)` case). `var` bindings are not
/// copied (they belong to the function scope, not the loop).
pub fn clone_lexical_env(heap: &Heap, env: GcIdx) -> GcIdx {
    // The child's parent is `env` itself. The body runs in `child` (so
    // closures capture a per-iteration binding), then the frame env is
    // restored to `env` (child's parent) before the update runs, so the
    // chain does not grow across iterations and outer scopes stay reachable.
    let child = new_env(heap, Some(env), false);
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            let vars = e.vars.lock().unwrap();
            let cloned: Vec<(Arc<str>, crate::value::Binding)> = vars
                .iter()
                .filter(|(_, b)| b.kind != BindingKind::Var)
                .map(|(k, b)| {
                    (
                        k.clone(),
                        crate::value::Binding {
                            value: Mutex::new(b.value.lock().unwrap().clone()),
                            kind: b.kind,
                            initialized: AtomicBool::new(
                                b.initialized.load(std::sync::atomic::Ordering::Relaxed),
                            ),
                        },
                    )
                })
                .collect();
            drop(vars);
            heap.with_obj(child.0, |cobj| {
                if let HeapObj::Environment(ce) = cobj {
                    for (k, b) in cloned {
                        ce.vars.lock().unwrap().insert(k, b);
                    }
                }
            });
        }
    });
    child
}

/// Per-iteration environment for `for (let ...)`: copy ONLY the named loop
/// variables into a fresh child env whose parent is `env`. Outer `let`s are
/// NOT copied, so mutations to them in the body persist in `env` (via the
/// chain). Each iteration's closures capture a distinct binding for the loop
/// variable while sharing the rest of the scope.
pub fn clone_loop_vars(heap: &Heap, env: GcIdx, names: &[Arc<str>]) -> GcIdx {
    let child = new_env(heap, Some(env), false);
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            let vars = e.vars.lock().unwrap();
            let cloned: Vec<(Arc<str>, crate::value::Binding)> = vars
                .iter()
                .filter(|(k, _)| names.iter().any(|n| n.as_ref() == k.as_ref()))
                .map(|(k, b)| {
                    (
                        k.clone(),
                        crate::value::Binding {
                            value: Mutex::new(b.value.lock().unwrap().clone()),
                            kind: b.kind,
                            initialized: AtomicBool::new(
                                b.initialized.load(std::sync::atomic::Ordering::Relaxed),
                            ),
                        },
                    )
                })
                .collect();
            drop(vars);
            heap.with_obj(child.0, |cobj| {
                if let HeapObj::Environment(ce) = cobj {
                    for (k, b) in cloned {
                        ce.vars.lock().unwrap().insert(k, b);
                    }
                }
            });
        }
    });
    child
}

/// Create a `with`-statement environment record wrapping `object`: name lookups
/// that miss the lexical chain fall back to `object`'s own properties.
pub fn new_with_env(heap: &Heap, parent: GcIdx, object: crate::value::Value) -> GcIdx {
    let env = HeapObj::Environment(crate::value::EnvironmentData {
        vars: Mutex::new(IndexMap::new()),
        parent: Mutex::new(Some(parent)),
        is_function_scope: false,
        with_object: Mutex::new(Some(object)),
    });
    GcIdx(heap.allocate(env))
}

/// True if `env` has a binding for `name` that is NOT a `var` (i.e. a
/// lexical `let`/`const`). Used by direct-eval leak-back to avoid clobbering
/// an existing lexical binding when a `var` of the same name is declared in
/// eval.
pub fn has_lexical_binding(heap: &Heap, env: GcIdx, name: &str) -> bool {
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            if let Some(b) = e.vars.lock().unwrap().get(name) {
                return b.kind != BindingKind::Var;
            }
        }
        false
    })
}

pub fn declare(heap: &Heap, env: GcIdx, name: &str, value: Value, kind: BindingKind) {
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            e.vars.lock().unwrap().insert(
                Arc::from(name),
                crate::value::Binding {
                    value: Mutex::new(value.clone()),
                    kind,
                    initialized: AtomicBool::new(true),
                },
            );
        }
    });
}

/// Declare a binding in the TDZ (uninitialized). Reading it before it is
/// initialized throws a ReferenceError.
pub fn declare_uninit(heap: &Heap, env: GcIdx, name: &str, kind: BindingKind) {
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            e.vars.lock().unwrap().insert(
                Arc::from(name),
                crate::value::Binding {
                    value: Mutex::new(Value::Undefined),
                    kind,
                    initialized: AtomicBool::new(false),
                },
            );
        }
    });
}

/// Collect `with`-statement object environment records along the scope chain
/// (closest first), so the VM can fall back to property lookup on each object
/// when a name is not bound lexically.
pub fn with_objects(heap: &Heap, env: GcIdx) -> Vec<Value> {
    let mut out = Vec::new();
    let mut cur = Some(env);
    while let Some(e_idx) = cur {
        let (obj, parent) = heap.with_obj(e_idx.0, |o| {
            if let HeapObj::Environment(e) = o {
                (
                    e.with_object.lock().unwrap().clone(),
                    *e.parent.lock().unwrap(),
                )
            } else {
                (None, None)
            }
        });
        if let Some(o) = obj {
            out.push(o);
        }
        cur = parent;
    }
    out
}
/// Get a binding, returning an error if it exists but is in the TDZ.
pub fn get_checked(heap: &Heap, env: GcIdx, name: &str) -> Result<Option<Value>, bool> {
    let mut cur = Some(env);
    while let Some(e_idx) = cur {
        let (val, in_tdz, parent) = heap.with_obj(e_idx.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                if let Some(b) = e.vars.lock().unwrap().get(name) {
                    if !b.initialized.load(Ordering::Relaxed) {
                        return (None, true, None);
                    }
                    return (Some(b.value.lock().unwrap().clone()), false, None);
                }
                return (None, false, *e.parent.lock().unwrap());
            }
            (None, false, None)
        });
        if in_tdz {
            return Err(true);
        }
        if let Some(v) = val {
            return Ok(Some(v));
        }
        cur = parent;
    }
    Err(false)
}

/// Initialize (or re-initialize) a binding's value and mark it initialized.
pub fn initialize(heap: &Heap, env: GcIdx, name: &str, value: Value) -> bool {
    let mut cur = Some(env);
    while let Some(e_idx) = cur {
        let (found, parent) = heap.with_obj(e_idx.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                if let Some(b) = e.vars.lock().unwrap().get(name) {
                    *b.value.lock().unwrap() = value.clone();
                    b.initialized.store(true, Ordering::Relaxed);
                    return (true, None);
                }
                return (false, *e.parent.lock().unwrap());
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

/// Initialize a binding in the *current* environment only (no parent walk).
/// Used for TDZ: the binding was declared uninitialized at scope entry; this
/// sets its value and lifts the TDZ. Returns false if no binding exists here.
pub fn initialize_local(heap: &Heap, env: GcIdx, name: &str, value: Value) -> bool {
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            if let Some(b) = e.vars.lock().unwrap().get(name) {
                *b.value.lock().unwrap() = value;
                b.initialized.store(true, Ordering::Relaxed);
                return true;
            }
        }
        false
    })
}

/// Declare a binding with a value directly in the current env (initialized).
/// Like `declare` but takes an explicit kind, used for const destructuring etc.
pub fn declare_typed(heap: &Heap, env: GcIdx, name: &str, value: Value, kind: BindingKind) {
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            e.vars.lock().unwrap().insert(
                Arc::from(name),
                crate::value::Binding {
                    value: Mutex::new(value),
                    kind,
                    initialized: AtomicBool::new(true),
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
                if let Some(b) = e.vars.lock().unwrap().get(name) {
                    return (Some(b.value.lock().unwrap().clone()), None);
                }
                return (None, *e.parent.lock().unwrap());
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
                if let Some(b) = e.vars.lock().unwrap().get(name) {
                    return (true, b.kind == BindingKind::Const, None);
                }
                return (false, false, *e.parent.lock().unwrap());
            }
            (false, false, None)
        });
        if found {
            if is_const {
                return false;
            }
            heap.with_obj(e_idx.0, |obj| {
                if let HeapObj::Environment(e) = obj {
                    if let Some(b) = e.vars.lock().unwrap().get(name) {
                        *b.value.lock().unwrap() = value.clone();
                    }
                }
            });
            return true;
        }
        cur = parent;
    }
    false
}

/// Outcome of a TDZ-aware assignment to a name.
pub enum SetOutcome {
    Set,
    Const,
    /// Binding exists but is in the TDZ (not yet initialized).
    Tdz,
    NotFound,
}

/// TDZ-aware set: refuses to write a binding that is still in the TDZ.
pub fn set_checked(heap: &Heap, env: GcIdx, name: &str, value: Value) -> SetOutcome {
    let mut cur = Some(env);
    while let Some(e_idx) = cur {
        let (outcome, parent) = heap.with_obj(e_idx.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                if let Some(b) = e.vars.lock().unwrap().get(name) {
                    if !b.initialized.load(Ordering::Relaxed) {
                        return (SetOutcome::Tdz, None);
                    }
                    if b.kind == BindingKind::Const {
                        return (SetOutcome::Const, None);
                    }
                    *b.value.lock().unwrap() = value.clone();
                    return (SetOutcome::Set, None);
                }
                return (SetOutcome::NotFound, *e.parent.lock().unwrap());
            }
            (SetOutcome::NotFound, None)
        });
        match outcome {
            SetOutcome::NotFound => cur = parent,
            other => return other,
        }
    }
    SetOutcome::NotFound
}

/// Returns true if `name` is bound as a `const` in the scope chain.
pub fn is_const(heap: &Heap, env: GcIdx, name: &str) -> bool {
    let mut cur = Some(env);
    while let Some(e_idx) = cur {
        let (is_c, parent) = heap.with_obj(e_idx.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                if let Some(b) = e.vars.lock().unwrap().get(name) {
                    return (b.kind == BindingKind::Const, None);
                }
                return (false, *e.parent.lock().unwrap());
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
                return (
                    e.vars.lock().unwrap().contains_key(name),
                    *e.parent.lock().unwrap(),
                );
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
            e.vars.lock().unwrap().contains_key(name)
        } else {
            false
        }
    });
    let _ = exists;
    heap.with_obj(root.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            if e.vars.lock().unwrap().contains_key(name) {
                if let Some(b) = e.vars.lock().unwrap().get(name) {
                    *b.value.lock().unwrap() = value;
                }
            } else {
                e.vars.lock().unwrap().insert(
                    Arc::from(name),
                    crate::value::Binding {
                        value: Mutex::new(value),
                        kind: BindingKind::Var,
                        initialized: AtomicBool::new(true),
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
                return *e.parent.lock().unwrap();
            }
            None
        });
        match parent {
            Some(p) => cur = p,
            None => return cur,
        }
    }
}
