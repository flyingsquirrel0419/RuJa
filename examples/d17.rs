use ruja::{Parser, Compiler};
fn main() {
    let src = "let i = 0; i;";
    let p = Parser::parse(src).unwrap();
    let mut c = Compiler::new();
    let (chunk, _) = c.compile_program(&p).unwrap();
    println!("ops: {:?}", chunk.code);
    println!("consts: {:?}", chunk.constants);
}
