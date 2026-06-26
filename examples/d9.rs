fn main() {
    let mut vm = ruja::Vm::new();
    println!("map = {:?}", vm.run("let m = new Map(); m.set('a', 1); m.get('a');"));
    let mut vm2 = ruja::Vm::new();
    println!("set = {:?}", vm.run("let s = new Set(); s.add(1); s.add(2); s.has(1);"));
    let mut vm3 = ruja::Vm::new();
    println!("sym = {:?}", vm.run("typeof Symbol();"));
}
