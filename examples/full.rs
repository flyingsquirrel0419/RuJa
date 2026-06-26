fn main() {
    let mut vm = ruja::Vm::new();
    println!("arith = {:?}", vm.run("1 + 2 * 3;"));
    println!("fib = {:?}", vm.run("function fib(n){ if(n<=1) return n; return fib(n-1)+fib(n-2); } fib(10);"));
    println!("obj = {:?}", vm.run("let o = {x:1, y:2}; o.x + o.y;"));
    println!("arr = {:?}", vm.run("let a = [1,2,3]; a.map(x => x*2).join(',');"));
    println!("reduce = {:?}", vm.run("[1,2,3,4,5].reduce((a,b)=>a+b, 0);"));
    println!("math = {:?}", vm.run("Math.floor(3.7) + Math.max(1,5,3);"));
    println!("str = {:?}", vm.run("'hello'.toUpperCase() + ' ' + 'world'.slice(0,3);"));
    println!("json = {:?}", vm.run("JSON.stringify({a:1, b:[2,3]});"));
    println!("closure = {:?}", vm.run("function mk(){ let c=0; return function(){ c++; return c; }; } let f=mk(); f(); f();"));
    println!("proto = {:?}", vm.run("function A(n){ this.name=n; } A.prototype.greet=function(){ return 'hi '+this.name; }; new A('Rex').greet();"));
    println!("try = {:?}", vm.run("let r=0; try { throw 42; } catch(e){ r=e; } r;"));
    println!("for = {:?}", vm.run("let s=0; for(let i=0;i<5;i++) s+=i; s;"));
}
