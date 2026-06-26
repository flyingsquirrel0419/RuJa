fn main() {
    { let mut vm = ruja::Vm::new(); println!("new = {:?}", vm.run("function Shape() {} let s = new Shape(); typeof s;")); }
    { let mut vm = ruja::Vm::new(); println!("proto_set = {:?}", vm.run("function Shape() {} Shape.prototype.x = 5; Shape.prototype.x;")); }
    { let mut vm = ruja::Vm::new(); println!("full = {:?}", vm.run("function Shape() {} Shape.prototype.describe = function() { return 'shape'; }; let s = new Shape(); s.describe();")); }
}
