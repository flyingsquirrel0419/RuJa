use super::*;

const MAX_ARRAY_BUFFER_LENGTH: usize = 1 << 26;
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

pub(crate) fn typed_array_intrinsic_constructor(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    Err(Error::type_err(
        "%TypedArray% intrinsic constructor cannot be called directly",
    ))
}

fn to_index_length(vm: &mut Vm, value: &Value, name: &str) -> error::Result<usize> {
    let n = vm.to_number(value)?;
    if n.is_nan() {
        return Ok(0);
    }
    let integer = n.trunc();
    if !integer.is_finite() || integer < 0.0 {
        return Err(Error::range(format!("Invalid {name} length")));
    }
    if integer > MAX_ARRAY_BUFFER_LENGTH as f64 {
        return Err(Error::range(format!("Invalid {name} length")));
    }
    Ok(integer as usize)
}

fn to_shared_array_buffer_length(vm: &mut Vm, value: &Value) -> error::Result<usize> {
    let n = vm.to_number(value)?;
    if n.is_nan() {
        return Ok(0);
    }
    let integer = n.trunc();
    if !integer.is_finite()
        || integer < 0.0
        || integer > MAX_SAFE_INTEGER
        || integer > usize::MAX as f64
    {
        return Err(Error::range("Invalid SharedArrayBuffer length"));
    }
    Ok(integer as usize)
}

fn to_array_like_length(vm: &mut Vm, value: &Value) -> error::Result<usize> {
    let n = vm.to_number(value)?;
    if n.is_nan() || n <= 0.0 {
        return Ok(0);
    }
    Ok(n.trunc().min(MAX_SAFE_INTEGER) as usize)
}

pub(crate) fn array_buffer_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target.is_none() {
        return Err(Error::type_err("ArrayBuffer constructor requires new"));
    }

    let length = match args.first() {
        Some(value) => to_shared_array_buffer_length(vm, value)?,
        None => 0,
    };
    let max_byte_length = match args.get(1) {
        Some(options @ Value::Object(_)) => {
            let value = vm.get_property(options, "maxByteLength")?;
            if value.is_undefined() {
                None
            } else {
                Some(to_shared_array_buffer_length(vm, &value)?)
            }
        }
        _ => None,
    };
    if max_byte_length.is_some_and(|max| length > max) {
        return Err(Error::range("Invalid ArrayBuffer length"));
    }
    let fallback_proto = if matches!(vm.array_buffer_proto, Value::Object(_)) {
        vm.array_buffer_proto.clone()
    } else {
        vm.object_proto.clone()
    };
    let proto = native_constructor_prototype_with_default(vm, "ArrayBuffer", fallback_proto)?;
    if length > MAX_ARRAY_BUFFER_LENGTH
        || max_byte_length.is_some_and(|max| max > MAX_ARRAY_BUFFER_LENGTH)
    {
        return Err(Error::range("Invalid ArrayBuffer length"));
    }
    let idx = vm
        .heap
        .allocate(HeapObj::ArrayBuffer(crate::value::ArrayBufferData {
            bytes: Arc::new(Mutex::new(vec![0; length])),
            waiters: Arc::new(Mutex::new(std::collections::HashMap::new())),
            detached: AtomicBool::new(false),
            immutable: AtomicBool::new(false),
            shared: false,
            max_byte_length,
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
        }))?;
    Ok(Value::Object(GcIdx(idx)))
}

pub(crate) fn shared_array_buffer_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target.is_none() {
        return Err(Error::type_err(
            "SharedArrayBuffer constructor requires new",
        ));
    }

    let length = match args.first() {
        Some(value) => to_shared_array_buffer_length(vm, value)?,
        None => 0,
    };
    let max_byte_length = match args.get(1) {
        Some(options @ Value::Object(_)) => {
            let value = vm.get_property(options, "maxByteLength")?;
            if value.is_undefined() {
                None
            } else {
                Some(to_shared_array_buffer_length(vm, &value)?)
            }
        }
        _ => None,
    };
    if max_byte_length.is_some_and(|max| length > max) {
        return Err(Error::range("Invalid SharedArrayBuffer length"));
    }
    let proto = native_constructor_prototype_with_default(
        vm,
        "SharedArrayBuffer",
        vm.object_proto.clone(),
    )?;
    if length > MAX_ARRAY_BUFFER_LENGTH
        || max_byte_length.is_some_and(|max| max > MAX_ARRAY_BUFFER_LENGTH)
    {
        return Err(Error::range("Invalid SharedArrayBuffer length"));
    }
    let idx = vm
        .heap
        .allocate(HeapObj::ArrayBuffer(crate::value::ArrayBufferData {
            bytes: Arc::new(Mutex::new(vec![0; length])),
            waiters: Arc::new(Mutex::new(std::collections::HashMap::new())),
            detached: AtomicBool::new(false),
            immutable: AtomicBool::new(false),
            shared: true,
            max_byte_length,
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
        }))?;
    Ok(Value::Object(GcIdx(idx)))
}

pub(crate) fn shared_array_buffer_from_agent_broadcast(
    vm: &mut Vm,
    broadcast: crate::vm::AgentBroadcast,
) -> error::Result<Value> {
    let constructor = crate::environment::get(&vm.heap, vm.global, "SharedArrayBuffer")
        .ok_or_else(|| Error::type_err("SharedArrayBuffer constructor is not available"))?;
    let proto = vm.get_property_by_key(&constructor, &PropertyKey::from("prototype"))?;
    let proto = if matches!(proto, Value::Object(_)) {
        proto
    } else {
        vm.object_proto.clone()
    };
    let idx = vm
        .heap
        .allocate(HeapObj::ArrayBuffer(crate::value::ArrayBufferData {
            bytes: broadcast.bytes,
            waiters: broadcast.waiters,
            detached: AtomicBool::new(false),
            immutable: AtomicBool::new(false),
            shared: true,
            max_byte_length: broadcast.max_byte_length,
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
        }))?;
    Ok(Value::Object(GcIdx(idx)))
}

pub(crate) fn array_buffer_is_view(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let is_view = match args.first() {
        Some(Value::Object(idx)) => vm.heap.with_obj(idx.0, |o| {
            matches!(o, HeapObj::TypedArray(_) | HeapObj::DataView(_))
        }),
        _ => false,
    };
    Ok(Value::Bool(is_view))
}

pub(crate) fn array_buffer_species_get(
    _vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    Ok(this.unwrap_or(Value::Undefined))
}

fn current_realm_array_buffer_constructor(vm: &mut Vm) -> error::Result<Value> {
    let realm_env = vm.native_callee_closure().unwrap_or(vm.global);
    crate::environment::get(&vm.heap, realm_env, "ArrayBuffer")
        .or_else(|| crate::environment::get(&vm.heap, vm.global, "ArrayBuffer"))
        .ok_or_else(|| Error::type_err("ArrayBuffer constructor is not available"))
}

fn current_realm_shared_array_buffer_constructor(vm: &mut Vm) -> error::Result<Value> {
    let realm_env = vm.native_callee_closure().unwrap_or(vm.global);
    crate::environment::get(&vm.heap, realm_env, "SharedArrayBuffer")
        .or_else(|| crate::environment::get(&vm.heap, vm.global, "SharedArrayBuffer"))
        .ok_or_else(|| Error::type_err("SharedArrayBuffer constructor is not available"))
}

fn current_realm_typed_array_constructor(
    vm: &mut Vm,
    kind: crate::value::TypedArrayKind,
) -> error::Result<Value> {
    let closure = vm.native_callee_closure().unwrap_or(vm.global);
    let realm = crate::environment::global_env_root(&vm.heap, closure);
    vm.realm_typed_array_constructors
        .get(&(realm.0, kind))
        .cloned()
        .or_else(|| {
            vm.realm_typed_array_constructors
                .get(&(vm.global.0, kind))
                .cloned()
        })
        .ok_or_else(|| Error::type_err("TypedArray constructor is not available"))
}

fn array_buffer_species_constructor(
    vm: &mut Vm,
    buffer: &Value,
    default_constructor: Value,
) -> error::Result<Value> {
    let constructor = vm.get_property(buffer, "constructor")?;
    if constructor.is_undefined() {
        return Ok(default_constructor);
    }
    if !matches!(constructor, Value::Object(_)) {
        return Err(Error::type_err("ArrayBuffer constructor is not an object"));
    }

    let species_key = PropertyKey::Symbol(vm.well_known_symbols.species);
    let species = vm.get_property_by_key(&constructor, &species_key)?;
    if species.is_undefined() || matches!(species, Value::Null) {
        return Ok(default_constructor);
    }
    if !vm.is_constructor_value(&species) {
        return Err(Error::type_err("ArrayBuffer species is not a constructor"));
    }
    Ok(species)
}

fn typed_array_species_constructor(
    vm: &mut Vm,
    exemplar: &Value,
    default_constructor: Value,
) -> error::Result<Value> {
    let constructor = vm.get_property(exemplar, "constructor")?;
    if constructor.is_undefined() {
        return Ok(default_constructor);
    }
    if !matches!(constructor, Value::Object(_)) {
        return Err(Error::type_err("TypedArray constructor is not an object"));
    }

    let species_key = PropertyKey::Symbol(vm.well_known_symbols.species);
    let species = vm.get_property_by_key(&constructor, &species_key)?;
    if species.is_undefined() || matches!(species, Value::Null) {
        return Ok(default_constructor);
    }
    if !vm.is_constructor_value(&species) {
        return Err(Error::type_err("TypedArray species is not a constructor"));
    }
    Ok(species)
}

fn typed_array_species_create(
    vm: &mut Vm,
    exemplar: &Value,
    source_kind: crate::value::TypedArrayKind,
    length: usize,
    name: &str,
    require_write: bool,
) -> error::Result<Value> {
    let default_constructor = current_realm_typed_array_constructor(vm, source_kind)?;
    let constructor = typed_array_species_constructor(vm, exemplar, default_constructor)?;
    let construct_args = [Value::Number(length as f64)];
    let construct_pin_count =
        vm.pin(exemplar) + vm.pin(&constructor) + vm.pin_many(&construct_args);
    let result = vm.construct(&constructor, &construct_args);
    vm.unpin_many(construct_pin_count);
    let result = result?;

    let (target_kind, target_buffer, target_byte_offset, target_fixed_length, target_tracking) =
        match &result {
            Value::Object(result_idx) => vm.heap.with_obj(result_idx.0, |obj| {
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
        }
        .ok_or_else(|| {
            Error::type_err(format!("TypedArray {name} species returned non-TypedArray"))
        })?;
    let target_buffer = target_buffer
        .ok_or_else(|| Error::type_err(format!("TypedArray {name} result has no ArrayBuffer")))?;
    if require_write && is_immutable_array_buffer(vm, &target_buffer) {
        return Err(Error::type_err(format!(
            "TypedArray {name} species returned an immutable result"
        )));
    }
    let target_byte_length = effective_view_byte_length(
        vm,
        Some(&target_buffer),
        target_byte_offset,
        target_fixed_length,
        target_tracking,
        target_kind.element_size(),
    )
    .ok_or_else(|| Error::type_err(format!("TypedArray {name} result is out of bounds")))?;
    if typed_array_element_count(target_kind, target_byte_length) < length {
        return Err(Error::type_err(format!(
            "TypedArray {name} species returned a shorter result"
        )));
    }
    if typed_array_content_type(target_kind) != typed_array_content_type(source_kind) {
        return Err(Error::type_err(format!(
            "TypedArray {name} species returned incompatible content type"
        )));
    }
    Ok(result)
}

pub(crate) fn array_buffer_slice(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    array_buffer_slice_impl(vm, args, this, false)
}

pub(crate) fn shared_array_buffer_slice(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    array_buffer_slice_impl(vm, args, this, true)
}

fn array_buffer_slice_impl(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    expect_shared: bool,
) -> error::Result<Value> {
    let kind = if expect_shared {
        "SharedArrayBuffer"
    } else {
        "ArrayBuffer"
    };
    let this = this.ok_or_else(|| Error::type_err(format!("{kind} slice called without this")))?;
    let (bytes, detached) = match &this {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::ArrayBuffer(buffer) = o {
                if buffer.shared == expect_shared {
                    return Some((
                        buffer.bytes.lock().clone(),
                        buffer.detached.load(std::sync::atomic::Ordering::Relaxed),
                    ));
                }
            }
            None
        }),
        _ => None,
    }
    .ok_or_else(|| Error::type_err(format!("{kind}.prototype.slice called on wrong receiver")))?;
    if detached {
        return Err(Error::type_err(format!(
            "{kind}.prototype.slice on detached buffer"
        )));
    }

    let len = bytes.len();
    let (from, to) = resolve_slice_bounds(vm, len, args.first(), args.get(1))?;
    let count = to - from;

    let default_ctor = if expect_shared {
        current_realm_shared_array_buffer_constructor(vm)?
    } else {
        current_realm_array_buffer_constructor(vm)?
    };
    let ctor = array_buffer_species_constructor(vm, &this, default_ctor)?;
    let result = vm.construct(&ctor, &[Value::Number(count as f64)])?;
    if result == this {
        return Err(Error::type_err("buffer species returned the source buffer"));
    }

    let (result_len, result_detached) = array_buffer_len_and_detached(vm, &result)
        .ok_or_else(|| Error::type_err("buffer species did not return a buffer"))?;
    if array_buffer_is_shared(vm, &result) != Some(expect_shared) {
        return Err(Error::type_err(
            "buffer species returned the wrong buffer brand",
        ));
    }
    if result_detached {
        return Err(Error::type_err("buffer species returned a detached buffer"));
    }
    let (_, _, result_immutable) = array_buffer_slots(vm, &result)
        .ok_or_else(|| Error::type_err("ArrayBuffer species did not return an ArrayBuffer"))?;
    if result_immutable {
        return Err(Error::type_err(
            "buffer species returned an immutable buffer",
        ));
    }
    if result_len < count {
        return Err(Error::type_err(
            "buffer species returned a buffer that is too small",
        ));
    }

    let Value::Object(idx) = &result else {
        return Err(Error::type_err("buffer species did not return an object"));
    };
    vm.heap.with_obj(idx.0, |o| {
        if let HeapObj::ArrayBuffer(buffer) = o {
            buffer.bytes.lock()[..count].copy_from_slice(&bytes[from..to]);
        }
    });
    Ok(result)
}

pub(crate) fn array_buffer_immutable_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("ArrayBuffer immutable getter needs this"))?;
    match this {
        Value::Object(idx) => vm
            .heap
            .with_obj(idx.0, |o| {
                if let HeapObj::ArrayBuffer(buffer) = o {
                    if !buffer.shared {
                        return Some(Value::Bool(
                            buffer.immutable.load(std::sync::atomic::Ordering::Relaxed),
                        ));
                    }
                }
                None
            })
            .ok_or_else(|| Error::type_err("ArrayBuffer immutable getter on non-ArrayBuffer")),
        _ => Err(Error::type_err(
            "ArrayBuffer immutable getter on non-object",
        )),
    }
}

fn ordinary_array_buffer_slots(
    vm: &Vm,
    this: Option<Value>,
    accessor: &str,
) -> error::Result<(GcIdx, Option<usize>, bool)> {
    let Value::Object(idx) = this.unwrap_or(Value::Undefined) else {
        return Err(Error::type_err(format!(
            "ArrayBuffer {accessor} called on non-object"
        )));
    };
    vm.heap
        .with_obj(idx.0, |obj| {
            let HeapObj::ArrayBuffer(buffer) = obj else {
                return None;
            };
            (!buffer.shared).then_some((
                idx,
                buffer.max_byte_length,
                buffer.detached.load(std::sync::atomic::Ordering::Relaxed),
            ))
        })
        .ok_or_else(|| Error::type_err(format!("ArrayBuffer {accessor} called on wrong receiver")))
}

pub(crate) fn array_buffer_resizable_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, max_byte_length, _) = ordinary_array_buffer_slots(vm, this, "resizable getter")?;
    Ok(Value::Bool(max_byte_length.is_some()))
}

pub(crate) fn array_buffer_max_byte_length_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (idx, max_byte_length, detached) =
        ordinary_array_buffer_slots(vm, this, "maxByteLength getter")?;
    if detached {
        return Ok(Value::Number(0.0));
    }
    let length = match max_byte_length {
        Some(length) => length,
        None => vm.heap.with_obj(idx.0, |obj| {
            let HeapObj::ArrayBuffer(buffer) = obj else {
                return 0;
            };
            buffer.bytes.lock().len()
        }),
    };
    Ok(Value::Number(length as f64))
}

pub(crate) fn array_buffer_resize(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (idx, max_byte_length, _) = ordinary_array_buffer_slots(vm, this, "resize")?;
    let max_byte_length =
        max_byte_length.ok_or_else(|| Error::type_err("ArrayBuffer is not resizable"))?;
    let new_byte_length =
        to_shared_array_buffer_length(vm, args.first().unwrap_or(&Value::Undefined))?;
    if new_byte_length > max_byte_length {
        return Err(Error::range("Invalid ArrayBuffer resize length"));
    }
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::ArrayBuffer(buffer) = obj else {
            return Err(Error::type_err("Invalid ArrayBuffer receiver"));
        };
        if buffer.detached.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(Error::type_err("ArrayBuffer resize on detached buffer"));
        }
        if buffer.immutable.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(Error::type_err("ArrayBuffer resize on immutable buffer"));
        }
        buffer.bytes.lock().resize(new_byte_length, 0);
        Ok(Value::Undefined)
    })
}

pub(crate) fn array_buffer_detached_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("ArrayBuffer detached getter needs this"))?;
    match this {
        Value::Object(idx) => vm
            .heap
            .with_obj(idx.0, |o| {
                if let HeapObj::ArrayBuffer(buffer) = o {
                    if !buffer.shared {
                        return Some(Value::Bool(
                            buffer.detached.load(std::sync::atomic::Ordering::Relaxed),
                        ));
                    }
                }
                None
            })
            .ok_or_else(|| Error::type_err("ArrayBuffer detached getter on non-ArrayBuffer")),
        _ => Err(Error::type_err("ArrayBuffer detached getter on non-object")),
    }
}

pub(crate) fn array_buffer_transfer(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    array_buffer_copy_and_detach(vm, args, this, false, true)
}

pub(crate) fn array_buffer_transfer_to_fixed_length(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    array_buffer_copy_and_detach(vm, args, this, false, false)
}

pub(crate) fn array_buffer_transfer_to_immutable(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    array_buffer_copy_and_detach(vm, args, this, true, false)
}

pub(crate) fn array_buffer_slice_to_immutable(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this =
        this.ok_or_else(|| Error::type_err("ArrayBuffer sliceToImmutable called without this"))?;
    let mut pins = Vec::with_capacity(args.len() + 1);
    pins.push(this.clone());
    pins.extend_from_slice(args);
    let pin_count = vm.pin_many(&pins);
    let result = array_buffer_slice_to_immutable_pinned(vm, args, this);
    vm.unpin_many(pin_count);
    result
}

fn array_buffer_slice_to_immutable_pinned(
    vm: &mut Vm,
    args: &[Value],
    this: Value,
) -> error::Result<Value> {
    if array_buffer_is_shared(vm, &this) != Some(false) {
        return Err(Error::type_err(
            "ArrayBuffer.prototype.sliceToImmutable called on non-ArrayBuffer",
        ));
    }
    let (len, detached) = array_buffer_len_and_detached(vm, &this).ok_or_else(|| {
        Error::type_err("ArrayBuffer.prototype.sliceToImmutable called on non-ArrayBuffer")
    })?;
    if detached {
        return Err(Error::type_err(
            "ArrayBuffer.prototype.sliceToImmutable on detached buffer",
        ));
    }

    let (from, to) = resolve_slice_bounds(vm, len, args.first(), args.get(1))?;
    let count = to - from;
    let (current_len, detached) = array_buffer_len_and_detached(vm, &this).ok_or_else(|| {
        Error::type_err("ArrayBuffer.prototype.sliceToImmutable called on non-ArrayBuffer")
    })?;
    if detached {
        return Err(Error::type_err(
            "ArrayBuffer.prototype.sliceToImmutable on detached buffer",
        ));
    }
    if current_len < to {
        return Err(Error::range(
            "ArrayBuffer sliceToImmutable source is too small",
        ));
    }

    let bytes = match &this {
        Value::Object(idx) => vm
            .heap
            .with_obj(idx.0, |o| {
                if let HeapObj::ArrayBuffer(buffer) = o {
                    Some(buffer.bytes.lock()[from..to].to_vec())
                } else {
                    None
                }
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    allocate_array_buffer_with_bytes_and_immutable(vm, bytes, true)
}

pub(crate) fn array_buffer_byte_length_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("ArrayBuffer byteLength getter needs this"))?;
    match this {
        Value::Object(idx) => vm
            .heap
            .with_obj(idx.0, |o| {
                if let HeapObj::ArrayBuffer(buffer) = o {
                    if !buffer.shared {
                        return if buffer.detached.load(std::sync::atomic::Ordering::Relaxed) {
                            Some(Value::Number(0.0))
                        } else {
                            Some(Value::Number(buffer.bytes.lock().len() as f64))
                        };
                    }
                }
                None
            })
            .ok_or_else(|| Error::type_err("ArrayBuffer byteLength getter on non-ArrayBuffer")),
        _ => Err(Error::type_err(
            "ArrayBuffer byteLength getter on non-object",
        )),
    }
}

pub(crate) fn shared_array_buffer_byte_length_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this =
        this.ok_or_else(|| Error::type_err("SharedArrayBuffer byteLength getter needs this"))?;
    match this {
        Value::Object(idx) => vm
            .heap
            .with_obj(idx.0, |o| {
                if let HeapObj::ArrayBuffer(buffer) = o {
                    if buffer.shared {
                        return Some(Value::Number(buffer.bytes.lock().len() as f64));
                    }
                }
                None
            })
            .ok_or_else(|| {
                Error::type_err("SharedArrayBuffer byteLength getter on wrong receiver")
            }),
        _ => Err(Error::type_err(
            "SharedArrayBuffer byteLength getter on non-object",
        )),
    }
}

fn shared_array_buffer_slots(
    vm: &Vm,
    this: Option<Value>,
    accessor: &str,
) -> error::Result<(GcIdx, Option<usize>)> {
    let Value::Object(idx) = this.unwrap_or(Value::Undefined) else {
        return Err(Error::type_err(format!(
            "SharedArrayBuffer {accessor} called on non-object"
        )));
    };
    vm.heap
        .with_obj(idx.0, |obj| {
            let HeapObj::ArrayBuffer(buffer) = obj else {
                return None;
            };
            buffer.shared.then_some((idx, buffer.max_byte_length))
        })
        .ok_or_else(|| {
            Error::type_err(format!(
                "SharedArrayBuffer {accessor} called on wrong receiver"
            ))
        })
}

pub(crate) fn shared_array_buffer_growable_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, max_byte_length) = shared_array_buffer_slots(vm, this, "growable getter")?;
    Ok(Value::Bool(max_byte_length.is_some()))
}

pub(crate) fn shared_array_buffer_max_byte_length_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (idx, max_byte_length) = shared_array_buffer_slots(vm, this, "maxByteLength getter")?;
    let length = match max_byte_length {
        Some(length) => length,
        None => vm.heap.with_obj(idx.0, |obj| {
            let HeapObj::ArrayBuffer(buffer) = obj else {
                return 0;
            };
            buffer.bytes.lock().len()
        }),
    };
    Ok(Value::Number(length as f64))
}

pub(crate) fn shared_array_buffer_grow(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (idx, max_byte_length) = shared_array_buffer_slots(vm, this, "grow")?;
    let max_byte_length =
        max_byte_length.ok_or_else(|| Error::type_err("SharedArrayBuffer is not growable"))?;
    let new_byte_length =
        to_shared_array_buffer_length(vm, args.first().unwrap_or(&Value::Undefined))?;
    if new_byte_length > max_byte_length {
        return Err(Error::range("Invalid SharedArrayBuffer grow length"));
    }
    vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::ArrayBuffer(buffer) = obj else {
            return Err(Error::type_err("Invalid SharedArrayBuffer receiver"));
        };
        let mut bytes = buffer.bytes.lock();
        if new_byte_length < bytes.len() {
            return Err(Error::range("SharedArrayBuffer cannot shrink"));
        }
        bytes.resize(new_byte_length, 0);
        Ok(Value::Undefined)
    })
}

#[derive(Clone, Copy)]
struct AtomicView {
    kind: crate::value::TypedArrayKind,
    buffer: GcIdx,
    byte_offset: usize,
    length: usize,
}

#[derive(Clone, Copy)]
enum AtomicRmwOp {
    Add,
    Sub,
    And,
    Or,
    Xor,
    Exchange,
}

fn validate_atomic_view(
    vm: &Vm,
    value: &Value,
    allow_immutable: bool,
) -> error::Result<AtomicView> {
    let Value::Object(idx) = value else {
        return Err(Error::type_err("Atomics operation requires a TypedArray"));
    };
    let slots = vm.heap.with_obj(idx.0, |obj| {
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
    });
    let (kind, buffer, byte_offset, byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("Atomics operation requires a TypedArray"))?;
    if matches!(
        kind,
        crate::value::TypedArrayKind::Uint8Clamped
            | crate::value::TypedArrayKind::Float32
            | crate::value::TypedArrayKind::Float64
    ) {
        return Err(Error::type_err(
            "Atomics operation requires an integer TypedArray",
        ));
    }
    let Value::Object(buffer) = buffer
        .ok_or_else(|| Error::type_err("Atomics operation requires a shared backing buffer"))?
    else {
        return Err(Error::type_err(
            "Atomics operation requires a shared backing buffer",
        ));
    };
    let usable = vm.heap.with_obj(buffer.0, |obj| {
        matches!(
            obj,
            HeapObj::ArrayBuffer(data)
                if !data.detached.load(std::sync::atomic::Ordering::Relaxed)
                    && (allow_immutable
                        || !data.immutable.load(std::sync::atomic::Ordering::Relaxed))
        )
    });
    if !usable {
        return Err(Error::type_err(
            "Atomics operation requires a mutable ArrayBuffer backing store",
        ));
    }
    let byte_length = effective_view_byte_length(
        vm,
        Some(&Value::Object(buffer)),
        byte_offset,
        byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("Atomics operation requires an in-bounds TypedArray"))?;
    Ok(AtomicView {
        kind,
        buffer,
        byte_offset,
        length: typed_array_element_count(kind, byte_length),
    })
}

fn atomic_index(vm: &mut Vm, value: &Value, length: usize) -> error::Result<usize> {
    let number = vm.to_number(value)?;
    let integer = if number.is_nan() { 0.0 } else { number.trunc() };
    if !integer.is_finite() || integer < 0.0 || integer > MAX_SAFE_INTEGER {
        return Err(Error::range("Invalid atomic access index"));
    }
    let index = integer as usize;
    if index >= length {
        return Err(Error::range("Atomic access index is out of bounds"));
    }
    Ok(index)
}

fn atomic_operand(
    vm: &mut Vm,
    kind: crate::value::TypedArrayKind,
    value: &Value,
) -> error::Result<(Value, Vec<u8>)> {
    let converted = match kind {
        crate::value::TypedArrayKind::BigInt64 | crate::value::TypedArrayKind::BigUint64 => {
            Value::BigInt(vm.to_bigint(value)?)
        }
        _ => {
            let number = vm.to_number(value)?;
            Value::Number(if number.is_nan() || number == 0.0 {
                0.0
            } else {
                number.trunc()
            })
        }
    };
    let bytes = typed_array_value_to_bytes(vm, kind, &converted)?;
    Ok((converted, bytes))
}

fn atomic_location(view: AtomicView, index: usize) -> error::Result<(usize, usize)> {
    let size = view.kind.element_size();
    let offset = view
        .byte_offset
        .checked_add(
            index
                .checked_mul(size)
                .ok_or_else(|| Error::range("Invalid atomic access index"))?,
        )
        .ok_or_else(|| Error::range("Invalid atomic access index"))?;
    let end = offset
        .checked_add(size)
        .ok_or_else(|| Error::range("Invalid atomic access index"))?;
    Ok((offset, end))
}

fn raw_atomic_value(bytes: &[u8]) -> u64 {
    let mut raw = [0_u8; 8];
    raw[..bytes.len()].copy_from_slice(bytes);
    u64::from_ne_bytes(raw)
}

fn write_raw_atomic_value(bytes: &mut [u8], value: u64) {
    bytes.copy_from_slice(&value.to_ne_bytes()[..bytes.len()]);
}

fn atomic_load_impl(vm: &mut Vm, args: &[Value]) -> error::Result<Value> {
    let view = validate_atomic_view(vm, args.first().unwrap_or(&Value::Undefined), true)?;
    let index = atomic_index(vm, args.get(1).unwrap_or(&Value::Undefined), view.length)?;
    let (offset, end) = atomic_location(view, index)?;
    vm.heap.with_obj(view.buffer.0, |obj| {
        let HeapObj::ArrayBuffer(buffer) = obj else {
            return Err(Error::type_err("Invalid shared backing buffer"));
        };
        let bytes = buffer.bytes.lock();
        if end > bytes.len() {
            return Err(Error::range("Atomic access index is out of bounds"));
        }
        typed_array_read_element(view.kind, &bytes[offset..end], 0)
            .ok_or_else(|| Error::range("Atomic access index is out of bounds"))
    })
}

fn atomic_store_impl(vm: &mut Vm, args: &[Value]) -> error::Result<Value> {
    let view = validate_atomic_view(vm, args.first().unwrap_or(&Value::Undefined), false)?;
    let index = atomic_index(vm, args.get(1).unwrap_or(&Value::Undefined), view.length)?;
    let (converted, element) =
        atomic_operand(vm, view.kind, args.get(2).unwrap_or(&Value::Undefined))?;
    let (offset, end) = atomic_location(view, index)?;
    vm.heap.with_obj(view.buffer.0, |obj| {
        let HeapObj::ArrayBuffer(buffer) = obj else {
            return Err(Error::type_err("Invalid shared backing buffer"));
        };
        let mut bytes = buffer.bytes.lock();
        if end > bytes.len() {
            return Err(Error::range("Atomic access index is out of bounds"));
        }
        bytes[offset..end].copy_from_slice(&element);
        Ok(converted)
    })
}

fn atomic_rmw_impl(vm: &mut Vm, args: &[Value], operation: AtomicRmwOp) -> error::Result<Value> {
    let view = validate_atomic_view(vm, args.first().unwrap_or(&Value::Undefined), false)?;
    let index = atomic_index(vm, args.get(1).unwrap_or(&Value::Undefined), view.length)?;
    let (_, operand) = atomic_operand(vm, view.kind, args.get(2).unwrap_or(&Value::Undefined))?;
    let (offset, end) = atomic_location(view, index)?;
    vm.heap.with_obj(view.buffer.0, |obj| {
        let HeapObj::ArrayBuffer(buffer) = obj else {
            return Err(Error::type_err("Invalid shared backing buffer"));
        };
        let mut bytes = buffer.bytes.lock();
        if end > bytes.len() {
            return Err(Error::range("Atomic access index is out of bounds"));
        }
        let target = &mut bytes[offset..end];
        let old = typed_array_read_element(view.kind, target, 0)
            .ok_or_else(|| Error::range("Atomic access index is out of bounds"))?;
        let current = raw_atomic_value(target);
        let operand = raw_atomic_value(&operand);
        let next = match operation {
            AtomicRmwOp::Add => current.wrapping_add(operand),
            AtomicRmwOp::Sub => current.wrapping_sub(operand),
            AtomicRmwOp::And => current & operand,
            AtomicRmwOp::Or => current | operand,
            AtomicRmwOp::Xor => current ^ operand,
            AtomicRmwOp::Exchange => operand,
        };
        write_raw_atomic_value(target, next);
        Ok(old)
    })
}

fn atomic_compare_exchange_impl(vm: &mut Vm, args: &[Value]) -> error::Result<Value> {
    let view = validate_atomic_view(vm, args.first().unwrap_or(&Value::Undefined), false)?;
    let index = atomic_index(vm, args.get(1).unwrap_or(&Value::Undefined), view.length)?;
    let (_, expected) = atomic_operand(vm, view.kind, args.get(2).unwrap_or(&Value::Undefined))?;
    let (_, replacement) = atomic_operand(vm, view.kind, args.get(3).unwrap_or(&Value::Undefined))?;
    let (offset, end) = atomic_location(view, index)?;
    vm.heap.with_obj(view.buffer.0, |obj| {
        let HeapObj::ArrayBuffer(buffer) = obj else {
            return Err(Error::type_err("Invalid shared backing buffer"));
        };
        let mut bytes = buffer.bytes.lock();
        if end > bytes.len() {
            return Err(Error::range("Atomic access index is out of bounds"));
        }
        let target = &mut bytes[offset..end];
        let old = typed_array_read_element(view.kind, target, 0)
            .ok_or_else(|| Error::range("Atomic access index is out of bounds"))?;
        if target == expected.as_slice() {
            target.copy_from_slice(&replacement);
        }
        Ok(old)
    })
}

fn validate_waitable_view(
    vm: &Vm,
    value: &Value,
    require_shared: bool,
) -> error::Result<(AtomicView, bool)> {
    let view = validate_atomic_view(vm, value, true)?;
    if !matches!(
        view.kind,
        crate::value::TypedArrayKind::Int32 | crate::value::TypedArrayKind::BigInt64
    ) {
        return Err(Error::type_err(
            "Atomics wait operation requires Int32Array or BigInt64Array",
        ));
    }
    let shared = vm.heap.with_obj(
        view.buffer.0,
        |obj| matches!(obj, HeapObj::ArrayBuffer(buffer) if buffer.shared),
    );
    if require_shared && !shared {
        return Err(Error::type_err("Atomics.wait requires a SharedArrayBuffer"));
    }
    Ok((view, shared))
}

fn atomics_count(vm: &mut Vm, value: Option<&Value>) -> error::Result<usize> {
    let Some(value) = value else {
        return Ok(usize::MAX);
    };
    if value.is_undefined() {
        return Ok(usize::MAX);
    }
    let number = vm.to_number(value)?;
    if number.is_nan() || number <= 0.0 {
        return Ok(0);
    }
    if number.is_infinite() || number >= usize::MAX as f64 {
        return Ok(usize::MAX);
    }
    Ok(number.trunc() as usize)
}

pub(crate) fn atomics_notify(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let (view, shared) =
        validate_waitable_view(vm, args.first().unwrap_or(&Value::Undefined), false)?;
    let index = atomic_index(vm, args.get(1).unwrap_or(&Value::Undefined), view.length)?;
    let count = atomics_count(vm, args.get(2))?;
    if !shared || count == 0 {
        return Ok(Value::Number(0.0));
    }
    let (offset, _) = atomic_location(view, index)?;
    let waiters = vm.heap.with_obj(view.buffer.0, |obj| {
        let HeapObj::ArrayBuffer(buffer) = obj else {
            return None;
        };
        Some(buffer.waiters.clone())
    });
    let waiters = waiters.ok_or_else(|| Error::type_err("Invalid shared backing buffer"))?;
    let mut lists = waiters.lock();
    let Some(queue) = lists.get_mut(&offset) else {
        return Ok(Value::Number(0.0));
    };
    let mut notified = 0_usize;
    while notified < count {
        let Some(waiter) = queue.pop_front() else {
            break;
        };
        *waiter.notified.lock() = true;
        waiter.wake.notify_one();
        notified += 1;
    }
    if queue.is_empty() {
        lists.remove(&offset);
    }
    Ok(Value::Number(notified as f64))
}

pub(crate) fn atomics_wait(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let (view, _) = validate_waitable_view(vm, args.first().unwrap_or(&Value::Undefined), true)?;
    let index = atomic_index(vm, args.get(1).unwrap_or(&Value::Undefined), view.length)?;
    let (_, expected) = atomic_operand(vm, view.kind, args.get(2).unwrap_or(&Value::Undefined))?;
    let timeout = match args.get(3) {
        None => f64::INFINITY,
        Some(value) => {
            let number = vm.to_number(value)?;
            if number.is_nan() {
                f64::INFINITY
            } else {
                number.max(0.0)
            }
        }
    };
    let (offset, end) = atomic_location(view, index)?;
    let (bytes, waiters) = vm
        .heap
        .with_obj(view.buffer.0, |obj| {
            let HeapObj::ArrayBuffer(buffer) = obj else {
                return None;
            };
            Some((buffer.bytes.clone(), buffer.waiters.clone()))
        })
        .ok_or_else(|| Error::type_err("Invalid shared backing buffer"))?;

    let mut lists = waiters.lock();
    {
        let bytes = bytes.lock();
        if end > bytes.len() {
            return Err(Error::range("Atomic access index is out of bounds"));
        }
        if bytes[offset..end] != expected {
            return Ok(Value::String(Arc::from("not-equal")));
        }
    }
    if !vm.agent_can_block {
        return Err(Error::type_err("This agent cannot suspend in Atomics.wait"));
    }
    if timeout == 0.0 {
        return Ok(Value::String(Arc::from("timed-out")));
    }

    let waiter = Arc::new(crate::value::AtomicsWaiter {
        notified: Mutex::new(false),
        wake: parking_lot::Condvar::new(),
    });
    lists.entry(offset).or_default().push_back(waiter.clone());
    drop(lists);

    let mut notified = waiter.notified.lock();
    if timeout.is_infinite() {
        while !*notified {
            waiter.wake.wait(&mut notified);
        }
    } else {
        let duration = std::time::Duration::from_secs_f64(timeout / 1000.0);
        let result = waiter.wake.wait_for(&mut notified, duration);
        if result.timed_out() && !*notified {
            drop(notified);
            let mut lists = waiters.lock();
            if let Some(queue) = lists.get_mut(&offset) {
                queue.retain(|entry| !Arc::ptr_eq(entry, &waiter));
                if queue.is_empty() {
                    lists.remove(&offset);
                }
            }
            return Ok(Value::String(Arc::from("timed-out")));
        }
    }
    Ok(Value::String(Arc::from("ok")))
}

fn atomics_wait_async_result(
    vm: &mut Vm,
    asynchronous: bool,
    value: Value,
) -> error::Result<Value> {
    let result = vm.new_object()?;
    vm.heap.with_obj(result.0, |obj| {
        let mut props = obj.props().lock();
        props.insert(
            PropertyKey::from("async"),
            PropertyDescriptor::data(Value::Bool(asynchronous)),
        );
        props.insert(PropertyKey::from("value"), PropertyDescriptor::data(value));
    });
    Ok(Value::Object(result))
}

pub(crate) fn atomics_wait_async(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let (view, _) = validate_waitable_view(vm, args.first().unwrap_or(&Value::Undefined), true)?;
    let index = atomic_index(vm, args.get(1).unwrap_or(&Value::Undefined), view.length)?;
    let (_, expected) = atomic_operand(vm, view.kind, args.get(2).unwrap_or(&Value::Undefined))?;
    let timeout = match args.get(3) {
        None => f64::INFINITY,
        Some(value) => {
            let number = vm.to_number(value)?;
            if number.is_nan() {
                f64::INFINITY
            } else {
                number.max(0.0)
            }
        }
    };
    let (offset, end) = atomic_location(view, index)?;
    let (bytes, waiters) = vm
        .heap
        .with_obj(view.buffer.0, |obj| {
            let HeapObj::ArrayBuffer(buffer) = obj else {
                return None;
            };
            Some((buffer.bytes.clone(), buffer.waiters.clone()))
        })
        .ok_or_else(|| Error::type_err("Invalid shared backing buffer"))?;

    let mut lists = waiters.lock();
    {
        let bytes = bytes.lock();
        if end > bytes.len() {
            return Err(Error::range("Atomic access index is out of bounds"));
        }
        if bytes[offset..end] != expected {
            return atomics_wait_async_result(vm, false, Value::String(Arc::from("not-equal")));
        }
    }
    if timeout == 0.0 {
        return atomics_wait_async_result(vm, false, Value::String(Arc::from("timed-out")));
    }

    let constructor = vm.current_realm_promise_constructor();
    let capability = new_promise_capability(vm, constructor)?;
    let waiter = Arc::new(crate::value::AtomicsWaiter {
        notified: Mutex::new(false),
        wake: parking_lot::Condvar::new(),
    });
    lists.entry(offset).or_default().push_back(waiter.clone());
    drop(lists);

    let external_jobs = vm.external_jobs.clone();
    let wait_id = {
        let mut external = external_jobs.lock();
        let wait_id = external.next_wait_id;
        external.next_wait_id = external.next_wait_id.wrapping_add(1);
        external
            .wait_roots
            .insert(wait_id, capability.resolve.clone());
        wait_id
    };
    let waiters_for_thread = waiters.clone();
    let waiter_for_thread = waiter.clone();
    let spawn_result = std::thread::Builder::new()
        .name("ruja-atomics-wait-async".to_string())
        .spawn(move || {
            let mut notified = waiter_for_thread.notified.lock();
            let outcome = if timeout.is_infinite() {
                while !*notified {
                    waiter_for_thread.wake.wait(&mut notified);
                }
                "ok"
            } else {
                let duration = std::time::Duration::from_secs_f64(timeout / 1000.0);
                let result = waiter_for_thread.wake.wait_for(&mut notified, duration);
                if result.timed_out() && !*notified {
                    drop(notified);
                    let mut lists = waiters_for_thread.lock();
                    if let Some(queue) = lists.get_mut(&offset) {
                        queue.retain(|entry| !Arc::ptr_eq(entry, &waiter_for_thread));
                        if queue.is_empty() {
                            lists.remove(&offset);
                        }
                    }
                    "timed-out"
                } else {
                    "ok"
                }
            };
            let mut external = external_jobs.lock();
            if let Some(resolve) = external.wait_roots.remove(&wait_id) {
                external.jobs.push_back(crate::vm::ExternalPromiseJob {
                    resolve,
                    value: Value::String(Arc::from(outcome)),
                });
            }
        });
    if let Err(error) = spawn_result {
        let mut lists = waiters.lock();
        if let Some(queue) = lists.get_mut(&offset) {
            queue.retain(|entry| !Arc::ptr_eq(entry, &waiter));
            if queue.is_empty() {
                lists.remove(&offset);
            }
        }
        vm.external_jobs.lock().wait_roots.remove(&wait_id);
        return Err(Error::internal(format!(
            "failed to spawn Atomics.waitAsync waiter: {error}"
        )));
    }
    atomics_wait_async_result(vm, true, capability.promise)
}

pub(crate) fn atomics_add(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    atomic_rmw_impl(vm, args, AtomicRmwOp::Add)
}

pub(crate) fn atomics_and(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    atomic_rmw_impl(vm, args, AtomicRmwOp::And)
}

pub(crate) fn atomics_compare_exchange(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    atomic_compare_exchange_impl(vm, args)
}

pub(crate) fn atomics_exchange(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    atomic_rmw_impl(vm, args, AtomicRmwOp::Exchange)
}

pub(crate) fn atomics_is_lock_free(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let number = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    let size = if number.is_nan() { 0.0 } else { number.trunc() };
    Ok(Value::Bool(matches!(size as i64, 1 | 2 | 4 | 8)))
}

pub(crate) fn atomics_load(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    atomic_load_impl(vm, args)
}

pub(crate) fn atomics_or(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    atomic_rmw_impl(vm, args, AtomicRmwOp::Or)
}

pub(crate) fn atomics_pause(
    _vm: &mut Vm,
    _args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::Undefined)
}

pub(crate) fn atomics_store(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    atomic_store_impl(vm, args)
}

pub(crate) fn atomics_sub(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    atomic_rmw_impl(vm, args, AtomicRmwOp::Sub)
}

pub(crate) fn atomics_xor(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    atomic_rmw_impl(vm, args, AtomicRmwOp::Xor)
}

pub(crate) fn install_atomics_in_env(
    vm: &mut Vm,
    env: GcIdx,
    global: Option<&Value>,
) -> error::Result<Value> {
    let entries: &[(&str, NativeFn, usize)] = &[
        ("add", atomics_add, 3),
        ("and", atomics_and, 3),
        ("compareExchange", atomics_compare_exchange, 4),
        ("exchange", atomics_exchange, 3),
        ("isLockFree", atomics_is_lock_free, 1),
        ("load", atomics_load, 2),
        ("notify", atomics_notify, 3),
        ("or", atomics_or, 3),
        ("pause", atomics_pause, 0),
        ("store", atomics_store, 3),
        ("sub", atomics_sub, 3),
        ("wait", atomics_wait, 4),
        ("waitAsync", atomics_wait_async, 4),
        ("xor", atomics_xor, 3),
    ];
    let function_proto = vm
        .realm_function_prototypes
        .get(&env.0)
        .cloned()
        .unwrap_or_else(|| vm.function_proto.clone());
    let object_proto = vm
        .realm_object_prototypes
        .get(&env.0)
        .cloned()
        .unwrap_or_else(|| vm.object_proto.clone());
    let mut props = IndexMap::new();
    for (name, function, length) in entries {
        let function = vm.new_native_function_in_env(name, *function, *length, env)?;
        set_function_object_proto(vm, function, &function_proto);
        props.insert(PropertyKey::from(*name), data_prop(Value::Object(function)));
    }
    let mut tag = data_prop(Value::String(Arc::from("Atomics")));
    tag.writable = false;
    props.insert(
        PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
        tag,
    );
    let atomics = Value::Object(GcIdx(vm.heap.allocate(HeapObj::Object(ObjectData {
        props: Mutex::new(props),
        proto: Mutex::new(Some(object_proto)),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Atomics")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    }))?));
    if let Some(global) = global {
        define_realm_global(vm, env, global, "Atomics", atomics.clone());
    } else {
        define_global(vm, "Atomics", atomics.clone());
    }
    Ok(atomics)
}

pub(crate) fn data_view_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target.is_none() {
        return Err(Error::type_err("DataView constructor requires new"));
    }

    let buffer = args.first().cloned().unwrap_or(Value::Undefined);
    let is_array_buffer = match &buffer {
        Value::Object(idx) => vm
            .heap
            .with_obj(idx.0, |o| matches!(o, HeapObj::ArrayBuffer(_))),
        _ => false,
    };
    if !is_array_buffer {
        return Err(Error::type_err("DataView buffer must be an ArrayBuffer"));
    }

    let byte_offset = match args.get(1) {
        Some(value) => to_index_length(vm, value, "DataView byteOffset")?,
        None => 0,
    };

    let (mut buffer_len, detached) = array_buffer_len_and_detached(vm, &buffer)
        .ok_or_else(|| Error::type_err("DataView buffer must be an ArrayBuffer"))?;
    if detached {
        return Err(Error::type_err("DataView buffer is detached"));
    }
    if byte_offset > buffer_len {
        return Err(Error::range("Invalid DataView byteOffset"));
    }
    let byte_length = match args.get(2) {
        Some(Value::Undefined) | None => buffer_len - byte_offset,
        Some(value) => {
            let byte_length = to_index_length(vm, value, "DataView byteLength")?;
            let (current_len, detached) = array_buffer_len_and_detached(vm, &buffer)
                .ok_or_else(|| Error::type_err("DataView buffer must be an ArrayBuffer"))?;
            if detached {
                return Err(Error::type_err("DataView buffer is detached"));
            }
            buffer_len = current_len;
            byte_length
        }
    };
    let length_tracking = args.get(2).is_none_or(Value::is_undefined)
        && match &buffer {
            Value::Object(idx) => vm.heap.with_obj(
                idx.0,
                |obj| matches!(obj, HeapObj::ArrayBuffer(data) if data.max_byte_length.is_some()),
            ),
            _ => false,
        };
    if byte_offset
        .checked_add(byte_length)
        .is_none_or(|end| end > buffer_len)
    {
        return Err(Error::range("Invalid DataView byteLength"));
    }

    let proto = native_constructor_prototype_with_default(vm, "DataView", vm.object_proto.clone())?;
    let (current_len, detached) = array_buffer_len_and_detached(vm, &buffer)
        .ok_or_else(|| Error::type_err("DataView buffer must be an ArrayBuffer"))?;
    if detached {
        return Err(Error::type_err("DataView buffer is detached"));
    }
    let in_bounds = if length_tracking {
        byte_offset <= current_len
    } else {
        byte_offset
            .checked_add(byte_length)
            .is_some_and(|end| end <= current_len)
    };
    if !in_bounds {
        return Err(Error::range("DataView buffer resized out of bounds"));
    }
    let idx = vm
        .heap
        .allocate(HeapObj::DataView(crate::value::DataViewData {
            buffer,
            byte_offset,
            byte_length,
            length_tracking,
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
        }))?;
    Ok(Value::Object(GcIdx(idx)))
}

fn array_buffer_len_and_detached(vm: &Vm, value: &Value) -> Option<(usize, bool)> {
    match value {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::ArrayBuffer(buffer) = o {
                Some((
                    buffer.bytes.lock().len(),
                    buffer.detached.load(std::sync::atomic::Ordering::Relaxed),
                ))
            } else {
                None
            }
        }),
        _ => None,
    }
}

fn array_buffer_slots(vm: &Vm, value: &Value) -> Option<(usize, bool, bool)> {
    match value {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::ArrayBuffer(buffer) = o {
                Some((
                    buffer.bytes.lock().len(),
                    buffer.detached.load(std::sync::atomic::Ordering::Relaxed),
                    buffer.immutable.load(std::sync::atomic::Ordering::Relaxed),
                ))
            } else {
                None
            }
        }),
        _ => None,
    }
}

fn array_buffer_is_shared(vm: &Vm, value: &Value) -> Option<bool> {
    match value {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::ArrayBuffer(buffer) = o {
                Some(buffer.shared)
            } else {
                None
            }
        }),
        _ => None,
    }
}

fn resolve_slice_bounds(
    vm: &mut Vm,
    len: usize,
    start: Option<&Value>,
    end: Option<&Value>,
) -> error::Result<(usize, usize)> {
    let from = match start {
        Some(value) => slice_bound(vm, value, len)?,
        None => 0,
    };
    let to = match end {
        Some(Value::Undefined) | None => len,
        Some(value) => slice_bound(vm, value, len)?,
    };
    Ok((from, to.max(from)))
}

fn slice_bound(vm: &mut Vm, value: &Value, len: usize) -> error::Result<usize> {
    let n = vm.to_number(value)?;
    if n.is_nan() {
        return Ok(0);
    }
    if n == f64::INFINITY {
        return Ok(len);
    }
    if n == f64::NEG_INFINITY {
        return Ok(0);
    }
    let integer = n.trunc();
    if integer < 0.0 {
        Ok(((len as f64) + integer).max(0.0) as usize)
    } else {
        Ok((integer as usize).min(len))
    }
}

fn array_buffer_new_length(
    vm: &mut Vm,
    old_len: usize,
    value: Option<&Value>,
) -> error::Result<usize> {
    match value {
        Some(Value::Undefined) | None => Ok(old_len),
        Some(value) => to_index_length(vm, value, "ArrayBuffer"),
    }
}

fn array_buffer_copy_and_detach(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    immutable: bool,
    preserve_resizable: bool,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("ArrayBuffer transfer called without this"))?;
    if array_buffer_is_shared(vm, &this) != Some(false) {
        return Err(Error::type_err(
            "ArrayBuffer transfer called on non-ArrayBuffer",
        ));
    }
    let (old_len, _, _) = array_buffer_slots(vm, &this)
        .ok_or_else(|| Error::type_err("ArrayBuffer transfer called on non-ArrayBuffer"))?;
    let new_len = array_buffer_new_length(vm, old_len, args.first())?;
    let (_, detached, source_immutable) = array_buffer_slots(vm, &this)
        .ok_or_else(|| Error::type_err("ArrayBuffer transfer called on non-ArrayBuffer"))?;
    if detached {
        return Err(Error::type_err("ArrayBuffer transfer on detached buffer"));
    }
    if source_immutable {
        return Err(Error::type_err("ArrayBuffer transfer on immutable buffer"));
    }

    let mut bytes = vec![0; new_len];
    if let Value::Object(idx) = &this {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::ArrayBuffer(buffer) = o {
                let source = buffer.bytes.lock();
                let copy_len = source.len().min(new_len);
                bytes[..copy_len].copy_from_slice(&source[..copy_len]);
            }
        });
    }
    let source_max_byte_length = if preserve_resizable {
        let Value::Object(idx) = &this else {
            unreachable!();
        };
        vm.heap.with_obj(idx.0, |obj| {
            let HeapObj::ArrayBuffer(buffer) = obj else {
                return None;
            };
            buffer.max_byte_length
        })
    } else {
        None
    };
    if source_max_byte_length.is_some_and(|max| new_len > max) {
        return Err(Error::range("Invalid ArrayBuffer transfer length"));
    }
    let result =
        allocate_array_buffer_with_bytes_options(vm, bytes, immutable, source_max_byte_length)?;
    if let Value::Object(idx) = &this {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::ArrayBuffer(buffer) = o {
                buffer
                    .detached
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                buffer.bytes.lock().clear();
            }
        });
    }
    Ok(result)
}

fn is_detached_array_buffer(vm: &Vm, value: &Value) -> bool {
    match value {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::ArrayBuffer(buffer) = o {
                buffer.detached.load(std::sync::atomic::Ordering::Relaxed)
            } else {
                false
            }
        }),
        _ => false,
    }
}

fn is_immutable_array_buffer(vm: &Vm, value: &Value) -> bool {
    match value {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::ArrayBuffer(buffer) = o {
                buffer.immutable.load(std::sync::atomic::Ordering::Relaxed)
            } else {
                false
            }
        }),
        _ => false,
    }
}

fn require_mutable_data_view_buffer(vm: &Vm, buffer: &Value) -> error::Result<()> {
    if is_immutable_array_buffer(vm, buffer) {
        return Err(Error::type_err("DataView setter on immutable buffer"));
    }
    Ok(())
}

pub(crate) fn effective_view_byte_length(
    vm: &Vm,
    buffer: Option<&Value>,
    byte_offset: usize,
    fixed_byte_length: usize,
    length_tracking: bool,
    alignment: usize,
) -> Option<usize> {
    let Some(Value::Object(buffer_idx)) = buffer else {
        return Some(fixed_byte_length);
    };
    let current_length = vm.heap.with_obj(buffer_idx.0, |obj| {
        let HeapObj::ArrayBuffer(buffer) = obj else {
            return None;
        };
        if buffer.detached.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }
        Some(buffer.bytes.lock().len())
    })?;
    if length_tracking {
        let remaining = current_length.checked_sub(byte_offset)?;
        Some(remaining - (remaining % alignment))
    } else {
        byte_offset
            .checked_add(fixed_byte_length)
            .filter(|end| *end <= current_length)
            .map(|_| fixed_byte_length)
    }
}

fn data_view_slots(
    vm: &Vm,
    this: Option<Value>,
    name: &str,
) -> error::Result<(Value, usize, usize)> {
    let this = this.ok_or_else(|| Error::type_err(format!("DataView {name} getter needs this")))?;
    match this {
        Value::Object(idx) => vm
            .heap
            .with_obj(idx.0, |o| {
                if let HeapObj::DataView(view) = o {
                    Some((view.buffer.clone(), view.byte_offset, view.byte_length))
                } else {
                    None
                }
            })
            .ok_or_else(|| Error::type_err(format!("DataView {name} getter on non-DataView"))),
        _ => Err(Error::type_err(format!(
            "DataView {name} getter on non-object"
        ))),
    }
}

fn data_view_effective_byte_length(vm: &Vm, view: Option<&Value>) -> error::Result<usize> {
    let Some(Value::Object(idx)) = view else {
        return Err(Error::type_err("DataView operation called on non-object"));
    };
    let slots = vm.heap.with_obj(idx.0, |obj| {
        let HeapObj::DataView(view) = obj else {
            return None;
        };
        Some((
            view.buffer.clone(),
            view.byte_offset,
            view.byte_length,
            view.length_tracking,
        ))
    });
    let (buffer, byte_offset, byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("DataView operation called on non-DataView"))?;
    effective_view_byte_length(
        vm,
        Some(&buffer),
        byte_offset,
        byte_length,
        length_tracking,
        1,
    )
    .ok_or_else(|| Error::type_err("DataView is detached or out of bounds"))
}

fn data_view_to_index(vm: &mut Vm, value: &Value, name: &str) -> error::Result<usize> {
    let n = vm.to_number(value)?;
    if n.is_nan() {
        return Ok(0);
    }
    if !n.is_finite() {
        return Err(Error::range(format!("Invalid DataView {name} offset")));
    }
    let integer = n.trunc();
    if integer < 0.0 || integer > MAX_SAFE_INTEGER {
        return Err(Error::range(format!("Invalid DataView {name} offset")));
    }
    Ok(integer as usize)
}

fn array_buffer_byte_at(vm: &Vm, buffer: &Value, byte_index: usize) -> error::Result<u8> {
    match buffer {
        Value::Object(idx) => vm
            .heap
            .with_obj(idx.0, |o| {
                if let HeapObj::ArrayBuffer(array_buffer) = o {
                    if array_buffer
                        .detached
                        .load(std::sync::atomic::Ordering::Relaxed)
                    {
                        return Some(Err(Error::type_err("DataView getter on detached buffer")));
                    }
                    let bytes = array_buffer.bytes.lock();
                    return bytes.get(byte_index).copied().map(Ok);
                }
                None
            })
            .unwrap_or_else(|| Err(Error::type_err("DataView buffer is not an ArrayBuffer"))),
        _ => Err(Error::type_err("DataView buffer is not an object")),
    }
}

fn array_buffer_set_byte_at(
    vm: &Vm,
    buffer: &Value,
    byte_index: usize,
    byte: u8,
) -> error::Result<()> {
    match buffer {
        Value::Object(idx) => vm
            .heap
            .with_obj(idx.0, |o| {
                if let HeapObj::ArrayBuffer(array_buffer) = o {
                    if array_buffer
                        .detached
                        .load(std::sync::atomic::Ordering::Relaxed)
                    {
                        return Some(Err(Error::type_err("DataView setter on detached buffer")));
                    }
                    let mut bytes = array_buffer.bytes.lock();
                    if let Some(slot) = bytes.get_mut(byte_index) {
                        *slot = byte;
                        return Some(Ok(()));
                    }
                    return Some(Err(Error::range("Invalid DataView byte offset")));
                }
                None
            })
            .unwrap_or_else(|| Err(Error::type_err("DataView buffer is not an ArrayBuffer"))),
        _ => Err(Error::type_err("DataView buffer is not an object")),
    }
}

fn array_buffer_bytes_at(
    vm: &Vm,
    buffer: &Value,
    byte_index: usize,
    byte_count: usize,
) -> error::Result<Vec<u8>> {
    match buffer {
        Value::Object(idx) => vm
            .heap
            .with_obj(idx.0, |o| {
                if let HeapObj::ArrayBuffer(array_buffer) = o {
                    if array_buffer
                        .detached
                        .load(std::sync::atomic::Ordering::Relaxed)
                    {
                        return Some(Err(Error::type_err("DataView getter on detached buffer")));
                    }
                    let bytes = array_buffer.bytes.lock();
                    let end = match byte_index.checked_add(byte_count) {
                        Some(end) => end,
                        None => return Some(Err(Error::range("Invalid DataView byte offset"))),
                    };
                    if let Some(slice) = bytes.get(byte_index..end) {
                        return Some(Ok(slice.to_vec()));
                    }
                    return Some(Err(Error::range("Invalid DataView byte offset")));
                }
                None
            })
            .unwrap_or_else(|| Err(Error::type_err("DataView buffer is not an ArrayBuffer"))),
        _ => Err(Error::type_err("DataView buffer is not an object")),
    }
}

fn array_buffer_set_bytes_at(
    vm: &Vm,
    buffer: &Value,
    byte_index: usize,
    new_bytes: &[u8],
) -> error::Result<()> {
    match buffer {
        Value::Object(idx) => vm
            .heap
            .with_obj(idx.0, |o| {
                if let HeapObj::ArrayBuffer(array_buffer) = o {
                    if array_buffer
                        .detached
                        .load(std::sync::atomic::Ordering::Relaxed)
                    {
                        return Some(Err(Error::type_err("DataView setter on detached buffer")));
                    }
                    let mut bytes = array_buffer.bytes.lock();
                    let end = match byte_index.checked_add(new_bytes.len()) {
                        Some(end) => end,
                        None => return Some(Err(Error::range("Invalid DataView byte offset"))),
                    };
                    if let Some(slot) = bytes.get_mut(byte_index..end) {
                        slot.copy_from_slice(new_bytes);
                        return Some(Ok(()));
                    }
                    return Some(Err(Error::range("Invalid DataView byte offset")));
                }
                None
            })
            .unwrap_or_else(|| Err(Error::type_err("DataView buffer is not an ArrayBuffer"))),
        _ => Err(Error::type_err("DataView buffer is not an object")),
    }
}

fn data_view_read_u8(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    signed: bool,
    name: &str,
) -> error::Result<Value> {
    let view = this.clone();
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let view_length = data_view_effective_byte_length(vm, view.as_ref())?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err("DataView getter on detached buffer"));
    }
    if request_index
        .checked_add(1)
        .is_none_or(|end| end > view_length)
    {
        return Err(Error::range("Invalid DataView byte offset"));
    }
    let byte_index = view_offset + request_index;
    let byte = array_buffer_byte_at(vm, &buffer, byte_index)?;
    let value = if signed {
        i8::from_ne_bytes([byte]) as f64
    } else {
        byte as f64
    };
    Ok(Value::Number(value))
}

fn data_view_write_u8(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    name: &str,
) -> error::Result<Value> {
    let view = this.clone();
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    require_mutable_data_view_buffer(vm, &buffer)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let number_value = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    let view_length = data_view_effective_byte_length(vm, view.as_ref())?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err("DataView setter on detached buffer"));
    }
    if request_index
        .checked_add(1)
        .is_none_or(|end| end > view_length)
    {
        return Err(Error::range("Invalid DataView byte offset"));
    }
    let byte_index = view_offset + request_index;
    array_buffer_set_byte_at(vm, &buffer, byte_index, to_uint8_element(number_value))?;
    Ok(Value::Undefined)
}

fn data_view_read_u16(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    signed: bool,
    name: &str,
) -> error::Result<Value> {
    let view = this.clone();
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let little_endian = args.get(1).is_some_and(|value| vm.to_boolean(value));
    let view_length = data_view_effective_byte_length(vm, view.as_ref())?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err("DataView getter on detached buffer"));
    }
    if request_index
        .checked_add(2)
        .is_none_or(|end| end > view_length)
    {
        return Err(Error::range("Invalid DataView byte offset"));
    }
    let byte_index = view_offset + request_index;
    let bytes = array_buffer_bytes_at(vm, &buffer, byte_index, 2)?;
    let raw = [bytes[0], bytes[1]];
    let value = if signed {
        if little_endian {
            i16::from_le_bytes(raw) as f64
        } else {
            i16::from_be_bytes(raw) as f64
        }
    } else if little_endian {
        u16::from_le_bytes(raw) as f64
    } else {
        u16::from_be_bytes(raw) as f64
    };
    Ok(Value::Number(value))
}

fn data_view_write_u16(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    name: &str,
) -> error::Result<Value> {
    let view = this.clone();
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    require_mutable_data_view_buffer(vm, &buffer)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let number_value = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    let little_endian = args.get(2).is_some_and(|value| vm.to_boolean(value));
    let view_length = data_view_effective_byte_length(vm, view.as_ref())?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err("DataView setter on detached buffer"));
    }
    if request_index
        .checked_add(2)
        .is_none_or(|end| end > view_length)
    {
        return Err(Error::range("Invalid DataView byte offset"));
    }
    let byte_index = view_offset + request_index;
    let value = to_uint16_element(number_value);
    let bytes = if little_endian {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    array_buffer_set_bytes_at(vm, &buffer, byte_index, &bytes)?;
    Ok(Value::Undefined)
}

fn data_view_read_u32(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    signed: bool,
    name: &str,
) -> error::Result<Value> {
    let view = this.clone();
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let little_endian = args.get(1).is_some_and(|value| vm.to_boolean(value));
    let view_length = data_view_effective_byte_length(vm, view.as_ref())?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err("DataView getter on detached buffer"));
    }
    if request_index
        .checked_add(4)
        .is_none_or(|end| end > view_length)
    {
        return Err(Error::range("Invalid DataView byte offset"));
    }
    let byte_index = view_offset + request_index;
    let bytes = array_buffer_bytes_at(vm, &buffer, byte_index, 4)?;
    let raw = [bytes[0], bytes[1], bytes[2], bytes[3]];
    let value = if signed {
        if little_endian {
            i32::from_le_bytes(raw) as f64
        } else {
            i32::from_be_bytes(raw) as f64
        }
    } else if little_endian {
        u32::from_le_bytes(raw) as f64
    } else {
        u32::from_be_bytes(raw) as f64
    };
    Ok(Value::Number(value))
}

fn data_view_write_u32(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    name: &str,
) -> error::Result<Value> {
    let view = this.clone();
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    require_mutable_data_view_buffer(vm, &buffer)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let number_value = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    let little_endian = args.get(2).is_some_and(|value| vm.to_boolean(value));
    let view_length = data_view_effective_byte_length(vm, view.as_ref())?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err("DataView setter on detached buffer"));
    }
    if request_index
        .checked_add(4)
        .is_none_or(|end| end > view_length)
    {
        return Err(Error::range("Invalid DataView byte offset"));
    }
    let byte_index = view_offset + request_index;
    let value = to_uint32_element(number_value);
    let bytes = if little_endian {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    array_buffer_set_bytes_at(vm, &buffer, byte_index, &bytes)?;
    Ok(Value::Undefined)
}

fn data_view_read_f32(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    name: &str,
) -> error::Result<Value> {
    let view = this.clone();
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let little_endian = args.get(1).is_some_and(|value| vm.to_boolean(value));
    let view_length = data_view_effective_byte_length(vm, view.as_ref())?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err("DataView getter on detached buffer"));
    }
    if request_index
        .checked_add(4)
        .is_none_or(|end| end > view_length)
    {
        return Err(Error::range("Invalid DataView byte offset"));
    }
    let byte_index = view_offset + request_index;
    let bytes = array_buffer_bytes_at(vm, &buffer, byte_index, 4)?;
    let raw = [bytes[0], bytes[1], bytes[2], bytes[3]];
    let value = if little_endian {
        f32::from_le_bytes(raw)
    } else {
        f32::from_be_bytes(raw)
    };
    Ok(Value::Number(value as f64))
}

fn data_view_write_f32(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    name: &str,
) -> error::Result<Value> {
    let view = this.clone();
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    require_mutable_data_view_buffer(vm, &buffer)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let number_value = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    let little_endian = args.get(2).is_some_and(|value| vm.to_boolean(value));
    let view_length = data_view_effective_byte_length(vm, view.as_ref())?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err("DataView setter on detached buffer"));
    }
    if request_index
        .checked_add(4)
        .is_none_or(|end| end > view_length)
    {
        return Err(Error::range("Invalid DataView byte offset"));
    }
    let byte_index = view_offset + request_index;
    let value = number_value as f32;
    let bytes = if little_endian {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    array_buffer_set_bytes_at(vm, &buffer, byte_index, &bytes)?;
    Ok(Value::Undefined)
}

fn f16_bits_to_f64(bits: u16) -> f64 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = (bits >> 10) & 0x1f;
    let fraction = bits & 0x03ff;
    match exponent {
        0 => {
            if fraction == 0 {
                let sign_bit = ((bits as u64) & 0x8000) << 48;
                f64::from_bits(sign_bit)
            } else {
                sign * (fraction as f64) * 2f64.powi(-24)
            }
        }
        0x1f => {
            if fraction == 0 {
                sign * f64::INFINITY
            } else {
                f64::NAN
            }
        }
        _ => sign * (1.0 + (fraction as f64) / 1024.0) * 2f64.powi(exponent as i32 - 15),
    }
}

fn round_shift_right_ties_even(value: u64, shift: i32) -> u64 {
    if shift <= 0 {
        return value << (-shift as u32);
    }
    if shift >= 64 {
        return 0;
    }
    let shift = shift as u32;
    let quotient = value >> shift;
    let halfway = 1u64 << (shift - 1);
    let remainder = value & ((1u64 << shift) - 1);
    if remainder > halfway || (remainder == halfway && quotient & 1 == 1) {
        quotient + 1
    } else {
        quotient
    }
}

fn f64_to_f16_bits(value: f64) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 48) & 0x8000) as u16;
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & 0x000f_ffff_ffff_ffff;

    if exponent_bits == 0x7ff {
        return if fraction == 0 {
            sign | 0x7c00
        } else {
            sign | 0x7e00
        };
    }
    if exponent_bits == 0 {
        return sign;
    }

    let exponent = exponent_bits - 1023;
    let significand = (1u64 << 52) | fraction;

    if exponent < -14 {
        let rounded = round_shift_right_ties_even(significand, 28 - exponent);
        return sign | (rounded as u16);
    }

    if exponent > 15 {
        return sign | 0x7c00;
    }

    let mut half_exponent = exponent;
    let mut rounded = round_shift_right_ties_even(significand, 42);
    if rounded == 0x800 {
        half_exponent += 1;
        rounded = 0x400;
    }
    if half_exponent > 15 {
        return sign | 0x7c00;
    }

    sign | (((half_exponent + 15) as u16) << 10) | ((rounded as u16) & 0x03ff)
}

fn data_view_read_f16(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    name: &str,
) -> error::Result<Value> {
    let view = this.clone();
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let little_endian = args.get(1).is_some_and(|value| vm.to_boolean(value));
    let view_length = data_view_effective_byte_length(vm, view.as_ref())?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err("DataView getter on detached buffer"));
    }
    if request_index
        .checked_add(2)
        .is_none_or(|end| end > view_length)
    {
        return Err(Error::range("Invalid DataView byte offset"));
    }
    let byte_index = view_offset + request_index;
    let bytes = array_buffer_bytes_at(vm, &buffer, byte_index, 2)?;
    let raw = [bytes[0], bytes[1]];
    let bits = if little_endian {
        u16::from_le_bytes(raw)
    } else {
        u16::from_be_bytes(raw)
    };
    Ok(Value::Number(f16_bits_to_f64(bits)))
}

fn data_view_write_f16(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    name: &str,
) -> error::Result<Value> {
    let view = this.clone();
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    require_mutable_data_view_buffer(vm, &buffer)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let number_value = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    let little_endian = args.get(2).is_some_and(|value| vm.to_boolean(value));
    let view_length = data_view_effective_byte_length(vm, view.as_ref())?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err("DataView setter on detached buffer"));
    }
    if request_index
        .checked_add(2)
        .is_none_or(|end| end > view_length)
    {
        return Err(Error::range("Invalid DataView byte offset"));
    }
    let byte_index = view_offset + request_index;
    let value = f64_to_f16_bits(number_value);
    let bytes = if little_endian {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    array_buffer_set_bytes_at(vm, &buffer, byte_index, &bytes)?;
    Ok(Value::Undefined)
}

fn data_view_read_f64(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    name: &str,
) -> error::Result<Value> {
    let view = this.clone();
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let little_endian = args.get(1).is_some_and(|value| vm.to_boolean(value));
    let view_length = data_view_effective_byte_length(vm, view.as_ref())?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err("DataView getter on detached buffer"));
    }
    if request_index
        .checked_add(8)
        .is_none_or(|end| end > view_length)
    {
        return Err(Error::range("Invalid DataView byte offset"));
    }
    let byte_index = view_offset + request_index;
    let bytes = array_buffer_bytes_at(vm, &buffer, byte_index, 8)?;
    let raw = [
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ];
    let value = if little_endian {
        f64::from_le_bytes(raw)
    } else {
        f64::from_be_bytes(raw)
    };
    Ok(Value::Number(value))
}

fn data_view_write_f64(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    name: &str,
) -> error::Result<Value> {
    let view = this.clone();
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    require_mutable_data_view_buffer(vm, &buffer)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let number_value = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    let little_endian = args.get(2).is_some_and(|value| vm.to_boolean(value));
    let view_length = data_view_effective_byte_length(vm, view.as_ref())?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err("DataView setter on detached buffer"));
    }
    if request_index
        .checked_add(8)
        .is_none_or(|end| end > view_length)
    {
        return Err(Error::range("Invalid DataView byte offset"));
    }
    let byte_index = view_offset + request_index;
    let bytes = if little_endian {
        number_value.to_le_bytes()
    } else {
        number_value.to_be_bytes()
    };
    array_buffer_set_bytes_at(vm, &buffer, byte_index, &bytes)?;
    Ok(Value::Undefined)
}

fn data_view_read_bigint64(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    signed: bool,
    name: &str,
) -> error::Result<Value> {
    let view = this.clone();
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let little_endian = args.get(1).is_some_and(|value| vm.to_boolean(value));
    let view_length = data_view_effective_byte_length(vm, view.as_ref())?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err("DataView getter on detached buffer"));
    }
    if request_index
        .checked_add(8)
        .is_none_or(|end| end > view_length)
    {
        return Err(Error::range("Invalid DataView byte offset"));
    }
    let byte_index = view_offset + request_index;
    let bytes = array_buffer_bytes_at(vm, &buffer, byte_index, 8)?;
    let raw = [
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ];
    let value = if signed {
        if little_endian {
            BigInt::from(i64::from_le_bytes(raw))
        } else {
            BigInt::from(i64::from_be_bytes(raw))
        }
    } else if little_endian {
        BigInt::from(u64::from_le_bytes(raw))
    } else {
        BigInt::from(u64::from_be_bytes(raw))
    };
    Ok(Value::BigInt(value))
}

fn bigint_to_u64_element(value: &BigInt) -> u64 {
    let modulus = BigInt::from(1u128 << 64);
    let wrapped = ((value % &modulus) + &modulus) % &modulus;
    num_traits::ToPrimitive::to_u64(&wrapped).unwrap_or(0)
}

fn data_view_write_bigint64(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    name: &str,
) -> error::Result<Value> {
    let view = this.clone();
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    require_mutable_data_view_buffer(vm, &buffer)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let bigint_value = vm.to_bigint(args.get(1).unwrap_or(&Value::Undefined))?;
    let little_endian = args.get(2).is_some_and(|value| vm.to_boolean(value));
    let view_length = data_view_effective_byte_length(vm, view.as_ref())?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err("DataView setter on detached buffer"));
    }
    if request_index
        .checked_add(8)
        .is_none_or(|end| end > view_length)
    {
        return Err(Error::range("Invalid DataView byte offset"));
    }
    let byte_index = view_offset + request_index;
    let value = bigint_to_u64_element(&bigint_value);
    let bytes = if little_endian {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    };
    array_buffer_set_bytes_at(vm, &buffer, byte_index, &bytes)?;
    Ok(Value::Undefined)
}

pub(crate) fn data_view_buffer_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("DataView buffer getter needs this"))?;
    match this {
        Value::Object(idx) => vm
            .heap
            .with_obj(idx.0, |obj| {
                let HeapObj::DataView(view) = obj else {
                    return None;
                };
                Some(view.buffer.clone())
            })
            .ok_or_else(|| Error::type_err("DataView buffer getter on non-DataView")),
        _ => Err(Error::type_err("DataView buffer getter on non-object")),
    }
}

pub(crate) fn data_view_byte_length_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("DataView byteLength getter needs this"))?;
    let slots = match this {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |obj| {
            let HeapObj::DataView(view) = obj else {
                return None;
            };
            Some((
                view.buffer.clone(),
                view.byte_offset,
                view.byte_length,
                view.length_tracking,
            ))
        }),
        _ => None,
    }
    .ok_or_else(|| Error::type_err("DataView byteLength getter on non-DataView"))?;
    let byte_length = effective_view_byte_length(vm, Some(&slots.0), slots.1, slots.2, slots.3, 1)
        .ok_or_else(|| Error::type_err("DataView byteLength getter on out-of-bounds view"))?;
    Ok(Value::Number(byte_length as f64))
}

pub(crate) fn data_view_byte_offset_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("DataView byteOffset getter needs this"))?;
    let slots = match this {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |obj| {
            let HeapObj::DataView(view) = obj else {
                return None;
            };
            Some((
                view.buffer.clone(),
                view.byte_offset,
                view.byte_length,
                view.length_tracking,
            ))
        }),
        _ => None,
    }
    .ok_or_else(|| Error::type_err("DataView byteOffset getter on non-DataView"))?;
    effective_view_byte_length(vm, Some(&slots.0), slots.1, slots.2, slots.3, 1)
        .ok_or_else(|| Error::type_err("DataView byteOffset getter on out-of-bounds view"))?;
    Ok(Value::Number(slots.1 as f64))
}

pub(crate) fn data_view_get_uint8(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_read_u8(vm, args, this, false, "getUint8")
}

pub(crate) fn data_view_get_int8(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_read_u8(vm, args, this, true, "getInt8")
}

pub(crate) fn data_view_set_uint8(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_write_u8(vm, args, this, "setUint8")
}

pub(crate) fn data_view_set_int8(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_write_u8(vm, args, this, "setInt8")
}

pub(crate) fn data_view_get_uint16(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_read_u16(vm, args, this, false, "getUint16")
}

pub(crate) fn data_view_get_int16(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_read_u16(vm, args, this, true, "getInt16")
}

pub(crate) fn data_view_set_uint16(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_write_u16(vm, args, this, "setUint16")
}

pub(crate) fn data_view_set_int16(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_write_u16(vm, args, this, "setInt16")
}

pub(crate) fn data_view_get_uint32(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_read_u32(vm, args, this, false, "getUint32")
}

pub(crate) fn data_view_get_int32(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_read_u32(vm, args, this, true, "getInt32")
}

pub(crate) fn data_view_set_uint32(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_write_u32(vm, args, this, "setUint32")
}

pub(crate) fn data_view_set_int32(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_write_u32(vm, args, this, "setInt32")
}

pub(crate) fn data_view_get_float16(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_read_f16(vm, args, this, "getFloat16")
}

pub(crate) fn data_view_set_float16(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_write_f16(vm, args, this, "setFloat16")
}

pub(crate) fn data_view_get_float32(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_read_f32(vm, args, this, "getFloat32")
}

pub(crate) fn data_view_set_float32(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_write_f32(vm, args, this, "setFloat32")
}

pub(crate) fn data_view_get_float64(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_read_f64(vm, args, this, "getFloat64")
}

pub(crate) fn data_view_set_float64(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_write_f64(vm, args, this, "setFloat64")
}

pub(crate) fn data_view_get_bigint64(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_read_bigint64(vm, args, this, true, "getBigInt64")
}

pub(crate) fn data_view_get_biguint64(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_read_bigint64(vm, args, this, false, "getBigUint64")
}

pub(crate) fn data_view_set_bigint64(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_write_bigint64(vm, args, this, "setBigInt64")
}

pub(crate) fn data_view_set_biguint64(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    data_view_write_bigint64(vm, args, this, "setBigUint64")
}

fn typed_array_slots(
    vm: &Vm,
    this: Option<Value>,
    name: &str,
) -> error::Result<(crate::value::TypedArrayKind, Option<Value>, usize, usize)> {
    let this =
        this.ok_or_else(|| Error::type_err(format!("TypedArray {name} getter needs this")))?;
    match this {
        Value::Object(idx) => vm
            .heap
            .with_obj(idx.0, |o| {
                if let HeapObj::TypedArray(array) = o {
                    let byte_length = effective_view_byte_length(
                        vm,
                        array.viewed_array_buffer.as_ref(),
                        array.byte_offset,
                        array.byte_length,
                        array.length_tracking,
                        array.kind.element_size(),
                    )
                    .unwrap_or(0);
                    let byte_offset = if byte_length == 0
                        && effective_view_byte_length(
                            vm,
                            array.viewed_array_buffer.as_ref(),
                            array.byte_offset,
                            array.byte_length,
                            array.length_tracking,
                            array.kind.element_size(),
                        )
                        .is_none()
                    {
                        0
                    } else {
                        array.byte_offset
                    };
                    Some((
                        array.kind,
                        array.viewed_array_buffer.clone(),
                        byte_offset,
                        byte_length,
                    ))
                } else {
                    None
                }
            })
            .ok_or_else(|| Error::type_err(format!("TypedArray {name} getter on non-TypedArray"))),
        _ => Err(Error::type_err(format!(
            "TypedArray {name} getter on non-object"
        ))),
    }
}

pub(crate) fn typed_array_buffer_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, buffer, _, _) = typed_array_slots(vm, this, "buffer")?;
    Ok(buffer.unwrap_or(Value::Undefined))
}

pub(crate) fn typed_array_byte_length_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, _, _, byte_length) = typed_array_slots(vm, this, "byteLength")?;
    Ok(Value::Number(byte_length as f64))
}

pub(crate) fn typed_array_byte_offset_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, _, byte_offset, _) = typed_array_slots(vm, this, "byteOffset")?;
    Ok(Value::Number(byte_offset as f64))
}

pub(crate) fn typed_array_length_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (kind, _, _, byte_length) = typed_array_slots(vm, this, "length")?;
    Ok(Value::Number(
        typed_array_element_count(kind, byte_length) as f64
    ))
}

pub(crate) fn typed_array_to_string_tag_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let Some(Value::Object(index)) = this else {
        return Ok(Value::Undefined);
    };
    let name = vm.heap.with_obj(index.0, |object| {
        let HeapObj::TypedArray(array) = object else {
            return None;
        };
        Some(array.kind.name())
    });
    Ok(name
        .map(|name| Value::String(Arc::from(name)))
        .unwrap_or(Value::Undefined))
}

pub(crate) fn typed_array_at(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("TypedArray at called without this"))?;
    let Value::Object(idx) = &this else {
        return Err(Error::type_err("TypedArray at called on non-object"));
    };
    let slots = vm.heap.with_obj(idx.0, |obj| {
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
    });
    let (kind, buffer, byte_offset, byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray at called on non-TypedArray"))?;
    let byte_length = effective_view_byte_length(
        vm,
        buffer.as_ref(),
        byte_offset,
        byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray at called on out-of-bounds view"))?;
    let length = typed_array_element_count(kind, byte_length);

    let index = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
    if index.is_infinite() {
        return Ok(Value::Undefined);
    }
    let relative = if index.is_nan() { 0.0 } else { index.trunc() };
    let actual = if relative >= 0.0 {
        relative
    } else {
        length as f64 + relative
    };
    if actual < 0.0 || actual >= length as f64 {
        return Ok(Value::Undefined);
    }
    vm.get_property_by_key(&this, &PropertyKey::from((actual as usize).to_string()))
}

fn typed_array_content_type(kind: crate::value::TypedArrayKind) -> &'static str {
    match kind {
        crate::value::TypedArrayKind::BigInt64 | crate::value::TypedArrayKind::BigUint64 => {
            "BigInt"
        }
        _ => "Number",
    }
}

pub(crate) fn typed_array_subarray(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("TypedArray subarray called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err("TypedArray subarray called on non-object"));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, viewed_array_buffer, byte_offset, fixed_byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray subarray called on non-TypedArray"))?;
    let source_buffer = viewed_array_buffer
        .ok_or_else(|| Error::type_err("TypedArray subarray missing viewed ArrayBuffer"))?;
    let byte_length = effective_view_byte_length(
        vm,
        Some(&source_buffer),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .unwrap_or(0);
    let length = typed_array_element_count(kind, byte_length);
    let (start, end) = resolve_slice_bounds(vm, length, args.first(), args.get(1))?;
    let new_length = end - start;
    let new_byte_offset = byte_offset
        .checked_add(
            start
                .checked_mul(kind.element_size())
                .ok_or_else(|| Error::range("Invalid TypedArray subarray byte offset"))?,
        )
        .ok_or_else(|| Error::range("Invalid TypedArray subarray byte offset"))?;

    let default_ctor = current_realm_typed_array_constructor(vm, kind)?;
    let ctor = typed_array_species_constructor(vm, &this, default_ctor)?;
    let mut construct_args = vec![source_buffer.clone(), Value::Number(new_byte_offset as f64)];
    if !length_tracking || args.get(1).is_some_and(|end| !end.is_undefined()) {
        construct_args.push(Value::Number(new_length as f64));
    }
    let pin_count = vm.pin(&source_buffer) + vm.pin(&ctor) + vm.pin_many(&construct_args);
    let result = vm.construct(&ctor, &construct_args);
    vm.unpin_many(pin_count);
    let result = result?;

    let (result_kind, _, _, result_byte_length) =
        typed_array_slots(vm, Some(result.clone()), "subarray result")?;
    if typed_array_content_type(result_kind) != typed_array_content_type(kind) {
        return Err(Error::type_err(
            "TypedArray species returned incompatible content type",
        ));
    }
    if typed_array_element_count(result_kind, result_byte_length) < new_length {
        return Err(Error::type_err(
            "TypedArray species returned a shorter typed array",
        ));
    }
    Ok(result)
}

pub(crate) fn typed_array_set(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let target = this.ok_or_else(|| Error::type_err("TypedArray set called without this"))?;
    let Value::Object(target_idx) = &target else {
        return Err(Error::type_err("TypedArray set called on non-object"));
    };
    let target_slots = vm.heap.with_obj(target_idx.0, |obj| {
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
    });
    let (target_kind, target_buffer, target_byte_offset, target_fixed_byte_length, target_tracking) =
        target_slots.ok_or_else(|| Error::type_err("TypedArray set called on non-TypedArray"))?;
    let target_buffer = target_buffer
        .ok_or_else(|| Error::type_err("TypedArray set missing viewed ArrayBuffer"))?;
    if is_immutable_array_buffer(vm, &target_buffer) {
        return Err(Error::type_err("TypedArray set on immutable buffer"));
    }

    let offset_number = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    let target_offset = if offset_number.is_nan() || offset_number == 0.0 {
        0.0
    } else {
        offset_number.trunc()
    };
    if target_offset < 0.0 {
        return Err(Error::range("TypedArray set offset is negative"));
    }

    let source = args.first().cloned().unwrap_or(Value::Undefined);
    let source_slots = match &source {
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

    let target_byte_length = effective_view_byte_length(
        vm,
        Some(&target_buffer),
        target_byte_offset,
        target_fixed_byte_length,
        target_tracking,
        target_kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray set target is out of bounds"))?;
    let target_length = typed_array_element_count(target_kind, target_byte_length);

    if let Some((source_kind, source_buffer, source_byte_offset, source_fixed_length, tracking)) =
        source_slots
    {
        let source_buffer = source_buffer
            .ok_or_else(|| Error::type_err("TypedArray set source missing viewed ArrayBuffer"))?;
        let source_byte_length = effective_view_byte_length(
            vm,
            Some(&source_buffer),
            source_byte_offset,
            source_fixed_length,
            tracking,
            source_kind.element_size(),
        )
        .ok_or_else(|| Error::type_err("TypedArray set source is out of bounds"))?;
        let source_length = typed_array_element_count(source_kind, source_byte_length);
        if !target_offset.is_finite()
            || target_offset > usize::MAX as f64
            || (target_offset as usize)
                .checked_add(source_length)
                .is_none_or(|end| end > target_length)
        {
            return Err(Error::range("TypedArray set source does not fit target"));
        }
        if typed_array_content_type(target_kind) != typed_array_content_type(source_kind) {
            return Err(Error::type_err(
                "TypedArray set source has incompatible content type",
            ));
        }

        let source_bytes = match &source_buffer {
            Value::Object(buffer_idx) => vm.heap.with_obj(buffer_idx.0, |obj| {
                let HeapObj::ArrayBuffer(buffer) = obj else {
                    return Err(Error::type_err("TypedArray set source buffer is invalid"));
                };
                if buffer.detached.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err(Error::type_err("TypedArray set source buffer is detached"));
                }
                let end = source_byte_offset
                    .checked_add(source_byte_length)
                    .ok_or_else(|| Error::range("TypedArray set source range overflow"))?;
                let bytes = buffer.bytes.lock();
                if end > bytes.len() {
                    return Err(Error::type_err(
                        "TypedArray set source resized out of bounds",
                    ));
                }
                Ok(bytes[source_byte_offset..end].to_vec())
            })?,
            _ => return Err(Error::type_err("TypedArray set source buffer is invalid")),
        };
        let target_start = target_byte_offset
            .checked_add(
                (target_offset as usize)
                    .checked_mul(target_kind.element_size())
                    .ok_or_else(|| Error::range("TypedArray set target range overflow"))?,
            )
            .ok_or_else(|| Error::range("TypedArray set target range overflow"))?;

        if target_kind == source_kind {
            let Value::Object(buffer_idx) = &target_buffer else {
                return Err(Error::type_err("TypedArray set target buffer is invalid"));
            };
            vm.heap.with_obj(buffer_idx.0, |obj| {
                let HeapObj::ArrayBuffer(buffer) = obj else {
                    return Err(Error::type_err("TypedArray set target buffer is invalid"));
                };
                let end = target_start
                    .checked_add(source_bytes.len())
                    .ok_or_else(|| Error::range("TypedArray set target range overflow"))?;
                let mut bytes = buffer.bytes.lock();
                if end > bytes.len() {
                    return Err(Error::type_err(
                        "TypedArray set target resized out of bounds",
                    ));
                }
                bytes[target_start..end].copy_from_slice(&source_bytes);
                Ok(())
            })?;
        } else {
            for index in 0..source_length {
                let value = typed_array_read_element(source_kind, &source_bytes, index)
                    .ok_or_else(|| Error::type_err("TypedArray set source read failed"))?;
                vm.set_property_strict(
                    &target,
                    &(target_offset as usize + index).to_string(),
                    value,
                )?;
            }
        }
    } else {
        if source.is_nullish() {
            return Err(Error::type_err(
                "Cannot convert undefined or null to object",
            ));
        }
        let source = vm.to_object(&source)?;
        let source_length_value = vm.get_property(&source, "length")?;
        let source_length = to_array_like_length(vm, &source_length_value)?;
        if !target_offset.is_finite()
            || target_offset > usize::MAX as f64
            || (target_offset as usize)
                .checked_add(source_length)
                .is_none_or(|end| end > target_length)
        {
            return Err(Error::range("TypedArray set source does not fit target"));
        }
        let pin_count = vm.pin(&target) + vm.pin(&source);
        let result: error::Result<()> = (|| {
            for index in 0..source_length {
                let value = vm.get_property(&source, &index.to_string())?;
                vm.set_property_strict(
                    &target,
                    &(target_offset as usize + index).to_string(),
                    value,
                )?;
            }
            Ok(())
        })();
        vm.unpin_many(pin_count);
        result?;
    }
    Ok(Value::Undefined)
}

pub(crate) fn typed_array_copy_within(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("TypedArray copyWithin called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err(
            "TypedArray copyWithin called on non-object",
        ));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, backing, byte_offset, fixed_byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray copyWithin called on non-TypedArray"))?;
    let backing = backing
        .ok_or_else(|| Error::type_err("TypedArray copyWithin missing viewed ArrayBuffer"))?;
    if is_immutable_array_buffer(vm, &backing) {
        return Err(Error::type_err("TypedArray copyWithin on immutable buffer"));
    }
    let initial_byte_length = effective_view_byte_length(
        vm,
        Some(&backing),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray copyWithin called on out-of-bounds view"))?;
    let initial_length = typed_array_element_count(kind, initial_byte_length);

    let pin_count = vm.pin(&this) + vm.pin(&backing);
    let result: error::Result<Value> = (|| {
        let to = slice_bound(
            vm,
            args.first().unwrap_or(&Value::Undefined),
            initial_length,
        )?;
        let from = slice_bound(vm, args.get(1).unwrap_or(&Value::Undefined), initial_length)?;
        let final_index = match args.get(2) {
            Some(value) if !value.is_undefined() => slice_bound(vm, value, initial_length)?,
            _ => initial_length,
        };
        let requested_count = final_index
            .saturating_sub(from)
            .min(initial_length.saturating_sub(to));
        if requested_count == 0 {
            return Ok(this.clone());
        }

        let current_byte_length = effective_view_byte_length(
            vm,
            Some(&backing),
            byte_offset,
            fixed_byte_length,
            length_tracking,
            kind.element_size(),
        )
        .ok_or_else(|| Error::type_err("TypedArray copyWithin buffer became out of bounds"))?;
        let current_length = typed_array_element_count(kind, current_byte_length);
        let count = requested_count
            .min(current_length.saturating_sub(from))
            .min(current_length.saturating_sub(to));
        if count == 0 {
            return Ok(this.clone());
        }

        let element_size = kind.element_size();
        let source_start = byte_offset
            .checked_add(
                from.checked_mul(element_size)
                    .ok_or_else(|| Error::range("TypedArray copyWithin source overflow"))?,
            )
            .ok_or_else(|| Error::range("TypedArray copyWithin source overflow"))?;
        let target_start = byte_offset
            .checked_add(
                to.checked_mul(element_size)
                    .ok_or_else(|| Error::range("TypedArray copyWithin target overflow"))?,
            )
            .ok_or_else(|| Error::range("TypedArray copyWithin target overflow"))?;
        let byte_count = count
            .checked_mul(element_size)
            .ok_or_else(|| Error::range("TypedArray copyWithin count overflow"))?;
        let source_end = source_start
            .checked_add(byte_count)
            .ok_or_else(|| Error::range("TypedArray copyWithin source overflow"))?;
        let target_end = target_start
            .checked_add(byte_count)
            .ok_or_else(|| Error::range("TypedArray copyWithin target overflow"))?;

        let Value::Object(buffer_idx) = &backing else {
            return Err(Error::type_err("TypedArray copyWithin buffer is invalid"));
        };
        vm.heap.with_obj(buffer_idx.0, |obj| {
            let HeapObj::ArrayBuffer(buffer) = obj else {
                return Err(Error::type_err("TypedArray copyWithin buffer is invalid"));
            };
            if buffer.detached.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(Error::type_err("TypedArray copyWithin buffer is detached"));
            }
            let mut bytes = buffer.bytes.lock();
            if source_end > bytes.len() || target_end > bytes.len() {
                return Err(Error::type_err(
                    "TypedArray copyWithin range became out of bounds",
                ));
            }
            bytes.copy_within(source_start..source_end, target_start);
            Ok(())
        })?;
        Ok(this.clone())
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn typed_array_slice(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("TypedArray slice called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err("TypedArray slice called on non-object"));
    };
    let source_slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (source_kind, source_buffer, source_byte_offset, source_fixed_length, source_tracking) =
        source_slots.ok_or_else(|| Error::type_err("TypedArray slice called on non-TypedArray"))?;
    let source_buffer = source_buffer
        .ok_or_else(|| Error::type_err("TypedArray slice missing viewed ArrayBuffer"))?;
    let initial_byte_length = effective_view_byte_length(
        vm,
        Some(&source_buffer),
        source_byte_offset,
        source_fixed_length,
        source_tracking,
        source_kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray slice called on out-of-bounds view"))?;
    let initial_length = typed_array_element_count(source_kind, initial_byte_length);

    let source_pin_count = vm.pin(&this) + vm.pin(&source_buffer);
    let operation: error::Result<Value> = (|| {
        let (start, end) = resolve_slice_bounds(vm, initial_length, args.first(), args.get(1))?;
        let requested_count = end.saturating_sub(start);
        let default_constructor = current_realm_typed_array_constructor(vm, source_kind)?;
        let constructor = typed_array_species_constructor(vm, &this, default_constructor)?;
        let construct_args = [Value::Number(requested_count as f64)];
        let construct_pin_count = vm.pin(&constructor) + vm.pin_many(&construct_args);
        let result = vm.construct(&constructor, &construct_args);
        vm.unpin_many(construct_pin_count);
        let result = result?;

        let target_slots = match &result {
            Value::Object(result_idx) => vm.heap.with_obj(result_idx.0, |obj| {
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
        }
        .ok_or_else(|| Error::type_err("TypedArray slice species returned a non-TypedArray"))?;
        let (target_kind, target_buffer, target_byte_offset, target_fixed_length, target_tracking) =
            target_slots;
        let target_buffer = target_buffer
            .ok_or_else(|| Error::type_err("TypedArray slice result has no ArrayBuffer"))?;
        if is_immutable_array_buffer(vm, &target_buffer) {
            return Err(Error::type_err(
                "TypedArray slice species returned an immutable result",
            ));
        }
        let target_byte_length = effective_view_byte_length(
            vm,
            Some(&target_buffer),
            target_byte_offset,
            target_fixed_length,
            target_tracking,
            target_kind.element_size(),
        )
        .ok_or_else(|| Error::type_err("TypedArray slice result is out of bounds"))?;
        if typed_array_element_count(target_kind, target_byte_length) < requested_count {
            return Err(Error::type_err(
                "TypedArray slice species returned a shorter result",
            ));
        }
        if typed_array_content_type(target_kind) != typed_array_content_type(source_kind) {
            return Err(Error::type_err(
                "TypedArray slice species returned incompatible content type",
            ));
        }
        if requested_count == 0 {
            return Ok(result);
        }

        let current_source_byte_length = effective_view_byte_length(
            vm,
            Some(&source_buffer),
            source_byte_offset,
            source_fixed_length,
            source_tracking,
            source_kind.element_size(),
        )
        .ok_or_else(|| Error::type_err("TypedArray slice source became out of bounds"))?;
        let current_source_length =
            typed_array_element_count(source_kind, current_source_byte_length);
        let copy_count = end.min(current_source_length).saturating_sub(start);
        if copy_count == 0 {
            return Ok(result);
        }

        let result_pin_count = vm.pin(&result) + vm.pin(&target_buffer);
        let copy_result: error::Result<()> = (|| {
            if source_kind == target_kind {
                let element_size = source_kind.element_size();
                let source_start =
                    source_byte_offset
                        .checked_add(start.checked_mul(element_size).ok_or_else(|| {
                            Error::range("TypedArray slice source offset overflow")
                        })?)
                        .ok_or_else(|| Error::range("TypedArray slice source offset overflow"))?;
                let byte_count = copy_count
                    .checked_mul(element_size)
                    .ok_or_else(|| Error::range("TypedArray slice byte count overflow"))?;
                let source_end = source_start
                    .checked_add(byte_count)
                    .ok_or_else(|| Error::range("TypedArray slice source range overflow"))?;
                let target_end = target_byte_offset
                    .checked_add(byte_count)
                    .ok_or_else(|| Error::range("TypedArray slice target range overflow"))?;
                let (Value::Object(source_buffer_idx), Value::Object(target_buffer_idx)) =
                    (&source_buffer, &target_buffer)
                else {
                    return Err(Error::type_err("TypedArray slice buffer is invalid"));
                };
                if source_buffer_idx == target_buffer_idx {
                    vm.heap.with_obj(source_buffer_idx.0, |obj| {
                        let HeapObj::ArrayBuffer(buffer) = obj else {
                            return Err(Error::type_err("TypedArray slice buffer is invalid"));
                        };
                        let mut bytes = buffer.bytes.lock();
                        if source_end > bytes.len() || target_end > bytes.len() {
                            return Err(Error::type_err("TypedArray slice range is out of bounds"));
                        }
                        for offset in 0..byte_count {
                            let byte = bytes[source_start + offset];
                            bytes[target_byte_offset + offset] = byte;
                        }
                        Ok(())
                    })?;
                } else {
                    let source_bytes = vm.heap.with_obj(source_buffer_idx.0, |obj| {
                        let HeapObj::ArrayBuffer(buffer) = obj else {
                            return Err(Error::type_err("TypedArray slice source is invalid"));
                        };
                        let bytes = buffer.bytes.lock();
                        if source_end > bytes.len() {
                            return Err(Error::type_err(
                                "TypedArray slice source is out of bounds",
                            ));
                        }
                        Ok(bytes[source_start..source_end].to_vec())
                    })?;
                    vm.heap.with_obj(target_buffer_idx.0, |obj| {
                        let HeapObj::ArrayBuffer(buffer) = obj else {
                            return Err(Error::type_err("TypedArray slice target is invalid"));
                        };
                        let mut bytes = buffer.bytes.lock();
                        if target_end > bytes.len() {
                            return Err(Error::type_err(
                                "TypedArray slice target is out of bounds",
                            ));
                        }
                        bytes[target_byte_offset..target_end].copy_from_slice(&source_bytes);
                        Ok(())
                    })?;
                }
            } else {
                for index in 0..copy_count {
                    let value = vm.get_property(&this, &(start + index).to_string())?;
                    vm.set_property_strict(&result, &index.to_string(), value)?;
                }
            }
            Ok(())
        })();
        vm.unpin_many(result_pin_count);
        copy_result?;
        Ok(result)
    })();
    vm.unpin_many(source_pin_count);
    operation
}

pub(crate) fn typed_array_find(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    typed_array_predicate_impl(
        vm,
        args,
        this,
        "find",
        TypedArrayPredicateMode::FindValue,
        false,
    )
}

pub(crate) fn typed_array_find_index(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    typed_array_predicate_impl(
        vm,
        args,
        this,
        "findIndex",
        TypedArrayPredicateMode::FindIndex,
        false,
    )
}

pub(crate) fn typed_array_find_last(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    typed_array_predicate_impl(
        vm,
        args,
        this,
        "findLast",
        TypedArrayPredicateMode::FindValue,
        true,
    )
}

pub(crate) fn typed_array_find_last_index(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    typed_array_predicate_impl(
        vm,
        args,
        this,
        "findLastIndex",
        TypedArrayPredicateMode::FindIndex,
        true,
    )
}

pub(crate) fn typed_array_some(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    typed_array_predicate_impl(vm, args, this, "some", TypedArrayPredicateMode::Some, false)
}

pub(crate) fn typed_array_every(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    typed_array_predicate_impl(
        vm,
        args,
        this,
        "every",
        TypedArrayPredicateMode::Every,
        false,
    )
}

pub(crate) fn typed_array_for_each(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    typed_array_predicate_impl(
        vm,
        args,
        this,
        "forEach",
        TypedArrayPredicateMode::ForEach,
        false,
    )
}

#[derive(Clone, Copy)]
enum TypedArrayPredicateMode {
    FindValue,
    FindIndex,
    Some,
    Every,
    ForEach,
}

fn typed_array_predicate_impl(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    name: &str,
    mode: TypedArrayPredicateMode,
    from_last: bool,
) -> error::Result<Value> {
    let this =
        this.ok_or_else(|| Error::type_err(format!("TypedArray {name} called without this")))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err(format!(
            "TypedArray {name} called on non-object"
        )));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, backing, byte_offset, fixed_byte_length, length_tracking) = slots
        .ok_or_else(|| Error::type_err(format!("TypedArray {name} called on non-TypedArray")))?;
    let byte_length = effective_view_byte_length(
        vm,
        backing.as_ref(),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err(format!("TypedArray {name} called on out-of-bounds view")))?;
    let length = typed_array_element_count(kind, byte_length);
    let callback = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&callback, &vm.heap) {
        return Err(Error::type_err(format!(
            "TypedArray {name} predicate is not callable"
        )));
    }
    let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);

    let pin_count = vm.pin(&this) + vm.pin(&callback) + vm.pin(&this_arg);
    let result: error::Result<Value> = (|| {
        for offset in 0..length {
            let index = if from_last {
                length - offset - 1
            } else {
                offset
            };
            let value = vm.get_property(&this, &index.to_string())?;
            let predicate_result = vm.call_function(
                &callback,
                &[value.clone(), Value::Number(index as f64), this.clone()],
                Some(this_arg.clone()),
            )?;
            let predicate_truthy = predicate_result.is_truthy();
            let should_stop = match mode {
                TypedArrayPredicateMode::Every => !predicate_truthy,
                TypedArrayPredicateMode::ForEach => false,
                _ => predicate_truthy,
            };
            if should_stop {
                return Ok(match mode {
                    TypedArrayPredicateMode::FindValue => value,
                    TypedArrayPredicateMode::FindIndex => Value::Number(index as f64),
                    TypedArrayPredicateMode::Some => Value::Bool(true),
                    TypedArrayPredicateMode::Every => Value::Bool(false),
                    TypedArrayPredicateMode::ForEach => Value::Undefined,
                });
            }
        }
        Ok(match mode {
            TypedArrayPredicateMode::FindValue => Value::Undefined,
            TypedArrayPredicateMode::FindIndex => Value::Number(-1.0),
            TypedArrayPredicateMode::Some => Value::Bool(false),
            TypedArrayPredicateMode::Every => Value::Bool(true),
            TypedArrayPredicateMode::ForEach => Value::Undefined,
        })
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn typed_array_includes(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("TypedArray includes called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err("TypedArray includes called on non-object"));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, backing, byte_offset, fixed_byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray includes called on non-TypedArray"))?;
    let byte_length = effective_view_byte_length(
        vm,
        backing.as_ref(),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray includes called on out-of-bounds view"))?;
    let length = typed_array_element_count(kind, byte_length);
    let target = args.first().cloned().unwrap_or(Value::Undefined);

    let pin_count = vm.pin(&this) + vm.pin(&target);
    let result: error::Result<Value> = (|| {
        let Some(start) = array_search_start(vm, args, length, 0.0)? else {
            return Ok(Value::Bool(false));
        };
        for index in start..length {
            let value = vm.get_property(&this, &index.to_string())?;
            if value.same_value_zero(&target) {
                return Ok(Value::Bool(true));
            }
        }
        Ok(Value::Bool(false))
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn typed_array_index_of(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("TypedArray indexOf called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err("TypedArray indexOf called on non-object"));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, backing, byte_offset, fixed_byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray indexOf called on non-TypedArray"))?;
    let byte_length = effective_view_byte_length(
        vm,
        backing.as_ref(),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray indexOf called on out-of-bounds view"))?;
    let length = typed_array_element_count(kind, byte_length);
    let target = args.first().cloned().unwrap_or(Value::Undefined);

    let pin_count = vm.pin(&this) + vm.pin(&target);
    let result: error::Result<Value> = (|| {
        let Some(start) = array_search_start(vm, args, length, 0.0)? else {
            return Ok(Value::Number(-1.0));
        };
        for index in start..length {
            let key = index.to_string();
            if vm.has_property(&this, &key)? {
                let value = vm.get_property(&this, &key)?;
                if vm.strict_eq(&value, &target) {
                    return Ok(Value::Number(index as f64));
                }
            }
        }
        Ok(Value::Number(-1.0))
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn typed_array_last_index_of(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("TypedArray lastIndexOf called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err(
            "TypedArray lastIndexOf called on non-object",
        ));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, backing, byte_offset, fixed_byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray lastIndexOf called on non-TypedArray"))?;
    let byte_length = effective_view_byte_length(
        vm,
        backing.as_ref(),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray lastIndexOf called on out-of-bounds view"))?;
    let length = typed_array_element_count(kind, byte_length);
    if length == 0 {
        return Ok(Value::Number(-1.0));
    }
    let target = args.first().cloned().unwrap_or(Value::Undefined);

    let pin_count = vm.pin(&this) + vm.pin(&target);
    let result: error::Result<Value> = (|| {
        let raw = match args.get(1) {
            Some(value) => vm.to_number(value)?,
            None => f64::INFINITY,
        };
        if raw.is_infinite() && raw.is_sign_negative() {
            return Ok(Value::Number(-1.0));
        }
        let start = if raw.is_nan() {
            0
        } else if raw.is_infinite() {
            length - 1
        } else {
            let integer = raw.trunc();
            if integer >= 0.0 {
                (integer as usize).min(length - 1)
            } else {
                let relative = length as f64 + integer;
                if relative < 0.0 {
                    return Ok(Value::Number(-1.0));
                }
                relative as usize
            }
        };
        for index in (0..=start).rev() {
            let key = index.to_string();
            if vm.has_property(&this, &key)? {
                let value = vm.get_property(&this, &key)?;
                if vm.strict_eq(&value, &target) {
                    return Ok(Value::Number(index as f64));
                }
            }
        }
        Ok(Value::Number(-1.0))
    })();
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn typed_array_reduce_right(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    typed_array_reduce_impl(vm, args, this, true)
}

pub(crate) fn typed_array_reduce(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    typed_array_reduce_impl(vm, args, this, false)
}

pub(crate) fn typed_array_map(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("TypedArray map called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err("TypedArray map called on non-object"));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, backing, byte_offset, fixed_byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray map called on non-TypedArray"))?;
    let byte_length = effective_view_byte_length(
        vm,
        backing.as_ref(),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray map called on out-of-bounds view"))?;
    let length = typed_array_element_count(kind, byte_length);
    let callback = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&callback, &vm.heap) {
        return Err(Error::type_err("TypedArray map callback is not callable"));
    }
    let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);

    let source_pin_count = vm.pin(&this) + vm.pin(&callback) + vm.pin(&this_arg);
    let operation: error::Result<Value> = (|| {
        let result = typed_array_species_create(vm, &this, kind, length, "map", true)?;
        let result_pin_count = vm.pin(&result);
        let map_result: error::Result<()> = (|| {
            for index in 0..length {
                let value = vm.get_property(&this, &index.to_string())?;
                let mapped = vm.call_function(
                    &callback,
                    &[value, Value::Number(index as f64), this.clone()],
                    Some(this_arg.clone()),
                )?;
                let mapped_pin_count = vm.pin(&mapped);
                let write_result = vm.set_property_strict(&result, &index.to_string(), mapped);
                vm.unpin_many(mapped_pin_count);
                write_result?;
            }
            Ok(())
        })();
        vm.unpin_many(result_pin_count);
        map_result?;
        Ok(result)
    })();
    vm.unpin_many(source_pin_count);
    operation
}

pub(crate) fn typed_array_filter(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("TypedArray filter called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err("TypedArray filter called on non-object"));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, backing, byte_offset, fixed_byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray filter called on non-TypedArray"))?;
    let byte_length = effective_view_byte_length(
        vm,
        backing.as_ref(),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray filter called on out-of-bounds view"))?;
    let length = typed_array_element_count(kind, byte_length);
    let callback = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&callback, &vm.heap) {
        return Err(Error::type_err(
            "TypedArray filter callback is not callable",
        ));
    }
    let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);

    let source_pin_count = vm.pin(&this) + vm.pin(&callback) + vm.pin(&this_arg);
    let operation: error::Result<Value> = (|| {
        let mut kept = Vec::new();
        for index in 0..length {
            let value = vm.get_property(&this, &index.to_string())?;
            let selected = vm.call_function(
                &callback,
                &[value.clone(), Value::Number(index as f64), this.clone()],
                Some(this_arg.clone()),
            )?;
            if selected.is_truthy() {
                kept.push(value);
            }
        }

        let result = typed_array_species_create(vm, &this, kind, kept.len(), "filter", true)?;
        let result_pin_count = vm.pin(&result);
        let write_result: error::Result<()> = (|| {
            for (index, value) in kept.into_iter().enumerate() {
                let value_pin_count = vm.pin(&value);
                let set_result = vm.set_property_strict(&result, &index.to_string(), value);
                vm.unpin_many(value_pin_count);
                set_result?;
            }
            Ok(())
        })();
        vm.unpin_many(result_pin_count);
        write_result?;
        Ok(result)
    })();
    vm.unpin_many(source_pin_count);
    operation
}

fn typed_array_reduce_impl(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
    from_right: bool,
) -> error::Result<Value> {
    let name = if from_right { "reduceRight" } else { "reduce" };
    let this =
        this.ok_or_else(|| Error::type_err(format!("TypedArray {name} called without this")))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err(format!(
            "TypedArray {name} called on non-object"
        )));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, backing, byte_offset, fixed_byte_length, length_tracking) = slots
        .ok_or_else(|| Error::type_err(format!("TypedArray {name} called on non-TypedArray")))?;
    let byte_length = effective_view_byte_length(
        vm,
        backing.as_ref(),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err(format!("TypedArray {name} called on out-of-bounds view")))?;
    let length = typed_array_element_count(kind, byte_length);
    let callback = args.first().cloned().unwrap_or(Value::Undefined);
    if !is_callable(&callback, &vm.heap) {
        return Err(Error::type_err(format!(
            "TypedArray {name} callback is not callable"
        )));
    }
    if length == 0 && args.len() < 2 {
        return Err(Error::type_err(
            "Reduce of empty TypedArray with no initial value",
        ));
    }

    let base_pin_count = vm.pin(&this) + vm.pin(&callback);
    let mut accumulator_pin_count = 0;
    let result: error::Result<Value> = (|| {
        let (mut accumulator, start_offset) = if args.len() >= 2 {
            (args[1].clone(), 0)
        } else {
            let index = if from_right { length - 1 } else { 0 };
            (vm.get_property(&this, &index.to_string())?, 1)
        };
        accumulator_pin_count = vm.pin(&accumulator);

        for offset in start_offset..length {
            let index = if from_right {
                length - offset - 1
            } else {
                offset
            };
            let value = vm.get_property(&this, &index.to_string())?;
            let next = vm.call_function(
                &callback,
                &[
                    accumulator,
                    value,
                    Value::Number(index as f64),
                    this.clone(),
                ],
                Some(Value::Undefined),
            )?;
            vm.unpin_many(accumulator_pin_count);
            accumulator_pin_count = vm.pin(&next);
            accumulator = next;
        }
        Ok(accumulator)
    })();
    vm.unpin_many(accumulator_pin_count);
    vm.unpin_many(base_pin_count);
    result
}

fn typed_array_sort_compare(
    vm: &mut Vm,
    left: &Value,
    right: &Value,
    comparator: Option<&Value>,
) -> error::Result<std::cmp::Ordering> {
    if let Some(comparator) = comparator {
        let result = vm.call_function(
            comparator,
            &[left.clone(), right.clone()],
            Some(Value::Undefined),
        )?;
        let number = vm.to_number(&result)?;
        return Ok(if number.is_nan() || number == 0.0 {
            std::cmp::Ordering::Equal
        } else if number < 0.0 {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        });
    }

    match (left, right) {
        (Value::BigInt(left), Value::BigInt(right)) => Ok(left.cmp(right)),
        (Value::Number(left), Value::Number(right)) => {
            if left.is_nan() {
                return Ok(if right.is_nan() {
                    std::cmp::Ordering::Equal
                } else {
                    std::cmp::Ordering::Greater
                });
            }
            if right.is_nan() {
                return Ok(std::cmp::Ordering::Less);
            }
            if *left == 0.0 && *right == 0.0 {
                return Ok(right.is_sign_negative().cmp(&left.is_sign_negative()));
            }
            Ok(left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
        }
        _ => Err(Error::type_err(
            "TypedArray sort encountered incompatible element values",
        )),
    }
}

fn typed_array_stable_sort(
    vm: &mut Vm,
    items: &mut [Value],
    comparator: Option<&Value>,
) -> error::Result<()> {
    let length = items.len();
    if length < 2 {
        return Ok(());
    }
    let mut buffer = Vec::with_capacity(length);
    let mut width = 1;
    while width < length {
        let mut start = 0;
        while start < length {
            let middle = (start + width).min(length);
            let end = (start + 2 * width).min(length);
            let mut left = start;
            let mut right = middle;
            buffer.clear();
            while left < middle && right < end {
                if typed_array_sort_compare(vm, &items[left], &items[right], comparator)?
                    == std::cmp::Ordering::Greater
                {
                    buffer.push(items[right].clone());
                    right += 1;
                } else {
                    buffer.push(items[left].clone());
                    left += 1;
                }
            }
            buffer.extend_from_slice(&items[left..middle]);
            buffer.extend_from_slice(&items[right..end]);
            items[start..end].clone_from_slice(&buffer);
            start += 2 * width;
        }
        width *= 2;
    }
    Ok(())
}

pub(crate) fn typed_array_sort(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let comparator = args.first().cloned().unwrap_or(Value::Undefined);
    let comparator = if comparator.is_undefined() {
        None
    } else {
        if !is_callable(&comparator, &vm.heap) {
            return Err(Error::type_err(
                "TypedArray sort comparator is not callable",
            ));
        }
        Some(comparator)
    };

    let this = this.ok_or_else(|| Error::type_err("TypedArray sort called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err("TypedArray sort called on non-object"));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, backing, byte_offset, fixed_byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray sort called on non-TypedArray"))?;
    let backing =
        backing.ok_or_else(|| Error::type_err("TypedArray sort missing viewed ArrayBuffer"))?;
    if is_immutable_array_buffer(vm, &backing) {
        return Err(Error::type_err("TypedArray sort on immutable buffer"));
    }
    let byte_length = effective_view_byte_length(
        vm,
        Some(&backing),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray sort called on out-of-bounds view"))?;
    let length = typed_array_element_count(kind, byte_length);

    let comparator_pin_count = comparator.as_ref().map_or(0, |value| vm.pin(value));
    let pin_count = vm.pin(&this) + vm.pin(&backing) + comparator_pin_count;
    let result: error::Result<()> = (|| {
        let mut items = Vec::with_capacity(length);
        for index in 0..length {
            items.push(vm.get_property(&this, &index.to_string())?);
        }
        typed_array_stable_sort(vm, &mut items, comparator.as_ref())?;
        for (index, value) in items.into_iter().enumerate() {
            vm.set_property_strict(&this, &index.to_string(), value)?;
        }
        Ok(())
    })();
    vm.unpin_many(pin_count);
    result?;
    Ok(this)
}

pub(crate) fn typed_array_to_sorted(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let comparator = args.first().cloned().unwrap_or(Value::Undefined);
    let comparator = if comparator.is_undefined() {
        None
    } else {
        if !is_callable(&comparator, &vm.heap) {
            return Err(Error::type_err(
                "TypedArray toSorted comparator is not callable",
            ));
        }
        Some(comparator)
    };

    let this = this.ok_or_else(|| Error::type_err("TypedArray toSorted called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err("TypedArray toSorted called on non-object"));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, backing, byte_offset, fixed_byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray toSorted called on non-TypedArray"))?;
    let byte_length = effective_view_byte_length(
        vm,
        backing.as_ref(),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray toSorted called on out-of-bounds view"))?;
    let length = typed_array_element_count(kind, byte_length);

    let comparator_pin_count = comparator.as_ref().map_or(0, |value| vm.pin(value));
    let source_pin_count = vm.pin(&this) + comparator_pin_count;
    let sorted: error::Result<Vec<Value>> = (|| {
        let mut items = Vec::with_capacity(length);
        for index in 0..length {
            items.push(vm.get_property(&this, &index.to_string())?);
        }
        typed_array_stable_sort(vm, &mut items, comparator.as_ref())?;
        Ok(items)
    })();
    vm.unpin_many(source_pin_count);
    let sorted = sorted?;

    let constructor = current_realm_typed_array_constructor(vm, kind)?;
    let construct_args = [Value::Number(length as f64)];
    let construct_pin_count = vm.pin(&constructor) + vm.pin_many(&construct_args);
    let result = vm.construct(&constructor, &construct_args);
    vm.unpin_many(construct_pin_count);
    let result = result?;
    let (result_kind, _, _, result_byte_length) =
        typed_array_slots(vm, Some(result.clone()), "toSorted result")?;
    if result_kind != kind || typed_array_element_count(result_kind, result_byte_length) < length {
        return Err(Error::type_err(
            "TypedArray toSorted constructor returned an incompatible result",
        ));
    }

    let result_pin_count = vm.pin(&result);
    let write_result: error::Result<()> = (|| {
        for (index, value) in sorted.into_iter().enumerate() {
            vm.set_property_strict(&result, &index.to_string(), value)?;
        }
        Ok(())
    })();
    vm.unpin_many(result_pin_count);
    write_result?;
    Ok(result)
}

pub(crate) fn typed_array_with(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("TypedArray with called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err("TypedArray with called on non-object"));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, backing, byte_offset, fixed_byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray with called on non-TypedArray"))?;
    let byte_length = effective_view_byte_length(
        vm,
        backing.as_ref(),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray with called on out-of-bounds view"))?;
    let length = typed_array_element_count(kind, byte_length);
    let index = args.first().cloned().unwrap_or(Value::Undefined);
    let replacement = args.get(1).cloned().unwrap_or(Value::Undefined);

    let source_pin_count = vm.pin(&this) + vm.pin(&index) + vm.pin(&replacement);
    let operation: error::Result<Value> = (|| {
        let index_number = vm.to_number(&index)?;
        let relative_index = if index_number.is_nan() || index_number == 0.0 {
            0.0
        } else if index_number.is_infinite() {
            index_number
        } else {
            index_number.trunc()
        };
        let actual_index = if relative_index >= 0.0 {
            relative_index
        } else {
            length as f64 + relative_index
        };
        let numeric_value = match kind {
            crate::value::TypedArrayKind::BigInt64 | crate::value::TypedArrayKind::BigUint64 => {
                Value::BigInt(vm.to_bigint(&replacement)?)
            }
            _ => Value::Number(vm.to_number(&replacement)?),
        };
        let numeric_pin_count = vm.pin(&numeric_value);
        let create_result: error::Result<Value> = (|| {
            if !actual_index.is_finite()
                || actual_index < 0.0
                || actual_index > usize::MAX as f64
                || !vm.has_property(&this, &(actual_index as usize).to_string())?
            {
                return Err(Error::range("TypedArray with index is out of bounds"));
            }

            let constructor = current_realm_typed_array_constructor(vm, kind)?;
            let construct_args = [Value::Number(length as f64)];
            let constructor_pin_count = vm.pin(&constructor) + vm.pin_many(&construct_args);
            let result = vm.construct(&constructor, &construct_args);
            vm.unpin_many(constructor_pin_count);
            let result = result?;
            let (result_kind, _, _, result_byte_length) =
                typed_array_slots(vm, Some(result.clone()), "with result")?;
            if result_kind != kind
                || typed_array_element_count(result_kind, result_byte_length) < length
            {
                return Err(Error::type_err(
                    "TypedArray with constructor returned an incompatible result",
                ));
            }

            let result_pin_count = vm.pin(&result);
            let copy_result: error::Result<()> = (|| {
                for target_index in 0..length {
                    let value = if target_index as f64 == actual_index {
                        numeric_value.clone()
                    } else {
                        vm.get_property(&this, &target_index.to_string())?
                    };
                    vm.set_property_strict(&result, &target_index.to_string(), value)?;
                }
                Ok(())
            })();
            vm.unpin_many(result_pin_count);
            copy_result?;
            Ok(result)
        })();
        vm.unpin_many(numeric_pin_count);
        create_result
    })();
    vm.unpin_many(source_pin_count);
    operation
}

pub(crate) fn typed_array_join(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("TypedArray join called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err("TypedArray join called on non-object"));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, buffer, byte_offset, fixed_byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray join called on non-TypedArray"))?;
    let byte_length = effective_view_byte_length(
        vm,
        buffer.as_ref(),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray join called on out-of-bounds view"))?;
    let length = typed_array_element_count(kind, byte_length);
    let separator = match args.first() {
        Some(value) if !value.is_undefined() => vm.to_string(value)?.to_string(),
        _ => ",".to_string(),
    };
    let mut parts = Vec::with_capacity(length);
    for index in 0..length {
        let value = vm.get_property(&this, &index.to_string())?;
        parts.push(if value.is_nullish() {
            String::new()
        } else {
            vm.to_string(&value)?.to_string()
        });
    }
    Ok(Value::String(Arc::from(parts.join(&separator))))
}

pub(crate) fn typed_array_to_locale_string(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this =
        this.ok_or_else(|| Error::type_err("TypedArray toLocaleString called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err(
            "TypedArray toLocaleString called on non-object",
        ));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, buffer, byte_offset, fixed_byte_length, length_tracking) = slots
        .ok_or_else(|| Error::type_err("TypedArray toLocaleString called on non-TypedArray"))?;
    let byte_length = effective_view_byte_length(
        vm,
        buffer.as_ref(),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray toLocaleString called on out-of-bounds view"))?;
    let length = typed_array_element_count(kind, byte_length);
    let locale_args = [
        args.first().cloned().unwrap_or(Value::Undefined),
        args.get(1).cloned().unwrap_or(Value::Undefined),
    ];
    let realm_env = vm.native_callee_closure().unwrap_or(vm.global);

    let source_pin_count = vm.pin(&this) + vm.pin_many(&locale_args);
    let result: error::Result<Value> = (|| {
        let mut output = String::new();
        for index in 0..length {
            if index > 0 {
                output.push(',');
            }
            let value = vm.get_property(&this, &index.to_string())?;
            if value.is_nullish() {
                continue;
            }

            let value_pin_count = vm.pin(&value);
            let element_result: error::Result<String> = (|| {
                let intrinsic_name = if matches!(value, Value::BigInt(_)) {
                    "BigInt"
                } else {
                    "Number"
                };
                let constructor = crate::environment::get(&vm.heap, realm_env, intrinsic_name)
                    .ok_or_else(|| {
                        Error::type_err(format!("Missing {intrinsic_name} intrinsic"))
                    })?;
                let prototype = vm.get_property(&constructor, "prototype")?;
                let method = vm.get_property_rx(&prototype, "toLocaleString", value.clone(), 0)?;
                if !is_callable(&method, &vm.heap) {
                    return Err(Error::type_err("toLocaleString is not callable"));
                }
                let method_pin_count = vm.pin(&method);
                let localized = vm.call_function(&method, &locale_args, Some(value.clone()));
                vm.unpin_many(method_pin_count);
                let localized = localized?;

                let localized_pin_count = vm.pin(&localized);
                let string = vm.to_string(&localized).map(|value| value.to_string());
                vm.unpin_many(localized_pin_count);
                string
            })();
            vm.unpin_many(value_pin_count);
            output.push_str(&element_result?);
        }
        Ok(Value::String(Arc::from(output)))
    })();
    vm.unpin_many(source_pin_count);
    result
}

pub(crate) fn typed_array_reverse(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("TypedArray reverse called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err("TypedArray reverse called on non-object"));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, backing, byte_offset, fixed_byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray reverse called on non-TypedArray"))?;
    let backing =
        backing.ok_or_else(|| Error::type_err("TypedArray reverse missing viewed ArrayBuffer"))?;
    if is_immutable_array_buffer(vm, &backing) {
        return Err(Error::type_err("TypedArray reverse on immutable buffer"));
    }
    let byte_length = effective_view_byte_length(
        vm,
        Some(&backing),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray reverse called on out-of-bounds view"))?;
    let length = typed_array_element_count(kind, byte_length);

    let pin_count = vm.pin(&this) + vm.pin(&backing);
    let result: error::Result<()> = (|| {
        for lower in 0..length / 2 {
            let upper = length - lower - 1;
            let lower_value = vm.get_property(&this, &lower.to_string())?;
            let upper_value = vm.get_property(&this, &upper.to_string())?;
            vm.set_property_strict(&this, &lower.to_string(), upper_value)?;
            vm.set_property_strict(&this, &upper.to_string(), lower_value)?;
        }
        Ok(())
    })();
    vm.unpin_many(pin_count);
    result?;
    Ok(this)
}

pub(crate) fn typed_array_to_reversed(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("TypedArray toReversed called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err(
            "TypedArray toReversed called on non-object",
        ));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, backing, byte_offset, fixed_byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray toReversed called on non-TypedArray"))?;
    let byte_length = effective_view_byte_length(
        vm,
        backing.as_ref(),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray toReversed called on out-of-bounds view"))?;
    let length = typed_array_element_count(kind, byte_length);

    let constructor = current_realm_typed_array_constructor(vm, kind)?;
    let construct_args = [Value::Number(length as f64)];
    let pin_count = vm.pin(&this) + vm.pin(&constructor) + vm.pin_many(&construct_args);
    let result = vm.construct(&constructor, &construct_args);
    vm.unpin_many(pin_count);
    let result = result?;
    let (result_kind, _, _, result_byte_length) =
        typed_array_slots(vm, Some(result.clone()), "toReversed result")?;
    if result_kind != kind || typed_array_element_count(result_kind, result_byte_length) < length {
        return Err(Error::type_err(
            "TypedArray toReversed constructor returned an incompatible result",
        ));
    }

    let pin_count = vm.pin(&this) + vm.pin(&result);
    let copy_result: error::Result<()> = (|| {
        for target_index in 0..length {
            let source_index = length - target_index - 1;
            let value = vm.get_property(&this, &source_index.to_string())?;
            vm.set_property_strict(&result, &target_index.to_string(), value)?;
        }
        Ok(())
    })();
    vm.unpin_many(pin_count);
    copy_result?;
    Ok(result)
}

fn typed_array_iterator(
    vm: &mut Vm,
    this: Option<Value>,
    name: &str,
    iterator_kind: CollectionIteratorKind,
) -> error::Result<Value> {
    let this =
        this.ok_or_else(|| Error::type_err(format!("TypedArray {name} called without this")))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err(format!(
            "TypedArray {name} called on non-object"
        )));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (element_kind, buffer, byte_offset, fixed_byte_length, length_tracking) = slots
        .ok_or_else(|| Error::type_err(format!("TypedArray {name} called on non-TypedArray")))?;
    effective_view_byte_length(
        vm,
        buffer.as_ref(),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        element_kind.element_size(),
    )
    .ok_or_else(|| Error::type_err(format!("TypedArray {name} called on out-of-bounds view")))?;
    new_collection_iterator(vm, this, iterator_kind)
}

pub(crate) fn typed_array_values(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    typed_array_iterator(vm, this, "values", CollectionIteratorKind::ArrayValues)
}

pub(crate) fn typed_array_keys(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    typed_array_iterator(vm, this, "keys", CollectionIteratorKind::ArrayKeys)
}

pub(crate) fn typed_array_entries(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    typed_array_iterator(vm, this, "entries", CollectionIteratorKind::ArrayEntries)
}

pub(crate) fn typed_array_fill(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("TypedArray fill called without this"))?;
    let Value::Object(array_idx) = &this else {
        return Err(Error::type_err("TypedArray fill called on non-object"));
    };
    let slots = vm.heap.with_obj(array_idx.0, |obj| {
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
    });
    let (kind, backing, byte_offset, fixed_byte_length, length_tracking) =
        slots.ok_or_else(|| Error::type_err("TypedArray fill called on non-TypedArray"))?;
    let backing =
        backing.ok_or_else(|| Error::type_err("TypedArray fill missing viewed ArrayBuffer"))?;
    let byte_length = effective_view_byte_length(
        vm,
        Some(&backing),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray fill called on out-of-bounds view"))?;
    let (detached, immutable) = match &backing {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::ArrayBuffer(buffer) = obj {
                return Some((
                    buffer.detached.load(std::sync::atomic::Ordering::Relaxed),
                    buffer.immutable.load(std::sync::atomic::Ordering::Relaxed),
                ));
            }
            None
        }),
        _ => None,
    }
    .ok_or_else(|| Error::type_err("TypedArray fill missing viewed ArrayBuffer"))?;
    if detached {
        return Err(Error::type_err("TypedArray fill on detached buffer"));
    }
    if immutable {
        return Err(Error::type_err("TypedArray fill on immutable buffer"));
    }

    let element = typed_array_value_to_bytes(vm, kind, args.first().unwrap_or(&Value::Undefined))?;
    let length = typed_array_element_count(kind, byte_length);
    let (start, end) = resolve_slice_bounds(vm, length, args.get(1), args.get(2))?;
    effective_view_byte_length(
        vm,
        Some(&backing),
        byte_offset,
        fixed_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| Error::type_err("TypedArray fill resized out of bounds"))?;
    let size = kind.element_size();
    let start = byte_offset
        .checked_add(
            start
                .checked_mul(size)
                .ok_or_else(|| Error::range("Invalid TypedArray fill start"))?,
        )
        .ok_or_else(|| Error::range("Invalid TypedArray fill start"))?;
    let end = byte_offset
        .checked_add(
            end.checked_mul(size)
                .ok_or_else(|| Error::range("Invalid TypedArray fill end"))?,
        )
        .ok_or_else(|| Error::range("Invalid TypedArray fill end"))?;
    let Value::Object(buffer_idx) = backing else {
        return Err(Error::type_err(
            "TypedArray fill missing viewed ArrayBuffer",
        ));
    };
    vm.heap.with_obj(buffer_idx.0, |obj| {
        let HeapObj::ArrayBuffer(buffer) = obj else {
            return Err(Error::type_err(
                "TypedArray fill missing viewed ArrayBuffer",
            ));
        };
        let mut bytes = buffer.bytes.lock();
        if end > bytes.len() {
            return Err(Error::type_err("TypedArray fill range is out of bounds"));
        }
        for chunk in bytes[start..end].chunks_exact_mut(size) {
            chunk.copy_from_slice(&element);
        }
        Ok(())
    })?;
    Ok(this)
}

pub(crate) fn to_uint8_element(n: f64) -> u8 {
    if !n.is_finite() || n == 0.0 {
        return 0;
    }
    n.trunc().rem_euclid(256.0) as u8
}

fn to_uint8_clamp_element(n: f64) -> u8 {
    if n.is_nan() || n <= 0.0 {
        return 0;
    }
    if n >= 255.0 {
        return 255;
    }
    let floor = n.floor();
    let fractional = n - floor;
    if fractional < 0.5 {
        floor as u8
    } else if fractional > 0.5 {
        floor as u8 + 1
    } else {
        let floor_int = floor as u8;
        if floor_int.is_multiple_of(2) {
            floor_int
        } else {
            floor_int + 1
        }
    }
}

fn to_uint16_element(n: f64) -> u16 {
    if !n.is_finite() || n == 0.0 {
        return 0;
    }
    n.trunc().rem_euclid(65536.0) as u16
}

fn to_uint32_element(n: f64) -> u32 {
    if !n.is_finite() || n == 0.0 {
        return 0;
    }
    n.trunc().rem_euclid(4294967296.0) as u32
}

pub(crate) fn typed_array_element_count(
    kind: crate::value::TypedArrayKind,
    byte_len: usize,
) -> usize {
    byte_len / kind.element_size()
}

pub(crate) fn typed_array_read_element(
    kind: crate::value::TypedArrayKind,
    bytes: &[u8],
    index: usize,
) -> Option<Value> {
    let size = kind.element_size();
    let offset = index.checked_mul(size)?;
    let end = offset.checked_add(size)?;
    if end > bytes.len() {
        return None;
    }
    let value = match kind {
        crate::value::TypedArrayKind::Uint8 | crate::value::TypedArrayKind::Uint8Clamped => {
            Value::Number(bytes[offset] as f64)
        }
        crate::value::TypedArrayKind::Int8 => Value::Number((bytes[offset] as i8) as f64),
        crate::value::TypedArrayKind::Uint16 => {
            Value::Number(u16::from_ne_bytes([bytes[offset], bytes[offset + 1]]) as f64)
        }
        crate::value::TypedArrayKind::Int16 => {
            Value::Number(i16::from_ne_bytes([bytes[offset], bytes[offset + 1]]) as f64)
        }
        crate::value::TypedArrayKind::Uint32 => Value::Number(u32::from_ne_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as f64),
        crate::value::TypedArrayKind::Int32 => Value::Number(i32::from_ne_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as f64),
        crate::value::TypedArrayKind::Float32 => Value::Number(f32::from_ne_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as f64),
        crate::value::TypedArrayKind::Float64 => Value::Number(f64::from_ne_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ])),
        crate::value::TypedArrayKind::BigInt64 => {
            Value::BigInt(BigInt::from(i64::from_ne_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ])))
        }
        crate::value::TypedArrayKind::BigUint64 => {
            Value::BigInt(BigInt::from(u64::from_ne_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ])))
        }
    };
    Some(value)
}

pub(crate) fn typed_array_value_to_bytes(
    vm: &mut Vm,
    kind: crate::value::TypedArrayKind,
    value: &Value,
) -> error::Result<Vec<u8>> {
    let bytes = match kind {
        crate::value::TypedArrayKind::Uint8 => vec![to_uint8_element(vm.to_number(value)?)],
        crate::value::TypedArrayKind::Uint8Clamped => {
            vec![to_uint8_clamp_element(vm.to_number(value)?)]
        }
        crate::value::TypedArrayKind::Int8 => vec![to_uint8_element(vm.to_number(value)?)],
        crate::value::TypedArrayKind::Uint16 => to_uint16_element(vm.to_number(value)?)
            .to_ne_bytes()
            .to_vec(),
        crate::value::TypedArrayKind::Int16 => to_uint16_element(vm.to_number(value)?)
            .to_ne_bytes()
            .to_vec(),
        crate::value::TypedArrayKind::Uint32 => to_uint32_element(vm.to_number(value)?)
            .to_ne_bytes()
            .to_vec(),
        crate::value::TypedArrayKind::Int32 => to_uint32_element(vm.to_number(value)?)
            .to_ne_bytes()
            .to_vec(),
        crate::value::TypedArrayKind::Float32 => {
            (vm.to_number(value)? as f32).to_ne_bytes().to_vec()
        }
        crate::value::TypedArrayKind::Float64 => vm.to_number(value)?.to_ne_bytes().to_vec(),
        crate::value::TypedArrayKind::BigInt64 | crate::value::TypedArrayKind::BigUint64 => {
            bigint_to_u64_element(&vm.to_bigint(value)?)
                .to_ne_bytes()
                .to_vec()
        }
    };
    Ok(bytes)
}

fn typed_array_iterable_to_bytes(
    vm: &mut Vm,
    kind: crate::value::TypedArrayKind,
    value: &Value,
) -> error::Result<Vec<u8>> {
    const MAX_TYPED_ARRAY_ITERABLE_LEN: usize = 1 << 16;
    let iterator = vm.make_iterator(value)?;
    let pin = vm.pin(&iterator);
    let mut values = Vec::new();
    loop {
        if values.len() >= MAX_TYPED_ARRAY_ITERABLE_LEN {
            vm.unpin(pin);
            return Err(Error::range(format!("Invalid {} length", kind.name())));
        }
        let (item, done) = vm.iterator_next(&iterator)?;
        if done {
            break;
        }
        values.push(item);
    }
    vm.unpin(pin);
    let mut bytes = Vec::with_capacity(values.len() * kind.element_size());
    for value in &values {
        bytes.extend_from_slice(&typed_array_value_to_bytes(vm, kind, value)?);
    }
    Ok(bytes)
}

fn array_buffer_prototype(vm: &mut Vm) -> Value {
    let closure = vm.native_callee_closure().unwrap_or(vm.global);
    let realm = crate::environment::global_env_root(&vm.heap, closure);
    vm.realm_array_buffer_prototypes
        .get(&realm.0)
        .cloned()
        .or_else(|| vm.realm_array_buffer_prototypes.get(&vm.global.0).cloned())
        .or_else(|| {
            matches!(vm.array_buffer_proto, Value::Object(_)).then(|| vm.array_buffer_proto.clone())
        })
        .unwrap_or_else(|| vm.object_proto.clone())
}

fn allocate_array_buffer_with_bytes(vm: &mut Vm, bytes: Vec<u8>) -> error::Result<Value> {
    allocate_array_buffer_with_bytes_and_immutable(vm, bytes, false)
}

fn allocate_array_buffer_with_bytes_and_immutable(
    vm: &mut Vm,
    bytes: Vec<u8>,
    immutable: bool,
) -> error::Result<Value> {
    allocate_array_buffer_with_bytes_options(vm, bytes, immutable, None)
}

fn allocate_array_buffer_with_bytes_options(
    vm: &mut Vm,
    bytes: Vec<u8>,
    immutable: bool,
    max_byte_length: Option<usize>,
) -> error::Result<Value> {
    let proto = array_buffer_prototype(vm);
    let idx = vm
        .heap
        .allocate(HeapObj::ArrayBuffer(crate::value::ArrayBufferData {
            bytes: Arc::new(Mutex::new(bytes)),
            waiters: Arc::new(Mutex::new(std::collections::HashMap::new())),
            detached: AtomicBool::new(false),
            immutable: AtomicBool::new(immutable),
            shared: false,
            max_byte_length,
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
        }))?;
    Ok(Value::Object(GcIdx(idx)))
}

fn allocate_typed_array_view(
    vm: &mut Vm,
    kind: crate::value::TypedArrayKind,
    proto: Value,
    viewed_array_buffer: Value,
    byte_offset: usize,
    byte_length: usize,
    length_tracking: bool,
) -> error::Result<Value> {
    let idx = vm
        .heap
        .allocate(HeapObj::TypedArray(crate::value::TypedArrayData {
            buffer: Mutex::new(Vec::new()),
            viewed_array_buffer: Some(viewed_array_buffer),
            byte_offset,
            byte_length,
            length_tracking,
            kind,
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
            extensible: AtomicBool::new(true),
        }))?;
    Ok(Value::Object(GcIdx(idx)))
}

fn allocate_typed_array_from_bytes(
    vm: &mut Vm,
    kind: crate::value::TypedArrayKind,
    proto: Value,
    bytes: Vec<u8>,
) -> error::Result<Value> {
    let byte_length = bytes.len();
    let viewed_array_buffer = allocate_array_buffer_with_bytes(vm, bytes)?;
    allocate_typed_array_view(vm, kind, proto, viewed_array_buffer, 0, byte_length, false)
}

fn typed_array_result_length(
    vm: &mut Vm,
    result: &Value,
    required_len: usize,
) -> error::Result<()> {
    let Value::Object(result_idx) = result else {
        return Err(Error::type_err(
            "TypedArray constructor returned a non-TypedArray",
        ));
    };
    let (kind, viewed_array_buffer, byte_offset, raw_byte_length, length_tracking) = vm
        .heap
        .with_obj(result_idx.0, |object| {
            let HeapObj::TypedArray(array) = object else {
                return None;
            };
            Some((
                array.kind,
                array.viewed_array_buffer.clone(),
                array.byte_offset,
                array.byte_length,
                array.length_tracking,
            ))
        })
        .ok_or_else(|| Error::type_err("TypedArray constructor returned a non-TypedArray"))?;
    let byte_length = effective_view_byte_length(
        vm,
        viewed_array_buffer.as_ref(),
        byte_offset,
        raw_byte_length,
        length_tracking,
        kind.element_size(),
    )
    .ok_or_else(|| {
        Error::type_err("TypedArray constructor returned a detached or out-of-bounds result")
    })?;
    let actual_len = typed_array_element_count(kind, byte_length);
    if actual_len < required_len {
        return Err(Error::type_err(
            "TypedArray constructor returned a shorter typed array",
        ));
    }
    if let Some(Value::Object(buffer_idx)) = viewed_array_buffer {
        let immutable = vm.heap.with_obj(buffer_idx.0, |o| {
            if let HeapObj::ArrayBuffer(buffer) = o {
                return buffer.immutable.load(std::sync::atomic::Ordering::Relaxed);
            }
            false
        });
        if immutable {
            return Err(Error::type_err(
                "TypedArray constructor returned an immutable ArrayBuffer-backed result",
            ));
        }
    }
    Ok(())
}

fn typed_array_mapped_value(
    vm: &mut Vm,
    value: Value,
    index: usize,
    map_fn: Option<&Value>,
    this_arg: &Value,
) -> error::Result<Value> {
    match map_fn {
        Some(map_fn) => vm.call_function(
            map_fn,
            &[value, Value::Number(index as f64)],
            Some(this_arg.clone()),
        ),
        None => Ok(value),
    }
}

fn typed_array_write_indexed_values(
    vm: &mut Vm,
    result: &Value,
    values: &[Value],
    map_fn: Option<&Value>,
    this_arg: &Value,
) -> error::Result<()> {
    typed_array_result_length(vm, result, values.len())?;
    let result_pin = vm.pin(result);
    let write_result: error::Result<()> = (|| {
        for (index, value) in values.iter().cloned().enumerate() {
            let mapped = typed_array_mapped_value(vm, value, index, map_fn, this_arg)?;
            vm.set_property_strict(result, &index.to_string(), mapped)?;
        }
        Ok(())
    })();
    vm.unpin(result_pin);
    write_result
}

fn typed_array_construct_and_fill(
    vm: &mut Vm,
    constructor: &Value,
    values: &[Value],
    map_fn: Option<&Value>,
    this_arg: &Value,
) -> error::Result<Value> {
    let values_pin_count = vm.pin_many(values);
    let result = match vm.construct(constructor, &[Value::Number(values.len() as f64)]) {
        Ok(result) => result,
        Err(err) => {
            vm.unpin_many(values_pin_count);
            return Err(err);
        }
    };
    let write_result = typed_array_write_indexed_values(vm, &result, values, map_fn, this_arg);
    vm.unpin_many(values_pin_count);
    write_result?;
    Ok(result)
}

fn typed_array_collect_iterator_values(
    vm: &mut Vm,
    source: &Value,
    iterator_method: &Value,
) -> error::Result<Vec<Value>> {
    let iterator = vm.call_function(iterator_method, &[], Some(source.clone()))?;
    if !matches!(iterator, Value::Object(_)) {
        return Err(Error::type_err("TypedArray.from iterator is not an object"));
    }
    let iterator_pin = vm.pin(&iterator);
    let next = match vm.get_property(&iterator, "next") {
        Ok(next) => next,
        Err(error) => {
            vm.unpin(iterator_pin);
            return Err(error);
        }
    };
    if !is_callable(&next, &vm.heap) {
        vm.unpin(iterator_pin);
        return Err(Error::type_err(
            "TypedArray.from iterator next is not callable",
        ));
    }
    let next_pin = vm.pin(&next);
    let fixed_pin_count = iterator_pin + next_pin;
    let mut value_pin_count = 0;
    let mut values = Vec::new();
    loop {
        let next_result = match vm.call_function(&next, &[], Some(iterator.clone())) {
            Ok(result) => result,
            Err(err) => {
                vm.unpin_many(fixed_pin_count + value_pin_count);
                return Err(err);
            }
        };
        if !matches!(next_result, Value::Object(_)) {
            vm.unpin_many(fixed_pin_count + value_pin_count);
            return Err(Error::type_err(
                "TypedArray.from iterator result is not an object",
            ));
        }
        let next_result_pin = vm.pin(&next_result);
        let done = match vm.get_property(&next_result, "done") {
            Ok(value) => vm.to_boolean(&value),
            Err(err) => {
                vm.unpin(next_result_pin);
                vm.unpin_many(fixed_pin_count + value_pin_count);
                return Err(err);
            }
        };
        if done {
            vm.unpin(next_result_pin);
            break;
        }
        let value = match vm.get_property(&next_result, "value") {
            Ok(value) => value,
            Err(err) => {
                vm.unpin(next_result_pin);
                vm.unpin_many(fixed_pin_count + value_pin_count);
                return Err(err);
            }
        };
        vm.unpin(next_result_pin);
        value_pin_count += vm.pin(&value);
        values.push(value);
    }
    vm.unpin_many(fixed_pin_count + value_pin_count);
    Ok(values)
}

pub(crate) fn typed_array_from(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let constructor = this.unwrap_or(Value::Undefined);
    if !is_callable(&constructor, &vm.heap) || !vm.is_constructor_value(&constructor) {
        return Err(Error::type_err(
            "TypedArray.from receiver is not a constructor",
        ));
    }

    let source = args.first().cloned().unwrap_or(Value::Undefined);
    let map_fn_value = args.get(1).cloned().unwrap_or(Value::Undefined);
    let map_fn = if matches!(map_fn_value, Value::Undefined) {
        None
    } else {
        if !is_callable(&map_fn_value, &vm.heap) {
            return Err(Error::type_err("TypedArray.from mapfn is not callable"));
        }
        Some(map_fn_value)
    };
    let this_arg = args.get(2).cloned().unwrap_or(Value::Undefined);

    let iterator_key = PropertyKey::Symbol(vm.well_known_symbols.iterator);
    let iterator_method = vm.get_property_by_key(&source, &iterator_key)?;
    if !iterator_method.is_undefined() && !iterator_method.is_null() {
        if !is_callable(&iterator_method, &vm.heap) {
            return Err(Error::type_err(
                "TypedArray.from iterator method is not callable",
            ));
        }
        let values = typed_array_collect_iterator_values(vm, &source, &iterator_method)?;
        return typed_array_construct_and_fill(
            vm,
            &constructor,
            &values,
            map_fn.as_ref(),
            &this_arg,
        );
    }

    let length_value = vm.get_property(&source, "length")?;
    let length = to_array_like_length(vm, &length_value)?;
    let result = vm.construct(&constructor, &[Value::Number(length as f64)])?;
    typed_array_result_length(vm, &result, length)?;
    let result_pin = vm.pin(&result);
    let source_pin = vm.pin(&source);
    let write_result: error::Result<()> = (|| {
        for index in 0..length {
            let value = vm.get_property(&source, &index.to_string())?;
            let mapped = typed_array_mapped_value(vm, value, index, map_fn.as_ref(), &this_arg)?;
            vm.set_property_strict(&result, &index.to_string(), mapped)?;
        }
        Ok(())
    })();
    vm.unpin(source_pin);
    vm.unpin(result_pin);
    write_result?;
    Ok(result)
}

pub(crate) fn typed_array_of(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let constructor = this.unwrap_or(Value::Undefined);
    if !is_callable(&constructor, &vm.heap) || !vm.is_constructor_value(&constructor) {
        return Err(Error::type_err(
            "TypedArray.of receiver is not a constructor",
        ));
    }
    typed_array_construct_and_fill(vm, &constructor, args, None, &Value::Undefined)
}

fn typed_array_constructor_with_kind(
    vm: &mut Vm,
    args: &[Value],
    kind: crate::value::TypedArrayKind,
) -> error::Result<Value> {
    if vm.current_native_new_target.is_none() {
        return Err(Error::type_err(
            "TypedArray constructor requires new".to_string(),
        ));
    }
    let buffer = match args.first() {
        None => Vec::new(),
        Some(Value::Undefined) => Vec::new(),
        Some(Value::Number(n)) => {
            let length = to_index_length(vm, &Value::Number(*n), kind.name())?;
            let byte_len = length
                .checked_mul(kind.element_size())
                .filter(|len| *len <= MAX_ARRAY_BUFFER_LENGTH)
                .ok_or_else(|| Error::range(format!("Invalid {} length", kind.name())))?;
            vec![0u8; byte_len]
        }
        Some(Value::Object(idx)) => {
            let array_like = Value::Object(*idx);
            let is_array_buffer = vm
                .heap
                .with_obj(idx.0, |o| matches!(o, HeapObj::ArrayBuffer(_)));
            if is_array_buffer {
                let byte_offset = match args.get(1) {
                    Some(Value::Undefined) | None => 0,
                    Some(value) => to_index_length(vm, value, "TypedArray byteOffset")?,
                };
                if byte_offset % kind.element_size() != 0 {
                    return Err(Error::range("Invalid TypedArray byteOffset"));
                }
                let element_length = match args.get(2) {
                    Some(Value::Undefined) | None => None,
                    Some(value) => Some(to_index_length(vm, value, "TypedArray length")?),
                };
                let buffer_len = vm.heap.with_obj(idx.0, |o| {
                    if let HeapObj::ArrayBuffer(buffer) = o {
                        if buffer.detached.load(std::sync::atomic::Ordering::Relaxed) {
                            return Err(Error::type_err("TypedArray buffer is detached"));
                        }
                        return Ok(buffer.bytes.lock().len());
                    }
                    Err(Error::type_err("TypedArray buffer is not an ArrayBuffer"))
                })?;
                let length_tracking = element_length.is_none()
                    && vm.heap.with_obj(idx.0, |o| {
                        matches!(o, HeapObj::ArrayBuffer(buffer) if buffer.max_byte_length.is_some())
                    });
                let byte_length = match element_length {
                    None => {
                        if byte_offset > buffer_len {
                            return Err(Error::range("Invalid TypedArray length"));
                        }
                        let remaining = buffer_len - byte_offset;
                        if !length_tracking && remaining % kind.element_size() != 0 {
                            return Err(Error::range("Invalid TypedArray length"));
                        }
                        remaining - (remaining % kind.element_size())
                    }
                    Some(element_length) => element_length
                        .checked_mul(kind.element_size())
                        .ok_or_else(|| Error::range("Invalid TypedArray length"))?,
                };
                if byte_offset
                    .checked_add(byte_length)
                    .is_none_or(|end| end > buffer_len)
                {
                    return Err(Error::range("Invalid TypedArray length"));
                }
                let proto = typed_array_constructor_prototype(vm, kind)?;
                return allocate_typed_array_view(
                    vm,
                    kind,
                    proto,
                    array_like,
                    byte_offset,
                    byte_length,
                    length_tracking,
                );
            }
            let source_view = vm.heap.with_obj(idx.0, |obj| {
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
            });
            if let Some((source_kind, source_buffer, source_offset, source_length, tracking)) =
                source_view
            {
                if effective_view_byte_length(
                    vm,
                    source_buffer.as_ref(),
                    source_offset,
                    source_length,
                    tracking,
                    source_kind.element_size(),
                )
                .is_none()
                {
                    return Err(Error::type_err(
                        "TypedArray source is detached or out of bounds",
                    ));
                }
            }
            let iterator_key = PropertyKey::Symbol(vm.well_known_symbols.iterator);
            let is_builtin_iterable = vm.heap.with_obj(idx.0, |o| {
                matches!(
                    o,
                    HeapObj::Generator(_)
                        | HeapObj::LazyGenerator(_)
                        | HeapObj::Map(_)
                        | HeapObj::Set(_)
                )
            });
            if is_builtin_iterable || vm.has_property_key(&array_like, &iterator_key)? {
                let iterator_method = vm.get_property_by_key(&array_like, &iterator_key)?;
                if is_builtin_iterable
                    || (!iterator_method.is_undefined() && !iterator_method.is_null())
                {
                    let buffer = typed_array_iterable_to_bytes(vm, kind, &array_like)?;
                    let proto = typed_array_constructor_prototype(vm, kind)?;
                    return allocate_typed_array_from_bytes(vm, kind, proto, buffer);
                }
            }
            let length_value = vm.get_property(&array_like, "length")?;
            let length = to_array_like_length(vm, &length_value)?;
            let byte_len = length
                .checked_mul(kind.element_size())
                .filter(|len| *len <= MAX_ARRAY_BUFFER_LENGTH)
                .ok_or_else(|| Error::range(format!("Invalid {} length", kind.name())))?;
            let mut bytes = Vec::with_capacity(byte_len);
            for index in 0..length {
                let item = vm.get_property(&array_like, &index.to_string())?;
                bytes.extend_from_slice(&typed_array_value_to_bytes(vm, kind, &item)?);
            }
            bytes
        }
        Some(value) => {
            let length = to_index_length(vm, value, kind.name())?;
            let byte_len = length
                .checked_mul(kind.element_size())
                .filter(|len| *len <= MAX_ARRAY_BUFFER_LENGTH)
                .ok_or_else(|| Error::range(format!("Invalid {} length", kind.name())))?;
            vec![0u8; byte_len]
        }
    };

    let proto = typed_array_constructor_prototype(vm, kind)?;
    allocate_typed_array_from_bytes(vm, kind, proto, buffer)
}

fn typed_array_constructor_prototype(
    vm: &mut Vm,
    kind: crate::value::TypedArrayKind,
) -> error::Result<Value> {
    let fallback_proto = vm
        .current_native_callee
        .clone()
        .and_then(|callee| {
            vm.get_property_by_key(&callee, &PropertyKey::from("prototype"))
                .ok()
        })
        .filter(|proto| matches!(proto, Value::Object(_)))
        .unwrap_or_else(|| vm.object_proto.clone());
    native_constructor_prototype_with_default(vm, kind.name(), fallback_proto)
}

pub(crate) fn uint8array_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    typed_array_constructor_with_kind(vm, args, crate::value::TypedArrayKind::Uint8)
}

pub(crate) fn uint8clampedarray_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    typed_array_constructor_with_kind(vm, args, crate::value::TypedArrayKind::Uint8Clamped)
}

pub(crate) fn int8array_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    typed_array_constructor_with_kind(vm, args, crate::value::TypedArrayKind::Int8)
}

pub(crate) fn uint16array_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    typed_array_constructor_with_kind(vm, args, crate::value::TypedArrayKind::Uint16)
}

pub(crate) fn int16array_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    typed_array_constructor_with_kind(vm, args, crate::value::TypedArrayKind::Int16)
}

pub(crate) fn uint32array_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    typed_array_constructor_with_kind(vm, args, crate::value::TypedArrayKind::Uint32)
}

pub(crate) fn int32array_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    typed_array_constructor_with_kind(vm, args, crate::value::TypedArrayKind::Int32)
}

pub(crate) fn float32array_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    typed_array_constructor_with_kind(vm, args, crate::value::TypedArrayKind::Float32)
}

pub(crate) fn float64array_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    typed_array_constructor_with_kind(vm, args, crate::value::TypedArrayKind::Float64)
}

pub(crate) fn bigint64array_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    typed_array_constructor_with_kind(vm, args, crate::value::TypedArrayKind::BigInt64)
}

pub(crate) fn biguint64array_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    typed_array_constructor_with_kind(vm, args, crate::value::TypedArrayKind::BigUint64)
}
