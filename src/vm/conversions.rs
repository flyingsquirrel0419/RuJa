//! Type conversion helpers (ToInt32, ToUint32, ToNumber, ToString,
//! ToPrimitive) split from vm/mod.rs for readability.

use super::*;
use crate::error::{self, Error};
use crate::value::HeapObj;
use crate::value::Value;
use num_traits::ToPrimitive;
use std::sync::Arc;

/// ES `ToInt32`: convert an f64 to a 32-bit signed integer using the spec's
/// modular reduction. Rust's `as i32` saturates large values to `i32::MAX`,
/// which broke `(2**31) | 0` (got `2147483647` instead of `-2147483648`)
/// and `(2**32) | 0` (got `2147483647` instead of `0`).
pub(crate) fn to_int32(n: f64) -> i32 {
    to_uint32(n) as i32
}

/// ES `ToUint32`: convert an f64 to a 32-bit unsigned integer via the spec's
/// modular reduction. `as u32` would saturate, so `(-1) >>> 0` and large
/// values were wrong.
pub(crate) fn to_uint32(n: f64) -> u32 {
    if !n.is_finite() || n.is_nan() {
        return 0;
    }
    // Truncate toward zero first.
    let int = n.trunc();
    // Reduce mod 2^32 using euclidean remainder (always non-negative),
    // which gives the correct uint32 for negatives: -1 -> 4294967295.
    let m = int.rem_euclid(4294967296.0);
    m as u32
}

fn trim_ecmascript_whitespace(s: &str) -> &str {
    s.trim_matches(|ch: char| ch.is_whitespace() || ch == '\u{FEFF}')
}

impl Vm {
    /// float, or NaN if it does not parse.
    pub(crate) fn string_to_number(s: &str) -> f64 {
        let t = trim_ecmascript_whitespace(s);
        if t.is_empty() {
            return 0.0;
        }
        if t == "Infinity" || t == "+Infinity" {
            return f64::INFINITY;
        }
        if t == "-Infinity" {
            return f64::NEG_INFINITY;
        }
        if t.eq_ignore_ascii_case("Infinity")
            || t.eq_ignore_ascii_case("+Infinity")
            || t.eq_ignore_ascii_case("-Infinity")
            || t.eq_ignore_ascii_case("inf")
            || t.eq_ignore_ascii_case("+inf")
            || t.eq_ignore_ascii_case("-inf")
        {
            return f64::NAN;
        }
        // Hex/binary/octal integer literals (no sign, no fraction).
        let (radix, digits) = if let Some(d) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X"))
        {
            (16, d)
        } else if let Some(d) = t.strip_prefix("0b").or_else(|| t.strip_prefix("0B")) {
            (2, d)
        } else if let Some(d) = t.strip_prefix("0o").or_else(|| t.strip_prefix("0O")) {
            (8, d)
        } else {
            return t.parse::<f64>().unwrap_or(f64::NAN);
        };
        let valid_digits = match radix {
            2 => digits.bytes().all(|byte| matches!(byte, b'0' | b'1')),
            8 => digits.bytes().all(|byte| matches!(byte, b'0'..=b'7')),
            16 => digits.bytes().all(|byte| byte.is_ascii_hexdigit()),
            _ => false,
        };
        if digits.is_empty() || !valid_digits {
            return f64::NAN;
        }
        match num_bigint::BigUint::parse_bytes(digits.as_bytes(), radix) {
            Some(number) => number.to_f64().unwrap_or(f64::INFINITY),
            None => f64::NAN,
        }
    }

    pub(crate) fn string_to_bigint(s: &str) -> Option<num_bigint::BigInt> {
        let t = trim_ecmascript_whitespace(s);
        if t.is_empty() {
            return Some(num_bigint::BigInt::from(0));
        }
        if let Some(digits) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
            return num_bigint::BigInt::parse_bytes(digits.as_bytes(), 16);
        }
        if let Some(digits) = t.strip_prefix("0o").or_else(|| t.strip_prefix("0O")) {
            return num_bigint::BigInt::parse_bytes(digits.as_bytes(), 8);
        }
        if let Some(digits) = t.strip_prefix("0b").or_else(|| t.strip_prefix("0B")) {
            return num_bigint::BigInt::parse_bytes(digits.as_bytes(), 2);
        }
        num_bigint::BigInt::parse_bytes(t.as_bytes(), 10)
    }

    pub(crate) fn number_to_bigint_exact(n: f64) -> Option<num_bigint::BigInt> {
        if !n.is_finite() || n.fract() != 0.0 {
            return None;
        }
        num_traits::FromPrimitive::from_f64(n)
    }

    pub fn to_number(&mut self, v: &Value) -> error::Result<f64> {
        Ok(match v {
            Value::Undefined => f64::NAN,
            Value::Null => 0.0,
            Value::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Value::Number(n) => *n,
            Value::BigInt(_) => {
                return Err(Error::type_err(
                    "Cannot convert BigInt to number".to_string(),
                ))
            }
            Value::String(s) => Self::string_to_number(s),
            Value::Object(_) => {
                // Per ES ToNumber on objects: run ToPrimitive(number hint)
                // (valueOf then toString), then convert the primitive result.
                let prim = self.to_primitive_number(v)?;
                self.to_number(&prim)?
            }
            Value::Symbol(_) => {
                return Err(Error::type_err(
                    "Cannot convert Symbol to number".to_string(),
                ));
            }
            Value::PrivateName(_) => {
                return Err(Error::type_err(
                    "Cannot convert private name to number".to_string(),
                ));
            }
            Value::Reference(_) => f64::NAN,
        })
    }

    pub fn to_numeric(&mut self, v: &Value) -> error::Result<Value> {
        let prim = match v {
            Value::Object(_) => self.to_primitive_number(v)?,
            _ => v.clone(),
        };
        match prim {
            Value::BigInt(_) => Ok(prim),
            _ => Ok(Value::Number(self.to_number(&prim)?)),
        }
    }

    pub fn to_bigint(&mut self, v: &Value) -> error::Result<num_bigint::BigInt> {
        let prim = match v {
            Value::Object(_) => self.to_primitive_number(v)?,
            _ => v.clone(),
        };
        match prim {
            Value::BigInt(n) => Ok(n),
            Value::Bool(b) => Ok(num_bigint::BigInt::from(if b { 1 } else { 0 })),
            Value::String(s) => Self::string_to_bigint(&s)
                .ok_or_else(|| Error::syntax(format!("Cannot convert {} to a BigInt", s))),
            Value::Undefined
            | Value::Null
            | Value::Number(_)
            | Value::Symbol(_)
            | Value::PrivateName(_) => {
                Err(Error::type_err("Cannot convert to a BigInt".to_string()))
            }
            Value::Object(_) | Value::Reference(_) => {
                Err(Error::type_err("Cannot convert to a BigInt".to_string()))
            }
        }
    }

    pub fn to_string(&mut self, v: &Value) -> error::Result<Arc<str>> {
        Ok(match v {
            Value::Undefined => Arc::from("undefined"),
            Value::Null => Arc::from("null"),
            Value::Bool(b) => Arc::from(b.to_string().as_str()),
            Value::Number(n) => Arc::from(crate::value::num_to_string(*n).as_str()),
            Value::String(s) => s.clone(),
            Value::BigInt(n) => Arc::from(n.to_string().as_str()),
            Value::Object(idx) => {
                let prim = self.to_primitive_hint(v, true)?;
                if matches!(prim, Value::Object(_)) {
                    return Err(Error::type_err(
                        "Cannot convert object to primitive value".to_string(),
                    ));
                }
                return self.to_string(&prim);
            }
            Value::Symbol(_) => {
                return Err(Error::type_err(
                    "Cannot convert Symbol to string".to_string(),
                ));
            }
            Value::PrivateName(_) => {
                return Err(Error::type_err(
                    "Cannot convert private name to string".to_string(),
                ));
            }
            Value::Reference(_) => Arc::from("[reference]"),
        })
    }

    /// Default-hint ToPrimitive (used by binary `+` and `==`): valueOf then
    /// toString, with "default" passed to @@toPrimitive.
    pub fn to_primitive(&mut self, v: &Value) -> error::Result<Value> {
        self.to_primitive_ex(v, false, "default")
    }
    /// Number-hint ToPrimitive (used by unary `+`, arithmetic, Number()):
    /// valueOf then toString, with "number" passed to @@toPrimitive.
    pub fn to_primitive_number(&mut self, v: &Value) -> error::Result<Value> {
        self.to_primitive_ex(v, false, "number")
    }
    /// Convert a value to a primitive per the ES OrdinaryToPrimitive
    /// abstract operation. For objects, invoke `valueOf` then `toString`
    /// (default/number hint) or `toString` then `valueOf` (string hint),
    /// returning the first non-object result. Arrays/objects without custom
    /// methods fall back to their default string form.
    pub fn to_primitive_hint(&mut self, v: &Value, string_hint: bool) -> error::Result<Value> {
        let hint = if string_hint { "string" } else { "number" };
        self.to_primitive_ex(v, string_hint, hint)
    }
    /// Shared ToPrimitive body. `string_hint` controls the valueOf/toString
    /// order; `hint` is the string passed to @@toPrimitive.
    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn to_primitive_ex(
        &mut self,
        v: &Value,
        string_hint: bool,
        hint: &'static str,
    ) -> error::Result<Value> {
        match v {
            Value::Object(_) => {
                // ES ToPrimitive: an object may define @@toPrimitive, which
                // takes precedence over valueOf/toString and receives the hint.
                {
                    let tp_key =
                        crate::value::PropertyKey::Symbol(self.well_known_symbols.to_primitive);
                    let method = self.get_property_by_key(v, &tp_key)?;
                    if !method.is_undefined() && !method.is_null() {
                        if !crate::builtins::is_callable(&method, &self.heap) {
                            return Err(Error::type_err(
                                "@@toPrimitive method is not callable".to_string(),
                            ));
                        }
                        let hint_str = Arc::from(hint);
                        let result = self.call_function(
                            &method,
                            &[Value::String(hint_str)],
                            Some(v.clone()),
                        )?;
                        if matches!(result, Value::Object(_)) {
                            return Err(Error::type_err(
                                "Cannot convert object to primitive value".to_string(),
                            ));
                        }
                        return Ok(result);
                    }
                }
                let methods: [&str; 2] = if string_hint {
                    ["toString", "valueOf"]
                } else {
                    ["valueOf", "toString"]
                };
                for name in methods {
                    let method = self.get_property(v, name)?;
                    if crate::builtins::is_callable(&method, &self.heap) {
                        let result = self.call_function(&method, &[], Some(v.clone()))?;
                        if !matches!(result, Value::Object(_)) {
                            return Ok(result);
                        }
                    }
                }
                // Per spec, OrdinaryToPrimitive throws when neither method
                // yields a primitive.
                Err(Error::type_err(
                    "Cannot convert object to primitive value".to_string(),
                ))
            }
            _ => Ok(v.clone()),
        }
    }

    /// Coerce a `Value` with ES `ToPropertyKey`, preserving Symbol keys.
    pub fn to_property_key_value(&mut self, v: &Value) -> error::Result<Value> {
        match v {
            Value::Symbol(_) => Ok(v.clone()),
            _ => {
                let prim = self.to_primitive_hint(v, true)?;
                match prim {
                    Value::Symbol(_) => Ok(prim),
                    _ => Ok(Value::String(self.to_string(&prim)?)),
                }
            }
        }
    }

    /// Coerce a `Value` to a property key as a `String`.
    ///
    /// Symbols cannot be converted to a string key and return `Err` (a Symbol
    /// must be looked up via [`get_property_key`] / [`set_property_key`] using
    /// the `Value::Symbol` directly).
    pub fn to_property_key(&mut self, v: &Value) -> error::Result<String> {
        match self.to_property_key_value(v)? {
            Value::String(s) => Ok(s.to_string()),
            Value::Symbol(_) => Err(Error::type_err(
                "Cannot convert a Symbol value to a string key".to_string(),
            )),
            _ => unreachable!("ToPropertyKey returns only String or Symbol"),
        }
    }

    pub(crate) fn coerce_property_key_record(
        &mut self,
        v: &Value,
    ) -> error::Result<crate::value::PropertyKey> {
        match self.to_property_key_value(v)? {
            Value::String(s) => Ok(crate::value::PropertyKey::from_rc(s)),
            Value::Symbol(id) => Ok(crate::value::PropertyKey::Symbol(id)),
            _ => unreachable!("ToPropertyKey returns only String or Symbol"),
        }
    }

    /// Get a property by a `Value` key, supporting string keys (via the
    /// existing `get_property(&str)` path) and Symbol keys (looked up directly
    /// in the object's `props` map as `PropertyKey::Symbol`).
    pub fn get_property_key(&mut self, obj: &Value, key: &Value) -> error::Result<Value> {
        // Per spec, base is checked for null/undefined BEFORE ToPropertyKey.
        match obj {
            Value::Null | Value::Undefined => {
                // Don't call ToPropertyKey on the key (which might throw);
                // just use a placeholder for the error message.
                return Err(Error::type_err(format!(
                    "Cannot read properties of {}",
                    obj.type_of()
                )));
            }
            _ => {}
        }
        let property_key = self.coerce_property_key_record(key)?;
        if let Some(name) = property_key.as_str() {
            return self.get_property(obj, name);
        }
        match obj {
            Value::String(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::Bool(_)
            | Value::Symbol(_) => {
                let proto = self.current_realm_primitive_prototype(obj);
                if proto.is_undefined() {
                    Ok(Value::Undefined)
                } else {
                    self.get_property_key_rx(&proto, &property_key, obj.clone())
                }
            }
            _ => self.get_property_by_key(obj, &property_key),
        }
    }

    /// Set a property by a `Value` key, supporting string and Symbol keys.
    pub fn set_property_key(
        &mut self,
        obj: &Value,
        key: &Value,
        value: Value,
    ) -> error::Result<()> {
        if matches!(obj, Value::Null | Value::Undefined) {
            return Err(Error::type_err("Cannot set property of primitive"));
        }
        let property_key = self.coerce_property_key_record(key)?;
        if let Some(name) = property_key.as_str() {
            return self.set_property(obj, name, value);
        }
        if matches!(obj, Value::Object(_)) {
            let success =
                self.try_set_property_key_with_receiver(obj, &property_key, value, obj)?;
            if !success && self.current_strict() {
                return Err(Error::type_err(
                    "Cannot assign to read only Symbol property",
                ));
            }
            Ok(())
        } else if self.current_strict() {
            Err(Error::type_err(
                "Cannot set property of primitive".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    /// Look up a property by a `PropertyKey` (string or Symbol), walking the
    /// prototype chain. Used internally by [`get_property_key`] for Symbol
    /// keys and by the iterator protocol for `Symbol.iterator`.
    pub fn get_property_by_key(
        &mut self,
        obj: &Value,
        key: &crate::value::PropertyKey,
    ) -> error::Result<Value> {
        if let Some(name) = key.as_str() {
            return self.get_property(obj, name);
        }
        match obj {
            Value::Object(_) => self.get_property_key_rx(obj, key, obj.clone()),
            Value::String(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::Bool(_)
            | Value::Symbol(_) => {
                let prototype = self.current_realm_primitive_prototype(obj);
                if prototype.is_undefined() {
                    Ok(Value::Undefined)
                } else {
                    self.get_property_key_rx(&prototype, key, obj.clone())
                }
            }
            _ => Ok(Value::Undefined),
        }
    }

    pub(crate) fn property_key_to_value(key: &crate::value::PropertyKey) -> Value {
        match key {
            crate::value::PropertyKey::Str(s) => Value::String(s.clone()),
            crate::value::PropertyKey::Symbol(id) => Value::Symbol(*id),
        }
    }

    fn has_own_property_key_raw(&self, obj: &Value, key: &crate::value::PropertyKey) -> bool {
        let Value::Object(idx) = obj else {
            return false;
        };
        if let Some(desc) = self.typed_array_integer_index_own_property_descriptor(obj, key) {
            return desc.is_some();
        }
        self.heap.with_obj(idx.0, |o| {
            if let HeapObj::ModuleNamespace(namespace) = o {
                if key
                    .as_str()
                    .is_some_and(|name| namespace.exports.lock().contains_key(name))
                {
                    return true;
                }
            }
            if o.props().lock().contains_key(key) {
                return true;
            }
            if let HeapObj::Array(a) = o {
                if key.as_str().is_some_and(|s| s == "length") {
                    return !a.is_arguments.load(std::sync::atomic::Ordering::Relaxed);
                }
                return key
                    .as_str()
                    .and_then(crate::value::parse_array_index)
                    .is_some_and(|i| a.is_dense_present(i));
            }
            if let HeapObj::Object(od) = o {
                if let Some(Value::String(s)) = od.primitive.lock().clone() {
                    return key.as_str().is_some_and(|name| {
                        let len = crate::value::utf16_len(&s);
                        name == "length"
                            || crate::builtins::canonical_string_index(key).is_some_and(|i| i < len)
                    });
                }
            }
            false
        })
    }

    pub(crate) fn own_property_descriptor_for_proxy_invariant(
        &self,
        obj: &Value,
        key: &crate::value::PropertyKey,
    ) -> Option<crate::value::PropertyDescriptor> {
        let Value::Object(idx) = obj else {
            return None;
        };
        if let Some(desc) = self.typed_array_integer_index_own_property_descriptor(obj, key) {
            return desc;
        }
        let ordinary = self
            .heap
            .with_obj(idx.0, |o| o.props().lock().get(key).cloned());
        if ordinary.is_some() {
            return ordinary;
        }
        let namespace_binding = self.heap.with_obj(idx.0, |o| {
            if let HeapObj::ModuleNamespace(namespace) = o {
                return key
                    .as_str()
                    .and_then(|name| namespace.exports.lock().get(name).cloned());
            }
            None
        });
        if let Some((env, name)) = namespace_binding {
            let value = crate::environment::get_checked(&self.heap, env, &name)
                .ok()
                .flatten()
                .unwrap_or(Value::Undefined);
            let mut desc = crate::value::PropertyDescriptor::data(value);
            desc.writable = true;
            desc.enumerable = true;
            desc.configurable = false;
            return Some(desc);
        }
        if key.as_str().is_some_and(|s| s == "length") {
            let array_length = self.heap.with_obj(idx.0, |o| {
                let HeapObj::Array(array) = o else {
                    return None;
                };
                if array
                    .is_arguments
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    return None;
                }
                Some(
                    array
                        .items
                        .lock()
                        .len()
                        .max(array.sparse_max.lock().unwrap_or(0)),
                )
            });
            if let Some(length) = array_length {
                let mut desc = crate::value::PropertyDescriptor::data(Value::Number(length as f64));
                desc.enumerable = false;
                desc.configurable = false;
                return Some(desc);
            }
        }
        let string_exotic = self.heap.with_obj(idx.0, |o| {
            if let HeapObj::Object(od) = o {
                return od.primitive.lock().clone();
            }
            None
        });
        if let Some(Value::String(s)) = string_exotic {
            let value = if key.as_str() == Some("length") {
                Some(Value::Number(crate::value::utf16_len(&s) as f64))
            } else {
                crate::builtins::canonical_string_index(key)
                    .and_then(|index| crate::value::utf16_get(&s, index))
                    .map(|unit| {
                        Value::String(Arc::from(crate::value::utf16_to_string(&[unit]).as_str()))
                    })
            };
            if let Some(value) = value {
                let mut desc = crate::value::PropertyDescriptor::data(value);
                desc.writable = false;
                desc.enumerable = key.as_str() != Some("length");
                desc.configurable = false;
                return Some(desc);
            }
        }
        let index = key.as_str().and_then(crate::value::parse_array_index)?;
        self.array_index_own_property_descriptor(idx.0, index, key)
    }

    /// Does `obj` (or its prototype chain) have an own/inherited property for
    /// the given `PropertyKey`? This is RuJa's internal `[[HasProperty]]`
    /// operation, including Proxy `has` traps.
    pub fn has_property_key(
        &mut self,
        obj: &Value,
        key: &crate::value::PropertyKey,
    ) -> error::Result<bool> {
        self.has_property_key_with_mode(obj, key, 0)
    }

    pub(crate) fn has_property_with_free_ordinary_edge(
        &mut self,
        obj: &Value,
        name: &str,
    ) -> error::Result<bool> {
        self.has_property_key_with_mode(obj, &crate::value::PropertyKey::from(name), 1)
    }

    fn has_property_key_with_mode(
        &mut self,
        obj: &Value,
        key: &crate::value::PropertyKey,
        ordinary_edge_credit: usize,
    ) -> error::Result<bool> {
        let mut traversal =
            self.try_new_property_traversal(std::slice::from_ref(obj), ordinary_edge_credit)?;
        let root_pin = self.pin(obj);
        let result = (|| {
            let mut current = obj.clone();
            loop {
                let Value::Object(idx) = &current else {
                    return Ok(false);
                };
                let idx = *idx;
                let proxy_info = self.heap.with_obj(idx.0, |object| {
                    let HeapObj::Proxy(proxy) = object else {
                        return None;
                    };
                    if *proxy.revoked.lock() {
                        return Some(Err(Error::type_err(
                            "Cannot perform 'has' on a proxy that has been revoked",
                        )));
                    }
                    Some(Ok((proxy.target.clone(), proxy.handler.clone())))
                });
                if let Some(proxy_info) = proxy_info {
                    let (target, handler) = proxy_info?;
                    traversal.note_proxy();
                    self.consume_fuel()?;
                    let proxy_pins = self.pin_many(&[target.clone(), handler.clone()]);
                    let proxy_result = (|| {
                        let trap = self.get_proxy_method(&handler, "has")?;
                        if trap.is_nullish() {
                            return Ok(None);
                        }
                        if !crate::builtins::is_callable(&trap, &self.heap) {
                            return Err(Error::type_err("Proxy has trap is not callable"));
                        }
                        let trap_result = self.call_function(
                            &trap,
                            &[target.clone(), Self::property_key_to_value(key)],
                            Some(handler.clone()),
                        )?;
                        let boolean_trap_result = self.to_boolean(&trap_result);
                        if !boolean_trap_result {
                            let target_descriptor =
                                crate::builtins::own_property_descriptor_for_key_or_throw(
                                    self, &target, key,
                                )?;
                            if let Some(target_descriptor) = target_descriptor {
                                if !target_descriptor.configurable {
                                    return Err(Error::type_err(
                                        "Proxy has trap cannot hide non-configurable property",
                                    ));
                                }
                                if !self.is_extensible(&target)? {
                                    return Err(Error::type_err(
                                        "Proxy has trap cannot hide non-extensible target property",
                                    ));
                                }
                            }
                        }
                        Ok(Some(boolean_trap_result))
                    })();
                    self.unpin_many(proxy_pins);
                    match proxy_result? {
                        Some(value) => return Ok(value),
                        None => {
                            self.advance_property_edge(&mut traversal, idx, &target, false)?;
                            current = target;
                            continue;
                        }
                    }
                }

                if let Some(has_index) = self.typed_array_integer_index_has_property(&current, key)
                {
                    return Ok(has_index);
                }
                if self.has_own_property_key_raw(&current, key) {
                    return Ok(true);
                }
                let prototype = self.heap.with_obj(idx.0, |object| {
                    object.proto().lock().clone().unwrap_or(Value::Undefined)
                });
                let Value::Object(prototype_idx) = &prototype else {
                    return Ok(false);
                };
                let prototype_is_proxy = self.heap.with_obj(prototype_idx.0, |object| {
                    matches!(object, HeapObj::Proxy(_))
                });
                self.advance_property_edge(&mut traversal, idx, &prototype, !prototype_is_proxy)?;
                current = prototype;
            }
        })();
        self.unpin_many(root_pin + traversal.pin_count());
        result
    }

    /// Does `obj` have an **own** property (not inherited)?
    pub fn has_own_property(&self, obj: &Value, name: &str) -> bool {
        if let Value::Object(idx) = obj {
            let pkey = crate::value::PropertyKey::from(name);
            if let Some(desc) = self.typed_array_integer_index_own_property_descriptor(obj, &pkey) {
                return desc.is_some();
            }
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::ModuleNamespace(namespace) = o {
                    if namespace.exports.lock().contains_key(name) {
                        return true;
                    }
                }
                o.props().lock().contains_key(&pkey)
            })
        } else {
            false
        }
    }

    /// Does `obj` (or its prototype chain) have a named property? Used by the
    /// `with` statement to decide whether to assign to a `with` object.
    /// Does `obj` (or its prototype chain) have a named property? Unlike the
    /// previous undefined-sentinel check, this walks the own-property maps so
    /// a property whose value is `undefined` is still "present" (per spec
    /// `[[HasProperty]]`). Used by the `with` statement.
    pub fn has_property(&mut self, obj: &Value, name: &str) -> error::Result<bool> {
        // Fast path: objects with a props map walk own + proto for the key.
        let pkey = crate::value::PropertyKey::from(name);
        if self.has_property_key(obj, &pkey)? {
            return Ok(true);
        }
        // Arrays expose indexed "properties" and `length`; strings expose
        // indexed chars and `length`. Treat those as present.
        match obj {
            Value::Object(idx) => {
                let (is_arr, has_dense_index, has_boxed_string_property) =
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Array(a) = o {
                            if name == "length"
                                && a.is_arguments.load(std::sync::atomic::Ordering::Relaxed)
                            {
                                return (false, false, false);
                            }
                            let has = crate::value::parse_array_index(name)
                                .is_some_and(|i| a.is_dense_present(i));
                            return (true, has, false);
                        }
                        if let HeapObj::Object(od) = o {
                            if let Some(Value::String(s)) = od.primitive.lock().clone() {
                                let len = crate::value::utf16_len(&s);
                                let has = name == "length"
                                    || crate::builtins::canonical_string_index_name(name)
                                        .is_some_and(|i| i < len);
                                return (false, false, has);
                            }
                        }
                        (false, false, false)
                    });
                if is_arr && (name == "length" || has_dense_index) {
                    return Ok(true);
                }
                if has_boxed_string_property {
                    return Ok(true);
                }
                Ok(false)
            }
            Value::String(st) => {
                let len = crate::value::utf16_len(st);
                Ok(name == "length"
                    || crate::builtins::canonical_string_index_name(name).is_some_and(|i| i < len))
            }
            _ => Ok(false),
        }
    }

    pub(crate) fn with_object_has_binding(
        &mut self,
        obj: &Value,
        name: &str,
    ) -> error::Result<bool> {
        if !self.has_property(obj, name)? {
            return Ok(false);
        }

        let unscopables_key = Value::Symbol(self.well_known_symbols.unscopables);
        let unscopables = self.get_property_key(obj, &unscopables_key)?;
        if !matches!(unscopables, Value::Object(_)) {
            return Ok(true);
        }

        let blocked = self.get_property(&unscopables, name)?;
        Ok(!self.to_boolean(&blocked))
    }

    /// Does `obj` have an OWN property named `name` (not inherited)?
    /// Used by ToPropertyDescriptor (Object.defineProperty) to tell a field
    /// that was explicitly set to `undefined` from a field that is simply
    /// absent on the descriptor object.
    pub fn has_own(&self, obj: &Value, name: &str) -> bool {
        let pkey = crate::value::PropertyKey::from(name);
        if let Some(desc) = self.typed_array_integer_index_own_property_descriptor(obj, &pkey) {
            return desc.is_some();
        }
        match obj {
            Value::Object(idx) => self.heap.with_obj(idx.0, |o| {
                if let HeapObj::ModuleNamespace(namespace) = o {
                    if namespace.exports.lock().contains_key(name) {
                        return true;
                    }
                }
                if let HeapObj::Array(a) = o {
                    if name == "length" {
                        if a.is_arguments.load(std::sync::atomic::Ordering::Relaxed) {
                            return o.props().lock().contains_key(&pkey);
                        }
                        return true;
                    }
                    if let Some(i) = crate::value::parse_array_index(name) {
                        return a.is_dense_present(i);
                    }
                    // array extra props live in props()
                }
                o.props().lock().contains_key(&pkey)
            }),
            Value::String(st) => {
                let len = crate::value::utf16_len(st);
                name == "length"
                    || crate::builtins::canonical_string_index_name(name).is_some_and(|i| i < len)
            }
            _ => false,
        }
    }

    pub fn to_boolean(&self, v: &Value) -> bool {
        v.is_truthy()
    }

    pub fn strict_eq(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            (Value::Number(x), Value::Number(y)) => {
                if x.is_nan() || y.is_nan() {
                    false
                } else {
                    x == y
                }
            }
            (Value::Bool(x), Value::Bool(y)) => x == y,
            (Value::String(x), Value::String(y)) => x == y,
            (Value::Object(x), Value::Object(y)) => x == y,
            (Value::Symbol(x), Value::Symbol(y)) => x == y,
            (Value::BigInt(x), Value::BigInt(y)) => x == y,
            _ => false,
        }
    }

    pub fn loose_eq(&mut self, a: &Value, b: &Value) -> error::Result<bool> {
        if std::mem::discriminant(a) == std::mem::discriminant(b) {
            return Ok(self.strict_eq(a, b));
        }
        Ok(match (a, b) {
            (Value::Null, Value::Undefined) | (Value::Undefined, Value::Null) => true,
            (Value::Number(_), Value::String(_)) => {
                let bn = self.to_number(b)?;
                self.strict_eq(a, &Value::Number(bn))
            }
            (Value::String(_), Value::Number(_)) => {
                let an = self.to_number(a)?;
                self.strict_eq(&Value::Number(an), b)
            }
            (Value::Bool(_), _) => {
                let an = self.to_number(a)?;
                self.loose_eq(&Value::Number(an), b)?
            }
            (_, Value::Bool(_)) => {
                let bn = self.to_number(b)?;
                self.loose_eq(a, &Value::Number(bn))?
            }
            // Object vs primitive: ToPrimitive the object, then compare.
            (Value::Object(_), _) if !b.is_object() => {
                let ap = self.to_primitive(a)?;
                self.loose_eq(&ap, b)?
            }
            (_, Value::Object(_)) if !a.is_object() => {
                let bp = self.to_primitive(b)?;
                self.loose_eq(a, &bp)?
            }
            // BigInt vs Number: compare numerically.
            (Value::BigInt(x), Value::Number(y)) => Self::number_to_bigint_exact(*y)
                .map(|v| *x == v)
                .unwrap_or(false),
            (Value::Number(x), Value::BigInt(y)) => Self::number_to_bigint_exact(*x)
                .map(|v| v == *y)
                .unwrap_or(false),
            // BigInt vs String: parse the string, then compare.
            (Value::BigInt(x), Value::String(s)) => {
                Self::string_to_bigint(s).map(|v| v == *x).unwrap_or(false)
            }
            (Value::String(s), Value::BigInt(y)) => {
                Self::string_to_bigint(s).map(|v| v == *y).unwrap_or(false)
            }
            _ => false,
        })
    }

    // ---- property access ----

    pub fn get_property(&mut self, obj: &Value, key: &str) -> error::Result<Value> {
        match obj {
            Value::String(s) => {
                if key == "length" {
                    return Ok(Value::Number(crate::value::utf16_len(s) as f64));
                }
                if let Some(idx) = crate::builtins::canonical_string_index_name(key) {
                    if let Some(unit) = crate::value::utf16_get(s, idx) {
                        return Ok(Value::String(Arc::from(
                            crate::value::utf16_to_string(&[unit]).as_str(),
                        )));
                    }
                    return Ok(Value::Undefined);
                }
                self.get_proto_property(obj, key)
            }
            Value::Number(_) => self.get_proto_property(obj, key),
            Value::BigInt(_) => self.get_proto_property(obj, key),
            Value::Bool(_) => self.get_proto_property(obj, key),
            Value::Symbol(_) => self.get_proto_property(obj, key),
            Value::Undefined | Value::Null => Err(Error::type_err(format!(
                "Cannot read properties of {} (reading '{}')",
                obj.type_of(),
                key
            ))),
            Value::Object(_) => self.get_property_key_direct(
                obj,
                &crate::value::PropertyKey::from(key),
                obj.clone(),
            ),
            #[allow(unreachable_patterns)]
            _ => Ok(Value::Undefined),
        }
    }
}
