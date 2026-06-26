fn main() {
    let mut vm = ruja::Vm::new();
    println!("ternary = {:?}", vm.run("5 > 3 ? 1 : 2;"));
    let mut vm2 = ruja::Vm::new();
    println!("fact = {:?}", vm2.run("function fact(n) { return n <= 1 ? 1 : n * fact(n-1); } fact(5);"));
    let mut vm3 = ruja::Vm::new();
    println!("closure = {:?}", vm3.run("function f(){ let c=0; return function(){ c++; return c; }; } let g=f(); g();"));
    let mut vm4 = ruja::Vm::new();
    println!("throw = {:?}", vm4.run("let r; try { throw 'boom'; } catch(e) { r=e; } r;"));
}
