fn main() {
    { let mut vm = ruja::Vm::new(); println!("proto = {:?}", vm.run("function Shape() {} Shape.prototype.describe = function() { return 'shape'; }; new Shape().describe();")); }
    { let mut vm = ruja::Vm::new(); println!("nested = {:?}", vm.run("function outer() { let x = 10; function inner() { return x; } return inner(); } outer();")); }
    { let mut vm = ruja::Vm::new(); println!("map = {:?}", vm.run("let m = new Map(); m.set('a', 1); m.size;")); }
}
