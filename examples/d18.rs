fn main() {
    { let mut vm = ruja::Vm::new(); println!("tc = {:?}", vm.run("let r; try { throw 'boom'; } catch(e) { r=e; } r;")); }
    { let mut vm = ruja::Vm::new(); println!("nest = {:?}", vm.run("function outer() { let x = 10; function inner() { return x; } return inner(); } outer();")); }
    { let mut vm = ruja::Vm::new(); println!("map = {:?}", vm.run("let m = new Map(); m.set('a', 1); m.get('a');")); }
    { let mut vm = ruja::Vm::new(); println!("proto = {:?}", vm.run("function Shape() {} Shape.prototype.describe = function() { return 'shape'; }; new Shape().describe();")); }
}
