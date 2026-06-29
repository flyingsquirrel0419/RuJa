//! Built-in objects and globals for the RuJa VM.
//!
//! All built-in constructors, prototypes, and global functions are registered
//! here. Native functions follow the `NativeFn` signature used by the VM.

use crate::environment as env;
use crate::error::{self, Error};
use crate::gc::Heap;
use crate::value::{
    ArrayData, BindingKind, FunctionData, FunctionKind, GcIdx, HeapObj, MapData, ObjectData,
    PropertyDescriptor, PropertyKey, SetData, Value,
};
use crate::vm::{NativeFn, Vm};
use indexmap::IndexMap;
use regex::Regex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use std::sync::Arc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn data_prop(value: Value) -> PropertyDescriptor {
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

fn install_methods(vm: &mut Vm, proto: &Value, methods: &[(Arc<str>, Value)]) {
    if let Value::Object(idx) = proto {
        vm.heap.with_obj(idx.0, |obj| {
            let props = obj.props();
            for (name, func) in methods {
                props
                    .lock()
                    .unwrap()
                    .insert(PropertyKey::from(name.clone()), data_prop(func.clone()));
            }
        });
    }
}

fn is_array(value: &Value, heap: &Heap) -> bool {
    match value {
        Value::Object(idx) => heap.with_obj(idx.0, |obj| matches!(obj, HeapObj::Array(_))),
        _ => false,
    }
}

fn is_callable(value: &Value, heap: &Heap) -> bool {
    match value {
        Value::Object(idx) => heap.with_obj(idx.0, |obj| obj.is_function()),
        _ => false,
    }
}

fn object_to_string(
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
                    if let Some(Value::Object(pidx)) = obj.proto().lock().unwrap().as_ref().cloned()
                    {
                        let constructor = vm.heap.with_obj(pidx.0, |p| {
                            p.props()
                                .lock()
                                .unwrap()
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

fn make_builtin_constructor(
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
        obj.props().lock().unwrap().insert(
            PropertyKey::from("prototype"),
            data_prop(Value::Object(proto_idx)),
        );
    });
    // prototype.constructor
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props().lock().unwrap().insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(ctor_idx)),
        );
        obj.props().lock().unwrap().insert(
            PropertyKey::from("name"),
            data_prop(Value::String(Arc::from(name))),
        );
        obj.props().lock().unwrap().insert(
            PropertyKey::from("message"),
            data_prop(Value::String(Arc::from(""))),
        );
    });

    (ctor_idx, proto_idx)
}

fn make_error_constructor(vm: &mut Vm, name: &str) -> (GcIdx, GcIdx) {
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
        obj.props().lock().unwrap().insert(
            PropertyKey::from("prototype"),
            data_prop(Value::Object(proto_idx)),
        );
    });
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props().lock().unwrap().insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(ctor_idx)),
        );
        obj.props().lock().unwrap().insert(
            PropertyKey::from("name"),
            data_prop(Value::String(Arc::from(name))),
        );
        obj.props().lock().unwrap().insert(
            PropertyKey::from("message"),
            data_prop(Value::String(Arc::from(""))),
        );
    });

    (ctor_idx, proto_idx)
}

fn define_global(vm: &mut Vm, name: &str, value: Value) {
    env::declare(&vm.heap, vm.global, name, value, BindingKind::Var);
}

fn get_arg(args: &[Value], idx: usize) -> Value {
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
        let first = &args[0];
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
    let first = &args[0];
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
                    .unwrap()
                    .contains_key(&crate::value::PropertyKey::from(key.as_str()))
                    || {
                        if let HeapObj::Array(a) = obj {
                            if key == "length" {
                                return true;
                            }
                            if let Ok(i) = key.parse::<usize>() {
                                return i < a.items.lock().unwrap().len();
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
                od.primitive.lock().unwrap().clone()
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
fn own_string_keys(vm: &mut Vm, obj: &Value) -> Vec<Arc<str>> {
    let mut keys = Vec::new();
    if let Value::Object(idx) = obj {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Array(a) = o {
                for i in 0..a.items.lock().unwrap().len() {
                    keys.push(Arc::from(i.to_string().as_str()));
                }
            }
            if let HeapObj::Map(m) = o {
                for (k, _) in m.entries.lock().unwrap().iter() {
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
            for (k, desc) in o.props().lock().unwrap().iter() {
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

fn make_value_array(vm: &mut Vm, items: Vec<Value>) -> Value {
    let arr = HeapObj::Array(ArrayData {
        items: Mutex::new(items),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
    });
    Value::Object(GcIdx(vm.heap.allocate(arr)))
}
fn norm_idx(n: f64, len: f64) -> f64 {
    if n < 0.0 {
        (len + n).max(0.0)
    } else {
        n.min(len)
    }
}

fn make_str_array(vm: &mut Vm, strs: Vec<Arc<str>>) -> Value {
    let items: Vec<Value> = strs.into_iter().map(Value::String).collect();
    let arr = HeapObj::Array(ArrayData {
        items: Mutex::new(items),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
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
    let mut vals = Vec::new();
    if let Value::Object(idx) = &obj {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Array(a) = o {
                vals.extend(a.items.lock().unwrap().clone());
            }
            if let HeapObj::Map(m) = o {
                for (_, v) in m.entries.lock().unwrap().iter() {
                    vals.push(v.clone());
                }
            }
            for (_k, desc) in o.props().lock().unwrap().iter() {
                if desc.enumerable {
                    vals.push(desc.value.clone());
                }
            }
        });
    }
    let arr = HeapObj::Array(ArrayData {
        items: Mutex::new(vals),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
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
        });
        pairs.push(Value::Object(GcIdx(vm.heap.allocate(pair))));
    }
    let arr = HeapObj::Array(ArrayData {
        items: Mutex::new(pairs),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
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
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        for pair in &pairs {
            // Each pair is an array [key, value].
            if let Value::Object(pi) = pair {
                let (k, v) = vm.heap.with_obj(pi.0, |o| {
                    if let HeapObj::Array(a) = o {
                        let it = a.items.lock().unwrap();
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
                        obj.props.lock().unwrap().insert(
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
            o.proto().lock().unwrap().clone().unwrap_or(Value::Null)
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
            *o.proto().lock().unwrap() = p;
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
                for d in od.props.lock().unwrap().values_mut() {
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
                let all_noncfg = od.props.lock().unwrap().values().all(|d| !d.configurable);
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
                    .unwrap()
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
                    .with_obj(idx.0, |o| o.props().lock().unwrap().get(&pkey).cloned())?;
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
                    *od.props.lock().unwrap() = dp;
                }
            });
            p.insert(
                PropertyKey::from(key.as_str()),
                data_prop(Value::Object(GcIdx(desc_obj))),
            );
        }
        vm.heap.with_obj(result_idx, |o| {
            if let HeapObj::Object(od) = o {
                *od.props.lock().unwrap() = p;
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
                .unwrap()
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
                    *od.props.lock().unwrap() = p;
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
                for d in o.props.lock().unwrap().values_mut() {
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
                .unwrap()
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
        obj.proto().lock().unwrap().as_ref().and_then(|p| {
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
                    .unwrap()
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
                    .unwrap()
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
                .unwrap()
                .insert(PropertyKey::from("message"), data_prop(Value::String(msg)));
            o.props
                .lock()
                .unwrap()
                .insert(PropertyKey::from("name"), data_prop(Value::String(name)));
            if let Some(c) = cause {
                o.props
                    .lock()
                    .unwrap()
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
                .unwrap()
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
// Math
// =========================================================================
fn math_unary(f: fn(f64) -> f64, vm: &mut Vm, args: &[Value]) -> error::Result<Value> {
    let n = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    Ok(Value::Number(f(n)))
}
fn math_floor(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::floor, vm, args)
}
fn math_ceil(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::ceil, vm, args)
}
fn math_round(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    // ES Math.round: round half towards +Infinity (equivalent to floor(x + 0.5)
    // for finite x), so Math.round(-0.5) === 0, Math.round(0.5) === 1.
    math_unary(
        |n| {
            if n.is_nan() || n.is_infinite() {
                n
            } else {
                (n + 0.5).floor()
            }
        },
        vm,
        args,
    )
}
fn math_trunc(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::trunc, vm, args)
}
fn math_abs(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::abs, vm, args)
}
fn math_sign(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let n = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    Ok(Value::Number(if n > 0.0 {
        1.0
    } else if n < 0.0 {
        -1.0
    } else {
        0.0
    }))
}
fn math_sqrt(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::sqrt, vm, args)
}
fn math_cbrt(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::cbrt, vm, args)
}
fn math_exp(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::exp, vm, args)
}
fn math_log(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::ln, vm, args)
}
fn math_log2(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::log2, vm, args)
}
fn math_log10(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::log10, vm, args)
}

fn math_hypot(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let mut sum = 0.0f64;
    let mut has_inf = false;
    for a in args {
        let n = vm.to_number(a)?;
        if n.is_infinite() {
            has_inf = true;
        } else {
            sum += n * n;
        }
    }
    Ok(Value::Number(if has_inf {
        f64::INFINITY
    } else {
        sum.sqrt()
    }))
}
fn math_atan2(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let y = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    let x = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    Ok(Value::Number(y.atan2(x)))
}
fn math_asin(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::asin, vm, args)
}
fn math_acos(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::acos, vm, args)
}
fn math_atan(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::atan, vm, args)
}
fn math_sinh(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::sinh, vm, args)
}
fn math_cosh(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::cosh, vm, args)
}
fn math_tanh(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::tanh, vm, args)
}
fn math_expm1(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::exp_m1, vm, args)
}
fn math_log1p(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::ln_1p, vm, args)
}
fn math_clz32(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let n = vm.to_number(args.first().unwrap_or(&Value::Undefined))? as u32;
    Ok(Value::Number(n.leading_zeros() as f64))
}
fn math_imul(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let a = vm.to_number(args.first().unwrap_or(&Value::Undefined))? as i32;
    let b = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))? as i32;
    Ok(Value::Number((a.wrapping_mul(b)) as f64))
}
fn math_fround(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let n = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    Ok(Value::Number(n as f32 as f64))
}
fn math_sin(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::sin, vm, args)
}
fn math_cos(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::cos, vm, args)
}
fn math_tan(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::tan, vm, args)
}
fn math_pow(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let a = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    let b = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    Ok(Value::Number(a.powf(b)))
}
fn math_max(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let mut m = f64::NEG_INFINITY;
    for a in args {
        let n = vm.to_number(a)?;
        if n > m {
            m = n;
        }
    }
    Ok(Value::Number(m))
}
fn math_min(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let mut m = f64::INFINITY;
    for a in args {
        let n = vm.to_number(a)?;
        if n < m {
            m = n;
        }
    }
    Ok(Value::Number(m))
}
fn math_random(_vm: &mut Vm, _args: &[Value], _: Option<Value>) -> error::Result<Value> {
    thread_local! { static STATE: AtomicU64 = const { AtomicU64::new(0x2545F4914F6CDD1D) }; }
    let r = STATE.with(|s| {
        let mut x = s.load(Ordering::Relaxed);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.store(x, Ordering::Relaxed);
        x as f64 / u64::MAX as f64
    });
    Ok(Value::Number(r))
}

fn build_math(vm: &mut Vm) -> Value {
    let mut props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    // build methods first, collect into a temp vec
    let mut method_entries: Vec<(&str, NativeFn, usize)> = vec![
        ("floor", math_floor, 1),
        ("ceil", math_ceil, 1),
        ("round", math_round, 1),
        ("trunc", math_trunc, 1),
        ("abs", math_abs, 1),
        ("sign", math_sign, 1),
        ("sqrt", math_sqrt, 1),
        ("cbrt", math_cbrt, 1),
        ("exp", math_exp, 1),
        ("log", math_log, 1),
        ("log2", math_log2, 1),
        ("log10", math_log10, 1),
        ("log1p", math_log1p, 1),
        ("expm1", math_expm1, 1),
        ("sin", math_sin, 1),
        ("asin", math_asin, 1),
        ("acos", math_acos, 1),
        ("atan", math_atan, 1),
        ("atan2", math_atan2, 2),
        ("sinh", math_sinh, 1),
        ("cosh", math_cosh, 1),
        ("tanh", math_tanh, 1),
        ("hypot", math_hypot, 2),
        ("clz32", math_clz32, 1),
        ("fround", math_fround, 1),
        ("imul", math_imul, 2),
        ("cos", math_cos, 1),
        ("tan", math_tan, 1),
        ("pow", math_pow, 2),
        ("max", math_max, 2),
        ("min", math_min, 2),
        ("random", math_random, 0),
    ];
    for (name, f, len) in method_entries.drain(..) {
        let idx = vm.new_native_function(name, f, len);
        props.insert(PropertyKey::from(name), data_prop(Value::Object(idx)));
    }
    props.insert(
        PropertyKey::from("PI"),
        data_prop(Value::Number(std::f64::consts::PI)),
    );
    props.insert(
        PropertyKey::from("E"),
        data_prop(Value::Number(std::f64::consts::E)),
    );
    props.insert(
        PropertyKey::from("LN2"),
        data_prop(Value::Number(std::f64::consts::LN_2)),
    );
    props.insert(
        PropertyKey::from("LN10"),
        data_prop(Value::Number(std::f64::consts::LN_10)),
    );
    props.insert(
        PropertyKey::from("LOG2E"),
        data_prop(Value::Number(std::f64::consts::LOG2_E)),
    );
    props.insert(
        PropertyKey::from("LOG10E"),
        data_prop(Value::Number(std::f64::consts::LOG10_E)),
    );
    props.insert(
        PropertyKey::from("SQRT2"),
        data_prop(Value::Number(std::f64::consts::SQRT_2)),
    );
    props.insert(
        PropertyKey::from("SQRT1_2"),
        data_prop(Value::Number(std::f64::consts::FRAC_1_SQRT_2)),
    );
    let obj = HeapObj::Object(ObjectData {
        props: Mutex::new(props),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(false),
        class_name: Some(Arc::from("Math")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    Value::Object(GcIdx(vm.heap.allocate(obj)))
}

// =========================================================================
// console
// =========================================================================
fn console_log(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let parts: Vec<String> = args
        .iter()
        .map(|a| format_for_console(vm, a, 0).unwrap_or_default())
        .collect();
    println!("{}", parts.join(" "));
    Ok(Value::Undefined)
}

/// Format a value for console output, approximating Node.js's inspect format:
/// arrays as `[ 1, 2, 3 ]`, objects as `{ a: 1, b: 2 }`, strings unquoted
/// at top level, nested strings quoted.
fn format_for_console(vm: &mut Vm, v: &Value, depth: usize) -> error::Result<String> {
    if depth > 6 {
        return Ok(vm.to_string(v)?.to_string());
    }
    match v {
        Value::Undefined => Ok("undefined".to_string()),
        Value::Null => Ok("null".to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) => Ok(crate::value::num_to_string(*n)),
        Value::BigInt(n) => Ok(format!("{}n", n)),
        Value::String(s) => {
            if depth == 0 {
                Ok(s.to_string())
            } else {
                Ok(format!("'{}'", s))
            }
        }
        Value::Symbol(_) => Ok("Symbol()".to_string()),
        Value::Object(idx) => {
            let (is_array, is_func, items, pairs) = vm.heap.with_obj(idx.0, |o| {
                let is_array = matches!(o, HeapObj::Array(_));
                let is_func = matches!(o, HeapObj::Function(_));
                let items = if let HeapObj::Array(a) = o {
                    a.items.lock().unwrap().clone()
                } else {
                    Vec::new()
                };
                let pairs: Vec<(Arc<str>, Value)> = o
                    .props()
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|(_, d)| d.enumerable)
                    .filter_map(|(k, d)| {
                        if let crate::value::PropertyKey::Str(s) = k {
                            Some((s.clone(), d.value.clone()))
                        } else {
                            None
                        }
                    })
                    .collect();
                (is_array, is_func, items, pairs)
            });
            if is_func {
                return Ok("[Function]".to_string());
            }
            if is_array {
                if items.is_empty() {
                    return Ok("[]".to_string());
                }
                let inner: Vec<String> = items
                    .iter()
                    .map(|i| format_for_console(vm, i, depth + 1))
                    .collect::<error::Result<Vec<_>>>()?;
                return Ok(format!("[ {} ]", inner.join(", ")));
            }
            if pairs.is_empty() {
                return Ok("{}".to_string());
            }
            let inner: Vec<String> = pairs
                .iter()
                .map(|(k, v)| {
                    let val = format_for_console(vm, v, depth + 1)?;
                    Ok(format!("{}: {}", k, val))
                })
                .collect::<error::Result<Vec<_>>>()?;
            Ok(format!("{{ {} }}", inner.join(", ")))
        }
    }
}
fn build_console(vm: &mut Vm) -> Value {
    let mut props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    for name in &["log", "error", "warn", "info", "debug", "dir", "trace"] {
        let idx = vm.new_native_function(name, console_log, 0);
        props.insert(PropertyKey::from(*name), data_prop(Value::Object(idx)));
    }
    let obj = HeapObj::Object(ObjectData {
        props: Mutex::new(props),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Object")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    Value::Object(GcIdx(vm.heap.allocate(obj)))
}

// =========================================================================
// JSON
// =========================================================================
fn json_stringify(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
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
                    a.items.lock().unwrap().clone()
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
    match stringify_value(vm, &v, &mut Vec::new(), "", &mut ctx) {
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
        HeapObj::Array(a) => a.items.lock().unwrap().clone(),
        HeapObj::Object(o) => o
            .props
            .lock()
            .unwrap()
            .values()
            .filter(|d| d.enumerable)
            .map(|d| d.value.clone())
            .collect(),
        _ => Vec::new(),
    });
    let result = children.iter().any(|c| has_json_cycle(vm, c, seen));
    seen.pop();
    result
}
fn stringify_value(
    vm: &mut Vm,
    v: &Value,
    seen: &mut Vec<usize>,
    indent: &str,
    ctx: &mut StringifyCtx,
) -> Option<String> {
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
        Value::Object(idx) => {
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
                HeapObj::Array(a) => (true, a.items.lock().unwrap().clone(), IndexMap::new()),
                HeapObj::Object(o) => (false, Vec::new(), o.props.lock().unwrap().clone()),
                HeapObj::Function(_) => (false, Vec::new(), IndexMap::new()),
                _ => (false, Vec::new(), obj.props().lock().unwrap().clone()),
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
                        let s = stringify_value(vm, &val, seen, &child_indent, ctx);
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
                let keys: Vec<(String, Value)> = if let Some(wl) = &ctx.whitelist {
                    props
                        .iter()
                        .filter_map(|(k, d)| {
                            let ks = match k {
                                crate::value::PropertyKey::Str(s) => s.to_string(),
                                _ => return None,
                            };
                            if wl.contains(&ks) && d.enumerable {
                                Some((ks, d.value.clone()))
                            } else {
                                None
                            }
                        })
                        .collect()
                } else {
                    props
                        .iter()
                        .filter_map(|(k, d)| {
                            if !d.enumerable {
                                return None;
                            }
                            match k {
                                crate::value::PropertyKey::Str(s) => {
                                    Some((s.to_string(), d.value.clone()))
                                }
                                _ => None,
                            }
                        })
                        .collect()
                };
                for (key_str, val) in keys {
                    let val =
                        apply_replacer(vm, ctx, &Value::String(Arc::from(key_str.as_str())), &val);
                    if let Some(vs) = stringify_value(vm, &val, seen, &child_indent, ctx) {
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

fn json_parse(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
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
    let parsed = parse_json_value(vm, &mut s.chars().peekable())?;
    if is_reviver_fn {
        if let Some(rf) = reviver {
            return apply_reviver(vm, &rf, &Value::String(Arc::from("")), &parsed);
        }
    }
    Ok(parsed)
}

/// Walk the parsed tree bottom-up, calling reviver(key, value) on each.
fn apply_reviver(vm: &mut Vm, reviver: &Value, key: &Value, val: &Value) -> error::Result<Value> {
    let walked = match val {
        Value::Object(idx) => {
            let (is_arr, items, props) = vm.heap.with_obj(idx.0, |o| match o {
                HeapObj::Array(a) => (true, a.items.lock().unwrap().clone(), IndexMap::new()),
                HeapObj::Object(o) => (false, Vec::new(), o.props.lock().unwrap().clone()),
                _ => (false, Vec::new(), IndexMap::new()),
            });
            if is_arr {
                let mut new_items = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    let k = Value::String(Arc::from(i.to_string().as_str()));
                    let w = apply_reviver(vm, reviver, &k, item)?;
                    if !w.is_undefined() {
                        new_items.push(w);
                    }
                }
                Value::Object(GcIdx(vm.heap.allocate(HeapObj::Array(
                    crate::value::ArrayData {
                        items: Mutex::new(new_items),
                        props: Mutex::new(IndexMap::new()),
                        proto: Mutex::new(Some(vm.array_proto.clone())),
                    },
                ))))
            } else {
                let mut new_props = IndexMap::new();
                for (pk, d) in &props {
                    if let crate::value::PropertyKey::Str(s) = pk {
                        let k = Value::String(s.clone());
                        let w = apply_reviver(vm, reviver, &k, &d.value)?;
                        if !w.is_undefined() {
                            let mut desc = data_prop(w);
                            desc.enumerable = true;
                            new_props.insert(pk.clone(), desc);
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
                ))))
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
) -> error::Result<Value> {
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
            parse_json_obj(vm, chars)
        }
        Some(&'[') => {
            chars.next();
            parse_json_arr(vm, chars)
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
        let val = parse_json_value(vm, chars)?;
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
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj))))
}
fn parse_json_arr(
    vm: &mut Vm,
    chars: &mut std::iter::Peekable<std::str::Chars>,
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
        items.push(parse_json_value(vm, chars)?);
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
    let obj = HeapObj::Array(ArrayData {
        items: Mutex::new(items),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj))))
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
fn date_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let ts = if args.is_empty() {
        now_ms()
    } else if args.len() == 1 {
        vm.to_number(&args[0])?
    } else {
        // Approximate: use the first numeric arg as a timestamp.
        vm.to_number(&args[0])?
    };
    if let Some(Value::Object(idx)) = &this {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Object(o) = o {
                o.props
                    .lock()
                    .unwrap()
                    .insert(PropertyKey::from("__time__"), data_prop(Value::Number(ts)));
            }
        });
        Ok(this.unwrap())
    } else {
        Ok(Value::String(Arc::from(format!("{}", ts as i64).as_str())))
    }
}
fn date_get_time(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = &this {
        let ts = vm.heap.with_obj(idx.0, |o| {
            o.props()
                .lock()
                .unwrap()
                .get(&PropertyKey::from("__time__"))
                .map(|d| d.value.clone())
        });
        if let Some(Value::Number(n)) = ts {
            return Ok(Value::Number(n));
        }
    }
    Ok(Value::Number(f64::NAN))
}
fn date_to_string(_vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = &this {
        let _ = idx;
    }
    Ok(Value::String(Arc::from("Date")))
}
fn date_now(_vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Number(now_ms()))
}

fn reflect_get(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let key = match args.get(1) {
        Some(v) => vm.to_property_key(v)?,
        None => return Ok(Value::Undefined),
    };
    vm.get_property(&target, &key)
}
fn reflect_set(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let key = match args.get(1) {
        Some(v) => vm.to_property_key(v)?,
        None => return Ok(Value::Bool(false)),
    };
    let value = args.get(2).cloned().unwrap_or(Value::Undefined);
    match vm.set_property(&target, &key, value) {
        Ok(()) => Ok(Value::Bool(true)),
        Err(_) => Ok(Value::Bool(false)),
    }
}
fn reflect_has(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let key = match args.get(1) {
        Some(v) => vm.to_property_key(v)?,
        None => return Ok(Value::Bool(false)),
    };
    let has = vm
        .get_property(&target, &key)
        .map(|v| !v.is_undefined())
        .unwrap_or(false);
    Ok(Value::Bool(has))
}
fn reflect_delete_property(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let key = match args.get(1) {
        Some(v) => vm.to_property_key(v)?,
        None => return Ok(Value::Bool(false)),
    };
    vm.delete_property(&target, &key)
        .map(|_| Value::Bool(true))
        .or(Ok(Value::Bool(false)))
}
fn reflect_own_keys(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let keys = own_string_keys(vm, &target);
    Ok(make_str_array(vm, keys))
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
                a.items.lock().unwrap().clone()
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
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    vm.construct(&target, &call_args)
}

fn build_reflect(vm: &mut Vm) -> Value {
    let mut props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    let entries: &[(&str, NativeFn, usize)] = &[
        ("get", reflect_get as NativeFn, 2),
        ("set", reflect_set as NativeFn, 3),
        ("has", reflect_has as NativeFn, 2),
        ("deleteProperty", reflect_delete_property as NativeFn, 2),
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
        let idx = vm.new_native_function(name, *f, *len);
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
    Value::Object(GcIdx(vm.heap.allocate(obj)))
}

fn build_json(vm: &mut Vm) -> Value {
    let mut props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    let pi = vm.new_native_function("parse", json_parse, 1);
    let si = vm.new_native_function("stringify", json_stringify, 3);
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
    Value::Object(GcIdx(vm.heap.allocate(obj)))
}

// =========================================================================
// Global functions
// =========================================================================
fn global_parse_int(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let input = match args.first() {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(v) => vm.to_string(v)?.to_string(),
        None => return Ok(Value::Number(f64::NAN)),
    };
    let mut radix = args
        .get(1)
        .and_then(|v| {
            if let Value::Number(n) = v {
                Some(*n as u32)
            } else {
                None
            }
        })
        .unwrap_or(0);
    let strip_hex = radix == 0 || radix == 16;
    let mut chars = input.char_indices().peekable();
    let neg = match chars.peek() {
        Some((_, '+')) => {
            chars.next();
            false
        }
        Some((_, '-')) => {
            chars.next();
            true
        }
        _ => false,
    };
    if strip_hex {
        let is_hex = matches!(chars.peek(), Some((_, '0')))
            && matches!(chars.clone().nth(1), Some((_, 'x')) | Some((_, 'X')));
        if is_hex {
            chars.next();
            chars.next();
            radix = 16;
        }
    }
    if radix == 0 {
        radix = 10;
    }
    if !(2..=36).contains(&radix) {
        return Ok(Value::Number(f64::NAN));
    }
    let valid = |c: char| c.is_digit(radix);
    let start = chars.peek().map(|(i, _)| *i).unwrap_or(input.len());
    let digits_end = input[start..]
        .char_indices()
        .find(|(_, c)| !valid(*c))
        .map(|(i, _)| start + i)
        .unwrap_or(input.len());
    let digits = &input[start..digits_end];
    if digits.is_empty() {
        return Ok(Value::Number(f64::NAN));
    }
    match i64::from_str_radix(digits, radix) {
        Ok(n) => Ok(Value::Number(if neg { -(n as f64) } else { n as f64 })),
        Err(_) => Ok(Value::Number(f64::NAN)),
    }
}
fn global_parse_float(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let s = match args.first() {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(v) => vm.to_string(v)?.to_string(),
        None => return Ok(Value::Number(f64::NAN)),
    };
    Ok(Value::Number(s.parse().unwrap_or(f64::NAN)))
}
fn global_is_nan(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let n = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    Ok(Value::Bool(n.is_nan()))
}
fn global_is_finite(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let n = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    Ok(Value::Bool(n.is_finite()))
}

/// `BigInt(x)`: convert a number, string, or boolean to a BigInt. Throws
/// RangeError for non-integral numbers and SyntaxError for unparseable strings.
fn global_bigint(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let arg = args.first().unwrap_or(&Value::Undefined);
    match arg {
        Value::BigInt(n) => Ok(Value::BigInt(n.clone())),
        Value::Bool(b) => Ok(Value::BigInt(num_bigint::BigInt::from(if *b {
            1
        } else {
            0
        }))),
        Value::Number(n) => {
            if n.is_nan() || n.is_infinite() || n.fract() != 0.0 {
                Err(Error::range(format!(
                    "The number {} cannot be converted to a BigInt because it is not an integer",
                    crate::value::num_to_string(*n)
                )))
            } else {
                Ok(Value::BigInt(num_bigint::BigInt::from(*n as i64)))
            }
        }
        Value::String(s) => {
            let t = s.trim();
            num_bigint::BigInt::parse_bytes(t.as_bytes(), 10)
                .map(Value::BigInt)
                .ok_or_else(|| Error::syntax(format!("Cannot convert {} to a BigInt", s)))
        }
        _ => Err(Error::syntax("Cannot convert to a BigInt".to_string())),
    }
}

/// `BigInt.prototype.toString()`: returns the decimal string of the BigInt.
fn bigint_to_string(_vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let _ = args;
    let v = match this {
        Some(Value::BigInt(n)) => n.clone(),
        Some(_) => num_bigint::BigInt::from(0),
        None => num_bigint::BigInt::from(0),
    };
    Ok(Value::String(Arc::from(v.to_string().as_str())))
}

/// `eval(x)`: if `x` is not a string, return it as-is. Otherwise parse and
/// run it. A direct `eval(...)` call (detected via the CallDirectEval opcode)
/// runs in the caller's scope; an indirect eval runs in the global scope.
fn global_eval(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let arg = args.first().cloned().unwrap_or(Value::Undefined);
    let src = match &arg {
        Value::String(s) => s.to_string(),
        // Non-string: return unchanged.
        _ => return Ok(arg),
    };
    // Default (indirect) behavior: run in the global scope.
    vm.eval_indirect(&src)
}

/// `new Function(p0, p1, ..., body)`: dynamically build a function from a
/// parameter list and a body source string. The last argument is the body;
/// earlier arguments are parameter names (comma-separated within each).
fn function_constructor(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    use crate::ast::FunctionExpr;
    use crate::value::{FunctionData, FunctionKind};
    use std::sync::Arc;

    // Build the parameter source: all args except the last, joined by commas.
    let (params_src, body_src) = if args.is_empty() {
        (String::new(), String::new())
    } else if args.len() == 1 {
        (String::new(), vm.to_string(&args[0])?.to_string())
    } else {
        let body = vm.to_string(&args[args.len() - 1])?.to_string();
        let params = args[..args.len() - 1]
            .iter()
            .map(|a| vm.to_string(a).map(|s| s.to_string()))
            .collect::<error::Result<Vec<String>>>()?
            .join(",");
        (params, body)
    };

    // Parse params + body together by wrapping in `function _f(PARAMS){ BODY }`,
    // so directives (e.g. "use strict") in the body are honored and the body
    // is parsed as a function statement list (not a top-level block).
    let wrapped = format!("function _f({}) {{ {} }}", params_src, body_src);
    let prog = crate::parser::Parser::parse(&wrapped)?;
    let params_fn = prog
        .body
        .into_iter()
        .find_map(|st| match st.node {
            crate::ast::StmtNode::FunctionDecl(f) => Some(f),
            _ => None,
        })
        .ok_or_else(|| error::Error::syntax("invalid Function body".to_string()))?;
    let params = params_fn.params.clone();
    let param_defaults = params_fn.param_defaults.clone();
    let rest_param = params_fn.rest_param.clone();
    let body = params_fn.body.clone();
    // The parser already applied directive-inherited strictness; a body-level
    // "use strict" is reflected in the parsed function (is_strict).
    let is_strict = params_fn.is_strict;
    let f = FunctionExpr {
        name: Some(Arc::from("anonymous")),
        params,
        param_defaults,
        rest_param,
        body,
        is_arrow: false,
        is_async: false,
        is_generator: false,
        param_decls: Vec::new(),
        is_strict,
    };
    let mut compiler = crate::compiler::Compiler::new();
    let (chunk, param_slots) = compiler.compile_function(&f)?;
    let fdef = std::sync::Arc::new(crate::function::FunctionDef {
        name: Some(Arc::from("anonymous")),
        params: f.params.clone(),
        param_slots,
        rest_param: f.rest_param.clone(),
        chunk: std::sync::Arc::new(chunk),
        num_locals: f.params.len() + 16,
        is_arrow: false,
        is_async: false,
        is_generator: false,
    });
    vm.functions.push(fdef.clone());
    let func_idx = vm.functions.len() - 1;
    // Create the function object with a fresh prototype.
    let proto = HeapObj::Object(crate::value::ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    let proto_val = Value::Object(GcIdx(vm.heap.allocate(proto)));
    let fd = FunctionData {
        name: Some(Arc::from("anonymous")),
        kind: FunctionKind::Interpreted { func: fdef },
        closure: vm.global,
        prototype: Mutex::new(Some(proto_val.clone())),
        proto: Mutex::new(match vm.function_proto {
            Value::Object(_) => Some(vm.function_proto.clone()),
            _ => None,
        }),
        props: Mutex::new(IndexMap::new()),
    };
    let f_idx = vm.heap.allocate(HeapObj::Function(fd));
    // link prototype.constructor back to the function
    if let Value::Object(pidx) = &proto_val {
        vm.heap.with_obj(pidx.0, |obj| {
            let mut desc = crate::value::PropertyDescriptor::data(Value::Object(GcIdx(f_idx)));
            desc.enumerable = false;
            obj.props()
                .lock()
                .unwrap()
                .insert(crate::value::PropertyKey::from("constructor"), desc);
        });
    }
    // Emit MakeClosure at top level is not needed; the function object is
    // already fully formed. We do NOT push a frame; the caller invokes it.
    let _ = func_idx;
    Ok(Value::Object(GcIdx(f_idx)))
}

// =========================================================================
// Array prototype + constructor
// =========================================================================

fn array_from(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let src_val = args.first().cloned().unwrap_or(Value::Undefined);
    let map_fn = args.get(1).cloned();
    // Array-like or iterable
    let mut items: Vec<Value> = Vec::new();
    if let Value::Object(idx) = &src_val {
        let (is_arr, arr_items, len) = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Array(a) = o {
                (true, a.items.lock().unwrap().clone(), 0)
            } else if let HeapObj::Iterator(_) = o {
                (false, Vec::new(), 0)
            } else {
                let len = o
                    .props()
                    .lock()
                    .unwrap()
                    .get(&crate::value::PropertyKey::from("length"))
                    .and_then(|d| {
                        if let Value::Number(n) = d.value {
                            Some(n as usize)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                (false, Vec::new(), len)
            }
        });
        if is_arr {
            items = arr_items;
        } else {
            // array-like: read index 0..len
            for i in 0..len {
                let key = i.to_string();
                let v = vm.get_property(&src_val, &key)?;
                items.push(v);
            }
        }
    } else if let Value::String(s) = &src_val {
        for ch in s.chars() {
            items.push(Value::String(Arc::from(ch.to_string().as_str())));
        }
    }
    if let Some(mfn) = map_fn {
        let mut mapped = Vec::new();
        for (i, v) in items.iter().enumerate() {
            mapped.push(vm.call_function(&mfn, &[v.clone(), Value::Number(i as f64)], None)?);
        }
        items = mapped;
    }
    Ok(make_value_array(vm, items))
}
fn array_of(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    Ok(make_value_array(vm, args.to_vec()))
}

fn array_is_array(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(is_array(
        args.first().unwrap_or(&Value::Undefined),
        &vm.heap,
    )))
}
fn array_push(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().extend_from_slice(args);
            }
        });
        let len = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().len()
            } else {
                0
            }
        });
        return Ok(Value::Number(len as f64));
    }
    Ok(Value::Number(0.0))
}
fn array_pop(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        return Ok(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().pop().unwrap_or(Value::Undefined)
            } else {
                Value::Undefined
            }
        }));
    }
    Ok(Value::Undefined)
}
/// Array.prototype.toString: delegates to join(",") (Object.prototype.toString
/// would otherwise return "[object Array]").
fn array_to_string(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    array_join(vm, &[], this)
}

fn array_join(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let sep = match args.first() {
        Some(Value::String(s)) => s.to_string(),
        Some(v) if !v.is_undefined() => vm.to_string(v)?.to_string(),
        _ => ",".to_string(),
    };
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let parts: Vec<String> = items
            .iter()
            .map(|i| {
                if i.is_nullish() {
                    String::new()
                } else {
                    vm.to_string(i).map(|s| s.to_string()).unwrap_or_default()
                }
            })
            .collect();
        return Ok(Value::String(Arc::from(parts.join(&sep).as_str())));
    }
    Ok(Value::String(Arc::from("")))
}
fn array_map(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let mut result = Vec::new();
        for (i, item) in items.iter().enumerate() {
            result.push(vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?);
        }
        let arr = HeapObj::Array(ArrayData {
            items: Mutex::new(result),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.array_proto.clone())),
        });
        return Ok(Value::Object(GcIdx(vm.heap.allocate(arr))));
    }
    Ok(Value::Undefined)
}
fn array_filter(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let mut result = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let keep = vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?;
            if keep.is_truthy() {
                result.push(item.clone());
            }
        }
        let arr = HeapObj::Array(ArrayData {
            items: Mutex::new(result),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.array_proto.clone())),
        });
        return Ok(Value::Object(GcIdx(vm.heap.allocate(arr))));
    }
    Ok(Value::Undefined)
}
fn array_reduce(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let (mut acc, start) = if args.len() >= 2 {
            (args[1].clone(), 0)
        } else {
            (items.first().cloned().unwrap_or(Value::Undefined), 1)
        };
        if items.is_empty() && args.len() < 2 {
            return Err(Error::type_err(
                "Reduce of empty array with no initial value",
            ));
        }
        for (i, item) in items.iter().enumerate().skip(start) {
            acc = vm.call_function(
                &cb,
                &[
                    acc,
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(2).cloned(),
            )?;
        }
        return Ok(acc);
    }
    Ok(Value::Undefined)
}
/// Build a heap array from a Vec of values.
fn make_array(vm: &mut Vm, items: Vec<Value>) -> Value {
    let idx = vm.heap.allocate(HeapObj::Array(crate::value::ArrayData {
        items: Mutex::new(items),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
    }));
    Value::Object(GcIdx(idx))
}

/// Normalize an array index argument (negative wraps from end).
fn norm_index(v: Value, len: f64, vm: &mut Vm) -> error::Result<usize> {
    let n = vm.to_number(&v)?;
    if n < 0.0 {
        Ok(((len + n).max(0.0)) as usize)
    } else {
        Ok((n as usize).min(len as usize))
    }
}

/// Sort items with an optional comparator callback (default: string compare).
fn sort_with_cb(vm: &mut Vm, items: &mut [Value], cmp: &Option<Value>) -> error::Result<()> {
    match cmp {
        None | Some(Value::Undefined) | Some(Value::Null) => {
            items.sort_by(|a, b| {
                if a.is_undefined() && b.is_undefined() {
                    return std::cmp::Ordering::Equal;
                }
                if a.is_undefined() {
                    return std::cmp::Ordering::Greater;
                }
                if b.is_undefined() {
                    return std::cmp::Ordering::Less;
                }
                let sa = vm.to_string(a).map(|s| s.to_string()).unwrap_or_default();
                let sb = vm.to_string(b).map(|s| s.to_string()).unwrap_or_default();
                sa.cmp(&sb)
            });
        }
        Some(cmp_fn) => {
            let n = items.len();
            for i in 1..n {
                let mut j = i;
                while j > 0 {
                    let a = items[j - 1].clone();
                    let b = items[j].clone();
                    let r = vm.call_function(cmp_fn, &[a.clone(), b.clone()], None)?;
                    let ord = vm.to_number(&r)?;
                    if ord > 0.0 {
                        items.swap(j - 1, j);
                        j -= 1;
                    } else {
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

fn array_reduce_right(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len();
        let (mut acc, start) = if args.len() >= 2 {
            (args[1].clone(), len)
        } else {
            (
                items.last().cloned().unwrap_or(Value::Undefined),
                len.saturating_sub(1),
            )
        };
        if items.is_empty() && args.len() < 2 {
            return Err(Error::type_err(
                "Reduce of empty array with no initial value",
            ));
        }
        let mut i = start;
        while i > 0 {
            i -= 1;
            acc = vm.call_function(
                &cb,
                &[
                    acc,
                    items[i].clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(2).cloned(),
            )?;
        }
        return Ok(acc);
    }
    Ok(Value::Undefined)
}

fn array_to_reversed(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().iter().rev().cloned().collect()
            } else {
                Vec::new()
            }
        });
        return Ok(make_array(vm, items));
    }
    Ok(Value::Undefined)
}

fn array_to_sorted(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let mut items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let cb = args.first().cloned();
        sort_with_cb(vm, &mut items, &cb)?;
        return Ok(make_array(vm, items));
    }
    Ok(Value::Undefined)
}

fn array_to_spliced(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len() as f64;
        let start = norm_index(get_arg(args, 0), len, vm)?;
        let start = start.min(items.len());
        let del_count = if args.len() >= 2 {
            let d = vm.to_number(&get_arg(args, 1))?;
            let d = if d < 0.0 { 0.0 } else { d };
            (d as usize).min(items.len().saturating_sub(start))
        } else {
            items.len() - start
        };
        let mut result = items[..start].to_vec();
        for a in args.iter().skip(2) {
            result.push(a.clone());
        }
        result.extend_from_slice(&items[start + del_count..]);
        return Ok(make_array(vm, result));
    }
    Ok(Value::Undefined)
}

fn array_with(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let mut items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len() as f64;
        let index = norm_index(get_arg(args, 0), len, vm)?;
        if index >= items.len() {
            return Err(Error::range("Invalid array index"));
        }
        items[index] = get_arg(args, 1);
        return Ok(make_array(vm, items));
    }
    Ok(Value::Undefined)
}

fn array_for_each(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        for (i, item) in items.iter().enumerate() {
            vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?;
        }
    }
    Ok(Value::Undefined)
}
/// Resolve a `fromIndex`-style argument (ToInteger, default 0) to a starting
/// position clamped into `[0, len]`. Negative wraps from the end.
fn from_index_arg(vm: &mut Vm, args: &[Value], idx: usize, len: usize) -> error::Result<usize> {
    let raw = match args.get(idx) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    if raw.is_nan() || raw == 0.0 || raw.is_infinite() {
        // +Inf -> len, -Inf/-0/NaN -> 0
        return Ok(if raw.is_infinite() && raw > 0.0 {
            len
        } else {
            0
        });
    }
    let n = raw as i64;
    let start = if n < 0 {
        (len as i64 + n).max(0) as usize
    } else {
        (n as usize).min(len)
    };
    Ok(start)
}

fn array_index_of(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len();
        let start = from_index_arg(vm, args, 1, len)?;
        for (i, v) in items.iter().enumerate().skip(start) {
            if v == &target {
                return Ok(Value::Number(i as f64));
            }
        }
        return Ok(Value::Number(-1.0));
    }
    Ok(Value::Number(-1.0))
}
fn array_includes(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len();
        let start = from_index_arg(vm, args, 1, len)?;
        // includes uses SameValueZero: NaN matches NaN (unlike indexOf's ===).
        for (_i, v) in items.iter().enumerate().skip(start) {
            let matched = if let (Value::Number(x), Value::Number(y)) = (v, &target) {
                x.is_nan() && y.is_nan() || x == y
            } else {
                v == &target
            };
            if matched {
                return Ok(Value::Bool(true));
            }
        }
        return Ok(Value::Bool(false));
    }
    Ok(Value::Bool(false))
}
fn array_slice(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len() as i64;
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
        let s = if start < 0 {
            (len + start).max(0) as usize
        } else {
            (start as usize).min(items.len())
        };
        let e = if end < 0 {
            (len + end).max(0) as usize
        } else {
            (end as usize).min(items.len())
        };
        let sliced = if s < e {
            items[s..e].to_vec()
        } else {
            Vec::new()
        };
        let arr = HeapObj::Array(ArrayData {
            items: Mutex::new(sliced),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.array_proto.clone())),
        });
        return Ok(Value::Object(GcIdx(vm.heap.allocate(arr))));
    }
    Ok(Value::Undefined)
}
fn array_concat(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let mut items = Vec::new();
    if let Some(Value::Object(idx)) = this {
        items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
    }
    for a in args {
        if let Value::Object(aidx) = a {
            let is_arr = vm
                .heap
                .with_obj(aidx.0, |obj| matches!(obj, HeapObj::Array(_)));
            if is_arr {
                let extra = vm.heap.with_obj(aidx.0, |obj| {
                    if let HeapObj::Array(a) = obj {
                        a.items.lock().unwrap().clone()
                    } else {
                        Vec::new()
                    }
                });
                items.extend(extra);
                continue;
            }
        }
        items.push(a.clone());
    }
    let arr = HeapObj::Array(ArrayData {
        items: Mutex::new(items),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}

fn array_reverse(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().reverse();
            }
        });
        return Ok(Value::Object(idx));
    }
    Ok(Value::Undefined)
}

fn array_sort(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cmp = args.first().cloned();
    if let Some(Value::Object(idx)) = this {
        // Collect items, sort via comparator (default: cast to string, UTF-16 code unit compare).
        let mut items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        match cmp {
            None | Some(Value::Undefined) | Some(Value::Null) => {
                // default: compare by string representation; undefined sorts to end.
                items.sort_by(|a, b| {
                    if a.is_undefined() && b.is_undefined() {
                        return std::cmp::Ordering::Equal;
                    }
                    if a.is_undefined() {
                        return std::cmp::Ordering::Greater;
                    }
                    if b.is_undefined() {
                        return std::cmp::Ordering::Less;
                    }
                    let sa = vm.to_string(a).map(|s| s.to_string()).unwrap_or_default();
                    let sb = vm.to_string(b).map(|s| s.to_string()).unwrap_or_default();
                    sa.cmp(&sb)
                });
            }
            Some(cmp_fn) => {
                let n = items.len();
                // Simple O(n^2) insertion sort to call back into JS comparator.
                for i in 1..n {
                    let mut j = i;
                    while j > 0 {
                        let a = items[j - 1].clone();
                        let b = items[j].clone();
                        let r = vm.call_function(&cmp_fn, &[a.clone(), b.clone()], None)?;
                        // Use the raw f64 sign so a comparator return value in
                        // (0, 1) (e.g. `3.1 - 2.3 == 0.8`) is still treated as
                        // "greater than zero". Casting to i64 truncates 0.8 to
                        // 0, which silently breaks sorts of non-integer
                        // doubles. NaN (from a bad comparator) is treated as 0
                        // (equal) per the spec's "comparison is inconsistent".
                        let ord = vm.to_number(&r)?;
                        if ord > 0.0 {
                            items.swap(j - 1, j);
                            j -= 1;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                *a.items.lock().unwrap() = items;
            }
        });
        return Ok(Value::Object(idx));
    }
    Ok(Value::Undefined)
}

fn array_shift(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        return Ok(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                let mut items = a.items.lock().unwrap();
                if items.is_empty() {
                    Value::Undefined
                } else {
                    items.remove(0)
                }
            } else {
                Value::Undefined
            }
        }));
    }
    Ok(Value::Undefined)
}
fn array_unshift(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                let mut items = a.items.lock().unwrap();
                for (i, v) in args.iter().enumerate() {
                    items.insert(i, v.clone());
                }
            }
        });
        let len = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().len()
            } else {
                0
            }
        });
        return Ok(Value::Number(len as f64));
    }
    Ok(Value::Number(0.0))
}
fn array_splice(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let items_clone = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let len = items_clone.len() as f64;
        let start = match args.first() {
            Some(v) => vm.to_number(v)?,
            None => 0.0,
        };
        let start = if start < 0.0 {
            (len + start).max(0.0) as usize
        } else {
            (start as usize).min(items_clone.len())
        };
        let delete_count = match args.get(1) {
            Some(v) => vm.to_number(v)?,
            None => 0.0,
        };
        let delete_count = if delete_count < 0.0 {
            0
        } else {
            (delete_count as usize).min(items_clone.len() - start)
        };
        let removed: Vec<Value> = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                let mut items = a.items.lock().unwrap();
                let r: Vec<Value> = items.drain(start..start + delete_count).collect();
                for (i, v) in args.iter().skip(2).enumerate() {
                    items.insert(start + i, v.clone());
                }
                r
            } else {
                Vec::new()
            }
        });
        let arr = make_value_array(vm, removed);
        return Ok(arr);
    }
    Ok(Value::Undefined)
}
fn array_last_index_of(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = args.first().unwrap_or(&Value::Undefined).clone();
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len();
        // fromIndex for lastIndexOf: default +Inf (start from end); negative
        // wraps from the end; clamped into [0, len-1].
        let raw = match args.get(1) {
            Some(v) => vm.to_number(v)?,
            None => f64::INFINITY,
        };
        let end = if raw.is_nan() {
            len
        } else if raw.is_infinite() && raw < 0.0 {
            return Ok(Value::Number(-1.0));
        } else {
            let n = raw as i64;
            (if n < 0 { len as i64 + n } else { n }).clamp(0, len as i64) as usize
        };
        for i in (0..end).rev() {
            if vm.strict_eq(&items[i], &target) {
                return Ok(Value::Number(i as f64));
            }
        }
    }
    Ok(Value::Number(-1.0))
}
fn array_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let n = match args.first() {
            Some(v) => vm.to_number(v)?,
            None => 0.0,
        };
        let len = items.len() as isize;
        let idx = if n < 0.0 {
            len + n as isize
        } else {
            n as isize
        };
        if idx >= 0 && idx < len {
            return Ok(items[idx as usize].clone());
        }
    }
    Ok(Value::Undefined)
}
fn array_flat(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let depth = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => 1.0,
    };
    let depth = if depth < 0.0 { 0 } else { depth as usize };
    fn flatten(vm: &mut Vm, items: &[Value], depth: usize, out: &mut Vec<Value>) {
        for v in items {
            let is_arr = match v {
                Value::Object(idx) => vm.heap.with_obj(idx.0, |o| matches!(o, HeapObj::Array(_))),
                _ => false,
            };
            if is_arr && depth > 0 {
                let sub = vm.heap.with_obj(
                    match v {
                        Value::Object(i) => i.0,
                        _ => 0,
                    },
                    |o| {
                        if let HeapObj::Array(a) = o {
                            a.items.lock().unwrap().clone()
                        } else {
                            Vec::new()
                        }
                    },
                );
                flatten(vm, &sub, depth - 1, out);
            } else {
                out.push(v.clone());
            }
        }
    }
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let mut out = Vec::new();
        flatten(vm, &items, depth, &mut out);
        return Ok(make_value_array(vm, out));
    }
    Ok(Value::Undefined)
}
fn array_flat_map(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    // flatMap(fn) = map(fn).flat(1)
    let items = if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    let fn_val = args.first().cloned().unwrap_or(Value::Undefined);
    let mut mapped: Vec<Value> = Vec::new();
    for (i, v) in items.iter().enumerate() {
        let result = vm.call_function(
            &fn_val,
            &[
                v.clone(),
                Value::Number(i as f64),
                this.clone().unwrap_or(Value::Undefined),
            ],
            None,
        )?;
        mapped.push(result);
    }
    let mut out = Vec::new();
    for v in &mapped {
        let is_arr = match v {
            Value::Object(idx) => vm.heap.with_obj(idx.0, |o| matches!(o, HeapObj::Array(_))),
            _ => false,
        };
        if is_arr {
            let sub = vm.heap.with_obj(
                match v {
                    Value::Object(i) => i.0,
                    _ => 0,
                },
                |o| {
                    if let HeapObj::Array(a) = o {
                        a.items.lock().unwrap().clone()
                    } else {
                        Vec::new()
                    }
                },
            );
            out.extend(sub);
        } else {
            out.push(v.clone());
        }
    }
    Ok(make_value_array(vm, out))
}
fn array_copy_within(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let len = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().len()
            } else {
                0
            }
        }) as f64;
        let target = match args.first() {
            Some(v) => vm.to_number(v)?,
            None => 0.0,
        };
        let start = match args.get(1) {
            Some(v) => vm.to_number(v)?,
            None => 0.0,
        };
        let end = match args.get(2) {
            Some(v) => vm.to_number(v)?,
            None => len,
        };
        let to = norm_idx(target, len) as usize;
        let from = norm_idx(start, len) as usize;
        let last = if end < 0.0 {
            (len + end).max(0.0) as usize
        } else {
            (end as usize).min(len as usize)
        };
        if from >= last || to >= len as usize {
            return Ok(Value::Object(idx));
        }
        let count = (last - from).min(len as usize - to);
        let src: Vec<Value> = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap()[from..from + count].to_vec()
            } else {
                Vec::new()
            }
        });
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                let mut items = a.items.lock().unwrap();
                for (i, v) in src.into_iter().enumerate() {
                    items[to + i] = v;
                }
            }
        });
        return Ok(Value::Object(idx));
    }
    Ok(Value::Undefined)
}
fn array_keys(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let len = if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().len()
            } else {
                0
            }
        })
    } else {
        0
    };
    let items: Vec<Value> = (0..len).map(|i| Value::Number(i as f64)).collect();
    Ok(make_value_array(vm, items))
}
fn array_values(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    Ok(make_value_array(vm, items))
}
fn array_entries(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    let pairs: Vec<Value> = items
        .iter()
        .enumerate()
        .map(|(i, v)| make_value_array(vm, vec![Value::Number(i as f64), v.clone()]))
        .collect();
    Ok(make_value_array(vm, pairs))
}

fn array_constructor(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    // `Array(n)` / `new Array(n)` with a single number argument creates a
    // sparse array of length n (filled with holes). Other argument forms
    // create an array of the given elements. `this` (from `new`) is ignored:
    // ES ArrayConstructor always returns a fresh Array exotic object, not the
    // `[[Construct]]`-provided ordinary object.
    let items = if args.len() == 1 {
        if let Some(Value::Number(n)) = args.first() {
            // Validate the length per ArrayCreate: must be a non-negative
            // integer that fits in u32. Negative / fractional / huge values
            // throw RangeError, not an OOM abort.
            if n.is_nan() || *n < 0.0 || n.is_infinite() || n.fract() != 0.0 {
                return Err(Error::range("Invalid array length"));
            }
            if *n >= (1u64 << 32) as f64 {
                return Err(Error::range("Invalid array length"));
            }
            // Avoid attempting an enormous allocation: cap at a sane limit.
            let len = *n as usize;
            if len > 1 << 24 {
                return Err(Error::range("Invalid array length"));
            }
            vec![Value::Undefined; len]
        } else {
            args.to_vec()
        }
    } else {
        args.to_vec()
    };
    let arr = HeapObj::Array(ArrayData {
        items: Mutex::new(items),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}

fn array_find(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        for (i, item) in items.iter().enumerate() {
            let found = vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?;
            if found.is_truthy() {
                return Ok(item.clone());
            }
        }
    }
    Ok(Value::Undefined)
}
fn array_find_index(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        for (i, item) in items.iter().enumerate() {
            let found = vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?;
            if found.is_truthy() {
                return Ok(Value::Number(i as f64));
            }
        }
    }
    Ok(Value::Number(-1.0))
}
fn array_find_last(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        for (i, item) in items.iter().enumerate().rev() {
            let found = vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?;
            if found.is_truthy() {
                return Ok(item.clone());
            }
        }
    }
    Ok(Value::Undefined)
}
fn array_fill(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len() as i64;
        let start = args
            .get(1)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i64)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let end = args
            .get(2)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i64)
                } else {
                    None
                }
            })
            .unwrap_or(len);
        let s = if start < 0 {
            (len + start).max(0) as usize
        } else {
            (start as usize).min(items.len())
        };
        let e = if end < 0 {
            (len + end).max(0) as usize
        } else {
            (end as usize).min(items.len())
        };
        if s < e {
            vm.heap.with_obj(idx.0, |obj| {
                if let HeapObj::Array(a) = obj {
                    let mut items = a.items.lock().unwrap();
                    for i in s..e.min(items.len()) {
                        items[i] = value.clone();
                    }
                }
            });
        }
        return Ok(Value::Object(idx));
    }
    Ok(Value::Undefined)
}
fn array_some(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        for (i, item) in items.iter().enumerate() {
            let found = vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?;
            if found.is_truthy() {
                return Ok(Value::Bool(true));
            }
        }
    }
    Ok(Value::Bool(false))
}
fn array_every(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        for (i, item) in items.iter().enumerate() {
            let ok = vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?;
            if !ok.is_truthy() {
                return Ok(Value::Bool(false));
            }
        }
    }
    Ok(Value::Bool(true))
}

// =========================================================================
// String prototype + constructor
// =========================================================================
fn str_val(vm: &mut Vm, this: &Option<Value>) -> error::Result<String> {
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
fn str_char_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let i = args
        .first()
        .and_then(|v| {
            if let Value::Number(n) = v {
                Some(*n as usize)
            } else {
                None
            }
        })
        .unwrap_or(0);
    // Operate on UTF-16 code units: charAt returns a 1-unit string (a
    // surrogate half for supplementary characters, like real JS).
    match crate::value::utf16_get(&s, i) {
        Some(unit) => Ok(Value::String(Arc::from(
            String::from_utf16_lossy(&[unit]).as_str(),
        ))),
        None => Ok(Value::String(Arc::from(""))),
    }
}
fn str_char_code_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let i = args
        .first()
        .and_then(|v| {
            if let Value::Number(n) = v {
                Some(*n as usize)
            } else {
                None
            }
        })
        .unwrap_or(0);
    Ok(crate::value::utf16_get(&s, i)
        .map(|unit| Value::Number(unit as f64))
        .unwrap_or(Value::Number(f64::NAN)))
}
fn str_code_point_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let i = args
        .first()
        .and_then(|v| {
            if let Value::Number(n) = v {
                Some(*n as usize)
            } else {
                None
            }
        })
        .unwrap_or(0);
    let len = crate::value::utf16_len(&s);
    if i >= len {
        return Ok(Value::Undefined);
    }
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

fn str_concat(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let mut result = s.to_string();
    for a in args {
        result.push_str(&vm.to_string(a)?);
    }
    Ok(Value::String(Arc::from(result.as_str())))
}

fn str_search(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let pattern = args.first().cloned().unwrap_or(Value::Undefined);
    let p = vm.to_string(&pattern)?;
    Ok(crate::value::utf16_index_of(&s, &p, 0)
        .map(|i| Value::Number(i as f64))
        .unwrap_or(Value::Number(-1.0)))
}

fn string_raw(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
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

fn string_from_code_point(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
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

fn str_index_of(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
fn str_slice(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
fn str_to_upper(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Arc::from(
        str_val(vm, &this)?.to_uppercase().as_str(),
    )))
}
fn str_to_lower(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Arc::from(
        str_val(vm, &this)?.to_lowercase().as_str(),
    )))
}
fn str_trim(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Arc::from(str_val(vm, &this)?.trim())))
}
fn str_split(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let limit = match args.get(1) {
        Some(Value::Undefined) | None => usize::MAX,
        Some(v) => vm.to_number(v).map(|n| n as usize).unwrap_or(usize::MAX),
    };
    // If the separator is a RegExp, split on regex matches.
    if let Some(Value::Object(idx)) = args.first() {
        let source = vm.heap.with_obj(idx.0, |o| {
            o.props()
                .lock()
                .unwrap()
                .get(&crate::value::PropertyKey::from("source"))
                .map(|d| d.value.clone())
        });
        if let Some(Value::String(source)) = source {
            let re =
                Regex::new(&source).map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
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
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}
fn str_replace(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
        let source = vm.heap.with_obj(idx.0, |o| {
            o.props()
                .lock()
                .unwrap()
                .get(&crate::value::PropertyKey::from("source"))
                .map(|d| d.value.clone())
        });
        if let Some(Value::String(source)) = source {
            let global = vm.heap.with_obj(idx.0, |o| {
                o.props()
                    .lock()
                    .unwrap()
                    .get(&crate::value::PropertyKey::from("global"))
                    .map(|d| d.value.clone())
            }) == Some(Value::Bool(true));
            let re =
                Regex::new(&source).map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
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
fn str_last_index_of(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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

fn str_includes(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
fn str_starts_with(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(
        str_val(vm, &this)?.starts_with(
            args.first()
                .map(crate::value::value_to_debug_string)
                .unwrap_or_default()
                .as_str(),
        ),
    ))
}
fn str_ends_with(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(
        str_val(vm, &this)?.ends_with(
            args.first()
                .map(crate::value::value_to_debug_string)
                .unwrap_or_default()
                .as_str(),
        ),
    ))
}
fn str_repeat(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let n = args
        .first()
        .and_then(|v| {
            if let Value::Number(n) = v {
                Some(*n as usize)
            } else {
                None
            }
        })
        .unwrap_or(0);
    Ok(Value::String(Arc::from(
        str_val(vm, &this)?.repeat(n).as_str(),
    )))
}

fn str_match(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    match args.first() {
        Some(Value::Object(idx)) => {
            let (source, flags) = vm.heap.with_obj(idx.0, |o| {
                let p = o.props().lock().unwrap();
                let src = p.get(&PropertyKey::from("source")).map(|d| d.value.clone());
                let flg = p
                    .get(&PropertyKey::from("flags"))
                    .map(|d| d.value.clone())
                    .unwrap_or(Value::Undefined);
                (src, flg)
            });
            if let Some(Value::String(source)) = source {
                let re = Regex::new(&source)
                    .map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
                let global = matches!(&flags, Value::String(ref f) if f.contains('g'));
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
fn array_find_last_index(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let fn_val = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().unwrap().clone()
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

fn str_pad_start(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let target = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    } as usize;
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
fn str_pad_end(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let target = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    } as usize;
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
fn str_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
fn str_trim_start(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    Ok(Value::String(Arc::from(s.trim_start())))
}
fn str_trim_end(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    Ok(Value::String(Arc::from(s.trim_end())))
}
fn str_replace_all(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
fn str_substring(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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

fn str_from_char_code(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
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
fn string_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
fn number_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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

fn number_is_integer(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    match args.first() {
        Some(Value::Number(n)) if n.is_finite() && n.fract() == 0.0 => Ok(Value::Bool(true)),
        _ => Ok(Value::Bool(false)),
    }
}
fn number_is_finite(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    match args.first() {
        Some(Value::Number(n)) if n.is_finite() => Ok(Value::Bool(true)),
        _ => Ok(Value::Bool(false)),
    }
}
fn number_is_nan(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    match args.first() {
        Some(Value::Number(n)) if n.is_nan() => Ok(Value::Bool(true)),
        _ => Ok(Value::Bool(false)),
    }
}
fn number_is_safe_integer(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    match args.first() {
        Some(Value::Number(n))
            if n.is_finite() && n.fract() == 0.0 && n.abs() <= 9007199254740991.0 =>
        {
            Ok(Value::Bool(true))
        }
        _ => Ok(Value::Bool(false)),
    }
}
fn number_parse_int(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    global_parse_int(vm, args, None)
}
fn number_parse_float(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    global_parse_float(vm, args, None)
}
fn num_to_fixed(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
    let digits = match args.first() {
        Some(v) => vm.to_number(v)? as usize,
        None => 0,
    };
    Ok(Value::String(Arc::from(format!("{:.*}", digits, n))))
}
fn num_to_precision(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
            let p = vm.to_number(v)? as usize;
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
                    format!("{}e{}", mantissa, exp)
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

fn num_to_exponential(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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

fn num_proto_to_string(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
    if n.fract() == 0.0 {
        let i = n as i64;
        if i >= 0 {
            return Ok(Value::String(Arc::from(
                format_i64_radix(i, radix).as_str(),
            )));
        }
    }
    Ok(Value::String(Arc::from(
        crate::value::num_to_string(n).as_str(),
    )))
}
fn format_i64_radix(n: i64, radix: u32) -> String {
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

fn boolean_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(_)) = &this {
        let prim = Value::Bool(args.first().unwrap_or(&Value::Undefined).is_truthy());
        vm.set_primitive(this.as_ref().unwrap(), prim);
        return Ok(this.unwrap());
    }
    Ok(Value::Bool(
        args.first().unwrap_or(&Value::Undefined).is_truthy(),
    ))
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
                .unwrap()
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
                .unwrap()
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
            .unwrap()
            .insert(PropertyKey::from("raw"), data_prop(Value::Object(raw_fn)));
    });
    let fcp_fn = vm.new_native_function("fromCodePoint", string_from_code_point, 1);
    vm.heap.with_obj(str_ctor.0, |obj| {
        obj.props().lock().unwrap().insert(
            PropertyKey::from("fromCodePoint"),
            data_prop(Value::Object(fcp_fn)),
        );
    });
    // String statics
    let from_char_code_fn = vm.new_native_function("fromCharCode", str_from_char_code, 1);
    vm.heap.with_obj(str_ctor.0, |obj| {
        obj.props().lock().unwrap().insert(
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
                    .unwrap()
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
                    *f.prototype.lock().unwrap() = Some(bproto.clone());
                }
            });
            let to_str = vm.new_native_function("toString", bigint_to_string, 0);
            if let Value::Object(pi) = bproto {
                vm.heap.with_obj(pi.0, |obj| {
                    obj.props().lock().unwrap().insert(
                        crate::value::PropertyKey::from("toString"),
                        crate::value::PropertyDescriptor::data(Value::Object(to_str)),
                    );
                    obj.props().lock().unwrap().insert(
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
        obj.props().lock().unwrap().insert(
            PropertyKey::from("resolve"),
            data_prop(Value::Object(resolve_static)),
        );
        obj.props().lock().unwrap().insert(
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
            obj.props.lock().unwrap().insert(
                PropertyKey::from("__regex_proto__"),
                data_prop(Value::Bool(true)),
            );
        }
    });
    // Store regex_proto on the constructor so regexp_constructor can use it.
    vm.heap.with_obj(regex_ctor.0, |o| {
        if let HeapObj::Function(f) = o {
            f.props.lock().unwrap().insert(
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
                .unwrap()
                .insert(PropertyKey::from("next"), data_prop(Value::Object(next_fn)));
            o.props().lock().unwrap().insert(
                PropertyKey::from("return"),
                data_prop(Value::Object(return_fn)),
            );
            o.props().lock().unwrap().insert(
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
        obj.props().lock().unwrap().insert(
            PropertyKey::from("prototype"),
            data_prop(Value::Object(function_proto_idx)),
        );
    });
    // The function prototype's `constructor` is the Function constructor.
    vm.heap.with_obj(function_proto_idx.0, |obj| {
        obj.props().lock().unwrap().insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(function_ctor_idx)),
        );
    });
    setup_collections(vm);
}

// =========================================================================
// Map
// =========================================================================
fn map_set(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let val = args.get(1).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                let mut entries = m.entries.lock().unwrap();
                if let Some(slot) = entries.iter_mut().find(|(k, _)| k == &key) {
                    slot.1 = val;
                } else {
                    entries.push((key, val));
                }
            }
        });
    }
    Ok(this.unwrap_or(Value::Undefined))
}
fn map_get(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|(k, _)| k == &key)
                    .map(|(_, v)| v.clone())
                    .unwrap_or(Value::Undefined)
            } else {
                Value::Undefined
            }
        }));
    }
    Ok(Value::Undefined)
}
fn map_has(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries.lock().unwrap().iter().any(|(k, _)| k == &key)
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}
fn map_delete(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                let mut entries = m.entries.lock().unwrap();
                let len = entries.len();
                entries.retain(|(k, _)| k != &key);
                entries.len() != len
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}

// --- WeakMap / WeakSet (true weak-reference semantics) ---

fn weakmap_constructor(vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    // The WeakMap prototype (with get/set/has/delete) is the constructor's
    // own `.prototype` property. `construct` passes a fresh Object whose
    // [[Prototype]] is that prototype as `this`; copy it so the returned
    // WeakMap object inherits the methods.
    let proto = match _this {
        Some(Value::Object(idx)) => vm
            .heap
            .with_obj(idx.0, |o| o.proto().lock().unwrap().clone()),
        _ => Some(vm.object_proto.clone()),
    };
    let obj_idx = vm
        .heap
        .allocate(HeapObj::WeakMap(crate::value::WeakMapData {
            entries: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(proto),
        }));
    Ok(Value::Object(GcIdx(obj_idx)))
}

fn weakmap_set(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let val = args.get(1).cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => {
            return Err(Error::type_err(
                "Invalid value used as weak map key".to_string(),
            ))
        }
    };
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakMap(wm) = obj {
                let mut entries = wm.entries.lock().unwrap();
                if let Some(slot) = entries.iter_mut().find(|(k, _)| *k == key_idx) {
                    slot.1 = val;
                } else {
                    entries.push((key_idx, val));
                }
            }
        });
    }
    Ok(this.unwrap_or(Value::Undefined))
}

fn weakmap_get(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => return Ok(Value::Undefined),
    };
    if let Some(Value::Object(idx)) = this {
        return Ok(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakMap(wm) = obj {
                wm.entries
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|(k, _)| *k == key_idx)
                    .map(|(_, v)| v.clone())
                    .unwrap_or(Value::Undefined)
            } else {
                Value::Undefined
            }
        }));
    }
    Ok(Value::Undefined)
}

fn weakmap_has(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => return Ok(Value::Bool(false)),
    };
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakMap(wm) = obj {
                wm.entries
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|(k, _)| *k == key_idx)
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}

fn weakmap_delete(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => return Ok(Value::Bool(false)),
    };
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakMap(wm) = obj {
                let mut entries = wm.entries.lock().unwrap();
                let len = entries.len();
                entries.retain(|(k, _)| *k != key_idx);
                entries.len() != len
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}

fn weakset_constructor(vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let proto = match _this {
        Some(Value::Object(idx)) => vm
            .heap
            .with_obj(idx.0, |o| o.proto().lock().unwrap().clone()),
        _ => Some(vm.object_proto.clone()),
    };
    let obj_idx = vm
        .heap
        .allocate(HeapObj::WeakSet(crate::value::WeakSetData {
            items: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(proto),
        }));
    Ok(Value::Object(GcIdx(obj_idx)))
}

fn weakset_add(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => {
            return Err(Error::type_err(
                "Invalid value used in weak set".to_string(),
            ))
        }
    };
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakSet(ws) = obj {
                let mut items = ws.items.lock().unwrap();
                if !items.contains(&key_idx) {
                    items.push(key_idx);
                }
            }
        });
    }
    Ok(this.unwrap_or(Value::Undefined))
}

fn weakset_has(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => return Ok(Value::Bool(false)),
    };
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakSet(ws) = obj {
                ws.items.lock().unwrap().contains(&key_idx)
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}

fn weakset_delete(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let key_idx = match &key {
        Value::Object(i) => i.0,
        _ => return Ok(Value::Bool(false)),
    };
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::WeakSet(ws) = obj {
                let mut items = ws.items.lock().unwrap();
                let len = items.len();
                items.retain(|k| *k != key_idx);
                items.len() != len
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}
fn map_clear(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries.lock().unwrap().clear();
            }
        });
    }
    Ok(Value::Undefined)
}
fn map_size(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Number(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries.lock().unwrap().len()
            } else {
                0
            }
        }) as f64));
    }
    Ok(Value::Number(0.0))
}
/// Collect Map entries as [key, value] arrays.
fn map_entries_list(vm: &mut Vm, this: &Option<Value>) -> Vec<Value> {
    if let Some(Value::Object(idx)) = this {
        let pairs: Vec<(Value, Value)> = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        pairs
            .into_iter()
            .map(|(k, v)| make_value_array(vm, vec![k, v]))
            .collect()
    } else {
        Vec::new()
    }
}
fn map_entries(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let pairs = map_entries_list(vm, &this);
    Ok(make_value_array(vm, pairs))
}
fn map_keys(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let keys: Vec<Value> = if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|(k, _)| k.clone())
                    .collect()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    Ok(make_value_array(vm, keys))
}
fn map_values(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let vals: Vec<Value> = if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|(_, v)| v.clone())
                    .collect()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    Ok(make_value_array(vm, vals))
}
fn map_for_each(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    let this_arg = args.get(1).cloned();
    if let Some(Value::Object(idx)) = this {
        let pairs: Vec<(Value, Value)> = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        });
        for (k, v) in &pairs {
            vm.call_function(
                &cb,
                &[
                    v.clone(),
                    k.clone(),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                this_arg.clone(),
            )?;
        }
    }
    Ok(Value::Undefined)
}
fn map_constructor(vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let obj_idx = vm.heap.allocate(HeapObj::Map(MapData {
        entries: Mutex::new(Vec::new()),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.map_proto.clone())),
    }));
    // Initialize from an optional iterable of [key, value] pairs.
    if let Some(iterable) = _args.first() {
        if !iterable.is_undefined() && !iterable.is_null() {
            let it = vm.make_iterator(iterable)?;
            loop {
                let (pair, done) = vm.iterator_next(&it)?;
                if done {
                    break;
                }
                let (k, v) = if let Value::Object(pi) = &pair {
                    vm.heap.with_obj(pi.0, |o| {
                        if let HeapObj::Array(a) = o {
                            let it2 = a.items.lock().unwrap();
                            (
                                it2.first().cloned().unwrap_or(Value::Undefined),
                                it2.get(1).cloned().unwrap_or(Value::Undefined),
                            )
                        } else {
                            (Value::Undefined, Value::Undefined)
                        }
                    })
                } else {
                    (Value::Undefined, Value::Undefined)
                };
                vm.heap.with_obj(obj_idx, |o| {
                    if let HeapObj::Map(m) = o {
                        m.entries.lock().unwrap().push((k, v));
                    }
                });
            }
        }
    }
    Ok(Value::Object(GcIdx(obj_idx)))
}

// =========================================================================
// Set
// =========================================================================
fn set_add(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj {
                let mut items = s.items.lock().unwrap();
                if !items.iter().any(|i| i == &val) {
                    items.push(val);
                }
            }
        });
    }
    Ok(this.unwrap_or(Value::Undefined))
}
fn set_has(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj {
                s.items.lock().unwrap().iter().any(|i| i == &val)
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}
fn set_delete(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj {
                let mut items = s.items.lock().unwrap();
                let len = items.len();
                items.retain(|i| i != &val);
                items.len() != len
            } else {
                false
            }
        })));
    }
    Ok(Value::Bool(false))
}
fn set_size(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Number(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj {
                s.items.lock().unwrap().len()
            } else {
                0
            }
        }) as f64));
    }
    Ok(Value::Number(0.0))
}
fn set_values_list(vm: &mut Vm, this: &Option<Value>) -> Vec<Value> {
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj {
                s.items.lock().unwrap().clone()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    }
}
fn set_entries(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let vals = set_values_list(vm, &this);
    let mut pairs: Vec<Value> = Vec::new();
    for v in vals {
        pairs.push(make_value_array(vm, vec![v.clone(), v]));
    }
    Ok(make_value_array(vm, pairs))
}
fn set_keys(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let vals = set_values_list(vm, &this);
    Ok(make_value_array(vm, vals))
}
fn set_values(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let vals = set_values_list(vm, &this);
    Ok(make_value_array(vm, vals))
}
fn set_for_each(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    let this_arg = args.get(1).cloned();
    let vals = set_values_list(vm, &this);
    for v in &vals {
        vm.call_function(
            &cb,
            &[
                v.clone(),
                v.clone(),
                this.clone().unwrap_or(Value::Undefined),
            ],
            this_arg.clone(),
        )?;
    }
    Ok(Value::Undefined)
}
fn set_constructor(vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let obj_idx = vm.heap.allocate(HeapObj::Set(SetData {
        items: Mutex::new(Vec::new()),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.set_proto.clone())),
    }));
    // Initialize from an optional iterable.
    if let Some(iterable) = _args.first() {
        if !iterable.is_undefined() && !iterable.is_null() {
            let it = vm.make_iterator(iterable)?;
            loop {
                let (v, done) = vm.iterator_next(&it)?;
                if done {
                    break;
                }
                vm.heap.with_obj(obj_idx, |o| {
                    if let HeapObj::Set(s) = o {
                        s.items.lock().unwrap().push(v);
                    }
                });
            }
        }
    }
    Ok(Value::Object(GcIdx(obj_idx)))
}

// =========================================================================
// Symbol
// =========================================================================
fn symbol_constructor(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let _desc = args.first().cloned().unwrap_or(Value::Undefined);
    let id = vm.next_symbol_id;
    vm.next_symbol_id += 1;
    Ok(Value::Symbol(id))
}
fn symbol_for(vm: &mut Vm, _args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let id = vm.next_symbol_id;
    vm.next_symbol_id += 1;
    Ok(Value::Symbol(id))
}
fn symbol_to_string(_vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    // RuJa's Symbol is `Value::Symbol(u32)` with no stored description, so
    // we return the no-description form "Symbol()".
    Ok(Value::String(Arc::from("Symbol()")))
}

// =========================================================================
// Extended setup 2: Map/Set/Symbol
// =========================================================================

// =========================================================================
// Promise
// =========================================================================
fn promise_constructor(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let executor = args.first().cloned().unwrap_or(Value::Undefined);
    // create the promise object
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: std::sync::Mutex::new(crate::value::PromiseStatus::Pending),
            result: Mutex::new(Value::Undefined),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.promise_proto.clone())),
        }));
    let p_val = Value::Object(GcIdx(p_idx));
    // create resolve/reject native functions bound via `this` = promise
    let resolve_target = vm.new_native_function("resolve", promise_resolve, 1);
    let reject_target = vm.new_native_function("reject", promise_reject, 1);
    // Wrap as bound functions with  = the promise, so resolve/reject know
    // which promise to settle.
    let resolve_fn = vm
        .heap
        .allocate(HeapObj::Function(crate::value::FunctionData {
            name: Some(Arc::from("resolve")),
            kind: crate::value::FunctionKind::Bound {
                target: resolve_target,
                this_val: p_val.clone(),
                bound_args: Vec::new(),
            },
            closure: vm.global,
            prototype: Mutex::new(None),
            proto: Mutex::new(match vm.function_proto {
                Value::Object(_) => Some(vm.function_proto.clone()),
                _ => None,
            }),
            props: Mutex::new(IndexMap::new()),
        }));
    let reject_fn = vm
        .heap
        .allocate(HeapObj::Function(crate::value::FunctionData {
            name: Some(Arc::from("reject")),
            kind: crate::value::FunctionKind::Bound {
                target: reject_target,
                this_val: p_val.clone(),
                bound_args: Vec::new(),
            },
            closure: vm.global,
            prototype: Mutex::new(None),
            proto: Mutex::new(match vm.function_proto {
                Value::Object(_) => Some(vm.function_proto.clone()),
                _ => None,
            }),
            props: Mutex::new(IndexMap::new()),
        }));
    match vm.call_function(
        &executor,
        &[
            Value::Object(GcIdx(resolve_fn)),
            Value::Object(GcIdx(reject_fn)),
        ],
        Some(p_val.clone()),
    ) {
        Ok(_) => {}
        Err(e) => {
            // executor threw: reject the promise with the thrown value
            let reason: Value = e
                .thrown_value
                .clone()
                .unwrap_or_else(|| Value::String(Arc::from(e.message.as_str())));
            vm.promise_reject(p_idx, reason);
        }
    }
    Ok(p_val)
}

fn promise_resolve(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let p_idx = match &this {
        Some(Value::Object(idx)) => idx.0,
        _ => return Ok(Value::Undefined),
    };
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    vm.promise_resolve(p_idx, value);
    Ok(Value::Undefined)
}
fn promise_reject(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let p_idx = match &this {
        Some(Value::Object(idx)) => idx.0,
        _ => return Ok(Value::Undefined),
    };
    let reason = args.first().cloned().unwrap_or(Value::Undefined);
    vm.promise_reject(p_idx, reason);
    Ok(Value::Undefined)
}

/// `Promise.resolve(v)`: returns a promise resolved with `v`. If `v` is already
/// a promise, it is returned as-is (simplified adoption).
fn promise_static_resolve(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = &value {
        let is_promise = vm
            .heap
            .with_obj(idx.0, |o| matches!(o, HeapObj::Promise(_)));
        if is_promise {
            return Ok(value);
        }
    }
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: std::sync::Mutex::new(crate::value::PromiseStatus::Fulfilled),
            result: Mutex::new(value),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.promise_proto.clone())),
        }));
    Ok(Value::Object(GcIdx(p_idx)))
}

/// `Promise.reject(r)`: returns a promise rejected with `r`.
fn promise_static_reject(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let reason = args.first().cloned().unwrap_or(Value::Undefined);
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: std::sync::Mutex::new(crate::value::PromiseStatus::Rejected),
            result: Mutex::new(reason),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.promise_proto.clone())),
        }));
    Ok(Value::Object(GcIdx(p_idx)))
}

fn promise_then(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let on_fulfilled = args.first().cloned().unwrap_or(Value::Undefined);
    let on_rejected = args.get(1).cloned().unwrap_or(Value::Undefined);
    let p_idx = match &this {
        Some(Value::Object(idx)) => idx.0,
        _ => return Err(Error::type_err("then called on non-promise".to_string())),
    };
    // Create a derived promise that settles with the handler's result.
    let derived = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: std::sync::Mutex::new(crate::value::PromiseStatus::Pending),
            result: Mutex::new(Value::Undefined),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.promise_proto.clone())),
        }));
    let (state, _result) = vm.heap.with_obj(p_idx, |o| {
        if let HeapObj::Promise(p) = o {
            (*p.state.lock().unwrap(), p.result.lock().unwrap().clone())
        } else {
            (crate::value::PromiseStatus::Fulfilled, Value::Undefined)
        }
    });
    let handler = crate::value::PromiseHandler {
        on_fulfilled: on_fulfilled.clone(),
        on_rejected: on_rejected.clone(),
        derived: Some(GcIdx(derived)),
    };
    match state {
        crate::value::PromiseStatus::Pending => {
            vm.heap.with_obj(p_idx, |o| {
                if let HeapObj::Promise(p) = o {
                    p.handlers.lock().unwrap().push(handler);
                }
            });
        }
        _ => {
            // already settled: schedule immediately, passing derived for chaining
            vm.microtask_queue.push_back(crate::vm::Microtask::Then {
                promise: GcIdx(p_idx),
                on_fulfilled,
                on_rejected,
                derived: Some(GcIdx(derived)),
            });
        }
    }
    Ok(Value::Object(GcIdx(derived)))
}

fn promise_catch(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    // p.catch(r) === p.then(undefined, r)
    let on_rejected = args.first().cloned().unwrap_or(Value::Undefined);
    promise_then(vm, &[Value::Undefined, on_rejected], this)
}

// =========================================================================
// RegExp
// =========================================================================
fn regexp_constructor(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
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
                        .unwrap()
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
            *obj.props.lock().unwrap() = props;
        }
    });
    Ok(Value::Object(GcIdx(obj_idx)))
}

fn regexp_test(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let source = read_regexp_source(vm, &this)?;
    let input = match args.first() {
        Some(Value::String(s)) => s.to_string(),
        Some(v) => vm.to_string(v)?.to_string(),
        None => String::new(),
    };
    let re = Regex::new(&source).map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
    Ok(Value::Bool(re.is_match(&input)))
}

fn regexp_exec(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let source = read_regexp_source(vm, &this)?;
    let input = match args.first() {
        Some(Value::String(s)) => s.to_string(),
        Some(v) => vm.to_string(v)?.to_string(),
        None => String::new(),
    };
    let re = Regex::new(&source).map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
    let flags = read_regexp_flags(vm, &this).unwrap_or_default();
    let global = flags.contains('g');
    let sticky = flags.contains('y');
    // Read lastIndex (a number property; default 0).
    let last_idx: f64 = match &this {
        Some(Value::Object(idx)) => vm.heap.with_obj(idx.0, |o| {
            o.props()
                .lock()
                .unwrap()
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
    if start > input.len() {
        if let Some(Value::Object(idx)) = &this {
            vm.heap.with_obj(idx.0, |o| {
                if let HeapObj::Object(obj) = o {
                    obj.props.lock().unwrap().insert(
                        PropertyKey::from("lastIndex"),
                        data_prop(Value::Number(0.0)),
                    );
                }
            });
        }
        return Ok(Value::Null);
    }
    let region = &input[start..];
    // For sticky, match must start exactly at `start`; for global, find from start.
    let m = if sticky {
        re.captures_at(region, 0)
            .filter(|c| c.get(0).map(|mch| mch.start() == 0).unwrap_or(false))
    } else {
        re.captures(region)
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
                let match_end = start + caps.get(0).map(|mch| mch.end()).unwrap_or(0);
                if let Some(Value::Object(idx)) = &this {
                    vm.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Object(obj) = o {
                            obj.props.lock().unwrap().insert(
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
                            obj.props.lock().unwrap().insert(
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

fn read_regexp_source(vm: &mut Vm, this: &Option<Value>) -> error::Result<String> {
    read_regexp_field(vm, this, "source")
}

/// Read the `flags` string of a RegExp object.
fn read_regexp_flags(vm: &mut Vm, this: &Option<Value>) -> error::Result<String> {
    read_regexp_field(vm, this, "flags")
}

/// Read a string field (`source`/`flags`/`lastIndex`) from a RegExp object.
fn read_regexp_field(vm: &mut Vm, this: &Option<Value>, field: &str) -> error::Result<String> {
    match this {
        Some(Value::Object(idx)) => {
            let s = vm.heap.with_obj(idx.0, |o| {
                o.props()
                    .lock()
                    .unwrap()
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

fn generator_next(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
                let state = g.state.lock().unwrap();
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
                .unwrap()
                .insert(PropertyKey::from("value"), data_prop(value));
            obj.props
                .lock()
                .unwrap()
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
fn gen_result(vm: &mut Vm, value: Value, done: bool, is_async_gen: bool) -> error::Result<Value> {
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
                .unwrap()
                .insert(PropertyKey::from("value"), data_prop(value));
            obj.props
                .lock()
                .unwrap()
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
fn generator_return(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
fn generator_throw(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
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
            o.props().lock().unwrap().insert(
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
            o.props().lock().unwrap().insert(
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
            obj.props().lock().unwrap().insert(
                PropertyKey::from("for"),
                data_prop(Value::Object(sym_for_idx)),
            );
            obj.props().lock().unwrap().insert(
                PropertyKey::from("iterator"),
                data_prop(Value::Symbol(vm.well_known_symbols.iterator)),
            );
            obj.props().lock().unwrap().insert(
                PropertyKey::from("asyncIterator"),
                data_prop(Value::Symbol(vm.well_known_symbols.async_iterator)),
            );
            obj.props().lock().unwrap().insert(
                PropertyKey::from("toPrimitive"),
                data_prop(Value::Symbol(vm.well_known_symbols.to_primitive)),
            );
            obj.props().lock().unwrap().insert(
                PropertyKey::from("hasInstance"),
                data_prop(Value::Symbol(vm.well_known_symbols.has_instance)),
            );
            obj.props().lock().unwrap().insert(
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

fn make_builtin_constructor_with(
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
        obj.props().lock().unwrap().insert(
            PropertyKey::from("prototype"),
            data_prop(Value::Object(proto_idx)),
        );
    });
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props().lock().unwrap().insert(
            PropertyKey::from("constructor"),
            data_prop(Value::Object(ctor_idx)),
        );
    });
    (ctor_idx, proto_idx)
}

// =========================================================================
// Function.prototype: call / apply / bind
// =========================================================================

/// `Function.prototype.call(thisArg, ...args)`: invoke `this` (a function)
/// with an explicit `this` binding and a list of arguments.
/// `Function.prototype.toString`: return a spec-ish string representation.
/// For native functions: `function name() { [native code] }`. For interpreted
/// functions, the source is not retained, so we emit `function name() { ... }`.
fn function_to_string(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let f = match this {
        Some(v) => v,
        None => return Ok(Value::String(Arc::from("function () { [native code] }"))),
    };
    if let Value::Object(idx) = &f {
        let (name, is_native) = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Function(fun) = o {
                let n = fun.name.as_ref().map(|s| s.to_string()).unwrap_or_default();
                let native = matches!(fun.kind, crate::value::FunctionKind::Native { .. });
                (n, native)
            } else {
                (String::new(), true)
            }
        });
        let body = if is_native { "[native code]" } else { "..." };
        return Ok(Value::String(Arc::from(
            format!("function {}() {{ {} }}", name, body).as_str(),
        )));
    }
    Ok(Value::String(Arc::from("function () { [native code] }")))
}

fn function_call(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = match this {
        Some(t) => t,
        None => return Err(error::Error::type_err("undefined is not a function")),
    };
    if !is_callable(&target, &vm.heap) {
        return Err(error::Error::type_err(format!(
            "{} is not a function",
            target.type_of()
        )));
    }
    let this_arg = args.first().cloned().unwrap_or(Value::Undefined);
    let call_args: &[Value] = if args.len() > 1 { &args[1..] } else { &[][..] };
    vm.call_function(&target, call_args, Some(this_arg))
}

/// `Function.prototype.apply(thisArg, [argsArray])`: invoke `this` (a
/// function) with an explicit `this` binding and an array-like of arguments.
fn function_apply(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = match this {
        Some(t) => t,
        None => return Err(error::Error::type_err("undefined is not a function")),
    };
    if !is_callable(&target, &vm.heap) {
        return Err(error::Error::type_err(format!(
            "{} is not a function",
            target.type_of()
        )));
    }
    let this_arg = args.first().cloned().unwrap_or(Value::Undefined);
    let arr_args: Vec<Value> = match args.get(1) {
        Some(Value::Undefined) | Some(Value::Null) => Vec::new(),
        Some(Value::Object(idx)) => vm.heap.with_obj(idx.0, |obj| match obj {
            HeapObj::Array(a) => a.items.lock().unwrap().clone(),
            _ => {
                // Array-like fallback: read .length and integer-indexed props.
                let len = obj
                    .props()
                    .lock()
                    .unwrap()
                    .get(&PropertyKey::from("length"))
                    .and_then(|d| {
                        if let Value::Number(n) = d.value {
                            Some(n as usize)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                (0..len)
                    .map(|i| {
                        obj.props()
                            .lock()
                            .unwrap()
                            .get(&PropertyKey::from(i.to_string().as_str()))
                            .map(|d| d.value.clone())
                            .unwrap_or(Value::Undefined)
                    })
                    .collect()
            }
        }),
        _ => Vec::new(),
    };
    vm.call_function(&target, &arr_args, Some(this_arg))
}

/// `Function.prototype.bind(thisArg, ...args)`: create a new function with a
/// fixed `this` binding and leading arguments.
fn function_bind(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = match this {
        Some(t) => t,
        None => return Err(error::Error::type_err("undefined is not a function")),
    };
    if !is_callable(&target, &vm.heap) {
        return Err(error::Error::type_err(format!(
            "{} is not a function",
            target.type_of()
        )));
    }
    let this_arg = args.first().cloned().unwrap_or(Value::Undefined);
    let bound_args: Vec<Value> = if args.len() > 1 {
        args[1..].to_vec()
    } else {
        Vec::new()
    };
    let target_idx = match &target {
        Value::Object(i) => *i,
        _ => return Err(error::Error::type_err("not a function")),
    };
    let bound = crate::value::FunctionData {
        name: Some(Arc::from("bound")),
        kind: crate::value::FunctionKind::Bound {
            target: target_idx,
            this_val: this_arg,
            bound_args,
        },
        closure: vm.global,
        prototype: Mutex::new(None),
        proto: Mutex::new(match vm.function_proto {
            Value::Object(_) => Some(vm.function_proto.clone()),
            _ => None,
        }),
        props: Mutex::new(IndexMap::new()),
    };
    let fidx = vm.heap.allocate(HeapObj::Function(bound));
    Ok(Value::Object(GcIdx(fidx)))
}

/// `Function.prototype` itself is a callable no-op function (per spec:
/// "an empty function"). Invoking it returns `undefined`.
fn function_proto_noop(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::Undefined)
}
