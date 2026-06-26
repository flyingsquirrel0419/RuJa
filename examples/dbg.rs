use ruja::{Parser, value::Value};
fn main() {
    // Check if parser still parses arithmetic
    let src = "1 + 2;";
    match Parser::parse(src) {
        Ok(p) => println!("AST body len: {}, first: {:?}", p.body.len(), p.body.first()),
        Err(e) => println!("parse err: {}", e),
    }
    // Now compile and dump bytecode
    let src2 = "1 + 2;";
    match Parser::parse(src2) {
        Ok(program) => {
            let mut compiler = ruja::compiler::Compiler::new();
            match compiler.compile_program(&program) {
                Ok(chunk) => {
                    println!("chunk ops: {:?}", chunk.code);
                    println!("chunk consts: {:?}", chunk.constants);
                }
                Err(e) => println!("compile err: {}", e),
            }
        }
        Err(e) => println!("parse err: {}", e),
    }
}
