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
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

struct GcCell {
    obj: Mutex<Option<Arc<HeapObj>>>,
    private_elements:
        Mutex<std::collections::HashMap<crate::value::PrivateSlotKey, crate::value::PrivateSlot>>,
    marked: AtomicBool,
    active_accesses: AtomicUsize,
}

pub struct Heap {
    cells: Mutex<Vec<GcCell>>,
    free_list: Mutex<Vec<usize>>,
    incremental_mark: Mutex<Option<IncrementalMark>>,
    alloc_since_gc: AtomicUsize,
    gc_threshold: AtomicUsize,
    /// Maximum number of live heap objects allowed. When this is exceeded,
    /// `allocate` returns `HeapLimitExceeded`. `0` means unlimited.
    max_objects: AtomicUsize,
}

struct ActiveObjectAccess<'a> {
    heap: &'a Heap,
    index: usize,
    dirty_on_drop: bool,
}

impl Drop for ActiveObjectAccess<'_> {
    fn drop(&mut self) {
        {
            let cells = self.heap.cells.lock();
            cells[self.index]
                .active_accesses
                .fetch_sub(1, Ordering::Relaxed);
        }
        if self.dirty_on_drop {
            self.heap.note_incremental_access(self.index);
        }
    }
}

struct IncrementalMark {
    marked: Vec<bool>,
    queued: Vec<bool>,
    worklist: Vec<TraceWork>,
    pending_ephemerons: std::collections::HashMap<usize, Vec<crate::value::Value>>,
    phase: IncrementalPhase,
    dirty: Vec<usize>,
    dirty_queued: Vec<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TraceWork {
    Cell(usize),
    VecEdges(VecTrace),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VecTrace {
    owner: usize,
    kind: VecTraceKind,
    next: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VecTraceKind {
    ArrayItems,
    IteratorItems,
    PromiseHandlers,
    FinalizationRegistryCells,
    MapEntries,
}

struct CellTrace {
    before: Vec<usize>,
    vector: Option<VecTrace>,
    after: Vec<usize>,
    ephemerons: Vec<(crate::value::WeakKey, crate::value::Value)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IncrementalPhase {
    Mark,
    Retrace { cursor: usize },
}

impl IncrementalMark {
    fn queue_index(&mut self, index: usize) {
        if index < self.marked.len() && !self.marked[index] && !self.queued[index] {
            self.queued[index] = true;
            self.worklist.push(TraceWork::Cell(index));
        }
    }

    fn queue_value(&mut self, value: &crate::value::Value) {
        let mut roots = Vec::new();
        push_value(value, &mut roots);
        for root in roots {
            self.queue_index(root);
        }
    }

    fn queue_dirty(&mut self, index: usize) {
        let IncrementalPhase::Retrace { cursor } = self.phase else {
            return;
        };
        if index < cursor
            && index < self.marked.len()
            && self.marked[index]
            && !self.dirty_queued[index]
        {
            self.dirty_queued[index] = true;
            self.dirty.push(index);
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

fn trace_realm_record(record: &crate::value::RealmRecord, worklist: &mut Vec<usize>) {
    for value in record.intrinsic_roots.lock().iter() {
        push_value(value, worklist);
    }
    for module in record.module_records.lock().values() {
        worklist.push(module.env.0);
        if let Some(meta) = module.import_meta() {
            worklist.push(meta.0);
        }
        if let Some(namespace) = module.namespace() {
            worklist.push(namespace.0);
        }
        if let Some(promise) = module.evaluation_promise() {
            worklist.push(promise.0);
        }
        if let Some(error) = module.error() {
            if let Some(value) = &error.thrown_value {
                push_value(value, worklist);
            }
        }
        if let Some(value) = module.completion_value() {
            push_value(&value, worklist);
        }
        if let Some(value) = module.synthetic_default() {
            push_value(&value, worklist);
        }
    }
    for entry in record.template_cache.lock().values() {
        push_value(&entry.value, worklist);
    }
}

fn trace_cell(cells: &[GcCell], index: usize, cursorize_vectors: bool) -> CellTrace {
    let Some(cell) = cells.get(index) else {
        return CellTrace {
            before: Vec::new(),
            vector: None,
            after: Vec::new(),
            ephemerons: Vec::new(),
        };
    };
    let obj_ref = cell.obj.lock();
    let Some(obj) = obj_ref.as_ref() else {
        return CellTrace {
            before: Vec::new(),
            vector: None,
            after: Vec::new(),
            ephemerons: Vec::new(),
        };
    };
    let mut before = Vec::new();
    if !cursorize_vectors {
        trace_obj_impl(obj.as_ref(), &mut before);
        for slot in cell.private_elements.lock().values() {
            trace_private_slot(slot, &mut before);
        }
        let ephemerons = match obj.as_ref() {
            HeapObj::WeakMap(map) => map
                .entries
                .lock()
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
            _ => Vec::new(),
        };
        return CellTrace {
            before,
            vector: None,
            after: Vec::new(),
            ephemerons,
        };
    }
    let mut after = Vec::new();
    let vector = match obj.as_ref() {
        HeapObj::Array(array) => {
            trace_properties_and_prototype(obj.as_ref(), &mut before);
            if let Some(map) = array.arguments_map.lock().as_ref() {
                after.push(map.env.0);
            }
            Some(VecTrace {
                owner: index,
                kind: VecTraceKind::ArrayItems,
                next: array.items.lock().len(),
            })
        }
        HeapObj::Iterator(iterator) => {
            trace_iterator_auxiliary_edges(iterator, &mut after);
            Some(VecTrace {
                owner: index,
                kind: VecTraceKind::IteratorItems,
                next: iterator.items.lock().len(),
            })
        }
        HeapObj::Promise(promise) => {
            trace_properties_and_prototype(obj.as_ref(), &mut before);
            push_value(&promise.result.lock(), &mut before);
            Some(VecTrace {
                owner: index,
                kind: VecTraceKind::PromiseHandlers,
                next: promise.handlers.lock().len(),
            })
        }
        HeapObj::FinalizationRegistry(registry) => {
            trace_properties_and_prototype(obj.as_ref(), &mut before);
            push_value(&registry.cleanup_callback, &mut before);
            before.push(registry.realm.0);
            Some(VecTrace {
                owner: index,
                kind: VecTraceKind::FinalizationRegistryCells,
                next: registry.cells.lock().len(),
            })
        }
        HeapObj::Map(map) => {
            trace_properties_and_prototype(obj.as_ref(), &mut before);
            Some(VecTrace {
                owner: index,
                kind: VecTraceKind::MapEntries,
                next: map.entries.lock().len(),
            })
        }
        _ => {
            trace_obj_impl(obj.as_ref(), &mut before);
            None
        }
    };
    let private_children = if vector.is_some() {
        &mut after
    } else {
        &mut before
    };
    for slot in cell.private_elements.lock().values() {
        trace_private_slot(slot, private_children);
    }
    let ephemerons = match obj.as_ref() {
        HeapObj::WeakMap(map) => map
            .entries
            .lock()
            .iter()
            .map(|(key, value)| (*key, value.clone()))
            .collect(),
        _ => Vec::new(),
    };
    CellTrace {
        before,
        vector: vector.filter(|trace| trace.next > 0),
        after,
        ephemerons,
    }
}

fn trace_vec_slots(
    cells: &Mutex<Vec<GcCell>>,
    trace: VecTrace,
    limit: usize,
) -> (Vec<usize>, usize) {
    let obj = {
        let cells = cells.lock();
        cells
            .get(trace.owner)
            .and_then(|cell| cell.obj.lock().clone())
    };
    let count = trace.next.min(limit);
    let next = trace.next - count;
    let Some(obj) = obj else {
        return (Vec::new(), next);
    };
    let mut roots = Vec::new();
    match (trace.kind, obj.as_ref()) {
        (VecTraceKind::ArrayItems, HeapObj::Array(array)) => {
            let items = array.items.lock();
            let end = trace.next.min(items.len());
            let start = next.min(end);
            for value in &items[start..end] {
                push_value(value, &mut roots);
            }
        }
        (VecTraceKind::IteratorItems, HeapObj::Iterator(iterator)) => {
            let items = iterator.items.lock();
            let end = trace.next.min(items.len());
            let start = next.min(end);
            for value in &items[start..end] {
                push_value(value, &mut roots);
            }
        }
        (VecTraceKind::PromiseHandlers, HeapObj::Promise(promise)) => {
            let handlers = promise.handlers.lock();
            let end = trace.next.min(handlers.len());
            let start = next.min(end);
            for handler in &handlers[start..end] {
                trace_promise_handler(handler, &mut roots);
            }
        }
        (VecTraceKind::FinalizationRegistryCells, HeapObj::FinalizationRegistry(registry)) => {
            let cells = registry.cells.lock();
            let end = trace.next.min(cells.len());
            let start = next.min(end);
            for cell in &cells[start..end] {
                push_value(&cell.held_value, &mut roots);
            }
        }
        (VecTraceKind::MapEntries, HeapObj::Map(map)) => {
            let entries = map.entries.lock();
            let end = trace.next.min(entries.len());
            let start = next.min(end);
            for index in start..end {
                if let Some((key, value)) = entries.get_index(index) {
                    push_value(&key.0, &mut roots);
                    push_value(value, &mut roots);
                }
            }
        }
        _ => {}
    }
    (roots, next)
}

fn schedule_trace(state: &mut IncrementalMark, trace: CellTrace) {
    for child in trace.before {
        state.queue_index(child);
    }
    if let Some(vector) = trace.vector {
        state.worklist.push(TraceWork::VecEdges(vector));
    }
    for child in trace.after {
        state.queue_index(child);
    }
    queue_ephemerons(state, trace.ephemerons);
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

/// Push reachable child indices of `obj` onto `worklist`. WeakMap ephemeron
/// values are deliberately deferred to the collector's fixed-point phase after
/// this ordinary strong-edge trace.
pub fn trace_obj(obj: &HeapObj, worklist: &mut Vec<usize>) {
    trace_obj_impl(obj, worklist);
}

fn trace_iterator_auxiliary_edges(
    iterator: &crate::value::IteratorData,
    worklist: &mut Vec<usize>,
) {
    if let Some(lazy) = iterator.lazy_iter.lock().as_ref() {
        push_value(lazy, worklist);
    }
    if let Some(next) = iterator.lazy_next.lock().as_ref() {
        push_value(next, worklist);
    }
    if let Some(generator) = iterator.generator.lock().as_ref() {
        push_value(generator, worklist);
    }
    if let Some(state) = iterator.for_in.lock().as_ref() {
        if let Some(object) = state.object.as_ref() {
            push_value(object, worklist);
        }
        for root in &state.traversal_roots {
            push_value(root, worklist);
        }
    }
}

fn trace_properties_and_prototype(obj: &HeapObj, worklist: &mut Vec<usize>) {
    let props = obj.props();
    for (_, desc) in props.lock().iter() {
        if !desc.is_accessor {
            push_value(&desc.value, worklist);
        } else {
            if let Some(getter) = &desc.get {
                push_value(getter, worklist);
            }
            if let Some(setter) = &desc.set {
                push_value(setter, worklist);
            }
        }
    }
    if let Some(proto) = obj.proto().lock().as_ref() {
        push_value(proto, worklist);
    }
}

#[inline(always)]
fn trace_promise_handler(handler: &crate::value::PromiseHandler, worklist: &mut Vec<usize>) {
    push_value(&handler.on_fulfilled, worklist);
    push_value(&handler.on_rejected, worklist);
    if let Some(derived) = &handler.derived {
        push_value(&derived.promise, worklist);
        push_value(&derived.resolve, worklist);
        push_value(&derived.reject, worklist);
    }
    if let Some(continuation) = &handler.continuation {
        match continuation {
            crate::value::PromiseContinuation::DynamicImport {
                capability, realm, ..
            } => {
                push_value(&capability.promise, worklist);
                push_value(&capability.resolve, worklist);
                push_value(&capability.reject, worklist);
                worklist.push(realm.0);
            }
            crate::value::PromiseContinuation::ShadowRealmImportValue {
                capability,
                caller_realm,
                ..
            } => {
                push_value(&capability.promise, worklist);
                push_value(&capability.resolve, worklist);
                push_value(&capability.reject, worklist);
                worklist.push(caller_realm.0);
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
                if let crate::value::ArrayFromAsyncAwaitKind::IteratorClose { original_reason } =
                    &frame.await_kind
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
                for completion in &frame.finally_completions {
                    push_value(&completion.value, worklist);
                }
                worklist.push(frame.env.0);
                worklist.extend(frame.catch_stack.iter().map(|(_, _, env, _)| env.0));
                worklist.extend(frame.finally_stack.iter().map(|guard| guard.env.0));
                for value in frame
                    .stack
                    .iter()
                    .chain(frame.locals.iter())
                    .chain(frame.compiler_temps.iter())
                {
                    push_value(value, worklist);
                }
            }
        }
    }
}

fn trace_obj_impl(obj: &HeapObj, worklist: &mut Vec<usize>) {
    if let HeapObj::Iterator(it) = obj {
        for value in it.items.lock().iter() {
            push_value(value, worklist);
        }
        trace_iterator_auxiliary_edges(it, worklist);
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
        if let Some(record) = e.realm_record.lock().as_ref() {
            trace_realm_record(record, worklist);
        }
        return;
    }
    if let HeapObj::ModuleNamespace(namespace) = obj {
        for (_, (env, _)) in namespace.exports.lock().iter() {
            worklist.push(env.0);
        }
    }
    trace_properties_and_prototype(obj, worklist);
    match obj {
        HeapObj::Object(o) => {
            for slot in o.private_fields.lock().values() {
                trace_private_slot(slot, worklist);
            }
        }
        HeapObj::Array(a) => {
            for value in a.items.lock().iter() {
                push_value(value, worklist);
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
            } else if let crate::value::FunctionKind::Wrapped { target } = &f.kind {
                push_value(target, worklist);
            }
        }
        HeapObj::IntlCollator(collator) => {
            if let Some(compare) = collator.bound_compare.lock().as_ref() {
                push_value(compare, worklist);
            }
        }
        HeapObj::Temporal(temporal) => match &temporal.kind {
            crate::value::TemporalKind::Instant { .. }
            | crate::value::TemporalKind::Duration { .. }
            | crate::value::TemporalKind::PlainDateTime { .. }
            | crate::value::TemporalKind::ZonedDateTime { .. } => {}
        },
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
            if let Some(record) = e.realm_record.lock().as_ref() {
                trace_realm_record(record, worklist);
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
            worklist.push(registry.realm.0);
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
            for handler in p.handlers.lock().iter() {
                trace_promise_handler(handler, worklist);
            }
        }
        HeapObj::Generator(g) => {
            worklist.push(g.closure.0);
            for v in g.state.lock().iter() {
                push_value(v, worklist);
            }
        }
        HeapObj::LazyGenerator(g) => {
            worklist.push(g.closure.lock().0);
            worklist.push(g.env.lock().0);
            for (_, _, env, _) in g.catch_stack.lock().iter() {
                worklist.push(env.0);
            }
            for guard in g.finally_stack.lock().iter() {
                worklist.push(guard.env.0);
            }
            for v in g.stack.lock().iter() {
                push_value(v, worklist);
            }
            for v in g.locals.lock().iter() {
                push_value(v, worklist);
            }
            for v in g.compiler_temps.lock().iter() {
                push_value(v, worklist);
            }
            for completion in g.finally_completions.lock().iter() {
                push_value(&completion.value, worklist);
            }
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
                *cells[idx].obj.lock() = Some(Arc::new(obj));
                cells[idx].private_elements.lock().clear();
                cells[idx].marked.store(false, Ordering::Relaxed);
                idx
            } else {
                let mut cells = self.cells.lock();
                let idx = cells.len();
                cells.push(GcCell {
                    obj: Mutex::new(Some(Arc::new(obj))),
                    private_elements: Mutex::new(std::collections::HashMap::new()),
                    marked: AtomicBool::new(false),
                    active_accesses: AtomicUsize::new(0),
                });
                idx
            }
        };
        if let Some(state) = incremental_mark.as_mut() {
            if idx >= state.marked.len() {
                state.marked.resize(idx + 1, false);
                state.queued.resize(idx + 1, false);
                state.dirty_queued.resize(idx + 1, false);
            }
            state.queue_index(idx);
        }
        self.alloc_since_gc.fetch_add(1, Ordering::Relaxed);
        Ok(idx)
    }

    pub fn collect(&self, roots: &[usize]) {
        self.collect_incremental(roots, usize::MAX);
    }

    /// Incremental GC: process up to `budget` trace work units, then sweep if
    /// marking is done. Cell headers, retrace visits, and cursorized vector
    /// slots each consume one unit.
    /// With `usize::MAX`, a new cycle is a full stop-the-world collection and an
    /// existing cycle is completed without yielding while retaining its prior
    /// marks. Finite-budget cycles snapshot roots on their first slice. New
    /// allocations are queued by `allocate`, while the final retrace catches
    /// edges added through the heap access APIs between slices.
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
                    phase: IncrementalPhase::Mark,
                    dirty: Vec::new(),
                    dirty_queued: vec![false; cells_len],
                },
                true,
            ),
        };
        state.marked.resize(cells_len, false);
        state.queued.resize(cells_len, false);
        state.dirty_queued.resize(cells_len, false);
        if new_cycle || budget == usize::MAX {
            for root in roots {
                state.queue_index(*root);
            }
        }
        if matches!(state.phase, IncrementalPhase::Retrace { .. }) {
            for root in roots {
                state.queue_index(*root);
            }
        }
        {
            let cells = self.cells.lock();
            for (index, cell) in cells.iter().enumerate() {
                if cell.active_accesses.load(Ordering::Relaxed) > 0 {
                    state.queue_index(index);
                    state.queue_dirty(index);
                }
            }
        }

        let mut traced_this_slice = 0usize;
        loop {
            while let Some(work) = state.worklist.pop() {
                match work {
                    TraceWork::Cell(index) => {
                        if index < state.queued.len() {
                            state.queued[index] = false;
                        }
                        if index >= cells_len || state.marked[index] {
                            continue;
                        }
                        if traced_this_slice >= budget && budget != usize::MAX {
                            state.queue_index(index);
                            *state_slot = Some(state);
                            return;
                        }
                        state.marked[index] = true;
                        traced_this_slice = traced_this_slice.saturating_add(1);

                        let trace = {
                            let cells = self.cells.lock();
                            trace_cell(&cells, index, budget != usize::MAX)
                        };
                        schedule_trace(&mut state, trace);
                        if let Some(values) = state.pending_ephemerons.remove(&index) {
                            for value in values {
                                state.queue_value(&value);
                            }
                        }
                    }
                    TraceWork::VecEdges(trace) => {
                        let remaining = if budget == usize::MAX {
                            trace.next
                        } else {
                            budget.saturating_sub(traced_this_slice)
                        };
                        if remaining == 0 {
                            state.worklist.push(TraceWork::VecEdges(trace));
                            *state_slot = Some(state);
                            return;
                        }
                        let (roots, next) = trace_vec_slots(&self.cells, trace, remaining);
                        let processed = trace.next - next;
                        traced_this_slice = traced_this_slice.saturating_add(processed);
                        if next > 0 {
                            state
                                .worklist
                                .push(TraceWork::VecEdges(VecTrace { next, ..trace }));
                        }
                        for root in roots {
                            state.queue_index(root);
                        }
                    }
                }
            }

            match state.phase {
                IncrementalPhase::Mark => {
                    // Remark current roots before beginning the resumable
                    // pre-sweep retrace. This catches roots published after the
                    // first finite-budget slice.
                    for root in roots {
                        state.queue_index(*root);
                    }
                    if !state.worklist.is_empty() {
                        continue;
                    }
                    state.pending_ephemerons.clear();
                    state.phase = IncrementalPhase::Retrace { cursor: 0 };
                }
                IncrementalPhase::Retrace { mut cursor } => {
                    while cursor < cells_len {
                        if traced_this_slice >= budget && budget != usize::MAX {
                            state.phase = IncrementalPhase::Retrace { cursor };
                            *state_slot = Some(state);
                            return;
                        }
                        let index = cursor;
                        cursor += 1;
                        traced_this_slice = traced_this_slice.saturating_add(1);
                        state.phase = IncrementalPhase::Retrace { cursor };
                        if state.marked[index] {
                            let trace = {
                                let cells = self.cells.lock();
                                trace_cell(&cells, index, budget != usize::MAX)
                            };
                            schedule_trace(&mut state, trace);
                        }
                        if !state.worklist.is_empty() {
                            break;
                        }
                    }
                    if !state.worklist.is_empty() {
                        continue;
                    }
                    while let Some(index) = state.dirty.pop() {
                        state.dirty_queued[index] = false;
                        if traced_this_slice >= budget && budget != usize::MAX {
                            state.dirty_queued[index] = true;
                            state.dirty.push(index);
                            *state_slot = Some(state);
                            return;
                        }
                        traced_this_slice = traced_this_slice.saturating_add(1);
                        if state.marked[index] {
                            let trace = {
                                let cells = self.cells.lock();
                                trace_cell(&cells, index, budget != usize::MAX)
                            };
                            schedule_trace(&mut state, trace);
                        }
                        if !state.worklist.is_empty() {
                            break;
                        }
                    }
                    if !state.worklist.is_empty() {
                        continue;
                    }
                    if cursor >= cells_len && state.dirty.is_empty() {
                        break;
                    }
                }
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
                match obj.as_ref() {
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
                        let mut cleanup_pending = registry.cleanup_pending.load(Ordering::Relaxed);
                        for cell in registry.cells.lock().iter_mut() {
                            if matches!(cell.target.as_ref(), Some(crate::value::Value::Object(idx))
                                if idx.0 >= marked.len() || !marked[idx.0])
                            {
                                cell.target = None;
                                cleanup_pending = true;
                            }
                            if matches!(cell.unregister_token.as_ref(), Some(crate::value::Value::Object(idx))
                                if idx.0 >= marked.len() || !marked[idx.0])
                            {
                                cell.unregister_token = None;
                            }
                        }
                        registry
                            .cleanup_pending
                            .store(cleanup_pending, Ordering::Relaxed);
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

    pub fn take_pending_finalization_registries(&self) -> Option<Vec<usize>> {
        let cells = self.cells.lock();
        let pending_count = cells
            .iter()
            .filter(|cell| {
                let obj = cell.obj.lock();
                let Some(HeapObj::FinalizationRegistry(registry)) = obj.as_deref() else {
                    return false;
                };
                registry.cleanup_pending.load(Ordering::Relaxed)
                    && !registry.cleanup_scheduled.load(Ordering::Relaxed)
            })
            .count();
        let mut pending = Vec::new();
        pending.try_reserve_exact(pending_count).ok()?;
        for (idx, cell) in cells.iter().enumerate() {
            if pending.len() == pending_count {
                break;
            }
            let obj = cell.obj.lock();
            let Some(HeapObj::FinalizationRegistry(registry)) = obj.as_deref() else {
                continue;
            };
            if registry.cleanup_pending.load(Ordering::Relaxed)
                && registry
                    .cleanup_scheduled
                    .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
            {
                pending.push(idx);
            }
        }
        Some(pending)
    }

    pub fn reset_finalization_cleanup_scheduled(&self, registries: &[usize]) {
        let cells = self.cells.lock();
        for registry in registries {
            let Some(cell) = cells.get(*registry) else {
                continue;
            };
            let obj = cell.obj.lock();
            if let Some(HeapObj::FinalizationRegistry(registry)) = obj.as_deref() {
                registry.cleanup_scheduled.store(false, Ordering::Relaxed);
            }
        }
    }

    pub fn live_count(&self) -> usize {
        // Lock order must match `allocate` (free_list before cells) to avoid a
        // lock-order inversion deadlock if both are ever held concurrently.
        let free = self.free_list.lock();
        let cells = self.cells.lock();
        cells.len() - free.len()
    }

    #[cfg(test)]
    pub(crate) fn incremental_in_progress(&self) -> bool {
        self.incremental_mark.lock().is_some()
    }

    #[cfg(test)]
    pub(crate) fn incremental_retrace_passed(&self, index: usize) -> bool {
        self.incremental_mark.lock().as_ref().is_some_and(
            |state| matches!(state.phase, IncrementalPhase::Retrace { cursor } if cursor > index),
        )
    }

    #[cfg(test)]
    pub(crate) fn incremental_is_dirty(&self, index: usize) -> bool {
        self.incremental_mark
            .lock()
            .as_ref()
            .is_some_and(|state| state.dirty_queued.get(index).copied().unwrap_or(false))
    }

    pub fn set_max_objects(&self, max: usize) {
        self.max_objects.store(max, Ordering::Relaxed);
    }

    pub fn max_objects(&self) -> usize {
        self.max_objects.load(Ordering::Relaxed)
    }

    pub(crate) fn is_live(&self, index: usize) -> bool {
        self.cells
            .lock()
            .get(index)
            .is_some_and(|cell| cell.obj.lock().is_some())
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
        let result = {
            let cells = self.cells.lock();
            let mut private_elements = cells[idx].private_elements.lock();
            f(&mut private_elements)
        };
        self.note_incremental_access(idx);
        result
    }

    /// Access a heap object without hiding it from re-entrant collection.
    ///
    /// A callback that invokes collection must first drop every guard acquired
    /// from an interior object mutex such as `props()`. Holding such a guard
    /// while tracing the same object would self-deadlock.
    pub fn with_obj<R>(&self, idx: usize, f: impl FnOnce(&HeapObj) -> R) -> R {
        self.note_incremental_access(idx);
        self.with_obj_access(idx, true, f)
    }

    /// Read an object while retaining the active-access root without applying
    /// the mutation barrier. The callback must not mutate interior object data.
    pub(crate) fn with_obj_read<R>(&self, idx: usize, f: impl FnOnce(&HeapObj) -> R) -> R {
        self.with_obj_access(idx, false, f)
    }

    fn with_obj_access<R>(
        &self,
        idx: usize,
        dirty_on_drop: bool,
        f: impl FnOnce(&HeapObj) -> R,
    ) -> R {
        let obj = {
            let cells = self.cells.lock();
            let cell = &cells[idx];
            cell.active_accesses.fetch_add(1, Ordering::Relaxed);
            let obj = cell
                .obj
                .lock()
                .clone()
                .unwrap_or_else(|| Arc::new(crate::value::HeapObj::placeholder()));
            obj
        };
        let active_access = ActiveObjectAccess {
            heap: self,
            index: idx,
            dirty_on_drop,
        };
        let result = f(obj.as_ref());
        drop(active_access);
        result
    }

    fn note_incremental_access(&self, idx: usize) {
        if let Some(state) = self.incremental_mark.lock().as_mut() {
            state.queue_dirty(idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{
        FinalizationRegistryCell, FinalizationRegistryData, GcIdx, IteratorData, MapData, MapKey,
        PrivateNameKey, PrivateSlot, PrivateSlotKey, PromiseData, PromiseHandler, PromiseStatus,
        PropertyDescriptor, PropertyKey, Value, WeakKey, WeakMapData,
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

    fn finish_incremental(heap: &Heap, roots: &[usize], budget: usize) {
        let work_units = {
            let cells = heap.cells.lock();
            cells.iter().fold(cells.len(), |units, cell| {
                let obj = cell.obj.lock();
                units.saturating_add(match obj.as_deref() {
                    Some(HeapObj::Array(array)) => array.items.lock().len(),
                    Some(HeapObj::Iterator(iterator)) => iterator.items.lock().len(),
                    Some(HeapObj::Promise(promise)) => promise.handlers.lock().len(),
                    Some(HeapObj::FinalizationRegistry(registry)) => registry.cells.lock().len(),
                    Some(HeapObj::Map(map)) => map.entries.lock().len(),
                    _ => 0,
                })
            })
        };
        let max_slices = work_units.saturating_mul(16).max(64);
        for _ in 0..max_slices {
            if heap.incremental_mark.lock().is_none() {
                return;
            }
            heap.collect_incremental(roots, budget);
        }
        panic!("incremental collection did not finish within {max_slices} slices");
    }

    fn pending_vec_trace(
        state: &IncrementalMark,
        owner: usize,
        kind: VecTraceKind,
    ) -> Option<VecTrace> {
        state.worklist.iter().rev().find_map(|work| match work {
            TraceWork::VecEdges(trace) if trace.owner == owner && trace.kind == kind => {
                Some(*trace)
            }
            _ => None,
        })
    }

    fn iterator(items: Vec<Value>, lazy: Option<Value>) -> HeapObj {
        HeapObj::Iterator(IteratorData {
            items: Mutex::new(items),
            index: AtomicUsize::new(0),
            lazy_iter: Mutex::new(lazy),
            lazy_next: Mutex::new(None),
            generator: Mutex::new(None),
            for_in: Mutex::new(None),
            async_from_sync: AtomicBool::new(false),
            done: AtomicBool::new(false),
        })
    }

    fn promise(handlers: Vec<PromiseHandler>) -> HeapObj {
        HeapObj::Promise(PromiseData {
            state: Mutex::new(PromiseStatus::Pending),
            result: Mutex::new(Value::Undefined),
            handlers: Mutex::new(handlers),
            props: Mutex::new(indexmap::IndexMap::new()),
            proto: Mutex::new(None),
            extensible: AtomicBool::new(true),
        })
    }

    fn promise_handler(on_fulfilled: Value, on_rejected: Value) -> PromiseHandler {
        PromiseHandler {
            on_fulfilled,
            on_rejected,
            derived: None,
            continuation: None,
        }
    }

    fn finalization_registry(
        cleanup_callback: Value,
        cells: Vec<FinalizationRegistryCell>,
    ) -> HeapObj {
        // Collector fixtures do not construct Realm environments. The marker
        // deliberately ignores this out-of-range root after tracing the slot.
        HeapObj::FinalizationRegistry(FinalizationRegistryData {
            cleanup_callback,
            realm: GcIdx(usize::MAX),
            cells: Mutex::new(cells),
            cleanup_pending: AtomicBool::new(false),
            cleanup_scheduled: AtomicBool::new(false),
            props: Mutex::new(indexmap::IndexMap::new()),
            proto: Mutex::new(None),
            extensible: AtomicBool::new(true),
        })
    }

    fn map(entries: impl IntoIterator<Item = (Value, Value)>) -> HeapObj {
        HeapObj::Map(MapData {
            entries: Mutex::new(
                entries
                    .into_iter()
                    .map(|(key, value)| (MapKey::new(key), value))
                    .collect(),
            ),
            props: Mutex::new(indexmap::IndexMap::new()),
            proto: Mutex::new(None),
            extensible: AtomicBool::new(true),
        })
    }

    fn advance_to_retrace_vec(
        heap: &Heap,
        roots: &[usize],
        owner: usize,
        kind: VecTraceKind,
        expected_next: usize,
    ) {
        for _ in 0..256 {
            heap.collect_incremental(roots, 1);
            let state = heap.incremental_mark.lock();
            let state = state.as_ref().unwrap();
            if matches!(state.phase, IncrementalPhase::Retrace { cursor } if cursor > owner)
                && pending_vec_trace(state, owner, kind)
                    .is_some_and(|trace| trace.next == expected_next)
            {
                return;
            }
        }
        panic!("incremental retrace did not park the expected vector cursor");
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
        finish_incremental(&heap, &roots, 1);
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
        finish_incremental(&heap, &[owner], 1);

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
        finish_incremental(&heap, &[owner], 1);

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
        finish_incremental(&heap, &[owner], 1);

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
        finish_incremental(&heap, &[owner, late_root], 1);

        assert_eq!(heap.live_count(), 3);
        assert!(heap.with_obj(late_root, |object| matches!(object, HeapObj::Object(_))));
        let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(fresh, garbage);
    }

    #[test]
    fn incremental_retrace_is_budgeted_and_revisits_scanned_objects() {
        let heap = Heap::new();
        let ordinary_owner = heap.allocate(HeapObj::placeholder()).unwrap();
        let private_owner = heap.allocate(HeapObj::placeholder()).unwrap();
        let ordinary_child = heap.allocate(HeapObj::placeholder()).unwrap();
        let private_child = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
        let roots = [ordinary_owner, private_owner];

        for _ in 0..2 {
            heap.collect_incremental(&roots, 1);
        }
        assert!(matches!(
            heap.incremental_mark
                .lock()
                .as_ref()
                .map(|state| state.phase),
            Some(IncrementalPhase::Retrace { cursor: 0 })
        ));

        for expected_cursor in 1..=2 {
            heap.collect_incremental(&roots, 1);
            assert!(matches!(
                heap.incremental_mark
                    .lock()
                    .as_ref()
                    .map(|state| state.phase),
                Some(IncrementalPhase::Retrace { cursor }) if cursor == expected_cursor
            ));
        }
        assert_eq!(heap.live_count(), 5, "partial retrace must not sweep");

        heap.with_obj(ordinary_owner, |object| {
            object.props().lock().insert(
                PropertyKey::from("ordinary"),
                PropertyDescriptor::data(Value::Object(GcIdx(ordinary_child))),
            );
        });
        let private_key = PrivateSlotKey::Private(PrivateNameKey {
            id: 2,
            description: Arc::from("private"),
        });
        heap.with_private_elements(private_owner, |elements| {
            elements.insert(
                private_key,
                PrivateSlot::Value(Value::Object(GcIdx(private_child))),
            );
        });
        heap.with_obj(ordinary_owner, |_| {});
        heap.with_private_elements(private_owner, |_| {});
        let dirty = heap.incremental_mark.lock().as_ref().unwrap().dirty.clone();
        assert_eq!(dirty, roots, "each access path must queue its owner once");
        finish_incremental(&heap, &roots, 1);

        assert_eq!(heap.live_count(), 4);
        let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(fresh, garbage);
        assert_ne!(fresh, ordinary_child);
        assert_ne!(fresh, private_child);
    }

    #[test]
    fn reentrant_collection_traces_an_active_object_after_mutation() {
        let heap = Heap::new();
        let owner = heap.allocate(HeapObj::placeholder()).unwrap();
        let child = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();

        heap.collect_incremental(&[owner], 1);
        heap.collect_incremental(&[owner], 1);
        assert!(matches!(
            heap.incremental_mark
                .lock()
                .as_ref()
                .map(|state| state.phase),
            Some(IncrementalPhase::Retrace { cursor: 1 })
        ));

        heap.with_obj(owner, |object| {
            object.props().lock().insert(
                PropertyKey::from("child"),
                PropertyDescriptor::data(Value::Object(GcIdx(child))),
            );
            heap.collect_incremental(&[], usize::MAX);
        });

        assert_eq!(heap.live_count(), 2);
        let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(fresh, garbage);
        assert_ne!(fresh, child);
    }

    #[test]
    fn new_collection_treats_an_active_object_as_a_root() {
        let heap = Heap::new();
        let owner = heap.allocate(HeapObj::placeholder()).unwrap();
        let child = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();

        heap.with_obj(owner, |object| {
            object.props().lock().insert(
                PropertyKey::from("child"),
                PropertyDescriptor::data(Value::Object(GcIdx(child))),
            );
            heap.collect(&[]);
        });

        assert!(heap.incremental_mark.lock().is_none());
        assert_eq!(heap.live_count(), 2);
        let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(fresh, garbage);
        assert_ne!(fresh, child);
    }

    #[test]
    fn active_object_access_releases_its_root_during_unwind() {
        let heap = Heap::new();
        let owner = heap.allocate(HeapObj::placeholder()).unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            heap.with_obj(owner, |_| panic!("host callback panic"));
        }));

        assert!(result.is_err());
        assert_eq!(
            heap.cells.lock()[owner]
                .active_accesses
                .load(Ordering::Relaxed),
            0
        );
        heap.collect(&[]);
        assert_eq!(heap.live_count(), 0);
    }

    #[test]
    fn zero_incremental_budget_does_not_trace_or_sweep() {
        let heap = Heap::new();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();

        heap.collect_incremental(&[], 0);
        heap.collect_incremental(&[], 0);

        assert_eq!(heap.live_count(), 1);
        assert!(matches!(
            heap.incremental_mark
                .lock()
                .as_ref()
                .map(|state| state.phase),
            Some(IncrementalPhase::Retrace { cursor: 0 })
        ));

        heap.collect_incremental(&[], 1);
        assert_eq!(heap.live_count(), 0);
        let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(fresh, garbage);
    }

    #[test]
    fn incremental_array_items_consume_one_budget_unit_per_slot() {
        let heap = Heap::new();
        let owner = heap
            .allocate(HeapObj::Array(crate::value::ArrayData::new(
                Vec::new(),
                None,
            )))
            .unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
        heap.with_obj(owner, |object| {
            let HeapObj::Array(array) = object else {
                panic!("array fixture lost its type");
            };
            array
                .items
                .lock()
                .extend((0..8).map(|value| Value::Number(value.into())));
        });

        heap.collect_incremental(&[owner], 1);
        assert_eq!(
            pending_vec_trace(
                heap.incremental_mark.lock().as_ref().unwrap(),
                owner,
                VecTraceKind::ArrayItems,
            )
            .map(|trace| trace.next),
            Some(8)
        );

        for remaining in (0..8).rev() {
            heap.collect_incremental(&[owner], 1);
            let next = pending_vec_trace(
                heap.incremental_mark.lock().as_ref().unwrap(),
                owner,
                VecTraceKind::ArrayItems,
            )
            .map(|trace| trace.next);
            assert_eq!(next, (remaining > 0).then_some(remaining));
            assert_eq!(heap.live_count(), 2, "partial trace must not sweep");
        }

        finish_incremental(&heap, &[owner], 1);
        assert_eq!(heap.live_count(), 1);
        let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(fresh, garbage);
    }

    #[test]
    fn usize_max_completes_pending_array_and_iterator_traces() {
        let heap = Heap::new();
        let array_child = heap.allocate(HeapObj::placeholder()).unwrap();
        let iterator_item = heap.allocate(HeapObj::placeholder()).unwrap();
        let iterator_lazy = heap.allocate(HeapObj::placeholder()).unwrap();
        let array = heap
            .allocate(HeapObj::Array(crate::value::ArrayData::new(
                Vec::new(),
                None,
            )))
            .unwrap();
        heap.with_obj(array, |object| {
            let HeapObj::Array(array) = object else {
                panic!("array fixture lost its type");
            };
            array.items.lock().push(Value::Object(GcIdx(array_child)));
        });
        let iterator = heap
            .allocate(iterator(
                vec![Value::Object(GcIdx(iterator_item))],
                Some(Value::Object(GcIdx(iterator_lazy))),
            ))
            .unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();

        heap.collect_incremental(&[array, iterator], 1);
        assert!(heap.incremental_mark.lock().is_some());
        heap.collect_incremental(&[array, iterator], usize::MAX);

        assert!(heap.incremental_mark.lock().is_none());
        assert_eq!(heap.live_count(), 5);
        let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(fresh, garbage);
        for retained in [array_child, iterator_item, iterator_lazy] {
            assert_ne!(fresh, retained);
        }
    }

    #[test]
    fn newly_queued_cell_preempts_a_parked_vec_trace() {
        let heap = Heap::new();
        let owner = heap
            .allocate(HeapObj::Array(crate::value::ArrayData::new(
                Vec::new(),
                None,
            )))
            .unwrap();
        heap.with_obj(owner, |object| {
            let HeapObj::Array(array) = object else {
                panic!("array fixture lost its type");
            };
            array
                .items
                .lock()
                .extend([Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]);
        });

        heap.collect_incremental(&[owner], 1);
        let late = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(
            heap.incremental_mark
                .lock()
                .as_ref()
                .unwrap()
                .worklist
                .last(),
            Some(&TraceWork::Cell(late))
        );

        heap.collect_incremental(&[owner], 1);
        let state = heap.incremental_mark.lock();
        let state = state.as_ref().unwrap();
        assert!(state.marked[late]);
        assert_eq!(
            pending_vec_trace(state, owner, VecTraceKind::ArrayItems).map(|trace| trace.next),
            Some(3)
        );
    }

    #[test]
    fn array_vec_trace_snapshots_growth_and_charges_removed_slots() {
        {
            let heap = Heap::new();
            let owner = heap
                .allocate(HeapObj::Array(crate::value::ArrayData::new(
                    Vec::new(),
                    None,
                )))
                .unwrap();
            let retained = heap.allocate(HeapObj::placeholder()).unwrap();
            let appended = heap.allocate(HeapObj::placeholder()).unwrap();
            let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
            heap.with_obj(owner, |object| {
                let HeapObj::Array(array) = object else {
                    panic!("array fixture lost its type");
                };
                array.items.lock().push(Value::Object(GcIdx(retained)));
            });

            heap.collect_incremental(&[owner], 1);
            heap.with_obj(owner, |object| {
                let HeapObj::Array(array) = object else {
                    panic!("array fixture lost its type");
                };
                array.items.lock().push(Value::Object(GcIdx(appended)));
            });
            heap.collect_incremental(&[owner], 1);
            {
                let state = heap.incremental_mark.lock();
                let state = state.as_ref().unwrap();
                assert!(pending_vec_trace(state, owner, VecTraceKind::ArrayItems).is_none());
                assert!(
                    !state.marked[appended],
                    "append is outside the current snapshot"
                );
            }
            finish_incremental(&heap, &[owner], 1);

            assert_eq!(heap.live_count(), 3);
            assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
        }

        {
            let heap = Heap::new();
            let owner = heap
                .allocate(HeapObj::Array(crate::value::ArrayData::new(
                    Vec::new(),
                    None,
                )))
                .unwrap();
            let retained = heap.allocate(HeapObj::placeholder()).unwrap();
            let removed = heap.allocate(HeapObj::placeholder()).unwrap();
            let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
            heap.with_obj(owner, |object| {
                let HeapObj::Array(array) = object else {
                    panic!("array fixture lost its type");
                };
                array.items.lock().extend([
                    Value::Object(GcIdx(retained)),
                    Value::Object(GcIdx(removed)),
                ]);
            });

            heap.collect_incremental(&[owner], 1);
            heap.with_obj(owner, |object| {
                let HeapObj::Array(array) = object else {
                    panic!("array fixture lost its type");
                };
                array.items.lock().truncate(1);
            });
            heap.collect_incremental(&[owner], 1);
            {
                let state = heap.incremental_mark.lock();
                let state = state.as_ref().unwrap();
                assert_eq!(
                    pending_vec_trace(state, owner, VecTraceKind::ArrayItems)
                        .map(|trace| trace.next),
                    Some(1),
                    "a removed snapshot slot still consumes one work unit"
                );
                assert!(!state.marked[removed]);
            }
            finish_incremental(&heap, &[owner], 1);

            assert_eq!(heap.live_count(), 2);
            assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
            assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), removed);
        }
    }

    #[test]
    fn iterator_auxiliary_edges_precede_item_continuation() {
        let heap = Heap::new();
        let item = heap.allocate(HeapObj::placeholder()).unwrap();
        let lazy = heap.allocate(HeapObj::placeholder()).unwrap();
        let owner = heap
            .allocate(iterator(
                vec![Value::Object(GcIdx(item))],
                Some(Value::Object(GcIdx(lazy))),
            ))
            .unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();

        heap.collect_incremental(&[owner], 1);
        {
            let state = heap.incremental_mark.lock();
            let state = state.as_ref().unwrap();
            assert_eq!(state.worklist.last(), Some(&TraceWork::Cell(lazy)));
            assert_eq!(
                pending_vec_trace(state, owner, VecTraceKind::IteratorItems)
                    .map(|trace| trace.next),
                Some(1)
            );
        }
        finish_incremental(&heap, &[owner], 1);

        assert_eq!(heap.live_count(), 3);
        assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
    }

    #[test]
    fn iterator_vec_trace_batches_finite_budget_and_snapshots_growth() {
        let heap = Heap::new();
        let appended = heap.allocate(HeapObj::placeholder()).unwrap();
        let owner = heap
            .allocate(iterator(
                vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)],
                None,
            ))
            .unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();

        heap.collect_incremental(&[owner], 1);
        heap.collect_incremental(&[owner], 2);
        assert_eq!(
            pending_vec_trace(
                heap.incremental_mark.lock().as_ref().unwrap(),
                owner,
                VecTraceKind::IteratorItems,
            )
            .map(|trace| trace.next),
            Some(1)
        );
        heap.with_obj(owner, |object| {
            let HeapObj::Iterator(iterator) = object else {
                panic!("iterator fixture lost its type");
            };
            iterator.items.lock().push(Value::Object(GcIdx(appended)));
        });
        heap.collect_incremental(&[owner], 1);
        {
            let state = heap.incremental_mark.lock();
            let state = state.as_ref().unwrap();
            assert!(pending_vec_trace(state, owner, VecTraceKind::IteratorItems).is_none());
            assert!(!state.marked[appended]);
        }
        finish_incremental(&heap, &[owner], 1);

        assert_eq!(heap.live_count(), 2);
        assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
    }

    #[test]
    fn map_entry_cursor_charges_one_record_and_traces_key_and_value() {
        let heap = Heap::new();
        let key = heap.allocate(HeapObj::placeholder()).unwrap();
        let value = heap.allocate(HeapObj::placeholder()).unwrap();
        let owner = heap
            .allocate(map([
                (Value::Number(0.0), Value::Undefined),
                (Value::Object(GcIdx(key)), Value::Object(GcIdx(value))),
            ]))
            .unwrap();

        heap.collect_incremental(&[owner], 1);
        heap.collect_incremental(&[owner], 1);

        let state = heap.incremental_mark.lock();
        let state = state.as_ref().unwrap();
        assert_eq!(
            pending_vec_trace(state, owner, VecTraceKind::MapEntries).map(|trace| trace.next),
            Some(1)
        );
        assert!(state.queued[key]);
        assert!(state.queued[value]);
        assert!(!state.marked[key]);
        assert!(!state.marked[value]);
    }

    #[test]
    fn map_entry_cursor_batches_finite_budget() {
        let heap = Heap::new();
        let owner = heap
            .allocate(map(
                (0..5).map(|key| (Value::Number(key.into()), Value::Undefined))
            ))
            .unwrap();

        heap.collect_incremental(&[owner], 1);
        heap.collect_incremental(&[owner], 3);

        assert_eq!(
            pending_vec_trace(
                heap.incremental_mark.lock().as_ref().unwrap(),
                owner,
                VecTraceKind::MapEntries,
            )
            .map(|trace| trace.next),
            Some(2)
        );
    }

    #[test]
    fn map_entry_cursor_snapshots_growth_and_charges_removed_records() {
        {
            let heap = Heap::new();
            let owner = heap.allocate(map([])).unwrap();
            let first = heap.allocate(HeapObj::placeholder()).unwrap();
            let appended = heap.allocate(HeapObj::placeholder()).unwrap();
            let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
            heap.with_obj(owner, |object| {
                let HeapObj::Map(map) = object else {
                    panic!("Map fixture lost its type");
                };
                map.entries
                    .lock()
                    .insert(MapKey::new(Value::Number(0.0)), Value::Object(GcIdx(first)));
            });

            heap.collect_incremental(&[owner], 1);
            heap.with_obj(owner, |object| {
                let HeapObj::Map(map) = object else {
                    panic!("Map fixture lost its type");
                };
                map.entries.lock().insert(
                    MapKey::new(Value::Number(1.0)),
                    Value::Object(GcIdx(appended)),
                );
            });
            heap.collect_incremental(&[owner], 1);
            {
                let state = heap.incremental_mark.lock();
                let state = state.as_ref().unwrap();
                assert!(pending_vec_trace(state, owner, VecTraceKind::MapEntries).is_none());
                assert!(!state.marked[appended]);
            }
            finish_incremental(&heap, &[owner], 1);

            assert_eq!(heap.live_count(), 3);
            assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
        }

        {
            let heap = Heap::new();
            let owner = heap
                .allocate(map([
                    (Value::Number(0.0), Value::Undefined),
                    (Value::Number(1.0), Value::Undefined),
                ]))
                .unwrap();
            heap.collect_incremental(&[owner], 1);
            heap.with_obj(owner, |object| {
                let HeapObj::Map(map) = object else {
                    panic!("Map fixture lost its type");
                };
                map.entries.lock().clear();
            });
            heap.collect_incremental(&[owner], 1);

            assert_eq!(
                pending_vec_trace(
                    heap.incremental_mark.lock().as_ref().unwrap(),
                    owner,
                    VecTraceKind::MapEntries,
                )
                .map(|trace| trace.next),
                Some(1),
                "a removed Map snapshot record must still consume one work unit"
            );
        }
    }

    #[test]
    fn map_entry_cursor_retraces_shift_and_reinsert() {
        let heap = Heap::new();
        let owner = heap.allocate(map([])).unwrap();
        let low = heap.allocate(HeapObj::placeholder()).unwrap();
        let middle = heap.allocate(HeapObj::placeholder()).unwrap();
        let high = heap.allocate(HeapObj::placeholder()).unwrap();
        let late = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
        heap.with_obj(owner, |object| {
            let HeapObj::Map(map) = object else {
                panic!("Map fixture lost its type");
            };
            map.entries.lock().extend([
                (MapKey::new(Value::Number(0.0)), Value::Object(GcIdx(low))),
                (
                    MapKey::new(Value::Number(1.0)),
                    Value::Object(GcIdx(middle)),
                ),
                (MapKey::new(Value::Number(2.0)), Value::Object(GcIdx(high))),
            ]);
        });

        advance_to_retrace_vec(&heap, &[owner], owner, VecTraceKind::MapEntries, 3);
        heap.collect_incremental(&[owner], 1);
        heap.with_obj(owner, |object| {
            let HeapObj::Map(map) = object else {
                panic!("Map fixture lost its type");
            };
            let mut entries = map.entries.lock();
            entries.shift_remove(&MapKey::new(Value::Number(0.0)));
            entries.insert(MapKey::new(Value::Number(3.0)), Value::Object(GcIdx(late)));
        });
        assert_eq!(
            heap.incremental_mark.lock().as_ref().unwrap().dirty,
            [owner]
        );
        finish_incremental(&heap, &[owner], 1);

        assert_eq!(heap.live_count(), 5);
        assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
        assert_ne!(heap.allocate(HeapObj::placeholder()).unwrap(), late);
    }

    #[test]
    fn map_entry_cursor_dirty_retrace_preserves_replaced_value() {
        let heap = Heap::new();
        let owner = heap.allocate(map([])).unwrap();
        let original = heap.allocate(HeapObj::placeholder()).unwrap();
        let replacement = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
        heap.with_obj(owner, |object| {
            let HeapObj::Map(map) = object else {
                panic!("Map fixture lost its type");
            };
            map.entries.lock().insert(
                MapKey::new(Value::Number(0.0)),
                Value::Object(GcIdx(original)),
            );
        });

        advance_to_retrace_vec(&heap, &[owner], owner, VecTraceKind::MapEntries, 1);
        heap.collect_incremental(&[owner], 1);
        heap.with_obj(owner, |object| {
            let HeapObj::Map(map) = object else {
                panic!("Map fixture lost its type");
            };
            map.entries.lock().insert(
                MapKey::new(Value::Number(0.0)),
                Value::Object(GcIdx(replacement)),
            );
        });
        assert_eq!(
            heap.incremental_mark.lock().as_ref().unwrap().dirty,
            [owner]
        );
        finish_incremental(&heap, &[owner], 1);

        assert_eq!(heap.live_count(), 3);
        assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
        assert_ne!(heap.allocate(HeapObj::placeholder()).unwrap(), replacement);
    }

    #[test]
    fn map_entry_cursor_retraces_clear_and_reinsert() {
        let heap = Heap::new();
        let owner = heap
            .allocate(map(
                (0..3).map(|key| (Value::Number(key.into()), Value::Undefined))
            ))
            .unwrap();
        let late = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();

        advance_to_retrace_vec(&heap, &[owner], owner, VecTraceKind::MapEntries, 3);
        heap.collect_incremental(&[owner], 1);
        heap.with_obj(owner, |object| {
            let HeapObj::Map(map) = object else {
                panic!("Map fixture lost its type");
            };
            let mut entries = map.entries.lock();
            entries.clear();
            entries.insert(MapKey::new(Value::Number(3.0)), Value::Object(GcIdx(late)));
        });
        finish_incremental(&heap, &[owner], 1);

        assert_eq!(heap.live_count(), 2);
        assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
        assert_ne!(heap.allocate(HeapObj::placeholder()).unwrap(), late);
    }

    #[test]
    fn read_only_map_access_does_not_redirty_a_dirty_cursor() {
        let heap = Heap::new();
        let owner = heap
            .allocate(map(
                (0..3).map(|key| (Value::Number(key.into()), Value::Undefined))
            ))
            .unwrap();

        advance_to_retrace_vec(&heap, &[owner], owner, VecTraceKind::MapEntries, 3);
        heap.with_obj(owner, |object| {
            let HeapObj::Map(map) = object else {
                panic!("Map fixture lost its type");
            };
            map.entries
                .lock()
                .insert(MapKey::new(Value::Number(0.0)), Value::Null);
        });

        let mut dirty_cursor_started = false;
        for _ in 0..32 {
            heap.collect_incremental(&[owner], 1);
            let state = heap.incremental_mark.lock();
            let state = state.as_ref().unwrap();
            if state.dirty.is_empty()
                && !state.dirty_queued[owner]
                && pending_vec_trace(state, owner, VecTraceKind::MapEntries)
                    .is_some_and(|trace| trace.next == 3)
            {
                dirty_cursor_started = true;
                break;
            }
        }
        assert!(dirty_cursor_started, "dirty Map cursor was not scheduled");

        for _ in 0..16 {
            let len = heap.with_obj_read(owner, |object| {
                let HeapObj::Map(map) = object else {
                    panic!("Map fixture lost its type");
                };
                map.entries.lock().len()
            });
            assert_eq!(len, 3);
            {
                let state = heap.incremental_mark.lock();
                let state = state.as_ref().unwrap();
                assert!(state.dirty.is_empty());
                assert!(!state.dirty_queued[owner]);
            }
            heap.collect_incremental(&[owner], 1);
            if heap.incremental_mark.lock().is_none() {
                break;
            }
        }
        assert!(heap.incremental_mark.lock().is_none());
    }

    #[test]
    fn map_cursor_preserves_fixed_record_private_lifo_order() {
        let heap = Heap::new();
        let owner = heap.allocate(map([])).unwrap();
        let fixed = heap.allocate(HeapObj::placeholder()).unwrap();
        let low_key = heap.allocate(HeapObj::placeholder()).unwrap();
        let low_value = heap.allocate(HeapObj::placeholder()).unwrap();
        let high_key = heap.allocate(HeapObj::placeholder()).unwrap();
        let high_value = heap.allocate(HeapObj::placeholder()).unwrap();
        let private = heap.allocate(HeapObj::placeholder()).unwrap();
        heap.with_obj(owner, |object| {
            let HeapObj::Map(map) = object else {
                panic!("Map fixture lost its type");
            };
            map.props.lock().insert(
                PropertyKey::from("fixed"),
                PropertyDescriptor::data(Value::Object(GcIdx(fixed))),
            );
            map.entries.lock().extend([
                (
                    MapKey::new(Value::Object(GcIdx(low_key))),
                    Value::Object(GcIdx(low_value)),
                ),
                (
                    MapKey::new(Value::Object(GcIdx(high_key))),
                    Value::Object(GcIdx(high_value)),
                ),
            ]);
        });
        heap.with_private_elements(owner, |elements| {
            elements.insert(
                PrivateSlotKey::Private(PrivateNameKey {
                    id: 8,
                    description: Arc::from("map-cursor-order"),
                }),
                PrivateSlot::Value(Value::Object(GcIdx(private))),
            );
        });

        heap.collect_incremental(&[owner], 1);
        assert_eq!(
            heap.incremental_mark
                .lock()
                .as_ref()
                .unwrap()
                .worklist
                .last(),
            Some(&TraceWork::Cell(private))
        );
        heap.collect_incremental(&[owner], 1);
        assert_eq!(
            pending_vec_trace(
                heap.incremental_mark.lock().as_ref().unwrap(),
                owner,
                VecTraceKind::MapEntries,
            )
            .map(|trace| trace.next),
            Some(2)
        );
        heap.collect_incremental(&[owner], 1);
        {
            let state = heap.incremental_mark.lock();
            let state = state.as_ref().unwrap();
            assert_eq!(state.worklist.last(), Some(&TraceWork::Cell(high_value)));
            assert!(state.worklist.contains(&TraceWork::Cell(high_key)));
            assert!(state.worklist.contains(&TraceWork::Cell(fixed)));
            assert_eq!(
                pending_vec_trace(state, owner, VecTraceKind::MapEntries).map(|trace| trace.next),
                Some(1)
            );
        }
        heap.collect_incremental(&[owner], 1);
        heap.collect_incremental(&[owner], 1);
        heap.collect_incremental(&[owner], 1);
        let state = heap.incremental_mark.lock();
        let state = state.as_ref().unwrap();
        assert_eq!(state.worklist.last(), Some(&TraceWork::Cell(low_value)));
        assert!(state.worklist.contains(&TraceWork::Cell(low_key)));
        assert!(state.worklist.contains(&TraceWork::Cell(fixed)));
    }

    #[test]
    fn usize_max_uses_direct_map_trace_and_completes_pending_cursor() {
        let direct_heap = Heap::new();
        let key = direct_heap.allocate(HeapObj::placeholder()).unwrap();
        let value = direct_heap.allocate(HeapObj::placeholder()).unwrap();
        let owner = direct_heap
            .allocate(map([(
                Value::Object(GcIdx(key)),
                Value::Object(GcIdx(value)),
            )]))
            .unwrap();
        direct_heap.collect_incremental(&[owner], usize::MAX);
        assert!(direct_heap.incremental_mark.lock().is_none());
        assert_eq!(direct_heap.live_count(), 3);

        let pending_heap = Heap::new();
        let key = pending_heap.allocate(HeapObj::placeholder()).unwrap();
        let value = pending_heap.allocate(HeapObj::placeholder()).unwrap();
        let owner = pending_heap
            .allocate(map([
                (Value::Number(0.0), Value::Undefined),
                (Value::Object(GcIdx(key)), Value::Object(GcIdx(value))),
            ]))
            .unwrap();
        pending_heap.collect_incremental(&[owner], 1);
        assert_eq!(
            pending_vec_trace(
                pending_heap.incremental_mark.lock().as_ref().unwrap(),
                owner,
                VecTraceKind::MapEntries,
            )
            .map(|trace| trace.next),
            Some(2)
        );
        pending_heap.collect_incremental(&[owner], usize::MAX);
        assert!(pending_heap.incremental_mark.lock().is_none());
        assert_eq!(pending_heap.live_count(), 3);
    }

    #[test]
    fn promise_handler_tracer_preserves_multi_root_push_order() {
        let mut roots = Vec::new();
        let handler = PromiseHandler {
            on_fulfilled: Value::Object(GcIdx(1)),
            on_rejected: Value::Object(GcIdx(2)),
            derived: Some(crate::value::PromiseReactionCapability {
                promise: Value::Object(GcIdx(3)),
                resolve: Value::Object(GcIdx(4)),
                reject: Value::Object(GcIdx(5)),
            }),
            continuation: Some(crate::value::PromiseContinuation::AsyncGenerator {
                generator: GcIdx(6),
                kind: crate::value::AsyncGeneratorAwaitKind::Resume,
            }),
        };

        trace_promise_handler(&handler, &mut roots);

        assert_eq!(roots, [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn promise_handler_cursor_charges_one_record_not_each_root() {
        let heap = Heap::new();
        let roots: Vec<_> = (0..6)
            .map(|_| heap.allocate(HeapObj::placeholder()).unwrap())
            .collect();
        let high_handler = PromiseHandler {
            on_fulfilled: Value::Object(GcIdx(roots[0])),
            on_rejected: Value::Object(GcIdx(roots[1])),
            derived: Some(crate::value::PromiseReactionCapability {
                promise: Value::Object(GcIdx(roots[2])),
                resolve: Value::Object(GcIdx(roots[3])),
                reject: Value::Object(GcIdx(roots[4])),
            }),
            continuation: Some(crate::value::PromiseContinuation::AsyncGenerator {
                generator: GcIdx(roots[5]),
                kind: crate::value::AsyncGeneratorAwaitKind::Resume,
            }),
        };
        let owner = heap
            .allocate(promise(vec![
                promise_handler(Value::Undefined, Value::Undefined),
                high_handler,
            ]))
            .unwrap();

        heap.collect_incremental(&[owner], 1);
        heap.collect_incremental(&[owner], 1);

        let state = heap.incremental_mark.lock();
        let state = state.as_ref().unwrap();
        assert_eq!(
            pending_vec_trace(state, owner, VecTraceKind::PromiseHandlers).map(|trace| trace.next),
            Some(1)
        );
        for root in roots {
            assert!(state.queued[root]);
            assert!(!state.marked[root]);
        }
    }

    #[test]
    fn promise_and_finalization_cursors_batch_finite_budget() {
        let fixtures = [
            (
                promise(
                    (0..5)
                        .map(|_| promise_handler(Value::Undefined, Value::Undefined))
                        .collect(),
                ),
                VecTraceKind::PromiseHandlers,
            ),
            (
                finalization_registry(
                    Value::Undefined,
                    (0..5)
                        .map(|_| FinalizationRegistryCell {
                            target: None,
                            held_value: Value::Undefined,
                            unregister_token: None,
                        })
                        .collect(),
                ),
                VecTraceKind::FinalizationRegistryCells,
            ),
        ];

        for (object, kind) in fixtures {
            let heap = Heap::new();
            let owner = heap.allocate(object).unwrap();
            heap.collect_incremental(&[owner], 1);
            heap.collect_incremental(&[owner], 3);
            assert_eq!(
                pending_vec_trace(heap.incremental_mark.lock().as_ref().unwrap(), owner, kind)
                    .map(|trace| trace.next),
                Some(2)
            );
        }
    }

    #[test]
    fn promise_and_finalization_cursors_snapshot_growth() {
        {
            let heap = Heap::new();
            let owner = heap.allocate(promise(Vec::new())).unwrap();
            let first = heap.allocate(HeapObj::placeholder()).unwrap();
            let appended = heap.allocate(HeapObj::placeholder()).unwrap();
            let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
            heap.with_obj(owner, |object| {
                let HeapObj::Promise(promise) = object else {
                    panic!("promise fixture lost its type");
                };
                promise.handlers.lock().push(promise_handler(
                    Value::Object(GcIdx(first)),
                    Value::Undefined,
                ));
            });
            heap.collect_incremental(&[owner], 1);
            heap.with_obj(owner, |object| {
                let HeapObj::Promise(promise) = object else {
                    panic!("promise fixture lost its type");
                };
                promise.handlers.lock().push(promise_handler(
                    Value::Object(GcIdx(appended)),
                    Value::Undefined,
                ));
            });
            heap.collect_incremental(&[owner], 1);
            {
                let state = heap.incremental_mark.lock();
                let state = state.as_ref().unwrap();
                assert!(pending_vec_trace(state, owner, VecTraceKind::PromiseHandlers).is_none());
                assert!(!state.marked[appended]);
            }
            finish_incremental(&heap, &[owner], 1);
            assert_eq!(heap.live_count(), 3);
            assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
        }

        {
            let heap = Heap::new();
            let owner = heap
                .allocate(finalization_registry(Value::Undefined, Vec::new()))
                .unwrap();
            let first = heap.allocate(HeapObj::placeholder()).unwrap();
            let appended = heap.allocate(HeapObj::placeholder()).unwrap();
            let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
            heap.with_obj(owner, |object| {
                let HeapObj::FinalizationRegistry(registry) = object else {
                    panic!("registry fixture lost its type");
                };
                registry.cells.lock().push(FinalizationRegistryCell {
                    target: None,
                    held_value: Value::Object(GcIdx(first)),
                    unregister_token: None,
                });
            });
            heap.collect_incremental(&[owner], 1);
            heap.with_obj(owner, |object| {
                let HeapObj::FinalizationRegistry(registry) = object else {
                    panic!("registry fixture lost its type");
                };
                registry.cells.lock().push(FinalizationRegistryCell {
                    target: None,
                    held_value: Value::Object(GcIdx(appended)),
                    unregister_token: None,
                });
            });
            heap.collect_incremental(&[owner], 1);
            {
                let state = heap.incremental_mark.lock();
                let state = state.as_ref().unwrap();
                assert!(
                    pending_vec_trace(state, owner, VecTraceKind::FinalizationRegistryCells,)
                        .is_none()
                );
                assert!(!state.marked[appended]);
            }
            finish_incremental(&heap, &[owner], 1);
            assert_eq!(heap.live_count(), 3);
            assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
        }
    }

    #[test]
    fn promise_and_finalization_retrace_growth_queues_fresh_snapshots() {
        {
            let heap = Heap::new();
            let owner = heap
                .allocate(promise(vec![promise_handler(
                    Value::Undefined,
                    Value::Undefined,
                )]))
                .unwrap();
            let appended = heap.allocate(HeapObj::placeholder()).unwrap();
            let garbage = heap.allocate(HeapObj::placeholder()).unwrap();

            for _ in 0..128 {
                heap.collect_incremental(&[owner], 1);
                let state = heap.incremental_mark.lock();
                let state = state.as_ref().unwrap();
                if matches!(state.phase, IncrementalPhase::Retrace { cursor: 1 })
                    && pending_vec_trace(state, owner, VecTraceKind::PromiseHandlers)
                        .is_some_and(|trace| trace.next == 1)
                {
                    break;
                }
            }
            heap.collect_incremental(&[owner], 1);
            heap.with_obj(owner, |object| {
                let HeapObj::Promise(promise) = object else {
                    panic!("promise fixture lost its type");
                };
                promise.handlers.lock().push(promise_handler(
                    Value::Object(GcIdx(appended)),
                    Value::Undefined,
                ));
            });
            assert_eq!(
                heap.incremental_mark.lock().as_ref().unwrap().dirty,
                [owner]
            );
            finish_incremental(&heap, &[owner], 1);

            assert_eq!(heap.live_count(), 2);
            assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
            assert_ne!(heap.allocate(HeapObj::placeholder()).unwrap(), appended);
        }

        {
            let heap = Heap::new();
            let owner = heap
                .allocate(finalization_registry(
                    Value::Undefined,
                    vec![FinalizationRegistryCell {
                        target: None,
                        held_value: Value::Undefined,
                        unregister_token: None,
                    }],
                ))
                .unwrap();
            let appended = heap.allocate(HeapObj::placeholder()).unwrap();
            let garbage = heap.allocate(HeapObj::placeholder()).unwrap();

            for _ in 0..128 {
                heap.collect_incremental(&[owner], 1);
                let state = heap.incremental_mark.lock();
                let state = state.as_ref().unwrap();
                if matches!(state.phase, IncrementalPhase::Retrace { cursor: 1 })
                    && pending_vec_trace(state, owner, VecTraceKind::FinalizationRegistryCells)
                        .is_some_and(|trace| trace.next == 1)
                {
                    break;
                }
            }
            heap.collect_incremental(&[owner], 1);
            heap.with_obj(owner, |object| {
                let HeapObj::FinalizationRegistry(registry) = object else {
                    panic!("registry fixture lost its type");
                };
                registry.cells.lock().push(FinalizationRegistryCell {
                    target: None,
                    held_value: Value::Object(GcIdx(appended)),
                    unregister_token: None,
                });
            });
            assert_eq!(
                heap.incremental_mark.lock().as_ref().unwrap().dirty,
                [owner]
            );
            finish_incremental(&heap, &[owner], 1);

            assert_eq!(heap.live_count(), 2);
            assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
            assert_ne!(heap.allocate(HeapObj::placeholder()).unwrap(), appended);
        }
    }

    #[test]
    fn promise_settlement_style_result_and_drain_is_retraced() {
        let heap = Heap::new();
        let owner = heap
            .allocate(promise(vec![promise_handler(
                Value::Undefined,
                Value::Undefined,
            )]))
            .unwrap();
        let result = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();

        for _ in 0..128 {
            heap.collect_incremental(&[owner], 1);
            let state = heap.incremental_mark.lock();
            let state = state.as_ref().unwrap();
            if matches!(state.phase, IncrementalPhase::Retrace { cursor: 1 })
                && pending_vec_trace(state, owner, VecTraceKind::PromiseHandlers)
                    .is_some_and(|trace| trace.next == 1)
            {
                break;
            }
        }
        heap.collect_incremental(&[owner], 1);
        heap.with_obj(owner, |object| {
            let HeapObj::Promise(promise) = object else {
                panic!("promise fixture lost its type");
            };
            *promise.result.lock() = Value::Object(GcIdx(result));
            let drained: Vec<_> = promise.handlers.lock().drain(..).collect();
            assert_eq!(drained.len(), 1);
        });
        assert_eq!(
            heap.incremental_mark.lock().as_ref().unwrap().dirty,
            [owner]
        );
        finish_incremental(&heap, &[owner], 1);

        assert_eq!(heap.live_count(), 2);
        assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
        assert_ne!(heap.allocate(HeapObj::placeholder()).unwrap(), result);
    }

    #[test]
    fn finalization_cleanup_callback_survives_finite_cursor_tracing() {
        let heap = Heap::new();
        let callback = heap.allocate(HeapObj::placeholder()).unwrap();
        let owner = heap
            .allocate(finalization_registry(
                Value::Object(GcIdx(callback)),
                vec![FinalizationRegistryCell {
                    target: None,
                    held_value: Value::Undefined,
                    unregister_token: None,
                }],
            ))
            .unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();

        heap.collect_incremental(&[owner], 1);
        finish_incremental(&heap, &[owner], 1);

        assert_eq!(heap.live_count(), 2);
        assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
        assert_ne!(heap.allocate(HeapObj::placeholder()).unwrap(), callback);
    }

    #[test]
    fn promise_and_finalization_cursors_charge_removed_slots() {
        let fixtures = [
            (
                promise(vec![
                    promise_handler(Value::Undefined, Value::Undefined),
                    promise_handler(Value::Undefined, Value::Undefined),
                ]),
                VecTraceKind::PromiseHandlers,
            ),
            (
                finalization_registry(
                    Value::Undefined,
                    vec![
                        FinalizationRegistryCell {
                            target: None,
                            held_value: Value::Undefined,
                            unregister_token: None,
                        },
                        FinalizationRegistryCell {
                            target: None,
                            held_value: Value::Undefined,
                            unregister_token: None,
                        },
                    ],
                ),
                VecTraceKind::FinalizationRegistryCells,
            ),
        ];

        for (object, kind) in fixtures {
            let heap = Heap::new();
            let owner = heap.allocate(object).unwrap();
            heap.collect_incremental(&[owner], 1);
            heap.with_obj(owner, |object| match object {
                HeapObj::Promise(promise) => promise.handlers.lock().truncate(1),
                HeapObj::FinalizationRegistry(registry) => registry.cells.lock().truncate(1),
                _ => panic!("cursor fixture lost its type"),
            });
            heap.collect_incremental(&[owner], 1);
            assert_eq!(
                pending_vec_trace(heap.incremental_mark.lock().as_ref().unwrap(), owner, kind)
                    .map(|trace| trace.next),
                Some(1),
                "a removed snapshot slot must still consume one work unit"
            );
        }
    }

    #[test]
    fn promise_handler_cursor_can_be_redirtied() {
        let heap = Heap::new();
        let owner = heap.allocate(promise(Vec::new())).unwrap();
        let original_low = heap.allocate(HeapObj::placeholder()).unwrap();
        let original_high = heap.allocate(HeapObj::placeholder()).unwrap();
        let first_replacement = heap.allocate(HeapObj::placeholder()).unwrap();
        let final_replacement = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
        heap.with_obj(owner, |object| {
            let HeapObj::Promise(promise) = object else {
                panic!("promise fixture lost its type");
            };
            promise.handlers.lock().extend([
                promise_handler(Value::Object(GcIdx(original_low)), Value::Undefined),
                promise_handler(Value::Object(GcIdx(original_high)), Value::Undefined),
            ]);
        });

        for _ in 0..128 {
            heap.collect_incremental(&[owner], 1);
            let state = heap.incremental_mark.lock();
            let state = state.as_ref().unwrap();
            if matches!(state.phase, IncrementalPhase::Retrace { cursor: 1 })
                && pending_vec_trace(state, owner, VecTraceKind::PromiseHandlers)
                    .is_some_and(|trace| trace.next == 2)
            {
                break;
            }
        }
        heap.collect_incremental(&[owner], 1);
        heap.with_obj(owner, |object| {
            let HeapObj::Promise(promise) = object else {
                panic!("promise fixture lost its type");
            };
            promise.handlers.lock()[1].on_fulfilled = Value::Object(GcIdx(first_replacement));
        });
        assert_eq!(
            heap.incremental_mark.lock().as_ref().unwrap().dirty,
            [owner]
        );

        let cells_len = heap.cells.lock().len();
        for _ in 0..256 {
            heap.collect_incremental(&[owner], 1);
            let state = heap.incremental_mark.lock();
            let state = state.as_ref().unwrap();
            if matches!(state.phase, IncrementalPhase::Retrace { cursor } if cursor == cells_len)
                && state.dirty.is_empty()
                && pending_vec_trace(state, owner, VecTraceKind::PromiseHandlers)
                    .is_some_and(|trace| trace.next == 2)
            {
                break;
            }
        }
        heap.collect_incremental(&[owner], 1);
        heap.with_obj(owner, |object| {
            let HeapObj::Promise(promise) = object else {
                panic!("promise fixture lost its type");
            };
            promise.handlers.lock()[1].on_fulfilled = Value::Object(GcIdx(final_replacement));
        });
        assert_eq!(
            heap.incremental_mark.lock().as_ref().unwrap().dirty,
            [owner]
        );
        finish_incremental(&heap, &[owner], 1);

        assert_eq!(heap.live_count(), 5);
        assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
        assert_ne!(
            heap.allocate(HeapObj::placeholder()).unwrap(),
            final_replacement
        );
    }

    #[test]
    fn finalization_cursor_traces_only_held_values() {
        let heap = Heap::new();
        let target = heap.allocate(HeapObj::placeholder()).unwrap();
        let held = heap.allocate(HeapObj::placeholder()).unwrap();
        let token = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
        let owner = heap
            .allocate(finalization_registry(
                Value::Undefined,
                vec![FinalizationRegistryCell {
                    target: Some(Value::Object(GcIdx(target))),
                    held_value: Value::Object(GcIdx(held)),
                    unregister_token: Some(Value::Object(GcIdx(token))),
                }],
            ))
            .unwrap();

        heap.collect_incremental(&[owner], 1);
        finish_incremental(&heap, &[owner], 1);

        assert_eq!(heap.live_count(), 2);
        assert!(heap.with_obj(owner, |object| {
            let HeapObj::FinalizationRegistry(registry) = object else {
                return false;
            };
            let cells = registry.cells.lock();
            cells[0].target.is_none()
                && cells[0].unregister_token.is_none()
                && cells[0].held_value == Value::Object(GcIdx(held))
        }));
        let reclaimed: std::collections::HashSet<_> = (0..3)
            .map(|_| heap.allocate(HeapObj::placeholder()).unwrap())
            .collect();
        assert_eq!(reclaimed, [target, token, garbage].into_iter().collect());
    }

    #[test]
    fn usize_max_uses_direct_promise_and_finalization_traces() {
        let heap = Heap::new();
        let promise_root = heap.allocate(HeapObj::placeholder()).unwrap();
        let registry_root = heap.allocate(HeapObj::placeholder()).unwrap();
        let promise = heap
            .allocate(promise(vec![promise_handler(
                Value::Object(GcIdx(promise_root)),
                Value::Undefined,
            )]))
            .unwrap();
        let registry = heap
            .allocate(finalization_registry(
                Value::Undefined,
                vec![FinalizationRegistryCell {
                    target: None,
                    held_value: Value::Object(GcIdx(registry_root)),
                    unregister_token: None,
                }],
            ))
            .unwrap();

        let cells = heap.cells.lock();
        let promise_trace = trace_cell(&cells, promise, false);
        let registry_trace = trace_cell(&cells, registry, false);
        assert!(promise_trace.vector.is_none());
        assert!(registry_trace.vector.is_none());
        assert_eq!(promise_trace.before, [promise_root]);
        assert_eq!(registry_trace.before, [usize::MAX, registry_root]);
    }

    #[test]
    fn promise_cursor_preserves_fixed_vector_private_lifo_order() {
        let heap = Heap::new();
        let fixed = heap.allocate(HeapObj::placeholder()).unwrap();
        let slot = heap.allocate(HeapObj::placeholder()).unwrap();
        let private = heap.allocate(HeapObj::placeholder()).unwrap();
        let owner = heap
            .allocate(promise(vec![promise_handler(
                Value::Object(GcIdx(slot)),
                Value::Undefined,
            )]))
            .unwrap();
        heap.with_obj(owner, |object| {
            let HeapObj::Promise(promise) = object else {
                panic!("promise fixture lost its type");
            };
            *promise.result.lock() = Value::Object(GcIdx(fixed));
        });
        heap.with_private_elements(owner, |elements| {
            elements.insert(
                PrivateSlotKey::Private(PrivateNameKey {
                    id: 7,
                    description: Arc::from("cursor-order"),
                }),
                PrivateSlot::Value(Value::Object(GcIdx(private))),
            );
        });

        heap.collect_incremental(&[owner], 1);
        assert_eq!(
            heap.incremental_mark
                .lock()
                .as_ref()
                .unwrap()
                .worklist
                .last(),
            Some(&TraceWork::Cell(private))
        );
        heap.collect_incremental(&[owner], 1);
        assert_eq!(
            pending_vec_trace(
                heap.incremental_mark.lock().as_ref().unwrap(),
                owner,
                VecTraceKind::PromiseHandlers,
            )
            .map(|trace| trace.next),
            Some(1)
        );
        heap.collect_incremental(&[owner], 1);
        let state = heap.incremental_mark.lock();
        let state = state.as_ref().unwrap();
        assert_eq!(state.worklist.last(), Some(&TraceWork::Cell(slot)));
        assert!(state.worklist.contains(&TraceWork::Cell(fixed)));
    }

    #[test]
    fn finalization_retain_compaction_is_retraced() {
        let heap = Heap::new();
        let owner = heap
            .allocate(finalization_registry(Value::Undefined, Vec::new()))
            .unwrap();
        let low = heap.allocate(HeapObj::placeholder()).unwrap();
        let middle = heap.allocate(HeapObj::placeholder()).unwrap();
        let high = heap.allocate(HeapObj::placeholder()).unwrap();
        let late = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
        heap.with_obj(owner, |object| {
            let HeapObj::FinalizationRegistry(registry) = object else {
                panic!("registry fixture lost its type");
            };
            registry.cells.lock().extend([
                FinalizationRegistryCell {
                    target: None,
                    held_value: Value::Object(GcIdx(low)),
                    unregister_token: None,
                },
                FinalizationRegistryCell {
                    target: None,
                    held_value: Value::Object(GcIdx(middle)),
                    unregister_token: None,
                },
                FinalizationRegistryCell {
                    target: None,
                    held_value: Value::Object(GcIdx(high)),
                    unregister_token: None,
                },
            ]);
        });

        for _ in 0..128 {
            heap.collect_incremental(&[owner], 1);
            let state = heap.incremental_mark.lock();
            let state = state.as_ref().unwrap();
            if matches!(state.phase, IncrementalPhase::Retrace { cursor: 1 })
                && pending_vec_trace(state, owner, VecTraceKind::FinalizationRegistryCells)
                    .is_some_and(|trace| trace.next == 3)
            {
                break;
            }
        }
        heap.collect_incremental(&[owner], 1);
        heap.with_obj(owner, |object| {
            let HeapObj::FinalizationRegistry(registry) = object else {
                panic!("registry fixture lost its type");
            };
            let mut cells = registry.cells.lock();
            cells[0].held_value = Value::Object(GcIdx(late));
            cells.retain(|cell| cell.held_value != Value::Object(GcIdx(middle)));
        });
        assert_eq!(
            heap.incremental_mark.lock().as_ref().unwrap().dirty,
            [owner]
        );
        finish_incremental(&heap, &[owner], 1);

        assert_eq!(heap.live_count(), 5);
        assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
        assert_ne!(heap.allocate(HeapObj::placeholder()).unwrap(), late);
    }

    #[test]
    fn usize_max_completes_pending_promise_and_finalization_cursors() {
        let fixtures = [
            (
                promise(vec![
                    promise_handler(Value::Undefined, Value::Undefined),
                    promise_handler(Value::Undefined, Value::Undefined),
                ]),
                VecTraceKind::PromiseHandlers,
            ),
            (
                finalization_registry(
                    Value::Undefined,
                    vec![
                        FinalizationRegistryCell {
                            target: None,
                            held_value: Value::Undefined,
                            unregister_token: None,
                        },
                        FinalizationRegistryCell {
                            target: None,
                            held_value: Value::Undefined,
                            unregister_token: None,
                        },
                    ],
                ),
                VecTraceKind::FinalizationRegistryCells,
            ),
        ];

        for (object, kind) in fixtures {
            let heap = Heap::new();
            let owner = heap.allocate(object).unwrap();
            heap.collect_incremental(&[owner], 1);
            assert_eq!(
                pending_vec_trace(heap.incremental_mark.lock().as_ref().unwrap(), owner, kind)
                    .map(|trace| trace.next),
                Some(2)
            );
            heap.collect_incremental(&[owner], usize::MAX);
            assert!(heap.incremental_mark.lock().is_none());
            assert_eq!(heap.live_count(), 1);
        }
    }

    #[test]
    fn dirty_array_trace_can_be_redirtied() {
        let heap = Heap::new();
        let owner = heap
            .allocate(HeapObj::Array(crate::value::ArrayData::new(
                Vec::new(),
                None,
            )))
            .unwrap();
        let original_low = heap.allocate(HeapObj::placeholder()).unwrap();
        let original_high = heap.allocate(HeapObj::placeholder()).unwrap();
        let first_replacement = heap.allocate(HeapObj::placeholder()).unwrap();
        let final_replacement = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();
        heap.with_obj(owner, |object| {
            let HeapObj::Array(array) = object else {
                panic!("array fixture lost its type");
            };
            array.items.lock().extend([
                Value::Object(GcIdx(original_low)),
                Value::Object(GcIdx(original_high)),
            ]);
        });

        for _ in 0..128 {
            heap.collect_incremental(&[owner], 1);
            let state = heap.incremental_mark.lock();
            let state = state.as_ref().unwrap();
            if matches!(state.phase, IncrementalPhase::Retrace { cursor: 1 })
                && pending_vec_trace(state, owner, VecTraceKind::ArrayItems)
                    .is_some_and(|trace| trace.next == 2)
            {
                break;
            }
        }
        heap.collect_incremental(&[owner], 1);
        heap.with_obj(owner, |object| {
            let HeapObj::Array(array) = object else {
                panic!("array fixture lost its type");
            };
            array.items.lock()[1] = Value::Object(GcIdx(first_replacement));
        });
        assert_eq!(
            heap.incremental_mark.lock().as_ref().unwrap().dirty,
            [owner]
        );

        let cells_len = heap.cells.lock().len();
        let mut dirty_trace_started = false;
        for _ in 0..256 {
            heap.collect_incremental(&[owner], 1);
            let state = heap.incremental_mark.lock();
            let state = state.as_ref().unwrap();
            if matches!(state.phase, IncrementalPhase::Retrace { cursor } if cursor == cells_len)
                && state.dirty.is_empty()
                && pending_vec_trace(state, owner, VecTraceKind::ArrayItems)
                    .is_some_and(|trace| trace.next == 2)
            {
                dirty_trace_started = true;
                break;
            }
        }
        assert!(dirty_trace_started, "dirty Array trace must begin");
        heap.collect_incremental(&[owner], 1);
        heap.with_obj(owner, |object| {
            let HeapObj::Array(array) = object else {
                panic!("array fixture lost its type");
            };
            array.items.lock()[1] = Value::Object(GcIdx(final_replacement));
        });
        assert_eq!(
            heap.incremental_mark.lock().as_ref().unwrap().dirty,
            [owner],
            "a mutation during a dirty trace must schedule another full pass"
        );
        finish_incremental(&heap, &[owner], 1);

        assert_eq!(heap.live_count(), 5);
        let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(fresh, garbage);
        assert_ne!(fresh, final_replacement);
    }

    #[test]
    fn active_reentrant_mutation_after_inner_slice_survives() {
        let heap = Heap::new();
        let owner = heap.allocate(HeapObj::placeholder()).unwrap();
        let child = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();

        heap.collect_incremental(&[owner], 1);
        heap.with_obj(owner, |object| {
            heap.collect_incremental(&[owner], 1);
            assert!(matches!(
                heap.incremental_mark
                    .lock()
                    .as_ref()
                    .map(|state| state.phase),
                Some(IncrementalPhase::Retrace { cursor: 1 })
            ));
            object.props().lock().insert(
                PropertyKey::from("late"),
                PropertyDescriptor::data(Value::Object(GcIdx(child))),
            );
            heap.collect_incremental(&[], usize::MAX);
        });

        assert_eq!(heap.live_count(), 2);
        let fresh = heap.allocate(HeapObj::placeholder()).unwrap();
        assert_eq!(fresh, garbage);
        assert_ne!(fresh, child);
    }

    #[test]
    fn active_object_unwind_runs_the_incremental_post_barrier() {
        let heap = Heap::new();
        let owner = heap.allocate(HeapObj::placeholder()).unwrap();
        let child = heap.allocate(HeapObj::placeholder()).unwrap();
        let garbage = heap.allocate(HeapObj::placeholder()).unwrap();

        heap.collect_incremental(&[owner], 1);
        heap.collect_incremental(&[owner], 1);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            heap.with_obj(owner, |object| {
                object.props().lock().insert(
                    PropertyKey::from("late"),
                    PropertyDescriptor::data(Value::Object(GcIdx(child))),
                );
                panic!("host callback panic");
            });
        }));
        assert!(result.is_err());
        assert_eq!(
            heap.incremental_mark.lock().as_ref().unwrap().dirty,
            [owner]
        );

        finish_incremental(&heap, &[owner], 1);
        assert_eq!(heap.live_count(), 2);
        assert_eq!(heap.allocate(HeapObj::placeholder()).unwrap(), garbage);
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
