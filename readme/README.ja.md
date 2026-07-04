# RuJa

<p align="center"><img src="../assets/logo.png" alt="RuJa" width="400"></p>

[![CI](https://github.com/flyingsquirrel0419/RuJa/actions/workflows/ci.yml/badge.svg)](https://github.com/flyingsquirrel0419/RuJa/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/ruja.svg)](https://crates.io/crates/ruja)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/)

[English](../README.md) · [한국어](README.ko.md) · [Español](README.es.md) · [日本語](README.ja.md) · [中文](README.zh.md)

Rustアプリケーション内で信頼できないスクリプトを実行するための**サンドボックス化された組込み可能な
JavaScriptランタイム** — **`unsafe`ゼロ**、**燃料ベースの実行制限**、**パニック監査完了**、
**ヒープおよびコールスタック上限**付き。

ホストプロセスがスクリプト入力に関わらずクラッシュ、ハング、OOMを起こしてはならない
プラグインシステム、ゲームスクリプティング、サンドボックス評価のために設計されています。
JavaScriptはスタックベースのバイトコードにコンパイルされ、マークアンドスイープGCを備えた
独自のVMで実行されます。VMは`Send`であり、エンジン全体に`unsafe`コードはありません。

### サンドボックス保証

- **実行燃料**: `vm.set_fuel(Some(100_000))`がオペコード数を制限;
  枯渇するとキャッチ不可能な`RangeError`をスロー
- **ヒープリミット**: `vm.set_max_heap_objects(Some(10_000))`が生存オブジェクト数を制限;
  超過するとキャッチ可能な`RangeError`をスロー
- **コールスタック上限**: 最大1000フレーム; 深い再帰は`RangeError`をスローし、
  ネイティブクラッシュではない
- **ReDoS安全な正規表現**: RE2方式の線形時間マッチング（バックトラッキングなし）
- **パニックフリー**: VMホットパスに`unwrap()`は0個; cargo-fuzzで検証（96k+
  反復、パニックなし）

### サポート言語サブセット

ES5.1 + クラス、async/await、ジェネレーター、Promise、分割代入、
getter/setter、タグ付きテンプレート、Symbol、Map/Set、WeakMap/WeakSet、
Reflect、Proxy、Uint8Array、BigInt、Date、正規表現など。
サポート対象サブセットの test262 合格率: **86.8%** (`language/statements`
+ `language/expressions`、非対応機能のテストを除外)。フルスイートとの
違いは [test262 適合性](../docs/test262.md) を参照してください。
サポートおよび意図的に非サポートの機能の全リストは
[制限事項](../docs/limitations.md)を参照してください。

```sh
$ cargo run --release -- examples/fib.js
0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55
```

## クイックスタート

```sh
git clone https://github.com/flyingsquirrel0419/RuJa.git
cd RuJa
cargo build --release

./target/release/ruja script.js   # ファイルを実行
./target/release/ruja -e "1+2*3"  # 式を評価
./target/release/ruja             # REPLを起動
```

## 例

```javascript
function fib(n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}
console.log([0,1,2,3,4,5,6,7,8,9,10].map(fib).join(", "));
```

他の例は [`examples/`](../examples/) ディレクトリにあります — ジェネレーター、async/await、
クラス階層、Promiseチェーン。

## ライブラリAPI

```rust
use ruja::{Vm, Value};

fn main() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    let result = vm.run("[1,2,3].reduce((a,b) => a+b, 0);");
    assert_eq!(result.unwrap(), Value::Number(6.0));
}
```

ネイティブ関数を登録してJSに公開:

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

> `Vm`は`Send`ですが`Sync`ではありません: スレッド間で移動できますが、
> 同時共有アクセスには外部同期が必要です（例: `Mutex<Vm>`）。
> [制限事項](../docs/limitations.md)を参照してください。

信頼できないコードに実行燃料予算を設定:

```rust
let mut vm = Vm::new().expect("failed to initialize VM");
vm.set_fuel(Some(1_000_000));      // ~100万オペコード後にRangeError
let _ = vm.run("while(true){}");    // Err("fuel exhausted")を返す
vm.set_fuel(None);                  // 再び無制限
```
チェックは協調的（各オペコード前）であり、プリエンプティブではありません —
単一の長いネイティブ呼び出しは細分化されません。[制限事項](../docs/limitations.md)を参照してください。

## ドキュメント

- [アーキテクチャ](../docs/architecture.md) — パイプライン、GC、モジュール構成
- [機能](../docs/features.md) — 言語と標準ライブラリの完全なリファレンス
- [制限事項](../docs/limitations.md) — 既知のギャップとエッジケース
- [test262](../docs/test262.md) — 適合性スイートランナーと合格率
- [変更履歴](../CHANGELOG.md) — リリース履歴
- [コントリビュート](../CONTRIBUTING.md) — 変更提案の方法

## ライセンス

Apache-2.0

---

⭐ RuJaが役立つと思ったら、GitHubでスターをお願いします — 他の人がこのプロジェクトを見つけやすくなります。
