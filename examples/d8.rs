use ruja::{Parser, Compiler};
fn main() {
    let src = "let x = 5; x;";
    let p = Parser::parse(src).unwrap();
    let mut c = Compiler::new();
    let (chunk, _) = c.compile_program(&p).unwrap();
    for (i, op) in chunk.code.iter().enumerate() {
        println!("{:3}: {:?}", i, op);
    }
    println!("consts: {:?}", chunk.constants);
}
