fn main() {
    let mut vm = ruja::Vm::new();
    // direct: just return 1
    let r = vm.run("function f(){ return 1; } f();");
    println!("result = {:?}", r);
    // simpler: no function
    let r2 = vm.run("1;");
    println!("literal = {:?}", r2);
}
