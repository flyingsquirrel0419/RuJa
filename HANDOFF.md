# RuJa Handoff - 2026-08-04

## Current unit: Temporal.ZonedDateTime fixed-offset civil surface

- Added all 24 ISO civil/calendar/offset accessors, branded/fixed-offset-string
  static `from`, `toInstant`, full option-aware `toString`, `toJSON`, and
  always-throwing `valueOf`. Exact integer civil conversion and formatting
  cover negative epochs, extended years, ISO week/year, and nine rounding
  modes while preserving method-Realm allocation and errors.
- Time-zone storage now distinguishes UTC, fixed offset, and named identifiers.
  This unit executes only UTC/fixed offsets; named IANA/DST remains rejected
  until a deterministic timezone backend exists.
- Frozen 208 exact Test262 paths with complete feature/include/flag/negative
  metadata. Exact gate is **208/0/0**. Forced 266-file diagnostic is
  **208/58**; blockers are 51 property-bag/options/IANA/string-grammar `from`
  cases, one property-bag accessor fixture, five PlainDateTime fixtures, and
  one shared helper requiring other Temporal constructors.
- GPT-5.6 reviews found and fixed civil-day range validation plus admission
  complement/disjointness gaps. Full Rust, exact/forced Test262, tooling,
  release, MSRV, wasm32, workflow YAML, fmt, and Clippy gates pass locally.
- Ordinary CI exposed an inaccessible default `/root/test262`; the optional
  live probe now follows existing tooling policy and treats `OSError` as an
  absent checkout. Explicit full-CI checkout validation remains mandatory.
- Implementation commit `64146f6` and portability fix `6a4c1a1` are pushed.
  Ordinary CI `30867357732` passes **3/3** and full Test262
  `30867357709` passes **57/57**, including exact **208/0/0** and forced
  **208/58/0-skip** gates. The following evidence-only docs commit does not
  require Actions observation per user instruction.
- Next bounded unit: implement the ISO property-bag path for
  `Temporal.ZonedDateTime.from`, preserving field/options observable order and
  keeping named-IANA transition semantics closed until a deterministic tzdb
  exists. Start from the frozen 51-file `from` blocker complement.
- Mandatory every-turn cleanup: close all subagents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs,
  analyzer dumps, Python/Cargo caches, and temporary files; prune worktrees;
  verify no related process, clean git/origin parity, and free disk.

## Current unit: Temporal.ZonedDateTime hidden-slot core

- Added Realm-local `%Temporal.ZonedDateTime%` with real hidden epoch,
  UTC/fixed-offset identifier, offset-minute, and ISO calendar slots. The
  constructor preserves ToBigInt/range/identifier/newTarget order and supports
  proper subclasses and foreign constructor Realms.
- Added strict branded `epochNanoseconds`, floor `epochMilliseconds`,
  `timeZoneId`, and `calendarId` accessors. Instant `compare`, `from`, and
  `equals` now recognize cross-Realm ZonedDateTime slots before observable
  primitive conversion.
- Frozen 36 exact Test262 paths. Exact gate is **36/0/0**; complete Instant
  compare is **30/0/0**, from+equals is **62/0/0**, and ZonedDateTime top-level
  constructor files were **18/0/2** at this checkpoint. The later fixed-offset
  civil unit owns the completed formatting surface; Duration remains separate.
- Added identifier byte fuel, Realm registry/root/rollback, exhaustive GC,
  allocation failure, constructor mode, and hidden-slot regressions. GPT-5.6
  audits caught strict-parser, registry, pin-reservation, negative-zero
  canonicalization, and stale-document risks; both final re-reviews are CLEAN.
- Final local gates pass: all-target/features tests including library
  **422/422**, builtins **593/593**, fuel **40/40**, and benchmark smoke; exact
  Test262 **36/0/0**, compare **30/0/0**, from+equals **62/0/0**, and root
  constructor **18/0/2**; tooling **186/186** with five expected
  absent-checkout skips; fmt; Clippy `-D warnings`; Rust 1.88 MSRV; wasm32;
  release build and Realm rollback; generated Intl/YAML/Python checks;
  vendored RegExp **38/38** plus Clippy/no_std.
- Commit `a4c4472` is pushed. Ordinary CI `30858592976` passes all **3/3**
  jobs and test262-full `30858593321` passes all **56/56** jobs, including the
  dedicated exact ZonedDateTime core gate.
- Root/vendor Cargo targets, sparse Test262 checkout, Python caches, and
  matching temporary files were deleted, reclaiming about **5.2 GiB**. No
  related process remains, the worktree is clean, and `HEAD == origin/main`.
- Mandatory every-turn cleanup: close all subagents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs,
  analyzer dumps, Python/Cargo caches, and temporary files; prune worktrees;
  verify no related process, clean git/origin parity, and free disk.

## Current unit: Temporal.Instant.compare completion

- Added Realm-local nonconstructable static `compare`, length 2. It converts
  first then second through the shared exact Instant path and directly compares
  hidden `Arc<BigInt>` epochs without result allocation or public getter reads.
- Added method-Realm, abrupt-order, string-hint, cross-Realm brand, exact fuel,
  generic native, and eleventh-allocation rollback regressions.
- Frozen 29 exact Test262 paths. Exact gate is **29/0/0**; complete compare is
  **29/0/1**. The sole blocker constructs a real ZonedDateTime and requires its
  hidden-nanoseconds fast path.
- GPT-5.6 corpus audit confirms the exact 29/1 split. Runtime review found and
  fixed an oversized pin reservation, weak per-input fuel proof, and missing
  positive-zero assertion; admission review found and removed one stale scope
  sentence. Both final re-reviews are CLEAN.
- Final local gates pass: all-target/features tests including library
  **420/420**, builtins **592/592**, fuel **39/39**, and benchmark smoke; exact
  Test262 **29/0/0** and complete compare **29/0/1**; tooling **185/185** with
  five expected absent-checkout skips; fmt; Clippy `-D warnings`; Rust 1.88
  MSRV; wasm32; release build and Realm rollback; generated Intl/YAML/Python
  checks; vendored RegExp **38/38** plus Clippy/no_std.
- Commit `73f65ac` is pushed. Ordinary CI `30853376546` passes all **3/3**
  jobs and test262-full `30853376548` passes all **55/55** jobs, including the
  dedicated exact compare gate.
- Root/vendor Cargo targets, sparse Test262 checkout, Python caches, and
  matching temporary files were deleted, reclaiming about **5.0 GiB**. No
  related process remains, the worktree is clean, and `HEAD == origin/main`.
- Mandatory every-turn cleanup: close all subagents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs,
  analyzer dumps, Python/Cargo caches, and temporary files; prune worktrees;
  verify no related process, clean git/origin parity, and free disk.

## Current unit: Temporal.Instant.prototype.toString fixed-offset completion

- Added Realm-local nonconstructable `toString`, exact i128 civil-date
  formatting, all nine as-if-positive rounding modes, precision/unit options,
  and UTC/fixed-offset time-zone strings. Option access order, method-Realm
  errors, GC pins, and time-zone byte fuel stay in the VM layer.
- Refactored the syntax parser to expose offset and annotation structure for
  deterministic time-zone identifiers without using host timezone APIs.
- Frozen 54 exact Test262 paths: 52 runnable toString files plus the two newly
  unblocked from/equals wrong-type files. Exact gate is **54/0/0**; toString is
  **52/0/2**, from+equals is **60/0/2**, and all three directories are
  **112/0/4**. The four skips are two real Temporal-constructor test/helper
  dependencies and two ZonedDateTime fast paths.
- GPT-5.6 requirements/runtime audits found option-validation ordering,
  AnnotatedTime designator/ambiguity, and exact unit-name edges. All findings
  are fixed; final re-review is CLEAN and the agent is closed. No fake Temporal
  constructors are admitted.
- Final local gates pass: all-target/features tests including library
  **419/419** and benchmark smoke; exact Test262 **54/0/0** and combined
  **112/0/4**; tooling **184/184** with five expected absent-checkout skips;
  fmt; Clippy `-D warnings`; Rust 1.88 MSRV; wasm32; release build and Realm
  rollback; generated Intl/YAML/Python checks; vendored RegExp **38/38** plus
  Clippy/no_std.
- Commit `9ee8278` is pushed. Ordinary CI `30848749761` passes all **3/3**
  jobs. test262-full `30848749763` passes all **54/54** jobs on attempt 2;
  attempt 1 had two transient Annex B RegExp timeouts and the failed annexB
  job passed on its isolated rerun. The dedicated exact toString job passes.
- Root and vendored Cargo targets, sparse Test262 checkout, Python caches, and
  matching temporary files were deleted, reclaiming over **7.3 GiB**. No
  related process remains, the worktree is clean, and `HEAD == origin/main`.
- Mandatory every-turn cleanup: close all subagents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs,
  analyzer dumps, Python/Cargo caches, and temporary files; prune worktrees;
  verify no related process, clean git/origin parity, and free disk.

## Current unit: Temporal.Instant shared string parser

- Expanded the shared exact-integer parser to independent basic/extended
  date, time, and offset forms; hour-only forms; nanosecond offset seconds;
  and audited RFC 9557 timezone/calendar/unknown annotations.
- String bytes are precharged as native fuel after observable conversion and
  before parsing. Direct tests cover grammar, boundaries, annotation rules,
  and exact N-1/exact fuel behavior.
- Frozen 36 new Test262 files. Exact gate is **36/0/0**; complete `from` plus
  `equals` directories are **58/0/4**, with wrong-type/toString branding and
  ZonedDateTime fast paths as the exact four skips.
- Final local gates pass: all-target/features tests including library
  **416/416**, integration suites, and benchmark smoke; tooling **183/183**
  with five expected absent-checkout skips; fmt; Clippy `-D warnings`; locked,
  wasm32, and release builds; generated Intl/YAML/Python checks; vendored
  RegExp **38/38** plus Clippy/no_std; exact Test262 **36/0/0** and combined
  **58/0/4**.
- GPT-5.6 review found three RFC 9557 grammar edges and one admission-proof
  weakness. Annotation values now require hyphen-separated alphanumeric
  components, named time-zone components enforce their leading character,
  time-zone annotations must precede key/value annotations, and the four
  blockers' existence/metadata/skip policy are frozen. Re-review is CLEAN and
  the agent is closed.
- Commit `b0da917` is pushed. Ordinary CI `30841833180` passes all **3/3**
  jobs and test262-full `30841833706` passes all **53/53** jobs, including the
  dedicated exact parser boundary. Root and vendor Cargo targets, the sparse
  Test262 checkout, Python caches, and matching temporary files were deleted;
  cleanup reclaimed about **7.6 GiB**. No related process remains, the worktree
  is clean, and `HEAD == origin/main`.
- Mandatory every-turn cleanup: close all subagents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs,
  analyzer dumps, Python/Cargo caches, and temporary files; prune worktrees;
  verify no related process, clean git/origin parity, and free disk.

## Current unit: Temporal.Instant.prototype.valueOf

- Added Realm-local, non-constructable `valueOf` that immediately throws the
  method Realm's `TypeError` for every receiver and exposes no epoch value.
- Frozen the complete seven-file pinned Test262 directory with exact path,
  feature, include, flag, and negative metadata validation. Dedicated CI
  requires **7/0/0**.
- Installation reserves and pins the new function transactionally; a dedicated
  ninth-allocation heap-cap regression verifies rollback and reclamation.
- Final local gates pass: complete Rust targets, fmt, Clippy, locked/wasm
  checks, vendored RegExp tests/Clippy/no_std, tooling **182/182**, workflow
  parse, and exact Test262 **7/0/0**. GPT-5.6 final review is CLEAN.
- Commit `c9391df` is pushed. Ordinary CI `30837091802` and all **51/51** jobs
  in test262-full `30837090949` pass, including the dedicated valueOf gate.
- Next bounded Instant work: complete static `compare` needs the shared RFC
  9557/offset parser expansion; its 30th test also requires a real
  `%Temporal.ZonedDateTime%` internal-slot fast path, not a test-only shape.
- Mandatory every-turn cleanup: close all subagents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs,
  analyzer dumps, Python/Cargo caches, and temporary files; prune worktrees;
  verify no related process, clean git/origin parity, and free disk.

## Current unit: Temporal.Instant.from and string conversion

- Added Realm-local, non-constructable `Temporal.Instant.from` plus shared
  exact string conversion for `equals`. Branded Instant copies bypass
  observable properties; objects use string-hint primitive conversion.
- Parser scope is extended ISO date-time with required `Z` or `+/-HH:MM`, up
  to nanosecond precision, alternate time separators, and leap seconds. RFC
  9557 annotations, compact/second offsets, and ZonedDateTime remain gated.
- Frozen exactly 15 Test262 files. Dedicated CI requires **15/0/0**.
- Required closeout: focused/full Rust tests, fmt, Clippy, exact Test262,
  tooling/workflow validation, GPT review, commit/push/CI.
- Mandatory every-turn cleanup: close all subagents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs,
  analyzer dumps, Python/Cargo caches, and temporary files; prune worktrees;
  verify no related process, clean git/origin parity, and free disk.

## Current unit: Temporal.Instant.prototype.equals

- Added non-constructable `equals` with exact hidden epoch comparison,
  receiver-first brand checking, and cross-Realm branded Instant support.
- Scope is intentionally branded-Instant-only. String, conversion-object, and
  ZonedDateTime inputs remain gated behind the future `ToTemporalInstant` unit.
- Added an exact seven-file Test262 admission plus an installer failure test at
  the new equals allocation point. Dedicated CI requires **7/0/0**.
- Required closeout: focused/full Rust tests, fmt, Clippy, vendor gates, exact
  7/0/0 Test262, tooling/workflow validation, GPT review, commit/push/CI.
- Mandatory every-turn cleanup: close all subagents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs,
  analyzer dumps, Python/Cargo caches, and temporary files; prune worktrees;
  verify no related process, clean git/origin parity, and free disk.

## Current unit: Temporal.Instant epoch factories

- Added `fromEpochMilliseconds` and `fromEpochNanoseconds` as non-constructable
  Realm-local native methods. They ignore receivers and observable global
  mutations, use exact standard coercions, and enforce inclusive epoch limits.
- Frozen all 19 Test262 files across both factory directories. The two
  `limits.js` files now use the completed `Temporal.Instant.from(string)` and
  `Temporal.Instant.prototype.equals` paths.
- Ordinary CI exposed an inaccessible default `/root/test262` path in the
  tooling test. The frozen list is now always checked statically; live file and
  metadata checks run only when a checkout is available through `TEST262`.
- Required closeout: focused/full Rust tests, fmt, Clippy, vendor gates, exact
  19/0/0 Test262, tooling/workflow validation, GPT review, commit/push/CI.
- Mandatory every-turn cleanup: close all subagents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs,
  analyzer dumps, Python/Cargo caches, and temporary files; prune worktrees;
  verify no related process, clean git/origin parity, and free disk.

## Current unit: Temporal.Instant core

- Added Realm-local `%Temporal.Instant%`, a dedicated hidden-slot heap object,
  exact constructor/prototype descriptors, strict branded epoch getters, and
  floor division for negative `epochMilliseconds` values.
- Constructor fallback and `Date.prototype.toTemporalInstant` use immutable
  Realm registries. Date bridging selects the method function's Realm even
  when borrowing the method across Realms or replacing the global `Temporal`.
- Installation and instance creation pin every temporary GC edge and preserve
  transactional Realm setup/rollback behavior under heap-cap failures.
- GPT review found and closed two follow-ups: getter installation now uses the
  GC-aware allocation/retry path, and a deleted or non-string
  `Symbol.toStringTag` falls back to `[object Object]` instead of leaking the
  Temporal brand through the host class tag.
- Frozen exactly 19 constructor/prototype/epoch-accessor Test262 files in the
  shared runner/analyzer boundary. Dedicated full CI requires **19/0/0**;
  parsing, arithmetic, duration, timezone, and remaining Instant methods stay
  outside this unit.
- Mandatory every-turn cleanup: close all subagents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs,
  analyzer dumps, Python/Cargo caches, and temporary files; prune worktrees;
  verify no related process, clean git/origin parity, and free disk.

## Current unit: Temporal namespace tags

- Added Realm-local `%Temporal%` and `%Temporal.Now%` ordinary namespace
  objects with Realm-local `%Object.prototype%`, exact `Now` and
  `Symbol.toStringTag` descriptors, and distinct identity across Realms.
- Installation reserves its temporary root before allocation, pins
  `%Temporal.Now%` while allocating `%Temporal%`, publishes only after both
  allocations succeed, and restores pin depth/global identity on heap-cap
  failure.
- Frozen exactly four pinned `built-ins/Temporal/**/toStringTag` files in the
  shared runner/analyzer admission. Dedicated full CI requires **4/0/0**;
  constructors, `Temporal.Now` methods, calendar arithmetic, and timezone
  behavior remain outside this unit.
- Final local gates pass: all-target/all-feature Rust tests, root Clippy and
  rustfmt, vendored RegExp tests/Clippy/no_std check, exact tooling admission,
  workflow parse, and pinned Temporal namespace **4/0/0**. The full tooling
  suite was not rerun against the intentionally sparse local Test262 checkout;
  full-CI setup validated every live manifest against a complete checkout.
- Commit `280aeb0` is pushed. Ordinary CI `30760417262` passes **3/3**;
  full Test262 CI `30760417251` passes every job, including the dedicated
  Temporal namespace **4/0/0** gate.
- Next bounded Temporal unit should be selected from fresh failure evidence;
  do not widen the four-file namespace admission.
- Mandatory every-turn cleanup: close all subagents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs,
  analyzer dumps, Python/Cargo caches, and temporary files; prune worktrees;
  verify no related process, clean git/origin parity, and free disk.

## Current unit: SuppressedError intrinsic

- Added Realm-local `SuppressedError(error, suppressed, message)` through the
  existing error-family factory. It inherits from `%Error%`, supports plain
  calls, custom/foreign `newTarget` fallback, and creates optional `message`
  before non-enumerable `error` and `suppressed` properties.
- Constructor inputs and the new object are pinned across observable message
  coercion. Existing dynamic `realm_error_prototypes` rooting, publication,
  rollback, and reclamation cover the new intrinsic without a separate VM
  registry.
- CI exposed a test-fixture lifetime bug: the abrupt-message object had been
  unrooted before an earlier independent forced GC. The regression now removes
  each fixture's global root only immediately before the constructor path it
  exercises.
- GPT-5.6 review found and closed one prototype bug: only `%Error.prototype%`
  now owns `toString`; `SuppressedError.prototype` inherits the exact
  Realm-local function. Forced-GC success, abrupt coercion, and root-reservation
  failure tests verify payload identity and exact pin restoration.
- Frozen all 22 pinned `built-ins/SuppressedError` paths with complete metadata
  in a shared runner/analyzer admission. Future files and broader
  explicit-resource-management syntax remain gated. Dedicated full-CI job
  requires exact **22 pass / 0 fail / 0 skip**.
- Local final gates pass: focused Rust tests, all-target/all-feature tests,
  rustfmt, root and vendored Clippy/tests/checks, 176 Test262 tooling tests,
  workflow YAML parse, and pinned SuppressedError **22/0/0**.
- Implementation and GC-regression commits `a73c826`, `26aae80`, and `5e79a32`
  are pushed. Final ordinary CI `30748824457` and full Test262 CI
  `30748824458` both pass; the dedicated SuppressedError job confirms exact
  **22/0/0**. The Annex B job's first attempt hit two known RegExp timeouts,
  then its automatic retry passed without a code change.
- Previous URI implementation commit `547a21e` passes ordinary CI
  `30745132322` and full Test262 CI `30745132329`.
- Mandatory every-turn cleanup remains: close agents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs,
  analyzer dumps, Python/Cargo caches, and temporary files; prune worktrees;
  verify no related process, clean git/origin parity, and free disk.
- Next bounded unit: classify the next complete independent Test262 failure
  cluster after SuppressedError; keep disposal syntax as its own larger unit.

## Current unit: URI decoder allocation and sandbox boundary

- Replaced `decodeURI`/`decodeURIComponent`'s whole-input `Vec<char>`
  conversion and per-sequence heap buffers with byte-indexed parsing, a fixed
  four-byte UTF-8 buffer, original-slice reserved escapes, and one fallible
  output reservation.
- Shared Unicode-scalar append now emits the U+F0000-U+F07FF collision range
  directly as two surrogate sentinels, avoiding a temporary String while
  preserving the distinction from lone UTF-16 surrogates for every caller.
- After observable `ToString`, native decode work precharges the input byte
  length as fuel. Direct regressions cover lowercase reserved spelling,
  supplementary/sentinel decoding, long component input, one coercion per
  call, fuel abort, and successful retry.
- Pinned URI remains **167 pass / 0 fail / 2 timeout / 4 skip**. The two RFC
  3629 exhaustive files each execute about 983,000 JavaScript calls per
  variant and still exceed the ordinary eight-second process limit; no timeout
  exception was added. Closing them requires interpreter call/dispatch
  throughput rather than a URI semantic change.
- Mandatory every-turn cleanup remains: close agents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries,
  logs, analyzer dumps, Python/Cargo caches, and temporary files; prune
  worktrees; verify no related process, clean git/origin parity, and free disk.
- Next bounded unit completed above: the 22-file `SuppressedError` intrinsic
  cluster is implemented while explicit-resource-management syntax stays
  separately gated.

## Current unit: Annex B legacy Date methods

- Added Realm-local, non-constructable `Date.prototype.getYear` and `setYear`.
  `setYear` snapshots DateValue before coercion, handles invalid dates and the
  legacy 0-99 offset, preserves state on abrupt coercion, and writes clipped
  results. `toGMTString` is the same function object as each Realm's original
  `toUTCString`.
- Exact five-file admission opens only three Reflect.construct/arrow-function
  metadata tests and two Symbol coercion tests. Its expected path/feature map
  is independently frozen in tooling tests and disjoint from all other
  manifests.
- Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
  moves the complete Date scope from **5/19/0** to **24/0/0**. Complete Annex B
  is expected to move from **1027/19/40** to **1051/0/35**; remaining skips are
  IsHTMLDDA-only.
- Focused Rust regression, exact tooling admission test, rustfmt, Clippy, and
  focused Test262 pass. GPT-5.6 final review is clean after exact manifest,
  descriptors, abrupt coercion, Symbol, missing argument, and Realm coverage
  were strengthened.
- Implementation commit `c3b1da6` and workflow gate commit `0c52930` are
  pushed. Latest ordinary CI `30727866413` passes **3/3**; latest full Test262
  Annex B job `91443023480` passes, including the exact Date admission and
  required **24/0/0** Date boundary. Full run `30727866412` passes **45/45**.
  Complete Annex B is raw **1050/0/35** with one unrelated timeout; normalized
  movement from **1027/19/40** is **+24 pass / -19 fail / -5 skip**. Aggregate
  is raw **33378/4115/10972**, 4 timeout / 0 error over 48469 files.
- Mandatory every-turn cleanup remains: close agents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs,
  analyzer dumps, Python/Cargo caches, and temporary files; prune worktrees;
  verify no related process, clean git/origin parity, and free disk.
- Next bounded unit: classify the complete Annex B residual 35 IsHTMLDDA skips
  as host-exotic unsupported coverage, then select the next independent
  conformance cluster outside Annex B.

## Current unit: Annex B global escape functions

- Added Realm-local, non-constructable `escape` and `unescape`. Both perform
  one `ToString`, preserve UTF-16 code units and lone surrogates, implement the
  exact legacy passthrough/hex and malformed-input rules, and preserve caller
  versus function Realm error identity.
- Native work consumes an O(1) UTF-8 byte-length fuel upper bound before any
  UTF-16 traversal. Checked lengths and fallible intermediate reserves convert
  capacity failures to RangeError. Direct regressions cover coercion order,
  malformed/non-rescanned input, surrogates, metadata, construction, created
  Realms, and fuel exhaustion.
- Exact four-file admission opens only each function's non-constructor and
  Symbol abrupt tests. Pinned Test262
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4` is focused **35/0/0** and
  complete Annex B is **1027/19/40**, from **992/50/44**. The remaining 19
  failures are legacy Date; all 40 skips require IsHTMLDDA.
- Local all-target/all-feature tests, Clippy `-D warnings`, fmt, wasm32,
  vendored RegExp checks, generated Intl checks, workflow YAML, and tooling
  **174/174** pass. Two GPT-5.6 final reviewers are CLEAN after one reviewer
  found and verified the pre-meter UTF-16 length-scan fix.
- Implementation commit `d8d9fc1` is pushed. Ordinary CI `30711317106` passes
  **3/3** and full rerun `30712728403` passes **45/45**. The initial full run
  `30711317089` had one unrelated RegExp contention timeout. Ordinary artifact
  is byte-identical; 31/32 full artifacts are byte-identical and only Annex B
  changes by **+35 pass / -31 fail / -4 skip**. Aggregate is
  **33355/4134/10977**, 3 timeout / 0 error over 48469 files.
- Mandatory every-turn cleanup remains: close agents/sessions; run root and
  vendor `cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs,
  analyzer dumps, Python/Cargo caches, and all temporary RuJa files; prune
  worktrees; verify no related process, clean git/origin parity, and free disk.
- Next bounded unit: close the remaining 19 Annex B legacy Date failures.

## Current unit: complete Annex B RegExp grammar boundary

- Completed the four residual `annexB/built-ins/RegExp` files. Invalid
  non-Unicode `\c` now lowers to separate reverse-solidus and `c` atoms;
  character-class `\c0`-`\c9` and `\c_` lower to their modulo-32 control
  values. Unicode/Unicode Sets rejection remains strict.
- `regex_control_escape_value` is shared by Unicode syntax validation, legacy
  class-range validation/decoding, Unicode class atom decoding, quantifier
  scanning, and linear/fancy backend normalization. Direct regressions cover
  literals, constructors, incomplete/punctuation/Cyrillic tails, class ranges,
  class-only digit/underscore semantics, ignore-case, source preservation,
  fancy lookahead, and `u`/`v` rejection.
- A frozen four-file runner/analyzer admission opens the two generator control
  tests and two already-executable malformed named-group tests. Future paths,
  metadata drift, and overlap with every other admission manifest are rejected.
  Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
  is exact **4/0/0** and complete Annex B RegExp is **62/0/0**.
- Local gates pass: full `cargo test --all-targets --all-features` (405 lib,
  577 builtins, every integration and Criterion smoke), warnings-denied Clippy,
  wasm32, rustfmt/diff, generated Intl checks, workflow YAML, vendored RegExp
  **38/38** plus Clippy/no_std, and Python tooling **173/173** with five
  expected unavailable-checkout skips. Two GPT-5.6 final reviews are CLEAN
  after their shared-helper and outside-class edge findings were fixed.
- Implementation commit `51516e1` and evidence commit `fb1ab8f [skip ci]` are
  pushed. Ordinary CI `30705669529` passes **3/3**; full CI `30705669498`
  passes **45/45**. Ordinary Test262 artifact is byte-identical. Full result
  artifacts are 31/32 byte-identical; only Annex B moves raw from
  **987/51/48** to **992/50/44**. Archived-binary replay under the new policy
  isolates only the two control files as runtime `fail -> pass`; normalized
  semantic movement is +4 pass / -4 skip. Aggregate is normalized
  **33319/4166/10981**, 3 timeout / 0 error over 48469; raw CI is
  **33320/4165/10981** from one unrelated existing Annex B failure passing.
- Final cleanup removed 6.9 GiB of root Cargo output, 162.8 MiB of vendor
  output, Cargo/Python caches, pinned Test262, both generations of CI artifacts,
  and current/preceding temporary RuJa files. Both GPT agents and all command
  sessions are closed; no RuJa process remains. Worktrees are pruned, tracked
  git is clean, `HEAD == origin/main`, repository is 193 MiB, and root has
  11 GiB free.
- Next bounded unit: reclassify the remaining complete-Annex-B **50 failures /
  44 skips**, then choose the smallest independent exact cluster. RegExp itself
  is closed and should stay at 62/0/0.
- Mandatory every-turn cleanup: before every final response, close every agent
  and command session; run root and vendored `cargo clean`; delete
  `/root/test262`, every current/preceding CI artifact download, binaries, all
  root/nested/vendor targets, logs/analyzer dumps, Python/Cargo caches, and
  temporary RuJa files; prune worktrees; verify no RuJa-related process, clean
  git/origin parity, and free disk space. Never retain artifacts between turns.

## Current unit: Stage 3 legacy RegExp constructor statics

- Every Realm's immutable `%RegExp%` now exposes the proposal's 19 exact
  configurable accessors backed by 14 independently valid state slots. Direct
  same-Realm built-in matches update state; proper subclasses invalidate it;
  custom and mismatched cross-Realm exec paths do not redirect state.
- Commits retain one backing input plus fixed UTF-16 ranges. Derived match,
  context, and capture Strings materialize/cache on first accessor read, so
  hidden global matching does not copy full contexts per match. Assigning
  `input` restores only that slot without corrupting lazy preceding-match state.
- Unicode-global `@@match` uses bulk execution only for unmetered calls with a
  same-Realm intrinsic exec and an infallible linear backend. Workspace is
  reserved before matching; state commits before outer Array allocation.
  Metered, cross-Realm, and error-capable backends use generic per-exec dispatch
  and preserve completed-match state on later aborts. Result Arrays use the
  method Realm in both paths.
- Native Function source now omits exact legacy accessor names such as
  `get $&` when they cannot form a NativeFunction IdentifierName. Accessor
  `.name` remains exact. This closes the full-CI Function.toString regression.
- Direct regressions cover descriptors, aliases, setter reentrancy, deletion,
  UTF-16, 10th capture, subclasses, GC, materialization/outer-Array failures,
  backend aborts, Fuel progress, custom protocols, bidirectional `/g` and `/gu`
  Realm state/result prototypes, missing `@@match`, and fast-path selection.
  Two GPT-5.6 final reviews are closed CLEAN.
- Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4` is exact
  legacy accessors **24/0/0**, full Annex B RegExp **58/0/4**, isolated Annex B
  **987/51/48**, and Function.toString **80/0/0**, all with 0 timeout/error.
- Implementation commits `91674cd` and `3c71f0b`, plus evidence commit
  `84ba871 [skip ci]`, are pushed. Ordinary CI `30703293505` passes 3/3; full
  CI `30703293498` passes 45/45. Ordinary artifact is byte-identical. Full
  results are 31/32 byte-identical; only Annex B moves raw +25 pass / -18 fail
  / -6 skip / -1 timeout. Normalized semantic change is +24 pass / -18 fail /
  -6 skip. Aggregate is **33315/4166/10985**, 3 timeout / 0 error over 48469.
- Final cleanup removed 7.6 GiB of Cargo output. Pinned Test262, CI artifact
  comparisons, every root/nested/vendor target, Python caches, and temporary
  outputs are absent. Agents and command sessions are closed; no RuJa process
  remains. Worktrees are pruned, tracked git is clean, `HEAD == origin/main`,
  repository is 191 MiB, and root has 14 GiB free.
- Next bounded unit: close the four remaining Annex B RegExp skips, beginning
  with invalid control escapes; keep broader named-group grammar separate if
  the resulting parser/runtime change stops being narrow.
- Mandatory every-turn cleanup: before every final response, close every agent
  and command session; run root and vendored `cargo clean`; delete
  `/root/test262`, every current/preceding CI artifact download, binaries, all
  root/nested/vendor targets, logs/analyzer dumps, Python/Cargo caches, and
  temporary RuJa files; prune worktrees; verify no RuJa-related process, clean
  git/origin parity, and free disk space. Never retain artifacts between turns.

## Current unit: Stage 3 legacy RegExp.prototype.compile

- Implemented Realm-local, non-constructable `RegExp.prototype.compile` with
  exact name/length/descriptors. RegExp allocation now stores a traced creating
  Realm and `LegacyFeaturesEnabled`; direct intrinsic instances/literals/internal
  creation enable legacy behavior, while proper subclasses and cross-Realm
  borrowed methods reject before argument coercion.
- RegExp pattern arguments snapshot internal source/flags without observable
  reads. Existing `regexp_initialize` owns coercion, validation, matcher commit,
  and strict `lastIndex` ordering. Invalid syntax is atomic; immutable
  `lastIndex` throws after matcher commit. GC regression proves the private Realm
  slot alone preserves its Realm and releases it after the RegExp becomes dead.
- Exact four-file admission opens three Symbol abrupt tests and one duplicate
  named-group syntax test. Pinned compile plus dependent split/flags scope is
  **26/0/0** from 1/21/4. Full RegExp subtree is **34/18/10** from 9/39/14;
  isolated Annex B is **963/69/54** from 938/90/58, with 0 timeout/error.
- Local gates pass: all-target/all-feature Rust including 403 library tests and
  Criterion smoke, warnings-denied Clippy, wasm32, rustfmt/diff, vendored RegExp
  tests/Clippy/no_std+alloc, Python tooling **171/171** with five expected
  unavailable-checkout skips, exact pinned compile 26/0/0, full RegExp, and full
  Annex B. Two final GPT-5.6 reviews are closed; their only low-risk GC coverage
  finding was fixed and revalidated.
- Implementation commit `12f47b4` and evidence commit `e97bd11 [skip ci]` are
  pushed. Ordinary CI `30696563521` passes 3/3; full CI `30696563526` passes
  45/45. Ordinary artifact is byte-identical. Full results are 31/32
  byte-identical; only Annex B moves +25 pass / -21 fail / -4 skip. The unchanged
  Annex B contention timeout passes isolated. Normalized aggregate is
  **33291/4184/10991**, 3 timeout / 0 error over 48469.
- Final cleanup removed 5.9 GiB of root Cargo output and 232.5 MiB of vendor
  output. Pinned Test262, current/previous ordinary and full CI artifacts, all
  nested targets, logs/analyzer dumps, temporary RuJa files, and Python/Cargo
  caches are absent. Both GPT agents and all command sessions are closed; no
  RuJa-related process remains. Worktrees are pruned, git is clean,
  `HEAD == origin/main`, the repository is 188 MiB, and root has 15 GiB free.
- Next unit: implement the separate legacy RegExp constructor static-accessor
  and successful-match state cluster (18 remaining failures, 24 files including
  skips). Keep invalid control escapes and broader named-group admission separate.
- Mandatory every-turn cleanup: before every final response, close every agent
  and command session; run root and vendored `cargo clean`; delete
  `/root/test262`, every current/preceding CI artifact download, binaries, all
  root/nested/vendor targets, logs/analyzer dumps, Python/Cargo caches, and
  temporary RuJa files; prune worktrees; verify no RuJa-related process, clean
  git/origin parity, and free disk space. Never retain artifacts between turns.

## Current unit: Annex B String legacy methods

- Implemented all 13 Annex B `CreateHTML` String methods as distinct
  non-constructable Realm-local native functions backed by one shared
  algorithm. Receiver `RequireObjectCoercible`/`ToString` precedes optional
  attribute `ToString`; only U+0022 in attributes becomes `&quot;`.
- `substr` now uses `ToIntegerOrInfinity`, including fractional negative start,
  explicit-undefined length, Infinity, and Symbol abrupt behavior.
  `trimLeft`/`trimRight` are exact descriptor-correct aliases of each Realm's
  `trimStart`/`trimEnd` function objects. Main and created Realms expose the
  same surface with their own Function prototype graph.
- Exact 16-file policy admission opens 14 non-constructor tests and two Symbol
  abrupt tests. Six real IsHTMLDDA cases remain skipped. Focused pinned Test262
  moves from 9/80/22 to **105/0/6**; isolated complete Annex B moves from
  842/170/74 to **938/90/58**, with 0 timeout/error.
- GPT-5.6 reviews are closed. One reviewer found that the first shared
  `CreateHTML` draft used the String unboxing helper and bypassed overridden
  boxed-String coercion hooks. Final code uses ordinary `ToString`; direct
  regression covers boxed `Symbol.toPrimitive`, hint/order, and abrupt identity.
  The other review is CLEAN.
- Local gates pass: all-target/all-feature Rust including 402 library tests,
  every integration and Criterion smoke, warnings-denied Clippy, release,
  wasm32, vendored RegExp tests/Clippy/no_std+alloc, rustfmt/diff, workflow YAML,
  Python tooling **170/170** with five expected unavailable-checkout skips, and
  pinned focused String **105/0/6**.
- Implementation commit `14df555` and evidence commit `b5da9f8 [skip ci]` are
  pushed. Ordinary CI `30693875975` passes 3/3; full CI `30693875949` passes
  45/45. Ordinary supported-subset artifact is byte-identical. Full artifacts
  are 31/32 byte-identical; only Annex B changes by +96 pass / -80 fail / -16
  skip. One CI contention timeout passes in the isolated run. Normalized total
  is **33266/4205/10995**, 3 timeout / 0 error over 48469, with 37471 pass/fail
  executions.
- Final cleanup removed 6.8 GiB of root Cargo output and 235.6 MiB of vendor
  output. Pinned Test262, both full and ordinary artifact pairs, all nested
  targets, logs/analyzer dumps, temporary RuJa files, and Python/Cargo caches
  are absent. Both GPT agents and all command sessions are closed; no
  RuJa-related process remains. One unrelated Codex CLI remains in
  `/root/linux`. Worktrees are pruned, git is clean, `HEAD == origin/main`, the
  repository is 186 MiB, and root has 16 GiB free.
- Next unit: audit the remaining Annex B String-adjacent clusters in bounded
  order: RegExp legacy behavior (39), Date legacy behavior (19), then global
  escape/unescape (31). Keep IsHTMLDDA separate until a real host exotic exists.
- Mandatory every-turn cleanup: before every final response, close every agent
  and command session; run root and vendored `cargo clean`; delete
  `/root/test262`, every current/preceding CI artifact download, binaries, all
  root/nested/vendor targets, logs/analyzer dumps, Python/Cargo caches, and
  temporary RuJa files; prune worktrees; verify no RuJa-related process, clean
  git/origin parity, and free disk space. Never retain artifacts between turns.

## Current unit: RealmRecord ownership and ShadowRealm importValue

- Each global Environment now directly owns a traced `RealmRecord` containing
  immutable intrinsic roots, its module cache, tagged-template cache, and an
  optional host module referrer. Published Realms no longer stay alive through
  VM lookup registries; safe-point GC prunes indexes after their Environment is
  unreachable. A direct regression proves unreachable ShadowRealm reclamation.
- Module loading, import.meta, completion, TLA abort, namespace, synthetic
  defaults, and thrown values now use the explicit Realm's cache. ShadowRealm
  gets a fresh cache. The non-standard Test262 `$262.createRealm` hook shares
  the caller cache intentionally, preserving the existing same in-flight
  ModuleRecord/rejection-identity host contract.
- Tagged-template identity is Realm-owned and validates a `Weak<Chunk>` before
  accepting its raw address key, preventing address-reuse ABA. Cooked/raw
  Arrays use the active Realm Array prototype and remain cache roots across GC.
- `ShadowRealm.prototype.importValue` performs synchronous receiver/specifier/
  export-name validation, creates an eval-Realm inner Promise for the existing
  dynamic-import loader, and connects it through an internal continuation to a
  caller-Realm outer Promise. Primitive and callable exports cross the membrane;
  missing/object exports and catchable target failures become caller-Realm
  TypeErrors. The internal path never reads observable `.then` and covers TLA.
- Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4` ShadowRealm
  is **64/0/0**. Direct module regression covers primitive/callable/object/
  missing/syntax/throw/TLA, caller error prototypes, one-evaluation cache
  identity, and two isolated ShadowRealm caches. Exact admission and CI gate
  moved from 60/0/4 to 64/0/0.
- Local gates pass: all-target/all-feature Rust including 402 library tests and
  Criterion smoke, warnings-denied Clippy, fmt/diff, Python tooling 169 tests
  with five expected unavailable-checkout skips, workflow YAML, exact admission,
  and pinned ShadowRealm 64/0/0. Two GPT reviews are CLEAN and closed.
- Implementation commit `9409531` is pushed. Ordinary CI `30690961696` passes
  3/3 and full matrix `30690961694` passes 44/44. Against `30687712195`, raw
  CI artifacts are 30/32 identical because the intended built-ins movement was
  joined by two Annex B contention timeouts. Isolated CI reruns reduced this to
  one; pinned local Annex B is exactly 842/170/74/0 and byte-identical to the
  prior artifact. Normalized artifacts are 31/32 identical; only built-ins moves
  from 16816/4115/2734 to 16820/4115/2730. Aggregate is
  **33170/4285/11011**, 3 timeout / 0 error over 48469, 37455 pass/fail runs.
  Ordered normalized hash is
  `950d43ff57ccf8b654c84cf6112c9f0ef994d59c8fafdd5a5b096251f20cc780`.
- Evidence commit `8ab9d48 docs(test262): record RealmRecord baseline [skip ci]`
  is pushed. Docs-only Actions need no observation per user instruction.
- Final cleanup removed 5.6 GiB of root Cargo output, all nested/vendor targets,
  both full-CI artifact sets, Annex B rerun downloads, pinned Test262, logs and
  Python/Cargo caches. Both agents and all command sessions are closed; no
  related process remains. Worktrees are pruned, git is clean, HEAD equals
  origin/main, the repository is 185 MiB, and root has 16 GiB free.
- Next unit: audit the largest bounded non-Temporal residual cluster from the
  latest full built-ins/language artifacts. Keep changes narrow; bare module
  specifiers and host resolution remain a larger independent module-loader unit.
- Mandatory every-turn cleanup: before every final response, close every agent
  and command session; run root and vendored `cargo clean`; delete `/root/test262`,
  all CI downloads, binaries, targets including vendor targets, logs/analyzer
  dumps, Python/Cargo caches, and temporary RuJa files; prune worktrees; verify
  no related process, clean git/origin parity, and free disk space.

## Current unit: non-module ShadowRealm boundary

- Pinned ShadowRealm baseline was 0/54/10 over 64 test entries. Two GPT-5.6
  audits classified 60 non-module and four async Module tests, and found the
  existing function closure Realm plus transactional secondary bootstrap were
  usable foundations. Both agents are closed.
- `FunctionKind::Wrapped` now owns a traced target and caller Realm, is never
  constructable, and participates in Function Realm traversal and native
  `toString`. Wrapped calls recursively wrap callable arguments/results,
  reject objects, replace catchable abrupt completions with caller-Realm
  TypeErrors, and preserve non-catchable host aborts.
- `ShadowRealm` is installed in main and created Realms. Its instance stores
  the inner environment in a traced internal slot. `evaluate` separates caller
  parse errors from target execution, does not drain microtasks, and uses the
  same membrane. Secondary Realm scalar globals and JSON are now Realm-local.
- Exact 60-file admission corrects the stale lowercase `shadowrealm` skip-key
  mismatch and leaves only four `importValue` Module tests gated. Current
  pinned result is 60/0/4. Direct regressions cover descriptors, isolation,
  callable arguments/results, caller-Realm errors/prototypes, and forced GC.
- `importValue` currently implements synchronous receiver/specifier/export
  validation and returns a caller-Realm rejected Promise; Realm-owned module
  cache/loading remains the next unit. Successful secondary Realms also remain
  VM-lifetime roots pending RealmRecord ownership.
- Final local gates pass on the latest tree: focused ShadowRealm 4/4, exact
  pinned Test262 60/0/4, all-target/all-feature Rust including 401 library
  tests and Criterion smoke, warnings-denied Clippy, wasm32, fmt/diff, focused
  tooling metadata, and workflow YAML parse. Full tooling requires the complete
  checkout; the current sparse checkout intentionally contains only
  ShadowRealm. The final GPT review is CLEAN and its agent is closed.
- Implementation commit `c8e4882` is pushed. Ordinary CI `30687712196`
  passes 3/3 and full matrix `30687712195` passes 45/45. Against
  `30683219888`, 31/32 result artifacts are byte-identical; only built-ins
  moves from 16756/4169/2740 to 16816/4115/2734. Aggregate is
  33166/4285/11015 with 3 timeout / 0 error over 48469 and 37451 pass/fail
  executions. Ordered hash is
  `7bd7ee319599e2f9b24ffead37760fc788ea2e21cfa0eae5a0547bb020216329`.
- Evidence commit `f3edce7 docs(test262): record ShadowRealm baseline [skip
  ci]` is pushed; docs-only Actions were not observed per user instruction.
  Final cleanup removed 7.1 GiB of root Cargo output, 162.8 MiB of vendor
  output, pinned Test262, both CI artifact sets, logs/analyzer temporaries, and
  Python/Cargo caches. All agents and command sessions are closed; no related
  process remains. Worktrees are pruned, git is clean, HEAD equals
  `origin/main`, and free disk is 16 GiB.
- Next unit: introduce RealmRecord-owned intrinsic/module registries so
  unreachable ShadowRealms can be reclaimed, then implement `importValue`
  against that Realm-owned module cache. Keep the four async Module tests
  closed until the loader is real.
- Mandatory every-turn cleanup: before every final response, close every agent
  and command session; run `cargo clean`; delete `/root/test262`, all binaries,
  CI downloads, targets including vendor targets, logs, analyzer dumps,
  Python/Cargo caches, and temporary files; prune stale worktrees; verify no
  related process, clean git/origin parity, and free disk space.

## Current unit: Map and Set prototype tags

- Fresh pinned audit force-ran all 4235 policy-skipped class files at 4235/0,
  constructor/subclass exotic at 248/0 plus 72/0 built-in subclasses, and 47
  self-contained class Module early errors at 47/0. No runtime failure remains
  within these audited cohorts; further admission requires exact ownership.
- Full built-ins analyzer found 3816 direct Temporal ReferenceErrors plus
  related Temporal assertion failures, 54 ShadowRealm failures, four
  contention/hostile timeouts, and exactly three bounded non-Temporal/
  non-ShadowRealm failures: Map/Set prototype @@toStringTag descriptors.
- Worktree installs Realm-local non-writable, non-enumerable, configurable
  `"Map"`/`"Set"` own tags. Direct regression covers main/created Realms,
  descriptor shape, assignment rejection, delete fallback, and redefinition.
- Exact pinned cluster is 3/0 and complete Map+Set is 498/0/89 over 587,
  improving from analyzer baseline 495/3/89. Full CI has an exact 3/0 gate.
- Local gates pass: all-target/all-feature Rust tests, warnings-denied Clippy,
  fmt/diff, Test262 tooling 168 tests with five expected unavailable-checkout
  skips, generated Intl checks, vendored RegExp 38 tests plus Clippy/no_std,
  and wasm32. Exact pinned cluster is 3/0; complete Map+Set is 498/0/89.
  Two GPT-5.6 reviews are closed: runtime review CLEAN; documentation review's
  cohort-scope and stale aggregate findings are fixed.
- Implementation commit `67645f4` is pushed. Ordinary CI `30683219889`
  passes 3/3 and full matrix `30683219888` passes 45/45. Against
  `30680109971`, 31/32 artifacts are byte-identical; only built-ins moves from
  16753/4172/2740 to 16756/4169/2740. Aggregate is 33106/4339/11021 with 3
  timeout / 0 error over 48469. Ordered hash is
  `0b27d26ed526f0642ed9c464e79bed73959c6a03fc3c9509846ea565a706a2e6`.
- Evidence commit `3864046` is pushed; docs-only Actions need no observation.
  Final cleanup removed 5.7 GiB of root Cargo output, 162.8 MiB of vendor
  output, both CI artifact sets, pinned Test262, analyzer output, and Python/
  Cargo caches. All agents and command sessions are closed; no related process
  remains. Worktrees are pruned, git is clean, HEAD equals origin/main, and
  free disk is 17 GiB.
- Mandatory every-turn cleanup: before every final response, close every agent
  and command session; run `cargo clean`; delete `/root/test262`, all binaries,
  CI downloads, targets including vendor targets, logs, analyzer dumps,
  Python/Cargo caches, and temporary files; prune stale worktrees; verify no
  related process, clean git/origin parity, and free disk space.

## Current unit: complete class-elements boundary

- The sole residual class-elements skip is now admitted through the exact
  shared `MODULE_CLASS_ELEMENTS_FILES` singleton. Runtime already conformed:
  anonymous `export default class` receives name `"default"` before static
  public fields execute.
- Module regression observes `default` through constructor name, static field,
  and exported side effect. Tooling freezes pinned metadata, policy identity,
  future/mirrored sibling rejection, and extra-feature rejection. Full CI has a
  pinned preflight.
- Focused Rust regression passes. Exact tooling test passes with pinned sparse
  Test262; all 167 tooling tests pass with unavailable-root mode. Complete
  class-elements is **2962/0/0** over 2962 files. Full pinned supported
  statements/expressions is **12766/0/7673** over 20439, exactly +1 pass / -1
  skip from the prior baseline.
- Two GPT-5.6-sol xhigh investigations agree runtime semantics and exact
  admission placement are correct. Two final GPT reviews are CLEAN.
- Implementation commit `b5a391f` is pushed. First ordinary CI exposed a
  `PermissionError` from the new live-metadata probe when `/root/test262` is
  inaccessible; the focused fix treats `OSError` as unavailable checkout and
  has a direct regression in commit `a048b18`.
- Latest ordinary CI `30680109952` passes 3/3. Full matrix `30680109971`
  passes 45/45, including pinned metadata preflight and exact 2962/0/0 gate.
- Against `30676958637`, 31/32 result artifacts are byte-identical. Only
  language/expressions moves from 7865/0/3237 to 7866/0/3236. Aggregate is
  33103/4342/11021 with 3 timeout / 0 error over 48469; ordered hash is
  `fa099659980197bf14ef3efbb7f7cfe5519c4942aaf98759ed5f34c2a35cb2bd`.
- Evidence commit `7fae817 docs(test262): record class elements baseline [skip
  ci]` is pushed. HEAD equals origin/main.
- Final cleanup complete: all agents and command sessions are closed; root and
  vendor targets, `/root/test262`, CI downloads, logs, analyzer dumps,
  Python/Cargo caches, and RuJa temporary files are absent. Worktrees are
  pruned, no related process remains, and the repository is clean.
- Next unit: audit the residual class exotic/early-error clusters, prioritizing
  a real runtime failure cluster over further admission-only work.
- Mandatory every-turn cleanup: before every final response, close every agent
  and command session; run `cargo clean`; delete `/root/test262`, all binaries,
  CI downloads, targets including vendor targets, logs, analyzer dumps,
  Python/Cargo caches, and temporary files; prune stale worktrees; verify no
  related process, clean git/origin parity, and free disk space.

## Current unit: complete frame-owned compiler temporaries

- Implementation replaces Environment-backed destructuring/iterator/
  Reference/switch temporaries with dense `CallFrame` slots marked by `Chunk`.
  Nested defaults, host reentry, class/with environment changes, generator and
  async suspension, and switch completion now preserve independent state.
- Generator/async continuation records and both GC root walkers trace the slot
  vector. Terminal generator paths release saved slots plus activation env,
  args, receiver, resume value, control stacks, lexical closure, and vector
  capacity. Return/throw payloads remain pinned through materialization.
- Direct regressions cover nested declaration/assignment/rest/for-in patterns,
  identifier/member/private/super References, normal and abrupt IteratorClose,
  class-static host reentry, suspension, switch propagation, hidden binding
  absence, and ordinary/generator GC release.
- Final local gates pass: all-target/all-feature Rust including 401 library
  tests and every Criterion smoke, warnings-denied Clippy, fmt, and diff.
  Pinned Test262 assignment-destructuring/with/for-in/for-of is 1233/0/190 over
  1423. A first
  supported-subset run exposed 29 switch failures; StoreEnvName/continue routing
  was fixed and focused switch is now 69/0/42. Final supported statements/
  expressions is 12765/0/7674 over 20439 with 0 timeout/error.
- Two GPT-5.6 reviews found switch opcode routing, unstarted generator cleanup,
  completed generator activation/closure/capacity retention, and post-completion
  return/throw/yield root gaps. All findings are fixed; both final reviews are
  CLEAN and the agents are closed.
- Implementation commit `153ad4b` is pushed. Ordinary CI `30676958635` passes
  3/3 and full matrix `30676958637` passes 45/45.
- Against `30670755993`, all 32 Test262 result artifacts are byte-identical.
  Aggregate remains 33102/4342/11022 with 3 timeout / 0 error over 48469 files;
  ordered hash remains
  `29c6d1925c7b420d829aa9a67eae71a16d7c78d168cd449635acff4c744ca39f`.
- Evidence commit `0b8ef2a docs(test262): record frame temporary baseline
  [skip ci]` is pushed.
- Final cleanup complete: agents and command sessions are closed; root/vendor
  targets, `/root/test262`, CI downloads, logs, analyzer dumps, Python/Cargo
  caches, and RuJa temporary files are absent. Stale worktrees are pruned; no
  related process remains; HEAD equals `origin/main`; git is clean; free disk
  is 18 GiB.
- Next after this unit: class default-parameter/object-rest admission candidate
  at 2961/0/1 over 2962 files, then remaining class exotic/early-error work.
- Mandatory every-turn cleanup: before every final response, close every agent
  and command session; run `cargo clean`; delete `/root/test262`, all binaries,
  CI downloads, targets including vendor targets, logs, analyzer dumps,
  Python/Cargo caches, and temporary files; prune stale worktrees; verify no
  related process, clean git/origin parity, and free disk space.

## Completed unit: primitive Reference admission

- The existing exact primitive-base Reference manifest now includes the final
  `get-value-prop-base-primitive.js` Symbol sibling. No runtime code changes;
  both sloppy and strict variants already conform.
- Runner/analyzer policy stays path-exact. Tooling freezes all four paths and
  their per-path pinned live metadata, shared identity, future-sibling waiver
  rejection, and unrelated broad feature gates. Full CI has fail-fast metadata
  and 664/0/0 result gates.
- Local pinned Test262 moves `language/types/reference` from 28/0/1 to 29/0/0
  and the complete Reference/with/compound boundary from 663/0/1 to 664/0/0.
- Local gates pass tooling 166/166 with the checkout plus the focused absent-
  checkout case, supported statements/expressions 12765/0/7674, all-target/
  all-feature Rust tests including 399 library tests and Criterion smoke,
  warnings-denied Clippy, rustfmt/diff, Python compile, generated Intl checks,
  vendor regress 38/38 plus Clippy/no_std, wasm32, and workflow YAML parse.
- Two GPT-5.6 final reviews are CLEAN after their metadata-exactness,
  CI-gating, future-sibling wording, and stale-history findings were fixed;
  both agents are closed.
- Implementation/docs commit `8870346` is pushed. Ordinary CI `30670755996`
  passes 3/3 and full matrix `30670755993` passes 45/45, including the exact
  metadata and 664/0/0 fail-fast gates.
- Against `30666470497`, 31/32 artifacts are identical; only language/types
  moves +1 pass / -1 skip. Aggregate is 33102/4342/11022 with 3 timeout / 0
  error over 48469 files and 37444 completed pass/fail executions. Result hash
  is `29c6d1925c7b420d829aa9a67eae71a16d7c78d168cd449635acff4c744ca39f`.
- Evidence commit `5a23e4d docs(test262): record primitive reference baseline
  [skip ci]` is pushed.
- Next runtime unit: nested destructuring currently reuses fixed compiler
  temporaries and can overwrite an outer target Reference, source carrier, or
  iterator state from an inner default expression. Reproductions cover
  identifier/member/private/super targets, lost sibling object properties, and
  missed IteratorClose. Allocate unique live destructuring temporaries first.
- Following class admission candidate: diagnostically removing only
  `default-parameters` and `object-rest` from both class-elements paths yields
  2961 pass / 0 fail / 1 skip over 2962 files, up from 2207/0/755. The sole
  remaining skip is module-only
  `language/expressions/class/elements/class-name-static-initializer-default-export.js`.
- Final cleanup complete: root and vendor build artifacts, Test262 checkout,
  both CI artifact sets, analyzer dumps, logs, Python/Cargo caches, and stale
  worktrees are absent. No related command or agent remains; HEAD equals
  `origin/main`, git is clean, and free disk space was verified.
- Mandatory every-turn cleanup: before every final response, close every agent
  and command session; run `cargo clean`; delete `/root/test262`, all binaries,
  CI downloads, targets including vendor targets, logs, analyzer dumps,
  Python/Cargo caches, and temporary files; prune stale worktrees; verify no
  related process, clean git, origin parity, and free disk space.

## Current unit: complete Function toString admission

- Worktree adds one exact 35-file Function.prototype.toString path-to-feature
  admission shared by runner/analyzer. Pinned metadata, disjoint ownership,
  future siblings, invalid/outside paths, and unrelated features are gated by
  tooling; full CI hard-gates the complete 80-file directory at 80/0/0.
- Capture-free regular patterns use direct regex-automata hybrid matchers with
  explicit forward/reverse 512 KiB caches. Retained accounting charges both
  NFAs and four times both cache capacities for allocator slack/hash buckets.
  Backend selection is independent of cache state. After three inefficient
  clears at under ten bytes/state, the matcher permanently switches to its
  charged PikeVM program; fresh fallback scratch is dropped after each call.
- Focused Rust tests cover cache coexistence/hits, finite charge after growth,
  forced hybrid saturation/permanent Pike fallback, anchoring, flags, and
  iteration. Two GPT-5.6-sol xhigh final reviews are CLEAN and both agents are
  closed.
- Final local results: exact toString 80/0/0 in about 7 seconds; complete
  Function 495/0/14; supported statements/expressions 12765/0/7674 over 20439;
  focused regex tests 20/20; Test262 tooling 166/166.
- Local gates pass: all-target/all-feature tests including 399 library tests
  and every Criterion smoke case, warnings-denied Clippy, rustfmt, wasm32,
  generated Intl checks, and vendor regress 38/38 plus Clippy/no_std.
  `actionlint` is not installed; workflow syntax/static Python checks pass.
- Implementation `c3d219e` and tooling fix `f6564fa` are pushed. Final ordinary
  CI `30666470505` passes 3/3 and full `30666470497` passes 45/45. The earlier
  runtime full run `30665512851` also passes 45/45.
- Against `30655563999`, 31/32 artifacts are byte-identical; only built-ins
  changes +35/-35. Aggregate is 33101/4342/11023 with 3 timeout / 0 error over
  48469 files and 37443 completed pass/fail executions. Result hash is
  `ece081c014986f16e5dd89e5e88960f629838e637ed7cd6326e19429fe9632c7`.
- Evidence commit `c504ba2 docs(test262): record complete function baseline
  [skip ci]` is pushed.
- Final cleanup complete: main and vendor build artifacts, Test262 checkout,
  CI downloads, logs, analyzer dumps, Python/Cargo caches, temporary files, and
  stale worktrees are absent. No related process remains; HEAD equals
  `origin/main`, git is clean, and free disk space was verified.
- Mandatory every-turn cleanup: before every final response, close every agent
  and command session; run `cargo clean`; delete `/root/test262`, all binaries,
  CI downloads, targets including vendor targets, logs, analyzer dumps,
  Python/Cargo caches, and temporary files; prune stale worktrees; verify no
  related process, clean git, origin parity, and free disk space.

## Current unit: exact Function source text

- Parser tokens retain original UTF-8 byte spans. Function declarations,
  expressions, generators, arrows, object/class methods and accessors, and
  classes carry exact source through AST/compiler/VM. Host source is converted
  once to RuJa UTF-16; eval/dynamic internal source is not recanonicalized.
- `Function.prototype.toString` returns exact interpreted source, specified
  `anonymous` dynamic Function source, `[[InitialName]]` NativeFunction syntax,
  or nameless NativeFunction syntax for bound/source-unavailable callable
  exotics. Non-callables throw `TypeError`.
- Test262 runner/analyzer and async harness preserve source bytes and line
  endings. Template TV/TRV still normalizes CR/CRLF independently. Valid
  legacy surrogate-range RegExp patterns rejected by the scalar backend retry
  through the bounded logical UTF-16 backend; resource errors never retry.
- Two GPT-5.6-sol xhigh reviews are complete and closed. Their source
  provenance, arrow span, native initial-name, line-ending, and async harness
  findings are incorporated; both final audits are clean.
- Pinned Test262 toString moves 6/39/35 -> 45/0/35; complete Function moves
  421/39/49 -> 460/0/49. Supported statements/expressions remains
  12765/0/7674 over 20439 files. No admission metadata changed.
- Local gates pass: rustfmt, warnings-denied all-target/all-feature Clippy,
  all-target/all-feature Rust tests including 396 library tests and benchmark
  smoke, generated Intl checks, vendor tests/Clippy/no_std, wasm32, Test262
  tooling 165/165, focused Function, and supported Test262.
- Implementation/docs commit `e227e55 fix(engine): preserve function source
  text` is pushed. Ordinary CI `30655563922` passes 3/3; full matrix
  `30655563999` passes 45/45.
- Against `30648114517`, 30/32 Test262 artifacts are byte-identical. Built-ins
  changes +39/-39; Annex B changes +2/-2 for
  `extended-pattern-char.js` and `legacy-octal-escape.js`. Aggregate is
  33066/4342/11058 with 3 timeout / 0 error over 48469 files and 37408
  completed pass/fail executions. Filename-sorted result hash is
  `3b1011c1a27818967d22907e96052f36b66beec5083d60272e3e707d01c0fa08`.
- Evidence commit `c2bef04 docs(test262): record function source baseline
  [skip ci]` is pushed.
- Final cleanup complete: main `cargo clean` removed 10.1 GiB and vendor clean
  removed 162.8 MiB. Test262, CI artifacts, binaries, targets, logs, analyzer
  dumps, Python/Cargo caches, and stale worktrees are absent. No related
  process remains; HEAD equals `origin/main`, git is clean, and disk has 19 GiB
  free.
- Next narrow candidate: remove the NativeFunction matcher timeout by
  optimizing legacy surrogate ranges, then evaluate exact async/private/Proxy
  toString admission only where the underlying feature boundary is real.
- Mandatory every-turn cleanup: before every final response, close every agent
  and command session; run `cargo clean`; delete `/root/test262`, all binaries,
  CI downloads, targets including vendor targets, logs, analyzer dumps,
  Python/Cargo caches, and temporary files; prune stale worktrees; verify no
  related process, clean git, origin parity, and free disk space.

## Current unit: derived constructor postcondition Realms

- Worktree snapshots the active caller Realm only for interpreted derived
  calls. Primitive-return TypeError and uninitialized-this ReferenceError use
  that Realm; body runtime errors and explicit throws retain callee provenance
  and identity.
- Regressions cover direct and three-Realm calls, foreign Reflect.construct,
  Bound and transparent Proxy constructors, foreign newTarget, body errors,
  explicit throw identity, and forced GC.
- Two GPT-5.6-sol xhigh reviews are complete and agents are closed. Both found
  no blocker; their performance, comment, GC error-kind, wrapper/newTarget, and
  identity recommendations are incorporated.
- Pinned Test262 exact Realm files move 0/2 -> 2/0, Construct moves 2/2/2 ->
  4/0/2, and full Function moves 419/41/49 -> 421/39/49, exactly +2/-2. The
  remaining 39 Function failures are Function.prototype.toString. Supported
  subset remains 12765/0/7674 over 20439 files; no admission changed.
- Local gates pass: rustfmt/diff, warnings-denied all-target/all-feature
  Clippy, all-target/all-feature Rust tests including 396 library tests and
  benchmark smoke, vendor tests/Clippy, wasm32, Test262 tooling 163/163,
  focused/full Function Test262, and supported Test262.
- Implementation/docs commit `a542780 fix(engine): preserve derived error
  realms` is pushed. Ordinary CI `30648115509` passes 3/3 and full matrix
  `30648114517` passes 45/45.
- Against `30643371359`, 31/32 Test262 artifacts are byte-identical and only
  built-ins changes +2/-2. Aggregate is 33025/4383/11058 with 3 timeout / 0
  error over 48469 files and 37408 completed pass/fail executions. The
  filename-sorted result hash is
  `699313f49afb4ba95f54f047375b3dc2e2ee37fa7bec33f50f4e14a5f4c7e091`.
- Evidence commit `d1534e8 docs(test262): record derived realm baseline [skip
  ci]` is pushed.
- Final cleanup complete: main `cargo clean` removed 6.1 GiB and vendor clean
  removed 174.9 MiB. Test262, both CI artifact sets, binaries, targets, logs,
  Python/Cargo caches, and stale worktrees are absent. No related process
  remains; HEAD equals `origin/main`, git is clean, and disk has 19 GiB free.
- Mandatory every-turn cleanup: before every final response, close every agent
  and command session; run `cargo clean`; delete `/root/test262`, all binaries,
  CI downloads, targets including vendor targets, logs, analyzer dumps,
  Python/Cargo caches, and temporary files; prune stale worktrees; verify no
  related process, clean git, origin parity, and free disk space.

## Current unit: configurable Function metadata

- Worktree removes virtual `name`/`length` reconstruction from function
  property reads. Real configurable own descriptors are now the sole observable
  source; deletion exposes ordinary prototype lookup, inherited getter receiver
  semantics, and later redefinition. Internal names remain for diagnostics and
  source rendering. Function `prototype` remains the only virtual fallback.
- Main and Test262-created Realm `%Function.prototype%` objects now have the
  specified empty own `name`.
- Rust regressions cover dynamic, interpreted, and native functions; delete,
  inherited custom getters, missing own descriptors, and redefinition.
- Two GPT-5.6-sol reviewers found no blocker and are closed. They audited every
  production FunctionData constructor, Proxy/delete/define invariants, Realm
  creation, and the need to retain internal diagnostic names.
- Local gates pass: rustfmt/diff check, all-target/all-feature Rust tests,
  warnings-denied Clippy, vendor regress tests and Clippy, wasm32 check,
  Test262 tooling 163/163, exact metadata Test262 4/0, full
  built-ins/Function 419/41/49, and supported subset 12765/0/7674. No admission
  metadata changed. Baseline Function result was 415/45/49, exact delta +4/-4.
- Implementation/docs commit `e93cfd1 fix(engine): preserve deleted function
  metadata` is pushed. Ordinary CI `30643371448` passes 3/3 and full matrix
  `30643371359` passes 45/45.
- Against `30638309791`, 31/32 Test262 artifacts are byte-identical and only
  built-ins changes +4/-4. Aggregate is 33023/4385/11058 with 3 timeout / 0
  error over 48469 files and 37408 completed pass/fail executions. The
  filename-sorted result hash is
  `e3ec3ed7e83a1c9afe88248dc65b713a2aa9a5e417e480af01c7bd022cb43566`.
- Evidence commit `ac822d3 docs(test262): record function metadata baseline
  [skip ci]` is pushed.
- Final cleanup complete: main `cargo clean` removed 5.7 GiB and vendor clean
  removed 174.9 MiB. Test262, both CI artifact sets, temporary binaries,
  targets, logs, Python/Cargo caches, and stale worktrees are absent. No related
  process remains; HEAD equals `origin/main`, git is clean, and disk has 19 GiB
  free.
- Mandatory every-turn cleanup: before every final response, close every agent
  and command session; run `cargo clean`; delete `/root/test262`, all baseline
  and current binaries, CI downloads, target directories including vendor
  targets, logs, analyzer dumps, Python/Cargo caches, and temporary files;
  prune stale worktrees; verify no related process, clean git, origin parity,
  and free disk space.

## Current unit: for-await early errors and AsyncIteratorClose

- Worktree implements for-await Await-context/of-only early errors and real
  asynchronous iterator close using the existing Await continuation path.
- Close getter/call/thenable/rejection/result validation follows original-throw
  precedence. Async-from-sync, Module, async-generator return/throw, same/outer
  continue, next rejection, and partial LHS evaluation have focused tests.
- Finally guards now save start/target/env/clean stack depth. Diversion restores
  state and resolves cleanup trampolines before deciding whether an outer
  finally owns break/continue. Saved environments are explicit GC roots.
- Two GPT-5.6-sol xhigh review rounds are complete and agents are closed before
  finalization. Findings fixed: static-block context, non-of parser forms,
  stack/env restoration, throw-before-validation, internal temp invariants,
  hidden temp retention, GC tracing, and nested outer-finally ownership.
- Local gates pass: rustfmt, warnings-denied all-target/all-feature Clippy,
  all-target/all-feature tests including 396 library tests and benchmark smoke,
  wasm32 check, Test262 tooling 163/163, direct AsyncIteratorClose 5/0,
  for-await 23/0/1211, Module syntax 24/0, and supported Test262
  12765/0/7674. No admission changed; preceding CI binary has the same Test262
  counts because callable asynchronous close ordering is owned by Rust tests.
- Implementation commit `b33df0d fix(engine): implement async iterator close`
  is pushed. Ordinary CI `30638309771` passes 3/3 and full matrix
  `30638309791` passes 45/45.
- Against preceding full run `30631392782`, all 32 Test262 result artifacts are
  byte-identical. Aggregate remains 33019/4389/11058 with 3 timeout / 0 error;
  filename-sorted content hash remains
  `5dfbb2cc10a220e0443a647dfa15ee03607f0e26f17e80696f46d7fd397cc1f4`.
- Evidence commit `b622c27 docs(test262): record async close baseline [skip ci]`
  is pushed.
- Final cleanup complete: `cargo clean` removed 10.1 GiB. Test262, baseline and
  current binaries, all CI downloads, targets, logs, Python/Cargo caches, and
  temporary analysis files are absent. No related process or stale worktree
  remains; HEAD equals `origin/main`, git is clean, and disk has 20 GiB free.
- Mandatory every-turn cleanup: close every agent/command session; run `cargo
  clean`; delete `/root/test262`, baseline/current binaries, CI downloads,
  targets, logs, analyzer dumps, Python/Cargo caches, and temporary files; prune
  stale worktrees; verify no related process, clean git, origin parity, and disk
  space before final response.

## Current unit: Annex B call assignment targets

- Implementation commit `cb080c5 fix(engine): implement Annex B call
  assignment targets` and evidence commit `c2f6093 docs(test262): record call
  target baseline [skip ci]` are pushed.
- Annex B.3.9 admits only sloppy ordinary calls for simple/non-logical compound
  assignment, update, and for-in/of. Strict, Module, logical assignment, and
  optional-chain targets remain early errors. General expressions preserve a
  call's abrupt completion; for-in/of replaces it with ReferenceError before
  iterator close.
- Finally completion state is guard-scoped rather than frame-scoped. Nested
  finally/catch, break/continue propagation, generator yield, async await, GC
  roots, terminal generator cleanup, and nested-function compiler control
  isolation have focused regressions.
- Two GPT-5.6-sol xhigh reviewers are closed. Iterative findings covered loop
  call precedence/no-close, completion nesting, operand roots, stale catch
  guards, multi-finally catch propagation, suspension state, and terminal
  generator retention. Final independent reviews found no blocker.
- Pinned Test262 `9e61c128`: exact B.3.9 **0/7 -> 7/0**, core assignment
  targets **316/0/8**, full Annex B **833/179/74 -> 840/172/74**, supported
  subset unchanged at **12765/0/7674** over 20439 files. No admission changed.
- Local gates pass: rustfmt, warnings-denied Clippy, all-target/all-feature
  tests including 395 library tests and benchmark smoke cases, wasm32, Test262
  tooling 163/163, exact/core/full Annex B, and supported Test262.
- Ordinary CI `30631392802` passes 3/3; full `30631392782` passes 45/45.
  Against `30624215959`, 31/32 artifacts are byte-identical and only Annex B
  changes +7/-7. Aggregate is 33019/4389/11058 with 3 timeout / 0 error;
  filename-sorted hash is
  `5dfbb2cc10a220e0443a647dfa15ee03607f0e26f17e80696f46d7fd397cc1f4`.
- Remaining separate issues: parser accepts `for await` outside async/module
  contexts; general for-await abrupt completion still needs true
  AsyncIteratorClose. Keep these as a dedicated async-iteration unit rather
  than widening B.3.9. Remaining Annex B failures are legacy built-in clusters.
- Final cleanup complete: `cargo clean` removed 10.0 GiB. Test262, baseline
  binary, CI artifacts, targets, Python/Cargo caches, and stale worktrees are
  absent. No related process remains, HEAD matches `origin/main`, and disk has
  20 GiB free.
- Mandatory every-turn cleanup: close every agent/command session; run `cargo
  clean`; delete Test262, baseline binaries, CI downloads, targets, logs,
  analyzer dumps, Python/Cargo caches, and temp files; prune stale worktrees;
  verify no related process, clean git, origin parity, and disk space.

## Current unit: Annex B HTML-like comments

- Implementation commit `12fdaa4 fix(lexer): implement Annex B HTML comments`
  is pushed. It implements Annex B.1.1 HTML-like comments for Script,
  direct/indirect eval, and dynamic Function source. Module tokenization remains
  standard and treats marker bytes as operators.
- Evidence commit `0aabe24 docs(test262): record HTML comment baseline [skip
  ci]` is pushed.
- `<!--` consumes the rest of a Script line from any inter-token position.
  `-->` requires the initial Script goal or an inter-token LineTerminator. A
  separate `html_close_allowed` state prevents string continuation and template
  token newlines from leaking into admission; multiline-comment line
  terminators still enable it.
- Dynamic Function parameter/body boundaries, first-line and prefixed close
  comments, LS/PS, ASI, strict/eval, Module operator fallback, regex/template
  raw text, interpolation, and nested templates have focused Rust coverage.
- Two GPT-5.6-sol xhigh reviews are closed. One found the token-internal newline
  leak and missing literal/Module/eval boundaries; all findings were fixed.
- Previous CI binary `30619831846`: exact HTML cohort **1/13**. Current:
  **14/0**. Five core negative guards remain **5/0**. Full Annex B moves
  **820/192/74 -> 833/179/74**, exactly **+13/-13**. Supported subset remains
  **12765/0/7674** over 20439 files. No admission changed.
- Local gates pass: warnings-denied Clippy, all-target/all-feature Rust tests
  including 394 library tests and benchmark smoke cases, doctest 1/1, wasm32,
  rustfmt, Test262 tooling 163/163, focused and full Annex B, and supported
  Test262.
- Ordinary CI `30624216090` passes 3/3; full `30624215959` passes 45/45. Against
  `30619831846`, 31/32 artifacts are byte-identical and only Annex B changes
  **+13/-13**. Aggregate is **33012/4396/11058**, with 3 timeout / 0 error over
  48469 total and 37408 executed. Filename-sorted hash is
  `afbfc79705afc42c18701784fe23eb65aed8c50488a24553ff3caf71fe272156`.
- Next: push evidence docs, then delete every checkout, binary, target, log,
  cache, and CI download. Remaining Annex B failures should be split into
  legacy assignment-target and built-in clusters.
- Final cleanup complete: `cargo clean` removed 9.1 GiB; Test262, baseline
  binary, CI downloads, targets, logs, Cargo/Python caches, and stale worktrees
  are absent. No related process remains, HEAD matches `origin/main`, and disk
  has 21 GiB free. Repeat this cleanup before every reply.
- Mandatory every-turn cleanup: close every agent and command session; run
  `cargo clean`; delete Test262, CI downloads, binaries, logs, analyzer dumps,
  Python/Cargo caches, targets, and temp files; prune worktrees; verify no
  related process, clean git, origin parity, and disk space.

## Current unit: Annex B catch eval declarations

- Implementation commit `e9e936e fix(engine): implement Annex B catch eval
  semantics` is pushed. It implements Annex B.3.4 for direct eval across a
  matching simple catch parameter. Eval `var` initializers update the catch
  binding while the variable-environment binding remains independently
  installed. Ordinary, generator, async, and async-generator declarations share
  the same conflict walk.
- Evidence commit `c77ecb7 docs(test262): record Annex B catch eval baseline
  [skip ci]` is pushed.
- Object Environment Records and only the matching simple catch parameter are
  ignored. Destructuring catch parameters, nested lexical bindings, and
  function-body top-level lexicals still block. The stop-environment lexical
  check compensates for RuJa storing FunctionBody lexicals and vars in one
  Environment Record.
- Parser early errors now recursively reject destructuring catch parameter vs
  body `var` collisions. Named class expressions no longer masquerade as catch
  body lexical declarations.
- Two GPT-5.6-sol xhigh reviews are closed. They found the function-declaration
  bypass, Object Environment Record handling, destructuring source early error,
  stop-environment lexical conflict, and named class-expression false positive.
- Local gates pass: warnings-denied Clippy, all-target/all-feature Rust tests
  including 393 library tests and benchmark smoke cases, doctest 1/1, wasm32,
  rustfmt, Test262 tooling 163/163, and focused Rust regressions.
- Pinned Test262 `9e61c128`: exact file **0/1 -> 1/0**, complete direct-eval
  directory **309/0**, full Annex B **819/193/74 -> 820/192/74**, supported
  subset unchanged at **12765/0/7674** over 20439 files. No admission changed.
- Ordinary CI `30619831879` passes 3/3; full `30619831846` passes 45/45. Against
  `30615320744`, 31/32 artifacts are byte-identical and only clean Annex B moves
  **+1/-1**. Aggregate is **32999/4409/11058**, with 3 timeout / 0 error over
  48469 total and 37408 executed. Filename-sorted hash is
  `85c4b611b5bd310546f4c7870dee2555d62b2bb7957d8049d29c6d1117c0ab26`.
- Remaining Annex B failures are separate HTML-comment, legacy
  assignment-target, and built-in clusters. Select one narrow cluster after
  cleanup rather than widening this completed declaration unit.
- Final cleanup complete: `cargo clean` removed 8.5 GiB; Test262, both CI
  artifact downloads, targets, logs, Cargo/Python caches, and stale worktrees
  are absent. No related process remains, HEAD matches `origin/main`, and disk
  has 21 GiB free. Repeat this cleanup before every reply.
- Mandatory every-turn cleanup: close all agents and command sessions; run
  `cargo clean`; delete Test262 checkout, CI downloads, binaries, logs, analyzer
  dumps, Python/Cargo caches, target directories, and temporary files; prune
  worktrees; verify no related process, clean git, origin parity, and disk space.

## Current unit: Annex B for-in initializers

- Implementation commit `f633a88 fix(engine): implement Annex B for-in
  initializers` is pushed. Ordinary CI `30615320781` passes 3/3 and full matrix
  `30615320744` passes 45/45. Evidence commit `8cd41f4 docs(test262): record
  Annex B for-in baseline [skip ci]` is pushed.
- Worktree implements B.3.5 for one sloppy `var BindingIdentifier = Initializer`
  in a `for-in` head. Initializer evaluation runs once before RHS through the
  normal Reference/PutValue path; strict, Module, lexical, destructuring,
  multiple-binding, bare assignment, and for-of forms retain early errors.
- `var` loop keys in for-in/for-of now update a resolved binding instead of
  creating a transient let binding. Parentheses restore `+In`. The unused
  PutValue result is popped, preserving derived-constructor abrupt
  iterator-close stack shape.
- Two GPT-5.6 reviews independently found the var loop Reference defect and
  parenthesized `in` gap. Broad Test262 then found the missing Pop via
  `derived-class-return-override-for-of-arrow.js`; root fix and focused Rust
  regression pass. Both agents are closed. One agent ran `cargo clean` while a
  main build was active; main rebuilt and reran every gate afterward.
- Final local gates pass: warnings-denied Clippy, all-target/all-feature tests
  including 393 library tests and benchmark smoke cases, doctest 1/1, wasm32,
  rustfmt, Test262 tooling 163/163, and focused parser/runtime regressions.
- Pinned Test262 `9e61c128`: exact B.3.5 **6/1 -> 7/0**, full Annex B
  **818/194/74 -> 819/193/74**, supported subset remains **12765/0/7674** over
  20439 files. No admission metadata changed.
- CI Annex B had two load-sensitive timeouts. Replacing only that shard with
  the clean exact rerun yields 31/32 byte-identical artifacts versus
  `30610833027`; only Annex B changes by **+1/-1**. Corrected aggregate is
  **32998/4410/11058**, with **3 timeout / 0 error**, and filename-sorted hash
  `268a045ffeffdc7395bac7aad9703f28d80a2e7dbbe8cb13a4f801c5948943a4`.
- Mandatory cleanup complete: `cargo clean` removed 9.5 GiB; Test262, CI
  downloads, baseline/current binaries, analyzer dump, targets, Python/Cargo
  caches, and stale worktrees are absent. No related process remains, HEAD
  matches `origin/main`, and disk has 21 GiB free. Repeat before every reply.
  Next narrow semantic unit is
  Annex B.3.4 catch/eval handling, currently represented by
  `annexB/language/eval-code/direct/var-env-lower-lex-catch-non-strict.js`.

## Current unit: Annex B if-clause functions

- Implementation commit `8535375 fix(engine): implement Annex B if functions`
  is pushed. Ordinary CI `30610833014` passes 3/3 and full matrix
  `30610833027` passes 45/45. Evidence commit `180f037 docs(test262): record
  Annex B if baseline [skip ci]` is pushed.
- Worktree implements Annex B.3.3 by lowering only sloppy ordinary bare-if
  FunctionDeclarations to synthetic Blocks. Strict, Module, generator, async,
  labelled, loop, and `with` forms retain early errors. Existing B.3.2 lexical
  binding and outer mirror semantics are reused unchanged.
- Consecutive labels are parsed iteratively with the statement-depth bound.
  Compiler control frames now distinguish iteration, switch, and non-loop
  labels; a loop frame carries every alias in a label chain, and nested function
  compilation cannot consume the outer statement's pending labels.
- GPT-5.6 reviews found function declaration line loss, recursive-label host
  stack overflow, non-loop label interception of unlabelled break/continue, and
  nested-function pending-label theft. All were fixed; all reviewers are closed
  and created no artifacts.
- Local gates pass: warnings-denied Clippy, all-target/all-feature Rust tests
  including 393 library tests and benchmark smoke cases, doctest 1/1, wasm32
  check, rustfmt, Test262 tooling 163/163, and focused regressions.
- Pinned Test262 `9e61c128`: exact B.3.3 cohort **0/480 -> 480/0**; full
  Annex B **338/674/74 -> 818/194/74**; supported subset remains
  **12765/0/7674** over 20439 files. No admission metadata changed.
- Against full baseline `30606086846`, 31/32 result artifacts are
  byte-identical; only Annex B changes by **+480/-480**. Aggregate is
  **32997/4411/11058**, with **3 timeout / 0 error** over 48469 total and 37408
  executed. Filename-sorted result content hash is
  `0e8c0315babb6e80150923cc8643717b3d07801b22996243aca3e4fa10972ea0`.
- Remaining Annex B failures are split across legacy Date/String/escape/RegExp
  built-ins, HTML comments, legacy assignment targets, and isolated declaration
  rules. Next narrow semantic unit: Annex B.3.5 sloppy `for-in` initializer
  (`annexB/language/statements/for-in/nonstrict-initializer.js`). Keep legacy
  built-ins as separate clusters rather than widening this parser unit.
- Mandatory turn cleanup is complete: `cargo clean` removed 5.9 GiB; Test262,
  CI downloads, targets, logs, caches, temporary files, Python caches, and stale
  worktrees are absent. No related process remains, HEAD matches `origin/main`,
  and the filesystem has 21 GiB free. Repeat this cleanup before every reply.

## Current unit: Annex B outer variable mirrors

- Implementation commit `186b8c9 fix(engine): implement Annex B function
  mirrors` is pushed. Ordinary CI `30606086847` passes 3/3 and full matrix
  `30606086846` passes 45/45. Evidence commit `dde98a9 docs(test262): record
  Annex B mirror baseline [skip ci]` is pushed.
- The current worktree implements Annex B.3.2 outer-variable semantics across
  Script, FunctionBody, direct/indirect eval, switch CaseBlocks, and created
  Realms. Block functions remain lexical; admitted outer vars are hoisted and
  updated only when their declaration is evaluated.
- A source-order `AnnexBDeclarationPlan` keeps actual `VarDeclaredNames`
  separate from legacy candidates. Global/eval declaration instantiation
  filters parameter, `arguments`, lexical, catch-pattern, restricted-property,
  and non-extensible-global conflicts. Same-named simple catch and Object
  Environment Records retain their Annex B exceptions.
- `AnnexBMirror` uses exact function-scope writes for declarative variable
  environments and the shared Environment Reference/PutValue path for Global
  Environment Records. Accessors, non-writable descriptors, and foreign-Realm
  globals therefore follow ordinary global assignment semantics.
- GPT-5.6 review found and drove fixes for global direct-eval intermediate
  lexical suppression, destructuring catch handling, Realm/global descriptor
  routing, deterministic source order, and the pre-existing Annex B labelled
  function hoist regression. Both reviewers are closed and created no
  artifacts.
- Local Rust gates pass: warnings-denied Clippy, all-target/all-feature tests
  including 392 library tests and every benchmark smoke case, rustfmt, doctest
  1/1, wasm32 check, and Python Test262 tooling.
- Pinned Test262 `9e61c128`: exact block/switch cohort **161/135 -> 296/0**;
  full Annex B **203/809/74 -> 338/674/74**; supported subset remains
  **12765/0/7674** over 20439 files.
- Against full run `30602031022`, 31/32 result artifacts are byte-identical;
  only Annex B changes by **+135 pass / -135 fail**. Aggregate is **32517 pass
  / 4891 fail / 11058 skip / 3 timeout / 0 error** over 48469 total and 37408
  executed. Filename-sorted result content hash is
  `4b23c22b5bd00fdc1989c9ff1233ba3fde8fa425301b09941de94b732f500883`.
- Next narrow semantic unit: Annex B.3.3 bare-if FunctionDeclaration parsing
  and synthetic block behavior. Do not fold it into the completed B.3.2 unit.

## Current unit: Annex B duplicate block functions

- Implementation commit: `eb3f14f fix(parser): allow Annex B function
  duplicates` is pushed to `origin/main`. Ordinary CI `30602031029` passes
  3/3 and full run `30602031022` passes 45/45. Against `30598482422`, only the
  Annex B artifact differs: corrected aggregate `32382/5026/11058/3`, exact
  deterministic `+2/-2`, 31/32 byte-identical, filename-sorted content hash
  `23f351811e03b9c36e8b51b9a9025a776967ab9ae18c46cd699f2a0758d00e8d`.
- Parser now allows duplicate lexical names in sloppy Block/CaseBlock only
  when every declaration is an ordinary FunctionDeclaration. Strict,
  generator/async, class/let/const/var mixtures still reject.
- CaseBlock recursive VarDeclaredNames now include `for` init/left declarations.
- Switch BlockDeclarationInstantiation deduplicates the binding and installs
  the source-order last function. Focused Rust regressions and the two pinned
  Test262 files pass 2/2. Annex B is `203/809/74` (`+2/-2`), function-code is
  `37/122`, and supported subset remains `12765/0/7674`.
- Final local gates pass: all-target/all-feature Rust tests including benches,
  Clippy `-D warnings`, rustfmt, doctest, wasm check, and Python tooling 163/163.
- Next semantic unit must implement the full Annex B outer variable mirror:
  block lexical binding plus conditional outer var hoist, copy only when the
  declaration is evaluated, parameter/top-level lexical conflict suppression,
  and ordinary functions only across Function/Global/Eval paths.
- Shared-workspace rule: close all read-only subagents before main starts any
  build/Test262/CI command. A subagent's mandatory cleanup must never delete
  shared `target` or Test262 paths while a main command is active.

## Current unit: declaration static semantics

- Implementation commit: `3b2dd8b fix(engine): align function declaration
  instantiation` (pushed to `origin/main`). Ordinary CI `30598482443` passes
  3/3; full matrix `30598482422` passes 45/45. Final evidence commit:
  `833437a docs(test262): record declaration baseline [skip ci]`.
- Remaining 43 dual-variant failures were one parser bug: strict Script
  FunctionDeclaration was wrongly lexical. Script/FunctionBody are now
  var-scoped; Module/Block remain lexical.
- Focused Test262 dynamic-import + function cohort: `1072/0/384`.
- Supported subset on `9e61c128`: `12765 pass / 0 fail / 7674 skip / 20439`.
- GPT audit also exposed and drove fixes for missing FunctionBody lexical/var
  intersection checks, parameter-expression body var environments, and
  user-visible `__argN` collisions in destructuring parameter carriers. The
  carriers now use the non-source `#argN` namespace.
- Full artifacts, corrected with the clean exact-corpus Annex B rerun, total
  `32380 pass / 5028 fail / 11058 skip / 3 timeout`, 48,469 total and 37,408
  executed. Deterministic delta from `30594697340`: `+54 pass / -54 fail`;
  26/32 byte-identical. Corrected content hash:
  `fec150f12df228289b84453363a62c41fbfe82dd9b15592b20c7f1c2c68ee8f0`.
  Delete this turn's Test262 checkout, targets, logs, and artifacts after
  recording the final docs commit.
- Final local gates pass: Rust all-target/all-feature including benches,
  Clippy `-D warnings`, rustfmt, doctest, wasm check, Python tooling 163/163,
  focused Test262 `1072/0/384`, supported subset `12765/0/7674`.
- Next narrow parser backlog from GPT review: Annex B sloppy Block/CaseBlock
  may permit duplicate ordinary FunctionDeclarations, while strict blocks and
  generator/async/lexical mixtures must still reject. Keep separate from this
  source-goal unit.

## Current unit: Test262 strict-variant fidelity

- Commits: `22ad0e3` runner fidelity, `e5abb7b` destructured-var fix,
  `bdbf57c` final CI/artifact documentation (`[skip ci]`).
- Final CI: ordinary `30594697312` success; full `30594697340` success (45/45).
- Full artifacts: `32326 pass / 5082 fail / 11058 skip / 3 timeout / 0 error`,
  48,469 total and 37,408 executed; 26/32 byte-identical to pre-dual run
  `30589502837`. Sorted content hash:
  `e7d63136e8cfbb994bd07632fc0d4b0c2e6a64170e9bdeb667e81e7dbfbafb1d`.
- Local final verification: Rust all-target/all-feature including benches,
  Clippy `-D warnings`, rustfmt, doctest, wasm check, Python tooling 163/163,
  supported subset `12722/43/7674`, loop cohort `676/0/190`, Intl.Collator
  `74/74`. All generated outputs were removed before handoff.

- Default Test262 files now execute non-strict then strict in independent
  processes; file-level counts remain stable.
- Current CI pin `9e61c12835c5e4a3bdba93850427e6742c4f64c4` supported subset now reports
  `12722 pass / 43 fail / 7674 skip / 20439 total`, correctly replacing the
  former inflated `12765/0` claim.
- GPT-5.6 reviewers confirmed the core semantics and identified analyzer
  timeout/error omission, raw labels, timeout coverage, module+raw coverage,
  and file-count coverage; all were incorporated.
- Failure-cluster analysis is complete for this unit. Do not widen
  admission to hide the remaining 43 strict failures; use them as the next
  engine-fix backlog after this runner-fidelity commit.
- The runner initially exposed 89 strict-only failures. The `var` destructuring
  compiler fix closes all 45 `statements/for-of` and the one
  `statements/for-in` failure. The remaining 43 are 37
  `expressions/dynamic-import` and 6 `statements/function` duplicate-declaration
  syntax errors.
- Full CI `30593166734` exposed three of the 45 strict `for-of` failures in the
  Intl.Collator exact gate. Root cause was `var` patterns using lexical
  initialization instead of assignment to their hoisted variable binding.
  The compiler fix and strict global/function/object/array/default regression
  pass; supported subset is now `12722/43/7674`.

> **절대 규칙: 매 사용자 턴마다 답변 전에 해당 턴에서 생성·재사용한 모든
> 산출물을 삭제한다.** 완료 여부와 무관하며 `target`, Test262 checkout/결과,
> CI 다운로드, 벤치 결과, 로그, 임시 파일·worktree, Python 캐시, 생성 lockfile,
> 이번 턴에서 내려받거나 재사용한 도구 캐시를 다음 턴으로 넘기지 않는다.
> 실행 중인 명령과 서브에이전트를 먼저 종료한 뒤 `cargo clean`, 프로세스·Git
> 상태·남은 디스크 확인까지 끝내야 해당 턴이 완료된다.
> 이 규칙은 구현·검증·문서·상태 보고·중단·자동 계속 턴 모두에 적용하며,
> 메인 에이전트와 모든 서브에이전트는 자기 턴에서 만든 산출물을 그 턴의
> 답변 또는 제어권 반환 전에 직접 삭제한다. 다음 턴의 정리에 맡기지 않는다.
> **문서만 수정하거나 상태만 답하는 턴도 동일하다. 매 턴마다 반드시 지운다.**

## Goal

Continue the active long-running goal of turning RuJa into a substantially
complete JavaScript engine. Close specification behavior rather than only
runner gaps. Every narrow unit includes focused and broad tests, documentation,
rustfmt, warnings-denied Clippy, exact Test262 evidence, a clean commit and
push, ordinary CI, full-matrix verification, and artifact aggregation.

The overall goal remains active. Do not mark it complete after one conformance
family.

**한 줄 규칙: 매 턴 답변 직전에 빌드·테스트·벤치·CI·캐시·임시 산출물을
전부 지우고, 프로세스 종료와 남은 디스크 공간까지 확인한다.**

**매 턴 필수:** 답변 전에 해당 턴에서 만든 산출물을 모두 삭제한다. 구현이
끝나지 않았거나 상태만 보고하는 턴도 예외가 없다.

**Non-negotiable turn rule:** every agent must delete all reproducible build,
test, CI, benchmark, cache, log, and temporary artifacts before every reply,
including status-only, documentation-only, interrupted, and incomplete turns.
This deletion is mandatory on **every turn**, not only at milestone or commit
boundaries; write the compact evidence first, then remove the generated output.
Treat each user request as a fresh cleanup obligation: artifacts created or
reused during that request must be deleted before its reply and must never be
kept for reuse by the next turn, even when work on the same unit continues.

**Reply gate, every turn without exception:** before sending any final or
status reply, stop all local and subagent commands, run `cargo clean` in the
root and every nested Cargo workspace, delete Test262/CI/benchmark/log/cache/
temporary output created or reused that turn, remove Python caches and
generated vendor lockfiles, prune stale worktrees, verify no matching process
remains, and report free disk space. Source files, ignored handoff text, and
compact SHA/count evidence are the only permitted carry-over. Never keep a
build target or Test262 checkout merely because the same unit continues next
turn.

Subagents are available. Prefer the strongest GPT-5.6 model for primary code
review and use Umans-provided GLM/Kimi only as complementary diversity when
useful; Umans is a provider, not a model. The coder model is lower priority.
Agents remain read-only unless a disjoint edit scope is explicitly assigned.
Use the `caveman` skill in lite mode for concise user updates without reducing
technical detail or verification depth.

Disk space is constrained. Cleanup is a mandatory end-of-turn gate on every
turn, including incomplete, status-only, and documentation-only work. Before
the final response for each turn, delete downloaded CI artifacts, copied
release binaries, completed benchmark output, runner logs, Python caches,
generated vendor targets, and stale Test262 worktrees, then run `cargo clean`.
Keep only compact SHA/count evidence when later comparison needs it. Never use
a broad `/tmp` name-pattern cleanup while a Test262 runner is active: its live
temporary directories use `ruja-test262-*`. Preserve the current pinned
worktree and any files required by an in-flight command, then remove them
immediately after the command completes.

Artifact cleanup is per turn, not per completed implementation unit. Never
defer generated-output cleanup to a later turn or leave it for the next agent.
Every reply is a cleanup boundary: no generated artifact may cross into the
next turn, even when the same agent immediately continues the same task.
Here, a turn means one user request through the agent's final response; each
continuation starts a new cleanup obligation even when the work unit is unchanged.

Do not carry downloadable artifacts, build targets, benchmark output, runner
logs, caches, or temporary worktrees into the next turn. Rebuild only what the
next turn needs.

Mandatory cleanup checklist for **every agent turn**:

1. Stop or finish every command that owns generated files; never clean under a
   live runner.
2. Delete that turn's CI downloads, copied binaries, benchmark results, Test262
   output/worktrees, logs, caches, temporary files, and other generated
   artifacts as soon as they are no longer needed.
3. Run `cargo clean` before the final response, even when the turn failed, was
   interrupted, only reported status, or changed documentation only.
4. Confirm remaining disk space and inspect the worktree for stray generated
   files. Source edits and compact evidence may remain; reproducible build and
   test output may not.

A turn is not complete until this checklist passes. Record any cleanup blocker
in the final response and remove it at the first safe point instead of silently
carrying artifacts into another turn.

This rule applies after every continuation of the long-running engine goal:
each agent must remove its own generated outputs before yielding or replying,
even when another agent or later turn will continue the same implementation
unit. No session may rely on the next session to clean its artifacts.

## Completed unit - exact SharedArrayBuffer and WeakRef class subclass admission

The final four implemented `class/subclass-builtins` cases now use one shared
exact path-to-feature map: declaration and expression forms for
SharedArrayBuffer and WeakRef. Runner and analyzer remove only the matching
single feature for those rows. Future siblings, outside paths, and extra
unsupported features remain skipped. Full-matrix setup runs the checkout-backed
live metadata tests before matrix fan-out.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4` passes exact
**4/4**, complete statement/expression subclass-builtins **72/72**, both class
trees **4190 pass / 0 fail / 4236 skip**, and supported language **12765 pass /
0 fail / 7674 skip** over 20439 files. Tooling is **154/154** on the complete
checkout. Debug all-target/all-feature Rust tests, release library **392/392**,
Clippy `-D warnings`, rustfmt, wasm32, doctest **1/1**, Python compilation, and
workflow YAML parsing pass.

The initial forced audit executed all 4240 currently skipped class files and
found **4240/4240 passing**. A proposed `class/dstr` prefix admission was fully
removed after both GPT-5.6 reviewers found it open-ended. More importantly,
the local Test262 runner executes an unqualified file only once rather than in
both default strict and sloppy variants, so that broad corpus cannot yet be
claimed as complete Test262 evidence. Next class unit should first add
strict/sloppy variant execution or freeze exact variant-aware metadata; raw
module `await` class-name negatives are a separate exact two-file candidate.

Implementation commit `8039532` and inaccessible-default-checkout follow-up
`2307de6` are pushed. The first ordinary run exposed the live metadata test's
uncaught `PermissionError`; the follow-up uses the same unavailable/inaccessible
checkout handling as established live tests. Latest ordinary CI `30589502849`
passes **3/3**, and full run `30589502837` passes **45/45**, including the new
pinned-checkout setup preflight.

Against preceding full run `30584440297`, **30/32** result artifacts are
byte-identical. Expressions move **7863/0/3239 -> 7865/0/3237** and statements
move **4898/0/4439 -> 4900/0/4437**, exactly the four declaration/expression
subclass files. Aggregate moves **32376 pass / 5028 fail / 11062 skip / 3
timeout / 0 error** to **32380 / 5028 / 11058 / 3 / 0** over 48469 files and
37408 executed. Sorted concatenated current result content hashes to
`0adac9b45ba16c186b359aa70f16cec591c45b5adcb727ee74a9bccc992d873e`.
The superseded in-flight full run for the first commit was canceled after the
follow-up push to avoid duplicate CI work.

## Completed unit - bounded VM-local RegExp matcher reuse

Commit `cac8fbe` (`perf(regexp): cache bounded compiled matchers`) is pushed on
`main`. RegExp construction, builtin exec, and internal String routes now share
one VM-local semantic LRU keyed by immutable source, `i/m/s/u/v`, and the
scalar-preferred / UTF-16-code-unit / logical-UTF-16-required input domain.
`d/g/y` and Realm identity share entries. Lookup remains after input ToString,
lastIndex coercion, and global/sticky out-of-range return; compile errors and
the terminal test failure are never cached. Publication reservation failure is
best-effort and cannot replace successful compilation with an error.

Every RuJa `CompiledRegex` backend field owns an Arc, leaving vendor Clone and
atomics-free no_std behavior unchanged. Cache hits and active-call copies are
allocation-free. Checked LRU accounting caps 16 entries, 256 KiB retained
source, 64 KiB per source, and 128 MiB conservative matcher charge. Rust
builders explicitly retain the existing 10 MiB NFA and 2 MiB lazy-DFA limits;
only capture-free sources up to 4 KiB are admitted and consume the whole 128
MiB budget. Regress reports a conservative logical-program charge. Fancy,
composite, captured, large, overflowed, and oversized matchers execute normally
but skip caching because their total retained scratch is not publicly bounded.

Tests cover all semantic key bits, d/g/y sharing, all input domains, constructor
seeding, separate VMs, cross-Realm and GC reuse, cache-hit coercion ordering,
callback-time reentrant eviction, every backend admission decision, LRU/source/
matcher limits, best-effort publication retry, and terminal compile-failure
ordering. Local gates pass all-target/all-feature debug library **392/392**,
release library **392/392**, every integration and benchmark smoke target,
warnings-denied Clippy, rustfmt, Rust 1.88 MSRV, wasm32, tooling **152/152**
with four expected no-checkout skips, and vendored regress **38/38**, Clippy,
and no_std. Complete RegExp Test262 remains **1223/0/656** and RegExp literals
remain **236/0/2**. The focused Criterion smoke is **86.797-91.599 ms** for
10,000 cached execs; treat this as execution evidence, not a comparative claim.

GPT-5.6 review first rejected underbounded Rust/fancy scratch accounting and
vendor-wide Arc Clone changes. Those changes were removed in favor of the
conservative admission above. Semantic review drove explicit m/s/u/v,
cache-hit ordering, and reentrant eviction tests plus the Decision Log. Final
production audit is CLEAN, with only a required re-audit on future regex crate
upgrades. All agents are closed.

Ordinary CI `30584440575` passes **3/3** and full CI `30584440297` passes
**45/45**. All 32 matrix result artifacts are byte-identical to preceding run
`30576696665`, with aggregate content SHA-256 `abdfe77d...`. Counts remain
**32376 pass / 5028 fail / 11062 skip / 3 timeout / 0 error** over 48469 files
and 37404 pass-or-fail runs. Both downloaded artifact sets, the local Test262
checkout/results, benchmark output, generated vendor lockfiles, Python caches,
and every Cargo target were deleted. `cargo clean` reclaimed about 10.8 GiB.

## Completed unit - RegExp exec terminal compilation boundary

`RegExpBuiltinExec` now has one terminal compiler helper for Unicode/input-
sensitive and non-Unicode routes. A VM-local `cfg(test)` countdown can replace
only a successfully compiled terminal matcher with a typed Resource failure.
Real syntax/resource errors leave it armed, and Rust, fancy, compiled-size
fallback, logical UTF-16, and Unicode primary-to-logical fallback variants are
asserted directly. Injection occurs before backend-input preparation, capture
metadata, matching, lastIndex publication, result materialization, or future
cache insertion.

Ordering tests cover input and lastIndex coercion, global/sticky out-of-range
reset, non-writable reset, input-time lastIndex mutation, non-global huge
lastIndex, main/foreign method Realms, nested countdowns in both directions,
Fuel/materialization priority, unchanged state, and immediate retry without
manual repair. `RegExp.prototype.test` now follows dynamic `RegExpExec`, so
input ToString precedes custom exec lookup, callable overrides receive the
right receiver/string, primitive results throw TypeError, and branded fallback
remains available for a non-callable override.

Local gates pass all-target/all-feature library **388/388**, every integration
target and benchmark smoke, release library **388/388**, rustfmt,
warnings-denied Clippy, Rust 1.88 MSRV, wasm32, generated Intl checks, tooling
**152/152** with four expected sparse-checkout skips, and vendored regress
**38/38** plus 14 doc-tests, Clippy, and no_std. Pinned Test262 complete
`built-ins/RegExp` is **1223/0/656** and RegExp literals are **236/0/2**.

Two GPT-5.6 review rounds found and drove closure of pre-backend injection,
genuine-error masking, synthetic Syntax injection, Realm conflation, repaired
retry state, missing lastIndex branches, fallback coverage, custom-exec scope,
inverse nested failure, and direct backend assertions. Final review is
`CLEAN`, and all agents are closed.

Commit `be0c906` (`fix(regexp): harden exec compilation boundary`) is pushed
on `main`. Ordinary CI `30576696658` passes **3/3** and full run
`30576696665` passes **45/45**. All 32 matrix result artifacts are
byte-identical to prior run `30571288316`; aggregate remains **32376 pass /
5028 fail / 11062 skip / 3 timeout / 0 error** over 48469 files with 37404
pass-or-fail runs.

The hook proves ordering but does not make vendor allocation fallible.
Compiled-matcher ownership/caching, compiler allocation failpoints, and
capture-metadata allocation remain next units.

## Completed unit - RegExp compiler/backend resource error typing

The active change introduces typed `Syntax` versus `Resource` results across
dynamic RegExp validation, Rust regex, fancy-regex, and the vendored regress
backend. Flags now validate before the allocation-avoiding Unicode source cap;
dynamic construction maps implementation limits to Realm-correct `RangeError`
while malformed syntax remains `SyntaxError`. All constructor, exec, search,
match, and legacy replacement compile callers use one mapper. Fancy and
logical runtime work limits are non-catchable Fuel aborts. Successful alternate
backend compilation remains a transparent fallback.

`RegExpBuiltinExec` now observes `lastIndex` and performs its out-of-range
reset/null path before compilation, capture metadata, and backend-input
preparation. The numeric comparison precedes host-width conversion. Focused
tests currently pass for typed adapters, local/foreign/cross-Realm errors,
Unicode property and string-set limits, Rust/logical/fancy nesting, syntax,
flag priority, and retry. Two GPT-5.6 audits identified and drove closure of
v-validator type erasure, missing regress guards, flag priority, Realm
coverage, literal resource-token propagation, and exec ordering. Deterministic
exec compile-failure injection, matcher caching, and compiler allocation
failpoints remain later units.

Local gates pass all-target/all-feature library **385/385**, every integration
target and benchmark smoke, release library **385/385**, rustfmt,
warnings-denied Clippy, Rust 1.88 MSRV, wasm32, generated Intl checks, tooling
**152** tests with four expected sparse-checkout skips, and vendored regress
**38/38** plus doc-tests, Clippy, and no_std. Pinned Test262 UnicodeSets is
**142/0/0**, complete built-ins/RegExp is **1223/0/656**, complete RegExp
literals is **236/0/2**, named exact is **86/0/0**, and related scope is
**100/0/1**.

Final GPT-5.6 reviews drove closure of validator type erasure, untyped regress
guards, invalid-flag priority, Realm coverage, literal resource-token
propagation, alternate slash parsing, full IdentifierPart flag scanning, exec
ordering, and stale documentation. The final code review is clean; its one
documentation-only finding was fixed before the final gates.

Commit `7416b5b` (`fix(regexp): type compiler resource errors`) is pushed on
`main`. Ordinary CI `30571287782` passes **3/3** and full run `30571288316`
passes **45/45**. All 32 matrix result artifacts are byte-identical to prior
run `30563774955`; aggregate remains **32376 pass / 5028 fail / 11062 skip /
3 timeout / 0 error** over 48469 files with 37404 pass-or-fail runs.

Deterministic exec compile-failure injection, matcher caching, compiler
allocation failpoints, and capture-metadata allocation remain later units.

## Completed unit - RegExp replacement native-container materialization

Builtin `RegExp.prototype[Symbol.replace]` now fallibly reserves and meters
seven owned native phases: non-ASCII input UTF-16 cache, collected results,
captures, callback arguments, static-substitution scratch, final UTF-16 output,
and exact UTF-8 decoding. ASCII input stays borrowed; non-ASCII input is
encoded once. Empty-match advancement uses checked `u64`, and source slicing
shares the cached representation. Replacement parsing streams to reusable
UTF-16 scratch and prepays template scan work. Repeated `exec` calls share the
input `Arc<str>` instead of copying it per match.

Result reserve/pin/append precedes global empty-match handling. All results are
collected before callbacks. Backward matches still observe capture getters,
named substitution, and callbacks before output suppression. Existing string
payloads stay shared. ToString-created payloads, final result `Arc<str>`,
dynamic named-group PropertyKeys, compiler/backend metadata, vendor matcher
storage, and legacy String paths remain separate runtime-wide OOM units.

Typed actual-growth failpoints cover all seven phases, every substitution and
final-output append source, no-match/zero-capture/static/functional/empty
bypasses, active and foreign Realm errors, pin/live cleanup, countdowns, and
immediate retry. Exact event logs freeze result/capture/groups/callback/named
ToString ordering. UTF-16 parity includes malformed and sentinel-collision
sequences with exact decoded byte-length assertions. Three exact Fuel fixtures
cover static source tokens, named substitution, and functional replacement.

Final local gates pass: all-target/all-feature library **384/384**, every
integration target and benchmark smoke, release library **381/381**, rustfmt,
warnings-denied Clippy, Rust 1.88 MSRV, wasm32, generated Intl checks, tooling
**152** tests with four expected sparse-checkout skips, and vendored RegExp
**38/38** plus Clippy/no_std. Pinned Test262 replacement is **68/0/2**, complete
built-ins/RegExp is **1223/0/656**, named exact is **86/0/0**, and related
scope is **100/0/1**. Two GPT-5.6 reviews found and drove closure of repeated
input Arc copies, source/template scans, wasm32 arithmetic, branch coverage,
observable ordering, exact decode reservation, and documentation scope. Final
re-reviews are `CLEAN`, and both agents are closed.

Commit `42bd3d3` (`fix(regexp): harden replacement materialization`) is pushed
on `main`. Ordinary CI `30563774957` passes **3/3** and full run
`30563774955` passes **45/45**. All 32 matrix result artifacts are byte-identical
to preceding run `30556614952`; aggregate remains **32376 pass / 5028 fail /
11062 skip / 3 timeout / 0 error** over 48469 files with 37404 pass-or-fail
runs.

Every generated target, sparse Test262 checkout, downloaded current/preceding
CI artifact, vendor lock/target, Cargo registry cache, Python cache, and log is
deleted before reply. The next unit should address a narrow remaining common
allocation owner, preferably fallible JS-string/property-key publication or
typed compiler/backend RegExp resource errors, without broadening this unit's
claim.

## Completed unit - RegExp post-match native-container materialization

Commit `3dacfac` (`fix(regexp): harden exec result materialization`) is pushed
on `main`. Builtin `RegExp.prototype.exec` now makes every owned post-match
native container fallible and Fuel-metered before growth: capture ranges,
endpoints, UTF-16 offset maps, result values and presence bitmaps, named string
groups, match-indices values/pair Arrays/presence/groups, outer indices
properties, and final exec-result properties. Named-group maps are completed
locally before heap allocation; duplicate names keep their first key position
while a later participating capture replaces only an earlier `undefined`.

Global/sticky `lastIndex` now publishes match zero's UTF-16 end before result
materialization, matching `RegExpBuiltinExec`; later Range or Fuel failure does
not roll it back. Fuel precedes reservation, charges endpoint sorting/input
scans/string candidates/Array presence/property publication, and includes the
full byte length of every hashed capture name. The hardened result-property
wrapper is builtin-exec-only; the excluded legacy String match path retains
its previous shared publisher semantics.

One deterministic 13-site failpoint matrix covers every reservation and every
participating pair countdown. It requires active-Realm `RangeError`, published
`lastIndex`, pin/live-count restoration, immediate retry, pair countdown-four
success, no-match and complete non-`d` bypass, unnamed groups behavior,
duplicate key order, indices identity, and early/nested/final foreign-Realm
failures. Exact Fuel tests prove zero-Fuel priority, `required - 1` failure,
the exact success boundary, and both groups maps charging a 4,096-byte name.
Two GPT-5.6 final reviewers are closed and `CLEAN`.

Local gates pass all-target/all-feature library **381/381**, every integration
target and **35** benchmark smokes, warnings-denied Clippy, rustfmt, Rust 1.88
MSRV, wasm32, release plus Realm rollback, generated Intl data, tooling
**152/152** with four expected absent-checkout skips, and vendored RegExp
**38/38** plus warnings-denied Clippy/no_std. Pinned Test262 remains exact:
named groups **86/0/0**, related scope **100/0/1**, and complete supported
RegExp policy **1223/0/656** over 1879 files with no timeout.

Ordinary CI `30556614944` passes **3/3**. Full run `30556614952` passes
**45/45**. All 32 matrix result artifacts are byte-identical to temporary-root
baseline `30547236070`; aggregate remains **32376 pass / 5028 fail / 11062
skip / 3 timeout / 0 error** over 48469 files with 37404 pass-or-fail runs.

Next RegExp resource units remain separate. Prefer replacement result/capture/
callback-argument/output containers and streaming substitution first. Then
introduce typed syntax-versus-resource errors across capture-name/compiler and
backend capture conversions, harden the pre-match input boundary table, and
cache compiled matchers plus capture metadata. String/Arc payload allocation,
vendor matcher allocation, and legacy String paths are not claimed by this
unit.

**Mandatory every-turn cleanup remains active:** before every reply, including
status-only and documentation-only replies, stop commands and agents, run root
and nested `cargo clean`, delete Test262 checkouts/results, CI downloads,
release binaries, logs, Python caches, generated lockfiles, and downloaded
Cargo/tool caches, then verify processes, Git state, and free disk space. No
artifact from this unit may cross into the next turn.

## Completed unit - RegExp fallible temporary roots

Commit `3e78a13` (`fix(regexp): make temporary roots fallible`) is pushed on
`main`. Every RegExp-core temporary heap root now reserves exact native root
capacity before publication. Single-value and atomic multi-value helpers cover
constructor, search, match, matchAll, split, replacement, RegExp string
iterators, `toString`, builtin exec, groups, and result arrays. Previously
missing roots now retain fresh `@@search` lastIndex/exec results, `@@match`
flags/results/captures/lastIndex, and `toString` source/flags across later
observable re-entry. Match-indices preflights every future participating pair
plus its optional groups object before the first nested allocation.

Two direct regressions force GC across fresh getters, setters, custom exec,
coercions, captures, and restoration, then sweep countdown failures through
every reachable native and generic object-valued reservation branch. The
sweeps cover constructor source/flags, matchAll flags/lastIndex, split
lastIndex/length/captures, empty match/iterator advancement, replacement
lastIndex/length/index/groups and functional callbacks, exec groups/indices,
pin cleanup, failed-partial GC, and exact retry. A getter-armed foreign-Realm
probe proves failure occurs before coercion and materializes only the method
Realm's `RangeError`. Two GPT-5.6 final reviewers are closed and `CLEAN`.

Local gates pass all-target/all-feature library **380/380**, every integration
target and benchmark smoke, release, warnings-denied Clippy, rustfmt, Rust 1.88
MSRV, wasm32, generated Intl data, tooling **152/152** with four expected
absent-checkout skips, vendored RegExp **38/38** plus exact CI Clippy/no_std,
named-group exact **86/0/0**, related scope **100/0/1**, and complete supported
RegExp policy **1223/0/656** over 1879 files.

Ordinary CI `30547236122` passes **3/3**. Full run `30547236070` passes
**45/45**. All 32 matrix result artifacts are byte-identical to named-group
baseline `30541663201`; aggregate remains **32376 pass / 5028 fail / 11062
skip / 3 timeout / 0 error** over 48469 files with 37404 pass-or-fail runs.

Next RegExp resource units remain separate: make capture boundary/name/groups
native containers fallible and metered; stream replacement substitution before
large temporary copies; then cache compiled matcher/capture metadata instead
of recompiling every exec. Prefer the smallest complete container family first.

**Mandatory every-turn cleanup remains active:** before every reply, including
status-only and documentation-only replies, stop commands and agents, run root
and nested `cargo clean`, delete Test262 checkouts/results, CI downloads,
release binaries, logs, Python caches, generated lockfiles, and downloaded
Cargo/tool caches, then verify processes, Git state, and free disk space. No
artifact from this unit may cross into the next turn.

## Completed unit - RegExp named capture exact admission

Commit `96d5967` (`test(regexp): admit named capture groups`) is pushed on
`main`. Runner and analyzer now share an exact 86-file `regexp-named-groups`
admission: 26 built-in named-group files, eight named replacement files, and
52 RegExp-literal files. It is disjoint from the existing seven match-indices
and 19 duplicate-name manifests. Future siblings remain skipped, and
`poisoned-stdlib.js` remains the related scope's single explicit skip because
its independent `Symbol.iterator` dependency was not lifted.

The pinned exact boundary passes **86/0/0** and the related 101-file scope is
**100 pass / 0 fail / 1 skip**. Local gates pass tooling **152/152**, library
**378/378**, all-target/all-feature integration and benchmark smoke, release,
rustfmt, warnings-denied root Clippy, Rust 1.88 MSRV, wasm32, generated Intl
data checks, and vendored RegExp **38/38** plus the workflow's exact Clippy and
`no_std` feature combinations. Two GPT-5.6 final reviewers returned `CLEAN`
and are closed.

Ordinary CI `30541663008` passes **3/3**. Full run `30541663201` passes
**45/45**, including the new `regexp-named-groups` job. Across the 32 matrix
artifacts, built-ins changes by **+34 pass / -34 skip** and language/literals
by **+52 / -52**, exactly the frozen 86-file boundary. Annex B independently
changed two previous timeouts into passes with no engine-code change, so the
aggregate is **32376 pass / 5028 fail / 11062 skip / 3 timeout / 0 error** over
48469 files, with 37404 pass-or-fail runs. The policy-attributable delta from
`30537085489` is exactly **+86 pass / -86 skip**.

Next runtime-hardening candidates found by the audit are intentionally separate:
cache compiled matchers/capture metadata instead of recompiling every `exec`;
preflight GC-pin and native capture/group storage growth; and stream
replacement substitution with pre-copy Fuel/output checks. Start with the
narrowest independently testable allocation path, not all three at once.

**Mandatory every-turn cleanup remains active:** before every reply, including
status-only and documentation-only replies, stop commands and subagents, run
root and nested `cargo clean`, delete Test262 snapshots/results, CI downloads,
release binaries, logs, Python caches, generated lockfiles, and downloaded
Cargo/tool caches, then verify processes, Git state, and free disk space. No
generated artifact from this unit may cross into the next turn.

## Completed unit - `WeakRef` and `FinalizationRegistry`

Commit `b69643c` is pushed on `main`. WeakRef and FinalizationRegistry now use
immutable Realm-local prototype registries and GC-retry allocation. WeakRef
job-kept object identities live in a fallible O(1) set. FinalizationRegistry
stores and traces its constructor Realm, enters it for cleanup jobs, publishes
one sweep-time pending bit instead of rescanning every native cell vector, caps
and meters cell operations, reserves initial callback dispatch roots before
cell removal, invokes cleanup cellwise outside the registry lock, observes
callback-time unregister, contains catchable callback abrupt completions, and
propagates non-catchable Fuel without replaying the selected cell. Scheduling
and native-container growth are fallible and retryable.

The exact pinned boundary freezes **29 WeakRef + 47 FinalizationRegistry**
paths and complete metadata. Both the exact manifest and complete two-directory
scope pass **76/0/0** locally and in the dedicated full-workflow gate; future
siblings are scope-closed. Direct regressions cover immutable foreign-Realm
fallback, independent constructor/job/new-target Realms, GC retry at exact heap
caps, kept-root/cell/cleanup reservation failure, callback dispatcher preflight,
pending preservation, callback-time unregister, catchable throw, Fuel abort,
pin balance, and recovery.

Local gates pass all-target/all-feature library **378/378**, every integration
target and benchmark smoke, tooling **151/151** with four expected
absent-checkout skips, generated-data checks, rustfmt, warnings-denied root and
vendored Clippy, vendored RegExp **38/38**, vendored `no_std`, wasm32, Rust 1.88
MSRV, release Realm rollback, workflow YAML parsing, and release Test262 exact
and full-scope **76/0/0**. Two GPT-5.6 final reviewers are closed and `CLEAN`.

Ordinary CI `30537085389` passes **3/3**. Full run `30537085489` passes
**43/43**, including `weak-references`. All 32 historical matrix artifacts are
byte-identical to Collator run `30501347064`; direct artifact aggregation is
**32288 pass / 5028 fail / 11148 skip / 48469 total / 37316 ran**. This corrects
the prior handoff prose, which overstated pass and ran by two despite the
underlying artifacts being unchanged. The overall complete-engine goal remains
active; this completes only the weak-reference unit.

Before every later reply, including status-only replies, delete every generated
artifact again: root and nested targets, Test262 checkout/results, CI downloads,
Python caches, generated vendor lockfiles, logs, temporary files/worktrees, and
downloaded or reused Cargo/tool caches. Finish commands and agents first, run
`cargo clean`, then verify processes, Git status, and free disk space.

## Completed unit - `Intl.Collator`

Commit `2c327c9` is pushed on `main`. Every Realm owns a distinct
callable/constructible constructor and prototype. Instances have immutable
internal slots backed by
ICU4X 2.2 collation data, cached bound `compare`, ordered `resolvedOptions`,
locale/Unicode-key negotiation for `co`, `kf`, and `kn`, supported locale
filtering, GC tracing, subclass/new-target behavior, and function-Realm error
semantics. `String.prototype.localeCompare` constructs the method Realm's
intrinsic Collator, so global constructor tampering does not affect it.

The exact pinned Test262 admission boundary is **74/0/0**. Whole Collator scope
is **74 pass / 0 fail / 1 skip**; the one held file requires other absent Intl
service constructors through shared harness code. `supportedValuesOf` advances
to **16/0/0**, with whole scope **16 pass / 0 fail / 9 skip**, because the
runtime now publishes ten collations validated against compiled ICU data.
Local gates pass library **375/375**, all integration/benchmark/example targets,
Intl **17/17**, tooling **150/150** with four expected absent-checkout skips,
rustfmt, warnings-denied Clippy, wasm32, Rust 1.88 MSRV, vendored RegExp
**38/38** plus warnings-denied Clippy and `no_std`, generated-data checks, exact
live manifests, and workflow YAML parsing. Both GPT-5.6 final reviews are closed
and `CLEAN`.

Ordinary CI `30501347115` passes **3/3**. Full run `30501347064` passes
**43/43**, including the new `intl-collator` gate and updated
`intl-supported-values` gate. The overall long-running complete-engine goal
remains active; this marks only the Collator unit complete.

Every continuation of this unit remains subject to the reply gate above:
delete `target`, nested targets and generated lockfiles, Test262 checkouts and
results, CI downloads, Python caches, logs, temporary files, and downloaded or
reused tool caches before every reply, then verify no related process remains
and report free disk space.

## Completed unit - `Intl.supportedValuesOf`

Commit `bab346a` is pushed on `main`. Every Realm now installs a distinct,
non-constructible `Intl.supportedValuesOf`. The method performs observable
`ToString`, accepts only the six case-sensitive ECMA-402 keys, throws native
function-Realm errors, and returns fresh function-Realm Arrays. The pinned
generator now emits the normative 16 calendars and 45 sanctioned simple units,
CLDR 48.2's 78 simple-digit numbering systems, and 445 primary IANA time-zone
identifiers after mapping UTC links and excluding `Etc/Unknown`. Currency
remains an honest empty capability set until its formatter provider exists;
the current Collator unit adds ten ICU-validated collation values. Publication
precharges count/string fuel, reserves Vec and namespace
storage fallibly, pins through GC retry, and has ArrayPresence and exact-cap
failure/recovery coverage.

The exact standalone Test262 manifest now owns 16 files at **16/0/0**. The whole
25-file scope is hard-gated at **16 pass / 0 fail / 9 skip**; the held files
instantiate absent DateTimeFormat, DisplayNames, NumberFormat, or
RelativeTimeFormat services. Existing Intl regressions remain **161/0/0** for
Locale and **41/0/0** for canonical locale lists. Local gates pass
all-target/all-feature library **373/373**, every integration target and
benchmark smoke, release library **373/373**, Intl integration **13/13**,
Python tooling **149/149** with four expected absent-checkout skips, rustfmt,
warnings-denied Clippy, wasm32, Rust 1.88 MSRV, vendored RegExp **38/38** plus
warnings-denied Clippy and `no_std`, generated-data checks, exact live manifests,
and workflow YAML parsing. GPT-5.6 runtime and tooling final reviews are closed
and `CLEAN`; tooling's one intermediate P2 test-evidence finding was fixed and
re-reviewed.

Ordinary CI `30495522348` passes **3/3**. Full run `30495522373` passes
**42/42**, including `intl-supported-values`. All 32 historical matrix result
artifacts are byte-identical to Locale-info run `30491266981`, so aggregate
remains **32290 pass / 5028 fail / 11148 skip / 3 timeout / 0 error** over
48469 files with 37318 run. The dedicated Intl job owns the 15 newly admitted
passes because `intl402` remains outside the historical matrix.

The next formatter unit is the current `Intl.Collator` work recorded above.
Keep NumberFormat, DateTimeFormat, DisplayNames, and their wider data
dependencies separate.

---

## Completed unit - `Intl.Locale-info`

Commit `abf9bc7` is pushed on `main`. `Intl.Locale` now implements
`firstDayOfWeek`, `getCalendars`, `getCollations`, `getHourCycles`,
`getNumberingSystems`, `getTimeZones`, `getTextInfo`, and `getWeekInfo` with
Realm-correct result objects and arrays, brand checks, explicit-region and
Unicode override precedence, likely-region fallback, and bounded fuel/allocation
charging. A deterministic generator pins CLDR 48.2 commit
`11299982335beb974c1c63c45265184e759c0f41`; generated calendar, collation,
hour-cycle, numbering-system, time-zone, text-direction, week, likely-subtag,
and subdivision data is checked in ordinary CI.

The frozen base Locale boundary remains **109/0/0** and the disjoint exact
Locale-info manifest passes **52/0/0**, for a combined **161/0/0** pinned
boundary. Local gates pass all-target/all-feature library **372/372**, release
library **372/372**, every integration target and benchmark smoke, Intl
integration **11/11**, Python tooling **148/148** with four expected
absent-checkout probes, rustfmt, warnings-denied root and vendored Clippy,
vendored tests **38/38**, vendored `no_std`, wasm32, Rust 1.88 MSRV, generated
data comparison, exact manifests, and workflow YAML parsing. GPT-5.6 runtime
and tooling reviews are closed after all findings were fixed; both final
reviews returned `CLEAN`.

Ordinary CI `30491266844` passes **3/3**. Full run `30491266981` passes
**41/41**, including the dedicated `intl-locale` job's exact 161-file and
52-file gates. All 32 historical matrix result artifacts are byte-identical to
base-Locale run `30485142505`, so aggregate remains **32290 pass / 5028 fail /
11148 skip / 3 timeout / 0 error** over 48469 files with 37318 run. Next safe
ECMA-402 unit was the now-complete `Intl.supportedValuesOf` boundary above.
Formatter locale negotiation remains a separately designed next unit.

---

## Completed unit - base `Intl.Locale`

Commit `2f37339` is pushed on `main`. Every Realm now owns a distinct
`%Intl.Locale%` constructor/prototype pair. Instances use an unforgeable
`HeapObj::IntlLocale` brand and one-time immutable canonical locale record;
constructor options follow the required coercion/get order and two-phase
recanonicalization. Base component and Unicode-keyword accessors, `toString`,
`maximize`, `minimize`, subclassing, eager new-target prototype observation,
foreign-Realm fallback, GC roots, exact pin reservation, and fuel precharges
are implemented. Locale objects take the direct internal-slot path through
`CanonicalizeLocaleList`. The subsequent `Intl.Locale-info` unit above now
provides calendar/collation/time-zone/week/text-direction data.

The frozen base boundary contains 108 non-`Intl.Locale-info` files below
`intl402/Locale` plus adjacent
`intl402/Intl/getCanonicalLocales/Locale-object.js`. Local and CI exact runs
pass **109/0/0**; the canonical-locale gate advances to **41/0/0**. Locale
scope closure retains all 52 Locale-info files and future files as skips.
Local gates pass all-target/all-feature tests with library **370/370**, Intl
**10/10**, every benchmark smoke target, warnings-denied Clippy, rustfmt,
wasm32, tooling **148/148** with four expected absent-checkout probes, and the
exact pinned manifests. GPT-5.6 runtime and tooling reviewers are closed after
their findings were fixed and both returned `CLEAN`.

Ordinary CI `30485142330` passes **3/3**. Full run `30485142505` passes
**40/40**, including the new `intl-locale` job and updated canonical gate.
The historical 32-shard matrix excludes `intl402`; all **32/32** result
artifacts are byte-identical to `30478610621`, so aggregate remains
**32290 pass / 5028 fail / 11148 skip / 3 timeout / 0 error** over 48469 files
with 37318 run. This identity is expected; the dedicated Intl gates own the
109-file conformance movement.

This base unit's former next step, `Intl.Locale-info`, is complete above. Keep
formatter constructors as separately designed and admitted units.

---

## Completed unit - `%Intl%` and canonical locale lists

Commit `a3c417c` is pushed on `main`. Every Realm now owns an ordinary `%Intl%`
namespace and Realm-local `getCanonicalLocales`. Locale-list coercion follows
the required String singleton, one-length-read, HasProperty/Get, object
ToString, stable deduplication, GC-rooting, and function-Realm Array/error
semantics. ICU4X `icu_locale` 2.2.0 supplies core CLDR canonicalization; a
deterministic formatter-independent generator pins CLDR 48.2 commit
`11299982335beb974c1c63c45265184e759c0f41` for missing U/T aliases. Structural
adapters preserve reserved long languages, numeric extension singletons,
canonical transformed languages, and repeated transform keys that ICU's map
would otherwise discard. Input length is fuel-charged before scanning and
subtag work receives a quadratic precharge before ICU sorting.

The exact 40-file manifest closes future files in its Intl directories until
explicit admission. The pinned 41-file boundary is **40 pass / 0 fail / 1
skip**, with only `Locale-object.js` retained behind `Intl.Locale`. Local gates
pass all-target/all-feature library **369/369**, every integration target and
benchmark smoke, Intl integration **6/6**, Python tooling **147/147** with four
expected absent-checkout skips, rustfmt, warnings-denied Clippy, generated alias
comparison, workflow YAML, wasm32, and Rust 1.88 MSRV. Two GPT-5.6 reviews
found and drove fixes for pre-scan fuel bypass, repeated tkey loss, future
ungated Intl files, and moving-rustfmt generator input; both agents are closed.

Ordinary CI `30478610635` and full Test262 run `30478610621` pass completely;
the dedicated `intl-canonical-locales` job is green. Against preceding clean
full run `30467210702`, all 32 matrix artifacts retain aggregate
**32290 pass / 5028 fail / 11148 skip / 48469 total / 37318 run** with zero
delta because `intl402` is intentionally outside that historical directory
matrix and is enforced by the new dedicated exact gate. Next ECMA-402 unit is
`Intl.Locale` construction/internal slots/canonical accessors; locale
negotiation and formatter constructors remain later independent scopes.

## Completed unit - static import attributes and typed modules

The Reference-record milestone remains complete at **663/0/1** and the
supported statements/expressions subset remains 100%; do not restart it. The
current ES Module unit adds decoded, UTF-16-sorted Import Attributes to
`ModuleRequest`, preserves them through all static import/re-export forms, and
deduplicates requests only with attribute equality. Static and dynamic relative
requests share JavaScript/JSON/text cache identities and ModuleRecords. JSON is
parsed once during graph resolution; graph-local payloads are pinned until
publication and synthetic data records initialize one internal default binding
without observable global `JSON.parse`.

Final local gates pass all-target/all-feature library **367/367**, release
library **367/367**, modules **33/33**, Python tooling **146/146** with four
expected absent-checkout skips, rustfmt, warnings-denied Clippy, wasm32,
doctest **1/1**, and every benchmark smoke target. Exact Test262 static grammar
plus JSON/text runtime is **30/30** on workflow pin `9e61c128`. GPT review found
and prompted linked-record dynamic evaluation, legacy StringLiteral escape,
`import-text` broad-gate, request-Realm prototypes, graph payload rooting,
physical-path diagnostics, getter ordering, exact-cap parent/child identity,
and duplicate-key subtree-root fixes; both final reviews are CLEAN.
Implementation commit `aa62191` and inaccessible-default-checkout portability
fix `cbd28fc` are pushed. Ordinary CI `30467208759` passes **2/2** and all
**39/39** jobs in full run `30467210702` pass. Against clean run `30454181980`,
30/32 Test262 result files are byte-identical; `language/import` is exactly
**+17 pass/-17 skip** and `language/module-code` is **+13 pass/-13 skip**.
Aggregate is **32290/5028/11148/3/0** over 48469 total with 37318 run, exactly
**+30 pass/-30 skip** and no regression. All downloaded comparison artifacts,
local Test262 checkout, Python caches, and Cargo targets were deleted before
the evidence commit. Evidence commit `08b7c36` is pushed; its docs-only
ordinary run `30470039033` and full run `30470039449` were immediately
cancelled without observation, per policy.
Intl `%Intl%` plus `getCanonicalLocales` was independently audited as the next
meaningful ECMA-402 foundation; it requires ICU4X plus an ECMA-402 grammar and
pinned CLDR alias adapter, so it remains a separate unit after this module
boundary.

## Completed unit - incremental GC pre-sweep retrace cursor

`collect_incremental` now persists explicit Mark/Retrace phases and applies a
finite budget to initial traces, current-root closure, every physical retrace
slot, and dirty revisits. Arc-owned cells keep active object callbacks visible
to re-entrant GC; access through `with_obj` or `with_private_elements` queues
an already-scanned marked owner once. Late roots, active accesses, and
allocations rejoin the ordinary worklist. `budget=0` cannot trace or
sweep a non-empty heap, while `budget=1` advances retrace by one physical slot
or dirty revisit. Sweep remains atomic.

Focused release GC tests pass **16/16**, all-target/all-feature release tests
pass with library **330/330**, warnings-denied Clippy and rustfmt pass, and
Python tooling is **145/145** with four absent-checkout probes skipped. Pinned
WeakMap/WeakSet/WeakRef/FinalizationRegistry is **302/302**. Two GPT-5.6 audits
drove independent barrier-path tests, actual low-cell cycle completion, precise
`usize::MAX` semantics, and the direct-cell barrier caveat; final runtime
review is CLEAN. Documentation records why sweep was not cursorized without a
stronger root/mutation barrier. Implementation commit `f4747ce` is pushed;
ordinary CI `30431870408` passes **2/2** and full run `30431870820` passes
**38/38**. Against `30428204190`, all **32/32** result artifacts are
byte-identical at aggregate **32260/5028/11178/3/0** over 48469 with 37288 run.
Evidence commit `74c581e` is pushed. Its docs-only ordinary run `30434003217`
and full run `30434003071` were immediately cancelled without observation, per
policy. Final mandatory cleanup passed: no root/nested Cargo target, pinned or
temporary Test262 checkout, CI download, benchmark output, runner log, Python
cache, generated vendor lockfile, matching process, subagent, or stale worktree
remains; 2.8 GiB is free.

Next collector units: cursorize the outgoing slots of large objects and
WeakMaps; then add the stronger root/mutation phase needed to pause cell and
weak cleanup sweep safely. Root/bitmap setup and VM finite-slice scheduling
also remain. Collectible Symbols follow those pause-time units.

## Completed unit - incremental Map entry cursor

Ordered `MapData.entries` now use one cursor work unit
per key/value record. It snapshots entry count, counts down, and batches one
slice under the `IndexMap` lock. Mutation access repairs append, shift removal,
clear, reinsertion, and value replacement. Collection reads, ordinary
Get/HasProperty/GetPrototypeOf, and iterable probes retain active-root
protection without repeatedly dirtying a parked Map cursor. Compiled JS
regression coverage exercises `map.get`, named/numeric `in`, and
`Object.getPrototypeOf`. Newly
reached cells use the direct tracer for `usize::MAX`; pending cursors drain to
completion. Focused GC is 48/48.
Interleaved 20-sample full-GC parent/current ranges overlap at
448.09-476.02 us and 444.98-467.40 us; finite budget-256 work falls from the
parent's atomic 459.93-466.37 us to an amortized 754.22-891.68 ns per invocation;
the stateful benchmark mixes ordinary slices with periodic setup, retrace, and
sweep and is not an individual-slice latency bound. GPT-5.6 audits selected
Map as the safe next unit: Set needs upper-generation/epoch reseeking, WeakMap
needs stable ephemeron storage, and LazyGenerator needs an ordered composite
cursor while leaving its shifting async queue atomic. Broad gates, Test262,
commit/push, CI, and artifact comparison are complete; only evidence docs and
cleanup remain.

The exhaustive barrier audit found remaining pure observer paths on the
conservative mutation API: own descriptors/keys, extensibility/integrity,
classification, Promise/await, RegExp/String/Array helpers, and host observers.
Do not claim universal read-side termination until a later narrow
barrier-classification unit migrates and tests those paths. One accidental
delete mutation conversion found during the audit was reverted before commit.

Final local gates pass all-target/all-feature library 363/363, release library
360/360, focused GC 48/48, Python tooling 145/145 with four expected skips,
rustfmt, warnings-denied Clippy, wasm32, doctest 1/1, and every benchmark smoke
target. Pinned `built-ins/Map` remains 144 pass / 1 fail / 59 skip over 204;
the one failure is the existing `Symbol.toStringTag` descriptor. Exhaustive
GPT-5.6 re-review is CLEAN. Implementation commit `ceee602` is pushed;
ordinary CI `30454181926` passes **2/2** and full run `30454181980` passes all
**37/37** jobs. Against preceding clean full run `30445593831`, all **32/32**
result artifacts are byte-identical; aggregate remains
**32260/5028/11178/3/0** over 48469 files with 37288 run. Evidence docs commit
`073bc97` is pushed. Its docs-only full run `30456806517` and ordinary run
`30456806722` received immediate cancellation requests without observation.
Final mandatory cleanup removed 1.6 GiB of Cargo output, both CI artifact
downloads, the pinned Test262 checkout, temporary logs/results, Python caches,
and generated vendor lockfiles. No root/nested target, matching process, open
subagent, or stale worktree remains; `HEAD == origin/main`, the tracked
worktree is clean, and 2.3 GiB is free.

## Completed unit - incremental Promise/FinalizationRegistry record cursors

The in-progress source diff extends the existing finite-budget vector work to
`PromiseData.handlers` and `FinalizationRegistryData.cells`. A shared
always-inlined Promise handler visitor preserves the direct tracer's exact root
order; registry cursors trace held values only, leaving targets and unregister
tokens weak. Snapshot countdown, Mark/ Retrace growth, removed-slot charging,
settlement result/drain, cleanup-callback liveness, dirty revisits, `usize::MAX`,
private-edge ordering, and registry retain compaction have direct regression
tests. Focused GC is 39/39; low-artifact all-target/all-feature and release
library tests are 353/353. Clippy `-D warnings`, rustfmt, wasm32, doctest 1/1,
every Criterion smoke target, and Python tooling 145/145 with four expected
skips pass. Pinned Promise/FinalizationRegistry Test262 is 489/0/287 over 776.
GPT-5.6 review found missing Retrace-growth, settlement-drain,
cleanup-callback, and finite-slice benchmark coverage; all were added and its
re-review is CLEAN.
Same-host quick full-GC A/B
shows no regression: Promise handlers are 1.3170-1.3337 ms parent versus
1.2913-1.3226 ms current, and registry cells are 332.37-338.25 us parent versus
306.86-312.13 us current. One Promise handler can still expose large nested
AsyncFunction vectors atomically; Map/Set/WeakMap/LazyGenerator cursorization
remains separate work. Only the evidence-doc commit and mandatory cleanup
remain.

Implementation commit `39cf24c` is pushed. Ordinary CI `30445593804` passes
2/2 and full CI `30445593831` passes 38/38. Against `30439701818`, all 32/32
result artifacts are byte-identical with content-set hash `22f72e0e...`;
aggregate remains 32260/5028/11178/3/0 over 48469 with 37288 run. Evidence
commit `09a5633` is pushed. Its docs-only ordinary run `30447803344` and full
run `30447802145` were cancelled without observation. Mandatory per-turn
cleanup passed: no target, Test262 checkout, CI download, benchmark output,
Python cache, generated vendor lockfile, matching process, open subagent, or
stale worktree remains; HEAD equals `origin/main`, the tracked worktree is
clean, and 2.3 GiB is free.

## Completed unit - incremental Array/Iterator vector cursors

The in-progress source diff replaces the separate Array partial state with one
LIFO `TraceWork` stack for cell headers and Array/internal Iterator vector
cursors. Finite passes snapshot vector length, count down, charge every slot,
batch the current remaining budget under one lock, and place children above the
continuation. Growth is found by physical retrace or dirty revisit; shrink
cannot extend or stall a pass. `usize::MAX` completes parked cursors but uses
the prior direct atomic tracer for newly reached vectors.

Active callbacks now queue both root and dirty barriers before nested GC, and
the RAII access guard runs the post barrier during normal return or unwind.
Twenty-five focused GC tests cover exact progress, batching, LIFO priority,
Array/Iterator growth, Array shrink, repeated dirty mutation, MAX completion,
active re-entry, and unwind. Low-artifact all-target/all-feature tests pass with
library 339/339, every integration suite, and all Criterion smoke cases.
Release library is 339/339; focused GC is 25/25; Clippy `-D warnings`, rustfmt,
wasm32, doctest 1/1, and Python tooling 145/145 with four expected skips pass.
Pinned Array/Iterator/WeakMap/WeakSet/WeakRef/FinalizationRegistry Test262 is
3788/0/109/0/0 over 3897. The first ordinary all-target attempt exhausted the
2.4 GiB free disk and its linker died with SIGBUS; after `cargo clean`, the
same gate passed with incremental compilation and debug info disabled.

A permanent `gc_dense_primitive_array_100k` Criterion case was added. The
parent atomic collector measured 238.29-242.58 us; the final full-GC fast path
measured 228.53-228.56 us. The first cursor implementation was rejected after
measuring a 29% regression; batching, slice iteration, and the `usize::MAX`
atomic fast path removed it. GPT-5.6 correctness review was CLEAN. A second
GPT-5.6 review found the performance, real growth/shrink coverage, test-helper
bound, stale docs, and fragile edge-splitting issues; all are addressed. One
final reviewer was terminated after failing to return, so do not claim a third
clean review.

Implementation commit `6a715c1` is pushed. Ordinary CI `30439702247` passes
2/2 and full run `30439701818` passes 38/38. Against preceding clean run
`30431870820`, all 32 result artifacts are byte-identical; aggregate remains
32260/5028/11178/3/0 over 48469 with 37288 run, and both content-set hashes are
`22f72e0e...`. Evidence docs commit `7a80378` is pushed. Its docs-only CI
`30442203368` and full run `30442203254` were cancelled without observation.
Before any reply, close reviewers, clean root/baseline targets, remove
`/root/RuJa-gc-baseline`, Test262/CI downloads, benchmark output, caches, logs,
and temporary files, then verify processes and disk space as required by the
per-turn rule above.

## Completed unit - Set algebra iterator, Realm, and storage pipeline

All seven Set composition methods now use rooted SetRecords, cached generic
iterator records, Realm-local GC-retrying result allocation, catchable
IteratorClose with original-completion priority, fallible result growth, and
per-slot Fuel. SetData uses generation-ordered slots plus a hash index; deletion
tombstones a slot, reinsertion appends, and generation/epoch cursors survive
stable compaction once tombstones reach `max(live, 64)`. Constant-live churn is
bounded without snapshot refresh, and `forEach` shares the rooted cursor path.
`difference` traverses its result copy in the small-receiver branch.

Local evidence passes all-target/all-feature release tests including
library **325/325**, focused Set integration/resource tests, warnings-denied
Clippy, rustfmt, tooling **145/145** with four absent-checkout probes skipped,
exact pinned Set algebra **186/186**, and full Set **351/2/30**. Exact seven-path
metadata admission and a dedicated literal 186-file full-workflow gate are in
place. GPT-5.6 reviews found and drove fixes for result-snapshot traversal,
observable iterator bypasses, incomplete roots/close, O(N^3) refresh, unbounded
tombstones, stale `forEach`, retry cursor loss, pre-close error materialization,
and failpoint gaps; final runtime and tooling/CI re-reviews are CLEAN. The
resource test now covers later countdown failures, post-step close, bounded
churn, active-iterator compaction, per-slot Fuel resume, exact-cap pair failure,
and Fuel-atomic compaction/clear. Implementation commit `9dc89b8` and CI
portability fix `96eab9e` are pushed. Ordinary CI `30426806030` passes **2/2**;
full run `30426233118` passes **38/38**, including dedicated Set algebra
**186/186**. Against `30411263253`, built-ins moves exactly **+7 pass / -7
skip** to **16639/4217/2809/3/0**. Normalized aggregate is
**32260/5028/11178/3/0** over 48469 with 37288 run; 30/32 raw artifacts are
identical, annexB only clears two prior contention timeouts, and normalized
identity is 31/32. The independent failure content-set hash remains
`4d596150d0ed50123f4d0ecdc0a187da21e74991fc858a9e10732c4fa95afa40`.
Evidence commit `cf3de4a` is pushed; its documentation-only full run
`30428204190` and ordinary run `30428204234` both passed. Final mandatory cleanup passed: no root/nested Cargo target,
pinned or temporary Test262 checkout, CI download, runner log, Python cache,
generated vendor lockfile, matching process, or stale worktree remains; 2.8
GiB is free. Next collector unit should cursorize large-object/WeakMap tracing,
final mutation retrace, and sweep before tackling collectible Symbols.

## Completed unit - WeakMap/WeakSet complete pipeline

WeakMap and WeakSet now consume iterables through direct cached zero-argument
iterator records with correct close/Fuel behavior, full rooting, GC-retrying
allocation, and Realm-local prototype/error/fallback identities. Methods enforce
brands; WeakMap includes both upsert methods. `WeakKey` accepts objects plus
local/well-known Symbols and rejects registered Symbols. Fallible HashMap/
HashSet storage gives average O(1) access and reserves only new entries.

GPT-5.6 audits found the prior root-order-dependent WeakMap mark bug, linear
Vec representation, quadratic fixed-point scan, non-progressing incremental
mark, missing slice-mutation barrier, callback/key validation inversion, and
an informational-only CI result. All are repaired: reachable WeakMap entries
use key-indexed pending activation; finite-budget state resumes and retraces
marked cells before sweep, while root snapshots, queued-identity deduplication,
and an allocation barrier prevent cross-slice quadratic growth or missed new
objects. Final host-root remark and collector-first allocation locking close
old-white root and concurrent publication races; registered Symbol
classification is O(1); callback
callability precedes key validation; and CI hard-gates the exact 226-file result.
Root-order, transitive-chain, incremental-mutation, strong-property, dead-cycle,
and heap-cell reuse tests pass.
Pinned WeakMap/WeakSet moves ordinary **55/76/95 -> 226/0/0** and forced
**91/135 -> 226/0** through an exact 95-path metadata admission. Adjacent
WeakRef/FinalizationRegistry is **76/76**. Focused iterator, Realm, Symbol,
brand, upsert, GC, reservation, Fuel, exact-cap, cleanup, and retry tests pass.

Final GPT-5.6 runtime and tooling/workflow re-reviews are CLEAN. Local gates
pass all targets/features and library **324/324**, Clippy, fmt, wasm32,
doctest, tooling **144/144**, vendored RegExp unit **29/29** plus doc
**14/14**, ordinary and forced pinned WeakMap/WeakSet **226/226**, and adjacent
WeakRef/FinalizationRegistry **76/76**. Implementation commit `9b0f736` is
pushed; ordinary CI `30411263256` passes **2/2**, full run `30411263253`
passes **37/37**, and its dedicated WeakMap/WeakSet job hard-gates **226/226**.

Against preceding full run `30403723369`, built-ins alone moves **+171 pass /
-76 fail / -95 skip** to **16632/4217/2816/3/0**. Raw artifacts were 30/32
byte-identical because annexB had two contention timeouts; a standalone rerun
with the current CI release binary is byte-identical to the preceding annexB
artifact, establishing corrected 31/32 identity. Corrected aggregate is
**32253/5028/11185/3/0** over 48469, with 37281 run and content-set hash
`4d596150d0ed50123f4d0ecdc0a187da21e74991fc858a9e10732c4fa95afa40`.
Evidence commit `e825737` is pushed. Its docs-only full run `30413334952` and
ordinary CI `30413334930` were immediately cancelled without observation, per
policy. Final mandatory cleanup passed: no root/nested Cargo target, pinned or
temporary Test262 checkout, CI download, runner log, Python cache, generated
vendor lockfile, matching process, or stale worktree remains; 4.8 GiB is free.
The dedicated Set algebra iterator/root/storage unit follows above and
supersedes this historical next-step marker.

Collector follow-up after this unit: cursorize large-object/WeakMap tracing,
the final mutation retrace, and sweep so finite GC budgets bound total native
work rather than only newly marked cells; make the Symbol arena collectible so
Symbol-keyed weak entries can release values before VM teardown. Current mark
queues deduplicate identities and ephemeron chains no longer rescan all maps.

## Completed unit - Set constructor iterator, Realm, and storage pipeline

`new Set(iterable)` now uses the direct cached zero-argument iterator record,
observes Proxy Get and all built-in iterator overrides, meters each step, and
separates step/non-catchable-Fuel propagation from catchable adder
IteratorClose. Prototype, iterable, result Set, adder, iterator/next, value, and
original errors are rooted. Allocation uses `Vm::alloc`; native
`Set.prototype.add` reserves only new ordered-slot/hash-index entries before
mutation.

Every Realm now installs GC-rooted Set, Set prototype, and Set Iterator
prototype identities with rollback and immutable fallback. Two GPT-5.6 audits
agree the constructor pipeline is correct after fixes. Their separate Set
algebra iterator/root/storage findings remain a later dedicated unit; broad
composition insertion changes were removed from this constructor patch.
Focused direct/resource tests pass. Pinned Set moves top-level ordinary
**16/0/6 -> 20/0/2**, forced **20/2 -> 22/0**, and full directory
**340/2/41 -> 344/2/37** through exact four-file admission. Broad local gates
pass default debug/release library **309/309**, all-feature library **312/312**,
es2015 **138/138**, `with` **62/62**, tooling **143/143**, vendored RegExp
**38/38**, all targets/features, fmt, Clippy, wasm32, and doctest. Final GPT-5.6
runtime/tooling findings are addressed. Implementation commit `4ee6c3a` is
pushed; ordinary CI `30403723434` passes **2/2** and full run `30403723369`
passes **36/36**. Against `30397512891`, 31/32 artifacts are byte-identical;
built-ins alone moves **+4 pass/-4 skip** to **16461/4293/2911/3/0**.
Aggregate is **32082/5104/11280/3/0** over 48469, with 37186 run and hash
`2b8d17790183867e4425c58403bd6fcd15d2d8f8c9366f8256a04f7f08a2b78a`.
Evidence commit `2d04778` is pushed. Its docs-only CI `30405945192` and full
run `30405945179` were immediately cancelled without observation, per policy.
Final mandatory cleanup passed: root/nested Cargo targets, pinned Test262 and
CI downloads, logs, Python caches, generated vendor lockfile, temporary
worktrees, and matching processes are absent; 5.4 GiB is free. Next units:
Those WeakMap/WeakSet and Set algebra follow-up units have since been completed
or are in final CI evidence collection above.

## Completed unit - Map constructor iterator, Realm, and storage pipeline

`new Map(iterable)` now uses the direct cached zero-argument iterator record,
observes Proxy Get and all built-in iterator overrides, meters each step, and
separates step-error/non-catchable-Fuel propagation from catchable post-step
IteratorClose. Prototype, iterable, result Map, cached adder, iterator record,
entry, key, value, and original errors are rooted in LIFO order. Allocation
uses `Vm::alloc`; native Map set/upsert insertion reserves before mutation.

Two GPT-5.6 design audits identified the same wrapper, rooting, Fuel, and
allocation defects. Focused direct and VM tests pass. Pinned Test262 top-level
Map constructor policy moves **19/0/11 -> 28/0/2** and forced execution stays
**30/0** through an exact nine-file admission; constructibility and mixed
TypedArray/WeakRef key tests remain outside scope. Final GPT-5.6 runtime and
documentation reviews are clean. Local gates pass release library **307/307**,
all-feature library **310/310**, builtins **561/561**, es2015 **137/137**,
`with` **62/62**, tooling **142/142**, vendored RegExp **38/38**, all
targets/features, fmt, Clippy, wasm32, and doctest. Pinned full Map is
**144/1/59** with only the existing toStringTag failure. Implementation commit
`4c0e28c` is pushed; ordinary CI `30397512857` passes **2/2** jobs and full run
`30397512891` passes **36/36**. Against `30390395072`, 31/32 result artifacts
are byte-identical; built-ins alone moves **+9 pass/-9 skip** to
**16457/4293/2915/3/0**. Aggregate is **32078/5104/11284/3/0** over 48469,
with 37182 run and content-set hash
`36f9fc0e9dc914015d869e2b199623aa4f9b42da116573d4f86a5dc638a89f78`.
Evidence commit `eb060c7` is pushed. Its docs-only CI `30399985341` and full
run `30399985329` were immediately cancelled without observation, per policy.
Final mandatory cleanup passed: root/nested targets, Test262 checkout, CI
downloads, logs, Python caches, generated vendor lockfile, temporary worktrees,
and matching processes are absent; 7.2GiB is free. Next adjacent unit is the
Set constructor iterator, close-priority, Realm, root, allocation, and storage
pipeline; WeakMap/WeakSet constructors follow.

## Completed unit - `Map.groupBy` iterator, Realm, and resource pipeline

`Map.groupBy` now uses the shared direct cached iterator record, zero-argument
metered steps, safe-index checking, original-completion-preserving close, and
non-closing step/Fuel paths. SameValueZero grouping roots every accumulated
value and distinct object key, releases redundant occupied-group key roots,
and publishes Realm-local Arrays into a Realm-local intrinsic Map through
fallible internal storage after normal completion. Each test262 Realm now
installs immutable Map and Map Iterator intrinsics; constructor fallback and
iterator output use those registries.

Both GPT-5.6 design reviews are closed and all findings are addressed. Focused
Map, Realm, and exact-admission tests pass. Fixed Test262
`9e61c12835c5e4a3bdba93850427e6742c4f64c4` A/B is ordinary **12/0/2 ->
14/0/0** and forced **14/0 -> 14/0** across all 14 files. Broad local gates
pass debug/release library **308/308**, builtins **561/561**, es2015
**136/136**, `with` **62/62**, tooling **141/141** with four optional live
probes skipped, vendored RegExp **38/38**, all targets/features, fmt, Clippy,
wasm32, doctest, and release build.

Final GPT-5.6 runtime and documentation reviews are CLEAN. Implementation
commit `061976a` is pushed; ordinary CI `30390397502` passes **2/2** jobs and
full run `30390395072` passes **36/36**. Against `30383198359`, 31/32 result
artifacts are byte-identical; `built-ins` alone moves **+2 pass/-2 skip** to
**16448/4293/2924/3/0**. Aggregate is **32069/5104/11293/3/0** over 48469,
with 37173 run and content-set hash
`58f6ae50b9c38de673d07c3307dc0c138209465c7babab1d0eff86dd80964166`.
Evidence commit `a1d735a` is pushed. Its docs-only CI `30393134187` and full
run `30393130068` were immediately cancelled without observation, per policy.
Next narrow adjacent unit is the Map constructor's wrapper iterator,
close-priority, Realm, root, and allocation pipeline.

## Mandatory per-turn artifact cleanup

Every turn must delete generated artifacts before the final response. This
includes the root and nested Cargo `target` trees, Test262 checkouts and CI
downloads, temporary logs/directories, Python caches, and any other generated
build or analysis output. Verify the paths are absent and report reclaimed or
remaining disk space. Do this even for documentation-only turns; never rely on
the next turn to clean the current turn's artifacts.

## Completed unit - `Object.groupBy` iterator and resource pipeline

`Object.groupBy` now uses a direct cached synchronous iterator record, calls
`next` with zero arguments, meters each step, observes all built-in iterator
overrides, and enforces the safe-index limit. Catchable callback/key/storage
errors close while preserving the original completion; step errors and host
Fuel do not close. Group and element storage reserve before mutation, every
accumulated value remains rooted, and Realm-local Arrays are published through
the fallible ordinary define path after normal iterator completion.
Output consumes one fuel unit per group, and LIFO cleanup releases accumulated
value roots before the iterator record after materialization.

Pinned Test262 moves from ordinary **13/0/1** to **14/0/0** and remains forced
**14/0** through one exact admission. Direct iterator/GC/close tests and VM
input-root, group, element, index-limit, result-property, Fuel, cleanup, and
retry tests pass. Two GPT-5.6 design-review findings are addressed. Final
reviews are also clean. Local gates pass library **307/307**, builtins
**561/561**, `with` **62/62**, release library
**304/304**, Python tooling **140/140**, all targets/features, rustfmt,
warnings-denied Clippy, release build, wasm32, and ordinary/forced focused
Test262 **14/14**. Two GPT-5.6 final reviews are CLEAN and closed after all
findings were addressed. Implementation commit `213d472` is pushed; ordinary
CI `30383198523` passes both jobs and full run `30383198359` passes all
**36/36** jobs. Thirty-one of 32 artifacts are byte-identical; `built-ins`
alone moves **+1 pass / -1 skip**, producing aggregate
**32067/5104/11295/3/0** and content-set hash
`8d7ac4bfebfa569639b2e93767c2496d6f933d89e8082d81eea4646c9aac6266`.
Evidence docs commit `d96f7bd` is pushed; its docs-only CI/full runs were
cancelled without observation. Final cleanup removed 2.9 GiB of Cargo output,
the Test262 checkout, both full artifact downloads, all runner/build logs, and
Python caches. No `/tmp/ruja*` output or related process remains; `HEAD ==
origin/main`. At that point the next adjacent candidate was `Map.groupBy`,
which still used the wrapper iterator and normal-close path before the current
unit repaired it.

## Completed unit - `Object.fromEntries` iterator protocol

`Object.fromEntries` now creates the result before `GetIterator`, caches the
iterator's `next`, and processes each yielded object through `Get("0")`,
`Get("1")`, `ToPropertyKey`, and fallible ordinary property definition. Step
errors do not close; primitive entries and later catchable abrupt completions
use the shared original-completion-preserving `IteratorClose` path.
Non-catchable Fuel aborts do not re-enter user cleanup. Pre-allocation/result
roots are reserved before pinning, and the result, iterator record, entry, key,
and value survive forced GC.

Pinned Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db` moves from normal
**12/0/13** to **25/0/0**. Forced fixed-policy execution isolates runtime
movement from **14/11** to **25/0**. The exact 13-file admission freezes live
feature metadata and rejects future/outside paths. Focused Rust GC, close
precedence, non-close, root-reservation, property-reservation, cleanup, and
retry tests pass. Local gates pass library **306/306**, builtins **560/560**,
`with` **62/62**, release library **303/303**, Python tooling **139/139**, all
targets/features, rustfmt, warnings-denied Clippy, release build, wasm32, and
ordinary/forced focused Test262 **25/25**. Two GPT-5.6 final reviews are CLEAN
and closed. Implementation commit `4601c00` and tooling follow-up `d0545c9`
are pushed; ordinary CI `30376165881` passes both jobs and full run
`30374968848` passes all **36/36** jobs. The preceding Annex B contention
timeout reruns at clean **201/811/74/0/0**, byte-identical to current. Corrected
artifacts have 31/32 files unchanged; `built-ins` alone moves **+13 pass / -13
skip**, producing aggregate **32066/5104/11296/3/0** and content-set hash
`9080e4e377d351a4621d58b154a8dae6967234a1bbdb7bef4b6865e4bde2baac`.
Evidence docs commit `1a34a84` is pushed; its docs-only CI/full runs were
cancelled without observation. Final cleanup removed 2.9 GiB of Cargo output,
the Test262 checkout, both full artifact downloads, runner logs, and Python
caches. No `/tmp/ruja*` output or related process remains; `HEAD ==
origin/main`. Next safe adjacent candidate is the `Object.groupBy`
direct-iterator close/Fuel/rooting audit.

## Completed unit - Array `@@unscopables`

Every Realm's Array intrinsic now creates a distinct null-prototype
`Symbol.unscopables` object with the exact 16 standard true-valued entries;
`with` is absent because it is a reserved word. Entries are writable,
enumerable, and configurable, while the Array prototype's symbol property is
non-writable, non-enumerable, and configurable. Construction reserves and pins
all three provisional objects, then releases them through one setup-result
boundary before publication.

Direct tests cover exact order and descriptors, main/foreign Realm identity
and mutation isolation, intrinsic-root GC survival, real `with` lookup for all
16 names, and exact pin/registry cleanup under root and property reservation
failure. Pinned Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`
moves from **0/4** on
release `b6964a1` to **4/0** locally without runner policy changes. Focused
and broad tests pass: library **305/305**, builtins **558/558**, `with`
**62/62**, Python tooling **138/138**, all targets/features, release build,
rustfmt, warnings-denied Clippy, and wasm32. Two GPT-5.6 reviews are clean after
their findings were addressed. Implementation commit `2448889` passes CI
`30362037365` and all 36 full-run jobs in `30362037348`. One raw Annex B
contention timeout reruns byte-identically at **201/811/74/0/0**. Corrected
artifacts have 31/32 files unchanged; built-ins alone moves **+4 pass/-4
fail**, producing **32053/5104/11309/3/0** with content-set hash
`1d221f690ccf985fbfb58e7358f586221e02a8b56afb4d2add0c29782a8fa6d5`.
Evidence docs commit `e3e9320` is pushed. Its docs-only CI/full runs were
cancelled without observation as requested. All local build, sparse-checkout,
downloaded CI, comparison, and Python cache artifacts were deleted before the
turn ended. Next safe candidate remains `Object.fromEntries` iterator/order/
close semantics; keep that as a separate narrow unit.

## Completed unit - string-valued Unicode RegExp sets

RegExp `v` now supports all seven Unicode 17 properties of strings and
`\q{...}` disjunctions. Empty alternatives are explicit, one-character strings
cross over with ordinary character sets, and `/iv` simple-folds each sequence
before union/intersection/subtraction. Grammar-level `MayContainStrings`
follows union OR, intersection AND, and subtraction's left operand, so valid
single-character negation is accepted while multi/empty string complements are
early errors. RegExp literal validation invokes the same vendored parser.

String alternatives are canonical and deduplicated. Matching orders
multi-code-point strings longest first, singles next, and empty last;
lookbehind reverses each sequence. The emitter uses a shared-prefix trie and
groups exact interned suffix subtrees behind bracket transitions. Static
property data is charged before cloning. Individual elements are capped at 256
code points, cumulative materialization and conservative pre-emission cost at
750,000 each, runtime work at a compiled-cost-scaled 32,000,000 ceiling, and
live Pike state at 64 MiB. Explicit alternatives and the conservative trie-node
upper bound are each capped at 65,536, preventing duplicate-empty and
allocation-heavy long q sets. Repeated aggregate RGI programs fail at construction.

Direct vendored and RuJa regressions cover folding, empty priority, negation,
lookbehind, punctuators, backspace, Emoji keycaps, logical surrogate/scalar
identity, and resource rejection. Exact admission now freezes 142 generated
Unicode-set and property-string paths. Focused Test262 is **142/0/0/0/0**;
complete RegExp is **1189/0/690/0/0**, exactly **+94 pass / -94 skip** from
the preceding **1095/0/784/0/0**. Low-artifact all-feature library and
integration gates pass with library **304/304** and builtins **557/557**;
warnings-denied Clippy, wasm32, vendored no-std, and formatting pass. Tooling
is **137/138** with only the known checkout-absent staging TypedArray fixture.
Final GPT-5.6 reviewers Beauvoir and Turing are CLEAN after the duplicate-empty
alternative pre-allocation bound and documentation corrections.

Implementation commit `b6964a1f96b5491219499c4a6641358102b06b34` is pushed.
Ordinary CI `30355514978` passes both jobs; full run `30355514979` passes all
**36/36**, including the exact 142-file job. Against baseline `30346121824`,
31/32 result artifacts are byte-identical and only `built-ins` moves exactly
**+94 pass / -94 skip** to **16428/4297/2940/3/0**. Aggregate is
**32049/5108/11309/3/0** over **48469** total and **37157** run. The
path-independent sorted-content evidence hash is
`ffbe2ebc3d7d3c5702fb59dfc16b9f744e4573caca94838f87ac04d5b5b9f039`.
Evidence commit `965862d` is pushed. Its docs-only CI `30358008815` and full
run `30358008707` were canceled to avoid duplicate work; no observation is
required. Final cleanup removed 5.0 GiB of root Cargo output, 276 MiB of
vendored Cargo output, the pinned Test262 checkout, both CI artifact downloads,
Python caches, and the generated vendor lockfile. No matching temporary output,
build runner, reviewer, or duplicate Codex session remains active.

## Completed unit - logical UTF-16 Unicode RegExp matching

- Vendored `regress` 0.11.1 supplies a native UTF-16 logical matcher only for
  collision-bearing u/v inputs or Unicode patterns the old backend cannot
  compile. Ordinary input retains Rust/fancy fast paths. `v` enables both
  Unicode flags.
- Bounded Pike execution charges instructions, candidate/state allocation,
  state copies, capture/loop slots, and backreference units; shares budget
  through lookarounds; caps live states; and has a one-candidate sticky API.
  Greedy one-character loops before an end assertion use constant live-state
  memory. Non-multiline `$` remains restricted to the actual input end.
- Logical backreferences compare symbols, duplicate-name references select
  participating captures conditionally, iv operands fold before set algebra,
  flat alternations use balanced IR, endpoints map in one scan, and logical
  input no longer retains a tuple per character. Default intrinsic global
  Symbol.match compiles and prepares input once; observable custom exec paths
  retain the specification loop. The fast path is limited to full-Unicode,
  non-sticky, observed/private-flags-equivalent receivers. String-valued v sets
  are rejected. Logical source is prebounded with an allocation-free,
  pre-validator scan to 262,144 UTF-16 units, then an escape-aware scan to 64
  property operands; every u/v
  construction validates these resource preconditions, and bracket/name/conditional
  reference storage is included in compiled cost.
- Named-capture preprocessing caps total names at 1,024, duplicates per name at
  64, stored path segments at 65,536, comparison work at 1,000,000, and
  duplicate-backreference expansion at 16,384.
- Generic loops charge only actual state splits. The 64 MiB live-state limit
  covers retained vector capacity across nested lookarounds, and global
  Symbol.match consumes VM fuel before retaining each result.
- Direct collision/property/capture/global/sticky/d-index/String tests and
  six vendored bounded API tests pass. Exact Co/Cs Test262 files pass
  independently at 14.90s and 14.92s under a frozen 30-second policy. Final
  complete RegExp run is **1095/0/784/0/0**.
- Root all-target testing with local debug info disabled passes library
  **299/299**, builtins **555/555**, all integration binaries, and Criterion
  smoke. Earlier release library passed; vendored regress now passes **31/31**, wasm32,
  doctest **1/1**, fmt, and warnings-denied Clippy pass. Tooling is **137/138**;
  only the known absent staging TypedArray fixture fails. Two GPT reviews found
  semantic and resource-bound issues; all final re-reviews are clean.
- Implementation `f84b825` plus tooling follow-up `754f220` pass ordinary CI
  `30346121819` and all 35 jobs in full run `30346121824`. Against baseline
  `30332410263`, 31/32 artifacts are byte-identical; `built-ins` alone moves
  exactly two skips to passes. Aggregate is **31955/5108/11403/3/0** over
  **48469** total and **37063** run. Sorted evidence hash:
  `990bcf4a214a9df0e9e1fe279c79c4961be2403c513dc6d1332c4db3a9589781`.
- Evidence docs commit `a9d1508` is pushed. Its docs-only CI/full runs were
  canceled per policy; no duplicate workflow remains intentionally active.
- Final local cleanup removed root/vendor targets, Python caches, nested vendor
  lockfiles, Test262 temporary output, and downloaded CI artifacts.
- The former separate 66-file string-valued v-set work is the current unit
  above; its complete generated property surface is 94 newly admitted files.

## Completed unit - canonical sentinel-range scalar ingress

- `utf16_from_scalar_str` and `push_utf16_scalar` canonicalize external
  well-formed Unicode scalars in `U+F0000..U+F07FF` through their two UTF-16
  units. Ordinary text takes the direct single-copy path.
- Source strings, RegExp literal text, template cooked/raw text, decoded JSON
  escapes, serde values/keys, and JSON/text data modules use the boundary.
  `JSON.parse` raw text is already an internal JS string and must not be
  canonicalized a second time. Serde export decodes valid pairs and replaces
  lone surrogates with U+FFFD because host strings must be well formed.
  Unicode RegExp normalization recombines canonical high/low units so raw,
  class, and constructor scalar patterns retain self-match behavior. Static
  and dynamic module specifiers plus public/CLI output decode canonical UTF-16
  at host boundaries. Direct `Value::String`/low-level PropertyKey construction
  remains an explicitly documented unchecked escape hatch. `Vm::to_string`
  and `to_property_key` expose internal canonical text; hosts use
  `to_string_pub`. Serde key replacement collisions keep the later property.
  Native errors carry a host/internal provenance bit: filesystem and callback
  host messages canonicalize once during materialization, while already-
  internal messages remain unchanged. Host display decodes internal messages;
  mixed module-link diagnostics canonicalize only their host path fragment.
- Focused source/JSON, template, serde, and data-module tests pass. Root
  all-target/all-feature tests pass with library **301/301**, builtins
  **551/551**, and Criterion smoke; release library **301/301**, doctest,
  wasm32, formatting, and warnings-denied Clippy pass.
- Pinned JSON.parse plus template Test262 is **157/0/4/0/0** over 161; complete
  RegExp remains **1093/0/786/0/0** over 1,879. Tooling is **136/137**, with
  only the known absent staging TypedArray fixture failing. Two GPT-5.6
  reviewers are `CLEAN` after requiring direct mixed host-path/internal-export
  error provenance coverage; both sessions are closed. Initial implementation
  commit `e04f833` passed ordinary CI `30329907142` and all 35 full-matrix jobs
  in `30329907125`, but artifact comparison exposed six eval-generated RegExp
  source regressions. The follow-up now distinguishes external source from
  already-canonical eval/dynamic-function source. Exact recovered Test262 is
  **6/0**. Final all-target/features passes with library **301/301**, builtins
  **551/551**, and Criterion smoke; release library **301/301**, wasm32,
  doctest **1/1**, formatting, and Clippy `-D warnings` pass. One GPT reviewer
  is `CLEAN`; the other found only API/evidence documentation omissions, now
  fixed. Follow-up commit `c504f61` is pushed. Replacement ordinary CI
  `30332410250` passes both jobs and full run `30332410263` passes **35/35**.
  All 32 result artifacts are byte-identical to clean baseline `30325163916`;
  aggregate remains **31953/5108/11405/3/0** over **48469** total and
  **37061** run, evidence hash
  `204943da73e6ce948c7b77e29165c806ea412c14cabd8312c06418c05fa1ea73`.
  Evidence commit `3fb10dc` is pushed; its docs-only Actions need no
  observation. Final cleanup deleted both downloaded artifact sets, the pinned
  `9e61c128` Test262 worktree, and Python caches. Cargo output was already
  absent and `cargo clean` removed 0 files. No generated output remains from
  this unit.
- This does not close Unicode RegExp matching. Lone surrogate sentinels still
  collide with private-use scalars in the backend. Next implementation unit:
  add logical-symbol matching over Unicode scalars plus lone surrogates; do not
  merge the separate 66-file string-valued `v`-set parser/trie work.

## Completed unit - exact complex `iv` RegExp word operands

- Complex `v` classes still use backend set-algebra syntax, but active-i
  `\w`/`\W` operands are now rewritten to exact ECMAScript WordCharacters.
  The ordinary whole-class HIR materializer remains disabled for nested
  algebra; only the atomic word escape is lowered.
- Direct regressions cover Rust-only Unicode words, long s, Kelvin, nested
  intersection/subtraction/union/complement, lookahead, and backreferences. A
  bounded manual word/literal probe agrees with Node 24.
- No Test262 admission changes. String properties, `\q{...}`, and the UTF-16
  sentinel collision remain separate.
- Root all-target/all-feature tests pass with library **299/299**, builtins
  **548/548**, and Criterion smoke. Release library **297/297**, wasm32,
  formatting, and warnings-denied Clippy pass. Split execution of the complete
  pinned RegExp directory totals **1093/0/786/0/0** over **1879** files.
- GPT-5.6 semantic reviewer Aristotle returned `CLEAN`. Documentation reviewer
  Dirac requested explicit union/right-word subtraction coverage, precise
  manual-Node wording, a direct remaining property-fold example, and removal
  of a stale empty-class limitation; all are addressed in the current diff.
- Python tooling runs **136/137**; the only failure is the known unrelated
  missing staging TypedArray fixture. All four RegExp admission invariants pass.
- Final reviewer Parfit caught an invalid property-fold limitation inferred
  from Node 24. Current ECMA-262 requires the observed long-s match; the stale
  limitation was removed and a direct specification regression was added.
- GPT-5.6 final reviewer Linnaeus returned `CLEAN` after that correction.
- Feature commit:
  `719743f828025c6e4effdf556fde1db80f7c5922 fix(regexp): make complex iv word operands exact`
- Ordinary CI `30323528176` passes **2/2** jobs. Full run `30323528182`
  passes **35/35** jobs. All 32 result artifacts are byte-identical to
  docs-only clean baseline `30317896191`; both aggregate to
  **31953/5108/11405/3/0** over **48469** total and **37061** run.
- Downloaded current/baseline artifacts and the temporary root-file symlink
  corpus were deleted immediately after comparison.
- Evidence commit:
  `77de3bc docs: record complex iv word CI evidence`
- Final cleanup removed **4.3 GiB** of Cargo output plus Python caches. No CI,
  Test262, benchmark, log, cache, or temporary artifact remains. Docs-only
  Actions from the evidence push need no observation.
- Next selection should compare the confirmed `U+F0000..U+F07FF` sentinel
  collision against the 66 string-valued `v`-set files and choose one coherent
  architecture unit; do not combine the two.

## Completed unit - exact Unicode ignore-case RegExp word boundaries

- Dedicated ECMAScript Unicode-i word-boundary HIR/look states now exist in
  vendored `regex-syntax`, `regex-automata`, and Fancy hard execution.
  Non-nullable repeated captures clear transactionally in PikeVM; nullable
  repeats retain Fancy `RepeatMatcher` semantics.
- `PrefilteredExact` combines a relaxed Rust rejection gate, a safe original
  Rust fast path when classifications agree, and a capture-erased exact linear
  matcher. Exact-position APIs anchor against the complete haystack for sticky
  and capture correction. Non-global replace evaluates only its first match.
- Adversarial coverage includes million-scalar scans, nested repeats, 100,001
  repeated captures, alternating capture clearing, later empty matches, sticky
  hostile suffixes, lazy first replacement, nullable exact bounds, and 20,000
  per-position matches without whole-input rescans.
- Complete pinned `built-ins/RegExp` remains **1093/0/786/0/0** with no
  admission change. Root all-target/all-feature tests, release, wasm32, docs,
  formatting, and warnings-denied Clippy pass; vendored syntax, automata, and
  Fancy suites pass **147+48**, **137**, and all upstream groups respectively.
  Four RegExp tooling admission tests pass. The complete 137-test tooling run
  has one unrelated external-checkout failure because pinned Test262 main lacks
  the staging TypedArray file required by the extensibility manifest.
- GPT-5.6 reviewers Dewey (`019fa5b1-4311-77a1-89a8-d949afc52c3f`) and
  Lagrange (`019fa5b1-717d-7263-9da8-b50c53133c0e`) found nullable-end API
  disagreement, quadratic global scans, duplicate Fancy programs, and hard-VM
  stack-cap growth. All were fixed; both reviewers returned `CLEAN` and were
  closed without building, editing, or cleaning the workspace.
- Feature commit `86482be` passed ordinary CI `30316291488` (**2/2 jobs**) and
  full matrix `30316291511` (**35/35 jobs**). Current 32-artifact aggregate is
  **31953/5108/11405/3/0/48469** with **37061** pass-or-fail executions. Against
  baseline `30310182698`, 31 files are byte-identical and Annex B changes by
  **+1 pass / -1 timeout** only.
- Evidence commit `1527255` records the CI and artifact comparison. Final
  cleanup removed all downloaded and generated output; the implementation
  remains part of the active overall engine goal.

## Completed unit - RegExp `u`/`v` flag mutual exclusion

- Feature commit:
  `24f4464a463e259d80757c1983af1783126aec6d fix(regexp): reject combined Unicode modes`
- Evidence commit:
  `375280f docs: record RegExp mode CI evidence`
- Common `validate_regex_flags` now rejects a completed flag set containing
  both `u` and `v`. Invalid-character and duplicate errors retain precedence;
  literal lexer/parser fallback and constructor initialization share the path.
- Direct tests cover `uv`, `vu`, mixed `d`/`y`, invalid/duplicate precedence,
  standalone modes, and both literal and constructor surfaces.
- Exact admission freezes only
  `built-ins/RegExp/prototype/unicodeSets/uv-flags.js` and
  `uv-flags-constructor.js`. The directory is **2/0/36/0/0**; future and
  outside `regexp-v-flag` files remain skipped.
- Complete pinned `built-ins/RegExp` is **1093/0/786/0/0**. The immediately
  preceding `0a880f3` release under the new policy is **1091/2/786/0/0**,
  proving exact two-file code movement.
- Local gates pass: fmt, Clippy `-D warnings`, all-target/all-feature tests
  including library **291/291**, builtins **548/548**, and Criterion smoke;
  release library **289/289**; wasm32; doctest **1/1**; Python tooling **137**
  with four expected missing-checkout skips. The pinned sparse checkout still
  has one unrelated absent staging fixture.
- GPT-5.6 production and admission reviewers are both `CLEAN`; production
  review requested parser fallback and diagnostic precedence tests, which were
  added before clean re-review. All reviewer sessions are closed and none
  built or cleaned the shared workspace.
- Ordinary CI `30307971537` passes both jobs. Full run `30307971322` passes all
  **35/35** jobs. Against immediate predecessor `30306477487`, 31 of 32 result
  artifacts are byte-identical. Only `built-ins` changes by **+2 pass / -2
  skip** to **16332/4297/3036/3/0**. Aggregate is
  **31953/5108/11405/3/0** over **48469** total and **37061** run.
- Final cleanup removed the A/B worktree/target, CI downloads, Python caches,
  Test262 temporary output, and Cargo output. No local process or reviewer
  session remains active.
- Next architecture unit is exact Unicode-ignoreCase `\b`/`\B`. It must make
  Rust and fancy routing agree on ECMAScript WordCharacters, repair the
  `CaptureCorrected` false-positive/false-negative prefilter assumptions, and
  preserve hostile no-match performance. Complex `/iv` classes containing
  `\w`/`\W` should remain a separate follow-up unless the same representation
  solves both without widening risk.

## Completed unit - character-only RegExp Unicode set algebra

- Feature commit:
  `eb65de67df4bc0fea6838f6b4d480a629f21ab6c fix(regexp): honor Unicode set pattern mode`
- Evidence commit:
  `0a880f3 docs: record Unicode set CI evidence`
- Root cause: RegExp normalization computed `unicode_mode = u || v`, but
  decimal escapes, identity escapes, and dot lowering retested only `u`.
  Valid `\p{...}` operands therefore lost their backslash in `v` mode and
  dot used legacy UTF-16 lowering. Those branches now use the shared mode.
  Unicode dot lowers to one scalar excluding all four ECMAScript
  LineTerminators unless global/local dotAll is active. The deliberately
  `u`-specific negated-property ignore-case workaround stays separate because
  `v` has different complement/case-fold ordering.
- Exact admission freezes 48 generated Test262 files covering every union,
  intersection, and subtraction pairing among characters, nested character
  classes, class escapes, and character property escapes. The 66 generated
  files using string properties or `\q{...}` remained gated until the current
  string-set unit.
- Focused Rust test and tooling admission test pass. Forced unfiltered audit
  of all 114 generated files improved from **32 pass / 82 fail** to **48 pass /
  66 fail**; the ordinary admitted run is **48/0/66** for that directory.
- Complete pinned `built-ins/RegExp` is **1091/0/788/0/0**. The immediately
  preceding `68d5b2b` release under the new policy is **1075/16/788/0/0**;
  therefore the code fix moves exactly 16 property-operand files. The other
  32 admitted character-only files already passed under the old release.
  Important tooling caveat: `test262_runner.py` currently ignores a `RUJA`
  environment variable and hardcodes `target/release/ruja`. The valid A/B run
  imported the module and assigned its `RUJA` global explicitly; an earlier
  environment-only old-binary run was discarded.
- Local gates pass: fmt, Clippy `-D warnings`, all-target/all-feature tests
  including library **291/291**, builtins **547/547**, and Criterion smoke;
  release library **289/289**; wasm32; doctest **1/1**; Python tooling **136**
  with four expected missing-checkout skips. The pinned sparse checkout run
  has one unrelated absent staging fixture.
- Initial GPT-5.6 audits identified separate next units: exact Unicode
  ignore-case word boundaries (high), `u`/`v` mutual exclusion (two files),
  complex `iv` classes containing `\w`/`\W`, and the sentinel collision.
  These are not combined into this character-property normalization unit.
  Final production and admission reviewers found Unicode dot LineTerminator,
  `/iv` documentation, and manifest-invariant gaps. All were fixed; both
  re-reviews are `CLEAN`. Every reviewer session is closed and none built or
  cleaned the shared workspace.
- Ordinary CI `30303993941` passes both jobs. Full run `30303993914` passes all
  **35/35** jobs. Against immediate predecessor `30300832056`, 30 of 32 result
  artifacts are byte-identical. `built-ins` moves exactly **+48 pass / -48
  skip** to **16330/4297/3038/3/0**.
- Annex B has one shared-runner contention timeout at **200/811/74/1/0**;
  independent current-release rerun reproduces clean **201/811/74/0/0**.
  Normalized aggregate is **31951/5108/11407/3/0** over **48469** total and
  **37059** run, exactly +48 pass/-48 skip from the preceding aggregate.
- Final cleanup removed the A/B worktree/target, CI downloads, Test262 temporary
  output, Python caches, and Cargo output. No reviewer or command session
  remains active.
- Next safest bounded unit is `u`/`v` RegExp flag mutual exclusion: reject both
  orders in the common flag validator and admit only
  `prototype/unicodeSets/uv-flags.js` plus `uv-flags-constructor.js`. Keep the
  larger exact `/iu`/`/iv` word-boundary routing fix as the following
  architecture unit; it must repair `CaptureCorrected` prefilter assumptions,
  not merely rewrite one source spelling.

## Completed unit - host-independent RegExp quantifier integers

- Feature commit:
  `d5b4c1c513920c7df250e5f95392807081fe504d fix(regexp): support arbitrary repeat bounds`

- Exact finite bounds now use `u128` or canonical decimal storage; infinity is
  distinct. Analysis and compiler accumulators saturate, and VM compilation
  emits one counter plus one body regardless of count magnitude.
- The lexer compares decimal ranges without machine conversion. Values above
  `u32::MAX` route directly to the non-delegated ECMAScript counter VM;
  validated braced repeats retry there only on `CompiledTooBig`. Syntax errors
  do not fallback.
- GPT-5.6 review found and the implementation fixed short-circuiting sibling
  traversal, ECMAScript legacy-brace/empty-group differences, missing typed
  `CompiledTooBig` retry on already-fancy patterns, mode-off oversized nullable
  repeat exposure, terminal repeat overcharging, and a public exact-count
  invariant. Successful capture/backreference/lookaround and legacy/`u`/`v`
  cases were added. Final re-review is `CLEAN`; all review sessions are closed.
  One earlier reviewer ran `cargo clean` while the parent
  test was linking; this was the apparent duplicate build event, not a second
  Codex parent session. Future read-only reviewers must be told explicitly not
  to build or clean the shared workspace.
- Local evidence: vendored full suite and doctests pass; root all-target/
  all-feature tests pass with library **291/291** and builtins **546/546**;
  root and vendored-library Clippy pass with `-D warnings`; rustfmt, wasm32,
  release library **289/289**, release build, doctest **1/1**, generated docs
  with 13 pre-existing warnings, and Python tooling **135** with four expected
  unavailable-checkout skips pass.
- On pinned Test262 `020cb74075849d1e404bbcdb62feb7a02e6966db`, complete
  `built-ins/RegExp` is **1043 pass / 0 fail / 836 skip / 0 timeout / 0
  error** over 1,879 files and 1,043 executions. The immediately preceding
  fixed-checkout baseline was **1042/1/836/0/0**; only
  `quantifier-integer-limit.js` moves.
- Ordinary CI `30298239835` passes both jobs. Full run `30298239883` passes all
  **35/35** jobs. Against immediate predecessor `30292332875`, 31 of 32 result
  artifacts are byte-identical; only `built-ins` moves by **+1 pass / -1
  fail**. Aggregate is **31903/5108/11455/3/0** over **48469** total and
  **37011** run.
- Next bounded RegExp candidate should be selected by a fresh failure audit.
  Prefer one isolated grammar or boundary family; do not combine nested `v`
  sets, sentinel collisions, and Unicode word boundaries into one unit.

## Completed unit - nullable RegExp match boundaries

- Feature commit:
  `7dd410259b67cc53b06c23951ada5897fd57a6b9 fix: honor nullable RegExp match boundaries`
- `CaptureCorrected` retains the Rust linear matcher as a no-match and
  leftmost-start prefilter. The bounded ECMAScript matcher now supplies the
  authoritative end and captures; find/capture iteration, replacement, and
  builtin `lastIndex` updates consume that end.
- Direct private tests cover `find_at`, `find_iter`, `captures_at`, and
  `captures_iter`, including nullable end disagreement, UTF-8 empty-match byte
  progress at 0/4/5, and a hostile nested-repeat input whose fancy-only path
  reaches the exact `BacktrackLimitExceeded` error while the hybrid path
  returns no match linearly. Public tests cover exec, prefixed search,
  global/sticky state, match/replace, Unicode/legacy compositions, and
  supplementary zero-width `matchAll` indices.
- Fixed-checkout release A/B is **1041/2/836/0/0** before and
  **1042/1/836/0/0** after over the same 1,879-file `built-ins/RegExp`
  directory. `nullable-quantifier.js` moves fail to pass; the sole remaining
  failure is `quantifier-integer-limit.js`.
- Local gates pass all-target/all-feature tests including Criterion smoke,
  debug library **287/287**, release library **285/285**, builtins **544/544**,
  wasm32, doctest **1/1**, rustfmt, Clippy `-D warnings`, generated docs with
  13 pre-existing warnings, and Python tooling **135** with four expected
  unavailable-checkout skips. GPT-5.6 final review is CLEAN.
- Ordinary CI `30289895334` passes both jobs. Full run `30289895937` passes
  all **35/35** jobs. Thirty result artifacts are byte-identical to immediate
  preceding run `30287220200`; built-ins changes exactly **+1 pass / -1
  fail** to **16281/4298/3086/3/0**. Annex B drops one preceding contention
  timeout and is byte-identical to clean run `30285089340` at
  **201/811/74/0/0**, SHA-256 `410da6b0...`. Corrected aggregate is
  **31902/5109/11455/3/0** over **48469** total and **37011** run.
- Next bounded unit is host-independent oversized RegExp quantifiers. Do not
  clamp or expand them. Parse bounds in a target-width-independent form,
  preserve finite versus infinity, make repeat analysis saturating, route
  only validated oversized/compiled-too-big repeats to a non-delegated
  counter VM, keep O(AST) compilation and existing work/branch limits, and
  test `2^32-1`, `2^32`, `2^53-1`, exact/open/bounded/lazy, nested overflow,
  captures, Unicode/legacy input, and wasm32.
- Cleanup remains mandatory every turn. Remove all downloaded artifacts,
  A/B worktrees/targets, logs, caches, and Cargo output before replying; no
  later session owns the cleanup debt.

## Completed unit - direct with-object Reference bases

- Feature commit:
  `8366015aa4cb7aa941d6b2c17e1dd2fec449b9dd perf: store with-object reference bases directly`
- `ReferenceBase::ObjectEnvironment` now stores the validated binding-object
  `GcIdx` directly. It remains separate from ordinary object property
  References because missing-binding, delete, assignment, and implicit call
  receiver semantics differ. Global identifiers still use `Environment`.
- Get, put, delete, call-this extraction, and GC root visitation reconstruct or
  visit the same index without allocation. Proxy wrapper identity and
  execution-Realm primitive wrappers are preserved.
- Direct tests cover the real resolver constructor, exact root identity,
  malformed internal environment payloads, forced GC in Proxy
  has/get/set/delete/call paths, foreign-Realm primitive boxing, and strict
  global fallback. GPT-5.6 final runtime review was CLEAN; its documentation
  review's two low findings were fixed.
- Local gates pass: Clippy `-D warnings`, rustfmt, all-target/all-feature tests
  with 286 library tests and 61 with tests, release library 286/286, wasm32,
  doctest 1/1, Python tooling 135/135, and rustdoc with the 13 pre-existing
  broken-link warnings. Debug symbols were disabled only for the local
  all-target test build because the disk otherwise filled during linking; CI
  passed with its normal profile.
- Pinned current/preceding expressions/class/with output is byte-identical at
  **10290/0/5360/15650**, SHA-256
  `9334343e68ac0571407ea9886c67643fb0c5ec7512696dd74afca5ef506172e5`.
  The current supported language subset is **12761/0/7678/20439**.
- The 30,000-operation with-object Reference benchmark measured 203.78 ms on
  preceding source and 201.83-208.30 ms on current source; Criterion found no
  significant change.
- Ordinary CI `30276201220` passes both jobs. Full Test262 run `30276195375`
  passes all **35/35** jobs. Thirty-one result artifacts match preceding run
  `30269385090` immediately; the downloaded CI binary removes one Annex B
  contention timeout and reproduces the preceding **201/811/74/0/0** artifact
  byte-for-byte. Corrected aggregate remains
  **31901/5110/11455/3/0** over **48469** total and **37011** run.
- Next audited allocation candidate is direct object storage for deferred raw
  property names. Keep `ToPropertyKey` timing deferred and treat it as a
  separate medium-risk unit.

## Completed unit - generic Array flat and flatMap

- Feature commit:
  `f9d3ef296a189734712b66ae9cb0140d0552c512 fix: make Array flattening generic and bounded`
- CI portability commit:
  `d346c05ef05cc811b30d19c46029c2d3952be379 test: tolerate unavailable test262 checkout`
- `flat` and `flatMap` now share an iterative specification-shaped
  `FlattenIntoArray` with generic `ToObject`, one source-length snapshot,
  method-specific depth/mapper validation, `ArraySpeciesCreate`, live
  `HasProperty`/`Get`, nested `IsArray`/length, mapper, and
  `CreateDataPropertyOrThrow` ordering.
- Explicit traversal frames own GC pin suffixes. Every source index consumes
  fuel; cyclic `Infinity` depth is bounded after 512 observable active-path
  replays when fuel is unbounded. Proxy wrappers normalize to terminal target
  identity, and active identity checks use a ref-count map rather than an
  O(depth) scan.
- Exact admission keeps `flat` and `flatMap` independent: one flat path and
  nine flatMap paths. Fixed-checkout A/B is old-policy/old-binary
  **18/15/10**, new-policy/old-binary **20/23/0**, old-policy/new-binary
  **33/0/10**, and new-policy/new-binary **43/0/0**.
- Local gates pass: all targets/features with **193/193** library tests,
  release library **192/192**, builtins **524/524**, arguments **15/15**,
  tooling **124/124**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, and wasm32 checking.
- GPT-5.6 reviewers Russell
  (`019f834f-c8ac-7b01-901f-2b1b8fa2e36f`) and James
  (`019f834f-c9fe-72d0-b940-4af81ae1496d`) returned `CLEAN` after cycle,
  complexity, allocation, and admission follow-ups; both are closed. Coder and
  Umans routes were not used.
- First ordinary CI `29808658860` found only the CI-only inaccessible
  `/root/test262` permission probe. Commit `d346c05` fixed that portability
  issue; ordinary CI `29809211211` passes both jobs.
- Feature full matrix `29808658857` passes all **33/33** jobs. Canonical
  artifacts: `/tmp/ruja-array-flat.29808658857`.
- Aggregate: **31262 pass / 5698 fail / 11502 skip / 5 timeout / 0 error /
  48467 total / 36960 pass-or-fail executed**.
- Against `/tmp/ruja-array-filter.29804173104.rerun`, 29 of 30 result files are
  byte-identical. Only `test262_built-ins_result.txt` changes by **+25 pass /
  -15 fail / -10 skip** to **15652/4887/3124/5/0**.
- The downloaded release binary independently reproduces direct flat and
  flatMap **43/43** on fixed Test262
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`.

## Completed unit - generic Array forEach

- Feature commit:
  `24458d56c82462d0503c0ac7882b2359c7d95315 fix: make Array forEach generic and live`
- `Array.prototype.forEach` now uses generic `ToObject`, one
  `LengthOfArrayLike` snapshot, callback validation, and live
  `HasProperty`/`Get` traversal with callback `thisArg` and receiver/index
  arguments.
- Native-frame roots survive getters, Proxy traps, callbacks, and forced GC;
  abrupt and fuel exits restore pin depth. Every logical index, including a
  hole, consumes one fuel unit.
- Exact admission freezes five feature-gated paths and keeps future siblings
  gated. Tooling is **125/125**.
- Fixed-checkout A/B is old-policy/old-binary **90/94/5/1**,
  new-policy/old-binary **91/98/0/1**, old-policy/new-binary **185/0/5/0**,
  and new-policy/new-binary **190/0/0/0** (pass/fail/skip/timeout).
- Adjacent TypedArray forEach remains **42/42**.
- Local gates pass: all targets/features with **195/195** library tests,
  release library **194/194**, builtins **526/526**, arguments **15/15**,
  tooling **125/125**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, and wasm32 checking.
- GPT-5.6 runtime reviewer McClintock
  (`019f83a7-5428-7442-b062-92f1b5a9c781`) and admission reviewer Raman
  (`019f83a7-55e4-7680-944c-4114e05d91fe`) returned `CLEAN` after one stale
  limitations entry was fixed; both sessions are closed. The coder route was
  not used.
- Ordinary CI `29812349225` passes both jobs. Full matrix `29812349142`
  passes all **33/33** jobs; canonical artifacts are at
  `/tmp/ruja-array-for-each.29812349142`.
- Aggregate: **31362 pass / 5604 fail / 11497 skip / 4 timeout / 0 error /
  48467 total / 36966 pass-or-fail executed**.
- Against `/tmp/ruja-array-flat.29808658857`, 29 of 30 result files are
  byte-identical. Only built-ins changes by **+100 pass / -94 fail / -5 skip /
  -1 timeout** to **15752/4793/3119/4/0**.
- The downloaded release binary independently reproduces Array forEach
  **190/190** and TypedArray forEach **42/42** on fixed Test262
  `9e61c12835c5e4a3bdba93850427e6742c4f64c4`.

## Completed Array join unit

Generic Array join shipped in `219fba4`:

- live generic `Get` traversal after `ToObject`, one length snapshot, and
  separator coercion
- cycle suppression begins after separator coercion, preserving finite
  separator re-entry while bounding cyclic element conversion
- GC roots across length, separator, indexed gets, and element conversions
- exact per-index fuel and checked string reservation
- exact four-path admission; direct Array join **23/23**, TypedArray join
  **32/32**, tooling **126/126**
- fixed-checkout A/B: old-policy/old-binary **15/4/4**,
  new-policy/old-binary **16/7/0**, old-policy/new-binary **19/0/4**, and
  new-policy/new-binary **23/0/0**
- local gates pass: all targets/features with **197/197** library tests,
  release library **196/196**, builtins **528/528**, arguments **15/15**,
  tooling **126/126**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, and wasm32 checking
- GPT runtime and admission reviews are CLEAN
- CI `29817687979` and full Test262 `29817687917` pass; downloaded artifacts
  aggregate to **31372/5598/11493/4/0** over **48467** total, with only
  `built-ins` changing from the preceding run
- the downloaded binary reproduces Array join **23/23** and TypedArray join
  **32/32**

## Completed recent units

Generic Array map shipped in `346965f`:

- generic live `HasProperty`/`Get` traversal after `ToObject`, one length
  snapshot, callback validation, and `ArraySpeciesCreate(length)`
- same-index `CreateDataPropertyOrThrow`, hole preservation, mutation,
  inherited values, Proxy species targets, and method-Realm results
- operation-wide roots plus current-value and mapped-result roots across
  callback and definition re-entry
- exact creation-plus-index fuel and abrupt pin-depth cleanup
- exact nine-path admission; direct Array map **216/216**, TypedArray map
  **85/85**, tooling **127/127**
- fixed-checkout A/B: old-policy/old-binary **95/111/9/1**,
  new-policy/old-binary **96/119/0/1**, old-policy/new-binary **207/0/9/0**,
  and new-policy/new-binary **216/0/0/0**
- GPT runtime review is CLEAN
- local gates pass: all targets/features with **198/198** library tests,
  release library **198/198**, builtins **529/529**, arguments **15/15**,
  tooling **127/127**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, and wasm32 checking
- CI `29821941785` and full Test262 `29821941873` pass; downloaded artifacts
  aggregate to **31493/5487/11484/3/0** over **48467** total, with only
  `built-ins` changing from the preceding run
- the downloaded binary reproduces Array map **216/216** and TypedArray map
  **85/85**

Generic Array reduce is complete:

- generic ascending traversal after `ToObject`, one length snapshot, and
  callback validation
- live omitted-initial accumulator discovery plus `HasProperty`/`Get` for
  every later index
- O(1) LIFO accumulator root replacement across callback results and later
  observable property work
- exact per-index fuel and abrupt pin-depth cleanup
- exact five-path admission; direct Array reduce **260/260**, TypedArray reduce
  **50/50**, tooling **128/128**
- fixed-checkout A/B: old-policy/old-binary **89/166/5**,
  new-policy/old-binary **90/170/0**, old-policy/new-binary **255/0/5**, and
  new-policy/new-binary **260/0/0**
- local gates pass: all targets/features with **200/200** library tests,
  release library **200/200**, builtins **530/530**, arguments **15/15**,
  tooling **128/128**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, and wasm32 checking; rustdoc reports only the 13 pre-existing
  broken-link warnings
- GPT runtime review is CLEAN; the documentation review's sole pending-evidence
  finding is resolved by the completed runs below
- feature commit `5362a2c` passes CI `29825824540` and full Test262
  `29825824539`; downloaded artifacts aggregate to
  **31664/5321/11479/3/0** over **48467** total, with only `built-ins`
  changing by **+171 pass / -166 fail / -5 skip**
- the downloaded binary reproduces Array reduce **260/260** and TypedArray
  reduce **50/50**

## Completed unit - generic Array reduceRight

- generic descending traversal after `ToObject`, one length snapshot, and
  callback validation
- live omitted-initial accumulator discovery plus `HasProperty`/`Get` for
  every remaining index
- exclusive-upper-bound indexing examines zero once without u64 underflow
- O(1) LIFO accumulator root replacement, exact per-index fuel, and abrupt
  pin-depth cleanup
- exact five-path admission; direct Array reduceRight **260/260**, TypedArray
  reduceRight **50/50**, tooling **129/129**
- fixed-checkout A/B: old-policy/old-binary **94/161/5**,
  new-policy/old-binary **95/165/0**, old-policy/new-binary **255/0/5**, and
  new-policy/new-binary **260/0/0**
- GPT runtime review is CLEAN; admission review confirms the exact metadata
  map and identified the documentation updates now applied
- local gates pass: all targets/features with **202/202** library tests,
  release library **202/202**, builtins **531/531**, arguments **15/15**,
  tooling **129/129**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, and wasm32 checking; rustdoc reports only the 13 pre-existing
  broken-link warnings
- runtime and admission/documentation GPT reviews are CLEAN
- feature commit `61cd755` passes CI `29829395637` and full Test262
  `29829395686`; downloaded artifacts aggregate to
  **31830/5160/11474/3/0** over **48467** total, with only `built-ins`
  changing by **+166 pass / -161 fail / -5 skip**
- the downloaded binary reproduces Array reduceRight **260/260** and
  TypedArray reduceRight **50/50**

## Completed unit - generic Array reverse

- generic in-place pair traversal after `ToObject` and one length snapshot
- exact lower Has/Get then upper Has/Get observation order
- all four existence states preserve holes and apply strict Set/Delete order
- operation roots plus pair-local lower/upper value roots across Proxy traps,
  forced GC, partial mutation, and abrupt completion
- one fuel unit per pair with no materialized-index length cap
- exact two-path admission; direct Array reverse **18/18**, TypedArray reverse
  **22/22**, tooling **130/130**
- fixed-checkout A/B: old-policy/old-binary **7/9/2**,
  new-policy/old-binary **8/10/0**, old-policy/new-binary **16/0/2**, and
  new-policy/new-binary **18/0/0**
- detached `methods-called-as-functions.js` passes as a forced-gate diagnostic
  but remains skipped by normal broad feature policy
- local gates pass: all targets/features with **204/204** library tests,
  release library **204/204**, builtins **532/532**, arguments **15/15**,
  tooling **130/130**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, and wasm32 checking; rustdoc reports only the 13 pre-existing
  broken-link warnings
- runtime and admission/documentation GPT reviews are CLEAN
- feature commit `2aa46ec` passes CI `29833297247` and full Test262
  `29833297271`; downloaded artifacts aggregate to
  **31841/5151/11472/3/0** over **48467** total, with only `built-ins`
  changing by **+11 pass / -9 fail / -2 skip**
- the downloaded binary reproduces Array reverse **18/18**, TypedArray reverse
  **22/22**, and the passing forced-gate detached-method diagnostic

## Completed unit - generic Array toReversed

- generic `ToObject` plus one `LengthOfArrayLike` snapshot
- method-Realm intrinsic `ArrayCreate` before indexed access, deliberately
  ignoring `constructor` and `Symbol.species`
- live descending `Get` with no `HasProperty`; holes become own `undefined`
  result properties and the source remains unmodified except for getter effects
- receiver, boxed source, result, and fetched values remain rooted across
  observable work; exact-cap allocation failure precedes indexed Gets, and GC
  retry preserves the source and foreign method Realm
- one loop fuel plus one property-definition fuel unit per copied index, with
  zero loop fuel for empty receivers
- exact one-path admission; direct Array toReversed **17/17**, TypedArray
  toReversed **9/9**, tooling **131/131**
- fixed-checkout A/B: old-policy/old-binary **8/8/1**,
  new-policy/old-binary **9/8/0**, old-policy/new-binary **16/0/1**, and
  new-policy/new-binary **17/0/0**
- local gates pass: all targets/features with **207/207** library tests,
  release library **206/206**, builtins **533/533**, arguments **15/15**,
  tooling **131/131**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, and wasm32 checking; rustdoc reports only the 13 pre-existing
  broken-link warnings
- runtime review is CLEAN after its allocation-boundary test suggestion was
  added; admission review confirms the exact metadata map and the documentation
  finding is resolved here
- feature commit `0e90184` passes CI `29845649747` and full Test262
  `29845649723`; downloaded artifacts aggregate to
  **31850/5143/11471/3/0** over **48467** total, with only `built-ins`
  changing by **+9 pass / -8 fail / -1 skip**
- the downloaded binary reproduces Array toReversed **17/17** and TypedArray
  toReversed **9/9**

## Completed unit - generic Array toSpliced

- generic `ToObject` plus one `LengthOfArrayLike` snapshot; argument count
  distinguishes no arguments, one `start` argument, and explicit `undefined`
  `skipCount`
- method-Realm intrinsic `ArrayCreate` after argument coercion, deliberately
  ignoring `constructor` and `Symbol.species`
- live ascending prefix and suffix `Get` operations around inserted arguments;
  discarded indices are not read and holes become own `undefined` properties
- receiver, arguments, boxed source, result, and copied values remain rooted
  across coercion, allocation, observable traps, and forced GC; exact-cap
  allocation failure precedes indexed Gets and GC retry preserves a foreign
  method Realm
- one loop fuel plus one property-definition fuel unit per result index, with
  balanced roots on normal and abrupt completion
- exact one-path admission; direct Array toSpliced **30/30**, adjacent Array
  splice **81/81**, tooling **132/132**
- fixed-checkout A/B: old-policy/old-binary **17/12/1**,
  new-policy/old-binary **18/12/0**, old-policy/new-binary **29/0/1**, and
  new-policy/new-binary **30/0/0**
- local gates pass: all targets/features with **210/210** library tests,
  release library **209/209**, builtins **534/534**, arguments **15/15**,
  tooling **132/132**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, and wasm32 checking; rustdoc reports only the 13 pre-existing
  broken-link warnings
- GPT runtime and admission reviews are CLEAN
- feature commit `174f006` passes CI `29850160241` and full Test262
  `29850160482`; downloaded artifacts aggregate to
  **31863/5131/11470/3/0** over **48467** total, with only `built-ins`
  changing by **+13 pass / -12 fail / -1 skip**
- the downloaded binary reproduces Array toSpliced **30/30** and Array splice
  **81/81**

## Completed unit - generic Array toLocaleString

- dedicated generic `ToObject` plus one `LengthOfArrayLike` snapshot replaces
  the incorrect Array `toString` alias
- RuJa's implementation-defined `,` separator is appended before each later
  live `Get`; nullish elements become empty fields, while other elements receive
  zero-argument `toLocaleString` invocation and returned-value `ToString`
- receiver, boxed source, current element, selected method, and localized result
  remain rooted across Proxy traps, calls, conversion, thrown values, and forced
  GC; direct, indirect, and join cross-recursion share one balanced marker stack
- one fuel unit per captured index, fallible intermediate string growth, and no
  unchecked `Arc<str>`-to-`String` copy; final Arc publication retains the known
  runtime-wide infallible allocation limitation
- exact four-path admission; direct Array toLocaleString **12/12**, Array join
  **23/23**, TypedArray toLocaleString **39/39**, Object toLocaleString
  **12/12**, tooling **133/133**, and the forced detached-method diagnostic pass
- fixed-checkout A/B: old-policy/old-binary **3/5/4**,
  new-policy/old-binary **5/7/0**, old-policy/new-binary **8/0/4**, and
  new-policy/new-binary **12/0/0**
- local gates pass: all targets/features with **212/212** library tests,
  release library **211/211**, builtins **535/535**, arguments **15/15**,
  tooling **133/133**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, and wasm32 checking; rustdoc reports only the 13 pre-existing
  broken-link warnings
- GPT runtime review is CLEAN after its unchecked intermediate-allocation and
  thrown-object survival findings were fixed; admission review is CLEAN on the
  fixed checkout
- feature commit `2dd3041` passes CI `29883661773` and full Test262
  `29883661759`; downloaded artifacts aggregate to
  **31872/5126/11466/3/0** over **48467** total, with only `built-ins`
  changing by **+9 pass / -5 fail / -4 skip**
- the downloaded binary reproduces every focused and adjacent cohort

## Completed unit - TypedArray toLocaleString semantic hardening

- feature commit `0a5d7ea` preserves ValidateTypedArray/internal-length
  semantics while switching each element to primitive `GetV`, a zero-argument
  call in the non-ECMA-402 runtime, and returned-value `ToString`
- mutable global `Number` and `BigInt` replacement no longer redirects method
  lookup; foreign method Realms still select primitive prototypes and generated
  errors
- receiver, current value, method, result, and thrown values survive observable
  GC; abrupt exits restore pin depth, each captured index consumes one fuel
  unit, and intermediate output growth is fallible
- direct TypedArray locale remains **39/39**, Array locale remains **12/12**,
  TypedArray-constructor inheritance remains **2/2**, and the release diagnostic
  changes from `1|2|bad` to `1|0|1`
- local gates pass: all targets/features with **214/214** library tests,
  release library **213/213**, builtins **535/535**, arguments **15/15**,
  tooling **133/133**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, and wasm32 checking; rustdoc reports only the 13 pre-existing
  broken-link warnings
- both GPT-5.6 reviews found no runtime defect; one review identified the
  pre-existing directory-prefix admission drift risk as a separate tooling
  follow-up
- CI `29886017567` and full matrix `29886017568` pass; downloaded artifacts at
  `/tmp/ruja-typed-array-to-locale-string-29886017568-final` aggregate to the
  unchanged **31872/5126/11466/3/0** over **48467** total and all 30 result
  files are byte-identical to full run `29883661759`
- the downloaded release binary reproduces diagnostic `1|0|1`, direct
  TypedArray locale **39/39**, and combined adjacent coverage **14/14**

## Completed unit - exact TypedArray locale admission and pinned Test262

- tooling commit `3f31c29` replaces the directory-prefix exception with an
  exact 39-file manifest and per-file feature metadata shared by runner and
  analyzer
- tooling asserts live directory equality, features, includes, flags, negative
  metadata, disjointness, invalid paths, extra features, and future/outside
  sibling rejection; full tooling remains **133/133**
- ordinary CI and every full-matrix consumer now checkout pinned Test262
  revision `9e61c12835c5e4a3bdba93850427e6742c4f64c4`; full setup runs the focused
  live admission test before constructing the matrix
- direct TypedArray locale remains **39/39** and adjacent combined coverage
  remains **14/14**; the policy admits no new current file
- local gates pass: all targets/features with **214/214** library tests,
  release library **213/213**, builtins **535/535**, arguments **15/15**,
  tooling **133/133**, rustfmt, Clippy `-D warnings`, wasm32, YAML parsing, and
  Python bytecode compilation
- three GPT-5.6 reviews are CLEAN after their CI live-metadata, directory
  equality, and shared-corpus pinning findings were fixed
- CI `29888462270` passes both jobs; full run `29888462280` passes setup's new
  pinned admission verification and all 33 jobs
- downloaded artifacts at
  `/tmp/ruja-typed-array-locale-admission-29888462280-final` aggregate to the
  unchanged **31872/5126/11466/3/0** over **48467** total; all 30 result files
  are byte-identical to full run `29886017568`

## Completed unit - TypedArray join resource hardening and exact admission

- feature commit `7605669` preserves ValidateTypedArray, internal-length
  snapshot, separator conversion, and live indexed access while rooting only
  the receiver and observed separator across coercion
- one fuel unit is consumed per captured index; Number and BigInt direct-native
  forced-GC paths, abrupt cleanup, resize/detach ordering, and method-Realm
  generated errors are covered
- checked streaming growth replaces the infallible `Vec<String>` plus join;
  final Arc publication retains the known runtime-wide limitation
- exact admission freezes all 32 direct files and their per-file metadata;
  tooling checks live equality, disjointness, symmetry, invalid paths, extra
  features, and future/outside siblings
- direct join remains **32/32**, combined adjacent coverage remains **94/94**,
  and tooling remains **133/133**
- local gates pass: all targets/features with **216/216** library tests,
  release library **215/215**, builtins **536/536**, arguments **15/15**,
  tooling **133/133**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, wasm32, YAML parsing, and Python bytecode compilation;
  rustdoc reports only the 13 pre-existing broken-link warnings
- independent GPT-5.6 runtime and admission reviews are CLEAN
- feature commit `7605669` passes CI `29890470545` and all 33 jobs in full run
  `29890470558`; downloaded artifacts at
  `/tmp/ruja-typed-array-join-29890470558-final` aggregate to unchanged
  **31872/5126/11466/3/0** over **48467** total
- all 30 result files are byte-identical to full run `29888462280`; the
  downloaded release binary reproduces direct join **32/32** and adjacent
  combined coverage **94/94**

## Completed unit - exact TypedArray toString admission

- tooling commit `739e3ff` replaces the duplicated four-path/four-feature union
  with one exact manifest and per-file feature metadata shared by runner and
  analyzer
- live tooling covers the parent `toString.js` plus three nested files,
  includes, flags, negative metadata, disjointness, invalid paths, extra
  features, and future/outside siblings; full setup now checks all three exact
  TypedArray string classes
- direct-native custom-join and fallback-tag forced-GC tests preserve receiver
  liveness and pin balance; builtins cover primitive boxing and foreign method-
  Realm TypeError without changing the runtime algorithm
- direct Test262 remains **4/4**, combined TypedArray string coverage is
  **75/75**, focused runtime coverage is **5/5**, and tooling is **133/133**
- local gates pass: all targets/features with **217/217** library tests,
  release library **216/216**, builtins **537/537**, arguments **15/15**,
  documentation **1/1**, rustfmt, Clippy `-D warnings`, release build, wasm32,
  YAML parsing, and Python bytecode compilation; rustdoc reports only the 13
  pre-existing broken-link warnings
- two GPT-5.6 reviews are CLEAN; tooling commit `739e3ff` passes CI
  `29892602601` and all 33 jobs in full run `29892602512`
- the initial `annexB` artifact had two contention timeouts; the same downloaded
  binary plus pinned sparse corpus reproduced baseline **201/811/74/0/0**, and
  rerunning only that shard produced a clean replacement artifact
- final artifacts at
  `/tmp/ruja-typed-array-to-string-admission-29892602512-rerun` aggregate to
  unchanged **31872/5126/11466/3/0** over **48467** total; all 30 result files
  are byte-identical to full run `29890470558`
- the downloaded release binary reproduces direct **4/4** and combined
  TypedArray string coverage **75/75**

## Completed unit - TypedArray search loop fuel

- feature commit `147f2b2` adds one fuel debit before every visited logical
  index in TypedArray `includes`, `indexOf`, and `lastIndexOf`; fromIndex
  coercion and empty-range returns remain outside loop charging
- direct-native tests cover N-1 abort, exact completion, immediate match,
  nonempty empty ranges, zero-length views, Number and BigInt, fuel remainder,
  and pin balance
- pinned Test262 remains **45/45** includes, **43/43** indexOf, and **42/42**
  lastIndexOf, or **130/130** combined
- local gates pass: all targets/features with **218/218** library tests,
  release library **217/217**, builtins **537/537**, arguments **15/15**,
  tooling **133/133**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, and wasm32 checking; rustdoc reports only the 13 pre-existing
  broken-link warnings
- both GPT-5.6 reviews are CLEAN after zero-length coverage was moved into the
  correct focused test and BigInt exact exhaustion was asserted
- feature commit `147f2b2` passes CI `29895173924` and all 33 jobs in full run
  `29895173852`; downloaded artifacts at
  `/tmp/ruja-typed-array-search-fuel-29895173852-final` aggregate to unchanged
  **31872/5126/11466/3/0** over **48467** total
- all 30 result files are byte-identical to full run `29892602512`; the
  downloaded release binary reproduces combined search coverage **130/130**

## Completed unit - nullish ToObject semantics

- feature commit `401a73a` makes the shared `Vm::to_object` abstract operation
  throw for `null` and `undefined` rather than allocating nonstandard wrappers
- ordinary `Object(nullish)` and `new Object(nullish)` still create fresh
  objects in the active function Realm, while a distinct `NewTarget` retains
  its constructor-derived prototype path; sloppy `this`, `for...in`,
  `Object.assign` sources, and `Object.prototype.toString` retain their
  separate nullish branches
- focused pinned Test262 moves from **942/4/6** to **946/0/6**, and complete
  `built-ins/Object` moves from **3295/4/112** to **3299/0/112**
- local gates pass: all targets/features with **218/218** library tests,
  release library **217/217**, builtins **537/537**, arguments **15/15**,
  tooling **133/133**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, generated documentation, and wasm32 checking; rustdoc reports
  only the 13 pre-existing broken-link warnings
- two GPT-5.6 reviews found no runtime defect; one review caught and resolved an
  incorrect prototype expectation in the newly added `Object.create` success
  test
- feature commit `401a73a` passes ordinary CI `29899362133`
- full matrix `29899362112` passes all 33 jobs; downloaded artifacts at
  `/tmp/ruja-nullish-to-object-29899362112-final` aggregate to
  **31876/5122/11466/3/0** over **48467** total and **36998** executed
- against full run `29895173852`, all 29 non-built-ins result files are
  byte-identical and `built-ins` alone moves by **+4 pass / -4 fail**
- the downloaded release binary reproduces focused **946/0/6** and complete
  Object **3299/0/112**

## Completed unit - residual language early errors

- feature commit `f1c421d` restricts Annex B labelled function declarations to
  ordinary sloppy functions and rejects labelled generator, async-function,
  and async-generator declarations
- raw and escaped class names whose decoded value is `await` are rejected
  throughout a Module source goal, including nested ordinary functions, while
  valid Script contexts remain accepted
- exact admission freezes five parse-negative files with per-file features and
  module status shared by runner and analyzer; full setup validates their live
  pinned metadata before matrix scheduling
- exact Test262 is **5/5**, the labelled directory is **21/0/3**, parser tests
  are **51/51**, and tooling is **134/134**
- local gates pass: all targets/features with **219/219** library tests,
  release library **218/218**, builtins **537/537**, arguments **15/15**,
  documentation **1/1**, rustfmt, Clippy `-D warnings`, release build, generated
  documentation, Python bytecode compilation, workflow YAML parsing, and
  wasm32 checking; rustdoc reports only the 13 pre-existing broken-link warnings
- two GPT-5.6 reviews are CLEAN after the Module rule was extended from
  top-level code to nested ordinary functions
- feature commit `f1c421d` passes ordinary CI `29903293887`; full-matrix setup
  passes the new live admission preflight
- full matrix `29903293969` passes all 33 jobs; downloaded artifacts at
  `/tmp/ruja-language-early-errors-29903293969-final` aggregate to
  **31881/5122/11461/3/0** over **48467** total and **37003** executed
- against run `29899362112`, 28 result files are byte-identical;
  `language/expressions` changes by **+1 pass / -1 skip** and
  `language/statements` by **+4 pass / -4 skip**
- the downloaded release binary reproduces exact **5/5** and labelled
  **21/0/3**

## Completed unit - Bound Function name and length metadata

- feature commit `75c030a` implements specification-shaped own `length` and
  `name` properties, including Proxy `[[GetOwnProperty]]`/`Get` order, numeric
  truncation without coercing non-numbers, bound-name chaining, exact
  descriptors, and prototype lookup after configurable-name deletion
- the wrapper is allocated and pinned before observable metadata operations;
  focused regressions cover forced GC, all abrupt stages, exact heap caps,
  signed zero, no-coercion, inherited metadata, own-key shape, and balanced
  pins
- exact admission is **9/9**; the complete bind directory moves from
  **84/7/9** to **93/0/7**, and `Function.prototype` is **223/40/46**
- local gates pass with **221/221** all-feature library tests, **220/220**
  release library tests, **539/539** builtins, **15/15** arguments, **135/135**
  tooling, **1/1** doctest, rustfmt, Clippy `-D warnings`, release build,
  rustdoc with 13 pre-existing warnings, wasm32, YAML, and Python compilation
- GPT-5.6 re-review is CLEAN after deleted-name inheritance and abrupt pin/cap
  findings were fixed
- CI `29907748052` and all 33 full-matrix jobs in `29907748376` pass
- downloaded artifacts at
  `/tmp/ruja-bound-function-metadata-29907748376-final` aggregate to
  **31890/5115/11459/3/0** over **48467** total and **37005** run; only
  `built-ins` changes against `29903293969`, exactly **+9 pass / -7 fail / -2
  skip**
- the downloaded binary reproduces exact **9/9** and complete bind **93/0/7**
- GPT analysis confirms ordinary Bound `[[Call]]` remains recursive,
  unmetered, uncapped, and quadratic in layered arguments; iterative call
  dispatch is the next priority

## Completed unit - iterative and fuel-bounded Bound calls

- feature commit `026ea21` replaces recursive ordinary Bound `[[Call]]`
  forwarding with one iterative Bound/Proxy traversal, one linear argument
  materialization, and one fuel debit per followed edge; follow-up `c64076f`
  makes the shared trap-array item/prototype root reservation fallible
- the cumulative argument cap is enforced before Proxy apply lookup; the
  innermost bound `this`, current-Realm trap argument Array, foreign target
  Realm, thrown identity, and Bound apply-trap behavior are preserved
- call inputs and traversal state stay rooted through observable work and
  collecting allocations; root and trap-call vector reservations are fallible
  and all tested exits restore the incoming pin depth
- local gates pass all targets/features with **222/222** library tests,
  release library **221/221**, builtins **539/539**, arguments **15/15**,
  tooling **135/135**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, generated documentation, Python/YAML checks, and wasm32;
  rustdoc retains only the 13 pre-existing broken-link warnings
- semantic/resource reviews are CLEAN; documentation review found the shared
  Array helper's remaining infallible pin growth, and re-review is CLEAN after
  `c64076f` added exact reserve plus injected-failure coverage
- feature CI `29912648216` and full run `29912648078` pass; follow-up CI
  `29916090205` and all 33 jobs in full run `29916090227` also pass
- final downloaded artifacts at
  `/tmp/ruja-value-array-roots-29916090227-final` aggregate to unchanged
  **31890/5115/11459/3/0** over **48467** total and **37005** run; all 30
  result files are byte-identical to feature run `29912648078`
- the downloaded binary reproduces Function.prototype **223/40/46** and
  Promise **442/0/287**

## Completed unit - iterative and rooted instanceof

- feature commit `419501e` roots both operands and the constructor prototype,
  iterates Bound/default-handler forwarding and prototype traversal, charges
  each logical edge once, and performs `[[GetPrototypeOf]]` before comparison
- observable Proxy apply traps remain normal calls; a separate 128-frame
  native-call cap makes recursive `Reflect.apply` re-entry catchable without
  reducing the existing 512 interpreted-frame cap
- direct regressions cover 50,000 Bound layers, 10,000 transparent wrappers,
  actual apply recursion, exact fuel and ordering, forced GC, stale slots,
  abrupt identity, foreign Realms, allocation failure, and cleanup
- local gates pass all targets/features with **223/223** library tests,
  release library **222/222**, builtins **539/539**, arguments **15/15**,
  tooling **135/135**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, generated docs, Python/YAML checks, and wasm32
- semantic and resource GPT-5.6 reviews are CLEAN after all reported P1/P2
  findings were fixed
- CI `29922123540` and all 33 jobs in full run `29922124267` pass
- artifacts at `/tmp/ruja-instanceof-29922124267-final` aggregate to unchanged
  **31890/5115/11459/3/0** over **48467** total and **37005** run; all 30
  result files are byte-identical to run `29916090227`
- the downloaded binary reproduces `instanceof` **50/0/4**, forced skipped
  coverage **4/4**, Proxy `getPrototypeOf` **19/19**, and Function.prototype
  **223/40/46**

## Completed milestone - Reference-record routing

The original Reference-record milestone is already complete in commits
`f63145d`, `00994c7`, and `bbfa6f2`: the current combined Reference/with/
compound diagnostic is **663/0/1**, and the compiler/VM audit found no
remaining ordinary expression-evaluation bypass. Do not restart that migration.

## Completed unit - fallible Proxy prototype validation roots

- commit `cff25cc` reserves every root directly owned by Proxy
  `[[GetPrototypeOf]]` and nested `[[IsExtensible]]`, and reserves deferred
  scratch before `pin -> push`
- `IsExtensible` replaces unbounded per-layer Boolean storage with an O(1)
  delayed consistency summary without changing abrupt or invariant priority
- exact failpoints cover input, target/handler, trap, returned prototype,
  scratch, expected root, later deferred cleanup, null expected, foreign Realm,
  exact fuel/order, and a 1,024-layer validating chain
- local gates pass all targets/features with **224/224** library tests,
  release library **223/223**, builtins **539/539**, arguments **15/15**,
  tooling **135/135**, documentation **1/1**, rustfmt, Clippy `-D warnings`,
  release build, generated docs, Python/YAML checks, and wasm32
- both GPT-5.6 reviews are CLEAN after exact-site and delayed-abrupt test
  findings were fixed
- CI `29927666067` and all 33 jobs in full run `29927657329` pass
- artifacts at `/tmp/ruja-proxy-prototype-roots-29927657329-final` aggregate to
  unchanged **31890/5115/11459/3/0** over **48467** total and **37005** run;
  all 30 result files are byte-identical to run `29922124267`
- the downloaded binary reproduces Proxy getPrototypeOf/isExtensible **31/31**
  and adjacent `instanceof` **50/0/4**

## Completed unit - fallible shared property traversal state

- commit `0bac2a2` reserves initial object identities and caller roots before
  publishing pins, and reserves each new directed edge, node identity, and
  reached root before commit
- Get, HasProperty, public and ordinary Set, and inherited Proxy `GetMethod`
  share the same fallible constructor while preserving fuel, credit, ordinary
  cycle rejection, and the 512 Proxy replay guard
- lazy `for-in` now persists directed edges, traced roots, Proxy presence, and
  replay count across pulls; fresh-key cyclic Proxies can no longer reset the
  guard on every `next()`
- terminal iterators release traversal collection capacity; abrupt retries
  re-observe Proxy prototype traps because each failed `next()` operation has
  already completed abruptly
- exact failpoints and regressions cover every reservation site, initial roots,
  primitive/nullish exclusions, retry atomicity, foreign Realms, zero-fuel
  priority, direct and cross-pull cycles, GC slot reuse, and cleanup
- local gates pass all targets/features with **225/225** library tests,
  release library **224/224**, builtins **539/539**, arguments **15/15**,
  tooling **135/135**, rustfmt, Clippy `-D warnings`, release build, generated
  docs, and wasm32; rustdoc has only 13 pre-existing broken-link warnings
- both GPT-5.6 re-reviews are CLEAN after completed-iterator capacity and
  Proxy retry semantics were resolved
- CI `29947430510` and all 33 jobs in full run `29947430421` pass
- artifacts at `/tmp/ruja-property-traversal-29947430421-final` aggregate to
  unchanged **31890/5115/11459/3/0** over **48467** total and **37005** run;
  all 30 result files are byte-identical to run `29927657329`
- the downloaded binary reproduces direct Proxy get/has/set/getPrototypeOf/
  getOwnPropertyDescriptor plus language for-in at **190/0/37**

## Completed unit - fallible lazy for-in key state

- commit `0686d0e` reserves filtered string snapshots before publication and
  visited names after an existing descriptor but before insertion
- symbol-only snapshots, absent descriptors, and already visited prototype
  duplicates do not reserve; terminal completion releases both capacities
- visited reservation failure preserves the consumed cursor and leaves the
  mark uncommitted, matching existing fuel/descriptor abrupt progression and
  allowing a same-name prototype key on retry
- exact failpoints cover atomic retry, Proxy observation, shadowing,
  duplicates, fuel priority, foreign Realms, cleanup, and capacity release
- local gates pass all targets/features with **226/226** library tests,
  release library **225/225**, builtins **539/539**, arguments **15/15**,
  tooling **135/135**, rustfmt, Clippy `-D warnings`, release build, generated
  docs, and wasm32; rustdoc has only 13 pre-existing broken-link warnings
- both GPT-5.6 implementation reviews and the documentation review are CLEAN
- CI `29951588373` and all 33 jobs in full run `29951587187` pass
- artifacts at `/tmp/ruja-for-in-key-state-29951587187-final` aggregate to
  unchanged **31890/5115/11459/3/0** over **48467** total and **37005** run;
  all 30 result files are byte-identical to run `29947430421`
- the downloaded binary reproduces direct Proxy get/has/set/getPrototypeOf/
  getOwnPropertyDescriptor plus language for-in at **190/0/37**

## Completed unit - fallible Proxy ownKeys entry collection

- commit `fe14b77` requests trap-result Vec capacity after each indexed fuel,
  Get, and String/Symbol validation, then publishes the key
- duplicate validation remains a complete-list second pass; membership is
  checked before requesting `IndexSet` capacity for a new key
- empty lists and duplicate seen entries skip their corresponding growth;
  Symbols remain collected and checked before consumer filtering
- exact failpoints cover partial-state discard, caller retry, abrupt/type/fuel
  priority, duplicate countdown, Symbol identity, foreign Realms, nested frame
  cleanup, and for-in snapshot atomicity
- local gates pass all targets/features with **227/227** library tests,
  release library **226/226**, builtins **539/539**, arguments **15/15**,
  tooling **135/135**, rustfmt, Clippy `-D warnings`, release build, generated
  docs, and wasm32; rustdoc has only 13 pre-existing broken-link warnings
- both GPT-5.6 implementation reviews and the corrected documentation review
  are CLEAN
- CI `29955284791` and all 33 jobs in full run `29955284788` pass
- artifacts at `/tmp/ruja-proxy-ownkeys-entry-state-29955284788-final`
  aggregate to unchanged **31890/5115/11459/3/0** over **48467** total and
  **37005** run; all 30 result files are byte-identical to run `29951587187`
- the downloaded binary reproduces Proxy/Reflect ownKeys, Object key/name/
  Symbol consumers, and language for-in at **211/0/60**

## Completed unit - fallible Proxy ownKeys validation frames

- commit `3903c54` reserves pending-frame capacity, then the exact
  `current`/`target` roots, before `pin -> push`
- publication remains after trap-list, duplicate, and `IsExtensible` work and
  before nested target traversal; transparent forwarding needs no frame while
  an empty trapped result does
- exact failpoints cover frame and roots, the real GC-pin reserve path, retry,
  priority, foreign Realms, second-frame cleanup, forced GC, a 1,024-layer
  trapped chain, for-in snapshot atomicity, and all depth baselines
- local gates pass all targets/features with **228/228** library tests,
  release library **227/227**, builtins **539/539**, arguments **15/15**,
  tooling **135/135**, rustfmt, Clippy `-D warnings`, release build, generated
  docs, and wasm32; rustdoc has only 13 pre-existing broken-link warnings
- both GPT-5.6 implementation reviews and the documentation review are CLEAN
- CI `29959362979` and all 33 jobs in full run `29959362973` pass
- the initial annexB artifact had two contention timeouts; the same downloaded
  binary and exact pinned corpus rerun is byte-identical to the baseline
  **201/811/74/0/0**
- the corrected artifact set at
  `/tmp/ruja-proxy-ownkeys-frame-state-29959362973-final` aggregates to unchanged
  **31890/5115/11459/3/0** over **48467** total and **37005** run; all 30 files
  match run `29955284788`
- the downloaded binary reproduces the selected Proxy/Reflect ownKeys, Object
  key/name/Symbol, and language for-in cohort at **211/0/60**

## Completed unit - fallible Proxy ownKeys direct roots

- commit `633b3d8` reserves the operation input, each Proxy target/handler
  pair, an object trap-result list, and an object-valued length before their
  corresponding pins
- operation ownership precedes revocation and fuel; layer ownership follows
  revocation and the edge fuel debit; list and length roots follow their
  respective successful validation/Get boundaries
- primitive Values contribute no roots and consume no reservation failure;
  nullish trap forwarding creates no trap-result state
- exact and real failpoints cover all four sites, priority, retry, foreign
  Realms, forced GC, published-outer-frame cleanup, and for-in snapshot
  atomicity at every site
- local gates pass all targets/features with **229/229** library tests,
  release library **228/228**, builtins **539/539**, arguments **15/15**,
  tooling **135/135**, rustfmt, Clippy `-D warnings`, release build, generated
  docs, and wasm32; rustdoc has only 13 pre-existing broken-link warnings
- both GPT-5.6 final reviews are CLEAN
- CI `29963410587` and all 33 jobs in full run `29963410566` pass
- artifacts at
  `/tmp/ruja-proxy-ownkeys-direct-roots-29963410566-final` aggregate to unchanged
  **31890/5115/11459/3/0** over **48467** total and **37005** run; all 30 files
  are byte-identical to the corrected `29959362973` baseline
- the downloaded binary reproduces the selected Proxy/Reflect ownKeys, Object
  key/name/Symbol, and language for-in cohort at **211/0/60**

## Completed unit - fallible Proxy ownKeys post-validation collections

- commit `0740941` reserves the non-extensible target-key set only after all
  descriptor and omission validation, and reserves filtered output only when
  an accepted key would actually exceed Vec capacity
- excluded Symbols, absent/non-enumerable descriptors, empty targets, and
  spare-capacity pushes do not consume a reservation failure
- exact failpoints cover descriptor and mismatch priority, second growth,
  partial retry, fuel, foreign Realms, reverse nested-frame logs, thrown marker
  identity, for-in snapshot atomicity, and layered retry
- local gates pass all targets/features with **230/230** library tests,
  release library **229/229**, builtins **539/539**, arguments **15/15**,
  tooling **135/135**, doctest **1/1**, rustfmt, Clippy `-D warnings`, release
  build, generated docs, and wasm32; rustdoc has 13 pre-existing warnings
- the final GPT-5.6 implementation review is CLEAN
- CI `29967042158` and all 33 jobs in full run `29967042192` pass
- artifacts at
  `/tmp/ruja-proxy-ownkeys-post-validation-29967042192-final` aggregate to
  unchanged **31890/5115/11459/3/0** over **48467** total and **37005** run;
  all 30 files are byte-identical to run `29963410566`
- the downloaded binary reproduces the selected Proxy/Reflect ownKeys, Object
  key/name/Symbol, and language for-in cohort at **211/0/60**

## Completed unit - growth-only Proxy ownKeys entry failpoints

- commit `fbef166` skips both entry reservation helpers while collection
  capacity remains and reaches the failpoint only at `len == capacity`
- helper tests explicitly preallocate spare capacity, check every reported
  slot, and fail at the exact full boundary for both Vec and IndexSet
- integration tests cover actual second growth, two-key spare reuse, preserved
  failure in the next fresh collection, and all prior ordering/retry behavior
- local gates pass all targets/features with **231/231** library tests,
  release library **230/230**, builtins **539/539**, arguments **15/15**,
  tooling **135/135**, doctest **1/1**, rustfmt, Clippy `-D warnings`, release
  build, generated docs, and wasm32; rustdoc has 13 pre-existing warnings
- the final GPT-5.6 review is CLEAN
- CI `29970600535` and all 33 jobs in full run `29970600531` pass
- artifacts at `/tmp/ruja-proxy-ownkeys-growth-only-29970600531-final`
  aggregate to unchanged **31890/5115/11459/3/0** over **48467** total and
  **37005** run; all 30 files are byte-identical to run `29967042192`
- the downloaded binary reproduces the selected Proxy/Reflect ownKeys, Object
  key/name/Symbol, and language for-in cohort at **211/0/60**

## Completed unit - fallible ordinary own-key collections

- commit `055b36e` reserves the index, String, and Symbol staging vectors plus
  the final result Vec and duplicate IndexSet only at actual growth
- final membership is checked first; a new key reserves both final collections
  before publication, while duplicate Array `length` and spare capacity consume
  no reservation failure
- exact regressions cover first/second growth, spare reuse, every producer,
  exclusions, exact fuel, foreign Realms, Proxy ordering, lazy for-in retry and
  snapshot atomicity, and balanced execution/native/pin depths
- local gates pass all targets/features with **233/233** library tests,
  release library **232/232**, builtins **539/539**, arguments **15/15**,
  modules **31/31**, tooling **135/135**, doctest **1/1**, rustfmt, Clippy
  `-D warnings`, release build, generated docs, and wasm32; rustdoc has 13
  pre-existing warnings
- CI `29973887440` and all 33 jobs in full run `29973887424` pass
- artifacts at
  `/tmp/ruja-ordinary-ownkeys-collections-29973887424-final` aggregate to
  unchanged **31890/5115/11459/3/0** over **48467** total and **37005** run;
  all 30 files are byte-identical to run `29970600531`

## Completed unit - fallible own-key consumer results

- commit `2930f21` reserves actual-growth result vectors for Object
  keys/values/entries, own names/Symbols, and Reflect ownKeys
- values reserve returned roots before pin/publication; entries independently
  reserve pair elements, pair/result roots, outer publication, and inner/outer
  Array presence storage
- the shared Realm-explicit Value-array path reserves roots and uses fallible
  dense-presence construction; the obsolete String-array shortcut no longer
  bypasses VM allocation
- Reflect ownKeys now returns an Array from the native callee Realm
- exact regressions cover capacity, first/second growth, all six APIs,
  descriptor/Get/fuel priority, empty/filtered paths, entries layers, retry,
  foreign Array/RangeError Realms, injected roots, exact-cap GC of ephemeral
  values/pairs, and balanced pin/context/native depths
- local gates pass all targets/features with **235/235** library tests,
  release library **234/234**, builtins **539/539**, arguments **15/15**,
  modules **31/31**, tooling **135/135**, doctest **1/1**, rustfmt, Clippy
  `-D warnings`, release build, generated docs, and wasm32; rustdoc has 13
  pre-existing warnings
- both final GPT-5.6 reviews are CLEAN
- focused eight-directory Test262 is **244/0/68**
- CI `29977240776` and all 33 jobs in full run `29977240759` pass
- the original annexB artifact shifted one pass to a timeout; the downloaded
  binary and exact pinned corpus rerun is baseline **201/811/74/0/0** and
  byte-identical to run `29973887424`
- corrected artifacts at
  `/tmp/ruja-ownkey-consumers-29977240759-corrected` aggregate to unchanged
  **31890/5115/11459/3/0** over **48467** total and **37005** run; all 30 files
  match run `29973887424`
- the downloaded binary reproduces focused **244/0/68**, persisted as
  `focused-ownkey-consumers.txt` beside the corrected artifacts

## Completed unit - fallible Proxy descriptor traversal state

- commit `2918d70` reserves all ten operation, layer, trap, pending-frame,
  validation-descriptor, descriptor-object, and descriptor-field sites before
  publication
- pending frames reserve only on actual growth; transparent forwarding skips
  trap/pending sites, primitive fields skip field-root sites, and the
  `undefined` trap-result path's absent and immediate non-configurable targets
  skip validation-descriptor roots
- callability, revocation, fuel, conversion, invariant, reverse-validation,
  Realm, caller, retry, forced-GC, and cleanup ordering are covered
- local gates pass all targets/features with **237/237** library tests,
  release library **236/236**, builtins **539/539**, arguments **15/15**,
  modules **31/31**, tooling **135/135**, doctest **1/1**, rustfmt, Clippy
  `-D warnings`, release build, generated docs, and wasm32; rustdoc has 13
  pre-existing warnings
- both GPT-5.6 final implementation reviews are CLEAN
- documentation commit `30d9882` records the evidence and the final GPT-5.6
  documentation review is CLEAN
- CI `29980403698` and all 33 jobs in full run `29980403702` pass
- artifacts at `/tmp/ruja-proxy-descriptor-29980403702-final` aggregate to
  unchanged **31890/5115/11459/3/0** over **48467** total and **37005** run;
  all 30 files are byte-identical to the corrected `29977240759` baseline
- the downloaded binary reproduces the 16-directory Proxy/Reflect/Object and
  for-in cohort at **656/0/60**, persisted as
  `focused-proxy-descriptor.txt` beside the artifacts

## Completed unit - fallible descriptor materialization and definition publication

- commit `ce280cb` replaces the temporary normalized descriptor object with a
  presence-aware internal record observed once in specification field order
- object-valued descriptor fields are rooted as observed; defineProperties
  retains records through its complete first pass before the first definition
- FromPropertyDescriptor, getOwnPropertyDescriptors, and Proxy defineProperty
  reserve directly owned maps, vectors, descriptor objects, and roots before
  publication, and use the current Realm's immutable Object prototype
- getOwnPropertyDescriptors obtains own keys before result allocation; Proxy
  descriptor materialization follows revocation, fuel, trap lookup, and
  callability and precedes trap invocation
- exact failpoints cover all growth/root sites, spare capacity, first/later
  failure, false/transparent Proxy paths, two-pass ordering, foreign Realms,
  cleanup, retry, real root reservation, and fresh value/get/set liveness
  through cap-triggered GC
- local gates pass all targets/features with **246/246** library tests,
  release library **246/246**, builtins **539/539**, arguments **15/15**,
  modules **31/31**, tooling **135/135**, doctest **1/1**, rustfmt, Clippy
  `-D warnings`, release build, generated docs, and wasm32; rustdoc has 13
  pre-existing warnings
- GPT-5.6 reviewers Maxwell and Lovelace returned CLEAN and are closed
- GPT-5.6 documentation reviewer Poincare returned CLEAN after three wording
  corrections and is closed
- local and downloaded-binary focused descriptor Test262 is **2457/0/24**
- CI `29986403996` and all 33 jobs in full run `29986403979` pass
- the original annexB artifact shifted one pass to a contention timeout; the
  downloaded binary and exact pinned corpus rerun restores **201/811/74/0/0**
- corrected artifacts at
  `/tmp/ruja-descriptor-materialization-29986403979-final` aggregate to
  unchanged **31890/5115/11459/3/0** over **48467** total and **37005** run;
  all 30 files are byte-identical to run `29980403702`

## Completed unit - fallible ordinary property storage publication

- commit `0a9c3f8` selects one virtual/property/dense/custom/arguments storage
  plan and reserves actual `props`, `items`, and `present` growth before commit
- existing keys, spare capacity, normal dense migration, compatible boxed
  String virtual properties, completed Proxy traps, and TypedArray integer
  indices skip irrelevant ordinary reservations
- dense/custom/sparse Array state, `sparse_max`, mapped parameter updates and
  detachment, length synchronization, and cache invalidation occur only after
  direct storage preflight succeeds
- direct TypedArray integer indices use element semantics; resolved Proxy
  targets and descriptor fields remain pinned through observable coercion;
  Module Namespace complete descriptors use String-only export rules,
  `SameValue`, and ordinary Symbol handling
- exact tests cover three-container growth at every site, spare/replacement and
  migration paths, failure atomicity, partial defineProperties, foreign Realms,
  Proxy/fuel priority, exotic exclusions, retry, and exact-cap forced GC
- local gates pass all targets/features with **251/251** library tests, release
  library **251/251**, builtins **539/539**, arguments **15/15**, modules
  **31/31**, tooling **135/135**, doctest **1/1**, rustfmt, Clippy `-D
  warnings`, release build, generated docs, and wasm32; rustdoc has 13 existing
  warnings
- GPT-5.6 reviewers Socrates and Volta returned CLEAN after all findings were
  fixed; both agents are closed and made no edits
- local and downloaded-binary focused Test262 is **1897/0/13**
- CI `30186299215` and all 33 jobs in full run `30186299205` pass
- artifacts at `/tmp/ruja-ordinary-storage-30186299205-final` aggregate to
  unchanged **31890/5115/11459/3/0** over **48467** total and **37005** run;
  29 files match run `29986403979`, and its exact-corpus annexB rerun is
  byte-identical to the current clean annexB artifact
- the guarantee covers directly owned target containers only; BigInt/value
  preparation, boxed-String canonicalization, Array length keys, IC strings,
  TypedArray byte vectors, ordinary Set, and GC worklists remain separate

## Completed unit - fallible Array length mutation

- commit `75401b9` reserves actual operation-root, length-property, dense-item,
  and presence growth before mutation
- documentation commit `3f36dd8` records architecture, limitations, local and
  remote verification, corrected full artifacts, and the next narrow scope
- shrink finds the highest non-configurable blocker in one scan and removes
  higher configurable custom indices with one retain pass; no deletion scratch
  allocation remains
- blocker rollback restores `blocker + 1`, preserves required partial deletion,
  and applies deferred `writable: false` at the specified point
- sparse shrink, rollback, equality, and growth never expand dense holes;
  virtual length materializes only for persistent `writable: false`
- one VM-owned canonical length key removes per-operation key allocation
- exact tests cover all reservation and spare-capacity paths, retry, atomic
  preflight, deletion order, partial rollback, fuel, foreign Realms, completed
  and transparent Proxies, cleanup, and forced GC across both conversions
- local gates pass all targets/features with **256/256** library tests, release
  library **255/255**, builtins **539/539**, arguments **15/15**, modules
  **31/31**, tooling **135/135**, doctest **1/1**, rustfmt, Clippy `-D
  warnings`, release build, generated docs, and wasm32; rustdoc has 13 existing
  warnings
- GPT-5.6 reviewers Wegener and Parfit returned CLEAN after sparse equality and
  canonical-key follow-ups
- fixed four-directory Test262 is unchanged at **4731/4/121/0/0** over
  **4856** total and **4735** run against the preceding downloaded binary
- ordinary CI `30188817875` passes
- all 33 jobs in full run `30188817855` pass; its original annexB artifact has
  one runner-contention timeout, while the same downloaded binary and pinned
  corpus rerun is byte-identical to baseline **201/811/74/0/0**
- corrected artifacts at `/tmp/ruja-array-length-30188817855-final` aggregate
  to unchanged **31890/5115/11459/3/0** over **48467** total and **37005** run;
  all 30 corrected files match run `30186299205`
- the downloaded release binary reproduces focused **4731/4/121/0/0**, stored
  as `focused-array-length.txt` beside the artifacts

## Completed unit - fallible Array index Set and borrowed inline cache

- implementation commit `c49e0fd` routes direct dense, custom, sparse, and
  Arguments index assignment through one representation-aware fallible Set
  publisher while reusing the existing PropertyKey
- documentation commit `fe7522c` records final local, CI, focused, and
  corrected full-matrix evidence
- mapped Arguments reruns the same-receiver `[[Set]]` preamble at recursive
  ordinary and transparent Proxy entries and updates receiver mappings only
  after successful `[[DefineOwnProperty]]` publication
- the nested inline cache borrows lookup/invalidation keys, tracks an exact
  4,096-entry cap, prunes empty buckets, and treats insertion allocation as
  optional; cap replacement reserves before clearing so failure retains all
  old entries
- exact tests cover every map/vector/cache growth site, spare capacity,
  overwrite, migration, descriptor preservation, sparse length, atomic retry,
  Realm errors, Proxy ordering, mapped-Arguments partial effects, recursive
  re-entry, cache retention/pruning/cap/clear, and cleanup
- local gates pass all targets/features with **261/261** library tests,
  release library **260/260**, builtins **539/539**, arguments **15/15**, Array
  index **50/50**, modules **31/31**, tooling **135/135**, doctest **1/1**,
  rustfmt, Clippy `-D warnings`, release build, generated docs, and wasm32;
  rustdoc has 13 existing warnings
- GPT-5.6 reviewers McClintock and Chandrasekhar returned CLEAN after the cache
  cap-replacement retention finding was fixed; both agents are closed
- local current, preceding release, and downloaded CI binaries produce
  byte-identical focused **4054/4/243/0/0** over the five Array/Reflect
  Set/arguments/assignment directories
- CI `30192642319` and all 33 jobs in full run `30192642310` pass
- artifacts at `/tmp/ruja-array-index-set-30192642310-final` aggregate to
  unchanged **31890/5115/11459/3/0** over **48467** total and **37005** run;
  29 files are byte-identical to run `30188817855`, and clean Annex B is
  byte-identical to that run's exact-corpus correction
- three-run release workload comparison covers dense overwrite/append, sparse
  Set, cache read hit, and invalidate hit/miss without measured regression

## Completed unit - fallible ordinary non-index Set receiver publication

- implementation commit `5ce09c8` routes direct ordinary receiver creation
  and replacement through Set-mode actual-growth preflight and borrowed
  post-commit cache invalidation
- documentation commit `2515d3b` records final local, CI, focused, and full
  artifact evidence
- boxed String virtual `length` and UTF-16 indices reject receiver shadowing
  without descriptor materialization; Module Namespace value-only definitions
  use live-binding `SameValue`, including NaN and signed zero
- public and abstract Set failure preserves property and cache state; retry,
  spare capacity, existing descriptor attributes, transparent Proxy fuel,
  completed Proxy suppression, global receivers, foreign Realm errors, and
  cleanup have exact regression coverage
- local gates pass all targets/features with **264/264** library tests,
  release library **263/263**, builtins **539/539**, arguments **15/15**, Array
  index **50/50**, modules **31/31**, tooling **135/135**, doctest **1/1**,
  rustfmt, Clippy `-D warnings`, release build, generated docs, and wasm32;
  rustdoc has 13 existing warnings
- GPT-5.6 reviewers Lovelace and Locke returned CLEAN; final reviewer Singer
  returned CLEAN after precommit IC-atomicity and documentation corrections;
  all agents are closed
- local current, preceding release, and downloaded CI binaries produce
  byte-identical focused **1683/0/81/0/0** over Reflect Set, String,
  assignment, and Module Namespace directories
- CI `30195285329` and all 33 jobs in full run `30195285326` pass
- artifacts at `/tmp/ruja-ordinary-non-index-set-30195285326-final` aggregate
  to unchanged **31890/5115/11459/3/0** over **48467** total and **37005**
  run; all 30 files are byte-identical to clean run `30192642310`
- receiver overwrite/create Criterion and three-run preceding-release
  comparison show no measured regression

## Completed unit - Object integrity levels

- implementation commit `5a3e7d5` routes seal, freeze, isSealed, and isFrozen
  through one Proxy-aware integrity pipeline with presence-aware internal
  descriptors
- direct descriptor inspection is attribute-only; ordinary descriptors update
  in place and dense Array values move into reserved custom storage without
  cloning existing BigInts
- Array length writability, deleted Arguments length, initial and promoted
  mapped-Arguments detachment, descriptor prototype pollution, Proxy partial
  effects, and Module Namespace TDZ/re-export order are corrected
- non-fixed TypedArray views over resizable ArrayBuffers reject
  preventExtensions; fixed views over fixed buffers or growable
  SharedArrayBuffers retain specification behavior
- direct predicates precharge the ordinary own-key fuel budget before scanning
  and avoid key/descriptor materialization; Namespace re-export edges use
  allocation-free Brent cycle detection and per-edge fuel
- integrity, preventExtensions Proxy-layer, and trap roots reserve before
  pinning; Array map growth remains fallible and retryable after required
  PreventExtensions and earlier-key effects
- local gates pass all targets/features with **266/266** library tests,
  release library **265/265**, builtins **541/541**, tooling **135/135**,
  doctest **1/1**, rustfmt, Clippy `-D warnings`, release build, generated docs,
  and wasm32; rustdoc has 13 existing warnings
- GPT-5.6 review passes found and drove fixes for BigInt clones, predicate
  performance/fuel, Namespace TDZ/bounds, promoted Arguments, TypedArray
  prevention, root preflight, and benchmark isolation; final reviewer Zeno is
  CLEAN
- current, preceding release, and downloaded CI binaries produce byte-identical
  Object integrity plus Namespace focused **257/0/20/0/0**; forced variable-
  length TypedArray preventExtensions staging improves from **0/2** to **2/2**
- isolated Criterion point estimates are about object freeze **5.30 ms**, Array
  freeze **10.18 ms**, object isFrozen **0.85 ms**, and Array isFrozen **0.93
  ms** for 10k entries; repeated preceding-release wall workloads show no
  measured regression
- CI `30201495431` and all 33 jobs in full run `30201495450` pass
- original full Annex B moved two passes to contention timeouts; the same
  downloaded binary and pinned corpus restores clean **201/811/74/0/0**
- corrected artifacts at `/tmp/ruja-integrity-level-30201495450-final`
  aggregate to unchanged **31890/5115/11459/3/0** over **48467** total and
  **37005** run; all 30 corrected files are byte-identical to run
  `30195285326`

## Completed unit - shared immutable BigInt values

- runtime `Value::BigInt` now stores `Arc<BigInt>`; `Value::bigint` and
  `From<BigInt>` are the preferred constructors
- public `Vm::to_bigint() -> BigInt` remains source-compatible; internal
  coercion shares storage
- arithmetic, bitwise, shifts, BigInt statics, boxing, TypedArray/DataView,
  Atomics, serde, compiler constants, and Temporal construction are migrated
- equality and Map/Set hashing remain value-based; no copy-on-write mutation or
  global interning was introduced
- direct tests cover 16K-digit pointer sharing, 4K-digit Map/Set, boxing,
  mapped Arguments freeze, Realm crossing, binary views, arithmetic, and serde
- local gates: library **267/267**, builtins **541/541**, BigInt **24/24**,
  release library **267/267**, tooling **135/135**, doctest **1/1**, fmt,
  Clippy `-D warnings`, wasm32, release, docs; 13 existing rustdoc warnings
- focused pinned Test262 A/B: BigInt language/builtins **496/0/44**, binary
  views/constructors **93/0/0**, identical counts on current and preceding
  release binaries
- Criterion: 16K-digit Value clone **24.6-27.6 ns**; small arithmetic 10K
  **45.3-53.0 ms**. 64K-digit property reads 100K improve **1.05s to 0.74s**;
  small arithmetic wall time remains **0.07s**
- limitation remains explicit: BigInt limbs and Arc control blocks are outside
  VM heap accounting and can still host-OOM; AST-to-constant transfer clones
  once; direct enum payload construction is an alpha API change
- reviewers Fermat and Singer supplied preimplementation audits; final GPT-5.6
  reviewer Arendt is CLEAN after one benchmark-reproduction documentation fix
- implementation commit `d8b4486`; evidence/documentation commits `6ca9c09`
  and the final post-CI docs commit
- CI `30203903239` and all 33 jobs in full run `30203903224` pass
- original Annex B artifact has one contention timeout; downloaded-binary rerun
  restores **201/811/74/0/0**
- corrected artifacts at `/tmp/ruja-shared-bigint-30203903224-final-v1`
  aggregate to unchanged **31890/5115/11459/3/0** over **48467** total and
  **37005** run; all 30 corrected files match the preceding clean run
- downloaded CI binary reproduces focused **496/0/44** and binary-view
  **93/0/0**

## Completed unit - compact canonical numeric PropertyKeys

- canonical array-index names are represented as inline `u32` on 64-bit
  targets; 32-bit targets retain Arc-backed indices to preserve map density
- private nested representation keeps `PropertyKey` exactly the size of
  `Arc<str>`: 16 bytes on x86_64 and 8 bytes on wasm32. Earlier 24-byte native
  and 12-byte wasm layouts were rejected after GPT-5.6 review found global
  map-density regression
- stack decimal views preserve string-compatible Hash/Eq and materialize Arc
  strings only at JavaScript-visible boundaries
- Number-derived canonical indices and generated Array/RegExp/String/JSON/
  TypedArray definitions use the compact constructors; object computed Get/Set
  retain the structured key instead of formatting and reparsing
- ownKeys, Object key consumers, JSON source/reviver paths, Proxy trap lists,
  for-in snapshots, method names, cache invalidation, and object-environment
  references treat inline indices as string keys
- direct tests cover size, cross-representation hash lookup, signed zero,
  noncanonical/boundary Number names, ordering, JSON, Proxy strings/Symbols,
  reads, writes, and deletes
- local gates: debug library **270/270**, release library **268/268**, builtins
  **541/541**, array-index **52/52**, tooling **135/135**, doctest **1/1**,
  complete integration suite, fmt, Clippy `-D warnings`, and wasm32 pass
- focused pinned Test262 Object key consumers, Reflect/Proxy ownKeys, JSON,
  TypedArray ownKeys, and property accessors are **263/0/31/0/0**; current and
  preceding release output is byte-identical
- retained map microbenchmarks isolate numeric and ordinary string lookup; one
  high-load quick run measured about 273 us versus 151-155 us per 10k lookups,
  or about 12 ns extra stack-format/hash cost per numeric lookup. End-to-end
  samples are not evidence because the two-CPU host load was 9-14
- GPT-5.6 reviewer Hume was clean. Chandrasekhar found the 24-byte layout,
  missed structured-key paths, benchmark gap, and public enum issue; all four
  drove the compact private-repr redesign and retained benchmarks. Both agents
  are closed
- final GPT-5.6 reviewer Ramanujan is CLEAN. Aquinas found wasm32 key growth
  and Arc replacement; conditional 32-bit storage, compile-time size equality,
  and Arc identity coverage fixed both, then Aquinas returned CLEAN. Both final
  agents are closed
- implementation commit `98d97b9` passes CI `30207697825` and all 33 jobs in
  full run `30207697847`
- original Annex B artifact has one contention timeout; downloaded-binary rerun
  restores **201/811/74/0/0**
- corrected artifacts at `/tmp/ruja-property-key-30207697847-final` aggregate
  to unchanged **31890/5115/11459/3/0** over **48467** total and **37005** run;
  all 30 corrected files match the preceding clean full run exactly
- evidence/documentation commit follows `98d97b9`; per user preference its
  docs-only Actions run does not need to be watched

## Completed unit - computed read-modify-write PropertyKey handoff

- ordinary computed compound, logical, and update lowering no longer emits a
  standalone `ToPropertyKey` before `MakePropertyRef`
- canonical numeric names reach compact structured coercion directly; on
  x86_64 this removes temporary String and Arc allocation, while wasm32 removes
  only the redundant opcode/value handoff and retains its Arc-backed key
- simple assignment, destructuring/loop targets, delete, super, private, and
  environment/with References are unchanged because their timing or base model
  differs
- direct bytecode regression requires three `MakePropertyRef` operations and
  zero `ToPropertyKey` operations for compound/logical/update fixtures
- integration tests cover signed zero, array-index upper boundaries,
  non-index names, null-base ordering, Proxy get/set, one key coercion, and
  forced GC during key coercion, get, RHS, and set
- pinned focused Test262 compound/logical/update/super/with at `f3766ec` is
  **926/0/23/0/0**; that release and preceding release output is byte-identical
- quick Criterion A/B: numeric 30k **100.89 ms to 100.06 ms**; string control
  **99.78 ms to 100.86 ms**. Treat roughly 1% shifts as timer noise; opcode and
  allocation-path removal are deterministic
- GPT-5.6 auditors Pauli and Noether found no Reference correctness gap. Pauli
  confirmed this exact three-site unit and identified boxed Reference clones as
  the next separate allocation scope. Noether independently measured current
  Reference-adjacent corpus at **851/0/4** on newer test262 and found four
  already-correct generator early-error files skipped only by broad policy
- local gates: all-feature library **271/271**, release library **269/269**,
  builtins **541/541**, operators **130/130**, tooling **135/135**, doctest
  **1/1**, all-target, wasm32, Clippy `-D warnings`, fmt, release build, and docs
  pass; rustdoc retains 13 existing warnings
- final GPT-5.6 correctness reviewer Dirac is CLEAN after requiring ephemeral
  base/key GC coverage, object-to-Symbol coercion across all three forms, and a
  stale compiler-comment correction
- final GPT-5.6 performance reviewer Erdos is CLEAN after narrowing all
  allocation claims to x86_64 and documenting wasm32's retained Arc-backed key
- implementation commit `f3766ec` passes CI `30210419512` and all 33 jobs in
  full run `30210419518`
- original Annex B artifact has two contention timeouts; downloaded-binary
  rerun restores **201/811/74/0/0**
- corrected artifacts at `/tmp/ruja-computed-ref-30210419518-final` aggregate
  to unchanged **31890/5115/11459/3/0** over **48467** total and **37005** run;
  all 30 corrected files match the preceding clean run exactly
- downloaded CI binary reproduces focused **926/0/23** byte-for-byte
- evidence documentation commit follows `f3766ec`; per user preference its
  docs-only Actions run does not need to be watched

## Completed unit - generator update-expression early-error admission

- exact manifest and tooling changes admit only four audited generator
  update-expression parse-negative files; no parser/runtime code changed
- all four directories pass **142/142**, the adjacent Reference cluster is
  **930/0/19**, and the supported subset is **12761/0/7678**
- local Rust/static gates pass with
**271/271** debug library, **269/269** release library, **135/135** tooling,
  all-target, wasm32, Clippy, fmt, and docs
- GPT-5.6 correctness reviewer Ptolemy is CLEAN; documentation reviewer Bacon's
  three stale-history/count findings are fixed, and its admission-scope verdict
  is CLEAN
- commit `d810343` passes CI `30213116749` and all 33 full jobs in
  `30213116898`
- raw full aggregate is **31893/5115/11455/4/0**; downloaded-binary Annex B
  rerun restores **201/811/74/0/0**, giving corrected
  **31894/5115/11455/3/0** over **48467** total and **37009** pass-or-fail run
- 28/30 result files are byte-identical to the preceding run; expressions is
  exactly **+4 pass/-4 skip**, and Annex B differs only by contention timeout
- all downloaded artifacts, local result logs, and pinned worktrees were
  deleted after evidence extraction

## Completed unit - borrowed Reference consumers

Complete the borrowed Reference-consumer unit. The implementation removes
defensive record and boxed-value clones, centralizes trace/count/pin traversal,
and adds exact root plus abrupt-cleanup tests. Local gates pass with **273/273**
debug library, **271/271** release library, **135/135** tooling, all-target,
wasm32, Clippy, fmt, and docs. Pinned Reference A/B is byte-identical at
**930/0/19**, supported language is **12761/0/7678**, and Criterion shows no
regression. GPT-5.6 reviewers Goodall and Boyle are CLEAN. Implementation
commit `68e3766` passes CI
`30215764494` and all 33 jobs in full run `30215764513`. Raw aggregate is
**31892/5115/11455/5/0**; downloaded-binary Annex B rerun restores
**201/811/74/0/0**, yielding corrected **31894/5115/11455/3/0** over
**48467** total and **37009** pass-or-fail run. The other 29 files match
preceding run `30214416788` byte-for-byte. Artifacts and sparse worktree were
deleted.

## Next unit

After closing the current native indexed PropertyKey evidence and cleanup,
continue with initial Reference Box representation or non-index Number
formatting;
JavaScript-visible key String allocation, TypedArray byte conversion, JSON
containers, Error strings, GC root enumeration, BigInt host allocation, and
mark worklists remain independent.

Completed implementation commit `00b5496`: all 24 retained reads use
`GetValueKeepReference`;
the opcode reserves one root suffix for resolved names and the two-suffix peak
for raw names before moving/pinning the sole box. Direct tests cover all 24
compiler branches, reservation-before-getter/raw-key ordering, stack restore,
retry, thrown getters, and pin cleanup. Local gates pass: all-feature debug
library **275/275**, release library **273/273**, complete integration suite,
tooling **135/135**, all-target, wasm32, Clippy `-D warnings`, fmt, docs, and
doctest **1/1**; rustdoc retains 13 existing warnings. GPT-5.6 reviewers Rawls
and Gauss are CLEAN after finding and verifying the raw-name peak reservation,
exhaustive compiler fixture, precise pin wording, and artifact cleanup. Both
agents are closed. No matching RuJa/Test262 `/tmp` artifacts remain.

CI `30218857008` and all 33 jobs in full run `30218856969` pass. Current full
artifacts aggregate to **31894/5115/11455/3/0** over **48467** total and
**37009** pass-or-fail run. Twenty-nine files match preceding run
`30217016070`; only Annex B differs because its one contention timeout returns
to the clean **201/811/74/0/0** result. Both artifact sets were deleted after
comparison. Commit this final evidence documentation, push it without waiting
for docs-only Actions, then run `cargo clean` and remove any remaining generated
targets/logs before starting the next unit.

## Current unit - embedded RegExp empty classes

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4` confirms five
existing failures for embedded `[]`/`[^]`. The normalizer now replaces only an
exact empty outer-class slice with the established never-match or mode-specific
universal backend atom. Direct tests cover literal/constructor, composition,
flags, quantifiers, UTF-16/Unicode/v behavior, lone surrogates, nested-v
subtraction, source preservation, and escaped brackets. All five exact files
pass; full RegExp changes **1036/7/836** to **1041/2/836**. Remaining failures
are `quantifier-integer-limit.js` and `nullable-quantifier.js`.

The separate exact admission of two variable-length TypedArray staging files
is audited and ready after this semantic RegExp unit.

Local gates now pass: all-feature debug library **275/275**, release library
**273/273**, builtins **542/542**, complete integration suite, tooling
**135/135**, all-target, wasm32, Clippy `-D warnings`, fmt, docs, and doctest
**1/1**; rustdoc retains 13 existing warnings. GPT-5.6 reviewers Planck and
Kepler are CLEAN after Fancy lookahead/backreference/capture-corrected fixtures
and historical ADR wording fixes; both agents are closed. Commit/push and CI
evidence are complete below.

Implementation commit `35d7ccc` passes CI `30221601414` and all 33 jobs in
full run `30221601417`. Built-ins is exactly **+5 pass/-5 fail**; 28 other
original files match baseline `30219956582`. Raw Annex B has two contention
timeouts; downloaded-binary rerun restores clean **201/811/74/0/0** and is
byte-identical to baseline. Corrected aggregate is
**31899/5110/11455/3/0** over **48467** total and **37009** pass-or-fail run.
All artifacts, rerun logs/binary, and the sparse worktree were deleted. Commit
and push this final evidence documentation without waiting for docs-only
Actions, then run `cargo clean` and remove generated vendor targets/caches.

## Current unit - variable-length TypedArray staging admission

The shared extensibility manifest admits exactly the Object and Reflect
`preventExtensions-variable-length-typed-arrays.js` staging files. Tooling
requires exact feature/include metadata, recursive singleton directory equality,
runner/analyzer agreement, future-sibling rejection, and continued exclusion of
the pinned `Object.seal` staging test. The full workflow preflight runs this
contract and appends the two singleton parent directories to its matrix.

Pinned focused execution is **2/0/0/0/0** over **2** tests. This is a policy-only
unit; runtime semantics and direct RAB/GSAB regressions already shipped in the
Object integrity unit. Supported-subset counts remain unchanged.

Feature commit `6f6f8db` passes CI `30223746065` and all **35/35** jobs in full
run `30223746062`. Both new shards are **1/1**. Twenty-nine preceding shards are
byte-identical to run `30221601417`; only Annex B contention changes. The
downloaded current binary reruns Annex B at clean **201/811/74/0/0**,
byte-identical to run `30219956582`. Corrected full results are
**31901/5110/11455/3/0** over **48469** total and **37011** pass-or-fail run,
exactly **+2 pass / +2 total / +2 run** from the corrected RegExp baseline.
All downloaded artifacts, binaries, logs, the temporary pinned worktree,
Python caches, and Cargo targets were deleted before ending this turn;
`cargo clean` removed 3.7 GiB and restored about 13 GiB free space.
Evidence documentation is committed and pushed as `47fe986`; its docs-only
Actions do not need observation.

## Current unit - native indexed PropertyKey pipelines

Ninety native numeric property-name formatting sites across Array, TypedArray,
Array iterators, call-argument materialization, JSON, RegExp, Proxy own-key
validation, and adjacent array-like constructors no longer create temporary
Rust Strings. Structured operations use `PropertyKey::from_integer_index`;
strict Set, Array iterator Get, and Array search preserve established string
dispatch through an owned stack decimal view. Larger u64 names and all 32-bit
numeric keys build their required Arc directly from that view.

Direct tests cover integer names **4294967294**, **4294967295**, **4294967296**,
and **9007199254740990**, primitive String search, Proxy get/set/delete keys,
and values/entries iterators at the named-integer boundary. Local gates pass:
all-target/features debug library **276/276**, release library **274/274**,
builtins **543/543**, array-index **52/52**, tooling **135/135**, supported
subset **12761/0/7678**, wasm32, Clippy `-D warnings`, fmt, doctest **1/1**, and
rustdoc with 13 existing warnings. Pinned current/preceding output for eight
affected built-in families is byte-identical at **6712/6/1082/0/0** over
**7800** tests. Five-run wall-time medians found no regression, but are only
shared-host smoke evidence. GPT-5.6 semantic reviewer Anscombe is CLEAN.
Performance/documentation reviewer Plato found and prompted the u64 stack
formatter, count correction, dispatch-exception wording, and benchmark-claim
downgrade; both reviewers are CLEAN after re-review and are closed. A post-fix
combined rerun had one transient TypedArray timeout; the same binary then passes
TypedArray **1446/1446**, restoring corrected combined **6712/6/1082/0/0**.
Implementation commit `9554cc3` passes ordinary CI `30228832557` and every job
in full run `30228832584`. All 32 result files are byte-identical to preceding
run `30225015795`; aggregate remains **31901/5110/11455/3/0** over **48469**
total and **37011** run. Evidence documentation is committed and pushed as
`19b4336`; its docs-only Actions do not need observation. Both downloaded
artifact sets, local Test262/RuJa worktrees, logs, benchmark scripts, Python
caches, and Cargo targets were deleted. No matching `/tmp` output or related
process remains; the worktree is clean and `HEAD == origin/main`.

## Current unit - stack-backed non-index Number property keys

Runtime Number ToString now writes the existing fixed/exponential algorithm
into 32-byte stack storage. `Vm::to_string(Number)` copies that view directly
into the required final Arc, while public `num_to_string` retains its owned
String contract. Dynamic non-index Number property keys therefore remove one
temporary String allocation, or two temporary Strings for exponential
normalization, without changing Value or PropertyKey layout.

The stack output is byte-identical to the preceding algorithm over semantic
edges and 20,000 deterministic random f64 bit patterns. Proxy tests require
exact negative, fractional, boundary, exponential, NaN, and infinite String
keys. Pinned current/preceding output over assignment, delete, relational,
property-accessor, Object, Proxy, and Reflect paths is byte-identical at
**4789/0/194/0/0** over **4983** tests. Forced-rebuild `in` Criterion A/B is
numeric **65.796 ms vs 70.611 ms** and string control **65.932 ms vs 66.754
ms**; no significant change.

GPT-5.6 audits rejected blanket Reference inner-field unboxing because common
identifier records would grow from 64 to about 120 bytes on x86-64 and 40 to
about 96 bytes on wasm32. A one-entry outer Reference-box cache or a
property-specialized record remains a later independent design. The runtime
review was clean. The test/documentation review found and prompted an `in`
benchmark that exercises the intended conversion path and validates its
30,000 result, corrected static parser/compiler allocation scope, and the
25-byte maximum fixed-form comment plus regression edge. Both reviewers are
clean after re-review and closed.

Local gates pass: all-target/features library **277/277**, release library
**275/275**, every integration test and benchmark smoke case, supported subset
**12761/0/7678**, focused current/preceding **4789/0/194/0/0**, tooling
**135/135**, wasm32, Clippy `-D warnings`, fmt, doctest **1/1**, and rustdoc
with 13 existing warnings. Implementation commit `05c80a3` passes ordinary CI
`30253320816` and all 34 jobs in full run `30253318891`. All 32 result
artifacts are byte-identical to preceding run `30248159616`; aggregate remains
**31901/5110/11455/3/0** over **48469** total and **37011** run. Evidence docs
commit `207555f` is pushed; its docs-only Actions need no observation. Both
artifact sets, benchmark target/worktree, pinned Test262 worktree, logs,
binaries, Python caches, and Cargo targets were deleted. No matching `/tmp`
output or related process remains; the worktree is clean and
`HEAD == origin/main`.

## Current unit - direct object Reference bases

Object-backed resolved, raw, super, and private References now use
`ReferenceBase::Object(GcIdx)` instead of allocating an inner `Box<Value>`.
Primitive bases and ObjectEnvironment remain boxed for receiver/Realm and
`with` semantics. Shared get/put helpers preserve behavior; delete, call-this,
root visitation, and retained raw-root reservation handle the direct variant.

Representation tests require direct object storage, primitive boxing, and one
exact GC root; target-specific compile-time assertions preserve x86_64
**32/16/64-byte** plus wasm32 **24/8/40-byte** Value/base/record layouts.
Existing operators **131/131**,
classes **105/105**, with **58/58**, root visitor, and retained-reservation
tests pass. Pinned current/preceding expressions/class/with output is
byte-identical at **10290/0/5360/0/0** over **15650** files. Forced-rebuild
Criterion A/B found no significant change: numeric **90.825 ms vs 89.879 ms**,
string **92.025 ms vs 93.394 ms**.

GPT-5.6 audits selected this direct variant over a specialized Value handle and
confirmed a safe one-entry outer Reference-box cache remains a later option.
Final review found that the retained raw-root failpoint still constructed the
legacy boxed base; it now uses `Object(GcIdx)` and requires that variant after
failure restoration and successful retry. The other review narrowed ABI
assertions from every 32-bit target to wasm32 and made x86_64 assertions
compile-time too. Both reviewers are clean after re-review and closed.

Local gates pass: all-target/features library **278/278**, release library
**276/276**, every integration test and benchmark smoke case, supported subset
**12761/0/7678**, focused current/preceding **10290/0/5360/0/0**, tooling
**135/135**, wasm32 with compile-time 32-bit ABI assertions, Clippy `-D
warnings`, fmt, doctest **1/1**, and rustdoc with 13 existing warnings.
Implementation commit `df80aa9` passes ordinary CI `30259064592` and all 34
jobs in full run `30259064604`. All 32 result artifacts are byte-identical to
preceding run `30255588286`; aggregate remains **31901/5110/11455/3/0** over
**48469** total and **37011** run. Evidence docs commit `a9cd46b` is pushed;
its docs-only Actions need no observation. Both artifact sets, benchmark
target/worktree, pinned Test262 worktrees, binaries, logs, Python caches, and
Cargo targets were deleted. No matching `/tmp` output or related process
remains; the worktree is clean and `HEAD == origin/main`.

## Current unit - VM-local outer Reference box reuse

Each VM now retains one rootless vacant `Box<ReferenceRecord>` for sequential
identifier, property, super, and private Reference reuse. Checkout precedes
observable re-entry. Get/put/delete/call/eval/type-of, explicit pop, catch,
frame, top-level, async, and generator terminal paths return eligible boxes.
Nested execution allocates independently; a full one-entry cache drops the
overflow record. Discarded stack tails are scanned and truncated in place with
no new Vec allocation.

The complete sentinel replacement removes every base, raw-name, receiver, and
nested Reference root and is intentionally absent from root enumeration.
Top-level `execute_chunk` now restores its incoming frame/stack depth instead
of accumulating one Halt frame per run. Async rejection materializes native
errors in the function Realm before settlement. Generator saved stacks move
instead of cloning References and are recycled on completion/error.

Six deterministic cache tests cover pointer reuse, all root-bearing fields,
sequential and re-entrant allocation, terminal/catch/uncaught errors,
cross-Realm async rejection, retained References across await/resumption, and
generator completion plus uncaught coercion errors. All-target/features passes
with library **284/284**, release library **284/284**, every integration and
benchmark smoke case, Clippy `-D warnings`, fmt, wasm32, doctest **1/1**,
rustdoc with 13 existing warnings, and tooling **135/135** on a complete pinned
worktree. Two GPT-5.6 reviewers are CLEAN and closed.

Pinned current/preceding expressions/class/with output is byte-identical at
**10290/0/5360/0/0** over **15650** files with SHA-256 `9334343e...`. The
supported subset remains **12761/0/7678/0/0** over **20439**, SHA-256
`0552a3a7...`. Final Criterion smoke samples show no significant change.
Implementation commit `6caf259` passes ordinary CI `30266752935` and all 34
jobs in full run `30266753068`. All 32 result artifacts are byte-identical to
preceding run `30261135576`; aggregate remains **31901/5110/11455/3/0** over
**48469** total and **37011** run. Evidence docs commit `2b344c6` is pushed;
its docs-only Actions need no observation. Both artifact sets, benchmark and
Test262 logs, the complete pinned worktree, Python caches, and Cargo targets
were deleted. No matching `/tmp` output or related process remains; the
worktree is clean and `HEAD == origin/main`.

## Current unit - direct deferred object Reference names

Deferred computed object and Proxy names now use
`UncoercedPropertyName::Object(GcIdx)` instead of an inner `Box<Value>`.
Primitive and internal recursive names retain the boxed payload. The only two
production constructors, `MakeRawPropertyRef` and `MakeSuperPropertyRef`, use
the compact form; GetValue, PutValue, delete, and `ResolvePropertyRef` share
one coercion helper. Existing null checks, pin scopes, and retained raw-name
two-copy root reservation remain unchanged, preserving RHS/coercion order and
one-time super resolution.

Direct tests cover both opcodes, direct/boxed representation, exact root order,
reservation failure/retry, Proxy key mutation after RHS evaluation, forced GC,
Symbol conversion, and super receiver identity. Audited sizes remain
16/32/64 bytes for nested name/name/record on x86_64 and 8/24/40 on wasm32.
The public Rust payload shape changes from `Box<Value>` to
`UncoercedPropertyName`; no stable enum ABI is claimed.

Local gates pass: non-benchmark all-feature tests with library **287/287**,
builtins **543/543**, operators **132/132**, ES2015 **134/134**, and every
integration suite; release library **287/287**; tooling **135** tests with four
expected checkout-unavailable skips; Clippy `-D warnings`, fmt, wasm32,
doctest **1/1**, and rustdoc with 13 existing warnings. The initial
`cargo test --all-targets` passed the library and integration work reached
before its Criterion-wide smoke exceeded the host command lifetime; both new
release benchmarks were then run explicitly and passed.

Pinned current/preceding direct-cohort output is byte-identical at
**1343/0/209/0/0** over **1552** files, SHA-256
`3e4263f9e2b4ce015ce68dcb2ef988a467a9cede897c61c624ae31ac356032ab`.
The complete supported statements/expressions output is byte-identical at
**12761/0/7678/0/0** over **20439**, SHA-256
`c59b10015e636a164867edd718fc2b0f018e5bcf2d0ed969fa0df136ade46dfc`.
Final forced-rebuild smoke samples measured object-key assignment at
413.89-423.33 ms current versus 436.25-462.47 ms preceding; String control was
138.27-143.33 ms versus 142.13-147.63 ms. Treat these only as no-regression
shared-host evidence.

GPT-5.6 runtime reviewer Nietzsche returned CLEAN. Documentation/performance
reviewer Bacon found independent benchmark assertions, reproduction docs,
stale super wording, and size-vs-ABI wording; all were fixed and re-review is
CLEAN. Both agents are closed.

Implementation commit `942be9bddc6e14a17ed78be343f4638383ce1962` is pushed.
Ordinary CI `30285088249` passes both jobs and all **35/35** jobs in full run
`30285089340` pass. Thirty-one current result artifacts match preceding run
`30276195375` immediately. Its sole Annex B contention timeout returns to
clean **201/811/74/0/0**, SHA-256
`410da6b0d17c7cdd50df356717c298529ca9bcb63be12c77143ec2a1b966153a`,
byte-identical to clean run `30269385090`. Corrected all 32 artifacts match;
aggregate remains **31901/5110/11455/3/0** over **48469** total and **37011**
run. Evidence docs commit `b0e386a` is pushed; its docs-only Actions need no
observation. Both downloaded artifact sets, the clean Annex comparison,
benchmark/Test262 output, temporary worktrees, Python caches, and Cargo targets
were deleted. `cargo clean` removed 3.3 GiB. No related process remains; the
worktree is clean and `HEAD == origin/main`.

## Current unit - Temporal.ZonedDateTime.from ISO property bags

Ordinary objects now enter an ISO property-bag path instead of the branded
ZonedDateTime check. Calendar and fields are read/coerced in exact observable
order, getter-produced objects stay pinned across JavaScript re-entry, and
`disambiguation`, `offset`, then `overflow` are read before algorithmic
validation. Arbitrary finite numeric fields remain mathematical integers in
`BigInt` until overflow regulation. UTC and minute-precision fixed zones use
exact local-minus-offset nanosecond arithmetic; named IANA/DST remains closed.

Focused Rust property-bag/parser regressions pass. Pinned Test262
`9e61c12835c5e4a3bdba93850427e6742c4f64c4` forced diagnostics are **243 pass /
23 fail / 0 skip** over the exact 266-file surface, up from **208/58/0**.
Admission/live metadata tooling passes with **243** admission and **23** exact
blockers. Two GPT-5.6 read-only reviewers classified the corpus and audited
observable order, roots, fuel, mathematical integers, and fixed-offset rules;
both made no edits and are ready to close.

Local gates pass: Rust all-target/features library **425/425**, builtins
**597/597**, every integration/benchmark smoke, Clippy with denied warnings,
fmt/diff, complete tooling **187/187**, exact admission **243/0/0**, and forced
surface **243/23/0**. One local review moved arbitrary-finite year narrowing
after required-field TypeErrors and added a regression. A fresh final GPT-5.6
reviewer returned no result and was closed; it made no edits or spawned
processes. Commit/push and CI observation remain. Mandatory every turn: close
agents, delete `/root/test262`, Cargo targets, Python caches, logs/temp
artifacts, prune worktrees, and verify no cargo/Test262/Codex subagent process
remains. Never delete or alter the active goal.

Implementation commit `02d019f` is pushed. Initial full CI run `30872169153`
proved the engine's exact job at **243/0/0**, but failed because the workflow
still grepped the preceding **208/0/0** and **208/58/0** literals. The workflow
expectations are being updated to **243/0/0** and **243/23/0** in a follow-up
commit. Ordinary CI `30872169144` and the rest of the first full matrix were
still running when this was recorded.

CI-count correction `f9adc40` is pushed. Superseded runs `30872169144` and
`30872169153` were cancelled to remove duplicate work. Final ordinary CI
`30872435648` passes **3/3** and final full run `30872435664` passes **57/57**,
including exact **243/0/0** and forced **243/23/0** fixed-offset jobs. Cargo
clean removed 9.4 GiB; `/root/test262`, Python caches, temporary worktrees, and
all local build/test artifacts were deleted. Worktree must be clean after the
final evidence-doc commit.

## Current unit - Temporal.ZonedDateTime.prototype.equals

Added Realm-local nonconstructable `equals`, sharing slot-producing
ToTemporalZonedDateTime conversion with static `from`. Receiver branding comes
first; the argument is fully converted before epoch, canonical time-zone, and
calendar identity comparison. Public properties are not observed. Dedicated
time-zone/calendar helpers preserve the future IANA/calendar canonicalization
extension point.

The shared parser now accepts date-only ZonedDateTime strings only when a
time-zone annotation immediately follows the date. Instant parsing and
date-only `Z`/numeric-offset forms remain rejected. Focused parser, hidden-slot,
cross-Realm, and allocation-boundary regressions pass. Two GPT-5.6 read-only
reviews completed and are closed; one returned CLEAN, while the corpus review
predated the date-only fix and its 49/6 estimate was superseded by local exact
measurement.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4` is frozen at exact
**50/0/0** and forced **50/5/0** for the 55-file equals directory. Ten former
fixed-offset blockers now pass, making that unchanged 266-file surface exact
**253/0/0** with forced **253/13/0**. Tooling, workflow, CHANGELOG, architecture,
features, limitations, and Test262 docs are updated. Local gates pass:
all-target/features library **425/425**, builtins **598/598**, every integration
and benchmark smoke case, tooling **188/188**, exact/forced Test262 gates,
Clippy with denied warnings, fmt/diff, workflow YAML, and Python compilation.
A final GPT-5.6 read-only review is CLEAN. Implementation commit `1490baa` is
pushed. Ordinary CI `30875867855` passes **3/3** and full Test262 CI
`30875867994` passes **58/58**, including the new equals and updated
fixed-offset jobs. Evidence-doc commit `21b0ae6` is pushed with `[skip ci]`.
Root/vendor Cargo targets, `/root/test262`, Python caches, temporary CI files,
and worktree metadata were cleaned; final verification found no related
process and `HEAD == origin/main`.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainDateTime with

Added Realm-local, length-1, non-constructable
`Temporal.PlainDateTime.prototype.with`. It brands before input observation,
roots input/options before getters, rejects six branded Temporal date/time
kinds through hidden slots, then observes `calendar`, `timeZone`, and ten
partial fields in exact specification order. A dedicated optional BigInt
collector preserves every missing receiver slot instead of applying the full
bag collector's zero time defaults. Overflow is observed only after field
conversion; ISO month/monthCode agreement, date/time constrain/reject, and the
exclusive PlainDateTime range are validated before a fresh method-Realm
intrinsic is allocated. Existing ordinals 1..193 stay stable; `with` is
allocation 194, complete installation is exact **194 allocations / 186 maximum
pins**.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
has direct **29/29**. One built-ins file remains an exact earlier missing
`Temporal.Now` dependency and 70 Intl402 files remain exact non-ISO calendar
blockers, giving complete forced **29 pass / 71 fail / 100**. Full pinned
`test+harness` ownership freezes **176 candidates / 176 rows**: 29 direct, one
direct dependency, 70 Intl blockers, 75 PlainDate/PlainMonthDay/PlainTime/
PlainYearMonth/ZonedDateTime homonyms, and one generic harness owner. Exact
metadata, candidate/ownership digests, direct/computed references and calls,
five absent-method false positives, runner/analyzer parity, future/outside
paths, arguments, results, normalized errors, and variant-specific failure
locations fail closed.

Runtime/resource coverage includes installer rollback at allocation 194,
every boundary and exact 194 capacity, brand-first behavior, all six Temporal
input fast rejections without public getter observation, method-Realm errors
and results, field preservation, month ambiguity, constrain/reject and range,
observable input/options/coercion GC, root preflight and coercion-root failure,
exact heap-cap failure, and GC retry. Local gates pass all-target/all-feature
release Rust including Criterion, warnings-denied release Clippy, rustfmt/diff,
Python/YAML, direct **29/29**, complete **29/71/100**, live tooling **252/252**,
and corpus-unavailable tooling **252 tests / 5 skips**. Two GPT-5.6 reviews
found and closed the six-kind fast-rejection coverage gap and same-error/
wrong-location diagnostic gap.

Feature commit `af5688c` (`feat(temporal): merge plain date-time fields`) is
pushed to `main`. Ordinary CI `31546018478` passes **3/3**, including the exact
direct and complete steps. Full Test262 CI `31546018472` passes **88/88** after
rerunning one unrelated ZonedDateTime.startOfDay sparse checkout whose first
attempt failed on runner CA certificate verification. The dedicated
PlainDateTime with job and both exact diagnostics pass.

Next narrow unit: audit and implement complete
`Temporal.PlainDateTime.prototype.until` and `since` as one shared hidden-record
difference cluster if their pinned ownership and prerequisite boundary remain
tractable. Keep `Temporal.Now`, named-IANA transition data, locale formatting,
and non-ISO calendar backends separate unless the live ownership audit proves
one is an unavoidable direct prerequisite.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainDate/PlainDateTime withCalendar

Added Realm-local, non-constructable, length-1
`Temporal.PlainDate.prototype.withCalendar` and
`Temporal.PlainDateTime.prototype.withCalendar`. Receiver branding precedes
calendar conversion; all hidden ISO fields are copied without public getters.
Five calendar-bearing Temporal kinds use hidden slots, while every other input
must already be a String and reuses the exact parser/fuel path. Results are
fresh method-Realm intrinsics and ignore mutable globals, constructors,
species, and subclasses. Non-ISO identifiers remain fail-closed until a real
calendar field/arithmetic backend exists. Existing ordinals 1..183 remain
stable; the methods are allocations 184 and 185, complete installation is
exact **185 allocations / 183 maximum pins**, and preflight reserves all 183
pins.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
has paired direct admission **36/36** and complete direct accounting **36 pass
/ 9 exact non-ISO failures / 45**. Explicit downstream has 50 files and reuses
the existing 87-file PlainYearMonth helper surface without duplication;
combined downstream remains **0 pass / 137 fail / 137** with 133 exact calendar
RangeErrors and four absent DateTimeFormat constructor TypeErrors. The earlier
PlainDate sibling-conversion epoch-year blocker now reaches `withCalendar` and
fails with the intended calendar RangeError instead of absent-method TypeError.
Complete executable candidate and call audits freeze **141 files / 300 lexical
mentions**, **122 files / 228 calls**, exact four direct directories, and two
harness calls with receiver ownership.

Runtime tests cover hidden-record preservation, fresh identity, mutable-global
replacement, cross-Realm receiver/result/error behavior, nonconstruction,
result root reservation failure, exact heap caps and GC retry, installer
rollback at allocations 184/185, zero-fuel hidden arguments, exact seven-byte
String fuel, and brand-before-fuel ordering. Two GPT-5.6 final audits found no
runtime defect; the tooling audit's indirect-reference gap was closed with a
candidate superset digest and exact live directory identity.

Local gates pass: all-target/all-feature release Rust including Criterion;
warnings-denied all-target/all-feature release Clippy; rustfmt, diff, workflow
YAML and Python compile; full live Python tooling **236/236**; exact direct
**36/36**, complete direct **36/9/45**, downstream **0/137**, and transitioned
sibling downstream **0/3** with exact normalized reasons.

Feature commit `3b0c73e` (`feat(temporal): add plain calendar replacement`)
and CI-boundary correction `bdebd20` (`fix(test262): refresh calendar blocker
gate`) are pushed to `main`. Final ordinary CI `31503894408` passes **3/3**.
Final full Test262 CI `31503894336` passes **84/84**.

Next narrow unit: inspect the final full-matrix artifact and choose the next
complete real Temporal/core-language boundary. Prefer a bounded ISO method or
shared prerequisite that advances non-ISO/Intl reachability without relabeling
ISO records or admitting incidental TypeErrors.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainDate serialization siblings

Added Realm-local, nonconstructable
`Temporal.PlainDate.prototype.toJSON` with length 0. It brands the receiver,
ignores arguments, and formats copied hidden ISO date/calendar slots through
the shared PlainDate formatter with `calendarName: auto`; overridden public
getters and `toString` are not observed. Tests cover extended years,
`JSON.stringify`, cross-Realm receivers and method-Realm TypeErrors,
nonconstruction, zero-extra-object execution, and installer OOM rollback.
Installer accounting is now 119 maximum live pins and 120 allocations;
toJSON is allocation 118.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
is direct toJSON exact/forced **8/0/0**. Direct toLocaleString remains exact
**0/0/7** and forced **5/2/0**; Intl402 toLocaleString remains exact
**0/0/14** and forced **2/12/0**. No true downstream caller exists outside
these 29 files. The five direct and two Intl forced-pass identities are frozen
in separate manifests, not only aggregate counts. Do not install an ISO-only
own toLocaleString: RuJa exposes partial `%Intl%`, so ECMA-402 requires a real
Realm-local `Intl.DateTimeFormat` with Temporal calendar/options/conflict/
defaults/time-zone behavior. All 21 locale files stay blocked until then.

Local gates pass: all-target/all-feature Rust library **453/453**, builtins
**633/633**, all integration targets and benchmark smoke; pinned and
corpus-unavailable Python tooling **206/206**; six exact/forced Test262 gates;
warnings-denied root Clippy; Rust 1.88; fmt/diff/YAML/Python; generated Intl
data; vendored RegExp **38/38** plus Clippy/no_std; wasm32; release build and
exact Realm rollback. GPT-5.6 runtime review was CLEAN. Tooling review found
one P2 aggregate-only false-positive identity gap; exact forced-pass manifests
fixed it and the same reviewer returned CLEAN.

Implementation commit `ba5ad57` is pushed. Ordinary CI `31234854582` passes
**3/3**. Full Test262 CI `31234854584` passes **74/74**. Dedicated serialization
job `93045806985` logs toJSON exact/forced **8/0/0**, direct locale exact
**0/0/7** and forced **5/2/0**, and Intl locale exact **0/0/14** and forced
**2/12/0**. All review agents are closed.

Next recommended narrow unit: audit
`Temporal.PlainDate.prototype.toPlainDateTime` across its complete pinned
direct/Intl/downstream surface before implementation. Reuse hidden PlainDate
slots and the existing PlainDateTime constructor boundary, while measuring
argument conversion order, Realm errors, GC/OOM/fuel, calendar identity, and
non-ISO blockers first. Keep DateTimeFormat/toLocaleString as a separate
architecture unit.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainDateTime.compare

Added Realm-local, nonconstructable static `compare`. It converts the first
operand completely before touching the second through the shared
`to_temporal_plain_date_time` compact-record boundary, discards calendar
identity, compares year through nanosecond lexicographically, ignores the call
receiver, and returns primitive Number -1, +0, or 1 without a result object
allocation. Branded PlainDateTime/ZonedDateTime hidden slots, ISO property
bags, strings, nine-field priority, first-abrupt short circuit, method-Realm
errors, fuel, GC/OOM rollback, and exact heap caps are covered. Installer
accounting is 95 maximum pins and 96 allocations. PlainDateTime.compare is
allocation 70 (baseline+69 failure); existing PlainDateTime.equals moved to
allocation 94 (baseline+93); the complete boundary loop is `17..96` with
baseline+96 success.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
is exact **40/0/0** for the independently reachable 42-file direct directory
and forced **40/2/0**. The blockers are `argument-plaindate.js` and
`calendar-temporal-object.js`, both requiring absent PlainDate-family hidden
kinds. `ZonedDateTime/compare/compares-exact-time-not-clock-time.js` moved into
its owner manifest, making that boundary exact **49/0/0** and forced
**49/1/0**; only `calendar-temporal-object.js` remains there.

Local gates pass: workspace/all-target/all-feature tests including library
**438/438**, builtins **617/617**, fuel **47/47**, every integration and
benchmark target; tooling **199/199** with five expected unavailable-checkout
skips plus live direct/downstream metadata tests; direct exact/forced Test262
**40/0** and **40/2**, downstream **49/0** and **49/1**; root Clippy denied
warnings; fmt/diff; Python/YAML; generated Intl checks; vendored RegExp
**38/38**, Clippy/no_std; wasm32; release Realm rollback. The first broad run
hit host `ENOSPC` during linking, not a code failure; release/incremental and
regenerable host caches were removed, 3.2 GiB restored, and the complete broad
gate then passed. One reviewer unexpectedly started duplicate Cargo commands;
it was stopped and closed, leaving one authoritative main validation session.
Final runtime GPT-5.6 review is CLEAN. Tooling/docs review found four stale
current-vs-historical ZonedDateTime count claims; all were corrected and its
live gates were clean. All review agents are closed.

Implementation commit `bf45482` is pushed. Ordinary CI `31200775282` passes
**3/3** and full Test262 CI `31200776243` passes **68/68**. Dedicated logs
confirm direct exact **40/0/0**, forced **40/2/0**, downstream exact
**49/0/0**, and forced **49/1/0**. Evidence-doc commit `46895ce` is pushed with
`[skip ci]`.

Next recommended narrow unit: `Temporal.PlainDate` hidden-slot core plus the
PlainDate-to-PlainDateTime midnight fast path. Read-only GPT-5.6 prioritization
estimates a 78-file core candidate with 67 independently reachable files and
11 `PlainDate.from` dependencies, plus six existing blocker releases:
PlainDateTime.from four paths (expected 65/5 to 69/1), equals one (39/2 to
40/1), and compare one (40/2 to 41/1), for an expected **73 exact-path** gain.
Treat these as audit targets, not admitted facts, until pinned execution.
Create a distinct PlainDate hidden kind/three-field record, Realm-local
constructor/prototype registry, constructor plus 16 ISO/calendar accessors,
valueOf and toStringTag, GC/Realm rollback coverage, a getter-free hidden-slot
midnight branch before the generic PlainDateTime property-bag path, and
calendar-identifier fast-path support. Do not reuse PlainDateTime branding.
Keep PlainDate.from, formatting, arithmetic, PlainMonthDay, and PlainYearMonth
out of that unit. `calendar-temporal-object.js` remains blocked until the two
sibling types exist. Re-evaluate the current `options-wrong-type.js` false
positive after PlainDate exists because it will finally reach its intended
assertion.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.ZonedDateTime.prototype.withTimeZone

Added Realm-local nonconstructable `withTimeZone`. Receiver branding precedes
argument conversion; String or branded ZonedDateTime inputs use the shared
canonical UTC/fixed-offset converter without observing public properties. The
result preserves hidden epoch/calendar identity, ignores subclasses, and uses
the method Realm intrinsic prototype. Namespace allocation boundaries move
from 49 to 50 objects. GPT review found and fixed an under-reserved installer
pin vector, inconsistent `UTC` versus `+00:00` hidden kinds, a false
cross-Realm fixture, and missing exact fuel/heap-cap GC retry evidence.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4` is frozen at exact
**14/0/0** and forced **14/2/0** for the 16-file method directory. Equals grows
to exact **52/0/0** with forced **52/3/0**. Local all-target/features tests pass
with library **426/426**, builtins **599/599**, every integration and benchmark
smoke case, tooling **189/189**, Clippy with denied warnings, fmt/diff, YAML,
Python compilation, release exact/forced gates, and focused heap/fuel tests.
A final GPT-5.6 review found one stale evidence attribution; docs now identify
the preceding 50/5 checkpoint correctly, and the reviewer is closed.

Implementation commit `593d047` is pushed. Ordinary CI `30879142138` passes
**3/3** and full Test262 CI `30879142121` passes **59/59**, including dedicated
`withTimeZone` **14/0/0**, forced **14/2/0**, equals **52/0/0**, and forced
**52/3/0** jobs. Evidence docs commit `fec8737` is pushed with `[skip ci]`.
Root/vendor Cargo targets, `/root/test262`, Python caches, logs/temp artifacts,
and stale worktree metadata were deleted. All subagents are closed. Final
verification found no related process or artifact directory, one worktree,
clean git, `HEAD == origin/main` at `fec8737`, and 12 GiB free disk.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.ZonedDateTime.prototype.withCalendar

Added Realm-local nonconstructable `withCalendar`. Receiver branding precedes
strict calendar conversion; branded ZonedDateTime input uses hidden slots
without observing public properties or coercion hooks. String input accepts
the ISO calendar plus valid date/time, instant, month-day, and year-month
syntax. A parsed time syntax record is shared with time-zone parsing while
consumer semantics remain separate. Results preserve exact epoch and complete
UTC/fixed-offset identity in the method Realm.

Property-bag `undefined => iso8601` defaulting is separated from the strict
method helper, where missing/undefined throws TypeError. Namespace boundaries
move to 50 maximum provisional pins and 51 allocations. Focused parser,
hidden-slot, cross-Realm TypeError/RangeError/result, UTC versus `+00:00`, exact
fuel/input-length charging, heap-cap GC retry, and installer rollback tests
pass.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4` is exact **14/0/0**
with forced **14/2/0** over the complete 16-file directory. The two blockers
construct missing PlainDate-family types or Temporal.Duration. Local gates
pass: all-target/features library **427/427**, builtins **600/600**, every
integration/benchmark smoke, tooling **190/190**, Clippy denied warnings,
fmt/diff, YAML, Python compilation, and release exact/forced gates. Three
GPT-5.6 read-only audits agree on 14/2; final diff review is CLEAN and all
agents are closed.

Implementation commit `86124ef` is pushed. Ordinary CI `30906827851` passes
**3/3** and full Test262 CI `30906827947` passes **60/60**, including dedicated
`withCalendar` exact **14/0/0** and forced **14/2/0** jobs. Evidence docs commit
`ef47c51` is pushed with `[skip ci]`. `cargo clean` removed 8.4 GiB; root/vendor
Cargo targets, `/root/test262`, Python caches, logs/temp artifacts, and stale
worktree metadata were deleted. Final verification found no related process or
artifact directory, one worktree, clean git, `HEAD == origin/main` at
`ef47c51`, and 8.8 GiB free disk.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.ZonedDateTime.compare

Added Realm-local static nonconstructable `compare`. It fully converts the
first branded ZonedDateTime, ISO property bag, or ZonedDateTime String before
touching the second, then directly orders hidden `BigInt` epoch nanoseconds.
The call receiver and branded public properties are ignored; zone and calendar
identity do not affect the result. Namespace installation now reserves 51
provisional pins and covers all 52 allocation boundaries.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
is exact **46/0/0** with forced **46/4/0** over the complete 50-file compare
directory. Two blockers construct missing Duration, one constructs PlainDate
family objects, and one reaches missing `toPlainDateTime`/PlainDateTime.compare
after the new compare assertion succeeds. Local gates pass: all-target/features
library **427/427**, builtins **601/601**, all integration/benchmark smoke,
Clippy denied warnings, fmt/diff, tooling **191/191** (5 live-only skips with
an unavailable full checkout), live compare metadata, workflow YAML, Python
compilation, exact/forced Test262, focused Realm/order/fuel/allocation tests.

Three GPT-5.6 read-only audits are closed. Final review found only three
documentation precision issues; all are fixed. Implementation commit `4b7cb2a`
is pushed. Ordinary CI `31085609728` passed engine tests, fmt, Clippy, wasm,
MSRV, and subset Test262 but failed only because the new tooling test did not
catch `PermissionError` from its default inaccessible `/root/test262` path.
The existing `except OSError => unavailable live checkout` pattern is now
applied and focused live/unavailable tests pass; follow-up commit/push and
replacement ordinary CI `31086742168` passes 3/3. Full run `31086744561`
passed 60 jobs but its Annex B shard timed out one of the two full-BMP RegExp
escape sweeps twice; both attempts were 61/0/1-timeout over the exact 62 files.
Local per-file timing identified leading/trailing BMP sweeps at about 12.5s
each versus the 8s per-variant default. An exact two-path 30s timeout set is
now shared by runner/analyzer; ordinary and outside files retain 8s. Local
Annex B is 62/0/0 and tooling 192/192. Timeout-policy commit `547df84` is
pushed. Final ordinary CI `31090247413` passes 3/3 and full Test262
`31090247258` passes 61/61, including compare exact/forced 46/0 and 46/4 plus
Annex B exact 62/0. Evidence-doc commit `6476570` is pushed with `[skip ci]`.
Root `cargo clean` removed 5.0GiB; vendor targets were empty. `/root/test262`,
Python caches, downloaded CI artifacts, and stale worktree metadata were
deleted. All agents are closed. Final verification must retain one worktree,
no Cargo/Test262/subagent process, clean git, `HEAD == origin/main`, and free
disk. Never delete or alter active goal.

## Current unit - Temporal.ZonedDateTime.prototype.startOfDay

Added Realm-local nonconstructable `startOfDay` for UTC and minute-precision
fixed offsets. It brands through hidden slots, derives the local ISO day with
floor semantics, converts local midnight back to an exact epoch, rejects
out-of-range results, preserves time-zone/calendar identity, and allocates in
the method Realm. Named IANA zones remain explicitly rejected until a
transition-aware `GetStartOfDay` backend exists. Namespace installation now
reserves 52 provisional pins and covers all 53 allocation boundaries.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4` is exact
**9/0/0** over the complete method directory with no blockers. Local gates
pass: all-target/features library **428/428**, builtins **602/602**, fuel
**42/42**, every integration/benchmark smoke, tooling **193/193** with five
live-only skips under an unavailable checkout, Clippy denied warnings,
fmt/diff, workflow YAML, Python compilation, release exact Test262, and
focused Realm/range/fuel/heap/allocation tests. Three GPT-5.6 audits are
closed; the final diff review is CLEAN.

Implementation commit `cd49a4b` is pushed. Ordinary CI `31094403387` passes
**3/3** and full Test262 `31094403310` passes **62/62**, including the new
dedicated exact **9/0/0** job. Evidence-doc commit `0f4b0ff` is pushed with
`[skip ci]`. Root `cargo clean` removed 6.3GiB; vendor targets were empty.
`/root/test262`, Python caches, temporary results, and stale worktree metadata
were deleted. Final verification found no related process or artifact
directory, one worktree, clean git, `HEAD == origin/main` at `0f4b0ff`, and
11GiB free disk.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.Duration hidden-slot core

Added Realm-local `%Temporal.Duration%` construction with ten immutable
integer-valued Number hidden slots. The constructor performs sequential
`ToIntegerIfIntegral`, defaults `undefined` to zero, canonicalizes negative
zero, rejects mixed signs, and validates date-unit and normalized day-time
limits with exact `BigInt` arithmetic before allocation. Realm-local
constructor/prototype registries, subclass/newTarget prototype selection,
method-Realm accessors/errors, GC roots, rollback, fuel, and heap-cap retry are
covered.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4` is exact
**76/0/0** and forced **76/2/0** over the frozen 78-file Duration core. The
two blockers require `Duration.from` and `Duration.prototype.total`. This unit
also unlocks nine ZonedDateTime wrong-type paths: fixed-offset is **255/11**
over 266, equals **54/1** over 55, compare **48/2** over 50,
`withTimeZone` **15/1** over 16, and `withCalendar` **15/1** over 16.

Local gates pass: all-target/features library **429/429**, builtins
**605/605**, fuel **43/43**, every integration and benchmark smoke test,
tooling **194/194**, exact/forced Test262 gates, Clippy denied warnings,
fmt/diff, workflow YAML, and Python compilation. Two final GPT-5.6 reviews
are closed; all validated findings were fixed. Implementation commit
`dc926dd` is pushed. Ordinary CI `31100834541` passes **3/3** and full
Test262 CI `31100834476` passes **63/63**.

Next recommended narrow unit: implement the PlainDateTime hidden-slot core,
then reassess ZonedDateTime `toPlainDateTime` and the compare/withTimeZone
blockers. Keep `Duration.from` and `Duration.prototype.total` as separately
measured units rather than widening the Duration admission prematurely.

Evidence-doc commit `303a802` is pushed with `[skip ci]`. Final cleanup
removed 246.2 MiB of Cargo output plus `/root/test262` and Python caches. No
related process or temporary artifact remains. Final verification found one
worktree, clean git, `HEAD == origin/main` at `303a802`, and 9.6 GiB free disk.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainDateTime hidden-slot core

Added Realm-local `%Temporal.PlainDateTime%` construction with compact
immutable ISO date-time and calendar hidden slots. The constructor performs
left-to-right BigInt-backed `ToIntegerWithTruncation`, defaults optional time
fields to zero, accepts only `undefined` or a bare ISO calendar identifier,
and validates the open PlainDateTime range one day beyond Instant limits.
Realm-local constructor/prototype registries, subclass/newTarget prototype
selection, 22 branded accessors, `@@toStringTag`, always-throwing `valueOf`,
method-Realm errors/results, GC roots, rollback, fuel, and heap-cap retry are
covered. Namespace installation uses 91 maximum live pins and 92 allocation
boundaries; Realm registries cover 55 boundaries.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4` is exact
**101/0/0** and forced **101/5/0** over the frozen 106-file core surface. The
five blockers require PlainDateTime arithmetic or `PlainDateTime.from`.
Existing ZonedDateTime gates remain fixed-offset **255/11**, equals **54/1**,
compare **48/2**, `withTimeZone` **15/1**, and `withCalendar` **15/1**.

Local gates pass: all-target/features library **430/430**, builtins
**608/608**, fuel **44/44**, every integration and benchmark smoke test,
tooling **195/195** with five expected unavailable-checkout skips, exact and
forced PlainDateTime gates, all affected ZonedDateTime gates, Clippy with
denied warnings, fmt/diff, workflow YAML, Python compilation, and release
build. Final GPT-5.6 runtime and tooling/documentation reviews are CLEAN and
all agents are closed. Implementation commit `71bcdf9` is pushed. Ordinary CI
`31107343779` passes **3/3** and full Test262 CI `31107343705` passes
**64/64**, including dedicated PlainDateTime exact/forced jobs. Evidence-doc
commit `eee23e9` is pushed with `[skip ci]`.

Next recommended narrow unit: implement
`Temporal.ZonedDateTime.prototype.toPlainDateTime`. Two read-only GPT-5.6
audits found an exact 10-file pinned directory with no external blocker:
four behavior files and six metadata/branding files should complete at
**10/0/0**. Reuse hidden ZonedDateTime slots,
`temporal_time_zone_offset_nanoseconds`, `temporal::iso_date_time`, and
`create_temporal_plain_date_time_in_realm`. Preserve brand-first ordering,
offset addition with Euclidean negative-time balancing, method-Realm result
prototype, subclass suppression, exact boundary handling, and OOM pin
balance. Expected installer counts move to 92 maximum live pins and 93
allocation boundaries. Do not approximate future named zones as fixed offsets.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.ZonedDateTime.prototype.toPlainDateTime

Added Realm-local nonconstructable `toPlainDateTime`. It brands through the
receiver's hidden ZonedDateTime slots, resolves the exact UTC/fixed offset,
uses checked `epoch + offset` arithmetic with Euclidean negative-time
balancing, copies the hidden ISO calendar, and creates a fresh PlainDateTime
with the method Realm intrinsic prototype. Public properties, mutable global
constructors, receiver subclasses, and arguments are not observed. Named IANA
zones remain rejected by the existing dispatcher until transition data exists.
Namespace installation now reserves 92 maximum live pins and covers all 93
allocation boundaries.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4` completes the
direct 10-file method directory at exact **10/0/0**. The conversion also moves
four `from` paths into the fixed-offset manifest, which is exact **259/0/0**
and forced **259/7/0** over 266, and admits the final `withTimeZone` blocker,
making that complete directory **16/0/0**. Compare remains **48/2**, equals
**54/1**, and withCalendar **15/1**.

Local gates pass: all-target/features library **431/431**, builtins
**610/610**, fuel **44/44**, every integration and benchmark smoke test,
tooling **196/196** with five expected unavailable-checkout skips, live exact
metadata tests, all direct/affected exact and forced Test262 gates, Clippy with
denied warnings, fmt/diff, workflow YAML, Python compilation, generated Intl
checks, vendored RegExp **38/38** plus Clippy/no_std, wasm32, release build,
and release Realm rollback. Runtime GPT-5.6 review is CLEAN. Tooling/docs
review found one stale `from` count; it was corrected from 88 to the verified
90, with all other paths/metadata/workflow claims clean. All agents are closed.

Implementation commit `044f765` is pushed. Ordinary CI `31113158807` passes
**3/3** and full Test262 CI `31113156512` passes **65/65**, including direct
**10/0/0**, fixed-offset **259/0** and **259/7**, and withTimeZone **16/0**
jobs. Evidence-doc commit `3b4b3ed` is pushed with `[skip ci]`.

Next recommended unit: implement complete currently reachable
`Temporal.PlainDateTime.from`, not parser-only or property-bag-only public
semantics. Two GPT-5.6 audits found 70 pinned files: 65 are reachable with the
current PlainDateTime/ZonedDateTime hidden kinds, while five require absent
PlainDate or sibling calendar-bearing Temporal types. Cover branded
PlainDateTime and ZonedDateTime, full PlainDateTime property-bag preparation,
String grammar, overflow/options ordering, Realm/range/GC/fuel behavior, and
exact manifests in one unit. This should unlock the 65 direct paths plus four
existing PlainDateTime accessor blockers, **69 exact paths** total. Preserve
BigInt-backed fields through observable option reads; do not reuse the
ZonedDateTime property-bag collector because its early positivity checks have
the wrong order. Expected installer accounting moves to roughly 93 maximum
live pins and 94 allocations. Keep the five PlainDate-dependent files as an
explicit complement and leave `datetime-math.js` for arithmetic.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainDateTime.from

Added Realm-local nonconstructable static `from` for branded PlainDateTime and
ZonedDateTime values, complete ISO property bags, and the audited PlainDateTime
String grammar. Property fields are read and coerced in spec order through a
BigInt-backed record, `options.overflow` is observed before required/range
validation, and constrain/reject, monthCode agreement, leap seconds, extreme
dates, hidden-slot copying, method-Realm results/errors, fuel, GC/OOM retry,
and installer rollback are covered. The shared parser now accepts bare
date-only input without widening date-only Z/offset syntax; optional offsets
and time-zone annotations after a time are validated then ignored. Installer
accounting is 93 maximum live pins and 94 allocation boundaries.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
is exact **64/0/0** for independently supported `from` paths and forced
**65/5/0** over all 70 files. Six files remain blocked: five require absent
PlainDate-family objects and one calls absent PlainDateTime `equals`.
`options-wrong-type.js` is deliberately blocked despite process success,
because the missing PlainDate constructor throws the expected TypeError before
the intended assertion. Four former `from`-dependent accessor paths moved into
the PlainDateTime core, now exact **105/0/0** and forced **105/1/0**; only
`datetime-math.js` remains there.

Local gates pass: workspace/all-target/all-feature tests including library
**434/434**, builtins **613/613**, fuel **45/45**, every integration and
benchmark target; tooling **197/197** with five expected unavailable-checkout
skips; direct/core exact and forced Test262; Clippy denied warnings; fmt/diff;
Python/YAML; generated Intl checks; vendored RegExp **38/38**, Clippy/no_std;
wasm32; release build and Realm rollback. Two early GPT-5.6 audits found and
resolved a date-only parser widening and a Test262 false-positive admission.
Final tooling/docs review is CLEAN; one extra runtime review was closed after
it did not complete, with full local and CI validation providing the final
runtime evidence. All agents are closed.

Implementation commit `31a7298` is pushed. Ordinary CI `31119606326` passes
**3/3** on attempt 4 after repeated setup-only runner failures. Full Test262 CI
`31119606515` passes **66/66** on attempt 3, including direct `from` exact
**64/0/0**, forced **65/5/0**, and PlainDateTime core exact **105/0/0** plus
forced **105/1/0**.

Next recommended narrow unit: audit and implement
`Temporal.PlainDateTime.prototype.equals`. It should remove the
`argument-propertybag-optional-properties.js` blocker and provide a reusable
hidden-slot conversion/equality boundary before static compare or arithmetic.
Keep PlainDate-family constructors and `datetime-math.js` as separately
measured units; do not admit `options-wrong-type.js` until its intended
assertions are actually reached.

Evidence-doc commit `5f1cf17` is pushed with `[skip ci]`. Final cleanup found
root/vendor Cargo targets and `/root/test262` already absent, then removed
Python/temp caches and pruned worktree metadata. Final verification found no
related Cargo/Test262/GitHub monitor/subagent process, one worktree, clean git,
`HEAD == origin/main` at `5f1cf17`, no workflow spawned for the evidence
commit, and 8.7 GiB free disk.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainDateTime.prototype.equals

Added Realm-local, nonconstructable `equals` on PlainDateTime.prototype. It
brands the receiver first, converts the argument through the shared
`to_temporal_plain_date_time` record boundary, compares all nine compact ISO
fields and canonical calendar identity, and returns a primitive Boolean without
allocating a result object. Static `from` is now the thin allocating wrapper
around the same converter. Hidden PlainDateTime/ZonedDateTime slots, complete
ISO property bags, audited strings, conversion order, negative fixed-offset
balancing, method-Realm errors, fuel, GC/OOM rollback, and no-result-allocation
heap caps are covered. Installer accounting is 94 maximum pins and 95 total
allocations; equals is the 93rd allocation with a dedicated baseline+92
failure boundary.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
is exact **39/0/0** for the independently reachable 41-file equals directory
and forced **39/2/0**. The two blockers are `argument-plaindate.js` and
`calendar-temporal-object.js`, both requiring absent PlainDate-family hidden
kinds. The downstream PlainDateTime.from boundary is now exact **65/0/0** with
five blockers and forced **66/4/0** over 70. `options-wrong-type.js` remains
blocked despite a process pass because absent PlainDate throws the expected
TypeError before the intended assertion.

Local gates pass: workspace/all-target/all-feature tests including library
**436/436**, builtins **615/615**, every integration and benchmark target;
fuel **46/46**; tooling **198/198** with five expected unavailable-checkout
skips plus live equals/from metadata tests; exact and forced equals/from
Test262 gates; Clippy denied warnings; fmt/diff; Python/YAML; generated Intl
checks; vendored RegExp **38/38**, Clippy/no_std; wasm32; release build and
release Realm rollback. Runtime and tooling/docs GPT-5.6 final reviews are
CLEAN. All review agents are closed.

Implementation commit `ed47123` is pushed. Ordinary CI `31195704556` passes
**3/3** and full Test262 CI `31195703365` passes **67/67**. Dedicated CI logs
confirm equals exact **39/0/0**, forced **39/2/0**, from exact **65/0/0**, and
forced **66/4/0**. Evidence-doc commit `291ee70` is pushed with `[skip ci]`.

Next recommended narrow unit: implement static
`Temporal.PlainDateTime.compare` using the same record converter. Read-only
GPT-5.6 reconnaissance audited 42 pinned files: exact reachable scope should
be **40/0/0**, forced **40/2/0**, with only `argument-plaindate.js` and
`calendar-temporal-object.js` blocked and no direct false positive. Convert
the first argument completely before touching the second, ignore calendar,
compare the nine ISO fields lexicographically, return exactly -1/+0/1, ignore
`this`, and cover Realm, ordering, fuel, GC/OOM, and allocation-free Number
results. Installing compare immediately after from should reserve 95 pins and
96 allocations; compare is allocation 70 (baseline+69 failure), existing
equals moves to allocation 94 (baseline+93), the boundary loop becomes
`17..96`, and success requires baseline+96. Re-audit these counts in code.
This also moves ZonedDateTime.compare from exact **48/0** to **49/0** and
forced **48/2** to **49/1** by admitting
`compares-exact-time-not-clock-time.js`. Intl402 gregory/era tests remain out.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainDate hidden-slot core

Added a distinct Realm-local `%Temporal.PlainDate%` hidden kind with compact
year/month/day and canonical calendar slots, constructor, 16 ISO accessors,
`valueOf`, and `@@toStringTag`. Date validity uses the spec noon boundary, so
the minimum and maximum PlainDate values remain valid even where midnight is
outside the PlainDateTime range. `ToTemporalDateTime` now recognizes branded
PlainDate values before property-bag processing, reads no shadowed properties,
observes overflow in the required order, and creates the midnight record only
when that PlainDateTime is representable. Installer accounting is 114 maximum
live pins and 115 allocations, with every boundary covered.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
records the direct core at exact **67/0/0** and forced **67/11/0** over 78;
all 11 blockers first require the deliberately separate
`Temporal.PlainDate.from` surface. The hidden conversion also releases four
PlainDateTime.from paths, one equals path, and one compare path. Their current
exact/forced boundaries are **69/0** and **69/1**, **40/0** and **40/1**, and
**41/0** and **41/1**. The remaining calendar-object cases need absent
PlainMonthDay/PlainYearMonth and stay blocked.

Local gates pass: all-target/all-feature library **440/440**, builtins
**620/620**, all integration and benchmark targets, tooling **200/200** with
five expected unavailable-checkout skips, all exact/forced Temporal gates,
Clippy denied warnings, fmt/diff, Python/YAML, generated Intl checks, vendored
RegExp **38/38** plus Clippy/no_std, wasm32, release build, and the exact
release Realm rollback test. Runtime GPT-5.6 review is CLEAN. Tooling/docs
review found one path-helper input-contract gap; both runner and analyzer were
fixed and the regression test passes. All review agents were closed.

Implementation commit `977efa9` is pushed. Ordinary CI `31207027893` passes
**3/3**. Full Test262 CI `31207028013` passes **68/68**. Dedicated jobs are
PlainDate core `92961099996`, PlainDateTime.from `92961100084`, equals
`92961100160`, and compare `92961100106`; CI logs reproduce the exact and
forced counts above.

Next recommended unit: implement complete currently reachable
`Temporal.PlainDate.from`, not an accessor-unblocking stub. Pinned direct
directory contains 71 files. Reuse hidden PlainDate/PlainDateTime/ZonedDateTime
fast paths and the existing ISO parser grammar, but add a date-only parsed
record and a dedicated property collector. Observable property order must be
`calendar`, `day`, `month`, `monthCode`, `year`; reading PlainDateTime time
fields would be a spec regression. Parse invalid strings before options,
observe overflow before algorithmic field validation, validate PlainDate at
noon, allocate in the method Realm, and measure the calendar-temporal-object
dependency rather than assuming all 71 paths are independently reachable.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainDate.from

Added Realm-local, nonconstructable static `from` for branded PlainDate,
PlainDateTime, and ZonedDateTime hidden slots, complete ISO date property bags,
and the audited PlainDate String grammar. The dedicated property collector
reads and coerces only `calendar`, `day`, `month`, `monthCode`, and
`year` in that order, so PlainDateTime time getters remain unobserved.
Syntactically invalid strings and non-String primitives fail before options;
parsed strings and prepared property bags observe `options.overflow` before
algorithmic date/range validation. Constrain/reject, monthCode agreement, leap
seconds, offsets/annotations, UTC-designator rejection, extreme noon range,
hidden copies, method-Realm results/errors, fuel, GC/OOM retry, and installer
rollback are covered. Installer accounting is 115 maximum live pins and 116
allocations; `PlainDate.from` is allocation 97.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
is exact **70/0/0** for the independently reachable 71-file direct directory
and forced **70/1/0**. The sole blocker,
`calendar-temporal-object.js`, requires absent PlainMonthDay and
PlainYearMonth hidden kinds. The former 11 PlainDate core blockers are all
admitted, making the complete core exact **78/0/0**. New tooling freezes
features, includes, flags, negative metadata, live complement, malformed path
contracts, runner/analyzer parity, and exact/future-sibling gating.

Local gates pass: the broad pre-review run completed library **443/443**,
builtins **622/622**, every integration target and benchmark smoke, root
Clippy with denied warnings, and fmt/diff. Tooling passes **201/201** with the
expected unavailable-checkout skips; direct/core exact and forced Test262,
generated Intl checks, vendored RegExp **38/38** plus Clippy/no_std, wasm32,
release build, and release Realm rollback pass. Final runtime GPT-5.6 review
found one P1: parsed out-of-range strings validated range before overflow. The
call was moved to `parse -> overflow -> range`, a getter-abrupt regression
test was added, and focused runtime/OOM/fuel, exact/forced Test262, Clippy, and
fmt were rerun successfully. Final tooling/docs review is CLEAN. All agents
are closed.

Implementation commit `9a494ed` is pushed. Ordinary CI `31212928609`
passes **3/3**. Full Test262 CI `31212928542` passes **70/70**. Dedicated
PlainDate.from job `92980709053` records exact **70/0/0** and forced
**70/1/0**; updated PlainDate core job `92980708761` records exact and
forced **78/0/0**.

Next recommended narrow unit: audit and implement static
`Temporal.PlainDate.compare` on the shared `to_temporal_plain_date` record
boundary. Convert the first argument completely before touching the second,
ignore calendar identity, compare year/month/day lexicographically, return
primitive -1/+0/1 without result allocation, ignore `this`, and cover
method Realm, abrupt ordering, fuel, installer/GC/OOM boundaries, exact
Test262 admission, and false-positive TypeError cases. Measure the pinned
directory and downstream effects before claiming counts; keep calendar sibling
dependencies explicit.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainDate.compare

Added Realm-local, nonconstructable static `Temporal.PlainDate.compare` on the
shared `to_temporal_plain_date` record boundary. It converts the first operand
fully before touching the second, compares only ISO year/month/day, ignores
calendar identity and `this`, and returns allocation-free `-1`, positive zero,
or `1`. Direct regressions cover branded hidden-slot non-observation,
PlainDateTime/ZonedDateTime conversion, property order, first-abrupt short
circuit, number and wrong-calendar TypeErrors, method Realm, cross-Realm
operands, nonconstruction, string fuel, observable GC rooting, installer
rollback, and zero-cap result execution. Installer accounting is now 116
maximum live pins and 117 allocations; compare is allocation 98.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
is direct exact **41/0/0** and forced **41/1/0** over 42 files. The direct
blocker requires absent PlainMonthDay/PlainYearMonth. A separate Intl402
contract is exact **1/0/0** and forced **1/2/0** over three files; its two
blockers require non-ISO `gregory` calendar construction and field semantics.
Both directories freeze exact paths, features/includes/flags/negative
metadata, live complements, malformed/future paths, and runner/analyzer parity.

Local gates pass: all-target/all-feature Rust tests including library
**445/445**, builtins **625/625**, every integration target and benchmark
smoke; Python tooling **203/203**; warnings-denied root Clippy; fmt/diff/YAML;
generated Intl aliases and locale-info; vendored RegExp **38/38** plus
Clippy/no_std; wasm32; release build and exact Realm rollback; and all four
direct/Intl exact/forced Test262 gates. Final GPT-5.6 runtime review is clean.
Tooling/docs review found one P3 stale-current installer count; both old values
were relabeled as historical checkpoints, leaving the current 116/117 count
unambiguous. All agents are closed.

Implementation commit `8c3268a` is pushed. Ordinary CI `31219039049` passes
**3/3**. Full Test262 CI `31219039216` passes **71/71**. Dedicated compare job
`93000054872` logs direct exact **41/0/0**, direct forced **41/1/0**, Intl402
exact **1/0/0**, and Intl402 forced **1/2/0**.

Next recommended narrow unit: audit `Temporal.PlainDate.prototype.equals` and
its downstream users before implementation. Reuse the date-only converter,
brand the receiver before argument conversion, compare ISO fields plus
calendar identity, preserve method-Realm errors and observable GC roots, and
measure the pinned direct/downstream corpus before claiming any admission.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainDate.prototype.equals

Added Realm-local, nonconstructable `Temporal.PlainDate.prototype.equals` on
the shared date-only record boundary. It brands the receiver first, converts
the argument with `ToTemporalDate`, compares ISO year/month/day, then calendar
identity, and returns an allocation-free Boolean. Tests cover hidden-slot
non-observation, all supported branded/property-bag/String forms, brand-first
abrupt completion, method-Realm errors, cross-Realm values, nonconstruction,
observable GC rooting, exact string fuel, installer OOM rollback, and an exact
zero-extra-object heap cap. Installer accounting is now 117 maximum live pins
and 118 allocations; equals is allocation 116.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
is direct exact **39/0/0** and forced **39/1/0** over 40 files. The blocker
requires absent PlainMonthDay/PlainYearMonth. Intl402 is exact **1/0/0** and
forced **1/5/0** over six files; blockers require non-ISO calendar semantics.
Four true downstream callers under add/subtract and Chinese/Dangi monthCode are
frozen separately as non-admitting dependencies, completing the audited
50-file equality surface. Admissions freeze paths, metadata, live complements,
runner/analyzer parity, and absent or unreadable corpus behavior.

Local gates pass: all-target/all-feature Rust tests including library
**447/447**, builtins **628/628**, every integration target and benchmark
smoke; Python tooling **204/204** with full pinned corpus and **204/204** with
the corpus unavailable; forced `PermissionError`; warnings-denied root Clippy;
Rust 1.88; fmt/diff/YAML/Python; generated Intl aliases and locale-info;
vendored RegExp **38/38** plus Clippy/no_std; wasm32; release build and exact
Realm rollback; and all four direct/Intl exact/forced Test262 gates. Final
GPT-5.6 runtime and tooling/docs reviews are CLEAN. Residual risk is explicit:
ISO-only execution cannot yet distinguish full non-ISO CalendarEquals behavior.

Implementation commit `2d6937d` and CI portability fix `cb080ae` are pushed.
The first ordinary CI exposed an unreadable `/root/test262` default and was
fixed by treating `OSError` as unavailable corpus, matching existing tooling.
Latest ordinary CI `31226030768` passes **3/3**. Latest full Test262 CI
`31226030571` passes **72/72**. Dedicated equals job `93020874434` logs direct
exact **39/0/0**, direct forced **39/1/0**, Intl402 exact **1/0/0**, and
Intl402 forced **1/5/0**. All review agents are closed.

Next recommended narrow unit: audit `Temporal.PlainDate.prototype.toString`
and its direct/downstream pinned corpus before implementation. Reuse hidden
date slots and the existing Temporal string parser/formatter boundaries, but
measure options ordering, calendar annotation modes, method-Realm errors,
fuel, GC, and non-ISO blockers before admitting paths.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainDate.prototype.toString

Added Realm-local, nonconstructable `Temporal.PlainDate.prototype.toString`.
It brands before observing options, accepts undefined/object/function options,
reads and coerces only `calendarName`, and formats copied hidden ISO date and
calendar slots. A date-only formatter reuses ISO extended-year padding and a
new calendar-annotation writer shared with ZonedDateTime, without importing
time/offset/time-zone semantics. Direct tests cover hidden getter
non-observation, all annotation modes, extended years, option ordering and
types, method-Realm errors, cross-Realm values, observable GC success and
abrupt pin cleanup, byte fuel, zero-extra-object heap execution, and installer
rollback. Installer accounting is now 118 maximum live pins and 119
allocations; toString is allocation 117.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
is direct exact and forced **18/0/0** over the complete 18-file directory.
Four files that previously passed through inherited Object.prototype.toString
are explicitly frozen false-positive risks. Intl402 has no independently
reachable admission: exact **0/0/8** and forced **0/8/0** because every file
combines non-ISO gregory or Persian construction/conversion. Whole-corpus audit
found zero true JS-level downstream callers outside the 26-file surface.

Local gates pass: all-target/all-feature Rust library **451/451**, builtins
**631/631**, fuel **52/52**, every integration target and benchmark smoke;
Python tooling **205/205**; warnings-denied root Clippy; Rust 1.88; fmt/diff,
Python/YAML; generated Intl data; vendored RegExp **38/38** plus Clippy/no_std;
wasm32; release build and exact Realm rollback; and all four direct/Intl
Test262 gates. Two final GPT-5.6 reviews are CLEAN. Early audit-agent release
output was identified as stale and discarded; agents were stopped before the
single authoritative main build and all agents are now closed.

Implementation commit `4b2afcf` is pushed. Ordinary CI `31230908261` passes
**3/3**. Full Test262 CI `31230908272` passes **73/73**. Dedicated toString job
`93034889261` logs direct exact **18/0/0**, direct forced **18/0/0**, Intl402
exact **0/0/8**, and Intl402 forced **0/8/0**.

Next recommended narrow unit: audit the PlainDate serialization siblings,
starting with `Temporal.PlainDate.prototype.toJSON`, while measuring whether
`toLocaleString` must remain owned by the incomplete Intl.DateTimeFormat
boundary. Reuse the date-only hidden formatter, but preserve each method's
distinct options, Realm, and downstream Test262 contract rather than aliasing
methods blindly.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainDate.prototype.toPlainDateTime

Added Realm-local, nonconstructable length-0
`Temporal.PlainDate.prototype.toPlainDateTime`. Receiver branding precedes
argument observation. Undefined becomes midnight; PlainDateTime and
ZonedDateTime use hidden local-time slots; property bags read hour,
microsecond, millisecond, minute, nanosecond, second in order with immediate
integer conversion and constrain overflow; Strings validate the audited
time-context grammar, reject Z, clamp leap seconds, and precharge input bytes.
The final PlainDateTime uses the receiver date/calendar and method Realm.
Installer accounting is 120 maximum pins and 121 allocations.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
is direct exact **32/0/0**, blockers exact **0/0/3**, and forced direct
**32/3/0** over 35 files. The three blockers require the absent
`%Temporal.PlainTime%` intrinsic. One Intl402 non-ISO calendar caller is exact
**0/0/1** and forced **0/1/0** as a non-admitting downstream dependency.
Admissions freeze complete paths, metadata, live complement, runner/analyzer
parity, forced per-file identity, future/outside/malformed paths, unavailable
corpus, and explicit PermissionError behavior.

Local gates pass: all-target/all-feature Rust tests including library
**457/457**, builtins **636/636**, every integration target and benchmark
smoke; Python tooling **207/207** with pinned corpus; root warnings-denied
Clippy; Rust 1.88; fmt/diff/YAML/Python; generated Intl aliases and locale-info;
vendored RegExp **38/38** plus Clippy/no_std; wasm32; release build and exact
Realm rollback; and all five exact/forced Test262 gates. Two GPT-5.6 final
reviews found the expected PlainTime gap, a missing late result-root failure
test, and stale architecture checkpoint wording. The test and wording were
fixed; PlainTime remains a documented next unit. All agents are closed.

Implementation commit `edb5608` and CI portability fix `a422ba6` are pushed.
The first ordinary CI exposed unreadable `/root/test262` handling and was fixed
with a direct PermissionError regression. Latest ordinary CI `31239261899`
passes **3/3**. Latest full Test262 CI `31239261901` passes **75/75**.
Dedicated job `93057772362` logs direct exact **32/0/0**, blockers
**0/0/3**, forced direct **32/3/0**, downstream exact **0/0/1**, and
downstream forced **0/1/0**.

Next recommended narrow unit: implement Realm-local `%Temporal.PlainTime%`
hidden slots, constructor/accessor core, and the ToTemporalTime fast path. Then
move the three direct blockers into admission and re-audit all PlainTime
downstream callers. Keep non-ISO calendar mutation separate.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainTime hidden-slot core

Added Realm-local `%Temporal.PlainTime%` with compact immutable six-field
hidden slots, subclass/newTarget prototype selection, six branded accessors,
`@@toStringTag`, static `from`, prototype `equals`, and always-throwing
`valueOf`. PlainTime, PlainDateTime, ZonedDateTime, ordered time property bags,
and audited Strings share one ToTemporalTime record conversion. `from` creates
a fresh method-Realm object; equals is allocation-free. PlainDate
`toPlainDateTime` now consumes the PlainTime hidden fast path. Constructor,
conversion, options, Realm, GC/root/OOM retry, fuel, and installer rollback are
covered. Installer accounting is 131 maximum live pins and 132 allocations;
Realm registry inventory is 59 values.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
passes exact and forced PlainTime **129/0/0**: core **40**, from **51**,
valueOf **7**, equals **31**. PlainDate `toPlainDateTime` direct coverage is now
complete **35/0/0**, with zero direct blockers; the separate Intl402 non-ISO
calendar downstream remains exact **0/0/1** and forced **0/1/0**. Exact
PlainTime helper construction also moves the ZonedDateTime fixed-offset
boundary from **259/7** to **260/6** over 266 files. Exact
manifests freeze live complements, metadata, runner/analyzer parity,
future/outside paths, and inaccessible corpus/file handling. Ordinary and full
CI both gained dedicated exact gates.

Local gates pass: all-target/all-feature Rust tests including library
**461/461**, builtins **639/639**, all integrations and benchmark smoke;
warnings-denied root Clippy; Rust 1.88; wasm32; generated Intl data; vendored
RegExp **38/38** plus Clippy/no_std; Python tooling **208/208**; fmt/diff/YAML;
release build and exact Realm rollback; exact PlainTime **129/129** and bridge
**35/35** diagnostics. An initial parallel gate attempt hit ENOSPC only; after
mandatory artifact cleanup every command passed sequentially. GPT-5.6 runtime
review was CLEAN. GPT-5.6 tooling review found stale full-workflow 32+3 counts,
missing unreadable-file coverage, and stale bridge architecture wording; all
were fixed and reverified. All agents are closed.

Implementation commit `b487e0f` and downstream admission fix `223b1df` are
pushed to `main`. The first full run exposed the newly reachable ZonedDateTime
plural-unit helper; local exact/forced reruns and tooling moved the fixed-offset
boundary to 260/6. Superseded runs `31242924727` and `31242924738` were
cancelled after the fix. Ordinary CI `31243409332` passes **3/3**. Full
Test262 CI `31243409334` passes **76/76** after retrying one unrelated
`language/keywords` artifact-upload failure; the retry passed both the shard
and upload step. Dedicated PlainTime, PlainDate.toPlainDateTime, and
ZonedDateTime fixed-offset jobs all pass their exact **129/0**, **35/0**, and
exact/forced **260/6** contracts.

Next recommended narrow unit: audit and implement static
`Temporal.PlainTime.compare`. It can reuse the allocation-free ToTemporalTime
record and should first freeze the complete pinned direct directory, including
conversion order, method-Realm errors, String/property-bag inputs, fuel, and
any genuine external blockers. Keep arithmetic/difference/rounding and locale
serialization as separate units.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainTime.compare

Added Realm-local, nonconstructable length-2
`Temporal.PlainTime.compare`. It reuses the shared allocation-free
ToTemporalTime record conversion, completes the first operand before observing
the second, and lexicographically compares hour through nanosecond to return
exactly -1, +0, or 1. Branded PlainTime/PlainDateTime/ZonedDateTime values use
hidden slots; property bags retain ordered getter/coercion semantics; Strings
retain byte fuel and the audited grammar. The receiver is ignored and native
errors use the function Realm. Installer accounting is now 132 maximum live
pins and 133 allocations; compare is allocation 131 and existing allocation
numbers remain stable. Runtime tests cover all six ordering fields in both
directions, +0, hidden getter non-observation, cross-Realm inputs/errors,
abrupt identity and second-input suppression, independent operand fuel,
zero-result allocation, and exact installer rollback/success boundaries.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
has exactly 32 direct compare files and no external callers. Forced execution
is **32/0/0**. A fifth exact PlainTime manifest freezes all metadata, live
directory complement, future compare paths, runner/analyzer parity, and
per-file results. Combined PlainTime admission is now exact **161/0/0**:
core 40, from 51, valueOf 7, equals 31, compare 32. Before implementation,
`argument-number.js` and `not-a-constructor.js` were false positives caused by
the absent method; exact admission does not rely on those results.

Local gates pass: all-feature library **463/463**, every integration target
including builtins **640/640** and fuel **55/55**, warnings-denied root Clippy,
Rust 1.88-compatible code, fmt/diff/Python/YAML, generated Intl data,
benchmark link smoke, wasm32, release library **463/463**, vendored RegExp
**38/38** plus Clippy/no_std, Python tooling **208/208**, and exact/forced
PlainTime **161/161**. Parallel all-target linking twice hit disk-induced lld
SIGBUS only; clean `CARGO_INCREMENTAL=0 -j1` partitioned runs passed fully.
Two GPT-5.6 final reviews are CLEAN after adding a missing future compare-path
rejection test. All agents are closed.

Commit `5ee358b` (`feat(temporal): add PlainTime compare`) is pushed to `main`.
Ordinary CI `31246715320` passes **3/3**. Full Test262 CI `31246715310`
passes **76/76**; dedicated job `93076755654` logs
`RATE=100.0 PASS=161 FAIL=0 SKIP=0 TOTAL=161 RAN=161`.

Next recommended narrow unit: audit and implement
`Temporal.PlainTime.prototype.toString`, reusing only the shared time record
and ISO/RFC 9557 formatting primitives while preserving its distinct options,
precision, rounding, Realm, fuel, and complete direct Test262 contract. Keep
toJSON/toLocaleString and arithmetic/difference/rounding as separate units.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainTime.prototype.toString

Added Realm-local, nonconstructable length-0
`Temporal.PlainTime.prototype.toString`. It brands before options, copies the
six hidden fields, then reads and converts `fractionalSecondDigits`,
`roundingMode`, and `smallestUnit` in specification order. The formatter
reuses the existing Temporal precision and rounding records plus a shared ISO
time writer, supports auto or 0-9 fractional digits, minute through
nanosecond units including accepted plurals, every rounding mode, carry, and
midnight rollover. Public accessors are not observed and the String primitive
result requires no GC heap object. Installer accounting is 133 maximum live
pins and 134 allocations; toString is allocation 132, so prior indices remain
stable.

Runtime coverage includes function shape/nonconstructability, option order and
coercion, brand-first failures, hidden getter non-observation, all precision
and rounding behavior, cross-Realm receivers/errors, abrupt identity, fuel for
all three produced option strings, allocation-free output, exact installer
rollback/success boundaries, forced GC in every getter/coercion, both options
and coercion root-reservation failures, non-observation after failure, pin/live
restoration, and retry. Refactoring Instant/ZonedDateTime time emission is
protected by its existing tests plus a dedicated PlainTime formatter unit.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
contains exactly 40 direct toString files and no external callers. All 40 pass
forced execution. A sixth exact PlainTime manifest freezes the complete live
directory, metadata, runner/analyzer parity, future/outside paths, and
per-file results. Combined PlainTime admission is exact **201/0/0**: core 40,
from 51, valueOf 7, equals 31, compare 32, toString 40. Before implementation,
`not-a-constructor.js`, `options-invalid.js`, and `options-wrong-type.js` were
false positives; exact admission does not rely on those results. The remaining
unadmitted PlainTime corpus is 292 of 493 files.

Local gates pass: all-target/all-feature Rust tests and Criterion smoke,
warnings-denied root Clippy, rustfmt/diff, Rust 1.88 MSRV, wasm32, generated
Intl data, workflow YAML, release build and Realm rollback, vendored RegExp
**38/38** plus Clippy/no_std, Python tooling **208/208** with five expected
absent-corpus skips, live exact-manifest tooling, and release PlainTime
**201/201**. Two GPT-5.6 final reviews are CLEAN after strengthening observable
GC/root-failure tests and eliminating two false-positive assertions. All
agents are closed.

Commit `4f44009` (`feat(temporal): add PlainTime toString`) is pushed to
`main`. Ordinary CI `31249739119` passes **3/3**. Full Test262 CI
`31249739122` passes **76/76**; dedicated job `93084063861` logs
`RATE=100.0 PASS=201 FAIL=0 SKIP=0 TOTAL=201 RAN=201`.

Next recommended narrow unit: audit and implement
`Temporal.PlainTime.prototype.toJSON`, first freezing its complete pinned
direct directory and all downstream callers. Reuse hidden-slot serialization
without dispatching through mutable `toString`; keep locale formatting and
arithmetic/difference/rounding methods separate.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainTime.prototype.toJSON

Added Realm-local, nonconstructable length-0
`Temporal.PlainTime.prototype.toJSON`. It brands and copies the hidden time
fields, ignores every argument, and serializes directly with auto precision.
It does not observe public accessors, supplied Proxy arguments, or a mutable
`toString`. The shared formatter remains allocation-free for the returned
String primitive. Installer accounting is 134 maximum live pins and 135
allocations; toJSON is allocation 133.

Runtime coverage includes function shape/nonconstructability, ignored
arguments, hidden getter and overridden toString non-observation, automatic
trailing-zero formatting, `JSON.stringify`, Realm-local TypeErrors, and
cross-Realm methods and receivers in every direction. Allocation tests cover
the exact installer failure/success boundary, publication, and allocation-free
serialization under an exact heap cap.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
contains exactly seven direct toJSON files and no downstream callers. All seven
pass forced execution. The exact PlainTime manifests now admit **208/208**;
285 of 493 PlainTime files remain unadmitted. The prior only false positive was
`not-a-constructor.js`.

Local gates pass: library **469/469**, builtins **642/642**, all other Rust
integration tests and Criterion smoke, warnings-denied Clippy, rustfmt/diff,
Rust 1.88 MSRV, wasm32, generated Intl data, workflow YAML, release Realm
rollback, vendored RegExp **38/38** plus Clippy/no_std, Python tooling
**208/208** with five expected absent-corpus skips, and release PlainTime
**208/208**. Runtime and tooling GPT-5.6 reviews are CLEAN. All agents and
temporary build/corpus artifacts are closed or removed.

Commit `920c23e` (`feat(temporal): add PlainTime toJSON`) is pushed to `main`.
Ordinary CI `31287052145` passes **3/3**; dedicated job `93177673553` logs
`RATE=100.0 PASS=208 FAIL=0 SKIP=0 TOTAL=208 RAN=208`. Full Test262 CI
`31287052137` passes **76/76** with no skipped jobs.

Next recommended narrow unit: audit `Temporal.PlainTime.prototype.round` as a
self-contained unit. Keep `toLocaleString` separate because its
Intl.DateTimeFormat dependency gives it a materially broader implementation
surface.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainTime.prototype.round

Added Realm-local, nonconstructable length-1
`Temporal.PlainTime.prototype.round`. It brands before its required argument,
accepts primitive String shorthand without observing `Object.prototype`, or
roots an options object while reading and coercing `roundingIncrement`,
`roundingMode`, and `smallestUnit` in specification order. It supports hour
through nanosecond, every valid divisor and all nine Temporal rounding modes,
wraps midnight, ignores public time getters and receiver subclasses, and
creates a fresh result in the native method Realm.

Rounding uses exact i128 values because nanoseconds per day exceed the JS safe
integer range. The field-producing helper rounds the specification's
unit-relative quantity, then balances it with preserved higher fields. This
detail keeps non-default-increment `halfEven` parity correct; GPT review caught
and the regression now freezes `01:10` rounded to 20 minutes as `01:00`.

Installer accounting is 135 maximum live pins and 136 allocations; round is
allocation 134, preserving every earlier index. Runtime coverage includes
shape/brand/abrupt identity, hidden getter and prototype-pollution
non-observation, option order/coercion, increment truncation/divisibility,
mode ties, midnight rollover, cross-Realm methods/receivers/errors/results,
subclass ignoring, exact result allocation failure, forced GC in every
getter/coercion, options/coercion/prototype root-reservation failures, cleanup,
retry, and produced-string fuel.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
contains exactly 42 direct round files and no external callers. All 42 pass
forced execution. Exact PlainTime admission is now **250/250** and 243 of 493
files remain unadmitted. Before implementation only `options-wrong-type.js`
was a forced false positive; direct absence simulation confirms
`not-a-constructor.js` fails because `isConstructor` rejects non-functions.

Local gates pass: all-target/all-feature Rust library **472/472**, builtins
**644/644**, all integrations and Criterion smoke, warnings-denied Clippy,
rustfmt/diff, Rust 1.88 MSRV, wasm32, release Realm rollback, generated Intl
data, workflow YAML, Python tooling, release PlainTime **250/250**, and
vendored RegExp **38/38** plus Clippy/no_std. Runtime and tooling GPT-5.6 final
reviews are CLEAN after the halfEven fix and false-positive execution check.
All agents are closed.

Commit `650d826` (`feat(temporal): add PlainTime round`) is pushed to `main`.
Ordinary CI `31301752645` passes **3/3**; dedicated job `93215546311` logs
`RATE=100.0 PASS=250 FAIL=0 SKIP=0 TOTAL=250 RAN=250`. Full Test262 CI
`31301752640` passes **76/76** with no skipped jobs.

Next recommended narrow unit: audit `Temporal.PlainTime.prototype.with`. It
can reuse hidden slots, ordered partial time-field conversion, overflow
regulation, method-Realm creation, and the existing PlainTime OOM/GC
infrastructure. Keep add/subtract and since/until separate because they first
need broader Duration conversion and difference machinery; keep locale work
behind real Intl.DateTimeFormat support.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainTime.prototype.with

Added Realm-local, nonconstructable length-1
`Temporal.PlainTime.prototype.with`. It brands the receiver before observing
arguments, rejects `calendar` and `timeZone`, reads the six time fields in
specification order, and merges only present fields with the receiver's hidden
slots. Instant and Duration objects remain valid ordinary property bags.

Partial time input is represented as an optional-field record instead of
sentinel numeric values. This preserves the specification distinction between
an absent property and a present property whose coercion yields zero, while
letting the shared overflow regulator handle `constrain` and `reject` without
re-observing user objects. Results use the intrinsic PlainTime constructor from
the method Realm and ignore receiver subclasses and public getters.

Installer accounting is 136 maximum live pins and 137 allocations; `with` is
allocation 135. Runtime coverage includes receiver/argument error precedence,
observable field order and abrupt identity, forbidden properties, empty bags,
constrain/reject overflow, cross-Realm methods/receivers/errors/results,
subclass ignoring, forced GC at every getter/coercion, root reservation and
result-allocation OOM, cleanup/retry, and produced-string fuel accounting.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
contains 22 direct `with` files. Exactly 21 pass forced execution; the remaining
`plaintimelike-invalid.js` is blocked before `with` because RuJa does not yet
provide PlainMonthDay and PlainYearMonth constructors. Combined exact
PlainTime admission is **271/271**, leaving 222 of 493 files unadmitted.

Local gates pass: all-target/all-feature Rust library **475/475**, builtins
**647/647**, all integrations and Criterion smoke, warnings-denied Clippy,
rustfmt/diff, Rust 1.88 MSRV, wasm32, release Realm rollback, generated Intl
data, workflow YAML, Python tooling **208/208**, exact PlainTime **271/271**,
and vendored RegExp **38/38** plus Clippy/no_std. Forced direct `with` reports
`RATE=95.5 PASS=21 FAIL=1 SKIP=0 TOTAL=22 RAN=22`. Two runtime GPT-5.6 reviews
and the final tooling review are CLEAN.

Commit `9157521` (`feat(temporal): add PlainTime with`) is pushed to `main`.
Ordinary CI `31304741505` passes **3/3**; dedicated job `93223703895` logs both
exact rates above. Full Test262 CI `31304741512` passes **76/76** with no skipped
jobs.

Next recommended narrow unit: audit the shared Temporal duration conversion
needed by `Temporal.PlainTime.prototype.add` and `subtract`, landing both only
if their observable conversion and balancing contract is genuinely shared.
Keep `since`/`until`, locale/Intl.DateTimeFormat, and
PlainMonthDay/PlainYearMonth as separate units.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.Duration.from

Added Realm-local, nonconstructable length-1 `Temporal.Duration.from` and the
shared `ToTemporalDuration` conversion boundary required by later arithmetic.
Branded Duration inputs copy all ten hidden fields without observing public
getters. Ordinary objects remain rooted while plural fields are read and
immediately converted in `days`, `hours`, `microseconds`, `milliseconds`,
`minutes`, `months`, `nanoseconds`, `seconds`, `weeks`, `years` order. At least
one recognized field is required; the existing BigInt validator enforces
integer values, one sign, 2^32 date-unit limits, and the normalized 2^53-second
time limit before allocation.

Primitive Strings are fuel-precharged and parsed by a checked-decimal ISO
Duration parser. It supports signs, case-insensitive units, comma/period
fractions, and exact i128 decomposition of up to nine fractional digits on the
final hour, minute, or second component. It rejects malformed order, trailing
subcomponents after a fraction, infinity, mixed signs, and range overflow.
Fresh clones use the method Realm intrinsic and ignore receiver subclasses.

Runtime coverage includes parser exactness/invalid grammar, observable getter
and coercion order, hidden getter non-observation, fresh copies, cross-Realm
inputs/results/errors, nonconstruction, forced GC in getter/coercion, input and
result-prototype root failures, result heap OOM and retry, exact source/field
String fuel, installer rollback, and initial root reservation failure.
`Duration.from` is allocation 136; all earlier indices remain stable. The
installer reserves 137 maximum live pins across 138 allocations.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
direct `Duration.from` is exact **29/29**. Forced complete-directory execution
is **29/2** over 31 files: one blocker reaches absent `Duration.with/total`,
and one reaches absent `Duration.toString`. Duration core moves
`prototype/blank/basic.js` into admission, becoming exact **77/77** and forced
**77/1** over 78. Both diagnostics verify per-path pass/fail identities before
aggregate rates; configured corpus absence/access errors fail closed.

Local gates pass: all-target/all-feature Rust library **479/479** and builtins
**649/649** before the final isolated installer reservation regression; that
new test also passes directly. All integrations and Criterion smoke,
warnings-denied Clippy, rustfmt/diff, Rust 1.88 MSRV, wasm32, release Realm
rollback, generated Intl data, workflow YAML, Python tooling **209/209** with
five expected absent-corpus skips, and vendored RegExp **38/38** plus
Clippy/no_std pass. Final release Test262 reproduces direct `29/0`, forced
`29/2`, core `77/0`, and forced core `77/1`. GPT runtime/tooling reviews found
pin-reservation and diagnostic fail-open issues; all were fixed and targeted
configured/optional corpus and identity-drift tests pass.

Commit `dc1f31c` (`feat(temporal): add Duration from`) is pushed to `main`.
Ordinary CI `31328007490` passes **3/3**. Full Test262 CI `31328007497` passes
**77/77**; dedicated jobs `93282062906` and `93282063061` log all four exact
rates above.

Next recommended narrow unit: implement `Temporal.PlainTime.prototype.add`
and `subtract` together through one operation-parameterized path. Reuse the
new `ToTemporalDuration`, validate all fields but ignore years/months/weeks/days
for clock arithmetic, convert each time field to exact i128/BigInt
nanoseconds before summing, negate only after successful conversion for
subtract, wrap with Euclidean modulo 24 hours, ignore the options argument,
and create the result in the method Realm. The pinned direct surface is 32
files per method, 64 total, with no remaining Duration.from blocker.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.PlainTime.prototype.add and subtract

Added Realm-local, nonconstructable length-1 `Temporal.PlainTime.prototype.add`
and `subtract` through one shared arithmetic path. Both brand the receiver
before observing the duration-like argument and reuse `ToTemporalDuration` for
branded hidden-slot copies, rooted ordered property bags, and checked ISO
Strings. All ten fields remain validated; years, months, weeks, and days are
ignored only when computing the clock result. The six time fields are rebuilt
as exact BigInt integers, converted separately to nanoseconds, summed within
i128, and balanced with Euclidean modulo over 24 hours. Subtract negates only
after conversion succeeds. Options and later arguments are never observed.

Results always use the native method Realm intrinsic PlainTime prototype and
ignore receiver subclasses, public time getters, constructors, and species.
Runtime coverage freezes both-direction midnight wrapping, maximum exact
seconds, all time-unit carry, date-unit ignoring after validation, all ten
getter/coercion order, abrupt identity, brand precedence, hidden Duration
getter non-observation, ignored Proxy options, cross-Realm results/errors,
nonconstruction, forced getter/coercion GC, input/result-prototype reservation
failures, result heap OOM/retry, and exact String/field fuel. `Duration.from`
remains allocation 136; add is 137 and subtract 138. The installer reserves
139 maximum live pins across 140 allocations, with every boundary tested.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
contains exactly 32 direct add files and 32 direct subtract files. Exact and
forced execution are **64/64**, moving the combined admitted PlainTime surface
from **271/271** to **335/335**. Shared metadata, runner/analyzer membership,
per-path diagnostic identity, live corpus complements, future-path rejection,
and ordinary/full workflow rates are frozen at the new boundary.

Local gates pass: all-target/all-feature Rust library **484/484**, builtins
**652/652**, every integration and Criterion smoke; warnings-denied Clippy;
rustfmt/diff; Rust 1.88 MSRV; wasm32; release Realm rollback; generated Intl
data; workflow YAML; Python tooling **209/209** with five expected absent-corpus
skips; and vendored RegExp **38/38** plus Clippy/no_std+alloc. Two independent
GPT-5.6 final reviews are CLEAN.

Commit `12c3815` (`feat(temporal): add PlainTime arithmetic`) is pushed to
`main`. Ordinary CI `31331430313` passes **3/3**. Full Test262 CI
`31331430291` passes **77/77**; its dedicated `temporal-plain-time` job passes
the exact **335/335** boundary.

Next recommended narrow unit: audit `Temporal.Duration.prototype.with` as the
smallest consumer of the complete ten-field hidden record and ordered partial
DurationLike conversion. Keep `Duration.prototype.total/toString`, PlainTime
`since/until`, calendar-relative arithmetic, and locale formatting separate.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.Duration.prototype.with

Added Realm-local, nonconstructable length-1
`Temporal.Duration.prototype.with`. Receiver branding precedes every argument
observation. The required argument must be an Object and remains rooted while
the ten plural duration properties are read and immediately converted in
`days`, `hours`, `microseconds`, `milliseconds`, `minutes`, `months`,
`nanoseconds`, `seconds`, `weeks`, `years` order. Branded Duration arguments
deliberately use this public property-bag path, so overridden getters remain
observable.

The shared object reader now returns an optional-field partial record without
validating it in isolation. `Duration.from` completes that record against zero;
`Duration.prototype.with` completes it against copied receiver slots. Final
sign/range validation occurs only after merge, permitting complete sign
replacement while rejecting conflicts with retained receiver fields.
Undefined fields preserve receiver values. Results use the native method Realm
intrinsic Duration prototype and ignore subclasses, constructors, and species.

Runtime coverage freezes brand/error precedence, primitive and empty-object
rejection, all getter/coercion order and abrupt identity, branded getter
observation, undefined merge, sign replacement/conflict, exact range,
cross-Realm receivers/results/errors, nonconstruction, forced getter/coercion
GC, input/result-prototype root failures, exact heap OOM and retry, String fuel,
installer rollback, and every allocation boundary. `Duration.from` remains
allocation 136, PlainTime add/subtract remain 137 and 138, and Duration.with is
139. The installer reserves 140 maximum live pins across 141 allocations.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
contains exactly 22 direct Duration.with files. Exact runner admission and
forced diagnostic execution are both **22/22**. Live metadata, disjoint
manifests, malformed/outside/future paths, configured-corpus failure, and
runner/analyzer sharing are frozen. The Duration.from complete diagnostic
remains **29 pass / 2 fail**: `argument-duration-max.js` now passes with and
fails only at absent `Duration.prototype.total`; the precision file remains
blocked by absent `toString`.

Local gates pass: all-target/all-feature Rust library **486/486**, builtins
**655/655**, every integration and Criterion smoke; warnings-denied Clippy;
rustfmt/diff; Rust 1.88 MSRV; wasm32; release Realm rollback; generated Intl
data; workflow YAML; Python tooling **210/210** with five expected
absent-corpus skips; exact/forced Duration.with **22/22**; forced Duration.from
**29/2**; and vendored RegExp **38/38** plus Clippy/no_std+alloc. An initial
full-test link attempt hit `ld` bus error with only 94 MB free; after mandatory
artifact cleanup, the identical test scope passed with test debug symbols
disabled. Two independent GPT-5.6 runtime/tooling audits found no remaining
defect.

Feature commit `d450bc1` (`feat(temporal): add Duration with`) is pushed to
`main`. Ordinary CI `31334805487` passes **3/3**. Full Test262 CI
`31334805481` passes **78/78**; dedicated job `93299323630` logs exact and
forced **22/22**.

Next recommended narrow unit: audit and implement
`Temporal.Duration.prototype.abs` and `negated` through one hidden-record sign
transform path if their direct Test262 surfaces confirm the shared contract.
Keep `total`, `toString`, balancing/rounding, calendar-relative arithmetic, and
locale formatting as separate units.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.Duration.prototype.abs and negated

Added Realm-local, nonconstructable length-0
`Temporal.Duration.prototype.abs` and `negated` through one
operation-parameterized hidden-record transform. Both brand and copy the
receiver once, ignore all arguments, avoid public accessors, constructor, and
species, and create a fresh intrinsic Duration in the native method Realm.
Abs maps all ten slots to absolute values; negated reverses every nonzero sign.
Every numeric zero is explicitly canonicalized to `+0`, preventing raw f64
negation from exposing `-0` through branded accessors on blank durations.

Runtime coverage freezes positive/negative/blank records, sparse zero fields,
fresh identity, signed-zero, hidden getter and argument non-observation,
receiver brands, nonconstruction, receiver subclasses, cross-Realm
receivers/results/errors, result-prototype root failure, exact heap OOM and GC
retry, installer rollback, and every allocation boundary. Normative install
order is `with`, `negated`, `abs`: existing allocations 136 through 139 remain
stable, negated is 140, abs is 141, maximum live pins are 142, and total
Temporal installer allocations are 143.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
contains exactly 9 direct abs files and 8 direct negated files. Method-specific
admission and combined forced diagnostic execution are exact **17/17** with no
blockers. Separate manifests preserve ownership while shared metadata,
runner/analyzer identity, malformed/outside/future rejection, configured
corpus setup failure, and ordinary/full workflow count gates freeze the
combined boundary.

Local gates pass: all-target/all-feature Rust library **489/489**, builtins
**657/657**, every integration and Criterion smoke; warnings-denied Clippy;
rustfmt/diff; Rust 1.88 MSRV; wasm32; release Realm rollback; generated Intl
data; workflow YAML; Python tooling **211/211** with five expected
absent-corpus skips; exact/forced unary Test262 **17/17**; and vendored RegExp
**38/38** plus Clippy/no_std+alloc. A first Python tooling run was interrupted
by concurrent review build artifacts filling the disk; agents were stopped,
all artifacts were deleted, and the identical 211-test suite then passed.
Final GPT runtime review found no defect. Tooling review clarified that path
helpers own manifest identity, setup owns configured-corpus availability, and
workflow total gates own complete-surface cardinality; docs now state those
separate responsibilities exactly.

Feature commit `e90ed40` (`feat(temporal): add Duration sign transforms`) is
pushed to `main`. Ordinary CI `31338280865` passes **3/3**. Full Test262 CI
`31338280840` passes **79/79**; dedicated job `93307998080` logs exact and
forced **17/17**.

Next recommended narrow unit: audit and implement
`Temporal.Duration.prototype.valueOf` as the always-throwing branded method,
including method-Realm TypeErrors, ignored arguments, nonconstruction, exact
admission, and zero-result-allocation/resource behavior. Keep `toString`,
`toJSON`, `total`, balancing/rounding, calendar-relative arithmetic, and locale
formatting separate until their broader shared contracts are audited.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.Duration.prototype.valueOf

Added Realm-local, nonconstructable length-0
`Temporal.Duration.prototype.valueOf`. The latest Temporal algorithm has no
receiver brand step: every call immediately throws a `TypeError` in the native
method Realm without inspecting `this`, arguments, public accessors,
constructor, or species. Valid, invalid, Proxy, local, and foreign receivers
therefore share the same first-step error, and implicit relational/numeric
coercion can no longer fall through to serialized Duration strings.

Runtime coverage freezes descriptor/name/length, nonconstruction, argument and
Proxy trap non-observation, valid/invalid and cross-Realm receiver errors,
relational coercion, exact error-only heap allocation, and installer rollback.
The previously established allocations 136 through 141 remain stable;
Duration.valueOf is allocation 142, `Temporal.Now` is 143, and `%Temporal%` is
144. The installer reserves 143 maximum live pins and every allocation
boundary through exact 144-object success is tested.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
contains exactly seven direct Duration valueOf files. Dedicated metadata,
runner/analyzer path identity, live-corpus/disjointness, absent/inaccessible
configured-corpus failure, malformed/outside/future path rejection, ordinary
CI, and a full-workflow job freeze exact **7/7**. The stale Duration core docs
were reconciled to current exact **77/0** and forced **77/1** boundaries.

Local gates pass: all-target/all-feature Rust library **491/491**, builtins
**658/658**, every integration and Criterion smoke; warnings-denied Clippy;
rustfmt/diff; Rust 1.88 MSRV; wasm32; release Realm rollback; generated Intl
data; workflow YAML; Python tooling **212/212** with five expected
absent-corpus skips; direct Test262 **7/7**; and vendored RegExp **38/38** plus
Clippy/no_std+alloc. Two independent GPT-5.6 final reviews are CLEAN.

Feature commit `97aaf19` (`feat(temporal): add Duration valueOf`) is pushed to
`main`. Ordinary CI `31341566767` passes **3/3**. Full Test262 CI
`31341566769` passes **80/80**; dedicated job `93316658620` logs
`RATE=100.0 PASS=7 FAIL=0 SKIP=0 TOTAL=7 RAN=7`.

Next recommended narrow unit: audit `Temporal.Duration.prototype.toString` as
the next foundational explicit serialization boundary, first measuring its
complete direct Test262 surface and option/rounding dependencies. Keep
`toJSON`, `total`, comparison, calendar-relative arithmetic, balancing, and
locale formatting separate unless the shared formatter contract proves a
small complete grouping.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.Duration.prototype.toString

Added Realm-local, nonconstructable, length-0
`Temporal.Duration.prototype.toString`. Receiver branding precedes options;
`fractionalSecondDigits`, `roundingMode`, and `smallestUnit` are rooted, read,
and coerced in normative order. Exact i128 nanosecond arithmetic implements all
nine signed rounding modes, balances only through the receiver's original
largest unit with a second floor, and carries complete days without balancing
into weeks/months/years. Rounded fields must remain exactly f64-representable
and pass the existing normalized Duration range check. The formatter emits an
Arc-backed ISO String directly, with auto trimming or fixed 0-9 precision and
no temporary/result GC object.

Runtime coverage freezes exact large subsecond formatting, positive/negative
rounding, day carry, blank fixed precision, invalid rounded boundaries,
branding/options/error precedence, method-Realm errors, public getter and
constructor non-observation, nonconstruction, option GC/root failure and fuel,
zero-result-object allocation, installer rollback, and every allocation
boundary. Existing valueOf remains allocation 142, toString is 143,
`Temporal.Now` is 144, and `%Temporal%` is 145. Maximum live pins are 144 and
exact complete installation requires 145 objects.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
contains exactly 44 direct Duration toString files. Dedicated metadata,
runner/analyzer exact identity, live-corpus/disjointness, absent/inaccessible
configured-corpus failure, malformed/outside/future rejection, ordinary CI,
and a full-workflow job freeze exact **44/44**. The downstream Duration.from
precision file moved into admission, making that complete surface exact
**30/0** and forced **30/1**; only the `total`-dependent maximum file remains.

Local gates pass: all-target/all-feature Rust library **495/495**, builtins
**660/660**, every integration and Criterion smoke; final dedicated Duration
fuel test and targeted Clippy; warnings-denied full Clippy; rustfmt/diff; Rust
1.88 MSRV; wasm32; release Realm rollback; generated Intl data; workflow YAML;
Python tooling **213/213** with five expected absent-corpus skips; direct
Test262 **44/44** and forced Duration.from **30/1**; vendored RegExp **38/38**
plus Clippy/no_std+alloc. Two independent GPT-5.6 final reviews are CLEAN.

Feature commit `fd04a6a` (`feat(temporal): add Duration toString`) is pushed to
`main`. Ordinary CI `31345547844` passes **3/3**. Full Test262 CI
`31345547848` passes **81/81**; dedicated job `93327137285` logs
`RATE=100.0 PASS=44 FAIL=0 SKIP=0 TOTAL=44 RAN=44`.

Next recommended narrow unit: audit and implement
`Temporal.Duration.prototype.toJSON`, reusing only the exact hidden-record ISO
formatter while preserving its distinct no-options, ignored-arguments,
branding, Realm, and resource contracts. Keep `total`, comparison,
calendar-relative arithmetic, locale formatting, and other balancing APIs
separate unless their complete direct surfaces prove a small shared unit.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.Duration.prototype.toJSON

Added Realm-local, nonconstructable, length-0
`Temporal.Duration.prototype.toJSON`. It brands the receiver, ignores every
argument, copies the hidden ten-field Duration record, converts fields exactly
to i128, and calls the shared ISO formatter with auto precision. It never
observes public getters or an overridden `toString`, and returns an Arc-backed
String primitive without allocating a result object. Prototype order is
`toString`, `toJSON`, `valueOf`.

Runtime coverage freezes exact maximum/subsecond formatting, JSON.stringify,
ignored Proxy/key arguments, hidden-getter and overridden-method
non-observation, cross-Realm branding errors, name/length/nonconstruction,
allocation-144 rollback, exhaustive installer boundaries, and no result-object
or method-root allocation. Allocation numbering is valueOf 142, toString 143,
toJSON 144, Temporal.Now 145, and `%Temporal%` 146; maximum live pins are 145
and exact complete installation requires 146 objects.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
contains exactly 12 direct Duration toJSON files. Dedicated metadata,
runner/analyzer identity, live-corpus/disjointness, absent/inaccessible corpus
failure, and malformed/outside/future rejection freeze exact **12/12**. No
downstream admission moved; Duration.from remains forced **30/1** and Duration
core remains forced **77/1**.

Local gates pass: all-target/all-feature Rust library **497/497**, builtins
**662/662**, every integration and Criterion smoke; warnings-denied Clippy;
rustfmt/diff; Rust 1.88 MSRV; wasm32; release Realm rollback; generated Intl
data; workflow YAML; Python tooling **214/214** with five expected
absent-corpus skips; direct Test262 **12/12**; vendored RegExp **38/38** plus
Clippy/no_std+alloc. Two independent GPT-5.6 final reviews are CLEAN.

Feature commit `33aa392` (`feat(temporal): add Duration toJSON`) is pushed to
`main`. Ordinary CI `31348692573` passes **3/3**. Full Test262 CI
`31348692626` passes **82/82**; dedicated job `93335933402` logs
`RATE=100.0 PASS=12 FAIL=0 SKIP=0 TOTAL=12 RAN=12`.

Next recommended narrow unit: audit `Temporal.Duration.prototype.total`; it
unlocks the sole remaining `Duration.from/argument-duration-max.js` blocker.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.Duration.prototype.total fixed-unit boundary

Added Realm-local, nonconstructable, length-1
`Temporal.Duration.prototype.total`. It brands first, handles String shorthand
without touching `Object.prototype`, and observes object options in
`relativeTo`-then-`unit` order while preserving roots across re-entry. The
independent no-relative day-through-nanosecond branch uses checked i128
nanoseconds and one exact `Ratio<BigInt>`-to-f64 conversion. Calendar units,
years/months/weeks, and every defined `relativeTo` remain explicit errors
instead of producing incomplete calendar or DST answers.

Runtime coverage freezes exact maximum/subsecond/repeating-rational results,
option order and abrupt identity, hidden-getter/prototype-pollution
non-observation, cross-Realm branding, unit-string fuel, GC roots, no Number
result allocation, installer rollback, and every allocation boundary.
Allocation numbering is valueOf 142, toString 143, toJSON 144, total 145,
Temporal.Now 146, and `%Temporal%` 147; maximum live pins are 146 and exact
complete installation requires 147 objects.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
contains 78 direct total files. Complete-directory admission accounting is
**28 pass / 0 fail / 50 skip**. Forced execution is **43 pass / 35 fail / 0
skip**; 15 matching-earlier-error passes are frozen separately as unsupported
false positives, leaving 35 real calendar/relative/zoned blockers. Downstream
Duration.from moves to exact/forced **31/0**, and Duration hidden-slot core to
exact/forced **78/0**.

Local gates pass: all-target/all-feature Rust library **499/499**, builtins
**665/665**, every integration and Criterion smoke; warnings-denied full
Clippy; rustfmt/diff; Rust 1.88 MSRV; wasm32; release Realm rollback; generated
Intl data; workflow YAML; Python tooling **215/215** with five expected
absent-corpus skips; direct total exact/forced boundaries; Duration.from
**31/31**; Duration core **78/78**; vendored RegExp **38/38** plus
Clippy/no_std+alloc. Three independent GPT-5.6 reviews found no implementation
or tooling issue; their two LOW stale-documentation findings were corrected
before commit.

Feature commit `cde5b30` (`feat(temporal): add fixed-unit Duration total`) is
pushed to `main`. Ordinary CI `31353106222` passes **3/3**. Full Test262 CI
`31353106207` passes **83/83**. Dedicated job `93348208374` logs admission
`RATE=100.0 PASS=28 FAIL=0 SKIP=0 TOTAL=28 RAN=28` and complete forced
diagnostic `RATE=55.1 PASS=43 FAIL=35 SKIP=0 TOTAL=78 RAN=78`. The admission
job runs the 28-path manifest directly; the 50 skips belong to the conceptual
78-file complete-directory admission boundary above.

Next recommended narrow unit: audit the 35 real total blockers and isolate the
smallest specification-complete `relativeTo` cohort. Prefer an ISO PlainDate
or PlainDateTime calendar branch only if its direct and downstream surfaces
can be completed without stubbing ZonedDateTime/IANA transition semantics.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal.Duration.prototype.total relativeTo boundary

Extended the Realm-local `Temporal.Duration.prototype.total` implementation
through the complete deterministic `relativeTo` boundary. Branded PlainDate,
PlainDateTime, and ZonedDateTime values use copied hidden records; property bags
preserve specification field order and roots across re-entry; strings support
plain and UTC/fixed-offset zoned forms. Named-zone annotations remain explicit
errors until a transition provider exists, avoiding fabricated DST answers.

ISO constrained year/month addition, signed fractional nudging, and
week/day/time totals use exact `Ratio<BigInt>` arithmetic before the final f64
conversion. Target date-time and epoch ranges are checked. Zero duration,
offset/leap-second/extreme-range behavior, hidden getter non-observation, fuel,
GC roots, and no-result-allocation behavior are covered by runtime tests.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
contains 78 direct total files. Exact admission is **77 pass / 0 fail / 0
skip**; forced execution is **77 pass / 1 fail / 0 skip**. The sole blocker,
`calendar-temporal-object.js`, fails before `total` because RuJa does not yet
provide real `Temporal.PlainMonthDay` and `Temporal.PlainYearMonth` objects.
There are no unsupported false positives in this partition.

Local gates pass: all-target/all-feature Rust library **499/499**, builtins
**666/666**, every integration and Criterion smoke; warnings-denied full
Clippy; rustfmt/diff; Rust 1.88 MSRV; wasm32; release Realm rollback; generated
Intl aliases/data; workflow YAML; Python tooling **215/215** with five expected
absent-corpus skips; direct total exact **77/77** and forced **77/78**; vendored
RegExp **38/38** plus Clippy/no_std+alloc. Independent implementation review is
CLEAN; tooling/documentation findings were corrected before commit.

Feature commit `2cd86ac` (`feat(temporal): add relative Duration total`) is
pushed to `main`. Ordinary CI `31358050712` passes **3/3**. Full Test262 CI
`31358050728` passes **83/83**. Dedicated job `93362202074` logs
`RATE=100.0 PASS=77 FAIL=0 SKIP=0 TOTAL=77 RAN=77` and forced diagnostic
`RATE=98.7 PASS=77 FAIL=1 SKIP=0 TOTAL=78 RAN=78`.

Next recommended narrow unit: implement specification-backed hidden-slot core
boundaries for `Temporal.PlainMonthDay` and `Temporal.PlainYearMonth`. This
unlocks the final direct Duration.total fixture without weakening branded
calendar-object semantics.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal calendar sibling hidden-slot cores

Added distinct Realm-local `Temporal.PlainMonthDay` and
`Temporal.PlainYearMonth` constructors, prototypes, hidden ISO reference-date
records, branded core getters, `valueOf`, and `@@toStringTag`. Constructor
conversion/range ordering, deferred `newTarget.prototype`, subclass and
foreign-Realm fallback, GC rooting, Realm rollback, and the complete
166-allocation Temporal installer are covered. Central calendar extraction now
recognizes all five calendar-bearing Temporal hidden kinds without observing
public properties.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
direct core admission is **89/89**. Forced candidate accounting is **89 pass /
15 fail / 104 total**; blockers require separate sibling factories or
serialization. Eleven downstream calendar-object fixtures entered their owner
manifests: PlainDate `from/compare/equals` are **71/42/40**,
PlainDateTime `from/compare/equals` are **70/42/41**, ZonedDateTime
fixed-offset/compare/equals/withCalendar are **261/50/55/16**, and Duration
total is complete **78/78**. PlainTime.with remains **21/1** because it calls
unimplemented sibling `from` factories.

Two GPT-5.6 final reviews found no remaining runtime defect after corrections.
Review findings fixed before commit: complete 104-file
features/includes/flags/negative metadata freeze, fail-closed configured
Test262 corpus access, current documentation boundaries, direct conversion and
endpoint coverage, fresh `newTarget.prototype` survival across allocation GC,
and default-stack final installer rollback coverage.

Local gates pass: all-target/all-feature Rust library **502/502**, builtins
**669/669**, every integration and Criterion smoke; warnings-denied full
Clippy; rustfmt/diff; wasm32; generated Intl aliases/data; workflow YAML;
Python tooling **216/216**; exact core **89/89**, forced **89/104**, Duration
total **78/78**; vendored RegExp **38/38** plus Clippy/no_std+alloc.

Feature commit `92c546c` (`feat(temporal): add calendar sibling cores`) is
pushed to `main`. Ordinary CI `31366055543` passes **3/3**. Full Test262 CI
`31366055544` passes **84/84**. Dedicated job `93385289515` passes literal
admission `RATE=100.0 PASS=89 FAIL=0 SKIP=0 TOTAL=89 RAN=89` and forced
diagnostic `RATE=85.6 PASS=89 FAIL=15 SKIP=0 TOTAL=104 RAN=104`. Ordinary CI
also confirms `RATE=100.0 PASS=78 FAIL=0 SKIP=0 TOTAL=78 RAN=78` for Duration
total.

Next recommended narrow unit: implement the real static `from` factories for
PlainMonthDay and PlainYearMonth, including dedicated partial-date string and
property-bag conversion, only when each complete observable-order boundary can
be admitted without serialization or arithmetic stubs.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal partial-date static factories

Added real Realm-local `Temporal.PlainMonthDay.from` and
`Temporal.PlainYearMonth.from`. Branded inputs clone hidden reference
components and calendars without public access; ISO property bags preserve
specified field/options order; dedicated `MM-DD` and `YYYY-MM` parsers share
the audited full date/time grammar. MonthDay uses arbitrary-size input years
only for leap/overflow validation and stores reference year 1972. YearMonth
stores reference day 1 and rejects non-positive months during conversion.
Non-leap `M00` is rejected syntactically. Receiver/subclass identity is ignored
in favor of the method Realm. Result allocation is rooted across GC/OOM retry,
and long strings plus property coercion are fuel bounded.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
factory admission is exact **74/74**. Forced complete accounting is **74 pass /
49 fail / 123 total**; all 49 blockers require the separate real
PlainMonthDay/PlainYearMonth `prototype.toString` formatter surface. No stub
was added. Four factory-dependent accessor/valueOf fixtures move the sibling
core to exact **93/93**, forced **93/104**. PlainTime combined admission is now
**336/336**, and its complete `prototype.with` directory is **22/22**.

Two GPT-5.6 final reviews found and resolved before commit: non-leap `M00`
syntax, YearMonth non-positive month error timing, four stale core blockers,
diagnostic relative/duplicate/outside-path fail-closed handling, and stale
features documentation. Runtime reviewer found no remaining Realm, GC/OOM,
fuel, parser, or installer defect after those corrections.

Local gates pass: lib **504/504**, builtins **672/672**, focused modules
**35/35** after an initial disk-full environment failure, warnings-denied full
Clippy, rustfmt/diff, workflow YAML/Python compile, exact factory **74/74**,
forced factory **74/123**, exact core **93/93**, forced core **93/104**, and
focused tooling metadata/fail-closed tests. CI supplies the complete clean
all-target/all-feature, vendored RegExp, no_std, wasm32, MSRV, Python tooling,
release rollback, and full Test262 evidence.

Feature commit `a169a77` (`feat(temporal): add partial-date factories`) is
pushed to `main`. Ordinary CI `31372944261` passes **3/3**. Full Test262 CI
`31372944275` passes **84/84**. Dedicated job `93407269469` logs exact core
`RATE=100.0 PASS=93 FAIL=0 SKIP=0 TOTAL=93 RAN=93`, forced core
`RATE=89.4 PASS=93 FAIL=11 SKIP=0 TOTAL=104 RAN=104`, exact factories
`RATE=100.0 PASS=74 FAIL=0 SKIP=0 TOTAL=74 RAN=74`, and forced factories
`RATE=60.2 PASS=74 FAIL=49 SKIP=0 TOTAL=123 RAN=123`. Ordinary CI also logs
PlainTime combined `336/336`, complete with `22/22`, and supported subset
`1636 pass / 0 fail / 677 skip`.

Next recommended narrow unit: implement complete, option-aware
`PlainMonthDay.prototype.toString` and `PlainYearMonth.prototype.toString`
formatters together only if their 49 factory helper blockers and direct
formatter directories can be admitted without faking calendar annotation or
reference-component semantics.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Current unit - Temporal partial-date formatters

Added Realm-local, non-constructable, length-0
`Temporal.PlainMonthDay.prototype.toString` and
`Temporal.PlainYearMonth.prototype.toString`. Both brand before options, root
the options object across the sole `calendarName` getter/string coercion,
preserve abrupt identity and method-Realm native errors, and format copied
hidden reference components without public accessors or temporary/result heap
objects. ISO `auto`/`never` use the short partial date; `always`/`critical`
include the complete reference date and annotation. Non-ISO hidden records
retain reference year/day even when `never` suppresses annotation. Extended
years use the shared audited formatter.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
direct formatter admission is exact **33/33**. All former formatter-dependent
core and factory blockers moved into admission, making those complete exact and
forced boundaries **104/104** and **123/123**. A deterministic 128-file
formatter dependency diagnostic reports **93 pass / 35 fail**; the 35 failures
belong to independently unimplemented MonthDay/YearMonth `with` and YearMonth
`add`/`subtract`. PlainTime remains complete **336/336**, including `with`
**22/22**. The ordinary language subset remains **1636 pass / 0 fail / 677
skip** because its sparse directories do not include these Temporal cohorts.

Two GPT-5.6 final reviews were used. Tooling/docs/CI review found no defect.
Runtime review found one future non-ISO `calendarName: "never"` reference
component bug; it was fixed and covered before publication. The first CI run
then exposed two tooling tests that did not tolerate an inaccessible default
`/root/test262` when no corpus was explicitly configured. Follow-up commit
adds the existing OSError/no-config fallback while preserving fail-closed
behavior whenever `TEST262` is set.

Local gates pass: all-target/all-feature Rust lib **509/509**, builtins
**674/674**, fuel **65/65**, every integration and Criterion smoke;
warnings-denied full Clippy; rustfmt/diff; generated Intl data; workflow YAML;
Python tooling **219 tests / 5 skips**; focused live metadata/fail-closed
tooling; vendored RegExp **38/38** plus Clippy/no_std; release build; exact
Test262 **104/104**, **123/123**, **33/33**, and diagnostic **93/128** with the
frozen 35 blockers.

Feature commit `54c7e58` (`feat(temporal): add partial-date formatters`) and CI
follow-up `a6f3165` (`fix(test262): tolerate absent default corpus`) are pushed
to `main`. Final ordinary CI `31382376709` passes **3/3**. Final full Test262
CI `31382376679` passes **84/84**. Ordinary Test262 job `93435187571` logs
literal core `104/104`, factories `123/123`, direct formatter `33/33`, and
dependency diagnostic `93 pass / 35 fail / 128 total`.

Next recommended narrow unit: implement complete hidden-slot
`PlainMonthDay.prototype.with` and `PlainYearMonth.prototype.with` together if
their shared field merge, overflow, observable order, Realm/resource, and 13
frozen formatter-downstream blockers can be admitted without arithmetic stubs.
After that, implement YearMonth `add`/`subtract` as a separate exact calendar
arithmetic unit for the remaining 22 formatter-downstream blockers.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal partial-date with

Added complete Realm-local `Temporal.PlainMonthDay.prototype.with` and
`Temporal.PlainYearMonth.prototype.with`. Both brand before observing inputs,
reject date/time Temporal brands plus `calendar`/`timeZone`, preserve exact
field/options order, merge hidden ISO records, canonicalize MonthDay reference
year 1972 and YearMonth reference day 1, and allocate through the method Realm.
MonthDay overflow always defaults to 1972 rather than a noncanonical receiver
reference year. GC/OOM/fuel and installer rollback are covered at **172
allocations / 171 maximum pins**.

Pinned direct Test262 is exact **43 pass / 0 fail / 0 skip**: MonthDay 21 and
YearMonth 22. The frozen 128-file formatter surface moved from **93/35** to
**106/22** without changing its denominator; the remaining 22 were exactly
YearMonth `add`/`subtract`. Local gates passed rustfmt, warnings-denied full
Clippy, lib **510/510**, builtins **677/677**, fuel **66/66**, installer
**39/39**, focused tooling, direct **43/43**, formatter **106/22**, workflow
YAML, and diff checks. Two GPT-5.6 final reviews found no remaining defect.

Feature commit `630a624` (`feat(temporal): add partial-date with methods`) and
CI follow-up `25b4a5c` (`fix(ci): run partial-date with matrix`) are pushed to
`main`. Final ordinary CI `31389260614` passes **3/3**. Final full Test262 CI
`31389260969` passes **84/84**.

Next narrow unit: complete `Temporal.PlainYearMonth.prototype.add` and
`subtract`, audit their full pinned direct directories rather than only the 22
formatter-dependent files, and keep any independent downstream dependency as
an explicit blocker.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainYearMonth arithmetic

Added Realm-local, non-constructable, length-1
`Temporal.PlainYearMonth.prototype.add` and `subtract`. Both brand first,
jointly root duration-like/options across every observable conversion, reuse
complete Duration validation, negate after conversion for subtract, validate
overflow before rejecting nonzero lower units, and perform checked Euclidean
ISO month arithmetic from canonical day 1. Results always store reference day
1 and use the method Realm prototype. Non-ISO hidden records fail closed until
calendar arithmetic exists. Installation is exact **174 allocations / 173
maximum pins** with exhaustive rollback coverage.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
has 73 direct files: add 36 and subtract 37. Exact admission is **71/71**;
forced complete accounting is **71 pass / 2 fail / 73**, with both failures
owned only by missing `PlainYearMonth.prototype.equals` after arithmetic
results succeed. All former 22 formatter blockers transitioned to pass, making
the frozen formatter surface exact **128/128**. All 73 paths have frozen live
metadata; runner/analyzer parity, admission/blocker disjointness, future paths,
corpus errors, duplicate arguments, and result drift fail closed. Ordinary and
full CI both run the live tooling and literal result gates.

Local gates pass: warnings-denied all-target/all-feature Clippy; rustfmt,
diff, YAML and Python compile; base lib **512/512**, builtins **679/679**, fuel
**67/67**; all-target/all-feature Rust including feature-only lib **515/515**,
all integrations and Criterion smoke; Python tooling **221 tests / 5 skips**;
live focused tooling; direct **71/71**, forced **71/2/73**, formatter
**128/128**. Two GPT-5.6 runtime/tooling reviews were used. Runtime found no
defect. Tooling found missing blocker metadata and ordinary-CI live validation;
both were fixed and revalidated before commit.

Feature commit `159e682` (`feat(temporal): add year-month arithmetic`) is
pushed to `main`. Ordinary CI `31393521659` passes **3/3**. Full Test262 CI
`31393521662` passes **84/84**.

Next narrow unit: implement complete real
`Temporal.PlainYearMonth.prototype.equals` first, moving the two arithmetic
blockers into admission without a stub; audit its full direct directory and
shared partial-date comparison dependencies before selecting compare/JSON.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainYearMonth equals

Added Realm-local, non-constructable, length-1
`Temporal.PlainYearMonth.prototype.equals`. It brands the receiver before
argument observation, reuses complete ToTemporalYearMonth conversion, compares
the full hidden ISO year/month/reference-day record plus canonical calendar
identifier, and returns an allocation-free Boolean. Branded arguments preserve
arbitrary reference day without public getters; property bags and strings
canonicalize reference day to 1. Method-Realm errors, observable GC, root
preflight and post-pin abrupt cleanup/retry, long-string fuel, and installer
rollback are covered. Installation is exact **175 allocations / 174 maximum
pins**; equals is allocation 173 and the namespace objects are 174/175.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
equals directory is exact **40 pass / 0 fail / 40** with frozen live metadata,
runner/analyzer parity, disjointness, future/outside/corpus failure gating,
exact argument identity, and result-drift diagnostics. The two former
arithmetic blockers moved into admission, making add/subtract exact **73 pass /
0 fail / 73**. Ordinary and full workflows enforce both literal boundaries.

Local gates pass: all-target/all-feature Rust including Criterion smoke; base
lib **517/517**, builtins **682/682**, fuel **68/68**; warnings-denied
all-target/all-feature Clippy; rustfmt, diff, workflow YAML and Python compile;
Python tooling **222 tests / 5 skips**; pinned equals **40/40** and arithmetic
**73/73**. Two GPT-5.6 final reviews were used. Tooling review found no defect;
runtime review found the stale final installer boundary and missing post-pin
abrupt cleanup test, both fixed and revalidated.

Feature commit `941cf88` (`feat(temporal): add year-month equality`) is pushed
to `main`. Ordinary CI `31401061440` passes **3/3**. Full Test262 CI
`31401058723` passes **84/84**.

Next narrow unit: audit and implement complete
`Temporal.PlainYearMonth.compare` before JSON/date conversion, preserving full
hidden reference/calendar semantics and exact pinned admission.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainYearMonth compare

Added Realm-local, non-constructable, length-2 static
`Temporal.PlainYearMonth.compare`. It completely converts the first operand
before observing the second, then lexicographically compares hidden ISO
`(year, month, reference day)` and returns primitive Number `-1`, `+0`, or
`1`. Calendar identifiers are validated during conversion but intentionally
ignored by ordering. Branded operands bypass public getters and allocate no
result object; method-Realm errors, first-abrupt short circuit, both observable
GC positions, root-preflight failure/retry, abrupt identity/retry, and each
String operand's exact fuel boundary are covered. Installation is exact **176
allocations / 175 maximum pins**; compare is allocation 174.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
built-ins compare directory is exact **39 pass / 0 fail / 39**. The complete
four-file Intl402 companion is frozen separately at **1 pass / 3 fail / 4**:
`future-calendar.js` passes while `compare-calendar.js`, `exhaustive.js`, and
`infinity-throws-rangeerror.js` remain explicit non-ISO Gregorian/era
conversion blockers. Live metadata, runner/analyzer parity, admission
disjointness, future/outside/corpus failure, exact argument identity, and
per-file result drift all fail closed. Ordinary and full workflows enforce
literal direct and Intl counts.

Local gates pass: all-target/all-feature Rust including Criterion smoke;
warnings-denied all-target/all-feature Clippy; rustfmt, diff, workflow YAML and
Python compile; focused compare Rust **6/6**; Temporal installer boundaries
**42/42**; focused Python tooling **2/2**; pinned direct **39/39** and Intl
**1/3/4**. Two GPT-5.6 final reviews found no runtime/spec or tooling/CI/docs
defects.

Feature commit `c1d0b17` (`feat(temporal): add year-month compare`) is pushed
to `main`. Ordinary CI `31408129430` passes **3/3**. Full Test262 CI
`31408127054` passes **84/84**.

Next narrow unit: audit and implement complete real
`Temporal.PlainYearMonth.prototype.toJSON` and its serialization dependencies
without a stub or false-positive TypeError admission; preserve hidden record,
calendar annotation, Realm, GC/root, fuel, exact pinned metadata, and explicit
downstream blocker accounting.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainYearMonth JSON

Added Realm-local, non-constructable, length-0
`Temporal.PlainYearMonth.prototype.toJSON`. It brands the receiver and sends
copied hidden year/month/reference-day/calendar slots directly through the
shared formatter with `calendarName = auto`. ISO records omit the hidden day
and annotation; non-ISO formatter behavior retains both. Arguments, public
getters, Proxy argument traps, and overridden `toString` are never observed.
Cross-Realm branded receivers work in both directions, incompatible-receiver
`TypeError`s use the method Realm, and the primitive String result allocates no
VM GC object. The new function is allocation 175 after compare, preserving all
existing method ordinals; installation is exact **177 allocations / 176
maximum pins**.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
has an exact eight-file direct directory and no downstream built-ins or
Intl402 caller. Exact admission and forced execution are **8 pass / 0 fail /
8**. Live metadata, runner/analyzer parity, disjointness, future/outside/corpus
failure, duplicate/missing arguments, and per-file result drift fail closed.
Ordinary and full workflows enforce the literal `8/0/8` boundary.

Local gates pass: all-target/all-feature Rust including Criterion smoke;
warnings-denied all-target/all-feature Clippy; rustfmt, diff, workflow YAML and
Python compile; focused runtime **3/3**; Temporal installer boundaries
**43/43**; full Python tooling **225 tests / 5 skips**; live focused tooling
**1/1**; pinned direct **8/8**. Two GPT-5.6 final reviews found no runtime/spec
or tooling/CI/docs defects.

Feature commit `7a81f5c` (`feat(temporal): add year-month JSON`) is pushed to
`main`. Ordinary CI `31457943151` passes **3/3**. Full Test262 CI
`31457943149` passes **84/84**.

Next narrow unit: audit and implement complete real
`Temporal.PlainYearMonth.prototype.toPlainDate`, including exact direct and
downstream caller accounting, hidden calendar/reference semantics, field
observation order, Realm, GC/root, fuel, allocation rollback, and non-ISO
blockers without stubs or false-positive errors.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainYearMonth toPlainDate

Added Realm-local, non-constructable, length-1
`Temporal.PlainYearMonth.prototype.toPlainDate`. It brands before observing
the argument, requires an Object, reads only `day`, uses truncating Temporal
integer conversion, applies default constrain against the hidden ISO
year/month, validates exact PlainDate limits, and creates the result with the
method Realm intrinsic prototype. Receiver accessors and unrelated input
fields are ignored. Input rooting survives getter/valueOf GC; root-preflight,
heap-cap retry, fuel, and abrupt paths remain balanced. Non-ISO hidden records
fail closed until a calendar backend exists. The function is allocation 176;
installation is exact **178 allocations / 177 maximum pins**.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
has an exact 12-file direct directory. Admission and forced execution are
**12 pass / 0 fail / 12**. A balanced live scan classifies all 1,432
`TemporalHelpers.assertPlainYearMonth` calls: 501 short/default, 39 explicit
non-null, and 892 literal-null calls across exactly 87 Intl402 files. Those
true downstream callers remain **0 pass / 87 fail / 87**, and every
strict/sloppy variant is pinned to the earlier
`RangeError: Invalid Temporal calendar identifier` blocker. They are not
falsely admitted as toPlainDate support.

Local gates pass: all-target/all-feature Rust including Criterion smoke;
warnings-denied all-target/all-feature Clippy; rustfmt, diff, workflow YAML and
Python compile; focused runtime **5/5**; Temporal installer boundaries
**44/44**; full Python tooling **227 tests / 5 skips**; live focused tooling
**2/2**; pinned direct **12/12** and downstream **0/87** with exact failure
reasons. Two GPT-5.6 final reviews found no runtime/spec or tooling/CI/docs
defects.

Feature commit `4409c7f` (`feat(temporal): add year-month date bridge`) is
pushed to `main`. Ordinary CI `31462907913` passes **3/3**. Full Test262 CI
`31462907919` passes **84/84**.

Next narrow unit: audit the next missing Temporal partial-date method,
starting with complete real `Temporal.PlainMonthDay.prototype.toJSON` and its
direct/downstream dependencies; preserve hidden reference/calendar semantics,
Realm, GC/root, fuel, exact pinned metadata, and non-ISO blocker accounting
without stubs or false-positive errors.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainMonthDay JSON

Added Realm-local, non-constructable, length-0
`Temporal.PlainMonthDay.prototype.toJSON`. It brands the receiver and sends
copied hidden reference-year/month/day/calendar slots directly through the
shared formatter with `calendarName = auto`. ISO records omit hidden year and
annotation; non-ISO records retain both. Arguments, public getters, Proxy
wrappers, and overridden `toString` are never observed. Cross-Realm and
subclass branded receivers work, incompatible-receiver `TypeError`s use the
method Realm, and the primitive String result allocates no VM GC object.
Existing ordinal 1..176 remain stable; the function is allocation 177 and
installation is exact **179 allocations / 178 maximum pins**.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
has an exact seven-file built-ins directory. Admission and forced execution
are **7 pass / 0 fail / 7**. The two-file Intl402 companion remains **0 pass /
2 fail / 2** and every strict/sloppy variant is pinned to
`RangeError: Invalid Temporal calendar identifier` from earlier `gregory`
construction. A live 13-file candidate audit freezes the direct 7, Intl 2,
and four outside textual candidates; the latter serialize options/result
arrays or use toString, so true downstream callers are exactly zero.

Local gates pass: all-target/all-feature Rust including Criterion smoke;
warnings-denied all-target/all-feature Clippy; rustfmt, diff, workflow YAML and
Python compile; focused runtime **3/3**; Temporal installer boundaries
**45/45**; full Python tooling **229 tests / 5 skips**; live focused tooling
**2/2**; pinned direct **7/7** and Intl **0/2** with exact failure reasons.
Two GPT-5.6 final reviews found no runtime/spec or tooling/CI/docs defects.

Feature commit `5de29b8` (`feat(temporal): add month-day JSON`) is pushed to
`main`. Ordinary CI `31467996597` passes **3/3**. Full Test262 CI
`31467996586` passes **84/84**.

Next narrow unit: audit and implement complete real
`Temporal.PlainMonthDay.prototype.toPlainDate`, including year-only input
observation, hidden month/day/reference semantics, constrain/limits, Realm,
GC/root/fuel, exact direct/downstream metadata, and explicit non-ISO blockers
without stubs or false-positive errors.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainMonthDay toPlainDate

Added Realm-local, non-constructable, length-1
`Temporal.PlainMonthDay.prototype.toPlainDate`. It brands before argument
observation, requires an Object, reads only `year`, performs truncating
Temporal integer conversion, and constrains the receiver's hidden ISO
month/day against that year. Exact PlainDate limits are validated and the
result uses the method Realm intrinsic prototype. Receiver accessors and input
`calendar`/`day`/`month`/`monthCode` remain unobserved. Input rooting survives
getter/valueOf GC; root-preflight, exact heap-cap retry, long-String fuel, and
installer rollback remain balanced. Existing ordinals 1..177 remain stable;
the method is allocation 178 and complete installation is exact **180
allocations / 179 maximum pins**. Non-ISO hidden records fail closed until a
calendar backend exists.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
has an exact 12-file built-ins directory. Admission and forced execution are
**12 pass / 0 fail / 12**. The sole Intl402 companion remains **0 pass / 1
fail / 1**, pinned to the earlier exact `RangeError: Invalid Temporal calendar
identifier` from `gregory` construction. A complete pinned `test/` exact-word
scan plus an extracted `TemporalHelpers.assertPlainMonthDay` body audit proves
true downstream callers are zero. Live metadata, runner/analyzer parity,
manifest disjointness, inaccessible corpus, future/outside paths, duplicate
arguments, result drift, and normalized exact error-reason drift fail closed.

Local gates pass: all-target/all-feature Rust including Criterion smoke;
warnings-denied all-target/all-feature Clippy; rustfmt, diff, workflow YAML and
Python compile; focused runtime **3/3**, allocation/fuel tests, full live
Python tooling **231/231**, corpus-unavailable tooling **231 tests / 5 skips**,
pinned direct **12/12**, and Intl **0/1** with exact failure reason. Two
GPT-5.6 final reviews found no remaining runtime/spec or tooling/CI/docs
defects after the complete-corpus/helper and exact-error checks were tightened.

Feature commit `35f428a` (`feat(temporal): add month-day date bridge`) is
pushed to `main`. Ordinary CI `31474458844` passes **3/3**. Full Test262 CI
`31474458819` passes **84/84**.

Next narrow unit: audit and implement complete real
`Temporal.PlainMonthDay.prototype.equals`, including hidden reference year,
month/day/calendar identity, complete input conversion and observation order,
Realm, GC/root/fuel, exact direct/Intl/downstream metadata, and non-ISO
blockers without stubs or false-positive errors.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainMonthDay equals

Added Realm-local, non-constructable, length-1
`Temporal.PlainMonthDay.prototype.equals`. Receiver branding precedes argument
observation; the existing complete converter handles branded values, ordered
property bags, and audited Strings. Equality compares hidden reference ISO
year, month, day, and canonical calendar identity. Branded argument getters
remain unobserved, cross-Realm/subclass values work, method-Realm errors and
observable GC/root/abrupt/fuel boundaries are preserved, and the Boolean
result allocates no VM heap object. Existing ordinals 1..178 remain stable;
equals is allocation 179 and complete installation is exact **181 allocations
/ 180 maximum pins**.

String-valued `monthCode` and `offset` fields perform string-hint
`ToPrimitive` and require a String primitive. A first CI pass exposed two
over-broad conversion attempts: generic ToString broke strict primitive
offset TypeErrors, then monthCode ToString broke
`PlainYearMonth/from/month-code-wrong-type.js`. Final code restores the shared
strict field helper, accepts object `toString` only when it returns String,
and adds direct regressions for numeric/Boolean/null/BigInt offset and numeric
monthCode TypeErrors. Exact sibling factory **123/123** and Duration total
**78/78** prove both shared boundaries.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
has a complete 36-file built-ins directory at **36 pass / 0 fail / 36**. The
four-file Intl402 complement is **1 pass / 3 non-ISO blockers / 4**. Seven
Chinese/Dangi downstream files contain eight true calls and remain **0 pass /
7 exact earlier calendar blockers / 7**. A complete 47-file candidate audit,
path-specific call counts, and token-aware `temporalHelpers.js` owner audit
freeze all direct/downstream sites. The lexical audit excludes comments,
Strings, regexps, template raw text and computed method declarations; includes
template expressions plus direct, optional and static computed-string calls;
and bounds helper ownership by balanced method bodies. GPT-5.6 runtime and
tooling reviews report no remaining issues.

Local gates pass: all-target/all-feature Rust including Criterion smoke;
warnings-denied all-target/all-feature Clippy; rustfmt, diff, workflow YAML and
Python compile; full live Python tooling **234/234**; corpus-unavailable
tooling **234 tests / 5 skips**; exact sibling factory **123/123**, Duration
total **78/78**, direct equals **36/36**, Intl **1/3/4**, and downstream
**0/7/7**.

Feature commit `a1a1904` (`feat(temporal): add month-day equality`) and strict
field corrections `684964d`, `5ef6e5e` are pushed to `main`. Final ordinary
CI `31486346339` passes **3/3**. Final full Test262 CI `31486346330` passes
**84/84**.

Next narrow unit: use the final pinned full-matrix evidence to choose the next
complete real Temporal method cluster, preferring an ISO direct boundary with
exact Intl/downstream blocker accounting over broad non-ISO stubs.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainDate sibling conversions

Added Realm-local, non-constructable, length-0
`Temporal.PlainDate.prototype.toPlainMonthDay` and `toPlainYearMonth`.
Receiver branding and hidden-slot copy precede all work; arguments and public
getters are ignored. ISO results publish canonical reference year 1972 and
reference day 1, allocate fresh objects with the native method Realm
intrinsics, and remain independent of replaced mutable `Temporal` globals.
Cross-Realm branded receivers work; non-ISO hidden calendars fail closed until
real calendar field backends exist. Existing ordinals 1..181 remain stable;
the methods are allocations 182 and 183, complete installation is exact **183
allocations / 182 maximum pins**, and installer preflight now reserves all 182
pin slots.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
has exact direct surfaces **7/7** and **8/8**, combined **15 pass / 0 fail /
15**. Three true Intl402 callers remain **0 pass / 3 exact earlier blockers /
3**: Chinese/Dangi construction, absent DateTimeFormat construction, and
absent PlainDate.withCalendar. A complete test-tree candidate audit freezes
the staging textual false candidate and path-specific direct/optional/computed
call counts. A token-aware complete harness scan freezes zero calls. Metadata,
manifest disjointness, inaccessible corpus, future/outside paths, duplicate
arguments, result drift, and normalized exact error-reason drift fail closed.

Runtime tests cover canonical hidden records, fresh identity, replaced globals,
method-Realm prototypes/errors, cross-Realm receivers, result-root reservation
failure/retry, exact heap-cap GC retry, pin restoration, installer rollback,
and zero additional native fuel. Four GPT-5.6 audits found and closed the
installer root preflight, sparse staging checkout, harness audit, and missing
resource/freshness/intrinsic/fuel coverage; final runtime review reports no
remaining issue.

Local gates pass: all-target/all-feature release Rust including Criterion;
warnings-denied all-target/all-feature release Clippy; rustfmt, diff, workflow
YAML and Python compile; full live Python tooling **235/235**;
corpus-unavailable tooling **235 tests / 5 skips**; exact direct **15/15** and
downstream **0/3** with exact failure reasons.

Feature commit `70d016d` (`feat(temporal): add plain date sibling conversions`)
is pushed to `main`. Ordinary CI `31493519891` passes **3/3**. Full Test262 CI
`31493519952` passes **84/84**.

Next narrow unit: inspect the final full-matrix failure artifact and select the
next complete real Temporal or core-language cluster with exact direct,
Intl/downstream, Realm, GC/root/fuel/allocation, documentation, and CI
boundaries. Prefer a bounded ISO implementation over non-ISO stubs.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainDateTime serialization

Added Realm-local, non-constructable, length-0
`Temporal.PlainDateTime.prototype.toString` and `toJSON`. Both brand first and
copy hidden ISO/calendar slots. `toString` validates and roots options, reads
`calendarName`, `fractionalSecondDigits`, `roundingMode`, and `smallestUnit` in
specification order, rounds the complete local ISO nanosecond value with
midnight carry, rechecks the exclusive range, and formats the hidden record.
`toJSON` ignores arguments and public `toString`, using auto/trunc/auto direct
serialization. Both return primitive Strings without a VM heap-object
allocation. Existing ordinals 1..185 remain stable; the methods are
allocations 186 and 187, complete installation is exact **187 allocations /
184 maximum pins**.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
has **49/49** built-ins `toString` and **8/8** built-ins `toJSON`, combined
**57/57** admission. Complete direct/Intl accounting is **57 pass / 7 fail /
64**; all seven blockers fail earlier at non-ISO calendar construction with
the exact calendar `RangeError`. One downstream `until/balance.js` file owns
six formatter calls but remains **0/1** at earlier missing `until`/`add` with
the exact undefined-function `TypeError`. Five baseline incidental passes are
frozen as false positives. Token-aware candidate and ownership audits freeze
**94 files / 243 references**, all 205 selected references categorized, and
zero harness-owned calls. Metadata, directory, manifest, future/outside,
duplicate argument, result, and normalized error drift fail closed.

Local gates pass: all-target/all-feature release Rust including Criterion;
warnings-denied all-target/all-feature release Clippy; rustfmt, diff, workflow
YAML and Python compile; live Python tooling **238/238**; corpus-unavailable
tooling **238 tests / 5 expected skips**; exact direct **57/57**, complete
**57/7/64**, and downstream **0/1** diagnostics. Two GPT-5.6 final reviews
found and closed an options-root failpoint coverage gap and stale gate wording;
no runtime or tooling defect remains.

Feature commit `b6cc933` (`feat(temporal): format plain date-times`) is pushed
to `main`. Ordinary CI `31513706624` passes **3/3**. Full Test262 CI
`31513706604` passes **84/84**.

Next narrow unit: inspect the final full-matrix failure artifact and choose the
next complete real Temporal method cluster, preferring a bounded ISO direct
surface with exact Intl/downstream accounting over non-ISO stubs.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainDateTime date/time bridges

Added Realm-local, non-constructable, length-0
`Temporal.PlainDateTime.prototype.toPlainDate`, `toPlainTime`, and
`withPlainTime`. The projections brand first and copy hidden date/time/calendar
slots into fresh method-Realm PlainDate/PlainTime intrinsics without observing
arguments, public getters, constructors, species, subclasses, or mutable
globals. `withPlainTime` maps missing/undefined to midnight; otherwise it uses
the complete admitted PlainTime converter for branded PlainTime/PlainDateTime,
UTC/fixed-offset ZonedDateTime, ordered property bags, and audited Strings. It
preserves hidden date/calendar fields, replaces all six time fields, rechecks
the complete PlainDateTime range, and returns a fresh method-Realm intrinsic.
Named-IANA zones remain blocked on deterministic transition data. Existing
ordinals 1..187 stay stable; the methods are allocations 188..190, complete
installation is exact **190 allocations / 186 maximum pins**.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
has direct **8/8 + 7/7 + 37/37 = 52/52**. The absent-method
`withPlainTime/argument-number.js` pass is frozen as a preimplementation false
positive. Complete token-aware Git-tree ownership is **135 candidate files /
49 direct calls / 1 Intl call / 91 homonym calls** with no true downstream
caller. The historically misnamed
`intl402/Temporal/PlainDate/prototype/withPlainTime/basic-roc.js` owns a real
PlainDateTime receiver call and remains **0/1** at the exact earlier
`RangeError: Invalid Temporal calendar identifier`. Exact metadata, manifests,
receiver ownership, future/outside/duplicate/result/error drift, sparse corpus,
and runner/analyzer parity fail closed.

Runtime regressions cover method-Realm prototypes/errors, hidden getter
non-observation, installer rollback at allocations 188..190, every allocation
boundary, exact 190-object capacity, result-root failure, exact heap-cap GC
retry, observable property-bag GC, conversion-before-result root failure, zero
projection fuel, and exact eight-byte String fuel. Local gates pass
all-target/all-feature release Rust including Criterion, warnings-denied
release Clippy, rustfmt/diff, Python/YAML, live tooling **242/242**,
corpus-unavailable tooling **242 tests / 5 skips**, direct **52/52**, and Intl
**0/1** exact diagnostics. GPT-5.6 runtime review was clean; tooling/docs review
found and closed the Intl ownership omission and ZonedDateTime scope wording.

Feature commit `5b0feb2` (`feat(temporal): add date-time bridges`) is pushed to
`main`. Ordinary CI `31523769059` passes **3/3**. Full Test262 CI
`31523769102` passes **85/85** after rerunning two unrelated checkout jobs that
initially failed all fetch retries with runner CA certificate errors.

Next narrow unit: implement complete `Temporal.PlainDateTime.prototype.add`
and `subtract` together. The pinned direct surfaces are 42 + 42 files; the
shared work needs checked ISO date addition, time-to-date carry, overflow
handling, Realm/resource boundaries, and exact accounting for the remaining
non-ISO Intl blockers. Keep `round`, difference methods, named-zone transitions,
and non-ISO calendar backends separate.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainDateTime ISO arithmetic

Added Realm-local, length-1, non-constructable
`Temporal.PlainDateTime.prototype.add` and `subtract`. Both brand before
argument/options observation and use complete Duration conversion for branded
slots, ordered bags, and audited Strings. Subtraction exact-negates hidden
fields without public `negated`/`add`; shared arithmetic adds time first,
retains signed day carry, revalidates the adjusted date-duration sandbox
bounds, applies ISO year/month overflow followed by weeks/days/carry, checks
the exclusive PlainDateTime range, and creates a fresh method-Realm intrinsic.
Non-ISO calendars remain fail-closed. Existing ordinals 1..190 stay stable;
the methods are allocations 191 and 192, complete installation is exact **192
allocations / 186 maximum pins**.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
has direct **42/42 + 42/42 = 84/84**. Complete full-tree ownership is **84
pass / 159 exact blockers / 243**: 148 Intl direct and seven Intl downstream
files fail at non-ISO calendar construction; four built-ins downstream files
fail at earlier missing `since`/`until`. The token-aware full `test+harness`
archive freezes **249 candidates / 239 ownership rows**, computed-string calls,
metadata/results/errors, downstream identity, and eight preimplementation
false positives fail closed.

Runtime resource coverage includes installer rollback at 191/192, every
allocation boundary and exact 192 capacity, brand-first observation, hidden
getter non-observation, method-Realm errors/results, argument/result root
failures, observable getter GC, exact heap-cap retry, three-byte String fuel,
negative carry, overflow, and huge adjusted durations. Local release Rust,
warnings-denied Clippy, rustfmt/diff, Python/YAML, live tooling **246/246**,
corpus-unavailable tooling **246 tests / 5 skips**, direct **84/84**, and
complete **84/159/243** diagnostics pass. Two GPT-5.6 reviews closed all
runtime/tooling findings.

Feature commit `a104a4e` (`feat(temporal): add date-time arithmetic`) is pushed
to `main`. Ordinary CI `31533029040` passes **3/3**. Full Test262 CI
`31533029052` passes **86/86**.

Current unit in progress: complete
`Temporal.PlainDateTime.prototype.round`, then commit/push/CI as an independent
append-only allocation 193 unit. Direct pinned surface is 45 files with no
Intl direct or true downstream caller; full ownership includes eight
Duration/ZonedDateTime homonym files.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.

## Completed unit - Temporal PlainDateTime round

Added Realm-local, length-1, non-constructable
`Temporal.PlainDateTime.prototype.round`. It brands before argument
observation, accepts String shorthand or ordered `roundingIncrement`,
`roundingMode`, and `smallestUnit` options, supports day through nanosecond
units and all nine modes, validates increment divisibility only after all
options are converted, and creates a fresh method-Realm intrinsic. Shared time
rounding now returns signed midnight carry: PlainTime discards it while
PlainDateTime applies it to the ISO civil date and rechecks the exclusive
range. Rounding uses non-negative time within the day rather than signed epoch
distance, preserving BCE floor/trunc/ceil direction. Existing ordinals 1..192
stay stable; `round` is allocation 193, complete installation is exact **193
allocations / 186 maximum pins**.

Pinned Test262 `9e61c12835c5e4a3bdba93850427e6742c4f64c4`
has exact direct **45/45** and no Intl direct directory or true downstream
PlainDateTime caller. Full `test+harness` ownership freezes **54 candidates /
54 rows / 85 direct references / 73 direct calls / 59 Duration or
ZonedDateTime homonym references and calls / 11 generic harness computed
calls**. A second full executable-token identity contract covers all **1,341
PlainDateTime-bearing files**, so alias, destructuring, dynamic computed, and
same-count semantic drift fail closed. Exact per-file features/includes/flags/
negative metadata, directory identity, arguments, results, runner/analyzer
parity, sparse checkout, and future/outside paths are also frozen.

Runtime/resource coverage includes installer rollback at allocation 193,
every boundary and exact 193 capacity, brand-first observation, method-Realm
errors/results, hidden getter non-observation, all modes/BCE/midnight/range,
observable options GC, options-root preflight, exact heap-cap failure, GC
retry, and PlainTime carry-discard regression. Local gates pass
all-target/all-feature release Rust including Criterion, warnings-denied
release Clippy, rustfmt/diff, Python/YAML, direct **45/45**, live tooling
**249/249**, and corpus-unavailable tooling **249 tests / 5 skips**. GPT-5.6
runtime review was clean; tooling review found and closed alias/computed,
field-level metadata, and same-count token-identity gaps, then finished clean.

Feature commit `b221afd` (`feat(temporal): round plain date-times`) is pushed
to `main`. Ordinary CI `31539252307` passes **3/3**. Full Test262 CI
`31539252292` passes **87/87** after rerunning one unrelated PlainDate.from
checkout job whose initial runner failed on repeated CA certificate errors.
The dedicated PlainDateTime round job and exact **45/45** step pass.

Next narrow unit: implement complete
`Temporal.PlainDateTime.prototype.with`. Pinned built-ins direct surface is 30
files; 70 Intl callers remain non-ISO calendar blockers. Use a new optional
nine-field partial collector rather than the full PlainDateTime bag collector,
which defaults missing time fields to zero and would incorrectly erase
receiver fields. Keep `until`/`since`, named-zone conversion, locale formatting,
and non-ISO calendar backends separate.

Mandatory every turn: close all agents/sessions; run root and vendor
`cargo clean`; delete `/root/test262`, CI artifacts, binaries, logs, analyzer
dumps, Python/Cargo caches, and temporary files; prune worktrees; verify no
related process, clean git/origin parity, and free disk. Never delete or alter
the active goal.
