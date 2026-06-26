use crate::environment::{BindingKind, Env};
use crate::error::{self, Error};
use crate::interpreter::Interpreter;
use crate::value::{FunctionKind, FunctionValue, InternalData, Obj, PropertyDescriptor, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type Native = crate::value::NativeFn;

fn native(name: &str, func: Native, length: usize) -> Value {
    let fv = FunctionValue {
        name: Some(Rc::from(name)),
        kind: FunctionKind::Native { func, length },
        closure: Env::new(),
        prototype: None,
        properties: RefCell::new(HashMap::new()),
    };
    Value::Function(Rc::new(fv))
}

fn make_proto(_interp: &Interpreter, parent: &Value) -> Value {
    let mut o = Obj::new();
    o.proto = Some(parent.clone());
    Value::Object(Rc::new(RefCell::new(o)))
}

fn set_method(obj: &Value, name: &str, func: Value) {
    match obj {
        Value::Object(o) => {
        o.borrow_mut().props.insert(Rc::from(name), PropertyDescriptor::data(func));
        }
        Value::Function(f) => {
            f.properties.borrow_mut().insert(Rc::from(name), PropertyDescriptor::data(func));
        }
        _ => {}
    }
}

pub fn setup(interp: &mut Interpreter) {
    // Create the base Object.prototype
    let object_proto = make_proto(interp, &Value::Null);
    interp.object_proto = object_proto.clone();

    let function_proto = make_proto(interp, &object_proto);
    interp.function_proto = function_proto.clone();

    let array_proto = make_proto(interp, &object_proto);
    interp.array_proto = array_proto.clone();

    let string_proto = make_proto(interp, &object_proto);
    interp.string_proto = string_proto.clone();

    let number_proto = make_proto(interp, &object_proto);
    interp.number_proto = number_proto.clone();

    let boolean_proto = make_proto(interp, &object_proto);
    interp.boolean_proto = boolean_proto.clone();

    let error_proto = make_proto(interp, &object_proto);
    interp.error_proto = error_proto.clone();

    // --- Object.prototype methods ---
    let op = object_proto.clone();
    set_method(&op, "toString", native("toString", obj_to_string, 0));
    set_method(&op, "hasOwnProperty", native("hasOwnProperty", obj_has_own, 1));
    set_method(&op, "valueOf", native("valueOf", obj_value_of, 0));
    set_method(&op, "isPrototypeOf", native("isPrototypeOf", obj_is_proto_of, 1));

    // --- Function.prototype ---
    let fp = function_proto.clone();
    set_method(&fp, "call", native("call", fn_call, 1));
    set_method(&fp, "apply", native("apply", fn_apply, 2));
    set_method(&fp, "bind", native("bind", fn_bind, 1));
    set_method(&fp, "toString", native("toString", fn_to_string, 0));

    // --- Array.prototype ---
    set_method(&array_proto, "push", native("push", array_push, 1));
    set_method(&array_proto, "pop", native("pop", array_pop, 0));
    set_method(&array_proto, "shift", native("shift", array_shift, 0));
    set_method(&array_proto, "unshift", native("unshift", array_unshift, 1));
    set_method(&array_proto, "join", native("join", array_join, 1));
    set_method(&array_proto, "indexOf", native("indexOf", array_index_of, 1));
    set_method(&array_proto, "slice", native("slice", array_slice, 2));
    set_method(&array_proto, "concat", native("concat", array_concat, 1));
    set_method(&array_proto, "reverse", native("reverse", array_reverse, 0));
    set_method(&array_proto, "forEach", native("forEach", array_for_each, 1));
    set_method(&array_proto, "map", native("map", array_map, 1));
    set_method(&array_proto, "filter", native("filter", array_filter, 1));
    set_method(&array_proto, "reduce", native("reduce", array_reduce, 1));
    set_method(&array_proto, "find", native("find", array_find, 1));
    set_method(&array_proto, "includes", native("includes", array_includes, 1));
    set_method(&array_proto, "toString", native("toString", array_to_string, 0));

    // --- String.prototype ---
    set_method(&string_proto, "charAt", native("charAt", str_char_at, 1));
    set_method(&string_proto, "charCodeAt", native("charCodeAt", str_char_code_at, 1));
    set_method(&string_proto, "indexOf", native("indexOf", str_index_of, 1));
    set_method(&string_proto, "slice", native("slice", str_slice, 2));
    set_method(&string_proto, "substring", native("substring", str_substring, 2));
    set_method(&string_proto, "substr", native("substr", str_substr, 2));
    set_method(&string_proto, "toUpperCase", native("toUpperCase", str_to_upper, 0));
    set_method(&string_proto, "toLowerCase", native("toLowerCase", str_to_lower, 0));
    set_method(&string_proto, "trim", native("trim", str_trim, 0));
    set_method(&string_proto, "split", native("split", str_split, 1));
    set_method(&string_proto, "replace", native("replace", str_replace, 2));
    set_method(&string_proto, "includes", native("includes", str_includes, 1));
    set_method(&string_proto, "startsWith", native("startsWith", str_starts_with, 1));
    set_method(&string_proto, "endsWith", native("endsWith", str_ends_with, 1));
    set_method(&string_proto, "repeat", native("repeat", str_repeat, 1));
    set_method(&string_proto, "concat", native("concat", str_concat, 1));
    set_method(&string_proto, "toString", native("toString", str_to_string_val, 0));
    set_method(&string_proto, "valueOf", native("valueOf", str_to_string_val, 0));

    // --- Number.prototype ---
    set_method(&number_proto, "toString", native("toString", num_to_string_method, 0));
    set_method(&number_proto, "toFixed", native("toFixed", num_to_fixed, 1));

    // --- Error.prototype ---
    set_method(&error_proto, "toString", native("toString", error_to_string, 0));
    let ep = error_proto.clone();
    if let Value::Object(o) = &ep {
        o.borrow_mut().props.insert(Rc::from("name"), PropertyDescriptor::data(Value::from_str("Error")));
        o.borrow_mut().props.insert(Rc::from("message"), PropertyDescriptor::data(Value::from_str("")));
    }

    // --- global constructors ---
    let g = interp.global.clone();

    // Object constructor + Object.keys/values/entries/create
    let object_ctor = native("Object", obj_ctor, 1);
    set_static(&g, "Object", &object_ctor);
    set_method(&object_ctor, "keys", native("keys", obj_keys, 1));
    set_method(&object_ctor, "values", native("values", obj_values, 1));
    set_method(&object_ctor, "entries", native("entries", obj_entries, 1));
    set_method(&object_ctor, "create", native("create", obj_create, 1));
    set_method(&object_ctor, "assign", native("assign", obj_assign, 2));
    set_method(&object_ctor, "freeze", native("freeze", obj_freeze, 1));
    if let Value::Function(f) = &object_ctor {
        f.properties.borrow_mut().insert(Rc::from("prototype"), PropertyDescriptor::data(object_proto.clone()));
    }

    // Array constructor
    let array_ctor = native("Array", array_ctor, 1);
    set_static(&g, "Array", &array_ctor);
    set_method(&array_ctor, "isArray", native("isArray", array_is_array, 1));
    set_method(&array_ctor, "from", native("from", array_from, 1));
    set_method(&array_ctor, "of", native("of", array_of, 0));
    if let Value::Function(f) = &array_ctor {
        f.properties.borrow_mut().insert(Rc::from("prototype"), PropertyDescriptor::data(array_proto.clone()));
    }

    // String constructor
    let string_ctor = native("String", string_ctor, 1);
    set_static(&g, "String", &string_ctor);
    set_method(&string_ctor, "fromCharCode", native("fromCharCode", str_from_char_code, 1));
    if let Value::Function(f) = &string_ctor {
        f.properties.borrow_mut().insert(Rc::from("prototype"), PropertyDescriptor::data(string_proto.clone()));
    }

    // Number constructor + constants
    let number_ctor = native("Number", number_ctor, 1);
    set_static(&g, "Number", &number_ctor);
    if let Value::Function(f) = &number_ctor {
        let mut props = f.properties.borrow_mut();
        props.insert(Rc::from("MAX_SAFE_INTEGER"), PropertyDescriptor::data(Value::Number(9007199254740991.0)));
        props.insert(Rc::from("MIN_SAFE_INTEGER"), PropertyDescriptor::data(Value::Number(-9007199254740991.0)));
        props.insert(Rc::from("MAX_VALUE"), PropertyDescriptor::data(Value::Number(f64::MAX)));
        props.insert(Rc::from("MIN_VALUE"), PropertyDescriptor::data(Value::Number(f64::MIN_POSITIVE)));
        props.insert(Rc::from("POSITIVE_INFINITY"), PropertyDescriptor::data(Value::Number(f64::INFINITY)));
        props.insert(Rc::from("NEGATIVE_INFINITY"), PropertyDescriptor::data(Value::Number(f64::NEG_INFINITY)));
        props.insert(Rc::from("NaN"), PropertyDescriptor::data(Value::Number(f64::NAN)));
        props.insert(Rc::from("EPSILON"), PropertyDescriptor::data(Value::Number(f64::EPSILON)));
        props.insert(Rc::from("isInteger"), PropertyDescriptor::data(native("isInteger", num_is_integer, 1)));
        props.insert(Rc::from("isFinite"), PropertyDescriptor::data(native("isFinite", num_is_finite, 1)));
        props.insert(Rc::from("isNaN"), PropertyDescriptor::data(native("isNaN", num_is_nan, 1)));
        props.insert(Rc::from("prototype"), PropertyDescriptor::data(number_proto.clone()));
    }

    // Boolean constructor
    let boolean_ctor = native("Boolean", boolean_ctor, 1);
    set_static(&g, "Boolean", &boolean_ctor);
    if let Value::Function(f) = &boolean_ctor {
        f.properties.borrow_mut().insert(Rc::from("prototype"), PropertyDescriptor::data(boolean_proto.clone()));
    }

    // Error constructors
    setup_error_ctor(&g, "Error", &error_proto);
    let type_error_proto = make_proto(interp, &error_proto);
    if let Value::Object(o) = &type_error_proto {
        o.borrow_mut().props.insert(Rc::from("name"), PropertyDescriptor::data(Value::from_str("TypeError")));
    }
    setup_error_ctor(&g, "TypeError", &type_error_proto);
    let range_error_proto = make_proto(interp, &error_proto);
    if let Value::Object(o) = &range_error_proto {
        o.borrow_mut().props.insert(Rc::from("name"), PropertyDescriptor::data(Value::from_str("RangeError")));
    }
    setup_error_ctor(&g, "RangeError", &range_error_proto);
    let ref_error_proto = make_proto(interp, &error_proto);
    if let Value::Object(o) = &ref_error_proto {
        o.borrow_mut().props.insert(Rc::from("name"), PropertyDescriptor::data(Value::from_str("ReferenceError")));
    }
    setup_error_ctor(&g, "ReferenceError", &ref_error_proto);
    let syntax_error_proto = make_proto(interp, &error_proto);
    if let Value::Object(o) = &syntax_error_proto {
        o.borrow_mut().props.insert(Rc::from("name"), PropertyDescriptor::data(Value::from_str("SyntaxError")));
    }
    setup_error_ctor(&g, "SyntaxError", &syntax_error_proto);

    // Math
    let math = {
        let mut o = Obj::new();
        o.props.insert(Rc::from("PI"), PropertyDescriptor::data(Value::Number(std::f64::consts::PI)));
        o.props.insert(Rc::from("E"), PropertyDescriptor::data(Value::Number(std::f64::consts::E)));
        o.props.insert(Rc::from("LN2"), PropertyDescriptor::data(Value::Number(std::f64::consts::LN_2)));
        o.props.insert(Rc::from("LN10"), PropertyDescriptor::data(Value::Number(std::f64::consts::LN_10)));
        o.props.insert(Rc::from("SQRT2"), PropertyDescriptor::data(Value::Number(std::f64::consts::SQRT_2)));
        o.props.insert(Rc::from("floor"), PropertyDescriptor::data(native("floor", math_floor, 1)));
        o.props.insert(Rc::from("ceil"), PropertyDescriptor::data(native("ceil", math_ceil, 1)));
        o.props.insert(Rc::from("round"), PropertyDescriptor::data(native("round", math_round, 1)));
        o.props.insert(Rc::from("abs"), PropertyDescriptor::data(native("abs", math_abs, 1)));
        o.props.insert(Rc::from("sqrt"), PropertyDescriptor::data(native("sqrt", math_sqrt, 1)));
        o.props.insert(Rc::from("pow"), PropertyDescriptor::data(native("pow", math_pow, 2)));
        o.props.insert(Rc::from("max"), PropertyDescriptor::data(native("max", math_max, 2)));
        o.props.insert(Rc::from("min"), PropertyDescriptor::data(native("min", math_min, 2)));
        o.props.insert(Rc::from("random"), PropertyDescriptor::data(native("random", math_random, 0)));
        o.props.insert(Rc::from("log"), PropertyDescriptor::data(native("log", math_log, 1)));
        o.props.insert(Rc::from("log2"), PropertyDescriptor::data(native("log2", math_log2, 1)));
        o.props.insert(Rc::from("log10"), PropertyDescriptor::data(native("log10", math_log10, 1)));
        o.props.insert(Rc::from("exp"), PropertyDescriptor::data(native("exp", math_exp, 1)));
        o.props.insert(Rc::from("sin"), PropertyDescriptor::data(native("sin", math_sin, 1)));
        o.props.insert(Rc::from("cos"), PropertyDescriptor::data(native("cos", math_cos, 1)));
        o.props.insert(Rc::from("tan"), PropertyDescriptor::data(native("tan", math_tan, 1)));
        o.props.insert(Rc::from("trunc"), PropertyDescriptor::data(native("trunc", math_trunc, 1)));
        o.props.insert(Rc::from("sign"), PropertyDescriptor::data(native("sign", math_sign, 1)));
        Value::Object(Rc::new(RefCell::new(o)))
    };
    set_static(&g, "Math", &math);

    // JSON
    let json = {
        let mut o = Obj::new();
        o.props.insert(Rc::from("parse"), PropertyDescriptor::data(native("parse", json_parse, 1)));
        o.props.insert(Rc::from("stringify"), PropertyDescriptor::data(native("stringify", json_stringify, 3)));
        Value::Object(Rc::new(RefCell::new(o)))
    };
    set_static(&g, "JSON", &json);

    // console
    let console = {
        let mut o = Obj::new();
        o.props.insert(Rc::from("log"), PropertyDescriptor::data(native("log", console_log, 0)));
        o.props.insert(Rc::from("error"), PropertyDescriptor::data(native("error", console_log, 0)));
        o.props.insert(Rc::from("warn"), PropertyDescriptor::data(native("warn", console_log, 0)));
        o.props.insert(Rc::from("info"), PropertyDescriptor::data(native("info", console_log, 0)));
        o.props.insert(Rc::from("debug"), PropertyDescriptor::data(native("debug", console_log, 0)));
        Value::Object(Rc::new(RefCell::new(o)))
    };
    set_static(&g, "console", &console);

    // global functions
    set_static(&g, "parseInt", &native("parseInt", global_parse_int, 1));
    set_static(&g, "parseFloat", &native("parseFloat", global_parse_float, 1));
    set_static(&g, "isNaN", &native("isNaN", global_is_nan, 1));
    set_static(&g, "isFinite", &native("isFinite", global_is_finite, 1));
    set_static(&g, "NaN", &Value::Number(f64::NAN));
    set_static(&g, "Infinity", &Value::Number(f64::INFINITY));
    set_static(&g, "undefined", &Value::Undefined);
}

fn set_static(g: &Env, name: &str, val: &Value) {
    g.declare(name, val.clone(), BindingKind::Const);
}

fn setup_error_ctor(g: &Env, name: &str, proto: &Value) {
    let ctor = native(name, error_ctor, 1);
    if let Value::Function(f) = &ctor {
        f.properties.borrow_mut().insert(Rc::from("prototype"), PropertyDescriptor::data(proto.clone()));
    }
    if let Value::Object(o) = proto {
        // link back constructor
        o.borrow_mut().props.insert(Rc::from("constructor"), PropertyDescriptor::data(ctor.clone()));
    }
    set_static(g, name, &ctor);
}

fn this_obj(this: Option<Value>) -> Value {
    this.unwrap_or(Value::Undefined)
}

// ---- Object.prototype ----
fn obj_to_string(interp: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let t = this_obj(this);
    Ok(Value::String(interp.to_string_val(&t)?))
}
fn obj_has_own(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.get(0).map(|v| v.to_string_debug()).unwrap_or_default();
    if let Value::Object(o) = &this_obj(this) {
        let key_rc: Rc<str> = Rc::from(key.as_str());
        Ok(Value::Bool(o.borrow().props.contains_key(&*key_rc)))
    } else { Ok(Value::Bool(false)) }
}
fn obj_value_of(_i: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(this_obj(this.clone()))
}
fn obj_is_proto_of(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let proto = this_obj(this);
    let mut cur = match args.get(0) {
        Some(Value::Object(o)) => o.borrow().proto.clone(),
        _ => None,
    };
    while let Some(p) = cur {
        if p.eq(&proto) { return Ok(Value::Bool(true)); }
        cur = match &p { Value::Object(o) => o.borrow().proto.clone(), _ => None };
    }
    Ok(Value::Bool(false))
}

// ---- Function.prototype ----
fn fn_call(interp: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let f = this_obj(this);
    let new_this = args.get(0).cloned().unwrap_or(Value::Undefined);
    let rest = &args[1.min(args.len())..];
    interp.call_function(&f, rest, Some(new_this))
}
fn fn_apply(interp: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let f = this_obj(this);
    let new_this = args.get(0).cloned().unwrap_or(Value::Undefined);
    let call_args = match args.get(1) {
        Some(Value::Object(o)) => {
            let o = o.borrow();
            if let InternalData::Array(items) = &o.internal { items.clone() } else { Vec::new() }
        }
        _ => Vec::new(),
    };
    interp.call_function(&f, &call_args, Some(new_this))
}
fn fn_bind(_interp: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = this_obj(this);
    if let Value::Function(f) = &target {
        let bound_this = args.get(0).cloned().unwrap_or(Value::Undefined);
        let bound_args: Vec<Value> = args[1.min(args.len())..].to_vec();
        let fv = FunctionValue {
            name: f.name.clone(),
            kind: FunctionKind::Bound { target: f.clone(), this_val: bound_this, bound_args },
            closure: Env::new(),
            prototype: None,
            properties: RefCell::new(HashMap::new()),
        };
        Ok(Value::Function(Rc::new(fv)))
    } else {
        Err(Error::type_err("bind called on non-function".to_string()))
    }
}
fn fn_to_string(_i: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let f = this_obj(this);
    if let Value::Function(fv) = &f {
        let n = fv.name.as_ref().map(|s| s.to_string()).unwrap_or_default();
        Ok(Value::String(Rc::from(format!("function {}() {{ [native code] }}", n).as_str())))
    } else {
        Ok(Value::String(Rc::from("function () { [native code] }")))
    }
}

// ---- Array.prototype ----
fn arr_items(this: &Option<Value>) -> Vec<Value> {
    match this_obj(this.clone()) {
        Value::Object(o) => {
            let o = o.borrow();
            if let InternalData::Array(items) = &o.internal { items.clone() } else { Vec::new() }
        }
        _ => Vec::new(),
    }
}
fn array_push(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Value::Object(o) = &this_obj(this) {
        let mut o = o.borrow_mut();
        if let InternalData::Array(items) = &mut o.internal {
            items.extend_from_slice(args);
            return Ok(Value::Number(items.len() as f64));
        }
    }
    Ok(Value::Number(0.0))
}
fn array_pop(_i: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Value::Object(o) = &this_obj(this) {
        let mut o = o.borrow_mut();
        if let InternalData::Array(items) = &mut o.internal {
            return Ok(items.pop().unwrap_or(Value::Undefined));
        }
    }
    Ok(Value::Undefined)
}
fn array_shift(_i: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Value::Object(o) = &this_obj(this) {
        let mut o = o.borrow_mut();
        if let InternalData::Array(items) = &mut o.internal {
            if items.is_empty() { return Ok(Value::Undefined); }
            return Ok(items.remove(0));
        }
    }
    Ok(Value::Undefined)
}
fn array_unshift(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Value::Object(o) = &this_obj(this) {
        let mut o = o.borrow_mut();
        if let InternalData::Array(items) = &mut o.internal {
            for (i, a) in args.iter().enumerate() { items.insert(i, a.clone()); }
            return Ok(Value::Number(items.len() as f64));
        }
    }
    Ok(Value::Number(0.0))
}
fn array_join(interp: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = arr_items(&this);
    let sep = match args.get(0) {
        Some(Value::Undefined) | None => ",".to_string(),
        Some(v) => interp.to_string_val(v)?.to_string(),
    };
    let parts: Vec<String> = items.iter()
        .map(|i| if i.is_nullish() { String::new() } else { interp.to_string_val(i).map(|s| s.to_string()).unwrap_or_default() })
        .collect();
    Ok(Value::String(Rc::from(parts.join(&sep).as_str())))
}
fn array_index_of(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = arr_items(&this);
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    for (i, item) in items.iter().enumerate() {
        if item == &target { return Ok(Value::Number(i as f64)); }
    }
    Ok(Value::Number(-1.0))
}
fn array_slice(interp: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = arr_items(&this);
    let len = items.len() as i64;
    let start = args.get(0).and_then(|v| if let Value::Number(n) = v { Some(*n as i64) } else { None }).unwrap_or(0);
    let end = args.get(1).and_then(|v| if let Value::Number(n) = v { Some(*n as i64) } else { None }).unwrap_or(len);
    let s = if start < 0 { (len + start).max(0) as usize } else { (start as usize).min(items.len()) };
    let e = if end < 0 { (len + end).max(0) as usize } else { (end as usize).min(items.len()) };
    let sliced: Vec<Value> = if s < e { items[s..e].to_vec() } else { Vec::new() };
    Ok(interp.new_array(sliced))
}
fn array_concat(interp: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let mut items = arr_items(&this);
    for a in args {
        match a {
            Value::Object(o) => {
                let o = o.borrow();
                if let InternalData::Array(extra) = &o.internal {
                    items.extend_from_slice(extra);
                    continue;
                }
            }
            _ => {}
        }
        items.push(a.clone());
    }
    Ok(interp.new_array(items))
}
fn array_reverse(_i: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let this_val = this_obj(this.clone());
    if let Value::Object(o) = &this_val {
        let mut o = o.borrow_mut();
        if let InternalData::Array(items) = &mut o.internal {
            items.reverse();
        }
    }
    Ok(this_val)
}
fn array_for_each(interp: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = arr_items(&this);
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    for (i, item) in items.iter().enumerate() {
        interp.call_function(&cb, &[item.clone(), Value::Number(i as f64), this_obj(this.clone())], Some(Value::Undefined))?;
    }
    Ok(Value::Undefined)
}
fn array_map(interp: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = arr_items(&this);
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    let mut result = Vec::new();
    for (i, item) in items.iter().enumerate() {
        result.push(interp.call_function(&cb, &[item.clone(), Value::Number(i as f64), this_obj(this.clone())], Some(Value::Undefined))?);
    }
    Ok(interp.new_array(result))
}
fn array_filter(interp: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = arr_items(&this);
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    let mut result = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let keep = interp.call_function(&cb, &[item.clone(), Value::Number(i as f64), this_obj(this.clone())], Some(Value::Undefined))?;
        if keep.is_truthy() { result.push(item.clone()); }
    }
    Ok(interp.new_array(result))
}
fn array_reduce(interp: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = arr_items(&this);
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    let (mut acc, start) = if args.len() >= 2 {
        (args[1].clone(), 0)
    } else {
        (items.get(0).cloned().unwrap_or(Value::Undefined), 1)
    };
    for i in start..items.len() {
        acc = interp.call_function(&cb, &[acc, items[i].clone(), Value::Number(i as f64), this_obj(this.clone())], Some(Value::Undefined))?;
    }
    Ok(acc)
}
fn array_find(interp: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = arr_items(&this);
    let cb = args.get(0).cloned().unwrap_or(Value::Undefined);
    for (i, item) in items.iter().enumerate() {
        let found = interp.call_function(&cb, &[item.clone(), Value::Number(i as f64), this_obj(this.clone())], Some(Value::Undefined))?;
        if found.is_truthy() { return Ok(item.clone()); }
    }
    Ok(Value::Undefined)
}
fn array_includes(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = arr_items(&this);
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    Ok(Value::Bool(items.iter().any(|i| i == &target)))
}
fn array_to_string(interp: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    array_join(interp, &[], this)
}

// ---- Array constructor ----
fn array_ctor(interp: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let items = if args.len() == 1 {
        if let Some(Value::Number(n)) = args.get(0) {
            vec![Value::Undefined; *n as usize]
        } else { args.to_vec() }
    } else { args.to_vec() };
    Ok(interp.new_array(items))
}
fn array_is_array(_i: &mut Interpreter, _args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    // Note: for static methods `this` isn't used; arg comes via args[0] in our calling convention.
    Ok(Value::Bool(false))
}
fn array_from(interp: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let v = args.get(0).cloned().unwrap_or(Value::Undefined);
    let items = interp.iter_to_values_pub(&v)?;
    Ok(interp.new_array(items))
}
fn array_of(interp: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    Ok(interp.new_array(args.to_vec()))
}

// ---- String.prototype ----
fn str_val(this: &Option<Value>) -> String {
    this_obj(this.clone()).to_string_debug()
}
fn str_char_at(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(&this);
    let idx = args.get(0).and_then(|v| if let Value::Number(n) = v { Some(*n as usize) } else { None }).unwrap_or(0);
    Ok(s.chars().nth(idx).map(|c| Value::from_str(&c.to_string())).unwrap_or(Value::String(Rc::from(""))))
}
fn str_char_code_at(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(&this);
    let idx = args.get(0).and_then(|v| if let Value::Number(n) = v { Some(*n as usize) } else { None }).unwrap_or(0);
    Ok(s.chars().nth(idx).map(|c| Value::Number(c as u32 as f64)).unwrap_or(Value::Number(f64::NAN)))
}
fn str_index_of(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(&this);
    let needle = args.get(0).map(|v| v.to_string_debug()).unwrap_or_default();
    Ok(Value::Number(s.find(&needle).map(|i| i as f64).unwrap_or(-1.0)))
}
fn str_slice(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(&this);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i64;
    let start = args.get(0).and_then(|v| if let Value::Number(n) = v { Some(*n as i64) } else { None }).unwrap_or(0);
    let end = args.get(1).and_then(|v| if let Value::Number(n) = v { Some(*n as i64) } else { None }).unwrap_or(len);
    let st = if start < 0 { (len + start).max(0) as usize } else { (start as usize).min(chars.len()) };
    let en = if end < 0 { (len + end).max(0) as usize } else { (end as usize).min(chars.len()) };
    let result: String = if st < en { chars[st..en].iter().collect() } else { String::new() };
    Ok(Value::String(Rc::from(result.as_str())))
}
fn str_substring(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(&this);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut start = args.get(0).and_then(|v| if let Value::Number(n) = v { Some(*n as usize) } else { None }).unwrap_or(0);
    let mut end = args.get(1).and_then(|v| if let Value::Number(n) = v { Some(*n as usize) } else { None }).unwrap_or(len);
    if start > end { std::mem::swap(&mut start, &mut end); }
    start = start.min(len); end = end.min(len);
    let result: String = chars[start..end].iter().collect();
    Ok(Value::String(Rc::from(result.as_str())))
}
fn str_substr(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(&this);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i64;
    let mut start = args.get(0).and_then(|v| if let Value::Number(n) = v { Some(*n as i64) } else { None }).unwrap_or(0);
    if start < 0 { start = (len + start).max(0); }
    let length = args.get(1).and_then(|v| if let Value::Number(n) = v { Some(*n as i64) } else { None }).unwrap_or(len - start);
    let st = start as usize;
    let en = (st + length.max(0) as usize).min(chars.len());
    let result: String = if st < en { chars[st..en].iter().collect() } else { String::new() };
    Ok(Value::String(Rc::from(result.as_str())))
}
fn str_to_upper(_i: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Rc::from(str_val(&this).to_uppercase().as_str())))
}
fn str_to_lower(_i: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Rc::from(str_val(&this).to_lowercase().as_str())))
}
fn str_trim(_i: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Rc::from(str_val(&this).trim())))
}
fn str_split(interp: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(&this);
    let sep = args.get(0).map(|v| v.to_string_debug());
    let parts: Vec<String> = match sep {
        None => vec![s],
        Some(sep) if sep.is_empty() => s.chars().map(|c| c.to_string()).collect(),
        Some(sep) => s.split(&sep).map(|p| p.to_string()).collect(),
    };
    let items: Vec<Value> = parts.into_iter().map(|p| Value::String(Rc::from(p.as_str()))).collect();
    Ok(interp.new_array(items))
}
fn str_replace(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(&this);
    let from = args.get(0).map(|v| v.to_string_debug()).unwrap_or_default();
    let to = args.get(1).map(|v| v.to_string_debug()).unwrap_or_default();
    Ok(Value::String(Rc::from(s.replacen(&from, &to, 1).as_str())))
}
fn str_includes(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(&this);
    let needle = args.get(0).map(|v| v.to_string_debug()).unwrap_or_default();
    Ok(Value::Bool(s.contains(&needle)))
}
fn str_starts_with(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(&this);
    let needle = args.get(0).map(|v| v.to_string_debug()).unwrap_or_default();
    Ok(Value::Bool(s.starts_with(&needle)))
}
fn str_ends_with(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(&this);
    let needle = args.get(0).map(|v| v.to_string_debug()).unwrap_or_default();
    Ok(Value::Bool(s.ends_with(&needle)))
}
fn str_repeat(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let s = str_val(&this);
    let n = args.get(0).and_then(|v| if let Value::Number(n) = v { Some(*n as usize) } else { None }).unwrap_or(0);
    Ok(Value::String(Rc::from(s.repeat(n).as_str())))
}
fn str_concat(interp: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let mut s = str_val(&this);
    for a in args { s.push_str(&interp.to_string_val(a)?.to_string()); }
    Ok(Value::String(Rc::from(s.as_str())))
}
fn str_to_string_val(_i: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    Ok(Value::String(Rc::from(str_val(&this).as_str())))
}

// ---- String constructor ----
fn string_ctor(interp: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let v = args.get(0).cloned().unwrap_or(Value::Undefined);
    Ok(Value::String(interp.to_string_val(&v)?))
}
fn str_from_char_code(_i: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let s: String = args.iter()
        .filter_map(|v| if let Value::Number(n) = v { char::from_u32(*n as u32) } else { None })
        .collect();
    Ok(Value::String(Rc::from(s.as_str())))
}

// ---- Number ----
fn number_ctor(interp: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let v = args.get(0).cloned().unwrap_or(Value::Number(0.0));
    Ok(Value::Number(interp.to_number(&v)?))
}
fn num_to_string_method(_interp: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let n = match this_obj(this) { Value::Number(n) => n, _ => 0.0 };
    Ok(Value::String(Rc::from(crate::value::num_to_string(n).as_str())))
}
fn num_to_fixed(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let n = match this_obj(this) { Value::Number(n) => n, _ => 0.0 };
    let d = args.get(0).and_then(|v| if let Value::Number(n) = v { Some(*n as usize) } else { None }).unwrap_or(0);
    Ok(Value::String(Rc::from(format!("{:.*}", d, n).as_str())))
}
fn num_is_integer(_i: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(matches!(args.get(0), Some(Value::Number(n)) if n.fract() == 0.0 && n.is_finite())))
}
fn num_is_finite(_i: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(matches!(args.get(0), Some(Value::Number(n)) if n.is_finite())))
}
fn num_is_nan(_i: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(matches!(args.get(0), Some(Value::Number(n)) if n.is_nan())))
}

// ---- Boolean ----
fn boolean_ctor(_i: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let v = args.get(0).cloned().unwrap_or(Value::Undefined);
    Ok(Value::Bool(v.is_truthy()))
}

// ---- Error constructor ----
fn error_ctor(_i: &mut Interpreter, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let msg = args.get(0).cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(o)) = this.clone() {
        let mut o = o.borrow_mut();
        if !msg.is_undefined() {
            o.props.insert(Rc::from("message"), PropertyDescriptor::data(msg));
        }
    }
    Ok(this_obj(this))
}
fn error_to_string(_i: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let o = this_obj(this);
    if let Value::Object(obj) = &o {
        let obj = obj.borrow();
        let name = obj.props.get("name").map(|d| d.value.to_string_debug()).unwrap_or_else(|| "Error".to_string());
        let msg = obj.props.get("message").map(|d| d.value.to_string_debug()).unwrap_or_default();
        if msg.is_empty() { return Ok(Value::String(Rc::from(name.as_str()))); }
        return Ok(Value::String(Rc::from(format!("{}: {}", name, msg).as_str())));
    }
    Ok(Value::String(Rc::from("Error")))
}

// ---- Object constructor ----
fn obj_ctor(_i: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let t = this_obj(this);
    if t.is_undefined() || t.is_null() {
        let o = Obj::new();
        Ok(Value::Object(Rc::new(RefCell::new(o))))
    } else { Ok(t) }
}
fn obj_keys(interp: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let v = args.get(0).cloned().unwrap_or(Value::Undefined);
    let keys = interp.own_enum_keys_pub(&v);
    let items: Vec<Value> = keys.into_iter().map(|k| Value::String(Rc::from(k.as_str()))).collect();
    Ok(interp.new_array(items))
}
fn obj_values(interp: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let v = args.get(0).cloned().unwrap_or(Value::Undefined);
    let keys = interp.own_enum_keys_pub(&v);
    let mut items = Vec::new();
    for k in keys { items.push(interp.get_property(&v, &k)?); }
    Ok(interp.new_array(items))
}
fn obj_entries(interp: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let v = args.get(0).cloned().unwrap_or(Value::Undefined);
    let keys = interp.own_enum_keys_pub(&v);
    let mut items = Vec::new();
    for k in keys {
        let val = interp.get_property(&v, &k)?;
        items.push(interp.new_array(vec![Value::String(Rc::from(k.as_str())), val]));
    }
    Ok(interp.new_array(items))
}
fn obj_create(_interp: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let proto = args.get(0).cloned().unwrap_or(Value::Null);
    let mut o = Obj::new();
    o.proto = match &proto { Value::Object(_) | Value::Null => Some(proto), _ => None };
    Ok(Value::Object(Rc::new(RefCell::new(o))))
}
fn obj_assign(interp: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let target = args.get(0).cloned().unwrap_or(Value::Undefined);
    for src in &args[1.min(args.len())..] {
        let keys = interp.own_enum_keys_pub(src);
        for k in keys {
            let v = interp.get_property(src, &k)?;
            interp.set_property(&target, &k, v)?;
        }
    }
    Ok(target)
}
fn obj_freeze(_i: &mut Interpreter, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let this_val = this_obj(this.clone());
    if let Value::Object(o) = &this_val {
        let mut o = o.borrow_mut();
        o.extensible = false;
        for (_, d) in o.props.iter_mut() {
            d.writable = false;
            d.configurable = false;
        }
    }
    Ok(this_val)
}

// ---- Math ----
fn num_arg(args: &[Value]) -> f64 {
    args.get(0).and_then(|v| if let Value::Number(n) = v { Some(*n) } else { None }).unwrap_or(f64::NAN)
}
fn math_floor(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> { Ok(Value::Number(num_arg(args).floor())) }
fn math_ceil(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> { Ok(Value::Number(num_arg(args).ceil())) }
fn math_round(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> {
    let n = args.get(0).and_then(|v| if let Value::Number(n) = v { Some(*n) } else { None }).unwrap_or(f64::NAN);
    Ok(Value::Number(n.round()))
}
fn math_abs(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> { Ok(Value::Number(num_arg(args).abs())) }
fn math_sqrt(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> { Ok(Value::Number(num_arg(args).sqrt())) }
fn math_pow(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> {
    let a = args.get(0).and_then(|v| if let Value::Number(n) = v { Some(*n) } else { None }).unwrap_or(f64::NAN);
    let b = args.get(1).and_then(|v| if let Value::Number(n) = v { Some(*n) } else { None }).unwrap_or(f64::NAN);
    Ok(Value::Number(a.powf(b)))
}
fn math_max(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> {
    let mut m = f64::NEG_INFINITY;
    for a in args { if let Value::Number(n) = a { if *n > m { m = *n; } } }
    Ok(Value::Number(m))
}
fn math_min(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> {
    let mut m = f64::INFINITY;
    for a in args { if let Value::Number(n) = a { if *n < m { m = *n; } } }
    Ok(Value::Number(m))
}
fn math_random(_i: &mut Interpreter, _args: &[Value], _t: Option<Value>) -> error::Result<Value> {
    // simple LCG since no rand crate
    use std::cell::Cell;
    thread_local! { static STATE: Cell<u64> = Cell::new(0x2545F4914F6CDD1D); }
    let r = STATE.with(|s| {
        let mut x = s.get();
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        s.set(x);
        x as f64 / u64::MAX as f64
    });
    Ok(Value::Number(r))
}
fn math_log(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> { Ok(Value::Number(num_arg(args).ln())) }
fn math_log2(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> { Ok(Value::Number(num_arg(args).log2())) }
fn math_log10(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> { Ok(Value::Number(num_arg(args).log10())) }
fn math_exp(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> { Ok(Value::Number(num_arg(args).exp())) }
fn math_sin(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> { Ok(Value::Number(num_arg(args).sin())) }
fn math_cos(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> { Ok(Value::Number(num_arg(args).cos())) }
fn math_tan(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> { Ok(Value::Number(num_arg(args).tan())) }
fn math_trunc(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> { Ok(Value::Number(num_arg(args).trunc())) }
fn math_sign(_i: &mut Interpreter, args: &[Value], _t: Option<Value>) -> error::Result<Value> {
    let n = args.get(0).and_then(|v| if let Value::Number(n) = v { Some(*n) } else { None }).unwrap_or(f64::NAN);
    Ok(Value::Number(if n > 0.0 { 1.0 } else if n < 0.0 { -1.0 } else { 0.0 }))
}

// ---- JSON ----
fn json_parse(interp: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let s = args.get(0).map(|v| v.to_string_debug()).unwrap_or_default();
    interp.parse_json(&s)
}
fn json_stringify(interp: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let v = args.get(0).cloned().unwrap_or(Value::Undefined);
    match interp.stringify_json(&v, None) {
        Some(s) => Ok(Value::String(Rc::from(s.as_str()))),
        None => Ok(Value::Undefined),
    }
}

// ---- console ----
fn console_log(interp: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let parts: Vec<String> = args.iter()
        .map(|a| interp.to_string_val(a).map(|s| s.to_string()).unwrap_or_default())
        .collect();
    println!("{}", parts.join(" "));
    Ok(Value::Undefined)
}

// ---- globals ----
fn global_parse_int(_i: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let s = args.get(0).map(|v| v.to_string_debug()).unwrap_or_default();
    let radix = args.get(1).and_then(|v| if let Value::Number(n) = v { Some(*n as u32) } else { None }).unwrap_or(10);
    let radix = if radix == 0 { 10 } else { radix };
    let trimmed = s.trim_start();
    let mut chars = trimmed.chars().peekable();
    let mut sign = 1.0;
    if chars.peek() == Some(&'+') { chars.next(); }
    else if chars.peek() == Some(&'-') { sign = -1.0; chars.next(); }
    let rest: String = chars.collect();
    let (digits, r) = if (radix == 16) && (rest.starts_with("0x") || rest.starts_with("0X")) {
        (&rest[2..], radix)
    } else { (rest.as_str(), radix) };
    match i64::from_str_radix(digits, r) {
        Ok(n) => Ok(Value::Number(sign * n as f64)),
        Err(_) => Ok(Value::Number(f64::NAN)),
    }
}
fn global_parse_float(_i: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let s = args.get(0).map(|v| v.to_string_debug()).unwrap_or_default();
    let trimmed = s.trim_start();
    // take longest valid float prefix
    let end = trimmed.char_indices()
        .take_while(|(_, c)| c.is_ascii_digit() || *c == '.' || *c == '+' || *c == '-' || *c == 'e' || *c == 'E')
        .last().map(|(i, _)| i + 1).unwrap_or(0);
    let prefix = &trimmed[..end];
    Ok(Value::Number(prefix.parse::<f64>().unwrap_or(f64::NAN)))
}
fn global_is_nan(_i: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(matches!(args.get(0), Some(Value::Number(n)) if n.is_nan())))
}
fn global_is_finite(_i: &mut Interpreter, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(matches!(args.get(0), Some(Value::Number(n)) if n.is_finite())))
}

// helper trait for Value
trait ValueDebug {
    fn to_string_debug(&self) -> String;
}
impl ValueDebug for Value {
    fn to_string_debug(&self) -> String {
        match self {
            Value::String(s) => s.to_string(),
            Value::Number(n) => crate::value::num_to_string(*n),
            Value::Bool(b) => b.to_string(),
            Value::Undefined => "undefined".to_string(),
            Value::Null => "null".to_string(),
            _ => format!("{:?}", self),
        }
    }
}
