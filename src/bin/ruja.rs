use ruja::{Value, Vm};
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::process::exit;
use std::thread;

const VERSION: &str = "0.3.0-alpha";
const HELP: &str = r#"Usage: ruja [OPTIONS] [FILE]

A JavaScript engine written in Rust (bytecode VM + GC).

Arguments:
  FILE                JavaScript file to execute. If omitted, starts REPL.

Options:
  -e, --eval <CODE>   Evaluate CODE and print the result
      --module <FILE> Evaluate FILE using the ECMAScript Module source goal
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
        Value::BigInt(n) => println!("{}n", n),
        Value::String(s) => println!("{}", s),
        Value::Object(_) => match vm.to_string_pub(v) {
            Ok(s) => println!("{}", s),
            Err(_) => println!("[object Object]"),
        },
        Value::Symbol(_) => println!("Symbol()"),
        Value::PrivateName(key) => println!("[private #{}]", key.description),
        Value::Reference(_) => println!("[reference]"),
    }
}

fn new_vm() -> Vm {
    let mut vm = Vm::new().expect("failed to initialize VM");
    if let Ok(value) = env::var("RUJA_AGENT_CAN_BLOCK") {
        vm.set_agent_can_block(value == "1" || value.eq_ignore_ascii_case("true"));
    }
    vm
}

fn run_file(path: &str, module: bool) -> i32 {
    match fs::read_to_string(path) {
        Ok(src) => {
            let mut vm = new_vm();
            let result = if module {
                vm.run_module(&src)
            } else {
                vm.run(&src)
            };
            match result {
                Ok(_) => match vm.run_external_jobs_until_idle() {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("{}", e);
                        1
                    }
                },
                Err(e) => {
                    eprintln!("{}", e);
                    1
                }
            }
        }
        Err(e) => {
            eprintln!("ruja: cannot read '{}': {}", path, e);
            1
        }
    }
}

fn run_eval(code: &str) -> i32 {
    let mut vm = new_vm();
    match vm.run(code) {
        Ok(v) => {
            if let Err(e) = vm.run_external_jobs_until_idle() {
                eprintln!("{}", e);
                return 1;
            }
            print_value(&mut vm, &v);
            0
        }
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

fn repl() -> i32 {
    let mut vm = new_vm();
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut buffer = String::new();

    println!("RuJa v{} - JavaScript REPL (Ctrl+C to exit)", VERSION);
    loop {
        let prompt = if buffer.matches('{').count() > buffer.matches('}').count() {
            "  ... "
        } else {
            "ruja> "
        };
        print!("{}", prompt);
        if stdout.flush().is_err() {
            break;
        }

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => {
                println!();
                break;
            }
            Ok(_) => {}
            Err(_) => break,
        }

        buffer.push_str(&line);
        if buffer.matches('{').count() > buffer.matches('}').count() {
            continue;
        }

        let trimmed = buffer.trim();
        if trimmed.is_empty() {
            buffer.clear();
            continue;
        }
        if trimmed == ".exit" || trimmed == ".quit" {
            break;
        }

        match vm.run(&buffer) {
            Ok(v) => {
                if !v.is_undefined() {
                    print_value(&mut vm, &v);
                }
            }
            Err(e) => eprintln!("{}", e),
        }
        buffer.clear();
    }
    0
}

fn main() {
    // Run the engine on a worker thread with a generous stack so that deep
    // (but legal) JS recursion, bounded by the engine's own
    // `MAX_CALL_STACK_DEPTH`, can not overflow the Rust thread stack and
    // abort the process. The default main-thread stack is 8 MiB; give the
    // worker 64 MiB to comfortably support a deep call limit.
    let worker = thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(main_impl)
        .expect("failed to spawn engine worker thread");
    let code = worker.join().unwrap_or(1);
    exit(code);
}

fn main_impl() -> i32 {
    let args: Vec<String> = env::args().collect();
    if args.len() == 1 {
        return repl();
    }
    let i = 1;
    if i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print!("{}", HELP);
                return 0;
            }
            "-V" | "--version" => {
                println!("ruja {}", VERSION);
                return 0;
            }
            "-e" | "--eval" => {
                if i + 1 >= args.len() {
                    eprintln!("ruja: -e requires an argument");
                    return 2;
                }
                return run_eval(&args[i + 1]);
            }
            "--module" => {
                if i + 1 >= args.len() {
                    eprintln!("ruja: --module requires a file");
                    return 2;
                }
                return run_file(&args[i + 1], true);
            }
            "--" => {
                if i + 1 < args.len() {
                    return run_file(&args[i + 1], false);
                }
                return 0;
            }
            arg if arg.starts_with('-') => {
                eprintln!("ruja: unknown option '{}'", arg);
                eprintln!("Try 'ruja --help' for more information.");
                return 2;
            }
            file => return run_file(file, file.ends_with(".mjs")),
        }
    }
    repl()
}
