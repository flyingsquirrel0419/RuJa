fn main() {
    let mut vm = ruja::Vm::new();
    println!("global_obj = {:?}", vm.run("typeof Object;"));
    println!("global_math = {:?}", vm.run("typeof Math;"));
}
