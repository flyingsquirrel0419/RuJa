//! Property access, prototype chain walking, and array index/length
//! setters split from vm/mod.rs for readability.

use super::*;

use crate::error::{self, Error};
use crate::value::HeapObj;
use crate::value::{GcIdx, PromiseStatus, TypedArrayKind, Value};
use std::sync::Arc;

pub(crate) const MAX_PROXY_CYCLE_REPLAYS: usize = 512;

struct TypedArrayNumericSlots {
    kind: TypedArrayKind,
    viewed_array_buffer: Option<Value>,
    byte_offset: usize,
    byte_length: usize,
    length_tracking: bool,
    numeric_index: f64,
}

pub(crate) struct PropertyTraversal {
    followed_edges: std::collections::HashSet<(usize, usize)>,
    rooted_nodes: std::collections::HashSet<usize>,
    pin_count: usize,
    ordinary_edge_credit: usize,
    proxy_seen: bool,
    cycle_replays: usize,
}

impl PropertyTraversal {
    fn try_new(initial_roots: &[Value], ordinary_edge_credit: usize) -> error::Result<Self> {
        let object_count = initial_roots
            .iter()
            .filter(|value| matches!(value, Value::Object(_)))
            .count();
        let mut rooted_nodes = std::collections::HashSet::new();
        rooted_nodes
            .try_reserve(object_count)
            .map_err(|_| Error::range("property traversal state is too large"))?;
        for value in initial_roots {
            if let Value::Object(idx) = value {
                rooted_nodes.insert(idx.0);
            }
        }
        Ok(Self {
            followed_edges: std::collections::HashSet::new(),
            rooted_nodes,
            pin_count: 0,
            ordinary_edge_credit,
            proxy_seen: false,
            cycle_replays: 0,
        })
    }

    pub(crate) fn pin_count(&self) -> usize {
        self.pin_count
    }

    fn grant_ordinary_edge_credit(&mut self) {
        self.ordinary_edge_credit = 1;
    }

    pub(crate) fn note_proxy(&mut self) {
        self.proxy_seen = true;
    }
}

enum OrdinarySetOutcome {
    Complete(bool),
    Forward(Value),
}

enum GetOwnPropertyOutcome {
    Value(Value),
    Accessor(Option<Value>),
    Absent,
}

pub(crate) struct TypedArrayDefineDescriptor<'a> {
    pub(crate) value: Option<&'a Value>,
    pub(crate) has_configurable: bool,
    pub(crate) configurable: bool,
    pub(crate) has_enumerable: bool,
    pub(crate) enumerable: bool,
    pub(crate) is_accessor: bool,
    pub(crate) has_writable: bool,
    pub(crate) writable: bool,
}

#[derive(Clone)]
pub(crate) struct ProxyDefinePropertyDescriptor {
    pub(crate) descriptor: crate::value::PropertyDescriptor,
    pub(crate) has_value: bool,
    pub(crate) has_writable: bool,
    pub(crate) has_enumerable: bool,
    pub(crate) has_configurable: bool,
    pub(crate) has_get: bool,
    pub(crate) has_set: bool,
}

impl ProxyDefinePropertyDescriptor {
    fn complete(descriptor: crate::value::PropertyDescriptor) -> Self {
        let is_accessor = descriptor.is_accessor;
        Self {
            descriptor,
            has_value: !is_accessor,
            has_writable: !is_accessor,
            has_enumerable: true,
            has_configurable: true,
            has_get: is_accessor,
            has_set: is_accessor,
        }
    }

    fn is_accessor(&self) -> bool {
        self.has_get || self.has_set
    }

    fn is_data(&self) -> bool {
        self.has_value || self.has_writable
    }

    fn value_only(value: Value) -> Self {
        Self {
            descriptor: crate::value::PropertyDescriptor {
                value,
                writable: false,
                enumerable: false,
                configurable: false,
                get: None,
                set: None,
                is_accessor: false,
            },
            has_value: true,
            has_writable: false,
            has_enumerable: false,
            has_configurable: false,
            has_get: false,
            has_set: false,
        }
    }
}

pub(crate) enum ProxyDefinePropertyOutcome {
    Ordinary(Value),
    Complete(bool),
}

fn descriptor_same_value(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            (left.is_nan() && right.is_nan()) || left.to_bits() == right.to_bits()
        }
        _ => left == right,
    }
}

fn compatible_complete_descriptor(
    current: Option<&crate::value::PropertyDescriptor>,
    desc: &crate::value::PropertyDescriptor,
    extensible: bool,
) -> bool {
    let Some(current) = current else {
        return extensible;
    };
    if current.configurable {
        return true;
    }
    if desc.configurable
        || desc.enumerable != current.enumerable
        || desc.is_accessor != current.is_accessor
    {
        return false;
    }
    if current.is_accessor {
        desc.get == current.get && desc.set == current.set
    } else {
        current.writable || (!desc.writable && descriptor_same_value(&desc.value, &current.value))
    }
}

fn compatible_proxy_define_descriptor(
    current: Option<&crate::value::PropertyDescriptor>,
    desc: &ProxyDefinePropertyDescriptor,
    extensible: bool,
) -> bool {
    let Some(current) = current else {
        return extensible;
    };
    if current.configurable {
        return true;
    }

    let descriptor = &desc.descriptor;
    if (desc.has_configurable && descriptor.configurable)
        || (desc.has_enumerable && descriptor.enumerable != current.enumerable)
        || ((desc.is_accessor() || desc.is_data()) && desc.is_accessor() != current.is_accessor)
    {
        return false;
    }
    if current.is_accessor {
        return (!desc.has_get || descriptor.get == current.get)
            && (!desc.has_set || descriptor.set == current.set);
    }
    if desc.is_data() && !current.writable {
        return (!desc.has_writable || !descriptor.writable)
            && (!desc.has_value || descriptor_same_value(&descriptor.value, &current.value));
    }
    true
}

impl Vm {
    pub(crate) fn is_compatible_property_descriptor(
        &self,
        current: Option<&crate::value::PropertyDescriptor>,
        desc: &crate::value::PropertyDescriptor,
        extensible: bool,
    ) -> bool {
        compatible_complete_descriptor(current, desc, extensible)
    }

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

    fn set_arguments_mapped_binding_for_key(
        &self,
        receiver_idx: usize,
        pkey: &crate::value::PropertyKey,
        value: &Value,
    ) {
        let Some(index) = pkey.as_str().and_then(crate::value::parse_array_index) else {
            return;
        };
        if let Some((env, name)) = self.arguments_mapped_binding_for_index(receiver_idx, index) {
            crate::environment::set(&self.heap, env, &name, value.clone());
        }
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

    pub(crate) fn sync_array_length_descriptor_after_index(&mut self, obj_idx: usize) {
        let updated = self.heap.with_obj(obj_idx, |object| {
            let HeapObj::Array(array) = object else {
                return false;
            };
            if array
                .is_arguments
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                return false;
            }
            let length = array
                .items
                .lock()
                .len()
                .max(array.sparse_max.lock().unwrap_or(0));
            if let Some(descriptor) = array
                .props
                .lock()
                .get_mut(&crate::value::PropertyKey::from("length"))
            {
                descriptor.value = Value::Number(length as f64);
            }
            true
        });
        if updated {
            self.ic_invalidate(obj_idx, "length");
        }
    }

    pub(crate) fn array_index_blocked_by_non_writable_length(
        &self,
        obj_idx: usize,
        index: usize,
    ) -> bool {
        self.heap.with_obj(obj_idx, |object| {
            let HeapObj::Array(array) = object else {
                return false;
            };
            if array
                .is_arguments
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                return false;
            }
            let old_length = array
                .items
                .lock()
                .len()
                .max(array.sparse_max.lock().unwrap_or(0));
            let length_writable = array
                .props
                .lock()
                .get(&crate::value::PropertyKey::from("length"))
                .is_none_or(|descriptor| descriptor.writable);
            index >= old_length && !length_writable
        })
    }

    /// Count the exact temporary heap roots `pin` will publish for a value.
    pub(crate) fn value_root_count(value: &Value) -> usize {
        match value {
            Value::Object(_) => 1,
            Value::Reference(reference) => {
                let base = match &reference.base {
                    crate::value::ReferenceBase::Unresolvable => 0,
                    crate::value::ReferenceBase::Environment(_) => 1,
                    crate::value::ReferenceBase::ObjectEnvironment(base)
                    | crate::value::ReferenceBase::Value(base) => Self::value_root_count(base),
                };
                let this_value = reference
                    .this_value
                    .as_ref()
                    .map_or(0, |value| Self::value_root_count(value));
                let name = match &reference.name {
                    crate::value::ReferencedName::UncoercedProperty(name) => {
                        Self::value_root_count(name)
                    }
                    _ => 0,
                };
                base.saturating_add(this_value).saturating_add(name)
            }
            _ => 0,
        }
    }

    /// Make a following known-size pin batch allocator-failure-safe.
    pub(crate) fn try_reserve_gc_pins(&mut self, additional: usize) -> error::Result<()> {
        #[cfg(test)]
        if std::mem::take(&mut self.fail_next_gc_pin_reservation) {
            return Err(Error::range("temporary root set is too large"));
        }
        #[cfg(test)]
        if let Some(remaining) = &mut self.gc_pin_reservation_failure_countdown {
            if *remaining == 0 {
                self.gc_pin_reservation_failure_countdown = None;
                return Err(Error::range("temporary root set is too large"));
            }
            *remaining -= 1;
        }
        self.gc_pins
            .try_reserve(additional)
            .map_err(|_| Error::range("temporary root set is too large"))
    }

    pub(crate) fn try_reserve_value_roots(&mut self, values: &[Value]) -> error::Result<()> {
        let required = values.iter().try_fold(0usize, |total, value| {
            total
                .checked_add(Self::value_root_count(value))
                .ok_or_else(|| Error::range("temporary root set is too large"))
        })?;
        if required != 0 {
            self.try_reserve_gc_pins(required)?;
        }
        Ok(())
    }

    fn try_reserve_get_prototype_scratch(
        &mut self,
        expected: &mut Vec<Option<Value>>,
    ) -> error::Result<()> {
        #[cfg(test)]
        if std::mem::take(&mut self.fail_next_get_prototype_scratch_reservation) {
            return Err(Error::range("getPrototypeOf validation chain is too large"));
        }
        expected
            .try_reserve(1)
            .map_err(|_| Error::range("getPrototypeOf validation chain is too large"))
    }

    fn try_reserve_get_prototype_root(
        &mut self,
        value: &Value,
        #[cfg(test)] site: GetPrototypeReservationSite,
    ) -> error::Result<()> {
        #[cfg(test)]
        if self.fail_get_prototype_reservation_site == Some(site) {
            self.fail_get_prototype_reservation_site = None;
            return Err(Error::range("temporary root set is too large"));
        }
        self.try_reserve_value_roots(std::slice::from_ref(value))
    }

    fn push_value_roots(roots: &mut Vec<usize>, value: &Value) {
        match value {
            Value::Object(idx) => roots.push(idx.0),
            Value::Reference(r) => {
                match &r.base {
                    crate::value::ReferenceBase::Unresolvable => {}
                    crate::value::ReferenceBase::Environment(env_idx) => roots.push(env_idx.0),
                    crate::value::ReferenceBase::ObjectEnvironment(base)
                    | crate::value::ReferenceBase::Value(base) => {
                        Self::push_value_roots(roots, base)
                    }
                }
                if let Some(this_value) = &r.this_value {
                    Self::push_value_roots(roots, this_value);
                }
                if let crate::value::ReferencedName::UncoercedProperty(name) = &r.name {
                    Self::push_value_roots(roots, name);
                }
            }
            _ => {}
        }
    }

    #[cfg(test)]
    pub(crate) fn fail_property_traversal_reservation(
        &mut self,
        site: PropertyTraversalReservationSite,
    ) -> error::Result<()> {
        if self.fail_property_traversal_reservation_site == Some(site) {
            self.fail_property_traversal_reservation_site = None;
            return Err(Error::range("property traversal state is too large"));
        }
        Ok(())
    }

    pub(crate) fn try_new_property_traversal(
        &mut self,
        initial_roots: &[Value],
        ordinary_edge_credit: usize,
    ) -> error::Result<PropertyTraversal> {
        let object_count = initial_roots
            .iter()
            .filter(|value| matches!(value, Value::Object(_)))
            .count();
        #[cfg(test)]
        if object_count != 0 {
            self.fail_property_traversal_reservation(
                PropertyTraversalReservationSite::InitialNodes,
            )?;
        }
        let traversal = PropertyTraversal::try_new(initial_roots, ordinary_edge_credit)?;
        self.try_reserve_value_roots(initial_roots)?;
        Ok(traversal)
    }

    pub(crate) fn advance_property_edge(
        &mut self,
        traversal: &mut PropertyTraversal,
        from: GcIdx,
        next: &Value,
        charge_ordinary_edge: bool,
    ) -> error::Result<()> {
        let Value::Object(next_idx) = next else {
            return Ok(());
        };
        if charge_ordinary_edge {
            if traversal.ordinary_edge_credit == 0 {
                self.consume_fuel()?;
            } else {
                traversal.ordinary_edge_credit -= 1;
            }
        }
        let edge = (from.0, next_idx.0);
        if traversal.followed_edges.contains(&edge) {
            if !traversal.proxy_seen {
                return Err(Error::type_err("Prototype chain cycle"));
            }
            // Proxy trap lookup is observable on every recursive pass. Replay
            // cyclic edges so a later lookup can mutate the target, while a
            // finite host guard replaces native-stack overflow for inert cycles.
            traversal.cycle_replays += 1;
            if traversal.cycle_replays > MAX_PROXY_CYCLE_REPLAYS {
                return Err(Error::range(
                    "Maximum cyclic property traversal depth exceeded",
                ));
            }
            return Ok(());
        }

        let needs_root = !traversal.rooted_nodes.contains(&next_idx.0);
        #[cfg(test)]
        self.fail_property_traversal_reservation(PropertyTraversalReservationSite::FollowedEdge)?;
        traversal
            .followed_edges
            .try_reserve(1)
            .map_err(|_| Error::range("property traversal state is too large"))?;
        if needs_root {
            #[cfg(test)]
            self.fail_property_traversal_reservation(PropertyTraversalReservationSite::RootedNode)?;
            traversal
                .rooted_nodes
                .try_reserve(1)
                .map_err(|_| Error::range("property traversal state is too large"))?;
            #[cfg(test)]
            self.fail_property_traversal_reservation(
                PropertyTraversalReservationSite::ReachedRoot,
            )?;
            self.try_reserve_value_roots(std::slice::from_ref(next))?;
        }

        traversal.followed_edges.insert(edge);
        if needs_root {
            traversal.rooted_nodes.insert(next_idx.0);
            traversal.pin_count += self.pin(next);
        }
        Ok(())
    }

    pub(crate) fn advance_for_in_property_edge(
        &mut self,
        iterator: GcIdx,
        from: GcIdx,
        next: &Value,
        charge_ordinary_edge: bool,
    ) -> error::Result<()> {
        let Value::Object(next_idx) = next else {
            return Ok(());
        };
        if charge_ordinary_edge {
            self.consume_fuel()?;
        }

        let edge = (from.0, next_idx.0);
        let edge_state = self.heap.with_obj(iterator.0, |object| {
            let HeapObj::Iterator(iterator) = object else {
                return None;
            };
            let state = iterator.for_in.lock();
            let state = state.as_ref()?;
            Some((
                state.followed_edges.contains(&edge),
                state.proxy_seen,
                !state.rooted_nodes.contains(&next_idx.0),
            ))
        });
        let (duplicate, proxy_seen, needs_root) =
            edge_state.ok_or_else(|| Error::internal("for-in traversal state missing"))?;
        if duplicate {
            if !proxy_seen {
                return Err(Error::type_err("Prototype chain cycle"));
            }
            let replay_count = self.heap.with_obj(iterator.0, |object| {
                let HeapObj::Iterator(iterator) = object else {
                    return None;
                };
                let mut state = iterator.for_in.lock();
                let state = state.as_mut()?;
                state.cycle_replays += 1;
                Some(state.cycle_replays)
            });
            let replay_count =
                replay_count.ok_or_else(|| Error::internal("for-in traversal state missing"))?;
            if replay_count > MAX_PROXY_CYCLE_REPLAYS {
                return Err(Error::range(
                    "Maximum cyclic property traversal depth exceeded",
                ));
            }
            return Ok(());
        }

        #[cfg(test)]
        self.fail_property_traversal_reservation(PropertyTraversalReservationSite::FollowedEdge)?;
        self.heap.with_obj(iterator.0, |object| {
            let HeapObj::Iterator(iterator) = object else {
                return Err(Error::internal("for-in traversal iterator missing"));
            };
            let mut state = iterator.for_in.lock();
            let state = state
                .as_mut()
                .ok_or_else(|| Error::internal("for-in traversal state missing"))?;
            state
                .followed_edges
                .try_reserve(1)
                .map_err(|_| Error::range("property traversal state is too large"))
        })?;
        if needs_root {
            #[cfg(test)]
            self.fail_property_traversal_reservation(PropertyTraversalReservationSite::RootedNode)?;
            self.heap.with_obj(iterator.0, |object| {
                let HeapObj::Iterator(iterator) = object else {
                    return Err(Error::internal("for-in traversal iterator missing"));
                };
                let mut state = iterator.for_in.lock();
                let state = state
                    .as_mut()
                    .ok_or_else(|| Error::internal("for-in traversal state missing"))?;
                state
                    .rooted_nodes
                    .try_reserve(1)
                    .map_err(|_| Error::range("property traversal state is too large"))
            })?;
            #[cfg(test)]
            self.fail_property_traversal_reservation(
                PropertyTraversalReservationSite::ReachedRoot,
            )?;
            self.heap.with_obj(iterator.0, |object| {
                let HeapObj::Iterator(iterator) = object else {
                    return Err(Error::internal("for-in traversal iterator missing"));
                };
                let mut state = iterator.for_in.lock();
                let state = state
                    .as_mut()
                    .ok_or_else(|| Error::internal("for-in traversal state missing"))?;
                state
                    .traversal_roots
                    .try_reserve(1)
                    .map_err(|_| Error::range("property traversal state is too large"))
            })?;
        }

        self.heap.with_obj(iterator.0, |object| {
            let HeapObj::Iterator(iterator) = object else {
                return Err(Error::internal("for-in traversal iterator missing"));
            };
            let mut state = iterator.for_in.lock();
            let state = state
                .as_mut()
                .ok_or_else(|| Error::internal("for-in traversal state missing"))?;
            state.followed_edges.insert(edge);
            if needs_root {
                state.rooted_nodes.insert(next_idx.0);
                state.traversal_roots.push(next.clone());
            }
            Ok(())
        })?;
        Ok(())
    }

    fn get_own_property_for_get(
        &mut self,
        obj: &Value,
        key: &crate::value::PropertyKey,
        include_direct_exotics: bool,
    ) -> error::Result<GetOwnPropertyOutcome> {
        let Value::Object(idx) = obj else {
            return Ok(GetOwnPropertyOutcome::Absent);
        };
        let key_str = key.as_str();

        // These instance-field compatibility paths stand in for prototype
        // accessors, so an ordinary own property must still be able to shadow
        // them just as it would shadow the eventual accessor.
        if include_direct_exotics
            && matches!(
                key_str,
                Some("length" | "byteLength" | "byteOffset" | "buffer")
            )
        {
            let uses_direct_exotic_field = self.heap.with_obj(idx.0, |object| {
                matches!(
                    object,
                    HeapObj::TypedArray(_) | HeapObj::ArrayBuffer(_) | HeapObj::DataView(_)
                )
            });
            if uses_direct_exotic_field {
                let descriptor = self
                    .heap
                    .with_obj(idx.0, |object| object.props().lock().get(key).cloned());
                if let Some(descriptor) = descriptor {
                    return if descriptor.is_accessor {
                        Ok(GetOwnPropertyOutcome::Accessor(descriptor.get))
                    } else {
                        Ok(GetOwnPropertyOutcome::Value(descriptor.value))
                    };
                }
            }
        }

        // Some built-ins still expose instance fields directly rather than
        // through prototype accessors. Restrict those compatibility paths to
        // direct property reads; Proxy forwarding and Reflect.get must retain
        // their explicit receiver semantics.
        if include_direct_exotics {
            let typed_array_info = self.heap.with_obj(idx.0, |object| {
                if let HeapObj::TypedArray(typed_array) = object {
                    Some((
                        typed_array.kind,
                        typed_array.viewed_array_buffer.clone(),
                        typed_array.byte_offset,
                        typed_array.byte_length,
                        typed_array.length_tracking,
                        typed_array.buffer.lock().len(),
                    ))
                } else {
                    None
                }
            });
            if let Some((
                kind,
                viewed_array_buffer,
                byte_offset,
                byte_length,
                length_tracking,
                owned_len,
            )) = typed_array_info
            {
                let effective = crate::builtins::effective_view_byte_length(
                    self,
                    viewed_array_buffer.as_ref(),
                    byte_offset,
                    if viewed_array_buffer.is_some() {
                        byte_length
                    } else {
                        owned_len
                    },
                    length_tracking,
                    kind.element_size(),
                );
                let buffer_len = effective.unwrap_or(0);
                match key_str {
                    Some("length") => {
                        return Ok(GetOwnPropertyOutcome::Value(Value::Number(
                            crate::builtins::typed_array_element_count(kind, buffer_len) as f64,
                        )))
                    }
                    Some("byteLength") => {
                        return Ok(GetOwnPropertyOutcome::Value(Value::Number(
                            buffer_len as f64,
                        )))
                    }
                    Some("byteOffset") => {
                        return Ok(GetOwnPropertyOutcome::Value(Value::Number(
                            if effective.is_some() { byte_offset } else { 0 } as f64,
                        )))
                    }
                    Some("buffer") => {
                        return Ok(GetOwnPropertyOutcome::Value(
                            viewed_array_buffer.unwrap_or(Value::Undefined),
                        ))
                    }
                    _ => {}
                }
            }

            let array_buffer_len = self.heap.with_obj(idx.0, |object| {
                if let HeapObj::ArrayBuffer(buffer) = object {
                    Some(
                        if buffer.detached.load(std::sync::atomic::Ordering::Relaxed) {
                            0
                        } else {
                            buffer.bytes.lock().len()
                        },
                    )
                } else {
                    None
                }
            });
            if key_str == Some("byteLength") {
                if let Some(length) = array_buffer_len {
                    return Ok(GetOwnPropertyOutcome::Value(Value::Number(length as f64)));
                }
            }

            let data_view_info = self.heap.with_obj(idx.0, |object| {
                if let HeapObj::DataView(view) = object {
                    Some((
                        view.buffer.clone(),
                        view.byte_offset,
                        view.byte_length,
                        view.length_tracking,
                    ))
                } else {
                    None
                }
            });
            if let Some((buffer, byte_offset, byte_length, length_tracking)) = data_view_info {
                match key_str {
                    Some("buffer") => return Ok(GetOwnPropertyOutcome::Value(buffer)),
                    Some("byteOffset") | Some("byteLength") => {
                        let Some(effective) = crate::builtins::effective_view_byte_length(
                            self,
                            Some(&buffer),
                            byte_offset,
                            byte_length,
                            length_tracking,
                            1,
                        ) else {
                            return Err(Error::type_err(
                                "DataView getter on detached or out-of-bounds buffer",
                            ));
                        };
                        let value = if key_str == Some("byteOffset") {
                            byte_offset
                        } else {
                            effective
                        };
                        return Ok(GetOwnPropertyOutcome::Value(Value::Number(value as f64)));
                    }
                    _ => {}
                }
            }
        }

        let namespace_binding = self.heap.with_obj(idx.0, |object| {
            if let HeapObj::ModuleNamespace(namespace) = object {
                return key_str.and_then(|name| namespace.exports.lock().get(name).cloned());
            }
            None
        });
        if let Some((env, name)) = namespace_binding {
            return match crate::environment::get_checked(&self.heap, env, &name) {
                Ok(Some(value)) => Ok(GetOwnPropertyOutcome::Value(value)),
                Ok(None) | Err(false) => Ok(GetOwnPropertyOutcome::Value(Value::Undefined)),
                Err(true) => Err(Error::reference(format!(
                    "Cannot access '{}' before initialization",
                    name
                ))),
            };
        }

        if let Some(descriptor) = self.typed_array_integer_index_own_property_descriptor(obj, key) {
            return Ok(GetOwnPropertyOutcome::Value(
                descriptor.map_or(Value::Undefined, |descriptor| descriptor.value),
            ));
        }

        let own_descriptor = self
            .heap
            .with_obj(idx.0, |object| object.props().lock().get(key).cloned());
        if let Some(descriptor) = &own_descriptor {
            if descriptor.is_accessor {
                return Ok(GetOwnPropertyOutcome::Accessor(descriptor.get.clone()));
            }
        }

        if let Some(index) = key_str.and_then(crate::value::parse_array_index) {
            if let Some((env, name)) = self.arguments_mapped_binding_for_index(idx.0, index) {
                if let Some(value) = crate::environment::get(&self.heap, env, &name) {
                    return Ok(GetOwnPropertyOutcome::Value(value));
                }
            }
        }
        if let Some(descriptor) = own_descriptor {
            return Ok(GetOwnPropertyOutcome::Value(descriptor.value));
        }

        if let Some(name) = key_str {
            let restricted_function_special = self.heap.with_obj(idx.0, |object| {
                if let HeapObj::Function(function) = object {
                    if matches!(name, "caller" | "arguments") {
                        if let crate::value::FunctionKind::Interpreted { func } = &function.kind {
                            if !func.is_arrow
                                && !func.is_async
                                && !func.is_generator
                                && !func.chunk.is_strict
                            {
                                return Some(name == "caller");
                            }
                        }
                    }
                }
                None
            });
            if let Some(is_caller) = restricted_function_special {
                return if is_caller {
                    self.function_caller_value(*idx)
                        .map(GetOwnPropertyOutcome::Value)
                } else {
                    Ok(GetOwnPropertyOutcome::Value(Value::Undefined))
                };
            }

            let is_global_this = self.heap.with_obj(idx.0, |object| {
                matches!(object, HeapObj::Object(data) if data.class_name.as_deref() == Some("global"))
            });
            if is_global_this {
                if let Some(value) = crate::environment::get(&self.heap, self.global, name) {
                    return Ok(GetOwnPropertyOutcome::Value(value));
                }
            }
        }

        let exotic_value = self.heap.with_obj(idx.0, |object| {
            if let HeapObj::Array(array) = object {
                if key_str == Some("length")
                    && !array
                        .is_arguments
                        .load(std::sync::atomic::Ordering::Relaxed)
                {
                    let dense_length = array.items.lock().len();
                    let sparse_length = array.sparse_max.lock().unwrap_or(0);
                    return Some(Value::Number(dense_length.max(sparse_length) as f64));
                }
                if let Some(index) = key_str.and_then(crate::value::parse_array_index) {
                    if array.is_dense_present(index) {
                        return Some(
                            array
                                .items
                                .lock()
                                .get(index)
                                .cloned()
                                .unwrap_or(Value::Undefined),
                        );
                    }
                }
            }
            if let HeapObj::Object(data) = object {
                if let Some(Value::String(string)) = data.primitive.lock().clone() {
                    if key_str == Some("length") {
                        return Some(Value::Number(crate::value::utf16_len(&string) as f64));
                    }
                    if let Some(index) = crate::builtins::canonical_string_index(key) {
                        if let Some(unit) = crate::value::utf16_get(&string, index) {
                            return Some(Value::String(Arc::from(
                                crate::value::utf16_to_string(&[unit]).as_str(),
                            )));
                        }
                    }
                }
            }
            None
        });
        if let Some(value) = exotic_value {
            return Ok(GetOwnPropertyOutcome::Value(value));
        }

        let function_value = self.heap.with_obj(idx.0, |object| {
            let HeapObj::Function(function) = object else {
                return None;
            };
            match key_str {
                Some("prototype") => function.prototype.lock().clone(),
                // Bound functions install a real configurable own `name`.
                // Once deleted, ordinary prototype lookup must proceed instead
                // of reviving the internal diagnostic name stored in FunctionData.
                Some("name")
                    if matches!(&function.kind, crate::value::FunctionKind::Bound { .. }) =>
                {
                    None
                }
                Some("name") => Some(function.name.as_ref().map_or_else(
                    || Value::String(Arc::from("")),
                    |name| Value::String(name.clone()),
                )),
                Some("length") => match &function.kind {
                    crate::value::FunctionKind::Native { length, .. } => {
                        Some(Value::Number(*length as f64))
                    }
                    crate::value::FunctionKind::Interpreted { func } => {
                        Some(Value::Number(func.length as f64))
                    }
                    _ => None,
                },
                _ => None,
            }
        });
        Ok(function_value.map_or(GetOwnPropertyOutcome::Absent, GetOwnPropertyOutcome::Value))
    }

    pub(crate) fn get_property_rx(
        &mut self,
        obj: &Value,
        key: &str,
        receiver: Value,
    ) -> error::Result<Value> {
        self.get_property_key_rx(obj, &crate::value::PropertyKey::from(key), receiver)
    }

    pub(crate) fn get_property_key_rx(
        &mut self,
        obj: &Value,
        key: &crate::value::PropertyKey,
        receiver: Value,
    ) -> error::Result<Value> {
        self.get_property_key_rx_with_mode(obj, key, receiver, false, 0)
    }

    pub(crate) fn get_property_key_direct(
        &mut self,
        obj: &Value,
        key: &crate::value::PropertyKey,
        receiver: Value,
    ) -> error::Result<Value> {
        self.get_property_key_rx_with_mode(obj, key, receiver, true, 0)
    }

    pub(crate) fn get_proxy_method(&mut self, handler: &Value, key: &str) -> error::Result<Value> {
        self.get_property_key_rx_with_mode(
            handler,
            &crate::value::PropertyKey::from(key),
            handler.clone(),
            true,
            1,
        )
    }

    fn get_property_key_rx_with_mode(
        &mut self,
        obj: &Value,
        key: &crate::value::PropertyKey,
        receiver: Value,
        mut include_direct_exotics: bool,
        ordinary_edge_credit: usize,
    ) -> error::Result<Value> {
        let traversal_roots = [obj.clone(), receiver.clone()];
        let mut traversal =
            self.try_new_property_traversal(&traversal_roots, ordinary_edge_credit)?;
        let root_pins = self.pin_many(&traversal_roots);
        let result = (|| {
            let mut current = obj.clone();
            loop {
                let Value::Object(idx) = &current else {
                    return Ok(Value::Undefined);
                };
                let idx = *idx;
                let proxy_info = self.heap.with_obj(idx.0, |object| {
                    let HeapObj::Proxy(proxy) = object else {
                        return None;
                    };
                    if *proxy.revoked.lock() {
                        return Some(Err(Error::type_err(
                            "Cannot perform 'get' on a proxy that has been revoked",
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
                        let trap = self.get_proxy_method(&handler, "get")?;
                        if trap.is_nullish() {
                            return Ok(None);
                        }
                        if !crate::builtins::is_callable(&trap, &self.heap) {
                            return Err(Error::type_err("Proxy get trap is not callable"));
                        }
                        let trap_result = self.call_function(
                            &trap,
                            &[
                                target.clone(),
                                Self::property_key_to_value(key),
                                receiver.clone(),
                            ],
                            Some(handler.clone()),
                        )?;
                        let result_pin = self.pin(&trap_result);
                        let validation = self.validate_proxy_get_result(&target, key, &trap_result);
                        self.unpin(result_pin);
                        validation?;
                        Ok(Some(trap_result))
                    })();
                    self.unpin_many(proxy_pins);
                    match proxy_result? {
                        Some(value) => return Ok(value),
                        None => {
                            self.advance_property_edge(&mut traversal, idx, &target, false)?;
                            current = target;
                            include_direct_exotics = false;
                            continue;
                        }
                    }
                }

                match self.get_own_property_for_get(&current, key, include_direct_exotics)? {
                    GetOwnPropertyOutcome::Value(value) => return Ok(value),
                    GetOwnPropertyOutcome::Accessor(getter) => {
                        let Some(getter) = getter.filter(|getter| !getter.is_undefined()) else {
                            return Ok(Value::Undefined);
                        };
                        return self.call_function(&getter, &[], Some(receiver.clone()));
                    }
                    GetOwnPropertyOutcome::Absent => {}
                }

                let prototype = self.heap.with_obj(idx.0, |object| {
                    object.proto().lock().clone().unwrap_or(Value::Undefined)
                });
                let Value::Object(prototype_idx) = &prototype else {
                    return Ok(Value::Undefined);
                };
                let prototype_is_proxy = self.heap.with_obj(prototype_idx.0, |object| {
                    matches!(object, HeapObj::Proxy(_))
                });
                self.advance_property_edge(&mut traversal, idx, &prototype, !prototype_is_proxy)?;
                current = prototype;
                include_direct_exotics = false;
            }
        })();
        self.unpin_many(root_pins + traversal.pin_count());
        result
    }

    fn validate_proxy_get_result(
        &mut self,
        target: &Value,
        key: &crate::value::PropertyKey,
        trap_result: &Value,
    ) -> error::Result<()> {
        let Some(target_desc) =
            crate::builtins::own_property_descriptor_for_key_or_throw(self, target, key)?
        else {
            return Ok(());
        };
        if target_desc.configurable {
            return Ok(());
        }
        if !target_desc.is_accessor {
            if !target_desc.writable && !descriptor_same_value(trap_result, &target_desc.value) {
                return Err(Error::type_err(
                    "Proxy get trap must return the value of a non-writable, non-configurable property",
                ));
            }
            return Ok(());
        }
        let getter_is_undefined = target_desc.get.as_ref().is_none_or(Value::is_undefined);
        if getter_is_undefined && !trap_result.is_undefined() {
            return Err(Error::type_err(
                "Proxy get trap must return undefined for a non-configurable accessor without a getter",
            ));
        }
        Ok(())
    }

    pub(crate) fn get_proto_property(&mut self, obj: &Value, key: &str) -> error::Result<Value> {
        let proto = self.current_realm_primitive_prototype(obj);
        if !proto.is_undefined() {
            return self.get_property_rx(&proto, key, obj.clone());
        }
        Ok(Value::Undefined)
    }

    /// Delete an own property. Returns true if removed (or didn't exist).
    pub fn delete_property(&mut self, obj: &Value, key: &str) -> error::Result<bool> {
        self.delete_property_key(obj, &crate::value::PropertyKey::from(key))
    }

    /// Delete an own string or Symbol property via the object's internal
    /// [[Delete]] operation, including Proxy `deleteProperty` traps.
    pub(crate) fn delete_property_key(
        &mut self,
        obj: &Value,
        key: &crate::value::PropertyKey,
    ) -> error::Result<bool> {
        let root_pin = self.pin(obj);
        let result = (|| {
            let mut current = obj.clone();
            let idx = loop {
                let Value::Object(idx) = &current else {
                    return Ok(true);
                };
                let proxy_result = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Proxy(proxy) = o {
                        if *proxy.revoked.lock() {
                            return Some(Err(Error::type_err(
                                "Cannot perform 'deleteProperty' on a proxy that has been revoked",
                            )));
                        }
                        Some(Ok((proxy.target.clone(), proxy.handler.clone())))
                    } else {
                        None
                    }
                });
                let Some(proxy_result) = proxy_result else {
                    break *idx;
                };

                let (target, handler) = proxy_result?;
                self.consume_fuel()?;
                let proxy_pins = self.pin_many(&[target.clone(), handler.clone()]);
                let proxy_result = (|| {
                    let trap = self.get_proxy_method(&handler, "deleteProperty")?;
                    if trap.is_nullish() {
                        return Ok(None);
                    }
                    let trap_pin = self.pin(&trap);
                    let trap_result = self.call_function(
                        &trap,
                        &[target.clone(), Self::property_key_to_value(key)],
                        Some(handler.clone()),
                    );
                    self.unpin(trap_pin);
                    let trap_result = trap_result?;
                    if !self.to_boolean(&trap_result) {
                        return Ok(Some(false));
                    }
                    if let Some(desc) = crate::builtins::own_property_descriptor_for_key_or_throw(
                        self, &target, key,
                    )? {
                        if !desc.configurable {
                            return Err(Error::type_err(
                                "Proxy deleteProperty trap cannot delete non-configurable property",
                            ));
                        }
                        if !self.is_extensible(&target)? {
                            return Err(Error::type_err(
                                "Proxy deleteProperty trap cannot delete non-extensible target property",
                            ));
                        }
                    }
                    Ok(Some(true))
                })();
                self.unpin_many(proxy_pins);

                match proxy_result? {
                    Some(result) => return Ok(result),
                    None => current = target,
                }
            };

            if let Some(name) = key.as_str() {
                let namespace_export = self.heap.with_obj(idx.0, |o| {
                    matches!(o, HeapObj::ModuleNamespace(namespace) if namespace.exports.lock().contains_key(name))
                });
                if namespace_export {
                    return Ok(false);
                }
                if let Some(slots) = self.typed_array_numeric_slots(idx, name) {
                    return Ok(!self.is_valid_typed_array_numeric_index(&slots));
                }
            }

            let array_delete = self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Array(a) = o {
                    if key.as_str().is_some_and(|name| name == "length") {
                        if a.is_arguments.load(std::sync::atomic::Ordering::Relaxed) {
                            let (exists, configurable) = a
                                .props
                                .lock()
                                .get(key)
                                .map_or((false, true), |d| (true, d.configurable));
                            if exists && !configurable {
                                return Some(false);
                            }
                            if exists {
                                a.props.lock().shift_remove(key);
                            }
                            return Some(true);
                        }
                        return Some(false);
                    }
                    if let Some(i) = key.as_str().and_then(crate::value::parse_array_index) {
                        let (exists, configurable) = {
                            let props = a.props.lock();
                            if let Some(desc) = props.get(key) {
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
                            a.props.lock().shift_remove(key);
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
            let string_exotic_nonconfigurable = self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Object(od) = o {
                    if let Some(Value::String(s)) = od.primitive.lock().clone() {
                        return key.as_str().is_some_and(|name| {
                            let len = crate::value::utf16_len(&s);
                            name == "length"
                                || crate::builtins::canonical_string_index(key)
                                    .is_some_and(|i| i < len)
                        });
                    }
                }
                false
            });
            if string_exotic_nonconfigurable {
                return Ok(false);
            }
            let (exists, configurable) = self.heap.with_obj(idx.0, |o| {
                o.props()
                    .lock()
                    .get(key)
                    .map_or((false, true), |d| (true, d.configurable))
            });
            if exists && !configurable {
                return Ok(false);
            }
            self.heap.with_obj(idx.0, |o| {
                o.props().lock().shift_remove(key);
            });
            if let Some(name) = key.as_str() {
                self.ic_invalidate(idx.0, name);
                let realm_env = self.realm_globals.iter().find_map(|(env, global)| {
                    matches!(global, Value::Object(global_idx) if global_idx == &idx)
                        .then_some(GcIdx(*env))
                });
                if let Some(realm_env) = realm_env {
                    crate::environment::delete_global_var_binding_exact(
                        &self.heap, realm_env, name,
                    );
                }
            }
            Ok(true)
        })();
        self.unpin(root_pin);
        result
    }

    pub fn set_property(&mut self, obj: &Value, key: &str, value: Value) -> error::Result<()> {
        self.set_property_impl(obj, key, value, true, false)
    }

    pub(crate) fn set_property_strict(
        &mut self,
        obj: &Value,
        key: &str,
        value: Value,
    ) -> error::Result<()> {
        self.set_property_impl(obj, key, value, true, true)
    }

    pub(crate) fn set_object_environment_property(
        &mut self,
        obj: &Value,
        key: &str,
        value: Value,
    ) -> error::Result<()> {
        self.set_property_impl(obj, key, value, false, false)
    }

    pub(crate) fn define_data_property(
        &mut self,
        obj: &Value,
        key: crate::value::PropertyKey,
        value: Value,
    ) -> error::Result<()> {
        self.define_own_property_or_throw(obj, key, crate::value::PropertyDescriptor::data(value))
    }

    pub(crate) fn create_data_property(
        &mut self,
        obj: &Value,
        key: crate::value::PropertyKey,
        value: Value,
    ) -> error::Result<bool> {
        self.define_own_property(obj, key, crate::value::PropertyDescriptor::data(value))
    }

    pub(crate) fn define_own_property_or_throw(
        &mut self,
        obj: &Value,
        key: crate::value::PropertyKey,
        desc: crate::value::PropertyDescriptor,
    ) -> error::Result<()> {
        if self.define_own_property(obj, key, desc)? {
            Ok(())
        } else {
            Err(Error::type_err(
                "Cannot define property: descriptor is incompatible or object is not extensible",
            ))
        }
    }

    pub(crate) fn define_own_property(
        &mut self,
        obj: &Value,
        key: crate::value::PropertyKey,
        desc: crate::value::PropertyDescriptor,
    ) -> error::Result<bool> {
        let proxy_descriptor = ProxyDefinePropertyDescriptor::complete(desc.clone());
        let object = match self.proxy_define_own_property(obj, &key, &proxy_descriptor, None)? {
            ProxyDefinePropertyOutcome::Ordinary(object) => object,
            ProxyDefinePropertyOutcome::Complete(result) => return Ok(result),
        };
        let obj = &object;
        if let Value::Object(idx) = obj {
            if let Some(name) = key.as_str() {
                self.ic_invalidate(idx.0, name);
            }
            let is_array_length = key.as_str() == Some("length")
                && self.heap.with_obj(idx.0, |object| {
                    matches!(object, HeapObj::Array(array) if !array.is_arguments.load(std::sync::atomic::Ordering::Relaxed))
                });
            if is_array_length {
                return self.define_array_length_property(
                    idx.0,
                    (!desc.is_accessor).then(|| desc.value.clone()),
                    !desc.is_accessor,
                    desc.writable,
                    true,
                    desc.enumerable,
                    true,
                    desc.configurable,
                    desc.is_accessor,
                );
            }
            let is_namespace = self
                .heap
                .with_obj(idx.0, |o| matches!(o, HeapObj::ModuleNamespace(_)));
            if is_namespace {
                let current = self.own_property_descriptor_for_proxy_invariant(obj, &key);
                let compatible = current.is_some_and(|current| {
                    !desc.configurable
                        && desc.enumerable == current.enumerable
                        && !desc.is_accessor
                        && (!desc.writable || current.writable)
                        && desc.value == current.value
                });
                return Ok(compatible);
            }
            let array_index = key.as_str().and_then(crate::value::parse_array_index);
            if array_index
                .is_some_and(|index| self.array_index_blocked_by_non_writable_length(idx.0, index))
            {
                return Ok(false);
            }
            let current = self.own_property_descriptor_for_proxy_invariant(obj, &key);
            let extensible = self.heap.with_obj(idx.0, |o| o.is_extensible());
            if !compatible_complete_descriptor(current.as_ref(), &desc, extensible) {
                return Ok(false);
            }
            self.heap.with_obj(idx.0, |o| {
                let HeapObj::Array(array) = o else {
                    o.props().lock().insert(key.clone(), desc.clone());
                    return;
                };
                let Some(index) = array_index else {
                    array.props.lock().insert(key.clone(), desc.clone());
                    return;
                };

                let is_arguments = array
                    .is_arguments
                    .load(std::sync::atomic::Ordering::Relaxed);
                let dense_default_data = !is_arguments
                    && !desc.is_accessor
                    && desc.writable
                    && desc.enumerable
                    && desc.configurable
                    && index < crate::value::MAX_DENSE_ARRAY_LEN;
                if dense_default_data {
                    let mut items = array.items.lock();
                    let mut present = array.present.lock();
                    while items.len() <= index {
                        items.push(Value::Undefined);
                        present.push(false);
                    }
                    items[index] = desc.value.clone();
                    if present.len() <= index {
                        present.resize(index + 1, false);
                    }
                    present[index] = true;
                    let dense_length = items.len();
                    array.props.lock().shift_remove(&key);
                    let mut sparse_max = array.sparse_max.lock();
                    if sparse_max.is_some_and(|sparse| sparse <= dense_length) {
                        *sparse_max = None;
                    }
                    return;
                }

                if is_arguments && !desc.is_accessor && index < crate::value::MAX_DENSE_ARRAY_LEN {
                    let mut items = array.items.lock();
                    let mut present = array.present.lock();
                    while items.len() <= index {
                        items.push(Value::Undefined);
                        present.push(false);
                    }
                    items[index] = desc.value.clone();
                    if present.len() <= index {
                        present.resize(index + 1, false);
                    }
                    present[index] = true;
                } else {
                    let dense_length = array.items.lock().len();
                    if index < dense_length {
                        if let Some(item) = array.items.lock().get_mut(index) {
                            *item = Value::Undefined;
                        }
                        if let Some(slot) = array.present.lock().get_mut(index) {
                            *slot = false;
                        }
                    } else {
                        let mut sparse_max = array.sparse_max.lock();
                        if sparse_max.is_none_or(|current| index >= current) {
                            *sparse_max = Some(index + 1);
                        }
                    }
                }
                array.props.lock().insert(key.clone(), desc.clone());
            });
            if array_index.is_some() {
                self.sync_array_length_descriptor_after_index(idx.0);
            }
            Ok(true)
        } else {
            Err(Error::type_err(
                "Cannot define property of primitive".to_string(),
            ))
        }
    }

    fn proxy_property_descriptor_object(
        &mut self,
        desc: &ProxyDefinePropertyDescriptor,
    ) -> error::Result<Value> {
        let desc_idx = self.new_object_in_current_realm()?;
        let desc_obj = Value::Object(desc_idx);
        self.heap.with_obj(desc_idx.0, |o| {
            let props = o.props();
            let mut props = props.lock();
            if desc.has_value {
                props.insert(
                    crate::value::PropertyKey::from("value"),
                    crate::value::PropertyDescriptor::data(desc.descriptor.value.clone()),
                );
            }
            if desc.has_writable {
                props.insert(
                    crate::value::PropertyKey::from("writable"),
                    crate::value::PropertyDescriptor::data(Value::Bool(desc.descriptor.writable)),
                );
            }
            if desc.has_get {
                props.insert(
                    crate::value::PropertyKey::from("get"),
                    crate::value::PropertyDescriptor::data(
                        desc.descriptor.get.clone().unwrap_or(Value::Undefined),
                    ),
                );
            }
            if desc.has_set {
                props.insert(
                    crate::value::PropertyKey::from("set"),
                    crate::value::PropertyDescriptor::data(
                        desc.descriptor.set.clone().unwrap_or(Value::Undefined),
                    ),
                );
            }
            if desc.has_enumerable {
                props.insert(
                    crate::value::PropertyKey::from("enumerable"),
                    crate::value::PropertyDescriptor::data(Value::Bool(desc.descriptor.enumerable)),
                );
            }
            if desc.has_configurable {
                props.insert(
                    crate::value::PropertyKey::from("configurable"),
                    crate::value::PropertyDescriptor::data(Value::Bool(
                        desc.descriptor.configurable,
                    )),
                );
            }
        });
        Ok(desc_obj)
    }

    /// Run Proxy [[DefineOwnProperty]] until a trap completes or an ordinary
    /// target is reached, retaining descriptor presence, roots, and fuel.
    pub(crate) fn proxy_define_own_property(
        &mut self,
        object: &Value,
        key: &crate::value::PropertyKey,
        desc: &ProxyDefinePropertyDescriptor,
        descriptor_object: Option<&Value>,
    ) -> error::Result<ProxyDefinePropertyOutcome> {
        let mut roots = vec![object.clone(), desc.descriptor.value.clone()];
        roots.extend(desc.descriptor.get.iter().cloned());
        roots.extend(desc.descriptor.set.iter().cloned());
        roots.extend(descriptor_object.iter().cloned().cloned());
        let root_pins = self.pin_many(&roots);
        let mut current = object.clone();
        let result = (|| loop {
            let Value::Object(idx) = &current else {
                return Ok(ProxyDefinePropertyOutcome::Ordinary(current));
            };
            let proxy_info = self.heap.with_obj(idx.0, |heap_object| {
                let HeapObj::Proxy(proxy) = heap_object else {
                    return None;
                };
                if *proxy.revoked.lock() {
                    return Some(Err(Error::type_err(
                        "Cannot perform 'defineProperty' on a proxy that has been revoked",
                    )));
                }
                Some(Ok((proxy.target.clone(), proxy.handler.clone())))
            });
            let Some(proxy_info) = proxy_info else {
                return Ok(ProxyDefinePropertyOutcome::Ordinary(current));
            };

            let (target, handler) = proxy_info?;
            self.consume_fuel()?;
            let proxy_pins = self.pin_many(&[target.clone(), handler.clone()]);
            let proxy_result = (|| {
                let trap = self.get_proxy_method(&handler, "defineProperty")?;
                if trap.is_nullish() {
                    return Ok(None);
                }
                if !crate::builtins::is_callable(&trap, &self.heap) {
                    return Err(Error::type_err("Proxy defineProperty trap is not callable"));
                }
                let trap_pin = self.pin(&trap);
                let descriptor_object = match descriptor_object {
                    Some(descriptor_object) => Ok(descriptor_object.clone()),
                    None => self.proxy_property_descriptor_object(desc),
                };
                let descriptor_object = match descriptor_object {
                    Ok(descriptor_object) => descriptor_object,
                    Err(error) => {
                        self.unpin(trap_pin);
                        return Err(error);
                    }
                };
                let descriptor_pin = self.pin(&descriptor_object);
                let trap_result = self.call_function(
                    &trap,
                    &[
                        target.clone(),
                        Self::property_key_to_value(key),
                        descriptor_object,
                    ],
                    Some(handler.clone()),
                );
                self.unpin(descriptor_pin);
                self.unpin(trap_pin);
                let trap_result = trap_result?;
                if !self.to_boolean(&trap_result) {
                    return Ok(Some(false));
                }

                self.validate_proxy_define_own_property_result(&target, key, desc)?;
                Ok(Some(true))
            })();
            self.unpin_many(proxy_pins);

            match proxy_result? {
                Some(result) => return Ok(ProxyDefinePropertyOutcome::Complete(result)),
                None => current = target,
            }
        })();
        self.unpin_many(root_pins);
        result
    }

    fn validate_proxy_define_own_property_result(
        &mut self,
        target: &Value,
        key: &crate::value::PropertyKey,
        desc: &ProxyDefinePropertyDescriptor,
    ) -> error::Result<()> {
        let target_desc =
            crate::builtins::own_property_descriptor_for_key_or_throw(self, target, key)?;
        let mut target_desc_roots = Vec::new();
        if let Some(target_desc) = target_desc.as_ref() {
            target_desc_roots.push(target_desc.value.clone());
            target_desc_roots.extend(target_desc.get.iter().cloned());
            target_desc_roots.extend(target_desc.set.iter().cloned());
        }
        let target_desc_pins = self.pin_many(&target_desc_roots);
        let validation = (|| {
            let extensible = self.is_extensible(target)?;
            let setting_configurable_false = desc.has_configurable && !desc.descriptor.configurable;
            let violates_invariant = match target_desc.as_ref() {
                None => !extensible || setting_configurable_false,
                Some(target_desc) => {
                    !compatible_proxy_define_descriptor(Some(target_desc), desc, extensible)
                        || (setting_configurable_false && target_desc.configurable)
                        || (!target_desc.configurable
                            && !target_desc.is_accessor
                            && target_desc.writable
                            && desc.has_writable
                            && !desc.descriptor.writable)
                }
            };
            if violates_invariant {
                return Err(Error::type_err(
                    "Proxy defineProperty trap violated the target invariant",
                ));
            }
            Ok(())
        })();
        self.unpin_many(target_desc_pins);
        validation
    }

    pub(crate) fn prevent_extensions(&mut self, obj: &Value) -> error::Result<bool> {
        let root_pin = self.pin(obj);
        let mut current = obj.clone();
        let result = (|| loop {
            let Value::Object(idx) = &current else {
                return Ok(true);
            };
            let proxy_info = self.heap.with_obj(idx.0, |object| {
                if let HeapObj::Proxy(proxy) = object {
                    if *proxy.revoked.lock() {
                        return Some(Err(Error::type_err(
                            "Cannot perform 'preventExtensions' on a proxy that has been revoked",
                        )));
                    }
                    Some(Ok((proxy.target.clone(), proxy.handler.clone())))
                } else {
                    None
                }
            });
            let Some(proxy_info) = proxy_info else {
                self.heap.with_obj(idx.0, HeapObj::prevent_extensions);
                return Ok(true);
            };

            let (target, handler) = proxy_info?;
            self.consume_fuel()?;
            let proxy_pins = self.pin_many(&[target.clone(), handler.clone()]);
            let proxy_result = (|| {
                let trap = self.get_proxy_method(&handler, "preventExtensions")?;
                if trap.is_nullish() {
                    return Ok(None);
                }
                let trap_pin = self.pin(&trap);
                let trap_result =
                    self.call_function(&trap, std::slice::from_ref(&target), Some(handler.clone()));
                self.unpin(trap_pin);
                let trap_result = trap_result?;
                if !self.to_boolean(&trap_result) {
                    return Ok(Some(false));
                }
                if self.is_extensible(&target)? {
                    return Err(Error::type_err(
                        "Proxy preventExtensions trap cannot report success for extensible target",
                    ));
                }
                Ok(Some(true))
            })();
            self.unpin_many(proxy_pins);

            match proxy_result? {
                Some(result) => return Ok(result),
                None => current = target,
            }
        })();
        self.unpin(root_pin);
        result
    }

    pub(crate) fn is_extensible(&mut self, obj: &Value) -> error::Result<bool> {
        self.try_reserve_value_roots(std::slice::from_ref(obj))?;
        let root_pin = self.pin(obj);
        let mut current = obj.clone();
        let mut first_trap_result = None;
        let mut inconsistent_trap_results = false;
        let result = (|| loop {
            let Value::Object(idx) = &current else {
                return Ok(false);
            };
            let proxy_info = self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Proxy(proxy) = o {
                    if *proxy.revoked.lock() {
                        return Some(Err(Error::type_err(
                            "Cannot perform 'isExtensible' on a proxy that has been revoked",
                        )));
                    }
                    Some(Ok((proxy.target.clone(), proxy.handler.clone())))
                } else {
                    None
                }
            });
            let Some(proxy_info) = proxy_info else {
                let target_result = self.heap.with_obj(idx.0, |o| o.is_extensible());
                if inconsistent_trap_results
                    || first_trap_result.is_some_and(|result| result != target_result)
                {
                    return Err(Error::type_err(
                        "Proxy isExtensible trap result must match target extensibility",
                    ));
                }
                return Ok(target_result);
            };
            let (target, handler) = proxy_info?;
            self.consume_fuel()?;
            let proxy_roots = [target.clone(), handler.clone()];
            self.try_reserve_value_roots(&proxy_roots)?;
            let proxy_pins = self.pin_many(&proxy_roots);
            let trap = match self.get_proxy_method(&handler, "isExtensible") {
                Ok(trap) => trap,
                Err(error) => {
                    self.unpin_many(proxy_pins);
                    return Err(error);
                }
            };
            if trap.is_nullish() {
                self.unpin_many(proxy_pins);
                current = target;
                continue;
            }
            if let Err(error) = self.try_reserve_value_roots(std::slice::from_ref(&trap)) {
                self.unpin_many(proxy_pins);
                return Err(error);
            }
            let trap_pin = self.pin(&trap);
            let trap_result =
                self.call_function(&trap, std::slice::from_ref(&target), Some(handler));
            self.unpin(trap_pin);
            let trap_result = match trap_result {
                Ok(result) => result,
                Err(error) => {
                    self.unpin_many(proxy_pins);
                    return Err(error);
                }
            };
            let boolean_trap_result = self.to_boolean(&trap_result);
            if let Some(first) = first_trap_result {
                inconsistent_trap_results |= first != boolean_trap_result;
            } else {
                first_trap_result = Some(boolean_trap_result);
            }
            self.unpin_many(proxy_pins);
            current = target;
        })();
        self.unpin(root_pin);
        result
    }

    fn set_property_impl(
        &mut self,
        obj: &Value,
        key: &str,
        value: Value,
        route_global_this: bool,
        force_strict: bool,
    ) -> error::Result<()> {
        let strict = force_strict || self.current_strict();
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
                // Native abstract operations bypass the bytecode store opcodes,
                // so invalidate their receiver cache entry here as well.
                self.ic_invalidate(idx.0, key);
                let is_global_this = self.heap.with_obj(idx.0, |o| {
                    matches!(o, HeapObj::Object(od) if od.class_name.as_deref() == Some("global"))
                });
                let is_proxy = self
                    .heap
                    .with_obj(idx.0, |object| matches!(object, HeapObj::Proxy(_)));
                if is_proxy {
                    let success = self.try_set_property_with_receiver(obj, key, value, obj)?;
                    if !success && strict {
                        return Err(Error::type_err("Proxy set operation returned false"));
                    }
                    return Ok(());
                }
                let is_namespace = self
                    .heap
                    .with_obj(idx.0, |o| matches!(o, HeapObj::ModuleNamespace(_)));
                if is_namespace {
                    if strict {
                        return Err(Error::type_err(format!(
                            "Cannot assign to read only module namespace property '{}'",
                            key
                        )));
                    }
                    return Ok(());
                }
                if self
                    .set_typed_array_numeric_property(*idx, key, &value)?
                    .is_some()
                {
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
                        let success = self.ordinary_set_with_receiver(*idx, &pkey, value, &obj)?;
                        if !success && strict {
                            return Err(Error::type_err(format!(
                                "Cannot assign to read only property '{}' of object",
                                key
                            )));
                        }
                        return Ok(());
                    }
                    let length_writable = self.heap.with_obj(idx.0, |object| {
                        let HeapObj::Array(array) = object else {
                            return false;
                        };
                        array
                            .props
                            .lock()
                            .get(&crate::value::PropertyKey::from("length"))
                            .is_none_or(|descriptor| descriptor.writable)
                    });
                    if !length_writable {
                        if strict {
                            return Err(Error::type_err("Cannot assign to read only array length"));
                        }
                        return Ok(());
                    }
                    let success = self.try_set_array_length(idx.0, value)?;
                    if !success && strict {
                        return Err(Error::type_err("Cannot assign to read only array length"));
                    }
                    return Ok(());
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
                            if strict {
                                return Err(Error::type_err(format!(
                                    "Cannot set property '{}' which has only a getter",
                                    key
                                )));
                            }
                            return Ok(());
                        }
                        if !desc.writable {
                            if strict {
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
                                let is_arguments =
                                    a.is_arguments.load(std::sync::atomic::Ordering::Relaxed);
                                let migrate_to_dense = !is_arguments
                                    && !desc.is_accessor
                                    && desc.writable
                                    && desc.enumerable
                                    && desc.configurable
                                    && i < crate::value::MAX_DENSE_ARRAY_LEN;
                                if (is_arguments || migrate_to_dense)
                                    && i < crate::value::MAX_DENSE_ARRAY_LEN
                                {
                                    let mut items = a.items.lock();
                                    let mut present = a.present.lock();
                                    while items.len() <= i {
                                        items.push(Value::Undefined);
                                        present.push(false);
                                    }
                                    items[i] = value.clone();
                                    if present.len() <= i {
                                        present.resize(i + 1, false);
                                    }
                                    present[i] = true;
                                }
                                let mut props = a.props.lock();
                                if migrate_to_dense {
                                    props.shift_remove(&pkey);
                                } else if let Some(descriptor) = props.get_mut(&pkey) {
                                    descriptor.value = value.clone();
                                }
                            }
                        });
                        return Ok(());
                    }
                    let dense_own_index = self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Array(a) = o {
                            a.is_dense_present(i)
                        } else {
                            false
                        }
                    });
                    if !dense_own_index {
                        // A missing Array element still uses ordinary [[Set]]
                        // prototype traversal. In particular, a Proxy in the
                        // chain must observe its `set` trap before Array length
                        // or extensibility can constrain creation on receiver.
                        let success =
                            self.try_set_property_key_with_receiver(obj, &pkey, value, obj)?;
                        if !success && strict {
                            return Err(Error::type_err(format!(
                                "Cannot assign to read only property '{}' of object",
                                key
                            )));
                        }
                        return Ok(());
                    }
                    // Dense array elements are own writable data properties,
                    // so prototype setters/non-writable data properties do
                    // not participate in this write.
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
                    self.sync_array_length_descriptor_after_index(idx.0);
                    return Ok(());
                }

                // --- Ordinary object [[Set]] ---
                let pkey = crate::value::PropertyKey::from(key);
                let string_exotic_index = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Object(od) = o {
                        if let Some(Value::String(s)) = od.primitive.lock().clone() {
                            return key
                                .parse::<usize>()
                                .ok()
                                .filter(|index| index.to_string() == key)
                                .is_some_and(|index| crate::value::utf16_get(&s, index).is_some());
                        }
                    }
                    false
                });
                if string_exotic_index {
                    if strict {
                        return Err(Error::type_err(format!(
                            "Cannot assign to read only property '{}' of object",
                            key
                        )));
                    }
                    return Ok(());
                }

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
                        if strict {
                            return Err(Error::type_err(format!(
                                "Cannot set property '{}' which has only a getter",
                                key
                            )));
                        }
                        return Ok(());
                    }
                    if !desc.writable {
                        if strict {
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

                let success = self.ordinary_set_with_receiver(*idx, &pkey, value, obj)?;
                if !success && strict {
                    return Err(Error::type_err(format!(
                        "Cannot assign to read only property '{}' of object",
                        key
                    )));
                }
                if success {
                    self.mirror_global_property_to_binding(
                        *idx,
                        key,
                        route_global_this,
                        is_global_this,
                    );
                }
                Ok(())
            }
            _ => {
                if obj.is_nullish() || strict {
                    Err(Error::type_err(
                        "Cannot set property of primitive".to_string(),
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    pub(crate) fn mirror_global_property_to_binding(
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
        let success = self.try_set_property_with_receiver(base, key, value, receiver)?;
        if !success && self.current_strict() {
            return Err(Error::type_err(format!(
                "Cannot assign to read only property '{}' of object",
                key
            )));
        }
        Ok(())
    }

    pub(crate) fn try_set_property_with_receiver(
        &mut self,
        base: &Value,
        key: &str,
        value: Value,
        receiver: &Value,
    ) -> error::Result<bool> {
        let pkey = crate::value::PropertyKey::from(key);
        self.try_set_property_key_with_receiver(base, &pkey, value, receiver)
    }

    pub(crate) fn try_set_property_key_with_receiver(
        &mut self,
        base: &Value,
        key: &crate::value::PropertyKey,
        value: Value,
        receiver: &Value,
    ) -> error::Result<bool> {
        let traversal_roots = [base.clone(), value.clone(), receiver.clone()];
        let mut traversal = self.try_new_property_traversal(&traversal_roots, 0)?;
        self.try_set_property_key_with_receiver_tracked(base, key, value, receiver, &mut traversal)
    }

    fn try_set_property_key_with_receiver_tracked(
        &mut self,
        base: &Value,
        key: &crate::value::PropertyKey,
        value: Value,
        receiver: &Value,
        traversal: &mut PropertyTraversal,
    ) -> error::Result<bool> {
        let root_pins = self.pin_many(&[base.clone(), value.clone(), receiver.clone()]);
        let mut current = base.clone();
        let result = (|| loop {
            let Value::Object(base_idx) = &current else {
                return Err(Error::type_err(
                    "Cannot set property of primitive".to_string(),
                ));
            };
            let base_idx = *base_idx;
            let proxy_info = self.heap.with_obj(base_idx.0, |object| {
                let HeapObj::Proxy(proxy) = object else {
                    return None;
                };
                if *proxy.revoked.lock() {
                    return Some(Err(Error::type_err(
                        "Cannot perform 'set' on a proxy that has been revoked",
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
                    let trap = self.get_proxy_method(&handler, "set")?;
                    if trap.is_nullish() {
                        return Ok(None);
                    }
                    if !crate::builtins::is_callable(&trap, &self.heap) {
                        return Err(Error::type_err("Proxy set trap is not callable"));
                    }
                    let trap_pin = self.pin(&trap);
                    let trap_result = self.call_function(
                        &trap,
                        &[
                            target.clone(),
                            Self::property_key_to_value(key),
                            value.clone(),
                            receiver.clone(),
                        ],
                        Some(handler.clone()),
                    );
                    self.unpin(trap_pin);
                    let trap_result = trap_result?;
                    if !self.to_boolean(&trap_result) {
                        return Ok(Some(false));
                    }
                    self.validate_proxy_set_result(&target, key, &value)?;
                    Ok(Some(true))
                })();
                self.unpin_many(proxy_pins);
                match proxy_result? {
                    Some(result) => return Ok(result),
                    None => {
                        traversal.grant_ordinary_edge_credit();
                        self.advance_property_edge(traversal, base_idx, &target, false)?;
                        current = target;
                        continue;
                    }
                }
            }

            match self.ordinary_set_with_receiver_tracked(
                base_idx,
                key,
                value.clone(),
                receiver,
                traversal,
            )? {
                OrdinarySetOutcome::Complete(result) => return Ok(result),
                OrdinarySetOutcome::Forward(next) => current = next,
            }
        })();
        self.unpin_many(root_pins + traversal.pin_count());
        result
    }

    fn validate_proxy_set_result(
        &mut self,
        target: &Value,
        key: &crate::value::PropertyKey,
        value: &Value,
    ) -> error::Result<()> {
        let Some(target_desc) =
            crate::builtins::own_property_descriptor_for_key_or_throw(self, target, key)?
        else {
            return Ok(());
        };
        if target_desc.configurable {
            return Ok(());
        }
        if !target_desc.is_accessor {
            if !target_desc.writable && !descriptor_same_value(value, &target_desc.value) {
                return Err(Error::type_err(
                    "Proxy set trap cannot change a non-writable, non-configurable property",
                ));
            }
            return Ok(());
        }
        if target_desc.set.as_ref().is_none_or(Value::is_undefined) {
            return Err(Error::type_err(
                "Proxy set trap cannot set a non-configurable accessor without a setter",
            ));
        }
        Ok(())
    }

    fn ordinary_set_with_receiver(
        &mut self,
        base_idx: GcIdx,
        pkey: &crate::value::PropertyKey,
        value: Value,
        receiver: &Value,
    ) -> error::Result<bool> {
        let base = Value::Object(base_idx);
        let traversal_roots = [base.clone(), value.clone(), receiver.clone()];
        let mut traversal = self.try_new_property_traversal(&traversal_roots, 0)?;
        self.try_set_property_key_with_receiver_tracked(
            &base,
            pkey,
            value,
            receiver,
            &mut traversal,
        )
    }

    fn ordinary_set_with_receiver_tracked(
        &mut self,
        mut base_idx: GcIdx,
        pkey: &crate::value::PropertyKey,
        value: Value,
        receiver: &Value,
        traversal: &mut PropertyTraversal,
    ) -> error::Result<OrdinarySetOutcome> {
        let key = pkey.as_str().unwrap_or("");
        loop {
            let is_module_namespace = self.heap.with_obj(base_idx.0, |object| {
                matches!(object, HeapObj::ModuleNamespace(_))
            });
            if is_module_namespace {
                return Ok(OrdinarySetOutcome::Complete(false));
            }
            let receiver_is_base =
                matches!(receiver, Value::Object(receiver_idx) if *receiver_idx == base_idx);
            if let Some(slots) = self.typed_array_numeric_slots(base_idx, key) {
                if receiver_is_base {
                    self.set_typed_array_numeric_slots(base_idx, slots, &value)?;
                    return Ok(OrdinarySetOutcome::Complete(true));
                }
                if !self.is_valid_typed_array_numeric_index(&slots) {
                    return Ok(OrdinarySetOutcome::Complete(true));
                }
                return self
                    .set_receiver_data_property(receiver, pkey.clone(), value)
                    .map(OrdinarySetOutcome::Complete);
            }
            let (desc, proto) = self.heap.with_obj(base_idx.0, |o| {
                let ordinary = o.props().lock().get(pkey).cloned();
                let array_exotic = ordinary.or_else(|| {
                    let HeapObj::Array(a) = o else {
                        return None;
                    };
                    if !a.is_arguments.load(std::sync::atomic::Ordering::Relaxed)
                        && pkey.as_str() == Some("length")
                    {
                        let length = a.items.lock().len().max(a.sparse_max.lock().unwrap_or(0));
                        let mut desc =
                            crate::value::PropertyDescriptor::data(Value::Number(length as f64));
                        desc.enumerable = false;
                        desc.configurable = false;
                        return Some(desc);
                    }
                    let index = pkey.as_str().and_then(crate::value::parse_array_index)?;
                    if !a.is_dense_present(index) {
                        return None;
                    }
                    Some(crate::value::PropertyDescriptor::data(Value::Undefined))
                });
                let string_exotic = array_exotic.or_else(|| {
                    let HeapObj::Object(od) = o else {
                        return None;
                    };
                    let Some(Value::String(s)) = od.primitive.lock().clone() else {
                        return None;
                    };
                    let name = pkey.as_str()?;
                    let is_length = name == "length";
                    let is_index = name
                        .parse::<usize>()
                        .is_ok_and(|i| i.to_string() == name && i < crate::value::utf16_len(&s));
                    if !is_length && !is_index {
                        return None;
                    }
                    let mut desc = crate::value::PropertyDescriptor::data(Value::Undefined);
                    desc.writable = false;
                    desc.enumerable = is_index;
                    desc.configurable = false;
                    Some(desc)
                });
                (string_exotic, o.proto().lock().clone())
            });
            if let Some(desc) = desc {
                if desc.is_accessor {
                    if let Some(setter) = desc.set {
                        self.call_function(
                            &setter,
                            std::slice::from_ref(&value),
                            Some(receiver.clone()),
                        )?;
                        return Ok(OrdinarySetOutcome::Complete(true));
                    }
                    return Ok(OrdinarySetOutcome::Complete(false));
                }
                if !desc.writable {
                    return Ok(OrdinarySetOutcome::Complete(false));
                }
                return self
                    .set_receiver_data_property(receiver, pkey.clone(), value)
                    .map(OrdinarySetOutcome::Complete);
            }
            match proto {
                Some(Value::Object(proto_idx)) => {
                    let is_proxy = self
                        .heap
                        .with_obj(proto_idx.0, |o| matches!(o, HeapObj::Proxy(_)));
                    let prototype = Value::Object(proto_idx);
                    self.advance_property_edge(traversal, base_idx, &prototype, !is_proxy)?;
                    if is_proxy {
                        return Ok(OrdinarySetOutcome::Forward(prototype));
                    }
                    base_idx = proto_idx;
                }
                _ => {
                    return self
                        .set_receiver_data_property(receiver, pkey.clone(), value)
                        .map(OrdinarySetOutcome::Complete)
                }
            }
        }
    }

    fn typed_array_numeric_slots(&self, idx: GcIdx, key: &str) -> Option<TypedArrayNumericSlots> {
        let numeric_index = crate::value::canonical_numeric_index_string(key)?;
        let (kind, viewed_array_buffer, byte_offset, byte_length, length_tracking) =
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::TypedArray(t) = o {
                    return Some((
                        t.kind,
                        t.viewed_array_buffer.clone(),
                        t.byte_offset,
                        t.byte_length,
                        t.length_tracking,
                    ));
                }
                None
            })?;
        Some(TypedArrayNumericSlots {
            kind,
            viewed_array_buffer,
            byte_offset,
            byte_length,
            length_tracking,
            numeric_index,
        })
    }

    fn is_valid_typed_array_numeric_index(&self, slots: &TypedArrayNumericSlots) -> bool {
        self.typed_array_valid_index(slots).is_some()
    }

    fn typed_array_valid_index(&self, slots: &TypedArrayNumericSlots) -> Option<usize> {
        let index = slots.numeric_index;
        if !index.is_finite()
            || index.is_sign_negative()
            || index.fract() != 0.0
            || index > usize::MAX as f64
        {
            return None;
        }
        let byte_length = crate::builtins::effective_view_byte_length(
            self,
            slots.viewed_array_buffer.as_ref(),
            slots.byte_offset,
            slots.byte_length,
            slots.length_tracking,
            slots.kind.element_size(),
        )?;
        let index = index as usize;
        let len = crate::builtins::typed_array_element_count(slots.kind, byte_length);
        (index < len).then_some(index)
    }

    pub(crate) fn typed_array_integer_index_own_property_descriptor(
        &self,
        obj: &Value,
        key: &crate::value::PropertyKey,
    ) -> Option<Option<crate::value::PropertyDescriptor>> {
        let Value::Object(idx) = obj else {
            return None;
        };
        let is_typed_array = self
            .heap
            .with_obj(idx.0, |o| matches!(o, HeapObj::TypedArray(_)));
        if !is_typed_array {
            return None;
        }
        let name = key.as_str()?;
        let slots = self.typed_array_numeric_slots(*idx, name)?;
        let Some(index) = self.typed_array_valid_index(&slots) else {
            return Some(None);
        };
        let value = if let Some(Value::Object(buffer_idx)) = &slots.viewed_array_buffer {
            let size = slots.kind.element_size();
            let Some(relative_offset) = index.checked_mul(size) else {
                return Some(None);
            };
            let Some(relative_end) = relative_offset.checked_add(size) else {
                return Some(None);
            };
            let byte_length = crate::builtins::effective_view_byte_length(
                self,
                slots.viewed_array_buffer.as_ref(),
                slots.byte_offset,
                slots.byte_length,
                slots.length_tracking,
                slots.kind.element_size(),
            )?;
            if relative_end > byte_length {
                return Some(None);
            }
            let value = self.heap.with_obj(buffer_idx.0, |o| {
                let HeapObj::ArrayBuffer(buffer) = o else {
                    return None;
                };
                if buffer.detached.load(std::sync::atomic::Ordering::Relaxed) {
                    return None;
                }
                let offset = slots.byte_offset.checked_add(relative_offset)?;
                let end = offset.checked_add(size)?;
                let bytes = buffer.bytes.lock();
                if end > bytes.len() {
                    return None;
                }
                crate::builtins::typed_array_read_element(slots.kind, &bytes[offset..end], 0)
            });
            let Some(value) = value else {
                return Some(None);
            };
            value
        } else {
            let value = self.heap.with_obj(idx.0, |o| {
                let HeapObj::TypedArray(t) = o else {
                    return None;
                };
                crate::builtins::typed_array_read_element(slots.kind, &t.buffer.lock(), index)
            });
            let Some(value) = value else {
                return Some(None);
            };
            value
        };
        let mut desc = crate::value::PropertyDescriptor::data(value);
        desc.writable = true;
        desc.enumerable = true;
        desc.configurable = true;
        Some(Some(desc))
    }

    pub(crate) fn typed_array_integer_index_has_property(
        &self,
        obj: &Value,
        key: &crate::value::PropertyKey,
    ) -> Option<bool> {
        let Value::Object(idx) = obj else {
            return None;
        };
        let name = key.as_str()?;
        let slots = self.typed_array_numeric_slots(*idx, name)?;
        Some(self.is_valid_typed_array_numeric_index(&slots))
    }

    pub(crate) fn typed_array_integer_index_own_property_key_count(
        &self,
        obj: &Value,
    ) -> Option<usize> {
        let Value::Object(idx) = obj else {
            return None;
        };
        self.heap.with_obj(idx.0, |o| {
            let HeapObj::TypedArray(array) = o else {
                return None;
            };
            let byte_length = crate::builtins::effective_view_byte_length(
                self,
                array.viewed_array_buffer.as_ref(),
                array.byte_offset,
                if array.viewed_array_buffer.is_some() {
                    array.byte_length
                } else {
                    array.buffer.lock().len()
                },
                array.length_tracking,
                array.kind.element_size(),
            )
            .unwrap_or(0);
            Some(crate::builtins::typed_array_element_count(
                array.kind,
                byte_length,
            ))
        })
    }

    fn set_typed_array_numeric_property(
        &mut self,
        idx: GcIdx,
        key: &str,
        value: &Value,
    ) -> error::Result<Option<bool>> {
        let Some(slots) = self.typed_array_numeric_slots(idx, key) else {
            return Ok(None);
        };
        self.set_typed_array_numeric_slots(idx, slots, value)
            .map(Some)
    }

    pub(crate) fn define_typed_array_integer_index_property(
        &mut self,
        obj: &Value,
        key: &crate::value::PropertyKey,
        desc: TypedArrayDefineDescriptor<'_>,
    ) -> error::Result<Option<bool>> {
        let Value::Object(idx) = obj else {
            return Ok(None);
        };
        let Some(name) = key.as_str() else {
            return Ok(None);
        };
        let Some(slots) = self.typed_array_numeric_slots(*idx, name) else {
            return Ok(None);
        };
        if self.typed_array_valid_index(&slots).is_none()
            || (desc.has_configurable && !desc.configurable)
            || (desc.has_enumerable && !desc.enumerable)
            || desc.is_accessor
            || (desc.has_writable && !desc.writable)
        {
            return Ok(Some(false));
        }
        if let Some(value) = desc.value {
            self.set_typed_array_numeric_slots(*idx, slots, value)?;
        }
        Ok(Some(true))
    }

    fn set_typed_array_numeric_slots(
        &mut self,
        idx: GcIdx,
        slots: TypedArrayNumericSlots,
        value: &Value,
    ) -> error::Result<bool> {
        let element_bytes = crate::builtins::typed_array_value_to_bytes(self, slots.kind, value)?;
        let Some(i) = self.typed_array_valid_index(&slots) else {
            return Ok(true);
        };
        let size = slots.kind.element_size();
        let Some(relative_offset) = i.checked_mul(size) else {
            return Ok(true);
        };
        let Some(relative_end) = relative_offset.checked_add(size) else {
            return Ok(true);
        };
        let Some(byte_length) = crate::builtins::effective_view_byte_length(
            self,
            slots.viewed_array_buffer.as_ref(),
            slots.byte_offset,
            slots.byte_length,
            slots.length_tracking,
            slots.kind.element_size(),
        ) else {
            return Ok(true);
        };
        if relative_end > byte_length {
            return Ok(true);
        }
        if let Some(Value::Object(buffer_idx)) = &slots.viewed_array_buffer {
            let immutable = self.heap.with_obj(buffer_idx.0, |o| {
                if let HeapObj::ArrayBuffer(buffer) = o {
                    return buffer.immutable.load(std::sync::atomic::Ordering::Relaxed);
                }
                false
            });
            if immutable {
                return Err(Error::type_err(
                    "Cannot write to immutable ArrayBuffer-backed TypedArray",
                ));
            }
        }
        if let Some(backing) = slots.viewed_array_buffer {
            if let Value::Object(buffer_idx) = backing {
                self.heap.with_obj(buffer_idx.0, |o| {
                    if let HeapObj::ArrayBuffer(buffer) = o {
                        if buffer.detached.load(std::sync::atomic::Ordering::Relaxed) {
                            return;
                        }
                        let Some(offset) = slots.byte_offset.checked_add(relative_offset) else {
                            return;
                        };
                        let Some(end) = offset.checked_add(size) else {
                            return;
                        };
                        let mut bytes = buffer.bytes.lock();
                        if end <= bytes.len() {
                            bytes[offset..end].copy_from_slice(&element_bytes);
                        }
                    }
                });
            }
        } else {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::TypedArray(t) = o {
                    let mut buffer = t.buffer.lock();
                    if relative_end <= buffer.len() {
                        buffer[relative_offset..relative_end].copy_from_slice(&element_bytes);
                    }
                }
            });
        }
        Ok(true)
    }

    fn set_receiver_data_property(
        &mut self,
        receiver: &Value,
        pkey: crate::value::PropertyKey,
        value: Value,
    ) -> error::Result<bool> {
        let Value::Object(receiver_idx) = receiver else {
            return Ok(false);
        };
        let receiver_is_proxy = self
            .heap
            .with_obj(receiver_idx.0, |object| matches!(object, HeapObj::Proxy(_)));
        if receiver_is_proxy {
            let pin_count = self.pin_many(&[receiver.clone(), value.clone()]);
            let result = (|| {
                let descriptor = crate::builtins::own_property_descriptor_for_key_or_throw(
                    self, receiver, &pkey,
                )?;
                match descriptor {
                    Some(descriptor) => {
                        if descriptor.is_accessor || !descriptor.writable {
                            return Ok(false);
                        }
                        self.define_receiver_value_property(receiver, pkey.clone(), value.clone())
                    }
                    None => {
                        self.define_receiver_data_property(receiver, pkey.clone(), value.clone())
                    }
                }
            })();
            self.unpin_many(pin_count);
            return result;
        }
        if let Some(success) = self.define_typed_array_integer_index_property(
            receiver,
            &pkey,
            TypedArrayDefineDescriptor {
                value: Some(&value),
                has_configurable: false,
                configurable: false,
                has_enumerable: false,
                enumerable: true,
                is_accessor: false,
                has_writable: false,
                writable: true,
            },
        )? {
            return Ok(success);
        }
        let receiver_is_array_length = pkey.as_str() == Some("length")
            && self.heap.with_obj(receiver_idx.0, |object| {
                matches!(object, HeapObj::Array(array) if !array.is_arguments.load(std::sync::atomic::Ordering::Relaxed))
            });
        if receiver_is_array_length {
            let length_writable = self.heap.with_obj(receiver_idx.0, |object| {
                let HeapObj::Array(array) = object else {
                    return false;
                };
                array
                    .props
                    .lock()
                    .get(&crate::value::PropertyKey::from("length"))
                    .is_none_or(|descriptor| descriptor.writable)
            });
            if !length_writable {
                return Ok(false);
            }
            return self.try_set_array_length(receiver_idx.0, value);
        }
        let namespace_binding = self.heap.with_obj(receiver_idx.0, |object| {
            if let HeapObj::ModuleNamespace(namespace) = object {
                return pkey
                    .as_str()
                    .and_then(|name| namespace.exports.lock().get(name).cloned());
            }
            None
        });
        if let Some((env, name)) = namespace_binding {
            match crate::environment::get_checked(&self.heap, env, &name) {
                Ok(_) => return Ok(false),
                Err(true) => {
                    return Err(Error::reference(format!(
                        "Cannot access '{}' before initialization",
                        name
                    )))
                }
                Err(false) => return Ok(false),
            }
        }
        let existing = self
            .heap
            .with_obj(receiver_idx.0, |o| o.props().lock().get(&pkey).cloned());
        if let Some(desc) = existing {
            if desc.is_accessor || !desc.writable {
                return Ok(false);
            }
        } else {
            if let Some(name) = pkey.as_str() {
                let array_receiver = self.heap.with_obj(receiver_idx.0, |o| {
                    if let HeapObj::Array(a) = o {
                        let present = crate::value::parse_array_index(name)
                            .is_some_and(|i| a.is_dense_present(i));
                        let extensible = a.extensible.load(std::sync::atomic::Ordering::Relaxed);
                        return Some((present, extensible));
                    }
                    None
                });
                if let Some((present, extensible)) = array_receiver {
                    if let Some(index) = crate::value::parse_array_index(name) {
                        if !present && !extensible {
                            return Ok(false);
                        }
                        if self.array_index_blocked_by_non_writable_length(receiver_idx.0, index) {
                            return Ok(false);
                        }
                        self.set_arguments_mapped_binding_for_key(receiver_idx.0, &pkey, &value);
                        self.set_array_index(receiver_idx.0, index, value)?;
                        self.sync_array_length_descriptor_after_index(receiver_idx.0);
                        return Ok(true);
                    }
                }
            }
            let is_extensible = self.heap.with_obj(receiver_idx.0, |o| o.is_extensible());
            if !is_extensible {
                return Ok(false);
            }
        }
        let cache_key = pkey.as_str().map(|s| s.to_string());
        self.heap.with_obj(receiver_idx.0, |o| {
            let props = o.props();
            let mut props = props.lock();
            if let Some(existing) = props.get_mut(&pkey) {
                existing.value = value.clone();
            } else {
                props.insert(
                    pkey.clone(),
                    crate::value::PropertyDescriptor::data(value.clone()),
                );
            }
        });
        self.set_arguments_mapped_binding_for_key(receiver_idx.0, &pkey, &value);
        if let Some(key) = cache_key {
            self.ic_invalidate(receiver_idx.0, &key);
        }
        Ok(true)
    }

    fn define_receiver_data_property(
        &mut self,
        object: &Value,
        key: crate::value::PropertyKey,
        value: Value,
    ) -> error::Result<bool> {
        let descriptor = crate::value::PropertyDescriptor::data(value.clone());
        let proxy_descriptor = ProxyDefinePropertyDescriptor::complete(descriptor.clone());
        let object = match self.proxy_define_own_property(object, &key, &proxy_descriptor, None)? {
            ProxyDefinePropertyOutcome::Ordinary(object) => object,
            ProxyDefinePropertyOutcome::Complete(result) => return Ok(result),
        };
        let Value::Object(object_idx) = object else {
            return Ok(false);
        };
        let object = Value::Object(object_idx);

        if let Some(success) = self.define_typed_array_integer_index_property(
            &object,
            &key,
            TypedArrayDefineDescriptor {
                value: Some(&value),
                has_configurable: true,
                configurable: true,
                has_enumerable: true,
                enumerable: true,
                is_accessor: false,
                has_writable: true,
                writable: true,
            },
        )? {
            return Ok(success);
        }

        if let Some(index) = key.as_str().and_then(crate::value::parse_array_index) {
            let blocked_by_array_length = self.heap.with_obj(object_idx.0, |heap_object| {
                let HeapObj::Array(array) = heap_object else {
                    return false;
                };
                if array
                    .is_arguments
                    .load(std::sync::atomic::Ordering::Relaxed)
                    || self
                        .array_index_own_property_descriptor(object_idx.0, index, &key)
                        .is_some()
                {
                    return false;
                }
                let old_length = array
                    .items
                    .lock()
                    .len()
                    .max(array.sparse_max.lock().unwrap_or(0));
                let length_writable = array
                    .props
                    .lock()
                    .get(&crate::value::PropertyKey::from("length"))
                    .is_none_or(|descriptor| descriptor.writable);
                index >= old_length && !length_writable
            });
            if blocked_by_array_length {
                return Ok(false);
            }
        }

        let success = self.define_own_property(&object, key.clone(), descriptor)?;
        if success {
            self.set_arguments_mapped_binding_for_key(object_idx.0, &key, &value);
        }
        Ok(success)
    }

    fn define_receiver_value_property(
        &mut self,
        object: &Value,
        key: crate::value::PropertyKey,
        value: Value,
    ) -> error::Result<bool> {
        let proxy_descriptor = ProxyDefinePropertyDescriptor::value_only(value.clone());
        let object = match self.proxy_define_own_property(object, &key, &proxy_descriptor, None)? {
            ProxyDefinePropertyOutcome::Ordinary(object) => object,
            ProxyDefinePropertyOutcome::Complete(result) => return Ok(result),
        };
        let Value::Object(object_idx) = object else {
            return Ok(false);
        };
        let object = Value::Object(object_idx);

        if let Some(success) = self.define_typed_array_integer_index_property(
            &object,
            &key,
            TypedArrayDefineDescriptor {
                value: Some(&value),
                has_configurable: false,
                configurable: false,
                has_enumerable: false,
                enumerable: false,
                is_accessor: false,
                has_writable: false,
                writable: false,
            },
        )? {
            return Ok(success);
        }

        let object_is_array_length = key.as_str() == Some("length")
            && self.heap.with_obj(object_idx.0, |heap_object| {
                matches!(heap_object, HeapObj::Array(array) if !array.is_arguments.load(std::sync::atomic::Ordering::Relaxed))
            });
        if object_is_array_length {
            return self.try_set_array_length(object_idx.0, value);
        }

        let current =
            crate::builtins::own_property_descriptor_for_key_or_throw(self, &object, &key)?;
        let array_index_state = key
            .as_str()
            .and_then(crate::value::parse_array_index)
            .and_then(|index| {
                self.heap.with_obj(object_idx.0, |heap_object| {
                    let HeapObj::Array(array) = heap_object else {
                        return None;
                    };
                    if array
                        .is_arguments
                        .load(std::sync::atomic::Ordering::Relaxed)
                    {
                        return None;
                    }
                    let old_length = array
                        .items
                        .lock()
                        .len()
                        .max(array.sparse_max.lock().unwrap_or(0));
                    let length_writable = array
                        .props
                        .lock()
                        .get(&crate::value::PropertyKey::from("length"))
                        .is_none_or(|descriptor| descriptor.writable);
                    Some((index, old_length, length_writable))
                })
            });
        if current.is_none()
            && array_index_state.is_some_and(|(index, old_length, length_writable)| {
                index >= old_length && !length_writable
            })
        {
            return Ok(false);
        }
        let descriptor = match current {
            Some(current) if current.is_accessor => {
                if !current.configurable {
                    return Ok(false);
                }
                crate::value::PropertyDescriptor {
                    value: value.clone(),
                    writable: false,
                    enumerable: current.enumerable,
                    configurable: current.configurable,
                    get: None,
                    set: None,
                    is_accessor: false,
                }
            }
            Some(mut current) => {
                if !current.writable && !descriptor_same_value(&value, &current.value) {
                    return Ok(false);
                }
                current.value = value.clone();
                current
            }
            None => crate::value::PropertyDescriptor {
                value: value.clone(),
                writable: false,
                enumerable: false,
                configurable: false,
                get: None,
                set: None,
                is_accessor: false,
            },
        };
        let success = self.define_own_property(&object, key.clone(), descriptor)?;
        if success {
            self.set_arguments_mapped_binding_for_key(object_idx.0, &key, &value);
        }
        Ok(success)
    }

    /// Internal `[[GetPrototypeOf]]`, including Proxy `getPrototypeOf` traps
    /// and the non-extensible target invariant.
    pub(crate) fn get_prototype_of(&mut self, object: &Value) -> error::Result<Option<Value>> {
        let Value::Object(_) = object else {
            return Err(Error::type_err(
                "Object prototype target must be an object".to_string(),
            ));
        };

        enum Step {
            Forward(Value),
            Validate {
                target: Value,
                expected: Option<Value>,
            },
            Return(Option<Value>),
        }

        self.try_reserve_value_roots(std::slice::from_ref(object))?;
        let root_pin = self.pin(object);
        let mut current = object.clone();
        let mut expected_prototypes = Vec::new();
        let mut expected_pins = 0;
        let result = (|| {
            let result_proto = loop {
                let Value::Object(idx) = &current else {
                    return Err(Error::type_err(
                        "Proxy getPrototypeOf target must be an object",
                    ));
                };
                let proxy_info = self.heap.with_obj(idx.0, |heap_object| {
                    if let HeapObj::Proxy(proxy) = heap_object {
                        if *proxy.revoked.lock() {
                            return Some(Err(Error::type_err(
                                "Cannot perform 'getPrototypeOf' on a proxy that has been revoked",
                            )));
                        }
                        Some(Ok((proxy.target.clone(), proxy.handler.clone())))
                    } else {
                        None
                    }
                });
                let Some(proxy_info) = proxy_info else {
                    break self
                        .heap
                        .with_obj(idx.0, |heap_object| heap_object.proto().lock().clone());
                };

                let (target, handler) = proxy_info?;
                self.consume_fuel()?;
                let proxy_roots = [target.clone(), handler.clone()];
                self.try_reserve_value_roots(&proxy_roots)?;
                let proxy_pins = self.pin_many(&proxy_roots);
                let step = (|| {
                    let trap = self.get_proxy_method(&handler, "getPrototypeOf")?;
                    if trap.is_nullish() {
                        return Ok(Step::Forward(target.clone()));
                    }
                    if !crate::builtins::is_callable(&trap, &self.heap) {
                        return Err(Error::type_err("getPrototypeOf trap is not callable"));
                    }
                    self.try_reserve_value_roots(std::slice::from_ref(&trap))?;
                    let trap_pin = self.pin(&trap);
                    let handler_proto = self.call_function(
                        &trap,
                        std::slice::from_ref(&target),
                        Some(handler.clone()),
                    );
                    self.unpin(trap_pin);
                    let handler_proto = handler_proto?;
                    let proto = match handler_proto {
                        Value::Object(_) => Some(handler_proto),
                        Value::Null => None,
                        _ => {
                            return Err(Error::type_err(
                                "Proxy getPrototypeOf trap must return an object or null",
                            ))
                        }
                    };

                    // The trap may return the only reference to this object.
                    // Root it while nested [[IsExtensible]] runs observable JS.
                    if let Some(prototype) = &proto {
                        self.try_reserve_get_prototype_root(
                            prototype,
                            #[cfg(test)]
                            GetPrototypeReservationSite::ResultRoot,
                        )?;
                    }
                    let proto_pin = proto
                        .as_ref()
                        .map(|prototype| self.pin(prototype))
                        .unwrap_or(0);
                    let extensible = self.is_extensible(&target);
                    self.unpin(proto_pin);
                    if extensible? {
                        return Ok(Step::Return(proto));
                    }
                    Ok(Step::Validate {
                        target: target.clone(),
                        expected: proto,
                    })
                })();
                self.unpin_many(proxy_pins);

                match step? {
                    Step::Forward(target) => current = target,
                    Step::Validate { target, expected } => {
                        // Keep each deferred expected prototype alive until the
                        // innermost target result can be checked in reverse.
                        self.try_reserve_get_prototype_scratch(&mut expected_prototypes)?;
                        if let Some(prototype) = &expected {
                            self.try_reserve_get_prototype_root(
                                prototype,
                                #[cfg(test)]
                                GetPrototypeReservationSite::ExpectedRoot,
                            )?;
                        }
                        expected_pins += expected
                            .as_ref()
                            .map(|prototype| self.pin(prototype))
                            .unwrap_or(0);
                        expected_prototypes.push(expected);
                        current = target;
                    }
                    Step::Return(proto) => break proto,
                }
            };

            for expected in expected_prototypes.iter().rev() {
                if expected != &result_proto {
                    return Err(Error::type_err(
                        "Proxy getPrototypeOf trap returned incompatible prototype",
                    ));
                }
            }
            Ok(result_proto)
        })();
        self.unpin_many(expected_pins);
        self.unpin(root_pin);
        result
    }

    /// Internal `[[SetPrototypeOf]]`, including Proxy `setPrototypeOf` traps.
    /// Returns the spec boolean status instead of throwing; callers choose
    /// whether false is a silent failure, a returned boolean, or a TypeError.
    pub(crate) fn set_prototype_of(
        &mut self,
        object: &Value,
        proto: Option<Value>,
    ) -> error::Result<bool> {
        let Value::Object(_) = object else {
            return Err(Error::type_err(
                "Object prototype target must be an object".to_string(),
            ));
        };

        enum Step {
            Forward(Value),
            Return(bool),
        }

        let root_pin = self.pin(object)
            + proto
                .as_ref()
                .map(|prototype| self.pin(prototype))
                .unwrap_or(0);
        let mut current = object.clone();
        let result = (|| loop {
            let Value::Object(idx) = &current else {
                return Err(Error::type_err(
                    "Proxy setPrototypeOf target must be an object",
                ));
            };
            let proxy_info = self.heap.with_obj(idx.0, |heap_object| {
                if let HeapObj::Proxy(proxy) = heap_object {
                    if *proxy.revoked.lock() {
                        return Some(Err(Error::type_err(
                            "Cannot perform 'setPrototypeOf' on a proxy that has been revoked",
                        )));
                    }
                    Some(Ok((proxy.target.clone(), proxy.handler.clone())))
                } else {
                    None
                }
            });
            if let Some(proxy_info) = proxy_info {
                let (target, handler) = proxy_info?;
                self.consume_fuel()?;
                let proxy_pins = self.pin_many(&[target.clone(), handler.clone()]);
                let step = (|| {
                    let trap = self.get_proxy_method(&handler, "setPrototypeOf")?;
                    if trap.is_nullish() {
                        return Ok(Step::Forward(target.clone()));
                    }
                    if !crate::builtins::is_callable(&trap, &self.heap) {
                        return Err(Error::type_err("setPrototypeOf trap is not callable"));
                    }
                    let trap_pin = self.pin(&trap);
                    let trap_result = self.call_function(
                        &trap,
                        &[target.clone(), proto.clone().unwrap_or(Value::Null)],
                        Some(handler.clone()),
                    );
                    self.unpin(trap_pin);
                    let trap_result = trap_result?;
                    if !self.to_boolean(&trap_result) {
                        return Ok(Step::Return(false));
                    }
                    if self.is_extensible(&target)? {
                        return Ok(Step::Return(true));
                    }
                    let target_proto = self.get_prototype_of(&target)?;
                    if target_proto != proto {
                        return Err(Error::type_err(
                            "Proxy setPrototypeOf trap returned true for incompatible prototype",
                        ));
                    }
                    Ok(Step::Return(true))
                })();
                self.unpin_many(proxy_pins);
                match step? {
                    Step::Forward(target) => {
                        current = target;
                        continue;
                    }
                    Step::Return(result) => return Ok(result),
                }
            }

            let current_proto = self
                .heap
                .with_obj(idx.0, |heap_object| heap_object.proto().lock().clone());
            if current_proto == proto {
                return Ok(true);
            }
            if self.is_realm_object_prototype(*idx) {
                return Ok(false);
            }
            if !self
                .heap
                .with_obj(idx.0, |heap_object| heap_object.is_extensible())
            {
                return Ok(false);
            }
            if let Some(Value::Object(proto_idx)) = &proto {
                if self.prototype_chain_blocks_set(proto_idx.0, idx.0)? {
                    return Ok(false);
                }
            }

            self.heap.with_obj(idx.0, |heap_object| {
                *heap_object.proto().lock() = proto.clone();
            });
            return Ok(true);
        })();
        self.unpin_many(root_pin);
        result
    }

    fn is_realm_object_prototype(&self, object: GcIdx) -> bool {
        self.realm_object_prototype_ids.contains(&object)
            || self.object_proto == Value::Object(object)
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
        self.realm_global_property_is_non_writable_data(self.global, name)
    }

    pub(crate) fn realm_global_property_is_non_writable_data(
        &self,
        env: GcIdx,
        name: &str,
    ) -> bool {
        let Value::Object(idx) = self.realm_global_for_env(env) else {
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

    /// Scan the ordinary prototype chain used by OrdinarySetPrototypeOf.
    /// Proxies stop the scan because their `[[GetPrototypeOf]]` method is not
    /// the ordinary internal method. Brent checkpoints reject an impossible
    /// pre-existing ordinary cycle without a depth cap or growing native set.
    pub(crate) fn prototype_chain_blocks_set(
        &mut self,
        start: usize,
        target: usize,
    ) -> error::Result<bool> {
        let mut current = GcIdx(start);
        let mut checkpoint = current;
        let mut checkpoint_span = 0usize;
        let mut checkpoint_power = 1usize;

        loop {
            self.consume_fuel()?;
            if current.0 == target {
                return Ok(true);
            }
            let next = self.heap.with_obj(current.0, |heap_object| {
                if matches!(heap_object, HeapObj::Proxy(_)) {
                    return None;
                }
                match heap_object.proto().lock().clone() {
                    Some(Value::Object(next)) => Some(next),
                    _ => None,
                }
            });
            let Some(next) = next else {
                return Ok(false);
            };
            current = next;
            checkpoint_span = checkpoint_span.saturating_add(1);
            if current == checkpoint {
                return Ok(true);
            }
            if checkpoint_span == checkpoint_power {
                checkpoint = current;
                checkpoint_span = 0;
                checkpoint_power = checkpoint_power.saturating_mul(2);
            }
        }
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
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn define_array_length_property(
        &mut self,
        idx: usize,
        value: Option<Value>,
        has_writable: bool,
        writable: bool,
        has_enumerable: bool,
        enumerable: bool,
        has_configurable: bool,
        configurable: bool,
        is_accessor: bool,
    ) -> error::Result<bool> {
        let mut roots = vec![Value::Object(GcIdx(idx))];
        roots.extend(value.iter().cloned());
        let pin_count = self.pin_many(&roots);
        let result = (|| {
            let new_len = if let Some(value) = value.as_ref() {
                let new_len = crate::vm::to_uint32(self.to_number(value)?) as usize;
                let number_len = self.to_number(value)?;
                if new_len as f64 != number_len {
                    return Err(Error::range("Invalid array length"));
                }
                Some(new_len)
            } else {
                None
            };

            // ArraySetLength performs both observable conversions before it
            // reads and validates the current length descriptor.
            let (old_len, old_writable, dense_len, property_count) =
                self.heap.with_obj(idx, |object| {
                    let HeapObj::Array(array) = object else {
                        return (0, false, 0, 0);
                    };
                    let dense_len = array.items.lock().len();
                    let old_len = dense_len.max(array.sparse_max.lock().unwrap_or(0));
                    let props = array.props.lock();
                    let old_writable = props
                        .get(&crate::value::PropertyKey::from("length"))
                        .is_none_or(|descriptor| descriptor.writable);
                    (old_len, old_writable, dense_len, props.len())
                });
            if is_accessor
                || (has_configurable && configurable)
                || (has_enumerable && enumerable)
                || (has_writable && writable && !old_writable)
            {
                return Ok(false);
            }

            let Some(new_len) = new_len else {
                if has_writable {
                    self.heap.with_obj(idx, |object| {
                        if let HeapObj::Array(array) = object {
                            let mut props = array.props.lock();
                            let descriptor = props
                                .entry(crate::value::PropertyKey::from("length"))
                                .or_insert_with(|| {
                                    let mut descriptor = crate::value::PropertyDescriptor::data(
                                        Value::Number(old_len as f64),
                                    );
                                    descriptor.enumerable = false;
                                    descriptor.configurable = false;
                                    descriptor
                                });
                            descriptor.writable = writable;
                        }
                    });
                    self.ic_invalidate(idx, "length");
                }
                return Ok(true);
            };

            if !old_writable {
                return Ok(new_len == old_len);
            }

            let mut effective_len = new_len;
            let mut removable = Vec::new();
            if new_len < old_len {
                for _ in 0..property_count {
                    self.consume_fuel()?;
                }
                let indexed_properties = self.heap.with_obj(idx, |object| {
                    let HeapObj::Array(array) = object else {
                        return Vec::new();
                    };
                    array
                        .props
                        .lock()
                        .iter()
                        .filter_map(|(key, descriptor)| {
                            let index = key.as_str().and_then(crate::value::parse_array_index)?;
                            Some((key.clone(), index, descriptor.configurable))
                        })
                        .collect::<Vec<_>>()
                });
                for (_, index, configurable) in &indexed_properties {
                    if *index >= new_len && !configurable {
                        effective_len = effective_len.max(index + 1);
                    }
                }
                removable.extend(indexed_properties.into_iter().filter_map(
                    |(key, index, configurable)| {
                        (index >= effective_len && configurable).then_some(key)
                    },
                ));
            }
            let dense_target = if effective_len <= crate::value::MAX_DENSE_ARRAY_LEN {
                effective_len
            } else {
                dense_len.min(crate::value::MAX_DENSE_ARRAY_LEN)
            };
            for _ in 0..dense_len.abs_diff(dense_target) {
                self.consume_fuel()?;
            }

            let delete_succeeded = self.heap.with_obj(idx, |object| {
                let HeapObj::Array(array) = object else {
                    return false;
                };
                let cap = crate::value::MAX_DENSE_ARRAY_LEN;
                let mut items = array.items.lock();
                let mut present = array.present.lock();
                if new_len < old_len {
                    // Delete in descending-index effect: the highest
                    // non-configurable index is the first failure, so lower
                    // indices must remain and length rolls back above it.
                    let mut props = array.props.lock();
                    for key in &removable {
                        props.shift_remove(key);
                    }
                }

                if effective_len <= cap {
                    if effective_len < items.len() {
                        items.truncate(effective_len);
                        present.truncate(effective_len);
                    } else {
                        while items.len() < effective_len {
                            items.push(Value::Undefined);
                            present.push(false);
                        }
                    }
                    drop(present);
                    drop(items);
                    *array.sparse_max.lock() = None;
                } else {
                    if items.len() > cap {
                        items.truncate(cap);
                        present.truncate(cap);
                    }
                    drop(present);
                    drop(items);
                    *array.sparse_max.lock() = Some(effective_len);
                }

                let mut props = array.props.lock();
                let length = props
                    .entry(crate::value::PropertyKey::from("length"))
                    .or_insert_with(|| {
                        let mut descriptor = crate::value::PropertyDescriptor::data(Value::Number(
                            effective_len as f64,
                        ));
                        descriptor.enumerable = false;
                        descriptor.configurable = false;
                        descriptor
                    });
                length.value = Value::Number(effective_len as f64);
                if has_writable && !writable {
                    length.writable = false;
                }
                effective_len == new_len
            });
            self.ic_invalidate(idx, "length");
            Ok(delete_succeeded)
        })();
        self.unpin_many(pin_count);
        result
    }

    fn try_set_array_length(&mut self, idx: usize, value: Value) -> error::Result<bool> {
        self.define_array_length_property(
            idx,
            Some(value),
            false,
            false,
            false,
            false,
            false,
            false,
            false,
        )
    }

    pub(crate) fn set_array_length(&mut self, idx: usize, value: Value) -> error::Result<()> {
        let success = self.try_set_array_length(idx, value)?;
        if !success && self.current_strict() {
            return Err(Error::type_err("Cannot assign to read only array length"));
        }
        Ok(())
    }

    // ---- GC roots ----
    pub fn collect_roots(&self) -> Vec<usize> {
        let mut roots = vec![self.global.0];
        Self::push_value_roots(&mut roots, &self.global_this);
        if let Some(v) = &self.pending_new_target {
            Self::push_value_roots(&mut roots, v);
        }
        if let Some(prototype) = &self.pending_new_target_prototype {
            match prototype {
                NewTargetPrototype::Observed(value) => Self::push_value_roots(&mut roots, value),
                NewTargetPrototype::FallbackRealm(realm) => roots.push(realm.0),
            }
        }
        for context in &self.execution_contexts {
            roots.push(context.realm_env.0);
            match &context.kind {
                ExecutionContextKind::Interpreted { callee } => {
                    Self::push_value_roots(&mut roots, callee);
                }
                ExecutionContextKind::Native {
                    callee,
                    new_target,
                    new_target_prototype,
                } => {
                    Self::push_value_roots(&mut roots, callee);
                    if let Some(value) = new_target {
                        Self::push_value_roots(&mut roots, value);
                    }
                    if let Some(prototype) = new_target_prototype {
                        match prototype {
                            NewTargetPrototype::Observed(value) => {
                                Self::push_value_roots(&mut roots, value);
                            }
                            NewTargetPrototype::FallbackRealm(realm) => roots.push(realm.0),
                        }
                    }
                }
            }
        }
        for v in &self.stack {
            Self::push_value_roots(&mut roots, v);
        }
        for f in &self.frames {
            Self::push_value_roots(&mut roots, &f.callee);
            roots.push(f.env.0);
            Self::push_value_roots(&mut roots, &f.this_val);
            if let Some(meta) = f.chunk.import_meta {
                roots.push(meta.0);
            }
            for l in &f.locals {
                Self::push_value_roots(&mut roots, l);
            }
            // Per-frame generator run-state can hold live heap values
            // (resume value sent via next(obj), and the yielded value before
            // it is moved into the LazyGenerator). Root them so a GC during
            // resume_generator does not collect them.
            Self::push_value_roots(&mut roots, &f.gen_resume_value.lock());
            if let Some(kind) = f.gen_delegate_resume.lock().clone() {
                let value = match kind {
                    ResumeKind::Next(value)
                    | ResumeKind::Throw(value)
                    | ResumeKind::Return(value)
                    | ResumeKind::DelegateThrow(value) => value,
                    ResumeKind::DelegateResult { value, .. } => value,
                    ResumeKind::DelegateMissingThrow => Value::Undefined,
                };
                Self::push_value_roots(&mut roots, &value);
            }
            // gen_yield is Mutex<Option<Value>>; peek without consuming via take+set.
            let y = f.gen_yield.lock().take();
            if let Some(v) = &y {
                Self::push_value_roots(&mut roots, v);
            }
            *f.gen_yield.lock() = y;
            if let Some(value) = &f.async_await_value {
                Self::push_value_roots(&mut roots, value);
            }
        }
        for proto in [
            &self.object_proto,
            &self.array_proto,
            &self.array_to_string_fn,
            &self.function_proto,
            &self.string_proto,
            &self.number_proto,
            &self.bigint_proto,
            &self.boolean_proto,
            &self.error_proto,
            &self.symbol_proto,
            &self.array_buffer_proto,
            &self.promise_ctor,
            &self.promise_proto,
            &self.iterator_base_proto,
            &self.iterator_proto,
            &self.string_iterator_proto,
            &self.map_iterator_proto,
            &self.set_iterator_proto,
            &self.regexp_string_iterator_proto,
            &self.map_proto,
            &self.set_proto,
            &self.generator_proto,
            &self.generator_function_proto,
            &self.async_iterator_proto,
            &self.async_generator_proto,
            &self.async_generator_function_proto,
        ] {
            Self::push_value_roots(&mut roots, proto);
        }
        // Pending microtasks hold live heap values (Promise handlers, resolve/
        // reject reasons). Root them so a GC between scheduling and running a
        // microtask does not collect them.
        for mt in &self.microtask_queue {
            match mt {
                Microtask::Then {
                    promise,
                    on_fulfilled,
                    on_rejected,
                    derived,
                    continuation,
                    realm,
                    ..
                } => {
                    roots.push(promise.0);
                    Self::push_value_roots(&mut roots, on_fulfilled);
                    Self::push_value_roots(&mut roots, on_rejected);
                    if let Some(realm) = realm {
                        roots.push(realm.0);
                    }
                    if let Some(capability) = derived {
                        Self::push_value_roots(&mut roots, &capability.promise);
                        Self::push_value_roots(&mut roots, &capability.resolve);
                        Self::push_value_roots(&mut roots, &capability.reject);
                    }
                    if let Some(continuation) = continuation {
                        match continuation {
                            crate::value::PromiseContinuation::DynamicImport {
                                capability,
                                realm,
                                ..
                            } => {
                                Self::push_value_roots(&mut roots, &capability.promise);
                                Self::push_value_roots(&mut roots, &capability.resolve);
                                Self::push_value_roots(&mut roots, &capability.reject);
                                roots.push(realm.0);
                            }
                            crate::value::PromiseContinuation::AsyncGenerator {
                                generator, ..
                            } => roots.push(generator.0),
                            crate::value::PromiseContinuation::AsyncFromSyncIterator {
                                capability,
                                iterator,
                                realm,
                                ..
                            } => {
                                Self::push_value_roots(&mut roots, &capability.promise);
                                Self::push_value_roots(&mut roots, &capability.resolve);
                                Self::push_value_roots(&mut roots, &capability.reject);
                                if let Some(iterator) = iterator {
                                    Self::push_value_roots(&mut roots, iterator);
                                }
                                roots.push(realm.0);
                            }
                            crate::value::PromiseContinuation::ArrayFromAsync(frame) => {
                                Self::push_value_roots(&mut roots, &frame.capability.promise);
                                Self::push_value_roots(&mut roots, &frame.capability.resolve);
                                Self::push_value_roots(&mut roots, &frame.capability.reject);
                                roots.push(frame.realm.0);
                                for value in [
                                    &frame.target,
                                    &frame.source,
                                    &frame.iterator,
                                    &frame.next_method,
                                    &frame.mapper,
                                    &frame.this_arg,
                                ] {
                                    Self::push_value_roots(&mut roots, value);
                                }
                                if let crate::value::ArrayFromAsyncAwaitKind::IteratorClose {
                                    original_reason,
                                } = &frame.await_kind
                                {
                                    Self::push_value_roots(&mut roots, original_reason);
                                }
                            }
                            crate::value::PromiseContinuation::AsyncFunction(frame) => {
                                Self::push_value_roots(&mut roots, &frame.capability.promise);
                                Self::push_value_roots(&mut roots, &frame.capability.resolve);
                                Self::push_value_roots(&mut roots, &frame.capability.reject);
                                Self::push_value_roots(&mut roots, &frame.callee);
                                Self::push_value_roots(&mut roots, &frame.this_val);
                                Self::push_value_roots(&mut roots, &frame.new_target);
                                Self::push_value_roots(&mut roots, &frame.finally_completion_val);
                                roots.push(frame.env.0);
                                roots.extend(frame.catch_stack.iter().map(|(_, _, env, _)| env.0));
                                for value in frame.stack.iter().chain(frame.locals.iter()) {
                                    Self::push_value_roots(&mut roots, value);
                                }
                            }
                        }
                    }
                }
                Microtask::Thenable {
                    thenable,
                    then,
                    resolve,
                    reject,
                    realm,
                } => {
                    Self::push_value_roots(&mut roots, thenable);
                    Self::push_value_roots(&mut roots, then);
                    Self::push_value_roots(&mut roots, resolve);
                    Self::push_value_roots(&mut roots, reject);
                    roots.push(realm.0);
                }
                Microtask::Resolve { promise, value } => {
                    roots.push(promise.0);
                    Self::push_value_roots(&mut roots, value);
                }
                Microtask::Reject { promise, reason } => {
                    roots.push(promise.0);
                    Self::push_value_roots(&mut roots, reason);
                }
                Microtask::ResolveInRealm {
                    promise,
                    value,
                    realm,
                } => {
                    roots.push(promise.0);
                    Self::push_value_roots(&mut roots, value);
                    roots.push(realm.0);
                }
                Microtask::RejectInRealm {
                    promise,
                    reason,
                    realm,
                } => {
                    roots.push(promise.0);
                    Self::push_value_roots(&mut roots, reason);
                    roots.push(realm.0);
                }
                Microtask::PromiseResolveAfterThen {
                    promise,
                    resolution,
                    then,
                    realm,
                } => {
                    roots.push(promise.0);
                    Self::push_value_roots(&mut roots, resolution);
                    Self::push_value_roots(&mut roots, then);
                    roots.push(realm.0);
                }
                Microtask::AsyncGeneratorDrain { generator } => roots.push(generator.0),
                Microtask::DynamicImport {
                    promise,
                    resolve,
                    reject,
                    realm,
                    ..
                } => {
                    roots.push(promise.0);
                    Self::push_value_roots(&mut roots, resolve);
                    Self::push_value_roots(&mut roots, reject);
                    roots.push(realm.0);
                }
                Microtask::FinalizationCleanup { registry } => roots.push(registry.0),
            }
        }
        // Global constants are reachable for the program lifetime.
        for v in &self.global_constants {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_globals.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_object_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_array_constructors.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_array_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_array_values_functions.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_promise_constructors.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_promise_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_generator_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_generator_function_constructors.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_generator_function_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_async_iterator_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_async_generator_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_async_generator_function_constructors.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_async_generator_function_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_primitive_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_date_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_eval_functions.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_throw_type_errors.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_function_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_async_function_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_iterator_constructors.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_iterator_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_array_iterator_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_wrap_for_valid_iterator_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_string_iterator_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_iterator_helper_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_error_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_heap_limit_errors.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_regexp_constructors.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_regexp_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_regexp_string_iterator_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_array_buffer_prototypes.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for v in self.realm_typed_array_constructors.values() {
            Self::push_value_roots(&mut roots, v);
        }
        for module in self.module_records.values() {
            roots.push(module.env.0);
            if let Some(meta) = module.import_meta() {
                roots.push(meta.0);
            }
            if let Some(promise) = module.evaluation_promise() {
                roots.push(promise.0);
            }
            if let Some(error) = module.error() {
                if let Some(value) = &error.thrown_value {
                    Self::push_value_roots(&mut roots, value);
                }
            }
            if let Some(value) = module.completion_value() {
                Self::push_value_roots(&mut roots, &value);
            }
        }
        for function in &self.functions {
            if let Some(meta) = function.chunk.import_meta {
                roots.push(meta.0);
            }
        }
        // Pinned temporary roots (e.g. Promise handlers held across call_function).
        roots.extend_from_slice(&self.gc_pins);
        roots.extend_from_slice(&self.kept_objects);
        {
            let external = self.external_jobs.lock();
            for value in external.wait_roots.values() {
                Self::push_value_roots(&mut roots, value);
            }
            for job in &external.jobs {
                Self::push_value_roots(&mut roots, &job.resolve);
                Self::push_value_roots(&mut roots, &job.value);
            }
        }
        roots
    }

    pub(crate) fn keep_during_job(&mut self, value: &Value) {
        if let Value::Object(idx) = value {
            if !self.kept_objects.contains(&idx.0) {
                self.kept_objects.push(idx.0);
            }
        }
    }

    pub(crate) fn clear_kept_objects(&mut self) {
        self.kept_objects.clear();
    }

    pub(crate) fn schedule_finalization_cleanup_jobs(&mut self) {
        for registry in self.heap.take_pending_finalization_registries() {
            self.microtask_queue
                .push_back(Microtask::FinalizationCleanup {
                    registry: GcIdx(registry),
                });
        }
    }

    pub fn gc(&mut self) {
        let roots = self.collect_roots();
        self.heap.collect(&roots);
        self.ic.clear();
        self.schedule_finalization_cleanup_jobs();
    }

    /// Pin a heap object as a temporary GC root. Returns the number of roots
    /// pushed so callers can pass it to `unpin`/`unpin_many`.
    pub fn pin(&mut self, v: &Value) -> usize {
        let before = self.gc_pins.len();
        Self::push_value_roots(&mut self.gc_pins, v);
        self.gc_pins.len() - before
    }

    /// Release the temporary root pinned at `token`.
    pub fn unpin(&mut self, token: usize) {
        self.unpin_many(token);
    }

    /// Pin multiple values at once; returns the count to unpin later.
    pub fn pin_many(&mut self, vals: &[Value]) -> usize {
        let before = self.gc_pins.len();
        for v in vals {
            Self::push_value_roots(&mut self.gc_pins, v);
        }
        self.gc_pins.len() - before
    }

    /// Release `n` most-recently pinned temporary roots.
    pub fn unpin_many(&mut self, n: usize) {
        for _ in 0..n {
            self.gc_pins.pop();
        }
    }

    fn settle_promise(
        &mut self,
        promise_idx: usize,
        result: Value,
        status: PromiseStatus,
        fallback_realm: GcIdx,
    ) -> error::Result<()> {
        let callbacks = self.heap.with_obj(promise_idx, |object| {
            let HeapObj::Promise(promise) = object else {
                return None;
            };
            if *promise.state.lock() != PromiseStatus::Pending {
                return None;
            }
            let handlers = promise.handlers.lock();
            Some(
                handlers
                    .iter()
                    .map(|handler| {
                        if status == PromiseStatus::Fulfilled {
                            handler.on_fulfilled.clone()
                        } else {
                            handler.on_rejected.clone()
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        });
        let Some(callbacks) = callbacks else {
            return Ok(());
        };

        // Realm discovery can traverse arbitrarily deep wrapper chains. Finish
        // every fallible traversal before making Promise settlement irreversible.
        let mut realms = Vec::with_capacity(callbacks.len());
        for callback in &callbacks {
            realms.push(self.promise_reaction_job_realm_with_fallback(callback, fallback_realm)?);
        }

        let handlers: Vec<crate::value::PromiseHandler> =
            self.heap.with_obj(promise_idx, |object| {
                let HeapObj::Promise(promise) = object else {
                    return Vec::new();
                };
                if *promise.state.lock() != PromiseStatus::Pending {
                    return Vec::new();
                }
                *promise.state.lock() = status;
                *promise.result.lock() = result;
                promise.handlers.lock().drain(..).collect()
            });
        debug_assert_eq!(handlers.len(), realms.len());
        for (handler, realm) in handlers.into_iter().zip(realms) {
            self.microtask_queue.push_back(Microtask::Then {
                promise: GcIdx(promise_idx),
                on_fulfilled: handler.on_fulfilled,
                on_rejected: handler.on_rejected,
                derived: handler.derived,
                continuation: handler.continuation,
                realm,
            });
        }
        Ok(())
    }

    /// Resolve a Promise and schedule its handlers.
    ///
    /// A non-catchable handler-Realm abort is returned before state changes, so
    /// embedders may refill fuel and retry the same settlement.
    pub fn promise_resolve(&mut self, promise_idx: usize, value: Value) -> error::Result<()> {
        let realm = self.current_realm_global_env();
        self.promise_resolve_in_realm(promise_idx, value, realm)
    }

    /// Reject a Promise and schedule its handlers with the same transactional
    /// host-abort behavior as [`Vm::promise_resolve`].
    pub fn promise_reject(&mut self, promise_idx: usize, reason: Value) -> error::Result<()> {
        let realm = self.current_realm_global_env();
        self.promise_reject_in_realm(promise_idx, reason, realm)
    }

    pub(crate) fn promise_resolve_in_realm(
        &mut self,
        promise_idx: usize,
        value: Value,
        realm: GcIdx,
    ) -> error::Result<()> {
        self.settle_promise(promise_idx, value, PromiseStatus::Fulfilled, realm)
    }

    pub(crate) fn promise_reject_in_realm(
        &mut self,
        promise_idx: usize,
        reason: Value,
        realm: GcIdx,
    ) -> error::Result<()> {
        self.settle_promise(promise_idx, reason, PromiseStatus::Rejected, realm)
    }

    fn run_external_promise_job(&mut self, job: ExternalPromiseJob) -> error::Result<()> {
        self.call_function(
            &job.resolve,
            std::slice::from_ref(&job.value),
            Some(Value::Undefined),
        )
        .map(|_| ())
    }

    fn has_staged_promise_settlement(&self) -> bool {
        matches!(
            self.microtask_queue.front(),
            Some(
                Microtask::Resolve { .. }
                    | Microtask::Reject { .. }
                    | Microtask::ResolveInRealm { .. }
                    | Microtask::RejectInRealm { .. }
                    | Microtask::PromiseResolveAfterThen { .. }
            )
        )
    }

    fn retryable_settlement_task(task: &Microtask) -> Option<Microtask> {
        match task {
            Microtask::Resolve { promise, value } => Some(Microtask::Resolve {
                promise: *promise,
                value: value.clone(),
            }),
            Microtask::Reject { promise, reason } => Some(Microtask::Reject {
                promise: *promise,
                reason: reason.clone(),
            }),
            Microtask::ResolveInRealm {
                promise,
                value,
                realm,
            } => Some(Microtask::ResolveInRealm {
                promise: *promise,
                value: value.clone(),
                realm: *realm,
            }),
            Microtask::RejectInRealm {
                promise,
                reason,
                realm,
            } => Some(Microtask::RejectInRealm {
                promise: *promise,
                reason: reason.clone(),
                realm: *realm,
            }),
            _ => None,
        }
    }

    fn run_queued_microtask(&mut self, task: Microtask) -> error::Result<()> {
        let (result, retry) = match task {
            Microtask::PromiseResolveAfterThen {
                promise,
                resolution,
                then,
                realm,
            } => {
                let result = crate::builtins::collections::continue_promise_resolution_after_then(
                    self, promise.0, resolution, then, realm,
                );
                let result = match result {
                    Ok(()) => Ok(()),
                    Err(abort) => {
                        self.microtask_queue.push_front(abort.continuation);
                        Err(abort.error)
                    }
                };
                (result, None)
            }
            task => {
                let retry = Self::retryable_settlement_task(&task);
                (self.run_microtask(task), retry)
            }
        };
        self.clear_kept_objects();
        if result.as_ref().is_err_and(|error| !error.catchable()) {
            if let Some(task) = retry {
                self.microtask_queue.push_front(task);
            }
        }
        result
    }

    /// Drain the microtask queue, running scheduled then/catch callbacks.
    pub fn run_microtasks(&mut self) -> error::Result<()> {
        // Drain in enqueue order (FIFO): Promise microtasks must fire in the
        // order they were scheduled, so pop from the front. (Vec::remove(0) is
        // O(n), but microtask queues are typically small per drain cycle.)
        loop {
            loop {
                if self.has_staged_promise_settlement() {
                    break;
                }
                let job = {
                    let mut external = self.external_jobs.lock();
                    external.jobs.pop_front()
                };
                let Some(job) = job else {
                    break;
                };
                self.run_external_promise_job(job)?;
            }
            let Some(task) = self.microtask_queue.pop_front() else {
                if self.external_jobs.lock().jobs.is_empty() {
                    break;
                }
                continue;
            };
            self.run_queued_microtask(task)?;
        }
        Ok(())
    }

    /// Execute a single microtask from the queue, if any. Returns true if
    /// a task was executed, false if the queue is empty. This allows hosts
    /// (e.g. WASM, server runtimes) to cooperatively interleave JS microtask
    /// execution with other work, rather than draining all microtasks at once.
    pub fn tick(&mut self) -> error::Result<bool> {
        let mut ran_external = false;
        let external_job = if self.has_staged_promise_settlement() {
            None
        } else {
            let mut external = self.external_jobs.lock();
            external.jobs.pop_front()
        };
        if let Some(job) = external_job {
            self.run_external_promise_job(job)?;
            ran_external = true;
        }
        if let Some(task) = self.microtask_queue.pop_front() {
            self.run_queued_microtask(task)?;
            Ok(true)
        } else {
            Ok(ran_external)
        }
    }

    fn run_dynamic_import_reaction(
        &mut self,
        evaluation_promise: GcIdx,
        target: &std::path::Path,
        capability: crate::value::PromiseReactionCapability,
        realm: GcIdx,
    ) -> error::Result<()> {
        let (state, result) = self.heap.with_obj(evaluation_promise.0, |object| {
            if let HeapObj::Promise(data) = object {
                (*data.state.lock(), data.result.lock().clone())
            } else {
                (PromiseStatus::Rejected, Value::Undefined)
            }
        });
        let pins = self.pin_many(&[
            capability.promise.clone(),
            capability.resolve.clone(),
            capability.reject.clone(),
            result.clone(),
        ]);
        let settlement = if state == PromiseStatus::Rejected {
            self.call_function(
                &capability.reject,
                std::slice::from_ref(&result),
                Some(Value::Undefined),
            )
        } else {
            match self.finish_dynamic_import(target) {
                Ok(namespace) => self.call_function(
                    &capability.resolve,
                    std::slice::from_ref(&namespace),
                    Some(Value::Undefined),
                ),
                Err(error) => match self.promise_rejection_reason_in_realm(&error, realm) {
                    Ok(reason) => self.call_function(
                        &capability.reject,
                        std::slice::from_ref(&reason),
                        Some(Value::Undefined),
                    ),
                    Err(error) => Err(error),
                },
            }
        };
        self.unpin_many(pins);
        settlement.map(|_| ())
    }

    fn run_microtask(&mut self, task: Microtask) -> error::Result<()> {
        match task {
            Microtask::Then {
                promise,
                on_fulfilled,
                on_rejected,
                derived,
                continuation,
                realm,
            } => match continuation {
                Some(crate::value::PromiseContinuation::AsyncGenerator { generator, kind }) => {
                    crate::builtins::regexp::run_async_generator_reaction(
                        self, generator, kind, promise,
                    )
                }
                Some(crate::value::PromiseContinuation::AsyncFromSyncIterator {
                    capability,
                    done,
                    iterator,
                    close_on_rejection,
                    realm,
                }) => self.run_async_from_sync_iterator_reaction(
                    capability,
                    done,
                    iterator,
                    close_on_rejection,
                    realm,
                    promise,
                ),
                Some(crate::value::PromiseContinuation::ArrayFromAsync(frame)) => {
                    crate::builtins::array::run_array_from_async_reaction(self, *frame, promise)
                }
                Some(crate::value::PromiseContinuation::AsyncFunction(frame)) => {
                    self.run_async_function_reaction(*frame, promise)
                }
                Some(crate::value::PromiseContinuation::DynamicImport {
                    target,
                    capability,
                    realm,
                }) => self.run_dynamic_import_reaction(promise, &target, capability, realm),
                None => self.run_then(promise, on_fulfilled, on_rejected, derived, realm),
            },
            Microtask::Thenable {
                thenable,
                then,
                resolve,
                reject,
                realm,
            } => self.run_thenable_job(thenable, then, resolve, reject, realm),
            Microtask::Resolve { promise, value } => self.promise_resolve(promise.0, value),
            Microtask::Reject { promise, reason } => self.promise_reject(promise.0, reason),
            Microtask::ResolveInRealm {
                promise,
                value,
                realm,
            } => self.promise_resolve_in_realm(promise.0, value, realm),
            Microtask::RejectInRealm {
                promise,
                reason,
                realm,
            } => self.promise_reject_in_realm(promise.0, reason, realm),
            Microtask::PromiseResolveAfterThen { .. } => {
                unreachable!("post-then resolution is handled by run_queued_microtask")
            }
            Microtask::AsyncGeneratorDrain { generator } => {
                crate::builtins::regexp::drain_async_generator_queue(self, generator)
            }
            Microtask::DynamicImport {
                promise,
                resolve,
                reject,
                realm,
                referrer,
                specifier,
                import_type,
            } => {
                let capability_pins =
                    self.pin_many(&[Value::Object(promise), resolve.clone(), reject.clone()]);
                let outcome =
                    self.dynamic_import_module(&referrer, &specifier, import_type.as_deref());
                let settlement = match outcome {
                    Ok(crate::module::DynamicImportResult::Ready(namespace)) => {
                        let value_pin = self.pin(&namespace);
                        let result = self.call_function(
                            &resolve,
                            std::slice::from_ref(&namespace),
                            Some(Value::Undefined),
                        );
                        self.unpin(value_pin);
                        result
                    }
                    Ok(crate::module::DynamicImportResult::Pending {
                        target,
                        evaluation_promise,
                    }) => {
                        let capability = crate::value::PromiseReactionCapability {
                            promise: Value::Object(promise),
                            resolve: resolve.clone(),
                            reject: reject.clone(),
                        };
                        let continuation = Some(crate::value::PromiseContinuation::DynamicImport {
                            target,
                            capability,
                            realm,
                        });
                        let state = self.heap.with_obj(evaluation_promise.0, |object| {
                            if let HeapObj::Promise(data) = object {
                                *data.state.lock()
                            } else {
                                PromiseStatus::Rejected
                            }
                        });
                        if state == PromiseStatus::Pending {
                            self.heap.with_obj(evaluation_promise.0, |object| {
                                if let HeapObj::Promise(data) = object {
                                    data.handlers.lock().push(crate::value::PromiseHandler {
                                        on_fulfilled: Value::Undefined,
                                        on_rejected: Value::Undefined,
                                        derived: None,
                                        continuation,
                                    });
                                }
                            });
                        } else {
                            self.microtask_queue.push_back(Microtask::Then {
                                promise: evaluation_promise,
                                on_fulfilled: Value::Undefined,
                                on_rejected: Value::Undefined,
                                derived: None,
                                continuation,
                                realm: None,
                            });
                        }
                        Ok(Value::Undefined)
                    }
                    Err(error) => match self.promise_rejection_reason_in_realm(&error, realm) {
                        Ok(reason) => {
                            let reason_pin = self.pin(&reason);
                            let result = self.call_function(
                                &reject,
                                std::slice::from_ref(&reason),
                                Some(Value::Undefined),
                            );
                            self.unpin(reason_pin);
                            result
                        }
                        Err(error) => Err(error),
                    },
                };
                self.unpin_many(capability_pins);
                settlement.map(|_| ())
            }
            Microtask::FinalizationCleanup { registry } => {
                crate::builtins::run_finalization_registry_cleanup_job(self, registry)?;
                self.schedule_finalization_cleanup_jobs();
                Ok(())
            }
        }
    }

    /// Returns true if there are pending microtasks in the queue.
    pub fn has_pending_microtasks(&self) -> bool {
        !self.microtask_queue.is_empty() || !self.external_jobs.lock().jobs.is_empty()
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
