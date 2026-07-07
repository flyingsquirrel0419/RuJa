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

fn json_ordered_string_keys(
    props: &IndexMap<PropertyKey, PropertyDescriptor>,
    enumerable_only: bool,
) -> Vec<String> {
    let mut keys: Vec<String> = props
        .iter()
        .filter_map(|(k, d)| {
            if enumerable_only && !d.enumerable {
                return None;
            }
            match k {
                PropertyKey::Str(s) => Some(s.to_string()),
                PropertyKey::Symbol(_) => None,
            }
        })
        .collect();
    keys.sort_by(
        |a, b| match (json_array_index(a.as_str()), json_array_index(b.as_str())) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        },
    );
    keys
}

pub(crate) fn json_stringify(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
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
        Value::Reference(_) => None,
        Value::Object(idx) => {
            // Check for toJSON method before any other processing.
            let to_json = vm.heap.with_obj(idx.0, |obj| {
                obj.props()
                    .lock()
                    .get(&PropertyKey::from("toJSON"))
                    .cloned()
            });
            if let Some(desc) = to_json {
                let to_json_val = desc.value.clone();
                let is_fn = vm.heap.with_obj(idx.0, |_obj| {
                    if let Value::Object(fidx) = &to_json_val {
                        vm.heap.with_obj(fidx.0, |o| o.is_function())
                    } else {
                        false
                    }
                });
                if is_fn {
                    let key_val = Value::String(Arc::from(""));
                    let result = vm.call_function(&to_json_val, &[], Some(v.clone()));
                    if let Ok(to_jsoned) = result {
                        let val = apply_replacer(vm, ctx, &key_val, &to_jsoned);
                        return stringify_value(vm, &val, seen, indent, ctx, depth);
                    }
                }
            }
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
                let keys: Vec<String> = if let Some(wl) = &ctx.whitelist {
                    props
                        .iter()
                        .filter_map(|(k, d)| {
                            let ks = match k {
                                crate::value::PropertyKey::Str(s) => s.to_string(),
                                _ => return None,
                            };
                            if wl.contains(&ks) && d.enumerable {
                                Some(ks)
                            } else {
                                None
                            }
                        })
                        .collect()
                } else {
                    json_ordered_string_keys(&props, true)
                };
                for key_str in keys {
                    let val = vm.get_property(v, &key_str).unwrap_or(Value::Undefined);
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
                    crate::value::ArrayData::new(new_items, Some(vm.array_proto.clone())),
                ))?))
            } else {
                let mut new_props = IndexMap::new();
                for key in json_ordered_string_keys(&props, false) {
                    let pk = PropertyKey::from(key.as_str());
                    if let Some(d) = props.get(&pk) {
                        let k = Value::String(Arc::from(key.as_str()));
                        let w = apply_reviver(vm, reviver, &k, &d.value, depth + 1)?;
                        if !w.is_undefined() {
                            let mut desc = data_prop(w);
                            desc.enumerable = true;
                            new_props.insert(pk, desc);
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
                ))?))
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
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj)?)))
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
    let obj = HeapObj::Array(ArrayData::new(items, Some(vm.array_proto.clone())));
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj)?)))
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

const MS_PER_SECOND: f64 = 1_000.0;
const MS_PER_MINUTE: f64 = 60.0 * MS_PER_SECOND;
const MS_PER_HOUR: f64 = 60.0 * MS_PER_MINUTE;
const MS_PER_DAY: f64 = 24.0 * MS_PER_HOUR;
const MAX_TIME_VALUE: f64 = 8.64e15;

fn date_time_clip(ts: f64) -> f64 {
    if ts.is_nan() || ts.is_infinite() || ts.abs() > MAX_TIME_VALUE {
        f64::NAN
    } else {
        ts
    }
}

fn date_days_from_civil(year: i64, month_one_based: i64, day: i64) -> i64 {
    let year = year - i64::from(month_one_based <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month_one_based + if month_one_based > 2 { -3 } else { 9 };
    let doy = (153 * month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn date_civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

fn date_make_day(year: f64, month: f64, date: f64) -> f64 {
    if year.is_nan() || month.is_nan() || date.is_nan() {
        return f64::NAN;
    }
    let mut year = year.trunc() as i64;
    if (0..=99).contains(&year) {
        year += 1900;
    }
    let month = month.trunc() as i64;
    let date = date.trunc() as i64;
    let year = year + month.div_euclid(12);
    let month_zero_based = month.rem_euclid(12);
    date_days_from_civil(year, month_zero_based + 1, 1) as f64 + (date - 1) as f64
}

fn date_make_time(hour: f64, min: f64, sec: f64, ms: f64) -> f64 {
    if hour.is_nan() || min.is_nan() || sec.is_nan() || ms.is_nan() {
        return f64::NAN;
    }
    hour.trunc() * MS_PER_HOUR
        + min.trunc() * MS_PER_MINUTE
        + sec.trunc() * MS_PER_SECOND
        + ms.trunc()
}

fn date_make_date(day: f64, time: f64) -> f64 {
    day * MS_PER_DAY + time
}

fn date_day(ts: f64) -> i64 {
    (ts / MS_PER_DAY).floor() as i64
}

fn date_time_within_day(ts: f64) -> f64 {
    ts.rem_euclid(MS_PER_DAY)
}

fn active_native_name(vm: &mut Vm) -> Option<Arc<str>> {
    let callee = vm.current_native_callee.clone()?;
    let Value::Object(idx) = callee else {
        return None;
    };
    vm.heap.with_obj(idx.0, |o| {
        if let HeapObj::Function(f) = o {
            f.name.clone()
        } else {
            None
        }
    })
}

pub(crate) fn date_constructor(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ts = if args.is_empty() {
        now_ms()
    } else if args.len() == 1 {
        vm.to_number(args.first().unwrap_or(&Value::Undefined))?
    } else {
        let year = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
        let month = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
        let date = match args.get(2) {
            Some(value) => vm.to_number(value)?,
            None => 1.0,
        };
        let hour = match args.get(3) {
            Some(value) => vm.to_number(value)?,
            None => 0.0,
        };
        let minute = match args.get(4) {
            Some(value) => vm.to_number(value)?,
            None => 0.0,
        };
        let second = match args.get(5) {
            Some(value) => vm.to_number(value)?,
            None => 0.0,
        };
        let ms = match args.get(6) {
            Some(value) => vm.to_number(value)?,
            None => 0.0,
        };
        date_make_date(
            date_make_day(year, month, date),
            date_make_time(hour, minute, second, ms),
        )
    };
    // ES TimeValue: values outside +/-8.64e15 ms become Invalid Date (NaN).
    let ts = date_time_clip(ts);
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
pub(crate) fn date_get_time(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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

pub(crate) fn date_get_component(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ts = match date_get_time(vm, &[], this)? {
        Value::Number(n) if n.is_finite() => n,
        _ => return Ok(Value::Number(f64::NAN)),
    };
    let name = active_native_name(vm).unwrap_or_else(|| Arc::from(""));
    let day = date_day(ts);
    let (year, month_one_based, date) = date_civil_from_days(day);
    let time = date_time_within_day(ts);
    let value = match name.as_ref() {
        "getFullYear" | "getUTCFullYear" => year as f64,
        "getMonth" | "getUTCMonth" => (month_one_based - 1) as f64,
        "getDate" | "getUTCDate" => date as f64,
        "getDay" | "getUTCDay" => (day + 4).rem_euclid(7) as f64,
        "getHours" | "getUTCHours" => (time / MS_PER_HOUR).floor(),
        "getMinutes" | "getUTCMinutes" => ((time % MS_PER_HOUR) / MS_PER_MINUTE).floor(),
        "getSeconds" | "getUTCSeconds" => ((time % MS_PER_MINUTE) / MS_PER_SECOND).floor(),
        "getMilliseconds" | "getUTCMilliseconds" => time % MS_PER_SECOND,
        _ => f64::NAN,
    };
    Ok(Value::Number(value))
}

pub(crate) fn date_set_component(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let value = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => f64::NAN,
    };
    if let Some(Value::Object(idx)) = &this {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Object(o) = o {
                o.props.lock().insert(
                    PropertyKey::from("__time__"),
                    data_prop(Value::Number(value)),
                );
            }
        });
    }
    Ok(Value::Number(value))
}

pub(crate) fn date_to_string(
    _vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = &this {
        let _ = idx;
    }
    Ok(Value::String(Arc::from("Date")))
}

pub(crate) fn date_get_timezone_offset(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::Number(0.0))
}

pub(crate) fn date_now(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::Number(now_ms()))
}

pub(crate) fn date_parse(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let source = match args.first() {
        Some(v) => vm.to_string(v)?,
        None => return Ok(Value::Number(f64::NAN)),
    };
    if let Ok(n) = source.trim().parse::<f64>() {
        return Ok(Value::Number(n));
    }
    Ok(Value::Number(f64::NAN))
}

pub(crate) fn date_utc(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let year = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => f64::NAN,
    };
    if year.is_nan() {
        return Ok(Value::Number(f64::NAN));
    }
    Ok(Value::Number(year))
}

pub(crate) fn reflect_get(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err("Reflect.get target must be an object"));
    }
    let key = match args.get(1) {
        Some(v) => vm.to_property_key_value(v)?,
        None => return Ok(Value::Undefined),
    };
    let receiver = args.get(2).cloned().unwrap_or_else(|| target.clone());
    match &key {
        Value::String(s) => vm.get_property_rx(&target, s, receiver, 0),
        Value::Symbol(_) => vm.get_property_key(&target, &key),
        _ => unreachable!("ToPropertyKey returns only String or Symbol"),
    }
}
pub(crate) fn reflect_set(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err("Reflect.set target must be an object"));
    }
    let key = match args.get(1) {
        Some(v) => vm.to_property_key_value(v)?,
        None => return Ok(Value::Bool(false)),
    };
    let value = args.get(2).cloned().unwrap_or(Value::Undefined);
    let receiver = args.get(3).cloned().unwrap_or_else(|| target.clone());
    let result = match &key {
        Value::String(s) => vm.set_property_with_receiver(&target, s, value, &receiver),
        Value::Symbol(_) => {
            if receiver == target {
                vm.set_property_key(&target, &key, value)
            } else {
                vm.set_property_key(&receiver, &key, value)
            }
        }
        _ => unreachable!("ToPropertyKey returns only String or Symbol"),
    };
    match result {
        Ok(()) => Ok(Value::Bool(true)),
        Err(_) => Ok(Value::Bool(false)),
    }
}
pub(crate) fn reflect_has(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err("Reflect.has target must be an object"));
    }
    let key = match args.get(1) {
        Some(v) => vm.to_property_key_value(v)?,
        None => return Ok(Value::Bool(false)),
    };
    let pkey = match key {
        Value::String(s) => PropertyKey::from(s.as_ref()),
        Value::Symbol(id) => PropertyKey::Symbol(id),
        _ => unreachable!("ToPropertyKey returns only String or Symbol"),
    };
    let has = vm.has_property_key(&target, &pkey)?;
    Ok(Value::Bool(has))
}
pub(crate) fn reflect_delete_property(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let key = match args.get(1) {
        Some(v) => vm.to_property_key(v)?,
        None => return Ok(Value::Bool(false)),
    };
    vm.delete_property(&target, &key)
        .map(|_| Value::Bool(true))
        .or(Ok(Value::Bool(false)))
}
fn reflect_define_property(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.defineProperty target must be an object",
        ));
    }
    object_define_property(vm, args, None).map(|_| Value::Bool(true))
}
fn reflect_get_own_property_descriptor(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.getOwnPropertyDescriptor target must be an object",
        ));
    }
    object_get_own_property_descriptor(vm, args, None)
}
pub(crate) fn reflect_own_keys(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err("Reflect.ownKeys target must be an object"));
    }
    let keys = own_property_keys(vm, &target, false, true, true)
        .iter()
        .map(property_key_to_value)
        .collect();
    make_value_array(vm, keys)
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

pub(crate) fn build_reflect(vm: &mut Vm) -> error::Result<Value> {
    let mut props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    let entries: &[(&str, NativeFn, usize)] = &[
        ("get", reflect_get as NativeFn, 2),
        ("set", reflect_set as NativeFn, 3),
        ("has", reflect_has as NativeFn, 2),
        ("deleteProperty", reflect_delete_property as NativeFn, 2),
        ("defineProperty", reflect_define_property as NativeFn, 3),
        (
            "getOwnPropertyDescriptor",
            reflect_get_own_property_descriptor as NativeFn,
            2,
        ),
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
        let idx = vm.new_native_function(name, *f, *len)?;
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
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj)?)))
}

pub(crate) fn build_json(vm: &mut Vm) -> error::Result<Value> {
    let mut props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    let pi = vm.new_native_function("parse", json_parse, 1)?;
    let si = vm.new_native_function("stringify", json_stringify, 3)?;
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
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj)?)))
}
