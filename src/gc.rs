//! Mark-and-sweep garbage collector.
//!
//! Heap objects are `HeapObj` (an enum) stored in cells. A `GcIdx` handle
//! (index into the cell array) is how the VM references them. The collector
//! traces from roots and sweeps unreachable cells.

use crate::value::HeapObj;
use std::cell::{Cell, RefCell};

pub struct GcCell {
    pub obj: RefCell<Option<HeapObj>>,
    pub marked: Cell<bool>,
}

pub struct Heap {
    pub cells: RefCell<Vec<GcCell>>,
    free_list: RefCell<Vec<usize>>,
    alloc_since_gc: Cell<usize>,
    gc_threshold: Cell<usize>,
}

/// Marker passed during the trace phase.
pub struct Marker<'a> {
    heap: &'a Heap,
    marked: &'a RefCell<Vec<bool>>,
}

impl<'a> Marker<'a> {
    pub fn mark_cell(&mut self, idx: usize) {
        let already = {
            let m = self.marked.borrow();
            idx >= m.len() || m[idx]
        };
        if already {
            return;
        }
        self.marked.borrow_mut()[idx] = true;
        // trace in place without cloning
        let cells = self.heap.cells.borrow();
        if let Some(cell) = cells.get(idx) {
            let obj_ref = cell.obj.borrow();
            if let Some(obj) = obj_ref.as_ref() {
                trace_obj(obj, self);
            }
        }
    }

    pub fn mark_value(&mut self, v: &crate::value::Value) {
        if let crate::value::Value::Object(idx) = v {
            self.mark_cell(idx.0);
        }
    }

    pub fn mark_idx(&mut self, idx: crate::value::GcIdx) {
        self.mark_cell(idx.0);
    }

    pub fn mark_values(&mut self, vals: &[crate::value::Value]) {
        for v in vals {
            self.mark_value(v);
        }
    }
}

/// Trace a HeapObj's children. Free function to keep value.rs clean.
pub fn trace_obj(obj: &HeapObj, marker: &mut Marker) {
    if let HeapObj::Iterator(it) = obj {
        for v in it.items.borrow().iter() {
            marker.mark_value(v);
        }
        return;
    }
    let props = obj.props();
    for (_, desc) in props.borrow().iter() {
        if !desc.is_accessor {
            marker.mark_value(&desc.value);
        } else {
            if let Some(g) = &desc.get {
                marker.mark_value(g);
            }
            if let Some(s) = &desc.set {
                marker.mark_value(s);
            }
        }
    }
    if let Some(proto) = obj.proto().borrow().as_ref() {
        marker.mark_value(proto);
    }
    match obj {
        HeapObj::Array(a) => {
            for v in a.items.borrow().iter() {
                marker.mark_value(v);
            }
        }
        HeapObj::Function(f) => {
            marker.mark_idx(f.closure);
            if let Some(p) = f.prototype.borrow().as_ref() {
                marker.mark_value(p);
            }
            if let crate::value::FunctionKind::Bound {
                target,
                this_val,
                bound_args,
            } = &f.kind
            {
                marker.mark_idx(*target);
                marker.mark_value(this_val);
                for a in bound_args {
                    marker.mark_value(a);
                }
            }
        }
        HeapObj::Environment(e) => {
            for (_, b) in e.vars.borrow().iter() {
                marker.mark_value(&b.value.borrow());
            }
            if let Some(p) = *e.parent.borrow() {
                marker.mark_idx(p);
            }
        }
        HeapObj::Map(m) => {
            for (k, v) in m.entries.borrow().iter() {
                marker.mark_value(k);
                marker.mark_value(v);
            }
        }
        HeapObj::Set(s) => {
            for v in s.items.borrow().iter() {
                marker.mark_value(v);
            }
        }
        HeapObj::Promise(p) => {
            marker.mark_value(&p.result.borrow());
            for h in p.handlers.borrow().iter() {
                marker.mark_value(&h.on_fulfilled);
                marker.mark_value(&h.on_rejected);
            }
        }
        HeapObj::Generator(g) => {
            marker.mark_idx(g.closure);
            for v in g.state.borrow().iter() {
                marker.mark_value(v);
            }
        }
        HeapObj::Iterator(it) => {
            for v in it.items.borrow().iter() {
                marker.mark_value(v);
            }
        }
        HeapObj::Object(_) => {}
    }
}

impl Heap {
    pub fn new() -> Self {
        Heap {
            cells: RefCell::new(Vec::new()),
            free_list: RefCell::new(Vec::new()),
            alloc_since_gc: Cell::new(0),
            gc_threshold: Cell::new(1024),
        }
    }

    pub fn allocate(&self, obj: HeapObj) -> usize {
        let idx = {
            let mut free = self.free_list.borrow_mut();
            if let Some(idx) = free.pop() {
                let cells = self.cells.borrow_mut();
                *cells[idx].obj.borrow_mut() = Some(obj);
                cells[idx].marked.set(false);
                idx
            } else {
                let mut cells = self.cells.borrow_mut();
                let idx = cells.len();
                cells.push(GcCell {
                    obj: RefCell::new(Some(obj)),
                    marked: Cell::new(false),
                });
                idx
            }
        };
        self.alloc_since_gc.set(self.alloc_since_gc.get() + 1);
        idx
    }

    pub fn collect(&self, roots: &[usize]) {
        let cells_len = self.cells.borrow().len();
        let marked = RefCell::new(vec![false; cells_len]);
        {
            let mut marker = Marker {
                heap: self,
                marked: &marked,
            };
            for &root in roots {
                marker.mark_cell(root);
            }
        }
        let mut free = self.free_list.borrow_mut();
        let m = marked.borrow();
        let mut cells = self.cells.borrow_mut();
        for (idx, cell) in cells.iter_mut().enumerate() {
            if !m[idx] && cell.obj.borrow().is_some() {
                *cell.obj.borrow_mut() = None;
                free.push(idx);
            }
        }
        self.alloc_since_gc.set(0);
        let live = cells.len() - free.len();
        self.gc_threshold.set((live * 2).max(1024));
    }

    pub fn maybe_collect(&self, roots: &[usize]) {
        if self.alloc_since_gc.get() >= self.gc_threshold.get() {
            self.collect(roots);
        }
    }

    pub fn live_count(&self) -> usize {
        self.cells.borrow().len() - self.free_list.borrow().len()
    }

    pub fn with_obj<R>(&self, idx: usize, f: impl FnOnce(&HeapObj) -> R) -> R {
        let cells = self.cells.borrow();
        let cell = &cells[idx];
        let obj_ref = cell.obj.borrow();
        let obj = obj_ref.as_ref().expect("use after free");
        f(obj)
    }
}
