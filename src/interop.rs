//! Conversion between `ruja::Value` and `serde_json::Value`.
//!
//! This module is gated behind the `serde` feature flag.

use crate::error;
use crate::value::{GcIdx, HeapObj, Value};
use crate::vm::Vm;
#[cfg(test)]
use std::sync::Arc;

enum Either {
    Arr(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

/// Convert a `ruja::Value` into a `serde_json::Value`.
pub fn to_json_value(vm: &mut Vm, v: &Value) -> serde_json::Value {
    match v {
        Value::Undefined | Value::Symbol(_) | Value::PrivateName(_) | Value::Reference(_) => {
            serde_json::Value::Null
        }
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Number(n) => {
            if let Ok(i) = crate::value::num_to_string(*n).parse::<i64>() {
                serde_json::Value::Number(i.into())
            } else {
                serde_json::Value::Number(
                    serde_json::Number::from_f64(*n).unwrap_or(serde_json::Number::from(0)),
                )
            }
        }
        Value::BigInt(n) => serde_json::Value::String(n.to_string()),
        Value::String(s) => {
            serde_json::Value::String(crate::value::utf16_to_scalar_string_lossy(s))
        }
        Value::Object(idx) => {
            // Extract the data we need from the heap object without
            // holding a borrow on vm.heap during recursive calls.
            let extracted = vm.heap.with_obj(idx.0, |obj| match obj {
                HeapObj::Array(a) => {
                    let items = a.items.lock().clone();
                    Some(Either::Arr(items))
                }
                HeapObj::Object(o) => {
                    let props = o.props.lock().clone();
                    let pairs: Vec<(String, Value)> = props
                        .iter()
                        .filter(|(_, d)| d.enumerable)
                        .filter_map(|(k, d)| {
                            k.as_str().map(|s| {
                                (
                                    crate::value::utf16_to_scalar_string_lossy(&s),
                                    d.value.clone(),
                                )
                            })
                        })
                        .collect();
                    Some(Either::Obj(pairs))
                }
                _ => None,
            });
            match extracted {
                Some(Either::Arr(items)) => {
                    let mut arr = Vec::new();
                    for item in items {
                        arr.push(to_json_value(vm, &item));
                    }
                    serde_json::Value::Array(arr)
                }
                Some(Either::Obj(pairs)) => {
                    let mut map = serde_json::Map::new();
                    for (key, val) in pairs {
                        map.insert(key, to_json_value(vm, &val));
                    }
                    serde_json::Value::Object(map)
                }
                None => serde_json::Value::Null,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{
        PrivateNameKey, PropertyKey, ReferenceBase, ReferenceRecord, ReferencedName,
    };

    #[test]
    fn internal_values_convert_to_json_null() {
        let mut vm = Vm::new().expect("failed to initialize VM");
        let private = Value::PrivateName(PrivateNameKey {
            id: 1,
            description: Arc::from("field"),
        });
        let reference = Value::Reference(Box::new(ReferenceRecord {
            base: ReferenceBase::Unresolvable,
            name: ReferencedName::Property(PropertyKey::from("missing")),
            strict: true,
            this_value: None,
        }));

        assert_eq!(to_json_value(&mut vm, &private), serde_json::Value::Null);
        assert_eq!(to_json_value(&mut vm, &reference), serde_json::Value::Null);
    }

    #[test]
    fn bigint_converts_to_an_exact_decimal_json_string() {
        let mut vm = Vm::new().expect("failed to initialize VM");
        let bigint = Value::bigint(
            num_bigint::BigInt::parse_bytes(b"123456789012345678901234567890", 10).unwrap(),
        );

        assert_eq!(
            to_json_value(&mut vm, &bigint),
            serde_json::Value::String("123456789012345678901234567890".to_string())
        );
    }

    #[test]
    fn serde_strings_enter_as_canonical_utf16() {
        let mut vm = Vm::new().expect("failed to initialize VM");
        let scalar = "\u{F0000}";
        let value = from_json_value(&mut vm, &serde_json::Value::String(scalar.to_string()))
            .expect("string conversion should succeed");
        let Value::String(value) = value else {
            panic!("expected converted string");
        };
        assert_eq!(crate::value::utf16_from_str(&value), [0xDB80, 0xDC00]);
        assert_eq!(
            to_json_value(&mut vm, &Value::String(value.clone())),
            serde_json::Value::String(scalar.to_string())
        );

        let mut map = serde_json::Map::new();
        map.insert(scalar.to_string(), serde_json::Value::Number(1.into()));
        let object = from_json_value(&mut vm, &serde_json::Value::Object(map))
            .expect("object conversion should succeed");
        let Value::Object(index) = object else {
            panic!("expected converted object");
        };
        let key_units = vm.heap.with_obj(index.0, |object| match object {
            HeapObj::Object(data) => data
                .props
                .lock()
                .keys()
                .find_map(|key| key.as_str().as_deref().map(crate::value::utf16_from_str)),
            _ => None,
        });
        assert_eq!(key_units, Some(vec![0xDB80, 0xDC00]));
        let mut expected = serde_json::Map::new();
        expected.insert(scalar.to_string(), serde_json::Value::Number(1.into()));
        assert_eq!(
            to_json_value(&mut vm, &Value::Object(index)),
            serde_json::Value::Object(expected)
        );

        let lone = Value::String(Arc::from(crate::value::utf16_to_string(&[0xD800])));
        assert_eq!(
            to_json_value(&mut vm, &lone),
            serde_json::Value::String("\u{FFFD}".to_string())
        );

        let colliding_keys = vm
            .run(
                "var object = {}; object[String.fromCharCode(0xD800)] = 1; object['\\uFFFD'] = 2; object;",
            )
            .expect("colliding-key object should evaluate");
        assert_eq!(
            to_json_value(&mut vm, &colliding_keys),
            serde_json::json!({ "\u{FFFD}": 2 })
        );
    }
}

/// Convert a `serde_json::Value` into a `ruja::Value`.
pub fn from_json_value(vm: &mut Vm, v: &serde_json::Value) -> error::Result<Value> {
    match v {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Number(i as f64))
            } else if let Some(u) = n.as_u64() {
                Ok(Value::Number(u as f64))
            } else {
                Ok(Value::Number(n.as_f64().unwrap_or(f64::NAN)))
            }
        }
        serde_json::Value::String(s) => Ok(Value::from_string(s)),
        serde_json::Value::Array(arr) => {
            let mut items = Vec::new();
            for item in arr {
                items.push(from_json_value(vm, item)?);
            }
            let obj = HeapObj::Array(crate::value::ArrayData::new(
                items,
                Some(vm.array_proto.clone()),
            ));
            Ok(Value::Object(GcIdx(vm.heap.allocate(obj)?)))
        }
        serde_json::Value::Object(map) => {
            let mut converted: Vec<(String, Value)> = Vec::new();
            for (key, val) in map {
                let ruja_val = from_json_value(vm, val)?;
                converted.push((crate::value::utf16_from_scalar_str(key), ruja_val));
            }
            let idx = vm.new_object()?;
            vm.heap.with_obj(idx.0, |o| {
                if let HeapObj::Object(od) = o {
                    let mut props = od.props.lock();
                    for (key, ruja_val) in &converted {
                        props.insert(
                            crate::value::PropertyKey::from(key.as_str()),
                            crate::value::PropertyDescriptor::data(ruja_val.clone()),
                        );
                    }
                }
            });
            Ok(Value::Object(idx))
        }
    }
}
