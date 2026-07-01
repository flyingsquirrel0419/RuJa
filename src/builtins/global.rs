use super::*;

// =========================================================================
// Global functions
// =========================================================================
pub(crate) fn global_parse_int(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let input = match args.first() {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(v) => vm.to_string(v)?.to_string(),
        None => return Ok(Value::Number(f64::NAN)),
    };
    let mut radix = args
        .get(1)
        .and_then(|v| {
            if let Value::Number(n) = v {
                Some(*n as u32)
            } else {
                None
            }
        })
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
    let valid = |c: char| c.is_digit(radix);
    let start = chars.peek().map(|(i, _)| *i).unwrap_or(input.len());
    let digits_end = input[start..]
        .char_indices()
        .find(|(_, c)| !valid(*c))
        .map(|(i, _)| start + i)
        .unwrap_or(input.len());
    let digits = &input[start..digits_end];
    if digits.is_empty() {
        return Ok(Value::Number(f64::NAN));
    }
    match i64::from_str_radix(digits, radix) {
        Ok(n) => Ok(Value::Number(if neg { -(n as f64) } else { n as f64 })),
        Err(_) => Ok(Value::Number(f64::NAN)),
    }
}
pub(crate) fn global_parse_float(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
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
pub(crate) fn global_is_finite(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let n = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    Ok(Value::Bool(n.is_finite()))
}

/// `BigInt(x)`: convert a number, string, or boolean to a BigInt. Throws
/// RangeError for non-integral numbers and SyntaxError for unparseable strings.
pub(crate) fn global_bigint(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let arg = args.first().unwrap_or(&Value::Undefined);
    match arg {
        Value::BigInt(n) => Ok(Value::BigInt(n.clone())),
        Value::Bool(b) => Ok(Value::BigInt(num_bigint::BigInt::from(if *b {
            1
        } else {
            0
        }))),
        Value::Number(n) => {
            if n.is_nan() || n.is_infinite() || n.fract() != 0.0 {
                Err(Error::range(format!(
                    "The number {} cannot be converted to a BigInt because it is not an integer",
                    crate::value::num_to_string(*n)
                )))
            } else {
                Ok(Value::BigInt(num_bigint::BigInt::from(*n as i64)))
            }
        }
        Value::String(s) => {
            let t = s.trim();
            num_bigint::BigInt::parse_bytes(t.as_bytes(), 10)
                .map(Value::BigInt)
                .ok_or_else(|| Error::syntax(format!("Cannot convert {} to a BigInt", s)))
        }
        _ => Err(Error::syntax("Cannot convert to a BigInt".to_string())),
    }
}

/// `BigInt.prototype.toString()`: returns the decimal string of the BigInt.
pub(crate) fn bigint_to_string(_vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let _ = args;
    let v = match this {
        Some(Value::BigInt(n)) => n.clone(),
        Some(_) => num_bigint::BigInt::from(0),
        None => num_bigint::BigInt::from(0),
    };
    Ok(Value::String(Arc::from(v.to_string().as_str())))
}

/// `eval(x)`: if `x` is not a string, return it as-is. Otherwise parse and
/// run it. A direct `eval(...)` call (detected via the CallDirectEval opcode)
/// runs in the caller's scope; an indirect eval runs in the global scope.
pub(crate) fn global_eval(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let arg = args.first().cloned().unwrap_or(Value::Undefined);
    let src = match &arg {
        Value::String(s) => s.to_string(),
        // Non-string: return unchanged.
        _ => return Ok(arg),
    };
    // Default (indirect) behavior: run in the global scope.
    vm.eval_indirect(&src)
}

/// `new Function(p0, p1, ..., body)`: dynamically build a function from a
/// parameter list and a body source string. The last argument is the body;
/// earlier arguments are parameter names (comma-separated within each).
pub(crate) fn function_constructor(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    use crate::ast::FunctionExpr;
    use crate::value::{FunctionData, FunctionKind};
    use std::sync::Arc;

    // Build the parameter source: all args except the last, joined by commas.
    let (params_src, body_src) = if args.is_empty() {
        (String::new(), String::new())
    } else if args.len() == 1 {
        (String::new(), vm.to_string(args.first().unwrap_or(&Value::Undefined))?.to_string())
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
    let wrapped = format!("function _f({}) {{ {} }}", params_src, body_src);
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
        is_async: false,
        is_generator: false,
        param_decls: Vec::new(),
        is_strict,
    };
    let mut compiler = crate::compiler::Compiler::new();
    let (chunk, param_slots) = compiler.compile_function(&f)?;
    let fdef = std::sync::Arc::new(crate::function::FunctionDef {
        name: Some(Arc::from("anonymous")),
        params: f.params.clone(),
        param_slots,
        rest_param: f.rest_param.clone(),
        chunk: std::sync::Arc::new(chunk),
        num_locals: f.params.len() + 16,
        is_arrow: false,
        is_async: false,
        is_generator: false,
        length: crate::compiler::Compiler::fn_length(&f),
    });
    vm.functions.push(fdef.clone());
    let func_idx = vm.functions.len() - 1;
    // Create the function object with a fresh prototype.
    let proto = HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    let proto_val = Value::Object(GcIdx(vm.heap.allocate(proto)));
    let fd = FunctionData {
        name: Some(Arc::from("anonymous")),
        kind: FunctionKind::Interpreted { func: fdef },
        closure: vm.global,
        prototype: Mutex::new(Some(proto_val.clone())),
        proto: Mutex::new(match vm.function_proto {
            Value::Object(_) => Some(vm.function_proto.clone()),
            _ => None,
        }),
        props: Mutex::new(IndexMap::new()),
    };
    let f_idx = vm.heap.allocate(HeapObj::Function(fd));
    // link prototype.constructor back to the function
    if let Value::Object(pidx) = &proto_val {
        vm.heap.with_obj(pidx.0, |obj| {
            let mut desc = crate::value::PropertyDescriptor::data(Value::Object(GcIdx(f_idx)));
            desc.enumerable = false;
            obj.props()
                .lock()
                .insert(crate::value::PropertyKey::from("constructor"), desc);
        });
    }
    // Emit MakeClosure at top level is not needed; the function object is
    // already fully formed. We do NOT push a frame; the caller invokes it.
    let _ = func_idx;
    Ok(Value::Object(GcIdx(f_idx)))
}

