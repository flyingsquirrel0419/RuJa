# Changelog

## [Unreleased]

### Changed

- Added a real Realm-local `%Temporal.ZonedDateTime%` hidden-slot core for UTC
  and fixed-offset identifiers. Construction, subclass/new-target behavior,
  exact epoch accessors, canonical time-zone/calendar identifiers, and
  ZonedDateTime-to-Instant fast paths now share immutable internal data. The
  exact 36-file Test262 boundary passes completely, removing the final skips
  from Instant `compare`, `from`, and `equals`.

  [Decision Log]
  - 목적과 의도: Instant conversion의 남은 ZonedDateTime blocker를 ordinary property 모방 없이 실제 재사용 가능한 객체 모델로 제거한다.
  - 기존 구현 및 제약 조건: Instant hidden slot과 exact parser는 있었지만 ZonedDateTime brand, Realm intrinsic, time-zone/calendar slot은 없었다. deterministic IANA transition backend도 없다.
  - 검토한 주요 대안: test-only shape, 공개 epoch getter, named IANA 식별자만 저장, UTC/fixed-offset hidden-slot core를 검토했다.
  - 선택한 방식: epoch `Arc<BigInt>`, canonical time-zone identifier/offset, ISO calendar identifier를 `TemporalKind::ZonedDateTime`에 저장하고 constructor와 네 accessor 및 ToTemporalInstant가 공유한다.
  - 다른 대안 대신 이 방식을 선택한 이유: observable property는 fast-path 의미론을 위반하고, tzdb 없는 named IANA 수용은 이후 offset 연산을 거짓 지원한다. UTC/fixed offset은 현재 deterministic formatter/parser와 정확히 결합된다.
  - 장점, 단점 및 영향: cross-Realm brand, subclass prototype, method-Realm errors, fuel, GC rollback과 exact 36/0/0이 검증된다. civil/calendar accessors, formatting, Duration 연동, named IANA/DST는 후속 단위다.

- Added Realm-local, non-constructable `Temporal.Instant.compare`. It converts
  both inputs in specification order through the shared exact Instant path,
  compares hidden epoch nanoseconds without observing public properties, and
  returns Number `-1`, `+0`, or `1`. String bytes remain fuel-metered. An exact
  29-file Test262 ownership boundary passes completely; the full directory is
  now **30 pass / 0 fail / 0 skip** after the ZonedDateTime core separately
  admits its internal-slot fast path.

  [Decision Log]
  - 목적과 의도: Instant의 정적 순서 비교를 기존 문자열·브랜드 변환 의미론과 하나의 경로로 제공한다.
  - 기존 구현 및 제약 조건: `from`과 `equals`는 exact `ToTemporalInstant` subset을 공유하지만 static `compare`는 없었다. `%Temporal.ZonedDateTime%` 객체 모델은 아직 없다.
  - 검토한 주요 대안: 공개 `epochNanoseconds` getter 사용, 두 입력의 동시 coercion, 비교 전 임시 Instant 객체 생성, shared epoch conversion을 순차 재사용하는 방식을 검토했다.
  - 선택한 방식: 첫 입력 변환을 완결한 뒤 둘째 입력을 변환하고 두 `Arc<BigInt>`를 직접 비교한다. installer는 Realm-local length-2 native function을 게시하며 29개 exact path만 연다.
  - 다른 대안 대신 이 방식을 선택한 이유: 공개 getter는 shadowing을 잘못 관찰하고 임시 객체는 불필요한 GC/heap 실패점을 만든다. 순차 shared conversion만 abrupt completion, Realm error, parser, fuel 순서를 보존한다.
  - 장점, 단점 및 영향: cross-Realm branded Instant, string-hint coercion, first-error short circuit, pre/post-epoch sign, allocation rollback이 검증된다. 후속 ZonedDateTime core가 fast path 1개를 별도 admission으로 완결했다.

- Added Realm-local, non-constructable `Temporal.Instant.prototype.toString`.
  It reads and converts options in specification order, implements all nine
  as-if-positive rounding modes, formats the complete Instant year range at
  exact nanosecond precision, and supports deterministic UTC/fixed-offset
  time-zone identifiers plus date-time, `T`-prefixed time, and unprefixed time
  forms. Unprefixed year-month/month-day ambiguities follow Temporal's early
  errors, and unit options accept only exact standard singular/plural names.
  Time-zone strings are precharged as sandbox fuel. An exact 54-file Test262
  boundary passes completely: 52
  `toString` files plus the newly unblocked `from`/`equals` wrong-type files.
  The `toString` directory is **52 pass / 0 fail / 2 skip**; only tests whose
  body or shared helper requires real
  Duration/PlainDateTime/PlainTime constructors and broader ZonedDateTime
  methods remain gated.

  [Decision Log]
  - 목적과 의도: Instant의 표준 문자열 직렬화, 옵션 관찰 순서, 나노초 반올림, Realm 브랜드 경계를 하나의 정확한 경로로 제공한다.
  - 기존 구현 및 제약 조건: Instant는 parse/from/equals/valueOf를 지원했지만 자체 toString이 없어 prototype coercion이 RangeError로 잘못 끝났고, Date formatter는 f64 밀리초와 제한된 연도 범위 때문에 재사용할 수 없었다. deterministic tzdb와 다른 Temporal constructors는 아직 없다.
  - 검토한 주요 대안: Date formatter 재사용, host timezone API, 가짜 Temporal constructors, 외부 full-Temporal 의존성, i128 기반 전용 formatter와 UTC/fixed-offset parser를 검토했다.
  - 선택한 방식: syntax parser가 date-time/time-zone 구조를 공유하고, pure i128 formatter가 civil date 역변환과 9개 as-if-positive rounding mode를 수행한다. VM layer는 brand/options/GC pin/fuel/Realm을 소유하며 54개 exact path만 연다.
  - 다른 대안 대신 이 방식을 선택한 이유: host timezone은 native/WASM 결과가 달라지고 가짜 constructors는 지원 범위를 왜곡한다. Instant 범위는 i128 안에 들어 exact 정수 경로로 precision과 pre-epoch floor 의미론을 보존할 수 있다.
  - 장점, 단점 및 영향: 기본 Z와 명시적 +00:00, negative epoch, extended year, exact singular/plural units, `T`-prefixed time, year-month/month-day ambiguity early errors, option coercion/validation order가 한 구현에서 검증된다. named IANA timezone, 나머지 Temporal constructors와 두 dependent 테스트는 후속이며 ZonedDateTime fast path는 이후 core 단위에서 완결됐다.

- Expanded the shared `Temporal.Instant` string parser used by `from` and
  `equals`. It now accepts independently basic or extended date, time, and
  offset forms; hour-only times and offsets; offset seconds with nanosecond
  fractions; and the audited RFC 9557 time-zone, calendar, and unknown
  annotation subset. Native parsing precharges input bytes as sandbox fuel.
  A frozen 36-file Test262 boundary passes completely; after the shared
  `toString` branding and ZonedDateTime core completions, the two directories
  are **62 pass / 0 fail / 0 skip**.

  [Decision Log]
  - 목적과 의도: `from`, `equals`, 후속 `compare`가 공유할 정확한 나노초 Instant 문자열 변환 기반을 확장한다.
  - 기존 구현 및 제약 조건: 기존 parser는 dashed date, colonized minute offset, annotation 없는 문자열만 처리했고 offset을 초 단위로 계산했다. 전체 Temporal parser crate 도입은 아직 없는 다른 Temporal type 문법까지 함께 소유하게 된다.
  - 검토한 주요 대안: 메서드별 parser 복제, Date parser 재사용, 외부 Temporal parser 의존성, 현재 정수 parser를 구조화해 확장하는 방식을 검토했다.
  - 선택한 방식: date/time/main-offset/annotation 단계를 분리하고 main offset을 나노초 정수로 계산한다. 변환 후 문자열 byte 길이를 fuel로 선차감하며 36개 exact path만 admission에 등록한다.
  - 다른 대안 대신 이 방식을 선택한 이유: Date parser는 밀리초 부동소수점과 관대한 정규화를 사용하고, 메서드별 복제는 coercion과 range 의미론을 분기시킨다. 현재 경로 확장은 공유 정밀도와 sandbox 경계를 보존한다.
  - 장점, 단점 및 영향: basic/extended 조합, sub-minute offset, annotation critical/중복 규칙과 Instant 경계값이 한 경로에서 검증된다. full RFC 9557 grammar는 후속이며 ZonedDateTime 내부 슬롯은 이후 core 단위에서 추가됐다.

- Added Realm-local, non-constructable `Temporal.Instant.prototype.valueOf`.
  The method always throws a `TypeError` without reading or branding its
  receiver, preventing implicit numeric or relational coercion of Instant
  values. A complete seven-file Test262 admission freezes the whole pinned
  method directory and its metadata.

  [Decision Log]
  - 목적과 의도: Instant가 암시적 primitive 변환에 참여하지 못하도록 표준의 항상-예외 `valueOf` 경계를 완결한다.
  - 기존 구현 및 제약 조건: Instant 내부 슬롯과 명시적 비교 경로는 있었지만 상속된 `Object.prototype.valueOf`가 객체를 반환해 관계 비교가 문자열 변환 등 후속 경로로 진행될 수 있었다.
  - 검토한 주요 대안: `Object.prototype.valueOf` 상속 유지, epoch BigInt 반환, receiver 브랜드 검사 후 예외, receiver를 관찰하지 않고 즉시 예외를 검토했다.
  - 선택한 방식: 각 Realm의 Instant prototype에 길이 0인 native `valueOf`를 설치하고 모든 호출에서 method Realm의 TypeError를 즉시 던진다.
  - 다른 대안 대신 이 방식을 선택한 이유: ECMAScript 알고리즘은 receiver 검사나 epoch 노출 없이 무조건 TypeError를 요구하며, epoch 반환은 Instant의 명시적 비교 API를 우회한다.
  - 장점, 단점 및 영향: 직접 호출과 관계 연산이 일관되게 거부되고 전체 7개 파일이 지원된다. 정렬과 동등성은 `equals` 및 후속 `compare` 구현을 사용해야 한다.

- Added `Temporal.Instant.from` and shared string conversion for
  `Temporal.Instant.prototype.equals`. This first parser boundary accepts exact
  extended ISO date-time strings with a required UTC designator or colonized
  minute offset, nanosecond precision, alternate time separators, and leap
  seconds. It also preserves branded-Instant fast paths, string-hint object
  coercion, method-Realm allocation, and the inclusive Instant range. A frozen
  15-file Test262 admission covers only this completed surface; RFC 9557
  annotations, compact offsets, and offset seconds were gated at this first
  boundary; later parser and ZonedDateTime core units now own them separately.

  [Decision Log]
  - 목적과 의도: Instant 생성과 비교가 공유하는 첫 `ToTemporalInstant` 문자열 변환 경계를 정확한 나노초 단위로 제공한다.
  - 기존 구현 및 제약 조건: branded Instant 비교와 epoch factory는 있었지만 문자열 입력은 거부했고, 기존 Date 파서는 부동소수점 밀리초와 관대한 정규화 때문에 재사용할 수 없었다.
  - 검토한 주요 대안: Date 파서 재사용, RFC 9557 전체 파서 일괄 구현, 검증 가능한 extended ISO 부분집합을 먼저 고정하는 방식을 검토했다.
  - 선택한 방식: 전용 정수 파서와 공유 변환 helper를 추가하고 현재 완결된 15개 Test262 파일만 admission으로 연다.
  - 다른 대안 대신 이 방식을 선택한 이유: 나노초 정밀도와 Temporal의 엄격한 날짜 검증을 보존하면서 아직 지원하지 않는 annotation 및 ZonedDateTime 의미론을 지원하는 것처럼 보이지 않게 하기 위해서다.
  - 장점, 단점 및 영향: `from`과 `equals`의 coercion 및 range 규칙이 한 경로로 통합된다. 전체 RFC 9557 문법은 후속 parser 단위가 필요하다.

- Added `Temporal.Instant.prototype.equals` for exact comparison between
  branded Instant values, including cross-Realm instances. Receiver branding
  is checked before the argument, and installation preserves the existing
  GC-retry/rooting contract. A frozen seven-file Test262 boundary covers this
  independently complete surface; string/object/ZonedDateTime conversion was
  gated here and is now owned by later `ToTemporalInstant` units.

  [Decision Log]
  - 목적과 의도: ISO 파싱 없이 완결 가능한 Instant 내부 슬롯 비교와 표준 메서드 형태를 먼저 제공한다.
  - 기존 구현 및 제약 조건: branded Instant와 정확한 epoch 저장소는 있었지만 `equals`가 없었고, 전체 알고리즘의 인수 변환은 아직 ISO 파서와 ZonedDateTime 지원에 의존한다.
  - 검토한 주요 대안: 전체 `ToTemporalInstant`를 동시에 구현하기, 임시 문자열 파서를 넣기, branded Instant 경계만 고정하기를 검토했다.
  - 선택한 방식: receiver를 먼저 브랜드 검사한 뒤 branded Instant 인수의 숨은 epoch 값을 직접 비교하고 독립적인 7개 파일만 admission에 등록했다.
  - 다른 대안 대신 이 방식을 선택한 이유: 관찰 가능한 프로퍼티에 의존하지 않는 정확한 비교를 제공하면서 미완성 변환 규칙을 부분 구현으로 위장하지 않기 위해서다.
  - 장점, 단점 및 영향: Realm과 GC 경계가 작고 검증 가능하다. 문자열, 변환 객체, ZonedDateTime 인수는 후속 `ToTemporalInstant` 단위 전까지 TypeError다.

- Added Realm-local `Temporal.Instant.fromEpochMilliseconds` and
  `fromEpochNanoseconds`. Both ignore the receiver, allocate through the
  method function's intrinsic Realm, preserve exact integer conversion, and
  enforce the inclusive Instant range. The frozen 19-file Test262 boundary now
  covers both complete factory directories, including their endpoint checks
  through `Temporal.Instant.from(string)` and `equals`.

  [Decision Log]
  - 목적과 의도: ISO 문자열 파싱과 독립적인 epoch 기반 Instant 생성 경로를 먼저 완결한다.
  - 기존 구현 및 제약 조건: epoch factory 자체는 완성됐지만 두 `limits.js`는 당시 없던 문자열 파싱과 `equals`를 함께 요구해 초기 admission에서 제외됐다. 두 의존성은 이제 정확한 정수 기반 경로로 구현됐다.
  - 검토한 주요 대안: 기존 17개 경계를 유지하기, 두 디렉터리를 prefix로 허용하기, 현재 전체 19개 경로와 메타데이터를 정확히 고정하기를 검토했다.
  - 선택한 방식: `ToNumber`/`NumberToBigInt` 및 `ToBigInt` 변환 뒤 기존 Realm-local Instant allocation helper를 재사용하고, 두 endpoint 파일을 포함한 19개 전체를 exact admission으로 등록했다.
  - 다른 대안 대신 이 방식을 선택한 이유: 구현된 endpoint 동작을 supported accounting에서 숨기지 않으면서 미래 Test262 파일을 자동 허용하지 않기 위해서다.
  - 장점, 단점 및 영향: 두 factory 디렉터리가 완전한 19/0/0 경계가 된다. 새로운 sibling 테스트는 별도 검토 전까지 계속 차단된다.

  Tooling validation now treats an inaccessible default Test262 checkout as
  unavailable while still checking the frozen 19-file list exactly. CI jobs
  that provide `TEST262` continue to verify the live pinned files and metadata.

- Added Realm-local `%Temporal.Instant%` construction, branded hidden
  `[[EpochNanoseconds]]` storage, exact `epochNanoseconds` and floor-rounded
  `epochMilliseconds` accessors, and intrinsic fallback behavior for custom
  `newTarget`. `Date.prototype.toTemporalInstant` now returns a real branded
  Instant selected from the method function's Realm. A frozen 19-file
  Test262 boundary covers this core surface.

  [Decision Log]
  - 목적과 의도: Temporal의 후속 연산이 신뢰할 수 있는 Instant 내부 슬롯과 Realm별 intrinsic identity를 먼저 확립한다.
  - 기존 구현 및 제약 조건: Date bridge는 관찰 가능한 일반 프로퍼티를 가진 모양만 비슷한 객체를 만들었고, 생성자와 브랜드 검증이 없었다.
  - 검토한 주요 대안: 일반 객체의 비공개 이름 프로퍼티, 기존 Date 저장소 재사용, 전용 heap variant를 검토했다.
  - 선택한 방식: 전용 `HeapObj::Temporal` 내부 슬롯과 immutable Realm registry를 사용하고, 생성과 Date bridge를 같은 allocation helper로 통합했다.
  - 다른 대안 대신 이 방식을 선택한 이유: JavaScript 프로퍼티 조작으로 브랜드나 epoch 값을 위조할 수 없어야 하며, 변경 가능한 전역 `Temporal` 바인딩에 intrinsic 선택이 의존하면 안 된다.
  - 장점, 단점 및 영향: 브랜드/Realm/GC 경계가 명확해지고 후속 Temporal 타입을 확장할 기반이 생긴다. 현재 enum과 registry가 Instant 중심이라 다른 Temporal 타입은 별도 단위로 추가해야 한다.

  Getter 설치도 GC 재시도 경로를 사용해 Realm 생성의 heap-cap 계약을
  유지한다. `Symbol.toStringTag`가 삭제되거나 문자열이 아니면 일반 객체
  fallback인 `[object Object]`를 반환한다.

- Added Realm-local `%Temporal%` and `%Temporal.Now%` namespace objects with
  specification-shaped prototypes, property descriptors, and
  `Symbol.toStringTag` values. A frozen four-file Test262 boundary covers only
  these namespace tags and requires **4 pass / 0 fail / 0 skip**; Temporal
  constructors and algorithms remain separate.

- Added Realm-local `SuppressedError(error, suppressed, message)` with the
  specified Error inheritance, constructor metadata, optional message
  coercion, and non-enumerable `error`/`suppressed` properties. Constructor
  inputs and the result stay rooted across observable message coercion. A
  frozen 22-file Test262 admission opens only the intrinsic tests; explicit
  resource-management syntax remains independently gated.

- Reworked `decodeURI` and `decodeURIComponent` into a byte-indexed decoder.
  Percent triplets now use a fixed four-byte stack buffer, reserved escapes
  retain their original spelling, and supplementary scalars append directly
  through RuJa's canonical UTF-16 sentinel representation without temporary
  per-scalar strings. The decoder reserves its intermediate result fallibly;
  final `Arc<str>` publication retains the existing infallible host-allocation
  boundary. It charges a conservative input-byte fuel bound after `ToString`
  but before native scanning. The existing focused URI boundary remains **167 pass / 0 fail / 2
  timeout / 4 skip**: the two RFC 3629 exhaustive tests execute roughly one
  million JavaScript calls per variant and remain an interpreter-throughput
  boundary rather than receiving a larger timeout.

- Completed the Annex B legacy Date boundary in every Realm. `getYear`
  returns the local year minus 1900, `setYear` snapshots the receiver before
  argument coercion and applies the legacy 0-99 year offset, and
  `toGMTString` aliases the original `toUTCString` function object. A frozen
  five-file admission opens only the verified Symbol and non-constructor
  metadata tests. Pinned focused Test262 moves from **5 pass / 19 fail / 0
  skip** to **24/0/0**. Commits `c3b1da6` and `0c52930` pass ordinary CI
  `30727866413` (**3/3**) and full Test262 CI `30727866412` (**45/45**).
  Complete Annex B is raw **1050 pass / 0 fail / 35 skip / 1 timeout**;
  normalized movement from **1027/19/40** is **+24 pass / -19 fail / -5
  skip**, with one unrelated timeout. Aggregate is raw **33378 pass / 4115
  fail / 10972 skip / 4 timeout / 0 error** over 48469 files.

- Implemented Annex B global `escape` and `unescape` in every Realm. Both
  functions coerce their argument exactly once, operate on UTF-16 code units,
  preserve lone surrogates, expose the specified metadata, and reject
  construction. Native scans consume a conservative sandbox fuel bound before
  result materialization, and intermediate buffers use checked lengths and
  fallible reservations. A frozen four-file admission opens only the supported
  Symbol and non-constructor tests. The focused pinned scope moves from **0
  pass / 31 fail / 4 skip** to **35/0/0**; complete Annex B moves from raw
  **992/50/44** to **1027/19/40**, with no timeout/error. Commit `d8d9fc1`
  passes ordinary CI `30711317106` (**3/3**) and full Test262 rerun
  `30712728403` (**45/45**). The ordinary artifact is byte-identical; 31/32
  full result artifacts are byte-identical and only Annex B changes. Aggregate
  is **33355 pass / 4134 fail / 10977 skip / 3 timeout / 0 error** over 48469
  files.

- Completed the residual Annex B RegExp boundary. Non-Unicode invalid `\c`
  escapes now preserve the reverse solidus and `c` as separate atoms, while
  character-class `\c0`-`\c9` and `\c_` use their Annex B control values.
  Lexer validation, quantifier scanning, class-range validation, and backend
  normalization share one control-tail predicate. A frozen four-file admission
  opens the two generator-based control tests and two already-supported
  non-Unicode malformed named-group tests. The complete pinned
  `annexB/built-ins/RegExp` subtree is now **62/0/0**, from **58/0/4**.
  Commit `51516e1` passes ordinary CI `30705669529` (**3/3**) and full
  Test262 CI `30705669498` (**45/45**). The ordinary artifact is byte-identical;
  31/32 full result artifacts are byte-identical and only Annex B changes.
  Normalized aggregate is **33319 pass / 4166 fail / 10981 skip / 3 timeout /
  0 error** over 48469 files.

- Implemented the Stage 3 legacy RegExp constructor static state in every
  Realm. `%RegExp%` now exposes the 19 configurable accessors from `input`/`$_`
  through `$1`-`$9`, with exact receiver checks and setter coercion order.
  Successful same-Realm direct matches update input, match, context, and
  capture state after result materialization; proper-subclass matches
  invalidate each slot, while directly borrowing the built-in `exec` across
  Realms changes neither Realm. Borrowed protocols update state only when the
  selected built-in `exec` and receiver belong to the same Realm. Match commits
  retain input and UTF-16 ranges; accessors materialize and cache context and
  capture Strings lazily. The unmetered same-exec-Realm Unicode-global
  `@@match` fast path is limited to the infallible linear backend, reconstructs
  captures for its last match, and commits before its outer result Array
  allocation. Native source formatting omits legacy accessor names that cannot
  form a NativeFunction `IdentifierName`, while preserving their exact `.name`
  properties. Exact accessor tests move from
  **0/24** to **24/0/0**; the complete Annex B RegExp subtree moves from
  **34 pass / 18 fail / 10 skip** to **58/0/4**. Commits `91674cd` and
  `3c71f0b` pass ordinary CI `30703293505` (**3/3**) and full Test262 CI
  `30703293498` (**45/45**). Ordinary artifacts are byte-identical; 31/32 full
  result artifacts are byte-identical and only Annex B changes. The normalized
  aggregate is **33315 pass / 4166 fail / 10985 skip / 3 timeout / 0 error**.

- Implemented the Stage 3 legacy `RegExp.prototype.compile` boundary in every
  Realm. RegExp instances now retain their creating Realm and whether they were
  allocated directly by that Realm's intrinsic constructor. `compile` rejects
  foreign-Realm and subclass receivers before argument coercion, snapshots
  RegExp pattern slots without observable property reads, and reuses
  `RegExpInitialize` so invalid input is atomic while a non-writable
  `lastIndex` fails after the new matcher is committed. Four exact feature
  admissions open Symbol and duplicate-name abrupt tests. The compile and
  dependent split/flags scope moves from **1 pass / 21 fail / 4 skip** to
  **26/0/0**; the complete Annex B RegExp subtree moves from normalized
  **9/39/14** to **34/18/10**, and complete Annex B moves from
  **938/90/58** to **963/69/54**, with no timeout/error.

- Completed the Annex B String legacy-method cluster. All 13 CreateHTML
  methods now perform receiver coercion before optional attribute coercion,
  escape only attribute quotation marks, and exist as non-constructable
  Realm-local native functions. `substr` now applies `ToIntegerOrInfinity`
  and treats an explicit `undefined` length as unbounded. `trimLeft` and
  `trimRight` are descriptor-correct references to the original `trimStart`
  and `trimEnd` function objects in every Realm. Exact admission opens 16
  previously skipped non-constructor/Symbol abrupt tests while retaining six
  real IsHTMLDDA skips. The focused scope moves from **9 pass / 80 fail / 22
  skip** to **105/0/6**, and complete Annex B moves from **842/170/74** to
  **938/90/58**.

- Moved intrinsic roots, module records, tagged-template identities, and the
  host module referrer into a `RealmRecord` owned by each global Environment.
  Published Realms are traced through that owner instead of VM-wide registry
  roots, so unreachable ShadowRealms and their lookup indexes can be reclaimed.
  ShadowRealms receive isolated module caches; `$262.createRealm` shares its
  host cache to preserve existing dynamic-import identity policy.
  `ShadowRealm.prototype.importValue` now evaluates relative modules in the
  target Realm, transfers primitives and fresh callable wrappers, and rejects
  missing/object exports or target failures with a caller-Realm `TypeError`.
  Its internal microtask and top-level-await continuation do not invoke an
  observable `.then`. Pinned ShadowRealm is **64 pass / 0 fail / 0 skip**.
  Commit `9409531` passes ordinary CI `30690961696` (**3/3**) and full matrix
  `30690961694` (**44/44**). Against `30687712195`, the contention-normalized
  result set is byte-identical for **31/32** artifacts; only `built-ins` moves
  **+4 pass / -4 skip**. Aggregate is **33,170 pass / 4,285 fail / 11,011
  skip**, with 3 timeout and 0 error over 48,469 files.

- Added the non-module ShadowRealm runtime: Realm-local constructors and
  prototypes, isolated `evaluate`, primitive transfer, and non-constructable
  `FunctionKind::Wrapped` membranes for callable arguments and results.
  Catchable target exceptions and object values are replaced by caller-Realm
  `TypeError` objects; initial parse failures remain caller-Realm
  `SyntaxError`s. Evaluation uses a fresh lexical environment while sloppy
  global `var`/function declarations publish with global binding descriptor
  rules. Secondary Realms now expose the same scalar globals and JSON namespace
  required by the main Realm. Pinned Test262 moves from **0 pass /
  54 fail / 10 skip** to **60 pass / 0 fail / 4 skip**. The Realm-owned
  `importValue` entry above subsequently closes those remaining four.

- `Map.prototype` and `Set.prototype` now expose their specified own
  `Symbol.toStringTag` data properties in every Realm. The properties are
  non-writable, non-enumerable, configurable, and continue to drive
  `Object.prototype.toString`; deletion falls back to `"Object"` and a later
  redefinition is observed. The three previously failing pinned Test262 files
  now pass, and the complete Map/Set scope is **498 pass / 0 fail / 89 skip**
  over 587 files. Full CI hard-gates the exact three-file boundary.

- Completed the pinned Test262 class-elements boundary. The anonymous
  default-export class already receives the inferred name `"default"` before
  public static field initialization; a module-graph regression now fixes that
  ordering. Runner and analyzer share one exact module admission for the sole
  previously skipped file, while tooling freezes its live metadata and rejects
  future, mirrored, and extra-feature cases. The two class-elements directories
  move from **2,961 pass / 0 fail / 1 skip** to **2,962/0/0**. Full CI now
  preflights the exact admission against pinned Test262. Supported statements/
  expressions move from **12,765/0/7,674** to **12,766/0/7,673** over 20,439
  files, exactly **+1 pass / -1 skip**. Implementation commit `b5a391f` and
  inaccessible-metadata-root regression `a048b18` pass ordinary CI
  `30680109952` (**3/3**) and full matrix `30680109971` (**45/45**). Against
  `30676958637`, **31/32** result artifacts are byte-identical; only
  language/expressions moves **+1 pass / -1 skip**. Aggregate is **33,103 pass /
  4,342 fail / 11,021 skip** over 48,469 files.

- Moved compiler-generated destructuring, iterator, Reference, and switch
  completion state from synthetic Environment bindings into dense `CallFrame`
  slots. Nested defaults, host reentry, class/with environment changes, and
  generator or async suspension now preserve independent outer state without
  leaking hidden global bindings or retained values. Completed generators also
  release their activation environment, arguments, receiver, resume value, and
  saved control state, lexical closure, and vector capacity; return/throw
  payloads stay pinned through result and error materialization. Direct
  regressions cover normal and abrupt iterator
  close, identifier/member/private/super targets, switch completion, reentry,
  suspension, and GC. The pinned assignment-destructuring/with/for-in/for-of
  boundary remains **1,233 pass / 0 fail / 190 skip**, and supported statements/
  expressions remains **12,765/0/7,674**; no Test262 admission policy changed.
  Commit `153ad4b` passes ordinary CI `30676958635` (**3/3**) and full matrix
  `30676958637` (**45/45**). All 32 Test262 result artifacts are byte-identical
  to `30670755993`; aggregate remains **33,102/4,342/11,022**, with 3 timeout
  and 0 error.

- Completed primitive-base Reference Test262 admission. The existing exact
  manifest now includes the final `GetValue` sibling that exercises Number,
  String, Boolean, and Symbol primitive boxing. Runner and analyzer retain the
  broad `Symbol` and `Proxy` gates outside these four frozen paths, while
  cross-Realm metadata is removed only for the two listed Realm cases. Tooling
  and full CI verify the exact path set, pinned live metadata, shared policy,
  future-sibling rejection, and the complete focused result. On pinned Test262,
  `language/types/reference` moves from **28/0/1** to **29/0/0**, and the
  combined Reference/with/compound-assignment boundary moves from
  **663/0/1** to **664/0/0**. Commit `8870346` passes ordinary CI
  `30670755996` (**3/3**) and full matrix `30670755993` (**45/45**). Against
  `30666470497`, **31/32** artifacts are identical; only language/types moves
  by **+1 pass / -1 skip**. Aggregate is **33,102 pass / 4,342 fail / 11,022
  skip / 3 timeout / 0 error**.

- Completed the exact `Function.prototype.toString` Test262 boundary. A frozen
  35-file path-to-feature manifest admits already conforming async, generator,
  private-method, Proxy, Reflect, and intrinsic traversal cases without
  removing their broad feature gates. Capture-free regular expressions now use
  a direct `regex-automata` matcher with one explicit, finitely charged
  hybrid cache. Conservative allocator overhead is included; inefficient cache
  clears permanently switch to a finitely charged PikeVM program with
  per-call scratch. Hot small Rust matchers and large logical UTF-16 programs
  coexist without cache thrash, cache-state-dependent semantics, or a higher
  sandbox retained-memory ceiling. The exact directory moves from **45/0/35** to
  **80/0/0**, and complete `built-ins/Function` moves from **460/0/49** to
  **495/0/14**, exactly **+35 pass / -35 skip**. Admission tooling safely
  handles absent or inaccessible external Test262 roots while full CI still
  validates the pinned live metadata. Commits `c3d219e` and `f6564fa` pass
  ordinary CI `30666470505` (**3/3**) and full matrix `30666470497`
  (**45/45**). Against `30655563999`, only built-ins changes by **+35/-35**;
  aggregate is **33,101 pass / 4,342 fail / 11,023 skip / 3 timeout / 0 error**.

- Derived-constructor postcondition errors now use the Realm of the execution
  context that resumes construction. A foreign constructor that returns a
  primitive or leaves `this` uninitialized therefore creates its `TypeError`
  or `ReferenceError` in the caller Realm, while errors and explicit throws
  from inside the body retain callee-Realm provenance and identity. Bound and
  transparent Proxy forwarding, foreign `newTarget`, borrowed foreign
  `Reflect.construct`, and forced GC preserve that split. On pinned Test262,
  the two exact cross-Realm Construct files move from **0/2** to **2/0**,
  `Function/internals/Construct` moves from **2/2/2** to **4/0/2**, and full
  `built-ins/Function` moves from **419/41/49** to **421/39/49**, exactly
  **+2 pass / -2 fail**. The supported statements/expressions subset remains
  **12,765/0/7,674**; no admission metadata changed. Implementation commit
  `a542780` passes ordinary CI `30648115509` (**3/3**) and full matrix
  `30648114517` (**45/45**). Against `30643371359`, **31/32** result artifacts
  are byte-identical and only built-ins changes by **+2/-2**. Aggregate is
  **33,025 pass / 4,383 fail / 11,058 skip / 3 timeout / 0 error**.

- Function `name` and `length` now remain deleted after their configurable own
  properties are removed. Property reads no longer reconstruct those values
  from internal function metadata, so inherited accessors and later
  `defineProperty` calls follow ordinary object semantics. `%Function.prototype%`
  now has the specified empty own `name` in both the main and Test262-created
  Realms. On pinned Test262, the four exact metadata tests move from **0/4** to
  **4/0** and full `built-ins/Function` moves from **415/45/49** to
  **419/41/49**, exactly **+4 pass / -4 fail**. The supported
  statements/expressions subset remains **12,765/0/7,674**; no admission
  metadata changed. Implementation commit `e93cfd1` passes ordinary CI
  `30643371448` (**3/3**) and full matrix `30643371359` (**45/45**). Against
  `30638309791`, **31/32** result artifacts are byte-identical and only
  built-ins changes by **+4/-4**. Aggregate is **33,023 pass / 4,385 fail /
  11,058 skip / 3 timeout / 0 error**.

- Implemented `for await...of` context early errors and specification-ordered
  `AsyncIteratorClose`. Script code now requires an async function, Module code
  permits top-level use, nested ordinary functions and class static blocks
  reject it, and every `for await` form other than `for...of` is rejected.
  Abrupt loop exits call and await the iterator's `return`, preserve an original
  throw over close errors, and let close errors replace break, continue, or
  return completions. Async-from-sync close, Module evaluation, and async
  generator requests share the same behavior. Finally guards now restore saved
  stack/environment state and distinguish control transfers that stay inside an
  outer guarded region from those that leave it. Pinned Test262 remains **5/0**
  for the direct AsyncIteratorClose files, **23/0/1,211** for all 1,234
  for-await files, **24/0** for Module for-await syntax, and **12,765/0/7,674**
  for the supported statements/expressions subset; no admission changed.
  Implementation commit `b33df0d` passes ordinary CI `30638309771` (**3/3**)
  and full matrix `30638309791` (**45/45**). All 32 Test262 result artifacts
  are byte-identical to `30631392782`; aggregate remains **33,019 pass / 4,389
  fail / 11,058 skip / 3 timeout / 0 error**.

- Implemented Annex B.3.9 runtime errors for sloppy ordinary call assignment
  targets across assignment, arithmetic/bitwise compound assignment, update,
  and `for-in/of`. Strict and Module code, logical assignment, and optional
  chains retain early errors. General expression forms preserve a call's own
  abrupt completion; loop heads replace it with `ReferenceError` before
  iterator close. Finally state is now guard-scoped and survives nested
  finally/catch control flow, generator/async suspension, and GC. Direct
  break/continue unwind only exited catch/finally regions, and nested function
  compilation uses independent control stacks. Pinned Test262 moves the exact
  cohort from **0/7** to **7/0** and full Annex B from **833/179/74** to
  **840/172/74**; the core 324-file assignment-target cohort remains
  **316/0/8**, and the supported subset remains **12,765/0/7,674**. Ordinary
  CI `30631392802` passes 3/3 and full matrix `30631392782` passes 45/45.
  Against `30624215959`, 31/32 result artifacts are byte-identical and only
  Annex B changes by **+7/-7**. Aggregate is **33,019 pass / 4,389 fail /
  11,058 skip / 3 timeout / 0 error**; filename-sorted content hash is
  `5dfbb2cc10a220e0443a647dfa15ee03607f0e26f17e80696f46d7fd397cc1f4`.

- Implemented Annex B.1.1 HTML-like comments for Script, eval, and dynamic
  Function source while preserving the Module lexical grammar. `<!--` consumes
  the rest of its line in Script-derived goals; `-->` does so only at the
  initial Script goal or after an inter-token line terminator, including one in
  a multiline comment. A dedicated admission state prevents string line
  continuations and template-internal newlines from enabling `-->`. Module code
  continues to tokenize the same characters as ordinary operators. Dynamic
  Function parameter and body parses retain their distinct newline boundaries.
  On pinned Test262, the 14-file HTML-comment cohort moves from **1/13** to
  **14/0**, five core negative guards remain **5/0**, and full Annex B moves
  from **820/192/74** to **833/179/74**, exactly **+13 pass / -13 fail**. The
  supported statements/expressions subset remains **12,765 pass / 0 fail /
  7,674 skip / 20,439 total**; no admission metadata changed. Ordinary CI
  `30624216090` passes 3/3 and full matrix `30624215959` passes 45/45. Against
  `30619831846`, 31/32 result artifacts are byte-identical and only Annex B
  changes by **+13/-13**. Aggregate is **33,012 pass / 4,396 fail / 11,058 skip
  / 3 timeout / 0 error**; filename-sorted content hash is
  `afbfc79705afc42c18701784fe23eb65aed8c50488a24553ff3caf71fe272156`.

- Implemented Annex B.3.4 direct-eval declarations across a matching simple
  catch parameter. Sloppy eval `var` initializers now update the active catch
  binding while installing the variable-environment binding, and ordinary,
  generator, async, and async-generator function declarations use the same
  admission scan. Destructuring catch parameters, intervening lexical bindings,
  and function-body top-level lexicals still reject conflicting eval
  declarations; Object Environment Records do not create false conflicts.
  Catch early errors now reject destructuring-parameter collisions with nested
  `var` declarations without misclassifying named class expressions. On pinned
  Test262, the exact direct-eval file moves from **0/1** to **1/0**, all 309
  direct-eval Annex B files pass, and full Annex B moves from **819/193/74** to
  **820/192/74**. The supported statements/expressions subset remains **12,765
  pass / 0 fail / 7,674 skip / 20,439 total**; no admission metadata changed.
  Ordinary CI `30619831879` passes 3/3 and full matrix `30619831846` passes
  45/45. Against `30615320744`, 31/32 result artifacts are byte-identical; the
  clean Annex B result changes exactly **+1 pass / -1 fail**. Aggregate is
  **32,999 pass / 4,409 fail / 11,058 skip / 3 timeout / 0 error**, with
  filename-sorted content hash
  `85c4b611b5bd310546f4c7870dee2555d62b2bb7957d8049d29c6d1117c0ab26`.

- Implemented Annex B.3.5 initializers for sloppy `for-in` heads containing one
  simple `var` binding. The initializer resolves and updates its binding exactly
  once before RHS enumeration, preserves anonymous function naming, and blocks
  RHS evaluation on abrupt completion. Parenthesized expressions restore the
  `+In` grammar parameter. `var` keys in both `for-in` and `for-of` now assign
  through the shared Reference/PutValue path instead of creating a transient
  lexical binding, so `with`, global objects, and direct eval observe the right
  binding. The unused PutValue result is discarded to keep abrupt iterator-close
  stacks balanced. Pinned Test262 moves the seven-file B.3.5 cohort from
  **6/1** to **7/0** and full Annex B from **818/194/74** to **819/193/74**;
  the supported statements/expressions subset remains **12,765 pass / 0 fail /
  7,674 skip / 20,439 total**. Ordinary CI `30615320781` passes 3/3 and full
  matrix `30615320744` passes 45/45. The CI Annex B shard had two
  load-sensitive timeouts; replacing only that result with the clean exact
  rerun leaves 31/32 artifacts byte-identical to `30610833027` and changes
  Annex B by exactly **+1 pass / -1 fail**. Corrected aggregate is **32,998
  pass / 4,410 fail / 11,058 skip / 3 timeout / 0 error**; filename-sorted
  result content hash is
  `268a045ffeffdc7395bac7aad9703f28d80a2e7dbbe8cb13a4f801c5948943a4`.

- Implemented Annex B.3.3 sloppy `if`-clause function declarations by lowering
  each ordinary declaration to the sole item of a synthetic block. Strict,
  Module, generator, async, labelled, and other single-statement positions keep
  their early errors, while the existing Annex B.3.2 declaration plan supplies
  the required lexical binding and conditional outer mirror. Consecutive labels
  are now parsed iteratively with a nesting bound, all labels around an
  iteration share one target, and break/continue distinguish iteration, switch,
  and non-loop label frames without leaking pending labels through nested
  function compilation. On pinned Test262, the exact 480-file B.3.3 cohort
  moves from **0/480** to **480/0** and full Annex B from **338/674/74** to
  **818/194/74**. The supported statements/expressions subset remains
  **12,765 pass / 0 fail / 7,674 skip / 20,439 total**. Ordinary CI
  `30610833014` passes 3/3 and full matrix `30610833027` passes 45/45. Against
  `30606086846`, 31/32 result artifacts are byte-identical and only Annex B
  changes by **+480 pass / -480 fail**. Aggregate is **32,997 pass / 4,411
  fail / 11,058 skip / 3 timeout / 0 error**; filename-sorted result content
  hash is `0e8c0315babb6e80150923cc8643717b3d07801b22996243aca3e4fa10972ea0`.

- Implemented Annex B.3.2 block-level function outer-variable semantics for
  Script, FunctionBody, direct eval, indirect eval, and created Realms. Block
  and CaseBlock functions now remain lexical while an independently admitted
  `var` binding is hoisted and updated only when its declaration is evaluated.
  Admission rejects parameter, `arguments`, lexical, destructuring-catch, and
  restricted/non-extensible-global conflicts; simple catch and object
  environments retain their legacy exceptions. Global updates use the shared
  Reference/PutValue path, preserving accessors, non-writable descriptors, and
  foreign-Realm global identity. Pinned Test262 moves the exact 296-file
  block/switch cohort from **161/135** to **296/0** and full Annex B from
  **203/809/74** to **338/674/74**, while the supported subset remains
  **12,765 pass / 0 fail / 7,674 skip / 20,439 total**. Ordinary CI
  `30606086847` passes 3/3 and full matrix `30606086846` passes 45/45.
  Against `30602031022`, 31/32 result artifacts are byte-identical and only
  Annex B changes by **+135 pass / -135 fail**. Aggregate is **32,517 pass /
  4,891 fail / 11,058 skip / 3 timeout / 0 error**; filename-sorted result
  content hash is `4b23c22b5bd00fdc1989c9ff1233ba3fde8fa425301b09941de94b732f500883`.

- Implemented the Annex B host exception for duplicate lexical names in sloppy
  Blocks and CaseBlocks. Duplicate names are accepted only when every binding
  is an ordinary `FunctionDeclaration`; strict code and generator, async,
  class, lexical, or `var` mixtures remain early errors. CaseBlock
  `VarDeclaredNames` now include `for` declaration heads recursively, and
  duplicate switch functions install the source-order final function in the
  shared CaseBlock binding. The two exact pinned Test262 files pass **2/2**;
  Annex B moves to **203 pass / 809 fail / 74 skip**, exactly **+2 pass / -2
  fail**, while the supported subset remains **12,765 / 0 / 7,674**.

- Corrected declaration static semantics by classifying top-level function
  declarations from their source grammar: Script and FunctionBody declarations
  are var-scoped regardless of strictness, while Module and Block declarations
  are lexical. This closes the 37 dynamic-import and six function-statement
  strict variants left by dual Test262 execution, restoring the pinned
  supported subset to **12,765 pass / 0 fail / 7,674 skip / 20,439 total**.
  FunctionBody lexical/var intersections are rejected during parsing. Ordinary
  CI `30598482443` passes all 3 jobs and full matrix `30598482422` passes all
  45 jobs. After replacing two load-sensitive Annex B timeouts with the clean
  exact-corpus rerun, the 32 artifacts total **32,380 pass / 5,028 fail /
  11,058 skip / 3 timeout**. The deterministic delta is **+54 pass / -54
  fail** across six shards; 26/32 artifacts are byte-identical and corrected
  content hash is `fec150f12df228289b84453363a62c41fbfe82dd9b15592b20c7f1c2c68ee8f0`.

- Completed FunctionDeclarationInstantiation for functions with parameter
  expressions. Their body var/function names are instantiated in the separate
  body environment, same-named initialized parameter values are copied before
  body function installation, and parameter-initializer closures keep the
  outer parameter binding. Destructuring carriers now use a non-source internal
  namespace, preventing collisions with legal names such as `__arg0`. Tests
  cover simple, default, rest, destructured, strict, Module, arrow FunctionBody,
  and stack-balanced function installation.

- Corrected Test262 execution semantics so files without a strictness flag run
  once in non-strict mode and once in strict mode, while `onlyStrict`,
  `noStrict`, `module`, and `raw` retain their required single execution.
  Variants run in independent RuJa processes with independent timeout,
  negative, and async classification, then collapse into one file-level result
  with variant-labeled analyzer diagnostics. The analyzer now retains
  timeout/error files and
  the legacy failure report delegates to the canonical analyzer path. This
  exposed 89 previously hidden strict failures. The accompanying `var`
  destructuring fix closes 46; pinned supported-subset truth is now **12,722
  pass / 43 fail / 7,674 skip / 20,439 total (99.7% of run)**.
  Tooling regressions inject an isolated temporary harness, avoiding dependence
  on a developer-specific default Test262 path.

- Fixed strict `var` destructuring declarations and `for-of` heads to assign
  extracted values through normal binding Reference resolution instead of
  attempting lexical initialization in the current environment. This closes the
  strict-only binding failure exposed in three admitted Intl.Collator tests and
  the related language cohort. Both strict and sloppy hoist collectors now
  include destructuring names, preserving function scope without global leaks.
  A `with` regression freezes that value extraction precedes binding resolution.
  Ordinary CI `30594697312` and full matrix `30594697340` pass. The final 32
  artifacts report **32,326 pass / 5,082 fail / 11,058 skip / 3 timeout**;
  26/32 are byte-identical to the preceding single-execution baseline.

- Admitted the final four implemented class built-in subclass tests for
  `SharedArrayBuffer` and `WeakRef` through one exact path-to-feature map shared
  by the Test262 runner and analyzer. The map covers declaration and expression
  forms only; future siblings, outside paths, and files with any additional
  unsupported feature remain skipped. The pinned exact cohort passes
  **4/4**, and the complete two `class/subclass-builtins` directories now pass
  **72/72** with no skips. The supported language subset advances to
  **12,765 pass / 0 fail / 7,674 skip** over 20,439 files. Full-matrix setup
  runs the live metadata test against its pinned checkout before fan-out.

- Added a bounded VM-local cache for reusable compiled RegExp matchers without
  changing `RegExpBuiltinExec` observation order. Keys contain immutable source,
  only compiler-semantic `i/m/s/u/v` flags, and the scalar/code-unit/logical
  UTF-16 input domain; `d/g/y` and Realm identity do not split equivalent
  entries. Cache hits clone RuJa-owned `Arc` handles without allocating, while
  publication is best-effort and never replaces a successful compilation with
  an error. LRU retention is bounded by entry, source, and conservative matcher
  budgets. Only small capture-free Rust matchers and bounded logical UTF-16
  matchers are admitted; fancy, composite, captured, and oversized matchers run
  uncached because their retained scratch pools lack a finite public bound.
  Tests cover semantic keys, constructor seeding, GC and cross-Realm reuse,
  coercion order, reentrant eviction, every backend policy, budget eviction,
  publication failure, and compile-failure retry.

- Made the `RegExpBuiltinExec` terminal compilation boundary deterministically
  testable without bypassing real backend selection or fallback. The test-only
  countdown can replace only a successfully compiled matcher with a typed
  resource failure; genuine compiler errors preserve the pending injection.
  Tests freeze input and `lastIndex` ordering, global/sticky short circuits,
  method-Realm errors, re-entry in both directions, backend variants,
  post-compile Fuel/materialization priority, unchanged state, and immediate
  retry. `RegExp.prototype.test` now uses dynamic `RegExpExec`, observing custom
  `exec` getters and call results in specification order.

- Split RegExp syntax diagnostics from compiler and backend resource limits.
  Dynamic construction now validates flags before bounded pattern work,
  reports local Unicode, Rust regex, fancy-regex, and regress construction
  limits as active-Realm `RangeError`, and preserves malformed patterns as
  `SyntaxError` unless an earlier source cap is required to avoid allocation.
  Literal flags consume the complete `IdentifierPart` run before bounded
  pattern work. The vendored logical backend exposes a typed resource-limit
  bit instead of requiring message matching; Rust and fancy adapters classify
  their structured size, nesting, and runtime-work variants. All constructor,
  exec, search, match, and legacy replacement compile call sites share the
  same mapper, while a successful alternate backend remains a valid fallback.

- Hardened builtin `RegExp.prototype[Symbol.replace]` replacement
  materialization. Collected results, captures, callback arguments,
  substitution scratch, final UTF-16 output, and UTF-8 decoding now reserve
  fallibly and consume Fuel before native work. Input slicing and Unicode
  empty-match advancement share one ASCII-borrowed or fallibly cached UTF-16
  source, avoiding repeated full-string scans and 32-bit index overflow.
  Existing strings remain shared through `Arc<str>` for callback publication;
  result collection completes before callbacks, backward matches still run
  substitutions before output suppression, and reservation failures preserve
  prior observable effects. Deterministic tests cover every growth site,
  conditional bypass, Realm identity, GC/pin cleanup, retry, ordering, large
  `lastIndex`, UTF-16 sentinel parity, and exact template/callback Fuel bounds.

- Made builtin RegExp `exec` post-match native containers allocator-fallible
  and fuel-metered. Capture ranges, endpoint sorting and UTF-16 offset maps,
  result values and Array presence bitmaps, named groups, match-indices pair
  Arrays and groups, and final result properties reserve before mutation and
  report catchable `RangeError` on growth failure. Named-group maps are built
  before heap publication, preserving duplicate-name insertion order while
  replacing only an earlier unmatched value. Global and sticky `lastIndex`
  now publish the matched end before all result materialization, matching
  `RegExpBuiltinExec`; later resource failure does not roll that side effect
  back. Deterministic tests cover every reservation site, nested countdowns,
  GC/pin cleanup, immediate retry, Realm identity, conditional branches, and
  Fuel-before-reservation plus capture-name hashing boundaries.

- Made every temporary GC-root publication in the RegExp core fallible before
  mutation. Constructor, search, match, matchAll, split, replacement, iterator,
  `toString`, exec, groups, and match-indices paths now reserve exact value-root
  capacity before pinning; `@@search`, `@@match`, and `toString` also retain
  previously unrooted fresh getter/exec results across later observable calls.
  Match-indices preflights all future pair and groups roots as one batch, so a
  reservation failure cannot expose partial nested materialization. Forced-GC
  and countdown sweeps cover every reachable reservation, pin cleanup, abrupt
  ordering, and retry.

- Froze the independently complete RegExp named-capture surface into an exact
  86-file Test262 admission. The boundary covers named-group execution,
  replacement coercion, Unicode and non-Unicode names and references, and
  RegExp-literal early errors while leaving `poisoned-stdlib.js` behind its
  separate `Symbol.iterator` gate. A dedicated full-workflow job requires the
  exact boundary at **86/0/0** and the related 101-file scope at **100 pass /
  0 fail / 1 explicit skip**; future sibling paths remain skipped.

- Hardened `WeakRef` and `FinalizationRegistry` as a complete frozen
  weak-reference unit. Constructor fallback now uses immutable Realm-local
  prototype registries and both branded objects allocate through GC retry.
  WeakRef job-kept roots use a fallible O(1) identity set. FinalizationRegistry
  stores and traces its constructor Realm, marks pending cleanup during sweep,
  and enters that Realm for its cleanup job. Cell growth, unregister scans,
  cleanup selection, nested callback roots, and job scheduling are bounded or
  fallible; cleanup removes and invokes one cell at a time, respects
  callback-time `unregister`, contains catchable callback throws, and
  propagates non-catchable Fuel aborts without losing later cells.
  The exact pinned Test262 manifest freezes all 29 WeakRef and 47
  FinalizationRegistry files with their audited metadata at **76/0/0**; a
  dedicated full-workflow job also requires the complete two-directory scope
  to remain **76 pass / 0 fail / 0 skip**.

- Added Realm-local, callable/constructable `Intl.Collator` backed by pinned
  ICU4X 2.2 compiled collation data. Construction implements ordered locale and
  option coercion, `co`/`kn`/`kf` negotiation, subclass and constructor-Realm
  prototype behavior, cached non-constructible bound compare functions,
  UTF-16 comparison, fresh Realm-local `resolvedOptions`, and exact Thai,
  numeric, case, sensitivity, punctuation, German phonebook/search behavior.
  `String.prototype.localeCompare` now constructs the method Realm's immutable
  intrinsic Collator without consulting a mutable global binding. Collator
  objects and compare cycles are GC-traced and comparison work is fuel-metered
  with fallible UTF-16 buffers. The frozen Test262 boundary passes **74/0/0**;
  the 75-file scope is **74 pass / 0 fail / 1 skip**, where the held shared
  harness requires the absent NumberFormat and DateTimeFormat constructors.

- Added Realm-local `Intl.supportedValuesOf` with observable `ToString`, exact
  key validation, fresh Arrays, and deterministic sorted data for the 16
  required calendars, 78 simple-digit numbering systems, 445 primary IANA time
  zones, 45 sanctioned simple units, and 10 provider-validated collations.
  Currency remains empty until its formatter providers exist, so the API does
  not advertise unsupported service behavior. Result publication is fuel-precharged,
  fallible, rooted across GC retry, and covered by reservation and exact heap
  cap failures. A frozen 16-file Test262 manifest passes **16/0/0**; the full
  25-file directory reports **16 pass / 0 fail / 9 formatter-dependent skip**.

- Completed the `Intl.Locale-info` surface. `Intl.Locale` now observes and
  canonicalizes `firstDayOfWeek`, exposes its branded accessor, and implements
  `getCalendars`, `getCollations`, `getHourCycles`, `getNumberingSystems`,
  `getTimeZones`, `getTextInfo`, and `getWeekInfo` with method-Realm Arrays and
  ordinary result objects. A deterministic generator pins Unicode CLDR 48.2
  calendar preferences, hour cycles, week data, script direction metadata,
  and canonical IANA region time zones; ICU4X supplies extended likely
  subtags. Region selection follows region, subdivision, likely-subtag, world
  fallback, and independent region-override precedence while preserving CLDR
  `001` inheritance. Native scans are fuel-precharged and result objects remain
  rooted across GC retry. A separate 52-file manifest expands the exact
  `Intl.Locale` boundary to **161 pass / 0 fail / 0 skip**, with an independent
  **52/0/0** Locale-info CI assertion.

- Added the base `Intl.Locale` surface with a Realm-local constructor and
  prototype, an unforgeable structural heap brand, canonical locale internal
  slots, ordered language/Unicode option processing, branded accessors,
  subclassing, `toString`, and ICU4X-backed `maximize`/`minimize`. Locale
  objects now take the internal-slot fast path in `CanonicalizeLocaleList`, so
  patched subclass methods are not observed. Construction uses eager
  `newTarget.prototype` observation with constructor-Realm fallback; all
  provisional intrinsic objects remain rooted across GC retry. Option and
  likely-subtag scans consume sandbox fuel before native work. A frozen
  109-file Test262 manifest admits every base `Intl.Locale` file plus the
  adjacent canonical-locale-list case at **109 pass / 0 fail / 0 skip**. The
  later Locale-info manifest now extends that frozen base without changing its
  historical ownership.

- Added the Realm-local `%Intl%` namespace and `Intl.getCanonicalLocales`.
  Locale-list coercion follows `CanonicalizeLocaleList` ordering, including
  String singleton handling, one `length` read, `HasProperty` before `Get`,
  object-only `ToString`, stable post-canonicalization deduplication, and a
  fresh result Array from the method Realm. Locale syntax and CLDR
  language/script/region/variant aliases use pinned ICU4X `icu_locale` 2.2.0;
  valid reserved 5-8-letter language subtags and numeric extension singletons
  are adapted around ICU4X parser limitations. A deterministic generator pins
  Unicode CLDR 48.2 commit
  `11299982335beb974c1c63c45265184e759c0f41` and supplies the complete
  structurally usable Unicode and transformed-extension type aliases missing
  from ICU4X without formatter-version input, while ICU4X supplies subdivision
  aliases and extended likely subtags. Transform fields are preserved outside
  ICU's unique-key map so repeated valid tkeys survive canonicalization. Index
  scans consume sandbox fuel, and locale parsing precharges input length before
  scanning plus quadratic subtag work before ICU sorted insertion. `%Intl%`, its
  `@@toStringTag`, cross-Realm Array/error behavior, coercion order, GC, and
  fuel recovery have direct tests. The frozen Test262 boundary admits exactly
  40 files, keeps future in-scope files closed until explicit admission, and
  delegates the adjacent Locale-object case to the exact Locale manifest;
  its dedicated gate reports **41 pass / 0 fail / 0 skip** over 41 files.
  Formatter constructors and the remaining ECMA-402 surface stay separately
  gated. The package now declares
  Rust 1.88 as its MSRV and CI checks it explicitly.

- Static import and re-export declarations now accept Import Attributes
  `with { ... }` clauses. Parsed ModuleRequest records retain decoded,
  duplicate-checked attributes in UTF-16 key order, and source requests are
  deduplicated only when both specifier and attributes match. The relative-file
  host recognizes `type: "json"` and `type: "text"`, rejects unsupported keys
  or values before loading, parses JSON once during graph resolution, and gives
  JavaScript, JSON, and text views distinct canonical cache identities. Static
  and dynamic imports share the same typed ModuleRecord and namespace, so JSON
  default objects preserve identity across import sites. Data modules initialize
  a rooted internal default value directly instead of evaluating synthetic
  source through observable global `JSON.parse`. Tests cover every import/re-export
  grammar form, decoded duplicate keys, attribute ordering, JSON/text/default/
  namespace semantics, re-exports, static/dynamic identity, self-text imports,
  poisoned globals, cache separation, invalid JSON, named-import rejection, and
  unsupported host attributes. The exact pinned Test262 boundary is **30/30**;
  broad module, source-phase, deferred-import, and bare-specifier gates remain.
  First loads use the request Realm's Object/Array prototypes; JSON construction
  roots partial trees, releases overwritten duplicate-key subtrees before
  parsing replacements, and uses the ordinary heap-limit GC retry. Final local
  gates pass all-target/all-feature library **367/367**, release library
  **367/367**, modules **33/33**, Python tooling **146/146** with four expected
  absent-checkout skips, rustfmt, warnings-denied Clippy, wasm32, doctest
  **1/1**, and every benchmark smoke target. The exact pinned Test262 run is
  **30/30** on revision `9e61c128`; two final GPT-5.6 reviews are CLEAN.
  Tooling treats an inaccessible default Test262 path as an absent checkout,
  while explicit CI checkouts still require exact live-manifest equality.
  Implementation commit `aa62191` and portability fix `cbd28fc` are pushed;
  ordinary CI `30467208759` passes **2/2** and all **39/39** jobs in full run
  `30467210702` pass. Against clean run `30454181980`, **30/32** Test262 result
  files are byte-identical. `language/import` is exactly **+17 pass/-17 skip**
  and `language/module-code` is **+13 pass/-13 skip**; aggregate is
  **32290 pass / 5028 fail / 11148 skip / 3 timeout / 0 error** over 48469
  files with 37318 run, exactly **+30 pass/-30 skip** and no regressions.

- Finite-budget incremental GC now cursorizes ordered Map entries, charging one
  work unit for each key/value record while scanning each slice under one lock.
  The cursor snapshots entry count and counts down, so append cannot extend a
  pass and removed records still consume bounded work. Mutation access schedules
  fresh retraces for replacement, shift removal, clear, and reinsertion, while
  read-only access paths cover collection reads, ordinary Get/HasProperty,
  GetPrototypeOf, iterator brands, and TypedArray iterable probes so those
  operations do not repeatedly dirty an active Map cursor. Newly reached cells
  use the direct Map tracer for `usize::MAX`; previously parked cursors drain
  without yielding. Exact tests
  cover key/value roots, one-record accounting, batching, Mark growth, removed
  records, Retrace mutation, repeated direct and compiled JS observations,
  two-record LIFO order, and direct/pending-MAX liveness; focused GC is
  **48/48**. Same-host parent/current full-GC ranges overlap at
  **448.09-476.02 us** and **444.98-467.40 us**. The
  stateful budget-256 benchmark changed from atomic **459.93-466.37 us** to an
  amortized **754.22-891.68 ns** per invocation. Set
  generation/compaction state, WeakMap ephemerons, and LazyGenerator's ordered
  multi-vector state require separate cursor designs. Remaining read-only
  own-descriptor/key enumeration, extensibility/integrity, classification,
  Promise/await, RegExp/String/Array, and host-observer paths still use the
  conservative mutation access API and remain a separate barrier-classification
  unit. Final gates pass all-target/all-feature library **363/363**, release
  library **360/360**, focused GC **48/48**, Python tooling **145/145** with four
  absent-checkout skips, rustfmt, warnings-denied Clippy, wasm32, doctest, and
  every benchmark smoke target. The pinned 204-file Map sweep remains
  **144 pass / 1 fail / 59 skip**; the sole failure is the pre-existing
  `Map.prototype[Symbol.toStringTag]` descriptor. Implementation commit
  `ceee602` passes ordinary CI `30454181926` (**2/2**) and all **37/37** jobs
  in full run `30454181980`. Against preceding clean full run `30445593831`,
  all **32/32** result artifacts are byte-identical; aggregate remains
  **32260 pass / 5028 fail / 11178 skip / 3 timeout / 0 error** over 48469
  files, with 37288 run.

- Finite-budget incremental GC now cursorizes pending Promise handlers and
  FinalizationRegistry cells alongside Array and internal Iterator vectors.
  Each pass snapshots record count, charges removed or compacted slots, scans
  the current slice under one lock, and relies on the existing access barrier
  for growth, replacement, and registry `retain` compaction. Promise handler
  traversal is shared with the direct tracer through an always-inlined helper;
  registry targets and unregister tokens remain weak while held values remain
  strong. Exact tests cover multi-root ordering, one-record accounting,
  batching, Mark- and Retrace-phase growth, removal, settlement-style
  result/drain mutation, dirty revisits, cleanup-callback and held-only
  liveness, private-edge LIFO order, retain compaction, direct tracing, and
  `usize::MAX` completion. Dedicated budget-256 Criterion cases exercise the
  finite cursor path in addition to full-GC fast-path A/B fixtures.
  One handler's nested AsyncFunction stack/local/catch vectors and other large
  object payloads remain atomic follow-ups. Same-host quick full-GC A/B samples
  show Promise handlers at **1.3170-1.3337 ms** before versus
  **1.2913-1.3226 ms** after, and registry cells at **332.37-338.25 us** before
  versus **306.86-312.13 us** after; this is no-regression evidence rather than
  a general throughput claim. Local gates pass all-target/all-feature tests and
  release library tests with **353/353**, focused GC **39/39**, warnings-denied
  Clippy, rustfmt, wasm32, doctest **1/1**, all Criterion smoke targets, and
  Python tooling **145/145** with four expected absent-checkout skips. Pinned
  Promise/FinalizationRegistry Test262 is **489 pass / 0 fail / 287 skip** over
  776 files. Implementation commit `39cf24c` passes ordinary CI
  `30445593804` (**2/2**) and full CI `30445593831` (**38/38**). Against the
  preceding clean full run `30439701818`, all **32/32** result artifacts are
  byte-identical with content-set hash `22f72e0e...`; aggregate remains
  **32260 pass / 5028 fail / 11178 skip / 3 timeout / 0 error** over 48469
  files, with 37288 run.

- Finite-budget incremental GC now cursorizes dense Array and internal
  Iterator item vectors through the same LIFO work stack as cell headers.
  Each pass snapshots its vector length, counts down to retain prior child
  visitation order, charges removed slots, batches the current slice under one
  lock, and lets newly queued roots and discovered children preempt the parked
  continuation. Growth is caught by the fresh physical retrace or a
  deduplicated dirty revisit, including mutation during an already-running
  dirty vector pass. Active object callbacks now queue both root and dirty
  barriers before nested collection, and their RAII guard runs the post-access
  barrier on normal return or unwind. `usize::MAX` completes existing cursors
  but keeps the direct atomic vector tracer for newly reached objects, avoiding
  resumable-state overhead in full GC. Exact tests cover one-slot progress,
  finite batching, zero/MAX budgets, LIFO priority, Array/Iterator growth,
  Array shrink, repeated dirty replacement, active re-entry, and panic unwind.
  Other object payloads and sweep remain atomic. A reproducible 100k primitive
  dense-Array benchmark measures the parent collector at **238.29-242.58 us**
  and this implementation at **228.53-228.56 us** in sequential quick runs;
  this is no-regression evidence, not a general throughput claim. Local gates
  pass all-target/all-feature tests with library **339/339** and every
  integration/benchmark smoke target, release library **339/339**, focused GC
  **25/25**, warnings-denied Clippy, rustfmt, wasm32, doctest **1/1**, and
  Python tooling **145/145** with four expected absent-checkout skips. Pinned
  Array/Iterator/WeakMap/WeakSet/WeakRef/FinalizationRegistry Test262 is
  **3788 pass / 0 fail / 109 skip** over 3897 files. Implementation commit
  `6a715c1` passes ordinary CI `30439702247` (**2/2**) and all **38/38** jobs
  in full run `30439701818`. Against preceding clean run `30431870820`, all
  **32/32** result artifacts are byte-identical; aggregate remains **32260
  pass / 5028 fail / 11178 skip / 3 timeout / 0 error** over 48469 files with
  37288 run.

- Incremental GC now persists explicit Mark and Retrace phases. Finite budgets
  apply to newly traced cells and every physical cell visited by the pre-sweep
  mutation retrace instead of being disabled during finalization. Access to an
  already-scanned object through ordinary or private-element heap APIs queues
  one deduplicated dirty revisit, while current roots, active object accesses,
  and intervening allocations rejoin the ordinary worklist. Heap cells retain
  `Arc`-owned objects during callbacks, so re-entrant collection can trace the
  current mutated object instead of observing an empty cell. Tests cover
  `budget=0`, exact one-slot retrace progress, mutation after cursor passage,
  dirty deduplication, late roots, allocation, re-entrant collection,
  ephemeron chains, private elements, and final reuse. Sweep and non-Array/
  Iterator large-object tracing remain atomic, preserving the existing safety boundary without
  overstating full pause-time bounds. Local gates pass all-target/all-feature
  release tests with library **330/330**, focused GC **16/16**, warnings-denied
  Clippy, rustfmt, wasm32, doctest, Python tooling **145/145**, and pinned
  WeakMap/WeakSet/WeakRef/FinalizationRegistry **302/302**. Same-machine
  Criterion A/B shows computed Reference access about **7.7% slower** from the
  Arc atomic pair and the object-environment Reference workload about **60%
  faster** after removing complete HeapObj take/restore cycles.
  The previously public `Heap::cells`/`GcCell` representation is now private,
  and the unused `Heap::with_obj_mut` escape hatch is removed. Downstream
  low-level heap integrations must use barrier-aware `Heap` accessors;
  `with_obj` remains public and now exposes the live object during nested
  access. Callbacks must release object interior-mutex guards before invoking
  collection. Implementation commit `f4747ce` passes ordinary CI
  `30431870408` (**2/2**) and full matrix `30431870820` (**38/38**). Against
  preceding run `30428204190`, all **32/32** result artifacts are byte-identical
  at **32260 pass / 5028 fail / 11178 skip / 3 timeout / 0 error** over 48469,
  with 37288 run.

- The seven Set composition methods now implement the complete SetRecord and
  iterator protocol with cached `has`, `keys`, and `next` methods, Realm-local
  result Sets, original-completion-preserving `IteratorClose`, and GC roots for
  every observable intermediate. Set storage uses generation-ordered slots,
  tombstones, bounded stable compaction, and an average-O(1) key index, so live
  iteration observes deletion and reinsertion without repeated snapshots or
  quadratic removal. `Set.prototype.forEach` shares the rooted cursor path.
  Native traversal consumes Fuel per physical slot and uses fallible per-value
  roots and result growth. Exact metadata admission moves the seven pinned
  directories from **179 pass / 0 fail / 7 skip** to **186/0/0**; the complete
  383-file Set directory is **351 pass / 2 fail / 30 skip**. Direct tests cover
  receiver/result snapshot distinctions, callback mutation, delete/reinsert
  order, iterator caching and close, foreign Realms, forced GC, root/storage
  failpoint sequences, post-step close, exact-cap Set-result/pair failure,
  Fuel-atomic compaction/clear, pin cleanup, and clean retry.
  Implementation commit `9dc89b8` passes ordinary CI `30426806030` (**2/2**)
  and full CI `30426233118` (**38/38**), including the dedicated **186/186**
  Set-algebra gate. Against `30411263253`, normalized full-matrix movement is
  exactly **+7 pass / -7 skip** in built-ins; aggregate results are
  **32260 pass / 5028 fail / 11178 skip / 3 timeout / 0 error** over 48469,
  with 37288 run. Thirty artifacts are byte-identical, annexB differs only
  because two prior contention timeouts passed, and built-ins contains the
  intended admission.

- `WeakMap` and `WeakSet` now execute iterable constructors through direct,
  cached synchronous iterator records. They perform one `@@iterator` Get with
  no `HasProperty` probe, call cached `next` and adder functions with standard
  arity/receivers, meter every step, preserve catchable IteratorClose
  completions, and never close step or non-catchable Fuel failures. Every Realm
  owns immutable WeakMap/WeakSet prototype identities with rollback and
  constructor-Realm error/fallback behavior. Methods enforce their internal
  brands; WeakMap also implements `getOrInsert` and `getOrInsertComputed` with
  callback re-entry semantics. Object, well-known Symbol, and non-registered
  Symbol keys are accepted while `Symbol.for` keys are rejected. Weak storage
  uses fallible `HashMap`/`HashSet` growth and an O(1) registered-Symbol index,
  while leaving duplicates/replacements allocation-free. GC now resolves
  reachable WeakMap ephemerons through key-indexed pending values instead of
  repeated whole-map scans. Finite-budget marks snapshot roots once, deduplicate
  the worklist, queue intervening allocations, resume across calls, and retrace
  current host roots and marked cells before sweep. Allocation publication and
  its mark barrier share collector lock order, so intervening mutations remain live, preventing
  values from being freed and later aliased through reused heap cells.
  Exact 95-file admission moves pinned WeakMap/WeakSet from **55 pass / 76 fail
  / 95 skip** to **226/0/0**; forced execution moves from **91/135** to
  **226/0**. Adjacent WeakRef/FinalizationRegistry remains **76/76**. Direct
  tests cover Proxy order, caching, built-in overrides, close priority, Realm
  identity, brands, Symbol classes, callback-first upsert validation and
  re-entry, forced and incremental GC, ephemeron chains, every reservation
  site, duplicate/update no-reserve behavior, Fuel,
  exact heap-cap retry, pin cleanup, and clean retry.

- `new Set(iterable)` now uses a direct cached synchronous iterator record
  instead of allocating a wrapper. It performs one `@@iterator` Get without a
  `HasProperty` probe, calls cached `next` with zero arguments, observes
  Array/Map/Set/generator overrides, and meters each step. Step/result/done/
  value failures and non-catchable host Fuel do not close; catchable adder
  failures close while preserving and rooting the original completion, with
  native errors materialized in the constructor Realm. Every Realm now owns
  immutable Set, Set prototype, and Set Iterator prototype identities. Set
  allocation uses the GC-retrying VM path, and native `Set.prototype.add`
  reserves new `[[SetData]]` storage before mutation without reserving for a
  duplicate. Exact four-file Test262 admission moves pinned top-level Set from
  ordinary **16 pass / 0 fail / 6 skip** to **20 pass / 0 fail / 2 skip** and
  forced execution from **20 pass / 2 fail** to **22 pass / 0 fail**. Full Set
  moves from **340/2/41** to **344/2/37**; its two failures remain independent
  `Set.prototype[Symbol.toStringTag]` tests. Direct tests cover iterator/adder
  cache and arity, Proxy order, built-in overrides, close priority, foreign
  Realm identities, forced GC, all reservation sites, duplicate insertion,
  Fuel, exact heap-cap retry, pin cleanup, and clean retry.
  Local gates pass default debug/release library **309/309**, all-feature
  library **312/312**, es2015 **138/138**, `with` **62/62**, Python tooling
  **143/143** with four optional absent-checkout probes skipped, and vendored
  RegExp **38/38**, plus all targets/features, rustfmt, warnings-denied Clippy,
  wasm32 and doctest; the complete pinned 383-file Set sweep reports
  **344 pass / 2 fail / 37 skip**.
  Independent GPT-5.6 runtime and tooling reviews are closed; their GC-evidence
  and wording findings are addressed. Implementation commit `4ee6c3a` passes
  ordinary CI `30403723434` (**2/2 jobs**) and full Test262 run `30403723369`
  (**36/36 jobs**). Against preceding run `30397512891`, 31 of 32 result
  artifacts are byte-identical; `built-ins` alone moves **+4 pass / -4 skip**
  to **16461/4293/2911/3/0**. Aggregate is **32082/5104/11280/3/0** over
  **48469** total and **37186** run; sorted content-set hash
  `2b8d17790183867e4425c58403bd6fcd15d2d8f8c9366f8256a04f7f08a2b78a`.

- `new Map(iterable)` now uses the shared direct cached synchronous iterator
  record instead of allocating a wrapper iterator. It performs one
  `@@iterator` Get with no `HasProperty` probe, calls cached `next` with zero
  arguments, observes Array/Map/Set/generator iterator overrides, and meters
  each step. Iterator-step failures and non-catchable host Fuel do not close;
  catchable entry/adder failures close while preserving and rooting the
  original completion, with native errors materialized in the constructor
  Realm first. The result Map, iterable, cached adder, iterator record, entry,
  key, and value remain rooted across observable calls. Map allocation uses
  the GC-retrying VM path, and all native `Map.prototype.set`/upsert insertion
  reserves `[[MapData]]` storage before mutation. Exact nine-file Test262
  admission moves the pinned top-level constructor cohort from ordinary
  **19 pass / 0 fail / 11 skip** to **28 pass / 0 fail / 2 skip**, while forced
  execution remains **30/30**. The two retained skips are the independent
  constructibility and mixed TypedArray/WeakRef key files. Direct tests cover
  iterator/adder caching and arity, Proxy order, built-in overrides, close and
  non-close boundaries, foreign-Realm errors, forced GC, root/storage failure,
  Fuel, pin cleanup, and clean retry. Local gates pass default release library
  **307/307**, all-feature library **310/310**, builtins **561/561**, es2015
  **137/137**, `with` **62/62**, Python tooling **142/142** with four optional
  absent-checkout live probes skipped, and vendored RegExp **38/38**, plus all
  targets/features, rustfmt, warnings-denied Clippy, wasm32, doctest, and the
  pinned 204-file Map sweep at **144 pass / 1 fail / 59 skip**. The unchanged
  failure is the independent `Map.prototype[Symbol.toStringTag]` descriptor.
  Two independent GPT-5.6 final reviews are clean. Implementation commit
  `4c0e28c` passes ordinary CI `30397512857` (**2/2 jobs**) and full Test262
  run `30397512891` (**36/36 jobs**). Against preceding run `30390395072`, 31
  of 32 result artifacts are byte-identical; `built-ins` alone moves **+9 pass
  / -9 skip** to **16457/4293/2915/3/0**. Aggregate:
  **32078/5104/11284/3/0** over **48469** total and **37182** run; sorted
  content-set hash
  `36f9fc0e9dc914015d869e2b199623aa4f9b42da116573d4f86a5dc638a89f78`.

- `Map.groupBy` now executes the complete collection-key `GroupBy` pipeline
  through a direct cached synchronous iterator record. It performs no
  `HasProperty` probe, calls `next` with zero arguments, observes overridden
  Array/Map/Set/generator iterators, meters each step, and enforces the safe
  integer index limit. Catchable callback/root/storage failures close while
  preserving the original completion; step failures and host Fuel do not
  close. SameValueZero keys retain object identity, merge `NaN`, canonicalize
  `-0`, and never run `ToPropertyKey`. Distinct object keys and all values stay
  rooted until fallible, per-group-metered output publication. Every Realm now
  owns Map and Map Iterator intrinsics; result Maps, group Arrays, iterator
  objects, native errors, and constructor fallback use immutable method-Realm
  identities. Internal publication bypasses overridden `set`, species, and
  mutable global Map. Exact Test262 admission moves the pinned 14-file
  directory from ordinary **12 pass / 0 fail / 2 skip** to **14 pass / 0 fail
  / 0 skip**, while forced execution remains **14/14**. Direct tests cover
  iterator/close order, SameValueZero, cross-Realm identities, root pressure,
  Fuel, deterministic storage/result failures, forced GC, cleanup, and retry.
  Local gates pass debug and release library **308/308**, builtins **561/561**,
  es2015 **136/136**, `with` **62/62**, Python tooling **141/141** with four
  optional absent-checkout live probes skipped, and vendored RegExp **38/38**,
  plus all targets/features, rustfmt, warnings-denied Clippy, release build,
  wasm32, doctest, and ordinary/forced focused Test262 **14/14**.
  Two independent GPT-5.6 final reviews are clean. Implementation commit
  `061976a` passes ordinary CI `30390397502` (**2/2 jobs**) and full Test262
  run `30390395072` (**36/36 jobs**). Against preceding run `30383198359`, 31
  of 32 result artifacts are byte-identical; `built-ins` alone moves **+2 pass
  / -2 skip** to **16448/4293/2924/3/0**. Aggregate:
  **32069/5104/11293/3/0** over **48469** total and **37173** run; sorted
  content-set hash
  `58f6ae50b9c38de673d07c3307dc0c138209465c7babab1d0eff86dd80964166`.

- `Object.groupBy` now executes the complete property-key `GroupBy` pipeline
  through a direct synchronous iterator record. It performs no `HasProperty`
  probe, caches `next`, calls it with zero arguments, observes overridden
  Array/Map/Set/generator iterators, meters every step, and enforces the
  `2^53 - 1` index limit before advancing. Catchable callback, key-conversion,
  and group-storage errors close the active iterator while preserving the
  original completion; native errors are materialized and rooted in the method
  Realm before user `return` runs. Non-catchable host Fuel and
  `IteratorStepValue` errors do not re-enter user cleanup. Input, iterator,
  callback, accumulated values,
  keys, result arrays, and the null-prototype result remain rooted across
  observable calls. Group/element growth and result property publication are
  fallible and retryable; result materialization consumes one fuel unit per
  group. LIFO cleanup releases accumulated-value roots before iterator roots.
  Exact Test262 admission moves
  `built-ins/Object/groupBy` from **13 pass / 0 fail / 1 skip** to **14 pass /
  0 fail / 0 skip**; forced execution remains **14/14**, while direct tests
  cover the iterator and resource boundaries absent from that directory.
  Local validation passes **307/307** library, **561/561** builtins,
  **62/62** `with`, **304/304** release-library, and **140/140** tooling tests,
  plus all targets/features, rustfmt, warnings-denied Clippy, release build, and
  wasm32 checking. Two independent GPT-5.6 final reviews are clean after LIFO
  rooting, pre-close error materialization, deterministic root/storage
  failpoints, and native step-Fuel probes were added. Implementation commit
  `213d472` passes ordinary CI `30383198523` and all **36/36** jobs in full run
  `30383198359`. Thirty-one of 32 result artifacts are byte-identical;
  `built-ins` alone moves **+1 pass / -1 skip**. Aggregate: **32067 pass / 5104
  fail / 11295 skip / 3 timeout / 0 error**; sorted content-set hash
  `8d7ac4bfebfa569639b2e93767c2496d6f933d89e8082d81eea4646c9aac6266`.

- `Object.fromEntries` now consumes every synchronous iterable through a
  cached iterator record instead of snapshotting only raw Array storage. It
  observes `next`, entry `0`, entry `1`, `ToPropertyKey`, and property creation
  in specification order; closes only catchable entry-processing abrupt
  completions; never re-enters user cleanup for a host Fuel abort;
  preserves the original throw when `return` also throws; and leaves
  `IteratorStepValue` failures unclosed. Result publication uses the fallible
  ordinary define path while the result, iterator, entry, key, and value stay
  GC-rooted. Exact Test262 admission expands the directory from **12 pass / 0
  fail / 13 skip** to **25 pass / 0 fail / 0 skip**; a forced fixed-policy A/B
  isolates the code change from **14 pass / 11 fail** to **25 pass / 0 fail**.
  Local validation passes **306/306** library, **560/560** builtins,
  **62/62** `with`, **303/303** release-library, and **139/139** tooling tests,
  plus all targets/features, rustfmt, warnings-denied Clippy, release build, and
  wasm32 checking. The optional live-metadata tooling probe also tolerates an
  absent or inaccessible configured Test262 checkout while retaining its
  unconditional frozen-manifest checks. Two independent GPT-5.6 final reviews
  are clean. Implementation commit `4601c00` and tooling follow-up `d0545c9`
  pass ordinary CI `30376165881`; all **36/36** jobs pass in full run
  `30374968848`. After replacing the preceding run's sole Annex B contention
  timeout with a byte-identical clean rerun, 31 of 32 result artifacts are
  unchanged and `built-ins` alone moves **+13 pass / -13 skip**. Aggregate:
  **32066 pass / 5104 fail / 11296 skip / 3 timeout / 0 error**; sorted
  content-set hash `9080e4e377d351a4621d58b154a8dae6967234a1bbdb7bef4b6865e4bde2baac`.

- Every Realm's `%Array.prototype%[Symbol.unscopables]` now holds its own
  null-prototype object with the exact 16 standard Array method names. Its
  entries are writable, enumerable, and configurable `true` data properties;
  the symbol property itself is non-writable, non-enumerable, and configurable.
  `with` is absent because it is a reserved word, while `with` statements now
  hide the listed intrinsic Array names through the existing
  object-environment path. Temporary-root and property-storage reservation
  failures restore all pins and leave Realm registries unpublished. The pinned
  Test262 directory moves from **0/4** to **4/4** without changing runner
  admission policy. Local verification passes **305/305** library,
  **558/558** builtins, **62/62** `with`, and **138/138** tooling tests plus
  all-target/all-feature tests, release build, rustfmt, warnings-denied Clippy,
  and wasm32 checking. Implementation commit `2448889` passes CI
  `30362037365` and all 36 jobs in full run `30362037348`. The full matrix
  moves only built-ins by **+4 pass / -4 fail** after normalizing one Annex B
  runner-contention timeout with a byte-identical rerun. Corrected aggregate:
  **32053 pass / 5104 fail / 11309 skip / 3 timeout / 0 error**.

- RegExp `v` now supports string-valued Unicode properties and `\q{...}`
  disjunctions, including empty strings, single-character crossover with
  ordinary classes, `/iv` folding before set algebra, grammar-accurate
  `MayContainStrings` negation, and backward matching in lookbehind. Vendored
  `regress` deduplicates mathematical string sets and lowers them through a
  shared-prefix trie whose equivalent suffix subtrees use bracket transitions;
  this keeps all seven Unicode 17 string properties inside bounded PikeVM
  execution. Static property tables are charged before cloning; construction
  rejects cumulative materialization or estimated emission over 750,000 units,
  more than 65,536 explicit alternatives, a conservative trie-node upper bound
  over 65,536, and elements over 256 code points. Exact Test262 admission covers all
  142 generated Unicode-set and string-property files; complete RegExp is
  **1189 pass / 0 fail / 690 skip** over 1,879 files.

- Well-formed UTF-8 entering JavaScript strings now passes through one
  canonical UTF-16 boundary. Source string and RegExp literals, template
  cooked/raw text, `JSON.parse` escapes, serde ingress, and JSON/text data
  modules preserve `U+F0000..U+F07FF` as surrogate pairs instead of confusing
  those valid scalars with RuJa's lone-surrogate sentinels. The remaining
  Unicode RegExp matcher transport collision is tracked separately. Serde
  export reconstructs valid pairs as Unicode scalars and replaces lone
  surrogates with U+FFFD because `serde_json::String` must be well formed.
  Unicode RegExp normalization recombines canonical adjacent surrogate units,
  preserving scalar literal, class, and constructor self-matches. Module
  specifiers and public/CLI string output decode canonical UTF-16 before
  crossing host UTF-8 boundaries. Native errors carry explicit host/internal
  text provenance, so OS and callback messages canonicalize exactly once while
  messages containing existing JavaScript strings remain unchanged. Host error
  display decodes internal text and preserves host text, including mixed
  module-link diagnostics with Unicode paths and JavaScript export names.
  Lexer source provenance keeps direct/indirect eval, test host/agent eval,
  and dynamic Function source from canonicalizing existing JS strings twice.

- Complex `iv` RegExp classes now lower `\w` and `\W` operands to the exact
  ECMAScript WordCharacters inventory before native set algebra. Nested
  intersection, subtraction, union, complement, lookaround, and backreference
  paths no longer inherit Rust's broader Unicode word class. Whole-class HIR
  materialization remains limited to ordinary classes, preserving complex
  `v` syntax and bounded backend execution. The string-set and UTF-16 logical
  matcher work is recorded separately above.

- Unicode `u`/`v` RegExp word boundaries under active ignore-case now use the
  exact ECMAScript WordCharacters set on every backend route. Vendored
  `regex-syntax` and `regex-automata` lower dedicated ECMAScript boundary HIR
  assertions to PikeVM look states; non-nullable repeated captures use
  transactional capture-clear states, while nullable repeats retain Fancy's
  exact `RepeatMatcher` behavior.

  `PrefilteredExact` uses a relaxed Rust language superset only to reject
  impossible matches, an exact linear matcher for repeated-capture language
  selection, and full-haystack exact-position APIs for capture correction and
  sticky matching. Empty-match iteration advances from the actual match, and
  non-global replacement no longer enumerates the unused suffix. Direct tests
  cover non-ASCII boundaries, capture clearing, sticky hostile suffixes,
  million-scalar no-match scans, and nested-repeat adversarial cases.
  Exact group-zero bounds are also recovered after a capture-erased nullable
  repeat selects the start, preventing `find` and capture API disagreement.
  Per-position global execution no longer rescans the complete input to prove
  the Rust fast path safe, avoiding quadratic behavior on many matches.

- RegExp flags `u` and `v` are now mutually exclusive in the common source
  validator. Literals fail during parsing and `RegExp` construction fails
  during initialization, while invalid and duplicate flag diagnostics retain
  precedence. Exact Test262 admission is limited to the literal and
  constructor mutual-exclusion files.

- RegExp `v` patterns now use Unicode pattern semantics consistently when
  normalizing decimal escapes, identity escapes, and dot atoms. In particular,
  `\p{...}` escapes retain their backslash for the backend and `.` consumes one
  Unicode code point while excluding all four ECMAScript LineTerminators
  unless dotAll is active.

  The frozen character-only Unicode set-operation matrix admits 48 generated
  Test262 files covering union, intersection, and subtraction across literal
  characters, nested character classes, class escapes, and character property
  escapes. This initial 48-file admission is now subsumed by the 142-file
  string-set corpus described above.

- RegExp quantifier bounds are now parsed independently of host pointer width.
  Finite values through `u128` stay inline, larger values retain canonical
  decimal text, and infinity remains a distinct bound. Analysis uses
  saturating sizes, while compilation emits one counter instruction rather
  than expanding the repeated expression.

  Quantifiers above the Rust backend's `u32` range route directly to the
  bounded ECMAScript counter VM. Validated braced repeats that hit
  `CompiledTooBig` use the same fallback, including patterns already using
  lookaround or backreferences; syntax errors do not. Forced routing
  marks every repeat subtree without short-circuiting sibling traversal and
  preserves legacy literal braces and quantified empty groups. Exact, open,
  bounded, lazy, nullable, capture, Unicode, legacy, and arbitrary-precision
  cases are covered. On pinned Test262 `020cb740`, complete
  `built-ins/RegExp` improves from **1042 pass / 1 fail / 836 skip** to
  **1043 / 0 / 836**; `quantifier-integer-limit.js` is the only changed file.

- Repeated-capture RegExp matching now uses the linear backend only to reject
  no-match inputs and locate the leftmost candidate start. The bounded
  ECMAScript backend supplies the authoritative match end and capture state;
  builtin iteration and `lastIndex` updates consume that end. This preserves
  `RepeatMatcher` semantics when nullable
  quantifiers legitimately consume farther than the linear prefilter.
  `exec`, global/sticky matching, replacement, and Unicode/legacy compositions
  share the corrected boundary behavior while hostile no-match probes retain
  their linear prefilter.

- Deferred computed property names now store object and Proxy identities as a
  direct `GcIdx` instead of allocating an inner `Box<Value>`. Primitive and
  internal recursive names remain boxed, and the outer Reference box remains
  unchanged. `MakeRawPropertyRef` and `MakeSuperPropertyRef` use the compact
  representation; GetValue, PutValue, delete, and `ResolvePropertyRef` share
  one coercion path. This changes the public Rust payload shape of
  `ReferencedName::UncoercedProperty` from `Box<Value>` to
  `UncoercedPropertyName`; the crate does not promise a stable Rust ABI, while
  compile-time assertions keep the audited target-width size budgets fixed.

  `ToPropertyKey` remains deferred: simple assignment evaluates its RHS first,
  nullish/delete paths keep their existing rejection order, and computed super
  read-modify-write resolves a key once. Direct tests cover both production
  opcodes, exact root identity, allocation-failure retention, Proxy key
  mutation after RHS evaluation, forced GC, Symbol conversion, and super
  receiver identity. Layout assertions retain 32/64-byte
  `ReferencedName`/`ReferenceRecord` sizes on x86_64 and 24/40 bytes on wasm32.
  A paired 30,000-operation benchmark isolates object-key simple assignment
  beside an unchanged primitive String-key control and verifies all 30,000
  coercions before sampling. Forced-rebuild shared-host smoke samples measured
  object keys at 413.89-423.33 ms on current source versus 436.25-462.47 ms on
  preceding source; the String control measured 138.27-143.33 ms versus
  142.13-147.63 ms. These short samples show no regression but are not treated
  as a stable throughput claim.

  On pinned Test262 `9e61c128`, current and preceding release binaries produce
  byte-identical output for the 1,552-file assignment/delete/optional/super/
  for-in/for-of cohort at **1343 pass / 0 fail / 209 skip**, SHA-256
  `3e4263f9e2b4ce015ce68dcb2ef988a467a9cede897c61c624ae31ac356032ab`.
  The complete supported statements/expressions subset is also byte-identical
  at **12761 pass / 0 fail / 7678 skip**, SHA-256
  `c59b10015e636a164867edd718fc2b0f018e5bcf2d0ed969fa0df136ade46dfc`.

  Implementation commit `942be9b` passes ordinary CI `30285088249` and all
  35 jobs in full Test262 run `30285089340`. Thirty-one result artifacts are
  byte-identical to preceding run `30276195375`; its one Annex B contention
  timeout returns to the clean **201/811/74/0/0** result, byte-identical to
  clean run `30269385090`. The corrected 32-file aggregate remains **31901
  pass / 5110 fail / 11455 skip / 3 timeout / 0 error** over **48469** tests
  and **37011** pass-or-fail executions.

- References resolved through a `with` object environment now store the
  binding object's `GcIdx` directly instead of allocating an inner
  `Box<Value>`. The dedicated `ObjectEnvironment` variant remains separate
  from ordinary object property References because its missing-binding,
  delete, assignment, and implicit call-receiver rules differ.

  Reference creation validates that the environment payload is an Object,
  get/set/delete/call paths reconstruct a non-allocating `Value::Object` view,
  and GC tracing visits the direct binding-object root exactly once. Direct
  tests cover the production resolver, malformed internal payloads, Proxy
  identity and forced GC across all consumers, cross-Realm primitive boxing,
  strict global-call fallback, and retained ABI sizes.

  Current and preceding release binaries produce byte-identical output over
  the 15,650-file expressions/class/with cohort at **10290 pass / 0 fail /
  5360 skip**. The current supported language subset remains **12761 / 0 /
  7678**. A new 30,000-operation `with` Reference benchmark measured 203.78
  ms on the preceding source and 201.83-208.30 ms on repeated current runs;
  Criterion reports no significant change.

  Implementation commit `8366015` passes ordinary CI `30276201220` and all
  35 jobs in full Test262 run `30276195375`. Thirty-one matrix result files
  are immediately byte-identical to the preceding run; Annex B's sole
  contention timeout disappears when rerun with the downloaded CI binary,
  restoring the byte-identical **201/811/74/0** result. The corrected 32-file
  aggregate remains **31901 pass / 5110 fail / 11455 skip / 3 timeout / 0
  error** over **48469** tests.

- The VM now retains one rootless vacant `Box<ReferenceRecord>` and reuses it
  for sequential identifier, property, super, and private References. Every
  checkout happens before observable re-entry; terminal get, put, delete,
  call, eval, `typeof`, stack-pop, and unwind paths return the allocation.
  Concurrent re-entry allocates independently and drops the overflow record
  when the one-entry cache is already occupied.

  The vacant sentinel replaces the complete record and has no GC roots, so it
  is intentionally absent from root enumeration. Top-level and async abrupt
  completion now restore their incoming frame/stack depths, and generator
  operand stacks move instead of cloning retained Reference boxes. Direct
  tests cover sequential reuse, re-entry, every root-bearing field, errors,
  async rejection/resumption, and generator suspension/completion/error.
  Pinned current and preceding Test262 output is byte-identical over **15,650**
  affected files at **10290 pass / 0 fail / 5360 skip**; the supported language
  subset remains **12761 / 0 / 7678**. Forced-rebuild Criterion smoke samples
  place numeric computed References at 94.802-96.313 ms current versus 98.159
  ms preceding, and the string control at 95.122-99.862 ms versus 96.140 ms;
  Criterion reports no significant change. Commit `6caf259` passes ordinary
  CI and all 34 full-matrix jobs. All 32 Test262 result artifacts are
  byte-identical to the preceding run.

- Object-backed property, raw, super, and private References now store their
  `GcIdx` directly in `ReferenceBase`. Their outer `ReferenceRecord` is the
  only base-related Box; primitive bases retain boxed `Value` storage. `Value`,
  `ReferenceBase`, and `ReferenceRecord` sizes remain 32/16/64 bytes on
  x86_64 and 24/8/40 bytes on wasm32.

  GC tracing visits direct object bases exactly once. Existing object,
  primitive, super, private, Proxy, `with`, retained-root, and abrupt-cleanup
  tests remain green. Pinned `language/expressions`, class, and `with` Test262
  output is byte-identical to the preceding release at **10290 pass / 0 fail /
  5360 skip / 15650 total**. Forced-rebuild Criterion A/B found no significant
  change in the existing 30,000-operation computed-Reference workloads. All 32
  post-push full-matrix artifacts are byte-identical to the preceding run.

- Runtime Number-to-String conversion now formats into a fixed 32-byte stack
  buffer. Dynamic non-index numeric property keys allocate only their required
  final `Arc<str>` instead of first creating a temporary Rust `String`;
  JavaScript-visible `num_to_string` keeps its owned `String` API.

  The stack formatter is byte-identical to the preceding algorithm over its
  semantic edge table and 20,000 deterministic random `f64` bit patterns.
  Proxy regressions cover negative, fractional, ArrayIndex-boundary,
  exponential, NaN, and infinite keys as exact JavaScript Strings. Pinned
  assignment/delete/relational/property-accessor and Object/Proxy/Reflect
  Test262 output is byte-identical at **4789 pass / 0 fail / 194 skip / 4983
  total**. Sequential Criterion A/B places 30,000 `in` conversions at 65.796
  ms current versus 70.611 ms preceding, with string controls at 65.932 ms
  versus 66.754 ms; no significant performance change was detected. All 32
  post-push full-matrix artifacts are byte-identical to the preceding run.

- Ninety numeric property-name formatting sites across Array, TypedArray,
  Array iterators, call argument materialization, JSON, RegExp, Proxy own-key
  validation, and adjacent array-like constructors no longer create temporary
  Rust `String` values. Structured operations use `from_integer_index`; paths
  that must retain established string dispatch use a stack decimal view.
  Generic Array paths preserve decimal property names above the canonical
  ArrayIndex boundary through `Number.MAX_SAFE_INTEGER`.

  A structured strict-Set helper reuses the established string `[[Set]]` path
  through a stack decimal view, preserving Array, TypedArray, Proxy, namespace,
  receiver, and error ordering. Direct tests cover `4294967294`, `4294967295`,
  `4294967296`, and `9007199254740990`, primitive String searches, Proxy
  get/set/delete keys, and Array iterator values/entries at the named-integer
  boundary. Pinned Array/TypedArray/JSON/RegExp/Proxy/Reflect Test262 output is
  byte-identical to the preceding binary at **6712 pass / 6 fail / 1082 skip /
  0 timeout / 0 error / 7800 total**. Five-run process wall-time diagnostics
  found no regression in Array reverse, TypedArray reverse, or Array iterator
  values; these shared-host samples are smoke evidence, not benchmark claims.
  The pinned supported subset remains **12761 pass / 0 fail / 7678 skip**.

- The two variable-length TypedArray `preventExtensions` staging tests are now
  admitted through the shared exact-path extensibility policy. Their three
  otherwise gated features are removed only for the two audited files, while
  recursive live-directory equality prevents future staging siblings from
  entering the full matrix. The adjacent `Object.seal` staging test remains
  gated.

  Pinned Test262
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4` runs both admitted files at
  **2 pass / 0 fail / 0 skip / 2 total**. Full CI now verifies their exact
  metadata and executes the two singleton staging directories as independent
  matrix shards. This is an admission-only change: the runtime behavior and
  direct fixed/length-tracking RAB/GSAB regressions shipped in the earlier
  Object integrity unit. Commit `6f6f8db` passes CI `30223746065` and all
  **35/35** jobs in full run `30223746062`. Both new shards are **1/1**; 29 of
  the 30 preceding shards are byte-identical to full run `30221601417`.
  Current Annex B has one contention timeout, and a downloaded-binary rerun is
  byte-identical to clean run `30219956582` at **201/811/74/0/0**. Corrected
  full results are **31901 pass / 5110 fail / 11455 skip / 3 timeout / 0 error /
  48469 total / 37011 pass-or-fail run**. Downloaded artifacts, the binary,
  rerun log, and pinned worktree were deleted after comparison.

- Embedded empty RegExp classes now lower at the class-scanner boundary instead
  of reaching the Rust backends as unsupported `[]` or `[^]` syntax. Positive
  empty classes become the existing never-match atom; negated empty classes
  become the existing Unicode-scalar or legacy UTF-16-code-unit universal
  atom. Exact slice matching preserves escaped brackets, non-empty classes,
  nested `v` subtraction, quantifiers, alternation, and original `.source`.

  Direct literal/constructor tests cover leading and trailing classes,
  alternation, ignore-case/global flags, quantifiers, newlines, lone
  surrogates, legacy versus Unicode astral behavior, `v` mode, and escaped
  brackets. On pinned Test262
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, all five affected files move
  from fail to pass and complete `built-ins/RegExp` moves from
  **1036 pass / 7 fail / 836 skip** to **1041 / 2 / 836**. The two remaining
  failures are `quantifier-integer-limit.js` and `nullable-quantifier.js`.
  Local gates pass all targets/features with **275/275** debug and **273/273**
  release library tests, builtins **542/542**, tooling **135/135**, doctest
  **1/1**, wasm32, rustfmt, warnings-denied Clippy, and generated documentation;
  rustdoc retains 13 existing warnings. GPT-5.6 correctness reviewer Planck and
  evidence/documentation reviewer Kepler are clean after adding Fancy backend
  fixtures and clarifying historical ADR boundaries. Commit `35d7ccc` passes
  CI `30221601414` and all 33 jobs in full run `30221601417`. Built-ins changes
  exactly **+5 pass / -5 fail**; 28 other original artifacts are byte-identical
  to preceding full run `30219956582`. The current Annex B artifact moves two
  passes to contention timeouts, while the downloaded CI binary rerun is
  byte-identical to the clean baseline **201/811/74/0/0**. Corrected full
  results are **31899 pass / 5110 fail / 11455 skip / 3 timeout / 0 error /
  48467 total / 37009 pass-or-fail run**. Artifacts and the sparse worktree were
  deleted after comparison.

- Retained Reference reads now use one `GetValueKeepReference` opcode instead
  of `Dup; GetValue`. The opcode moves the sole boxed Reference off the stack,
  pre-reserves and pins its complete GC root set, resolves the value, then
  restores the original Reference before releasing pins. Raw property names
  reserve the two simultaneously live root suffixes needed during key
  coercion. This preserves the old normal and abrupt stack contract without
  allocating a second `Box`.

  All 24 compiler sites that retain a Reference for update, compound/logical
  assignment, calls, or tagged calls use the move opcode; no retained
  `Dup; GetValue` pair remains. Direct tests cover bytecode shape, reservation
  failure before an observable getter or raw `super` key coercion, forced GC,
  thrown getters, retry, and exact pin cleanup. Reference-heavy classes
  **105/105**, ES2015 **133/133**, operators **130/130**, and `with` **58/58**
  pass. Local gates pass all targets/features with **275/275** debug and
  **273/273** release library tests, tooling, wasm32, rustfmt, warnings-denied
  Clippy, and generated documentation; rustdoc retains 13 existing warnings.
  Final GPT-5.6 correctness and scope/documentation reviews are clean after the
  raw-name peak-root and exhaustive compiler-fixture fixes. Pinned Reference
  Test262 is byte-identical to the
  preceding binary at **930 pass / 0 fail / 19 skip**, and the supported subset
  remains **12761/0/7678**. Two short Criterion A/B samples show about a 2%
  numeric improvement and overlapping string-control results; the deterministic
  evidence is removal of 24 boxed-Reference clones. Initial Reference creation
  boxes remain. Commit `00b5496` passes CI `30218857008` and all 33 jobs in
  full run `30218856969`. Full artifacts aggregate to the corrected baseline
  **31894 pass / 5115 fail / 11455 skip / 3 timeout / 0 error / 48467 total /
  37009 pass-or-fail run**. Twenty-nine result files are byte-identical to
  preceding full run `30217016070`; its sole Annex B contention timeout returns
  to the clean **201/811/74/0/0** result. Downloaded artifacts were deleted
  immediately after comparison.

- Reference consumers now borrow `ReferenceRecord` fields instead of deep-
  cloning their outer `Box` in `GetValue`, `PutValue`, and delete. Raw
  `PutValue` and deferred-key resolution publish the record's heap roots
  directly, and boxed base/receiver extraction clones only the contained
  `Value`. One authoritative
  `visit_gc_roots` walk now serves heap tracing, temporary-root counting, and
  pin publication, preventing those three root definitions from drifting.

  Direct tests cover nested Environment/base/receiver/raw-name roots, exact
  root counts, pin suffixes, abrupt key coercion, cleanup, and VM reuse.
  Reference-heavy classes **105/105**, ES2015 **133/133**, operators
  **130/130**, and `with` **58/58** pass. Local gates pass all targets/features
  with **273/273** library tests, release library **271/271**, tooling
  **135/135**, doctest **1/1**, wasm32 checking, rustfmt, warnings-denied
  Clippy, and generated documentation. Rustdoc retains 13 existing warnings.
  Pinned Reference Test262 remains byte-identical at **930 pass / 0 fail / 19
  skip**, and the supported subset remains **12761/0/7678**. Repeated 30k
  Criterion samples show no regression: current numeric/string point estimates
  are about **82.6-83.0 ms / 83.7-84.4 ms**, versus preceding **83.9-84.7 ms /
  85.1-88.3 ms**. Final GPT-5.6 correctness and performance/documentation
  reviews are clean. Commit `68e3766` passes CI `30215764494` and all 33 jobs
  in full run `30215764513`. Raw artifacts aggregate to **31892 pass / 5115
  fail / 11455 skip / 5 timeout / 0 error** because two Annex B passes moved
  to contention timeouts. The downloaded binary rerun restores
  **201/811/74/0/0**, producing corrected **31894/5115/11455/3/0** over
  **48467** total and **37009** pass-or-fail run. The other 29 result files are
  byte-identical to preceding full run `30214416788`. Artifacts and sparse
  worktree were deleted after comparison. Reference creation boxes and
  required `Dup` clones remain.

- Test262 language early-error admission now includes the four exact generator
  update-expression parse-negative files for prefix/postfix increment and
  decrement. RuJa already rejects each invalid `yield` target correctly; this
  is a policy-only change that removes the broad `generators` skip only from
  those audited paths. The shared manifest now freezes nine files, and tooling
  proves runner/analyzer symmetry, pinned metadata, and continued rejection of
  future siblings. The four complete update directories pass **142/142** on
  pinned Test262; the adjacent Reference cluster moves from **926 pass / 0 fail
  / 23 skip** to **930 pass / 0 fail / 19 skip**, exactly **+4 pass / -4
  skip**, and the supported subset is **12761 pass / 0 fail / 7678 skip**.
  Local gates pass all targets/features with **271/271** library tests,
  **541/541** builtins tests, **130/130** operators tests, release library
  **269/269**, tooling **135/135**, doctest **1/1**, wasm32 checking, rustfmt,
  warnings-denied Clippy, and generated documentation. Rustdoc retains 13
  existing warnings. Final GPT-5.6 correctness review is clean; the independent
  documentation review's three stale historical/count descriptions were
  corrected. No runtime or parser code changed. Commit `d810343` passes CI
  `30213116749` and all 33 jobs in full run `30213116898`. The original Annex B
  artifact moves one pass to a contention timeout; the downloaded binary rerun
  restores **201/811/74/0/0**. Corrected full results aggregate to **31894 pass
  / 5115 fail / 11455 skip / 3 timeout / 0 error / 48467 total / 37009
  pass-or-fail run**. Of 30 result files, 28 are byte-identical to the preceding
  run; Annex B differs only by the timeout, and expressions moves exactly **+4
  pass / -4 skip**. Downloaded artifacts and the pinned worktree were deleted
  after verification.

- Computed compound assignment, logical assignment, and update expressions now
  pass their evaluated key directly to `MakePropertyRef`. On 64-bit targets,
  canonical numeric names therefore become inline `PropertyKey` indices
  without first allocating a temporary JavaScript String and reparsing it.
  wasm32 removes the redundant opcode/value handoff but retains its Arc-backed
  numeric key. Null-base ordering,
  Symbol/object key coercion, Proxy receivers, strictness, `with`, private
  names, and `super` keep their existing Reference semantics.

  Bytecode, numeric-boundary, null-base, Proxy, and forced-GC regressions cover
  the three read-modify-write forms. A retained 30k-operation numeric/string
  Criterion pair measured the numeric case at 100.06 ms after the change versus
  100.89 ms before it; the roughly 1% samples are timer-noise evidence only,
  while opcode removal is deterministic. Pinned Test262 compound, logical,
  update, `super`, and `with` coverage remains byte-identical at **926 pass / 0
  fail / 23 skip / 0 timeout / 0 error** between the `f3766ec` implementation
  and its preceding release binaries. Local gates pass all targets/features
  with **271/271** library tests, **541/541** builtins tests, **130/130**
  operators tests, release library **269/269**, tooling **135/135**, doctest
  **1/1**, wasm32 checking, rustfmt, warnings-denied Clippy, release build, and
  generated documentation. Rustdoc retains 13 existing warnings. Two final GPT-5.6 reviews are clean after
  tightening wasm32 documentation and adding ephemeral-GC plus object-to-Symbol
  key coverage. Implementation commit `f3766ec` passes CI `30210419512` and all
  33 jobs in full run `30210419518`. The original Annex B artifact moves two
  passes to runner-contention timeouts; the downloaded binary rerun restores
  **201/811/74/0/0**. Corrected artifacts at
  `/tmp/ruja-computed-ref-30210419518-final` aggregate to unchanged **31890 pass
  / 5115 fail / 11459 skip / 3 timeout / 0 error / 48467 total / 37005 run**;
  all 30 corrected files match the preceding clean run exactly. The downloaded
  binary also reproduces focused **926/0/23** byte-for-byte.

- Runtime BigInt values now use shared immutable `Arc<BigInt>` storage, making
  `Value` clones constant-time for multi-limb integers across properties,
  constant pools, boxing, mapped Arguments, descriptors, and Realm crossings.
  Arithmetic and TypedArray/DataView/Atomics conversions borrow operands and
  allocate only fresh results; equality and Map/Set hashing remain value-based.
  The public `Vm::to_bigint() -> BigInt` contract is unchanged. Direct enum
  construction now takes `Arc<BigInt>`; embedders can use
  `Value::bigint(BigInt)` or `Value::from(BigInt)`.

  This is a runtime clone optimization, not a BigInt OOM guarantee. Parser AST
  transfer still clones once, and `num-bigint` limbs plus Arc control blocks
  remain outside the VM heap cap. A 64K-digit property-read stress workload
  improves from 1.05 s to 0.74 s, while small arithmetic is unchanged within
  timer noise. Focused Test262 BigInt operations are **496 pass / 0 fail / 44
  skip**, and BigInt TypedArray/DataView coverage is **93/93** on both current
  and preceding release binaries. Local gates pass all targets/features with
  **267/267** library tests, **541/541** builtins tests, **24/24** BigInt tests,
  release library **267/267**, tooling **135/135**, doctest **1/1**, rustfmt,
  warnings-denied Clippy, release build, generated documentation, and wasm32
  checking. Rustdoc retains 13 existing warnings. CI `30203903239` and all 33
  jobs in full run `30203903224` pass. The original Annex B artifact has one
  runner-contention timeout; the downloaded binary rerun restores
  **201/811/74/0/0**. Corrected artifacts at
  `/tmp/ruja-shared-bigint-30203903224-final-v1` aggregate to unchanged
  **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error / 48467 total /
  37005 run**; all 30 corrected result files match the preceding clean run.
  The downloaded binary reproduces both focused cohorts.

- TypedArray `toString` Test262 admission now maps its four audited files to
  exact per-file feature metadata instead of subtracting one four-feature union
  from every listed path. The shared manifest covers the parent `toString.js`
  and three nested files; tooling verifies the live file set, features,
  includes, flags, negative metadata, disjointness, invalid paths, extra
  features, and future siblings. Full-matrix setup now validates exact
  TypedArray `toString`, `join`, and `toLocaleString` admission before scheduling
  shards.

  The runtime algorithm is unchanged. New direct-native tests retain custom
  join and fallback-tag receivers across forced GC, verify balanced pins,
  primitive boxing, and foreign method-Realm errors. Direct Test262 remains
  **4/4**, and combined TypedArray string coverage remains **75/75**. Local
  verification passes all targets/features with **217/217** library tests,
  **537/537** builtins tests, and **15/15** arguments tests, plus **216/216**
  release library tests, **133/133** Python tooling tests, **1/1** doctest,
  rustfmt, warnings-denied Clippy, release build, generated documentation, and
  wasm32 checking. Two independent GPT-5.6 reviews found no runtime or
  admission defect. Tooling commit `739e3ff` passes CI `29892602601` and all 33
  full-matrix jobs in `29892602512`. The initial `annexB` artifact had two
  runner-contention timeouts; the same downloaded binary and pinned corpus
  reproduced the baseline **201/811/74/0/0**, and the rerun artifact is clean.
  Final downloaded artifacts aggregate to the unchanged **31872 pass / 5126
  fail / 11466 skip / 3 timeout / 0 error / 48467 total / 36998 run**; all 30
  result files are byte-identical to full run `29890470558`. The downloaded
  release binary reproduces direct coverage **4/4** and adjacent coverage
  **75/75**.

- TypedArray `toLocaleString` Test262 admission now freezes the complete
  **39-file** directory and each file's exact feature metadata instead of
  removing one broad feature set for every present or future sibling. Runner
  and analyzer share the same manifest and per-file map; tooling verifies live
  files, includes, flags, negative metadata, disjointness, invalid paths, and
  future-sibling closure. The full-matrix setup job runs that verification
  against the corpus it will schedule.

  Ordinary CI and every full-matrix shard now use the same pinned Test262
  revision, `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, eliminating
  cross-job corpus drift. Current direct coverage remains **39/39**, adjacent
  Array locale plus TypedArray-constructor inheritance remains **14/14**, and
  the exact policy admits no new test. Local verification passes **133/133**
  tooling tests, all targets/features with **214/214** library tests,
  **535/535** builtins tests, and **15/15** arguments tests, plus **213/213**
  release library tests, rustfmt, warnings-denied Clippy, wasm32 checking, YAML
  parsing, and Python bytecode compilation. Three GPT-5.6 admission/workflow
  reviews are clean after adding CI corpus pinning and directory equality.
  Tooling commit `3f31c29` passes CI `29888462270` and full matrix
  `29888462280`. Downloaded artifacts aggregate to the unchanged **31872 pass /
  5126 fail / 11466 skip / 3 timeout / 0 error / 48467 total / 36998 run**;
  all 30 result files are byte-identical to full run `29886017568`.

- The embedding methods `Vm::promise_resolve` and `Vm::promise_reject` now
  return `error::Result<()>` instead of `()`. This 0.4.0-alpha API change lets
  embedders observe non-catchable host aborts before Promise settlement becomes
  irreversible instead of silently dropping them.

### Fixed

- Object integrity operations now use one Proxy-aware
  `SetIntegrityLevel`/`TestIntegrityLevel` pipeline across ordinary objects,
  Arrays, Arguments, functions, iterators, TypedArrays, and Module Namespace
  objects. `Object.isFrozen(Object.seal([]))` is correctly false because Array
  `length` remains writable, deleted Arguments `length` no longer reappears,
  Proxy freeze descriptors are presence-aware internal records rather than
  prototype-pollutable JavaScript objects, and Module Namespace TDZ reads throw
  in seal, freeze, and frozen predicates.

  Direct objects inspect only descriptor attributes. Existing ordinary
  descriptors update in place, while dense Array values move into reserved
  custom-property storage instead of being cloned. This removes unchecked
  large-BigInt descriptor clones from ordinary and dense-Array integrity
  operations. Integrity roots, Array property growth, fuel, partial effects,
  promoted mapped-Arguments value snapshot/detachment, retry, foreign Realms,
  and exact-cap GC have deterministic regressions. Non-fixed TypedArray views
  over resizable buffers now reject `[[PreventExtensions]]`; fixed views over
  fixed buffers and growable SharedArrayBuffers retain the specification
  result. Nested preventExtensions roots reserve before every pin. Module
  Namespace re-export traversal uses allocation-free cycle detection and fuel
  per indirection. Direct predicate scans precharge the same
  conservative fuel budget as own-key materialization. Already-frozen direct
  predicates avoid key and descriptor materialization; separated
  10k-property/element benchmarks and repeated release workloads show no
  measured steady-state regression.

  Local gates pass all targets/features with **266/266** library tests and
  **541/541** builtins tests, release library **265/265**, tooling **135/135**,
  doctest **1/1**, rustfmt, warnings-denied Clippy, release build, generated
  documentation, and wasm32 checking. Rustdoc retains 13 existing warnings.
  Fixed Test262 Object integrity plus Module Namespace coverage is **257 pass /
  0 fail / 20 skip / 0 timeout / 0 error**, byte-identical to the preceding
  release binary. Forced execution of the two variable-length TypedArray
  preventExtensions staging tests improves from **0/2** on the preceding
  release to **2/2**; normal policy still skips those staging files. CI
  `30201495431` and all 33 full-matrix jobs in `30201495450` pass. The original
  Annex B shard had two contention timeouts; the same downloaded binary and
  pinned corpus restored the clean result. Corrected artifacts aggregate to
  unchanged **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error** and
  all 30 result files are byte-identical to run `30195285326`.

- Ordinary non-index `Set` receiver publication now uses the shared fallible
  storage publisher. New receiver properties reserve actual `props` growth
  before mutation; spare capacity and existing writable properties do not
  reserve, and replacements preserve descriptor attributes. Cache invalidation
  borrows the existing property key after commit instead of allocating a
  temporary `String`, so failed preflight leaves receiver state and cache
  entries unchanged and retryable.

  Receiver checks now recognize boxed String virtual `length` and UTF-16 index
  properties as non-writable without materializing descriptors. Module
  Namespace receiver definitions implement value-only `SameValue`: identical
  exports and `NaN` succeed, different values and opposite signed zero fail,
  and uninitialized exports retain their TDZ `ReferenceError` ordering.

  Exact regressions cover full/spare/existing maps, descriptor preservation,
  cache atomicity, retry, transparent Proxy fuel priority, completed Proxies,
  global receivers, foreign Realm errors and cleanup, surrogate-pair indices,
  Namespace `SameValue`, and allocation-failpoint exclusion. Local verification
  passes all targets/features with **264/264** library tests, **539/539**
  builtins tests, **15/15** arguments tests, **50/50** Array-index tests, and
  **31/31** module tests, plus **263/263** release library tests, **135/135**
  tooling tests, **1/1** doctest, rustfmt, warnings-denied Clippy, release build,
  generated documentation, and wasm32 checking. Rustdoc retains 13 existing
  warnings. GPT-5.6 reviewers Lovelace and Locke are clean. The four-directory
  focused Test262 output is byte-identical to the preceding release at **1683
  pass / 0 fail / 81 skip / 0 timeout / 0 error**. New receiver overwrite and
  creation workloads measure about 1.13 seconds per 100,000 and 130 ms per
  10,000 operations, with no three-run regression against that binary. CI
  `30195285329` and all 33 jobs in full run `30195285326` pass. Downloaded
  artifacts aggregate to unchanged **31890 pass / 5115 fail / 11459 skip / 3
  timeout / 0 error** and all 30 files are byte-identical to the preceding
  clean full run. The downloaded release binary also reproduces focused
  **1683/0/81/0/0** byte-for-byte.

- Direct Array index assignment now uses the same representation-aware,
  fallible storage publisher as property definition. Dense append, sparse
  creation, and custom-to-dense migration reserve actual `props`, `items`, and
  `present` growth before mutation; existing dense/custom writes and spare
  capacity do not reserve. The removed `set_array_index` path no longer builds
  a second numeric key or performs unchecked map/vector growth.

  Array prototype traversal, extensibility, and non-writable `length` checks
  remain ahead of publication; storage commits before logical length changes.
  Mapped Arguments reruns its pre-update at every same-receiver `[[Set]]`
  entry, including recursive transparent Proxy forwarding, while receiver
  `[[DefineOwnProperty]]` updates the parameter map only after successful
  storage. This preserves required partial effects across Proxy getters,
  false/throw traps, and catchable allocation failure.

  The inline cache is now grouped by object identity with an exact 4,096-entry
  count. Reads and invalidations borrow `&str` without temporary `String`
  allocation, empty object buckets are removed, every GC clear resets the
  count, and key/map allocation during optional insertion is best-effort rather
  than a host abort. At the cap, the replacement bucket is fully reserved
  before the old cache is cleared, so failed optional insertion retains all
  existing entries. New Criterion coverage exercises dense overwrite/append,
  sparse Set, read hits, and invalidate hit/miss paths; three-run comparison
  with the preceding release binary found no slowdown.

  Exact regressions cover every storage and cache reservation site, atomic
  retry, no-reservation replacements, descriptor preservation, Realm-correct
  errors, completed/transparent Proxies, recursive mapped-Arguments ordering,
  cleanup, cache overwrite/pruning/cap behavior, and Array length stability.
  Local verification passes all targets/features with **261/261** library
  tests, **539/539** builtins tests, **15/15** arguments tests, **50/50** Array
  index tests, and **31/31** module tests, plus **260/260** release library
  tests, **135/135** Python tooling tests, **1/1** doctest, rustfmt,
  warnings-denied Clippy, release build, generated documentation, and wasm32
  checking. Rustdoc retains 13 pre-existing warnings. Two GPT-5.6 final reviews
  are clean. The five-directory focused Test262 output is byte-identical to the
  preceding binary at **4054 pass / 4 fail / 243 skip / 0 timeout / 0 error**.
  CI `30192642319` and all 33 jobs in full run `30192642310` pass. Downloaded
  artifacts aggregate to the unchanged **31890 pass / 5115 fail / 11459 skip /
  3 timeout / 0 error**; all 30 files are byte-identical to the corrected
  preceding baseline. The downloaded release binary also reproduces focused
  **4054/4/243/0/0** byte-for-byte.

- Array `length` definition now reserves actual operation-root, property-map,
  dense-item, and presence growth before mutation. Shrink scans once for the
  highest non-configurable blocker and removes configurable indexed
  descriptors with one linear retain pass, avoiding allocation-sized deletion
  scratch state while preserving descending-deletion rollback semantics and
  deferred `writable: false`.

  Sparse Arrays remain sparse across shrink, blocked rollback, equal-length
  definition, and growth; none of those paths expands dense holes to the
  logical length. Virtual `length` stays unmaterialized unless a persistent
  non-writable descriptor is required, and the VM reuses one canonical
  `length` key instead of allocating it per operation. Resolved targets and
  values remain rooted across both observable numeric conversions.

  Exact regressions cover every reservation site, spare capacity, retry and
  atomicity, sparse truncation and rollback, deletion order, deferred
  writability, exact fuel, foreign Realms, transparent and completed Proxies,
  cleanup, and forced GC between the two conversions. Local verification
  passes all targets/features with **256/256** library tests, **539/539**
  builtins tests, **15/15** arguments tests, and **31/31** module tests, plus
  **255/255** release library tests, **135/135** Python tooling tests, **1/1**
  doctest, rustfmt, warnings-denied Clippy, release build, generated
  documentation, and wasm32 checking. Rustdoc retains 13 pre-existing
  warnings. Two independent GPT-5.6 final reviews are clean. The focused
  four-directory Test262 cohort remains **4731 pass / 4 fail / 121 skip / 0
  timeout / 0 error**, identical to the preceding downloaded binary.

  Implementation commit `75401b9` passes CI `30188817875` and all 33 jobs in
  full run `30188817855`. The initial `annexB` artifact shifted one pass to a
  runner-contention timeout; the same downloaded binary and pinned corpus rerun
  is byte-identical to the baseline **201/811/74/0/0**. The corrected 30-file
  evidence aggregates to unchanged **31890 pass / 5115 fail / 11459 skip / 3
  timeout / 0 error / 48467 total / 37005 run**, with every shard matching full
  run `30186299205`. The downloaded release binary reproduces the focused
  cohort at **4731/4/121/0/0**.

- Ordinary property definition now plans storage before publication and
  fallibly reserves only actual growth of the target's `props` map and Array
  `items`/`present` vectors. The shared path is used by complete VM descriptors
  and presence-aware Object/Reflect descriptors; dense, custom, sparse, and
  mapped-arguments representations commit only after every directly owned
  container is ready. Existing keys, spare capacity, dense migration, boxed
  String virtual properties, completed Proxy traps, and TypedArray integer
  indices do not request irrelevant ordinary storage.

  Direct TypedArray definition now uses integer-indexed exotic semantics, and
  the resolved target plus descriptor fields stay rooted through observable
  value coercion. Module Namespace string exports validate complete descriptors
  with `SameValue` and propagate live-binding errors, while Symbol properties
  use ordinary compatible definition. Mapped arguments update or detach their
  parameter binding only after storage publication succeeds.

  Exact regressions cover actual `props -> items -> present` growth at each
  failure site, spare and replacement paths, dense/custom/sparse atomicity,
  defineProperties partial mutation, mapped-arguments retry, Proxy/fuel
  priority, foreign Realm errors, String/TypedArray/Namespace exclusions, and
  exact-cap GC during TypedArray coercion of an otherwise unpublished target.
  Local verification passes all targets/features with **251/251** library
  tests, **539/539** builtins tests, **15/15** arguments tests, and **31/31**
  module tests, plus **251/251** release library tests, **135/135** Python
  tooling tests, **1/1** doctest, rustfmt, warnings-denied Clippy, release
  build, generated documentation, and wasm32 checking. Rustdoc retains 13
  pre-existing warnings. Two GPT-5.6 final reviews are clean.

  Implementation commit `0a9c3f8` passes CI `30186299215` and all 33 jobs in
  full run `30186299205`. Local and downloaded binaries reproduce the focused
  property-definition cohort at **1897 pass / 0 fail / 13 skip**. Full
  artifacts aggregate to unchanged **31890 pass / 5115 fail / 11459 skip / 3
  timeout / 0 error / 48467 total / 37005 run**; 29 shards are byte-identical
  to the previous artifacts, and the previous binary's exact-corpus annexB
  rerun is byte-identical to the current clean annexB artifact.

- Property descriptor conversion and publication now use one presence-aware
  internal record instead of allocating a normalized JS object and rereading
  it. `ToPropertyDescriptor` observes inherited fields in specification order
  and roots object-valued `value`, `get`, and `set` results before later
  callbacks. `Object.defineProperties` retains those records through its
  complete conversion pass before defining any target property, while
  `Object.defineProperty` and `Reflect.defineProperty` convert exactly once.

  `FromPropertyDescriptor`, `Object.getOwnPropertyDescriptors`, and Proxy
  `defineProperty` descriptor objects reserve their directly owned maps and
  roots before publication. `Object.getOwnPropertyDescriptors` now performs
  `[[OwnPropertyKeys]]` before allocating its result, and descriptor objects
  use the current Realm's registered `%Object.prototype%` without a main-Realm
  fallback. Proxy definition delays descriptor-object materialization until
  revocation, fuel, trap lookup, and callability have succeeded, and reserves
  validation fields across observable target invariant checks.

  Deterministic regressions cover all root and collection sites, spare versus
  actual growth, first and later failures, two-pass conversion, ownKeys and
  trap priority, false and transparent Proxy paths, foreign Realms, cleanup,
  retry, exact-cap collection, and liveness of freshly observed value/get/set
  objects through cap-triggered GC. Local verification passes all
  targets/features with **246/246** library tests, **539/539** builtins tests,
  **15/15** arguments tests, and **31/31** module tests, plus **246/246**
  release library tests, **135/135** Python tooling tests, **1/1** doctest,
  rustfmt, warnings-denied Clippy, release build, generated documentation, and
  wasm32 checking. Rustdoc retains only the 13 pre-existing broken-link
  warnings. Two GPT-5.6 final implementation reviews are clean. The local and
  downloaded-CI-binary nine-directory descriptor cohort is **2457 pass / 0
  fail / 24 skip**. Implementation commit `ce280cb` passes CI `29986403996`.
  All 33 jobs in full run `29986403979` pass. Its original annexB artifact
  shifted one baseline pass to a contention timeout; the downloaded binary and
  exact pinned corpus restore **201/811/74/0/0**. Corrected artifacts at
  `/tmp/ruja-descriptor-materialization-29986403979-final` aggregate to
  unchanged **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error /
  48467 total / 37005 run**, and all 30 result files are byte-identical to run
  `29980403702`.

- Iterative Proxy `[[GetOwnProperty]]` descriptor traversal now reserves every
  directly owned operation, layer, trap, pending-frame, validation-descriptor,
  descriptor-object, and object-valued descriptor-field root before
  publication. Pending frames reserve only at actual vector growth, while a
  fixed three-slot root set retains target descriptor fields across observable
  `IsExtensible` work without another temporary vector.

  Reservation preserves ECMAScript error priority: trap and accessor
  callability checks precede their roots; on the `undefined` trap-result path,
  an absent target descriptor and a hidden non-configurable descriptor finish
  before unnecessary validation roots, while only configurable target fields
  survive across `IsExtensible`.
  Transparent forwarding creates no trap or pending-frame reservation,
  primitive fields create no descriptor-field root reservation, and spare
  pending capacity creates no frame-vector reservation. Failures remain
  catchable in the operation Realm, unwind all pins and frames, and permit a
  complete retry.

  Deterministic regressions cover all ten reservation sites, real GC-pin and
  pending-vector growth, first and second nested failures, revocation, fuel,
  callability and invariant priority, reverse validation, foreign Realms,
  forced-GC survival of a uniquely created descriptor value, cleanup, retry,
  and Object keys/values/entries caller ordering. Local verification passes all
  targets/features with **237/237** library tests, **539/539** builtins tests,
  **15/15** arguments tests, and **31/31** module tests, plus **236/236**
  release library tests, **135/135** Python tooling tests, **1/1** doctest,
  rustfmt, warnings-denied Clippy, release build, generated documentation, and
  wasm32 checking. Rustdoc retains only the 13 pre-existing broken-link
  warnings. Two GPT-5.6 final reviews are clean.

  Implementation commit `2918d70` passes CI `29980403698` and all 33 jobs in
  full run `29980403702`. Its 30 result files are byte-identical to the
  corrected `29977240759` baseline and aggregate to unchanged **31890 pass /
  5115 fail / 11459 skip / 3 timeout / 0 error / 48467 total / 37005 run**.
  The downloaded CI binary reproduces the 16-directory Proxy, Reflect, Object,
  and `for-in` cohort at **656 pass / 0 fail / 60 skip**. Final descriptor-object
  materialization, `Object.getOwnPropertyDescriptors` and
  `Object.defineProperties` containers, and Proxy `defineProperty` descriptor
  containers remain separate allocator-safety scopes.

- `Object.keys`, `Object.values`, `Object.entries`,
  `Object.getOwnPropertyNames`, `Object.getOwnPropertySymbols`, and
  `Reflect.ownKeys` now reserve caller-owned result vectors only at actual
  growth. `Object.values` reserves object roots before pinning returned values;
  `Object.entries` separately reserves pair elements, pair/result roots, and
  the outer result before publication. The shared Value-array path reserves GC
  pins and its dense-presence bitmap fallibly, and the obsolete String-array
  shortcut no longer bypasses rooted VM allocation.

  Result reservation remains after the required descriptor check and, for
  values/entries, after the successful `Get`. Empty and filtered results use no
  capacity or presence allocation. `Object.entries` moves the owned string key
  into its pair instead of allocating a second `Arc<str>`. `Reflect.ownKeys`
  now creates its result Array in the native callee Realm rather than the main
  Realm.

  Deterministic regressions cover exact spare/full capacity, first and second
  growth, all six APIs, filtered/empty paths, inner versus outer entries
  failures, producer-fuel priority, retry, foreign Array and RangeError Realms,
  root reservation, ephemeral getter values and pair survival through exact-cap
  GC, and balanced pin/context/native depths. Local verification passes all
  targets/features with **235/235** library tests, **539/539** builtins tests,
  **15/15** arguments tests, and **31/31** module tests, plus **234/234** release
  library tests, **135/135** Python tooling tests, **1/1** doctest, rustfmt,
  warnings-denied Clippy, release build, generated documentation, and wasm32
  checking. Rustdoc retains only the 13 pre-existing broken-link warnings.

  Implementation commit `2930f21` passes CI `29977240776` and all 33 jobs in
  full run `29977240759`. The original annexB artifact had one contention
  timeout; the downloaded CI binary on pinned Test262
  `020cb74075849d1e404bbcdb62feb7a02e6966db` reproduces the baseline
  **201/811/74/0/0** byte-for-byte. The corrected evidence set at
  `/tmp/ruja-ownkey-consumers-29977240759-corrected` aggregates to unchanged
  **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error / 48467 total /
  37005 run** and all 30 result files match run `29973887424`. Its downloaded
  binary reproduces the eight-directory consumer cohort at **244 pass / 0 fail
  / 68 skip**. Descriptor traversal state, other own-key callers, unrelated
  `ArrayData::new` sites, shared strings, and GC worklists remain separate.

- Ordinary `[[OwnPropertyKeys]]` now reserves all five directly owned native
  collections only when their next publication requires growth: index, String,
  and Symbol staging vectors plus the final result `Vec` and duplicate
  `IndexSet`. Final seen/result capacity is secured before either collection is
  mutated, and a duplicate key bypasses both reservations. The existing
  operation-wide fuel charge still completes before any collection
  materialization.

  Exact-capacity and integration regressions cover first and second actual
  growth, spare reuse, producer-level Array `length` duplication, empty and
  excluded paths, exact N-1/N fuel for all five sites, foreign operation Realms,
  Proxy trap/extensibility versus reverse descriptor priority, unposted and
  layered lazy `for...in` retry, balanced VM depths, and ordered success/retry
  across ordinary objects, dense and holey Arrays, primitive and boxed UTF-16
  Strings, attached and zero-length TypedArrays, Symbols, and Module Namespace
  exports. Local verification passes all targets/features with **233/233**
  library tests, **539/539** builtins tests, and **15/15** arguments tests,
  plus **232/232** release library tests, **31/31** module tests, **135/135**
  Python tooling tests, **1/1** doctest, rustfmt, warnings-denied Clippy,
  release build, generated documentation, and wasm32 checking. Rustdoc retains
  only the 13 pre-existing broken-link warnings. Implementation commit
  `055b36e` passes CI `29973887440` and all 33 jobs in full run
  `29973887424`. Artifacts at
  `/tmp/ruja-ordinary-ownkeys-collections-29973887424-final` aggregate to
  unchanged **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error /
  48467 total / 37005 run**; all 30 result files are byte-identical to run
  `29970600531`. Numeric key formatting, shared PropertyKey/Error strings, and
  caller-owned result containers remain separate allocator-safety scopes.

- Proxy `ownKeys` entry reservation now models native allocation only when the
  trap-result `Vec` or duplicate `IndexSet` is full. Spare-capacity publication
  bypasses both `try_reserve` and the injected failure, while the next actual
  growth still reports the existing catchable `RangeError`. This removes
  impossible per-entry OOM failures from the allocator-safety evidence without
  changing successful ECMAScript behavior.

  Helper-level regressions explicitly reserve spare capacity, check every slot
  up to the collection's reported capacity, and fail exactly at the full
  boundary. Integration coverage uses countdown failures across actual second
  growth, proves a two-key collection preserves the pending failure for the
  next fresh collection, and retains getter, type, duplicate, fuel, Symbol,
  Realm, nested-frame, retry, and lazy `for...in` ordering. Local verification
  passes all targets/features with **231/231** library tests, **539/539**
  builtins tests, and **15/15** arguments tests, plus **230/230** release
  library tests, **135/135** Python tooling tests, **1/1** doctest, rustfmt,
  warnings-denied Clippy, release build, generated documentation, and wasm32
  checking. Rustdoc retains only the 13 pre-existing broken-link warnings. The
  final GPT-5.6 review is clean. Commit `fbef166` passes CI `29970600535` and
  all 33 jobs in full run `29970600531`. Artifacts at
  `/tmp/ruja-proxy-ownkeys-growth-only-29970600531-final` aggregate to unchanged
  **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error / 48467 total /
  37005 run**; all 30 result files are byte-identical to run `29967042192`.
  The downloaded binary reproduces the selected Proxy, Reflect, Object, and
  `for-in` cohort at **211 pass / 0 fail / 60 skip**.

- Proxy `ownKeys` now reserves both post-validation collections before native
  growth. A non-extensible target's complete key list is observed before the
  target-key `IndexSet` reserves once for exact-set comparison, while the
  consumer-filtered `Vec` reserves only when an accepted String or Symbol
  would exceed its current capacity. Excluded Symbols, absent and
  non-enumerable descriptors, empty lists, and spare-capacity pushes therefore
  introduce no allocation failure.

  Exact-site regressions cover descriptor and omission priority, non-extensible
  mismatch errors, actual second growth, partial filtered-state retry, exact
  fuel boundaries, foreign operation Realms, nested reverse-frame observation,
  thrown-value identity, lazy `for...in` snapshot atomicity, and layered retry
  through both the filtered result and iterator snapshot. Local verification
  passes all targets/features with **230/230** library tests, **539/539**
  builtins tests, and **15/15** arguments tests, plus **229/229** release
  library tests, **135/135** Python tooling tests, **1/1** doctest, rustfmt,
  warnings-denied Clippy, release build, generated documentation, and wasm32
  checking. Rustdoc retains only the 13 pre-existing broken-link warnings. The
  final GPT-5.6 review found no code defect. Implementation commit `0740941`
  passes CI `29967042158` and all 33 jobs in full run `29967042192`. Artifacts
  at `/tmp/ruja-proxy-ownkeys-post-validation-29967042192-final` aggregate to
  unchanged **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error /
  48467 total / 37005 run**; all 30 result files are byte-identical to run
  `29963410566`. The downloaded binary reproduces the selected Proxy, Reflect,
  Object, and `for-in` cohort at **211 pass / 0 fail / 60 skip**. Shared index
  and PropertyKey/Error strings, ordinary own-key producers, GC root
  enumeration, and mark worklists remain separate allocator-safety scopes.

- Proxy `ownKeys` now reserves every directly owned temporary GC root before
  its corresponding pin: the operation input before dispatch, each Proxy
  target/handler pair after the edge fuel debit, an object trap-result list
  after list-type validation, and an object-valued `length` after `Get` but
  before observable `ToNumber`. Values that contribute no GC roots bypass the
  reservation, so primitive inputs and lengths and nullish trap forwarding do
  not introduce allocation failures.

  Exact-site and real GC-pin regressions cover revocation and fuel priority,
  `GetMethod`/Call/list/length abrupt ordering, primitive no-op boundaries,
  nullish forwarding, caller retry, foreign operation Realms, forced GC,
  already-published outer-frame cleanup, and uncommitted lazy `for...in`
  snapshots at all four sites. Local verification passes all targets/features
  with **229/229** library tests, **539/539** builtins tests, and **15/15**
  arguments tests, plus **228/228** release library tests, **135/135** Python
  tooling tests, rustfmt, warnings-denied Clippy, release build, generated
  documentation, and wasm32 checking. Rustdoc retains only the 13 pre-existing
  broken-link warnings. Two GPT-5.6 final reviews are clean. Implementation
  commit `633b3d8` passes CI `29963410587` and all 33 jobs in full run
  `29963410566`. Artifacts at
  `/tmp/ruja-proxy-ownkeys-direct-roots-29963410566-final` aggregate to unchanged
  **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error / 48467 total /
  37005 run**; all 30 result files are byte-identical to the corrected
  `29959362973` baseline. The downloaded binary reproduces the selected Proxy,
  Reflect, Object, and `for-in` cohort at **211 pass / 0 fail / 60 skip**.
  Post-validation filtered results, the non-extensible target-key set, index
  and PropertyKey/Error strings, GC root enumeration, and mark worklists remain
  separate allocator-safety scopes.

- Proxy `ownKeys` validation frames now reserve their operation-local vector
  before reserving the `current` and `target` GC roots. Only after both
  reservations succeed does the operation pin those values and publish the
  frame, so allocation failure cannot expose a half-published frame or leak a
  root. The boundary remains after trap-result collection, duplicate
  validation, and `IsExtensible`, and before nested target key traversal.

  Transparent forwarding creates no frame, while even an empty trapped result
  retains invariant state. Exact-site regressions cover frame and root
  failures, the real GC-pin reserve path, caller retry, duplicate,
  `IsExtensible`, and fuel priority, foreign Realms, second-frame countdown
  failures, forced GC, a 1,024-layer trapped chain, lazy `for...in` snapshot
  atomicity, and balanced pins, execution contexts, and native-call depth.

  Local verification passes all targets/features with **228/228** library
  tests, **539/539** builtins tests, and **15/15** arguments tests, plus
  **227/227** release library tests, **135/135** Python tooling tests, rustfmt,
  warnings-denied Clippy, release build, generated documentation, and wasm32
  checking. Rustdoc retains only the 13 pre-existing broken-link warnings. Two
  GPT-5.6 reviews are clean after publication order, actual root reservation,
  nested cleanup, Realm, and retry checks. Implementation commit `3903c54`
  passes CI `29959362979` and all 33 jobs in full run `29959362973`. The initial
  `annexB` artifact had two runner-contention timeouts; the same downloaded
  binary and exact pinned corpus reproduced the baseline **201/811/74/0/0**
  byte-for-byte. With that rerun substituted, artifacts at
  `/tmp/ruja-proxy-ownkeys-frame-state-29959362973-final` aggregate to unchanged
  **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error / 48467 total /
  37005 run**, and all 30 result files match run `29955284788`. The downloaded
  binary reproduces the selected Proxy, Reflect, Object, and `for-in` cohort at
  **211 pass / 0 fail / 60 skip**. Operation input, target/handler, trap-result
  list, and length-value roots, filtered results, non-extensible target sets,
  PropertyKey/Error strings, GC root enumeration, and mark worklists remain
  separate allocator-safety scopes.

- Proxy `ownKeys` trap-result collection now requests capacity for an
  additional native key only after the corresponding array-like `Get` succeeds
  and returns a String or Symbol. Duplicate validation remains a separate pass
  and reserves the `IndexSet` only after proving the current key is new. Native
  allocation failure therefore becomes a catchable `RangeError`. Trap-result
  reservation is reached only after the current index's fuel, `Get`, and
  successful type validation; `IndexSet` reservation is reached only after the
  current duplicate check. An abrupt earlier step returns first.

  A reservation failure discards every initialized operation-local collection
  and restores roots; a subsequent caller-initiated retry starts from the
  `ownKeys` trap. Empty lists require no entry reservation, and a duplicate
  requires no additional `IndexSet` reservation after its earlier trap-result
  collection. Symbol keys are still collected and duplicate-checked before
  consumer filtering. Exact-site regressions cover partial collection, retry
  observation, invalid and abrupt entries, exact fuel, duplicate countdown,
  Symbol identity, foreign Realms, nested pending-frame cleanup, and lazy
  `for...in` snapshot atomicity.

  Local verification passes all targets/features with **227/227** library
  tests, **539/539** builtins tests, and **15/15** arguments tests, plus
  **226/226** release library tests, **135/135** Python tooling tests, rustfmt,
  warnings-denied Clippy, release build, generated documentation, and wasm32
  checking. Rustdoc retains only the 13 pre-existing broken-link warnings. Two
  GPT-5.6 reviews are clean after exact ordering, duplicate no-reservation,
  Symbol filtering, Realm, and retry checks. Implementation commit `fe14b77`
  passes CI `29955284791` and all 33 jobs in full run `29955284788`.
  Downloaded artifacts at
  `/tmp/ruja-proxy-ownkeys-entry-state-29955284788-final` aggregate to unchanged
  **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error / 48467 total /
  37005 run**; all 30 result files are byte-identical to run `29951587187`.
  The downloaded binary reproduces the selected Proxy, Reflect, Object, and
  `for-in` cohort at **211 pass / 0 fail / 60 skip**. Operation input,
  target/handler, trap-result list, and length-value roots, pending validation
  frames and roots, filtered results, non-extensible target sets,
  PropertyKey/Error strings, GC root enumeration, and mark worklists remain
  separate allocator-safety scopes.

- Lazy `for...in` now reserves the iterator-owned key snapshot before
  publishing it and reserves the visited-key set before marking an existing
  descriptor. Native allocation failure therefore becomes a catchable
  `RangeError` without exposing a partial snapshot or a visited mark that was
  never committed. Symbol-only snapshots, absent descriptors, and already
  visited prototype duplicates do not consume these reservations.

  A visited-key reservation failure retains the already consumed candidate
  cursor. This preserves the specification order in which the candidate is
  removed before `[[GetOwnProperty]]`, matches the existing fuel and descriptor
  abrupt-completion policy, and lets a same-name prototype property be
  observed on retry when the failed child mark was not committed. Completed
  iterators release both key collection capacities. Exact-site regressions
  cover retry behavior, shadowing, duplicates, fuel priority, Proxy observation
  order, foreign-Realm errors, terminal cleanup, and balanced pins and
  execution contexts.

  Local verification passes all targets/features with **226/226** library
  tests, **539/539** builtins tests, and **15/15** arguments tests, plus
  **225/225** release library tests, **135/135** Python tooling tests, rustfmt,
  warnings-denied Clippy, release build, generated documentation, and wasm32
  checking. Rustdoc retains only the 13 pre-existing broken-link warnings. Two
  GPT-5.6 reviews are clean after retry semantics and exact no-reservation
  boundaries were resolved. Implementation commit `0686d0e` passes CI
  `29951588373` and all 33 jobs in full run `29951587187`. Downloaded artifacts
  at `/tmp/ruja-for-in-key-state-29951587187-final` aggregate to unchanged
  **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error / 48467 total /
  37005 run**; all 30 result files are byte-identical to run `29947430421`.
  The downloaded binary reproduces the affected Proxy and `for-in` cohort at
  **190 pass / 0 fail / 37 skip**. Proxy own-key trap collection,
  PropertyKey/Error strings, GC root enumeration, and mark worklists remain
  separate allocator-safety scopes.

- Shared ordinary property traversal now reserves its initial node set, each
  new directed edge and node, and every newly owned GC root before publishing
  state. Get, HasProperty, receiver-aware Set, ordinary Set, and inherited
  Proxy `GetMethod` paths therefore return a catchable `RangeError` on native
  traversal allocation failure without leaking pins or partially committing an
  edge. Fuel, ordinary-cycle rejection, and the 512 observable Proxy replay
  guard retain their prior ordering.

  Lazy `for...in` now keeps directed edges, rooted node identities, Proxy
  presence, and replay count in its GC-traced iterator state across separate
  `next()` calls. A cyclic Proxy can no longer reset the replay guard by
  yielding one fresh key per pull. Completed iterators release the retained
  collection capacities. Exact-site regressions cover construction and all
  edge reservations, retry behavior, foreign-Realm errors, zero-fuel priority,
  ordinary and Proxy cycles, GC slot reuse, and balanced pins and execution
  contexts.

  Local verification passes all targets/features with **225/225** library
  tests, **539/539** builtins tests, and **15/15** arguments tests, plus
  **224/224** release library tests, **135/135** Python tooling tests, rustfmt,
  warnings-denied Clippy, release build, generated documentation, and wasm32
  checking. Rustdoc retains only the 13 pre-existing broken-link warnings. Two
  GPT-5.6 reviews are clean after completed-iterator capacity retention and
  retry-semantics follow-up. Commit `0bac2a2` passes CI `29947430510` and all
  33 jobs in full run `29947430421`. Downloaded artifacts at
  `/tmp/ruja-property-traversal-29947430421-final` aggregate to unchanged
  **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error / 48467 total /
  37005 run**; all 30 result files are byte-identical to run `29927657329`.
  The downloaded binary reproduces the affected Proxy and `for-in` cohort at
  **190 pass / 0 fail / 37 skip**. Per-key `for-in` collections, trap-call
  internals, PropertyKey/Error strings, GC root enumeration, and mark
  worklists remain separate allocator-safety scopes.

- Proxy `[[GetPrototypeOf]]` and its nested `[[IsExtensible]]` validation now
  reserve every directly owned temporary root fallibly at the existing
  observable boundary. Input, target/handler, trap, returned prototype, and
  deferred expected-prototype roots return a catchable `RangeError` instead of
  reaching infallible `Vec` growth. The deferred validation vector uses
  `try_reserve` before `pin -> push`, so allocation failure cannot leak a root.

  Nested Proxy `[[IsExtensible]]` no longer stores an unbounded `Vec<bool>`;
  one first result plus an inconsistency flag preserves delayed invariant
  validation in O(1) space. Regressions inject failures at each exact reserve
  site, verify fuel/getter/call ordering, clean up four already-deferred roots,
  cover a deferred `null`, preserve a deeper sentinel throw over a known
  mismatch, materialize foreign-Realm errors, and complete a 1,024-layer
  validating chain with balanced roots and execution contexts.

  Local verification passes all targets/features with **224/224** library
  tests, **539/539** builtins tests, and **15/15** arguments tests, plus
  **223/223** release library tests, **135/135** Python tooling tests, **1/1**
  doctest, rustfmt, warnings-denied Clippy, release build, generated
  documentation, Python bytecode compilation, workflow YAML parsing, and
  wasm32 checking. Rustdoc retains only the 13 pre-existing broken-link
  warnings. Two GPT-5.6 reviews are clean after exact-site failpoints,
  delayed-abrupt priority, later deferred cleanup, and `null` continuation
  coverage were added. Commit `cff25cc` passes CI `29927666067` and all 33 jobs
  in full run `29927657329`. Downloaded artifacts at
  `/tmp/ruja-proxy-prototype-roots-29927657329-final` aggregate to unchanged
  **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error / 48467 total /
  37005 run**; all 30 result files are byte-identical to run `29922124267`.
  The downloaded binary reproduces Proxy getPrototypeOf/isExtensible **31/31**
  and adjacent `instanceof` **50/0/4**. Shared `PropertyTraversal`, trap-call,
  GC-worklist, and error-representation allocation remain separate scopes.

- `instanceof` and `OrdinaryHasInstance` now keep both operands rooted and
  traverse Bound Function targets iteratively. Each Bound edge consumes one
  fuel unit before observing the target; ordinary prototype edges consume one
  unit before `[[GetPrototypeOf]]`, while Proxy prototype traversal retains its
  existing internal debit. The prototype walk now performs
  `[[GetPrototypeOf]]` before `SameValue`, so `F.prototype instanceof F` is
  false instead of accepting a constructor's prototype object as its own
  instance.

  Direct and Bound/Proxy-wrapped uses of the default
  `%Function.prototype%[@@hasInstance]` share the iterative state machine,
  while observable Proxy `apply` traps still execute normally. Every native
  dispatch participates in a separate 128-frame active-native guard, making
  deep re-entrant native Proxy traps throw a catchable `RangeError` without
  weakening the existing 512-frame interpreted limit. The broader guard can
  also reject otherwise valid builtin/callback native re-entry deeper than
  128. Regressions cover 50,000 Bound layers, 10,000 transparent Proxy
  handlers, actual apply-trap recursion, exact fuel and revocation order,
  forced GC, stale prototype slots, abrupt identity, foreign Realms, injected
  reservation failure, and pin/depth restoration.

  Local verification passes all targets/features with **223/223** library
  tests, **539/539** builtins tests, and **15/15** arguments tests, plus
  **222/222** release library tests, **135/135** Python tooling tests, **1/1**
  doctest, rustfmt, warnings-denied Clippy, release build, generated
  documentation, Python bytecode compilation, workflow YAML parsing, and
  wasm32 checking. Rustdoc retains only the 13 pre-existing broken-link
  warnings. Two GPT-5.6 reviews are clean after wrapped-default recursion,
  actual apply-trap native recursion, fixture cleanup, primitive reservation,
  and foreign-Realm allocation-error findings were fixed. Feature commit
  `419501e` passes CI `29922123540` and all 33 jobs in full run `29922124267`.
  Downloaded artifacts at `/tmp/ruja-instanceof-29922124267-final` aggregate
  to the unchanged **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error
  / 48467 total / 37005 run** and all 30 result files are byte-identical to
  run `29916090227`. The downloaded binary reproduces `instanceof` **50/0/4**,
  the four forced skipped tests **4/4**, Proxy `getPrototypeOf` **19/19**, and
  Function.prototype **223/40/46**. Fallible allocation inside broader nested
  property and prototype traversal remains a separate runtime-wide unit.

- Ordinary Bound Function `[[Call]]` forwarding is now one iterative,
  fuel-metered state machine shared with Proxy apply dispatch. Every Bound or
  Proxy edge consumes fuel, layered bound arguments are collected once and
  materialized in linear order, the innermost bound `this` wins, and Proxy
  apply traps receive an argument Array from the current operation Realm. The
  cumulative argument limit is checked before an apply getter can observe an
  oversized call.

  The call inputs, wrappers, handlers, traps, and materialized arguments remain
  rooted across observable operations and collecting allocations. Native root
  and trap-argument reservations are fallible, all normal and abrupt exits
  restore the incoming pin depth, and Promise settlement/reaction exact-fuel
  behavior remains transactional. Regressions cover 20,000 Bound layers,
  Bound-Proxy-Bound ordering, exact fuel, forced GC, abrupt identity, foreign
  Realms, exact argument caps, and impossible root reservations. Local
  verification passes all targets/features with **222/222** library tests,
  **539/539** builtins tests, and **15/15** arguments tests, plus **221/221**
  release library tests, **135/135** Python tooling tests, **1/1** doctest,
  rustfmt, warnings-denied Clippy, release build, generated documentation,
  Python bytecode compilation, workflow YAML parsing, and wasm32 checking.
  Rustdoc retains only the 13 pre-existing broken-link warnings. Semantic
  review was clean, while resource review raised and closed infallible native
  reservation paths in the feature commit. Later documentation review found
  one remaining infallible root-vector growth inside the shared trap-array
  allocator. Follow-up commit `c64076f` pre-reserves its exact item and
  prototype roots and adds a test-only one-shot reservation failure proving a
  catchable `RangeError` before pin mutation. GPT-5.6 re-review is clean.
  Feature commit `026ea21` passes CI `29912648216` and full run `29912648078`;
  follow-up `c64076f` passes CI `29916090205` and all 33 jobs in full run
  `29916090227`. Final downloaded artifacts at
  `/tmp/ruja-value-array-roots-29916090227-final` aggregate to the unchanged
  **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error / 48467 total /
  37005 run**. All 30 result files are byte-identical to feature run
  `29912648078`, and the downloaded release binary reproduces
  Function.prototype **223/40/46** and Promise **442/0/287**. Bound forwarding
  in `OrdinaryHasInstance` remains a separate `instanceof` unit.

- `Function.prototype.bind` now creates Bound Function exotic objects with
  specification-shaped own `length` and `name` properties. The target's own
  `length` is observed through `[[GetOwnProperty]]` before an optional `Get`,
  numeric lengths are truncated and reduced by the bound argument count
  without coercing non-numbers, and `name` is read unconditionally and
  prefixed with `"bound "`. Both properties are non-writable,
  non-enumerable, configurable data properties in `length`, `name` order.
  Deleting the configurable bound `name` now resumes ordinary prototype lookup
  instead of reviving an internal diagnostic name.

  The bound object is allocated and rooted before the observable length and
  name operations. Regressions cover Proxy order, abrupt completion identity,
  forced GC, exact heap caps, signed zero, non-number no-coercion, inherited
  metadata, deletion and rebinding, own-key shape, and pin restoration. Exact
  Test262 admission freezes the nine previously failing files and their live
  feature, include, flag, and negative metadata; the cohort is **9/9**, the
  complete bind directory moves from **84 pass / 7 fail / 9 skip** to **93
  pass / 0 fail / 7 skip**, and `Function.prototype` is **223 pass / 40 fail /
  46 skip**. Local verification passes all targets/features with **221/221**
  library tests, **539/539** builtins tests, and **15/15** arguments tests,
  plus **220/220** release library tests, **135/135** Python tooling tests,
  **1/1** doctest, rustfmt, warnings-denied Clippy, release build, generated
  documentation, Python bytecode compilation, workflow YAML parsing, and
  wasm32 checking. Rustdoc retains only the 13 pre-existing broken-link
  warnings. GPT-5.6 review is clean after fixing deleted-name inheritance and
  expanding abrupt GC/cap coverage. Feature commit `75c030a` passes CI
  `29907748052` and all 33 jobs in full run `29907748376`. Downloaded artifacts
  aggregate to **31890 pass / 5115 fail / 11459 skip / 3 timeout / 0 error /
  48467 total / 37005 run**. Against full run `29903293969`, 29 result files
  are byte-identical and `built-ins` alone changes by **+9 pass / -7 fail / -2
  skip**. The downloaded release binary reproduces exact **9/9** and complete
  bind-directory **93/0/7**. Iterative, fuel-bounded Bound `[[Call]]` dispatch
  remains a separate resource-safety unit.

- Labelled statements now reject generator, async-function, and
  async-generator declarations in sloppy as well as strict code. The Annex B
  exception remains limited to ordinary sloppy `label: function f() {}`;
  `async` followed by a line terminator remains an expression statement rather
  than an async declaration. Class declaration and expression names whose
  decoded StringValue is `await` now also fail throughout a Module source goal,
  including inside nested ordinary functions, whether the source spelling is
  raw or escaped.

  Exact Test262 admission freezes the three labelled-declaration parse-negative
  files and the two module class-name files with per-file feature and module
  metadata shared by runner and analyzer. Full-matrix setup validates the live
  pinned files before scheduling shards; future siblings, extra features, and
  unrelated module files remain gated. The exact cohort is **5/5**, and the
  labelled-statement directory is **21 pass / 0 fail / 3 skip**. Local
  verification passes all targets/features with **219/219** library tests,
  **537/537** builtins tests, and **15/15** arguments tests, plus **218/218**
  release library tests, **134/134** Python tooling tests, **1/1** doctest,
  rustfmt, warnings-denied Clippy, release build, generated documentation,
  Python bytecode compilation, workflow YAML parsing, and wasm32 checking.
  Rustdoc retains only the 13 pre-existing broken intra-doc-link warnings. Two
  GPT-5.6 reviews are clean after extending the class-name rule from module
  top-level code to the complete Module parse goal. Feature commit `f1c421d`
  passes CI `29903293887` and all 33 jobs in full run `29903293969`, including
  the new live admission preflight. Downloaded artifacts aggregate to **31881
  pass / 5122 fail / 11461 skip / 3 timeout / 0 error / 48467 total / 37003
  run**. Against full run `29899362112`, 28 result files are byte-identical;
  `language/expressions` moves by **+1 pass / -1 skip** and
  `language/statements` by **+4 pass / -4 skip**. The downloaded release binary
  reproduces exact coverage **5/5** and labelled coverage **21 pass / 0 fail /
  3 skip**.

- Generic `ToObject` coercion now throws a method-Realm `TypeError` for
  `null` and `undefined` instead of manufacturing wrapper objects. The
  `Object` constructor keeps its distinct specification path: ordinary
  `Object(nullish)` and `new Object(nullish)` create a fresh ordinary object in
  the active function Realm, while construction with a distinct `NewTarget`
  keeps its constructor-derived prototype path. Nullish `Object.assign`
  sources and the other algorithms with explicit nullish exceptions continue
  to branch before the shared coercion helper.

  The four previously failing pinned Test262 cases in `Object.create` and
  `Object.defineProperties` now pass. Their focused directories move from
  **942 pass / 4 fail / 6 skip** to **946 pass / 0 fail / 6 skip**, and the
  complete `built-ins/Object` subtree moves from **3295 pass / 4 fail / 112
  skip** to **3299 pass / 0 fail / 112 skip**. Local verification passes all
  targets/features with **218/218** library tests, **537/537** builtins tests,
  and **15/15** arguments tests, plus **217/217** release library tests,
  **133/133** Python tooling tests, **1/1** doctest, rustfmt, warnings-denied
  Clippy, release build, generated documentation, and wasm32 checking. Rustdoc
  retains only the 13 pre-existing broken intra-doc-link warnings. Two
  independent GPT-5.6 reviews found no runtime defect after correcting one
  newly added `Object.create` test expectation. Feature commit `401a73a`
  passes CI `29899362133` and all 33 jobs in full run `29899362112`.
  Downloaded artifacts aggregate to **31876 pass / 5122 fail / 11466 skip / 3
  timeout / 0 error / 48467 total / 36998 run**. Against full run
  `29895173852`, 29 result files are byte-identical and only `built-ins` moves
  by **+4 pass / -4 fail**. The downloaded release binary reproduces focused
  coverage **946/946** and complete `built-ins/Object` coverage **3299/3299**.

- TypedArray `includes`, `indexOf`, and `lastIndexOf` now consume one
  cooperative fuel unit before every visited logical index. Validation,
  internal-length snapshot, `fromIndex` coercion, empty-range returns,
  SameValueZero for `includes`, strict equality for the index methods, and
  resize behavior retain their existing order. Immediate matches charge only
  the indices actually visited, while empty and out-of-range searches consume
  no loop fuel.

  Direct-native tests cover N-1 exhaustion, exact completion, early matches,
  zero-length views, nonempty empty ranges, Number and BigInt values, fuel
  remainder, and pin balance. Current pinned Test262 remains **45/45** for
  `includes`, **43/43** for `indexOf`, and **42/42** for `lastIndexOf`, or
  **130/130** combined. Local verification passes all targets/features with
  **218/218** library tests, **537/537** builtins tests, and **15/15** arguments
  tests, plus **217/217** release library tests, **133/133** Python tooling
  tests, **1/1** doctest, rustfmt, warnings-denied Clippy, release build,
  generated documentation, and wasm32 checking. Two independent GPT-5.6
  reviews found no remaining runtime or test defect. Feature commit `147f2b2`
  passes CI `29895173924` and all 33 full-matrix jobs in `29895173852`.
  Downloaded artifacts aggregate to the unchanged **31872 pass / 5126 fail /
  11466 skip / 3 timeout / 0 error / 48467 total / 36998 run**; all 30 result
  files are byte-identical to full run `29892602512`. The downloaded release
  binary reproduces the combined search cohort **130/130**.

- `%TypedArray%.prototype.join` now retains its receiver and observed separator
  across separator coercion, performs one cooperative fuel charge for every
  captured index, and grows its intermediate result through checked
  reservation instead of an infallible `Vec<String>` plus concatenation. The
  existing ValidateTypedArray, internal-length snapshot, separator coercion,
  live integer-indexed `Get`, and element `ToString` order is preserved for
  Number and BigInt views, including resize, detach, foreign-Realm errors,
  abrupt completion, and forced GC.

  Test262 admission now freezes the complete **32-file** join directory and
  each file's exact feature metadata. Runner and analyzer share that closed
  boundary; tooling verifies the live directory, includes, flags, negative
  metadata, disjointness, invalid paths, extra features, and future siblings.
  The full-matrix setup verifies both exact TypedArray string admissions before
  scheduling shards. Direct join remains **32/32**, and adjacent TypedArray
  join, Array join, and TypedArray `toLocaleString` remain **94/94**. Local
  verification passes all targets/features with **216/216** library tests,
  **536/536** builtins tests, and **15/15** arguments tests, plus **215/215**
  release library tests, **133/133** Python tooling tests, **1/1** doctest,
  rustfmt, warnings-denied Clippy, release build, generated documentation, and
  wasm32 checking. Two independent GPT-5.6 reviews found no runtime or
  admission defect. Final `Arc<str>` publication retains the runtime-wide
  infallible-allocation limitation. Feature commit `7605669` passes CI
  `29890470545` and all 33 full-matrix jobs in `29890470558`. Downloaded
  artifacts aggregate to the unchanged **31872 pass / 5126 fail / 11466 skip /
  3 timeout / 0 error / 48467 total / 36998 run**; all 30 result files are
  byte-identical to full run `29888462280`. The downloaded release binary also
  reproduces direct join **32/32** and adjacent combined coverage **94/94**.

- `%TypedArray%.prototype.toLocaleString` now keeps TypedArray-specific
  validation and internal-length snapshot semantics while performing primitive
  `GetV` lookup for every current element. The selected `toLocaleString` is
  called with the element as `this` and no arguments, so the non-ECMA-402
  runtime ignores caller locale/options values and mutable global `Number` or
  `BigInt` replacement no longer changes intrinsic primitive lookup.

  The source, current value, selected method, returned value, and abrupt
  completions remain rooted across observable getters, calls, conversion, and
  forced GC. Every captured index consumes one fuel unit and intermediate
  output growth uses fallible reservation; final `Arc<str>` publication retains
  the runtime-wide infallible-allocation limitation. The existing fixed-checkout
  Test262 cohort remains **39/39**, Array `toLocaleString` remains **12/12**,
  and TypedArray-constructor inheritance remains **2/2**. Local verification
  passes all targets/features with **214/214** library tests, **535/535**
  builtins tests, and **15/15** arguments tests, plus **213/213** release
  library tests, **133/133** Python tooling tests, **1/1** doctest, rustfmt,
  warnings-denied Clippy, release build, generated documentation, and wasm32
  checking. Two independent GPT-5.6 reviews found no runtime defect.
  Feature commit `0a5d7ea` passes CI `29886017567` and full matrix
  `29886017568`. Downloaded artifacts aggregate to the unchanged **31872 pass /
  5126 fail / 11466 skip / 3 timeout / 0 error / 48467 total / 36998 run**;
  all 30 result files are byte-identical to full run `29883661759`. The
  downloaded release binary independently reproduces diagnostic `1|0|1`,
  direct TypedArray locale **39/39**, and the adjacent combined **14/14**.

- `Array.prototype.toLocaleString` now has its own generic ECMAScript
  algorithm instead of aliasing `Array.prototype.toString`. It performs
  `ToObject`, captures `LengthOfArrayLike` once, uses RuJa's
  implementation-defined `","` list separator, and performs a live `Get` for
  every index without `HasProperty`. Null and undefined produce empty fields;
  every other element receives an observable `toLocaleString` lookup and call
  with the original element as `this` and no arguments, followed by `ToString`
  of the returned value. Locale/options arguments are deliberately ignored in
  the current non-ECMA-402 runtime.

  Receiver, boxed source, current element, selected method, and returned value
  remain rooted across length coercion, Proxy traps, invocation, conversion,
  and forced GC. Direct, indirect, and join/toLocaleString cross-recursion use
  one balanced stringification stack; every captured index consumes one fuel
  unit. Output growth uses fallible reservation without an intermediate
  `Arc<str>` copy, although final `Arc<str>` publication retains the existing
  runtime-wide allocation limitation. Exact admission freezes only the four
  direct files hidden by broad `Reflect.construct`, arrow-function, or
  resizable-buffer gates. On fixed Test262 checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the preceding policy and binary
  are **3 pass / 5 fail / 4 skip**; applying the final policy to that binary is
  **5/7/0**; the repaired runtime under the preceding policy is **8/0/4**; and
  the final cohort is **12/12**. Adjacent Array join remains **23/23**,
  `%TypedArray%.prototype.toLocaleString` remains **39/39**, and
  `Object.prototype.toLocaleString` remains **12/12**. Local verification
  passes all targets/features with **212/212** library tests, **535/535**
  builtins tests, and **15/15** arguments tests, plus **211/211** release
  library tests, **133/133** Python tooling tests, **1/1** doctest, rustfmt,
  warnings-denied Clippy, release build, generated documentation, and wasm32
  checking. Feature commit `2dd3041` passes CI `29883661773` and full matrix
  `29883661759`. Downloaded artifacts aggregate to **31872 pass / 5126 fail /
  11466 skip / 3 timeout / 0 error / 48467 total / 36998 run**; only the
  built-ins result changes from the preceding matrix, by exactly **+9 pass /
  -5 fail / -4 skip**. The downloaded release binary independently reproduces
  every focused and adjacent cohort plus the forced detached-method diagnostic.

- `Array.prototype.toSpliced` now follows the generic ECMAScript
  change-array-by-copy algorithm instead of splicing a represented-Array
  snapshot. It performs `ToObject`, captures `LengthOfArrayLike` once,
  uses argument count so no arguments delete nothing, one `start` argument
  deletes the tail, and an explicit `undefined` `skipCount` deletes nothing. It
  creates a fresh intrinsic Array in the method Realm without reading
  `constructor` or `Symbol.species`. Retained prefix and suffix indices are read
  live in specification order, discarded indices are not read, inserted values
  are placed between those ranges, and holes become own `undefined` data
  properties in the dense result.

  Receiver, arguments, boxed source, fresh result, and copied values remain
  rooted across coercion, allocation, Proxy traps, property definition, and
  forced GC. Result allocation retries after collecting garbage, an exact-cap
  failure happens before indexed access, every result index consumes one loop
  plus one property-definition fuel unit, and computed result lengths above
  `2^53 - 1` fail in the method Realm. Exact admission freezes only the direct
  `not-a-constructor.js` path hidden by the broad `Reflect.construct` gate. On
  fixed Test262 checkout `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the
  preceding policy and binary are **17 pass / 12 fail / 1 skip**; applying the
  final policy to that binary is **18/12/0**; the repaired runtime under the
  preceding policy is **29/0/1**; and the final cohort is **30/30**. Adjacent
  `Array.prototype.splice` remains **81/81**. Local verification passes all
  targets/features with **210/210** library tests, **534/534** builtins tests,
  and **15/15** arguments tests, plus **209/209** release library tests,
  **132/132** Python tooling tests, **1/1** doctest, rustfmt, warnings-denied
  Clippy, release build, generated documentation, and wasm32 checking. Feature
  commit `174f006` passes CI `29850160241` and full matrix `29850160482`.
  Downloaded artifacts aggregate to **31863 pass / 5131 fail / 11470 skip / 3
  timeout / 0 error / 48467 total / 36994 run**; only the built-ins result
  changes from the preceding matrix, by exactly **+13 pass / -12 fail / -1
  skip**. The downloaded release binary independently reproduces Array
  toSpliced **30/30** and Array splice **81/81**.

- `Array.prototype.toReversed` now follows the generic ECMAScript
  change-array-by-copy algorithm instead of reversing represented-Array
  storage. It performs `ToObject`, captures `LengthOfArrayLike` once, creates a
  fresh intrinsic Array in the method Realm without reading `constructor` or
  `Symbol.species`, then performs live descending `Get` operations. Holes and
  missing inherited indices become own `undefined` data properties, source
  mutation by getters is observed by later reads, and the source itself is not
  mutated by the method. Primitive receivers, abrupt completions, the
  `2^32 - 1` Array length boundary, and method-Realm results and errors are
  handled in specification order.

  Receiver, boxed source, fresh result, and current values remain rooted across
  allocation, Proxy traps, property definition, and forced GC. Result
  allocation retries after collecting garbage, an exact-cap failure happens
  before indexed access, and every copied index consumes one loop plus one
  property-definition fuel unit. Exact admission freezes only the direct
  `not-a-constructor.js` path hidden by the broad `Reflect.construct` gate. On
  fixed Test262 checkout `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the
  preceding policy and binary are **8 pass / 8 fail / 1 skip**; applying the
  final policy to that binary is **9/8/0**; the repaired runtime under the
  preceding policy is **16/0/1**; and the final cohort is **17/17**. Adjacent
  `%TypedArray%.prototype.toReversed` remains **9/9**. Local verification
  passes all targets/features with **207/207** library tests, **533/533**
  builtins tests, and **15/15** arguments tests, plus **206/206** release
  library tests, **131/131** Python tooling tests, **1/1** doctest, rustfmt,
  warnings-denied Clippy, release build, generated documentation, and wasm32
  checking. Feature commit `0e90184` passes CI `29845649747` and full matrix
  `29845649723`. Downloaded artifacts aggregate to **31850 pass / 5143 fail /
  11471 skip / 3 timeout / 0 error / 48467 total / 36993 run**; only the
  built-ins result changes from the preceding matrix, by exactly **+9 pass /
  -8 fail / -1 skip**. The downloaded release binary independently reproduces
  Array toReversed **17/17** and TypedArray toReversed **9/9**.

- `Array.prototype.reverse` now follows the generic in-place ECMAScript
  algorithm instead of directly reversing represented-Array storage. It
  performs `ToObject`, one `LengthOfArrayLike` snapshot, and ordered lower/
  upper `HasProperty` and conditional `Get` operations before applying the
  specified `Set`/`Set`, `Set`/`Delete`, or `Delete`/`Set` mutation for each
  pair. Generic and primitive receivers, holes, inherited properties, live
  Proxy traps, strict mutation failures, partial abrupt effects, method-Realm
  errors, and the original receiver return value are observable correctly.

  Receiver and boxed object remain operation roots, while both fetched pair
  values remain rooted across opposite-side traps and mutations. Pair-local
  roots are released after every normal or abrupt pair, and every pair consumes
  one cooperative fuel unit without imposing a source-length materialization
  cap. Exact admission freezes only the two direct reverse files hidden by
  broad feature gates. On fixed Test262 checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the preceding policy and binary
  are **7 pass / 9 fail / 2 skip**; applying the final policy to that binary is
  **8/10/0**; the repaired runtime under the preceding policy is **16/0/2**;
  and the final cohort is **18/18**. Adjacent
  `%TypedArray%.prototype.reverse` remains **22/22**, and the broader
  forced-gate detached Array-method diagnostic now passes completely while the
  file remains skipped by normal broad feature policy. Local verification
  passes all targets/features with **204/204** library tests, **532/532**
  builtins tests, and **15/15** arguments tests, plus **204/204** release
  library tests, **130/130** Python tooling tests, **1/1** doctest, rustfmt,
  warnings-denied Clippy, release build, generated documentation, and wasm32
  checking. Feature commit `2aa46ec` passes CI `29833297247` and full matrix
  `29833297271`. Downloaded artifacts aggregate to **31841 pass / 5151 fail /
  11472 skip / 3 timeout / 0 error / 48467 total / 36992 run**; only the
  built-ins result changes from the preceding matrix, by exactly **+11 pass /
  -9 fail / -2 skip**. The downloaded release binary independently reproduces
  Array reverse **18/18**, TypedArray reverse **22/22**, and the passing
  forced-gate detached-method diagnostic.

- `Array.prototype.reduceRight` now follows the generic ECMAScript algorithm
  instead of reducing a reversed copy of represented-Array storage. It performs
  `ToObject`, one `LengthOfArrayLike` snapshot, callback validation, live
  descending `HasProperty`/`Get` accumulator discovery when the initial value
  is omitted, and live callback traversal from `length - 1` to zero. Generic
  and primitive receivers, holes, inheritance, mutation, Proxy traps, explicit
  `undefined` initial values, callback arguments, abrupt completion, and
  method-Realm errors are observable correctly.

  Receiver, arguments, boxed object, current value, and accumulator remain
  rooted across observable work. Callback result roots replace prior
  accumulator roots in LIFO order with O(1) native root growth. Every examined
  logical index consumes one fuel unit, and decrement-before-access loops avoid
  unsigned underflow at zero. Exact admission freezes only the five direct
  reduceRight files hidden by broad feature gates. On fixed Test262 checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the preceding policy and binary
  are **94 pass / 161 fail / 5 skip**; applying the final policy to that binary
  is **95/165/0**; the repaired runtime under the preceding policy is
  **255/0/5**; and the final cohort is **260/260**. Adjacent
  `%TypedArray%.prototype.reduceRight` remains **50/50**. The detached-method
  diagnostic now clears reduceRight and reaches the independent generic
  `reverse` gap. Local verification passes all targets/features with
  **202/202** library tests, **531/531** builtins tests, and **15/15**
  arguments tests, plus **202/202** release library tests, **129/129** Python
  tooling tests, **1/1** doctest, rustfmt, warnings-denied Clippy, release
  build, generated documentation, and wasm32 checking. Feature commit
  `61cd755` passes CI `29829395637` and full matrix `29829395686`. Downloaded
  artifacts aggregate to **31830 pass / 5160 fail / 11474 skip / 3 timeout /
  0 error / 48467 total / 36990 run**; only the built-ins result changes from
  the preceding matrix, by exactly **+166 pass / -161 fail / -5 skip**. The
  downloaded release binary independently reproduces Array reduceRight
  **260/260** and TypedArray reduceRight **50/50**.

- `Array.prototype.reduce` now follows the generic ECMAScript algorithm
  instead of reducing a copied represented-Array backing vector. It performs
  `ToObject`, one `LengthOfArrayLike` snapshot, callback validation, live
  `HasProperty`/`Get` accumulator discovery when the initial value is omitted,
  and ascending live indexed callback traversal. Generic and primitive
  receivers, holes, inheritance, mutation, Proxy traps, explicit `undefined`
  initial values, callback arguments, abrupt completion, and method-Realm
  errors are observable correctly.

  Receiver, arguments, boxed object, current value, and accumulator are rooted
  across every observable operation. Callback result roots replace prior
  accumulator roots in LIFO order without growing with iteration count. Every
  logical index, including holes examined during initial discovery, consumes
  one fuel unit, and all exits restore pin depth. Exact admission freezes only
  the five direct reduce files hidden by broad feature gates. On fixed Test262
  checkout `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the preceding policy and
  binary are **89 pass / 166 fail / 5 skip**; applying the final policy to that
  binary is **90/170/0**; the repaired runtime under the preceding policy is
  **255/0/5**; and the final cohort is **260/260**. Adjacent
  `%TypedArray%.prototype.reduce` remains **50/50**. The detached-method
  diagnostic now clears reduce and reaches the independent generic
  `reduceRight` gap. Local verification passes all targets/features with
  **200/200** library tests, **530/530** builtins tests, and **15/15**
  arguments tests, plus **200/200** release library tests, **128/128** Python
  tooling tests, **1/1** doctest, rustfmt, warnings-denied Clippy, release
  build, generated documentation, and wasm32 checking. Feature commit
  `5362a2c` passes CI `29825824540` and full matrix `29825824539`. Downloaded
  artifacts aggregate to **31664 pass / 5321 fail / 11479 skip / 3 timeout /
  0 error / 48467 total / 36985 run**; only the built-ins result changes from
  the preceding matrix, by exactly **+171 pass / -166 fail / -5 skip**. The
  downloaded release binary independently reproduces Array reduce **260/260**
  and TypedArray reduce **50/50**.

- `Array.prototype.map` now follows the generic ECMAScript algorithm instead
  of mapping a copied represented-Array backing vector. It performs
  `ToObject`, one `LengthOfArrayLike` snapshot, callback validation,
  `ArraySpeciesCreate`, then live `HasProperty`/`Get`, callback, and
  `CreateDataPropertyOrThrow` operations in specification order. Generic and
  primitive receivers, holes, inheritance, mutation, Proxy traps, custom
  species, resizable TypedArrays, abrupt completion, and method-Realm results
  are observable correctly.

  Native-frame values are pinned across every getter, callback, species
  constructor, Proxy definition, and forced collection. Result creation and
  every logical source index consume cooperative fuel, while normal, property,
  callback, definition, allocation, and fuel exits restore pin depth. Exact
  admission freezes only the nine direct map files hidden by broad feature
  gates. On fixed Test262 checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the preceding policy and binary
  are **95 pass / 111 fail / 9 skip / 1 timeout**; applying the final policy
  to that binary is **96/119/0/1**; the repaired runtime under the preceding
  policy is **207/0/9/0**; and the final cohort is **216/216**. Adjacent
  `%TypedArray%.prototype.map` remains **85/85**. The detached-method
  diagnostic now clears map and reaches the independent generic `reduce` gap.

  Local verification passes all targets and features with **198/198** library
  tests, **529/529** builtins tests, and **15/15** arguments tests, plus
  **198/198** release library tests, **127/127** Python tooling tests, **1/1**
  doctest, rustfmt, warnings-denied Clippy, release build, generated
  documentation, and wasm32 checking.

  CI `29821941785` and full Test262 `29821941873` pass. Downloaded full-run
  artifacts aggregate to **31493 pass / 5487 fail / 11484 skip / 3 timeout /
  0 error / 48467 total / 36980 run**, exactly **+121 pass / -111 fail / -9
  skip / -1 timeout** from the preceding join run. Only the `built-ins` shard
  changes; all other 29 result files are byte-identical. The downloaded binary
  independently reproduces Array map **216/216** and TypedArray map **85/85**.

- `Array.prototype.join` now follows the generic ECMAScript algorithm instead
  of joining a copied represented-Array backing vector. It performs
  `ToObject`, one `LengthOfArrayLike` snapshot, separator coercion, and live
  indexed `Get` plus element `ToString` operations in specification order.
  Generic and primitive receivers, holes, inherited indices, Proxy traps,
  separator mutation, element mutation, resizable TypedArrays, nullish
  elements, abrupt conversions, and method-Realm errors are observable
  correctly.

  Receiver and arguments are pinned before boxing; the boxed object and each
  current element remain roots across getters, Proxy traps, separator or
  element coercion, and forced collection. Every logical index consumes one
  fuel unit, all exits restore pin depth, and incremental string growth uses
  checked reservation that reports `RangeError` instead of panicking on an
  impossible result capacity. Active receiver tracking starts only after
  separator coercion, so valid finite separator re-entry remains observable
  while direct and indirect cyclic element conversion produces an empty field
  without overflowing the Rust stack.

  Exact admission freezes only the four direct join files hidden by broad
  feature gates. On fixed Test262 checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the preceding binary and policy
  are **15 pass / 4 fail / 4 skip**; applying the final policy to that binary
  is **16/7/0**; and the repaired runtime is **23/23**. Adjacent
  `%TypedArray%.prototype.join` remains **32/32**. The broader detached-method
  diagnostic now clears join and reaches the independent generic `map` gap.

  Local verification passes all targets and features with **197/197** library
  tests, **528/528** builtins tests, and **15/15** arguments tests, plus
  **196/196** release library tests, **126/126** Python tooling tests, **1/1**
  doctest, rustfmt, warnings-denied Clippy, release build, generated
  documentation, and wasm32 checking.

  CI `29817687979` and full Test262 `29817687917` pass. Downloaded full-run
  artifacts aggregate to **31372 pass / 5598 fail / 11493 skip / 4 timeout /
  0 error / 48467 total / 36970 run**, a **+10 pass / -6 fail / -4 skip**
  change from the preceding run with no reverse transition. Only the
  `built-ins` shard changes: direct join contributes the expected eight net
  passes, while generic join also repairs two `Array.prototype.toString`
  tests that delegate to it. The downloaded binary independently reproduces
  Array join **23/23** and TypedArray join **32/32**.

- `Array.prototype.forEach` now follows the generic ECMAScript algorithm
  instead of iterating a copied represented-Array backing vector. It performs
  `ToObject`, one `LengthOfArrayLike` snapshot, callback validation, and live
  `HasProperty`/`Get` operations in specification order. Generic and primitive
  receivers, holes, inherited indices, Proxy traps, callback mutation,
  `thisArg`, callback arguments, abrupt completion, and method-Realm errors are
  therefore observable correctly.

  The receiver, callback arguments, boxed object, and current value remain GC
  roots across property operations and callback execution. Every logical
  index, including a hole, consumes one fuel unit, and every normal, callback,
  property, or fuel exit restores the incoming pin depth.

  Exact admission freezes only the five direct forEach files hidden by broad
  feature gates. On fixed Test262 checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the preceding binary under the
  final policy is **91 pass / 98 fail / 0 skip / 1 timeout**; the repaired
  runtime is **190 pass / 0 fail / 0 skip / 0 timeout**. Adjacent
  `%TypedArray%.prototype.forEach` remains **42/42**. The broader detached
  method diagnostic now clears forEach and reaches the independent generic
  `join` gap.

  Local verification passes all targets and features with **195/195** library
  tests, **526/526** builtins tests, and **15/15** arguments tests, plus
  **194/194** release library tests, **125/125** Python tooling tests, **1/1**
  doctest, rustfmt, warnings-denied Clippy, release build, generated
  documentation, and wasm32 checking. GPT-5.6 runtime reviewer McClintock
  (`019f83a7-5428-7442-b062-92f1b5a9c781`) and admission reviewer Raman
  (`019f83a7-55e4-7680-944c-4114e05d91fe`) report `CLEAN` after the stale
  limitations entry was corrected; both sessions are closed.

  Feature commit `24458d56c82462d0503c0ac7882b2359c7d95315` is pushed to
  `main`. Ordinary CI `29812349225` passes both jobs, and full matrix
  `29812349142` passes all **33/33** jobs. Its 30 result artifacts at
  `/tmp/ruja-array-for-each.29812349142` aggregate to **31362 pass / 5604 fail
  / 11497 skip / 4 timeout / 0 error / 48467 total / 36966 pass-or-fail
  executed**.

  Against `/tmp/ruja-array-flat.29808658857`, 29 result files are
  byte-identical. Only `test262_built-ins_result.txt` changes from
  **15652/4887/3124/5/0** to **15752/4793/3119/4/0**, exactly **+100 pass /
  -94 fail / -5 skip / -1 timeout** with no error, total, or unrelated-shard
  drift. The downloaded release binary independently reproduces direct Array
  forEach **190/190** and TypedArray forEach **42/42** on the fixed checkout.

- `Array.prototype.flat` and `flatMap` now share a generic, species-aware
  `FlattenIntoArray` implementation instead of copying represented-Array
  backing vectors. Both methods box receivers, snapshot `LengthOfArrayLike`,
  preserve their specified validation and species order, and perform live
  `HasProperty`, `Get`, `IsArray`, nested length, mapper, and
  `CreateDataPropertyOrThrow` operations. Holes, inherited indices, generic
  and Proxy receivers, callback mutation, custom species, and partial abrupt
  results are observable in specification order.

  Flattening uses an explicit frame stack rather than Rust recursion. Nested
  sources and mapped values remain pinned across getters, callbacks, Proxy
  traps, collection, and target definitions; every normal, semantic,
  allocation, property, callback, and fuel exit restores the incoming pin
  depth. Each visited source index consumes fuel. Cyclic `flat(Infinity)`
  terminates through the configured sandbox budget or, with unbounded fuel,
  after 512 observable active-path replays instead of overflowing the native
  stack or growing until host OOM.

  Exact admission keeps the two method surfaces separate and freezes only the
  10 files hidden by broader feature gates. On fixed Test262 checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the preceding binary and policy
  are **18 pass / 15 fail / 10 skip**; applying the final exact policy to that
  binary is **20/23/0**; and the repaired runtime is **43/43**. The broader
  detached-method diagnostic now clears both methods and reaches the
  independent `forEach` gap.

  Local all-target/all-feature tests pass with **193/193** library tests,
  **524/524** builtins tests, **15/15** arguments tests, **192/192** release
  library tests, **124/124** Test262 tooling tests, and **1/1** doctest, plus
  rustfmt, warnings-denied Clippy, release build, generated documentation, and
  wasm32 checking. GPT-5.6 reviewers Russell
  (`019f834f-c8ac-7b01-901f-2b1b8fa2e36f`) and James
  (`019f834f-c9fe-72d0-b940-4af81ae1496d`) report `CLEAN` after the final
  cycle, complexity, allocation, admission, and documentation corrections;
  both sessions are closed, and coder and Umans routes were not used.

  Feature commit `f9d3ef296a189734712b66ae9cb0140d0552c512` is pushed to
  `main`. Its first ordinary CI run `29808658860` exposed one tooling-only
  portability error because the admission test did not catch `PermissionError`
  while probing an unavailable `/root/test262`; test commit
  `d346c05ef05cc811b30d19c46029c2d3952be379` adds the same `OSError` handling
  used by neighboring admissions, and ordinary CI `29809211211` passes both
  jobs. The feature full matrix `29808658857` passes all **33/33** jobs; the
  redundant test-only full run was cancelled after the feature matrix stayed
  authoritative.

  The 30 result files at `/tmp/ruja-array-flat.29808658857` aggregate to
  **31262 pass / 5698 fail / 11502 skip / 5 timeout / 0 error / 48467 total /
  36960 pass-or-fail executed**. Against
  `/tmp/ruja-array-filter.29804173104.rerun`, 29 files are byte-for-byte
  identical. Only `test262_built-ins_result.txt` changes from
  **15627/4902/3134/5/0** to **15652/4887/3124/5/0**, exactly **+25 pass / -15
  fail / -10 skip** with no timeout, error, total, or unrelated-shard drift.
  The downloaded release binary independently reproduces direct flat and
  flatMap **43/43** on the fixed checkout.

- `Array.prototype.filter` now follows the generic, species-aware ECMAScript
  algorithm instead of filtering a represented-Array snapshot. It boxes
  primitive receivers, snapshots `LengthOfArrayLike`, validates the callback
  before `ArraySpeciesCreate(source, 0)`, and performs live `HasProperty`,
  `Get`, callback, and `CreateDataPropertyOrThrow` operations. Holes, inherited
  values, generic and Proxy receivers, callback mutation, custom species,
  dense selected indices, descriptor failures, and partial results now retain
  observable order.

  Receiver, arguments, boxed source, species result, and each present value
  remain rooted across getters, constructors, callbacks, Proxy traps,
  collection, and exact-cap allocation. Every logical source index consumes
  one fuel unit before property work, and all normal, semantic, callback,
  property-definition, allocation, and fuel exits restore the incoming pin
  depth.

  Exact admission freezes eight feature-gated files. On fixed Test262 checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the preceding policy and binary
  are **120 pass / 113 fail / 8 skip / 1 timeout**; applying the new policy to
  that binary is **121/120/0/1**; and the repaired runtime is **242/242**. The
  adjacent TypedArray filter directory remains **85/85**, and the broader
  diagnostic now reaches the independent generic `flat` gap.

  Local all-target/all-feature tests pass with **191/191** library tests,
  **522/522** builtins tests, **15/15** arguments tests, **190/190** release
  library tests, **123/123** Test262 tooling tests, **1/1** documentation tests,
  rustfmt, warnings-denied Clippy, and wasm32 all-features checking. Final
  GPT-5.6 runtime and admission/documentation reviews report `CLEAN`; both
  sessions are closed, and coder and Umans routes were not used. Final CI,
  matrix, and artifact evidence follows below.

  Feature commit `55cc204801028ab7be2b79f90c6fcb7b8638215f` is pushed to
  `main`. Ordinary CI `29804173132` passes both jobs, and full matrix
  `29804173104` passes all **33/33** jobs. The initial annexB artifact contained
  two unrelated transient timeouts; both the preceding and feature binaries
  reproduced annexB at **201/811/74/0/0** locally, and annexB job rerun
  `88556448007` restored that exact result.

  The 30 canonical result files at
  `/tmp/ruja-array-filter.29804173104.rerun` aggregate to **31237 pass / 5713
  fail / 11512 skip / 5 timeout / 0 error / 48467 total / 36950 pass-or-fail
  executed**. Against the preceding fill matrix, 29 files are byte-for-byte
  identical. Only `test262_built-ins_result.txt` changes from
  **15505/5015/3142/6/0** to **15627/4902/3134/5/0**, exactly **+122 pass /
  -113 fail / -8 skip / -1 timeout**, with no error, total, or unrelated-shard
  drift. The downloaded release binary independently reproduces direct Array
  filter **242/242** and TypedArray filter **85/85** on fixed checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`.

- `Array.prototype.fill` now follows the generic ECMAScript algorithm instead
  of rewriting represented-Array backing storage. It boxes primitive receivers,
  snapshots `LengthOfArrayLike` once, coerces start and end in order, retains
  safe-integer indices, and performs an ascending live strict `Set` for every
  selected property. Generic objects, Proxies, inherited setters, non-writable
  failures, sparse tails, arguments objects, and borrowed TypedArrays now retain
  observable order and partial mutation without consulting species.

  Receiver, arguments, boxed object, and fill value remain rooted across
  getters, coercions, setters, traps, collection, and primitive-box heap-cap
  retry. Every selected index consumes one fuel unit before property work, and
  normal, semantic, strict-Set, allocation, and fuel exits restore the incoming
  pin depth.

  Exact admission freezes six feature-gated files. On fixed Test262 checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the preceding policy and binary
  are **8 pass / 8 fail / 6 skip**; applying the new policy to that binary is
  **9/13/0**; and the repaired runtime is **22/22**. The adjacent TypedArray
  fill directory remains **52/52**. At this fill unit boundary the broader
  diagnostic reached the independent generic `filter` gap; the later filter
  entry above records its progression to `flat`.

  Local all-target/all-feature tests pass with **188/188** library tests,
  **521/521** builtins tests, **15/15** arguments tests, **187/187** release
  library tests, **122/122** Test262 tooling tests, **1/1** documentation tests,
  rustfmt, warnings-denied Clippy, and wasm32 all-features checking. Final
  GPT-5.6 runtime and admission/documentation reviews report `CLEAN`; both
  sessions are closed, and coder and Umans routes were not used. Final CI,
  matrix, and artifact evidence follows below.

  Feature commit `4bb72313b5cb211414333fd14087ac533935da93` is pushed to
  `main`. Ordinary CI `29759957749` passes both jobs, and full matrix
  `29759957873` passes all **33/33** jobs. The 30 result files downloaded to
  `/tmp/ruja-array-fill.29759957873` aggregate to **31115 pass / 5826 fail /
  11520 skip / 6 timeout / 0 error / 48467 total / 36941 pass-or-fail
  executed**. Against the preceding iterator matrix, 29 files are
  byte-for-byte identical. Only `test262_built-ins_result.txt` changes from
  **15491/5023/3148/6/0** to **15505/5015/3142/6/0**, exactly **+14 pass / -8
  fail / -6 skip**, with no timeout, error, total, or unrelated-shard drift.
  The downloaded release binary independently reproduces direct Array fill
  **22/22** and TypedArray fill **52/52** on the fixed checkout.

- `Array.prototype.entries`, `keys`, and `values` now create generic lazy Array
  iterators instead of represented-Array snapshots. Iterator creation performs
  `ToObject` once, while each `next` re-reads the live `LengthOfArrayLike`,
  advances its safe-integer index before indexed `Get` or result allocation,
  preserves inherited and Proxy access, and permanently releases its source on
  completion. Keys avoid indexed `Get`; entries and iterator result objects use
  the active method Realm. Array, Map, and Set iterator `next` methods enforce
  distinct internal brands.

  Mapped and unmapped arguments objects now receive the Realm's immutable
  original `%Array.prototype.values%` as an own `Symbol.iterator` even if user
  code replaced the observable Array prototype method. Iteration observes
  deletion, replacement, and non-callable own methods. Mutable iterator source
  and `u64` index slots are traced and rooted across getters, Proxy traps,
  current-Realm pair/result allocation, exact-cap collection, and abrupt exits.

  Exact admission freezes 47 feature-gated files and combines them with 18
  already executed direct files. On fixed Test262 checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the preceding binary and policy
  are **12 pass / 6 fail / 47 skip**; the repaired runtime under the old policy
  is **18/0/47**; and the final exact cohort is **65/65**. Applying the new
  policy to the preceding binary produces **39 pass / 26 fail**, proving 26
  runtime fail-to-pass transitions and no reverse transition. The shared
  TypedArray, Map, Set, and String iterator compatibility sweep is **94/94**.
  At this iterator unit boundary, the broader
  `methods-called-as-functions.js` diagnostic reached the independent generic
  `fill` gap and remained outside admission; the later fill entry above records
  its progression to `filter`.

  Final local gates pass with **186/186** all-feature library tests inside the
  complete all-target suite, **185/185** release library tests, **520/520**
  builtins tests, **15/15** arguments tests, **121/121** Test262 tooling tests,
  **1/1** documentation tests, rustfmt, Clippy with `-D warnings`, and wasm32
  all-features checking. GPT-5.6 reviewers Zeno
  (`019f7fe9-85ee-70a0-8018-4de31e315ec5`) and Avicenna
  (`019f7fe9-87a0-7970-ab6a-86592a461ac7`) drove brand, override,
  allocation, dead-cursor, and admission corrections before both returned
  `CLEAN`. Both sessions are closed; coder and Umans routes were not used.

  Feature commit `7ebc8eea42c11141db89498c4ea7c9dd4e54fbea` is pushed to
  `main`. Ordinary CI `29755632464` passes both jobs, and full matrix
  `29755632391` passes all **33/33** jobs. The 30 downloaded result files at
  `/tmp/ruja-array-iterators.29755632391` aggregate to **31101 pass / 5834
  fail / 11526 skip / 6 timeout / 0 error / 48467 total / 36935 pass-or-fail
  executed**. Against the preceding copyWithin matrix, 28 files are
  byte-for-byte identical. `test262_built-ins_result.txt` changes by exactly
  **+51 pass / -6 fail / -45 skip**, while
  `test262_language_arguments-object_result.txt` changes by **+2 pass / -2
  skip**; timeout, error, total, and every unrelated shard are unchanged. The
  downloaded release binary independently reproduces the exact **65/65** and
  shared iterator **94/94** cohorts on the fixed checkout.

- `Array.prototype.copyWithin` now follows the generic ECMAScript algorithm
  instead of copying represented Array backing storage. It boxes primitive
  receivers, snapshots `LengthOfArrayLike`, coerces target/start/end in order,
  selects the overlap direction, and performs live `HasProperty` plus
  `Get`/strict `Set` or `DeletePropertyOrThrow` operations. Inherited values,
  holes, generic and Proxy receivers, TypedArrays, same-range traps, partial
  mutation before abrupt completion, and lengths through `2^53 - 1` now retain
  observable semantics without materializing a source vector.

  Receiver, arguments, boxed object, and fetched values remain rooted across
  coercion, accessors, traps, setters, collection, and primitive-box heap-cap
  retry. Every logical iteration consumes one fuel unit before property work,
  and all normal, semantic, Proxy-false, allocation, and fuel exits restore the
  incoming pin depth. Primitive wrappers and errors retain the method Realm.

  Exact admission freezes eight feature-gated files with shared runner/analyzer
  metadata and future-sibling closure. On fixed Test262 checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the complete direct directory
  moves from **17 pass / 14 fail / 8 skip** to **39/39**. With the new policy
  applied to the preceding binary, runtime moves from **21 pass / 18 fail** to
  **39/0** with no reverse transition. Array and TypedArray copyWithin coverage
  is **104/104**, and Python tooling is **120/120**.

  Final local gates pass with **181/181** all-feature library tests inside the
  full all-target suite, **180/180** release library tests, **120/120** Test262
  tooling tests, **1/1** documentation tests, rustfmt, Clippy with `-D
  warnings`, and wasm32 all-features checking. Two independent GPT-5.6 reviews
  report `CLEAN`; coder and Umans routes were not used for this review.

  Feature commit `1678cc6b6789ee37f881d40460f9edb95da0fc90` is pushed to
  `main`. Ordinary CI `29747022947` passes both jobs, and full matrix
  `29747022998` passes all **33/33** jobs. The 30 downloaded result files at
  `/tmp/ruja-array-copy-within.29747022998` aggregate to **31048 pass / 5840
  fail / 11573 skip / 6 timeout / 0 error / 48467 total / 36888 pass-or-fail
  executed**. Against the preceding constructor-traversal matrix, 29 files are
  byte-for-byte identical. Only `test262_built-ins_result.txt` changes by
  exactly **+22 pass / -14 fail / -8 skip**, with no timeout, error, total, or
  unrelated-shard drift. The downloaded release binary independently
  reproduces direct **39/39** and combined Array/TypedArray **104/104**
  copyWithin coverage on the fixed checkout.

- Bound Functions and Proxies now retain immutable `[[Construct]]` capability,
  making `IsConstructor` constant-time and side-effect free. Constructor Realm
  lookup and actual Bound/Proxy construction consume one fuel unit per followed
  edge with revocation checked first. The shared dispatcher roots every
  wrapper, Proxy target/handler/trap, argument array, prototype, and fallback
  Realm across observable calls and collecting allocation.

  Bound arguments are collected outer-to-inner and flattened once in reverse
  wrapper order, preserving `innerArgs, outerArgs, callArgs` and per-wrapper
  `newTarget` substitution in linear time. The combined list shares the
  1,048,576-entry argument cap; direct constructor/newTarget validation occurs
  before any argument pin growth, and Bound overflow is rejected before an
  observable target Proxy `construct` lookup. Ordinary interpreted receiver
  allocation now uses the GC-retrying VM allocator. Eager native constructors
  retain either the observed prototype or one already-resolved fallback Realm,
  avoiding a second `GetFunctionRealm` traversal.

  Promise settlement precomputes every selected handler Realm before changing
  state. Intrinsic resolving functions retain Realm-rooted, phase-specific
  Resolve, Reject, or post-`then` work when Fuel aborts: completed reaction
  handlers, thenables, observable `then` access, and selected allocation-error
  rejection are not replayed. Direct settlement retains the resolver operation
  Realm for handler fallback, while nested resolving functions and
  allocation-error materialization use the callable `then` job Realm. Staged
  settlement runs before later external jobs, while arbitrary species-provided
  capability functions are never automatically replayed.

  Regressions cover exact fuel and revocation order, direct and Bound argument
  caps without transient pin growth, 4,096 ordered Bound layers, 20,000-deep
  constructor chains, forced GC and heap-cap allocation, fallback Realms,
  Promise one-shot ownership, phase transfer, queue-front/FIFO order, task-only
  GC roots, thenables that call resolve/reject, selected heap-limit rejection,
  and custom capability no-replay behavior. On fixed Test262 checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the 248-file constructor cohort
  remains **204 pass / 7 fail / 37 skip** under policy and **239 pass / 9 fail**
  with gates forced; the 734-file species cohort remains **663 pass / 71 skip**
  and **734/734** forced. File-by-file A/B against the preceding release binary
  has zero status transitions in all four runs.

  Final local gates pass with **179/179** all-feature library tests inside the
  full all-target suite, **178/178** release library tests, **119/119** test262
  tooling tests, **1/1** documentation tests, rustfmt, Clippy with `-D
  warnings`, and wasm32 all-features checking. Two independent GPT-5.6 reviews
  report `CLEAN`; coder and Umans routes were not used for this review.

  Feature commit `5122d2ab54fa72a7ba535c1b1bb13157a24f0aeb` is pushed to
  `main`. Ordinary CI `29741522154` passes both jobs, and full matrix
  `29741522114` passes all **33/33** jobs. The 30 downloaded result files at
  `/tmp/ruja-constructor-traversal.29741522114` aggregate to **31026 pass / 5854
  fail / 11581 skip / 6 timeout / 0 error / 48467 total / 36880 pass-or-fail
  executed**. Every result file is byte-for-byte identical to the preceding
  concat matrix at `/tmp/ruja-array-concat.29728440863.rerun`. The downloaded
  release binary independently reproduces both fixed-checkout cohorts and all
  four policy/forced counts above.

- `Array.prototype.concat` now follows the generic ECMAScript pipeline:
  `ToObject`, `ArraySpeciesCreate`, `IsConcatSpreadable`,
  `LengthOfArrayLike`, safe-integer validation, `HasProperty`/`Get`,
  `CreateDataPropertyOrThrow`, and the final strict length `Set`. Arrays,
  Proxy-wrapped Arrays, explicitly spreadable objects and TypedArrays,
  primitive receivers and arguments, inherited indices, holes, custom species
  results, and foreign intrinsic Array constructors now preserve the required
  observable order. Default results grow sparsely beyond
  `MAX_DENSE_ARRAY_LEN` instead of cloning or preallocating dense backing
  storage.

  Receiver, arguments, boxed receivers, result objects, and copied values stay
  rooted across species constructors, getters, Proxy traps, property creation,
  and final length writes. Outer items and source indices consume cooperative
  execution fuel, abrupt and fuel exits restore pin depth, and default result
  allocation retries after collection at an exact heap-object cap. Ordinary
  own `length`, `byteLength`, `byteOffset`, and `buffer` properties now shadow
  the temporary TypedArray, ArrayBuffer, and DataView direct-field compatibility
  paths, fixing the TypedArray length override exposed by generic concat.

  Exact Test262 admission freezes the nine feature-gated concat files with
  complete metadata, runner/analyzer symmetry, disjointness, and future-sibling
  closure checks. On fixed checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the direct directory moves from
  **16 pass / 44 fail / 9 skip** under the previous policy to **69 pass / 0
  fail / 0 skip**. With the new policy forced onto the clean pre-feature
  binary, runtime alone moves from **17 pass / 52 fail** to **69/0**, with no
  reverse transition. The affected Array and Proxy cohort is **320 pass / 0
  fail / 3 skip**.

  Local gates pass all targets and features with lib **172/172**, builtins
  **516/516**, operators **126/126**, fuel **29/29**, release lib **171/171**,
  warnings-denied Clippy, rustfmt/diff, wasm32 all-features, and Python tooling
  **119/119**. Two independent GPT 5.6 audits reproduced the baseline,
  verified the implementation and exact admission, and identified the separate
  shared constructor-chain fuel gap now recorded in the limitations. A third
  final-diff reviewer returned `CLEAN`; all three sessions are closed. Neither
  the coder model nor an Umans provider route was used. Feature commit
  `549693cf5942ed60816ad80402ab4dd0dcc97412` is pushed to `main`. Ordinary CI
  `29728440834` passes both jobs.

  Full matrix `29728440863` passes all **33/33** jobs after rerunning its
  Annex B shard. The initial artifact had two load-sensitive Annex B timeouts;
  the same downloaded release binary on the fixed 1,086-file corpus reproduced
  the baseline **201 pass / 811 fail / 74 skip / 0 timeout**, and the isolated
  CI rerun did likewise. The 30 final result files at
  `/tmp/ruja-array-concat.29728440863.rerun` aggregate to **31026 pass / 5854
  fail / 11581 skip / 6 timeout / 0 error / 48467 total / 36880 pass-or-fail
  executed** (**64.0%** all-file, **84.1%** executed).

  Compared with full-matrix baseline `29723329226`, 29 files are byte-for-byte
  identical. Only built-ins changes from **15365/5087/3210/6/0** to
  **15418/5043/3201/6/0**, exactly **+53 pass / -44 fail / -9 skip** with no
  timeout, error, total, or unrelated-shard drift. The downloaded release
  binary independently passes the exact concat directory **69/69**.

- `%Array.prototype%` is now a real Array exotic in every Realm, so indexed
  definitions update its length through the same invariants as ordinary
  Arrays. Realm-local Array constructors are registered, rooted, and rolled
  back transactionally for cross-Realm `ArraySpeciesCreate`. Default indexed
  data descriptors have one representation in dense storage; accessors,
  non-default descriptors, and sparse entries remain in the property table.

  `push`, `pop`, `shift`, `unshift`, `splice`, `slice`, and `with` now use
  generic ToObject, LengthOfArrayLike, Get, HasProperty, Set, Delete, and
  CreateDataProperty operations. Slice preserves holes and uses species,
  Splice uses species for deleted elements, and With ignores species while
  materializing holes. The Array constructor and Slice create sparse results
  above `MAX_DENSE_ARRAY_LEN`; With retains the documented 1,048,576-element
  sandbox cap because every result position must be materialized.

  Proxy `IsArray` traversal is iterative and fuel-metered with revocation
  checked first. Array length shrink precharges descriptor scans and dense
  resize work before mutation. Constructor, species, and method
  paths use GC-retrying allocation, preserve live values through observable
  re-entry, restore pin depth on every exit, and behave transactionally under
  exact heap caps and fuel exhaustion.

  Exact Test262 admission freezes **20** feature-gated paths with complete
  metadata and future-sibling closure checks. On checkout
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the complete seven-method
  Push/Pop/Shift/Unshift/Splice/Slice/With cohort plus six direct prototype
  checks is **268/268**. The clean pre-feature binary is **129 pass / 119 fail
  / 20 skip**, for exactly **+139 pass / -119 fail / -20 skip**. At that
  intermediate boundary, the unrelated `concat` assertion kept the broader
  methods-called-as-functions aggregate outside admission; the concat unit
  above clears that assertion, while `copyWithin` remains independent work.

  Local gates pass all targets/features with lib **168/168**, builtins
  **511/511**, operators **126/126**, fuel **29/29**, release lib **167/167**,
  warnings-denied Clippy, rustfmt/diff, wasm32, and Python tooling **118/118**.
  Four mutations independently prove intrinsic exotic allocation, indexed
  representation ownership, foreign intrinsic species handling, and exact
  Proxy fuel. Two GPT 5.6 final reviewers returned `CLEAN`; all four GPT review
  sessions are closed, and neither the coder model nor an Umans provider route
  was used. Feature commit `48f33da967ce53565a374803a4c18372d61a84b1`
  is pushed, and ordinary CI `29723329186` passes both jobs.

  Full matrix `29723329226` passes all **33/33** jobs. Its 30 downloaded
  results at `/tmp/ruja-array-exotic.29723329226.complete` aggregate to
  **30973 pass / 5898 fail / 11590 skip / 6 timeout / 0 error / 48467 total /
  36871 pass-or-fail executed** (**63.9%** all-file, **84.0%** executed).
  Compared with full-matrix baseline `29718464780`, 29 files are
  byte-identical; only built-ins changes from **15216/5216/3230/6/0** to
  **15365/5087/3210/6/0**, exactly **+149 pass / -129 fail / -20 skip** with
  no timeout, error, total, or unrelated-shard drift.

  A same-runner, fixed-corpus binary A/B covers every built-ins file that is
  executed under the new admission policy. Runtime behavior has **141
  fail-to-pass** transitions and no reverse transition: **132** under Array,
  **8** under Iterator zip/zipKeyed, and **1** TypedArray sort-stability test.
  On the baseline binary, the 20 newly admitted paths are **8 pass / 12 fail**;
  all 20 pass after the runtime fix. Composing that admission movement with
  the 141 runtime transitions exactly reproduces the full artifact delta. The
  non-Array diagnostic sweep caps exceptional per-file timeouts at eight
  seconds; the official artifacts independently preserve all six timeouts.

- `for...in` now uses a lazy, GC-traced iterator state that invokes
  `[[OwnPropertyKeys]]`, `[[GetOwnProperty]]`, and `[[GetPrototypeOf]]` at the
  observable phase of each advancement. Proxy trap order, early `break`,
  Symbols, deleted keys, non-enumerable shadowing, absent descriptors,
  revocation, abrupt values, primitive String UTF-16 indices, nullish sources,
  and Map object-property boundaries are preserved.

  Proxy `[[OwnPropertyKeys]]` is iterative rather than Rust-recursive. It roots
  every target and handler, meters every layer, validates duplicates, performs
  target extensibility before target keys, obtains all target descriptors
  before omission errors, and enforces non-extensible exact-set invariants.
  Ordinary snapshots precharge every native key source before materializing
  collections. Candidate and prototype traversal are also fuel-bounded, and
  inert Proxy cycles use the existing finite replay guard. `Object.hasOwn` and
  `Object.prototype.hasOwnProperty` now use complete Proxy descriptors.

  Regressions cover lazy trap/body order, transparent and fabricated keys,
  Symbol filtering, shadowing, deleted descriptors, revocation, abrupt
  identity, primitive boxing, collection boundaries, deep Proxy targets,
  exact fuel and heap caps, forced GC and cell reuse, pin cleanup, and ordinary
  or Proxy prototype cycles. Four mutation probes independently prove snapshot
  precharge, extensibility ordering, iterator GC tracing, and complete target
  descriptor traversal. Two GPT 5.6 reviewers returned `CLEAN`; the coder model
  and Umans provider were not used.

  Exact Test262 admission freezes **22** paths: all **21** direct Proxy
  getOwnPropertyDescriptor files and one removed-enumerate file. On fixed
  checkout `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, those paths are
  **22/22**, `language/statements/for-in` is **78/0/37/115**, and supported
  expressions/statements are **12752/0/7687/20439**. Local all-target and
  all-feature tests pass with lib **162/162**, builtins **507/507**, and fuel
  **29/29**, together with release lib **161/161**, warnings-denied Clippy,
  rustfmt/diff, release, wasm32, and Python tooling **117/117**. Feature commit
  `e98a31a1d0b5c93f4a34c37b7e37abb61dd1ebcd` is pushed, and ordinary CI
  `29718464784` passes both jobs.

  Full matrix `29718464780` passes all **33/33** jobs. Its 30 downloaded
  artifacts at `/tmp/ruja-proxy-for-in.29718464780.complete.faEQF1` aggregate
  to **30824 pass / 6027 fail / 11610 skip / 6 timeout / 0 error / 48467 total
  / 36851 pass-or-fail executed** (**63.6%** all-file, **83.6%** executed).
  Compared with the preceding confirmed matrix, 29 files are byte-identical;
  only built-ins changes from **15194/5216/3252/6/0** to
  **15216/5216/3230/6/0**, exactly **+22 pass / -22 skip** with no failure,
  timeout, error, corpus, total, or unrelated-shard drift.

- Ordinary `[[Get]]`, `[[HasProperty]]`, and `[[Set]]` now traverse prototype
  chains iteratively without the former 4096/1024/1024 correctness limits.
  One rooted `PropertyTraversal` records directed edges, retains every reached
  object across observable GC, preserves the original Get receiver, and
  charges fuel per ordinary-to-ordinary edge while each Proxy layer retains
  its existing charge. Inherited Proxy trap lookup is no longer truncated.

  Proxy-induced cycles replay repeated edges so first- or later-pass trap
  getters can mutate the target before transparent forwarding. Inert cycles
  end with a catchable RangeError after 512 replays instead of overflowing the
  native stack, and configured fuel can stop them earlier. Revocation precedes
  fuel accounting. Exact legacy Proxy budgets retain one initial ordinary-edge
  credit for GetMethod lookup and transparently forwarded Set.

  Value-key Get/Set now perform one Symbol-preserving `ToPropertyKey`, including
  Symbols returned by `@@toPrimitive`; Symbol Set shares the receiver-aware
  internal method. Module Namespace Set returns false for every key and custom
  receiver. Every Realm now installs the required own
  `Array.prototype.length` descriptor `{value: 0, writable: true, enumerable:
  false, configurable: false}`.

  Regressions cover 5000 ordinary edges; deep String and Symbol Get/Has/Set;
  inherited get/has/set/defineProperty traps; original receivers; forced GC
  after severed prototype edges; WeakRef liveness; pin cleanup; exact ordinary
  and inherited-handler fuel; revoked zero-fuel ordering; ordinary and Proxy
  cycles; first- and second-pass trap mutation; Module Namespace custom
  receivers; created-Realm Array descriptors; and object keys coercing to
  Symbols. Three manual mutations prove repeated-target rechecks, persistent
  traversal roots, and revoked-before-fuel ordering are required.

  `tools/test262_proxy_has_admission.txt` freezes all **26/26** direct Proxy
  Has files with checksum
  `7adb36802ee9e5db473422707e7e7da98b16fc52420d02c666741e4c5b2de6ef`.
  Tooling verifies exact live metadata, includes, flags, no negative entries,
  disjointness, runner/analyzer symmetry, and closed future/extra-feature
  gates. Proxy get/has/set/defineProperty plus Reflect get/has/set is
  **135/135**, Module Namespace internals are **36/36**, and supported
  expressions/statements remain **12751/0/7687/20438**.

  Feature commit `d6e8e61d788fecf81772d3828e1ac47f5c138abf` passes local
  all-target/all-feature tests with lib **155/155**, builtins **504/504**,
  operators **126/126**, fuel **28/28**, warnings-denied Clippy, rustfmt/diff,
  debug and release builds, wasm32, the release Realm rollback sweep, and
  Python tooling **116/116**. Independent GPT runtime and admission reviews
  found no remaining blocker; the coder model and Umans provider were not
  used. Ordinary CI `29702222060` passes both jobs.

  The first full-matrix artifact audit was not accepted on aggregate totals
  alone. A fixed-corpus binary A/B found that corrected own-`undefined`
  lookup exposed two old dense-copy bugs: `Array.prototype.slice` converted a
  source hole into an own `undefined` instead of copying an inherited value,
  and `Array.prototype.with` skipped inherited values while materializing
  holes. Follow-up commit
  `70973c4f78d4076db24dfbbf877c308a27602fdc` now makes Slice use
  HasProperty followed by Get and makes With Get every non-replaced index.
  Fresh results use the current Realm's intrinsic Array prototype, and source,
  replacement, and result objects remain rooted across observable lookup and
  every abrupt exit.

  Focused Test262 A/B moves Array slice from **41/22/8** to **42/21/8** and
  Array with from **13/7/1** to **15/5/1**, with no pass-to-fail transition.
  The full Array comparison has four fail-to-pass transitions and no
  regression; Object descriptor tests have 18 fail-to-pass transitions from
  the corrected own-accessor-without-getter semantics. Forced-GC, abrupt pin
  cleanup, inherited-value, and preserved-hole checks cover the follow-up.

  Ordinary CI first exposed an unrelated timing flake in the existing
  `Atomics.waitAsync` notification test: deliberate allocation pressure could
  exceed its one-second notification deadline on a loaded debug runner. The
  engine path passed **20/20** local repetitions. Test-only commit
  `a3f0f3711582a17e9b329a5932427c0a88ca0438` raises that notification
  deadline to ten seconds while retaining the independent ten-millisecond
  timeout assertion. Ordinary CI `29704449961` passes both jobs.

  Its green full matrix `29704449969` was still diagnostic rather than
  accepted: 29 result files matched the prior baseline, but built-ins fell to
  **14827/5583/3252/6**. Test262's shared TypedArray constructor harness calls
  `slice()` and then `splice()` on the copy; general ArrayCreate had installed
  a fixed own length descriptor that legacy dense mutators did not update.
  The visible descriptor therefore stayed stale even though the backing store
  changed.

  Final follow-up commit
  `29d0f275ee30a4c18fcdb790f15c5b6b951134bc` allocates successful Slice and
  With copies through a current-Realm hole-only path whose length remains
  derived from dense backing storage. It rejects copy results above
  `MAX_DENSE_ARRAY_LEN` before native vector allocation because legacy sparse
  mutators do not yet honor `sparse_max`; bounded slices from a pre-existing
  sparse source still work. A same-corpus scan of all **1,995** supported files
  that include `testTypedArray.js` has no status transition. Six copy/rooting/
  mutator mutations and one cap-boundary mutation each fail focused coverage
  and pass after restoration. Two GPT 5.6 final reviews report `CLEAN` after
  finding and closing the sparse-result blocker.

  Final local gates pass lib **158/158**, builtins **505/505**, all other Rust
  targets/features, warnings-denied Clippy, rustfmt/diff, release, wasm32, the
  release Realm rollback sweep, and Python tooling **116/116**. On fixed
  Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`, the complete built-ins
  diagnostic is **15194 pass / 5216 fail / 3252 skip / 6 timeout / 0 error**
  with **23668 total / 20410 run**. Ordinary CI `29708233592` passes both
  jobs for the final feature commit.

  Final matrix `29708233596` succeeds across all **33/33** jobs. Its 30
  downloaded result files at
  `/tmp/ruja-property-final.29708233596.complete.0MkZJg` aggregate to **30802
  pass / 6027 fail / 11632 skip / 6 timeout / 0 error / 48467 total / 36829
  pass-or-fail executed** (**63.6%** all-file, **83.6%** executed). Against
  `/tmp/ruja-property-baseline.yB3i4f`, 29 files are byte-identical. Only
  `test262_built-ins_result.txt` changes from **15146/5238/3278** to
  **15194/5216/3252**, exactly **+48 pass / -22 fail / -26 skip** with no
  timeout, error, corpus, total, or unrelated-shard drift.

- Proxy `[[Set]]` and receiver-side `[[DefineOwnProperty]]` forwarding now use
  iterative, fuel-metered, GC-rooted state machines instead of recursive
  helpers with a 128-layer limit. Ordinary prototype traversal hands a Proxy
  continuation back to the shared Set driver; missing traps tail-forward,
  false traps short-circuit target descriptor lookup, and truthy traps retain
  target invariants, revocation, callable validation, cycle detection, and
  abrupt-completion order.

  Receiver creation preserves the complete CreateDataProperty descriptor,
  while an existing writable receiver property delegates only `{value}`.
  Presence-aware descriptor objects allocate in the current execution Realm,
  and reaching an ordinary target preserves TypedArray, Array,
  mapped-arguments, namespace, and extensibility behavior. Regressions cover
  100,000 Proxy layers, exact 3N/2N and nested fuel, forced GC, unique abrupt
  markers, revoked inner targets, false-result suppression, descriptor
  mutation, exact heap caps, Realm identity, cycles, and pin cleanup. Mutation
  checks prove the Set fuel charge, operation-wide value root, and partial
  descriptor presence are necessary.

  The Proxy defineProperty manifest now admits the complete **24/24** direct
  directory with checksum
  `a002341a5009bf858ed9c0ca44bfdb3d15e3fcb36fabe50cc12e1b298e671db5`.
  A separate Proxy Set manifest admits all **27/27** direct files with checksum
  `544af6a5bdcc955a21df6775bade2f624694931c46831d07485cfe4207938396`.
  Direct Proxy define/set plus Reflect.set is **69/69**; the combined
  define-property cohort is **1773/13/13/1799**, Proxy get/isExtensible remains
  **31/31**, and the supported subset remains **12751/0/7687/20438**.

  Feature commit `cd6a65313103bcaf8de900df6046f2a8f600f5ff` passes all local
  Rust targets/features with lib **150/150**, builtins **503/503**,
  warnings-denied Clippy, rustfmt/diff, debug and release builds, wasm32, the
  release Realm rollback sweep, and Python tooling **115/115**. Mutation-backed
  GPT review found no code or admission defect; the stale documentation it
  identified is corrected in the same bounded unit. The former ordinary Set
  and handler GetMethod guards are removed by the coordinated traversal entry
  above. The coder model and Umans provider were not used.

  Ordinary CI `29698474332` passes both jobs, and full matrix `29698474309`
  passes all **33/33** jobs. The 30 downloaded result files at
  `/tmp/ruja-proxy-set-results.29698474309.MKYKPL` aggregate to **30754 pass /
  6049 fail / 11658 skip / 6 timeout / 0 error / 48467 total / 36803
  pass-or-fail executed**, or **63.5%** of all files and **83.6%** of executed
  files. Against the preceding defineProperty baseline, 29 files are
  byte-identical. Only `built-ins` changes from **15116/5238/3308** to
  **15146/5238/3278**, exactly **+30 pass / -30 skip** with no failure,
  timeout, error, corpus, or total drift.

  GPT 5.6 reviewers Linnaeus (`019f7b70-4012-70d1-9eef-c5d82af25b09`) and
  Laplace (`019f7b70-6471-7713-8c97-0b2a4ec61743`) audited the implementation
  boundary and exact admissions before finalization. Meitner
  (`019f7b8d-5fea-7860-ac5b-69eb09208b2d`) found no code issue at any severity,
  and Parfit (`019f7b8d-614b-7a23-a956-4e00becef175`) found no admission or
  tooling issue; its sole stale-documentation finding is resolved here. All
  sessions are closed with no duplicate agent left running.

- Proxy `[[DefineOwnProperty]]` now uses one iterative, fuel-metered,
  GC-rooted state machine for internal complete descriptors and public
  `Object.defineProperty`, `Object.defineProperties`, and
  `Reflect.defineProperty` calls. Partial descriptor presence is retained;
  revocation, `GetMethod`, trap invocation, false results, target descriptor,
  extensibility, compatibility, configurable, and writable-tightening checks
  remain in specification order. Missing traps no longer recurse through Rust,
  and non-callable traps fail before descriptor-object allocation.

  The callable Proxy traps required by that path are stack-safe as well.
  Proxy creation stores immutable callability and constructability metadata,
  and `[[Call]]` tail-transforms transparent targets and Proxy-valued `apply`
  traps while charging one fuel unit per layer. Argument arrays use the current
  execution Realm, observable values remain rooted by the outer call cleanup
  scope, and every normal, thrown, allocation, or host-fuel return restores the
  incoming pin depth.

  Regressions cover 100,000 transparent define-property layers, 64 nested
  invariant layers, 25,000 transparent callable traps, 4,096 nested Proxy
  `apply` traps, exact N-1/N fuel boundaries, forced collection, unique thrown
  marker objects, method-Realm errors, exact heap caps, and cleanup after every
  exit. Mutation checks prove the DefineOwnProperty and Proxy Call fuel charges,
  descriptor-value root, non-callable GetMethod ordering, and current-Realm
  argument-array allocation are necessary.

  At feature commit `96ea1384519e5f1ef2c1bc4f7abd360976a5c0bd`,
  `tools/test262_proxy_define_property_admission.txt` froze exactly 21 of the
  24 direct `built-ins/Proxy/defineProperty` files with manifest checksum
  `ccccef0672a93f9c70ce5ee42cfe11b7d1401e18b916776031e484b1f803cdfa`.
  The three assignment-driven files were then excluded because they entered
  the separate 128-layer receiver-definition path. At that commit the direct
  directory was **21 pass / 0 fail / 3 skip / 24 total**; the combined
  Object/Reflect/Proxy
  define-property cohort is **1770/13/16/1799**, the Proxy-call cohort is
  **58/0/13/71**, and the construct cohort is **11/0/28/39**. The supported
  subset remains **12751/0/7687/20438**.

  Feature commit `96ea1384519e5f1ef2c1bc4f7abd360976a5c0bd` passes all local
  Rust targets/features with lib **147/147**, builtins **501/501**,
  warnings-denied Clippy, rustfmt/diff, debug and release builds, wasm32, and
  Python tooling **114/114**.

  Ordinary CI `29695748045` and all **33/33** jobs in full matrix
  `29695748050` pass. The 30 result files at
  `/tmp/ruja-define-property-results.29695748050.FS0S7x` aggregate to
  **30724 pass / 6049 fail / 11688 skip / 6 timeout / 0 error / 48467 total /
  36773 pass-or-fail executed**. Twenty-nine result files are byte-identical
  to prototype-internal baseline `29691533326`; only `built-ins` changes from
  **15095/5238/3329** to **15116/5238/3308**, exactly **+21 pass / -21 skip**.

  GPT reviewers Boyle (`019f7afc-66a3-7612-8c5d-f1804ea89e45`) and Huygens
  (`019f7afc-67e0-7431-a5ec-56dfb7a57a61`) found the callable-Proxy recursion,
  non-callable allocation-order boundary, separate receiver path, and exact
  21-file corpus. Kepler (`019f7b22-ed89-7870-99a2-1aa61088da66`) found the
  foreign-Realm argument-array bug and confirmed its mutation-backed fix.
  Euclid (`019f7b22-ef54-7212-bc02-48dd5c6fa0b9`) found the anonymous-object
  abrupt-test false positive; unique marker assertions replaced it. All four
  sessions are closed, and neither the coder model nor an Umans provider route
  was used.

- Proxy `[[GetPrototypeOf]]` and `[[SetPrototypeOf]]` forwarding now walks
  transparently through arbitrary legal depth without Rust recursion. Each
  Proxy layer is rooted and consumes one fuel unit while preserving
  revocation, trap lookup and call order, false results, abrupt completions,
  nested invariants, and method-Realm errors. Deferred non-extensible
  `getPrototypeOf` expectations are pinned and validated from the innermost
  Proxy outward, while the proposed prototype remains pinned throughout
  `setPrototypeOf`.

  Ordinary `[[SetPrototypeOf]]` cycle detection no longer accepts cycles past
  a 4096-link cutoff. It scans raw ordinary prototype slots with one fuel unit
  per candidate and uses constant-memory Brent checkpoints, stopping at a
  non-ordinary `[[GetPrototypeOf]]` method as required. Regressions cover
  100,000 transparent Proxy layers, exact fuel boundaries, a 5000-link cycle,
  nested trapped invariants, forced GC, abrupt completion, Realm-sensitive
  public methods, and pin restoration. A reviewer-identified WeakRef
  same-job-retention false positive was removed; mutation testing then proved
  the deferred expected-prototype root is necessary.

  `tools/test262_prototype_internal_admission.txt` freezes exactly 40 files,
  split **4/19/17** across `Object/setPrototypeOf`, `Proxy/getPrototypeOf`, and
  `Proxy/setPrototypeOf`. The three exact directories are **48/48**, adding
  direct Reflect get/set gives **72/72**, and the six-directory prototype
  probe is **110 pass / 0 fail / 1 skip / 111 total**. The supported subset
  remains **12751/0/7687/20438**. Local gates pass all Rust targets/features
  with lib **143/143**, builtins **500/500**, warnings-denied Clippy,
  rustfmt/diff, release, wasm32, and Python tooling **113/113**.

  Feature commit `c226b655c21ad140e7bd1e941333d6eca64b1fce` passed ordinary
  CI `29691533311` and all **33/33** jobs in full matrix `29691533326`. The
  30 result files at `/tmp/ruja-prototype-results.29691533326.0v6mCe`
  aggregate to **30703 pass / 6049 fail / 11709 skip / 6 timeout / 0 error /
  48467 total / 36752 pass-or-fail executed**. Twenty-nine result files are
  byte-identical to the extensibility baseline; only `built-ins` changes from
  **15055/5238/3369** to **15095/5238/3329**, exactly **+40 pass / -40 skip**
  with no failure, timeout, error, corpus, or total drift.

  GPT reviewers Pauli (`019f7aa5-8618-7130-a54e-7ac58c47e431`) audited the
  exact corpus, Erdos (`019f7ac0-f6f1-70f2-b853-e5a33fd749de`) returned
  `CLEAN`, and Newton (`019f7ac0-f84b-79d1-9fcf-ddf6db33321c`) found the
  WeakRef test flaw and confirmed its correction. Gibbs
  (`019f7aa5-850a-7761-a747-f4148810accc`) was stopped without a usable
  result. All sessions are closed; no coder or Umans route was used.

- `Object.preventExtensions` and `Reflect.preventExtensions` now persist state
  for every observable exotic object, including collections and their
  iterators, RegExp String iterators, weak collections/references,
  FinalizationRegistry, Promises, sync and async generators, TypedArrays,
  ordinary and shared ArrayBuffers, and DataViews. New string or Symbol
  properties and prototype replacement are rejected afterward while existing
  configurable properties retain their specified write/delete behavior.

  Proxy `[[PreventExtensions]]` forwarding now walks transparently through
  arbitrary legal depth without Rust recursion. Every Proxy layer is rooted
  and charged one fuel unit; trap lookup/call order, revocation, false results,
  and abrupt completions are preserved. Truthy traps validate the target with
  its complete nested `[[IsExtensible]]` internal method rather than raw heap
  storage. Forced-GC and 100,000-layer exact-fuel regressions verify stack
  safety, pin restoration, and VM reuse.

  The audit also fixed integrity-level behavior exposed by the new state.
  Non-specialized exotics now process real own descriptors for `seal`,
  `freeze`, `isSealed`, and `isFrozen`; temporary descriptors use GC-retrying
  allocation while the operation target remains pinned. Exact-cap tests reuse
  the two required temporary cells across multiple properties and verify the
  saturated failure boundary. Map entries are no longer misreported as object
  own keys, and non-empty Maps seal without losing collection data. Non-empty
  TypedArrays correctly reject sealing and freezing.

  Exact Test262 admission adds 29 files, split **1/4/12/12** across
  `Object/isExtensible`, `Object/preventExtensions`, `Proxy/isExtensible`, and
  `Proxy/preventExtensions`. The combined six-directory boundary is
  **120/120**, direct Reflect remains **153/153**, and the supported subset
  remains **12751/0/7687/20438**. Local gates pass all Rust targets/features
  with lib **140/140**, builtins **499/499**, warnings-denied Clippy,
  rustfmt/diff, release, wasm32, and Python tooling **112/112**.

  Feature commit `57961ef` passed ordinary CI `29688399116` and all **33/33**
  jobs in full matrix `29688399107`. The 30 result files at
  `/tmp/ruja-extensibility-results.29688399107.lqESmN` aggregate to **30663
  pass / 6049 fail / 11749 skip / 6 timeout / 0 error / 48467 total / 36712
  pass-or-fail executed**. Twenty-nine artifacts are byte-identical to the
  prior baseline; only `built-ins` changes by exactly **+29 pass / -29 skip**.

  GPT 5.6 explorers Singer (`019f7a2e-85cb-7970-84c4-109b9c29a4c4`) and
  Faraday (`019f7a2e-87b4-7c32-94e6-ab61189934f7`) audited variant coverage,
  Proxy order, and the exact corpus. Final reviewers Descartes
  (`019f7a4d-1ac7-7022-8026-cd6a4b88b72e`) and Beauvoir
  (`019f7a4d-1cae-7251-9176-a57f7fe05fbd`) found the integrity shortcut, Map
  own-key leak, and raw allocation path; all were corrected before landing.
  All sessions are closed, and no coder or Umans route was used.

- Every created Realm's original `%Object.prototype%` now implements the
  Immutable Prototype Exotic Object `[[SetPrototypeOf]]` behavior instead of
  accepting a replacement prototype. Requests for the existing `null`
  prototype still succeed and the object remains extensible. Different
  prototypes return `false` through `Reflect.setPrototypeOf`; borrowed
  `Object.setPrototypeOf` and `__proto__` setters throw a TypeError from their
  own method Realm. Transparent Proxy delegation, truthy traps over extensible
  targets, and non-extensible target invariants retain their specified order.

  The rooted environment-to-prototype registry now owns a non-rooting reverse
  identity `HashSet`, keeping ordinary prototype mutation at expected O(1)
  cost instead of scanning every retained Realm. Registration and failed-Realm
  rollback update both collections together. A GPT review found that the
  first rollback used `debug_assert!(set.remove(...))`, which erased the
  removal in release builds and could leave a reusable heap index marked
  immutable. Removal is now unconditional, and ordinary CI runs the complete
  heap-boundary rollback sweep in release mode.

  Pinned Test262 has no created-Realm Object-prototype mutation case, so this
  correctness fix intentionally changes no admission manifest. The related
  cohort is **37 pass / 0 fail / 21 skip / 58 total**, direct Reflect remains
  **153/153**, and the supported subset remains **12751/0/7687/20438**. Local
  gates pass all Rust targets/features, lib tests **137/137**, builtins
  **496/496**, warnings-denied Clippy, rustfmt/diff, release, wasm32, Python
  tooling **111/111**, and the release rollback sweep. Feature commit
  `9d38dc1` passed ordinary CI `29684489555`, including the new release gate.

  Full matrix `29684489558` passes all **33/33** jobs. Its 30 result files at
  `/tmp/ruja-object-proto-results.29684489558.8OEqey` aggregate to **30634
  pass / 6049 fail / 11778 skip / 6 timeout / 0 error / 48467 total / 36683
  pass-or-fail executed**. All 30 files are byte-identical to the prior
  Reflect-complete baseline, confirming the expected zero Test262 delta.

  GPT 5.6 reviewers Ramanujan (`019f79ec-7765-7640-8a4e-6cde989edf28`),
  Halley (`019f79ec-7891-7822-9a56-198aafe967e4`), Fermat
  (`019f79f9-d555-76d1-a954-3b123be85ed9`), and Pascal
  (`019f79f9-d750-7b61-b9ca-1fbb4255cd19`) audited semantics, Test262,
  performance, Realm rollback, and release behavior. Their O(Realms) and
  release-only stale-index findings were corrected before landing. All four
  sessions are closed; no coder or Umans route was used.

- Every main and Test262-created Realm now owns a distinct `Reflect` namespace
  object and 13 distinct native methods. The namespace inherits from that
  Realm's `%Object.prototype%`, its methods inherit from the matching
  `%Function.prototype%`, generated errors use the method Realm, and the
  global binding retains the standard writable, non-enumerable, configurable
  descriptor. `Reflect[Symbol.toStringTag]` is now the non-writable,
  non-enumerable, configurable string `"Reflect"`, so the observable brand is
  `[object Reflect]` without hard-coding an internal tag fallback.

  Runtime construction uses a narrowly scoped GC-retrying native-function
  allocator. Each provisional method is pinned before the next allocation and
  the namespace object is allocated while the complete method set remains
  rooted; success and exact-cap failure both restore the incoming pin depth.
  Deterministic regressions force collection before the first method, midway
  through the method batch, and at the final object allocation, then verify
  all method identities and names. A methods-only hard cap also proves complete
  cleanup on failure. This closes a review-found bug where Realm construction
  could reject despite reclaimable garbage.

  `tools/test262_reflect_remaining_admission.txt` freezes exactly the 71
  residual direct Reflect files after the existing 82-file admissions. Live
  `features`, `includes`, `flags`, and `negative` metadata are checked, overlap
  is rejected against every other admission manifest, and future paths or
  added unsupported features remain skipped. Pinned Test262
  `020cb74075849d1e404bbcdb62feb7a02e6966db` now runs the complete direct
  `built-ins/Reflect` directory at **153 pass / 0 fail / 0 skip**; the new
  residual set is **71/71** and the supported subset remains
  **12751/0/7687/20438**. Local gates pass all Rust targets/features, lib tests
  **137/137**, builtins **495/495**, warnings-denied Clippy, rustfmt/diff,
  release, wasm32, and Python tooling **111/111**.

  Feature commit `09306d8` passed ordinary CI `29682312654` and all **33/33**
  jobs in full matrix `29682312645`. The 30 result artifacts at
  `/tmp/ruja-reflect-complete-results.29682312645.qE94cM` aggregate to
  **30634 pass / 6049 fail / 11778 skip / 6 timeout / 0 error / 48467 total /
  36683 pass-or-fail executed** (**63.2%** of all files, **83.5%** of executed
  files). Against the Reflect-key baseline, 29 artifacts are byte-identical;
  only `built-ins` changes from **14955/5238/3469** to
  **15026/5238/3398**, exactly **+71 pass / -71 skip**.

  GPT 5.6 reviewers Turing (`019f79b3-ef56-7ac2-8877-ffda879e8353`) and Bohr
  (`019f79b3-f1a7-7f61-8c0b-2f3b5f745b2c`) returned `CLEAN` after the
  heap-cap and metadata gaps were corrected. Explorers Rawls
  (`019f79a0-661f-7b61-a838-bb3b10288063`) and Hubble
  (`019f79a0-6736-7052-b3f7-1d971ac17e2f`) verified the Realm/admission
  boundary and identified separate prototype, extensibility, and fuel defects
  now recorded as limitations. All sessions are closed; no coder or Umans
  route was used.

- `Reflect.get`, `Reflect.set`, and `Reflect.has` now apply
  `ToPropertyKey(undefined)` when the property-key argument is omitted. They
  no longer return `undefined` or `false` before reading a property named
  `"undefined"`, creating that property with an `undefined` value, or invoking
  a Proxy trap. Target validation still precedes key conversion;
  `Reflect.get` and `Reflect.set` retain their target receiver defaults, and
  `Reflect.set` retains its `undefined` value default. A shared conversion
  helper also keeps `Reflect.deleteProperty` on the same path.

  Ordinary, explicit-`undefined`, accessor, Proxy, revoked-Proxy, abrupt-trap,
  target-before-key, receiver-distinction, and forced-GC regressions cover the
  correction. Temporary Proxy arguments remain rooted while traps collect or
  throw, and every completion restores the incoming pin depth. Pinned
  Test262 has no test that distinguishes an omitted key from an explicit
  `undefined` key for these methods, so these deterministic local regressions
  are the behavioral proof rather than the admission delta.

  `tools/test262_reflect_set_has_admission.txt` freezes all 18 direct
  `Reflect.set` and all 10 direct `Reflect.has` files with exact live metadata.
  The new set/has admission is **28 pass / 0 fail / 0 skip**; together with the
  existing `Reflect.get` admission the focused result is **39/0/0**. Future
  paths and unrelated feature gates remain skipped. Supported Test262 remains
  **12751 pass / 0 fail / 7687 skip / 20438 total** on
  `020cb74075849d1e404bbcdb62feb7a02e6966db`. Final local gates pass all Rust
  targets/features, lib tests **136/136**, builtins **494/494**, operators
  **123/123**, bugfixes **68/68**, Fuel **28/28**, warnings-denied Clippy,
  rustfmt/diff, release, wasm32, and Python tooling **110/110**.

  GPT 5.6 reviewers Hypatia (`019f7972-ac1d-77f2-a2d0-aa1137c03189`) and
  Banach (`019f7972-addb-7950-b8f9-926eb1639cd8`) returned `CLEAN`; explorers
  Harvey (`019f7960-b499-7b90-a40d-ce5918fcf44d`) and Socrates
  (`019f7960-b5a8-7d70-bb7e-9ddbffeb6e5a`) independently found the third
  `Reflect.get` bug, the lack of an upstream omission discriminator, and the
  remaining deep property-traversal caps. All sessions are closed, and no
  coder or Umans route was used.

  Feature commit `04ce30c` passed ordinary CI `29679935417` after rerunning
  one unrelated Atomics timing flake, and all **33/33** jobs in full matrix
  `29679935409`. The 30 result artifacts at
  `/tmp/ruja-reflect-keys-results.29679935409.2y3wRm` aggregate to **30563 pass
  / 6049 fail / 11849 skip / 6 timeout / 0 error / 48467 total / 36612
  pass-or-fail executed** (**63.1%** of all files, **83.5%** of executed
  files). Twenty-nine files are byte-identical to Proxy-delete baseline
  `29677977505`; only `built-ins` changes from **14927/5238/3497** to
  **14955/5238/3469**, exactly **+28 pass / -28 skip** without failure,
  timeout, error, corpus, or total drift.

- Proxy `[[Delete]]` now forwards through trapless targets with an iterative
  worklist instead of recursive Rust calls. The original receiver remains a GC
  root for the complete operation; each target, handler, and fresh trap owns a
  bounded LIFO pin scope through observable lookup, invocation, descriptor
  validation, and extensibility validation. Missing/null traps, falsy results,
  Symbol keys, nested revocation, strict deletion, and the non-configurable and
  non-extensible invariants retain their specified order at arbitrary finite
  depth. A 100,000-layer regression completes without a host-stack dependency.

  Every traversed Proxy layer now consumes cooperative execution fuel not only
  in `[[Delete]]`, but also in nested handler `[[Get]]`, target
  `[[GetOwnProperty]]`, and `[[IsExtensible]]` walks. This closes a bypass where
  a shallow delete wrapped a deep handler or invariant target and completed
  after fuel reached zero. Forced-GC, abrupt getter/trap/invariant, exact-fuel,
  refill, pin-balance, target-state, and VM-reuse tests cover those paths.
  `Reflect.deleteProperty(target)` also performs `ToPropertyKey(undefined)`
  instead of returning `false` before deleting a property named `"undefined"`.

  A frozen 28-file Proxy/Reflect delete admission runs **28 pass / 0 fail / 0
  skip** on Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`; future files and
  unrelated feature gates stay skipped. Supported Test262 remains **12751 pass
  / 0 fail / 7687 skip / 20438 total**. Final local gates pass all Rust
  targets/features, lib tests **135/135**, builtins **493/493**, operators
  **123/123**, bugfixes **68/68**, Fuel **28/28**, warnings-denied Clippy,
  rustfmt/diff, release, wasm32, and Python tooling **109/109**. GPT 5.6
  reviewers Mill (`019f7924-1876-7653-81f1-29d05376a591`) and Peirce
  (`019f7924-16df-7d82-8ebf-ab3177b86ddf`) returned `CLEAN` after the nested
  fuel bypass and GC test weakness were corrected; all review sessions are
  closed, and no coder or Umans route was used.

  Feature commit `c85a6b8` passed ordinary CI `29677977508` and all **33/33**
  jobs in full matrix `29677977505`. The 30 artifacts at
  `/tmp/ruja-proxy-delete-feature.29677977505.bAHxEI` aggregate to **30535 pass
  / 6049 fail / 11877 skip / 6 timeout / 0 error / 48467 total / 36584
  pass-or-fail executed** (**63.0%** of all files, **83.5%** of executed
  files). Twenty-nine files are byte-identical to generic-sort baseline
  `29675860634`; only `built-ins` changes from **14899 pass / 5238 fail / 3525
  skip** to **14927 / 5238 / 3497**, exactly **+28 pass / -28 skip**.

- `Array.prototype.sort` and `toSorted` now execute one generic,
  mode-driven `SortIndexedProperties` algorithm after comparator validation,
  `ToObject`, and a single `LengthOfArrayLike`. `sort` performs live ascending
  `HasProperty`/`Get` collection, then strict ascending `Set` and
  `DeletePropertyOrThrow` behavior on the original receiver. `toSorted`
  performs `ArrayCreate` before indexed access, reads every captured index
  without `HasProperty`, and creates a dense own-property result without
  mutating the source. Generic objects, boxed primitives, inherited values,
  accessors, Proxy traps, and getter-driven length/index mutation now follow
  the specified observable order.

  Collected values are rooted immediately across later re-entry, `length`
  coercion is rooted, and collection, comparison, writeback, and deletion
  consume execution fuel with pin-depth restoration on every error or host
  abort. Lengths above 1,048,576 are rejected before indexed scanning as an
  explicit sandbox policy; `toSorted` retains the required `ArrayCreate`
  ordering. Missing Array elements now use receiver-aware generic `[[Set]]`,
  so a Proxy prototype's `set` trap runs before receiver length/extensibility
  checks and a trap throw stops sort writeback immediately.

  On pinned Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`, focused
  `sort`/`toSorted` improves from **44 pass / 23 fail / 8 skip** to **67 / 0 /
  8**, exactly **+23 pass / -23 fail**. Supported Test262 remains **12751 pass
  / 0 fail / 7687 skip / 20438 total**. Final local gates pass all Rust
  targets/features, builtins **493/493**, lib tests **132/132**, bugfixes
  **68/68**, Fuel **28/28**, operators **122/122**, warnings-denied Clippy,
  rustfmt/diff, release, wasm32, and Python tooling **108/108**. GPT 5.6
  reviewers Leibniz (`019f78e7-b4d4-7892-9ebe-4e39a212b6f6`) and
  Chandrasekhar (`019f78e7-b612-73d1-b18b-d285329283fb`) independently
  reviewed the final implementation. Leibniz found the Proxy-prototype setter
  bypass; after the receiver-aware fix both reviewers returned `CLEAN` and
  were closed. No coder model or Umans provider route was used.

  Feature commit `d142220` passed ordinary CI `29675860658` and all **33/33**
  jobs in automatic full matrix `29675860634`. The 30 artifacts at
  `/tmp/ruja-array-sort-generic.29675860634.cEBP0e` aggregate to **30507 pass /
  6049 fail / 11905 skip / 6 timeout / 0 error / 48467 total / 36556
  pass-or-fail executed** (**62.9%** of all files, **83.5%** of executed
  files). Twenty-nine result files are byte-identical to direct-Array baseline
  `29673722819`; only `built-ins` changes from **14876 pass / 5261 fail** to
  **14899 / 5238**, exactly reproducing **+23 pass / -23 fail** without corpus
  drift. GitHub's Node 20 deprecation annotations were warnings only.

- `Array.prototype.sort` and `toSorted` now retain every materialized value,
  receiver, comparator, comparator result, and fresh destination that must
  survive observable JavaScript re-entry. Comparator and default-string
  conversion errors propagate immediately through the stable `O(n log n)`
  merge sort, non-callable comparators are rejected before receiver access,
  and custom comparators are not called for `undefined`. Default comparison
  uses RuJa's sentinel-aware UTF-16 code units, including lone surrogates.

  `sort` now distinguishes holes from explicit `undefined`, writes the sorted
  present values through strict indexed `[[Set]]`, deletes the remaining
  initial range, preserves values appended beyond that range by comparator
  code, and keeps writable indexed descriptors synchronized with dense Array
  storage. `toSorted` creates and pins its destination before comparison, so a
  catchable allocation failure precedes comparator side effects, then
  materializes its sorted copy only after successful comparison. Forced-GC,
  abrupt-completion, exact-cap allocation, shrink/grow, holes, descriptor,
  UTF-16, and comparator-bound regressions cover both methods.

  On pinned Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`, focused
  `sort`/`toSorted` moves from **30 pass / 37 fail / 8 skip** to **44 / 23 /
  8**, exactly **+14 pass / -14 fail**. Supported Test262 remains **12751 pass
  / 0 fail / 7687 skip / 20438 total**. Final local gates pass all Rust
  targets/features, builtins **487/487**, lib tests **131/131**, operators
  **122/122**, warnings-denied Clippy, rustfmt/diff, release, wasm32, and Python
  tooling **108/108**. GPT 5.6 reviewers Aristotle
  (`019f788b-c307-7732-95d7-4585c01a2793`) and Darwin
  (`019f788b-c4a8-7130-b752-020cccd02403`) found and rechecked writeback,
  `undefined`, UTF-16, allocation-order, hole, and descriptor defects; both
  were closed with no remaining code finding. No coder model or Umans route
  was used.

  Feature commit `584c17b` passed ordinary CI `29673722811` and all **33/33**
  jobs in full matrix `29673722819`. The 30 artifacts at
  `/tmp/ruja-array-sort-feature.29673722819.g4q1CX` aggregate to **30484 pass /
  6072 fail / 11905 skip / 6 timeout / 0 error / 48467 total / 36556
  pass-or-fail executed** (**62.9%** of all files, **83.4%** of executed
  files). Twenty-nine files are byte-identical to Array callback baseline
  `29671480315`; only `built-ins` changes, by exactly **+14 pass / -14 fail**.

- Native Array materializers now preserve every heap value they retain across
  JavaScript re-entry. `Array.prototype.map` and `flatMap` pin their source
  snapshots and each callback result until the destination Array owns the
  values. `Array.of` pins its arguments, constructor, and constructed result
  through every observable property definition and the final `length` set.
  Callback throws, Proxy trap errors, and exact-cap result allocation failures
  share one cleanup path and restore the incoming temporary-root depth.

  Previously, a callback result stored only in a Rust `Vec<Value>` could be
  collected by the next callback and its generation-free `GcIdx` cell reused.
  Deterministic regressions observed `map`/`flatMap` changing `"1,2"` into
  `"2,2"`, while a 5,652-case accumulating RegExp harness reached
  `src/value.rs` with `env has no props`. A custom `Array.of` Proxy similarly
  lost its first element after its wrapper slot was reused. All three now
  survive forced collection, and abrupt/final-allocation tests verify pin
  balance plus subsequent VM reuse.

  Final local gates pass all Rust targets/features, builtins **480/480**, lib
  tests **129/129**, warnings-denied Clippy, rustfmt/diff, release, wasm32,
  Python tooling **108/108**, and supported Test262
  **12751 pass / 0 fail / 7687 skip / 20438 total** on
  `020cb74075849d1e404bbcdb62feb7a02e6966db`. The original accumulating
  differential is **5652/5652** with no Node mismatch. The 256 focused
  `Array.of`/`map`/`flatMap` files have zero per-file status changes from the
  preceding grammar binary. GPT 5.6 reviewers Curie
  (`019f783f-f39c-78f2-952b-f514cbbbcef7`) and Hooke
  (`019f783f-f4da-7cd1-b3ee-f752f7aeff89`) returned `CLEAN`; both sessions
  are closed and no coder or Umans route was used. Feature commit `6f822ce`
  passed ordinary CI `29671480301` and all **33/33** jobs in full matrix
  `29671480315`. The 30 artifacts at
  `/tmp/ruja-array-gc-feature.29671480315.01KzGH` aggregate to **30470 pass /
  6086 fail / 11905 skip / 6 timeout / 0 error / 48467 total / 36556
  pass-or-fail executed** (**62.9%** of all files, **83.4%** of executed
  files). Every result is byte-identical to grammar baseline `29669380082`,
  confirming no Test262 status movement.

- RegExp source validation now enforces the ECMAScript quantifier grammar
  before backend compilation. A quantifier consumes exactly one atom and may
  have only one optional lazy `?`; repeated `*`, `+`, `?`, or braced prefixes,
  assertion quantifiers, and malformed legacy escapes that previously hid a
  following quantifier are syntax errors. Legacy quantified lookahead remains
  available only outside `u`/`v` as required by Annex B.

  Character-class range validation is mode-aware. Legacy patterns are
  flattened to UTF-16 code units and preserve Annex B octal, control,
  incomplete-`\c`, character-set-endpoint, raw astral, and non-ASCII identity
  escape behavior. `u`/`v` ranges compare scalar endpoints, combine adjacent
  surrogate escapes, reject descending ranges and character-set range
  endpoints, and validate `v` subtraction operands. Unicode modes also reject
  unescaped standalone `]`/`}`, malformed `\xHH`, and unterminated nested `v`
  classes while retaining valid subtraction and escaped punctuation.

  On Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`, full
  `built-ins/RegExp` moves from **1024 pass / 19 fail / 836 skip / 0 timeout**
  to **1036 / 7 / 836 / 0**, exactly **+12 pass / -12 fail**. The remaining
  seven files are five valid empty-class backend failures, one oversized
  quantifier-integer backend limit, and one nullable-quantifier hybrid-boundary
  mismatch. Node 24 differentials cover 1,219 legacy class combinations and
  858 quantifier/escape combinations without a regression. Two independent
  GPT 5.6 reviews returned `CLEAN` after closing UTF-16 endpoint, octal/control,
  malformed-escape, surrogate-pair, and `v` subtraction defects; no coder
  model or Umans route was used. Feature commit `8578ea2` passed ordinary CI
  `29669380090` and all 33 jobs in full matrix `29669380082`. Twenty-nine of
  30 result artifacts are byte-identical to the lookaround baseline;
  `built-ins` is exactly **+12/-12**. The aggregate is **30470 pass / 6086 fail
  / 11905 skip / 6 timeout / 0 error / 48467 total / 36556 pass-or-fail
  executed** (**62.9%** of all files, **83.4%** of executed files).

- ECMAScript RegExp lookahead and lookbehind now execute as directional
  subpatterns in the vendored matcher. Lookahead runs forward; lookbehind runs
  backward with reversed concatenation, backward literals, wildcards,
  newlines, delegates, captures, ordinary backreferences, and duplicate-name
  capture sets. Positive assertions are atomic, restore the outer cursor, and
  retain successful captures; negative assertions roll matcher state back.
  Unmatched ECMAScript backreferences continue to match the empty string.

  Legacy non-`u`/`v` quantified lookahead now follows the Annex B
  `RepeatMatcher` exception. Finite and nullable repeats preserve required
  captures and reject an empty iteration at the matcher boundary so child
  alternatives can backtrack correctly. Non-Unicode ignore-case normalization
  materializes the ECMAScript legacy canonicalization closure for literals,
  escapes, and classes, preserving non-ASCII case pairs without admitting the
  Unicode-only long-s and Kelvin folds. Scoped flags survive trailing
  lookahead because the unsafe assertion optimizer is disabled only in
  ECMAScript mode.

  ECMAScript matcher work is explicitly bounded even on successful paths:
  speculative branch creation, attempted repeat iterations, and
  repeated-capture clearing share one finite work budget. Deterministic
  unanchored scanning is free. ECMAScript hard execution retains its
  **100,000**-entry stack cap; mode-off `fancy-regex` keeps upstream
  failed-backtrack accounting and the **1,000,000**-entry cap. Stress probes
  for a 100-million zero-width repeat and catastrophic failed matching
  terminate under the work bound.

  On Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`, complete lookbehind is
  **17/17** and full `built-ins/RegExp` moves from **991 pass / 52 fail / 836
  skip / 0 timeout** to **1024 / 19 / 836 / 0**, exactly **+33 pass / -33
  fail**. The remaining 19 are 11 grammar early errors, five empty-class
  matcher files, one quantifier integer-limit file, one nullable-quantifier
  hybrid mismatch, and one Unicode restricted-bracket early error. Rust
  all-target tests, vendored tests **447/447**, release, wasm32, formatting,
  diff checks, and warnings-denied Clippy pass. Two independent GPT 5.6 reviews
  returned `CLEAN` after closing legacy case-fold, nested assertion,
  local-flag optimizer, successful-work accounting, and stack-growth defects.
  No coder model or Umans provider route was used. Feature commit `f1e48f1`
  passed ordinary CI `29666307842` and all 33 jobs in full matrix
  `29666307826`. Twenty-eight of 30 downloaded result files are byte-identical
  to the duplicate-name baseline. `built-ins` moves **+33 pass / -33 fail**;
  Annex B moves **+2 / -2** in exactly its positive and negative
  quantified-assertion files. The artifact aggregate is **30458 pass / 6098
  fail / 11905 skip / 6 timeout / 0 error / 48467 total / 36556 pass-or-fail
  executed** (**62.8%** of all files, **83.3%** of executed files).

- RegExp named captures may now reuse a name when every pair is separated by
  a disjunction alternative, matching ECMAScript `MightBothParticipate` early
  errors. `exec`, `match`, `matchAll`, `replace`, `replaceAll`, `search`,
  `split`, `test`, `groups`, and `indices.groups` select the sole
  participating capture while retaining first-occurrence property order and
  pair-object identity. Same-branch duplicates remain syntax errors.

  Named backreferences use an ID-based backend capture-set table, match empty
  when no alias participated, and preserve capture state across quantified
  alternatives and backtracking. Repeated captures are cleared in the backend
  rather than guessed after a match. A linear Rust matcher prefilters ordinary
  repeated-capture patterns, so capture correction does not move no-match
  probes onto a catastrophic backtracking path. Case-insensitive
  backreferences now compare equal code-point counts with Unicode simple
  folding under `u`/`v` and the legacy ECMAScript uppercase relation otherwise.

  The vendored `fancy-regex` 0.18.0 fork isolates all changes behind
  `ecmascript_mode`: mode-off patterns retain upstream delegation, capture-set
  instructions store only IDs, copy-on-write save membership is bitset-backed,
  and per-iteration clearing is charged to the backend work limit. Reviewer
  stress cases improve from 1.22 seconds / 271 MB to 0.03 seconds / 14 MB for
  4,000 aliases and references; doubling to 8,000 takes 0.06 seconds / 19 MB.
  A 6,400-capture repeat that previously took about 4.8 seconds now terminates
  at the work limit in 0.02 seconds.

  Exact Test262 admission freezes 15 positive duplicate-name files and four
  same-alternative parse-negative files. The admission is **19/19** and full
  `built-ins/RegExp` is **991 pass / 52 fail / 836 skip / 0 timeout**, exactly
  **+13 pass / -13 skip** with no failure increase. Rust all-target tests,
  vendored tests, Python tooling **108/108**, release, wasm32, formatting, and
  warnings-denied Clippy pass; the supported subset remains
  **12751/0/7687/20438**. Two GPT 5.6 reviews found and closed capture
  post-processing, Unicode byte-length, mode-isolation, quadratic resource,
  and packaging hazards. Crates.io publication is explicitly disabled while
  the fork is a path dependency; re-enabling it requires upstreaming the
  patches or publishing the fork first. Feature commit `48b8b78` passed
  ordinary CI `29662790684` and all 33 jobs in full matrix `29662790652`.
  Twenty-eight of 30 result artifacts are byte-identical to the preceding
  baseline; `built-ins` moves **+15 pass / -15 skip** and
  `language/literals` moves **+4 / -4**, exactly the 19 frozen files. The
  aggregate is **30423 pass / 6133 fail / 11905 skip / 6 timeout / 0 error /
  48467 total**.
- RegExp `d` now exposes the complete match-indices result shape. Native exec
  adds the own `indices` property after `groups`, creates method-Realm pair
  Arrays for every participating capture, stores explicit `undefined` for
  unmatched captures, and builds a null-prototype `indices.groups` whose named
  properties alias the numeric pair objects. Internal flag slots, property
  descriptors/order, foreign Realms, repeated and empty captures, and
  `__proto__` names are covered.

  Backend byte ranges are converted to original UTF-16 coordinates in one
  ordered pass. Unicode execution over dynamically concatenated surrogate
  sentinels uses a scalar matcher view plus an explicit boundary map, so
  match strings, `index`, `lastIndex`, groups, and indices agree with ordinary
  supplementary strings, including a `lastIndex` inside a pair. Nested pair
  allocation is rooted, consumes fuel per capture, succeeds at the exact heap
  cap, and restores pin depth when one cell below the requirement.

  Named groups now decode raw and escaped ECMAScript identifier names, lower
  definitions/references without changing public `source`, and preserve
  unmatched, forward, self, and digit-suffixed backreference semantics.
  Invalid declarations/references are literal early errors; legacy non-`u`
  `\k` identity escapes and incomplete `\u` Annex B behavior are preserved.
  Exact Test262 admission remains frozen to seven audited named-group index
  files.

  Final local gates include tooling **107/107**, Rust lib/unit **126/126**,
  builtins **473/473**, bugfixes **67/67**, Fuel **27/27**, release, wasm32,
  warnings-denied Clippy, formatting, match-indices **14/14**,
  `regexp-modifiers` **70/70**, full RegExp **978/52/849/0**, and the supported
  subset **12751/0/7687/20438**. Against the previous RegExp diagnostic this is
  **+18 pass / -11 fail / -7 skip** with no old passing file regressed. GPT 5.6
  reviewers found and closed byte/UTF-16 mapping, GC, named-reference,
  malformed-escape, early-error, and lookbehind-regression defects; every
  session is closed and no coder or Umans route was used. Feature commit
  `7ceb8e9` plus CI-portability fix `08f2fa4` passed ordinary CI
  `29657974089`. Full matrix `29657718789` also passed all 30 shards. Its
  downloaded artifacts report **30404 pass / 6133 fail / 11924 skip / 6
  timeout / 0 error / 48467 total**, or **62.7%** of all files and **83.2%**
  of executed files. Twenty-eight shard results are byte-identical to the
  preceding feature baseline; `built-ins` contains the exact RegExp delta
  (**+18 pass / -11 fail / -7 skip**). The separate Annex B artifact delta
  (**+3 pass / -3 fail**) came from independently cloned, unpinned Test262
  HEADs: old and new binaries produced no Annex B status changes against the
  same local checkout.
- Active-ignoreCase RegExp `\w`/`\W` escapes now use ECMAScript
  WordCharacters outside classes and in ordinary character classes. Plain
  `i` mode remains exactly ASCII alphanumeric plus underscore; `iu` and `iv`
  add only the canonicalized `U+017F` long s and `U+212A` Kelvin sign. Local
  modifier add/remove scopes, complements, negated classes, Annex B ranges,
  escaped-backslash parity, UTF-16 code-unit input, and mixed literal/word
  classes are covered.

  Ordinary classes are normalized through `regex-syntax` HIR, closed under
  the applicable ECMAScript canonicalization relation, complemented only
  after closure, and emitted under a scoped backend case-disable. Legacy
  non-Unicode equivalence groups and a **128-entry** size-bounded class cache
  avoid rescanning the BMP for repeated dynamic patterns. Complex nested `v`
  set operations retain native fallback with explicit nesting depth so one
  class cannot corrupt normalization of a later class.

  Plain-`i` `\b`/`\B` uses the same ASCII word inventory. Unicode boundaries
  deliberately remain on the existing linear backend: a lookaround rewrite
  moved nested quantified input onto `fancy-regex` and could exhaust its
  backtracking limit. Unicode boundaries, ignore-case backreferences, the
  `U+F0000..U+F07FF` UTF-16 sentinel collision, and full nested-`v` set algebra
  remain documented limitations instead of being hidden by a partial fix.

  Final local gates include tooling **106/106**, Rust lib/unit **124/124**,
  builtins **470/470**, bugfixes **67/67**, Fuel **26/26**, release, wasm32,
  warnings-denied Clippy, formatting, `regexp-modifiers` **70/0**, full RegExp
  **960/63/856/0**, and the supported subset **12751/0/7687/20438**. GPT 5.6
  reviewers Ohm and Boole reproduced the backtracking, nested-`v`, cache-cost,
  and per-class fallback defects. All findings were addressed, Boole's final
  follow-up returned `CLEAN`, and both sessions were closed. No coder model or
  Umans provider route was used. Feature commit `844593b` passed CI
  `29653243121` and full matrix `29653243102`. All 30 result files are
  byte-identical to corrected baseline `29646302891`; the aggregate remains
  **30383 pass / 6147 fail / 11931 skip / 6 timeout / 0 error / 48467 total**
  with **36530** pass-or-fail executions.
- The Test262 runner's machine-readable `RATE=` line now preserves actual
  `SKIP` and `TOTAL` counts when `RAN=0`. The old zero-execution branch
  printed zeros even though the human-readable summary was correct, causing
  the full workflow to omit 150 skipped-only files from its aggregate.
  All-skip, empty, timeout-only, and error-only directories retain the
  existing key order and zero pass rate while reporting their real totals.
  The regression is covered by Python tooling **106/106**, formatting,
  warnings-denied Clippy, and a real all-skipped directory run. Commit
  `8f70593` passed CI `29646302861` and full matrix `29646302891`. Of 30
  result files, 27 are byte-identical to the feature run and only the three
  skipped-only `RATE=` lines change. The corrected aggregate is **30383 pass
  / 6147 fail / 11931 skip / 6 timeout / 0 error / 48467 total** (**62.7%**
  of all files, **83.2%** of executed files).
- RegExp `\d`/`\D` now use the ECMAScript ASCII digit set instead of
  Rust's Unicode `Nd` set, and `\s`/`\S` use the exact ECMAScript
  WhiteSpace plus LineTerminator set. This includes `U+FEFF` and excludes
  `U+0085` and `U+180E`. The lowering is shared by the linear and
  backreference regex backends, works outside classes and in ordinary,
  non-nested character classes, and preserves non-Unicode UTF-16 matching.
  Annex B ranges with a character class escape endpoint, including
  `[a-\d]`, now preserve the required
  literal hyphen and set-union behavior.

  The Test262 runner and analyzers give 30 seconds only to the six generated
  complement-set files and 60 seconds only to the one 65,536-code-unit
  exhaustive file; neighboring tests retain the 8-second default. Focused
  `built-ins/RegExp/CharacterClassEscapes` closes at **12 pass / 0 fail / 0
  skip / 0 timeout**. The complete `built-ins/RegExp` subtree improves from
  **951 pass / 66 fail / 856 skip / 6 timeout** to **960 / 63 / 856 / 0**,
  exactly **+9 pass / -3 fail / -6 timeout**. Final local gates include
  tooling **105/105**, Rust lib/unit **124/124**, builtins **469/469**,
  bugfixes **67/67**, Fuel **26/26**, release, wasm32, warnings-denied Clippy,
  formatting, and the supported subset at **12751/12751**. GPT 5.6 reviewers
  Godel and Sagan returned `CLEAN`; no coder or Umans route was used. Feature
  commit `c9065fa` passed CI `29644825071` and full matrix `29644825073`.
  Of 30 matrix result files, 28 are byte-identical to the preceding run;
  built-ins improves by **+9 pass / -3 fail / -6 timeout**, and the Annex B
  RegExp range test improves by **+1 pass / -1 fail**. Artifacts and the
  corrected aggregate are recorded in `docs/test262.md`.
- `RegExp.prototype[Symbol.replace]` now follows the generic ECMA-262
  algorithm instead of iterating the native regex backend directly. It
  observes replacement callability, global state, strict `lastIndex` writes,
  and dynamic `RegExpExec`; collects result objects before processing them;
  and then reads match length, index, captures, and groups in specification
  order. Callable Proxy replacers are supported, global empty matches advance
  by UTF-16 index, and replacement output is assembled in code units.

  The shared substitution path now implements `$$`, `$&`, ``$` ``, `$'`,
  `$n`, `$nn`, and `$<name>` without confusing ordinary trailing digits for
  capture tokens. Replacement arguments respect the sandbox argument cap,
  observable results remain rooted across re-entrant GC, exact-cap allocation
  is preserved, and match/output work consumes fuel. The Test262 host now also
  provides the required `print` shim for synchronous and asynchronous tests.

  Focused `built-ins/RegExp/prototype/Symbol.replace` closes at **60 pass / 0
  fail / 10 skip**. The complete `built-ins/RegExp` subtree improves from
  **922 pass / 95 fail / 856 skip / 6 timeout** to **951 / 66 / 856 / 6**,
  exactly **+29 pass / -29 fail**. Final local gates include tooling
  **102/102**, Rust lib/unit **124/124**, builtins **468/468**, bugfixes
  **67/67**, Fuel **26/26**, release, wasm32, warnings-denied Clippy,
  formatting, and the supported subset at **12751/12751**. GPT 5.6 reviewers
  Ampere and Nash returned `CLEAN`; no coder or Umans route was used. Feature
  commit `55c5943` passed CI `29640862796` and full matrix `29640862819`. Of
  30 result files at `/tmp/ruja-artifacts-regexp-replace-feature.v6d1PN`, 29
  are byte-identical to the preceding RegExp split artifacts; only built-ins
  changed. The aggregate is **30373 pass / 6151 fail / 11781 skip / 12 timeout
  / 0 error / 48317 total** (**62.9%** of all files, **83.2%** of executed
  files).
- `RegExp.prototype[Symbol.split]` now follows the generic specification
  algorithm instead of relying on `String.prototype.split`'s RegExp class-name
  shortcut. It performs ordered string conversion, species construction,
  flags handling, sticky matching, strict `lastIndex` writes, capture
  insertion, limit checks, and UTF-16 index advancement. Result arrays use the
  method Realm and the sandbox allocator; every observable intermediate is
  rooted across re-entrant GC, and native search/capture work consumes fuel.
  `String.prototype.split` delegates only through a callable `@@split`; a
  nullish hook now falls back to ordinary UTF-16 code-unit string splitting.

  Non-Unicode RegExp execution now has an explicit code-unit backend mode, so
  raw supplementary characters, escaped surrogate pairs, lone-surrogate
  matches, sticky mid-pair `lastIndex`, captures, and repeated-capture clearing
  agree with ECMAScript without changing scalar-backed replacement behavior.
  Callable Proxy hooks are recognized. Native RegExp match arrays are created
  through a narrowly scoped GC-retrying path in the executing Realm, allowing
  earlier split matches to be reclaimed at an exact heap cap without changing
  the allocation contract of unrelated array helpers.

  Focused Test262 closes at **43 pass / 0 fail / 1 skip** for `@@split` and
  **117 / 0 / 3** for `String.prototype.split`. The complete
  `built-ins/RegExp` subtree improves from **880 pass / 137 fail / 856 skip / 6
  timeout** to **922 / 95 / 856 / 6**, exactly **+42 pass / -42 fail**. Final
  local gates include tooling **101/101**, Rust lib/unit **122/122**, builtins
  **467/467**, release, wasm32, warnings-denied Clippy, formatting, and the
  supported subset at **12751/12751**. GPT 5.6 reviewers Fermat and Beauvoir
  returned `CLEAN`. Feature commit `0e08dc8` passed CI `29638102394` and full
  matrix `29638102407`. Of 30 result files, 29 are byte-identical to the
  preceding RegExp artifacts; built-ins changed by **+42 pass / -42 fail**.
  The aggregate is **30344 pass / 6180 fail / 11781 skip / 12 timeout / 0
  error / 48317 total** (**62.8%** of all files, **83.1%** of executed files).
- RegExp construction now follows the specification's `IsRegExp` and
  constructor phases. An explicit internal `[[RegExpMatcher]]` marker replaces
  the observable class-name approximation; `Symbol.match` overrides are
  honored before the internal fallback; calls can return the input only under
  the constructor shortcut; RegExp inputs copy internal source/flags while
  regexp-like inputs perform ordered property access; and allocation occurs
  after new-target prototype selection but before source/flags conversion.
  `%RegExp%`, `%RegExp.prototype%`, and
  `%RegExpStringIteratorPrototype%` are immutable per-Realm registry entries,
  traced and transactionally rolled back. RegExp literals, `RegExpCreate`, and
  `@@matchAll` use those Realm-local intrinsics without consulting replaced
  globals. `@@matchAll` now preserves `ToString`/species/flags/`lastIndex`
  order, uses strict `Set`, returns Realm-correct iterator/result objects, and
  roots every observable intermediate across GC.

  The adjacent receiver-aware property path now follows OrdinarySet and Proxy
  invariants through nested Proxies, null traps, custom receivers, fresh
  descriptor objects, TypedArray/Array/mapped-arguments exotics, and
  allocation-triggered GC. Array index writes honor non-writable length and
  synchronize sparse/materialized length descriptors and inline caches.
  Array length definition performs both observable conversions, descending
  deletion with rollback above a non-configurable index, requested
  writability changes, and synthetic own-length lookup before prototype
  setters or Proxy traps. Registration is now **13 eager / 32 deferred**
  native constructors and the transactional Realm inventory is **31**
  registry families.

  Eight exact RegExp construction files are newly admitted and pass. The full
  `built-ins/RegExp` result improves from **865 pass / 144 fail / 864 skip / 6
  timeout** to **880 / 137 / 856 / 6**; focused `@@matchAll` is **25 pass / 0
  fail / 1 skip**, and the supported subset remains **12751 / 0 / 7687**.
  Final local gates include tooling **101/101**, Rust lib/unit **120/120**,
  bugfixes **67/67**, builtins **463/463**, release, wasm32, warnings-denied
  Clippy, and formatting. GPT 5.6 reviewers Dirac and Laplace returned `CLEAN`.
  Feature commit `ff492ff` passed CI `29633368519` and full matrix
  `29633368501`. Of the 30 artifacts at
  `/tmp/ruja-artifacts-regexp-feature.kNQTlF`, 29 are byte-identical to the
  Dynamic Function baseline; built-ins changed by **+108 pass / -100 fail / -8
  skip**. The aggregate is **30302 pass / 6222 fail / 11781 skip / 12 timeout /
  0 error / 48317 total** (**62.7%** of all files, **83.0%** of executed files).
- The Dynamic Function family now follows the shared CreateDynamicFunction
  protocol. `Function`, `AsyncFunction`, `GeneratorFunction`, and
  `AsyncGeneratorFunction` convert parameter arguments left-to-right before
  converting the body, apply the local-trust string-compilation policy after
  conversion, validate parameter and body grammar independently with
  delimiter-safe line boundaries, and perform the combined parse for strict
  early errors. Calls use the active constructor as the effective new target;
  construction uses the actual `NewTarget`. Generated closures and non-object
  prototype fallbacks come from immutable intrinsics in that constructor's
  Realm, including BoundFunction and transparent-Proxy new targets. Ordinary
  and generator-family `.prototype` objects receive the correct Realm-local
  parent. Compilation-table publication occurs only after observable
  prototype lookup, allocations use the sandbox GC retry, and failed one- or
  two-cell creation rolls back only the outer compiled-function suffix while
  preserving successful re-entrant compilation. `Function.prototype.bind`
  now copies the target's actual `[[Prototype]]`; fresh Proxy trap results are
  rooted through invariant checks and bound allocation. Parser coverage now
  treats lexer-promoted contextual names consistently as BindingIdentifiers,
  revalidates late strictness, distinguishes direct `catch (let)` from
  lexical/catch binding patterns, and enforces class-static-block arrow
  parameter and lexical-`arguments` early errors. Registration is now **14
  eager / 31 deferred** native constructors. Seven exact Test262 files are
  newly admitted and pass; the four complete constructor directories improve
  from **420 pass / 61 fail / 92 skip** with the preceding binary to **429 /
  52 / 92** with the same runner, with nine fail-to-pass transitions and no
  regressions. Final local gates include tooling **101/101**, Rust lib/unit
  **114/114**, builtins **461/461**, classes **105/105**, modules **31/31**,
  Fuel **24/24**, release, and wasm32. GPT 5.6 reviewers Ptolemy and Copernicus
  returned `CLEAN`. Feature commit `a320d15` passed CI `29624418616` and full
  matrix `29624418655`. Of the 30 artifacts at
  `/tmp/ruja-artifacts-dynamic-function-feature.upHgF8`, 29 are byte-identical
  to the Date baseline; built-ins changed by **+13 pass / -6 fail / -7 skip**.
  The aggregate is **30194 pass / 6322 fail / 11789 skip / 12 timeout / 0
  error / 48317 total** (**62.5%** of all files, **82.7%** of executed files).
- Date calls and construction now follow separate native paths. Calling Date
  ignores the supplied `this`, does not coerce supplied argument values, and
  returns a date String;
  construction computes and clips its Date value before observing
  `NewTarget.prototype`, so abrupt conversion prevents that lookup. A
  non-object prototype falls back to the immutable `%Date.prototype%` from
  the new target's Realm through BoundFunction and transparent-Proxy targets.
  Each created Realm now owns its Date constructor, prototype, methods, and
  static functions, and the new `realm_date_prototypes` registry is traced,
  rolled back after failed Realm construction, and included in the **29**
  registry-family inventory. Constructed Dates store `[[DateValue]]` in an
  internal private slot rather than an observable `__time__` property, while
  `%Date.prototype%` remains unbranded. Date uses the common sandbox allocator,
  roots getter-produced prototypes across collection, consumes exactly one
  cell, and preserves the exact hard heap cap. Removing the final generic
  receiver-preallocation user leaves **18 eager / 27 deferred** native
  constructors. Exact Date admission is **5/5**; the broad Date subtree
  improves from **512 pass / 4 fail / 78 skip** with the preceding binary to
  **516 / 0 / 78** with the same runner. Final local gates include tooling
  **101/101**, Rust lib/unit **108/108**, builtins **461/461**, classes
  **105/105**, modules **31/31**, Fuel **24/24**, release, and wasm32. Both GPT
  5.6 reviews returned `CLEAN`. Feature commit `5bdc7bd` passed CI
  `29618073392` and full matrix `29618073439`. Of the 30 result files at
  `/tmp/ruja-artifacts-date-feature.ezDrIL`, 29 are byte-identical to the
  primitive-wrapper artifacts; built-ins changed by **+5 pass / -5 skip**.
  The aggregate is **30181 pass / 6328 fail / 11796 skip / 12 timeout / 0
  error / 48317 total** (**62.5%** of all files, **82.7%** of executed files).
- String, Number, and Boolean construction is now owned by the constructor
  body instead of generic receiver preallocation. Calls ignore the supplied
  `this` and return primitives; construction completes String/Number
  conversion before reading `NewTarget.prototype`, while Boolean performs its
  non-observable conversion before that lookup. Non-object prototypes fall
  back to the immutable `%String.prototype%`, `%Number.prototype%`, or
  `%Boolean.prototype%` from the new target's Realm, including BoundFunction
  and transparent-Proxy targets, without consulting replaced global bindings.
  `String(Symbol())` remains the descriptive call conversion while
  `new String(Symbol())` throws before prototype observation. Wrapper
  allocation pins a getter-produced prototype, uses the sandbox allocator,
  succeeds with exactly one free cell, and returns the Realm-local reserve at
  a saturated cap. At that checkpoint, registration coverage inventoried
  **18 eager / 26 deferred / 1 preallocated** native constructors and left
  Date for the separate follow-up above. The exact ten-file admission is
  **10/10**, all 13 wrapper subclass files pass, and the broad String/Number/
  Boolean result is **1504 pass / 0 fail / 110 skip**; with the same runner the
  preceding binary was **1500 / 4 / 110**. Final local gates include tooling
  **101/101**, Rust lib/unit **104/104**, builtins **461/461**, classes
  **105/105**, modules **31/31**, Fuel **24/24**, release, and wasm32. Feature
  commit `ddf3d55` passed CI `29613370285` and full matrix `29613370302`.
  Of the 30 result files at
  `/tmp/ruja-artifacts-primitive-wrappers-feature.ArVzjB`, 29 are byte-identical
  to the preceding feature artifacts; built-ins changed by **+10 pass / -10
  skip**. The aggregate is **30176 pass / 6328 fail / 11801 skip / 12 timeout
  / 0 error / 48317 total** (**62.5%** of all files, **82.7%** of executed
  files).
- Native function constructibility is now independent from observable
  `.prototype` state. `FunctionKind::Native` stores
  `Option<NativeConstructMode>`: `None` means no `[[Construct]]`, while a
  present mode selects the receiver/prototype protocol. BigInt and Symbol keep
  `[[Construct]]` for `extends` and `newTarget` checks but reject construction
  before coercion; Proxy and `%TypedArray%` own their validation without an
  automatic `NewTarget.prototype` read. Proxy now requires `new`, each created
  Realm owns its Proxy constructor and `revocable` function, construct-trap
  argument arrays use the operation Realm, and revocable intermediates remain
  rooted at an exact heap cap. IsConstructor, BoundFunction/transparent-Proxy
  construction, and normal/spread `super()` forwarding use the full iterative
  `[[Construct]]` path. Proxy `get`, `getOwnPropertyDescriptor`, and
  `isExtensible` forwarding also avoid host recursion; trap-bearing chains
  retain pending results as LIFO-safe GC roots and validate invariants in
  reverse order. Regressions exercise 20,000 constructor wrappers, 100,000
  transparent and trap-bearing Proxy layers, fresh descriptor objects under
  GC, Realm identity, and body-before-coercion order. Final local gates include
  tooling **101/101**, all-target Rust lib/unit **100/100**, builtins
  **461/461**, classes **105/105**, modules **31/31**, Fuel **24/24**, release,
  and wasm32. The pinned affected cohort improves from **250 pass / 8 fail /
  338 skip** to **251 / 7 / 338** with the same runner; five exact native-
  construct files are newly admitted and pass. Feature commit `894e4bc`
  passed CI `29609644806` and full matrix `29609644698`. Of the 30 result
  files at `/tmp/ruja-artifacts-native-constructibility-feature.UeJr8Q`, 29
  are byte-identical to the allocation-metadata baseline; built-ins changes by
  **+5 pass / -5 skip**. The aggregate is **30166 pass / 6328 fail / 11811
  skip / 12 timeout / 0 error / 48317 total** (**62.4%** of all files,
  **82.7%** of executed files).
- The initial native-construction metadata migration removed the immutable
  function-name allowlist that decided whether generic dispatch allocated a
  receiver. It introduced explicit eager, deferred, and migration-time
  preallocation modes; the later primitive-wrapper and Date units above
  removed the final preallocation user and the enum variant. The eager path
  preserves the existing observable
  `NewTarget.prototype` and fallback-error order, while bound functions and
  transparent Proxies keep forwarding the original new target. Construction
  pins the resolved constructor, new target, and every argument through the
  complete observable path, and scoped pending-new-target state is restored
  even when pre-dispatch setup or normal/spread `super()` fails. Exact-cap
  `new Array()` no longer wastes a discarded ordinary receiver, and WeakMap
  and WeakSet now require `new`, allocate their specialized objects, and honor
  subclass prototypes. At that checkpoint, registration tests inventoried
  **19 eager / 19 deferred** constructors in both the main and created Realms.
  Local gates
  include tooling **100/100**, all-target Rust lib/unit **95/95**, builtins
  **461/461**, classes **104/104**, modules **31/31**, Fuel **24/24**, release,
  and wasm32. The eager constructor cohort improves from **3952 pass / 1223
  fail** to **3954 / 1221** solely through the WeakMap/WeakSet fixes; the
  deferred cohort, Promise **433/0/270**, dynamic import **620/0/384**, and the
  supported subset **12751/0/7687** are unchanged. Feature commit `6cc6dff`
  passed CI `29596899916` and full matrix `29596899918`; 29 of 30 result files
  at `/tmp/ruja-artifacts-native-construct-feature.KyS5dn` are byte-identical
  to the Realm-rollback baseline, with only built-ins changing by **+2 pass /
  -2 fail**. The aggregate is now **30161 pass / 6328 fail / 11816 skip / 12
  timeout / 0 error / 48317 total** (**62.4%** of all files, **82.7%** of
  executed files).
- `$262.createRealm()` construction is now transactional under the hard heap
  object cap. The fresh global environment is pinned before any collecting
  allocation, one lexical transaction owns every nested installer pin through
  intrinsic population and final wrapper attachment, and every failure
  truncates that pin suffix before removing every per-Realm registry family.
  The resulting partial object graph is collectible before the
  caller Realm materializes its `RangeError`. Regressions sweep every failing
  capacity through the production host path, repeat the final-wrapper
  boundary, prove exact-cap success and later VM reuse, and force collection
  both before intrinsic publication and while the complete provisional graph
  is live. The current and preceding release binaries produce the same
  **109 pass / 8 fail / 69 skip** result across all 186 pinned Test262 files
  containing `$262.createRealm`; Promise remains **433/0/270**, dynamic import
  **620/0/384**, and the supported subset **12751/0/7687**. Local gates include
  tooling **100/100**, Rust lib/unit **90/90**, builtins **458/458**, modules
  **31/31**, and Fuel **24/24**. Feature commit `87741b1` passed CI
  `29587781649` and full matrix `29587781683`; all 30 result files at
  `/tmp/ruja-artifacts-realm-rollback-feature.S47HSt` are byte-for-byte
  identical to the preceding hard-heap baseline, preserving **30159 pass /
  6330 fail / 11816 skip / 12 timeout / 0 error / 48317 total** (**62.4%** of
  all files, **82.7%** of executed files).
- Catchable errors can now settle already-created Promises at an exact heap
  object cap. Error materialization pins the operation Realm prototype, tries
  one rooted GC allocation, and falls back only on typed `HeapLimitExceeded`
  to an immutable, preallocated Realm-local
  `RangeError("heap limit exceeded")` that already counts toward the cap.
  Intrinsic Error prototypes and emergency values are permanent roots;
  explicit thrown objects preserve identity and Fuel remains a host abort.
  Promise resolving getters, self-resolution, callable-then job setup, initial
  dynamic-import jobs, and post-evaluation dynamic-import continuations now
  reject their consumed capabilities instead of propagating the allocation
  error while leaving them pending. Regressions also cover fresh allocation
  after reclaiming garbage, repeated bounded sentinel reuse, immutability,
  cross-Realm prototypes, and intrinsic survival after mutable globals are
  replaced. Final local gates include Promise **433/0/270**, dynamic import
  **620/0/384**, supported subset **12751/0/7687**, tooling **100/100**, Rust
  lib/unit **87/87**, builtins **458/458**, modules **31/31**, and Fuel
  **24/24**. Feature commit `82dd814` passed CI `29582672359` and full matrix
  `29582673229`; all 30 result files match the preceding async-job baseline
  byte-for-byte, preserving **30159 pass / 6330 fail / 11816 skip / 12 timeout
  / 0 error / 48317 total** (**62.4%** of all files, **82.7%** of executed
  files).
- Deferred Promise, thenable, dynamic-import, await, async-iterator, and
  async-generator jobs now retain the Realm that owns their operation instead
  of consulting whichever execution context happens to be active later.
  Catchable native failures become real Error objects in that Realm, explicit
  JavaScript throws preserve identity, and non-catchable Fuel exhaustion stays
  a host abort rather than becoming a Promise rejection. Promise executors,
  reaction settlement, await replacement capabilities, and generated iterator
  results keep their live values pinned through every collecting allocation.
  Aborted async frames restore stack and frame depth, post-await module aborts
  cache an errored module record without poisoning unrelated siblings, and
  async generators release queue ownership, drain queued siblings, root drain
  jobs, retry only terminal `next()` settlement, and never replay a body whose
  state already advanced. `MakeClosure` also roots its fresh `.prototype`
  across name-environment and function allocation. Final local gates include
  Promise **433/0/270**, dynamic import **620/0/384**, supported subset
  **12751/0/7687**, tooling **100/100**, Rust lib/unit **77/77**, builtins
  **458/458**, modules **31/31**, and Fuel **24/24**. Feature commit `d3698b3`
  passed CI `29577669220` and full matrix `29577669208`; all 30 result files
  match the execution-context baseline byte-for-byte, preserving **30159 pass
  / 6330 fail / 11816 skip / 12 timeout / 0 error / 48317 total** (**62.4%**
  of all files, **82.7%** of executed files).
- Native and interpreted calls now use a stack-ordered execution-context model
  instead of VM-wide scalar native-callee and construction slots. Each
  active call or resumption owns its callee Realm, while native contexts also
  own `NewTarget` and the already-observed prototype. Interpreted setup and
  bytecode execution therefore keep primitive lookup, sloppy `this`, global
  writes, arguments/rest allocation, and catchable error objects in the callee
  Realm across native callbacks, bound/Proxy forwarding, generator method
  borrowing, async suspension, nested calls, abrupt completion, and forced GC.
  Native-only accessors read only the top native context, every context is
  traced as a GC root, and all normal and `Result`-based abrupt paths restore
  the previous stack depth. The supported subset remains **12751/0/7687**,
  tooling is **100/100**, Rust lib/unit tests are **69/69**, and builtins are
  **457/457**. Feature commit `46fecef` passed CI `29567895773` and full
  matrix `29567895748`; all 30 result files match the Promise-keyed baseline
  byte-for-byte, preserving **30159 pass / 6330 fail / 11816 skip / 12 timeout
  / 0 error / 48317 total** (**62.4%** of all files, **82.7%** of executed
  files).
- `Promise.allKeyed` and `Promise.allSettledKeyed` now snapshot raw own keys,
  then observe each key's Proxy-aware descriptor before any value read,
  `C.resolve` call, or `then` operation. Missing and non-enumerable descriptors
  are skipped without advancing the compact result index, while accepted
  values, promises, records, callbacks, and `then` values stay rooted across
  every observable re-entry. Exact keyed admission is **63/63**, broad Promise
  is **433/0/270** normally and **703/703** with all gates lifted, the supported
  subset remains **12751/0/7687**, tooling is **100/100**, and Rust builtins are
  **457/457**. Feature commit `3489f00` passed CI `29562059144` and full matrix
  `29562059145`; 29 of 30 result files match the Function.apply baseline
  byte-for-byte and built-ins changes by exactly **+45 pass / -45 skip**,
  producing **30159 pass / 6330 fail / 11816 skip / 12 timeout / 0 error /
  48317 total** (**62.4%** of all files, **82.7%** of executed files).
- `Function.prototype.apply` now shares the observable
  `CreateListFromArrayLike` implementation used by `Reflect.apply` and
  `Reflect.construct`. It reads and coerces `length` before ordered indexed
  `Get` operations, observes inherited properties, accessors, Proxies, array
  holes, and TypedArrays, rejects non-object argument lists, and preserves the
  specified omitted/`null`/`undefined` no-argument behavior. The shared helper
  checks the 1,048,576-element sandbox cap after `ToLength`, pins every result
  immediately across later re-entry, and keeps the complete list rooted
  through the target call on both normal and abrupt paths. Direct
  `Function.prototype.apply` Test262 is now **48/48**, direct Reflect remains
  **19/19**, the supported subset remains **12751/0/7687**, tooling is
  **99/99**, and Rust builtins are **454/454**. Feature commit `ffea75a` passed
  CI `29558468870` and full matrix `29558468852`; 29 of 30 result files match
  the Reflect baseline byte-for-byte and built-ins changes by exactly **+6
  pass / -4 fail / -2 skip**, producing **30114 pass / 6330 fail / 11861 skip
  / 12 timeout / 0 error / 48317 total** (**62.3%** of all files, **82.6%** of
  executed files).
- `Reflect.apply` and `Reflect.construct` now keep an observable
  `argumentsList.length` result rooted through `ToLength`, pin each indexed
  value immediately, and retain the complete materialized list through the
  target call or construction. Every normal and abrupt path restores the GC
  pin stack. Forced-GC regressions cover length coercion, later index getters,
  nested and Proxy calls, post-materialization `NewTarget.prototype` lookup,
  returned and thrown arguments, target errors, and Promise executors. The
  complete direct Test262 surface is admitted at **19/19**; broad Promise
  remains **388/0/315** normally and **699/4** with all gates lifted, the
  supported subset remains **12751/0/7687**, tooling is **98/98**, and Rust
  builtins are **453/453**. Feature commit `be24904` passed CI `29555440736`
  and full matrix `29555440756`; 29 of 30 result files match the prior
  baseline byte-for-byte and built-ins changes by exactly **+19 pass / -19
  skip**, producing **30108 pass / 6334 fail / 11863 skip / 12 timeout / 0
  error / 48317 total** (**62.3%** of all files, **82.6%** of executed files).
- Promise construction now validates executor callability before observing
  `NewTarget.prototype`. Promise uses its internally allocating native path,
  while observable prototype values and the fresh Promise remain GC-rooted
  across instance and resolving-state allocation. Regressions cover abrupt
  getter identity, getter-before-executor order, zero executor calls on
  prototype failure, target- and NewTarget-Realm behavior, and forced
  allocation-time collection. Exact Test262 admission is **1/1**; broad
  Promise is **388/0/315** normally and **699/4** with every gate lifted, the
  supported subset remains **12751/0/7687**, tooling is **97/97**, and Rust
  builtins are **452/452**. Feature commit `568171c` plus CI portability
  follow-up `c224613` passed CI `29552437436` and full matrix `29552437437`;
  only built-ins changes by **+1 pass / -1 skip**, producing **30089 pass /
  6334 fail / 11882 skip / 12 timeout / 0 error / 48317 total** (**62.3%** of
  all files, **82.6%** of executed files).
- `Promise.prototype.finally` now performs receiver validation and
  `SpeciesConstructor`, creates Realm-correct anonymous `ThenFinally` and
  `CatchFinally` built-ins, calls `onFinally` with no arguments, applies the
  abstract `PromiseResolve(C, result)` operation without consulting
  `C.resolve`, and restores the original fulfillment or rejection through an
  anonymous value thunk or thrower. Promise reactions now normalize
  non-callable handlers and always pass successful handler results through the
  capability resolve function, preserving observable overridden-`then` calls,
  thenable job ordering, self-resolution checks, and subclass constructor
  counts. `Promise.resolve` also validates its receiver constructor before the
  same-Promise fast path. Exact Test262 admission is **37/37**; complete
  `finally`, forced `then`, forced `resolve`, and four-combinator diagnostics
  are **29/29**, **75/75**, **30/30**, and **390/390**. Broad Promise is
  **387/0/316** normally and **698/5** with every gate lifted, the supported
  subset remains **12751/0/7687**, tooling is **96/96**, and Rust builtins are
  **452/452**. Feature commit `0581788` passed CI `29549608490` and full
  matrix `29549608468`; only built-ins changes by **+24 pass / -24 skip**,
  producing **30088 pass / 6334 fail / 11883 skip / 12 timeout / 0 error /
  48317 total** (**62.3%** of all files, **82.6%** of executed files).
- Promise combinators now materialize catchable native setup failures as real
  method-Realm Error objects instead of rejection strings, preserve explicit
  thrown-value identity, and propagate non-catchable host aborts. A shared
  GetPromiseResolve path makes `all`, `allSettled`, `any`, and `race` reject
  their existing capability when `resolve` lookup or callability is abrupt;
  iterator lookup remains unobserved in that case. The exact Test262 cohort is
  **95/95**, all four directories are **229/0/161**, broad Promise is
  **363/0/340**, the supported subset remains **12751/0/7687**, tooling is
  **95/95**, and Rust builtins are **449/449**. Feature commit `fa21315` passed
  CI `29515343282` and full matrix `29515343238`; only built-ins changes by
  **+95 pass / -95 skip**, producing **30064 pass / 6334 fail / 11907 skip /
  12 timeout / 0 error / 48317 total** (**62.2%** of all files, **82.6%** of
  executed files).
- `Promise.all`, `Promise.allSettled`, `Promise.any`, and `Promise.race` now
  close an active input iterator when the receiver's `resolve` call or the
  resolved value's `then` lookup/call completes abruptly. Catchable close
  failures preserve the original throw completion, while observable iterator,
  capability, and rejection values remain rooted across close callbacks and
  GC. The exact Test262 cohort is **12/12**, the supported subset remains
  **12751/0/7687**, tooling is **94/94**, and Rust builtins are **448/448**.
  Feature commit `d0352df` passed CI `29508890153` and full matrix
  `29508890137`; only built-ins changes by **+12 pass / -12 skip**, producing
  **29969 pass / 6334 fail / 12002 skip / 12 timeout / 0 error / 48317 total**
  (**62.0%** of all files, **82.6%** of executed files).
- `%AsyncFromSyncIteratorPrototype%` now completes `next`, `return`, and
  `throw` through intrinsic Promise capabilities and reaction jobs instead of
  synchronously draining the microtask queue during async-generator `yield*`.
  The adapter observes Promise `constructor` access, unwraps iterator values,
  closes unfinished sync iterators on rejected `next`/`throw` values while
  preserving the original rejection, and does not close a second time for
  `return`. Generator wrappers, Realm-local Promises/results, queued resume
  phases, and GC-held rejection state are covered by regressions. The exact
  Test262 corpus is **38/38**, the supported subset remains
  **12751/0/7687**, tooling is **93/93**, and focused async iterator/generator
  tests are **91/91**. Feature commit `a257066` passed CI `29503902319` and
  full matrix `29503902678`; only built-ins changes by **+38 pass / -38
  skip**, producing **29957 pass / 6334 fail / 12014 skip / 12 timeout / 0
  error / 48317 total** (**62.0%** of all files, **82.5%** of executed files).
- Math now exposes the standard non-writable, non-enumerable, configurable
  `Symbol.toStringTag` data property, so direct and borrowed string conversion
  produce `[object Math]` while deletion falls back to `[object Object]`.
  Created Realms receive independent Math objects whose object and native
  function prototypes come from that Realm. Four existing Test262 failures in
  Array `every`/`some`, Math, and String now pass; complete Math and String
  diagnostics are **285/0/42** and **1136/0/87**, the supported subset remains
  **12751/0/7687**, tooling is **92/92**, and Rust builtins are **447/447**.
  Feature commit `01a65a1` passed CI `29497747578` and full matrix
  `29497747662`; only built-ins changes by **+4 pass / -4 fail**, producing
  **29919 pass / 6334 fail / 12052 skip / 12 timeout / 0 error / 48317 total**
  (**61.9%** of all files, **82.5%** of executed files).
- `Array.fromAsync` now implements async-iterable, sync-iterable, and
  array-like collection through a GC-traced Promise continuation state
  machine. It awaits iterator values and mapper results at the required
  boundaries, performs `AsyncIteratorClose` only for the completion classes
  that require it, preserves direct-`next` versus yielded-rejection
  provenance, and creates Promises, arrays, iterator results, and native
  errors in the required Realm without consulting mutable globals. Native
  `[[Set]]` and `CreateDataProperty` writes now invalidate inline caches, and
  `Array.prototype.splice(start)` correctly treats omitted `deleteCount` as
  deletion through the tail. Exact Test262 admission is **95/95**, the
  supported subset remains **12751/0/7687**, tooling is **92/92**, and Rust
  builtins are **446/446**. Feature commit `1a48969` passed CI `29493228430`
  and full matrix `29493228431`. Only built-ins changes: the 95-file cohort
  moves **+95 pass / -4 fail / -91 skip**, while two existing splice and two
  Function/cache tests move from fail to pass, for a total **+99 pass / -8
  fail / -91 skip**. The normalized aggregate is **29915 pass / 6338 fail /
  12052 skip / 12 timeout / 0 error / 48317 total** (**61.9%** of all files,
  **82.5%** of executed files).
- `%AsyncIteratorPrototype%[Symbol.asyncDispose]` is now installed separately
  in every Realm. It creates its result Promise and generated native errors in
  the method Realm, performs dynamic `GetMethod("return")`, invokes the method
  with an empty argument list, assimilates returned thenables through the
  intrinsic `%Promise%`, and resolves to `undefined`; getter, call,
  non-callable, and rejected-return abrupt completions reject instead of
  escaping synchronously. Observable receiver/method/capability/wrapper state
  is pinned across user callbacks and GC. Exact Test262 admission is **9/9**,
  the complete AsyncIteratorPrototype diagnostic is **13/13**, the supported
  subset remains **12751/0/7687**, tooling is **92/92**, and Rust builtins are
  **438/438**. Feature commit `d8c48fa` passed CI `29485564973` and full matrix
  `29485565185`; only built-ins changes by **+9 pass / -9 skip**, producing
  **29816 pass / 6346 fail / 12143 skip / 12 timeout / 0 error / 48317 total**
  (**61.7%** of all files, **82.5%** of executed files).
- Created Realms now own directly rooted `%AsyncIteratorPrototype%`,
  `%AsyncGeneratorPrototype%`, `%AsyncGeneratorFunction%`, and
  `%AsyncGeneratorFunction.prototype%` graphs. Source and dynamic async
  generators, distinct-`NewTarget` and instance fallbacks, native methods,
  internal await assimilation, delayed resume errors, and iterator-result
  objects select the required Realm without consulting mutable globals.
  Borrowed `next`/`return`/`throw` methods return request Promises from the
  method Realm while completion records and native body errors remain in the
  generator Realm. Exact frozen admission adds three cross-Realm files at
  **3/3**; complete diagnostics are **23/23** for
  `AsyncGeneratorFunction` and **48/48** for `AsyncGeneratorPrototype`, the
  supported subset remains **12751/0/7687**, tooling is **91/91**, generators
  are **76/76**, and Rust builtins remain **437/437**. Feature commit
  `7827093` passed CI `29480165633` and full matrix `29480165138`; only
  built-ins changes by **+2 pass / -2 skip**, producing **29807 pass / 6346
  fail / 12152 skip / 12 timeout / 0 error / 48317 total** (**61.7%** of all
  files, **82.4%** of executed files). The third admitted language-expression
  file was already executed by the broader async-generator rule.
- Created Realms now own rooted synchronous `%GeneratorFunction%`,
  `%GeneratorFunction.prototype%`, and `%Generator.prototype%` intrinsic
  graphs. Source and dynamic generator functions, distinct-`NewTarget`
  fallback, generator-instance fallback, native method identities, and
  iterator-result objects select the defining or method Realm. The
  constructor remains rooted after all configurable graph links are deleted,
  and `%GeneratorPrototype%.next.length` is corrected to `1`. Exact frozen
  admission adds three cross-Realm files at **3/3**; all 23
  `built-ins/GeneratorFunction` files pass diagnostically, the generator
  expression subtree is **290/290**, the supported subset remains
  **12751/0/7687**, tooling is **90/90**, generators are **74/74**, and Rust
  builtins remain **437/437**. Feature commit `c768189` exposed a CI-only
  permission assumption while probing an unavailable Test262 checkout; fix
  commit `935df28` makes the optional live-metadata check tolerate filesystem
  lookup errors. Final CI `29475407227` and full matrix `29475407238`
  succeeded. Of the 30 result artifacts, only built-ins changes by **+2 pass /
  -2 skip**, producing **29805 pass / 6346 fail / 12154 skip / 12 timeout / 0
  error / 48317 total** (**61.7%** of all files, **82.4%** of executed files).
  The third admitted language-expression test was already executed by the
  broader generator-prefix rule, so it adds coverage without changing the
  aggregate.
- Created Realms now own rooted `%Promise%` and `%Promise.prototype%`
  intrinsic graphs. Promise construction, species defaults, async functions,
  `await`, dynamic import, resolving/capability functions, combinator result
  containers, and `AggregateError` allocation select the operation Realm
  without consulting mutable global bindings. Forced-GC and cross-Realm
  regressions cover intrinsic identity, internal function prototypes,
  self-resolution errors, `all`/`allSettled`/`any`/`withResolvers` results,
  `for await`, and `Reflect.construct` fallback. Exact Promise Realm Test262
  admission moves one file from skip to pass at **256 pass / 0 fail / 447
  skip**; the supported subset remains **12751/0/7687**, tooling is **89/89**,
  and Rust builtins are **437/437**. Feature commit `6916e03` passed CI
  `29471610323` and full matrix `29471610399`; only built-ins changes by **+1
  pass / -1 skip**, producing **29803 pass / 6346 fail / 12156 skip / 12
  timeout / 0 error / 48317 total** (**61.7%** of all files, **82.4%** of
  executed files).
- `Object.prototype.toString` now boxes primitive receivers, performs
  Proxy-aware `IsArray` before callable fallback, preserves revoked-Proxy
  ordering, and reads `@@toStringTag` from the rooted object. Promise and
  GeneratorFunction prototypes expose their standard tags. Proxy revokers now
  retain the associated Proxy through a traced object slot until first use
  instead of storing an untraced numeric heap index. Exact Object-prototype
  admission grows to 46 files; the complete subtree is **248/248**, broad
  `built-ins/Object` is **3170/120/121**, the supported subset remains
  **12751/0/7687**, and tooling is **88/88**. Feature commit `0d64d5b` passed
  CI `29466781968` and full matrix `29466782034`; six skipped files and the
  existing `Promise.prototype[Symbol.toStringTag]` failure move to pass,
  producing **29802 pass / 6346 fail / 12157 skip / 12 timeout / 0 error /
  48317 total** (**61.7%** of all files, **82.4%** of executed files).
- Every Realm now owns fresh `%Object.prototype%` methods and `__proto__`
  accessors instead of cloning main-Realm function values. Foreign globals,
  `%Function.prototype%`, Error and primitive-wrapper prototypes, TypedArray
  intrinsics, and the Atomics namespace now inherit from that Realm's rooted
  `%Object.prototype%`. Legacy accessor lookup and `isPrototypeOf` use
  Proxy-aware internal prototype operations with fuel accounting, while
  `propertyIsEnumerable` preserves Symbol-valued `ToPropertyKey` results and
  descriptor errors. Exact Test262 admission adds 40 files: the complete
  `Object.prototype` subtree is **242 pass / 0 fail / 6 skip**, broad
  `built-ins/Object` is **3164/120/127**, the supported subset remains
  **12751/0/7687**, and tooling is **88/88**. Feature commit `44ff53f` passed
  CI `29463295702` and full matrix `29463295657`; only built-ins changes by
  **+41 pass / -1 fail / -40 skip**, producing **29795 pass / 6347 fail /
  12163 skip / 12 timeout / 0 error / 48317 total** (**61.7%** of all files,
  **82.4%** of executed files).
- `Object` call and construction now follow the active function and distinct
  `NewTarget` rules without generic receiver preallocation. Nullish values and
  primitive wrappers use the active constructor Realm, object arguments are
  returned only for the intrinsic `NewTarget`, and subclass/`Reflect.construct`
  paths allocate from the observed `NewTarget.prototype`. Constructor Realm
  fallback now follows arbitrarily deep bound/Proxy targets, rejects revoked
  Proxies, normalizes lexical closures to their global Realm, and reads the
  rooted `%Object.prototype%` intrinsic instead of mutable global bindings.
  Exact Test262 admission adds three Object constructor files at **3/3**;
  `built-ins/Object` is **3123 pass / 121 fail / 167 skip**, the supported
  subset remains **12751/0/7687**, and tooling is **87/87**. Commit `436e7ea`
  passed CI `29458827810` and full matrix `29458827839`; only built-ins changes
  by **+3 pass / -3 skip**, producing **29754 pass / 6348 fail / 12203 skip /
  12 timeout / 0 error / 48317 total**.
- Classes now parse and retain restricted decorator expressions and implement
  the audited class/public-element core: source-order evaluation, reverse
  application, method/class replacements, field initializer transforms with
  explicit `this`, computed Symbol context names, and both decorator/export
  placements. Public/private instance/static auto-accessors use hidden backing
  slots and correctly distinguish the contextual `accessor` keyword from
  methods, fields, escapes, and line-terminated forms. An exact 24-file
  Test262 manifest passes **24/24**; current supported coverage is **12751 pass
  / 0 fail / 7687 skip**, and pinned coverage is **12752/0/7687**. Broader
  decorator context/private/auto semantics remain gated and are not claimed.
  CI `29334768817` and full matrix `29334768891` succeeded; the 30 artifacts
  move exactly **+24 pass / -24 skip** to **29056 pass / 6614 fail / 12635
  skip / 12 timeout / 0 error / 48317 total**.
- Public class, method, getter, setter, and field decorators now expose fresh
  `context.access` (`has`/`get`/`set`) and `addInitializer` functions with
  receiver-argument semantics, per-call lifetime enforcement, callable
  validation, and GC-rooted extra-initializer queues. Application follows the
  specified static-method, instance-method, static-field, instance-field,
  class phases; extra initializers run at their instance/static/class
  boundaries with explicit `this`. Class replacements must be constructable
  and rebind the class's inner name before static initialization. The exact
  public-only runtime boundary in pending Test262 PR #5048 passes **201/201**
  at PR head `58b825d0`; current-main exact **24/24** and supported coverage
  **12751/0/7687** remain green without prematurely opening the broad gate.
  Feature commit `4d7dbc9` passed CI `29343897291` and full matrix
  `29343897349`; all 30 result artifacts are byte-for-byte identical to the
  preceding decorator baseline.
- Public decorated auto-accessors now pass `{ get, set }` to each decorator,
  validate and compose optional `get`/`set`/`init` replacements, preserve
  computed Symbol names, and run returned/extra initializers at the specified
  instance and static boundaries. Auto-accessor decorators apply in the
  method/accessor phase while their backing storage initializes with fields.
  Static private methods/accessors are installed before class decorators, so
  class decorators can observe them through public methods. The expanded
  public-only boundary in pending Test262 PR #5048 passes **295/295** at
  `58b825d0`; current-main exact **24/24** and supported coverage
  **12751/0/7687** remain green. Feature commit `7a7531a` passed CI
  `29349296562` and full matrix `29349296827`; all 30 artifacts are identical
  to the preceding baseline.
- Private instance/static field decorators now retain the actual private-name
  identity in `context.access`; `has` performs a brand check, while `get` and
  `set` use the private slot and reject wrong brands. Context names include the
  leading `#`, initializer transforms and extra initializers retain their
  field ordering, and public properties with the same description cannot
  collide. The pending Test262 PR #5048 public-plus-private-field boundary
  passes **363/363** at `58b825d0`; current exact **24/24** and supported
  coverage **12751/0/7687** remain green. Feature commit `d31bbb4` passed CI
  `29353983772` and full matrix `29353983750`; all 30 artifacts are identical
  to the preceding baseline.
- Private instance/static methods, getters, and setters now support decorator
  replacement, private-brand `context.access`, and extra initializers across
  ordinary, async, generator, and async-generator method forms. Mutable
  compiler-internal callable bindings feed both instance initialization and
  static private installation; when a class decorator returns a replacement,
  static private callables are installed on that final class without
  duplicating slots on an identity result. The inner class name observes the
  original constructor while class decorators run and is rebound to the final
  replacement before static fields and blocks. The pending Test262 PR #5048
  public-plus-private-callable diagnostic is **501 pass / 8 fail / 0 skip /
  509 total**; all eight failures are generator assertions whose only blocker
  is the missing global `Iterator` constructor. Current exact **24/24** and
  supported coverage **12751/0/7687** remain green. Feature commit `72fe364`
  passed CI `29359759264` and full matrix `29359759319`; all 30 result
  artifacts are identical to the preceding baseline.
- The realm-specific global `Iterator` is now subclassable but rejects direct
  construction, and `%Iterator.prototype%` provides the specified
  `Symbol.iterator`, `Symbol.dispose`, `constructor`, and
  `Symbol.toStringTag` surface. Generator, Array, Map, Set, and RegExp String
  iterator prototypes inherit through this common base while retaining their
  concrete tags. RegExp String Iterator `next` now also rejects incompatible
  receivers. An exact 24-file Iterator/prototype manifest passes **24/24**;
  all of `built-ins/Iterator` is **23 pass / 0 fail / 491 skip**, and the five
  related concrete-prototype directories are **31 pass / 0 fail / 96 skip**.
  The pending decorator PR boundary now passes **509/509**. Unsupported
  Iterator helper, sequencing, and joint-iteration proposals remain gated
  explicitly instead of running as accidental failures. Feature commits
  `3b6da8a` and `5a9ff6f` passed CI `29364732026` and full matrix
  `29364732182`; only the built-ins result changed, producing **29085 pass /
  6495 fail / 12725 skip / 12 timeout / 0 error / 48317 total**.
- Primitive and boxed strings now expose the Realm-specific
  `String.prototype[Symbol.iterator]` and `%StringIteratorPrototype%` through
  the public iterator protocol instead of an internal snapshot fallback.
  The branded iterator preserves UTF-16 surrogate representation, has the
  specified ancestry, tag, descriptors, exhaustion, and extensibility, and
  observes method replacement/deletion and boxed-string coercion. Realm
  prototypes, native errors, and cached `next` methods remain live across GC.
  The exact Iterator manifest expands from 24 to **37/37**, covering all 13
  current String iterator files. Commit `4af8c31` passed CI `29381725859` and
  full matrix `29381725849`; 29 artifacts are unchanged and built-ins moves
  exactly **+13 pass / -13 skip**, producing **29098 pass / 6495 fail / 12712
  skip / 12 timeout / 0 error / 48317 total**.
- `Iterator.from` now accepts iterable and direct iterator inputs, preserves
  intrinsic Iterator instances, and otherwise returns a Realm-specific
  branded wrapper with cached `next` and dynamic `return` forwarding.
  `Iterator.prototype.toArray` is the first eager helper, with cached `next`,
  specified `done`/`value` ordering, Realm-correct Array allocation, GC-safe
  iterator results, and bounded materialization with iterator cleanup.
  Supporting `Array.from` iterable/generic-constructor ordering, mapping,
  abrupt IteratorClose, primitive iterators, and array-like property reads are
  corrected. The exact Iterator manifest expands to **74/74**. Commit
  `a6d3949` passed CI `29386623291` and full matrix `29386623314`; only
  built-ins changes by **+46 pass / -9 fail / -37 skip**, producing **29144
  pass / 6486 fail / 12675 skip / 12 timeout / 0 error / 48317 total**.
- `Iterator.prototype.map` and `filter` now return lazy, branded,
  Realm-specific Iterator Helper objects. They cache the source `next`, defer
  callback execution until stepping, preserve `done`/`value` ordering and
  callback indices, close dynamically on callback failures, distinguish all
  helper suspension states during reentrant close, and preserve non-catchable
  fuel aborts. GC tracing, helper integrity operations, `Iterator Helper`
  tagging, Realm-specific results/errors, and exact mathematical counters are
  covered by regressions. The exact Iterator manifest expands to **147/147**,
  including
  all **36 map** and **37 filter** files. Commit `8c911a5` passed CI
  `29390731665` and full matrix `29390731676`; only built-ins changes by
  **+73 pass / -73 skip**, producing **29217 pass / 6486 fail / 12602 skip /
  12 timeout / 0 error / 48317 total**.
- `Iterator.prototype.take` and `drop` now use the same branded,
  Realm-specific lazy helper machinery. Exact `BigUint` limits preserve large
  finite values, infinity remains unbounded, limit conversion closes the
  provisional iterator on abrupt or invalid input, `take` closes at its
  boundary, and `drop` does not read skipped values. Native helper loops now
  charge VM fuel, and radix-prefixed string-to-number conversion accepts large
  valid inputs while rejecting non-JavaScript spellings. The exact Iterator
  manifest expands to **214/214**, including all **33 take** and **34 drop**
  files. Commit `d36456b` passed CI `29396353550` and full matrix
  `29396353596`; only built-ins changes by **+67 pass / -67 skip**, producing
  **29284 pass / 6486 fail / 12535 skip / 12 timeout / 0 error / 48317
  total**.
- `Iterator.prototype.flatMap` now lazily drains one mapper-produced iterator
  at a time through the branded Realm-specific helper machinery. Inner
  iterators cache `next`, remain GC-rooted across stepping and close, reject
  primitive mapper results, preserve mapper and inner-step errors while
  closing the outer source, and close inner before outer on explicit return.
  Reentrant running-state calls and native-loop fuel are covered by Rust
  regressions. The exact Iterator manifest expands to **258/258** with all
  **44 flatMap** files. Commit `043852b` passed CI `29401085165` and full
  matrix `29401085162`; only built-ins changes by **+44 pass / -44 skip**,
  producing **29328 pass / 6486 fail / 12491 skip / 12 timeout / 0 error /
  48317 total**.
- `Iterator.prototype.reduce` now eagerly consumes direct iterators with
  cached `next`, omitted-versus-explicit initial-value handling, specified
  callback indices, method-Realm errors, native-loop fuel, and GC-rooted
  accumulator replacement. Iterator-step errors propagate directly while
  reducer errors close the source and preserve the original abrupt completion.
  The exact Iterator manifest expands to **288/288** with all **30 reduce**
  files. Commit `92ce768` passed CI `29405803022` and full matrix
  `29405803115`; only built-ins changes by **+30 pass / -30 skip**, producing
  **29358 pass / 6486 fail / 12461 skip / 12 timeout / 0 error / 48317
  total**.
- `Iterator.prototype.forEach` now eagerly consumes direct iterators with a
  cached `next`, `(value, index)` callbacks, undefined results, method-Realm
  errors, native-loop fuel, and GC-rooted object values. Iterator-step errors
  propagate directly while callback errors close the source and preserve the
  original abrupt completion. The exact Iterator manifest expands to
  **315/315** with all **27 forEach** files. Commit `20dc605` passed CI
  `29410076808` and full matrix `29410076834`; only built-ins changes by
  **+27 pass / -27 skip**, producing **29385 pass / 6486 fail / 12434 skip /
  12 timeout / 0 error / 48317 total**.
- `Iterator.prototype.some` now eagerly consumes direct iterators with cached
  `next`, exact mathematical callback indices, method-Realm errors, native-loop
  fuel, and GC-rooted object values. Exhaustion does not close; predicate
  failures perform abrupt close preserving the original error; truthy results
  perform normal close and propagate close failures. Lazy `map`/`filter`/
  `flatMap` and eager `reduce`/`forEach` counters now share exact `BigUint`
  semantics without the previous non-spec safe-integer cap. The exact Iterator
  manifest expands to **348/348** with all **33 some** files. Commit `74f540b`
  passed CI `29415121304` and full matrix `29415121337`; only built-ins changes
  by **+33 pass / -33 skip**, producing **29418 pass / 6486 fail / 12401 skip /
  12 timeout / 0 error / 48317 total**.
- `Iterator.prototype.every` now eagerly consumes direct iterators with cached
  `next`, exact mathematical callback indices, method-Realm errors, native-loop
  fuel, and GC-rooted object values. Exhaustion returns true without closing;
  predicate failures preserve the original abrupt completion while closing;
  falsey results perform normal close and propagate close failures. Rust
  regressions cover zero-close step errors and the full close/Realm matrix.
  The exact Iterator manifest expands to **381/381** with all **33 every**
  files. Commit `fdd6223` passed CI `29420614915` and full matrix
  `29420614912`; only built-ins changes by **+33 pass / -33 skip**, producing
  **29451 pass / 6486 fail / 12368 skip / 12 timeout / 0 error / 48317 total**.
- `Iterator.prototype.find` now eagerly consumes direct iterators with cached
  `next`, exact mathematical callback indices, native-loop fuel, and
  method-Realm errors. It returns the original first matching value, keeping
  object results rooted through normal close; exhaustion returns undefined
  without closing. Predicate failures preserve the original abrupt completion,
  while matches propagate normal-close failures. The exact Iterator manifest
  expands to **413/413** with all **32 find** files, closing every current
  synchronous `%Iterator.prototype%` helper directory. Commit `2bbe8e7` passed
  CI `29426108186` and full matrix `29426108093`; only built-ins changes by
  **+32 pass / -32 skip**, producing **29483 pass / 6486 fail / 12336 skip /
  12 timeout / 0 error / 48317 total**.
- Static `Iterator.concat` now validates object arguments and caches their
  iterator methods left-to-right, then lazily opens and drains each iterator.
  Only the active iterator is closed by `return`; pre-start return and natural
  exhaustion do not open or close sources, while opener and step failures
  complete without opening later sources. Iterator Helpers now retain their
  creation Realm so yielded results and resumed protocol errors use it, while
  terminal results and direct validation use the borrowed method Realm. Exact
  admission expands to **445/445** with all **32 concat** files. Commit
  `decf8d5` passed CI `29432285167` and full matrix `29432285229`; only
  built-ins changes by **+32 pass / -32 skip**, producing **29515 pass / 6486
  fail / 12304 skip / 12 timeout / 0 error / 48317 total**.
- Static `Iterator.zip` now eagerly opens and caches input iterators, then
  produces fresh Realm-specific tuple arrays in `shortest`, `longest`, and
  `strict` modes. Construction observes options before inputs, materializes
  per-input padding, and applies reverse `IteratorCloseAll` ordering with the
  specified abrupt-completion priority. Open records and padding are
  GC-traced, wide native scans charge fuel, and a fuel-aborted `return()`
  cannot leave the helper executing. `Array.prototype.fill` now materializes
  sparse holes, fixing the array inputs used by the joint-iteration tests.
  Exact Iterator admission expands to **483/483** with all **38 zip** files;
  all of `built-ins/Iterator` is **469 pass / 0 fail / 45 skip**, and the
  supported subset remains **12751 pass / 0 fail / 7687 skip / 20438 total**.
  Feature commit `6f8c291` passed CI `29438450582` and full matrix
  `29438450881`; built-ins moves by exactly **+39 pass / -1 fail / -38 skip**,
  including the `fill` correction. The normalized aggregate is **29554 pass /
  6485 fail / 12266 skip / 12 timeout / 0 error / 48317 total**.
- Static `Iterator.zipKeyed` now snapshots eligible own enumerable string and
  Symbol keys, rechecks descriptors, and yields fresh null-prototype records in
  `shortest`, `longest`, and `strict` modes. Longest padding uses keyed property
  reads, and all setup, reverse-close, Realm, GC, reentrancy, and fuel behavior
  shares the audited zip helper machinery. Proxy `ownKeys` now validates trap
  result key types, duplicates, target invariants, and filtering in specification
  order; the public own-key consumers and Object descriptor operations preserve
  symbols, validation order, atomic conversion, SameValue invariants, and GC
  roots. Exact admission adds all **44 zipKeyed** and **40 Proxy/Reflect
  ownKeys** files, expanding the Iterator manifest to **527/527**. Feature
  commit `93de368`, CI-checkout fix `5339ba3`, and unsupported `IsHTMLDDA`
  classification fix `e59bc43` passed CI `29448974479` and full matrix
  `29448974428`. The final aggregate is **29751 pass / 6348 fail / 12206 skip /
  12 timeout / 0 error / 48317 total**, or **61.6%** of all files and **82.4%**
  of executed files.
- Foreign Realm `Object` constructors now own distinct copies of all 23 static
  methods with their Realm's `%Function.prototype%` and native error Realm.
  `keys`/`values`/`entries`, own-key lists, `groupBy`, `fromEntries`, and
  descriptor helpers allocate Arrays and ordinary objects from the method
  Realm; Proxy descriptor normalization and primitive boxing follow the same
  rule. Iterator records, accumulated values, entry getter results, keys, and
  descriptor objects remain rooted across user callbacks, coercion, and forced
  GC. Commit `6dad4e3` passed CI `29454210263` and full matrix `29454210292`;
  all 30 result artifacts are unchanged at **29751 pass / 6348 fail / 12206
  skip / 12 timeout / 0 error / 48317 total** because current upstream
  Test262 has no direct Object-static method-Realm case.
- Private instance and static auto-accessor decorators now expose the private
  name and branded `access` functions while keeping their synthetic backing
  storage internal. Optional `get`, `set`, and `init` replacements, extra
  initializers, source-order backing initialization, static class replacement,
  GC retention, and Realm-specific errors are covered. The complete pending
  Test262 decorator PR #5048 now passes **657/657**; its files remain outside
  the current-main admission, which stays **24/24**, while the supported subset
  remains **12751 pass / 0 fail / 7687 skip / 20438 total**. Commit `e7cdaf2`
  passed CI `29378359326` and full matrix `29378359319`; all 30 result artifacts
  are byte-for-byte identical to the preceding baseline, retaining **29085
  pass / 6495 fail / 12725 skip / 12 timeout / 0 error / 48317 total**.
- Decorated computed class elements no longer let `@decorators[0]` escape the
  restricted decorator grammar by being reparsed as an initializer-free
  computed field followed by an ASI boundary. The parser rejects only that
  ambiguous field fallback while preserving decorated computed methods,
  initialized fields, parenthesized computed decorator expressions, and
  computed arguments to decorator calls. Both pending Test262 early-error
  files now pass; the complete PR #5048 diagnostic is **569 pass / 88 fail /
  0 skip / 657 total**, with every remaining failure confined to private
  auto-accessor decorators. Commit `1981ee2` passed CI `29374930032` and full
  matrix `29374930033`; all 30 result artifacts are byte-for-byte identical to
  the preceding baseline.
- The final value-producing `LoadEnvName` bytecode and its duplicate VM
  environment resolver have been removed. Switch `continue` completion-value
  propagation now emits `LoadRef` plus `GetValue`, so all executable
  identifier, member, private, and super reads/calls/assignments/updates/
  deletes use the shared specification Reference-record machinery. The
  focused Reference/`with`/compound cluster passes **663/663** with one
  feature skip, switch statements pass **69/69**, and the supported subset
  remains **12751/12751**. Switch completion bindings are now unique per
  nested switch, completion stores stay stack-balanced, and break/continue
  scope unwinding resumes only after all enclosing `finally` bodies. The
  unwind trampoline preserves same-loop `for...of` IteratorClose semantics.
  Commits `00994c7` and `bbfa6f2` passed CI `29370269695` and full matrix
  `29370269812`; all 30 Test262 result artifacts are unchanged.
- Catch handlers now restore the operand stack depth captured at `try` entry
  before pushing the thrown value. This prevents failed decorator and other
  callback operands from remaining as hidden GC roots after a caught throw;
  the relative depth survives async/generator suspension. Field decorator
  initializers also run in source order after reverse-order decorator calls.

- Test262 now admits the exact **272** generated class declaration/expression
  files whose sole remaining gate was `destructuring-binding`. The frozen set
  covers 136 ordinary and static method parameter-pattern cases in both class
  forms. Runner and analyzer retain all broad gates and remove only the single
  feature for exact manifest members. The manifest passes **272/272** and
  raises pinned supported coverage to **12728 pass / 0 fail / 7711 skip /
  20439 total**. At commit `3fe3343`, CI `29326685124` and full matrix
  `29326685148` succeeded; expressions and statements each moved by **+136
  pass / -136 skip**, producing **29032 pass / 6614 fail / 12659 skip / 12
  timeout**.
- Object, public class, and private class accessors now enforce their grammar's
  formal-parameter arity after parsing: getters accept no parameters, and
  setters accept exactly one non-rest parameter. Valid setter defaults and
  destructuring remain supported. This fixes two class and one object-literal
  Test262 parse-negative failures. An exact manifest admits the **56** class
  files whose sole remaining gate was `default-parameters`; it passes **56/56**
  and raises pinned supported coverage to **12456 pass / 0 fail / 7983 skip /
  20439 total**. At commit `99db3dc`, CI `29322169799` and full matrix
  `29322169773` succeeded; expressions and statements each moved by **+28 pass
  / -28 skip**, producing **28760 pass / 6614 fail / 12931 skip / 12
  timeout**.
- Private getter/setter duplicate-name early errors now require matching
  instance/static placement. The parser rejects all four mismatched accessor
  orders while retaining valid complementary pairs in either order, escaped
  private-name identity, and per-class private environments. Test262 admission
  is frozen to the exact **37** private class files exposed by this boundary;
  runner and analyzer remove only each path's recorded private feature gates.
  The manifest passes **37/37**, and pinned supported coverage is **12400 pass
  / 0 fail / 8039 skip / 20439 total**. At commit `cf398c0`, CI
  `29316245920` and full matrix `29316245922` succeeded; expressions moved by
  **+32 pass / -32 skip** and statements by **+5 pass / -5 skip**, producing
  **28704 pass / 6614 fail / 12987 skip / 12 timeout**.
- Test262 now admits the final **5** files whose only unsupported blocker was a
  public or static class-field gate: derived-constructor `this` binding during
  instance-field initialization, computed field-name abrupt completion, and
  static field/static-block ordering and abrupt completion. The files use a
  separate exact manifest; both broad gates remain frozen against future
  upstream expansion. Pinned supported coverage is **12363 pass / 0 fail /
  8076 skip**. At commit `9c5a2c2`, CI `29307597382` and full matrix
  `29307597365` succeeded; expressions moved by **+1 pass / -1 skip** and
  statements by **+4 pass / -4 skip**, producing **28667 pass / 6614 fail /
  13024 skip / 12 timeout**.
- Test262 now admits an exact frozen set of **120** generated computed public
  and static class-field files across declaration/expression and field-only /
  field-plus-method families. Runner and analyzer share exact manifest
  membership and remove only the two public-field feature gates; four
  top-level-`await` module siblings and unrelated class files remain skipped.
  Pinned supported coverage rises to **12358 pass / 0 fail / 8081 skip**. At
  commit `31013cc`, CI `29303864482` and full matrix `29303864484` succeeded;
  exactly the expressions and statements shards moved by **+60 pass / -60
  skip** each, producing **28662 pass / 6614 fail / 13029 skip / 12 timeout**.
- Environment References now preserve the exact declarative record selected by
  identifier resolution through `PutValue`, including bindings deleted during
  RHS evaluation. Active `with` objects are traced by GC, and global `var`
  reads, writes, accessors, descriptor failures, and property deletion use the
  correct Realm global object without stale binding mirrors. Compiler-internal
  `StoreEnvName` writes resolve their target before entering this exact-base
  path. Focused Reference Test262 is **1426 pass / 0 fail / 27 skip**, the
  pinned supported subset remains **12238 pass / 0 fail / 8201 skip**, and the
  current Annex B diagnostic is **206 pass / 830 fail / 50 skip**. At follow-up
  commit `db0e5a9`, CI `29301189893` and full matrix `29301189900` succeeded;
  all 30 result artifacts exactly match the preceding confirmed matrix.
- RegExp literals now use a dedicated bytecode operation that constructs from
  the executing Realm's original `%RegExp.prototype%` instead of resolving a
  mutable lexical or global `RegExp` binding. Realm prototypes are retained as
  GC roots, and interpreted-frame selection remains correct when native array,
  generator, async, or async-generator operations re-enter foreign code. The
  obsolete `LoadGlobal` bytecode is removed. Literal Test262 is **474 pass / 0
  fail / 60 skip**, the supported subset remains **12238 pass / 0 fail / 8201
  skip**, and the full aggregate is unchanged at commit `953a821`. CI
  `29295535589` and full matrix `29295535579` succeeded; all 30 result
  artifacts exactly match the preceding matrix.
- Direct `super` assignment targets in `for-in` and `for-of` now create a
  super property Reference with distinct base and actual-this components
  instead of an ordinary property Reference rooted at the home prototype.
  Setters and Proxy traps therefore receive the loop method's object,
  primitive, or constructor receiver. Regressions cover computed keys,
  iteration counts, dynamic super-base changes, static methods, IteratorClose
  error priority, and key/coercion/Proxy GC. Focused Test262 is **684 pass / 0
  fail / 190 skip**, and the supported subset remains **12238 pass / 0 fail /
  8201 skip** at commit `b395956`. CI `29292458497` and full matrix
  `29292458431` succeeded; all 30 artifacts exactly match the preceding matrix.
- Parenthesized optional-chain tagged templates now preserve the final member
  or private Reference and call through `CallRef`; optional-chain call results
  remain unbound through an explicit `undefined` receiver. This restores the
  tag receiver, callee snapshot, nullish/error ordering, and GC rooting for
  member, computed, private, nested, and call-result forms while direct
  optional-chain tags remain syntax errors. Focused Test262 is **63 pass / 0
  fail / 2 skip**, and the supported subset remains **12238 pass / 0 fail /
  8201 skip** at commit `e9faed5`. CI `29289456680` and full matrix
  `29289456698` succeeded; all 30 artifacts exactly match the preceding matrix.
- Private prefix/postfix updates and compound/logical assignments now retain a
  single private Reference through `GetValue`, coercion or RHS evaluation, and
  `PutValue`. This unifies their base, brand, accessor, mutation, error-Realm,
  and GC semantics with other assignment targets and removes the duplicate
  `GetPrivate`/`SetPrivate` bytecodes. Regressions cover all eight Number/BigInt
  update forms, coercion and RHS mutation, short-circuiting, readonly and
  arithmetic errors, wrong brands, foreign Realms, and forced GC. Related
  Test262 is **532/532**, class Test262 is **1672 pass / 0 fail / 2387 skip**,
  and the supported subset remains **12238 pass / 0 fail / 8201 skip** at
  commit `17de0d9`. CI `29285326599` and full matrix `29285326527` succeeded;
  all 30 result artifacts exactly match the preceding matrix.
- Ordinary, optional, grouped, and spread private calls now retain one private
  Reference through `GetValue`, argument evaluation, and `CallRef`. Private
  brand checks and accessor getters therefore run before arguments, the callee
  is snapshotted before argument-side mutation, callable private accessors are
  supported, and temporary receivers remain rooted through argument GC. This
  also removes the duplicate `CallPrivateMethod` bytecode and its spread stack
  corruption. Class Test262 is **1672 pass / 0 fail / 2387 skip**, and the
  supported subset remains **12238 pass / 0 fail / 8201 skip** at commit
  `0dd6dfc`. CI `29281286232` and full matrix `29281286160` succeeded; all 30
  result artifacts are byte-for-byte identical to the preceding matrix.
- Sloppy identifier deletion now evaluates one identifier Reference and routes
  it through the same `DeleteValue` operation as property deletion. This
  preserves declarative and eval binding deletion, targets the correct foreign
  Realm global object, and keeps a global lexical binding from accidentally
  deleting a configurable property it shadows. `with` object References remain
  rooted through Proxy delete traps and GC. The duplicate `DeleteVar` bytecode
  and manual environment walk are removed. Focused Test262 delete/with coverage
  is **250/250**, and the supported subset remains **12238 pass / 0 fail / 8201
  skip** at commit `0c2f783`. CI `29277397932` and full matrix `29277398192`
  succeeded; all 30 artifacts are byte-for-byte identical to the preceding
  matrix.
- Interpreted runtime errors are now materialized in the Realm of the frame
  that raised them before that frame is caught, suspended, or removed. This
  preserves foreign `Error` prototypes through nested native callbacks,
  generator parameter/body execution, async and async-generator resumes,
  module evaluation, and frame-boundary GC without changing native-function
  Realm selection or explicit thrown-value identity. Six private method,
  getter, and setter brand-check Realm tests are newly admitted. The pinned
  supported subset is **12238 pass / 0 fail / 8201 skip** at commit `c66fc1e`.
  CI `29273287748` and full matrix `29273287842` succeeded; exactly the
  expressions shard moved by **+6 pass / -6 skip**, producing **28542 pass /
  6614 fail / 13149 skip / 12 timeout / 0 error** overall.
- Test262 runner and analyzer now give four known slow legacy RegExp literal
  stress files a bounded 20-second timeout instead of the ordinary 8 seconds.
  They take **3.8-6.3 seconds** locally and repeatedly timed out only under
  concurrent CI load. All other ordinary tests retain the 8-second limit, and
  the existing TypedArray stress exception remains separate. Tooling is
  **67/67** and the literals shard is **474 pass / 0 fail / 60 skip / 0
  timeout** at commit `d6b142f`. CI `29269378405` and full matrix
  `29269378378` succeeded without a literals retry; all 30 artifacts are
  byte-for-byte identical to the preceding confirmed matrix.
- Removed the unreachable legacy member-assignment fallback and its
  `SetProp`/`SetElem` bytecodes. All valid simple, compound, logical,
  prefix/postfix, destructuring, and loop member targets now have only their
  retained Reference paths; parenthesized computed targets are covered across
  read-modify-write forms. Focused Test262 assignment/compound/logical coverage
  is **1017 pass / 0 fail / 0 skip**, and the supported subset remains **12232
  pass / 0 fail / 8207 skip** at commit `ea8e492`. CI `29266489197` and full
  matrix `29266489031` succeeded; all 30 result artifacts are byte-for-byte
  identical to the preceding confirmed matrix.
- Direct and optional-chain property delete now retain a raw Reference and use
  a specification-ordered `DeleteValue`: box/check the base before coercing the
  referenced name, invoke `[[Delete]]`, and apply the Reference's strict flag.
  This fixes non-configurable String wrapper indices, roots temporary bases and
  keys across coercion and Proxy traps, and removes the legacy `DeleteProp`
  opcode. String exotic reads/has/delete now reject non-canonical index names
  such as `"01"` and `"+0"`. Proxy delete invariants now invoke nested target
  `[[GetOwnProperty]]` and `[[IsExtensible]]` operations. Focused Test262 is
  **107 pass / 0 fail / 0 skip**, and the supported subset remains **12232 pass
  / 0 fail / 8207 skip** at commit `cba970d`. CI `29263990433` and full matrix
  `29263989422` succeeded; after retrying one transient literals timeout, all
  30 result artifacts are byte-for-byte identical to the preceding matrix.
- Ordinary member targets in simple assignment, destructuring assignment, and
  `for-in`/`for-of` assignment now retain one raw property Reference through
  the value or source evaluation. `ToPropertyKey` remains deferred until
  `PutValue`, while the Reference roots temporary base and key values across
  observable calls and GC. The assignment-only `MakePropertyRefForSet` opcode
  and separate destructuring base/key temporaries are removed. Focused Test262
  is **1169 pass / 0 fail / 190 skip**, and the supported subset remains
  **12232 pass / 0 fail / 8207 skip** at commit `345e3f3`. CI `29260495441`
  and full matrix `29260497188` succeeded; all 30 result artifacts are
  byte-for-byte identical to the preceding confirmed matrix.
- Every `super` property write path now uses the same specification Reference
  machinery as reads and calls: simple and destructuring assignment,
  compound/logical assignment, prefix/postfix update, and delete. Computed
  super References retain an uncoerced referenced name so simple assignment
  evaluates its RHS before `ToPropertyKey`, null bases reject before coercion,
  and delete throws `ReferenceError` without coercion. Read-modify-write forms
  use `ResolvePropertyRef` to coerce exactly once before one `GetValue` and one
  `PutValue`; writes use the Reference's distinct actual-this receiver.
  Deferred names are traced through stack, environment, and temporary pin
  roots. The obsolete `GetSuperProp` and `SetSuperProp` opcodes are removed.
  Focused Test262 is **1299 pass / 0 fail / 23 skip**, and the supported subset
  remains **12232 pass / 0 fail / 8207 skip** at commit `e0bd2a4`. CI
  `29257232329` and full matrix `29257232453` succeeded; all 30 result artifacts
  are byte-for-byte identical to the preceding confirmed matrix.
- `super` property reads, direct/spread/optional calls, and tagged templates
  now create specification Reference records with distinct `[[Base]]` and
  `[[ThisValue]]` components. Computed keys run before the dynamic super-base
  lookup and before property-key coercion, null super bases throw for string
  and Symbol keys, and the retained Reference roots both base and receiver
  across getters, Proxy traps, arguments, spread, interpolation, and forced
  GC. Concise methods and accessors now retain an immutable `[[HomeObject]]`,
  including borrowed primitive receivers and nested methods, without changing
  it when the function is copied to another property. The obsolete
  `CallSuper` opcode is removed. Local focused Test262 is **214 pass / 0 fail /
  45 skip**, and the supported subset remains **12232 pass / 0 fail / 8207
  skip**. CI `29252936209` and `test262-full` `29252935590` pass for commit
  `aa83f7e`; all 30 artifacts are byte-for-byte identical to the preceding
  baseline and retain the normalized **28536 pass / 6614 fail / 13155 skip / 12
  timeout / 0 error / 48317 total / 35150 pass-or-fail executed** aggregate.
- Tagged templates now classify their tag call target explicitly: identifier,
  member, and private tags retain a Reference and call through `CallRef`;
  `super` tags retain the actual derived-instance receiver through
  `CallThis`; non-Reference expression tags remain unbound. This fixes strict
  identifier tags inside `with` receiving `undefined` instead of the with
  object and fixes `super` tagged templates losing the derived receiver. Tag getters run
  before template-object/interpolation evaluation, and temporary bases and
  returned functions remain rooted across interpolation GC. The obsolete
  `GetMethodForCall` opcode is removed. CI `29247565071` and `test262-full`
  `29247565062` pass for commit `4f6975f`; all 30 artifacts reproduce **28536
  pass / 6614 fail / 13155 skip / 12 timeout / 0 error / 48317 total / 35150
  pass-or-fail executed** with no shard movement.
- Optional member calls now retain property References across all four call
  shapes: `o?.m()`, `o.m?.()`, `(o?.m)()`, and `(o?.m)?.()`, including
  computed keys and spread arguments. Base-nullish and callee-nullish exits
  discard the exact live stack values before skipping key or argument
  evaluation, while grouped non-optional calls still evaluate arguments before
  throwing for an undefined callee. Proxy, primitive, and temporary GC-only
  bases preserve the original receiver. CI `29244031712` and `test262-full`
  `29244032070` pass for commit `81b50cf`; all 30 artifacts reproduce **28536
  pass / 6614 fail / 13155 skip / 12 timeout / 0 error / 48317 total / 35150
  pass-or-fail executed** with no shard movement.
- Ordinary non-optional member calls, including spread calls, now retain the
  property Reference used to resolve the callee and derive `this` through
  `CallRef`/`CallRefSpread`. Object, primitive, Symbol-keyed, and Proxy-backed
  bases therefore share the same Reference semantics as member value reads,
  with one property lookup and a rooted base throughout observable argument
  evaluation. Optional, private, tagged-template, and `super` calls remain on
  their existing bounded paths. CI `29240428689` and `test262-full`
  `29240428617` pass for commit `9686f03`; all 30 artifacts reproduce **28536
  pass / 6614 fail / 13155 skip / 12 timeout / 0 error / 48317 total / 35150
  pass-or-fail executed** with no shard movement.
- Ordinary string- and Symbol-keyed member reads, including optional-chain
  reads, now create property Reference records and resolve them through
  `GetValue`. Proxy `[[Get]]` forwarding preserves the receiver through nested
  proxies and `Reflect.get`, validates fixed data/accessor invariants, treats a
  null trap as absent, and observes Proxy-aware target descriptors. String
  exotic values and all values held across observable trap/descriptor work are
  preserved correctly. The frozen Proxy/Reflect `[[Get]]` admission is
  **30/30**. A follow-up excludes mapped arguments exotic objects from the
  own-data read cache so parameter-map values remain live after failed deletes.
  CI `29236604702` and `test262-full` `29236604723` pass for follow-up commit
  `50b84f8`; downloaded artifacts aggregate to **28536 pass / 6614 fail /
  13155 skip / 12 timeout / 0 error / 48317 total / 35150 pass-or-fail
  executed**, a movement of **+30 pass / -30 skip**.
- Primitive-base property References now select the current execution Realm's
  global object and String, Number, Boolean, BigInt, and Symbol prototypes for
  `GetValue`, `PutValue`, and `ToObject`. Primitive writes invoke inherited
  setters and Proxy `set` traps with the original primitive receiver, including
  cross-Realm eval. Child Realms now have independent Object, BigInt, and Symbol
  intrinsics, and Proxy/ordinary `[[Set]]` traversal shares cycle detection
  while retaining separate safe depth budgets. Frozen primitive Reference
  admission is **3/3** and the combined Reference/with/compound gate is **663
  pass / 0 fail / 1 skip**. CI `29224760629` and `test262-full` `29224760619`
  pass for follow-up commit `5f78f18`; downloaded artifacts aggregate to
  **28506 pass / 6614 fail / 13185 skip / 12 timeout / 0 error / 48317 total /
  35120 pass-or-fail executed**, a movement of **+4 pass / -1 fail / -3 skip**.
- Ordinary identifier reads now compile to `LoadRef` followed by `GetValue`,
  routing lexical, import, global-object, TDZ, and `with` object-environment
  resolution through the same specification Reference-record path used by
  assignment and update expressions. Direct calls and direct `eval` retain
  their dedicated Reference-preserving call opcodes. The combined
  `language/types/reference`, `language/statements/with`, and
  `language/expressions/compound-assignment` gate is **660 pass / 0 fail / 4
  skip**. CI `29220379603` and `test262-full` `29220379613` pass for commit
  `f63145d`; downloaded artifacts reproduce **28502 pass / 6615 fail / 13188
  skip / 12 timeout / 0 error / 48317 total / 35117 pass-or-fail executed**.
- `Date.prototype[Symbol.toPrimitive]` now implements the generic ordinary
  conversion algorithm with string-first `"default"`/`"string"` hints and a
  number-first `"number"` hint, including observable property access, callable
  Proxies, abrupt completion, exact intrinsic metadata, and primitive-result
  validation. Removing the configurable intrinsic now exposes ordinary
  number-first default conversion instead of a stale Date-specific VM path.
  Frozen Test262 admission is **18/18**. CI `29218430070` and `test262-full`
  `29218430199` pass for commit `8939b5e`; downloaded artifacts aggregate to
  **28502 pass / 6615 fail / 13188 skip / 12 timeout / 0 error / 48317 total /
  35117 pass-or-fail executed**, a movement of **+15 pass / -14 fail / -1
  skip**.
- `JSON.rawJSON` and `JSON.isRawJSON` now create and recognize unforgeably
  branded, frozen, null-prototype raw-value objects. Primitive JSON text is
  validated after `ToString` while preserving the original spelling, including
  large integers and escaped lone surrogates, and `JSON.stringify` emits branded
  values verbatim after normal `toJSON` and replacer processing. The JSON
  intrinsic also exposes the specified `Symbol.toStringTag`. Frozen Test262
  admission for raw values and branding is **17/17**, and the complete
  `built-ins/JSON` directory is **165/165**. CI `29215737017` and
  `test262-full` `29215737023` pass for commit `196e8fd`; downloaded artifacts
  aggregate to **28487 pass / 6629 fail / 13189 skip / 12 timeout / 0 error /
  48317 total / 35116 pass-or-fail executed**, a movement of **+19 pass / -17
  fail / -2 skip**.
- `JSON.stringify` now follows a holder-based, fallible
  `SerializeJSONProperty` pipeline. Ordinary property access, `toJSON(key)`,
  replacer `this`, abrupt completion, transformed-value circularity, callable
  Proxies, wrapper coercion, replacer PropertyList ordering, `space` coercion,
  and UTF-16 JSON quoting are observable in specification order. Callback
  replacements remain GC-rooted throughout recursive serialization. The
  frozen `built-ins/JSON/stringify` admission is **66/66**. CI `29213833296`
  and `test262-full` `29213833314` pass for commit `ac708fd`; downloaded
  artifacts aggregate to **28468 pass / 6646 fail / 13191 skip / 12 timeout /
  0 error / 48317 total / 35114 pass-or-fail executed**. The exact stringify
  slice moves **+45 pass / -30 fail / -15 skip**; one unrelated built-ins result
  varied between full runs, making the aggregate delta **+44 / -29 / -15**.
- `JSON.parse` now performs the specification's holder-based
  `InternalizeJSONProperty` walk in place. Reviver calls observe forward
  mutations, inherited properties, array-length snapshots, the root wrapper,
  deletion and create-data-property failures, callable and revoked Proxies,
  and the original primitive token through `context.source`. Proxy-aware
  `Array.isArray` and `[[DefineOwnProperty]]` invariant checks support the same
  path without weakening non-extensible or non-configurable targets. The
  frozen `built-ins/JSON/parse` admission is **77/77**. CI `29211878288` and
  `test262-full` `29211878312` pass for commit `caa689c`; downloaded artifacts
  aggregate to **28424 pass / 6675 fail / 13206 skip / 12 timeout / 0 error /
  48317 total / 35099 pass-or-fail executed**, a matrix movement of **+26 pass
  / -14 fail / -12 skip**.
- The `in` operator now preserves Symbol property keys instead of stringifying
  them, so well-known and user-created Symbol properties participate in
  `[[HasProperty]]` correctly across ordinary and exotic objects.
- File-backed ES Module graphs now parse and link side-effect and named
  imports, local/named/star exports, dependency-first evaluation, immutable
  live import bindings, abrupt dependency completion, and a canonical-path
  Realm cache. Module declaration instantiation is now separated from
  evaluation, so cyclic graphs expose hoisted functions, preserve lexical TDZ,
  and cache one abrupt evaluation result across every member of the cycle.
  Default imports, default function/class declarations and expressions, mixed
  default-plus-named imports, and default re-exports share the same live-binding
  linker path. Namespace imports and `export * as` now produce canonical,
  null-prototype Module Namespace Exotic Objects with sorted keys, live export
  descriptors, TDZ behavior, immutable assignment semantics, and correct
  cyclic/ambiguous-star identity.
- Module import/export specifiers now accept arbitrary well-formed string
  export names, including namespace re-exports, while rejecting string names
  without a required local import binding and lone-surrogate export names as
  early SyntaxErrors.
- The frozen Test262 module admission now covers top-level `await` syntax in
  module blocks, exports, classes, loops, and expression contexts while keeping
  dynamic-import syntax and asynchronous module evaluation separately gated.
- File-backed module evaluation now runs each body through a Promise-backed
  continuation, schedules ready sibling modules without blocking on suspended
  dependencies, serializes async cycle members in DFS order, waits for whole
  dependency SCCs, propagates rejections through importer SCCs, and pins all
  evaluation Promises through GC until the root module settles. Pending sibling
  evaluations persist in canonical module records when another dependency
  rejects, so later importers resume the same Promise instead of re-evaluating
  the module or inheriting a stale state. Its exact
  Test262 admission separates 27 module-runtime files from parse and script-goal
  syntax coverage. CI `29184350526` and `test262-full` `29184350527` confirm
  the feature; downloaded artifacts aggregate to **27724 pass / 6720 fail / 12
  timeout / 0 error / 14011 skip / 48467 total / 34444 pass-or-fail
  executed**, exactly **+31 pass / -31 skip** from the preceding matrix.
- Script-file dynamic `import()` now has a dedicated AST and bytecode path,
  preserves the canonical source referrer through nested function chunks,
  returns a fresh intrinsic Promise for every call, and resolves relative
  modules to their cached live Module Namespace object without recursively
  draining unrelated Promise jobs. The first exact Test262 runtime slice covers
  six top-level script imports, specifier coercion, thenable use, and live
  default/named bindings.
- Promise resolving functions now share a heap-backed `[[AlreadyResolved]]`
  record, assimilate generic thenables exactly once, reject self-resolution,
  preserve native JavaScript Error objects, and keep the first resolve/reject
  call authoritative even when a thenable job is still pending.
  CI `29186666321` and `test262-full` `29186666320` confirm the combined unit;
  downloaded artifacts aggregate to **27730 pass / 6720 fail / 12 timeout / 0
  error / 13855 skip / 48317 total / 34450 pass-or-fail executed**, exactly
  **+6 pass / -6 skip** from the preceding matrix.
- The frozen script-origin dynamic-import admission now covers twelve exact
  files, adding fresh-Promise identity, direct-eval referrer inheritance,
  missing-module rejection, abrupt specifier coercion, and TypeError/URIError
  module-evaluation rejection. CI `29188197692` and `test262-full`
  `29188197664` pass for commit `6a81a07`; artifact comparison changes only the
  expressions shard by **+6 pass / -6 skip**, yielding **27736 pass / 6720
  fail / 12 timeout / 0 error / 13849 skip / 48317 total / 34456 pass-or-fail
  executed**. This also corrects the preceding prose aggregate, which
  overstated artifact skip and total by 150.
- Module-origin dynamic imports now use a shared canonical module runtime for
  status, namespace, evaluation Promise, rejection, and host completion state.
  Static/dynamic namespace identity, repeated self-imports, and in-flight TLA
  imports no longer duplicate evaluation or recursively drain the host queue;
  pending imports settle through evaluation-Promise reactions, with capability
  and completion state traced through GC. The frozen admission is **44/44** and
  the supported subset is **11633 pass / 0 fail**. CI `29190914767` and
  `test262-full` `29190914770` pass for commit `8c2a5a4`; artifacts move only
  the expressions shard by **+32 pass / -32 skip** to **27768 pass / 6720 fail
  / 12 timeout / 0 error / 13817 skip / 48317 total / 34488 pass-or-fail
  executed**.
- `import.meta` now parses as a dedicated module-only expression, evaluates to
  one mutable null-prototype object per module, rejects `import(import.meta)`
  asynchronously through normal specifier coercion, and remains distinct
  across file-backed and inline module evaluations. Nested inline functions
  retain their originating meta object and GC root. Frozen Test262 admission is
  **23/23**, dynamic import is **45/45**, and the supported subset is **11656
  pass / 0 fail**. CI `29193198575` and `test262-full` `29193198576` pass for
  commit `408ed9f`; artifacts move expressions by **+23 pass / -23 skip** to
  **27791 pass / 6720 fail / 12 timeout / 0 error / 13794 skip / 48317 total /
  34511 pass-or-fail executed**.
- Dynamic import now admits all 32 generated ambiguous-export and circular
  re-export instantiation-rejection variants. Promise rejection is verified as
  `SyntaxError` across top-level and nested arrow, async-function,
  async-generator, block, and loop contexts. Exact dynamic-import coverage is
  **77/77** and the supported subset is **11688/11688**. CI `29195024392` and
  `test262-full` `29195024404` pass for commit `26482ef`; downloaded artifacts
  move only expressions by **+32 pass / -32 skip** to **27823 pass / 6720 fail
  / 12 timeout / 0 error / 13762 skip / 48317 total / 34543 pass-or-fail
  executed**.
- Dynamic import now admits all 32 generated module-evaluation rejection cases,
  preserving fixture-thrown TypeError and URIError objects through Promise
  rejection in nested arrow, async-function, async-generator, block, branch,
  and loop contexts. Exact coverage is **107/107** and the supported subset is
  **11718/11718**. CI `29197086179` and `test262-full` `29197086220` pass for
  commit `14341e0`; artifacts move only expressions by **+30 pass / -30 skip**
  to **27853 pass / 6720 fail / 12 timeout / 0 error / 13732 skip / 48317 total
  / 34573 pass-or-fail executed**.
- Dynamic import now admits all 32 generated missing-module and abrupt
  specifier-coercion rejection cases. Nested arrow, async-function,
  async-generator, block, branch, and loop contexts preserve asynchronous host
  rejection and the original coercion throw value. Exact coverage is
  **137/137** and the supported subset is **11748/11748**. CI `29198663249` and
  `test262-full` `29198663253` pass for commit `eb27527`; artifacts move only
  expressions by **+30 pass / -30 skip** to **27883 pass / 6720 fail / 12
  timeout / 0 error / 13702 skip / 48317 total / 34603 pass-or-fail executed**.
- Dynamic import now admits the complete 28-file assignment-expression
  subtree. References, calls, members, assignments, short-circuiting,
  observable coercion, `await`, `yield`, `new.target`, and cover grammar are
  evaluated before host loading. Exact coverage is **164/164** and the
  supported subset is **11775/11775**. CI `29200204814` and `test262-full`
  `29200204823` pass for commit `5a781d3`; artifacts move only expressions by
  **+27 pass / -27 skip** to **27910 pass / 6720 fail / 12 timeout / 0 error /
  13675 skip / 48317 total / 34630 pass-or-fail executed**.
- Dynamic import now admits 16 additional root runtime files covering fresh
  intrinsic Promises, once-only evaluation, canonical namespace reuse,
  indirect resolution, errored async cycles, and for-await rejection. Exact
  dynamic-import paths now remove fixture-level `top-level-await` consistently
  in the runner and analyzer, so the errored-cycle test is executed rather than
  only listed. Exact coverage is **180/180** and the supported subset is
  **11791/11791**. CI `29201893578` and `test262-full` `29201893579` pass for
  commit `65b881f`; artifacts move only expressions by **+16 pass / -16 skip**
  to **27926 pass / 6720 fail / 12 timeout / 0 error / 13659 skip / 48317 total
  / 34646 pass-or-fail executed**.
- The standard dynamic-import `usage/` subtree is complete at **108/108**.
  Generated arrow, async-function, async-generator, block, label, branch, and
  loop contexts now cover named/default live bindings, host resolution,
  computed `then` calls, thenables, and specifier coercion. Exact coverage is
  **282/282** and the supported subset is **11893/11893**. CI `29203471348` and
  `test262-full` `29203471334` pass for commit `1299636`; artifacts move only
  expressions by **+102 pass / -102 skip** to **28028 pass / 6720 fail / 12
  timeout / 0 error / 13557 skip / 48317 total / 34748 pass-or-fail executed**.
- The standard dynamic-import `namespace/` subtree is complete at **67/67**.
  Await and Promise-reaction variants cover get/has, descriptors, sorted keys,
  set/delete strictness, extensibility, prototype invariants, nested namespaces,
  and Symbol surfaces. Exact paths now lift supported Symbol and Reflect
  metadata in both runner and analyzer, so all admitted files execute. Exact
  coverage is **347/347** and the supported subset is **11958/11958**. CI
  `29205144135` and `test262-full` `29205144132` pass for commit `65f2b45`;
  artifacts move only expressions by **+65 pass / -65 skip** to **28093 pass /
  6720 fail / 12 timeout / 0 error / 13492 skip / 48317 total / 34813
  pass-or-fail executed**.
- Direct `new import(...)` and property-access variants now produce early
  SyntaxError, while parenthesized `new (import(...))` remains a valid
  NewExpression constructor operand. Standard dynamic-import syntax is complete
  at **251/251**, including import-attributes trailing-comma grammar; source and
  defer proposals remain excluded. Exact coverage is **598/598** and the
  supported subset is **12209/12209**. CI `29207058820` and `test262-full`
  `29207058806` pass for commit `954de2f`; artifacts move only expressions by
  **+251 pass / -251 skip** to **28344 pass / 6720 fail / 12 timeout / 0 error /
  13241 skip / 48317 total / 35064 pass-or-fail executed**.
- Dynamic import attributes now evaluate options in the specified observable
  order, enumerate `with` through Proxy-aware property operations, require
  string values, and support relative JSON/text data modules without executing
  JSON as JavaScript or colliding with real module cache paths. Unsupported
  keys and types reject with `TypeError`. Shared Proxy `ownKeys` handling now
  rejects duplicates and target-invariant violations, while `JSON.parse`
  applies `ToString`, exposes length 2, preserves negative zero and large
  exponents, and decodes standard escapes. The 23-file import-attributes slice
  is **23/23**, exact dynamic import is **621/621**, and the supported subset is
  **12232 pass / 0 fail / 8207 skip / 20439 total**. CI `29209531923` and
  `test262-full` `29209531948` confirm feature commit `22c49ec`; downloaded
  artifacts aggregate to **28398 pass / 6689 fail / 13218 skip / 12 timeout /
  0 error / 48317 total / 35087 pass-or-fail executed**, exactly **+54 pass /
  -31 fail / -23 skip** from the preceding matrix.
- Test262 negative parse, resolution, and runtime tests now use distinct
  non-evaluating CLI paths. Parse validation includes compiler-hosted static
  semantics, so early errors are checked without accidentally executing code.
- Programs now carry an explicit Script or Module source type. `Vm::run_module`
  and CLI `--module`/`.mjs` execution use implicit strict mode, an isolated
  declarative top-level environment, undefined top-level `this`, module-aware
  `await` grammar parameters, and duplicate-label early errors. Static
  import/export parsing and module-graph linking remain separate follow-up
  work.
- Async functions now inherit from a distinct `%AsyncFunction.prototype%`
  with the standard constructor and `Symbol.toStringTag`. The dynamic
  `%AsyncFunction%` constructor parses async bodies, creates non-constructable
  functions without an own `prototype`, and preserves constructor/prototype
  identity per Realm instead of accidentally routing through the ordinary
  Function constructor or sharing the main Realm's async intrinsic.
- `%TypedArray%.prototype[Symbol.toStringTag]` is now the standard configurable
  getter, returning the internal TypedArray kind name even after detach while
  returning `undefined` for primitives and objects without TypedArray slots.
- `Object.prototype.toString` now observes `Symbol.toStringTag`, propagates
  getter failures, ignores non-string tags, and derives fallback tags only from
  the internal slots named by the specification. BigInt now exposes its
  standard prototype tag instead of relying on an incorrect built-in fallback.
- `%TypedArray%.prototype.with` now coerces index before replacement value,
  validates the resulting index against the current view, and copies the
  original snapshot length into a fresh same-kind intrinsic result without
  consulting `constructor` or `@@species`.
- Same-kind TypedArray copy methods now use a GC-rooted Realm intrinsic
  constructor table, so replacing global TypedArray constructor bindings does
  not affect `with`, `toReversed`, or `toSorted`, including cross-Realm calls.
- `%TypedArray%.prototype.toLocaleString` now validates and snapshots its
  receiver, invokes each current Number/BigInt value's locale conversion with
  two forwarded locale arguments from the method's Realm, stringifies each
  result, and preserves the snapshot visit range across detach or resize
  effects. Number locale arguments are not treated as a radix, and BigInt has
  its own locale conversion rather than inheriting Object behavior.
- `%TypedArray%.prototype.lastIndexOf` now validates and snapshots its receiver,
  distinguishes omitted from explicit `undefined` `fromIndex`, skips indexes
  invalidated by detach or resize, and searches current values in reverse with
  strict equality.
- `%TypedArray%.prototype.indexOf` now validates and snapshots its receiver,
  coerces `fromIndex` in specification order, skips integer indexes invalidated
  by detach or resize, and compares Number/BigInt elements with strict equality.
- `%TypedArray%.prototype.filter` now visits the snapshot range before species
  construction, preserves each selected current value, and creates a writable
  same-content-type destination sized to the final selection count.
- `%TypedArray%.prototype.map` now creates and validates its writable species
  result before iteration, then maps the snapshot visit range from current
  integer-indexed reads with callback-result conversion into the destination.
- `%TypedArray%.prototype.reduce` now shares a direction-aware reduction core
  with `reduceRight`, validating and snapshotting the receiver while reading
  each current integer-indexed value immediately before callback invocation.
- `%TypedArray%.prototype.toSorted` now stably sorts a value snapshot into a
  fresh same-type intrinsic TypedArray, preserves the source (including
  immutable-backed views), and ignores user `constructor` and `@@species`.
- GC collection now invalidates property inline-cache entries before swept heap
  cells can be reused, preventing a new object from observing stale properties
  cached for the cell's previous occupant.
- `%TypedArray%.prototype.sort` now performs a stable numeric sort over a
  snapshot of the validated view, with BigInt ordering, NaN and signed-zero
  handling, observable comparator coercion, and current-bounds writes after
  resize or detach side effects.
- GC now traces Proxy target and handler internal slots, and Proxy receiver
  data-property creation roots its inputs and transient descriptor across
  `defineProperty` trap lookup and invocation. Allocation pressure no longer
  drops a live Proxy's target or descriptor values during TypedArray prototype
  writes.
- `%TypedArray%.prototype.reduceRight` now validates and snapshots the receiver,
  selects the last current element when no initial accumulator is supplied, and
  reads each remaining integer-indexed value in descending order so detach and
  resize side effects are observed without extending the initial visit range.
- Interpreted function call environments are now pinned from allocation through
  setup and execution, so GC pressure during arguments/rest construction cannot
  discard a derived constructor's uninitialized `this` binding before `super()`
  initializes it.
- `%TypedArray%.prototype.includes` now validates and snapshots the receiver,
  coerces `fromIndex` after that snapshot, reads current integer-indexed values,
  and compares with SameValueZero across detach and resize side effects.
- `%TypedArray%.prototype.forEach` now validates and snapshots the receiver,
  reads each current integer-indexed value before callback invocation, ignores
  callback results, and preserves the initial visit count across detach,
  shrink, and growth before returning `undefined`.
- `%TypedArray%.prototype.every` now validates and snapshots the receiver,
  reads each current integer-indexed value before callback invocation,
  short-circuits on the first falsy result, and preserves the initial visit
  count across detach, shrink, and growth.
- `%TypedArray%.prototype.some` now validates and snapshots the receiver, reads
  each current integer-indexed value before callback invocation, short-circuits
  on the first truthy result, and preserves the initial visit count across
  detach, shrink, and growth.
- `%TypedArray%.prototype.findLastIndex` now shares the reverse receiver
  validation, callback protocol, length snapshot, and current-value reads of
  `findLast` while returning the matching index or `-1`.
- `%TypedArray%.prototype.findLast` now validates and snapshots the receiver,
  reads current integer-indexed values from the final index toward zero, and
  preserves the reverse visit count across detach, shrink, and growth while
  returning the value observed before a matching callback.
- `%TypedArray%.prototype.findIndex` now shares `find`'s receiver validation,
  callback protocol, initial-length snapshot, and current-value reads across
  detach, shrink, and growth while returning the matching index or `-1`.
- `%TypedArray%.prototype.find` now validates the receiver and predicate,
  snapshots the internal length, reads each current integer-indexed value before
  callback invocation, and preserves iteration count across detach, shrink, and
  growth while returning the value observed before a matching callback.
- `%TypedArray%.prototype.slice` now computes bounds from the initial internal
  length, creates a writable species result, revalidates the source after
  observable construction, and copies same-kind elements byte-for-byte while
  converting different kinds by value. `%TypedArray%[Symbol.species]` and
  unaligned length-tracking resizable-buffer views now follow intrinsic rules.
- `%TypedArray%.prototype.copyWithin` now validates writable receivers before
  argument coercion, snapshots the internal length, revalidates bounds after
  coercion-driven resize or detach, and performs overlap-safe raw byte copies
  that preserve NaN payloads and other element bit patterns.
- `%TypedArray%.prototype.toReversed` now validates and snapshots the source's
  internal length, creates a distinct same-kind TypedArray without consulting
  `constructor` or `@@species`, and copies Number or BigInt elements in reverse
  order without mutating the source.
- `%TypedArray%.prototype.reverse` now validates a writable TypedArray before
  mutation, snapshots its internal element length, and swaps Number or BigInt
  elements through integer-indexed access. Fixed and length-tracking resizable
  views reverse their current in-bounds range, while detached, immutable, and
  out-of-bounds views throw before mutation.
- `%TypedArray%.prototype.keys` and `entries` now use the shared Array Iterator
  prototype with dedicated key and key-value iterator modes. Both validate at
  creation, track resizable views on each pull, throw when a live fixed view
  becomes out of bounds, and remain exhausted after completion.
- `%TypedArray%.prototype.values` and the default iterator now validate the
  receiver at creation and revalidate dynamic bounds on every `next()` pull.
  Collection iterators, their sources, and iterator-result objects are rooted
  through the current job, preventing rare GC-boundary state loss, while a
  genuinely exhausted iterator remains exhausted after buffer regrowth.
- `%TypedArray%.prototype.join` now validates and snapshots the initial view
  length before separator coercion. Detach, shrink, or growth during separator
  conversion therefore preserves the initial iteration count, while later
  out-of-bounds element reads contribute empty fields as required.
- `%TypedArray%.prototype.set` now implements both TypedArray-source and
  array-like-source copying, including offset coercion order, immutable and
  out-of-bounds validation, Number/BigInt content checks, overlapping-buffer
  snapshots, same-type bit-preserving copies, and element-by-element behavior
  while resizable targets change during source access or value conversion.
- `%TypedArray%.prototype.subarray` now preserves the source's raw byte offset
  while it is detached or out of bounds, performs begin/end coercion before
  construction, and keeps an omitted-end result length-tracking when the
  source is length-tracking. Resizes during coercion therefore use the initial
  length snapshot without losing the source view's internal offset.
- `%TypedArray%.prototype.fill` now snapshots the validated initial length,
  converts value/start/end in specification order, then revalidates dynamic
  bounds before writing. Growth during value conversion does not widen an
  omitted range, while a fixed view resized out of bounds throws TypeError.
  TypedArrays also expose `values()` as their default `@@iterator`, with lazy
  reads that observe current length-tracking bounds.
- `%TypedArray%.prototype.at` now validates the receiver and snapshots its
  length before index coercion, applies relative indexing, and performs the
  final integer-indexed read against the current backing-store bounds. Initial
  out-of-bounds views throw while views resized out of bounds during coercion
  return `undefined`, including Number and BigInt TypedArrays.
- TypedArray and DataView instances over resizable or growable buffers now
  record whether their length is fixed or tracking. Dynamic view records drive
  length/byteLength/byteOffset getters, integer-index get/set/has/own-keys,
  DataView access after argument coercion, Atomics length snapshots, and
  out-of-bounds recovery after the backing store grows again. Constructing from
  an out-of-bounds TypedArray now throws instead of copying an empty view.
- `ArrayBuffer` now accepts `maxByteLength`, exposes standard `resizable` and
  `maxByteLength` accessors, and implements `resize()` with detach revalidation,
  shrinking, zero-initialized growth, and immutable/shared brand rejection.
  `transfer()` preserves a resizable source's maximum length while
  `transferToFixedLength()` and `transferToImmutable()` produce fixed-length
  destinations. Internally allocated ArrayBuffer construction now validates
  length options before observing `new.target.prototype`.
- `Atomics` now exposes the synchronous `add`, `and`, `compareExchange`,
  `exchange`, `isLockFree`, `load`, `or`, `store`, `sub`, and `xor`
  operations for Number and BigInt integer TypedArrays. Each operation holds
  the backing-buffer mutex across its complete read/modify/write sequence,
  accepts mutable ArrayBuffer and SharedArrayBuffer backing stores, preserves
  coercion and validation order, and permits `load` on immutable buffers.
  `notify` and blocking `wait` use a FIFO waiter list and Condvar wakeups with
  count, timeout, Number/BigInt, and `CanBlock` semantics. `waitAsync` returns
  immediate records when it can settle synchronously and otherwise resolves an
  externally queued Promise job after notification or timeout. `pause` is
  available as the implementation-defined no-op hint permitted by ECMAScript.
- SharedArrayBuffer backing bytes and waiter lists now use shared `Arc` storage.
  The test262 host can start independent worker VMs, broadcast one SAB backing
  store into each worker heap, collect reports, sleep, and expose monotonic
  time. This exercises real cross-thread wait/notify behavior instead of
  emulating agents inside one VM.
- `%TypedArray%.prototype.fill` now writes Number and BigInt elements through
  ArrayBuffer and SharedArrayBuffer views with offset/range handling and
  immutable-buffer rejection.
- `SharedArrayBuffer` now has a distinct shared brand with constructor and
  cross-Realm `new.target` prototype semantics, `byteLength`, `@@species`,
  `@@toStringTag`, and species-aware `slice()`. Fixed-length shared buffers
  back TypedArray and DataView views without detachment, while ordinary
  ArrayBuffer accessors, transfer methods, and immutable operations reject the
  shared brand. The constructor now accepts `maxByteLength`, exposes
  `growable` and `maxByteLength`, and grows shared backing storage
  monotonically through `grow()` while preserving existing bytes and
  zero-initializing the extension.
- `FinalizationRegistry` now stores weak registration targets and unregister
  tokens alongside strongly traced held values and cleanup callbacks. GC sweep
  moves dead targets into cleanup jobs, unregister removes every matching
  registration, callback failures stay contained at the cleanup-job boundary,
  and constructor, brand, Symbol-target, cross-Realm, extensibility, and
  descriptor behavior follows the ECMAScript surface.
- `WeakRef` now has a dedicated weak heap exotic instead of a hidden strong
  property. Object targets are cleared during GC sweep, unregistered and
  well-known Symbol targets are accepted, registered Symbols and other
  primitives are rejected, and `deref()` validates its receiver and keeps a
  live target through the current job. Constructor/new-target prototype
  selection, cross-Realm fallback, extensibility, and standard descriptors
  follow the ECMAScript surface.
- Optional chains now compile as one shared short-circuit boundary instead of
  closing each `?.` jump at the current member. Non-optional member/call tails
  are skipped with their arguments, grouped optional member calls preserve
  their Reference receiver, `super.method?.()` preserves `this`, and private
  forms such as `obj?.#field` and `obj?.receiver.#field` follow the same chain.
  Optional delete now removes the final property only on a live chain, while
  private delete and optional-chain tagged templates are rejected as early
  errors.
- Async-from-sync iteration now follows `AsyncFromSyncIteratorContinuation`:
  it creates an intrinsic Promise capability, observes and propagates abrupt
  Promise `constructor` access, and unwraps iterator values with the required
  job ordering before producing iterator-result objects. `for await (async of
  iterable)` is also accepted while the non-await `for (async of iterable)`
  early error remains intact.
- Ordinary async functions now suspend at pending `await` operations and resume
  from Promise reaction jobs. Continuations preserve operand/local stacks,
  lexical and catch/finally environments, `this`, `new.target`, and the result
  capability across GC; pending fulfillment, rejection into `catch`, nested
  async calls, and FIFO job ordering no longer continue with `undefined` or run
  ahead of their callers. `for await...of` now lowers iterator-result and
  async-from-sync value waits through the same resumable Await bytecode, fixing
  Promise/Await interleaving.
- Async-function completion now resolves through a fresh Promise capability,
  so returned Promises and generic thenables are assimilated instead of being
  exposed as nested fulfillment values. Then getters remain observable and
  callable thenables run through the PromiseResolveThenable job queue. The
  async-enabled class-elements diagnostic reports **2695 pass / 0 fail / 267
  skip / 2962 total**.
- Async generators now serialize `next`, `return`, and `throw` requests and
  resume suspended `await` operations through Promise jobs instead of draining
  them synchronously. Explicit `return expr` observes its required Await
  boundary, yielded values and thenables settle in queue order, delegated
  return/getter errors reach the generator body, and broken Promise
  `constructor` access rejects the active request.
- GC now traces suspended generators' current and catch environments, queued
  async-generator capabilities, and the target Promises of pending resolve and
  reject jobs. A collection between `await` suspension and resumption no longer
  drops block-scoped bindings.

### Test tooling

- The default Module binding slice adds 47 exact `language/module-code/` files,
  bringing that subtree to **115 pass / 0 fail / 484 skip**. It covers ordinary,
  generator, async, and async-generator default declarations, default
  expressions and name inference, mixed imports, live bindings, re-exports,
  abrupt completion, and early errors. Namespace-dependent files remain gated.
  Feature commit `f74d4bb` is confirmed by CI `29177056387` and `test262-full`
  `29177056390`; artifacts are **27276 pass / 6750 fail / 14429 skip / 12
  timeout / 0 error / 48467 total**. The module slice is exactly **+47 pass /
  -47 skip**; one unrelated built-ins pass returned to timeout as timing
  variance.
- The cyclic Module slice adds 22 exact `language/module-code/` files, bringing
  that subtree to **68 pass / 0 fail / 531 skip**. Negative parse, resolution,
  and runtime tests use phase-specific CLI paths, and parse checks include
  compiler-hosted static semantics without evaluation. Feature commit
  `55f3b87` is confirmed by CI `29176281928` and `test262-full` `29176281902`;
  downloaded artifacts are **27230 pass / 6750 fail / 14476 skip / 11 timeout /
  0 error / 48467 total**. The module admission is exactly **+22 pass / -22
  skip**; one unrelated built-ins timeout passed as timing variance.
- The frozen Module graph slice adds 11 exact `language/module-code/` files,
  bringing that subtree to **46 pass / 0 fail / 553 skip**. Test262 module
  entries are staged with their relative fixture graph in an isolated
  temporary directory instead of modifying the upstream checkout. The
  authoritative supported language subset remains **11589 pass / 0 fail /
  8850 skip / 20439 total**. Feature commit `f0d5525` is confirmed by CI
  `29174854010` and `test262-full` `29174854002`; downloaded artifacts are
  **27207 pass / 6750 fail / 14498 skip / 12 timeout / 0 error / 48467
  total**, exactly **+11 pass / -11 skip** with failures unchanged.
- The first frozen ES Module source-goal slice admits 35 exact
  `language/module-code/` files at **35 pass / 0 fail / 0 skip**. Test262
  module files now execute through the CLI's real `--module` path; files that
  require import/export syntax or linking remain gated. The authoritative
  supported language subset remains **11589 pass / 0 fail / 8850 skip /
  20439 total**. Feature commits `6d0254f` and `9a22731` are confirmed by CI
  `29173247360` and `test262-full` `29173247358`. The final artifact is
  **27196 pass / 6750 fail / 14509 skip / 12 timeout / 0 error / 48467
  total**: 35 module files moved from skip to pass and the audited per-Realm
  `%AsyncFunction%` intrinsic closed five existing failures.
- The complete pinned `built-ins/TypedArray/` subtree is now admitted at
  **1446 pass / 0 fail / 0 skip / 1446 total**. The final 13 constructor
  surface, `Symbol.species`, and `%TypedArray%.of` descriptor/receiver files
  are frozen to exact paths so future tests remain gated until audited.
  Independent implementation reviews found no admission-blocking semantics
  defect. New regressions preserve rejection of detached or out-of-bounds
  empty constructor results and root `of` arguments across GC during
  construction and element conversion. The authoritative supported language
  subset remains **11589 pass / 0 fail / 8850 skip / 20439 total**.
  CI `29170927445` and `test262-full` `29170927448` confirm the change at
  **27157 pass / 6755 fail / 14544 skip / 11 timeout / 0 error / 48467
  total**. The admission moved exactly 13 files from skip to pass; one
  additional built-ins timeout became a pass as unrelated timing variance.
- The frozen `built-ins/TypedArray/from/` set is fully admitted at **21 pass /
  0 fail / 0 skip / 21 total**. Independent review removed the non-standard
  65,536-item iterable cap, rejects detached or out-of-bounds constructor
  results even for empty sources, and roots iterator/result/value objects
  across observable callbacks and GC. The full TypedArray built-ins subtree is
  now **1433 pass / 0 fail / 13 skip / 1446 total**.
  CI `29169720681` and `test262-full` `29169720682` confirm the independently
  reviewed change at **27143 pass / 6755 fail / 14557 skip / 12 timeout / 0
  error / 48467 total**. Exactly 21 files moved from skip to pass.
- The pinned Test262 `built-ins/TypedArray/prototype` subtree now has complete
  admitted file coverage at **1404 pass / 0 fail / 0 skip / 1404 total**. The
  final parent `constructor` and `Symbol.iterator` identity/descriptor files
  are frozen to exact paths. Main/created-Realm alias identity remains stable
  across independent mutation and forced GC.
  CI `29168553907` and `test262-full` `29168553904` confirm the independently
  reviewed change at **27122 pass / 6755 fail / 14578 skip / 12 timeout / 0
  error / 48467 total**. Exactly two files moved from skip to pass.
- TypedArray `toString` and `Symbol.iterator` now close at **5 pass / 0 fail /
  0 skip / 5 total**. `%TypedArray%.prototype.toString` is the same rooted
  intrinsic function object as `Array.prototype.toString`, including during
  later Realm creation after mutable prototype replacement. Array toString now
  follows its generic observable `join` lookup and Object fallback algorithm,
  so detached Number and BigInt TypedArrays throw through their validated join.
  CI `29167399296` and `test262-full` `29167399293` confirm the independently
  reviewed change at **27120 pass / 6755 fail / 14580 skip / 12 timeout / 0
  error / 48467 total**. Four skipped files and one existing failing file moved
  to pass.
- The exact TypedArray `byteLength`, `byteOffset`, and `length` accessor paths
  are now fully admitted at **52 pass / 0 fail / 0 skip / 52 total**. Admission
  is frozen to the audited files so future tests remain gated until reviewed.
  Focused regressions cover foreign-Realm getter/error provenance, forced-GC
  survival, and fixed or length-tracking views over growable shared buffers.
  CI `29166327417` and `test262-full` `29166327397` confirm the change at
  **27115 pass / 6756 fail / 14584 skip / 12 timeout / 0 error / 48467 total**.
  Against the preceding identical matrix, exactly 33 files moved from skip to
  pass.
- The exact `built-ins/TypedArray/prototype/buffer/` path is now fully admitted
  at **12 pass / 0 fail / 0 skip / 12 total**, covering backing-buffer identity
  after detach, Number/BigInt views, accessor descriptors, and incompatible or
  inherited receivers. Internally allocated TypedArray buffers now inherit from
  the originating Realm's intrinsic `ArrayBuffer.prototype`, and Realm-created
  native accessors inherit from that Realm's `Function.prototype`.
  CI `29165210243` and `test262-full` `29165210230` confirm the independently
  reviewed change at **27082 pass / 6756 fail / 14617 skip / 12 timeout / 0
  error / 48467 total**. Against the preceding identical matrix, exactly 12
  files moved from skip to pass.
- The exact `built-ins/TypedArray/prototype/Symbol.toStringTag/` path now closes
  at **18 pass / 0 fail / 0 skip / 18 total**, up from **2 pass / 16 fail**,
  covering Number/BigInt kinds, detached buffers, incompatible receivers, and
  accessor descriptors.
  CI `29163993136` and `test262-full` `29163993119` confirm the independently
  reviewed change at **27070 pass / 6756 fail / 14629 skip / 12 timeout / 0
  error / 48467 total**. Against the preceding identical matrix, 12 skipped
  files and eight failing files moved to pass, including the broader Object tag
  corrections.
- The exact `built-ins/TypedArray/prototype/with/` path now closes at **22 pass
  / 0 fail / 0 skip / 22 total**, up from **1 pass / 21 fail**, covering
  Number/BigInt coercion order, current resizable-buffer bounds, immutable
  sources, intrinsic same-kind construction, and species avoidance.
  CI `29162715549` and `test262-full` `29162715530` confirm the independently
  reviewed change at **27050 pass / 6764 fail / 14641 skip / 12 timeout / 0
  error / 48467 total**. Against the preceding identical matrix, exactly 22
  files moved from skip to pass.
- The exact `built-ins/TypedArray/prototype/toLocaleString/` path now closes at
  **39 pass / 0 fail / 0 skip / 39 total**, up from **6 pass / 33 fail**,
  covering Number/BigInt conversion hooks, abrupt completions, internal length,
  and fixed-length and length-tracking resizable views.
  CI `29161632961` and `test262-full` `29161632974` confirm the reviewed change
  at **27028 pass / 6764 fail / 14663 skip / 12 timeout / 0 error / 48467
  total**. Against the preceding identical matrix, exactly 39 files moved from
  skip to pass.
- The exact `built-ins/TypedArray/prototype/lastIndexOf/` path now closes at
  **42 pass / 0 fail / 0 skip / 42 total**, up from **6 pass / 36 fail**,
  covering Number/BigInt comparison, relative and infinite `fromIndex`,
  detached buffers, and fixed-length and length-tracking resizable views.
  CI `29159883869` and `test262-full` `29159883857` confirm the change at
  **26989 pass / 6764 fail / 14702 skip / 12 timeout / 0 error / 48467 total**.
  Against the preceding identical matrix, exactly 42 files moved from skip to
  pass.
- The exact `built-ins/TypedArray/prototype/indexOf/` path now closes at **43
  pass / 0 fail / 0 skip / 43 total**, up from **6 pass / 37 fail**, covering
  Number/BigInt strict comparison, `fromIndex` coercion, detached buffers, and
  fixed-length and length-tracking resizable views.
  CI `29158983437` and `test262-full` `29158983402` confirm the change at
  **26947 pass / 6764 fail / 14744 skip / 12 timeout / 0 error / 48467 total**.
  Against the preceding identical matrix, exactly 43 files moved from skip to
  pass.
- The exact `built-ins/TypedArray/prototype/filter/` path now closes at **85
  pass / 0 fail / 0 skip / 85 total**, covering callback-before-species order,
  Number/BigInt results, immutable destinations, and resizable buffers.
  CI `29157976820` and `test262-full` `29157976810` confirm the change at
  **26904 pass / 6764 fail / 14787 skip / 12 timeout / 0 error / 48467 total**.
  Against the preceding identical matrix, exactly 85 files moved from skip to
  pass.
- The exact `built-ins/TypedArray/prototype/map/` path now closes at **85 pass /
  0 fail / 0 skip / 85 total**, covering Number/BigInt species results,
  immutable destinations, resizable buffers, callback effects, and conversion.
  CI `29157121160` and `test262-full` `29157121173` confirm the change at **26819
  pass / 6764 fail / 14872 skip / 12 timeout / 0 error / 48467 total**. Against
  the preceding identical matrix, exactly 85 files moved from skip to pass.
- The exact `built-ins/TypedArray/prototype/reduce/` path now closes at **50
  pass / 0 fail / 0 skip / 50 total**, while the refactored `reduceRight` path
  remains at **50 pass / 0 fail / 0 skip / 50 total**. CI `29156205544` and
  `test262-full` `29156205566` confirm the change at **26734 pass / 6764 fail /
  14957 skip / 12 timeout / 0 error / 48467 total**. Against the preceding
  identical matrix, exactly 50 files moved from skip to pass.
- The exact `built-ins/TypedArray/prototype/toSorted/` path now closes at **12
  pass / 0 fail / 0 skip / 12 total**, covering default/custom comparators,
  same-type copying, immutable sources, and species avoidance. CI `29155327452`
  and `test262-full` `29155327470` confirm the change at **26684 pass / 6764
  fail / 15007 skip / 12 timeout / 0 error / 48467 total**. Against the
  preceding identical matrix, exactly 12 files moved from skip to pass.
- The exact `built-ins/TypedArray/prototype/sort/` path now closes at **36 pass
  / 0 fail / 0 skip / 36 total**, covering stable Number and BigInt ordering,
  immutable buffers, and resizable-buffer comparator side effects. CI
  `29154453789` and `test262-full` `29154453779` confirm the final change at
  **26672 pass / 6764 fail / 15019 skip / 12 timeout / 0 error / 48467 total**.
  Against the preceding identical matrix, exactly 36 files moved from skip to
  pass. The final run also confirms the stale property-cache failure exposed by
  the first candidate run is fixed.
- The exact `built-ins/TypedArray/prototype/reduceRight/` path now closes at
  **50 pass / 0 fail / 0 skip / 50 total**, covering Number and BigInt content,
  resizable buffers, abrupt callbacks, and default/custom accumulators. CI
  `29152822658` and `test262-full` `29152822656` confirm the final change at
  **26636 pass / 6764 fail / 15055 skip / 12 timeout / 0 error / 48467 total**.
  Against the preceding identical matrix, exactly 50 files moved from skip to
  pass. The final full run also confirms the Proxy GC regression exposed by the
  first candidate run is fixed.
- The exact `built-ins/TypedArray/prototype/includes/` path now closes at **45
  pass / 0 fail / 0 skip / 45 total**, including resizable-buffer matrices that
  repeatedly construct dynamic TypedArray subclasses under GC pressure. CI
  `29151186097` and `test262-full` `29151186100` confirm the change at **26586
  pass / 6764 fail / 15105 skip / 12 timeout / 0 error / 48467 total**. Against
  the preceding identical matrix from `test262-full` `29150690813`, all 45
  focused files moved from skip to pass and the call-environment GC fix moved
  one additional file from fail to pass.
- The exact `built-ins/TypedArray/prototype/forEach/` path now closes at **42
  pass / 0 fail / 0 skip / 42 total**. The shared `every`, `some`, and
  find-family paths remain at **240 pass / 0 fail / 0 skip / 240 total**. CI
  `29150129716` and `test262-full` `29150129689` confirm the change at **26540
  pass / 6765 fail / 15150 skip / 12 timeout / 0 error / 47718 total**. Against
  the preceding identical matrix, 42 files moved from skip while the aggregate
  gained 43 pass and lost one fail.
- The exact `built-ins/TypedArray/prototype/every/` path now closes at **44 pass
  / 0 fail / 0 skip / 44 total**. The shared `some` and find-family paths remain
  at **196 pass / 0 fail / 0 skip / 196 total**. CI `29149369782` and
  `test262-full` `29149369800` confirm the change at **26497 pass / 6766 fail /
  15192 skip / 12 timeout / 0 error / 47718 total**. Against the preceding
  identical matrix, exactly 44 files moved from skip to pass.
- The exact `built-ins/TypedArray/prototype/some/` path now closes at **44 pass
  / 0 fail / 0 skip / 44 total**. The four shared find paths remain at **152
  pass / 0 fail / 0 skip / 152 total** after predicate-loop consolidation. CI
  `29148631959` and `test262-full` `29148631970` confirm the change at **26453
  pass / 6766 fail / 15236 skip / 12 timeout / 0 error / 47718 total**. Against
  the preceding identical matrix, 44 files moved from skip while the aggregate
  gained 43 pass and one fail.
- The exact `built-ins/TypedArray/prototype/findLastIndex/` path now closes at
  **38 pass / 0 fail / 0 skip / 38 total**. CI `29147889854` and
  `test262-full` `29147889860` confirm the change at **26410 pass / 6765 fail /
  15280 skip / 12 timeout / 0 error / 47718 total**. Against the preceding
  identical matrix, 38 files moved from skip while the aggregate gained 39
  pass and lost one fail.
- The exact `built-ins/TypedArray/prototype/findLast/` path now closes at **38
  pass / 0 fail / 0 skip / 38 total**. CI `29147184493` and `test262-full`
  `29147184510` confirm the change at **26371 pass / 6766 fail / 15318 skip /
  12 timeout / 0 error / 47718 total**. Against the preceding identical
  matrix, 38 files moved from skip while the aggregate gained 39 pass and lost
  one fail.
- The exact `built-ins/TypedArray/prototype/findIndex/` path now closes locally
  at **38 pass / 0 fail / 0 skip / 38 total**. CI `29146424305` and
  `test262-full` `29146424303` confirm the change; the full aggregate is
  **26332 pass / 6767 fail / 15356 skip / 12 timeout / 0 error / 47718 total**.
  Against the immediately preceding identical matrix, 38 files moved out of
  skip while the aggregate changed by 35 pass and 3 fail.
- The exact `built-ins/TypedArray/prototype/find/` path now closes at **38 pass
  / 0 fail / 0 skip / 38 total**. CI `29145657670` and `test262-full`
  `29145657675` confirm the change at **26297 pass / 6764 fail / 15394 skip /
  12 timeout / 0 error / 48467 total**.
- The exact `built-ins/TypedArray/prototype/slice/` path now closes at **92 pass
  / 0 fail / 0 skip / 92 total**. CI `29144932312` and `test262-full`
  `29144932309` confirm the change at **26259 pass / 6764 fail / 15432 skip /
  12 timeout / 0 error / 48467 total**.
- The exact `built-ins/TypedArray/prototype/copyWithin/` path now closes at **65
  pass / 0 fail / 0 skip / 65 total**. Three detach stress files receive a
  path-limited 600-second timeout because their full constructor/factory matrix
  takes about 100 seconds in the interpreter; all other files retain 8 seconds.
  CI `29143846038` and `test262-full` `29143846110` confirm the change at
  **26164 pass / 6767 fail / 15524 skip / 12 timeout / 0 error / 48467 total**.
- The exact `built-ins/TypedArray/prototype/toReversed/` path now closes at **9
  pass / 0 fail / 0 skip / 9 total**. CI `29142341248` and `test262-full`
  `29142341265` confirm the change at **26100 pass / 6766 fail / 15589 skip /
  12 timeout / 0 error / 48467 total**.
- The exact `built-ins/TypedArray/prototype/reverse/` path now closes at **22
  pass / 0 fail / 0 skip / 22 total**. CI `29141851460` and `test262-full`
  `29141851451` confirm the change at **26089 pass / 6768 fail / 15598 skip /
  12 timeout / 0 error / 48467 total**.
- The exact `built-ins/TypedArray/prototype/{keys,entries}/` paths now close at
  **38 pass / 0 fail / 0 skip / 38 total**. CI `29141404792` and
  `test262-full` `29141404775` confirm the change at **26067 pass / 6768 fail /
  15620 skip / 12 timeout / 0 error / 48467 total**.
- The exact `built-ins/TypedArray/prototype/values/` path now closes at **21
  pass / 0 fail / 0 skip / 21 total**; the matching `Symbol.iterator` path is
  **1 pass / 0 fail / 0 skip / 1 total** locally. CI `29140858679` and
  `test262-full` `29140858676` confirm the change at **26031 pass / 6766 fail /
  14909 skip / 12 timeout / 47718 total**.
- The exact `built-ins/TypedArray/prototype/join/` path now closes at **32 pass
  / 0 fail / 0 skip / 32 total**, including BigInt, detached, out-of-bounds,
  separator-coercion, and resizable-buffer behavior. CI `29139734054` and
  `test262-full` `29139734042` confirm the change at **26010 pass / 6766 fail /
  14931 skip / 47718 total**.
- The exact `built-ins/TypedArray/prototype/set/` path now closes at **110 pass
  / 0 fail / 0 skip / 110 total**, covering array-like and TypedArray sources,
  BigInt, SharedArrayBuffer, immutable buffers, overlap, and resize behavior.
  CI `29139260377` and `test262-full` `29139260415` confirm the change at
  **25978 pass / 6766 fail / 14963 skip / 47718 total**.
- The exact `built-ins/TypedArray/prototype/subarray/` path now closes at **67
  pass / 0 fail / 0 skip / 67 total**, including detached-buffer coercion,
  species-constructor arguments, and resizable-buffer view semantics. CI
  `29138611001` and `test262-full` `29138610981` confirm the change at **25868
  pass / 6766 fail / 15073 skip / 47718 total**.
- The exact `built-ins/TypedArray/prototype/fill/` path now closes at **52 pass
  / 0 fail / 0 skip / 52 total**, including Number/BigInt, immutable-buffer,
  coercion-order, and resizable-buffer coverage. CI `29138124604` and
  `test262-full` `29138124559` confirm the change at **25801 pass / 6766 fail /
  15140 skip / 47718 total**.
- The exact `built-ins/TypedArray/prototype/at/` path now admits the implemented
  TypedArray, BigInt, arrow-function, and resizable-buffer dependencies at
  **15 pass / 0 fail / 0 skip / 15 total**. CI `29137525369` and
  `test262-full` `29137525322` confirm the change; downloaded artifacts report
  **25749 pass / 6766 fail / 11 timeout / 0 error / 15941 skip / 48467 total /
  32526 executed**, or **79.2%** of executed files and **53.1%** of the matrix.
- Resizable view admission adds eight TypedArray-constructor files and thirty
  DataView files, raising those focused paths to **682 pass / 0 fail / 56 skip
  / 738 total** and **522 pass / 0 fail / 39 skip / 561 total**. A narrow
  `built-ins/TypedArray/` exception admits **26 pass / 0 fail** for dynamic
  indexed-exotic and length/byteLength/byteOffset coverage while leaving the
  unimplemented prototype method families gated. CI `29136994077` and
  `test262-full` `29136994074` confirm the change; downloaded artifacts report
  **25734 pass / 6769 fail / 11 timeout / 0 error / 15953 skip / 48467 total /
  32514 executed**, or **79.1%** of executed files and **53.1%** of the matrix.
- The ArrayBuffer exception now admits `resizable-arraybuffer` and only the
  receiver brands needed by those tests, raising the focused path to **194 pass
  / 0 fail / 27 skip / 221 total** without opening unrelated SharedArrayBuffer
  coverage. Atomics now closes at **389 pass / 0 fail / 0 skip / 389 total**
  after admitting the five resize/grow coercion-order cases. CI `29136048993`
  and `test262-full` `29136049024` confirm the change; downloaded artifacts
  report **25670 pass / 6769 fail / 11 timeout / 0 error / 16017 skip / 48467
  total / 32450 executed**, or **79.1%** of executed files and **53.0%** of the
  matrix.
- The exact `built-ins/SharedArrayBuffer/` exception now admits the
  `resizable-arraybuffer` feature after implementing the growable SAB core.
  Focused coverage closes at **104 pass / 0 fail / 0 skip / 104 total**. The
  broader resizable ArrayBuffer and length-tracking view integration remain
  gated. CI `29135330020` and `test262-full` `29135330077` confirm the change;
  downloaded artifacts report **25593 pass / 6769 fail / 11 timeout / 0 error
  / 16094 skip / 48467 total / 32373 executed**, or **79.1%** of executed
  files and **52.8%** of the matrix.
- The runner and analyzer now admit `Atomics.waitAsync`, `Atomics.pause`, and
  their required async/destructuring metadata only on the completed Atomics
  paths. Fixed-length Atomics reports **384 pass / 0 fail / 5 skip / 389
  total**; all five skipped files require growable or resizable buffers. The
  supported language subset remains **11589 pass / 0 fail / 8850 skip / 20439
  total**. CI `29119574209` and `test262-full` `29119574146` confirm the
  change; downloaded artifacts report **25549 pass / 6769 fail / 11 timeout /
  0 error / 16138 skip / 48467 total / 32329 executed**, or **79.0%** of
  executed files and **52.7%** of the matrix.
- `tools/test262_support.py` now forwards `CanBlockIsTrue` and
  `CanBlockIsFalse` metadata to the RuJa host. Exact-path admission adds
  `Atomics.notify` and `Atomics.wait`, raising the focused Atomics path to
  **279 pass / 0 fail / 110 skip / 389 total**. The remaining files are 101
  `waitAsync`, 5 `pause`, and 4 resizable-buffer cases. The supported language
  subset remains **11589 pass / 0 fail / 8850 skip / 20439 total**. CI
  `29117508967` and `test262-full` `29117508706` confirm the change;
  downloaded artifacts report **25444 pass / 6769 fail / 11 timeout / 0 error
  / 16243 skip / 48467 total / 32224 executed**, or **79.0%** of executed
  files and **52.5%** of the matrix.
- The runner and analyzer now keep `Atomics`, `Atomics.pause`, and
  `Atomics.waitAsync` behind global feature gates, then admit only the ten
  completed synchronous operation directories and three Atomics object-surface
  files. The focused path reports **154 pass / 0 fail / 235 skip / 389 total**;
  all skipped files belong to `wait`, `notify`, `waitAsync`, or `pause`. The
  supported language subset remains **11589 pass / 0 fail / 8850 skip / 20439
  total**. CI `29115320336` and `test262-full` `29115320329` confirm the
  change; downloaded artifacts report **25319 pass / 6769 fail / 11 timeout /
  0 error / 16368 skip / 48467 total / 32099 executed**, or **78.9%** of
  executed files and **52.2%** of the matrix.
- The runner and analyzer now admit the exact `built-ins/SharedArrayBuffer/`
  path without opening the broader SharedArrayBuffer or Atomics feature gates.
  The fixed-length coverage reports **60 pass / 0 fail / 44 skip / 104 total**;
  the 44 growable/resizable cases remain intentionally gated. Tooling tests
  verify that the feature exception does not apply outside this built-in path,
  and the supported language subset remains **11589 pass / 0 fail / 8850 skip
  / 20439 total**. CI `29113667245` and `test262-full` `29113667267`
  confirm the change; downloaded artifacts report **25165 pass / 6830 fail /
  11 timeout / 0 error / 16461 skip / 48467 total / 32006 executed**, or
  **78.6%** of executed files and **51.9%** of the matrix.
- `FinalizationRegistry` is removed from the global unsupported-feature gate.
  Its exact built-in path reports **47 pass / 0 fail / 0 skip**, the completed
  receiver-brand coverage raises `built-ins/WeakRef/` to **29 pass / 0 fail /
  0 skip**, and the related `Object.seal` test passes. The supported language
  subset remains **11589 pass / 0 fail / 8850 skip / 20439 total**. CI
  `29111665821` and `test262-full` `29111666010` confirm the change; downloaded
  artifacts report **25105 pass / 6830 fail / 11 timeout / 0 error / 16521
  skip / 48467 total / 31946 executed**, or **78.6%** of executed files and
  **51.8%** of the matrix.
- The runner and analyzer now admit `built-ins/WeakRef/` through an exact-path
  feature exception. The focused path reports **28 pass / 0 fail / 1 skip / 29
  total**; the remaining test requires `FinalizationRegistry` solely to check
  `deref()` receiver branding. The supported language subset remains **11589
  pass / 0 fail / 8850 skip / 20439 total**. CI `29110157712` and
  `test262-full` `29110157754` confirm the change; downloaded artifacts report
  **25056 pass / 6830 fail / 11 timeout / 0 error / 16570 skip / 48467 total /
  31897 executed**, or **78.6%** of executed files and **51.7%** of the matrix.
- The complete `language/expressions/optional-chaining/` path is admitted at
  **38 pass / 0 fail / 0 skip**, including its async cases. Class-elements now
  also admit optional-chaining and destructuring coverage, reaching **2957
  pass / 0 fail / 5 skip / 2962 total**. Together these add 44 tests to the
  supported subset at **11589 pass / 0 fail / 8850 skip / 20439 total**. CI
  `29108590113` and `test262-full` `29108590134` confirm the change; the full
  aggregate is **25028 pass / 6830 fail / 11 timeout / 0 error / 16598 skip /
  48467 total / 31869 ran**, or **78.5%** of executed files and **51.6%** of
  the matrix.
- The class-elements paths now admit their fully green `Symbol`,
  `Symbol.iterator`, and `Symbol.asyncIterator` coverage. The focused paths
  reach **2951 pass / 0 fail / 11 skip / 2962 total**, adding 256 tests to the
  supported subset at **11545 pass / 0 fail / 8894 skip / 20439 total** while
  keeping the exception scoped to class elements. CI `29106363624` and
  `test262-full` `29106363581` confirm the admission; the full aggregate is
  **24984 pass / 6830 fail / 11 timeout / 0 error / 16642 skip / 48467 total /
  31825 ran**, or **78.5%** of executed files and **51.5%** of the matrix.
- The fully green `language/expressions/await/` path is now admitted by default
  at **22 pass / 0 fail / 0 skip**. Together with resumable ordinary async
  functions, the supported subset reaches **11266 pass / 0 fail / 9173 skip /
  20439 total**. CI `29103907303` and
  `test262-full` `29103907305` confirm the change; the full aggregate is
  **24705 pass / 6830 fail / 11 timeout / 0 error / 16921 skip / 48467 total /
  31546 ran**, or **78.3%** of executed files and **51.0%** of the matrix.
- The fully green `language/statements/for-await-of/` async-iteration slice is
  now admitted by default at **23 pass / 0 fail / 1211 skip / 1234 total**.
  The supported subset reaches **11289 pass / 0 fail / 9150 skip / 20439
  total**. CI `29105422326` and `test262-full` `29105422278` confirm the
  change; the full aggregate is **24728 pass / 6830 fail / 11 timeout / 0
  error / 16898 skip / 48467 total / 31569 ran**, or **78.3%** of executed
  files and **51.0%** of the matrix.
- Async completion is now admitted on the fully green
  `language/statements/class/definition/` path. Its two async `super` method
  tests move the focused path to **65 pass / 0 fail / 0 skip**, and the
  supported subset reaches **11251 pass / 0 fail / 9188 skip / 20439 total**;
  unrelated async paths remain gated. The admission is confirmed by CI
  `29102497188` and `test262-full` `29102497174`.
- Async-function result assimilation and scoped class-element async admission
  are confirmed by CI `29101286102` and `test262-full` `29101286000`; the
  supported-summary follow-up is confirmed by CI `29101459432` and
  `test262-full` `29101459422`. The latest full aggregate is **24690 pass /
  6830 fail / 9 timeout / 0 error / 16938 skip / 48467 total / 31520 ran**,
  or **78.3%** of executed files and **50.9%** of the matrix.
- Async execution is now admitted on the fully green
  `language/{expressions,statements}/class/elements/` paths. Combined with the
  async-generator admission, the supported subset reaches **11249 pass / 0
  fail / 9190 skip / 20439 total**; async paths outside the exact admissions
  remain gated.
- The runner and analyzer now admit the fully green async-generator statement
  and expression paths. Their async and feature exceptions remain scoped to
  `language/{expressions,statements}/async-generator/`. The supported subset
  rises to **10761 pass / 0 fail / 9678 skip / 20439 total**.
- The iterator-result descriptor fix is confirmed by `test262-full`
  29096575687 on `42ac4c4` with no engine-outcome regressions.
- The primary CI job now builds, tests, and runs clippy across all Cargo targets
  and features, so optional interop, examples, and benchmarks cannot silently
  stop compiling.
- The async-generator receiver-brand fix is confirmed by `test262-full`
  29095756104 on `43cc099` at **23277 pass / 6830 fail / 11 timeout / 0
  error / 18349 skip / 48467 total / 30107 ran**, retaining **77.3%** of
  executed files and **48.0%** of the matrix. Engine outcomes are unchanged
  apart from the six opt-in async receiver diagnostics.
- The async iterator-kind semantic fix is confirmed by `test262-full`
  29094206133 on `a906242` at **23277 pass / 6830 fail / 11 timeout / 0
  error / 18349 skip / 48467 total / 30107 ran**, retaining **77.3%** of
  executed files. The current upstream snapshot restores 749 unsupported,
  skip-only files, so the all-matrix rate changes from **48.8%** to **48.0%**
  without an engine outcome change.
- The async function-form admission is confirmed by the full matrix at
  **23277 pass / 6830 fail / 11 timeout / 0 error / 17600 skip / 47718 total /
  30107 ran**, or **77.3%** of executed files and **48.8%** of the current
  matrix. The preceding async arrow-function admission reported **23142 pass /
  6830 fail / 17735 skip / 29972 ran**.
- The preceding `test262-full` documentation confirmation recorded an upstream
  denominator contraction of 749 unsupported files. Engine outcomes remain
  **22999 pass / 6830 fail / 11 timeout / 0 error / 29829 ran**; the current
  snapshot reports **17878 skip / 47718 total** instead of treating the
  denominator-only change as an engine improvement.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit
  `generators` and `Symbol.iterator` only on
  `language/expressions/yield/`, where the complete synchronous path is green.
- The runner and analyzer now share test262 async harness assembly and process
  classification. Async files receive a host `print` shim plus
  `doneprintHandle.js`; exactly one completion marker is required, failure,
  duplicate, missing, unexpected-output, process-error, and timeout outcomes
  remain distinct. Async execution stays gated unless an exact path is
  explicitly admitted, with `TEST262_RUN_ASYNC=1` available for broader
  diagnostics, and CI runs focused tooling regression tests.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  complete synchronous, parse-negative, and async object method-definition
  coverage only on `language/expressions/object/method-definition/`. Async
  files remain gated everywhere else unless an opt-in diagnostic is requested.
- The runner and analyzer now admit `async-functions`, default parameters, and
  async completion only on `language/expressions/async-arrow-function/`, while
  preserving those gates outside the exact path.
- The runner and analyzer now admit `async-functions`, default parameters, and
  async completion only on `language/{expressions,statements}/async-function/`;
  async function coverage outside those exact paths remains gated.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented default-parameter, destructuring-binding, generator,
  object-rest, and `Symbol.iterator` coverage only on
  `language/expressions/arrow-function/`; those feature gates remain active
  outside the ordinary arrow-function path.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented default-parameter, destructuring-binding, generator,
  object-rest, private-field early-error, and `Symbol.iterator` coverage only
  on `language/{statements,expressions}/function/`; those feature gates remain
  active outside the two ordinary function paths.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented generator, default-parameter, destructuring-binding, object-rest,
  Symbol, and `Symbol.iterator` coverage only on
  `language/{statements,expressions}/generators/`; unrelated feature gates in
  those trees remain active.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit
  implemented generator and async-function coverage only on
  `language/statements/class/definition/`; those feature gates remain in
  place outside that path.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now honor the
  test262 `raw` flag by running those files without harness/include prelude
  injection, so directive-prologue parse-negative tests are evaluated from
  their real source start.
- `tools/test262_analyze.py` now mirrors the runner's handling of
  `onlyStrict` tests and indented `negative:` metadata, so strict-mode and
  parse-negative test262 files are no longer reported as false failure
  buckets during focused analysis.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  Symbol-key function-name inference tests without unskipping broader Symbol
  iterator coverage, keeping the unsupported-feature boundary narrow.
- `tools/test262_runner.py` now admits the Symbol-backed object-spread
  generated tests in array/call/new expression contexts without unskipping
  broader Symbol iterator coverage.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit
  implemented `language/statements/for-of/` destructuring, object rest,
  optional-chaining, Proxy, and `Symbol.iterator` coverage without unskipping
  generator coverage.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the four
  implemented `language/statements/for-of/generator-close-via-*.js` files
  without opening broader generator coverage.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented `built-ins/TypedArrayConstructors/` coverage by lifting only the
  TypedArray, concrete TypedArray constructor, ArrayBuffer, DataView, Reflect,
  Proxy, Symbol, well-known Symbol, and generator feature gates needed by that
  path.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented `built-ins/ArrayBuffer/` coverage by lifting only the
  ArrayBuffer, `Reflect.construct`, and Symbol feature gates needed by that
  path.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented `built-ins/DataView/` coverage by lifting only the DataView,
  ArrayBuffer, Float16Array, Reflect, and typed-array helper feature gates
  needed by that path.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented `built-ins/Error/prototype/stack/` coverage by lifting only the
  Error stack accessor, Proxy, Reflect, and `Reflect.construct` feature gates
  needed by that path.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented `language/statements/with/` coverage by lifting only Proxy,
  Reflect, TypedArray, generator, async function, and async iteration gates on
  the `with` statement path.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented `language/expressions/assignment/` destructuring assignment
  coverage by lifting only the `destructuring-binding`, `object-rest`, Symbol,
  `Symbol.iterator`, and Proxy gates on that path.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented private-field Reference coverage for
  `language/expressions/compound-assignment/` and
  `language/expressions/logical-assignment/` by lifting only
  `class-fields-private` on those paths.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit
  implemented public and private instance/static class fields and methods,
  private-name `in`, generator methods, async methods, and async generator
  methods across `language/{statements,expressions}/class/elements/`. Proxy
  coverage is also admitted on those paths now that private elements can be
  stamped directly onto Proxy receivers without invoking handler traps. These
  feature gates remain in place outside the class-elements paths. The obsolete
  direct-eval, contextual-keyword, and initialization-order file allowlists
  have been removed.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented Symbol, Symbol iterator, TypedArray, WeakMap/WeakSet, Proxy,
  async-function, generator, and async-iteration coverage only on
  `language/statements/class/subclass/`; those feature gates remain in place
  outside the subclass path.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented AggregateError, ArrayBuffer/DataView/TypedArray, Promise, and
  WeakMap/WeakSet coverage only on the statement and expression
  `class/subclass-builtins/` paths. SharedArrayBuffer and WeakRef remain
  skipped because those globals are not implemented yet.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the five
  implemented `error-cause` files for `Error`, NativeError, and `AggregateError`
  without unskipping broader AggregateError coverage.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented `built-ins/AggregateError/` coverage by lifting only the
  AggregateError, `error-cause`, Symbol iterator, Reflect constructor, Symbol,
  and Reflect feature gates on that path.
- `tools/test262_runner.py` and `tools/test262_analyze.py` now admit the
  implemented Error and NativeError `proto-from-ctor-realm.js` files by lifting
  only the Reflect/Symbol gates on those exact files.
- `tools/test262_analyze.py` now mirrors the runner's unsupported-feature
  boundary exactly, and `tools/analyze_failures.py` passes test paths into
  `should_skip()`, so path-scoped feature exceptions are not hidden during
  focused diagnostics.

### Runtime hardening

- Optional serde interop now handles internal PrivateName and Reference values
  using the module's existing non-JSON-value policy (`null`), removes stale
  imports, and compiles cleanly with the current `Value` enum. The serde embed
  example also handles VM/function-registration failures correctly and runs
  end to end again.
- Iterator result objects created for generators, async generators, Array and
  collection iterators, RegExp String iterators, and delegation now expose
  `value` and `done` as writable, enumerable, configurable data properties, as
  required by `CreateIterResultObject`. `Object.keys`, object spread,
  `JSON.stringify`, and descriptor inspection no longer hide both fields.
- `%AsyncGeneratorPrototype%` `next`, `return`, and `throw` now validate the
  async-generator internal brand before resuming. Incompatible primitive,
  ordinary object/function, async-generator function/prototype, and synchronous
  generator receivers return a rejected Promise containing a native
  `TypeError` instead of throwing synchronously, resolving spuriously, or
  reaching the lazy-generator panic path.
- Async generators now use distinct `%AsyncIteratorPrototype%`,
  `%AsyncGeneratorPrototype%`, and `%AsyncGeneratorFunction.prototype%`
  intrinsics instead of aliasing synchronous generator/Object prototypes.
  Internal iterators also retain whether they are Async-from-Sync adapters, so
  adapter values are awaited while manually implemented async iterators keep
  yielded Promise identity.
- Async generators now await ordinary yielded and returned values before
  creating their outer iterator-result object. A rejected yielded value is
  thrown back into the suspended body so surrounding `catch` blocks can
  recover; otherwise the request rejects and the generator closes. Interpreted
  async functions now reject with native JavaScript Error objects for VM
  `ReferenceError`/`SyntaxError` failures while preserving explicit throw
  values and propagating non-catchable host aborts.
- `await` now assimilates generic thenables through their observable `then`
  getter and call, including rejection and abrupt completion. Async `yield*`
  awaits delegated iterator results and values, then creates a fresh outer
  iterator-result object; synchronous `yield*` continues forwarding the raw
  delegated result without observing its `value` getter.
- `String.prototype.match`, `replace`, `replaceAll`, and `split` now consult
  their well-known Symbol dispatch hooks only when the pattern/separator is an
  object. Primitive String, Number, Boolean, and BigInt values proceed through
  direct string conversion without touching prototype Symbol getters, matching
  the current specification and test262.
- `yield*` now runs as a VM delegation state machine that forwards the inner
  iterator's result object unchanged and routes outer `next`, `throw`, and
  `return` completions through the corresponding inner iterator methods. It
  validates iterator result objects, preserves final values, performs missing
  `throw` cleanup through `return`, and observes well-known Symbol properties
  through boxed primitive prototype chains. Async generators prefer
  `Symbol.asyncIterator`, fall back to `Symbol.iterator`, await delegated
  results, and convert protocol errors into rejected iterator-result Promises.
- Object literal method parsing no longer treats an escaped identifier such as
  `\u0061sync` as the contextual `async` method prefix. Escaped `async`
  followed by another method name is now rejected during parsing, while an
  ordinary method, property, or shorthand property actually named `async`
  remains valid.
- Formal-parameter parsing now disables both `YieldExpression` and
  `AwaitExpression` for generator/async functions and arrow functions while
  resetting those grammar contexts inside nested function and arrow bodies.
  Expression-bodied ordinary arrows in generator parameter initializers may
  therefore use sloppy `yield` as an identifier, while actual arrow defaults
  still reject outer `yield`/`await` expressions. Parenthesized non-arrow
  assignments continue to parse in their enclosing generator/async context.
- Generator formal parameters now reject duplicate bound names in non-simple
  parameter lists and reject `YieldExpression` before generator execution can
  begin. Sloppy direct eval in a parameter initializer now raises `SyntaxError`
  when an introduced `var` conflicts with that parameter binding, while
  non-conflicting eval variables remain visible to the generator body.
- `%GeneratorFunction.prototype%.prototype` now exposes the intrinsic
  GeneratorPrototype with the required descriptor and supplies the fallback
  prototype when a generator function has a non-object `prototype` value.
  Generator and async functions inherit Function.prototype's restricted
  `caller`/`arguments` accessors instead of taking the sloppy ordinary-function
  legacy path, and FunctionExpression names use their own Yield/Await grammar
  parameters rather than the surrounding generator context.
- Generator `yield` now parses at AssignmentExpression precedence, honors the
  no-LineTerminator restriction before its operand and `*`, recognizes omitted
  operands before conditional/template delimiters, and rejects an
  unparenthesized nested `yield` in higher-precedence expressions.
- The VM call-stack guard now trips at a more conservative depth before
  recursive interpreted calls can exhaust smaller debug/CI thread stacks,
  preserving the catchable `RangeError` behavior for runaway recursion.

### test262 conformance improvements

Supported-subset pass rate: **100.0%** (up from 88.6%).
Current supported subset count: **9838 pass / 0 fail / 10601 skip / 0 timeout**.

- **Async generator iterator-kind diagnostic**: with the two async-generator
  paths fully opened only for diagnostics, distinct async intrinsics and
  Async-from-Sync tracking improve the 924-file run from **919 pass / 5 fail**
  to **921 pass / 3 fail**. The remaining failures are microtask-order checks;
  the default supported subset remains unchanged until they are fixed.
- **Async generator receiver-brand diagnostic**: the 84-file
  `AsyncGeneratorFunction`, `AsyncGeneratorPrototype`, and
  `AsyncIteratorPrototype` built-in cluster improves from **57 pass / 27 fail**
  to **63 pass / 21 fail** after all six incompatible-receiver tests begin
  rejecting with `TypeError`. The 301-file statement async-generator path
  remains at **298 pass / 3 fail**; its residual failures and the request-order
  built-in cases require the planned async-generator request queue.
- **Primitive String protocol dispatch guards**: the 16 current test262 cases
  covering primitive values passed to `match`, `replace`, `replaceAll`, and
  `split` now avoid inherited Symbol hook access. The full admitted
  `built-ins/String` comparison reports **1130 admitted files / 0 status
  regressions** against the prior green baseline.
- **Async object method-definition diagnostic**: with async execution enabled,
  the complete 303-file object method-definition path first improved from
  **238 pass / 65 fail** to **286 pass / 17 fail** with async-generator
  delegation, then to **296 pass / 7 fail** after thenable assimilation and
  async delegated-result rewrapping, and finally to **303 pass / 0 fail** after
  ordinary async-generator yield awaiting and native async rejection error
  object fixes.
- **Complete async object method-definition admission**: the exact path now
  runs all **303 files with 303 pass / 0 fail / 0 skip** by default. The 101
  newly admitted async files raise the supported subset to **9661 pass / 0
  fail / 10778 skip / 20439 total** while all other async paths retain their
  existing gate.
- **Complete async arrow-function admission**: the exact 60-file path moves
  from **18 pass / 0 fail / 42 skip** to **60 pass / 0 fail / 0 skip** under
  the default runner. The supported subset rises to **9703 pass / 0 fail /
  10736 skip / 20439 total** without opening async coverage elsewhere.
- **Complete async function-form admission**: the 93 expression and 74
  declaration files move together from **32 pass / 0 fail / 135 skip** to
  **167 pass / 0 fail / 0 skip**. The supported subset rises to **9838 pass /
  0 fail / 10601 skip / 20439 total** while preserving all outside gates.
- **Complete synchronous `yield*` semantics and admission**: delegated
  iteration now preserves raw result objects, completion values, method
  receiver/arguments, getter and call errors, protocol-violation cleanup, and
  primitive `Symbol.iterator` lookup. `language/expressions/yield` improves
  from **34 pass / 29 fail** under the prior relaxed diagnostic to **63 pass /
  0 fail / 0 skip** under the default runner, raising the supported subset to
  **9560 pass / 0 fail / 10878 skip**.
- **Synchronous object method-definition admission**: the focused object
  method-definition path now exercises implemented async/generator grammar,
  class-element contexts, default parameters, and Symbol-backed computed
  names. It rises from **52 pass / 0 fail / 251 skip** to **202 pass / 0 fail /
  101 skip**; every remaining skip carries the test262 `async` flag. The
  supported subset rises to **9497 pass / 0 fail / 10941 skip**.
- **Escaped object async-method prefix early error**: object literal method
  parsing now distinguishes the exact contextual `async` token from an
  escaped identifier. With the relevant method-definition feature skips
  temporarily lifted, the focused path improves from **158 pass / 1 fail /
  144 skip** to **159 pass / 0 fail / 144 skip**.
- **Complete ordinary arrow-function admission**: the ordinary arrow path now
  exercises all implemented default-parameter, destructuring-binding, nested
  generator, object-rest, and `Symbol.iterator` cases. It rises from **144
  pass / 0 fail / 199 skip** to **343 pass / 0 fail / 0 skip**, and the
  supported subset rises to **9347 pass / 0 fail / 11091 skip**.
- **Complete ordinary function declaration/expression admission**: the two
  ordinary function paths now exercise all implemented default-parameter,
  destructuring-binding, nested generator, object-rest, private-name
  early-error, and `Symbol.iterator` cases. They rise from **307 pass / 0 fail
  / 408 skip** to **715 pass / 0 fail / 0 skip**, and the supported subset
  rises to **9148 pass / 0 fail / 11290 skip**.
- **Complete generator statement/expression admission**: non-simple formal
  parameters reject duplicate bound names, generator parameter initializers
  reject `yield`, and sloppy direct eval rejects a `var` that conflicts with a
  parameter binding before the generator body runs. The runner now admits the
  implemented default-parameter, destructuring-binding, object-rest, Symbol,
  and `Symbol.iterator` coverage on the two generator paths. They rise from
  **155 pass / 0 fail / 401 skip** to **556 pass / 0 fail / 0 skip**, and the
  supported subset rises to **8740 pass / 0 fail / 11698 skip**.
- **Generator function intrinsic and binding semantics**: generator objects
  now use the intrinsic GeneratorPrototype fallback, generator/async functions
  inherit restricted `caller` and `arguments` accessors, ordinary nested
  FunctionExpressions may use a sloppy `yield` name, and GeneratorExpressions
  reject that name. The statement/expression generator paths move from **149
  pass / 6 fail / 401 skip** with the generator gate temporarily lifted to
  **155 pass / 0 fail / 401 skip** under the default runner. The supported
  subset rises to **8339 pass / 0 fail / 12099 skip**.
- **Generator yield grammar in class definitions**: `yield` now obeys its
  AssignmentExpression-level grammar in class generator methods, including
  bare yields before line terminators, conditional colons, and template
  substitution tails. Newlines before `yield*` and unparenthesized nested
  yields are rejected during parsing. `language/statements/class/definition`
  rises from **38 pass / 0 fail / 27 skip** to **63 pass / 0 fail / 2 skip**,
  and the supported subset rises to **8186 pass / 0 fail / 12252 skip**.
- **Built-in subclass coverage admission**: generated class declaration and
  expression tests now exercise subclass construction for the implemented
  AggregateError, ArrayBuffer/DataView/TypedArray, Promise, and WeakMap/WeakSet
  families. The two `class/subclass-builtins/` paths report **68 pass / 0 fail
  / 4 skip**; only the SharedArrayBuffer and WeakRef declaration/expression
  pairs remain skipped. The supported subset rises to **8161 pass / 0 fail /
  12277 skip**.
- **Subclass constructor classification**: interpreted async functions,
  generators, and async generators are now consistently non-constructors;
  async functions no longer receive an own `prototype`, while generator
  functions retain theirs. `IsConstructor` rejection occurs before superclass
  `prototype` lookup through direct, bound, and Proxy values. The Symbol
  intrinsic carries the constructor identity required for class heritage but
  rejects construction through `new.target`. The complete
  `language/statements/class/subclass/` path reports **109 pass / 0 fail / 0
  skip**, and the supported subset rises to **8127 pass / 0 fail / 12311
  skip**.
- **Private elements on Proxy and exotic receivers**: ECMAScript private
  elements now belong to the common GC cell rather than only ordinary-object
  and function payloads. Derived constructors can stamp fields, methods, and
  accessors onto Proxy, revoked Proxy, Array, collection, Promise,
  ArrayBuffer/DataView/TypedArray, iterator, and function receivers without
  forwarding private access to Proxy traps. GC traces cell-owned private
  values and clears brands when cells are reclaimed. The three private Proxy
  test262 files pass, class-elements reports **2207 pass / 0 fail / 755
  skip**, and the supported subset rises to **8113 pass / 0 fail / 12325
  skip**.
- **Private assignment-target References**: private names are now represented
  directly in VM `ReferenceRecord`s, and private reads/writes flow through
  `GetValue`/`PutValue`. Destructuring and `for-in`/`for-of` preserve the
  private target before source access, defer brand checks until the write, and
  raise `TypeError` for missing private slots. All 14 `privatefieldset-*`
  files pass, the full class-elements paths report **2203 pass / 0 fail / 759
  skip**, and broad private class-element admission raises the supported
  subset to **8109 pass / 0 fail / 12329 skip**.
- **Instance private method initialization order**: constructors now install
  all instance private methods and accessors before running any public or
  private field initializer, while preserving field source order and the
  derived-constructor `super()` boundary. The focused class-elements run
  reports **555 pass / 0 fail / 2407 skip**, the relaxed private-class
  diagnostic improves to **2195 pass / 8 fail / 759 skip**, and the supported
  subset rises to **6461 pass / 0 fail / 13977 skip**.
- **Private async/generator contextual-keyword early errors**: escaped
  `await` is now rejected as a binding, identifier reference, or label inside
  async method bodies, while a bare `yield` cannot be parsed as the direct
  operand of a unary expression inside generator method bodies. Nested
  ordinary functions and escaped property names retain their valid behavior.
  The runner admits the 32 matching private method parse-negative files; the
  focused class-elements run reports **553 pass / 0 fail / 2409 skip**, and
  the supported subset rises to **6459 pass / 0 fail / 13979 skip**.
- **Private-name direct eval visibility**: direct eval parsing now inherits
  private names visible through the caller's class environment, so
  `eval("this.#m")` is accepted in class methods, instance field
  initializers, private accessors, private methods, and static private
  elements while preserving runtime private-name identity checks. The runner
  now admits the 12 implemented
  `language/statements/class/elements/private-*-visible-to-direct-eval*.js`
  files. The focused `language/{statements,expressions}/class/elements` run
  now reports **521 pass / 0 fail / 2441 skip**, and the supported subset
  rises to **6427 pass / 0 fail / 14011 skip**.
- **Class special-method constructor early errors**: class parsing now rejects
  instance `async constructor()`, `* constructor()`, and
  `async * constructor()` methods as `SyntaxError`, while still allowing static
  async/generator methods named `constructor`. The class-elements runner
  exception now admits implemented generator, async method, and async generator
  method coverage on the class-elements paths. The focused
  `language/{statements,expressions}/class/elements` run now reports **509 pass
  / 0 fail / 2453 skip**, and the supported subset rises to **6415 pass / 0
  fail / 14023 skip**.
- **Class field initializer arrow `super` context**: public class field
  initializers now parse with the class field initializer `super` and
  `new.target` context, allowing `super.prop` while continuing to reject
  `super()` calls. Static public/private field initializer scopes now also
  bind `#super` to the constructor, so arrows created by static field
  initializers resolve `super.staticProp` through the class constructor home
  object. The focused `language/{statements,expressions}/class/elements` run
  now reports **422 pass / 0 fail / 2540 skip**, and the supported subset rises
  to **6328 pass / 0 fail / 14110 skip** after admitting the implemented public
  class field coverage.
- **Class field initializer direct eval context**: direct eval calls emitted
  from public/private class field initializer values now carry initializer
  context through nested function chunks. Eval source containing `arguments`
  is rejected as `SyntaxError`, `super()` is disallowed while `super.prop`
  remains valid, and `new.target` evaluates as `undefined` for instance field
  initializers instead of inheriting the constructor's `new.target`. With
  public class field skips temporarily lifted, the direct-eval class-elements
  slice reports **82 pass / 0 fail / 94 skip**.
- **For-of runner admission**: the `language/statements/for-of/` path
  exception now admits implemented destructuring-binding, object-rest,
  optional-chaining, Proxy, and `Symbol.iterator` coverage without opening
  broad generator coverage. The focused `for-of` path now reports **598 pass /
  0 fail / 153 skip** after admitting the four implemented generator-close
  files.
- **Generator close via `for-of` abrupt completion**: generator `return()`
  resumes suspended generators with a return completion instead of marking them
  done immediately, so active `finally` blocks run when `for-of` exits through
  `break`, `continue`, `return`, or `throw`. Generator suspension now preserves
  active `finally` guards and pending finally completions across yields.
- **`with` runner admission**: the `language/statements/with/` path exception
  now admits the remaining implemented object-environment coverage, including
  TypedArray prototype-chain binding deletion and async/generator declaration
  parse-negative files, without unskipping those features more broadly. The
  `with` path now closes at **181 pass / 0 fail / 0 skip**.
- **Object rest assignment primitive sources and early errors**: object
  destructuring assignment now rejects object rest elements followed by another
  assignment property, and object rest copy now boxes primitive sources before
  enumerating own properties. This lets `({...rest} = "foo")` copy string
  indices while keeping nullish sources as `TypeError`.
- **Optional-chain assignment target early errors**: optional member/call
  chains now preserve enough parser-side chain-boundary state to reject direct
  and chained optional expressions as assignment, update, destructuring, and
  `for-in` targets while preserving parenthesized member targets such as
  `(a?.b).c = 1`.
- **Assignment destructuring runner admission**: the
  `language/expressions/assignment/` path exception now admits the implemented
  destructuring assignment, object rest, optional-chaining parse-negative,
  Symbol-key, `Symbol.iterator`, and Proxy coverage without opening generator
  coverage more broadly. Array assignment patterns now skip `IteratorClose`
  when iterator stepping itself throws while still closing for target/default
  abrupt completions. `language/expressions/assignment` reports **453 pass / 0
  fail / 32 skip**.
- **Generator assignment destructuring parser coverage**: generator assignment
  destructuring now parses bare `yield` before a closing array pattern bracket
  as a `YieldExpression`, and rejects generator shorthand `{ yield }`
  assignment targets as syntax errors. Array assignment destructuring now
  preserves iterator-close errors when a suspended generator is resumed through
  `return()`, while still preserving original throw completions. Lazy custom
  iterator `next` validation is delayed until `IteratorNext`, so target
  reference evaluation can suspend before a missing `next` is observed. The
  runner now admits generator coverage on the whole
  `language/expressions/assignment/` path, which reports **485 pass / 0 fail /
  0 skip**; the broader Reference-adjacent cluster reports **1198 pass / 0 fail
  / 0 skip**.
- **Private-field Reference runner admission**: compound and logical assignment
  paths now admit the already-implemented private-field Reference coverage
  without opening private class fields more broadly. The combined
  `language/expressions/compound-assignment
  language/expressions/logical-assignment` run reports **532 pass / 0 fail / 0
  skip**. The Reference-adjacent cluster
  `language/expressions/assignment language/expressions/compound-assignment
  language/expressions/logical-assignment language/expressions/update
  language/statements/with` reports **1198 pass / 0 fail / 0 skip**.
- **Error `cause` semantics**: `Error` and NativeError constructors now perform
  observable `HasProperty(options, "cause")` followed by `Get`, after installing
  the message property. `AggregateError` now uses its `(errors, message,
  options)` signature, reports `length === 2`, creates a non-enumerable `errors`
  array, and shares the same `InstallErrorCause` path. The five `error-cause`
  test262 files now pass. Error-family constructors now also apply
  `GetPrototypeFromConstructor(newTarget, "%<ErrorName>.prototype%")` when
  `newTarget.prototype` is not an object, including cross-realm `newTarget`
  functions. The focused built-ins run reports `built-ins/Error` at **87 pass /
  0 fail / 6 skip**, `built-ins/NativeErrors` at **87 pass / 0 fail / 7
  skip**, and `built-ins/AggregateError` closed at **25 pass / 0 fail / 0
  skip**.
- **Error stack accessor runner admission**: the
  `built-ins/Error/prototype/stack/` path exception now admits the
  already-implemented stack accessor coverage, including Proxy receivers and
  `Reflect.construct` constructor checks. The normal `built-ins/Error` runner
  now reports **83 pass / 0 fail / 10 skip**.
- **ArrayBuffer detached accessor admission**:
  `%ArrayBuffer.prototype.detached%` is now exposed as a spec-shaped accessor
  returning whether the receiver's backing store has been detached, while
  rejecting non-ArrayBuffer receivers. The runner now executes implemented
  `built-ins/ArrayBuffer/` coverage under a path-scoped feature exception at
  **122 pass / 0 fail / 99 skip**; remaining skips stay behind
  SharedArrayBuffer, resizable ArrayBuffer, DataView, and typed-array helper
  coverage.
- **TypedArray generator object-argument admission**: the
  `built-ins/TypedArrayConstructors/` path exception now admits generator
  metadata for the already-supported iterable constructor path, covering
  generator abrupt completion during `IterableToArrayLike`. The normal runner
  now reports **674 pass / 0 fail / 64 skip** on that path.
- **Mapped arguments object index writes**: property-Reference writes to sloppy
  mapped arguments objects now update the linked parameter binding, including
  writes after `Object.defineProperty(arguments, "0", ...)`. Dense arguments
  indices are also treated as own data properties during `[[Set]]`, so
  prototype numeric setters no longer intercept writes to `arguments[0]`. The
  focused `language/arguments-object` test262 run now reports **126 pass / 0
  fail / 137 skip**.
- **Object destructuring `RequireObjectCoercible`**: empty object assignment
  patterns such as `({} = null)` and rest-only object assignment patterns now
  throw `TypeError` for nullish sources instead of completing silently. With
  only the `destructuring-binding` skip lifted for diagnostics, the focused
  `language/expressions/assignment/dstr` run improves to **249 pass / 11 fail
  / 108 skip** while the default supported subset remains green.
- **Array rest assignment pattern early errors**: assignment destructuring now
  rejects array rest elements followed by another element, elision, another
  rest element, a trailing comma, or an initializer. With only the
  `destructuring-binding` skip lifted for diagnostics, the focused
  `language/expressions/assignment/dstr` run improves to **254 pass / 6 fail /
  108 skip** while the default supported subset remains green.
- **Object shorthand destructuring default function names**: object assignment
  shorthand defaults such as `{ fn = function() {} } = source` now apply
  `SetFunctionName` when the default initializer is an anonymous function,
  arrow function, class, or parenthesized anonymous function. With only the
  `destructuring-binding` skip lifted for diagnostics, the focused
  `language/expressions/assignment/dstr` run improves to **258 pass / 2 fail /
  108 skip** while the default supported subset remains green.
- **Array cover grammar for nested object assignment defaults**: array literals
  that may become assignment patterns now defer nested object-literal shorthand
  initializer early errors until the outer assignment decision is known. This
  lets sloppy nested object defaults such as `[{ x = yield }] = vals` and
  `[...{ x = yield }] = vals` treat `yield` as an identifier in assignment
  patterns while ordinary array literals still reject `{x = ...}`. With only
  the `destructuring-binding` skip lifted for diagnostics, the focused
  `language/expressions/assignment/dstr` run now closes at **260 pass / 0 fail
  / 108 skip**.
- **Binding destructuring default function names**: declaration and `for`
  binding patterns now apply `SetFunctionName` when a direct binding
  identifier's default initializer is an anonymous function, arrow function,
  class, or parenthesized anonymous function. With only the
  `destructuring-binding` skip lifted for diagnostics,
  `language/statements/{variable,let,const,for}/dstr` now closes at **412 pass
  / 0 fail / 156 skip** while the default supported subset remains green.
- **For-in/of array rest assignment-pattern early errors**: non-declaration
  `for` heads now reject array assignment patterns where a rest element is
  followed by a comma or elision before `in`/`of`, matching the ordinary
  assignment-pattern early error. With only the `destructuring-binding` skip
  lifted for diagnostics, `language/statements/for-in/dstr` now closes at
  **27 pass / 0 fail / 6 skip**, and `language/statements/for-of/dstr`
  improves to **417 pass / 32 fail / 120 skip** while the default supported
  subset remains green.
- **For-of assignment-pattern cover defaults**: non-declaration `for-of`
  heads now keep object shorthand defaults and nested object defaults in cover
  grammar until the `of` decision is known, so assignment patterns such as
  `for ({ x = 1 } of values)` and `for ([{ x = yield }] of values)` parse and
  execute through the destructuring assignment path. With only the
  `destructuring-binding` skip lifted for diagnostics,
  `language/statements/for-of/dstr` improves to **433 pass / 16 fail / 120
  skip** while the default supported subset remains green.
- **For-of assignment-pattern `in` initializers**: non-declaration `for-of`
  heads now distinguish a top-level `of` delimiter before parsing the left
  side, allowing `in` expressions inside array, shorthand object, and renamed
  object default initializers such as `for ([x = "x" in obj] of values)`.
  With only the `destructuring-binding` skip lifted for diagnostics,
  `language/statements/for-of/dstr` improves to **436 pass / 13 fail / 120
  skip** while the default supported subset remains green.
- **For-of destructuring IteratorClose semantics**: array binding patterns now
  close non-exhausted inner iterators on normal and non-iterator abrupt
  completion, array binding initialization now observes a missing
  `Array.prototype[Symbol.iterator]`, and non-declaration `for-of`
  assignment heads no longer close an inner iterator when that iterator's own
  step throws. With only the `destructuring-binding` skip lifted for
  diagnostics, `language/statements/for-of/dstr` now closes at **449 pass / 0
  fail / 120 skip** while the default supported subset remains green.
- **Property Reference records for simple member assignment**: ordinary member
  assignment now lowers final writes through an explicit property Reference
  while preserving simple-assignment ordering. `obj[x] = rhs` still evaluates
  the RHS before nullish-base failure and delays `ToPropertyKey` until after
  the RHS, while `PutValue` now handles the final write with Symbol keys, Proxy
  receiver identity, strict failed-write behavior, and primitive sloppy no-op
  semantics intact. The focused assignment/member-expression test262 cluster
  reports **204 pass / 0 fail / 282 skip**.
- **Property Reference records for destructuring member targets**:
  destructuring assignment targets such as `({ a: obj[key] } = rhs)` now finish
  member writes through `MakePropertyRefForSet` and `PutValue` instead of the
  legacy `SetElem`/`SetProp` opcodes. This preserves Symbol keys produced by
  `@@toPrimitive`, Proxy receiver identity, strict failed-write behavior, and
  primitive sloppy no-op semantics for member targets nested inside object and
  array assignment patterns. The focused `language/expressions/assignment/dstr`
  run remains closed at **90 pass / 0 fail / 278 skip**.
- **Property Reference records for `for-in`/`for-of` member heads**:
  non-declaration loop heads such as `for (obj[key] in source)` and
  `for (obj[key] of iterable)` now store through the property Reference
  `PutValue` path instead of direct `SetElem`/`SetProp` opcodes. This preserves
  Symbol keys and Proxy `set` receiver identity for loop-head assignment. The
  focused `language/statements/{for-in,for-of}` run reports **191 pass / 0 fail
  / 675 skip**.
- **Object rest destructuring assignment targets**: object rest assignment
  patterns such as `({ a, ...holder.rest } = rhs)` now validate the actual rest
  target and compile it through the destructuring assignment target path. Rest
  targets can now be identifiers or member expressions, including Proxy-backed
  member targets that rely on `PutValue` receiver identity. The focused
  `language/expressions/assignment/dstr` run remains closed at **90 pass / 0
  fail / 278 skip**.
- **Object rest computed-key exclusion**: object rest destructuring now excludes
  computed property names from the rest object after `ToPropertyKey`, including
  Symbol keys, and copies remaining enumerable Symbol properties into the rest
  result. This aligns `({ [key]: v, ...rest } = rhs)` and declaration patterns
  with ordinary object rest semantics. The focused
  `language/expressions/{assignment,destructuring}` run remains closed at
  **203 pass / 0 fail / 282 skip**.
- **Class field anonymous function names**: public and private class field
  initializers now apply `SetFunctionName` to anonymous function, arrow, and
  class values before defining the field. This gives static and instance field
  initializers names such as `"field"` and `"#field"` without changing
  non-anonymous initializer values. With class field skips temporarily lifted,
  `static-field-anonymous-function-name.js` now reports **2 pass / 0 fail / 0
  skip** across declaration and expression forms; the broader class-elements
  diagnostic now reports **1559 pass / 105 fail / 1298 skip** after the
  subsequent field early-error fixes.
- **Class field `ContainsArguments` early errors**: public and private class
  field initializers now reject `arguments` during parsing, including lexical
  occurrences inside arrows while preserving ordinary function-expression
  boundaries. With public/private class field and arrow gates temporarily
  lifted, the generated `*init-err-contains-arguments.js` class-elements
  cluster reports **60 pass / 0 fail / 0 skip**.
- **Class field `constructor` PropName early errors**: public instance and
  static fields whose non-computed literal name is `constructor` now fail during
  parsing, while computed `["constructor"]` fields remain valid and define data
  properties. With public/static class field and computed-name gates temporarily
  lifted, the focused constructor-PropName class-elements cluster reports
  **11 pass / 0 fail / 0 skip**.
- **Public class field computed-name records**: public instance and static
  field computed names now evaluate once during class definition and are stored
  as field keys for later `DefineField` execution. Instance field keys are no
  longer re-evaluated for each `new C()`, and static/instance public field keys
  now evaluate in field declaration order while initializers keep their existing
  static-vs-instance timing. With public/static class field and computed-name
  gates temporarily lifted, the focused incremental/intercalated/error
  computed-name class-elements cluster reports **12 pass / 0 fail / 0 skip**,
  and the broader class-elements diagnostic reports **1582 pass / 82 fail /
  1298 skip**. Remaining failures include full ordered class-element evaluation
  across computed methods and static blocks.
- **Ordered class element evaluation**: class parsing now preserves source
  order across methods, fields, and static blocks, and class compilation uses
  that order for public computed method names, public field computed names, and
  static element initialization. This fixes field-before-method computed-name
  ordering, allows computed method names to read the initialized inner class
  binding, and runs static block bodies and static field initializers in source
  order after all element names have been evaluated. With public/static class
  field and computed-name gates temporarily lifted, the generated
  `cpn-class-*-fields-methods-*` cluster reports **60 pass / 0 fail / 2
  skip**, and the broader class-elements diagnostic reports **1583 pass / 81
  fail / 1298 skip**.
- **TypedArray `[[HasProperty]]` prototype delegation**: ordinary property keys
  missing from a TypedArray now continue through the prototype's real
  `[[HasProperty]]` operation instead of raw own-property lookup. This preserves
  integer-indexed exotic handling for canonical numeric keys while propagating
  Proxy `has` traps on the prototype chain. With the Proxy gate temporarily
  lifted, `built-ins/TypedArrayConstructors/internals/HasProperty` reports
  **26 pass / 0 fail / 6 skip**.
- **Property Reference records for member logical assignment**: ordinary member
  logical assignments now preserve an explicit property Reference from
  `GetValue` through the conditional short-circuit and `PutValue` paths. This
  aligns `obj.x ||= rhs`, `obj[x] &&= rhs`, and `obj[x] ??= rhs` with the
  identifier Reference path while keeping short-circuit result values, computed
  key coercion count, Symbol keys, Proxy receiver identity, and strict
  failed-write behavior stable.
- **Property Reference records for member update expressions**: ordinary
  member update expressions now create an explicit property Reference and
  preserve it through `GetValue`, numeric increment/decrement, and `PutValue`.
  This aligns `obj.x++`, `++obj[x]`, and Symbol-keyed update targets with the
  identifier Reference path while preserving prefix/postfix result values,
  Proxy receiver identity, and strict failed-write behavior. The focused
  update-expression test262 cluster reports **138 pass / 0 fail / 4 skip**.
- **Property Reference records for member compound assignment**: ordinary
  member compound assignments now lower through the same spec
  `Reference -> GetValue -> PutValue` path as identifier compound
  assignments. The new property Reference path preserves Symbol keys, keeps
  computed keys single-evaluated, passes Proxy `set` traps the original
  receiver, and applies the Reference's strict flag when the final write
  fails. `language/expressions/compound-assignment` remains closed at **406
  pass / 0 fail / 48 skip**, with new Rust regression coverage for Proxy
  receiver and read-only-property strict/sloppy behavior.
- **Static class field initializer `this` binding**: static public and private
  field initializers now run with `this` bound to the class constructor, so
  `static g = this.f` and arrows created inside static initializers see the
  same receiver that `DefineField` passes to the initializer. With static
  public/private field skips temporarily lifted, the focused
  `static-field-init-with-this.js` and
  `static-field-init-this-inside-arrow-function.js` statement/expression
  tests now report **4 pass / 0 fail / 0 skip**. The
  `language/statements/class/elements` diagnostic improves from **466 pass /
  90 fail / 978 skip** to **469 pass / 87 fail / 978 skip**. The default
  supported subset remains **5099 pass / 0 fail / 0 timeout**.
- **Public class field `[[DefineOwnProperty]]` semantics**: public field
  initialization now routes through `CreateDataPropertyOrThrow` instead of
  raw property-map insertion. Fields therefore fail with `TypeError` when a
  previous initializer freezes the receiver and invoke Proxy `defineProperty`
  traps when a derived constructor returns a Proxy receiver. With public class
  field and Proxy skips temporarily lifted, the focused
  `class-field-on-frozen-objects.js` and
  `public-class-field-initialization-is-visible-to-proxy.js` tests now run at
  **2 pass / 0 fail / 0 skip**. The broader
  `language/{statements,expressions}/class/elements` diagnostic with public,
  private, and Proxy gates temporarily lifted reports **1460 pass / 184 fail /
  1318 skip**. The default supported subset remains **5099 pass / 0 fail / 0
  timeout**.
- **Private element duplicate initialization**: private field, private method,
  and private accessor initialization now rejects attempts to add the same
  class private name to the same receiver twice instead of overwriting the
  existing private slot. This matches derived-constructor cases where a base
  constructor returns an object that is reused across multiple subclass
  constructions. With private class feature skips temporarily lifted, the
  focused
  `language/statements/class/elements/private-method-double-initialisation*.js`
  and `privatefieldadd-typeerror.js` cluster now runs at **5 pass / 0 fail /
  0 skip**. The default supported subset remains **5099 pass / 0 fail / 0
  timeout**.
- **Public class fields baseline**: class bodies now parse public instance and
  static field declarations, including computed names and `static`/`get`/`set`/
  `async` names that are fields rather than method prefixes. Field
  initialization uses `DefineDataProperty`, so inherited setters are not
  invoked and uninitialized fields become own `undefined` data properties. With
  public class field skips temporarily lifted, the focused
  `language/{statements,expressions}/class/elements` diagnostic now reports
  **309 pass / 113 fail / 2540 skip**. The default supported subset remains
  **5099 pass / 0 fail / 0 timeout** because full public-field coverage is
  still gated behind direct-eval, computed-name ordering, and static
  initializer follow-ups.
- **DataView prototype and constructor ordering**: `DataView.prototype` now
  exposes spec-shaped `@@toStringTag`, `Reflect.construct(DataView, ...)`
  validates invalid byte offsets before reading `newTarget.prototype`, and
  rechecks detached ArrayBuffers after the observable prototype lookup. The
  runner now executes implemented `built-ins/DataView/` coverage under a
  path-scoped feature exception at **492 pass / 0 fail / 69 skip**; remaining
  skips stay behind SharedArrayBuffer and resizable ArrayBuffer coverage.
- **TypedArray constructors coverage lift**: the runner now executes the
  implemented `built-ins/TypedArrayConstructors/` tests under a path-scoped
  feature exception instead of requiring ad hoc temporary skip removal. The
  focused run reports **599 pass / 0 fail / 139 skip**; the remaining skips
  stay concentrated in SharedArrayBuffer, resizable ArrayBuffer, Proxy, broad
  Symbol iterator, and generator coverage.
- **For-of iterator protocol edges**: `for...of` now caches the iterator
  `next` method at `GetIterator` time, rejects non-object iterator results,
  applies `ToBoolean` to `done`, validates `return()` results during
  `IteratorClose`, preserves original throws when close also throws, and keeps
  hidden iterator-close state alive across labeled `continue`. The focused
  `language/statements/for-of` run now reports **113 pass / 0 fail / 638
  skip**, and the supported subset increases to **5099 pass / 0 fail / 0
  timeout**.
- **Object spread Symbol keys**: `{...source}` now copies enumerable own Symbol
  properties and follows `[[OwnPropertyKeys]]` order for integer index keys,
  string keys, then Symbol keys. It also re-checks each property descriptor at
  copy time and propagates Proxy `ownKeys` failures instead of falling back to
  the target. The focused
  `language/expressions/{array,call,new}/spread-obj-{spread-order,symbol-property,with-overrides}.js`
  cluster now runs at **9 pass / 0 fail**, and the supported subset increases
  to **5079 pass / 0 fail / 0 timeout**.
- **Symbol-key `SetFunctionName`**: object literal methods, anonymous
  function/arrow/class property values, and public class methods/accessors now
  infer function `name` properties from runtime Symbol property keys, using
  `[description]` formatting and `get ` / `set ` prefixes while preserving the
  cover-expression exception for `(0, function() {})`. The focused Symbol
  function-name cluster now runs at **10 pass / 0 fail / 5 skip**, and the
  supported subset increases to **5070 pass / 0 fail / 0 timeout**.
- **Disposal well-known Symbols**: `Symbol.dispose` and
  `Symbol.asyncDispose` are now exposed as shared well-known Symbols with
  spec-shaped static property descriptors and no global-registry keys.
  `tools/test262_runner.py` and `tools/test262_analyze.py` keep broader
  `explicit-resource-management` syntax coverage skipped while allowing the
  focused `built-ins/Symbol/{dispose,asyncDispose}` intrinsic tests to run at
  **6 pass / 0 fail / 0 skip**.
- **`%ThrowTypeError%` Realm identity**: restricted
  `Function.prototype.caller`/`arguments` accessors and strict-mode unmapped
  arguments objects now reuse the same canonical Realm `%ThrowTypeError%`
  intrinsic even when the arguments object is created by a function nested
  inside `new Function(...)`. `$262.createRealm()` now receives a Realm-local
  `Function.prototype` for dynamic functions, so cross-Realm restricted
  accessors compare against that Realm's thrower instead of the main Realm's.
  The focused
  `built-ins/Function/prototype/caller built-ins/Function/prototype/arguments`
  run improves from **0 pass / 2 fail / 0 skip** to **2 pass / 0 fail / 0
  skip**.
- **`Function.prototype[@@hasInstance]`**: `instanceof` now performs
  `GetMethod(C, @@hasInstance)` before falling back to `OrdinaryHasInstance`,
  and `Function.prototype[Symbol.hasInstance]` exposes the default hook with
  spec-shaped `name`, `length`, and property attributes. The
  `Symbol.hasInstance` test262 feature skip is removed; the supported subset
  remains green while increasing to **5060 pass / 0 fail / 0 timeout**.
- **Symbol prototype well-known properties**: `Symbol.prototype[@@toPrimitive]`
  and `Symbol.prototype[@@toStringTag]` now expose spec-shaped descriptors,
  Symbol primitives can resolve symbol-keyed prototype properties, and unary
  minus now uses `ToNumeric` so BigInt object wrappers negate as BigInt. The
  `Symbol.toPrimitive` and `Symbol.toStringTag` test262 feature skips are
  removed; the supported subset remains green while increasing to **5057 pass
  / 0 fail / 0 timeout**.
- **String exotic objects and coercion**: `String(object)` now performs
  observable `ToPrimitive` with string hint instead of bypassing overridden
  `toString` on arrays, while `OrdinaryToPrimitive` now skips non-callable
  `toString`/`valueOf` candidates. Boxed String numeric index properties now
  stay read-only/enumerable exotic own properties for assignment and
  `propertyIsEnumerable`, and `String.prototype.localeCompare` treats
  canonically equivalent Unicode strings as equal. The focused
  `built-ins/String` run improves from **1093 pass / 3 fail / 127 skip** to
  **1096 pass / 0 fail / 127 skip**.
- **Global `undefined` Reference semantics**: source `undefined` now parses as
  an IdentifierReference, so assignment uses `PutValue` against the
  non-writable global property and `delete undefined` uses the identifier
  delete path. Sloppy assignment remains ignored while returning the RHS,
  strict assignment throws `TypeError`, and delete returns `false`. The focused
  `built-ins/global built-ins/undefined` run improves from **33 pass / 4 fail
  / 0 skip** to **37 pass / 0 fail / 0 skip**.
- **Proxy `[[Construct]]` semantics**: constructable Proxy objects now follow
  their target's constructability, dispatch `construct` traps with a current
  Realm argument array, validate trap callability and object return values, and
  delegate through the target when no trap is present. The focused
  `built-ins/Proxy` run improves from **3 pass / 1 fail / 307 skip** to **4
  pass / 0 fail / 307 skip**.
- **Map/Set iterator prototype shape**: Map and Set iterators now inherit from
  shared `%MapIteratorPrototype%`/`%SetIteratorPrototype%` objects instead of
  carrying own `next` methods, expose spec-shaped `next` and
  `@@toStringTag` properties, and reject `next` calls on receivers without
  collection-iterator internal slots. The focused
  `built-ins/MapIteratorPrototype built-ins/SetIteratorPrototype` run improves
  from **9 pass / 5 fail / 8 skip** to **14 pass / 0 fail / 8 skip**.
- **DataView constructor ordering**: `DataView` now rejects function calls
  before coercing constructor arguments, and detached ArrayBuffers are checked
  only after the observable `byteOffset` coercion. The focused
  `built-ins/DataView` run improves from **266 pass / 2 fail / 293 skip** to
  **268 pass / 0 fail / 293 skip**.
- **DataView constructor length**: `DataView.length` now has the spec value
  `1` with the standard non-writable, non-enumerable, configurable descriptor.
  With DataView-related skips temporarily lifted, `built-ins/DataView/length.js`
  now passes, and the broader `built-ins/DataView` diagnostic reports
  **310 pass / 11 fail / 240 skip**.
- **DataView immutable-buffer setters**: implemented DataView setter
  validation for immutable ArrayBuffer backing stores. The implemented
  numeric and BigInt setters now throw `TypeError` before reading
  `byteOffset` or `value` arguments when the viewed buffer is immutable. With
  DataView-related skips temporarily lifted, `built-ins/DataView` improves to
  **320 pass / 1 fail / 240 skip**, leaving only the unsupported
  `setFloat16` immutable-buffer case in that diagnostic.
- **DataView Float16 accessors**: `DataView.prototype.getFloat16` and
  `setFloat16` now read and write IEEE-754 binary16 values with spec-shaped
  endian handling, ties-to-even rounding, signed zero, infinities, NaN, and the
  same validation ordering as the other DataView numeric methods. With
  DataView-related skips temporarily lifted, `built-ins/DataView` now closes at
  **321 pass / 0 fail / 240 skip**; additionally lifting `Float16Array` for the
  DataView diagnostic reports **352 pass / 0 fail / 209 skip**.
- **Date component getter receiver validation**: Date component getters now
  use a `thisTimeValue`-style receiver check, so ordinary objects, arrays,
  arguments objects, primitives, and objects spoofing RuJa's internal
  `__time__` property throw `TypeError` instead of reading as Invalid Date.
  `%Date.prototype%` is no longer Date-branded, while constructed Date and
  Date subclass instances still expose the Date brand. The focused Date
  component getter run improves from **80 pass / 16 fail / 32 skip** to
  **96 pass / 0 fail / 32 skip**; the broader `built-ins/Date` diagnostic now
  reports **309 pass / 173 fail / 112 skip**.
- **Date.UTC and TimeClip semantics**: `Date.UTC` now performs left-to-right
  numeric coercion for all supplied components, applies default month/date/time
  fields, normalizes 0-99 years, and returns the clipped MakeDate result.
  `TimeClip` now truncates fractional milliseconds and normalizes negative
  zero, so `Date` construction, `getTime`/`valueOf`, and `setTime` expose
  integer clipped time values. The focused
  `built-ins/Date/UTC built-ins/Date/prototype/{getTime,valueOf,setTime}` run
  improves from **20 pass / 16 fail / 6 skip** to **36 pass / 0 fail / 6
  skip**; the broader `built-ins/Date` diagnostic now reports **326 pass / 156
  fail / 112 skip**.
- **Date time-component setters**: `setMilliseconds`, `setSeconds`,
  `setMinutes`, `setHours`, and their UTC variants now read the receiver's
  DateValue before argument coercion, coerce optional arguments left to right,
  preserve omitted lower-order components, apply `TimeClip`, and expose
  spec-shaped `length` values. Invalid Date receivers still coerce supplied
  arguments but return `NaN` without overwriting side effects from coercion.
  The focused time-setter run improves from **28 pass / 68 fail / 12 skip** to
  **96 pass / 0 fail / 12 skip**; the broader `built-ins/Date` diagnostic now
  reports **394 pass / 88 fail / 112 skip**.
- **Date date-component setters**: `setDate`, `setMonth`, `setFullYear`, and
  their UTC variants now preserve the existing time within day, coerce optional
  arguments left to right, avoid the constructor-only 1900 offset for
  `setFullYear(0..99)`, and apply the distinct Invalid Date semantics for
  date/month setters versus full-year setters. The focused date-setter run
  improves from **23 pass / 41 fail / 9 skip** to **64 pass / 0 fail / 9
  skip**; the broader `built-ins/Date` diagnostic now reports **435 pass / 47
  fail / 112 skip**.
- **Date stringification, JSON, and ISO parsing**: Date prototype string
  methods now validate Date receivers, render UTC-backed date/time strings,
  return `Invalid Date` for invalid time values, and expose proper
  `toISOString` RangeError behavior. `Date.prototype.toJSON` now follows the
  generic `ToObject`/`ToPrimitive(number)`/`Invoke(toISOString)` path, while
  `Date.parse` recognizes the ISO and Date string forms emitted by RuJa.
  Single-argument Date construction now copies Date receivers without calling
  user hooks and parses Date strings. The focused string/parse/JSON run
  improves from **26 pass / 37 fail / 13 skip** to **63 pass / 0 fail / 13
  skip**; the broader `built-ins/Date` diagnostic now reports **476 pass / 6
  fail / 112 skip**, with the remaining failures isolated to Temporal
  `toTemporalInstant` coverage.
- **Date toTemporalInstant bridge**: `Date.prototype.toTemporalInstant` now
  validates Date-branded receivers, throws `RangeError` for invalid dates, and
  returns a branded `%Temporal.Instant%` with hidden epoch storage and
  `epochNanoseconds` exposed through the prototype accessor. The
  focused `built-ins/Date/prototype/toTemporalInstant` run improves from **0
  pass / 6 fail / 2 skip** to **6 pass / 0 fail / 2 skip**, closing the
  broader `built-ins/Date` diagnostic at **482 pass / 0 fail / 112 skip**.
- **BigInt TypedArray constructor surface**: BigInt typed array constructors
  and prototypes now expose non-writable, non-enumerable, non-configurable
  `BYTES_PER_ELEMENT` own properties, and typed array prototype accessors
  reject non-typed-array receivers. The focused
  `built-ins/TypedArrayConstructors` run improves from **10 pass / 6 fail /
  722 skip** to **16 pass / 0 fail / 722 skip**.
- **TypedArray integer-indexed `[[Set]]` ordering**: TypedArray numeric index
  assignments now run element value conversion before detached-buffer,
  out-of-bounds, invalid-index, or immutable-buffer validation. This preserves
  observable `ToNumber`/`ToBigInt` side effects and abrupt completions even
  when the write ultimately has no effect. With TypedArray-related skips
  temporarily lifted, `built-ins/TypedArrayConstructors/internals/Set`
  improves from **15 pass / 8 fail / 30 skip** to **21 pass / 2 fail / 30
  skip**; the remaining failures are detached-buffer realm constructor
  coverage.
- **TypedArray ArrayBuffer constructor ordering**: TypedArray constructors
  taking an ArrayBuffer now coerce `byteOffset` and explicit `length` before
  rechecking whether the backing buffer was detached, while still applying
  byte-offset alignment before length coercion. This prevents views from being
  created over buffers detached during argument conversion. With
  TypedArray-related skips temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/{ctors,ctors-bigint}/buffer-arg` run
  reports **44 pass / 0 fail / 62 skip**.
- **Cross-realm TypedArray constructors**: `$262.createRealm()` now exposes
  realm-local `ArrayBuffer`, `DataView`, `%TypedArray%`, and concrete
  TypedArray constructors instead of leaving `other[TA.name]` absent. Concrete
  constructors inherit from that realm's hidden `%TypedArray%`, their
  prototypes inherit from that realm's `%TypedArray%.prototype`, and detached
  buffers created through cross-realm constructors keep the expected
  integer-indexed behavior. With TypedArray-related skips temporarily lifted,
  the broader `built-ins/TypedArrayConstructors` diagnostic improves from
  **463 pass / 10 fail / 265 skip** to **473 pass / 0 fail / 265 skip**.
- **Constructor realm prototype fallback**: `GetPrototypeFromConstructor`
  fallback now uses the active `newTarget` function realm's intrinsic
  prototype instead of always falling back to the current VM realm. This lets
  `Reflect.construct()` with a cross-realm `newTarget` whose `.prototype` is
  non-object allocate TypedArrays, ArrayBuffers, DataViews, and RegExps with
  the other realm's intrinsic prototype. Focused `proto-from-ctor-realm`
  checks now pass at **13 pass / 0 fail / 0 skip** across
  `TypedArrayConstructors`, `ArrayBuffer`, `DataView`, and `RegExp`.
- **TypedArray integer-indexed `[[HasProperty]]`**: canonical numeric index
  property checks on TypedArrays now follow Integer-Indexed exotic semantics:
  valid in-bounds indexes report present, while detached, out-of-bounds,
  `"-0"`, fractional, negative, and infinite canonical numeric strings return
  `false` without falling through to ordinary prototype lookup. Non-canonical
  keys such as `"+1"` still use ordinary lookup. With TypedArray, ArrayBuffer,
  DataView, Reflect, and `Reflect.construct` skips temporarily lifted, focused
  `built-ins/TypedArrayConstructors/internals/HasProperty` improves from
  **14 pass / 10 fail / 8 skip** to **22 pass / 2 fail / 8 skip**; the
  remaining failures are the missing `%TypedArray%.prototype.subarray` method.
  Under the same expanded diagnostic, broader `built-ins/TypedArrayConstructors`
  improves to **584 pass / 9 fail / 145 skip**.
- **TypedArray `subarray()` inherited method**:
  `%TypedArray%.prototype.subarray` now creates offset views over the same
  ArrayBuffer, normalizes begin/end like slice bounds, rejects detached
  buffers, and routes allocation through `@@species` while preserving
  Number-vs-BigInt content type. Concrete typed-array prototypes inherit the
  method without gaining own `subarray` properties. Focused
  `built-ins/TypedArrayConstructors/prototype/subarray` now passes at
  **2 pass / 0 fail / 0 skip**. With TypedArray, ArrayBuffer, DataView,
  Reflect, and `Reflect.construct` skips temporarily lifted, focused
  `built-ins/TypedArrayConstructors/internals/HasProperty` improves from
  **22 pass / 2 fail / 8 skip** to **24 pass / 0 fail / 8 skip**, and broader
  `built-ins/TypedArrayConstructors` improves to **586 pass / 7 fail / 145
  skip**. The remaining broader failures are concentrated in
  Integer-Indexed `[[OwnPropertyKeys]]` ordering, `Reflect.set` receiver
  writes, and one typed-array-argument validation ordering case.
- **TypedArray integer-indexed `[[OwnPropertyKeys]]`**:
  `Reflect.ownKeys()` and shared own-key enumeration now synthesize attached
  TypedArray integer index keys before ordinary string and symbol keys,
  including offset `subarray()` views, while detached buffers expose no
  integer-indexed own keys. Focused
  `built-ins/TypedArrayConstructors/internals/OwnPropertyKeys` improves from
  **0 pass / 4 fail / 6 skip** to **4 pass / 0 fail / 6 skip** with
  TypedArray, ArrayBuffer, DataView, Reflect, and `Reflect.construct` skips
  temporarily lifted; with Symbol also lifted it reports **8 pass / 0 fail /
  2 skip**. Under the same expanded diagnostic, broader
  `built-ins/TypedArrayConstructors` improves to **590 pass / 3 fail / 145
  skip**. The remaining failures are now `Reflect.set` receiver writes and
  one typed-array-argument validation ordering case.
- **TypedArray receiver-aware integer-indexed `[[Set]]`**:
  `Reflect.set(target, index, value, receiver)` now routes valid
  integer-indexed writes through the receiver instead of always mutating the
  target. Plain-object receivers get ordinary data properties without
  coercing the value, while TypedArray receivers apply their own integer-index
  validation and element conversion; invalid receiver indexes fail before
  value coercion. Focused `built-ins/TypedArrayConstructors/internals/Set`
  improves from **41 pass / 2 fail / 10 skip** to **43 pass / 0 fail / 10
  skip** with TypedArray, ArrayBuffer, DataView, Reflect, and
  `Reflect.construct` skips temporarily lifted. Under the same expanded
  diagnostic, broader `built-ins/TypedArrayConstructors` improves to **592
  pass / 1 fail / 145 skip**; the remaining failure is the
  typed-array-argument validation ordering case.
- **TypedArray constructor `newTarget.prototype` ordering**:
  TypedArray construction now defers observable `newTarget.prototype` lookup
  until allocation, after argument validation and conversion have completed.
  `Reflect.construct(TA, [Symbol()], newTarget)` now reports the required
  `ToIndex` `TypeError` without touching a throwing custom prototype getter.
  Focused `built-ins/TypedArrayConstructors/ctors/typedarray-arg` improves
  from **12 pass / 1 fail / 1 skip** to **13 pass / 0 fail / 1 skip** with
  TypedArray, ArrayBuffer, DataView, Reflect, and `Reflect.construct` skips
  temporarily lifted. Under the same expanded diagnostic, broader
  `built-ins/TypedArrayConstructors` closes at **593 pass / 0 fail / 145
  skip**.
- **TypedArray Symbol-key `Reflect.set`**:
  `Reflect.set()` now routes Symbol property keys through the same
  receiver-aware ordinary `[[Set]]` path as string keys, so Symbol-named
  non-writable own data properties on TypedArrays return `false` instead of
  silently reporting success. With concrete TypedArray, Symbol, Proxy,
  ArrayBuffer, DataView, Reflect, and `Reflect.construct` gates admitted for
  the path, `built-ins/TypedArrayConstructors` now reports **673 pass / 0 fail
  / 65 skip** through the normal runner.
- **`Error.isError` static method**: `Error.isError(value)` is now exposed as
  a non-constructable unary builtin and recognizes real Error/NativeError
  objects, Error subclasses, and `$262.createRealm()` Error objects while
  rejecting primitives, constructors, ordinary objects, and objects that only
  spoof `Error.prototype`. `$262.createRealm()` also exposes `Array` and the
  native error constructor surface needed by cross-realm Error checks. The
  focused `built-ins/Error/isError` run improves from **0 pass / 10 fail / 2
  skip** to **10 pass / 0 fail / 2 skip**; the broader `built-ins/Error`
  diagnostic now reports **46 pass / 28 fail / 19 skip**.
- **`Error.prototype.toString` edge cases**:
  `Error.prototype.toString` now throws `TypeError` when called with a
  primitive receiver and omits the separating colon when either the resolved
  `name` or `message` string is empty. The focused
  `built-ins/Error/prototype/toString` run now closes at **15 pass / 0 fail /
  2 skip**, and the broader `built-ins/Error` diagnostic improves to **48 pass
  / 26 fail / 19 skip** with the remaining failures isolated to
  `Error.prototype.stack`.
- **`Error.prototype.stack` accessor**:
  `%Error.prototype%` now exposes a Realm-local `stack` accessor that accepts
  real Error objects, leaves newly constructed Error objects without an own
  `stack` data property, defines receiver-local stack data properties through
  the setter, and throws the receiver Realm's `TypeError` for forbidden
  prototype writes. Native error synthesis now preserves the throwing native
  callee's Realm and uses the Realm's original intrinsic Error prototypes
  instead of mutable global `TypeError`/`Error` bindings. `$262.createRealm()`
  now builds Realm-local Error and NativeError constructor/prototype chains for
  those cross-Realm checks. The
  focused `built-ins/Error/prototype/stack` run closes at **35 pass / 0 fail /
  0 skip**, and the broader `built-ins/Error` runner now reports **83 pass /
  0 fail / 10 skip**.
- **ArrayBuffer static surface**: `ArrayBuffer` now rejects calls without
  `new` before length coercion, exposes `ArrayBuffer.isView()` for typed-array
  and DataView receivers, provides the `ArrayBuffer[Symbol.species]` getter,
  and uses the intrinsic `%ArrayBuffer.prototype%` fallback for
  `Reflect.construct` new targets with non-object prototypes. The focused
  `built-ins/ArrayBuffer` run improves from **41 pass / 50 fail / 130 skip**
  to **52 pass / 39 fail / 130 skip**.
- **ArrayBuffer slice species construction**: `ArrayBuffer.prototype.slice`
  now uses `SpeciesConstructor`, accepts nullish `@@species` as the default
  `ArrayBuffer` constructor, calls custom species constructors with the slice
  length, rejects invalid species results, and preserves larger result buffer
  lengths while copying sliced bytes. The focused `built-ins/ArrayBuffer` run
  improves from **52 pass / 39 fail / 130 skip** to **57 pass / 34 fail / 130
  skip**.
- **ArrayBuffer transfer and immutable surface**: fixed-length ArrayBuffers now
  expose `transfer`, `transferToFixedLength`, `transferToImmutable`,
  `sliceToImmutable`, and the `immutable` accessor with descriptor-compatible
  names/lengths. Transfer operations copy, resize, zero-pad/truncate, detach
  the source, reject detached/immutable sources in spec order, and
  `ArrayBuffer.prototype.slice` now rejects detached sources and immutable
  species results. Related coercion fixes let `Array.from` read TypedArray
  array-like lengths, trim ES whitespace including `\uFEFF` in string numeric
  conversion, and coerce `Array.prototype.slice` bounds such as `null`. The
  focused `built-ins/ArrayBuffer` run improves from **57 pass / 34 fail / 130
  skip** to **90 pass / 1 fail / 130 skip**.
- **VM GC return-value rooting**: frame-boundary and top-level GC safe points
  now pin interpreted function return values and thrown values until the caller
  can observe them, and native calls pin their receiver while dispatching. This
  prevents a freshly returned `ArrayBuffer` from being swept during the long
  `sliceToImmutable` argument-coercion test and closes the focused
  `built-ins/ArrayBuffer` run at **91 pass / 0 fail / 130 skip**.
- **TypedArray intrinsic prototype shape**: concrete TypedArray constructors
  now report the spec `length` of `3`, inherit from a shared `%TypedArray%`
  intrinsic constructor, and their prototypes inherit `buffer`, `byteLength`,
  `byteOffset`, and `length` accessors from a shared `%TypedArray%.prototype`
  instead of defining them as own properties. With TypedArray skips temporarily
  lifted, the focused constructor/prototype-shape probe now reports
  **120 pass / 0 fail / 11 skip**.
- **TypedArray static `from`/`of`**: concrete TypedArray constructors now
  inherit `%TypedArray%.from` and `%TypedArray%.of`, construct the result before
  reading array-like elements, call mapper functions with the expected
  arguments and receiver, cache iterable `next` methods, and reject immutable
  ArrayBuffer-backed results before value conversion. With TypedArray skips
  temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/{from,from/BigInt,of,of/BigInt}` run
  closes at **126 pass / 0 fail / 0 skip**, and the broader
  `built-ins/TypedArrayConstructors` diagnostic now reports **473 pass / 54
  fail / 211 skip**.
- **TypedArray integer-indexed `[[Delete]]`**: deleting canonical numeric index
  strings on TypedArrays now follows Integer-Indexed exotic semantics: valid
  in-bounds indexes return `false`, while detached buffers, `"-0"`, fractional,
  negative, infinite, and out-of-bounds canonical numeric keys return `true`.
  Non-canonical keys continue through ordinary delete. With TypedArray skips
  temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/internals/Delete` diagnostic improves to
  **29 pass / 2 fail / 8 skip**, and the broader
  `built-ins/TypedArrayConstructors` diagnostic moves from **419 pass / 54 fail
  / 265 skip** to **431 pass / 42 fail / 265 skip**.
- **TypedArray integer-indexed `[[GetOwnProperty]]`**: valid canonical numeric
  index strings on TypedArrays now synthesize spec-shaped data descriptors
  with the element value and writable/enumerable/configurable all `true`.
  Detached buffers and invalid canonical numeric keys such as `"-0"`,
  fractional, negative, infinite, and out-of-bounds indexes stop ordinary
  fallback and report no descriptor, while non-canonical keys continue through
  ordinary properties. The same descriptor path feeds Proxy `has` and
  `deleteProperty` invariants for non-extensible TypedArray targets. With
  TypedArray skips temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/internals/GetOwnProperty` diagnostic now
  reports **18 pass / 2 fail / 4 skip**; the remaining failures are
  cross-realm detached-buffer constructor coverage. The broader
  `built-ins/TypedArrayConstructors` diagnostic reports **429 pass / 44 fail /
  265 skip**.
- **TypedArray integer-indexed `[[Get]]`**: canonical numeric index property
  reads now use Integer-Indexed exotic element access instead of Rust integer
  parsing or ordinary prototype lookup. Valid indexes read numeric and BigInt
  elements from owned or ArrayBuffer-backed storage, detached buffers and
  invalid canonical numeric keys such as `"-0"`, fractional, negative,
  infinite, and out-of-bounds indexes return `undefined` without touching
  inherited accessors, and non-canonical numeric-looking keys such as `"+1"`
  continue through ordinary own/prototype lookup. With TypedArray skips
  temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/internals/Get` diagnostic now reports
  **20 pass / 2 fail / 6 skip**; the remaining failures are cross-realm
  detached-buffer constructor coverage. The broader
  `built-ins/TypedArrayConstructors` diagnostic improves to **437 pass / 36
  fail / 265 skip**.
- **TypedArray integer-indexed `[[DefineOwnProperty]]`**: defining canonical
  numeric index properties now follows Integer-Indexed exotic validation.
  Invalid or detached indexes reject, accessor descriptors and descriptors
  requesting non-configurable, non-enumerable, or non-writable attributes
  reject, valid value descriptors write through element conversion for numeric
  and BigInt arrays, and non-canonical numeric-looking keys remain ordinary
  properties. With TypedArray skips temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/internals/DefineOwnProperty` diagnostic now
  reports **16 pass / 2 fail / 36 skip**; the remaining failures are
  cross-realm detached-buffer constructor coverage. The broader
  `built-ins/TypedArrayConstructors` diagnostic improves to **453 pass / 20
  fail / 265 skip**.
- **Nullish computed property write/delete ordering**: simple computed
  property assignment and `delete` now reject `null`/`undefined` bases before
  observable `ToPropertyKey` coercion. Assignment still evaluates the RHS
  before the `PutValue` `TypeError`, while delete evaluates only the computed
  key expression before the nullish-base failure. The focused
  `language/expressions/{assignment,delete,member-expression}` run remains
  **273 pass / 0 fail / 282 skip**.
- **Array destructuring assignment IteratorClose ordering**: array assignment
  patterns now close unfinished iterators on normal partial completion,
  evaluate rest assignment target references before draining rest values, and
  close iterators when rest-target or rest-iterator evaluation completes
  abruptly. The focused `language/expressions/assignment/dstr` run closes at
  **90 pass / 0 fail / 278 skip**, and the broader Reference-adjacent cluster
  `language/expressions/assignment language/expressions/compound-assignment
  language/expressions/logical-assignment language/expressions/update
  language/statements/with` remains **835 pass / 0 fail / 363 skip**.
- **`%ThrowTypeError%` intrinsic**: restricted function and arguments
  accessors now use an anonymous, frozen, non-extensible `%ThrowTypeError%`
  function per Realm. Strict and non-simple-parameter unmapped arguments reuse
  the same Realm-local thrower for `callee`, while `$262.createRealm()` gets a
  distinct intrinsic. The focused `built-ins/ThrowTypeError` run improves from
  **8 pass / 6 fail / 0 skip** to **14 pass / 0 fail / 0 skip**.
- **Private slot brand checks**: class private field, accessor, and method
  access now throws `TypeError` for primitive receivers and objects missing the
  private slot instead of returning `undefined` or creating a new slot.
  Class-element initialization now uses separate `InitPrivate` opcodes, private
  method slots are not writable. With private class feature skips temporarily
  lifted, the focused `language/{statements,expressions}/class/elements` probe
  improves from **1045 pass / 587 fail / 1330 skip** to **1085 pass / 547 fail
  / 1330 skip**. The remaining same-spelling cross-class failures require
  per-evaluation private-name identity instead of textual `#name` keys.
- **Per-evaluation private-name identity**: class evaluation now allocates a
  fresh opaque private-name key for each private field/method/accessor name and
  stores it in the class lexical environment captured by constructors and
  methods. Private slots use those opaque keys, while RegExp and Proxy internal
  slots use a separate internal-key namespace. Same-spelling private names from
  different class evaluations or from superclass/subclass bodies no longer
  share brands. With private class feature skips temporarily lifted, the
  focused `language/{statements,expressions}/class/elements` probe improves from
  **1085 pass / 547 fail / 1330 skip** to **1096 pass / 536 fail / 1330 skip**.
- **Private names before division**: the lexer now treats private names as
  value-ending tokens for slash disambiguation, so `this.#x / y` and
  `this.#x /= y` parse as division and divide-assignment instead of starting a
  RegExp literal. With private class feature skips temporarily lifted, the
  focused `language/expressions/compound-assignment` diagnostic now reports
  **454 pass / 0 fail / 0 skip**.
- **`String.prototype.matchAll` and RegExp `@@matchAll`**:
  `String.prototype.matchAll` is now exposed with spec-shaped builtin
  properties, validates non-global RegExp arguments before delegation, calls
  custom `@@matchAll` methods with the original receiver value, and falls back
  through a forced-global intrinsic RegExp. `RegExp.prototype[Symbol.matchAll]`
  now creates a lazy RegExp String Iterator through `RegExpExec`, preserving
  species construction, cached `lastIndex`, custom `exec`, match result
  arrays, and empty-match advancement. The focused
  `built-ins/String/prototype/matchAll
  built-ins/RegExp/prototype/Symbol.matchAll` run moves from **5 pass / 43 fail
  / 3 skip** to **48 pass / 0 fail / 3 skip**.
- **`String.prototype.normalize` Unicode forms**: `String.prototype.normalize`
  is now exposed as a non-constructor prototype builtin with spec-shaped
  `name`, `length`, and descriptor properties. It defaults to NFC, coerces the
  `form` argument observably, rejects invalid forms with `RangeError`, and
  returns NFC/NFD/NFKC/NFKD output through Unicode normalization. The focused
  `built-ins/String/prototype/normalize` run now closes at **11 pass / 0 fail /
  3 skip**.
- **RegExp `@@match` prototype builtin**:
  `RegExp.prototype[Symbol.match]` is now installed with the expected builtin
  shape and routes through `RegExpExec`, so direct `r[Symbol.match](value)`
  calls observe public `flags`, custom `exec`, `lastIndex` writes, global
  empty-match advancement, and abrupt completions. `String.prototype.match`
  now falls back through an intrinsic RegExp clone, preserving RegExp source
  and flags when an own `@@match` is `undefined`. The focused
  `built-ins/RegExp/prototype/Symbol.match` run moves from **8 pass / 44 fail /
  1 skip** to **52 pass / 0 fail / 1 skip** after non-Unicode surrogate escapes
  are lowered for the Rust regex backend.
- **URI encode/decode globals**: `encodeURI`, `encodeURIComponent`,
  `decodeURI`, and `decodeURIComponent` now implement ECMAScript percent
  encoding/decoding over UTF-16 code units, preserve `decodeURI` reserved
  escapes, reject malformed UTF-8 and lone surrogates with `URIError`, and keep
  `String.fromCharCode` pairs in RuJa's surrogate-sentinel range distinguishable
  from lone surrogates. The focused
  `built-ins/{decodeURI,decodeURIComponent,encodeURI,encodeURIComponent}` run
  improves from **74 pass / 93 fail / 2 timeout / 4 skip** to **167 pass / 0
  fail / 2 timeout / 4 skip**.
- **Array `some`/`every` generic iteration**: `Array.prototype.some` and
  `Array.prototype.every` now follow `LengthOfArrayLike`/`HasProperty` before
  `Get`, so array-like receivers, boxed primitives, inherited sparse indexes,
  callback `thisArg`, length snapshots, and abrupt completions are observed.
  The focused `built-ins/Array/prototype/{some,every}` run improves from
  **225 pass / 202 fail / 10 skip** to **427 pass / 0 fail / 10 skip**.
- **RegExp boolean flag accessors**: `global`, `ignoreCase`, `multiline`,
  `dotAll`, `sticky`, `unicode`, `unicodeSets`, and `hasIndices` now enforce
  RegExp internal-slot receiver validation. Real RegExp objects still expose
  their stored flag bits, the current realm `%RegExp.prototype%` returns
  `undefined`, and ordinary or cross-realm prototype receivers throw
  `TypeError`. The focused
  `built-ins/RegExp/prototype/{flags,global,ignoreCase,multiline,dotAll,sticky,unicode,unicodeSets,hasIndices}`
  run now closes at **62 pass / 0 fail / 54 skip**.
- **`String.prototype.replaceAll` and RegExp `@@replace`**:
  `String.prototype.replaceAll` now follows the spec's observable ordering for
  `IsRegExp`, global-flag validation, `@@replace` delegation, receiver/search
  coercion, callable replacers, empty search strings, and `$` substitution
  tokens. `RegExp.prototype[Symbol.replace]` is now installed for global,
  sticky, capture, named-capture, and functional replacement paths, while
  `RegExp.prototype.toString` observes the public `source`/`flags` getters.
  The same slice fixes `super[Symbol.*]` method lookup/calls and nested array
  binding temporaries uncovered by the focused test262 file. The focused
  `built-ins/String/prototype/replaceAll` run now closes at **35 pass / 0 fail /
  10 skip**.
- **`RegExp.escape` static builtin**: `RegExp.escape` is now installed on each
  realm-local `RegExp` constructor with the expected own property shape,
  rejects non-string inputs without coercion, and implements the ES
  `EncodeForRegExpEscape` rules for initial ASCII alphanumerics, syntax
  characters, `/`, control escapes, whitespace/line terminators, other
  punctuators, and lone UTF-16 surrogates. The focused
  `built-ins/RegExp/escape` run now closes at **19 pass / 0 fail / 1 skip**.
- **Object integrity for arrays, arguments, functions, and Proxy traps**:
  `Object.seal`/`Object.freeze` now route through the Proxy-aware
  `[[PreventExtensions]]` path so false Proxy traps throw for the `Object.*`
  forms, materialize dense Array and arguments indexes so sealed/frozen
  descriptors are observable, and freeze Array `length` by honoring its
  non-writable descriptor during length assignment. `Object.isSealed`/
  `Object.isFrozen` now require non-extensible ordinary objects/functions and
  report Array/arguments integrity from their materialized descriptors while
  preserving primitive receivers as already sealed/frozen. The focused
  `built-ins/Object/{seal,freeze,isSealed,isFrozen}` run now closes at **218
  pass / 0 fail / 21 skip**.
- **TypedArray constructor surface**: the existing byte-backed TypedArray
  exotic now exposes the full constructor family (`Int8Array`,
  `Uint8ClampedArray`, `Int16Array`, `Uint16Array`, `Int32Array`,
  `Uint32Array`, `Float32Array`, `Float64Array`, `BigInt64Array`, and
  `BigUint64Array`) alongside `Uint8Array`, with element-size-aware
  `length`/`byteLength`, indexed reads/writes, BigInt element conversion, and
  `[[Extensible]]` tracking for `Object.seal`/`Object.preventExtensions`.
  This removes the final TypedArray-constructor failures from the Object
  integrity focused run.
- **TypedArray constructor inputs**: typed-array constructors now reject
  function-call usage without `new`, use the active typed-array prototype as
  the fallback for `GetPrototypeFromConstructor`, coerce primitive lengths with
  `ToIndex`-style `NaN`/`undefined` handling, read ordinary array-like
  `length`/indexed properties observably, and consume iterable arguments via
  `IteratorToList` before element conversion. `Array.prototype[Symbol.iterator]`
  is now exposed as the `values` method so array-backed iterable constructor
  inputs use the normal iterator protocol. With TypedArray-related skips
  temporarily lifted, the focused
  `built-ins/TypedArrayConstructors/ctors/{no-args,length-arg,object-arg}`
  diagnostic moved from **8 pass / 20 fail / 19 skip** to **25 pass / 3 fail /
  19 skip**.
- **ArrayBuffer-backed TypedArray views**: typed-array instances now carry
  `[[ViewedArrayBuffer]]`, `[[ByteOffset]]`, and `[[ByteLength]]` slots.
  Constructors accept `ArrayBuffer` inputs with range/alignment checks, expose
  the original buffer through `.buffer`, report view-relative `length`,
  `byteLength`, and `byteOffset`, and route indexed reads/writes through the
  shared backing buffer. With TypedArray-related skips temporarily lifted, the
  focused
  `built-ins/TypedArrayConstructors/ctors/{no-args,length-arg,object-arg}`
  diagnostic now reports **26 pass / 2 fail / 19 skip**; the remaining
  failures are isolated to iterable zero-fill coverage and shared
  ArrayIteratorPrototype mutation semantics.
- **TypedArray prototype numeric `[[Set]]`**: assignment now recognizes full
  `CanonicalNumericIndexString` keys, including `"NaN"` and `"-0"`, when a
  TypedArray appears on an ordinary object's prototype chain. Invalid numeric
  indexes are treated as successful no-ops instead of creating receiver
  properties, while valid inherited numeric indexes still create receiver data
  properties. With TypedArray-related skips temporarily lifted, the focused
  `language/statements/with` diagnostic now reports **171 pass / 0 fail / 10
  skip**.
- **TypedArray backing buffers and ArrayIteratorPrototype `next`**:
  typed-array views now trace their `[[ViewedArrayBuffer]]` during GC, keeping
  zero-filled length allocations alive across harness pressure. Array iterator
  objects now inherit `next` and `@@iterator` from a shared prototype instead
  of masking prototype writes with an own `next`, so typed-array construction
  observes `Object.getPrototypeOf([].values()).next` overrides through the
  iterator protocol. With TypedArray-related skips temporarily lifted, the
  focused `built-ins/TypedArrayConstructors/ctors/{no-args,length-arg,object-arg}`
  diagnostic now reports **28 pass / 0 fail / 19 skip**, closing that
  constructor probe.
- **Proxy SetIntegrityLevel/TestIntegrityLevel**: transparent Proxy receivers
  for `Object.seal`/`Object.freeze` now tighten the target's own descriptors
  through the Proxy-aware `[[DefineOwnProperty]]` path, and
  `Object.isSealed`/`Object.isFrozen` now use Proxy `ownKeys` and
  `getOwnPropertyDescriptor` semantics instead of treating Proxy objects as
  ordinary empty exotics. With `Proxy`/`Reflect`/`Symbol` skips temporarily
  lifted, the focused Object integrity proxy diagnostic runs at **6 pass / 0
  fail**.
- **Proxy prototype internal methods**: `Object.getPrototypeOf`,
  `Reflect.getPrototypeOf`, `Object.setPrototypeOf`, `Reflect.setPrototypeOf`,
  the `__proto__` accessor, and `instanceof` now route through Proxy
  `getPrototypeOf`/`setPrototypeOf` traps, including nullish trap delegation,
  revoked-proxy errors, and non-extensible target invariants. With
  `Proxy`/`Reflect` skips temporarily lifted, the focused
  `built-ins/Proxy/{getPrototypeOf,setPrototypeOf}
  built-ins/Reflect/{getPrototypeOf,setPrototypeOf}` diagnostic runs at
  **29 pass / 0 fail / 31 skip**, and the broader Proxy descriptor/prototype
  diagnostic improves from **21 pass / 46 fail / 63 skip** to **46 pass / 21
  fail / 63 skip**. The remaining failures are isolated to Proxy descriptor
  conversion and define/getOwnPropertyDescriptor invariants.
- **Proxy.revocable revoke function shape**: `Proxy.revocable()` now creates
  its revoke closure through the native function helper so the closure exposes
  spec-shaped own `length` and `name` properties in the expected order, while
  keeping the associated proxy in a non-observable private slot rather than an
  ordinary own property. With `Proxy`/`Reflect`/`Symbol` skips temporarily
  lifted, `built-ins/Proxy/revocable` now runs at **17 pass / 0 fail / 1
  skip** after callable Proxy support for function targets.
- **Callable Proxy `[[Call]]` support**: Proxy objects whose target is
  callable are now treated as callable by `typeof`, `IsCallable` checks, and
  ordinary function calls. Proxy calls throw on revoked proxies, forward to the
  target when `handler.apply` is nullish, and invoke callable `apply` traps
  with `(target, thisArgument, argumentsList)`. With Proxy-related skips
  temporarily lifted, `built-ins/Proxy/apply` now runs at **13 pass / 0 fail /
  1 skip**.
- **Object descriptor helpers observe Proxy descriptors**: `Object.values`,
  `Object.entries`, and `Object.getOwnPropertyDescriptors` now use the
  Proxy-aware `[[GetOwnProperty]]` path for each snapshotted key, so
  `getOwnPropertyDescriptor` traps run before enumerable filtering and
  descriptor materialization instead of being bypassed through target storage.
  With `Proxy`/`Reflect`/`Symbol` skips temporarily lifted, focused
  `built-ins/Object/{values,entries,getOwnPropertyDescriptors}` now runs at
  **59 pass / 0 fail / 0 skip** after the separate RegExp internal-slot
  exposure fix.
- **RegExp internal slots hidden from own keys**: RegExp instances now keep
  source, flags, and derived flag bits in non-observable internal storage
  rather than ordinary own properties, leaving `lastIndex` as the only default
  own string key. The public `RegExp.prototype.flags` getter still observes
  `global`/`sticky`/other property overrides through normal `Get` semantics,
  while the individual flag getters read the internal slots. This fixes
  `Object.getOwnPropertyNames`, `Reflect.ownKeys`, and
  `Object.getOwnPropertyDescriptors` order for RegExp instances without
  colliding with subclass `#private` fields.
- **Reference-preserving identifier calls through `with`**: direct
  IdentifierReference calls now retain their Reference record through the VM
  call opcode, so `with (o) { f() }` still binds `this` to `o`, while value
  expressions such as `(0, f)()`, `(cond ? f : f)()`, and `(f && f)()` lose
  the object-environment `this` binding as required. Spread and optional
  identifier calls share the same Reference-preserving path, and direct
  `eval(...)` keeps its existing intrinsic-eval behavior. The focused
  `language/statements/with` test262 run closes at **169 pass / 0 fail / 12
  skip**.
- **Optional method-call argument order**: optional member calls now resolve the
  method and short-circuit nullish method values before evaluating arguments,
  so `o.m?.(sideEffect())` and spread arguments skip side effects when `o.m` is
  `null`/`undefined` while preserving `this` for present methods. The focused
  `language/expressions/optional-chaining` test262 directory remains feature
  skipped by the runner (**38 skip**), so this edge is covered by local
  `operators` regressions.
- **Symbol computed keys for member assignments**: computed member update,
  numeric/bitwise compound assignment, and logical assignment now coerce keys
  with `ToPropertyKey` instead of `ToString`, preserving Symbol property keys
  while still evaluating the base, key, and right-hand side in spec order. The
  focused `language/expressions/{compound-assignment,logical-assignment,
  prefix-increment,postfix-increment}` test262 run closes at **532 pass / 0
  fail / 71 skip**.
- **Map/Set zero-key canonicalization**: keyed collections now normalize
  numeric `-0` to `+0` when creating internal `MapKey`s, and the `MapKey`
  hash implementation now matches SameValueZero equality for both zero signs.
  This preserves O(1) lookups while making Map replacement, Set de-duplication,
  and key iteration agree with `CanonicalizeKeyedCollectionKey`. The focused
  zero-key test262 probe now runs at **2 pass / 0 fail**.
- **Map prototype receiver brand checks**: `Map.prototype` methods now reject
  receivers without a `[[MapData]]` internal slot with `TypeError` instead of
  silently returning `undefined`, `false`, empty arrays, or the original
  receiver. The focused `built-ins/Map/prototype/{get,set,has,delete,clear,
  entries,keys,values,forEach,size}` cluster now runs at **60 pass / 11 fail /
  47 skip**, with remaining failures isolated to true MapIterator and live
  iteration semantics.
- **Set prototype size accessor and receiver brand checks**:
  `Set.prototype.size` is now a spec-shaped `"get size"` accessor instead of
  a data method, Set instance `size` reads now use ordinary prototype lookup,
  and Set prototype methods now reject receivers without `[[SetData]]` with
  `TypeError`. `Set.prototype.clear` is exposed with the same receiver
  validation. The focused `built-ins/Set/prototype/{size,add,has,delete,clear,
  entries,keys,values,forEach}` cluster now runs at **130 pass / 9 fail / 26
  skip**, with remaining failures isolated to true SetIterator and live
  iteration semantics.
- **Map/Set collection iterators and live forEach**: `Map` and `Set`
  `entries`/`keys`/`values` now return iterator objects with `next()` result
  objects instead of snapshot arrays, built-in iteration uses the same lazy
  collection iterator path, `Map.prototype[Symbol.iterator]` and
  `Set.prototype[Symbol.iterator]` reuse the spec method objects, and
  `Set.prototype.keys === Set.prototype.values`. Map/Set `forEach` now observes
  values added during iteration, skips deleted unvisited values, and revisits
  delete-then-readded values in insertion order. The focused
  `built-ins/{Map,Set}/prototype` iterator/forEach/Symbol.iterator cluster now
  runs at **104 pass / 0 fail / 38 skip**.
- **Set composition methods and constructor iterable compliance**:
  `Set.prototype.union`, `intersection`, `difference`,
  `symmetricDifference`, `isSubsetOf`, `isSupersetOf`, and `isDisjointFrom`
  are now exposed with Set-like operand handling, direct Set result
  allocation, live receiver traversal where required, iterator closing for
  early exits, Set-like size validation, and SameValueZero key semantics.
  `Array.prototype.values()` now returns an iterator object instead of an array
  snapshot, which lets Set-like `keys()` methods return array iterators without
  relaxing `GetIteratorFromMethod` validation. `new Set(iterable)` now checks
  `new.target`, observes the instance `add` method once before iteration, calls
  it for each iterated value, and closes the iterator when `add` throws. The
  focused Set composition cluster runs at **179 pass / 0 fail / 7 skip**,
  `built-ins/Set` now closes at **340 pass / 0 fail / 43 skip**, and
  `built-ins/Map built-ins/Set` improves to **432 pass / 17 fail / 138 skip**.
- **Map constructor iterable compliance and upsert methods**: `new
  Map(iterable)` now requires construction with `new`, observes the instance
  `set` method once before iterator creation, calls it for each entry pair,
  accepts array-like pair objects through ordinary property access, and closes
  the source iterator when pair access or `set` fails while preserving the
  original abrupt completion if iterator closing also throws. `Map.prototype`
  additionally exposes `getOrInsert` and `getOrInsertComputed` with
  SameValueZero key canonicalization and computed-callback overwrite
  semantics. The focused `built-ins/Map built-ins/Set` run now closes at
  **449 pass / 0 fail / 138 skip**.
- **Earlier shallow `Map.groupBy` static-grouping unit**: `Map.groupBy` was
  exposed as a static
  built-in, iterates arbitrary sync iterables, calls the grouping callback with
  `(value, index)`, stores group keys with SameValueZero Map-key semantics
  instead of `ToPropertyKey`, returns a real Map instance, and closes custom
  iterators when the callback abruptly completes. Forced execution of the
  focused `built-ins/Map/groupBy` directory was **14 pass / 0 fail**; ordinary
  policy at the current pin remains **12 pass / 0 fail / 2 skip**. The direct
  iterator/Realm/resource audit in the current Unreleased section supersedes
  this shallow evidence and admits the two exact skipped files.
- **Map/Set feature lift**: `Map` and `Set` are removed from the test262
  unsupported-feature skip list after the expanded `built-ins/Map
  built-ins/Set` diagnostic verifies at **473 pass / 0 fail / 114 skip**.
  The supported subset remains green while increasing to **5017 pass / 0 fail
  / 0 timeout**.
- **String well-formed Unicode methods**: `String.prototype.isWellFormed`
  and `String.prototype.toWellFormed` now follow UTF-16 surrogate-pair
  semantics, reject nullish receivers, preserve valid internal surrogate-pair
  representations, and replace unpaired surrogates with U+FFFD for
  `toWellFormed`. The focused
  `built-ins/String/prototype/isWellFormed
  built-ins/String/prototype/toWellFormed` run now closes at **14 pass / 0
  fail / 2 skip**.
- **`Array.of` constructor and property semantics**: `Array.of` now uses a
  constructable `this` value with the argument count, creates element data
  properties without invoking prototype setters, routes final `length` through
  strict `Set`, and propagates constructor/property abrupt completions.
  Test262 realms now expose a constructable `Function` constructor so
  cross-realm constructor fallback cases can run. The focused
  `built-ins/Array/of` run now closes at **14 pass / 0 fail / 2 skip**.
- **Reflect.construct `newTarget` semantics**: `Reflect.construct` now
  validates constructor-ness in spec order, builds its argument list through
  ordinary array-like property access, forwards the optional `newTarget` into
  allocation, and uses `newTarget.prototype` with `%Object.prototype%`
  fallback when it is not an object. Bound constructors preserve the caller's
  `newTarget` instead of resetting it to the bound target. With
  `Reflect`/`Reflect.construct` skips temporarily lifted, the focused
  `built-ins/Reflect/construct` diagnostic now runs at **10 pass / 0 fail**.
- **Reflect.apply array-like arguments**: `Reflect.apply` now validates
  callability before observing `argumentsList`, then builds the call argument
  list through ordinary array-like `length` and indexed property access instead
  of cloning only dense Array storage. This makes primitive or missing
  `argumentsList` values throw `TypeError`, propagates abrupt `length`/index
  gets, and accepts ordinary array-like objects and functions. With
  `Reflect`/`Symbol` skips temporarily lifted, the focused
  `built-ins/Reflect/apply` diagnostic now runs at **8 pass / 0 fail / 1
  skip**.
- **Number static method descriptors**: `Number.isFinite`,
  `Number.isInteger`, `Number.isNaN`, `Number.isSafeInteger`,
  `Number.parseInt`, and `Number.parseFloat` are now installed as writable,
  non-enumerable, configurable constructor properties, while numeric constants
  remain non-writable and non-configurable. The focused
  `built-ins/Number/{isFinite,isInteger,isNaN,isSafeInteger}` run now closes
  at **26 pass / 0 fail / 8 skip**.
- **Boolean prototype receiver checks**: `Boolean.prototype` now carries the
  wrapped `false` primitive value, `Boolean.prototype.valueOf` returns Boolean
  primitives from primitive/boxed Boolean receivers, and `valueOf`/`toString`
  reject non-Boolean receivers with `TypeError`. The focused
  `built-ins/Boolean` run now closes at **46 pass / 0 fail / 5 skip**.
- **Number/String prototype receiver checks**: `Number.prototype` and
  `String.prototype` now carry their required wrapped primitive values,
  `Number.prototype.valueOf` and `String.prototype.valueOf` reject receivers
  without matching wrapper data, and `$262.createRealm()` exposes realm-local
  primitive wrapper constructors for cross-realm TypeError checks. The focused
  `built-ins/Number/prototype/valueOf built-ins/String/prototype/valueOf` run
  now closes at **16 pass / 0 fail / 2 skip**.
- **Number prototype toString radix order**: `Number.prototype.toString` now
  validates its Number receiver before radix coercion, treats an omitted or
  explicit `undefined` radix as decimal, and propagates abrupt completions from
  radix `ToNumber` instead of silently falling back to base 10. The focused
  `built-ins/Number/prototype/toString built-ins/String/prototype/toString`
  run now closes at **95 pass / 0 fail / 2 skip**.
- **Number prototype toFixed integer conversion**: `Number.prototype.toFixed`
  now uses `ThisNumberValue`, applies `ToIntegerOrInfinity` semantics to
  `fractionDigits`, validates the range before the NaN return path, delegates
  `|x| >= 1e21` to ordinary Number stringification, and preserves the spec's
  exact fixed-point output and tie-up rounding. Number stringification now uses
  the shortest decimal for integer-valued doubles, so `toString` and
  `toFixed(0)` differ where the spec requires. The focused
  `built-ins/Number/prototype/toFixed` run now closes at **14 pass / 0 fail /
  2 skip**.
- **Number exponential/precision formatting**: `Number.prototype.toExponential`
  and `Number.prototype.toPrecision` now use `ThisNumberValue`, truncate their
  digit arguments with `ToIntegerOrInfinity`, apply the special-value return
  path after argument coercion but before range checks, normalize exponent signs,
  format `-0` as `+0`, and use exact-rational half-up decimal rounding for
  exponential notation. The broader `built-ins/Number` run now closes at **312
  pass / 0 fail / 28 skip**.
- **Math.pow NaN and infinite exponent edges**: `Math.pow` now handles
  exponent `NaN` and `abs(base) === 1` with infinite exponents before
  delegating to Rust's `powf`, while preserving the required `x ** ±0 === 1`
  behavior. The focused `built-ins/Math/pow` run now closes at **27 pass / 0
  fail / 1 skip**.
- **Math.sumPrecise**: `Math.sumPrecise` is now exposed as a unary Math
  builtin, consumes iterable Number values without coercing non-numbers, closes
  the iterator on non-number failures, preserves `NaN`, infinity, and signed
  zero semantics, and accumulates finite values through exact-rational
  summation before final IEEE-754 rounding. The focused
  `built-ins/Math/sumPrecise` run now closes at **8 pass / 0 fail / 2 skip**,
  and broader `built-ins/Math` closes at **284 pass / 0 fail / 43 skip**.
- **Number parse function identity**: `Number.parseInt` and
  `Number.parseFloat` now reference the same built-in function objects as the
  global `parseInt` and `parseFloat` properties instead of separate native
  wrappers. The broader `built-ins/Number` run now improves to **301 pass /
  11 fail / 28 skip**.
- **PrivateName lexical grammar**: private class names now use the same
  `IdentifierName` Unicode escape and raw Unicode scanning rules as ordinary
  identifiers, including `Other_ID_Start`, ZWNJ, and ZWJ handling. This fixes
  private fields, methods, and accessors whose names are spelled with
  `\uXXXX`/`\u{...}` escapes or non-ASCII source text. The focused
  private-name diagnostic now closes at **50 pass / 0 fail**.
- **Private method function identity**: instance private methods are now
  created once during class evaluation and copied into each instance private
  slot from a shared class-environment binding, instead of allocating a fresh
  function object in every constructor call. This preserves private method
  names, `super` HomeObject capture, and `this.#m` identity across instances.
  With private method skips temporarily lifted, the focused
  `language/{statements,expressions}/class/elements/private-methods`
  diagnostic now closes at **2 pass / 0 fail / 8 skip**.
- **Object/Reflect preventExtensions semantics**: Array and arguments objects
  now store their own `[[Extensible]]` state, assignment/receiver-set paths
  reject new indexed or named properties on non-extensible arrays, arguments,
  and functions, and `Object.preventExtensions`/`Reflect.preventExtensions`
  now route through Proxy `preventExtensions` traps with the correct
  throw-vs-boolean behavior. The focused `built-ins/Object/preventExtensions`
  run now closes at **36 pass / 0 fail / 4 skip**, and the adjacent
  `built-ins/{Reflect,Proxy}/preventExtensions` probe closes at **19 pass / 0
  fail / 3 skip** with skips temporarily lifted.
- **Object/Reflect isExtensible Proxy semantics**: `Object.isExtensible` now
  routes object receivers through the Proxy-aware `[[IsExtensible]]` helper,
  returns `false` for primitive receivers, and enforces Proxy trap result
  invariants against the target's actual extensibility. `Reflect.isExtensible`
  now rejects primitive targets with `TypeError` and shares the same Proxy
  trap path. With `Proxy`/`Reflect` skips temporarily lifted, the focused
  `built-ins/{Object,Reflect,Proxy}/isExtensible` probe now closes at **55
  pass / 0 fail / 3 skip**.
- **parseInt radix and large-prefix conformance**: global `parseInt` now
  applies `ToNumber`/`ToInt32` to its radix argument, so string, boxed, object,
  infinite, and modulo-2^32 radix values follow the spec. Digit accumulation no
  longer overflows through Rust integer parsing, so large valid prefixes return
  their nearest IEEE-754 Number value instead of `NaN`. The focused
  `built-ins/parseInt` run now closes at **53 pass / 0 fail / 2 skip**.
- **Math inverse hyperbolic methods**: `Math.acosh`, `Math.asinh`, and
  `Math.atanh` are now exposed as unary native functions with spec-shaped
  `name`, `length`, and own-property descriptors. They reuse the normal
  `ToNumber` unary Math path and preserve NaN, infinity, and signed-zero
  behavior through the host libm operations. The focused
  `built-ins/Math/acosh built-ins/Math/asinh built-ins/Math/atanh` run now
  closes at **14 pass / 0 fail / 3 skip**.
- **Math integer conversion and signed-zero edges**: `Math.clz32` and
  `Math.imul` now use the engine's spec-shaped `ToUint32`/`ToInt32` helpers
  instead of Rust casts, so infinities, `NaN`, modulo-2^32 values, and signed
  multiplication results match ECMAScript. `Math.sign` now preserves `NaN` and
  `-0`. The focused
  `built-ins/Math/{cbrt,clz32,cosh,expm1,fround,imul,log10,log1p,log2,sign,sinh,tanh,trunc}`
  run now closes at **68 pass / 0 fail / 13 skip**.
- **Math max/min/round edge semantics**: `Math.max` and `Math.min` now coerce
  every argument before returning `NaN`, propagate `NaN` after observable
  coercions, and apply the spec signed-zero ordering where `+0` is greater
  than `-0`. `Math.round` now preserves `-0` for `[-0.5, -0]`, returns `+0`
  for positive values below `0.5`, and keeps already-integral large Number
  values unchanged. The focused
  `built-ins/Math/{max,min,round}` run now closes at **28 pass / 0 fail / 3
  skip**.
- **String literal escape conformance**: string literals now decode UTF-8
  `NonEscapeCharacter` escapes such as `\А` as source code points instead of
  corrupting the UTF-8 tail byte, allow literal U+2028/U+2029 in strings per
  JSON-superset source text, decode sloppy legacy octal escapes, and reject
  legacy octal/non-octal decimal escapes in strict-mode strings. The focused
  `language/literals/string` run now closes at **71 pass / 0 fail / 2 skip**,
  and broader `language/literals` improves to **434 pass / 40 fail / 60 skip**.
- **RegExp quantifier early errors**: RegExp literal validation now rejects
  quantifiers that appear before any atom, including `/?/`, `/{2}/`,
  `/{2,}/`, and `/{2,3}/`, and the same validation is shared by
  `new RegExp(pattern)`. Escaped quantifier characters, character classes, and
  normal atom quantifiers such as `/a?/` and `/a{2}/` remain accepted. The
  focused `language/literals/regexp` diagnostic now runs at **144 pass / 36
  fail / 58 skip**, and broader `language/literals` improves to **438 pass /
  36 fail / 60 skip**.
- **RegExp assertion quantifier early errors**: RegExp literal validation now
  rejects quantifiers applied to lookbehind assertions in all modes and to
  lookahead assertions in Unicode mode, while preserving Annex B non-Unicode
  lookahead quantifiers at the lexical validation layer. The `RegExp`
  constructor and parser fallback path share the same validation. The focused
  `language/literals/regexp` diagnostic now runs at **156 pass / 24 fail / 58
  skip**, and broader `language/literals` improves to **450 pass / 24 fail /
  60 skip**.
- **RegExp Unicode-mode syntax early errors**: RegExp literal and constructor
  validation now reject malformed/out-of-range `\u{...}` escapes, invalid
  Unicode-mode identity/control/decimal escapes, bare `{` pattern characters,
  and character-class ranges whose endpoints are multi-character class escapes
  such as `\d` or `\s`. This closes the remaining RegExp literal
  parse-negative bucket. The focused `language/literals/regexp` diagnostic now
  runs at **168 pass / 12 fail / 58 skip**, and broader `language/literals`
  improves to **462 pass / 12 fail / 60 skip**.
- **RegExp Unicode property escape validation**: Unicode-mode RegExp literal
  and constructor validation now parse `\p{...}`/`\P{...}` bodies instead of
  accepting any non-empty ASCII name. Property-less escapes are limited to
  binary properties or `General_Category` values, so bare script names such as
  `\p{Greek}` and loose-cased names such as `\p{Ascii}` now report early
  syntax errors. Explicit `Script=...`, `Script_Extensions=...`, `gc=...`,
  binary-property aliases, and existing modifier/property escape cases remain
  accepted when the backend supports them.
- **RegExp null escapes and UTF-8 literal source**: RegExp literals now keep
  non-ASCII pattern source as Unicode code points instead of UTF-8 byte
  fragments, and the internal regex backend lowers ES `\0` null-character
  escapes to the backend-supported `\x00` form without changing the public
  `source`. `String.prototype.search` now accepts RegExp arguments for these
  probes and returns UTF-16 indices while preserving `lastIndex`. The focused
  `language/literals/regexp` diagnostic now runs at **173 pass / 7 fail / 58
  skip**, and broader `language/literals` improves to **467 pass / 7 fail /
  60 skip**.
- **RegExp sticky start assertions**: `RegExp.prototype.exec` now runs
  global/sticky matches against the full input at the UTF-16 `lastIndex`
  position instead of slicing the input first, so `^` still observes the real
  beginning of input and multiline line starts. Global `lastIndex` updates now
  use the actual match end even when the search skips ahead. The focused
  `language/literals/regexp` diagnostic now runs at **174 pass / 6 fail / 58
  skip**, and broader `language/literals` improves to **468 pass / 6 fail /
  60 skip**.
- **RegExp non-Unicode case folding**: the internal regex backend now
  protects non-ASCII literal atoms and `\uXXXX`/`\xNN` escapes from Rust's
  Unicode case folding when a pattern has `i` without `u`, while preserving
  Unicode case folding for `iu`. This matches ES canonicalization for cases
  such as Kelvin sign `\u212a`. The focused `language/literals/regexp`
  diagnostic now runs at **175 pass / 5 fail / 58 skip**, and broader
  `language/literals` improves to **469 pass / 5 fail / 60 skip**.
- **RegExp Unicode surrogate-pair escapes**: the internal regex backend now
  lowers adjacent Unicode-mode surrogate-pair escapes such as
  `\ud800\udc00` to scalar `\u{...}` backend escapes while preserving the
  public `source` text. Character classes now treat those pairs as one
  Unicode scalar instead of two independent surrogate atoms. The focused
  `language/literals/regexp` diagnostic now runs at **177 pass / 3 fail / 58
  skip**, and broader `language/literals` improves to **471 pass / 3 fail /
  60 skip**, with the remaining RegExp literal failures isolated to
  backreference support.
- **RegExp exec result shape and `lastIndex` coercion**:
  `RegExp.prototype.exec` now returns match arrays with enumerable `index`,
  `input`, and `groups` properties, treats a missing argument as
  `"undefined"`, reads `lastIndex` through ordinary `Get`/`ToLength` on every
  call, and reports `TypeError` when global/sticky `lastIndex` write-back
  fails. Lone surrogate escapes now lower to RuJa's internal surrogate sentinel
  in Unicode mode and to code-unit-aware backend atoms in non-Unicode mode, so
  `/\udf06/u` keeps scalar semantics while `/\udf06/` can match the low half of
  a surrogate pair.
- **RegExp repeated capture clearing**: `RegExp.prototype.exec` now clears
  descendant captures left over from earlier iterations of quantified
  capturing and non-capturing groups when those descendants did not participate
  in the final iteration. This matches ES repeated-capture semantics for cases
  like `/(z)((a+)?(b+)?(c))*/`, where the final optional `(b+)` capture must be
  `undefined` instead of the previous iteration's `"bbb"`, and
  `/(?:(a)|(b))*/`, where `(a)` must be cleared after the final `(b)`
  iteration. The same clearing now feeds `String.prototype.match` and function
  replacement callbacks. The focused `built-ins/RegExp/prototype/exec`
  diagnostic is **75 pass / 0 fail / 4 skip**; the broader
  `built-ins/String/prototype/{match,replace}` diagnostic now closes at
  **100 pass / 0 fail / 6 skip**.
- **String match RegExp creation and `@@match` dispatch**:
  `String.prototype.match` now follows the `@@match` dispatch path before
  ordinary matching, so custom `searchValue[Symbol.match]` getters and methods
  are observable. Values without a custom matcher are converted through a
  `RegExpCreate`-style intrinsic RegExp instead of returning `null`, and that
  internally-created RegExp observes an overridden
  `RegExp.prototype[Symbol.match]` before falling back to RuJa's internal match
  algorithm. The focused
  `built-ins/String/prototype/match` diagnostic now closes at **47 pass / 0
  fail / 4 skip**.
- **String search `@@search` dispatch and RegExp search semantics**:
  `String.prototype.search` now observes custom object
  `searchValue[Symbol.search]` getters and methods, creates an intrinsic
  RegExp for ordinary search values, and lets internally-created RegExp
  objects dispatch through an overridden `RegExp.prototype[Symbol.search]`.
  `RegExp.prototype[Symbol.search]` is now exposed with custom `exec`
  dispatch, object-or-null result validation, strict `lastIndex` writes, and
  restoration of the previous `lastIndex` after the search. The focused
  `built-ins/String/prototype/search
  built-ins/RegExp/prototype/Symbol.search` diagnostic now closes at **61
  pass / 0 fail / 5 skip**.
- **String split `@@split` dispatch and RegExp separator semantics**:
  `String.prototype.split` now has the spec `length` of 2, rejects nullish
  receivers before coercion, observes custom `separator[Symbol.split]`
  getters and methods, and propagates abrupt completions from limit coercion.
  Ordinary separators now use `ToString` in order, while `undefined`
  separators and zero limits follow the expected array result shape. RegExp
  separators now include captured substrings, honor split limits, ignore
  boundary zero-length matches, and normalize additional ES RegExp escapes for
  the backend (`[]`, `[^]`, control escapes, class backspace, and incomplete
  `\x`). The focused `built-ins/String/prototype/split` diagnostic now closes
  at **117 pass / 0 fail / 3 skip**, and the combined
  `built-ins/String/prototype/search built-ins/String/prototype/split
  built-ins/RegExp/prototype/Symbol.search` run closes at **178 pass / 0 fail
  / 8 skip**.
- **String replace substitution tokens and `@@replace` dispatch**:
  `String.prototype.replace` string replacements now expand ECMAScript
  replacement tokens (`$$`, `$&`, ``$` ``, `$'`, `$n`, `$nn`) for both RegExp
  and plain string search values instead of delegating to backend replacement
  syntax or raw `String::replacen`. Unmatched captures substitute as the empty
  string, numeric fallback cases such as `$11` and `$01` match JS behavior,
  and the replacement-string path now uses the same repeated-capture clearing
  as `RegExp.prototype.exec`. Plain replacement now also coerces `searchValue`
  before non-callable `replaceValue`, and custom `searchValue[Symbol.replace]`
  methods are observed before ordinary replacement. The focused
  `built-ins/String/prototype/replace` diagnostic now closes at **53 pass / 0
  fail / 2 skip**.
- **String replace callback offsets**: Function replacements for both
  RegExp and string search values now receive the match offset as a UTF-16
  code-unit index instead of a Rust UTF-8 byte offset, so matches after
  supplementary characters report the same offset that JS exposes through
  string indexing. The focused `built-ins/String/prototype/replace`
  diagnostic now closes at **53 pass / 0 fail / 2 skip**.
- **RegExp named capture groups**: Named captures now feed the shared match
  result surface: `RegExp.prototype.exec` and non-global
  `String.prototype.match` expose a null-prototype `groups` object,
  RegExp function replacements receive that groups object as their final
  argument, and replacement strings expand `$<name>` using the same capture
  metadata. The focused `built-ins/RegExp/prototype/exec
  built-ins/String/prototype/{match,replace}` diagnostic now closes at
  **175 pass / 0 fail / 10 skip**.
- **RegExp backreferences and identity escapes**: RegExp compilation now keeps
  the existing Rust regex fast path for ordinary patterns while routing true
  numeric backreferences through a backtracking-capable backend. Non-Unicode
  legacy decimal escapes and identity escapes that Rust regex does not accept
  are lowered to equivalent backend literals without changing public
  `source`. The focused `language/literals/regexp` diagnostic now closes at
  **180 pass / 0 fail / 58 skip**, and broader `language/literals` closes at
  **474 pass / 0 fail / 60 skip**.
- **Map prototype size accessor**: `Map.prototype.size` is now installed as
  the spec accessor property instead of a data method. The getter has the
  expected `"get size"` name/zero length, validates that the receiver is a
  real Map, and Map instance reads now go through the ordinary prototype
  lookup path so overriding or deleting `Map.prototype.size` is observable.
  The focused `built-ins/Map/prototype/size` cluster now runs at **6 pass / 0
  fail / 5 skip**.
- **RegExp literal line-terminator early errors**: regular-expression
  literals now reject CR, LF, LS, and PS immediately after a backslash instead
  of treating the line terminator as an escaped pattern character. This makes
  parse-negative literals such as `/\\\n/` stop before executing test code and
  also routes `eval()` of those literals through `SyntaxError`. The focused
  `language/literals/regexp` diagnostic now runs at
  **55 pass / 125 fail / 58 skip**, and broader `language/literals` improves
  to **324 pass / 150 fail / 60 skip**.
- **RegExp flags and modifiers early errors**: RegExp literals, parser
  recovery for statement-start regexes, and the `RegExp` constructor now share
  syntax validation for duplicate/invalid flags plus RegExp modifiers groups.
  Modifier groups only accept source-text `i`, `m`, and `s`, reject duplicate
  add/remove flags, reject add/remove intersections, require a colon, and do
  not accept Unicode escapes or case-folded flag spellings. This closes the
  focused modifiers parse-negative cluster while leaving the remaining
  `built-ins/RegExp/regexp-modifiers` failures isolated to runtime modifier
  semantics. The focused `language/literals/regexp` diagnostic now runs at
  **140 pass / 40 fail / 58 skip**, broader `language/literals` improves to
  **409 pass / 65 fail / 60 skip**, and
  `built-ins/RegExp/regexp-modifiers` runs at **37 pass / 33 fail / 0 skip**.
- **RegExp modifiers backend normalization**: the internal regex compiler now
  lowers ES modifier groups with an empty remove-list, such as `(?s-:...)`,
  to the Rust regex backend's equivalent `(?s:...)` form while preserving the
  public `source` string. Constructor validation now uses the same normalized
  compile path as execution. This closes the backend syntax failures for
  add-only modifier groups without changing modifier properties on the
  RegExp instance. The focused `built-ins/RegExp/regexp-modifiers` run now
  improves to **57 pass / 13 fail / 0 skip**.
- **RegExp modifier runtime semantics**: backend normalization now tracks
  modifier-local `s` and `i` state when lowering dot, word-boundary, word
  character, and Unicode property escapes. Non-Unicode `.` now follows ES
  UTF-16 code-unit semantics instead of Rust scalar matching, local
  `(?-i:...)` word escapes use the ES ASCII word set inside and outside
  character classes, and modifier-local `\p{Lu}`/`\P{Lu}` probes plus their
  `Uppercase_Letter` aliases compile both inside and outside character classes
  in Unicode mode. The focused
  `built-ins/RegExp/regexp-modifiers` run now closes at **70 pass / 0 fail /
  0 skip**.
- **RegExp prototype accessors**: `RegExp.prototype.source` and
  `RegExp.prototype.flags` are now accessor properties with spec-shaped
  getter functions. RegExp instances keep their raw pattern and flags in
  internal storage, while the public `source` getter escapes empty patterns,
  slashes, and line terminators for literal reconstruction and the `flags`
  getter reads boolean flag accessors in `dgimsuvy` order. `$262.createRealm()`
  now exposes a realm-local `RegExp` intrinsic with accessor getters bound to
  that realm, so `%RegExp.prototype.source%` accepts only its own realm
  prototype. The focused `built-ins/RegExp/prototype/flags
  built-ins/RegExp/prototype/source` run now closes at **18 pass / 0 fail / 10
  skip**.
- **Proxy-aware `[[HasProperty]]` for `with`/Reflect**:
  internal property-existence checks now route through Proxy `has` traps,
  including revoked-proxy errors and basic non-configurable/non-extensible
  target invariants. `with` object-environment binding lookup, the `in`
  operator, `Array.from` iterator detection, async iterator detection, and
  `Reflect.has` now share the same observable `[[HasProperty]]` path.
  Symbol-key Proxy `get` is used for `Symbol.unscopables`, and
  `Reflect.get`/`set`/`has` now preserve Symbol property keys. `Reflect.set`
  also honors its receiver argument enough for Proxy receivers to observe
  `getOwnPropertyDescriptor`/`defineProperty` during ordinary data-property
  writes, returns `false` instead of `true` for receiver/target
  non-writable failures, and propagates abrupt completions from Proxy `set`
  traps. `Reflect.defineProperty` and `Reflect.getOwnPropertyDescriptor` are
  exposed for this path; `Reflect.defineProperty` now returns `false` for
  failed ordinary definitions instead of throwing, propagates abrupt
  completions while reading descriptor fields, and
  `Reflect.getOwnPropertyDescriptor` now observes Proxy
  `getOwnPropertyDescriptor` trap completions. With the
  `Proxy`/`Reflect` skips temporarily lifted, focused
  `language/statements/with built-ins/Reflect/has` now runs at
  **183 pass / 0 fail / 8 skip**, and the focused
  `built-ins/Reflect/get built-ins/Reflect/set built-ins/Reflect/has
  built-ins/Reflect/defineProperty
  built-ins/Reflect/getOwnPropertyDescriptor` diagnostic now runs at
  **49 pass / 0 fail / 15 skip**.
- **Proxy `set` trap failure propagation for `with` References**: Proxy
  `[[Set]]` now checks the `set` trap's boolean result instead of discarding
  it. Strict `PutValue` through a Proxy-backed `with` object now throws
  `TypeError` when the trap returns a falsy value, while sloppy assignment
  remains a silent failed write. This covers simple, compound, update, and
  logical assignment forms that preserve the same object-environment
  Reference. The focused `language/statements/with` run remains at **169 pass
  / 0 fail / 12 skip**.
- **Proxy-aware `[[Delete]]` for delete/Reflect**:
  property deletion now routes through Proxy `deleteProperty` traps with the
  handler as `this`, preserves string and Symbol property keys, falls through
  to nested proxy targets when the trap is null or missing, and enforces the
  non-configurable/non-extensible target invariants for truthy trap results.
  `Reflect.deleteProperty` now rejects primitive targets and returns the
  actual internal `[[Delete]]` boolean instead of always returning `true`.
  `Proxy.revocable()` now revokes through the native callee rather than the
  call receiver, so revoked proxy deletes throw. The test262
  `$262.createRealm()` host now also exposes the constructable `Proxy`
  constructor on the created global. With `Proxy`, `Reflect`, and
  `proxy-missing-checks` skips temporarily lifted, focused
  `built-ins/Reflect/deleteProperty built-ins/Proxy/deleteProperty` now runs
  at **25 pass / 0 fail / 3 skip**.
- **Array search array-like access**: `Array.prototype.indexOf`,
  `lastIndexOf`, and `includes` now use `LengthOfArrayLike` plus per-index
  property access instead of scanning only RuJa's dense array storage.
  Generic calls on ordinary array-like objects, Boolean/Number primitives
  with prototype `length`/index properties, boxed strings, sparse arrays, and
  holes now follow the expected `HasProperty`/`Get` behavior. Array
  `length` shrinkage now preserves non-configurable indexed own properties,
  so searches still observe those elements after accessor side effects try to
  shorten the receiver. The focused
  `built-ins/Array/prototype/includes
  built-ins/Array/prototype/indexOf
  built-ins/Array/prototype/lastIndexOf` cluster now runs at
  **409 pass / 0 fail / 20 skip**.
- **Array find array-like access**: `Array.prototype.find`, `findIndex`,
  `findLast`, and `findLastIndex` now share the spec order for
  `ToObject(this)`, `LengthOfArrayLike`, predicate callability checks, and
  per-index `Get`. They no longer clone dense Array storage before iteration,
  so array-like receivers, nullish receiver errors, throwing `length`/index
  accessors, callback `thisArg`, holes as `undefined`, and mutations during
  traversal are observable. The focused
  `built-ins/Array/prototype/{find,findIndex,findLast,findLastIndex}`
  diagnostic improves from **38 pass / 24 fail / 32 skip** to **62 pass / 0
  fail / 32 skip**, and the combined Array search/find run closes at
  **471 pass / 0 fail / 52 skip**.
- **Array `at` array-like access**: `Array.prototype.at` now applies
  `ToObject(this)` with nullish receiver errors, reads `length` through
  `LengthOfArrayLike`, uses indexed property access for generic array-like
  receivers, and normalizes `-0` to property key `"0"`. The focused
  `built-ins/Array/prototype/at` run now closes at **11 pass / 0 fail / 2
  skip**.
- **String search argument coercion**: `String.prototype.indexOf` and
  `lastIndexOf` now coerce `searchString` through `ToString` before reading
  the position argument, so missing arguments search for `"undefined"`, object
  search values run observable `toString` first, and abrupt completions occur
  in spec order. `indexOf` positions now clamp negative values to 0 instead
  of using Array-style from-index wrapping, and `lastIndexOf` clamps finite
  negative values to 0 while preserving the `NaN`/`+Infinity` search-from-end
  path. The UTF-16 last-index helper also handles needles longer than the
  haystack without panicking. The focused
  `built-ins/String/prototype/indexOf
  built-ins/String/prototype/lastIndexOf` cluster now runs at
  **62 pass / 0 fail / 10 skip**.
- **String slice/substring argument coercion**:
  `String.prototype.slice` and `substring` now coerce start/end arguments
  through `ToIntegerOrInfinity` in spec order. `slice` now observes object
  `valueOf`/`toString`, propagates abrupt completions from `start` before
  `end`, and treats explicit `undefined` end as the string length.
  `substring` now truncates fractional positions and also treats missing or
  explicit `undefined` end as the string length before clamping/swapping. The
  Math intrinsic object is now extensible like an ordinary ECMAScript object,
  so borrowed string methods assigned onto `Math` are callable. The
  focused
  `built-ins/String/prototype/slice
  built-ins/String/prototype/substring` cluster now runs at
  **80 pass / 0 fail / 4 skip**.
- **String trim whitespace set**: `String.prototype.trim`, `trimStart`, and
  `trimEnd` now use the ECMAScript `WhiteSpace` plus `LineTerminator` set
  instead of Rust's host whitespace predicate, so BOM (`\uFEFF`) is trimmed
  at string boundaries while non-ECMAScript whitespace such as `\u180E` and
  `\u0085` is preserved. RegExp objects constructed from RegExp inputs now
  retain the wrapped pattern source/flags, and arguments objects now stringify
  through their object brand instead of RuJa's internal array storage. The
  focused
  `built-ins/String/prototype/trim
  built-ins/String/prototype/trimStart
  built-ins/String/prototype/trimEnd` cluster now runs at
  **145 pass / 0 fail / 30 skip**.
- **String repeat count coercion**: `String.prototype.repeat` now applies
  `ToIntegerOrInfinity`-style truncation to its count before range checking,
  so `NaN`, `undefined`, `false`, `"0"`, and `0.9` produce the empty string
  instead of throwing, while negative counts and infinities still throw
  `RangeError`. The focused `built-ins/String/prototype/repeat` cluster now
  runs at **13 pass / 0 fail / 3 skip**.
- **String index position coercion**: `String.prototype.charAt`,
  `charCodeAt`, and `codePointAt` now coerce explicit `undefined`, `NaN`,
  non-numeric strings, fractional values, and infinities through the shared
  integer-position path before range checking. This preserves index-0 access
  for `undefined`/`NaN` while keeping negative, infinite, and out-of-range
  positions on the empty/`NaN`/`undefined` result paths. The focused
  `built-ins/String/prototype/charAt
  built-ins/String/prototype/charCodeAt
  built-ins/String/prototype/codePointAt` cluster now runs at
  **66 pass / 0 fail / 5 skip**.
- **Symbol intrinsic surface completion**: `Symbol.length` now has the spec
  value/descriptor, `Symbol.prototype.valueOf` is exposed with Symbol wrapper
  validation, `Object.getPrototypeOf(Symbol())` returns `Symbol.prototype`,
  and nullish `Object.getPrototypeOf` inputs throw `TypeError`. The remaining
  well-known Symbol constructor properties
  (`isConcatSpreadable`, `matchAll`, `replace`, `search`, `split`, plus the
  existing well-known properties) are now installed as non-writable,
  non-enumerable, non-configurable data properties with stored descriptions.
  Array, Map, Promise, RegExp, and Set now expose named
  `get [Symbol.species]` accessors that return the receiver, so subclass
  species lookup follows the inherited accessor path. With the `Symbol`
  feature skip temporarily lifted, the whole `built-ins/Symbol` diagnostic now
  runs at **67 pass / 0 fail / 31 skip**.
- **`new.target` eval-context early errors**: `new.target` is now rejected in
  script/global code, indirect eval code, and direct eval code reached through
  arrow-function code, while direct eval inside non-arrow function code sees
  the caller's active `new.target`. Function parameter defaults also parse
  `new.target` in the same ordinary-function context. The focused
  `language/global-code language/eval-code` cluster now runs at
  **331 pass / 0 fail / 58 skip**.
- **Symbol description/keyFor registry semantics**: Symbols now retain
  optional descriptions, well-known Symbols expose spec-style descriptions,
  `Symbol.prototype.description` and `Symbol.keyFor` are implemented with
  the right primitive/wrapper receiver validation, and `String(Symbol(...))`
  / `Symbol.prototype.toString` include descriptions. Test262-created realms
  now receive distinct `Symbol`, `Symbol.for`, and `Symbol.keyFor` function
  objects while sharing the VM-level global Symbol registry, closing the
  cross-realm registry cases. Sloppy writes to coercible Symbol primitives are
  ignored while strict writes still throw, and nullish member assignments keep
  throwing in sloppy mode. Focused
  `built-ins/Symbol/for built-ins/Symbol/keyFor
  built-ins/Symbol/prototype/description built-ins/Symbol/prototype/toString`
  with the `Symbol` feature skip temporarily lifted runs at **28 pass / 0
  fail / 4 skip**.
- **`Reflect.ownKeys` Symbol key and Proxy abrupt-completion coverage**:
  `Reflect.ownKeys` now rejects primitive targets with `TypeError`, returns
  the full `[[OwnPropertyKeys]]` list by preserving non-enumerable string keys
  and Symbol keys in spec order, and propagates abrupt completions from Proxy
  `ownKeys` trap result conversion instead of falling back to target keys.
  `Symbol.for` now uses a VM-level global symbol registry for repeat-key
  identity, which closes the Symbol-backed `Reflect.ownKeys` ordering case.
  Focused `built-ins/Reflect/ownKeys` with `Proxy`/`Reflect`/`Symbol` feature
  skips temporarily lifted now runs at **13 pass / 0 fail / 0 skip**.
- **Destructuring assignment Reference target preservation**: identifier
  targets in object and array destructuring assignments now capture the
  spec Reference before reading the source property, stepping the iterator, or
  evaluating a default initializer. This keeps `with` object-environment
  targets stable even when a getter, iterator step, or default expression
  deletes the selected property before `PutValue`. The focused Reference
  cluster
  `language/statements/with language/expressions/assignment
  language/expressions/destructuring language/expressions/compound-assignment
  language/expressions/logical-assignment language/expressions/delete` runs at
  **904 pass / 0 fail / 363 skip**, and the supported subset remains
  **5003 pass / 0 fail / 0 timeout**.
- **Object assignment shorthand defaults**: object-literal cover grammar now
  accepts shorthand default forms such as `{ x = 1 }` long enough for simple
  destructuring assignment (`{ x = 1 } = rhs`) to consume them as assignment
  patterns, while ordinary object literals and compound assignments still
  reject the form as a `SyntaxError`. With diagnostic feature skips
  temporarily lifted, `language/expressions/assignment
  language/statements/for-in language/statements/for-of` improves from
  **1009 pass / 220 fail / 122 skip** to
  **1022 pass / 207 fail / 122 skip**.
- **Object prototype receiver coercion**: `Object.prototype.valueOf` now
  applies `ToObject(this)` and rejects nullish receivers, so primitive
  receivers produce wrapper objects while detached calls throw `TypeError`.
  `Object.prototype.toLocaleString` now performs the observable
  `Invoke(this, "toString")` path instead of aliasing
  `Object.prototype.toString`, preserving primitive receivers for strict
  user-defined `toString` methods and propagating accessor/call failures.
  The focused `built-ins/Object/prototype/valueOf
  built-ins/Object/prototype/toLocaleString` cluster now runs at
  **30 pass / 0 fail / 2 skip**, closing 10 full-suite failures.
- **Object legacy accessor methods**: `Object.prototype.__defineGetter__`,
  `__defineSetter__`, `__lookupGetter__`, and `__lookupSetter__` are now
  installed with spec-shaped `name`/`length` descriptors. The define methods
  apply the required `ToObject`/callable/key-coercion order, define enumerable
  configurable accessors through the ordinary `DefinePropertyOrThrow` path,
  and preserve an existing complementary getter or setter. The lookup methods
  walk ordinary prototype chains and return the first accessor getter/setter,
  or `undefined` for data properties and missing accessors. The focused
  legacy accessor cluster now runs at **42 pass / 0 fail / 12 skip**.
- **Object prototype `__proto__` accessor and prototype mutation**:
  `Object.prototype` now has a null `[[Prototype]]` and the Annex B
  `__proto__` accessor with named getter/setter functions. Ordinary
  `__proto__` access now flows through the inherited accessor instead of a
  VM-wide shortcut, so null-prototype objects and own data properties shadow
  it correctly. `Object.setPrototypeOf`, `Reflect.setPrototypeOf`, and the
  legacy setter share the same prototype mutation status path, rejecting
  immutable `Object.prototype`, non-extensible targets, and ordinary cycles
  while allowing the Proxy-shadowed cycle case required by test262.
  `Object.prototype.isPrototypeOf` now follows the specified nullish receiver
  order. The focused `built-ins/Object/prototype` run now closes at
  **191 pass / 0 fail / 57 skip**.
- **`Object.assign` target/source semantics**: `Object.assign` now applies
  `ToObject` to primitive targets, skips nullish sources, copies enumerable
  string and Symbol keys in own-key order, and throws `TypeError` when the
  required `Set(..., Throw=true)` operation fails. Property-key based
  `[[Get]]`/`[[Set]]` now observes array dense elements, string exotic
  indices/`length`, and array receiver index/length writes, while Proxy
  `ownKeys` trap order is preserved for normal array key-list results. The
  focused `built-ins/Object/assign` run now closes at
  **25 pass / 0 fail / 13 skip**.
- **`Object.fromEntries` entry coercion**: `Object.fromEntries` now rejects
  nullish iterables, requires each entry value to be an object, reads
  `entry[0]`/`entry[1]` through ordinary property access instead of only
  unpacking array storage, and preserves Symbol property keys via
  `ToPropertyKey`. Boxed string entries such as `Object("ab")` now create
  `{ a: "b" }`, while primitive string entries throw `TypeError`. The focused
  `built-ins/Object/fromEntries` run now closes at
  **11 pass / 0 fail / 14 skip**.
- **`Object.groupBy` static grouping**: `Object.groupBy` is now exposed as a
  static built-in, iterates arbitrary sync iterables, calls the grouping
  callback with `(value, index)`, converts callback results with
  `ToPropertyKey`, preserves Symbol group keys, returns a null-prototype
  result object, and closes custom iterators when callback/key coercion
  abruptly completes. The focused `built-ins/Object/groupBy` run now closes at
  **13 pass / 0 fail / 1 skip**.
- **Native Error constructor shape**: Native Error constructors now inherit
  from `%Error%` instead of directly from `%Function.prototype%`, expose own
  non-enumerable `name`/`length` properties, and keep their prototype objects
  as ordinary objects rather than Error-branded instances. The focused
  `built-ins/Object/getPrototypeOf built-ins/NativeErrors` run now closes at
  **118 pass / 0 fail / 15 skip**.
- **Promise built-in surface expansion**: `Symbol.species` is now exposed,
  `String(Symbol(...))` follows the special `String` constructor path instead
  of ordinary `ToString`, and `Promise` exposes `all`, `race`, `allSettled`,
  `any`, `try`, `withResolvers`, `prototype.finally`, and the
  `Promise[@@species]` accessor. Promise resolve/reject functions created by
  the constructor are now anonymous unary built-ins with the expected
  `length`/`name` descriptors and no own `prototype`. Static
  `Promise.resolve` and `Promise.reject` now create and invoke a
  `NewPromiseCapability` from their receiver constructor, so subclass/custom
  constructor capabilities and bad receivers follow the spec path.
  `Promise.prototype.catch` and `Promise.prototype.finally` now invoke the
  receiver's observable `then` property instead of bypassing it through RuJa's
  internal Promise path. `Promise.prototype.then` now validates its receiver
  as a real Promise, resolves the derived promise through
  `SpeciesConstructor`, and stores Promise reaction capabilities so custom
  species constructors and capability executor validation follow the spec path.
  The `Promise` constructor now rejects calls made without `new` and invokes
  its executor with `undefined` as the receiver, letting ordinary sloppy
  functions see `globalThis` while strict executors preserve `undefined`.
  `Promise.try` now creates its result through `NewPromiseCapability(this)`,
  so subclass/custom receivers, constructor abrupt completions, and
  non-constructor receiver validation follow the spec path. Class computed
  method and accessor names now use `ToPropertyKey`, and method definition
  preserves Symbol keys, so `static get [Symbol.species]` defines the
  well-known Symbol property instead of a string-named property. Promise
  reactions that return an already-settled Promise now schedule direct
  pass-through adoption instead of storing an undrainable handler, avoiding
  hangs while preserving pending-Promise adoption.
  `Promise.race` now constructs through the receiver capability, reads
  `C.resolve` once, and invokes each resolved entry's observable `then` with
  the capability resolve/reject functions. `Promise.all` now follows the same
  constructor capability and `C.resolve` path, creates per-element resolving
  functions, invokes each resolved entry's observable `then`, and resolves the
  outer capability with the ordered result array. `Promise.allSettled` now
  follows the same constructor capability and `C.resolve` path, creates paired
  per-element resolve/reject functions sharing an `alreadyCalled` guard,
  records ordered fulfilled/rejected result objects, and rejects the outer
  capability if the final capability resolve abruptly completes. `Promise.all`
  also now rejects its outer capability if the final capability resolve
  abruptly completes. `Promise.any` now follows the receiver constructor
  capability and `C.resolve` path, invokes observable `then`, tracks
  per-element rejection functions with `alreadyCalled` guards, preserves
  rejection order, and rejects with a minimal `AggregateError` carrying a
  non-enumerable `errors` array. `Promise.allKeyed` and
  `Promise.allSettledKeyed` are now exposed for the `await-dictionary`
  proposal surface: they construct through the receiver capability, read
  `C.resolve` once, enumerate own enumerable string and Symbol keys, preserve
  key order independently of settlement order, invoke each resolved entry's
  observable `then`, and resolve to a null-prototype keyed result object.
  The diagnostic `built-ins/Promise` run with only the `Promise` feature skip
  lifted is now **255 pass / 0 fail / 0 timeout / 448 skip**. Focused
  `built-ins/Promise/allKeyed built-ins/Promise/allSettledKeyed` runs at
  **18 pass / 0 fail / 45 skip**, `built-ins/Promise/any` runs at
  **26 pass / 0 fail / 68 skip**, and the
  `all`/`race`/`allSettled`/`any`/`resolve` diagnostic cluster runs at
  **136 pass / 0 fail / 284 skip**. The Promise skip remains in the supported
  runner until the broader skipped async/proposal coverage is intentionally
  lifted.
- **`super`/`for-of` feature lift**: method parameter default initializers now
  preserve the enclosing method's `super` property parse context while still
  rejecting direct `super()` calls, and non-declaration
  `for ([x] of iterable)` / `for ({x} of iterable)` heads now use the existing
  destructuring-assignment compiler path instead of discarding the iterator
  value. `super` and `for-of` are removed from the test262 skip filters after
  focused verification over `language/statements/for-of` and object method
  definitions ran at **134 pass / 0 fail / 920 skip**, raising the supported
  subset to **5003 pass / 0 fail / 0 timeout**.
- **ES2015 syntax/global feature lift**: `computed-property-names`,
  `rest-parameters`, `object-spread`, and `globalThis` are removed from the
  test262 skip filters after the supported subset verified at 0 failures. The
  focused computed/object cluster runs at **370 pass / 0 fail / 848 skip**,
  the call/new/array/super spread cluster at **217 pass / 0 fail / 80 skip**,
  and the class/function/arrow cluster at **1117 pass / 0 fail / 8367 skip**.
  This raises the supported subset to **5000 pass / 0 fail / 0 timeout**.
- **Class feature lift**: `class` is removed from the test262 skip filters
  after tightening class numeric method/accessor names, `static constructor`
  parsing, class-element early errors, and dynamic `super()` constructor
  lookup through the active class constructor's current `[[Prototype]]`.
  `super(...)` now evaluates arguments before the not-a-constructor check,
  including spread calls, so catchable TypeErrors match test262 ordering. The
  focused class directories run at **522 pass / 0 fail / 7904 skip**, raising
  the supported subset to **4741 pass / 0 fail / 0 timeout**.
- **Thrown custom object display**: uncaught ordinary objects created by custom
  constructors now include their prototype constructor name in the host error
  message, preserving test262's `Test262Error` signal without changing the
  caught thrown value. This closes the remaining `language/line-terminators`
  failures, raising that shard to **41 pass / 0 fail / 0 skip**.
- **Statement-list regex literal recovery**: parser primary-expression
  handling now recovers regular expression literals that the eager lexer can
  only tokenize as `/` after a preceding block-like statement boundary. This
  closes the `language/statementList` regex-literal failures, raising that
  shard to **60 pass / 0 fail / 20 skip**.
- **Block-scope declaration early errors**: block statement-list early-error
  checks now treat block-level function declarations as lexical declarations
  and include nested statement `var` names in a block's `VarDeclaredNames`.
  `for-in`/`for-of` declaration heads now also reject multiple declarators.
  This closes the focused `language/block-scope` failures, raising that shard
  to **94 pass / 0 fail / 51 skip**.
- **Escaped reserved-word early errors**: identifiers containing Unicode
  escapes now remain identifier-name tokens instead of being promoted to
  keyword/literal tokens, and reserved words such as escaped `true`, `false`,
  `null`, or `var` are rejected in identifier-reference, binding, shorthand,
  and label positions. Escaped reserved words still work as property names.
  The focused
  `language/literals/boolean language/literals/null language/reserved-words
  language/keywords language/future-reserved-words` cluster now runs at
  **113 pass / 0 fail / 1 skip**, and `language/literals` improves to
  **315 pass / 159 fail / 60 skip**.
- **Destructuring assignment feature lift**: object/array destructuring
  assignment patterns now reject escaped reserved words when they would become
  binding identifiers, including shorthand object assignment properties and
  arrow/function destructuring parameters, while escaped reserved words remain
  valid property names in renamed patterns. `destructuring-assignment` is
  removed from the test262 skip filters at **135 pass / 0 fail / 6 skip**,
  raising the supported subset to **4470 pass / 0 fail / 0 timeout**.
- **`with` object-environment HasBinding**: `with` statements now box
  primitive binding objects with `ToObject` after the nullish TypeError check,
  and object-environment binding lookup uses `[[HasProperty]]` over the
  prototype chain instead of own-property checks. Inherited `with` properties
  now resolve for reads, calls, assignments, and compound assignments, while
  primitive strings expose `length` inside `with`. The focused
  `language/statements/with language/expressions/assignment
  language/expressions/prefix-increment language/expressions/prefix-decrement
  language/expressions/postfix-increment
  language/expressions/postfix-decrement` cluster runs at **398 pass / 0 fail
  / 410 skip**.
- **`with` `@@unscopables` HasBinding**: `Symbol.unscopables` is now exposed
  on the `Symbol` constructor, and `with` object environment records consult
  it after a successful `[[HasProperty]]` check. Object-valued unscopables can
  hide bindings, primitive unscopables values are ignored, abrupt getters
  propagate, and strict reads/writes re-check properties deleted by the
  unscopables getter. This closes `language/statements/with` at **169 pass / 0
  fail / 12 skip** and moves the Reference-focused with/assignment/inc/dec
  cluster to **409 pass / 0 fail / 399 skip**.
- **`delete` through `with` object environments**: identifier deletion now
  routes `with` object environment records through the same `[[HasProperty]]`
  and `Symbol.unscopables` HasBinding logic used by reads and writes before
  applying ordinary property deletion. This preserves inherited `with`
  bindings, leaves unscopables-hidden properties untouched while falling
  through to outer bindings, and propagates abrupt unscopables getters. The
  focused `language/statements/with language/expressions/delete` run stays at
  **235 pass / 0 fail / 15 skip**, and the broader Reference-focused delete
  cluster runs at **404 pass / 0 fail / 409 skip**.
- **`typeof` through `with` object environments**: `typeof identifier` now
  creates the same spec Reference record as ordinary identifier evaluation
  before applying `GetValue`, so `with` object properties, inherited
  properties, `Symbol.unscopables`, abrupt unscopables getters, and TDZ
  bindings are all observed correctly. The focused
  `language/statements/with` run stays at **169 pass / 0 fail / 12 skip**,
  while the broader Reference-focused cluster now runs at **900 pass / 0 fail
  / 367 skip**.
- **Identifier writes through destructuring and `for-in`/`for-of` heads**:
  destructuring-assignment identifier targets and non-declaration
  `for-in`/`for-of` identifier heads now create a spec Reference record before
  `PutValue`, matching ordinary assignment. This preserves `with`
  object-environment `[[HasProperty]]`, inherited binding, and
  `Symbol.unscopables` behavior instead of writing through the current
  environment directly.
- **Direct eval through `with` object environments**: unqualified `eval(...)`
  calls now resolve the callee at runtime before deciding whether the call is
  direct eval. A `with` object can shadow `eval` with an ordinary function,
  abrupt `eval` getters propagate before argument evaluation, and
  `with ({ eval }) { eval(src) }` still stays direct when the resolved value
  is the current Realm's intrinsic `%eval%`. The focused
  `language/statements/with` run stays closed at **169 pass / 0 fail / 12
  skip**, and the supported subset remains at **4276 pass / 0 fail / 16162
  skip** while closing this untracked Reference/eval edge.
- **Private-field assignment targets**: private-field update, compound, and
  logical assignments now preserve the evaluated private reference base instead
  of re-evaluating the object expression or only returning the computed value.
  `obj.#x++`, `obj.#x += y`, and `obj.#x ||= y` now update the private slot or
  accessor through the same object, and logical short-circuit paths keep the
  existing value as the expression result. The focused
  `language/expressions/compound-assignment
  language/expressions/logical-assignment language/expressions/update` run is
  **463 pass / 0 fail / 69 skip**; private class feature tests remain skipped
  by the runner, so local class regression tests cover this edge.
- **Private-name delete early errors**: strict/class code now rejects
  `delete obj.#x` and covered forms such as `delete (g().#x)` at parse time
  instead of compiling them and reaching `$DONOTEVALUATE()`. With private class
  feature skips temporarily lifted, the focused
  `language/statements/class/elements/syntax/early-errors/delete
  language/expressions/class/elements/syntax/early-errors/delete` diagnostic
  improves from **0 pass / 48 fail / 144 skip** to **48 pass / 0 fail / 144
  skip**, and the broader private class early-error diagnostic is now
  **136 pass / 60 fail / 248 skip**. The default supported-subset count is
  unchanged because those private-feature tests remain skipped.
- **Private-bound-name early errors**: class parsing now rejects private names
  named `#constructor` and duplicate private bound names across static and
  instance elements, while still allowing the spec's one private getter plus
  one private setter exception. With private class feature skips temporarily
  lifted, the broader private class early-error diagnostic improves from
  **136 pass / 60 fail / 248 skip** to **162 pass / 34 fail / 248 skip**. The
  default supported-subset count is unchanged because those private-feature
  tests remain skipped.
- **Private-name reference early errors**: the parser now applies
  `AllPrivateNamesValid` after building the AST, so class methods, nested
  functions, nested classes, static blocks, computed names, and initializers
  reject undeclared private-name references while preserving lexical access to
  outer class private names. `super.#x` is rejected as a syntax error. With
  private class feature skips temporarily lifted, the broader private class
  early-error diagnostic improves from **162 pass / 34 fail / 248 skip** to
  **196 pass / 0 fail / 248 skip**. The default supported-subset count is
  unchanged because those private-feature tests remain skipped.
- **Private method function names**: private methods now compile their function
  `name` property with the spec `#name` display form instead of the bare
  identifier while keeping the internal private slot key unchanged. With
  private class feature skips temporarily lifted over
  `language/statements/class language/expressions/class`, the diagnostic
  improves from **934 pass / 170 fail / 7322 skip** to **936 pass / 168 fail /
  7322 skip**. The default supported-subset count is unchanged because those
  private-feature tests remain skipped.
- **Private async/generator method heads**: class bodies now parse private
  `async #name()`, `* #name()`, and `async * #name()` method heads, including
  static forms, so they preserve their async/generator flags while using the
  private method lowering path. With private class feature skips temporarily
  lifted over `language/statements/class language/expressions/class`, the
  diagnostic improves from **936 pass / 168 fail / 7322 skip** to **948 pass /
  156 fail / 7322 skip**. The default supported-subset count is unchanged
  because those private-feature tests remain skipped.
- **Strict destructuring assignment targets**: strict-mode destructuring
  assignment patterns now reject `eval` and `arguments` targets recursively,
  including non-declaration `for-in`/`for-of` heads. With the
  `destructuring-binding` diagnostic temporarily lifted over
  `language/expressions/assignment language/statements/for-in
  language/statements/for-of`, the result improves from **1003 pass / 226 fail /
  122 skip** to **1009 pass / 220 fail / 122 skip**. The default
  supported-subset count is unchanged because destructuring-binding tests
  remain skipped.
- **Class static block feature lift**: class static blocks now parse as
  dedicated static initialization blocks instead of ordinary function bodies,
  so `return` is rejected, `super.prop` is accepted, and static-block early
  errors reject direct `await`, `yield`, `arguments`, and duplicate labels
  without crossing function/static-block boundaries. Class methods now carry
  async-method metadata through compilation. The `class-static-block` feature
  is removed from the test262 skip filters; the supported subset moves to
  **4335 pass / 0 fail / 16103 skip**.
- **Arrow lexical `new.target`**: arrow closures now capture their enclosing
  frame's `new.target` at creation time and reuse it when executing later,
  including arrows returned from constructors. `optional-catch-binding` and
  `new.target` are now removed from the test262 skip filters. The focused
  `language/statements/try language/expressions/new.target
  language/expressions/arrow-function` cluster runs at **204 pass / 0 fail /
  354 skip**, and the supported subset moves to **4215 pass / 0 fail /
  16223 skip**.
- **`for-in-order` enumeration**: `for...in`, JSON object serialization, and
  JSON reviver traversal now use ES own-property ordering: array-index keys in
  ascending numeric order followed by string keys in insertion order.
  `Object.create(proto, descriptors)` now applies its descriptor map, so
  non-enumerable own properties correctly shadow inherited enumerable keys.
  The `for-in-order` feature is now removed from the test262 skip filters; its
  9 metadata tests run at **9 pass / 0 fail**, and the supported subset moves
  to **4219 pass / 0 fail / 16219 skip**.
- **Logical-assignment feature lift**: member logical assignments now perform
  the nullish-base `ToObject` check after evaluating the computed property
  expression but before `ToPropertyKey`, and identifier logical assignments
  now apply NamedEvaluation to anonymous function, arrow, and class RHS values.
  `logical-assignment-operators` is removed from the test262 skip filters; the
  focused `language/expressions/logical-assignment` directory runs at **57
  pass / 0 fail / 21 skip**, the Reference-focused with/assignment/logical
  assignment/update cluster runs at **338 pass / 0 fail / 406 skip**, and the
  supported subset moves to **4276 pass / 0 fail / 16162 skip**.
- **Mapped arguments exotic descriptors**: non-strict arguments objects now
  use `Object.prototype`, expose `length` as a configurable ordinary data
  property, report `Array.isArray(arguments) === false`, and keep mapped
  parameter bindings synchronized through descriptor redefinitions until an
  accessor descriptor or non-writable data descriptor unmaps the index.
  Computed deletion of arguments properties now shares the same
  configurability checks as direct deletion, and accessor indices no longer
  fall through to dense element storage when no getter is present. Sloppy
  function `caller` lookup now supports the Annex B call-stack path needed by
  `arguments.callee.caller`, while strict callers remain restricted. Member
  calls with spread arguments now preserve their receiver and spread arity.
  The focused `language/arguments-object` cluster now runs at **126 pass / 0
  fail / 137 skip**.
- **Logical-assignment Reference preservation**: identifier logical
  assignments (`&&=`, `||=`, `??=`) now keep the original spec Reference from
  `GetValue` through `PutValue`, so a `with` or global-object property deleted
  by the RHS is written back through the original reference instead of
  re-resolving to an outer binding. Member logical assignments now also clean
  up their saved target pair on short-circuit paths, preserving the existing
  value as the expression result. After the feature lift above, the focused
  `language/statements/with language/expressions/assignment
  language/expressions/logical-assignment language/expressions/update` cluster
  runs at **338 pass / 0 fail / 406 skip** with additional regression
  coverage for these Reference edges.
- **Strict directive and future-reserved-word early errors**: sloppy bindings
  may now use strict-only future reserved words such as `implements`,
  `interface`, `package`, `private`, `protected`, `public`, `static`, and
  `yield`, while `enum` remains always reserved and strict binding/
  identifier-reference positions reject the full strict-only set. String
  literal tokens now remember whether they contained an escape sequence or line
  continuation, so escaped `"use strict"` spellings no longer create strict
  mode and Function-constructor/direct-eval strict bodies report `SyntaxError`
  for reserved identifier references. The focused
  `language/future-reserved-words language/directive-prologue` cluster now
  runs at **117 pass / 0 fail / 0 skip**.
- **Identifier Unicode tables and reserved binding names**: identifier lexing
  now uses Unicode identifier property tables with the ES `$`, `_`, ZWNJ/ZWJ,
  and grandfathered `Other_ID_Start`/`Other_ID_Continue` additions, while
  invalid Pattern_Syntax characters such as U+2E2F surface as `SyntaxError`
  instead of accidental binding names. `import` and `export` are now rejected
  as binding names in variable declarations, function names, and parameters.
  The focused `language/identifiers` cluster now runs at **208 pass / 0 fail /
  60 skip**, and the CI subset now runs locally at **866 pass / 0 fail / 0
  timeout**.
- **Object.values/Object.entries enumerable snapshot semantics**:
  `Object.values` and `Object.entries` now perform the required nullish
  `ToObject` check, re-check each snapshotted own string key's current
  enumerable descriptor before reading its value, and omit keys deleted or
  made non-enumerable by earlier getters. Empty-handler Proxies now forward
  own-key, own-descriptor, `hasOwnProperty`, and `Object.defineProperty`
  no-trap operations to their targets for this path. The focused
  `built-ins/Object/values built-ins/Object/entries built-ins/Object/hasOwn
  built-ins/Object/getOwnPropertyDescriptors` cluster now runs at **98 pass /
  0 fail / 23 skip**.
- **String static constructors**: `String.fromCodePoint` now throws
  `RangeError` for non-integral, non-finite, negative, and out-of-range code
  point inputs instead of silently truncating through integer casts.
  `String.raw` now appends the empty string when a substitution is missing,
  rather than the string `"undefined"`, while still converting explicit
  `undefined` raw segments through `ToString`. The focused
  `built-ins/String/fromCodePoint built-ins/String/raw
  built-ins/String/fromCharCode` cluster now runs at **51 pass / 0 fail / 7
  skip**.
- **Function-code `this` binding and primitive receiver semantics**:
  non-strict interpreted functions now apply the required `this` binding
  conversion, mapping nullish receivers to the global object and boxing
  primitive receivers with `ToObject`, while strict functions preserve the raw
  receiver. Primitive prototype accessor lookup now keeps the original
  primitive receiver through the prototype chain so strict getters see the
  primitive and sloppy getters receive the boxed object.
- **Function declaration instantiation edges**: non-strict duplicate
  parameters now keep distinct raw argument slots so an omitted later duplicate
  initializes the shared binding to `undefined`; `var` declarations now reuse
  parameter bindings instead of reporting lexical redeclarations; function
  declarations overwrite parameter and `arguments` bindings; and strict
  block-level function declarations stay block-scoped instead of leaking
  through Annex B hoisting. The focused `language/function-code` cluster now
  runs at **217 pass / 0 fail / 0 skip**.
- **Global declaration instantiation edges**: global scripts now perform
  declaration-instantiation preflight for lexical/global-var collisions,
  restricted global properties, and non-extensible global objects before any
  script body side effect runs. Global function declarations now use
  `CreateGlobalFunctionBinding` descriptor rules, global `var` declarations
  use `CreateGlobalVarBinding`, sloppy direct-eval global `var` properties are
  configurable, and strict global block-level function declarations remain
  block-scoped. The focused `language/global-code` cluster now runs at **31
  pass / 0 fail / 11 skip**.
- **Eval global declaration bindings**: non-strict direct and indirect eval
  now preflight global `var`/function declarations with
  `EvalDeclarationInstantiation`-style checks and create configurable global
  eval bindings, while `$262.evalScript()` keeps script-global
  non-configurable binding semantics. Same-Realm indirect eval also gets a
  fresh lexical environment so eval-local lexical and strict `var`/function
  bindings do not leak to the global object. Direct eval now uses the caller's
  variable environment for non-strict `var`/function declarations, preserves
  existing local var bindings during eval declaration instantiation, makes
  newly-created local eval bindings deletable, respects `with` object lookup
  inside eval source, and reflects cross-Realm indirect eval declarations on
  that Realm's global object. Direct eval during function, arrow, and
  generator parameter initialization now rejects `var arguments` declarations
  against an existing arguments binding, and generator calls now run parameter
  and declaration-instantiation bytecode before returning the suspended
  generator object. The focused `language/eval-code` cluster now runs at
  **225 pass / 0 fail / 122 skip**.
- **Numeric literal early errors**: numeric and BigInt literal lexing now
  rejects malformed radix prefixes, invalid numeric separator placement,
  BigInt suffixes on fractional/exponent/legacy-octal-like forms, and
  identifier-start characters immediately after numeric literals. Legacy
  octal and non-octal decimal literals are preserved for sloppy mode but now
  surface as `SyntaxError` in strict mode. The focused
  `language/literals/bigint language/literals/numeric` cluster now runs at
  **216 pass / 0 fail / 0 skip**, and broader `language/literals` improves to
  **312 pass / 162 fail / 60 skip**.
- **Unicode whitespace/comment lexing and `String.fromCharCode` coercion**:
  lexer whitespace/comment handling now recognizes ES Unicode space separators,
  treats only CR/LF/LS/PS as line terminators, reports unterminated multiline
  comments and regular-expression literals, and preserves ASI newline tracking
  across multiline comments. `String.fromCharCode` now applies `ToNumber` and
  `ToUint16` to every argument instead of ignoring non-number values. The
  focused `language/comments language/white-space` cluster now runs at **85
  pass / 0 fail / 34 skip**, and the CI subset rises to **823 pass / 41 fail /
  2 timeout** locally.
- **test262 `$262.createRealm()` and native constructability**: added a
  test262 host object with `createRealm()`/`evalScript()`, realm-bound global
  `eval` and `parseInt` functions, and indirect eval execution against the
  callee's Realm environment. Native functions are now constructable only when
  marked with an internal constructor prototype, so `new parseInt` throws while
  `new Proxy(...)` remains constructable without exposing `Proxy.prototype`.
  This closes the remaining cross-realm eval/template/non-constructor checks
  plus the Proxy subclass edge case, raising the supported subset to **4180
  pass / 0 fail / 0 timeout**.
- **Small language conformance edges**: rest-parameter functions now create
  unmapped arguments objects in sloppy mode, non-extensible objects reject
  `__proto__` prototype mutation, and Symbol-keyed assignment respects
  accessors, inherited setters, non-writable descriptors, and extensibility.
  Parser/lexer handling now rejects a line terminator after `throw`, reports
  unterminated string literals, rejects reserved-word object-literal shorthand
  such as `({ this })`, and accepts `undefined` as a `var` binding name. The
  focused language cluster
  `language/asi language/computed-property-names language/keywords
  language/rest-parameters language/types` now runs at **290 pass / 0 fail / 9
  skip**.
- **String search methods and `Symbol.match`**: aligned
  `String.prototype.includes`, `startsWith`, and `endsWith` with
  `RequireObjectCoercible`, `IsRegExp`, `ToString`, and position/end-position
  ordering. `Symbol.match` is now exposed, `Object.defineProperty` preserves
  Symbol property keys, property lookup invokes accessor getters for
  Symbol-keyed `@@match`, and generated Symbols no longer collide with
  well-known Symbols. The focused String search cluster now runs at **63 pass /
  0 fail / 12 skip**.
- **Sparse array holes and own-key enumeration**: dense arrays now track
  whether each backing-store slot is actually present, so array literal
  elisions, `Array(length)` holes, `delete array[index]`, `hasOwnProperty`,
  `propertyIsEnumerable`, `Object.keys`, and
  `Object.getOwnPropertyNames` agree on absent elements. `for...in` now uses
  the same present-bit model for arrays and includes boxed String exotic
  indices. The focused Object own-key cluster now runs at **90 pass / 0 fail /
  14 skip**.
- **ArrayBuffer/DataView prototype accessors and detach host hook**:
  `ArrayBuffer.prototype.byteLength` and DataView `buffer`/`byteLength`/
  `byteOffset` are now installed as spec-visible accessor properties with
  named getter functions and receiver validation. The test262 host object now
  exposes `$262.detachArrayBuffer()`, ArrayBuffers track detached state, and
  detached ArrayBuffers/DataViews report the required byteLength/byteOffset
  behavior. The focused built-ins accessor cluster now runs at **29 pass / 0
  fail / 19 skip**.
- **DataView 8-bit element accessors**: implemented
  `DataView.prototype.getUint8`, `getInt8`, `setUint8`, and `setInt8` with
  DataView receiver validation, `ToIndex` byte-offset conversion, setter value
  conversion ordering, detached-buffer checks, byte-range validation, Uint8
  wrapping writes, and signed Int8 reads. The focused DataView 8-bit method
  cluster now runs at **49 pass / 0 fail / 29 skip**.
- **DataView 16-bit element accessors**: implemented
  `DataView.prototype.getUint16`, `getInt16`, `setUint16`, and `setInt16` with
  big-endian defaults, `ToBoolean` little-endian handling, Uint16 wrapping
  writes, signed Int16 reads, and the same `ToIndex`/value/detached/range
  validation ordering as the 8-bit methods. The focused DataView 16-bit
  method cluster now runs at **56 pass / 0 fail / 28 skip**.
- **DataView 32-bit element accessors**: implemented
  `DataView.prototype.getUint32`, `getInt32`, `setUint32`, and `setInt32` with
  big-endian defaults, `ToBoolean` little-endian handling, Uint32 wrapping
  writes, signed Int32 reads, and the same `ToIndex`/value/detached/range
  validation ordering as the smaller DataView integer methods. The focused
  DataView 32-bit method cluster now runs at **56 pass / 0 fail / 38 skip**.
- **DataView floating-point element accessors**: implemented
  `DataView.prototype.getFloat32`, `setFloat32`, `getFloat64`, and
  `setFloat64` with IEEE-754 byte encoding/decoding, big-endian defaults,
  `ToBoolean` little-endian handling, `-0`/NaN/Infinity preservation, and the
  same `ToIndex`/value/detached/range validation ordering as the integer
  DataView methods. The focused DataView float method cluster now runs at
  **62 pass / 0 fail / 28 skip**.
- **DataView BigInt element accessors**: implemented
  `DataView.prototype.getBigInt64`, `getBigUint64`, `setBigInt64`, and
  `setBigUint64` with signed/unsigned 64-bit BigInt reads, big-endian
  defaults, `ToBoolean` little-endian handling, `ToBigInt` setter conversion,
  modulo-`2^64` byte writes, and the same receiver, `ToIndex`, detached-buffer,
  and byte-range validation ordering as the numeric DataView methods. The
  official runner still skips this focused cluster while `ArrayBuffer` and
  `DataView` remain marked unsupported; with only those feature skips lifted
  for diagnosis, the BigInt DataView cluster runs at **40 pass / 3 fail / 26
  skip**, with remaining failures requiring immutable ArrayBuffer and
  additional typed-array receiver support. The shared `BigInt()` constructor
  conversion path now also handles primitive-producing objects and reports
  `TypeError` for missing/nullish input.
- **BigInt fixed-width statics**: implemented `BigInt.asIntN` and
  `BigInt.asUintN` with `ToIndex(bits)` before `ToBigInt(value)`, signed and
  unsigned modulo-`2^bits` wrapping, correct `name`/`length` descriptors, and
  non-constructable native functions. The focused BigInt fixed-width static
  cluster now runs at **14 pass / 0 fail / 14 skip**, and the broader
  `built-ins/BigInt` smoke run improves to **49 pass / 25 fail / 29 skip**.
- **BigInt prototype conversion methods**: implemented
  `BigInt.prototype.valueOf` and radix-aware
  `BigInt.prototype.toString(radix)` with `thisBigIntValue` receiver checks,
  `ToNumber`/`ToIntegerOrInfinity` radix validation, own
  `BigInt.prototype.constructor`, the non-writable `BigInt` constructor
  `prototype` property, primitive-wrapper `Object(value)` prototype wiring,
  and ordinary `ToPrimitive` lookup for boxed primitives. The focused BigInt
  prototype/valueOf/toString cluster now runs at **16 pass / 0 fail / 5 skip**,
  and the broader `built-ins/BigInt` smoke run is now **74 pass / 0 fail / 29
  skip**.
- **`Object.hasOwn`**: added the ES2022 static own-property predicate with
  `ToObject` before `ToPropertyKey`, symbol-key support, primitive string
  wrapper `length`/index handling, correct `name`/`length` descriptors, and no
  constructable `prototype` property. The focused `Object.hasOwn` test262
  cluster now runs at **56 pass / 0 fail / 6 skip**.
- **`Object.getOwnPropertyDescriptor` conformance**: aligned
  `Object.getOwnPropertyDescriptor` with `ToObject` before `ToPropertyKey`,
  symbol property keys, string exotic `length`/index descriptors, and
  `FromPropertyDescriptor` result-object attributes. Built-in constructor
  `length`/`name`/`prototype` descriptors and a small set of missing
  descriptor-visible built-in prototype members were also tightened. The
  focused `Object.getOwnPropertyDescriptor` test262 cluster now runs at **308
  pass / 0 fail / 2 skip**.
- **Object own-key enumeration**: `Object.keys`,
  `Object.getOwnPropertyNames`, `Object.getOwnPropertySymbols`, and
  `Object.getOwnPropertyDescriptors` now share array-index-first own-key
  ordering, apply `ToObject` with nullish `TypeError` checks, include
  non-enumerable string keys where required, preserve Symbol keys, and
  synthesize primitive string index/`length` keys for descriptor collection.
  `Object.getOwnPropertySymbols` is now exposed on `Object`, and the focused
  `Object.getOwnPropertyDescriptors` plus `Object.getOwnPropertySymbols`
  clusters now run at **13 pass / 0 fail / 17 skip**. The broader focused
  own-key smoke run is **97 pass / 6 fail / 31 skip**, with remaining failures
  tied to receiver-brand handling and sparse-array hole representation work.
- **`Object.prototype.toString` receiver brands**: removed an unsafe native
  `toString` dispatch workaround and stopped installing Error-prototype
  `name`/`message`/`toString` properties on `Object.prototype`. The builtin
  now distinguishes `null` from `undefined`, reports BigInt primitives,
  boxed primitive wrappers, functions, arrays, arguments objects, Date
  instances, and Error instances with the expected brands, and keeps
  `Error.prototype.toString` separate. The focused
  `built-ins/Object/prototype/toString` cluster now has **0 failures**, and
  the combined Object toString/own-key smoke run is **105 pass / 3 fail / 37
  skip**, with the remaining failures isolated to sparse array holes and dense
  array-index deletion.
- **ArrayBuffer and DataView subclass internals**: added minimal
  `ArrayBuffer` and `DataView` exotic heap objects, constructor/prototype
  bootstrap, `ArrayBuffer.prototype.slice`, and DataView `buffer`/
  `byteOffset`/`byteLength` accessors. Subclass construction now initializes
  the required internal slots and `ArrayBuffer.prototype.slice` returns the
  default subclass constructor result while clamping inverted slice ranges and
  rejecting oversized backing-store lengths, closing the ArrayBuffer/DataView
  subclass checks and raising the supported subset to **4177 pass / 3 fail / 0
  timeout**. Promise GC tracing now also marks downstream derived promises held
  in pending handlers, fixing a stress failure exposed by the additional
  bootstrap allocations.
- **Uint8Array subclass exotic construction**: `Uint8Array` now exposes a
  constructor `prototype`, subclass construction allocates typed-array exotic
  objects with `new.target.prototype`, and integer-index writes update the
  backing buffer with Uint8 wrapping semantics. This closes the remaining
  generic builtin subclassing check, raising the supported subset to **4173
  pass / 7 fail / 0 timeout**.
- **Promise subclass executor validation**: `Promise` construction now rejects
  non-callable executors before allocating the promise object and uses
  `new.target.prototype` when creating subclass promise instances. This closes
  the Promise subclass regular-construction check, raising the supported
  subset to **4172 pass / 8 fail / 0 timeout**.
- **Date subclass component semantics**: `Date` construction with multiple
  date/time components now stores a clipped time value instead of treating the
  year as a raw timestamp, and Date component getters now derive calendar
  fields from the stored time value. This closes the remaining Date subclass
  regular-construction check, raising the supported subset to **4171 pass / 9
  fail / 0 timeout**.
- **Private accessors and non-extensible private slots**: class private
  accessors now parse and install as private accessor slots, static private
  fields/methods/accessors initialize on the constructor object, function
  objects now track extensibility for `Object.preventExtensions`, and adding a
  new private slot to a non-extensible object now throws `TypeError`. This
  closes the private-field non-extensible class checks, raising the supported
  subset to **4170 pass / 10 fail / 0 timeout**.
- **For-of IteratorClose on abrupt completion**: `for...of` now closes
  unfinished iterators when loop bodies complete abruptly via `return`,
  `break`, or throw, while preserving same-loop `continue` without closing the
  iterator. Iterator `return()` errors now override the pending for-of
  completion where required, while destructuring assignment keeps preserving
  the original throw. This closes the derived-constructor return-override
  for-of checks, raising the supported subset to **4168 pass / 12 fail / 0
  timeout**.
- **GeneratorFunction constructor and prototype chain**: generator functions now
  inherit from `%GeneratorFunction.prototype%`, whose `constructor` exposes the
  non-global `GeneratorFunction` constructor. Dynamic generator functions parse
  and compile as `function*`, their own `prototype` objects inherit from the
  generator prototype without an own `constructor` reference, and generator
  calls now use the callee's `prototype` for created generator objects. This
  closes the remaining GeneratorFunction subclass `prototype` and regular
  subclassing checks, raising the supported subset to **4166 pass / 14 fail / 0
  timeout**.
- **Array, RegExp, and String subclass exotic construction**: native
  constructors now share `new.target.prototype` fallback handling for
  OrdinaryCreateFromConstructor-style allocation. Array subclass construction
  now returns Array exotic objects with the subclass prototype, RegExp
  subclass construction now uses the subclass prototype and preserves
  non-configurable `lastIndex` descriptors across `test`/`exec`, and boxed
  String objects now materialize their own non-writable, non-enumerable,
  non-configurable `length` descriptor. This closes Array/RegExp subclass
  checks plus String `length`, raising the supported subset to **4164 pass /
  16 fail / 0 timeout**.
- **Dynamic Function subclass construction**: native constructors now preserve
  the active `new.target` for native constructor bodies, and the dynamic
  `Function` constructor now creates function objects with the
  `new.target.prototype` internal prototype plus own `length`, `name`, and
  `prototype` descriptors. This closes Function subclass `length`/`name` and
  `instanceof` checks, improves GeneratorFunction subclass descriptor checks,
  and raises the supported subset to **4158 pass / 22 fail / 0 timeout**.
- **Class method descriptor validation**: static class methods and accessors now
  route through own-property descriptor validation when defining constructor
  properties, so computed static `prototype` methods/accessors throw instead of
  overwriting the constructor's non-configurable `prototype` property. This
  improves `language/statements/class` to **187 pass / 22 fail** and raises the
  supported subset to **4152 pass / 28 fail / 0 timeout**.
- **Class declaration completion and binding mutability**: class declarations
  now produce an empty statement completion, so direct eval returns
  `undefined` or the previous non-empty completion rather than the constructor.
  The outer class declaration binding is now initialized as a mutable lexical
  binding while the inner class-name binding captured by methods and heritage
  remains immutable. This improves `language/statements/class` to **186 pass /
  23 fail** and raises the supported subset to **4151 pass / 29 fail / 0
  timeout**.
- **Var initializer Reference resolution**: `var x = init` now resolves the
  binding Reference before evaluating `init`, matching the spec's
  `BindingInitialization` order for `VariableDeclaration`. This preserves
  `with` object references when the initializer mutates the same property and
  keeps global `var` bindings synchronized with their global object
  descriptors. This closes `language/statements/variable` at **77 pass / 0
  fail** and raises the supported subset to **4147 pass / 33 fail / 0
  timeout**.
- **Contextual `of` division lexing**: `/` after contextual `of` now remains a
  division operator in expression contexts such as `instance/of/g`, while
  raw `of` delimiters in `for...of` heads still allow a following regex
  literal. This closes `language/expressions/division` at **41 pass / 0
  fail** and raises the supported subset to **4146 pass / 34 fail / 0
  timeout**.
- **Addition primitive coercion**: binary `+` now performs `ToPrimitive`
  before BigInt mixing checks, concatenates when either primitive is a string
  so BigInt-to-string concatenation is allowed, and treats Date objects with
  the default hint as string-hinted ordinary primitives. This closes
  `language/expressions/addition` at **38 pass / 0 fail** and raises the
  supported subset to **4144 pass / 36 fail / 0 timeout**.
- **Operator edge semantics**: BigInt exponentiation now throws `RangeError`
  for negative exponents, BigInt relational comparisons now coerce
  Boolean/nullish numeric operands through `ToNumeric`, `in` now rejects
  primitive right-hand sides before property-key conversion, `instanceof`
  now returns `false` for primitive left-hand sides before reading
  `prototype`, and strict non-generator `yield` is rejected during parsing.
  This closes `language/expressions/exponentiation`, `greater-than`,
  `less-than`, `in`, and `instanceof` at **188 pass / 0 fail** and raises the
  supported subset to **4142 pass / 38 fail / 0 timeout**.
- **Class heritage strictness and strict arguments objects**: class heritage
  expressions now parse under strict mode while preserving script-goal
  `await` class names, and strict function calls now create an unmapped
  `arguments` object whose `callee` accessor throws `TypeError`. This closes
  `language/statements/class/strict-mode` at **2 pass / 0 fail** and raises
  the supported subset to **4135 pass / 45 fail / 0 timeout**.
- **Switch CaseBlock scoping and redeclarations**: switch `var`
  declarations now bind in the enclosing variable environment instead of the
  switch lexical environment, while function declarations in case bodies stay
  scoped to the CaseBlock. Switch redeclaration early errors now treat
  function declarations as lexical names. This closes
  `language/statements/switch` at **69 pass / 0 fail** and raises the
  supported subset to **4133 pass / 47 fail / 0 timeout**.
- **Boxed String methods and Date method surface**: String prototype methods
  now read the wrapped primitive from `new String(...)` objects, so indexed
  operations like `charAt` agree with boxed string index properties. The
  bootstrap also installs `String.prototype.length`, `Date.parse`, `Date.UTC`,
  and the ES5 Date prototype method surface needed for property-access checks.
  This closes `language/expressions/property-accessors` at **15 pass / 0
  fail** and raises the supported subset to **4127 pass / 53 fail / 0
  timeout**.
- **Tagged-template call context and conditional `in` grammar**: tagged
  templates used as member expressions now preserve their receiver as `this`,
  ``new tag`...` `` constructs the tag result rather than the tag function
  itself, and constructor arguments after a tagged template are applied to that
  result. Conditional-expression true branches now allow `in` even inside
  no-`in` contexts such as `for` heads. This reduces
  `language/expressions/tagged-template` to its remaining cross-realm
  `$262.createRealm()` failure, closes
  `language/expressions/conditional/in-branch-1.js`, and raises the supported
  subset to **4124 pass / 56 fail / 0 timeout**.
- **Call-expression environment and argument ordering**: explicit named
  function-expression bindings now live in the function closure environment
  rather than the call body's variable environment, so body `var` declarations
  with the same name create the required separate binding. Sloppy direct eval
  now accepts `static` as a contextual `var` binding name, and member calls now
  perform the property lookup before evaluating arguments while leaving the
  callability check after argument evaluation. This improves
  `language/expressions/call` to **48 pass / 1 fail** and raises the
  supported subset to **4121 pass / 59 fail / 0 timeout**.
- **Named function-expression bindings**: named function expressions now create
  an immutable inner name binding. Sloppy assignments to that binding are
  ignored, while strict assignments throw `TypeError`; direct eval and lexical
  arrows inside the function body resolve to the same protected binding. This
  closes `language/expressions/function` at **53 pass / 0 fail** and raises
  the supported subset to **4118 pass / 62 fail / 0 timeout**.
- **Object.prototype.propertyIsEnumerable**: implemented the missing
  prototype method, including Symbol keys, array index/length behavior, string
  index enumerability, and nullish receiver errors. This unblocks test262
  `propertyHelper.js` descriptor checks for object literal accessor and method
  definitions.
- **Live array-like `for...of` iteration**: array and arguments-object
  iterators now read `length` and indexed properties lazily on each pull
  instead of snapshotting values at iterator creation. Array growth,
  contraction, accessor-index exceptions, strict arguments mutation, and
  sloppy mapped-arguments aliasing now match the covered test262 semantics.
  `Object.defineProperty(array, index, descriptor)` also advances array
  length for indexed descriptors, and deleting mapped arguments elements
  breaks the parameter alias as required.
- **`for...of` head parsing and early errors**: the parser now requires the
  `of` delimiter to be the raw keyword, allows `async` as a contextual
  identifier on assignment left-hand sides, rejects `const let` heads, and
  validates array/object assignment patterns before accepting them as
  `for...of` targets.
- **`for...of` lexical head environments**: `let`/`const` loop heads now use
  a temporary TDZ environment while evaluating the right-hand iterable and a
  fresh per-iteration lexical environment before binding each iterator value.
  Destructuring defaults and statement-body closures now capture the same
  initialized iteration binding, and `typeof` on TDZ bindings throws
  `ReferenceError` while unbound names still report `"undefined"`.
- **`for...in` lexical head environments**: `let`/`const` loop heads now mirror
  `for...of` by evaluating the right-hand object under a temporary TDZ
  environment and binding each enumerated property key inside a fresh
  per-iteration lexical environment. Destructuring defaults and loop-body
  closures now capture the initialized iteration binding, and array/object
  assignment-pattern left-hand sides are validated before accepting them as
  `for...in` targets.
- **Compile-only loop scope unwinding**: `break`/`continue` unwinding now pops
  only scopes that have a runtime environment record, so `for`/`for...in`/
  `for...of` compiler-only loop scopes no longer over-pop direct-eval
  environments when a labelled `continue` exits an inner loop. This preserves
  `UpdateEmpty` completion values for `for...in` and `for...of` labelled
  continue paths and reduces the `for...in` subset to **69 pass / 5 fail** and
  the `for...of` subset to **81 pass / 1 fail**.
- **`for...in` enumeration and descriptor preservation**: `for...in`
  enumeration now treats non-enumerable own string properties as visited so
  they shadow prototype properties, and rechecks a key's current
  enumerability before yielding so deleted not-yet-visited properties are
  skipped. `Object.defineProperty` now preserves existing descriptor fields
  that are absent from a redefinition descriptor and rejects invalid
  non-configurable/non-extensible redefinitions, keeping property order and
  enumerability intact for cases like `{ a, b }` followed by redefining `a`.
  The `for...in` subset improves to **72 pass / 2 fail**.
- **Array-index assignment through prototype setters**: writing to a missing
  array index now observes inherited accessor setters before extending the
  array, so member-expression `for...in` heads like `for ([let][1] in obj)`
  route through the same [[Set]] semantics as ordinary element assignment.
  The `for...in` subset improves to **73 pass / 1 fail**.
- **Direct eval `var` leakage from lexical `for...in` heads**: sloppy direct
  eval now copies `var`/function declarations back to the caller's variable
  environment rather than the temporary TDZ lexical environment used while
  evaluating a lexical `for...in` right-hand side. This preserves closures
  created before, during, and after the loop head expression and brings the
  `for...in` subset to **74 pass / 0 fail**.
- **Ordinary object `[[Set]]` own-property precedence**: assignment now handles
  an object's own accessor/data descriptor before consulting inherited
  setters or non-writable data properties. This lets ordinary function
  `.prototype` and `prototype.constructor` own data properties remain writable
  even when `Function.prototype`/`Object.prototype` define same-named accessors,
  reducing the `function` statement subset to **8 failures**.
- **Catch parameter early errors**: `try` statements now reject duplicate
  catch-parameter bound names and direct catch-block lexical/function
  redeclarations of the catch parameter while still allowing `var` and nested
  block shadowing. This fixes the `early-catch-duplicates`,
  `early-catch-lex`, and `early-catch-function` test262 cases and reduces the
  `try` statement subset to **9 failures**.
- **Native runtime errors through `finally`**: catchable native VM errors such
  as `ReferenceError` and `TypeError` now divert through active `finally`
  guards before reaching an outer `catch`, matching the same path as explicit
  JS `throw`. Re-thrown Error objects also preserve their specific error kind
  in host error reporting. This fixes test262 `S12.14_A3` and `S12.14_A13_T2`
  and reduces the `try` statement subset to **7 failures**.
- **Native Error constructor call branding**: plain calls to native Error
  subclasses, such as `EvalError(1)` and `TypeError(1)`, now allocate through
  the active callee's `prototype` instead of always using `Error.prototype`.
  Native constructor dispatch also clears consumed `new.target` state so a
  native `new Error(...)` call cannot leak construct state into the next call.
  This fixes test262 `S12.14_A19_T1` and `S12.14_A19_T2` and reduces the
  `try` statement subset to **5 failures**.
- **Declarative binding deletion semantics**: `delete` now returns `false`
  for declarative environment bindings, including lexical bindings and
  catch parameters, instead of deleting `let`/`const`-classified bindings.
  This preserves catch parameter values through `delete e`, fixes test262
  `S12.14_A4`, and reduces the `try` statement subset to **4 failures**.
- **`try`/`finally` completion replacement semantics**: abrupt completions
  entering a `finally` block now keep the original completion isolated from
  the `finally` body's own completion. Normal expression values inside
  `finally` no longer overwrite a pending empty `break`, while a `throw`
  inside `finally` correctly replaces a pending `return`, `break`, or outer
  `throw`. Non-throw abrupt completions also disable skipped catch handlers
  before entering `finally`, so a `finally`-body `throw` cannot be caught by
  the catch clause of the same already-completed try statement. This brings
  `language/statements/try` to **98 pass / 0 fail**.
- **Function.prototype restricted properties**: `%Function.prototype%` now has
  inherited `caller` and `arguments` accessor properties whose getter and
  setter throw `TypeError`. Bound functions created by
  `Function.prototype.bind` therefore do not gain own `caller`/`arguments`
  properties but still inherit the required restricted accessors, reducing the
  `function` statement subset to **7 failures**.
- **Function body `"use strict"` with non-simple parameters**: function
  declarations, function expressions, object/class methods, and arrow block
  bodies now reject a directive prologue `"use strict"` when the formal
  parameter list contains defaults, rest, or destructuring. Directive
  detection now runs before synthesized destructuring-parameter prelude
  statements are prepended, reducing the `function` statement subset to
  **6 failures**.
- **Object/class method formal-parameter early errors**: concise object
  methods, async object methods, and class/private methods now reject duplicate
  formal parameter bound names, including duplicates introduced by
  destructuring patterns. Object async method parsing also enforces the
  required no-LineTerminator restriction between `async` and the property
  name, reducing the object method-definition subset to **3 failures**.
- **`yield` contextual identifier parsing**: sloppy non-generator contexts now
  parse `yield` as an identifier in bindings, expressions, destructuring
  patterns, object method parameters/defaults, and computed property names,
  while generator parameter/body contexts continue to parse `yield` as the
  generator keyword. This brings the object method-definition subset to
  **40 pass / 0 fail**.
- **`let` declaration ASI/lookahead parsing**: `let` followed by a binding
  name now remains a LexicalDeclaration across line terminators in
  StatementListItem positions, so cases like `let\nlet` and `let\nawait 0`
  fail during parse instead of executing. Escaped `l\u0065t` stays an
  identifier, and single-statement bodies still use ExpressionStatement
  lookahead rules, reducing `language/statements/let/syntax` to
  **26 pass / 4 fail**.
- **Parenthesized assignment-pattern targets**: parenthesized object/array
  literals are no longer accepted as assignment targets for an outer
  assignment. This preserves valid inner destructuring such as `({} = obj)`
  while rejecting `({}) = 1` and arrow-expression bodies like
  `() => ({}) = 1`, reducing `language/expressions/assignmenttargettype` to
  **313 pass / 3 fail**.
- **`await` contextual identifier parsing**: sloppy non-async script and
  function contexts now parse `await` as a contextual identifier in
  declarations, formal parameters, assignment/reference positions,
  destructuring patterns, object method parameters, computed property names,
  and nested non-async functions inside async bodies. Async function, async
  method, and async arrow parameter/body contexts still parse `await` as the
  async keyword. This brings `language/expressions/await` to
  **7 pass / 0 fail** and reduces `language/expressions/assignmenttargettype`
  to **314 pass / 2 fail**.
- **`import.meta` assignment target early errors**: direct and parenthesized
  assignments to `import.meta` are now rejected during parsing instead of
  reaching runtime. This closes the remaining
  `language/expressions/assignmenttargettype` failures, bringing that
  directory to **316 pass / 0 fail**.
- **Object destructuring assignment target order**: member-expression targets
  inside object assignment patterns are now evaluated before reading the
  source property value, including computed source keys. This fixes the
  test262 target-reference evaluation-order case and reduces
  `language/expressions/assignment/destructuring` to **1 pass / 5 fail**.
- **Array destructuring assignment iterator semantics**: array assignment
  patterns now use the iterator protocol, evaluate member targets before
  stepping the iterator, apply assignment defaults, and close unfinished
  iterators when default evaluation or target assignment throws while
  preserving the original throw completion. Lazy iterator `next()` results now
  read `done` before `value`, so done iterators do not invoke `value` getters.
  This reduces `language/expressions/assignment/destructuring` to
  **5 pass / 1 fail**.
- **Duplicate `__proto__` object assignment properties**: object assignment
  patterns now allow duplicate static `__proto__` colon properties while
  keeping the Annex B duplicate-`__proto__` early error for object literals.
  Destructuring assignment expressions also preserve the RHS value as their
  expression result, so nested assignments such as `result = ({ x } = obj)`
  store `obj`. This brings `language/expressions/assignment/destructuring` to
  **6 pass / 0 fail**.
- **Computed property names in `for` heads**: computed property names and
  computed member keys now parse their bracketed expressions with `in`
  allowed, even when the surrounding expression is being parsed under
  `for (... in ...)` lookahead. This fixes object accessor names such as
  `{ get ["x" in obj]() {} }` inside `for` initializers and reduces
  `language/expressions/object` to **271 pass / 14 fail**.
- **Object literal strict early errors**: strict object literal shorthand
  properties now reject reserved IdentifierReferences such as `let` and
  `yield`, and object accessors/methods apply body `"use strict"` directives
  to formal-parameter `eval`/`arguments` checks. This reduces
  `language/expressions/object` to **275 pass / 10 fail** and raises the
  supported subset to **3953 pass / 225 fail / 2 timeout**.
- **String literal line continuations**: string literals now treat a backslash
  followed by a LineTerminatorSequence as a LineContinuation that contributes
  no cooked characters. This fixes computed object accessor names and reduces
  `language/expressions/object` to **276 pass / 9 fail**, raising the
  supported subset to **3954 pass / 224 fail / 2 timeout**.
- **Direct eval lexical declaration conflicts**: sloppy direct eval now rejects
  `var`/function declarations that would hoist over an existing caller
  `let`/`const` binding. This fixes method/accessor body lexical environment
  conflict cases, reduces `language/expressions/object` to
  **279 pass / 6 fail**, and raises the supported subset to
  **3960 pass / 218 fail / 2 timeout**.
- **Object method parameter/body environments**: functions with parameter
  expressions now evaluate defaults and synthetic destructuring preludes before
  pushing a separate body variable environment. Parameter closures no longer
  see later body `var` declarations, while direct eval `var`s created during
  parameter evaluation remain visible to both parameter and body closures.
  Parser parameter scratch state is also scoped per nested function/method, so
  nested function expressions in defaults no longer steal outer defaults. This
  brings `language/expressions/object` to **285 pass / 0 fail** and raises the
  supported subset to **3979 pass / 199 fail / 2 timeout**.
- **Arrow formal-parameter early errors**: arrow functions now reject
  duplicate bound names introduced by destructuring parameters and reject a
  line terminator before `=>` for both parenthesized and parenless parameter
  forms. Async-arrow lookahead also preserves the required no-LineTerminator
  restrictions around `async` and `=>`. This brings
  `language/expressions/arrow-function/syntax/early-errors` to
  **25 pass / 0 fail** and raises the supported subset to
  **3990 pass / 188 fail / 2 timeout**.
- **Sloppy arrow contextual parameters**: non-strict arrow functions now allow
  `eval`, `arguments`, and `yield` as formal parameter names where the grammar
  permits them, while strict enclosing code or a block-body `"use strict"`
  directive still rejects `eval`/`arguments`. This brings
  `language/expressions/arrow-function/syntax` to **45 pass / 0 fail**,
  raises `language/expressions/arrow-function` to **88 pass / 2 fail**, and
  raises the supported subset to **3996 pass / 182 fail / 2 timeout**.
- **Arrow lexical `arguments`**: arrow function calls no longer create their
  own `arguments` object binding, so `arguments` references inside an arrow
  resolve through the captured lexical environment unless shadowed by an
  explicit parameter. This raises `language/expressions/arrow-function` to
  **89 pass / 1 fail** and the supported subset to
  **3997 pass / 181 fail / 2 timeout**.
- **Lexical arrow `super()` binding order**: `super()` calls now perform the
  superclass constructor call before rebinding the derived constructor's
  lexical `this` environment and forward the active constructor's
  `new.target`. A repeated `super()` call, including one captured in an arrow
  and invoked after the constructor returns, now throws `ReferenceError` only
  after the superclass constructor has run. This closes
  `language/expressions/arrow-function` at **90 pass / 0 fail** and raises the
  supported subset to **3999 pass / 179 fail / 2 timeout**.
- **`super()` constructor mixed spread arguments**: `super(...)` now lowers
  mixed spread and non-spread arguments through the same iterator-backed
  argument-array path used by ordinary calls and `new`. This preserves
  left-to-right evaluation, handles empty spreads, and reports unresolvable
  spread operands as `ReferenceError`. This raises
  `language/expressions/super` to **30 pass / 6 fail** and the supported
  subset to **4003 pass / 175 fail / 2 timeout**.
- **Lexical arrow `super` property parsing**: block-bodied arrow functions
  now preserve the enclosing method's `super` parse context instead of
  resetting it like ordinary function bodies. This allows `super.x` and
  `super["x"]` inside arrows nested in object methods while still rejecting
  `super` in arrows without an enclosing super binding. This raises
  `language/expressions/super` to **32 pass / 4 fail** and the supported
  subset to **4005 pass / 173 fail / 2 timeout**.
- **Direct eval lexical `super` parsing**: direct eval now inherits the
  caller's `super` parse context when the caller environment has a `#super`
  binding. This allows `eval("super.x")` and computed `super` property access
  inside object methods while preserving SyntaxError for eval code without an
  enclosing super binding. This raises `language/expressions/super` to
  **34 pass / 2 fail** and the supported subset to
  **4007 pass / 171 fail / 2 timeout**.
- **Computed `super[...]` putvalue evaluation order**: compound assignment and
  update expressions now evaluate a `super` property target by checking the
  derived constructor `this` binding before evaluating a computed property
  expression, then reuse the same receiver/base/key reference for the get and
  set. This closes `language/expressions/super` at **36 pass / 0 fail** and
  raises the supported subset to **4009 pass / 169 fail / 2 timeout**.
- **Nullish/logical chain early errors**: unparenthesized `??` mixed directly
  with `&&` or `||` now throws a parse-time `SyntaxError`, while
  parenthesized combinations still parse and evaluate. This closes
  `language/expressions/coalesce` at **22 pass / 0 fail** and raises the
  supported subset to **4013 pass / 165 fail / 2 timeout**.
- **BigInt `ToNumeric` operator semantics**: unary plus and unsigned right
  shift now reject BigInt operands with `TypeError`, while BigInt-aware
  arithmetic, bitwise, and signed shift operations preserve BigInt results
  after `ToNumeric`, including boxed BigInts. `ToNumber` no longer silently
  converts BigInt except through the `Number()` constructor, and string
  numeric conversion no longer accepts incorrectly-cased Infinity spellings.
  This closes the BigInt failures in `bitwise-and`, `bitwise-or`,
  `bitwise-xor`, and `unsigned-right-shift`, reduces `unary-plus` to
  **0 failures**, and raises the supported subset to
  **4034 pass / 144 fail / 2 timeout**.
- **Native Error subclass construction**: `Error.prototype` now inherits
  `Object.prototype` during bootstrap, NativeError subclass instances no
  longer receive own `message` properties when the message argument is
  omitted, and `name` is inherited through the prototype chain so
  `class Err extends EvalError {}` instances report `EvalError`. This closes
  `language/statements/class/subclass/builtin-objects/NativeError` at
  **18 pass / 0 fail** and raises the supported subset to
  **4047 pass / 131 fail / 2 timeout**.
- **Class element grammar and named class expression scope**: class bodies now
  accept empty `;` elements, computed accessor names, and generator methods,
  and named class expressions create an inner immutable class-name binding
  instead of leaking the name to the outer scope. Class names now reject
  `yield` even in sloppy surrounding scripts. This closes
  `language/expressions/class` at **48 pass / 0 fail**, improves
  `language/statements/class/syntax` to **9 pass / 4 fail**, and raises the
  supported subset to **4061 pass / 117 fail / 2 timeout**.
- **Class declaration early errors**: script and block statement lists now
  reject duplicate lexical class declarations and lexical/`var` name clashes
  during parsing, and escaped `static` is no longer accepted as the class
  `static` modifier. This improves `language/statements/class/syntax` to
  **12 pass / 1 fail** and raises the supported subset to
  **4064 pass / 114 fail / 2 timeout**.
- **Class `super` property HomeObject setup**: class evaluation now gives
  constructor and instance methods a per-class `#super` binding based on
  `Class.prototype`, while static methods, static accessors, and static blocks
  bind `Class` in their own closure environment. SuperProperty evaluation
  reads the HomeObject prototype dynamically. This allows base-class
  constructor and method `super.prop`, fixes static `super.x` lookup on
  subclasses, closes `language/statements/class/super` and
  `language/statements/class/syntax` at **21 pass / 0 fail**, and raises the
  supported subset to **4069 pass / 109 fail / 2 timeout**.
- **Class definition/name-binding semantics**: class declarations now hoist as
  immutable lexical bindings, anonymous class assignment infers constructor
  display names, class bodies parse nested functions in strict context, and
  method/accessor display names no longer create body bindings that shadow
  outer variables. Class `extends` now performs the superclass `prototype`
  getter exactly once and reuses that value for prototype wiring, while
  derived constructors return the `this` object bound by `super()` when no
  object is explicitly returned. This closes
  `language/statements/class/definition` and
  `language/statements/class/name-binding` at **41 pass / 0 fail** and raises
  the supported subset to **4080 pass / 98 fail / 2 timeout**.
- **Dynamic class `super` references**: `super` property reads, calls, simple
  assignments, updates, and compound assignments now derive the super base
  from the method HomeObject at evaluation time instead of using a stale
  class-definition-time prototype value. This follows later
  `Object.setPrototypeOf` changes, and simple `super.x = rhs` /
  `super[expr] = rhs` evaluates `rhs` before throwing `TypeError` when the
  dynamic super base is `null`. This closes `language/expressions/assignment`
  at **110 pass / 0 fail** and raises the supported subset to
  **4082 pass / 96 fail / 2 timeout**.
- **Null-extending classes and bound subclass construction**:
  `class C extends null {}` now wires `C.prototype.[[Prototype]]` to `null`
  while making the constructor inherit from `%Function.prototype%`, and
  `super()` in such a class throws `TypeError` because `%Function.prototype%`
  is not a constructor. Constructing a bound class now ignores the bound
  `this` value and delegates to the target constructor with prepended bound
  arguments. This raises `language/statements/class/subclass` to
  **75 pass / 19 fail** and the supported subset to
  **4089 pass / 89 fail / 2 timeout**.
- **C-style `for` lexical head environments**: `for (let/const ...; ...; ...)`
  now creates a runtime loop-head lexical environment, evaluates the first
  condition/body/update in a per-iteration child environment, and reclones a
  sibling environment before each update so body closures keep pre-update
  bindings while the update prepares the next iteration. The parser also
  applies the head/body `var` redeclaration early error to ordinary `for`
  loops and accepts `async of => {}` as a normal async-arrow initializer. This
  closes `language/statements/for` at **93 pass / 0 fail** and raises the
  supported subset to **4103 pass / 77 fail / 0 timeout**.
- **Label identifiers and strict labelled functions**: labelled statements now
  accept contextual `await` labels in non-module code and contextual `yield`
  labels in sloppy non-generator code, including escaped spellings, while
  strict labelled function declarations are rejected during parsing. This
  closes `language/statements/labeled` at **17 pass / 0 fail** and raises the
  supported subset to **4108 pass / 72 fail / 0 timeout**.
- **Function statement-control parser boundaries and raw meta-property
  tokens**: nested function bodies now reset loop/switch/label parsing context
  so inner `break`/`continue` cannot target an outer function's labels.
  `async function` declarations and expressions now require no line terminator
  between `async` and `function`, `new.target` requires raw `new` and `target`
  tokens, and `debugger` is parsed as a statement-only keyword. This closes
  the remaining supported-subset failures in `language/statements/break`,
  `language/statements/continue`, `language/statements/debugger`,
  `language/statements/async-function`, and `language/expressions/new.target`,
  raising the supported subset to **4114 pass / 66 fail / 0 timeout**.
- **Call frame operand-stack isolation**: each `CallFrame` now records its
  stack base, and `Pop`/`Return`/`Halt` cannot consume operands below the
  current frame. This prevents nested calls with loop-body cleanup (for
  example `out.push(f())` where `f` contains `for...in`/`for...of`) from
  corrupting the caller's method-call receiver stack.
- **BigInt exact comparison semantics**: BigInt equality and relational
  comparisons now avoid lossy `f64` conversion. BigInt-vs-Number comparison
  handles integer, fractional, `NaN`, and infinity cases separately, while
  BigInt-vs-String now implements `StringToBigInt` for empty strings and
  `0x`/`0o`/`0b` prefixes. This fixes the BigInt equality and comparison
  test262 clusters, including large literals beyond IEEE-754 precision.
- **Arbitrary-precision BigInt prefixed literals**: hex/octal/binary BigInt
  literals are parsed with `num_bigint` instead of overflowing through `i64`,
  so literals such as `0x10000000000000000n` preserve their exact value.
- **UTF-16 string comparison and iteration**: string relational comparison now
  uses UTF-16 code-unit order, and string iteration (`for...of`, destructuring,
  spread) yields Unicode code points by combining valid surrogate pairs. Lone
  surrogate escapes are accepted and preserved internally with private
  sentinels because Rust `String` cannot store surrogate code points directly.

- **for-of/for-in member expression LHS**: `for (x.y of [23])` now correctly
  evaluates the member expression as the assignment target using Swap-based
  stack reordering. Previously threw "Cannot set property of primitive".
- **for-in/for-of duplicate var names**: `for (var [x, x] in obj)` is no
  longer a SyntaxError. Duplicate names are only an error for `let`/`const`
  declarations, not `var`.
- **`delete x` (implicit global)**: `delete x` where `x` was created by an
  implicit global assignment (`x = 1` without `var`) now returns `true` and
  removes the binding, matching spec non-strict-mode behavior.

- **`with`-statement scope semantics**: `var x = expr` inside a `with`-block
  now correctly resolves the assignment target through the environment chain.
  When the `with`-object has a matching property, the assignment targets the
  with-object (not the function-scope var binding). When a `var` binding
  already exists in a closer scope (e.g. inside a function defined within
  the `with`-block), the var binding takes precedence over the with-object
  property. This fixes ~45 `with`-statement test262 failures.
- **Identifier resolution order**: `LoadEnvName`, `get_value`, and
  `put_value` now walk the environment chain in spec order — at each
  environment record, var/let/const bindings are checked before
  with-object properties. This ensures a var binding in a child scope
  shadows a with-object property on a parent scope, while a with-object
  property shadows an outer var binding.
- **Var hoisting vs initializer separation**: a new `HoistVar` opcode
  creates the hoisted `var` binding as `undefined` in the function-scope
  root without touching `with`-object properties. The `DeclareVar` opcode
  then sets the initializer value via `set_checked`, which respects the
  environment chain precedence. This prevents var hoisting from
  clobbering existing with-object properties.
- **Try/catch environment unwinding**: when a `throw` inside a `try` body
  diverts to a `catch` handler, the frame environment is now restored to
  the try-entry point, unwinding any scopes or `with`-environments opened
  inside the try body but not popped because the throw bypassed their
  `Pop*` opcodes. `catch_stack` entries now store the saved environment
  alongside the handler IP.
- **`PutValue` env-chain traversal**: `put_value` for Reference-based
  assignments now walks the environment chain from the reference's base
  env, matching spec `SetMutableBinding` semantics. A deleted with-object
  property is recreated on the closest with-object (non-strict) rather
  than falling through to an outer with-object.

- **Template-literal raw/cooked escapes**: template segments now correctly
  handle line continuations (cooked empty, raw preserves `\\` + line
  terminator), legacy octal/hex/unicode invalid escapes (SyntaxError for
  untagged templates, `undefined` cooked for tagged templates), and raw
  values for invalid escapes per spec. Nested template literals inside
  interpolations are lexed correctly via a template-context stack.
- **Switch completion values with `continue`**: when `continue` exits a
  `switch` (e.g. inside `do-while`), the current switch completion value is
  now propagated to the enclosing completion slot. This makes
  `do { switch { case: { 6; continue; } } } while (false)` evaluate to `6`
  per spec.
- **Reference type for compound assignments**: identifier compound
  assignments (`x += y`) now use spec-conforming LoadRef → GetValue →
  operate → PutValue, preserving the original binding even if deleted
  between get and put (the `with` + getter-delete pattern). Unresolved
  references always throw ReferenceError per spec.
- **Resolved Reference bases across compound-assignment RHS evaluation**:
  `LoadRef` now records the resolved declarative environment or object
  environment base before evaluating the right-hand side. Direct eval can no
  longer introduce an inner `var` binding that steals the final `PutValue`;
  the compound-assignment test262 directory now passes 406/406 locally.
  Strict object-environment references also throw ReferenceError if the
  property disappears between `GetValue` and `PutValue`.
- **Reference GC rooting and global object writes**: GC root collection now
  follows object bases stored inside `Value::Reference`, so compound
  assignment RHS evaluation cannot collect a captured object/environment
  reference. Object-environment `PutValue` also bypasses the legacy
  global-env write shortcut, preserving sloppy global object property writes
  when a getter deletes the property before the final put.
- **`with` statement completion values and early errors**: `with` now resets
  its own completion value before evaluating the body, so empty normal and
  empty abrupt completions are updated to `undefined` while expression bodies
  preserve their value. Direct eval now preserves inherited strictness through
  to the compiled chunk, making `with` inside strict direct eval throw
  SyntaxError. Expression statements beginning with `let [` are rejected even
  across a line terminator, matching the grammar lookahead restriction.
- **Compiled function table index rebasing**: code compiled after previous
  functions already exist in the VM now rebases `MakeClosure`/`MakeClass`
  indices before appending function definitions. This prevents direct eval and
  repeated `Vm::run` calls from accidentally constructing an older function
  body when the new source creates function expressions.
- **Reference type for simple identifier assignment**: `x = rhs` now creates
  the identifier Reference before evaluating `rhs`, then stores through
  `PutValue`. This preserves the originally resolved with-object property even
  if `rhs` deletes it, and prevents direct eval in `rhs` from introducing a
  nearer `var x` that steals the final store. Unresolvable identifier
  References are now represented explicitly, so sloppy `with` assignments to
  names that were absent before `rhs` still create implicit globals rather
  than new with-object properties.
- **Inherited read-only data properties in `[[Set]]`**: ordinary assignment
  now rejects writes when the prototype chain contains a non-writable data
  property. Sloppy assignment fails silently and strict assignment throws
  `TypeError`, while writable inherited data properties still allow creating
  an own property on the receiver. Object literal data properties and object
  spread now use own data-property definition instead of ordinary `[[Set]]`,
  so inherited read-only `Object.prototype` properties do not block object
  initialization; non-computed `__proto__` colon properties still update the
  literal object's prototype.
- **Keyword IdentifierNames after member `.`**: dot property access now accepts
  every keyword token covered by the shared `as_keyword_str()` helper, so
  escaped keyword property names such as `obj.st\u0061tic = 42` parse as
  `obj.static` instead of throwing a SyntaxError.
- **Assignment function-name inference**: anonymous function assignment now
  performs SetFunctionName only for a bare identifier left-hand side. Member
  targets such as `obj.attr = function() {}` and parenthesized identifier
  targets such as `(fn) = function() {}` no longer infer a name, while
  `cover = (function() {})` still infers `cover` from the bare identifier
  assignment target.
- **Native function own descriptors**: native functions now install own
  non-writable, non-enumerable, configurable `length` and `name` data
  properties when allocated. Strict writes such as `Function.length = 42`
  now throw `TypeError`, and `Object.getOwnPropertyDescriptor(Function,
  "length")` reports the expected descriptor. The assignment test262 subset
  improves to **101 pass / 9 fail**.
- **Global object property descriptors**: global bindings are now mirrored
  onto `globalThis` as own properties, with `NaN`/`Infinity`/`undefined`
  installed as non-writable, non-configurable data properties. Sloppy
  implicit globals create configurable enumerable global object properties,
  strict script top-level `this` resolves to `globalThis`, and top-level
  `var` declarations create non-configurable enumerable global object
  properties without treating `var x;` as `x = undefined`. Initializers for
  existing read-only globals such as `var NaN = 42` now respect the global
  object's non-writable descriptor instead of mutating only the environment
  binding. This removes the remaining direct assignment failures, improves
  `language/expressions/delete` to **61 pass / 5 fail**, and raises the
  supported subset to **3799 pass / 379 fail / 2 timeout**.
- **Delete reference semantics**: `delete` now evaluates non-reference
  operands before returning `true`, treats function parameter bindings as
  non-deletable mutable bindings, deletes configurable global object
  properties such as `JSON` even when mirrored by a global env binding, and
  throws `ReferenceError` for `delete super.x` / `delete super[x]` in the
  correct evaluation order. The `language/expressions/delete` subset now
  passes **66/66** run tests, and the supported subset rises to
  **3804 pass / 374 fail / 2 timeout**.
- **Update-expression Reference semantics**: prefix/postfix
  increment/decrement now evaluate the target once, preserve the original
  Reference across `GetValue` and `PutValue`, use `ToNumeric` so BigInt update
  results stay BigInt, and call computed property-key coercion only once.
  The four increment/decrement test262 directories now pass **130/130** run
  tests.
- **Object literal computed property keys**: computed data and accessor
  property names now perform `ToPropertyKey` immediately after evaluating the
  key expression, before evaluating the property value or creating the
  accessor function, while preserving Symbol keys. The
  `language/expressions/object` subset improves to
  **250 pass / 35 fail / 285 ran**, and the supported subset rises to
  **3828 pass / 350 fail / 2 timeout**.
- **Object literal method semantics**: concise methods and accessors are now
  non-constructors, ordinary concise methods no longer get an own
  `prototype` property, and object accessor methods bind `super` through the
  literal home object. `super.x` reads and `super.x = v` writes now use the
  original receiver instead of the prototype object. The
  `language/expressions/object` subset improves to
  **254 pass / 31 fail / 285 ran**, and the supported subset rises to
  **3840 pass / 338 fail / 2 timeout**.
- **Object literal `__proto__` semantics**: duplicate non-computed
  `__proto__:` prototype-mutation properties now throw a parse-time
  `SyntaxError`, while computed `["__proto__"]` and shorthand `{__proto__}`
  remain ordinary data properties. Own data `__proto__` properties now take
  precedence over the legacy prototype getter. The
  `language/expressions/object` subset improves to
  **258 pass / 27 fail / 285 ran**, and the supported subset rises to
  **3850 pass / 328 fail / 2 timeout**.
- **test262 metadata parser indentation**: the local runner now accepts
  `negative:` metadata with arbitrary YAML indentation, matching current
  test262 files such as update-expression early-error tests.
- **Null/undefined base before ToPropertyKey**: `base[key]` compound
  assignments now check for null/undefined base (ToObject) after
  evaluating the key expression but before calling ToPropertyKey,
  matching spec evaluation order. `null[throwingToString] *= x` throws
  TypeError (not the toString error). ToPropertyKey is called exactly
  once per spec (T4 series).
- **With-object own-property checks**: `with`-object binding lookups in
  get_value/put_value now use `has_own_property` (not `has_property`),
  so inherited prototype properties are not mistakenly found on the
  binding object.
- **`with` + `var` initialization**: `var foo = x` inside a `with` block
  now also sets the with-object's property when it already has one, so
  `with(o){ var foo = "set in with" }` results in `o.foo === "set in with"`.
- **Const/TDZ enforcement in put_value**: `put_value` now uses
  `set_checked` so const reassignment throws TypeError and TDZ access
  throws ReferenceError.
See [docs/test262.md](docs/test262.md#three-pass-rate-scopes) for the
three distinct pass-rate scopes and what each measures.

- **Switch lexical scope**: `switch` now creates a lexical environment
  (like a block). Function declarations, `var`, and `let`/`const` in case
  bodies are hoisted into the switch scope instead of leaking to the
  enclosing scope.
- **Catch parameter scope**: `catch (x)` now uses block scope
  (`push_scope(false)` + `PushScope`/`PopScope`) instead of function scope,
  so catch bindings are properly lexically scoped.
- **Escaped get/set**: `Token` gains `had_escape` field; escaped identifiers
  like `\u0067et` are treated as regular property names, not getter keywords.
- **For-of destructuring init errors**: `for (const [x] = 1 of [])` now
  throws `SyntaxError`.
- **For-of/for-in head-body name clash**: `for (let x of []) { var x; }`
  now throws `SyntaxError` per spec EarlyErrors.
- **Class constructor call check**: class constructors throw `TypeError`
  when called without `new`. `CallSuperCtor` sets `pending_new_target`
  so `super()` is treated as a construct call.
- **Derived constructor return override**: returning a non-object value
  from a derived constructor throws `TypeError` per spec.
- **Class accessor enumerability**: class getter/setter accessors now use
  `DefineClassAccessor` with `enumerable=false`.
- **Class prototype writability**: class constructor `.prototype` is now
  non-writable per spec.
- **Double super() check**: a second `super()` call throws `ReferenceError`.
- **Class extends validation**: `ValidateExtends` opcode checks superclass
  is a constructor with valid prototype.
- **BigInt increment/decrement**: `Inc`/`Dec` opcodes handle both Number and
  BigInt types.
- **Delete on null/undefined**: `delete null[0]` throws `TypeError`.
- **Object.prototype.toString**: returns `[object Object]` for plain objects.
- **String.prototype.toString/valueOf**: added missing methods to
  `String.prototype`.

- **Inline-cache invalidation**: `SetElem` (`o["x"] = v`) and
  `Object.defineProperty` now invalidate the monomorphic property cache
  so that subsequent `GetProp` reads the freshly written value instead of
  a stale cached value.

- **BigInt divide/modulo by zero**: `1n / 0n` and `1n % 0n` now throw a
  `RangeError` instead of returning `0n`.

- **BigInt exponent overflow**: `BigInt` exponentiation with an exponent
  that does not fit in a `u32` now throws a `RangeError` instead of
  silently clamping to zero and returning the wrong value.

- **Arrow function early errors**: arrow functions reject duplicate parameter
  names in sloppy and strict mode, and reject `eval`/`arguments` parameter
  names when strict mode applies.

- **Tagged-template objects**: each template-literal site now returns a
  cached, frozen template object with a frozen `raw` property, matching
  `GetTemplateObject`. `Object.getOwnPropertyDescriptor` also returns
  descriptors for Array exotic `length` and index properties.

- **for-in/for-of non-declaration parsing**: `for (x in obj)` and
  `for ((x) in obj)` now parse and assign correctly (was: SyntaxError or
  undefined). Added `no_in` flag to prevent `in` being consumed as a binary
  operator in for-head expressions.
- **StoreGlobal stack imbalance**: function declaration hoisting left an
  `undefined` on the stack, corrupting `console.log(f())` when `f` contained
  a nested function declaration. Fixed by emitting `Pop` after each hoisted
  function declaration.
- **with-statement var semantics**: `var foo = "x"` inside `with(o)` where
  `o` has a `foo` property now sets `o.foo` per ES5 spec. Previously the
  assignment went to the function-scope root, bypassing the with-object.
- **Strict-mode eval/arguments enforcement**: `eval = 42`, `var eval`,
  `function eval()`, `function f(eval)`, and duplicate parameters in strict
  mode now throw `SyntaxError` at parse time.
- **try-finally continue/break**: `continue`/`break` inside a `finally`
  block caused an infinite loop because `finally_stack` was not popped
  before compiling the finally body, causing `DivertContinue` to loop back
  into the same finally.
- **Class declarations in single-statement position**: `if (x) class C {}`
  and similar now throw `SyntaxError` per ES6 spec.
- **for-in/for-of non-declaration left side assignment**: the iterator value
  was left on the stack and discarded instead of being assigned to the
  variable. Now uses `compile_assign_target` for proper assignment.
- **eval stack corruption**: eval ran on the shared VM stack, so `Halt`
  could pop caller values when the eval body ended via break/continue. Fixed
  by pushing a sentinel and truncating the stack after eval.
- **do-while continue target**: `continue` in `do-while` jumped to the loop
  body start instead of the condition test, causing infinite loops.
- **let/const in single-statement position**: `if (x) let y = 1;` now throws
  `SyntaxError` per ES6 spec (lexical declarations require a block).
- **Switch completion value tracking**: switch now returns the last non-empty
  expression value as its completion, matching ES spec `UpdateEmpty` semantics.
- **Assignment target validation**: invalid assignments like `x - y = 1` or
  `1 + 2 = 3` now throw `SyntaxError` at parse time. Valid targets: identifiers,
  member/element access, private field access, and destructuring patterns.
- **test262 runner**: handles `onlyStrict` flag by prepending `'use strict'`.

## [0.4.0-alpha] - 2026-07-02

### Heap Limit Enforcement Overhaul

- **`Heap::allocate` now returns `Result<usize, HeapLimitExceeded>`** with a
  `From<HeapLimitExceeded> for Arc<Error>` impl, so exceeding the limit
  produces a catchable `RangeError("heap limit exceeded")` at *every*
  allocation site — not just object literals.
- **Eliminated `allocate_unchecked` / raw `heap.allocate().unwrap_or(0)`**:
  all 59+ call sites that previously bypassed the heap limit (Array methods
  like `map`/`filter`/`slice`, `JSON.parse`, `RegExp.exec`, `Proxy`,
  `Map`/`Set` constructors, Promise allocation, generator creation, etc.)
  now propagate the error via `?`.
- **Removed sentinel `usize::MAX`** return from `Heap::allocate` — the
  previous sentinel pattern could cause index-out-of-bounds panics when
  callers forgot to check it.
- **Fixed GC-on-allocate with empty roots**: `Heap::allocate` no longer
  calls `self.collect(&[])` (which would sweep every live object). GC
  before allocation is now done by `Vm::alloc` with the correct root set
  via `self.collect_roots()`.
- **Signature changes**: `Vm::new()`, `register_fn()`, `setup()`,
  `setup_full()`, `make_builtin_constructor()`, `make_error_constructor()`,
  `make_builtin_constructor_with()`, `build_math()`, `build_json()`,
  `build_reflect()`, `build_console()`, `make_value_array()`,
  `make_str_array()`, `make_array()`, `map_entries_list()`,
  `clone_lexical_env()`, `clone_loop_vars()`, `new_env()`,
  `new_with_env()`, `new_iterator()`, `new_lazy_iterator()`,
  `new_generator_iterator()`, `make_error_value()` now return `Result`.
- **Verification tests**: added `heap_limit_enforced_in_json_parse`,
  `heap_limit_enforced_in_array_map`, `heap_limit_enforced_in_regexp`
  to `tests/fuel.rs`, confirming the limit is enforced through builtin
  code paths that were previously bypassable.

### Security / Hardening
- **Generator resume panic**: `resume_generator` used `frames.pop().expect(...)`, which
  would abort the process if a generator frame was missing. Converted to an
  internal `Error` so the VM reports a catchable runtime error instead of panicking.
- **Number radix formatting panic**: `biguint_to_radix` used `String::from_utf8(...).unwrap()`
  on an ASCII-only digit buffer. Replaced with `unwrap_or_default()` to remove the
  unconditional panic path.
- **Direct `args[idx]` indexing**: Replaced the remaining direct `args[0]`/`args[1]`
  accesses in `src/builtins.rs` with safe `get()`/`first()` fallbacks. All call sites
  were already guarded by length checks, but the new form removes any latent panic
  path if a builtin is invoked with fewer arguments through meta-programming.
- **VM invariant unwraps**: Added an empty-frame guard at the top of the
  `interpret_inner_raw` loop and converted the two `finally_stack.last().unwrap()`
  paths in throw/finally diversion to `ok_or_else` propagation. The remaining
  `frames.last().unwrap()` calls are loop-invariant and will be hardened during
  the `vm.rs` module split.
- **Lock poisoning panic**: Replaced `std::sync::Mutex` with `parking_lot::Mutex`
  throughout the engine. `parking_lot::lock()` is panic-free, removing ~200
  latent `lock().unwrap()` panic paths (the remaining unwraps are on `Option`/
  `Result`/`Vec` operations, not on mutex acquisition).
- **cargo-fuzz target**: Added `fuzz/fuzz_targets/fuzz_target_1.rs` exercising the
  public `Vm::run` API with fuel-capped execution. Initial 30-second run completed
  over 50,000 iterations without triggering a panic.
- **Module split**: `src/builtins.rs` (7,000+ lines) was split into
  `src/builtins/{mod,math,json,global,array,string,collections,regexp,function}.rs`.
  `src/vm.rs` was split into `src/vm/{mod,ops}.rs`, with the main opcode dispatch
  loop and helpers moved to `ops.rs`.

### Performance
- **Map/Set O(1) lookups**: Replaced `Vec`-backed linear scans with
  `IndexMap`/`IndexSet` using a `MapKey(Value)` wrapper that implements
  `Hash`/`Eq` via SameValueZero semantics (NaN == NaN, -0 == +0).
- **UTF-16 ASCII fast-path**: `utf16_len`, `utf16_get`, `utf16_slice` now
  check `is_ascii()` first, skipping `encode_utf16().count()` for the
  common case where byte length equals UTF-16 length.
- **Monomorphic inline cache**: `GetProp` caches `(heap_idx, key)` -> `Value`
  to skip the prototype-chain walk on repeated property reads. Cache is
  invalidated on `SetProp` to prevent stale reads. Capped at 4096 entries.

### Features
- **Proxy**: `new Proxy(target, handler)` with `get`/`set` traps and
  `Proxy.revocable()` returning `{ proxy, revoke }`. Revoked proxies throw
  `TypeError` on trap invocation.
- **TypedArray (Uint8Array)**: `new Uint8Array(length)` or
  `new Uint8Array(arrayLike)` with index access, `length`, `byteLength`,
  and `byteOffset` properties. `TypedArrayKind` enum defined for all 8
  typed array element kinds.
- **toJSON support**: `JSON.stringify` now calls `toJSON()` on objects
  that define it before serializing, matching ES spec behavior.
- **UTF-16 correctness**: `str_includes`, `str_split` (empty separator),
  and `str_replace_all` (empty pattern) now use UTF-16 code-unit iteration
  instead of Rust `chars()`.
- **RegExp lastIndex UTF-16**: `RegExp.exec` bounds check and match offset
  calculation now use UTF-16 code-unit indices, preventing panics on
  supplementary characters.
- **VM invariant unwrap removal**: All 50 `frames.last().unwrap()` calls
  in `vm/ops.rs` and `vm/mod.rs` converted to `current_frame()?` /
  `current_frame_mut()?` safe propagation. Zero `unwrap()` calls remain
  in those files.
- **Incremental GC**: `Heap::collect_incremental(roots, budget)` marks up
  to `budget` cells per call, allowing the VM to avoid long GC pauses.
- **Async tick API**: `Vm::tick()` executes a single microtask and returns,
  enabling cooperative event-loop scheduling by hosts.

### Documentation
- Added `docs/audit-panics.md` documenting the `unwrap()`/`expect()` inventory in
  `src/vm.rs` and `src/builtins.rs`, reachability policy, and remaining work.

## [0.3.0-alpha] - 2026-07-01

### Added
- **Execution fuel / interrupt**: `Vm::set_fuel(Some(n))` bounds execution
  to ~n opcodes; exhaustion throws `RangeError("fuel exhausted")` that is
  **not catchable** by user `try/catch` (a host-level abort), so untrusted
  code cannot swallow it and keep looping. `None` (default) is unbounded.
  Cooperative, checked before each opcode.
- `Map`/`Set`/`Array.includes` keys now compare by **SameValueZero**
  (`NaN === NaN`, `-0 === +0`), so `new Map().set(NaN,1).get(NaN)` returns 1.

- **Full test262 CI**: `.github/workflows/test262-full.yml` runs the entire
  test262 suite across directory-split parallel jobs and aggregates results
  into the GitHub Actions summary. `intl402`/`staging` are excluded;
  unsupported-feature tests are skipped via an expanded `SKIP_FEATURES` set.
  Baseline: 76,397 tests, 60,178 run, 19,987 pass (33.2%).

### Security
- **Array-index DoS (OOM)**: `a[0x80000000]` used to materialize ~2B dense
  slots and OOM-kill the host. Now only `0..2^32-1` are array indices (ES
  spec); valid indices beyond the dense cap are stored sparsely, so
  `a[0x80000000]` returns the value and advances `length` without holes.
- **`String.prototype.repeat` panic**: `repeat(Infinity)` panicked with
  a capacity overflow; `repeat(-1)` returned `""`. Now validates the count
  (non-negative integer, 256 MiB result cap) and throws `RangeError`.
- **`padStart`/`padEnd` hang**: `padStart(Infinity)` hung the engine in an
  unbounded fill loop. Now clamps negatives to 0 and throws `RangeError`
  on `Infinity`/absurd lengths.
- **`JSON.parse` / `JSON.stringify` stack overflow**: deeply nested input
  (e.g. `"[" * 100000`) aborted the process via native-stack overflow in
  `parse_json_value` / `stringify_value` / `has_json_cycle`. All three now
  take a depth parameter capped at 256 and throw/return instead of crashing.
- **`Array.from` DoS**: `Array.from({length: 2**26})` materialized 64M dense
  slots and hung. Now capped at 4M with a `RangeError`.
- **Prototype-cycle DoS**: `a.__proto__=b` (where b's chain contained a)
  created a cycle; a later property read overflowed the native stack and
  aborted the process. Cyclic `__proto__` assignments now throw `TypeError`
  in strict mode / no-op in sloppy mode, and `get_property_rx` carries a
  depth cap as a backstop.
- **`Array.prototype.sort` DoS (O(n^2))**: `a.sort(cmp)` used an inline
  O(n^2) insertion sort; sorting 10k random elements took ~30s and called
  the comparator ~250k times. Now uses a stable merge sort (O(n log n));
  comparator calls dropped to ~9k for 1k elements. NaN/non-number
  comparator results are treated as 0 (equal); thrown errors propagate.

### Fixed (conformance)
- **`Date` TimeValue range**: `new Date(1e20).getTime()` returned the raw
  number instead of `NaN`. ES TimeValue must be within +/-8.64e15 ms;
  out-of-range/NaN/Infinity are now Invalid Date, matching V8/Node.
- **`Number.prototype.toString(radix)` fractional**: `(1.5).toString(2)`
  returned `"1.5"` instead of `"1.1"`. Now converts both the integer and
  fractional parts in the requested radix (common cases match V8/Node;
  minimal shortest-round-trip representation is still longer).
- **`String.prototype.charAt` range**: `charAt(-1)` returned `"a"` instead
  of `""` (Rust `as usize` saturates negatives to 0). Now uses `ToInteger`
  with an explicit range check, matching V8/Node.
- **`ToInt32`/`ToUint32`**: bitwise ops coerced with Rust's `as i32`/`as
  u32`, which saturate large values to `INT32_MAX`/`UINT32_MAX`. Now uses
  modular reduction (`(2**31)|0` -> `-2147483648`, `(2**32)|0` -> `0`).
- **`charCodeAt`/`codePointAt`**: negative/out-of-range indices returned
  the index-0 value instead of `NaN`/`undefined` (Rust `as usize` saturates
  negatives to 0). Now uses `ToInteger` with explicit range checks.
- **`String.prototype.split` limit**: negatives returned `[]` instead of
  all parts; `NaN` returned all parts instead of `[]`. Now `NaN` -> 0,
  negative/infinite -> unbounded, otherwise trunc toward zero.
- **`Number.prototype.toFixed`**: `toFixed(-1)` returned `"1"`, `toFixed(200)`
  produced a 201-digit string. Now validates `0..=100` and throws
  `RangeError`, matching V8/Node.
- **`Number.prototype.toPrecision`**: `toPrecision(0/-1/101)` produced wrong
  output instead of `RangeError`. Now validates `1..=100`.
- **`Object.defineProperty`**: a non-object descriptor (e.g. `true`) was
  silently accepted. Now throws `TypeError` per `ToPropertyDescriptor`.


### Fixed
- `gc::live_count` now locks `free_list` before `cells` to match
  `allocate()`, removing a lock-order inversion deadlock.
- GC alloc counter uses `fetch_add` instead of a racy load+store.
- Removed the global `#![allow(unreachable_patterns)]`; a duplicate lexer
  arm and a shadowed bool/bigint loose-eq arm were real dead code and are
  gone, remaining intentional fallbacks carry a local `#[allow]`.

### Changed
- Documented that `pub` internal modules are not a semver-stable API
  (embed against the re-exports), Map/Set are O(n) `Vec`-backed, and
  `with_obj` is non-reentrant on the same index. test262 numbers clarified
  as a curated subset, not full conformance.
- **Unicode identifiers & escapes**: `IdentifierStart`/`IdentifierContinue`
  now accept Unicode letters and the `\uXXXX` / `\u{XXXX}` escape forms
  inside identifiers (`\u{63}ase` parses as `case`; `café`/`π`/CJK names
  lex correctly). NEL/LS/PS are recognized as line terminators. Invalid
  escapes and non-id Unicode bytes advance the cursor instead of looping.
- **Destructuring parameters**: arrow functions and ordinary functions
  accept destructuring params (`([a, b]) =>`, `function f({x, y})`),
  including nested patterns and defaults (`[[x, y, z] = [4, 5, 6]]) =>`).
  Each destructuring param binds from a synthesized positional temp.
- **Object-literal methods**: generator methods (`*foo() {}`) and async
  methods (`async foo() {}`, `async *foo() {}`) now parse; reserved words
  (`return`, `class`, `default`, ...) are accepted as property keys.
- **Sloppy-mode `this`**: top-level `this` in non-strict script binds to
  the global object.
- **test262 negative-test handling**: the runner parses `negative: { phase,
  type }` metadata so a test that expects a `SyntaxError`/`TypeError`
  passes when RuJa raises the matching error; tests run via a temp file
  instead of `-e` argv so long sources and non-ASCII survive intact.
- **test262 subset pass rate**: raised from ~20% to ~67% on a
  representative `language/` subset (arrow-function 35%→69%, function
  16%→57%, object 26%→69%, identifiers 28%→59%).
- **test262 harness**: the runner now loads the real test262 harness files
  (`assert.js`, `sta.js`, and per-test `includes:` like `propertyHelper.js`,
  `compareArray.js`) instead of a hand-rolled stub. This makes pass/fail
  accurate (the stub was too lenient, e.g. `-0` vs `+0`). Pass rate is now
  measured against the real conformance assertions: 20.1% (was 28.3% under
  the lenient stub — the drop is correctness, not regression).
- **`Function.prototype.toString`**: returns `function name() { [native code] }`
  for native functions and `function name() { ... }` for interpreted ones.
  This fixes function-to-primitive coercion (`fn + 1`) which previously threw
  because the function had no `toString`.
- **Boxed primitives store their value**: `new Number(5)`, `new Boolean(true)`,
  `new String("x")`, and `Object(x)` now keep the wrapped primitive on the
  object, so `.valueOf()` returns it and `ToPrimitive` resolves to it
  (`new Number(5) + 1 === 6`). Previously wrappers were empty objects.
- **`ToPrimitive` throws on unconvertible objects**: when both `valueOf` and
  `toString` return objects, OrdinaryToPrimitive now throws `TypeError` per
  spec (was: silently fell back to a string form).
- **`Object(1n) + 1` throws `TypeError`**: BigInt-wrapper arithmetic now
  applies the BigInt/Number mixing rule after ToPrimitive unwraps the box.
- **Vertical tab / form feed are whitespace**: the lexer now treats `\x0b`
  and `\x0c` as whitespace, fixing a class of test262 parse failures.
- test262 expressions pass rate: 28.3% -> 31.9% (2476 -> 2790 passing).
- **`Vm` is now `Send`**: the engine migrated from `Rc`/`RefCell`/`Cell`
  to `Arc`/`Mutex`/atomics for shared ownership and interior mutability.
  A `Vm` can be moved between threads; concurrent shared access still needs
  external synchronization (e.g. `Mutex<Vm>`). The GC trace loop is now
  worklist-based to avoid re-entrant locking of the cells mutex (which
  would deadlock under `Mutex`). `with_obj` takes the object out of its
  cell during the callback so the cells mutex is never held across a
  user/allocation callback.
now run the `finally` body before completing the transfer (single-level).
- **Private class fields** (`#field = init`): isolated per-instance storage
  via `GetPrivate`/`SetPrivate` opcodes; not enumerable or in `Object.keys`.
 a known limitation).
- **Sloppy-mode `this`**: plain function calls now bind `this` to `globalThis`
  in non-strict mode (strict mode stays `undefined`).
- **`new C(...spread)`**: constructor calls with spread arguments via a new
  `NewSpread` opcode.
- **Tagged template literals**: `tag`q0${e0}q1`` calls `tag(strings, e0)`
  with a `strings.raw` array.
- **Async arrow functions**: `async () => ...`, `async (a,b) => ...`,
  `async x => ...`.
- **JSON.stringify** replacer (array whitelist / function) and space
  (indentation); **JSON.parse** reviver (bottom-up transform).
- **String.replace** with a function callback (match, captures, offset,
  string); **String.split** with a RegExp separator.
- **Reflect** global: get/set/has/deleteProperty/ownKeys/getPrototypeOf/
  setPrototypeOf/isExtensible/preventExtensions/apply/construct.
- **WeakMap**/`WeakSet` globals (API-compatible; entries are strong-ref).
- **Date** global (minimal): `Date.now()`, constructor, `getTime()`.

### Added (round 2)
- **Static initialization blocks executed**: `static { }` now runs with
  `this` = the constructor in source order (was parsed-but-ignored). Fixed
  the `CallThis` stack ordering and a `StoreEnv` undefined leak that left
  the constructor off the top of the stack.
- **Private class methods** (`#method() {}`): called via `this.#method(...)`;
  private method calls use a new `CallPrivateMethod` opcode so `this` binds
  to the receiver. Private field `++`/`--` also works.
- **BigInt literals**: `123n`, `0xffn`, `0o17n`, `0b101n` with exact
  arithmetic (`+ - * / % **`), comparison, `===`/`==` (BigInt vs Number is
  `false` for `===`, numeric for `==`); mixing throws `TypeError`. `BigInt()`
  constructor and `BigInt.prototype.toString` supported.
- **Nested try/finally**: non-local transfers (`return`/`throw`/`break`/
  `continue`) now run **all** enclosing `finally` blocks innermost-first
  (was: only the innermost for break/continue). Guard ordering is tracked
  with push-sequence numbers so a throw runs a finally nested inside the
  nearest catch before reaching the catch; a `return`/`throw` inside a
  `finally` overrides the pending completion.


### Added
- **Object spread** `{...a, y:2}` copies enumerable own properties via a new
  `ObjSpread` opcode.
- **Object rest destructuring** `{a, ...r} = obj` collects remaining own
  enumerable properties via a new `ObjRest(n)` opcode; `Pattern::Object` now
  carries an optional rest field.
- **Getters/setters** in object literals (`get x() {}` / `set x(v) {}`) and
  class methods (static + instance) via a new `DefineAccessor` opcode.
  Inherited accessors bind `this` to the receiver (`get_property_rx`).
- **`new.target`** meta-property via a new `NewTarget` opcode; `Construct`
  sets `pending_new_target` on the pushed frame.
- **`for(;;)`** with any combination of empty init/condition/update.
- **Numeric separators** (`1_000`, `0xff_ff`, `0b1010_1010`, `3.14_15`).
- **`globalThis`** routes property get/set to the global environment record;
  rooted in `collect_roots` to survive GC.
- **`__proto__`** accessor: get returns `[[Prototype]]`, set updates it.
- **Object statics**: `getPrototypeOf`/`setPrototypeOf`,
  `preventExtensions`/`isExtensible`, `seal`/`isSealed`/`isFrozen`,
  `getOwnPropertyDescriptors`, `defineProperties`.
- **Array**: `reduceRight`, `toReversed`, `toSorted`, `toSpliced`, `with`.
- **String**: `codePointAt`, `concat`, `search`, `String.raw`,
  `String.fromCodePoint`.
- **Number**: `toPrecision`, `toExponential`.
- **Math**: `imul`.
- **`console.log`** now formats arrays as `[ 1, 2, 3 ]` and objects as
  `{ a: 1 }` (Node.js inspect-style) instead of bare `toString`.

### Fixed
- **Labeled block break**: `lab:{r=1; break lab; r=2;}` previously returned
  `2` because `StmtNode::Block` never received a labeled frame. Block now
  takes the non-loop labeled-statement branch that pushes a break-only frame.
- **`to_number` on objects** now runs `ToPrimitive` (valueOf then toString)
  instead of returning `NaN`, so `+{valueOf(){return 7}}` yields `7` and
  `1 + [1]` yields `11`.


## [0.2.1-alpha] - 2026-06-28

### Fixed
- **GC root safety**: `collect_roots` now roots the microtask queue (Promise
  handlers, resolve/reject values), `generator_proto`, and `global_constants`,
  all of which were previously missing. A new `gc_pins` stack lets call paths
  pin heap values held in Rust locals (Promise handler, call args, derived
  promise) across allocations. Per-instruction GC was unsafe (it could free
  values held in Rust locals); it now runs at safe points only (after `run()`
  settles all frames, and throttled at frame boundaries). Fixes use-after-free
  panics under heavy allocation + Promise chains.
- **Runtime error source lines**: errors now report their source line, e.g.
  `ReferenceError: undefinedVar is not defined (at line 3)`. Previously every
  error reported `(at line 0)` because the compiler emitted all ops with line
  0 and the AST carried no line info. `Stmt` now carries a `line` (set by the
  parser at statement start), the compiler tracks `current_line` and flows it
  into every `Op`, and `Chunk::line_for_ip` resolves it.
- **Unimplemented Op panic**: the dispatch fallthrough arm now panics with
  the offending op (Op derives Debug) instead of silently skipping, so
  compiler bugs surface immediately.
- **`run()` test helper**: the shared test helper now panics on runtime error
  instead of returning `Value::Undefined`, so a test can no longer silently
 pass on a thrown error. Tests that genuinely expect an error use `run_err`.
- **Call-stack depth limit**: unbounded JS recursion now throws a catchable
  `RangeError: Maximum call stack size exceeded` instead of overflowing the
  Rust thread stack and aborting the process with `SIGSEGV`. The engine caps
  the interpreted call depth, and the `ruja` binary runs execution on a
  64 MiB worker thread so the limit can be generous.
- **`writable: false` honored by ordinary assignment**: writing to a
  non-writable own data property now fails per ES `[[Set]]` — throwing a
  `TypeError` in strict mode and failing silently in non-strict mode —
  instead of always overwriting the value.
- **Accessor (getter/setter) descriptors**: `Object.defineProperty` now
  reads `get`/`set` from the descriptor (rejecting a get+value or set+value
  mix with a TypeError), and `get_property`/`set_property` invoke the
  accessor. Inherited setters up the prototype chain are honored on write.
- **`Array.length` validation**: assigning a fractional, negative,
  non-numeric, or out-of-`uint32`-range value to an array's `length` now
  throws `RangeError: Invalid array length` (matching V8) instead of silently
  truncating via `as usize` or attempting an enormous allocation.
- **`num_to_string` exponential precision**: `String(n)` for values rendered
  in exponential notation (e.g. `5e-17`, `9e-17`, `9.99e-7`) is now exact,
  using Rust's `{:e}` formatting. Previously `n / 10f64.powi(exp)` introduced
  floating-point error (`5e-17` -> `4.999999999999999e-17`) and the exponent
  could be padded (`e-07` instead of `e-7`). The mantissa is now
  normalized (trailing zeros and a dangling `.` stripped) and the exponent
  digits are stripped of leading zeros, so output stays correct regardless
  of how the formatter rounds a given value.
- **`String()`/`Number()`/`Boolean()` as functions return primitives**:
  previously these routed through the generic `Object` constructor and
  returned `[object Object]` for every input. They now use dedicated
  constructors: `String(x)` returns the ToString coercion (`String()` is `""`),
  `Number(x)` returns the ToNumber coercion (`Number()` is `0`,
  `Number(undefined)` is `NaN`), and `Boolean(x)` returns the ToBoolean
  coercion. `new String/Number/Boolean(x)` still constructs an object with the
  correct prototype (RuJa does not model wrapper-object internal slots, so the
  primitive is not stored, but `typeof new String(5)` is now `"object"`).
- **Deeply-nested expression DoS**: untrusted input with deeply-nested
  expressions (e.g. thousands of nested parens) previously overflowed the Rust
  parser stack and aborted the process. The parser now caps expression nesting
  depth and throws a SyntaxError instead.
- **`Array()` constructor**: `Array(n)` / `new Array(n)` (single numeric arg)
  and `Array(a, b, c)` now create real arrays. Previously the generic
  `object_constructor` was wired in, returning `[object Object]` for every
  input. Invalid lengths (negative, fractional, out of `uint32` range) throw
  `RangeError: Invalid array length`.
- **`delete` respects `configurable`**: `delete o.x` on a non-configurable
  own property now returns `false` (or throws a TypeError in strict mode)
  instead of forcibly removing it.
- **`ToPrimitive` honors `valueOf`/`toString`**: object-to-primitive coercion
  (used by `+`, comparison, etc.) now calls the object's `valueOf` then
  `toString` (or vice-versa for the string hint). Arrays join correctly
  (`[1,2] + [3,4]` is `"1,23,4"`); a custom `valueOf`/`toString` is honored.
- **Labeled statements**: `label: stmt`, `break label`, and `continue label`
  now parse and compile (for `while`/`for`/`do...while`). A `break label`
  exits the matching outer loop; `continue label` resumes it.
- **`try/finally` non-local transfers**: a `return` or `throw` in a
  `try` (or `catch`) is now suspended across the `finally` block and re-raised
  afterward, so a `return` inside `finally` correctly overrides an earlier
  completion. (`break`/`continue` in `try`/`catch` still bypass `finally`.)

### Changed
- **README `Known limitations`** rewritten to reflect the implemented state
  (for-await, strict mode, eval isolation, array-destructuring iterator
  protocol, Function constructor are done) and list only the genuine remaining
  limits.
- **`interpret_inner` refactor**: the largest call/closure-related Op
  handlers (`op_call`, `op_call_method`, `op_call_method_opt`,
  `op_call_spread`, `op_new`, `op_await`, `op_make_closure`) extracted into
  dedicated methods, shrinking the dispatch loop from 1366 to ~1216 lines.

## [0.2.0-alpha] - 2026-06-28

### Added
- **Symbol-keyed properties**: a `PropertyKey` model (string/Symbol) backs all
  object `props` maps, so `[Symbol.iterator]` and arbitrary Symbol keys store
  and read correctly and are skipped by `for...in`/`JSON.stringify`.
- **Per-frame generator run-state**: `gen_mode`/`gen_yield`/`gen_suspended`/
  `gen_resume_value` moved from VM-global fields into `CallFrame`, so a
  generator body that calls `next()` on another generator is fully isolated.
- **`yield*` delegation**: `yield* expr` forwards each value of a delegated
  iterable/generator to the outer generator (supports arrays, strings, nesting).
- **Custom `Symbol.iterator`**: `make_iterator` honors a user-defined
  `[Symbol.iterator]()` method, wrapping the returned iterator in a lazy
  `IteratorData` that calls the JS `next()` per pull (infinite iterables work).
- **Computed property keys** `[expr]` in object literals now accept any
  expression (was restricted to identifiers/strings).
- **`async function*`**: `next()` returns a Promise resolved with `{value, done}`;
  `await` works inside the body (synchronous microtask-drain model).
- **TDZ for default-parameter self-reference**: `function f(a = a)` throws
  `ReferenceError` when the default is used (parameter is in the TDZ during
  default evaluation).
- **`with` statement**: dynamic object environment records; name lookups and
  assignments check the `with` object's properties first (precedence over the
  lexical chain), then fall back to lexical/global.
- **`eval`**: global `eval(x)` returns non-strings unchanged and parses/compiles/
  runs strings at runtime. Indirect eval runs globally (var leaks to global);
  direct `eval(...)` is detected from runtime callee resolution and runs in the
  caller's scope.

- **Strict mode**: `"use strict"` directive prologues are parsed and propagated
  through the AST/compiler scope chain. `with` is a SyntaxError in strict mode;
  duplicate formal parameters are rejected (non-strict still allows them, last
  wins via a per-parameter slot map). Classes are always strict.
- **Generator `throw`/`return` injection**: `g.throw(e)` injects the exception
  at the suspended `yield` point (the body's try/catch can handle it; otherwise
  it propagates out). `g.return(v)` force-completes the generator with `v`.
  Driven by a new `ResumeKind` (Next/Throw/Return) and a frame-level
  `force_throw`.
- **`for await...of`**: async iteration via `Symbol.asyncIterator` (falling
  back to the sync `Symbol.iterator` protocol), awaiting each `next()` result.
  `Symbol.asyncIterator` is now exposed on the global `Symbol` object.
- **Direct eval lexical isolation**: `let`/`const`/`class` declared in direct
  `eval` no longer leak to the caller; `var`/function declarations still leak to
  the caller's function scope (and not over existing lexical bindings).
- **Iterator protocol for array destructuring**: `let [a, b] = iterable` now
  uses the iterator protocol, so generators, custom iterables, and strings
  destructure correctly (not just arrays). Rest uses a new `IteratorCollectRest`.
- **`Function` constructor**: `new Function(p0, ..., body)` dynamically compiles
  a function from parameter and body strings; a body `"use strict"` directive
  is honored (strict body rejects duplicate parameters).
- **Strict eval sandbox (minimal)**: under strict mode, direct eval no longer
  leaks `var` to the caller (in-function). `Chunk.is_strict` threads caller
  strictness to the eval.

- Bytecode compiler: AST -> stack-machine Op codes (single-pass, lexical scopes)
- Stack-based VM with call frames, operand stack, and return/call dispatch
- Mark-and-sweep garbage collector (gc.rs) tracing from VM roots
- New value model: HeapObj enum with GcIdx heap handles
- Environment-based variable storage (environment.rs)
- Try/catch/finally with Throw jumping to catch handlers
- Built-in objects: Object/Array/String/Number/Boolean/Function/Math/JSON/console/Error
- Array methods: push, pop, map, filter, reduce, forEach, find, includes, slice, concat, join
- String methods: charAt, charCodeAt, slice, split, replace, includes, startsWith, endsWith, repeat, trim, toUpperCase, toLowerCase
- Math: floor, ceil, round, abs, sqrt, pow, max, min, sin, cos, tan, log, exp, random, and constants
- JSON parse and stringify
- parseInt, parseFloat, isNaN, isFinite globals
- 17 passing integration tests + 13 unit tests

### Changed
- Replaced v1.0 tree-walking interpreter with bytecode VM
- Replaced Rc<RefCell> value model with GC-managed HeapObj
- Variables stored in environment chain instead of local slots

### Fixed
- Silent bug: `for...of` produced wrong values (0/empty) — was not compiled
- `extends` inheritance: subclass methods now resolve through the prototype chain
- `super.f() + 5` now returns 15 (was 2)
- Static methods now return their value (e.g. `C.s()` returns 42)
- `for...in` no longer leaks non-enumerable builtin prototype methods
- `break`/`continue` were no-ops (caused infinite loops) — now functional via loop jump stack
- `++`/`--` threw or returned wrong values — correct prefix/postfix semantics + store back
- Unary `+` was negation — now coerces to number (`+"5" === 5`)
- `>`/`>=` on strings always returned false — now correct
- `in` operator returned the key — now returns a boolean
- `void` returned its operand — now returns undefined
- `delete` returns boolean and removes the property
- `instanceof` returns a boolean (walks the prototype chain)
- `typeof undeclaredVar` threw — now returns "undefined"
- `switch` fallthrough and `default` were broken — now correct
- `finally` blocks never executed — now run on both try-normal and catch paths
- `Math.round` rounds half toward +Infinity per ES (`round(-0.5) === 0`)
- Default-param prologue left a stale stack value corrupting subsequent calls
- Builtin prototype methods and `constructor` are now non-enumerable
- Error constructor now links instances to `<Error>.prototype` (instanceof works)

## [0.1.0-alpha] - 2026-06-26

Initial alpha: tree-walking interpreter, ES5.1 subset, 56 tests.
