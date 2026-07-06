//! Property access, prototype chain walking, and array index/length
//! setters split from vm/mod.rs for readability.

use super::*;
use crate::error::{self, Error};
use crate::value::HeapObj;
use crate::value::{GcIdx, PromiseStatus, Value};
use std::sync::Arc;

impl Vm {
    pub(crate) fn function_caller_value(&self, callee_idx: GcIdx) -> error::Result<Value> {
        let Some(frame_index) = self
            .frames
            .iter()
            .rposition(|frame| matches!(frame.callee, Value::Object(idx) if idx == callee_idx))
        else {
            return Ok(Value::Undefined);
        };

        let Some(caller_frame) = self.frames[..frame_index]
            .iter()
            .rev()
            .find(|frame| matches!(frame.callee, Value::Object(_)))
        else {
            return Ok(Value::Undefined);
        };

        if caller_frame.chunk.is_strict {
            return Err(Error::type_err(
                "'caller' and 'arguments' are restricted function properties",
            ));
        }
        Ok(caller_frame.callee.clone())
    }

    pub(crate) fn arguments_mapped_binding_for_index(
        &self,
        obj_idx: usize,
        index: usize,
    ) -> Option<(GcIdx, Arc<str>)> {
        self.heap.with_obj(obj_idx, |o| {
            if let HeapObj::Array(a) = o {
                a.arguments_map.lock().as_ref().and_then(|m| {
                    m.names
                        .get(index)
                        .and_then(|n| n.as_ref())
                        .map(|n| (m.env, n.clone()))
                })
            } else {
                None
            }
        })
    }

    pub(crate) fn remove_arguments_mapping_for_index(&self, obj_idx: usize, index: usize) {
        self.heap.with_obj(obj_idx, |o| {
            if let HeapObj::Array(a) = o {
                if let Some(map) = a.arguments_map.lock().as_mut() {
                    if let Some(slot) = map.names.get_mut(index) {
                        *slot = None;
                    }
                }
            }
        });
    }

    pub(crate) fn array_index_own_property_descriptor(
        &self,
        obj_idx: usize,
        index: usize,
        key: &crate::value::PropertyKey,
    ) -> Option<crate::value::PropertyDescriptor> {
        let (ordinary, dense) = self.heap.with_obj(obj_idx, |o| {
            if let HeapObj::Array(a) = o {
                let ordinary = a.props.lock().get(key).cloned();
                let dense = if index < a.items.lock().len() && a.is_dense_present(index) {
                    Some(
                        a.items
                            .lock()
                            .get(index)
                            .cloned()
                            .unwrap_or(Value::Undefined),
                    )
                } else {
                    None
                };
                (ordinary, dense)
            } else {
                (None, None)
            }
        });
        let mut desc = ordinary.or_else(|| dense.map(crate::value::PropertyDescriptor::data))?;
        if !desc.is_accessor {
            if let Some((env, name)) = self.arguments_mapped_binding_for_index(obj_idx, index) {
                if let Some(mapped_value) = crate::environment::get(&self.heap, env, &name) {
                    desc.value = mapped_value;
                }
            }
        }
        Some(desc)
    }

    fn push_value_roots(roots: &mut Vec<usize>, value: &Value) {
        match value {
            Value::Object(idx) => roots.push(idx.0),
            Value::Reference(r) => match &r.base {
                crate::value::ReferenceBase::Unresolvable => {}
                crate::value::ReferenceBase::Environment(env_idx) => roots.push(env_idx.0),
                crate::value::ReferenceBase::ObjectEnvironment(base)
                | crate::value::ReferenceBase::Value(base) => Self::push_value_roots(roots, base),
            },
            _ => {}
        }
    }

    pub(crate) fn get_property_rx(
        &mut self,
        obj: &Value,
        key: &str,
        receiver: Value,
        depth: usize,
    ) -> error::Result<Value> {
        // Bound recursion so a prototype cycle (which __proto__ assignment
        // should already reject) cannot overflow the native stack.
        if depth > 4096 {
            return Ok(Value::Undefined);
        }
        match obj {
            Value::Object(idx) => {
                let pkey = crate::value::PropertyKey::from(key);
                // Own accessor on this object?
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
                        return self.call_function(&getter, &[], Some(receiver));
                    }
                    return Ok(Value::Undefined);
                }
                // Own data property?
                let val = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Array(a) = o {
                        if key == "length"
                            && !a.is_arguments.load(std::sync::atomic::Ordering::Relaxed)
                        {
                            let len = a.items.lock().len();
                            let sparse = a.sparse_max.lock().unwrap_or(0);
                            return Some(Value::Number(len.max(sparse) as f64));
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
                                    return Some(v);
                                }
                            }
                            if let Some(d) = a.props.lock().get(&pkey) {
                                if !d.is_accessor {
                                    return Some(d.value.clone());
                                }
                            }
                            if i >= crate::value::MAX_DENSE_ARRAY_LEN {
                                let pkey = crate::value::PropertyKey::from_string(key.to_string());
                                if let Some(d) = a.props.lock().get(&pkey) {
                                    return Some(d.value.clone());
                                }
                                return Some(Value::Undefined);
                            }
                            if a.is_dense_present(i) {
                                return Some(
                                    a.items.lock().get(i).cloned().unwrap_or(Value::Undefined),
                                );
                            }
                            return None;
                        }
                    }
                    let r = o.props().lock().get(&pkey).map(|d| d.value.clone());
                    r
                });
                if let Some(v) = val {
                    return Ok(v);
                }
                // Walk up.
                let p = self.heap.with_obj(idx.0, |o| o.proto().lock().clone());
                if let Some(proto) = p {
                    if !proto.is_undefined() {
                        return self.get_property_rx(&proto, key, receiver, depth + 1);
                    }
                }
                Ok(Value::Undefined)
            }
            _ => self.get_property(obj, key),
        }
    }

    pub(crate) fn get_proto_property(&mut self, obj: &Value, key: &str) -> error::Result<Value> {
        let proto = match obj {
            Value::String(_) => self.string_proto.clone(),
            Value::Number(_) => self.number_proto.clone(),
            Value::BigInt(_) => self.bigint_proto.clone(),
            Value::Bool(_) => self.boolean_proto.clone(),
            Value::Symbol(_) => self.symbol_proto.clone(),
            _ => return Ok(Value::Undefined),
        };
        if !proto.is_undefined() {
            return self.get_property_rx(&proto, key, obj.clone(), 0);
        }
        Ok(Value::Undefined)
    }

    /// Delete an own property. Returns true if removed (or didn't exist).
    pub fn delete_property(&mut self, obj: &Value, key: &str) -> error::Result<bool> {
        if let Value::Object(idx) = obj {
            let pkey = crate::value::PropertyKey::from(key);
            let array_delete = self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Array(a) = o {
                    if key == "length" {
                        if a.is_arguments.load(std::sync::atomic::Ordering::Relaxed) {
                            let (exists, configurable) = a
                                .props
                                .lock()
                                .get(&pkey)
                                .map_or((false, true), |d| (true, d.configurable));
                            if exists && !configurable {
                                return Some(false);
                            }
                            if exists {
                                a.props.lock().shift_remove(&pkey);
                            }
                            return Some(true);
                        }
                        return Some(false);
                    }
                    if let Some(i) = crate::value::parse_array_index(key) {
                        let (exists, configurable) = {
                            let props = a.props.lock();
                            if let Some(desc) = props.get(&pkey) {
                                (true, desc.configurable)
                            } else {
                                (a.is_dense_present(i), true)
                            }
                        };
                        if exists && !configurable {
                            return Some(false);
                        }
                        if exists {
                            if let Some(map) = a.arguments_map.lock().as_mut() {
                                if let Some(slot) = map.names.get_mut(i) {
                                    *slot = None;
                                }
                            }
                            a.props.lock().shift_remove(&pkey);
                            let mut items = a.items.lock();
                            if i < items.len() {
                                items[i] = Value::Undefined;
                                if let Some(slot) = a.present.lock().get_mut(i) {
                                    *slot = false;
                                }
                            }
                        }
                        return Some(true);
                    }
                }
                None
            });
            if let Some(result) = array_delete {
                return Ok(result);
            }
            let (exists, configurable) = self.heap.with_obj(idx.0, |o| {
                o.props()
                    .lock()
                    .get(&pkey)
                    .map_or((false, true), |d| (true, d.configurable))
            });
            if exists && !configurable {
                return Ok(false);
            }
            self.heap.with_obj(idx.0, |o| {
                o.props().lock().shift_remove(&pkey);
            });
        }
        Ok(true)
    }

    pub fn set_property(&mut self, obj: &Value, key: &str, value: Value) -> error::Result<()> {
        self.set_property_impl(obj, key, value, true)
    }

    pub(crate) fn set_object_environment_property(
        &mut self,
        obj: &Value,
        key: &str,
        value: Value,
    ) -> error::Result<()> {
        self.set_property_impl(obj, key, value, false)
    }

    pub(crate) fn define_data_property(
        &mut self,
        obj: &Value,
        key: crate::value::PropertyKey,
        value: Value,
    ) -> error::Result<()> {
        if let Value::Object(idx) = obj {
            self.heap.with_obj(idx.0, |o| {
                o.props()
                    .lock()
                    .insert(key, crate::value::PropertyDescriptor::data(value));
            });
            Ok(())
        } else {
            Err(Error::type_err(
                "Cannot set property of primitive".to_string(),
            ))
        }
    }

    pub(crate) fn define_own_property_or_throw(
        &mut self,
        obj: &Value,
        key: crate::value::PropertyKey,
        desc: crate::value::PropertyDescriptor,
    ) -> error::Result<()> {
        if let Value::Object(idx) = obj {
            let current = self
                .heap
                .with_obj(idx.0, |o| o.props().lock().get(&key).cloned());
            if current.is_none() {
                let extensible = self.heap.with_obj(idx.0, |o| o.is_extensible());
                if !extensible {
                    return Err(Error::type_err(
                        "Cannot define property, object is not extensible",
                    ));
                }
            }
            if let Some(current) = current {
                if !current.configurable {
                    if desc.configurable
                        || desc.enumerable != current.enumerable
                        || desc.is_accessor != current.is_accessor
                    {
                        return Err(Error::type_err("Cannot redefine non-configurable property"));
                    }
                    if current.is_accessor {
                        if desc.get != current.get || desc.set != current.set {
                            return Err(Error::type_err(
                                "Cannot redefine non-configurable property",
                            ));
                        }
                    } else if !current.writable && (desc.writable || desc.value != current.value) {
                        return Err(Error::type_err("Cannot redefine non-configurable property"));
                    }
                }
            }
            self.heap.with_obj(idx.0, |o| {
                o.props().lock().insert(key, desc);
            });
            Ok(())
        } else {
            Err(Error::type_err(
                "Cannot define property of primitive".to_string(),
            ))
        }
    }

    fn set_property_impl(
        &mut self,
        obj: &Value,
        key: &str,
        value: Value,
        route_global_this: bool,
    ) -> error::Result<()> {
        // ES [[Set]] semantics, simplified:
        //  1. Walk the prototype chain for an accessor descriptor with a
        //     `set` function; if found, call it and return.
        //  2. Otherwise, if `obj` has its OWN data descriptor that is
        //     non-writable, the assignment fails: in strict mode throw a
        //     TypeError; otherwise silently ignore.
        //  3. Otherwise define/overwrite an own writable data property.
        // Arrays route `length` and integer-index writes through dedicated
        // logic below before falling back to ordinary object semantics.
        match obj {
            Value::Object(idx) => {
                let is_global_this = self.heap.with_obj(idx.0, |o| {
                    matches!(o, HeapObj::Object(od) if od.class_name.as_deref() == Some("global"))
                });
                // Proxy trap: if this object is a Proxy, call handler.set.
                let proxy_info = self.heap.with_obj(idx.0, |o| {
                    if let crate::value::HeapObj::Proxy(p) = o {
                        if *p.revoked.lock() {
                            return Some(Err(crate::error::Error::type_err(
                                "Cannot perform 'set' on a proxy that has been revoked".to_string(),
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
                            let trap = self.get_property(&handler, "set")?;
                            if !trap.is_undefined() {
                                let receiver = obj.clone();
                                self.call_function(
                                    &trap,
                                    &[target, key_val, value, receiver],
                                    Some(handler),
                                )?;
                                return Ok(());
                            }
                            return self.set_property(&target, key, value);
                        }
                    }
                }
                // __proto__ assignment sets the object's [[Prototype]].
                if key == "__proto__" {
                    match &value {
                        Value::Object(_) | Value::Null => {
                            // Reject prototype cycles: setting __proto__ to
                            // an object whose chain already contains this
                            // object would create a cycle, which later made
                            // property reads overflow the native stack and
                            // abort the process. ES throws TypeError here.
                            let proto = if value.is_null() {
                                None
                            } else {
                                Some(value.clone())
                            };
                            let (extensible, current_proto) = self
                                .heap
                                .with_obj(idx.0, |o| (o.is_extensible(), o.proto().lock().clone()));
                            if !extensible && current_proto != proto {
                                if self.current_strict() {
                                    return Err(Error::type_err(
                                        "Cannot mutate prototype of non-extensible object"
                                            .to_string(),
                                    ));
                                }
                                return Ok(());
                            }
                            if let Value::Object(target) = &value {
                                if self.proto_chain_contains(target.0, idx.0) {
                                    if self.current_strict() {
                                        return Err(Error::type_err(
                                            "Cyclic __proto__ value".to_string(),
                                        ));
                                    }
                                    return Ok(());
                                }
                            }
                            self.heap.with_obj(idx.0, |o| {
                                *o.proto().lock() = proto;
                            });
                            return Ok(());
                        }
                        // non-object, non-null: ignore (spec: no-op in sloppy mode)
                        _ => return Ok(()),
                    }
                }
                let typed_array_index = self.heap.with_obj(idx.0, |o| {
                    if matches!(o, HeapObj::TypedArray(_)) {
                        crate::value::parse_array_index(key)
                    } else {
                        None
                    }
                });
                if let Some(i) = typed_array_index {
                    let byte = crate::builtins::to_uint8_element(self.to_number(&value)?);
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::TypedArray(t) = o {
                            if let Some(slot) = t.buffer.lock().get_mut(i) {
                                *slot = byte;
                            }
                        }
                    });
                    return Ok(());
                }
                // --- Array fast paths ---
                let array_length_kind = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Array(a) = o {
                        if key == "length" {
                            return Some(a.is_arguments.load(std::sync::atomic::Ordering::Relaxed));
                        }
                    }
                    None
                });
                if let Some(is_arguments) = array_length_kind {
                    if is_arguments {
                        let pkey = crate::value::PropertyKey::from(key);
                        return self.ordinary_set_with_receiver(*idx, &pkey, key, value, &obj);
                    }
                    return self.set_array_length(idx.0, value);
                }
                let array_index = self.heap.with_obj(idx.0, |o| {
                    if matches!(o, HeapObj::Array(_)) {
                        crate::value::parse_array_index(key)
                    } else {
                        None
                    }
                });
                if let Some(i) = array_index {
                    let pkey = crate::value::PropertyKey::from(key);
                    let own_desc = self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Array(a) = o {
                            return a.props.lock().get(&pkey).cloned();
                        }
                        None
                    });
                    if let Some(desc) = own_desc {
                        if desc.is_accessor {
                            if let Some(setter) = desc.set {
                                self.call_function(
                                    &setter,
                                    std::slice::from_ref(&value),
                                    Some(obj.clone()),
                                )?;
                                return Ok(());
                            }
                            if self.current_strict() {
                                return Err(Error::type_err(format!(
                                    "Cannot set property '{}' which has only a getter",
                                    key
                                )));
                            }
                            return Ok(());
                        }
                        if !desc.writable {
                            if self.current_strict() {
                                return Err(Error::type_err(format!(
                                    "Cannot assign to read only property '{}' of object",
                                    key
                                )));
                            }
                            return Ok(());
                        }
                        if let Some((env, name)) = self.arguments_mapped_binding_for_index(idx.0, i)
                        {
                            crate::environment::set(&self.heap, env, &name, value.clone());
                        }
                        self.heap.with_obj(idx.0, |o| {
                            if let HeapObj::Array(a) = o {
                                if let Some(desc) = a.props.lock().get_mut(&pkey) {
                                    desc.value = value.clone();
                                }
                            }
                        });
                        return Ok(());
                    }
                    let dense_own_index = self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Array(a) = o {
                            i < a.items.lock().len()
                        } else {
                            false
                        }
                    });
                    if !dense_own_index {
                        match self.find_setter(*idx, &pkey) {
                            Some(Some(setter)) => {
                                self.call_function(
                                    &setter,
                                    std::slice::from_ref(&value),
                                    Some(obj.clone()),
                                )?;
                                return Ok(());
                            }
                            Some(None) => {
                                if self.current_strict() {
                                    return Err(Error::type_err(format!(
                                        "Cannot set property '{}' which has only a getter",
                                        key
                                    )));
                                }
                                return Ok(());
                            }
                            None => {}
                        }
                        if self.has_non_writable_data_property_in_proto(*idx, &pkey) {
                            if self.current_strict() {
                                return Err(Error::type_err(format!(
                                    "Cannot assign to read only property '{}' of object",
                                    key
                                )));
                            }
                            return Ok(());
                        }
                    } else {
                        // Dense array elements are own writable data properties,
                        // so prototype setters/non-writable data properties do
                        // not participate in this write.
                    }
                    let mapped = self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Array(a) = o {
                            a.arguments_map.lock().as_ref().and_then(|m| {
                                m.names
                                    .get(i)
                                    .and_then(|n| n.as_ref())
                                    .map(|n| (m.env, n.clone()))
                            })
                        } else {
                            None
                        }
                    });
                    if let Some((env, name)) = mapped {
                        crate::environment::set(&self.heap, env, &name, value.clone());
                    }
                    self.set_array_index(idx.0, i, value)?;
                    return Ok(());
                }

                // --- Ordinary object [[Set]] ---
                let pkey = crate::value::PropertyKey::from(key);

                // 1. Own property descriptors take precedence over inherited
                // setters/non-writable data descriptors.
                let own_desc = self
                    .heap
                    .with_obj(idx.0, |o| o.props().lock().get(&pkey).cloned());
                if let Some(desc) = own_desc {
                    if desc.is_accessor {
                        if let Some(setter) = desc.set {
                            self.call_function(
                                &setter,
                                std::slice::from_ref(&value),
                                Some(obj.clone()),
                            )?;
                            return Ok(());
                        }
                        if self.current_strict() {
                            return Err(Error::type_err(format!(
                                "Cannot set property '{}' which has only a getter",
                                key
                            )));
                        }
                        return Ok(());
                    }
                    if !desc.writable {
                        if self.current_strict() {
                            return Err(Error::type_err(format!(
                                "Cannot assign to read only property '{}' of object",
                                key
                            )));
                        }
                        return Ok(());
                    }
                    self.heap.with_obj(idx.0, |o| {
                        if let Some(existing) = o.props().lock().get_mut(&pkey) {
                            existing.value = value;
                        }
                    });
                    self.mirror_global_property_to_binding(
                        *idx,
                        key,
                        route_global_this,
                        is_global_this,
                    );
                    return Ok(());
                }

                // 2. Look for an accessor `set` up the prototype chain.
                // find_setter returns:
                //   Some(Some(setter)) — accessor with setter: call it.
                //   Some(None)          — accessor without setter: throw in strict.
                //   None               — no accessor found: proceed to data.
                match self.find_setter(*idx, &pkey) {
                    Some(Some(setter)) => {
                        self.call_function(
                            &setter,
                            std::slice::from_ref(&value),
                            Some(obj.clone()),
                        )?;
                        return Ok(());
                    }
                    Some(None) => {
                        // Accessor property with no setter.
                        if self.current_strict() {
                            return Err(Error::type_err(format!(
                                "Cannot set property '{}' which has only a getter",
                                key
                            )));
                        }
                        return Ok(());
                    }
                    None => {} // No accessor found; continue to data property checks.
                }

                // 3. A writable inherited data property permits creating an own
                // property on the receiver; a non-writable one blocks assignment.
                if self.has_non_writable_data_property_in_proto(*idx, &pkey) {
                    if self.current_strict() {
                        return Err(Error::type_err(format!(
                            "Cannot assign to read only property '{}' of object",
                            key
                        )));
                    }
                    return Ok(());
                }

                // 4. Define a new own writable data property.
                // Check extensibility: adding a new property to a
                // non-extensible object throws TypeError in strict mode.
                let is_extensible = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Object(od) = o {
                        od.extensible.load(std::sync::atomic::Ordering::Relaxed)
                    } else {
                        true // arrays, functions, etc. are extensible by default
                    }
                });
                let has_own = self
                    .heap
                    .with_obj(idx.0, |o| o.props().lock().contains_key(&pkey));
                if !is_extensible && !has_own {
                    if self.current_strict() {
                        return Err(Error::type_err(format!(
                            "Cannot add property '{}', object is not extensible",
                            key
                        )));
                    }
                    return Ok(());
                }
                self.heap.with_obj(idx.0, |o| {
                    let props = o.props();
                    let mut props = props.lock();
                    if let Some(existing) = props.get_mut(&pkey) {
                        existing.value = value;
                    } else {
                        props.insert(pkey, crate::value::PropertyDescriptor::data(value));
                    }
                });
                self.mirror_global_property_to_binding(
                    *idx,
                    key,
                    route_global_this,
                    is_global_this,
                );
                Ok(())
            }
            _ => Err(Error::type_err(
                "Cannot set property of primitive".to_string(),
            )),
        }
    }

    fn mirror_global_property_to_binding(
        &self,
        idx: GcIdx,
        key: &str,
        route_global_this: bool,
        is_global_this: bool,
    ) {
        if route_global_this
            && is_global_this
            && crate::environment::has(&self.heap, self.global, key)
        {
            let final_value = self.heap.with_obj(idx.0, |o| {
                o.props()
                    .lock()
                    .get(&crate::value::PropertyKey::from(key))
                    .map(|d| d.value.clone())
            });
            if let Some(final_value) = final_value {
                crate::environment::set(&self.heap, self.global, key, final_value);
            }
        }
    }

    pub(crate) fn set_property_with_receiver(
        &mut self,
        base: &Value,
        key: &str,
        value: Value,
        receiver: &Value,
    ) -> error::Result<()> {
        let Value::Object(base_idx) = base else {
            return Err(Error::type_err(
                "Cannot set property of primitive".to_string(),
            ));
        };
        let pkey = crate::value::PropertyKey::from(key);
        self.ordinary_set_with_receiver(*base_idx, &pkey, key, value, receiver)
    }

    fn ordinary_set_with_receiver(
        &mut self,
        mut base_idx: GcIdx,
        pkey: &crate::value::PropertyKey,
        key: &str,
        value: Value,
        receiver: &Value,
    ) -> error::Result<()> {
        for _ in 0..1024 {
            let (desc, proto) = self.heap.with_obj(base_idx.0, |o| {
                (
                    o.props().lock().get(pkey).cloned(),
                    o.proto().lock().clone(),
                )
            });
            if let Some(desc) = desc {
                if desc.is_accessor {
                    if let Some(setter) = desc.set {
                        self.call_function(
                            &setter,
                            std::slice::from_ref(&value),
                            Some(receiver.clone()),
                        )?;
                        return Ok(());
                    }
                    if self.current_strict() {
                        return Err(Error::type_err(format!(
                            "Cannot set property '{}' which has only a getter",
                            key
                        )));
                    }
                    return Ok(());
                }
                if !desc.writable {
                    if self.current_strict() {
                        return Err(Error::type_err(format!(
                            "Cannot assign to read only property '{}' of object",
                            key
                        )));
                    }
                    return Ok(());
                }
                return self.set_receiver_data_property(receiver, pkey.clone(), key, value);
            }
            match proto {
                Some(Value::Object(proto_idx)) => base_idx = proto_idx,
                _ => return self.set_receiver_data_property(receiver, pkey.clone(), key, value),
            }
        }
        Err(Error::type_err("Prototype chain too deep".to_string()))
    }

    fn set_receiver_data_property(
        &mut self,
        receiver: &Value,
        pkey: crate::value::PropertyKey,
        key: &str,
        value: Value,
    ) -> error::Result<()> {
        let Value::Object(receiver_idx) = receiver else {
            if self.current_strict() {
                return Err(Error::type_err(
                    "Cannot set property of primitive".to_string(),
                ));
            }
            return Ok(());
        };
        let existing = self
            .heap
            .with_obj(receiver_idx.0, |o| o.props().lock().get(&pkey).cloned());
        if let Some(desc) = existing {
            if desc.is_accessor || !desc.writable {
                if self.current_strict() {
                    return Err(Error::type_err(format!(
                        "Cannot assign to read only property '{}' of object",
                        key
                    )));
                }
                return Ok(());
            }
        } else {
            let is_extensible = self.heap.with_obj(receiver_idx.0, |o| {
                if let HeapObj::Object(od) = o {
                    od.extensible.load(std::sync::atomic::Ordering::Relaxed)
                } else {
                    true
                }
            });
            if !is_extensible {
                if self.current_strict() {
                    return Err(Error::type_err(format!(
                        "Cannot add property '{}', object is not extensible",
                        key
                    )));
                }
                return Ok(());
            }
        }
        let cache_key = pkey.as_str().map(|s| s.to_string());
        self.heap.with_obj(receiver_idx.0, |o| {
            let props = o.props();
            let mut props = props.lock();
            if let Some(existing) = props.get_mut(&pkey) {
                existing.value = value;
            } else {
                props.insert(pkey, crate::value::PropertyDescriptor::data(value));
            }
        });
        if let Some(key) = cache_key {
            self.ic_invalidate(receiver_idx.0, &key);
        }
        Ok(())
    }

    /// Strictness of the currently-executing frame, used by ordinary
    /// [[Set]]/[[DefineOwnProperty]] to decide whether a failed assignment
    /// throws a TypeError or is silently ignored. The top-level program has
    /// no frame; its strictness comes from the compiled top-level chunk.
    pub(crate) fn current_strict(&self) -> bool {
        self.frames
            .last()
            .map(|f| f.chunk.is_strict)
            .unwrap_or(false)
    }

    pub(crate) fn global_property_is_non_writable_data(&self, name: &str) -> bool {
        let Value::Object(idx) = &self.global_this else {
            return false;
        };
        let pkey = crate::value::PropertyKey::from(name);
        self.heap.with_obj(idx.0, |obj| {
            obj.props()
                .lock()
                .get(&pkey)
                .is_some_and(|d| !d.is_accessor && !d.writable)
        })
    }

    pub(crate) fn has_global_lexical_declaration(&self, env: GcIdx, name: &str) -> bool {
        self.global_binding_kind(env, name).is_some_and(|kind| {
            matches!(
                kind,
                crate::value::BindingKind::Let
                    | crate::value::BindingKind::Const
                    | crate::value::BindingKind::FunctionName
            )
        })
    }

    fn global_binding_kind(&self, env: GcIdx, name: &str) -> Option<crate::value::BindingKind> {
        self.heap.with_obj(env.0, |obj| {
            if let HeapObj::Environment(e) = obj {
                return e.vars.lock().get(name).map(|binding| binding.kind);
            }
            None
        })
    }

    pub(crate) fn has_restricted_global_property(&self, global_this: &Value, name: &str) -> bool {
        self.global_property_descriptor(global_this, name)
            .is_some_and(|desc| !desc.configurable)
    }

    pub(crate) fn can_declare_global_var(&self, global_this: &Value, name: &str) -> bool {
        if self.global_property_descriptor(global_this, name).is_some() {
            return true;
        }
        self.global_object_is_extensible(global_this)
    }

    pub(crate) fn can_declare_global_function(&self, global_this: &Value, name: &str) -> bool {
        let Some(desc) = self.global_property_descriptor(global_this, name) else {
            return self.global_object_is_extensible(global_this);
        };
        desc.configurable || (!desc.is_accessor && desc.writable && desc.enumerable)
    }

    fn global_property_descriptor(
        &self,
        global_this: &Value,
        name: &str,
    ) -> Option<crate::value::PropertyDescriptor> {
        let Value::Object(idx) = global_this else {
            return None;
        };
        let pkey = crate::value::PropertyKey::from(name);
        self.heap
            .with_obj(idx.0, |obj| obj.props().lock().get(&pkey).cloned())
    }

    fn global_object_is_extensible(&self, global_this: &Value) -> bool {
        let Value::Object(idx) = global_this else {
            return false;
        };
        self.heap.with_obj(idx.0, |obj| obj.is_extensible())
    }

    pub(crate) fn create_global_var_binding(&mut self, name: &str) -> error::Result<()> {
        self.create_global_var_binding_with_configurable(name, false)
    }

    pub(crate) fn create_global_var_binding_with_configurable(
        &mut self,
        name: &str,
        configurable: bool,
    ) -> error::Result<()> {
        let global_this = self.global_this.clone();
        let existing_desc = self.global_property_descriptor(&global_this, name);
        if existing_desc.is_none() {
            if !self.global_object_is_extensible(&global_this) {
                return Err(Error::type_err(format!(
                    "Cannot declare global variable '{}'",
                    name
                )));
            }
            self.set_global_var_property_with_configurable(name, Value::Undefined, configurable);
        }
        if !crate::environment::has(&self.heap, self.global, name) {
            let value = existing_desc
                .filter(|desc| !desc.is_accessor)
                .map(|desc| desc.value)
                .unwrap_or(Value::Undefined);
            crate::environment::declare(
                &self.heap,
                self.global,
                name,
                value,
                crate::value::BindingKind::Var,
            );
        }
        Ok(())
    }

    pub(crate) fn create_global_function_binding(
        &mut self,
        name: &str,
        value: Value,
    ) -> error::Result<()> {
        self.create_global_function_binding_with_configurable(name, value, false)
    }

    pub(crate) fn create_global_function_binding_with_configurable(
        &mut self,
        name: &str,
        value: Value,
        configurable: bool,
    ) -> error::Result<()> {
        let global_this = self.global_this.clone();
        if !self.can_declare_global_function(&global_this, name) {
            return Err(Error::type_err(format!(
                "Cannot declare global function '{}'",
                name
            )));
        }
        let Value::Object(idx) = global_this else {
            return Ok(());
        };
        let pkey = crate::value::PropertyKey::from(name);
        let desc_configurable = self
            .global_property_descriptor(&Value::Object(idx), name)
            .map(|desc| desc.configurable)
            .unwrap_or(true);
        self.heap.with_obj(idx.0, |obj| {
            obj.props().lock().insert(
                pkey,
                crate::value::PropertyDescriptor {
                    value: value.clone(),
                    writable: true,
                    enumerable: true,
                    configurable: if desc_configurable {
                        configurable
                    } else {
                        false
                    },
                    get: None,
                    set: None,
                    is_accessor: false,
                },
            );
        });
        crate::environment::declare(
            &self.heap,
            self.global,
            name,
            value,
            crate::value::BindingKind::Var,
        );
        Ok(())
    }

    pub(crate) fn set_global_eval_var_property(&mut self, name: &str, value: Value) {
        let Value::Object(idx) = &self.global_this else {
            return;
        };
        let pkey = crate::value::PropertyKey::from(name);
        self.heap.with_obj(idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            if let Some(desc) = props.get_mut(&pkey) {
                if !desc.is_accessor && desc.writable {
                    desc.value = value;
                }
                return;
            }
            props.insert(
                pkey,
                crate::value::PropertyDescriptor {
                    value,
                    writable: true,
                    enumerable: true,
                    configurable: true,
                    get: None,
                    set: None,
                    is_accessor: false,
                },
            );
        });
    }

    pub(crate) fn set_global_var_property(&mut self, name: &str, value: Value) {
        self.set_global_var_property_with_configurable(name, value, false);
    }

    pub(crate) fn set_global_var_property_with_configurable(
        &mut self,
        name: &str,
        value: Value,
        configurable: bool,
    ) {
        let Value::Object(idx) = &self.global_this else {
            return;
        };
        let pkey = crate::value::PropertyKey::from(name);
        self.heap.with_obj(idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            if let Some(desc) = props.get_mut(&pkey) {
                if !desc.is_accessor && desc.writable {
                    desc.value = value;
                }
                return;
            }
            props.insert(
                pkey,
                crate::value::PropertyDescriptor {
                    value,
                    writable: true,
                    enumerable: true,
                    configurable,
                    get: None,
                    set: None,
                    is_accessor: false,
                },
            );
        });
    }

    /// Does the prototype chain starting at `start` contain an object with
    /// heap index `target`? Used to reject cyclic `__proto__` assignments.
    /// Bounded by a depth cap so a pre-existing (should-be-impossible) cycle
    /// cannot hang the engine.
    pub(crate) fn proto_chain_contains(&self, start: usize, target: usize) -> bool {
        let mut cur = start;
        for _ in 0..4096 {
            if cur == target {
                return true;
            }
            let next = self.heap.with_obj(cur, |o| o.proto().lock().clone());
            match next {
                Some(Value::Object(p)) => cur = p.0,
                _ => return false,
            }
        }
        false
    }

    /// Walk the prototype chain starting at `idx` looking for an accessor
    /// descriptor for `key`. Returns:
    /// - `Some(Some(setter))` if an accessor with a setter is found.
    /// - `Some(None)` if an accessor is found but it has no setter
    ///   (`set: undefined`); the caller must throw TypeError in strict mode.
    /// - `None` if no accessor was found on the chain.
    pub(crate) fn find_setter(
        &mut self,
        mut idx: GcIdx,
        key: &crate::value::PropertyKey,
    ) -> Option<Option<Value>> {
        let mut depth = 0;
        while depth < 1024 {
            depth += 1;
            let (found, proto) = self.heap.with_obj(idx.0, |o| {
                let props = o.props();
                let result = props.lock().get(key).and_then(|d| {
                    if d.is_accessor {
                        Some(d.set.clone())
                    } else {
                        None
                    }
                });
                let proto = o.proto().lock().clone();
                (result, proto)
            });
            if let Some(setter_opt) = found {
                return Some(setter_opt);
            }
            match proto {
                Some(Value::Object(pidx)) => idx = pidx,
                _ => break,
            }
        }
        None
    }

    pub(crate) fn has_non_writable_data_property_in_proto(
        &self,
        idx: GcIdx,
        key: &crate::value::PropertyKey,
    ) -> bool {
        let mut next = self.heap.with_obj(idx.0, |o| o.proto().lock().clone());
        let mut depth = 0;
        while depth < 1024 {
            depth += 1;
            let proto_idx = match next {
                Some(Value::Object(proto_idx)) => proto_idx,
                _ => return false,
            };
            let (found_non_writable, proto) = self.heap.with_obj(proto_idx.0, |o| {
                let found = o
                    .props()
                    .lock()
                    .get(key)
                    .is_some_and(|d| !d.is_accessor && !d.writable);
                let proto = o.proto().lock().clone();
                (found, proto)
            });
            if found_non_writable {
                return true;
            }
            next = proto;
        }
        false
    }

    /// Set an integer-indexed element of an array, extending with
    /// `undefined` holes as needed.
    pub(crate) fn set_array_index(
        &mut self,
        idx: usize,
        i: usize,
        value: Value,
    ) -> error::Result<()> {
        // Spec allows arrays to be sparse. To keep untrusted code from
        // forcing a huge dense allocation (`a[0x80000000]` used to OOM-kill
        // the host with ~2B slots), indices at or beyond the dense cap are
        // stored as named string properties while `length` is advanced to
        // cover them. Reads of the holes between return `undefined`, exactly
        // as a real sparse array does.
        if i >= crate::value::MAX_DENSE_ARRAY_LEN {
            self.heap.with_obj(idx, |o| {
                if let HeapObj::Array(a) = o {
                    let pkey = crate::value::PropertyKey::from_string(i.to_string());
                    a.props
                        .lock()
                        .insert(pkey, crate::value::PropertyDescriptor::data(value));
                    let mut sm = a.sparse_max.lock();
                    if sm.is_none_or(|cur| i >= cur) {
                        // length must cover index i, i.e. i+1.
                        *sm = Some(i + 1);
                    }
                }
            });
            return Ok(());
        }
        self.heap.with_obj(idx, |o| {
            if let HeapObj::Array(a) = o {
                let is_arguments = a.is_arguments.load(std::sync::atomic::Ordering::Relaxed);
                let mut items = a.items.lock();
                let mut present = a.present.lock();
                if !is_arguments {
                    while items.len() <= i {
                        items.push(Value::Undefined);
                        present.push(false);
                    }
                }
                if i < items.len() {
                    items[i] = value;
                    if present.len() <= i {
                        present.resize(i + 1, false);
                    }
                    present[i] = true;
                } else {
                    let pkey = crate::value::PropertyKey::from_string(i.to_string());
                    a.props
                        .lock()
                        .insert(pkey, crate::value::PropertyDescriptor::data(value));
                }
            }
        });
        Ok(())
    }

    /// ES [[Set]] for `Array.prototype.length`. Validates the value per
    /// `ArraySetLength`: must be a non-negative integer in the 32-bit range,
    /// else a RangeError ("Invalid array length"); then truncate or extend.
    pub(crate) fn set_array_length(&mut self, idx: usize, value: Value) -> error::Result<()> {
        let new_len = match value {
            Value::Number(n) => {
                // Must be a non-negative integer that fits in u32, and equal
                // to its uint32 truncation (i.e. no fractional part).
                if n.is_nan() || n < 0.0 || n.is_infinite() {
                    return Err(Error::range("Invalid array length"));
                }
                if n.fract() != 0.0 {
                    return Err(Error::range("Invalid array length"));
                }
                let as_u32 = n as u32;
                if (as_u32 as f64) != n {
                    return Err(Error::range("Invalid array length"));
                }
                if n >= (1u64 << 32) as f64 {
                    return Err(Error::range("Invalid array length"));
                }
                as_u32 as usize
            }
            _ => {
                // Non-numeric assignment to length: ToUint32 semantics would
                // require conversion; for explicit non-numbers we throw as
                // V8 does for clearly-invalid values like "abc".
                return Err(Error::range("Invalid array length"));
            }
        };
        self.heap.with_obj(idx, |o| {
            if let HeapObj::Array(a) = o {
                let cap = crate::value::MAX_DENSE_ARRAY_LEN;
                let mut items = a.items.lock();
                let mut present = a.present.lock();
                // Drop any sparse properties whose index is >= new_len, and
                // recompute sparse_max so length stays consistent.
                {
                    let mut props = a.props.lock();
                    let mut to_remove = Vec::new();
                    for k in props.keys() {
                        if let crate::value::PropertyKey::Str(s) = k {
                            if let Some(i) = crate::value::parse_array_index(s) {
                                if i >= new_len {
                                    to_remove.push(k.clone());
                                }
                            }
                        }
                    }
                    for k in to_remove {
                        props.shift_remove(&k);
                    }
                }
                if new_len <= cap {
                    // Fits in the dense backing store.
                    if new_len < items.len() {
                        items.truncate(new_len);
                        present.truncate(new_len);
                    } else {
                        while items.len() < new_len {
                            items.push(Value::Undefined);
                            present.push(false);
                        }
                    }
                    drop(present);
                    drop(items);
                    *a.sparse_max.lock() = None;
                } else {
                    // Beyond the dense cap: keep the dense store capped,
                    // advance length via sparse_max, and do NOT allocate
                    // millions of holes.
                    if items.len() > cap {
                        items.truncate(cap);
                        present.truncate(cap);
                    }
                    drop(present);
                    drop(items);
                    *a.sparse_max.lock() = Some(new_len);
                }
            }
        });
        Ok(())
    }

    // ---- GC roots ----
    pub fn collect_roots(&self) -> Vec<usize> {
        let mut roots = vec![self.global.0];
        Self::push_value_roots(&mut roots, &self.global_this);
        if let Some(v) = &self.pending_new_target {
            Self::push_value_roots(&mut roots, v);
        }
        for v in &self.stack {
            Self::push_value_roots(&mut roots, v);
        }
        for f in &self.frames {
            Self::push_value_roots(&mut roots, &f.callee);
            roots.push(f.env.0);
            Self::push_value_roots(&mut roots, &f.this_val);
            for l in &f.locals {
                Self::push_value_roots(&mut roots, l);
            }
            // Per-frame generator run-state can hold live heap values
            // (resume value sent via next(obj), and the yielded value before
            // it is moved into the LazyGenerator). Root them so a GC during
            // resume_generator does not collect them.
            Self::push_value_roots(&mut roots, &f.gen_resume_value.lock());
            // gen_yield is Mutex<Option<Value>>; peek without consuming via take+set.
            let y = f.gen_yield.lock().take();
            if let Some(v) = &y {
                Self::push_value_roots(&mut roots, v);
            }
            *f.gen_yield.lock() = y;
        }
        for proto in [
            &self.object_proto,
            &self.array_proto,
            &self.function_proto,
            &self.string_proto,
            &self.number_proto,
            &self.bigint_proto,
            &self.boolean_proto,
            &self.error_proto,
            &self.symbol_proto,
            &self.promise_proto,
            &self.iterator_proto,
            &self.map_proto,
            &self.set_proto,
            &self.generator_proto,
            &self.generator_function_proto,
        ] {
            Self::push_value_roots(&mut roots, proto);
        }
        // Pending microtasks hold live heap values (Promise handlers, resolve/
        // reject reasons). Root them so a GC between scheduling and running a
        // microtask does not collect them.
        for mt in &self.microtask_queue {
            match mt {
                Microtask::Then {
                    on_fulfilled,
                    on_rejected,
                    derived,
                    ..
                } => {
                    Self::push_value_roots(&mut roots, on_fulfilled);
                    Self::push_value_roots(&mut roots, on_rejected);
                    if let Some(idx) = derived {
                        roots.push(idx.0);
                    }
                }
                Microtask::Resolve { value, .. } => {
                    Self::push_value_roots(&mut roots, value);
                }
                Microtask::Reject { reason, .. } => {
                    Self::push_value_roots(&mut roots, reason);
                }
            }
        }
        // Global constants are reachable for the program lifetime.
        for v in &self.global_constants {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_eval_functions.values() {
            Self::push_value_roots(&mut roots, v);
        }
        // Pinned temporary roots (e.g. Promise handlers held across call_function).
        roots.extend_from_slice(&self.gc_pins);
        roots
    }

    pub fn gc(&self) {
        let roots = self.collect_roots();
        self.heap.collect(&roots);
    }

    /// Pin a heap object as a temporary GC root. Returns a guard token to pass
    /// to `unpin` when the value is no longer held in a Rust local.
    pub fn pin(&mut self, v: &Value) -> usize {
        if let Value::Object(idx) = v {
            self.gc_pins.push(idx.0);
            1
        } else {
            0
        }
    }

    /// Release the temporary root pinned at `token`.
    pub fn unpin(&mut self, token: usize) {
        if token != 0 {
            // Swap-remove is unsafe here (would move another live pin's index),
            // so just clear by setting to an invalid/no-op slot. We truncate
            // trailing sentinels lazily; pins are short-lived (single call).
            // Simplest correct approach: only the most-recent pin is popped.
            if token + 1 == self.gc_pins.len() {
                self.gc_pins.pop();
            } else {
                // Overwritten with a stale slot; collect_roots tolerates dupes.
                self.gc_pins[token] = usize::MAX;
            }
        }
    }

    /// Pin multiple values at once; returns the count to unpin later.
    pub fn pin_many(&mut self, vals: &[Value]) -> usize {
        let mut n = 0;
        for v in vals {
            if let Value::Object(idx) = v {
                self.gc_pins.push(idx.0);
                n += 1;
            }
        }
        n
    }

    /// Release `n` most-recently pinned temporary roots.
    pub fn unpin_many(&mut self, n: usize) {
        for _ in 0..n {
            self.gc_pins.pop();
        }
    }

    /// Allocate a plain object and return its handle.
    /// Resolve a promise: set state to Fulfilled and schedule its handlers.
    pub fn promise_resolve(&mut self, promise_idx: usize, value: Value) {
        let handlers: Vec<crate::value::PromiseHandler> = self.heap.with_obj(promise_idx, |o| {
            if let HeapObj::Promise(p) = o {
                if *p.state.lock() != PromiseStatus::Pending {
                    return Vec::new();
                }
                *p.state.lock() = PromiseStatus::Fulfilled;
                *p.result.lock() = value.clone();
                p.handlers.lock().drain(..).collect()
            } else {
                Vec::new()
            }
        });
        for h in handlers {
            self.microtask_queue.push_back(Microtask::Then {
                promise: GcIdx(promise_idx),
                on_fulfilled: h.on_fulfilled,
                on_rejected: h.on_rejected,
                derived: h.derived,
            });
        }
    }

    /// Reject a promise: set state to Rejected and schedule its handlers.
    pub fn promise_reject(&mut self, promise_idx: usize, reason: Value) {
        let handlers: Vec<crate::value::PromiseHandler> = self.heap.with_obj(promise_idx, |o| {
            if let HeapObj::Promise(p) = o {
                if *p.state.lock() != PromiseStatus::Pending {
                    return Vec::new();
                }
                *p.state.lock() = PromiseStatus::Rejected;
                *p.result.lock() = reason.clone();
                p.handlers.lock().drain(..).collect()
            } else {
                Vec::new()
            }
        });
        for h in handlers {
            self.microtask_queue.push_back(Microtask::Then {
                promise: GcIdx(promise_idx),
                on_fulfilled: h.on_fulfilled,
                on_rejected: h.on_rejected,
                derived: h.derived,
            });
        }
    }

    /// Drain the microtask queue, running scheduled then/catch callbacks.
    pub fn run_microtasks(&mut self) -> error::Result<()> {
        // Drain in enqueue order (FIFO): Promise microtasks must fire in the
        // order they were scheduled, so pop from the front. (Vec::remove(0) is
        // O(n), but microtask queues are typically small per drain cycle.)
        while let Some(task) = self.microtask_queue.pop_front() {
            match task {
                Microtask::Then {
                    promise,
                    on_fulfilled,
                    on_rejected,
                    derived,
                } => self.run_then(promise, on_fulfilled, on_rejected, derived)?,
                Microtask::Resolve { promise, value } => {
                    self.promise_resolve(promise.0, value);
                }
                Microtask::Reject { promise, reason } => {
                    self.promise_reject(promise.0, reason);
                }
            }
        }
        Ok(())
    }

    /// Execute a single microtask from the queue, if any. Returns true if
    /// a task was executed, false if the queue is empty. This allows hosts
    /// (e.g. WASM, server runtimes) to cooperatively interleave JS microtask
    /// execution with other work, rather than draining all microtasks at once.
    pub fn tick(&mut self) -> error::Result<bool> {
        if let Some(task) = self.microtask_queue.pop_front() {
            match task {
                Microtask::Then {
                    promise,
                    on_fulfilled,
                    on_rejected,
                    derived,
                } => self.run_then(promise, on_fulfilled, on_rejected, derived)?,
                Microtask::Resolve { promise, value } => {
                    self.promise_resolve(promise.0, value);
                }
                Microtask::Reject { promise, reason } => {
                    self.promise_reject(promise.0, reason);
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Returns true if there are pending microtasks in the queue.
    pub fn has_pending_microtasks(&self) -> bool {
        !self.microtask_queue.is_empty()
    }

    /// Inline cache lookup: returns cached value if (obj_idx, key) was seen.
    pub(crate) fn ic_get(&self, obj_idx: usize, key: &str) -> Option<Value> {
        self.ic.get(&(obj_idx, key.to_string())).cloned()
    }

    /// Store a value in the inline cache.
    pub(crate) fn ic_put(&mut self, obj_idx: usize, key: String, val: Value) {
        // Limit cache size to avoid unbounded growth.
        if self.ic.len() > 4096 {
            self.ic.clear();
        }
        self.ic.insert((obj_idx, key), val);
    }

    /// Invalidate a cache entry when a property is written.
    pub(crate) fn ic_invalidate(&mut self, obj_idx: usize, key: &str) {
        self.ic.remove(&(obj_idx, key.to_string()));
    }
}
