fn main() {
    let mut vm = ruja::Vm::new();
    println!("m1 = {:?}", vm.run("[1,2,3].map(x => x*2);"));
    println!("m2 = {:?}", vm.run("[1,2,3].map(function(x){ return x*2; });"));
    println!("red = {:?}", vm.run("[1,2,3].reduce((a,b)=>a+b, 0);"));
}
