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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

pub struct GcCell {
    pub obj: Mutex<Option<HeapObj>>,
    pub marked: AtomicBool,
}

pub struct Heap {
    pub cells: Mutex<Vec<GcCell>>,
    free_list: Mutex<Vec<usize>>,
    alloc_since_gc: AtomicUsize,
    gc_threshold: AtomicUsize,
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
        for v in it.items.lock().unwrap().iter() {
            push_value(v, worklist);
        }
        if let Some(lazy) = it.lazy_iter.lock().unwrap().as_ref() {
            push_value(lazy, worklist);
        }
        return;
    }
    if let HeapObj::Environment(e) = obj {
        for (_, b) in e.vars.lock().unwrap().iter() {
            push_value(&b.value.lock().unwrap(), worklist);
        }
        if let Some(p) = *e.parent.lock().unwrap() {
            worklist.push(p.0);
        }
        return;
    }
    let props = obj.props();
    for (_, desc) in props.lock().unwrap().iter() {
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
    if let Some(proto) = obj.proto().lock().unwrap().as_ref() {
        push_value(proto, worklist);
    }
    match obj {
        HeapObj::Array(a) => {
            for v in a.items.lock().unwrap().iter() {
                push_value(v, worklist);
            }
        }
        HeapObj::Function(f) => {
            worklist.push(f.closure.0);
            if let Some(p) = f.prototype.lock().unwrap().as_ref() {
                push_value(p, worklist);
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
            for (_, b) in e.vars.lock().unwrap().iter() {
                push_value(&b.value.lock().unwrap(), worklist);
            }
            if let Some(p) = *e.parent.lock().unwrap() {
                worklist.push(p.0);
            }
        }
        HeapObj::Map(m) => {
            for (k, v) in m.entries.lock().unwrap().iter() {
                push_value(k, worklist);
                push_value(v, worklist);
            }
        }
        HeapObj::WeakMap(wm) => {
            for (key_idx, v) in wm.entries.lock().unwrap().iter() {
                if *key_idx < marked.len() && marked[*key_idx] {
                    push_value(v, worklist);
                }
            }
        }
        HeapObj::WeakSet(_) => {}
        HeapObj::Set(s) => {
            for v in s.items.lock().unwrap().iter() {
                push_value(v, worklist);
            }
        }
        HeapObj::Promise(p) => {
            push_value(&p.result.lock().unwrap(), worklist);
            for h in p.handlers.lock().unwrap().iter() {
                push_value(&h.on_fulfilled, worklist);
                push_value(&h.on_rejected, worklist);
            }
        }
        HeapObj::Generator(g) => {
            worklist.push(g.closure.0);
            for v in g.state.lock().unwrap().iter() {
                push_value(v, worklist);
            }
        }
        HeapObj::LazyGenerator(g) => {
            worklist.push(g.closure.0);
            for v in g.stack.lock().unwrap().iter() {
                push_value(v, worklist);
            }
            for v in g.locals.lock().unwrap().iter() {
                push_value(v, worklist);
            }
            push_value(&g.resume_value.lock().unwrap(), worklist);
            for v in g.args.lock().unwrap().iter() {
                push_value(v, worklist);
            }
            push_value(&g.this_val.lock().unwrap(), worklist);
        }
        HeapObj::Iterator(it) => {
            for v in it.items.lock().unwrap().iter() {
                push_value(v, worklist);
            }
            if let Some(lazy) = it.lazy_iter.lock().unwrap().as_ref() {
                push_value(lazy, worklist);
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
        }
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

impl Heap {
    pub fn allocate(&self, obj: HeapObj) -> usize {
        let idx = {
            let mut free = self.free_list.lock().unwrap();
            if let Some(idx) = free.pop() {
                let cells = self.cells.lock().unwrap();
                *cells[idx].obj.lock().unwrap() = Some(obj);
                cells[idx].marked.store(false, Ordering::Relaxed);
                idx
            } else {
                let mut cells = self.cells.lock().unwrap();
                let idx = cells.len();
                cells.push(GcCell {
                    obj: Mutex::new(Some(obj)),
                    marked: AtomicBool::new(false),
                });
                idx
            }
        };
        self.alloc_since_gc.fetch_add(1, Ordering::Relaxed);
        idx
    }

    pub fn collect(&self, roots: &[usize]) {
        let cells_len = self.cells.lock().unwrap().len();
        let mut marked = vec![false; cells_len];
        let mut worklist: Vec<usize> = roots.to_vec();
        // Iterate the worklist to a fixed point. Ephemeron (WeakMap) values
        // are only marked once their key is marked, so a value reachable only
        // through a WeakMap may need several passes.
        let mut changed = true;
        while changed {
            changed = false;
            // Drain the worklist, marking newly-reachable indices and
            // collecting their children. The cells mutex is held only for the
            // brief window of extracting an object's children.
            while let Some(idx) = worklist.pop() {
                if idx >= cells_len || marked[idx] {
                    continue;
                }
                marked[idx] = true;
                changed = true;
                // Extract this object's children without holding the cells
                // lock during the recursive trace.
                let children: Vec<usize> = {
                    let cells = self.cells.lock().unwrap();
                    if let Some(cell) = cells.get(idx) {
                        let obj_ref = cell.obj.lock().unwrap();
                        if let Some(obj) = obj_ref.as_ref() {
                            let mut w = Vec::new();
                            trace_obj(obj, &marked, &mut w);
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
        }
        // Sweep: free unmarked cells.
        let mut free = self.free_list.lock().unwrap();
        let mut cells = self.cells.lock().unwrap();
        for (idx, cell) in cells.iter_mut().enumerate() {
            if !marked[idx] && cell.obj.lock().unwrap().is_some() {
                *cell.obj.lock().unwrap() = None;
                free.push(idx);
            }
        }
        // Sweep dead entries from WeakMap/WeakSet.
        for cell in cells.iter() {
            let obj_ref = cell.obj.lock().unwrap();
            if let Some(obj) = obj_ref.as_ref() {
                match obj {
                    HeapObj::WeakMap(wm) => {
                        wm.entries
                            .lock()
                            .unwrap()
                            .retain(|(k, _)| *k < marked.len() && marked[*k]);
                    }
                    HeapObj::WeakSet(ws) => {
                        ws.items
                            .lock()
                            .unwrap()
                            .retain(|k| *k < marked.len() && marked[*k]);
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

    pub fn live_count(&self) -> usize {
        // Lock order must match `allocate` (free_list before cells) to avoid a
        // lock-order inversion deadlock if both are ever held concurrently.
        let free = self.free_list.lock().unwrap();
        let cells = self.cells.lock().unwrap();
        cells.len() - free.len()
    }

    pub fn with_obj<R>(&self, idx: usize, f: impl FnOnce(&HeapObj) -> R) -> R {
        // Take the object out of the cell so the cells mutex can be released
        // before running `f`. This prevents re-entrant locking of the cells
        // mutex (e.g. when `f` allocates or triggers a GC) from deadlocking.
        // The object is put back after `f` returns.
        let obj = {
            let cells = self.cells.lock().unwrap();
            let cell = &cells[idx];
            let mut slot = cell.obj.lock().unwrap();
            slot.take().expect("use after free")
        };
        let result = f(&obj);
        {
            let cells = self.cells.lock().unwrap();
            let cell = &cells[idx];
            *cell.obj.lock().unwrap() = Some(obj);
        }
        result
    }
}
