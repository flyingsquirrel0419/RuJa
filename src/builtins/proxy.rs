use super::*;

pub(crate) fn proxy_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
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

    let proto = vm.heap.with_obj(
        match &target {
            Value::Object(idx) => idx.0,
            _ => unreachable!(),
        },
        |o| o.proto().lock().clone(),
    );

    let idx = vm
        .heap
        .allocate_unchecked(HeapObj::Proxy(crate::value::ProxyData {
            target,
            handler,
            revoked: Mutex::new(false),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(proto),
        }));
    Ok(Value::Object(GcIdx(idx)))
}

/// `Proxy.revocable(target, handler)` returns `{ proxy, revoke }`.
pub(crate) fn proxy_revocable(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let proxy_val = proxy_constructor(vm, args, None)?;
    let proxy_idx = match &proxy_val {
        Value::Object(idx) => idx.0,
        _ => unreachable!(),
    };

    // Create a revoke function that sets revoked = true.
    let revoke_fn_idx = vm.heap.allocate_unchecked(HeapObj::Function(FunctionData {
        name: Some(Arc::from("")),
        kind: FunctionKind::Native {
            func: proxy_revoke,
            length: 0,
        },
        closure: vm.global,
        prototype: Mutex::new(None),
        proto: Mutex::new(match vm.function_proto {
            Value::Object(_) => Some(vm.function_proto.clone()),
            _ => None,
        }),
        props: Mutex::new(IndexMap::new()),
    }));

    // Store proxy idx in revoke fn props so the native fn can find it.
    vm.heap.with_obj(revoke_fn_idx, |o| {
        if let HeapObj::Function(f) = o {
            f.props.lock().insert(
                PropertyKey::from("__proxy_idx__"),
                data_prop(Value::Number(proxy_idx as f64)),
            );
        }
    });

    // Build { proxy, revoke } object.
    let obj_idx = vm.new_object();
    let obj = Value::Object(obj_idx);
    if let Value::Object(oidx) = &obj {
        vm.heap.with_obj(oidx.0, |o| {
            if let HeapObj::Object(od) = o {
                od.props
                    .lock()
                    .insert(PropertyKey::from("proxy"), data_prop(proxy_val));
                od.props.lock().insert(
                    PropertyKey::from("revoke"),
                    data_prop(Value::Object(GcIdx(revoke_fn_idx))),
                );
            }
        });
    }
    Ok(obj)
}

fn proxy_revoke(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let proxy_idx = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Function(f) = o {
                f.props
                    .lock()
                    .get(&PropertyKey::from("__proxy_idx__"))
                    .and_then(|d| match &d.value {
                        Value::Number(n) => Some(*n as usize),
                        _ => None,
                    })
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
