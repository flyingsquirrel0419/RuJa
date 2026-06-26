fn main() {
    let mut vm = ruja::Vm::new();
    println!("a[0] = {:?}", vm.run("let a = [1, 2, 3]; a[0];"));
    println!("a[2] = {:?}", vm.run("let a = [1, 2, 3]; a[2];"));
    println!("len = {:?}", vm.run("[1,2,3].length;"));
    println!("typeof = {:?}", vm.run("typeof [1,2];"));
}
