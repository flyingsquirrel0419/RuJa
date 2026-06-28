//! Shared function definition used by both compiler and VM.

use crate::bytecode::Chunk;
use std::rc::Rc;

#[derive(Clone)]
pub struct FunctionDef {
    pub name: Option<Rc<str>>,
    pub params: Vec<Rc<str>>,
    /// Local slot for each parameter (params may share a slot when a
    /// non-strict function has duplicate parameter names; the last value
    /// wins). Falls back to `i` when empty (legacy callers).
    pub param_slots: Vec<usize>,
    pub rest_param: Option<Rc<str>>,
    pub chunk: Rc<Chunk>,
    pub num_locals: usize,
    pub is_arrow: bool,
    pub is_async: bool,
    pub is_generator: bool,
}
