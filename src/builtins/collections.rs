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
                m.entries.lock().contains_key(&MapKey(key))
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

pub(crate) fn weakmap_constructor(
    vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    // The WeakMap prototype (with get/set/has/delete) is the constructor's
    // own `.prototype` property. `construct` passes a fresh Object whose
    // [[Prototype]] is that prototype as `this`; copy it so the returned
    // WeakMap object inherits the methods.
    let proto = match _this {
        Some(Value::Object(idx)) => vm.heap.with_obj(idx.0, |o| o.proto().lock().clone()),
        _ => Some(vm.object_proto.clone()),
    };
    let obj_idx = vm
        .heap
        .allocate(HeapObj::WeakMap(crate::value::WeakMapData {
            entries: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(proto),
        }))?;
    Ok(Value::Object(GcIdx(obj_idx)))
}

pub(crate) fn weakmap_set(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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

pub(crate) fn weakmap_get(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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

pub(crate) fn weakmap_has(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => return Ok(Value::Bool(false)),
    };
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakMap(wm) = obj {
                wm.entries.lock().iter().any(|(k, _)| *k == key_idx)
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}

pub(crate) fn weakmap_delete(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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

pub(crate) fn weakset_constructor(
    vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let proto = match _this {
        Some(Value::Object(idx)) => vm.heap.with_obj(idx.0, |o| o.proto().lock().clone()),
        _ => Some(vm.object_proto.clone()),
    };
    let obj_idx = vm
        .heap
        .allocate(HeapObj::WeakSet(crate::value::WeakSetData {
            items: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(proto),
        }))?;
    Ok(Value::Object(GcIdx(obj_idx)))
}

pub(crate) fn weakset_add(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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

pub(crate) fn weakset_has(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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

pub(crate) fn weakset_delete(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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
pub(crate) fn map_entries_list(vm: &mut Vm, this: &Option<Value>) -> error::Result<Vec<Value>> {
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
        let mut out = Vec::with_capacity(pairs.len());
        for (k, v) in pairs {
            out.push(make_value_array(vm, vec![k, v])?);
        }
        Ok(out)
    } else {
        Ok(Vec::new())
    }
}
pub(crate) fn map_entries(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let pairs = map_entries_list(vm, &this)?;
    make_value_array(vm, pairs)
}
pub(crate) fn map_keys(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let keys: Vec<Value> = if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries.lock().iter().map(|(k, _)| k.0.clone()).collect()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    make_value_array(vm, keys)
}
pub(crate) fn map_values(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let vals: Vec<Value> = if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries.lock().values().cloned().collect()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    make_value_array(vm, vals)
}
pub(crate) fn map_for_each(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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
pub(crate) fn map_constructor(
    vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let obj_idx = vm.heap.allocate(HeapObj::Map(MapData {
        entries: Mutex::new(IndexMap::new()),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.map_proto.clone())),
    }))?;
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
pub(crate) fn set_entries(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let vals = set_values_list(vm, &this);
    let mut pairs: Vec<Value> = Vec::new();
    for v in vals {
        pairs.push(make_value_array(vm, vec![v.clone(), v])?);
    }
    make_value_array(vm, pairs)
}
pub(crate) fn set_keys(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let vals = set_values_list(vm, &this);
    make_value_array(vm, vals)
}
pub(crate) fn set_values(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let vals = set_values_list(vm, &this);
    make_value_array(vm, vals)
}
pub(crate) fn set_for_each(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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
pub(crate) fn set_constructor(
    vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let obj_idx = vm.heap.allocate(HeapObj::Set(SetData {
        items: Mutex::new(IndexSet::new()),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.set_proto.clone())),
    }))?;
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
pub(crate) fn symbol_constructor(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
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
pub(crate) fn symbol_to_string(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
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
pub(crate) fn promise_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target.is_none() {
        return Err(Error::type_err(
            "Promise constructor must be called with new",
        ));
    }
    let executor = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&executor, &vm.heap) {
        return Err(Error::type_err("Promise resolver is not a function"));
    }
    let proto = native_constructor_prototype(vm, vm.promise_proto.clone())?;
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: Mutex::new(crate::value::PromiseStatus::Pending),
            result: Mutex::new(Value::Undefined),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
        }))?;
    let p_val = Value::Object(GcIdx(p_idx));
    let resolve_fn = create_promise_resolving_function(vm, p_val.clone(), promise_resolve)?;
    let pins = vm.pin_many(&[p_val.clone(), resolve_fn.clone()]);
    let reject_fn = create_promise_resolving_function(vm, p_val.clone(), promise_reject);
    vm.unpin_many(pins);
    let reject_fn = reject_fn?;
    match vm.call_function(&executor, &[resolve_fn, reject_fn], Some(Value::Undefined)) {
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

fn create_bound_native_function(
    vm: &mut Vm,
    name: &str,
    target_name: &str,
    func: NativeFn,
    length: usize,
    this_val: Value,
) -> error::Result<Value> {
    let target = vm.new_native_function(target_name, func, length)?;
    let target_val = Value::Object(target);
    let pins = vm.pin_many(&[target_val, this_val.clone()]);
    let idx = vm.heap.allocate(HeapObj::Function(FunctionData {
        name: Some(Arc::from(name)),
        kind: FunctionKind::Bound {
            target,
            this_val,
            bound_args: Vec::new(),
        },
        closure: vm.global,
        lexical_new_target: Value::Undefined,
        is_class_ctor: AtomicBool::new(false),
        prototype: Mutex::new(None),
        proto: Mutex::new(match vm.function_proto {
            Value::Object(_) => Some(vm.function_proto.clone()),
            _ => None,
        }),
        props: Mutex::new(builtin_function_own_props(name, length)),
        extensible: AtomicBool::new(true),
        private_fields: Mutex::new(std::collections::HashMap::new()),
    }));
    vm.unpin_many(pins);
    let idx = idx?;
    Ok(Value::Object(GcIdx(idx)))
}

fn create_promise_resolving_function(
    vm: &mut Vm,
    promise: Value,
    func: NativeFn,
) -> error::Result<Value> {
    create_bound_native_function(vm, "", "", func, 1, promise)
}

struct PromiseCapability {
    promise: Value,
    resolve: Value,
    reject: Value,
}

pub(crate) fn promise_capability_executor(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let capability_obj = match this {
        Some(Value::Object(idx)) => idx,
        _ => return Err(Error::type_err("Promise capability executor receiver")),
    };
    let resolve = args.first().cloned().unwrap_or(Value::Undefined);
    let reject = args.get(1).cloned().unwrap_or(Value::Undefined);
    vm.heap.with_obj(capability_obj.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        let existing_resolve = props
            .get(&PropertyKey::from("resolve"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let existing_reject = props
            .get(&PropertyKey::from("reject"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        if !existing_resolve.is_undefined() || !existing_reject.is_undefined() {
            return Err(Error::type_err("Promise capability already resolved"));
        }
        props.insert(
            PropertyKey::from("resolve"),
            PropertyDescriptor::data(resolve),
        );
        props.insert(
            PropertyKey::from("reject"),
            PropertyDescriptor::data(reject),
        );
        Ok(Value::Undefined)
    })
}

fn new_promise_capability(vm: &mut Vm, ctor: Value) -> error::Result<PromiseCapability> {
    if !vm.is_constructor_value(&ctor) {
        return Err(Error::type_err(
            "Promise capability receiver is not a constructor",
        ));
    }

    let capability_idx = vm.new_object()?;
    let capability = Value::Object(capability_idx);
    let executor = create_bound_native_function(
        vm,
        "",
        "",
        promise_capability_executor,
        2,
        capability.clone(),
    )?;
    let pins = vm.pin_many(&[ctor.clone(), capability.clone(), executor.clone()]);
    let promise_result = vm.construct(&ctor, std::slice::from_ref(&executor));
    let promise = match promise_result {
        Ok(promise) => promise,
        Err(err) => {
            vm.unpin_many(pins);
            return Err(err);
        }
    };
    let (resolve, reject) = vm.heap.with_obj(capability_idx.0, |obj| {
        let props = obj.props();
        let props = props.lock();
        let resolve = props
            .get(&PropertyKey::from("resolve"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let reject = props
            .get(&PropertyKey::from("reject"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        (resolve, reject)
    });
    vm.unpin_many(pins);

    if !is_callable(&resolve, &vm.heap) || !is_callable(&reject, &vm.heap) {
        return Err(Error::type_err(
            "Promise capability functions are not callable",
        ));
    }
    Ok(PromiseCapability {
        promise,
        resolve,
        reject,
    })
}

pub(crate) fn promise_resolve(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let p_idx = match &this {
        Some(Value::Object(idx)) => idx.0,
        _ => return Ok(Value::Undefined),
    };
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    vm.promise_resolve(p_idx, value);
    Ok(Value::Undefined)
}
pub(crate) fn promise_reject(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let p_idx = match &this {
        Some(Value::Object(idx)) => idx.0,
        _ => return Ok(Value::Undefined),
    };
    let reason = args.first().cloned().unwrap_or(Value::Undefined);
    vm.promise_reject(p_idx, reason);
    Ok(Value::Undefined)
}

/// `Promise.resolve(v)`: create a promise capability from the receiver
/// constructor and resolve it with `v`.
pub(crate) fn promise_static_resolve(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &value {
        let is_promise = vm
            .heap
            .with_obj(idx.0, |o| matches!(o, HeapObj::Promise(_)));
        if is_promise {
            let value_constructor =
                vm.get_property_by_key(&value, &PropertyKey::from("constructor"))?;
            if value_constructor == ctor {
                return Ok(value);
            }
        }
    }
    let capability = new_promise_capability(vm, ctor)?;
    let pins = vm.pin_many(&[
        capability.promise.clone(),
        capability.resolve.clone(),
        value.clone(),
    ]);
    let result = vm.call_function(
        &capability.resolve,
        std::slice::from_ref(&value),
        Some(Value::Undefined),
    );
    vm.unpin_many(pins);
    result?;
    Ok(capability.promise)
}

/// `Promise.reject(r)`: returns a promise rejected with `r`.
pub(crate) fn promise_static_reject(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let reason = args.first().cloned().unwrap_or(Value::Undefined);
    let capability = new_promise_capability(vm, ctor)?;
    let pins = vm.pin_many(&[
        capability.promise.clone(),
        capability.reject.clone(),
        reason.clone(),
    ]);
    let result = vm.call_function(
        &capability.reject,
        std::slice::from_ref(&reason),
        Some(Value::Undefined),
    );
    vm.unpin_many(pins);
    result?;
    Ok(capability.promise)
}

fn make_pending_promise(vm: &mut Vm) -> error::Result<Value> {
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: Mutex::new(crate::value::PromiseStatus::Pending),
            result: Mutex::new(Value::Undefined),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.promise_proto.clone())),
        }))?;
    Ok(Value::Object(GcIdx(p_idx)))
}

fn make_fulfilled_promise(vm: &mut Vm, value: Value) -> error::Result<Value> {
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: Mutex::new(crate::value::PromiseStatus::Fulfilled),
            result: Mutex::new(value),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.promise_proto.clone())),
        }))?;
    Ok(Value::Object(GcIdx(p_idx)))
}

fn make_rejected_promise(vm: &mut Vm, reason: Value) -> error::Result<Value> {
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: Mutex::new(crate::value::PromiseStatus::Rejected),
            result: Mutex::new(reason),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.promise_proto.clone())),
        }))?;
    Ok(Value::Object(GcIdx(p_idx)))
}

fn promise_rejection_value(err: &Arc<error::Error>) -> Value {
    err.thrown_value
        .clone()
        .unwrap_or_else(|| Value::String(Arc::from(err.message.as_str())))
}

fn make_aggregate_error(vm: &mut Vm, errors: Value) -> error::Result<Value> {
    let proto = match env::get(&vm.heap, vm.global, "AggregateError") {
        Some(Value::Object(ctor)) => vm.heap.with_obj(ctor.0, |obj| {
            obj.props()
                .lock()
                .get(&PropertyKey::from("prototype"))
                .map(|desc| desc.value.clone())
        }),
        _ => None,
    }
    .filter(|value| matches!(value, Value::Object(_)))
    .unwrap_or_else(|| vm.error_proto.clone());

    let idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(proto)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Error")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let idx = GcIdx(idx);
    vm.heap.with_obj(idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("name"),
            data_prop(Value::String(Arc::from("AggregateError"))),
        );
        props.insert(PropertyKey::from("errors"), data_prop(errors));
    });
    Ok(Value::Object(idx))
}

fn call_promise_capability_function(
    vm: &mut Vm,
    function: &Value,
    value: Value,
) -> error::Result<Value> {
    let pins = vm.pin_many(&[function.clone(), value.clone()]);
    let result = vm.call_function(
        function,
        std::slice::from_ref(&value),
        Some(Value::Undefined),
    );
    vm.unpin_many(pins);
    result
}

fn reject_promise_capability(
    vm: &mut Vm,
    capability: &PromiseCapability,
    reason: Value,
) -> error::Result<()> {
    call_promise_capability_function(vm, &capability.reject, reason).map(|_| ())
}

fn promise_capability_reject_and_return(
    vm: &mut Vm,
    capability: &PromiseCapability,
    err: Arc<error::Error>,
) -> error::Result<Value> {
    reject_promise_capability(vm, capability, promise_rejection_value(&err))?;
    Ok(capability.promise.clone())
}

pub(crate) fn promise_all_resolve_element(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let record_idx = match this {
        Some(Value::Object(idx)) => idx,
        _ => return Err(Error::type_err("Promise.all resolve element receiver")),
    };
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    let (already_called, index, state) = vm.heap.with_obj(record_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        let already_called = matches!(
            props
                .get(&PropertyKey::from("alreadyCalled"))
                .map(|desc| &desc.value),
            Some(Value::Bool(true))
        );
        props.insert(
            PropertyKey::from("alreadyCalled"),
            PropertyDescriptor::data(Value::Bool(true)),
        );
        let index = match props
            .get(&PropertyKey::from("index"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n >= 0.0 => n as usize,
            _ => 0,
        };
        let state = props
            .get(&PropertyKey::from("state"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        (already_called, index, state)
    });
    if already_called {
        return Ok(Value::Undefined);
    }

    let state_idx = match state {
        Value::Object(idx) => idx,
        _ => return Err(Error::type_err("Promise.all state record")),
    };
    let (values, resolve, reject, remaining) = vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let props = props.lock();
        let values = props
            .get(&PropertyKey::from("values"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let resolve = props
            .get(&PropertyKey::from("resolve"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let reject = props
            .get(&PropertyKey::from("reject"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let remaining = match props
            .get(&PropertyKey::from("remaining"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n > 0.0 => n as usize,
            _ => 0,
        };
        (values, resolve, reject, remaining)
    });
    let values_idx = match &values {
        Value::Object(idx) => *idx,
        _ => return Err(Error::type_err("Promise.all values array")),
    };
    vm.heap.with_obj(values_idx.0, |obj| {
        if let HeapObj::Array(array) = obj {
            let mut items = array.items.lock();
            let mut present = array.present.lock();
            if index >= items.len() {
                items.resize(index + 1, Value::Undefined);
                present.resize(index + 1, false);
            }
            items[index] = value;
            present[index] = true;
            Ok(())
        } else {
            Err(Error::type_err("Promise.all values array"))
        }
    })?;

    let remaining = remaining.saturating_sub(1);
    vm.heap.with_obj(state_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(remaining as f64)),
        );
    });
    if remaining == 0 {
        if let Err(err) = call_promise_capability_function(vm, &resolve, values) {
            call_promise_capability_function(vm, &reject, promise_rejection_value(&err))?;
        }
    }
    Ok(Value::Undefined)
}

fn promise_all_settled_element(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    status: &str,
    key: &str,
) -> error::Result<Value> {
    let record_idx = match this {
        Some(Value::Object(idx)) => idx,
        _ => return Err(Error::type_err("Promise.allSettled element receiver")),
    };
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    let (already_called, index, state) = vm.heap.with_obj(record_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        let already_called = matches!(
            props
                .get(&PropertyKey::from("alreadyCalled"))
                .map(|desc| &desc.value),
            Some(Value::Bool(true))
        );
        props.insert(
            PropertyKey::from("alreadyCalled"),
            PropertyDescriptor::data(Value::Bool(true)),
        );
        let index = match props
            .get(&PropertyKey::from("index"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n >= 0.0 => n as usize,
            _ => 0,
        };
        let state = props
            .get(&PropertyKey::from("state"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        (already_called, index, state)
    });
    if already_called {
        return Ok(Value::Undefined);
    }

    let state_idx = match state {
        Value::Object(idx) => idx,
        _ => return Err(Error::type_err("Promise.allSettled state record")),
    };
    let (values, resolve, reject, remaining) = vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let props = props.lock();
        let values = props
            .get(&PropertyKey::from("values"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let resolve = props
            .get(&PropertyKey::from("resolve"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let reject = props
            .get(&PropertyKey::from("reject"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let remaining = match props
            .get(&PropertyKey::from("remaining"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n > 0.0 => n as usize,
            _ => 0,
        };
        (values, resolve, reject, remaining)
    });
    let result_pins = vm.pin_many(std::slice::from_ref(&value));
    let result = settled_result_object(vm, status, key, value);
    vm.unpin_many(result_pins);
    let result = result?;
    let values_idx = match &values {
        Value::Object(idx) => *idx,
        _ => return Err(Error::type_err("Promise.allSettled values array")),
    };
    vm.heap.with_obj(values_idx.0, |obj| {
        if let HeapObj::Array(array) = obj {
            let mut items = array.items.lock();
            let mut present = array.present.lock();
            if index >= items.len() {
                items.resize(index + 1, Value::Undefined);
                present.resize(index + 1, false);
            }
            items[index] = result;
            present[index] = true;
            Ok(())
        } else {
            Err(Error::type_err("Promise.allSettled values array"))
        }
    })?;

    let remaining = remaining.saturating_sub(1);
    vm.heap.with_obj(state_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(remaining as f64)),
        );
    });
    if remaining == 0 {
        if let Err(err) = call_promise_capability_function(vm, &resolve, values) {
            call_promise_capability_function(vm, &reject, promise_rejection_value(&err))?;
        }
    }
    Ok(Value::Undefined)
}

pub(crate) fn promise_all_settled_resolve_element(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    promise_all_settled_element(vm, args, this, "fulfilled", "value")
}

pub(crate) fn promise_all_settled_reject_element(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    promise_all_settled_element(vm, args, this, "rejected", "reason")
}

pub(crate) fn promise_any_reject_element(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let record_idx = match this {
        Some(Value::Object(idx)) => idx,
        _ => return Err(Error::type_err("Promise.any reject element receiver")),
    };
    let reason = args.first().cloned().unwrap_or(Value::Undefined);
    let (already_called, index, state) = vm.heap.with_obj(record_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        let already_called = matches!(
            props
                .get(&PropertyKey::from("alreadyCalled"))
                .map(|desc| &desc.value),
            Some(Value::Bool(true))
        );
        props.insert(
            PropertyKey::from("alreadyCalled"),
            PropertyDescriptor::data(Value::Bool(true)),
        );
        let index = match props
            .get(&PropertyKey::from("index"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n >= 0.0 => n as usize,
            _ => 0,
        };
        let state = props
            .get(&PropertyKey::from("state"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        (already_called, index, state)
    });
    if already_called {
        return Ok(Value::Undefined);
    }

    let state_idx = match state {
        Value::Object(idx) => idx,
        _ => return Err(Error::type_err("Promise.any state record")),
    };
    let (errors, reject, remaining) = vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let props = props.lock();
        let errors = props
            .get(&PropertyKey::from("errors"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let reject = props
            .get(&PropertyKey::from("reject"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let remaining = match props
            .get(&PropertyKey::from("remaining"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n > 0.0 => n as usize,
            _ => 0,
        };
        (errors, reject, remaining)
    });
    let errors_idx = match &errors {
        Value::Object(idx) => *idx,
        _ => return Err(Error::type_err("Promise.any errors array")),
    };
    vm.heap.with_obj(errors_idx.0, |obj| {
        if let HeapObj::Array(array) = obj {
            let mut items = array.items.lock();
            let mut present = array.present.lock();
            if index >= items.len() {
                items.resize(index + 1, Value::Undefined);
                present.resize(index + 1, false);
            }
            items[index] = reason;
            present[index] = true;
            Ok(())
        } else {
            Err(Error::type_err("Promise.any errors array"))
        }
    })?;

    let remaining = remaining.saturating_sub(1);
    vm.heap.with_obj(state_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(remaining as f64)),
        );
    });
    if remaining == 0 {
        let error = make_aggregate_error(vm, errors)?;
        call_promise_capability_function(vm, &reject, error)?;
    }
    Ok(Value::Undefined)
}

fn settled_result_object(
    vm: &mut Vm,
    status: &str,
    key: &str,
    value: Value,
) -> error::Result<Value> {
    let obj = vm.new_object()?;
    vm.heap.with_obj(obj.0, |o| {
        let props = o.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("status"),
            data_prop(Value::String(Arc::from(status))),
        );
        props.insert(PropertyKey::from(key), data_prop(value));
    });
    Ok(Value::Object(obj))
}

pub(crate) fn promise_static_all(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let capability = new_promise_capability(vm, ctor.clone())?;
    let mut pins = vm.pin_many(&[
        ctor.clone(),
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
    ]);
    let promise_resolve = match vm.get_property(&ctor, "resolve") {
        Ok(promise_resolve) => promise_resolve,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };
    if !is_callable(&promise_resolve, &vm.heap) {
        let result = reject_promise_capability(
            vm,
            &capability,
            Value::String(Arc::from("Promise.all resolve is not callable")),
        )
        .map(|_| capability.promise.clone());
        vm.unpin_many(pins);
        return result;
    }

    let iterable = args.first().cloned().unwrap_or(Value::Undefined);
    pins += vm.pin_many(&[promise_resolve.clone(), iterable.clone()]);
    let iter = match vm.make_iterator(&iterable) {
        Ok(iter) => iter,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };

    let values = make_value_array(vm, Vec::new())?;
    pins += vm.pin_many(std::slice::from_ref(&values));
    let state_idx = vm.new_object()?;
    let state = Value::Object(state_idx);
    vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("values"),
            PropertyDescriptor::data(values.clone()),
        );
        props.insert(
            PropertyKey::from("resolve"),
            PropertyDescriptor::data(capability.resolve.clone()),
        );
        props.insert(
            PropertyKey::from("reject"),
            PropertyDescriptor::data(capability.reject.clone()),
        );
        props.insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(1.0)),
        );
    });
    pins += vm.pin_many(std::slice::from_ref(&state));
    let mut index = 0usize;

    loop {
        let (value, done) = match vm.iterator_next(&iter) {
            Ok(step) => step,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        if done {
            let remaining = vm.heap.with_obj(state_idx.0, |obj| {
                let props = obj.props();
                let mut props = props.lock();
                let remaining = match props
                    .get(&PropertyKey::from("remaining"))
                    .map(|desc| desc.value.clone())
                {
                    Some(Value::Number(n)) if n > 0.0 => n as usize,
                    _ => 0,
                }
                .saturating_sub(1);
                props.insert(
                    PropertyKey::from("remaining"),
                    PropertyDescriptor::data(Value::Number(remaining as f64)),
                );
                remaining
            });
            if remaining == 0 {
                let resolve_result =
                    call_promise_capability_function(vm, &capability.resolve, values.clone());
                let result = match resolve_result {
                    Ok(_) => Ok(capability.promise.clone()),
                    Err(err) => promise_capability_reject_and_return(vm, &capability, err),
                };
                vm.unpin_many(pins);
                return result;
            }
            vm.unpin_many(pins);
            return Ok(capability.promise);
        }

        if let Value::Object(values_idx) = &values {
            vm.heap.with_obj(values_idx.0, |obj| {
                if let HeapObj::Array(array) = obj {
                    array.items.lock().push(Value::Undefined);
                    array.present.lock().push(false);
                }
            });
        }
        vm.heap.with_obj(state_idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            let remaining = match props
                .get(&PropertyKey::from("remaining"))
                .map(|desc| desc.value.clone())
            {
                Some(Value::Number(n)) if n > 0.0 => n,
                _ => 0.0,
            };
            props.insert(
                PropertyKey::from("remaining"),
                PropertyDescriptor::data(Value::Number(remaining + 1.0)),
            );
        });

        let record_idx = vm.new_object()?;
        let record = Value::Object(record_idx);
        let record_pins = vm.pin_many(std::slice::from_ref(&record));
        vm.heap.with_obj(record_idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            props.insert(
                PropertyKey::from("alreadyCalled"),
                PropertyDescriptor::data(Value::Bool(false)),
            );
            props.insert(
                PropertyKey::from("index"),
                PropertyDescriptor::data(Value::Number(index as f64)),
            );
            props.insert(
                PropertyKey::from("state"),
                PropertyDescriptor::data(state.clone()),
            );
        });
        let resolve_element_result = create_bound_native_function(
            vm,
            "",
            "",
            promise_all_resolve_element,
            1,
            record.clone(),
        );
        vm.unpin_many(record_pins);
        let resolve_element = match resolve_element_result {
            Ok(resolve_element) => resolve_element,
            Err(err) => {
                vm.unpin_many(pins);
                return Err(err);
            }
        };
        let element_pins = vm.pin_many(&[value.clone(), record, resolve_element.clone()]);
        let next_promise_result = vm.call_function(
            &promise_resolve,
            std::slice::from_ref(&value),
            Some(ctor.clone()),
        );
        vm.unpin_many(element_pins);
        let next_promise = match next_promise_result {
            Ok(next_promise) => next_promise,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then = match vm.get_property(&next_promise, "then") {
            Ok(then) => then,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then_pins = vm.pin_many(&[next_promise.clone(), then.clone()]);
        let then_result = vm.call_function(
            &then,
            &[resolve_element, capability.reject.clone()],
            Some(next_promise),
        );
        vm.unpin_many(then_pins);
        if let Err(err) = then_result {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
        index += 1;
    }
}

pub(crate) fn promise_static_race(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let capability = new_promise_capability(vm, ctor.clone())?;
    let mut pins = vm.pin_many(&[
        ctor.clone(),
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
    ]);
    let promise_resolve = match vm.get_property(&ctor, "resolve") {
        Ok(promise_resolve) => promise_resolve,
        Err(err) => {
            vm.unpin_many(pins);
            return Err(err);
        }
    };
    if !is_callable(&promise_resolve, &vm.heap) {
        vm.unpin_many(pins);
        return Err(Error::type_err("Promise.race resolve is not callable"));
    }

    let iterable = args.first().cloned().unwrap_or(Value::Undefined);
    pins += vm.pin_many(&[promise_resolve.clone(), iterable.clone()]);
    let iter = match vm.make_iterator(&iterable) {
        Ok(iter) => iter,
        Err(err) => {
            let reason = promise_rejection_value(&err);
            let reject_result = reject_promise_capability(vm, &capability, reason);
            vm.unpin_many(pins);
            reject_result?;
            return Ok(capability.promise);
        }
    };

    loop {
        let (value, done) = match vm.iterator_next(&iter) {
            Ok(step) => step,
            Err(err) => {
                let reason = promise_rejection_value(&err);
                let reject_result = reject_promise_capability(vm, &capability, reason);
                vm.unpin_many(pins);
                reject_result?;
                return Ok(capability.promise);
            }
        };
        if done {
            vm.unpin_many(pins);
            return Ok(capability.promise);
        }

        let value_pins = vm.pin_many(std::slice::from_ref(&value));
        let next_promise_result = vm.call_function(
            &promise_resolve,
            std::slice::from_ref(&value),
            Some(ctor.clone()),
        );
        vm.unpin_many(value_pins);
        let next_promise = match next_promise_result {
            Ok(next_promise) => next_promise,
            Err(err) => {
                let reason = promise_rejection_value(&err);
                let reject_result = reject_promise_capability(vm, &capability, reason);
                vm.unpin_many(pins);
                reject_result?;
                return Ok(capability.promise);
            }
        };
        let then = match vm.get_property(&next_promise, "then") {
            Ok(then) => then,
            Err(err) => {
                let reason = promise_rejection_value(&err);
                let reject_result = reject_promise_capability(vm, &capability, reason);
                vm.unpin_many(pins);
                reject_result?;
                return Ok(capability.promise);
            }
        };
        let then_pins = vm.pin_many(&[next_promise.clone(), then.clone()]);
        let then_result = vm.call_function(
            &then,
            &[capability.resolve.clone(), capability.reject.clone()],
            Some(next_promise),
        );
        vm.unpin_many(then_pins);
        if let Err(err) = then_result {
            let reason = promise_rejection_value(&err);
            let reject_result = reject_promise_capability(vm, &capability, reason);
            vm.unpin_many(pins);
            reject_result?;
            return Ok(capability.promise);
        }
    }
}

pub(crate) fn promise_static_all_settled(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let capability = new_promise_capability(vm, ctor.clone())?;
    let mut pins = vm.pin_many(&[
        ctor.clone(),
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
    ]);
    let promise_resolve = match vm.get_property(&ctor, "resolve") {
        Ok(promise_resolve) => promise_resolve,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };
    if !is_callable(&promise_resolve, &vm.heap) {
        let result = reject_promise_capability(
            vm,
            &capability,
            Value::String(Arc::from("Promise.allSettled resolve is not callable")),
        )
        .map(|_| capability.promise.clone());
        vm.unpin_many(pins);
        return result;
    }

    let iterable = args.first().cloned().unwrap_or(Value::Undefined);
    pins += vm.pin_many(&[promise_resolve.clone(), iterable.clone()]);
    let iter = match vm.make_iterator(&iterable) {
        Ok(iter) => iter,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };

    let values = make_value_array(vm, Vec::new())?;
    pins += vm.pin_many(std::slice::from_ref(&values));
    let state_idx = vm.new_object()?;
    let state = Value::Object(state_idx);
    vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("values"),
            PropertyDescriptor::data(values.clone()),
        );
        props.insert(
            PropertyKey::from("resolve"),
            PropertyDescriptor::data(capability.resolve.clone()),
        );
        props.insert(
            PropertyKey::from("reject"),
            PropertyDescriptor::data(capability.reject.clone()),
        );
        props.insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(1.0)),
        );
    });
    pins += vm.pin_many(std::slice::from_ref(&state));
    let mut index = 0usize;

    loop {
        let (value, done) = match vm.iterator_next(&iter) {
            Ok(step) => step,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        if done {
            let remaining = vm.heap.with_obj(state_idx.0, |obj| {
                let props = obj.props();
                let mut props = props.lock();
                let remaining = match props
                    .get(&PropertyKey::from("remaining"))
                    .map(|desc| desc.value.clone())
                {
                    Some(Value::Number(n)) if n > 0.0 => n as usize,
                    _ => 0,
                }
                .saturating_sub(1);
                props.insert(
                    PropertyKey::from("remaining"),
                    PropertyDescriptor::data(Value::Number(remaining as f64)),
                );
                remaining
            });
            if remaining == 0 {
                let resolve_result =
                    call_promise_capability_function(vm, &capability.resolve, values.clone());
                let result = match resolve_result {
                    Ok(_) => Ok(capability.promise.clone()),
                    Err(err) => promise_capability_reject_and_return(vm, &capability, err),
                };
                vm.unpin_many(pins);
                return result;
            }
            vm.unpin_many(pins);
            return Ok(capability.promise);
        }

        if let Value::Object(values_idx) = &values {
            vm.heap.with_obj(values_idx.0, |obj| {
                if let HeapObj::Array(array) = obj {
                    array.items.lock().push(Value::Undefined);
                    array.present.lock().push(false);
                }
            });
        }
        vm.heap.with_obj(state_idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            let remaining = match props
                .get(&PropertyKey::from("remaining"))
                .map(|desc| desc.value.clone())
            {
                Some(Value::Number(n)) if n > 0.0 => n,
                _ => 0.0,
            };
            props.insert(
                PropertyKey::from("remaining"),
                PropertyDescriptor::data(Value::Number(remaining + 1.0)),
            );
        });

        let record_idx = vm.new_object()?;
        let record = Value::Object(record_idx);
        let record_pins = vm.pin_many(std::slice::from_ref(&record));
        vm.heap.with_obj(record_idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            props.insert(
                PropertyKey::from("alreadyCalled"),
                PropertyDescriptor::data(Value::Bool(false)),
            );
            props.insert(
                PropertyKey::from("index"),
                PropertyDescriptor::data(Value::Number(index as f64)),
            );
            props.insert(
                PropertyKey::from("state"),
                PropertyDescriptor::data(state.clone()),
            );
        });
        let resolve_element_result = create_bound_native_function(
            vm,
            "",
            "",
            promise_all_settled_resolve_element,
            1,
            record.clone(),
        );
        let resolve_element = match resolve_element_result {
            Ok(resolve_element) => resolve_element,
            Err(err) => {
                vm.unpin_many(record_pins);
                vm.unpin_many(pins);
                return Err(err);
            }
        };
        let resolve_pin = vm.pin_many(std::slice::from_ref(&resolve_element));
        let reject_element_result = create_bound_native_function(
            vm,
            "",
            "",
            promise_all_settled_reject_element,
            1,
            record.clone(),
        );
        vm.unpin_many(resolve_pin);
        vm.unpin_many(record_pins);
        let reject_element = match reject_element_result {
            Ok(reject_element) => reject_element,
            Err(err) => {
                vm.unpin_many(pins);
                return Err(err);
            }
        };
        let element_pins = vm.pin_many(&[
            value.clone(),
            record,
            resolve_element.clone(),
            reject_element.clone(),
        ]);
        let next_promise_result = vm.call_function(
            &promise_resolve,
            std::slice::from_ref(&value),
            Some(ctor.clone()),
        );
        vm.unpin_many(element_pins);
        let next_promise = match next_promise_result {
            Ok(next_promise) => next_promise,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then = match vm.get_property(&next_promise, "then") {
            Ok(then) => then,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then_pins = vm.pin_many(&[next_promise.clone(), then.clone()]);
        let then_result = vm.call_function(
            &then,
            &[resolve_element, reject_element],
            Some(next_promise),
        );
        vm.unpin_many(then_pins);
        if let Err(err) = then_result {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
        index += 1;
    }
}

pub(crate) fn promise_static_any(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let capability = new_promise_capability(vm, ctor.clone())?;
    let mut pins = vm.pin_many(&[
        ctor.clone(),
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
    ]);
    let promise_resolve = match vm.get_property(&ctor, "resolve") {
        Ok(promise_resolve) => promise_resolve,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };
    if !is_callable(&promise_resolve, &vm.heap) {
        let result = reject_promise_capability(
            vm,
            &capability,
            Value::String(Arc::from("Promise.any resolve is not callable")),
        )
        .map(|_| capability.promise.clone());
        vm.unpin_many(pins);
        return result;
    }

    let iterable = args.first().cloned().unwrap_or(Value::Undefined);
    pins += vm.pin_many(&[promise_resolve.clone(), iterable.clone()]);
    let iter = match vm.make_iterator(&iterable) {
        Ok(iter) => iter,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };

    let errors = make_value_array(vm, Vec::new())?;
    pins += vm.pin_many(std::slice::from_ref(&errors));
    let state_idx = vm.new_object()?;
    let state = Value::Object(state_idx);
    vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("errors"),
            PropertyDescriptor::data(errors.clone()),
        );
        props.insert(
            PropertyKey::from("reject"),
            PropertyDescriptor::data(capability.reject.clone()),
        );
        props.insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(1.0)),
        );
    });
    pins += vm.pin_many(std::slice::from_ref(&state));
    let mut index = 0usize;

    loop {
        let (value, done) = match vm.iterator_next(&iter) {
            Ok(step) => step,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        if done {
            let remaining = vm.heap.with_obj(state_idx.0, |obj| {
                let props = obj.props();
                let mut props = props.lock();
                let remaining = match props
                    .get(&PropertyKey::from("remaining"))
                    .map(|desc| desc.value.clone())
                {
                    Some(Value::Number(n)) if n > 0.0 => n as usize,
                    _ => 0,
                }
                .saturating_sub(1);
                props.insert(
                    PropertyKey::from("remaining"),
                    PropertyDescriptor::data(Value::Number(remaining as f64)),
                );
                remaining
            });
            if remaining == 0 {
                let error = make_aggregate_error(vm, errors.clone());
                let reject_result = error.and_then(|error| {
                    call_promise_capability_function(vm, &capability.reject, error)
                });
                let result = match reject_result {
                    Ok(_) => Ok(capability.promise.clone()),
                    Err(err) => Err(err),
                };
                vm.unpin_many(pins);
                return result;
            }
            vm.unpin_many(pins);
            return Ok(capability.promise);
        }

        if let Value::Object(errors_idx) = &errors {
            vm.heap.with_obj(errors_idx.0, |obj| {
                if let HeapObj::Array(array) = obj {
                    array.items.lock().push(Value::Undefined);
                    array.present.lock().push(false);
                }
            });
        }
        vm.heap.with_obj(state_idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            let remaining = match props
                .get(&PropertyKey::from("remaining"))
                .map(|desc| desc.value.clone())
            {
                Some(Value::Number(n)) if n > 0.0 => n,
                _ => 0.0,
            };
            props.insert(
                PropertyKey::from("remaining"),
                PropertyDescriptor::data(Value::Number(remaining + 1.0)),
            );
        });

        let record_idx = vm.new_object()?;
        let record = Value::Object(record_idx);
        let record_pins = vm.pin_many(std::slice::from_ref(&record));
        vm.heap.with_obj(record_idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            props.insert(
                PropertyKey::from("alreadyCalled"),
                PropertyDescriptor::data(Value::Bool(false)),
            );
            props.insert(
                PropertyKey::from("index"),
                PropertyDescriptor::data(Value::Number(index as f64)),
            );
            props.insert(
                PropertyKey::from("state"),
                PropertyDescriptor::data(state.clone()),
            );
        });
        let reject_element_result =
            create_bound_native_function(vm, "", "", promise_any_reject_element, 1, record.clone());
        vm.unpin_many(record_pins);
        let reject_element = match reject_element_result {
            Ok(reject_element) => reject_element,
            Err(err) => {
                vm.unpin_many(pins);
                return Err(err);
            }
        };
        let element_pins = vm.pin_many(&[value.clone(), record, reject_element.clone()]);
        let next_promise_result = vm.call_function(
            &promise_resolve,
            std::slice::from_ref(&value),
            Some(ctor.clone()),
        );
        vm.unpin_many(element_pins);
        let next_promise = match next_promise_result {
            Ok(next_promise) => next_promise,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then = match vm.get_property(&next_promise, "then") {
            Ok(then) => then,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then_pins = vm.pin_many(&[next_promise.clone(), then.clone()]);
        let then_result = vm.call_function(
            &then,
            &[capability.resolve.clone(), reject_element],
            Some(next_promise),
        );
        vm.unpin_many(then_pins);
        if let Err(err) = then_result {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
        index += 1;
    }
}

pub(crate) fn promise_static_try(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let capability = new_promise_capability(vm, ctor)?;
    let callback = args.first().cloned().unwrap_or(Value::Undefined);

    let mut roots = vec![
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
        callback.clone(),
    ];
    roots.extend(args.iter().skip(1).cloned());
    let pins = vm.pin_many(&roots);

    let callback_result = if is_callable(&callback, &vm.heap) {
        vm.call_function(&callback, &args[1..], Some(Value::Undefined))
    } else {
        Err(Error::type_err("Promise.try callback is not a function"))
    };
    let settle_result = match callback_result {
        Ok(value) => call_promise_capability_function(vm, &capability.resolve, value),
        Err(err) => {
            call_promise_capability_function(vm, &capability.reject, promise_rejection_value(&err))
        }
    };
    vm.unpin_many(pins);
    settle_result?;
    Ok(capability.promise)
}

pub(crate) fn promise_with_resolvers_executor(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let result_obj = match this {
        Some(Value::Object(idx)) => idx,
        _ => return Err(Error::type_err("Promise.withResolvers executor receiver")),
    };
    let resolve = args.first().cloned().unwrap_or(Value::Undefined);
    let reject = args.get(1).cloned().unwrap_or(Value::Undefined);
    vm.heap.with_obj(result_obj.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("resolve"),
            PropertyDescriptor::data(resolve),
        );
        props.insert(
            PropertyKey::from("reject"),
            PropertyDescriptor::data(reject),
        );
    });
    Ok(Value::Undefined)
}

pub(crate) fn promise_with_resolvers(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    if !vm.is_constructor_value(&ctor) {
        return Err(Error::type_err(
            "Promise.withResolvers receiver is not a constructor",
        ));
    }

    let result_idx = vm.new_object()?;
    let result = Value::Object(result_idx);
    let executor = create_bound_native_function(
        vm,
        "",
        "",
        promise_with_resolvers_executor,
        2,
        result.clone(),
    )?;
    let pins = vm.pin_many(&[ctor.clone(), result.clone(), executor.clone()]);
    let promise = vm.construct(&ctor, std::slice::from_ref(&executor));
    vm.unpin_many(pins);
    let promise = promise?;

    vm.heap.with_obj(result_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("promise"),
            PropertyDescriptor::data(promise),
        );
    });
    Ok(result)
}

pub(crate) fn promise_finally(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let promise = this.unwrap_or(Value::Undefined);
    let on_finally = args.first().cloned().unwrap_or(Value::Undefined);
    let then = vm.get_property(&promise, "then")?;
    vm.call_function(&then, &[on_finally.clone(), on_finally], Some(promise))
}

pub(crate) fn promise_species_get(
    _vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    Ok(this.unwrap_or(Value::Undefined))
}

fn promise_species_constructor(
    vm: &mut Vm,
    promise: &Value,
    default_constructor: Value,
) -> error::Result<Value> {
    let constructor = vm.get_property_by_key(promise, &PropertyKey::from("constructor"))?;
    if constructor.is_undefined() {
        return Ok(default_constructor);
    }
    if !matches!(constructor, Value::Object(_)) {
        return Err(Error::type_err("Promise constructor is not an object"));
    }

    let species_key = PropertyKey::Symbol(vm.well_known_symbols.species);
    let species = vm.get_property_by_key(&constructor, &species_key)?;
    if species.is_undefined() || matches!(species, Value::Null) {
        return Ok(default_constructor);
    }
    if !vm.is_constructor_value(&species) {
        return Err(Error::type_err("Promise species is not a constructor"));
    }
    Ok(species)
}

pub(crate) fn promise_then(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let on_fulfilled = args.first().cloned().unwrap_or(Value::Undefined);
    let on_rejected = args.get(1).cloned().unwrap_or(Value::Undefined);
    let promise = this.unwrap_or(Value::Undefined);
    let p_idx = match &promise {
        Value::Object(idx)
            if vm
                .heap
                .with_obj(idx.0, |obj| matches!(obj, HeapObj::Promise(_))) =>
        {
            idx.0
        }
        _ => return Err(Error::type_err("then called on non-promise")),
    };
    let constructor = promise_species_constructor(vm, &promise, vm.promise_ctor.clone())?;
    let capability = new_promise_capability(vm, constructor)?;
    let derived = crate::value::PromiseReactionCapability {
        promise: capability.promise,
        resolve: capability.resolve,
        reject: capability.reject,
    };
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
        derived: Some(derived.clone()),
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
                derived: Some(derived.clone()),
            });
        }
    }
    Ok(derived.promise)
}

pub(crate) fn promise_catch(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let promise = this.unwrap_or(Value::Undefined);
    let on_rejected = args.first().cloned().unwrap_or(Value::Undefined);
    let then = vm.get_property(&promise, "then")?;
    vm.call_function(&then, &[Value::Undefined, on_rejected], Some(promise))
}

// =========================================================================
