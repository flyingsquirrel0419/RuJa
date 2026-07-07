use super::*;

// RegExp
// =========================================================================
fn regexp_last_index_prop(value: Value) -> PropertyDescriptor {
    let mut desc = data_prop(value);
    desc.configurable = false;
    desc
}

const REGEXP_SOURCE_SLOT: &str = "__regexp_source__";
const REGEXP_FLAGS_SLOT: &str = "__regexp_flags__";

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
    crate::lexer::validate_regex_literal(&pattern, &flags).map_err(Error::syntax)?;
    // Validate the pattern eagerly so bad regexes throw at construction time.
    compile_regex(&pattern, &flags).map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
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
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(regex_proto_val)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("RegExp")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let mut props = IndexMap::new();
    props.insert(
        PropertyKey::from(REGEXP_SOURCE_SLOT),
        data_prop(Value::String(Arc::from(pattern.as_str()))),
    );
    props.insert(
        PropertyKey::from(REGEXP_FLAGS_SLOT),
        data_prop(Value::String(Arc::from(flags.as_str()))),
    );
    props.insert(
        PropertyKey::from("hasIndices"),
        data_prop(Value::Bool(flags.contains('d'))),
    );
    props.insert(
        PropertyKey::from("global"),
        data_prop(Value::Bool(flags.contains('g'))),
    );
    props.insert(
        PropertyKey::from("ignoreCase"),
        data_prop(Value::Bool(flags.contains('i'))),
    );
    props.insert(
        PropertyKey::from("multiline"),
        data_prop(Value::Bool(flags.contains('m'))),
    );
    props.insert(
        PropertyKey::from("dotAll"),
        data_prop(Value::Bool(flags.contains('s'))),
    );
    props.insert(
        PropertyKey::from("unicode"),
        data_prop(Value::Bool(flags.contains('u'))),
    );
    props.insert(
        PropertyKey::from("unicodeSets"),
        data_prop(Value::Bool(flags.contains('v'))),
    );
    props.insert(
        PropertyKey::from("sticky"),
        data_prop(Value::Bool(flags.contains('y'))),
    );
    props.insert(
        PropertyKey::from("lastIndex"),
        regexp_last_index_prop(Value::Number(0.0)),
    );
    vm.heap.with_obj(obj_idx, |o| {
        if let HeapObj::Object(obj) = o {
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

pub(crate) fn regexp_to_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let source = escape_regexp_source_for_accessor(&read_regexp_source(vm, &this)?);
    let flags = read_regexp_flags(vm, &this).unwrap_or_default();
    Ok(Value::String(Arc::from(
        format!("/{source}/{flags}").as_str(),
    )))
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
            let value = vm.heap.with_obj(idx.0, |o| {
                o.props()
                    .lock()
                    .get(&PropertyKey::from(field))
                    .map(|d| d.value.clone())
            });
            Ok(match value {
                Some(Value::Bool(v)) => Value::Bool(v),
                _ => Value::Bool(false),
            })
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
    let input = match args.first() {
        Some(Value::String(s)) => s.to_string(),
        Some(v) => vm.to_string(v)?.to_string(),
        None => String::new(),
    };
    let flags = read_regexp_flags(vm, &this).unwrap_or_default();
    let re = compile_regex(&source, &flags)
        .map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
    let global = flags.contains('g');
    let sticky = flags.contains('y');
    // Read lastIndex (a number property; default 0).
    let last_idx: f64 = match &this {
        Some(Value::Object(idx)) => vm.heap.with_obj(idx.0, |o| {
            o.props()
                .lock()
                .get(&PropertyKey::from("lastIndex"))
                .map(|d| match &d.value {
                    Value::Number(n) => *n,
                    _ => 0.0,
                })
                .unwrap_or(0.0)
        }),
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
        if let Some(Value::Object(idx)) = &this {
            vm.heap.with_obj(idx.0, |o| {
                if let HeapObj::Object(obj) = o {
                    obj.props.lock().insert(
                        PropertyKey::from("lastIndex"),
                        regexp_last_index_prop(Value::Number(0.0)),
                    );
                }
            });
        }
        return Ok(Value::Null);
    }
    let Some(start_byte) = crate::value::utf16_index_to_byte(&input, start) else {
        if global || sticky {
            if let Some(Value::Object(idx)) = &this {
                vm.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Object(obj) = o {
                        obj.props.lock().insert(
                            PropertyKey::from("lastIndex"),
                            regexp_last_index_prop(Value::Number(0.0)),
                        );
                    }
                });
            }
        }
        return Ok(Value::Null);
    };
    // Run against the whole input so `^` still observes the real input start
    // and multiline line starts; sticky only requires the match to begin at
    // lastIndex.
    let m = re.captures_at(&input, start_byte).filter(|c| {
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
                if let Some(Value::Object(idx)) = &this {
                    vm.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Object(obj) = o {
                            obj.props.lock().insert(
                                PropertyKey::from("lastIndex"),
                                regexp_last_index_prop(Value::Number(match_end as f64)),
                            );
                        }
                    });
                }
            }
            make_value_array(vm, items)
        }
        None => {
            // No match: for global/sticky, reset lastIndex to 0.
            if global || sticky {
                if let Some(Value::Object(idx)) = &this {
                    vm.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Object(obj) = o {
                            obj.props.lock().insert(
                                PropertyKey::from("lastIndex"),
                                regexp_last_index_prop(Value::Number(0.0)),
                            );
                        }
                    });
                }
            }
            Ok(Value::Null)
        }
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
    let storage_field = match field {
        "source" => REGEXP_SOURCE_SLOT,
        "flags" => REGEXP_FLAGS_SLOT,
        other => other,
    };
    match this {
        Some(Value::Object(idx)) => {
            let s = vm.heap.with_obj(idx.0, |o| {
                o.props()
                    .lock()
                    .get(&crate::value::PropertyKey::from(storage_field))
                    .map(|d| d.value.clone())
            });
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
    vm.heap.with_obj(map_ctor.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::Symbol(vm.well_known_symbols.species),
            accessor_get_prop(Value::Object(map_species_getter)),
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
    let mut method_props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    for (n, f, len) in methods {
        let func_idx = vm.new_native_function(n, *f, *len)?;
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
        closure: vm.global,
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
