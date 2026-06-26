fn main() {
    let mut vm = ruja::Vm::new();
    println!("add = {:?}", vm.run("function add(a, b) { return a + b; } add(3, 4);"));
    println!("obj = {:?}", vm.run("let o = {x: 1, y: 2}; o.x + o.y;"));
    println!("arr = {:?}", vm.run("let a = [1, 2, 3]; a[0] + a[2];"));
    println!("fib = {:?}", vm.run("function fib(n) { if (n <= 1) return n; return fib(n-1) + fib(n-2); } fib(10);"));
    println!("closure = {:?}", vm.run("function mk() { let c = 0; return function() { c++; return c; }; } let f = mk(); f(); f();"));
}
