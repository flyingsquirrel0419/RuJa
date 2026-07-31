//! Shared function definition used by both compiler and VM.

use crate::bytecode::Chunk;
use std::sync::Arc;

#[derive(Clone)]
pub struct FunctionDef {
    /// Exact source text returned by Function.prototype.toString for parsed
    /// ECMAScript functions. Built-ins and bound functions leave this absent.
    pub source: Option<Arc<str>>,
    pub name: Option<Arc<str>>,
    pub params: Vec<Arc<str>>,
    /// Raw argument local slot for each formal parameter. Duplicate
    /// non-strict parameter names keep distinct raw slots so a later omitted
    /// formal still initializes the shared parameter binding to `undefined`.
    /// Falls back to `i` when empty (legacy callers).
    pub param_slots: Vec<usize>,
    pub rest_param: Option<Arc<str>>,
    pub chunk: Arc<Chunk>,
    pub num_locals: usize,
    pub is_arrow: bool,
    pub is_async: bool,
    pub is_generator: bool,
    /// Whether the formal parameter list has defaults/rest/destructuring
    /// semantics that require a distinct body variable environment.
    pub has_parameter_expressions: bool,
    /// ES function `length`: number of params before the first default or
    /// the rest parameter.
    pub length: usize,
    /// True for object/class methods (enables super property access).
    pub is_method: bool,
    /// Explicit named function expressions carry an immutable inner name
    /// binding in their closure environment. Declarations and inferred display
    /// names do not.
    pub has_name_binding: bool,
    pub is_derived: bool,
}
