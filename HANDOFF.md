# RuJa Handoff - 2026-07-22

## Goal

Continue the active long-running goal of turning RuJa into a substantially
complete JavaScript engine. Close specification behavior rather than only
runner gaps. Every narrow unit includes focused and broad tests, documentation,
rustfmt, warnings-denied Clippy, exact Test262 evidence, a clean commit and
push, ordinary CI, full-matrix verification, and artifact aggregation.

The overall goal remains active. Do not mark it complete after one conformance
family.

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

## Next unit

Make lazy `for-in`'s per-key native collections fallible at their actual
observation boundaries: reserve `remaining_keys` before snapshot publication
and `visited_keys` before marking an existing descriptor. Preserve
OwnPropertyKeys/GetOwnProperty order, shadowing, duplicate suppression, retry
semantics, fuel, and terminal capacity release. Keep broader Proxy own-key
frames, PropertyKey/Error strings, trap-call internals, GC root enumeration,
and mark worklists as later independently audited layers.
