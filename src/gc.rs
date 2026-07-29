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

pub(crate) const HEAP_LIMIT_MESSAGE: &str = "heap limit exceeded";

#[derive(Debug, Clone, Copy)]
pub struct HeapLimitExceeded;

impl std::convert::From<HeapLimitExceeded> for std::sync::Arc<crate::error::Error> {
    fn from(_: HeapLimitExceeded) -> Self {
        std::sync::Arc::new(crate::error::Error {
            kind: crate::error::ErrorKind::Range,
            message: HEAP_LIMIT_MESSAGE.into(),
            stack: Vec::new(),
            thrown_value: None,
            line: None,
            text_is_host_unicode: false,
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
    incremental_mark: Mutex<Option<IncrementalMark>>,
    alloc_since_gc: AtomicUsize,
    gc_threshold: AtomicUsize,
    /// Maximum number of live heap objects allowed. When this is exceeded,
    /// `allocate` returns `HeapLimitExceeded`. `0` means unlimited.
    max_objects: AtomicUsize,
}

struct IncrementalMark {
    marked: Vec<bool>,
    queued: Vec<bool>,
    worklist: Vec<usize>,
    pending_ephemerons: std::collections::HashMap<usize, Vec<crate::value::Value>>,
}

impl IncrementalMark {
    fn queue_index(&mut self, index: usize) {
        if index < self.marked.len() && !self.marked[index] && !self.queued[index] {
            self.queued[index] = true;
            self.worklist.push(index);
        }
    }

    fn queue_value(&mut self, value: &crate::value::Value) {
        let mut roots = Vec::new();
        push_value(value, &mut roots);
        for root in roots {
            self.queue_index(root);
        }
    }
}

fn push_value(value: &crate::value::Value, worklist: &mut Vec<usize>) {
    value.visit_gc_roots(&mut |root| worklist.push(root));
}

fn trace_private_slot(slot: &crate::value::PrivateSlot, worklist: &mut Vec<usize>) {
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

fn trace_cell(
    cells: &[GcCell],
    index: usize,
) -> (
    Vec<usize>,
    Vec<(crate::value::WeakKey, crate::value::Value)>,
) {
    let Some(cell) = cells.get(index) else {
        return (Vec::new(), Vec::new());
    };
    let obj_ref = cell.obj.lock();
    let Some(obj) = obj_ref.as_ref() else {
        return (Vec::new(), Vec::new());
    };
    let mut children = Vec::new();
    trace_obj(obj, &mut children);
    for slot in cell.private_elements.lock().values() {
        trace_private_slot(slot, &mut children);
    }
    let ephemerons = match obj {
        HeapObj::WeakMap(map) => map
            .entries
            .lock()
            .iter()
            .map(|(key, value)| (*key, value.clone()))
            .collect(),
        _ => Vec::new(),
    };
    (children, ephemerons)
}

fn queue_ephemerons(
    state: &mut IncrementalMark,
    ephemerons: Vec<(crate::value::WeakKey, crate::value::Value)>,
) {
    for (key, value) in ephemerons {
        match key {
            crate::value::WeakKey::Object(key_index)
                if key_index < state.marked.len() && state.marked[key_index] =>
            {
                state.queue_value(&value);
            }
            crate::value::WeakKey::Object(key_index) => state
                .pending_ephemerons
                .entry(key_index)
                .or_default()
                .push(value),
            // Symbols are stable VM identities until the Symbol table itself
            // becomes collectible.
            crate::value::WeakKey::Symbol(_) => {
                state.queue_value(&value);
            }
        }
    }
}

/// Push reachable child indices of `obj` onto `worklist`. Called while NOT
/// holding the cells mutex, so it may lock any object field freely. WeakMap
/// ephemeron values are deliberately deferred to the collector's fixed-point
/// phase after this ordinary strong-edge trace.
pub fn trace_obj(obj: &HeapObj, worklist: &mut Vec<usize>) {
    if let HeapObj::Iterator(it) = obj {
        for v in it.items.lock().iter() {
            push_value(v, worklist);
        }
        if let Some(lazy) = it.lazy_iter.lock().as_ref() {
            push_value(lazy, worklist);
        }
        if let Some(next) = it.lazy_next.lock().as_ref() {
            push_value(next, worklist);
        }
        if let Some(gen) = it.generator.lock().as_ref() {
            push_value(gen, worklist);
        }
        if let Some(state) = it.for_in.lock().as_ref() {
            if let Some(object) = state.object.as_ref() {
                push_value(object, worklist);
            }
            for root in &state.traversal_roots {
                push_value(root, worklist);
            }
        }
        return;
    }
    if let HeapObj::Environment(e) = obj {
        for (_, b) in e.vars.lock().iter() {
            push_value(&b.value.lock(), worklist);
            if let Some((target, _)) = &b.indirect {
                worklist.push(target.0);
            }
        }
        if let Some(with_object) = e.with_object.lock().as_ref() {
            push_value(with_object, worklist);
        }
        if let Some(p) = *e.parent.lock() {
            worklist.push(p.0);
        }
        return;
    }
    if let HeapObj::ModuleNamespace(namespace) = obj {
        for (_, (env, _)) in namespace.exports.lock().iter() {
            worklist.push(env.0);
        }
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
            if let Some(home_object) = f.home_object.lock().as_ref() {
                push_value(home_object, worklist);
            }
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
                ..
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
                if let Some((target, _)) = &b.indirect {
                    worklist.push(target.0);
                }
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
        // WeakMap values are activated in a separate ephemeron fixed point
        // after ordinary strong marking. Visiting them here would make
        // liveness depend on root/worklist order.
        HeapObj::WeakMap(_) => {}
        HeapObj::WeakSet(_) | HeapObj::WeakRef(_) => {}
        HeapObj::FinalizationRegistry(registry) => {
            push_value(&registry.cleanup_callback, worklist);
            for cell in registry.cells.lock().iter() {
                push_value(&cell.held_value, worklist);
            }
        }
        HeapObj::Set(s) => {
            for key in s.items.lock().iter() {
                push_value(&key.0, worklist);
            }
        }
        HeapObj::CollectionIterator(it) => {
            push_value(&it.source.lock(), worklist);
            if let Some(next) = it.next_method.lock().as_ref() {
                push_value(next, worklist);
            }
        }
        HeapObj::IteratorHelper(it) => {
            worklist.push(it.resume_realm.0);
            push_value(&it.iterator, worklist);
            push_value(&it.next_method, worklist);
            if let Some(callback) = &it.callback {
                push_value(callback, worklist);
            }
            if let Some(inner) = it.inner_iterator.lock().as_ref() {
                push_value(&inner.iterator, worklist);
                push_value(&inner.next_method, worklist);
            }
            for record in &it.concat_iterables {
                push_value(&record.iterable, worklist);
                push_value(&record.open_method, worklist);
            }
            for record in it.zip_iterators.lock().iter().flatten() {
                push_value(&record.iterator, worklist);
                push_value(&record.next_method, worklist);
            }
            for value in &it.zip_padding {
                push_value(value, worklist);
            }
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
        HeapObj::Proxy(proxy) => {
            push_value(&proxy.target, worklist);
            push_value(&proxy.handler, worklist);
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
                        crate::value::PromiseContinuation::DynamicImport {
                            capability,
                            realm,
                            ..
                        } => {
                            push_value(&capability.promise, worklist);
                            push_value(&capability.resolve, worklist);
                            push_value(&capability.reject, worklist);
                            worklist.push(realm.0);
                        }
                        crate::value::PromiseContinuation::AsyncGenerator { generator, .. } => {
                            worklist.push(generator.0)
                        }
                        crate::value::PromiseContinuation::AsyncFromSyncIterator {
                            capability,
                            iterator,
                            realm,
                            ..
                        } => {
                            push_value(&capability.promise, worklist);
                            push_value(&capability.resolve, worklist);
                            push_value(&capability.reject, worklist);
                            if let Some(iterator) = iterator {
                                push_value(iterator, worklist);
                            }
                            worklist.push(realm.0);
                        }
                        crate::value::PromiseContinuation::ArrayFromAsync(frame) => {
                            push_value(&frame.capability.promise, worklist);
                            push_value(&frame.capability.resolve, worklist);
                            push_value(&frame.capability.reject, worklist);
                            worklist.push(frame.realm.0);
                            for value in [
                                &frame.target,
                                &frame.source,
                                &frame.iterator,
                                &frame.next_method,
                                &frame.mapper,
                                &frame.this_arg,
                            ] {
                                push_value(value, worklist);
                            }
                            if let crate::value::ArrayFromAsyncAwaitKind::IteratorClose {
                                original_reason,
                            } = &frame.await_kind
                            {
                                push_value(original_reason, worklist);
                            }
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
                            worklist.extend(frame.catch_stack.iter().map(|(_, _, env, _)| env.0));
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
            for (_, _, env, _) in g.catch_stack.lock().iter() {
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
            if let Some(next) = it.lazy_next.lock().as_ref() {
                push_value(next, worklist);
            }
            if let Some(gen) = it.generator.lock().as_ref() {
                push_value(gen, worklist);
            }
            if let Some(state) = it.for_in.lock().as_ref() {
                if let Some(object) = state.object.as_ref() {
                    push_value(object, worklist);
                }
                for root in &state.traversal_roots {
                    push_value(root, worklist);
                }
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
            incremental_mark: Mutex::new(None),
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
        let mut incremental_mark = self.incremental_mark.lock();
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
        if let Some(state) = incremental_mark.as_mut() {
            if idx >= state.marked.len() {
                state.marked.resize(idx + 1, false);
                state.queued.resize(idx + 1, false);
            }
            state.queue_index(idx);
        }
        self.alloc_since_gc.fetch_add(1, Ordering::Relaxed);
        Ok(idx)
    }

    pub fn collect(&self, roots: &[usize]) {
        self.collect_incremental(roots, usize::MAX);
    }

    /// Incremental GC: mark up to `budget` cells, then sweep if marking is done.
    /// With budget = usize::MAX, this is equivalent to a full stop-the-world GC.
    /// Finite-budget cycles snapshot roots on their first slice. New allocations
    /// are queued by `allocate`, while the final retrace catches edges added to
    /// already-marked objects between slices.
    pub fn collect_incremental(&self, roots: &[usize], budget: usize) {
        let mut state_slot = self.incremental_mark.lock();
        let cells_len = self.cells.lock().len();
        let (mut state, new_cycle) = match state_slot.take() {
            Some(state) => (state, false),
            None => (
                IncrementalMark {
                    marked: vec![false; cells_len],
                    queued: vec![false; cells_len],
                    worklist: Vec::new(),
                    pending_ephemerons: std::collections::HashMap::new(),
                },
                true,
            ),
        };
        state.marked.resize(cells_len, false);
        state.queued.resize(cells_len, false);
        if new_cycle || budget == usize::MAX {
            for root in roots {
                state.queue_index(*root);
            }
        }

        let mut marked_this_slice = 0usize;
        let mut finalizing = false;
        let mut rescanned = false;
        loop {
            while let Some(idx) = state.worklist.pop() {
                if idx < state.queued.len() {
                    state.queued[idx] = false;
                }
                if idx >= cells_len || state.marked[idx] {
                    continue;
                }
                if !finalizing && marked_this_slice >= budget && budget != usize::MAX {
                    state.queue_index(idx);
                    *state_slot = Some(state);
                    return;
                }
                state.marked[idx] = true;
                marked_this_slice += 1;

                let (children, ephemerons) = {
                    let cells = self.cells.lock();
                    trace_cell(&cells, idx)
                };
                for child in children {
                    state.queue_index(child);
                }
                queue_ephemerons(&mut state, ephemerons);
                if let Some(values) = state.pending_ephemerons.remove(&idx) {
                    for value in values {
                        state.queue_value(&value);
                    }
                }
            }
            if !finalizing {
                // Finish against the current root set in the same stop-the-
                // world phase as sweep. This catches old white objects that
                // became host roots between finite-budget slices without
                // rescanning all roots on every slice.
                finalizing = true;
                for root in roots {
                    state.queue_index(*root);
                }
                continue;
            }
            if rescanned {
                break;
            }

            // A mutator can change an already-marked object between slices.
            // Rebuild ephemeron dependencies and retrace marked cells once
            // immediately before sweep, which is the incremental write-barrier
            // equivalent for this single-threaded VM.
            rescanned = true;
            state.pending_ephemerons.clear();
            let cells = self.cells.lock();
            for index in 0..cells_len {
                if !state.marked[index] {
                    continue;
                }
                let (children, ephemerons) = trace_cell(&cells, index);
                for child in children {
                    state.queue_index(child);
                }
                queue_ephemerons(&mut state, ephemerons);
            }
        }

        let marked = state.marked;
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
                        wm.entries.lock().retain(|key, _| match key {
                            crate::value::WeakKey::Object(index) => {
                                *index < marked.len() && marked[*index]
                            }
                            crate::value::WeakKey::Symbol(_) => true,
                        });
                    }
                    HeapObj::WeakSet(ws) => {
                        ws.items.lock().retain(|key| match key {
                            crate::value::WeakKey::Object(index) => {
                                *index < marked.len() && marked[*index]
                            }
                            crate::value::WeakKey::Symbol(_) => true,
                        });
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

    pub fn maybe_collect(&self, roots: &[usize]) -> bool {
        if self.alloc_since_gc.load(Ordering::Relaxed) >= self.gc_threshold.load(Ordering::Relaxed)
        {
            self.collect(roots);
            true
        } else {
            false
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
    use crate::value::{
        GcIdx, PrivateNameKey, PrivateSlot, PrivateSlotKey, PropertyDescriptor, PropertyKey, Value,
        WeakKey, WeakMapData,
    };
    use std::sync::Arc;

    fn weak_map(
        heap: &Heap,
        entries: impl IntoIterator<Item = (WeakKey, Value)>,
        strong_properties: impl IntoIterator<Item = (PropertyKey, PropertyDescriptor)>,
    ) -> usize {
        heap.allocate(HeapObj::WeakMap(WeakMapData {
            entries: Mutex::new(entries.into_iter().collect()),
            props: Mutex::new(strong_properties.into_iter().collect()),
            proto: Mutex::new(None),
            extensible: AtomicBool::new(true),
        }))
        .unwrap()
    }

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

    #[test]
    fn weak_map_live_values_are_root_order_independent() {
        for map_first in [false, true] {
            let heap = Heap::new();
            let key = heap.allocate(HeapObj::placeholder()).unwrap();
            let value = heap.allocate(HeapObj::placeholder()).unwrap();
            let map = weak_map(
                &heap,
                [(WeakKey::Object(key), Value::Object(GcIdx(value)))],
                [],
            );
            let roots = if map_first {
                vec![key, map]
            } else {
                vec![map, key]
            };
            heap.collect(&roots);
            assert_eq!(heap.live_count(), 3, "map_first={map_first}");
            let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
            assert_ne!(fresh, value, "live ephemeron value must not be reused");
        }
    }

    #[test]
    fn weak_map_ephemeron_chains_reach_a_fixed_point() {
        let heap = Heap::new();
        let first_key = heap.allocate(HeapObj::placeholder()).unwrap();
        let second_key = heap.allocate(HeapObj::placeholder()).unwrap();
        let value = heap.allocate(HeapObj::placeholder()).unwrap();
        let first_map = weak_map(
            &heap,
            [(WeakKey::Object(first_key), Value::Object(GcIdx(second_key)))],
            [],
        );
        let second_map = weak_map(
            &heap,
            [(WeakKey::Object(second_key), Value::Object(GcIdx(value)))],
            [],
        );

        heap.collect(&[first_key, first_map, second_map]);
        assert_eq!(heap.live_count(), 5);
        let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_ne!(fresh, value, "transitive ephemeron value must remain live");
    }

    #[test]
    fn incremental_marking_resumes_through_ephemeron_chains() {
        let heap = Heap::new();
        let first_key = heap.allocate(HeapObj::placeholder()).unwrap();
        let second_key = heap.allocate(HeapObj::placeholder()).unwrap();
        let value = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
        let first_map = weak_map(
            &heap,
            [(WeakKey::Object(first_key), Value::Object(GcIdx(second_key)))],
            [],
        );
        let second_map = weak_map(
            &heap,
            [(WeakKey::Object(second_key), Value::Object(GcIdx(value)))],
            [],
        );
        let roots = [first_key, first_map, second_map];

        for _ in 0..4 {
            heap.collect_incremental(&roots, 1);
            assert_eq!(heap.live_count(), 6, "partial marks must not sweep");
        }
        heap.collect_incremental(&roots, 1);
        assert_eq!(heap.live_count(), 5);
        let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(fresh, garbage, "completed incremental mark must sweep");
    }

    #[test]
    fn incremental_marking_retraces_mutated_marked_objects_before_sweep() {
        let heap = Heap::new();
        let owner = heap.allocate(HeapObj::placeholder()).unwrap();
        let first_child = heap.allocate(HeapObj::placeholder()).unwrap();
        let late_child = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
        heap.with_obj(owner, |object| {
            object.props().lock().insert(
                PropertyKey::from("first"),
                PropertyDescriptor::data(Value::Object(GcIdx(first_child))),
            );
        });

        heap.collect_incremental(&[owner], 1);
        heap.with_obj(owner, |object| {
            object.props().lock().insert(
                PropertyKey::from("late"),
                PropertyDescriptor::data(Value::Object(GcIdx(late_child))),
            );
        });
        heap.collect_incremental(&[owner], 1);
        heap.collect_incremental(&[owner], 1);

        assert_eq!(heap.live_count(), 3);
        let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(fresh, garbage);
    }

    #[test]
    fn incremental_marking_deduplicates_roots_already_in_the_worklist() {
        let heap = Heap::new();
        let roots: Vec<_> = (0..100)
            .map(|_| heap.allocate(HeapObj::placeholder()).unwrap())
            .collect();

        heap.collect_incremental(&roots, 1);
        assert_eq!(
            heap.incremental_mark
                .lock()
                .as_ref()
                .unwrap()
                .worklist
                .len(),
            99
        );
        heap.collect_incremental(&roots, 1);
        assert_eq!(
            heap.incremental_mark
                .lock()
                .as_ref()
                .unwrap()
                .worklist
                .len(),
            98
        );
    }

    #[test]
    fn incremental_marking_queues_allocations_created_between_slices() {
        let heap = Heap::new();
        let owner = heap.allocate(HeapObj::placeholder()).unwrap();
        let child = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
        heap.with_obj(owner, |object| {
            object.props().lock().insert(
                PropertyKey::from("child"),
                PropertyDescriptor::data(Value::Object(GcIdx(child))),
            );
        });

        heap.collect_incremental(&[owner], 1);
        let late = heap.allocate(HeapObj::placeholder()).unwrap();
        heap.collect_incremental(&[owner], 1);
        heap.collect_incremental(&[owner], 1);

        assert_eq!(heap.live_count(), 3);
        let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(fresh, garbage);
        assert_ne!(fresh, late);
    }

    #[test]
    fn incremental_allocation_barrier_preserves_marks_when_reusing_a_low_cell() {
        let heap = Heap::new();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
        let owner = heap.allocate(HeapObj::placeholder()).unwrap();
        let child = heap.allocate(HeapObj::placeholder()).unwrap();
        heap.with_obj(owner, |object| {
            object.props().lock().insert(
                PropertyKey::from("child"),
                PropertyDescriptor::data(Value::Object(GcIdx(child))),
            );
        });
        heap.collect(&[owner]);
        assert_eq!(heap.live_count(), 2);

        heap.collect_incremental(&[owner], 1);
        let late = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(late, garbage);
        heap.collect_incremental(&[owner], 1);

        assert_eq!(heap.live_count(), 3);
        assert!(heap.with_obj(child, |object| matches!(object, HeapObj::Object(_))));
    }

    #[test]
    fn incremental_completion_remarks_roots_added_between_slices() {
        let heap = Heap::new();
        let owner = heap.allocate(HeapObj::placeholder()).unwrap();
        let child = heap.allocate(HeapObj::placeholder()).unwrap();
        let late_root = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
        heap.with_obj(owner, |object| {
            object.props().lock().insert(
                PropertyKey::from("child"),
                PropertyDescriptor::data(Value::Object(GcIdx(child))),
            );
        });

        heap.collect_incremental(&[owner], 1);
        heap.collect_incremental(&[owner, late_root], 1);

        assert_eq!(heap.live_count(), 3);
        assert!(heap.with_obj(late_root, |object| matches!(object, HeapObj::Object(_))));
        let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(fresh, garbage);
    }

    #[test]
    fn strong_weak_map_property_can_activate_an_ephemeron_value() {
        let heap = Heap::new();
        let key = heap.allocate(HeapObj::placeholder()).unwrap();
        let value = heap.allocate(HeapObj::placeholder()).unwrap();
        let map = weak_map(
            &heap,
            [(WeakKey::Object(key), Value::Object(GcIdx(value)))],
            [(
                PropertyKey::from("key"),
                PropertyDescriptor::data(Value::Object(GcIdx(key))),
            )],
        );

        heap.collect(&[map]);
        assert_eq!(heap.live_count(), 3);
    }

    #[test]
    fn dead_ephemeron_cycle_does_not_retain_its_key_or_value() {
        let heap = Heap::new();
        let key = heap.allocate(HeapObj::placeholder()).unwrap();
        let value = heap.allocate(HeapObj::placeholder()).unwrap();
        let map = weak_map(
            &heap,
            [(WeakKey::Object(key), Value::Object(GcIdx(value)))],
            [],
        );

        heap.collect(&[map]);
        assert_eq!(heap.live_count(), 1);
        assert!(heap.with_obj(map, |object| {
            let HeapObj::WeakMap(map) = object else {
                return false;
            };
            map.entries.lock().is_empty()
        }));
    }
}
