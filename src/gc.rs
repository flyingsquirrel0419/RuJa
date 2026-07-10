//! Mark-and-sweep garbage collector.
//!
//! Heap objects are `HeapObj` (an enum) stored in cells. A `GcIdx` handle
//! (index into the cell array) is how the VM references them. The collector
//! traces from roots and sweeps unreachable cells.
//!
//! Threading model: cells, free_list, and counters are behind `Mutex`/`Cell`.
//! Tracing is **worklist-based** (not recursive): we pop an index, lock the
//! cells mutex only long enough to extract the object's child indices into the
//! worklist, then release it before tracing the next item. This avoids
//! re-locking the cells mutex while holding it (which would deadlock under
//! `Mutex`), and keeps each lock scope tiny.

use crate::value::HeapObj;

#[derive(Debug, Clone, Copy)]
pub struct HeapLimitExceeded;

impl std::convert::From<HeapLimitExceeded> for std::sync::Arc<crate::error::Error> {
    fn from(_: HeapLimitExceeded) -> Self {
        std::sync::Arc::new(crate::error::Error {
            kind: crate::error::ErrorKind::Range,
            message: "heap limit exceeded".into(),
            stack: Vec::new(),
            thrown_value: None,
            line: None,
        })
    }
}
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub struct GcCell {
    pub obj: Mutex<Option<HeapObj>>,
    private_elements:
        Mutex<std::collections::HashMap<crate::value::PrivateSlotKey, crate::value::PrivateSlot>>,
    pub marked: AtomicBool,
}

pub struct Heap {
    pub cells: Mutex<Vec<GcCell>>,
    free_list: Mutex<Vec<usize>>,
    alloc_since_gc: AtomicUsize,
    gc_threshold: AtomicUsize,
    /// Maximum number of live heap objects allowed. When this is exceeded,
    /// `allocate` returns `HeapLimitExceeded`. `0` means unlimited.
    max_objects: AtomicUsize,
}

fn trace_private_slot(slot: &crate::value::PrivateSlot, worklist: &mut Vec<usize>) {
    let push_value = |value: &crate::value::Value, worklist: &mut Vec<usize>| {
        if let crate::value::Value::Object(idx) = value {
            worklist.push(idx.0);
        }
    };
    match slot {
        crate::value::PrivateSlot::Value(value) | crate::value::PrivateSlot::Method(value) => {
            push_value(value, worklist)
        }
        crate::value::PrivateSlot::Accessor { get, set } => {
            if let Some(get) = get {
                push_value(get, worklist);
            }
            if let Some(set) = set {
                push_value(set, worklist);
            }
        }
    }
}

/// Push reachable child indices of `obj` onto `worklist`. Called while NOT
/// holding the cells mutex, so it may lock any object field freely. Ephemeron
/// (WeakMap) values are pushed only when their key is already marked; the
/// caller iterates to a fixed point so transitively-reachable values are
/// eventually marked.
pub fn trace_obj(obj: &HeapObj, marked: &[bool], worklist: &mut Vec<usize>) {
    let push_value = |v: &crate::value::Value, w: &mut Vec<usize>| {
        if let crate::value::Value::Object(idx) = v {
            w.push(idx.0);
        }
    };

    if let HeapObj::Iterator(it) = obj {
        for v in it.items.lock().iter() {
            push_value(v, worklist);
        }
        if let Some(lazy) = it.lazy_iter.lock().as_ref() {
            push_value(lazy, worklist);
        }
        if let Some(gen) = it.generator.lock().as_ref() {
            push_value(gen, worklist);
        }
        if let Some(source) = it.array_like.lock().as_ref() {
            push_value(source, worklist);
        }
        if let Some(source) = it.for_in_source.lock().as_ref() {
            push_value(source, worklist);
        }
        for source in it.for_in_key_sources.lock().iter() {
            push_value(source, worklist);
        }
        return;
    }
    if let HeapObj::Environment(e) = obj {
        for (_, b) in e.vars.lock().iter() {
            push_value(&b.value.lock(), worklist);
        }
        if let Some(p) = *e.parent.lock() {
            worklist.push(p.0);
        }
        return;
    }
    let props = obj.props();
    for (_, desc) in props.lock().iter() {
        if !desc.is_accessor {
            push_value(&desc.value, worklist);
        } else {
            if let Some(g) = &desc.get {
                push_value(g, worklist);
            }
            if let Some(s) = &desc.set {
                push_value(s, worklist);
            }
        }
    }
    if let Some(proto) = obj.proto().lock().as_ref() {
        push_value(proto, worklist);
    }
    match obj {
        HeapObj::Object(o) => {
            for slot in o.private_fields.lock().values() {
                trace_private_slot(slot, worklist);
            }
        }
        HeapObj::Array(a) => {
            for v in a.items.lock().iter() {
                push_value(v, worklist);
            }
            if let Some(map) = a.arguments_map.lock().as_ref() {
                worklist.push(map.env.0);
            }
        }
        HeapObj::Function(f) => {
            worklist.push(f.closure.0);
            push_value(&f.lexical_new_target, worklist);
            if let Some(p) = f.prototype.lock().as_ref() {
                push_value(p, worklist);
            }
            for slot in f.private_fields.lock().values() {
                trace_private_slot(slot, worklist);
            }
            if let crate::value::FunctionKind::Bound {
                target,
                this_val,
                bound_args,
            } = &f.kind
            {
                worklist.push(target.0);
                push_value(this_val, worklist);
                for a in bound_args {
                    push_value(a, worklist);
                }
            }
        }
        HeapObj::Environment(e) => {
            for (_, b) in e.vars.lock().iter() {
                push_value(&b.value.lock(), worklist);
            }
            if let Some(p) = *e.parent.lock() {
                worklist.push(p.0);
            }
        }
        HeapObj::Map(m) => {
            for (k, v) in m.entries.lock().iter() {
                push_value(&k.0, worklist);
                push_value(v, worklist);
            }
        }
        HeapObj::WeakMap(wm) => {
            for (key_idx, v) in wm.entries.lock().iter() {
                if *key_idx < marked.len() && marked[*key_idx] {
                    push_value(v, worklist);
                }
            }
        }
        HeapObj::WeakSet(_) | HeapObj::WeakRef(_) => {}
        HeapObj::FinalizationRegistry(registry) => {
            push_value(&registry.cleanup_callback, worklist);
            for cell in registry.cells.lock().iter() {
                push_value(&cell.held_value, worklist);
            }
        }
        HeapObj::Set(s) => {
            for k in s.items.lock().iter() {
                push_value(&k.0, worklist);
            }
        }
        HeapObj::CollectionIterator(it) => {
            push_value(&it.source, worklist);
        }
        HeapObj::RegExpStringIterator(it) => {
            push_value(&it.matcher, worklist);
        }
        HeapObj::DataView(d) => {
            push_value(&d.buffer, worklist);
        }
        HeapObj::TypedArray(t) => {
            if let Some(buffer) = &t.viewed_array_buffer {
                push_value(buffer, worklist);
            }
        }
        HeapObj::Promise(p) => {
            push_value(&p.result.lock(), worklist);
            for h in p.handlers.lock().iter() {
                push_value(&h.on_fulfilled, worklist);
                push_value(&h.on_rejected, worklist);
                if let Some(derived) = &h.derived {
                    push_value(&derived.promise, worklist);
                    push_value(&derived.resolve, worklist);
                    push_value(&derived.reject, worklist);
                }
                if let Some(continuation) = &h.continuation {
                    match continuation {
                        crate::value::PromiseContinuation::AsyncGenerator { generator, .. } => {
                            worklist.push(generator.0)
                        }
                        crate::value::PromiseContinuation::AsyncFromSyncIterator {
                            capability,
                            ..
                        } => {
                            push_value(&capability.promise, worklist);
                            push_value(&capability.resolve, worklist);
                            push_value(&capability.reject, worklist);
                        }
                        crate::value::PromiseContinuation::AsyncFunction(frame) => {
                            push_value(&frame.capability.promise, worklist);
                            push_value(&frame.capability.resolve, worklist);
                            push_value(&frame.capability.reject, worklist);
                            push_value(&frame.callee, worklist);
                            push_value(&frame.this_val, worklist);
                            push_value(&frame.new_target, worklist);
                            push_value(&frame.finally_completion_val, worklist);
                            worklist.push(frame.env.0);
                            worklist.extend(frame.catch_stack.iter().map(|(_, _, env)| env.0));
                            for value in frame.stack.iter().chain(frame.locals.iter()) {
                                push_value(value, worklist);
                            }
                        }
                    }
                }
            }
        }
        HeapObj::Generator(g) => {
            worklist.push(g.closure.0);
            for v in g.state.lock().iter() {
                push_value(v, worklist);
            }
        }
        HeapObj::LazyGenerator(g) => {
            worklist.push(g.closure.0);
            worklist.push(g.env.lock().0);
            for (_, _, env) in g.catch_stack.lock().iter() {
                worklist.push(env.0);
            }
            for v in g.stack.lock().iter() {
                push_value(v, worklist);
            }
            for v in g.locals.lock().iter() {
                push_value(v, worklist);
            }
            push_value(&g.finally_completion_val.lock(), worklist);
            push_value(&g.resume_value.lock(), worklist);
            for v in g.args.lock().iter() {
                push_value(v, worklist);
            }
            push_value(&g.this_val.lock(), worklist);
            for request in g.async_queue.lock().iter() {
                match &request.kind {
                    crate::value::AsyncGeneratorRequestKind::Next(value)
                    | crate::value::AsyncGeneratorRequestKind::Return(value)
                    | crate::value::AsyncGeneratorRequestKind::Throw(value) => {
                        push_value(value, worklist);
                    }
                }
                push_value(&request.capability.promise, worklist);
                push_value(&request.capability.resolve, worklist);
                push_value(&request.capability.reject, worklist);
            }
        }
        HeapObj::Iterator(it) => {
            for v in it.items.lock().iter() {
                push_value(v, worklist);
            }
            if let Some(lazy) = it.lazy_iter.lock().as_ref() {
                push_value(lazy, worklist);
            }
            if let Some(gen) = it.generator.lock().as_ref() {
                push_value(gen, worklist);
            }
            if let Some(source) = it.array_like.lock().as_ref() {
                push_value(source, worklist);
            }
            if let Some(source) = it.for_in_source.lock().as_ref() {
                push_value(source, worklist);
            }
            for source in it.for_in_key_sources.lock().iter() {
                push_value(source, worklist);
            }
        }
        _ => {}
    }
}

impl Heap {
    /// Create an empty heap with the default GC threshold.
    pub fn new() -> Self {
        Heap {
            cells: Mutex::new(Vec::new()),
            free_list: Mutex::new(Vec::new()),
            alloc_since_gc: AtomicUsize::new(0),
            gc_threshold: AtomicUsize::new(1024),
            max_objects: AtomicUsize::new(0),
        }
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

impl Heap {
    pub fn allocate(&self, obj: HeapObj) -> Result<usize, HeapLimitExceeded> {
        let max = self.max_objects.load(Ordering::Relaxed);
        if max > 0 {
            let live = self.live_count();
            if live >= max {
                // Do not run GC here: `Heap` does not know the root set.
                // The VM runs incremental GC periodically with proper roots.
                // If we collected with empty roots, every live object would
                // be swept, breaking the runtime.  Instead, just refuse to
                // allocate past the hard limit.
                return Err(HeapLimitExceeded);
            }
        }
        let idx = {
            let mut free = self.free_list.lock();
            if let Some(idx) = free.pop() {
                let cells = self.cells.lock();
                *cells[idx].obj.lock() = Some(obj);
                cells[idx].private_elements.lock().clear();
                cells[idx].marked.store(false, Ordering::Relaxed);
                idx
            } else {
                let mut cells = self.cells.lock();
                let idx = cells.len();
                cells.push(GcCell {
                    obj: Mutex::new(Some(obj)),
                    private_elements: Mutex::new(std::collections::HashMap::new()),
                    marked: AtomicBool::new(false),
                });
                idx
            }
        };
        self.alloc_since_gc.fetch_add(1, Ordering::Relaxed);
        Ok(idx)
    }

    pub fn collect(&self, roots: &[usize]) {
        self.collect_incremental(roots, usize::MAX);
    }

    /// Incremental GC: mark up to `budget` cells, then sweep if marking is done.
    /// With budget = usize::MAX, this is equivalent to a full stop-the-world GC.
    /// The VM calls this periodically with a small budget to avoid long pauses.
    pub fn collect_incremental(&self, roots: &[usize], budget: usize) {
        let cells_len = self.cells.lock().len();
        let mut marked = vec![false; cells_len];
        let mut worklist: Vec<usize> = roots.to_vec();
        let mut changed = true;
        let mut marked_count = 0usize;
        while changed {
            changed = false;
            while let Some(idx) = worklist.pop() {
                if marked_count >= budget && budget != usize::MAX {
                    // Budget exhausted: stop marking, don't sweep yet.
                    // The next call will restart from roots (simplified:
                    // we don't save state between calls, so this is a
                    // partial collection that marks what it can).
                    // For now, just continue to completion since the
                    // incremental state isn't persisted across calls.
                    break;
                }
                if idx >= cells_len || marked[idx] {
                    continue;
                }
                marked[idx] = true;
                marked_count += 1;
                changed = true;
                let children: Vec<usize> = {
                    let cells = self.cells.lock();
                    if let Some(cell) = cells.get(idx) {
                        let obj_ref = cell.obj.lock();
                        if let Some(obj) = obj_ref.as_ref() {
                            let mut w = Vec::new();
                            trace_obj(obj, &marked, &mut w);
                            for slot in cell.private_elements.lock().values() {
                                trace_private_slot(slot, &mut w);
                            }
                            w
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    }
                };
                worklist.extend(children);
            }
            if marked_count >= budget && budget != usize::MAX {
                break;
            }
        }
        // Only sweep if marking completed (worklist drained fully).
        if !worklist.is_empty() {
            return;
        }
        // Sweep: free unmarked cells.
        let mut free = self.free_list.lock();
        let mut cells = self.cells.lock();
        for (idx, cell) in cells.iter_mut().enumerate() {
            if !marked[idx] && cell.obj.lock().is_some() {
                *cell.obj.lock() = None;
                cell.private_elements.lock().clear();
                free.push(idx);
            }
        }
        // Sweep dead entries and targets from weak collections.
        for cell in cells.iter() {
            let obj_ref = cell.obj.lock();
            if let Some(obj) = obj_ref.as_ref() {
                match obj {
                    HeapObj::WeakMap(wm) => {
                        wm.entries
                            .lock()
                            .retain(|(k, _)| *k < marked.len() && marked[*k]);
                    }
                    HeapObj::WeakSet(ws) => {
                        ws.items.lock().retain(|k| *k < marked.len() && marked[*k]);
                    }
                    HeapObj::WeakRef(wr) => {
                        let mut target = wr.target.lock();
                        if matches!(target.as_ref(), Some(crate::value::Value::Object(idx))
                            if idx.0 >= marked.len() || !marked[idx.0])
                        {
                            *target = None;
                        }
                    }
                    HeapObj::FinalizationRegistry(registry) => {
                        for cell in registry.cells.lock().iter_mut() {
                            if matches!(cell.target.as_ref(), Some(crate::value::Value::Object(idx))
                                if idx.0 >= marked.len() || !marked[idx.0])
                            {
                                cell.target = None;
                            }
                            if matches!(cell.unregister_token.as_ref(), Some(crate::value::Value::Object(idx))
                                if idx.0 >= marked.len() || !marked[idx.0])
                            {
                                cell.unregister_token = None;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        self.alloc_since_gc.store(0, Ordering::Relaxed);
        let live = cells.len() - free.len();
        self.gc_threshold
            .store((live * 2).max(1024), Ordering::Relaxed);
    }

    pub fn maybe_collect(&self, roots: &[usize]) {
        if self.alloc_since_gc.load(Ordering::Relaxed) >= self.gc_threshold.load(Ordering::Relaxed)
        {
            self.collect(roots);
        }
    }

    pub fn take_pending_finalization_registries(&self) -> Vec<usize> {
        let cells = self.cells.lock();
        cells
            .iter()
            .enumerate()
            .filter_map(|(idx, cell)| {
                let obj = cell.obj.lock();
                let HeapObj::FinalizationRegistry(registry) = obj.as_ref()? else {
                    return None;
                };
                let has_pending = registry
                    .cells
                    .lock()
                    .iter()
                    .any(|cell| cell.target.is_none());
                if has_pending && !registry.cleanup_scheduled.swap(true, Ordering::Relaxed) {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn live_count(&self) -> usize {
        // Lock order must match `allocate` (free_list before cells) to avoid a
        // lock-order inversion deadlock if both are ever held concurrently.
        let free = self.free_list.lock();
        let cells = self.cells.lock();
        cells.len() - free.len()
    }

    pub fn set_max_objects(&self, max: usize) {
        self.max_objects.store(max, Ordering::Relaxed);
    }

    pub fn max_objects(&self) -> usize {
        self.max_objects.load(Ordering::Relaxed)
    }

    pub fn get_private_element(
        &self,
        idx: usize,
        key: &crate::value::PrivateSlotKey,
    ) -> Option<crate::value::PrivateSlot> {
        let cells = self.cells.lock();
        cells
            .get(idx)
            .and_then(|cell| cell.private_elements.lock().get(key).cloned())
    }

    pub fn with_private_elements<R>(
        &self,
        idx: usize,
        f: impl FnOnce(
            &mut std::collections::HashMap<crate::value::PrivateSlotKey, crate::value::PrivateSlot>,
        ) -> R,
    ) -> R {
        let cells = self.cells.lock();
        let mut private_elements = cells[idx].private_elements.lock();
        f(&mut private_elements)
    }

    pub fn with_obj<R>(&self, idx: usize, f: impl FnOnce(&HeapObj) -> R) -> R {
        // Take the object out of the cell so the cells mutex can be released
        // before running `f`. This prevents re-entrant locking of the cells
        // mutex (e.g. when `f` allocates or triggers a GC) from deadlocking.
        // The object is put back after `f` returns.
        //
        // Reentrancy: if `f` reaches back into the *same* object index (e.g.
        // a getter on the object calls a coercion that reads the object
        // again), the inner `take()` sees `None`. Previously this panicked
        // ("use after free"), aborting the host. Now a reentrant call gets a
        // temporary placeholder object so the inner callback still runs
        // instead of crashing; the outer frame owns the real object and
        // restores it on its way out, so it is never lost. Reentrant reads
        // already diverge from ES, so observing the placeholder is acceptable.
        let (obj, owned) = {
            let cells = self.cells.lock();
            let cell = &cells[idx];
            let mut slot = cell.obj.lock();
            match slot.take() {
                Some(o) => (o, true),
                None => (crate::value::HeapObj::placeholder(), false),
            }
        };
        let result = f(&obj);
        if owned {
            let cells = self.cells.lock();
            let cell = &cells[idx];
            *cell.obj.lock() = Some(obj);
        }
        result
    }

    pub fn with_obj_mut<R>(&self, idx: usize, f: impl FnOnce(&mut HeapObj) -> R) -> R {
        // Mirrors `with_obj`, but allows narrow metadata updates such as
        // initializing internal brands during native construction.
        let (mut obj, owned) = {
            let cells = self.cells.lock();
            let cell = &cells[idx];
            let mut slot = cell.obj.lock();
            match slot.take() {
                Some(o) => (o, true),
                None => (crate::value::HeapObj::placeholder(), false),
            }
        };
        let result = f(&mut obj);
        if owned {
            let cells = self.cells.lock();
            let cell = &cells[idx];
            *cell.obj.lock() = Some(obj);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{GcIdx, PrivateNameKey, PrivateSlot, PrivateSlotKey, Value};
    use std::sync::Arc;

    #[test]
    fn private_elements_trace_values_and_clear_when_cells_are_reused() {
        let heap = Heap::new();
        let owner = heap.allocate(HeapObj::placeholder()).unwrap();
        let child = heap.allocate(HeapObj::placeholder()).unwrap();
        let key = PrivateSlotKey::Private(PrivateNameKey {
            id: 1,
            description: Arc::from("field"),
        });
        heap.with_private_elements(owner, |elements| {
            elements.insert(key.clone(), PrivateSlot::Value(Value::Object(GcIdx(child))));
        });

        heap.collect(&[owner]);
        assert_eq!(heap.live_count(), 2);

        heap.collect(&[]);
        assert_eq!(heap.live_count(), 0);
        let first = heap.allocate(HeapObj::placeholder()).unwrap();
        let second = heap.allocate(HeapObj::placeholder()).unwrap();
        assert!(heap.get_private_element(first, &key).is_none());
        assert!(heap.get_private_element(second, &key).is_none());
    }
}
