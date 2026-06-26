fn main() {
    let mut vm = ruja::Vm::new();
    println!("g_floor = {:?}", vm.run("Math.floor(3.7);"));
    let mut vm2 = ruja::Vm::new();
    println!("fn_floor = {:?}", vm2.run("function f(){ return Math.floor(3.7); } f();"));
    let mut vm3 = ruja::Vm::new();
    println!("typeof_math_in_fn = {:?}", vm3.run("function f(){ return typeof Math; } f();"));
}
