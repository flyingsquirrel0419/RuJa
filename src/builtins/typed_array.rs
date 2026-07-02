use super::*;

pub(crate) fn uint8array_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
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
                    buf.push(vm.to_number(item)? as u8);
                }
                let idx = vm
                    .heap
                    .allocate(HeapObj::TypedArray(crate::value::TypedArrayData {
                        buffer: buf,
                        kind: crate::value::TypedArrayKind::Uint8,
                        props: Mutex::new(IndexMap::new()),
                        proto: Mutex::new(Some(vm.object_proto.clone())),
                    }))?;
                return Ok(Value::Object(GcIdx(idx)));
            }
            _ => 0,
        }
    };

    let idx = vm
        .heap
        .allocate(HeapObj::TypedArray(crate::value::TypedArrayData {
            buffer: vec![0u8; length],
            kind: crate::value::TypedArrayKind::Uint8,
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.object_proto.clone())),
        }))?;
    Ok(Value::Object(GcIdx(idx)))
}
