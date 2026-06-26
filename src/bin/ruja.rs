use ruja::{Vm, Value};
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::process::exit;

const VERSION: &str = "2.0.0-alpha";
const HELP: &str = r#"Usage: ruja [OPTIONS] [FILE]

A JavaScript engine written in Rust (v2.0 bytecode VM).

Arguments:
  FILE                JavaScript file to execute. If omitted, starts REPL.

Options:
  -e, --eval <CODE>   Evaluate CODE and print the result
  -h, --help          Print this help message
  -V, --version       Print version information

Examples:
  ruja script.js          Run a JavaScript file
  ruja -e "1 + 2 * 3"     Evaluate an expression
  ruja                    Start the interactive REPL
"#;

fn print_value(vm: &mut Vm, v: &Value) {
    match v {
        Value::Undefined => {}
        Value::Null => println!("null"),
        Value::Bool(b) => println!("{}", b),
        Value::Number(n) => println!("{}", ruja::value::num_to_string(*n)),
        Value::String(s) => println!("{}", s),
        Value::Object(_) | Value::Symbol(_) => {
            match vm.to_string(v) {
                Ok(s) => println!("{}", s),
                Err(_) => println!("[object Object]"),
            }
        }
    }
}

fn run_file(path: &str) -> i32 {
    match fs::read_to_string(path) {
        Ok(src) => {
            let mut vm = Vm::new();
            match vm.run(&src) {
                Ok(_) => 0,
                Err(e) => { eprintln!("{}", e); 1 }
            }
        }
        Err(e) => { eprintln!("ruja: cannot read '{}': {}", path, e); 1 }
    }
}

fn run_eval(code: &str) -> i32 {
    let mut vm = Vm::new();
    match vm.run(code) {
        Ok(v) => { print_value(&mut vm, &v); 0 }
        Err(e) => { eprintln!("{}", e); 1 }
    }
}

fn repl() -> i32 {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut buffer = String::new();
    let mut in_block = false;
    println!("RuJa v{} - JavaScript REPL (Ctrl+C to exit)", VERSION);
    loop {
        let prompt = if in_block { "  ... " } else { "ruja> " };
        print!("{}", prompt);
        if stdout.flush().is_err() { break; }
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => { println!(); break; }
            Ok(_) => {}
            Err(_) => break,
        }
        buffer.push_str(&line);
        let opens = buffer.matches('{').count();
        let closes = buffer.matches('}').count();
        in_block = opens > closes;
        if in_block { continue; }
        let trimmed = buffer.trim();
        if trimmed.is_empty() { buffer.clear(); continue; }
        if trimmed == ".exit" || trimmed == ".quit" { break; }
        let mut vm = Vm::new();
        match vm.run(&buffer) {
            Ok(v) => { if !v.is_undefined() { print_value(&mut vm, &v); } }
            Err(e) => eprintln!("{}", e),
        }
        buffer.clear();
    }
    0
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() == 1 { exit(repl()); }
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => { print!("{}", HELP); exit(0); }
            "-V" | "--version" => { println!("ruja {}", VERSION); exit(0); }
            "-e" | "--eval" => {
                if i + 1 >= args.len() { eprintln!("ruja: -e requires an argument"); exit(2); }
                exit(run_eval(&args[i + 1]));
            }
            arg if arg.starts_with('-') => {
                eprintln!("ruja: unknown option '{}'", arg); exit(2);
            }
            file => { exit(run_file(file)); }
        }
        i += 1;
    }
    exit(repl());
}
