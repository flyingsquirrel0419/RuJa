# RuJa

[English](../README.md) · [한국어](README.ko.md) · [Español](README.es.md) · [日本語](README.ja.md) · [中文](README.zh.md)

用 Rust 编写的 JavaScript 引擎 — **字节码 VM** + **标记清除 GC**，**零外部依赖**。

运行实用的 ES5.1 子集加精选 ES2015+ 特性：类、async/await、生成器、Promise、解构赋值、
Symbol、Map/Set、正则表达式等。JavaScript 被编译为基于栈的字节码，在自研 VM 上执行，
并通过垃圾回收管理内存。

```sh
$ cargo run --release -- examples/fib.js
0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55
```

## 快速开始

```sh
git clone https://github.com/flyingsquirrel0419/RuJa.git
cd RuJa
cargo build --release

./target/release/ruja script.js   # 运行文件
./target/release/ruja -e "1+2*3"  # 求值表达式
./target/release/ruja             # 启动 REPL
```

## 示例

```javascript
function fib(n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}
console.log([0,1,2,3,4,5,6,7,8,9,10].map(fib).join(", "));
```

更多示例在 [`examples/`](../examples/) 目录 — 生成器、async/await、类继承、Promise 链式调用。

## 库 API

```rust
use ruja::{Vm, Value};

fn main() {
    let mut vm = Vm::new();
    let result = vm.run("[1,2,3].reduce((a,b) => a+b, 0);");
    assert_eq!(result.unwrap(), Value::Number(6.0));
}
```

## 文档

- [架构](../docs/architecture.md) — 流水线、GC、模块结构
- [功能](../docs/features.md) — 语言与标准库完整参考
- [限制](../docs/limitations.md) — 已知不足与边界情况
- [更新日志](../CHANGELOG.md) — 版本历史

## 许可证

Apache-2.0
