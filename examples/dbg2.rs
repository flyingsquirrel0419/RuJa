use ruja::{Parser, Compiler};
fn main() {
    let src = "let s = 0; for (let i = 0; i < 3; i++) { s += i; } s;";
    let p = Parser::parse(src).unwrap();
    let mut c = Compiler::new();
    let chunk = c.compile_program(&p).unwrap();
    for (i, op) in chunk.code.iter().enumerate() {
        println!("{:3}: {:?}", i, op);
    }
    println!("consts: {:?}", chunk.constants);
}
