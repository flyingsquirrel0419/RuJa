use super::*;

// Map
// =========================================================================
fn require_map_receiver(vm: &Vm, this: Option<Value>, name: &str) -> error::Result<GcIdx> {
    let Some(Value::Object(idx)) = this else {
        return Err(Error::type_err(format!("{name} called on non-Map")));
    };
    if vm
        .heap
        .with_obj_read(idx.0, |obj| matches!(obj, HeapObj::Map(_)))
    {
        Ok(idx)
    } else {
        Err(Error::type_err(format!("{name} called on non-Map")))
    }
}

pub(crate) fn new_collection_iterator(
    vm: &mut Vm,
    source: Value,
    kind: CollectionIteratorKind,
) -> error::Result<Value> {
    let realm = vm
        .native_callee_closure()
        .map(|closure| env::global_env_root(&vm.heap, closure))
        .unwrap_or(vm.global);
    let proto = match kind {
        CollectionIteratorKind::StringValues
            if matches!(vm.string_iterator_proto, Value::Object(_)) =>
        {
            vm.string_iterator_proto.clone()
        }
        CollectionIteratorKind::ArrayEntries
        | CollectionIteratorKind::ArrayKeys
        | CollectionIteratorKind::ArrayValues
            if vm.realm_array_iterator_prototypes.contains_key(&realm.0) =>
        {
            vm.realm_array_iterator_prototypes[&realm.0].clone()
        }
        CollectionIteratorKind::MapEntries
        | CollectionIteratorKind::MapKeys
        | CollectionIteratorKind::MapValues
            if vm.realm_map_iterator_prototypes.contains_key(&realm.0) =>
        {
            vm.realm_map_iterator_prototypes[&realm.0].clone()
        }
        CollectionIteratorKind::SetEntries | CollectionIteratorKind::SetValues
            if vm.realm_set_iterator_prototypes.contains_key(&realm.0) =>
        {
            vm.realm_set_iterator_prototypes[&realm.0].clone()
        }
        _ => vm.object_proto.clone(),
    };
    let pin_count = vm.pin(&source) + vm.pin(&proto);
    let allocation = vm.alloc(HeapObj::CollectionIterator(CollectionIteratorData {
        source: Mutex::new(source),
        next_method: Mutex::new(None),
        kind,
        index: Mutex::new(0),
        set_physical_index: Mutex::new(0),
        set_compaction_epoch: Mutex::new(u64::MAX),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(proto)),
        extensible: std::sync::atomic::AtomicBool::new(true),
    }));
    vm.unpin_many(pin_count);
    let iterator = Value::Object(allocation?);
    vm.keep_during_job(&iterator)?;
    Ok(iterator)
}

fn active_string_iterator_realm(vm: &Vm) -> GcIdx {
    vm.native_callee_closure()
        .map(|closure| env::global_env_root(&vm.heap, closure))
        .unwrap_or(vm.global)
}

fn string_iterator_method(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    if receiver.is_nullish() {
        return Err(Error::type_err(
            "String.prototype[Symbol.iterator] requires an object-coercible receiver",
        ));
    }
    let string = vm.to_string(&receiver)?;
    let realm = active_string_iterator_realm(vm);
    let proto = vm
        .realm_string_iterator_prototypes
        .get(&realm.0)
        .cloned()
        .ok_or_else(|| Error::internal("missing String Iterator prototype intrinsic"))?;
    let source = Value::String(string);
    let pin_count = vm.pin(&source) + vm.pin(&proto);
    let allocation = vm.alloc(HeapObj::CollectionIterator(CollectionIteratorData {
        source: Mutex::new(source),
        next_method: Mutex::new(None),
        kind: CollectionIteratorKind::StringValues,
        index: Mutex::new(0),
        set_physical_index: Mutex::new(0),
        set_compaction_epoch: Mutex::new(u64::MAX),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(proto)),
        extensible: std::sync::atomic::AtomicBool::new(true),
    }));
    vm.unpin_many(pin_count);
    let iterator = Value::Object(allocation?);
    vm.keep_during_job(&iterator)?;
    Ok(iterator)
}

fn string_iterator_next(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let pin_count = vm.pin(&receiver);
    let result = (|| {
        let Value::Object(iter_idx) = receiver else {
            return Err(Error::type_err(
                "String Iterator next called on incompatible receiver",
            ));
        };
        let Some((source, raw_index)) = vm.heap.with_obj(iter_idx.0, |obj| {
            let HeapObj::CollectionIterator(iter) = obj else {
                return None;
            };
            if iter.kind != CollectionIteratorKind::StringValues {
                return None;
            }
            Some((iter.source.lock().clone(), *iter.index.lock()))
        }) else {
            return Err(Error::type_err(
                "String Iterator next called on incompatible receiver",
            ));
        };
        let Value::String(string) = source else {
            if source.is_undefined() {
                return gen_result(vm, Value::Undefined, true, false);
            }
            return Err(Error::type_err(
                "String Iterator next called on incompatible receiver",
            ));
        };

        let len = crate::value::utf16_len(&string) as u64;
        if raw_index >= len {
            vm.heap.with_obj(iter_idx.0, |obj| {
                if let HeapObj::CollectionIterator(iter) = obj {
                    *iter.source.lock() = Value::Undefined;
                }
            });
            return gen_result(vm, Value::Undefined, true, false);
        }

        let index = raw_index as usize;
        let first = crate::value::utf16_get(&string, index).unwrap();
        let width = if (0xD800..=0xDBFF).contains(&first)
            && crate::value::utf16_get(&string, index + 1)
                .is_some_and(|second| (0xDC00..=0xDFFF).contains(&second))
        {
            2
        } else {
            1
        };
        let start = crate::value::utf16_index_to_byte(&string, index)
            .ok_or_else(|| Error::internal("invalid String Iterator position"))?;
        let end = crate::value::utf16_index_to_byte(&string, index + width)
            .ok_or_else(|| Error::internal("invalid String Iterator position"))?;
        let value = Value::String(Arc::from(&string[start..end]));
        vm.heap.with_obj(iter_idx.0, |obj| {
            if let HeapObj::CollectionIterator(iter) = obj {
                *iter.index.lock() = raw_index + width as u64;
            }
        });
        gen_result(vm, value, false, false)
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn setup_string_iterator_proto_in_env(
    vm: &mut Vm,
    realm: GcIdx,
    string_proto: &Value,
    iterator_base_proto: Value,
) -> error::Result<Value> {
    let next = vm.new_native_function_in_env("next", string_iterator_next, 0, realm)?;
    let iterator_method =
        vm.new_native_function_in_env("[Symbol.iterator]", string_iterator_method, 0, realm)?;
    let proto = Value::Object(GcIdx(vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(iterator_base_proto)),
        extensible: std::sync::atomic::AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?));
    if let Value::Object(proto_idx) = &proto {
        vm.heap.with_obj(proto_idx.0, |obj| {
            obj.props()
                .lock()
                .insert(PropertyKey::from("next"), data_prop(Value::Object(next)));
            let mut tag = data_prop(Value::String(Arc::from("String Iterator")));
            tag.writable = false;
            obj.props().lock().insert(
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
                tag,
            );
        });
    }
    vm.define_own_property_or_throw(
        string_proto,
        PropertyKey::symbol(vm.well_known_symbols.iterator),
        data_prop(Value::Object(iterator_method)),
    )?;
    vm.realm_string_iterator_prototypes
        .insert(realm.0, proto.clone());
    if realm == vm.global {
        vm.string_iterator_proto = proto.clone();
    }
    Ok(proto)
}

pub(crate) fn setup_array_iterator_proto(vm: &mut Vm) -> error::Result<()> {
    let iterator_base = vm.iterator_base_proto.clone();
    setup_array_iterator_proto_in_env(vm, vm.global, iterator_base)?;
    Ok(())
}

pub(crate) fn setup_array_iterator_proto_in_env(
    vm: &mut Vm,
    realm: GcIdx,
    iterator_base: Value,
) -> error::Result<Value> {
    let next = vm.new_native_function_in_env("next", array_iterator_next, 0, realm)?;
    let proto = Value::Object(GcIdx(vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(iterator_base)),
        extensible: std::sync::atomic::AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?));
    if let Value::Object(index) = &proto {
        vm.heap.with_obj(index.0, |object| {
            object
                .props()
                .lock()
                .insert(PropertyKey::from("next"), data_prop(Value::Object(next)));
            let mut tag = data_prop(Value::String(Arc::from("Array Iterator")));
            tag.writable = false;
            object.props().lock().insert(
                PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
                tag,
            );
        });
    }
    vm.realm_array_iterator_prototypes
        .insert(realm.0, proto.clone());
    if realm == vm.global {
        vm.iterator_proto = proto.clone();
    }
    Ok(proto)
}

pub(crate) fn setup_map_set_iterator_protos(vm: &mut Vm) -> error::Result<()> {
    let iterator_base = vm.iterator_base_proto.clone();
    setup_map_iterator_proto_in_env(vm, vm.global, iterator_base.clone())?;
    setup_set_iterator_proto_in_env(vm, vm.global, iterator_base)?;
    Ok(())
}

pub(crate) fn setup_map_iterator_proto_in_env(
    vm: &mut Vm,
    realm: GcIdx,
    iterator_base: Value,
) -> error::Result<Value> {
    let map_next = vm.new_native_function_in_env("next", map_iterator_next, 0, realm)?;
    let map_proto =
        collection_iterator_proto(vm, Value::Object(map_next), "Map Iterator", iterator_base)?;
    let prototype = Value::Object(map_proto);
    vm.realm_map_iterator_prototypes
        .insert(realm.0, prototype.clone());
    if realm == vm.global {
        vm.map_iterator_proto = prototype.clone();
    }
    Ok(prototype)
}

pub(crate) fn setup_set_iterator_proto_in_env(
    vm: &mut Vm,
    realm: GcIdx,
    iterator_base: Value,
) -> error::Result<Value> {
    let set_next = vm.new_native_function_in_env("next", set_iterator_next, 0, realm)?;
    let set_proto =
        collection_iterator_proto(vm, Value::Object(set_next), "Set Iterator", iterator_base)?;
    let prototype = Value::Object(set_proto);
    vm.realm_set_iterator_prototypes
        .insert(realm.0, prototype.clone());
    if realm == vm.global {
        vm.set_iterator_proto = prototype.clone();
    }
    Ok(prototype)
}

fn collection_iterator_proto(
    vm: &mut Vm,
    next: Value,
    to_string_tag: &'static str,
    iterator_base: Value,
) -> error::Result<GcIdx> {
    let proto_idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(iterator_base)),
        extensible: std::sync::atomic::AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    vm.heap.with_obj(proto_idx, |obj| {
        obj.props()
            .lock()
            .insert(PropertyKey::from("next"), data_prop(next));
        let desc = PropertyDescriptor {
            value: Value::String(Arc::from(to_string_tag)),
            writable: false,
            enumerable: false,
            configurable: true,
            get: None,
            set: None,
            is_accessor: false,
        };
        obj.props().lock().insert(
            PropertyKey::symbol(vm.well_known_symbols.to_string_tag),
            desc,
        );
    });
    Ok(GcIdx(proto_idx))
}

pub(crate) fn collection_iterator_this(
    _vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    this.ok_or_else(|| Error::type_err("Iterator method called on non-iterator"))
}

#[derive(Clone, Copy)]
enum CollectionIteratorBrand {
    Array,
    Map,
    Set,
}

impl CollectionIteratorBrand {
    fn accepts(self, kind: CollectionIteratorKind) -> bool {
        match self {
            Self::Array => matches!(
                kind,
                CollectionIteratorKind::ArrayEntries
                    | CollectionIteratorKind::ArrayKeys
                    | CollectionIteratorKind::ArrayValues
            ),
            Self::Map => matches!(
                kind,
                CollectionIteratorKind::MapEntries
                    | CollectionIteratorKind::MapKeys
                    | CollectionIteratorKind::MapValues
            ),
            Self::Set => matches!(
                kind,
                CollectionIteratorKind::SetEntries | CollectionIteratorKind::SetValues
            ),
        }
    }
}

fn array_iterator_next(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    collection_iterator_next(vm, args, this, CollectionIteratorBrand::Array)
}

fn map_iterator_next(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    collection_iterator_next(vm, args, this, CollectionIteratorBrand::Map)
}

fn set_iterator_next(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    collection_iterator_next(vm, args, this, CollectionIteratorBrand::Set)
}

fn collection_iterator_next(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
    brand: CollectionIteratorBrand,
) -> error::Result<Value> {
    let receiver = this.unwrap_or(Value::Undefined);
    let mut pin_count = vm.pin(&receiver);
    let result = (|| {
        let Value::Object(iter_idx) = receiver else {
            return Err(Error::type_err("Iterator next called on non-iterator"));
        };
        let Some((source, kind, index, set_physical_index, set_compaction_epoch)) =
            vm.heap.with_obj(iter_idx.0, |obj| {
                let HeapObj::CollectionIterator(iter) = obj else {
                    return None;
                };
                if matches!(
                    iter.kind,
                    CollectionIteratorKind::WrappedIterator | CollectionIteratorKind::StringValues
                ) {
                    return None;
                }
                Some((
                    iter.source.lock().clone(),
                    iter.kind,
                    *iter.index.lock(),
                    *iter.set_physical_index.lock(),
                    *iter.set_compaction_epoch.lock(),
                ))
            })
        else {
            return Err(Error::type_err("Iterator next called on non-iterator"));
        };
        if !brand.accepts(kind) {
            return Err(Error::type_err(
                "Iterator next called on incompatible iterator kind",
            ));
        }
        if source.is_undefined() {
            return gen_result(vm, Value::Undefined, true, false);
        }
        pin_count += vm.pin(&source);

        if matches!(
            kind,
            CollectionIteratorKind::ArrayEntries
                | CollectionIteratorKind::ArrayKeys
                | CollectionIteratorKind::ArrayValues
        ) {
            return array_iterator_step(vm, iter_idx, &source, kind, index);
        }

        let mut next_index = index.saturating_add(1);
        let mut next_set_physical_index = set_physical_index;
        let mut next_set_compaction_epoch = set_compaction_epoch;
        let next_value = match (&source, kind, usize::try_from(index).ok()) {
            (Value::Object(source_idx), CollectionIteratorKind::MapEntries, Some(index)) => {
                map_entry_at(vm, *source_idx, index)?
                    .map(|(key, value)| make_value_array_in_current_realm(vm, vec![key, value]))
                    .transpose()?
            }
            (Value::Object(source_idx), CollectionIteratorKind::MapKeys, Some(index)) => {
                map_entry_at(vm, *source_idx, index)?.map(|(key, _)| key)
            }
            (Value::Object(source_idx), CollectionIteratorKind::MapValues, Some(index)) => {
                map_entry_at(vm, *source_idx, index)?.map(|(_, value)| value)
            }
            (Value::Object(source_idx), CollectionIteratorKind::SetEntries, _) => {
                let (value, advanced, physical_index, compaction_epoch) = set_value_at(
                    vm,
                    iter_idx,
                    *source_idx,
                    index,
                    set_physical_index,
                    set_compaction_epoch,
                )?;
                next_index = advanced;
                next_set_physical_index = physical_index;
                next_set_compaction_epoch = compaction_epoch;
                value
                    .map(|value| make_value_array_in_current_realm(vm, vec![value.clone(), value]))
                    .transpose()?
            }
            (Value::Object(source_idx), CollectionIteratorKind::SetValues, _) => {
                let (value, advanced, physical_index, compaction_epoch) = set_value_at(
                    vm,
                    iter_idx,
                    *source_idx,
                    index,
                    set_physical_index,
                    set_compaction_epoch,
                )?;
                next_index = advanced;
                next_set_physical_index = physical_index;
                next_set_compaction_epoch = compaction_epoch;
                value
            }
            _ => None,
        };

        if let Some(value) = next_value {
            vm.heap.with_obj(iter_idx.0, |obj| {
                if let HeapObj::CollectionIterator(iter) = obj {
                    *iter.index.lock() = next_index;
                    *iter.set_physical_index.lock() = next_set_physical_index;
                    *iter.set_compaction_epoch.lock() = next_set_compaction_epoch;
                }
            });
            let value_pin = vm.pin(&value);
            let result = gen_result(vm, value, false, false);
            vm.unpin_many(value_pin);
            result
        } else {
            finish_collection_iterator(vm, iter_idx)
        }
    })();
    vm.unpin_many(pin_count);
    result
}

fn array_iterator_step(
    vm: &mut Vm,
    iter_idx: GcIdx,
    source: &Value,
    kind: CollectionIteratorKind,
    index: u64,
) -> error::Result<Value> {
    let typed_array_slots = match source {
        Value::Object(source_idx) => vm.heap.with_obj(source_idx.0, |obj| {
            let HeapObj::TypedArray(array) = obj else {
                return None;
            };
            Some((
                array.kind,
                array.viewed_array_buffer.clone(),
                array.byte_offset,
                array.byte_length,
                array.length_tracking,
            ))
        }),
        _ => None,
    };
    let length = if let Some((element_kind, buffer, byte_offset, byte_length, tracking)) =
        typed_array_slots
    {
        let byte_length = effective_view_byte_length(
            vm,
            buffer.as_ref(),
            byte_offset,
            byte_length,
            tracking,
            element_kind.element_size(),
        )
        .ok_or_else(|| Error::type_err("Array iterator source is out of bounds"))?;
        typed_array_element_count(element_kind, byte_length) as u64
    } else {
        length_of_array_like_u64(vm, source)?
    };
    if index >= length {
        return finish_collection_iterator(vm, iter_idx);
    }

    vm.heap.with_obj(iter_idx.0, |obj| {
        if let HeapObj::CollectionIterator(iter) = obj {
            *iter.index.lock() = index + 1;
        }
    });
    let index_value = Value::Number(index as f64);
    let result = match kind {
        CollectionIteratorKind::ArrayKeys => index_value,
        CollectionIteratorKind::ArrayValues => {
            let name = PropertyKey::integer_index_str(index);
            vm.get_property(source, &name)?
        }
        CollectionIteratorKind::ArrayEntries => {
            let name = PropertyKey::integer_index_str(index);
            let element = vm.get_property(source, &name)?;
            let element_pin = vm.pin(&element);
            let pair = make_value_array_in_current_realm(vm, vec![index_value, element]);
            vm.unpin_many(element_pin);
            pair?
        }
        _ => unreachable!(),
    };
    let result_pin = vm.pin(&result);
    let completion = gen_result(vm, result, false, false);
    vm.unpin_many(result_pin);
    completion
}

fn finish_collection_iterator(vm: &mut Vm, iter_idx: GcIdx) -> error::Result<Value> {
    vm.heap.with_obj(iter_idx.0, |obj| {
        if let HeapObj::CollectionIterator(iter) = obj {
            *iter.source.lock() = Value::Undefined;
        }
    });
    gen_result(vm, Value::Undefined, true, false)
}

fn map_entry_at(vm: &Vm, idx: GcIdx, index: usize) -> error::Result<Option<(Value, Value)>> {
    Ok(vm.heap.with_obj_read(idx.0, |obj| {
        if let HeapObj::Map(map) = obj {
            map.entries
                .lock()
                .get_index(index)
                .map(|(key, value)| (key.0.clone(), value.clone()))
        } else {
            None
        }
    }))
}

fn set_value_at(
    vm: &mut Vm,
    iterator: GcIdx,
    idx: GcIdx,
    index: u64,
    physical_index: usize,
    compaction_epoch: u64,
) -> error::Result<(Option<Value>, u64, usize, u64)> {
    let mut cursor = index;
    let mut physical_cursor = physical_index;
    let mut epoch = compaction_epoch;
    loop {
        let slot = vm.heap.with_obj(idx.0, |object| {
            let HeapObj::Set(set) = object else {
                return None;
            };
            set.items
                .lock()
                .slot_for_cursor(cursor, physical_cursor, epoch)
        });
        let Some((current_epoch, next_physical_index, generation, key)) = slot else {
            return Ok((None, cursor, physical_cursor, epoch));
        };
        vm.consume_fuel()?;
        cursor = generation + 1;
        physical_cursor = next_physical_index;
        epoch = current_epoch;
        vm.heap.with_obj(iterator.0, |object| {
            if let HeapObj::CollectionIterator(iterator) = object {
                *iterator.index.lock() = cursor;
                *iterator.set_physical_index.lock() = physical_cursor;
                *iterator.set_compaction_epoch.lock() = epoch;
            }
        });
        if let Some(key) = key {
            return Ok((Some(key.0), cursor, physical_cursor, epoch));
        }
    }
}

fn map_keys_in_order(vm: &Vm, idx: GcIdx) -> Vec<MapKey> {
    vm.heap.with_obj_read(idx.0, |obj| {
        if let HeapObj::Map(map) = obj {
            map.entries.lock().keys().cloned().collect()
        } else {
            Vec::new()
        }
    })
}

struct SetRecord {
    object: Value,
    size: f64,
    has: Value,
    keys: Value,
}

struct SetRecordKeysIterator {
    object: Value,
    next: Value,
}

fn new_empty_set(vm: &mut Vm) -> error::Result<Value> {
    let realm = vm.current_realm_global_env();
    let prototype = vm.set_prototype_for_env(realm);
    reserve_set_value_roots(
        vm,
        std::slice::from_ref(&prototype),
        #[cfg(test)]
        crate::vm::SetReservationSite::AlgebraResultRoots,
    )?;
    let prototype_pin = vm.pin(&prototype);
    #[cfg(test)]
    if std::mem::take(&mut vm.set_algebra_garbage_before_result_allocation) {
        while vm.max_heap_objects > 0 && vm.heap.live_count() < vm.max_heap_objects {
            if let Err(error) = vm.new_object() {
                vm.unpin_many(prototype_pin);
                return Err(error);
            }
        }
        vm.set_algebra_live_before_result_allocation = Some(vm.heap.live_count());
    }
    let allocation = vm.alloc(HeapObj::Set(SetData {
        items: Mutex::new(crate::value::SetStorage::new()),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype)),
        extensible: AtomicBool::new(true),
    }));
    vm.unpin_many(prototype_pin);
    Ok(Value::Object(allocation?))
}

#[cfg(test)]
fn take_set_reservation_failure(vm: &mut Vm, site: crate::vm::SetReservationSite) -> bool {
    let Some((configured_site, remaining)) = vm.fail_set_reservation else {
        return false;
    };
    if configured_site != site {
        return false;
    }
    if remaining != 0 {
        vm.fail_set_reservation = Some((configured_site, remaining - 1));
        return false;
    }
    vm.fail_set_reservation = None;
    true
}

fn reserve_set_root_slots(
    vm: &mut Vm,
    additional: usize,
    #[cfg(test)] site: crate::vm::SetReservationSite,
) -> error::Result<()> {
    #[cfg(test)]
    if take_set_reservation_failure(vm, site) {
        return Err(Error::range("Set temporary root set is too large"));
    }
    vm.try_reserve_gc_pins(additional)
}

fn reserve_set_value_roots(
    vm: &mut Vm,
    values: &[Value],
    #[cfg(test)] site: crate::vm::SetReservationSite,
) -> error::Result<()> {
    #[cfg(test)]
    if take_set_reservation_failure(vm, site) {
        return Err(Error::range("Set temporary root set is too large"));
    }
    vm.try_reserve_value_roots(values)
}

fn reserve_set_entry(vm: &mut Vm, idx: GcIdx, key: &MapKey) -> error::Result<()> {
    let needs_entry = vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::Set(set) = obj else {
            return false;
        };
        !set.items.lock().contains(key)
    });
    if !needs_entry {
        return Ok(());
    }
    #[cfg(test)]
    if take_set_reservation_failure(vm, crate::vm::SetReservationSite::EntryStorage) {
        return Err(Error::range("Set entry storage is too large"));
    }
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::Set(set) = obj else {
            return Err(Error::internal("Set entry target lost its Set data"));
        };
        let mut items = set.items.lock();
        items
            .try_reserve(1)
            .map_err(|_| Error::range("Set entry storage is too large"))?;
        Ok(())
    })
}

fn insert_set_entry(vm: &mut Vm, idx: GcIdx, value: Value) -> error::Result<()> {
    let key = MapKey::new(value);
    reserve_set_entry(vm, idx, &key)?;
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::Set(set) = obj else {
            return Err(Error::internal("Set entry target lost its Set data"));
        };
        set.items
            .lock()
            .insert(key)
            .ok_or_else(|| Error::range("Set insertion order is exhausted"))?;
        Ok(())
    })
}

fn reserve_set_algebra_entry(vm: &mut Vm, idx: GcIdx, key: &MapKey) -> error::Result<()> {
    let needs_entry = vm.heap.with_obj(idx.0, |object| {
        let HeapObj::Set(set) = object else {
            return false;
        };
        !set.items.lock().contains(key)
    });
    if !needs_entry {
        return Ok(());
    }
    #[cfg(test)]
    if take_set_reservation_failure(vm, crate::vm::SetReservationSite::AlgebraResultStorage) {
        return Err(Error::range("Set algebra result storage is too large"));
    }
    vm.heap.with_obj(idx.0, |object| {
        let HeapObj::Set(set) = object else {
            return Err(Error::internal("Set algebra result lost its Set data"));
        };
        let mut items = set.items.lock();
        items
            .try_reserve(1)
            .map_err(|_| Error::range("Set algebra result storage is too large"))?;
        Ok(())
    })
}

fn insert_set_algebra_entry(vm: &mut Vm, idx: GcIdx, value: Value) -> error::Result<()> {
    let key = MapKey::new(value);
    reserve_set_algebra_entry(vm, idx, &key)?;
    vm.heap.with_obj(idx.0, |object| {
        let HeapObj::Set(set) = object else {
            return Err(Error::internal("Set algebra result lost its Set data"));
        };
        set.items
            .lock()
            .insert(key)
            .ok_or_else(|| Error::range("Set insertion order is exhausted"))?;
        Ok(())
    })
}

fn consume_set_native_work(vm: &mut Vm, work: usize) -> error::Result<()> {
    for _ in 0..work {
        vm.consume_fuel()?;
    }
    Ok(())
}

fn delete_set_entry(vm: &mut Vm, index: GcIdx, key: &MapKey) -> error::Result<bool> {
    let compaction_work = vm.heap.with_obj(index.0, |object| {
        let HeapObj::Set(set) = object else {
            return 0;
        };
        set.items.lock().removal_compaction_work(key)
    });
    consume_set_native_work(vm, compaction_work)?;
    Ok(vm.heap.with_obj(index.0, |object| {
        let HeapObj::Set(set) = object else {
            return false;
        };
        set.items.lock().shift_remove(key)
    }))
}

fn set_delete_direct(vm: &mut Vm, set: &Value, value: &Value) -> error::Result<()> {
    if let Value::Object(index) = set {
        delete_set_entry(vm, *index, &MapKey::new(value.clone()))?;
    }
    Ok(())
}

fn set_has_direct(vm: &Vm, idx: GcIdx, value: &Value) -> bool {
    vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::Set(set) = obj {
            set.items.lock().contains(&MapKey::new(value.clone()))
        } else {
            false
        }
    })
}

fn set_record_size(vm: &mut Vm, value: &Value) -> error::Result<f64> {
    let number_size = vm.to_number(value)?;
    if number_size.is_nan() {
        return Err(Error::type_err("Set-like object size must not be NaN"));
    }
    let int_size = if number_size == 0.0 {
        0.0
    } else if number_size.is_infinite() {
        number_size
    } else {
        number_size.trunc()
    };
    if int_size < 0.0 {
        return Err(Error::range("Set-like object size must be non-negative"));
    }
    Ok(int_size)
}

fn require_set_record(vm: &mut Vm, value: Value) -> error::Result<(SetRecord, usize)> {
    if !matches!(value, Value::Object(_)) {
        return Err(Error::type_err("Set-like object must be an object"));
    }
    let raw_size = vm.get_property(&value, "size")?;
    reserve_set_value_roots(
        vm,
        std::slice::from_ref(&raw_size),
        #[cfg(test)]
        crate::vm::SetReservationSite::AlgebraRecordRoots,
    )?;
    let raw_size_pin = vm.pin(&raw_size);
    let size = set_record_size(vm, &raw_size);
    vm.unpin_many(raw_size_pin);
    let size = size?;
    let has = vm.get_property(&value, "has")?;
    if !is_callable(&has, &vm.heap) {
        return Err(Error::type_err("Set-like object has is not callable"));
    }
    reserve_set_value_roots(
        vm,
        std::slice::from_ref(&has),
        #[cfg(test)]
        crate::vm::SetReservationSite::AlgebraRecordRoots,
    )?;
    let has_pin = vm.pin(&has);
    let keys = match vm.get_property(&value, "keys") {
        Ok(keys) => keys,
        Err(error) => {
            vm.unpin_many(has_pin);
            return Err(error);
        }
    };
    if !is_callable(&keys, &vm.heap) {
        vm.unpin_many(has_pin);
        return Err(Error::type_err("Set-like object keys is not callable"));
    }
    if let Err(error) = reserve_set_value_roots(
        vm,
        std::slice::from_ref(&keys),
        #[cfg(test)]
        crate::vm::SetReservationSite::AlgebraRecordRoots,
    ) {
        vm.unpin_many(has_pin);
        return Err(error);
    }
    let keys_pin = vm.pin(&keys);
    Ok((
        SetRecord {
            object: value,
            size,
            has,
            keys,
        },
        has_pin + keys_pin,
    ))
}

fn set_record_has(vm: &mut Vm, record: &SetRecord, value: Value) -> error::Result<bool> {
    Ok(vm
        .call_function(&record.has, &[value], Some(record.object.clone()))?
        .is_truthy())
}

fn set_record_keys_iterator(
    vm: &mut Vm,
    record: &SetRecord,
) -> error::Result<(SetRecordKeysIterator, usize)> {
    let iter_obj = vm.call_function(&record.keys, &[], Some(record.object.clone()))?;
    if !matches!(iter_obj, Value::Object(_)) {
        return Err(Error::type_err("Set-like keys must return an object"));
    }
    reserve_set_value_roots(
        vm,
        std::slice::from_ref(&iter_obj),
        #[cfg(test)]
        crate::vm::SetReservationSite::AlgebraIteratorRoots,
    )?;
    let object_pin = vm.pin(&iter_obj);
    let next = match vm.get_property(&iter_obj, "next") {
        Ok(next) => next,
        Err(error) => {
            vm.unpin_many(object_pin);
            return Err(error);
        }
    };
    if let Err(error) = reserve_set_value_roots(
        vm,
        std::slice::from_ref(&next),
        #[cfg(test)]
        crate::vm::SetReservationSite::AlgebraIteratorRoots,
    ) {
        vm.unpin_many(object_pin);
        return Err(error);
    }
    let next_pin = vm.pin(&next);
    Ok((
        SetRecordKeysIterator {
            object: iter_obj,
            next,
        },
        object_pin + next_pin,
    ))
}

fn set_record_keys_iterator_next(
    vm: &mut Vm,
    iterator: &SetRecordKeysIterator,
) -> error::Result<(Value, bool)> {
    match iterator_helper_step(vm, &iterator.object, &iterator.next, true)? {
        Some(value) => Ok((value, false)),
        None => Ok((Value::Undefined, true)),
    }
}

fn set_record_keys_iterator_close(
    vm: &mut Vm,
    iterator: &SetRecordKeysIterator,
) -> error::Result<()> {
    let return_method = vm.get_property(&iterator.object, "return")?;
    if return_method.is_undefined() || return_method.is_null() {
        return Ok(());
    }
    if !is_callable(&return_method, &vm.heap) {
        return Err(Error::type_err("Iterator return is not callable"));
    }
    let result = vm.call_function(&return_method, &[], Some(iterator.object.clone()))?;
    if !matches!(result, Value::Object(_)) {
        return Err(Error::type_err("Iterator return result is not an object"));
    }
    Ok(())
}

fn close_set_record_iterator_after_error<T>(
    vm: &mut Vm,
    iterator: &SetRecordKeysIterator,
    error: Arc<Error>,
    realm: GcIdx,
) -> error::Result<T> {
    if !error.catchable() {
        return Err(error);
    }
    close_iterator_after_error_in_realm(vm, &iterator.object, error, realm)
}

fn for_each_set_record_iterator_key<F>(
    vm: &mut Vm,
    iter: &SetRecordKeysIterator,
    mut visit: F,
) -> error::Result<()>
where
    F: FnMut(&mut Vm, Value) -> error::Result<bool>,
{
    let realm = vm.current_realm_global_env();
    #[cfg(test)]
    if std::mem::take(&mut vm.set_algebra_zero_fuel_before_step) {
        vm.set_fuel(Some(0));
    }
    loop {
        let (value, done) = set_record_keys_iterator_next(vm, iter)?;
        if done {
            break;
        }
        let mut value_pin = 0;
        let visit_result = (|| {
            reserve_set_value_roots(
                vm,
                std::slice::from_ref(&value),
                #[cfg(test)]
                crate::vm::SetReservationSite::AlgebraTraversalRoots,
            )?;
            value_pin = vm.pin(&value);
            visit(vm, value)
        })();
        vm.unpin_many(value_pin);
        match visit_result {
            Ok(true) => {}
            Ok(false) => {
                set_record_keys_iterator_close(vm, iter)?;
                break;
            }
            Err(error) if !error.catchable() => return Err(error),
            Err(error) => {
                return close_set_record_iterator_after_error(vm, iter, error, realm);
            }
        }
    }
    Ok(())
}

fn for_each_set_record_key<F>(vm: &mut Vm, record: &SetRecord, visit: F) -> error::Result<()>
where
    F: FnMut(&mut Vm, Value) -> error::Result<bool>,
{
    let (iter, iterator_pins) = set_record_keys_iterator(vm, record)?;
    let result = for_each_set_record_iterator_key(vm, &iter, visit);
    vm.unpin_many(iterator_pins);
    result
}

struct SetVisitQueue {
    cursor: u64,
    physical_index: usize,
    compaction_epoch: u64,
}

impl SetVisitQueue {
    fn new() -> Self {
        Self {
            cursor: 0,
            physical_index: 0,
            compaction_epoch: u64::MAX,
        }
    }

    fn next(&mut self, vm: &mut Vm, set: GcIdx) -> error::Result<Option<MapKey>> {
        #[cfg(test)]
        if std::mem::take(&mut vm.set_algebra_zero_fuel_before_step) {
            vm.set_fuel(Some(0));
        }
        loop {
            let slot = vm.heap.with_obj(set.0, |object| {
                let HeapObj::Set(set) = object else {
                    return None;
                };
                set.items.lock().slot_for_cursor(
                    self.cursor,
                    self.physical_index,
                    self.compaction_epoch,
                )
            });
            let Some((compaction_epoch, physical_index, generation, key)) = slot else {
                return Ok(None);
            };
            vm.consume_fuel()?;
            self.cursor = generation + 1;
            self.physical_index = physical_index;
            self.compaction_epoch = compaction_epoch;
            if let Some(key) = key {
                return Ok(Some(key));
            }
        }
    }
}

fn with_set_traversal_value<R>(
    vm: &mut Vm,
    key: MapKey,
    body: impl FnOnce(&mut Vm, Value) -> error::Result<R>,
) -> error::Result<R> {
    let value = key.0;
    reserve_set_value_roots(
        vm,
        std::slice::from_ref(&value),
        #[cfg(test)]
        crate::vm::SetReservationSite::AlgebraTraversalRoots,
    )?;
    let pin_count = vm.pin(&value);
    let result = body(vm, value);
    vm.unpin_many(pin_count);
    result
}

fn with_set_record<R>(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    name: &str,
    body: impl FnOnce(&mut Vm, GcIdx, &SetRecord) -> error::Result<R>,
) -> error::Result<R> {
    let receiver = this.unwrap_or(Value::Undefined);
    let index = require_set_receiver(vm, Some(receiver.clone()), name)?;
    let other = args.first().cloned().unwrap_or(Value::Undefined);
    reserve_set_value_roots(
        vm,
        &[receiver.clone(), other.clone()],
        #[cfg(test)]
        crate::vm::SetReservationSite::AlgebraInputs,
    )?;
    let mut pin_count = vm.pin_many(&[receiver, other.clone()]);
    let result = (|| {
        let (record, record_pins) = require_set_record(vm, other)?;
        pin_count += record_pins;
        body(vm, index, &record)
    })();
    vm.unpin_many(pin_count);
    result
}

fn new_pinned_set_result(vm: &mut Vm) -> error::Result<(Value, GcIdx, usize)> {
    let result = new_empty_set(vm)?;
    let Value::Object(index) = result else {
        return Err(Error::internal(
            "Set algebra result allocation was not an object",
        ));
    };
    let pin_count = vm.pin(&result);
    Ok((result, index, pin_count))
}

fn copy_set_for_algebra(vm: &mut Vm, source: GcIdx) -> error::Result<(Value, GcIdx, usize)> {
    let (result, result_index, result_pin) = new_pinned_set_result(vm)?;
    let copy = (|| {
        let mut queue = SetVisitQueue::new();
        while let Some(key) = queue.next(vm, source)? {
            with_set_traversal_value(vm, key, |vm, value| {
                insert_set_algebra_entry(vm, result_index, value)
            })?;
        }
        Ok(())
    })();
    if let Err(error) = copy {
        vm.unpin_many(result_pin);
        return Err(error);
    }
    Ok((result, result_index, result_pin))
}

fn extend_collection_visit_queue(
    queue: &mut Vec<MapKey>,
    cursor: usize,
    current: &[MapKey],
    last_yielded: Option<&MapKey>,
) {
    let remaining: Vec<MapKey> = queue[cursor..].to_vec();
    let cutoff = current
        .iter()
        .enumerate()
        .filter(|(_, key)| remaining.iter().any(|known| known == *key))
        .map(|(index, _)| index)
        .max()
        .or_else(|| last_yielded.and_then(|last| current.iter().position(|key| key == last)));

    let candidates: Vec<MapKey> = if let Some(cutoff) = cutoff {
        current.iter().skip(cutoff + 1).cloned().collect()
    } else if remaining.is_empty() {
        current.to_vec()
    } else {
        Vec::new()
    };

    for key in candidates {
        if !remaining.iter().any(|known| known == &key)
            && !queue[cursor..].iter().any(|known| known == &key)
        {
            queue.push(key);
        }
    }
}

pub(crate) fn map_set(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let val = args.get(1).cloned().unwrap_or(Value::Undefined);
    let idx = require_map_receiver(vm, this.clone(), "Map.prototype.set")?;
    insert_map_entry(vm, idx, key, val)?;
    Ok(this.unwrap_or(Value::Undefined))
}
pub(crate) fn map_get(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let idx = require_map_receiver(vm, this, "Map.prototype.get")?;
    Ok(vm.heap.with_obj_read(idx.0, |obj| {
        if let HeapObj::Map(m) = obj {
            m.entries
                .lock()
                .get(&MapKey::new(key))
                .cloned()
                .unwrap_or(Value::Undefined)
        } else {
            Value::Undefined
        }
    }))
}
pub(crate) fn map_has(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let idx = require_map_receiver(vm, this, "Map.prototype.has")?;
    Ok(Value::Bool(vm.heap.with_obj_read(idx.0, |obj| {
        if let HeapObj::Map(m) = obj {
            m.entries.lock().contains_key(&MapKey::new(key))
        } else {
            false
        }
    })))
}
pub(crate) fn map_delete(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let key = args.first().cloned().unwrap_or(Value::Undefined);
    let idx = require_map_receiver(vm, this, "Map.prototype.delete")?;
    Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::Map(m) = obj {
            m.entries.lock().shift_remove(&MapKey::new(key)).is_some()
        } else {
            false
        }
    })))
}

fn canonicalize_keyed_collection_key(value: Value) -> Value {
    MapKey::new(value).0
}

fn map_get_direct(vm: &Vm, idx: GcIdx, key: &Value) -> Option<Value> {
    vm.heap.with_obj_read(idx.0, |obj| {
        if let HeapObj::Map(m) = obj {
            m.entries.lock().get(&MapKey::new(key.clone())).cloned()
        } else {
            None
        }
    })
}

#[cfg(test)]
fn take_map_reservation_failure(vm: &mut Vm, site: crate::vm::MapReservationSite) -> bool {
    let Some((configured_site, remaining)) = vm.fail_map_reservation else {
        return false;
    };
    if configured_site != site {
        return false;
    }
    if remaining != 0 {
        vm.fail_map_reservation = Some((configured_site, remaining - 1));
        return false;
    }
    vm.fail_map_reservation = None;
    true
}

fn reserve_map_root_slots(
    vm: &mut Vm,
    additional: usize,
    #[cfg(test)] site: crate::vm::MapReservationSite,
) -> error::Result<()> {
    #[cfg(test)]
    if take_map_reservation_failure(vm, site) {
        return Err(Error::range("Map temporary root set is too large"));
    }
    vm.try_reserve_gc_pins(additional)
}

fn reserve_map_entry(vm: &mut Vm, idx: GcIdx, key: &MapKey) -> error::Result<()> {
    let needs_entry = vm.heap.with_obj_read(idx.0, |obj| {
        let HeapObj::Map(map) = obj else {
            return false;
        };
        !map.entries.lock().contains_key(key)
    });
    if !needs_entry {
        return Ok(());
    }
    #[cfg(test)]
    if take_map_reservation_failure(vm, crate::vm::MapReservationSite::EntryStorage) {
        return Err(Error::range("Map entry storage is too large"));
    }
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::Map(map) = obj else {
            return Err(Error::internal("Map entry target lost its Map data"));
        };
        let mut entries = map.entries.lock();
        if entries.len() == entries.capacity() {
            entries
                .try_reserve(1)
                .map_err(|_| Error::range("Map entry storage is too large"))?;
        }
        Ok(())
    })
}

fn insert_map_entry(vm: &mut Vm, idx: GcIdx, key: Value, value: Value) -> error::Result<()> {
    let key = MapKey::new(key);
    reserve_map_entry(vm, idx, &key)?;
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::Map(map) = obj else {
            return Err(Error::internal("Map entry target lost its Map data"));
        };
        map.entries.lock().insert(key, value);
        Ok(())
    })
}

fn reserve_map_group_by_groups(
    vm: &mut Vm,
    groups: &mut IndexMap<MapKey, Vec<Value>>,
) -> error::Result<()> {
    let _ = &vm;
    #[cfg(test)]
    if take_group_by_reservation_failure(vm, crate::vm::GroupByReservationSite::MapGroups) {
        return Err(Error::range("Map.groupBy group list is too large"));
    }
    if groups.len() < groups.capacity() {
        return Ok(());
    }
    groups
        .try_reserve(1)
        .map_err(|_| Error::range("Map.groupBy group list is too large"))
}

fn reserve_map_group_by_elements(vm: &mut Vm, values: &mut Vec<Value>) -> error::Result<()> {
    let _ = &vm;
    #[cfg(test)]
    if take_group_by_reservation_failure(vm, crate::vm::GroupByReservationSite::MapElements) {
        return Err(Error::range("Map.groupBy element list is too large"));
    }
    if values.len() < values.capacity() {
        return Ok(());
    }
    values
        .try_reserve(1)
        .map_err(|_| Error::range("Map.groupBy element list is too large"))
}

fn reserve_map_group_by_result_entry(vm: &mut Vm, map: GcIdx) -> error::Result<()> {
    #[cfg(test)]
    if take_group_by_reservation_failure(vm, crate::vm::GroupByReservationSite::MapResultEntries) {
        return Err(Error::range("Map.groupBy result is too large"));
    }
    vm.heap.with_obj(map.0, |object| {
        let HeapObj::Map(map) = object else {
            return Err(Error::internal("Map.groupBy result lost its Map data"));
        };
        let mut entries = map.entries.lock();
        if entries.len() < entries.capacity() {
            return Ok(());
        }
        entries
            .try_reserve(1)
            .map_err(|_| Error::range("Map.groupBy result is too large"))
    })
}

pub(crate) fn map_group_by(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let items = args.first().cloned().unwrap_or(Value::Undefined);
    if items.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let callback = args.get(1).cloned().unwrap_or(Value::Undefined);
    if !is_callable(&callback, &vm.heap) {
        return Err(Error::type_err("Map.groupBy callback must be callable"));
    }

    let realm = vm.current_realm_global_env();
    reserve_group_by_value_roots(
        vm,
        &[items.clone(), callback.clone()],
        #[cfg(test)]
        crate::vm::GroupByReservationSite::InputRoots,
    )?;
    let input_pins = vm.pin_many(&[items.clone(), callback.clone()]);
    let result = (|| -> error::Result<Value> {
        reserve_group_by_root_slots(
            vm,
            4,
            #[cfg(test)]
            crate::vm::GroupByReservationSite::IteratorRoots,
        )?;
        let iterator = get_sync_iterator(vm, items)?;
        let iterator_pins = vm.pin_many(&[iterator.iterator.clone(), iterator.next_method.clone()]);
        let mut groups: IndexMap<MapKey, Vec<Value>> = IndexMap::new();
        let mut group_pins = 0;
        let grouping = (|| -> error::Result<()> {
            #[cfg(test)]
            let mut index = vm.group_by_index_override.take().unwrap_or(0);
            #[cfg(not(test))]
            let mut index = 0u64;
            loop {
                if index >= 9_007_199_254_740_991 {
                    return close_iterator_after_error_in_realm(
                        vm,
                        &iterator.iterator,
                        Error::type_err("Map.groupBy index exceeds the safe integer limit"),
                        realm,
                    );
                }
                #[cfg(test)]
                if std::mem::take(&mut vm.group_by_zero_fuel_before_step) {
                    vm.set_fuel(Some(0));
                }
                let Some(value) =
                    iterator_helper_step(vm, &iterator.iterator, &iterator.next_method, true)?
                else {
                    return Ok(());
                };

                let mut value_pin = 0;
                let mut key_pin = 0;
                let process = (|| -> error::Result<MapKey> {
                    reserve_group_by_value_roots(
                        vm,
                        std::slice::from_ref(&value),
                        #[cfg(test)]
                        crate::vm::GroupByReservationSite::ValueRoots,
                    )?;
                    value_pin = vm.pin(&value);
                    let key_value = vm.call_function(
                        &callback,
                        &[value.clone(), Value::Number(index as f64)],
                        Some(Value::Undefined),
                    )?;
                    reserve_group_by_value_roots(
                        vm,
                        std::slice::from_ref(&key_value),
                        #[cfg(test)]
                        crate::vm::GroupByReservationSite::KeyRoots,
                    )?;
                    key_pin = vm.pin(&key_value);
                    Ok(MapKey::new(key_value))
                })();
                let key = match process {
                    Ok(key) => key,
                    Err(error) => {
                        vm.unpin_many(value_pin + key_pin);
                        if !error.catchable() {
                            return Err(error);
                        }
                        return close_iterator_after_error_in_realm(
                            vm,
                            &iterator.iterator,
                            error,
                            realm,
                        );
                    }
                };

                let storage = if let Some(values) = groups.get_mut(&key) {
                    // The first key stored in the group already owns the
                    // identity root; repeated callback keys are discarded.
                    vm.unpin_many(key_pin);
                    key_pin = 0;
                    reserve_map_group_by_elements(vm, values).map(|()| values.push(value))
                } else {
                    reserve_map_group_by_groups(vm, &mut groups).and_then(|()| {
                        let mut values = Vec::new();
                        reserve_map_group_by_elements(vm, &mut values)?;
                        values.push(value);
                        groups.insert(key, values);
                        Ok(())
                    })
                };
                if let Err(error) = storage {
                    vm.unpin_many(value_pin + key_pin);
                    return close_iterator_after_error_in_realm(
                        vm,
                        &iterator.iterator,
                        error,
                        realm,
                    );
                }
                group_pins += value_pin + key_pin;
                index += 1;
            }
        })();
        if let Err(error) = grouping {
            vm.unpin_many(group_pins);
            vm.unpin_many(iterator_pins);
            return Err(error);
        }

        #[cfg(test)]
        {
            vm.map_group_by_output_pin_depth = Some(vm.gc_pins.len());
        }

        let output = (|| -> error::Result<Value> {
            let prototype = vm.map_prototype_for_env(realm);
            let output_roots = Vm::value_root_count(&prototype)
                .checked_add(2)
                .ok_or_else(|| Error::range("Map.groupBy temporary root set is too large"))?;
            vm.try_reserve_gc_pins(output_roots)?;
            let prototype_pin = vm.pin(&prototype);
            let allocation = vm.alloc(HeapObj::Map(MapData {
                entries: Mutex::new(IndexMap::new()),
                props: Mutex::new(IndexMap::new()),
                proto: Mutex::new(Some(prototype)),
                extensible: AtomicBool::new(true),
            }));
            vm.unpin_many(prototype_pin);
            let map_idx = allocation?;
            let map = Value::Object(map_idx);
            let map_pin = vm.pin(&map);
            let completion = (|| -> error::Result<Value> {
                for (key, values) in groups {
                    vm.consume_fuel()?;
                    #[cfg(test)]
                    if take_group_by_reservation_failure(
                        vm,
                        crate::vm::GroupByReservationSite::MapResultArrays,
                    ) {
                        return Err(Error::range("Map.groupBy result Array is too large"));
                    }
                    let array = make_value_array_in_env(vm, values, realm)?;
                    let array_pin = vm.pin(&array);
                    let publication = (|| {
                        reserve_map_group_by_result_entry(vm, map_idx)?;
                        vm.heap.with_obj(map_idx.0, |object| {
                            let HeapObj::Map(map) = object else {
                                return Err(Error::internal(
                                    "Map.groupBy result lost its Map data",
                                ));
                            };
                            map.entries.lock().insert(key, array);
                            Ok(())
                        })
                    })();
                    vm.unpin_many(array_pin);
                    publication?;
                }
                Ok(map)
            })();
            vm.unpin_many(map_pin);
            completion
        })();
        vm.unpin_many(group_pins);
        vm.unpin_many(iterator_pins);
        output
    })();
    vm.unpin_many(input_pins);
    result
}

pub(crate) fn map_get_or_insert(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let idx = require_map_receiver(vm, this, "Map.prototype.getOrInsert")?;
    let key = canonicalize_keyed_collection_key(args.first().cloned().unwrap_or(Value::Undefined));
    if let Some(value) = map_get_direct(vm, idx, &key) {
        return Ok(value);
    }
    let value = args.get(1).cloned().unwrap_or(Value::Undefined);
    insert_map_entry(vm, idx, key, value.clone())?;
    Ok(value)
}

pub(crate) fn map_get_or_insert_computed(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let idx = require_map_receiver(vm, this, "Map.prototype.getOrInsertComputed")?;
    let callback = args.get(1).cloned().unwrap_or(Value::Undefined);
    if !is_callable(&callback, &vm.heap) {
        return Err(Error::type_err(
            "Map.prototype.getOrInsertComputed callback is not callable",
        ));
    }
    let key = canonicalize_keyed_collection_key(args.first().cloned().unwrap_or(Value::Undefined));
    if let Some(value) = map_get_direct(vm, idx, &key) {
        return Ok(value);
    }
    let value = vm.call_function(
        &callback,
        std::slice::from_ref(&key),
        Some(Value::Undefined),
    )?;
    insert_map_entry(vm, idx, key, value.clone())?;
    Ok(value)
}

// --- WeakMap / WeakSet (true weak-reference semantics) ---

fn require_weakmap_receiver(vm: &Vm, this: Option<Value>, name: &str) -> error::Result<GcIdx> {
    let Some(Value::Object(index)) = this else {
        return Err(Error::type_err(format!("{name} called on non-WeakMap")));
    };
    if vm
        .heap
        .with_obj(index.0, |object| matches!(object, HeapObj::WeakMap(_)))
    {
        Ok(index)
    } else {
        Err(Error::type_err(format!("{name} called on non-WeakMap")))
    }
}

fn require_weakset_receiver(vm: &Vm, this: Option<Value>, name: &str) -> error::Result<GcIdx> {
    let Some(Value::Object(index)) = this else {
        return Err(Error::type_err(format!("{name} called on non-WeakSet")));
    };
    if vm
        .heap
        .with_obj(index.0, |object| matches!(object, HeapObj::WeakSet(_)))
    {
        Ok(index)
    } else {
        Err(Error::type_err(format!("{name} called on non-WeakSet")))
    }
}

fn weak_collection_key(vm: &Vm, value: &Value) -> Option<crate::value::WeakKey> {
    match value {
        Value::Object(index) => Some(crate::value::WeakKey::Object(index.0)),
        Value::Symbol(id) if can_be_held_weakly(vm, value) => {
            Some(crate::value::WeakKey::Symbol(*id))
        }
        _ => None,
    }
}

#[cfg(test)]
fn take_weak_collection_reservation_failure(
    vm: &mut Vm,
    site: crate::vm::WeakCollectionReservationSite,
) -> bool {
    let Some((configured_site, remaining)) = vm.fail_weak_collection_reservation else {
        return false;
    };
    if configured_site != site {
        return false;
    }
    if remaining != 0 {
        vm.fail_weak_collection_reservation = Some((configured_site, remaining - 1));
        return false;
    }
    vm.fail_weak_collection_reservation = None;
    true
}

fn reserve_weak_collection_roots(
    vm: &mut Vm,
    additional: usize,
    message: &'static str,
    #[cfg(test)] site: crate::vm::WeakCollectionReservationSite,
) -> error::Result<()> {
    #[cfg(test)]
    if take_weak_collection_reservation_failure(vm, site) {
        return Err(Error::range(message));
    }
    vm.try_reserve_gc_pins(additional)
}

fn reserve_weakmap_entry(
    vm: &mut Vm,
    index: GcIdx,
    key: crate::value::WeakKey,
) -> error::Result<()> {
    let needs_entry = vm.heap.with_obj(index.0, |object| {
        let HeapObj::WeakMap(map) = object else {
            return false;
        };
        !map.entries.lock().contains_key(&key)
    });
    if !needs_entry {
        return Ok(());
    }
    #[cfg(test)]
    if take_weak_collection_reservation_failure(
        vm,
        crate::vm::WeakCollectionReservationSite::MapEntryStorage,
    ) {
        return Err(Error::range("WeakMap entry storage is too large"));
    }
    vm.heap.with_obj(index.0, |object| {
        let HeapObj::WeakMap(map) = object else {
            return Err(Error::internal("WeakMap target lost its internal data"));
        };
        let mut entries = map.entries.lock();
        if entries.len() == entries.capacity() {
            entries
                .try_reserve(1)
                .map_err(|_| Error::range("WeakMap entry storage is too large"))?;
        }
        Ok(())
    })
}

fn insert_weakmap_entry(
    vm: &mut Vm,
    index: GcIdx,
    key: crate::value::WeakKey,
    value: Value,
) -> error::Result<()> {
    reserve_weakmap_entry(vm, index, key)?;
    vm.heap.with_obj(index.0, |object| {
        let HeapObj::WeakMap(map) = object else {
            return Err(Error::internal("WeakMap target lost its internal data"));
        };
        map.entries.lock().insert(key, value);
        Ok(())
    })
}

fn reserve_weakset_entry(
    vm: &mut Vm,
    index: GcIdx,
    key: crate::value::WeakKey,
) -> error::Result<()> {
    let needs_entry = vm.heap.with_obj(index.0, |object| {
        let HeapObj::WeakSet(set) = object else {
            return false;
        };
        !set.items.lock().contains(&key)
    });
    if !needs_entry {
        return Ok(());
    }
    #[cfg(test)]
    if take_weak_collection_reservation_failure(
        vm,
        crate::vm::WeakCollectionReservationSite::SetEntryStorage,
    ) {
        return Err(Error::range("WeakSet entry storage is too large"));
    }
    vm.heap.with_obj(index.0, |object| {
        let HeapObj::WeakSet(set) = object else {
            return Err(Error::internal("WeakSet target lost its internal data"));
        };
        let mut items = set.items.lock();
        if items.len() == items.capacity() {
            items
                .try_reserve(1)
                .map_err(|_| Error::range("WeakSet entry storage is too large"))?;
        }
        Ok(())
    })
}

fn insert_weakset_entry(
    vm: &mut Vm,
    index: GcIdx,
    key: crate::value::WeakKey,
) -> error::Result<()> {
    reserve_weakset_entry(vm, index, key)?;
    vm.heap.with_obj(index.0, |object| {
        let HeapObj::WeakSet(set) = object else {
            return Err(Error::internal("WeakSet target lost its internal data"));
        };
        set.items.lock().insert(key);
        Ok(())
    })
}

pub(crate) fn weakmap_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err(
            "WeakMap constructor must be called with new",
        ));
    }
    let realm = vm.current_realm_global_env();
    let iterable = args.first().cloned().unwrap_or(Value::Undefined);
    let fallback = vm.weakmap_prototype_for_env(realm);
    let prototype = native_constructor_prototype_with_default(vm, "WeakMap", fallback)?;
    let initial_roots = Vm::value_root_count(&prototype)
        .checked_add(Vm::value_root_count(&iterable))
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| Error::range("WeakMap temporary root set is too large"))?;
    reserve_weak_collection_roots(
        vm,
        initial_roots,
        "WeakMap temporary root set is too large",
        #[cfg(test)]
        crate::vm::WeakCollectionReservationSite::MapConstructorRoots,
    )?;
    let mut initial_pins = vm.pin_many(&[prototype.clone(), iterable.clone()]);
    let allocation = vm.alloc(HeapObj::WeakMap(crate::value::WeakMapData {
        entries: Mutex::new(std::collections::HashMap::new()),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype)),
        extensible: AtomicBool::new(true),
    }));
    let map = match allocation {
        Ok(index) => Value::Object(index),
        Err(error) => {
            vm.unpin_many(initial_pins);
            return Err(error);
        }
    };
    initial_pins += vm.pin(&map);

    let result = (|| -> error::Result<Value> {
        if iterable.is_nullish() {
            return Ok(map.clone());
        }
        reserve_weak_collection_roots(
            vm,
            6,
            "WeakMap temporary root set is too large",
            #[cfg(test)]
            crate::vm::WeakCollectionReservationSite::MapIteratorRoots,
        )?;
        let adder = vm.get_property(&map, "set")?;
        if !is_callable(&adder, &vm.heap) {
            return Err(Error::type_err("WeakMap set is not callable"));
        }
        let adder_pin = vm.pin(&adder);
        let iterator = match get_sync_iterator(vm, iterable.clone()) {
            Ok(iterator) => iterator,
            Err(error) => {
                vm.unpin_many(adder_pin);
                return Err(error);
            }
        };
        let iterator_pins = vm.pin_many(&[iterator.iterator.clone(), iterator.next_method.clone()]);
        let iteration = (|| -> error::Result<Value> {
            loop {
                #[cfg(test)]
                if std::mem::take(&mut vm.weakmap_constructor_zero_fuel_before_step) {
                    vm.set_fuel(Some(0));
                }
                let Some(entry) =
                    iterator_helper_step(vm, &iterator.iterator, &iterator.next_method, true)?
                else {
                    return Ok(map.clone());
                };
                if !matches!(entry, Value::Object(_)) {
                    return close_iterator_after_error_in_realm(
                        vm,
                        &iterator.iterator,
                        Error::type_err("Iterator value is not an object"),
                        realm,
                    );
                }
                let mut entry_pins = vm.pin(&entry);
                let status = (|| -> error::Result<()> {
                    let key = vm.get_property(&entry, "0")?;
                    entry_pins += vm.pin(&key);
                    let value = vm.get_property(&entry, "1")?;
                    entry_pins += vm.pin(&value);
                    vm.call_function(&adder, &[key, value], Some(map.clone()))?;
                    Ok(())
                })();
                vm.unpin_many(entry_pins);
                if let Err(error) = status {
                    if !error.catchable() {
                        return Err(error);
                    }
                    return close_iterator_after_error_in_realm(
                        vm,
                        &iterator.iterator,
                        error,
                        realm,
                    );
                }
            }
        })();
        vm.unpin_many(iterator_pins);
        vm.unpin_many(adder_pin);
        iteration
    })();
    vm.unpin_many(initial_pins);
    result
}

pub(crate) fn weakmap_set(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.clone().unwrap_or(Value::Undefined);
    let index = require_weakmap_receiver(vm, this, "WeakMap.prototype.set")?;
    let key_value = args.first().cloned().unwrap_or(Value::Undefined);
    let Some(key) = weak_collection_key(vm, &key_value) else {
        return Err(Error::type_err("Invalid value used as weak map key"));
    };
    let value = args.get(1).cloned().unwrap_or(Value::Undefined);
    insert_weakmap_entry(vm, index, key, value)?;
    Ok(receiver)
}

pub(crate) fn weakmap_get(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let index = require_weakmap_receiver(vm, this, "WeakMap.prototype.get")?;
    let key_value = args.first().cloned().unwrap_or(Value::Undefined);
    let Some(key) = weak_collection_key(vm, &key_value) else {
        return Ok(Value::Undefined);
    };
    Ok(vm.heap.with_obj(index.0, |object| {
        let HeapObj::WeakMap(map) = object else {
            return Value::Undefined;
        };
        map.entries
            .lock()
            .get(&key)
            .cloned()
            .unwrap_or(Value::Undefined)
    }))
}

pub(crate) fn weakmap_has(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let index = require_weakmap_receiver(vm, this, "WeakMap.prototype.has")?;
    let key_value = args.first().cloned().unwrap_or(Value::Undefined);
    let Some(key) = weak_collection_key(vm, &key_value) else {
        return Ok(Value::Bool(false));
    };
    Ok(Value::Bool(vm.heap.with_obj(index.0, |object| {
        let HeapObj::WeakMap(map) = object else {
            return false;
        };
        map.entries.lock().contains_key(&key)
    })))
}

pub(crate) fn weakmap_delete(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let index = require_weakmap_receiver(vm, this, "WeakMap.prototype.delete")?;
    let key_value = args.first().cloned().unwrap_or(Value::Undefined);
    let Some(key) = weak_collection_key(vm, &key_value) else {
        return Ok(Value::Bool(false));
    };
    Ok(Value::Bool(vm.heap.with_obj(index.0, |object| {
        let HeapObj::WeakMap(map) = object else {
            return false;
        };
        map.entries.lock().remove(&key).is_some()
    })))
}

pub(crate) fn weakmap_get_or_insert(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let index = require_weakmap_receiver(vm, this, "WeakMap.prototype.getOrInsert")?;
    let key_value = args.first().cloned().unwrap_or(Value::Undefined);
    let Some(key) = weak_collection_key(vm, &key_value) else {
        return Err(Error::type_err("Invalid value used as weak map key"));
    };
    if let Some(value) = vm.heap.with_obj(index.0, |object| {
        let HeapObj::WeakMap(map) = object else {
            return None;
        };
        map.entries.lock().get(&key).cloned()
    }) {
        return Ok(value);
    }
    let value = args.get(1).cloned().unwrap_or(Value::Undefined);
    insert_weakmap_entry(vm, index, key, value.clone())?;
    Ok(value)
}

pub(crate) fn weakmap_get_or_insert_computed(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.clone().unwrap_or(Value::Undefined);
    let index = require_weakmap_receiver(vm, this, "WeakMap.prototype.getOrInsertComputed")?;
    let callback = args.get(1).cloned().unwrap_or(Value::Undefined);
    if !is_callable(&callback, &vm.heap) {
        return Err(Error::type_err(
            "WeakMap.prototype.getOrInsertComputed callback is not callable",
        ));
    }
    let key_value = args.first().cloned().unwrap_or(Value::Undefined);
    let Some(key) = weak_collection_key(vm, &key_value) else {
        return Err(Error::type_err("Invalid value used as weak map key"));
    };
    if let Some(value) = vm.heap.with_obj(index.0, |object| {
        let HeapObj::WeakMap(map) = object else {
            return None;
        };
        map.entries.lock().get(&key).cloned()
    }) {
        return Ok(value);
    }

    let root_count = Vm::value_root_count(&receiver)
        .checked_add(Vm::value_root_count(&key_value))
        .and_then(|count| count.checked_add(Vm::value_root_count(&callback)))
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| Error::range("WeakMap temporary root set is too large"))?;
    reserve_weak_collection_roots(
        vm,
        root_count,
        "WeakMap temporary root set is too large",
        #[cfg(test)]
        crate::vm::WeakCollectionReservationSite::MapComputedRoots,
    )?;
    let mut pin_count = vm.pin_many(&[receiver, key_value.clone(), callback.clone()]);
    let result = (|| -> error::Result<Value> {
        let value = vm.call_function(
            &callback,
            std::slice::from_ref(&key_value),
            Some(Value::Undefined),
        )?;
        pin_count += vm.pin(&value);
        insert_weakmap_entry(vm, index, key, value.clone())?;
        Ok(value)
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn weakset_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err(
            "WeakSet constructor must be called with new",
        ));
    }
    let realm = vm.current_realm_global_env();
    let iterable = args.first().cloned().unwrap_or(Value::Undefined);
    let fallback = vm.weakset_prototype_for_env(realm);
    let prototype = native_constructor_prototype_with_default(vm, "WeakSet", fallback)?;
    let initial_roots = Vm::value_root_count(&prototype)
        .checked_add(Vm::value_root_count(&iterable))
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| Error::range("WeakSet temporary root set is too large"))?;
    reserve_weak_collection_roots(
        vm,
        initial_roots,
        "WeakSet temporary root set is too large",
        #[cfg(test)]
        crate::vm::WeakCollectionReservationSite::SetConstructorRoots,
    )?;
    let mut initial_pins = vm.pin_many(&[prototype.clone(), iterable.clone()]);
    let allocation = vm.alloc(HeapObj::WeakSet(crate::value::WeakSetData {
        items: Mutex::new(std::collections::HashSet::new()),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(prototype)),
        extensible: AtomicBool::new(true),
    }));
    let set = match allocation {
        Ok(index) => Value::Object(index),
        Err(error) => {
            vm.unpin_many(initial_pins);
            return Err(error);
        }
    };
    initial_pins += vm.pin(&set);

    let result = (|| -> error::Result<Value> {
        if iterable.is_nullish() {
            return Ok(set.clone());
        }
        reserve_weak_collection_roots(
            vm,
            4,
            "WeakSet temporary root set is too large",
            #[cfg(test)]
            crate::vm::WeakCollectionReservationSite::SetIteratorRoots,
        )?;
        let adder = vm.get_property(&set, "add")?;
        if !is_callable(&adder, &vm.heap) {
            return Err(Error::type_err("WeakSet add is not callable"));
        }
        let adder_pin = vm.pin(&adder);
        let iterator = match get_sync_iterator(vm, iterable.clone()) {
            Ok(iterator) => iterator,
            Err(error) => {
                vm.unpin_many(adder_pin);
                return Err(error);
            }
        };
        let iterator_pins = vm.pin_many(&[iterator.iterator.clone(), iterator.next_method.clone()]);
        let iteration = (|| -> error::Result<Value> {
            loop {
                #[cfg(test)]
                if std::mem::take(&mut vm.weakset_constructor_zero_fuel_before_step) {
                    vm.set_fuel(Some(0));
                }
                let Some(value) =
                    iterator_helper_step(vm, &iterator.iterator, &iterator.next_method, true)?
                else {
                    return Ok(set.clone());
                };
                let value_pin = vm.pin(&value);
                let status = vm.call_function(&adder, &[value], Some(set.clone()));
                vm.unpin_many(value_pin);
                if let Err(error) = status {
                    if !error.catchable() {
                        return Err(error);
                    }
                    return close_iterator_after_error_in_realm(
                        vm,
                        &iterator.iterator,
                        error,
                        realm,
                    );
                }
            }
        })();
        vm.unpin_many(iterator_pins);
        vm.unpin_many(adder_pin);
        iteration
    })();
    vm.unpin_many(initial_pins);
    result
}

pub(crate) fn weakset_add(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let receiver = this.clone().unwrap_or(Value::Undefined);
    let index = require_weakset_receiver(vm, this, "WeakSet.prototype.add")?;
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    let Some(key) = weak_collection_key(vm, &value) else {
        return Err(Error::type_err("Invalid value used in weak set"));
    };
    insert_weakset_entry(vm, index, key)?;
    Ok(receiver)
}

pub(crate) fn weakset_has(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let index = require_weakset_receiver(vm, this, "WeakSet.prototype.has")?;
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    let Some(key) = weak_collection_key(vm, &value) else {
        return Ok(Value::Bool(false));
    };
    Ok(Value::Bool(vm.heap.with_obj(index.0, |object| {
        let HeapObj::WeakSet(set) = object else {
            return false;
        };
        set.items.lock().contains(&key)
    })))
}

pub(crate) fn weakset_delete(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let index = require_weakset_receiver(vm, this, "WeakSet.prototype.delete")?;
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    let Some(key) = weak_collection_key(vm, &value) else {
        return Ok(Value::Bool(false));
    };
    Ok(Value::Bool(vm.heap.with_obj(index.0, |object| {
        let HeapObj::WeakSet(set) = object else {
            return false;
        };
        set.items.lock().remove(&key)
    })))
}

// --- WeakRef --------------------------------------------------------------

fn can_be_held_weakly(vm: &Vm, target: &Value) -> bool {
    match target {
        Value::Object(_) => true,
        Value::Symbol(id) => !vm.registered_symbol_ids.contains(id),
        _ => false,
    }
}

pub(crate) fn weak_ref_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err("WeakRef constructor requires new"));
    }
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !can_be_held_weakly(vm, &target) {
        return Err(Error::type_err("WeakRef target cannot be held weakly"));
    }

    let proto = native_constructor_prototype_with_default(vm, "WeakRef", vm.object_proto.clone())?;
    vm.try_reserve_value_roots(std::slice::from_ref(&proto))?;
    let proto_pin = vm.pin(&proto);
    let weak_ref = vm.alloc(HeapObj::WeakRef(crate::value::WeakRefData {
        target: Mutex::new(Some(target.clone())),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(proto)),
        extensible: AtomicBool::new(true),
    }));
    vm.unpin_many(proto_pin);
    let weak_ref = weak_ref?;
    vm.keep_during_job(&target)?;
    Ok(Value::Object(weak_ref))
}

pub(crate) fn weak_ref_deref(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let Some(Value::Object(idx)) = this else {
        return Err(Error::type_err(
            "WeakRef.prototype.deref called on incompatible receiver",
        ));
    };
    let target = vm.heap.with_obj(idx.0, |obj| match obj {
        HeapObj::WeakRef(weak_ref) => Some(weak_ref.target.lock().clone()),
        _ => None,
    });
    let Some(target) = target else {
        return Err(Error::type_err(
            "WeakRef.prototype.deref called on incompatible receiver",
        ));
    };
    if let Some(target) = target {
        vm.keep_during_job(&target)?;
        Ok(target)
    } else {
        Ok(Value::Undefined)
    }
}

// --- FinalizationRegistry -------------------------------------------------

fn require_finalization_registry(vm: &Vm, this: Option<Value>, name: &str) -> error::Result<GcIdx> {
    let Some(Value::Object(idx)) = this else {
        return Err(Error::type_err(format!(
            "FinalizationRegistry.prototype.{name} called on incompatible receiver"
        )));
    };
    if vm
        .heap
        .with_obj(idx.0, |obj| matches!(obj, HeapObj::FinalizationRegistry(_)))
    {
        Ok(idx)
    } else {
        Err(Error::type_err(format!(
            "FinalizationRegistry.prototype.{name} called on incompatible receiver"
        )))
    }
}

pub(crate) fn finalization_registry_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err(
            "FinalizationRegistry constructor requires new",
        ));
    }
    let cleanup_callback = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&cleanup_callback, &vm.heap) {
        return Err(Error::type_err(
            "FinalizationRegistry cleanup callback is not callable",
        ));
    }
    let proto = native_constructor_prototype_with_default(
        vm,
        "FinalizationRegistry",
        vm.object_proto.clone(),
    )?;
    let realm = vm.current_realm_global_env();
    vm.try_reserve_value_roots(std::slice::from_ref(&proto))?;
    let proto_pin = vm.pin(&proto);
    let registry = vm.alloc(HeapObj::FinalizationRegistry(
        crate::value::FinalizationRegistryData {
            cleanup_callback,
            realm,
            cells: Mutex::new(Vec::new()),
            cleanup_pending: AtomicBool::new(false),
            cleanup_scheduled: AtomicBool::new(false),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
            extensible: AtomicBool::new(true),
        },
    ));
    vm.unpin_many(proto_pin);
    Ok(Value::Object(registry?))
}

pub(crate) fn finalization_registry_register(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let registry = require_finalization_registry(vm, this, "register")?;
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !can_be_held_weakly(vm, &target) {
        return Err(Error::type_err(
            "FinalizationRegistry target cannot be held weakly",
        ));
    }
    let held_value = args.get(1).cloned().unwrap_or(Value::Undefined);
    if target == held_value {
        return Err(Error::type_err(
            "FinalizationRegistry target and held value must differ",
        ));
    }
    let unregister_token = match args.get(2).cloned().unwrap_or(Value::Undefined) {
        Value::Undefined => None,
        token if can_be_held_weakly(vm, &token) => Some(token),
        _ => {
            return Err(Error::type_err(
                "FinalizationRegistry unregister token cannot be held weakly",
            ))
        }
    };
    #[cfg(test)]
    if std::mem::take(&mut vm.fail_next_finalization_cell_reservation) {
        return Err(Error::range(
            "FinalizationRegistry cell storage is too large",
        ));
    }
    vm.consume_fuel()?;
    vm.heap.with_obj(registry.0, |obj| {
        let HeapObj::FinalizationRegistry(registry) = obj else {
            return Err(Error::internal(
                "FinalizationRegistry target lost its internal data",
            ));
        };
        let mut cells = registry.cells.lock();
        if cells.len() >= super::call_arguments::MAX_MATERIALIZED_CALL_ARGUMENTS {
            return Err(Error::range(
                "FinalizationRegistry cell storage is too large",
            ));
        }
        cells
            .try_reserve(1)
            .map_err(|_| Error::range("FinalizationRegistry cell storage is too large"))?;
        cells.push(crate::value::FinalizationRegistryCell {
            target: Some(target),
            held_value,
            unregister_token,
        });
        Ok(())
    })?;
    Ok(Value::Undefined)
}

pub(crate) fn finalization_registry_unregister(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let registry = require_finalization_registry(vm, this, "unregister")?;
    let token = args.first().cloned().unwrap_or(Value::Undefined);
    if !can_be_held_weakly(vm, &token) {
        return Err(Error::type_err(
            "FinalizationRegistry unregister token cannot be held weakly",
        ));
    }
    let cell_count = vm.heap.with_obj(registry.0, |obj| {
        let HeapObj::FinalizationRegistry(registry) = obj else {
            return 0;
        };
        registry.cells.lock().len()
    });
    vm.consume_fuel_units(cell_count.min(i64::MAX as usize) as i64)?;
    let removed = vm.heap.with_obj(registry.0, |obj| {
        let HeapObj::FinalizationRegistry(registry) = obj else {
            return false;
        };
        let mut cells = registry.cells.lock();
        let old_len = cells.len();
        cells.retain(|cell| cell.unregister_token.as_ref() != Some(&token));
        registry.cleanup_pending.store(
            cells.iter().any(|cell| cell.target.is_none()),
            std::sync::atomic::Ordering::Relaxed,
        );
        cells.len() != old_len
    });
    Ok(Value::Bool(removed))
}

pub(crate) fn run_finalization_registry_cleanup_job(
    vm: &mut Vm,
    registry: GcIdx,
) -> error::Result<bool> {
    let registry_value = Value::Object(registry);
    vm.try_reserve_value_roots(std::slice::from_ref(&registry_value))?;
    let registry_pin = vm.pin(&registry_value);
    let realm = vm.heap.with_obj(registry.0, |obj| {
        let HeapObj::FinalizationRegistry(registry) = obj else {
            return None;
        };
        Some(registry.realm)
    });
    let Some(realm) = realm else {
        vm.unpin_many(registry_pin);
        return Ok(true);
    };
    let result = vm.with_execution_context(
        crate::vm::ExecutionContext {
            realm_env: realm,
            kind: crate::vm::ExecutionContextKind::Job,
        },
        |vm| {
            vm.heap.with_obj(registry.0, |obj| {
                if let HeapObj::FinalizationRegistry(registry) = obj {
                    registry
                        .cleanup_scheduled
                        .store(false, std::sync::atomic::Ordering::Relaxed);
                }
            });

            loop {
                let Some((callback, pending_index, held_value, cell_count)) =
                    vm.heap.with_obj(registry.0, |obj| {
                        let HeapObj::FinalizationRegistry(registry) = obj else {
                            return None;
                        };
                        let cells = registry.cells.lock();
                        cells
                            .iter()
                            .enumerate()
                            .find(|(_, cell)| cell.target.is_none())
                            .map(|(index, cell)| {
                                (
                                    registry.cleanup_callback.clone(),
                                    index,
                                    cell.held_value.clone(),
                                    cells.len(),
                                )
                            })
                    })
                else {
                    vm.heap.with_obj(registry.0, |obj| {
                        if let HeapObj::FinalizationRegistry(registry) = obj {
                            registry
                                .cleanup_pending
                                .store(false, std::sync::atomic::Ordering::Relaxed);
                        }
                    });
                    return Ok(true);
                };

                vm.consume_fuel_units(cell_count.min(i64::MAX as usize) as i64)?;
                #[cfg(test)]
                if std::mem::take(&mut vm.fail_next_finalization_cleanup_reservation) {
                    return Err(Error::range(
                        "FinalizationRegistry cleanup storage is too large",
                    ));
                }
                vm.try_reserve_value_roots(&[
                    callback.clone(),
                    held_value.clone(),
                    callback.clone(),
                    held_value.clone(),
                ])?;
                let cleanup_pins = vm.pin(&callback) + vm.pin(&held_value);
                let removed = vm.heap.with_obj(registry.0, |obj| {
                    let HeapObj::FinalizationRegistry(registry) = obj else {
                        return false;
                    };
                    let mut cells = registry.cells.lock();
                    if cells
                        .get(pending_index)
                        .is_some_and(|cell| cell.target.is_none())
                    {
                        cells.remove(pending_index);
                        registry.cleanup_pending.store(
                            cells.iter().any(|cell| cell.target.is_none()),
                            std::sync::atomic::Ordering::Relaxed,
                        );
                        true
                    } else {
                        false
                    }
                });
                let callback_result = if removed {
                    Some(vm.call_function_with_reserved_roots(
                        &callback,
                        std::slice::from_ref(&held_value),
                        Some(Value::Undefined),
                    ))
                } else {
                    None
                };
                vm.unpin_many(cleanup_pins);
                if let Some(Err(error)) = callback_result {
                    if error.catchable() {
                        return Ok(false);
                    }
                    return Err(error);
                }
            }
        },
    );
    vm.unpin_many(registry_pin);
    result
}

pub(crate) fn map_clear(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let idx = require_map_receiver(vm, this, "Map.prototype.clear")?;
    vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::Map(m) = obj {
            m.entries.lock().clear();
        }
    });
    Ok(Value::Undefined)
}
pub(crate) fn map_size(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let Some(Value::Object(idx)) = this else {
        return Err(Error::type_err(
            "Map.prototype.size getter called on non-Map".to_string(),
        ));
    };
    vm.heap.with_obj_read(idx.0, |obj| {
        if let HeapObj::Map(m) = obj {
            Ok(Value::Number(m.entries.lock().len() as f64))
        } else {
            Err(Error::type_err(
                "Map.prototype.size getter called on non-Map".to_string(),
            ))
        }
    })
}
/// Collect Map entries as [key, value] arrays.
pub(crate) fn map_entries_list(vm: &mut Vm, this: &Option<Value>) -> error::Result<Vec<Value>> {
    let idx = require_map_receiver(vm, this.clone(), "Map.prototype.entries")?;
    let pairs: Vec<(Value, Value)> = vm.heap.with_obj_read(idx.0, |obj| {
        if let HeapObj::Map(m) = obj {
            m.entries
                .lock()
                .iter()
                .map(|(k, v)| (k.0.clone(), v.clone()))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    });
    let mut out = Vec::with_capacity(pairs.len());
    for (k, v) in pairs {
        out.push(make_value_array(vm, vec![k, v])?);
    }
    Ok(out)
}
pub(crate) fn map_entries(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let idx = require_map_receiver(vm, this, "Map.prototype.entries")?;
    new_collection_iterator(vm, Value::Object(idx), CollectionIteratorKind::MapEntries)
}
pub(crate) fn map_keys(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let idx = require_map_receiver(vm, this, "Map.prototype.keys")?;
    new_collection_iterator(vm, Value::Object(idx), CollectionIteratorKind::MapKeys)
}
pub(crate) fn map_values(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let idx = require_map_receiver(vm, this, "Map.prototype.values")?;
    new_collection_iterator(vm, Value::Object(idx), CollectionIteratorKind::MapValues)
}
pub(crate) fn map_for_each(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    let this_arg = args.get(1).cloned();
    let idx = require_map_receiver(vm, this.clone(), "Map.prototype.forEach")?;
    if !is_callable(&cb, &vm.heap) {
        return Err(Error::type_err(
            "Map.prototype.forEach callback is not callable",
        ));
    }
    let mut queue = map_keys_in_order(vm, idx);
    let mut cursor = 0;
    let mut last_yielded: Option<MapKey> = None;
    while cursor < queue.len() {
        let key = queue[cursor].clone();
        cursor += 1;
        let value = vm.heap.with_obj_read(idx.0, |obj| {
            if let HeapObj::Map(map) = obj {
                map.entries.lock().get(&key).cloned()
            } else {
                None
            }
        });
        let Some(value) = value else {
            extend_collection_visit_queue(
                &mut queue,
                cursor,
                &map_keys_in_order(vm, idx),
                last_yielded.as_ref(),
            );
            continue;
        };
        let key_value = key.0.clone();
        vm.call_function(
            &cb,
            &[value, key_value, this.clone().unwrap_or(Value::Undefined)],
            this_arg.clone(),
        )?;
        last_yielded = Some(key);
        extend_collection_visit_queue(
            &mut queue,
            cursor,
            &map_keys_in_order(vm, idx),
            last_yielded.as_ref(),
        );
    }
    Ok(Value::Undefined)
}
pub(crate) fn map_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err("Map constructor must be called with new"));
    }
    let realm = vm.current_realm_global_env();
    let iterable = args.first().cloned().unwrap_or(Value::Undefined);
    let fallback = vm.map_prototype_for_env(realm);
    let proto = native_constructor_prototype_with_default(vm, "Map", fallback)?;
    let initial_roots = Vm::value_root_count(&proto)
        .checked_add(Vm::value_root_count(&iterable))
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| Error::range("Map temporary root set is too large"))?;
    reserve_map_root_slots(
        vm,
        initial_roots,
        #[cfg(test)]
        crate::vm::MapReservationSite::ConstructorRoots,
    )?;
    let mut initial_pins = vm.pin_many(&[proto.clone(), iterable.clone()]);
    let allocation = vm.alloc(HeapObj::Map(MapData {
        entries: Mutex::new(IndexMap::new()),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(proto)),
        extensible: AtomicBool::new(true),
    }));
    let map = match allocation {
        Ok(index) => Value::Object(index),
        Err(error) => {
            vm.unpin_many(initial_pins);
            return Err(error);
        }
    };
    initial_pins += vm.pin(&map);

    let result = (|| -> error::Result<Value> {
        if iterable.is_nullish() {
            return Ok(map.clone());
        }

        // Adder, iterator record, and current entry/key/value are the maximum
        // values simultaneously live across observable iteration operations.
        reserve_map_root_slots(
            vm,
            6,
            #[cfg(test)]
            crate::vm::MapReservationSite::IteratorRoots,
        )?;
        let set = vm.get_property(&map, "set")?;
        if !is_callable(&set, &vm.heap) {
            return Err(Error::type_err("Map set is not callable"));
        }
        let set_pin = vm.pin(&set);
        let iterator_result = get_sync_iterator(vm, iterable.clone());
        let iterator = match iterator_result {
            Ok(iterator) => iterator,
            Err(error) => {
                vm.unpin_many(set_pin);
                return Err(error);
            }
        };
        let iterator_pins = vm.pin_many(&[iterator.iterator.clone(), iterator.next_method.clone()]);
        let iteration = (|| -> error::Result<Value> {
            loop {
                #[cfg(test)]
                if std::mem::take(&mut vm.map_constructor_zero_fuel_before_step) {
                    vm.set_fuel(Some(0));
                }
                // IteratorStepValue failures propagate without IteratorClose.
                let Some(pair) =
                    iterator_helper_step(vm, &iterator.iterator, &iterator.next_method, true)?
                else {
                    return Ok(map.clone());
                };
                if !matches!(pair, Value::Object(_)) {
                    return close_iterator_after_error_in_realm(
                        vm,
                        &iterator.iterator,
                        Error::type_err("Iterator value is not an object"),
                        realm,
                    );
                }

                let mut pair_pins = vm.pin(&pair);
                let entry_result = (|| -> error::Result<()> {
                    let key = vm.get_property(&pair, "0")?;
                    pair_pins += vm.pin(&key);
                    let value = vm.get_property(&pair, "1")?;
                    pair_pins += vm.pin(&value);
                    vm.call_function(&set, &[key, value], Some(map.clone()))?;
                    Ok(())
                })();
                vm.unpin_many(pair_pins);
                if let Err(error) = entry_result {
                    if !error.catchable() {
                        return Err(error);
                    }
                    return close_iterator_after_error_in_realm(
                        vm,
                        &iterator.iterator,
                        error,
                        realm,
                    );
                }
            }
        })();
        vm.unpin_many(iterator_pins);
        vm.unpin_many(set_pin);
        iteration
    })();
    vm.unpin_many(initial_pins);
    result
}

// =========================================================================
// Set
// =========================================================================
fn require_set_receiver(vm: &Vm, this: Option<Value>, name: &str) -> error::Result<GcIdx> {
    let Some(Value::Object(idx)) = this else {
        return Err(Error::type_err(format!("{name} called on non-Set")));
    };
    if vm
        .heap
        .with_obj(idx.0, |obj| matches!(obj, HeapObj::Set(_)))
    {
        Ok(idx)
    } else {
        Err(Error::type_err(format!("{name} called on non-Set")))
    }
}

pub(crate) fn set_add(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = args.first().cloned().unwrap_or(Value::Undefined);
    let idx = require_set_receiver(vm, this.clone(), "Set.prototype.add")?;
    insert_set_entry(vm, idx, val)?;
    Ok(this.unwrap_or(Value::Undefined))
}
pub(crate) fn set_has(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = args.first().cloned().unwrap_or(Value::Undefined);
    let idx = require_set_receiver(vm, this, "Set.prototype.has")?;
    Ok(Value::Bool(vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::Set(s) = obj {
            s.items.lock().contains(&MapKey::new(val))
        } else {
            false
        }
    })))
}
pub(crate) fn set_delete(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let val = args.first().cloned().unwrap_or(Value::Undefined);
    let idx = require_set_receiver(vm, this, "Set.prototype.delete")?;
    Ok(Value::Bool(delete_set_entry(vm, idx, &MapKey::new(val))?))
}
pub(crate) fn set_size(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let idx = require_set_receiver(vm, this, "Set.prototype.size getter")?;
    Ok(Value::Number(vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::Set(s) = obj {
            s.items.lock().len()
        } else {
            0
        }
    }) as f64))
}
pub(crate) fn set_clear(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let idx = require_set_receiver(vm, this, "Set.prototype.clear")?;
    let work = vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::Set(set) = obj else {
            return 0;
        };
        set.items.lock().physical_len()
    });
    consume_set_native_work(vm, work)?;
    vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::Set(s) = obj {
            s.items.lock().clear();
        }
    });
    Ok(Value::Undefined)
}
pub(crate) fn set_values_list(
    vm: &mut Vm,
    this: Option<Value>,
    name: &str,
) -> error::Result<Vec<Value>> {
    let idx = require_set_receiver(vm, this, name)?;
    Ok(vm.heap.with_obj(idx.0, |obj| {
        if let HeapObj::Set(s) = obj {
            s.items
                .lock()
                .iter()
                .map(|key| key.0.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    }))
}
pub(crate) fn set_entries(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let idx = require_set_receiver(vm, this, "Set.prototype.entries")?;
    new_collection_iterator(vm, Value::Object(idx), CollectionIteratorKind::SetEntries)
}
pub(crate) fn set_values(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let idx = require_set_receiver(vm, this, "Set.prototype.values")?;
    new_collection_iterator(vm, Value::Object(idx), CollectionIteratorKind::SetValues)
}
pub(crate) fn set_for_each(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    let callback_this = args.get(1).cloned();
    let callback_this_root = callback_this.clone().unwrap_or(Value::Undefined);
    let receiver = this.unwrap_or(Value::Undefined);
    let idx = require_set_receiver(vm, Some(receiver.clone()), "Set.prototype.forEach")?;
    if !is_callable(&cb, &vm.heap) {
        return Err(Error::type_err(
            "Set.prototype.forEach callback is not callable",
        ));
    }
    vm.try_reserve_value_roots(&[receiver.clone(), cb.clone(), callback_this_root.clone()])?;
    let pin_count = vm.pin_many(&[receiver.clone(), cb.clone(), callback_this_root]);
    let result = (|| {
        let mut queue = SetVisitQueue::new();
        while let Some(key) = queue.next(vm, idx)? {
            with_set_traversal_value(vm, key, |vm, value| {
                vm.call_function(
                    &cb,
                    &[value.clone(), value, receiver.clone()],
                    callback_this.clone(),
                )?;
                Ok(())
            })?;
        }
        Ok(Value::Undefined)
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn set_union(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    with_set_record(vm, args, this, "Set.prototype.union", |vm, index, other| {
        let (iterator, iterator_pins) = set_record_keys_iterator(vm, other)?;
        let realm = vm.current_realm_global_env();
        let operation = (|| {
            let (result, result_index, result_pin) = match copy_set_for_algebra(vm, index) {
                Ok(result) => result,
                Err(error) => {
                    return close_set_record_iterator_after_error(vm, &iterator, error, realm);
                }
            };
            let iteration = for_each_set_record_iterator_key(vm, &iterator, |vm, value| {
                insert_set_algebra_entry(vm, result_index, value)?;
                Ok(true)
            });
            vm.unpin_many(result_pin);
            iteration?;
            Ok(result)
        })();
        vm.unpin_many(iterator_pins);
        operation
    })
}

pub(crate) fn set_intersection(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    with_set_record(
        vm,
        args,
        this,
        "Set.prototype.intersection",
        |vm, index, other| {
            let (result, result_index, result_pin) = new_pinned_set_result(vm)?;
            let this_size = vm.heap.with_obj(index.0, |object| {
                let HeapObj::Set(set) = object else {
                    return 0;
                };
                set.items.lock().len()
            }) as f64;
            let operation = (|| {
                if this_size <= other.size {
                    let mut queue = SetVisitQueue::new();
                    while let Some(key) = queue.next(vm, index)? {
                        with_set_traversal_value(vm, key, |vm, value| {
                            if set_record_has(vm, other, value.clone())? {
                                insert_set_algebra_entry(vm, result_index, value)?;
                            }
                            Ok(())
                        })?;
                    }
                    Ok(())
                } else {
                    for_each_set_record_key(vm, other, |vm, value| {
                        if set_has_direct(vm, index, &value) {
                            insert_set_algebra_entry(vm, result_index, value)?;
                        }
                        Ok(true)
                    })
                }
            })();
            vm.unpin_many(result_pin);
            operation?;
            Ok(result)
        },
    )
}

pub(crate) fn set_difference(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    with_set_record(
        vm,
        args,
        this,
        "Set.prototype.difference",
        |vm, index, other| {
            let this_size = vm.heap.with_obj(index.0, |object| {
                let HeapObj::Set(set) = object else {
                    return 0;
                };
                set.items.lock().len()
            }) as f64;
            let (result, result_index, result_pin) = copy_set_for_algebra(vm, index)?;
            let operation = (|| {
                if this_size <= other.size {
                    let mut queue = SetVisitQueue::new();
                    while let Some(key) = queue.next(vm, result_index)? {
                        with_set_traversal_value(vm, key, |vm, value| {
                            if set_record_has(vm, other, value.clone())? {
                                set_delete_direct(vm, &result, &value)?;
                            }
                            Ok(())
                        })?;
                    }
                    Ok(())
                } else {
                    for_each_set_record_key(vm, other, |vm, value| {
                        set_delete_direct(vm, &result, &value)?;
                        Ok(true)
                    })
                }
            })();
            vm.unpin_many(result_pin);
            operation?;
            Ok(result)
        },
    )
}

pub(crate) fn set_symmetric_difference(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    with_set_record(
        vm,
        args,
        this,
        "Set.prototype.symmetricDifference",
        |vm, index, other| {
            let (iterator, iterator_pins) = set_record_keys_iterator(vm, other)?;
            let realm = vm.current_realm_global_env();
            let operation = (|| {
                let (result, result_index, result_pin) = match copy_set_for_algebra(vm, index) {
                    Ok(result) => result,
                    Err(error) => {
                        return close_set_record_iterator_after_error(vm, &iterator, error, realm);
                    }
                };
                let iteration = for_each_set_record_iterator_key(vm, &iterator, |vm, value| {
                    if set_has_direct(vm, index, &value) {
                        set_delete_direct(vm, &result, &value)?;
                    } else {
                        insert_set_algebra_entry(vm, result_index, value)?;
                    }
                    Ok(true)
                });
                vm.unpin_many(result_pin);
                iteration?;
                Ok(result)
            })();
            vm.unpin_many(iterator_pins);
            operation
        },
    )
}

pub(crate) fn set_is_subset_of(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    with_set_record(
        vm,
        args,
        this,
        "Set.prototype.isSubsetOf",
        |vm, index, other| {
            let this_size = vm.heap.with_obj(index.0, |object| {
                let HeapObj::Set(set) = object else {
                    return 0;
                };
                set.items.lock().len()
            }) as f64;
            if this_size > other.size {
                return Ok(Value::Bool(false));
            }
            let mut queue = SetVisitQueue::new();
            while let Some(key) = queue.next(vm, index)? {
                let present = with_set_traversal_value(vm, key, |vm, value| {
                    set_record_has(vm, other, value)
                })?;
                if !present {
                    return Ok(Value::Bool(false));
                }
            }
            Ok(Value::Bool(true))
        },
    )
}

pub(crate) fn set_is_superset_of(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    with_set_record(
        vm,
        args,
        this,
        "Set.prototype.isSupersetOf",
        |vm, index, other| {
            let this_size = vm.heap.with_obj(index.0, |object| {
                let HeapObj::Set(set) = object else {
                    return 0;
                };
                set.items.lock().len()
            }) as f64;
            if this_size < other.size {
                return Ok(Value::Bool(false));
            }
            let mut result = true;
            for_each_set_record_key(vm, other, |vm, value| {
                if !set_has_direct(vm, index, &value) {
                    result = false;
                    return Ok(false);
                }
                Ok(true)
            })?;
            Ok(Value::Bool(result))
        },
    )
}

pub(crate) fn set_is_disjoint_from(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    with_set_record(
        vm,
        args,
        this,
        "Set.prototype.isDisjointFrom",
        |vm, index, other| {
            let this_size = vm.heap.with_obj(index.0, |object| {
                let HeapObj::Set(set) = object else {
                    return 0;
                };
                set.items.lock().len()
            }) as f64;
            if this_size <= other.size {
                let mut queue = SetVisitQueue::new();
                while let Some(key) = queue.next(vm, index)? {
                    let present = with_set_traversal_value(vm, key, |vm, value| {
                        set_record_has(vm, other, value)
                    })?;
                    if present {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            } else {
                let mut result = true;
                for_each_set_record_key(vm, other, |vm, value| {
                    if set_has_direct(vm, index, &value) {
                        result = false;
                        return Ok(false);
                    }
                    Ok(true)
                })?;
                Ok(Value::Bool(result))
            }
        },
    )
}

pub(crate) fn set_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err("Set constructor must be called with new"));
    }
    let realm = vm.current_realm_global_env();
    let iterable = args.first().cloned().unwrap_or(Value::Undefined);
    let fallback = vm.set_prototype_for_env(realm);
    let proto = native_constructor_prototype_with_default(vm, "Set", fallback)?;
    let initial_roots = Vm::value_root_count(&proto)
        .checked_add(Vm::value_root_count(&iterable))
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| Error::range("Set temporary root set is too large"))?;
    reserve_set_root_slots(
        vm,
        initial_roots,
        #[cfg(test)]
        crate::vm::SetReservationSite::ConstructorRoots,
    )?;
    let mut initial_pins = vm.pin_many(&[proto.clone(), iterable.clone()]);
    #[cfg(test)]
    if std::mem::take(&mut vm.set_constructor_garbage_before_allocation) {
        while vm.max_heap_objects > 0 && vm.heap.live_count() < vm.max_heap_objects {
            if let Err(error) = vm.new_object() {
                vm.unpin_many(initial_pins);
                return Err(error);
            }
        }
        vm.set_constructor_live_before_allocation = Some(vm.heap.live_count());
    }
    let allocation = vm.alloc(HeapObj::Set(SetData {
        items: Mutex::new(crate::value::SetStorage::new()),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(proto)),
        extensible: AtomicBool::new(true),
    }));
    let set = match allocation {
        Ok(index) => Value::Object(index),
        Err(error) => {
            vm.unpin_many(initial_pins);
            return Err(error);
        }
    };
    initial_pins += vm.pin(&set);

    let result = (|| -> error::Result<Value> {
        if iterable.is_nullish() {
            return Ok(set.clone());
        }

        reserve_set_root_slots(
            vm,
            4,
            #[cfg(test)]
            crate::vm::SetReservationSite::IteratorRoots,
        )?;
        let add = vm.get_property(&set, "add")?;
        if !is_callable(&add, &vm.heap) {
            return Err(Error::type_err("Set add is not callable"));
        }
        let add_pin = vm.pin(&add);
        let iterator_result = get_sync_iterator(vm, iterable.clone());
        let iterator = match iterator_result {
            Ok(iterator) => iterator,
            Err(error) => {
                vm.unpin_many(add_pin);
                return Err(error);
            }
        };
        let iterator_pins = vm.pin_many(&[iterator.iterator.clone(), iterator.next_method.clone()]);
        let iteration = (|| -> error::Result<Value> {
            loop {
                #[cfg(test)]
                if std::mem::take(&mut vm.set_constructor_zero_fuel_before_step) {
                    vm.set_fuel(Some(0));
                }
                let Some(value) =
                    iterator_helper_step(vm, &iterator.iterator, &iterator.next_method, true)?
                else {
                    return Ok(set.clone());
                };
                let value_pin = vm.pin(&value);
                let add_result = vm.call_function(&add, &[value], Some(set.clone()));
                vm.unpin_many(value_pin);
                if let Err(error) = add_result {
                    if !error.catchable() {
                        return Err(error);
                    }
                    return close_iterator_after_error_in_realm(
                        vm,
                        &iterator.iterator,
                        error,
                        realm,
                    );
                }
            }
        })();
        vm.unpin_many(iterator_pins);
        vm.unpin_many(add_pin);
        iteration
    })();
    vm.unpin_many(initial_pins);
    result
}

// =========================================================================
// Symbol
// =========================================================================
pub(crate) fn symbol_constructor(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_some() {
        return Err(Error::type_err("Symbol is not a constructor"));
    }
    let desc = match args.first().unwrap_or(&Value::Undefined) {
        Value::Undefined => None,
        value => Some(vm.to_string(value)?),
    };
    let id = vm.next_symbol_id;
    vm.next_symbol_id += 1;
    vm.symbol_descriptions.insert(id, desc);
    Ok(Value::Symbol(id))
}
pub(crate) fn symbol_for(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let key = vm.to_string(args.first().unwrap_or(&Value::Undefined))?;
    if let Some(id) = vm.symbol_registry.get(&key) {
        return Ok(Value::Symbol(*id));
    }
    let id = vm.next_symbol_id;
    vm.next_symbol_id += 1;
    vm.symbol_descriptions.insert(id, Some(key.clone()));
    vm.symbol_registry.insert(key, id);
    vm.registered_symbol_ids.insert(id);
    Ok(Value::Symbol(id))
}

pub(crate) fn symbol_key_for(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let symbol = match args.first().unwrap_or(&Value::Undefined) {
        Value::Symbol(id) => *id,
        _ => return Err(Error::type_err("Symbol.keyFor requires a symbol")),
    };
    for (key, id) in &vm.symbol_registry {
        if *id == symbol {
            return Ok(Value::String(key.clone()));
        }
    }
    Ok(Value::Undefined)
}

fn this_symbol_value(vm: &Vm, this: Option<Value>) -> error::Result<u32> {
    match this.unwrap_or(Value::Undefined) {
        Value::Symbol(id) => Ok(id),
        Value::Object(idx) => {
            let primitive = vm.heap.with_obj(idx.0, |obj| match obj {
                HeapObj::Object(data) => data.primitive.lock().clone(),
                _ => None,
            });
            if let Some(Value::Symbol(id)) = primitive {
                Ok(id)
            } else {
                Err(Error::type_err("Symbol method called on non-symbol"))
            }
        }
        _ => Err(Error::type_err("Symbol method called on non-symbol")),
    }
}

pub(crate) fn symbol_description_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let id = this_symbol_value(vm, this)?;
    Ok(vm
        .symbol_descriptions
        .get(&id)
        .cloned()
        .flatten()
        .map(Value::String)
        .unwrap_or(Value::Undefined))
}

pub(crate) fn symbol_value_of(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::Symbol(this_symbol_value(vm, this)?))
}

pub(crate) fn symbol_to_primitive(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::Symbol(this_symbol_value(vm, this)?))
}

pub(crate) fn symbol_to_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let id = this_symbol_value(vm, this)?;
    let desc = vm.symbol_descriptions.get(&id).and_then(|d| d.as_ref());
    Ok(Value::String(Arc::from(match desc {
        Some(desc) => format!("Symbol({desc})"),
        None => "Symbol()".to_string(),
    })))
}

// =========================================================================
// Extended setup 2: Map/Set/Symbol
// =========================================================================

// =========================================================================
// Promise
// =========================================================================
pub(crate) fn promise_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Err(Error::type_err(
            "Promise constructor must be called with new",
        ));
    }
    let executor = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&executor, &vm.heap) {
        return Err(Error::type_err("Promise resolver is not a function"));
    }
    let fallback = vm.current_realm_promise_prototype();
    let proto = native_constructor_prototype_with_default(vm, "Promise", fallback)?;
    // The observable prototype and fresh Promise live only in Rust locals at
    // these allocation boundaries, so keep both in the collector root set.
    let proto_pin = vm.pin(&proto);
    let p_idx = vm.alloc(HeapObj::Promise(crate::value::PromiseData {
        state: Mutex::new(crate::value::PromiseStatus::Pending),
        result: Mutex::new(Value::Undefined),
        handlers: Mutex::new(Vec::new()),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(proto)),
        extensible: AtomicBool::new(true),
    }));
    vm.unpin(proto_pin);
    let p_val = Value::Object(p_idx?);
    let promise_pin = vm.pin(&p_val);
    let resolving_functions = create_promise_resolving_functions(vm, p_val.clone());
    vm.unpin(promise_pin);
    let (resolve_fn, reject_fn) = resolving_functions?;
    let executor_pins = vm.pin_many(&[p_val.clone(), resolve_fn.clone(), reject_fn.clone()]);
    let realm = vm.current_realm_global_env();
    let result = match vm.call_function(
        &executor,
        &[resolve_fn, reject_fn.clone()],
        Some(Value::Undefined),
    ) {
        Ok(_) => Ok(()),
        Err(e) => {
            // executor threw: reject the promise with the thrown value
            match vm.promise_rejection_reason_in_realm(&e, realm) {
                Ok(reason) => vm
                    .call_function(
                        &reject_fn,
                        std::slice::from_ref(&reason),
                        Some(Value::Undefined),
                    )
                    .map(|_| ()),
                Err(error) => Err(error),
            }
        }
    };
    vm.unpin_many(executor_pins);
    result?;
    Ok(p_val)
}

fn create_bound_native_function(
    vm: &mut Vm,
    name: &str,
    target_name: &str,
    func: NativeFn,
    length: usize,
    this_val: Value,
) -> error::Result<Value> {
    let realm = vm.current_realm_global_env();
    create_bound_native_function_in_env(vm, name, target_name, func, length, this_val, realm)
}

fn create_bound_native_function_in_env(
    vm: &mut Vm,
    name: &str,
    target_name: &str,
    func: NativeFn,
    length: usize,
    this_val: Value,
    realm: GcIdx,
) -> error::Result<Value> {
    let realm = env::global_env_root(&vm.heap, realm);
    // The bound receiver is the closure record for these internal functions.
    // Root it before allocating the native target, since that allocation may
    // trigger a collection before the bound function starts tracing it.
    let this_pin = vm.pin(&this_val);
    let target = match vm.new_native_function_in_env(target_name, func, length, realm) {
        Ok(target) => target,
        Err(error) => {
            vm.unpin(this_pin);
            return Err(error);
        }
    };
    let target_val = Value::Object(target);
    let target_pin = vm.pin(&target_val);
    let constructable = vm.is_constructor_value(&target_val);
    let function_proto = vm
        .realm_function_prototypes
        .get(&realm.0)
        .cloned()
        .unwrap_or_else(|| vm.function_proto.clone());
    let idx = vm.heap.allocate(HeapObj::Function(FunctionData {
        name: Some(Arc::from(name)),
        kind: FunctionKind::Bound {
            target,
            this_val,
            bound_args: Vec::new(),
            constructable,
        },
        closure: realm,
        lexical_new_target: Value::Undefined,
        home_object: Mutex::new(None),
        is_class_ctor: AtomicBool::new(false),
        prototype: Mutex::new(None),
        proto: Mutex::new(match function_proto {
            Value::Object(_) => Some(function_proto),
            _ => None,
        }),
        props: Mutex::new(builtin_function_own_props(name, length)),
        extensible: AtomicBool::new(true),
        private_fields: Mutex::new(std::collections::HashMap::new()),
    }));
    vm.unpin(target_pin);
    vm.unpin(this_pin);
    let idx = idx?;
    Ok(Value::Object(GcIdx(idx)))
}

fn create_promise_resolving_functions(
    vm: &mut Vm,
    promise: Value,
) -> error::Result<(Value, Value)> {
    let realm = vm.current_realm_global_env();
    create_promise_resolving_functions_in_env(vm, promise, realm)
}

fn create_promise_resolving_functions_in_env(
    vm: &mut Vm,
    promise: Value,
    realm: GcIdx,
) -> error::Result<(Value, Value)> {
    let realm = env::global_env_root(&vm.heap, realm);
    let state_idx = vm.new_object_in_env(realm)?;
    let state = Value::Object(state_idx);
    vm.heap.with_obj(state_idx.0, |object| {
        let mut props = object.props().lock();
        props.insert(
            PropertyKey::from("promise"),
            PropertyDescriptor::data(promise.clone()),
        );
        props.insert(
            PropertyKey::from("alreadyResolved"),
            PropertyDescriptor::data(Value::Bool(false)),
        );
    });
    let pins = vm.pin_many(&[promise, state.clone()]);
    let resolve =
        create_bound_native_function_in_env(vm, "", "", promise_resolve, 1, state.clone(), realm);
    let resolve = match resolve {
        Ok(resolve) => resolve,
        Err(error) => {
            vm.unpin_many(pins);
            return Err(error);
        }
    };
    let resolve_pin = vm.pin(&resolve);
    let reject = create_bound_native_function_in_env(vm, "", "", promise_reject, 1, state, realm);
    vm.unpin(resolve_pin);
    vm.unpin_many(pins);
    Ok((resolve, reject?))
}

struct PromiseResolvingTarget {
    promise: usize,
    state: GcIdx,
}

fn take_promise_resolving_target(vm: &Vm, this: Option<Value>) -> Option<PromiseResolvingTarget> {
    let Some(Value::Object(state)) = this else {
        return None;
    };
    vm.heap.with_obj(state.0, |object| {
        let mut props = object.props().lock();
        let already_resolved = props
            .get(&PropertyKey::from("alreadyResolved"))
            .is_some_and(|descriptor| descriptor.value == Value::Bool(true));
        if already_resolved {
            return None;
        }
        if let Some(descriptor) = props.get_mut(&PropertyKey::from("alreadyResolved")) {
            descriptor.value = Value::Bool(true);
        }
        let promise = props
            .get(&PropertyKey::from("promise"))
            .and_then(|descriptor| match descriptor.value {
                Value::Object(promise) => Some(promise.0),
                _ => None,
            })?;
        Some(PromiseResolvingTarget { promise, state })
    })
}

fn restore_promise_resolving_target(vm: &Vm, target: &PromiseResolvingTarget) {
    let pending = vm.heap.with_obj(target.promise, |object| {
        matches!(object, HeapObj::Promise(promise) if *promise.state.lock() == crate::value::PromiseStatus::Pending)
    });
    if !pending {
        return;
    }
    vm.heap.with_obj(target.state.0, |object| {
        if let Some(descriptor) = object
            .props()
            .lock()
            .get_mut(&PropertyKey::from("alreadyResolved"))
        {
            descriptor.value = Value::Bool(false);
        }
    });
}

pub(crate) struct PromiseCapability {
    pub(crate) promise: Value,
    pub(crate) resolve: Value,
    pub(crate) reject: Value,
}

pub(crate) fn promise_capability_executor(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let capability_obj = match this {
        Some(Value::Object(idx)) => idx,
        _ => return Err(Error::type_err("Promise capability executor receiver")),
    };
    let resolve = args.first().cloned().unwrap_or(Value::Undefined);
    let reject = args.get(1).cloned().unwrap_or(Value::Undefined);
    vm.heap.with_obj(capability_obj.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        let existing_resolve = props
            .get(&PropertyKey::from("resolve"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let existing_reject = props
            .get(&PropertyKey::from("reject"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        if !existing_resolve.is_undefined() || !existing_reject.is_undefined() {
            return Err(Error::type_err("Promise capability already resolved"));
        }
        props.insert(
            PropertyKey::from("resolve"),
            PropertyDescriptor::data(resolve),
        );
        props.insert(
            PropertyKey::from("reject"),
            PropertyDescriptor::data(reject),
        );
        Ok(Value::Undefined)
    })
}

pub(crate) fn new_promise_capability(vm: &mut Vm, ctor: Value) -> error::Result<PromiseCapability> {
    let realm = vm.current_realm_global_env();
    new_promise_capability_in_env(vm, ctor, realm)
}

pub(crate) fn new_promise_capability_in_env(
    vm: &mut Vm,
    ctor: Value,
    realm: GcIdx,
) -> error::Result<PromiseCapability> {
    if !vm.is_constructor_value(&ctor) {
        return Err(Error::type_err(
            "Promise capability receiver is not a constructor",
        ));
    }

    let capability_idx = vm.new_object()?;
    let capability = Value::Object(capability_idx);
    let executor = create_bound_native_function_in_env(
        vm,
        "",
        "",
        promise_capability_executor,
        2,
        capability.clone(),
        realm,
    )?;
    let pins = vm.pin_many(&[ctor.clone(), capability.clone(), executor.clone()]);
    let promise_result = vm.construct(&ctor, std::slice::from_ref(&executor));
    let promise = match promise_result {
        Ok(promise) => promise,
        Err(err) => {
            vm.unpin_many(pins);
            return Err(err);
        }
    };
    let (resolve, reject) = vm.heap.with_obj(capability_idx.0, |obj| {
        let props = obj.props();
        let props = props.lock();
        let resolve = props
            .get(&PropertyKey::from("resolve"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let reject = props
            .get(&PropertyKey::from("reject"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        (resolve, reject)
    });
    vm.unpin_many(pins);

    if !is_callable(&resolve, &vm.heap) || !is_callable(&reject, &vm.heap) {
        return Err(Error::type_err(
            "Promise capability functions are not callable",
        ));
    }
    Ok(PromiseCapability {
        promise,
        resolve,
        reject,
    })
}

pub(crate) fn promise_resolve(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let target = match take_promise_resolving_target(vm, this) {
        Some(target) => target,
        None => return Ok(Value::Undefined),
    };
    let mut continuation_owns_resolution = false;
    let result = promise_resolve_once(vm, args, target.promise, &mut continuation_owns_resolution);
    if !continuation_owns_resolution && result.as_ref().is_err_and(|error| !error.catchable()) {
        restore_promise_resolving_target(vm, &target);
    }
    result
}

fn preserve_promise_resolution_continuation(
    vm: &mut Vm,
    task: crate::vm::Microtask,
    result: error::Result<()>,
    continuation_owns_resolution: &mut bool,
) -> error::Result<()> {
    if result.as_ref().is_err_and(|error| !error.catchable()) {
        vm.microtask_queue.push_front(task);
        *continuation_owns_resolution = true;
    }
    result
}

fn promise_resolve_once(
    vm: &mut Vm,
    args: &[Value],
    p_idx: usize,
    continuation_owns_resolution: &mut bool,
) -> error::Result<Value> {
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    let promise_realm = vm.current_realm_global_env();
    if value == Value::Object(GcIdx(p_idx)) {
        let error = Error::type_err("A Promise cannot resolve to itself");
        let reason = vm.make_error_value(&error)?;
        let settlement = vm.promise_reject(p_idx, reason.clone());
        preserve_promise_resolution_continuation(
            vm,
            crate::vm::Microtask::RejectInRealm {
                promise: GcIdx(p_idx),
                reason,
                realm: promise_realm,
            },
            settlement,
            continuation_owns_resolution,
        )?;
        return Ok(Value::Undefined);
    }
    if matches!(value, Value::Object(_)) {
        let pins = vm.pin_many(&[Value::Object(GcIdx(p_idx)), value.clone()]);
        let then = match vm.get_property(&value, "then") {
            Ok(then) => then,
            Err(error) => {
                let reason = match vm.promise_rejection_reason_in_realm(&error, promise_realm) {
                    Ok(reason) => reason,
                    Err(error) => {
                        vm.unpin_many(pins);
                        return Err(error);
                    }
                };
                let settlement = vm.promise_reject(p_idx, reason.clone());
                let settlement = preserve_promise_resolution_continuation(
                    vm,
                    crate::vm::Microtask::RejectInRealm {
                        promise: GcIdx(p_idx),
                        reason,
                        realm: promise_realm,
                    },
                    settlement,
                    continuation_owns_resolution,
                );
                vm.unpin_many(pins);
                settlement?;
                return Ok(Value::Undefined);
            }
        };
        let resolution =
            continue_promise_resolution_after_then(vm, p_idx, value, then, promise_realm);
        let resolution = match resolution {
            Ok(()) => Ok(()),
            Err(abort) => {
                vm.microtask_queue.push_front(abort.continuation);
                *continuation_owns_resolution = true;
                Err(abort.error)
            }
        };
        vm.unpin_many(pins);
        resolution?;
        return Ok(Value::Undefined);
    }
    let settlement = vm.promise_resolve(p_idx, value.clone());
    preserve_promise_resolution_continuation(
        vm,
        crate::vm::Microtask::ResolveInRealm {
            promise: GcIdx(p_idx),
            value,
            realm: promise_realm,
        },
        settlement,
        continuation_owns_resolution,
    )?;
    Ok(Value::Undefined)
}

pub(crate) struct PromiseResolutionAbort {
    pub(crate) error: std::sync::Arc<Error>,
    pub(crate) continuation: crate::vm::Microtask,
}

pub(crate) fn continue_promise_resolution_after_then(
    vm: &mut Vm,
    p_idx: usize,
    resolution: Value,
    then: Value,
    promise_realm: GcIdx,
) -> Result<(), Box<PromiseResolutionAbort>> {
    let pins = vm.pin_many(&[
        Value::Object(GcIdx(p_idx)),
        resolution.clone(),
        then.clone(),
    ]);
    let retry_after_then = || crate::vm::Microtask::PromiseResolveAfterThen {
        promise: GcIdx(p_idx),
        resolution: resolution.clone(),
        then: then.clone(),
        realm: promise_realm,
    };
    let result = if is_callable(&then, &vm.heap) {
        match vm.constructor_realm_or_fallback(&then, promise_realm) {
            Ok(then_realm) => {
                match create_promise_resolving_functions_in_env(
                    vm,
                    Value::Object(GcIdx(p_idx)),
                    then_realm,
                ) {
                    Ok((resolve, reject)) => {
                        vm.microtask_queue
                            .push_back(crate::vm::Microtask::Thenable {
                                thenable: resolution,
                                then,
                                resolve,
                                reject,
                                realm: then_realm,
                            });
                        Ok(())
                    }
                    Err(error) => match vm.promise_rejection_reason_in_realm(&error, then_realm) {
                        Ok(reason) => {
                            match vm.promise_reject_in_realm(p_idx, reason.clone(), then_realm) {
                                Ok(()) => Ok(()),
                                Err(error) => Err(Box::new(PromiseResolutionAbort {
                                    error,
                                    continuation: crate::vm::Microtask::RejectInRealm {
                                        promise: GcIdx(p_idx),
                                        reason,
                                        realm: then_realm,
                                    },
                                })),
                            }
                        }
                        Err(error) => Err(Box::new(PromiseResolutionAbort {
                            error,
                            continuation: retry_after_then(),
                        })),
                    },
                }
            }
            Err(error) => Err(Box::new(PromiseResolutionAbort {
                error,
                continuation: retry_after_then(),
            })),
        }
    } else {
        match vm.promise_resolve_in_realm(p_idx, resolution.clone(), promise_realm) {
            Ok(()) => Ok(()),
            Err(error) => Err(Box::new(PromiseResolutionAbort {
                error,
                continuation: crate::vm::Microtask::ResolveInRealm {
                    promise: GcIdx(p_idx),
                    value: resolution.clone(),
                    realm: promise_realm,
                },
            })),
        }
    };
    vm.unpin_many(pins);
    result
}

pub(crate) fn promise_reject(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let target = match take_promise_resolving_target(vm, this) {
        Some(target) => target,
        None => return Ok(Value::Undefined),
    };
    let reason = args.first().cloned().unwrap_or(Value::Undefined);
    let promise_realm = vm.current_realm_global_env();
    let result = vm
        .promise_reject_in_realm(target.promise, reason.clone(), promise_realm)
        .map(|()| Value::Undefined);
    if result.as_ref().is_err_and(|error| !error.catchable()) {
        vm.microtask_queue
            .push_front(crate::vm::Microtask::RejectInRealm {
                promise: GcIdx(target.promise),
                reason,
                realm: promise_realm,
            });
    }
    result
}

fn promise_resolve_with_constructor(
    vm: &mut Vm,
    ctor: Value,
    value: Value,
) -> error::Result<Value> {
    if !vm.is_constructor_value(&ctor) {
        return Err(Error::type_err(
            "Promise.resolve receiver is not a constructor",
        ));
    }
    let pins = vm.pin_many(&[ctor.clone(), value.clone()]);
    let result = (|| -> error::Result<Value> {
        if let Value::Object(idx) = &value {
            let is_promise = vm
                .heap
                .with_obj(idx.0, |o| matches!(o, HeapObj::Promise(_)));
            if is_promise {
                let value_constructor =
                    vm.get_property_by_key(&value, &PropertyKey::from("constructor"))?;
                if value_constructor == ctor {
                    return Ok(value.clone());
                }
            }
        }
        let capability = new_promise_capability(vm, ctor.clone())?;
        let capability_pins = vm.pin_many(&[
            capability.promise.clone(),
            capability.resolve.clone(),
            value.clone(),
        ]);
        let result = vm.call_function(
            &capability.resolve,
            std::slice::from_ref(&value),
            Some(Value::Undefined),
        );
        vm.unpin_many(capability_pins);
        result?;
        Ok(capability.promise)
    })();
    vm.unpin_many(pins);
    result
}

/// `Promise.resolve(v)`: create a promise capability from the receiver
/// constructor and resolve it with `v`.
pub(crate) fn promise_static_resolve(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    promise_resolve_with_constructor(vm, ctor, value)
}

/// `Promise.reject(r)`: returns a promise rejected with `r`.
pub(crate) fn promise_static_reject(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let reason = args.first().cloned().unwrap_or(Value::Undefined);
    let capability = new_promise_capability(vm, ctor)?;
    let pins = vm.pin_many(&[
        capability.promise.clone(),
        capability.reject.clone(),
        reason.clone(),
    ]);
    let result = vm.call_function(
        &capability.reject,
        std::slice::from_ref(&reason),
        Some(Value::Undefined),
    );
    vm.unpin_many(pins);
    result?;
    Ok(capability.promise)
}

fn make_pending_promise(vm: &mut Vm) -> error::Result<Value> {
    let prototype = vm.current_realm_promise_prototype();
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: Mutex::new(crate::value::PromiseStatus::Pending),
            result: Mutex::new(Value::Undefined),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(prototype)),
            extensible: AtomicBool::new(true),
        }))?;
    Ok(Value::Object(GcIdx(p_idx)))
}

fn make_fulfilled_promise(vm: &mut Vm, value: Value) -> error::Result<Value> {
    let prototype = vm.current_realm_promise_prototype();
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: Mutex::new(crate::value::PromiseStatus::Fulfilled),
            result: Mutex::new(value),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(prototype)),
            extensible: AtomicBool::new(true),
        }))?;
    Ok(Value::Object(GcIdx(p_idx)))
}

fn make_rejected_promise(vm: &mut Vm, reason: Value) -> error::Result<Value> {
    let prototype = vm.current_realm_promise_prototype();
    let p_idx = vm
        .heap
        .allocate(HeapObj::Promise(crate::value::PromiseData {
            state: Mutex::new(crate::value::PromiseStatus::Rejected),
            result: Mutex::new(reason),
            handlers: Mutex::new(Vec::new()),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(prototype)),
            extensible: AtomicBool::new(true),
        }))?;
    Ok(Value::Object(GcIdx(p_idx)))
}

fn promise_rejection_value(vm: &mut Vm, err: &Arc<error::Error>) -> error::Result<Value> {
    if !err.catchable() {
        return Err(err.clone());
    }
    match err.thrown_value.clone() {
        Some(reason) => Ok(reason),
        None => vm.make_error_value(err),
    }
}

fn get_promise_resolve(vm: &mut Vm, ctor: &Value) -> error::Result<Value> {
    let promise_resolve = vm.get_property(ctor, "resolve")?;
    if !is_callable(&promise_resolve, &vm.heap) {
        return Err(Error::type_err("Promise resolve is not callable"));
    }
    Ok(promise_resolve)
}

fn make_aggregate_error(vm: &mut Vm, errors: Value) -> error::Result<Value> {
    let proto = vm.error_prototype_for_env("AggregateError", vm.current_realm_global_env());

    let idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(proto)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Error")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let idx = GcIdx(idx);
    vm.heap.with_obj(idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("name"),
            data_prop(Value::String(Arc::from("AggregateError"))),
        );
        props.insert(PropertyKey::from("errors"), data_prop(errors));
    });
    Ok(Value::Object(idx))
}

fn call_promise_capability_function(
    vm: &mut Vm,
    function: &Value,
    value: Value,
) -> error::Result<Value> {
    let pins = vm.pin_many(&[function.clone(), value.clone()]);
    let result = vm.call_function(
        function,
        std::slice::from_ref(&value),
        Some(Value::Undefined),
    );
    vm.unpin_many(pins);
    result
}

fn reject_promise_capability(
    vm: &mut Vm,
    capability: &PromiseCapability,
    reason: Value,
) -> error::Result<()> {
    call_promise_capability_function(vm, &capability.reject, reason).map(|_| ())
}

fn promise_capability_reject_and_return(
    vm: &mut Vm,
    capability: &PromiseCapability,
    err: Arc<error::Error>,
) -> error::Result<Value> {
    let mut pins = vm.pin_many(&[
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
    ]);
    let reason = match promise_rejection_value(vm, &err) {
        Ok(reason) => reason,
        Err(err) => {
            vm.unpin_many(pins);
            return Err(err);
        }
    };
    pins += vm.pin(&reason);
    let rejected = reject_promise_capability(vm, capability, reason);
    vm.unpin_many(pins);
    rejected?;
    Ok(capability.promise.clone())
}

fn promise_combinator_close_and_reject(
    vm: &mut Vm,
    capability: &PromiseCapability,
    iterator: &Value,
    err: Arc<error::Error>,
) -> error::Result<Value> {
    let mut pins = vm.pin_many(&[
        iterator.clone(),
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
    ]);
    let reason = match promise_rejection_value(vm, &err) {
        Ok(reason) => reason,
        Err(err) => {
            vm.unpin_many(pins);
            return Err(err);
        }
    };
    pins += vm.pin(&reason);
    let close = vm.iterator_close(iterator);
    if let Err(close_err) = close {
        if !close_err.catchable() {
            vm.unpin_many(pins);
            return Err(close_err);
        }
    }
    let rejected = reject_promise_capability(vm, capability, reason);
    vm.unpin_many(pins);
    rejected?;
    Ok(capability.promise.clone())
}

pub(crate) fn promise_all_resolve_element(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let record_idx = match this {
        Some(Value::Object(idx)) => idx,
        _ => return Err(Error::type_err("Promise.all resolve element receiver")),
    };
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    let (already_called, index, state) = vm.heap.with_obj(record_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        let already_called = matches!(
            props
                .get(&PropertyKey::from("alreadyCalled"))
                .map(|desc| &desc.value),
            Some(Value::Bool(true))
        );
        props.insert(
            PropertyKey::from("alreadyCalled"),
            PropertyDescriptor::data(Value::Bool(true)),
        );
        let index = match props
            .get(&PropertyKey::from("index"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n >= 0.0 => n as usize,
            _ => 0,
        };
        let state = props
            .get(&PropertyKey::from("state"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        (already_called, index, state)
    });
    if already_called {
        return Ok(Value::Undefined);
    }

    let state_idx = match state {
        Value::Object(idx) => idx,
        _ => return Err(Error::type_err("Promise.all state record")),
    };
    let (values, resolve, reject, remaining) = vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let props = props.lock();
        let values = props
            .get(&PropertyKey::from("values"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let resolve = props
            .get(&PropertyKey::from("resolve"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let reject = props
            .get(&PropertyKey::from("reject"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let remaining = match props
            .get(&PropertyKey::from("remaining"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n > 0.0 => n as usize,
            _ => 0,
        };
        (values, resolve, reject, remaining)
    });
    let values_idx = match &values {
        Value::Object(idx) => *idx,
        _ => return Err(Error::type_err("Promise.all values array")),
    };
    vm.heap.with_obj(values_idx.0, |obj| {
        if let HeapObj::Array(array) = obj {
            let mut items = array.items.lock();
            let mut present = array.present.lock();
            if index >= items.len() {
                items.resize(index + 1, Value::Undefined);
                present.resize(index + 1, false);
            }
            items[index] = value;
            present[index] = true;
            Ok(())
        } else {
            Err(Error::type_err("Promise.all values array"))
        }
    })?;

    let remaining = remaining.saturating_sub(1);
    vm.heap.with_obj(state_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(remaining as f64)),
        );
    });
    if remaining == 0 {
        if let Err(err) = call_promise_capability_function(vm, &resolve, values) {
            let reason = promise_rejection_value(vm, &err)?;
            call_promise_capability_function(vm, &reject, reason)?;
        }
    }
    Ok(Value::Undefined)
}

fn promise_all_settled_element(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    status: &str,
    key: &str,
) -> error::Result<Value> {
    let record_idx = match this {
        Some(Value::Object(idx)) => idx,
        _ => return Err(Error::type_err("Promise.allSettled element receiver")),
    };
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    let (already_called, index, state) = vm.heap.with_obj(record_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        let already_called = matches!(
            props
                .get(&PropertyKey::from("alreadyCalled"))
                .map(|desc| &desc.value),
            Some(Value::Bool(true))
        );
        props.insert(
            PropertyKey::from("alreadyCalled"),
            PropertyDescriptor::data(Value::Bool(true)),
        );
        let index = match props
            .get(&PropertyKey::from("index"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n >= 0.0 => n as usize,
            _ => 0,
        };
        let state = props
            .get(&PropertyKey::from("state"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        (already_called, index, state)
    });
    if already_called {
        return Ok(Value::Undefined);
    }

    let state_idx = match state {
        Value::Object(idx) => idx,
        _ => return Err(Error::type_err("Promise.allSettled state record")),
    };
    let (values, resolve, reject, remaining) = vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let props = props.lock();
        let values = props
            .get(&PropertyKey::from("values"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let resolve = props
            .get(&PropertyKey::from("resolve"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let reject = props
            .get(&PropertyKey::from("reject"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let remaining = match props
            .get(&PropertyKey::from("remaining"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n > 0.0 => n as usize,
            _ => 0,
        };
        (values, resolve, reject, remaining)
    });
    let result_pins = vm.pin_many(std::slice::from_ref(&value));
    let result = settled_result_object(vm, status, key, value);
    vm.unpin_many(result_pins);
    let result = result?;
    let values_idx = match &values {
        Value::Object(idx) => *idx,
        _ => return Err(Error::type_err("Promise.allSettled values array")),
    };
    vm.heap.with_obj(values_idx.0, |obj| {
        if let HeapObj::Array(array) = obj {
            let mut items = array.items.lock();
            let mut present = array.present.lock();
            if index >= items.len() {
                items.resize(index + 1, Value::Undefined);
                present.resize(index + 1, false);
            }
            items[index] = result;
            present[index] = true;
            Ok(())
        } else {
            Err(Error::type_err("Promise.allSettled values array"))
        }
    })?;

    let remaining = remaining.saturating_sub(1);
    vm.heap.with_obj(state_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(remaining as f64)),
        );
    });
    if remaining == 0 {
        if let Err(err) = call_promise_capability_function(vm, &resolve, values) {
            let reason = promise_rejection_value(vm, &err)?;
            call_promise_capability_function(vm, &reject, reason)?;
        }
    }
    Ok(Value::Undefined)
}

pub(crate) fn promise_all_settled_resolve_element(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    promise_all_settled_element(vm, args, this, "fulfilled", "value")
}

pub(crate) fn promise_all_settled_reject_element(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    promise_all_settled_element(vm, args, this, "rejected", "reason")
}

fn make_type_error_object(vm: &mut Vm, message: &str) -> error::Result<Value> {
    let proto = vm.error_prototype_for_env("TypeError", vm.current_realm_global_env());

    let idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(proto)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Error")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    let idx = GcIdx(idx);
    vm.heap.with_obj(idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("name"),
            data_prop(Value::String(Arc::from("TypeError"))),
        );
        props.insert(
            PropertyKey::from("message"),
            data_prop(Value::String(Arc::from(message))),
        );
    });
    Ok(Value::Object(idx))
}

fn make_null_proto_object(vm: &mut Vm) -> error::Result<Value> {
    let idx = vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(None),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Object")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?;
    Ok(Value::Object(GcIdx(idx)))
}

fn array_items_snapshot(value: &Value, vm: &Vm, name: &str) -> error::Result<Vec<Value>> {
    let idx = match value {
        Value::Object(idx) => *idx,
        _ => return Err(Error::type_err(format!("{name} is not an array"))),
    };
    vm.heap.with_obj(idx.0, |obj| match obj {
        HeapObj::Array(array) => Ok(array.items.lock().clone()),
        _ => Err(Error::type_err(format!("{name} is not an array"))),
    })
}

fn keyed_result_property_key(value: &Value) -> error::Result<PropertyKey> {
    match value {
        Value::String(s) => Ok(PropertyKey::from(s.clone())),
        Value::Symbol(id) => Ok(PropertyKey::symbol(*id)),
        _ => Err(Error::type_err("Promise keyed result key is invalid")),
    }
}

fn keyed_data_prop(value: Value) -> PropertyDescriptor {
    PropertyDescriptor {
        value,
        writable: true,
        enumerable: true,
        configurable: true,
        get: None,
        set: None,
        is_accessor: false,
    }
}

fn append_keyed_entry_placeholder(
    vm: &Vm,
    keys: &Value,
    values: &Value,
    key: &PropertyKey,
) -> error::Result<usize> {
    let keys_idx = match keys {
        Value::Object(idx) => *idx,
        _ => return Err(Error::type_err("Promise keyed keys array")),
    };
    let values_idx = match values {
        Value::Object(idx) => *idx,
        _ => return Err(Error::type_err("Promise keyed values array")),
    };
    let arrays_are_valid = vm
        .heap
        .with_obj(keys_idx.0, |obj| matches!(obj, HeapObj::Array(_)))
        && vm
            .heap
            .with_obj(values_idx.0, |obj| matches!(obj, HeapObj::Array(_)));
    if !arrays_are_valid {
        return Err(Error::type_err("Promise keyed entry storage"));
    }

    let index = vm.heap.with_obj(keys_idx.0, |obj| {
        let HeapObj::Array(array) = obj else {
            unreachable!("validated Promise keyed keys array")
        };
        let mut items = array.items.lock();
        let mut present = array.present.lock();
        let index = items.len();
        items.push(property_key_to_value(key));
        present.push(true);
        index
    });
    vm.heap.with_obj(values_idx.0, |obj| {
        let HeapObj::Array(array) = obj else {
            unreachable!("validated Promise keyed values array")
        };
        array.items.lock().push(Value::Undefined);
        array.present.lock().push(false);
    });
    Ok(index)
}

fn make_keyed_result_object(vm: &mut Vm, keys: Value, values: Value) -> error::Result<Value> {
    let pins = vm.pin_many(&[keys.clone(), values.clone()]);
    let key_items = array_items_snapshot(&keys, vm, "Promise keyed keys")?;
    let value_items = array_items_snapshot(&values, vm, "Promise keyed values")?;
    let result = make_null_proto_object(vm);
    vm.unpin_many(pins);
    let result = result?;
    let result_idx = match result {
        Value::Object(idx) => idx,
        _ => return Err(Error::type_err("Promise keyed result object")),
    };
    vm.heap.with_obj(result_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        for (index, key_value) in key_items.iter().enumerate() {
            let key = keyed_result_property_key(key_value)?;
            let value = value_items.get(index).cloned().unwrap_or(Value::Undefined);
            props.insert(key, keyed_data_prop(value));
        }
        Ok::<(), Arc<Error>>(())
    })?;
    Ok(Value::Object(result_idx))
}

fn promise_keyed_element(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    settled: Option<(&str, &str)>,
) -> error::Result<Value> {
    let record_idx = match this {
        Some(Value::Object(idx)) => idx,
        _ => return Err(Error::type_err("Promise keyed element receiver")),
    };
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    let (already_called, index, state) = vm.heap.with_obj(record_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        let already_called = matches!(
            props
                .get(&PropertyKey::from("alreadyCalled"))
                .map(|desc| &desc.value),
            Some(Value::Bool(true))
        );
        props.insert(
            PropertyKey::from("alreadyCalled"),
            PropertyDescriptor::data(Value::Bool(true)),
        );
        let index = match props
            .get(&PropertyKey::from("index"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n >= 0.0 => n as usize,
            _ => 0,
        };
        let state = props
            .get(&PropertyKey::from("state"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        (already_called, index, state)
    });
    if already_called {
        return Ok(Value::Undefined);
    }

    let state_idx = match state {
        Value::Object(idx) => idx,
        _ => return Err(Error::type_err("Promise keyed state record")),
    };
    let (keys, values, resolve, reject, remaining) = vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let props = props.lock();
        let keys = props
            .get(&PropertyKey::from("keys"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let values = props
            .get(&PropertyKey::from("values"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let resolve = props
            .get(&PropertyKey::from("resolve"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let reject = props
            .get(&PropertyKey::from("reject"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let remaining = match props
            .get(&PropertyKey::from("remaining"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n > 0.0 => n as usize,
            _ => 0,
        };
        (keys, values, resolve, reject, remaining)
    });

    let stored_value = if let Some((status, key)) = settled {
        let result_pins = vm.pin_many(std::slice::from_ref(&value));
        let result = settled_result_object(vm, status, key, value);
        vm.unpin_many(result_pins);
        result?
    } else {
        value
    };
    let values_idx = match &values {
        Value::Object(idx) => *idx,
        _ => return Err(Error::type_err("Promise keyed values array")),
    };
    vm.heap.with_obj(values_idx.0, |obj| {
        if let HeapObj::Array(array) = obj {
            let mut items = array.items.lock();
            let mut present = array.present.lock();
            if index >= items.len() {
                items.resize(index + 1, Value::Undefined);
                present.resize(index + 1, false);
            }
            items[index] = stored_value;
            present[index] = true;
            Ok(())
        } else {
            Err(Error::type_err("Promise keyed values array"))
        }
    })?;

    let remaining = remaining.saturating_sub(1);
    vm.heap.with_obj(state_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(remaining as f64)),
        );
    });
    if remaining == 0 {
        let result_pins = vm.pin_many(&[keys.clone(), values.clone()]);
        let result = make_keyed_result_object(vm, keys, values);
        vm.unpin_many(result_pins);
        match result {
            Ok(result) => {
                if let Err(err) = call_promise_capability_function(vm, &resolve, result) {
                    let reason = promise_rejection_value(vm, &err)?;
                    call_promise_capability_function(vm, &reject, reason)?;
                }
            }
            Err(err) => {
                let reason = promise_rejection_value(vm, &err)?;
                call_promise_capability_function(vm, &reject, reason)?;
            }
        }
    }
    Ok(Value::Undefined)
}

pub(crate) fn promise_all_keyed_resolve_element(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    promise_keyed_element(vm, args, this, None)
}

pub(crate) fn promise_all_settled_keyed_resolve_element(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    promise_keyed_element(vm, args, this, Some(("fulfilled", "value")))
}

pub(crate) fn promise_all_settled_keyed_reject_element(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    promise_keyed_element(vm, args, this, Some(("rejected", "reason")))
}

pub(crate) fn promise_any_reject_element(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let record_idx = match this {
        Some(Value::Object(idx)) => idx,
        _ => return Err(Error::type_err("Promise.any reject element receiver")),
    };
    let reason = args.first().cloned().unwrap_or(Value::Undefined);
    let (already_called, index, state) = vm.heap.with_obj(record_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        let already_called = matches!(
            props
                .get(&PropertyKey::from("alreadyCalled"))
                .map(|desc| &desc.value),
            Some(Value::Bool(true))
        );
        props.insert(
            PropertyKey::from("alreadyCalled"),
            PropertyDescriptor::data(Value::Bool(true)),
        );
        let index = match props
            .get(&PropertyKey::from("index"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n >= 0.0 => n as usize,
            _ => 0,
        };
        let state = props
            .get(&PropertyKey::from("state"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        (already_called, index, state)
    });
    if already_called {
        return Ok(Value::Undefined);
    }

    let state_idx = match state {
        Value::Object(idx) => idx,
        _ => return Err(Error::type_err("Promise.any state record")),
    };
    let (errors, reject, remaining) = vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let props = props.lock();
        let errors = props
            .get(&PropertyKey::from("errors"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let reject = props
            .get(&PropertyKey::from("reject"))
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let remaining = match props
            .get(&PropertyKey::from("remaining"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n > 0.0 => n as usize,
            _ => 0,
        };
        (errors, reject, remaining)
    });
    let errors_idx = match &errors {
        Value::Object(idx) => *idx,
        _ => return Err(Error::type_err("Promise.any errors array")),
    };
    vm.heap.with_obj(errors_idx.0, |obj| {
        if let HeapObj::Array(array) = obj {
            let mut items = array.items.lock();
            let mut present = array.present.lock();
            if index >= items.len() {
                items.resize(index + 1, Value::Undefined);
                present.resize(index + 1, false);
            }
            items[index] = reason;
            present[index] = true;
            Ok(())
        } else {
            Err(Error::type_err("Promise.any errors array"))
        }
    })?;

    let remaining = remaining.saturating_sub(1);
    vm.heap.with_obj(state_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(remaining as f64)),
        );
    });
    if remaining == 0 {
        let error = make_aggregate_error(vm, errors)?;
        call_promise_capability_function(vm, &reject, error)?;
    }
    Ok(Value::Undefined)
}

fn settled_result_object(
    vm: &mut Vm,
    status: &str,
    key: &str,
    value: Value,
) -> error::Result<Value> {
    let obj = vm.new_object_in_current_realm()?;
    vm.heap.with_obj(obj.0, |o| {
        let props = o.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("status"),
            data_prop(Value::String(Arc::from(status))),
        );
        props.insert(PropertyKey::from(key), data_prop(value));
    });
    Ok(Value::Object(obj))
}

pub(crate) fn promise_static_all(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let capability = new_promise_capability(vm, ctor.clone())?;
    let mut pins = vm.pin_many(&[
        ctor.clone(),
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
    ]);
    let promise_resolve = match get_promise_resolve(vm, &ctor) {
        Ok(promise_resolve) => promise_resolve,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };

    let iterable = args.first().cloned().unwrap_or(Value::Undefined);
    pins += vm.pin_many(&[promise_resolve.clone(), iterable.clone()]);
    let iter = match vm.make_iterator(&iterable) {
        Ok(iter) => iter,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };
    pins += vm.pin(&iter);

    let values = make_value_array_in_current_realm(vm, Vec::new())?;
    pins += vm.pin_many(std::slice::from_ref(&values));
    let state_idx = vm.new_object()?;
    let state = Value::Object(state_idx);
    vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("values"),
            PropertyDescriptor::data(values.clone()),
        );
        props.insert(
            PropertyKey::from("resolve"),
            PropertyDescriptor::data(capability.resolve.clone()),
        );
        props.insert(
            PropertyKey::from("reject"),
            PropertyDescriptor::data(capability.reject.clone()),
        );
        props.insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(1.0)),
        );
    });
    pins += vm.pin_many(std::slice::from_ref(&state));
    let mut index = 0usize;

    loop {
        let (value, done) = match vm.iterator_next(&iter) {
            Ok(step) => step,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        if done {
            let remaining = vm.heap.with_obj(state_idx.0, |obj| {
                let props = obj.props();
                let mut props = props.lock();
                let remaining = match props
                    .get(&PropertyKey::from("remaining"))
                    .map(|desc| desc.value.clone())
                {
                    Some(Value::Number(n)) if n > 0.0 => n as usize,
                    _ => 0,
                }
                .saturating_sub(1);
                props.insert(
                    PropertyKey::from("remaining"),
                    PropertyDescriptor::data(Value::Number(remaining as f64)),
                );
                remaining
            });
            if remaining == 0 {
                let resolve_result =
                    call_promise_capability_function(vm, &capability.resolve, values.clone());
                let result = match resolve_result {
                    Ok(_) => Ok(capability.promise.clone()),
                    Err(err) => promise_capability_reject_and_return(vm, &capability, err),
                };
                vm.unpin_many(pins);
                return result;
            }
            vm.unpin_many(pins);
            return Ok(capability.promise);
        }

        if let Value::Object(values_idx) = &values {
            vm.heap.with_obj(values_idx.0, |obj| {
                if let HeapObj::Array(array) = obj {
                    array.items.lock().push(Value::Undefined);
                    array.present.lock().push(false);
                }
            });
        }
        vm.heap.with_obj(state_idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            let remaining = match props
                .get(&PropertyKey::from("remaining"))
                .map(|desc| desc.value.clone())
            {
                Some(Value::Number(n)) if n > 0.0 => n,
                _ => 0.0,
            };
            props.insert(
                PropertyKey::from("remaining"),
                PropertyDescriptor::data(Value::Number(remaining + 1.0)),
            );
        });

        let record_idx = vm.new_object()?;
        let record = Value::Object(record_idx);
        let record_pins = vm.pin_many(std::slice::from_ref(&record));
        vm.heap.with_obj(record_idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            props.insert(
                PropertyKey::from("alreadyCalled"),
                PropertyDescriptor::data(Value::Bool(false)),
            );
            props.insert(
                PropertyKey::from("index"),
                PropertyDescriptor::data(Value::Number(index as f64)),
            );
            props.insert(
                PropertyKey::from("state"),
                PropertyDescriptor::data(state.clone()),
            );
        });
        let resolve_element_result = create_bound_native_function(
            vm,
            "",
            "",
            promise_all_resolve_element,
            1,
            record.clone(),
        );
        vm.unpin_many(record_pins);
        let resolve_element = match resolve_element_result {
            Ok(resolve_element) => resolve_element,
            Err(err) => {
                vm.unpin_many(pins);
                return Err(err);
            }
        };
        let element_pins = vm.pin_many(&[value.clone(), record, resolve_element.clone()]);
        let next_promise_result = vm.call_function(
            &promise_resolve,
            std::slice::from_ref(&value),
            Some(ctor.clone()),
        );
        vm.unpin_many(element_pins);
        let next_promise = match next_promise_result {
            Ok(next_promise) => next_promise,
            Err(err) => {
                let result = promise_combinator_close_and_reject(vm, &capability, &iter, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then = match vm.get_property(&next_promise, "then") {
            Ok(then) => then,
            Err(err) => {
                let result = promise_combinator_close_and_reject(vm, &capability, &iter, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then_pins = vm.pin_many(&[next_promise.clone(), then.clone()]);
        let then_result = vm.call_function(
            &then,
            &[resolve_element, capability.reject.clone()],
            Some(next_promise),
        );
        vm.unpin_many(then_pins);
        if let Err(err) = then_result {
            let result = promise_combinator_close_and_reject(vm, &capability, &iter, err);
            vm.unpin_many(pins);
            return result;
        }
        index += 1;
    }
}

pub(crate) fn promise_static_race(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let capability = new_promise_capability(vm, ctor.clone())?;
    let mut pins = vm.pin_many(&[
        ctor.clone(),
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
    ]);
    let promise_resolve = match get_promise_resolve(vm, &ctor) {
        Ok(promise_resolve) => promise_resolve,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };

    let iterable = args.first().cloned().unwrap_or(Value::Undefined);
    pins += vm.pin_many(&[promise_resolve.clone(), iterable.clone()]);
    let iter = match vm.make_iterator(&iterable) {
        Ok(iter) => iter,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };
    pins += vm.pin(&iter);

    loop {
        let (value, done) = match vm.iterator_next(&iter) {
            Ok(step) => step,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        if done {
            vm.unpin_many(pins);
            return Ok(capability.promise);
        }

        let value_pins = vm.pin_many(std::slice::from_ref(&value));
        let next_promise_result = vm.call_function(
            &promise_resolve,
            std::slice::from_ref(&value),
            Some(ctor.clone()),
        );
        vm.unpin_many(value_pins);
        let next_promise = match next_promise_result {
            Ok(next_promise) => next_promise,
            Err(err) => {
                let result = promise_combinator_close_and_reject(vm, &capability, &iter, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then = match vm.get_property(&next_promise, "then") {
            Ok(then) => then,
            Err(err) => {
                let result = promise_combinator_close_and_reject(vm, &capability, &iter, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then_pins = vm.pin_many(&[next_promise.clone(), then.clone()]);
        let then_result = vm.call_function(
            &then,
            &[capability.resolve.clone(), capability.reject.clone()],
            Some(next_promise),
        );
        vm.unpin_many(then_pins);
        if let Err(err) = then_result {
            let result = promise_combinator_close_and_reject(vm, &capability, &iter, err);
            vm.unpin_many(pins);
            return result;
        }
    }
}

pub(crate) fn promise_static_all_settled(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let capability = new_promise_capability(vm, ctor.clone())?;
    let mut pins = vm.pin_many(&[
        ctor.clone(),
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
    ]);
    let promise_resolve = match get_promise_resolve(vm, &ctor) {
        Ok(promise_resolve) => promise_resolve,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };

    let iterable = args.first().cloned().unwrap_or(Value::Undefined);
    pins += vm.pin_many(&[promise_resolve.clone(), iterable.clone()]);
    let iter = match vm.make_iterator(&iterable) {
        Ok(iter) => iter,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };
    pins += vm.pin(&iter);

    let values = make_value_array_in_current_realm(vm, Vec::new())?;
    pins += vm.pin_many(std::slice::from_ref(&values));
    let state_idx = vm.new_object()?;
    let state = Value::Object(state_idx);
    vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("values"),
            PropertyDescriptor::data(values.clone()),
        );
        props.insert(
            PropertyKey::from("resolve"),
            PropertyDescriptor::data(capability.resolve.clone()),
        );
        props.insert(
            PropertyKey::from("reject"),
            PropertyDescriptor::data(capability.reject.clone()),
        );
        props.insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(1.0)),
        );
    });
    pins += vm.pin_many(std::slice::from_ref(&state));
    let mut index = 0usize;

    loop {
        let (value, done) = match vm.iterator_next(&iter) {
            Ok(step) => step,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        if done {
            let remaining = vm.heap.with_obj(state_idx.0, |obj| {
                let props = obj.props();
                let mut props = props.lock();
                let remaining = match props
                    .get(&PropertyKey::from("remaining"))
                    .map(|desc| desc.value.clone())
                {
                    Some(Value::Number(n)) if n > 0.0 => n as usize,
                    _ => 0,
                }
                .saturating_sub(1);
                props.insert(
                    PropertyKey::from("remaining"),
                    PropertyDescriptor::data(Value::Number(remaining as f64)),
                );
                remaining
            });
            if remaining == 0 {
                let resolve_result =
                    call_promise_capability_function(vm, &capability.resolve, values.clone());
                let result = match resolve_result {
                    Ok(_) => Ok(capability.promise.clone()),
                    Err(err) => promise_capability_reject_and_return(vm, &capability, err),
                };
                vm.unpin_many(pins);
                return result;
            }
            vm.unpin_many(pins);
            return Ok(capability.promise);
        }

        if let Value::Object(values_idx) = &values {
            vm.heap.with_obj(values_idx.0, |obj| {
                if let HeapObj::Array(array) = obj {
                    array.items.lock().push(Value::Undefined);
                    array.present.lock().push(false);
                }
            });
        }
        vm.heap.with_obj(state_idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            let remaining = match props
                .get(&PropertyKey::from("remaining"))
                .map(|desc| desc.value.clone())
            {
                Some(Value::Number(n)) if n > 0.0 => n,
                _ => 0.0,
            };
            props.insert(
                PropertyKey::from("remaining"),
                PropertyDescriptor::data(Value::Number(remaining + 1.0)),
            );
        });

        let record_idx = vm.new_object()?;
        let record = Value::Object(record_idx);
        let record_pins = vm.pin_many(std::slice::from_ref(&record));
        vm.heap.with_obj(record_idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            props.insert(
                PropertyKey::from("alreadyCalled"),
                PropertyDescriptor::data(Value::Bool(false)),
            );
            props.insert(
                PropertyKey::from("index"),
                PropertyDescriptor::data(Value::Number(index as f64)),
            );
            props.insert(
                PropertyKey::from("state"),
                PropertyDescriptor::data(state.clone()),
            );
        });
        let resolve_element_result = create_bound_native_function(
            vm,
            "",
            "",
            promise_all_settled_resolve_element,
            1,
            record.clone(),
        );
        let resolve_element = match resolve_element_result {
            Ok(resolve_element) => resolve_element,
            Err(err) => {
                vm.unpin_many(record_pins);
                vm.unpin_many(pins);
                return Err(err);
            }
        };
        let resolve_pin = vm.pin_many(std::slice::from_ref(&resolve_element));
        let reject_element_result = create_bound_native_function(
            vm,
            "",
            "",
            promise_all_settled_reject_element,
            1,
            record.clone(),
        );
        vm.unpin_many(resolve_pin);
        vm.unpin_many(record_pins);
        let reject_element = match reject_element_result {
            Ok(reject_element) => reject_element,
            Err(err) => {
                vm.unpin_many(pins);
                return Err(err);
            }
        };
        let element_pins = vm.pin_many(&[
            value.clone(),
            record,
            resolve_element.clone(),
            reject_element.clone(),
        ]);
        let next_promise_result = vm.call_function(
            &promise_resolve,
            std::slice::from_ref(&value),
            Some(ctor.clone()),
        );
        vm.unpin_many(element_pins);
        let next_promise = match next_promise_result {
            Ok(next_promise) => next_promise,
            Err(err) => {
                let result = promise_combinator_close_and_reject(vm, &capability, &iter, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then = match vm.get_property(&next_promise, "then") {
            Ok(then) => then,
            Err(err) => {
                let result = promise_combinator_close_and_reject(vm, &capability, &iter, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then_pins = vm.pin_many(&[next_promise.clone(), then.clone()]);
        let then_result = vm.call_function(
            &then,
            &[resolve_element, reject_element],
            Some(next_promise),
        );
        vm.unpin_many(then_pins);
        if let Err(err) = then_result {
            let result = promise_combinator_close_and_reject(vm, &capability, &iter, err);
            vm.unpin_many(pins);
            return result;
        }
        index += 1;
    }
}

fn promise_static_keyed(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    all_settled: bool,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let capability = new_promise_capability(vm, ctor.clone())?;
    let mut pins = vm.pin_many(&[
        ctor.clone(),
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
    ]);
    let promise_resolve = match get_promise_resolve(vm, &ctor) {
        Ok(promise_resolve) => promise_resolve,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };

    let promises = args.first().cloned().unwrap_or(Value::Undefined);
    pins += vm.pin_many(&[promise_resolve.clone(), promises.clone()]);
    if !matches!(promises, Value::Object(_)) {
        let err = make_type_error_object(vm, "Promise keyed input is not an object")?;
        let result =
            reject_promise_capability(vm, &capability, err).map(|_| capability.promise.clone());
        vm.unpin_many(pins);
        return result;
    }

    let property_keys = match own_property_keys_or_throw(vm, &promises, false, true, true) {
        Ok(keys) => keys,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };
    let keys_array = match make_value_array_in_current_realm(vm, Vec::new()) {
        Ok(keys_array) => keys_array,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };
    let keys_array_pins = vm.pin_many(std::slice::from_ref(&keys_array));
    let values = match make_value_array_in_current_realm(vm, Vec::new()) {
        Ok(values) => values,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(keys_array_pins);
            vm.unpin_many(pins);
            return result;
        }
    };
    vm.unpin_many(keys_array_pins);
    pins += vm.pin_many(&[keys_array.clone(), values.clone()]);
    let state_idx = match vm.new_object() {
        Ok(state_idx) => state_idx,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };
    let state = Value::Object(state_idx);
    vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("keys"),
            PropertyDescriptor::data(keys_array.clone()),
        );
        props.insert(
            PropertyKey::from("values"),
            PropertyDescriptor::data(values.clone()),
        );
        props.insert(
            PropertyKey::from("resolve"),
            PropertyDescriptor::data(capability.resolve.clone()),
        );
        props.insert(
            PropertyKey::from("reject"),
            PropertyDescriptor::data(capability.reject.clone()),
        );
        props.insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(1.0)),
        );
    });
    pins += vm.pin_many(std::slice::from_ref(&state));

    for key in &property_keys {
        // PerformPromiseAllKeyed interleaves [[GetOwnProperty]] with this
        // key's Get/resolve/then chain. Pre-filtering every descriptor here
        // would observably reorder Proxy traps across keys.
        let descriptor = match own_property_descriptor_for_key_or_throw(vm, &promises, key) {
            Ok(descriptor) => descriptor,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        if !descriptor.is_some_and(|descriptor| descriptor.enumerable) {
            continue;
        }
        let value = match vm.get_property_by_key(&promises, key) {
            Ok(value) => value,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let value_pin = vm.pin(&value);
        let index = match append_keyed_entry_placeholder(vm, &keys_array, &values, key) {
            Ok(index) => index,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin(value_pin);
                vm.unpin_many(pins);
                return result;
            }
        };
        let next_promise_result = vm.call_function(
            &promise_resolve,
            std::slice::from_ref(&value),
            Some(ctor.clone()),
        );
        vm.unpin(value_pin);
        let next_promise = match next_promise_result {
            Ok(next_promise) => next_promise,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let next_promise_pin = vm.pin(&next_promise);
        let record_idx = match vm.new_object() {
            Ok(record_idx) => record_idx,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin(next_promise_pin);
                vm.unpin_many(pins);
                return result;
            }
        };
        let record = Value::Object(record_idx);
        let record_pins = vm.pin_many(std::slice::from_ref(&record));
        vm.heap.with_obj(record_idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            props.insert(
                PropertyKey::from("alreadyCalled"),
                PropertyDescriptor::data(Value::Bool(false)),
            );
            props.insert(
                PropertyKey::from("index"),
                PropertyDescriptor::data(Value::Number(index as f64)),
            );
            props.insert(
                PropertyKey::from("state"),
                PropertyDescriptor::data(state.clone()),
            );
        });
        let resolve_element_result = create_bound_native_function(
            vm,
            "",
            "",
            if all_settled {
                promise_all_settled_keyed_resolve_element
            } else {
                promise_all_keyed_resolve_element
            },
            1,
            record.clone(),
        );
        let resolve_element = match resolve_element_result {
            Ok(resolve_element) => resolve_element,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(record_pins);
                vm.unpin(next_promise_pin);
                vm.unpin_many(pins);
                return result;
            }
        };
        let reject_element = if all_settled {
            let resolve_pin = vm.pin_many(std::slice::from_ref(&resolve_element));
            let reject_element_result = create_bound_native_function(
                vm,
                "",
                "",
                promise_all_settled_keyed_reject_element,
                1,
                record.clone(),
            );
            vm.unpin_many(resolve_pin);
            match reject_element_result {
                Ok(reject_element) => reject_element,
                Err(err) => {
                    let result = promise_capability_reject_and_return(vm, &capability, err);
                    vm.unpin_many(record_pins);
                    vm.unpin(next_promise_pin);
                    vm.unpin_many(pins);
                    return result;
                }
            }
        } else {
            capability.reject.clone()
        };
        let callback_pins = vm.pin_many(&[resolve_element.clone(), reject_element.clone()]);
        vm.heap.with_obj(state_idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            let remaining = match props
                .get(&PropertyKey::from("remaining"))
                .map(|desc| desc.value.clone())
            {
                Some(Value::Number(n)) if n > 0.0 => n,
                _ => 0.0,
            };
            props.insert(
                PropertyKey::from("remaining"),
                PropertyDescriptor::data(Value::Number(remaining + 1.0)),
            );
        });
        let then = match vm.get_property(&next_promise, "then") {
            Ok(then) => then,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(callback_pins);
                vm.unpin_many(record_pins);
                vm.unpin(next_promise_pin);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then_pin = vm.pin(&then);
        let then_result = vm.call_function(
            &then,
            &[resolve_element, reject_element],
            Some(next_promise),
        );
        vm.unpin(then_pin);
        if let Err(err) = then_result {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(callback_pins);
            vm.unpin_many(record_pins);
            vm.unpin(next_promise_pin);
            vm.unpin_many(pins);
            return result;
        }
        vm.unpin_many(callback_pins);
        vm.unpin_many(record_pins);
        vm.unpin(next_promise_pin);
    }

    let remaining = vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        let remaining = match props
            .get(&PropertyKey::from("remaining"))
            .map(|desc| desc.value.clone())
        {
            Some(Value::Number(n)) if n > 0.0 => n as usize,
            _ => 0,
        }
        .saturating_sub(1);
        props.insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(remaining as f64)),
        );
        remaining
    });
    if remaining == 0 {
        let result_pins = vm.pin_many(&[keys_array.clone(), values.clone()]);
        let result = make_keyed_result_object(vm, keys_array, values);
        vm.unpin_many(result_pins);
        let result = match result {
            Ok(result) => result,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let resolve_result = call_promise_capability_function(vm, &capability.resolve, result);
        let result = match resolve_result {
            Ok(_) => Ok(capability.promise.clone()),
            Err(err) => promise_capability_reject_and_return(vm, &capability, err),
        };
        vm.unpin_many(pins);
        return result;
    }

    vm.unpin_many(pins);
    Ok(capability.promise)
}

pub(crate) fn promise_static_all_keyed(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    promise_static_keyed(vm, args, this, false)
}

pub(crate) fn promise_static_all_settled_keyed(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    promise_static_keyed(vm, args, this, true)
}

pub(crate) fn promise_static_any(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let capability = new_promise_capability(vm, ctor.clone())?;
    let mut pins = vm.pin_many(&[
        ctor.clone(),
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
    ]);
    let promise_resolve = match get_promise_resolve(vm, &ctor) {
        Ok(promise_resolve) => promise_resolve,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };

    let iterable = args.first().cloned().unwrap_or(Value::Undefined);
    pins += vm.pin_many(&[promise_resolve.clone(), iterable.clone()]);
    let iter = match vm.make_iterator(&iterable) {
        Ok(iter) => iter,
        Err(err) => {
            let result = promise_capability_reject_and_return(vm, &capability, err);
            vm.unpin_many(pins);
            return result;
        }
    };
    pins += vm.pin(&iter);

    let errors = make_value_array_in_current_realm(vm, Vec::new())?;
    pins += vm.pin_many(std::slice::from_ref(&errors));
    let state_idx = vm.new_object()?;
    let state = Value::Object(state_idx);
    vm.heap.with_obj(state_idx.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("errors"),
            PropertyDescriptor::data(errors.clone()),
        );
        props.insert(
            PropertyKey::from("reject"),
            PropertyDescriptor::data(capability.reject.clone()),
        );
        props.insert(
            PropertyKey::from("remaining"),
            PropertyDescriptor::data(Value::Number(1.0)),
        );
    });
    pins += vm.pin_many(std::slice::from_ref(&state));
    let mut index = 0usize;

    loop {
        let (value, done) = match vm.iterator_next(&iter) {
            Ok(step) => step,
            Err(err) => {
                let result = promise_capability_reject_and_return(vm, &capability, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        if done {
            let remaining = vm.heap.with_obj(state_idx.0, |obj| {
                let props = obj.props();
                let mut props = props.lock();
                let remaining = match props
                    .get(&PropertyKey::from("remaining"))
                    .map(|desc| desc.value.clone())
                {
                    Some(Value::Number(n)) if n > 0.0 => n as usize,
                    _ => 0,
                }
                .saturating_sub(1);
                props.insert(
                    PropertyKey::from("remaining"),
                    PropertyDescriptor::data(Value::Number(remaining as f64)),
                );
                remaining
            });
            if remaining == 0 {
                let error = make_aggregate_error(vm, errors.clone());
                let reject_result = error.and_then(|error| {
                    call_promise_capability_function(vm, &capability.reject, error)
                });
                let result = match reject_result {
                    Ok(_) => Ok(capability.promise.clone()),
                    Err(err) => Err(err),
                };
                vm.unpin_many(pins);
                return result;
            }
            vm.unpin_many(pins);
            return Ok(capability.promise);
        }

        if let Value::Object(errors_idx) = &errors {
            vm.heap.with_obj(errors_idx.0, |obj| {
                if let HeapObj::Array(array) = obj {
                    array.items.lock().push(Value::Undefined);
                    array.present.lock().push(false);
                }
            });
        }
        vm.heap.with_obj(state_idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            let remaining = match props
                .get(&PropertyKey::from("remaining"))
                .map(|desc| desc.value.clone())
            {
                Some(Value::Number(n)) if n > 0.0 => n,
                _ => 0.0,
            };
            props.insert(
                PropertyKey::from("remaining"),
                PropertyDescriptor::data(Value::Number(remaining + 1.0)),
            );
        });

        let record_idx = vm.new_object()?;
        let record = Value::Object(record_idx);
        let record_pins = vm.pin_many(std::slice::from_ref(&record));
        vm.heap.with_obj(record_idx.0, |obj| {
            let props = obj.props();
            let mut props = props.lock();
            props.insert(
                PropertyKey::from("alreadyCalled"),
                PropertyDescriptor::data(Value::Bool(false)),
            );
            props.insert(
                PropertyKey::from("index"),
                PropertyDescriptor::data(Value::Number(index as f64)),
            );
            props.insert(
                PropertyKey::from("state"),
                PropertyDescriptor::data(state.clone()),
            );
        });
        let reject_element_result =
            create_bound_native_function(vm, "", "", promise_any_reject_element, 1, record.clone());
        vm.unpin_many(record_pins);
        let reject_element = match reject_element_result {
            Ok(reject_element) => reject_element,
            Err(err) => {
                vm.unpin_many(pins);
                return Err(err);
            }
        };
        let element_pins = vm.pin_many(&[value.clone(), record, reject_element.clone()]);
        let next_promise_result = vm.call_function(
            &promise_resolve,
            std::slice::from_ref(&value),
            Some(ctor.clone()),
        );
        vm.unpin_many(element_pins);
        let next_promise = match next_promise_result {
            Ok(next_promise) => next_promise,
            Err(err) => {
                let result = promise_combinator_close_and_reject(vm, &capability, &iter, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then = match vm.get_property(&next_promise, "then") {
            Ok(then) => then,
            Err(err) => {
                let result = promise_combinator_close_and_reject(vm, &capability, &iter, err);
                vm.unpin_many(pins);
                return result;
            }
        };
        let then_pins = vm.pin_many(&[next_promise.clone(), then.clone()]);
        let then_result = vm.call_function(
            &then,
            &[capability.resolve.clone(), reject_element],
            Some(next_promise),
        );
        vm.unpin_many(then_pins);
        if let Err(err) = then_result {
            let result = promise_combinator_close_and_reject(vm, &capability, &iter, err);
            vm.unpin_many(pins);
            return result;
        }
        index += 1;
    }
}

pub(crate) fn promise_static_try(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    let capability = new_promise_capability(vm, ctor)?;
    let callback = args.first().cloned().unwrap_or(Value::Undefined);

    let mut roots = vec![
        capability.promise.clone(),
        capability.resolve.clone(),
        capability.reject.clone(),
        callback.clone(),
    ];
    roots.extend(args.iter().skip(1).cloned());
    let pins = vm.pin_many(&roots);

    let callback_result = if is_callable(&callback, &vm.heap) {
        vm.call_function(&callback, &args[1..], Some(Value::Undefined))
    } else {
        Err(Error::type_err("Promise.try callback is not a function"))
    };
    let settle_result = match callback_result {
        Ok(value) => call_promise_capability_function(vm, &capability.resolve, value),
        Err(err) => match promise_rejection_value(vm, &err) {
            Ok(reason) => {
                let reason_pin = vm.pin(&reason);
                let result = call_promise_capability_function(vm, &capability.reject, reason);
                vm.unpin_many(reason_pin);
                result
            }
            Err(err) => Err(err),
        },
    };
    vm.unpin_many(pins);
    settle_result?;
    Ok(capability.promise)
}

pub(crate) fn promise_with_resolvers_executor(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let result_obj = match this {
        Some(Value::Object(idx)) => idx,
        _ => return Err(Error::type_err("Promise.withResolvers executor receiver")),
    };
    let resolve = args.first().cloned().unwrap_or(Value::Undefined);
    let reject = args.get(1).cloned().unwrap_or(Value::Undefined);
    vm.heap.with_obj(result_obj.0, |obj| {
        let props = obj.props();
        let mut props = props.lock();
        props.insert(
            PropertyKey::from("resolve"),
            PropertyDescriptor::data(resolve),
        );
        props.insert(
            PropertyKey::from("reject"),
            PropertyDescriptor::data(reject),
        );
    });
    Ok(Value::Undefined)
}

pub(crate) fn promise_with_resolvers(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ctor = this.unwrap_or(Value::Undefined);
    if !vm.is_constructor_value(&ctor) {
        return Err(Error::type_err(
            "Promise.withResolvers receiver is not a constructor",
        ));
    }

    let result_idx = vm.new_object_in_current_realm()?;
    let result = Value::Object(result_idx);
    let executor = create_bound_native_function(
        vm,
        "",
        "",
        promise_with_resolvers_executor,
        2,
        result.clone(),
    )?;
    let pins = vm.pin_many(&[ctor.clone(), result.clone(), executor.clone()]);
    let promise = vm.construct(&ctor, std::slice::from_ref(&executor));
    vm.unpin_many(pins);
    let promise = promise?;

    vm.heap.with_obj(result_idx.0, |obj| {
        obj.props().lock().insert(
            PropertyKey::from("promise"),
            PropertyDescriptor::data(promise),
        );
    });
    Ok(result)
}

fn promise_finally_state(vm: &Vm, this: Option<Value>) -> error::Result<(Value, Value)> {
    let Some(Value::Object(state)) = this else {
        return Err(Error::internal("missing Promise finally closure state"));
    };
    vm.heap.with_obj(state.0, |object| {
        let props = object.props().lock();
        let constructor = props
            .get(&PropertyKey::from("constructor"))
            .map(|descriptor| descriptor.value.clone())
            .ok_or_else(|| Error::internal("missing Promise finally constructor"))?;
        let on_finally = props
            .get(&PropertyKey::from("onFinally"))
            .map(|descriptor| descriptor.value.clone())
            .ok_or_else(|| Error::internal("missing Promise finally callback"))?;
        Ok((constructor, on_finally))
    })
}

fn promise_finally_value_thunk(
    _vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    Ok(this.unwrap_or(Value::Undefined))
}

fn promise_finally_thrower(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    Err(Error::thrown(this.unwrap_or(Value::Undefined), &vm.heap))
}

fn promise_finally_handler(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    rejected: bool,
) -> error::Result<Value> {
    let original = args.first().cloned().unwrap_or(Value::Undefined);
    let (constructor, on_finally) = promise_finally_state(vm, this)?;
    let realm = vm.current_realm_global_env();
    let result = vm.call_function(&on_finally, &[], Some(Value::Undefined))?;
    let promise = promise_resolve_with_constructor(vm, constructor, result)?;

    let promise_pin = vm.pin(&promise);
    let continuation = create_bound_native_function_in_env(
        vm,
        "",
        "",
        if rejected {
            promise_finally_thrower
        } else {
            promise_finally_value_thunk
        },
        0,
        original,
        realm,
    );
    let continuation = match continuation {
        Ok(continuation) => continuation,
        Err(error) => {
            vm.unpin(promise_pin);
            return Err(error);
        }
    };
    let continuation_pin = vm.pin(&continuation);
    let then = vm.get_property(&promise, "then");
    let result = match then {
        Ok(then) => vm.call_function(&then, &[continuation], Some(promise)),
        Err(error) => Err(error),
    };
    vm.unpin(continuation_pin);
    vm.unpin(promise_pin);
    result
}

fn promise_then_finally(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    promise_finally_handler(vm, args, this, false)
}

fn promise_catch_finally(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    promise_finally_handler(vm, args, this, true)
}

fn create_promise_finally_functions(
    vm: &mut Vm,
    constructor: Value,
    on_finally: Value,
    realm: GcIdx,
) -> error::Result<(Value, Value)> {
    let captures = vm.pin_many(&[constructor.clone(), on_finally.clone()]);
    let state = match vm.new_object_in_env(realm) {
        Ok(state) => state,
        Err(error) => {
            vm.unpin_many(captures);
            return Err(error);
        }
    };
    vm.heap.with_obj(state.0, |object| {
        let mut props = object.props().lock();
        props.insert(
            PropertyKey::from("constructor"),
            PropertyDescriptor::data(constructor),
        );
        props.insert(
            PropertyKey::from("onFinally"),
            PropertyDescriptor::data(on_finally),
        );
    });
    let state = Value::Object(state);
    let state_pin = vm.pin(&state);

    let then_finally = create_bound_native_function_in_env(
        vm,
        "",
        "",
        promise_then_finally,
        1,
        state.clone(),
        realm,
    );
    let then_finally = match then_finally {
        Ok(then_finally) => then_finally,
        Err(error) => {
            vm.unpin(state_pin);
            vm.unpin_many(captures);
            return Err(error);
        }
    };
    let then_pin = vm.pin(&then_finally);
    let catch_finally =
        create_bound_native_function_in_env(vm, "", "", promise_catch_finally, 1, state, realm);
    vm.unpin(then_pin);
    vm.unpin(state_pin);
    vm.unpin_many(captures);
    Ok((then_finally, catch_finally?))
}

pub(crate) fn promise_finally(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let promise = this.unwrap_or(Value::Undefined);
    if !matches!(promise, Value::Object(_)) {
        return Err(Error::type_err(
            "Promise.prototype.finally receiver is not an object",
        ));
    }

    let on_finally = args.first().cloned().unwrap_or(Value::Undefined);
    let realm = vm.current_realm_global_env();
    let default_constructor = vm.promise_constructor_for_env(realm);
    let roots = vm.pin_many(&[
        promise.clone(),
        on_finally.clone(),
        default_constructor.clone(),
    ]);
    let result = (|| -> error::Result<Value> {
        let constructor = promise_species_constructor(vm, &promise, default_constructor.clone())?;
        let (then_finally, catch_finally) = if is_callable(&on_finally, &vm.heap) {
            create_promise_finally_functions(vm, constructor, on_finally.clone(), realm)?
        } else {
            (on_finally.clone(), on_finally.clone())
        };
        let handler_roots = vm.pin_many(&[then_finally.clone(), catch_finally.clone()]);
        let then = vm.get_property(&promise, "then");
        let call_result = match then {
            Ok(then) => {
                vm.call_function(&then, &[then_finally, catch_finally], Some(promise.clone()))
            }
            Err(error) => Err(error),
        };
        vm.unpin_many(handler_roots);
        call_result
    })();
    vm.unpin_many(roots);
    result
}

pub(crate) fn promise_species_get(
    _vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    Ok(this.unwrap_or(Value::Undefined))
}

fn promise_species_constructor(
    vm: &mut Vm,
    promise: &Value,
    default_constructor: Value,
) -> error::Result<Value> {
    let constructor = vm.get_property_by_key(promise, &PropertyKey::from("constructor"))?;
    if constructor.is_undefined() {
        return Ok(default_constructor);
    }
    if !matches!(constructor, Value::Object(_)) {
        return Err(Error::type_err("Promise constructor is not an object"));
    }

    let species_key = PropertyKey::symbol(vm.well_known_symbols.species);
    let species = vm.get_property_by_key(&constructor, &species_key)?;
    if species.is_undefined() || matches!(species, Value::Null) {
        return Ok(default_constructor);
    }
    if !vm.is_constructor_value(&species) {
        return Err(Error::type_err("Promise species is not a constructor"));
    }
    Ok(species)
}

pub(crate) fn promise_then(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let on_fulfilled = args.first().cloned().unwrap_or(Value::Undefined);
    let on_fulfilled = if is_callable(&on_fulfilled, &vm.heap) {
        on_fulfilled
    } else {
        Value::Undefined
    };
    let on_rejected = args.get(1).cloned().unwrap_or(Value::Undefined);
    let on_rejected = if is_callable(&on_rejected, &vm.heap) {
        on_rejected
    } else {
        Value::Undefined
    };
    let promise = this.unwrap_or(Value::Undefined);
    let p_idx = match &promise {
        Value::Object(idx)
            if vm
                .heap
                .with_obj(idx.0, |obj| matches!(obj, HeapObj::Promise(_))) =>
        {
            idx.0
        }
        _ => return Err(Error::type_err("then called on non-promise")),
    };
    let default_constructor = vm.current_realm_promise_constructor();
    let constructor = promise_species_constructor(vm, &promise, default_constructor)?;
    let capability = new_promise_capability(vm, constructor)?;
    let derived = crate::value::PromiseReactionCapability {
        promise: capability.promise,
        resolve: capability.resolve,
        reject: capability.reject,
    };
    let (state, _result) = vm.heap.with_obj(p_idx, |o| {
        if let HeapObj::Promise(p) = o {
            (*p.state.lock(), p.result.lock().clone())
        } else {
            (crate::value::PromiseStatus::Fulfilled, Value::Undefined)
        }
    });
    let handler = crate::value::PromiseHandler {
        on_fulfilled: on_fulfilled.clone(),
        on_rejected: on_rejected.clone(),
        derived: Some(derived.clone()),
        continuation: None,
    };
    match state {
        crate::value::PromiseStatus::Pending => {
            vm.heap.with_obj(p_idx, |o| {
                if let HeapObj::Promise(p) = o {
                    p.handlers.lock().push(handler);
                }
            });
        }
        _ => {
            // already settled: schedule immediately, passing derived for chaining
            let realm = match state {
                crate::value::PromiseStatus::Fulfilled => {
                    vm.promise_reaction_job_realm(&on_fulfilled)?
                }
                crate::value::PromiseStatus::Rejected => {
                    vm.promise_reaction_job_realm(&on_rejected)?
                }
                crate::value::PromiseStatus::Pending => None,
            };
            vm.microtask_queue.push_back(crate::vm::Microtask::Then {
                promise: GcIdx(p_idx),
                on_fulfilled,
                on_rejected,
                derived: Some(derived.clone()),
                continuation: None,
                realm,
            });
        }
    }
    Ok(derived.promise)
}

pub(crate) fn promise_catch(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let promise = this.unwrap_or(Value::Undefined);
    let on_rejected = args.first().cloned().unwrap_or(Value::Undefined);
    let then = vm.get_property(&promise, "then")?;
    vm.call_function(&then, &[Value::Undefined, on_rejected], Some(promise))
}

// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promise_constructor_roots_result_before_resolving_state_allocation() {
        let mut vm = Vm::new().expect("failed to initialize VM");
        let constructor = vm.promise_ctor.clone();
        let executor = vm
            .run("(function () {})")
            .expect("failed to create Promise executor");
        let executor_pin = vm.pin(&executor);

        // Fill the heap with collectable objects so the resolving-state
        // allocation is guaranteed to collect immediately after the Promise.
        for _ in 0..16 {
            vm.new_object().expect("failed to create GC pressure");
        }
        vm.set_max_heap_objects(Some(vm.heap.live_count() + 1));

        let promise = vm.construct(&constructor, &[executor]);
        vm.unpin(executor_pin);
        let promise = promise.expect("Promise construction should survive collection");
        assert!(matches!(
            promise,
            Value::Object(idx)
                if vm.heap.with_obj(idx.0, |object| matches!(object, HeapObj::Promise(_)))
        ));
    }

    #[test]
    fn promise_keyed_descriptor_paths_restore_pin_depth() {
        let mut vm = Vm::new().expect("failed to initialize VM");
        let baseline = vm.gc_pins.len();
        vm.run(
            r#"
            var marker = {};
            var skipped = new Proxy({ key: 1 }, {
              getOwnPropertyDescriptor: function() { return undefined; }
            });
            var abrupt = new Proxy({ key: 1 }, {
              getOwnPropertyDescriptor: function() { throw marker; }
            });
            Promise.allKeyed(skipped);
            Promise.allSettledKeyed(skipped);
            Promise.allKeyed(abrupt).then(undefined, function() {});
            Promise.allSettledKeyed(abrupt).then(undefined, function() {});
            "#,
        )
        .expect("keyed descriptor paths should return Promises");
        assert_eq!(vm.gc_pins.len(), baseline);
    }
}
