//! RuJa — a small bytecode-VM JavaScript engine in Rust.
//!
//! The public embedding API is [`Vm`], [`Value`], [`Compiler`], [`Lexer`],
//! [`Parser`], [`Error`], and [`GcIdx`]/[`HeapObj`] (see the re-exports below).
//!
//! The remaining modules (`ast`, `bytecode`, `compiler`, `environment`,
//! `gc`, `lexer`, `parser`, `token`, `value`, `vm`) are exposed as `pub`
//! for testing and inspection, but their internals are **not** part of the
//! semver-stable API: names, fields, and shapes may change between releases
//! (RuJa is `0.x` and not yet on crates.io). Embed against the re-exports,
//! not the module internals.

pub mod ast;
pub mod bytecode;
pub mod compiler;
pub mod environment;
pub mod error;
pub mod function;
pub mod gc;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod value;
pub mod vm;
// builtins is an internal bootstrap module; not part of the public API.
mod builtins;

pub use compiler::Compiler;
pub use error::{Error, ErrorKind};
pub use lexer::Lexer;
pub use parser::Parser;
pub use value::{GcIdx, HeapObj, Value};
pub use vm::{NativeFn, Vm};
