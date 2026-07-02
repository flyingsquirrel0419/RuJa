# RuJa Handoff — 2026-07-02

## 완료된 작업 (이번 라운드)

### 0.4.0-alpha 릴리즈
- Cargo.toml/Cargo.lock 버전 0.4.0-alpha
- CHANGELOG.md: [0.4.0-alpha] 섹션 추가, 0.2.0 → 0.2.0-alpha 수정
- GitHub 릴리즈 노트: 0.3.0-alpha + 0.4.0-alpha (둘 다 prerelease)
- 태그 v0.4.0-alpha, 푸시 완료

### test262 서브셋 재정의
- docs/test262.md: "Supported Subset" 섹션 추가 (ES5.1 core + selected ES2015+ 명시)
- 0.4.0-alpha 기준 서브셋 통과율 측정:
  - identifiers/keywords/types/comments/whitespace/punctuators: 76.8%
  - expressions (all): 59.7%
  - statements (all): 43.0%
  - subset aggregate: ~56% of language/ tests
- README: "RuJa does not claim full ES conformance" 문구 추가, test262 링크 보강
- docs/limitations.md: test262 문구 서브셋 기반으로 업데이트

### 벤치마크 공개
- docs/benchmarks.md: RuJa vs Boa vs QuickJS vs Node.js 비교표
  - fib(25): RuJa 719ms / Boa 53ms / QuickJS 5.2ms / Node 0.6ms
  - loop 100k: RuJa 248ms / Boa 59ms / QuickJS 3.4ms / Node 0.3ms
  - array push 10k: RuJa 31ms / Boa 6.1ms / QuickJS 0.6ms / Node 0.2ms
- "14x slower than Boa, 100-140x slower than QuickJS" 정직하게 공개
- overhead 원인 분석 (Arc/Mutex, no JIT, worklist GC)
- README에 벤치마크 링크 추가

### Dogfooding 데모
- examples/plugin_runner.rs: 샌드박싱 플러그인 러너 데모
  - 4가지 시나리오: 정상 플러그인, 무한루프(fuel 차단), OOM 공격(heap cap 차단), throw(catchable)
  - 각 플러그인에 독립적인 fuel/heap 리미트 설정
  - 호스트 API (log, fetch_data, compute_hash) 노출
- README에 데모 링크 추가

### vm/mod.rs 추가 모듈 분할
- vm/mod.rs: 3222줄 → 820줄 (구조체 정의 + 코어 인터프리터)
- vm/conversions.rs: 737줄 (ToInt32, ToUint32, ToNumber, ToString, ToPrimitive)
- vm/property.rs: 734줄 (프로퍼티 접근, 프로토타입 체인, 배열 인덱스/길이 설정)
- vm/async_runtime.rs: 969줄 (Promise 마이크로태스크, 제너레이터, async/await)
- vm/ops.rs: 2083줄 (기존 옵코드 디스패치, 변경 없음)
- 전체 테스트 519개 통과, clippy 경고 0개

## 현재 상태
- 버전: 0.4.0-alpha
- 전체 테스트 519개 + doctest 1개 통과
- clippy 경고 0개
- CI: 최신 커밋 (0.4.0-alpha 릴리즈) 푸시 완료

## 남은 작업 (로드맵)
- (v1.0 태그는 사용자 요청으로 제외)
- 추가 test262 conformance 개선 (ongoing)
- 벤치마크 성능 개선 (Mutex 오버헤드 줄이기 등)
- vm/ops.rs 추가 분할 (선택, 2083줄)

## 기술적 메모
- Heap::allocate는 Result<usize, HeapLimitExceeded> 반환
- Vm::new()/register_fn() 등이 Result 반환
- vm 모듈 구조: mod.rs (코어) + ops.rs (옵코드) + conversions.rs + property.rs + async_runtime.rs
- 각 분할 파일은 `use super::*`로 공통 import 처리, pub(crate) fn으로 크로스 파일 접근
