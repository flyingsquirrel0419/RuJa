# RuJa

[English](../README.md) · [한국어](README.ko.md) · [Español](README.es.md) · [日本語](README.ja.md) · [中文](README.zh.md)

Rust로 작성된 JavaScript 엔진 — **바이트코드 VM** + **마크 앤 스윕 GC**,
**외부 의존성 없음**.

실용적인 ES5.1 서브셋과 ES2015+ 기능을 지원합니다: 클래스, async/await,
제너레이터, Promise, 구조분해 할당, Symbol, Map/Set, 정규표현식 등. JavaScript를
스택 기반 바이트코드로 컴파일하여 자체 VM에서 실행하고 가비지 컬렉션으로 메모리를 관리합니다.

```sh
$ cargo run --release -- examples/fib.js
0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55
```

## 빠른 시작

```sh
git clone https://github.com/flyingsquirrel0419/RuJa.git
cd RuJa
cargo build --release

./target/release/ruja script.js   # 파일 실행
./target/release/ruja -e "1+2*3"  # 표현식 평가
./target/release/ruja             # REPL 시작
```

## 예제

```javascript
function fib(n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}
console.log([0,1,2,3,4,5,6,7,8,9,10].map(fib).join(", "));
```

더 많은 예제는 [`examples/`](../examples/) 디렉토리에 있습니다 — 제너레이터, async/await,
클래스 상속, Promise 체이닝.

## 라이브러리 API

```rust
use ruja::{Vm, Value};

fn main() {
    let mut vm = Vm::new();
    let result = vm.run("[1,2,3].reduce((a,b) => a+b, 0);");
    assert_eq!(result.unwrap(), Value::Number(6.0));
}
```

## 문서

- [아키텍처](../docs/architecture.md) — 파이프라인, GC, 모듈 구조
- [기능](../docs/features.md) — 언어 및 표준 라이브러리 전체 참조
- [한계](../docs/limitations.md) — 알려진 한계 및 엣지 케이스
- [변경 이력](../CHANGELOG.md) — 릴리스 히스토리

## 라이선스

Apache-2.0
