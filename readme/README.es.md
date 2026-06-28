# RuJa

[English](../README.md) · [한국어](README.ko.md) · [Español](README.es.md) · [日本語](README.ja.md) · [中文](README.zh.md)

Un motor de JavaScript escrito en Rust — **VM de bytecode** + **recolector de basura mark-and-sweep**,
con **cero dependencias externas**.

Ejecuta un subconjunto pragmático de ES5.1 más funciones seleccionadas de ES2015+:
clases, async/await, generadores, Promesas, desestructuración, Symbols, Map/Set,
expresiones regulares y más. JavaScript se compila a un bytecode basado en pila
y se ejecuta en una VM propia con gestión automática de memoria.

```sh
$ cargo run --release -- examples/fib.js
0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55
```

## Inicio rápido

```sh
git clone https://github.com/flyingsquirrel0419/RuJa.git
cd RuJa
cargo build --release

./target/release/ruja script.js   # ejecutar un archivo
./target/release/ruja -e "1+2*3"  # evaluar una expresión
./target/release/ruja             # iniciar el REPL
```

## Ejemplos

```javascript
function fib(n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}
console.log([0,1,2,3,4,5,6,7,8,9,10].map(fib).join(", "));
```

Más ejemplos en el directorio [`examples/`](../examples/) — generadores, async/await,
jerarquías de clases y encadenamiento de Promesas.

## API de biblioteca

```rust
use ruja::{Vm, Value};

fn main() {
    let mut vm = Vm::new();
    let result = vm.run("[1,2,3].reduce((a,b) => a+b, 0);");
    assert_eq!(result.unwrap(), Value::Number(6.0));
}
```

## Documentación

- [Arquitectura](../docs/architecture.md) — pipeline, GC y estructura de módulos
- [Características](../docs/features.md) — referencia completa del lenguaje y la biblioteca estándar
- [Limitaciones](../docs/limitations.md) — brechas conocidas y casos límite
- [Changelog](../CHANGELOG.md) — historial de versiones

## Licencia

MIT
