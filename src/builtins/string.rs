use super::*;
use unicode_normalization::UnicodeNormalization;

// =========================================================================
// String prototype + constructor
// =========================================================================
pub(crate) fn str_val(vm: &mut Vm, this: &Option<Value>) -> error::Result<String> {
    match this {
        None | Some(Value::Undefined) | Some(Value::Null) => Err(Error::type_err(
            "String.prototype method called on null or undefined",
        )),
        Some(Value::String(s)) => Ok(s.to_string()),
        Some(Value::Object(idx)) => {
            let boxed = vm.heap.with_obj(idx.0, |o| match o {
                HeapObj::Object(o) => o.primitive.lock().clone(),
                _ => None,
            });
            if let Some(Value::String(s)) = boxed {
                Ok(s.to_string())
            } else {
                vm.to_string(&Value::Object(*idx)).map(|s| s.to_string())
            }
        }
        Some(v) => Ok(vm.to_string(v)?.to_string()),
    }
}

fn to_integer_or_zero(vm: &mut Vm, value: &Value) -> error::Result<f64> {
    let n = vm.to_number(value)?;
    if n.is_nan() || n == 0.0 {
        Ok(0.0)
    } else if n.is_infinite() {
        Ok(n)
    } else {
        Ok(n.trunc())
    }
}

fn split_limit(vm: &mut Vm, value: Option<&Value>) -> error::Result<usize> {
    match value {
        None | Some(Value::Undefined) => Ok(u32::MAX as usize),
        Some(value) => Ok(crate::vm::to_uint32(vm.to_number(value)?) as usize),
    }
}

fn string_array_from_parts(vm: &mut Vm, parts: Vec<String>) -> error::Result<Value> {
    let array = array_create_in_current_realm(vm, 0)?;
    let array_pin = vm.pin(&array);
    let result = (|| {
        for (index, part) in parts.into_iter().enumerate() {
            vm.consume_fuel()?;
            vm.define_data_property(
                &array,
                PropertyKey::from_integer_index(index as u64),
                Value::String(Arc::from(part.as_str())),
            )?;
        }
        Ok(array)
    })();
    vm.unpin_many(array_pin);
    result
}

fn split_string_parts(
    vm: &mut Vm,
    input: &str,
    separator: &str,
    limit: usize,
) -> error::Result<Vec<String>> {
    // StringIndexOf is defined over UTF-16 code units, not Rust Unicode scalar
    // boundaries; this also preserves lone-surrogate separators and results.
    let input = crate::value::utf16_from_str(input);
    let separator = crate::value::utf16_from_str(separator);
    if separator.is_empty() {
        let mut parts = Vec::with_capacity(input.len().min(limit));
        for unit in input.into_iter().take(limit) {
            vm.consume_fuel()?;
            parts.push(crate::value::utf16_to_string(&[unit]));
        }
        return Ok(parts);
    }

    let mut parts = Vec::new();
    if separator.len() > input.len() {
        parts.push(crate::value::utf16_to_string(&input));
        return Ok(parts);
    }
    let mut part_start = 0usize;
    let mut search_index = 0usize;
    let last_search_index = input.len() - separator.len();
    while search_index <= last_search_index {
        vm.consume_fuel()?;
        if input[search_index..search_index + separator.len()] == separator {
            parts.push(crate::value::utf16_to_string(
                &input[part_start..search_index],
            ));
            if parts.len() == limit {
                return Ok(parts);
            }
            part_start = search_index + separator.len();
            search_index = part_start;
        } else {
            search_index += 1;
        }
    }
    parts.push(crate::value::utf16_to_string(&input[part_start..]));
    Ok(parts)
}

fn string_search_position(
    vm: &mut Vm,
    args: &[Value],
    len: usize,
    default: f64,
) -> error::Result<usize> {
    let pos = match args.get(1) {
        Some(Value::Undefined) | None => default,
        Some(value) => to_integer_or_zero(vm, value)?,
    };
    if pos.is_nan() || pos <= 0.0 {
        Ok(0)
    } else if pos.is_infinite() {
        Ok(len)
    } else {
        Ok((pos as usize).min(len))
    }
}

fn string_search_arg(vm: &mut Vm, value: &Value) -> error::Result<String> {
    if is_regexp(vm, value)? {
        return Err(Error::type_err("First argument must not be a RegExp"));
    }
    Ok(vm.to_string(value)?.to_string())
}

fn is_js_trim_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\u{0009}'
            | '\u{000A}'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{000D}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    ) || matches!(ch, '\u{2000}'..='\u{200A}')
}

fn is_regexp(vm: &mut Vm, value: &Value) -> error::Result<bool> {
    is_regexp_spec(vm, value)
}

fn is_well_formed_utf16(units: &[u16]) -> bool {
    let mut i = 0;
    while i < units.len() {
        let unit = units[i];
        if (0xD800..=0xDBFF).contains(&unit) {
            if i + 1 < units.len() && (0xDC00..=0xDFFF).contains(&units[i + 1]) {
                i += 2;
                continue;
            }
            return false;
        }
        if (0xDC00..=0xDFFF).contains(&unit) {
            return false;
        }
        i += 1;
    }
    true
}

fn to_well_formed_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        let Some(unit) = crate::value::utf16_single_unit_from_internal_char(ch) else {
            out.push(ch);
            continue;
        };
        if (0xD800..=0xDBFF).contains(&unit) {
            if let Some(next) = chars.peek().copied() {
                if let Some(low) = crate::value::utf16_single_unit_from_internal_char(next) {
                    if (0xDC00..=0xDFFF).contains(&low) {
                        out.push(ch);
                        out.push(chars.next().unwrap());
                        continue;
                    }
                }
            }
            out.push('\u{FFFD}');
        } else if (0xDC00..=0xDFFF).contains(&unit) {
            out.push('\u{FFFD}');
        } else {
            out.push(ch);
        }
    }
    out
}

pub(crate) fn str_char_at(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let pos = match args.first() {
        Some(v) => to_integer_or_zero(vm, v)?,
        None => 0.0,
    };
    let i = pos as i64;
    if i < 0 || (i as usize) >= crate::value::utf16_len(&s) {
        return Ok(Value::String(Arc::from("")));
    }
    match crate::value::utf16_get(&s, i as usize) {
        Some(unit) => Ok(Value::String(Arc::from(
            crate::value::utf16_to_string(&[unit]).as_str(),
        ))),
        None => Ok(Value::String(Arc::from(""))),
    }
}
pub(crate) fn str_char_code_at(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let pos = match args.first() {
        Some(v) => to_integer_or_zero(vm, v)?,
        None => 0.0,
    };
    let i = pos as i64;
    if i < 0 || (i as usize) >= crate::value::utf16_len(&s) {
        return Ok(Value::Number(f64::NAN));
    }
    Ok(crate::value::utf16_get(&s, i as usize)
        .map(|unit| Value::Number(unit as f64))
        .unwrap_or(Value::Number(f64::NAN)))
}
pub(crate) fn str_code_point_at(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let pos = match args.first() {
        Some(v) => to_integer_or_zero(vm, v)?,
        None => 0.0,
    };
    let i = pos as i64;
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

pub(crate) fn str_is_well_formed(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let units = crate::value::utf16_from_str(&s);
    Ok(Value::Bool(is_well_formed_utf16(&units)))
}

pub(crate) fn str_to_well_formed(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let units = crate::value::utf16_from_str(&s);
    if is_well_formed_utf16(&units) {
        return Ok(Value::String(Arc::from(s.as_str())));
    }
    let formed = to_well_formed_string(&s);
    Ok(Value::String(Arc::from(formed.as_str())))
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
    let receiver = this.clone().unwrap_or(Value::Undefined);
    if receiver.is_nullish() {
        return Err(Error::type_err(
            "String.prototype method called on null or undefined",
        ));
    }
    let search_value = args.first().cloned().unwrap_or(Value::Undefined);
    let search_key = PropertyKey::symbol(vm.well_known_symbols.search);
    if matches!(search_value, Value::Object(_)) {
        let searcher = vm.get_property_by_key(&search_value, &search_key)?;
        if !searcher.is_nullish() {
            let is_callable = matches!(&searcher, Value::Object(idx) if {
                vm.heap.with_obj(idx.0, |o| o.is_function())
            });
            if !is_callable {
                return Err(Error::type_err("Symbol.search method is not callable"));
            }
            return vm.call_function(
                &searcher,
                std::slice::from_ref(&receiver),
                Some(search_value),
            );
        }
    }
    let s = str_val(vm, &Some(receiver))?;
    let regexp = regexp_create_intrinsic(vm, &search_value)?;
    let searcher = vm.get_property_by_key(&regexp, &search_key)?;
    if !searcher.is_nullish() {
        let is_callable = matches!(&searcher, Value::Object(idx) if {
            vm.heap.with_obj(idx.0, |o| o.is_function())
        });
        if !is_callable {
            return Err(Error::type_err("Symbol.search method is not callable"));
        }
        return vm.call_function(
            &searcher,
            &[Value::String(Arc::from(s.as_str()))],
            Some(regexp),
        );
    }
    regexp_search_internal(vm, regexp, &s)
}

fn regexp_search_internal(vm: &mut Vm, regexp: Value, s: &str) -> error::Result<Value> {
    let regexp = Some(regexp);
    let source = read_regexp_source_arc(vm, &regexp)?;
    let flags = read_regexp_flags_arc(vm, &regexp)?;
    let re = compile_regex_for_input_cached(vm, source.clone(), &flags, s)
        .map_err(regexp_compile_error)?;
    meter_logical_regex_input(vm, &re, s)?;
    let matched = if flags.contains('y') {
        re.find_at(s, 0)?.filter(|m| m.start() == 0)
    } else {
        re.find(s)?
    };
    Ok(matched
        .map(|m| Value::Number(crate::value::utf16_len(&s[..m.start()]) as f64))
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
            if let Some(sub) = args.get(i + 1) {
                result.push_str(&vm.to_string(sub)?);
            }
        }
        i += 1;
    }
    Ok(Value::String(Arc::from(result.as_str())))
}

pub(crate) fn string_from_code_point(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let mut units: Vec<u16> = Vec::new();
    for a in args {
        let cp = vm.to_number(a)?;
        if !cp.is_finite() || cp.fract() != 0.0 || !(0.0..=0x10FFFF as f64).contains(&cp) {
            return Err(Error::range("Invalid code point"));
        }
        let cp = cp as u32;
        if cp <= 0xFFFF {
            units.push(cp as u16);
        } else {
            let cp = cp - 0x10000;
            units.push(0xD800 + ((cp >> 10) as u16));
            units.push(0xDC00 + ((cp & 0x3FF) as u16));
        }
    }
    Ok(Value::String(Arc::from(
        crate::value::utf16_to_string(&units).as_str(),
    )))
}

pub(crate) fn str_index_of(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let n = vm
        .to_string(args.first().unwrap_or(&Value::Undefined))?
        .to_string();
    let len = crate::value::utf16_len(&s);
    let start = string_search_position(vm, args, len, 0.0)?;
    Ok(crate::value::utf16_index_of(&s, &n, start)
        .map(|i| Value::Number(i as f64))
        .unwrap_or(Value::Number(-1.0)))
}
pub(crate) fn str_slice(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let len = crate::value::utf16_len(&s);
    let start = match args.first() {
        Some(value) => to_integer_or_zero(vm, value)?,
        None => 0.0,
    };
    let end = match args.get(1) {
        Some(Value::Undefined) | None => len as f64,
        Some(value) => to_integer_or_zero(vm, value)?,
    };
    let st = if start == f64::NEG_INFINITY {
        0
    } else if start < 0.0 {
        ((len as f64) + start).max(0.0) as usize
    } else if start.is_infinite() {
        len
    } else {
        (start as usize).min(len)
    };
    let en = if end == f64::NEG_INFINITY {
        0
    } else if end < 0.0 {
        ((len as f64) + end).max(0.0) as usize
    } else if end.is_infinite() {
        len
    } else {
        (end as usize).min(len)
    };
    let r = crate::value::utf16_slice(&s, st, en);
    Ok(Value::String(Arc::from(r.as_str())))
}
pub(crate) fn str_to_upper(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::String(Arc::from(
        str_val(vm, &this)?.to_uppercase().as_str(),
    )))
}
pub(crate) fn str_to_lower(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::String(Arc::from(
        str_val(vm, &this)?.to_lowercase().as_str(),
    )))
}

pub(crate) fn str_locale_compare(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let left = str_val(vm, &this)?;
    let right = vm
        .to_string(args.first().unwrap_or(&Value::Undefined))?
        .to_string();
    let locales = args.get(1).cloned().unwrap_or(Value::Undefined);
    let options = args.get(2).cloned().unwrap_or(Value::Undefined);
    super::intl::compare_strings_with_collator(vm, &left, &right, locales, options)
}

pub(crate) fn str_trim(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    Ok(Value::String(Arc::from(
        s.trim_matches(is_js_trim_whitespace),
    )))
}
pub(crate) fn str_split(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let receiver = this.clone().unwrap_or(Value::Undefined);
    if receiver.is_nullish() {
        return Err(Error::type_err(
            "String.prototype method called on null or undefined",
        ));
    }
    let separator = args.first().cloned().unwrap_or(Value::Undefined);
    let limit_value = args.get(1).cloned().unwrap_or(Value::Undefined);
    if matches!(separator, Value::Object(_)) {
        let split_key = PropertyKey::symbol(vm.well_known_symbols.split);
        let splitter = vm.get_property_by_key(&separator, &split_key)?;
        if !splitter.is_nullish() {
            if !is_callable(&splitter, &vm.heap) {
                return Err(Error::type_err("Symbol.split method is not callable"));
            }
            return vm.call_function(&splitter, &[receiver.clone(), limit_value], Some(separator));
        }
    }
    let s = str_val(vm, &Some(receiver))?;
    let limit = split_limit(vm, args.get(1))?;
    let sep = if separator.is_undefined() {
        None
    } else {
        Some(vm.to_string(&separator)?.to_string())
    };
    if limit == 0 {
        return string_array_from_parts(vm, Vec::new());
    }
    let parts: Vec<String> = match sep {
        None => vec![s],
        Some(sep) => split_string_parts(vm, &s, &sep, limit)?,
    };
    string_array_from_parts(vm, parts)
}
pub(crate) fn str_replace(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.clone().unwrap_or(Value::Undefined);
    if receiver.is_nullish() {
        return Err(Error::type_err(
            "String.prototype method called on null or undefined",
        ));
    }
    let replacement = args.get(1).cloned().unwrap_or(Value::Undefined);
    if let Some(search_value) = args
        .first()
        .filter(|value| matches!(value, Value::Object(_)))
        .cloned()
    {
        let replace_key = PropertyKey::symbol(vm.well_known_symbols.replace);
        let replacer = vm.get_property_by_key(&search_value, &replace_key)?;
        if !replacer.is_nullish() {
            let is_callable = matches!(&replacer, Value::Object(idx) if {
                vm.heap.with_obj(idx.0, |o| o.is_function())
            });
            if !is_callable {
                return Err(Error::type_err("Symbol.replace method is not callable"));
            }
            return vm.call_function(
                &replacer,
                &[receiver.clone(), replacement],
                Some(search_value),
            );
        }
    }
    let s = str_val(vm, &Some(receiver))?;
    // Is the replacement a function?
    let is_fn = if let Value::Object(idx) = &replacement {
        vm.heap.with_obj(idx.0, |o| o.is_function())
    } else {
        false
    };
    // If the search value is a RegExp, use regex replacement.
    if let Some(Value::Object(idx)) = args.first() {
        let is_regexp_obj = vm.heap.with_obj(
            idx.0,
            |o| matches!(o, HeapObj::Object(od) if od.class_name.as_deref() == Some("RegExp")),
        );
        if is_regexp_obj {
            let regexp = Some(Value::Object(*idx));
            let source = read_regexp_source_arc(vm, &regexp)?;
            let flags_str = read_regexp_flags_arc(vm, &regexp)?;
            let global = flags_str.contains('g');
            let re = compile_regex_for_input_cached(vm, source.clone(), &flags_str, &s)
                .map_err(regexp_compile_error)?;
            meter_logical_regex_input(vm, &re, &s)?;
            let capture_names = regex_capture_names(&source, &flags_str).map_err(Error::syntax)?;
            if is_fn {
                let mut result = String::new();
                let mut last_end = 0;
                for caps in re.captures_iter(&s)? {
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
                    cap_args.push(Value::Number(
                        crate::value::utf16_len(&s[..m.start()]) as f64
                    ));
                    cap_args.push(Value::String(Arc::from(s.as_str())));
                    let groups = make_regexp_groups_object(vm, &caps, &capture_names)?;
                    if !groups.is_undefined() {
                        cap_args.push(groups);
                    }
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
            let to_str = vm.to_string(&replacement)?.to_string();
            let mut result = String::new();
            let mut last_end = 0;
            let mut replaced = false;
            for caps in re.captures_iter(&s)? {
                let Some(m) = caps.get(0) else {
                    continue;
                };
                result.push_str(&s[last_end..m.start()]);
                result.push_str(&regexp_replace_substitution(
                    &to_str,
                    &s,
                    &caps,
                    &capture_names,
                ));
                last_end = m.end();
                replaced = true;
                if !global {
                    break;
                }
            }
            if !replaced {
                return Ok(Value::String(Arc::from(s.as_str())));
            }
            result.push_str(&s[last_end..]);
            return Ok(Value::String(Arc::from(result.as_str())));
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
                Value::Number(crate::value::utf16_len(&s[..pos]) as f64),
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
    let to_str = vm.to_string(&replacement)?.to_string();
    if let Some(pos) = s.find(&from) {
        let mut result = String::new();
        result.push_str(&s[..pos]);
        result.push_str(&replace_substitution(
            &to_str,
            &s,
            pos,
            pos + from.len(),
            &from,
            &[],
            &[],
        ));
        result.push_str(&s[pos + from.len()..]);
        return Ok(Value::String(Arc::from(result.as_str())));
    }
    Ok(Value::String(Arc::from(s.as_str())))
}

fn regexp_replace_substitution(
    replacement: &str,
    input: &str,
    caps: &CompiledCaptures<'_>,
    capture_names: &[RegexCaptureName],
) -> String {
    let Some(matched) = caps.get(0) else {
        return replacement.to_string();
    };
    let captures: Vec<Option<&str>> = (1..caps.len())
        .map(|index| caps.get(index).map(|capture| capture.as_str()))
        .collect();
    replace_substitution(
        replacement,
        input,
        matched.start(),
        matched.end(),
        matched.as_str(),
        &captures,
        capture_names,
    )
}

pub(super) fn replace_substitution(
    replacement: &str,
    input: &str,
    matched_start: usize,
    matched_end: usize,
    matched: &str,
    captures: &[Option<&str>],
    capture_names: &[RegexCaptureName],
) -> String {
    let capture_count = captures.len();
    let mut result = String::new();
    let mut chars = replacement.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch != '$' {
            result.push(ch);
            continue;
        }
        let Some((next_index, next)) = chars.peek().copied() else {
            result.push('$');
            continue;
        };
        match next {
            '$' => {
                chars.next();
                result.push('$');
            }
            '&' => {
                chars.next();
                result.push_str(matched);
            }
            '`' => {
                chars.next();
                result.push_str(&input[..matched_start]);
            }
            '\'' => {
                chars.next();
                result.push_str(&input[matched_end..]);
            }
            '<' if !capture_names.is_empty() => {
                let name_start = next_index + next.len_utf8();
                if let Some(close_offset) = replacement[name_start..].find('>') {
                    let name_end = name_start + close_offset;
                    let name = &replacement[name_start..name_end];
                    for capture_index in named_capture_indices(capture_names, name) {
                        if let Some(Some(capture)) = captures.get(capture_index - 1) {
                            result.push_str(capture);
                            break;
                        }
                    }
                    chars.next();
                    while chars
                        .peek()
                        .is_some_and(|(index, _)| *index < name_end + '>'.len_utf8())
                    {
                        chars.next();
                    }
                } else {
                    result.push('$');
                }
            }
            '0'..='9' => {
                let first = next.to_digit(10).unwrap() as usize;
                let mut consumed = 1;
                let mut capture_index = 0;
                let after_next_index = next_index + next.len_utf8();
                if let Some(second) = replacement[after_next_index..].chars().next() {
                    if second.is_ascii_digit() {
                        let second_digit = second.to_digit(10).unwrap() as usize;
                        let two_digit = first * 10 + second_digit;
                        if (1..=capture_count).contains(&two_digit) {
                            capture_index = two_digit;
                            consumed = 2;
                        }
                    }
                }
                if capture_index == 0 && first != 0 && first <= capture_count {
                    capture_index = first;
                }
                if capture_index == 0 {
                    result.push('$');
                    result.push(next);
                    chars.next();
                    continue;
                }
                for _ in 0..consumed {
                    chars.next();
                }
                if let Some(Some(capture)) = captures.get(capture_index - 1) {
                    result.push_str(capture);
                }
            }
            _ => result.push('$'),
        }
    }
    result
}
/// String.prototype.lastIndexOf(searchString, fromIndex): last occurrence at
/// or before `fromIndex` (default +Inf -> search from end).
pub(crate) fn str_last_index_of(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let n = vm
        .to_string(args.first().unwrap_or(&Value::Undefined))?
        .to_string();
    let len = crate::value::utf16_len(&s);
    let raw = match args.get(1) {
        Some(v) => vm.to_number(v)?,
        None => f64::INFINITY,
    };
    let end = if raw.is_nan() || (raw.is_infinite() && raw > 0.0) {
        len
    } else if raw <= 0.0 || (raw.is_infinite() && raw < 0.0) {
        0
    } else {
        (raw.trunc() as usize).min(len)
    };
    Ok(crate::value::utf16_last_index_of(&s, &n, end)
        .map(|i| Value::Number(i as f64))
        .unwrap_or(Value::Number(-1.0)))
}

pub(crate) fn str_includes(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let n = string_search_arg(vm, args.first().unwrap_or(&Value::Undefined))?;
    let len = crate::value::utf16_len(&s);
    let start = string_search_position(vm, args, len, 0.0)?;
    let tail = crate::value::utf16_slice(&s, start, len);
    Ok(Value::Bool(tail.contains(n.as_str())))
}
pub(crate) fn str_starts_with(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let search = string_search_arg(vm, args.first().unwrap_or(&Value::Undefined))?;
    let len = crate::value::utf16_len(&s);
    let start = string_search_position(vm, args, len, 0.0)?;
    let tail = crate::value::utf16_slice(&s, start, len);
    Ok(Value::Bool(tail.starts_with(search.as_str())))
}
pub(crate) fn str_ends_with(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let search = string_search_arg(vm, args.first().unwrap_or(&Value::Undefined))?;
    let len = crate::value::utf16_len(&s);
    let end = string_search_position(vm, args, len, len as f64)?;
    let head = crate::value::utf16_slice(&s, 0, end);
    Ok(Value::Bool(head.ends_with(search.as_str())))
}
pub(crate) fn str_repeat(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    // ES String.prototype.repeat applies ToIntegerOrInfinity to count.
    // Negative values, Infinity, or too-large results throw RangeError.
    // Without this guard, `"x".repeat(Infinity)` panicked the engine with a
    // capacity overflow, and `"x".repeat(-1)` silently produced "" instead of
    // throwing. Cap the result length to keep untrusted code from OOM-allocating.
    let s = str_val(vm, &this)?;
    let count = match args.first() {
        Some(value) => to_integer_or_zero(vm, value)?,
        None => 0.0,
    };
    if count < 0.0 || count.is_infinite() {
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
    let receiver = this.clone().unwrap_or(Value::Undefined);
    if receiver.is_nullish() {
        return Err(Error::type_err(
            "String.prototype method called on null or undefined",
        ));
    }
    let match_key = PropertyKey::symbol(vm.well_known_symbols.r#match);
    let search_value = args.first().cloned().unwrap_or(Value::Undefined);
    if matches!(search_value, Value::Object(_)) {
        let matcher = vm.get_property_by_key(&search_value, &match_key)?;
        if !matcher.is_nullish() {
            let is_callable = matches!(&matcher, Value::Object(idx) if {
                vm.heap.with_obj(idx.0, |o| o.is_function())
            });
            if !is_callable {
                return Err(Error::type_err("Symbol.match method is not callable"));
            }
            return vm.call_function(
                &matcher,
                std::slice::from_ref(&receiver),
                Some(search_value),
            );
        }
    }
    let s = str_val(vm, &Some(receiver))?;
    let regexp = regexp_create_intrinsic(vm, &search_value)?;
    let matcher = vm.get_property_by_key(&regexp, &match_key)?;
    if !matcher.is_nullish() {
        let is_callable = matches!(&matcher, Value::Object(idx) if {
            vm.heap.with_obj(idx.0, |o| o.is_function())
        });
        if !is_callable {
            return Err(Error::type_err("Symbol.match method is not callable"));
        }
        return vm.call_function(
            &matcher,
            &[Value::String(Arc::from(s.as_str()))],
            Some(regexp),
        );
    }
    regexp_match_internal(vm, regexp, &s)
}

pub(crate) fn str_match_all(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.clone().unwrap_or(Value::Undefined);
    if receiver.is_nullish() {
        return Err(Error::type_err(
            "String.prototype method called on null or undefined",
        ));
    }

    let search_value = args.first().cloned().unwrap_or(Value::Undefined);
    if matches!(search_value, Value::Object(_)) {
        if is_regexp(vm, &search_value)? {
            let flags = vm.get_property(&search_value, "flags")?;
            if flags.is_nullish() {
                return Err(Error::type_err(
                    "RegExp flags must not be null or undefined",
                ));
            }
            let flags = vm.to_string(&flags)?.to_string();
            if !flags.contains('g') {
                return Err(Error::type_err(
                    "String.prototype.matchAll called with a non-global RegExp argument",
                ));
            }
        }

        let match_all_key = PropertyKey::symbol(vm.well_known_symbols.match_all);
        let matcher = vm.get_property_by_key(&search_value, &match_all_key)?;
        if !matcher.is_nullish() {
            let is_callable = matches!(&matcher, Value::Object(idx) if {
                vm.heap.with_obj(idx.0, |o| o.is_function())
            });
            if !is_callable {
                return Err(Error::type_err("Symbol.matchAll method is not callable"));
            }
            return vm.call_function(
                &matcher,
                std::slice::from_ref(&receiver),
                Some(search_value),
            );
        }
    }

    let s = str_val(vm, &Some(receiver))?;
    let regexp = regexp_create_intrinsic_with_flags(vm, &search_value, Some("g"))?;
    let match_all_key = PropertyKey::symbol(vm.well_known_symbols.match_all);
    let matcher = vm.get_property_by_key(&regexp, &match_all_key)?;
    if matcher.is_nullish() {
        return Err(Error::type_err("Symbol.matchAll method is not callable"));
    }
    let is_callable = matches!(&matcher, Value::Object(idx) if {
        vm.heap.with_obj(idx.0, |o| o.is_function())
    });
    if !is_callable {
        return Err(Error::type_err("Symbol.matchAll method is not callable"));
    }
    vm.call_function(
        &matcher,
        &[Value::String(Arc::from(s.as_str()))],
        Some(regexp),
    )
}

pub(super) fn regexp_match_internal(vm: &mut Vm, regexp: Value, s: &str) -> error::Result<Value> {
    let regexp = Some(regexp);
    let source = read_regexp_source_arc(vm, &regexp)?;
    let flags_str = read_regexp_flags_arc(vm, &regexp)?;
    let re = compile_regex_for_input_cached(vm, source.clone(), &flags_str, s)
        .map_err(regexp_compile_error)?;
    meter_logical_regex_input(vm, &re, s)?;
    let capture_names = regex_capture_names(&source, &flags_str).map_err(Error::syntax)?;
    let global = flags_str.contains('g');
    if global {
        let matches = re.find_iter_metered(s, || vm.consume_fuel())?;
        let items = matches
            .into_iter()
            .map(|matched| Value::String(Arc::from(matched.as_str())))
            .collect::<Vec<_>>();
        if items.is_empty() {
            Ok(Value::Null)
        } else {
            make_value_array(vm, items)
        }
    } else {
        match re.captures(s)? {
            Some(caps) => {
                let items: Vec<Value> = caps
                    .iter()
                    .map(|c| match c {
                        Some(m) => Value::String(Arc::from(m.as_str())),
                        None => Value::Undefined,
                    })
                    .collect();
                let match_start = caps
                    .get(0)
                    .map(|m| crate::value::utf16_len(&s[..m.start()]))
                    .unwrap_or(0);
                let groups = make_regexp_groups_object(vm, &caps, &capture_names)?;
                let groups_pin = vm.pin(&groups);
                let completion = (|| {
                    let result = make_value_array(vm, items)?;
                    add_regexp_exec_result_props(vm, &result, match_start, s, groups, None)?;
                    Ok(result)
                })();
                vm.unpin_many(groups_pin);
                completion
            }
            None => Ok(Value::Null),
        }
    }
}
pub(crate) fn str_pad_start(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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
    let mut units = crate::value::utf16_from_str(&out);
    units.truncate(need);
    out = crate::value::utf16_to_string(&units);
    out.push_str(&s);
    Ok(Value::String(Arc::from(out.as_str())))
}
pub(crate) fn str_pad_end(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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
    let mut units = crate::value::utf16_from_str(&out);
    units.truncate(target);
    out = crate::value::utf16_to_string(&units);
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
            crate::value::utf16_to_string(&[unit]).as_str(),
        )));
    }
    Ok(Value::Undefined)
}
pub(crate) fn str_trim_start(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    Ok(Value::String(Arc::from(
        s.trim_start_matches(is_js_trim_whitespace),
    )))
}
pub(crate) fn str_trim_end(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    Ok(Value::String(Arc::from(
        s.trim_end_matches(is_js_trim_whitespace),
    )))
}
pub(crate) fn str_replace_all(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.clone().unwrap_or(Value::Undefined);
    if receiver.is_nullish() {
        return Err(Error::type_err(
            "String.prototype method called on null or undefined",
        ));
    }

    let search_value = args.first().cloned().unwrap_or(Value::Undefined);
    let replace_value = args.get(1).cloned().unwrap_or(Value::Undefined);

    if !search_value.is_nullish() {
        if is_regexp(vm, &search_value)? {
            let flags = vm.get_property_by_key(&search_value, &PropertyKey::from("flags"))?;
            if flags.is_nullish() {
                return Err(Error::type_err(
                    "RegExp flags must not be null or undefined",
                ));
            }
            let flags = vm.to_string(&flags)?;
            if !flags.contains('g') {
                return Err(Error::type_err(
                    "String.prototype.replaceAll called with a non-global RegExp argument",
                ));
            }
        }

        if matches!(search_value, Value::Object(_)) {
            let replace_key = PropertyKey::symbol(vm.well_known_symbols.replace);
            let replacer = vm.get_property_by_key(&search_value, &replace_key)?;
            if !replacer.is_nullish() {
                let is_callable = matches!(&replacer, Value::Object(idx) if {
                    vm.heap.with_obj(idx.0, |o| o.is_function())
                });
                if !is_callable {
                    return Err(Error::type_err("Symbol.replace method is not callable"));
                }
                return vm.call_function(&replacer, &[receiver, replace_value], Some(search_value));
            }
        }
    }

    let s = str_val(vm, &Some(receiver))?;
    let search_string = vm.to_string(&search_value)?.to_string();
    let functional_replace = matches!(&replace_value, Value::Object(idx) if {
        vm.heap.with_obj(idx.0, |o| o.is_function())
    });
    let replacement_string = if functional_replace {
        String::new()
    } else {
        vm.to_string(&replace_value)?.to_string()
    };

    let match_positions = replace_all_match_positions(&s, &search_string);
    let search_len = crate::value::utf16_len(&search_string);
    let string_len = crate::value::utf16_len(&s);
    let mut result = String::new();
    let mut last_end = 0;

    for position in match_positions {
        result.push_str(&crate::value::utf16_slice(&s, last_end, position));

        let end = position + search_len;
        if functional_replace {
            let replacement = vm.call_function(
                &replace_value,
                &[
                    Value::String(Arc::from(search_string.as_str())),
                    Value::Number(position as f64),
                    Value::String(Arc::from(s.as_str())),
                ],
                None,
            )?;
            result.push_str(vm.to_string(&replacement)?.as_ref());
        } else {
            result.push_str(&string_replace_substitution(
                &replacement_string,
                &s,
                position,
                end,
                &search_string,
            ));
        }

        last_end = end;
    }

    result.push_str(&crate::value::utf16_slice(&s, last_end, string_len));
    Ok(Value::String(Arc::from(result.as_str())))
}

fn replace_all_match_positions(input: &str, search: &str) -> Vec<usize> {
    let input_len = crate::value::utf16_len(input);
    let search_len = crate::value::utf16_len(search);
    if search_len == 0 {
        return (0..=input_len).collect();
    }

    let mut positions = Vec::new();
    let mut next_position = 0;
    while let Some(position) = crate::value::utf16_index_of(input, search, next_position) {
        positions.push(position);
        next_position = position + search_len;
        if next_position > input_len {
            break;
        }
    }
    positions
}

fn string_replace_substitution(
    replacement: &str,
    input: &str,
    matched_start: usize,
    matched_end: usize,
    matched: &str,
) -> String {
    let input_len = crate::value::utf16_len(input);
    let mut result = String::new();
    let mut chars = replacement.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '$' {
            result.push(ch);
            continue;
        }
        let Some(next) = chars.peek().copied() else {
            result.push('$');
            continue;
        };
        match next {
            '$' => {
                chars.next();
                result.push('$');
            }
            '&' => {
                chars.next();
                result.push_str(matched);
            }
            '`' => {
                chars.next();
                result.push_str(&crate::value::utf16_slice(input, 0, matched_start));
            }
            '\'' => {
                chars.next();
                result.push_str(&crate::value::utf16_slice(input, matched_end, input_len));
            }
            _ => result.push('$'),
        }
    }
    result
}

pub(crate) fn str_normalize(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let form = match args.first() {
        None | Some(Value::Undefined) => "NFC".to_string(),
        Some(value) => vm.to_string(value)?.to_string(),
    };
    let normalized = match form.as_str() {
        "NFC" => s.nfc().collect::<String>(),
        "NFD" => s.nfd().collect::<String>(),
        "NFKC" => s.nfkc().collect::<String>(),
        "NFKD" => s.nfkd().collect::<String>(),
        _ => return Err(Error::range("Invalid normalization form".to_string())),
    };
    Ok(Value::String(Arc::from(normalized.as_str())))
}

pub(crate) fn str_substring(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let len = crate::value::utf16_len(&s) as f64;
    let mut start = match args.first() {
        Some(v) => to_integer_or_zero(vm, v)?,
        None => 0.0,
    };
    let mut end = match args.get(1) {
        Some(Value::Undefined) | None => len,
        Some(v) => to_integer_or_zero(vm, v)?,
    };
    if start < 0.0 {
        start = 0.0;
    }
    if end < 0.0 {
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

fn to_uint16_code_unit(n: f64) -> u16 {
    if !n.is_finite() || n == 0.0 {
        return 0;
    }
    n.trunc().rem_euclid(65536.0) as u16
}

pub(crate) fn str_from_char_code(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    // Build from UTF-16 code units. Unlike char::from_u32, this handles
    // surrogate pairs and lone surrogates correctly (each arg is one code
    // unit in [0, 65535] after ToUint16).
    let mut codes = Vec::with_capacity(args.len());
    for arg in args {
        codes.push(to_uint16_code_unit(vm.to_number(arg)?));
    }
    let s = crate::value::utf16_from_codes(&codes);
    Ok(Value::String(Arc::from(s.as_str())))
}

fn new_primitive_wrapper(vm: &mut Vm, intrinsic: &str, primitive: Value) -> error::Result<Value> {
    let fallback = vm.current_realm_primitive_prototype(&primitive);
    let prototype = native_constructor_prototype_with_default(vm, intrinsic, fallback)?;
    let wrapper = super::new_object_with_prototype(vm, prototype)?;
    vm.set_primitive(&wrapper, primitive);
    Ok(wrapper)
}

pub(crate) fn string_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    fn symbol_string(vm: &Vm, id: u32) -> Value {
        let desc = vm.symbol_descriptions.get(&id).and_then(|d| d.as_ref());
        Value::String(Arc::from(match desc {
            Some(desc) => format!("Symbol({desc})"),
            None => "Symbol()".to_string(),
        }))
    }

    let constructing = vm.current_native_new_target().is_some();
    let primitive = match args.first() {
        // `String()` with no argument yields "", distinct from
        // `String(undefined)` which yields "undefined".
        None => Value::String(Arc::from("")),
        Some(Value::Symbol(id)) if !constructing => symbol_string(vm, *id),
        Some(value) => Value::String(vm.to_string(value)?),
    };
    if !constructing {
        return Ok(primitive);
    }

    let Value::String(string) = &primitive else {
        unreachable!("String constructor must produce a String primitive");
    };
    let length = crate::value::utf16_len(string) as f64;
    let wrapper = new_primitive_wrapper(vm, "String", primitive)?;
    if let Value::Object(idx) = &wrapper {
        vm.heap.with_obj(idx.0, |object| {
            object.props().lock().insert(
                PropertyKey::from("length"),
                const_prop(Value::Number(length)),
            );
        });
    }
    Ok(wrapper)
}
pub(crate) fn number_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    fn number_from_constructor_arg(vm: &mut Vm, v: &Value) -> error::Result<Value> {
        let prim = match v {
            Value::Object(_) => vm.to_primitive_number(v)?,
            _ => v.clone(),
        };
        match prim {
            Value::BigInt(n) => Ok(Value::Number(
                num_traits::ToPrimitive::to_f64(n.as_ref()).unwrap_or_else(|| {
                    if n.sign() == num_bigint::Sign::Minus {
                        f64::NEG_INFINITY
                    } else {
                        f64::INFINITY
                    }
                }),
            )),
            _ => Ok(Value::Number(vm.to_number(&prim)?)),
        }
    }

    let primitive = match args.first() {
        None => Value::Number(0.0),
        Some(value) => number_from_constructor_arg(vm, value)?,
    };
    if vm.current_native_new_target().is_none() {
        return Ok(primitive);
    }

    new_primitive_wrapper(vm, "Number", primitive)
}

pub(crate) fn number_is_integer(
    _vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    match args.first() {
        Some(Value::Number(n)) if n.is_finite() && n.fract() == 0.0 => Ok(Value::Bool(true)),
        _ => Ok(Value::Bool(false)),
    }
}
pub(crate) fn number_is_finite(
    _vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    match args.first() {
        Some(Value::Number(n)) if n.is_finite() => Ok(Value::Bool(true)),
        _ => Ok(Value::Bool(false)),
    }
}
pub(crate) fn number_is_nan(
    _vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    match args.first() {
        Some(Value::Number(n)) if n.is_nan() => Ok(Value::Bool(true)),
        _ => Ok(Value::Bool(false)),
    }
}
pub(crate) fn number_is_safe_integer(
    _vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    match args.first() {
        Some(Value::Number(n))
            if n.is_finite() && n.fract() == 0.0 && n.abs() <= 9007199254740991.0 =>
        {
            Ok(Value::Bool(true))
        }
        _ => Ok(Value::Bool(false)),
    }
}
pub(crate) fn num_to_fixed(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let n = this_number_value(vm, this)?;
    let d = match args.first() {
        None | Some(Value::Undefined) => 0.0,
        Some(v) => {
            let number = vm.to_number(v)?;
            if number.is_nan() {
                0.0
            } else {
                number.trunc()
            }
        }
    };
    if !(0.0..=100.0).contains(&d) {
        return Err(Error::range(
            "toFixed() digits argument must be between 0 and 100",
        ));
    }
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
    if n.abs() >= 1e21 {
        return Ok(Value::String(Arc::from(
            crate::value::num_to_string(n).as_str(),
        )));
    }
    Ok(Value::String(Arc::from(
        format_to_fixed_decimal(n, d as usize).as_str(),
    )))
}

fn format_to_fixed_decimal(n: f64, digits: usize) -> String {
    let negative = n < 0.0;
    let x = if negative { -n } else { n };
    let scale = BigInt::from(10u32).pow(digits as u32);
    let scaled = f64_to_exact_ratio(x) * Ratio::from_integer(scale);
    let (mut rounded, rem) = scaled.numer().div_rem(scaled.denom());
    if &rem * 2 >= scaled.denom().clone() {
        rounded += 1;
    }

    let sign = if negative { "-" } else { "" };
    let mut decimal = rounded.to_string();
    if digits == 0 {
        return format!("{sign}{decimal}");
    }
    if decimal.len() <= digits {
        decimal = format!("{}{}", "0".repeat(digits + 1 - decimal.len()), decimal);
    }
    let split = decimal.len() - digits;
    format!("{sign}{}.{}", &decimal[..split], &decimal[split..])
}
pub(crate) fn num_to_precision(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let n = this_number_value(vm, this)?;
    match args.first() {
        Some(v) if !v.is_undefined() => {
            // ES: precision must be an integer in 1..=100, else RangeError.
            let pf = to_integer_or_zero(vm, v)?;
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
            if !(1.0..=100.0).contains(&pf) {
                return Err(Error::range(
                    "toPrecision() argument must be between 1 and 100",
                ));
            }
            let p = pf as usize;
            if n == 0.0 {
                return Ok(Value::String(Arc::from(format_precision_zero(p).as_str())));
            }
            // Use Rust's formatting with significant digits.
            let s = format!("{:.*e}", p - 1, n);
            // Convert exponential "1.23e4" to "12300" form for integer exp, else keep exp.
            let s = if let Some(pos) = s.find('e') {
                let mantissa = &s[..pos];
                let exp: i32 = s[pos + 1..].parse().unwrap_or(0);
                if exp >= p as i32 || exp < -6 {
                    normalize_exponential_string(&s, false)
                } else if exp >= 0 {
                    // Convert to fixed notation.
                    let negative = mantissa.starts_with('-');
                    let m = mantissa.trim_start_matches('-').replace('.', "");
                    let target_len = (exp + 1) as usize;
                    let mut result = if m.len() >= target_len {
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
                    };
                    if negative {
                        result.insert(0, '-');
                    }
                    result
                } else {
                    let negative = mantissa.starts_with('-');
                    let mut result = String::from("0.");
                    result.push_str(&"0".repeat((-exp - 1) as usize));
                    result.push_str(&mantissa.trim_start_matches('-').replace('.', ""));
                    if negative {
                        result.insert(0, '-');
                    }
                    result
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

pub(crate) fn num_to_exponential(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let n = this_number_value(vm, this)?;
    let fraction_digits = match args.first() {
        Some(v) if !v.is_undefined() => Some(to_integer_or_zero(vm, v)?),
        _ => None,
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
    if let Some(d) = fraction_digits {
        if !(0.0..=100.0).contains(&d) {
            return Err(Error::range(
                "toExponential() argument must be between 0 and 100",
            ));
        }
    }
    let n = if n == 0.0 { 0.0 } else { n };
    match fraction_digits {
        Some(d) => {
            let s = format_to_exponential_decimal(n, d as usize);
            Ok(Value::String(Arc::from(s.as_str())))
        }
        _ => {
            let s = format!("{:e}", n);
            let s = normalize_exponential_string(&s, true);
            Ok(Value::String(Arc::from(s.as_str())))
        }
    }
}

fn format_to_exponential_decimal(n: f64, fraction_digits: usize) -> String {
    if n == 0.0 {
        let mantissa = if fraction_digits == 0 {
            "0".to_string()
        } else {
            format!("0.{}", "0".repeat(fraction_digits))
        };
        return format!("{mantissa}e+0");
    }

    let negative = n < 0.0;
    let x = n.abs();
    let exact = f64_to_exact_ratio(x);
    let mut exponent = decimal_exponent(x, &exact);
    let scaled = exact * pow10_ratio(fraction_digits as i32 - exponent);
    let (mut rounded, rem) = scaled.numer().div_rem(scaled.denom());
    if &rem * 2 >= scaled.denom().clone() {
        rounded += 1;
    }

    let limit = BigInt::from(10u32).pow((fraction_digits + 1) as u32);
    if rounded >= limit {
        rounded /= 10;
        exponent += 1;
    }

    let mut digits = rounded.to_string();
    if digits.len() < fraction_digits + 1 {
        digits = format!(
            "{}{}",
            "0".repeat(fraction_digits + 1 - digits.len()),
            digits
        );
    }
    let mantissa = if fraction_digits == 0 {
        digits
    } else {
        format!("{}.{}", &digits[..1], &digits[1..])
    };
    let sign = if negative { "-" } else { "" };
    let exp_sign = if exponent >= 0 { "+" } else { "-" };
    format!("{sign}{mantissa}e{exp_sign}{}", exponent.abs())
}

fn decimal_exponent(x: f64, exact: &Ratio<BigInt>) -> i32 {
    let mut exponent = x.log10().floor() as i32;
    while exact < &pow10_ratio(exponent) {
        exponent -= 1;
    }
    while exact >= &pow10_ratio(exponent + 1) {
        exponent += 1;
    }
    exponent
}

fn pow10_ratio(exponent: i32) -> Ratio<BigInt> {
    let scale = BigInt::from(10u32).pow(exponent.unsigned_abs());
    if exponent >= 0 {
        Ratio::from_integer(scale)
    } else {
        Ratio::new(BigInt::from(1u32), scale)
    }
}

fn format_precision_zero(precision: usize) -> String {
    if precision == 1 {
        "0".to_string()
    } else {
        format!("0.{}", "0".repeat(precision - 1))
    }
}

fn normalize_exponential_string(s: &str, trim_mantissa: bool) -> String {
    let Some(pos) = s.find('e') else {
        return s.to_string();
    };
    let mut mantissa = s[..pos].to_string();
    if trim_mantissa {
        mantissa = mantissa
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();
        if mantissa.is_empty() || mantissa == "-" {
            mantissa.push('0');
        }
    }
    let exp = &s[pos + 1..];
    let (sign, digits) = if let Some(rest) = exp.strip_prefix('-') {
        ("-", rest)
    } else if let Some(rest) = exp.strip_prefix('+') {
        ("+", rest)
    } else {
        ("+", exp)
    };
    let digits = digits.trim_start_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    format!("{mantissa}e{sign}{digits}")
}

pub(crate) fn num_proto_to_string(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
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

pub(crate) fn boolean_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let primitive = Value::Bool(args.first().unwrap_or(&Value::Undefined).is_truthy());
    if vm.current_native_new_target().is_none() {
        return Ok(primitive);
    }

    new_primitive_wrapper(vm, "Boolean", primitive)
}
