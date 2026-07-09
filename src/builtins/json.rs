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
        Value::Symbol(_) | Value::PrivateName(_) => None,
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
const MAKE_DAY_YEAR_LIMIT: f64 = 1_000_000_000.0;
const MAKE_DAY_MONTH_LIMIT: f64 = 12_000_000_000.0;
const MAKE_DAY_DATE_LIMIT: f64 = 1_000_000_000_000.0;
const DATE_WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const DATE_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn date_time_clip(ts: f64) -> f64 {
    if ts.is_nan() || ts.is_infinite() || ts.abs() > MAX_TIME_VALUE {
        f64::NAN
    } else if ts == 0.0 {
        0.0
    } else {
        let clipped = ts.trunc();
        if clipped == 0.0 {
            0.0
        } else {
            clipped
        }
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

fn date_days_from_civil_i128(year: i128, month_one_based: i128, day: i128) -> Option<i128> {
    let year = year.checked_sub(i128::from(month_one_based <= 2))?;
    let era = if year >= 0 {
        year
    } else {
        year.checked_sub(399)?
    } / 400;
    let yoe = year.checked_sub(era.checked_mul(400)?)?;
    let month = month_one_based.checked_add(if month_one_based > 2 { -3 } else { 9 })?;
    let doy = 153_i128
        .checked_mul(month)?
        .checked_add(2)?
        .checked_div(5)?
        .checked_add(day)?
        .checked_sub(1)?;
    let doe = yoe
        .checked_mul(365)?
        .checked_add(yoe / 4)?
        .checked_sub(yoe / 100)?
        .checked_add(doy)?;
    era.checked_mul(146097)?
        .checked_add(doe)?
        .checked_sub(719468)
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

fn date_limited_integer(value: f64, limit: f64) -> Option<i128> {
    let value = value.trunc();
    if value.abs() > limit {
        None
    } else {
        Some(value as i128)
    }
}

fn date_make_day(year: f64, month: f64, date: f64) -> f64 {
    if !year.is_finite() || !month.is_finite() || !date.is_finite() {
        return f64::NAN;
    }
    let Some(year) = date_limited_integer(year, MAKE_DAY_YEAR_LIMIT) else {
        return f64::NAN;
    };
    let Some(month) = date_limited_integer(month, MAKE_DAY_MONTH_LIMIT) else {
        return f64::NAN;
    };
    let Some(date) = date_limited_integer(date, MAKE_DAY_DATE_LIMIT) else {
        return f64::NAN;
    };
    let Some(year) = year.checked_add(month.div_euclid(12)) else {
        return f64::NAN;
    };
    if (year as f64).abs() > MAKE_DAY_YEAR_LIMIT {
        return f64::NAN;
    }
    let month_zero_based = month.rem_euclid(12);
    let Some(day) = date_days_from_civil_i128(year, month_zero_based + 1, 1)
        .and_then(|day| day.checked_add(date.checked_sub(1)?))
    else {
        return f64::NAN;
    };
    day as f64
}

fn date_make_day_with_year_offset(year: f64, month: f64, date: f64) -> f64 {
    let year = if year.is_finite() {
        let int_year = year.trunc();
        if (0.0..=99.0).contains(&int_year) {
            int_year + 1900.0
        } else {
            year
        }
    } else {
        year
    };
    date_make_day(year, month, date)
}

fn date_make_time(hour: f64, min: f64, sec: f64, ms: f64) -> f64 {
    if !hour.is_finite() || !min.is_finite() || !sec.is_finite() || !ms.is_finite() {
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

fn date_time_components(ts: f64) -> (f64, f64, f64, f64, f64) {
    let day = date_day(ts) as f64;
    let time = date_time_within_day(ts);
    let hour = (time / MS_PER_HOUR).floor();
    let minute = ((time % MS_PER_HOUR) / MS_PER_MINUTE).floor();
    let second = ((time % MS_PER_MINUTE) / MS_PER_SECOND).floor();
    let ms = time % MS_PER_SECOND;
    (day, hour, minute, second, ms)
}

fn date_components(ts: f64) -> (f64, f64, f64, f64, f64) {
    let day = date_day(ts);
    let (year, month_one_based, date) = date_civil_from_days(day);
    let time = date_time_within_day(ts);
    (
        year as f64,
        (month_one_based - 1) as f64,
        date as f64,
        day as f64,
        time,
    )
}

fn date_year_string(year: i64) -> String {
    if year < 0 {
        format!("-{:04}", year.saturating_abs())
    } else {
        format!("{year:04}")
    }
}

fn date_iso_year_string(year: i64) -> String {
    if (0..=9999).contains(&year) {
        format!("{year:04}")
    } else if year < 0 {
        format!("-{:06}", year.saturating_abs())
    } else {
        format!("+{year:06}")
    }
}

fn date_time_string(ts: f64) -> String {
    let (_, hour, minute, second, _) = date_time_components(ts);
    format!(
        "{:02}:{:02}:{:02}",
        hour as i64, minute as i64, second as i64
    )
}

fn date_date_string(ts: f64) -> String {
    let day = date_day(ts);
    let (year, month_one_based, date) = date_civil_from_days(day);
    let weekday = (day + 4).rem_euclid(7) as usize;
    let month = (month_one_based - 1) as usize;
    format!(
        "{} {} {:02} {}",
        DATE_WEEKDAYS[weekday],
        DATE_MONTHS[month],
        date,
        date_year_string(year)
    )
}

fn date_utc_string(ts: f64) -> String {
    let day = date_day(ts);
    let (year, month_one_based, date) = date_civil_from_days(day);
    let weekday = (day + 4).rem_euclid(7) as usize;
    let month = (month_one_based - 1) as usize;
    format!(
        "{}, {:02} {} {} {} GMT",
        DATE_WEEKDAYS[weekday],
        date,
        DATE_MONTHS[month],
        date_year_string(year),
        date_time_string(ts)
    )
}

fn date_date_time_string(ts: f64) -> String {
    format!("{} {} GMT+0000", date_date_string(ts), date_time_string(ts))
}

fn date_iso_string(ts: f64) -> String {
    let day = date_day(ts);
    let (year, month_one_based, date) = date_civil_from_days(day);
    let (_, hour, minute, second, ms) = date_time_components(ts);
    format!(
        "{}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        date_iso_year_string(year),
        month_one_based,
        date,
        hour as i64,
        minute as i64,
        second as i64,
        ms as i64
    )
}

fn date_parse_digits_i64(text: &str) -> Option<i64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse::<i64>().ok()
}

fn date_month_from_name(name: &str) -> Option<i64> {
    DATE_MONTHS
        .iter()
        .position(|month| *month == name)
        .map(|idx| idx as i64)
}

fn date_parse_hms(text: &str) -> Option<(i64, i64, i64)> {
    let mut parts = text.split(':');
    let hour = date_parse_digits_i64(parts.next()?)?;
    let minute = date_parse_digits_i64(parts.next()?)?;
    let second = date_parse_digits_i64(parts.next()?)?;
    if parts.next().is_some()
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
    {
        return None;
    }
    Some((hour, minute, second))
}

fn date_make_utc_time(
    year: i64,
    month_zero_based: i64,
    date: i64,
    hour: i64,
    minute: i64,
    second: i64,
    ms: i64,
) -> f64 {
    date_time_clip(date_make_date(
        date_make_day(year as f64, month_zero_based as f64, date as f64),
        date_make_time(hour as f64, minute as f64, second as f64, ms as f64),
    ))
}

fn date_parse_timezone_offset(text: &str) -> Option<i64> {
    if text == "Z" || text.is_empty() {
        return Some(0);
    }
    let sign = match text.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let rest = &text[1..];
    let (hour, minute) = if rest.len() == 5 && rest.as_bytes().get(2) == Some(&b':') {
        (
            date_parse_digits_i64(&rest[..2])?,
            date_parse_digits_i64(&rest[3..])?,
        )
    } else if rest.len() == 4 {
        (
            date_parse_digits_i64(&rest[..2])?,
            date_parse_digits_i64(&rest[2..])?,
        )
    } else {
        return None;
    };
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) {
        return None;
    }
    Some(sign * (hour * 60 + minute))
}

fn date_parse_iso_string(source: &str) -> Option<f64> {
    let s = source.trim();
    if s.is_empty() {
        return None;
    }
    let bytes = s.as_bytes();
    let (sign, year_start, year_len) = match bytes[0] {
        b'+' => (1_i64, 1, 6),
        b'-' => (-1_i64, 1, 6),
        b'0'..=b'9' => (1_i64, 0, 4),
        _ => return None,
    };
    if s.len() < year_start + year_len {
        return None;
    }
    let year_digits = date_parse_digits_i64(&s[year_start..year_start + year_len])?;
    if sign < 0 && year_digits == 0 {
        return None;
    }
    let year = sign * year_digits;
    let mut rest = &s[year_start + year_len..];
    if rest.is_empty() {
        return Some(date_make_utc_time(year, 0, 1, 0, 0, 0, 0));
    }
    if !rest.starts_with('-') || rest.len() < 3 {
        return None;
    }
    let month = date_parse_digits_i64(&rest[1..3])?;
    if !(1..=12).contains(&month) {
        return None;
    }
    rest = &rest[3..];
    if rest.is_empty() {
        return Some(date_make_utc_time(year, month - 1, 1, 0, 0, 0, 0));
    }
    if !rest.starts_with('-') || rest.len() < 3 {
        return None;
    }
    let date = date_parse_digits_i64(&rest[1..3])?;
    if !(1..=31).contains(&date) {
        return None;
    }
    rest = &rest[3..];
    if rest.is_empty() {
        return Some(date_make_utc_time(year, month - 1, date, 0, 0, 0, 0));
    }
    if !rest.starts_with('T') || rest.len() < 6 {
        return None;
    }
    let hour = date_parse_digits_i64(&rest[1..3])?;
    let minute = date_parse_digits_i64(&rest[4..6])?;
    if rest.as_bytes().get(3) != Some(&b':')
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
    {
        return None;
    }
    rest = &rest[6..];
    let mut second = 0;
    let mut ms = 0;
    if rest.starts_with(':') {
        if rest.len() < 3 {
            return None;
        }
        second = date_parse_digits_i64(&rest[1..3])?;
        if !(0..=59).contains(&second) {
            return None;
        }
        rest = &rest[3..];
    }
    if rest.starts_with('.') {
        let digits = rest[1..].bytes().take_while(|b| b.is_ascii_digit()).count();
        if digits == 0 {
            return None;
        }
        let raw = &rest[1..1 + digits.min(3)];
        ms = date_parse_digits_i64(raw)? * 10_i64.pow((3 - raw.len()) as u32);
        rest = &rest[1 + digits..];
    }
    let offset_minutes = date_parse_timezone_offset(rest)?;
    let time = date_make_utc_time(year, month - 1, date, hour, minute, second, ms);
    Some(date_time_clip(time - offset_minutes as f64 * MS_PER_MINUTE))
}

fn date_parse_legacy_string(source: &str) -> Option<f64> {
    let parts: Vec<&str> = source.split_whitespace().collect();
    if parts.len() >= 6 && parts[0].ends_with(',') {
        let date = date_parse_digits_i64(parts[1])?;
        let month = date_month_from_name(parts[2])?;
        let year = parts[3].parse::<i64>().ok()?;
        let (hour, minute, second) = date_parse_hms(parts[4])?;
        if parts[5] != "GMT" {
            return None;
        }
        return Some(date_make_utc_time(
            year, month, date, hour, minute, second, 0,
        ));
    }
    if parts.len() >= 6 {
        let month = date_month_from_name(parts[1])?;
        let date = date_parse_digits_i64(parts[2])?;
        let year = parts[3].parse::<i64>().ok()?;
        let (hour, minute, second) = date_parse_hms(parts[4])?;
        let offset = parts[5]
            .strip_prefix("GMT")
            .and_then(date_parse_timezone_offset)?;
        let time = date_make_utc_time(year, month, date, hour, minute, second, 0);
        return Some(date_time_clip(time - offset as f64 * MS_PER_MINUTE));
    }
    None
}

fn date_parse_string(source: &str) -> f64 {
    date_parse_iso_string(source)
        .or_else(|| date_parse_legacy_string(source))
        .unwrap_or(f64::NAN)
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
    if !matches!(this, Some(Value::Object(_))) {
        return Ok(Value::String(Arc::from(
            date_date_time_string(now_ms()).as_str(),
        )));
    }

    let ts = if args.is_empty() {
        now_ms()
    } else if args.len() == 1 {
        let value = args.first().unwrap_or(&Value::Undefined);
        if let Ok((_, ts)) = date_this_time_value(vm, Some(value.clone())) {
            ts
        } else {
            let value = vm.to_primitive(value)?;
            match value {
                Value::String(source) => date_parse_string(&source),
                _ => vm.to_number(&value)?,
            }
        }
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
            date_make_day_with_year_offset(year, month, date),
            date_make_time(hour, minute, second, ms),
        )
    };
    // ES TimeValue: values outside +/-8.64e15 ms become Invalid Date (NaN).
    let ts = date_time_clip(ts);
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj_mut(idx.0, |o| {
            if let HeapObj::Object(o) = o {
                o.class_name = Some(Arc::from("Date"));
                o.props
                    .lock()
                    .insert(PropertyKey::from("__time__"), data_prop(Value::Number(ts)));
            }
        });
        Ok(Value::Object(idx))
    } else {
        unreachable!("Date constructor function calls return before constructing a time value")
    }
}

fn date_this_time_value(vm: &Vm, this: Option<Value>) -> error::Result<(GcIdx, f64)> {
    let Some(Value::Object(idx)) = this else {
        return Err(Error::type_err("Date method called on non-Date receiver"));
    };
    let (is_date, ts) = vm.heap.with_obj(idx.0, |obj| {
        let ts = obj
            .props()
            .lock()
            .get(&PropertyKey::from("__time__"))
            .and_then(|d| match &d.value {
                Value::Number(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(f64::NAN);
        (obj.class_name() == "Date", ts)
    });
    if !is_date {
        return Err(Error::type_err("Date method called on non-Date receiver"));
    }
    Ok((idx, ts))
}

pub(crate) fn date_get_time(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, ts) = date_this_time_value(vm, this)?;
    Ok(Value::Number(ts))
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
    let (idx, ts) = date_this_time_value(vm, this)?;
    let name = active_native_name(vm).unwrap_or_else(|| Arc::from(""));
    let value = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => f64::NAN,
    };
    let value = match name.as_ref() {
        "setTime" => date_time_clip(value),
        "setMilliseconds" | "setUTCMilliseconds" => {
            date_set_time_component(vm, args, ts, value, 1)?
        }
        "setSeconds" | "setUTCSeconds" => date_set_time_component(vm, args, ts, value, 2)?,
        "setMinutes" | "setUTCMinutes" => date_set_time_component(vm, args, ts, value, 3)?,
        "setHours" | "setUTCHours" => date_set_time_component(vm, args, ts, value, 4)?,
        "setDate" | "setUTCDate" => date_set_date_component(vm, args, ts, value, 1)?,
        "setMonth" | "setUTCMonth" => date_set_date_component(vm, args, ts, value, 2)?,
        "setFullYear" | "setUTCFullYear" => date_set_date_component(vm, args, ts, value, 3)?,
        _ => value,
    };
    if value.is_nan()
        && !matches!(name.as_ref(), "setTime" | "setFullYear" | "setUTCFullYear")
        && ts.is_nan()
    {
        return Ok(Value::Number(f64::NAN));
    }
    vm.heap.with_obj(idx.0, |o| {
        if let HeapObj::Object(o) = o {
            o.props.lock().insert(
                PropertyKey::from("__time__"),
                data_prop(Value::Number(value)),
            );
        }
    });
    Ok(Value::Number(value))
}

fn date_set_time_component(
    vm: &mut Vm,
    args: &[Value],
    ts: f64,
    first: f64,
    arity: usize,
) -> error::Result<f64> {
    let (day, old_hour, old_minute, old_second, old_ms) = date_time_components(ts);
    let (hour, minute, second, ms) = match arity {
        1 => (old_hour, old_minute, old_second, first),
        2 => {
            let ms = match args.get(1) {
                Some(v) => vm.to_number(v)?,
                None => old_ms,
            };
            (old_hour, old_minute, first, ms)
        }
        3 => {
            let second = match args.get(1) {
                Some(v) => vm.to_number(v)?,
                None => old_second,
            };
            let ms = match args.get(2) {
                Some(v) => vm.to_number(v)?,
                None => old_ms,
            };
            (old_hour, first, second, ms)
        }
        4 => {
            let minute = match args.get(1) {
                Some(v) => vm.to_number(v)?,
                None => old_minute,
            };
            let second = match args.get(2) {
                Some(v) => vm.to_number(v)?,
                None => old_second,
            };
            let ms = match args.get(3) {
                Some(v) => vm.to_number(v)?,
                None => old_ms,
            };
            (first, minute, second, ms)
        }
        _ => unreachable!(),
    };
    if ts.is_nan() {
        return Ok(f64::NAN);
    }
    Ok(date_time_clip(date_make_date(
        day,
        date_make_time(hour, minute, second, ms),
    )))
}

fn date_set_date_component(
    vm: &mut Vm,
    args: &[Value],
    ts: f64,
    first: f64,
    arity: usize,
) -> error::Result<f64> {
    let base = if arity == 3 && ts.is_nan() { 0.0 } else { ts };
    let (old_year, old_month, old_date, _, time) = date_components(base);
    let (year, month, date) = match arity {
        1 => (old_year, old_month, first),
        2 => {
            let date = match args.get(1) {
                Some(v) => vm.to_number(v)?,
                None => old_date,
            };
            (old_year, first, date)
        }
        3 => {
            let month = match args.get(1) {
                Some(v) => vm.to_number(v)?,
                None => old_month,
            };
            let date = match args.get(2) {
                Some(v) => vm.to_number(v)?,
                None => old_date,
            };
            (first, month, date)
        }
        _ => unreachable!(),
    };
    if ts.is_nan() && arity != 3 {
        return Ok(f64::NAN);
    }
    Ok(date_time_clip(date_make_date(
        date_make_day(year, month, date),
        time,
    )))
}

pub(crate) fn date_to_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, ts) = date_this_time_value(vm, this)?;
    if ts.is_nan() {
        return Ok(Value::String(Arc::from("Invalid Date")));
    }
    let name = active_native_name(vm).unwrap_or_else(|| Arc::from(""));
    let result = match name.as_ref() {
        "toDateString" | "toLocaleDateString" => date_date_string(ts),
        "toTimeString" | "toLocaleTimeString" => {
            format!("{} GMT+0000", date_time_string(ts))
        }
        "toUTCString" => date_utc_string(ts),
        "toString" | "toLocaleString" => date_date_time_string(ts),
        _ => date_date_time_string(ts),
    };
    Ok(Value::String(Arc::from(result.as_str())))
}

pub(crate) fn date_to_iso_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, ts) = date_this_time_value(vm, this)?;
    if !ts.is_finite() {
        return Err(Error::range("Invalid time value"));
    }
    Ok(Value::String(Arc::from(date_iso_string(ts).as_str())))
}

pub(crate) fn date_to_json(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let value = this.unwrap_or(Value::Undefined);
    if value.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let object = vm.to_object(&value)?;
    let primitive = vm.to_primitive_number(&object)?;
    if let Value::Number(n) = primitive {
        if !n.is_finite() {
            return Ok(Value::Null);
        }
    }
    let to_iso = vm.get_property(&object, "toISOString")?;
    if !is_callable(&to_iso, &vm.heap) {
        return Err(Error::type_err("toISOString is not callable"));
    }
    vm.call_function(&to_iso, &[], Some(object))
}

pub(crate) fn date_to_temporal_instant(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, ts) = date_this_time_value(vm, this)?;
    if !ts.is_finite() {
        return Err(Error::range("Invalid time value"));
    }

    let epoch_nanoseconds = BigInt::from(ts as i64) * BigInt::from(1_000_000_i64);
    let mut props = IndexMap::new();
    props.insert(
        PropertyKey::from("epochNanoseconds"),
        data_prop(Value::BigInt(epoch_nanoseconds)),
    );
    let obj = HeapObj::Object(ObjectData {
        props: Mutex::new(props),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Temporal.Instant")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    Ok(Value::Object(vm.alloc(obj)?))
}

pub(crate) fn date_get_timezone_offset(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, ts) = date_this_time_value(vm, this)?;
    if ts.is_nan() {
        return Ok(Value::Number(f64::NAN));
    }
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
    Ok(Value::Number(date_parse_string(&source)))
}

pub(crate) fn date_utc(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let year = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => f64::NAN,
    };
    let month = match args.get(1) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let date = match args.get(2) {
        Some(v) => vm.to_number(v)?,
        None => 1.0,
    };
    let hour = match args.get(3) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let minute = match args.get(4) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let second = match args.get(5) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let ms = match args.get(6) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let time = date_make_date(
        date_make_day_with_year_offset(year, month, date),
        date_make_time(hour, minute, second, ms),
    );
    Ok(Value::Number(date_time_clip(time)))
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
        Value::String(s) => vm.try_set_property_with_receiver(&target, s, value, &receiver),
        Value::Symbol(_) => {
            if receiver == target {
                vm.set_property_key(&target, &key, value).map(|_| true)
            } else {
                vm.set_property_key(&receiver, &key, value).map(|_| true)
            }
        }
        _ => unreachable!("ToPropertyKey returns only String or Symbol"),
    };
    result.map(Value::Bool)
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
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.deleteProperty target must be an object",
        ));
    }
    let key = match args.get(1) {
        Some(v) => vm.to_property_key_value(v)?,
        None => return Ok(Value::Bool(false)),
    };
    let pkey = match key {
        Value::String(s) => PropertyKey::from_rc(s),
        Value::Symbol(id) => PropertyKey::Symbol(id),
        _ => unreachable!("ToPropertyKey returns only String or Symbol"),
    };
    vm.delete_property_key(&target, &pkey).map(Value::Bool)
}
fn reflect_define_property(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.defineProperty target must be an object",
        ));
    }
    object_define_property_result(vm, args, false).map(Value::Bool)
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
    let keys = own_property_keys_or_throw(vm, &target, false, true, true)?
        .iter()
        .map(property_key_to_value)
        .collect();
    make_value_array(vm, keys)
}
fn reflect_get_prototype_of(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    reflect_get_prototype_of_strict(vm, args)
}
fn reflect_set_prototype_of(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    reflect_set_prototype_of_result(vm, args).map(Value::Bool)
}
fn reflect_is_extensible(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.isExtensible target must be an object",
        ));
    }
    vm.is_extensible(&target).map(Value::Bool)
}
fn reflect_prevent_extensions(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.preventExtensions target must be an object",
        ));
    }
    vm.prevent_extensions(&target).map(Value::Bool)
}
fn reflect_apply(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);
    let args_arr = args.get(2).cloned().unwrap_or(Value::Undefined);
    if !is_callable(&target, &vm.heap) {
        return Err(Error::type_err("target is not callable"));
    }
    let call_args = reflect_create_list_from_array_like(vm, &args_arr)?;
    vm.call_function(&target, &call_args, Some(this_arg))
}

fn reflect_to_length(vm: &mut Vm, value: &Value) -> error::Result<usize> {
    const MAX_SAFE_LENGTH: f64 = 9_007_199_254_740_991.0;
    const MAX_MATERIALIZED_ARGS: usize = 1 << 20;

    let n = vm.to_number(value)?;
    if n.is_nan() || n <= 0.0 {
        return Ok(0);
    }
    if n.is_infinite() || n > MAX_MATERIALIZED_ARGS as f64 {
        return Err(Error::range("Reflect.construct argumentsList too large"));
    }
    Ok(n.trunc().min(MAX_SAFE_LENGTH) as usize)
}

fn reflect_create_list_from_array_like(vm: &mut Vm, value: &Value) -> error::Result<Vec<Value>> {
    if !matches!(value, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.construct argumentsList must be an object",
        ));
    }
    let length = vm.get_property(value, "length")?;
    let len = reflect_to_length(vm, &length)?;
    let mut list = Vec::with_capacity(len);
    for index in 0..len {
        list.push(vm.get_property(value, &index.to_string())?);
    }
    Ok(list)
}

fn reflect_construct(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let args_arr = args.get(1).cloned().unwrap_or(Value::Undefined);
    if !vm.is_constructor_value(&target) {
        return Err(Error::type_err("target is not a constructor"));
    }
    let new_target = if let Some(value) = args.get(2) {
        if !vm.is_constructor_value(value) {
            return Err(Error::type_err("newTarget is not a constructor"));
        }
        value.clone()
    } else {
        target.clone()
    };
    let call_args = reflect_create_list_from_array_like(vm, &args_arr)?;
    vm.construct_with_new_target(&target, &call_args, &new_target)
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
