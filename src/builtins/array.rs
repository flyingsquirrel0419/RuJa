use super::*;

// =========================================================================
// Array prototype + constructor
// =========================================================================

pub(crate) fn array_from(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let src_val = args.first().cloned().unwrap_or(Value::Undefined);
    let map_fn = args.get(1).cloned();
    // Array-like or iterable
    let mut items: Vec<Value> = Vec::new();
    // Cap total materialized elements so an infinite or huge iterable (e.g.
    // a generator that yields forever) cannot OOM the host. Matches the
    // engine's dense-array model.
    // Cap total materialized elements so an infinite or huge iterable (e.g.
    // a generator that yields forever) cannot OOM the host. 65k keeps an
    // infinite iterable from running for many seconds before the cap trips.
    const MAX_ARRAY_FROM_LEN: usize = 1 << 16; // 65,536
    if let Value::Object(idx) = &src_val {
        // Iterable protocol first: if the object has a Symbol.iterator
        // (generators, sets, maps, user iterables), drain it. This was
        // missing before, so `Array.from(gen())` returned [] instead of the
        // elements. Capped so an infinite iterable cannot OOM the host.
        let has_iter = vm.heap.with_obj(idx.0, |o| {
            matches!(
                o,
                HeapObj::Generator(_)
                    | HeapObj::LazyGenerator(_)
                    | HeapObj::Map(_)
                    | HeapObj::Set(_)
            )
        }) || {
            let pkey = crate::value::PropertyKey::Symbol(vm.well_known_symbols.iterator);
            vm.has_property_key(&src_val, &pkey)
        };
        if has_iter {
            let iter = vm.make_iterator(&src_val)?;
            // The iterator must survive any GC triggered by `iterator_next`
            // (which allocates result objects). Without pinning, the iterator
            // was collected mid-loop, so an infinite iterable stopped at ~593
            // elements instead of running to the cap.
            let pin = vm.pin(&iter);
            loop {
                if items.len() >= MAX_ARRAY_FROM_LEN {
                    vm.unpin(pin);
                    return Err(Error::range("Invalid array length"));
                }
                let (v, done) = vm.iterator_next(&iter)?;
                if done {
                    break;
                }
                items.push(v);
            }
            vm.unpin(pin);
            return finish_array_from(vm, items, map_fn);
        }
        let (is_arr, arr_items, len) = vm.heap.with_obj(idx.0, |o| {
            if let HeapObj::Array(a) = o {
                (true, a.items.lock().clone(), 0)
            } else if let HeapObj::Iterator(_) = o {
                (false, Vec::new(), 0)
            } else {
                let len = o
                    .props()
                    .lock()
                    .get(&crate::value::PropertyKey::from("length"))
                    .and_then(|d| {
                        if let Value::Number(n) = d.value {
                            Some(n as usize)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                (false, Vec::new(), len)
            }
        });
        if is_arr {
            items = arr_items;
        } else {
            // array-like: read index 0..len. Cap to prevent untrusted input
            // like `{length: 2**26}` from forcing a multi-second / multi-GB
            // materialization (a trivial DoS). Node tolerates large lengths
            // but RuJa materializes densely, so we bound it.
            if len > MAX_ARRAY_FROM_LEN {
                return Err(Error::range("Invalid array length"));
            }
            for i in 0..len {
                let key = i.to_string();
                let v = vm.get_property(&src_val, &key)?;
                items.push(v);
            }
        }
    } else if let Value::String(s) = &src_val {
        for ch in s.chars() {
            items.push(Value::String(Arc::from(ch.to_string().as_str())));
        }
    }
    finish_array_from(vm, items, map_fn)
}

/// Apply an optional map function and box the items into an Array.
pub(crate) fn finish_array_from(
    vm: &mut Vm,
    mut items: Vec<Value>,
    map_fn: Option<Value>,
) -> error::Result<Value> {
    if let Some(mfn) = map_fn {
        let mut mapped = Vec::new();
        for (i, v) in items.iter().enumerate() {
            mapped.push(vm.call_function(&mfn, &[v.clone(), Value::Number(i as f64)], None)?);
        }
        items = mapped;
    }
    Ok(make_value_array(vm, items))
}
pub(crate) fn array_of(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    Ok(make_value_array(vm, args.to_vec()))
}

pub(crate) fn array_is_array(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(is_array(
        args.first().unwrap_or(&Value::Undefined),
        &vm.heap,
    )))
}
pub(crate) fn array_push(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().extend_from_slice(args);
            }
        });
        let len = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().len()
            } else {
                0
            }
        });
        return Ok(Value::Number(len as f64));
    }
    Ok(Value::Number(0.0))
}
pub(crate) fn array_pop(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        return Ok(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().pop().unwrap_or(Value::Undefined)
            } else {
                Value::Undefined
            }
        }));
    }
    Ok(Value::Undefined)
}
/// Array.prototype.toString: delegates to join(",") (Object.prototype.toString
/// would otherwise return "[object Array]").
pub(crate) fn array_to_string(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    array_join(vm, &[], this)
}

pub(crate) fn array_join(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let sep = match args.first() {
        Some(Value::String(s)) => s.to_string(),
        Some(v) if !v.is_undefined() => vm.to_string(v)?.to_string(),
        _ => ",".to_string(),
    };
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let parts: Vec<String> = items
            .iter()
            .map(|i| {
                if i.is_nullish() {
                    String::new()
                } else {
                    vm.to_string(i).map(|s| s.to_string()).unwrap_or_default()
                }
            })
            .collect();
        return Ok(Value::String(Arc::from(parts.join(&sep).as_str())));
    }
    Ok(Value::String(Arc::from("")))
}
pub(crate) fn array_map(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let mut result = Vec::new();
        for (i, item) in items.iter().enumerate() {
            result.push(vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?);
        }
        let arr = HeapObj::Array(ArrayData {
            items: Mutex::new(result),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.array_proto.clone())),
            sparse_max: Mutex::new(None),
        });
        return Ok(Value::Object(GcIdx(vm.heap.allocate(arr))));
    }
    Ok(Value::Undefined)
}
pub(crate) fn array_filter(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let mut result = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let keep = vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?;
            if keep.is_truthy() {
                result.push(item.clone());
            }
        }
        let arr = HeapObj::Array(ArrayData {
            items: Mutex::new(result),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.array_proto.clone())),
            sparse_max: Mutex::new(None),
        });
        return Ok(Value::Object(GcIdx(vm.heap.allocate(arr))));
    }
    Ok(Value::Undefined)
}
pub(crate) fn array_reduce(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let (mut acc, start) = if args.len() >= 2 {
            (args.get(1).cloned().unwrap_or(Value::Undefined), 0)
        } else {
            (items.first().cloned().unwrap_or(Value::Undefined), 1)
        };
        if items.is_empty() && args.len() < 2 {
            return Err(Error::type_err(
                "Reduce of empty array with no initial value",
            ));
        }
        for (i, item) in items.iter().enumerate().skip(start) {
            acc = vm.call_function(
                &cb,
                &[
                    acc,
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(2).cloned(),
            )?;
        }
        return Ok(acc);
    }
    Ok(Value::Undefined)
}
/// Build a heap array from a Vec of values.
pub(crate) fn make_array(vm: &mut Vm, items: Vec<Value>) -> Value {
    let idx = vm.heap.allocate(HeapObj::Array(crate::value::ArrayData {
        items: Mutex::new(items),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
        sparse_max: Mutex::new(None),
    }));
    Value::Object(GcIdx(idx))
}

/// Normalize an array index argument (negative wraps from end).
pub(crate) fn norm_index(v: Value, len: f64, vm: &mut Vm) -> error::Result<usize> {
    let n = vm.to_number(&v)?;
    if n < 0.0 {
        Ok(((len + n).max(0.0)) as usize)
    } else {
        Ok((n as usize).min(len as usize))
    }
}

/// Sort items with an optional comparator callback (default: string compare).
pub(crate) fn sort_with_cb(vm: &mut Vm, items: &mut [Value], cmp: &Option<Value>) -> error::Result<()> {
    match cmp {
        None | Some(Value::Undefined) | Some(Value::Null) => {
            items.sort_by(|a, b| {
                if a.is_undefined() && b.is_undefined() {
                    return std::cmp::Ordering::Equal;
                }
                if a.is_undefined() {
                    return std::cmp::Ordering::Greater;
                }
                if b.is_undefined() {
                    return std::cmp::Ordering::Less;
                }
                let sa = vm.to_string(a).map(|s| s.to_string()).unwrap_or_default();
                let sb = vm.to_string(b).map(|s| s.to_string()).unwrap_or_default();
                sa.cmp(&sb)
            });
        }
        Some(cmp_fn) => {
            // Stable O(n log n) merge sort. The previous O(n^2) bubble sort
            // made sorting 10k random elements with a comparator take ~30s (a
            // trivial DoS). A hand-rolled merge sort is used instead of
            // `slice::sort_by` because the ES comparator may have side
            // effects (mutating VM state during `call_function`), which
            // defeats pdqsort's purity assumptions and degrades it to O(n^2).
            // Merge sort compares each pair at most once per merge level and
            // stays O(n log n) regardless. NaN / non-number results are
            // treated as 0 (equal), matching ES SortCompare; the first thrown
            // error is captured and propagated after sorting settles.
            let err: std::cell::Cell<Option<Arc<crate::error::Error>>> = std::cell::Cell::new(None);
            let mut compare = |vm: &mut Vm, a: &Value, b: &Value| -> std::cmp::Ordering {
                // If a comparator call already threw, short-circuit: restore
                // the captured error (Cell::take cleared it) and return Equal
                // so the sort settles quickly without more VM calls.
                if let Some(e) = err.take() {
                    err.set(Some(e));
                    return std::cmp::Ordering::Equal;
                }
                match vm.call_function(cmp_fn, &[a.clone(), b.clone()], None) {
                    Ok(r) => match vm.to_number(&r) {
                        Ok(ord) => {
                            if ord.is_nan() {
                                std::cmp::Ordering::Equal
                            } else if ord < 0.0 {
                                std::cmp::Ordering::Less
                            } else if ord > 0.0 {
                                std::cmp::Ordering::Greater
                            } else {
                                std::cmp::Ordering::Equal
                            }
                        }
                        Err(_) => std::cmp::Ordering::Equal,
                    },
                    Err(e) => {
                        err.set(Some(e));
                        std::cmp::Ordering::Equal
                    }
                }
            };
            merge_sort(vm, items, &mut compare)?;
            if let Some(e) = err.into_inner() {
                return Err(e);
            }
        }
    }
    Ok(())
}

/// In-place stable merge sort. `compare` may mutate the VM (ES comparators
/// can have side effects); unlike `slice::sort_by`, this never degrades to
/// O(n^2) on an inconsistent/side-effecting comparator because each pair is
/// compared at most once along a given merge path.
fn merge_sort<F>(vm: &mut Vm, items: &mut [Value], compare: &mut F) -> error::Result<()>
where
    F: FnMut(&mut Vm, &Value, &Value) -> std::cmp::Ordering,
{
    let n = items.len();
    if n < 2 {
        return Ok(());
    }
    // Bottom-up merge sort with a scratch buffer.
    let mut buf: Vec<Value> = Vec::with_capacity(n);
    let mut width = 1;
    while width < n {
        let mut i = 0;
        while i < n {
            let left = i;
            let mid = (i + width).min(n);
            let right = (i + 2 * width).min(n);
            // Merge [left, mid) and [mid, right) into buf, then copy back.
            let mut a = left;
            let mut b = mid;
            buf.clear();
            while a < mid && b < right {
                if compare(vm, &items[a], &items[b]) == std::cmp::Ordering::Greater {
                    buf.push(items[b].clone());
                    b += 1;
                } else {
                    buf.push(items[a].clone());
                    a += 1;
                }
            }
            while a < mid {
                buf.push(items[a].clone());
                a += 1;
            }
            while b < right {
                buf.push(items[b].clone());
                b += 1;
            }
            items[left..right].clone_from_slice(&buf);
            i += 2 * width;
        }
        width *= 2;
    }
    Ok(())
}

pub(crate) fn array_reduce_right(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len();
        let (mut acc, start) = if args.len() >= 2 {
            (args.get(1).cloned().unwrap_or(Value::Undefined), len)
        } else {
            (
                items.last().cloned().unwrap_or(Value::Undefined),
                len.saturating_sub(1),
            )
        };
        if items.is_empty() && args.len() < 2 {
            return Err(Error::type_err(
                "Reduce of empty array with no initial value",
            ));
        }
        let mut i = start;
        while i > 0 {
            i -= 1;
            acc = vm.call_function(
                &cb,
                &[
                    acc,
                    items[i].clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(2).cloned(),
            )?;
        }
        return Ok(acc);
    }
    Ok(Value::Undefined)
}

pub(crate) fn array_to_reversed(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().iter().rev().cloned().collect()
            } else {
                Vec::new()
            }
        });
        return Ok(make_array(vm, items));
    }
    Ok(Value::Undefined)
}

pub(crate) fn array_to_sorted(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let mut items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let cb = args.first().cloned();
        sort_with_cb(vm, &mut items, &cb)?;
        return Ok(make_array(vm, items));
    }
    Ok(Value::Undefined)
}

pub(crate) fn array_to_spliced(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len() as f64;
        let start = norm_index(get_arg(args, 0), len, vm)?;
        let start = start.min(items.len());
        let del_count = if args.len() >= 2 {
            let d = vm.to_number(&get_arg(args, 1))?;
            let d = if d < 0.0 { 0.0 } else { d };
            (d as usize).min(items.len().saturating_sub(start))
        } else {
            items.len() - start
        };
        let mut result = items[..start].to_vec();
        for a in args.iter().skip(2) {
            result.push(a.clone());
        }
        result.extend_from_slice(&items[start + del_count..]);
        return Ok(make_array(vm, result));
    }
    Ok(Value::Undefined)
}

pub(crate) fn array_with(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let mut items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len() as f64;
        let index = norm_index(get_arg(args, 0), len, vm)?;
        if index >= items.len() {
            return Err(Error::range("Invalid array index"));
        }
        items[index] = get_arg(args, 1);
        return Ok(make_array(vm, items));
    }
    Ok(Value::Undefined)
}

pub(crate) fn array_for_each(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        for (i, item) in items.iter().enumerate() {
            vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?;
        }
    }
    Ok(Value::Undefined)
}
/// Resolve a `fromIndex`-style argument (ToInteger, default 0) to a starting
/// position clamped into `[0, len]`. Negative wraps from the end.
pub(crate) fn from_index_arg(vm: &mut Vm, args: &[Value], idx: usize, len: usize) -> error::Result<usize> {
    let raw = match args.get(idx) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    if raw.is_nan() || raw == 0.0 || raw.is_infinite() {
        // +Inf -> len, -Inf/-0/NaN -> 0
        return Ok(if raw.is_infinite() && raw > 0.0 {
            len
        } else {
            0
        });
    }
    let n = raw as i64;
    let start = if n < 0 {
        (len as i64 + n).max(0) as usize
    } else {
        (n as usize).min(len)
    };
    Ok(start)
}

pub(crate) fn array_index_of(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len();
        let start = from_index_arg(vm, args, 1, len)?;
        for (i, v) in items.iter().enumerate().skip(start) {
            if v == &target {
                return Ok(Value::Number(i as f64));
            }
        }
        return Ok(Value::Number(-1.0));
    }
    Ok(Value::Number(-1.0))
}
pub(crate) fn array_includes(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len();
        let start = from_index_arg(vm, args, 1, len)?;
        // includes uses SameValueZero: NaN matches NaN (unlike indexOf's ===).
        for (_i, v) in items.iter().enumerate().skip(start) {
            if v.same_value_zero(&target) {
                return Ok(Value::Bool(true));
            }
        }
        return Ok(Value::Bool(false));
    }
    Ok(Value::Bool(false))
}
pub(crate) fn array_slice(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len() as i64;
        let start = args
            .first()
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i64)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let end = args
            .get(1)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i64)
                } else {
                    None
                }
            })
            .unwrap_or(len);
        let s = if start < 0 {
            (len + start).max(0) as usize
        } else {
            (start as usize).min(items.len())
        };
        let e = if end < 0 {
            (len + end).max(0) as usize
        } else {
            (end as usize).min(items.len())
        };
        let sliced = if s < e {
            items[s..e].to_vec()
        } else {
            Vec::new()
        };
        let arr = HeapObj::Array(ArrayData {
            items: Mutex::new(sliced),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(vm.array_proto.clone())),
            sparse_max: Mutex::new(None),
        });
        return Ok(Value::Object(GcIdx(vm.heap.allocate(arr))));
    }
    Ok(Value::Undefined)
}
pub(crate) fn array_concat(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let mut items = Vec::new();
    if let Some(Value::Object(idx)) = this {
        items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
    }
    for a in args {
        if let Value::Object(aidx) = a {
            let is_arr = vm
                .heap
                .with_obj(aidx.0, |obj| matches!(obj, HeapObj::Array(_)));
            if is_arr {
                let extra = vm.heap.with_obj(aidx.0, |obj| {
                    if let HeapObj::Array(a) = obj {
                        a.items.lock().clone()
                    } else {
                        Vec::new()
                    }
                });
                items.extend(extra);
                continue;
            }
        }
        items.push(a.clone());
    }
    let arr = HeapObj::Array(ArrayData {
        items: Mutex::new(items),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
        sparse_max: Mutex::new(None),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}

pub(crate) fn array_reverse(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().reverse();
            }
        });
        return Ok(Value::Object(idx));
    }
    Ok(Value::Undefined)
}

pub(crate) fn array_sort(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cmp = args.first().cloned();
    if let Some(Value::Object(idx)) = this {
        // Collect items, sort via comparator (default: cast to string, UTF-16 code unit compare).
        let mut items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        match cmp {
            None | Some(Value::Undefined) | Some(Value::Null) => {
                // default: compare by string representation; undefined sorts to end.
                items.sort_by(|a, b| {
                    if a.is_undefined() && b.is_undefined() {
                        return std::cmp::Ordering::Equal;
                    }
                    if a.is_undefined() {
                        return std::cmp::Ordering::Greater;
                    }
                    if b.is_undefined() {
                        return std::cmp::Ordering::Less;
                    }
                    let sa = vm.to_string(a).map(|s| s.to_string()).unwrap_or_default();
                    let sb = vm.to_string(b).map(|s| s.to_string()).unwrap_or_default();
                    sa.cmp(&sb)
                });
            }
            Some(cmp_fn) => {
                // Delegate to the O(n log n) merge sort in `sort_with_cb`,
                // which also handles NaN comparator results and thrown errors
                // per ES SortCompare. (The previous inline O(n^2) insertion
                // sort made `a.sort(cmp)` on 10k elements take ~30s.)
                let cmp_arg = Some(cmp_fn);
                sort_with_cb(vm, &mut items, &cmp_arg)?;
            }
        }
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                *a.items.lock() = items;
            }
        });
        return Ok(Value::Object(idx));
    }
    Ok(Value::Undefined)
}

pub(crate) fn array_shift(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        return Ok(vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                let mut items = a.items.lock();
                if items.is_empty() {
                    Value::Undefined
                } else {
                    items.remove(0)
                }
            } else {
                Value::Undefined
            }
        }));
    }
    Ok(Value::Undefined)
}
pub(crate) fn array_unshift(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                let mut items = a.items.lock();
                for (i, v) in args.iter().enumerate() {
                    items.insert(i, v.clone());
                }
            }
        });
        let len = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().len()
            } else {
                0
            }
        });
        return Ok(Value::Number(len as f64));
    }
    Ok(Value::Number(0.0))
}
pub(crate) fn array_splice(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let items_clone = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let len = items_clone.len() as f64;
        let start = match args.first() {
            Some(v) => vm.to_number(v)?,
            None => 0.0,
        };
        let start = if start < 0.0 {
            (len + start).max(0.0) as usize
        } else {
            (start as usize).min(items_clone.len())
        };
        let delete_count = match args.get(1) {
            Some(v) => vm.to_number(v)?,
            None => 0.0,
        };
        let delete_count = if delete_count < 0.0 {
            0
        } else {
            (delete_count as usize).min(items_clone.len() - start)
        };
        let removed: Vec<Value> = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                let mut items = a.items.lock();
                let r: Vec<Value> = items.drain(start..start + delete_count).collect();
                for (i, v) in args.iter().skip(2).enumerate() {
                    items.insert(start + i, v.clone());
                }
                r
            } else {
                Vec::new()
            }
        });
        let arr = make_value_array(vm, removed);
        return Ok(arr);
    }
    Ok(Value::Undefined)
}
pub(crate) fn array_last_index_of(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let target = args.first().unwrap_or(&Value::Undefined).clone();
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len();
        // fromIndex for lastIndexOf: default +Inf (start from end); negative
        // wraps from the end; clamped into [0, len-1].
        let raw = match args.get(1) {
            Some(v) => vm.to_number(v)?,
            None => f64::INFINITY,
        };
        let end = if raw.is_nan() {
            len
        } else if raw.is_infinite() && raw < 0.0 {
            return Ok(Value::Number(-1.0));
        } else {
            let n = raw as i64;
            (if n < 0 { len as i64 + n } else { n }).clamp(0, len as i64) as usize
        };
        for i in (0..end).rev() {
            if vm.strict_eq(&items[i], &target) {
                return Ok(Value::Number(i as f64));
            }
        }
    }
    Ok(Value::Number(-1.0))
}
pub(crate) fn array_at(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let n = match args.first() {
            Some(v) => vm.to_number(v)?,
            None => 0.0,
        };
        let len = items.len() as isize;
        let idx = if n < 0.0 {
            len + n as isize
        } else {
            n as isize
        };
        if idx >= 0 && idx < len {
            return Ok(items[idx as usize].clone());
        }
    }
    Ok(Value::Undefined)
}
pub(crate) fn array_flat(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let depth = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => 1.0,
    };
    let depth = if depth < 0.0 { 0 } else { depth as usize };
    fn flatten(vm: &mut Vm, items: &[Value], depth: usize, out: &mut Vec<Value>) {
        for v in items {
            let is_arr = match v {
                Value::Object(idx) => vm.heap.with_obj(idx.0, |o| matches!(o, HeapObj::Array(_))),
                _ => false,
            };
            if is_arr && depth > 0 {
                let sub = vm.heap.with_obj(
                    match v {
                        Value::Object(i) => i.0,
                        _ => 0,
                    },
                    |o| {
                        if let HeapObj::Array(a) = o {
                            a.items.lock().clone()
                        } else {
                            Vec::new()
                        }
                    },
                );
                flatten(vm, &sub, depth - 1, out);
            } else {
                out.push(v.clone());
            }
        }
    }
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let mut out = Vec::new();
        flatten(vm, &items, depth, &mut out);
        return Ok(make_value_array(vm, out));
    }
    Ok(Value::Undefined)
}
pub(crate) fn array_flat_map(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    // flatMap(fn) = map(fn).flat(1)
    let items = if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    let fn_val = args.first().cloned().unwrap_or(Value::Undefined);
    let mut mapped: Vec<Value> = Vec::new();
    for (i, v) in items.iter().enumerate() {
        let result = vm.call_function(
            &fn_val,
            &[
                v.clone(),
                Value::Number(i as f64),
                this.clone().unwrap_or(Value::Undefined),
            ],
            None,
        )?;
        mapped.push(result);
    }
    let mut out = Vec::new();
    for v in &mapped {
        let is_arr = match v {
            Value::Object(idx) => vm.heap.with_obj(idx.0, |o| matches!(o, HeapObj::Array(_))),
            _ => false,
        };
        if is_arr {
            let sub = vm.heap.with_obj(
                match v {
                    Value::Object(i) => i.0,
                    _ => 0,
                },
                |o| {
                    if let HeapObj::Array(a) = o {
                        a.items.lock().clone()
                    } else {
                        Vec::new()
                    }
                },
            );
            out.extend(sub);
        } else {
            out.push(v.clone());
        }
    }
    Ok(make_value_array(vm, out))
}
pub(crate) fn array_copy_within(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    if let Some(Value::Object(idx)) = this {
        let len = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().len()
            } else {
                0
            }
        }) as f64;
        let target = match args.first() {
            Some(v) => vm.to_number(v)?,
            None => 0.0,
        };
        let start = match args.get(1) {
            Some(v) => vm.to_number(v)?,
            None => 0.0,
        };
        let end = match args.get(2) {
            Some(v) => vm.to_number(v)?,
            None => len,
        };
        let to = norm_idx(target, len) as usize;
        let from = norm_idx(start, len) as usize;
        let last = if end < 0.0 {
            (len + end).max(0.0) as usize
        } else {
            (end as usize).min(len as usize)
        };
        if from >= last || to >= len as usize {
            return Ok(Value::Object(idx));
        }
        let count = (last - from).min(len as usize - to);
        let src: Vec<Value> = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock()[from..from + count].to_vec()
            } else {
                Vec::new()
            }
        });
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                let mut items = a.items.lock();
                for (i, v) in src.into_iter().enumerate() {
                    items[to + i] = v;
                }
            }
        });
        return Ok(Value::Object(idx));
    }
    Ok(Value::Undefined)
}
pub(crate) fn array_keys(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let len = if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().len()
            } else {
                0
            }
        })
    } else {
        0
    };
    let items: Vec<Value> = (0..len).map(|i| Value::Number(i as f64)).collect();
    Ok(make_value_array(vm, items))
}
pub(crate) fn array_values(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    Ok(make_value_array(vm, items))
}
pub(crate) fn array_entries(vm: &mut Vm, _args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let items = if let Some(Value::Object(idx)) = this {
        vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        })
    } else {
        Vec::new()
    };
    let pairs: Vec<Value> = items
        .iter()
        .enumerate()
        .map(|(i, v)| make_value_array(vm, vec![Value::Number(i as f64), v.clone()]))
        .collect();
    Ok(make_value_array(vm, pairs))
}

pub(crate) fn array_constructor(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    // `Array(n)` / `new Array(n)` with a single number argument creates a
    // sparse array of length n (filled with holes). Other argument forms
    // create an array of the given elements. `this` (from `new`) is ignored:
    // ES ArrayConstructor always returns a fresh Array exotic object, not the
    // `[[Construct]]`-provided ordinary object.
    let items = if args.len() == 1 {
        if let Some(Value::Number(n)) = args.first() {
            // Validate the length per ArrayCreate: must be a non-negative
            // integer that fits in u32. Negative / fractional / huge values
            // throw RangeError, not an OOM abort.
            if n.is_nan() || *n < 0.0 || n.is_infinite() || n.fract() != 0.0 {
                return Err(Error::range("Invalid array length"));
            }
            if *n >= (1u64 << 32) as f64 {
                return Err(Error::range("Invalid array length"));
            }
            // Avoid attempting an enormous allocation: cap at a sane limit.
            let len = *n as usize;
            if len > 1 << 24 {
                return Err(Error::range("Invalid array length"));
            }
            vec![Value::Undefined; len]
        } else {
            args.to_vec()
        }
    } else {
        args.to_vec()
    };
    let arr = HeapObj::Array(ArrayData {
        items: Mutex::new(items),
        props: Mutex::new(IndexMap::new()),
        proto: Mutex::new(Some(vm.array_proto.clone())),
        sparse_max: Mutex::new(None),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(arr))))
}

pub(crate) fn array_find(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        for (i, item) in items.iter().enumerate() {
            let found = vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?;
            if found.is_truthy() {
                return Ok(item.clone());
            }
        }
    }
    Ok(Value::Undefined)
}
pub(crate) fn array_find_index(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        for (i, item) in items.iter().enumerate() {
            let found = vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?;
            if found.is_truthy() {
                return Ok(Value::Number(i as f64));
            }
        }
    }
    Ok(Value::Number(-1.0))
}
pub(crate) fn array_find_last(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        for (i, item) in items.iter().enumerate().rev() {
            let found = vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?;
            if found.is_truthy() {
                return Ok(item.clone());
            }
        }
    }
    Ok(Value::Undefined)
}
pub(crate) fn array_fill(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let value = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        let len = items.len() as i64;
        let start = args
            .get(1)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i64)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let end = args
            .get(2)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i64)
                } else {
                    None
                }
            })
            .unwrap_or(len);
        let s = if start < 0 {
            (len + start).max(0) as usize
        } else {
            (start as usize).min(items.len())
        };
        let e = if end < 0 {
            (len + end).max(0) as usize
        } else {
            (end as usize).min(items.len())
        };
        if s < e {
            vm.heap.with_obj(idx.0, |obj| {
                if let HeapObj::Array(a) = obj {
                    let mut items = a.items.lock();
                    for i in s..e.min(items.len()) {
                        items[i] = value.clone();
                    }
                }
            });
        }
        return Ok(Value::Object(idx));
    }
    Ok(Value::Undefined)
}
pub(crate) fn array_some(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        for (i, item) in items.iter().enumerate() {
            let found = vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?;
            if found.is_truthy() {
                return Ok(Value::Bool(true));
            }
        }
    }
    Ok(Value::Bool(false))
}
pub(crate) fn array_every(vm: &mut Vm, args: &[Value], this: Option<Value>) -> error::Result<Value> {
    let cb = args.first().cloned().unwrap_or(Value::Undefined);
    if let Some(Value::Object(idx)) = this {
        let items = vm.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Array(a) = obj {
                a.items.lock().clone()
            } else {
                Vec::new()
            }
        });
        for (i, item) in items.iter().enumerate() {
            let ok = vm.call_function(
                &cb,
                &[
                    item.clone(),
                    Value::Number(i as f64),
                    this.clone().unwrap_or(Value::Undefined),
                ],
                args.get(1).cloned(),
            )?;
            if !ok.is_truthy() {
                return Ok(Value::Bool(false));
            }
        }
    }
    Ok(Value::Bool(true))
}

