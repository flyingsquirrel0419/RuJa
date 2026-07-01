use super::*;

// RegExp
// =========================================================================
pub(crate) fn regexp_constructor(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let pattern = match args.first() {
        Some(Value::String(s)) => s.to_string(),
        Some(v) if !v.is_undefined() => vm.to_string(v)?.to_string(),
        _ => String::new(),
    };
    let flags = match args.get(1) {
        Some(Value::String(s)) => s.to_string(),
        Some(v) if !v.is_undefined() => vm.to_string(v)?.to_string(),
        _ => String::new(),
    };
    // Validate the pattern eagerly so bad regexes throw at construction time.
    Regex::new(&pattern).map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
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
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(regex_proto_val)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("RegExp")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }));
    let mut props = IndexMap::new();
    props.insert(
        PropertyKey::from("source"),
        data_prop(Value::String(Arc::from(pattern.as_str()))),
    );
    props.insert(
        PropertyKey::from("flags"),
        data_prop(Value::String(Arc::from(flags.as_str()))),
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
        PropertyKey::from("lastIndex"),
        data_prop(Value::Number(0.0)),
    );
    vm.heap.with_obj(obj_idx, |o| {
        if let HeapObj::Object(obj) = o {
            *obj.props.lock() = props;
        }
    });
    Ok(Value::Object(GcIdx(obj_idx)))
}

pub(crate) fn regexp_test(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let source = read_regexp_source(vm, &this)?;
    let input = match args.first() {
        Some(Value::String(s)) => s.to_string(),
        Some(v) => vm.to_string(v)?.to_string(),
        None => String::new(),
    };
    let flags = read_regexp_flags(vm, &this).unwrap_or_default();
    let re = compile_regex(&source, &flags)
        .map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
    Ok(Value::Bool(re.is_match(&input)))
}

pub(crate) fn regexp_exec(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
                        data_prop(Value::Number(0.0)),
                    );
                }
            });
        }
        return Ok(Value::Null);
    }
    let region = crate::value::utf16_slice(&input, start, utf16_len);
    // For sticky, match must start exactly at `start`; for global, find from start.
    let m = if sticky {
        re.captures_at(&region, 0)
            .filter(|c| c.get(0).map(|mch| mch.start() == 0).unwrap_or(false))
    } else {
        re.captures(&region)
    };
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
                let match_end = start + crate::value::utf16_len(caps.get(0).map(|mch| mch.as_str()).unwrap_or(""));
                if let Some(Value::Object(idx)) = &this {
                    vm.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Object(obj) = o {
                            obj.props.lock().insert(
                                PropertyKey::from("lastIndex"),
                                data_prop(Value::Number(match_end as f64)),
                            );
                        }
                    });
                }
            }
            Ok(make_value_array(vm, items))
        }
        None => {
            // No match: for global/sticky, reset lastIndex to 0.
            if global || sticky {
                if let Some(Value::Object(idx)) = &this {
                    vm.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Object(obj) = o {
                            obj.props.lock().insert(
                                PropertyKey::from("lastIndex"),
                                data_prop(Value::Number(0.0)),
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
pub(crate) fn read_regexp_field(vm: &mut Vm, this: &Option<Value>, field: &str) -> error::Result<String> {
    match this {
        Some(Value::Object(idx)) => {
            let s = vm.heap.with_obj(idx.0, |o| {
                o.props()
                    .lock()
                    .get(&crate::value::PropertyKey::from(field))
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

pub(crate) fn generator_next(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
    }));
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
            }));
        Ok(Value::Object(GcIdx(p_idx)))
    } else {
        Ok(result_obj)
    }
}

/// Build a {value, done} object, wrapped in a Promise for async generators.
pub(crate) fn gen_result(vm: &mut Vm, value: Value, done: bool, is_async_gen: bool) -> error::Result<Value> {
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }));
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
            }));
        Ok(Value::Object(GcIdx(p_idx)))
    } else {
        Ok(result_obj)
    }
}

/// `generator.return(v)`: force-complete the generator. If it is suspended at
/// a `yield`, the value `v` becomes the result of the yield* / next() call and
/// the generator is marked done. If it was already done, returns {value:v,
/// done:true}.
pub(crate) fn generator_return(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
pub(crate) fn generator_throw(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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

pub fn setup_collections(vm: &mut Vm) {
    // Map
    let (map_ctor, map_proto) = make_builtin_constructor_with(
        vm,
        "Map",
        map_constructor,
        &[
            ("set", map_set, 2),
            ("get", map_get, 1),
            ("has", map_has, 1),
            ("delete", map_delete, 1),
            ("clear", map_clear, 0),
            ("size", map_size, 0),
            ("entries", map_entries, 0),
            ("keys", map_keys, 0),
            ("values", map_values, 0),
            ("forEach", map_for_each, 1),
        ],
    );
    vm.map_proto = Value::Object(map_proto);
    define_global(vm, "Map", Value::Object(map_ctor));
    // Map.prototype[Symbol.iterator] === Map.prototype.entries
    let map_entries_fn = vm.new_native_function("entries", map_entries, 0);
    if let Value::Object(mp) = vm.map_proto.clone() {
        vm.heap.with_obj(mp.0, |o| {
            o.props().lock().insert(
                PropertyKey::Symbol(vm.well_known_symbols.iterator),
                data_prop(Value::Object(map_entries_fn)),
            );
        });
    }
    // Set
    let (set_ctor, set_proto) = make_builtin_constructor_with(
        vm,
        "Set",
        set_constructor,
        &[
            ("add", set_add, 1),
            ("has", set_has, 1),
            ("delete", set_delete, 1),
            ("size", set_size, 0),
            ("entries", set_entries, 0),
            ("keys", set_keys, 0),
            ("values", set_values, 0),
            ("forEach", set_for_each, 1),
        ],
    );
    vm.set_proto = Value::Object(set_proto);
    define_global(vm, "Set", Value::Object(set_ctor));
    // Set.prototype[Symbol.iterator] === Set.prototype.values
    let set_values_fn = vm.new_native_function("values", set_values, 0);
    if let Value::Object(sp) = vm.set_proto.clone() {
        vm.heap.with_obj(sp.0, |o| {
            o.props().lock().insert(
                PropertyKey::Symbol(vm.well_known_symbols.iterator),
                data_prop(Value::Object(set_values_fn)),
            );
        });
    }
    // WeakMap / WeakSet: true weak-reference semantics. Keys are object
    // heap indices held weakly; GC sweeps entries whose key was collected.
    let (weakmap_ctor, weakmap_proto) = make_builtin_constructor_with(
        vm,
        "WeakMap",
        weakmap_constructor,
        &[
            ("get", weakmap_get, 1),
            ("set", weakmap_set, 2),
            ("has", weakmap_has, 1),
            ("delete", weakmap_delete, 1),
        ],
    );
    define_global(vm, "WeakMap", Value::Object(weakmap_ctor));
    let _ = weakmap_proto;
    let (weakset_ctor, weakset_proto) = make_builtin_constructor_with(
        vm,
        "WeakSet",
        weakset_constructor,
        &[
            ("add", weakset_add, 1),
            ("has", weakset_has, 1),
            ("delete", weakset_delete, 1),
        ],
    );
    define_global(vm, "WeakSet", Value::Object(weakset_ctor));
    let _ = weakset_proto;

    // Symbol
    let sym_idx = vm.new_native_function("Symbol", symbol_constructor, 1);
    define_global(vm, "Symbol", Value::Object(sym_idx));
    let sym_for_idx = vm.new_native_function("for", symbol_for, 1);
    if let Value::Object(idx) = Value::Object(sym_idx) {
        vm.heap.with_obj(idx.0, |obj| {
            obj.props().lock().insert(
                PropertyKey::from("for"),
                data_prop(Value::Object(sym_for_idx)),
            );
            obj.props().lock().insert(
                PropertyKey::from("iterator"),
                data_prop(Value::Symbol(vm.well_known_symbols.iterator)),
            );
            obj.props().lock().insert(
                PropertyKey::from("asyncIterator"),
                data_prop(Value::Symbol(vm.well_known_symbols.async_iterator)),
            );
            obj.props().lock().insert(
                PropertyKey::from("toPrimitive"),
                data_prop(Value::Symbol(vm.well_known_symbols.to_primitive)),
            );
            obj.props().lock().insert(
                PropertyKey::from("hasInstance"),
                data_prop(Value::Symbol(vm.well_known_symbols.has_instance)),
            );
            obj.props().lock().insert(
                PropertyKey::from("toStringTag"),
                data_prop(Value::Symbol(vm.well_known_symbols.to_string_tag)),
            );
        });
    }
    // Symbol.prototype: a plain Object with a toString method. Symbol is a
    // value type (not a constructor), so build the proto manually rather than
    // going through make_builtin_constructor.
    let sym_tostring_idx = vm.new_native_function("toString", symbol_to_string, 0);
    let mut sym_proto_props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    sym_proto_props.insert(
        PropertyKey::from("toString"),
        data_prop(Value::Object(sym_tostring_idx)),
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
    let sym_proto_idx = GcIdx(vm.heap.allocate(sym_proto_obj));
    vm.symbol_proto = Value::Object(sym_proto_idx);
}

pub(crate) fn make_builtin_constructor_with(
    vm: &mut Vm,
    name: &str,
    ctor: NativeFn,
    methods: &[(&str, NativeFn, usize)],
) -> (GcIdx, GcIdx) {
    let mut method_props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    for (n, f, len) in methods {
        let func_idx = vm.new_native_function(n, *f, *len);
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
    let proto_idx = GcIdx(vm.heap.allocate(proto_obj));
    let ctor_func = FunctionData {
        name: Some(Arc::from(name)),
        kind: FunctionKind::Native {
            func: ctor,
            length: 0,
        },
        closure: vm.global,
        prototype: Mutex::new(Some(Value::Object(proto_idx))),
        proto: Mutex::new(match vm.function_proto {
            Value::Object(_) => Some(vm.function_proto.clone()),
            _ => None,
        }),
        props: Mutex::new(IndexMap::new()),
    };
    let ctor_idx = GcIdx(vm.heap.allocate(HeapObj::Function(ctor_func)));
    vm.heap.with_obj(ctor_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("prototype"),
            data_prop(Value::Object(proto_idx)),
        );
    });
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(ctor_idx)),
        );
    });
    (ctor_idx, proto_idx)
}

// =========================================================================
