//! Built-in objects and globals for the RuJa VM.
//!
//! All built-in constructors, prototypes, and global functions are registered
//! here. Native functions follow the `NativeFn` signature used by the VM.

pub(crate) mod global;
pub(crate) mod json;
pub(crate) mod math;

pub(crate) mod array;
pub(crate) use array::*;

pub(crate) mod string;
pub(crate) use string::*;

pub(crate) mod collections;
pub(crate) use collections::*;
pub(crate) mod regexp;
pub(crate) use regexp::*;
pub(crate) mod function;
pub(crate) mod proxy;
pub(crate) mod typed_array;
pub(crate) use function::*;
pub(crate) use global::{
    bigint_as_int_n, bigint_as_uint_n, bigint_to_string, bigint_value_of, function_constructor,
    generator_function_constructor, global_bigint, global_eval, global_is_finite, global_is_nan,
    global_parse_float, global_parse_int,
};
pub(crate) use json::{
    build_json, build_reflect, date_constructor, date_get_component, date_get_time,
    date_get_timezone_offset, date_now, date_parse, date_set_component, date_to_string, date_utc,
};
pub(crate) use math::{build_console, build_math};
pub(crate) use proxy::*;
pub(crate) use typed_array::*;

use crate::environment as env;
use crate::error::{self, Error};
use crate::gc::Heap;
use crate::value::{
    ArrayData, BindingKind, FunctionData, FunctionKind, GcIdx, HeapObj, MapData, MapKey,
    ObjectData, PropertyDescriptor, PropertyKey, SetData, Value,
};
use crate::vm::{NativeFn, Vm};
use indexmap::{IndexMap, IndexSet};
use num_bigint::{BigInt, BigUint};
use num_integer::Integer;
use num_rational::Ratio;
use num_traits::{Signed, ToPrimitive, Zero};
use regex::{Regex, RegexBuilder};

/// Compile a regex pattern applying ES flags: `i` (case-insensitive),
/// `m` (multiline ^/$), `s` (dotall). Other flags (`g`/`y`/`u`) do not affect
/// the regex engine here and are handled by the caller.
fn compile_regex(source: &str, flags: &str) -> Result<Regex, regex::Error> {
    let backend_source = normalize_regex_for_backend(source);
    let mut b = RegexBuilder::new(&backend_source);
    b.case_insensitive(flags.contains('i'));
    b.multi_line(flags.contains('m'));
    b.dot_matches_new_line(flags.contains('s'));
    b.build()
}

fn normalize_regex_for_backend(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_class = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            out.push(ch);
            escaped = true;
            continue;
        }
        if ch == '[' {
            in_class = true;
            out.push(ch);
            continue;
        }
        if ch == ']' && in_class {
            in_class = false;
            out.push(ch);
            continue;
        }

        if !in_class && ch == '(' && chars.peek() == Some(&'?') {
            out.push(ch);
            out.push(chars.next().unwrap());
            let mut modifiers = String::new();
            while matches!(chars.peek(), Some('i' | 'm' | 's')) {
                modifiers.push(chars.next().unwrap());
            }
            if !modifiers.is_empty() && chars.peek() == Some(&'-') {
                chars.next();
                if chars.peek() == Some(&':') {
                    out.push_str(&modifiers);
                    continue;
                }
                out.push_str(&modifiers);
                out.push('-');
                continue;
            }
            out.push_str(&modifiers);
            continue;
        }

        out.push(ch);
    }

    out
}
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use std::sync::Arc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn data_prop(value: Value) -> PropertyDescriptor {
    PropertyDescriptor {
        value,
        writable: true,
        enumerable: false,
        configurable: true,
        get: None,
        set: None,
        is_accessor: false,
    }
}

pub(crate) fn builtin_function_own_props(
    name: &str,
    length: usize,
) -> IndexMap<PropertyKey, PropertyDescriptor> {
    let mut props = IndexMap::new();
    let mut length_desc = PropertyDescriptor::data(Value::Number(length as f64));
    length_desc.writable = false;
    length_desc.enumerable = false;
    length_desc.configurable = true;
    props.insert(PropertyKey::from("length"), length_desc);

    let mut name_desc = PropertyDescriptor::data(Value::String(Arc::from(name)));
    name_desc.writable = false;
    name_desc.enumerable = false;
    name_desc.configurable = true;
    props.insert(PropertyKey::from("name"), name_desc);
    props
}

/// Create a non-writable, non-enumerable, non-configurable data property
/// descriptor (for built-in constants like Number.MAX_VALUE).
pub(crate) fn const_prop(value: Value) -> PropertyDescriptor {
    PropertyDescriptor {
        value,
        writable: false,
        enumerable: false,
        configurable: false,
        get: None,
        set: None,
        is_accessor: false,
    }
}

fn accessor_get_prop(get: Value) -> PropertyDescriptor {
    PropertyDescriptor {
        value: Value::Undefined,
        writable: false,
        enumerable: false,
        configurable: true,
        get: Some(get),
        set: None,
        is_accessor: true,
    }
}

fn accessor_prop(get: Value, set: Value) -> PropertyDescriptor {
    PropertyDescriptor {
        value: Value::Undefined,
        writable: false,
        enumerable: false,
        configurable: true,
        get: Some(get),
        set: Some(set),
        is_accessor: true,
    }
}

pub(crate) fn install_symbol_static_properties(
    vm: &Vm,
    props: &mut IndexMap<PropertyKey, PropertyDescriptor>,
) {
    let symbols = &vm.well_known_symbols;
    for (name, id) in [
        ("asyncIterator", symbols.async_iterator),
        ("hasInstance", symbols.has_instance),
        ("isConcatSpreadable", symbols.is_concat_spreadable),
        ("iterator", symbols.iterator),
        ("match", symbols.r#match),
        ("matchAll", symbols.match_all),
        ("replace", symbols.replace),
        ("search", symbols.search),
        ("species", symbols.species),
        ("split", symbols.split),
        ("toPrimitive", symbols.to_primitive),
        ("toStringTag", symbols.to_string_tag),
        ("unscopables", symbols.unscopables),
    ] {
        props.insert(PropertyKey::from(name), const_prop(Value::Symbol(id)));
    }
}

pub(crate) fn native_constructor_prototype(vm: &mut Vm, fallback: Value) -> error::Result<Value> {
    if let Some(new_target) = vm.current_native_new_target.clone() {
        let proto = vm.get_property_by_key(&new_target, &PropertyKey::from("prototype"))?;
        if matches!(proto, Value::Object(_)) {
            return Ok(proto);
        }
    }
    Ok(fallback)
}

pub(crate) fn install_methods(vm: &mut Vm, proto: &Value, methods: &[(Arc<str>, Value)]) {
    if let Value::Object(idx) = proto {
        vm.heap.with_obj(idx.0, |obj| {
            let props = obj.props();
            for (name, func) in methods {
                props
                    .lock()
                    .insert(PropertyKey::from(name.clone()), data_prop(func.clone()));
            }
        });
    }
}

pub(crate) fn is_array(value: &Value, heap: &Heap) -> bool {
    match value {
        Value::Object(idx) => heap.with_obj(idx.0, |obj| match obj {
            HeapObj::Array(a) => !a.is_arguments.load(Ordering::Relaxed),
            // Tagged-template objects are ordinary objects with class_name "Array"
            // and Array.prototype, so they are recognized as arrays.
            HeapObj::Object(o) => o.class_name.as_deref() == Some("Array"),
            _ => false,
        }),
        _ => false,
    }
}

pub(crate) fn is_callable(value: &Value, heap: &Heap) -> bool {
    match value {
        Value::Object(idx) => heap.with_obj(idx.0, |obj| obj.is_function()),
        _ => false,
    }
}

pub(crate) fn object_to_string(
    vm: &mut Vm,
    this: Option<Value>,
    class_hint: Option<&str>,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_null() {
        return Ok(Value::String(Arc::from("[object Null]")));
    }
    if this.is_undefined() {
        return Ok(Value::String(Arc::from("[object Undefined]")));
    }
    if let Value::String(_) = &this {
        return Ok(Value::String(Arc::from("[object String]")));
    }
    if let Value::Number(_) = &this {
        return Ok(Value::String(Arc::from("[object Number]")));
    }
    if let Value::Bool(_) = &this {
        return Ok(Value::String(Arc::from("[object Boolean]")));
    }
    if let Value::Symbol(_) = &this {
        return Ok(Value::String(Arc::from("[object Symbol]")));
    }
    if let Value::BigInt(_) = &this {
        return Ok(Value::String(Arc::from("[object BigInt]")));
    }
    if let Value::Object(idx) = &this {
        let class = if let Some(hint) = class_hint {
            hint.to_string()
        } else {
            vm.heap.with_obj(idx.0, |obj| obj.class_name().to_string())
        };
        let result = format!("[object {}]", class);
        return Ok(Value::String(Arc::from(result.as_str())));
    }
    Ok(Value::String(Arc::from("[object Object]")))
}

// ---------------------------------------------------------------------------
// Built-in builders
// ---------------------------------------------------------------------------

pub(crate) fn make_builtin_constructor(
    vm: &mut Vm,
    name: &str,
    methods: &[(&str, NativeFn, usize)],
) -> error::Result<(GcIdx, GcIdx)> {
    let proto_value = vm.object_proto.clone();

    let mut method_props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    for (n, f, len) in methods {
        let func_idx = vm.new_native_function(n, *f, *len)?;
        method_props.insert(PropertyKey::from(*n), data_prop(Value::Object(func_idx)));
    }

    let proto_obj = HeapObj::Object(ObjectData {
        props: Mutex::new(method_props),
        proto: Mutex::new(Some(proto_value.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from(name)),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    let proto_idx = GcIdx(vm.heap.allocate(proto_obj)?);

    let ctor_func = FunctionData {
        name: Some(Arc::from(name)),
        kind: FunctionKind::Native {
            func: object_constructor,
            length: 1,
        },
        closure: vm.global,
        lexical_new_target: Value::Undefined,
        is_class_ctor: std::sync::atomic::AtomicBool::new(false),
        prototype: Mutex::new(Some(Value::Object(proto_idx))),
        proto: Mutex::new(match vm.function_proto {
            Value::Object(_) => Some(vm.function_proto.clone()),
            _ => None,
        }),
        props: Mutex::new(builtin_function_own_props(name, 1)),
        extensible: AtomicBool::new(true),
        private_fields: Mutex::new(std::collections::HashMap::new()),
    };
    let ctor_idx = GcIdx(vm.heap.allocate(HeapObj::Function(ctor_func))?);
    // constructor.prototype
    vm.heap.with_obj(ctor_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(Value::Object(proto_idx)),
        );
    });
    // prototype.constructor
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(ctor_idx)),
        );
    });

    Ok((ctor_idx, proto_idx))
}

pub(crate) fn make_error_constructor(vm: &mut Vm, name: &str) -> error::Result<(GcIdx, GcIdx)> {
    let proto_parent = if matches!(vm.error_proto, Value::Object(_)) {
        vm.error_proto.clone()
    } else {
        vm.object_proto.clone()
    };
    let proto_obj = HeapObj::Object(ObjectData {
        props: Mutex::new(builtin_function_own_props(name, 1)),
        proto: Mutex::new(Some(proto_parent)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from(name)),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    let proto_idx = GcIdx(vm.heap.allocate(proto_obj)?);

    let ctor_func = FunctionData {
        name: Some(Arc::from(name)),
        kind: FunctionKind::Native {
            func: error_constructor,
            length: 1,
        },
        closure: vm.global,
        lexical_new_target: Value::Undefined,
        is_class_ctor: std::sync::atomic::AtomicBool::new(false),
        prototype: Mutex::new(Some(Value::Object(proto_idx))),
        proto: Mutex::new(match vm.function_proto {
            Value::Object(_) => Some(vm.function_proto.clone()),
            _ => None,
        }),
        props: Mutex::new(IndexMap::new()),
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
    let ts_fn = vm.new_native_function("toString", error_to_string, 0)?;
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(ctor_idx)),
        );
        obj.props().lock().insert(
            PropertyKey::from("name"),
            data_prop(Value::String(Arc::from(name))),
        );
        obj.props().lock().insert(
            PropertyKey::from("message"),
            data_prop(Value::String(Arc::from(""))),
        );
        obj.props().lock().insert(
            PropertyKey::from("toString"),
            data_prop(Value::Object(ts_fn)),
        );
    });

    Ok((ctor_idx, proto_idx))
}

pub(crate) fn define_global(vm: &mut Vm, name: &str, value: Value) {
    env::declare(&vm.heap, vm.global, name, value.clone(), BindingKind::Var);
    define_global_property(vm, name, data_prop(value));
}

pub(crate) fn define_global_const(vm: &mut Vm, name: &str, value: Value) {
    env::declare(&vm.heap, vm.global, name, value.clone(), BindingKind::Var);
    define_global_property(vm, name, const_prop(value));
}

fn define_global_property(vm: &mut Vm, name: &str, desc: PropertyDescriptor) {
    if let Value::Object(idx) = &vm.global_this {
        vm.heap.with_obj(idx.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from(name), desc.clone());
        });
    }
}

fn init_global_this(vm: &mut Vm) -> error::Result<()> {
    let globalthis_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("global")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    vm.global_this = Value::Object(GcIdx(globalthis_idx));

    for (name, value) in env::own_bindings(&vm.heap, vm.global) {
        define_global_property(vm, &name, data_prop(value));
    }
    define_global(vm, "globalThis", vm.global_this.clone());
    Ok(())
}

fn define_realm_global(vm: &mut Vm, env: GcIdx, global: &Value, name: &str, value: Value) {
    crate::environment::declare(&vm.heap, env, name, value.clone(), BindingKind::Var);
    if let Value::Object(idx) = global {
        vm.heap.with_obj(idx.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from(name), data_prop(value));
        });
    }
}

fn make_test262_realm(vm: &mut Vm) -> error::Result<Value> {
    let realm_env = crate::environment::new_env(&vm.heap, None, true)?;
    let global_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("realm-global")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let global = Value::Object(GcIdx(global_idx));

    crate::environment::declare(
        &vm.heap,
        realm_env,
        "this",
        global.clone(),
        BindingKind::Const,
    );
    define_realm_global(vm, realm_env, &global, "globalThis", global.clone());

    let eval_idx = vm.new_native_function_in_env("eval", global_eval, 1, realm_env)?;
    let eval_value = Value::Object(eval_idx);
    vm.realm_eval_functions
        .insert(realm_env.0, eval_value.clone());
    define_realm_global(vm, realm_env, &global, "eval", eval_value);

    let parse_int_idx =
        vm.new_native_function_in_env("parseInt", global_parse_int, 2, realm_env)?;
    define_realm_global(
        vm,
        realm_env,
        &global,
        "parseInt",
        Value::Object(parse_int_idx),
    );
    if let Some(object) = crate::environment::get(&vm.heap, vm.global, "Object") {
        define_realm_global(vm, realm_env, &global, "Object", object);
    }
    if let Some(bigint) = crate::environment::get(&vm.heap, vm.global, "BigInt") {
        define_realm_global(vm, realm_env, &global, "BigInt", bigint);
    }
    if let Some(proxy) = crate::environment::get(&vm.heap, vm.global, "Proxy") {
        define_realm_global(vm, realm_env, &global, "Proxy", proxy);
    }
    let symbol_idx = vm.new_native_function_in_env("Symbol", symbol_constructor, 0, realm_env)?;
    let symbol_for_idx = vm.new_native_function_in_env("for", symbol_for, 1, realm_env)?;
    let symbol_key_for_idx =
        vm.new_native_function_in_env("keyFor", symbol_key_for, 1, realm_env)?;
    vm.heap.with_obj(symbol_idx.0, |obj| {
        let mut props = obj.props().lock();
        props.insert(
            PropertyKey::from("for"),
            data_prop(Value::Object(symbol_for_idx)),
        );
        props.insert(
            PropertyKey::from("keyFor"),
            data_prop(Value::Object(symbol_key_for_idx)),
        );
        install_symbol_static_properties(vm, &mut props);
        props.insert(
            PropertyKey::from("prototype"),
            const_prop(vm.symbol_proto.clone()),
        );
    });
    define_realm_global(vm, realm_env, &global, "Symbol", Value::Object(symbol_idx));

    Ok(global)
}

fn test262_create_realm(vm: &mut Vm, _args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let global = make_test262_realm(vm)?;
    let realm = vm.new_object()?;
    vm.heap.with_obj(realm.0, |obj| {
        obj.props()
            .lock()
            .insert(PropertyKey::from("global"), data_prop(global));
    });
    Ok(Value::Object(realm))
}

fn test262_eval_script(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let src = match args.first().cloned().unwrap_or(Value::Undefined) {
        Value::String(s) => s.to_string(),
        _ => return Ok(Value::Undefined),
    };
    vm.eval_script_global(&src)
}

fn test262_detach_array_buffer(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let buffer = args.first().cloned().unwrap_or(Value::Undefined);
    match buffer {
        Value::Object(idx) => {
            let detached = vm.heap.with_obj(idx.0, |obj| {
                if let HeapObj::ArrayBuffer(array_buffer) = obj {
                    array_buffer
                        .detached
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    array_buffer.bytes.lock().clear();
                    true
                } else {
                    false
                }
            });
            if detached {
                Ok(Value::Undefined)
            } else {
                Err(Error::type_err(
                    "$262.detachArrayBuffer called on non-ArrayBuffer",
                ))
            }
        }
        _ => Err(Error::type_err(
            "$262.detachArrayBuffer called on non-object",
        )),
    }
}

fn install_test262_host(vm: &mut Vm) -> error::Result<()> {
    let host = vm.new_object()?;
    let create_realm = vm.new_native_function("createRealm", test262_create_realm, 0)?;
    let eval_script = vm.new_native_function("evalScript", test262_eval_script, 1)?;
    let detach_array_buffer =
        vm.new_native_function("detachArrayBuffer", test262_detach_array_buffer, 1)?;
    vm.heap.with_obj(host.0, |obj| {
        let mut props = obj.props().lock();
        props.insert(
            PropertyKey::from("createRealm"),
            data_prop(Value::Object(create_realm)),
        );
        props.insert(
            PropertyKey::from("evalScript"),
            data_prop(Value::Object(eval_script)),
        );
        props.insert(
            PropertyKey::from("detachArrayBuffer"),
            data_prop(Value::Object(detach_array_buffer)),
        );
        props.insert(
            PropertyKey::from("global"),
            data_prop(vm.global_this.clone()),
        );
    });
    define_global(vm, "$262", Value::Object(host));
    Ok(())
}

pub(crate) fn get_arg(args: &[Value], idx: usize) -> Value {
    args.get(idx).cloned().unwrap_or(Value::Undefined)
}

// ---------------------------------------------------------------------------
// Object constructor / prototype
// ---------------------------------------------------------------------------

fn object_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        if args.is_empty() {
            return Ok(Value::Object(idx));
        }
        let first = args.first().unwrap_or(&Value::Undefined);
        match first {
            Value::Undefined | Value::Null => {}
            Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Symbol(_)
            | Value::BigInt(_) => {
                return vm.to_object(first);
            }
            Value::Object(_) => return Ok(first.clone()),
            Value::Reference(_) => {
                return Err(Error::type_err("Reference is not an object".to_string()))
            }
        }
        let new_idx = vm.new_object()?;
        return Ok(Value::Object(new_idx));
    }
    // Called as function
    if args.is_empty() {
        let new_idx = vm.new_object()?;
        return Ok(Value::Object(new_idx));
    }
    let first = args.first().unwrap_or(&Value::Undefined);
    match first {
        Value::Undefined | Value::Null => {
            let new_idx = vm.new_object()?;
            Ok(Value::Object(new_idx))
        }
        Value::Bool(_)
        | Value::Number(_)
        | Value::String(_)
        | Value::Symbol(_)
        | Value::BigInt(_) => vm.to_object(first),
        Value::Object(_) => Ok(first.clone()),
        Value::Reference(_) => Err(Error::type_err("Reference is not an object".to_string())),
    }
}

fn object_to_string_native(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    object_to_string(vm, this, None)
}

fn object_to_locale_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let to_string = vm.get_property(&this, "toString")?;
    if !is_callable(&to_string, &vm.heap) {
        return Err(Error::type_err("toString is not a function".to_string()));
    }
    vm.call_function(&to_string, &[], Some(this))
}

fn object_has_own_key(vm: &Vm, obj: &Value, key: &PropertyKey) -> bool {
    if let Value::Object(idx) = obj {
        if let Some(target) = vm.heap.with_obj(idx.0, |heap_obj| {
            if let HeapObj::Proxy(proxy) = heap_obj {
                if *proxy.revoked.lock() {
                    None
                } else {
                    Some(proxy.target.clone())
                }
            } else {
                None
            }
        }) {
            return object_has_own_key(vm, &target, key);
        }
    }

    match obj {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |heap_obj| {
            if heap_obj.props().lock().contains_key(key) {
                return true;
            }
            if let HeapObj::Array(a) = heap_obj {
                if key.as_str() == Some("length") {
                    return !a.is_arguments.load(Ordering::Relaxed);
                }
                if let Some(name) = key.as_str() {
                    if let Some(i) = crate::value::parse_array_index(name) {
                        return a.is_dense_present(i);
                    }
                }
            }
            if let HeapObj::Object(od) = heap_obj {
                if let Some(Value::String(s)) = od.primitive.lock().clone() {
                    if key.as_str() == Some("length") {
                        return true;
                    }
                    return key
                        .as_str()
                        .and_then(|name| name.parse::<usize>().ok())
                        .is_some_and(|i| i < crate::value::utf16_len(&s));
                }
            }
            false
        }),
        Value::String(s) => {
            if key.as_str() == Some("length") {
                return true;
            }
            key.as_str()
                .and_then(|name| name.parse::<usize>().ok())
                .is_some_and(|i| i < crate::value::utf16_len(s))
        }
        _ => false,
    }
}

fn to_property_key_descriptor(vm: &mut Vm, value: &Value) -> error::Result<PropertyKey> {
    match vm.to_property_key_value(value)? {
        Value::String(s) => Ok(PropertyKey::from_rc(s)),
        Value::Symbol(id) => Ok(PropertyKey::Symbol(id)),
        _ => unreachable!("ToPropertyKey returns only String or Symbol"),
    }
}

pub(crate) fn property_key_to_value(key: &PropertyKey) -> Value {
    match key {
        PropertyKey::Str(s) => Value::String(s.clone()),
        PropertyKey::Symbol(id) => Value::Symbol(*id),
    }
}

fn object_has_own_property(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    let key = to_property_key_descriptor(vm, args.first().unwrap_or(&Value::Undefined))?;
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    Ok(Value::Bool(object_has_own_key(vm, &this, &key)))
}

fn object_has_own(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let key = to_property_key_descriptor(vm, args.get(1).unwrap_or(&Value::Undefined))?;
    Ok(Value::Bool(object_has_own_key(vm, &obj, &key)))
}

fn object_property_is_enumerable(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let key = match args.first() {
        Some(Value::Symbol(id)) => PropertyKey::Symbol(*id),
        Some(v) => PropertyKey::from(vm.to_property_key(v)?),
        None => PropertyKey::from(""),
    };
    match &this {
        Value::Object(idx) => {
            let enumerable = vm.heap.with_obj(idx.0, |obj| {
                if let HeapObj::Array(a) = obj {
                    if key.as_str() == Some("length") {
                        return false;
                    }
                    if let Some(name) = key.as_str() {
                        if let Some(i) = crate::value::parse_array_index(name) {
                            return a.is_dense_present(i);
                        }
                    }
                }
                obj.props()
                    .lock()
                    .get(&key)
                    .is_some_and(|desc| desc.enumerable)
            });
            Ok(Value::Bool(enumerable))
        }
        Value::String(s) => {
            let enumerable = key
                .as_str()
                .and_then(|name| name.parse::<usize>().ok())
                .is_some_and(|i| i < crate::value::utf16_len(s));
            Ok(Value::Bool(enumerable))
        }
        _ => Ok(Value::Bool(false)),
    }
}

fn object_value_of(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    vm.to_object(&this)
}

fn legacy_accessor_descriptor(vm: &mut Vm, slot: &str, accessor: Value) -> error::Result<Value> {
    let idx = vm.new_object()?;
    vm.heap.with_obj(idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(PropertyKey::from(slot), data_prop(accessor));
        props.insert(
            PropertyKey::from("enumerable"),
            data_prop(Value::Bool(true)),
        );
        props.insert(
            PropertyKey::from("configurable"),
            data_prop(Value::Bool(true)),
        );
    });
    Ok(Value::Object(idx))
}

fn object_define_legacy_accessor(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    slot: &str,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let object = vm.to_object(&this)?;
    let accessor = get_arg(args, 1);
    if !is_callable(&accessor, &vm.heap) {
        return Err(Error::type_err("Accessor must be a function".to_string()));
    }
    let key = get_arg(args, 0);
    let desc = legacy_accessor_descriptor(vm, slot, accessor)?;
    let define_args = [object, key, desc];
    object_define_property_result(vm, &define_args, true)?;
    Ok(Value::Undefined)
}

fn object_define_getter(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    object_define_legacy_accessor(vm, args, this, "get")
}

fn object_define_setter(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    object_define_legacy_accessor(vm, args, this, "set")
}

fn object_lookup_legacy_accessor(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    slot: &str,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let mut object = vm.to_object(&this)?;
    let key = to_property_key_descriptor(vm, &get_arg(args, 0))?;
    for _ in 0..1024 {
        if let Some(desc) = own_property_descriptor_for_key(vm, &object, &key) {
            if desc.is_accessor {
                return Ok(match slot {
                    "get" => desc.get.unwrap_or(Value::Undefined),
                    "set" => desc.set.unwrap_or(Value::Undefined),
                    _ => Value::Undefined,
                });
            }
            return Ok(Value::Undefined);
        }
        let next = match object {
            Value::Object(idx) => vm.heap.with_obj(idx.0, |obj| obj.proto().lock().clone()),
            _ => None,
        };
        match next {
            Some(next @ Value::Object(_)) => object = next,
            _ => return Ok(Value::Undefined),
        }
    }
    Ok(Value::Undefined)
}

fn object_lookup_getter(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    object_lookup_legacy_accessor(vm, args, this, "get")
}

fn object_lookup_setter(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    object_lookup_legacy_accessor(vm, args, this, "set")
}

fn global_uri_identity(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(
        vm.to_string(args.first().unwrap_or(&Value::Undefined))?,
    ))
}

/// `Number.prototype.valueOf` / `Boolean.prototype.valueOf` /
/// `String.prototype.valueOf`: return the wrapped primitive of `this`.
fn string_proto_to_string(
    _vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    match this {
        Some(Value::String(s)) => Ok(Value::String(s)),
        Some(Value::Object(idx)) => {
            // Boxed string: extract the primitive value.
            _vm.heap.with_obj(idx.0, |o| {
                if let HeapObj::Object(od) = o {
                    if let Some(Value::String(s)) = od.primitive.lock().clone() {
                        return Ok(Value::String(s));
                    }
                }
                Ok(Value::String(Arc::from("")))
            })
        }
        _ => Ok(Value::String(Arc::from(""))),
    }
}

fn boxed_value_of(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = &this {
        let prim = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Object(od) = o {
                od.primitive.lock().clone()
            } else {
                None
            }
        });
        if let Some(p) = prim {
            return Ok(p);
        }
    }
    Ok(this.unwrap_or(Value::Undefined))
}

/// `Boolean.prototype.toString`: return "true" or "false".
fn boolean_to_string(_vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = match &this {
        Some(Value::Bool(b)) => *b,
        Some(Value::Object(idx)) => _vm
            .heap
            .with_obj(idx.0, |o| {
                if let HeapObj::Object(od) = o {
                    od.primitive.lock().clone()
                } else {
                    None
                }
            })
            .map(|v| v.is_truthy())
            .unwrap_or(false),
        _ => false,
    };
    Ok(Value::String(Arc::from(if val { "true" } else { "false" })))
}

/// `Number.prototype.toString(radix)`: convert number to string in given radix.
fn num_proto_to_string(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let radix = if args.is_empty() {
        10.0
    } else {
        vm.to_number(&args[0]).unwrap_or(10.0)
    };
    let n = match &this {
        Some(Value::Number(n)) => *n,
        Some(Value::Object(idx)) => vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Object(od) = o {
                if let Some(Value::Number(n)) = od.primitive.lock().clone() {
                    return n;
                }
            }
            f64::NAN
        }),
        _ => f64::NAN,
    };
    if radix == 10.0 {
        let s = vm.to_string(&Value::Number(n))?;
        return Ok(Value::String(s));
    }
    let radix = radix as u32;
    if !(2..=36).contains(&radix) {
        return Err(Error::range("toString() radix must be between 2 and 36"));
    }
    let s = if n.is_nan() {
        "NaN".to_string()
    } else if n.is_infinite() {
        if n > 0.0 {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        }
    } else {
        format_radix(n, radix)
    };
    Ok(Value::String(Arc::from(s.as_str())))
}

/// `Number.prototype.valueOf` (same as boxed_value_of for Number).
fn number_value_of(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = &this {
        let prim = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Object(od) = o {
                od.primitive.lock().clone()
            } else {
                None
            }
        });
        if let Some(p) = prim {
            return Ok(p);
        }
    }
    Ok(this.unwrap_or(Value::Undefined))
}

/// Format a number in a given radix (2-36). Handles integers and fractions.
fn format_radix(n: f64, radix: u32) -> String {
    if n == 0.0 {
        return "0".to_string();
    }
    let neg = n < 0.0;
    let n = n.abs();
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut int_part = n.trunc() as u64;
    let frac_part = n.fract();
    let mut int_str = String::new();
    if int_part == 0 {
        int_str.push('0');
    } else {
        while int_part > 0 {
            let d = (int_part % radix as u64) as usize;
            int_str.insert(0, digits[d] as char);
            int_part /= radix as u64;
        }
    }
    let mut result = int_str;
    if frac_part > 0.0 {
        result.push('.');
        let mut f = frac_part;
        for _ in 0..52 {
            f *= radix as f64;
            let d = f.trunc() as usize;
            if d >= radix as usize {
                break;
            }
            result.push(digits[d] as char);
            f -= d as f64;
            if f < 1e-15 {
                break;
            }
        }
    }
    if neg {
        format!("-{}", result)
    } else {
        result
    }
}

fn array_index_key(name: &str) -> Option<u32> {
    if name.is_empty()
        || !name.bytes().all(|b| b.is_ascii_digit())
        || (name.len() > 1 && name.starts_with('0'))
    {
        return None;
    }
    name.parse::<u32>()
        .ok()
        .filter(|n| (*n as u64) < (1u64 << 32) - 1)
}

fn push_unique_key(
    keys: &mut Vec<PropertyKey>,
    seen: &mut IndexSet<PropertyKey>,
    key: PropertyKey,
) {
    if seen.insert(key.clone()) {
        keys.push(key);
    }
}

pub(crate) fn own_property_keys(
    vm: &mut Vm,
    obj: &Value,
    enumerable_only: bool,
    include_strings: bool,
    include_symbols: bool,
) -> Vec<PropertyKey> {
    if let Value::Object(idx) = obj {
        if let Some((target, handler)) = vm.heap.with_obj(idx.0, |heap_obj| {
            if let HeapObj::Proxy(proxy) = heap_obj {
                if *proxy.revoked.lock() {
                    None
                } else {
                    Some((proxy.target.clone(), proxy.handler.clone()))
                }
            } else {
                None
            }
        }) {
            if let Ok(trap) = vm.get_property(&handler, "ownKeys") {
                if !trap.is_undefined() {
                    if let Ok(key_list) =
                        vm.call_function(&trap, std::slice::from_ref(&target), Some(handler))
                    {
                        let items = if let Value::Object(list_idx) = &key_list {
                            vm.heap.with_obj(list_idx.0, |o| {
                                if let HeapObj::Array(a) = o {
                                    return Some(a.items.lock().clone());
                                }
                                None
                            })
                        } else {
                            None
                        };
                        if let Some(items) = items {
                            let mut keys = Vec::new();
                            let mut seen = IndexSet::new();
                            for item in items {
                                let Ok(key) = to_property_key_descriptor(vm, &item) else {
                                    continue;
                                };
                                if enumerable_only
                                    && !own_property_descriptor_for_key(vm, &target, &key)
                                        .is_some_and(|desc| desc.enumerable)
                                {
                                    continue;
                                }
                                match key {
                                    PropertyKey::Str(_) if include_strings => {
                                        push_unique_key(&mut keys, &mut seen, key);
                                    }
                                    PropertyKey::Symbol(_) if include_symbols => {
                                        push_unique_key(&mut keys, &mut seen, key);
                                    }
                                    _ => {}
                                }
                            }
                            return keys;
                        }
                    }
                }
            }
            return own_property_keys(
                vm,
                &target,
                enumerable_only,
                include_strings,
                include_symbols,
            );
        }
    }

    let mut keys = Vec::new();
    let mut seen = IndexSet::new();
    match obj {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |o| {
            let mut index_keys: Vec<u32> = Vec::new();
            let mut string_keys: Vec<PropertyKey> = Vec::new();
            let mut symbol_keys: Vec<PropertyKey> = Vec::new();

            if let HeapObj::Array(a) = o {
                if include_strings {
                    for (i, present) in a.present.lock().iter().copied().enumerate() {
                        if present {
                            index_keys.push(i as u32);
                        }
                    }
                    if !enumerable_only {
                        string_keys.push(PropertyKey::from("length"));
                    }
                }
            }

            if let HeapObj::Object(od) = o {
                if include_strings {
                    if let Some(Value::String(s)) = od.primitive.lock().clone() {
                        for i in 0..crate::value::utf16_len(&s) {
                            index_keys.push(i as u32);
                        }
                        if !enumerable_only {
                            string_keys.push(PropertyKey::from("length"));
                        }
                    }
                }
            }

            if let HeapObj::Map(m) = o {
                if include_strings {
                    for (k, _) in m.entries.lock().iter().map(|(k, v)| (&k.0, v)) {
                        if let Value::String(s) = k {
                            string_keys.push(PropertyKey::from(s.clone()));
                        }
                    }
                }
            }

            for (k, desc) in o.props().lock().iter() {
                if enumerable_only && !desc.enumerable {
                    continue;
                }
                match k {
                    PropertyKey::Str(s) if include_strings => {
                        if let Some(index) = array_index_key(s) {
                            index_keys.push(index);
                        } else {
                            string_keys.push(PropertyKey::from(s.clone()));
                        }
                    }
                    PropertyKey::Symbol(id) if include_symbols => {
                        symbol_keys.push(PropertyKey::Symbol(*id));
                    }
                    _ => {}
                }
            }

            index_keys.sort_unstable();
            index_keys.dedup();
            for n in index_keys {
                push_unique_key(
                    &mut keys,
                    &mut seen,
                    PropertyKey::from(n.to_string().as_str()),
                );
            }
            for key in string_keys {
                push_unique_key(&mut keys, &mut seen, key);
            }
            for key in symbol_keys {
                push_unique_key(&mut keys, &mut seen, key);
            }
        }),
        Value::String(s) if include_strings => {
            for i in 0..crate::value::utf16_len(s) {
                push_unique_key(
                    &mut keys,
                    &mut seen,
                    PropertyKey::from(i.to_string().as_str()),
                );
            }
            if !enumerable_only {
                push_unique_key(&mut keys, &mut seen, PropertyKey::from("length"));
            }
        }
        _ => {}
    }
    keys
}

/// Collect an object's own enumerable string keys in array-index-first then property order.
pub(crate) fn own_string_keys(vm: &mut Vm, obj: &Value) -> Vec<Arc<str>> {
    own_property_keys(vm, obj, true, true, false)
        .into_iter()
        .filter_map(|key| match key {
            PropertyKey::Str(s) => Some(s),
            PropertyKey::Symbol(_) => None,
        })
        .collect()
}

pub(crate) fn make_value_array(vm: &mut Vm, items: Vec<Value>) -> error::Result<Value> {
    let arr = HeapObj::Array(ArrayData::new(items, Some(vm.array_proto.clone())));
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr)?)))
}
pub(crate) fn norm_idx(n: f64, len: f64) -> f64 {
    if n < 0.0 {
        (len + n).max(0.0)
    } else {
        n.min(len)
    }
}

pub(crate) fn make_str_array(vm: &mut Vm, strs: Vec<Arc<str>>) -> error::Result<Value> {
    let items: Vec<Value> = strs.into_iter().map(Value::String).collect();
    let arr = HeapObj::Array(ArrayData::new(items, Some(vm.array_proto.clone())));
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr)?)))
}

fn object_keys(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let keys = own_string_keys(vm, &obj);
    make_str_array(vm, keys)
}

fn object_values(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let keys = own_string_keys(vm, &obj);
    let mut vals = Vec::with_capacity(keys.len());
    for k in &keys {
        if !own_property_descriptor_for_key(vm, &obj, &PropertyKey::from(k.clone()))
            .is_some_and(|desc| desc.enumerable)
        {
            continue;
        }
        vals.push(vm.get_property(&obj, k)?);
    }
    let arr = HeapObj::Array(ArrayData::new(vals, Some(vm.array_proto.clone())));
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr)?)))
}

fn object_entries(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let keys = own_string_keys(vm, &obj);
    let mut pairs = Vec::new();
    for k in keys {
        if !own_property_descriptor_for_key(vm, &obj, &PropertyKey::from(k.clone()))
            .is_some_and(|desc| desc.enumerable)
        {
            continue;
        }
        let v = vm.get_property(&obj, &k)?;
        let pair = HeapObj::Array(ArrayData::new(
            vec![Value::String(k.clone()), v],
            Some(vm.array_proto.clone()),
        ));
        pairs.push(Value::Object(GcIdx(vm.heap.allocate(pair)?)));
    }
    let arr = HeapObj::Array(ArrayData::new(pairs, Some(vm.array_proto.clone())));
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr)?)))
}

fn object_assign(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if target.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let to = vm.to_object(&target)?;
    for src in &args[1..] {
        if src.is_nullish() {
            continue;
        }
        let from = vm.to_object(src)?;
        let keys = own_property_keys(vm, &from, false, true, true);
        for k in keys {
            if !own_property_descriptor_for_key(vm, &from, &k).is_some_and(|desc| desc.enumerable) {
                continue;
            }
            let v = vm.get_property_by_key(&from, &k)?;
            if !vm.try_set_property_key_with_receiver(&to, &k, v, &to)? {
                return Err(Error::type_err("Cannot assign to read only property"));
            }
        }
    }
    Ok(to)
}

fn object_is(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let a = args.first().cloned().unwrap_or(Value::Undefined);
    let b = args.get(1).cloned().unwrap_or(Value::Undefined);
    // Object.is: SameValue (distinguishes -0/+0 and treats NaN as equal)
    let same = match (&a, &b) {
        (Value::Number(x), Value::Number(y)) => {
            if x.is_nan() && y.is_nan() {
                true
            } else if *x == 0.0 && *y == 0.0 {
                x.is_sign_negative() == y.is_sign_negative()
            } else {
                x == y
            }
        }
        _ => vm.strict_eq(&a, &b),
    };
    Ok(Value::Bool(same))
}
fn object_from_entries(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let entries = args.first().cloned().unwrap_or(Value::Undefined);
    if entries.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    // Accept an array (or array-like) of [key, value] pairs.
    if let Value::Object(arr_idx) = &entries {
        let pairs: Vec<Value> = vm.heap.with_obj(arr_idx.0, |o| {
            if let HeapObj::Array(a) = o {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        for pair in &pairs {
            // Each entry object is read through Get(entry, "0") / Get(entry, "1").
            if !matches!(pair, Value::Object(_)) {
                return Err(Error::type_err("Iterator value is not an entry object"));
            }
            let key = vm.get_property_by_key(pair, &PropertyKey::from("0"))?;
            let value = vm.get_property_by_key(pair, &PropertyKey::from("1"))?;
            let key = to_property_key_descriptor(vm, &key)?;
            vm.heap.with_obj(obj_idx, |o| {
                if let HeapObj::Object(obj) = o {
                    // Own enumerable data property (data_prop is
                    // non-enumerable, which would hide it from
                    // Object.keys / JSON.stringify).
                    obj.props.lock().insert(
                        key,
                        PropertyDescriptor {
                            value,
                            writable: true,
                            enumerable: true,
                            configurable: true,
                            get: None,
                            set: None,
                            is_accessor: false,
                        },
                    );
                }
            });
        }
    }
    Ok(Value::Object(GcIdx(obj_idx)))
}
fn object_create(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let proto = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(proto, Value::Object(_) | Value::Null) {
        return Err(Error::type_err(
            "Object prototype may only be an Object or null",
        ));
    }
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(if proto.is_null() { None } else { Some(proto) }),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let obj = Value::Object(GcIdx(obj_idx));
    if let Some(props) = args.get(1) {
        if !props.is_undefined() {
            object_define_properties(vm, &[obj.clone(), props.clone()], None)?;
        }
    }
    Ok(obj)
}
fn object_get_own_property_names(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let keys: Vec<Arc<str>> = own_property_keys(vm, &obj, false, true, false)
        .into_iter()
        .filter_map(|key| match key {
            PropertyKey::Str(s) => Some(s),
            PropertyKey::Symbol(_) => None,
        })
        .collect();
    make_str_array(vm, keys)
}

fn object_get_own_property_symbols(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let symbols: Vec<Value> = own_property_keys(vm, &obj, false, false, true)
        .into_iter()
        .filter_map(|key| match key {
            PropertyKey::Symbol(id) => Some(Value::Symbol(id)),
            PropertyKey::Str(_) => None,
        })
        .collect();
    make_value_array(vm, symbols)
}

fn object_get_prototype_of(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if obj.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let object = vm.to_object(&obj)?;
    if let Value::Object(idx) = &object {
        return Ok(vm
            .heap
            .with_obj(idx.0, |o| o.proto().lock().clone().unwrap_or(Value::Null)));
    }
    Ok(Value::Null)
}

pub(crate) fn object_set_prototype_of(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    let proto = args.get(1).cloned().unwrap_or(Value::Undefined);
    if obj.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let p = prototype_arg(proto)?;
    if matches!(obj, Value::Object(_)) && !vm.set_prototype_of(&obj, p)? {
        return Err(Error::type_err("Cannot mutate object prototype"));
    }
    Ok(obj)
}

pub(crate) fn reflect_set_prototype_of_result(vm: &mut Vm, args: &[Value]) -> error::Result<bool> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(obj, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.setPrototypeOf target must be an object",
        ));
    }
    let proto = args.get(1).cloned().unwrap_or(Value::Undefined);
    vm.set_prototype_of(&obj, prototype_arg(proto)?)
}

fn prototype_arg(proto: Value) -> error::Result<Option<Value>> {
    if proto.is_null() {
        Ok(None)
    } else if matches!(proto, Value::Object(_)) {
        Ok(Some(proto))
    } else {
        Err(Error::type_err(
            "Object prototype may only be an Object or null",
        ))
    }
}

fn object_proto_get(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let object = vm.to_object(&this)?;
    if let Value::Object(idx) = object {
        return Ok(vm
            .heap
            .with_obj(idx.0, |o| o.proto().lock().clone().unwrap_or(Value::Null)));
    }
    Ok(Value::Undefined)
}

fn object_proto_set(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let proto = args.first().cloned().unwrap_or(Value::Undefined);
    let p = match proto {
        Value::Object(_) => Some(proto),
        Value::Null => None,
        _ => return Ok(Value::Undefined),
    };
    if !matches!(this, Value::Object(_)) {
        return Ok(Value::Undefined);
    }
    if !vm.set_prototype_of(&this, p)? {
        return Err(Error::type_err("Cannot mutate object prototype"));
    }
    Ok(Value::Undefined)
}

pub(crate) fn reflect_get_prototype_of_strict(vm: &mut Vm, args: &[Value]) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(obj, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.getPrototypeOf target must be an object",
        ));
    }
    object_get_prototype_of(vm, args, None)
}

fn object_prevent_extensions(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &obj {
        vm.heap.with_obj(idx.0, |o| match o {
            HeapObj::Object(od) => od.extensible.store(false, Ordering::Relaxed),
            HeapObj::Function(f) => f.extensible.store(false, Ordering::Relaxed),
            _ => {}
        });
    }
    Ok(obj)
}

fn object_is_extensible(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &obj {
        let ext = vm.heap.with_obj(idx.0, |o| o.is_extensible());
        return Ok(Value::Bool(ext));
    }
    Ok(Value::Bool(true))
}

fn object_seal(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &obj {
        vm.heap.with_obj(idx.0, |o| match o {
            HeapObj::Object(od) => {
                od.extensible.store(false, Ordering::Relaxed);
                for d in od.props.lock().values_mut() {
                    d.configurable = false;
                }
            }
            HeapObj::Function(f) => {
                f.extensible.store(false, Ordering::Relaxed);
                for d in f.props.lock().values_mut() {
                    d.configurable = false;
                }
            }
            _ => {}
        });
    }
    Ok(obj)
}

fn object_is_sealed(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &obj {
        let sealed = vm.heap.with_obj(idx.0, |o| match o {
            HeapObj::Object(od) => od.props.lock().values().all(|d| !d.configurable),
            HeapObj::Array(_) => false,
            _ => true,
        });
        return Ok(Value::Bool(sealed));
    }
    Ok(Value::Bool(true))
}

fn object_is_frozen(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &obj {
        let frozen = vm.heap.with_obj(idx.0, |o| match o {
            HeapObj::Object(od) => {
                let ext = od.extensible.load(Ordering::Relaxed);
                let all_frozen = od
                    .props
                    .lock()
                    .values()
                    .all(|d| !d.configurable && !d.writable && !d.is_accessor);
                !ext && all_frozen
            }
            HeapObj::Array(_) => false,
            _ => true,
        });
        return Ok(Value::Bool(frozen));
    }
    Ok(Value::Bool(true))
}

fn object_get_own_property_descriptors(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let result_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let keys = own_property_keys(vm, &obj, false, true, true);
    let mut props = IndexMap::new();
    for key in keys {
        if let Some(desc) = own_property_descriptor_for_key(vm, &obj, &key) {
            props.insert(
                key,
                PropertyDescriptor::data(from_property_descriptor(vm, desc)?),
            );
        }
    }
    vm.heap.with_obj(result_idx, |o| {
        if let HeapObj::Object(od) = o {
            *od.props.lock() = props;
        }
    });
    Ok(Value::Object(GcIdx(result_idx)))
}

fn object_define_properties(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    let props = args.get(1).cloned().unwrap_or(Value::Undefined);
    // Collect (key, descriptor) pairs first to avoid borrowing vm during iteration.
    let pairs: Vec<(String, Value)> = if let Value::Object(_) = &props {
        let keys = own_string_keys(vm, &props);
        keys.into_iter()
            .filter_map(|k| {
                let desc = vm.get_property(&props, &k).ok()?;
                if desc.is_undefined() {
                    None
                } else {
                    Some((k.to_string(), desc))
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    for (key, desc) in pairs {
        let dp = vec![obj.clone(), Value::String(Arc::from(key.as_str())), desc];
        object_define_property(vm, &dp, None)?;
    }
    Ok(obj)
}

fn canonical_string_index(key: &PropertyKey) -> Option<usize> {
    let name = key.as_str()?;
    let index = name.parse::<usize>().ok()?;
    if index.to_string() == name {
        Some(index)
    } else {
        None
    }
}

fn string_exotic_own_property_descriptor(s: &str, key: &PropertyKey) -> Option<PropertyDescriptor> {
    if key.as_str() == Some("length") {
        let mut desc = PropertyDescriptor::data(Value::Number(crate::value::utf16_len(s) as f64));
        desc.writable = false;
        desc.enumerable = false;
        desc.configurable = false;
        return Some(desc);
    }

    let index = canonical_string_index(key)?;
    let unit = crate::value::utf16_get(s, index)?;
    let mut desc = PropertyDescriptor::data(Value::String(Arc::from(
        crate::value::utf16_to_string(&[unit]).as_str(),
    )));
    desc.writable = false;
    desc.enumerable = true;
    desc.configurable = false;
    Some(desc)
}

fn own_property_descriptor_for_key(
    vm: &mut Vm,
    obj: &Value,
    key: &PropertyKey,
) -> Option<PropertyDescriptor> {
    if let Value::Object(idx) = obj {
        if let Some(target) = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Proxy(proxy) = o {
                if *proxy.revoked.lock() {
                    None
                } else {
                    Some(proxy.target.clone())
                }
            } else {
                None
            }
        }) {
            return own_property_descriptor_for_key(vm, &target, key);
        }

        let array_descriptor = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Array(a) = o {
                if key.as_str() == Some("length") {
                    if let Some(desc) = a.props.lock().get(key).cloned() {
                        return Some(desc);
                    }
                    if a.is_arguments.load(Ordering::Relaxed) {
                        return None;
                    }
                    let mut desc =
                        PropertyDescriptor::data(Value::Number(a.items.lock().len() as f64));
                    desc.writable = true;
                    desc.enumerable = false;
                    desc.configurable = false;
                    return Some(desc);
                }
            }
            None
        });
        if let Some(desc) = array_descriptor {
            return Some(desc);
        }
        let is_array = vm.heap.with_obj(idx.0, |o| matches!(o, HeapObj::Array(_)));
        if is_array {
            if let Some(i) = canonical_string_index(key) {
                return vm.array_index_own_property_descriptor(idx.0, i, key);
            }
        }
    }

    match obj {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |o| {
            let ordinary = o.props().lock().get(key).cloned();
            if ordinary.is_some() {
                return ordinary;
            }

            if let HeapObj::Object(od) = o {
                if let Some(Value::String(s)) = od.primitive.lock().clone() {
                    return string_exotic_own_property_descriptor(&s, key);
                }
            }

            None
        }),
        Value::String(s) => string_exotic_own_property_descriptor(s, key),
        _ => None,
    }
}

fn from_property_descriptor(vm: &mut Vm, desc: PropertyDescriptor) -> error::Result<Value> {
    let desc_obj = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let mut props = IndexMap::new();
    if desc.is_accessor {
        props.insert(
            PropertyKey::from("get"),
            PropertyDescriptor::data(desc.get.unwrap_or(Value::Undefined)),
        );
        props.insert(
            PropertyKey::from("set"),
            PropertyDescriptor::data(desc.set.unwrap_or(Value::Undefined)),
        );
    } else {
        props.insert(
            PropertyKey::from("value"),
            PropertyDescriptor::data(desc.value),
        );
        props.insert(
            PropertyKey::from("writable"),
            PropertyDescriptor::data(Value::Bool(desc.writable)),
        );
    }
    props.insert(
        PropertyKey::from("enumerable"),
        PropertyDescriptor::data(Value::Bool(desc.enumerable)),
    );
    props.insert(
        PropertyKey::from("configurable"),
        PropertyDescriptor::data(Value::Bool(desc.configurable)),
    );
    vm.heap.with_obj(desc_obj, |o| {
        if let HeapObj::Object(od) = o {
            *od.props.lock() = props;
        }
    });
    Ok(Value::Object(GcIdx(desc_obj)))
}

fn object_get_own_property_descriptor(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let object = args.first().unwrap_or(&Value::Undefined);
    if object.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let obj = vm.to_object(object)?;
    let key = to_property_key_descriptor(vm, args.get(1).unwrap_or(&Value::Undefined))?;
    if let Value::Object(idx) = &obj {
        if let Some(proxy_result) = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Proxy(proxy) = o {
                if *proxy.revoked.lock() {
                    return Some(Err(Error::type_err(
                        "Cannot perform 'getOwnPropertyDescriptor' on a proxy that has been revoked",
                    )));
                }
                Some(Ok((proxy.target.clone(), proxy.handler.clone())))
            } else {
                None
            }
        }) {
            let (target, handler) = proxy_result?;
            let key_value = property_key_to_value(&key);
            let trap = vm.get_property(&handler, "getOwnPropertyDescriptor")?;
            if trap.is_undefined() {
                return object_get_own_property_descriptor(vm, &[target, key_value], None);
            }
            let result = vm.call_function(&trap, &[target, key_value], Some(handler))?;
            if result.is_undefined() {
                return Ok(Value::Undefined);
            }
            if !matches!(result, Value::Object(_)) {
                return Err(Error::type_err(
                    "Proxy getOwnPropertyDescriptor trap must return an object or undefined",
                ));
            }
            return Ok(result);
        }
    }
    match own_property_descriptor_for_key(vm, &obj, &key) {
        Some(desc) => from_property_descriptor(vm, desc),
        None => Ok(Value::Undefined),
    }
}

fn object_freeze(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = target {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Object(o) = obj {
                o.extensible.store(false, Ordering::Relaxed);
                for d in o.props.lock().values_mut() {
                    d.writable = false;
                    d.configurable = false;
                }
            }
        });
    }
    Ok(target)
}

fn object_define_property(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    object_define_property_result(vm, args, true)?;
    Ok(target)
}

pub(crate) fn object_define_property_result(
    vm: &mut Vm,
    args: &[Value],
    throw_on_failure: bool,
) -> error::Result<bool> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let key = args
        .get(1)
        .map(|v| to_property_key_descriptor(vm, v))
        .transpose()?
        .unwrap_or_else(|| PropertyKey::from(""));
    let desc = args.get(2).cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = target {
        let mut value = Value::Undefined;
        let mut writable = false;
        let mut enumerable = false;
        let mut configurable = false;
        let mut get = None;
        let mut set = None;
        let mut has_value = false;
        let mut has_writable = false;
        let mut has_enumerable = false;
        let mut has_configurable = false;
        let mut has_get = false;
        let mut has_set = false;
        // ToPropertyDescriptor: the descriptor must be an Object, else a

        // TypeError. Without this, Object.defineProperty(o, "x", true)

        // silently succeeded instead of throwing (diverging from V8/Node).

        if !matches!(desc, Value::Object(_)) {
            return Err(Error::type_err(format!(
                "Property description must be an object: {}",
                crate::value::value_to_debug_string(&desc)
            )));
        }

        if let Some(proxy_result) = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Proxy(proxy) = obj {
                if *proxy.revoked.lock() {
                    return Some(Err(Error::type_err(
                        "Cannot perform 'defineProperty' on a proxy that has been revoked",
                    )));
                }
                Some(Ok((proxy.target.clone(), proxy.handler.clone())))
            } else {
                None
            }
        }) {
            let (proxy_target, proxy_handler) = proxy_result?;
            let trap = vm.get_property(&proxy_handler, "defineProperty")?;
            let key_value = property_key_to_value(&key);
            if trap.is_undefined() {
                return object_define_property_result(
                    vm,
                    &[proxy_target, key_value, desc.clone()],
                    throw_on_failure,
                );
            }

            let trap_result = vm.call_function(
                &trap,
                &[proxy_target, key_value, desc.clone()],
                Some(proxy_handler),
            )?;
            if !trap_result.is_truthy() {
                if throw_on_failure {
                    return Err(Error::type_err("Proxy defineProperty trap returned false"));
                }
                return Ok(false);
            }
            return Ok(true);
        }

        if let Value::Object(_) = desc {
            // Presence of each field is determined by an OWN property on the
            // descriptor object, mirroring ToPropertyDescriptor: a missing
            // field must NOT flip the has_* flags, otherwise a plain
            // `{value: 1, writable: false}` descriptor would be misread as
            // an accessor (get/set absent but `get_property` returns
            // `Ok(undefined)`).
            if vm.has_own(&desc, "value") {
                value = vm.get_property(&desc, "value")?;
                has_value = true;
            }
            if vm.has_own(&desc, "writable") {
                writable = vm.get_property(&desc, "writable")?.is_truthy();
                has_writable = true;
            }
            if vm.has_own(&desc, "get") {
                let v = vm.get_property(&desc, "get")?;
                if !v.is_undefined() && !is_callable(&v, &vm.heap) {
                    return Err(Error::type_err("Getter must be a function"));
                }
                get = if v.is_undefined() { None } else { Some(v) };
                has_get = true;
            }
            if vm.has_own(&desc, "set") {
                let v = vm.get_property(&desc, "set")?;
                if !v.is_undefined() && !is_callable(&v, &vm.heap) {
                    return Err(Error::type_err("Setter must be a function"));
                }
                set = if v.is_undefined() { None } else { Some(v) };
                has_set = true;
            }
            if vm.has_own(&desc, "enumerable") {
                enumerable = vm.get_property(&desc, "enumerable")?.is_truthy();
                has_enumerable = true;
            }
            if vm.has_own(&desc, "configurable") {
                configurable = vm.get_property(&desc, "configurable")?.is_truthy();
                has_configurable = true;
            }
        }
        // A descriptor is an accessor descriptor if it has get/set, and a
        // data descriptor if it has value/writable. Mixing the two is a
        // TypeError per [[DefineOwnProperty]].
        let is_accessor = has_get || has_set;
        let is_data = has_value || has_writable;
        if is_accessor && is_data {
            return Err(Error::type_err(
                "Invalid property descriptor. Cannot both specify accessors and a value or writable attribute",
            ));
        }
        let current = own_property_descriptor_for_key(vm, &target, &key);
        let mapped_arguments_index = key
            .as_str()
            .and_then(crate::value::parse_array_index)
            .and_then(|i| {
                vm.arguments_mapped_binding_for_index(idx.0, i)
                    .map(|mapped| (i, mapped))
            });
        if current.is_none() {
            let extensible = vm.heap.with_obj(idx.0, |obj| obj.is_extensible());
            if !extensible {
                if throw_on_failure {
                    return Err(Error::type_err(format!(
                        "Cannot define property '{}', object is not extensible",
                        key.as_str().unwrap_or("Symbol")
                    )));
                }
                return Ok(false);
            }
        }
        let map_value = value.clone();
        let descriptor = if let Some(mut current) = current {
            if !current.configurable {
                if has_configurable && configurable {
                    if throw_on_failure {
                        return Err(Error::type_err("Cannot redefine non-configurable property"));
                    }
                    return Ok(false);
                }
                if has_enumerable && enumerable != current.enumerable {
                    if throw_on_failure {
                        return Err(Error::type_err("Cannot redefine non-configurable property"));
                    }
                    return Ok(false);
                }
                if is_accessor != current.is_accessor && (is_accessor || is_data) {
                    if throw_on_failure {
                        return Err(Error::type_err("Cannot redefine non-configurable property"));
                    }
                    return Ok(false);
                }
                if current.is_accessor {
                    if has_get && get != current.get {
                        if throw_on_failure {
                            return Err(Error::type_err(
                                "Cannot redefine non-configurable property",
                            ));
                        }
                        return Ok(false);
                    }
                    if has_set && set != current.set {
                        if throw_on_failure {
                            return Err(Error::type_err(
                                "Cannot redefine non-configurable property",
                            ));
                        }
                        return Ok(false);
                    }
                } else if is_data && !current.writable {
                    if has_writable && writable {
                        if throw_on_failure {
                            return Err(Error::type_err(
                                "Cannot redefine non-configurable property",
                            ));
                        }
                        return Ok(false);
                    }
                    if has_value && value != current.value {
                        if throw_on_failure {
                            return Err(Error::type_err(
                                "Cannot redefine non-configurable property",
                            ));
                        }
                        return Ok(false);
                    }
                }
            }
            if has_enumerable {
                current.enumerable = enumerable;
            }
            if has_configurable {
                current.configurable = configurable;
            }
            if is_accessor {
                current.value = Value::Undefined;
                current.writable = false;
                if has_get {
                    current.get = get;
                }
                if has_set {
                    current.set = set;
                }
                current.is_accessor = true;
            } else if is_data {
                if has_value {
                    current.value = value;
                }
                if has_writable {
                    current.writable = writable;
                }
                current.get = None;
                current.set = None;
                current.is_accessor = false;
            }
            current
        } else if is_accessor {
            PropertyDescriptor {
                value: Value::Undefined,
                writable: false,
                enumerable,
                configurable,
                get,
                set,
                is_accessor: true,
            }
        } else if is_data {
            PropertyDescriptor {
                value,
                writable,
                enumerable,
                configurable,
                get: None,
                set: None,
                is_accessor: false,
            }
        } else {
            // Generic descriptor (only enumerable/configurable).
            PropertyDescriptor {
                value: Value::Undefined,
                writable: false,
                enumerable,
                configurable,
                get: None,
                set: None,
                is_accessor: false,
            }
        };
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                if let Some(i) = key.as_str().and_then(crate::value::parse_array_index) {
                    if i >= a.items.lock().len() {
                        let new_len = i + 1;
                        if new_len <= crate::value::MAX_DENSE_ARRAY_LEN {
                            let mut items = a.items.lock();
                            let mut present = a.present.lock();
                            while items.len() < new_len {
                                items.push(Value::Undefined);
                                present.push(false);
                            }
                            *a.sparse_max.lock() = None;
                        } else {
                            *a.sparse_max.lock() = Some(new_len);
                        }
                    }
                }
            }
            obj.props().lock().insert(key.clone(), descriptor);
        });
        if let Some((i, (env, name))) = mapped_arguments_index {
            if is_accessor {
                vm.remove_arguments_mapping_for_index(idx.0, i);
            } else {
                if has_value {
                    crate::environment::set(&vm.heap, env, &name, map_value);
                }
                if has_writable && !writable {
                    vm.remove_arguments_mapping_for_index(idx.0, i);
                }
            }
        }
        if let Some(key) = key.as_str() {
            vm.ic_invalidate(idx.0, key);
        }
    }
    Ok(true)
}

// Minimal stubs to keep the crate compiling while parser/lexer work is in progress.

fn active_error_constructor_prototype(vm: &mut Vm) -> error::Result<Value> {
    if let Some(callee) = vm.current_native_callee.clone() {
        let proto = vm.get_property_by_key(&callee, &PropertyKey::from("prototype"))?;
        if matches!(proto, Value::Object(_)) {
            return Ok(proto);
        }
    }
    Ok(vm.error_proto.clone())
}

fn new_error_object(vm: &mut Vm, proto: Value) -> error::Result<GcIdx> {
    let obj = HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(proto)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Error")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    Ok(GcIdx(vm.heap.allocate(obj)?))
}

fn error_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let msg = match args.first() {
        Some(Value::Undefined) | None => None,
        Some(v) => Some(vm.to_string(v)?),
    };
    // Use the `this` provided by `construct` (already linked to <Error>.prototype).
    // When called as a plain function (Error(msg) without `new`), `this` is
    // undefined (strict) or the global object (sloppy). In sloppy mode we
    // detect the global object by checking its class_name; in strict mode
    // `this` is None. Both cases create a fresh object. But `construct`
    // passes a fresh object with class_name=None, so we must NOT treat
    // that as "not an error" — only reject the global object.
    let idx = match this {
        Some(Value::Object(i)) => {
            // Check if `this` is the global object (sloppy-mode plain call).
            // The global object has class_name "global". A fresh object from
            // `construct` has class_name None.
            let is_global = vm.heap.with_obj(i.0, |obj| {
                if let HeapObj::Object(o) = obj {
                    o.class_name.as_deref() == Some("global")
                } else {
                    false
                }
            });
            if is_global {
                let proto = active_error_constructor_prototype(vm)?;
                new_error_object(vm, proto)?
            } else {
                i
            }
        }
        _ => {
            // Called as Error(msg) or TypeError(msg) without new: create a
            // fresh object from the active constructor's prototype.
            let proto = active_error_constructor_prototype(vm)?;
            new_error_object(vm, proto)?
        }
    };
    // Optional `cause` from the options object (second argument).
    let cause = args.get(1).and_then(|v| {
        if let Value::Object(oi) = v {
            vm.heap.with_obj(oi.0, |o| {
                o.props()
                    .lock()
                    .get(&PropertyKey::from("cause"))
                    .map(|d| d.value.clone())
            })
        } else {
            None
        }
    });
    vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::Object(o) = obj {
            let mut props = o.props.lock();
            if let Some(msg) = msg {
                props.insert(PropertyKey::from("message"), data_prop(Value::String(msg)));
            }
            if let Some(c) = cause {
                props.insert(PropertyKey::from("cause"), data_prop(c));
            }
        }
    });
    Ok(Value::Object(idx))
}

pub fn setup(vm: &mut Vm) -> error::Result<()> {
    let (object_ctor, object_proto) = make_builtin_constructor(
        vm,
        "Object",
        &[
            ("toString", object_to_string_native, 0),
            ("toLocaleString", object_to_locale_string, 0),
            ("hasOwnProperty", object_has_own_property, 1),
            ("isPrototypeOf", object_is_prototype_of, 1),
            ("propertyIsEnumerable", object_property_is_enumerable, 1),
            ("valueOf", object_value_of, 0),
            ("__defineGetter__", object_define_getter, 2),
            ("__defineSetter__", object_define_setter, 2),
            ("__lookupGetter__", object_lookup_getter, 1),
            ("__lookupSetter__", object_lookup_setter, 1),
        ],
    )?;
    // Object static methods
    for (n, f, len) in [
        ("keys", object_keys as NativeFn, 1),
        ("values", object_values as NativeFn, 1),
        ("entries", object_entries as NativeFn, 1),
        ("assign", object_assign as NativeFn, 2),
        ("is", object_is as NativeFn, 2),
        ("hasOwn", object_has_own as NativeFn, 2),
        ("fromEntries", object_from_entries as NativeFn, 1),
        ("create", object_create as NativeFn, 2),
        ("freeze", object_freeze as NativeFn, 1),
        (
            "getOwnPropertyNames",
            object_get_own_property_names as NativeFn,
            1,
        ),
        (
            "getOwnPropertySymbols",
            object_get_own_property_symbols as NativeFn,
            1,
        ),
        (
            "getOwnPropertyDescriptor",
            object_get_own_property_descriptor as NativeFn,
            2,
        ),
        ("defineProperty", object_define_property as NativeFn, 3),
        ("defineProperties", object_define_properties as NativeFn, 2),
        ("getPrototypeOf", object_get_prototype_of as NativeFn, 1),
        ("setPrototypeOf", object_set_prototype_of as NativeFn, 2),
        (
            "preventExtensions",
            object_prevent_extensions as NativeFn,
            1,
        ),
        ("isExtensible", object_is_extensible as NativeFn, 1),
        ("seal", object_seal as NativeFn, 1),
        ("isSealed", object_is_sealed as NativeFn, 1),
        ("isFrozen", object_is_frozen as NativeFn, 1),
        (
            "getOwnPropertyDescriptors",
            object_get_own_property_descriptors as NativeFn,
            1,
        ),
    ] {
        let m = vm.new_native_function(n, f, len)?;
        vm.heap.with_obj(object_ctor.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from(n), data_prop(Value::Object(m)));
        });
    }
    define_global(vm, "Object", Value::Object(object_ctor));
    vm.object_proto = Value::Object(object_proto);
    let proto_get = vm.new_native_function("get __proto__", object_proto_get, 0)?;
    let proto_set = vm.new_native_function("set __proto__", object_proto_set, 1)?;
    vm.heap.with_obj(object_proto.0, |obj| {
        *obj.proto().lock() = None;
        obj.props().lock().insert(
            PropertyKey::from("__proto__"),
            accessor_prop(Value::Object(proto_get), Value::Object(proto_set)),
        );
    });

    let (error_ctor, error_proto) = make_error_constructor(vm, "Error")?;
    vm.error_proto = Value::Object(error_proto);
    define_global(vm, "Error", Value::Object(error_ctor));
    for name in [
        "TypeError",
        "RangeError",
        "ReferenceError",
        "SyntaxError",
        "EvalError",
        "URIError",
        "AggregateError",
    ] {
        let (ctor, _) = make_error_constructor(vm, name)?;
        define_global(vm, name, Value::Object(ctor));
    }
    Ok(())
}

// =========================================================================
// Extended setup
// =========================================================================
pub fn setup_full(vm: &mut Vm) -> error::Result<()> {
    // Allocate Function.prototype first so that every function created during
    // the rest of bootstrap inherits call/apply/bind via its [[Prototype]].
    let function_proto_idx =
        vm.new_native_function("Function.prototype", function_proto_noop, 0)?;
    vm.function_proto = Value::Object(function_proto_idx);
    setup(vm)?;
    // Per spec, Function.prototype's [[Prototype]] is Object.prototype.
    // (Function.prototype is itself a function, but it inherits Object.prototype
    // methods like isPrototypeOf, hasOwnProperty, toString, etc.)
    vm.heap.with_obj(function_proto_idx.0, |obj| {
        *obj.proto().lock() = Some(vm.object_proto.clone());
    });
    init_global_this(vm)?;
    // Math
    let math = build_math(vm)?;
    define_global(vm, "Math", math);
    // console
    let console = build_console(vm)?;
    define_global(vm, "console", console);
    // JSON
    let json = build_json(vm)?;
    define_global(vm, "JSON", json);
    // Reflect
    let reflect = build_reflect(vm)?;
    define_global(vm, "Reflect", reflect);

    // Proxy constructor + revocable.
    let proxy_ctor_idx = vm.new_native_function("Proxy", proxy_constructor, 2)?;
    vm.heap.with_obj(proxy_ctor_idx.0, |o| {
        if let HeapObj::Function(f) = o {
            f.prototype.lock().replace(Value::Undefined);
        }
    });
    let proxy_rev_idx = vm.new_native_function("revocable", proxy_revocable, 2)?;
    vm.heap.with_obj(proxy_ctor_idx.0, |o| {
        if let HeapObj::Function(f) = o {
            f.props.lock().insert(
                PropertyKey::from("revocable"),
                data_prop(Value::Object(proxy_rev_idx)),
            );
        }
    });
    define_global(vm, "Proxy", Value::Object(proxy_ctor_idx));

    let (array_buffer_ctor, array_buffer_proto) = make_builtin_constructor_with(
        vm,
        "ArrayBuffer",
        1,
        array_buffer_constructor,
        &[("slice", array_buffer_slice, 2)],
    )?;
    let array_buffer_byte_length_getter =
        vm.new_native_function("get byteLength", array_buffer_byte_length_get, 0)?;
    vm.heap.with_obj(array_buffer_proto.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("byteLength"),
            accessor_get_prop(Value::Object(array_buffer_byte_length_getter)),
        );
    });
    define_global(vm, "ArrayBuffer", Value::Object(array_buffer_ctor));

    let (data_view_ctor, data_view_proto) = make_builtin_constructor_with(
        vm,
        "DataView",
        3,
        data_view_constructor,
        &[
            ("getFloat32", data_view_get_float32, 1),
            ("getFloat64", data_view_get_float64, 1),
            ("getBigInt64", data_view_get_bigint64, 1),
            ("getBigUint64", data_view_get_biguint64, 1),
            ("getInt16", data_view_get_int16, 1),
            ("getInt32", data_view_get_int32, 1),
            ("getInt8", data_view_get_int8, 1),
            ("getUint16", data_view_get_uint16, 1),
            ("getUint32", data_view_get_uint32, 1),
            ("getUint8", data_view_get_uint8, 1),
            ("setFloat32", data_view_set_float32, 2),
            ("setFloat64", data_view_set_float64, 2),
            ("setBigInt64", data_view_set_bigint64, 2),
            ("setBigUint64", data_view_set_biguint64, 2),
            ("setInt16", data_view_set_int16, 2),
            ("setInt32", data_view_set_int32, 2),
            ("setInt8", data_view_set_int8, 2),
            ("setUint16", data_view_set_uint16, 2),
            ("setUint32", data_view_set_uint32, 2),
            ("setUint8", data_view_set_uint8, 2),
        ],
    )?;
    let data_view_buffer_getter = vm.new_native_function("get buffer", data_view_buffer_get, 0)?;
    let data_view_byte_length_getter =
        vm.new_native_function("get byteLength", data_view_byte_length_get, 0)?;
    let data_view_byte_offset_getter =
        vm.new_native_function("get byteOffset", data_view_byte_offset_get, 0)?;
    vm.heap.with_obj(data_view_proto.0, |obj| {
        let mut props = obj.props().lock();
        props.insert(
            PropertyKey::from("buffer"),
            accessor_get_prop(Value::Object(data_view_buffer_getter)),
        );
        props.insert(
            PropertyKey::from("byteLength"),
            accessor_get_prop(Value::Object(data_view_byte_length_getter)),
        );
        props.insert(
            PropertyKey::from("byteOffset"),
            accessor_get_prop(Value::Object(data_view_byte_offset_getter)),
        );
    });
    define_global(vm, "DataView", Value::Object(data_view_ctor));

    // Uint8Array constructor.
    let u8_ctor_idx = vm.new_native_function("Uint8Array", uint8array_constructor, 1)?;
    let u8_proto_idx = GcIdx(vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Uint8Array")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?);
    vm.heap.with_obj(u8_ctor_idx.0, |o| {
        if let HeapObj::Function(f) = o {
            *f.prototype.lock() = Some(Value::Object(u8_proto_idx));
            f.props.lock().insert(
                PropertyKey::from("prototype"),
                const_prop(Value::Object(u8_proto_idx)),
            );
        }
    });
    vm.heap.with_obj(u8_proto_idx.0, |o| {
        o.props().lock().insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(u8_ctor_idx)),
        );
    });
    define_global(vm, "Uint8Array", Value::Object(u8_ctor_idx));
    // Date (minimal: now() and constructor returning a timestamp wrapper)
    let (date_ctor, date_proto) = make_builtin_constructor_with(
        vm,
        "Date",
        7,
        date_constructor,
        &[
            ("valueOf", date_get_time, 0),
            ("getTime", date_get_time, 0),
            ("getFullYear", date_get_component, 0),
            ("getUTCFullYear", date_get_component, 0),
            ("getMonth", date_get_component, 0),
            ("getUTCMonth", date_get_component, 0),
            ("getDate", date_get_component, 0),
            ("getUTCDate", date_get_component, 0),
            ("getDay", date_get_component, 0),
            ("getUTCDay", date_get_component, 0),
            ("getHours", date_get_component, 0),
            ("getUTCHours", date_get_component, 0),
            ("getMinutes", date_get_component, 0),
            ("getUTCMinutes", date_get_component, 0),
            ("getSeconds", date_get_component, 0),
            ("getUTCSeconds", date_get_component, 0),
            ("getMilliseconds", date_get_component, 0),
            ("getUTCMilliseconds", date_get_component, 0),
            ("setTime", date_set_component, 1),
            ("setMilliseconds", date_set_component, 1),
            ("setUTCMilliseconds", date_set_component, 1),
            ("setSeconds", date_set_component, 1),
            ("setUTCSeconds", date_set_component, 1),
            ("setMinutes", date_set_component, 1),
            ("setUTCMinutes", date_set_component, 1),
            ("setHours", date_set_component, 1),
            ("setUTCHours", date_set_component, 1),
            ("setDate", date_set_component, 1),
            ("setUTCDate", date_set_component, 1),
            ("setMonth", date_set_component, 1),
            ("setUTCMonth", date_set_component, 1),
            ("setFullYear", date_set_component, 1),
            ("setUTCFullYear", date_set_component, 1),
            ("toString", date_to_string, 0),
            ("toLocaleString", date_to_string, 0),
            ("toUTCString", date_to_string, 0),
            ("toTimeString", date_to_string, 0),
            ("toDateString", date_to_string, 0),
            ("toLocaleDateString", date_to_string, 0),
            ("toLocaleTimeString", date_to_string, 0),
            ("toISOString", date_to_string, 0),
            ("toJSON", date_to_string, 1),
            ("getTimezoneOffset", date_get_timezone_offset, 0),
        ],
    )?;
    vm.date_proto = Value::Object(date_proto);
    define_global(vm, "Date", Value::Object(date_ctor));
    let now_fn = vm.new_native_function("now", date_now, 0)?;
    let parse_fn = vm.new_native_function("parse", date_parse, 1)?;
    let utc_fn = vm.new_native_function("UTC", date_utc, 7)?;
    if let Value::Object(dc) = Value::Object(date_ctor) {
        vm.heap.with_obj(dc.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from("now"), data_prop(Value::Object(now_fn)));
            obj.props().lock().insert(
                PropertyKey::from("parse"),
                data_prop(Value::Object(parse_fn)),
            );
            obj.props()
                .lock()
                .insert(PropertyKey::from("UTC"), data_prop(Value::Object(utc_fn)));
        });
    }
    // Array
    let (array_ctor, array_proto) = make_builtin_constructor_with(
        vm,
        "Array",
        1,
        array_constructor,
        &[
            ("push", array_push, 1),
            ("pop", array_pop, 0),
            ("join", array_join, 1),
            ("map", array_map, 1),
            ("filter", array_filter, 1),
            ("reduce", array_reduce, 1),
            ("reduceRight", array_reduce_right, 1),
            ("toReversed", array_to_reversed, 0),
            ("toSorted", array_to_sorted, 1),
            ("toSpliced", array_to_spliced, 2),
            ("with", array_with, 2),
            ("forEach", array_for_each, 1),
            ("indexOf", array_index_of, 1),
            ("includes", array_includes, 1),
            ("slice", array_slice, 2),
            ("concat", array_concat, 1),
            ("find", array_find, 1),
            ("findIndex", array_find_index, 1),
            ("findLast", array_find_last, 1),
            ("findLastIndex", array_find_last_index, 1),
            ("fill", array_fill, 1),
            ("some", array_some, 1),
            ("every", array_every, 1),
            ("reverse", array_reverse, 0),
            ("sort", array_sort, 1),
            ("shift", array_shift, 0),
            ("unshift", array_unshift, 1),
            ("splice", array_splice, 2),
            ("lastIndexOf", array_last_index_of, 1),
            ("at", array_at, 1),
            ("flat", array_flat, 0),
            ("flatMap", array_flat_map, 1),
            ("copyWithin", array_copy_within, 2),
            ("keys", array_keys, 0),
            ("values", array_values, 0),
            ("entries", array_entries, 0),
            ("toString", array_to_string, 0),
            ("toLocaleString", array_to_string, 0),
        ],
    )?;
    // override the constructor function to use array_constructor
    vm.array_proto = Value::Object(array_proto);
    define_global(vm, "Array", Value::Object(array_ctor));
    // Array statics
    for (n, f, len) in [
        ("isArray", array_is_array as NativeFn, 1),
        ("from", array_from as NativeFn, 1),
        ("of", array_of as NativeFn, 0),
    ] {
        let m = vm.new_native_function(n, f, len)?;
        vm.heap.with_obj(array_ctor.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from(n), data_prop(Value::Object(m)));
        });
    }
    let array_species_getter =
        vm.new_native_function("get [Symbol.species]", promise_species_get, 0)?;
    vm.heap.with_obj(array_ctor.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::Symbol(vm.well_known_symbols.species),
            accessor_get_prop(Value::Object(array_species_getter)),
        );
    });
    // String
    let (str_ctor, str_proto) = make_builtin_constructor_with(
        vm,
        "String",
        1,
        string_constructor,
        &[
            ("charAt", str_char_at, 1),
            ("charCodeAt", str_char_code_at, 1),
            ("indexOf", str_index_of, 1),
            ("lastIndexOf", str_last_index_of, 1),
            ("valueOf", boxed_value_of, 0),
            ("slice", str_slice, 2),
            ("toUpperCase", str_to_upper, 0),
            ("toLowerCase", str_to_lower, 0),
            ("toLocaleUpperCase", str_to_upper, 0),
            ("toLocaleLowerCase", str_to_lower, 0),
            ("localeCompare", str_locale_compare, 1),
            ("trim", str_trim, 0),
            ("split", str_split, 1),
            ("replace", str_replace, 2),
            ("includes", str_includes, 1),
            ("startsWith", str_starts_with, 1),
            ("endsWith", str_ends_with, 1),
            ("repeat", str_repeat, 1),
            ("match", str_match, 1),
            ("padStart", str_pad_start, 1),
            ("padEnd", str_pad_end, 1),
            ("at", str_at, 1),
            ("trimStart", str_trim_start, 0),
            ("trimEnd", str_trim_end, 0),
            ("replaceAll", str_replace_all, 2),
            ("substring", str_substring, 2),
            ("substr", str_substr, 2),
            ("codePointAt", str_code_point_at, 1),
            ("concat", str_concat, 1),
            ("search", str_search, 1),
            ("toString", string_proto_to_string, 0),
            ("valueOf", boxed_value_of, 0),
        ],
    )?;
    vm.string_proto = Value::Object(str_proto);
    vm.heap.with_obj(str_proto.0, |obj| {
        obj.props()
            .lock()
            .insert(PropertyKey::from("length"), const_prop(Value::Number(0.0)));
    });
    define_global(vm, "String", Value::Object(str_ctor));
    // String static methods
    let raw_fn = vm.new_native_function("raw", string_raw, 1)?;
    vm.heap.with_obj(str_ctor.0, |obj| {
        obj.props()
            .lock()
            .insert(PropertyKey::from("raw"), data_prop(Value::Object(raw_fn)));
    });
    let fcp_fn = vm.new_native_function("fromCodePoint", string_from_code_point, 1)?;
    vm.heap.with_obj(str_ctor.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("fromCodePoint"),
            data_prop(Value::Object(fcp_fn)),
        );
    });
    // String statics
    let from_char_code_fn = vm.new_native_function("fromCharCode", str_from_char_code, 1)?;
    vm.heap.with_obj(str_ctor.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("fromCharCode"),
            data_prop(Value::Object(from_char_code_fn)),
        );
    });
    // Number
    let (num_ctor, num_proto) = make_builtin_constructor_with(
        vm,
        "Number",
        1,
        number_constructor,
        &[
            ("toFixed", num_to_fixed, 1),
            ("toPrecision", num_to_precision, 1),
            ("toExponential", num_to_exponential, 1),
            ("toString", num_proto_to_string, 1),
            ("toLocaleString", num_proto_to_string, 0),
            ("valueOf", boxed_value_of, 0),
        ],
    )?;
    vm.number_proto = Value::Object(num_proto);
    // Number static methods + constants
    let statics: &[(&str, NativeFn, usize)] = &[
        ("isInteger", number_is_integer, 1),
        ("isFinite", number_is_finite, 1),
        ("isNaN", number_is_nan, 1),
        ("isSafeInteger", number_is_safe_integer, 1),
        ("parseInt", number_parse_int, 2),
        ("parseFloat", number_parse_float, 1),
    ];
    let mut static_props: Vec<(Arc<str>, Value)> = Vec::new();
    for (name, fnp, len) in statics {
        let idx = vm.new_native_function(name, *fnp, *len)?;
        static_props.push((Arc::from(*name), Value::Object(idx)));
    }
    static_props.push((
        Arc::from("MAX_SAFE_INTEGER"),
        Value::Number(9007199254740991.0),
    ));
    static_props.push((
        Arc::from("MIN_SAFE_INTEGER"),
        Value::Number(-9007199254740991.0),
    ));
    static_props.push((Arc::from("EPSILON"), Value::Number(f64::EPSILON)));
    static_props.push((Arc::from("MAX_VALUE"), Value::Number(f64::MAX)));
    static_props.push((Arc::from("MIN_VALUE"), Value::Number(5e-324f64)));
    static_props.push((Arc::from("POSITIVE_INFINITY"), Value::Number(f64::INFINITY)));
    static_props.push((
        Arc::from("NEGATIVE_INFINITY"),
        Value::Number(f64::NEG_INFINITY),
    ));
    static_props.push((Arc::from("NaN"), Value::Number(f64::NAN)));
    vm.heap.with_obj(num_ctor.0, |o| {
        if let HeapObj::Function(f) = o {
            for (name, val) in &static_props {
                f.props
                    .lock()
                    .insert(PropertyKey::from(name.clone()), const_prop(val.clone()));
            }
        }
    });
    define_global(vm, "Number", Value::Object(num_ctor));
    // Boolean
    let (bool_ctor, bool_proto) = make_builtin_constructor_with(
        vm,
        "Boolean",
        1,
        boolean_constructor,
        &[
            ("valueOf", boxed_value_of, 0),
            ("toString", boolean_to_string, 0),
        ],
    )?;
    vm.boolean_proto = Value::Object(bool_proto);
    define_global(vm, "Boolean", Value::Object(bool_ctor));
    // globals
    let idx = vm.new_native_function("parseInt", global_parse_int, 2)?;
    define_global(vm, "parseInt", Value::Object(idx));
    let idx = vm.new_native_function("parseFloat", global_parse_float, 1)?;
    define_global(vm, "parseFloat", Value::Object(idx));
    let idx = vm.new_native_function("isNaN", global_is_nan, 1)?;
    define_global(vm, "isNaN", Value::Object(idx));
    let idx = vm.new_native_function("isFinite", global_is_finite, 1)?;
    define_global(vm, "isFinite", Value::Object(idx));
    let eval_idx = vm.new_native_function("eval", global_eval, 1)?;
    let eval_value = Value::Object(eval_idx);
    vm.realm_eval_functions
        .insert(vm.global.0, eval_value.clone());
    define_global(vm, "eval", eval_value);
    for name in [
        "decodeURI",
        "decodeURIComponent",
        "encodeURI",
        "encodeURIComponent",
    ] {
        let idx = vm.new_native_function(name, global_uri_identity, 1)?;
        define_global(vm, name, Value::Object(idx));
    }
    define_global_const(vm, "NaN", Value::Number(f64::NAN));
    define_global_const(vm, "Infinity", Value::Number(f64::INFINITY));
    define_global_const(vm, "undefined", Value::Undefined);
    // BigInt constructor (function form only; no prototype methods yet).
    let bigint_idx = vm.new_native_function("BigInt", global_bigint, 1)?;
    let as_int_n = vm.new_native_function("asIntN", bigint_as_int_n, 2)?;
    let as_uint_n = vm.new_native_function("asUintN", bigint_as_uint_n, 2)?;
    vm.heap.with_obj(bigint_idx.0, |obj| {
        if let HeapObj::Function(f) = obj {
            let mut props = f.props.lock();
            props.insert(
                PropertyKey::from("asIntN"),
                data_prop(Value::Object(as_int_n)),
            );
            props.insert(
                PropertyKey::from("asUintN"),
                data_prop(Value::Object(as_uint_n)),
            );
        }
    });
    define_global(vm, "BigInt", Value::Object(bigint_idx));
    // BigInt prototype with minimal members.
    {
        let bp_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.object_proto.clone())),
            extensible: AtomicBool::new(true),
            class_name: Some(Arc::from("BigInt")),
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        }))?;
        let bproto = Value::Object(GcIdx(bp_idx));
        vm.bigint_proto = bproto.clone();
        {
            let bi = bigint_idx;
            vm.heap.with_obj(bi.0, |obj| {
                if let HeapObj::Function(f) = obj {
                    *f.prototype.lock() = Some(bproto.clone());
                    f.props
                        .lock()
                        .insert(PropertyKey::from("prototype"), const_prop(bproto.clone()));
                }
            });
            let to_str = vm.new_native_function("toString", bigint_to_string, 0)?;
            let value_of = vm.new_native_function("valueOf", bigint_value_of, 0)?;
            if let Value::Object(pi) = bproto {
                vm.heap.with_obj(pi.0, |obj| {
                    obj.props().lock().insert(
                        crate::value::PropertyKey::from("constructor"),
                        data_prop(Value::Object(bi)),
                    );
                    obj.props().lock().insert(
                        crate::value::PropertyKey::from("toString"),
                        data_prop(Value::Object(to_str)),
                    );
                    obj.props().lock().insert(
                        crate::value::PropertyKey::from("valueOf"),
                        data_prop(Value::Object(value_of)),
                    );
                });
            }
        }
    }
    // Promise
    let (promise_ctor, promise_proto) = make_builtin_constructor_with(
        vm,
        "Promise",
        1,
        promise_constructor,
        &[
            ("then", promise_then, 2),
            ("catch", promise_catch, 1),
            ("finally", promise_finally, 1),
        ],
    )?;
    vm.promise_ctor = Value::Object(promise_ctor);
    vm.promise_proto = Value::Object(promise_proto);
    // Static methods on the Promise constructor.
    let resolve_static = vm.new_native_function("resolve", promise_static_resolve, 1)?;
    let reject_static = vm.new_native_function("reject", promise_static_reject, 1)?;
    let all_static = vm.new_native_function("all", promise_static_all, 1)?;
    let all_keyed_static = vm.new_native_function("allKeyed", promise_static_all_keyed, 1)?;
    let race_static = vm.new_native_function("race", promise_static_race, 1)?;
    let all_settled_static = vm.new_native_function("allSettled", promise_static_all_settled, 1)?;
    let all_settled_keyed_static =
        vm.new_native_function("allSettledKeyed", promise_static_all_settled_keyed, 1)?;
    let any_static = vm.new_native_function("any", promise_static_any, 1)?;
    let try_static = vm.new_native_function("try", promise_static_try, 1)?;
    let with_resolvers_static =
        vm.new_native_function("withResolvers", promise_with_resolvers, 0)?;
    let species_getter = vm.new_native_function("get [Symbol.species]", promise_species_get, 0)?;
    vm.heap.with_obj(promise_ctor.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("resolve"),
            data_prop(Value::Object(resolve_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("reject"),
            data_prop(Value::Object(reject_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("all"),
            data_prop(Value::Object(all_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("allKeyed"),
            data_prop(Value::Object(all_keyed_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("race"),
            data_prop(Value::Object(race_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("allSettled"),
            data_prop(Value::Object(all_settled_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("allSettledKeyed"),
            data_prop(Value::Object(all_settled_keyed_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("any"),
            data_prop(Value::Object(any_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("try"),
            data_prop(Value::Object(try_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("withResolvers"),
            data_prop(Value::Object(with_resolvers_static)),
        );
        obj.props().lock().insert(
            PropertyKey::Symbol(vm.well_known_symbols.species),
            accessor_get_prop(Value::Object(species_getter)),
        );
    });
    define_global(vm, "Promise", Value::Object(promise_ctor));
    // RegExp
    let (regex_ctor, regex_proto) = make_builtin_constructor_with(
        vm,
        "RegExp",
        2,
        regexp_constructor,
        &[
            ("test", regexp_test, 1),
            ("exec", regexp_exec, 1),
            ("toString", regexp_to_string, 0),
        ],
    )?;
    let source_getter = vm.new_native_function("get source", regexp_source_get, 0)?;
    let flags_getter = vm.new_native_function("get flags", regexp_flags_get, 0)?;
    let has_indices_getter = vm.new_native_function("get hasIndices", regexp_has_indices_get, 0)?;
    let global_getter = vm.new_native_function("get global", regexp_global_get, 0)?;
    let ignore_case_getter = vm.new_native_function("get ignoreCase", regexp_ignore_case_get, 0)?;
    let multiline_getter = vm.new_native_function("get multiline", regexp_multiline_get, 0)?;
    let dot_all_getter = vm.new_native_function("get dotAll", regexp_dot_all_get, 0)?;
    let unicode_getter = vm.new_native_function("get unicode", regexp_unicode_get, 0)?;
    let unicode_sets_getter =
        vm.new_native_function("get unicodeSets", regexp_unicode_sets_get, 0)?;
    let sticky_getter = vm.new_native_function("get sticky", regexp_sticky_get, 0)?;
    vm.heap.with_obj(regex_proto.0, |o| {
        if let HeapObj::Object(obj) = o {
            let mut props = obj.props.lock();
            props.insert(
                PropertyKey::from("__regex_proto__"),
                data_prop(Value::Bool(true)),
            );
            props.insert(
                PropertyKey::from("source"),
                accessor_get_prop(Value::Object(source_getter)),
            );
            props.insert(
                PropertyKey::from("flags"),
                accessor_get_prop(Value::Object(flags_getter)),
            );
            props.insert(
                PropertyKey::from("hasIndices"),
                accessor_get_prop(Value::Object(has_indices_getter)),
            );
            props.insert(
                PropertyKey::from("global"),
                accessor_get_prop(Value::Object(global_getter)),
            );
            props.insert(
                PropertyKey::from("ignoreCase"),
                accessor_get_prop(Value::Object(ignore_case_getter)),
            );
            props.insert(
                PropertyKey::from("multiline"),
                accessor_get_prop(Value::Object(multiline_getter)),
            );
            props.insert(
                PropertyKey::from("dotAll"),
                accessor_get_prop(Value::Object(dot_all_getter)),
            );
            props.insert(
                PropertyKey::from("unicode"),
                accessor_get_prop(Value::Object(unicode_getter)),
            );
            props.insert(
                PropertyKey::from("unicodeSets"),
                accessor_get_prop(Value::Object(unicode_sets_getter)),
            );
            props.insert(
                PropertyKey::from("sticky"),
                accessor_get_prop(Value::Object(sticky_getter)),
            );
        }
    });
    // Store regex_proto on the constructor so regexp_constructor can use it.
    let regexp_species_getter =
        vm.new_native_function("get [Symbol.species]", promise_species_get, 0)?;
    vm.heap.with_obj(regex_ctor.0, |o| {
        if let HeapObj::Function(f) = o {
            f.props.lock().insert(
                PropertyKey::from("__proto__"),
                data_prop(Value::Object(regex_proto)),
            );
            f.props.lock().insert(
                PropertyKey::Symbol(vm.well_known_symbols.species),
                accessor_get_prop(Value::Object(regexp_species_getter)),
            );
        }
    });
    define_global(vm, "RegExp", Value::Object(regex_ctor));
    // Generator prototype with next(). Generator instances inherit this proto.
    let generator_proto_idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Generator")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    {
        let next_fn = vm.new_native_function("next", generator_next, 0)?;
        let return_fn = vm.new_native_function("return", generator_return, 1)?;
        let throw_fn = vm.new_native_function("throw", generator_throw, 1)?;
        vm.heap.with_obj(generator_proto_idx, |o| {
            o.props()
                .lock()
                .insert(PropertyKey::from("next"), data_prop(Value::Object(next_fn)));
            o.props().lock().insert(
                PropertyKey::from("return"),
                data_prop(Value::Object(return_fn)),
            );
            o.props().lock().insert(
                PropertyKey::from("throw"),
                data_prop(Value::Object(throw_fn)),
            );
        });
    }
    vm.generator_proto = Value::Object(GcIdx(generator_proto_idx));
    // Function constructor: new Function(p0, ..., body)
    let function_ctor_idx = vm.new_native_function("Function", function_constructor, 1)?;
    vm.heap.with_obj(function_ctor_idx.0, |obj| {
        if let HeapObj::Function(f) = obj {
            f.prototype
                .lock()
                .replace(Value::Object(function_proto_idx));
        }
    });
    define_global(vm, "Function", Value::Object(function_ctor_idx));
    // %GeneratorFunction% is not exposed as a global binding, but generator
    // functions inherit from %GeneratorFunction.prototype%, whose constructor
    // property exposes it.
    let generator_function_ctor_idx =
        vm.new_native_function("GeneratorFunction", generator_function_constructor, 1)?;
    let generator_function_proto_idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.function_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("GeneratorFunction")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    vm.generator_function_proto = Value::Object(GcIdx(generator_function_proto_idx));
    vm.heap.with_obj(generator_function_proto_idx, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(generator_function_ctor_idx)),
        );
    });
    vm.heap.with_obj(generator_function_ctor_idx.0, |obj| {
        if let HeapObj::Function(f) = obj {
            f.prototype
                .lock()
                .replace(Value::Object(GcIdx(generator_function_proto_idx)));
        }
        obj.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(Value::Object(GcIdx(generator_function_proto_idx))),
        );
    });
    // Install call/apply/bind on Function.prototype (allocated at the top of
    // setup_full) so every function inherits them via its [[Prototype]].
    let call_fn = vm.new_native_function("call", function_call, 1)?;
    let apply_fn = vm.new_native_function("apply", function_apply, 2)?;
    let bind_fn = vm.new_native_function("bind", function_bind, 1)?;
    let tostring_fn = vm.new_native_function("toString", function_to_string, 0)?;
    let throw_type_error_fn =
        vm.new_native_function("ThrowTypeError", function_throw_type_error, 0)?;
    install_methods(
        vm,
        &Value::Object(function_proto_idx),
        &[
            (Arc::from("call"), Value::Object(call_fn)),
            (Arc::from("apply"), Value::Object(apply_fn)),
            (Arc::from("bind"), Value::Object(bind_fn)),
            (Arc::from("toString"), Value::Object(tostring_fn)),
        ],
    );
    // Function.prototype points to the function prototype object.
    vm.heap.with_obj(function_ctor_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("prototype"),
            const_prop(Value::Object(function_proto_idx)),
        );
    });
    // The function prototype's `constructor` is the Function constructor.
    vm.heap.with_obj(function_proto_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(function_ctor_idx)),
        );
        let restricted = PropertyDescriptor {
            value: Value::Undefined,
            writable: false,
            enumerable: false,
            configurable: true,
            get: Some(Value::Object(throw_type_error_fn)),
            set: Some(Value::Object(throw_type_error_fn)),
            is_accessor: true,
        };
        props.insert(PropertyKey::from("caller"), restricted.clone());
        props.insert(PropertyKey::from("arguments"), restricted);
    });
    setup_collections(vm)?;
    install_test262_host(vm)?;
    Ok(())
}

// =========================================================================

fn object_is_prototype_of(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    let arg = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(arg, Value::Object(_)) {
        return Ok(Value::Bool(false));
    }
    if this.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let this_obj = vm.to_object(&this)?;
    let Value::Object(this_idx) = this_obj else {
        return Ok(Value::Bool(false));
    };
    let Value::Object(arg_idx) = arg else {
        return Ok(Value::Bool(false));
    };
    let mut cur = vm
        .heap
        .with_obj(arg_idx.0, |o| o.proto().lock().clone())
        .unwrap_or(Value::Null);
    let mut depth = 0;
    while let Value::Object(idx) = &cur {
        if depth > 1024 {
            break;
        }
        depth += 1;
        if *idx == this_idx {
            return Ok(Value::Bool(true));
        }
        let proto = vm.heap.with_obj(idx.0, |o| o.proto().lock().clone());
        cur = proto.unwrap_or(Value::Null);
        if cur.is_null() {
            break;
        }
    }
    Ok(Value::Bool(false))
}

fn error_to_string(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    let name = vm.get_property(&this, "name")?;
    let name_str = if name.is_undefined() {
        "Error".to_string()
    } else {
        vm.to_string(&name)?.to_string()
    };
    let msg = vm.get_property(&this, "message")?;
    let msg_str = if msg.is_undefined() {
        String::new()
    } else {
        vm.to_string(&msg)?.to_string()
    };
    if msg_str.is_empty() {
        Ok(Value::String(Arc::from(name_str)))
    } else {
        Ok(Value::String(Arc::from(format!(
            "{}: {}",
            name_str, msg_str
        ))))
    }
}
