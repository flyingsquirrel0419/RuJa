fn main() {
    let mut vm = ruja::Vm::new();
    // does function see its own name as a local?
    println!("typeof_f = {:?}", vm.run("function f(){ return typeof f; } f();"));
    // does it see other globals?
    println!("typeof_obj = {:?}", vm.run("function f(){ return typeof Object; } f();"));
}
