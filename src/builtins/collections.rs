use super::*;

// Map
// =========================================================================
pub(crate) fn map_set(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let val = args.get(1).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries.lock().insert(MapKey(key), val);
            }
        });
    }
    Ok(this.unwrap_or(Value::Undefined))
}
pub(crate) fn map_get(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries
                    .lock()
                    .get(&MapKey(key))
                    .cloned()
                    .unwrap_or(Value::Undefined)
            } else {
                Value::Undefined
            }
        }));
    }
    Ok(Value::Undefined)
}
pub(crate) fn map_has(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries
                    .lock()
                    .contains_key(&MapKey(key))
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}
pub(crate) fn map_delete(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries.lock().shift_remove(&MapKey(key)).is_some()
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}

// --- WeakMap / WeakSet (true weak-reference semantics) ---

pub(crate) fn weakmap_constructor(vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    // The WeakMap prototype (with get/set/has/delete) is the constructor's
    // own `.prototype` property. `construct` passes a fresh Object whose
    // [[Prototype]] is that prototype as `this`; copy it so the returned
    // WeakMap object inherits the methods.
    let proto = match _this {
        Some(Value::Object(idx)) => vm
            .heap
            .with_obj(idx.0, |o| o.proto().lock().clone()),
        _ => Some(vm.object_proto.clone()),
    };
    let obj_idx = vm
        .heap
        .allocate(HeapObj::WeakMap(crate::value::WeakMapData {
            entries: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(proto),
        }));
    Ok(Value::Object(GcIdx(obj_idx)))
}

pub(crate) fn weakmap_set(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let val = args.get(1).cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => {
            return Err(Error::type_err(
                "Invalid value used as weak map key".to_string(),
            ))
        }
    };
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakMap(wm) = obj {
                let mut entries = wm.entries.lock();
                if let Some(slot) = entries.iter_mut().find(|(k, _)| *k == key_idx) {
                    slot.1 = val;
                } else {
                    entries.push((key_idx, val));
                }
            }
        });
    }
    Ok(this.unwrap_or(Value::Undefined))
}

pub(crate) fn weakmap_get(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => return Ok(Value::Undefined),
    };
    if let Some(Value::Object(idx)) = this {
        return Ok(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakMap(wm) = obj {
                wm.entries
                    .lock()
                    .iter()
                    .find(|(k, _)| *k == key_idx)
                    .map(|(_, v)| v.clone())
                    .unwrap_or(Value::Undefined)
            } else {
                Value::Undefined
            }
        }));
    }
    Ok(Value::Undefined)
}

pub(crate) fn weakmap_has(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => return Ok(Value::Bool(false)),
    };
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakMap(wm) = obj {
                wm.entries
                    .lock()
                    .iter()
                    .any(|(k, _)| *k == key_idx)
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}

pub(crate) fn weakmap_delete(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => return Ok(Value::Bool(false)),
    };
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakMap(wm) = obj {
                let mut entries = wm.entries.lock();
                let len = entries.len();
                entries.retain(|(k, _)| *k != key_idx);
                entries.len() != len
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}

pub(crate) fn weakset_constructor(vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let proto = match _this {
        Some(Value::Object(idx)) => vm
            .heap
            .with_obj(idx.0, |o| o.proto().lock().clone()),
        _ => Some(vm.object_proto.clone()),
    };
    let obj_idx = vm
        .heap
        .allocate(HeapObj::WeakSet(crate::value::WeakSetData {
            items: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(proto),
        }));
    Ok(Value::Object(GcIdx(obj_idx)))
}

pub(crate) fn weakset_add(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => {
            return Err(Error::type_err(
                "Invalid value used in weak set".to_string(),
            ))
        }
    };
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakSet(ws) = obj {
                let mut items = ws.items.lock();
                if !items.contains(&key_idx) {
                    items.push(key_idx);
                }
            }
        });
    }
    Ok(this.unwrap_or(Value::Undefined))
}

pub(crate) fn weakset_has(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => return Ok(Value::Bool(false)),
    };
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakSet(ws) = obj {
                ws.items.lock().contains(&key_idx)
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}

pub(crate) fn weakset_delete(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => return Ok(Value::Bool(false)),
    };
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakSet(ws) = obj {
                let mut items = ws.items.lock();
                let len = items.len();
                items.retain(|k| *k != key_idx);
                items.len() != len
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}
pub(crate) fn map_clear(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries.lock().clear();
            }
        });
    }
    Ok(Value::Undefined)
}
pub(crate) fn map_size(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Number(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries.lock().len()
            } else {
                0
            }
        }) as f64));
    }
    Ok(Value::Number(0.0))
}
/// Collect Map entries as [key, value] arrays.
pub(crate) fn map_entries_list(vm: &mut Vm, this: &Option<Value>) -> Vec<Value> {
    if let Some(Value::Object(idx)) = this {
        let pairs: Vec<(Value, Value)> = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries
                    .lock()
                    .iter()
                    .map(|(k, v)| (k.0.clone(), v.clone()))
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        });
        pairs
            .into_iter()
            .map(|(k, v)| make_value_array(vm, vec![k, v]))
            .collect()
    } else {
        Vec::new()
    }
}
pub(crate) fn map_entries(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let pairs = map_entries_list(vm, &this);
    Ok(make_value_array(vm, pairs))
}
pub(crate) fn map_keys(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let keys: Vec<Value> = if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries
                    .lock()
                    .iter()
                    .map(|(k, _)| k.0.clone())
                    .collect()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    Ok(make_value_array(vm, keys))
}
pub(crate) fn map_values(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let vals: Vec<Value> = if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries
                    .lock()
                    .values()
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    Ok(make_value_array(vm, vals))
}
pub(crate) fn map_for_each(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    let this_arg = args.get(1).cloned();
    if let Some(Value::Object(idx)) = this {
        let pairs: Vec<(Value, Value)> = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries
                    .lock()
                    .iter()
                    .map(|(k, v)| (k.0.clone(), v.clone()))
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        });
        for (k, v) in &pairs {
            vm.call_function(
                &cb,
                &[
                    v.clone(),
                    k.clone(),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                this_arg.clone(),
            )?;
        }
    }
    Ok(Value::Undefined)
}
pub(crate) fn map_constructor(vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let obj_idx = vm.heap.allocate(HeapObj::Map(MapData {
        entries: Mutex::new(IndexMap::new()),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.map_proto.clone())),
    }));
    // Initialize from an optional iterable of [key, value] pairs.
    if let Some(iterable) = _args.first() {
        if !iterable.is_undefined() && !iterable.is_null() {
            let it = vm.make_iterator(iterable)?;
            loop {
                let (pair, done) = vm.iterator_next(&it)?;
                if done {
                    break;
                }
                let (k, v) = if let Value::Object(pi) = &pair {
                    vm.heap.with_obj(pi.0, |o| {
                        if let HeapObj::Array(a) = o {
                            let it2 = a.items.lock();
                            (
                                it2.first().cloned().unwrap_or(Value::Undefined),
                                it2.get(1).cloned().unwrap_or(Value::Undefined),
                            )
                        } else {
                            (Value::Undefined, Value::Undefined)
                        }
                    })
                } else {
                    (Value::Undefined, Value::Undefined)
                };
                vm.heap.with_obj(obj_idx, |o| {
                    if let HeapObj::Map(m) = o {
                        m.entries.lock().insert(MapKey(k), v);
                    }
                });
            }
        }
    }
    Ok(Value::Object(GcIdx(obj_idx)))
}

// =========================================================================
// Set
// =========================================================================
pub(crate) fn set_add(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj {
                s.items.lock().insert(MapKey(val));
            }
        });
    }
    Ok(this.unwrap_or(Value::Undefined))
}
pub(crate) fn set_has(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj {
                s.items.lock().contains(&MapKey(val))
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}
pub(crate) fn set_delete(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj {
                s.items.lock().shift_remove(&MapKey(val))
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}
pub(crate) fn set_size(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Number(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj {
                s.items.lock().len()
            } else {
                0
            }
        }) as f64));
    }
    Ok(Value::Number(0.0))
}
pub(crate) fn set_values_list(vm: &mut Vm, this: &Option<Value>) -> Vec<Value> {
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj {
                s.items
                    .lock()
                    .iter()
                    .map(|k| k.0.clone())
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    }
}
pub(crate) fn set_entries(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let vals = set_values_list(vm, &this);
    let mut pairs: Vec<Value> = Vec::new();
    for v in vals {
        pairs.push(make_value_array(vm, vec![v.clone(), v]));
    }
    Ok(make_value_array(vm, pairs))
}
pub(crate) fn set_keys(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let vals = set_values_list(vm, &this);
    Ok(make_value_array(vm, vals))
}
pub(crate) fn set_values(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let vals = set_values_list(vm, &this);
    Ok(make_value_array(vm, vals))
}
pub(crate) fn set_for_each(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    let this_arg = args.get(1).cloned();
    let vals = set_values_list(vm, &this);
    for v in &vals {
        vm.call_function(
            &cb,
            &[
                v.clone(),
                v.clone(),
                this.clone().unwrap_or(Value::Undefined),
            ],
            this_arg.clone(),
        )?;
    }
    Ok(Value::Undefined)
}
pub(crate) fn set_constructor(vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let obj_idx = vm.heap.allocate(HeapObj::Set(SetData {
        items: Mutex::new(IndexSet::new()),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.set_proto.clone())),
    }));
    // Initialize from an optional iterable.
    if let Some(iterable) = _args.first() {
        if !iterable.is_undefined() && !iterable.is_null() {
            let it = vm.make_iterator(iterable)?;
            loop {
                let (v, done) = vm.iterator_next(&it)?;
                if done {
                    break;
                }
                vm.heap.with_obj(obj_idx, |o| {
                    if let HeapObj::Set(s) = o {
                        s.items.lock().insert(MapKey(v));
                    }
                });
            }
        }
    }
    Ok(Value::Object(GcIdx(obj_idx)))
}

// =========================================================================
// Symbol
// =========================================================================
pub(crate) fn symbol_constructor(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let _desc = args.first().cloned().unwrap_or(Value::Undefined);
    let id = vm.next_symbol_id;
    vm.next_symbol_id += 1;
    Ok(Value::Symbol(id))
}
pub(crate) fn symbol_for(vm: &mut Vm, _args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let id = vm.next_symbol_id;
    vm.next_symbol_id += 1;
    Ok(Value::Symbol(id))
}
pub(crate) fn symbol_to_string(_vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    // RuJa's Symbol is `Value::Symbol(u32)` with no stored description, so
    // we return the no-description form "Symbol()".
    Ok(Value::String(Arc::from("Symbol()")))
}

// =========================================================================
// Extended setup 2: Map/Set/Symbol
// =========================================================================

// =========================================================================
// Promise
// =========================================================================
pub(crate) fn promise_constructor(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let executor = args.first().cloned().unwrap_or(Value::Undefined);
    // create the promise object
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: Mutex::new(crate::value::PromiseStatus::Pending),
            result: Mutex::new(Value::Undefined),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.promise_proto.clone())),
        }));
    let p_val = Value::Object(GcIdx(p_idx));
    // create resolve/reject native functions bound via `this` = promise
    let resolve_target = vm.new_native_function("resolve", promise_resolve, 1);
    let reject_target = vm.new_native_function("reject", promise_reject, 1);
    // Wrap as bound functions with  = the promise, so resolve/reject know
    // which promise to settle.
    let resolve_fn = vm
        .heap
        .allocate(HeapObj::Function(crate::value::FunctionData {
            name: Some(Arc::from("resolve")),
            kind: crate::value::FunctionKind::Bound {
                target: resolve_target,
                this_val: p_val.clone(),
                bound_args: Vec::new(),
            },
            closure: vm.global,
            prototype: Mutex::new(None),
            proto: Mutex::new(match vm.function_proto {
                Value::Object(_) => Some(vm.function_proto.clone()),
                _ => None,
            }),
            props: Mutex::new(IndexMap::new()),
        }));
    let reject_fn = vm
        .heap
        .allocate(HeapObj::Function(crate::value::FunctionData {
            name: Some(Arc::from("reject")),
            kind: crate::value::FunctionKind::Bound {
                target: reject_target,
                this_val: p_val.clone(),
                bound_args: Vec::new(),
            },
            closure: vm.global,
            prototype: Mutex::new(None),
            proto: Mutex::new(match vm.function_proto {
                Value::Object(_) => Some(vm.function_proto.clone()),
                _ => None,
            }),
            props: Mutex::new(IndexMap::new()),
        }));
    match vm.call_function(
        &executor,
        &[
            Value::Object(GcIdx(resolve_fn)),
            Value::Object(GcIdx(reject_fn)),
        ],
        Some(p_val.clone()),
    ) {
        Ok(_) => {}
        Err(e) => {
            // executor threw: reject the promise with the thrown value
            let reason: Value = e
                .thrown_value
                .clone()
                .unwrap_or_else(|| Value::String(Arc::from(e.message.as_str())));
            vm.promise_reject(p_idx, reason);
        }
    }
    Ok(p_val)
}

pub(crate) fn promise_resolve(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let p_idx = match &this {
        Some(Value::Object(idx)) => idx.0,
        _ => return Ok(Value::Undefined),
    };
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    vm.promise_resolve(p_idx, value);
    Ok(Value::Undefined)
}
pub(crate) fn promise_reject(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let p_idx = match &this {
        Some(Value::Object(idx)) => idx.0,
        _ => return Ok(Value::Undefined),
    };
    let reason = args.first().cloned().unwrap_or(Value::Undefined);
    vm.promise_reject(p_idx, reason);
    Ok(Value::Undefined)
}

/// `Promise.resolve(v)`: returns a promise resolved with `v`. If `v` is already
/// a promise, it is returned as-is (simplified adoption).
pub(crate) fn promise_static_resolve(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &value {
        let is_promise = vm
            .heap
            .with_obj(idx.0, |o| matches!(o, HeapObj::Promise(_)));
        if is_promise {
            return Ok(value);
        }
    }
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: Mutex::new(crate::value::PromiseStatus::Fulfilled),
            result: Mutex::new(value),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.promise_proto.clone())),
        }));
    Ok(Value::Object(GcIdx(p_idx)))
}

/// `Promise.reject(r)`: returns a promise rejected with `r`.
pub(crate) fn promise_static_reject(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let reason = args.first().cloned().unwrap_or(Value::Undefined);
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: Mutex::new(crate::value::PromiseStatus::Rejected),
            result: Mutex::new(reason),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.promise_proto.clone())),
        }));
    Ok(Value::Object(GcIdx(p_idx)))
}

pub(crate) fn promise_then(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let on_fulfilled = args.first().cloned().unwrap_or(Value::Undefined);
    let on_rejected = args.get(1).cloned().unwrap_or(Value::Undefined);
    let p_idx = match &this {
        Some(Value::Object(idx)) => idx.0,
        _ => return Err(Error::type_err("then called on non-promise".to_string())),
    };
    // Create a derived promise that settles with the handler's result.
    let derived = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: Mutex::new(crate::value::PromiseStatus::Pending),
            result: Mutex::new(Value::Undefined),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.promise_proto.clone())),
        }));
    let (state, _result) = vm.heap.with_obj(p_idx, |o| {
        if let HeapObj::Promise(p) = o {
            (*p.state.lock(), p.result.lock().clone())
        } else {
            (crate::value::PromiseStatus::Fulfilled, Value::Undefined)
        }
    });
    let handler = crate::value::PromiseHandler {
        on_fulfilled: on_fulfilled.clone(),
        on_rejected: on_rejected.clone(),
        derived: Some(GcIdx(derived)),
    };
    match state {
        crate::value::PromiseStatus::Pending => {
            vm.heap.with_obj(p_idx, |o| {
                if let HeapObj::Promise(p) = o {
                    p.handlers.lock().push(handler);
                }
            });
        }
        _ => {
            // already settled: schedule immediately, passing derived for chaining
            vm.microtask_queue.push_back(crate::vm::Microtask::Then {
                promise: GcIdx(p_idx),
                on_fulfilled,
                on_rejected,
                derived: Some(GcIdx(derived)),
            });
        }
    }
    Ok(Value::Object(GcIdx(derived)))
}

pub(crate) fn promise_catch(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    // p.catch(r) === p.then(undefined, r)
    let on_rejected = args.first().cloned().unwrap_or(Value::Undefined);
    promise_then(vm, &[Value::Undefined, on_rejected], this)
}

// =========================================================================
