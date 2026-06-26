fn main() {
    let mut vm = ruja::Vm::new();
    println!("retfn = {:?}", vm.run("function mk(){ return function(){ return 5; }; } typeof mk();"));
    println!("callfn = {:?}", vm.run("function mk(){ return function(){ return 5; }; } mk()();"));
}
