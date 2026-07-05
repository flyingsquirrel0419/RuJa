use super::*;

const MAX_ARRAY_BUFFER_LENGTH: usize = 1 << 26;

fn to_index_length(vm: &mut Vm, value: &Value, name: &str) -> error::Result<usize> {
    let n = vm.to_number(value)?;
    if !n.is_finite() || n < 0.0 {
        return Err(Error::range(format!("Invalid {name} length")));
    }
    let integer = n.trunc();
    if integer > MAX_ARRAY_BUFFER_LENGTH as f64 {
        return Err(Error::range(format!("Invalid {name} length")));
    }
    Ok(integer as usize)
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

pub(crate) fn to_uint8_element(n: f64) -> u8 {
    if !n.is_finite() || n == 0.0 {
        return 0;
    }
    n.trunc().rem_euclid(256.0) as u8
}

pub(crate) fn uint8array_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let proto = native_constructor_prototype(vm, vm.object_proto.clone())?;
    let length = if args.is_empty() {
        0usize
    } else {
        match &args[0] {
            Value::Number(n) => {
                if *n < 0.0 || n.is_nan() || n.is_infinite() {
                    return Err(Error::type_err("Invalid typed array length".to_string()));
                }
                *n as usize
            }
            Value::Object(idx) => {
                // Initialize from array-like: copy elements.
                let items = vm.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Array(a) = o {
                        a.items.lock().clone()
                    } else {
                        Vec::new()
                    }
                });
                let mut buf = Vec::with_capacity(items.len());
                for item in &items {
                    buf.push(to_uint8_element(vm.to_number(item)?));
                }
                let idx = vm
                    .heap
                    .allocate(HeapObj::TypedArray(crate::value::TypedArrayData {
                        buffer: Mutex::new(buf),
                        kind: crate::value::TypedArrayKind::Uint8,
                        props: Mutex::new(IndexMap::new()),
                        proto: Mutex::new(Some(proto)),
                    }))?;
                return Ok(Value::Object(GcIdx(idx)));
            }
            _ => 0,
        }
    };

    let idx = vm
        .heap
        .allocate(HeapObj::TypedArray(crate::value::TypedArrayData {
            buffer: Mutex::new(vec![0u8; length]),
            kind: crate::value::TypedArrayKind::Uint8,
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
        }))?;
    Ok(Value::Object(GcIdx(idx)))
}
