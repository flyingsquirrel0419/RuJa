fn main() {
    {
        let mut vm = ruja::Vm::new();
        println!("closure = {:?}", vm.run("function mk(){ let c=0; return function(){ c++; return c; }; } let f=mk(); f(); f();"));
    }
    {
        let mut vm = ruja::Vm::new();
        println!("proto = {:?}", vm.run("function A(n){ this.name=n; } A.prototype.greet=function(){ return 'hi '+this.name; }; new A('Rex').greet();"));
    }
    {
        let mut vm = ruja::Vm::new();
        println!("try = {:?}", vm.run("let r=0; try { throw 42; } catch(e){ r=e; } r;"));
    }
    {
        let mut vm = ruja::Vm::new();
        println!("for = {:?}", vm.run("let s=0; for(let i=0;i<5;i++) s+=i; s;"));
    }
}
