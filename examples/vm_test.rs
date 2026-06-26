fn main() {
    let mut vm = ruja::Vm::new();
    println!("1+2*3 = {:?}", vm.run("1 + 2 * 3;"));
    println!("x+y = {:?}", vm.run("let x = 5; let y = 10; x + y;"));
    println!("sum = {:?}", vm.run("let s = 0; for (let i = 0; i < 5; i++) { s += i; } s;"));
    println!("if = {:?}", vm.run("if (true) { 42; } else { 0; }"));
    println!("while = {:?}", vm.run("let i = 0; while (i < 3) { i++; } i;"));
}
