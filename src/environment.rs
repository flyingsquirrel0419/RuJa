//! Environment helpers (EnvironmentData lives in value.rs as a HeapObj variant).

use crate::gc::Heap;
use crate::value::{BindingKind, GcIdx, HeapObj, Value};
use indexmap::IndexMap;
use parking_lot::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use std::sync::Arc;

pub fn new_env(
    heap: &Heap,
    parent: Option<GcIdx>,
    is_function_scope: bool,
) -> Result<GcIdx, crate::gc::HeapLimitExceeded> {
    let env = HeapObj::Environment(crate::value::EnvironmentData {
        vars: Mutex::new(IndexMap::new()),
        parent: Mutex::new(parent),
        is_function_scope,
        with_object: Mutex::new(None),
    });
    Ok(GcIdx(heap.allocate(env)?))
}

/// Create a per-iteration child environment for a `for (let ...)` loop: copy
/// the current lexical (`let`/`const`) bindings of `env` into a fresh child
/// environment whose parent is `env`. This gives each iteration its own
/// binding so closures created in the body capture distinct values (the
/// classic `for (let i...) out.push(()=>i)` case). `var` bindings are not
/// copied (they belong to the function scope, not the loop).
pub fn clone_lexical_env(heap: &Heap, env: GcIdx) -> Result<GcIdx, crate::gc::HeapLimitExceeded> {
    // The child's parent is `env` itself. The body runs in `child` (so
    // closures capture a per-iteration binding), then the frame env is
    // restored to `env` (child's parent) before the update runs, so the
    // chain does not grow across iterations and outer scopes stay reachable.
    let child = new_env(heap, Some(env), false)?;
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            let vars = e.vars.lock();
            let cloned: Vec<(Arc<str>, crate::value::Binding)> = vars
                .iter()
                .filter(|(_, b)| b.kind != BindingKind::Var)
                .map(|(k, b)| {
                    (
                        k.clone(),
                        crate::value::Binding {
                            value: Mutex::new(b.value.lock().clone()),
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
                        ce.vars.lock().insert(k, b);
                    }
                }
            });
        }
    });
    Ok(child)
}

/// Per-iteration environment for `for (let ...)`: copy ONLY the named loop
/// variables into a fresh child env whose parent is `env`. Outer `let`s are
/// NOT copied, so mutations to them in the body persist in `env` (via the
/// chain). Each iteration's closures capture a distinct binding for the loop
/// variable while sharing the rest of the scope.
pub fn clone_loop_vars(
    heap: &Heap,
    env: GcIdx,
    names: &[Arc<str>],
) -> Result<GcIdx, crate::gc::HeapLimitExceeded> {
    let child = new_env(heap, Some(env), false)?;
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            let vars = e.vars.lock();
            let cloned: Vec<(Arc<str>, crate::value::Binding)> = vars
                .iter()
                .filter(|(k, _)| names.iter().any(|n| n.as_ref() == k.as_ref()))
                .map(|(k, b)| {
                    (
                        k.clone(),
                        crate::value::Binding {
                            value: Mutex::new(b.value.lock().clone()),
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
                        ce.vars.lock().insert(k, b);
                    }
                }
            });
        }
    });
    Ok(child)
}

/// Create a `with`-statement environment record wrapping `object`: name lookups
/// that miss the lexical chain fall back to `object`'s own properties.
pub fn new_with_env(
    heap: &Heap,
    parent: GcIdx,
    object: crate::value::Value,
) -> Result<GcIdx, crate::gc::HeapLimitExceeded> {
    let env = HeapObj::Environment(crate::value::EnvironmentData {
        vars: Mutex::new(IndexMap::new()),
        parent: Mutex::new(Some(parent)),
        is_function_scope: false,
        with_object: Mutex::new(Some(object)),
    });
    Ok(GcIdx(heap.allocate(env)?))
}

/// True if `env` has a binding for `name` that is NOT a `var` (i.e. a
/// lexical `let`/`const`). Used by direct-eval leak-back to avoid clobbering
/// an existing lexical binding when a `var` of the same name is declared in
/// eval.
pub fn has_lexical_binding(heap: &Heap, env: GcIdx, name: &str) -> bool {
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            if let Some(b) = e.vars.lock().get(name) {
                return b.kind != BindingKind::Var;
            }
        }
        false
    })
}

pub fn declare(heap: &Heap, env: GcIdx, name: &str, value: Value, kind: BindingKind) {
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            e.vars.lock().insert(
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
            e.vars.lock().insert(
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
                (e.with_object.lock().clone(), *e.parent.lock())
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
                if let Some(b) = e.vars.lock().get(name) {
                    if !b.initialized.load(Ordering::Relaxed) {
                        return (None, true, None);
                    }
                    return (Some(b.value.lock().clone()), false, None);
                }
                return (None, false, *e.parent.lock());
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
                if let Some(b) = e.vars.lock().get(name) {
                    *b.value.lock() = value.clone();
                    b.initialized.store(true, Ordering::Relaxed);
                    return (true, None);
                }
                return (false, *e.parent.lock());
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
            if let Some(b) = e.vars.lock().get(name) {
                *b.value.lock() = value;
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
            e.vars.lock().insert(
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
                if let Some(b) = e.vars.lock().get(name) {
                    return (Some(b.value.lock().clone()), None);
                }
                return (None, *e.parent.lock());
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
                if let Some(b) = e.vars.lock().get(name) {
                    return (true, b.kind == BindingKind::Const, None);
                }
                return (false, false, *e.parent.lock());
            }
            (false, false, None)
        });
        if found {
            if is_const {
                return false;
            }
            heap.with_obj(e_idx.0, |obj| {
                if let HeapObj::Environment(e) = obj {
                    if let Some(b) = e.vars.lock().get(name) {
                        *b.value.lock() = value.clone();
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
#[derive(Debug)]
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
                if let Some(b) = e.vars.lock().get(name) {
                    if !b.initialized.load(Ordering::Relaxed) {
                        return (SetOutcome::Tdz, None);
                    }
                    if b.kind == BindingKind::Const {
                        return (SetOutcome::Const, None);
                    }
                    *b.value.lock() = value.clone();
                    return (SetOutcome::Set, None);
                }
                // With environment: if the with-object has this property as a
                // data property, set it there (ES5 with-statement semantics).
                // Accessor properties are handled by the VM's property-set path.
                if let Some(crate::value::Value::Object(wi)) = e.with_object.lock().clone() {
                    let pkey = crate::value::PropertyKey::from(name);
                    let found = heap.with_obj(wi.0, |o| {
                        o.props()
                            .lock()
                            .get(&pkey)
                            .map(|d| !d.is_accessor)
                            .unwrap_or(false)
                    });
                    if found {
                        heap.with_obj(wi.0, |o| {
                            if let Some(d) = o.props().lock().get_mut(&pkey) {
                                d.value = value.clone();
                            }
                        });
                        return (SetOutcome::Set, None);
                    }
                }
                return (SetOutcome::NotFound, *e.parent.lock());
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
                if let Some(b) = e.vars.lock().get(name) {
                    return (b.kind == BindingKind::Const, None);
                }
                return (false, *e.parent.lock());
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
                return (e.vars.lock().contains_key(name), *e.parent.lock());
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

/// Try to delete a binding from the environment chain. Returns:
/// - `true` if the binding was removed or doesn't exist.
/// - `false` if the binding exists but is non-configurable (var/function).
pub fn delete_binding(heap: &Heap, env: GcIdx, name: &str) -> bool {
    let mut cur = Some(env);
    while let Some(e_idx) = cur {
        let (result, parent) = heap.with_obj(e_idx.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                // With environment: check the with-object's properties first.
                // If the with-object has the property, the delete targets it
                // (not the environment binding).
                if let Some(crate::value::Value::Object(wi)) = e.with_object.lock().clone() {
                    let pkey = crate::value::PropertyKey::from(name);
                    let has_prop = heap.with_obj(wi.0, |o| o.props().lock().contains_key(&pkey));
                    if has_prop {
                        // Delete from the with-object
                        let deleted = heap.with_obj(wi.0, |o| {
                            let mut props = o.props().lock();
                            if let Some(d) = props.get(&pkey) {
                                if d.configurable {
                                    props.shift_remove(&pkey);
                                    true
                                } else {
                                    false
                                }
                            } else {
                                true
                            }
                        });
                        return (deleted, None);
                    }
                }
                let mut vars = e.vars.lock();
                if let Some(b) = vars.get(name) {
                    // var and function declarations are non-configurable
                    if b.kind == BindingKind::Var {
                        return (false, None);
                    }
                    // let/const can be deleted (they're block-scoped)
                    vars.shift_remove(name);
                    return (true, None);
                }
                return (true, *e.parent.lock());
            }
            (true, None)
        });
        if !result {
            return false;
        }
        match parent {
            Some(p) => cur = Some(p),
            None => return true,
        }
    }
    true
}

/// Ensure a `var` binding exists in the function-scope root (variable
/// environment), creating it as `undefined` if not already present. Unlike
/// `declare_var`, this does NOT set a value — it only creates the hoisted
/// binding so that identifier resolution finds it before reaching a
/// `with`-object. Used by `Op::DeclareVar` before `set_checked`.
pub fn ensure_var(heap: &Heap, env: GcIdx, name: &str) {
    let root = function_scope_root(heap, env);
    let exists = heap.with_obj(root.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            e.vars.lock().contains_key(name)
        } else {
            false
        }
    });
    if !exists {
        heap.with_obj(root.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                e.vars.lock().insert(
                    Arc::from(name),
                    crate::value::Binding {
                        value: Mutex::new(Value::Undefined),
                        kind: BindingKind::Var,
                        initialized: AtomicBool::new(true),
                    },
                );
            }
        });
    }
}

pub fn declare_var(heap: &Heap, env: GcIdx, name: &str, value: Value) {
    // Always declare/hoist at the function-scope root first.
    let root = function_scope_root(heap, env);
    let already_exists = heap.with_obj(root.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            e.vars.lock().contains_key(name)
        } else {
            false
        }
    });
    if !already_exists {
        heap.with_obj(root.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                e.vars.lock().insert(
                    Arc::from(name),
                    crate::value::Binding {
                        value: Mutex::new(Value::Undefined),
                        kind: BindingKind::Var,
                        initialized: AtomicBool::new(true),
                    },
                );
            }
        });
    }
    // Set at function-scope root. var declarations always create/update
    // the function-scoped binding, not the with object's property.
    // (Per spec, var declarations use the variable environment, not the
    // lexical environment that `with` modifies.)
    // Check existence first (drop the borrow) before mutating.
    let exists = heap.with_obj(root.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            e.vars.lock().contains_key(name)
        } else {
            false
        }
    });
    let _ = exists;
    heap.with_obj(root.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            if e.vars.lock().contains_key(name) {
                if let Some(b) = e.vars.lock().get(name) {
                    *b.value.lock() = value;
                }
            } else {
                e.vars.lock().insert(
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
                return *e.parent.lock();
            }
            None
        });
        match parent {
            Some(p) => cur = p,
            None => return cur,
        }
    }
}
