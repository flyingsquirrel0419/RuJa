fn main() {
    let mut vm = ruja::Vm::new();
    println!("vars = {:?}", vm.run("let x = 5; let y = 10; x + y;"));
    let mut vm2 = ruja::Vm::new();
    println!("x = {:?}", vm2.run("let x = 5; x;"));
}
