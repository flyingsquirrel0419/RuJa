fn main() {
    let mut vm = ruja::Vm::new();
    println!("add = {:?}", vm.run("function add(a,b){ return a+b; } add(3,4);"));
    println!("noret = {:?}", vm.run("function f(){ 5; } f();"));
    println!("ret1 = {:?}", vm.run("function f(){ return 1; } f();"));
}
