use super::*;

// =========================================================================
// Math
// =========================================================================
fn math_unary(f: fn(f64) -> f64, vm: &mut Vm, args: &[Value]) -> error::Result<Value> {
    let n = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    Ok(Value::Number(f(n)))
}
fn math_floor(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::floor, vm, args)
}
fn math_ceil(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::ceil, vm, args)
}
fn math_round(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    // ES Math.round rounds half toward +Infinity while preserving -0 for
    // inputs in [-0.5, -0].
    math_unary(
        |n| {
            if n.is_nan() || n.is_infinite() {
                n
            } else if n < 0.0 && n >= -0.5 || n == 0.0 && n.is_sign_negative() {
                -0.0
            } else if (0.0..0.5).contains(&n) {
                0.0
            } else if n.fract() == 0.0 {
                n
            } else {
                (n + 0.5).floor()
            }
        },
        vm,
        args,
    )
}
fn math_trunc(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::trunc, vm, args)
}
fn math_abs(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::abs, vm, args)
}
fn math_sign(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let n = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    Ok(Value::Number(if n > 0.0 {
        1.0
    } else if n < 0.0 {
        -1.0
    } else {
        n
    }))
}
fn math_sqrt(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::sqrt, vm, args)
}
fn math_cbrt(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::cbrt, vm, args)
}
fn math_exp(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::exp, vm, args)
}
fn math_log(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::ln, vm, args)
}
fn math_log2(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::log2, vm, args)
}
fn math_log10(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::log10, vm, args)
}

fn math_hypot(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let mut sum = 0.0f64;
    let mut has_inf = false;
    for a in args {
        let n = vm.to_number(a)?;
        if n.is_infinite() {
            has_inf = true;
        } else {
            sum += n * n;
        }
    }
    Ok(Value::Number(if has_inf {
        f64::INFINITY
    } else {
        sum.sqrt()
    }))
}
fn math_atan2(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let y = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    let x = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    Ok(Value::Number(y.atan2(x)))
}
fn math_asin(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::asin, vm, args)
}
fn math_acos(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::acos, vm, args)
}
fn math_atan(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::atan, vm, args)
}
fn math_acosh(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::acosh, vm, args)
}
fn math_asinh(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::asinh, vm, args)
}
fn math_atanh(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::atanh, vm, args)
}
fn math_sinh(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::sinh, vm, args)
}
fn math_cosh(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::cosh, vm, args)
}
fn math_tanh(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::tanh, vm, args)
}
fn math_expm1(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::exp_m1, vm, args)
}
fn math_log1p(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::ln_1p, vm, args)
}
fn math_clz32(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let n = crate::vm::to_uint32(vm.to_number(args.first().unwrap_or(&Value::Undefined))?);
    Ok(Value::Number(n.leading_zeros() as f64))
}
fn math_imul(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let a = crate::vm::to_int32(vm.to_number(args.first().unwrap_or(&Value::Undefined))?);
    let b = crate::vm::to_int32(vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?);
    Ok(Value::Number(a.wrapping_mul(b) as f64))
}
fn math_fround(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let n = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    Ok(Value::Number(n as f32 as f64))
}
fn math_sin(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::sin, vm, args)
}
fn math_cos(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::cos, vm, args)
}
fn math_tan(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::tan, vm, args)
}
fn math_pow(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let a = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    let b = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    Ok(Value::Number(a.powf(b)))
}
fn math_max(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let mut m = f64::NEG_INFINITY;
    let mut saw_nan = false;
    for a in args {
        let n = vm.to_number(a)?;
        if n.is_nan() {
            saw_nan = true;
        } else if n > m || n == 0.0 && m == 0.0 && n.is_sign_positive() && m.is_sign_negative() {
            m = n;
        }
    }
    Ok(Value::Number(if saw_nan { f64::NAN } else { m }))
}
fn math_min(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let mut m = f64::INFINITY;
    let mut saw_nan = false;
    for a in args {
        let n = vm.to_number(a)?;
        if n.is_nan() {
            saw_nan = true;
        } else if n < m || n == 0.0 && m == 0.0 && n.is_sign_negative() && m.is_sign_positive() {
            m = n;
        }
    }
    Ok(Value::Number(if saw_nan { f64::NAN } else { m }))
}
fn math_random(_vm: &mut Vm, _args: &[Value], _: Option<Value>) -> error::Result<Value> {
    thread_local! { static STATE: AtomicU64 = const { AtomicU64::new(0x2545F4914F6CDD1D) }; }
    let r = STATE.with(|s| {
        let mut x = s.load(Ordering::Relaxed);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.store(x, Ordering::Relaxed);
        x as f64 / u64::MAX as f64
    });
    Ok(Value::Number(r))
}

pub(crate) fn build_math(vm: &mut Vm) -> error::Result<Value> {
    let mut props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    // build methods first, collect into a temp vec
    let mut method_entries: Vec<(&str, NativeFn, usize)> = vec![
        ("floor", math_floor, 1),
        ("ceil", math_ceil, 1),
        ("round", math_round, 1),
        ("trunc", math_trunc, 1),
        ("abs", math_abs, 1),
        ("sign", math_sign, 1),
        ("sqrt", math_sqrt, 1),
        ("cbrt", math_cbrt, 1),
        ("exp", math_exp, 1),
        ("log", math_log, 1),
        ("log2", math_log2, 1),
        ("log10", math_log10, 1),
        ("log1p", math_log1p, 1),
        ("expm1", math_expm1, 1),
        ("sin", math_sin, 1),
        ("asin", math_asin, 1),
        ("acos", math_acos, 1),
        ("atan", math_atan, 1),
        ("atan2", math_atan2, 2),
        ("acosh", math_acosh, 1),
        ("asinh", math_asinh, 1),
        ("atanh", math_atanh, 1),
        ("sinh", math_sinh, 1),
        ("cosh", math_cosh, 1),
        ("tanh", math_tanh, 1),
        ("hypot", math_hypot, 2),
        ("clz32", math_clz32, 1),
        ("fround", math_fround, 1),
        ("imul", math_imul, 2),
        ("cos", math_cos, 1),
        ("tan", math_tan, 1),
        ("pow", math_pow, 2),
        ("max", math_max, 2),
        ("min", math_min, 2),
        ("random", math_random, 0),
    ];
    for (name, f, len) in method_entries.drain(..) {
        let idx = vm.new_native_function(name, f, len)?;
        props.insert(PropertyKey::from(name), data_prop(Value::Object(idx)));
    }
    props.insert(
        PropertyKey::from("PI"),
        const_prop(Value::Number(std::f64::consts::PI)),
    );
    props.insert(
        PropertyKey::from("E"),
        const_prop(Value::Number(std::f64::consts::E)),
    );
    props.insert(
        PropertyKey::from("LN2"),
        const_prop(Value::Number(std::f64::consts::LN_2)),
    );
    props.insert(
        PropertyKey::from("LN10"),
        const_prop(Value::Number(std::f64::consts::LN_10)),
    );
    props.insert(
        PropertyKey::from("LOG2E"),
        const_prop(Value::Number(std::f64::consts::LOG2_E)),
    );
    props.insert(
        PropertyKey::from("LOG10E"),
        const_prop(Value::Number(std::f64::consts::LOG10_E)),
    );
    props.insert(
        PropertyKey::from("SQRT2"),
        const_prop(Value::Number(std::f64::consts::SQRT_2)),
    );
    props.insert(
        PropertyKey::from("SQRT1_2"),
        const_prop(Value::Number(std::f64::consts::FRAC_1_SQRT_2)),
    );
    let obj = HeapObj::Object(ObjectData {
        props: Mutex::new(props),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Math")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj)?)))
}

// =========================================================================
// console
// =========================================================================
pub(crate) fn console_log(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let parts: Vec<String> = args
        .iter()
        .map(|a| format_for_console(vm, a, 0).unwrap_or_default())
        .collect();
    println!("{}", parts.join(" "));
    Ok(Value::Undefined)
}

/// Format a value for console output, approximating Node.js's inspect format:
/// arrays as `[ 1, 2, 3 ]`, objects as `{ a: 1, b: 2 }`, strings unquoted
/// at top level, nested strings quoted.
pub(crate) fn format_for_console(vm: &mut Vm, v: &Value, depth: usize) -> error::Result<String> {
    if depth > 6 {
        return Ok(vm.to_string(v)?.to_string());
    }
    match v {
        Value::Undefined => Ok("undefined".to_string()),
        Value::Null => Ok("null".to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) => Ok(crate::value::num_to_string(*n)),
        Value::BigInt(n) => Ok(format!("{}n", n)),
        Value::String(s) => {
            if depth == 0 {
                Ok(s.to_string())
            } else {
                Ok(format!("'{}'", s))
            }
        }
        Value::Symbol(_) => Ok("Symbol()".to_string()),
        Value::Reference(_) => Ok("[reference]".to_string()),
        Value::Object(idx) => {
            let (is_array, is_func, items, pairs) = vm.heap.with_obj(idx.0, |o| {
                let is_array = matches!(o, HeapObj::Array(_));
                let is_func = matches!(o, HeapObj::Function(_));
                let items = if let HeapObj::Array(a) = o {
                    a.items.lock().clone()
                } else {
                    Vec::new()
                };
                let pairs: Vec<(Arc<str>, Value)> = o
                    .props()
                    .lock()
                    .iter()
                    .filter(|(_, d)| d.enumerable)
                    .filter_map(|(k, d)| {
                        if let crate::value::PropertyKey::Str(s) = k {
                            Some((s.clone(), d.value.clone()))
                        } else {
                            None
                        }
                    })
                    .collect();
                (is_array, is_func, items, pairs)
            });
            if is_func {
                return Ok("[Function]".to_string());
            }
            if is_array {
                if items.is_empty() {
                    return Ok("[]".to_string());
                }
                let inner: Vec<String> = items
                    .iter()
                    .map(|i| format_for_console(vm, i, depth + 1))
                    .collect::<error::Result<Vec<_>>>()?;
                return Ok(format!("[ {} ]", inner.join(", ")));
            }
            if pairs.is_empty() {
                return Ok("{}".to_string());
            }
            let inner: Vec<String> = pairs
                .iter()
                .map(|(k, v)| {
                    let val = format_for_console(vm, v, depth + 1)?;
                    Ok(format!("{}: {}", k, val))
                })
                .collect::<error::Result<Vec<_>>>()?;
            Ok(format!("{{ {} }}", inner.join(", ")))
        }
    }
}
pub(crate) fn build_console(vm: &mut Vm) -> error::Result<Value> {
    let mut props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    for name in &["log", "error", "warn", "info", "debug", "dir", "trace"] {
        let idx = vm.new_native_function(name, console_log, 0)?;
        props.insert(PropertyKey::from(*name), data_prop(Value::Object(idx)));
    }
    let obj = HeapObj::Object(ObjectData {
        props: Mutex::new(props),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Object")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj)?)))
}
