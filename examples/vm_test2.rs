fn main() {
    let mut vm = ruja::Vm::new();
    let r = vm.run("1 + 2;");
    println!("1+2 = {:?}", r);
}
