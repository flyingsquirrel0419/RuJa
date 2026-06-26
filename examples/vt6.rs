fn main() {
    let mut vm = ruja::Vm::new();
    println!("call = {:?}", vm.run("function f(n){ return n; } f(10);"));
    println!("call2 = {:?}", vm.run("function f(n){ return n*2; } f(5);"));
}
