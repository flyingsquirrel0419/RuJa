use ruja::{Parser, Compiler};
fn main() {
    let src = "function fib(n){ if(n<=1) return n; return fib(n-1); } fib(5);";
    let p = Parser::parse(src).unwrap();
    let mut c = Compiler::new();
    let (chunk, _) = c.compile_program(&p).unwrap();
    for (i, op) in chunk.code.iter().enumerate() {
        println!("{:3}: {:?}", i, op);
    }
}
