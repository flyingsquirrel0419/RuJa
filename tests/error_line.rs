//! Runtime error source-line reporting (`(at line N)`).

mod common;
use common::run;
use ruja::Value;

fn run_err_msg(src: &str) -> String {
    let mut vm = ruja::Vm::new();
    match vm.run(src) {
        Err(e) => e.to_string(),
        Ok(v) => panic!("expected error, got value: {:?}", v),
    }
}

#[test]
fn reference_error_reports_line() {
    let msg = run_err_msg("undefinedVar;");
    assert!(msg.contains("(at line 1)"), "got: {}", msg);
}

#[test]
fn reference_error_reports_correct_line() {
    let msg = run_err_msg("\n\n\nundefinedVar;");
    assert!(msg.contains("(at line 4)"), "got: {}", msg);
}

#[test]
fn type_error_reports_line() {
    let msg = run_err_msg("var o = undefined;\no.foo;");
    assert!(msg.contains("TypeError"), "got: {}", msg);
    assert!(msg.contains("(at line 2)"), "got: {}", msg);
}

#[test]
fn error_caught_still_has_line_in_message() {
    // When caught and re-thrown/stringified, the line is preserved on the Error.
    let src = r#"
        try { undefinedVar; } catch(e) { throw e; }
    "#;
    let msg = run_err_msg(src);
    assert!(msg.contains("undefinedVar"), "got: {}", msg);
    assert!(msg.contains("(at line"), "got: {}", msg);
}

#[test]
fn no_line_for_synthetic_one_liner_zero_is_ok() {
    // A program that errors on the first line reports line 1, not 0.
    let msg = run_err_msg("null.x");
    assert!(msg.contains("(at line 1)"), "got: {}", msg);
    // sanity: the value path (no error) still works
    assert_eq!(run("1 + 2;"), Value::Number(3.0));
}
