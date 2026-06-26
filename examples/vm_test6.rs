fn main() {
    let mut vm = ruja::Vm::new();
    println!("push = {:?}", vm.run("let a = [1,2]; a.push(3); a.length;"));
    println!("map = {:?}", vm.run("[1,2,3].map(x => x*2);"));
    println!("join = {:?}", vm.run("[1,2,3].join(',');"));
    println!("reduce = {:?}", vm.run("[1,2,3,4,5].reduce((a,b) => a+b, 0);"));
    println!("str = {:?}", vm.run("'hello'.toUpperCase();"));
    println!("json = {:?}", vm.run("JSON.stringify({a:1, b:[2,3]});"));
    println!("fib = {:?}", vm.run("function fib(n){ if(n<=1) return n; return fib(n-1)+fib(n-2); } fib(15);"));
    println!("proto = {:?}", vm.run("function Animal(n){ this.name=n; } Animal.prototype.speak=function(){ return this.name+'!'; }; new Animal('Rex').speak();"));
}
