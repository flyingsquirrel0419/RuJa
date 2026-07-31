# RuJa

<p align="center"><img src="../assets/logo.png" alt="RuJa" width="400"></p>

[![CI](https://github.com/flyingsquirrel0419/RuJa/actions/workflows/ci.yml/badge.svg)](https://github.com/flyingsquirrel0419/RuJa/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/ruja.svg)](https://crates.io/crates/ruja)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/)

[English](../README.md) · [한국어](README.ko.md) · [Español](README.es.md) · [日本語](README.ja.md) · [中文](README.zh.md)

Rust 애플리케이션 내에서 신뢰할 수 없는 스크립트를 실행하기 위한 **샌드박스된, 임베더블 JavaScript 런타임** —
**`unsafe` 제로**, **퓨엘 기반 실행 제한**, **패닉 감사 완료**, **힙 및 콜스택 상한** 포함.

호스트 프로세스가 스크립트 입력에 관계없이 크래시, 중단, OOM이 발생하지 않아야 하는
플러그인 시스템, 게임 스크립팅, 샌드박스 평가를 위해 설계되었습니다. JavaScript는
스택 기반 바이트코드로 컴파일되어 마크 앤 스윕 GC를 갖춘 자체 VM에서 실행됩니다.
VM은 `Send`이며 엔진 전체에 `unsafe` 코드가 없습니다.

### 샌드박스 보장

- **실행 퓨엘**: `vm.set_fuel(Some(100_000))`으로 옵코드 수를 제한;
  소진 시 캐치 불가능한 `RangeError` 발생
- **힙 리밋**: `vm.set_max_heap_objects(Some(10_000))`으로 살아있는 객체 수 제한;
  초과 시 캐치 가능한 `RangeError` 발생
- **콜스택 상한**: 최대 1000 프레임; 깊은 재귀는 `RangeError`를 발생시키며
  네이티브 크래시가 아님
- **ReDoS 안전 정규식**: RE2 방식 선형 시간 매칭 (백트래킹 없음)
- **패닉 프리**: VM 핫 경로에 `unwrap()` 0개; cargo-fuzz 검증 (96k+
  반복, 패닉 없음)

### 지원 언어 서브셋

ES5.1 + 클래스, async/await, 제너레이터, Promise, 구조분해 할당,
getter/setter, 태그드 템플릿, Symbol, Map/Set, WeakMap/WeakSet,
Reflect, Proxy, Uint8Array, BigInt, Date, 정규표현식 등.
지원 서브셋 test262 통과율: **99.7%** (12,722 통과 / 43 실패,
미지원 기능 7,674개 제외). 전체 스위트 대비
차이는 [test262 적합성](../docs/test262.md) 문서를 참조하세요.
지원 및 의도적으로 미지원 기능 전체 목록은
[한계](../docs/limitations.md) 문서를 참조하세요.

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
    let mut vm = Vm::new().expect("failed to initialize VM");
    let result = vm.run("[1,2,3].reduce((a,b) => a+b, 0);");
    assert_eq!(result.unwrap(), Value::Number(6.0));
}
```

네이티브 함수를 등록해서 JS에 노출하기:

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

> `Vm`은 `Send`이지만 `Sync`는 아닙니다: 스레드 간 이동은 가능하지만
> 동시 공유 접근에는 외부 동기화가 필요합니다 (예: `Mutex<Vm>`).
> [한계](../docs/limitations.md) 문서를 참조하세요.

신뢰할 수 없는 코드에 실행 퓨엄 예산 걸기:

```rust
let mut vm = Vm::new().expect("failed to initialize VM");
vm.set_fuel(Some(1_000_000));      // ~1M 옵코드 후 RangeError
let _ = vm.run("while(true){}");    // Err("fuel exhausted") 반환
vm.set_fuel(None);                  // 다시 무제한
```
체크는 협력적(각 옵코드 전)이며 선점형이 아닙니다 — 하나의 긴 네이티브
호출은 세분화되지 않습니다. [한계](../docs/limitations.md) 문서를 참조하세요.

## 문서

- [아키텍처](../docs/architecture.md) — 파이프라인, GC, 모듈 구조
- [기능](../docs/features.md) — 언어 및 표준 라이브러리 전체 참조
- [한계](../docs/limitations.md) — 알려진 한계 및 엣지 케이스
- [test262](../docs/test262.md) — 적합성 스위트 러너 및 통과율
- [변경 이력](../CHANGELOG.md) — 릴리스 히스토리
- [기여](../CONTRIBUTING.md) — 변경 제안 방법

## 라이선스

Apache-2.0

---

⭐ RuJa가 도움이 되셨다면 GitHub에서 별을 눌러주세요 — 다른 사람들이 이 프로젝트를 발견하는 데 도움이 됩니다.
