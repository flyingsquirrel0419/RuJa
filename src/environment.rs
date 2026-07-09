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
                            deletable: b.deletable,
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
                            deletable: b.deletable,
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

/// Clone loop variables from the current per-iteration env into a fresh
/// sibling env. The sibling has the same parent as `env`, so repeated
/// `for (let ...)` iterations do not grow the environment chain.
pub fn clone_loop_vars_to_sibling(
    heap: &Heap,
    env: GcIdx,
    names: &[Arc<str>],
) -> Result<GcIdx, crate::gc::HeapLimitExceeded> {
    let parent = heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            *e.parent.lock()
        } else {
            None
        }
    });
    let sibling = new_env(heap, parent, false)?;
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
                            deletable: b.deletable,
                        },
                    )
                })
                .collect();
            drop(vars);
            heap.with_obj(sibling.0, |sobj| {
                if let HeapObj::Environment(se) = sobj {
                    for (k, b) in cloned {
                        se.vars.lock().insert(k, b);
                    }
                }
            });
        }
    });
    Ok(sibling)
}

/// Create a `with`-statement environment record wrapping `object`: name lookups
/// that miss lexical bindings fall back to `object`'s [[HasProperty]] result.
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

pub fn has_lexical_declaration_between(
    heap: &Heap,
    env: GcIdx,
    stop_env: GcIdx,
    name: &str,
) -> bool {
    let mut cur = Some(env);
    while let Some(e_idx) = cur {
        let (found, parent) = heap.with_obj(e_idx.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                if let Some(b) = e.vars.lock().get(name) {
                    if matches!(b.kind, BindingKind::Let | BindingKind::Const) {
                        return (true, None);
                    }
                }
                return (false, *e.parent.lock());
            }
            (false, None)
        });
        if found {
            return true;
        }
        if e_idx == stop_env {
            return false;
        }
        cur = parent;
    }
    false
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
                    deletable: false,
                },
            );
        }
    });
}

pub fn own_bindings(heap: &Heap, env: GcIdx) -> Vec<(Arc<str>, Value)> {
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            return e
                .vars
                .lock()
                .iter()
                .map(|(name, binding)| (name.clone(), binding.value.lock().clone()))
                .collect();
        }
        Vec::new()
    })
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
                    deletable: false,
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

pub fn find_binding_env(heap: &Heap, env: GcIdx, name: &str) -> Option<GcIdx> {
    let mut cur = Some(env);
    while let Some(e_idx) = cur {
        let (found, parent) = heap.with_obj(e_idx.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                if e.vars.lock().contains_key(name) {
                    return (true, None);
                }
                return (false, *e.parent.lock());
            }
            (false, None)
        });
        if found {
            return Some(e_idx);
        }
        cur = parent;
    }
    None
}

pub fn binding_initialized(heap: &Heap, env: GcIdx, name: &str) -> Option<bool> {
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            return e
                .vars
                .lock()
                .get(name)
                .map(|b| b.initialized.load(Ordering::Relaxed));
        }
        None
    })
}

pub fn bind_this_value(heap: &Heap, env: GcIdx, value: Value) -> crate::error::Result<()> {
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            if let Some(b) = e.vars.lock().get("this") {
                if b.initialized.load(Ordering::Relaxed) {
                    return Err(crate::error::Error::reference(
                        "super() has already been called",
                    ));
                }
                *b.value.lock() = value;
                b.initialized.store(true, Ordering::Relaxed);
                return Ok(());
            }
        }
        Err(crate::error::Error::reference(
            "super() called outside derived constructor",
        ))
    })
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
                    deletable: false,
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
                    return (
                        true,
                        matches!(b.kind, BindingKind::Const | BindingKind::FunctionName),
                        None,
                    );
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
    FunctionName,
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
                    if b.kind == BindingKind::FunctionName {
                        return (SetOutcome::FunctionName, None);
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

pub fn has_own_binding(heap: &Heap, env: GcIdx, name: &str) -> bool {
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            return e.vars.lock().contains_key(name);
        }
        false
    })
}

pub fn binding_env_and_kind(heap: &Heap, env: GcIdx, name: &str) -> Option<(GcIdx, BindingKind)> {
    let mut cur = Some(env);
    while let Some(e_idx) = cur {
        let (found, parent) = heap.with_obj(e_idx.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                if let Some(b) = e.vars.lock().get(name) {
                    return (Some(b.kind), None);
                }
                return (None, *e.parent.lock());
            }
            (None, None)
        });
        if let Some(kind) = found {
            return Some((e_idx, kind));
        }
        cur = parent;
    }
    None
}

/// Try to delete a binding from the environment chain. Returns:
/// - `true` if the binding was removed or doesn't exist.
/// - `false` if a declarative binding exists; ordinary var, parameter,
///   lexical, and catch bindings are non-configurable.
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
                if let Some(binding) = vars.get(name) {
                    if binding.deletable {
                        vars.shift_remove(name);
                        return (true, None);
                    }
                    return (false, None);
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
    ensure_var_with_deletable(heap, env, name, false);
}

pub fn ensure_var_with_deletable(heap: &Heap, env: GcIdx, name: &str, deletable: bool) {
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
                        deletable,
                    },
                );
            }
        });
    }
}

pub fn declare_var(heap: &Heap, env: GcIdx, name: &str, value: Value) {
    declare_var_with_deletable(heap, env, name, value, false);
}

pub fn declare_var_with_deletable(
    heap: &Heap,
    env: GcIdx,
    name: &str,
    value: Value,
    deletable: bool,
) {
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
                        deletable,
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
                        deletable,
                    },
                );
            }
        }
    });
}

/// Force-delete a var binding from a specific environment record.
/// Used by `delete x` in non-strict mode for implicit globals (which per
/// spec should be configurable properties on the global object).
pub fn delete_var_binding(heap: &Heap, env: GcIdx, name: &str) -> bool {
    heap.with_obj(env.0, |obj| {
        if let HeapObj::Environment(e) = obj {
            let mut vars = e.vars.lock();
            if vars.contains_key(name) {
                vars.shift_remove(name);
                return true;
            }
        }
        false
    })
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

pub fn global_env_root(heap: &Heap, env: GcIdx) -> GcIdx {
    let mut cur = env;
    loop {
        let parent = heap.with_obj(cur.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                *e.parent.lock()
            } else {
                None
            }
        });
        match parent {
            Some(p) => cur = p,
            None => return cur,
        }
    }
}
