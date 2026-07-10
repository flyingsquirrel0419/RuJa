//! Conversion between `ruja::Value` and `serde_json::Value`.
//!
//! This module is gated behind the `serde` feature flag.

use crate::error;
use crate::value::{GcIdx, HeapObj, Value};
use crate::vm::Vm;
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
        Value::String(s) => serde_json::Value::String(s.to_string()),
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
                        .filter_map(|(k, d)| k.as_str().map(|s| (s.to_string(), d.value.clone())))
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
        }));

        assert_eq!(to_json_value(&mut vm, &private), serde_json::Value::Null);
        assert_eq!(to_json_value(&mut vm, &reference), serde_json::Value::Null);
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
        serde_json::Value::String(s) => Ok(Value::String(Arc::from(s.as_str()))),
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
                converted.push((key.clone(), ruja_val));
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
