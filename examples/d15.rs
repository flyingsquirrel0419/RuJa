use ruja::Parser;
fn main() {
    let src = "let s = 0; for (let i = 0; i < 3; i++) { s += i; } s;";
    let p = Parser::parse(src).unwrap();
    println!("{:#?}", p.body);
}
