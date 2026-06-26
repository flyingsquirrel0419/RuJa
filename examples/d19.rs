fn main() {
    { let mut vm = ruja::Vm::new(); println!("throw_only = {:?}", vm.run("throw 'x';")); }
    { let mut vm = ruja::Vm::new(); println!("catch_val = {:?}", vm.run("try { throw 42; } catch(e) { e; }")); }
    { let mut vm = ruja::Vm::new(); println!("assign = {:?}", vm.run("let r = 0; r = 5; r;")); }
}
