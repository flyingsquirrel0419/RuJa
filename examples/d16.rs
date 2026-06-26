fn main() {
    { let mut vm = ruja::Vm::new(); println!("for = {:?}", vm.run("let s = 0; for (let i = 0; i < 3; i++) { s += i; } s;")); }
    { let mut vm = ruja::Vm::new(); println!("inc = {:?}", vm.run("let i = 0; i++; i;")); }
    { let mut vm = ruja::Vm::new(); println!("while = {:?}", vm.run("let i = 0; while (i < 3) { i++; } i;")); }
}
