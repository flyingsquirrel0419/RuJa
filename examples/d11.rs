fn main() {
    { let mut vm = ruja::Vm::new(); println!("typeof_shape = {:?}", vm.run("function Shape() {} typeof Shape;")); }
    { let mut vm = ruja::Vm::new(); println!("proto_typeof = {:?}", vm.run("function Shape() {} typeof Shape.prototype;")); }
    { let mut vm = ruja::Vm::new(); println!("new_shape = {:?}", vm.run("function Shape() {} new Shape(); typeof new Shape();")); }
}
