// Scratch probe: print actual values for ES2015 features.
fn main() {
    let mut vm = ruja::Vm::new();
    let cases: &[(&str, &str)] = &[
        ("class extends", "class A { f(){return 1;} } class B extends A { g(){return 2;} } new B().g()"),
        ("class extends inherited call", "class A { f(){return 7;} } class B extends A {} new B().f()"),
        ("super call", "class A { f(){return 10;} } class B extends A { f(){ return super.f() + 5; } } new B().f()"),
        ("static method", "class C { static s(){return 42;} } C.s()"),
        ("static field-ish", "class C { static of(...a){return a.length;} } C.of(1,2,3)"),
        ("for of arr sum", "let s=0; for(let x of [1,2,3]){ s+=x; } s"),
        ("for of string", "let s=''; for(let c of 'abc'){ s+=c; } s"),
        ("template literal", "let n=5; `n=${n}`"),
        ("array find", "[4,5,6].find(x=>x>4)"),
        ("string includes", "'abc'.includes('b')"),
        ("symbol desc", "Symbol('foo').toString()"),
        ("typeof Promise", "typeof Promise"),
        ("spread array", "[1, ...[2,3], 4].length"),
    ];
    for (name, src) in cases {
        match vm.run(src) {
            Ok(v) => println!("{:<30} => {:?}", name, v),
            Err(e) => println!("{:<30} => ERR {}", name, e),
        }
    }
}
