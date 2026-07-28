use super::call_arguments::MAX_MATERIALIZED_CALL_ARGUMENTS;
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
const REGEXP_MATCHER_SLOT: &str = "[[RegExpMatcher]]";
const REGEXP_HAS_INDICES_SLOT: &str = "[[RegExpHasIndices]]";
const REGEXP_GLOBAL_SLOT: &str = "[[RegExpGlobal]]";
const REGEXP_IGNORE_CASE_SLOT: &str = "[[RegExpIgnoreCase]]";
const REGEXP_MULTILINE_SLOT: &str = "[[RegExpMultiline]]";
const REGEXP_DOT_ALL_SLOT: &str = "[[RegExpDotAll]]";
const REGEXP_UNICODE_SLOT: &str = "[[RegExpUnicode]]";
const REGEXP_UNICODE_SETS_SLOT: &str = "[[RegExpUnicodeSets]]";
const REGEXP_STICKY_SLOT: &str = "[[RegExpSticky]]";

fn regexp_internal_slot_key(name: &str) -> crate::value::PrivateSlotKey {
    crate::value::PrivateSlotKey::Internal(Arc::from(name))
}

fn has_regexp_matcher_slot(vm: &Vm, value: &Value) -> bool {
    let Value::Object(idx) = value else {
        return false;
    };
    vm.heap.with_obj(idx.0, |object| {
        let HeapObj::Object(data) = object else {
            return false;
        };
        data.private_fields
            .lock()
            .contains_key(&regexp_internal_slot_key(REGEXP_MATCHER_SLOT))
    })
}

fn regexp_prototype_from_constructor(vm: &mut Vm, new_target: &Value) -> error::Result<Value> {
    let prototype = vm.get_property_by_key(new_target, &PropertyKey::from("prototype"))?;
    if matches!(prototype, Value::Object(_)) {
        return Ok(prototype);
    }
    vm.constructor_realm_default_prototype(
        new_target,
        "RegExp",
        vm.current_realm_regexp_prototype(),
    )
}

pub(crate) fn regexp_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let pattern = args.first().cloned().unwrap_or(Value::Undefined);
    let supplied_flags = args.get(1).cloned().unwrap_or(Value::Undefined);
    let pattern_is_regexp = is_regexp_spec(vm, &pattern)?;
    let constructing = vm.current_native_new_target().is_some();
    let new_target = vm
        .current_native_new_target()
        .or_else(|| vm.current_native_callee())
        .cloned()
        .ok_or_else(|| Error::type_err("RegExp constructor has no active function"))?;

    if !constructing && pattern_is_regexp && supplied_flags.is_undefined() {
        let pattern_constructor = vm.get_property(&pattern, "constructor")?;
        if same_value(&new_target, &pattern_constructor) {
            return Ok(pattern);
        }
    }

    // The extracted values can be fresh getter results. Root each result as
    // soon as it becomes observable because the following getter, prototype
    // lookup, allocation, and ToString operations can all re-enter the VM.
    let mut pin_count = 0;
    let result = (|| {
        let (pattern_source, flags) = if has_regexp_matcher_slot(vm, &pattern) {
            let pattern_source = Value::String(Arc::from(
                read_regexp_source(vm, &Some(pattern.clone()))?.as_str(),
            ));
            let flags = if supplied_flags.is_undefined() {
                Value::String(Arc::from(
                    read_regexp_flags(vm, &Some(pattern.clone()))?.as_str(),
                ))
            } else {
                supplied_flags.clone()
            };
            pin_count += vm.pin(&pattern_source);
            pin_count += vm.pin(&flags);
            (pattern_source, flags)
        } else if pattern_is_regexp {
            let pattern_source = vm.get_property(&pattern, "source")?;
            pin_count += vm.pin(&pattern_source);
            let flags = if supplied_flags.is_undefined() {
                vm.get_property(&pattern, "flags")?
            } else {
                supplied_flags.clone()
            };
            pin_count += vm.pin(&flags);
            (pattern_source, flags)
        } else {
            pin_count += vm.pin(&pattern);
            pin_count += vm.pin(&supplied_flags);
            (pattern.clone(), supplied_flags.clone())
        };

        let prototype = regexp_prototype_from_constructor(vm, &new_target)?;
        let object = regexp_alloc(vm, prototype)?;
        regexp_initialize(vm, object, pattern_source, flags)
    })();
    vm.unpin_many(pin_count);
    result
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
    regexp_create_intrinsic_with_flags(vm, pattern, None)
}

pub(crate) fn regexp_create_literal(
    vm: &mut Vm,
    pattern: &str,
    flags: &str,
) -> error::Result<Value> {
    // This helper is called by an interpreted bytecode opcode. Native
    // builtins can re-enter a function from another Realm while their own
    // callee is still active, so the executing frame is authoritative here.
    let realm = vm.current_interpreted_realm_global_env();
    let proto = vm
        .realm_regexp_prototypes
        .get(&realm.0)
        .cloned()
        .unwrap_or_else(|| vm.regexp_proto.clone());
    create_regexp_object(vm, pattern.to_string(), flags.to_string(), proto)
}

pub(crate) fn regexp_create_intrinsic_with_flags(
    vm: &mut Vm,
    pattern: &Value,
    flags_override: Option<&str>,
) -> error::Result<Value> {
    let flags = flags_override
        .map(|flags| Value::String(Arc::from(flags)))
        .unwrap_or(Value::Undefined);
    let object = regexp_alloc(vm, vm.current_realm_regexp_prototype())?;
    regexp_initialize(vm, object, pattern.clone(), flags)
}

fn create_regexp_object(
    vm: &mut Vm,
    pattern: String,
    flags: String,
    proto: Value,
) -> error::Result<Value> {
    let object = regexp_alloc(vm, proto)?;
    regexp_initialize(
        vm,
        object,
        Value::String(Arc::from(pattern.as_str())),
        Value::String(Arc::from(flags.as_str())),
    )
}

fn regexp_alloc(vm: &mut Vm, proto: Value) -> error::Result<Value> {
    let mut props = IndexMap::new();
    props.insert(
        PropertyKey::from("lastIndex"),
        regexp_last_index_prop(Value::Number(0.0)),
    );
    let private_fields = std::collections::HashMap::from([(
        regexp_internal_slot_key(REGEXP_MATCHER_SLOT),
        crate::value::PrivateSlot::Value(Value::Bool(true)),
    )]);
    let pin_count = vm.pin(&proto);
    let result = vm
        .alloc(HeapObj::Object(crate::value::ObjectData {
            props: Mutex::new(props),
            proto: Mutex::new(Some(proto)),
            extensible: AtomicBool::new(true),
            class_name: Some(Arc::from("RegExp")),
            private_fields: Mutex::new(private_fields),
            primitive: Mutex::new(None),
        }))
        .map(Value::Object);
    vm.unpin_many(pin_count);
    result
}

fn regexp_initialize(
    vm: &mut Vm,
    object: Value,
    pattern: Value,
    flags: Value,
) -> error::Result<Value> {
    let pin_count = vm.pin_many(&[object.clone(), pattern.clone(), flags.clone()]);
    let result = (|| {
        let pattern = if pattern.is_undefined() {
            String::new()
        } else {
            vm.to_string(&pattern)?.to_string()
        };
        let flags = if flags.is_undefined() {
            String::new()
        } else {
            vm.to_string(&flags)?.to_string()
        };

        if flags.contains('u') || flags.contains('v') {
            validate_logical_utf16_source_length(&pattern)
                .map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
        }
        crate::lexer::validate_regex_literal(&pattern, &flags).map_err(Error::syntax)?;
        // Validate the pattern eagerly so bad regexes throw at construction time.
        if flags.contains('u') || flags.contains('v') {
            validate_logical_utf16_construction_limits(&pattern, &flags)
                .map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
        }
        compile_regex_for_input(&pattern, &flags, "")
            .map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
        let Value::Object(object_idx) = object else {
            unreachable!("RegExpAlloc must return an object");
        };
        vm.heap.with_obj(object_idx.0, |o| {
            if let HeapObj::Object(obj) = o {
                let mut private_fields = obj.private_fields.lock();
                private_fields.insert(
                    regexp_internal_slot_key(REGEXP_SOURCE_SLOT),
                    crate::value::PrivateSlot::Value(Value::String(Arc::from(pattern.as_str()))),
                );
                private_fields.insert(
                    regexp_internal_slot_key(REGEXP_FLAGS_SLOT),
                    crate::value::PrivateSlot::Value(Value::String(Arc::from(flags.as_str()))),
                );
                private_fields.insert(
                    regexp_internal_slot_key(REGEXP_HAS_INDICES_SLOT),
                    crate::value::PrivateSlot::Value(Value::Bool(flags.contains('d'))),
                );
                private_fields.insert(
                    regexp_internal_slot_key(REGEXP_GLOBAL_SLOT),
                    crate::value::PrivateSlot::Value(Value::Bool(flags.contains('g'))),
                );
                private_fields.insert(
                    regexp_internal_slot_key(REGEXP_IGNORE_CASE_SLOT),
                    crate::value::PrivateSlot::Value(Value::Bool(flags.contains('i'))),
                );
                private_fields.insert(
                    regexp_internal_slot_key(REGEXP_MULTILINE_SLOT),
                    crate::value::PrivateSlot::Value(Value::Bool(flags.contains('m'))),
                );
                private_fields.insert(
                    regexp_internal_slot_key(REGEXP_DOT_ALL_SLOT),
                    crate::value::PrivateSlot::Value(Value::Bool(flags.contains('s'))),
                );
                private_fields.insert(
                    regexp_internal_slot_key(REGEXP_UNICODE_SLOT),
                    crate::value::PrivateSlot::Value(Value::Bool(flags.contains('u'))),
                );
                private_fields.insert(
                    regexp_internal_slot_key(REGEXP_UNICODE_SETS_SLOT),
                    crate::value::PrivateSlot::Value(Value::Bool(flags.contains('v'))),
                );
                private_fields.insert(
                    regexp_internal_slot_key(REGEXP_STICKY_SLOT),
                    crate::value::PrivateSlot::Value(Value::Bool(flags.contains('y'))),
                );
            }
        });
        set_regexp_last_index(vm, &Value::Object(object_idx), 0.0)?;
        Ok(Value::Object(object_idx))
    })();
    vm.unpin_many(pin_count);
    result
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
    if full_unicode
        && !flags.contains('y')
        && has_unmodified_intrinsic_regexp_exec(vm, &rx)
        && flags == read_regexp_flags(vm, &Some(rx.clone()))?
    {
        set_regexp_last_index(vm, &rx, 0.0)?;
        let result = super::string::regexp_match_internal(vm, rx.clone(), &s)?;
        set_regexp_last_index(vm, &rx, 0.0)?;
        return Ok(result);
    }

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

fn has_unmodified_intrinsic_regexp_exec(vm: &Vm, rx: &Value) -> bool {
    let Value::Object(rx_idx) = rx else {
        return false;
    };
    let prototype = vm.heap.with_obj(rx_idx.0, |object| {
        let HeapObj::Object(data) = object else {
            return None;
        };
        if data.class_name.as_deref() != Some("RegExp")
            || data.props.lock().contains_key(&PropertyKey::from("exec"))
        {
            return None;
        }
        data.proto.lock().clone()
    });
    let Some(Value::Object(prototype_idx)) = prototype else {
        return false;
    };
    if !vm
        .realm_regexp_prototypes
        .values()
        .any(|value| value == &Value::Object(prototype_idx))
    {
        return false;
    }
    let exec = vm.heap.with_obj(prototype_idx.0, |prototype| {
        prototype
            .props()
            .lock()
            .get(&PropertyKey::from("exec"))
            .and_then(|descriptor| (!descriptor.is_accessor).then(|| descriptor.value.clone()))
    });
    let Some(Value::Object(exec_idx)) = exec else {
        return false;
    };
    vm.heap.with_obj(exec_idx.0, |object| {
        matches!(
            object,
            HeapObj::Function(function)
                if matches!(
                    &function.kind,
                    FunctionKind::Native { func, .. }
                        if std::ptr::fn_addr_eq(*func, regexp_exec as NativeFn)
                )
        )
    })
}

pub(crate) fn regexp_symbol_match_all(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let Some(rx @ Value::Object(_)) = this else {
        return Err(Error::type_err("RegExp method called on non-object"));
    };
    let s = vm
        .to_string(args.first().unwrap_or(&Value::Undefined))?
        .to_string();
    let default_constructor = vm.current_realm_regexp_constructor();
    let constructor = regexp_species_constructor(vm, &rx, default_constructor)?;
    let mut pin_count = vm.pin(&constructor);
    let result = (|| {
        let flags_value = vm.get_property(&rx, "flags")?;
        pin_count += vm.pin(&flags_value);
        let flags = vm.to_string(&flags_value)?.to_string();

        let matcher = vm.construct(
            &constructor,
            &[rx.clone(), Value::String(Arc::from(flags.as_str()))],
        )?;
        pin_count += vm.pin(&matcher);
        let last_index_value = vm.get_property(&rx, "lastIndex")?;
        pin_count += vm.pin(&last_index_value);
        let last_index = regexp_to_length(vm, &last_index_value)?;
        set_regexp_last_index(vm, &matcher, last_index)?;
        let global = flags.contains('g');
        let full_unicode = flags.contains('u') || flags.contains('v');
        new_regexp_string_iterator(vm, matcher, Arc::from(s.as_str()), global, full_unicode)
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn regexp_symbol_split(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let Some(rx @ Value::Object(_)) = this else {
        return Err(Error::type_err("RegExp method called on non-object"));
    };

    let mut pin_count = vm.pin(&rx);
    let result = (|| {
        let string = vm
            .to_string(args.first().unwrap_or(&Value::Undefined))?
            .to_string();
        let default_constructor = vm.current_realm_regexp_constructor();
        let constructor = regexp_species_constructor(vm, &rx, default_constructor)?;
        pin_count += vm.pin(&constructor);

        let flags_value = vm.get_property(&rx, "flags")?;
        pin_count += vm.pin(&flags_value);
        let flags = vm.to_string(&flags_value)?.to_string();
        let full_unicode = flags.contains('u') || flags.contains('v');
        let new_flags = if flags.contains('y') {
            flags.clone()
        } else {
            format!("{flags}y")
        };

        let splitter = vm.construct(
            &constructor,
            &[rx.clone(), Value::String(Arc::from(new_flags.as_str()))],
        )?;
        pin_count += vm.pin(&splitter);

        let array = array_create_in_current_realm(vm, 0)?;
        pin_count += vm.pin(&array);
        let limit = match args.get(1) {
            None | Some(Value::Undefined) => u32::MAX as usize,
            Some(value) => crate::vm::to_uint32(vm.to_number(value)?) as usize,
        };
        if limit == 0 {
            return Ok(array);
        }

        if string.is_empty() {
            let match_result = regexp_exec_dispatch(vm, &splitter, &string)?;
            if !match_result.is_null() {
                return Ok(array);
            }
            regexp_split_append(vm, &array, 0, Value::String(Arc::from("")))?;
            return Ok(array);
        }

        let size = crate::value::utf16_len(&string);
        let mut length_a = 0usize;
        let mut last_match_end = 0usize;
        let mut search_index = 0usize;

        while search_index < size {
            vm.consume_fuel()?;
            vm.set_property_strict(&splitter, "lastIndex", Value::Number(search_index as f64))?;
            let match_result = regexp_exec_dispatch(vm, &splitter, &string)?;
            if match_result.is_null() {
                search_index = advance_string_index(&string, search_index, full_unicode);
                continue;
            }

            let match_pin = vm.pin(&match_result);
            let reached_limit: error::Result<bool> = (|| {
                let last_index_value = vm.get_property(&splitter, "lastIndex")?;
                let last_index_pin = vm.pin(&last_index_value);
                let match_end = regexp_to_length(vm, &last_index_value);
                vm.unpin_many(last_index_pin);
                let match_end = (match_end? as usize).min(size);

                if match_end == last_match_end {
                    search_index = advance_string_index(&string, search_index, full_unicode);
                    return Ok(false);
                }

                let substring = crate::value::utf16_slice(&string, last_match_end, search_index);
                regexp_split_append(
                    vm,
                    &array,
                    length_a,
                    Value::String(Arc::from(substring.as_str())),
                )?;
                length_a += 1;
                if length_a == limit {
                    return Ok(true);
                }

                last_match_end = match_end;
                let result_length_value = vm.get_property(&match_result, "length")?;
                let result_length_pin = vm.pin(&result_length_value);
                let result_length = regexp_to_length(vm, &result_length_value);
                vm.unpin_many(result_length_pin);
                let capture_count = (result_length? as usize).saturating_sub(1);

                for capture_index in 1..=capture_count {
                    vm.consume_fuel()?;
                    let key = PropertyKey::from_integer_index(capture_index as u64);
                    let next_capture = vm.get_property_by_key(&match_result, &key)?;
                    let capture_pin = vm.pin(&next_capture);
                    let append_result = regexp_split_append(vm, &array, length_a, next_capture);
                    vm.unpin_many(capture_pin);
                    append_result?;
                    length_a += 1;
                    if length_a == limit {
                        return Ok(true);
                    }
                }
                search_index = last_match_end;
                Ok(false)
            })();
            vm.unpin_many(match_pin);
            if reached_limit? {
                return Ok(array);
            }
        }

        let substring = crate::value::utf16_slice(&string, last_match_end, size);
        regexp_split_append(
            vm,
            &array,
            length_a,
            Value::String(Arc::from(substring.as_str())),
        )?;
        Ok(array)
    })();
    vm.unpin_many(pin_count);
    result
}

fn regexp_split_append(
    vm: &mut Vm,
    array: &Value,
    index: usize,
    value: Value,
) -> error::Result<()> {
    vm.define_data_property(array, PropertyKey::from_integer_index(index as u64), value)
}

fn new_regexp_string_iterator(
    vm: &mut Vm,
    matcher: Value,
    string: Arc<str>,
    global: bool,
    full_unicode: bool,
) -> error::Result<Value> {
    let realm = vm.current_realm_global_env();
    let prototype = vm
        .realm_regexp_string_iterator_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing RegExp String Iterator prototype intrinsic"))?;
    let pin_count = vm.pin_many(&[matcher.clone(), prototype.clone()]);
    let result = vm
        .alloc(HeapObj::RegExpStringIterator(RegExpStringIteratorData {
            matcher,
            string,
            global,
            full_unicode,
            done: AtomicBool::new(false),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(prototype)),
            extensible: AtomicBool::new(true),
        }))
        .map(Value::Object);
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn setup_regexp_string_iterator_proto(vm: &mut Vm) -> error::Result<()> {
    let iterator_base = vm.iterator_base_proto.clone();
    setup_regexp_string_iterator_proto_in_env(vm, vm.global, iterator_base)?;
    Ok(())
}

pub(crate) fn setup_regexp_string_iterator_proto_in_env(
    vm: &mut Vm,
    realm: GcIdx,
    iterator_base: Value,
) -> error::Result<Value> {
    let next_fn = vm.new_native_function_in_env("next", regexp_string_iterator_next, 0, realm)?;
    let roots = [Value::Object(next_fn), iterator_base.clone()];
    let pin_count = vm.pin_many(&roots);
    let result = (|| {
        let proto_idx = vm.alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(iterator_base)),
            extensible: AtomicBool::new(true),
            class_name: Some(Arc::from("RegExp String Iterator")),
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?;
        vm.heap.with_obj(proto_idx.0, |obj| {
            let mut props = obj.props().lock();
            props.insert(PropertyKey::from("next"), data_prop(Value::Object(next_fn)));
            props.insert(
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
                PropertyDescriptor {
                    value: Value::String(Arc::from("RegExp String Iterator")),
                    writable: false,
                    enumerable: false,
                    configurable: true,
                    get: None,
                    set: None,
                    is_accessor: false,
                },
            );
        });
        let prototype = Value::Object(proto_idx);
        vm.realm_regexp_string_iterator_prototypes
            .insert(realm.0, prototype.clone());
        if realm == vm.global {
            vm.regexp_string_iterator_proto = prototype.clone();
        }
        Ok(prototype)
    })();
    vm.unpin_many(pin_count);
    result
}

fn regexp_string_iterator_next(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let Some(Value::Object(iter_idx)) = this else {
        return Err(Error::type_err("Iterator next called on non-iterator"));
    };
    let Some((matcher, string, global, full_unicode, already_done)) =
        vm.heap.with_obj(iter_idx.0, |obj| {
            if let HeapObj::RegExpStringIterator(iter) = obj {
                Some((
                    iter.matcher.clone(),
                    iter.string.clone(),
                    iter.global,
                    iter.full_unicode,
                    iter.done.load(Ordering::Relaxed),
                ))
            } else {
                None
            }
        })
    else {
        return Err(Error::type_err(
            "RegExp String Iterator next called on incompatible receiver",
        ));
    };
    if already_done {
        return gen_result(vm, Value::Undefined, true, false);
    }

    let result = regexp_exec_dispatch(vm, &matcher, &string)?;
    if result.is_null() {
        vm.heap.with_obj(iter_idx.0, |obj| {
            if let HeapObj::RegExpStringIterator(iter) = obj {
                iter.done.store(true, Ordering::Relaxed);
            }
        });
        return gen_result(vm, Value::Undefined, true, false);
    }

    let mut pin_count = vm.pin(&result);
    let outcome = (|| {
        if global {
            let matched_value = vm.get_property(&result, "0")?;
            pin_count += vm.pin(&matched_value);
            let matched = vm.to_string(&matched_value)?.to_string();
            if matched.is_empty() {
                let last_index = vm.get_property(&matcher, "lastIndex")?;
                pin_count += vm.pin(&last_index);
                let this_index = regexp_to_length(vm, &last_index)? as usize;
                let next_index = advance_string_index(&string, this_index, full_unicode);
                set_regexp_last_index(vm, &matcher, next_index as f64)?;
            }
        } else {
            vm.heap.with_obj(iter_idx.0, |obj| {
                if let HeapObj::RegExpStringIterator(iter) = obj {
                    iter.done.store(true, Ordering::Relaxed);
                }
            });
        }
        gen_result(vm, result, false, false)
    })();
    vm.unpin_many(pin_count);
    outcome
}

fn regexp_species_constructor(
    vm: &mut Vm,
    rx: &Value,
    default_constructor: Value,
) -> error::Result<Value> {
    let constructor = vm.get_property(rx, "constructor")?;
    if constructor.is_undefined() {
        return Ok(default_constructor);
    }
    if !matches!(constructor, Value::Object(_)) {
        return Err(Error::type_err("RegExp constructor is not an object"));
    }
    let species_key = PropertyKey::symbol(vm.well_known_symbols.species);
    let species = vm.get_property_by_key(&constructor, &species_key)?;
    if species.is_undefined() || matches!(species, Value::Null) {
        return Ok(default_constructor);
    }
    if !vm.is_constructor_value(&species) {
        return Err(Error::type_err("RegExp species is not a constructor"));
    }
    Ok(species)
}

pub(crate) fn is_regexp_spec(vm: &mut Vm, value: &Value) -> error::Result<bool> {
    let Value::Object(_) = value else {
        return Ok(false);
    };
    let match_key = PropertyKey::symbol(vm.well_known_symbols.r#match);
    let matcher = vm.get_property_by_key(value, &match_key)?;
    if !matcher.is_undefined() {
        return Ok(vm.to_boolean(&matcher));
    }
    Ok(has_regexp_matcher_slot(vm, value))
}

fn regexp_exec_dispatch(vm: &mut Vm, rx: &Value, s: &str) -> error::Result<Value> {
    let exec = vm.get_property(rx, "exec")?;
    if is_callable(&exec, &vm.heap) {
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
    let Some(rx @ Value::Object(_)) = this else {
        return Err(Error::type_err("RegExp method called on non-object"));
    };
    let replace_value = args.get(1).cloned().unwrap_or(Value::Undefined);
    let mut persistent_pins = vm.pin(&rx) + vm.pin(&replace_value);
    let result = (|| {
        let string = regexp_rooted_to_string(vm, args.first().unwrap_or(&Value::Undefined))?;
        let string_length = crate::value::utf16_len(&string);
        let functional_replace = is_callable(&replace_value, &vm.heap);
        let replacement_template = if functional_replace {
            String::new()
        } else {
            regexp_rooted_to_string(vm, &replace_value)?
        };

        let flags_value = vm.get_property(&rx, "flags")?;
        let flags = regexp_rooted_to_string(vm, &flags_value)?;
        let global = flags.contains('g');
        if global {
            set_regexp_last_index(vm, &rx, 0.0)?;
        }

        // The specification collects every result before invoking a replacer.
        // Keep each object rooted because a later exec call may re-enter JS and GC.
        let mut results = Vec::new();
        loop {
            vm.consume_fuel()?;
            let match_result = regexp_exec_dispatch(vm, &rx, &string)?;
            if match_result.is_null() {
                break;
            }
            persistent_pins += vm.pin(&match_result);

            if global {
                let matched_value = vm.get_property(&match_result, "0")?;
                let matched = regexp_rooted_to_string(vm, &matched_value)?;
                if matched.is_empty() {
                    let last_index_value = vm.get_property(&rx, "lastIndex")?;
                    let last_index_pin = vm.pin(&last_index_value);
                    let this_index = regexp_to_length(vm, &last_index_value);
                    vm.unpin_many(last_index_pin);
                    let this_index = this_index? as usize;
                    let full_unicode = flags.contains('u') || flags.contains('v');
                    let next_index = advance_string_index(&string, this_index, full_unicode);
                    set_regexp_last_index(vm, &rx, next_index as f64)?;
                }
            }

            results.push(match_result);
            if !global {
                break;
            }
        }

        let max_captures = MAX_MATERIALIZED_CALL_ARGUMENTS.saturating_sub(3);
        let mut accumulated_result = Vec::<u16>::new();
        let mut next_source_position = 0usize;

        for match_result in &results {
            vm.consume_fuel()?;
            let result_length_value = vm.get_property(match_result, "length")?;
            let result_length_pin = vm.pin(&result_length_value);
            let result_length = regexp_to_length(vm, &result_length_value);
            vm.unpin_many(result_length_pin);
            let result_length = result_length?;
            let captures_count = (result_length - 1.0).max(0.0);
            if captures_count > max_captures as f64 {
                return Err(Error::range("RegExp replacement capture list is too large"));
            }
            let captures_count = captures_count as usize;

            let matched_value = vm.get_property(match_result, "0")?;
            let matched = regexp_rooted_to_string(vm, &matched_value)?;
            let match_length = crate::value::utf16_len(&matched);

            let index_value = vm.get_property(match_result, "index")?;
            let index_pin = vm.pin(&index_value);
            let raw_position = regexp_to_integer_or_infinity(vm, &index_value);
            vm.unpin_many(index_pin);
            let raw_position = raw_position?;
            let position = if raw_position <= 0.0 {
                0
            } else if raw_position >= string_length as f64 {
                string_length
            } else {
                raw_position as usize
            };

            let mut captures = Vec::with_capacity(captures_count);
            for capture_number in 1..=captures_count {
                vm.consume_fuel()?;
                let key = PropertyKey::from_integer_index(capture_number as u64);
                let capture_value = vm.get_property_by_key(match_result, &key)?;
                if capture_value.is_undefined() {
                    captures.push(None);
                } else {
                    captures.push(Some(regexp_rooted_to_string(vm, &capture_value)?));
                }
            }

            let named_captures = vm.get_property(match_result, "groups")?;
            let named_captures_pin = vm.pin(&named_captures);
            let replacement = (|| {
                if functional_replace {
                    let mut replacer_args = Vec::with_capacity(captures_count + 4);
                    replacer_args.push(Value::String(Arc::from(matched.as_str())));
                    for capture in &captures {
                        replacer_args.push(match capture {
                            Some(capture) => Value::String(Arc::from(capture.as_str())),
                            None => Value::Undefined,
                        });
                    }
                    replacer_args.push(Value::Number(position as f64));
                    replacer_args.push(Value::String(Arc::from(string.as_str())));
                    if !named_captures.is_undefined() {
                        replacer_args.push(named_captures.clone());
                    }
                    if replacer_args.len() > MAX_MATERIALIZED_CALL_ARGUMENTS {
                        return Err(Error::range("argument list too large"));
                    }
                    let replacement = vm.call_function(&replace_value, &replacer_args, None)?;
                    regexp_rooted_to_string(vm, &replacement)
                } else {
                    let named_captures_object = if named_captures.is_undefined() {
                        None
                    } else {
                        if named_captures.is_null() {
                            return Err(Error::type_err("RegExp match groups cannot be null"));
                        }
                        Some(vm.to_object(&named_captures)?)
                    };
                    let object_pin = named_captures_object
                        .as_ref()
                        .map_or(0, |object| vm.pin(object));
                    let substitution = regexp_get_substitution(
                        vm,
                        &matched,
                        &string,
                        position,
                        &captures,
                        named_captures_object.as_ref(),
                        &replacement_template,
                    );
                    vm.unpin_many(object_pin);
                    substitution
                }
            })();
            vm.unpin_many(named_captures_pin);
            let replacement = replacement?;

            if position >= next_source_position {
                let preceding = crate::value::utf16_slice(&string, next_source_position, position);
                regexp_append_utf16(vm, &mut accumulated_result, &preceding)?;
                regexp_append_utf16(vm, &mut accumulated_result, &replacement)?;
                next_source_position = position.saturating_add(match_length);
            }
        }

        if next_source_position < string_length {
            let tail = crate::value::utf16_slice(&string, next_source_position, string_length);
            regexp_append_utf16(vm, &mut accumulated_result, &tail)?;
        }
        let output = crate::value::utf16_to_string(&accumulated_result);
        Ok(Value::String(Arc::from(output.as_str())))
    })();
    vm.unpin_many(persistent_pins);
    result
}

fn regexp_rooted_to_string(vm: &mut Vm, value: &Value) -> error::Result<String> {
    let pin_count = vm.pin(value);
    let result = vm.to_string(value).map(|string| string.to_string());
    vm.unpin_many(pin_count);
    result
}

fn regexp_to_integer_or_infinity(vm: &mut Vm, value: &Value) -> error::Result<f64> {
    let number = vm.to_number(value)?;
    if number.is_nan() {
        return Ok(0.0);
    }
    if number == 0.0 || number.is_infinite() {
        return Ok(number);
    }
    Ok(number.trunc())
}

fn regexp_append_utf16(vm: &mut Vm, output: &mut Vec<u16>, value: &str) -> error::Result<()> {
    for ch in value.chars() {
        if let Some(unit) = crate::value::utf16_single_unit_from_internal_char(ch) {
            vm.consume_fuel()?;
            output.push(unit);
            continue;
        }
        let mut encoded = [0; 2];
        for unit in ch.encode_utf16(&mut encoded) {
            vm.consume_fuel()?;
            output.push(*unit);
        }
    }
    Ok(())
}

fn regexp_get_substitution(
    vm: &mut Vm,
    matched: &str,
    string: &str,
    position: usize,
    captures: &[Option<String>],
    named_captures: Option<&Value>,
    replacement_template: &str,
) -> error::Result<String> {
    let string_length = crate::value::utf16_len(string);
    let match_length = crate::value::utf16_len(matched);
    let mut result = String::new();
    let mut offset = 0usize;

    while offset < replacement_template.len() {
        vm.consume_fuel()?;
        let remainder = &replacement_template[offset..];
        if remainder.starts_with("$$") {
            result.push('$');
            offset += 2;
        } else if remainder.starts_with("$`") {
            result.push_str(&crate::value::utf16_slice(string, 0, position));
            offset += 2;
        } else if remainder.starts_with("$&") {
            result.push_str(matched);
            offset += 2;
        } else if remainder.starts_with("$'") {
            let tail_position = position.saturating_add(match_length).min(string_length);
            result.push_str(&crate::value::utf16_slice(
                string,
                tail_position,
                string_length,
            ));
            offset += 2;
        } else if remainder.starts_with('$')
            && remainder.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
        {
            let bytes = remainder.as_bytes();
            let first = (bytes[1] - b'0') as usize;
            let mut digit_count = if bytes.get(2).is_some_and(u8::is_ascii_digit) {
                2
            } else {
                1
            };
            let mut capture_index = if digit_count == 2 {
                first * 10 + (bytes[2] - b'0') as usize
            } else {
                first
            };
            if capture_index > captures.len() && digit_count == 2 {
                digit_count = 1;
                capture_index = first;
            }
            let reference_length = 1 + digit_count;
            if (1..=captures.len()).contains(&capture_index) {
                if let Some(capture) = &captures[capture_index - 1] {
                    result.push_str(capture);
                }
            } else {
                result.push_str(&remainder[..reference_length]);
            }
            offset += reference_length;
        } else if let Some(group_remainder) = remainder.strip_prefix("$<") {
            if let (Some(named_captures), Some(close_offset)) =
                (named_captures, group_remainder.find('>'))
            {
                let reference_length = 2 + close_offset + 1;
                let group_name = &group_remainder[..close_offset];
                let capture = vm.get_property(named_captures, group_name)?;
                if !capture.is_undefined() {
                    result.push_str(&regexp_rooted_to_string(vm, &capture)?);
                }
                offset += reference_length;
            } else {
                result.push_str("$<");
                offset += 2;
            }
        } else {
            let ch = remainder
                .chars()
                .next()
                .expect("non-empty replacement remainder");
            result.push(ch);
            offset += ch.len_utf8();
        }
    }

    Ok(result)
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
            if let Some(value) = read_regexp_private_bool(vm, idx, slot_name) {
                return Ok(Value::Bool(value));
            }
            if is_current_realm_regexp_prototype(vm, idx) {
                return Ok(Value::Undefined);
            }
            Err(Error::type_err(
                "RegExp getter called on incompatible receiver",
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

struct RegExpBackendInput<'a> {
    text: std::borrow::Cow<'a, str>,
    byte_to_utf16: Option<Vec<(usize, usize)>>,
    logical_unicode: bool,
}

impl RegExpBackendInput<'_> {
    fn as_str(&self) -> &str {
        self.text.as_ref()
    }

    fn utf16_len(&self) -> usize {
        self.byte_to_utf16
            .as_ref()
            .and_then(|boundaries| boundaries.last().map(|(_, offset)| *offset))
            .unwrap_or_else(|| crate::value::utf16_len(self.as_str()))
    }

    fn byte_index_for_utf16(&self, offset: usize) -> Option<usize> {
        if self.logical_unicode {
            let mut chars = self.as_str().char_indices().peekable();
            let mut utf16 = 0usize;
            while let Some((byte, ch)) = chars.next() {
                if utf16 == offset {
                    return Some(byte);
                }
                let unit = crate::value::utf16_single_unit_from_internal_char(ch);
                let width = if unit.is_some_and(|unit| (0xd800..=0xdbff).contains(&unit))
                    && chars.peek().is_some_and(|(_, next)| {
                        crate::value::utf16_single_unit_from_internal_char(*next)
                            .is_some_and(|unit| (0xdc00..=0xdfff).contains(&unit))
                    }) {
                    chars.next();
                    2
                } else {
                    unit.map_or_else(|| ch.len_utf16(), |_| 1)
                };
                if utf16 < offset && offset < utf16 + width {
                    return Some(byte);
                }
                utf16 += width;
            }
            return (utf16 == offset).then_some(self.as_str().len());
        }
        if let Some(boundaries) = &self.byte_to_utf16 {
            return match boundaries.binary_search_by_key(&offset, |(_, utf16)| *utf16) {
                Ok(index) => Some(boundaries[index].0),
                Err(index) if index > 0 && index < boundaries.len() => {
                    Some(boundaries[index - 1].0)
                }
                Err(_) => None,
            };
        }
        if let Some(byte) = crate::value::utf16_index_to_byte(self.as_str(), offset) {
            return Some(byte);
        }
        let mut utf16 = 0usize;
        for (byte, ch) in self.as_str().char_indices() {
            let width = if crate::value::utf16_single_unit_from_internal_char(ch).is_some() {
                1
            } else {
                ch.len_utf16()
            };
            if utf16 < offset && offset < utf16 + width {
                return Some(byte);
            }
            utf16 += width;
        }
        None
    }

    fn utf16_offset_for_byte(&self, byte: usize) -> Option<usize> {
        if let Some(boundaries) = &self.byte_to_utf16 {
            return boundaries
                .binary_search_by_key(&byte, |(boundary, _)| *boundary)
                .ok()
                .map(|index| boundaries[index].1);
        }
        self.as_str()
            .is_char_boundary(byte)
            .then(|| crate::value::utf16_len(&self.as_str()[..byte]))
    }
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
    let re = if flags.contains('u') || flags.contains('v') {
        compile_regex_for_input(&source, &flags, &input)
    } else {
        // The non-Unicode matcher runs on a sentinel-backed UTF-16 view so
        // lastIndex may address either half of a supplementary code point.
        compile_regex_for_code_units(&source, &flags)
    }
    .map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
    let backend_input = regexp_backend_input(
        vm,
        &input,
        &flags,
        matches!(re, CompiledRegex::LogicalUtf16(_)),
    )?;
    let capture_names = regex_capture_names(&source, &flags).map_err(Error::syntax)?;
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
    let utf16_len = backend_input.utf16_len();
    if start > utf16_len {
        if global || sticky {
            if let Some(value) = &this_value {
                set_regexp_last_index(vm, value, 0.0)?;
            }
        }
        return Ok(Value::Null);
    }
    let Some(start_byte) = backend_input.byte_index_for_utf16(start) else {
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
    let m = if sticky {
        re.captures_exact_at(backend_input.as_str(), start_byte)?
    } else {
        re.captures_at(backend_input.as_str(), start_byte)?
    };
    match m {
        Some(caps) => {
            let capture_ranges = regexp_capture_index_pairs(&caps, &backend_input);
            let items: Vec<Value> = capture_ranges
                .iter()
                .map(|range| match range {
                    Some((start, end)) => Value::String(Arc::from(
                        crate::value::utf16_slice(&input, *start, *end).as_str(),
                    )),
                    None => Value::Undefined,
                })
                .collect();
            if global || sticky {
                let match_end = capture_ranges
                    .first()
                    .copied()
                    .flatten()
                    .map(|(_, end)| end)
                    .unwrap_or(start);
                if let Some(value) = &this_value {
                    set_regexp_last_index(vm, value, match_end as f64)?;
                }
            }
            let match_start = capture_ranges
                .first()
                .copied()
                .flatten()
                .map(|(start, _)| start)
                .unwrap_or(start);
            let result = make_regexp_exec_array(vm, items)?;
            let result_pin = vm.pin(&result);
            let completion = (|| {
                let groups = make_regexp_groups_object_from_ranges(
                    vm,
                    &input,
                    &capture_ranges,
                    &capture_names,
                )?;
                let groups_pin = vm.pin(&groups);
                let completion = (|| {
                    let indices = flags
                        .contains('d')
                        .then(|| make_regexp_indices_array(vm, &capture_ranges, &capture_names))
                        .transpose()?;
                    add_regexp_exec_result_props(
                        vm,
                        &result,
                        match_start,
                        &input,
                        groups,
                        indices,
                    )?;
                    Ok(result)
                })();
                vm.unpin_many(groups_pin);
                completion
            })();
            vm.unpin_many(result_pin);
            completion
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

fn make_regexp_exec_array(vm: &mut Vm, items: Vec<Value>) -> error::Result<Value> {
    // Native RegExp captures contain only strings and undefined, so this
    // operation can retry GC without exposing unrooted object-valued locals.
    debug_assert!(items.iter().all(|value| !matches!(value, Value::Object(_))));
    let prototype = vm.array_prototype_for_env(vm.current_realm_global_env());
    vm.alloc(HeapObj::Array(ArrayData::new(items, Some(prototype))))
        .map(Value::Object)
}

fn regexp_capture_index_pairs(
    caps: &CompiledCaptures<'_>,
    backend_input: &RegExpBackendInput<'_>,
) -> Vec<Option<(usize, usize)>> {
    let byte_ranges: Vec<Option<(usize, usize)>> = caps
        .iter()
        .map(|capture| capture.map(|matched| (matched.start(), matched.end())))
        .collect();
    let mut endpoints: Vec<usize> = byte_ranges
        .iter()
        .flatten()
        .flat_map(|(start, end)| [*start, *end])
        .collect();
    endpoints.sort_unstable();
    endpoints.dedup();

    if backend_input.byte_to_utf16.is_some() {
        return byte_ranges
            .into_iter()
            .map(|range| {
                range.map(|(start, end)| {
                    (
                        backend_input
                            .utf16_offset_for_byte(start)
                            .expect("capture start must map to a UTF-16 offset"),
                        backend_input
                            .utf16_offset_for_byte(end)
                            .expect("capture end must map to a UTF-16 offset"),
                    )
                })
            })
            .collect();
    }

    // Convert all capture boundaries in one left-to-right pass. Re-scanning
    // the input once per capture makes a large, attacker-controlled pattern
    // quadratic in the number of captures and input length.
    let mut utf16_offsets = std::collections::HashMap::with_capacity(endpoints.len());
    let mut previous_byte = 0usize;
    let mut previous_utf16 = 0usize;
    for endpoint in endpoints {
        debug_assert!(backend_input.as_str().is_char_boundary(endpoint));
        previous_utf16 += crate::value::utf16_len(&backend_input.as_str()[previous_byte..endpoint]);
        utf16_offsets.insert(endpoint, previous_utf16);
        previous_byte = endpoint;
    }

    byte_ranges
        .into_iter()
        .map(|range| {
            range.map(|(start, end)| {
                (
                    *utf16_offsets
                        .get(&start)
                        .expect("capture start must have a UTF-16 offset"),
                    *utf16_offsets
                        .get(&end)
                        .expect("capture end must have a UTF-16 offset"),
                )
            })
        })
        .collect()
}

fn make_regexp_groups_object_from_ranges(
    vm: &mut Vm,
    input: &str,
    ranges: &[Option<(usize, usize)>],
    names: &[RegexCaptureName],
) -> error::Result<Value> {
    if names.is_empty() {
        return Ok(Value::Undefined);
    }
    let groups = vm.alloc(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(None),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Object")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    vm.heap.with_obj(groups.0, |object| {
        let props = object.props();
        let mut props = props.lock();
        let mut matched_names = IndexSet::new();
        for capture in names {
            let value = ranges
                .get(capture.index)
                .copied()
                .flatten()
                .map(|(start, end)| {
                    Value::String(Arc::from(
                        crate::value::utf16_slice(input, start, end).as_str(),
                    ))
                })
                .unwrap_or(Value::Undefined);
            if matched_names.contains(&capture.name) {
                debug_assert!(value.is_undefined());
                continue;
            }
            if !value.is_undefined() {
                matched_names.insert(capture.name.clone());
            }
            props.insert(
                PropertyKey::from(capture.name.clone()),
                PropertyDescriptor::data(value),
            );
        }
    });
    Ok(Value::Object(groups))
}

fn make_regexp_indices_array(
    vm: &mut Vm,
    pairs: &[Option<(usize, usize)>],
    capture_names: &[RegexCaptureName],
) -> error::Result<Value> {
    let prototype = vm.array_prototype_for_env(vm.current_realm_global_env());
    let mut pair_values = Vec::with_capacity(pairs.len());
    let mut pin_count = 0usize;
    let completion = (|| {
        for pair in pairs {
            vm.consume_fuel()?;
            let value = match pair {
                Some((start, end)) => {
                    let pair = vm.alloc(HeapObj::Array(ArrayData::new(
                        vec![Value::Number(*start as f64), Value::Number(*end as f64)],
                        Some(prototype.clone()),
                    )))?;
                    let value = Value::Object(pair);
                    pin_count += vm.pin(&value);
                    value
                }
                None => Value::Undefined,
            };
            pair_values.push(value);
        }

        let groups = if capture_names.is_empty() {
            Value::Undefined
        } else {
            let groups = vm.alloc(HeapObj::Object(ObjectData {
                props: Mutex::new(IndexMap::new()),
                proto: Mutex::new(None),
                extensible: AtomicBool::new(true),
                class_name: Some(Arc::from("Object")),
                private_fields: Mutex::new(std::collections::HashMap::new()),
                primitive: Mutex::new(None),
            }))?;
            let groups = Value::Object(groups);
            pin_count += vm.pin(&groups);
            let Value::Object(groups_idx) = groups else {
                unreachable!("indices groups allocation must return an object");
            };
            vm.heap.with_obj(groups_idx.0, |object| {
                let props = object.props();
                let mut props = props.lock();
                let mut matched_names = IndexSet::new();
                for capture in capture_names {
                    if let Some(value) = pair_values.get(capture.index) {
                        if matched_names.contains(&capture.name) {
                            debug_assert!(value.is_undefined());
                            continue;
                        }
                        if !value.is_undefined() {
                            matched_names.insert(capture.name.clone());
                        }
                        props.insert(
                            PropertyKey::from(capture.name.clone()),
                            enumerable_data_prop(value.clone()),
                        );
                    }
                }
            });
            Value::Object(groups_idx)
        };

        let indices = ArrayData::new(pair_values, Some(prototype));
        indices
            .props
            .lock()
            .insert(PropertyKey::from("groups"), enumerable_data_prop(groups));
        vm.alloc(HeapObj::Array(indices)).map(Value::Object)
    })();
    vm.unpin_many(pin_count);
    completion
}

fn regexp_backend_input<'a>(
    vm: &mut Vm,
    input: &'a str,
    flags: &str,
    preserve_logical_utf16: bool,
) -> error::Result<RegExpBackendInput<'a>> {
    if flags.contains('u') || flags.contains('v') {
        if preserve_logical_utf16 {
            let mut chars = input.char_indices().peekable();
            while let Some((_, ch)) = chars.next() {
                vm.consume_fuel()?;
                let unit = crate::value::utf16_single_unit_from_internal_char(ch);
                if unit.is_some_and(|unit| (0xd800..=0xdbff).contains(&unit))
                    && chars.peek().is_some_and(|(_, next)| {
                        crate::value::utf16_single_unit_from_internal_char(*next)
                            .is_some_and(|unit| (0xdc00..=0xdfff).contains(&unit))
                    })
                {
                    chars.next();
                }
            }
            return Ok(RegExpBackendInput {
                text: std::borrow::Cow::Borrowed(input),
                byte_to_utf16: None,
                logical_unicode: true,
            });
        }

        let mut previous_high_surrogate = false;
        let has_split_surrogate_pair = input.chars().any(|ch| {
            let unit = crate::value::utf16_single_unit_from_internal_char(ch);
            let found = previous_high_surrogate
                && unit.is_some_and(|unit| (0xdc00..=0xdfff).contains(&unit));
            previous_high_surrogate = unit.is_some_and(|unit| (0xd800..=0xdbff).contains(&unit));
            found
        });
        if !has_split_surrogate_pair {
            return Ok(RegExpBackendInput {
                text: std::borrow::Cow::Borrowed(input),
                byte_to_utf16: None,
                logical_unicode: false,
            });
        }

        let units = crate::value::utf16_from_str(input);
        let mut backend = String::new();
        let mut boundaries = vec![(0usize, 0usize)];
        let mut index = 0usize;
        while index < units.len() {
            vm.consume_fuel()?;
            let unit = units[index];
            if (0xd800..=0xdbff).contains(&unit)
                && units
                    .get(index + 1)
                    .is_some_and(|low| (0xdc00..=0xdfff).contains(low))
            {
                let low = units[index + 1];
                let scalar = 0x10000 + (((unit as u32 - 0xd800) << 10) | (low as u32 - 0xdc00));
                backend.push(char::from_u32(scalar).expect("valid surrogate pair scalar"));
                index += 2;
            } else {
                backend.push_str(crate::value::utf16_to_string(&[unit]).as_str());
                index += 1;
            }
            boundaries.push((backend.len(), index));
        }
        return Ok(RegExpBackendInput {
            text: std::borrow::Cow::Owned(backend),
            byte_to_utf16: Some(boundaries),
            logical_unicode: false,
        });
    }

    // Sentinel-backed code units make every legal non-Unicode lastIndex a
    // backend string boundary without changing the original JS String.
    if input
        .chars()
        .all(|ch| crate::value::utf16_single_unit_from_internal_char(ch).is_some())
    {
        return Ok(RegExpBackendInput {
            text: std::borrow::Cow::Borrowed(input),
            byte_to_utf16: None,
            logical_unicode: false,
        });
    }

    let mut backend = String::new();
    for unit in crate::value::utf16_from_str(input) {
        vm.consume_fuel()?;
        backend.push_str(crate::value::utf16_to_string(&[unit]).as_str());
    }
    Ok(RegExpBackendInput {
        text: std::borrow::Cow::Owned(backend),
        byte_to_utf16: None,
        logical_unicode: false,
    })
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
    indices: Option<Value>,
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
        if let Some(indices) = indices {
            props.insert(PropertyKey::from("indices"), enumerable_data_prop(indices));
        }
    });
    Ok(())
}

fn set_regexp_last_index(vm: &mut Vm, target: &Value, value: f64) -> error::Result<()> {
    vm.set_property_strict(target, "lastIndex", Value::Number(value))
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
        let key = regexp_internal_slot_key(slot_name);
        obj.private_fields
            .lock()
            .get(&key)
            .and_then(|slot| match slot {
                crate::value::PrivateSlot::Value(value @ Value::String(_)) => Some(value.clone()),
                crate::value::PrivateSlot::Value(_)
                | crate::value::PrivateSlot::Method(_)
                | crate::value::PrivateSlot::Accessor { .. } => None,
            })
    })
}

fn read_regexp_private_bool(vm: &mut Vm, idx: GcIdx, slot_name: &str) -> Option<bool> {
    vm.heap.with_obj(idx.0, |o| {
        let HeapObj::Object(obj) = o else {
            return None;
        };
        let key = regexp_internal_slot_key(slot_name);
        obj.private_fields
            .lock()
            .get(&key)
            .and_then(|slot| match slot {
                crate::value::PrivateSlot::Value(Value::Bool(value)) => Some(*value),
                crate::value::PrivateSlot::Value(_)
                | crate::value::PrivateSlot::Method(_)
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
    let closure = vm.native_callee_closure().unwrap_or(vm.global);
    let realm = crate::environment::global_env_root(&vm.heap, closure);
    matches!(
        vm.realm_regexp_prototypes.get(&realm.0),
        Some(Value::Object(prototype)) if *prototype == value
    )
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
    let resumed = if is_lazy {
        let resume = _args.first().cloned().unwrap_or(Value::Undefined);
        vm.resume_generator(GcIdx(g_idx), crate::vm::ResumeKind::Next(resume))
    } else {
        // Legacy eager generator (kept for safety).
        Ok(vm.heap.with_obj(g_idx, |o| {
            if let HeapObj::Generator(g) = o {
                let state = g.state.lock();
                let idx = g.ip.load(Ordering::Relaxed);
                if idx < state.len() {
                    g.ip.store(idx + 1, Ordering::Relaxed);
                    (state[idx].clone(), false, false, false)
                } else {
                    g.done.store(true, Ordering::Relaxed);
                    (Value::Undefined, true, false, false)
                }
            } else {
                (Value::Undefined, true, false, false)
            }
        }))
    };
    complete_generator_resume(vm, GcIdx(g_idx), resumed, is_async_gen)
}

fn validate_async_generator_receiver(
    vm: &mut Vm,
    this: Option<Value>,
) -> error::Result<std::result::Result<GcIdx, Value>> {
    if let Some(Value::Object(idx)) = this {
        let is_async_generator = vm.heap.with_obj(
            idx.0,
            |obj| matches!(obj, HeapObj::LazyGenerator(generator) if generator.is_async),
        );
        if is_async_generator {
            return Ok(Ok(idx));
        }
    }
    Ok(Err(wrap_generator_error(
        vm,
        Error::type_err("AsyncGenerator method called on incompatible receiver"),
        true,
    )?))
}

fn async_generator_realm(vm: &Vm, generator: GcIdx) -> GcIdx {
    let closure = vm.heap.with_obj(generator.0, |object| {
        if let HeapObj::LazyGenerator(data) = object {
            Some(data.closure)
        } else {
            None
        }
    });
    closure
        .map(|env| crate::environment::global_env_root(&vm.heap, env))
        .unwrap_or(vm.global)
}

pub(crate) fn async_generator_next(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    match validate_async_generator_receiver(vm, this)? {
        Ok(generator) => enqueue_async_generator_request(
            vm,
            generator,
            crate::value::AsyncGeneratorRequestKind::Next(
                args.first().cloned().unwrap_or(Value::Undefined),
            ),
        ),
        Err(rejected) => Ok(rejected),
    }
}

fn enqueue_async_generator_request(
    vm: &mut Vm,
    generator: GcIdx,
    kind: crate::value::AsyncGeneratorRequestKind,
) -> error::Result<Value> {
    let generator_value = Value::Object(generator);
    let generator_pin = vm.pin(&generator_value);
    let constructor = vm.current_realm_promise_constructor();
    let capability = match crate::builtins::new_promise_capability(vm, constructor) {
        Ok(capability) => capability,
        Err(error) => {
            vm.unpin(generator_pin);
            return Err(error);
        }
    };
    let promise = capability.promise.clone();
    let request = crate::value::AsyncGeneratorRequest {
        kind,
        capability: crate::value::PromiseReactionCapability {
            promise: capability.promise,
            resolve: capability.resolve,
            reject: capability.reject,
        },
    };
    let should_process = vm.heap.with_obj(generator.0, |obj| {
        let HeapObj::LazyGenerator(data) = obj else {
            return false;
        };
        data.async_queue.lock().push_back(request);
        !data.async_processing.swap(true, Ordering::AcqRel)
    });
    if should_process {
        let result = process_async_generator_queue(vm, generator);
        vm.unpin(generator_pin);
        result?;
    } else {
        vm.unpin(generator_pin);
    }
    Ok(promise)
}

fn abort_async_generator_host_error(vm: &mut Vm, generator: GcIdx) {
    let schedule_drain = vm.heap.with_obj(generator.0, |obj| {
        let HeapObj::LazyGenerator(data) = obj else {
            return false;
        };
        data.started.store(true, Ordering::Release);
        data.done.store(true, Ordering::Release);
        data.delegating.store(false, Ordering::Release);
        data.async_suspended_yield.store(false, Ordering::Release);
        data.async_delegate_await_kind.store(0, Ordering::Release);
        if data.async_processing.swap(false, Ordering::AcqRel) {
            let mut queue = data.async_queue.lock();
            queue.pop_front();
            !queue.is_empty()
        } else {
            false
        }
    });
    if schedule_drain {
        vm.microtask_queue
            .push_back(crate::vm::Microtask::AsyncGeneratorDrain { generator });
    }
}

fn reschedule_async_generator_after_error(vm: &mut Vm, generator: GcIdx) {
    let schedule_drain = vm.heap.with_obj(generator.0, |obj| {
        let HeapObj::LazyGenerator(data) = obj else {
            return false;
        };
        data.async_processing.swap(false, Ordering::AcqRel) && !data.async_queue.lock().is_empty()
    });
    if schedule_drain {
        vm.microtask_queue
            .push_back(crate::vm::Microtask::AsyncGeneratorDrain { generator });
    }
}

fn can_retry_terminal_async_generator_next(vm: &Vm, generator: GcIdx) -> bool {
    vm.heap.with_obj(generator.0, |obj| {
        let HeapObj::LazyGenerator(data) = obj else {
            return false;
        };
        if !data.done.load(Ordering::Acquire) {
            return false;
        }
        let queue = data.async_queue.lock();
        matches!(
            queue.front(),
            Some(crate::value::AsyncGeneratorRequest {
                kind: crate::value::AsyncGeneratorRequestKind::Next(_),
                ..
            })
        )
    })
}

pub(crate) fn drain_async_generator_queue(vm: &mut Vm, generator: GcIdx) -> error::Result<()> {
    let generator_pin = vm.pin(&Value::Object(generator));
    let should_process = vm.heap.with_obj(generator.0, |obj| {
        let HeapObj::LazyGenerator(data) = obj else {
            return false;
        };
        !data.async_queue.lock().is_empty() && !data.async_processing.swap(true, Ordering::AcqRel)
    });
    let result = if should_process {
        process_async_generator_queue(vm, generator)
    } else {
        Ok(())
    };
    vm.unpin(generator_pin);
    result
}

fn process_async_generator_queue(vm: &mut Vm, generator: GcIdx) -> error::Result<()> {
    let retry_terminal_next = can_retry_terminal_async_generator_next(vm, generator);
    let result = process_async_generator_queue_inner(vm, generator);
    if let Err(error) = &result {
        // Only a terminal `next()` completion is replayable. Other paths may
        // already have advanced generator bytecode before returning the error.
        if error.catchable()
            && retry_terminal_next
            && can_retry_terminal_async_generator_next(vm, generator)
        {
            reschedule_async_generator_after_error(vm, generator);
        } else {
            abort_async_generator_host_error(vm, generator);
        }
    }
    result
}

fn process_async_generator_queue_inner(vm: &mut Vm, generator: GcIdx) -> error::Result<()> {
    loop {
        let request = vm.heap.with_obj(generator.0, |obj| {
            if let HeapObj::LazyGenerator(data) = obj {
                data.async_queue.lock().front().cloned()
            } else {
                None
            }
        });
        let Some(request) = request else {
            vm.heap.with_obj(generator.0, |obj| {
                if let HeapObj::LazyGenerator(data) = obj {
                    data.async_processing.store(false, Ordering::Release);
                }
            });
            return Ok(());
        };

        let (started, done, suspended_yield, delegating) = vm.heap.with_obj(generator.0, |obj| {
            if let HeapObj::LazyGenerator(data) = obj {
                (
                    data.started.load(Ordering::Acquire),
                    data.done.load(Ordering::Acquire),
                    data.async_suspended_yield.load(Ordering::Acquire),
                    data.delegating.load(Ordering::Acquire),
                )
            } else {
                (true, true, false, false)
            }
        });

        match request.kind {
            crate::value::AsyncGeneratorRequestKind::Next(value) => {
                if done {
                    finish_async_generator_request(vm, generator, Value::Undefined, true, false)?;
                    continue;
                }
                set_async_generator_suspended_yield(vm, generator, false);
                let resumed = vm.resume_generator(generator, crate::vm::ResumeKind::Next(value));
                if handle_async_generator_resume(vm, generator, resumed, None)? {
                    return Ok(());
                }
            }
            crate::value::AsyncGeneratorRequestKind::Return(value) => {
                if done || !started {
                    begin_async_generator_await(
                        vm,
                        generator,
                        value,
                        crate::value::AsyncGeneratorAwaitKind::ResolveReturn,
                    )?;
                    return Ok(());
                }
                if suspended_yield {
                    set_async_generator_suspended_yield(vm, generator, false);
                    let await_kind = if delegating {
                        crate::value::AsyncGeneratorAwaitKind::ResumeReturnDelegated
                    } else {
                        crate::value::AsyncGeneratorAwaitKind::ResumeReturn
                    };
                    begin_async_generator_await(vm, generator, value, await_kind)?;
                    return Ok(());
                }
                let resumed = vm.resume_generator(generator, crate::vm::ResumeKind::Return(value));
                if handle_async_generator_resume(vm, generator, resumed, None)? {
                    return Ok(());
                }
            }
            crate::value::AsyncGeneratorRequestKind::Throw(reason) => {
                if done || !started {
                    vm.heap.with_obj(generator.0, |obj| {
                        if let HeapObj::LazyGenerator(data) = obj {
                            data.started.store(true, Ordering::Release);
                            data.done.store(true, Ordering::Release);
                        }
                    });
                    finish_async_generator_request(vm, generator, reason, true, true)?;
                    continue;
                }
                set_async_generator_suspended_yield(vm, generator, false);
                let resumed = vm.resume_generator(generator, crate::vm::ResumeKind::Throw(reason));
                if handle_async_generator_resume(vm, generator, resumed, None)? {
                    return Ok(());
                }
            }
        }
    }
}

fn set_async_generator_suspended_yield(vm: &Vm, generator: GcIdx, value: bool) {
    vm.heap.with_obj(generator.0, |obj| {
        if let HeapObj::LazyGenerator(data) = obj {
            data.async_suspended_yield.store(value, Ordering::Release);
        }
    });
}

/// Returns true when processing suspended on an Await job.
fn handle_async_generator_resume(
    vm: &mut Vm,
    generator: GcIdx,
    resumed: error::Result<(Value, bool, bool, bool)>,
    source: Option<crate::value::AsyncGeneratorAwaitKind>,
) -> error::Result<bool> {
    let (value, done, forwarded_result, awaiting) = match resumed {
        Ok(result) => result,
        Err(error) => {
            let reason = generator_error_reason(vm, generator, &error)?;
            finish_async_generator_request(vm, generator, reason, true, true)?;
            return Ok(false);
        }
    };

    if awaiting {
        let delegate_await = vm.heap.with_obj(generator.0, |object| {
            matches!(object, HeapObj::LazyGenerator(data) if data.async_delegate_await_kind.load(Ordering::Acquire) != 0)
        });
        begin_async_generator_await(
            vm,
            generator,
            value,
            if delegate_await {
                crate::value::AsyncGeneratorAwaitKind::ResumeDelegate
            } else {
                crate::value::AsyncGeneratorAwaitKind::Resume
            },
        )?;
        return Ok(true);
    }

    if forwarded_result {
        set_async_generator_suspended_yield(vm, generator, true);
        settle_async_generator_request(vm, generator, value, false)?;
        return Ok(false);
    }

    if done {
        if matches!(
            source,
            Some(crate::value::AsyncGeneratorAwaitKind::ResumeReturnDelegated)
        ) {
            begin_async_generator_await(
                vm,
                generator,
                value,
                crate::value::AsyncGeneratorAwaitKind::ResolveReturn,
            )?;
            return Ok(true);
        }
        finish_async_generator_request(vm, generator, value, true, false)?;
        return Ok(false);
    }

    begin_async_generator_await(
        vm,
        generator,
        value,
        crate::value::AsyncGeneratorAwaitKind::ResolveYield,
    )?;
    Ok(true)
}

fn begin_async_generator_await(
    vm: &mut Vm,
    generator: GcIdx,
    value: Value,
    kind: crate::value::AsyncGeneratorAwaitKind,
) -> error::Result<()> {
    let base_pins = vm.pin_many(&[Value::Object(generator), value.clone()]);
    let result = begin_async_generator_await_pinned(vm, generator, value, kind);
    vm.unpin_many(base_pins);
    result
}

fn begin_async_generator_await_pinned(
    vm: &mut Vm,
    generator: GcIdx,
    value: Value,
    kind: crate::value::AsyncGeneratorAwaitKind,
) -> error::Result<()> {
    let generator_realm = async_generator_realm(vm, generator);
    let promise = vm.promise_resolve_for_await_in_env(value, generator_realm)?;

    let state = vm.heap.with_obj(promise.0, |obj| {
        if let HeapObj::Promise(data) = obj {
            *data.state.lock()
        } else {
            crate::value::PromiseStatus::Fulfilled
        }
    });
    let handler = crate::value::PromiseHandler {
        on_fulfilled: Value::Undefined,
        on_rejected: Value::Undefined,
        derived: None,
        continuation: Some(crate::value::PromiseContinuation::AsyncGenerator { generator, kind }),
    };
    if state == crate::value::PromiseStatus::Pending {
        vm.heap.with_obj(promise.0, |obj| {
            if let HeapObj::Promise(data) = obj {
                data.handlers.lock().push(handler);
            }
        });
    } else {
        vm.microtask_queue.push_back(crate::vm::Microtask::Then {
            promise,
            on_fulfilled: Value::Undefined,
            on_rejected: Value::Undefined,
            derived: None,
            continuation: Some(crate::value::PromiseContinuation::AsyncGenerator {
                generator,
                kind,
            }),
            realm: None,
        });
    }
    Ok(())
}

fn finish_async_generator_request(
    vm: &mut Vm,
    generator: GcIdx,
    value: Value,
    done: bool,
    rejected: bool,
) -> error::Result<()> {
    let settled_value = if rejected {
        value
    } else {
        gen_result_in_env(vm, value, done, false, async_generator_realm(vm, generator))?
    };
    settle_async_generator_request(vm, generator, settled_value, rejected)
}

fn settle_async_generator_request(
    vm: &mut Vm,
    generator: GcIdx,
    settled_value: Value,
    rejected: bool,
) -> error::Result<()> {
    let capability = vm.heap.with_obj(generator.0, |obj| {
        if let HeapObj::LazyGenerator(data) = obj {
            data.async_queue
                .lock()
                .front()
                .map(|request| request.capability.clone())
        } else {
            None
        }
    });
    let Some(capability) = capability else {
        return Ok(());
    };
    let function = if rejected {
        capability.reject.clone()
    } else {
        capability.resolve.clone()
    };
    let pins = vm.pin_many(&[
        Value::Object(generator),
        capability.promise,
        capability.resolve,
        capability.reject,
        function.clone(),
        settled_value.clone(),
    ]);
    let result = vm.call_function(&function, &[settled_value], Some(Value::Undefined));
    vm.unpin_many(pins);
    result?;
    vm.heap.with_obj(generator.0, |obj| {
        if let HeapObj::LazyGenerator(data) = obj {
            data.async_queue.lock().pop_front();
        }
    });
    Ok(())
}

fn generator_error_reason(
    vm: &mut Vm,
    generator: GcIdx,
    error: &Arc<Error>,
) -> error::Result<Value> {
    vm.promise_rejection_reason_in_realm(error, async_generator_realm(vm, generator))
}

pub(crate) fn run_async_generator_reaction(
    vm: &mut Vm,
    generator: GcIdx,
    kind: crate::value::AsyncGeneratorAwaitKind,
    promise: GcIdx,
) -> error::Result<()> {
    let generator_pin = vm.pin(&Value::Object(generator));
    let result = run_async_generator_reaction_pinned(vm, generator, kind, promise);
    if result.is_err() {
        abort_async_generator_host_error(vm, generator);
    }
    vm.unpin(generator_pin);
    result
}

fn run_async_generator_reaction_pinned(
    vm: &mut Vm,
    generator: GcIdx,
    kind: crate::value::AsyncGeneratorAwaitKind,
    promise: GcIdx,
) -> error::Result<()> {
    let (state, result) = vm.heap.with_obj(promise.0, |obj| {
        if let HeapObj::Promise(data) = obj {
            (*data.state.lock(), data.result.lock().clone())
        } else {
            (crate::value::PromiseStatus::Fulfilled, Value::Undefined)
        }
    });

    if matches!(kind, crate::value::AsyncGeneratorAwaitKind::ResumeDelegate) {
        let phase = vm.heap.with_obj(generator.0, |object| {
            if let HeapObj::LazyGenerator(data) = object {
                data.async_delegate_await_kind.load(Ordering::Acquire)
            } else {
                0
            }
        });
        let resume = if state == crate::value::PromiseStatus::Rejected {
            crate::vm::ResumeKind::DelegateThrow(result)
        } else {
            match phase {
                1 => crate::vm::ResumeKind::DelegateResult {
                    value: result,
                    return_completion: false,
                },
                2 => crate::vm::ResumeKind::DelegateResult {
                    value: result,
                    return_completion: true,
                },
                3 => crate::vm::ResumeKind::DelegateMissingThrow,
                _ => {
                    return Err(Error::internal(
                        "async generator lost its delegated await phase",
                    ));
                }
            }
        };
        let resumed = vm.resume_generator(generator, resume);
        let waiting = handle_async_generator_resume(vm, generator, resumed, Some(kind))?;
        return if waiting {
            Ok(())
        } else {
            process_async_generator_queue(vm, generator)
        };
    }

    if state == crate::value::PromiseStatus::Rejected
        && matches!(kind, crate::value::AsyncGeneratorAwaitKind::ResolveReturn)
    {
        finish_async_generator_request(vm, generator, result, true, true)?;
        return process_async_generator_queue(vm, generator);
    }

    if state == crate::value::PromiseStatus::Fulfilled
        && matches!(kind, crate::value::AsyncGeneratorAwaitKind::ResolveYield)
    {
        set_async_generator_suspended_yield(vm, generator, true);
        finish_async_generator_request(vm, generator, result, false, false)?;
        return process_async_generator_queue(vm, generator);
    }

    if state == crate::value::PromiseStatus::Fulfilled
        && matches!(kind, crate::value::AsyncGeneratorAwaitKind::ResolveReturn)
    {
        let resumed = vm.resume_generator(generator, crate::vm::ResumeKind::Return(result));
        handle_async_generator_resume(vm, generator, resumed, None)?;
        return process_async_generator_queue(vm, generator);
    }

    let resume = if state == crate::value::PromiseStatus::Rejected {
        crate::vm::ResumeKind::Throw(result)
    } else if matches!(
        kind,
        crate::value::AsyncGeneratorAwaitKind::ResumeReturn
            | crate::value::AsyncGeneratorAwaitKind::ResumeReturnDelegated
    ) {
        crate::vm::ResumeKind::Return(result)
    } else {
        crate::vm::ResumeKind::Next(result)
    };
    let resumed = vm.resume_generator(generator, resume);
    let waiting = handle_async_generator_resume(vm, generator, resumed, Some(kind))?;
    if waiting {
        Ok(())
    } else {
        process_async_generator_queue(vm, generator)
    }
}

fn complete_generator_resume(
    vm: &mut Vm,
    generator: GcIdx,
    mut resumed: error::Result<(Value, bool, bool, bool)>,
    is_async_gen: bool,
) -> error::Result<Value> {
    loop {
        let (value, done, forwarded_result, _awaiting) = match resumed {
            Ok(result) => result,
            Err(error) => return wrap_generator_error(vm, error, is_async_gen),
        };
        if forwarded_result {
            return wrap_generator_result(vm, value, is_async_gen);
        }
        if !is_async_gen {
            return gen_result(vm, value, done, false);
        }

        match vm.await_value(value) {
            Ok(value) => return gen_result(vm, value, done, true),
            Err(error) if !error.catchable() => return Err(error),
            Err(error) if !done => {
                let reason = vm.promise_rejection_reason_in_realm(
                    &error,
                    async_generator_realm(vm, generator),
                )?;
                resumed = vm.resume_generator(generator, crate::vm::ResumeKind::Throw(reason));
            }
            Err(error) => return wrap_generator_error(vm, error, true),
        }
    }
}

/// Build a {value, done} object, wrapped in a Promise for async generators.
pub(crate) fn gen_result(
    vm: &mut Vm,
    value: Value,
    done: bool,
    is_async_gen: bool,
) -> error::Result<Value> {
    let env = vm.current_realm_global_env();
    gen_result_in_env(vm, value, done, is_async_gen, env)
}

pub(crate) fn gen_result_in_env(
    vm: &mut Vm,
    value: Value,
    done: bool,
    is_async_gen: bool,
    env: GcIdx,
) -> error::Result<Value> {
    let object_prototype = vm.object_prototype_for_env(env);
    let obj_idx = vm.alloc(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(object_prototype)),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    vm.heap.with_obj(obj_idx.0, |o| {
        if let HeapObj::Object(obj) = o {
            obj.props
                .lock()
                .insert(PropertyKey::from("value"), enumerable_data_prop(value));
            obj.props.lock().insert(
                PropertyKey::from("done"),
                enumerable_data_prop(Value::Bool(done)),
            );
        }
    });
    let result_obj = Value::Object(obj_idx);
    vm.keep_during_job(&result_obj);
    if is_async_gen {
        wrap_generator_result_in_env(vm, result_obj, env)
    } else {
        Ok(result_obj)
    }
}

fn wrap_generator_result(
    vm: &mut Vm,
    result_obj: Value,
    is_async_gen: bool,
) -> error::Result<Value> {
    if is_async_gen {
        let env = vm.current_realm_global_env();
        wrap_generator_result_in_env(vm, result_obj, env)
    } else {
        Ok(result_obj)
    }
}

fn wrap_generator_result_in_env(
    vm: &mut Vm,
    result_obj: Value,
    env: GcIdx,
) -> error::Result<Value> {
    let prototype = vm.promise_prototype_for_env(env);
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: Mutex::new(crate::value::PromiseStatus::Fulfilled),
            result: Mutex::new(result_obj),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(prototype)),
            extensible: AtomicBool::new(true),
        }))?;
    Ok(Value::Object(GcIdx(p_idx)))
}

fn wrap_generator_error(
    vm: &mut Vm,
    error: Arc<Error>,
    is_async_gen: bool,
) -> error::Result<Value> {
    if !is_async_gen {
        return Err(error);
    }
    let realm = vm.current_realm_global_env();
    let reason = vm.promise_rejection_reason_in_realm(&error, realm)?;
    let prototype = vm.current_realm_promise_prototype();
    let reason_pin = vm.pin(&reason);
    let promise = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: Mutex::new(crate::value::PromiseStatus::Rejected),
            result: Mutex::new(reason),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(prototype)),
            extensible: AtomicBool::new(true),
        }));
    vm.unpin(reason_pin);
    let promise = promise?;
    Ok(Value::Object(GcIdx(promise)))
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
    let resumed = if is_lazy {
        vm.resume_generator(GcIdx(g_idx), crate::vm::ResumeKind::Return(ret.clone()))
    } else {
        Ok((ret.clone(), true, false, false))
    };
    complete_generator_resume(vm, GcIdx(g_idx), resumed, is_async_gen)
}

pub(crate) fn async_generator_return(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    match validate_async_generator_receiver(vm, this)? {
        Ok(generator) => enqueue_async_generator_request(
            vm,
            generator,
            crate::value::AsyncGeneratorRequestKind::Return(
                args.first().cloned().unwrap_or(Value::Undefined),
            ),
        ),
        Err(rejected) => Ok(rejected),
    }
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
        let error = Error::thrown(exc, &vm.heap);
        return wrap_generator_error(vm, error, is_async_gen);
    }
    let resumed = vm.resume_generator(GcIdx(g_idx), crate::vm::ResumeKind::Throw(exc));
    complete_generator_resume(vm, GcIdx(g_idx), resumed, is_async_gen)
}

pub(crate) fn async_generator_throw(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    match validate_async_generator_receiver(vm, this)? {
        Ok(generator) => enqueue_async_generator_request(
            vm,
            generator,
            crate::value::AsyncGeneratorRequestKind::Throw(
                args.first().cloned().unwrap_or(Value::Undefined),
            ),
        ),
        Err(rejected) => Ok(rejected),
    }
}

pub fn setup_collections(vm: &mut Vm) -> error::Result<()> {
    setup_map_set_iterator_protos(vm)?;
    // Map
    let (map_ctor, map_proto) = make_builtin_constructor_with(
        vm,
        "Map",
        0,
        map_constructor,
        NativeConstructMode::InternalEagerPrototype,
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
            PropertyKey::symbol(vm.well_known_symbols.species),
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
                PropertyKey::symbol(vm.well_known_symbols.iterator),
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
        NativeConstructMode::InternalEagerPrototype,
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
            PropertyKey::symbol(vm.well_known_symbols.species),
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
                PropertyKey::symbol(vm.well_known_symbols.iterator),
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
        NativeConstructMode::InternalEagerPrototype,
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
        NativeConstructMode::InternalEagerPrototype,
        &[
            ("add", weakset_add, 1),
            ("has", weakset_has, 1),
            ("delete", weakset_delete, 1),
        ],
    )?;
    define_global(vm, "WeakSet", Value::Object(weakset_ctor));
    let _ = weakset_proto;

    // Symbol
    let sym_idx = vm.new_native_constructor(
        "Symbol",
        symbol_constructor,
        0,
        NativeConstructMode::InternalDeferredPrototype,
    )?;
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
    // Symbol has [[Construct]] for extends/newTarget checks, but construction
    // always throws. Build its primitive wrapper prototype without a generic
    // receiver-producing constructor helper.
    let sym_tostring_idx = vm.new_native_function("toString", symbol_to_string, 0)?;
    let sym_valueof_idx = vm.new_native_function("valueOf", symbol_value_of, 0)?;
    let sym_to_primitive_idx =
        vm.new_native_function("[Symbol.toPrimitive]", symbol_to_primitive, 1)?;
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
        PropertyKey::symbol(vm.well_known_symbols.to_primitive),
        PropertyDescriptor {
            value: Value::Object(sym_to_primitive_idx),
            writable: false,
            enumerable: false,
            configurable: true,
            get: None,
            set: None,
            is_accessor: false,
        },
    );
    sym_proto_props.insert(
        PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
        PropertyDescriptor {
            value: Value::String(Arc::from("Symbol")),
            writable: false,
            enumerable: false,
            configurable: true,
            get: None,
            set: None,
            is_accessor: false,
        },
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
        if let HeapObj::Function(function) = obj {
            *function.prototype.lock() = Some(Value::Object(sym_proto_idx));
        }
    });
    Ok(())
}

pub(crate) fn make_builtin_constructor_with(
    vm: &mut Vm,
    name: &str,
    length: usize,
    ctor: NativeFn,
    construct_mode: NativeConstructMode,
    methods: &[(&str, NativeFn, usize)],
) -> error::Result<(GcIdx, GcIdx)> {
    make_builtin_constructor_with_proto_class_in_env(
        vm,
        name,
        length,
        (ctor, construct_mode),
        methods,
        vm.global,
        Some(name),
    )
}

pub(crate) fn make_builtin_constructor_with_in_env(
    vm: &mut Vm,
    name: &str,
    length: usize,
    ctor: NativeFn,
    construct_mode: NativeConstructMode,
    methods: &[(&str, NativeFn, usize)],
    env: GcIdx,
) -> error::Result<(GcIdx, GcIdx)> {
    make_builtin_constructor_with_proto_class_in_env(
        vm,
        name,
        length,
        (ctor, construct_mode),
        methods,
        env,
        Some(name),
    )
}

pub(crate) fn make_builtin_constructor_with_array_prototype_in_env(
    vm: &mut Vm,
    name: &str,
    length: usize,
    ctor: NativeFn,
    construct_mode: NativeConstructMode,
    methods: &[(&str, NativeFn, usize)],
    env: GcIdx,
) -> error::Result<(GcIdx, GcIdx)> {
    make_builtin_constructor_with_prototype_kind_in_env(
        vm,
        name,
        length,
        (ctor, construct_mode),
        methods,
        env,
        BuiltinPrototypeKind::Array,
    )
}

pub(crate) fn make_builtin_constructor_with_proto_class(
    vm: &mut Vm,
    name: &str,
    length: usize,
    ctor: NativeFn,
    construct_mode: NativeConstructMode,
    methods: &[(&str, NativeFn, usize)],
    proto_class_name: Option<&str>,
) -> error::Result<(GcIdx, GcIdx)> {
    make_builtin_constructor_with_proto_class_in_env(
        vm,
        name,
        length,
        (ctor, construct_mode),
        methods,
        vm.global,
        proto_class_name,
    )
}

pub(crate) fn make_builtin_constructor_with_proto_class_in_env(
    vm: &mut Vm,
    name: &str,
    length: usize,
    constructor: (NativeFn, NativeConstructMode),
    methods: &[(&str, NativeFn, usize)],
    env: GcIdx,
    proto_class_name: Option<&str>,
) -> error::Result<(GcIdx, GcIdx)> {
    make_builtin_constructor_with_prototype_kind_in_env(
        vm,
        name,
        length,
        constructor,
        methods,
        env,
        BuiltinPrototypeKind::Ordinary(proto_class_name),
    )
}

enum BuiltinPrototypeKind<'a> {
    Ordinary(Option<&'a str>),
    Array,
}

fn make_builtin_constructor_with_prototype_kind_in_env(
    vm: &mut Vm,
    name: &str,
    length: usize,
    constructor: (NativeFn, NativeConstructMode),
    methods: &[(&str, NativeFn, usize)],
    env: GcIdx,
    prototype_kind: BuiltinPrototypeKind<'_>,
) -> error::Result<(GcIdx, GcIdx)> {
    let (ctor, construct_mode) = constructor;
    let realm = crate::environment::global_env_root(&vm.heap, env);
    let object_proto = vm
        .realm_object_prototypes
        .get(&realm.0)
        .cloned()
        .unwrap_or_else(|| vm.object_proto.clone());
    let function_proto = vm
        .realm_function_prototypes
        .get(&realm.0)
        .cloned()
        .unwrap_or_else(|| vm.function_proto.clone());
    let mut method_props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    for (n, f, len) in methods {
        let func_idx = vm.new_native_function_in_env(n, *f, *len, env)?;
        method_props.insert(PropertyKey::from(*n), data_prop(Value::Object(func_idx)));
    }
    let proto_obj = match prototype_kind {
        BuiltinPrototypeKind::Ordinary(class_name) => HeapObj::Object(ObjectData {
            props: Mutex::new(method_props),
            proto: Mutex::new(Some(object_proto)),
            extensible: AtomicBool::new(true),
            class_name: class_name.map(Arc::from),
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }),
        BuiltinPrototypeKind::Array => {
            let array = ArrayData::new(Vec::new(), Some(object_proto));
            *array.props.lock() = method_props;
            HeapObj::Array(array)
        }
    };
    let proto_idx = GcIdx(vm.heap.allocate(proto_obj)?);
    let ctor_func = FunctionData {
        name: Some(Arc::from(name)),
        kind: FunctionKind::Native {
            func: ctor,
            length,
            construct_mode: Some(construct_mode),
        },
        closure: env,
        lexical_new_target: Value::Undefined,
        home_object: Mutex::new(None),
        is_class_ctor: std::sync::atomic::AtomicBool::new(false),
        prototype: Mutex::new(Some(Value::Object(proto_idx))),
        proto: Mutex::new(match function_proto {
            Value::Object(_) => Some(function_proto),
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
