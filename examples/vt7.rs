fn main() {
    let mut vm = ruja::Vm::new();
    println!("mul = {:?}", vm.run("3 * 4;"));
    println!("ret = {:?}", vm.run("function f(n){ return n * 2; } f(5);"));
}
