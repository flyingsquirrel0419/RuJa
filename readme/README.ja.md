# RuJa

[English](../README.md) · [한국어](README.ko.md) · [Español](README.es.md) · [日本語](README.ja.md) · [中文](README.zh.md)

Rustで書かれたJavaScriptエンジン — **バイトコードVM** + **マークアンドスイープGC**、
**外部依存関係ゼロ**。

実用的なES5.1サブセットとES2015+の機能を選択してサポートしています: クラス、async/await、
ジェネレーター、Promise、分割代入、Symbol、Map/Set、正規表現など。JavaScriptを
スタックベースのバイトコードにコンパイルし、独自のVMで実行、ガベージコレクションでメモリを管理します。

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
    let mut vm = Vm::new();
    let result = vm.run("[1,2,3].reduce((a,b) => a+b, 0);");
    assert_eq!(result.unwrap(), Value::Number(6.0));
}
```

## ドキュメント

- [アーキテクチャ](../docs/architecture.md) — パイプライン、GC、モジュール構成
- [機能](../docs/features.md) — 言語と標準ライブラリの完全なリファレンス
- [制限事項](../docs/limitations.md) — 既知のギャップとエッジケース
- [変更履歴](../CHANGELOG.md) — リリース履歴

## ライセンス

MIT
