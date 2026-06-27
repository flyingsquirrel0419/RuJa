//! Shared test helpers for the RuJa integration test suite.

use ruja::{Value, Vm};

/// Run a source string and return the value of the last top-level expression,
/// substituting `undefined` if evaluation errors out.
pub fn run(src: &str) -> Value {
    let mut vm = Vm::new();
    vm.run(src).unwrap_or(Value::Undefined)
}

/// Run a source string that is expected to error at runtime. Returns the
/// error message (a Rust `String`). Panics if evaluation succeeds.
#[allow(dead_code)]
pub fn run_err(src: &str) -> String {
    let mut vm = Vm::new();
    match vm.run(src) {
        Err(e) => e.to_string(),
        Ok(v) => panic!("expected error, got value: {:?}", v),
    }
}
