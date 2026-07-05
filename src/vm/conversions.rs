//! Type conversion helpers (ToInt32, ToUint32, ToNumber, ToString,
//! ToPrimitive) split from vm/mod.rs for readability.

use super::*;
use crate::error::{self, Error};
use crate::value::HeapObj;
use crate::value::Value;
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

impl Vm {
    /// float, or NaN if it does not parse.
    pub(crate) fn string_to_number(s: &str) -> f64 {
        let t = s.trim();
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
        if digits.is_empty() {
            return f64::NAN;
        }
        match u64::from_str_radix(digits, radix) {
            Ok(n) => n as f64,
            Err(_) => f64::NAN,
        }
    }

    pub(crate) fn string_to_bigint(s: &str) -> Option<num_bigint::BigInt> {
        let t = s.trim();
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
            Value::Undefined | Value::Null | Value::Number(_) | Value::Symbol(_) => {
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
                let is_array = self
                    .heap
                    .with_obj(idx.0, |obj| matches!(obj, HeapObj::Array(_)));
                if is_array {
                    // join items outside the borrow
                    let items = self.heap.with_obj(idx.0, |obj| {
                        if let HeapObj::Array(a) = obj {
                            a.items.lock().clone()
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
                                self.to_string(i).map(|s| s.to_string()).unwrap_or_default()
                            }
                        })
                        .collect();
                    Arc::from(parts.join(",").as_str())
                } else {
                    let prim = self.to_primitive_hint(v, true)?;
                    if matches!(prim, Value::Object(_)) {
                        return Err(Error::type_err(
                            "Cannot convert object to primitive value".to_string(),
                        ));
                    }
                    return self.to_string(&prim);
                }
            }
            Value::Symbol(_) => {
                return Err(Error::type_err(
                    "Cannot convert Symbol to string".to_string(),
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
                    if matches!(method, Value::Object(_)) {
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
                let is_date_default = if hint == "default" {
                    match v {
                        Value::Object(idx) => self.heap.with_obj(idx.0, |o| {
                            o.proto().lock().as_ref() == Some(&self.date_proto)
                        }),
                        _ => false,
                    }
                } else {
                    false
                };
                let effective_string_hint = string_hint || is_date_default;

                // Arrays have a well-defined default toString (join with ",");
                // honor it directly rather than looking up a method that may
                // not be installed on Array.prototype yet.
                let is_array = match v {
                    Value::Object(idx) => self
                        .heap
                        .with_obj(idx.0, |obj| matches!(obj, HeapObj::Array(_))),
                    _ => false,
                };
                let methods: [&str; 2] = if effective_string_hint {
                    ["toString", "valueOf"]
                } else {
                    ["valueOf", "toString"]
                };
                if is_array && !effective_string_hint {
                    // valueOf on an array returns the array (object), so skip
                    // straight to toString to avoid a pointless call.
                    return Ok(Value::String(self.to_string(v)?));
                }
                for name in methods {
                    let method = self.get_property(v, name)?;
                    if matches!(method, Value::Object(_)) {
                        let result = self.call_function(&method, &[], Some(v.clone()))?;
                        if !matches!(result, Value::Object(_)) {
                            return Ok(result);
                        }
                    }
                }
                // Both returned objects (or were missing): fall back to a
                // best-effort string form.
                // Both returned objects (or were missing): per spec
                // OrdinaryToPrimitive throws a TypeError when neither yields
                // a primitive.
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
        match key {
            Value::Symbol(id) => {
                let pkey = crate::value::PropertyKey::Symbol(*id);
                self.get_property_by_key(obj, &pkey)
            }
            other => {
                let s = self.to_property_key(other)?;
                self.get_property(obj, &s)
            }
        }
    }

    /// Set a property by a `Value` key, supporting string and Symbol keys.
    pub fn set_property_key(
        &mut self,
        obj: &Value,
        key: &Value,
        value: Value,
    ) -> error::Result<()> {
        match key {
            Value::Symbol(id) => {
                if let Value::Object(idx) = obj {
                    let pkey = crate::value::PropertyKey::Symbol(*id);
                    self.heap.with_obj(idx.0, |o| {
                        o.props()
                            .lock()
                            .insert(pkey, crate::value::PropertyDescriptor::data(value.clone()));
                    });
                    Ok(())
                } else {
                    Err(Error::type_err(
                        "Cannot set property of primitive".to_string(),
                    ))
                }
            }
            other => {
                let s = self.to_property_key(other)?;
                self.set_property(obj, &s, value)
            }
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
        let mut cur = obj.clone();
        let mut depth = 0;
        while let Value::Object(idx) = &cur {
            if depth > 1024 {
                break;
            }
            depth += 1;
            let (found, proto) = self.heap.with_obj(idx.0, |o| {
                let props = o.props();
                let v = props.lock().get(key).map(|d| d.value.clone());
                let proto = o.proto().lock().clone();
                (v, proto)
            });
            if let Some(v) = found {
                return Ok(v);
            }
            cur = proto.unwrap_or(Value::Undefined);
            if cur.is_undefined() {
                break;
            }
        }
        Ok(Value::Undefined)
    }

    /// Does `obj` (or its prototype chain) have an own/inherited property for
    /// the given `PropertyKey`? Used by the iterator protocol to detect a
    /// user-defined `Symbol.iterator`.
    pub fn has_property_key(&self, obj: &Value, key: &crate::value::PropertyKey) -> bool {
        let mut cur = obj.clone();
        let mut depth = 0;
        while let Value::Object(idx) = &cur {
            if depth > 1024 {
                break;
            }
            depth += 1;
            let (has, proto) = self.heap.with_obj(idx.0, |o| {
                (o.props().lock().contains_key(key), o.proto().lock().clone())
            });
            if has {
                return true;
            }
            cur = proto.unwrap_or(Value::Undefined);
            if cur.is_undefined() {
                break;
            }
        }
        false
    }

    /// Does `obj` have an **own** property (not inherited)? Used by the
    /// `with`-statement HasBinding check, which per spec only considers own
    /// properties of the binding object.
    pub fn has_own_property(&self, obj: &Value, name: &str) -> bool {
        if let Value::Object(idx) = obj {
            let pkey = crate::value::PropertyKey::from(name);
            self.heap
                .with_obj(idx.0, |o| o.props().lock().contains_key(&pkey))
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
        if self.has_property_key(obj, &pkey) {
            return Ok(true);
        }
        // Arrays expose indexed "properties" and `length`; strings expose
        // indexed chars and `length`. Treat those as present.
        match obj {
            Value::Object(idx) => {
                let (is_arr, len) = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Array(a) = o {
                        (true, a.items.lock().len())
                    } else {
                        (false, 0)
                    }
                });
                if is_arr && (name == "length" || name.parse::<usize>().is_ok_and(|i| i < len)) {
                    return Ok(true);
                }
                Ok(false)
            }
            Value::String(st) => {
                let len = crate::value::utf16_len(st);
                Ok(name == "length" || name.parse::<usize>().is_ok_and(|i| i < len))
            }
            _ => Ok(false),
        }
    }

    /// Does `obj` have an OWN property named `name` (not inherited)?
    /// Used by ToPropertyDescriptor (Object.defineProperty) to tell a field
    /// that was explicitly set to `undefined` from a field that is simply
    /// absent on the descriptor object.
    pub fn has_own(&self, obj: &Value, name: &str) -> bool {
        let pkey = crate::value::PropertyKey::from(name);
        match obj {
            Value::Object(idx) => self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Array(a) = o {
                    if name == "length" {
                        return true;
                    }
                    if let Ok(i) = name.parse::<usize>() {
                        return i < a.items.lock().len();
                    }
                    // array extra props live in props()
                }
                o.props().lock().contains_key(&pkey)
            }),
            Value::String(st) => {
                let len = crate::value::utf16_len(st);
                name == "length" || name.parse::<usize>().is_ok_and(|i| i < len)
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
                if let Ok(idx) = key.parse::<usize>() {
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
            Value::Object(idx) => {
                // TypedArray index access: read from buffer.
                let ta_info = self.heap.with_obj(idx.0, |o| {
                    if let crate::value::HeapObj::TypedArray(t) = o {
                        Some((t.kind, t.buffer.lock().len()))
                    } else {
                        None
                    }
                });
                if let Some((kind, buf_len)) = ta_info {
                    if key == "length" {
                        return Ok(Value::Number(buf_len as f64));
                    }
                    if key == "byteLength" {
                        return Ok(Value::Number((buf_len * kind.element_size()) as f64));
                    }
                    if key == "byteOffset" {
                        return Ok(Value::Number(0.0));
                    }
                    if let Ok(i) = key.parse::<usize>() {
                        let elem_size = kind.element_size();
                        let offset = i * elem_size;
                        if offset + elem_size <= buf_len {
                            let val = self.heap.with_obj(idx.0, |o| {
                                if let crate::value::HeapObj::TypedArray(t) = o {
                                    let buffer = t.buffer.lock();
                                    match kind {
                                        crate::value::TypedArrayKind::Uint8 => buffer[i] as f64,
                                        _ => buffer[i] as f64, // simplified for now
                                    }
                                } else {
                                    f64::NAN
                                }
                            });
                            return Ok(Value::Number(val));
                        }
                        return Ok(Value::Undefined);
                    }
                    // Non-index property: fall through to object props.
                }
                let array_buffer_len = self.heap.with_obj(idx.0, |o| {
                    if let crate::value::HeapObj::ArrayBuffer(buffer) = o {
                        if buffer.detached.load(std::sync::atomic::Ordering::Relaxed) {
                            Some(0)
                        } else {
                            Some(buffer.bytes.lock().len())
                        }
                    } else {
                        None
                    }
                });
                if let Some(len) = array_buffer_len {
                    if key == "byteLength" {
                        return Ok(Value::Number(len as f64));
                    }
                }
                let data_view_info = self.heap.with_obj(idx.0, |o| {
                    if let crate::value::HeapObj::DataView(view) = o {
                        Some((view.buffer.clone(), view.byte_offset, view.byte_length))
                    } else {
                        None
                    }
                });
                if let Some((buffer, byte_offset, byte_length)) = data_view_info {
                    match key {
                        "buffer" => return Ok(buffer),
                        "byteOffset" | "byteLength" => {
                            let detached = match &buffer {
                                Value::Object(buffer_idx) => {
                                    self.heap.with_obj(buffer_idx.0, |o| {
                                        if let crate::value::HeapObj::ArrayBuffer(array_buffer) = o
                                        {
                                            array_buffer
                                                .detached
                                                .load(std::sync::atomic::Ordering::Relaxed)
                                        } else {
                                            false
                                        }
                                    })
                                }
                                _ => false,
                            };
                            if detached {
                                return Err(Error::type_err(
                                    "DataView getter on detached buffer".to_string(),
                                ));
                            }
                            if key == "byteOffset" {
                                return Ok(Value::Number(byte_offset as f64));
                            }
                            return Ok(Value::Number(byte_length as f64));
                        }
                        _ => {}
                    }
                }
                // Proxy trap: if this object is a Proxy, call handler.get.
                let proxy_info = self.heap.with_obj(idx.0, |o| {
                    if let crate::value::HeapObj::Proxy(p) = o {
                        if *p.revoked.lock() {
                            return Some(Err(crate::error::Error::type_err(
                                "Cannot perform 'get' on a proxy that has been revoked".to_string(),
                            )));
                        }
                        Some(Ok((p.target.clone(), p.handler.clone())))
                    } else {
                        None
                    }
                });
                if let Some(result) = proxy_info {
                    match result {
                        Err(e) => return Err(e),
                        Ok((target, handler)) => {
                            let key_val = Value::String(Arc::from(key));
                            let trap = self.get_property(&handler, "get")?;
                            if !trap.is_undefined() {
                                let receiver = obj.clone();
                                return self.call_function(
                                    &trap,
                                    &[target, key_val, receiver],
                                    Some(handler),
                                );
                            }
                            // No trap: forward to target.
                            return self.get_property(&target, key);
                        }
                    }
                }
                // Honor an accessor getter on this object (own property).
                // Inherited accessors are handled by the recursive proto-chain
                // walk below, since `get_property` is called again on the
                // prototype. The getter must be invoked outside the
                // `with_obj` borrow, so we look it up first.
                let pkey = crate::value::PropertyKey::from(key);
                if let Some(getter) = self.heap.with_obj(idx.0, |o| {
                    o.props().lock().get(&pkey).and_then(|d| {
                        if d.is_accessor {
                            d.get.clone()
                        } else {
                            None
                        }
                    })
                }) {
                    if !getter.is_undefined() {
                        return self.call_function(&getter, &[], Some(obj.clone()));
                    }
                    return Ok(Value::Undefined);
                }
                if key == "__proto__" {
                    if let Some(value) = self.heap.with_obj(idx.0, |o| {
                        o.props().lock().get(&pkey).and_then(|d| {
                            if d.is_accessor {
                                None
                            } else {
                                Some(d.value.clone())
                            }
                        })
                    }) {
                        return Ok(value);
                    }
                }
                // __proto__ getter returns the object's [[Prototype]].
                if key == "__proto__" {
                    return Ok(self
                        .heap
                        .with_obj(idx.0, |o| o.proto().lock().clone().unwrap_or(Value::Null)));
                }
                if let Some(value) = self.heap.with_obj(idx.0, |o| {
                    o.props().lock().get(&pkey).and_then(|d| {
                        if d.is_accessor {
                            None
                        } else {
                            Some(d.value.clone())
                        }
                    })
                }) {
                    return Ok(value);
                }
                // globalThis routes property reads to the global environment.
                let is_global_this = self.heap.with_obj(idx.0, |o| {
                    matches!(o, HeapObj::Object(od) if od.class_name.as_deref() == Some("global"))
                });
                if is_global_this {
                    if let Some(v) = crate::environment::get(&self.heap, self.global, key) {
                        return Ok(v);
                    }
                }
                // array
                let proto = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Array(a) = o {
                        if key == "length" {
                            let len = a.items.lock().len();
                            let sparse = a.sparse_max.lock().unwrap_or(0);
                            return Ok::<Value, Error>(Value::Number(len.max(sparse) as f64));
                        }
                        if let Some(i) = crate::value::parse_array_index(key) {
                            if let Some(mapped) = a.arguments_map.lock().as_ref().and_then(|m| {
                                m.names
                                    .get(i)
                                    .and_then(|n| n.as_ref())
                                    .map(|n| (m.env, n.clone()))
                            }) {
                                if let Some(v) =
                                    crate::environment::get(&self.heap, mapped.0, &mapped.1)
                                {
                                    return Ok(v);
                                }
                            }
                            if let Some(d) = a.props.lock().get(&pkey) {
                                if !d.is_accessor {
                                    return Ok(d.value.clone());
                                }
                            }
                            // Indices beyond the dense cap are stored as named
                            // properties (sparse array); read them from there.
                            if i >= crate::value::MAX_DENSE_ARRAY_LEN {
                                let pkey = crate::value::PropertyKey::from_string(key.to_string());
                                if let Some(d) = a.props.lock().get(&pkey) {
                                    return Ok(d.value.clone());
                                }
                                return Ok(Value::Undefined);
                            }
                            let items = a.items.lock();
                            return Ok(items.get(i).cloned().unwrap_or(Value::Undefined));
                        }
                    }
                    if let HeapObj::Map(m) = o {
                        if key == "size" {
                            return Ok(Value::Number(m.entries.lock().len() as f64));
                        }
                    }
                    if let HeapObj::Set(s) = o {
                        if key == "size" {
                            return Ok(Value::Number(s.items.lock().len() as f64));
                        }
                    }
                    // Boxed String: `new String("abc").length` returns the
                    // string length, and integer indices return characters.
                    if let HeapObj::Object(od) = o {
                        if let Some(Value::String(s)) = od.primitive.lock().clone() {
                            if key == "length" {
                                return Ok(Value::Number(crate::value::utf16_len(&s) as f64));
                            }
                            if let Ok(i) = key.parse::<usize>() {
                                if let Some(unit) = crate::value::utf16_get(&s, i) {
                                    return Ok(Value::String(Arc::from(
                                        crate::value::utf16_to_string(&[unit]).as_str(),
                                    )));
                                }
                                return Ok(Value::Undefined);
                            }
                        }
                    }
                    let props = o.props();
                    if let Some(desc) = props.lock().get(&pkey) {
                        return Ok(desc.value.clone());
                    }
                    // function-specific: .prototype lives in a dedicated field
                    if let HeapObj::Function(f) = o {
                        if key == "prototype" {
                            if let Some(p) = f.prototype.lock().as_ref() {
                                return Ok(p.clone());
                            }
                        }
                        if key == "name" {
                            if let Some(n) = &f.name {
                                return Ok(Value::String(n.clone()));
                            }
                            return Ok(Value::String(Arc::from("")));
                        }
                        if key == "length" {
                            if let crate::value::FunctionKind::Native { length, .. } = &f.kind {
                                return Ok(Value::Number(*length as f64));
                            }
                            if let crate::value::FunctionKind::Interpreted { func } = &f.kind {
                                return Ok(Value::Number(func.length as f64));
                            }
                        }
                    }
                    Ok(Value::Undefined)
                });
                let val = proto?;
                if !val.is_undefined() {
                    return Ok(val);
                }
                // walk proto chain, preserving the original receiver so that
                // getters inherited from a prototype bind `this` to the receiver.
                let p = self.heap.with_obj(idx.0, |o| o.proto().lock().clone());
                if let Some(proto) = p {
                    if !proto.is_undefined() {
                        return self.get_property_rx(&proto, key, obj.clone(), 0);
                    }
                }
                Ok(Value::Undefined)
            }
            #[allow(unreachable_patterns)]
            _ => Ok(Value::Undefined),
        }
    }
}
