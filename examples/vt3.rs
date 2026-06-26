fn main() {
    let mut vm = ruja::Vm::new();
    println!("ret5 = {:?}", vm.run("function f(){ return 5; } f();"));
    println!("add = {:?}", vm.run("function add(a,b){ return a+b; } add(3,4);"));
    println!("fib = {:?}", vm.run("function fib(n){ if(n<=1) return n; return fib(n-1)+fib(n-2); } fib(10);"));
}
