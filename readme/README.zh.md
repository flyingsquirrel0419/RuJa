# RuJa

<p align="center"><img src="../assets/logo.png" alt="RuJa" width="400"></p>

[![CI](https://github.com/flyingsquirrel0419/RuJa/actions/workflows/ci.yml/badge.svg)](https://github.com/flyingsquirrel0419/RuJa/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/ruja.svg)](https://crates.io/crates/ruja)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/)

[English](../README.md) · [한국어](README.ko.md) · [Español](README.es.md) · [日本語](README.ja.md) · [中文](README.zh.md)

一个**沙盒化的、可嵌入的 JavaScript 运行时**，用于在 Rust 应用中运行不受信任的脚本 —
**零 `unsafe`**、**燃料计量执行**、**已通过 panic 审计**，带有**堆和调用栈上限**。

专为插件系统、游戏脚本和沙盒评估而设计，确保宿主进程不会因脚本输入而崩溃、挂起或 OOM。
JavaScript 被编译为基于栈的字节码，在带有标记清除 GC 的自研 VM 上执行。VM 是 `Send` 的
（可在线程间移动），引擎中没有任何 `unsafe` 代码。

### 沙盒保证

- **执行燃料**: `vm.set_fuel(Some(100_000))` 限制操作码数量;
  耗尽时抛出不可捕获的 `RangeError`
- **堆上限**: `vm.set_max_heap_objects(Some(10_000))` 限制存活对象数;
  超出时抛出可捕获的 `RangeError`
- **调用栈上限**: 最多 1000 帧; 深度递归抛出 `RangeError`，
  而非原生崩溃
- **ReDoS 安全的正则表达式**: RE2 风格的线性时间匹配（无回溯）
- **无 panic**: VM 热路径中 0 个 `unwrap()`; cargo-fuzz 验证（96k+
  次迭代，无 panic）

### 支持的语言子集

ES5.1 + 类、async/await、生成器、Promise、解构赋值、
getter/setter、标签模板、Symbol、Map/Set、WeakMap/WeakSet、
Reflect、Proxy、Uint8Array、BigInt、Date、正则表达式等。
**支持子集 test262 通过率: 87.0%**（`language/statements`
+ `language/expressions`，排除不支持功能的测试）。
与完整套件的差异请参见 [test262 一致性](../docs/test262.md)。
支持及有意不支持的功能完整列表请参见
[限制](../docs/limitations.md)。

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
    let mut vm = Vm::new().expect("failed to initialize VM");
    let result = vm.run("[1,2,3].reduce((a,b) => a+b, 0);");
    assert_eq!(result.unwrap(), Value::Number(6.0));
}
```

注册原生函数并暴露给 JS:

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

> `Vm` 是 `Send`（但不是 `Sync`）: 可以在线程间移动，但
> 并发共享访问需要外部同步（例如 `Mutex<Vm>`）。
> 参见[限制](../docs/limitations.md)。

为不受信任的代码设置执行燃料预算:

```rust
let mut vm = Vm::new().expect("failed to initialize VM");
vm.set_fuel(Some(1_000_000));      // ~100万操作码后触发 RangeError
let _ = vm.run("while(true){}");    // 返回 Err("fuel exhausted")
vm.set_fuel(None);                  // 再次无限制
```
检查是协作式的（每个操作码之前），不是抢占式的 — 单个
长时间的原生调用不会被细分。参见[限制](../docs/limitations.md)。

## 文档

- [架构](../docs/architecture.md) — 流水线、GC、模块结构
- [功能](../docs/features.md) — 语言与标准库完整参考
- [限制](../docs/limitations.md) — 已知不足与边界情况
- [test262](../docs/test262.md) — 一致性测试套件运行器和通过率
- [更新日志](../CHANGELOG.md) — 版本历史
- [贡献](../CONTRIBUTING.md) — 如何提出变更

## 许可证

Apache-2.0

---

⭐ 如果你觉得 RuJa 有用，请在 GitHub 上点个 Star — 帮助更多人发现这个项目。
