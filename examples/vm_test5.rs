fn main() {
    let mut vm = ruja::Vm::new();
    println!("console.log = {:?}", vm.run("typeof console.log;"));
    println!("Math.floor = {:?}", vm.run("typeof Math.floor;"));
    println!("Math.floor(3.7) = {:?}", vm.run("Math.floor(3.7);"));
    println!("Array.isArray = {:?}", vm.run("typeof Array.isArray;"));
    println!("JSON = {:?}", vm.run("typeof JSON.stringify;"));
    println!("parseInt = {:?}", vm.run("parseInt('42');"));
}
