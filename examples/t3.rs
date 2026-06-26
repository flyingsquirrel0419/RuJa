fn main() {
    let mut vm = ruja::Vm::new();
    println!("join = {:?}", vm.run("[1,2,3].map(x => x*2).join(',');"));
    println!("len = {:?}", vm.run("[1,2,3].map(x => x*2).length;"));
}
