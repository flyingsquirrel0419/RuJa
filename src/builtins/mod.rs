//! Built-in objects and globals for the RuJa VM.
//!
//! All built-in constructors, prototypes, and global functions are registered
//! here. Native functions follow the `NativeFn` signature used by the VM.


pub(crate) mod math;
pub(crate) mod json;
pub(crate) mod global;


pub(crate) mod array;
pub(crate) use array::*;

pub(crate) mod string;
pub(crate) use string::*;

pub(crate) mod collections;
pub(crate) use collections::*;
pub(crate) mod regexp;
pub(crate) use regexp::*;
pub(crate) mod function;
pub(crate) use function::*;
pub(crate) use math::{build_console, build_math};
pub(crate) use json::{build_json, build_reflect, date_constructor, date_get_time, date_now, date_to_string};
pub(crate) use global::{bigint_to_string, function_constructor, global_bigint, global_eval, global_is_finite, global_is_nan, global_parse_float, global_parse_int};

use crate::environment as env;
use crate::error::{self, Error};
use crate::gc::Heap;
use crate::value::{
    ArrayData, BindingKind, FunctionData, FunctionKind, GcIdx, HeapObj, MapData, ObjectData,
    PropertyDescriptor, PropertyKey, SetData, Value,
};
use crate::vm::{NativeFn, Vm};
use indexmap::IndexMap;
use num_bigint::{BigInt, BigUint};
use num_integer::Integer;
use num_rational::Ratio;
use num_traits::{Signed, ToPrimitive, Zero};
use regex::{Regex, RegexBuilder};

/// Compile a regex pattern applying ES flags: `i` (case-insensitive),
/// `m` (multiline ^/$), `s` (dotall). Other flags (`g`/`y`/`u`) do not affect
/// the regex engine here and are handled by the caller.
fn compile_regex(source: &str, flags: &str) -> Result<Regex, regex::Error> {
    let mut b = RegexBuilder::new(source);
    b.case_insensitive(flags.contains('i'));
    b.multi_line(flags.contains('m'));
    b.dot_matches_new_line(flags.contains('s'));
    b.build()
}
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use parking_lot::Mutex;

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
        Value::Object(idx) => heap.with_obj(idx.0, |obj| matches!(obj, HeapObj::Array(_))),
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
    if this.is_nullish() {
        return Ok(Value::String(Arc::from("[object Null]")));
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
    if let Value::Object(idx) = &this {
        let class = if let Some(hint) = class_hint {
            hint.to_string()
        } else {
            vm.heap.with_obj(idx.0, |obj| {
                let name = obj.class_name();
                if name == "Object" {
                    // check constructor name via prototype
                    if let Some(Value::Object(pidx)) = obj.proto().lock().as_ref().cloned()
                    {
                        let constructor = vm.heap.with_obj(pidx.0, |p| {
                            p.props()
                                .lock()
                                .get(&crate::value::PropertyKey::from("constructor"))
                                .map(|d| d.value.clone())
                        });
                        if let Some(Value::Object(fidx)) = constructor {
                            let fname = vm.heap.with_obj(fidx.0, |f| {
                                if let HeapObj::Function(fd) = f {
                                    fd.name.clone()
                                } else {
                                    None
                                }
                            });
                            if let Some(n) = fname {
                                return n.to_string();
                            }
                        }
                    }
                }
                name.to_string()
            })
        };
        return Ok(Value::String(Arc::from(
            format!("[object {}]", class).as_str(),
        )));
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
) -> (GcIdx, GcIdx) {
    let proto_value = vm.object_proto.clone();

    let mut method_props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    for (n, f, len) in methods {
        let func_idx = vm.new_native_function(n, *f, *len);
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
    let proto_idx = GcIdx(vm.heap.allocate(proto_obj));

    let ctor_func = FunctionData {
        name: Some(Arc::from(name)),
        kind: FunctionKind::Native {
            func: object_constructor,
            length: 1,
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
    // constructor.prototype
    vm.heap.with_obj(ctor_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("prototype"),
            data_prop(Value::Object(proto_idx)),
        );
    });
    // prototype.constructor
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
    });

    (ctor_idx, proto_idx)
}

pub(crate) fn make_error_constructor(vm: &mut Vm, name: &str) -> (GcIdx, GcIdx) {
    let error_proto_val = vm.error_proto.clone();
    let proto_obj = HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(error_proto_val.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from(name)),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    let proto_idx = GcIdx(vm.heap.allocate(proto_obj));

    let ctor_func = FunctionData {
        name: Some(Arc::from(name)),
        kind: FunctionKind::Native {
            func: error_constructor,
            length: 1,
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
        obj.props().lock().insert(
            PropertyKey::from("name"),
            data_prop(Value::String(Arc::from(name))),
        );
        obj.props().lock().insert(
            PropertyKey::from("message"),
            data_prop(Value::String(Arc::from(""))),
        );
    });

    (ctor_idx, proto_idx)
}

pub(crate) fn define_global(vm: &mut Vm, name: &str, value: Value) {
    env::declare(&vm.heap, vm.global, name, value, BindingKind::Var);
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
        }
        let new_idx = vm.new_object();
        return Ok(Value::Object(new_idx));
    }
    // Called as function
    if args.is_empty() {
        let new_idx = vm.new_object();
        return Ok(Value::Object(new_idx));
    }
    let first = args.first().unwrap_or(&Value::Undefined);
    match first {
        Value::Undefined | Value::Null => {
            let new_idx = vm.new_object();
            Ok(Value::Object(new_idx))
        }
        Value::Bool(_)
        | Value::Number(_)
        | Value::String(_)
        | Value::Symbol(_)
        | Value::BigInt(_) => vm.to_object(first),
        Value::Object(_) => Ok(first.clone()),
    }
}

fn object_to_string_native(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    object_to_string(vm, this, None)
}

fn object_has_own_property(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    let key = if let Some(a) = args.first() {
        vm.to_property_key(a)?
    } else {
        String::new()
    };
    match &this {
        Value::Object(idx) => {
            let has = vm.heap.with_obj(idx.0, |obj| {
                obj.props()
                    .lock()
                    .contains_key(&crate::value::PropertyKey::from(key.as_str()))
                    || {
                        if let HeapObj::Array(a) = obj {
                            if key == "length" {
                                return true;
                            }
                            if let Ok(i) = key.parse::<usize>() {
                                return i < a.items.lock().len();
                            }
                        }
                        false
                    }
            });
            Ok(Value::Bool(has))
        }
        Value::String(s) => {
            if key == "length" {
                return Ok(Value::Bool(true));
            }
            if let Ok(i) = key.parse::<usize>() {
                return Ok(Value::Bool(i < crate::value::utf16_len(s)));
            }
            Ok(Value::Bool(false))
        }
        _ => Ok(Value::Bool(false)),
    }
}

fn object_value_of(_vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(v) = this {
        return Ok(v);
    }
    Ok(Value::Undefined)
}

/// `Number.prototype.valueOf` / `Boolean.prototype.valueOf` /
/// `String.prototype.valueOf`: return the wrapped primitive of `this`.
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
    // No wrapped primitive: fall back to `this` (an ordinary object).
    Ok(this.unwrap_or(Value::Undefined))
}

/// Collect an object's own enumerable string keys in array-index-first then property order.
pub(crate) fn own_string_keys(vm: &mut Vm, obj: &Value) -> Vec<Arc<str>> {
    let mut keys = Vec::new();
    if let Value::Object(idx) = obj {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Array(a) = o {
                for i in 0..a.items.lock().len() {
                    keys.push(Arc::from(i.to_string().as_str()));
                }
            }
            if let HeapObj::Map(m) = o {
                for (k, _) in m.entries.lock().iter() {
                    if let Value::String(s) = k {
                        keys.push(s.clone());
                    }
                }
            }
            // Spec enumeration order: array-index keys (canonical integer
            // indices) in ascending numeric order, then the remaining string
            // keys in insertion order.
            let mut index_keys: Vec<u32> = Vec::new();
            let mut other_keys: Vec<Arc<str>> = Vec::new();
            for (k, desc) in o.props().lock().iter() {
                if !desc.enumerable {
                    continue;
                }
                if let crate::value::PropertyKey::Str(s) = k {
                    // A string is an array index iff it is a canonical decimal
                    // integer in [0, 2^32-1) (no leading zeros, no sign).
                    let is_index = !s.is_empty()
                        && s.bytes().all(|b| b.is_ascii_digit())
                        && !(s.len() > 1 && s.starts_with('0'))
                        && s.parse::<u32>()
                            .map(|n| (n as u64) < (1u64 << 32))
                            .unwrap_or(false);
                    if is_index {
                        index_keys.push(s.parse::<u32>().unwrap());
                    } else {
                        other_keys.push(s.clone());
                    }
                }
            }
            index_keys.sort_unstable();
            for n in index_keys {
                keys.push(Arc::from(n.to_string().as_str()));
            }
            for k in other_keys {
                keys.push(k);
            }
        });
    }
    keys
}

pub(crate) fn make_value_array(vm: &mut Vm, items: Vec<Value>) -> Value {
    let arr = HeapObj::Array(ArrayData {
        items: Mutex::new(items),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
        sparse_max: Mutex::new(None),
    });
    Value::Object(GcIdx(vm.heap.allocate(arr)))
}
pub(crate) fn norm_idx(n: f64, len: f64) -> f64 {
    if n < 0.0 {
        (len + n).max(0.0)
    } else {
        n.min(len)
    }
}

pub(crate) fn make_str_array(vm: &mut Vm, strs: Vec<Arc<str>>) -> Value {
    let items: Vec<Value> = strs.into_iter().map(Value::String).collect();
    let arr = HeapObj::Array(ArrayData {
        items: Mutex::new(items),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
        sparse_max: Mutex::new(None),
    });
    Value::Object(GcIdx(vm.heap.allocate(arr)))
}

fn object_keys(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    let keys = own_string_keys(vm, &obj);
    Ok(make_str_array(vm, keys))
}

fn object_values(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    // Use the same ordered key list as Object.keys so values line up with keys.
    let keys = own_string_keys(vm, &obj);
    let mut vals = Vec::with_capacity(keys.len());
    for k in &keys {
        vals.push(vm.get_property(&obj, k)?);
    }
    let arr = HeapObj::Array(ArrayData {
        items: Mutex::new(vals),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
        sparse_max: Mutex::new(None),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}

fn object_entries(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    let keys = own_string_keys(vm, &obj);
    let mut pairs = Vec::new();
    for k in keys {
        let v = vm.get_property(&obj, &k)?;
        let pair = HeapObj::Array(ArrayData {
            items: Mutex::new(vec![Value::String(k.clone()), v]),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.array_proto.clone())),
            sparse_max: Mutex::new(None),
        });
        pairs.push(Value::Object(GcIdx(vm.heap.allocate(pair))));
    }
    let arr = HeapObj::Array(ArrayData {
        items: Mutex::new(pairs),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
        sparse_max: Mutex::new(None),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}

fn object_assign(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    for src in &args[1..] {
        let keys = own_string_keys(vm, src);
        for k in keys {
            let v = vm.get_property(src, &k)?;
            vm.set_property(&target, &k, v)?;
        }
    }
    Ok(target)
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
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }));
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
            // Each pair is an array [key, value].
            if let Value::Object(pi) = pair {
                let (k, v) = vm.heap.with_obj(pi.0, |o| {
                    if let HeapObj::Array(a) = o {
                        let it = a.items.lock();
                        (
                            it.first().cloned().unwrap_or(Value::Undefined),
                            it.get(1).cloned().unwrap_or(Value::Undefined),
                        )
                    } else {
                        (Value::Undefined, Value::Undefined)
                    }
                });
                let _key_str = vm.to_string(&k)?.to_string();
                let key_str = vm.to_string(&k)?.to_string();
                vm.heap.with_obj(obj_idx, |o| {
                    if let HeapObj::Object(obj) = o {
                        // Own enumerable data property (data_prop is
                        // non-enumerable, which would hide it from
                        // Object.keys / JSON.stringify).
                        obj.props.lock().insert(
                            PropertyKey::from(key_str.as_str()),
                            PropertyDescriptor {
                                value: v,
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
    }
    Ok(Value::Object(GcIdx(obj_idx)))
}
fn object_create(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let proto = args.first().cloned().unwrap_or(Value::Undefined);
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(if proto.is_null() { None } else { Some(proto) }),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }));
    Ok(Value::Object(GcIdx(obj_idx)))
}
fn object_get_own_property_names(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    let keys = own_string_keys(vm, &obj);
    Ok(make_str_array(vm, keys))
}

fn object_get_prototype_of(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &obj {
        return Ok(vm.heap.with_obj(idx.0, |o| {
            o.proto().lock().clone().unwrap_or(Value::Null)
        }));
    }
    Ok(Value::Null)
}

fn object_set_prototype_of(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    let proto = args.get(1).cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &obj {
        let p = if proto.is_null() {
            None
        } else if matches!(proto, Value::Object(_)) {
            Some(proto.clone())
        } else {
            return Err(Error::type_err(
                "Object prototype may only be an Object or null",
            ));
        };
        vm.heap.with_obj(idx.0, |o| {
            *o.proto().lock() = p;
        });
    }
    Ok(obj)
}

fn object_prevent_extensions(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &obj {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Object(od) = o {
                od.extensible.store(false, Ordering::Relaxed);
            }
        });
    }
    Ok(obj)
}

fn object_is_extensible(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &obj {
        let ext = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Object(od) = o {
                od.extensible.load(Ordering::Relaxed)
            } else {
                true
            }
        });
        return Ok(Value::Bool(ext));
    }
    Ok(Value::Bool(true))
}

fn object_seal(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &obj {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Object(od) = o {
                od.extensible.store(false, Ordering::Relaxed);
                for d in od.props.lock().values_mut() {
                    d.configurable = false;
                }
            }
        });
    }
    Ok(obj)
}

fn object_is_sealed(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &obj {
        let sealed = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Object(od) = o {
                let all_noncfg = od.props.lock().values().all(|d| !d.configurable);
                all_noncfg
            } else {
                true
            }
        });
        return Ok(Value::Bool(sealed));
    }
    Ok(Value::Bool(true))
}

fn object_is_frozen(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &obj {
        let frozen = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Object(od) = o {
                let ext = od.extensible.load(Ordering::Relaxed);
                let all_frozen = od
                    .props
                    .lock()
                    .values()
                    .all(|d| !d.configurable && !d.writable && !d.is_accessor);
                !ext && all_frozen
            } else {
                true
            }
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
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    let result_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }));
    if let Value::Object(idx) = &obj {
        let keys = own_string_keys(vm, &obj);
        let descs: Vec<(String, crate::value::PropertyDescriptor)> = keys
            .iter()
            .filter_map(|k| {
                let pkey = crate::value::PropertyKey::from(k.as_ref());
                let d = vm
                    .heap
                    .with_obj(idx.0, |o| o.props().lock().get(&pkey).cloned())?;
                Some((k.to_string(), d))
            })
            .collect();
        let mut p = IndexMap::new();
        for (key, d) in descs {
            let desc_obj = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
                props: Mutex::new(IndexMap::new()),
                proto: Mutex::new(Some(vm.object_proto.clone())),
                extensible: AtomicBool::new(true),
                class_name: None,
                private_fields: Mutex::new(std::collections::HashMap::new()),
                primitive: Mutex::new(None),
            }));
            let mut dp = IndexMap::new();
            if d.is_accessor {
                dp.insert(
                    PropertyKey::from("get"),
                    data_prop(d.get.clone().unwrap_or(Value::Undefined)),
                );
                dp.insert(
                    PropertyKey::from("set"),
                    data_prop(d.set.clone().unwrap_or(Value::Undefined)),
                );
            } else {
                dp.insert(PropertyKey::from("value"), data_prop(d.value.clone()));
                dp.insert(
                    PropertyKey::from("writable"),
                    data_prop(Value::Bool(d.writable)),
                );
            }
            dp.insert(
                PropertyKey::from("enumerable"),
                data_prop(Value::Bool(d.enumerable)),
            );
            dp.insert(
                PropertyKey::from("configurable"),
                data_prop(Value::Bool(d.configurable)),
            );
            vm.heap.with_obj(desc_obj, |o| {
                if let HeapObj::Object(od) = o {
                    *od.props.lock() = dp;
                }
            });
            p.insert(
                PropertyKey::from(key.as_str()),
                data_prop(Value::Object(GcIdx(desc_obj))),
            );
        }
        vm.heap.with_obj(result_idx, |o| {
            if let HeapObj::Object(od) = o {
                *od.props.lock() = p;
            }
        });
    }
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
fn object_get_own_property_descriptor(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let obj = args.first().cloned().unwrap_or(Value::Undefined);
    let key = match args.get(1) {
        Some(v) => vm.to_property_key(v)?,
        None => return Ok(Value::Undefined),
    };
    if let Value::Object(idx) = &obj {
        let desc = vm.heap.with_obj(idx.0, |o| {
            o.props()
                .lock()
                .get(&crate::value::PropertyKey::from(key.as_str()))
                .cloned()
        });
        if let Some(d) = desc {
            let desc_obj = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
                props: Mutex::new(IndexMap::new()),
                proto: Mutex::new(Some(vm.object_proto.clone())),
                extensible: AtomicBool::new(true),
                class_name: None,
                private_fields: Mutex::new(std::collections::HashMap::new()),
                primitive: Mutex::new(None),
            }));
            let mut p = IndexMap::new();
            if d.is_accessor {
                p.insert(
                    PropertyKey::from("get"),
                    data_prop(d.get.clone().unwrap_or(Value::Undefined)),
                );
                p.insert(
                    PropertyKey::from("set"),
                    data_prop(d.set.clone().unwrap_or(Value::Undefined)),
                );
            } else {
                p.insert(PropertyKey::from("value"), data_prop(d.value.clone()));
                p.insert(
                    PropertyKey::from("writable"),
                    data_prop(Value::Bool(d.writable)),
                );
            }
            p.insert(
                PropertyKey::from("enumerable"),
                data_prop(Value::Bool(d.enumerable)),
            );
            p.insert(
                PropertyKey::from("configurable"),
                data_prop(Value::Bool(d.configurable)),
            );
            vm.heap.with_obj(desc_obj, |o| {
                if let HeapObj::Object(od) = o {
                    *od.props.lock() = p;
                }
            });
            return Ok(Value::Object(GcIdx(desc_obj)));
        }
    }
    Ok(Value::Undefined)
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
    let key = args
        .get(1)
        .map(|v| vm.to_property_key(v))
        .transpose()?
        .unwrap_or_default();
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

        if let Value::Object(_) = desc {
            // Presence of each field is determined by an OWN property on the
            // descriptor object, mirroring ToPropertyDescriptor: a missing
            // field must NOT flip the has_* flags, otherwise a plain
            // `{value: 1, writable: false}` descriptor would be misread as
            // an accessor (get/set absent but `get_property` returns
            // `Ok(undefined)`).
            if vm.has_own(&desc, "value") {
                if let Ok(v) = vm.get_property(&desc, "value") {
                    value = v;
                    has_value = true;
                }
            }
            if vm.has_own(&desc, "writable") {
                if let Ok(v) = vm.get_property(&desc, "writable") {
                    writable = v.is_truthy();
                    has_writable = true;
                }
            }
            if vm.has_own(&desc, "get") {
                if let Ok(v) = vm.get_property(&desc, "get") {
                    if !v.is_undefined() && !is_callable(&v, &vm.heap) {
                        return Err(Error::type_err("Getter must be a function"));
                    }
                    get = if v.is_undefined() { None } else { Some(v) };
                    has_get = true;
                }
            }
            if vm.has_own(&desc, "set") {
                if let Ok(v) = vm.get_property(&desc, "set") {
                    if !v.is_undefined() && !is_callable(&v, &vm.heap) {
                        return Err(Error::type_err("Setter must be a function"));
                    }
                    set = if v.is_undefined() { None } else { Some(v) };
                    has_set = true;
                }
            }
            if vm.has_own(&desc, "enumerable") {
                if let Ok(v) = vm.get_property(&desc, "enumerable") {
                    enumerable = v.is_truthy();
                }
            }
            if vm.has_own(&desc, "configurable") {
                if let Ok(v) = vm.get_property(&desc, "configurable") {
                    configurable = v.is_truthy();
                }
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
        let descriptor = if is_accessor {
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
            obj.props()
                .lock()
                .insert(PropertyKey::from(key.as_str()), descriptor);
        });
    }
    Ok(target)
}

// Minimal stubs to keep the crate compiling while parser/lexer work is in progress.

fn error_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let msg = args
        .first()
        .map(|v| vm.to_string(v).unwrap_or_else(|_| Arc::from("")))
        .unwrap_or_else(|| Arc::from(""));
    // Use the `this` provided by `construct` (already linked to <Error>.prototype).
    let idx = match this {
        Some(Value::Object(i)) => i,
        _ => vm.new_object(),
    };
    // Inherit `name` from the prototype (each Error subclass proto sets it),
    // falling back to "Error".
    let proto_idx = vm.heap.with_obj(idx.0, |obj| {
        obj.proto().lock().as_ref().and_then(|p| {
            if let Value::Object(pi) = p {
                Some(*pi)
            } else {
                None
            }
        })
    });
    let name = proto_idx
        .and_then(|pi| {
            vm.heap.with_obj(pi.0, |o| {
                o.props()
                    .lock()
                    .get(&PropertyKey::from("name"))
                    .and_then(|d| {
                        if let Value::String(s) = &d.value {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
            })
        })
        .unwrap_or_else(|| Arc::from("Error"));
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
            o.props
                .lock()
                .insert(PropertyKey::from("message"), data_prop(Value::String(msg)));
            o.props
                .lock()
                .insert(PropertyKey::from("name"), data_prop(Value::String(name)));
            if let Some(c) = cause {
                o.props
                    .lock()
                    .insert(PropertyKey::from("cause"), data_prop(c));
            }
        }
    });
    Ok(Value::Object(idx))
}

pub fn setup(vm: &mut Vm) {
    let (object_ctor, object_proto) = make_builtin_constructor(
        vm,
        "Object",
        &[
            ("toString", object_to_string_native, 0),
            ("hasOwnProperty", object_has_own_property, 1),
            ("valueOf", object_value_of, 0),
        ],
    );
    // Object static methods
    for (n, f, len) in [
        ("keys", object_keys as NativeFn, 1),
        ("values", object_values as NativeFn, 1),
        ("entries", object_entries as NativeFn, 1),
        ("assign", object_assign as NativeFn, 2),
        ("is", object_is as NativeFn, 2),
        ("fromEntries", object_from_entries as NativeFn, 1),
        ("create", object_create as NativeFn, 2),
        ("freeze", object_freeze as NativeFn, 1),
        (
            "getOwnPropertyNames",
            object_get_own_property_names as NativeFn,
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
        let m = vm.new_native_function(n, f, len);
        vm.heap.with_obj(object_ctor.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from(n), data_prop(Value::Object(m)));
        });
    }
    define_global(vm, "Object", Value::Object(object_ctor));
    vm.object_proto = Value::Object(object_proto);

    let (error_ctor, error_proto) = make_error_constructor(vm, "Error");
    vm.error_proto = Value::Object(error_proto);
    define_global(vm, "Error", Value::Object(error_ctor));
    for name in [
        "TypeError",
        "RangeError",
        "ReferenceError",
        "SyntaxError",
        "EvalError",
        "URIError",
    ] {
        let (ctor, _) = make_error_constructor(vm, name);
        define_global(vm, name, Value::Object(ctor));
    }
}


// =========================================================================
// Extended setup
// =========================================================================
pub fn setup_full(vm: &mut Vm) {
    // Allocate Function.prototype first so that every function created during
    // the rest of bootstrap inherits call/apply/bind via its [[Prototype]].
    let function_proto_idx = vm.new_native_function("Function.prototype", function_proto_noop, 0);
    vm.function_proto = Value::Object(function_proto_idx);
    setup(vm);
    // Math
    let math = build_math(vm);
    define_global(vm, "Math", math);
    // console
    let console = build_console(vm);
    define_global(vm, "console", console);
    // JSON
    let json = build_json(vm);
    define_global(vm, "JSON", json);
    // Reflect
    let reflect = build_reflect(vm);
    define_global(vm, "Reflect", reflect);
    // Date (minimal: now() and constructor returning a timestamp wrapper)
    let (date_ctor, date_proto) = make_builtin_constructor_with(
        vm,
        "Date",
        date_constructor,
        &[
            ("getTime", date_get_time, 0),
            ("toString", date_to_string, 0),
        ],
    );
    vm.date_proto = Value::Object(date_proto);
    define_global(vm, "Date", Value::Object(date_ctor));
    let now_fn = vm.new_native_function("now", date_now, 0);
    if let Value::Object(dc) = Value::Object(date_ctor) {
        vm.heap.with_obj(dc.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from("now"), data_prop(Value::Object(now_fn)));
        });
    }
    // Array
    let (array_ctor, array_proto) = make_builtin_constructor_with(
        vm,
        "Array",
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
        ],
    );
    // override the constructor function to use array_constructor
    vm.array_proto = Value::Object(array_proto);
    define_global(vm, "Array", Value::Object(array_ctor));
    // Array statics
    for (n, f, len) in [
        ("isArray", array_is_array as NativeFn, 1),
        ("from", array_from as NativeFn, 1),
        ("of", array_of as NativeFn, 0),
    ] {
        let m = vm.new_native_function(n, f, len);
        vm.heap.with_obj(array_ctor.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from(n), data_prop(Value::Object(m)));
        });
    }
    // String
    let (str_ctor, str_proto) = make_builtin_constructor_with(
        vm,
        "String",
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
        ],
    );
    vm.string_proto = Value::Object(str_proto);
    define_global(vm, "String", Value::Object(str_ctor));
    // String static methods
    let raw_fn = vm.new_native_function("raw", string_raw, 1);
    vm.heap.with_obj(str_ctor.0, |obj| {
        obj.props()
            .lock()
            .insert(PropertyKey::from("raw"), data_prop(Value::Object(raw_fn)));
    });
    let fcp_fn = vm.new_native_function("fromCodePoint", string_from_code_point, 1);
    vm.heap.with_obj(str_ctor.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("fromCodePoint"),
            data_prop(Value::Object(fcp_fn)),
        );
    });
    // String statics
    let from_char_code_fn = vm.new_native_function("fromCharCode", str_from_char_code, 1);
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
        number_constructor,
        &[
            ("toFixed", num_to_fixed, 1),
            ("toPrecision", num_to_precision, 1),
            ("toExponential", num_to_exponential, 1),
            ("toString", num_proto_to_string, 1),
            ("valueOf", boxed_value_of, 0),
        ],
    );
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
        let idx = vm.new_native_function(name, *fnp, *len);
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
    static_props.push((Arc::from("MIN_VALUE"), Value::Number(f64::MIN_POSITIVE)));
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
                    .insert(PropertyKey::from(name.clone()), data_prop(val.clone()));
            }
        }
    });
    define_global(vm, "Number", Value::Object(num_ctor));
    // Boolean
    let (bool_ctor, bool_proto) = make_builtin_constructor_with(
        vm,
        "Boolean",
        boolean_constructor,
        &[("valueOf", boxed_value_of, 0)],
    );
    vm.boolean_proto = Value::Object(bool_proto);
    define_global(vm, "Boolean", Value::Object(bool_ctor));
    // globals
    let idx = vm.new_native_function("parseInt", global_parse_int, 2);
    define_global(vm, "parseInt", Value::Object(idx));
    let idx = vm.new_native_function("parseFloat", global_parse_float, 1);
    define_global(vm, "parseFloat", Value::Object(idx));
    let idx = vm.new_native_function("isNaN", global_is_nan, 1);
    define_global(vm, "isNaN", Value::Object(idx));
    let idx = vm.new_native_function("isFinite", global_is_finite, 1);
    define_global(vm, "isFinite", Value::Object(idx));
    let eval_idx = vm.new_native_function("eval", global_eval, 1);
    define_global(vm, "eval", Value::Object(eval_idx));
    define_global(vm, "NaN", Value::Number(f64::NAN));
    define_global(vm, "Infinity", Value::Number(f64::INFINITY));
    define_global(vm, "undefined", Value::Undefined);
    // BigInt constructor (function form only; no prototype methods yet).
    let bigint_idx = vm.new_native_function("BigInt", global_bigint, 1);
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
        }));
        let bproto = Value::Object(GcIdx(bp_idx));
        vm.bigint_proto = bproto.clone();
        {
            let bi = bigint_idx;
            vm.heap.with_obj(bi.0, |obj| {
                if let HeapObj::Function(f) = obj {
                    *f.prototype.lock() = Some(bproto.clone());
                }
            });
            let to_str = vm.new_native_function("toString", bigint_to_string, 0);
            if let Value::Object(pi) = bproto {
                vm.heap.with_obj(pi.0, |obj| {
                    obj.props().lock().insert(
                        crate::value::PropertyKey::from("toString"),
                        crate::value::PropertyDescriptor::data(Value::Object(to_str)),
                    );
                    obj.props().lock().insert(
                        crate::value::PropertyKey::from("valueOf"),
                        crate::value::PropertyDescriptor::data(Value::Object(to_str)),
                    );
                });
            }
        }
    }
    // globalThis: an object whose property accesses route to the global
    // environment record. Marked via class_name so VM get/set can detect it.
    let globalthis_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("global")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }));
    vm.global_this = Value::Object(GcIdx(globalthis_idx));
    define_global(vm, "globalThis", vm.global_this.clone());
    // Promise
    let (promise_ctor, promise_proto) = make_builtin_constructor_with(
        vm,
        "Promise",
        promise_constructor,
        &[("then", promise_then, 2), ("catch", promise_catch, 1)],
    );
    vm.promise_proto = Value::Object(promise_proto);
    // Static methods on the Promise constructor.
    let resolve_static = vm.new_native_function("resolve", promise_static_resolve, 1);
    let reject_static = vm.new_native_function("reject", promise_static_reject, 1);
    vm.heap.with_obj(promise_ctor.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("resolve"),
            data_prop(Value::Object(resolve_static)),
        );
        obj.props().lock().insert(
            PropertyKey::from("reject"),
            data_prop(Value::Object(reject_static)),
        );
    });
    define_global(vm, "Promise", Value::Object(promise_ctor));
    // RegExp
    let (regex_ctor, regex_proto) = make_builtin_constructor_with(
        vm,
        "RegExp",
        regexp_constructor,
        &[("test", regexp_test, 1), ("exec", regexp_exec, 1)],
    );
    vm.heap.with_obj(regex_proto.0, |o| {
        if let HeapObj::Object(obj) = o {
            obj.props.lock().insert(
                PropertyKey::from("__regex_proto__"),
                data_prop(Value::Bool(true)),
            );
        }
    });
    // Store regex_proto on the constructor so regexp_constructor can use it.
    vm.heap.with_obj(regex_ctor.0, |o| {
        if let HeapObj::Function(f) = o {
            f.props.lock().insert(
                PropertyKey::from("__proto__"),
                data_prop(Value::Object(regex_proto)),
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
    }));
    {
        let next_fn = vm.new_native_function("next", generator_next, 0);
        let return_fn = vm.new_native_function("return", generator_return, 1);
        let throw_fn = vm.new_native_function("throw", generator_throw, 1);
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
    let function_ctor_idx = vm.new_native_function("Function", function_constructor, 1);
    define_global(vm, "Function", Value::Object(function_ctor_idx));
    // Install call/apply/bind on Function.prototype (allocated at the top of
    // setup_full) so every function inherits them via its [[Prototype]].
    let call_fn = vm.new_native_function("call", function_call, 1);
    let apply_fn = vm.new_native_function("apply", function_apply, 2);
    let bind_fn = vm.new_native_function("bind", function_bind, 1);
    let tostring_fn = vm.new_native_function("toString", function_to_string, 0);
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
            data_prop(Value::Object(function_proto_idx)),
        );
    });
    // The function prototype's `constructor` is the Function constructor.
    vm.heap.with_obj(function_proto_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(function_ctor_idx)),
        );
    });
    setup_collections(vm);
}

// =========================================================================
