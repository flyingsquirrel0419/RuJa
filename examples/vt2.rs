fn main() {
    let mut vm = ruja::Vm::new();
    println!("ret1 = {:?}", vm.run("function f(){ return 1; } f();"));
    println!("ret5 = {:?}", vm.run("function f(){ return 5; } f();"));
    println!("noargs = {:?}", vm.run("function f(){ return 42; } f();"));
}
