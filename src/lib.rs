#![allow(unreachable_patterns)]
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
