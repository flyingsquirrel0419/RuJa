use super::*;

// Function.prototype: call / apply / bind
// =========================================================================

/// `Function.prototype.call(thisArg, ...args)`: invoke `this` (a function)
/// with an explicit `this` binding and a list of arguments.
/// `Function.prototype.toString`: return a spec-ish string representation.
/// For native functions: `function name() { [native code] }`. For interpreted
/// functions, the source is not retained, so we emit `function name() { ... }`.
pub(crate) fn function_to_string(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let f = match this {
        Some(v) => v,
        None => return Ok(Value::String(Arc::from("function () { [native code] }"))),
    };
    if let Value::Object(idx) = &f {
        let (name, is_native) = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Function(fun) = o {
                let n = fun.name.as_ref().map(|s| s.to_string()).unwrap_or_default();
                let native = matches!(fun.kind, crate::value::FunctionKind::Native { .. });
                (n, native)
            } else {
                (String::new(), true)
            }
        });
        let body = if is_native { "[native code]" } else { "..." };
        return Ok(Value::String(Arc::from(
            format!("function {}() {{ {} }}", name, body).as_str(),
        )));
    }
    Ok(Value::String(Arc::from("function () { [native code] }")))
}

pub(crate) fn function_call(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = match this {
        Some(t) => t,
        None => return Err(error::Error::type_err("undefined is not a function")),
    };
    if !is_callable(&target, &vm.heap) {
        return Err(error::Error::type_err(format!(
            "{} is not a function",
            target.type_of()
        )));
    }
    let this_arg = args.first().cloned().unwrap_or(Value::Undefined);
    let call_args: &[Value] = if args.len() > 1 { &args[1..] } else { &[][..] };
    vm.call_function(&target, call_args, Some(this_arg))
}

/// `Function.prototype.apply(thisArg, [argsArray])`: invoke `this` (a
/// function) with an explicit `this` binding and an array-like of arguments.
pub(crate) fn function_apply(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = match this {
        Some(t) => t,
        None => return Err(error::Error::type_err("undefined is not a function")),
    };
    if !is_callable(&target, &vm.heap) {
        return Err(error::Error::type_err(format!(
            "{} is not a function",
            target.type_of()
        )));
    }
    let this_arg = args.first().cloned().unwrap_or(Value::Undefined);
    let arr_args: Vec<Value> = match args.get(1) {
        Some(Value::Undefined) | Some(Value::Null) => Vec::new(),
        Some(Value::Object(idx)) => vm.heap.with_obj(idx.0, |obj| match obj {
            HeapObj::Array(a) => a.items.lock().clone(),
            _ => {
                // Array-like fallback: read .length and integer-indexed props.
                let len = obj
                    .props()
                    .lock()
                    .get(&PropertyKey::from("length"))
                    .and_then(|d| {
                        if let Value::Number(n) = d.value {
                            Some(n as usize)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                (0..len)
                    .map(|i| {
                        obj.props()
                            .lock()
                            .get(&PropertyKey::from(i.to_string().as_str()))
                            .map(|d| d.value.clone())
                            .unwrap_or(Value::Undefined)
                    })
                    .collect()
            }
        }),
        _ => Vec::new(),
    };
    vm.call_function(&target, &arr_args, Some(this_arg))
}

/// `Function.prototype.bind(thisArg, ...args)`: create a new function with a
/// fixed `this` binding and leading arguments.
pub(crate) fn function_bind(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = match this {
        Some(t) => t,
        None => return Err(error::Error::type_err("undefined is not a function")),
    };
    if !is_callable(&target, &vm.heap) {
        return Err(error::Error::type_err(format!(
            "{} is not a function",
            target.type_of()
        )));
    }
    let this_arg = args.first().cloned().unwrap_or(Value::Undefined);
    let bound_args: Vec<Value> = if args.len() > 1 {
        args[1..].to_vec()
    } else {
        Vec::new()
    };
    let target_idx = match &target {
        Value::Object(i) => *i,
        _ => return Err(error::Error::type_err("not a function")),
    };
    let bound = crate::value::FunctionData {
        name: Some(Arc::from("bound")),
        kind: crate::value::FunctionKind::Bound {
            target: target_idx,
            this_val: this_arg,
            bound_args,
        },
        closure: vm.global,
        prototype: Mutex::new(None),
        proto: Mutex::new(match vm.function_proto {
            Value::Object(_) => Some(vm.function_proto.clone()),
            _ => None,
        }),
        props: Mutex::new(IndexMap::new()),
    };
    let fidx = vm.heap.allocate(HeapObj::Function(bound));
    Ok(Value::Object(GcIdx(fidx)))
}

/// `Function.prototype` itself is a callable no-op function (per spec:
/// "an empty function"). Invoking it returns `undefined`.
pub(crate) fn function_proto_noop(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::Undefined)
}
