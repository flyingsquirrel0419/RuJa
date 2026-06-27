fn main() {
    let mut vm = ruja::Vm::new();
    println!(
        "t = {:?}",
        vm.run(
            "class A { constructor() { this.y = 42; } getY() { return this; } } new A().getY();"
        )
    );
    println!(
        "y = {:?}",
        vm.run(
            "class A { constructor() { this.y = 42; } getY() { return this.y; } } new A().getY();"
        )
    );
    println!("o = {:?}", vm.run("let o = {y: 42}; o.y;"));
}
