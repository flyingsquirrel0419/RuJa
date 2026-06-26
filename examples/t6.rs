fn main() {
    let mut vm = ruja::Vm::new();
    println!("inc = {:?}", vm.run("let i=0; i++; i;"));
    println!("loop3 = {:?}", vm.run("let i=0; while(i<3) i++; i;"));
}
