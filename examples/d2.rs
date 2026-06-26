fn main() {
    { let mut vm = ruja::Vm::new(); println!("simple_fn = {:?}", vm.run("function f(){ return 42; } f();")); }
    { let mut vm = ruja::Vm::new(); println!("recur1 = {:?}", vm.run("function g(n){ if(n<=0) return 0; return 1; } g(3);")); }
    { let mut vm = ruja::Vm::new(); println!("typeof_f = {:?}", vm.run("function f(){ return typeof f; } f();")); }
    { let mut vm = ruja::Vm::new(); println!("recur = {:?}", vm.run("function g(n){ if(n<=0) return 0; return 1+g(n-1); } g(3);")); }
}
