# RuJa

<p align="center"><img src="../assets/logo.png" alt="RuJa" width="400"></p>

[![CI](https://github.com/flyingsquirrel0419/RuJa/actions/workflows/ci.yml/badge.svg)](https://github.com/flyingsquirrel0419/RuJa/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/ruja.svg)](https://crates.io/crates/ruja)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/)

[English](../README.md) · [한국어](README.ko.md) · [Español](README.es.md) · [日本語](README.ja.md) · [中文](README.zh.md)

Un **runtime de JavaScript embebible y aislado (sandboxed)** para ejecutar scripts
no confiables dentro de aplicaciones Rust — **cero `unsafe`**, **ejecución limitada
por combustible (fuel)**, **auditado contra pánicos**, con **límites de heap y pila
de llamadas**.

Diseñado para sistemas de plugins, scripting de juegos y evaluación aislada donde
el proceso anfitrión no debe caer, colgarse ni quedarse sin memoria, independientemente
del script. JavaScript se compila a bytecode basado en pila y se ejecuta en una VM
propia con GC mark-and-sweep. La VM es `Send` (movible entre hilos) sin código
`unsafe` en todo el motor.

### Garantías del sandbox

- **Combustible de ejecución**: `vm.set_fuel(Some(100_000))` limita el número de opcodes;
  el agotamiento lanza un `RangeError` no capturable
- **Límite de heap**: `vm.set_max_heap_objects(Some(10_000))` limita los objetos vivos;
  superarlo lanza un `RangeError` capturable
- **Límite de pila de llamadas**: máximo 1000 frames; la recursión profunda lanza
  `RangeError`, no un crash nativo
- **Regex segura contra ReDoS**: matching en tiempo lineal estilo RE2 (sin backtracking)
- **Sin pánicos**: 0 `unwrap()` en la ruta caliente de la VM; verificado con cargo-fuzz
  (96k+ iteraciones sin pánicos)

### Subconjunto de lenguaje soportado

ES5.1 + clases, async/await, generadores, Promesas, desestructuración,
getters/setters, plantillas etiquetadas, Symbols, Map/Set, WeakMap/WeakSet,
Reflect, Proxy, Uint8Array, BigInt, Date, expresiones regulares y más.
Consulta [limitaciones](../docs/limitations.md) para la lista completa de
**Tasa de aprobación del subconjunto soportado: 87.0%** (`language/statements`
+ `language/expressions`, pruebas de funciones no soportadas excluidas).
Véase [conformidad test262](../docs/test262.md) para la diferencia con la suite completa.
características soportadas y no soportadas intencionalmente.

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
    let mut vm = Vm::new().expect("failed to initialize VM");
    let result = vm.run("[1,2,3].reduce((a,b) => a+b, 0);");
    assert_eq!(result.unwrap(), Value::Number(6.0));
}
```

Registrar funciones nativas y exponerlas a JS:

```rust
use ruja::{error::Result, NativeFn, Value, Vm};

fn add(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> Result<Value> {
    let a = vm.to_number(&args[0])?;
    let b = vm.to_number(&args[1])?;
    Ok(Value::Number(a + b))
}

fn main() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn("add", add as NativeFn, 2).unwrap();
    assert_eq!(vm.run("add(3, 4)").unwrap(), Value::Number(7.0));
}
```

> `Vm` es `Send` (pero no `Sync`): puede moverse entre hilos, pero el acceso
> concurrente compartido necesita sincronización externa (ej. `Mutex<Vm>`).
> Consulta [Limitaciones](../docs/limitations.md).

Limitar código no confiable con un presupuesto de combustible:

```rust
let mut vm = Vm::new().expect("failed to initialize VM");
vm.set_fuel(Some(1_000_000));      // ~1M opcodes antes de un RangeError
let _ = vm.run("while(true){}");    // devuelve Err("fuel exhausted")
vm.set_fuel(None);                  // ilimitado de nuevo
```
La verificación es cooperativa (antes de cada opcode), no preventiva — una sola
llamada nativa larga no se subdivide. Consulta [Limitaciones](../docs/limitations.md).

## Documentación

- [Arquitectura](../docs/architecture.md) — pipeline, GC y estructura de módulos
- [Características](../docs/features.md) — referencia completa del lenguaje y la biblioteca estándar
- [Limitaciones](../docs/limitations.md) — brechas conocidas y casos límite
- [test262](../docs/test262.md) — runner de suite de conformidad y tasa de aprobación
- [Changelog](../CHANGELOG.md) — historial de versiones
- [Contribuir](../CONTRIBUTING.md) — cómo proponer cambios

## Licencia

Apache-2.0

---

⭐ Si RuJa te resulta útil, por favor considera darle una estrella en GitHub — ayuda a otros a descubrir el proyecto.
