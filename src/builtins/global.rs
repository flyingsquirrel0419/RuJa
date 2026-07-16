use super::*;

const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

// =========================================================================
// Global functions
// =========================================================================
pub(crate) fn global_parse_int(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let input = match args.first() {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(v) => vm.to_string(v)?.to_string(),
        None => return Ok(Value::Number(f64::NAN)),
    };
    let mut radix = args
        .get(1)
        .map(|v| vm.to_number(v).map(|n| crate::vm::to_int32(n) as u32))
        .transpose()?
        .unwrap_or(0);
    let strip_hex = radix == 0 || radix == 16;
    let mut chars = input.char_indices().peekable();
    let neg = match chars.peek() {
        Some((_, '+')) => {
            chars.next();
            false
        }
        Some((_, '-')) => {
            chars.next();
            true
        }
        _ => false,
    };
    if strip_hex {
        let is_hex = matches!(chars.peek(), Some((_, '0')))
            && matches!(chars.clone().nth(1), Some((_, 'x')) | Some((_, 'X')));
        if is_hex {
            chars.next();
            chars.next();
            radix = 16;
        }
    }
    if radix == 0 {
        radix = 10;
    }
    if !(2..=36).contains(&radix) {
        return Ok(Value::Number(f64::NAN));
    }
    let digit_value = |c: char| c.to_digit(radix);
    let start = chars.peek().map(|(i, _)| *i).unwrap_or(input.len());
    let digits_end = input[start..]
        .char_indices()
        .find(|(_, c)| digit_value(*c).is_none())
        .map(|(i, _)| start + i)
        .unwrap_or(input.len());
    let digits = &input[start..digits_end];
    if digits.is_empty() {
        return Ok(Value::Number(f64::NAN));
    }
    let mut number = 0.0;
    for c in digits.chars() {
        let digit = digit_value(c).unwrap_or(0) as f64;
        number = number * radix as f64 + digit;
    }
    if neg {
        number = -number;
    }
    Ok(Value::Number(number))
}
pub(crate) fn global_parse_float(
    _vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    // Parse the longest prefix matching the StrDecimalLiteral grammar:
    // optional sign, digits, optional `.` digits, optional exponent. Anything
    // after that prefix is ignored (NaN only if no valid prefix exists).
    let s = match args.first() {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(v) => _vm.to_string(v)?.to_string(),
        None => return Ok(Value::Number(f64::NAN)),
    };
    let bytes = s.as_bytes();
    let mut i = 0usize;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let digits_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let mut have_int = i > digits_start;
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        // `3.` is a valid prefix; a lone `.` with no digits anywhere is not.
        have_int = have_int || i > frac_start;
    }
    if !have_int {
        // Empty input or sign-only: not a valid number.
        if bytes.is_empty() {
            return Ok(Value::Number(f64::NAN));
        }
        // Check for `Infinity`/`+Infinity`/`-Infinity` prefix.
        let rest = &s[if bytes[0] == b'+' || bytes[0] == b'-' {
            1
        } else {
            0
        }..];
        if rest.starts_with("Infinity") {
            let val = if bytes.first() == Some(&b'-') {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
            return Ok(Value::Number(val));
        }
        return Ok(Value::Number(f64::NAN));
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        let mut j = i + 1;
        if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
            j += 1;
        }
        let exp_start = j;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j > exp_start {
            i = j;
        }
    }
    if i == 0 {
        return Ok(Value::Number(f64::NAN));
    }
    Ok(Value::Number(s[..i].parse().unwrap_or(f64::NAN)))
}
pub(crate) fn global_is_nan(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let n = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    Ok(Value::Bool(n.is_nan()))
}
pub(crate) fn global_is_finite(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let n = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    Ok(Value::Bool(n.is_finite()))
}

/// `BigInt(x)`: convert a primitive or primitive-producing object to a BigInt.
/// Throws RangeError for non-integral numbers, SyntaxError for unparseable
/// strings, and TypeError for unsupported primitive types.
pub(crate) fn global_bigint(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let arg = args.first().unwrap_or(&Value::Undefined);
    let prim = match arg {
        Value::Object(_) => vm.to_primitive_number(arg)?,
        _ => arg.clone(),
    };
    match prim {
        Value::BigInt(n) => Ok(Value::BigInt(n.clone())),
        Value::Bool(b) => Ok(Value::BigInt(num_bigint::BigInt::from(if b {
            1
        } else {
            0
        }))),
        Value::Number(n) => {
            if let Some(bigint) = Vm::number_to_bigint_exact(n) {
                Ok(Value::BigInt(bigint))
            } else {
                Err(Error::range(format!(
                    "The number {} cannot be converted to a BigInt because it is not an integer",
                    crate::value::num_to_string(n)
                )))
            }
        }
        Value::String(s) => Vm::string_to_bigint(&s)
            .map(Value::BigInt)
            .ok_or_else(|| Error::syntax(format!("Cannot convert {} to a BigInt", s))),
        Value::Undefined | Value::Null | Value::Symbol(_) | Value::PrivateName(_) => {
            Err(Error::type_err("Cannot convert to a BigInt".to_string()))
        }
        Value::Object(_) | Value::Reference(_) => {
            Err(Error::type_err("Cannot convert to a BigInt".to_string()))
        }
    }
}

fn bigint_to_index(vm: &mut Vm, value: &Value, name: &str) -> error::Result<usize> {
    let n = vm.to_number(value)?;
    if n.is_nan() {
        return Ok(0);
    }
    if !n.is_finite() {
        return Err(Error::range(format!("Invalid {name} bits")));
    }
    let integer = n.trunc();
    if integer < 0.0 || integer > MAX_SAFE_INTEGER {
        return Err(Error::range(format!("Invalid {name} bits")));
    }
    Ok(integer as usize)
}

fn bigint_uint_n(bits: usize, value: BigInt) -> BigInt {
    if bits == 0 {
        return BigInt::zero();
    }
    let modulus = BigInt::from(1u8) << bits;
    ((value % &modulus) + &modulus) % &modulus
}

pub(crate) fn bigint_as_int_n(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let bits = bigint_to_index(
        vm,
        args.first().unwrap_or(&Value::Undefined),
        "BigInt.asIntN",
    )?;
    let bigint = vm.to_bigint(args.get(1).unwrap_or(&Value::Undefined))?;
    if bits == 0 {
        return Ok(Value::BigInt(BigInt::zero()));
    }
    let modulus = BigInt::from(1u8) << bits;
    let threshold = BigInt::from(1u8) << (bits - 1);
    let wrapped = ((bigint % &modulus) + &modulus) % &modulus;
    if wrapped >= threshold {
        Ok(Value::BigInt(wrapped - modulus))
    } else {
        Ok(Value::BigInt(wrapped))
    }
}

pub(crate) fn bigint_as_uint_n(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let bits = bigint_to_index(
        vm,
        args.first().unwrap_or(&Value::Undefined),
        "BigInt.asUintN",
    )?;
    let bigint = vm.to_bigint(args.get(1).unwrap_or(&Value::Undefined))?;
    Ok(Value::BigInt(bigint_uint_n(bits, bigint)))
}

fn this_bigint_value(vm: &mut Vm, value: Option<Value>) -> error::Result<BigInt> {
    match value {
        Some(Value::BigInt(n)) => Ok(n),
        Some(Value::Object(idx)) => {
            let primitive = vm.heap.with_obj(idx.0, |o| {
                if let HeapObj::Object(od) = o {
                    od.primitive.lock().clone()
                } else {
                    None
                }
            });
            if let Some(Value::BigInt(n)) = primitive {
                Ok(n)
            } else {
                Err(Error::type_err(
                    "BigInt method called on incompatible receiver",
                ))
            }
        }
        _ => Err(Error::type_err(
            "BigInt method called on incompatible receiver",
        )),
    }
}

/// `BigInt.prototype.toString([radix])`: stringify a BigInt in radix 2..36.
pub(crate) fn bigint_to_string(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let n = this_bigint_value(vm, this)?;
    let radix = match args.first() {
        None | Some(Value::Undefined) => 10,
        Some(value) => {
            let number = vm.to_number(value)?;
            let integer = if number.is_nan() { 0.0 } else { number.trunc() };
            if !(2.0..=36.0).contains(&integer) {
                return Err(Error::range("toString() radix must be between 2 and 36"));
            }
            integer as u32
        }
    };
    Ok(Value::String(Arc::from(n.to_str_radix(radix).as_str())))
}

/// `BigInt.prototype.valueOf()`: return the primitive BigInt value.
pub(crate) fn bigint_value_of(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::BigInt(this_bigint_value(vm, this)?))
}

/// `eval(x)`: if `x` is not a string, return it as-is. Otherwise parse and
/// run it. Unqualified `eval(...)` is upgraded to direct eval by `CallEval`
/// only when runtime name resolution produced the Realm's intrinsic eval
/// function; other calls run as indirect eval in the callee's Realm.
pub(crate) fn global_eval(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let arg = args.first().cloned().unwrap_or(Value::Undefined);
    let src = match &arg {
        Value::String(s) => s.to_string(),
        // Non-string: return unchanged.
        _ => return Ok(arg),
    };
    // Default (indirect) behavior: run in the eval function's own Realm. RuJa
    // represents a native function's Realm with its closure environment.
    let global_env = vm.native_callee_closure().unwrap_or(vm.global);
    let global_this = crate::environment::get(&vm.heap, global_env, "globalThis")
        .unwrap_or_else(|| vm.global_this.clone());
    vm.eval_indirect_in(global_env, global_this, &src)
}

/// `new Function(p0, p1, ..., body)`: dynamically build a function from a
/// parameter list and a body source string. The last argument is the body;
/// earlier arguments are parameter names (comma-separated within each).
pub(crate) fn function_constructor(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    dynamic_function_constructor(vm, args, false, false)
}

pub(crate) fn generator_function_constructor(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    dynamic_function_constructor(vm, args, true, false)
}

pub(crate) fn async_function_constructor(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    dynamic_function_constructor(vm, args, false, true)
}

pub(crate) fn async_generator_function_constructor(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    dynamic_function_constructor(vm, args, true, true)
}

fn dynamic_function_constructor(
    vm: &mut Vm,
    args: &[Value],
    is_generator: bool,
    is_async: bool,
) -> error::Result<Value> {
    use crate::ast::FunctionExpr;
    use crate::value::{FunctionData, FunctionKind, PropertyDescriptor, PropertyKey};
    use std::sync::Arc;

    // Build the parameter source: all args except the last, joined by commas.
    let (params_src, body_src) = if args.is_empty() {
        (String::new(), String::new())
    } else if args.len() == 1 {
        (
            String::new(),
            vm.to_string(args.first().unwrap_or(&Value::Undefined))?
                .to_string(),
        )
    } else {
        let body = vm.to_string(&args[args.len() - 1])?.to_string();
        let params = args[..args.len() - 1]
            .iter()
            .map(|a| vm.to_string(a).map(|s| s.to_string()))
            .collect::<error::Result<Vec<String>>>()?
            .join(",");
        (params, body)
    };

    // Parse params + body together by wrapping in `function _f(PARAMS){ BODY }`,
    // so directives (e.g. "use strict") in the body are honored and the body
    // is parsed as a function statement list (not a top-level block).
    let wrapped = match (is_async, is_generator) {
        (true, true) => format!("async function* _f({}) {{ {} }}", params_src, body_src),
        (true, false) => format!("async function _f({}) {{ {} }}", params_src, body_src),
        (false, true) => format!("function* _f({}) {{ {} }}", params_src, body_src),
        _ => format!("function _f({}) {{ {} }}", params_src, body_src),
    };
    let prog = crate::parser::Parser::parse(&wrapped)?;
    let params_fn = prog
        .body
        .into_iter()
        .find_map(|st| match st.node {
            crate::ast::StmtNode::FunctionDecl(f) => Some(f),
            _ => None,
        })
        .ok_or_else(|| error::Error::syntax("invalid Function body".to_string()))?;
    let params = params_fn.params.clone();
    let param_defaults = params_fn.param_defaults.clone();
    let rest_param = params_fn.rest_param.clone();
    let body = params_fn.body.clone();
    // The parser already applied directive-inherited strictness; a body-level
    // "use strict" is reflected in the parsed function (is_strict).
    let is_strict = params_fn.is_strict;
    let f = FunctionExpr {
        name: Some(Arc::from("anonymous")),
        params,
        param_defaults,
        rest_param,
        body,
        is_arrow: false,
        is_async,
        is_generator,
        param_decls: Vec::new(),
        is_strict,
        is_method: false,
        has_name_binding: false,
    };
    let mut compiler = crate::compiler::Compiler::new();
    let (chunk, param_slots) = compiler.compile_function(&f)?;
    let chunk = vm.append_compiled_functions(chunk, compiler.take_functions());
    let function_realm = vm.native_callee_closure().unwrap_or(vm.global);
    let fdef = std::sync::Arc::new(crate::function::FunctionDef {
        name: Some(Arc::from("anonymous")),
        params: f.params.clone(),
        param_slots,
        rest_param: f.rest_param.clone(),
        chunk: std::sync::Arc::new(chunk),
        num_locals: f.params.len() + 16,
        is_arrow: false,
        is_async,
        is_generator,
        has_parameter_expressions: crate::compiler::Compiler::has_parameter_expressions(&f),
        length: crate::compiler::Compiler::fn_length(&f),
        is_method: false,
        has_name_binding: false,
        is_derived: false,
    });
    vm.functions.push(fdef.clone());
    let func_idx = vm.functions.len() - 1;
    // Create the function object with a fresh prototype.
    let has_prototype = !is_async || is_generator;
    let proto_val = if has_prototype {
        let proto = HeapObj::Object(crate::value::ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(if is_generator {
                if is_async {
                    vm.async_generator_prototype_for_env(function_realm)
                } else {
                    vm.generator_prototype_for_env(function_realm)
                }
            } else {
                vm.object_proto.clone()
            })),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        });
        Value::Object(GcIdx(vm.heap.allocate(proto)?))
    } else {
        Value::Undefined
    };
    let fallback_function_proto = if is_generator {
        let proto = if is_async {
            vm.async_generator_function_prototype_for_env(function_realm)
        } else {
            vm.generator_function_prototype_for_env(function_realm)
        };
        if matches!(proto, Value::Object(_)) {
            proto
        } else {
            let realm = crate::environment::global_env_root(&vm.heap, function_realm);
            vm.realm_function_prototypes
                .get(&realm.0)
                .cloned()
                .unwrap_or_else(|| vm.function_proto.clone())
        }
    } else if is_async {
        let realm = crate::environment::global_env_root(&vm.heap, function_realm);
        vm.realm_async_function_prototypes
            .get(&realm.0)
            .cloned()
            .unwrap_or_else(|| vm.function_proto.clone())
    } else {
        let realm = crate::environment::global_env_root(&vm.heap, function_realm);
        vm.realm_function_prototypes
            .get(&realm.0)
            .cloned()
            .unwrap_or_else(|| vm.function_proto.clone())
    };
    let function_object_proto = if is_generator {
        let intrinsic = if is_async {
            "AsyncGeneratorFunction"
        } else {
            "GeneratorFunction"
        };
        native_constructor_prototype_with_default(vm, intrinsic, fallback_function_proto.clone())?
    } else if let Some(proto) = vm.current_native_new_target_prototype.clone() {
        if matches!(proto, Value::Object(_)) {
            proto
        } else {
            fallback_function_proto.clone()
        }
    } else if let Some(new_target) = vm.current_native_new_target.clone() {
        let proto = vm.get_property_by_key(&new_target, &PropertyKey::from("prototype"))?;
        if matches!(proto, Value::Object(_)) {
            proto
        } else {
            fallback_function_proto.clone()
        }
    } else {
        fallback_function_proto
    };
    let mut props = IndexMap::new();
    let mut len_desc = PropertyDescriptor::data(Value::Number(fdef.length as f64));
    len_desc.writable = false;
    len_desc.enumerable = false;
    len_desc.configurable = true;
    props.insert(PropertyKey::from("length"), len_desc);
    let mut name_desc = PropertyDescriptor::data(Value::String(Arc::from("anonymous")));
    name_desc.writable = false;
    name_desc.enumerable = false;
    name_desc.configurable = true;
    props.insert(PropertyKey::from("name"), name_desc);
    if has_prototype {
        let mut proto_desc = PropertyDescriptor::data(proto_val.clone());
        proto_desc.enumerable = false;
        proto_desc.configurable = false;
        props.insert(PropertyKey::from("prototype"), proto_desc);
    }

    let fd = FunctionData {
        name: Some(Arc::from("anonymous")),
        kind: FunctionKind::Interpreted { func: fdef },
        closure: function_realm,
        lexical_new_target: Value::Undefined,
        home_object: Mutex::new(None),
        is_class_ctor: std::sync::atomic::AtomicBool::new(false),
        prototype: Mutex::new(has_prototype.then_some(proto_val.clone())),
        proto: Mutex::new(match function_object_proto {
            Value::Object(_) => Some(function_object_proto),
            _ => None,
        }),
        props: Mutex::new(props),
        extensible: std::sync::atomic::AtomicBool::new(true),
        private_fields: Mutex::new(std::collections::HashMap::new()),
    };
    let f_idx = vm.heap.allocate(HeapObj::Function(fd))?;
    // link prototype.constructor back to the function
    if has_prototype && !is_generator {
        if let Value::Object(pidx) = &proto_val {
            vm.heap.with_obj(pidx.0, |obj| {
                let mut desc = crate::value::PropertyDescriptor::data(Value::Object(GcIdx(f_idx)));
                desc.enumerable = false;
                obj.props()
                    .lock()
                    .insert(crate::value::PropertyKey::from("constructor"), desc);
            });
        }
    }
    // Emit MakeClosure at top level is not needed; the function object is
    // already fully formed. We do NOT push a frame; the caller invokes it.
    let _ = func_idx;
    Ok(Value::Object(GcIdx(f_idx)))
}
