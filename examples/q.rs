fn main() {
    let mut vm = ruja::Vm::new();
    println!("v1 = {:?}", vm.run("let x = 5; let y = 10; x + y;"));
    println!("v2 = {:?}", vm.run("var a = 1; a = 2; a;"));
}
