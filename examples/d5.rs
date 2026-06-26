fn main() {
    let mut vm = ruja::Vm::new();
    println!("g_typeof = {:?}", vm.run("typeof Object;"));
    println!("fn_param = {:?}", vm.run("function f(x){ return x; } f(5);"));
    println!("fn_global = {:?}", vm.run("function f(){ return Math.floor(3.7); } f();"));
}
