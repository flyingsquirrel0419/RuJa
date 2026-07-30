use super::call_arguments::{create_list_from_array_like, MAX_MATERIALIZED_CALL_ARGUMENTS};
use super::*;

// Function.prototype: call / apply / bind
// =========================================================================

/// `Function.prototype.call(thisArg, ...args)`: invoke `this` (a function)
/// with an explicit `this` binding and a list of arguments.
/// `Function.prototype.toString`: return a spec-ish string representation.
/// For native functions: `function name() { [native code] }`. For interpreted
/// functions, the source is not retained, so we emit `function name() { ... }`.
pub(crate) fn function_to_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let f = match this {
        Some(v) => v,
        None => return Ok(Value::String(Arc::from("function () { [native code] }"))),
    };
    if let Value::Object(idx) = &f {
        let (name, is_native) = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Function(fun) = o {
                let n = fun.name.as_ref().map(|s| s.to_string()).unwrap_or_default();
                let native = matches!(
                    fun.kind,
                    crate::value::FunctionKind::Native { .. }
                        | crate::value::FunctionKind::Bound { .. }
                );
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

pub(crate) fn function_call(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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
pub(crate) fn function_apply(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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
    let arguments_list = args.get(1).cloned().unwrap_or(Value::Undefined);
    let (call_args, call_args_pin_count) = match arguments_list {
        Value::Undefined | Value::Null => (Vec::new(), 0),
        _ => create_list_from_array_like(vm, &arguments_list, MAX_MATERIALIZED_CALL_ARGUMENTS)?,
    };
    let result = vm.call_function(&target, &call_args, Some(this_arg));
    vm.unpin_many(call_args_pin_count);
    result
}

/// `Function.prototype.bind(thisArg, ...args)`: create a new function with a
/// fixed `this` binding and leading arguments.
pub(crate) fn function_bind(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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
    let bound_arg_count = bound_args.len();
    let target_idx = match &target {
        Value::Object(i) => *i,
        _ => return Err(error::Error::type_err("not a function")),
    };
    // BoundFunctionCreate copies the target's actual [[Prototype]], including
    // an observable Proxy getPrototypeOf trap. The surrounding call roots the
    // target and captured arguments, but this returned prototype is a new Rust
    // local and must survive the bound function's GC-aware allocation.
    let function_proto = vm.get_prototype_of(&target)?;
    let constructable = vm.is_constructor_value(&target);
    let proto_pin = function_proto
        .as_ref()
        .map(|prototype| vm.pin(prototype))
        .unwrap_or(0);
    let bound = crate::value::FunctionData {
        name: Some(Arc::from("bound")),
        kind: crate::value::FunctionKind::Bound {
            target: target_idx,
            this_val: this_arg,
            bound_args,
            constructable,
        },
        closure: vm.global,
        lexical_new_target: Value::Undefined,
        home_object: Mutex::new(None),
        is_class_ctor: std::sync::atomic::AtomicBool::new(false),
        prototype: Mutex::new(None),
        proto: Mutex::new(function_proto),
        // Reserve metadata slots before the object becomes the root that keeps
        // target, thisArg, bound arguments, and [[Prototype]] alive across the
        // observable target length/name operations below.
        props: Mutex::new(IndexMap::with_capacity(2)),
        extensible: std::sync::atomic::AtomicBool::new(true),
        private_fields: Mutex::new(std::collections::HashMap::new()),
    };
    let result = vm.alloc(HeapObj::Function(bound));
    vm.unpin_many(proto_pin);
    let bound_idx = result?;
    let bound_value = Value::Object(bound_idx);
    let bound_pin = vm.pin(&bound_value);
    let result = (|| {
        let length_key = PropertyKey::from("length");
        let target_has_length =
            own_property_descriptor_for_key_or_throw(vm, &target, &length_key)?.is_some();
        let length = if target_has_length {
            match vm.get_property(&target, "length")? {
                Value::Number(target_length) => {
                    bound_function_length(target_length, bound_arg_count)
                }
                _ => 0.0,
            }
        } else {
            0.0
        };

        let target_name = match vm.get_property(&target, "name")? {
            Value::String(name) => name,
            _ => Arc::from(""),
        };
        let bound_name = Arc::from(format!("bound {target_name}").as_str());

        let mut length_desc = PropertyDescriptor::data(Value::Number(length));
        length_desc.writable = false;
        length_desc.enumerable = false;
        length_desc.configurable = true;
        let mut name_desc = PropertyDescriptor::data(Value::String(bound_name));
        name_desc.writable = false;
        name_desc.enumerable = false;
        name_desc.configurable = true;
        vm.heap.with_obj(bound_idx.0, |object| {
            let HeapObj::Function(function) = object else {
                unreachable!("newly allocated bound function changed heap kind")
            };
            let mut props = function.props.lock();
            props.insert(length_key, length_desc);
            props.insert(PropertyKey::from("name"), name_desc);
        });
        Ok(bound_value)
    })();
    vm.unpin_many(bound_pin);
    result
}

fn bound_function_length(target_length: f64, bound_arg_count: usize) -> f64 {
    if target_length == f64::INFINITY {
        return f64::INFINITY;
    }
    if !target_length.is_finite() {
        return 0.0;
    }
    let integer = target_length.trunc();
    let bound_arg_count = bound_arg_count as f64;
    if integer > bound_arg_count {
        integer - bound_arg_count
    } else {
        0.0
    }
}

/// `Function.prototype[Symbol.hasInstance](value)`: expose
/// OrdinaryHasInstance as the default instanceof hook.
pub(crate) fn function_symbol_has_instance(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let target = this.unwrap_or(Value::Undefined);
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    vm.ordinary_has_instance(&target, &value).map(Value::Bool)
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

/// %ThrowTypeError% used by the restricted `caller` and `arguments`
/// accessors on Function.prototype.
pub(crate) fn function_throw_type_error(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    Err(error::Error::type_err(
        "'caller' and 'arguments' are restricted function properties",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_argument_list_pins_balance_on_success_and_errors() {
        let mut vm = Vm::new().expect("failed to initialize VM");
        vm.run(
            r#"
            var itemError = {};
            var abruptItems = {
              length: 2,
              get 0() { return {}; },
              get 1() { throw itemError; }
            };
            var completeItems = {
              length: 2,
              get 0() { return { label: "first" }; },
              get 1() { return { label: "second" }; }
            };
            var targetError = {};
            function returningTarget(first) { return first; }
            function throwingTarget() { throw targetError; }
            "#,
        )
        .expect("failed to create Function.apply fixtures");

        let returning_target = vm
            .run("returningTarget")
            .expect("failed to read returning target");
        let throwing_target = vm
            .run("throwingTarget")
            .expect("failed to read throwing target");
        let abrupt_items = vm
            .run("abruptItems")
            .expect("failed to read abrupt argument list");
        let complete_items = vm
            .run("completeItems")
            .expect("failed to read complete argument list");
        let baseline = vm.gc_pins.len();

        assert!(function_apply(
            &mut vm,
            &[Value::Undefined, Value::Number(1.0)],
            Some(returning_target.clone()),
        )
        .is_err());
        assert_eq!(vm.gc_pins.len(), baseline);

        assert!(function_apply(
            &mut vm,
            &[Value::Undefined, abrupt_items],
            Some(returning_target.clone()),
        )
        .is_err());
        assert_eq!(vm.gc_pins.len(), baseline);

        let returned = function_apply(
            &mut vm,
            &[Value::Undefined, complete_items.clone()],
            Some(returning_target.clone()),
        )
        .expect("successful apply should return the target result");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.get_property(&returned, "label")
                .expect("returned argument should remain valid"),
            Value::String(Arc::from("first"))
        );

        assert!(function_apply(
            &mut vm,
            &[Value::Undefined, complete_items],
            Some(throwing_target),
        )
        .is_err());
        assert_eq!(vm.gc_pins.len(), baseline);

        function_apply(&mut vm, &[Value::Undefined], Some(returning_target.clone()))
            .expect("an omitted argument list should call with no arguments");
        assert_eq!(vm.gc_pins.len(), baseline);

        for nullish in [Value::Undefined, Value::Null] {
            function_apply(
                &mut vm,
                &[Value::Undefined, nullish],
                Some(returning_target.clone()),
            )
            .expect("nullish argument lists should call with no arguments");
            assert_eq!(vm.gc_pins.len(), baseline);
        }
    }
}
