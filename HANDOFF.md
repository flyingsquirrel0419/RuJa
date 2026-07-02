# RuJa Handoff — 2026-07-02

## 완료된 작업 (이번 라운드)

### Heap limit 전면 강제 (Critical fix)
- `Heap::allocate` 시그니처를 `Result<usize, HeapLimitExceeded>`로 변경
- `From<HeapLimitExceeded> for Arc<Error>` 구현 → catchable `RangeError` 변환
- 59+개 `unwrap_or(0)` / raw `heap.allocate()` 호출부 전부 `?` 에러 전파로 변환
- `alloc_checked` / `allocate_unchecked` 제거, `Vm::alloc` 하나로 통일
- `Heap::allocate` 내부 `collect(&[])` 버그 수정 (빈 roots로 GC 실행 → 모든 객체 회수되던 치명적 버그)
- `Vm::new()`, `register_fn()`, `setup()`, `setup_full()` 등 20+ 함수 시그니처 `Result` 반환으로 변경
- 검증 테스트 3개 추가: `heap_limit_enforced_in_json_parse`, `heap_limit_enforced_in_array_map`, `heap_limit_enforced_in_regexp`

### 문서 업데이트
- CHANGELOG.md: "Heap Limit Enforcement Overhaul" 섹션 추가
- docs/audit-panics.md: heap limit 강제 내용 및 Future work 항목 추가
- README.md: `Vm::new()` 코드 예제 `Result` 반환에 맞게 수정, `register_fn` API 사용
- 4개 번역 README (ko/es/ja/zh)를 영문 README와 동일 구조로 전면 재작성:
  - 샌드박스 포지셔닝, 보장 섹션, 임베딩 API 예제, fuel 예제, test262/CONTRIBUTING 문서 링크 추가
  - 구 버전의 "JavaScript engine written in Rust" 포지셔닝 제거

## 현재 상태
- 전체 테스트 471개 + doctest 1개 통과
- clippy 경고 0개
- CI 녹색 (commit `99dd6c0`)
- 버전: 0.3.0-alpha

## 남은 작업 (로드맵)

### 1. test262 서브셋 재정의
- 전체 test262 통과율(33.2%) 경쟁에 매몰하지 말 것
- `docs/limitations.md`에 명시된 지원 범위 내에서 100% 통과 목표
- "ES5.1 + destructuring + classes + async/await + Map/Set + BigInt 서브셋은 spec-complete"라고 주장할 수 있어야 함
- 의도적으로 미지원 기능(Modules/Intl/Atomics/SharedArrayBuffer)은 "의도적 미지원"으로 문서화 유지

### 2. 벤치마크 정직하게 공개
- criterion으로 fib/loop/array 벤치 실행 (이미 `benches/basic.rs`에 있음)
- QuickJS-rs, Boa와 비교표 작성해서 README 또는 별도 docs/benchmarks.md에 공개
- "느리지만 안전하고 unsafe 0" 트레이드오프를 스스로 먼저 명시하는 것이 신뢰에 유리

### 3. Dogfooding 데모
- 후보: webhook/플러그인 스크립트 러너 (fuel 제한 걸고 사용자 JS 실행하는 미니 서비스)
- 또는: 게임/CLI 툴의 모딩 스크립트 엔진
- 별도 작은 레포로 만들어서 RuJa README에 "여기 이렇게 씀" 링크
- "또 다른 JS 엔진" → "이거 써서 이런 게 됨"으로 포지셔닝 전환

### 4. vm/mod.rs 추가 모듈 분할 (선택)
- `vm/mod.rs`가 여전히 3,175줄 → 런타임 헬퍼 / generator-promise 로직으로 분할 가능
- `ops.rs`도 opcode 카테고리별(arithmetic/control-flow/object-ops) 분할 여지 있음
- 컨트리뷰터 진입 장벽 낮추는 목적

### 5. 버전 1.0 태그 준비
- 현재 0.3.0-alpha
- 위 로드맵 1-3 완료 후 "안전 보장 범위가 명확한 v1.0"으로 태그
- "기능 다 갖췄다"가 아니라 "안전 보장 범위가 명확하다"로 1.0 찍기

## 기술적 메모
- `Heap::allocate`는 더 이상 내부에서 GC를 실행하지 않음 (roots를 모르므로)
- GC before allocate는 `Vm::alloc`에서 `self.collect_roots()`로 올바르게 수행
- `Vm::new()`가 `Result`를 반환하므로 모든 호출부에서 `?` 또는 `.expect()` 필요
- `register_fn()`도 `Result<()>` 반환
