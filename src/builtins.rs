//! Built-in objects and globals for the RuJa VM (v2.0).
//!
//! All built-in constructors, prototypes, and global functions are registered
//! here. Native functions follow the `NativeFn` signature used by the VM.

use crate::environment as env;
use crate::error::{self, Error};
use crate::gc::Heap;
use crate::value::{
    ArrayData, BindingKind, FunctionData, FunctionKind, GcIdx, HeapObj, MapData, ObjectData,
    PromiseData, PromiseHandler, PromiseStatus, PropertyDescriptor, SetData, Value,
};
use crate::vm::{NativeFn, Vm};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

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

fn read_only(value: Value) -> PropertyDescriptor {
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

fn method_prop(vm: &mut Vm, name: &str, func: NativeFn, length: usize) -> (Rc<str>, Value) {
    let idx = vm.new_native_function(name, func, length);
    (Rc::from(name), Value::Object(idx))
}

fn bound_method(vm: &mut Vm, name: &str, func: NativeFn, length: usize) -> (Rc<str>, Value) {
    method_prop(vm, name, func, length)
}

fn new_plain_object(heap: &Heap, proto: Option<Value>) -> GcIdx {
    let obj = HeapObj::Object(ObjectData {
        props: RefCell::new(HashMap::new()),
        proto: RefCell::new(proto),
        extensible: Cell::new(true),
        class_name: Some(Rc::from("Object")),
    });
    GcIdx(heap.allocate(obj))
}

fn new_object_with_props(heap: &Heap, proto: Option<Value>, props: Vec<(&str, Value)>) -> GcIdx {
    let mut map: HashMap<Rc<str>, PropertyDescriptor> = HashMap::new();
    for (k, v) in props {
        map.insert(Rc::from(k), data_prop(v));
    }
    let obj = HeapObj::Object(ObjectData {
        props: RefCell::new(map),
        proto: RefCell::new(proto),
        extensible: Cell::new(true),
        class_name: Some(Rc::from("Object")),
    });
    GcIdx(heap.allocate(obj))
}

fn install_methods(vm: &mut Vm, proto: &Value, methods: &[(Rc<str>, Value)]) {
    if let Value::Object(idx) = proto {
        vm.heap.with_obj(idx.0, |obj| {
            let props = obj.props();
            for (name, func) in methods {
                props.borrow_mut().insert(name.clone(), data_prop(func.clone()));
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

fn object_to_string(vm: &mut Vm, this: Option<Value>, class_hint: Option<&str>) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    if this.is_nullish() {
        return Ok(Value::String(Rc::from("[object Null]")));
    }
    if let Value::String(_) = &this {
        return Ok(Value::String(Rc::from("[object String]")));
    }
    if let Value::Number(_) = &this {
        return Ok(Value::String(Rc::from("[object Number]")));
    }
    if let Value::Bool(_) = &this {
        return Ok(Value::String(Rc::from("[object Boolean]")));
    }
    if let Value::Symbol(_) = &this {
        return Ok(Value::String(Rc::from("[object Symbol]")));
    }
    if let Value::Object(idx) = &this {
        let class = if let Some(hint) = class_hint {
            hint.to_string()
        } else {
            vm.heap.with_obj(idx.0, |obj| {
                let name = obj.class_name();
                if name == "Object" {
                    // check constructor name via prototype
                    if let Some(proto) = obj.proto().borrow().as_ref() {
                        if let Value::Object(pidx) = proto {
                            let constructor = vm.heap.with_obj(pidx.0, |p| {
                                p.props().borrow().get("constructor").map(|d| d.value.clone())
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
                }
                name.to_string()
            })
        };
        return Ok(Value::String(Rc::from(format!("[object {}]", class).as_str())));
    }
    Ok(Value::String(Rc::from("[object Object]")))
}

// ---------------------------------------------------------------------------
// Built-in builders
// ---------------------------------------------------------------------------

fn make_builtin_constructor(vm: &mut Vm, name: &str, methods: &[(&str, NativeFn, usize)]) -> (GcIdx, GcIdx) {
    let proto_value = vm.object_proto.clone();

    let mut method_props: HashMap<Rc<str>, PropertyDescriptor> = HashMap::new();
    for (n, f, len) in methods {
        let func_idx = vm.new_native_function(n, *f, *len);
        method_props.insert(Rc::from(*n), data_prop(Value::Object(func_idx)));
    }

    let proto_obj = HeapObj::Object(ObjectData {
        props: RefCell::new(method_props),
        proto: RefCell::new(Some(proto_value.clone())),
        extensible: Cell::new(true),
        class_name: Some(Rc::from(name)),
    });
    let proto_idx = GcIdx(vm.heap.allocate(proto_obj));

    let ctor_func = FunctionData {
        name: Some(Rc::from(name)),
        kind: FunctionKind::Native { func: object_constructor, length: 1 },
        closure: vm.global,
        prototype: RefCell::new(Some(Value::Object(proto_idx))),
        props: RefCell::new(HashMap::new()),
    };
    let ctor_idx = GcIdx(vm.heap.allocate(HeapObj::Function(ctor_func)));
    // constructor.prototype
    vm.heap.with_obj(ctor_idx.0, |obj| {
        obj.props().borrow_mut().insert(Rc::from("prototype"), data_prop(Value::Object(proto_idx)));
    });
    // prototype.constructor
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props().borrow_mut().insert(Rc::from("constructor"), data_prop(Value::Object(ctor_idx)));
    });

    (ctor_idx, proto_idx)
}


fn make_error_constructor(vm: &mut Vm, name: &str) -> (GcIdx, GcIdx) {
    let error_proto_val = vm.error_proto.clone();
    let proto_obj = HeapObj::Object(ObjectData {
        props: RefCell::new(HashMap::new()),
        proto: RefCell::new(Some(error_proto_val.clone())),
        extensible: Cell::new(true),
        class_name: Some(Rc::from(name)),
    });
    let proto_idx = GcIdx(vm.heap.allocate(proto_obj));

    let ctor_func = FunctionData {
        name: Some(Rc::from(name)),
        kind: FunctionKind::Native { func: error_constructor, length: 1 },
        closure: vm.global,
        prototype: RefCell::new(Some(Value::Object(proto_idx))),
        props: RefCell::new(HashMap::new()),
    };
    let ctor_idx = GcIdx(vm.heap.allocate(HeapObj::Function(ctor_func)));
    vm.heap.with_obj(ctor_idx.0, |obj| {
        obj.props().borrow_mut().insert(Rc::from("prototype"), data_prop(Value::Object(proto_idx)));
    });
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props().borrow_mut().insert(Rc::from("constructor"), data_prop(Value::Object(ctor_idx)));
    });

    (ctor_idx, proto_idx)
}

fn define_global(vm: &mut Vm, name: &str, value: Value) {
    env::declare(&vm.heap, vm.global, name, value, BindingKind::Var);
}

fn define_global_property(vm: &mut Vm, name: &str, value: Value) {
    env::declare(&vm.heap, vm.global, name, value, BindingKind::Var);
}

fn get_arg<T: Default>(args: &[Value], idx: usize) -> Value {
    args.get(idx).cloned().unwrap_or(Value::Undefined)
}

fn to_index(len: usize, value: &Value, vm: &mut Vm) -> error::Result<usize> {
    let n = vm.to_number(value)?;
    if n.is_nan() { return Ok(0); }
    if n.is_infinite() || n > len as f64 { return Ok(len); }
    if n < 0.0 { return Ok(0); }
    Ok(n as usize)
}

fn to_relative_index(len: usize, value: &Value, vm: &mut Vm) -> error::Result<usize> {
    let n = vm.to_number(value)?;
    if n.is_nan() { return Ok(0); }
    let idx = n as isize;
    if idx < 0 { Ok((len as isize + idx).max(0) as usize) } else { Ok(idx as usize) }
}

fn to_length(vm: &mut Vm, value: &Value) -> error::Result<usize> {
    let n = vm.to_number(value)?;
    if n.is_nan() || n <= 0.0 { return Ok(0); }
    if n.is_infinite() { return Ok(usize::MAX); }
    if n > usize::MAX as f64 { return Ok(usize::MAX); }
    Ok(n as usize)
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
            Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Symbol(_) => {
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
        Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Symbol(_) => vm.to_object(first),
        Value::Object(_) => Ok(first.clone()),
    }
}

fn object_to_string_native(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    object_to_string(vm, this, None)
}

fn object_has_own_property(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let this = this.unwrap_or(Value::Undefined);
    let key = if let Some(a) = args.get(0) {
        vm.to_property_key(a)?
    } else {
        String::new()
    };
    match &this {
        Value::Object(idx) => {
            let has = vm.heap.with_obj(idx.0, |obj| {
                obj.props().borrow().contains_key(key.as_str()) || {
                    if let HeapObj::Array(a) = obj {
                        if key == "length" { return true; }
                        if let Ok(i) = key.parse::<usize>() {
                            return i < a.items.borrow().len();
                        }
                    }
                    false
                }
            });
            Ok(Value::Bool(has))
        }
        Value::String(s) => {
            if key == "length" { return Ok(Value::Bool(true)); }
            if let Ok(i) = key.parse::<usize>() {
                return Ok(Value::Bool(i < s.chars().count()));
            }
            Ok(Value::Bool(false))
        }
        _ => Ok(Value::Bool(false)),
    }
}

fn object_value_of(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(v) = this { return Ok(v); }
    Ok(Value::Undefined)
}

fn object_is_prototype_of(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let proto_val = this.unwrap_or(Value::Undefined);
    let candidate = args.get(0).cloned().unwrap_or(Value::Undefined);
    if proto_val.is_nullish() || candidate.is_nullish() { return Ok(Value::Bool(false)); }
    if let (Value::Object(pidx), Value::Object(cidx)) = (&proto_val, &candidate) {
        let mut cur = cidx.clone();
        loop {
            let proto = vm.heap.with_obj(cur.0, |obj| obj.proto().borrow().clone());
            match proto {
                Some(Value::Object(next)) => {
                    if next == *pidx { return Ok(Value::Bool(true)); }
                    cur = next;
                }
                _ => return Ok(Value::Bool(false)),
            }
        }
    }
    Ok(Value::Bool(false))
}

fn object_keys(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    let mut keys = Vec::new();
    if let Value::Object(idx) = target {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                for i in 0..a.items.borrow().len() {
                    keys.push(Value::String(Rc::from(i.to_string().as_str())));
                }
            } else {
                for k in obj.props().borrow().keys() {
                    keys.push(Value::String(k.clone()));
                }
            }
        });
    }
    let arr = HeapObj::Array(ArrayData {
        items: RefCell::new(keys),
        props: RefCell::new(HashMap::new()),
        proto: RefCell::new(Some(vm.array_proto.clone())),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}

fn object_values(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    let mut values = Vec::new();
    if let Value::Object(idx) = target {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                values.extend(a.items.borrow().iter().cloned());
            } else {
                for d in obj.props().borrow().values() {
                    values.push(d.value.clone());
                }
            }
        });
    }
    let arr = HeapObj::Array(ArrayData {
        items: RefCell::new(values),
        props: RefCell::new(HashMap::new()),
        proto: RefCell::new(Some(vm.array_proto.clone())),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}

fn object_entries(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    let mut entries = Vec::new();
    let array_proto = vm.array_proto.clone();
    let object_proto = vm.object_proto.clone();
    if let Value::Object(idx) = target {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                for (i, v) in a.items.borrow().iter().enumerate() {
                    let pair = new_object_with_props(&vm.heap, Some(object_proto.clone()), vec![
                        ("0", Value::String(Rc::from(i.to_string().as_str()))),
                        ("1", v.clone()),
                    ]);
                    entries.push(Value::Object(pair));
                }
            } else {
                for (k, d) in obj.props().borrow().iter() {
                    let pair = new_object_with_props(&vm.heap, Some(object_proto.clone()), vec![
                        ("0", Value::String(k.clone())),
                        ("1", d.value.clone()),
                    ]);
                    entries.push(Value::Object(pair));
                }
            }
        });
    }
    let arr = HeapObj::Array(ArrayData {
        items: RefCell::new(entries),
        props: RefCell::new(HashMap::new()),
        proto: RefCell::new(Some(array_proto)),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}

fn object_create(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let proto = args.get(0).cloned().unwrap_or(Value::Null);
    let proto_opt = if proto.is_nullish() { None } else { Some(proto) };
    let idx = new_plain_object(&vm.heap, proto_opt);
    if let Some(props) = args.get(1) {
        object_define_properties(vm, &[Value::Object(idx), props.clone()], None)?;
    }
    Ok(Value::Object(idx))
}

fn object_assign(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    if !target.is_object() { return Err(Error::type_err("Object.assign target must be an object".to_string())); }
    let Value::Object(tidx) = target else { unreachable!() };
    for src in args.iter().skip(1) {
        if let Value::Object(sidx) = src {
            let (keys, is_array) = vm.heap.with_obj(sidx.0, |obj| {
                let mut keys = Vec::new();
                if let HeapObj::Array(a) = obj {
                    for i in 0..a.items.borrow().len() { keys.push(i.to_string()); }
                    (keys, true)
                } else {
                    for k in obj.props().borrow().keys() { keys.push(k.to_string()); }
                    (keys, false)
                }
            });
            for k in keys {
                let val = vm.get_property(src, &k)?;
                vm.set_property(&Value::Object(tidx), &k, val)?;
                if is_array && !matches!(k.parse::<usize>(), Ok(_)) { continue; }
            }
        }
    }
    Ok(Value::Object(tidx))
}

fn object_freeze(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = target {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Object(o) = obj {
                o.extensible.set(false);
                for d in o.props.borrow_mut().values_mut() {
                    d.writable = false;
                    d.configurable = false;
                }
            }
        });
    }
    Ok(target)
}

fn object_define_property(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    let key = args.get(1).map(|v| vm.to_property_key(v)).transpose()?.unwrap_or_default();
    let desc = args.get(2).cloned().unwrap_or(Value::Undefined);
    if let Value::Object(idx) = target {
        let mut value = Value::Undefined;
        let mut writable = false;
        let mut enumerable = false;
        let mut configurable = false;
        let mut is_data = false;
        if let Value::Object(didx) = desc {
            if let Some(v) = vm.get_property(&desc, "value").ok() { value = v; is_data = true; }
            if let Some(v) = vm.get_property(&desc, "writable").ok() { writable = v.is_truthy(); is_data = true; }
            if let Some(v) = vm.get_property(&desc, "enumerable").ok() { enumerable = v.is_truthy(); }
            if let Some(v) = vm.get_property(&desc, "configurable").ok() { configurable = v.is_truthy(); }
        }
        let descriptor = if is_data {
            PropertyDescriptor { value, writable, enumerable, configurable, get: None, set: None, is_accessor: false }
        } else {
            PropertyDescriptor { value: Value::Undefined, writable: false, enumerable, configurable, get: None, set: None, is_accessor: false }
        };
        vm.heap.with_obj(idx.0, |obj| {
            obj.props().borrow_mut().insert(Rc::from(key.as_str()), descriptor);
        });
    }
    Ok(target)
}

fn object_get_own_property_descriptor(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    let key = args.get(1).map(|v| vm.to_property_key(v)).transpose()?.unwrap_or_default();
    let object_proto = vm.object_proto.clone();
    if let Value::Object(idx) = target {
        let value = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                if key == "length" {
                    return Some(Value::Number(a.items.borrow().len() as f64));
                }
                if let Ok(i) = key.parse::<usize>() {
                    return a.items.borrow().get(i).cloned();
                }
            }
            obj.props().borrow().get(key.as_str()).map(|d| d.value.clone())
        });
        if let Some(v) = value {
            let desc = new_object_with_props(&vm.heap, Some(object_proto), vec![
                ("value", v),
                ("writable", Value::Bool(true)),
                ("enumerable", Value::Bool(true)),
                ("configurable", Value::Bool(true)),
            ]);
            return Ok(Value::Object(desc));
        }
    }
    Ok(Value::Undefined)
}

fn object_define_properties(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    let props = args.get(1).cloned().unwrap_or(Value::Undefined);
    if let (Value::Object(_), Value::Object(_)) = (&target, &props) {
        let keys = object_keys(vm, &[props.clone()], None)?;
        if let Value::Object(kidx) = keys {
            let key_objs = vm.heap.with_obj(kidx.0, |obj| {
                if let HeapObj::Array(a) = obj { a.items.borrow().clone() } else { Vec::new() }
            });
            for k_val in key_objs {
                let key_str = vm.to_string(&k_val)?;
                let desc = vm.get_property(&props, &key_str)?;
                object_define_property(vm, &[target.clone(), k_val, desc], None)?;
            }
        }
    }
    Ok(target)
}


// Minimal stubs to keep the crate compiling while parser/lexer work is in progress.

fn error_constructor(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let msg = args.get(0)
        .map(|v| vm.to_string(v).unwrap_or_else(|_| Rc::from("")))
        .unwrap_or_else(|| Rc::from(""));
    let idx = vm.new_object();
    vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::Object(o) = obj {
            o.props.borrow_mut().insert(Rc::from("message"), data_prop(Value::String(msg)));
            o.props.borrow_mut().insert(Rc::from("name"), data_prop(Value::String(Rc::from("Error"))));
        }
    });
    Ok(Value::Object(idx))
}

pub fn setup(vm: &mut Vm) {
    let (object_ctor, object_proto) = make_builtin_constructor(vm, "Object", &[
        ("toString", object_to_string_native, 0),
        ("hasOwnProperty", object_has_own_property, 1),
        ("valueOf", object_value_of, 0),
    ]);
    define_global(vm, "Object", Value::Object(object_ctor));
    vm.object_proto = Value::Object(object_proto);

    let (error_ctor, _error_proto) = make_error_constructor(vm, "Error");
    define_global(vm, "Error", Value::Object(error_ctor));
}

// =========================================================================
// Math
// =========================================================================
fn math_unary(f: fn(f64) -> f64, vm: &mut Vm, args: &[Value]) -> error::Result<Value> {
    let n = vm.to_number(args.get(0).unwrap_or(&Value::Undefined))?;
    Ok(Value::Number(f(n)))
}
fn math_floor(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> { math_unary(f64::floor, vm, args) }
fn math_ceil(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> { math_unary(f64::ceil, vm, args) }
fn math_round(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> { math_unary(|n| n.round(), vm, args) }
fn math_trunc(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> { math_unary(f64::trunc, vm, args) }
fn math_abs(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> { math_unary(f64::abs, vm, args) }
fn math_sign(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let n = vm.to_number(args.get(0).unwrap_or(&Value::Undefined))?;
    Ok(Value::Number(if n > 0.0 {1.0} else if n < 0.0 {-1.0} else {0.0}))
}
fn math_sqrt(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> { math_unary(f64::sqrt, vm, args) }
fn math_cbrt(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> { math_unary(f64::cbrt, vm, args) }
fn math_exp(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> { math_unary(f64::exp, vm, args) }
fn math_log(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> { math_unary(f64::ln, vm, args) }
fn math_log2(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> { math_unary(f64::log2, vm, args) }
fn math_log10(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> { math_unary(f64::log10, vm, args) }
fn math_sin(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> { math_unary(f64::sin, vm, args) }
fn math_cos(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> { math_unary(f64::cos, vm, args) }
fn math_tan(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> { math_unary(f64::tan, vm, args) }
fn math_pow(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let a = vm.to_number(args.get(0).unwrap_or(&Value::Undefined))?;
    let b = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    Ok(Value::Number(a.powf(b)))
}
fn math_max(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let mut m = f64::NEG_INFINITY;
    for a in args { let n = vm.to_number(a)?; if n > m { m = n; } }
    Ok(Value::Number(m))
}
fn math_min(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let mut m = f64::INFINITY;
    for a in args { let n = vm.to_number(a)?; if n < m { m = n; } }
    Ok(Value::Number(m))
}
fn math_random(vm: &mut Vm, _args: &[Value], _: Option<Value>) -> error::Result<Value> {
    use std::cell::Cell;
    thread_local! { static STATE: Cell<u64> = Cell::new(0x2545F4914F6CDD1D); }
    let r = STATE.with(|s| { let mut x = s.get(); x ^= x << 13; x ^= x >> 7; x ^= x << 17; s.set(x); x as f64 / u64::MAX as f64 });
    Ok(Value::Number(r))
}

fn build_math(vm: &mut Vm) -> Value {
    let mut props: HashMap<Rc<str>, PropertyDescriptor> = HashMap::new();
    // build methods first, collect into a temp vec
    let mut method_entries: Vec<(&str, NativeFn, usize)> = vec![
        ("floor", math_floor, 1), ("ceil", math_ceil, 1), ("round", math_round, 1),
        ("trunc", math_trunc, 1), ("abs", math_abs, 1), ("sign", math_sign, 1),
        ("sqrt", math_sqrt, 1), ("cbrt", math_cbrt, 1), ("exp", math_exp, 1),
        ("log", math_log, 1), ("log2", math_log2, 1), ("log10", math_log10, 1),
        ("sin", math_sin, 1), ("cos", math_cos, 1), ("tan", math_tan, 1),
        ("pow", math_pow, 2), ("max", math_max, 2), ("min", math_min, 2),
        ("random", math_random, 0),
    ];
    for (name, f, len) in method_entries.drain(..) {
        let idx = vm.new_native_function(name, f, len);
        props.insert(Rc::from(name), data_prop(Value::Object(idx)));
    }
    props.insert(Rc::from("PI"), data_prop(Value::Number(std::f64::consts::PI)));
    props.insert(Rc::from("E"), data_prop(Value::Number(std::f64::consts::E)));
    props.insert(Rc::from("LN2"), data_prop(Value::Number(std::f64::consts::LN_2)));
    props.insert(Rc::from("LN10"), data_prop(Value::Number(std::f64::consts::LN_10)));
    props.insert(Rc::from("LOG2E"), data_prop(Value::Number(std::f64::consts::LOG2_E)));
    props.insert(Rc::from("LOG10E"), data_prop(Value::Number(std::f64::consts::LOG10_E)));
    props.insert(Rc::from("SQRT2"), data_prop(Value::Number(std::f64::consts::SQRT_2)));
    props.insert(Rc::from("SQRT1_2"), data_prop(Value::Number(std::f64::consts::FRAC_1_SQRT_2)));
    let obj = HeapObj::Object(ObjectData {
        props: RefCell::new(props), proto: RefCell::new(Some(vm.object_proto.clone())),
        extensible: Cell::new(false), class_name: Some(Rc::from("Math")),
    });
    Value::Object(GcIdx(vm.heap.allocate(obj)))
}

// =========================================================================
// console
// =========================================================================
fn console_log(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let parts: Vec<String> = args.iter().map(|a| vm.to_string(a).map(|s| s.to_string()).unwrap_or_default()).collect();
    println!("{}", parts.join(" "));
    Ok(Value::Undefined)
}
fn build_console(vm: &mut Vm) -> Value {
    let mut props: HashMap<Rc<str>, PropertyDescriptor> = HashMap::new();
    for name in &["log", "error", "warn", "info", "debug", "dir", "trace"] {
        let idx = vm.new_native_function(name, console_log, 0);
        props.insert(Rc::from(*name), data_prop(Value::Object(idx)));
    }
    let obj = HeapObj::Object(ObjectData {
        props: RefCell::new(props), proto: RefCell::new(Some(vm.object_proto.clone())),
        extensible: Cell::new(true), class_name: Some(Rc::from("Object")),
    });
    Value::Object(GcIdx(vm.heap.allocate(obj)))
}

// =========================================================================
// JSON
// =========================================================================
fn json_stringify(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let v = args.get(0).unwrap_or(&Value::Undefined);
    match stringify_value(vm, v) {
        Some(s) => Ok(Value::String(Rc::from(s.as_str()))),
        None => Ok(Value::Undefined),
    }
}
fn stringify_value(vm: &mut Vm, v: &Value) -> Option<String> {
    match v {
        Value::Undefined => None,
        Value::Null => Some("null".into()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => {
            if n.is_nan() || n.is_infinite() { None } else { Some(crate::value::num_to_string(*n)) }
        }
        Value::String(s) => Some(format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\t', "\\t"))),
        Value::Symbol(_) => None,
        Value::Object(idx) => {
            let (is_arr, items, props, proto) = vm.heap.with_obj(idx.0, |obj| {
                match obj {
                    HeapObj::Array(a) => (true, a.items.borrow().clone(), HashMap::new(), None),
                    HeapObj::Object(o) => (false, Vec::new(), o.props.borrow().clone(), o.proto.borrow().clone()),
                    HeapObj::Function(_) => (false, Vec::new(), HashMap::new(), None),
                    _ => (false, Vec::new(), obj.props().borrow().clone(), obj.proto().borrow().clone()),
                }
            });
            if is_arr {
                let parts: Vec<String> = items.iter().filter_map(|i| stringify_value(vm, i)).collect();
                Some(format!("[{}]", parts.join(",")))
            } else {
                let _ = proto;
                let mut pairs = Vec::new();
                for (k, d) in &props {
                    if !d.enumerable { continue; }
                    if let Some(vs) = stringify_value(vm, &d.value) {
                        pairs.push(format!("\"{}\":{}", k, vs));
                    }
                }
                Some(format!("{{{}}}", pairs.join(",")))
            }
        }
    }
}
fn json_parse(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let s = match args.get(0) { Some(Value::String(s)) => s.to_string(), _ => return Ok(Value::Null) };
    parse_json_value(vm, &mut s.chars().peekable())
}
fn parse_json_value(vm: &mut Vm, chars: &mut std::iter::Peekable<std::str::Chars>) -> error::Result<Value> {
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() { chars.next(); } else { break; }
    }
    match chars.peek() {
        Some(&'{') => { chars.next(); parse_json_obj(vm, chars) }
        Some(&'[') => { chars.next(); parse_json_arr(vm, chars) }
        Some(&'"') => { chars.next(); parse_json_str(chars) }
        Some('t') => { chars.take(4).for_each(|_|{}); Ok(Value::Bool(true)) }
        Some('f') => { chars.take(5).for_each(|_|{}); Ok(Value::Bool(false)) }
        Some('n') => { chars.take(4).for_each(|_|{}); Ok(Value::Null) }
        Some(c) if *c == '-' || c.is_ascii_digit() => parse_json_num(chars),
        _ => Err(Error::syntax("Invalid JSON".to_string())),
    }
}
fn parse_json_obj(vm: &mut Vm, chars: &mut std::iter::Peekable<std::str::Chars>) -> error::Result<Value> {
    let mut props: HashMap<Rc<str>, PropertyDescriptor> = HashMap::new();
    loop {
        while let Some(&c) = chars.peek() { if c.is_whitespace() { chars.next(); } else { break; } }
        if chars.peek() == Some(&'}') { chars.next(); break; }
        let key = match parse_json_str(chars)? {
            Value::String(s) => s.to_string(),
            _ => String::new(),
        };
        while chars.peek() != Some(&':') { chars.next(); }
        chars.next();
        let val = parse_json_value(vm, chars)?;
        props.insert(Rc::from(key.as_str()), data_prop(val));
        while let Some(&c) = chars.peek() { if c.is_whitespace() || c == ',' { chars.next(); } else { break; } }
        if chars.peek() == Some(&'}') { chars.next(); break; }
    }
    let obj = HeapObj::Object(ObjectData { props: RefCell::new(props), proto: RefCell::new(Some(vm.object_proto.clone())), extensible: Cell::new(true), class_name: None });
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj))))
}
fn parse_json_arr(vm: &mut Vm, chars: &mut std::iter::Peekable<std::str::Chars>) -> error::Result<Value> {
    let mut items = Vec::new();
    loop {
        while let Some(&c) = chars.peek() { if c.is_whitespace() { chars.next(); } else { break; } }
        if chars.peek() == Some(&']') { chars.next(); break; }
        items.push(parse_json_value(vm, chars)?);
        while let Some(&c) = chars.peek() { if c.is_whitespace() || c == ',' { chars.next(); } else { break; } }
        if chars.peek() == Some(&']') { chars.next(); break; }
    }
    let obj = HeapObj::Array(ArrayData { items: RefCell::new(items), props: RefCell::new(HashMap::new()), proto: RefCell::new(Some(vm.array_proto.clone())) });
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj))))
}
fn parse_json_str(chars: &mut std::iter::Peekable<std::str::Chars>) -> error::Result<Value> {
    let mut s = String::new();
    while let Some(c) = chars.next() {
        if c == '"' { break; }
        if c == '\\' {
            match chars.next() {
                Some('n') => s.push('\n'), Some('t') => s.push('\t'), Some('"') => s.push('"'),
                Some('\\') => s.push('\\'), Some(c) => s.push(c), None => break,
            }
        } else { s.push(c); }
    }
    Ok(Value::String(Rc::from(s.as_str())))
}
fn parse_json_num(chars: &mut std::iter::Peekable<std::str::Chars>) -> error::Result<Value> {
    let mut s = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E' { s.push(c); chars.next(); } else { break; }
    }
    Ok(Value::Number(s.parse().unwrap_or(f64::NAN)))
}
fn build_json(vm: &mut Vm) -> Value {
    let mut props: HashMap<Rc<str>, PropertyDescriptor> = HashMap::new();
    let pi = vm.new_native_function("parse", json_parse, 1);
    let si = vm.new_native_function("stringify", json_stringify, 3);
    props.insert(Rc::from("parse"), data_prop(Value::Object(pi)));
    props.insert(Rc::from("stringify"), data_prop(Value::Object(si)));
    let obj = HeapObj::Object(ObjectData { props: RefCell::new(props), proto: RefCell::new(Some(vm.object_proto.clone())), extensible: Cell::new(true), class_name: Some(Rc::from("JSON")) });
    Value::Object(GcIdx(vm.heap.allocate(obj)))
}

// =========================================================================
// Global functions
// =========================================================================
fn global_parse_int(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let s = match args.get(0) { Some(Value::String(s)) => s.trim().to_string(), Some(v) => vm.to_string(v)?.to_string(), None => return Ok(Value::Number(f64::NAN)) };
    let radix = args.get(1).and_then(|v| if let Value::Number(n)=v {Some(*n as u32)} else {None}).unwrap_or(10);
    let radix = if radix == 0 {10} else {radix};
    let s = s.trim_start_matches('+');
    let neg = s.starts_with('-');
    let s = s.trim_start_matches('-');
    let s = if radix == 16 && (s.starts_with("0x") || s.starts_with("0X")) { &s[2..] } else { s };
    match i64::from_str_radix(s, radix) { Ok(n) => Ok(Value::Number((if neg {-n} else {n}) as f64)), Err(_) => Ok(Value::Number(f64::NAN)) }
}
fn global_parse_float(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let s = match args.get(0) { Some(Value::String(s)) => s.trim().to_string(), Some(v) => vm.to_string(v)?.to_string(), None => return Ok(Value::Number(f64::NAN)) };
    Ok(Value::Number(s.parse().unwrap_or(f64::NAN)))
}
fn global_is_nan(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let n = vm.to_number(args.get(0).unwrap_or(&Value::Undefined))?;
    Ok(Value::Bool(n.is_nan()))
}
fn global_is_finite(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let n = vm.to_number(args.get(0).unwrap_or(&Value::Undefined))?;
    Ok(Value::Bool(n.is_finite()))
}

// =========================================================================
// Array prototype + constructor
// =========================================================================
fn array_is_array(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(is_array(args.get(0).unwrap_or(&Value::Undefined), &vm.heap)))
}
fn array_push(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow_mut().extend_from_slice(args);
            }
        });
        let len = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj { a.items.borrow().len() } else { 0 }
        });
        return Ok(Value::Number(len as f64));
    }
    Ok(Value::Number(0.0))
}
fn array_pop(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        return Ok(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj { a.items.borrow_mut().pop().unwrap_or(Value::Undefined) } else { Value::Undefined }
        }));
    }
    Ok(Value::Undefined)
}
fn array_join(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let sep = match args.get(0) { Some(Value::String(s)) => s.to_string(), Some(v) if !v.is_undefined() => vm.to_string(v)?.to_string(), _ => ",".to_string() };
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| { if let HeapObj::Array(a)=obj { a.items.borrow().clone() } else { Vec::new() } });
        let parts: Vec<String> = items.iter().map(|i| if i.is_nullish() { String::new() } else { vm.to_string(i).map(|s|s.to_string()).unwrap_or_default() }).collect();
        return Ok(Value::String(Rc::from(parts.join(&sep).as_str())));
    }
    Ok(Value::String(Rc::from("")))
}
fn array_map(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| { if let HeapObj::Array(a)=obj { a.items.borrow().clone() } else { Vec::new() } });
        let mut result = Vec::new();
        for (i, item) in items.iter().enumerate() {
            result.push(vm.call_function(&cb, &[item.clone(), Value::Number(i as f64), this.clone().unwrap_or(Value::Undefined)], Some(Value::Undefined))?);
        }
        let arr = HeapObj::Array(ArrayData { items: RefCell::new(result), props: RefCell::new(HashMap::new()), proto: RefCell::new(Some(vm.array_proto.clone())) });
        return Ok(Value::Object(GcIdx(vm.heap.allocate(arr))));
    }
    Ok(Value::Undefined)
}
fn array_filter(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| { if let HeapObj::Array(a)=obj { a.items.borrow().clone() } else { Vec::new() } });
        let mut result = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let keep = vm.call_function(&cb, &[item.clone(), Value::Number(i as f64), this.clone().unwrap_or(Value::Undefined)], Some(Value::Undefined))?;
            if keep.is_truthy() { result.push(item.clone()); }
        }
        let arr = HeapObj::Array(ArrayData { items: RefCell::new(result), props: RefCell::new(HashMap::new()), proto: RefCell::new(Some(vm.array_proto.clone())) });
        return Ok(Value::Object(GcIdx(vm.heap.allocate(arr))));
    }
    Ok(Value::Undefined)
}
fn array_reduce(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| { if let HeapObj::Array(a)=obj { a.items.borrow().clone() } else { Vec::new() } });
        let (mut acc, start) = if args.len() >= 2 { (args[1].clone(), 0) } else { (items.get(0).cloned().unwrap_or(Value::Undefined), 1) };
        for i in start..items.len() {
            acc = vm.call_function(&cb, &[acc, items[i].clone(), Value::Number(i as f64), this.clone().unwrap_or(Value::Undefined)], Some(Value::Undefined))?;
        }
        return Ok(acc);
    }
    Ok(Value::Undefined)
}
fn array_for_each(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| { if let HeapObj::Array(a)=obj { a.items.borrow().clone() } else { Vec::new() } });
        for (i, item) in items.iter().enumerate() {
            vm.call_function(&cb, &[item.clone(), Value::Number(i as f64), this.clone().unwrap_or(Value::Undefined)], Some(Value::Undefined))?;
        }
    }
    Ok(Value::Undefined)
}
fn array_index_of(_vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let pos = _vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().iter().position(|i| i == &target)
            } else { None }
        });
        return Ok(Value::Number(pos.map(|i| i as f64).unwrap_or(-1.0)));
    }
    Ok(Value::Number(-1.0))
}
fn array_includes(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let found = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj { a.items.borrow().iter().any(|i| i == &target) } else { false }
        });
        return Ok(Value::Bool(found));
    }
    Ok(Value::Bool(false))
}
fn array_slice(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| { if let HeapObj::Array(a)=obj { a.items.borrow().clone() } else { Vec::new() } });
        let len = items.len() as i64;
        let start = args.get(0).and_then(|v| if let Value::Number(n)=v {Some(*n as i64)} else {None}).unwrap_or(0);
        let end = args.get(1).and_then(|v| if let Value::Number(n)=v {Some(*n as i64)} else {None}).unwrap_or(len);
        let s = if start < 0 {(len+start).max(0) as usize} else {(start as usize).min(items.len())};
        let e = if end < 0 {(len+end).max(0) as usize} else {(end as usize).min(items.len())};
        let sliced = if s < e { items[s..e].to_vec() } else { Vec::new() };
        let arr = HeapObj::Array(ArrayData { items: RefCell::new(sliced), props: RefCell::new(HashMap::new()), proto: RefCell::new(Some(vm.array_proto.clone())) });
        return Ok(Value::Object(GcIdx(vm.heap.allocate(arr))));
    }
    Ok(Value::Undefined)
}
fn array_concat(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let mut items = Vec::new();
    if let Some(Value::Object(idx)) = this {
        items = vm.heap.with_obj(idx.0, |obj| { if let HeapObj::Array(a)=obj { a.items.borrow().clone() } else { Vec::new() } });
    }
    for a in args {
        if let Value::Object(aidx) = a {
            let is_arr = vm.heap.with_obj(aidx.0, |obj| matches!(obj, HeapObj::Array(_)));
            if is_arr {
                let extra = vm.heap.with_obj(aidx.0, |obj| { if let HeapObj::Array(a)=obj { a.items.borrow().clone() } else { Vec::new() } });
                items.extend(extra);
                continue;
            }
        }
        items.push(a.clone());
    }
    let arr = HeapObj::Array(ArrayData { items: RefCell::new(items), props: RefCell::new(HashMap::new()), proto: RefCell::new(Some(vm.array_proto.clone())) });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}
fn array_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = if args.len() == 1 {
        if let Some(Value::Number(n)) = args.get(0) { vec![Value::Undefined; *n as usize] } else { args.to_vec() }
    } else { args.to_vec() };
    if let Some(Value::Object(idx)) = this { return Ok(Value::Object(idx)); }
    let arr = HeapObj::Array(ArrayData { items: RefCell::new(items), props: RefCell::new(HashMap::new()), proto: RefCell::new(Some(vm.array_proto.clone())) });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}

fn array_find(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| { if let HeapObj::Array(a)=obj { a.items.borrow().clone() } else { Vec::new() } });
        for (i, item) in items.iter().enumerate() {
            let found = vm.call_function(&cb, &[item.clone(), Value::Number(i as f64), this.clone().unwrap_or(Value::Undefined)], Some(Value::Undefined))?;
            if found.is_truthy() { return Ok(item.clone()); }
        }
    }
    Ok(Value::Undefined)
}
fn array_find_index(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| { if let HeapObj::Array(a)=obj { a.items.borrow().clone() } else { Vec::new() } });
        for (i, item) in items.iter().enumerate() {
            let found = vm.call_function(&cb, &[item.clone(), Value::Number(i as f64), this.clone().unwrap_or(Value::Undefined)], Some(Value::Undefined))?;
            if found.is_truthy() { return Ok(Value::Number(i as f64)); }
        }
    }
    Ok(Value::Number(-1.0))
}
fn array_find_last(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| { if let HeapObj::Array(a)=obj { a.items.borrow().clone() } else { Vec::new() } });
        for (i, item) in items.iter().enumerate().rev() {
            let found = vm.call_function(&cb, &[item.clone(), Value::Number(i as f64), this.clone().unwrap_or(Value::Undefined)], Some(Value::Undefined))?;
            if found.is_truthy() { return Ok(item.clone()); }
        }
    }
    Ok(Value::Undefined)
}
fn array_fill(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let value = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| { if let HeapObj::Array(a)=obj { a.items.borrow().clone() } else { Vec::new() } });
        let len = items.len() as i64;
        let start = args.get(1).and_then(|v| if let Value::Number(n)=v {Some(*n as i64)} else {None}).unwrap_or(0);
        let end = args.get(2).and_then(|v| if let Value::Number(n)=v {Some(*n as i64)} else {None}).unwrap_or(len);
        let s = if start < 0 { (len+start).max(0) as usize } else { (start as usize).min(items.len()) };
        let e = if end < 0 { (len+end).max(0) as usize } else { (end as usize).min(items.len()) };
        if s < e {
            vm.heap.with_obj(idx.0, |obj| {
                if let HeapObj::Array(a) = obj {
                    let mut items = a.items.borrow_mut();
                    for i in s..e.min(items.len()) { items[i] = value.clone(); }
                }
            });
        }
        return Ok(Value::Object(idx));
    }
    Ok(Value::Undefined)
}
fn array_some(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| { if let HeapObj::Array(a)=obj { a.items.borrow().clone() } else { Vec::new() } });
        for (i, item) in items.iter().enumerate() {
            let found = vm.call_function(&cb, &[item.clone(), Value::Number(i as f64), this.clone().unwrap_or(Value::Undefined)], Some(Value::Undefined))?;
            if found.is_truthy() { return Ok(Value::Bool(true)); }
        }
    }
    Ok(Value::Bool(false))
}
fn array_every(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| { if let HeapObj::Array(a)=obj { a.items.borrow().clone() } else { Vec::new() } });
        for (i, item) in items.iter().enumerate() {
            let ok = vm.call_function(&cb, &[item.clone(), Value::Number(i as f64), this.clone().unwrap_or(Value::Undefined)], Some(Value::Undefined))?;
            if !ok.is_truthy() { return Ok(Value::Bool(false)); }
        }
    }
    Ok(Value::Bool(true))
}

// =========================================================================
// String prototype + constructor
// =========================================================================
fn str_val(vm: &mut Vm, this: &Option<Value>) -> error::Result<String> {
    match this { Some(Value::String(s)) => Ok(s.to_string()), Some(Value::Object(idx)) => Ok(vm.heap.with_obj(idx.0, |o| { if let HeapObj::Object(o)=o { if let Some(cn)=&o.class_name { cn.to_string() } else { "[object Object]".into() } } else { "[object Object]".into() } })), Some(v) => Ok(vm.to_string(v)?.to_string()), None => Ok("undefined".into()) }
}
fn str_char_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let i = args.get(0).and_then(|v| if let Value::Number(n)=v {Some(*n as usize)} else {None}).unwrap_or(0);
    Ok(s.chars().nth(i).map(|c| Value::String(Rc::from(c.to_string().as_str()))).unwrap_or(Value::String(Rc::from(""))))
}
fn str_char_code_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let i = args.get(0).and_then(|v| if let Value::Number(n)=v {Some(*n as usize)} else {None}).unwrap_or(0);
    Ok(s.chars().nth(i).map(|c| Value::Number(c as u32 as f64)).unwrap_or(Value::Number(f64::NAN)))
}
fn str_index_of(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let n = args.get(0).map(|v| crate::value::value_to_debug_string(v)).unwrap_or_default();
    Ok(Value::Number(s.find(&n).map(|i| i as f64).unwrap_or(-1.0)))
}
fn str_slice(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i64;
    let start = args.get(0).and_then(|v| if let Value::Number(n)=v {Some(*n as i64)} else {None}).unwrap_or(0);
    let end = args.get(1).and_then(|v| if let Value::Number(n)=v {Some(*n as i64)} else {None}).unwrap_or(len);
    let st = if start < 0 {(len+start).max(0) as usize} else {(start as usize).min(chars.len())};
    let en = if end < 0 {(len+end).max(0) as usize} else {(end as usize).min(chars.len())};
    let r: String = if st < en { chars[st..en].iter().collect() } else { String::new() };
    Ok(Value::String(Rc::from(r.as_str())))
}
fn str_to_upper(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Rc::from(str_val(vm, &this)?.to_uppercase().as_str())))
}
fn str_to_lower(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Rc::from(str_val(vm, &this)?.to_lowercase().as_str())))
}
fn str_trim(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Rc::from(str_val(vm, &this)?.trim())))
}
fn str_split(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let sep = args.get(0).map(|v| crate::value::value_to_debug_string(v));
    let parts: Vec<String> = match sep { None => vec![s], Some(sep) if sep.is_empty() => s.chars().map(|c| c.to_string()).collect(), Some(sep) => s.split(&sep).map(|p| p.to_string()).collect() };
    let items: Vec<Value> = parts.into_iter().map(|p| Value::String(Rc::from(p.as_str()))).collect();
    let arr = HeapObj::Array(ArrayData { items: RefCell::new(items), props: RefCell::new(HashMap::new()), proto: RefCell::new(Some(vm.array_proto.clone())) });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}
fn str_replace(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let from = args.get(0).map(|v| crate::value::value_to_debug_string(v)).unwrap_or_default();
    let to = args.get(1).map(|v| crate::value::value_to_debug_string(v)).unwrap_or_default();
    Ok(Value::String(Rc::from(s.replacen(&from, &to, 1).as_str())))
}
fn str_includes(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(str_val(vm, &this)?.contains(args.get(0).map(|v| crate::value::value_to_debug_string(v)).unwrap_or_default().as_str())))
}
fn str_starts_with(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(str_val(vm, &this)?.starts_with(args.get(0).map(|v| crate::value::value_to_debug_string(v)).unwrap_or_default().as_str())))
}
fn str_ends_with(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(str_val(vm, &this)?.ends_with(args.get(0).map(|v| crate::value::value_to_debug_string(v)).unwrap_or_default().as_str())))
}
fn str_repeat(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let n = args.get(0).and_then(|v| if let Value::Number(n)=v {Some(*n as usize)} else {None}).unwrap_or(0);
    Ok(Value::String(Rc::from(str_val(vm, &this)?.repeat(n).as_str())))
}
fn str_from_char_code(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let s: String = args.iter().filter_map(|v| if let Value::Number(n)=v { char::from_u32(*n as u32) } else { None }).collect();
    Ok(Value::String(Rc::from(s.as_str())))
}
fn string_constructor(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(vm.to_string(args.get(0).unwrap_or(&Value::Undefined))?))
}
fn number_constructor(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    Ok(Value::Number(vm.to_number(args.get(0).unwrap_or(&Value::Undefined))?))
}
fn boolean_constructor(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(args.get(0).unwrap_or(&Value::Undefined).is_truthy()))
}

// =========================================================================
// Extended setup
// =========================================================================
pub fn setup_full(vm: &mut Vm) {
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
    // Array
    let (array_ctor, array_proto) = make_builtin_constructor(vm, "Array", &[
        ("push", array_push, 1), ("pop", array_pop, 0), ("join", array_join, 1),
        ("map", array_map, 1), ("filter", array_filter, 1), ("reduce", array_reduce, 1),
        ("forEach", array_for_each, 1), ("indexOf", array_index_of, 1),
        ("includes", array_includes, 1), ("slice", array_slice, 2), ("concat", array_concat, 1),
        ("find", array_find, 1), ("findIndex", array_find_index, 1), ("findLast", array_find_last, 1),
        ("fill", array_fill, 1), ("some", array_some, 1), ("every", array_every, 1),
    ]);
    // override the constructor function to use array_constructor
    vm.array_proto = Value::Object(array_proto);
    define_global(vm, "Array", Value::Object(array_ctor));
    // String
    let (str_ctor, str_proto) = make_builtin_constructor(vm, "String", &[
        ("charAt", str_char_at, 1), ("charCodeAt", str_char_code_at, 1),
        ("indexOf", str_index_of, 1), ("slice", str_slice, 2),
        ("toUpperCase", str_to_upper, 0), ("toLowerCase", str_to_lower, 0),
        ("trim", str_trim, 0), ("split", str_split, 1), ("replace", str_replace, 2),
        ("includes", str_includes, 1), ("startsWith", str_starts_with, 1),
        ("endsWith", str_ends_with, 1), ("repeat", str_repeat, 1),
    ]);
    vm.string_proto = Value::Object(str_proto);
    define_global(vm, "String", Value::Object(str_ctor));
    // Number
    let (num_ctor, num_proto) = make_builtin_constructor(vm, "Number", &[]);
    vm.number_proto = Value::Object(num_proto);
    define_global(vm, "Number", Value::Object(num_ctor));
    // Boolean
    let (bool_ctor, bool_proto) = make_builtin_constructor(vm, "Boolean", &[]);
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
    define_global(vm, "NaN", Value::Number(f64::NAN));
    define_global(vm, "Infinity", Value::Number(f64::INFINITY));
    define_global(vm, "undefined", Value::Undefined);
    setup_collections(vm);
}

// =========================================================================
// Map
// =========================================================================
fn map_set(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.get(0).cloned().unwrap_or(Value::Undefined);
    let val = args.get(1).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                let mut entries = m.entries.borrow_mut();
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
    let key = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries.borrow().iter().find(|(k, _)| k == &key).map(|(_, v)| v.clone()).unwrap_or(Value::Undefined)
            } else { Value::Undefined }
        }));
    }
    Ok(Value::Undefined)
}
fn map_has(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj { m.entries.borrow().iter().any(|(k, _)| k == &key) } else { false }
        })));
    }
    Ok(Value::Bool(false))
}
fn map_delete(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                let mut entries = m.entries.borrow_mut();
                let len = entries.len();
                entries.retain(|(k, _)| k != &key);
                entries.len() != len
            } else { false }
        })));
    }
    Ok(Value::Bool(false))
}
fn map_clear(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj { m.entries.borrow_mut().clear(); }
        });
    }
    Ok(Value::Undefined)
}
fn map_size(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Number(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj { m.entries.borrow().len() } else { 0 }
        }) as f64));
    }
    Ok(Value::Number(0.0))
}
fn map_constructor(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let obj = HeapObj::Map(MapData {
        entries: RefCell::new(Vec::new()),
        props: RefCell::new(HashMap::new()),
        proto: RefCell::new(Some(vm.map_proto.clone())),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj))))
}

// =========================================================================
// Set
// =========================================================================
fn set_add(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj {
                let mut items = s.items.borrow_mut();
                if !items.iter().any(|i| i == &val) { items.push(val); }
            }
        });
    }
    Ok(this.unwrap_or(Value::Undefined))
}
fn set_has(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj { s.items.borrow().iter().any(|i| i == &val) } else { false }
        })));
    }
    Ok(Value::Bool(false))
}
fn set_delete(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj {
                let mut items = s.items.borrow_mut();
                let len = items.len();
                items.retain(|i| i != &val);
                items.len() != len
            } else { false }
        })));
    }
    Ok(Value::Bool(false))
}
fn set_size(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Number(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj { s.items.borrow().len() } else { 0 }
        }) as f64));
    }
    Ok(Value::Number(0.0))
}
fn set_constructor(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let obj = HeapObj::Set(SetData {
        items: RefCell::new(Vec::new()),
        props: RefCell::new(HashMap::new()),
        proto: RefCell::new(Some(vm.set_proto.clone())),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj))))
}

// =========================================================================
// Symbol
// =========================================================================
fn symbol_constructor(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let _desc = args.get(0).cloned().unwrap_or(Value::Undefined);
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
    Ok(Value::String(Rc::from("Symbol()")))
}

// =========================================================================
// Extended setup 2: Map/Set/Symbol
// =========================================================================
pub fn setup_collections(vm: &mut Vm) {
    // Map
    let (map_ctor, map_proto) = make_builtin_constructor_with(vm, "Map", map_constructor, &[
        ("set", map_set, 2), ("get", map_get, 1), ("has", map_has, 1),
        ("delete", map_delete, 1), ("clear", map_clear, 0), ("size", map_size, 0),
    ]);
    vm.map_proto = Value::Object(map_proto);
    define_global(vm, "Map", Value::Object(map_ctor));
    // Set
    let (set_ctor, set_proto) = make_builtin_constructor_with(vm, "Set", set_constructor, &[
        ("add", set_add, 1), ("has", set_has, 1), ("delete", set_delete, 1), ("size", set_size, 0),
    ]);
    vm.set_proto = Value::Object(set_proto);
    define_global(vm, "Set", Value::Object(set_ctor));
    // Symbol
    let sym_idx = vm.new_native_function("Symbol", symbol_constructor, 1);
    define_global(vm, "Symbol", Value::Object(sym_idx));
    let sym_for_idx = vm.new_native_function("for", symbol_for, 1);
    if let Value::Object(idx) = Value::Object(sym_idx) {
        vm.heap.with_obj(idx.0, |obj| {
            obj.props().borrow_mut().insert(Rc::from("for"), data_prop(Value::Object(sym_for_idx)));
            obj.props().borrow_mut().insert(Rc::from("iterator"), data_prop(Value::Symbol(vm.well_known_symbols.iterator)));
        });
    }
    // Symbol.prototype: a plain Object with a toString method. Symbol is a
    // value type (not a constructor), so build the proto manually rather than
    // going through make_builtin_constructor.
    let sym_tostring_idx = vm.new_native_function("toString", symbol_to_string, 0);
    let mut sym_proto_props: HashMap<Rc<str>, PropertyDescriptor> = HashMap::new();
    sym_proto_props.insert(Rc::from("toString"), data_prop(Value::Object(sym_tostring_idx)));
    sym_proto_props.insert(Rc::from("constructor"), data_prop(Value::Object(sym_idx)));
    let sym_proto_obj = HeapObj::Object(ObjectData {
        props: RefCell::new(sym_proto_props),
        proto: RefCell::new(Some(vm.object_proto.clone())),
        extensible: Cell::new(true),
        class_name: Some(Rc::from("Symbol")),
    });
    let sym_proto_idx = GcIdx(vm.heap.allocate(sym_proto_obj));
    vm.symbol_proto = Value::Object(sym_proto_idx);
}

fn make_builtin_constructor_with(vm: &mut Vm, name: &str, ctor: NativeFn, methods: &[(&str, NativeFn, usize)]) -> (GcIdx, GcIdx) {
    let mut method_props: HashMap<Rc<str>, PropertyDescriptor> = HashMap::new();
    for (n, f, len) in methods {
        let func_idx = vm.new_native_function(n, *f, *len);
        method_props.insert(Rc::from(*n), data_prop(Value::Object(func_idx)));
    }
    let proto_obj = HeapObj::Object(ObjectData {
        props: RefCell::new(method_props),
        proto: RefCell::new(Some(vm.object_proto.clone())),
        extensible: Cell::new(true),
        class_name: Some(Rc::from(name)),
    });
    let proto_idx = GcIdx(vm.heap.allocate(proto_obj));
    let ctor_func = FunctionData {
        name: Some(Rc::from(name)),
        kind: FunctionKind::Native { func: ctor, length: 0 },
        closure: vm.global,
        prototype: RefCell::new(Some(Value::Object(proto_idx))),
        props: RefCell::new(HashMap::new()),
    };
    let ctor_idx = GcIdx(vm.heap.allocate(HeapObj::Function(ctor_func)));
    vm.heap.with_obj(ctor_idx.0, |obj| {
        obj.props().borrow_mut().insert(Rc::from("prototype"), data_prop(Value::Object(proto_idx)));
    });
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props().borrow_mut().insert(Rc::from("constructor"), data_prop(Value::Object(ctor_idx)));
    });
    (ctor_idx, proto_idx)
}
