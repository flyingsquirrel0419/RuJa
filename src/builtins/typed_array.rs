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
        Some(value) => to_index_length(vm, value, "ArrayBuffer")?,
        None => 0,
    };
    let fallback_proto = if matches!(vm.array_buffer_proto, Value::Object(_)) {
        vm.array_buffer_proto.clone()
    } else {
        vm.object_proto.clone()
    };
    let proto = native_constructor_prototype_with_default(vm, "ArrayBuffer", fallback_proto)?;
    let idx = vm
        .heap
        .allocate(HeapObj::ArrayBuffer(crate::value::ArrayBufferData {
            bytes: Mutex::new(vec![0; length]),
            detached: AtomicBool::new(false),
            immutable: AtomicBool::new(false),
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

fn current_realm_typed_array_constructor(
    vm: &mut Vm,
    kind: crate::value::TypedArrayKind,
) -> error::Result<Value> {
    let realm_env = vm.native_callee_closure().unwrap_or(vm.global);
    crate::environment::get(&vm.heap, realm_env, kind.name())
        .or_else(|| crate::environment::get(&vm.heap, vm.global, kind.name()))
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

pub(crate) fn array_buffer_slice(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("ArrayBuffer slice called without this"))?;
    let (bytes, detached) = match &this {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::ArrayBuffer(buffer) = o {
                Some((
                    buffer.bytes.lock().clone(),
                    buffer.detached.load(std::sync::atomic::Ordering::Relaxed),
                ))
            } else {
                None
            }
        }),
        _ => None,
    }
    .ok_or_else(|| Error::type_err("ArrayBuffer.prototype.slice called on non-ArrayBuffer"))?;
    if detached {
        return Err(Error::type_err(
            "ArrayBuffer.prototype.slice on detached buffer",
        ));
    }

    let len = bytes.len();
    let (from, to) = resolve_slice_bounds(vm, len, args.first(), args.get(1))?;
    let count = to - from;

    let default_ctor = current_realm_array_buffer_constructor(vm)?;
    let ctor = array_buffer_species_constructor(vm, &this, default_ctor)?;
    let result = vm.construct(&ctor, &[Value::Number(count as f64)])?;
    if result == this {
        return Err(Error::type_err(
            "ArrayBuffer species returned the source buffer",
        ));
    }

    let (result_len, result_detached) = array_buffer_len_and_detached(vm, &result)
        .ok_or_else(|| Error::type_err("ArrayBuffer species did not return an ArrayBuffer"))?;
    if result_detached {
        return Err(Error::type_err(
            "ArrayBuffer species returned a detached buffer",
        ));
    }
    let (_, _, result_immutable) = array_buffer_slots(vm, &result)
        .ok_or_else(|| Error::type_err("ArrayBuffer species did not return an ArrayBuffer"))?;
    if result_immutable {
        return Err(Error::type_err(
            "ArrayBuffer species returned an immutable buffer",
        ));
    }
    if result_len < count {
        return Err(Error::type_err(
            "ArrayBuffer species returned a buffer that is too small",
        ));
    }

    let Value::Object(idx) = &result else {
        return Err(Error::type_err(
            "ArrayBuffer species did not return an object",
        ));
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
                    Some(Value::Bool(
                        buffer.immutable.load(std::sync::atomic::Ordering::Relaxed),
                    ))
                } else {
                    None
                }
            })
            .ok_or_else(|| Error::type_err("ArrayBuffer immutable getter on non-ArrayBuffer")),
        _ => Err(Error::type_err(
            "ArrayBuffer immutable getter on non-object",
        )),
    }
}

pub(crate) fn array_buffer_transfer(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    array_buffer_copy_and_detach(vm, args, this, false)
}

pub(crate) fn array_buffer_transfer_to_fixed_length(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    array_buffer_copy_and_detach(vm, args, this, false)
}

pub(crate) fn array_buffer_transfer_to_immutable(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    array_buffer_copy_and_detach(vm, args, this, true)
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
                    if buffer.detached.load(std::sync::atomic::Ordering::Relaxed) {
                        Some(Value::Number(0.0))
                    } else {
                        Some(Value::Number(buffer.bytes.lock().len() as f64))
                    }
                } else {
                    None
                }
            })
            .ok_or_else(|| Error::type_err("ArrayBuffer byteLength getter on non-ArrayBuffer")),
        _ => Err(Error::type_err(
            "ArrayBuffer byteLength getter on non-object",
        )),
    }
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
    if byte_offset
        .checked_add(byte_length)
        .is_none_or(|end| end > buffer_len)
    {
        return Err(Error::range("Invalid DataView byteLength"));
    }

    let proto = native_constructor_prototype_with_default(vm, "DataView", vm.object_proto.clone())?;
    let (_, detached) = array_buffer_len_and_detached(vm, &buffer)
        .ok_or_else(|| Error::type_err("DataView buffer must be an ArrayBuffer"))?;
    if detached {
        return Err(Error::type_err("DataView buffer is detached"));
    }
    let idx = vm
        .heap
        .allocate(HeapObj::DataView(crate::value::DataViewData {
            buffer,
            byte_offset,
            byte_length,
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
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("ArrayBuffer transfer called without this"))?;
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
    let result = allocate_array_buffer_with_bytes_and_immutable(vm, bytes, immutable)?;
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
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
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
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    require_mutable_data_view_buffer(vm, &buffer)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let number_value = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
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
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let little_endian = args.get(1).is_some_and(|value| vm.to_boolean(value));
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
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    require_mutable_data_view_buffer(vm, &buffer)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let number_value = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    let little_endian = args.get(2).is_some_and(|value| vm.to_boolean(value));
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
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let little_endian = args.get(1).is_some_and(|value| vm.to_boolean(value));
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
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    require_mutable_data_view_buffer(vm, &buffer)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let number_value = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    let little_endian = args.get(2).is_some_and(|value| vm.to_boolean(value));
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
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let little_endian = args.get(1).is_some_and(|value| vm.to_boolean(value));
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
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    require_mutable_data_view_buffer(vm, &buffer)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let number_value = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    let little_endian = args.get(2).is_some_and(|value| vm.to_boolean(value));
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
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let little_endian = args.get(1).is_some_and(|value| vm.to_boolean(value));
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
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    require_mutable_data_view_buffer(vm, &buffer)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let number_value = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    let little_endian = args.get(2).is_some_and(|value| vm.to_boolean(value));
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
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let little_endian = args.get(1).is_some_and(|value| vm.to_boolean(value));
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
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    require_mutable_data_view_buffer(vm, &buffer)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let number_value = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
    let little_endian = args.get(2).is_some_and(|value| vm.to_boolean(value));
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
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let little_endian = args.get(1).is_some_and(|value| vm.to_boolean(value));
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
    let (buffer, view_offset, view_length) = data_view_slots(vm, this, name)?;
    require_mutable_data_view_buffer(vm, &buffer)?;
    let request_index = data_view_to_index(vm, args.first().unwrap_or(&Value::Undefined), name)?;
    let bigint_value = vm.to_bigint(args.get(1).unwrap_or(&Value::Undefined))?;
    let little_endian = args.get(2).is_some_and(|value| vm.to_boolean(value));
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
    let (buffer, _, _) = data_view_slots(vm, this, "buffer")?;
    Ok(buffer)
}

pub(crate) fn data_view_byte_length_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (buffer, _, byte_length) = data_view_slots(vm, this, "byteLength")?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err(
            "DataView byteLength getter on detached buffer",
        ));
    }
    Ok(Value::Number(byte_length as f64))
}

pub(crate) fn data_view_byte_offset_get(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (buffer, byte_offset, _) = data_view_slots(vm, this, "byteOffset")?;
    if is_detached_array_buffer(vm, &buffer) {
        return Err(Error::type_err(
            "DataView byteOffset getter on detached buffer",
        ));
    }
    Ok(Value::Number(byte_offset as f64))
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
                    Some((
                        array.kind,
                        array.viewed_array_buffer.clone(),
                        array.byte_offset,
                        array.byte_length,
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
    let (kind, viewed_array_buffer, byte_offset, byte_length) =
        typed_array_slots(vm, Some(this.clone()), "subarray")?;
    let source_buffer = viewed_array_buffer
        .ok_or_else(|| Error::type_err("TypedArray subarray missing viewed ArrayBuffer"))?;
    if is_detached_array_buffer(vm, &source_buffer) {
        return Err(Error::type_err("TypedArray subarray on detached buffer"));
    }

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
    let construct_args = [
        source_buffer.clone(),
        Value::Number(new_byte_offset as f64),
        Value::Number(new_length as f64),
    ];
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
    if matches!(vm.array_buffer_proto, Value::Object(_)) {
        vm.array_buffer_proto.clone()
    } else {
        vm.object_proto.clone()
    }
}

fn allocate_array_buffer_with_bytes(vm: &mut Vm, bytes: Vec<u8>) -> error::Result<Value> {
    allocate_array_buffer_with_bytes_and_immutable(vm, bytes, false)
}

fn allocate_array_buffer_with_bytes_and_immutable(
    vm: &mut Vm,
    bytes: Vec<u8>,
    immutable: bool,
) -> error::Result<Value> {
    let proto = array_buffer_prototype(vm);
    let idx = vm
        .heap
        .allocate(HeapObj::ArrayBuffer(crate::value::ArrayBufferData {
            bytes: Mutex::new(bytes),
            detached: AtomicBool::new(false),
            immutable: AtomicBool::new(immutable),
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
) -> error::Result<Value> {
    let idx = vm
        .heap
        .allocate(HeapObj::TypedArray(crate::value::TypedArrayData {
            buffer: Mutex::new(Vec::new()),
            viewed_array_buffer: Some(viewed_array_buffer),
            byte_offset,
            byte_length,
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
    allocate_typed_array_view(vm, kind, proto, viewed_array_buffer, 0, byte_length)
}

fn typed_array_result_length(
    vm: &mut Vm,
    result: &Value,
    required_len: usize,
) -> error::Result<()> {
    let (kind, viewed_array_buffer, _, byte_length) =
        typed_array_slots(vm, Some(result.clone()), "static result")?;
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
    const MAX_TYPED_ARRAY_FROM_LEN: usize = 1 << 16;
    let iterator = vm.call_function(iterator_method, &[], Some(source.clone()))?;
    if !matches!(iterator, Value::Object(_)) {
        return Err(Error::type_err("TypedArray.from iterator is not an object"));
    }
    let next = vm.get_property(&iterator, "next")?;
    if !is_callable(&next, &vm.heap) {
        return Err(Error::type_err(
            "TypedArray.from iterator next is not callable",
        ));
    }
    let pin_count = vm.pin(&iterator) + vm.pin(&next);
    let mut values = Vec::new();
    loop {
        if values.len() >= MAX_TYPED_ARRAY_FROM_LEN {
            vm.unpin_many(pin_count);
            return Err(Error::range("Invalid TypedArray.from length"));
        }
        let next_result = match vm.call_function(&next, &[], Some(iterator.clone())) {
            Ok(result) => result,
            Err(err) => {
                vm.unpin_many(pin_count);
                return Err(err);
            }
        };
        if !matches!(next_result, Value::Object(_)) {
            vm.unpin_many(pin_count);
            return Err(Error::type_err(
                "TypedArray.from iterator result is not an object",
            ));
        }
        let done = match vm.get_property(&next_result, "done") {
            Ok(value) => vm.to_boolean(&value),
            Err(err) => {
                vm.unpin_many(pin_count);
                return Err(err);
            }
        };
        if done {
            break;
        }
        let value = match vm.get_property(&next_result, "value") {
            Ok(value) => value,
            Err(err) => {
                vm.unpin_many(pin_count);
                return Err(err);
            }
        };
        values.push(value);
    }
    vm.unpin_many(pin_count);
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
                let byte_length = match element_length {
                    None => {
                        if byte_offset > buffer_len {
                            return Err(Error::range("Invalid TypedArray length"));
                        }
                        let remaining = buffer_len - byte_offset;
                        if remaining % kind.element_size() != 0 {
                            return Err(Error::range("Invalid TypedArray length"));
                        }
                        remaining
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
                );
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
