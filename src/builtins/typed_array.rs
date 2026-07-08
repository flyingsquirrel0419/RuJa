use super::*;

const MAX_ARRAY_BUFFER_LENGTH: usize = 1 << 26;
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

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
    let length = match args.first() {
        Some(value) => to_index_length(vm, value, "ArrayBuffer")?,
        None => 0,
    };
    let proto = native_constructor_prototype(vm, vm.object_proto.clone())?;
    let idx = vm
        .heap
        .allocate(HeapObj::ArrayBuffer(crate::value::ArrayBufferData {
            bytes: Mutex::new(vec![0; length]),
            detached: AtomicBool::new(false),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
        }))?;
    Ok(Value::Object(GcIdx(idx)))
}

pub(crate) fn array_buffer_slice(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let this = this.ok_or_else(|| Error::type_err("ArrayBuffer slice called without this"))?;
    let bytes = match &this {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::ArrayBuffer(buffer) = o {
                if buffer.detached.load(std::sync::atomic::Ordering::Relaxed) {
                    Some(Vec::new())
                } else {
                    Some(buffer.bytes.lock().clone())
                }
            } else {
                None
            }
        }),
        _ => None,
    }
    .ok_or_else(|| Error::type_err("ArrayBuffer.prototype.slice called on non-ArrayBuffer"))?;

    let len = bytes.len();
    let start = match args.first() {
        Some(value) => vm.to_number(value)?.trunc() as isize,
        None => 0,
    };
    let end = match args.get(1) {
        Some(Value::Undefined) | None => len as isize,
        Some(value) => vm.to_number(value)?.trunc() as isize,
    };
    let from = if start < 0 {
        (len as isize + start).max(0) as usize
    } else {
        (start as usize).min(len)
    };
    let to = if end < 0 {
        (len as isize + end).max(0) as usize
    } else {
        (end as usize).min(len)
    };
    let to = to.max(from);
    let count = to - from;

    let ctor = vm.get_property(&this, "constructor")?;
    let ctor = if ctor.is_undefined() {
        crate::environment::get(&vm.heap, vm.global, "ArrayBuffer").unwrap_or(Value::Undefined)
    } else {
        ctor
    };
    let result = vm.construct(&ctor, &[Value::Number(count as f64)])?;
    if let Value::Object(idx) = &result {
        vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::ArrayBuffer(buffer) = o {
                *buffer.bytes.lock() = bytes[from..to].to_vec();
            }
        });
    }
    Ok(result)
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
    let buffer = args.first().cloned().unwrap_or(Value::Undefined);
    let buffer_len = match &buffer {
        Value::Object(idx) => vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::ArrayBuffer(array_buffer) = o {
                if array_buffer
                    .detached
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    None
                } else {
                    Some(array_buffer.bytes.lock().len())
                }
            } else {
                None
            }
        }),
        _ => None,
    }
    .ok_or_else(|| Error::type_err("DataView buffer must be an ArrayBuffer"))?;

    let byte_offset = match args.get(1) {
        Some(value) => to_index_length(vm, value, "DataView byteOffset")?,
        None => 0,
    };
    if byte_offset > buffer_len {
        return Err(Error::range("Invalid DataView byteOffset"));
    }
    let byte_length = match args.get(2) {
        Some(Value::Undefined) | None => buffer_len - byte_offset,
        Some(value) => to_index_length(vm, value, "DataView byteLength")?,
    };
    if byte_offset
        .checked_add(byte_length)
        .is_none_or(|end| end > buffer_len)
    {
        return Err(Error::range("Invalid DataView byteLength"));
    }

    let proto = native_constructor_prototype(vm, vm.object_proto.clone())?;
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
    let fallback_proto = vm
        .current_native_callee
        .clone()
        .and_then(|callee| {
            vm.get_property_by_key(&callee, &PropertyKey::from("prototype"))
                .ok()
        })
        .filter(|proto| matches!(proto, Value::Object(_)))
        .unwrap_or_else(|| vm.object_proto.clone());
    let proto = native_constructor_prototype(vm, fallback_proto)?;
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
                    return typed_array_iterable_to_bytes(vm, kind, &array_like).and_then(
                        |buffer| {
                            let idx = vm.heap.allocate(HeapObj::TypedArray(
                                crate::value::TypedArrayData {
                                    buffer: Mutex::new(buffer),
                                    kind,
                                    props: Mutex::new(IndexMap::new()),
                                    proto: Mutex::new(Some(proto)),
                                    extensible: AtomicBool::new(true),
                                },
                            ))?;
                            Ok(Value::Object(GcIdx(idx)))
                        },
                    );
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

    let idx = vm
        .heap
        .allocate(HeapObj::TypedArray(crate::value::TypedArrayData {
            buffer: Mutex::new(buffer),
            kind,
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
            extensible: AtomicBool::new(true),
        }))?;
    Ok(Value::Object(GcIdx(idx)))
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
