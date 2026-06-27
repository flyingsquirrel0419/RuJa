#![allow(unreachable_patterns)]
#![allow(dead_code)]
pub mod ast;
pub mod builtins;
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

pub use compiler::Compiler;
pub use lexer::Lexer;
pub use parser::Parser;
pub use value::Value;
pub use vm::Vm;
