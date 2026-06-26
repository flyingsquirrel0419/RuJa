use ruja::Parser;
fn main() {
    let src = "try { throw 42; } catch(e) { e; }";
    let p = Parser::parse(src).unwrap();
    println!("{:#?}", p.body);
}
