use ruja::{Parser, Compiler};
fn main() {
    let src = "class A { constructor() { this.y = 42; } getY() { return this.y; } }";
    let p = Parser::parse(src).unwrap();
    let mut c = Compiler::new();
    let (chunk, funcs) = c.compile_program(&p).unwrap();
    println!("main chunk:");
    for (i, op) in chunk.code.iter().enumerate() { println!("  {:3}: {:?}", i, op); }
    for (fi, f) in funcs.iter().enumerate() {
        println!("func {}: {:?}", fi, f.name);
        for (i, op) in f.chunk.code.iter().enumerate() { println!("  {:3}: {:?}", i, op); }
        println!("  consts: {:?}", f.chunk.constants);
    }
}
