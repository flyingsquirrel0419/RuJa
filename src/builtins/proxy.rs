use super::*;

pub(crate) fn proxy_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err("Proxy constructor requires 'new'"));
    }
    proxy_create(vm, args)
}

fn proxy_create(vm: &mut Vm, args: &[Value]) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let handler = args.get(1).cloned().unwrap_or(Value::Undefined);

    // Target and handler must be objects.
    if !matches!(&target, Value::Object(_)) {
        return Err(Error::type_err(
            "Cannot create proxy with a non-object as target".to_string(),
        ));
    }
    if !matches!(&handler, Value::Object(_)) {
        return Err(Error::type_err(
            "Cannot create proxy with a non-object as handler".to_string(),
        ));
    }

    let callable = is_callable(&target, &vm.heap);
    let constructable = vm.is_constructor_value(&target);
    let proto = vm.heap.with_obj(
        match &target {
            Value::Object(idx) => idx.0,
            _ => unreachable!(),
        },
        |o| o.proto().lock().clone(),
    );

    let pin_count = vm.pin_many(&[target.clone(), handler.clone()]);
    let result = vm
        .alloc(HeapObj::Proxy(crate::value::ProxyData {
            target,
            handler,
            callable,
            constructable,
            revoked: Mutex::new(false),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(proto),
        }))
        .map(Value::Object);
    vm.unpin_many(pin_count);
    result
}

/// `Proxy.revocable(target, handler)` returns `{ proxy, revoke }`.
pub(crate) fn proxy_revocable(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let proxy_val = proxy_create(vm, args)?;
    let proxy_idx = match &proxy_val {
        Value::Object(idx) => idx.0,
        _ => unreachable!(),
    };
    let mut pin_count = vm.pin(&proxy_val);
    let realm = vm.native_callee_closure().unwrap_or(vm.global);
    let result = (|| {
        // Create a revoke function that sets revoked = true.
        let revoke_fn_idx = vm.new_native_function_in_env("", proxy_revoke, 0, realm)?;
        let revoke = Value::Object(revoke_fn_idx);
        pin_count += vm.pin(&revoke);

        // Keep the associated proxy off the revoke function's observable own keys.
        vm.heap.with_obj(revoke_fn_idx.0, |o| {
            if let HeapObj::Function(f) = o {
                let key = crate::value::PrivateSlotKey::Internal(Arc::from("__proxy_idx__"));
                f.private_fields.lock().insert(
                    key,
                    crate::value::PrivateSlot::Value(Value::Object(GcIdx(proxy_idx))),
                );
            }
        });

        // Build { proxy, revoke } only after both intermediate values are rooted.
        let obj_idx = vm.new_object_in_current_realm()?;
        let obj = Value::Object(obj_idx);
        vm.heap.with_obj(obj_idx.0, |o| {
            if let HeapObj::Object(od) = o {
                od.props
                    .lock()
                    .insert(PropertyKey::from("proxy"), data_prop(proxy_val));
                od.props
                    .lock()
                    .insert(PropertyKey::from("revoke"), data_prop(revoke));
            }
        });
        Ok(obj)
    })();
    vm.unpin_many(pin_count);
    result
}

fn proxy_revoke(vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = vm.current_native_callee().cloned() {
        let proxy_idx = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Function(f) = o {
                let key = crate::value::PrivateSlotKey::Internal(Arc::from("__proxy_idx__"));
                let mut fields = f.private_fields.lock();
                let proxy_idx = match fields.get(&key) {
                    Some(crate::value::PrivateSlot::Value(Value::Object(proxy_idx))) => {
                        Some(proxy_idx.0)
                    }
                    _ => None,
                };
                fields.remove(&key);
                proxy_idx
            } else {
                None
            }
        });
        if let Some(pid) = proxy_idx {
            vm.heap.with_obj(pid, |o| {
                if let HeapObj::Proxy(p) = o {
                    *p.revoked.lock() = true;
                }
            });
        }
    }
    Ok(Value::Undefined)
}
