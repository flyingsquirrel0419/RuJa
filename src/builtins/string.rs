use super::*;

// =========================================================================
// String prototype + constructor
// =========================================================================
pub(crate) fn str_val(vm: &mut Vm, this: &Option<Value>) -> error::Result<String> {
    match this {
        Some(Value::String(s)) => Ok(s.to_string()),
        Some(Value::Object(idx)) => Ok(vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Object(o) = o {
                if let Some(cn) = &o.class_name {
                    cn.to_string()
                } else {
                    "[object Object]".into()
                }
            } else {
                "[object Object]".into()
            }
        })),
        Some(v) => Ok(vm.to_string(v)?.to_string()),
        None => Ok("undefined".into()),
    }
}
pub(crate) fn str_char_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    // ES: position is ToInteger; negatives and out-of-range yield "".
    // Rust's `as usize` saturates negatives to 0, so "abc".charAt(-1)
    // returned "a" instead of "".
    let pos = match args.first() {
        Some(Value::Number(n)) => *n,
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    if pos.is_nan() {
        return Ok(Value::String(Arc::from("")));
    }
    let i = pos.trunc() as i64;
    if i < 0 || (i as usize) >= crate::value::utf16_len(&s) {
        return Ok(Value::String(Arc::from("")));
    }
    match crate::value::utf16_get(&s, i as usize) {
        Some(unit) => Ok(Value::String(Arc::from(
            String::from_utf16_lossy(&[unit]).as_str(),
        ))),
        None => Ok(Value::String(Arc::from(""))),
    }
}
pub(crate) fn str_char_code_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    // ES: position is ToInteger(position); NaN -> 0, but a negative or
    // out-of-range index yields NaN. Rust's `as usize` saturates negatives
    // to 0, which made "abc".charCodeAt(-1) wrongly return 97.
    let pos = match args.first() {
        Some(Value::Number(n)) => *n,
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    if pos.is_nan() {
        return Ok(Value::Number(f64::NAN));
    }
    let i = pos.trunc() as i64;
    if i < 0 || (i as usize) >= crate::value::utf16_len(&s) {
        return Ok(Value::Number(f64::NAN));
    }
    Ok(crate::value::utf16_get(&s, i as usize)
        .map(|unit| Value::Number(unit as f64))
        .unwrap_or(Value::Number(f64::NAN)))
}
pub(crate) fn str_code_point_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let pos = match args.first() {
        Some(Value::Number(n)) => *n,
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    if pos.is_nan() {
        return Ok(Value::Undefined);
    }
    let i = pos.trunc() as i64;
    let len = crate::value::utf16_len(&s) as i64;
    if i < 0 || i >= len {
        return Ok(Value::Undefined);
    }
    let i = i as usize;
    let unit = crate::value::utf16_get(&s, i).unwrap_or(0) as u32;
    if (0xD800..=0xDBFF).contains(&unit) {
        // High surrogate; combine with next unit.
        if let Some(low) = crate::value::utf16_get(&s, i + 1) {
            let low = low as u32;
            if (0xDC00..=0xDFFF).contains(&low) {
                let cp = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                return Ok(Value::Number(cp as f64));
            }
        }
    }
    Ok(Value::Number(unit as f64))
}

pub(crate) fn str_concat(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let mut result = s.to_string();
    for a in args {
        result.push_str(&vm.to_string(a)?);
    }
    Ok(Value::String(Arc::from(result.as_str())))
}

pub(crate) fn str_search(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let pattern = args.first().cloned().unwrap_or(Value::Undefined);
    let p = vm.to_string(&pattern)?;
    Ok(crate::value::utf16_index_of(&s, &p, 0)
        .map(|i| Value::Number(i as f64))
        .unwrap_or(Value::Number(-1.0)))
}

pub(crate) fn string_raw(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    // String.raw(template, ...substitutions)
    let template = args.first().cloned().unwrap_or(Value::Undefined);
    let raw = vm.get_property(&template, "raw")?;
    let len_val = vm.get_property(&raw, "length")?;
    let len = vm.to_number(&len_val)? as usize;
    let mut result = String::new();
    let mut i = 0;
    while i < len {
        let seg = vm.get_property_key(&raw, &Value::Number(i as f64))?;
        result.push_str(&vm.to_string(&seg)?);
        if i + 1 < len {
            let sub = args.get(i + 1).cloned().unwrap_or(Value::Undefined);
            result.push_str(&vm.to_string(&sub)?);
        }
        i += 1;
    }
    Ok(Value::String(Arc::from(result.as_str())))
}

pub(crate) fn string_from_code_point(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let mut units: Vec<u16> = Vec::new();
    for a in args {
        let cp = vm.to_number(a)? as u32;
        if cp > 0x10FFFF {
            return Err(Error::range("Invalid code point"));
        }
        if cp <= 0xFFFF {
            units.push(cp as u16);
        } else {
            let cp = cp - 0x10000;
            units.push(0xD800 + ((cp >> 10) as u16));
            units.push(0xDC00 + ((cp & 0x3FF) as u16));
        }
    }
    Ok(Value::String(Arc::from(
        String::from_utf16_lossy(&units).as_str(),
    )))
}

pub(crate) fn str_index_of(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let n = args
        .first()
        .map(crate::value::value_to_debug_string)
        .unwrap_or_default();
    let len = crate::value::utf16_len(&s);
    let start = from_index_arg(vm, args, 1, len)?;
    Ok(crate::value::utf16_index_of(&s, &n, start)
        .map(|i| Value::Number(i as f64))
        .unwrap_or(Value::Number(-1.0)))
}
pub(crate) fn str_slice(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let len = crate::value::utf16_len(&s) as i64;
    let start = args
        .first()
        .and_then(|v| {
            if let Value::Number(n) = v {
                Some(*n as i64)
            } else {
                None
            }
        })
        .unwrap_or(0);
    let end = args
        .get(1)
        .and_then(|v| {
            if let Value::Number(n) = v {
                Some(*n as i64)
            } else {
                None
            }
        })
        .unwrap_or(len);
    let st = if start < 0 {
        (len + start).max(0) as usize
    } else {
        (start as usize).min(len as usize)
    };
    let en = if end < 0 {
        (len + end).max(0) as usize
    } else {
        (end as usize).min(len as usize)
    };
    let r = crate::value::utf16_slice(&s, st, en);
    Ok(Value::String(Arc::from(r.as_str())))
}
pub(crate) fn str_to_upper(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Arc::from(
        str_val(vm, &this)?.to_uppercase().as_str(),
    )))
}
pub(crate) fn str_to_lower(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Arc::from(
        str_val(vm, &this)?.to_lowercase().as_str(),
    )))
}
pub(crate) fn str_trim(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Arc::from(str_val(vm, &this)?.trim())))
}
pub(crate) fn str_split(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    // ES split limit: NaN -> 0 (empty result); a negative or non-finite
    // value is treated as unbounded (matching V8/Node, where -1 yields all
    // parts). `n as usize` saturated negatives to 0, wrongly producing [].
    let limit = match args.get(1) {
        Some(Value::Undefined) | None => usize::MAX,
        Some(v) => match vm.to_number(v) {
            Ok(n) if n.is_nan() => 0,
            Ok(n) if n < 0.0 || n.is_infinite() => usize::MAX,
            Ok(n) => n.trunc() as usize,
            Err(_) => usize::MAX,
        },
    };
    // If the separator is a RegExp, split on regex matches.
    if let Some(Value::Object(idx)) = args.first() {
        let (source, flags) = vm.heap.with_obj(idx.0, |o| {
            let p = o.props().lock();
            let src = p
                .get(&crate::value::PropertyKey::from("source"))
                .map(|d| d.value.clone());
            let flg = p
                .get(&crate::value::PropertyKey::from("flags"))
                .map(|d| d.value.clone());
            (src, flg)
        });
        if let (Some(Value::String(source)), flags_val) = (source, flags) {
            let flags_str = match flags_val {
                Some(Value::String(f)) => f.to_string(),
                _ => String::new(),
            };
            let re = compile_regex(&source, &flags_str)
                .map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
            let mut parts: Vec<String> = Vec::new();
            let mut last_end = 0;
            for m in re.find_iter(&s) {
                if parts.len() >= limit {
                    break;
                }
                parts.push(s[last_end..m.start()].to_string());
                last_end = m.end();
            }
            if parts.len() < limit {
                parts.push(s[last_end..].to_string());
            }
            let items: Vec<Value> = parts
                .into_iter()
                .map(|p| Value::String(Arc::from(p.as_str())))
                .collect();
            let arr = HeapObj::Array(ArrayData {
                items: Mutex::new(items),
                props: Mutex::new(IndexMap::new()),
                proto: Mutex::new(Some(vm.array_proto.clone())),
                sparse_max: Mutex::new(None),
            });
            return Ok(Value::Object(GcIdx(vm.heap.allocate(arr))));
        }
    }
    let sep = args.first().map(crate::value::value_to_debug_string);
    let parts: Vec<String> = match sep {
        None => vec![s],
        Some(sep) if sep.is_empty() => s.chars().take(limit).map(|c| c.to_string()).collect(),
        Some(sep) => s.split(&sep).take(limit).map(|p| p.to_string()).collect(),
    };
    let items: Vec<Value> = parts
        .into_iter()
        .map(|p| Value::String(Arc::from(p.as_str())))
        .collect();
    let arr = HeapObj::Array(ArrayData {
        items: Mutex::new(items),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
        sparse_max: Mutex::new(None),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}
pub(crate) fn str_replace(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let replacement = args.get(1).cloned().unwrap_or(Value::Undefined);
    // Is the replacement a function?
    let is_fn = if let Value::Object(idx) = &replacement {
        vm.heap.with_obj(idx.0, |o| o.is_function())
    } else {
        false
    };
    let to_str = if is_fn {
        String::new()
    } else {
        vm.to_string(&replacement)?.to_string()
    };
    // If the search value is a RegExp, use regex replacement.
    if let Some(Value::Object(idx)) = args.first() {
        let (source, flags) = vm.heap.with_obj(idx.0, |o| {
            let p = o.props().lock();
            let src = p
                .get(&crate::value::PropertyKey::from("source"))
                .map(|d| d.value.clone());
            let flg = p
                .get(&crate::value::PropertyKey::from("flags"))
                .map(|d| d.value.clone());
            (src, flg)
        });
        if let (Some(Value::String(source)), flags_val) = (source, flags) {
            let flags_str = match flags_val {
                Some(Value::String(f)) => f.to_string(),
                _ => String::new(),
            };
            let global = flags_str.contains('g');
            let re = compile_regex(&source, &flags_str)
                .map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
            if is_fn {
                let mut result = String::new();
                let mut last_end = 0;
                for caps in re.captures_iter(&s) {
                    let m = caps.get(0).unwrap();
                    result.push_str(&s[last_end..m.start()]);
                    let mut cap_args = vec![Value::String(Arc::from(m.as_str()))];
                    // capture groups (1-indexed)
                    for i in 1..caps.len() {
                        match caps.get(i) {
                            Some(g) => cap_args.push(Value::String(Arc::from(g.as_str()))),
                            None => cap_args.push(Value::Undefined),
                        }
                    }
                    cap_args.push(Value::Number(m.start() as f64));
                    cap_args.push(Value::String(Arc::from(s.as_str())));
                    let r = vm.call_function(&replacement, &cap_args, None)?;
                    result.push_str(vm.to_string(&r)?.as_ref());
                    last_end = m.end();
                    if !global {
                        break;
                    }
                }
                result.push_str(&s[last_end..]);
                return Ok(Value::String(Arc::from(result.as_str())));
            }
            let replaced = if global {
                re.replace_all(&s, to_str.as_str())
            } else {
                re.replace(&s, to_str.as_str())
            };
            return Ok(Value::String(Arc::from(replaced.as_ref())));
        }
    }
    let from = match args.first() {
        Some(v) => vm.to_string(v)?.to_string(),
        None => return Ok(Value::String(Arc::from(s.as_str()))),
    };
    if is_fn {
        if let Some(pos) = s.find(&from) {
            let cap_args = vec![
                Value::String(Arc::from(from.as_str())),
                Value::Number(pos as f64),
                Value::String(Arc::from(s.as_str())),
            ];
            let r = vm.call_function(&replacement, &cap_args, None)?;
            let r_str = vm.to_string(&r)?;
            let mut result = String::new();
            result.push_str(&s[..pos]);
            result.push_str(r_str.as_ref());
            result.push_str(&s[pos + from.len()..]);
            return Ok(Value::String(Arc::from(result.as_str())));
        }
        return Ok(Value::String(Arc::from(s.as_str())));
    }
    Ok(Value::String(Arc::from(
        s.replacen(&from, &to_str, 1).as_str(),
    )))
}
/// String.prototype.lastIndexOf(searchString, fromIndex): last occurrence at
/// or before `fromIndex` (default +Inf -> search from end).
pub(crate) fn str_last_index_of(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let n = args
        .first()
        .map(crate::value::value_to_debug_string)
        .unwrap_or_default();
    let len = crate::value::utf16_len(&s);
    let raw = match args.get(1) {
        Some(v) => vm.to_number(v)?,
        None => f64::INFINITY,
    };
    let end = if raw.is_nan() {
        len
    } else if raw.is_infinite() && raw < 0.0 {
        return Ok(Value::Number(-1.0));
    } else {
        let n_int = raw as i64;
        (if n_int < 0 { len as i64 + n_int } else { n_int }).max(0) as usize
    };
    Ok(crate::value::utf16_last_index_of(&s, &n, end)
        .map(|i| Value::Number(i as f64))
        .unwrap_or(Value::Number(-1.0)))
}

pub(crate) fn str_includes(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let n = args
        .first()
        .map(crate::value::value_to_debug_string)
        .unwrap_or_default();
    let len = s.chars().count();
    let start = from_index_arg(vm, args, 1, len)?;
    let mut byte_off = 0;
    for _ in 0..start {
        byte_off += s[byte_off..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0);
        if byte_off >= s.len() {
            break;
        }
    }
    Ok(Value::Bool(s[byte_off..].contains(n.as_str())))
}
pub(crate) fn str_starts_with(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(
        str_val(vm, &this)?.starts_with(
            args.first()
                .map(crate::value::value_to_debug_string)
                .unwrap_or_default()
                .as_str(),
        ),
    ))
}
pub(crate) fn str_ends_with(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(
        str_val(vm, &this)?.ends_with(
            args.first()
                .map(crate::value::value_to_debug_string)
                .unwrap_or_default()
                .as_str(),
        ),
    ))
}
pub(crate) fn str_repeat(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    // ES String.prototype.repeat: count must be a non-negative integer; a
    // negative, non-integer, Infinity, or too-large count throws RangeError.
    // Without this guard, `"x".repeat(Infinity)` panicked the engine with a
    // capacity overflow, and `"x".repeat(-1)` silently produced "" instead of
    // throwing. Cap the result length to keep untrusted code from OOM-allocating.
    let s = str_val(vm, &this)?;
    let count = match args.first() {
        Some(Value::Number(n)) => *n,
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    if count.is_nan() || count < 0.0 || count.is_infinite() {
        return Err(Error::range("Invalid count value"));
    }
    if count.fract() != 0.0 {
        return Err(Error::range("Invalid count value"));
    }
    const MAX_REPEAT_LEN: usize = 1 << 28; // 256 MiB
    let slen = crate::value::utf16_len(&s);
    if slen > 0 && (count as usize) > MAX_REPEAT_LEN / slen {
        return Err(Error::range("Invalid count value"));
    }
    Ok(Value::String(Arc::from(s.repeat(count as usize).as_str())))
}

pub(crate) fn str_match(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    match args.first() {
        Some(Value::Object(idx)) => {
            let (source, flags) = vm.heap.with_obj(idx.0, |o| {
                let p = o.props().lock();
                let src = p.get(&PropertyKey::from("source")).map(|d| d.value.clone());
                let flg = p
                    .get(&PropertyKey::from("flags"))
                    .map(|d| d.value.clone())
                    .unwrap_or(Value::Undefined);
                (src, flg)
            });
            if let Some(Value::String(source)) = source {
                let flags_str = match &flags {
                    Value::String(f) => f.to_string(),
                    _ => String::new(),
                };
                let re = compile_regex(&source, &flags_str)
                    .map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
                let global = flags_str.contains('g');
                if global {
                    // Collect all matches (full-match substrings).
                    let items: Vec<Value> = re
                        .find_iter(&s)
                        .map(|m| Value::String(Arc::from(m.as_str())))
                        .collect();
                    if items.is_empty() {
                        Ok(Value::Null)
                    } else {
                        Ok(make_value_array(vm, items))
                    }
                } else {
                    match re.captures(&s) {
                        Some(caps) => {
                            let items: Vec<Value> = caps
                                .iter()
                                .map(|c| match c {
                                    Some(m) => Value::String(Arc::from(m.as_str())),
                                    None => Value::Undefined,
                                })
                                .collect();
                            Ok(make_value_array(vm, items))
                        }
                        None => Ok(Value::Null),
                    }
                }
            } else {
                Ok(Value::Null)
            }
        }
        _ => Ok(Value::Null),
    }
}
pub(crate) fn array_find_last_index(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let fn_val = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        for (i, v) in items.iter().enumerate().rev() {
            let result = vm.call_function(
                &fn_val,
                &[v.clone(), Value::Number(i as f64), Value::Object(idx)],
                None,
            )?;
            if result.is_truthy() {
                return Ok(Value::Number(i as f64));
            }
        }
    }
    Ok(Value::Number(-1.0))
}

pub(crate) fn str_pad_start(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    // targetLength uses ToLength semantics: negatives clamp to 0, but a
    // non-finite or absurdly large length must throw RangeError (Node throws
    // "Invalid string length"). Without this guard, `"x".padStart(Infinity)`
    // hung the engine in an unbounded fill loop.
    let target = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    if target.is_nan() || target < 0.0 {
        return Ok(Value::String(Arc::from(s.as_str())));
    }
    if target.is_infinite() || target > (1u64 << 28) as f64 {
        return Err(Error::range("Invalid string length"));
    }
    let target = target as usize;
    let pad = match args.get(1) {
        Some(Value::String(p)) => p.to_string(),
        Some(v) if !v.is_undefined() => vm.to_string(v)?.to_string(),
        _ => " ".to_string(),
    };
    let cur_len = crate::value::utf16_len(&s);
    if pad.is_empty() || cur_len >= target {
        return Ok(Value::String(Arc::from(s.as_str())));
    }
    let need = target - cur_len;
    let pad_len = crate::value::utf16_len(&pad);
    if pad_len == 0 {
        return Ok(Value::String(Arc::from(s.as_str())));
    }
    let mut out = String::new();
    while crate::value::utf16_len(&out) < need {
        out.push_str(&pad);
    }
    // Truncate by code units.
    let mut units: Vec<u16> = out.encode_utf16().collect();
    units.truncate(need);
    out = String::from_utf16_lossy(&units);
    out.push_str(&s);
    Ok(Value::String(Arc::from(out.as_str())))
}
pub(crate) fn str_pad_end(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let target = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    if target.is_nan() || target < 0.0 {
        return Ok(Value::String(Arc::from(s.as_str())));
    }
    if target.is_infinite() || target > (1u64 << 28) as f64 {
        return Err(Error::range("Invalid string length"));
    }
    let target = target as usize;
    let pad = match args.get(1) {
        Some(Value::String(p)) => p.to_string(),
        Some(v) if !v.is_undefined() => vm.to_string(v)?.to_string(),
        _ => " ".to_string(),
    };
    let cur_len = crate::value::utf16_len(&s);
    if pad.is_empty() || cur_len >= target {
        return Ok(Value::String(Arc::from(s.as_str())));
    }
    let need = target - cur_len;
    let mut out = s.clone();
    while crate::value::utf16_len(&out) - cur_len < need {
        out.push_str(&pad);
    }
    let mut units: Vec<u16> = out.encode_utf16().collect();
    units.truncate(target);
    out = String::from_utf16_lossy(&units);
    Ok(Value::String(Arc::from(out.as_str())))
}
pub(crate) fn str_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let n = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    } as isize;
    let len = crate::value::utf16_len(&s) as isize;
    let idx = if n < 0 { len + n } else { n };
    if idx >= 0 && idx < len {
        // Return a 1-code-unit string (surrogate half for supplementary).
        let unit = crate::value::utf16_get(&s, idx as usize).unwrap();
        return Ok(Value::String(Arc::from(
            String::from_utf16_lossy(&[unit]).as_str(),
        )));
    }
    Ok(Value::Undefined)
}
pub(crate) fn str_trim_start(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    Ok(Value::String(Arc::from(s.trim_start())))
}
pub(crate) fn str_trim_end(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    Ok(Value::String(Arc::from(s.trim_end())))
}
pub(crate) fn str_replace_all(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let from = match args.first() {
        Some(Value::String(p)) => p.to_string(),
        Some(v) => vm.to_string(v)?.to_string(),
        None => return Ok(Value::String(Arc::from(s.as_str()))),
    };
    let to = match args.get(1) {
        Some(Value::String(p)) => p.to_string(),
        Some(v) => vm.to_string(v)?.to_string(),
        None => "undefined".to_string(),
    };
    if from.is_empty() {
        let mut out = String::new();
        for ch in s.chars() {
            out.push_str(&to);
            out.push(ch);
        }
        out.push_str(&to);
        return Ok(Value::String(Arc::from(out.as_str())));
    }
    Ok(Value::String(Arc::from(s.replace(&from, &to))))
}
pub(crate) fn str_substring(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let len = crate::value::utf16_len(&s) as f64;
    let mut start = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let mut end = match args.get(1) {
        Some(v) => vm.to_number(v)?,
        None => len,
    };
    if start < 0.0 || start.is_nan() {
        start = 0.0;
    }
    if end < 0.0 || end.is_nan() {
        end = 0.0;
    }
    if start > len {
        start = len;
    }
    if end > len {
        end = len;
    }
    if start > end {
        std::mem::swap(&mut start, &mut end);
    }
    let start = start as usize;
    let end = end as usize;
    let result = crate::value::utf16_slice(&s, start, end);
    Ok(Value::String(Arc::from(result.as_str())))
}

pub(crate) fn str_substr(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let len = crate::value::utf16_len(&s) as f64;
    let mut start = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let length = match args.get(1) {
        Some(v) => vm.to_number(v)?,
        None => f64::INFINITY,
    };
    // Negative start counts from the end (legacy behavior).
    if start < 0.0 {
        start = (len + start).max(0.0);
    }
    if start > len {
        start = len;
    }
    let end = if length.is_nan() || length < 0.0 {
        start
    } else {
        (start + length).min(len)
    };
    let start = start as usize;
    let end = end as usize;
    let result = crate::value::utf16_slice(&s, start, end);
    Ok(Value::String(Arc::from(result.as_str())))
}

pub(crate) fn str_from_char_code(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    // Build from UTF-16 code units. Unlike char::from_u32, this handles
    // surrogate pairs and lone surrogates correctly (each arg is one code
    // unit in [0, 65535] after ToUint16).
    let codes: Vec<u16> = args
        .iter()
        .filter_map(|v| {
            if let Value::Number(n) = v {
                Some((*n as u32) as u16)
            } else {
                None
            }
        })
        .collect();
    let s = crate::value::utf16_from_codes(&codes);
    Ok(Value::String(Arc::from(s.as_str())))
}
pub(crate) fn string_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(_)) = &this {
        let prim = match args.first() {
            None => Value::String(Arc::from("")),
            Some(v) => Value::String(vm.to_string(v)?),
        };
        vm.set_primitive(this.as_ref().unwrap(), prim);
        return Ok(this.unwrap());
    }
    // `String()` with no argument yields "" (per spec), distinct from
    // `String(undefined)` which yields "undefined".
    match args.first() {
        None => Ok(Value::String(Arc::from(""))),
        Some(v) => Ok(Value::String(vm.to_string(v)?)),
    }
}
pub(crate) fn number_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(_)) = &this {
        let prim = match args.first() {
            None => Value::Number(0.0),
            Some(v) => Value::Number(vm.to_number(v)?),
        };
        vm.set_primitive(this.as_ref().unwrap(), prim);
        return Ok(this.unwrap());
    }
    match args.first() {
        None => Ok(Value::Number(0.0)),
        Some(v) => Ok(Value::Number(vm.to_number(v)?)),
    }
}

pub(crate) fn number_is_integer(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    match args.first() {
        Some(Value::Number(n)) if n.is_finite() && n.fract() == 0.0 => Ok(Value::Bool(true)),
        _ => Ok(Value::Bool(false)),
    }
}
pub(crate) fn number_is_finite(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    match args.first() {
        Some(Value::Number(n)) if n.is_finite() => Ok(Value::Bool(true)),
        _ => Ok(Value::Bool(false)),
    }
}
pub(crate) fn number_is_nan(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    match args.first() {
        Some(Value::Number(n)) if n.is_nan() => Ok(Value::Bool(true)),
        _ => Ok(Value::Bool(false)),
    }
}
pub(crate) fn number_is_safe_integer(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    match args.first() {
        Some(Value::Number(n))
            if n.is_finite() && n.fract() == 0.0 && n.abs() <= 9007199254740991.0 =>
        {
            Ok(Value::Bool(true))
        }
        _ => Ok(Value::Bool(false)),
    }
}
pub(crate) fn number_parse_int(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    global_parse_int(vm, args, None)
}
pub(crate) fn number_parse_float(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    global_parse_float(vm, args, None)
}
pub(crate) fn num_to_fixed(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let n = match &this {
        Some(Value::Number(n)) => *n,
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    if n.is_nan() {
        return Ok(Value::String(Arc::from("NaN")));
    }
    if !n.is_finite() {
        return Ok(Value::String(Arc::from(if n > 0.0 {
            "Infinity"
        } else {
            "-Infinity"
        })));
    }
    // ES: fractionDigits must be an integer in 0..=100, else RangeError.
    // Without this, toFixed(-1) silently returned "1" and toFixed(200)
    // produced a 201-digit string, both diverging from V8/Node.
    let d = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    if d.is_nan() || d < 0.0 || d.fract() != 0.0 || d > 100.0 {
        return Err(Error::range(
            "toFixed() digits argument must be between 0 and 100",
        ));
    }
    let digits = d as usize;
    Ok(Value::String(Arc::from(format!("{:.*}", digits, n))))
}
pub(crate) fn num_to_precision(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let n = match &this {
        Some(Value::Number(n)) => *n,
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    if n.is_nan() {
        return Ok(Value::String(Arc::from("NaN")));
    }
    if !n.is_finite() {
        return Ok(Value::String(Arc::from(if n > 0.0 {
            "Infinity"
        } else {
            "-Infinity"
        })));
    }
    match args.first() {
        Some(v) if !v.is_undefined() => {
            // ES: precision must be an integer in 1..=100, else RangeError.
            let pf = vm.to_number(v)?;
            if pf.is_nan() || pf < 1.0 || pf.fract() != 0.0 || pf > 100.0 {
                return Err(Error::range(
                    "toPrecision() argument must be between 1 and 100",
                ));
            }
            let p = pf as usize;
            if p == 0 {
                return Ok(Value::String(Arc::from("0")));
            }
            // Use Rust's formatting with significant digits.
            let s = format!("{:.*e}", p - 1, n);
            // Convert exponential "1.23e4" to "12300" form for integer exp, else keep exp.
            let s = if let Some(pos) = s.find('e') {
                let mantissa = &s[..pos];
                let exp: i32 = s[pos + 1..].parse().unwrap_or(0);
                if exp >= 0 && exp < p as i32 {
                    // Convert to fixed notation.
                    let m = mantissa.replace('.', "");
                    let target_len = (exp + 1) as usize;
                    if m.len() >= target_len {
                        let mut result = m[..target_len].to_string();
                        if m.len() > target_len {
                            result.push('.');
                            result.push_str(&m[target_len..]);
                        }
                        result
                    } else {
                        let mut result = m.clone();
                        result.push_str(&"0".repeat(target_len - m.len()));
                        result
                    }
                } else {
                    let sign = if exp >= 0 { "+" } else { "" };
                    format!("{}e{}{}", mantissa, sign, exp)
                }
            } else {
                s
            };
            Ok(Value::String(Arc::from(s.as_str())))
        }
        _ => Ok(Value::String(Arc::from(
            crate::value::num_to_string(n).as_str(),
        ))),
    }
}

pub(crate) fn num_to_exponential(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let n = match &this {
        Some(Value::Number(n)) => *n,
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    if n.is_nan() {
        return Ok(Value::String(Arc::from("NaN")));
    }
    if !n.is_finite() {
        return Ok(Value::String(Arc::from(if n > 0.0 {
            "Infinity"
        } else {
            "-Infinity"
        })));
    }
    match args.first() {
        Some(v) if !v.is_undefined() => {
            let d = vm.to_number(v)? as usize;
            let s = format!("{:.*e}", d, n);
            // Rust uses e0, e1; JS uses e+0, e+1.
            let s = s.replace('e', "e+");
            Ok(Value::String(Arc::from(s.as_str())))
        }
        _ => {
            let s = format!("{:e}", n);
            let s = s.replace('e', "e+");
            Ok(Value::String(Arc::from(s.as_str())))
        }
    }
}

pub(crate) fn num_proto_to_string(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let n = match &this {
        Some(Value::Number(n)) => *n,
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let radix = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => 10.0,
    } as u32;
    if radix == 10 || radix == 0 {
        return Ok(Value::String(Arc::from(
            crate::value::num_to_string(n).as_str(),
        )));
    }
    if !(2..=36).contains(&radix) {
        return Err(Error::range(
            "toString() radix must be between 2 and 36".to_string(),
        ));
    }
    if n.fract() == 0.0 && n.abs() <= i64::MAX as f64 {
        let i = n as i64;
        let prefix = if i < 0 { "-" } else { "" };
        return Ok(Value::String(Arc::from(
            format!("{}{}", prefix, format_i64_radix(i.abs(), radix).as_str()).as_str(),
        )));
    }
    // Non-integer: convert integer and fractional parts in the given radix.
    // Without this, (1.5).toString(2) returned "1.5" instead of "1.1".
    Ok(Value::String(Arc::from(
        format_f64_radix(n, radix).as_str(),
    )))
}
pub(crate) fn format_i64_radix(n: i64, radix: u32) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut n = n.unsigned_abs();
    let mut out = Vec::new();
    while n > 0 {
        out.push(digits[(n % radix as u64) as usize]);
        n /= radix as u64;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// Convert an f64 to its exact rational value. Assumes finite input.
pub(crate) fn f64_to_exact_ratio(v: f64) -> Ratio<BigInt> {
    let bits = v.to_bits();
    let mant = bits & 0xfffffffffffff;
    let exp_biased = ((bits >> 52) & 0x7ff) as i32;
    let mant_int = if exp_biased == 0 {
        BigInt::from(mant)
    } else {
        BigInt::from((1u64 << 52) | mant)
    };
    let true_exp = if exp_biased == 0 {
        1 - 1023
    } else {
        exp_biased - 1023
    };
    let shift = 52 - true_exp;
    if shift >= 0 {
        Ratio::new(mant_int, BigInt::from(1u32) << (shift as u32))
    } else {
        Ratio::new(mant_int << ((-shift) as u32), BigInt::from(1))
    }
}

/// Half the distance between `vabs` and the next representable f64.
pub(crate) fn half_ulp(vabs: f64) -> Ratio<BigInt> {
    let bits = vabs.to_bits();
    let exp_biased = ((bits >> 52) & 0x7ff) as i32;
    if exp_biased == 0 {
        Ratio::new(BigInt::from(1), BigInt::from(1u32) << 1075u32)
    } else {
        let true_exp = exp_biased - 1023;
        let shift = true_exp - 53;
        if shift >= 0 {
            Ratio::new(BigInt::from(1u32) << (shift as u32), BigInt::from(1))
        } else {
            Ratio::new(BigInt::from(1), BigInt::from(1u32) << ((-shift) as u32))
        }
    }
}

/// Format a non-negative `BigUint` in the requested radix.
pub(crate) fn biguint_to_radix(mut n: BigUint, radix: u32) -> String {
    if n.is_zero() {
        return "0".to_string();
    }
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    let r = BigUint::from(radix);
    while n > BigUint::zero() {
        let (q, rem) = n.div_rem(&r);
        n = q;
        out.push(digits[*rem.to_u32_digits().first().unwrap_or(&0) as usize]);
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// Format an f64 in a non-decimal radix (2..=36) with the shortest
/// round-trip-precise representation. Mirrors ES Number.prototype.toString(radix).
pub(crate) fn format_f64_radix(n: f64, radix: u32) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n.is_infinite() {
        return if n < 0.0 {
            "-Infinity".to_string()
        } else {
            "Infinity".to_string()
        };
    }
    if n == 0.0 {
        return "0".to_string();
    }
    let neg = n < 0.0;
    let vabs = n.abs();
    let exact = f64_to_exact_ratio(vabs);
    let int_part = exact.floor().to_integer();

    let mut residual = &exact - Ratio::from_integer(int_part.clone());
    if residual.is_zero() {
        let s = biguint_to_radix(int_part.abs().to_biguint().unwrap(), radix);
        return if neg { format!("-{}", s) } else { s };
    }

    let base = BigInt::from(radix);
    let mut pow = BigInt::from(1);
    let mut m = BigInt::from(0);
    let half = half_ulp(vabs);

    for k in 1..=4096usize {
        residual *= Ratio::from_integer(base.clone());
        let d: BigInt = residual.floor().to_integer();
        residual -= Ratio::from_integer(d.clone());
        m = &m * &base + &d;
        pow *= &base;

        let candidate_down = Ratio::new(&int_part * &pow + &m, pow.clone());
        let up_numer: BigInt = &int_part * &pow + &m + 1;
        let candidate_up = Ratio::new(up_numer, pow.clone());

        let diff_down = (&candidate_down - &exact).abs();
        let diff_up = (&candidate_up - &exact).abs();

        let ok_down =
            diff_down < half || (diff_down == half && candidate_down.to_f64() == Some(vabs));
        let ok_up = diff_up < half || (diff_up == half && candidate_up.to_f64() == Some(vabs));

        if ok_down || ok_up {
            let m_final = if ok_down && ok_up {
                if diff_up < diff_down {
                    m.clone() + 1
                } else if diff_down < diff_up {
                    m.clone()
                } else {
                    // Tie on a representable boundary: choose the value whose
                    // last digit is even in the target radix.
                    let down_digit: i32 = (&m % &base).to_i32().unwrap_or(0);
                    if down_digit % 2 == 0 {
                        m.clone()
                    } else {
                        m.clone() + 1
                    }
                }
            } else if ok_up {
                m.clone() + 1
            } else {
                m.clone()
            };

            if m_final == pow {
                let next_int: BigInt = &int_part + 1;
                let s = biguint_to_radix(next_int.abs().to_biguint().unwrap(), radix);
                return if neg { format!("-{}", s) } else { s };
            }

            // Trim trailing zeros to obtain the shortest representation.
            let mut trimmed_m = m_final.clone();
            let mut trimmed_pow = pow.clone();
            let mut trimmed_k = k;
            while (&trimmed_m % &base).is_zero() && trimmed_k > 0 {
                trimmed_m /= &base;
                trimmed_pow /= &base;
                trimmed_k -= 1;
            }

            let total = &int_part * &trimmed_pow + &trimmed_m;
            let (int_q, frac_r) = total.div_rem(&trimmed_pow);
            let mut int_s = biguint_to_radix(int_q.abs().to_biguint().unwrap(), radix);
            if neg {
                int_s.insert(0, '-');
            }
            if frac_r.is_zero() {
                return int_s;
            }
            let frac_s = biguint_to_radix(frac_r.abs().to_biguint().unwrap(), radix);
            let frac_padded = format!("{:0>width$}", frac_s, width = trimmed_k);
            return format!("{}.{}", int_s, frac_padded);
        }
    }

    // Fallback (should rarely happen): fall back to a fixed number of digits.
    format!("{}", n)
}

pub(crate) fn boolean_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(_)) = &this {
        let prim = Value::Bool(args.first().unwrap_or(&Value::Undefined).is_truthy());
        vm.set_primitive(this.as_ref().unwrap(), prim);
        return Ok(this.unwrap());
    }
    Ok(Value::Bool(
        args.first().unwrap_or(&Value::Undefined).is_truthy(),
    ))
}

