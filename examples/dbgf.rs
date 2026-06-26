use ruja::{Parser, Compiler};
fn main() {
    let src = "function f(){ return 1; } f();";
    let p = Parser::parse(src).unwrap();
    let mut c = Compiler::new();
    let (chunk, funcs) = c.compile_program(&p).unwrap();
    println!("main ops: {:?}", chunk.code);
    println!("main consts: {:?}", chunk.constants);
    for (i, f) in funcs.iter().enumerate() {
        println!("func {} ops: {:?}", i, f.chunk.code);
        println!("func {} consts: {:?}", i, f.chunk.constants);
    }
}
