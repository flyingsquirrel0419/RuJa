//! Built-in objects and globals for the RuJa VM (v2.0).
//!
//! All built-in constructors, prototypes, and global functions are registered
//! here. Native functions follow the `NativeFn` signature used by the VM.

use crate::environment as env;
use crate::error::{self, Error};
use crate::gc::Heap;
use crate::value::{
    ArrayData, BindingKind, FunctionData, FunctionKind, GcIdx, HeapObj, MapData, ObjectData,
    PropertyDescriptor, SetData, Value,
};
use crate::vm::{NativeFn, Vm};
use indexmap::IndexMap;
use regex::Regex;
use std::cell::{Cell, RefCell};

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
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(proto),
        extensible: Cell::new(true),
        class_name: Some(Rc::from("Object")),
    });
    GcIdx(heap.allocate(obj))
}

fn new_object_with_props(heap: &Heap, proto: Option<Value>, props: Vec<(&str, Value)>) -> GcIdx {
    let mut map: IndexMap<Rc<str>, PropertyDescriptor> = IndexMap::new();
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
                props
                    .borrow_mut()
                    .insert(name.clone(), data_prop(func.clone()));
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
                                p.props()
                                    .borrow()
                                    .get("constructor")
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
                }
                name.to_string()
            })
        };
        return Ok(Value::String(Rc::from(
            format!("[object {}]", class).as_str(),
        )));
    }
    Ok(Value::String(Rc::from("[object Object]")))
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

    let mut method_props: IndexMap<Rc<str>, PropertyDescriptor> = IndexMap::new();
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
        kind: FunctionKind::Native {
            func: object_constructor,
            length: 1,
        },
        closure: vm.global,
        prototype: RefCell::new(Some(Value::Object(proto_idx))),
        props: RefCell::new(IndexMap::new()),
    };
    let ctor_idx = GcIdx(vm.heap.allocate(HeapObj::Function(ctor_func)));
    // constructor.prototype
    vm.heap.with_obj(ctor_idx.0, |obj| {
        obj.props()
            .borrow_mut()
            .insert(Rc::from("prototype"), data_prop(Value::Object(proto_idx)));
    });
    // prototype.constructor
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props()
            .borrow_mut()
            .insert(Rc::from("constructor"), data_prop(Value::Object(ctor_idx)));
    });

    (ctor_idx, proto_idx)
}

fn make_error_constructor(vm: &mut Vm, name: &str) -> (GcIdx, GcIdx) {
    let error_proto_val = vm.error_proto.clone();
    let proto_obj = HeapObj::Object(ObjectData {
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(Some(error_proto_val.clone())),
        extensible: Cell::new(true),
        class_name: Some(Rc::from(name)),
    });
    let proto_idx = GcIdx(vm.heap.allocate(proto_obj));

    let ctor_func = FunctionData {
        name: Some(Rc::from(name)),
        kind: FunctionKind::Native {
            func: error_constructor,
            length: 1,
        },
        closure: vm.global,
        prototype: RefCell::new(Some(Value::Object(proto_idx))),
        props: RefCell::new(IndexMap::new()),
    };
    let ctor_idx = GcIdx(vm.heap.allocate(HeapObj::Function(ctor_func)));
    vm.heap.with_obj(ctor_idx.0, |obj| {
        obj.props()
            .borrow_mut()
            .insert(Rc::from("prototype"), data_prop(Value::Object(proto_idx)));
    });
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props()
            .borrow_mut()
            .insert(Rc::from("constructor"), data_prop(Value::Object(ctor_idx)));
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
    if n.is_nan() {
        return Ok(0);
    }
    if n.is_infinite() || n > len as f64 {
        return Ok(len);
    }
    if n < 0.0 {
        return Ok(0);
    }
    Ok(n as usize)
}

fn to_relative_index(len: usize, value: &Value, vm: &mut Vm) -> error::Result<usize> {
    let n = vm.to_number(value)?;
    if n.is_nan() {
        return Ok(0);
    }
    let idx = n as isize;
    if idx < 0 {
        Ok((len as isize + idx).max(0) as usize)
    } else {
        Ok(idx as usize)
    }
}

fn to_length(vm: &mut Vm, value: &Value) -> error::Result<usize> {
    let n = vm.to_number(value)?;
    if n.is_nan() || n <= 0.0 {
        return Ok(0);
    }
    if n.is_infinite() {
        return Ok(usize::MAX);
    }
    if n > usize::MAX as f64 {
        return Ok(usize::MAX);
    }
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
        Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Symbol(_) => {
            vm.to_object(first)
        }
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
                        if key == "length" {
                            return true;
                        }
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
            if key == "length" {
                return Ok(Value::Bool(true));
            }
            if let Ok(i) = key.parse::<usize>() {
                return Ok(Value::Bool(i < s.chars().count()));
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

/// Collect an object's own enumerable string keys in array-index-first then property order.
fn own_string_keys(vm: &mut Vm, obj: &Value) -> Vec<Rc<str>> {
    let mut keys = Vec::new();
    if let Value::Object(idx) = obj {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Array(a) = o {
                for i in 0..a.items.borrow().len() {
                    keys.push(Rc::from(i.to_string().as_str()));
                }
            }
            if let HeapObj::Map(m) = o {
                for (k, _) in m.entries.borrow().iter() {
                    if let Value::String(s) = k {
                        keys.push(s.clone());
                    }
                }
            }
            for (k, desc) in o.props().borrow().iter() {
                if desc.enumerable {
                    keys.push(k.clone());
                }
            }
        });
    }
    keys
}

fn make_value_array(vm: &mut Vm, items: Vec<Value>) -> Value {
    let arr = HeapObj::Array(ArrayData {
        items: RefCell::new(items),
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(Some(vm.array_proto.clone())),
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

fn make_str_array(vm: &mut Vm, strs: Vec<Rc<str>>) -> Value {
    let items: Vec<Value> = strs.into_iter().map(Value::String).collect();
    let arr = HeapObj::Array(ArrayData {
        items: RefCell::new(items),
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(Some(vm.array_proto.clone())),
    });
    Value::Object(GcIdx(vm.heap.allocate(arr)))
}

fn object_keys(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let obj = args.get(0).cloned().unwrap_or(Value::Undefined);
    let keys = own_string_keys(vm, &obj);
    Ok(make_str_array(vm, keys))
}

fn object_values(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let obj = args.get(0).cloned().unwrap_or(Value::Undefined);
    let mut vals = Vec::new();
    if let Value::Object(idx) = &obj {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Array(a) = o {
                vals.extend(a.items.borrow().clone());
            }
            if let HeapObj::Map(m) = o {
                for (_, v) in m.entries.borrow().iter() {
                    vals.push(v.clone());
                }
            }
            for (_k, desc) in o.props().borrow().iter() {
                if desc.enumerable {
                    vals.push(desc.value.clone());
                }
            }
        });
    }
    let arr = HeapObj::Array(ArrayData {
        items: RefCell::new(vals),
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(Some(vm.array_proto.clone())),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}

fn object_entries(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let obj = args.get(0).cloned().unwrap_or(Value::Undefined);
    let keys = own_string_keys(vm, &obj);
    let mut pairs = Vec::new();
    for k in keys {
        let v = vm.get_property(&obj, &k)?;
        let pair = HeapObj::Array(ArrayData {
            items: RefCell::new(vec![Value::String(k.clone()), v]),
            props: RefCell::new(IndexMap::new()),
            proto: RefCell::new(Some(vm.array_proto.clone())),
        });
        pairs.push(Value::Object(GcIdx(vm.heap.allocate(pair))));
    }
    let arr = HeapObj::Array(ArrayData {
        items: RefCell::new(pairs),
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(Some(vm.array_proto.clone())),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}

fn object_assign(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    for src in &args[1..] {
        let keys = own_string_keys(vm, src);
        for k in keys {
            let v = vm.get_property(src, &k)?;
            vm.set_property(&target, &k, v)?;
        }
    }
    Ok(target)
}

fn object_is_prototype_of(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let proto_val = this.unwrap_or(Value::Undefined);
    let candidate = args.get(0).cloned().unwrap_or(Value::Undefined);
    if proto_val.is_nullish() || candidate.is_nullish() {
        return Ok(Value::Bool(false));
    }
    if let (Value::Object(pidx), Value::Object(cidx)) = (&proto_val, &candidate) {
        let mut cur = cidx.clone();
        loop {
            let proto = vm.heap.with_obj(cur.0, |obj| obj.proto().borrow().clone());
            match proto {
                Some(Value::Object(next)) => {
                    if next == *pidx {
                        return Ok(Value::Bool(true));
                    }
                    cur = next;
                }
                _ => return Ok(Value::Bool(false)),
            }
        }
    }
    Ok(Value::Bool(false))
}

fn object_is(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let a = args.get(0).cloned().unwrap_or(Value::Undefined);
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
    let entries = args.get(0).cloned().unwrap_or(Value::Undefined);
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(Some(vm.object_proto.clone())),
        extensible: Cell::new(true),
        class_name: None,
    }));
    if let Value::Object(arr_idx) = &entries {
        let items: Vec<Value> = vm.heap.with_obj(arr_idx.0, |o| {
            if let HeapObj::Array(a) = o {
                a.items.borrow().clone()
            } else {
                Vec::new()
            }
        });
        for pair in &items {
            if let Value::Object(pi) = pair {
                let (k, v) = vm.heap.with_obj(pi.0, |o| {
                    if let HeapObj::Array(a) = o {
                        let it = a.items.borrow();
                        (
                            it.get(0).cloned().unwrap_or(Value::Undefined),
                            it.get(1).cloned().unwrap_or(Value::Undefined),
                        )
                    } else {
                        (Value::Undefined, Value::Undefined)
                    }
                });
                let key = vm.to_property_key(&k)?;
                vm.heap.with_obj(obj_idx, |o| {
                    if let HeapObj::Object(obj) = o {
                        obj.props
                            .borrow_mut()
                            .insert(Rc::from(key.as_str()), data_prop(v));
                    }
                });
            }
        }
    }
    Ok(Value::Object(GcIdx(obj_idx)))
}
fn object_create(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let proto = args.get(0).cloned().unwrap_or(Value::Undefined);
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(if proto.is_null() { None } else { Some(proto) }),
        extensible: Cell::new(true),
        class_name: None,
    }));
    Ok(Value::Object(GcIdx(obj_idx)))
}
fn object_get_own_property_names(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let obj = args.get(0).cloned().unwrap_or(Value::Undefined);
    let keys = own_string_keys(vm, &obj);
    Ok(make_str_array(vm, keys))
}
fn object_get_own_property_descriptor(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let obj = args.get(0).cloned().unwrap_or(Value::Undefined);
    let key = match args.get(1) {
        Some(v) => vm.to_property_key(v)?,
        None => return Ok(Value::Undefined),
    };
    if let Value::Object(idx) = &obj {
        let desc = vm
            .heap
            .with_obj(idx.0, |o| o.props().borrow().get(key.as_str()).cloned());
        if let Some(d) = desc {
            let desc_obj = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
                props: RefCell::new(IndexMap::new()),
                proto: RefCell::new(Some(vm.object_proto.clone())),
                extensible: Cell::new(true),
                class_name: None,
            }));
            let mut p = IndexMap::new();
            if d.is_accessor {
                p.insert(
                    Rc::from("get"),
                    data_prop(d.get.clone().unwrap_or(Value::Undefined)),
                );
                p.insert(
                    Rc::from("set"),
                    data_prop(d.set.clone().unwrap_or(Value::Undefined)),
                );
            } else {
                p.insert(Rc::from("value"), data_prop(d.value.clone()));
                p.insert(Rc::from("writable"), data_prop(Value::Bool(d.writable)));
            }
            p.insert(Rc::from("enumerable"), data_prop(Value::Bool(d.enumerable)));
            p.insert(
                Rc::from("configurable"),
                data_prop(Value::Bool(d.configurable)),
            );
            vm.heap.with_obj(desc_obj, |o| {
                if let HeapObj::Object(od) = o {
                    *od.props.borrow_mut() = p;
                }
            });
            return Ok(Value::Object(GcIdx(desc_obj)));
        }
    }
    Ok(Value::Undefined)
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

fn object_define_property(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
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
        let mut is_data = false;
        if let Value::Object(_didx) = desc {
            if let Some(v) = vm.get_property(&desc, "value").ok() {
                value = v;
                is_data = true;
            }
            if let Some(v) = vm.get_property(&desc, "writable").ok() {
                writable = v.is_truthy();
                is_data = true;
            }
            if let Some(v) = vm.get_property(&desc, "enumerable").ok() {
                enumerable = v.is_truthy();
            }
            if let Some(v) = vm.get_property(&desc, "configurable").ok() {
                configurable = v.is_truthy();
            }
        }
        let descriptor = if is_data {
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
                .borrow_mut()
                .insert(Rc::from(key.as_str()), descriptor);
        });
    }
    Ok(target)
}

fn object_define_properties(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    let props = args.get(1).cloned().unwrap_or(Value::Undefined);
    if let (Value::Object(_), Value::Object(_)) = (&target, &props) {
        let keys = object_keys(vm, &[props.clone()], None)?;
        if let Value::Object(kidx) = keys {
            let key_objs = vm.heap.with_obj(kidx.0, |obj| {
                if let HeapObj::Array(a) = obj {
                    a.items.borrow().clone()
                } else {
                    Vec::new()
                }
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

fn error_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let msg = args
        .get(0)
        .map(|v| vm.to_string(v).unwrap_or_else(|_| Rc::from("")))
        .unwrap_or_else(|| Rc::from(""));
    // Use the `this` provided by `construct` (already linked to <Error>.prototype).
    let idx = match this {
        Some(Value::Object(i)) => i,
        _ => vm.new_object(),
    };
    vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::Object(o) = obj {
            o.props
                .borrow_mut()
                .insert(Rc::from("message"), data_prop(Value::String(msg)));
            o.props.borrow_mut().insert(
                Rc::from("name"),
                data_prop(Value::String(Rc::from("Error"))),
            );
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
    ] {
        let m = vm.new_native_function(n, f, len);
        vm.heap.with_obj(object_ctor.0, |obj| {
            obj.props()
                .borrow_mut()
                .insert(Rc::from(n), data_prop(Value::Object(m)));
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
    let n = vm.to_number(args.get(0).unwrap_or(&Value::Undefined))?;
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
    let n = vm.to_number(args.get(0).unwrap_or(&Value::Undefined))?;
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
    let y = vm.to_number(args.get(0).unwrap_or(&Value::Undefined))?;
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
    let n = vm.to_number(args.get(0).unwrap_or(&Value::Undefined))? as u32;
    Ok(Value::Number(n.leading_zeros() as f64))
}
fn math_fround(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let n = vm.to_number(args.get(0).unwrap_or(&Value::Undefined))?;
    Ok(Value::Number(n as f32 as f64))
}
fn math_trunc2(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    math_unary(f64::trunc, vm, args)
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
    let a = vm.to_number(args.get(0).unwrap_or(&Value::Undefined))?;
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
    thread_local! { static STATE: Cell<u64> = Cell::new(0x2545F4914F6CDD1D); }
    let r = STATE.with(|s| {
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        x as f64 / u64::MAX as f64
    });
    Ok(Value::Number(r))
}

fn build_math(vm: &mut Vm) -> Value {
    let mut props: IndexMap<Rc<str>, PropertyDescriptor> = IndexMap::new();
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
        ("cos", math_cos, 1),
        ("tan", math_tan, 1),
        ("pow", math_pow, 2),
        ("max", math_max, 2),
        ("min", math_min, 2),
        ("random", math_random, 0),
    ];
    for (name, f, len) in method_entries.drain(..) {
        let idx = vm.new_native_function(name, f, len);
        props.insert(Rc::from(name), data_prop(Value::Object(idx)));
    }
    props.insert(
        Rc::from("PI"),
        data_prop(Value::Number(std::f64::consts::PI)),
    );
    props.insert(Rc::from("E"), data_prop(Value::Number(std::f64::consts::E)));
    props.insert(
        Rc::from("LN2"),
        data_prop(Value::Number(std::f64::consts::LN_2)),
    );
    props.insert(
        Rc::from("LN10"),
        data_prop(Value::Number(std::f64::consts::LN_10)),
    );
    props.insert(
        Rc::from("LOG2E"),
        data_prop(Value::Number(std::f64::consts::LOG2_E)),
    );
    props.insert(
        Rc::from("LOG10E"),
        data_prop(Value::Number(std::f64::consts::LOG10_E)),
    );
    props.insert(
        Rc::from("SQRT2"),
        data_prop(Value::Number(std::f64::consts::SQRT_2)),
    );
    props.insert(
        Rc::from("SQRT1_2"),
        data_prop(Value::Number(std::f64::consts::FRAC_1_SQRT_2)),
    );
    let obj = HeapObj::Object(ObjectData {
        props: RefCell::new(props),
        proto: RefCell::new(Some(vm.object_proto.clone())),
        extensible: Cell::new(false),
        class_name: Some(Rc::from("Math")),
    });
    Value::Object(GcIdx(vm.heap.allocate(obj)))
}

// =========================================================================
// console
// =========================================================================
fn console_log(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let parts: Vec<String> = args
        .iter()
        .map(|a| vm.to_string(a).map(|s| s.to_string()).unwrap_or_default())
        .collect();
    println!("{}", parts.join(" "));
    Ok(Value::Undefined)
}
fn build_console(vm: &mut Vm) -> Value {
    let mut props: IndexMap<Rc<str>, PropertyDescriptor> = IndexMap::new();
    for name in &["log", "error", "warn", "info", "debug", "dir", "trace"] {
        let idx = vm.new_native_function(name, console_log, 0);
        props.insert(Rc::from(*name), data_prop(Value::Object(idx)));
    }
    let obj = HeapObj::Object(ObjectData {
        props: RefCell::new(props),
        proto: RefCell::new(Some(vm.object_proto.clone())),
        extensible: Cell::new(true),
        class_name: Some(Rc::from("Object")),
    });
    Value::Object(GcIdx(vm.heap.allocate(obj)))
}

// =========================================================================
// JSON
// =========================================================================
fn json_stringify(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let v = args.get(0).unwrap_or(&Value::Undefined);
    // Reject circular references per ECMAScript (TypeError).
    if let Value::Object(_) = v {
        if has_json_cycle(vm, v, &mut Vec::new()) {
            return Err(Error::type_err(
                "Converting circular structure to JSON".to_string(),
            ));
        }
    }
    match stringify_value(vm, v, &mut Vec::new()) {
        Some(s) => Ok(Value::String(Rc::from(s.as_str()))),
        None => Ok(Value::Undefined),
    }
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
        HeapObj::Array(a) => a.items.borrow().clone(),
        HeapObj::Object(o) => o
            .props
            .borrow()
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
fn stringify_value(vm: &mut Vm, v: &Value, seen: &mut Vec<usize>) -> Option<String> {
    match v {
        Value::Undefined => None,
        Value::Null => Some("null".into()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => {
            if n.is_nan() || n.is_infinite() {
                None
            } else {
                Some(crate::value::num_to_string(*n))
            }
        }
        Value::String(s) => Some(format!(
            "\"{}\"",
            s.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\t', "\\t")
        )),
        Value::Symbol(_) => None,
        Value::Object(idx) => {
            // Detect circular references; throw TypeError per ECMAScript.
            if seen.contains(&idx.0) {
                return None;
            }
            seen.push(idx.0);
            let (is_arr, items, props, proto) = vm.heap.with_obj(idx.0, |obj| match obj {
                HeapObj::Array(a) => (true, a.items.borrow().clone(), IndexMap::new(), None),
                HeapObj::Object(o) => (
                    false,
                    Vec::new(),
                    o.props.borrow().clone(),
                    o.proto.borrow().clone(),
                ),
                HeapObj::Function(_) => (false, Vec::new(), IndexMap::new(), None),
                _ => (
                    false,
                    Vec::new(),
                    obj.props().borrow().clone(),
                    obj.proto().borrow().clone(),
                ),
            });
            if is_arr {
                let parts: Vec<String> = items
                    .iter()
                    .filter_map(|i| stringify_value(vm, i, seen))
                    .collect();
                seen.pop();
                Some(format!("[{}]", parts.join(",")))
            } else {
                let _ = proto;
                let mut pairs = Vec::new();
                for (k, d) in &props {
                    if !d.enumerable {
                        continue;
                    }
                    if let Some(vs) = stringify_value(vm, &d.value, seen) {
                        pairs.push(format!("\"{}\":{}", k, vs));
                    }
                }
                seen.pop();
                Some(format!("{{{}}}", pairs.join(",")))
            }
        }
    }
}
fn json_parse(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let s = match args.get(0) {
        Some(Value::String(s)) => s.to_string(),
        _ => return Ok(Value::Null),
    };
    parse_json_value(vm, &mut s.chars().peekable())
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
    let mut props: IndexMap<Rc<str>, PropertyDescriptor> = IndexMap::new();
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
            chars.next();
        }
        chars.next();
        let val = parse_json_value(vm, chars)?;
        // JSON-parsed properties are enumerable (data_prop is non-enumerable for builtins).
        let mut desc = data_prop(val);
        desc.enumerable = true;
        props.insert(Rc::from(key.as_str()), desc);
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
        props: RefCell::new(props),
        proto: RefCell::new(Some(vm.object_proto.clone())),
        extensible: Cell::new(true),
        class_name: None,
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
        items: RefCell::new(items),
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(Some(vm.array_proto.clone())),
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
    Ok(Value::String(Rc::from(s.as_str())))
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
fn build_json(vm: &mut Vm) -> Value {
    let mut props: IndexMap<Rc<str>, PropertyDescriptor> = IndexMap::new();
    let pi = vm.new_native_function("parse", json_parse, 1);
    let si = vm.new_native_function("stringify", json_stringify, 3);
    props.insert(Rc::from("parse"), data_prop(Value::Object(pi)));
    props.insert(Rc::from("stringify"), data_prop(Value::Object(si)));
    let obj = HeapObj::Object(ObjectData {
        props: RefCell::new(props),
        proto: RefCell::new(Some(vm.object_proto.clone())),
        extensible: Cell::new(true),
        class_name: Some(Rc::from("JSON")),
    });
    Value::Object(GcIdx(vm.heap.allocate(obj)))
}

// =========================================================================
// Global functions
// =========================================================================
fn global_parse_int(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let input = match args.get(0) {
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
    let valid = |c: char| c.to_digit(radix).is_some();
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
    let s = match args.get(0) {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(v) => vm.to_string(v)?.to_string(),
        None => return Ok(Value::Number(f64::NAN)),
    };
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

fn array_from(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let src_val = args.get(0).cloned().unwrap_or(Value::Undefined);
    let map_fn = args.get(1).cloned();
    // Array-like or iterable
    let mut items: Vec<Value> = Vec::new();
    if let Value::Object(idx) = &src_val {
        let (is_arr, arr_items, len) = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Array(a) = o {
                (true, a.items.borrow().clone(), 0)
            } else if let HeapObj::Iterator(_) = o {
                (false, Vec::new(), 0)
            } else {
                let len = o
                    .props()
                    .borrow()
                    .get("length")
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
            items.push(Value::String(Rc::from(ch.to_string().as_str())));
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
        args.get(0).unwrap_or(&Value::Undefined),
        &vm.heap,
    )))
}
fn array_push(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow_mut().extend_from_slice(args);
            }
        });
        let len = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().len()
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
                a.items.borrow_mut().pop().unwrap_or(Value::Undefined)
            } else {
                Value::Undefined
            }
        }));
    }
    Ok(Value::Undefined)
}
fn array_join(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let sep = match args.get(0) {
        Some(Value::String(s)) => s.to_string(),
        Some(v) if !v.is_undefined() => vm.to_string(v)?.to_string(),
        _ => ",".to_string(),
    };
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
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
        return Ok(Value::String(Rc::from(parts.join(&sep).as_str())));
    }
    Ok(Value::String(Rc::from("")))
}
fn array_map(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
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
                Some(Value::Undefined),
            )?);
        }
        let arr = HeapObj::Array(ArrayData {
            items: RefCell::new(result),
            props: RefCell::new(IndexMap::new()),
            proto: RefCell::new(Some(vm.array_proto.clone())),
        });
        return Ok(Value::Object(GcIdx(vm.heap.allocate(arr))));
    }
    Ok(Value::Undefined)
}
fn array_filter(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
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
                Some(Value::Undefined),
            )?;
            if keep.is_truthy() {
                result.push(item.clone());
            }
        }
        let arr = HeapObj::Array(ArrayData {
            items: RefCell::new(result),
            props: RefCell::new(IndexMap::new()),
            proto: RefCell::new(Some(vm.array_proto.clone())),
        });
        return Ok(Value::Object(GcIdx(vm.heap.allocate(arr))));
    }
    Ok(Value::Undefined)
}
fn array_reduce(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
            } else {
                Vec::new()
            }
        });
        let (mut acc, start) = if args.len() >= 2 {
            (args[1].clone(), 0)
        } else {
            (items.get(0).cloned().unwrap_or(Value::Undefined), 1)
        };
        for i in start..items.len() {
            acc = vm.call_function(
                &cb,
                &[
                    acc,
                    items[i].clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                Some(Value::Undefined),
            )?;
        }
        return Ok(acc);
    }
    Ok(Value::Undefined)
}
fn array_for_each(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
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
                Some(Value::Undefined),
            )?;
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
            } else {
                None
            }
        });
        return Ok(Value::Number(pos.map(|i| i as f64).unwrap_or(-1.0)));
    }
    Ok(Value::Number(-1.0))
}
fn array_includes(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let found = vm.heap.with_obj(idx.0, |obj| {
            // includes uses SameValueZero: NaN matches NaN (unlike indexOf's ===).
            if let HeapObj::Array(a) = obj {
                a.items.borrow().iter().any(|i| {
                    if let (Value::Number(x), Value::Number(y)) = (i, &target) {
                        x.is_nan() && y.is_nan() || x == y
                    } else {
                        i == &target
                    }
                })
            } else {
                false
            }
        });
        return Ok(Value::Bool(found));
    }
    Ok(Value::Bool(false))
}
fn array_slice(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len() as i64;
        let start = args
            .get(0)
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
            items: RefCell::new(sliced),
            props: RefCell::new(IndexMap::new()),
            proto: RefCell::new(Some(vm.array_proto.clone())),
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
                a.items.borrow().clone()
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
                        a.items.borrow().clone()
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
        items: RefCell::new(items),
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(Some(vm.array_proto.clone())),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}

fn array_reverse(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow_mut().reverse();
            }
        });
        return Ok(Value::Object(idx));
    }
    Ok(Value::Undefined)
}

fn array_sort(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cmp = args.get(0).cloned();
    if let Some(Value::Object(idx)) = this {
        // Collect items, sort via comparator (default: cast to string, UTF-16 code unit compare).
        let mut items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
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
                        let ord = vm.to_number(&r)? as i64;
                        if ord > 0 {
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
                *a.items.borrow_mut() = items;
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
                let mut items = a.items.borrow_mut();
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
                let mut items = a.items.borrow_mut();
                for (i, v) in args.iter().enumerate() {
                    items.insert(i, v.clone());
                }
            }
        });
        let len = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().len()
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
                a.items.borrow().clone()
            } else {
                Vec::new()
            }
        });
        let len = items_clone.len() as f64;
        let start = match args.get(0) {
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
                let mut items = a.items.borrow_mut();
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
    let target = args.get(0).unwrap_or(&Value::Undefined).clone();
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
            } else {
                Vec::new()
            }
        });
        for (i, v) in items.iter().enumerate().rev() {
            if vm.strict_eq(v, &target) {
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
                a.items.borrow().clone()
            } else {
                Vec::new()
            }
        });
        let n = match args.get(0) {
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
    let depth = match args.get(0) {
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
                            a.items.borrow().clone()
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
                a.items.borrow().clone()
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
                a.items.borrow().clone()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    let fn_val = args.get(0).cloned().unwrap_or(Value::Undefined);
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
                        a.items.borrow().clone()
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
                a.items.borrow().len()
            } else {
                0
            }
        }) as f64;
        let target = match args.get(0) {
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
                a.items.borrow()[from..from + count].to_vec()
            } else {
                Vec::new()
            }
        });
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                let mut items = a.items.borrow_mut();
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
                a.items.borrow().len()
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
                a.items.borrow().clone()
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
                a.items.borrow().clone()
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

fn array_constructor(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = if args.len() == 1 {
        if let Some(Value::Number(n)) = args.get(0) {
            vec![Value::Undefined; *n as usize]
        } else {
            args.to_vec()
        }
    } else {
        args.to_vec()
    };
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Object(idx));
    }
    let arr = HeapObj::Array(ArrayData {
        items: RefCell::new(items),
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(Some(vm.array_proto.clone())),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}

fn array_find(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
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
                Some(Value::Undefined),
            )?;
            if found.is_truthy() {
                return Ok(item.clone());
            }
        }
    }
    Ok(Value::Undefined)
}
fn array_find_index(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
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
                Some(Value::Undefined),
            )?;
            if found.is_truthy() {
                return Ok(Value::Number(i as f64));
            }
        }
    }
    Ok(Value::Number(-1.0))
}
fn array_find_last(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
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
                Some(Value::Undefined),
            )?;
            if found.is_truthy() {
                return Ok(item.clone());
            }
        }
    }
    Ok(Value::Undefined)
}
fn array_fill(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let value = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
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
                    let mut items = a.items.borrow_mut();
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
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
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
                Some(Value::Undefined),
            )?;
            if found.is_truthy() {
                return Ok(Value::Bool(true));
            }
        }
    }
    Ok(Value::Bool(false))
}
fn array_every(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
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
                Some(Value::Undefined),
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
        .get(0)
        .and_then(|v| {
            if let Value::Number(n) = v {
                Some(*n as usize)
            } else {
                None
            }
        })
        .unwrap_or(0);
    Ok(s.chars()
        .nth(i)
        .map(|c| Value::String(Rc::from(c.to_string().as_str())))
        .unwrap_or(Value::String(Rc::from(""))))
}
fn str_char_code_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let i = args
        .get(0)
        .and_then(|v| {
            if let Value::Number(n) = v {
                Some(*n as usize)
            } else {
                None
            }
        })
        .unwrap_or(0);
    Ok(s.chars()
        .nth(i)
        .map(|c| Value::Number(c as u32 as f64))
        .unwrap_or(Value::Number(f64::NAN)))
}
fn str_index_of(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let n = args
        .get(0)
        .map(|v| crate::value::value_to_debug_string(v))
        .unwrap_or_default();
    Ok(Value::Number(s.find(&n).map(|i| i as f64).unwrap_or(-1.0)))
}
fn str_slice(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i64;
    let start = args
        .get(0)
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
        (start as usize).min(chars.len())
    };
    let en = if end < 0 {
        (len + end).max(0) as usize
    } else {
        (end as usize).min(chars.len())
    };
    let r: String = if st < en {
        chars[st..en].iter().collect()
    } else {
        String::new()
    };
    Ok(Value::String(Rc::from(r.as_str())))
}
fn str_to_upper(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Rc::from(
        str_val(vm, &this)?.to_uppercase().as_str(),
    )))
}
fn str_to_lower(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Rc::from(
        str_val(vm, &this)?.to_lowercase().as_str(),
    )))
}
fn str_trim(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Rc::from(str_val(vm, &this)?.trim())))
}
fn str_split(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let sep = args.get(0).map(|v| crate::value::value_to_debug_string(v));
    let limit = match args.get(1) {
        Some(Value::Undefined) | None => usize::MAX,
        Some(v) => vm.to_number(v).map(|n| n as usize).unwrap_or(usize::MAX),
    };
    let parts: Vec<String> = match sep {
        None => vec![s],
        Some(sep) if sep.is_empty() => s.chars().take(limit).map(|c| c.to_string()).collect(),
        Some(sep) => s.split(&sep).take(limit).map(|p| p.to_string()).collect(),
    };
    let items: Vec<Value> = parts
        .into_iter()
        .map(|p| Value::String(Rc::from(p.as_str())))
        .collect();
    let arr = HeapObj::Array(ArrayData {
        items: RefCell::new(items),
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(Some(vm.array_proto.clone())),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}
fn str_replace(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let to = match args.get(1) {
        Some(v) => vm.to_string(v)?.to_string(),
        None => "undefined".to_string(),
    };
    // If the search value is a RegExp, use regex replacement.
    if let Some(Value::Object(idx)) = args.get(0) {
        let source = vm.heap.with_obj(idx.0, |o| {
            o.props().borrow().get("source").map(|d| d.value.clone())
        });
        if let Some(Value::String(source)) = source {
            let global = vm.heap.with_obj(idx.0, |o| {
                o.props().borrow().get("global").map(|d| d.value.clone())
            }) == Some(Value::Bool(true));
            let re =
                Regex::new(&source).map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
            let replaced = if global {
                re.replace_all(&s, to.as_str())
            } else {
                re.replace(&s, to.as_str())
            };
            return Ok(Value::String(Rc::from(replaced.as_ref())));
        }
    }
    let from = match args.get(0) {
        Some(v) => vm.to_string(v)?.to_string(),
        None => return Ok(Value::String(Rc::from(s.as_str()))),
    };
    Ok(Value::String(Rc::from(s.replacen(&from, &to, 1).as_str())))
}
fn str_includes(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(
        str_val(vm, &this)?.contains(
            args.get(0)
                .map(|v| crate::value::value_to_debug_string(v))
                .unwrap_or_default()
                .as_str(),
        ),
    ))
}
fn str_starts_with(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(
        str_val(vm, &this)?.starts_with(
            args.get(0)
                .map(|v| crate::value::value_to_debug_string(v))
                .unwrap_or_default()
                .as_str(),
        ),
    ))
}
fn str_ends_with(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(
        str_val(vm, &this)?.ends_with(
            args.get(0)
                .map(|v| crate::value::value_to_debug_string(v))
                .unwrap_or_default()
                .as_str(),
        ),
    ))
}
fn str_repeat(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let n = args
        .get(0)
        .and_then(|v| {
            if let Value::Number(n) = v {
                Some(*n as usize)
            } else {
                None
            }
        })
        .unwrap_or(0);
    Ok(Value::String(Rc::from(
        str_val(vm, &this)?.repeat(n).as_str(),
    )))
}

fn str_match(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    match args.get(0) {
        Some(Value::Object(idx)) => {
            let source = vm.heap.with_obj(idx.0, |o| {
                o.props().borrow().get("source").map(|d| d.value.clone())
            });
            if let Some(Value::String(source)) = source {
                let re = Regex::new(&source)
                    .map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
                match re.captures(&s) {
                    Some(caps) => {
                        let items: Vec<Value> = caps
                            .iter()
                            .map(|c| match c {
                                Some(m) => Value::String(Rc::from(m.as_str())),
                                None => Value::Undefined,
                            })
                            .collect();
                        Ok(make_value_array(vm, items))
                    }
                    None => Ok(Value::Null),
                }
            } else {
                Ok(Value::Null)
            }
        }
        _ => Ok(Value::Null),
    }
}
fn array_find_last_index(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let fn_val = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.borrow().clone()
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
    let target = match args.get(0) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    } as usize;
    let pad = match args.get(1) {
        Some(Value::String(p)) => p.to_string(),
        Some(v) if !v.is_undefined() => vm.to_string(v)?.to_string(),
        _ => " ".to_string(),
    };
    if pad.is_empty() || s.len() >= target {
        return Ok(Value::String(Rc::from(s.as_str())));
    }
    let need = target - s.len();
    let mut out = String::new();
    while out.len() < need {
        out.push_str(&pad);
    }
    out.truncate(need);
    out.push_str(&s);
    Ok(Value::String(Rc::from(out.as_str())))
}
fn str_pad_end(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let target = match args.get(0) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    } as usize;
    let pad = match args.get(1) {
        Some(Value::String(p)) => p.to_string(),
        Some(v) if !v.is_undefined() => vm.to_string(v)?.to_string(),
        _ => " ".to_string(),
    };
    if pad.is_empty() || s.len() >= target {
        return Ok(Value::String(Rc::from(s.as_str())));
    }
    let mut out = s.clone();
    while out.len() < target {
        out.push_str(&pad);
    }
    out.truncate(target);
    Ok(Value::String(Rc::from(out.as_str())))
}
fn str_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let n = match args.get(0) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    } as isize;
    let len = s.chars().count() as isize;
    let idx = if n < 0 { len + n } else { n };
    if idx >= 0 && idx < len {
        let ch = s.chars().nth(idx as usize).unwrap();
        return Ok(Value::String(Rc::from(ch.to_string().as_str())));
    }
    Ok(Value::Undefined)
}
fn str_trim_start(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    Ok(Value::String(Rc::from(s.trim_start())))
}
fn str_trim_end(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    Ok(Value::String(Rc::from(s.trim_end())))
}
fn str_replace_all(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let from = match args.get(0) {
        Some(Value::String(p)) => p.to_string(),
        Some(v) => vm.to_string(v)?.to_string(),
        None => return Ok(Value::String(Rc::from(s.as_str()))),
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
        return Ok(Value::String(Rc::from(out.as_str())));
    }
    Ok(Value::String(Rc::from(s.replace(&from, &to))))
}
fn str_substring(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(vm, &this)?;
    let len = s.chars().count() as f64;
    let mut start = match args.get(0) {
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
    let result: String = s.chars().skip(start).take(end - start).collect();
    Ok(Value::String(Rc::from(result.as_str())))
}

fn str_from_char_code(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let s: String = args
        .iter()
        .filter_map(|v| {
            if let Value::Number(n) = v {
                char::from_u32(*n as u32)
            } else {
                None
            }
        })
        .collect();
    Ok(Value::String(Rc::from(s.as_str())))
}
fn string_constructor(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(
        vm.to_string(args.get(0).unwrap_or(&Value::Undefined))?,
    ))
}
fn number_constructor(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    Ok(Value::Number(
        vm.to_number(args.get(0).unwrap_or(&Value::Undefined))?,
    ))
}

fn number_is_integer(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    match args.get(0) {
        Some(Value::Number(n)) if n.is_finite() && n.fract() == 0.0 => Ok(Value::Bool(true)),
        _ => Ok(Value::Bool(false)),
    }
}
fn number_is_finite(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    match args.get(0) {
        Some(Value::Number(n)) if n.is_finite() => Ok(Value::Bool(true)),
        _ => Ok(Value::Bool(false)),
    }
}
fn number_is_nan(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    match args.get(0) {
        Some(Value::Number(n)) if n.is_nan() => Ok(Value::Bool(true)),
        _ => Ok(Value::Bool(false)),
    }
}
fn number_is_safe_integer(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    match args.get(0) {
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
        return Ok(Value::String(Rc::from("NaN")));
    }
    if !n.is_finite() {
        return Ok(Value::String(Rc::from(if n > 0.0 {
            "Infinity"
        } else {
            "-Infinity"
        })));
    }
    let digits = match args.get(0) {
        Some(v) => vm.to_number(v)? as usize,
        None => 0,
    };
    Ok(Value::String(Rc::from(format!("{:.*}", digits, n))))
}
fn num_proto_to_string(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let n = match &this {
        Some(Value::Number(n)) => *n,
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let radix = match args.get(0) {
        Some(v) => vm.to_number(v)?,
        None => 10.0,
    } as u32;
    if radix == 10 || radix == 0 {
        return Ok(Value::String(Rc::from(
            crate::value::num_to_string(n).as_str(),
        )));
    }
    if radix < 2 || radix > 36 {
        return Err(Error::range(
            "toString() radix must be between 2 and 36".to_string(),
        ));
    }
    if n.fract() == 0.0 {
        let i = n as i64;
        if i >= 0 {
            return Ok(Value::String(Rc::from(format_i64_radix(i, radix).as_str())));
        }
    }
    Ok(Value::String(Rc::from(
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

fn boolean_constructor(_vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(
        args.get(0).unwrap_or(&Value::Undefined).is_truthy(),
    ))
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
    let (array_ctor, array_proto) = make_builtin_constructor(
        vm,
        "Array",
        &[
            ("push", array_push, 1),
            ("pop", array_pop, 0),
            ("join", array_join, 1),
            ("map", array_map, 1),
            ("filter", array_filter, 1),
            ("reduce", array_reduce, 1),
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
                .borrow_mut()
                .insert(Rc::from(n), data_prop(Value::Object(m)));
        });
    }
    // String
    let (str_ctor, str_proto) = make_builtin_constructor(
        vm,
        "String",
        &[
            ("charAt", str_char_at, 1),
            ("charCodeAt", str_char_code_at, 1),
            ("indexOf", str_index_of, 1),
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
        ],
    );
    vm.string_proto = Value::Object(str_proto);
    define_global(vm, "String", Value::Object(str_ctor));
    // Number
    let (num_ctor, num_proto) = make_builtin_constructor(
        vm,
        "Number",
        &[
            ("toFixed", num_to_fixed, 1),
            ("toString", num_proto_to_string, 1),
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
    let mut static_props: Vec<(Rc<str>, Value)> = Vec::new();
    for (name, fnp, len) in statics {
        let idx = vm.new_native_function(name, *fnp, *len);
        static_props.push((Rc::from(*name), Value::Object(idx)));
    }
    static_props.push((
        Rc::from("MAX_SAFE_INTEGER"),
        Value::Number(9007199254740991.0),
    ));
    static_props.push((
        Rc::from("MIN_SAFE_INTEGER"),
        Value::Number(-9007199254740991.0),
    ));
    static_props.push((Rc::from("EPSILON"), Value::Number(f64::EPSILON)));
    static_props.push((Rc::from("MAX_VALUE"), Value::Number(f64::MAX)));
    static_props.push((Rc::from("MIN_VALUE"), Value::Number(f64::MIN_POSITIVE)));
    static_props.push((Rc::from("POSITIVE_INFINITY"), Value::Number(f64::INFINITY)));
    static_props.push((
        Rc::from("NEGATIVE_INFINITY"),
        Value::Number(f64::NEG_INFINITY),
    ));
    static_props.push((Rc::from("NaN"), Value::Number(f64::NAN)));
    vm.heap.with_obj(num_ctor.0, |o| {
        if let HeapObj::Function(f) = o {
            for (name, val) in &static_props {
                f.props
                    .borrow_mut()
                    .insert(name.clone(), data_prop(val.clone()));
            }
        }
    });
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
    // Promise
    let (promise_ctor, promise_proto) = make_builtin_constructor_with(
        vm,
        "Promise",
        promise_constructor,
        &[("then", promise_then, 2), ("catch", promise_catch, 1)],
    );
    vm.promise_proto = Value::Object(promise_proto);
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
            obj.props
                .borrow_mut()
                .insert(Rc::from("__regex_proto__"), data_prop(Value::Bool(true)));
        }
    });
    // Store regex_proto on the constructor so regexp_constructor can use it.
    vm.heap.with_obj(regex_ctor.0, |o| {
        if let HeapObj::Function(f) = o {
            f.props
                .borrow_mut()
                .insert(Rc::from("__proto__"), data_prop(Value::Object(regex_proto)));
        }
    });
    define_global(vm, "RegExp", Value::Object(regex_ctor));
    // Generator prototype with next(). Generator instances inherit this proto.
    let generator_proto_idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(Some(vm.object_proto.clone())),
        extensible: Cell::new(true),
        class_name: Some(Rc::from("Generator")),
    }));
    {
        let next_fn = vm.new_native_function("next", generator_next, 0);
        vm.heap.with_obj(generator_proto_idx, |o| {
            o.props()
                .borrow_mut()
                .insert(Rc::from("next"), data_prop(Value::Object(next_fn)));
        });
    }
    vm.generator_proto = Value::Object(GcIdx(generator_proto_idx));
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
                m.entries
                    .borrow()
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
    let key = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries.borrow().iter().any(|(k, _)| k == &key)
            } else {
                false
            }
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
                m.entries.borrow_mut().clear();
            }
        });
    }
    Ok(Value::Undefined)
}
fn map_size(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Number(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Map(m) = obj {
                m.entries.borrow().len()
            } else {
                0
            }
        }) as f64));
    }
    Ok(Value::Number(0.0))
}
fn map_constructor(vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let obj = HeapObj::Map(MapData {
        entries: RefCell::new(Vec::new()),
        props: RefCell::new(IndexMap::new()),
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
                if !items.iter().any(|i| i == &val) {
                    items.push(val);
                }
            }
        });
    }
    Ok(this.unwrap_or(Value::Undefined))
}
fn set_has(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        return Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Set(s) = obj {
                s.items.borrow().iter().any(|i| i == &val)
            } else {
                false
            }
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
                s.items.borrow().len()
            } else {
                0
            }
        }) as f64));
    }
    Ok(Value::Number(0.0))
}
fn set_constructor(vm: &mut Vm, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let obj = HeapObj::Set(SetData {
        items: RefCell::new(Vec::new()),
        props: RefCell::new(IndexMap::new()),
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

// =========================================================================
// Promise
// =========================================================================
fn promise_constructor(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let executor = args.get(0).cloned().unwrap_or(Value::Undefined);
    // create the promise object
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: std::cell::Cell::new(crate::value::PromiseStatus::Pending),
            result: RefCell::new(Value::Undefined),
            handlers: RefCell::new(Vec::new()),
            props: RefCell::new(IndexMap::new()),
            proto: RefCell::new(Some(vm.promise_proto.clone())),
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
            name: Some(Rc::from("resolve")),
            kind: crate::value::FunctionKind::Bound {
                target: resolve_target,
                this_val: p_val.clone(),
                bound_args: Vec::new(),
            },
            closure: vm.global,
            prototype: RefCell::new(None),
            props: RefCell::new(IndexMap::new()),
        }));
    let reject_fn = vm
        .heap
        .allocate(HeapObj::Function(crate::value::FunctionData {
            name: Some(Rc::from("reject")),
            kind: crate::value::FunctionKind::Bound {
                target: reject_target,
                this_val: p_val.clone(),
                bound_args: Vec::new(),
            },
            closure: vm.global,
            prototype: RefCell::new(None),
            props: RefCell::new(IndexMap::new()),
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
                .unwrap_or_else(|| Value::String(Rc::from(e.message.as_str())));
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
    let value = args.get(0).cloned().unwrap_or(Value::Undefined);
    vm.promise_resolve(p_idx, value);
    Ok(Value::Undefined)
}
fn promise_reject(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let p_idx = match &this {
        Some(Value::Object(idx)) => idx.0,
        _ => return Ok(Value::Undefined),
    };
    let reason = args.get(0).cloned().unwrap_or(Value::Undefined);
    vm.promise_reject(p_idx, reason);
    Ok(Value::Undefined)
}

fn promise_then(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let on_fulfilled = args.get(0).cloned().unwrap_or(Value::Undefined);
    let on_rejected = args.get(1).cloned().unwrap_or(Value::Undefined);
    let p_idx = match &this {
        Some(Value::Object(idx)) => idx.0,
        _ => return Err(Error::type_err("then called on non-promise".to_string())),
    };
    // Create a derived promise that settles with the handler's result.
    let derived = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: std::cell::Cell::new(crate::value::PromiseStatus::Pending),
            result: RefCell::new(Value::Undefined),
            handlers: RefCell::new(Vec::new()),
            props: RefCell::new(IndexMap::new()),
            proto: RefCell::new(Some(vm.promise_proto.clone())),
        }));
    let (state, _result) = vm.heap.with_obj(p_idx, |o| {
        if let HeapObj::Promise(p) = o {
            (p.state.get(), p.result.borrow().clone())
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
                    p.handlers.borrow_mut().push(handler);
                }
            });
        }
        _ => {
            // already settled: schedule immediately, passing derived for chaining
            vm.microtask_queue.push(crate::vm::Microtask::Then {
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
    let on_rejected = args.get(0).cloned().unwrap_or(Value::Undefined);
    promise_then(vm, &[Value::Undefined, on_rejected], this)
}

// =========================================================================
// RegExp
// =========================================================================
fn regexp_constructor(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let pattern = match args.get(0) {
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
                    o.props().borrow().get("prototype").map(|d| d.value.clone())
                })
                .unwrap_or(vm.object_proto.clone()),
            _ => vm.object_proto.clone(),
        }
    };
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(Some(regex_proto_val)),
        extensible: Cell::new(true),
        class_name: Some(Rc::from("RegExp")),
    }));
    let mut props = IndexMap::new();
    props.insert(
        Rc::from("source"),
        data_prop(Value::String(Rc::from(pattern.as_str()))),
    );
    props.insert(
        Rc::from("flags"),
        data_prop(Value::String(Rc::from(flags.as_str()))),
    );
    props.insert(
        Rc::from("global"),
        data_prop(Value::Bool(flags.contains('g'))),
    );
    props.insert(
        Rc::from("ignoreCase"),
        data_prop(Value::Bool(flags.contains('i'))),
    );
    props.insert(
        Rc::from("multiline"),
        data_prop(Value::Bool(flags.contains('m'))),
    );
    vm.heap.with_obj(obj_idx, |o| {
        if let HeapObj::Object(obj) = o {
            *obj.props.borrow_mut() = props;
        }
    });
    Ok(Value::Object(GcIdx(obj_idx)))
}

fn regexp_test(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let source = read_regexp_source(vm, &this)?;
    let input = match args.get(0) {
        Some(Value::String(s)) => s.to_string(),
        Some(v) => vm.to_string(v)?.to_string(),
        None => String::new(),
    };
    let re = Regex::new(&source).map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
    Ok(Value::Bool(re.is_match(&input)))
}

fn regexp_exec(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let source = read_regexp_source(vm, &this)?;
    let input = match args.get(0) {
        Some(Value::String(s)) => s.to_string(),
        Some(v) => vm.to_string(v)?.to_string(),
        None => String::new(),
    };
    let re = Regex::new(&source).map_err(|e| Error::syntax(format!("Invalid regex: {}", e)))?;
    match re.captures(&input) {
        Some(caps) => {
            let items: Vec<Value> = caps
                .iter()
                .map(|c| match c {
                    Some(m) => Value::String(Rc::from(m.as_str())),
                    None => Value::Undefined,
                })
                .collect();
            Ok(make_value_array(vm, items))
        }
        None => Ok(Value::Null),
    }
}

fn read_regexp_source(vm: &mut Vm, this: &Option<Value>) -> error::Result<String> {
    match this {
        Some(Value::Object(idx)) => {
            let s = vm.heap.with_obj(idx.0, |o| {
                o.props().borrow().get("source").map(|d| d.value.clone())
            });
            match s {
                Some(Value::String(s)) => Ok(s.to_string()),
                _ => Err(Error::type_err("not a RegExp".to_string())),
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
    let (value, done) = vm.heap.with_obj(g_idx, |o| {
        if let HeapObj::Generator(g) = o {
            let state = g.state.borrow_mut();
            let idx = g.ip.get();
            if idx < state.len() {
                g.ip.set(idx + 1);
                (state[idx].clone(), false)
            } else {
                g.done.set(true);
                (Value::Undefined, true)
            }
        } else {
            (Value::Undefined, true)
        }
    });
    // return {value, done}
    let obj_idx = vm.heap.allocate(HeapObj::Object(crate::value::ObjectData {
        props: RefCell::new(IndexMap::new()),
        proto: RefCell::new(Some(vm.object_proto.clone())),
        extensible: Cell::new(true),
        class_name: None,
    }));
    vm.heap.with_obj(obj_idx, |o| {
        if let HeapObj::Object(obj) = o {
            obj.props
                .borrow_mut()
                .insert(Rc::from("value"), data_prop(value));
            obj.props
                .borrow_mut()
                .insert(Rc::from("done"), data_prop(Value::Bool(done)));
        }
    });
    Ok(Value::Object(GcIdx(obj_idx)))
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
        ],
    );
    vm.map_proto = Value::Object(map_proto);
    define_global(vm, "Map", Value::Object(map_ctor));
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
        ],
    );
    vm.set_proto = Value::Object(set_proto);
    define_global(vm, "Set", Value::Object(set_ctor));
    // Symbol
    let sym_idx = vm.new_native_function("Symbol", symbol_constructor, 1);
    define_global(vm, "Symbol", Value::Object(sym_idx));
    let sym_for_idx = vm.new_native_function("for", symbol_for, 1);
    if let Value::Object(idx) = Value::Object(sym_idx) {
        vm.heap.with_obj(idx.0, |obj| {
            obj.props()
                .borrow_mut()
                .insert(Rc::from("for"), data_prop(Value::Object(sym_for_idx)));
            obj.props().borrow_mut().insert(
                Rc::from("iterator"),
                data_prop(Value::Symbol(vm.well_known_symbols.iterator)),
            );
        });
    }
    // Symbol.prototype: a plain Object with a toString method. Symbol is a
    // value type (not a constructor), so build the proto manually rather than
    // going through make_builtin_constructor.
    let sym_tostring_idx = vm.new_native_function("toString", symbol_to_string, 0);
    let mut sym_proto_props: IndexMap<Rc<str>, PropertyDescriptor> = IndexMap::new();
    sym_proto_props.insert(
        Rc::from("toString"),
        data_prop(Value::Object(sym_tostring_idx)),
    );
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

fn make_builtin_constructor_with(
    vm: &mut Vm,
    name: &str,
    ctor: NativeFn,
    methods: &[(&str, NativeFn, usize)],
) -> (GcIdx, GcIdx) {
    let mut method_props: IndexMap<Rc<str>, PropertyDescriptor> = IndexMap::new();
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
        kind: FunctionKind::Native {
            func: ctor,
            length: 0,
        },
        closure: vm.global,
        prototype: RefCell::new(Some(Value::Object(proto_idx))),
        props: RefCell::new(IndexMap::new()),
    };
    let ctor_idx = GcIdx(vm.heap.allocate(HeapObj::Function(ctor_func)));
    vm.heap.with_obj(ctor_idx.0, |obj| {
        obj.props()
            .borrow_mut()
            .insert(Rc::from("prototype"), data_prop(Value::Object(proto_idx)));
    });
    vm.heap.with_obj(proto_idx.0, |obj| {
        obj.props()
            .borrow_mut()
            .insert(Rc::from("constructor"), data_prop(Value::Object(ctor_idx)));
    });
    (ctor_idx, proto_idx)
}
