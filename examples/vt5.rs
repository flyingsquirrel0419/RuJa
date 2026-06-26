fn main() {
    let mut vm = ruja::Vm::new();
    println!("typeof fib = {:?}", vm.run("function fib(n){ return n; } typeof fib;"));
    println!("typeof add = {:?}", vm.run("function add(a,b){ return a+b; } typeof add;"));
}
