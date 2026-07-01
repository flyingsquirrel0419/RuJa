use super::*;

// =========================================================================
// JSON
// =========================================================================
/// Returns the numeric array index if `s` is a canonical decimal integer in
/// [0, 2^32-1) (no leading zeros), else None. Used to order keys like Object.keys.
fn json_array_index(s: &str) -> Option<u32> {
    if s.is_empty() || (s.len() > 1 && s.starts_with('0')) || !s.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    s.parse::<u32>().ok().filter(|n| (*n as u64) < (1u64 << 32))
}

pub(crate) fn json_stringify(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let v = args.first().unwrap_or(&Value::Undefined).clone();
    let replacer = args.get(1).cloned().unwrap_or(Value::Undefined);
    let space_arg = args.get(2).cloned().unwrap_or(Value::Undefined);

    // Determine the gap (indentation) string.
    let gap: String = match &space_arg {
        Value::Number(n) => {
            let n = (*n as usize).min(10);
            " ".repeat(n)
        }
        Value::String(s) => {
            if s.len() <= 10 {
                s.to_string()
            } else {
                s[..10].to_string()
            }
        }
        _ => String::new(),
    };

    // Build the replacer whitelist from an array replacer.
    let whitelist: Option<Vec<String>> = if let Value::Object(idx) = &replacer {
        let is_arr = vm.heap.with_obj(idx.0, |o| matches!(o, HeapObj::Array(_)));
        if is_arr {
            let items = vm.heap.with_obj(idx.0, |o| {
                if let HeapObj::Array(a) = o {
                    a.items.lock().clone()
                } else {
                    Vec::new()
                }
            });
            let mut wl = Vec::new();
            for item in items {
                match item {
                    Value::String(s) => wl.push(s.to_string()),
                    Value::Number(n) => wl.push(crate::value::num_to_string(n)),
                    _ => {}
                }
            }
            Some(wl)
        } else {
            None
        }
    } else {
        None
    };
    let replacer_fn = if matches!(replacer, Value::Object(_)) && whitelist.is_none() {
        let is_fn = if let Value::Object(idx) = &replacer {
            vm.heap.with_obj(idx.0, |o| o.is_function())
        } else {
            false
        };
        if is_fn {
            Some(replacer.clone())
        } else {
            None
        }
    } else {
        None
    };

    // Reject circular references per ECMAScript (TypeError).
    if let Value::Object(_) = &v {
        if has_json_cycle(vm, &v, &mut Vec::new()) {
            return Err(Error::type_err(
                "Converting circular structure to JSON".to_string(),
            ));
        }
    }
    let mut ctx = StringifyCtx {
        gap,
        whitelist,
        replacer_fn,
    };
    match stringify_value(vm, &v, &mut Vec::new(), "", &mut ctx, 0) {
        Some(s) => Ok(Value::String(Arc::from(s.as_str()))),
        None => Ok(Value::Undefined),
    }
}

struct StringifyCtx {
    gap: String,
    whitelist: Option<Vec<String>>,
    replacer_fn: Option<Value>,
}

/// Detect whether `v` (transitively) contains a cycle through object/array
/// references. Strings, numbers, and other primitives are never cyclic.
fn has_json_cycle(vm: &mut Vm, v: &Value, seen: &mut Vec<usize>) -> bool {
    has_json_cycle_depth(vm, v, seen, 0)
}

fn has_json_cycle_depth(vm: &mut Vm, v: &Value, seen: &mut Vec<usize>, depth: usize) -> bool {
    // Guard the recursion so deep (but acyclic) input cannot overflow the
    // native stack before stringify_value's own depth cap is reached.
    if depth > 256 {
        return false;
    }
    let idx = match v {
        Value::Object(idx) => idx.0,
        _ => return false,
    };
    if seen.contains(&idx) {
        return true;
    }
    seen.push(idx);
    // Collect child values out of the borrow scope before recursing.
    let children: Vec<Value> = vm.heap.with_obj(idx, |obj| match obj {
        HeapObj::Array(a) => a.items.lock().clone(),
        HeapObj::Object(o) => o
            .props
            .lock()
            .values()
            .filter(|d| d.enumerable)
            .map(|d| d.value.clone())
            .collect(),
        _ => Vec::new(),
    });
    let result = children
        .iter()
        .any(|c| has_json_cycle_depth(vm, c, seen, depth + 1));
    seen.pop();
    result
}
fn stringify_value(
    vm: &mut Vm,
    v: &Value,
    seen: &mut Vec<usize>,
    indent: &str,
    ctx: &mut StringifyCtx,
    depth: usize,
) -> Option<String> {
    // Guard against deeply-nested user values overflowing the native stack.
    const MAX_STRINGIFY_DEPTH: usize = 256;
    if depth > MAX_STRINGIFY_DEPTH {
        return None;
    }
    // (Top-level replacer application is handled by callers; this function
    //  applies the replacer per-property via apply_replacer.)
    match v.clone() {
        Value::Undefined => None,
        Value::Null => Some("null".into()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(if n.is_nan() || n.is_infinite() {
            "null".to_string()
        } else {
            crate::value::num_to_string(n)
        }),
        Value::BigInt(n) => Some(n.to_string()),
        Value::String(s) => Some(format!(
            "\"{}\"",
            s.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\t', "\\t")
        )),
        Value::Symbol(_) => None,
        Value::Object(idx) => {
            if seen.contains(&idx.0) {
                return None;
            }
            seen.push(idx.0);
            let is_function = vm.heap.with_obj(idx.0, |obj| obj.is_function());
            if is_function {
                seen.pop();
                return None;
            }
            let (is_arr, items, props) = vm.heap.with_obj(idx.0, |obj| match obj {
                HeapObj::Array(a) => (true, a.items.lock().clone(), IndexMap::new()),
                HeapObj::Object(o) => (false, Vec::new(), o.props.lock().clone()),
                HeapObj::Function(_) => (false, Vec::new(), IndexMap::new()),
                _ => (false, Vec::new(), obj.props().lock().clone()),
            });
            let child_indent = if ctx.gap.is_empty() {
                String::new()
            } else {
                format!("{}{}", indent, ctx.gap)
            };
            if is_arr {
                let parts: Vec<String> = items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| {
                        // Apply replacer
                        let val = apply_replacer(
                            vm,
                            ctx,
                            &Value::String(Arc::from(i.to_string().as_str())),
                            item,
                        );
                        let s = stringify_value(vm, &val, seen, &child_indent, ctx, depth + 1);
                        let s = s.unwrap_or_else(|| "null".to_string());
                        if ctx.gap.is_empty() {
                            s
                        } else {
                            format!("{}{}", child_indent, s)
                        }
                    })
                    .collect();
                seen.pop();
                if parts.is_empty() {
                    Some("[]".into())
                } else if ctx.gap.is_empty() {
                    Some(format!("[{}]", parts.join(",")))
                } else {
                    Some(format!("[\n{}\n{}]", parts.join(",\n"), indent))
                }
            } else {
                let mut pairs = Vec::new();
                let mut keys: Vec<(String, Value)> = if let Some(wl) = &ctx.whitelist {
                    props
                        .iter()
                        .filter_map(|(k, d)| {
                            let ks = match k {
                                crate::value::PropertyKey::Str(s) => s.to_string(),
                                _ => return None,
                            };
                            if wl.contains(&ks) && d.enumerable {
                                Some((ks, d.value.clone()))
                            } else {
                                None
                            }
                        })
                        .collect()
                } else {
                    props
                        .iter()
                        .filter_map(|(k, d)| {
                            if !d.enumerable {
                                return None;
                            }
                            match k {
                                crate::value::PropertyKey::Str(s) => {
                                    Some((s.to_string(), d.value.clone()))
                                }
                                _ => None,
                            }
                        })
                        .collect()
                };
                // ES enumeration order: array-index keys ascending, then the
                // rest in insertion order (props preserves insertion order).
                keys.sort_by(
                    |(a, _), (b, _)| match (json_array_index(a), json_array_index(b)) {
                        (Some(x), Some(y)) => x.cmp(&y),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    },
                );
                for (key_str, val) in keys {
                    let val =
                        apply_replacer(vm, ctx, &Value::String(Arc::from(key_str.as_str())), &val);
                    if let Some(vs) = stringify_value(vm, &val, seen, &child_indent, ctx, depth + 1)
                    {
                        if ctx.gap.is_empty() {
                            pairs.push(format!("\"{}\":{}", key_str, vs));
                        } else {
                            pairs.push(format!("{}\"{}\": {}", child_indent, key_str, vs));
                        }
                    }
                }
                seen.pop();
                if pairs.is_empty() {
                    Some("{}".into())
                } else if ctx.gap.is_empty() {
                    Some(format!("{{{}}}", pairs.join(",")))
                } else {
                    Some(format!("{{\n{}\n{}}}", pairs.join(",\n"), indent))
                }
            }
        }
    }
}

/// Apply a function replacer: replacer(key, value) -> new value.
fn apply_replacer(vm: &mut Vm, ctx: &StringifyCtx, key: &Value, val: &Value) -> Value {
    if let Some(rf) = &ctx.replacer_fn {
        vm.call_function(rf, &[key.clone(), val.clone()], Some(val.clone()))
            .unwrap_or_else(|_| val.clone())
    } else {
        val.clone()
    }
}

pub(crate) fn json_parse(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let s = match args.first() {
        Some(Value::String(s)) => s.to_string(),
        _ => return Ok(Value::Null),
    };
    let reviver = args.get(1).cloned();
    let is_reviver_fn = if let Some(Value::Object(idx)) = &reviver {
        vm.heap.with_obj(idx.0, |o| o.is_function())
    } else {
        false
    };
    let parsed = parse_json_value(vm, &mut s.chars().peekable(), 0)?;
    if is_reviver_fn {
        if let Some(rf) = reviver {
            return apply_reviver(vm, &rf, &Value::String(Arc::from("")), &parsed, 0);
        }
    }
    Ok(parsed)
}

/// Walk the parsed tree bottom-up, calling reviver(key, value) on each.
fn apply_reviver(
    vm: &mut Vm,
    reviver: &Value,
    key: &Value,
    val: &Value,
    depth: usize,
) -> error::Result<Value> {
    // The parse step already caps nesting, but guard defensively.
    if depth > 256 {
        return Err(Error::syntax(
            "Maximum JSON nesting depth exceeded".to_string(),
        ));
    }
    let walked = match val {
        Value::Object(idx) => {
            let (is_arr, items, props) = vm.heap.with_obj(idx.0, |o| match o {
                HeapObj::Array(a) => (true, a.items.lock().clone(), IndexMap::new()),
                HeapObj::Object(o) => (false, Vec::new(), o.props.lock().clone()),
                _ => (false, Vec::new(), IndexMap::new()),
            });
            if is_arr {
                let mut new_items = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    let k = Value::String(Arc::from(i.to_string().as_str()));
                    let w = apply_reviver(vm, reviver, &k, item, depth + 1)?;
                    if !w.is_undefined() {
                        new_items.push(w);
                    }
                }
                Value::Object(GcIdx(vm.heap.allocate(HeapObj::Array(
                    crate::value::ArrayData {
                        items: Mutex::new(new_items),
                        props: Mutex::new(IndexMap::new()),
                        proto: Mutex::new(Some(vm.array_proto.clone())),
                        sparse_max: Mutex::new(None),
                    },
                ))))
            } else {
                let mut new_props = IndexMap::new();
                for (pk, d) in &props {
                    if let crate::value::PropertyKey::Str(s) = pk {
                        let k = Value::String(s.clone());
                        let w = apply_reviver(vm, reviver, &k, &d.value, depth + 1)?;
                        if !w.is_undefined() {
                            let mut desc = data_prop(w);
                            desc.enumerable = true;
                            new_props.insert(pk.clone(), desc);
                        }
                    }
                }
                Value::Object(GcIdx(vm.heap.allocate(HeapObj::Object(
                    crate::value::ObjectData {
                        props: Mutex::new(new_props),
                        proto: Mutex::new(Some(vm.object_proto.clone())),
                        extensible: AtomicBool::new(true),
                        class_name: None,
                        private_fields: Mutex::new(std::collections::HashMap::new()),
                        primitive: Mutex::new(None),
                    },
                ))))
            }
        }
        _ => val.clone(),
    };
    // Call the reviver on this level.
    let result = vm.call_function(
        reviver,
        &[key.clone(), walked.clone()],
        Some(walked.clone()),
    )?;
    Ok(result)
}
fn parse_json_value(
    vm: &mut Vm,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    depth: usize,
) -> error::Result<Value> {
    // Guard against pathological nesting that would overflow the native
    // stack: `JSON.parse("[".repeat(100000)+...]")` used to abort the host.
    // Node tolerates deep nesting on its larger stack; we cap recursion and
    // surface a SyntaxError instead of crashing.
    const MAX_JSON_DEPTH: usize = 256;
    if depth > MAX_JSON_DEPTH {
        return Err(Error::syntax(
            "Maximum JSON nesting depth exceeded".to_string(),
        ));
    }
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
    match chars.peek() {
        Some(&'{') => {
            chars.next();
            parse_json_obj(vm, chars, depth)
        }
        Some(&'[') => {
            chars.next();
            parse_json_arr(vm, chars, depth)
        }
        Some(&'"') => {
            chars.next();
            parse_json_str(chars)
        }
        Some('t') => {
            chars.take(4).for_each(|_| {});
            Ok(Value::Bool(true))
        }
        Some('f') => {
            chars.take(5).for_each(|_| {});
            Ok(Value::Bool(false))
        }
        Some('n') => {
            chars.take(4).for_each(|_| {});
            Ok(Value::Null)
        }
        Some(c) if *c == '-' || c.is_ascii_digit() => parse_json_num(chars),
        _ => Err(Error::syntax("Invalid JSON".to_string())),
    }
}
fn parse_json_obj(
    vm: &mut Vm,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    depth: usize,
) -> error::Result<Value> {
    let mut props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    loop {
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }
        if chars.peek() == Some(&'}') {
            chars.next();
            break;
        }
        // consume the opening quote of the key string
        if chars.peek() == Some(&'"') {
            chars.next();
        }
        let key = match parse_json_str(chars)? {
            Value::String(s) => s.to_string(),
            _ => String::new(),
        };
        while chars.peek() != Some(&':') {
            match chars.peek() {
                None => return Err(Error::syntax("Invalid JSON: expected ':'".to_string())),
                Some(&_) => {
                    chars.next();
                }
            }
        }
        chars.next();
        let val = parse_json_value(vm, chars, depth + 1)?;
        // JSON-parsed properties are enumerable (data_prop is non-enumerable for builtins).
        let mut desc = data_prop(val);
        desc.enumerable = true;
        props.insert(PropertyKey::from(key.as_str()), desc);
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == ',' {
                chars.next();
            } else {
                break;
            }
        }
        if chars.peek() == Some(&'}') {
            chars.next();
            break;
        }
    }
    let obj = HeapObj::Object(ObjectData {
        props: Mutex::new(props),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj))))
}
fn parse_json_arr(
    vm: &mut Vm,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    depth: usize,
) -> error::Result<Value> {
    let mut items = Vec::new();
    loop {
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }
        if chars.peek() == Some(&']') {
            chars.next();
            break;
        }
        items.push(parse_json_value(vm, chars, depth + 1)?);
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == ',' {
                chars.next();
            } else {
                break;
            }
        }
        if chars.peek() == Some(&']') {
            chars.next();
            break;
        }
    }
    let obj = HeapObj::Array(ArrayData {
        items: Mutex::new(items),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
        sparse_max: Mutex::new(None),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj))))
}
fn parse_json_str(chars: &mut std::iter::Peekable<std::str::Chars>) -> error::Result<Value> {
    let mut s = String::new();
    while let Some(c) = chars.next() {
        if c == '"' {
            break;
        }
        if c == '\\' {
            match chars.next() {
                Some('n') => s.push('\n'),
                Some('t') => s.push('\t'),
                Some('"') => s.push('"'),
                Some('\\') => s.push('\\'),
                Some(c) => s.push(c),
                None => break,
            }
        } else {
            s.push(c);
        }
    }
    Ok(Value::String(Arc::from(s.as_str())))
}
fn parse_json_num(chars: &mut std::iter::Peekable<std::str::Chars>) -> error::Result<Value> {
    let mut s = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E' {
            s.push(c);
            chars.next();
        } else {
            break;
        }
    }
    Ok(Value::Number(s.parse().unwrap_or(f64::NAN)))
}
fn now_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}
pub(crate) fn date_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let ts = if args.is_empty() {
        now_ms()
    } else if args.len() == 1 {
        vm.to_number(args.first().unwrap_or(&Value::Undefined))?
    } else {
        // Approximate: use the first numeric arg as a timestamp.
        vm.to_number(args.first().unwrap_or(&Value::Undefined))?
    };
    // ES TimeValue: a finite time value must be within +/-8.64e15 ms of the
    // epoch; anything else is an Invalid Date (NaN). Without this, `new
    // Date(1e20).getTime()` returned the raw number instead of NaN.
    const MAX_TIME_VALUE: f64 = 8.64e15;
    let ts = if ts.is_nan() || ts.is_infinite() || ts.abs() > MAX_TIME_VALUE {
        f64::NAN
    } else {
        ts
    };
    if let Some(Value::Object(idx)) = &this {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Object(o) = o {
                o.props
                    .lock()
                    .insert(PropertyKey::from("__time__"), data_prop(Value::Number(ts)));
            }
        });
        Ok(this.unwrap())
    } else {
        Ok(Value::String(Arc::from(format!("{}", ts as i64).as_str())))
    }
}
pub(crate) fn date_get_time(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = &this {
        let ts = vm.heap.with_obj(idx.0, |o| {
            o.props()
                .lock()
                .get(&PropertyKey::from("__time__"))
                .map(|d| d.value.clone())
        });
        if let Some(Value::Number(n)) = ts {
            return Ok(Value::Number(n));
        }
    }
    Ok(Value::Number(f64::NAN))
}
pub(crate) fn date_to_string(_vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = &this {
        let _ = idx;
    }
    Ok(Value::String(Arc::from("Date")))
}
pub(crate) fn date_now(_vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Number(now_ms()))
}

pub(crate) fn reflect_get(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let key = match args.get(1) {
        Some(v) => vm.to_property_key(v)?,
        None => return Ok(Value::Undefined),
    };
    vm.get_property(&target, &key)
}
pub(crate) fn reflect_set(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let key = match args.get(1) {
        Some(v) => vm.to_property_key(v)?,
        None => return Ok(Value::Bool(false)),
    };
    let value = args.get(2).cloned().unwrap_or(Value::Undefined);
    match vm.set_property(&target, &key, value) {
        Ok(()) => Ok(Value::Bool(true)),
        Err(_) => Ok(Value::Bool(false)),
    }
}
pub(crate) fn reflect_has(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let key = match args.get(1) {
        Some(v) => vm.to_property_key(v)?,
        None => return Ok(Value::Bool(false)),
    };
    let has = vm
        .get_property(&target, &key)
        .map(|v| !v.is_undefined())
        .unwrap_or(false);
    Ok(Value::Bool(has))
}
pub(crate) fn reflect_delete_property(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let key = match args.get(1) {
        Some(v) => vm.to_property_key(v)?,
        None => return Ok(Value::Bool(false)),
    };
    vm.delete_property(&target, &key)
        .map(|_| Value::Bool(true))
        .or(Ok(Value::Bool(false)))
}
pub(crate) fn reflect_own_keys(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let keys = own_string_keys(vm, &target);
    Ok(make_str_array(vm, keys))
}
fn reflect_get_prototype_of(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    object_get_prototype_of(vm, args, None)
}
fn reflect_set_prototype_of(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    object_set_prototype_of(vm, args, None)
}
fn reflect_is_extensible(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    object_is_extensible(vm, args, None)
}
fn reflect_prevent_extensions(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    object_prevent_extensions(vm, args, None)
}
fn reflect_apply(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);
    let args_arr = args.get(2).cloned().unwrap_or(Value::Undefined);
    let call_args = if let Value::Object(idx) = &args_arr {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Array(a) = o {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    vm.call_function(&target, &call_args, Some(this_arg))
}
fn reflect_construct(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let args_arr = args.get(1).cloned().unwrap_or(Value::Undefined);
    let call_args = if let Value::Object(idx) = &args_arr {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Array(a) = o {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    vm.construct(&target, &call_args)
}

pub(crate) fn build_reflect(vm: &mut Vm) -> Value {
    let mut props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    let entries: &[(&str, NativeFn, usize)] = &[
        ("get", reflect_get as NativeFn, 2),
        ("set", reflect_set as NativeFn, 3),
        ("has", reflect_has as NativeFn, 2),
        ("deleteProperty", reflect_delete_property as NativeFn, 2),
        ("ownKeys", reflect_own_keys as NativeFn, 1),
        ("getPrototypeOf", reflect_get_prototype_of as NativeFn, 1),
        ("setPrototypeOf", reflect_set_prototype_of as NativeFn, 2),
        ("isExtensible", reflect_is_extensible as NativeFn, 1),
        (
            "preventExtensions",
            reflect_prevent_extensions as NativeFn,
            1,
        ),
        ("apply", reflect_apply as NativeFn, 3),
        ("construct", reflect_construct as NativeFn, 2),
    ];
    for (name, f, len) in entries {
        let idx = vm.new_native_function(name, *f, *len);
        props.insert(PropertyKey::from(*name), data_prop(Value::Object(idx)));
    }
    let obj = HeapObj::Object(ObjectData {
        props: Mutex::new(props),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Reflect")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    Value::Object(GcIdx(vm.heap.allocate(obj)))
}

pub(crate) fn build_json(vm: &mut Vm) -> Value {
    let mut props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    let pi = vm.new_native_function("parse", json_parse, 1);
    let si = vm.new_native_function("stringify", json_stringify, 3);
    props.insert(PropertyKey::from("parse"), data_prop(Value::Object(pi)));
    props.insert(PropertyKey::from("stringify"), data_prop(Value::Object(si)));
    let obj = HeapObj::Object(ObjectData {
        props: Mutex::new(props),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("JSON")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    Value::Object(GcIdx(vm.heap.allocate(obj)))
}

