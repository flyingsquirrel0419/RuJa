use super::*;

// =========================================================================
// Array prototype + constructor
// =========================================================================

fn array_from_async_error_reason(
    vm: &mut Vm,
    error: &Arc<Error>,
    realm: GcIdx,
) -> error::Result<Value> {
    vm.promise_rejection_reason_in_realm(error, realm)
}

fn settle_array_from_async(
    vm: &mut Vm,
    frame: &crate::value::ArrayFromAsyncContinuation,
    value: Value,
    reject: bool,
) -> error::Result<()> {
    let function = if reject {
        &frame.capability.reject
    } else {
        &frame.capability.resolve
    };
    let pins = vm.pin_many(&[
        frame.capability.promise.clone(),
        function.clone(),
        value.clone(),
    ]);
    let result = vm.call_function(function, &[value], Some(Value::Undefined));
    vm.unpin_many(pins);
    result.map(|_| ())
}

fn reject_array_from_async_error(
    vm: &mut Vm,
    frame: &crate::value::ArrayFromAsyncContinuation,
    error: &Arc<Error>,
) -> error::Result<()> {
    let reason = array_from_async_error_reason(vm, error, frame.realm)?;
    settle_array_from_async(vm, frame, reason, true)
}

fn await_array_from_async(
    vm: &mut Vm,
    mut frame: crate::value::ArrayFromAsyncContinuation,
    value: Value,
    await_kind: crate::value::ArrayFromAsyncAwaitKind,
) -> error::Result<()> {
    frame.await_kind = await_kind;
    let realm = frame.realm;
    let pins = vm.pin_many(&[
        frame.capability.promise.clone(),
        frame.capability.resolve.clone(),
        frame.capability.reject.clone(),
        frame.target.clone(),
        frame.source.clone(),
        frame.iterator.clone(),
        frame.next_method.clone(),
        frame.mapper.clone(),
        frame.this_arg.clone(),
        value.clone(),
    ]);
    let result = (|| -> error::Result<()> {
        let wrapper = vm.promise_resolve_for_await_in_env(value, realm)?;
        let handler = crate::value::PromiseHandler {
            on_fulfilled: Value::Undefined,
            on_rejected: Value::Undefined,
            derived: None,
            continuation: Some(crate::value::PromiseContinuation::ArrayFromAsync(Box::new(
                frame,
            ))),
        };
        let status = vm.heap.with_obj(wrapper.0, |object| {
            if let HeapObj::Promise(data) = object {
                *data.state.lock()
            } else {
                crate::value::PromiseStatus::Fulfilled
            }
        });
        if status == crate::value::PromiseStatus::Pending {
            vm.heap.with_obj(wrapper.0, |object| {
                if let HeapObj::Promise(data) = object {
                    data.handlers.lock().push(handler);
                }
            });
        } else {
            vm.microtask_queue.push_back(crate::vm::Microtask::Then {
                promise: wrapper,
                on_fulfilled: Value::Undefined,
                on_rejected: Value::Undefined,
                derived: None,
                continuation: handler.continuation,
                realm: None,
            });
        }
        Ok(())
    })();
    vm.unpin_many(pins);
    result
}

/// Shared ArrayCreate path for built-ins that must preserve both the method
/// Realm's intrinsic prototype and the sandbox allocator's GC retry.
pub(crate) fn array_create_in_realm(
    vm: &mut Vm,
    length: usize,
    realm: GcIdx,
) -> error::Result<Value> {
    if length > u32::MAX as usize {
        return Err(Error::range("Invalid array length"));
    }
    let prototype = vm
        .realm_array_prototypes
        .get(&realm.0)
        .cloned()
        .unwrap_or_else(|| vm.array_proto.clone());
    let array = vm.alloc(HeapObj::Array(ArrayData::new(Vec::new(), Some(prototype))))?;
    if length != 0 {
        vm.set_array_length(array.0, Value::Number(length as f64))?;
    }
    Ok(Value::Object(array))
}

pub(crate) fn array_create_in_current_realm(vm: &mut Vm, length: usize) -> error::Result<Value> {
    let realm = vm.current_realm_global_env();
    array_create_in_realm(vm, length, realm)
}

fn pin_array_from_async_frame(
    vm: &mut Vm,
    frame: &crate::value::ArrayFromAsyncContinuation,
) -> usize {
    vm.pin_many(&[
        frame.capability.promise.clone(),
        frame.capability.resolve.clone(),
        frame.capability.reject.clone(),
        frame.target.clone(),
        frame.source.clone(),
        frame.iterator.clone(),
        frame.next_method.clone(),
        frame.mapper.clone(),
        frame.this_arg.clone(),
    ])
}

fn array_from_async_get_method(
    vm: &mut Vm,
    value: &Value,
    key: PropertyKey,
) -> error::Result<Option<Value>> {
    let method = vm.get_property_by_key(value, &key)?;
    if method.is_nullish() {
        return Ok(None);
    }
    if !is_callable(&method, &vm.heap) {
        return Err(Error::type_err("iterator method is not callable"));
    }
    Ok(Some(method))
}

fn array_from_async_finish(
    vm: &mut Vm,
    frame: crate::value::ArrayFromAsyncContinuation,
) -> error::Result<()> {
    match vm.set_property_strict(&frame.target, "length", Value::Number(frame.index as f64)) {
        Ok(()) => settle_array_from_async(vm, &frame, frame.target.clone(), false),
        Err(error) => reject_array_from_async_error(vm, &frame, &error),
    }
}

fn array_from_async_define_and_continue(
    vm: &mut Vm,
    mut frame: crate::value::ArrayFromAsyncContinuation,
    value: Value,
    iterable: bool,
) -> error::Result<()> {
    let key = PropertyKey::from_integer_index(frame.index as u64);
    let define =
        vm.define_own_property_or_throw(&frame.target, key, PropertyDescriptor::data(value));
    if let Err(error) = define {
        if iterable {
            let reason = array_from_async_error_reason(vm, &error, frame.realm)?;
            return array_from_async_close(vm, frame, reason);
        }
        return reject_array_from_async_error(vm, &frame, &error);
    }
    frame.index += 1;
    if iterable {
        array_from_async_next(vm, frame)
    } else if frame.index >= frame.length {
        array_from_async_finish(vm, frame)
    } else {
        let next = match vm.get_property(&frame.source, &frame.index.to_string()) {
            Ok(value) => value,
            Err(error) => return reject_array_from_async_error(vm, &frame, &error),
        };
        await_array_from_async(
            vm,
            frame,
            next,
            crate::value::ArrayFromAsyncAwaitKind::ArrayLikeValue,
        )
    }
}

fn array_from_async_map_or_define(
    vm: &mut Vm,
    frame: crate::value::ArrayFromAsyncContinuation,
    value: Value,
    iterable: bool,
) -> error::Result<()> {
    if frame.mapper.is_undefined() {
        return array_from_async_define_and_continue(vm, frame, value, iterable);
    }
    let mapped = vm.call_function(
        &frame.mapper,
        &[value, Value::Number(frame.index as f64)],
        Some(frame.this_arg.clone()),
    );
    let mapped = match mapped {
        Ok(value) => value,
        Err(error) if iterable => {
            let reason = array_from_async_error_reason(vm, &error, frame.realm)?;
            return array_from_async_close(vm, frame, reason);
        }
        Err(error) => return reject_array_from_async_error(vm, &frame, &error),
    };
    let kind = if iterable {
        crate::value::ArrayFromAsyncAwaitKind::MappedValue
    } else {
        crate::value::ArrayFromAsyncAwaitKind::ArrayLikeMappedValue
    };
    await_array_from_async(vm, frame, mapped, kind)
}

fn array_from_async_next(
    vm: &mut Vm,
    frame: crate::value::ArrayFromAsyncContinuation,
) -> error::Result<()> {
    let pins = pin_array_from_async_frame(vm, &frame);
    let result = array_from_async_next_inner(vm, frame);
    vm.unpin_many(pins);
    result
}

fn array_from_async_next_inner(
    vm: &mut Vm,
    frame: crate::value::ArrayFromAsyncContinuation,
) -> error::Result<()> {
    if frame.index >= MAX_SAFE_ARRAY_LENGTH as usize {
        let error = Error::type_err("Array.fromAsync result exceeds maximum safe length");
        let reason = array_from_async_error_reason(vm, &error, frame.realm)?;
        return array_from_async_close(vm, frame, reason);
    }
    if frame.sync_iterator {
        let next = match vm.iterator_next_await_start_in_env(&frame.iterator, frame.realm) {
            Ok(value) => value,
            Err(error) => return reject_array_from_async_error(vm, &frame, &error),
        };
        return await_array_from_async(
            vm,
            frame,
            next,
            crate::value::ArrayFromAsyncAwaitKind::IteratorNext,
        );
    }
    let next = vm.call_function(&frame.next_method, &[], Some(frame.iterator.clone()));
    let next = match next {
        Ok(value) => value,
        Err(error) => return reject_array_from_async_error(vm, &frame, &error),
    };
    await_array_from_async(
        vm,
        frame,
        next,
        crate::value::ArrayFromAsyncAwaitKind::IteratorNext,
    )
}

fn array_from_async_close(
    vm: &mut Vm,
    frame: crate::value::ArrayFromAsyncContinuation,
    original_reason: Value,
) -> error::Result<()> {
    let pins = pin_array_from_async_frame(vm, &frame) + vm.pin(&original_reason);
    let result = array_from_async_close_inner(vm, frame, original_reason);
    vm.unpin_many(pins);
    result
}

fn array_from_async_close_inner(
    vm: &mut Vm,
    frame: crate::value::ArrayFromAsyncContinuation,
    original_reason: Value,
) -> error::Result<()> {
    if frame.sync_iterator {
        let returned =
            vm.async_from_sync_iterator_close_start_in_env(&frame.iterator, frame.realm)?;
        return await_array_from_async(
            vm,
            frame,
            returned,
            crate::value::ArrayFromAsyncAwaitKind::IteratorClose { original_reason },
        );
    }
    let return_method =
        match array_from_async_get_method(vm, &frame.iterator, PropertyKey::from("return")) {
            Ok(method) => method,
            Err(error) if !error.catchable() => return Err(error),
            Err(_) => return settle_array_from_async(vm, &frame, original_reason, true),
        };
    let Some(return_method) = return_method else {
        return settle_array_from_async(vm, &frame, original_reason, true);
    };
    let returned = match vm.call_function(&return_method, &[], Some(frame.iterator.clone())) {
        Ok(value) => value,
        Err(error) if !error.catchable() => return Err(error),
        Err(_) => return settle_array_from_async(vm, &frame, original_reason, true),
    };
    await_array_from_async(
        vm,
        frame,
        returned,
        crate::value::ArrayFromAsyncAwaitKind::IteratorClose { original_reason },
    )
}

pub(crate) fn run_array_from_async_reaction(
    vm: &mut Vm,
    frame: crate::value::ArrayFromAsyncContinuation,
    promise: GcIdx,
) -> error::Result<()> {
    let pins = vm.pin_many(&[
        Value::Object(promise),
        frame.capability.promise.clone(),
        frame.capability.resolve.clone(),
        frame.capability.reject.clone(),
        frame.target.clone(),
        frame.source.clone(),
        frame.iterator.clone(),
        frame.next_method.clone(),
        frame.mapper.clone(),
        frame.this_arg.clone(),
    ]);
    let result = run_array_from_async_reaction_inner(vm, frame, promise);
    vm.unpin_many(pins);
    result
}

fn run_array_from_async_reaction_inner(
    vm: &mut Vm,
    frame: crate::value::ArrayFromAsyncContinuation,
    promise: GcIdx,
) -> error::Result<()> {
    let (status, result) = vm.heap.with_obj(promise.0, |object| {
        if let HeapObj::Promise(data) = object {
            (*data.state.lock(), data.result.lock().clone())
        } else {
            (crate::value::PromiseStatus::Rejected, Value::Undefined)
        }
    });
    let rejected = status == crate::value::PromiseStatus::Rejected;
    use crate::value::ArrayFromAsyncAwaitKind as AwaitKind;
    match frame.await_kind {
        AwaitKind::IteratorNext => {
            if rejected {
                return settle_array_from_async(vm, &frame, result, true);
            }
            if !matches!(result, Value::Object(_)) {
                let error = Error::type_err("Iterator result is not an object");
                return reject_array_from_async_error(vm, &frame, &error);
            }
            let result_pin = vm.pin(&result);
            let fields = (|| -> error::Result<(Value, bool)> {
                let done = vm.get_property(&result, "done")?.is_truthy();
                if done {
                    Ok((Value::Undefined, true))
                } else {
                    Ok((vm.get_property(&result, "value")?, false))
                }
            })();
            vm.unpin_many(result_pin);
            match fields {
                Ok((_, true)) => array_from_async_finish(vm, frame),
                Ok((value, false)) => array_from_async_map_or_define(vm, frame, value, true),
                Err(error) => reject_array_from_async_error(vm, &frame, &error),
            }
        }
        AwaitKind::MappedValue => {
            if rejected {
                array_from_async_close(vm, frame, result)
            } else {
                array_from_async_define_and_continue(vm, frame, result, true)
            }
        }
        AwaitKind::ArrayLikeValue => {
            if rejected {
                settle_array_from_async(vm, &frame, result, true)
            } else {
                array_from_async_map_or_define(vm, frame, result, false)
            }
        }
        AwaitKind::ArrayLikeMappedValue => {
            if rejected {
                settle_array_from_async(vm, &frame, result, true)
            } else {
                array_from_async_define_and_continue(vm, frame, result, false)
            }
        }
        AwaitKind::IteratorClose {
            ref original_reason,
        } => settle_array_from_async(vm, &frame, original_reason.clone(), true),
    }
}

pub(crate) fn array_from_async(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let realm = env::global_env_root(&vm.heap, vm.native_callee_closure().unwrap_or(vm.global));
    let constructor = vm.promise_constructor_for_env(realm);
    let capability = new_promise_capability_in_env(vm, constructor, realm)?;
    let promise = capability.promise.clone();
    let items = args.first().cloned().unwrap_or(Value::Undefined);
    let mapper = args.get(1).cloned().unwrap_or(Value::Undefined);
    let this_arg = args.get(2).cloned().unwrap_or(Value::Undefined);
    let array_constructor = this.unwrap_or(Value::Undefined);
    let reaction_capability = crate::value::PromiseReactionCapability {
        promise: capability.promise,
        resolve: capability.resolve,
        reject: capability.reject,
    };
    let pins = vm.pin_many(&[
        promise.clone(),
        reaction_capability.resolve.clone(),
        reaction_capability.reject.clone(),
        items.clone(),
        mapper.clone(),
        this_arg.clone(),
        array_constructor.clone(),
    ]);
    let setup = (|| -> error::Result<()> {
        let initial = crate::value::ArrayFromAsyncContinuation {
            capability: reaction_capability,
            realm,
            target: Value::Undefined,
            source: Value::Undefined,
            iterator: Value::Undefined,
            next_method: Value::Undefined,
            sync_iterator: false,
            mapper,
            this_arg,
            index: 0,
            length: 0,
            await_kind: crate::value::ArrayFromAsyncAwaitKind::IteratorNext,
        };
        if !initial.mapper.is_undefined() && !is_callable(&initial.mapper, &vm.heap) {
            let error = Error::type_err("Array.fromAsync mapper is not callable");
            return reject_array_from_async_error(vm, &initial, &error);
        }
        let async_method = array_from_async_get_method(
            vm,
            &items,
            PropertyKey::symbol(vm.well_known_symbols.async_iterator),
        );
        let async_method = match async_method {
            Ok(method) => method,
            Err(error) => return reject_array_from_async_error(vm, &initial, &error),
        };
        let (iterator_method, sync_iterator) = if let Some(method) = async_method {
            (Some(method), false)
        } else {
            let method = array_from_async_get_method(
                vm,
                &items,
                PropertyKey::symbol(vm.well_known_symbols.iterator),
            );
            match method {
                Ok(method) => (method, true),
                Err(error) => return reject_array_from_async_error(vm, &initial, &error),
            }
        };
        if let Some(iterator_method) = iterator_method {
            let iterator = match vm.call_function(&iterator_method, &[], Some(items.clone())) {
                Ok(value) if matches!(value, Value::Object(_)) => value,
                Ok(_) => {
                    let error = Error::type_err("iterator method must return an object");
                    return reject_array_from_async_error(vm, &initial, &error);
                }
                Err(error) => return reject_array_from_async_error(vm, &initial, &error),
            };
            let iterator_pin = vm.pin(&iterator);
            let next_method = vm.get_property(&iterator, "next");
            vm.unpin_many(iterator_pin);
            let next_method = match next_method {
                Ok(method) => method,
                Err(error) => return reject_array_from_async_error(vm, &initial, &error),
            };
            let iterator = if sync_iterator {
                let record_pins = vm.pin_many(&[iterator.clone(), next_method.clone()]);
                let record = vm.new_async_from_sync_iterator(iterator, next_method.clone());
                vm.unpin_many(record_pins);
                match record {
                    Ok(iterator) => iterator,
                    Err(error) => return reject_array_from_async_error(vm, &initial, &error),
                }
            } else {
                iterator
            };
            let iterator_pins = vm.pin_many(&[iterator.clone(), next_method.clone()]);
            let target = if vm.is_constructor_value(&array_constructor) {
                match vm.construct(&array_constructor, &[]) {
                    Ok(value) => value,
                    Err(error) => {
                        vm.unpin_many(iterator_pins);
                        return reject_array_from_async_error(vm, &initial, &error);
                    }
                }
            } else {
                match array_create_in_realm(vm, 0, realm) {
                    Ok(value) => value,
                    Err(error) => {
                        vm.unpin_many(iterator_pins);
                        return reject_array_from_async_error(vm, &initial, &error);
                    }
                }
            };
            let result = array_from_async_next(
                vm,
                crate::value::ArrayFromAsyncContinuation {
                    target,
                    iterator,
                    next_method,
                    sync_iterator,
                    ..initial
                },
            );
            vm.unpin_many(iterator_pins);
            return result;
        }

        if items.is_nullish() {
            let error = Error::type_err("Cannot convert undefined or null to object");
            return reject_array_from_async_error(vm, &initial, &error);
        }
        let source = match vm.to_object(&items) {
            Ok(value) => value,
            Err(error) => return reject_array_from_async_error(vm, &initial, &error),
        };
        let source_pin = vm.pin(&source);
        let result = (|| -> error::Result<()> {
            let length = match length_of_array_like(vm, &source) {
                Ok(length) => length,
                Err(error) => return reject_array_from_async_error(vm, &initial, &error),
            };
            let target = if vm.is_constructor_value(&array_constructor) {
                match vm.construct(&array_constructor, &[Value::Number(length as f64)]) {
                    Ok(value) => value,
                    Err(error) => return reject_array_from_async_error(vm, &initial, &error),
                }
            } else {
                match array_create_in_realm(vm, length, realm) {
                    Ok(value) => value,
                    Err(error) => return reject_array_from_async_error(vm, &initial, &error),
                }
            };
            let frame = crate::value::ArrayFromAsyncContinuation {
                target,
                source,
                length,
                ..initial
            };
            let frame_pins = pin_array_from_async_frame(vm, &frame);
            let result = if length == 0 {
                array_from_async_finish(vm, frame)
            } else {
                let first = match vm.get_property(&frame.source, "0") {
                    Ok(value) => value,
                    Err(error) => {
                        let result = reject_array_from_async_error(vm, &frame, &error);
                        vm.unpin_many(frame_pins);
                        return result;
                    }
                };
                await_array_from_async(
                    vm,
                    frame,
                    first,
                    crate::value::ArrayFromAsyncAwaitKind::ArrayLikeValue,
                )
            };
            vm.unpin_many(frame_pins);
            result
        })();
        vm.unpin(source_pin);
        result
    })();
    vm.unpin_many(pins);
    setup?;
    Ok(promise)
}

pub(crate) fn array_from(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let src_val = args.first().cloned().unwrap_or(Value::Undefined);
    let constructor = this.unwrap_or(Value::Undefined);
    let map_fn = match args.get(1) {
        None | Some(Value::Undefined) => None,
        Some(value) if is_callable(value, &vm.heap) => Some(value.clone()),
        Some(_) => return Err(Error::type_err("Array.from mapper is not callable")),
    };
    let this_arg = args.get(2).cloned().unwrap_or(Value::Undefined);
    // Cap total materialized elements so an infinite or huge iterable (e.g.
    // a generator that yields forever) cannot OOM the host. 65k keeps an
    // infinite iterable from running for many seconds before the cap trips.
    const MAX_ARRAY_FROM_LEN: usize = 1 << 16; // 65,536
    if src_val.is_nullish() {
        return Err(Error::type_err("Array.from requires an array-like value"));
    }

    let iterator_key = PropertyKey::symbol(vm.well_known_symbols.iterator);
    let iterator_method = vm.get_property_by_key(&src_val, &iterator_key)?;
    if !iterator_method.is_nullish() {
        if !is_callable(&iterator_method, &vm.heap) {
            return Err(Error::type_err("iterator method is not callable"));
        }
        let mut pin_count = vm.pin(&iterator_method);
        let result = (|| -> error::Result<Value> {
            let target = if vm.is_constructor_value(&constructor) {
                vm.construct(&constructor, &[])?
            } else {
                make_value_array(vm, Vec::new())?
            };
            pin_count += vm.pin(&target);
            let iter_obj = vm.call_function(&iterator_method, &[], Some(src_val.clone()))?;
            let iter = vm.new_lazy_iterator(iter_obj)?;
            pin_count += vm.pin(&iter);
            let mut index = 0usize;
            loop {
                let (mut value, done) = vm.iterator_next(&iter)?;
                if done {
                    break;
                }
                if index >= MAX_ARRAY_FROM_LEN {
                    let _ = vm.iterator_close(&iter);
                    return Err(Error::range("Invalid array length"));
                }
                if let Some(mapper) = &map_fn {
                    value = match vm.call_function(
                        mapper,
                        &[value, Value::Number(index as f64)],
                        Some(this_arg.clone()),
                    ) {
                        Ok(value) => value,
                        Err(error) => {
                            let _ = vm.iterator_close(&iter);
                            return Err(error);
                        }
                    };
                }
                let value_pin = vm.pin(&value);
                let define_result = vm.define_own_property_or_throw(
                    &target,
                    PropertyKey::from_integer_index(index as u64),
                    PropertyDescriptor::data(value),
                );
                vm.unpin_many(value_pin);
                if let Err(error) = define_result {
                    let _ = vm.iterator_close(&iter);
                    return Err(error);
                }
                index += 1;
            }
            vm.set_property_strict(&target, "length", Value::Number(index as f64))?;
            Ok(target)
        })();
        vm.unpin_many(pin_count);
        return result;
    }

    let len_value = vm.get_property(&src_val, "length")?;
    let len_number = vm.to_number(&len_value)?;
    let len = if len_number.is_nan() || len_number <= 0.0 {
        0
    } else {
        len_number.trunc().min((MAX_ARRAY_FROM_LEN + 1) as f64) as usize
    };
    if len > MAX_ARRAY_FROM_LEN {
        return Err(Error::range("Invalid array length"));
    }
    let target = if vm.is_constructor_value(&constructor) {
        vm.construct(&constructor, &[Value::Number(len as f64)])?
    } else {
        make_value_array(vm, Vec::new())?
    };
    let mut pin_count = vm.pin(&target);
    let result = (|| -> error::Result<Value> {
        for index in 0..len {
            let mut value = vm.get_property(&src_val, &index.to_string())?;
            if let Some(mapper) = &map_fn {
                value = vm.call_function(
                    mapper,
                    &[value, Value::Number(index as f64)],
                    Some(this_arg.clone()),
                )?;
            }
            let value_pin = vm.pin(&value);
            let define_result = vm.define_own_property_or_throw(
                &target,
                PropertyKey::from_integer_index(index as u64),
                PropertyDescriptor::data(value),
            );
            vm.unpin_many(value_pin);
            define_result?;
        }
        vm.set_property_strict(&target, "length", Value::Number(len as f64))?;
        Ok(target.clone())
    })();
    vm.unpin_many(pin_count);
    result
}
pub(crate) fn array_of(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let len = args.len();
    let constructor = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin_many(args);
    pin_count += vm.pin(&constructor);

    let completion = (|| {
        let result = if vm.is_constructor_value(&constructor) {
            vm.construct(&constructor, &[Value::Number(len as f64)])?
        } else {
            make_value_array(vm, Vec::new())?
        };
        pin_count += vm.pin(&result);

        for (i, item) in args.iter().enumerate() {
            vm.define_own_property_or_throw(
                &result,
                PropertyKey::from_integer_index(i as u64),
                PropertyDescriptor::data(item.clone()),
            )?;
        }
        vm.set_property_strict(&result, "length", Value::Number(len as f64))?;
        Ok(result)
    })();
    vm.unpin_many(pin_count);
    completion
}

pub(crate) fn array_is_array(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    is_array_or_throw(vm, args.first().unwrap_or(&Value::Undefined)).map(Value::Bool)
}
pub(crate) fn array_push(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let arg_count = u64::try_from(args.len())
            .map_err(|_| Error::type_err("Array.prototype.push result is too large"))?;
        let new_len = len
            .checked_add(arg_count)
            .filter(|length| *length <= MAX_SAFE_ARRAY_LENGTH_U64)
            .ok_or_else(|| Error::type_err("Array.prototype.push result is too large"))?;

        for (index, item) in (len..new_len).zip(args.iter()) {
            vm.consume_fuel()?;
            vm.set_property_strict(&object, &index.to_string(), item.clone())?;
        }
        vm.set_property_strict(&object, "length", Value::Number(new_len as f64))?;
        Ok(Value::Number(new_len as f64))
    })();
    vm.unpin_many(pin_count);
    result
}
pub(crate) fn array_pop(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        if len == 0 {
            vm.set_property_strict(&object, "length", Value::Number(0.0))?;
            return Ok(Value::Undefined);
        }

        let new_len = len - 1;
        let key = new_len.to_string();
        let element = vm.get_property(&object, &key)?;
        pin_count += vm.pin(&element);
        delete_property_or_throw(vm, &object, &key)?;
        vm.set_property_strict(&object, "length", Value::Number(new_len as f64))?;
        Ok(element)
    })();
    vm.unpin_many(pin_count);
    result
}
pub(crate) fn array_to_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    if receiver.is_nullish() {
        return Err(Error::type_err(
            "Array.prototype.toString called on null or undefined",
        ));
    }
    let object = vm.to_object(&receiver)?;
    let join = vm.get_property(&object, "join")?;
    if is_callable(&join, &vm.heap) {
        vm.call_function(&join, &[], Some(object))
    } else {
        object_to_string(vm, Some(object), None)
    }
}

pub(crate) fn array_to_locale_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    fn append(result: &mut String, value: &str) -> error::Result<()> {
        result
            .try_reserve(value.len())
            .map_err(|_| Error::range("Array toLocaleString result too large"))?;
        result.push_str(value);
        Ok(())
    }

    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    let completion = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let Value::Object(object_idx) = object else {
            unreachable!("ToObject must return an object")
        };
        if vm.active_array_joins.contains(&object_idx) {
            return Ok(Value::String(Arc::from("")));
        }
        vm.active_array_joins.push(object_idx);
        let locale_result = (|| {
            let mut result = String::new();
            let mut index = 0u64;
            while index < len {
                vm.consume_fuel()?;
                if index > 0 {
                    append(&mut result, ",")?;
                }

                let element = vm.get_property(&object, &index.to_string())?;
                if !element.is_nullish() {
                    let element_pin = vm.pin(&element);
                    let element_result: error::Result<Arc<str>> = (|| {
                        let method = vm.get_property(&element, "toLocaleString")?;
                        if !is_callable(&method, &vm.heap) {
                            return Err(Error::type_err("toLocaleString is not callable"));
                        }
                        let method_pin = vm.pin(&method);
                        let localized = vm.call_function(&method, &[], Some(element.clone()));
                        vm.unpin(method_pin);
                        let localized = localized?;

                        let localized_pin = vm.pin(&localized);
                        let string = vm.to_string(&localized);
                        vm.unpin(localized_pin);
                        string
                    })();
                    vm.unpin(element_pin);
                    let element_string = element_result?;
                    append(&mut result, element_string.as_ref())?;
                }
                index += 1;
            }
            Ok(Value::String(Arc::from(result)))
        })();
        let active = vm
            .active_array_joins
            .pop()
            .expect("active Array stringification marker must be balanced");
        debug_assert_eq!(active, object_idx);
        locale_result
    })();
    vm.unpin_many(pin_count);
    completion
}

pub(crate) fn array_join(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    fn append(result: &mut String, value: &str) -> error::Result<()> {
        result
            .try_reserve(value.len())
            .map_err(|_| Error::range("Array join result too large"))?;
        result.push_str(value);
        Ok(())
    }

    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let completion = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let separator = match args.first() {
            Some(value) if !value.is_undefined() => vm.to_string(value)?.to_string(),
            _ => ",".to_string(),
        };
        let Value::Object(object_idx) = object else {
            unreachable!("ToObject must return an object")
        };
        if vm.active_array_joins.contains(&object_idx) {
            return Ok(Value::String(Arc::from("")));
        }
        vm.active_array_joins.push(object_idx);
        let join_result = (|| {
            let mut result = String::new();
            let mut index = 0u64;
            while index < len {
                vm.consume_fuel()?;
                if index > 0 {
                    append(&mut result, &separator)?;
                }
                let element = vm.get_property(&object, &index.to_string())?;
                if !element.is_nullish() {
                    let element_pin = vm.pin(&element);
                    let element_string = vm.to_string(&element);
                    vm.unpin(element_pin);
                    append(&mut result, element_string?.as_ref())?;
                }
                index += 1;
            }
            Ok(Value::String(Arc::from(result)))
        })();
        let active = vm
            .active_array_joins
            .pop()
            .expect("active Array join marker must be balanced");
        debug_assert_eq!(active, object_idx);
        join_result
    })();
    vm.unpin_many(pin_count);
    completion
}
pub(crate) fn array_map(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let callback = get_arg(args, 0);
        if !is_callable(&callback, &vm.heap) {
            return Err(Error::type_err("Array mapper is not callable"));
        }
        let this_arg = get_arg(args, 1);
        let result = array_species_create(vm, &object, len)?;
        pin_count += vm.pin(&result);

        let mut index = 0u64;
        while index < len {
            vm.consume_fuel()?;
            let key = index.to_string();
            if vm.has_property(&object, &key)? {
                let value = vm.get_property(&object, &key)?;
                let value_pin = vm.pin(&value);
                let mapped = vm.call_function(
                    &callback,
                    &[value, Value::Number(index as f64), object.clone()],
                    Some(this_arg.clone()),
                );
                vm.unpin(value_pin);
                let mapped = mapped?;
                let mapped_pin = vm.pin(&mapped);
                let define = vm.define_own_property_or_throw(
                    &result,
                    PropertyKey::from(key),
                    PropertyDescriptor::data(mapped),
                );
                vm.unpin(mapped_pin);
                define?;
            }
            index += 1;
        }
        Ok(result)
    })();
    vm.unpin_many(pin_count);
    result
}
pub(crate) fn array_filter(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let callback = get_arg(args, 0);
        if !is_callable(&callback, &vm.heap) {
            return Err(Error::type_err("Array predicate is not callable"));
        }
        let this_arg = get_arg(args, 1);
        let result = array_species_create(vm, &object, 0)?;
        pin_count += vm.pin(&result);

        let mut source_index = 0u64;
        let mut target_index = 0u64;
        while source_index < len {
            vm.consume_fuel()?;
            let source_key = source_index.to_string();
            if vm.has_property(&object, &source_key)? {
                let value = vm.get_property(&object, &source_key)?;
                let value_pin = vm.pin(&value);
                let selected = vm.call_function(
                    &callback,
                    &[
                        value.clone(),
                        Value::Number(source_index as f64),
                        object.clone(),
                    ],
                    Some(this_arg.clone()),
                );
                let selected = match selected {
                    Ok(selected) => selected,
                    Err(error) => {
                        vm.unpin(value_pin);
                        return Err(error);
                    }
                };
                if selected.is_truthy() {
                    let define = vm.define_own_property_or_throw(
                        &result,
                        PropertyKey::from_integer_index(target_index),
                        PropertyDescriptor::data(value),
                    );
                    vm.unpin(value_pin);
                    define?;
                    target_index += 1;
                } else {
                    vm.unpin(value_pin);
                }
            }
            source_index += 1;
        }
        Ok(result)
    })();
    vm.unpin_many(pin_count);
    result
}
pub(crate) fn array_reduce(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let mut accumulator_pin = 0;
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let callback = get_arg(args, 0);
        if !is_callable(&callback, &vm.heap) {
            return Err(Error::type_err("Array reducer is not callable"));
        }

        let mut index = 0u64;
        let mut accumulator = if args.len() >= 2 {
            get_arg(args, 1)
        } else {
            let mut found = None;
            while index < len {
                vm.consume_fuel()?;
                let key = index.to_string();
                if vm.has_property(&object, &key)? {
                    found = Some(vm.get_property(&object, &key)?);
                    index += 1;
                    break;
                }
                index += 1;
            }
            found.ok_or_else(|| Error::type_err("Reduce of empty array with no initial value"))?
        };
        accumulator_pin = vm.pin(&accumulator);

        while index < len {
            vm.consume_fuel()?;
            let key = index.to_string();
            if vm.has_property(&object, &key)? {
                let value = vm.get_property(&object, &key)?;
                let value_pin = vm.pin(&value);
                let next = vm.call_function(
                    &callback,
                    &[
                        accumulator,
                        value,
                        Value::Number(index as f64),
                        object.clone(),
                    ],
                    Some(Value::Undefined),
                );
                vm.unpin(value_pin);
                let next = next?;
                let next_pin = vm.pin(&next);
                vm.unpin_many(next_pin + accumulator_pin);
                accumulator = next;
                accumulator_pin = vm.pin(&accumulator);
            }
            index += 1;
        }
        Ok(accumulator)
    })();
    vm.unpin(accumulator_pin);
    vm.unpin_many(pin_count);
    result
}
/// Build a heap array from a Vec of values.
pub(crate) fn make_array(vm: &mut Vm, items: Vec<Value>) -> error::Result<Value> {
    let idx = vm
        .heap
        .allocate(HeapObj::Array(crate::value::ArrayData::new(
            items,
            Some(vm.array_proto.clone()),
        )))?;
    Ok(Value::Object(GcIdx(idx)))
}

/// Normalize an array index argument (negative wraps from end).
pub(crate) fn norm_index(v: Value, len: f64, vm: &mut Vm) -> error::Result<usize> {
    let n = vm.to_number(&v)?;
    if n < 0.0 {
        Ok(((len + n).max(0.0)) as usize)
    } else {
        Ok((n as usize).min(len as usize))
    }
}

fn validate_sort_compare_fn(vm: &Vm, cmp: &Option<Value>) -> error::Result<()> {
    if let Some(compare_fn) = cmp {
        if !compare_fn.is_undefined() && !is_callable(compare_fn, &vm.heap) {
            return Err(Error::type_err("Array sort comparator is not callable"));
        }
    }
    Ok(())
}

const MAX_MATERIALIZED_ARRAY_SORT_LENGTH: usize = crate::value::MAX_DENSE_ARRAY_LEN;

#[derive(Clone, Copy)]
enum SortIndexedPropertiesMode {
    SkipHoles,
    ReadThroughHoles,
}

fn array_sort_object_and_length(
    vm: &mut Vm,
    this: Option<Value>,
    compare_fn: &Option<Value>,
    pin_count: &mut usize,
) -> error::Result<(Value, usize)> {
    if let Some(compare_fn) = compare_fn {
        *pin_count += vm.pin(compare_fn);
    }

    let receiver = this.unwrap_or(Value::Undefined);
    if receiver.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let object = vm.to_object(&receiver)?;
    *pin_count += vm.pin(&object);
    let len = length_of_array_like(vm, &object)?;
    Ok((object, len))
}

fn ensure_array_sort_materialization_limit(len: usize) -> error::Result<()> {
    if len > MAX_MATERIALIZED_ARRAY_SORT_LENGTH {
        return Err(Error::range("Array sort input too large"));
    }
    Ok(())
}

fn collect_sort_indexed_properties(
    vm: &mut Vm,
    object: &Value,
    len: usize,
    mode: SortIndexedPropertiesMode,
) -> error::Result<(Vec<Value>, usize)> {
    let mut items = Vec::new();
    let mut pin_count = 0;
    let completion = (|| {
        for index in 0..len {
            vm.consume_fuel()?;
            let key = index.to_string();
            let read = match mode {
                SortIndexedPropertiesMode::SkipHoles => vm.has_property(object, &key)?,
                SortIndexedPropertiesMode::ReadThroughHoles => true,
            };
            if !read {
                continue;
            }
            let value = vm.get_property(object, &key)?;
            // A later HasProperty/Get can remove the source edge and collect.
            pin_count += vm.pin(&value);
            items.push(value);
        }
        Ok(())
    })();
    if let Err(error) = completion {
        vm.unpin_many(pin_count);
        return Err(error);
    }
    Ok((items, pin_count))
}

fn compare_array_sort_undefined(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a.is_undefined(), b.is_undefined()) {
        (true, true) => Some(std::cmp::Ordering::Equal),
        (true, false) => Some(std::cmp::Ordering::Greater),
        (false, true) => Some(std::cmp::Ordering::Less),
        (false, false) => None,
    }
}

/// Sort items with an optional comparator callback (default: string compare).
/// Callers keep `items` and `cmp` rooted until the destination owns the values.
pub(crate) fn sort_with_cb(
    vm: &mut Vm,
    items: &mut [Value],
    cmp: &Option<Value>,
) -> error::Result<()> {
    match cmp {
        None | Some(Value::Undefined) => {
            let mut compare = |vm: &mut Vm, a: &Value, b: &Value| {
                if let Some(order) = compare_array_sort_undefined(a, b) {
                    return Ok(order);
                }
                let sa = vm.to_string(a)?;
                let sb = vm.to_string(b)?;
                let a_units = crate::value::utf16_from_str(&sa);
                let b_units = crate::value::utf16_from_str(&sb);
                Ok(a_units.cmp(&b_units))
            };
            merge_sort(vm, items, &mut compare)?;
        }
        Some(cmp_fn) => {
            // Stable O(n log n) merge sort. The previous O(n^2) bubble sort
            // made sorting 10k random elements with a comparator take ~30s (a
            // trivial DoS). A hand-rolled merge sort is used instead of
            // `slice::sort_by` because the ES comparator may have side
            // effects (mutating VM state during `call_function`), which
            // defeats pdqsort's purity assumptions and degrades it to O(n^2).
            // Merge sort compares each pair at most once per merge level and
            // stays O(n log n) regardless. The comparator result is rooted
            // across observable ToNumber conversion, and the first abrupt
            // completion stops comparison immediately.
            let mut compare = |vm: &mut Vm, a: &Value, b: &Value| {
                if let Some(order) = compare_array_sort_undefined(a, b) {
                    return Ok(order);
                }
                let result = vm.call_function(cmp_fn, &[a.clone(), b.clone()], None)?;
                let result_pin = vm.pin(&result);
                let number = vm.to_number(&result);
                vm.unpin_many(result_pin);
                let ord = number?;
                Ok(if ord.is_nan() {
                    std::cmp::Ordering::Equal
                } else if ord < 0.0 {
                    std::cmp::Ordering::Less
                } else if ord > 0.0 {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                })
            };
            merge_sort(vm, items, &mut compare)?;
        }
    }
    Ok(())
}

/// In-place stable merge sort. `compare` may mutate the VM (ES comparators
/// can have side effects); unlike `slice::sort_by`, this never degrades to
/// O(n^2) on an inconsistent/side-effecting comparator because each pair is
/// compared at most once along a given merge path.
fn merge_sort<F>(vm: &mut Vm, items: &mut [Value], compare: &mut F) -> error::Result<()>
where
    F: FnMut(&mut Vm, &Value, &Value) -> error::Result<std::cmp::Ordering>,
{
    let n = items.len();
    if n < 2 {
        return Ok(());
    }
    // Bottom-up merge sort with a scratch buffer.
    let mut buf: Vec<Value> = Vec::with_capacity(n);
    let mut width = 1;
    while width < n {
        let mut i = 0;
        while i < n {
            let left = i;
            let mid = (i + width).min(n);
            let right = (i + 2 * width).min(n);
            // Merge [left, mid) and [mid, right) into buf, then copy back.
            let mut a = left;
            let mut b = mid;
            buf.clear();
            while a < mid && b < right {
                vm.consume_fuel()?;
                if compare(vm, &items[a], &items[b])? == std::cmp::Ordering::Greater {
                    buf.push(items[b].clone());
                    b += 1;
                } else {
                    buf.push(items[a].clone());
                    a += 1;
                }
            }
            while a < mid {
                buf.push(items[a].clone());
                a += 1;
            }
            while b < right {
                buf.push(items[b].clone());
                b += 1;
            }
            items[left..right].clone_from_slice(&buf);
            i += 2 * width;
        }
        width *= 2;
    }
    Ok(())
}

pub(crate) fn array_reduce_right(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let mut accumulator_pin = 0;
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let callback = get_arg(args, 0);
        if !is_callable(&callback, &vm.heap) {
            return Err(Error::type_err("Array reducer is not callable"));
        }

        let mut index = len;
        let mut accumulator = if args.len() >= 2 {
            get_arg(args, 1)
        } else {
            let mut found = None;
            while index > 0 {
                index -= 1;
                vm.consume_fuel()?;
                let key = index.to_string();
                if vm.has_property(&object, &key)? {
                    found = Some(vm.get_property(&object, &key)?);
                    break;
                }
            }
            found.ok_or_else(|| Error::type_err("Reduce of empty array with no initial value"))?
        };
        accumulator_pin = vm.pin(&accumulator);

        while index > 0 {
            index -= 1;
            vm.consume_fuel()?;
            let key = index.to_string();
            if vm.has_property(&object, &key)? {
                let value = vm.get_property(&object, &key)?;
                let value_pin = vm.pin(&value);
                let next = vm.call_function(
                    &callback,
                    &[
                        accumulator,
                        value,
                        Value::Number(index as f64),
                        object.clone(),
                    ],
                    Some(Value::Undefined),
                );
                vm.unpin(value_pin);
                let next = next?;
                let next_pin = vm.pin(&next);
                vm.unpin_many(next_pin + accumulator_pin);
                accumulator = next;
                accumulator_pin = vm.pin(&accumulator);
            }
        }
        Ok(accumulator)
    })();
    vm.unpin(accumulator_pin);
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn array_to_reversed(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;

        // Change-array-by-copy methods intentionally bypass @@species. The
        // intrinsic result must exist before any indexed Get becomes visible.
        let result = array_create_u64_in_current_realm(vm, len)?;
        pin_count += vm.pin(&result);

        let mut index = 0u64;
        while index < len {
            vm.consume_fuel()?;
            let from = len - index - 1;
            let value = vm.get_property(&object, &from.to_string())?;
            let value_pin = vm.pin(&value);
            let define = vm.define_own_property_or_throw(
                &result,
                PropertyKey::from_integer_index(index),
                PropertyDescriptor::data(value),
            );
            vm.unpin(value_pin);
            define?;
            index += 1;
        }
        Ok(result.clone())
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn array_to_sorted(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let cb = args.first().cloned();
    validate_sort_compare_fn(vm, &cb)?;
    let mut pin_count = 0;
    let completion = (|| {
        let (object, len) = array_sort_object_and_length(vm, this, &cb, &mut pin_count)?;

        // ArrayCreate precedes SortIndexedProperties, including its indexed
        // Gets. Pin the fresh result before applying the sandbox list cap.
        let result = array_create_in_current_realm(vm, len)?;
        pin_count += vm.pin(&result);
        ensure_array_sort_materialization_limit(len)?;

        let (mut items, item_pins) = collect_sort_indexed_properties(
            vm,
            &object,
            len,
            SortIndexedPropertiesMode::ReadThroughHoles,
        )?;
        pin_count += item_pins;
        sort_with_cb(vm, &mut items, &cb)?;

        let Value::Object(result_idx) = result else {
            return Err(Error::internal("ArrayCreate returned a non-object"));
        };
        vm.heap.with_obj(result_idx.0, |obj| {
            if let HeapObj::Array(array) = obj {
                *array.items.lock() = items;
                array.present.lock().fill(true);
            }
        });
        Ok(Value::Object(result_idx))
    })();
    vm.unpin_many(pin_count);
    completion
}

pub(crate) fn array_to_spliced(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let actual_start =
            relative_array_index(to_integer_or_infinity(vm, &get_arg(args, 0))?, len);
        let insert_count = u64::try_from(args.len().saturating_sub(2))
            .map_err(|_| Error::type_err("Array.prototype.toSpliced result is too large"))?;
        let actual_skip_count = match args.len() {
            0 => 0,
            1 => len - actual_start,
            _ => {
                let skip_count = to_integer_or_infinity(vm, &args[1])?;
                if skip_count <= 0.0 {
                    0
                } else {
                    skip_count.min((len - actual_start) as f64) as u64
                }
            }
        };
        let new_len = len as u128 + insert_count as u128 - actual_skip_count as u128;
        if new_len > MAX_SAFE_ARRAY_LENGTH_U64 as u128 {
            return Err(Error::type_err(
                "Array.prototype.toSpliced result is too large",
            ));
        }
        let new_len = new_len as u64;

        // Change-array-by-copy deliberately ignores @@species. Allocation and
        // every argument coercion must complete before indexed source reads.
        let result = array_create_u64_in_current_realm(vm, new_len)?;
        pin_count += vm.pin(&result);

        let mut write_index = 0u64;
        while write_index < actual_start {
            vm.consume_fuel()?;
            let value = vm.get_property(&object, &write_index.to_string())?;
            let value_pin = vm.pin(&value);
            let define = vm.define_own_property_or_throw(
                &result,
                PropertyKey::from_integer_index(write_index),
                PropertyDescriptor::data(value),
            );
            vm.unpin(value_pin);
            define?;
            write_index += 1;
        }

        for item in args.iter().skip(2) {
            vm.consume_fuel()?;
            vm.define_own_property_or_throw(
                &result,
                PropertyKey::from_integer_index(write_index),
                PropertyDescriptor::data(item.clone()),
            )?;
            write_index += 1;
        }

        let mut read_index = actual_start + actual_skip_count;
        while write_index < new_len {
            vm.consume_fuel()?;
            let value = vm.get_property(&object, &read_index.to_string())?;
            let value_pin = vm.pin(&value);
            let define = vm.define_own_property_or_throw(
                &result,
                PropertyKey::from_integer_index(write_index),
                PropertyDescriptor::data(value),
            );
            vm.unpin(value_pin);
            define?;
            write_index += 1;
            read_index += 1;
        }
        Ok(result.clone())
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn array_with(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let relative_index = to_integer_or_infinity(vm, &get_arg(args, 0))?;
        let actual_index = if relative_index >= 0.0 {
            relative_index
        } else {
            len as f64 + relative_index
        };
        if actual_index < 0.0 || actual_index >= len as f64 {
            return Err(Error::range("Invalid array index"));
        }
        if len > crate::value::MAX_DENSE_ARRAY_LEN as u64 {
            return Err(Error::range("Array.with result too large"));
        }

        let result = array_create_u64_in_current_realm(vm, len)?;
        pin_count += vm.pin(&result);
        let replacement = get_arg(args, 1);
        let actual_index = actual_index as u64;
        let mut index = 0;
        while index < len {
            vm.consume_fuel()?;
            let value = if index == actual_index {
                replacement.clone()
            } else {
                vm.get_property(&object, &index.to_string())?
            };
            let value_pin = vm.pin(&value);
            let define = vm.define_own_property_or_throw(
                &result,
                PropertyKey::from_integer_index(index),
                PropertyDescriptor::data(value),
            );
            vm.unpin(value_pin);
            define?;
            index += 1;
        }
        Ok(result.clone())
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn array_for_each(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let callback = get_arg(args, 0);
        if !is_callable(&callback, &vm.heap) {
            return Err(Error::type_err("Array callback is not callable"));
        }
        let this_arg = get_arg(args, 1);

        let mut index = 0u64;
        while index < len {
            vm.consume_fuel()?;
            let key = index.to_string();
            if vm.has_property(&object, &key)? {
                let value = vm.get_property(&object, &key)?;
                let value_pin = vm.pin(&value);
                let callback_result = vm.call_function(
                    &callback,
                    &[value.clone(), Value::Number(index as f64), object.clone()],
                    Some(this_arg.clone()),
                );
                vm.unpin(value_pin);
                callback_result?;
            }
            index += 1;
        }
        Ok(Value::Undefined)
    })();
    vm.unpin_many(pin_count);
    result
}
/// Resolve a `fromIndex`-style argument (ToInteger, default 0) to a starting
/// position clamped into `[0, len]`. Negative wraps from the end.
pub(crate) fn from_index_arg(
    vm: &mut Vm,
    args: &[Value],
    idx: usize,
    len: usize,
) -> error::Result<usize> {
    let raw = match args.get(idx) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    if raw.is_nan() || raw == 0.0 || raw.is_infinite() {
        // +Inf -> len, -Inf/-0/NaN -> 0
        return Ok(if raw.is_infinite() && raw > 0.0 {
            len
        } else {
            0
        });
    }
    let n = raw as i64;
    let start = if n < 0 {
        (len as i64 + n).max(0) as usize
    } else {
        (n as usize).min(len)
    };
    Ok(start)
}

const MAX_SAFE_ARRAY_LENGTH: f64 = 9_007_199_254_740_991.0;
const MAX_SAFE_ARRAY_LENGTH_U64: u64 = 9_007_199_254_740_991;
const MAX_FLATTEN_CYCLE_REPLAYS: usize = 512;

fn to_length(vm: &mut Vm, value: &Value) -> error::Result<usize> {
    let n = vm.to_number(value)?;
    if n.is_nan() || n <= 0.0 {
        return Ok(0);
    }
    if n.is_infinite() {
        return Ok(MAX_SAFE_ARRAY_LENGTH as usize);
    }
    Ok(n.trunc().min(MAX_SAFE_ARRAY_LENGTH) as usize)
}

fn length_of_array_like(vm: &mut Vm, value: &Value) -> error::Result<usize> {
    let len = vm.get_property(value, "length")?;
    let pin_count = vm.pin(&len);
    let completion = to_length(vm, &len);
    vm.unpin_many(pin_count);
    completion
}

fn to_length_u64(vm: &mut Vm, value: &Value) -> error::Result<u64> {
    let number = vm.to_number(value)?;
    if number.is_nan() || number <= 0.0 {
        return Ok(0);
    }
    if number.is_infinite() {
        return Ok(MAX_SAFE_ARRAY_LENGTH_U64);
    }
    Ok(number.trunc().min(MAX_SAFE_ARRAY_LENGTH) as u64)
}

pub(super) fn length_of_array_like_u64(vm: &mut Vm, value: &Value) -> error::Result<u64> {
    let length = vm.get_property(value, "length")?;
    let pin_count = vm.pin(&length);
    let result = to_length_u64(vm, &length);
    vm.unpin_many(pin_count);
    result
}

pub(super) fn array_method_to_object(vm: &mut Vm, receiver: &Value) -> error::Result<Value> {
    if receiver.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    vm.to_object(receiver)
}

fn to_integer_or_infinity(vm: &mut Vm, value: &Value) -> error::Result<f64> {
    let number = vm.to_number(value)?;
    if number.is_nan() || number == 0.0 {
        Ok(0.0)
    } else if number.is_infinite() {
        Ok(number)
    } else {
        Ok(number.trunc())
    }
}

fn relative_array_index(integer: f64, len: u64) -> u64 {
    if integer == f64::NEG_INFINITY {
        return 0;
    }
    if integer < 0.0 {
        return (len as f64 + integer).max(0.0) as u64;
    }
    if integer == f64::INFINITY {
        return len;
    }
    integer.min(len as f64) as u64
}

fn array_create_u64_in_current_realm(vm: &mut Vm, length: u64) -> error::Result<Value> {
    if length > u32::MAX as u64 {
        return Err(Error::range("Invalid array length"));
    }
    let length = usize::try_from(length).map_err(|_| Error::range("Invalid array length"))?;
    array_create_in_current_realm(vm, length)
}

fn array_species_create(vm: &mut Vm, original: &Value, length: u64) -> error::Result<Value> {
    let mut pin_count = vm.pin(original);
    let result = (|| {
        if !is_array_or_throw(vm, original)? {
            return array_create_u64_in_current_realm(vm, length);
        }

        let mut constructor = vm.get_property(original, "constructor")?;
        pin_count += vm.pin(&constructor);
        if vm.is_constructor_value(&constructor) {
            let current_realm = vm.current_realm_global_env();
            let constructor_realm = vm.constructor_realm(&constructor)?;
            let is_foreign_intrinsic = constructor_realm != current_realm
                && vm
                    .realm_array_constructors
                    .get(&constructor_realm.0)
                    .is_some_and(|intrinsic| intrinsic == &constructor);
            if is_foreign_intrinsic {
                constructor = Value::Undefined;
            }
        }
        if matches!(constructor, Value::Object(_)) {
            let species_key = PropertyKey::symbol(vm.well_known_symbols.species);
            let species = vm.get_property_by_key(&constructor, &species_key)?;
            pin_count += vm.pin(&species);
            constructor = if matches!(species, Value::Null) {
                Value::Undefined
            } else {
                species
            };
        }
        if constructor.is_undefined() {
            return array_create_u64_in_current_realm(vm, length);
        }
        if !vm.is_constructor_value(&constructor) {
            return Err(Error::type_err("Array species is not a constructor"));
        }
        vm.construct(&constructor, &[Value::Number(length as f64)])
    })();
    vm.unpin_many(pin_count);
    result
}

fn is_concat_spreadable(vm: &mut Vm, value: &Value) -> error::Result<bool> {
    if !value.is_object() {
        return Ok(false);
    }
    let key = PropertyKey::symbol(vm.well_known_symbols.is_concat_spreadable);
    let spreadable = vm.get_property_by_key(value, &key)?;
    if !spreadable.is_undefined() {
        return Ok(spreadable.is_truthy());
    }
    is_array_or_throw(vm, value)
}

fn delete_property_or_throw(vm: &mut Vm, object: &Value, key: &str) -> error::Result<()> {
    if vm.delete_property(object, key)? {
        Ok(())
    } else {
        Err(Error::type_err(format!(
            "Cannot delete property '{}' of object",
            key
        )))
    }
}

fn array_find_object_and_callback(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<(Value, usize, Value, Value)> {
    let receiver = this.unwrap_or(Value::Undefined);
    if receiver.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let object = vm.to_object(&receiver)?;
    let len = length_of_array_like(vm, &object)?;
    let callback = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&callback, &vm.heap) {
        return Err(Error::type_err("Array predicate is not callable"));
    }
    let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);
    Ok((object, len, callback, this_arg))
}

fn array_find_value_at(vm: &mut Vm, object: &Value, index: usize) -> error::Result<Value> {
    vm.get_property(object, &index.to_string())
}

pub(crate) fn array_search_start(
    vm: &mut Vm,
    args: &[Value],
    len: usize,
    default: f64,
) -> error::Result<Option<usize>> {
    if len == 0 {
        return Ok(None);
    }
    let raw = match args.get(1) {
        Some(v) => vm.to_number(v)?,
        None => default,
    };
    if raw.is_nan() {
        return Ok(Some(0));
    }
    if raw.is_infinite() {
        return Ok(if raw.is_sign_positive() {
            Some(len)
        } else {
            Some(0)
        });
    }
    let n = raw.trunc();
    if n < 0.0 {
        Ok(Some(((len as f64 + n).max(0.0)) as usize))
    } else {
        Ok(Some((n as usize).min(len)))
    }
}

fn array_search_has_property(vm: &mut Vm, object: &Value, key: &str) -> error::Result<bool> {
    if vm.has_property(object, key)? {
        return Ok(true);
    }
    match object {
        Value::Bool(_) | Value::Number(_) | Value::BigInt(_) | Value::Symbol(_) => {
            Ok(!vm.get_property(object, key)?.is_undefined())
        }
        _ => Ok(false),
    }
}

pub(crate) fn array_index_of(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let object = this.unwrap_or(Value::Undefined);
    let len = length_of_array_like(vm, &object)?;
    let Some(start) = array_search_start(vm, args, len, 0.0)? else {
        return Ok(Value::Number(-1.0));
    };
    for i in start..len {
        let key = i.to_string();
        if array_search_has_property(vm, &object, &key)? {
            let value = vm.get_property(&object, &key)?;
            if vm.strict_eq(&value, &target) {
                return Ok(Value::Number(i as f64));
            }
        }
    }
    Ok(Value::Number(-1.0))
}
pub(crate) fn array_includes(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let object = this.unwrap_or(Value::Undefined);
    let len = length_of_array_like(vm, &object)?;
    let Some(start) = array_search_start(vm, args, len, 0.0)? else {
        return Ok(Value::Bool(false));
    };
    // includes uses SameValueZero and intentionally reads holes as undefined.
    for i in start..len {
        let key = i.to_string();
        let value = vm.get_property(&object, &key)?;
        if value.same_value_zero(&target) {
            return Ok(Value::Bool(true));
        }
    }
    Ok(Value::Bool(false))
}
pub(crate) fn array_slice(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let start = match args.first() {
            Some(value) => relative_array_index(to_integer_or_infinity(vm, value)?, len),
            None => 0,
        };
        let end = match args.get(1) {
            Some(value) if !value.is_undefined() => {
                relative_array_index(to_integer_or_infinity(vm, value)?, len)
            }
            _ => len,
        };
        let count = end.saturating_sub(start);
        let result = array_species_create(vm, &object, count)?;
        pin_count += vm.pin(&result);

        let mut source_index = start;
        let mut target_index = 0;
        while source_index < end {
            vm.consume_fuel()?;
            let source_key = source_index.to_string();
            if vm.has_property(&object, &source_key)? {
                let value = vm.get_property(&object, &source_key)?;
                let value_pin = vm.pin(&value);
                let define = vm.define_own_property_or_throw(
                    &result,
                    PropertyKey::from_integer_index(target_index),
                    PropertyDescriptor::data(value),
                );
                vm.unpin(value_pin);
                define?;
            }
            source_index += 1;
            target_index += 1;
        }
        vm.set_property_strict(&result, "length", Value::Number(target_index as f64))?;
        Ok(result.clone())
    })();
    vm.unpin_many(pin_count);
    result
}
pub(crate) fn array_concat(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let result = array_species_create(vm, &object, 0)?;
        pin_count += vm.pin(&result);

        let mut next_index = 0u64;
        for item_index in 0..=args.len() {
            let item = if item_index == 0 {
                &object
            } else {
                &args[item_index - 1]
            };
            if is_concat_spreadable(vm, item)? {
                let length = length_of_array_like_u64(vm, item)?;
                if next_index > MAX_SAFE_ARRAY_LENGTH_U64 - length {
                    return Err(Error::type_err(
                        "Array.prototype.concat result is too large",
                    ));
                }
                // Empty spreadable inputs still consume host work.
                vm.consume_fuel()?;
                let mut source_index = 0u64;
                while source_index < length {
                    vm.consume_fuel()?;
                    let source_key = source_index.to_string();
                    if vm.has_property(item, &source_key)? {
                        let value = vm.get_property(item, &source_key)?;
                        let value_pin = vm.pin(&value);
                        let define = vm.define_own_property_or_throw(
                            &result,
                            PropertyKey::from_integer_index(next_index),
                            PropertyDescriptor::data(value),
                        );
                        vm.unpin(value_pin);
                        define?;
                    }
                    source_index += 1;
                    next_index += 1;
                }
            } else {
                if next_index >= MAX_SAFE_ARRAY_LENGTH_U64 {
                    return Err(Error::type_err(
                        "Array.prototype.concat result is too large",
                    ));
                }
                vm.consume_fuel()?;
                vm.define_own_property_or_throw(
                    &result,
                    PropertyKey::from_integer_index(next_index),
                    PropertyDescriptor::data(item.clone()),
                )?;
                next_index += 1;
            }
        }
        vm.set_property_strict(&result, "length", Value::Number(next_index as f64))?;
        Ok(result.clone())
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn array_reverse(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let middle = len / 2;

        for lower in 0..middle {
            vm.consume_fuel()?;
            let upper = len - lower - 1;
            let lower_key = lower.to_string();
            let upper_key = upper.to_string();
            let mut pair_pins = 0;
            let pair_result: error::Result<()> = (|| {
                let lower_exists = vm.has_property(&object, &lower_key)?;
                let lower_value = if lower_exists {
                    let value = vm.get_property(&object, &lower_key)?;
                    pair_pins += vm.pin(&value);
                    Some(value)
                } else {
                    None
                };

                let upper_exists = vm.has_property(&object, &upper_key)?;
                let upper_value = if upper_exists {
                    let value = vm.get_property(&object, &upper_key)?;
                    pair_pins += vm.pin(&value);
                    Some(value)
                } else {
                    None
                };

                match (lower_value, upper_value) {
                    (Some(lower_value), Some(upper_value)) => {
                        vm.set_property_strict(&object, &lower_key, upper_value)?;
                        vm.set_property_strict(&object, &upper_key, lower_value)?;
                    }
                    (None, Some(upper_value)) => {
                        vm.set_property_strict(&object, &lower_key, upper_value)?;
                        delete_property_or_throw(vm, &object, &upper_key)?;
                    }
                    (Some(lower_value), None) => {
                        delete_property_or_throw(vm, &object, &lower_key)?;
                        vm.set_property_strict(&object, &upper_key, lower_value)?;
                    }
                    (None, None) => {}
                }
                Ok(())
            })();
            vm.unpin_many(pair_pins);
            pair_result?;
        }
        Ok(object)
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn array_sort(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cmp = args.first().cloned();
    validate_sort_compare_fn(vm, &cmp)?;
    let mut pin_count = 0;
    let completion = (|| {
        let (object, len) = array_sort_object_and_length(vm, this, &cmp, &mut pin_count)?;
        ensure_array_sort_materialization_limit(len)?;
        let (mut items, item_pins) = collect_sort_indexed_properties(
            vm,
            &object,
            len,
            SortIndexedPropertiesMode::SkipHoles,
        )?;
        pin_count += item_pins;

        sort_with_cb(vm, &mut items, &cmp)?;
        let item_count = items.len();
        for (index, item) in items.into_iter().enumerate() {
            vm.consume_fuel()?;
            vm.set_property_strict(&object, &index.to_string(), item)?;
        }
        for index in item_count..len {
            vm.consume_fuel()?;
            if !vm.delete_property(&object, &index.to_string())? {
                return Err(Error::type_err(format!(
                    "Cannot delete array index '{}' during sort",
                    index
                )));
            }
        }
        Ok(object)
    })();
    vm.unpin_many(pin_count);
    completion
}

pub(crate) fn array_shift(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        if len == 0 {
            vm.set_property_strict(&object, "length", Value::Number(0.0))?;
            return Ok(Value::Undefined);
        }

        let first = vm.get_property(&object, "0")?;
        pin_count += vm.pin(&first);
        let mut index = 1;
        while index < len {
            vm.consume_fuel()?;
            let from_key = index.to_string();
            let to_key = (index - 1).to_string();
            if vm.has_property(&object, &from_key)? {
                let value = vm.get_property(&object, &from_key)?;
                let value_pin = vm.pin(&value);
                let set = vm.set_property_strict(&object, &to_key, value);
                vm.unpin(value_pin);
                set?;
            } else {
                delete_property_or_throw(vm, &object, &to_key)?;
            }
            index += 1;
        }
        delete_property_or_throw(vm, &object, &(len - 1).to_string())?;
        vm.set_property_strict(&object, "length", Value::Number((len - 1) as f64))?;
        Ok(first)
    })();
    vm.unpin_many(pin_count);
    result
}
pub(crate) fn array_unshift(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let arg_count = u64::try_from(args.len())
            .map_err(|_| Error::type_err("Array.prototype.unshift result is too large"))?;
        let new_len = len
            .checked_add(arg_count)
            .filter(|length| *length <= MAX_SAFE_ARRAY_LENGTH_U64)
            .ok_or_else(|| Error::type_err("Array.prototype.unshift result is too large"))?;

        if arg_count > 0 {
            let mut index = len;
            while index > 0 {
                vm.consume_fuel()?;
                let from_key = (index - 1).to_string();
                let to_key = (index + arg_count - 1).to_string();
                if vm.has_property(&object, &from_key)? {
                    let value = vm.get_property(&object, &from_key)?;
                    let value_pin = vm.pin(&value);
                    let set = vm.set_property_strict(&object, &to_key, value);
                    vm.unpin(value_pin);
                    set?;
                } else {
                    delete_property_or_throw(vm, &object, &to_key)?;
                }
                index -= 1;
            }
            for (index, item) in args.iter().enumerate() {
                vm.consume_fuel()?;
                vm.set_property_strict(&object, &index.to_string(), item.clone())?;
            }
        }
        vm.set_property_strict(&object, "length", Value::Number(new_len as f64))?;
        Ok(Value::Number(new_len as f64))
    })();
    vm.unpin_many(pin_count);
    result
}
pub(crate) fn array_splice(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let actual_start = match args.first() {
            Some(value) => relative_array_index(to_integer_or_infinity(vm, value)?, len),
            None => 0,
        };
        let insert_count = u64::try_from(args.len().saturating_sub(2))
            .map_err(|_| Error::type_err("Array.prototype.splice result is too large"))?;
        let actual_delete_count = match args.len() {
            0 => 0,
            1 => len - actual_start,
            _ => {
                let delete_count = to_integer_or_infinity(vm, &args[1])?;
                if delete_count <= 0.0 {
                    0
                } else {
                    delete_count.min((len - actual_start) as f64) as u64
                }
            }
        };
        let new_len = len as u128 + insert_count as u128 - actual_delete_count as u128;
        if new_len > MAX_SAFE_ARRAY_LENGTH_U64 as u128 {
            return Err(Error::type_err(
                "Array.prototype.splice result is too large",
            ));
        }
        let new_len = new_len as u64;

        let removed = array_species_create(vm, &object, actual_delete_count)?;
        pin_count += vm.pin(&removed);
        let mut removed_index = 0;
        while removed_index < actual_delete_count {
            vm.consume_fuel()?;
            let source_key = (actual_start + removed_index).to_string();
            if vm.has_property(&object, &source_key)? {
                let value = vm.get_property(&object, &source_key)?;
                let value_pin = vm.pin(&value);
                let define = vm.define_own_property_or_throw(
                    &removed,
                    PropertyKey::from_integer_index(removed_index),
                    PropertyDescriptor::data(value),
                );
                vm.unpin(value_pin);
                define?;
            }
            removed_index += 1;
        }
        vm.set_property_strict(
            &removed,
            "length",
            Value::Number(actual_delete_count as f64),
        )?;

        if insert_count < actual_delete_count {
            let mut index = actual_start;
            let shift_end = len - actual_delete_count;
            while index < shift_end {
                vm.consume_fuel()?;
                let from_key = (index + actual_delete_count).to_string();
                let to_key = (index + insert_count).to_string();
                if vm.has_property(&object, &from_key)? {
                    let value = vm.get_property(&object, &from_key)?;
                    let value_pin = vm.pin(&value);
                    let set = vm.set_property_strict(&object, &to_key, value);
                    vm.unpin(value_pin);
                    set?;
                } else {
                    delete_property_or_throw(vm, &object, &to_key)?;
                }
                index += 1;
            }
            let mut index = len;
            while index > new_len {
                vm.consume_fuel()?;
                delete_property_or_throw(vm, &object, &(index - 1).to_string())?;
                index -= 1;
            }
        } else if insert_count > actual_delete_count {
            let mut index = len - actual_delete_count;
            while index > actual_start {
                vm.consume_fuel()?;
                let from_key = (index + actual_delete_count - 1).to_string();
                let to_key = (index + insert_count - 1).to_string();
                if vm.has_property(&object, &from_key)? {
                    let value = vm.get_property(&object, &from_key)?;
                    let value_pin = vm.pin(&value);
                    let set = vm.set_property_strict(&object, &to_key, value);
                    vm.unpin(value_pin);
                    set?;
                } else {
                    delete_property_or_throw(vm, &object, &to_key)?;
                }
                index -= 1;
            }
        }

        for (offset, item) in args.iter().skip(2).enumerate() {
            vm.consume_fuel()?;
            let offset = u64::try_from(offset)
                .map_err(|_| Error::type_err("Array.prototype.splice result is too large"))?;
            vm.set_property_strict(&object, &(actual_start + offset).to_string(), item.clone())?;
        }
        vm.set_property_strict(&object, "length", Value::Number(new_len as f64))?;
        Ok(removed.clone())
    })();
    vm.unpin_many(pin_count);
    result
}
pub(crate) fn array_last_index_of(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let target = args.first().unwrap_or(&Value::Undefined).clone();
    let object = this.unwrap_or(Value::Undefined);
    let len = length_of_array_like(vm, &object)?;
    if len == 0 {
        return Ok(Value::Number(-1.0));
    }
    let raw = match args.get(1) {
        Some(v) => vm.to_number(v)?,
        None => f64::INFINITY,
    };
    if raw.is_infinite() && raw.is_sign_negative() {
        return Ok(Value::Number(-1.0));
    }
    let start = if raw.is_nan() {
        0
    } else if raw.is_infinite() {
        len - 1
    } else {
        let n = raw.trunc();
        if n >= 0.0 {
            (n as usize).min(len - 1)
        } else {
            let k = len as f64 + n;
            if k < 0.0 {
                return Ok(Value::Number(-1.0));
            }
            k as usize
        }
    };
    for i in (0..=start).rev() {
        let key = i.to_string();
        if array_search_has_property(vm, &object, &key)? {
            let value = vm.get_property(&object, &key)?;
            if vm.strict_eq(&value, &target) {
                return Ok(Value::Number(i as f64));
            }
        }
    }
    Ok(Value::Number(-1.0))
}
pub(crate) fn array_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    if receiver.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let object = vm.to_object(&receiver)?;
    let len = length_of_array_like(vm, &object)?;
    let relative_index = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let integer_index = if relative_index.is_nan() {
        0.0
    } else {
        relative_index.trunc()
    };
    let k = if integer_index >= 0.0 {
        integer_index
    } else {
        len as f64 + integer_index
    };
    if k < 0.0 || k >= len as f64 {
        return Ok(Value::Undefined);
    }
    vm.get_property(&object, &(k as usize).to_string())
}
pub(crate) fn array_flat(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let source_length = length_of_array_like_u64(vm, &object)?;
        let mut depth = match args.first() {
            Some(value) if !value.is_undefined() => to_integer_or_infinity(vm, value)?,
            _ => 1.0,
        };
        if depth < 0.0 {
            depth = 0.0;
        }
        let target = array_species_create(vm, &object, 0)?;
        pin_count += vm.pin(&target);
        flatten_into_array(vm, &target, &object, source_length, 0, depth, None)?;
        Ok(target.clone())
    })();
    vm.unpin_many(pin_count);
    result
}

struct FlattenFrame {
    source: Value,
    source_id: usize,
    source_length: u64,
    source_index: u64,
    depth: f64,
    apply_mapper: bool,
    owned_pins: usize,
    repeats_active_source: bool,
}

fn flatten_source_identity(vm: &Vm, value: &Value) -> error::Result<usize> {
    let mut current = value.clone();
    loop {
        let Value::Object(index) = &current else {
            return Err(Error::internal("FlattenIntoArray source must be an object"));
        };
        let target = vm.heap.with_obj(index.0, |object| match object {
            HeapObj::Proxy(proxy) => Some(proxy.target.clone()),
            _ => None,
        });
        match target {
            Some(target) => current = target,
            None => return Ok(index.0),
        }
    }
}

fn flatten_into_array(
    vm: &mut Vm,
    target: &Value,
    source: &Value,
    source_length: u64,
    start: u64,
    depth: f64,
    mapper: Option<(&Value, &Value)>,
) -> error::Result<u64> {
    let source_id = flatten_source_identity(vm, source)?;
    let mut frames = vec![FlattenFrame {
        source: source.clone(),
        source_id,
        source_length,
        source_index: 0,
        depth,
        apply_mapper: mapper.is_some(),
        owned_pins: 0,
        repeats_active_source: false,
    }];
    let mut target_index = start;
    let mut active_pins = 0usize;
    let mut active_cycle_replays = 0usize;
    let mut active_sources = std::collections::HashMap::from([(source_id, 1usize)]);

    let result = (|| loop {
        let Some(frame) = frames.last_mut() else {
            return Ok(target_index);
        };
        if frame.source_index >= frame.source_length {
            let completed = frames.pop().expect("flatten frame must exist");
            if completed.repeats_active_source {
                active_cycle_replays -= 1;
            }
            if let std::collections::hash_map::Entry::Occupied(mut entry) =
                active_sources.entry(completed.source_id)
            {
                if *entry.get() == 1 {
                    entry.remove();
                } else {
                    *entry.get_mut() -= 1;
                }
            }
            vm.unpin_many(completed.owned_pins);
            active_pins -= completed.owned_pins;
            continue;
        }

        vm.consume_fuel()?;
        let source_index = frame.source_index;
        frame.source_index += 1;
        let source = frame.source.clone();
        let frame_depth = frame.depth;
        let apply_mapper = frame.apply_mapper;
        let source_key = source_index.to_string();
        if !vm.has_property(&source, &source_key)? {
            continue;
        }

        let mut element = vm.get_property(&source, &source_key)?;
        let mut element_pins = vm.pin(&element);
        active_pins += element_pins;
        if apply_mapper {
            let (mapper_function, this_arg) =
                mapper.expect("the initial flatten frame owns the mapper");
            element = vm.call_function(
                mapper_function,
                &[element, Value::Number(source_index as f64), source.clone()],
                Some(this_arg.clone()),
            )?;
            let mapped_pin = vm.pin(&element);
            element_pins += mapped_pin;
            active_pins += mapped_pin;
        }

        let should_flatten = frame_depth > 0.0 && is_array_or_throw(vm, &element)?;
        if should_flatten {
            let element_length = length_of_array_like_u64(vm, &element)?;
            let next_depth = if frame_depth == f64::INFINITY {
                f64::INFINITY
            } else {
                frame_depth - 1.0
            };
            let source_id = flatten_source_identity(vm, &element)?;
            let repeats_active_source = next_depth == f64::INFINITY
                && active_sources.get(&source_id).copied().unwrap_or(0) > 0;
            if repeats_active_source {
                if active_cycle_replays >= MAX_FLATTEN_CYCLE_REPLAYS {
                    return Err(Error::range(
                        "Maximum cyclic Array flattening depth exceeded",
                    ));
                }
                active_cycle_replays += 1;
            }
            *active_sources.entry(source_id).or_insert(0) += 1;
            frames.push(FlattenFrame {
                source: element,
                source_id,
                source_length: element_length,
                source_index: 0,
                depth: next_depth,
                apply_mapper: false,
                owned_pins: element_pins,
                repeats_active_source,
            });
            continue;
        }

        if target_index >= MAX_SAFE_ARRAY_LENGTH_U64 {
            return Err(Error::type_err("Array.prototype.flat result is too large"));
        }
        let define = vm.define_own_property_or_throw(
            target,
            PropertyKey::from_integer_index(target_index),
            PropertyDescriptor::data(element),
        );
        vm.unpin_many(element_pins);
        active_pins -= element_pins;
        define?;
        target_index += 1;
    })();
    vm.unpin_many(active_pins);
    result
}

pub(crate) fn array_flat_map(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let source_length = length_of_array_like_u64(vm, &object)?;
        let mapper = get_arg(args, 0);
        if !is_callable(&mapper, &vm.heap) {
            return Err(Error::type_err("Array mapper is not callable"));
        }
        let this_arg = get_arg(args, 1);
        let target = array_species_create(vm, &object, 0)?;
        pin_count += vm.pin(&target);
        flatten_into_array(
            vm,
            &target,
            &object,
            source_length,
            0,
            1.0,
            Some((&mapper, &this_arg)),
        )?;
        Ok(target.clone())
    })();
    vm.unpin_many(pin_count);
    result
}
pub(crate) fn array_copy_within(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;

        let mut to = relative_array_index(to_integer_or_infinity(vm, &get_arg(args, 0))?, len);
        let mut from = relative_array_index(to_integer_or_infinity(vm, &get_arg(args, 1))?, len);
        let final_index = match args.get(2) {
            None | Some(Value::Undefined) => len,
            Some(value) => relative_array_index(to_integer_or_infinity(vm, value)?, len),
        };
        let mut count = final_index.saturating_sub(from).min(len - to);

        let direction = if from < to && to < from + count {
            from += count - 1;
            to += count - 1;
            -1i64
        } else {
            1i64
        };
        let mut from = from as i64;
        let mut to = to as i64;

        while count > 0 {
            vm.consume_fuel()?;
            let from_key = from.to_string();
            let to_key = to.to_string();
            if vm.has_property(&object, &from_key)? {
                let value = vm.get_property(&object, &from_key)?;
                let value_pin = vm.pin(&value);
                let set = vm.set_property_strict(&object, &to_key, value);
                vm.unpin(value_pin);
                set?;
            } else {
                delete_property_or_throw(vm, &object, &to_key)?;
            }
            from += direction;
            to += direction;
            count -= 1;
        }
        Ok(object.clone())
    })();
    vm.unpin_many(pin_count);
    result
}
pub(crate) fn array_keys(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    create_array_iterator(vm, this, CollectionIteratorKind::ArrayKeys)
}
pub(crate) fn array_values(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    create_array_iterator(vm, this, CollectionIteratorKind::ArrayValues)
}
pub(crate) fn array_entries(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    create_array_iterator(vm, this, CollectionIteratorKind::ArrayEntries)
}

fn create_array_iterator(
    vm: &mut Vm,
    this: Option<Value>,
    kind: CollectionIteratorKind,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        new_collection_iterator(vm, object, kind)
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn array_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    // `Array(n)` / `new Array(n)` with a single number argument creates a
    // sparse array of length n (filled with holes). Other argument forms
    // create an array of the given elements. `this` (from `new`) is ignored:
    // ES ArrayConstructor always returns a fresh Array exotic object, not the
    // `[[Construct]]`-provided ordinary object.
    let (items, holes_len) = if args.len() == 1 {
        if let Some(Value::Number(n)) = args.first() {
            // Validate the length per ArrayCreate: must be a non-negative
            // integer that fits in u32. Negative / fractional / huge values
            // throw RangeError, not an OOM abort.
            if n.is_nan() || *n < 0.0 || n.is_infinite() || n.fract() != 0.0 {
                return Err(Error::range("Invalid array length"));
            }
            if *n >= (1u64 << 32) as f64 {
                return Err(Error::range("Invalid array length"));
            }
            let len = *n as usize;
            (Vec::new(), Some(len))
        } else {
            (args.to_vec(), None)
        }
    } else {
        (args.to_vec(), None)
    };
    let realm = vm.current_realm_global_env();
    let default_proto = vm
        .realm_array_prototypes
        .get(&realm.0)
        .cloned()
        .unwrap_or_else(|| vm.array_proto.clone());
    let proto = native_constructor_prototype_with_default(vm, "Array", default_proto)?;
    let sparse_length = holes_len.filter(|length| *length > crate::value::MAX_DENSE_ARRAY_LEN);
    let arr = if let Some(len) = holes_len {
        if sparse_length.is_some() {
            HeapObj::Array(ArrayData::new(Vec::new(), Some(proto)))
        } else {
            HeapObj::Array(ArrayData::new_holes(len, Some(proto)))
        }
    } else {
        HeapObj::Array(ArrayData::new(items, Some(proto)))
    };
    let pin_count = match &arr {
        HeapObj::Array(array) => {
            let mut pin_count = vm.pin_many(&array.items.lock());
            if let Some(prototype) = array.proto.lock().as_ref() {
                pin_count += vm.pin(prototype);
            }
            pin_count
        }
        _ => unreachable!("Array constructor must allocate ArrayData"),
    };
    let result = (|| {
        let array = vm.alloc(arr)?;
        if let Some(length) = sparse_length {
            vm.set_array_length(array.0, Value::Number(length as f64))?;
        }
        Ok(Value::Object(array))
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn array_find(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let (object, len, callback, this_arg) = array_find_object_and_callback(vm, args, this)?;
    for i in 0..len {
        let value = array_find_value_at(vm, &object, i)?;
        let found = vm.call_function(
            &callback,
            &[value.clone(), Value::Number(i as f64), object.clone()],
            Some(this_arg.clone()),
        )?;
        if found.is_truthy() {
            return Ok(value);
        }
    }
    Ok(Value::Undefined)
}
pub(crate) fn array_find_index(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (object, len, callback, this_arg) = array_find_object_and_callback(vm, args, this)?;
    for i in 0..len {
        let value = array_find_value_at(vm, &object, i)?;
        let found = vm.call_function(
            &callback,
            &[value, Value::Number(i as f64), object.clone()],
            Some(this_arg.clone()),
        )?;
        if found.is_truthy() {
            return Ok(Value::Number(i as f64));
        }
    }
    Ok(Value::Number(-1.0))
}
pub(crate) fn array_find_last(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (object, len, callback, this_arg) = array_find_object_and_callback(vm, args, this)?;
    let mut i = len;
    while i > 0 {
        i -= 1;
        let value = array_find_value_at(vm, &object, i)?;
        let found = vm.call_function(
            &callback,
            &[value.clone(), Value::Number(i as f64), object.clone()],
            Some(this_arg.clone()),
        )?;
        if found.is_truthy() {
            return Ok(value);
        }
    }
    Ok(Value::Undefined)
}

pub(crate) fn array_find_last_index(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (object, len, callback, this_arg) = array_find_object_and_callback(vm, args, this)?;
    let mut i = len;
    while i > 0 {
        i -= 1;
        let value = array_find_value_at(vm, &object, i)?;
        let found = vm.call_function(
            &callback,
            &[value, Value::Number(i as f64), object.clone()],
            Some(this_arg.clone()),
        )?;
        if found.is_truthy() {
            return Ok(Value::Number(i as f64));
        }
    }
    Ok(Value::Number(-1.0))
}
pub(crate) fn array_fill(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    pin_count += vm.pin_many(args);
    let result = (|| {
        let object = array_method_to_object(vm, &receiver)?;
        pin_count += vm.pin(&object);
        let len = length_of_array_like_u64(vm, &object)?;
        let mut index = relative_array_index(to_integer_or_infinity(vm, &get_arg(args, 1))?, len);
        let final_index = match args.get(2) {
            None | Some(Value::Undefined) => len,
            Some(value) => relative_array_index(to_integer_or_infinity(vm, value)?, len),
        };
        let value = get_arg(args, 0);

        while index < final_index {
            vm.consume_fuel()?;
            vm.set_property_strict(&object, &index.to_string(), value.clone())?;
            index += 1;
        }
        Ok(object)
    })();
    vm.unpin_many(pin_count);
    result
}
pub(crate) fn array_some(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let (object, len, callback, this_arg) = array_find_object_and_callback(vm, args, this)?;
    for i in 0..len {
        let key = i.to_string();
        if !array_search_has_property(vm, &object, &key)? {
            continue;
        }
        let value = vm.get_property(&object, &key)?;
        let found = vm.call_function(
            &callback,
            &[value, Value::Number(i as f64), object.clone()],
            Some(this_arg.clone()),
        )?;
        if found.is_truthy() {
            return Ok(Value::Bool(true));
        }
    }
    Ok(Value::Bool(false))
}
pub(crate) fn array_every(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (object, len, callback, this_arg) = array_find_object_and_callback(vm, args, this)?;
    for i in 0..len {
        let key = i.to_string();
        if !array_search_has_property(vm, &object, &key)? {
            continue;
        }
        let value = vm.get_property(&object, &key)?;
        let ok = vm.call_function(
            &callback,
            &[value, Value::Number(i as f64), object.clone()],
            Some(this_arg.clone()),
        )?;
        if !ok.is_truthy() {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}
