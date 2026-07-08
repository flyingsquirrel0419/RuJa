use super::*;
use std::fmt::Write as _;

// RegExp
// =========================================================================
fn regexp_last_index_prop(value: Value) -> PropertyDescriptor {
    let mut desc = data_prop(value);
    desc.configurable = false;
    desc
}

const REGEXP_SOURCE_SLOT: &str = "[[RegExpSource]]";
const REGEXP_FLAGS_SLOT: &str = "[[RegExpFlags]]";
const REGEXP_HAS_INDICES_SLOT: &str = "[[RegExpHasIndices]]";
const REGEXP_GLOBAL_SLOT: &str = "[[RegExpGlobal]]";
const REGEXP_IGNORE_CASE_SLOT: &str = "[[RegExpIgnoreCase]]";
const REGEXP_MULTILINE_SLOT: &str = "[[RegExpMultiline]]";
const REGEXP_DOT_ALL_SLOT: &str = "[[RegExpDotAll]]";
const REGEXP_UNICODE_SLOT: &str = "[[RegExpUnicode]]";
const REGEXP_UNICODE_SETS_SLOT: &str = "[[RegExpUnicodeSets]]";
const REGEXP_STICKY_SLOT: &str = "[[RegExpSticky]]";

pub(crate) fn regexp_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let pattern_is_regexp = matches!(args.first(), Some(Value::Object(idx)) if {
        vm.heap.with_obj(idx.0, |o| {
            matches!(o, HeapObj::Object(od) if od.class_name.as_deref() == Some("RegExp"))
        })
    });
    let pattern = match args.first() {
        Some(v) if pattern_is_regexp => read_regexp_source(vm, &Some(v.clone()))?,
        Some(Value::String(s)) => s.to_string(),
        Some(v) if !v.is_undefined() => vm.to_string(v)?.to_string(),
        _ => String::new(),
    };
    let flags = match args.get(1) {
        Some(Value::String(s)) => s.to_string(),
        Some(v) if !v.is_undefined() => vm.to_string(v)?.to_string(),
        _ if pattern_is_regexp => read_regexp_flags(vm, &args.first().cloned())?,
        _ => String::new(),
    };
    // Look up RegExp.prototype via the global RegExp constructor.
    let regex_proto_val = {
        let reg = crate::environment::get(&vm.heap, vm.global, "RegExp");
        match reg {
            Some(Value::Object(ci)) => vm
                .heap
                .with_obj(ci.0, |o| {
                    o.props()
                        .lock()
                        .get(&crate::value::PropertyKey::from("prototype"))
                        .map(|d| d.value.clone())
                })
                .unwrap_or(vm.object_proto.clone()),
            _ => vm.object_proto.clone(),
        }
    };
    let regex_proto_val = native_constructor_prototype(vm, regex_proto_val)?;
    create_regexp_object(vm, pattern, flags, regex_proto_val)
}

pub(crate) fn regexp_escape(
    _vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let Some(Value::String(input)) = args.first() else {
        return Err(Error::type_err(
            "RegExp.escape requires a string".to_string(),
        ));
    };
    Ok(Value::String(Arc::from(
        regexp_escape_string(input).as_str(),
    )))
}

pub(crate) fn regexp_create_intrinsic(vm: &mut Vm, pattern: &Value) -> error::Result<Value> {
    let (pattern, flags) = if matches!(pattern, Value::Object(idx) if {
        vm.heap.with_obj(idx.0, |o| {
            matches!(o, HeapObj::Object(od) if od.class_name.as_deref() == Some("RegExp"))
        })
    }) {
        (
            read_regexp_source(vm, &Some(pattern.clone()))?,
            read_regexp_flags(vm, &Some(pattern.clone()))?,
        )
    } else if pattern.is_undefined() {
        (String::new(), String::new())
    } else {
        (vm.to_string(pattern)?.to_string(), String::new())
    };
    create_regexp_object(vm, pattern, flags, vm.regexp_proto.clone())
}

fn create_regexp_object(
    vm: &mut Vm,
    pattern: String,
    flags: String,
    proto: Value,
) -> error::Result<Value> {
    crate::lexer::validate_regex_literal(&pattern, &flags).map_err(Error::syntax)?;
    // Validate the pattern eagerly so bad regexes throw at construction time.
    compile_regex(&pattern, &flags).map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(proto)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("RegExp")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let mut props = IndexMap::new();
    props.insert(
        PropertyKey::from("lastIndex"),
        regexp_last_index_prop(Value::Number(0.0)),
    );
    vm.heap.with_obj(obj_idx, |o| {
        if let HeapObj::Object(obj) = o {
            let mut private_fields = obj.private_fields.lock();
            private_fields.insert(
                Arc::from(REGEXP_SOURCE_SLOT),
                crate::value::PrivateSlot::Value(Value::String(Arc::from(pattern.as_str()))),
            );
            private_fields.insert(
                Arc::from(REGEXP_FLAGS_SLOT),
                crate::value::PrivateSlot::Value(Value::String(Arc::from(flags.as_str()))),
            );
            private_fields.insert(
                Arc::from(REGEXP_HAS_INDICES_SLOT),
                crate::value::PrivateSlot::Value(Value::Bool(flags.contains('d'))),
            );
            private_fields.insert(
                Arc::from(REGEXP_GLOBAL_SLOT),
                crate::value::PrivateSlot::Value(Value::Bool(flags.contains('g'))),
            );
            private_fields.insert(
                Arc::from(REGEXP_IGNORE_CASE_SLOT),
                crate::value::PrivateSlot::Value(Value::Bool(flags.contains('i'))),
            );
            private_fields.insert(
                Arc::from(REGEXP_MULTILINE_SLOT),
                crate::value::PrivateSlot::Value(Value::Bool(flags.contains('m'))),
            );
            private_fields.insert(
                Arc::from(REGEXP_DOT_ALL_SLOT),
                crate::value::PrivateSlot::Value(Value::Bool(flags.contains('s'))),
            );
            private_fields.insert(
                Arc::from(REGEXP_UNICODE_SLOT),
                crate::value::PrivateSlot::Value(Value::Bool(flags.contains('u'))),
            );
            private_fields.insert(
                Arc::from(REGEXP_UNICODE_SETS_SLOT),
                crate::value::PrivateSlot::Value(Value::Bool(flags.contains('v'))),
            );
            private_fields.insert(
                Arc::from(REGEXP_STICKY_SLOT),
                crate::value::PrivateSlot::Value(Value::Bool(flags.contains('y'))),
            );
            *obj.props.lock() = props;
        }
    });
    Ok(Value::Object(GcIdx(obj_idx)))
}

pub(crate) fn regexp_test(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::Bool(!matches!(
        regexp_exec(vm, args, this)?,
        Value::Null
    )))
}

pub(crate) fn regexp_symbol_search(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let Some(rx @ Value::Object(_)) = this else {
        return Err(Error::type_err("not a RegExp".to_string()));
    };
    let s = vm
        .to_string(args.first().unwrap_or(&Value::Undefined))?
        .to_string();
    let previous_last_index = vm.get_property(&rx, "lastIndex")?;
    if !same_value(&previous_last_index, &Value::Number(0.0)) {
        vm.set_property_strict(&rx, "lastIndex", Value::Number(0.0))?;
    }
    let result = regexp_exec_dispatch(vm, &rx, &s)?;
    let current_last_index = vm.get_property(&rx, "lastIndex")?;
    if !same_value(&current_last_index, &previous_last_index) {
        vm.set_property_strict(&rx, "lastIndex", previous_last_index)?;
    }
    if result.is_null() {
        return Ok(Value::Number(-1.0));
    }
    vm.get_property(&result, "index")
}

pub(crate) fn regexp_symbol_match(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let Some(rx @ Value::Object(_)) = this else {
        return Err(Error::type_err("not a RegExp".to_string()));
    };
    let s = vm
        .to_string(args.first().unwrap_or(&Value::Undefined))?
        .to_string();
    let flags_value = vm.get_property(&rx, "flags")?;
    let flags = vm.to_string(&flags_value)?.to_string();
    let global = flags.contains('g');
    if !global {
        return regexp_exec_dispatch(vm, &rx, &s);
    }

    let full_unicode = flags.contains('u') || flags.contains('v');
    set_regexp_last_index(vm, &rx, 0.0)?;
    let mut matches = Vec::new();

    loop {
        let result = regexp_exec_dispatch(vm, &rx, &s)?;
        if result.is_null() {
            if matches.is_empty() {
                return Ok(Value::Null);
            }
            return make_value_array(vm, matches);
        }
        let matched_value = vm.get_property(&result, "0")?;
        let matched = vm.to_string(&matched_value)?.to_string();
        if matched.is_empty() {
            let last_index = vm.get_property(&rx, "lastIndex")?;
            let this_index = regexp_to_length(vm, &last_index)? as usize;
            let next_index = advance_string_index(&s, this_index, full_unicode);
            set_regexp_last_index(vm, &rx, next_index as f64)?;
        }
        matches.push(Value::String(Arc::from(matched.as_str())));
    }
}

fn regexp_exec_dispatch(vm: &mut Vm, rx: &Value, s: &str) -> error::Result<Value> {
    let exec = vm.get_property(rx, "exec")?;
    let is_callable = matches!(&exec, Value::Object(idx) if {
        vm.heap.with_obj(idx.0, |o| o.is_function())
    });
    if is_callable {
        let result = vm.call_function(&exec, &[Value::String(Arc::from(s))], Some(rx.clone()))?;
        if matches!(result, Value::Object(_) | Value::Null) {
            return Ok(result);
        }
        return Err(Error::type_err(
            "RegExp exec result must be an object or null",
        ));
    }
    regexp_exec(vm, &[Value::String(Arc::from(s))], Some(rx.clone()))
}

fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            (x.is_nan() && y.is_nan()) || x.to_bits() == y.to_bits()
        }
        _ => a == b,
    }
}

pub(crate) fn regexp_to_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let Some(this_value @ Value::Object(_)) = this else {
        return Err(Error::type_err(
            "RegExp method called on incompatible receiver",
        ));
    };
    let source_value = vm.get_property(&this_value, "source")?;
    let flags_value = vm.get_property(&this_value, "flags")?;
    let source = vm.to_string(&source_value)?.to_string();
    let flags = vm.to_string(&flags_value)?.to_string();
    Ok(Value::String(Arc::from(
        format!("/{source}/{flags}").as_str(),
    )))
}

pub(crate) fn regexp_symbol_replace(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    struct MatchRecord {
        matched: String,
        start_byte: usize,
        end_byte: usize,
        start_utf16: usize,
        captures: Vec<Option<String>>,
        groups: Value,
    }

    let rx = this.ok_or_else(|| Error::type_err("not a RegExp".to_string()))?;
    let Value::Object(_) = rx else {
        return Err(Error::type_err("not a RegExp".to_string()));
    };

    let s = vm
        .to_string(args.first().unwrap_or(&Value::Undefined))?
        .to_string();
    let replace_value = args.get(1).cloned().unwrap_or(Value::Undefined);
    let functional_replace = matches!(&replace_value, Value::Object(idx) if {
        vm.heap.with_obj(idx.0, |o| o.is_function())
    });
    let replace_string = if functional_replace {
        String::new()
    } else {
        vm.to_string(&replace_value)?.to_string()
    };

    let source = read_regexp_source(vm, &Some(rx.clone()))?;
    let flags = read_regexp_flags(vm, &Some(rx.clone())).unwrap_or_default();
    let global = flags.contains('g');
    let sticky = flags.contains('y');
    let full_unicode = flags.contains('u') || flags.contains('v');
    let re = compile_regex(&source, &flags)
        .map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
    let capture_names = regex_capture_names(&source);

    if global {
        set_regexp_last_index(vm, &rx, 0.0)?;
    }

    let input_len = crate::value::utf16_len(&s);
    let mut next_index = if global {
        0
    } else {
        let last_index = vm.get_property(&rx, "lastIndex")?;
        regexp_to_length(vm, &last_index)? as usize
    };
    let mut matches = Vec::new();

    loop {
        if next_index > input_len {
            if global || sticky {
                set_regexp_last_index(vm, &rx, 0.0)?;
            }
            break;
        }
        let Some(start_byte) = crate::value::utf16_index_to_byte(&s, next_index) else {
            if global || sticky {
                set_regexp_last_index(vm, &rx, 0.0)?;
            }
            break;
        };
        let caps = re
            .captures_at_ecma(&s, start_byte, &source, &flags)?
            .filter(|caps| {
                !sticky
                    || caps
                        .get(0)
                        .map(|matched| matched.start() == start_byte)
                        .unwrap_or(false)
            });
        let Some(caps) = caps else {
            if global || sticky {
                set_regexp_last_index(vm, &rx, 0.0)?;
            }
            break;
        };
        let Some(matched) = caps.get(0) else {
            break;
        };
        let match_start = crate::value::utf16_len(&s[..matched.start()]);
        let match_end = crate::value::utf16_len(&s[..matched.end()]);
        let groups = make_regexp_groups_object(vm, &caps, &capture_names)?;
        let captures = (1..caps.len())
            .map(|index| caps.get(index).map(|capture| capture.as_str().to_string()))
            .collect();
        matches.push(MatchRecord {
            matched: matched.as_str().to_string(),
            start_byte: matched.start(),
            end_byte: matched.end(),
            start_utf16: match_start,
            captures,
            groups,
        });

        if !global {
            break;
        }

        next_index = if match_end == match_start {
            advance_string_index(&s, match_end, full_unicode)
        } else {
            match_end
        };
        set_regexp_last_index(vm, &rx, next_index as f64)?;
    }

    let mut result = String::new();
    let mut next_source_position = 0;
    for record in matches {
        result.push_str(&s[next_source_position..record.start_byte]);
        if functional_replace {
            let mut call_args = vec![Value::String(Arc::from(record.matched.as_str()))];
            for capture in &record.captures {
                match capture {
                    Some(capture) => call_args.push(Value::String(Arc::from(capture.as_str()))),
                    None => call_args.push(Value::Undefined),
                }
            }
            call_args.push(Value::Number(record.start_utf16 as f64));
            call_args.push(Value::String(Arc::from(s.as_str())));
            if !record.groups.is_undefined() {
                call_args.push(record.groups.clone());
            }
            let replacement = vm.call_function(&replace_value, &call_args, None)?;
            result.push_str(vm.to_string(&replacement)?.as_ref());
        } else {
            let captures: Vec<Option<&str>> =
                record.captures.iter().map(|c| c.as_deref()).collect();
            result.push_str(&crate::builtins::string::replace_substitution(
                &replace_string,
                &s,
                record.start_byte,
                record.end_byte,
                &record.matched,
                &captures,
                &capture_names,
            ));
        }
        next_source_position = record.end_byte;
    }
    result.push_str(&s[next_source_position..]);
    Ok(Value::String(Arc::from(result.as_str())))
}

fn advance_string_index(input: &str, index: usize, unicode: bool) -> usize {
    if !unicode {
        return index + 1;
    }
    let units = crate::value::utf16_from_str(input);
    if index + 1 >= units.len() {
        return index + 1;
    }
    let first = units[index];
    let second = units[index + 1];
    if (0xD800..=0xDBFF).contains(&first) && (0xDC00..=0xDFFF).contains(&second) {
        index + 2
    } else {
        index + 1
    }
}

pub(crate) fn regexp_source_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let Some(Value::Object(this_idx)) = this else {
        return Err(Error::type_err(
            "RegExp getter called on incompatible receiver",
        ));
    };
    if is_current_realm_regexp_prototype(vm, this_idx) {
        return Ok(Value::String(Arc::from("(?:)")));
    }
    let raw_source = read_regexp_source(vm, &Some(Value::Object(this_idx)))?;
    Ok(Value::String(Arc::from(
        escape_regexp_source_for_accessor(&raw_source).as_str(),
    )))
}

pub(crate) fn regexp_flags_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let Some(this_value @ Value::Object(_)) = this else {
        return Err(Error::type_err(
            "RegExp getter called on incompatible receiver",
        ));
    };
    let mut flags = String::new();
    for (field, flag) in [
        ("hasIndices", 'd'),
        ("global", 'g'),
        ("ignoreCase", 'i'),
        ("multiline", 'm'),
        ("dotAll", 's'),
        ("unicode", 'u'),
        ("unicodeSets", 'v'),
        ("sticky", 'y'),
    ] {
        let value = vm.get_property(&this_value, field)?;
        if vm.to_boolean(&value) {
            flags.push(flag);
        }
    }
    Ok(Value::String(Arc::from(flags.as_str())))
}

fn regexp_bool_field_get(vm: &mut Vm, this: Option<Value>, field: &str) -> error::Result<Value> {
    match this {
        Some(Value::Object(idx)) => {
            let Some(slot_name) = regexp_bool_slot_name(field) else {
                return Ok(Value::Bool(false));
            };
            Ok(Value::Bool(
                read_regexp_private_bool(vm, idx, slot_name).unwrap_or(false),
            ))
        }
        _ => Err(Error::type_err(
            "RegExp getter called on incompatible receiver",
        )),
    }
}

pub(crate) fn regexp_global_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    regexp_bool_field_get(vm, this, "global")
}

pub(crate) fn regexp_ignore_case_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    regexp_bool_field_get(vm, this, "ignoreCase")
}

pub(crate) fn regexp_multiline_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    regexp_bool_field_get(vm, this, "multiline")
}

pub(crate) fn regexp_has_indices_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    regexp_bool_field_get(vm, this, "hasIndices")
}

pub(crate) fn regexp_dot_all_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    regexp_bool_field_get(vm, this, "dotAll")
}

pub(crate) fn regexp_unicode_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    regexp_bool_field_get(vm, this, "unicode")
}

pub(crate) fn regexp_unicode_sets_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    regexp_bool_field_get(vm, this, "unicodeSets")
}

pub(crate) fn regexp_sticky_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    regexp_bool_field_get(vm, this, "sticky")
}

pub(crate) fn regexp_exec(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let source = read_regexp_source(vm, &this)?;
    let input = vm
        .to_string(args.first().unwrap_or(&Value::Undefined))?
        .to_string();
    let flags = read_regexp_flags(vm, &this).unwrap_or_default();
    let re = compile_regex(&source, &flags)
        .map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
    let capture_names = regex_capture_names(&source);
    let global = flags.contains('g');
    let sticky = flags.contains('y');
    let this_value = match &this {
        Some(value @ Value::Object(_)) => Some(value.clone()),
        _ => None,
    };
    let last_idx = match &this_value {
        Some(value) => {
            let last_index_value = vm.get_property(value, "lastIndex")?;
            regexp_to_length(vm, &last_index_value)?
        }
        _ => 0.0,
    };
    // Start position: for global/sticky, read lastIndex; else 0.
    let start: usize = if global || sticky {
        last_idx as usize
    } else {
        0
    };
    let utf16_len = crate::value::utf16_len(&input);
    if start > utf16_len {
        if global || sticky {
            if let Some(value) = &this_value {
                set_regexp_last_index(vm, value, 0.0)?;
            }
        }
        return Ok(Value::Null);
    }
    let Some(start_byte) = crate::value::utf16_index_to_byte(&input, start) else {
        if global || sticky {
            if let Some(value) = &this_value {
                set_regexp_last_index(vm, value, 0.0)?;
            }
        }
        return Ok(Value::Null);
    };
    // Run against the whole input so `^` still observes the real input start
    // and multiline line starts; sticky only requires the match to begin at
    // lastIndex.
    let m = re
        .captures_at_ecma(&input, start_byte, &source, &flags)?
        .filter(|c| {
            !sticky
                || c.get(0)
                    .map(|mch| mch.start() == start_byte)
                    .unwrap_or(false)
        });
    match m {
        Some(caps) => {
            let items: Vec<Value> = caps
                .iter()
                .map(|c| match c {
                    Some(mch) => Value::String(Arc::from(mch.as_str())),
                    None => Value::Undefined,
                })
                .collect();
            if global || sticky {
                let match_end = caps
                    .get(0)
                    .map(|mch| crate::value::utf16_len(&input[..mch.end()]))
                    .unwrap_or(start);
                if let Some(value) = &this_value {
                    set_regexp_last_index(vm, value, match_end as f64)?;
                }
            }
            let match_start = caps
                .get(0)
                .map(|mch| crate::value::utf16_len(&input[..mch.start()]))
                .unwrap_or(start);
            let result = make_value_array(vm, items)?;
            let groups = make_regexp_groups_object(vm, &caps, &capture_names)?;
            add_regexp_exec_result_props(vm, &result, match_start, &input, groups)?;
            Ok(result)
        }
        None => {
            // No match: for global/sticky, reset lastIndex to 0.
            if global || sticky {
                if let Some(value) = &this_value {
                    set_regexp_last_index(vm, value, 0.0)?;
                }
            }
            Ok(Value::Null)
        }
    }
}

fn regexp_to_length(vm: &mut Vm, value: &Value) -> error::Result<f64> {
    let number = vm.to_number(value)?;
    if number.is_nan() || number <= 0.0 {
        return Ok(0.0);
    }
    if number.is_infinite() {
        return Ok(9_007_199_254_740_991.0);
    }
    Ok(number.trunc().min(9_007_199_254_740_991.0))
}

fn enumerable_data_prop(value: Value) -> PropertyDescriptor {
    PropertyDescriptor {
        value,
        writable: true,
        enumerable: true,
        configurable: true,
        get: None,
        set: None,
        is_accessor: false,
    }
}

pub(crate) fn add_regexp_exec_result_props(
    vm: &mut Vm,
    result: &Value,
    match_start: usize,
    input: &str,
    groups: Value,
) -> error::Result<()> {
    let Value::Object(idx) = result else {
        return Ok(());
    };
    vm.heap.with_obj(idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("index"),
            enumerable_data_prop(Value::Number(match_start as f64)),
        );
        props.insert(
            PropertyKey::from("input"),
            enumerable_data_prop(Value::String(Arc::from(input))),
        );
        props.insert(PropertyKey::from("groups"), enumerable_data_prop(groups));
    });
    Ok(())
}

fn set_regexp_last_index(vm: &mut Vm, target: &Value, value: f64) -> error::Result<()> {
    let Value::Object(idx) = target else {
        return Err(Error::type_err("not a RegExp".to_string()));
    };
    let key = PropertyKey::from("lastIndex");
    let outcome = vm.heap.with_obj(idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        match props.get_mut(&key) {
            Some(desc) if desc.is_accessor || !desc.writable => false,
            Some(desc) => {
                desc.value = Value::Number(value);
                true
            }
            None => {
                props.insert(key, regexp_last_index_prop(Value::Number(value)));
                true
            }
        }
    });
    if outcome {
        vm.ic_invalidate(idx.0, "lastIndex");
        Ok(())
    } else {
        Err(Error::type_err(
            "Cannot assign to read only property 'lastIndex' of object",
        ))
    }
}

pub(crate) fn read_regexp_source(vm: &mut Vm, this: &Option<Value>) -> error::Result<String> {
    read_regexp_field(vm, this, "source")
}

/// Read the `flags` string of a RegExp object.
pub(crate) fn read_regexp_flags(vm: &mut Vm, this: &Option<Value>) -> error::Result<String> {
    read_regexp_field(vm, this, "flags")
}

/// Read a string field (`source`/`flags`/`lastIndex`) from a RegExp object.
pub(crate) fn read_regexp_field(
    vm: &mut Vm,
    this: &Option<Value>,
    field: &str,
) -> error::Result<String> {
    match this {
        Some(Value::Object(idx)) => {
            let s = match field {
                "source" => read_regexp_private_string(vm, *idx, REGEXP_SOURCE_SLOT),
                "flags" => read_regexp_private_string(vm, *idx, REGEXP_FLAGS_SLOT),
                other => vm.heap.with_obj(idx.0, |o| {
                    o.props()
                        .lock()
                        .get(&crate::value::PropertyKey::from(other))
                        .map(|d| d.value.clone())
                }),
            };
            match s {
                Some(Value::String(s)) => Ok(s.to_string()),
                _ => {
                    if field == "lastIndex" {
                        Ok("0".to_string())
                    } else {
                        Err(Error::type_err("not a RegExp".to_string()))
                    }
                }
            }
        }
        _ => Err(Error::type_err("not a RegExp".to_string())),
    }
}

fn regexp_bool_slot_name(field: &str) -> Option<&'static str> {
    match field {
        "hasIndices" => Some(REGEXP_HAS_INDICES_SLOT),
        "global" => Some(REGEXP_GLOBAL_SLOT),
        "ignoreCase" => Some(REGEXP_IGNORE_CASE_SLOT),
        "multiline" => Some(REGEXP_MULTILINE_SLOT),
        "dotAll" => Some(REGEXP_DOT_ALL_SLOT),
        "unicode" => Some(REGEXP_UNICODE_SLOT),
        "unicodeSets" => Some(REGEXP_UNICODE_SETS_SLOT),
        "sticky" => Some(REGEXP_STICKY_SLOT),
        _ => None,
    }
}

fn read_regexp_private_string(vm: &mut Vm, idx: GcIdx, slot_name: &str) -> Option<Value> {
    vm.heap.with_obj(idx.0, |o| {
        let HeapObj::Object(obj) = o else {
            return None;
        };
        obj.private_fields
            .lock()
            .get(slot_name)
            .and_then(|slot| match slot {
                crate::value::PrivateSlot::Value(value @ Value::String(_)) => Some(value.clone()),
                crate::value::PrivateSlot::Value(_)
                | crate::value::PrivateSlot::Accessor { .. } => None,
            })
    })
}

fn read_regexp_private_bool(vm: &mut Vm, idx: GcIdx, slot_name: &str) -> Option<bool> {
    vm.heap.with_obj(idx.0, |o| {
        let HeapObj::Object(obj) = o else {
            return None;
        };
        obj.private_fields
            .lock()
            .get(slot_name)
            .and_then(|slot| match slot {
                crate::value::PrivateSlot::Value(Value::Bool(value)) => Some(*value),
                crate::value::PrivateSlot::Value(_)
                | crate::value::PrivateSlot::Accessor { .. } => None,
            })
    })
}

fn escape_regexp_source_for_accessor(source: &str) -> String {
    if source.is_empty() {
        return "(?:)".to_string();
    }
    let mut out = String::with_capacity(source.len());
    for ch in source.chars() {
        match ch {
            '/' => out.push_str("\\/"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            _ => out.push(ch),
        }
    }
    out
}

fn regexp_escape_string(source: &str) -> String {
    let units = crate::value::utf16_from_str(source);
    let mut out = String::new();
    let mut i = 0;
    let mut first = true;

    while i < units.len() {
        let unit = units[i];
        let code_units = if (0xD800..=0xDBFF).contains(&unit) && i + 1 < units.len() {
            let low = units[i + 1];
            if (0xDC00..=0xDFFF).contains(&low) {
                i += 2;
                &units[i - 2..i]
            } else {
                i += 1;
                &units[i - 1..i]
            }
        } else {
            i += 1;
            &units[i - 1..i]
        };

        let code_point = regexp_escape_code_point_value(code_units);
        if first && is_ascii_letter_or_decimal_digit(code_point) {
            push_hex_escape(&mut out, code_point);
        } else {
            push_encoded_regexp_escape(&mut out, code_point, code_units);
        }
        first = false;
    }

    out
}

fn regexp_escape_code_point_value(units: &[u16]) -> u32 {
    debug_assert!(!units.is_empty());
    if units.len() == 2 {
        let high = units[0] as u32;
        let low = units[1] as u32;
        0x10000 + (((high - 0xD800) << 10) | (low - 0xDC00))
    } else {
        units[0] as u32
    }
}

fn is_ascii_letter_or_decimal_digit(code_point: u32) -> bool {
    matches!(code_point, 0x30..=0x39 | 0x41..=0x5A | 0x61..=0x7A)
}

fn push_encoded_regexp_escape(out: &mut String, code_point: u32, units: &[u16]) {
    match code_point {
        0x09 => out.push_str("\\t"),
        0x0A => out.push_str("\\n"),
        0x0B => out.push_str("\\v"),
        0x0C => out.push_str("\\f"),
        0x0D => out.push_str("\\r"),
        0x5E | 0x24 | 0x5C | 0x2E | 0x2A | 0x2B | 0x3F | 0x28 | 0x29 | 0x5B | 0x5D | 0x7B
        | 0x7D | 0x7C | 0x2F => {
            out.push('\\');
            out.push(char::from_u32(code_point).unwrap());
        }
        _ if is_regexp_escape_other_punctuator(code_point)
            || is_regexp_escape_whitespace_or_lineterminator(code_point)
            || (0xD800..=0xDFFF).contains(&code_point) =>
        {
            if code_point <= 0xFF {
                push_hex_escape(out, code_point);
            } else {
                for unit in units {
                    push_unicode_escape(out, *unit);
                }
            }
        }
        _ => out.push_str(&crate::value::utf16_to_string(units)),
    }
}

fn is_regexp_escape_other_punctuator(code_point: u32) -> bool {
    matches!(
        code_point,
        0x2C | 0x2D
            | 0x3D
            | 0x3C
            | 0x3E
            | 0x23
            | 0x26
            | 0x21
            | 0x25
            | 0x3A
            | 0x3B
            | 0x40
            | 0x7E
            | 0x27
            | 0x60
            | 0x22
    )
}

fn is_regexp_escape_whitespace_or_lineterminator(code_point: u32) -> bool {
    matches!(
        code_point,
        0x0009 | 0x000A | 0x000B | 0x000C | 0x000D | 0x0020 | 0x00A0 | 0x1680 | 0x2000
            ..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000 | 0xFEFF
    )
}

fn push_hex_escape(out: &mut String, code_point: u32) {
    debug_assert!(code_point <= 0xFF);
    write!(out, "\\x{code_point:02x}").unwrap();
}

fn push_unicode_escape(out: &mut String, unit: u16) {
    write!(out, "\\u{unit:04x}").unwrap();
}

fn is_current_realm_regexp_prototype(vm: &mut Vm, value: GcIdx) -> bool {
    let realm_env = vm.native_callee_closure().unwrap_or(vm.global);
    let Some(Value::Object(regexp_ctor)) = crate::environment::get(&vm.heap, realm_env, "RegExp")
    else {
        return false;
    };
    let proto = vm.heap.with_obj(regexp_ctor.0, |o| {
        o.props()
            .lock()
            .get(&PropertyKey::from("prototype"))
            .map(|desc| desc.value.clone())
    });
    matches!(proto, Some(Value::Object(proto_idx)) if proto_idx == value)
}

pub(crate) fn generator_next(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let g_idx = match &this {
        Some(Value::Object(idx)) => idx.0,
        _ => return Err(Error::type_err("not a generator".to_string())),
    };
    // Lazy generators run their body incrementally across next() calls.
    let (is_lazy, is_async_gen) = vm.heap.with_obj(g_idx, |o| {
        if let HeapObj::LazyGenerator(g) = o {
            (true, g.is_async)
        } else {
            (matches!(o, HeapObj::Generator(_)), false)
        }
    });
    let (value, done) = if is_lazy {
        let resume = _args.first().cloned().unwrap_or(Value::Undefined);
        vm.resume_generator(GcIdx(g_idx), crate::vm::ResumeKind::Next(resume))?
    } else {
        // Legacy eager generator (kept for safety).
        vm.heap.with_obj(g_idx, |o| {
            if let HeapObj::Generator(g) = o {
                let state = g.state.lock();
                let idx = g.ip.load(Ordering::Relaxed);
                if idx < state.len() {
                    g.ip.store(idx + 1, Ordering::Relaxed);
                    (state[idx].clone(), false)
                } else {
                    g.done.store(true, Ordering::Relaxed);
                    (Value::Undefined, true)
                }
            } else {
                (Value::Undefined, true)
            }
        })
    };
    // return {value, done}
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    vm.heap.with_obj(obj_idx, |o| {
        if let HeapObj::Object(obj) = o {
            obj.props
                .lock()
                .insert(PropertyKey::from("value"), data_prop(value));
            obj.props
                .lock()
                .insert(PropertyKey::from("done"), data_prop(Value::Bool(done)));
        }
    });
    let result_obj = Value::Object(GcIdx(obj_idx));
    if is_async_gen {
        // async function*: next() returns a Promise resolved with {value, done}.
        let p_idx = vm
            .heap
            .allocate(HeapObj::Promise(crate::value::PromiseData {
                state: Mutex::new(crate::value::PromiseStatus::Fulfilled),
                result: Mutex::new(result_obj.clone()),
                handlers: Mutex::new(Vec::new()),
                props: Mutex::new(IndexMap::new()),
                proto: Mutex::new(Some(vm.promise_proto.clone())),
            }))?;
        Ok(Value::Object(GcIdx(p_idx)))
    } else {
        Ok(result_obj)
    }
}

/// Build a {value, done} object, wrapped in a Promise for async generators.
pub(crate) fn gen_result(
    vm: &mut Vm,
    value: Value,
    done: bool,
    is_async_gen: bool,
) -> error::Result<Value> {
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    vm.heap.with_obj(obj_idx, |o| {
        if let HeapObj::Object(obj) = o {
            obj.props
                .lock()
                .insert(PropertyKey::from("value"), data_prop(value));
            obj.props
                .lock()
                .insert(PropertyKey::from("done"), data_prop(Value::Bool(done)));
        }
    });
    let result_obj = Value::Object(GcIdx(obj_idx));
    if is_async_gen {
        let p_idx = vm
            .heap
            .allocate(HeapObj::Promise(crate::value::PromiseData {
                state: Mutex::new(crate::value::PromiseStatus::Fulfilled),
                result: Mutex::new(result_obj),
                handlers: Mutex::new(Vec::new()),
                props: Mutex::new(IndexMap::new()),
                proto: Mutex::new(Some(vm.promise_proto.clone())),
            }))?;
        Ok(Value::Object(GcIdx(p_idx)))
    } else {
        Ok(result_obj)
    }
}

/// `generator.return(v)`: force-complete the generator. If it is suspended at
/// a `yield`, the value `v` becomes the result of the yield* / next() call and
/// the generator is marked done. If it was already done, returns {value:v,
/// done:true}.
pub(crate) fn generator_return(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let g_idx = match &this {
        Some(Value::Object(idx)) => idx.0,
        _ => return Err(Error::type_err("not a generator".to_string())),
    };
    let is_async_gen = vm.heap.with_obj(g_idx, |o| {
        if let HeapObj::LazyGenerator(g) = o {
            g.is_async
        } else {
            false
        }
    });
    let ret = args.first().cloned().unwrap_or(Value::Undefined);
    let is_lazy = vm
        .heap
        .with_obj(g_idx, |o| matches!(o, HeapObj::LazyGenerator(_)));
    let (value, done) = if is_lazy {
        vm.resume_generator(GcIdx(g_idx), crate::vm::ResumeKind::Return(ret.clone()))?
    } else {
        (ret.clone(), true)
    };
    gen_result(vm, value, done, is_async_gen)
}

/// `generator.throw(v)`: inject an exception into the suspended generator. The
/// generator resumes so the suspended `yield` throws `v`; if the body catches
/// it, the catch handler runs and the next value is returned, otherwise the
/// exception propagates out of the `throw()` call.
pub(crate) fn generator_throw(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let g_idx = match &this {
        Some(Value::Object(idx)) => idx.0,
        _ => return Err(Error::type_err("not a generator".to_string())),
    };
    let is_async_gen = vm.heap.with_obj(g_idx, |o| {
        if let HeapObj::LazyGenerator(g) = o {
            g.is_async
        } else {
            false
        }
    });
    let exc = args.first().cloned().unwrap_or(Value::Undefined);
    let already_done = vm.heap.with_obj(
        g_idx,
        |o| matches!(o, HeapObj::LazyGenerator(g) if g.done.load(Ordering::Relaxed)),
    );
    if already_done {
        // Per spec, throw on a finished generator re-throws.
        return Err(Error::thrown(exc, &vm.heap));
    }
    let (value, done) = vm.resume_generator(GcIdx(g_idx), crate::vm::ResumeKind::Throw(exc))?;
    gen_result(vm, value, done, is_async_gen)
}

pub fn setup_collections(vm: &mut Vm) -> error::Result<()> {
    // Map
    let (map_ctor, map_proto) = make_builtin_constructor_with(
        vm,
        "Map",
        0,
        map_constructor,
        &[
            ("set", map_set, 2),
            ("get", map_get, 1),
            ("has", map_has, 1),
            ("delete", map_delete, 1),
            ("clear", map_clear, 0),
            ("entries", map_entries, 0),
            ("keys", map_keys, 0),
            ("values", map_values, 0),
            ("forEach", map_for_each, 1),
            ("getOrInsert", map_get_or_insert, 2),
            ("getOrInsertComputed", map_get_or_insert_computed, 2),
        ],
    )?;
    vm.map_proto = Value::Object(map_proto);
    define_global(vm, "Map", Value::Object(map_ctor));
    let map_size_getter = vm.new_native_function("get size", map_size, 0)?;
    vm.heap.with_obj(map_proto.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("size"),
            accessor_get_prop(Value::Object(map_size_getter)),
        );
    });
    let map_species_getter =
        vm.new_native_function("get [Symbol.species]", promise_species_get, 0)?;
    let map_group_by_fn = vm.new_native_function("groupBy", map_group_by, 2)?;
    vm.heap.with_obj(map_ctor.0, |obj| {
        let mut props = obj.props().lock();
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.species),
            accessor_get_prop(Value::Object(map_species_getter)),
        );
        props.insert(
            PropertyKey::from("groupBy"),
            data_prop(Value::Object(map_group_by_fn)),
        );
    });
    // Map.prototype[Symbol.iterator] === Map.prototype.entries
    if let Value::Object(mp) = vm.map_proto.clone() {
        vm.heap.with_obj(mp.0, |o| {
            let entries = o
                .props()
                .lock()
                .get(&PropertyKey::from("entries"))
                .map(|desc| desc.value.clone())
                .unwrap_or(Value::Undefined);
            o.props().lock().insert(
                PropertyKey::Symbol(vm.well_known_symbols.iterator),
                data_prop(entries),
            );
        });
    }
    // Set
    let (set_ctor, set_proto) = make_builtin_constructor_with(
        vm,
        "Set",
        0,
        set_constructor,
        &[
            ("add", set_add, 1),
            ("has", set_has, 1),
            ("delete", set_delete, 1),
            ("clear", set_clear, 0),
            ("entries", set_entries, 0),
            ("keys", set_keys, 0),
            ("values", set_values, 0),
            ("forEach", set_for_each, 1),
            ("union", set_union, 1),
            ("intersection", set_intersection, 1),
            ("difference", set_difference, 1),
            ("symmetricDifference", set_symmetric_difference, 1),
            ("isSubsetOf", set_is_subset_of, 1),
            ("isSupersetOf", set_is_superset_of, 1),
            ("isDisjointFrom", set_is_disjoint_from, 1),
        ],
    )?;
    vm.set_proto = Value::Object(set_proto);
    define_global(vm, "Set", Value::Object(set_ctor));
    let set_size_getter = vm.new_native_function("get size", set_size, 0)?;
    vm.heap.with_obj(set_proto.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("size"),
            accessor_get_prop(Value::Object(set_size_getter)),
        );
    });
    let set_species_getter =
        vm.new_native_function("get [Symbol.species]", promise_species_get, 0)?;
    vm.heap.with_obj(set_ctor.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::Symbol(vm.well_known_symbols.species),
            accessor_get_prop(Value::Object(set_species_getter)),
        );
    });
    // Set.prototype.keys === Set.prototype.values and @@iterator is values.
    if let Value::Object(sp) = vm.set_proto.clone() {
        vm.heap.with_obj(sp.0, |o| {
            let values = o
                .props()
                .lock()
                .get(&PropertyKey::from("values"))
                .map(|desc| desc.value.clone())
                .unwrap_or(Value::Undefined);
            o.props()
                .lock()
                .insert(PropertyKey::from("keys"), data_prop(values.clone()));
            o.props().lock().insert(
                PropertyKey::Symbol(vm.well_known_symbols.iterator),
                data_prop(values),
            );
        });
    }
    // WeakMap / WeakSet: true weak-reference semantics. Keys are object
    // heap indices held weakly; GC sweeps entries whose key was collected.
    let (weakmap_ctor, weakmap_proto) = make_builtin_constructor_with(
        vm,
        "WeakMap",
        0,
        weakmap_constructor,
        &[
            ("get", weakmap_get, 1),
            ("set", weakmap_set, 2),
            ("has", weakmap_has, 1),
            ("delete", weakmap_delete, 1),
        ],
    )?;
    define_global(vm, "WeakMap", Value::Object(weakmap_ctor));
    let _ = weakmap_proto;
    let (weakset_ctor, weakset_proto) = make_builtin_constructor_with(
        vm,
        "WeakSet",
        0,
        weakset_constructor,
        &[
            ("add", weakset_add, 1),
            ("has", weakset_has, 1),
            ("delete", weakset_delete, 1),
        ],
    )?;
    define_global(vm, "WeakSet", Value::Object(weakset_ctor));
    let _ = weakset_proto;

    // Symbol
    let sym_idx = vm.new_native_function("Symbol", symbol_constructor, 0)?;
    define_global(vm, "Symbol", Value::Object(sym_idx));
    let sym_for_idx = vm.new_native_function("for", symbol_for, 1)?;
    let sym_key_for_idx = vm.new_native_function("keyFor", symbol_key_for, 1)?;
    if let Value::Object(idx) = Value::Object(sym_idx) {
        vm.heap.with_obj(idx.0, |obj| {
            let mut props = obj.props().lock();
            props.insert(
                PropertyKey::from("for"),
                data_prop(Value::Object(sym_for_idx)),
            );
            props.insert(
                PropertyKey::from("keyFor"),
                data_prop(Value::Object(sym_key_for_idx)),
            );
            install_symbol_static_properties(vm, &mut props);
        });
    }
    // Symbol.prototype: a plain Object with a toString method. Symbol is a
    // value type (not a constructor), so build the proto manually rather than
    // going through make_builtin_constructor.
    let sym_tostring_idx = vm.new_native_function("toString", symbol_to_string, 0)?;
    let sym_valueof_idx = vm.new_native_function("valueOf", symbol_value_of, 0)?;
    let sym_description_getter =
        vm.new_native_function("get description", symbol_description_get, 0)?;
    let mut sym_proto_props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    sym_proto_props.insert(
        PropertyKey::from("toString"),
        data_prop(Value::Object(sym_tostring_idx)),
    );
    sym_proto_props.insert(
        PropertyKey::from("valueOf"),
        data_prop(Value::Object(sym_valueof_idx)),
    );
    sym_proto_props.insert(
        PropertyKey::from("description"),
        accessor_get_prop(Value::Object(sym_description_getter)),
    );
    sym_proto_props.insert(
        PropertyKey::from("constructor"),
        data_prop(Value::Object(sym_idx)),
    );
    let sym_proto_obj = HeapObj::Object(ObjectData {
        props: Mutex::new(sym_proto_props),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Symbol")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    let sym_proto_idx = GcIdx(vm.heap.allocate(sym_proto_obj)?);
    vm.symbol_proto = Value::Object(sym_proto_idx);
    vm.heap.with_obj(sym_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(Value::Object(sym_proto_idx)),
        );
    });
    Ok(())
}

pub(crate) fn make_builtin_constructor_with(
    vm: &mut Vm,
    name: &str,
    length: usize,
    ctor: NativeFn,
    methods: &[(&str, NativeFn, usize)],
) -> error::Result<(GcIdx, GcIdx)> {
    make_builtin_constructor_with_in_env(vm, name, length, ctor, methods, vm.global)
}

pub(crate) fn make_builtin_constructor_with_in_env(
    vm: &mut Vm,
    name: &str,
    length: usize,
    ctor: NativeFn,
    methods: &[(&str, NativeFn, usize)],
    env: GcIdx,
) -> error::Result<(GcIdx, GcIdx)> {
    let mut method_props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    for (n, f, len) in methods {
        let func_idx = vm.new_native_function_in_env(n, *f, *len, env)?;
        method_props.insert(PropertyKey::from(*n), data_prop(Value::Object(func_idx)));
    }
    let proto_obj = HeapObj::Object(ObjectData {
        props: Mutex::new(method_props),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from(name)),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    let proto_idx = GcIdx(vm.heap.allocate(proto_obj)?);
    let ctor_func = FunctionData {
        name: Some(Arc::from(name)),
        kind: FunctionKind::Native { func: ctor, length },
        closure: env,
        lexical_new_target: Value::Undefined,
        is_class_ctor: std::sync::atomic::AtomicBool::new(false),
        prototype: Mutex::new(Some(Value::Object(proto_idx))),
        proto: Mutex::new(match vm.function_proto {
            Value::Object(_) => Some(vm.function_proto.clone()),
            _ => None,
        }),
        props: Mutex::new(builtin_function_own_props(name, length)),
        extensible: AtomicBool::new(true),
        private_fields: Mutex::new(std::collections::HashMap::new()),
    };
    let ctor_idx = GcIdx(vm.heap.allocate(HeapObj::Function(ctor_func))?);
    vm.heap.with_obj(ctor_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(Value::Object(proto_idx)),
        );
    });
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(ctor_idx)),
        );
    });
    Ok((ctor_idx, proto_idx))
}

// =========================================================================
