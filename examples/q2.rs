fn main() {
    let mut vm = ruja::Vm::new();
    println!("c = {:?}", vm.run("function mk(){ let c=5; return function(){ return c; }; } mk()();"));
    println!("inc = {:?}", vm.run("function mk(){ let c=0; return function(){ c++; return c; }; } let f=mk(); f(); f();"));
}
