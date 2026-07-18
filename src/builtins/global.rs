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
    if vm.current_native_new_target().is_some() {
        return Err(Error::type_err("BigInt is not a constructor"));
    }
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

fn parse_dynamic_function_source(
    params_src: &str,
    body_src: &str,
    is_generator: bool,
    is_async: bool,
) -> error::Result<crate::ast::FunctionExpr> {
    let wrapped = match (is_async, is_generator) {
        (true, true) => format!("async function* _f({params_src}\n) {{\n{body_src}\n}}"),
        (true, false) => format!("async function _f({params_src}\n) {{\n{body_src}\n}}"),
        (false, true) => format!("function* _f({params_src}\n) {{\n{body_src}\n}}"),
        (false, false) => format!("function _f({params_src}\n) {{\n{body_src}\n}}"),
    };
    let program = crate::parser::Parser::parse(&wrapped)?;
    let mut statements = program.body.into_iter();
    let Some(statement) = statements.next() else {
        return Err(error::Error::syntax("invalid Function body".to_string()));
    };
    if statements.next().is_some() {
        return Err(error::Error::syntax("invalid Function body".to_string()));
    }
    let crate::ast::StmtNode::FunctionDecl(function) = statement.node else {
        return Err(error::Error::syntax("invalid Function body".to_string()));
    };
    if function.name.as_deref() != Some("_f")
        || function.is_async != is_async
        || function.is_generator != is_generator
    {
        return Err(error::Error::syntax("invalid Function body".to_string()));
    }
    Ok(function)
}

fn dynamic_function_prototype_with_default(
    vm: &mut Vm,
    intrinsic: &str,
    fallback: Value,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_some() {
        return native_constructor_prototype_with_default(vm, intrinsic, fallback);
    }
    let Some(constructor) = vm.current_native_callee().cloned() else {
        return Ok(fallback);
    };
    let prototype = vm.get_property_by_key(&constructor, &PropertyKey::from("prototype"))?;
    if matches!(prototype, Value::Object(_)) {
        return Ok(prototype);
    }
    vm.constructor_realm_default_prototype(&constructor, intrinsic, fallback)
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

    // CreateDynamicFunction converts every parameter before the body. Keeping
    // the loop explicit also preserves abrupt-completion order.
    let (params_src, body_src) = if args.is_empty() {
        (String::new(), String::new())
    } else if args.len() == 1 {
        (
            String::new(),
            vm.to_string(args.first().unwrap_or(&Value::Undefined))?
                .to_string(),
        )
    } else {
        let mut param_strings = Vec::with_capacity(args.len() - 1);
        for arg in &args[..args.len() - 1] {
            param_strings.push(vm.to_string(arg)?.to_string());
        }
        let params = param_strings.join(",");
        let body = vm.to_string(&args[args.len() - 1])?.to_string();
        (params, body)
    };

    // RuJa's local-trust host policy permits string compilation unconditionally.
    // This is the HostEnsureCanCompileStrings boundary: it must remain after
    // every observable ToString and before any parse or prototype lookup.

    // Parameters and body are separate grammar parses in ECMA-262. Validate
    // each side independently so comments or delimiters cannot bridge the
    // synthetic boundary, then parse the combined source for direct early
    // errors such as a strict body with non-simple parameters.
    let params_only = parse_dynamic_function_source(&params_src, "", is_generator, is_async)?;
    if crate::compiler::Compiler::parameter_prelude_len(&params_only) != params_only.body.len() {
        return Err(error::Error::syntax(
            "invalid Function parameters".to_string(),
        ));
    }
    let body_only = parse_dynamic_function_source("", &body_src, is_generator, is_async)?;
    if !body_only.params.is_empty() || body_only.rest_param.is_some() {
        return Err(error::Error::syntax("invalid Function body".to_string()));
    }
    let params_fn = parse_dynamic_function_source(&params_src, &body_src, is_generator, is_async)?;
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
    let compiled_functions = compiler.take_functions();
    let function_realm = vm.native_callee_closure().unwrap_or(vm.global);
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
    let intrinsic = if is_generator {
        if is_async {
            "AsyncGeneratorFunction"
        } else {
            "GeneratorFunction"
        }
    } else if is_async {
        "AsyncFunction"
    } else {
        "Function"
    };
    let function_object_proto =
        dynamic_function_prototype_with_default(vm, intrinsic, fallback_function_proto)?;
    let mut pin_count = vm.pin(&function_object_proto);
    let function_checkpoint = vm.functions.len();
    let chunk = vm.append_compiled_functions(chunk, compiled_functions);
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
    let fd = FunctionData {
        name: Some(Arc::from("anonymous")),
        kind: FunctionKind::Interpreted { func: fdef },
        closure: function_realm,
        lexical_new_target: Value::Undefined,
        home_object: Mutex::new(None),
        is_class_ctor: std::sync::atomic::AtomicBool::new(false),
        prototype: Mutex::new(None),
        proto: Mutex::new(match &function_object_proto {
            Value::Object(_) => Some(function_object_proto.clone()),
            _ => None,
        }),
        props: Mutex::new(props),
        extensible: std::sync::atomic::AtomicBool::new(true),
        private_fields: Mutex::new(std::collections::HashMap::new()),
    };
    let result = (|| -> error::Result<Value> {
        let f_idx = vm.alloc(HeapObj::Function(fd))?;
        let function = Value::Object(f_idx);
        pin_count += vm.pin(&function);

        let has_prototype = !is_async || is_generator;
        if has_prototype {
            let prototype_parent = if is_generator {
                if is_async {
                    vm.async_generator_prototype_for_env(function_realm)
                } else {
                    vm.generator_prototype_for_env(function_realm)
                }
            } else {
                vm.object_prototype_for_env(function_realm)
            };
            pin_count += vm.pin(&prototype_parent);
            let prototype_idx = vm.alloc(HeapObj::Object(crate::value::ObjectData {
                props: Mutex::new(IndexMap::new()),
                proto: Mutex::new(Some(prototype_parent)),
                extensible: AtomicBool::new(true),
                class_name: None,
                private_fields: Mutex::new(std::collections::HashMap::new()),
                primitive: Mutex::new(None),
            }))?;
            let prototype = Value::Object(prototype_idx);
            pin_count += vm.pin(&prototype);

            let mut descriptor = PropertyDescriptor::data(prototype.clone());
            descriptor.enumerable = false;
            descriptor.configurable = false;
            vm.heap.with_obj(f_idx.0, |object| {
                let HeapObj::Function(data) = object else {
                    return;
                };
                *data.prototype.lock() = Some(prototype.clone());
                data.props
                    .lock()
                    .insert(PropertyKey::from("prototype"), descriptor);
            });

            if !is_generator {
                vm.heap.with_obj(prototype_idx.0, |object| {
                    let mut descriptor = PropertyDescriptor::data(function.clone());
                    descriptor.enumerable = false;
                    object
                        .props()
                        .lock()
                        .insert(PropertyKey::from("constructor"), descriptor);
                });
            }
        }
        Ok(function)
    })();
    vm.unpin_many(pin_count);
    if result.is_err() {
        vm.functions.truncate(function_checkpoint);
    }
    result
}
