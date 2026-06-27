//! Shared test helpers for the RuJa integration test suite.

use ruja::{Value, Vm};

/// Run a source string and return the value of the last top-level expression,
/// substituting `undefined` if evaluation errors out.
pub fn run(src: &str) -> Value {
    let mut vm = Vm::new();
    vm.run(src).unwrap_or(Value::Undefined)
}
