# Architecture

RuJa is a self-contained JavaScript engine with no external runtime
dependencies. Source flows through four stages before execution:

```
source ─► Lexer ─► Parser ─► Compiler ─► Bytecode ─► VM
                              │              │
                              └─ AST         └─ Op stream
```

## Pipeline

- **Lexer** (`src/lexer.rs`) — tokenization with automatic semicolon insertion
  (ASI) and template literal support.
- **Parser** (`src/parser.rs`) — Pratt-style recursive descent producing an AST.
  Expression nesting is depth-capped to prevent stack overflow on untrusted input.
- **Compiler** (`src/compiler.rs`) — single-pass AST → bytecode compilation
  with lexical scope resolution, hoisting, and TDZ tracking.
- **Bytecode** (`src/bytecode.rs`) — a stack-machine instruction set (`Op`).
- **VM** (`src/vm/mod.rs`, `src/vm/ops.rs`) — the dispatch loop: call frames,
  operand stack, property access, type coercion, and non-local control flow.
  `ops.rs` holds the opcode dispatch and immediate helpers; `mod.rs` holds
  the `Vm` struct, public API, and runtime helpers. Identifier, property,
  private-name, and `super` expression evaluation is represented by rooted
  `ReferenceRecord` values and resolved through shared `GetValue`, `PutValue`,
  call, and delete operations; compiler-internal switch completion reads use
  the same path.
- **GC** (`src/gc.rs`) — mark-and-sweep collector that traces from VM roots.
- **Values** (`src/value.rs`) — the `HeapObj` enum
  (Object/Array/Function/Environment/Map/Set/Promise/Generator) referenced by
  `GcIdx` handles.
- **Builtins** (`src/builtins/mod.rs` + submodules) — the standard library:
  Object, Array, String, Number, Boolean, Function, Math, JSON, console, RegExp,
  Map, Set, Symbol, Promise, Proxy, TypedArray, and the Error hierarchy.

## Compiler temporary storage

Compiler-generated carriers for destructuring sources, iterator state,
pre-evaluated assignment References, and switch completion values are dense
slots owned by `CallFrame`. `Chunk` maps only marked constant operands to these
slots; ordinary identifier operands continue to use Environment Records.
`DeclareEnv`, `LoadEnv`, `StoreEnv`, and `StoreEnvName` route marked operands
to frame storage, while iterator-close bytecodes read the same slots directly.

Frame ownership prevents nested defaults, host reentry, direct or indirect
eval, `with`, and temporary class environments from aliasing an outer
compiler value. It also prevents synthetic names and their last values from
accumulating in the global environment. Generator and async continuations save
and trace the slot vector across suspension. Completion drops it; completed
generators also release their activation environment, arguments, receiver,
resume value, saved control state, lexical closure, and native vector capacity.
Only the already-rooted Realm global remains as an inert terminal environment.
Return and throw payloads are pinned across result, Promise, and catch
materialization after that state is released.

```text
[Decision Log]
- 목적과 의도: 중첩 실행과 환경 전환에서도 compiler temporary의 identity와 수명을 실행 frame에 한정한다.
- 기존 구현 및 제약 조건: 고정 Environment binding 이름은 nested destructuring이 outer source, iterator, Reference를 덮어썼다. 실행 깊이 기반 고유 이름은 재진입 충돌을 줄여도 global binding과 마지막 값을 계속 보존하며 class/with 환경 변경에 의존한다.
- 검토한 주요 대안: 실행 깊이 namespace, scope 종료 시 environment binding 삭제, 전용 temporary environment, 또는 Chunk metadata와 CallFrame dense slot.
- 선택한 방식: fresh temporary constant를 Chunk의 dense slot에 표시하고 관련 opcode가 CallFrame storage를 사용한다. suspension record와 GC root walk가 같은 값을 보존하고 terminal generator 경로가 저장 실행 상태를 비운다.
- 다른 대안 대신 이 방식을 선택한 이유: 이름 namespace와 삭제 방식은 mutable environment 위치 및 예외 unwind를 추적해야 하고 사용자 Environment Record에 synthetic state를 남긴다. frame slot은 call/reentry/suspension이라는 실제 수명 경계와 일치한다.
- 장점, 단점 및 영향: nested destructuring, IteratorClose, pre-evaluated Reference, switch completion이 서로 격리되고 global binding/value 누수가 사라진다. 새 compiler temporary를 추가할 때는 반드시 Chunk marker를 사용하고 해당 operand를 소비하는 opcode가 frame-slot routing을 지원해야 한다.
```

## Function metadata properties

Function objects keep internal metadata for invocation, diagnostics, and source
rendering, but observable `name` and `length` values live only in ordinary own
property descriptors installed when each function is created. Because those
descriptors are configurable, deletion must not expose the internal values
again. A subsequent read therefore follows the normal prototype chain and a
subsequent definition creates a new own property through the shared property
machinery. The only function-specific virtual property fallback is the
constructor `prototype` slot.

```text
[Decision Log]
- 목적과 의도: 함수 내부 실행 metadata와 ECMAScript ordinary property semantics의 경계를 고정한다.
- 기존 구현 및 제약 조건: 모든 production 생성 경로가 name/length descriptor를 설치하지만 get fallback도 같은 값을 합성해 configurable 삭제를 무효화했다. 내부 name은 toString과 진단에서 필요하다.
- 검토한 주요 대안: 삭제 시 내부 metadata 변경, deleted-key tombstone, 또는 descriptor-only observation.
- 선택한 방식: name/length는 실제 own descriptor로만 관찰하고 FunctionData는 내부 용도로 유지한다. prototype slot만 기존 virtual fallback을 유지한다.
- 다른 대안 대신 이 방식을 선택한 이유: descriptor-only 방식이 기존 delete, inheritance, accessor receiver, Proxy, defineProperty 규칙을 그대로 재사용하며 내부 실행 상태를 변형하지 않는다.
- 장점, 단점 및 영향: 함수 종류와 Realm에 무관한 ordinary semantics를 얻는다. 새 함수 생성 경로는 반드시 표준 descriptor를 설치해야 하며 virtual metadata 추가 시 configurable 삭제 동작을 별도로 검토해야 한다.
```

## Function source text

Lexer tokens carry half-open byte ranges into the original source unit. The
parser uses those ranges before AST lowering to retain the exact syntactic text
for function declarations and expressions, arrows, methods, accessors, and
classes. Object and class methods exclude surrounding separators and a class
method's `static` prefix; a class constructor function retains the complete
`class ... }` production. Compiler-generated helpers have no source text and
therefore use NativeFunction syntax.

Parsed host source slices pass through the scalar-to-internal UTF-16 boundary,
while direct eval and other already-canonical internal source slices are copied
unchanged. This distinction prevents host scalars in RuJa's private surrogate
sentinel range from aliasing lone UTF-16 code units. Dynamic Function
constructors synthesize their specified `anonymous` source from the original
internal parameter and body strings with literal LF boundaries rather than
exposing the parser-only `_f` wrapper.

`Function.prototype.toString` returns retained source for interpreted
ECMAScript functions. Built-ins include their `[[InitialName]]` in a valid
NativeFunction production; bound, source-unavailable synthetic, and callable
Proxy objects use its nameless form. Non-callable receivers throw `TypeError`.
Test262 source files and harnesses are decoded from bytes so CR and CRLF are
not changed by Python universal newline handling.

Source preservation does not alter template value semantics. Template cooked
and raw values normalize CR and CRLF to LF as required by TV/TRV, while the
function source slice retains the original source line terminators.

```text
[Decision Log]
- 목적과 의도: ECMAScript 함수의 정확한 source text를 보존하면서 internal UTF-16, class lowering, dynamic Function, callable exotic 경계를 유지한다.
- 기존 구현 및 제약 조건: parser와 compiler가 comments/whitespace/escape spelling을 버렸고 interpreted 함수는 합성 `...` 본문을 반환했다. host source와 eval source는 sentinel-range scalar 처리 방식이 다르며 compiler는 class와 method를 별도 FunctionDef로 낮춘다.
- 검토한 주요 대안: AST에서 source를 재출력, 런타임에 파일 전체와 line/column 보관, 모든 함수에 합성 NativeFunction 반환, 또는 token byte span에서 production별 source slice 보존.
- 선택한 방식: token에 byte span을 기록하고 parser가 production 경계에서 provenance-aware Arc<str> 조각을 만든 뒤 FunctionExpr/ClassMethod/ClassExpr와 FunctionDef로 전달한다. source가 없는 합성/native/callable exotic 함수만 NativeFunction 문법을 사용한다.
- 다른 대안 대신 이 방식을 선택한 이유: AST 재출력은 comments, whitespace, escapes, line endings를 복원할 수 없고 파일 전체 보관은 함수 수명 동안 불필요한 source를 유지한다. 전부 NativeFunction으로 처리하면 명세가 요구하는 exact source를 회피한다.
- 장점, 단점 및 영향: ordinary/generator/arrow/method/accessor/class/dynamic 함수와 CR/LF/CRLF 및 Unicode escape spelling이 정확히 보존된다. 각 함수 source 조각은 독립 Arc allocation이므로 중첩 source의 최악 메모리 중복은 향후 shared source-unit range 구조로 줄일 수 있다.
```

## Script and Module lexical goals

Lexer construction fixes the source goal before tokenization. Script, direct
and indirect eval, and dynamic Function parsing enable Annex B HTML-like
comments; Module parsing disables them before any token is produced. Module
source therefore interprets `<`, `!`, `--`, and `>` as ordinary operators even
when their spelling resembles an HTML marker.

Script `<!--` consumes the remaining characters on its line from any
inter-token position. Script `-->` is admitted only by the initial Script
lexical goal or after an input-element line terminator. `html_close_allowed`
tracks that condition independently from source line/column accounting:
newlines inside string continuations or template tokens advance diagnostics but
must not admit a following close comment. A multiline comment containing a
line terminator does admit one. Dynamic Function's existing separate parameter
and body validation supplies the required boundary: a parameter marker has no
synthetic leading newline, while body text is parsed after one.

```text
[Decision Log]
- 목적과 의도: Annex B.1.1 HTML-like comments를 Script 계열 source goal에만 구현하고 Module 및 literal 내부 tokenization을 보존한다.
- 기존 구현 및 제약 조건: lexer는 Script와 Module을 같은 comment grammar로 시작했고 HTML marker를 인식하지 않았다. `saw_newline`은 진단 line tracking과 token 사이 LineTerminator를 함께 나타내며 string/template 내부 줄바꿈에도 설정된다.
- 검토한 주요 대안: parser에서 marker token을 사후 제거하기, source text 전처리, `saw_newline` 재사용, 또는 source-goal flag와 별도 close-admission state를 lexer에 두기.
- 선택한 방식: Script/internal lexer만 Annex B mode를 켜고 Module lexer는 끈다. `<!--`는 line comment로 처리하며 `-->`는 initial/inter-token state에서만 처리한다. multiline comment의 실제 LineTerminator만 state를 다시 켠다.
- 다른 대안 대신 이 방식을 선택한 이유: parser 사후 처리는 lexical goal과 ASI 정보를 잃고 전처리는 literal/regex/template 내용을 손상한다. `saw_newline` 재사용은 token 내부 줄바꿈을 오인한다. 독립 state는 명세의 input-element 경계를 직접 표현한다.
- 장점, 단점 및 영향: first-line, whitespace/block-comment prefix, LS/PS, ASI, eval, dynamic Function, Module operator fallback, regex/template raw 및 interpolation 경계가 한 scanner 규칙으로 정렬된다. source goal은 lexer 생성 시 고정되므로 향후 새 parse entry point도 mode를 명시해야 한다.
```

## Annex B declaration planning

Sloppy Script and FunctionBody compilation performs a separate Annex B.3.2
planning pass. The plan records each admitted ordinary FunctionDeclaration
site and its outer variable name without adding nested block functions to the
ordinary `VarDeclaredNames` collector. BlockDeclarationInstantiation always
creates the lexical binding. `AnnexBMirror` copies that exact block binding to
the variable environment only when the declaration statement is evaluated.

Global and eval declaration instantiation filter the plan before bytecode is
emitted. Function parameters, an existing `arguments` object, top-level and
intermediate declarative bindings, destructuring catch parameters, restricted
global properties, and non-extensible globals suppress the outer binding.
Object Environment Records and a same-named simple catch parameter retain the
web-legacy exceptions. Global mirrors construct an Environment Reference and
use shared PutValue, so Realm-specific accessors and property descriptors are
observed instead of mutating the environment map and global object separately.

The same environment walk implements Annex B.3.4 for ordinary eval `var` and
function declarations. It ignores Object Environment Records and only the
matching simple catch parameter; destructuring catch parameters and all other
intervening declarative bindings remain conflicts. At the variable-environment
boundary it checks only `let`, `const`, and import bindings. This boundary check
is required because RuJa stores FunctionBody top-level lexical bindings in the
same Environment Record as parameter and var bindings, while the specification
models a separate lexical environment. All four function declaration forms use
this walk, preventing function declarations from bypassing lexical conflicts.

The current plan keys sites by parsed `Stmt` address. This is valid because a
plan is built and consumed synchronously against the same owned AST; plans are
crate-private and never survive cloning or reparsing. A future persistent or
cached declaration plan must introduce stable AST node IDs before crossing
that ownership boundary.

Annex B.3.3 is represented directly as its specification rewrite: a sloppy
ordinary FunctionDeclaration used as an `if` clause becomes the sole statement
of a synthetic BlockStatement. The parser does not admit generator, async,
strict, Module, labelled, loop, or `with` variants. Reusing the ordinary block
AST means block declaration instantiation and the B.3.2 site plan remain the
single runtime model instead of adding an `if`-specific declaration path.

Label chains are parsed iteratively up to the statement-depth limit, then
rebuilt with their source lines. The compiler flattens labels whose ultimate
item is an iteration into one control frame carrying every alias. Explicit
`Iteration`, `Switch`, and `Label` frame kinds keep unlabelled `break` and
`continue` resolution distinct, and pending labels are isolated while nested
function bodies compile.

Annex B.3.5 keeps its initializer on the existing `VarDecl` used as the
`ForIn.left` node. The parser admits it only for one sloppy `var`
BindingIdentifier. Compilation evaluates that declaration once before the RHS,
which reuses the ordinary LoadRef/PutValue sequence and therefore captures a
`with` object binding before initializer side effects. Per-iteration `var` key
updates also resolve a Reference rather than declaring a binding in the loop's
compiler scope; PutValue's expression result is immediately popped because
loop binding evaluation has no value result. Parenthesized expressions parse
with `+In`, even when the surrounding initializer has the `~In` restriction.

```text
[Decision Log]
- 목적과 의도: Annex B.3.4의 simple catch parameter 예외를 direct eval의 모든 var-scoped declaration에 적용하면서 destructuring catch와 lexical conflict는 유지한다.
- 기존 구현 및 제약 조건: eval var conflict 검사는 simple catch도 차단했고 function declaration은 검사를 건너뛰었다. RuJa는 FunctionBody top-level lexical binding과 var binding을 같은 Environment Record에 저장한다.
- 검토한 주요 대안: exact Test262 admission, catch evaluation 시 synthetic outer assignment, 환경 표현 전체 분리, 또는 기존 EvalDeclarationInstantiation walk를 binding-kind aware하게 확장하기.
- 선택한 방식: Object Environment Record와 이름이 같은 simple catch만 무시하는 공용 eval declaration walk를 사용하고, stop environment에서는 lexical binding kind만 차단한다. source destructuring catch의 VarDeclaredNames 충돌은 parser early error로 검사한다.
- 다른 대안 대신 이 방식을 선택한 이유: admission은 엔진 결함을 숨기고 synthetic assignment는 declaration instantiation과 initializer resolution을 섞는다. 환경 전체 분리는 이 단위보다 넓고, binding kind 검사는 현재 표현에서 명세 경계를 보존한다.
- 장점, 단점 및 영향: var initializer와 네 function declaration form, with, global configurable binding, generator resume가 같은 규칙을 따른다. 현재 환경 압축 표현을 유지하므로 향후 Environment Record 분리 시 stop-boundary 특례를 제거해야 한다.
```

```text
[Decision Log]
- 목적과 의도: Annex B.3.2의 lexical block binding과 조건부 outer var binding을 분리하고, 선언이 실제 평가된 시점에만 동일한 함수 객체를 복사한다.
- 기존 구현 및 제약 조건: Sloppy block functions를 곧바로 function-scope var로 취급해 false branch, lexical shadowing, eval 환경 검사, switch 실행 위치를 표현할 수 없었다. RuJa의 Global Environment Record는 환경 map과 Realm global property를 함께 사용한다.
- 검토한 주요 대안: 모든 block function을 var로 유지하기, AST를 재작성해 synthetic var/assignment를 삽입하기, 이름 단위 전역 플래그만 저장하기, 또는 site별 declaration plan과 전용 mirror opcode를 사용하기.
- 선택한 방식: 실제 VarDeclaredNames와 별도인 source-order plan을 만들고, Block/CaseBlock lexical instantiation 후 admitted site에만 AnnexBMirror를 방출한다. Eval/Global instantiation이 plan을 환경별로 필터링하고 global mirror는 Reference/PutValue를 재사용한다.
- 다른 대안 대신 이 방식을 선택한 이유: 이름 단위 상태는 같은 이름의 여러 실행 위치를 구분하지 못하고 AST 재작성은 source metadata와 기존 compiler passes를 흐린다. 전용 opcode는 synthetic assignment의 정확한 평가 시점을 보존하면서 값 쓰기는 공용 Reference 의미론에 맡긴다.
- 장점, 단점 및 영향: false branch와 switch case timing, parameter/arguments/lexical/catch 충돌, duplicate final-function binding, accessor/non-writable/foreign-Realm global behavior가 한 모델로 정렬된다. B.3.3도 synthetic Block으로 같은 모델을 재사용한다. 계획은 현재 동일 AST 수명에 묶여 있으며 persistent compilation에는 stable node ID가 필요하다.
```

```text
[Decision Log]
- 목적과 의도: Annex B.3.3의 sloppy bare-if FunctionDeclaration을 명세 rewrite와 같은 block semantics로 구현하고, 중첩 label 제어 대상을 정확히 보존한다.
- 기존 구현 및 제약 조건: single-statement parser는 모든 FunctionDeclaration을 거부했고, compiler는 label 하나와 switch 여부 bool만 저장해 label chain 및 non-loop label 안의 unlabelled control을 구분하지 못했다.
- 검토한 주요 대안: if compiler에 전용 function-hoist 특례를 추가하기, parser admission만 넓히기, 또는 synthetic Block AST와 명시적 control-frame kind를 사용하기.
- 선택한 방식: sloppy ordinary if-clause function만 synthetic Block으로 낮추고 기존 B.3.2 plan을 재사용한다. label chain은 iterative parse 후 loop frame alias로 합치며 frame kind로 break/continue 대상을 선택한다.
- 다른 대안 대신 이 방식을 선택한 이유: runtime 특례는 BlockDeclarationInstantiation와 outer mirror 규칙을 중복시키고 parser admission만으로는 lexical binding을 만들지 못한다. 명시적 frame kind는 label/switch/iteration의 서로 다른 제어 규칙을 자료형으로 고정한다.
- 장점, 단점 및 영향: B.3.3의 네 production과 dangling-else ownership이 한 AST 규칙으로 정렬되고 hostile label nesting은 Rust stack을 소모하지 않는다. synthetic block은 source에 없지만 declaration line metadata를 보존한다.
```

```text
[Decision Log]
- 목적과 의도: Annex B.3.5 initializer의 평가 순서와 var loop binding의 동적 Reference 의미론을 명세와 일치시킨다.
- 기존 구현 및 제약 조건: parser는 모든 for-in declaration initializer를 거부했고, compiler는 var iteration key를 현재 scope의 임시 let binding처럼 생성했다. PutValue는 저장값을 stack에 남긴다.
- 검토한 주요 대안: initializer를 첫 iteration에 평가하기, for-in 전용 저장 opcode를 추가하기, 또는 기존 VarDecl 및 Reference/PutValue 경로를 재사용하기.
- 선택한 방식: sloppy 단일 var identifier만 admission하고 기존 VarDecl을 RHS 전에 한 번 compile한다. iteration key는 매번 ResolveBinding하며 사용하지 않는 PutValue 결과는 즉시 Pop한다.
- 다른 대안 대신 이 방식을 선택한 이유: 첫 iteration 평가는 빈 RHS와 abrupt ordering을 깨고 전용 opcode는 with/global/eval 의미론을 중복한다. 공용 Reference 경로는 이미 이 환경 구분을 구현한다.
- 장점, 단점 및 영향: initializer once/order/name inference, Object Environment Record, global property identity, direct eval binding kind, iterator-close stack이 한 경로로 정렬된다. 이 단위는 destructuring, let/const, strict, Module, for-of initializer를 의도적으로 허용하지 않는다.
```

## Module requests and typed modules

Module parsing stores every static import or re-export as a `ModuleRequest`
containing its decoded specifier and UTF-16-sorted Import Attributes. Equal
requests are deduplicated only after attributes are included; the loader then
maps a relative source path plus its selected JavaScript/JSON/text type to one
canonical ModuleRecord key. Static and dynamic imports therefore share
evaluation state, namespace identity, and JSON default-object identity.

JSON and text files become synthetic one-default-export ModuleRecords. The VM
parses or materializes their default value once while the graph resolves and
stores it in the record. Graph-local values are pinned until publication; VM
root enumeration traces cached synthetic defaults afterward. Dependency
evaluation only initializes the synthetic binding. User code cannot observe or
replace that operation through the global `JSON.parse` property.

```text
[Decision Log]
- 목적과 의도: Static Import Attributes와 JSON/text modules를 dynamic import의 기존 typed-module surface와 하나의 ModuleRequest, cache, linking model로 통합한다.
- 기존 구현 및 제약 조건: ModuleRequest는 specifier만 저장했고 static grammar는 module specifier 직후 semicolon을 요구했다. JSON/text loading은 dynamic import 전용이었으며 filesystem path만 사용하는 graph resolution은 같은 파일의 JavaScript, JSON, text identity를 구분할 수 없었다. Module graph는 dependency-first로 instantiate/evaluate하며 namespace와 live binding은 ModuleRecord environment를 참조한다.
- 검토한 주요 대안: `with {}`를 파싱만 하고 무시하기, static loader를 dynamic loader와 별도로 복제하기, JSON source를 JavaScript object literal로 변환하기, synthetic module에서 전역 `JSON.parse`를 호출하기, 또는 parsed Value를 graph loading 중 임시 Rust local로 보존하기.
- 선택한 방식: Decoded attributes를 UTF-16 key order로 ModuleRequest에 저장하고 request equality에 포함한다. Host가 지원하는 유일한 key `type`을 graph loading 전에 검증하고 physical path와 module type으로 cache key를 만든다. JSON/text default value를 resolution에서 한 번 만들고 synthetic ModuleRecord에 보존하며 graph-local payload/environment는 publication까지 pin한다. 평가 시에는 저장된 value로 default binding만 초기화한다. Static/dynamic paths는 같은 resolver와 ModuleRecord cache를 사용한다.
- 다른 대안 대신 이 방식을 선택한 이유: Attribute 무시는 host semantics와 cache identity를 깨뜨리고, loader 복제는 static/dynamic namespace identity를 갈라놓는다. Object literal 변환은 `__proto__`와 JSON grammar가 달라지며, 전역 `JSON.parse` 호출은 사용자 mutation을 관찰한다. Graph loading 중 heap Value를 보관하면 아직 VM root registry에 publish되지 않은 값의 GC ownership이 불명확하다.
- 장점, 단점 및 영향: 모든 import/re-export form이 같은 request를 전달하고 invalid JSON 및 named JSON imports가 linking 전에 실패하며 static/dynamic JSON objects가 동일하다. Escaped lone surrogates를 포함한 JSON strings를 보존하고 JavaScript/JSON/text views는 충돌하지 않으며 self-text import도 cycle 없이 동작한다. 현재 host는 relative files와 `type: "json"`/`"text"`만 지원한다. Bare specifiers, source-phase/deferred imports, Realm별 module graph/cache, arbitrary host attribute policies는 후속 범위다.
```

## Garbage collection

A mark-and-sweep collector with optional incremental marking reclaims
reference cycles. Collection runs at safe points only (after a run settles,
and throttled at frame boundaries). Incremental marking via
`collect_incremental(roots, budget)` charges trace work units: one per newly
traced cell header, physical pre-sweep retrace slot, dirty revisit, and Array
or internal Iterator item, Promise handler, FinalizationRegistry cell, or Map
entry. Cursorized containers snapshot the pass-start length and count down so
growth cannot extend a pass; consecutive slots use one lock scope while
retaining per-slot accounting. The
`usize::MAX` stop-the-world path keeps the direct atomic vector tracer instead
of constructing resumable state. The retrace phase stores its physical cursor
across calls. Mutation access to an already-scanned object conservatively queues
its identity once, so edges added between slices are traced before sweep. Proven
read-only collection and ordinary Get/HasProperty/GetPrototypeOf snapshots
retain the active-access root without dirtying the owner, which prevents those
tested Map observations from indefinitely restarting a multi-slice cursor.
Sweep, ordinary property tables, Promise continuation payloads, and other
container payloads remain atomic, so the
collector does not claim a strict pause-time bound. There is no
generational collector. A `gc_pins` stack lets call paths pin
heap values held in Rust locals across allocations that could trigger a GC.

```text
[Decision Log]
- 목적과 의도: Bound the pre-sweep mutation retrace and live stable indexed-container edges without weakening incremental GC liveness or slowing the unbounded full-collection path.
- 기존 구현 및 제약 조건: The finite budget originally stopped only initial cell marking; final root closure, complete marked-cell retrace, Array/Iterator items, Promise handlers, FinalizationRegistry cells, Map entries, and sweep ran atomically. Interior-mutability containers can change between slices, allocation continues, FinalizationRegistry targets/tokens are weak, and sweep cannot safely pause without a stronger root/mutation barrier. Treating every interior access as mutation can also requeue a dirty Map after each read and prevent completion.
- 검토한 주요 대안: Cursorize sweep immediately, remove final retrace, persist borrowed guards or copied vectors, scan one vector slot under fresh locks, batch all finite vector work without per-slot accounting, keep conservative dirtiness for reads, or combine a snapshot-length cursor with mutation-specific access barriers.
- 선택한 방식: Persist one LIFO `TraceWork` stack containing cell headers and cursors for Array/Iterator items, Promise handlers, FinalizationRegistry cells, and ordered Map entries. Snapshot pass-start length, count down to preserve prior reverse visitation, charge every snapshot record including removed records, batch only the current slice's remaining records under one lock, and place discovered children above the continuation. Share one always-inlined Promise-handler root visitor between finite and direct tracing and trace only registry held values. Newly reached cells use the direct tracer for `usize::MAX`, while parked cursors drain to completion. Active mutation access is both a current root and a dirty owner; proven collection, ordinary property, prototype, and iterable read snapshots retain only the active root.
- 다른 대안 대신 이 방식을 선택한 이유: Borrowed guards cannot cross calls; copied vectors multiply memory and clone cost; per-slot locking regresses dense arrays; live-length cursors can be extended indefinitely; and pausing sweep can free a cell before a later host edge publishes it. Snapshot cursors plus retrace/dirty passes preserve liveness and termination while retaining the existing fast full-GC traversal.
- 장점, 단점 및 영향: `budget=0` performs no trace or sweep on a non-empty heap; `budget=1` advances one cell, retrace/dirty header, or cursorized record. Growth is found by the fresh retrace or dirty revisit, shift removal and replacement are repaired by a dirty pass, removed records still consume bounded snapshot work, and repeated direct or compiled Map Get/HasProperty/GetPrototypeOf observations cannot livelock a dirty cursor. Newly queued roots preempt parked cursors, and `usize::MAX` completes pending cursors without yielding. Read-only callers must not mutate through interior locks. Own-descriptor/key enumeration, extensibility/integrity, classification, Promise/await, RegExp/String/Array, and host-observer paths still use conservative mutation access and require a later classification unit before claiming a universal observation guarantee. One Promise handler can still expose large AsyncFunction stack/local/catch vectors within one unit. Root/bitmap setup, ordinary properties, Set generation storage, WeakMap ephemerons, LazyGenerator state, nested Promise continuations, weak cleanup, and sweep remain explicit cursorization follow-ups.
```

Native functions carry `Option<NativeConstructMode>` metadata instead of
deriving constructibility from an observable `.prototype` slot. `None` means
the function has no `[[Construct]]` internal method.
`Some(InternalEagerPrototype)` lets an internal allocator run after dispatch
has observed `NewTarget.prototype`, while
`Some(InternalDeferredPrototype)` gives the native body ownership of whether
and when that lookup occurs. The obsolete generic native-receiver
preallocation mode has no remaining user and was removed. Registration tests
inventory constructors and ordinary native functions in both the main and
created Realms so callability, constructibility, and allocation policy cannot
silently collapse together.

BigInt and Symbol intentionally have `[[Construct]]`: they can participate in
class heritage and serve as a `newTarget`, but their bodies throw before
coercing arguments. Proxy and the abstract `%TypedArray%` constructor also use
the body-controlled mode because dispatch must not observe
`NewTarget.prototype` before their own validation. Each created Realm installs
its own Proxy constructor and `revocable`; the result pair, revoker, and
construct-trap argument array are allocated with that operation Realm's
intrinsics. Exact-cap construction pins every provisional Proxy value before a
collecting allocation.

Bound Functions and Proxies store immutable `constructable` metadata when
their exotic object is created. `is_constructor_value` therefore implements
`IsConstructor` as a constant-time `[[Construct]]` capability read: it neither
walks a target chain, checks Proxy revocation, nor consumes fuel.

`constructor_realm` and `construct_with_new_target` share one constructor-step
classifier for the operations that do follow targets. A Bound or live Proxy
edge consumes one fuel unit immediately before traversal. Proxy revocation is
validated before that charge and before observable `construct` lookup. Each
followed wrapper, Proxy target, handler, trap, and argument array belongs to
the outer construction pin scope, so normal, thrown, allocation, and host-abort
exits restore the incoming pin depth even if a trap getter revokes its Proxy.

Construction records Bound wrapper IDs outer-to-inner and materializes the
argument list once in reverse wrapper order, followed by the original call
arguments. This preserves `innerArgs, outerArgs, callArgs` and per-wrapper
`newTarget` substitution while making copying linear in wrappers plus values.
The combined argument list shares the 1,048,576-entry call-argument sandbox
cap. Normal and spread `super()`, Reflect, species constructors, and native
constructor dispatch all use this path.

Eager native construction records either the already-observed object
prototype or the already-resolved fallback Realm in `NewTargetPrototype`.
Native bodies reuse that state, so non-object fallback does not repeat
`GetFunctionRealm`; both variants are GC roots in pending and active execution
contexts. Ordinary interpreted receiver allocation now uses the collecting VM
allocator while pinning its resolved prototype across a heap-cap retry.

Interpreted derived constructors retain the active caller Realm immediately
before installing the callee execution context. ECMAScript conceptually removes
that callee context before checking the returned value and initialized `this`;
RuJa performs local call finalization before truncating its context record. The
snapshot lets only the post-body primitive-return `TypeError` and
uninitialized-this `ReferenceError` use the resumed caller Realm. Body runtime
errors continue through frame interpretation in the callee Realm, and an
already-thrown JavaScript value is never re-materialized. `newTarget` affects
prototype selection but not this error Realm. The caller context and Realm
registries keep the snapshot live across collecting error allocation.

```text
[Decision Log]
- 목적과 의도: interpreted [[Construct]]의 body 오류 Realm과 postcondition 오류 Realm을 명세 execution-context 경계대로 분리한다.
- 기존 구현 및 제약 조건: call finalization과 callee context truncation 순서 때문에 공용 error mapper가 postcondition 오류도 callee Realm에 귀속했다.
- 검토한 주요 대안: 공용 mapper 변경, teardown 순서 재작성, Error record에 Realm 저장, 또는 derived-only caller Realm snapshot.
- 선택한 방식: derived call만 caller Realm을 저장하고 두 postcondition 오류를 명시적으로 materialize한다. body/setup 오류 경로는 변경하지 않는다.
- 다른 대안 대신 이 방식을 선택한 이유: 가장 작은 상태로 spec Realm split을 직접 표현하며 native, async, generator, catch/finally teardown을 흔들지 않는다.
- 장점, 단점 및 영향: Bound/Proxy, foreign Reflect/newTarget, nested Realm, explicit throw identity, body errors와 GC가 동일 contract를 공유한다. 향후 context truncation을 spec 순서로 이동하면 snapshot 중복 여부를 다시 평가해야 한다.
```

```text
[Decision Log]
- 목적과 의도: Bound and Proxy constructor operations must preserve ECMAScript capability, Realm, argument, trap, and newTarget semantics while remaining stack-safe, fuel-bounded, linear, and GC-safe.
- 기존 구현 및 제약 조건: Bound IsConstructor walked targets, Realm and Construct edges were unmetered, every Bound layer recopied the accumulated argument list, eager native fallback repeated Realm traversal, and Proxy traversal roots depended on retained revoked slots.
- 검토한 주요 대안: Meter the existing IsConstructor walk, impose a wrapper-depth cap, keep repeated argument prepending, flatten each time a Proxy is reached, or cache immutable capability and carry one argument/prototype plan through the shared dispatcher.
- 선택한 방식: Cache Bound constructability at creation, keep IsConstructor constant-time, meter only GetFunctionRealm and actual Construct edges after revocation validation, collect wrapper IDs and flatten once, and pass observed-prototype or fallback-Realm state into native execution.
- 다른 대안 대신 이 방식을 선택한 이유: IsConstructor does not recursively inspect targets in ECMAScript; fixed depth caps reject legal programs; repeated or intermediate flattening remains quadratic; and recomputing fallback Realm changes exact fuel and can reorder errors relative to native constructor validation.
- 장점, 단점 및 영향: Deep legal chains remain stack-safe and now have exact host work bounds, 4,096 one-argument layers preserve order with linear materialization, intrinsic Promise settlement retains staged work after Fuel without replaying completed handlers or then access, and all construction callers share one policy. Arbitrary species capability functions are not replayed after a host abort, unbounded hosts can still spend linear time, and temporary wrapper/root vectors remain native memory subject to the broader process-memory policy.
```

Deep Proxy property forwarding uses the same stack-safety rule. Transparent
`get` chains are iterative and do not consume ordinary prototype depth.
`getOwnPropertyDescriptor` stores rooted target/trap-result pairs while
descending and validates them from the ordinary target outward;
`isExtensible` similarly collects trap booleans and checks invariants in
reverse. Short-lived roots are removed before pending roots are installed so
the LIFO pin stack cannot discard a fresh descriptor result. This trades
`O(depth)` temporary host memory for bounded Rust call-stack use and exact GC
liveness at depths exercised up to 100,000 wrappers.

```text
[Decision Log]
- 목적과 의도: Separate native `[[Construct]]` presence from receiver allocation while making wrapper forwarding stack-safe and GC-safe at adversarial depth.
- 기존 구현 및 제약 조건: Native constructibility depended on a mutable prototype slot, Proxy was shared across created Realms, `super()` used `[[Call]]`, and recursive Bound/Proxy/property forwarding could overflow the Rust stack or lose temporary values during collection.
- 검토한 주요 대안: Keep prototype-presence inference, add constructor-name exceptions, reject BigInt and Symbol at IsConstructor time, cap wrapper depth, or model constructibility explicitly and flatten the abstract-operation traversals.
- 선택한 방식: Store `Option<NativeConstructMode>`, let body-controlled constructors reject or validate in spec order, route all construction including `super()` through one iterative dispatcher, and reverse-validate rooted Proxy trap results.
- 다른 대안 대신 이 방식을 선택한 이유: Observable properties cannot represent internal methods, a depth cap changes valid JavaScript behavior, and per-builtin forwarding would duplicate new-target and GC rules. Explicit metadata plus iterative traversal preserves the abstract operation without using host recursion.
- 장점, 단점 및 영향: BigInt, Symbol, Proxy, BoundFunction, Realm, and exact-cap behavior now share one testable contract; 100,000-layer Proxy operations no longer abort the process. Iterative descriptor validation retains `O(depth)` pending state, while family-specific fallback and coercion order is handled by the constructor units below.
```

String, Number, and Boolean use `InternalDeferredPrototype` because their
constructor algorithms must finish primitive conversion before any observable
`NewTarget.prototype` read. Their native bodies distinguish calls from
construction through the active execution context's `NewTarget`, never through
the supplied `this`. A call therefore returns a primitive even when invoked
with `Function.prototype.call` and an object receiver. Construction selects the
wrapper-specific default from the existing GC-rooted
`realm_primitive_prototypes` registry after following BoundFunction and Proxy
new targets to their function Realm. It then pins the selected prototype while
the common sandbox allocator creates one ordinary object and initializes its
wrapped primitive slot. String adds its immutable UTF-16 `length` property
after allocation.

```text
[Decision Log]
- 목적과 의도: Preserve the specification's primitive-conversion, prototype-selection, and allocation order for String, Number, and Boolean in every Realm.
- 기존 구현 및 제약 조건: Generic preallocation read `NewTarget.prototype` too early, hardcoded `%Object.prototype%` as the fallback, and treated an object `this` as proof of construction. Wrapper allocation must still honor GC rooting and the exact heap cap.
- 검토한 주요 대안: Add wrapper-name exceptions to generic dispatch, add three more eager allocation modes, or let this family own conversion, prototype lookup, and allocation in its existing native bodies.
- 선택한 방식: Use `InternalDeferredPrototype`, select immutable Realm primitive prototypes from the VM registry, and share one rooted wrapper-allocation helper across the three bodies.
- 다른 대안 대신 이 방식을 선택한 이유: The three algorithms all convert before `GetPrototypeFromConstructor`, while String also has call-only Symbol behavior. Dispatcher exceptions would duplicate builtin semantics and still conflate calls with construction.
- 장점, 단점 및 영향: Observable order, foreign-Realm fallback, Bound/Proxy forwarding, direct calls, and cap failures share one tested path. The helper is deliberately limited to primitive wrappers; Date uses the separate body-controlled path below.
```

Date also uses `InternalDeferredPrototype`, but it has a distinct call branch.
Calls return a current date String without coercing supplied arguments or
branding an object `this`. Construction copies a Date input or converts the
single/component arguments, applies `TimeClip`, and only then observes
`NewTarget.prototype`; abrupt conversion therefore prevents the lookup. A
non-object prototype selects the immutable Date prototype from the new
target's function Realm through the GC-rooted `realm_date_prototypes` map.

Each Realm installs its own Date constructor, prototype, prototype methods,
and static functions. `%Date.prototype%` is an ordinary unbranded object, and
constructed instances store `[[DateValue]]` in an internal private slot rather
than an observable property. The selected prototype is pinned while the
common sandbox allocator creates exactly one object, so a cap-triggered
collection cannot reclaim it and a saturated heap still uses the existing
Realm-local emergency `RangeError` path.

```text
[Decision Log]
- 목적과 의도: Preserve Date's call-versus-construct split, conversion/prototype order, hidden DateValue, Realm fallback, and exact sandbox allocation contract.
- 기존 구현 및 제약 조건: Generic receiver preallocation read `NewTarget.prototype` before Date conversion, treated an object `this` as construction, exposed Date state through `__time__`, and reused main-Realm Date intrinsics in created Realms.
- 검토한 주요 대안: Keep generic preallocation with Date-specific repair steps, reuse the primitive-wrapper allocator, or let the Date body own conversion, prototype selection, slot initialization, and allocation.
- 선택한 방식: Use `InternalDeferredPrototype`, install a complete Date intrinsic graph per Realm, resolve immutable Date fallbacks through a traced registry, and allocate one internally branded object after all observable conversion.
- 다른 대안 대신 이 방식을 선택한 이유: Date calls do not wrap a primitive and construction has Date-copy, parsing, component, and TimeClip branches. Reusing preallocation or the primitive helper would preserve the wrong order or misrepresent the internal slot.
- 장점, 단점 및 영향: Direct/call/apply/bound use, subclasses, foreign new targets, abrupt order, forced GC, and exact-cap failure now share one tested path. The Realm registry inventory grows from 28 to 29 families and remains manually synchronized across storage, tracing, rollback, and tests.
```

The four Dynamic Function constructors use `InternalDeferredPrototype` so
CreateDynamicFunction owns every observable step. At that milestone the
registration inventory was **14 eager / 31 deferred** native constructors.
Parameter arguments are converted left-to-right before the body, after which
RuJa's local-trust `HostEnsureCanCompileStrings` policy permits compilation.
Parameters and body are parsed separately with line terminators at both synthetic boundaries; a
third combined parse enforces cross-part early errors such as a strict body
with non-simple parameters. This prevents comments or delimiter text in one
part from consuming the other part while preserving the specification's
conversion-before-parse order.

A call treats the active native constructor as the effective new target, while
`Reflect.construct` and `new` retain the supplied `NewTarget`. The active
constructor's closure selects the generated function Realm. If
`NewTarget.prototype` is not an object, the fallback comes from the immutable
Realm function-prototype registries for `%Function%`, `%AsyncFunction%`,
`%GeneratorFunction%`, or `%AsyncGeneratorFunction%`; mutable global bindings
are not consulted. Ordinary generated functions create a prototype whose
parent is that Realm's `%Object.prototype%`, while generator families use the
corresponding generator prototype.

Compilation remains side-effect free until the observable prototype lookup
has completed. Nested compiled definitions are appended afterward, and a
failed allocation truncates only that outer suffix so compilation performed
re-entrantly by a prototype getter remains valid. Async functions allocate one
function cell; the other three kinds allocate a function plus their own
prototype through the GC-aware sandbox allocator. Every intermediate and
getter-produced prototype is pinned across a collecting allocation.

BoundFunctionCreate now obtains the target's real `[[Prototype]]`, including a
Proxy `getPrototypeOf` trap, instead of hardcoding the main Realm
`%Function.prototype%`. Proxy trap results are pinned before re-entrant
non-extensible-target invariant checks, and the selected result stays pinned
through bound-function allocation.

The allocated Bound Function is then rooted before its observable metadata
steps. `length` uses `HasOwnProperty` semantics through the target's
`[[GetOwnProperty]]`; only an own property triggers `Get(target, "length")`,
and only a Number value participates in truncation and bound-argument
subtraction. `name` is always read with `Get`, accepts only a String value, and
is prefixed with `"bound "`. The resulting own properties are configurable,
non-writable, non-enumerable data properties inserted in `length`, `name`
order. Because these are real properties, deleting bound `name` resumes
ordinary prototype lookup; the internal FunctionData name is not exposed as a
replacement exotic property.

```text
[Decision Log]
- 목적과 의도: Make BoundFunctionCreate expose specification-shaped name and length metadata without weakening observable order, Proxy semantics, Realm behavior, or sandbox GC limits.
- 기존 구현 및 제약 조건: Bound functions exposed a synthetic internal name and had no real own metadata descriptors; adding the required target getters also required the newly allocated wrapper and captured state to survive re-entrant collection.
- 검토한 주요 대안: Synthesize metadata in generic property lookup, eagerly copy target fields without abstract operations, coerce every target length, or install properties before allocating the bound object.
- 선택한 방식: Allocate and pin the Bound Function first, run exact HasOwnProperty/Get steps against the live target, compute length only for Number values, install ordered configurable data descriptors, and suppress the internal-name fallback only for Bound functions after deletion.
- 다른 대안 대신 이 방식을 선택한 이유: Synthetic lookup cannot model deletion or descriptors; direct field reads bypass Proxies and accessors; ToNumber is forbidden here; and observable metadata work before allocation would not match BoundFunctionCreate and would leave no wrapper root for captured state.
- 장점, 단점 및 영향: Exact metadata, abrupt completion identity, inherited-name behavior, forced GC, and exact-cap allocation now share one path. Ordinary and internal native functions retain their existing fallback behavior. Bound call-chain iteration, fuel, and argument materialization remain a separate dispatch concern.
```

Ordinary Bound Function `[[Call]]` and Proxy apply forwarding now use one
iterative call traversal. Each Bound or Proxy edge consumes fuel before it is
followed. Bound wrapper IDs are retained outer-to-inner, then their arguments
are materialized once in reverse wrapper order before the original call
arguments. This preserves `innerArgs, outerArgs, callArgs` and the innermost
bound `this` without repeated vector prepending.

A Proxy apply boundary materializes pending Bound arguments exactly once,
creates the trap argument Array with the current operation Realm's intrinsic,
and resets traversal so a Bound apply trap can itself be dispatched normally.
The cumulative 1,048,576-entry argument limit is checked before the apply
getter runs. Entry arguments, current targets, Bound wrappers, handlers,
traps, and arrays are rooted for every observable operation. Root-vector and
trap-call reservations are fallible, and every normal, thrown, allocation, or
host-abort exit restores the incoming pin depth.

The shared Realm-aware value-Array allocator computes the exact roots for its
items and prototype and reserves that capacity before publishing either. This
last reservation was added in follow-up commit `c64076f` after documentation
review found that the original call dispatcher still reached an infallible
`pin_many` inside trap-array construction. A test-only one-shot reservation
failure exercises the real helper and proves that it returns a catchable
`RangeError` without changing pin depth.

```text
[Decision Log]
- 목적과 의도: Make legal deep Bound calls stack-safe and linear while preserving exact Call, Proxy, Realm, fuel, argument-limit, and GC behavior.
- 기존 구현 및 제약 조건: Ordinary Bound calls recursed once per wrapper, prepended the complete argument vector at every layer, charged no traversal fuel, checked only the original call arguments, and depended on infallible native vector growth around observable Proxy work.
- 검토한 주요 대안: Raise the call-stack cap, impose a Bound-depth limit, flatten every time a Proxy is reached, retain recursive dispatch with a guard, or carry one rooted traversal and materialization plan through Bound and Proxy edges.
- 선택한 방식: Traverse Bound and Proxy call edges iteratively, meter each edge, retain wrapper IDs, materialize once at a Proxy trap or final target, enforce the cumulative cap before trap lookup, and reserve every native root or trap-call vector fallibly before mutation.
- 다른 대안 대신 이 방식을 선택한 이유: Fixed depth limits reject valid ECMAScript; recursive guards still consume the Rust stack; intermediate flattening remains quadratic; and infallible growth can abort the host instead of producing the sandbox's catchable RangeError.
- 장점, 단점 및 영향: A 20,000-layer ordinary call is stack-safe, argument work is linear, Bound apply traps and foreign Realms preserve identity, exact fuel bounds wrapper traversal, and all tested abrupt paths restore roots. The shared value-Array path now turns root-capacity failure into RangeError before publishing roots. Unbounded hosts can still request linear work across arbitrarily deep legal chains. Bound forwarding in OrdinaryHasInstance was intentionally left as a separate state machine at this boundary and is completed by the iterative instanceof unit below.
```

```text
[Decision Log]
- 목적과 의도: Implement one specification-shaped CreateDynamicFunction path for all four dynamic constructors without weakening Realm identity or the exact heap cap.
- 기존 구현 및 제약 조건: The old wrapper parse allowed synthetic-boundary ambiguity, prototype selection happened on incomplete rules, generated functions used main-Realm parents, raw allocation bypassed the sandbox retry, and compilation-table entries could leak after failure.
- 검토한 주요 대안: Keep four mostly separate constructors, broaden generic native preallocation, parse only one combined wrapper, or centralize conversion, parsing, Realm fallback, publication, and allocation in the existing dynamic body.
- 선택한 방식: Use deferred native construction, three grammar checks with newline boundaries, immutable constructor-Realm registries, post-lookup table publication, and rooted one- or two-cell allocation with suffix rollback.
- 다른 대안 대신 이 방식을 선택한 이유: Generic preallocation observes `NewTarget.prototype` too early, a combined-only parse does not model separate parameter/body grammar, and per-kind copies would drift on ordering and GC cleanup. One kind-parameterized path keeps the shared abstract operation explicit.
- 장점, 단점 및 영향: Call/construct order, all four Realm fallbacks, Bound/Proxy new targets, parser early errors, forced GC, and exact-cap failures now share tested invariants. String compilation remains synchronous and is governed by the local-trust host policy rather than opcode fuel; generated source is retained by the Function source-text pipeline above.
```

RegExp construction also uses `InternalDeferredPrototype`, bringing the
current native-constructor inventory to **13 eager / 32 deferred**. The
constructor first classifies its pattern through the shared specification
`IsRegExp` operation. An explicit internal `[[RegExpMatcher]]` marker provides
the fallback brand only when an observable `Symbol.match` value is absent.
Calls take the identity shortcut only when the pattern is RegExp, flags are
absent, and the pattern's constructor is the active constructor. Otherwise the
algorithm selects copied internal source/flags or ordered regexp-like property
gets, resolves the actual new target's prototype and Realm fallback, allocates
one matcher object, and only then performs source/flags string conversion and
initialization.

Each Realm now retains immutable `%RegExp%`, `%RegExp.prototype%`, and
`%RegExpStringIteratorPrototype%` identities. These add two registry families
because the RegExp prototype map already existed for literals; the complete
transactional inventory is now **31**. Literal creation, `RegExpCreate`,
species fallback, and `@@matchAll` use those maps rather than mutable global
bindings. The RegExp String Iterator and each match result use the method's
Realm, while species values, flags, lastIndex values, matcher state, and
iterator state are pinned across every re-entrant conversion, trap, call, and
allocation.

Native `RegExpBuiltinExec` now treats backend byte positions as an internal
transport detail. `RegExpBackendInput` either borrows the original internal
string or owns a normalized matcher view plus a sorted backend-byte to
original-UTF-16 boundary table. The owned path is required when a JavaScript
string contains adjacent sentinel-backed high and low surrogates under `u` or
`v`: the backend must see one scalar, while `lastIndex`, match `index`, capture
strings, and `d`-flag ranges must remain measured in the original two code
units. A Unicode `lastIndex` inside that pair maps to the code point's starting
boundary, matching `GetStringIndex` behavior.

Well-formed UTF-8 is canonicalized only where it crosses into a JavaScript
string. `utf16_from_scalar_str` has a direct single-copy path for text that
does not use the sentinel range; `push_utf16_scalar` expands a scalar in
`U+F0000..U+F07FF` through its UTF-16 pair. The lexer tags host source
separately from already-canonical source assembled by `eval`, `$262.evalScript`,
`$262.agent.start`, and dynamic Function constructors. Raw string, RegExp, and template characters
use that provenance; decoded Unicode escapes always produce a scalar and use
the ingress conversion. Serde and data-module boundaries use the same
conversion. `JSON.parse` copies raw input as an already
canonical JavaScript string and applies the helper only to newly decoded
Unicode escapes, avoiding double conversion. Serde export performs the inverse
UTF-16 decoding for valid pairs and replaces lone surrogates with U+FFFD because
its host string type cannot represent them. When two object keys collapse to
the same replacement string, serde export deterministically retains the later
key in internal property order. Unicode RegExp normalization
recombines adjacent sentinel-backed high/low units into one scalar atom before
backend compilation; non-Unicode mode retains separate code-unit atoms. Native
errors carry an explicit host/internal text-provenance bit. Ordinary error
constructors retain canonical internal text; `Error::host`, `syntax_host`, and
`type_err_host` mark OS or callback Unicode for one conversion during error
materialization. Module and script filesystem failures use the host path, so
paths cannot reintroduce an ambiguous raw scalar while errors containing
existing JavaScript strings are not converted twice. Mixed module-link errors
canonicalize only the host path fragment. `Error::Display` performs the inverse
operation for internal text and leaves host-tagged text unchanged.

```text
[Decision Log]
- 목적과 의도: Preserve valid Unicode scalars whose values overlap RuJa's internal lone-surrogate sentinel range at each audited well-formed UTF-8 ingress boundary.
- 기존 구현 및 제약 조건: Rust String cannot contain surrogate code points; RuJa stores each lone surrogate as U+F0000..U+F07FF, while valid scalars in that same range require two JavaScript UTF-16 code units. Applying conversion to an already-internal string would expand sentinel code units twice.
- 검토한 주요 대안: Replace every string with Vec<u16>, reserve a different private-use range, canonicalize every Arc<str> construction, or canonicalize only external scalar-producing boundaries.
- 선택한 방식: Keep the compact existing representation, add one scalar-ingress helper with a no-collision fast path, distinguish host source from already-canonical eval/dynamic-function source in the lexer, call the helper from external source text, serde, decoded JSON escapes, and JSON/text data-module loading, decode internal UTF-16 when exporting serde strings and keys, and tag native errors by host/internal text provenance.
- 다른 대안 대신 이 방식을 선택한 이유: No Unicode scalar range can also encode all lone surrogates injectively; a global Arc<str> rewrite cannot distinguish external UTF-8 from already-canonical internal text; and Vec<u16> would change every string consumer before the RegExp-specific matcher issue is solved.
- 장점, 단점 및 영향: String length, equality, code-unit access, templates, JSON, eval-generated source, constructor-based embedding, module paths, CLI output, and data modules now agree for both sentinel-range endpoints. Ordinary ingress performs one direct copy; internal source preserves lone UTF-16 surrogates without a second expansion; serde export is lossless for valid pairs and necessarily lossy for lone surrogates. Public enum construction remains an unchecked low-level escape hatch, and Unicode RegExp matching still needs a logical-symbol backend because its current scalar alphabet identifies a lone surrogate sentinel with a private-use scalar.
```

After matching, all capture endpoints are converted in one ordered pass and
reused for result strings, named `groups`, `lastIndex`, and match indices.
With the internal `[[RegExpHasIndices]]` flag set, exec allocates one
method-Realm Array per participating capture, explicit `undefined` entries for
nonparticipating captures, and a null-prototype `indices.groups` object whose
named properties alias the same pair objects. Pair arrays and groups are
pinned through nested allocation, materialization consumes fuel per capture,
and exact heap-cap failure restores the original pin depth.

The same work made the write pipeline receiver-aware end to end. OrdinarySet
stops at the nearest data descriptor, delegates through Proxy prototypes, and
preserves the original receiver. Proxy `set`, `getOwnPropertyDescriptor`,
`defineProperty`, `has`, and `isExtensible` invariant checks support nested
Proxies and root fresh trap results. CreateDataProperty and value-only
DefineProperty retain Array, integer-indexed TypedArray, and mapped-arguments
exotic behavior. ArraySetLength owns its two observable conversions, sparse
length calculation, non-writable guards, descending deletion and rollback,
descriptor synchronization, and inline-cache invalidation. An unmaterialized
Array `length` is synthesized as an own non-configurable data descriptor before
prototype traversal.

```text
[Decision Log]
- 목적과 의도: Make RegExp construction, RegExp String iteration, and their observable property writes follow ECMAScript ordering, Realm identity, Proxy invariants, and the exact sandbox heap contract.
- 기존 구현 및 제약 조건: RegExp identity depended on an observable class-name approximation, generic preallocation selected prototypes too early, mutable globals supplied Realm fallbacks, matchAll intermediates were not uniformly rooted, and partial property helpers bypassed Proxy and Array/TypedArray/arguments exotic methods.
- 검토한 주요 대안: Patch individual Test262 failures, retain eager allocation with RegExp exceptions, broaden feature admission, or centralize the abstract operations and exotic dispatch before admitting exact files.
- 선택한 방식: Use deferred RegExp allocation with an internal matcher marker and immutable Realm registries; route matchAll through ordered species and strict Set operations; and share receiver-aware Set/DefineProperty dispatchers with a complete ArraySetLength implementation.
- 다른 대안 대신 이 방식을 선택한 이유: Local exceptions preserve the same incorrect observable order and GC hazards, while broad admission would mix unsupported RegExp syntax and matching semantics into a constructor/iterator/property unit. Shared abstract operations keep Proxy invariants and exotic receivers consistent across callers.
- 장점, 단점 및 영향: Eight skips become passes, the built-ins failure count drops by a net 100, all supported language tests remain green, and forced-GC/exact-cap cases use one rooted path. The cost is two more manually synchronized Realm registries and a larger property core; 137 broader RegExp failures remain explicit follow-up scope.
```

### RegExp duplicate named captures

RegExp source validation and runtime compilation share
`scan_regex_named_captures`. Each nested disjunction frame unions names from
completed alternatives but rejects a name when two concatenated terms can
participate together. Capture occurrences remain ordered and retain separate
numeric indices; a second index map groups all occurrences by decoded
ECMAScript name. Result construction walks occurrence order, replaces an
earlier `undefined` with the sole participating capture, and leaves the
`IndexMap` slot in place so property enumeration follows the first occurrence.
`indices.groups` reuses the numeric pair object rather than allocating a copy.

```text
pattern
  -> structural named-capture scan and MightBothParticipate early errors
  -> ordered (name, capture index) occurrences
  -> name -> [capture indices] table
  -> numbered backend pattern plus short (?@set_id) references
  -> RegExp VM selects zero or one populated capture in each set
  -> groups / indices.groups participating value with first-name order
```

RuJa vendors `fancy-regex` 0.18.0 because duplicate-name backreferences and
ECMAScript repeated-capture clearing require matcher-state operations that
cannot be repaired after a match. The fork stores each capture set once and
keeps only its ID in parser, AST, compiler, and VM instructions. Quantified
capture entry clears descendant slots through the backend's copy-on-write
state; current-delta membership is a bitset, and every cleared slot consumes
the existing backend work budget. Case-insensitive backreferences consume the
same number of scalar values as the capture and use Unicode simple folding for
`u`/`v`, otherwise the legacy ECMAScript uppercase relation.

Patterns with backreferences use the bounded backend directly. Ordinary
patterns with repeated captures use `CaptureCorrected`: the linear Rust
matcher establishes whether and at which leftmost start a match exists, then
the ECMAScript backend supplies the match end and captures; callers use that
end for subsequent iteration and `lastIndex`. End positions cannot be shared:
nullable quantifiers may select a
longer ECMAScript match than the linear backend while keeping the same leftmost
start. This keeps no-match probes on the linear path while removing the old
post-match heuristic that could mistake trailing text for the final quantified
iteration. All fork
behavior is gated by `ecmascript_mode`; mode-off analysis retains upstream
delegation. Because Cargo substitutes a registry dependency when packaging a
path dependency, crates.io publication remains disabled until the fork is
upstreamed or published separately.

```text
[Decision Log]
- 목적과 의도: Support ECMAScript duplicate named captures, participating backreferences, and repeated-capture state without introducing quadratic compilation or an unbounded no-match path.
- 기존 구현 및 제약 조건: One name owned one index, duplicate declarations were always rejected, unmatched aliases needed empty backreference semantics, Rust captures retain stale values across iterations, post-match reconstruction could not observe matcher state, and path forks cannot transparently survive crates.io packaging.
- 검토한 주요 대안: Expand every alias into nested conditionals, guess the last iteration after matching, route every repeated-capture pattern entirely through a backtracking VM, hand-roll a new RegExp engine, or add isolated matcher-state primitives to a vendored backend.
- 선택한 방식: Share a structural early-error scanner, retain ordered occurrences plus one index table per name, lower references to ID-based BackrefSet instructions, clear captures transactionally in the VM, prefilter ordinary matches linearly, and gate the fork behind explicit ECMAScript options and a finite work budget.
- 다른 대안 대신 이 방식을 선택한 이유: Conditional expansion and repeated name scans are quadratic, post-processing is observably wrong, broad backtracking routing regresses hostile no-match patterns, and a replacement engine is too broad for this conformance unit. Backend state is the only point that can clear captures with correct backtracking restoration.
- 장점, 단점 및 영향: Exact duplicate-name syntax, groups, indices, replacement, backreference, Unicode case-fold, and quantified-state semantics are directly tested with linear source/table growth and bounded runtime work. The tradeoffs are a maintained backend fork and disabled crates.io publication until that fork has a registry path. The once-remaining hard variable-lookbehind case is closed by the directional matcher below.
```

```text
[Decision Log]
- 목적과 의도: Preserve linear rejection of hostile no-match inputs while making nullable quantified captures use ECMAScript match boundaries everywhere.
- 기존 구현 및 제약 조건: The Rust prefilter and ECMAScript matcher can agree on the leftmost start but legitimately choose different ends; exec, iteration, replacement, and lastIndex must all follow one authoritative result.
- 검토한 주요 대안: Require identical start/end boundaries, remove the prefilter, special-case one nullable pattern, or use the prefilter only to reject and locate a candidate start.
- 선택한 방식: Keep the prefilter as a no-match and leftmost-start oracle, require only start agreement, and use the bounded ECMAScript matcher for the end, captures, and next iteration position.
- 다른 대안 대신 이 방식을 선택한 이유: End equality rejects valid RepeatMatcher behavior, removing the prefilter regresses no-match complexity, and pattern-specific rewriting cannot cover composed nullable quantifiers. Start agreement retains the linear search boundary without overriding matcher semantics.
- 장점, 단점 및 영향: All capture-bearing APIs now share the ECMAScript boundary and global progress while failed probes remain linear. Each successful repeated-capture match pays one bounded backend execution; direct find APIs now use that result instead of returning the prefilter boundary.
```

### RegExp directional lookaround

`compile_regex_with_input_mode` detects assertions outside escaped text and
character classes and routes those patterns through the vendored ECMAScript
backend. Normalization still owns JavaScript-specific UTF-16 and case-fold
semantics. In particular, non-Unicode ignore-case literals and classes are
materialized from the legacy `Canonicalize` equivalence relation under a
scoped backend case disable; this admits `U+00E9` case pairs without also
admitting Unicode-only long-s or Kelvin folds.

The backend compiler carries an explicit forward or backward direction.
Lookahead compiles its subpattern forward; lookbehind compiles backward.
Backward concatenation visits terms in reverse execution order while leaving
alternatives and greediness unchanged. Capture groups save their end before
their start, and dedicated backward instructions consume literals, scalar
wildcards, general newlines, delegates, ordinary backreferences, and duplicate
name capture sets from the cursor toward the start of the input.

```text
RegExp source + flags
  -> source validation and JavaScript normalization
  -> lookaround detection
  -> ECMAScript fancy-regex parser
  -> forward lookahead / backward lookbehind compiler
  -> atomic assertion VM with one shared work budget
  -> capture byte ranges
  -> RuJa UTF-16 result materialization
```

Positive assertions save the outer cursor, enter an atomic region, run the
subpattern, leave the region, and restore only the cursor. Captures produced by
the successful assertion therefore remain observable, but later matching
cannot backtrack into the assertion. Negative assertions use a transactional
branch whose successful negative path discards subpattern state.

Annex B permits quantified lookahead only in legacy non-`u`/`v` patterns. The
parser exposes that exception only when both ECMAScript and legacy modes are
active. Finite and unbounded nullable repeats carry an explicit upper bound and
an ECMAScript empty-iteration failure mode, allowing child alternatives to
backtrack while preserving the required capture from completed iterations.

Resource accounting is also mode-specific. Every ECMAScript branch push,
attempted repeat iteration, and repeated-capture clear consumes the same finite
work budget, including paths that succeed without failed backtracking. A
terminal bound or no-progress check does not charge another iteration. The
deterministic `SplitUnanchored` search preamble is not charged. ECMAScript hard
execution retains a 100,000-entry stack cap; mode-off callers keep upstream's
one-million-entry cap and failed-backtrack counter.

```text
[Decision Log]
- 목적과 의도: Implement ECMAScript lookahead, lookbehind, backward captures and backreferences, and Annex B quantified-lookahead semantics without an unbounded backtracking path.
- 기존 구현 및 제약 조건: Rust regex cannot express variable-length lookbehind or assertion capture semantics, upstream fancy-regex searched lookbehind prefixes forward, successful zero-width repetitions could bypass the failed-backtrack counter, and broad backend routing would weaken RuJa's resource boundary.
- 검토한 주요 대안: Continue translating assertions to Rust regex, enumerate candidate lookbehind starts, post-process captures, replace the complete matcher, or add directional instructions and explicit ECMAScript accounting to the maintained backend.
- 선택한 방식: Detect assertions at the RuJa boundary, normalize JavaScript case and UTF-16 semantics first, compile lookbehind backward with atomic cursor restoration, implement the legacy RepeatMatcher exception in the parser/VM, charge speculative ECMAScript branches, attempted repeat iterations, and capture clears to one bounded budget, and cap the ECMAScript hard stack at 100,000 entries.
- 다른 대안 대신 이 방식을 선택한 이유: Translation cannot preserve assertion capture/backreference order, prefix enumeration changes greediness and scales with input length, post-processing cannot reconstruct transactional matcher state, and a new engine is too broad for this unit. Directional compilation follows the specification directly while reusing the audited VM state model.
- 장점, 단점 및 영향: The complete Test262 lookbehind subtree passes, hard duplicate-name lookbehind works, positive assertions remain atomic, and hostile successful zero-width or branch-growth patterns terminate under explicit limits. The cost is a larger maintained backend fork. At this historical decision boundary, unrelated RegExp grammar, empty-class, sentinel, nested-v, and linear-boundary work remained separate; the following sections record the later grammar and empty-class closures.
```

### RegExp grammar validation

`validate_regex_literal` validates ECMAScript source grammar before any
backend-specific normalization or compilation. Quantifier validation uses an
explicit `NoAtom` / `Atom` / `Prefix` state machine: an atom admits one
quantifier prefix, `Prefix` admits only one lazy `?`, and assertions reset the
state. Escape scanning consumes a complete atom only when its syntax is valid,
so malformed legacy `\x`, `\c`, or identity `\k` cannot hide a repeated
quantifier. Named-backreference skipping is enabled only after the shared
named-capture scan proves that the pattern has named captures.

Class range validation has two representations. Legacy mode tokenizes class
contents into UTF-16 code units because raw supplementary characters and
identity escapes may contribute two range atoms. It decodes Annex B octal and
control forms and preserves the incomplete-`\c` fallback as separate `\` and
`c` atoms. Unicode modes keep scalar endpoints, combine a fixed lead/trail
surrogate escape pair, reject character-set range endpoints, and distinguish a
single range `-` from the `v` subtraction operator. The Unicode syntax pass
owns nested `v` class depth and restricted brackets.

```text
RegExp source + flags
  -> flags and named-group validation
  -> Unicode escape/bracket/class-depth validation
  -> legacy UTF-16 or Unicode-scalar class-range validation
  -> atom/prefix/lazy quantifier state validation
  -> assertion/modifier validation
  -> JavaScript normalization and bounded backend compilation
```

```text
[Decision Log]
- 목적과 의도: Enforce ECMAScript RegExp early errors independently of backend parser quirks while preserving legacy UTF-16 and Annex B behavior.
- 기존 구현 및 제약 조건: The validator tracked only whether any atom had appeared, skipped malformed escapes too broadly, compared only Unicode character-set endpoints, and delegated range order and restricted brackets to backends whose grammars differ from ECMAScript.
- 검토한 주요 대안: Continue relying on backend errors, patch the 12 Test262 files by spelling, parse every RegExp into a new full AST, or add bounded source validators for the finite grammar surfaces.
- 선택한 방식: Use a small quantifier state machine, syntax-aware escape boundaries, a UTF-16 legacy class tokenizer, scalar Unicode endpoints with surrogate-pair composition, and explicit nested-v/subtraction checks before compilation.
- 다른 대안 대신 이 방식을 선택한 이유: Backend acceptance is observably wrong even for unexecuted literals, path-specific patches hide equivalent constructors, and a replacement parser is too broad for this unit. Mode-specific validators map directly to the relevant grammar invariants and can be differential-tested against Node.
- 장점, 단점 및 영향: Twelve failures become passes with no matrix movement outside built-ins; malformed quantifiers and ranges fail consistently for literals and constructors, and 1,219 class differentials show no regression. At this historical boundary, full v set algebra, Annex B backend lowering, empty-class execution, large-count policy, and hybrid nullable matching remained separate; the next section records the empty-class closure.
```

### RegExp host-independent quantifier bounds

The RegExp grammar scanner records each syntactically valid braced
quantifier's decimal spans while it performs the existing atom/prefix early
error pass. It compares canonical decimal lengths and digits directly, so
`min > max` is rejected without converting either side to a machine integer.
Bounds above `u32::MAX`, the maximum accepted by the linear Rust parser, are
routed directly to the ECMAScript counter backend. A linear-backend
`CompiledTooBig` error retries that backend only when the scanner already
proved the source contains a valid braced repeat; syntax errors never retry.
The backend does not expose which subtree exceeded its limit, so this guard
proves that counter compilation is applicable rather than attributing the
error to one repeat. The retry remains bounded and may also rescue a valid
pattern whose non-repeat subtree triggered the original limit.

The vendored AST stores a finite count as `Small(u128)` or canonical decimal
`Big(Arc<str>)`, and represents infinity as a separate enum variant. Analysis
uses saturating size arithmetic. Compilation lowers each count to whether it
is reachable by the host's `usize` execution counter; an unreachable finite
minimum cannot be completed by any host-sized input and is stopped by the
same VM work budget as other hostile repeats. The repeated body is emitted
once, so compile time and program size remain O(AST), independent of the
decimal value. Forced routing recursively marks every repeat subtree and all
its ancestors hard, preventing an oversized sibling from being delegated
back to the linear compiler.

```text
RegExp source + flags
  -> class/escape-aware quantifier scan
  -> exact decimal range comparison
  -> ordinary linear backend when representable
  -> direct or CompiledTooBig counter fallback when required
  -> one bounded repeat instruction plus one body
```

```text
[Decision Log]
- 목적과 의도: Accept every ECMAScript DecimalDigits quantifier value without host-width truncation, source-size expansion, or an unbounded execution path.
- 기존 구현 및 제약 조건: The vendored parser stored bounds in usize, the linear backend rejects values above u32 and can reject large representable repeats as CompiledTooBig, and broad fallback could accidentally reinterpret backend syntax errors.
- 검토한 주요 대안: Clamp at u32 or Number.MAX_SAFE_INTEGER, expand the repeated body, parse into f64, route every RegExp through the counter VM, or preserve exact bounds and select the counter path only when required.
- 선택한 방식: Store exact finite decimal values separately from infinity, compare ranges as canonical digits, saturate static size analysis, emit one counter loop, route above-u32 values directly, and retry only validated braced CompiledTooBig patterns.
- 다른 대안 대신 이 방식을 선택한 이유: Clamping and f64 lose specified integer identity, expansion makes compile cost proportional to the numeric value, and broad VM routing weakens the established linear fast path. Exact AST bounds plus selective routing preserve semantics and resource limits.
- 장점, 단점 및 영향: The final RegExp diagnostic failure becomes a pass with O(AST) compilation and bounded runtime work. The vendored backend gains a maintained exact-count type and routing option; finite values beyond host reach intentionally terminate by resource limit when their nullable body cannot fail earlier.
```

### RegExp empty-class lowering

ECMAScript permits an empty positive character class, which never matches, and
an empty negated class, which matches every input element. The Rust regex
backends reject `[]` and `[^]` as unclosed classes. Whole-pattern forms already
used equivalent backend atoms, but embedded forms such as `[]a`, `a[^]`, and
`a|b|[]` reached the backend unchanged.

The normalizer records the output offset when it enters an outer character
class. At that class's closing bracket, it compares only the complete
normalized class slice with exact `[]` and `[^]` spellings. A positive empty
class becomes `[^\s\S]`. A negated empty class becomes a dot-all scalar atom
in `u`/`v` mode or the existing complete UTF-16 plus surrogate-sentinel class
in legacy mode. The replacement remains one atom, so concatenation,
alternation, and following quantifiers retain their parse structure. Escaped
brackets, transformed class escapes, non-empty classes, and nested `v` classes
cannot equal either exact slice.

```text
[Decision Log]
- 목적과 의도: Execute ECMAScript empty character classes in every pattern position without changing source preservation, UTF-16 mode, Unicode mode, or backend selection.
- 기존 구현 및 제약 조건: Whole-source [] and [^] had explicit backend atoms, while five embedded Test262 forms reached backends that reject empty classes. Global text replacement would confuse escaped brackets and nested classes, and the normalized fragment must remain one quantifiable atom.
- 검토한 주요 대안: Patch the five tests by source spelling, teach both Rust backends new syntax, replace every textual [] occurrence, parse a new full RegExp AST, or reuse the existing class scanner's exact outer-class boundary.
- 선택한 방식: Share the established never-match and universal backend atoms, detect only exact normalized [] or [^] slices when an outer class closes, replace that complete slice before ignore-case materialization, and leave all other class state unchanged.
- 다른 대안 대신 이 방식을 선택한 이유: Test-specific patches miss constructors and equivalent compositions; backend changes duplicate ECMAScript policy; text replacement is escape-unsafe; and a new AST is disproportionate. The scanner already owns escape state, nested-v depth, class output boundaries, and mode information.
- 장점, 단점 및 영향: Five existing Test262 failures become passes with exact +5/-5 movement, literals and constructors share one path, and direct tests cover flags, quantifiers, UTF-16, Unicode, v mode, and escaped brackets. The oversized quantifier, nullable hybrid boundary, and broader valid nested-v set syntax remain independent RegExp units.
```

### RegExp character-only Unicode set algebra

Both `u` and `v` select Unicode pattern semantics even though only `v` enables
set operations. The normalizer previously used the shared `unicode_mode` flag
for some transformations but tested only literal `u` for decimal escapes,
identity escapes, and dot lowering. That split stripped the backslash from
valid `\p{...}` operands in `v` classes and lowered `.` as a legacy UTF-16
atom. These branches now consume the shared mode decision. Unicode dot lowers
to an exact one-scalar class excluding LF, CR, U+2028, and U+2029; active
dotAll, including inline modifiers, lowers to an explicit scalar dot-all atom.
The separate
`u`-specific negated-property ignore-case workaround remains unchanged because
`v` defines a different complement and case-folding order.

The initial admission froze the complete 48-file generated matrix whose operands are
single characters, nested character classes, character-class escapes, or
character property escapes. Every union, intersection, and subtraction pair
in that character-only `/v` matrix runs. Files involving properties of strings
or `\q{...}` string literals were kept feature-gated at that stage. The
string-valued unit below now extends this exact corpus. The adjacent word-set
unit lowers complex `/iv` `\w` and `\W` operands to ECMAScript
WordCharacters.

```text
[Decision Log]
- 목적과 의도: Make v-mode normalization obey Unicode pattern semantics and expose the complete supported character-only set algebra without admitting string-valued elements.
- 기존 구현 및 제약 조건: The normalizer already computed u-or-v Unicode mode but several branches retested only u, the Rust backend supports character set algebra and character properties, and Test262 applies one broad regexp-v-flag feature to both character and string-valued matrices.
- 검토한 주요 대안: Keep all v tests skipped, patch only property spellings, route every v pattern to a new engine, admit the complete regexp-v-flag feature, or correct the shared mode boundary and freeze the exact character-only matrix.
- 선택한 방식: Use unicode_mode for decimal, identity-escape, and exact LineTerminator-aware dot normalization; preserve the u-only negated-property case-folding rule; and remove v/property feature gates only for 48 exact generated character-set files.
- 다른 대안 대신 이 방식을 선택한 이유: Source spelling patches miss constructors and dot semantics, a new engine is disproportionate to a normalization split, and broad admission would hide unsupported string properties and q-string operands. Exact admission ties policy to behavior already exercised exhaustively by generated tests.
- 장점, 단점 및 영향: The 48-file character-only matrix ran at 100% while legacy and u behavior shared existing paths. The admission manifest required maintenance when Test262 changed; the later string-valued unit closes the 66 generated follow-up cases and expands the exact corpus further.
```

### RegExp Unicode-set word operands

Complex `v` classes remain in the backend's native set-algebra syntax because
the ordinary whole-class materializer does not parse nested intersection or
subtraction. That fallback previously left active-ignoreCase `\w` and `\W`
unchanged, allowing Rust's broader Unicode word inventory to enter otherwise
valid ECMAScript intersections, differences, unions, and complements.

The normalizer now separates two decisions. Every active-ignoreCase word
escape is lowered in place to the exact ECMAScript inventory: ASCII letters,
digits, underscore, U+017F, and U+212A, with the complement represented as a
nested negated class. Only ordinary classes set the flag that requests
whole-class HIR materialization. Complex `v` classes retain their structure,
so the backend performs the existing algebra over exact word operands instead
of reparsing or flattening the source.

This closes word-operand leakage through nested classes and through both the
linear and hard backreference/lookaround routes. String-valued properties and
`\q{...}` use the bounded logical matcher described below.

```text
[Decision Log]
- 목적과 의도: Remove Rust-only Unicode word characters from every complex iv set operand without replacing the existing bounded set-algebra backend.
- 기존 구현 및 제약 조건: Ordinary ignore-case classes are materialized as complete HIR sets, while nested v intersection and subtraction deliberately retain backend syntax. One shared guard disabled both whole-class materialization and the smaller word-escape rewrite, so Rust's Unicode word inventory leaked into complex sets.
- 검토한 주요 대안: Keep the documented mismatch, hand-parse all v algebra in the normalizer, add a second complete RegExp AST, replace the backend, or separate per-escape lowering from whole-class materialization.
- 선택한 방식: Always rewrite active-ignoreCase w/W operands to exact nested ECMAScript classes, but mark only ordinary classes for whole-class HIR materialization; retain native nested-v operators and existing execution routing.
- 다른 대안 대신 이 방식을 선택한 이유: The escape is an atomic grammar operand and can be replaced without interpreting surrounding algebra, while reparsing or replacing the full backend would combine word semantics with property folding and string-valued sets. Separating the two decisions fixes the proven leak at its source and preserves current resource bounds.
- 장점, 단점 및 영향: Direct tests cover word/property and word/literal intersections, differences, unions, nested complements, lookaround, and backreferences for Rust-only word characters and the two ECMAScript Unicode additions. A bounded Node word/literal probe is secondary evidence; current ECMA-262 remains authoritative where engines differ. Sentinel-range scalars and string-valued v operands remained explicit independent units until the later logical matcher and string-set units closed them.
```

### RegExp string-valued Unicode sets

`v` classes and atom escapes now retain a mathematical set of canonical,
deduplicated code-point sequences. `\q{...}` preserves empty alternatives;
single-character alternatives cross over exactly with ordinary character
sets. Under `/iv`, every sequence is simple-folded character by character
before union, intersection, or subtraction. A grammar-level
`MayContainStrings` bit follows the specification's OR/AND/left-operand rules,
so negated classes reject only expressions that may still contain strings.
Literal validation invokes the same vendored parser to report these static
errors before evaluation.

Final matching order is multi-code-point strings longest first, then one
character, then empty. Lookbehind reverses each sequence before lowering.
The emitter builds a shared-prefix trie and merges equal suffix subtrees behind
one bracket transition, avoiding one Pike state per Unicode string. Static
property data is charged before cloning; cumulative materialization and
estimated pre-emission work are each capped at 750,000 units, and elements
over 256 code points, more than 65,536 explicit alternatives, or a conservative
trie-node upper bound over 65,536 are rejected. Runtime work scales from 256
through 8,192
units per input symbol according to compiled state cost and remains capped at
32,000,000; aggregate live Pike state remains capped at 64 MiB.

```text
[Decision Log]
- 목적과 의도: Implement complete bounded ECMAScript string-valued Unicode set semantics rather than enabling the existing flat alternative prototype.
- 기존 구현 및 제약 조건: The vendored parser discarded empty q alternatives, compared iv strings before folding, modeled negation per operand, emitted one Pike branch per string, and failed to reverse strings in lookbehind; RuJa requires cooperative work and live-state bounds.
- 검토한 주요 대안: Keep all string sets gated, admit only parse-negative tests, raise the runtime budget around a flat alternative ladder, add an unbounded backtracking route, or canonicalize sets and lower a bounded shared trie.
- 선택한 방식: Preserve and deduplicate canonical sequences, charge static tables before cloning, track specification MayContainStrings through algebra, partition multi/single/empty matching order, reverse lookbehind sequences, merge common prefixes and exact interned suffix subtrees, and enforce parser, emission, work, and live-state limits.
- 다른 대안 대신 이 방식을 선택한 이유: Partial admission would leave core v semantics absent; a larger flat budget scales with the Unicode table rather than matched input; unbounded backtracking violates the sandbox contract. Canonical algebra plus trie/DAG lowering expresses the specification while reducing branch work structurally.
- 장점, 단점 및 영향: All seven Unicode 17 string properties, q strings, iv algebra, valid and invalid negation, lookbehind, lone-surrogate separation, and exhaustive RGI inputs pass. The implementation retains explicit conservative resource ceilings and a maintained vendored parser instead of claiming unbounded host-regex behavior.
```

### RegExp Unicode mode flag exclusivity

`u` and `v` both select Unicode-aware pattern parsing, but `v` is not an
additive extension that can be combined with `u`: the flag set containing both
is invalid. Literal scanning, parser fallback, and the `RegExp` constructor all
call `validate_regex_literal`, so exclusivity belongs in its shared flag
validator rather than in either backend compiler.

The validator first completes the existing allowed-character and duplicate
checks, then rejects a set containing both modes. This preserves deterministic
diagnostic precedence for sources such as `uvv` and `uvG`. Exact Test262
admission contains only the parse-negative literal and constructor files; the
rest of the broad `regexp-v-flag` feature remains governed by its own bounded
admissions.

```text
[Decision Log]
- 목적과 의도: Reject the statically invalid simultaneous u and v RegExp modes at the shared specification boundary for literals and constructors.
- 기존 구현 및 제약 조건: Both modes were accepted independently, all source paths already converge on one validator, and Test262 labels this two-file rule with the same broad feature used by unsupported Unicode-set syntax.
- 검토한 주요 대안: Let the backend choose a mode, reject only literals, reject only constructor flags, special-case the two tests, or add one shared post-scan exclusivity invariant with exact admission.
- 선택한 방식: Preserve invalid and duplicate scans, reject seen u plus v afterward, exercise both orders and mixed flags directly, and remove regexp-v-flag only for two frozen paths.
- 다른 대안 대신 이 방식을 선택한 이유: Backend selection occurs too late for parse-negative literals, path-local checks can diverge, and broad feature admission would run unsupported set syntax. The common validator already owns all flag-set early errors.
- 장점, 단점 및 영향: Literal and constructor behavior become consistent with no matcher or runtime cost. Two skips become passes; the admission manifest and tooling guard are additional maintenance, while all remaining v behavior stays independently gated.
```

### Exact Unicode ignore-case word boundaries

ECMAScript WordCharacters contains ASCII letters, digits, and underscore,
plus long s and Kelvin sign when Unicode mode and ignore-case are both active.
Rust's Unicode `\b`/`\B` instead treats many additional letters and digits as
word characters. Previously, a linear pattern used that broader boundary
while an otherwise equivalent lookaround or backreference pattern used the
exact fancy lowering. Quantified-capture hybrids also assumed both backends
selected the same start, so the mismatch could produce either a false negative
or an internal disagreement error.

The vendored `regex-syntax` HIR has dedicated positive and negative ECMAScript
Unicode-ignore-case look variants. `regex-automata` lowers them to native NFA
look states whose word predicate is ASCII alphanumeric or underscore plus long
s and Kelvin sign. The PikeVM evaluates those assertions without synthesized
lookaround branches. Fancy's hard VM uses equivalent native assertion variants,
so regular and hard routes share the same character relation.

ECMAScript repeated-capture clearing is also represented inside the regular
NFA. Before each non-nullable repeated iteration, Thompson compilation emits
transactional `CaptureClear` states for descendant groups; PikeVM restores
slots while exploring alternatives. Nullable repeated captures remain on
Fancy's hard `RepeatMatcher` route because ECMAScript's greedy nullable-repeat
choice cannot be reproduced by ordinary Pike priority alone.

`PrefilteredExact` gives its matchers narrow authority. A relaxed Rust pattern
with affected assertions erased is a language superset and may only reject an
impossible match. When the input contains no scalar whose Rust/ECMAScript word
classification differs, bulk iteration may use the original Rust boundary
matcher. Per-position APIs avoid rescanning the complete input for that proof;
they use the relaxed gate and native exact matcher, preventing quadratic global
`exec` iteration. Patterns with repeated captures additionally use a
capture-erased exact linear matcher
to choose a language-valid start; the exact matcher then runs at that position
to recover authoritative group-zero bounds and captures. This distinction is
required because removing captures can move a nullable repeat from Fancy's
`RepeatMatcher` route to PikeVM and change its preferred end.

```text
source pattern
  -> relaxed Rust superset: reject impossible input only
  -> class-agreement input: original Rust boundary fast path
  -> repeated captures: capture-erased exact PikeVM start gate
  -> exact-position matcher: recover bounds/captures without later scanning
```

`find_at_pos` and `captures_at_pos` set an exact-position VM option while
retaining the complete haystack, rather than compiling a second program or
matching a sliced suffix. Sticky matching therefore
cannot scan to a later start and lookbehind/boundary assertions still observe
preceding input. Exact iteration advances an empty match from its actual byte
position by one internal character. Non-global replacement asks for only the
first capture set. Deterministic unanchored scanning is excluded from Fancy's
speculative work charge, so a million-scalar no-match search remains linear;
branching and RepeatMatcher work stay bounded.

```text
[Decision Log]
- 목적과 의도: Make Unicode ignore-case word boundaries backend-independent while preserving exact captures, iteration progress, and linear rejection of hostile no-match inputs.
- 기존 구현 및 제약 조건: Rust boundaries use a broader Unicode word set, synthesized exact lookarounds consumed branch work, sticky correction could scan later starts, Pike captures retained stale repeated alternatives, and nullable repeats require ECMAScript RepeatMatcher priority.
- 검토한 주요 대안: Accept Rust semantics, route every pattern through the hard VM, retain synthesized lookarounds, trust a relaxed candidate start, post-process captures, or add exact assertion and capture-clear states to the maintained regex stack.
- 선택한 방식: Add custom HIR/NFA and hard-VM boundary assertions; add transactional PikeVM CaptureClear states for non-nullable repeats; retain nullable repeats in Fancy; use relaxed, class-agreement, and capture-erased exact linear gates; and expose full-haystack exact-position APIs.
- 다른 대안 대신 이 방식을 선택한 이유: Rust semantics violate ECMA-262, all-hard routing makes benign scans consume speculative work, synthesized assertions amplify branching, relaxed starts are unsound, and post-processing cannot reconstruct captures observed during matching. Native states preserve linear matching where its priority is sufficient.
- 장점, 단점 및 영향: Boundary semantics, sticky location, repeated captures, replacement laziness, and UTF-8 empty iteration now agree across routes. Common non-nullable patterns stay linear and hostile probes terminate predictably. Three maintained path forks increase update work; nullable repeated captures and genuinely hard constructs still use the bounded backtracking VM, and complex iv classes containing w/W remain separate set-algebra work.
```

`MakeClosure` follows the same rule for an ordinary function's fresh
`.prototype`: the prototype is pinned before a named-function environment or
the function object can allocate, then released only after the function owns
it. Allocation failures restore both the temporary environment pin and the
prototype pin, so a forced collection cannot reuse the prototype's heap slot.

Native Error materialization is also a collecting boundary. It first pins the
selected Realm intrinsic prototype and performs the ordinary rooted GC retry.
If every capped cell is still live, the VM returns an immutable, preallocated
`RangeError("heap limit exceeded")` owned by that Realm. The reserve is created
during Realm intrinsic setup, counts as an ordinary live heap object, and is a
permanent GC root together with the Realm's intrinsic Error prototypes. It
therefore settles an already-created Promise without allocating past the
sandbox limit or borrowing another Realm's Error identity.

The reserve is non-extensible and its `name`, `message`, and `stack` properties
are non-writable and non-configurable. Repeated fully saturated failures in the
same Realm intentionally share object identity; a fresh Error is still used
whenever GC reclaims a normal cell. Explicit JavaScript thrown values bypass
materialization, and non-catchable Fuel exhaustion never uses the reserve.
Because Error materialization can now collect, every caller must pin any heap
value held only in a Rust local that remains needed after the call.

```text
[Decision Log]
- 목적과 의도: Keep existing Promise and dynamic-import capabilities settleable at an exact object cap without weakening the cap.
- 기존 구현 및 제약 조건: A catchable heap failure needed one more GC cell for its JavaScript Error object; allocation failure propagated to the host after the Promise resolver or job had already been consumed.
- 검토한 주요 대안: Allow a bounded number of over-cap cells; defer settlement until a later GC; use one VM-wide fallback; preallocate one fallback per Realm.
- 선택한 방식: Try one rooted GC allocation, then return an immutable preallocated RangeError from the operation Realm.
- 다른 대안 대신 이 방식을 선택한 이유: Over-cap cells violate the host contract, deferred retries cannot guarantee a free cell and change job ordering, and a VM-wide object violates Realm identity.
- 장점, 단점 및 영향: The live-object ceiling remains exact and repeated failures always settle existing Promises; one cell is reserved per Realm and saturated failures expose shared identity as a documented host-limit deviation.
```

## Transactional test262 Realm construction

The test262 host builds a Realm by publishing intrinsic objects into per-Realm
VM registries as setup progresses. Those provisional entries are intentional:
later installers consult earlier intrinsic identities, and the same maps keep
the growing object graph alive across collecting allocations. They do not,
however, make a partially initialized Realm observable to JavaScript.

Realm creation therefore uses one transaction from fresh environment
allocation through final wrapper attachment. It records the incoming
`gc_pins` depth, pins the environment before any object can reach it, populates
the intrinsic graph, allocates the host wrapper, and attaches the Realm global.
A successful commit releases only the transaction's pins. Any error first
truncates the complete transaction-owned pin suffix and then removes that
environment's entries from all 35 rooted per-Realm registry families and the
non-rooting `%Object.prototype%` reverse identity index. Native error
materialization runs afterward in the calling Realm, so its collecting retry
can reclaim the abandoned graph.

Rollback does not rewind the heap allocator, inline cache, GC counters, fuel,
or finalization queue. A cap-triggered collection may legitimately clear the
cache or enqueue cleanup for pre-existing registries; restoring either would
reintroduce stale heap indices or lose required jobs. Realm setup itself does
not publish module records, template objects, generated symbols, or Promise
jobs, so registry roots and the transaction pin suffix are the complete
logical rollback surface.

```text
[Decision Log]
- 목적과 의도: Make failed test262 Realm construction leave no inaccessible GC roots while preserving exact heap-cap and error-Realm behavior.
- 기존 구현 및 제약 조건: Intrinsic installers publish 35 families of Realm roots incrementally and use fallible LIFO temporary pins; wrapper allocation remains fallible after every registry has been populated.
- 검토한 주요 대안: Publish nothing until setup completes; clean only the last inserted map; make every installer independently error-safe; or own all provisional roots and pins in one outer transaction.
- 선택한 방식: Keep provisional registry publication, pin the fresh environment, capture the incoming pin depth, include wrapper attachment in the transaction, truncate the owned pin suffix on every result, and remove every Realm registry entry on error.
- 다른 대안 대신 이 방식을 선택한 이유: Later installers require earlier intrinsic identities, map-specific cleanup misses other roots, and duplicating rollback in every installer creates drift. One lexical owner matches the actual observability boundary.
- 장점, 단점 및 영향: Every hard-cap failure point is reusable and collectible before caller-Realm error materialization. The registry inventory remains manually synchronized across the VM fields, root tracer, rollback helper, identity indexes, and regression counter, so new Realm registries must update every applicable site.
```

## Realm Object prototype identity

Every Realm's original `%Object.prototype%` uses the Immutable Prototype
Exotic Object `[[SetPrototypeOf]]` behavior. A request for its current `null`
prototype succeeds, while a different prototype returns `false` without
changing the object. Public `Object.setPrototypeOf` and the legacy
`__proto__` setter convert that status into a TypeError from the invoked
method's Realm; `Reflect.setPrototypeOf` returns the boolean. Proxy dispatch
still occurs before target handling, so transparent Proxies delegate and a
truthy trap over an extensible target may report success without mutating the
target. The ordinary same-prototype check remains before the immutable check.

`realm_object_prototypes` is the authoritative environment-to-intrinsic map
and GC root. `realm_object_prototype_ids` is a non-rooting reverse `HashSet`
used only for expected O(1) identity checks. Main and created Realms publish
both entries through one registration helper. Failed Realm construction
removes the owning map entry and then unconditionally removes the reverse
identity before the heap slot can be reused. The removal result is stored
before `debug_assert!`; placing the mutation inside the assertion would erase
it from release builds. The registry rollback counter includes both
collections, and CI executes the full heap-boundary rollback sweep in release
mode to preserve that guarantee.

```text
[Decision Log]
- 목적과 의도: Apply immutable-prototype semantics to the original Object prototype of every Realm without slowing unrelated prototype mutations or retaining stale heap identities.
- 기존 구현 및 제약 조건: SetPrototypeOf recognized only the main VM Object prototype. The environment-keyed Realm map already owned every intrinsic as a GC root, but scanning it would make each ordinary mutation O(number of Realms), and GcIdx slots can be reused after failed Realm construction.
- 검토한 주요 대안: Keep the main-only special case; scan all Realm map values; add a flag to every generic object allocation; or maintain a non-rooting reverse identity set beside the authoritative map.
- 선택한 방식: Register each intrinsic in the rooted map and an O(1) reverse HashSet through one helper, consult that set after same-prototype equality, and remove its entry unconditionally during transactional Realm rollback.
- 다른 대안 대신 이 방식을 선택한 이유: Main-only behavior violates Realm semantics, a linear scan creates attacker-controlled work, and an object-layout flag would widen dozens of unrelated allocation paths. The reverse index reuses the existing lifecycle boundary with a small, auditable surface.
- 장점, 단점 및 영향: All Realms now preserve the required null prototype through direct, borrowed, Proxy, and post-GC calls with constant expected lookup cost. The map and reverse index must remain synchronized; release rollback CI and the 32-collection counter guard against stale identities after slot reuse.
```

Observable materializers follow the same ownership rule. When an abstract
operation reads heap values into a Rust collection and a later getter, proxy
trap, coercion, call, or construction can re-enter JavaScript, every value is
pinned immediately after it is read. The caller owns those pins through the
last re-entrant operation and releases them on both normal and abrupt
completion. A Rust `Vec<Value>` is storage, not a GC root.

`src/builtins/call_arguments.rs` centralizes that contract for
`CreateListFromArrayLike`. `Reflect.apply`, `Reflect.construct`, and
`Function.prototype.apply` therefore share the same observable `length`,
`ToLength`, indexed `Get`, resource-cap, and pin-cleanup behavior instead of
maintaining array-specific shortcuts. `Function.prototype.apply` handles its
specified omitted, `null`, and `undefined` no-argument cases before entering
the shared object-only operation. The materialized list and its pin count move
together into the final call so a later getter or target re-entry cannot make
an earlier argument collectible.

## Native Array callback result ownership

`Array.prototype.map` and `flatMap` use live generic indexed traversal. Each
current source value is pinned across its callback, and each fresh mapped
result remains pinned until the destination owns it. `flatMap` additionally
retains mapped container roots while copying their elements, so tracing the
container also preserves nested values.

`Array.of` follows the same rule across a different observable boundary. Its
arguments and constructor remain rooted through construction, and the returned
object is pinned before the first `defineProperty` trap. The result stays live
through every element definition and the final `length` set. All three methods
run their fallible body inside a completion scope and release one LIFO pin
suffix afterward, so callback throws, Proxy trap errors, and result-allocation
failures restore the incoming pin depth.

```text
[Decision Log]
- 목적과 의도: Prevent native Array methods from exposing collected-and-reused heap slots when JavaScript re-enters the VM during callback or property-definition work.
- 기존 구현 및 제약 조건: GcIdx is a generation-free cell index, the collector sees only VM roots, and map/flatMap callback results plus the Array.of result lived only in Rust locals across observable calls. Rust locals do not participate in tracing.
- 검토한 주요 대안: Make all Rust Value locals implicit roots, add generations to every GcIdx, disable collection during native built-ins, pin only the final value, or explicitly own temporary roots at each observable boundary.
- 선택한 방식: Pin source/argument values before re-entry, pin each fresh callback or constructed result immediately, retain those roots until the destination owns the values, and release them from one success/error cleanup scope.
- 다른 대안 대신 이 방식을 선택한 이유: Global implicit rooting or generation handles require a VM-wide representation change, disabling GC weakens the sandbox, and late pinning occurs after a slot may already be reused. Explicit lexical ownership matches the existing gc_pins contract and keeps this correction bounded.
- 장점, 단점 및 영향: Forced GC can no longer turn prior map/flatMap results or a custom Array.of result into another HeapObj; abrupt and exact-cap failures leave no pin leak. Root storage grows with live object-valued inputs/results for the duration of the operation, while snapshot-based methods with broader observable-semantics issues remain separate follow-up work.
```

## Native Array SortIndexedProperties and writeback

`Array.prototype.sort` and `toSorted` use a shared stable merge sort whose
comparison closure returns a VM completion. This is necessary because both
custom comparator calls and default `ToString` conversion can execute
JavaScript. Materialized values, the receiver, and the comparator remain
explicit temporary roots for the complete operation; a custom comparator
result is additionally pinned across `ToNumber`. The first abrupt completion
stops merging immediately and the outer completion scope releases the entire
LIFO root suffix.

Both methods validate the comparator, apply `ToObject`, and cache
`LengthOfArrayLike` once. A shared `SortIndexedProperties` collector then
performs ascending, live VM property operations. `sort` uses the skip-holes
mode, issuing `HasProperty` and then `Get` only for present own or inherited
indices. `toSorted` uses the read-through-holes mode and issues `Get` for every
captured index, so holes and missing properties become explicit `undefined`
entries. Collection finishes before comparison starts, and getter, Proxy,
conversion, comparator, setter, or deletion errors stop the algorithm at the
first observable failure.

The common comparison path orders `undefined` after every defined value
without invoking a custom comparator. Default comparison converts each value
and compares RuJa's decoded UTF-16 code units rather than Rust scalar or UTF-8
order, preserving lone-surrogate sentinels. After sorting, `sort` writes the
materialized list through ascending strict indexed `[[Set]]` and deletes the
unused captured range in ascending order. Missing Array elements use the
generic receiver-aware setter path, so inherited ordinary descriptors and
Proxy `set` traps run before receiver length or extensibility is considered.
Existing own dense elements retain their metadata-synchronizing fast path.

`toSorted` has different ownership and hole behavior. It creates and pins its
fresh Array before the first indexed read, as required by `ArrayCreate`.
After successful sorting it installs every value as an own, present result
index without consulting inherited setters. Allocation failure therefore
occurs before source access or comparator side effects, and comparator failure
leaves the unreachable destination available for later collection without
leaking a pin.

Collection, merge comparisons, writeback, and deletion consume execution fuel.
The sandbox rejects captured lengths above `MAX_DENSE_ARRAY_LEN` before any
indexed scan. For `toSorted`, this policy check occurs after `ArrayCreate` so
the specified invalid-Array-length error order remains observable.

```text
[Decision Log]
- 목적과 의도: Implement the complete observable SortIndexedProperties boundary for Array sort and toSorted while preserving GC ownership, stable comparison, Array metadata, and sandbox resource limits.
- 기존 구현 및 제약 조건: The first hardening pass was correct only for direct dense Arrays. It bypassed ToObject and LengthOfArrayLike, inherited indices, accessors, Proxy Has/Get, and generic receiver writeback. Array's missing-index fast path also skipped Proxy prototypes, and Rust-owned Values remain invisible to the collector unless explicitly pinned.
- 검토한 주요 대안: Keep separate dense and generic algorithms, snapshot own keys before sorting, route every Array index write through the generic setter, remove the hard length cap and rely only on optional fuel, or share one mode-driven collector plus targeted fast paths.
- 선택한 방식: Cache the receiver and length once, collect with live ascending VM property operations under explicit skip-holes/read-through-holes modes, root every retained value immediately, preserve the shared completion-returning merge sort, use generic strict Set/Delete for sort, and keep direct destination installation for fresh toSorted Arrays.
- 다른 대안 대신 이 방식을 선택한 이유: Separate algorithms would drift in comparison and cleanup order, own-key snapshots miss live HasProperty/Get effects, routing existing dense writes through the generic path would discard their metadata optimization, and optional fuel alone does not bound an unmetered host. One collector exposes the specification distinction while retaining narrow exotic-object ownership points.
- 장점, 단점 및 영향: Generic objects, boxed primitives, inherited accessors, Proxy traps, getter mutation, partial writeback, forced GC, and fuel aborts now follow one tested order. The focused sort/toSorted Test262 set has no failures. The explicit 1,048,576 scan cap is intentionally stricter than ECMAScript for very large sparse receivers, temporary root and merge storage scale with the collected list, and default comparison still allocates UTF-16 vectors.
```

## Iterative Proxy deletion and nested traversal fuel

`Vm::delete_property_key` owns one root for its original receiver and advances
an owned `current` value through transparent Proxy targets. Each iteration
checks revocation, consumes one fuel unit, pins that layer's target and handler,
and performs observable `GetMethod(handler, "deleteProperty")`. A nullish trap
releases the per-layer pins and continues; a present trap is pinned through its
call, then the target remains rooted through descriptor and extensibility
invariants. One outer completion scope releases the original root on every
normal, thrown, allocation, or host-fuel completion. Ordinary exotic deletion
runs once after the loop reaches the final non-Proxy target.

Valid finite Proxy target chains have no specification depth bound. Proxy
targets are fixed at construction, so forwarding does not need a visited set or
an arbitrary host limit. Hosts that need a work bound opt into fuel. That bound
must include nested abstract operations: a shallow delete can read its trap
through a deep Proxy handler and can validate a truthy result through deep
`[[GetOwnProperty]]` and `[[IsExtensible]]` target chains. The shared iterative
`[[Get]]`, descriptor, and extensibility loops therefore charge each Proxy
layer as well. With no fuel budget they remain stack-safe and unbounded by
design.

```text
[Decision Log]
- 목적과 의도: Remove native-stack dependence from Proxy deletion while preserving exact trap order, GC lifetime, invariant validation, and an optional host work bound.
- 기존 구현 및 제약 조건: Trapless delete forwarding recursively called delete_property_key; Rust Value locals are not GC roots; and nested Proxy handler Get, target descriptor, and extensibility loops could bypass a fuel charge placed only on the outer delete wrapper.
- 검토한 주요 대안: Keep recursion, impose a fixed depth cap, track visited targets, accumulate every transparent layer, or advance one rooted current target iteratively while metering every nested Proxy operation.
- 선택한 방식: Pin the original receiver, iterate one Proxy layer at a time with constant per-hop target/handler/trap pins, preserve the first real trap's descriptor and extensibility checks, and consume fuel in Delete plus shared Get, GetOwnProperty, and IsExtensible traversal.
- 다른 대안 대신 이 방식을 선택한 이유: Recursion can abort the host, a cap rejects valid programs, target links are immutable and do not require cycle detection, and accumulated layers add unnecessary memory. A rooted current value directly models transparent forwarding while shared fuel closes nested-operation bypasses.
- 장점, 단점 및 영향: A 100,000-layer delete is stack-safe, forced GC and every abrupt path restore pin depth, and hosts can stop deep handlers or invariant targets with fuel. Unbounded hosts can still spend linear time on arbitrarily deep legal chains. Set and receiver-side DefineOwnProperty use the later iterative state machine, and the coordinated ordinary Get, HasProperty, and Set audit below removes their former caps.
```

## Reflect omitted property-key normalization

Every property-key-taking Reflect entry point validates its target before key
conversion. `Reflect.get`, `Reflect.set`, `Reflect.has`, and
`Reflect.deleteProperty` then call one shared helper that reads argument slot
one with `undefined` as its default and performs `ToPropertyKey`. An absent
slot is therefore semantically identical to an explicitly supplied
`undefined`; it is not a signal to return early. Receiver and value defaults
remain method-specific and are applied only after key conversion.

The native call boundary pins its callee, argument list, and receiver for the
complete call. Once conversion reaches an internal property operation, the
Proxy get, set, and has paths own their target, handler, trap, receiver, and
value roots across observable re-entry. The common key helper adds no new GC
or fuel ownership: it removes three early exits and routes omitted arguments
through the already-audited explicit-`undefined` paths. Forced collection in
normal and throwing traps verifies that those existing ownership boundaries
also hold for omitted keys.

```text
[Decision Log]
- 목적과 의도: Make omitted Reflect property keys obey the same ECMAScript conversion, receiver, Proxy, and abrupt-completion semantics as explicit undefined keys.
- 기존 구현 및 제약 조건: Reflect.get, Reflect.set, and Reflect.has each had a private missing-slot early return, while deleteProperty already performed ToPropertyKey(undefined); upstream Test262 does not distinguish these cases.
- 검토한 주요 대안: Keep per-method branches and patch three return values, duplicate the correct deleteProperty expression, or centralize only the argument-slot default and conversion while retaining each method's internal operation.
- 선택한 방식: Validate each target first, call one shared slot-one ToPropertyKey helper, and then apply each method's existing receiver, value, and internal-method behavior.
- 다른 대안 대신 이 방식을 선택한 이유: No return value can emulate an accessor, property creation, Proxy trap, revocation, or thrown completion. Sharing only conversion prevents future omission drift without merging semantically different get, set, has, and delete operations.
- 장점, 단점 및 영향: Omitted and explicit undefined keys now agree through ordinary and Proxy paths, local GC and abrupt regressions provide coverage missing from Test262, and the change adds no allocation or fuel policy. The later coordinated property-traversal state machine removes the deep Get, Has, and Set caps while preserving this coercion order.
```

## Realm-local Reflect intrinsic allocation

`build_reflect_in_env` receives an explicit global environment and
`%Object.prototype%`. The main-Realm wrapper supplies the VM globals, while
Test262 Realm population calls it only after that Realm's `%Function.prototype%`,
`%Object.prototype%`, global object, and registry roots exist. The resulting
namespace and all 13 methods are therefore distinct per Realm, and native calls
derive their function prototype and error Realm from the supplied environment.
The namespace owns the observable `Symbol.toStringTag` descriptor instead of
depending on its internal diagnostic class name.

Realm creation may run under a hard heap-object cap. The ordinary native
function registration helper intentionally remains a non-collecting bootstrap
allocator because older batch installers do not all root provisional values.
Reflect uses a separate GC-retrying entry point: after each method allocation,
the method is pushed onto `gc_pins` before another allocation can collect. The
final namespace object is allocated through the sandbox allocator while all
methods remain pinned, and one cleanup path removes the complete pin suffix on
success or failure. Once allocation succeeds, the namespace owns the methods
before those temporary pins are released and the caller publishes it globally.

The allocation regression gives the batch exactly 14 slots and varies
unreachable garbage so collection occurs before method 1, method 8, or the
final namespace allocation. It then collects again and verifies all methods
remain distinct callable functions with their exact names. A 13-slot failure
case verifies that no provisional method or pin survives rollback.

```text
[Decision Log]
- 목적과 의도: Install a specification-shaped Reflect namespace in every Realm without allowing heap-cap collection to reclaim provisional methods or reject while reclaimable capacity exists.
- 기존 구현 및 제약 조건: Reflect existed only in the main Realm, lacked @@toStringTag, and accumulated method handles in an untraced Rust map through raw non-retrying allocations. Globally changing native registration to collect would expose unpinned provisional values in older intrinsic builders.
- 검토한 주요 대안: Share the main Reflect object, install only the missing tag, force one unconditional GC before the batch, change every native-function allocation globally, or add one rooted GC-retrying runtime-installer path.
- 선택한 방식: Parameterize Reflect by Realm, allocate each method through the dedicated retrying path, pin it before the next allocation, allocate the namespace through Vm::alloc, and release all temporary pins through one result boundary.
- 다른 대안 대신 이 방식을 선택한 이유: Shared or main-only objects violate Realm identity; a pre-batch collection cannot protect future collecting allocations; and changing the global bootstrap helper without auditing every caller can publish stale GcIdx values. The narrow helper makes the new runtime path correct without silently widening GC behavior elsewhere.
- 장점, 단점 및 영향: Main and created Realms now have correct object/function/error provenance and exact-cap behavior, and direct Reflect Test262 closes at 153/153. The VM retains two native-function allocation paths until older intrinsic batches receive the same rooting audit, and separate Reflect internal-method defects remain explicit follow-up work.
```

Promise keyed combinators use a separate two-stage observable protocol. They
first snapshot raw `[[OwnPropertyKeys]]`, including non-enumerable keys, and
then perform Proxy-aware `[[GetOwnProperty]]` inside the per-key loop. An
undefined or non-enumerable descriptor skips that key before `Get`,
`C.resolve`, state allocation, or index advancement. Accepted keys therefore
form a compact result while descriptor traps remain interleaved with that
key's `Get`, resolve call, `then` lookup, and `then` invocation. Pre-filtering
all descriptors during key enumeration is forbidden because it changes
observable Proxy order and bypasses a delegating Proxy's descriptor trap.

Every accepted keyed entry pins its property value through `C.resolve`, then
keeps the resulting promise, shared state, element callbacks, and observable
`then` value rooted until invocation completes. Those roots are released in
LIFO order on both success and rejection, while skipped keys allocate no entry
state. This keeps the specification's operation order and the collector's
manual `gc_pins` ownership discipline aligned at each re-entry boundary.

## Promise and async jobs

Deferred jobs own the Realm selected when the job is created. Dynamic import
records the initiating Realm, thenable jobs record the callable `then` Realm,
and Promise reaction jobs record the selected handler Realm before later Proxy
revocation or unrelated re-entry can change how an error is constructed. Job
payloads and Promise continuations trace these Realm environments together
with their capabilities, handlers, promises, and generator references.

Handler Realm selection treats catchable `GetFunctionRealm` failures, such as
a revoked callable Proxy, as the specification's current-Realm fallback while
propagating non-catchable host aborts. Pending Promise settlement precomputes
every selected handler Realm before changing state or draining handlers. If
Fuel ends that preflight, the Promise, handler list, resolving one-shot flag,
and FIFO queue remain retryable. Intrinsic resolving functions claim their
one-shot state and enqueue only the unfinished direct Resolve/Reject stage, so
external jobs, reactions, async state machines, and thenables that already ran
are not replayed. Staged settlement runs before later external jobs, while a
direct Resolve/Reject stage that aborts again is pushed back to the front.
Direct staged settlement roots the resolver operation Realm for later handler
fallback. Promise resolution that completed observable `Get(resolution,
"then")` retains that Realm together with the resolution, observed `then`, and
claimed one-shot state until `GetFunctionRealm(then)` selects the thenable-job
Realm. Nested resolving functions and allocation-error materialization then use
that selected job Realm. Retry therefore resumes after the Get without invoking
the getter again or confusing the resolver and job Realms.
An arbitrary species-provided capability function is invoked once and never
automatically replayed after Fuel, because replay could duplicate unknown user
effects.

`promise_rejection_reason_in_realm` is the common completion boundary. It
preserves an explicit thrown JavaScript value, materializes a catchable native
error in the operation Realm, and returns non-catchable Fuel exhaustion to the
host unchanged. Promise construction, resolution, reactions, await,
`Array.fromAsync`, Async-from-Sync iteration, async iterator disposal, and
async generators use that classification before converting a completion into
a rejection. Values that exist only in Rust locals are pinned before error or
replacement-capability allocation.

Host aborts also unwind the owning state machine. Initial and resumed async
functions remove suspended frames and restore the operand stack; a module
continuation marks only its own cached record errored; unrelated pending module
jobs remain resumable. An async generator marks the active request aborted,
releases queue ownership, and schedules a rooted drain for queued siblings.
Only a terminal `next()` result is retryable after a catchable allocation
failure. Other errors may occur after bytecode state advanced, so replaying the
original request is forbidden.

## Execution contexts and Realms

`Vm::execution_contexts` is the authoritative LIFO record of active JavaScript
calls and resumptions. It is intentionally separate from the bytecode frame
stack: a native builtin can call interpreted code before a frame exists, and a
suspended generator or async function can later resume beneath an unrelated
native caller. Every context owns its callee Realm environment and callee;
native contexts additionally own `NewTarget` and either an already-observed
`NewTarget.prototype` value or its already-resolved fallback Realm.

Interpreted dispatch pushes a setup context before class validation, sloppy
`this` conversion, and arguments/rest allocation so those pre-frame operations
use the callee Realm. The interpreter then pushes a frame context while
bytecode runs. Keeping both entries is deliberate: the setup context covers
the interval before frame creation, while the frame context makes later
generator and async resumptions independent of whichever native method resumed
them.

General Realm lookup uses only the top execution context, then falls back to
the active frame and VM global. Native callee and construction accessors also
read only a top native context; searching downward would leak an outer native
call into an active interpreted call. Interpreted error lookup accepts a top
interpreted context and otherwise falls back to the active frame, preserving a
suspended function's Realm when a borrowed native `next`, `throw`, or `return`
method resumes it. Catchable errors are materialized before the owning context
is popped.

The GC traces every execution context's Realm environment, callee,
`NewTarget`, and cached prototype. Rust scope cleanup restores the previous
context depth on all normal and `Result`-based abrupt paths; unwinding and then
reusing a VM after a caught Rust panic remains outside the engine's supported
recovery contract.

## Exotic extensibility and Proxy preventExtensions

Every observable stateful `HeapObj` variant owns an atomic extensibility bit.
This includes collection objects, collection and RegExp String iterators,
WeakRef and FinalizationRegistry objects, Promises, sync and async generators,
TypedArrays, ArrayBuffers (shared or ordinary), and DataViews. Module namespace
objects remain intrinsically non-extensible. `HeapObj::is_extensible` and
`HeapObj::prevent_extensions` are the exhaustive storage boundary, so adding a
new public object kind requires an explicit extensibility decision instead of
silently inheriting `true`.

`Vm::prevent_extensions` is an iterative Proxy internal-method state machine.
It pins the original receiver for the complete operation, then checks
revocation, consumes one fuel unit, and pins each target, handler, and present
trap across observable lookup and call. A missing trap advances to the target
without recursion. A false trap result returns false. A truthy result invokes
the target's complete `[[IsExtensible]]`, including nested Proxy traps and
their errors, before enforcing the invariant. Reaching a non-Proxy target
updates that variant's atomic state. One cleanup boundary restores the incoming
pin depth after normal, thrown, allocation, or host-fuel completion.

Changing exotic extensibility exposed a pre-existing integrity-level shortcut:
`Object.seal`, `freeze`, `isSealed`, and `isFrozen` had treated unknown exotic
variants as if non-extensibility alone proved their descriptors immutable.
Non-specialized exotics now use the shared `SetIntegrityLevel` and
`TestIntegrityLevel` paths. Operation targets stay pinned, temporary descriptor
objects allocate through GC-retrying `Vm::alloc`, and exact-cap tests prove two
temporary cells can be reclaimed and reused across every own key. Map
collection entries are internal collection data, not object own keys; removing
them from ordinary own-key enumeration is required for sealing a non-empty Map.
The existing specialized ordinary-object, Array, Function, and Iterator Helper
paths remain because they materialize or mutate their descriptor storage
directly.

```text
[Decision Log]
- 목적과 의도: Make every observable object obey persistent extensibility, preserve Proxy preventExtensions ordering and invariants at arbitrary legal depth, and prevent seal/freeze from reporting integrity that descriptors do not have.
- 기존 구현 및 제약 조건: Several exotic variants had no extensibility state; transparent Proxy forwarding recursed without fuel or explicit roots; truthy traps inspected nested Proxy storage directly; and integrity predicates treated unrecognized non-extensible exotics as sealed or frozen.
- 검토한 주요 대안: Store one extensibility flag on every GC cell, keep a side table keyed by GcIdx, add flags only to the initially reported variants, raise a Proxy depth limit, or give each public variant explicit state and reuse the complete integrity/internal-method paths.
- 선택한 방식: Keep per-variant atomic state behind exhaustive HeapObj helpers, walk Proxy targets iteratively with constant per-layer roots and fuel, validate truthy results through full IsExtensible, and route non-specialized exotics through rooted GC-retrying SetIntegrityLevel and TestIntegrityLevel.
- 다른 대안 대신 이 방식을 선택한 이유: Cell-wide metadata would also describe internal Environment and Iterator records and complicate slot reuse; a side table risks stale GcIdx identity; a partial variant list recreates the bug when new exotics appear; and fixed depth limits reject valid programs. Existing complete internal-method helpers preserve observable order and typed-array or module-namespace behavior.
- 장점, 단점 및 영향: Deep transparent chains are stack-safe and host-bounded, nested traps and Realm errors remain observable in order, every current exotic blocks new properties after prevention, and integrity operations process real descriptors under heap caps. The cost is one atomic field per stateful variant plus duplicated constructor initialization. Receiver-side DefineOwnProperty and the remaining ordinary property traversals were completed by the later Set and coordinated traversal state machines.
```

## Iterative prototype internal methods

`Vm::get_prototype_of` implements the Proxy `[[GetPrototypeOf]]` algorithm as
an iterative state machine. The original receiver remains pinned for the whole
operation. Each Proxy layer is checked for revocation before consuming one fuel
unit, and its target and handler are pinned across observable trap lookup and
invocation. A missing trap advances to the target without native recursion. A
present trap must return an object or `null`; that result remains rooted while
the target's complete nested `[[IsExtensible]]` operation runs.

When a trapped target is non-extensible, the state machine records and pins
the expected prototype, then continues into the target. Once an ordinary
prototype or an extensible trapped result is reached, deferred expectations
are checked from the innermost Proxy outward. This is the iterative equivalent
of returning through nested internal-method calls: an inner mismatch or abrupt
completion prevents any outer validation from completing. A single cleanup
boundary releases every deferred root and restores the incoming pin depth on
normal, TypeError, user-throw, GC, and host-fuel exits.

`Vm::set_prototype_of` uses the same forwarding discipline. Both the original
receiver and proposed prototype are pinned before any handler lookup can run
GC. False trap results return immediately; truthy results inspect the target's
full nested `[[IsExtensible]]` and, when required, `[[GetPrototypeOf]]` before
enforcing the invariant. Missing traps advance iteratively until the ordinary
target or another observable trap decides the result. This preserves the
specified revocation, `GetMethod`, call, boolean conversion, and invariant
order from the ECMAScript Proxy algorithms.

Ordinary `[[SetPrototypeOf]]` no longer stops cycle detection after 4096
objects. `prototype_chain_blocks_set` follows raw ordinary prototype slots,
charges one fuel unit per visited candidate, and stops when it reaches `null`
or an object such as Proxy whose `[[GetPrototypeOf]]` method is non-ordinary.
That stop is required by `OrdinarySetPrototypeOf`; invoking a Proxy trap here
would incorrectly reject the specified Proxy-shadowed cycle exception. Brent
checkpoints detect an impossible pre-existing all-ordinary cycle with constant
native memory, avoiding both an infinite default-fuel walk and a growing set of
reusable `GcIdx` identities.

Transparent chains are O(n) time and constant state beyond active roots.
Fully trapped non-extensible nesting performs the specification-required
nested extensibility and prototype checks, so its worst case is O(n^2) work
with O(n) deferred expected prototypes; fuel bounds that work. Regressions use
100,000 transparent layers, exact N-1/N fuel boundaries, a 5000-link ordinary
cycle, nested invariant fuel, forced GC, abrupt completion, Realm-sensitive
public methods, and a WeakRef mutation test proving deferred roots are real.

```text
[Decision Log]
- 목적과 의도: Remove the native-recursion and 4096-link correctness limits from prototype internal methods while preserving every observable Proxy step, invariant, Realm error, GC root, and host resource bound.
- 기존 구현 및 제약 조건: Transparent Proxy get/setPrototypeOf forwarding recursively called the same Rust method without fuel; trapped getPrototypeOf invariants recursively re-entered the target; proposed prototypes and several observable intermediates were not owned by one cleanup scope; and ordinary cycle detection silently returned false after scanning only 4096 candidates, allowing a longer cycle.
- 검토한 주요 대안: Raise the depth cap, retain recursion behind a separate recursion guard, track ordinary candidates in a HashSet, invoke full GetPrototypeOf while checking ordinary cycles, or use iterative Proxy state machines plus constant-memory Brent checkpoints for the ordinary-only scan.
- 선택한 방식: Pin operation inputs once, consume one fuel unit and root observable values per Proxy layer, defer non-extensible getPrototypeOf expectations for reverse validation, iterate missing setPrototypeOf traps, and scan ordinary prototype slots with fuel and Brent cycle detection until null or a non-ordinary GetPrototypeOf method.
- 다른 대안 대신 이 방식을 선택한 이유: A larger cap still rejects valid programs and accepts cycles beyond its boundary; Rust recursion remains stack-dependent; a HashSet adds infallible native growth and stores reusable heap identities; and calling Proxy GetPrototypeOf during OrdinarySetPrototypeOf violates the specification's non-ordinary-method stop rule. The chosen state machines preserve exact call order and make every unbounded walk host-metered.
- 장점, 단점 및 영향: Legal transparent chains are stack-safe at arbitrary depth, ordinary cycles are rejected without a fixed limit, trap results and proposed prototypes survive observable GC, and fuel aborts leave the VM reusable with its pin depth restored. Fully trapped non-extensible chains retain specification-driven quadratic work and linear deferred storage; the VM-wide fallible native-temporary policy remains a separate architecture task.
```

## Iterative Proxy defineProperty and call dispatch

Proxy `[[DefineOwnProperty]]` now enters one shared VM state machine from both
the internal complete-descriptor path and the public `Object.defineProperty`,
`Object.defineProperties`, and `Reflect.defineProperty` paths. A dedicated
descriptor record retains every `has_*` bit, so a partial public descriptor is
not accidentally completed before Proxy compatibility checks. Ordinary
targets still use the existing partial-descriptor application path.

The state machine pins the original receiver and descriptor values for the
whole operation. At each Proxy layer it checks revocation, consumes one fuel
unit, roots the target, handler, trap, and materialized descriptor object, and
advances through a missing trap without Rust recursion. A present non-callable
trap fails before descriptor-object allocation. A false result short-circuits;
a truthy result then performs the target's complete `[[GetOwnProperty]]` and
`[[IsExtensible]]` operations before compatibility, configurable, and
writable-tightening invariants are checked. Target descriptor fields remain
rooted across every observable nested operation.

Making deep callable `defineProperty` traps stack-safe required fixing Proxy
`[[Call]]` at the same boundary. Proxy creation now stores immutable callable
and constructable metadata instead of discovering those internal methods by
recursively walking targets. The call dispatcher consumes one fuel unit per
Proxy layer and tail-transforms its current function, `this`, and arguments
for transparent targets or Proxy-valued `apply` traps. Added roots belong to
the outer call cleanup scope, so normal, thrown, allocation, and host-fuel
returns restore the incoming pin depth. `apply` argument arrays allocate in
the current execution Realm, and non-callable traps are rejected before that
allocation.

Transparent paths are linear and stack-safe at arbitrary legal depth when
fuel is unbounded; configured fuel gives hosts an exact per-layer work bound.
Specification-required nested invariant checks can still perform superlinear
work, and Proxy-valued `apply` traps can retain per-layer argument arrays until
the call resolves. Receiver-side property definition reached through ordinary
`[[Set]]` is completed by the state machine below. Revoked Proxy heap cells
still retain their target and handler strongly even though observable
operations reject them; that storage issue remains explicit follow-up work.

```text
[Decision Log]
- 목적과 의도: Make direct Proxy DefineOwnProperty and the callable traps it invokes stack-safe, fuel-bounded, GC-safe, and Realm-correct without changing observable ECMAScript order.
- 기존 구현 및 제약 조건: Transparent defineProperty forwarding and callable Proxy traps recursively re-entered Rust; public descriptors could lose partial-field presence at the wrong boundary; deep work bypassed fuel; and temporary descriptor values, trap arguments, and foreign-Realm arrays were not owned by one cleanup protocol.
- 검토한 주요 대안: Raise recursion guards, admit only shallow Test262 cases, duplicate public and internal algorithms, complete every descriptor eagerly, or use shared iterative state machines plus immutable Proxy call/construct metadata.
- 선택한 방식: Preserve partial descriptors in an explicit record, route both entry points through one rooted per-layer DefineOwnProperty state machine, store callability and constructability at Proxy creation, and tail-transform Proxy Call state while charging fuel and allocating argument arrays in the current Realm.
- 다른 대안 대신 이 방식을 선택한 이유: Larger guards still reject valid programs and remain stack-dependent; shallow admission would hide the sandbox failure; duplicated algorithms drift in trap and invariant order; eager completion changes compatibility semantics; and immutable metadata directly represents whether ProxyCreate installed the Call and Construct internal methods.
- 장점, 단점 및 영향: One implementation now covers 100,000 transparent defineProperty layers and 25,000 callable trap layers with exact fuel and pin cleanup, partial descriptors and Realm errors remain observable in order, and mutation tests prove the critical roots and allocation ordering. Receiver-side Set delegation is completed by the following state machine; revoked-slot retention and VM-wide fallible native temporary storage remain separate bounded units.
```

## Iterative Proxy Set and receiver definition

`Vm::try_set_property_key_with_receiver_tracked` owns one iterative driver for
Proxy `[[Set]]` and ordinary prototype traversal that reaches a Proxy. The
original base, value, and receiver remain pinned for the whole operation. Each
Proxy layer checks revocation, consumes one fuel unit, records the object for
cycle detection, and roots its target and handler across observable `set` trap
lookup. A missing trap advances directly to the target. A present trap is
validated before invocation, receives the original receiver, and
short-circuits on a false result before the target descriptor invariant walk.

Ordinary `[[Set]]` owns its specialized TypedArray, Array, mapped
arguments, accessor, and data-property behavior. When its prototype is a
Proxy, it returns a forwarding outcome to the outer driver instead of
recursively calling back into Proxy Set. This removes the native recursion and
the separate 128-layer Proxy guard. The coordinated traversal state machine
below later removes the former 1024-object ordinary guard.

`OrdinarySetWithOwnDescriptor` distinguishes two receiver definitions. A
missing receiver property uses the complete CreateDataProperty descriptor
`{value, writable: true, enumerable: true, configurable: true}`. An existing
writable data property uses only `{value}` so its attributes are preserved.
Both forms now retain descriptor presence bits and delegate through the shared
iterative Proxy `[[DefineOwnProperty]]` state machine. The trap descriptor
object is created in the current execution Realm and materializes only present
fields in FromPropertyDescriptor order. Reaching an ordinary target returns to
the existing TypedArray, Array length/index, mapped-arguments, namespace, and
extensibility paths.

Deep regressions cover 100,000 transparent Set/receiver layers, exact 3N and
2N fuel boundaries, nested target descriptor and callable-Proxy fuel, forced
collection, unique abrupt values, revoked inner targets, false-result
short-circuiting, descriptor mutation between lookup and trap call, Realm
identity, exact heap caps, cycle rejection, and pin-depth restoration.

```text
[Decision Log]
- 목적과 의도: Remove the 128-layer Proxy Set and receiver DefineOwnProperty correctness limits without losing ECMAScript trap order, descriptor presence, Realm identity, GC lifetime, cycle rejection, or host work bounds.
- 기존 구현 및 제약 조건: Missing Set and receiver defineProperty traps recursively re-entered Rust, both paths stopped at a fixed Proxy depth, complete and value-only receiver descriptors used duplicated validators and materializers, and ordinary prototype traversal recursively handed control to Proxy Set.
- 검토한 주요 대안: Raise the depth constant, retain separate recursive receiver helpers, build independent iterative Set and receiver-definition stacks, or let one rooted Set driver tail-forward while reusing the existing presence-aware iterative DefineOwnProperty state machine.
- 선택한 방식: Pin operation inputs once, iterate Proxy Set targets with one fuel charge per layer, return an explicit forwarding outcome at ordinary-to-Proxy boundaries, represent receiver descriptors with presence bits, and route both complete and value-only definitions through the shared Proxy DefineOwnProperty driver before specialized ordinary fallback.
- 다른 대안 대신 이 방식을 선택한 이유: A larger cap remains non-conforming and stack-dependent; duplicated iterative algorithms would drift in GetMethod, invariant, and cleanup order; an explicit continuation stack is unnecessary for transparent tail forwarding; and the existing DefineOwnProperty driver already owns the required roots, Realm allocation, and compatibility rules when descriptor presence is preserved.
- 장점, 단점 및 영향: Deep legal Proxy Set and receiver-definition chains are stack-safe and exactly fuel-bounded, one cleanup scope restores roots on every Result exit, and complete versus value-only descriptors remain observable. The coordinated property traversal below replaces the shared node-visited policy and removes the former ordinary Get, HasProperty, Set, and handler-prototype limits.
```

## Iterative ordinary property traversal

Ordinary `[[Get]]`, `[[HasProperty]]`, and `[[Set]]` now use iterative drivers
without the former 4096/1024/1024 depth cutoffs. `PropertyTraversal` records
directed `(from, to)` edges, pins each newly reached object until operation
cleanup, and owns ordinary-edge fuel credit. Directed edges are necessary
because a Proxy trap getter can mutate a previously visited target before
returning `undefined`; rejecting a repeated object before its own lookup would
skip that observable change. Keeping every reached object rooted also prevents
a collected heap cell from being reused under an identity retained by the
edge set.

Get extracts own-property handling into an explicit value/accessor/absent
result while retaining direct compatibility paths for TypedArray,
ArrayBuffer, and DataView fields. Inherited getters receive the original
receiver. Has checks TypedArray canonical numeric indices before ordinary own
properties. Set keeps its TypedArray, Array, String, mapped-arguments, and
receiver-definition paths, and Module Namespace `[[Set]]` now returns false
for every key and receiver. Public Value-key Get/Set first perform one
`ToPropertyKey` conversion into `PropertyKey`, preserving Symbols returned by
`@@toPrimitive`.

Fuel charges one unit per ordinary-to-ordinary edge and one per Proxy internal
method layer; an ordinary-to-Proxy edge relies on the Proxy charge. Proxy
`GetMethod` lookup and a transparently forwarded Set receive one initial
ordinary-edge credit to preserve the established exact per-Proxy budgets, but
deeper inherited handlers are metered. Revocation is validated before fuel.
Nested handler, invariant, and receiver operations create independent
traversal state.

Pure ordinary repeated edges fail immediately. A Proxy cycle is different:
each pass can run an observable trap lookup, so repeated edges are replayed.
An inert cycle is stopped after 512 replays with a catchable RangeError rather
than a native stack overflow; configured fuel can stop it earlier. This guard
does not limit acyclic chain depth. Traversal memory is O(depth) because both
the edge set and persistent GC roots grow with reached objects. Construction
and new-edge growth reserve HashSet and GC-root capacity before committing the
edge or pin, so allocation failure is catchable and leaves traversal state
retryable.

The same intrinsic audit installs the required own `Array.prototype.length`
descriptor in every Realm: value 0, writable, non-enumerable, and
non-configurable. This restores transparent Proxy Has forwarding for that
property and avoids a traversal-specific special case.

Treating an own data value of `undefined` as present also exposed two older
Array copy shortcuts that had passed by relying on the former incorrect Get
sentinel. `Array.prototype.slice` was changed to perform HasProperty before
Get, while `Array.prototype.with` was changed to Get every non-replaced index.
The interim result allocator intentionally used dense, hole-only `ArrayData`
and rejected copies above `MAX_DENSE_ARRAY_LEN` because the mutators of that
unit did not yet understand sparse logical length. The later Array-exotic unit
below supersedes that allocation restriction and completes the coupled
generic and sparse method paths. This paragraph records why the temporary
boundary existed; it is not the current Array architecture.

```text
[Decision Log]
- 목적과 의도: Remove non-conforming ordinary Get, HasProperty, Set, and inherited Proxy GetMethod depth cutoffs without losing receiver semantics, observable mutation order, GC identity, host work bounds, or correct Array hole copying once own undefined values stop acting as absence sentinels.
- 기존 구현 및 제약 조건: Get recursively returned undefined after 4096 hops, Has recursively returned false after 1024 hops, Set rejected after 1024 hops, Symbol Set bypassed the shared internal method, and node-based cycle rejection could suppress a later Proxy trap lookup. Rust-local object identities were not roots and could be reused after observable GC. Slice and With copied dense slots directly, so their inherited-value behavior depended on the old incorrect own-undefined fallback. General ArrayCreate installs an own length descriptor, but legacy push, pop, and splice still mutate dense backing storage directly and can leave that descriptor stale; sparse copy results would expose the same gap.
- 검토한 주요 대안: Raise the traversal constants, remove guards without fuel, keep separate string and Symbol walkers, reject every repeated object, restore the undefined sentinel to hide Array copy defects, retain general ArrayCreate and repair every legacy mutator in this unit, allow sparse copy results, or use rooted directed-edge traversal plus specification-shaped Array copy loops with a bounded hole-only allocator.
- 선택한 방식: For that bounded unit, preserve separate Get, Has, and Set exotic ordering while sharing PropertyTraversal for directed edges, persistent roots, fuel credit, and Proxy-cycle replay. Coerce Value keys once into PropertyKey, return false from Module Namespace Set, install the missing Array prototype length descriptor at Realm construction, and repair Slice/With at their own Has/Get boundaries. The interim fresh-copy path used the current Realm prototype without a stored length descriptor and rejected lengths above the dense cap before allocation.
- 다른 대안 대신 이 방식을 선택한 이유: Larger constants remain incorrect and stack-dependent; unmetered loops are unsafe for a sandbox; duplicate key paths drift; node rejection is observably wrong after trap mutation; native recursion cannot safely represent legal deep chains; and restoring sentinel behavior would make a present undefined property observably inherit through its prototype. The Array loops must implement their own distinct hole policies. Expanding this unit into every generic mutator would obscure the property-traversal correction, while returning a sparse copy before those mutators honor sparse_max creates immediately observable length and index errors.
- 장점, 단점 및 영향: Acyclic chains and inherited traps are stack-safe at arbitrary legal depth, exact fuel and LIFO pin cleanup are testable, Symbol and receiver behavior use one path, Proxy Has gains complete direct admission, and Slice/With no longer regress when Get is corrected. At that stage, dense copy results remained compatible with existing mutators and copying more than 1,048,576 elements raised a sandbox RangeError. Memory grows linearly with traversal depth, and inert Proxy cycles retain a deliberate 512-replay host guard that can differ from another engine's implementation-specific stack limit. The Array-specific copy cap, generic receiver gap, sparse mutators, and prototype facade described by this historical decision are superseded by the Array exotic unit below.
```

## Lazy Proxy-aware for-in enumeration

`ForInIteratorState` mirrors the state described by `CreateForInIterator`: it
retains the current object, whether that object has been snapshotted, the
visited string keys, and the current own-key snapshot plus cursor. It also
retains directed prototype edges, rooted node identities, Proxy presence, and
the cycle-replay count across pulls. The state lives in `IteratorData`, and GC
tracing follows both its current object and every traversal root. Creating an
iterator first boxes a non-nullish primitive and pins that wrapper across the
GC-retrying iterator allocation; `null` and `undefined` produce an already
empty iteration without traversal nodes.

Each `iterator_next_resume` advances only far enough to yield one key or reach
completion. It obtains a current object's keys through `[[OwnPropertyKeys]]`,
discards Symbols without invoking `[[GetOwnProperty]]` for them, then queries
each remaining string descriptor at the point it is processed. A name enters
the visited set only when the descriptor still exists, and is yielded only
when that descriptor is enumerable. After the snapshot is exhausted, the
iterator calls `[[GetPrototypeOf]]` and repeats on the returned object. This
preserves deleted-key, non-enumerable-shadowing, absent-descriptor, abrupt
completion, revocation, and early `break` behavior. State locks are never held
across an observable trap.

`own_property_keys_or_throw` now uses an explicit stack of pending Proxy
frames instead of recursive Rust calls. Every Proxy layer validates revocation,
consumes fuel, and roots its target and handler across `GetMethod` and trap
execution. A present trap validates its array-like result and duplicates,
then performs `IsExtensible(target)` before the target's
`[[OwnPropertyKeys]]`. On unwind, every target key's descriptor is queried
before an omitted non-configurable key is reported; non-extensible targets
then require an exact key set. The trap's original order is retained for the
caller. Missing traps tail-forward without imposing a legal chain-depth cap.

Ordinary own-key snapshots precharge fuel for every native key source before
materializing vectors or sets: typed-array indices, dense Array presence,
boxed or primitive String indices, module namespace exports, and stored
properties. String byte length is a conservative upper bound for UTF-16 key
count. Candidate processing and ordinary prototype edges are separately
metered. Newly reached prototypes are reserved and installed transactionally
in the iterator's traced state. Ordinary cycles fail when an edge repeats;
observable Proxy cycles may replay and are bounded by configured fuel or the
shared 512-replay host guard even when a Proxy yields one fresh key per pull.
Terminal completion replaces the traversal collections so a reachable
completed iterator does not retain capacity proportional to its former depth.

`Object.hasOwn` and `Object.prototype.hasOwnProperty` now use the same complete
Proxy `[[GetOwnProperty]]` path as `propertyIsEnumerable`, so virtual
descriptors, transparent nested targets, revocation, and abrupt values are no
longer bypassed. Map entries remain internal collection data and therefore do
not appear in object enumeration.

```text
[Decision Log]
- 목적과 의도: Make for-in enumerate ordinary and Proxy objects through the required internal methods while remaining lazy, stack-safe, GC-safe, and bounded by host fuel.
- 기존 구현 및 제약 조건: make_for_in_keys eagerly walked raw heap properties and prototype slots, treated Map entries as object keys, bypassed Proxy traps, retained per-key source objects, and could materialize an unmetered ordinary snapshot before a host could stop it.
- 검토한 주요 대안: Patch only transparent Proxy forwarding, eagerly collect all Proxy keys before loop execution, recurse through Proxy targets and prototypes, expose the internal iterator to JavaScript, or keep one lazy state machine backed by complete internal methods.
- 선택한 방식: Store CreateForInIterator-shaped state in IteratorData, advance one observable phase per pull, use iterative pending frames for Proxy OwnPropertyKeys invariants, trace and pin all live objects, and precharge ordinary snapshot work before native collection growth.
- 다른 대안 대신 이 방식을 선택한 이유: A forwarding-only patch would still miss descriptor and prototype traps; eager collection changes break and mutation order; Rust recursion makes legal depth host-stack-dependent; and an exposed iterator adds non-standard surface. Reusing the internal-method helpers keeps Object and for-in behavior aligned.
- 장점, 단점 및 영향: Proxy trap order, Symbol filtering, shadowing, abrupt completion, primitive boxing, and early break now follow the specification; deep chains are iterative and every unbounded walk is fuel- or replay-bounded. Iterator state and pending invariant frames use O(keys plus depth) native memory, non-ASCII String snapshot fuel is conservatively overcharged, and the 512 replay guard is an intentional sandbox policy for inert Proxy cycles.
```

## Array exotic prototype and generic indexed methods

Every Realm now installs `%Array.prototype%` as an actual `HeapObj::Array`
with empty `ArrayData`, not an ordinary object carrying an Array class tag.
Array index definitions therefore participate in `ArraySetLength`, so writing
prototype index `2` changes its length to `3`. The VM also records each
Realm's intrinsic Array constructor alongside its Array prototype. Both
registries are GC roots, Realm construction rolls them back transactionally,
and `ArraySpeciesCreate` can distinguish the current Realm's intrinsic
constructor from an observable foreign constructor.

`ArrayData` now has one representation rule. Default writable, enumerable,
configurable indexed data properties live in `items` and `present`; accessors,
non-default descriptors, and sparse entries live in `props`. A default
descriptor found in the legacy property table migrates into dense storage on
write, preventing duplicate states whose visible value depended on which
lookup path ran first. Mapped arguments retain their specialized aliasing
path. Array length shrink computes one removable-property plan, precharges
descriptor scanning and dense resize work after observable conversion but
before mutation, and applies the planned property and logical-length update
without rescanning mutable storage.

`push`, `pop`, `shift`, `unshift`, `splice`, `slice`, `copyWithin`, and `with`
now operate on generic receivers through ToObject, LengthOfArrayLike, Get,
HasProperty, Set, DeletePropertyOrThrow, and CreateDataProperty. Generic lengths
use the full ToLength range through `2^53 - 1`; creation of an actual Array
still enforces the ECMAScript uint32 length limit. Slice preserves holes and
uses `ArraySpeciesCreate`. Splice uses species for the deleted-elements result.
CopyWithin mutates through live property operations without a snapshot. With
intentionally ignores species and reads every non-replaced position, so source
holes become own `undefined` values. All observable values and fresh results
remain pinned until ownership transfers or an abrupt completion restores the
incoming pin depth.

The Array constructor and Slice can allocate sparse results beyond
`MAX_DENSE_ARRAY_LEN` without first reserving a dense vector. Dense allocation
uses the VM's collecting allocator, including retry at an exact heap-object
cap. With still rejects a result above 1,048,576 elements because its
read-through-holes contract must materialize every index and the sandbox does
not yet provide fallible native temporary storage for that many values.
`IsArray` follows Proxy targets iteratively and checks revocation before
charging one fuel unit per layer; that loop neither allocates nor re-enters
JavaScript between target reads. Large ArraySetLength scans and dense resizes
are similarly fuel-bounded before any irreversible mutation.

```text
[Decision Log]
- 목적과 의도: Model the intrinsic Array prototype and the coupled mutation and copy methods with real Array exotic semantics while preserving Realm, species, GC, fuel, and heap-cap invariants.
- 기존 구현 및 제약 조건: Array.prototype was an ordinary tagged facade, default indexed descriptors could exist in both dense storage and props, push/pop/splice derived positions from dense backing length, Slice and With accepted only represented Arrays, copy results above the dense cap were rejected, IsArray Proxy traversal was unmetered, and raw allocation could fail despite reclaimable garbage.
- 검토한 주요 대안: Special-case only Array.prototype length, patch each failing Test262 path, retain dense-only results, convert every Array method in one release, replace ArrayData entirely, or establish one exotic representation invariant and repair the tightly coupled constructor, species, mutation, and copy surface first.
- 선택한 방식: Allocate every intrinsic prototype as ArrayData, track Realm Array constructors, keep default dense descriptors in items/present and exceptional descriptors in props, implement seven methods through shared internal operations, permit sparse constructor and Slice results, and precharge Proxy, length-scan, and resize work before mutation.
- 다른 대안 대신 이 방식을 선택한 이유: A prototype-only special case would diverge from ordinary Arrays; individual test patches would miss generic receivers and observable order; dense-only allocation rejects legal sparse results; changing every callback and legacy method at once is too broad to validate atomically; and retaining duplicate indexed representations makes descriptor and mutator behavior order-dependent.
- 장점, 단점 및 영향: Array.prototype now has the same length/index invariants as every Array, generic receivers and species are observable in specification order, sparse construction no longer needs a giant vector, and exact fuel, GC retry, Realm rollback, and abrupt cleanup are regression-tested. The representation migration adds property-path complexity, With retains a deliberate 1,048,576-element sandbox cap, and at this decision boundary older methods such as reverse, fill, and several callback methods still needed their own generic and rooting audits; the later fill pipeline section records that follow-up's completion.
```

### Array `@@unscopables` intrinsic

Each Realm creates a distinct null-prototype `@@unscopables` object while
installing its Array intrinsic. The object contains the specification's 16
names in creation order, with mutable enumerable data properties whose values
are `true`. `%Array.prototype%` owns the list through a non-writable,
non-enumerable, configurable symbol property. `with` is absent because it is a
reserved word and cannot be an unqualified identifier in a `with` statement.

The Array constructor, prototype, and fresh list stay pinned while the
collecting allocator and fallible property publication run. Publishing through ordinary
`[[DefineOwnProperty]]` keeps heap reservation failures transactional instead
of bypassing the VM's fallible property-storage path. Once attached, normal GC
tracing through the Realm-rooted Array prototype owns the list.

```text
[Decision Log]
- 목적과 의도: Complete the observable Array intrinsic shape and make legacy with resolution honor the standard Array exclusion list in every Realm.
- 기존 구현 및 제약 조건: Symbol.unscopables and object-environment HasBinding were implemented, but Array intrinsic installation never created its required null-prototype list; Realm construction and property growth can trigger GC or host allocation failure.
- 검토한 주요 대안: Add a shared process-wide list, synthesize values inside with resolution, install only names currently implemented by RuJa, insert raw heap properties, or construct the exact Realm-owned intrinsic object through fallible property operations.
- 선택한 방식: Reserve three temporary roots, allocate one null-prototype object per Realm, define the exact 16 true-valued entries in specification order, omit with, publish the standard symbol descriptor on that Realm's Array prototype, and release all pins through one setup-result boundary before Realm publication.
- 다른 대안 대신 이 방식을 선택한 이유: The object and its mutability are JavaScript-observable; a shared or synthesized list breaks Realm identity and mutation isolation, implementation-dependent names diverge from ECMA-262, and raw insertion bypasses allocation-failure handling.
- 장점, 단점 및 영향: Main and created Realms expose exact independent lists, existing with bindings consume them without a new special case, and failed setup remains within the Realm transaction without leaking temporary pins. Intrinsic setup performs one additional heap allocation plus 17 fallible property publications per Realm.
```

### Generic Array concat pipeline

`Array.prototype.concat` now treats its boxed receiver and each argument as an
ordered input stream rather than reading represented Array backing vectors.
It creates the result before querying spreadability, then performs one
`Symbol.isConcatSpreadable` Get per object. An undefined override falls back to
Proxy-aware `IsArray`; a defined value is converted with `ToBoolean`.
Spreadable inputs capture `LengthOfArrayLike`, reject `n + length` above
`2^53 - 1` before indexed work, and execute `HasProperty` followed by
conditional `Get` for every logical index. Missing properties advance the
target index without materialization, while present values use
`CreateDataPropertyOrThrow`. Non-spread inputs become one data property. The
algorithm always ends with strict `Set(result, "length", n)` so custom species
setters, false Proxy traps, and non-writable lengths remain observable.

The receiver and all arguments are pinned before any boxing or species work.
The boxed receiver and species result join that operation-wide root set, and a
fetched value receives a temporary root across its observable result property
definition. One cleanup boundary restores the incoming pin depth for semantic
throws, Proxy revocation, fuel aborts, and heap-limit errors. Default species
allocation uses the existing collecting Array allocator. Since the result
starts at length zero and only present properties are defined, a large sparse
source does not reserve its logical length as dense storage. One fuel unit is
charged for every outer input, including an empty spreadable object, plus one
for every scanned source index, including holes.

TypedArray concat coverage exposed a transitional property-model defect:
TypedArray, ArrayBuffer, and DataView instance-field compatibility reads ran
before ordinary own descriptors. Those fields conceptually stand in for
prototype accessors, so an own `length`, `byteLength`, `byteOffset`, or `buffer`
descriptor now shadows the compatibility value before that fallback executes.

```text
[Decision Log]
- 목적과 의도: Replace the dense-only concat shortcut with the complete generic, species-aware, sparse-safe algorithm while preserving Realm, GC, fuel, and heap-cap invariants.
- 기존 구현 및 제약 조건: Concat cloned ArrayData.items, spread only direct represented Arrays, converted holes to own undefined values, bypassed ToObject and observable property operations, allocated outside the collecting VM path, and ignored custom species and Symbol.isConcatSpreadable. TypedArray compatibility reads also hid valid ordinary own length properties.
- 검토한 주요 대안: Patch only the failing typed-array files, keep a fast represented-Array branch, preallocate from captured lengths, reuse Set for copied indices, broaden Test262 feature gates by prefix, or implement the abstract-operation sequence directly and freeze only audited feature-gated files.
- 선택한 방식: Reuse the shared ToObject, LengthOfArrayLike, ArraySpeciesCreate, Proxy-aware IsArray, property-definition, strict-Set, Realm registry, and collecting allocation paths; retain u64 logical indices; root every observable value; meter each outer item and index; and admit nine exact Test262 paths through one manifest shared by both tools.
- 다른 대안 대신 이 방식을 선택한 이유: Fast backing-storage paths cannot preserve holes, inherited indices, Proxy traps, or custom species; preallocation is unsafe for huge sparse lengths; Set invokes inherited setters instead of CreateDataProperty; prefix admission expands silently with upstream; and one generic path keeps represented and non-represented receivers behaviorally identical.
- 장점, 단점 및 영향: All 69 direct concat files pass, sparse output above the dense cap is bounded by fuel rather than allocation size, observable failures restore roots, and TypedArray own length shadowing is corrected for every direct compatibility field. The algorithm must still scan every spreadable logical index as ECMAScript requires, native key strings and root vectors remain infallible Rust allocations, and shared Proxy or Bound constructor traversal needs separate per-edge fuel and linear argument collection.
```

### Generic Array copyWithin pipeline

`Array.prototype.copyWithin` snapshots only `LengthOfArrayLike`, then coerces
target, start, and an optional non-undefined end in source order through
`ToIntegerOrInfinity`. Relative positions remain `u64` through the full
`2^53 - 1` range. Overlap selects backward iteration only when the target begins
inside the source interval; all other copies advance forward.

Each logical iteration consumes one fuel unit before observable work. It then
performs `HasProperty(source)`. A present source is read with `Get` and written
with strict `Set`; an absent source invokes `DeletePropertyOrThrow` on the
destination. The method therefore preserves inherited values and holes,
observes live mutations from earlier traps, retains partial writes before a
later abrupt completion, and performs same-range operations rather than
silently treating them as a no-op. It never snapshots or materializes the
source interval, so native temporary memory is constant apart from property
traversal state and index strings.

The receiver and all arguments are rooted before boxing or coercion. The boxed
object remains rooted for the operation, and every fetched value receives a
temporary root across an observable setter or Proxy trap. One cleanup boundary
restores the incoming pin depth after semantic throws, Proxy false results,
collection, heap-cap retry during primitive boxing, or fuel abort. Primitive
boxing and native errors use the method Realm.

```text
[Decision Log]
- 목적과 의도: Implement the complete generic copyWithin algorithm without dense-array shortcuts, source snapshots, or host work that configured fuel cannot bound.
- 기존 구현 및 제약 조건: The method accepted only represented Arrays, used dense backing length, coerced arguments incompletely, copied a temporary Vec of values, collapsed holes, bypassed prototypes and Proxy traps, returned undefined for primitives and generic objects, and had no loop fuel or operation-wide roots.
- 검토한 주요 대안: Patch the 18 failing Test262 files, retain a dense fast path, snapshot presence/value records before mutation, share one algorithm with reverse or fill, cap logical length at the dense limit, or execute the abstract property operations directly.
- 선택한 방식: Reuse ToObject, u64 LengthOfArrayLike and relative-index helpers, iterate in the specification-selected direction, call HasProperty plus Get/strict Set or DeletePropertyOrThrow for every position, root the receiver/arguments/object/value, and charge one fuel unit immediately before each indexed step.
- 다른 대안 대신 이 방식을 선택한 이유: Test-only patches and dense fast paths miss live traps, inherited values, holes, and generic receivers; snapshots change mutation and abrupt-completion order; reverse and fill have different property sequences; and a dense cap rejects legal sparse objects near the safe-integer limit.
- 장점, 단점 및 영향: The direct fixed Test262 directory is 39/39, TypedArray borrowing remains compatible, MAX_SAFE_INTEGER work uses O(1) source storage, and exact fuel, Realm, GC, heap-cap, partial-mutation, and pin cleanup behavior is regression-tested. Unconfigured hosts can still request a specification-required linear scan, and native property-key strings plus traversal work remain subject to the broader process-memory policy.
```

### Generic Array fill pipeline

`Array.prototype.fill` now boxes its receiver and snapshots
`LengthOfArrayLike` exactly once before coercing start and an optional,
non-undefined end through `ToIntegerOrInfinity`. Relative positions stay `u64`
through `2^53 - 1`; the fill value itself is never coerced. Each selected index
then consumes one fuel unit and performs a live strict `Set`, in ascending
order. This preserves inherited setters, Proxy traps, non-writable failures,
resizable TypedArray behavior, partial mutation before abrupt completion, and
safe-integer property keys without consulting species or allocating a result.

The receiver and every argument are rooted before boxing. The boxed object
stays rooted across the observable length read, index coercions, and every
setter or Proxy trap, while the original fill value remains rooted as part of
the argument suffix. One cleanup boundary restores the incoming pin depth on
normal return, semantic throws, strict-Set rejection, collection, primitive
boxing at an exact heap cap, or fuel abort. Boxing and native errors therefore
retain the active method Realm.

```text
[Decision Log]
- 목적과 의도: Replace the represented-Array fill shortcut with the complete generic live-Set algorithm while preserving safe-integer, fuel, GC, Realm, and abrupt-completion behavior.
- 기존 구현 및 제약 조건: Fill cloned and rewrote dense backing storage, ignored observable length and sparse indices, accepted only numeric bounds, returned undefined for primitive and generic receivers, bypassed descriptors, prototypes, Proxies, arguments mappings, and TypedArrays, and performed an unmetered host loop.
- 검토한 주요 대안: Patch only the failing direct tests, retain a dense fast path, reuse the specialized TypedArray bulk fill, precompute property writes, cap work at the dense-array limit, or execute the abstract-operation sequence directly.
- 선택한 방식: Reuse ToObject, u64 LengthOfArrayLike and relative-index helpers; coerce bounds once in specification order; call strict Set for each ascending index after one fuel charge; root the receiver, arguments, and boxed object for the entire operation; and return that object.
- 다른 대안 대신 이 방식을 선택한 이유: Backing and bulk paths cannot preserve setters, Proxy order, per-index TypedArray conversion, or partial writes; a precomputed write set loses live mutation; and a dense cap rejects legal sparse array-like lengths. One generic path gives represented Arrays and borrowed receivers identical observable semantics.
- 장점, 단점 및 영향: Direct Array fill is 22/22 and adjacent TypedArray fill remains 52/52; safe-integer tails need constant native state; every observable exit restores roots; and configured fuel bounds the logical scan. Unbounded hosts can still request a specification-required linear scan, and native index-string plus property-traversal allocations remain governed by broader process-memory policy.
```

### Generic Array filter pipeline

`Array.prototype.filter` now boxes its receiver, snapshots
`LengthOfArrayLike` once as a safe-integer `u64`, validates the callback, and
performs `ArraySpeciesCreate(source, 0)` before indexed traversal. Each logical
index consumes one fuel unit, then runs live `HasProperty`; present values are
read with `Get` and passed to the callback with the captured index and source.
Truthy selections are compacted into ascending result indices through
`CreateDataPropertyOrThrow`. The path therefore preserves holes, inherited
values, Proxy order, callback mutation, custom species, descriptor failures,
and partial result definitions without snapshotting source values.

The receiver and all arguments are rooted before boxing. The source and species
result remain roots for the entire operation, and each present source value is
temporarily rooted across callback execution and a selected result's observable
Proxy define trap. One cleanup boundary restores the incoming pin depth after
length, constructor, species, `HasProperty`, `Get`, callback, property-definition,
heap-cap, or fuel failures. Default arrays and native errors use the active
method Realm; custom species objects retain their constructor semantics.

```text
[Decision Log]
- 목적과 의도: Replace the represented-Array filter snapshot with the complete generic, species-aware, live-property algorithm while preserving Realm, GC, fuel, and abrupt-completion behavior.
- 기존 구현 및 제약 조건: Filter cloned dense ArrayData.items, called the predicate for holes, ignored observable length, prototypes, Proxies, primitives, generic receivers, species, and result descriptors, allocated through a raw heap path, and performed no loop fuel or operation-wide rooting.
- 검토한 주요 대안: Patch only direct failures, retain a dense fast path, collect selected values before constructing the result, reuse the specialized TypedArray filter, cap traversal at the dense-array limit, or implement the abstract-operation sequence directly.
- 선택한 방식: Reuse ToObject, u64 LengthOfArrayLike, callable validation, ArraySpeciesCreate, Proxy-aware HasProperty/Get, callback dispatch, and CreateDataPropertyOrThrow; root all observable state; and charge one fuel unit before each logical source index.
- 다른 대안 대신 이 방식을 선택한 이유: Dense and preselected snapshots change holes, mutation, species timing, and abrupt partial results; TypedArray filter has different validation and species timing; and a dense cap rejects legal sparse array-like lengths. One generic path keeps represented Arrays, arguments, primitives, Proxies, and borrowed receivers observably aligned.
- 장점, 단점 및 영향: All 242 direct fixed Test262 files pass, the prior million-index sparse timeout becomes a pass, TypedArray filter remains 85/85, and exact fuel, Realm, GC, heap-cap, descriptor, and cleanup behavior is regression-tested. Unconfigured hosts can still request a specification-required linear scan through a huge sparse length, while native index strings and property traversal remain governed by broader process-memory policy.
```

### Generic live Array iterator records

`entries`, `keys`, and `values` all create the same lazy
`CollectionIteratorData` shape after one method-Realm `ToObject`. The iterator
stores its source behind a mutex and its cursor as `u64`, so safe-integer
array-like positions are not truncated on 32-bit targets. Every `next` reads
the current `LengthOfArrayLike`; TypedArray sources instead use their current
buffer witness so resize, out-of-bounds, and detach state remain observable.
Keys return the cursor without an indexed Get. Values and entries advance the
cursor before indexed Get or any result allocation, preserving progress after
an abrupt getter or heap failure. Completion replaces the source with
`undefined` before allocating the done result, both releasing the collection
and making completion sticky if that allocation fails.

The source, fetched element, entry pair, and iterator result remain roots
across Proxy access and collecting allocation. Entry pairs and all iterator
results use the active `next` function Realm. Array, Map, and Set prototypes
have separate native `next` entry points that accept only their own iterator
kinds; String iteration retains its separate brand. The obsolete internal
`IteratorData.array_like` cursor and its `usize` resume path were removed, so
Array and arguments iteration always observes the actual `@@iterator`
protocol.

Arguments creation records each Realm's immutable original
`%Array.prototype.values%` in a traced registry and installs that identity as
an own writable, non-enumerable, configurable `Symbol.iterator`. The registry
is included in Realm rollback. Arguments allocation pins its values,
prototype, iterator function, restricted callee, and unpublished object while
using the GC-retrying VM allocator. Mapped and unmapped objects therefore
survive reclaimable heap caps without exposing a partially initialized object,
while later deletion or replacement of the own iterator remains observable.

```text
[Decision Log]
- 목적과 의도: Replace represented-Array snapshots and the duplicate internal array-like cursor with one specification-shaped, generic, live, Realm-correct, and GC-safe Array iterator path.
- 기존 구현 및 제약 조건: Entries and keys eagerly materialized arrays, values accepted only a narrow source shape, the shared iterator used a usize cursor and immutable source, arguments lacked the required own iterator identity, cross-brand next calls were accepted, and arguments allocation bypassed GC retry.
- 검토한 주요 대안: Patch only detached receiver tests, retain a dense fast path, keep the old IteratorData fallback for arguments, snapshot length or values, use one unbranded next function for every collection, or represent the specification iterator record directly.
- 선택한 방식: Perform ToObject at iterator creation, store a mutable rooted source and u64 cursor, read live length and indexed values per next, advance before abrupt indexed work, release the source at completion, allocate pairs/results in the method Realm, preserve the original Realm Array-values identity for arguments, and remove the unreachable fallback cursor.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshots lose live mutation and Proxy order, usize truncates legal safe-integer positions on wasm32, fallback paths drift in deletion and override behavior, and a shared unbranded native entry point violates internal-slot checks. One record keeps Array and TypedArray behavior aligned without claiming unrelated Array methods.
- 장점, 단점 및 영향: Generic, primitive, arguments, Proxy, inherited, resizable, detached, abrupt, cross-Realm, and exact-cap cases share one tested path; completion releases retained sources; and wasm32 cannot re-enter the obsolete cursor. Each next still performs specification-required property work. At this iterator decision boundary generic reverse, fill, and other snapshot-based Array methods remained separate follow-ups; the preceding fill pipeline section records fill's later completion.
```

## Generic Array FlattenIntoArray

`Array.prototype.flat` and `flatMap` share one specification-shaped
`flatten_into_array` path. The entry points perform `ToObject`, one source
length snapshot, depth or mapper validation, and `ArraySpeciesCreate` in their
specified order. The shared loop then observes every source index through
`HasProperty` and `Get`, applies the mapper only in the initial flatMap frame,
checks `IsArray`, reads each nested length at descent time, and defines dense
target properties from the supplied `start` index without setting an ordinary
custom species result's length.

The algorithm uses a Rust `Vec<FlattenFrame>` rather than native recursion.
Each child frame owns the exact GC pin suffix for its source and any original
flatMap input retained below a mapped result. Frame completion pops that suffix;
the outer cleanup pops all still-live suffixes after abrupt completion. One
fuel unit is charged before each logical source index, which bounds huge sparse
lengths when an embedder configures a budget. Infinite-depth traversal tracks
only repeated sources on the active path: it permits 512 observable replays so
getters can break a cycle, then raises `RangeError` instead of growing host
memory forever. Active identities are normalized through transparent Proxy
wrappers and stored in a ref-count map, so fresh wrappers cannot bypass the
guard and cycle checks do not turn deep acyclic nesting into quadratic native
work. Acyclic nesting has no fixed depth cutoff.

```text
[Decision Log]
- 목적과 의도: Replace Array-only snapshot flattening with one generic, observable, GC-safe, and sandbox-bounded implementation shared by flat and flatMap.
- 기존 구현 및 제약 조건: flat coerced depth before receiver length, accepted only represented Arrays, copied dense backing vectors, lost holes and inherited values, ignored species and Proxy operations, and recursively consumed the native stack; flatMap separately snapshotted and mapped the dense backing vector.
- 검토한 주요 대안: Patch only detached calls, retain an Array fast path, recursively translate the specification, materialize nested lists before output, cap depth at a fixed number, or use an explicit traversal stack with lexical pins.
- 선택한 방식: Keep the entry-point ordering separate, share an iterative frame stack for FlattenIntoArray, perform live property operations, transfer temporary roots into child frames, use CreateDataPropertyOrThrow semantics, enforce the safe-integer target bound, meter every visited source index, and bound only repeated active-path sources after 512 observable replays.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot and fast paths change observable mutation and Proxy order, native recursion makes Infinity cycles a host crash risk, a fixed depth cap rejects finite valid programs, and late rooting permits heap-cell reuse during getters or callbacks. Frames preserve depth-first order while making both roots and resource work explicit.
- 장점, 단점 및 영향: Direct flat and flatMap Test262 are 43/43, cyclic Infinity inputs terminate by fuel or RangeError, and all abrupt paths restore pin depth. Frame storage and retained roots grow linearly with active acyclic nesting depth; a cycle that would mutate only after more than 512 replays is intentionally terminated by the sandbox guard. Copy-by-value and other independent Array methods remain separate conformance units.
```

## Generic Array forEach

`Array.prototype.forEach` now boxes its receiver, snapshots
`LengthOfArrayLike` once, validates the callback after that observable length
read, and scans the captured index range with live `HasProperty` and `Get`
operations. A callback therefore observes inherited values and mutations to
unvisited indices, skips deleted indices, and never visits indices added beyond
the captured length. The callback receives `(value, index, object)` with the
supplied `thisArg`; its result is discarded.

The receiver and argument slice are pinned before boxing or length access, and
the boxed object plus each fetched value remain pinned across Proxy traps,
getters, and callback execution. A single outer cleanup restores all persistent
pins on normal or abrupt completion, while each temporary value pin is released
immediately after its callback. One fuel unit is charged per logical index,
including holes, so huge sparse array-like inputs remain bounded by the
embedder's cooperative budget.

```text
[Decision Log]
- 목적과 의도: Replace represented-Array snapshot iteration with specification-shaped generic, live, GC-safe, and fuel-bounded forEach traversal.
- 기존 구현 및 제약 조건: The old method accepted only represented Arrays, cloned dense storage, invoked callbacks for holes, ignored inheritance and Proxy operations, observed no later mutation, accepted invalid detached receivers, and had no explicit fuel or pin discipline.
- 검토한 주요 대안: Patch only detached calls, retain a dense Array fast path, share filter through a discarded result, precompute present values, or implement the direct indexed traversal.
- 선택한 방식: Perform ToObject, one LengthOfArrayLike snapshot, callback validation, then live HasProperty/Get/callback work while pinning all native-frame roots and charging every logical index.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot and fast paths change observable holes, inheritance, mutation, and Proxy order; reusing filter would incorrectly allocate and consult species. The direct loop mirrors the specification and existing generic Array runtime contracts.
- 장점, 단점 및 영향: Direct Array forEach is 190/190 and adjacent TypedArray forEach remains 42/42 on the fixed corpus; callback and fuel failures restore pin depth. Sparse scans remain linear in captured length as required, but configured fuel bounds their sandbox cost. Copy-by-value and other independent snapshot methods remain separate units.
```

## Generic Array join

`Array.prototype.join` now boxes its receiver, snapshots
`LengthOfArrayLike` once, coerces the separator, and then performs a live `Get`
for every captured index. Missing and explicitly nullish elements both append
only the separator, while every other fetched value is converted immediately.
Separator coercion can therefore mutate values before the first indexed read,
and an element conversion can mutate later values without extending the
captured range.

The receiver and argument slice are pinned before any observable operation.
The boxed object remains pinned for the whole scan, and each fetched element is
pinned across `ToString`; one outer cleanup restores persistent roots after
normal, property, conversion, allocation, or fuel completion. One fuel unit is
charged per logical index. The Rust string builder uses `try_reserve` before
each separator and element append so capacity overflow or allocation refusal
becomes a catchable `RangeError` rather than a host panic. Active receiver
identities bound cyclic element `toString`/`join` re-entry only after each
call's separator coercion. This preserves valid finite re-entry from a
separator while direct or indirect element cycles contribute an empty field.

```text
[Decision Log]
- 목적과 의도: Replace represented-Array snapshot joining with a specification-shaped generic, live, GC-safe, fuel-bounded, and allocation-aware traversal.
- 기존 구현 및 제약 조건: The old method coerced the separator before the receiver and length, accepted only represented Arrays, cloned dense storage, ignored inheritance and Proxy reads, swallowed element conversion errors, observed no later mutation, and had no explicit fuel or pin discipline.
- 검토한 주요 대안: Patch only detached calls, retain a dense fast path, collect all element strings before joining, reuse TypedArray join, impose a fixed source-length cap, or build the result incrementally from generic indexed reads.
- 선택한 방식: Perform ToObject, one LengthOfArrayLike snapshot, separator ToString, then live Get and immediate element ToString while pinning native-frame roots, charging every index, reserving each string append fallibly, and tracking active receiver identities to suppress cyclic native re-entry.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot and fast paths change observable mutation, inheritance, Proxy, and abrupt order; TypedArray join has distinct receiver validation; and a fixed index cap rejects valid sparse programs that cooperative fuel can bound. Incremental construction follows the specification without retaining a second element snapshot.
- 장점, 단점 및 영향: Direct Array join is 23/23 and adjacent TypedArray join remains 32/32 on the fixed corpus; abrupt and fuel exits restore pin depth, finite separator re-entry remains observable, and direct or indirect element cycles cannot overflow the native stack. Runtime and output work remain linear in captured length and produced bytes, while configured fuel and checked reservation prevent unbounded native traversal or String capacity panic. Final conversion of the completed Rust String into Arc<str> still follows the runtime-wide infallible allocation model. Copy-by-value and other independent snapshot methods remain separate units.
```

## Generic Array map

`Array.prototype.map` boxes its receiver, snapshots `LengthOfArrayLike`,
validates the callback, and creates the species result at that captured length
before indexed work. It then performs live `HasProperty` and `Get` operations,
calls the mapper with `(value, index, object)` and the supplied `thisArg`, and
defines the mapped value at the same index. Holes remain holes, inherited
values are visited, and callback mutation affects only unvisited indices
inside the captured range.

The receiver, arguments, boxed object, and species result remain roots for the
operation. Each fetched value is pinned across callback execution, and each
mapped result is pinned across the potentially observable species-target
definition. One fuel unit is charged for every captured source index. Creating
an intrinsic dense result also uses the existing Array length materialization
meter, so a three-element ordinary map consumes three creation units plus
three scan units; custom sparse species results pay their own constructor work.
All normal and abrupt exits restore the incoming pin depth.

```text
[Decision Log]
- 목적과 의도: Replace represented-Array snapshot mapping with specification-shaped generic, live, species-aware, GC-safe, and fuel-bounded traversal.
- 기존 구현 및 제약 조건: The old method accepted only represented Arrays, copied dense storage, invoked callbacks for holes, ignored inheritance, mutation, Proxy operations, species, detached receiver errors, and method-Realm result allocation, then allocated the result only after every callback.
- 검토한 주요 대안: Patch only detached calls, retain a dense fast path, collect mapped values before allocation, share filter with an index-preserving mode, or implement the direct indexed algorithm.
- 선택한 방식: Perform ToObject, one length snapshot, callback validation, ArraySpeciesCreate(length), then live HasProperty/Get/callback/CreateDataPropertyOrThrow while explicitly rooting every native-frame value and metering result materialization plus each logical index.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot and fast paths change observable holes, mutation, inheritance, Proxy, allocation, and abrupt order; adapting filter would hide map's fixed-index and captured-length result contract. The direct loop mirrors the specification and reuses the established species helper without a second materialized list.
- 장점, 단점 및 영향: Direct Array map is 216/216 and adjacent TypedArray map remains 85/85 on the fixed corpus; forced GC, Proxy definitions, callback errors, species allocation failures, and fuel aborts preserve roots and cleanup. Runtime remains linear in captured length as required, with cooperative fuel bounding sparse scans. Copy-by-value and other independent snapshot methods remain separate units.
```

## Generic Array reduce

`Array.prototype.reduce` boxes its receiver, snapshots `LengthOfArrayLike`, and
validates the callback before inspecting indexed properties. An explicitly
provided initial value is used even when it is `undefined`; otherwise the
method scans upward with live `HasProperty` and `Get` operations until it finds
the first accumulator. The remaining captured indices are visited in ascending
order, skipping absent properties and calling the reducer with
`(accumulator, value, index, object)` and `undefined` as the call this-value.

Receiver, arguments, and boxed object remain roots for the operation. The
current value is pinned across callback execution. A callback result is pinned,
then the temporary new root and previous accumulator root are removed together
in LIFO order before exactly one persistent root is installed for the new
accumulator. This keeps native root storage O(1) while preserving the live
accumulator across later Proxy traps, getters, forced GC, and abrupt exits. One
fuel unit is charged for every examined logical index, including holes during
omitted-initial discovery.

```text
[Decision Log]
- 목적과 의도: Replace represented-Array snapshot reduction with specification-shaped generic, live, GC-safe, and fuel-bounded ascending traversal.
- 기존 구현 및 제약 조건: The old method accepted only represented Arrays, copied dense storage, treated a leading hole as undefined, invoked callbacks for holes, ignored inheritance and mutation, accepted invalid detached receivers, used a non-standard third argument as callback this, and did not root a changing accumulator.
- 검토한 주요 대안: Patch only detached calls, retain a dense Array fast path, precompute present values, share one direction-parameterized core with the still-unrepaired reduceRight, or implement reduce's direct ascending algorithm first.
- 선택한 방식: Perform ToObject, one length snapshot, callback validation, live omitted-initial discovery, then one ascending HasProperty/Get/callback loop with explicit current-value and accumulator root ownership and one fuel charge per examined index.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot and fast paths change observable holes, inheritance, mutation, and Proxy order. Combining reduceRight before its independent baseline and descending-order audit would widen this unit. The direct loop mirrors the specification and keeps the next change reviewable.
- 장점, 단점 및 영향: Direct Array reduce is 260/260 and adjacent TypedArray reduce remains 50/50 on the fixed corpus; object-to-object accumulator replacement, forced GC, fuel aborts, property errors, callback throws, and empty-without-initial errors restore pin depth. Runtime remains linear in captured length with O(1) native temporary roots. Copy-by-value and other independent snapshot methods remain separate units.
```

## Generic Array reduceRight

`Array.prototype.reduceRight` boxes its receiver, snapshots
`LengthOfArrayLike`, and validates the callback before inspecting indexed
properties. An explicitly provided initial value is used even when it is
`undefined`; otherwise the method scans downward with live `HasProperty` and
`Get` operations until it finds the first accumulator. Remaining captured
indices are visited in descending order, skipping absent properties and
calling the reducer with `(accumulator, value, index, object)` and `undefined`
as the call this-value.

The receiver, arguments, boxed object, current value, and changing accumulator
follow the same explicit root ownership as ascending reduce. The descending
loops hold the next index as an exclusive upper bound and decrement before
property access, so index zero is examined exactly once without unsigned
underflow. A callback result temporarily becomes the newest root; that root
and the old accumulator root are popped together before exactly one persistent
new accumulator root is installed. One fuel unit is charged for every examined
logical index, including holes during omitted-initial discovery.

```text
[Decision Log]
- 목적과 의도: Replace represented-Array snapshot reduction with specification-shaped generic, live, GC-safe, and fuel-bounded descending traversal.
- 기존 구현 및 제약 조건: The old method accepted only represented Arrays, copied dense storage, selected physical storage rather than live properties, invoked callbacks for holes, ignored inheritance and mutation, accepted invalid detached receivers, used a non-standard third argument as callback this, and did not root a changing accumulator.
- 검토한 주요 대안: Patch only detached calls, reverse a collected value list, parameterize ascending reduce before auditing reverse boundaries, or implement the direct descending algorithm with an exclusive upper-bound index.
- 선택한 방식: Perform ToObject, one length snapshot, callback validation, live omitted-initial discovery, then one descending HasProperty/Get/callback loop with decrement-before-access indexing, explicit current-value and accumulator root ownership, and one fuel charge per examined index.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshots change observable holes, inheritance, mutation, and Proxy order. Premature direction parameterization can hide zero-boundary and omitted-initial position defects. An exclusive upper bound mirrors the specification while making underflow impossible.
- 장점, 단점 및 영향: Direct Array reduceRight is 260/260 and adjacent TypedArray reduceRight remains 50/50 on the fixed corpus; forced GC, fuel aborts, property errors, callback throws, and empty-without-initial errors restore pin depth. Runtime remains linear in captured length with O(1) native temporary roots. Copy-by-value and other independent snapshot methods remain separate units.
```

## Generic Array reverse

`Array.prototype.reverse` boxes its receiver, snapshots `LengthOfArrayLike`,
and processes `floor(length / 2)` lower/upper pairs in place. For each pair it
performs lower `HasProperty` and conditional `Get`, then upper `HasProperty` and
conditional `Get`. Only after those observations does it apply the specified
strict writes and deletion for the pair's four possible existence states.
Thus inherited values participate, holes move as holes, Proxy traps observe the
normative order, and an abrupt mutation retains all preceding partial effects.

The receiver and boxed object remain roots for the operation. A fetched lower
value is pinned before the upper existence check, and both fetched values stay
pinned across strict writes and deletes. Pair-local roots are removed after
every completion before an error propagates. One fuel unit is charged per pair;
the method materializes no index list, so huge sparse lengths remain bounded by
cooperative fuel rather than an implementation-only length cap.

```text
[Decision Log]
- 목적과 의도: Replace represented-Array storage reversal with the specification-shaped generic, sparse, observable, GC-safe, and fuel-bounded in-place algorithm.
- 기존 구현 및 제약 조건: The old method accepted only represented Arrays, reversed dense storage and presence bits without internal property operations, returned undefined for generic or primitive receivers, ignored inheritance and Proxy traps, could not report strict Set/Delete failures, and exposed no cooperative work bound.
- 검토한 주요 대안: Patch only detached calls, collect all indexed values before rewriting, retain a dense fast path, reuse the sorting collector, or implement the direct four-state pair algorithm.
- 선택한 방식: Perform ToObject and one length snapshot, then for each pair execute ordered HasProperty/Get observations and the exact Set/Delete branch while rooting fetched values and charging one pair fuel unit.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot, dense, and sorting paths reorder Proxy effects, erase holes or inheritance, and prevent specification-required partial mutations. The direct pair state machine is allocation-free and maps each observable step to the normative algorithm.
- 장점, 단점 및 영향: Direct Array reverse is 18/18 and adjacent TypedArray reverse remains 22/22 on the fixed corpus; forced GC and abrupt Has/Get/Set/Delete paths restore pin depth. Runtime is linear in half the captured length and can partially mutate before an error as required. ToReversed, toSpliced, and other copy-by-value methods remain independent units.
```

## Iterative OrdinaryHasInstance traversal

`InstanceofOperator` roots its left and right operands before any
`@@hasInstance` lookup can re-enter JavaScript. `OrdinaryHasInstance` then
uses one iterative state machine for Bound Function forwarding and prototype
walking. Bound targets are observed only after an edge fuel debit. Ordinary
prototype edges are debited before `[[GetPrototypeOf]]`; Proxy edges retain
the traversal helper's internal debit so one logical edge is never charged
twice. The walk performs `[[GetPrototypeOf]]`, tests for `null`, and only then
applies `SameValue` to the constructor prototype.

Calls that reach the exact default
`%Function.prototype%[@@hasInstance]` through transparent Bound or Proxy
wrappers trampoline back into the state machine with their transformed
`this`, first argument, and intrinsic Realm. Observable Proxy `apply` traps
remain ordinary calls. Interpreted calls retain the 512-frame VM limit;
every native dispatch participates in an independent 128-frame active-native
limit, so an apply trap that recursively calls `Reflect.apply` produces a
catchable `RangeError` instead of exhausting the Rust stack. This broader
guard can also reject otherwise valid builtin/callback native re-entry deeper
than 128 even when interpreted depth remains available. Both counters and all
operation roots are restored on normal and abrupt exits.

```text
[Decision Log]
- 목적과 의도: Make instanceof specification-ordered, stack-safe, fuel-bounded, Realm-correct, and GC-safe across deep Bound and Proxy wrapper graphs.
- 기존 구현 및 제약 조건: OrdinaryHasInstance recursively followed Bound targets, operands and the constructor prototype could become stale across observable work, ordinary prototype edges were unmetered, comparison before GetPrototypeOf accepted a constructor prototype as its own instance, and recursive native Proxy apply traps could still overflow the host stack.
- 검토한 주요 대안: Add only a recursion cap, flatten Bound targets at bind creation, bypass every Proxy wrapper, charge both the caller and shared property traversal, or reuse the interpreted frame counter for native calls.
- 선택한 방식: Root the operation state, iterate Bound/default-handler forwarding and prototype traversal, preserve observable Proxy apply behavior, debit each edge exactly once, order GetPrototypeOf before SameValue, and cap native re-entry independently from interpreted frames.
- 다른 대안 대신 이 방식을 선택한 이유: A depth cap rejects valid deep wrapper chains, eager flattening changes observable target identity and metadata, broad Proxy bypass skips apply traps, duplicate debits make fuel depend on implementation layering, and one shared depth counter would reduce valid interpreted recursion to protect a different host-stack boundary.
- 장점, 단점 및 영향: Fifty-thousand-layer Bound chains complete without native recursion, self-prototype and Proxy ordering match the specification, hostile native re-entry becomes a catchable error, and exact fuel, GC, Realm, abrupt identity, and cleanup are covered. The 128 limit applies to all active native dispatch, so sufficiently deep valid native builtin/callback re-entry now throws before the 512 interpreted-frame boundary. Nested property and GetPrototypeOf scratch allocation is still only partially fallible and remains a separate runtime-wide allocation unit.
```

## Fallible Proxy prototype validation state

Proxy `[[GetPrototypeOf]]` remains iterative across transparent and validating
targets. Each root it owns directly is now preceded by an exact
`try_reserve_gc_pins`: the input after object validation, target/handler after
the edge fuel debit, trap after `GetMethod`, returned object after type
validation, and deferred expected prototype after nested `IsExtensible`.
Deferred entries reserve scratch storage before publishing their root, then
validate in reverse after the terminal target prototype is known.

The nested Proxy `[[IsExtensible]]` walk applies the same fallible root rule.
It records the first Boolean trap result and whether any later result differs,
rather than retaining every Boolean in a vector. Validation still waits until
the terminal target result is observed, so a deeper revoked proxy or abrupt
trap completion outranks an already-known mismatch exactly as before.

```text
[Decision Log]
- 목적과 의도: Turn directly owned Proxy prototype-validation scratch and root growth into catchable sandbox errors without changing ECMAScript observation order.
- 기존 구현 및 제약 조건: GetPrototypeOf pinned five classes of values through infallible Vec pushes, appended deferred prototypes after pinning, and called an IsExtensible implementation that retained an unbounded Vec<bool> and used more infallible pins. Ordinary Result errors cleaned up correctly, but allocator failure could bypass that control flow.
- 검토한 주요 대안: Reserve all capacity at entry, use one broad depth cap, make pin globally fallible in the same change, retain Vec<bool> with try_reserve, or stage exact reserves at the operations that acquire each value.
- 선택한 방식: Reserve each root at its current semantic boundary, reserve deferred scratch before pin and push, use exact test-only site failpoints, and reduce IsExtensible trap-result storage to a delayed O(1) consistency summary.
- 다른 대안 대신 이 방식을 선택한 이유: Entry preallocation can pre-empt revocation, fuel, getter, call, type, or invariant errors; a depth cap rejects valid chains; a global pin API migration crosses every VM subsystem; and retaining every Boolean allocates despite only equality with one terminal result mattering.
- 장점, 단점 및 영향: Direct reserve failures are catchable, Realm-correct, ordered, and leak-free; later failures release earlier deferred roots; null continuations need no object root; and validating chains no longer allocate result Booleans per layer. This does not make the transitive path fully allocator-fallible: PropertyTraversal HashSets and pins, trap execution, PropertyKey and Error strings, GC root enumeration, and mark worklists remain separate units.
```

## Fallible shared property traversal state

`PropertyTraversal` construction reserves its initial object-identity set and
the caller-owned GC root suffix before callers publish pins. Advancing a new
edge preserves semantic priority: ordinary fuel or credit is consumed first,
duplicate ordinary or Proxy replay handling runs next, and only a genuinely
new edge reserves the edge set. A newly reached node additionally reserves the
node set and GC root storage before the edge, node, or pin is committed.
Reservation failure therefore cannot leave a half-visible edge or leak a root.

Lazy `for...in` cannot use operation-local traversal state because each public
`next()` returns after one key. Its iterator-owned edge set, rooted-node set,
traced root vector, Proxy marker, and replay count persist across calls. This
closes the case where a cyclic Proxy returned itself while producing a fresh
key on every pull and previously obtained a new 512-replay budget each time.
An abrupt `next()` remains a completed operation: a later call re-observes the
Proxy prototype trap rather than caching its prior result across the error.

```text
[Decision Log]
- 목적과 의도: Make shared property-chain state allocation catchable and atomic, and enforce one cycle-replay budget across the complete lifetime of a lazy for-in traversal.
- 기존 구현 및 제약 조건: PropertyTraversal collected initial nodes and inserted edges and roots through infallible HashSet/Vec growth. Lazy for-in recreated that traversal on every next call, so a self-returning Proxy with one fresh key per pull could bypass the documented replay guard. Persisting raw heap indices without traced Values would allow GC slot reuse to change their identity.
- 검토한 주요 대안: Reserve a large fixed capacity at entry, impose a prototype depth cap, make every VM pin globally fallible in one patch, keep operation-local for-in traversal, cache a successful Proxy prototype result across a thrown allocation error, or persist only numeric heap indices.
- 선택한 방식: Reserve exact local capacity immediately before each state publication, keep caller initial roots in the existing pin suffix, store lazy for-in traversal state and corresponding Values in IteratorData, trace those Values in both GC iterator paths, and release collection capacity at terminal completion.
- 다른 대안 대신 이 방식을 선택한 이유: Entry preallocation changes failure priority and can over-allocate; a depth cap rejects legal acyclic chains; a global pin migration crosses unrelated subsystems; operation-local state resets cycle protection; caching across an abrupt operation suppresses later Proxy observations; and untraced indices become stale after collection.
- 장점, 단점 및 영향: Get, HasProperty, Set, inherited Proxy GetMethod, and for-in edge growth now fail through ordinary Result cleanup with retryable state, exact fuel and cycle ordering, Realm-correct errors, and stable GC identities. Persistent for-in state uses O(depth) memory while active and releases that capacity when done. Key snapshots, visited-key growth, trap-call internals, PropertyKey/Error strings, GC root enumeration, and mark worklists remain explicitly outside this unit.
```

## Fallible lazy for-in key state

After `OwnPropertyKeys` completes, lazy `for...in` counts only string keys and
reserves the iterator-owned snapshot before replacing any prior state. The
snapshot is published by consuming the returned key vector directly, so there
is no second infallible temporary collection. A symbol-only result requires no
reservation. Snapshot failure leaves the iterator unvisited and causes the
next pull to re-observe `OwnPropertyKeys`.

For each consumed candidate, `[[GetOwnProperty]]` runs before visited-key
growth. An absent descriptor does not mark the name, while an existing
descriptor reserves the visited set before insertion. Reservation failure
leaves the mark uncommitted but retains the consumed candidate cursor, because
the specification removes the candidate before descriptor lookup and the
existing fuel and descriptor abrupt paths have the same progression. A retry
can therefore observe a same-name prototype property. Already visited names
skip both descriptor lookup and reservation. Terminal completion releases the
snapshot and visited-set capacities; prototype transitions retain capacity for
reuse during the active traversal.

```text
[Decision Log]
- 목적과 의도: Make the two key collections directly owned by a lazy for-in iterator allocator-fallible without changing observation, shadowing, fuel, or retry order.
- 기존 구현 및 제약 조건: OwnPropertyKeys results were filtered through an infallible temporary Vec and assigned to remaining_keys without reservation, while every existing descriptor inserted into visited_keys through infallible IndexSet growth. Iterator pulls deliberately preserve consumed-candidate progression across fuel and descriptor abrupt completions.
- 검토한 주요 대안: Reserve for all returned keys, build a second filtered vector, roll back the cursor on visited reservation failure, mark absent descriptors, reserve on duplicate prototype names, or combine Proxy own-key frames and GC worklists into the same patch.
- 선택한 방식: Count string keys without allocation, reserve the snapshot immediately before publication, consume the returned PropertyKey vector directly, and reserve a new visited entry only after an existing descriptor is observed. Keep the already consumed cursor on failure and release both capacities only at terminal completion.
- 다른 대안 대신 이 방식을 선택한 이유: Reserving symbols changes failure behavior for discarded data; a second vector retains an infallible allocation; cursor rollback repeats a key that the specification already removed and disagrees with existing abrupt progression; absent and duplicate keys need no new state; and broader allocator ownership would obscure this exact boundary.
- 장점, 단점 및 영향: Snapshot and visited growth now produce catchable, Realm-correct errors with atomic collection publication, exact no-op boundaries, stable Proxy and fuel ordering, and terminal capacity release. A failed child visited mark intentionally permits a same-name prototype key on retry. Proxy own-key trap-result vectors and duplicate sets, pending validation frames, filtered results, PropertyKey/Error strings, GC root enumeration, and mark worklists remain separate units.
```

## Fallible Proxy ownKeys entry collection

`CreateListFromArrayLike` for a Proxy `ownKeys` result remains incremental. Each
logical index consumes fuel, performs `Get`, and validates that the value is a
String or Symbol before the native key vector checks whether the next push
would exceed capacity. Only a full vector requests capacity and can fail;
spare-capacity pushes publish directly. The operation does not preallocate from
`length`, because doing so could fail before observable index access on a large
array-like result.

Duplicate validation remains a second pass after every index has been read.
For each key, membership is checked before growth; an existing key therefore
throws the required duplicate `TypeError` without reserving. A new key reserves
the `IndexSet` before insertion only when the set is full; spare capacity does
not create a failure boundary. String/Symbol consumer filtering and target
invariant checks happen later. Any reservation failure discards the
operation-local collections and unwinds all owned pins, so a retry starts from
the `ownKeys` trap and cannot observe a partial list.

```text
[Decision Log]
- 목적과 의도: Make Proxy ownKeys trap-result keys and duplicate-detection entries allocator-fallible while preserving CreateListFromArrayLike and Proxy invariant observation order.
- 기존 구현 및 제약 조건: The trap-result Vec pushed every validated String or Symbol through infallible growth, and the later IndexSet used infallible insert. The array-like length can reach MAX_SAFE_INTEGER, every index Get is observable and fuel-bounded, duplicate validation must wait until all entries are collected, and consumer filtering occurs only after Proxy invariants.
- 검토한 주요 대안: Reserve the complete reported length, combine collection with duplicate validation, reserve before Get or type validation, reserve before checking membership, impose a fixed key limit, or include pending frames and target-key sets in the same patch.
- 선택한 방식: After each successful Get and key-type validation, request Vec capacity only if the next push would exceed current capacity; keep the complete-list duplicate pass, test membership first, and request IndexSet capacity only for a new key when the set is full. Translate either actual growth failure into the current operation Realm's RangeError.
- 다른 대안 대신 이 방식을 선택한 이유: Length preallocation and early reservation change getter and error priority; fused duplicate detection can suppress later entry errors; reserving duplicates creates failures for state that will not grow; a fixed cap rejects valid programs; and broader ownership boundaries would make retry and cleanup evidence harder to isolate.
- 장점, 단점 및 영향: Both directly owned entry collections now fail through ordinary Result cleanup only at real growth boundaries, with exact fuel, getter, type, duplicate, Symbol, Realm, retry, nested-frame, and for-in snapshot behavior. Helper-level tests fill the allocator-reported capacity and prove spare slots cannot consume a failure. Reservation remains amortized, and contains plus insert hashes each unique key twice. Operation input, target/handler, trap-result list, and length-value roots, pending validation frames and roots, filtered vectors, non-extensible target sets, index strings, PropertyKey/Error strings, GC root enumeration, and mark worklists remain separate units.
```

## Fallible Proxy ownKeys validation frames

A trapped `ownKeys` layer cannot validate target invariants until the innermost
target key list is known. After that layer has collected and duplicate-checked
its trap result and completed `IsExtensible(target)`, the VM first requests
capacity for one additional pending frame. It then reserves the GC roots
required by the frame's `object` and `target`, pins the same two values, and
publishes the frame. No fallible operation remains between pinning and push.

Frame or root reservation failure occurs before `current` advances to the
target, so nested `[[OwnPropertyKeys]]` and later descriptor/invariant work do
not begin. Every already published outer frame remains covered by
`pending_pins` and is unwound with the operation root. Transparent forwarding
does not create a frame; a trapped empty result still does because omission and
non-extensible exact-set invariants remain applicable.

```text
[Decision Log]
- 목적과 의도: Make Proxy ownKeys pending validation-frame publication allocator-fallible and atomic without changing trap, invariant, nested traversal, or retry order.
- 기존 구현 및 제약 조건: Each validating Proxy layer pinned current and target through infallible gc_pins growth and then pushed a PendingProxyKeys frame through infallible Vec growth. Frames must survive later nested ownKeys calls, forced GC, and reverse invariant validation, while transparent layers require no frame.
- 검토한 주요 대안: Reserve roots before frame capacity, pin before either reserve, preallocate a fixed maximum chain, store only heap indices, merge frame state into recursive calls, or combine every remaining ownKeys root and result collection in this patch.
- 선택한 방식: After successful trap-list, duplicate, and IsExtensible work, request capacity for one additional frame, reserve roots from the exact current/target Value pair, pin that pair, push the frame, and only then advance current and filters.
- 다른 대안 대신 이 방식을 선택한 이유: Reserving roots first can leave unnecessary global capacity after local frame failure; pinning before reserve can leak on allocation error; fixed caps reject valid chains; untraced indices can become stale after GC; recursion risks host-stack failure; and broader ownership would obscure this publication boundary.
- 장점, 단점 및 영향: Frame and root failures are catchable, Realm-correct, retryable, and leak-free after all earlier observations but before nested target work. Nested countdown and 1,024-layer trapped-chain tests cover existing-frame cleanup and iterative growth. Operation input, target/handler, trap-result list, and length-value roots, filtered vectors, non-extensible target sets, index strings, PropertyKey/Error strings, GC root enumeration, and mark worklists remain separate units.
```

## Fallible Proxy ownKeys direct roots

The iterative `[[OwnPropertyKeys]]` operation now reserves each root set at the
boundary where it becomes owned. The input is reserved before its first pin.
After a Proxy is proven live, its target and handler are reserved after the
Proxy-edge fuel debit and before trap lookup. A trap result is first validated
as an object and then reserved before reading `length`; an object-valued length
is reserved after `Get` and before `ToNumber` can invoke user code.

One shared helper counts the roots contributed by each `Value` before touching
the GC-pin vector. Primitive inputs and lengths therefore perform no reserve,
and a missing or nullish trap forwards without creating trap-result state.
Operation-root failure precedes revocation and fuel because the operation must
own its input before dispatch, while layer-root failure follows revocation and
the edge fuel debit. Ordinary result cleanup unwinds every pin and previously
published validation frame when any later reservation fails.

```text
[Decision Log]
- 목적과 의도: Make every temporary GC root directly owned by Proxy ownKeys allocator-fallible at the exact point where the operation assumes ownership.
- 기존 구현 및 제약 조건: The operation input, Proxy target/handler, trap-result object, and object-valued length were pinned through infallible gc_pins growth. Their observation boundaries differ, primitive Values contribute no roots, and nested Proxy validation may already have published outer frames when an inner list or length fails.
- 검토한 주요 대안: Reserve a fixed root budget at entry, reserve every Value including primitives, move target/handler reservation before fuel, pin first and rely on cleanup, keep only injected site tests, or combine post-validation key collections and GC internals into this patch.
- 선택한 방식: Use one root-count-aware reservation helper immediately before each pin, retain the existing observation order around revocation, fuel, Call, list validation, length Get, and ToNumber, and verify both exact sites and the real GC-pin reserve path.
- 다른 대안 대신 이 방식을 선택한 이유: Entry preallocation over-reserves paths never taken and changes failure priority; reserving primitives creates spurious failures; moving layer reservation changes fuel order; pin-first can abort during vector growth; synthetic sites alone do not prove the production reserve path; and broader allocator ownership would obscure cleanup and retry evidence.
- 장점, 단점 및 영향: All directly owned ownKeys roots now fail through catchable, Realm-correct RangeError paths with primitive/nullish no-op behavior, nested cleanup, caller retry, forced-GC survival, and for-in snapshot atomicity. Root counting adds a bounded scan over at most two Values per site. Filtered output growth, non-extensible target-key sets, index and PropertyKey/Error strings, GC root enumeration, and mark worklists remain independent units.
```

## Fallible Proxy ownKeys post-validation collections

Reverse validation first observes every target descriptor and omission rule.
Only after that work succeeds does a non-extensible frame reserve its complete
target-key `IndexSet` and publish the keys used for exact-set comparison. This
keeps descriptor throws and missing non-configurable-key errors ahead of any
allocation failure while placing reservation before native set growth.

Consumer filtering remains a later pass. Each candidate consumes its existing
fuel, applies the String/Symbol filter, and optionally observes
`[[GetOwnProperty]]` and enumerability before requesting capacity. The helper
checks `len == capacity`, so an injected or real reserve failure is possible
only when the next accepted key would actually grow the result vector. A
failure discards the operation-local partial result; caller retry re-observes
the complete Proxy operation and never publishes a partial `for...in`
snapshot.

```text
[Decision Log]
- 목적과 의도: Make the two post-validation collections directly owned by Proxy ownKeys allocator-fallible without changing invariant validation, consumer filtering, or retry semantics.
- 기존 구현 및 제약 조건: The non-extensible target-key IndexSet collected a fully observed target list through infallible growth, and the filtered result Vec pushed accepted keys infallibly after per-key fuel and optional descriptor lookup. Reverse Proxy frames can re-enter descriptor traps, and for-in must not publish a partial key snapshot.
- 검토한 주요 대안: Reserve target keys before descriptor traversal, reserve the trap-result length for every frame, reserve before filtering or descriptor lookup, request capacity before every accepted push even when spare capacity exists, impose a fixed key cap, or combine shared string and GC-worklist allocation in the same patch.
- 선택한 방식: Reserve the target-key set once after all descriptor and omission validation but before insertion and exact-set comparison; reserve the filtered Vec only after a key passes every filter and only when its next push requires growth.
- 다른 대안 대신 이 방식을 선택한 이유: Earlier reservation changes abrupt-completion priority and can over-allocate discarded keys; unconditional failpoints model failures where the allocator is never called; fixed caps reject valid programs; and broader allocation ownership would obscure the reverse-frame, Realm, and atomic-retry boundary.
- 장점, 단점 및 영향: Both collections now report catchable, operation-Realm RangeError with exact no-growth exclusions, completed descriptor observations, reverse-frame cleanup, caller retry, and for-in snapshot atomicity. Target-set reservation is O(number of target keys), and filtered growth remains amortized. Shared index and PropertyKey/Error strings, ordinary own-key producers, GC root enumeration, and mark worklists remain independent units.
```

## Fallible ordinary own-key collections

`ordinary_own_property_keys` retains its up-front work charge: TypedArray
indices, stored properties, Array presence slots, Module Namespace exports,
and a String byte-length upper bound for UTF-16 keys contribute to the scan
estimate. The full precomputed work charge is consumed before native key
materialization starts. Each accepted candidate then checks the capacity of
its type-specific index, String, or Symbol staging vector and requests one
additional slot only when full.

Sorted index keys, insertion-ordered strings, and symbols are published into a
single final result. Membership is checked first; for a new key, both the
duplicate `IndexSet` and result `Vec` reserve before either is mutated. A
duplicate such as a materialized Array `length` alongside the synthetic length
therefore consumes no final reservation. Any failure discards all local state,
unwinds the operation root, and leaves a lazy `for...in` snapshot unpublished
so caller retry re-runs the ordinary key snapshot.

```text
[Decision Log]
- 목적과 의도: Make every native collection directly owned by ordinary [[OwnPropertyKeys]] allocator-fallible at its real growth boundary without changing fuel, key order, filtering, Proxy invariant, or retry semantics.
- 기존 구현 및 제약 조건: Index, String, and Symbol candidates were pushed into three infallible staging Vecs, while final deduplication inserted into an infallible IndexSet and Vec. Fuel is intentionally prepaid before materialization; Array and boxed String synthesize length, Module Namespace exports have specified ordering, and a pending Proxy validates descriptors only after the ordinary target snapshot succeeds.
- 검토한 주요 대안: Reserve from the precomputed work count, preallocate all five collections at entry, remove staging vectors in a broad rewrite, impose a key-count cap, combine numeric/Arc string allocation, or reserve seen and result after mutating one side.
- 선택한 방식: Check len against capacity immediately before each accepted staging push; after sorting and filtering, check final membership, reserve both seen and result when needed, then publish to both collections. Keep all string construction and caller-owned conversion containers outside this unit.
- 다른 대안 대신 이 방식을 선택한 이유: Work counts include holes, excluded keys, duplicates, and different key classes, so bulk reservation creates false failures and over-allocation; a broad rewrite obscures ordering; fixed caps reject valid programs; stable Rust has no fallible Arc<str> construction; and one-sided publication complicates atomic cleanup evidence.
- 장점, 단점 및 영향: Ordinary objects, Arrays, primitive and boxed Strings, TypedArrays, Symbols, and Module Namespace exports now share catchable, Realm-correct growth failure with exact fuel, duplicate, no-op, Proxy-order, retry, and for-in atomicity evidence. The three staging vectors plus final Vec/IndexSet remain O(number of candidates). Numeric formatting, PropertyKey/Error Arc strings, caller result containers, GC root enumeration, and mark worklists remain independent scopes.
```

## Fallible own-key consumer materialization

The six public key-array consumers own a second publication layer after
`[[OwnPropertyKeys]]`: `Object.keys`, `Object.values`, `Object.entries`,
`Object.getOwnPropertyNames`, `Object.getOwnPropertySymbols`, and
`Reflect.ownKeys`. Each accepted result now requests vector capacity only when
full. Keys reserve after their enumerable descriptor succeeds; values reserve
after `Get`, then reserve GC-pin capacity before `pin -> push`; entries reserve
their two pair elements after `Get`, create the pair in the method Realm, then
reserve the outer vector and pair root before publication.

Names, Symbols, and Reflect conversion perform no descriptor or value access.
Empty and filtered lists therefore create an empty Array without consuming a
result or presence reservation. Non-empty result Arrays use
`ArrayData::try_new`, which reserves the dense presence bitmap before resize.
`make_value_array_in_env` computes and reserves every object root before
pinning, and all Realm-explicit callers delegate to that path. Consequently a
foreign `Reflect.ownKeys` now receives its own Realm's `%Array.prototype%`.

```text
[Decision Log]
- 목적과 의도: Make the complete result-materialization layer of the six public own-key consumers catchable and Realm-correct without changing snapshot, descriptor, Get, ordering, or partial-observation semantics.
- 기존 구현 및 제약 조건: The producer snapshot was fallible, but consumer Vec growth, Object.entries pair storage, some pins, a temporary root Vec, and ArrayData's presence bitmap were infallible. Reflect.ownKeys used the main Realm Array, while keys/values/entries must observe descriptors and Gets in snapshot order.
- 검토한 주요 대안: Preallocate from snapshot length, reserve before descriptor/Get, share one bulk conversion for all consumers, leave Array presence as hard OOM, clone entry key strings, or rewrite every ArrayData constructor and own-key caller together.
- 선택한 방식: Reserve each accepted consumer publication at its actual growth boundary; reserve object roots before pinning; treat entries pair elements and outer pairs independently; route Realm-explicit arrays through make_value_array_in_env; and add a fallible dense-presence constructor used by that shared Value-array path.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot length overallocates filtered results and changes failure priority; early reservation precedes required observable work; values and entries need different root lifetimes; leaving the bitmap would preserve an end-to-end abort; key ownership removes an unnecessary Arc allocation; and a global Array/caller rewrite exceeds this reviewable boundary.
- 장점, 단점 및 영향: All six APIs now have exact growth, presence, Realm, retry, fuel, GC, and cleanup evidence, and shared Value-array callers gain reserve-before-pin/presence safety. Descriptor result materialization, JSON and descriptor-related own-key caller containers, unrelated direct ArrayData::new constructors, PropertyKey/Error strings, GC root enumeration, and mark worklists remain independent scopes.
```

## Fallible Proxy descriptor traversal state

`own_property_descriptor_for_key_or_throw` evaluates a Proxy
`getOwnPropertyDescriptor` chain iteratively. It now reserves its operation
input before the first pin, each target/handler layer after revocation and the
edge fuel debit, and a callable trap after `GetMethod` and callability
validation. A trapped layer reserves pending-frame capacity only when the
vector is full, then reserves every frame root before publishing the frame.
Transparent forwarding therefore creates neither trap nor pending-frame
state.

Descriptor conversion reserves the returned descriptor object and each
object-valued `value`, `get`, and `set` field at the point ownership begins.
Getter and setter callability errors remain ahead of their root reservations.
Target descriptor fields use a fixed three-value root set across observable
`IsExtensible` work. On the `undefined` trap-result path, an absent target
descriptor returns immediately, a hidden non-configurable descriptor throws
immediately, and only a configurable target descriptor retains fields across
extensibility observation. Reverse validation still processes outer trapped
layers only after the terminal descriptor is known, and every abrupt
completion unwinds operation-local roots and frames.

```text
[Decision Log]
- 목적과 의도: Make every native collection and temporary GC root directly owned by iterative Proxy getOwnPropertyDescriptor traversal allocator-fallible without changing trap, descriptor conversion, invariant, or caller observation order.
- 기존 구현 및 제약 조건: Operation, layer, trap, pending-frame, descriptor-conversion, and validation values were pinned or pushed through infallible Vec growth; nested frames validate in reverse; descriptor Get and IsExtensible can execute user code; and several primitive or terminal error paths need no retained root.
- 검토한 주요 대안: Reserve a maximum root budget at entry, preallocate pending frames from Proxy depth, root every primitive field, reserve before callability or invariant checks, replace all descriptor materialization and callers in one patch, or impose a fixed Proxy-depth limit.
- 선택한 방식: Reserve each root at its semantic ownership boundary, reserve pending storage only when full and before any frame root or publication, retain target descriptor fields in a fixed array across IsExtensible, skip trap/pending sites for transparent forwarding and field-root sites for primitives, and skip validation-descriptor roots for absent or immediately non-configurable targets on the undefined trap-result path.
- 다른 대안 대신 이 방식을 선택한 이유: Entry reservation and primitive rooting introduce false failures; Proxy depth is not known before observable traversal; early reservation changes required error priority; a fixed depth rejects valid programs; and final descriptor objects and caller containers have separate allocation and Realm boundaries.
- 장점, 단점 및 영향: All ten sites now have catchable Realm-correct failure, actual-growth, ordering, nested reverse-validation, GC, retry, and cleanup evidence while successful semantics and Test262 admission remain unchanged. Final FromPropertyDescriptor object construction, Object.getOwnPropertyDescriptors and defineProperties containers, Proxy defineProperty descriptor containers, shared strings, GC root enumeration, and mark worklists remain independent scopes.
```

## Fallible descriptor materialization and definition publication

`ToPropertyDescriptor` now produces a presence-aware internal descriptor
record directly. It retains the input object while observing inherited
`enumerable`, `configurable`, `value`, `writable`, `get`, and `set` fields in
specification order, and roots each newly observed object-valued data or
accessor field before a later callback can collect it. Getter and setter
callability checks remain ahead of publication. This removes the temporary
ordinary descriptor object that previously required a second property walk
and could change observable results between conversion and definition.

`Object.defineProperties` stores the converted records and their field roots
in a fallibly grown operation-local vector, completing every descriptor
conversion before the first target definition. `Object.defineProperty` and
`Reflect.defineProperty` validate the target before reserving their argument
roots, convert once, and pass the same record to ordinary, Array,
TypedArray, Module Namespace, mapped-arguments, or Proxy definition. A Proxy
creates its descriptor object only after revocation, fuel, trap lookup, and
callability succeed; the exact present-property count is reserved before the
object is allocated, and target descriptor fields remain rooted across
invariant validation.

`FromPropertyDescriptor` reserves its fixed four-property map and exact value,
getter, setter, and Realm-prototype roots before allocation.
`Object.getOwnPropertyDescriptors` obtains `[[OwnPropertyKeys]]` before
allocating the result object, then reserves each accepted result property and
materialized descriptor root before publication. Both paths require the
current Realm's registered `%Object.prototype%` rather than silently falling
back to the main Realm.

```text
[Decision Log]
- 목적과 의도: Make descriptor conversion, object materialization, public Object/Reflect callers, and Proxy defineProperty publication allocator-fallible while preserving ECMAScript field observation, two-pass definition, invariant, Realm, and retry semantics.
- 기존 구현 및 제약 조건: ToPropertyDescriptor first allocated a normalized JS object and later reread its own properties; Object.defineProperties retained those objects in an infallible Vec; FromPropertyDescriptor and getOwnPropertyDescriptors inserted through infallible maps; Proxy defineProperty eagerly reused or built descriptor objects through infallible property insertion. Every field Get, ownKeys trap, defineProperty trap, and invariant check can execute user code and trigger GC.
- 검토한 주요 대안: Preallocate from own-key counts, keep the normalized object as the internal record, reserve all roots and properties at public-call entry, create Proxy descriptor objects before trap lookup, impose fixed descriptor or key limits, or combine ordinary property storage, JSON, shared strings, and GC worklists in the same patch.
- 선택한 방식: Convert once into a presence-aware Rust record in specification field order; reserve and root observed object fields at ownership boundaries; retain records through the complete first defineProperties pass; reserve maps and vectors only at actual growth; materialize Realm-correct descriptor objects only when their caller requires them; and allocate getOwnPropertyDescriptors output only after ownKeys succeeds.
- 다른 대안 대신 이 방식을 선택한 이유: Rereading a normalized object duplicates observable semantics and allocation; count-based entry reservation over-allocates filtered or absent fields and changes abrupt-completion priority; eager Proxy materialization runs before required trap errors; fixed limits reject valid programs; and broader container ownership would obscure the conversion/publication boundary.
- 장점, 단점 및 영향: Descriptor fields now have one observable conversion, the defineProperties conversion pass completes before the first target mutation, public and Proxy output is Realm-correct, and every directly owned root or collection has exact growth, GC, cleanup, and retry evidence. Ordinary object property-map insertion, Array backing and length storage, ordinary Set/set_array_index, seal/freeze materialization, TypedArray byte conversion, JSON containers, unrelated ArrayData constructors, PropertyKey/Error and IC temporary strings, GC root enumeration, and mark worklists remain independent hard-host-OOM scopes.
```

## Fallible ordinary property storage publication

Complete VM descriptors and presence-aware Object/Reflect descriptor records
now converge on one ordinary storage plan. The plan classifies publication as
a virtual String no-op, ordinary property, dense Array element, custom Array
element, or custom arguments element. It derives map ownership from the chosen
representation rather than descriptor presence, so normal dense elements do
not reserve `props`, mapped arguments reserve both their descriptor map and
dense vectors, and replacing an existing key never reserves map growth.

Preflight checks real `IndexMap` and `Vec` capacity in fixed `props`, `items`,
then `present` order. It reserves every required directly owned container
before changing values, presence bits, `sparse_max`, Array length, mapped
bindings, or inline caches. Dense values needed after resize are cloned before
commit. Publication then moves prepared values into storage and performs
length, mapping, and cache maintenance only after direct storage succeeds.

Transparent Proxy traversal returns a resolved target. `Vm::define_own_property`
reserves and pins that target plus descriptor value/get/set fields through the
remaining exotic coercion and publication. This is required for TypedArray
`ToNumber`/`ToBigInt`, which can run JavaScript and collect an otherwise
unpublished target. Integer-indexed TypedArray keys bypass ordinary storage;
Module Namespace exports use complete-descriptor `SameValue` validation only
for String keys, while Symbol keys retain ordinary semantics.

```text
[Decision Log]
- 목적과 의도: Make ordinary property-map and Array backing publication catchable and atomic at actual native-container growth while aligning direct exotic definition and GC lifetime with ECMAScript semantics.
- 기존 구현 및 제약 조건: Object/Reflect and direct VM paths duplicated infallible map/vector mutation; Array representation depends on descriptor shape; mapped arguments have post-publication parameter effects; transparent Proxy targets and descriptor objects can otherwise become unrooted before TypedArray coercion; String and Module Namespace properties may be virtual.
- 검토한 주요 대안: Reserve every object map and Array vector at entry, keep duplicated publishers, preallocate from the numeric index, materialize boxed String properties, route TypedArray indices through ordinary maps, or combine Array length, ordinary Set, shared strings, byte conversion, and GC worklists into one change.
- 선택한 방식: Build one representation-aware plan, reserve only actual `props`/`items`/`present` growth in deterministic order, prepare owned dense values before commit, publish representation state once, delay mapped bindings and cache work, pin resolved targets and descriptor fields across exotic coercion, and retain virtual/exotic no-storage paths.
- 다른 대안 대신 이 방식을 선택한 이유: Entry reservation creates false failures; duplicated mutation drifts semantically; index-sized preallocation can over-allocate or reject valid sparse definitions; virtual materialization changes observable storage; ordinary TypedArray maps violate integer-indexed semantics; and the remaining host-allocation owners have independent rollback and ordering contracts.
- 장점, 단점 및 영향: Direct containers now have exact growth, atomic failure, Realm, retry, partial-operation, Proxy priority, and forced-GC evidence. The guarantee intentionally excludes shared key/value representation, boxed-String canonicalization, Array length-key maintenance, inline-cache temporary strings, TypedArray byte vectors, ordinary Set/set_array_index, seal/freeze, JSON, direct ArrayData constructors, GC root enumeration, and mark worklists.
```

## Fallible Array length mutation

`ArraySetLength` reserves and pins its target/value roots before both observable
numeric conversions, then reads the old descriptor. After validation, it
reserves only actual growth of the `props` map, dense `items`, and `present`
bitmap before any directly owned Array state changes. A VM-owned canonical
`PropertyKey` supplies `length`, so the operation does not allocate a fresh
shared String key.

Shrink needs no deletion scratch collection. One linear scan finds the highest
non-configurable indexed property at or above the requested length. A single
`IndexMap::retain` removes configurable indexed properties above the effective
length, after which dense storage is truncated only when it already covers
that range. This is equivalent to descending deletion: a blocker restores
length to its index plus one, lower indices remain untouched, higher
configurable indices are gone, and a requested `writable: false` is applied
only after rollback.

Sparse representation is preserved when dense backing is shorter than the
logical length. Shrink, blocked rollback, equal-length definition, and growth
therefore update logical metadata without materializing holes. Virtual
`length` also remains virtual for value changes and `writable: true`; storage
is created only when `writable: false` must persist.

```text
[Decision Log]
- 목적과 의도: Make Array length mutation allocator-fallible and representation-safe while preserving ArraySetLength conversion, deletion, rollback, writability, Realm, fuel, Proxy, and GC semantics.
- 기존 구현 및 제약 조건: Length conversion can execute JavaScript twice; shrinking previously built deletion scratch vectors, sparse rollback could resize dense storage to a large blocker index, virtual length was materialized on ordinary updates, and direct map/vector growth was infallible. Partial deletion at a non-configurable index is required behavior rather than an atomic transaction.
- 검토한 주요 대안: Collect every deletion key, delete through repeated map removal, preallocate dense storage to the logical length, always materialize the length descriptor, reserve all containers at entry, or combine ordinary Set, index-key strings, and inline-cache allocation in the same patch.
- 선택한 방식: Root conversion operands before observable work; preflight only actual directly owned growth; scan once for the highest blocker; remove eligible custom indices with one retain pass; truncate dense backing only within its existing range; keep sparse and virtual state unmaterialized; and use one VM-owned canonical length key.
- 다른 대안 대신 이 방식을 선택한 이유: Scratch keys and repeated removal add fallible allocation or quadratic work; logical-length dense allocation is unbounded and changes representation; eager descriptor materialization creates false failures; entry reservation changes abrupt-completion priority; and index Set plus cache invalidation have separate publication contracts.
- 장점, 단점 및 영향: Array length mutation now has bounded linear shrink work, exact growth reservations, no deletion scratch allocation, sparse-storage preservation, Realm-correct catchable failure, balanced cleanup, and retry evidence. Required partial deletion remains observable. Ordinary Set/set_array_index, numeric and cache temporary strings, seal/freeze, shared strings, TypedArray byte conversion, JSON containers, direct ArrayData constructors, GC root enumeration, and mark worklists remain independent scopes.
```

## Fallible Array index Set and borrowed inline cache

Direct Array index assignment has a dedicated `Set` storage-planning mode that
shares the ordinary publisher's `props`, `items`, and `present` preflight and
commit machinery. It reuses the caller's existing `PropertyKey`, so sparse Set
does not format or allocate a second numeric key. Regular default data indices
use dense storage below the cap, custom descriptors retain their attributes,
and sparse indices publish one property plus `sparse_max`. Arguments values
within existing dense backing remain unmaterialized unless a custom descriptor
already exists; out-of-backing indices use ordinary property storage without
changing the ordinary Arguments `length` property.

Prototype traversal, Proxy traps, extensibility, and non-writable Array
`length` checks complete before storage preflight. Successful publication then
synchronizes logical Array length, but invalidates a materialized length cache
entry only when its value changed. Mapped Arguments performs its parameter-map
pre-update before traversal allocation at the first same-receiver `[[Set]]`
entry and again at every recursive entry reached through ordinary or
transparent Proxy forwarding. The receiver's Arguments
`[[DefineOwnProperty]]` post-update occurs only after successful storage. An
abrupt publication therefore retains earlier `[[Set]]` and observable Proxy
effects without publishing the indexed property.

The inline cache stores one nested String-key map per object and maintains an
exact global entry count. `ic_get` and `ic_invalidate` use borrowed `&str`
lookups, invalidation prunes empty object buckets, and every GC-related clear
resets the count. New cache entries fallibly prepare their owned key and actual
outer/inner map growth; failure simply skips this optional optimization.
Overwrite uses existing capacity. At the 4,096-entry cap, the next entry's
replacement bucket is fully reserved before the old cache is cleared; a failed
reservation therefore preserves all existing entries.

```text
[Decision Log]
- 목적과 의도: Make direct Array index Set publication allocator-fallible and representation-safe while removing temporary inline-cache lookup/invalidation allocation without changing Array, Arguments, Proxy, Realm, or abrupt-completion semantics.
- 기존 구현 및 제약 조건: Three direct Set paths grew Array maps/vectors inside mutation, sparse Set formatted a second key, mapped bindings could change on the wrong side of failure, length synchronization invalidated unchanged descriptors, and every cache lookup/invalidation allocated a String. Arguments [[Set]] pre-updates its map before ordinary traversal, while receiver Arguments [[DefineOwnProperty]] post-updates only after successful definition and transparent Proxy recursion can enter both operations repeatedly.
- 검토한 주요 대안: Keep set_array_index with local reserves, route every Array write through full generic traversal, materialize every Arguments index descriptor, update mapped bindings only before or only after publication, retain one tuple-key cache with temporary Strings, scan the cache on invalidation, or make cache allocation observable errors.
- 선택한 방식: Add a Set-specific representation plan over the shared fallible publisher; reuse the existing PropertyKey; preserve default dense and custom/sparse representations; rerun mapped preambles at each same-receiver Set entry and post-update after receiver publication; update length only when changed; group cache entries by object with a counted 4,096 cap; and make cache insertion best-effort fallible while borrowing lookup/invalidation keys.
- 다른 대안 대신 이 방식을 선택한 이유: Local reserve logic would duplicate representation rules; fully generic traversal adds hot-path overhead; eager Arguments descriptors create false allocation and storage; one-sided mapping order violates one of the two exotic operations; tuple lookup requires allocation; cache scans are O(n); and an optimization must not turn a successful JS read into an abrupt completion.
- 장점, 단점 및 영향: Direct dense, custom, sparse, mapped-Arguments, transparent/completed Proxy, Realm, retry, and cache-cap paths now have deterministic allocation and ordering evidence. Borrowed cache access removes per-hit/per-write temporary allocation, and release workload comparison found no regression. Initial PropertyKey/shared String creation, ordinary non-index Set publication, seal/freeze, TypedArray byte conversion, JSON containers, unrelated ArrayData constructors, PropertyKey/Error strings, GC root enumeration, and mark worklists remain independent scopes.
```

## Fallible ordinary non-index Set receiver publication

`OrdinarySetWithOwnDescriptor` now publishes ordinary non-index receiver data
through the same Set-mode storage planner as Array indices. A missing property
reserves actual `props` growth before mutation; an existing writable data
descriptor retains its attributes and replaces only its value. The publisher
invalidates a String-key cache entry after commit through borrowed `&str`, so
allocation failure leaves both property state and cache state untouched.

Direct non-Proxy receivers retain their exotic boundaries before storage
planning. TypedArray canonical numeric indices use element definition, Array
length and indices use their dedicated publishers, and Module Namespace String
exports compare the requested value with the live binding using `SameValue`.
Boxed String virtual `length` and UTF-16 indices are recognized directly as
non-writable, avoiding both a false ordinary-map shadow and allocation of a
temporary character descriptor. Proxy receivers continue through their full
`[[GetOwnProperty]]` and `[[DefineOwnProperty]]` paths before reaching an
ordinary target.

```text
[Decision Log]
- 목적과 의도: Make ordinary non-index Set receiver publication catchable and atomic at native map growth while preserving receiver exotic methods, abrupt order, descriptor attributes, Realm identity, and hot-path cache behavior.
- 기존 구현 및 제약 조건: The final ordinary receiver branch cloned a cache String and inserted into props without reserve; it inspected only materialized props, so boxed String indices could be shadowed; Module Namespace exports always rejected receiver Set even when a value-only descriptor was SameValue; and Proxy, TypedArray, Array, global, strictness, fuel, and mapped-Arguments paths already had established order.
- 검토한 주요 대안: Add a local try_reserve around the manual insert, call the full Proxy-aware descriptor operation for every direct receiver, materialize String virtual descriptors, reject every Namespace write, route all Set forms through generic DefineOwnProperty, or extend the shared Set planner only after exotic classification.
- 선택한 방식: Classify Proxy, TypedArray, Array, Namespace, and boxed String receiver behavior first; preserve or create one complete ordinary data descriptor; publish it through Set-mode actual-growth preflight; and borrow the existing key for post-commit cache invalidation.
- 다른 대안 대신 이 방식을 선택한 이유: Local reserve logic would duplicate the publisher; full descriptor dispatch adds roots, fuel, and false allocation to ordinary receivers; String materialization allocates unnecessary value data; blanket Namespace rejection violates value-only SameValue; and fully generic definition adds hot-path work while duplicating already-correct exotic routing.
- 장점, 단점 및 영향: Full/spare/existing map, retry, cache, Proxy/fuel, global, Realm, String UTF-16, Namespace NaN/signed-zero, and cleanup boundaries now have deterministic evidence. Receiver overwrite/create benchmarks show no measured regression. Initial PropertyKey/shared String creation, seal/freeze materialization, TypedArray byte conversion, JSON containers, unrelated ArrayData constructors, Error strings, GC root enumeration, and mark worklists remain separate scopes.
```

## `Object.fromEntries` iterator pipeline

`Object.fromEntries` creates its Realm-local ordinary result before acquiring
the source iterator, then retains the iterator object and its cached `next`
method for the whole loop. Each yielded entry stays rooted while property `0`
and property `1` are read and while the key is converted. Result properties
flow through the ordinary fallible define publisher, preserving insertion
order and bypassing inherited setters.

The iterator-step boundary is deliberate. Errors from calling `next`,
validating its result, or reading `done`/`value` propagate without
`IteratorClose`. Once a value has been yielded, a primitive entry, abrupt
entry getter, key conversion, or result publication closes the iterator while
preserving the original throw over a catchable close failure. Non-catchable
host Fuel aborts never re-enter user `return` code. Temporary roots are
released through one boundary around each entry and one around the full
iterator record.

```text
[Decision Log]
- 목적과 의도: Implement the complete AddEntriesFromIterable behavior for Object.fromEntries, including observable order, IteratorClose boundaries, Realm provenance, GC safety, and fallible result publication.
- 기존 구현 및 제약 조건: The previous implementation allocated the correct result and coerced Array entries, but cloned only ArrayData.items, treated every other iterable as empty, bypassed Symbol.iterator and next, could not close iterators, and inserted result properties directly into IndexMap. Observable getters and conversions can allocate or force GC, while property growth can fail.
- 검토한 주요 대안: Extend the Array snapshot with array-like fallback, reuse Vm::make_iterator, build a hidden JavaScript adder function literally, iterate entry objects themselves, or use the existing direct iterator-record and ordinary property helpers.
- 선택한 방식: Reserve input/result roots before allocation, create the result first, acquire a direct synchronous iterator record with one Symbol.iterator Get and cached next, use the shared iterator-step helper, root each yielded entry/key/value, apply Get(0), Get(1), ToPropertyKey, and fallible DefineOwnProperty in order, close catchable post-step entry-processing errors through the shared abrupt-completion-preserving helper, and propagate host Fuel without user cleanup re-entry.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshots and array-like fallback do not implement the iterable contract; Vm::make_iterator adds an observable HasProperty probe for ordinary objects; a materialized hidden adder adds heap and rooting complexity without an observable function identity; iterating entries violates indexed Get semantics; and raw map insertion bypasses reservation failure and shared descriptor behavior.
- 장점, 단점 및 영향: Arbitrary iterables, exact ordering, duplicate-key order, Symbol keys, define semantics, close precedence, foreign Realms, forced GC, and retryable reservation failures now have direct evidence. The loop consumes one native fuel unit per iterator step through the shared helper; Object.groupBy is documented in its completed pipeline below.
```

## `Object.groupBy` iterator and grouping pipeline

`Object.groupBy` validates inputs before acquiring one direct synchronous
iterator record. The cached `next` method is called without arguments and each
step consumes native fuel. A yielded value and callback result are rooted
through callback execution and `ToPropertyKey`; every accumulated value stays
rooted until its Realm-local group Array is published on the null-prototype
result. The result is created only after iteration completes, as required by
`GroupBy` followed by `Object.groupBy`.

Errors from `next`, result validation, `done`, or `value` propagate without
closing. Catchable callback, key conversion, safe-index-limit, and host group
storage errors close the active iterator while preserving the original abrupt
completion. Native errors become rooted method-Realm error objects before
`return`, so close-time heap-limit changes cannot alter later materialization.
A non-catchable Fuel abort never invokes user `return`. Group map
and element vectors reserve before mutation; result properties use the
fallible ordinary define path after the iterator is already complete.
Materialization consumes one fuel unit per group. Because temporary roots are
LIFO, cleanup keeps iterator roots beneath accumulated-value roots until every
result Array/property operation finishes, then releases values before the
iterator record.

```text
[Decision Log]
- 목적과 의도: Implement the complete property-key GroupBy and Object.groupBy algorithms with exact iterator observability, abrupt-completion boundaries, Realm provenance, GC safety, and fallible host storage.
- 기존 구현 및 제약 조건: The previous implementation wrapped Vm::make_iterator, which added HasProperty, bypassed overrides on several built-in iterables, and called next(undefined). Callback/key errors used a normal-close helper that could replace the original throw and re-enter cleanup on Fuel. Host grouping vectors and result properties grew through unchecked or raw insertion paths.
- 검토한 주요 대안: Patch Vm::make_iterator globally, retain the wrapper and special-case close precedence, share a new generic GroupBy abstraction with Map.groupBy immediately, or use the existing direct iterator record and add Object-specific fallible group storage.
- 선택한 방식: Pin items and callback, acquire and pin a direct iterator record, step through the shared zero-argument metered helper, root each value/key and every accumulated value, materialize native errors in the method Realm before closing catchable post-step grouping failures through the original-completion-preserving helper, reserve group/element storage before mutation, then meter each output group, build Realm-local Arrays, and publish them through DefineOwnProperty after normal iterator completion. Preserve LIFO root order by releasing accumulated values before the iterator record.
- 다른 대안 대신 이 방식을 선택한 이유: Global iterator-wrapper changes have a much larger behavioral surface; wrapper special cases retain its wrong observability; combining Map.groupBy would mix collection-key and property-key semantics into this narrow unit. The direct helpers already encode the required step and close boundaries used by Object.fromEntries.
- 장점, 단점 및 영향: Proxy order, next arity/cache, built-in iterator overrides, close precedence, step/output Fuel, safe-index limit, forced heap-cap GC, allocation failure, clean retry, descriptors, null prototype, and foreign-Realm Arrays/errors now have direct evidence. Accumulated values and the completed iterator record remain pinned until output publication, trading root-set size for explicit GC correctness. The adjacent Map.groupBy collection-key pipeline is documented below.
```

## Map constructor iterator, Realm, and storage pipeline

The Map constructor allocates through the VM's GC-retrying path after rooting
the selected `newTarget` prototype and iterable. A non-null iterable causes one
instance `set` lookup before iterator acquisition. The constructor pins that
cached adder, obtains a direct synchronous iterator record through ordinary
`@@iterator` Get, pins its iterator and cached `next`, and calls `next` with no
arguments through the shared metered step helper. This removes the old wrapper
allocation, observable Proxy `has` trap, built-in iterator fast-path bypasses,
and implicit `undefined` argument.

Iterator-step/result/done/value failures propagate without close. Once a value
has been produced, primitive entries and catchable `0`/`1`/adder failures run
IteratorClose while retaining the original completion. Native errors are
materialized in the constructor Realm before user `return` can run; host Fuel
does not re-enter cleanup, while close-time Fuel still overrides a catchable
JavaScript throw. The result Map, iterable, adder, iterator record, entry, key,
and value follow nested LIFO root lifetimes across every observable operation.

Native Map insertion canonicalizes through `MapKey`, checks whether a new slot
is required, reserves `IndexMap` capacity, and only then mutates `[[MapData]]`.
This shared fallible path covers `Map.prototype.set`, constructor population,
and both upsert methods without charging replacements for unused capacity.

```text
[Decision Log]
- 목적과 의도: Complete the Map constructor's iterable pipeline with exact iterator observability, close priority, Realm provenance, GC safety, Fuel bounds, and fallible internal storage.
- 기존 구현 및 제약 조건: Vm::make_iterator added a Proxy HasProperty probe, bypassed Map/Set/generator iterator overrides, allocated a wrapper, and called next(undefined). The result Map, adder, iterator, entry, key, value, and original throw were incompletely rooted; host Fuel could invoke user cleanup; raw heap allocation and IndexMap insertion bypassed retry/reservation boundaries. Forced Test262 was green but omitted these behaviors.
- 검토한 주요 대안: Patch Vm::make_iterator globally, special-case its wrapper for Map, insert directly without the observable adder, retain infallible Map storage, or reuse the direct iterator and close helpers already proven by GroupBy/Object.fromEntries.
- 선택한 방식: Root prototype/iterable before VM allocation, reserve the maximum live root slots before observation, cache and pin the instance adder plus direct iterator record, use zero-argument metered steps, close only catchable post-step failures after Realm materialization, and route native Map insertion through reserve-before-mutation storage.
- 다른 대안 대신 이 방식을 선택한 이유: A global iterator rewrite changes many unfinished consumers; wrapper exceptions retain wrong observability and allocation; bypassing the adder violates Map construction; unchecked insertion can abort the host. Shared direct helpers preserve the specification boundary with a narrow collection-local change.
- 장점, 단점 및 영향: Direct evidence covers Proxy order, next/adder cache and arity, Array/Map/Set/generator overrides, step non-close, entry/adder close priority, foreign errors, close-time GC, host Fuel, root and entry reservation, pin restoration, and retry. Each active constructor reserves a small fixed temporary-root budget; replacements avoid unnecessary MapData growth. Set/WeakMap/WeakSet constructor wrappers remain separate later audits.
```

## Set constructor iterator, Realm, and storage pipeline

The Set constructor follows the same direct synchronous iterator boundary as
Map, but each successful step passes one rooted value to the cached observable
`add` method. Prototype, iterable, result Set, adder, iterator, cached `next`,
and current value have explicit nested root lifetimes. Iterator-step failures
propagate without close; catchable adder failures materialize native errors in
the constructor Realm and close while preserving the original completion.
Host Fuel never enters user cleanup, while a close-time Fuel failure overrides
the catchable completion that initiated close.

Set, Set prototype, and Set Iterator prototype are installed per Realm and
stored in GC-rooted transactional registries. Constructor fallback and iterator
creation use those immutable identities after observable global/prototype links
are replaced or deleted. Failed provisional Realm construction removes both
new registries with the rest of the intrinsic graph.

Native `Set.prototype.add` canonicalizes through `MapKey`, checks whether the
key is new, reserves both ordered-slot and hash-index capacity, and mutates only
after reservation. Duplicate insertion needs no capacity and therefore cannot
fail at that boundary. The constructor unit originally left composition as a
separate audit; the later Set algebra section below now completes that shared
storage, iterator, root, Realm, and Fuel pipeline.

```text
[Decision Log]
- 목적과 의도: Complete the Set constructor iterable pipeline with exact iterator observability, close priority, Realm provenance, GC safety, Fuel bounds, and atomic native insertion.
- 기존 구현 및 제약 조건: The constructor used Vm::make_iterator, adding Proxy HasProperty, bypassing built-in overrides, allocating a wrapper, and calling next(undefined). Set/result/adder/iterator/value roots were incomplete, close could replace the original throw, Set allocation and IndexSet growth bypassed retry/reservation, and created Realms had no Set or Set Iterator intrinsics.
- 검토한 주요 대안: Patch the global wrapper, clone Map behavior without Realm registries, harden all Set algebra storage in the same unit, or reuse the proven direct iterator/close helpers and isolate constructor/native-add storage.
- 선택한 방식: Install Realm-local Set and Set Iterator intrinsics, root prototype/iterable before VM allocation, reserve the maximum live constructor roots, cache the adder and direct iterator record, use zero-argument metered steps, close only catchable post-step adder failures, and reserve native Set storage before mutation.
- 다른 대안 대신 이 방식을 선택한 이유: Global wrapper changes affect unfinished consumers; main-Realm fallback fails foreign construction; broad Set algebra hardening adds independent iterator-close/root problems. The narrow shared helpers repair the complete constructor boundary without introducing partial composition semantics.
- 장점, 단점 및 영향: Direct evidence covers Proxy order, cache/receiver/arity, built-in overrides, close and non-close precedence, foreign Set/iterator/error identities, registry GC survival and rollback, forced GC, all root/storage failure sites, duplicate no-reserve insertion, host Fuel, exact-cap allocation, pin restoration, and retry. Set algebra and weak collections were retained as separately reviewable follow-up units and are now documented in their later sections.
```

## `Map.groupBy` iterator, Realm, and collection pipeline

`Map.groupBy` shares the direct synchronous iterator-record boundary with
`Object.groupBy`: it performs one `@@iterator` Get, caches `next`, calls it
without arguments, meters each step, checks the `2^53 - 1` limit before
advancing, and never closes for `IteratorStepValue` failures. Catchable
callback, root-reservation, and group-storage failures close while preserving
the original completion. Native Type/Range errors are materialized in the
method Realm before user `return`; non-catchable Fuel does not re-enter user
cleanup.

Keys remain full ECMAScript values. `MapKey` applies SameValueZero, including
`NaN` equality and `-0` canonicalization, without `ToPropertyKey`. Every
accumulated value and each stored object-key identity stays rooted through
output. A repeated object key releases its redundant callback root after the
existing group is found. Group maps, element vectors, result Arrays, and
result Map entries expose deterministic fallible boundaries; output consumes
one fuel unit per group and starts only after normal iterator completion.

Each Realm now installs and roots its own `%Map%`, `%Map.prototype%`, and
`%MapIteratorPrototype%`. Constructor fallback, static `groupBy`, prototype
methods, result Maps, group Arrays, iterator objects, and iterator result
objects therefore use immutable intrinsic identities from the active method
or constructor Realm rather than mutable globals. Result publication inserts
directly into internal `[[MapData]]`; overridden `Map.prototype.set`,
`Symbol.species`, and the global `Map` binding are not observed.

```text
[Decision Log]
- 목적과 의도: Implement the complete collection-key GroupBy and Map.groupBy algorithms with exact iterator observability, SameValueZero identity, Realm provenance, GC safety, and bounded host storage.
- 기존 구현 및 제약 조건: The prior implementation used Vm::make_iterator, which added an observable HasProperty probe, bypassed several built-in iterator overrides, and called next(undefined). Close errors could replace the original callback throw, Fuel could invoke cleanup, object keys and accumulated values were not fully rooted, output used the main Realm Map/Array prototypes, and raw allocation/insertion bypassed fallible resource boundaries. Test262 Realms did not install Map at all.
- 검토한 주요 대안: Patch Vm::make_iterator globally, retain the wrapper with Map-specific exceptions, coerce keys and reuse Object.groupBy storage, construct output through observable new Map/set/species paths, or use the established direct iterator helpers with Map-specific rooted storage and intrinsic registries.
- 선택한 방식: Pin inputs and the cached iterator record, step through the zero-argument metered helper, root each value and callback key, group with MapKey SameValueZero semantics, release redundant occupied-group key roots, preserve original abrupt completions through the shared close helper, meter output groups, create Realm-local Arrays and an intrinsic Map after completion, and publish through fallible internal MapData insertion. Install Realm-local Map and Map Iterator intrinsics and route constructor fallback through their immutable registries.
- 다른 대안 대신 이 방식을 선택한 이유: A global wrapper rewrite has a much larger behavior surface; wrapper exceptions retain wrong observability; property-key coercion changes Map semantics; observable constructor/set/species calls violate Map.groupBy; and main-Realm fallbacks violate method-Realm allocation. The direct helpers and explicit registries preserve the specification boundaries while keeping this unit narrow.
- 장점, 단점 및 영향: Direct evidence covers Proxy order, callback arity/receiver, Array/Map/Set/generator overrides, SameValueZero object/NaN/zero keys, close and non-close precedence, safe-index errors, step/output Fuel, root/storage/result failures, forced GC, clean retry, foreign Map/Array/iterator/error identities, immutable constructor fallback, and internal publication. Root pressure scales with retained values plus distinct object keys, which is required until output publication. Map constructor iteration remains a separate adjacent audit.
```

## Object integrity levels

`Object.seal`, `Object.freeze`, `Object.isSealed`, and `Object.isFrozen` share
the specification integrity pipeline. Proxy objects retain observable
`preventExtensions`, `ownKeys`, descriptor-trap, invariant, and define-trap
ordering. Module Namespace exports retain TDZ observation. Direct objects use
an attribute-only descriptor view because integrity checks do not consume data
values or accessor functions.

Integrity definitions carry only present `configurable: false` and, for frozen
data properties, `writable: false`. Existing ordinary descriptors update in
place. A dense Array index reserves destination property-map growth, moves its
owned value into a custom descriptor, then clears dense presence; mapped
Arguments obtains the current aliased value and detaches only after successful
publication, including indices already promoted into property storage. Array
`length` continues through ArraySetLength, including its writable bit. Deleted
Arguments `length` is not synthesized by ownKeys.

TypedArray `[[PreventExtensions]]` first applies `IsTypedArrayFixedLength`:
length-tracking views and fixed views over non-shared resizable ArrayBuffers
return false without changing extensibility. Fixed views over fixed buffers or
growable SharedArrayBuffers continue through ordinary prevention. Every
preventExtensions operation, Proxy layer, and trap root reserves before pinning.
Module Namespace binding observation follows re-export indirection with Brent
cycle detection, consumes fuel before each edge, and clones neither the binding
value nor an owned visited set.

Direct integrity predicates scan stored attributes without creating numeric
keys or descriptor records after confirming non-extensibility. Proxy, Module
Namespace, and mixed dense/custom Array states retain the common observable
path. Predicate scans consume fuel for their complete direct key work before
returning a result.

```text
[Decision Log]
- 목적과 의도: Make Object integrity operations specification-correct, allocator-aware, and fast for repeated direct predicates without hiding Proxy, Module Namespace, Array, Arguments, Realm, fuel, or required partial effects.
- 기존 구현 및 제약 조건: Separate object-kind shortcuts skipped Array length writability, synthesized deleted Arguments length, swallowed Namespace TDZ, bypassed common Proxy semantics and fuel, materialized descriptor objects through mutable Object.prototype, and cloned existing BigInt values. PreventExtensions and earlier per-key updates must remain visible after later failure.
- 검토한 주요 대안: Patch each specialized path independently, retain JavaScript descriptor objects, clone complete descriptors for every key, make all predicates collect PropertyKeys, redesign every Value as shared storage in this unit, or route every direct object through Proxy machinery.
- 선택한 방식: Share the specification-level operation; represent integrity definitions with presence-aware internal records; inspect direct descriptor attributes without values; mutate ordinary attributes in place; reserve then move dense Array values; retain observable Proxy and Namespace paths; and use a direct allocation-free predicate scan only where no JavaScript hook can observe it.
- 다른 대안 대신 이 방식을 선택한 이유: Per-kind patches had already drifted; JavaScript descriptor objects expose prototype pollution and allocate GC cells; complete descriptor cloning creates host-OOM risk for immutable BigInts; universal key collection regresses steady-state predicates; a global Value representation change is wider than this integrity unit; and direct objects need no Proxy traversal.
- 장점, 단점 및 영향: Array length, Arguments deletion/promoted mapping, Proxy descriptors, resizable-buffer TypedArray prevention, Namespace TDZ/re-export fuel, nested root reservation, map growth, retry, partial effects, exact-cap GC, large BigInt values, and repeated predicates now have coverage. Initial numeric PropertyKey/shared String creation remains infallible. Proxy traps must still observe complete descriptor values. The following shared immutable BigInt unit removes the mapped-Arguments deep clone.
```

## Shared immutable BigInt values

Runtime `Value::BigInt` payloads use `Arc<BigInt>`. JavaScript BigInts are
immutable, so property reads, constant-pool clones, boxing, mapped Arguments,
Realm crossings, and descriptor publication can share limb storage without an
observable identity change. Equality and Map/Set hashing remain value-based.
Arithmetic, bitwise, shift, and fixed-width conversions borrow both operands
and allocate only the result; no operation mutates shared storage.

The public `Vm::to_bigint() -> BigInt` API keeps its owned return contract for
embedders. Internal coercion uses a shared return type. Direct Rust construction
with the public enum payload changes from `Value::BigInt(BigInt)` to
`Value::bigint(BigInt)`, `Value::from(BigInt)`, or
`Value::BigInt(Arc<BigInt>)`. Parser AST nodes remain owned and incur one clone
while entering the constant pool; removing that compile-time copy is
independent work.

```text
[Decision Log]
- 목적과 의도: Make semantic Value duplication constant-time for multi-limb BigInts while preserving ECMAScript value semantics, worker-thread compatibility, and the existing owned embedding conversion API.
- 기존 구현 및 제약 조건: Value derived Clone over an owned BigInt, so property reads, mapped Arguments detachment, boxing, and constant-pool copies cloned limb vectors. Values cross Atomics worker boundaries and therefore must remain Send. BigInt limb allocation is outside the GC heap cap.
- 검토한 주요 대안: Keep owned BigInts, use Rc, globally intern BigInts, use Arc with copy-on-write arithmetic, introduce a Small/Shared tagged representation now, or move AST BigInts to shared storage in the same unit.
- 선택한 방식: Store immutable runtime BigInts in Arc, borrow operands for all arithmetic and binary conversion paths, allocate a fresh Arc only for new numeric results, preserve value-based equality/hash, and retain an owned public to_bigint compatibility wrapper.
- 다른 대안 대신 이 방식을 선택한 이유: Rc is not Send; global interning retains attacker-controlled values; copy-on-write makes cost depend on aliasing and can reintroduce deep clones; Small/Shared and AST migration have wider representation and performance surfaces; leaving values owned preserves a known large-clone cost.
- 장점, 단점 및 영향: Runtime clone is O(1), a 64K-digit property-read stress workload improves about 29.5%, and focused BigInt plus binary-view Test262 output is unchanged. Every BigInt still allocates an Arc control block, parser-to-constant transfer still clones once, direct enum construction is an alpha API change, and Arc/num-bigint allocation remains infallible host allocation rather than a catchable heap-limit error.
```

## Compact canonical numeric property keys

On 64-bit targets, canonical ECMAScript array-index names (`0` through
`4294967294`) are stored inside `PropertyKey` as a `u32`. Ordinary strings
remain `Arc<str>` and Symbols remain `u32` ids. A private two-variant
representation nests the index and Symbol forms together, preserving the
previous 16-byte `PropertyKey` size on x86_64. On 32-bit targets the same safe
Rust layout would grow keys from 8 to 12 bytes, so canonical indices retain the
previous Arc-backed string representation there. Both target widths therefore
keep `PropertyKey` exactly the size of `Arc<str>`.

Numeric text is formatted into a ten-byte stack view only when an API needs
string semantics, hashing, an inline-cache name, or a JavaScript String result.
Object `[[Get]]` and `[[Set]]` retain the structured key, so computed numeric
access does not format and then parse the same key before dispatch. Hash and
equality remain string-compatible, including lookup against a legacy Arc-backed
canonical string. `ownKeys` sorts inline indices numerically and materializes
JavaScript Strings only at the observable result boundary.

```text
[Decision Log]
- 목적과 의도: Remove canonical numeric PropertyKey allocation while preserving ECMAScript string identity, own-key ordering, Proxy invariants, object-map density, and ordinary string-key performance.
- 기존 구현 및 제약 조건: Every generated or Number-derived index formatted a String and allocated Arc<str>; stable Rust has no genuinely fallible Arc<str> constructor; PropertyKey is present in every property map and key worklist, so representation growth has global cost.
- 검토한 주요 대안: Keep allocation and add a test-only failure shim, claim String/Arc construction is fallible, add an 11-byte decimal enum variant, use unsafe tagged pointer storage, globally intern property strings, or nest u32 Index/Symbol forms behind a private compact representation.
- 선택한 방식: Store canonical indices as u32 in a private inline variant shared with Symbols on 64-bit targets, retain the previous Arc-backed index form on 32-bit targets, use a stack decimal view at string boundaries, preserve string-compatible Hash/Eq, and route simple object computed Get/Set directly through PropertyKey.
- 다른 대안 대신 이 방식을 선택한 이유: A failure shim does not harden real host OOM; stable Arc allocation cannot report failure; the decimal enum grew PropertyKey from 16 to 24 bytes; unsafe pointer tagging was unjustified; global interning retains attacker-controlled names; and the nested representation remains safe Rust and two words.
- 장점, 단점 및 영향: Canonical key creation and storage allocate no string on 64-bit targets, PropertyKey size is unchanged on both target widths, public representation is no longer exhaustively matchable, and own-key/JSON/Proxy boundaries retain exact text. Hashing an inline index formats at most ten stack bytes; 32-bit canonical keys, JS-visible String materialization, non-index Number formatting, Reference record boxes, and native indexed callers were explicit follow-ups. The native-caller follow-up is recorded below.
```

## Native indexed property-key pipelines

Native Array, TypedArray, call-argument, JSON, RegExp, and Proxy array-like
loops create `PropertyKey::from_integer_index` from numeric cursors when the
operation consumes a structured key. Strict `Set`, Array iterator `Get`, and
Array search `HasProperty`/`Get` instead format the cursor into an owned stack
view and enter their established string dispatch. Those exceptions preserve
specialized exotic, primitive String, receiver, strict-error, and inline-cache
behavior without a temporary Rust `String` or a prebuilt Arc-backed key.

`from_integer_index` is required rather than a narrowing `u32` cast because
generic Array methods and Array iterators can observe decimal names through
`Number.MAX_SAFE_INTEGER`. On 64-bit targets names through `4294967294` stay
inline and allocation-free. Larger integer names remain Arc-backed strings,
and 32-bit targets retain the existing Arc-backed representation for every
numeric key; both now build the Arc directly from stack digits without an
intermediate `String`. Array iterator `Get` retains the established
`get_property` call shape after a direct structured-dispatch diagnostic showed
a repeatable regression on the shared host. The diagnostic is not retained as
release benchmark evidence.

```text
[Decision Log]
- 목적과 의도: Remove temporary native-loop integer-name Strings without narrowing ECMAScript property-name range, changing observable property operations, or accepting a measured iterator regression.
- 기존 구현 및 제약 조건: Ninety native numeric property-name sites formatted a cursor with to_string before the operation. Generic Array and iterator lengths reach Number.MAX_SAFE_INTEGER, TypedArray indices are currently buffer-capped, Proxy traps observe exact String keys, and strict Set plus primitive String search have specialized dispatch and error-order paths.
- 검토한 주요 대안: Keep all formatting, cast every cursor to u32, add method-specific fast paths, route every operation through structured APIs, replace the full Set implementation, or combine structured keys with the existing dispatch shape where measurement requires it.
- 선택한 방식: Use from_integer_index for structured operations, format every u64 fallback directly into stack digits, and pass stack views to strict Set, Array iterator Get, and Array search operations that retain string dispatch.
- 다른 대안 대신 이 방식을 선택한 이유: Keeping formatting preserves avoidable host allocation; u32 casts corrupt names at 4294967295 and above; per-method shortcuts duplicate semantics; rewriting Set expands risk across exotics; and performance evidence rejected a semantically correct but slower iterator variant.
- 장점, 단점 및 영향: Native integer names avoid temporary Rust String allocation, all safe-integer names and Proxy observations remain exact, and focused 7800-file Test262 output is byte-identical. Shared-host wall-time diagnostics found no regression but are not release benchmark evidence. Above-array-index names and all 32-bit numeric keys still allocate Arc strings; wasm32 type-checks this path but does not execute 32-bit tests. JS-visible key materialization and non-index Number formatting remain separate scopes.
```

## Stack-backed non-index Number property keys

Runtime Number-to-String conversion writes into a fixed 32-byte
`NumberString` buffer. Canonical array indices still use the compact
`PropertyKey` path above; this unit covers values such as `-1`, `1.5`,
`4294967295`, `1e21`, `NaN`, and infinities. Fixed and exponential forms
preserve the previous ECMAScript-facing spelling, including signed exponent
normalization and negative-zero handling. Converting the final view to a
runtime String still allocates the required `Arc<str>`, but no temporary Rust
`String` is created first.

The 32-byte capacity has headroom over Rust's shortest finite `f64` output
and the normalized exponent sign. Boundary cases plus 20,000 deterministic
raw `f64` bit patterns compare the stack formatter against the preceding
implementation. Proxy get, set, has, and delete tests verify that observable
property names remain exact. The public `num_to_string` helper retains its
owned `String` return type for API compatibility. Parser/compiler static
numeric property names therefore inherit the stack normalization internally,
but retain their final owned `String` and existing output behavior. Object
`ToPrimitive`/Symbol ordering is unchanged.

```text
[Decision Log]
- 목적과 의도: Remove temporary heap allocation from runtime non-index Number property-key formatting without changing observable ECMAScript key text or public conversion APIs.
- 기존 구현 및 제약 조건: Runtime Number conversion formatted into an owned String and then copied into the final Arc<str>; exponential normalization created a second temporary String. Canonical indices already have a separate inline path, while JavaScript-visible Strings still require shared runtime storage.
- 검토한 주요 대안: Keep the temporary String, add a general small-string crate, intern numeric names globally, change public num_to_string to return a borrowed view, specialize parser/static-name callers separately, or broadly unbox Reference records.
- 선택한 방식: Use a fixed 32-byte fmt::Write buffer for the existing fixed/exponential algorithm, expose it only inside the crate, retain the owned public wrapper, and allocate only the final Arc<str> at the runtime String boundary.
- 다른 대안 대신 이 방식을 선택한 이유: The bounded shortest-f64 representation needs no general allocator or interner; changing the public helper expands API impact; parser callers still need owned constants; object coercion has separate ordering constraints; and broad Reference unboxing approximately doubles the common record layout on both audited target widths.
- 장점, 단점 및 영향: Non-index runtime Number keys lose one temporary allocation, or two during exponent normalization. Owned public and static-name callers retain one final String but also avoid the raw exponential temporary. Exact text, Proxy observations, wasm32 compilation, and the public API remain stable. Final Arc allocation, JavaScript-visible String materialization, and specialized Reference representation remain separate scopes.
```

## Computed read-modify-write property references

Ordinary computed compound, logical, and update expressions evaluate the base
and key, perform the required null-base check, then let `MakePropertyRef`
coerce the key directly into `PropertyKey`. The previous bytecode emitted a
standalone `ToPropertyKey` first. On 64-bit targets, canonical Numbers therefore
created an `Arc<str>` JavaScript String which `MakePropertyRef` immediately
parsed back into the inline index representation.

Simple assignment, destructuring and loop targets, and delete retain their raw
Reference paths because their specification-visible key-coercion timing is
different. `super` retains an explicit `[[ThisValue]]` and resolves its raw
name once before duplication. Private names and `with` environment References
remain separate paths.

```text
[Decision Log]
- 목적과 의도: Carry compact numeric keys through ordinary computed read-modify-write References without changing evaluation order or observable Reference behavior.
- 기존 구현 및 제약 조건: Compound, logical, and update code already retained one Reference but emitted ToPropertyKey before MakePropertyRef, allocating and reparsing canonical numeric names on 64-bit targets. Null bases must reject before coercion; object keys, Proxy traps, GC, strictness, with, super, and private names have distinct observable rules.
- 검토한 주요 대안: Change every raw Reference, remove CheckNullBase, add a new opcode, redesign boxed Reference records, or route only the three ordinary computed read-modify-write forms directly through existing MakePropertyRef.
- 선택한 방식: Remove the redundant ToPropertyKey opcode only from ordinary computed compound/logical/update lowering, retain CheckNullBase, and keep every deferred, super, private, and environment Reference path unchanged.
- 다른 대안 대신 이 방식을 선택한 이유: Existing MakePropertyRef already pins base/key and performs Symbol-preserving structured coercion; broad raw-Reference changes would alter simple-assignment/delete timing; a new opcode duplicates existing behavior; Reference boxing is a separate allocation and clone problem.
- 장점, 단점 및 영향: Canonical numeric keys avoid temporary String and Arc allocation on 64-bit targets. wasm32 removes the redundant opcode and Value::String handoff but retains its final Arc-backed numeric key allocation. Focused Test262 output is byte-identical and direct tests cover opcode shape, boundaries, null bases, Proxy order, and forced GC. Boxed Reference records, non-index key formatting, and JavaScript-visible key Strings remain; native indexed callers are covered by the following completed unit above.
```

## Borrowed Reference consumption and root traversal

`GetValue`, `PutValue`, and delete borrow their `ReferenceRecord` instead of
cloning its `Box` and every nested base, raw name, or explicit receiver.
Deferred key resolution pins the record directly. Extracting an owned receiver
or base clones only its contained `Value`, avoiding a temporary Box allocation.

`Value::visit_gc_roots` is the single root definition for internal Values and
Reference records. Heap tracing, temporary-root sizing, and pin publication all
use that visitor, including Environment bases, object/value bases, explicit
`[[ThisValue]]`, uncoerced property names, and nested References. A direct test
requires the same ordered root set from visitor, count, and pin operations;
abrupt raw-assignment and computed-super coercion tests require exact cleanup.

```text
[Decision Log]
- 목적과 의도: Remove defensive Reference deep clones while preserving one exact GC-root contract across heap tracing and temporary roots.
- 기존 구현 및 제약 조건: Reference records are boxed recursive Values. GetValue, PutValue, delete, deferred-key rooting, and boxed receiver extraction cloned whole boxes even though cloning is not a GC root; three separate root match trees could drift as the record layout changed.
- 검토한 주요 대안: Keep all clones, change the public payload to Arc, inline ReferenceRecord into every Value, add a retained-reference opcode immediately, or first borrow consumers and centralize root traversal.
- 선택한 방식: Keep Box representation and bytecode stable, borrow immutable consumers, publish roots directly from ReferenceRecord, clone only an owned inner Value when required, and route trace/count/pin through Value::visit_gc_roots.
- 다른 대안 대신 이 방식을 선택한 이유: Arc adds atomic traffic and a source-level payload change; inline records enlarge every Value; a new opcode changes dozens of compiler sites and requires a separate move/rooting proof; defensive clones allocate but do not protect GcIdx values.
- 장점, 단점 및 영향: Five defensive record clones and five boxed-value clones disappear without layout or admission changes. Root coverage and abrupt cleanup have one deterministic oracle. Initial Reference creation boxes, required Dup clones for retained References, host allocator failure, and recursion through deliberately nested internal References remain separate work.
```

## Retained Reference move opcode

Update, compound/logical assignment, method and identifier calls, and tagged
calls need both a Reference and its resolved value. These paths previously
emitted `Dup; GetValue`; cloning `Value::Reference(Box<ReferenceRecord>)`
allocated and recursively cloned the record even though only one owner was
needed after resolution. `GetValueKeepReference` now has the explicit stack
contract `[Reference] -> [Reference, resolved value]` and moves the original
box through resolution.

Before any observable getter, Proxy trap, key coercion, or GC can run, the
opcode reserves and pins the Reference's complete root set. Raw property names
temporarily pin the same record again inside shared key coercion, so those
References reserve two complete root suffixes up front. It removes the operand
only while the outer roots are pinned. On normal or abrupt completion it
restores the original Reference to the stack before releasing pins, matching
the prior `Dup; GetValue` unwind shape. Reservation failure restores the input
without invoking user code. The compiler uses this opcode at all 24 retained
Reference reads; ordinary value duplication and standalone `GetValue` remain
separate operations.

```text
[Decision Log]
- 목적과 의도: Remove boxed Reference duplication from every retained read while preserving exact stack, evaluation-order, abrupt-completion, and GC-root semantics.
- 기존 구현 및 제약 조건: Dup cloned the recursive Box before GetValue at 24 update, assignment, call, and tag sites. GetValue can invoke key coercion, getters, and Proxy traps or trigger GC, and frame unwinding expects the retained Reference to remain on the operand stack after an error.
- 검토한 주요 대안: Keep Dup, convert the public Reference payload to Arc, borrow a raw stack pointer across re-entry, add separate pin/unpin bytecodes, specialize every compiler form, or add one fused move-and-resolve opcode.
- 선택한 방식: Add GetValueKeepReference with an explicit stack contract; move the sole Box off the stack, pre-reserve one root suffix for resolved names or the two-suffix peak for raw names, pin the outer roots, resolve through shared GetValue, restore the Reference before unpinning, then publish the resolved value only on success.
- 다른 대안 대신 이 방식을 선택한 이유: Arc changes public representation and adds atomic traffic; a stack pointer can be invalidated by re-entrant stack growth; split bytecodes expose unsafe intermediate states; per-form implementations would drift; and the fused opcode keeps one auditable ownership and cleanup boundary.
- 장점, 단점 및 영향: Twenty-four deep Box clones disappear with no public layout or JavaScript semantic change. Direct failpoints prove reservation before ordinary getters and raw super key coercion, retry, thrown-getter cleanup, and restoration of the incoming pin depth; the shared Reference-root test separately proves exact root count and order. Focused Test262 output remains byte-identical. The opcode still scans and pins roots before re-entry, raw names reserve a two-suffix transient peak, and initial Reference creation boxes remain independent work.
```

## Direct object Reference bases

Property, raw-property, super, and private References store an object base as
`ReferenceBase::Object(GcIdx)`. The outer `Box<ReferenceRecord>` already breaks
the recursive `Value` type, so wrapping the same object identity in another
`Box<Value>` was unnecessary. Primitive and nullish bases retain
`ReferenceBase::Value(Box<Value>)`; this preserves primitive receiver identity,
boxing Realm, and nullish error timing. `ObjectEnvironment` remains distinct
because `with` identifier resolution has different get, put, delete, and
missing-binding behavior.

GetValue, PutValue, delete, call-receiver extraction, root visitation, and the
retained-read peak reservation reconstruct a temporary `Value::Object` view
from the direct index. No observable operation occurs during reconstruction.
The representation preserves `Value`, `ReferenceBase`, and `ReferenceRecord`
sizes at 32/16/64 bytes on x86_64 and 24/8/40 bytes on wasm32. Direct tests
require object bases to use the compact variant, primitive bases to remain
boxed, and the object index to appear once in the shared root visitor.

```text
[Decision Log]
- 목적과 의도: Remove the redundant inner base Box from object-backed References without changing bytecode, outer Reference ownership, GC identity, or JavaScript evaluation order.
- 기존 구현 및 제약 조건: Every property Reference allocated an outer 64-byte record and a second Box<Value> even when the base was only a GcIdx. Non-object raw names and super receivers require recursive boxes; object raw names were later specialized below. Primitive property bases must preserve their original Value and Realm-sensitive boxing behavior.
- 검토한 주요 대안: Keep all allocations, add a VM-local one-entry outer-box cache, split Value into specialized identifier/property Reference handles, place all Reference fields inline, convert records to Arc, or add one direct object-base enum variant.
- 선택한 방식: Store object property bases directly as GcIdx, retain boxed Value for non-object bases, keep ObjectEnvironment semantically distinct for a later representation unit, share get/put helpers across both property-base forms, and extend the single root visitor plus retained-root reservation to the new variant.
- 다른 대안 대신 이 방식을 선택한 이유: An outer cache adds lifecycle and sentinel state while leaving inner boxes; split handles change every consumer API; blanket inlining grows common identifier records from 64 to about 120 bytes on x86_64; Arc retains allocation and adds atomic traffic. The direct variant removes one allocation across resolved, raw, super, and private object References with unchanged top-level ABI.
- 장점, 단점 및 영향: Object-backed References lose one host allocation and one deallocation each, all target-width layout assertions hold, and 15,650 pinned Test262 files are byte-identical. Primitive bases, non-object raw names, and super receivers still allocate where their recursive boundary requires it; with-object base storage and object raw-name storage are recorded below. Cold and simultaneously live outer Reference records remain separate from this representation change and are addressed by the following VM-local cache unit.
```

## Direct with-object Reference bases

Identifier References resolved through a `with` object environment use
`ReferenceBase::ObjectEnvironment(GcIdx)`. `PushWithEnv` performs `ToObject`
before creating the environment, so every valid binding object already has a
stable heap index. The resolver validates this invariant when it creates the
Reference; malformed internal environment data returns an internal error
instead of silently changing identifier resolution.

The dedicated variant remains distinct from `ReferenceBase::Object`. A normal
object property Reference and a `with` binding differ when the property is
deleted after resolution, when strict assignment observes a missing binding,
and when an unqualified call derives its `this` value. Global identifier
resolution uses `ReferenceBase::Environment`, not this variant. GetValue,
PutValue, delete, and call-receiver extraction reconstruct a temporary
`Value::Object` view from the index without allocation or observable work.
The shared root visitor publishes the index once. Proxy wrapper identity and
cross-Realm primitive wrapper identity are therefore retained unchanged.

The target-width layout contracts remain unchanged: `Value`, `ReferenceBase`,
and `ReferenceRecord` are 32/16/64 bytes on x86_64 and 24/8/40 bytes on
wasm32.

Production-constructor and malformed-payload tests, forced-GC Proxy
get/set/delete/call tests, a foreign-Realm primitive test, and a strict global
fallback test cover the semantic boundary directly. Current and preceding
release binaries also produce byte-identical output over the complete pinned
expressions/class/with cohort.

```text
[Decision Log]
- 목적과 의도: Remove the redundant inner Box<Value> from every with-object identifier Reference while preserving Object Environment Record semantics, Proxy identity, Realm-sensitive primitive boxing, and GC reachability.
- 기존 구현 및 제약 조건: PushWithEnv already applies ToObject, but EnvironmentData stores a general Value and the resolver previously boxed that Value again. Object-environment get, put, delete, missing-binding, and call-this behavior differs from ordinary property References; global identifiers use a declarative Environment base.
- 검토한 주요 대안: Keep the allocation, merge ObjectEnvironment with the ordinary Object variant, store the complete environment index, specialize EnvironmentData itself, or retain a dedicated variant with a direct binding-object index.
- 선택한 방식: Retain a dedicated ObjectEnvironment variant, store its validated GcIdx directly, rebuild temporary Value::Object views at the four consumers, and trace the index through the shared Reference root visitor.
- 다른 대안 대신 이 방식을 선택한 이유: Merging variants loses semantic dispatch; storing the environment keeps an unnecessarily broad object graph and adds a heap lookup; changing EnvironmentData expands the unit beyond Reference representation. Direct GcIdx storage follows the existing ToObject invariant and removes the allocation at its only production constructor.
- 장점, 단점 및 영향: Each resolved with identifier loses one inner allocation, including copies through temporary environment bindings. ABI size and JavaScript behavior stay fixed. The resolver now has one explicit internal invariant check; ordinary environment storage still uses Value and simultaneously live outer Reference records still allocate independently.
```

## Direct object storage for deferred property names

Computed names whose `ToPropertyKey` must occur after later evaluation use
`UncoercedPropertyName`. Object and Proxy identities are represented directly
as `Object(GcIdx)`; primitive String, Number, BigInt, Boolean, Symbol, nullish,
and internal recursive Values retain `Value(Box<Value>)`. This removes an
inner allocation without coercing early or changing the outer
`Box<ReferenceRecord>` ownership boundary.

`MakeRawPropertyRef` and `MakeSuperPropertyRef` are the only production
constructors. GetValue, PutValue, delete, and `ResolvePropertyRef` share one
coercion helper that reconstructs a temporary `Value::Object` view from the
index. The reconstruction performs no observable operation. Existing
null-base checks and pin scopes remain in place, so simple assignment still
evaluates its RHS before key coercion, delete and optional-chain paths retain
their specified ordering, and read-modify-write super names resolve exactly
once. The shared Reference root visitor traces a direct object name exactly as
it traced the former boxed Value.

The nested representation preserves the audited top-level size budget:
`UncoercedPropertyName`, `ReferencedName`, and `ReferenceRecord` are 16/32/64
bytes on x86_64 and 8/24/40 bytes on wasm32. A 30,000-operation object-key
simple-assignment benchmark measures the changed path beside a primitive
String control and asserts both the final property value and all 30,000
observable key coercions before sampling. Short forced-rebuild samples show no
regression and are retained only as shared-host smoke evidence. Pinned current
and preceding release binaries are byte-identical over both the six directly
affected Test262 directories and the complete supported statements/expressions
subset. The public Rust payload of `ReferencedName::UncoercedProperty` changes
from `Box<Value>` to `UncoercedPropertyName`; RuJa does not promise a stable
Rust enum ABI, and the assertions cover size only.

```text
[Decision Log]
- 목적과 의도: Remove the redundant inner Box<Value> from deferred object-key References while preserving exact ToPropertyKey timing, object/Proxy identity, GC reachability, and super receiver behavior.
- 기존 구현 및 제약 조건: Simple assignment, destructuring and loop targets, delete, optional delete, and computed super References retain a pre-coercion key across later evaluation. ReferencedName must break recursive Value size, object keys may invoke observable Proxy and @@toPrimitive code only at their consumer, and retained raw reads reserve a two-copy root peak before re-entry.
- 검토한 주요 대안: Keep every raw name boxed, coerce object keys at Reference creation, add a new top-level ReferencedName variant, use Arc<Value>, store every deferred Value inline, or introduce a nested payload that specializes only object identity.
- 선택한 방식: Keep one UncoercedProperty variant and give it an UncoercedPropertyName payload with direct Object(GcIdx) and boxed Value alternatives; centralize consumer coercion; retain all existing null checks, root reservations, and pin boundaries; assert target-width sizes on x86_64 and wasm32.
- 다른 대안 대신 이 방식을 선택한 이유: Early coercion violates assignment and delete ordering; a top-level variant does not improve the established enum size budget and broadens every match; Arc retains allocation and adds atomic traffic; inline Value is recursively impossible. The nested payload changes only storage while leaving compiler bytecode and Reference semantics stable.
- 장점, 단점 및 영향: Each deferred object or Proxy name loses one allocation/deallocation, and environment-stored temporary Reference clones avoid copying that inner Box. Object identity, Symbol results, abrupt completion, and the retained two-suffix reservation remain unchanged. The public Rust variant payload changes source shape, but audited target-width sizes stay fixed; no stable enum ABI is promised. Primitive and internal non-object names plus explicit super receivers still require recursive boxes; simultaneously live outer records and host allocation failure remain separate scopes.
```

## VM-local outer Reference box reuse

Each `Vm` owns one optional vacant `Box<ReferenceRecord>`. Creating a
Reference takes that allocation before the record can participate in key
coercion, getters, Proxy traps, calls, eval, or any other observable re-entry.
Recycling replaces the complete record with an unresolvable Symbol-name
sentinel whose root visitor is empty. The cache is therefore omitted from GC
root enumeration; no stale base, raw name, receiver, Realm, or private name can
survive through it.

Terminal `GetValue`, `PutValue`, delete, call/eval, immediate environment
store, `typeof`, explicit pop, catch/frame unwind, and isolated async/generator
stack disposal return boxes. `GetValueKeepReference` and successful raw-name
resolution keep ownership because later bytecode still needs the same record.
Generator suspension moves its operand stack into the generator object and
moves it back on resume instead of cloning recursive boxes. Completion and
error recycle discarded generator values. Top-level and async execution
restore their incoming frame and stack depths on every exit.

Observable re-entry is safe without a global pool or `unsafe`: a checked-out
record is absent from the cache, so nested execution either consumes an older
vacant box or allocates another. When nested execution fills the one slot, the
outer record is dropped on return. Bulk stack cleanup scans the discarded tail
only while the slot is vacant, removes its last Reference in place, and then
truncates the original Vec. It preserves LIFO preference without allocating a
second buffer or looping over every discarded value.

```text
[Decision Log]
- 목적과 의도: Reuse the remaining outer Reference allocation across sequential bytecode operations while preserving exclusive ownership, GC liveness, re-entry, abrupt completion, and generator suspension semantics.
- 기존 구현 및 제약 조건: Every VM-created Reference allocated a fresh outer Box. Records may contain environment/object roots, boxed primitive bases, raw keys, explicit super receivers, or nested internal References; retained reads cross observable calls; generator and async execution temporarily own isolated operand stacks.
- 검토한 주요 대안: Keep allocating, use a global or thread-local pool, convert the public payload to Arc, inline records into Value, cache rooted records, add a multi-entry allocator, or retain one VM-local rootless allocation.
- 선택한 방식: Add one private VM-local vacant box; overwrite the whole record with a rootless sentinel before storage; check it out before re-entry; return terminal and discarded records explicitly; move suspended generator stacks; and restore top-level/async stacks through recycling cleanup.
- 다른 대안 대신 이 방식을 선택한 이유: Global pools cross VM and Realm lifetimes; Arc retains allocation and adds atomic traffic; inlining enlarges every Value; rooted cache state complicates tracing and stale-lifetime proofs; multiple entries need a capacity policy. One rootless slot captures sequential locality and naturally bounds re-entrant retention.
- 장점, 단점 및 영향: Sequential References reuse one allocation, nested execution remains ownership-safe, cached state contributes zero roots, and uncaught/async/generator cleanup is deterministic. Simultaneously live References still allocate; a full slot drops overflow; primitive bases, non-object raw names, and super receivers retain their required inner boxes; direct with-object bases and object raw names are recorded above; host allocation failure remains infallible.
```

---

## Weak collection iterator, Realm, storage, and ephemeron pipeline

WeakMap and WeakSet constructors share the direct synchronous iterator record
used by Map and Set: one observable `@@iterator` Get, one cached `next`, zero
arguments per step, and no wrapper or `HasProperty` probe. Adder lookup precedes
iterator acquisition. Step/result/done/value failures and host Fuel propagate
without close; catchable entry/adder/storage failures use Realm-aware
IteratorClose while preserving and rooting the original completion. Prototype,
iterable, result collection, adder, iterator/next, entry/key/value, and callback
results remain rooted across every collecting or re-entrant boundary.

Each Realm installs and roots distinct WeakMap and WeakSet prototypes. Failed
provisional Realm construction removes both registries transactionally, and
constructor fallback uses the immutable NewTarget Realm intrinsic even after
observable globals and prototype links are replaced. Native allocation uses
the VM's collect-and-retry path. New weak entries reserve hash storage before
mutation; duplicate WeakSet members and WeakMap replacements reserve nothing.

`WeakKey` represents either a heap object identity or a Symbol identity.
`Symbol.for` values fail `CanBeHeldWeakly`; local and well-known Symbols are
accepted. HashMap/HashSet storage satisfies the specification's average
sublinear access requirement. Object keys remain weak. When a reachable
WeakMap is marked, the collector activates values whose keys are already live
and indexes every other value under its pending object key. Marking that key
then activates the pending values without rescanning unrelated maps, reaching
the ephemeron fixed point in work proportional to marked objects and reachable
entries. Finite-budget marks snapshot roots once, deduplicate queued identities,
queue every allocation made during the cycle, and retrace every marked cell
once immediately before sweep. The completion phase also remarks the current
host roots and shares the collector lock with allocation publication, so
mutations between slices cannot hide a new object, host root, strong edge, or
WeakMap entry. Sweep then removes dead entries,
preventing root-order-dependent value loss and stale heap-index aliasing.
Symbol-keyed entries remain live because RuJa's Symbol table is not yet
collectible.

```text
[Decision Log]
- 목적과 의도: Complete WeakMap/WeakSet observable semantics and resource ownership while making weak-key liveness independent of GC root order.
- 기존 구현 및 제약 조건: Constructors ignored iterables, only the main Realm installed weak collections, methods silently accepted wrong receivers, only object keys were represented, unchecked linear Vec storage violated average sublinear access, and one-pass WeakMap tracing could free a live-key value while retaining its stale entry.
- 검토한 주요 대안: Patch constructors only, retain Vec with more Fuel, globally unskip WeakMap/WeakSet features, store every key strongly, add heap generations without fixing marking, use hash storage with one-pass tracing, or combine hash storage with an ephemeron fixed point and exact admission.
- 선택한 방식: Install Realm-local intrinsics; use the shared direct iterator/close protocol; represent object and admissible Symbol identities as WeakKey; reserve HashMap/HashSet growth before publication; enforce method brands and callback-first upsert validation; and resolve reachable WeakMap ephemerons through deduplicated key-indexed pending values, with root snapshots, an allocation barrier, persistent finite-budget state, and a pre-sweep mutation retrace.
- 다른 대안 대신 이 방식을 선택한 이유: Constructor-only repair leaves method, Symbol, Realm, performance, and ABA-visible GC defects; extra Fuel does not satisfy average sublinear access; strong keys destroy weak semantics; generations only mask stale aliases; one-pass tracing remains root-order dependent; and global feature removal overclaims unrelated paths.
- 장점, 단점 및 영향: The complete pinned WeakMap/WeakSet corpus is 226/226, access and registered-Symbol classification are average O(1), insertion failures are catchable and atomic, and direct tests prove root-order independence, transitive ephemerons, incremental progress and mutation safety, dead cycles, cell reuse safety, Realm rollback, close/Fuel boundaries, and retry. The final incremental retrace is one linear sweep-side pass; Symbol-keyed entries can persist until VM teardown because Symbol GC remains separate work.
```

---

## Logical UTF-16 Unicode RegExp fallback

RuJa's internal lone-surrogate representation uses `U+F0000..U+F07FF`
sentinels. A Rust `char` backend cannot distinguish those sentinels from the
legal Unicode scalars with the same values. Unicode `u`/`v` execution therefore
uses a vendored `regress` 0.11.1 fallback whenever the input contains a
sentinel-backed code unit. The fallback parses the canonical pattern as logical
`u32` symbols, reads the input as native UTF-16, combines only valid surrogate
pairs, and leaves lone surrogates in `D800..DFFF`. Existing Rust and
`fancy-regex` paths remain the default for ordinary inputs.

The same fallback validates and executes Unicode patterns that the existing
backend cannot compile, including `General_Category=Surrogate`. `v` explicitly
enables both Unicode and UnicodeSets modes. `RegExpBuiltinExec` preserves the
original internal string and records logical UTF-16 boundaries, so a
`lastIndex` inside a valid pair rewinds to the pair start while captures,
`index`, `indices`, and `lastIndex` remain UTF-16 offsets. String search,
replace, and match entry points share the input-aware compiler.

Case-sensitive backreferences compare logical UTF-16 symbols rather than raw
code-unit slices, so a captured lone high surrogate cannot consume the leading
half of a valid pair. Duplicate-name backreferences try only participating
captures and match empty only when every same-name capture is unmatched. In
`iv` classes each character, range, escape, property,
and nested set is case-closed before union, intersection, subtraction, or
complement. String sequences are canonicalized before algebra and lowered
through a shared trie. Flat ordinary alternations are built as a balanced IR tree, preventing a
large legal disjunction from creating parser/optimizer recursion proportional
to its arm count.

The vendored PikeVM exposes bounded search and exact-start UTF-16 APIs. Sticky
matching executes one candidate only. Instruction dispatch, candidate state,
state clones, capture/loop slots, and backreference code units consume one
shared budget; aggregate live alternative state across nested lookarounds is
capped at a conservative 64 MiB and
compiled state cost at 1,000,000 units. Input-dependent work grows linearly
with a compiled-cost multiplier clamped to 256 through 8,192, up to 32,000,000
units.
Greedy one-character loops followed only by an end assertion do not retain an
alternative per symbol, which keeps generated full-Unicode property tests at
constant live-state memory. Lookarounds share the same budget. Exhaustion is a
non-catchable fuel abort, never a false no-match result. This is cooperative
resource control, not an OS deadline.

Logical input is metered once per symbol and converted to UTF-16 once per
backend operation. Match and capture endpoints are collected, sorted, and
mapped to internal byte boundaries in one input scan rather than rescanning
from the beginning per endpoint. `RegExpBackendInput` computes the one
`lastIndex` start boundary without retaining a tuple per character. Default
global `Symbol.match` on an unmodified realm RegExp prototype uses the existing
single-compile iterator and consumes VM fuel before retaining each result; own,
Proxy, accessor, and modified-prototype `exec`
paths retain the observable RegExpExec loop.

Logical source length is scanned without allocation and capped at 262,144
UTF-16 units before the general regex validator can materialize pattern copies;
64 escape-parity-aware property operands are checked after syntax validation. Every
Unicode RegExp validates these logical-backend resource preconditions at
construction, while full logical compilation remains lazy to avoid compiling
ordinary generated patterns twice. Post-compile accounting includes instructions, capture and
loop state, group names, and bracket intervals. Sequential candidate state is
charged by slot count; only actual retained branch clones use conservative byte
cost, and the live-state check accounts for `Vec` capacity across the complete
lookaround recursion tree. This lets
large linear generated matrices complete without weakening branch-memory
bounds.

Named-capture prepass work is bounded before path and duplicate-reference IR
materialization: at most 1,024 named groups, 64 groups sharing one name, 65,536
stored alternative-path segments, 1,000,000 path-comparison units, and 16,384
duplicate-backreference alternatives. These limits prevent legal-size source
from turning duplicate-name analysis into quadratic allocation or CPU work.

RegExp validation and compilation preserve `Syntax` versus `Resource` as a
typed internal result through the dynamic constructor and every runtime
compile call site. Flags are validated before the allocation-avoiding source
cap. The Rust backend classifies compiled-size and structured AST nesting or
capture limits as resources; fancy-regex classifies NFA size and parser
recursion limits; regress tags each parser, named-capture, and string-set
resource guard directly. Only the final builtin boundary chooses
`SyntaxError` or the active Realm's `RangeError`. Runtime matcher work-limit
errors remain non-catchable Fuel aborts. Oversized Unicode source is rejected
before general pattern validation to avoid allocating validator copies, after
complete dynamic or literal flag validation. Backend selection retains
fallback semantics: a limit in one candidate backend is not observable when
another bounded backend compiles the same ECMAScript pattern successfully.

```text
[Decision Log]
- 목적과 의도: Preserve the semantic distinction between invalid RegExp syntax and implementation resource exhaustion across every compiler/backend boundary.
- 기존 구현 및 제약 조건: Compiler helpers returned String, constructor and legacy String paths mapped every failure to SyntaxError, regress exposed only diagnostic text, and Unicode patterns intentionally fall back between multiple bounded backends.
- 검토한 주요 대안: Match diagnostic strings, classify every compiler failure as RangeError, remove fallback, eagerly compile and store every matcher, or carry a narrow typed result to the JavaScript error boundary.
- 선택한 방식: Add typed syntax/resource results in RuJa and regress, classify structured Rust/fancy variants, validate flags before bounded pattern work, share one JavaScript mapper across all compile callers, and retain successful fallback.
- 다른 대안 대신 이 방식을 선택한 이유: Message matching is brittle; blanket RangeError corrupts real syntax diagnostics; removing fallback regresses supported semantics; matcher storage changes execution ordering and ownership beyond this unit. Typed propagation is narrow and reviewable.
- 장점, 단점 및 영향: Dynamic resource limits now produce Realm-correct RangeError while reachable malformed-pattern diagnostics remain SyntaxError, and backend fallback stays transparent. Exec observes and bounds lastIndex before compilation; the deterministic terminal-failure and bounded matcher-cache boundaries are documented below, while compiler allocation failpoints remain separate work.
```

### RegExp exec terminal compilation boundary

`RegExpBuiltinExec` now routes its Unicode/input-sensitive and non-Unicode
compiler choices through one terminal helper. In tests, a VM-local countdown
can replace only an already successful terminal result with a typed resource
failure. The real compiler therefore still selects Rust, fancy, or logical
UTF-16 execution and completes every bounded fallback first. A genuine syntax
or resource error returns unchanged and leaves the countdown armed. The
injected matcher is dropped before backend-input preparation, capture-name
materialization, matching, `lastIndex` publication, or result construction.

This boundary fixes evidence rather than claiming allocator coverage inside
third-party compilers. Direct tests establish both countdown directions under
re-entry, main/foreign method Realm selection, input and `lastIndex` abrupt
priority, global/sticky out-of-range reset, non-global large-index behavior,
Rust/fancy/size-fallback/logical variants, materialization bypass, and retry
without repairing preserved state. `RegExp.prototype.test` now performs input
`ToString` and dynamic `RegExpExec`; callable custom `exec` methods are observed
and an absent/non-callable override falls back to builtin execution only for a
branded RegExp.

```text
[Decision Log]
- 목적과 의도: Freeze the exact observable boundary of exec-time matcher compilation before adding matcher ownership and allocation hardening, while repairing RegExp.prototype.test's dynamic RegExpExec semantics.
- 기존 구현 및 제약 조건: RegExp instances compile again during builtin exec, Test262 cannot force terminal compiler resource failure, an early synthetic error could hide successful backend fallback or replace a genuine compiler error, and test called the builtin matcher directly instead of observing an overridden exec.
- 검토한 주요 대안: Inject before backend selection, inject Syntax and Resource indiscriminately, force process OOM, patch every vendor allocation in the same unit, cache matchers immediately, or inject only after terminal success and keep actual allocation work separate.
- 선택한 방식: Centralize exec compilation, preserve genuine errors, apply a test-only one-shot/countdown Resource result only after successful terminal compilation, keep it before all later preparation/publication, and route test through the existing dynamic RegExpExec dispatcher.
- 다른 대안 대신 이 방식을 선택한 이유: Pre-backend injection changes fallback semantics; synthetic Syntax is unreachable for an initialized matcher and can mask real diagnostics; process OOM is nondeterministic; vendor fallibility and cache publication have independent ownership contracts. A post-success hook proves ordering without overstating production allocation guarantees.
- 장점, 단점 및 영향: Backend fallback remains transparent, method-Realm RangeError identity and unchanged lastIndex are deterministic, nested failure unwinds cleanly, immediate retry succeeds, and custom exec is specification-observable. The bounded cache below reuses admitted terminal matchers; compiler and vendor allocation remain non-catchable, and unbounded matcher variants still rebuild.
```

The vendored crate retains its upstream MIT OR Apache-2.0 license files.

```text
[Decision Log]
- 목적과 의도: Distinguish every legal Unicode scalar from every lone UTF-16 surrogate during u/v matching without changing RuJa's canonical JavaScript String representation.
- 기존 구현 및 제약 조건: Rust strings cannot contain surrogate scalar values, RuJa stores lone surrogates in a private-use sentinel range that is also legal JavaScript text, and the existing Rust/fancy backends therefore collapse two distinct logical symbols. Replacing the sentinel range cannot solve an injectivity problem over the complete Unicode scalar domain.
- 검토한 주요 대안: Move the sentinel range, globally prefix-encode every regex symbol, rewrite only Co/Cs properties, replace every regex backend, use regress's unbounded classical UTF-16 API, or route only collision-bearing Unicode inputs through a bounded native UTF-16 executor.
- 선택한 방식: Vendor regress 0.11.1; validate logical resource preconditions for every u/v pattern at construction; lazily compile canonical pattern code points and native UTF-16 input; route sentinel-bearing u/v inputs and existing-backend compile fallbacks through a bounded PikeVM; add bounded named-capture prepass/lowering, exact sticky execution, logical-symbol and duplicate-name backreferences, operand-first iv folding, balanced alternation IR, shared byte-based state/backreference accounting, metered global collection, batched offset conversion, and UTF-16 boundary preservation.
- 다른 대안 대신 이 방식을 선택한 이유: Sentinel relocation is mathematically insufficient; prefix encoding requires complete rewrites of literals, classes, properties, complements, lookarounds, and captures; property-only repair leaves literals and dot wrong; global replacement widens performance and compatibility risk; the upstream UTF-16 backtracker has no sandbox bound. A narrow native UTF-16 fallback preserves the established fast path and supplies the required logical domain directly.
- 장점, 단점 및 영향: Lone surrogates and U+F0000..U+F07FF scalars now differ across literals, properties, complements, dot, captures, backreferences, global/sticky matching, and d indices. Two exact generated Test262 files became admissible in this unit. Large flat alternatives have logarithmic IR depth, endpoint conversion is linear, and live branch state has a byte cap. The fallback maintains a second parser for its bounded domain and remains cooperative DFS under explicit work and state limits; the later string-set unit extends that parser under the same bounds.
```

---

## Set algebra live traversal and resource ownership

Set uses generation-ordered slots for specification order and a
`HashMap<MapKey, usize>` for average-O(1) membership. Deletion tombstones the
existing slot; reinsertion appends a new generation. Iterator cursors store
that generation rather than a Vec index, so allocation-free stable compaction
can remove tombstones without invalidating active iterators. Compaction runs
when tombstones reach `max(live entries, 64)`, bounding constant-live churn.
Set iterators, `forEach`, and composition algorithms naturally observe entries
added after their current generation without cloning or rescanning the Set.
Fuel is charged before each visited physical slot and before compaction/clear
work; a Fuel failure precedes mutation.

The seven composition methods first build a rooted SetRecord containing the
observed object, numeric size, `has`, and `keys`. Iterator branches cache the
iterator's `next` property once. Values yielded by internal and external
iteration receive fallible temporary roots before callbacks or result
publication. Union and symmetric difference acquire the iterator before
copying the receiver and close it if later catchable host allocation fails;
post-step catchable failures close through the cached iterator record while
non-catchable Fuel remains a host abort. Results use the active Realm's
immutable Set prototype and GC-retrying allocation path.

```text
[Decision Log]
- 목적과 의도: Make all Set composition methods specification-correct under live mutation, deletion/reinsertion, foreign Realms, GC, Fuel, and catchable host allocation failure.
- 기존 구현 및 제약 조건: Composition used main-Realm results, incompletely rooted SetRecord and iterator state, optimized internal iterators past observable next lookup and return, iterated the receiver instead of the result snapshot in one difference branch, and refreshed value-only snapshots with quadratic-to-cubic work that could not distinguish deleted/reinserted generations.
- 검토한 주요 대안: Keep IndexSet and clone snapshots, add generation comparisons to the refresh queue, globally wrap iterators, shift-remove ordered entries, or represent SetData as append-only slots plus a membership index.
- 선택한 방식: Store ordered generation slots and a key-to-slot hash index; tombstone deletion and append reinsertion; keep generation cursors across stable in-place compaction; compact at a bounded tombstone ratio; meter traversal, compaction, and clear before mutation; root one yielded value at a time; cache generic iterator methods; and close active iterators on catchable post-acquisition failures while preserving original throw priority.
- 다른 대안 대신 이 방식을 선택한 이유: Snapshot refresh remains superlinear and duplicates key roots, shifting raw indices invalidates live cursors, wrapper iterators alter observability, and uncompacted append-only slots permit unbounded constant-live host memory. Generation cursors preserve specification order while allowing allocation-free compaction and average-O(1) membership.
- 장점, 단점 및 영향: Set algebra, ordinary Set iterators, and forEach share stable live-order semantics; callback mutation is linear in bounded physical slots; result growth is fallible and atomic; constant-live churn stays bounded; and Realm/GC/close behavior has deterministic tests. A non-empty collection may retain capacity from a larger historical live peak until clear or empty, matching the remaining native-container capacity model.
```

---

## ECMA-402 locale canonicalization foundation

Every Realm receives an ordinary `%Intl%` namespace backed by that Realm's
`%Object.prototype%`; its native `getCanonicalLocales` function creates result
Arrays and errors through the function Realm. `CanonicalizeLocaleList` keeps
the source object rooted across one `length` read and the required
`HasProperty`/`Get` sequence. Only String and Object elements proceed to locale
canonicalization, and canonical strings are deduplicated after conversion in
first-seen order. Logical indices consume VM fuel, while locale parsing
precharges the square of subtag count plus input chunks to cover ICU's sorted
insertion paths, so huge sparse lists and adversarial tags cannot bypass
cooperative limits.

ICU4X `icu_locale` 2.2.0 provides UTS 35 casing, ordering, CLDR
language/script/region/variant aliases, subdivision aliases, and extended
likely-subtag selection. Its parser does not accept the structurally valid
reserved 5-8-letter language range, so RuJa validates the original identifier
with collision-free private-use language placeholders, canonicalizes, and
restores those exact lowercase subtags. Original syntax is validated before
legacy whole-tag replacement. ICU4X also documents missing BCP47 extension
type aliases; a reproducible generator pins Unicode CLDR 48.2 commit
`11299982335beb974c1c63c45265184e759c0f41`, filters aliases through the U/T
value grammar, and emits sorted static Unicode and transform tables under the
Unicode-3.0 terms.

The outer canonicalizer only applies simple aliases inside a transformed
language extension and represents fields as a unique-key map. RuJa therefore
extracts the complete transform extension, applies the ordinary Locale
canonicalizer to `tlang`, canonicalizes every tfield through generated aliases,
stable-sorts fields by key, and restores all occurrences so repeated valid
tkeys are not discarded.
ICU4X 2.2 does not parse numeric extension singletons, so RuJa structurally
validates and removes those segments before ICU processing, then restores them
in canonical digit order ahead of alphabetic extensions and private use.

```text
[Decision Log]
- 목적과 의도: Establish a specification-correct, Realm-safe locale canonicalization base that later ECMA-402 constructors can share.
- 기존 구현 및 제약 조건: RuJa had no `%Intl%`; broad Test262 Symbol/Proxy gates hid observable locale-list cases; ICU4X has complete core CLDR alias and likely-subtag data but deliberately omits some extension aliases and rejects reserved 5-8-letter language subtags.
- 검토한 주요 대안: Hand-maintain only pinned Test262 mappings, write all CLDR canonicalization from scratch, depend on ICU4X without adapters, bundle raw CLDR XML at runtime, or use ICU4X plus generated extension data and a narrow structural parser adapter.
- 선택한 방식: Pin ICU4X 2.2.0 and CLDR 48.2 independently; validate original syntax, adapt only reserved long languages, apply ICU canonicalization, preserve repeated transform fields outside ICU's map, and apply deterministically emitted U/T aliases; implement exact locale-list observability, Realm allocation, GC rooting, fallible result growth, and fuel metering before input scans.
- 다른 대안 대신 이 방식을 선택한 이유: Test-only maps preserve known conformance gaps; a new CLDR engine duplicates complex likely-subtag logic; unadapted ICU fails required grammar and extension cases; runtime XML increases startup, binary, parser, and allocation risk. The layered design keeps authoritative data broad while isolating documented upstream gaps.
- 장점, 단점 및 영향: The exact 40-file `%Intl%`/getCanonicalLocales manifest is green and its adjacent Locale-object case is now owned by the separate Locale manifest; cross-Realm identity and errors are deterministic, and CLDR updates are reproducible. ICU adds compiled locale data and dependencies; formatter data, `%Intl%.[[FallbackSymbol]]` observability, locale negotiation, and the rest of ECMA-402 remain later units.
```

### `Intl.Locale` structural object and Realm flow

Each Realm installs a distinct `%Intl.Locale%` constructor and prototype and
keeps both in GC-rooted Realm registries. Construction uses the VM's eager
native-constructor mode: observable `newTarget.prototype` resolution happens
before the native body, and a non-object prototype falls back to the
new-target Realm's immutable Locale prototype registry. The installer pins
every provisional getter, method, prototype, and constructor until both
objects are linked and published in those registries.

Locale instances use `HeapObj::IntlLocale`, whose one-time immutable record
stores the canonical tag and relevant Unicode keyword values. The enum variant
is the unforgeable `[[InitializedLocale]]` brand; ordinary properties,
prototype inheritance, and Proxy wrapping cannot copy it. Its ordinary class
name remains `Object`, so deleting the configurable inherited
`@@toStringTag` restores `[object Object]` as required. GC traces only the
instance's ordinary properties and prototype because the locale record holds
no VM heap edges.

The constructor canonicalizes the input tag, observes language/script/region/
variants and Unicode options in specification order, rebuilds and
recanonicalizes after each option phase, then initializes the record exactly
once. Locale inputs and locale-list elements read the internal tag directly.
`maximize` and `minimize` transform only the language identifier with ICU4X's
extended likely-subtag data, preserve variants/extensions/private use, and
construct the fresh result through the method Realm's registered intrinsic.
String grammar scans and likely-subtag work are precharged against VM fuel.

```text
[Decision Log]
- 목적과 의도: Add the complete base Intl.Locale object contract without weakening Realm, GC, brand, or sandbox guarantees.
- 기존 구현 및 제약 조건: Canonicalization returned only strings; ordinary hidden properties were forgeable or allocation-heavy; generic constructor fallback could not locate an Intl-nested intrinsic; ICU locale values and Rust strings are outside the heap-object cap; Locale-info requires calendar, collation, time-zone, week, and text-direction data not present in the current dependency set.
- 검토한 주요 대안: Store a public/private ordinary property, infer the brand from prototype or class name, retain a full ICU Locale per object, use the generic constructor builder, implement Locale-info with partial tables, or add a dedicated compact heap variant and Realm installer while separating the data-dependent surface.
- 선택한 방식: Use a one-time immutable IntlLocale record in a dedicated ordinary-behaving heap variant; install constructor/prototype transactionally per Realm with eager prototype observation; reuse the shared canonicalizer and ICU4X LocaleExpander; precharge native scans; freeze the 109-file base boundary and keep Intl.Locale-info scope-closed.
- 다른 대안 대신 이 방식을 선택한 이유: Properties/prototypes/class names cannot supply an unforgeable brand and the class name is observably wrong after deleting @@toStringTag; retaining parsed ICU structures duplicates native memory; the generic builder bypasses rooted GC retry and Intl-specific Realm fallback; partial Locale-info would overclaim data semantics.
- 장점, 단점 및 영향: Base constructor, options, descriptors, brands, subclassing, cross-Realm fallback, canonical locale-list integration, and likely-subtag transforms are exact and independently gated. One canonical string plus relevant keyword strings keeps instance state compact. Native Arc/String/ICU temporary bytes still are not included in the heap-object cap, and Intl.Locale-info remains a separate data-provider unit.
```

### `Intl.Locale-info` generated data boundary

Locale-info keeps immutable locale identifiers in `IntlLocaleRecord` and does
not attach provider objects to each instance. Calls derive the explicit region,
canonical `sd` subdivision, ICU4X likely region, and independent `rg` override,
then binary-search generated static tables. CLDR `001` inheritance makes a
valid override region available even when it has no explicit row. Text
direction uses the explicit or likely script; time zones deliberately use only
the explicit locale region as required by `TimeZonesOfLocale`.

`tools/generate_intl_locale_info.py` pins CLDR 48.2 and emits calendar
preferences, hour cycles, effective week rows, known script directions, and
region-to-time-zone lists. Time-zone generation combines Windows territory
mappings with region-encoded timezone entries, rewrites aliases through each
entry's canonical IANA identifier, deduplicates, and sorts. The generated
calendar identifiers define RuJa's current Locale-info `AvailableCalendars`
set independently of formatter constructors. Collation results now share the
provider-validated Collator matrix; numbering systems retain the `latn`
no-matching-locale fallback until NumberFormat exists.

Every returned Array and ordinary information object is allocated from the
active native function Realm. Arrays are rooted before containing objects can
allocate; result fields use CreateDataProperty attributes. Linear tag scans and
quadratic likely-subtag work plus result entry/string chunks are charged before
ICU execution or result materialization; result Vec capacity is reserved
fallibly. Runtime code does no XML parsing, filesystem access, or network I/O.

```text
[Decision Log]
- 목적과 의도: Supply complete deterministic Locale inspection data without weakening Realm identity, GC safety, sandbox metering, or wasm portability.
- 기존 구현 및 제약 조건: Base Locale stored only canonical keyword slots; formatter providers were absent; CLDR defaults are inherited rather than repeated per region; windowsZones alone omits valid region zones such as Antarctica/Troll.
- 검토한 주요 대안: Runtime CLDR parsing, host ICU bindings, test-only maps, ICU4X formatter crates, world-only fallbacks, or generated CLDR tables plus existing locale expansion.
- 선택한 방식: Generate sorted immutable tables from pinned CLDR; combine timezone.xml and windowsZones; model `001` inheritance during lookup; preserve formatter-absence fallbacks; publish fresh Realm-local values from branded methods.
- 다른 대안 대신 이 방식을 선택한 이유: Runtime and host providers are nondeterministic or resource-heavy, test maps are incomplete, formatter crates widen the unit, and world defaults erase required region distinctions. The chosen boundary is broad data with narrow runtime machinery.
- 장점, 단점 및 영향: Locale-info is reproducible, fast, network-free at runtime, and exact over 52 pinned tests. Its original generated source was roughly 31 KiB; the shared file is roughly 44 KiB after the subsequent supportedValuesOf tables. CLDR preference updates are observable, and formatter constructors will later replace only the documented collation/numbering fallback paths.
```

### `Intl.supportedValuesOf` capability boundary

`Intl.supportedValuesOf` is installed as a Realm-local non-constructor next to
`getCanonicalLocales`. It performs observable `ToString`, dispatches only the
six case-sensitive specification keys, and publishes a newly allocated Array
through the native function Realm. Static lists are already sorted and unique,
so runtime work is linear publication rather than sorting or provider access.

The existing pinned generator emits the ECMA-402 fixed 16-calendar and
45-simple-unit lists, CLDR 48.2's 78 simple-digit numbering systems, and 445
primary time-zone identifiers. Time-zone generation maps `Etc/UTC` and
`Etc/GMT` links to `UTC`, removes `Etc/Unknown`, and retains primary geographic
and nonzero `Etc/GMT` identifiers. Collation and currency are
implementation-defined formatter capability sets. Collator now publishes ten
values confirmed against ICU4X baked metadata and accepted by RuJa's locale
matrix; currency remains empty until NumberFormat or DisplayNames establishes
real support. This avoids advertising values no RuJa service can consume.

List entry count and string chunks consume fuel before any result allocation.
The native Vec reserves fallibly; contained values and the Realm Array
prototype are pinned across Array allocation and GC retry. Fresh bounded
`Arc<str>` bytes remain outside the heap-object cap, matching Locale-info's
documented native-memory limitation.

```text
[Decision Log]
- 목적과 의도: Implement the complete standalone Intl.supportedValuesOf contract while preserving honest formatter capability reporting and sandbox guarantees.
- 기존 구현 및 제약 조건: Locale-info already generated regional data, but no global capability lists or formatter constructors existed; broad Intl-enumeration gating hid 25 files, ten of which directly instantiate absent formatters.
- 검토한 주요 대안: Return test-only literals, expose every CLDR collation/currency, wait for all formatters, admit the whole directory, query host ICU at runtime, or generate fixed/pinned data and scope-close formatter integration.
- 선택한 방식: Extend the pinned CLDR generator with normative fixed lists and primary time-zone filtering; initially expose empty formatter-owned capability sets, then let Collator supply its provider-validated collation set; publish through the method Realm with shared fuel/allocation handling; freeze the standalone files and retain absent-formatter files as skips.
- 다른 대안 대신 이 방식을 선택한 이유: Test literals and host providers are incomplete or nondeterministic, fabricated formatter capabilities violate the API meaning, formatter-first coupling delays an independent standard function, and directory admission would claim behavior RuJa cannot execute.
- 장점, 단점 및 영향: Six-key enumeration is deterministic, Realm-correct, GC-safe, fuel-bounded, wasm-compatible, and exact over the standalone pinned boundary. Collation output broadened with the real Collator provider and currency remains empty; nine absent-formatter integration tests remain visible skips rather than false support.
```

### `Intl.Collator` provider, Realm, and comparison flow

Each Realm installs a callable and constructable `%Intl.Collator%` plus its
prototype and roots both in Realm registries. Instances use the dedicated
`HeapObj::IntlCollator` brand. A one-time record owns resolved locale/usage/
collation/numeric/case-first/sensitivity/punctuation slots and an ICU4X
`CollatorBorrowed<'static>`; a separate traced slot caches the anonymous bound
compare function. The resulting Collator/Function cycle is ordinary
mark-and-sweep data and disappears when no external root remains.

Locale lists reuse the canonical locale operation. Availability is
conservative: the language must receive script and region data from ICU4X's
CLDR likely-subtag provider, excluding arbitrary reserved identifiers and
`und`/`zxx`. `co`, `kn`, and `kf` are negotiated independently; supported
options override supported Unicode extensions, while unsupported options do
not displace a valid extension. Ten collation types were checked directly
against ICU4X 2.2 baked `CollationMetadataV1` and form one sorted matrix shared
by construction, `supportedValuesOf("collation")`, and
`Locale.prototype.getCollations`.

Sensitivity maps to ICU strength/case-level options, punctuation maps to
shifted alternate handling with punctuation max-variable, and comparison uses
potentially ill-formed UTF-16 slices so lone surrogates follow ECMA string
semantics. String conversion order is observable before Collator construction.
`String.prototype.localeCompare` constructs the immutable method-Realm
Collator intrinsic, never the replaceable `Intl.Collator` property, preserving
Realm prototypes, heap-cap allocation order, and constructor semantics.
ICU4X compiled data omits general search collations; German search uses
phonebook primary weights to preserve AE/Ä equivalence, and other search
locales use ICU's documented root fallback.

```text
[Decision Log]
- 목적과 의도: Add a real locale-sensitive comparison service while preserving ECMA-402 observability, Realm isolation, GC safety, and sandbox resource boundaries.
- 기존 구현 및 제약 조건: String.localeCompare only normalized NFC and compared Rust strings; no Collator brand, locale negotiation, bound compare identity, search behavior, or formatter capability source existed. ICU4X has broad deterministic collation data and direct UTF-16 comparison but silently falls back for unsupported locale/type requests and omits search tailoring data.
- 검토한 주요 대안: Keep lexical comparison, wrap host ICU, construct an ICU comparator on every call, store forgeable ordinary properties, report every ICU enum as supported, accept every valid locale, or add a branded object with a conservative provider-aligned capability boundary.
- 선택한 방식: Pin icu_collator 2.2.0 compiled data; store one borrowed comparator in a branded heap record; cache and trace a Realm-native bound function; use CLDR likely-subtag availability plus a baked-metadata-validated collation matrix; construct the intrinsic service from localeCompare; meter scans/comparison and reserve UTF-16 buffers fallibly.
- 다른 대안 대신 이 방식을 선택한 이유: Lexical and host behavior are non-conforming or nondeterministic; per-call reconstruction loses object/heap semantics; properties and prototypes cannot prove brands; enum presence and parser validity do not prove provider support. The selected split makes fallback and optional capability claims explicit and auditable.
- 장점, 단점 및 영향: The exact 74-file Collator/localeCompare boundary is Realm-correct, GC-traced, UTF-16-aware, and backed by reproducible data. ICU compiled data increases binary/link size and native data is outside the heap-object cap. General search tailoring and the shared this-value harness remain bounded follow-up work with NumberFormat/DateTimeFormat.
```

### Weak-reference Realm, resource, and cleanup boundary

WeakRef and FinalizationRegistry prototypes are immutable Realm registry roots,
not values rediscovered through replaceable global bindings. Construction pins
the selected prototype and allocates through the VM's GC-retry path. WeakRef's
per-job target roots use a fallible identity set, avoiding the prior quadratic
Vec duplicate scan while retaining the ECMAScript job-liveness guarantee.

FinalizationRegistry registration reserves cell storage before publication and
caps native cell count at the shared materialization boundary. Unregister and
cleanup scans consume Fuel before mutation. Each registry stores and traces its
constructor Realm; the cleanup job enters that Realm before callback dispatch,
whose own function Realm then becomes active. Cleanup selects one cleared cell,
preflights both its outer pins and nested callback-call roots, removes that cell,
then invokes the callback outside the registry lock. The next selection observes
callback-time unregister or registration. Catchable callback errors are
contained at the host-job boundary and leave later cells eligible for a future
job; non-catchable Fuel propagates to the host. Sweep publishes one atomic
pending bit per registry, so scheduling scans bounded heap cells rather than
rescanning every native cell vector. Pending-registry collection and microtask
queue growth reserve fallibly and reset publication flags if scheduling cannot
allocate, so cleanup is postponed rather than causing a host OOM path.

```text
[Decision Log]
- 목적과 의도: Freeze the complete WeakRef/FinalizationRegistry surface while making weak cleanup obey Realm identity, cellwise specification order, and sandbox resource rules.
- 기존 구현 및 제약 조건: All 76 Test262 surface files passed, but prototype fallback consulted mutable Realm globals, constructors bypassed GC retry, job-kept roots used quadratic infallible Vec growth, registry cells and scheduling grew infallibly, and cleanup removed every dead cell before callbacks while swallowing Fuel.
- 검토한 주요 대안: Treat 76/76 as complete without runtime changes, keep prefix admission, batch all holdings before callbacks, propagate every callback throw, make public GC fallible, or add immutable registries plus transactional fallible scheduling and cellwise cleanup.
- 선택한 방식: Root original prototypes per Realm; store and trace each registry's constructor Realm; use VM allocation and complete callback dispatcher-entry root preflights; store job-kept identities in a fallible HashSet; meter/cap cell operations; publish one sweep-time pending bit; reserve scheduling before publishing flags; remove and invoke one cleanup cell at a time; contain catchable errors while propagating Fuel.
- 다른 대안 대신 이 방식을 선택한 이유: Surface tests contain no GC cleanup execution and could not prove sandbox safety; prefix admission silently accepts future files; batching violates callback-time unregister and abrupt ordering; propagating catchable cleanup errors exposes host jobs to script; changing GC's public contract would widen unrelated embedding APIs.
- 장점, 단점 및 영향: Constructor/job/callback Realm separation, Realm tampering, heap-cap retry, nested-call allocation failure, unregister re-entry, catchable throw, and Fuel abort behavior are directly tested and the 76-file policy is scope-closed. Sweep still scans bounded heap cells atomically; native cell bytes remain outside heap-object accounting, and non-GC Symbol targets can remain live until a future Symbol-arena unit.
```

---

### RegExp temporary-root publication

RegExp methods retain values returned by observable getters, custom `exec`,
constructors, and coercions while later calls may collect the heap. Every such
value is now published through a RegExp-local fallible pin helper: it counts
all GC roots represented by the value, reserves the complete batch, and only
then appends indices to the VM root stack. Multi-value entry and initialization
batches reserve atomically. Values observed one at a time reserve immediately
after the observable operation, preserving specification ordering.

Match-indices knows its future object count before nested materialization. It
reserves one root for every participating pair plus one optional groups object
before allocating the first pair. Existing raw pins in that loop therefore
cannot grow the root vector or fail halfway through publication. Outer cleanup
boundaries release all earlier roots when a later reservation, getter,
coercion, allocation, or callback fails.

```text
[Decision Log]
- 목적과 의도: Prevent host allocation aborts and stale heap identities when RegExp methods retain temporary objects across observable re-entry.
- 기존 구현 및 제약 조건: RegExp used infallible gc_pins growth at dozens of sites; search, match, and toString also left fresh lastIndex, exec-result, flags, source, and capture values unrooted across later getters or coercions. Reservation must not move ahead of observable specification steps.
- 검토한 주요 대안: Rely on native call frames, reserve one large entry batch for each method, make pin globally fallible, catch allocator panic, or add local exact reservation immediately before every RegExp publication.
- 선택한 방식: Use single-value and atomic multi-value RegExp pin helpers, retain existing cleanup scopes, add missing semantic roots, and preflight the statically known indices pair/groups batch before nested allocation.
- 다른 대안 대신 이 방식을 선택한 이유: Native Rust locals are not GC roots; entry-wide reservation changes getter ordering and over-reserves attacker-controlled paths; a global pin API migration is a separate cross-module unit; panic recovery cannot restore allocator or VM invariants. Local post-observation reservation preserves behavior and bounds this change.
- 장점, 단점 및 영향: Root growth failure is a catchable RangeError, every prior pin is released, fresh intermediates survive forced GC, and retry remains possible without rolling back earlier observable side effects. Native capture/name/property containers and compiler metadata remain separate hardening work; bounded matcher reuse is described below.
```

### RegExp post-match container publication

Builtin `RegExp.prototype.exec` separates the matcher result from its native
post-match materialization. For global and sticky expressions, match zero's
UTF-16 end is computed and written to `lastIndex` first, as required by
`RegExpBuiltinExec`. Capture ranges and result payloads are then built in
fallibly reserved local vectors. Endpoint sorting prepays conservative
`n log n` Fuel; byte-to-UTF-16 conversion reserves its offset map and prepays
both endpoint and input-scan work. String slicing and copying are conservatively
charged by input bytes per capture candidate, and named-group map insertion
prepays every capture-name byte before hashing.

Result and indices Arrays use `ArrayData::try_new`, including the dense
presence bitmap. Named string and indices groups build a reserved local
`IndexMap` before allocating the null-prototype heap object. `IndexMap::entry`
retains the first property position for duplicate names and replaces an
earlier `undefined` only when a later alternative participated. All
participating indices pairs and the indices-groups object retain the existing
batch root preflight; the outer indices and exec-result property maps reserve
before their first insertion. Thus a catchable reservation failure cannot
publish a partial groups object or partially decorated result Array, and it
does not roll back an already specified `lastIndex` write.

```text
[Decision Log]
- 목적과 의도: Make the complete builtin-exec post-match native container layer catchable, metered, and atomic while preserving RegExpBuiltinExec side-effect order.
- 기존 구현 및 제약 조건: Matcher capture vectors already existed, but capture boundary conversion used infallible Vec/HashMap collection, Array presence used ArrayData::new, groups mutated a heap object while native maps grew, and result properties inserted without reservation. Global/sticky lastIndex was incorrectly delayed until after capture strings were copied.
- 검토한 주요 대안: Reserve all future storage before matching or lastIndex publication, impose a smaller capture limit, mutate heap maps one property at a time, retain a separate matched-name IndexSet, combine compiler/backend/replacement allocations into one patch, or isolate the post-match ownership boundary.
- 선택한 방식: Publish match zero's end first; prepay conservative Fuel before each reservation and native operation for capture, endpoint, sort, scan, slice, full name-byte hashing, presence, and property work; exact-reserve local vectors and presence maps at their specification phase; construct groups maps locally with entry replacement; allocate heap objects only after native maps are complete; inject deterministic failure at each owned reservation.
- 다른 대안 대신 이 방식을 선택한 이유: Entry-wide reservation changes observable lastIndex and setter ordering; lower caps reject valid programs; incremental heap mutation permits partial publication; IndexSet duplicates storage and allocation; compiler/backend/replacement paths have different typed-error and callback-order contracts. The isolated boundary can be proved end to end without overstating allocator coverage.
- 장점, 단점 및 영향: Every owned container failure is a Realm-correct catchable RangeError, pins and failed partial objects are collectible, duplicate property order and indices identity survive retry, and native work has an exact cooperative Fuel boundary. String/Arc payload allocation, capture-name/compiler metadata, backend capture conversion and input boundary tables, replacement containers, compiler/vendor allocation, and legacy String paths remain separate units.
```

### RegExp replacement native-container boundary

Builtin `RegExp.prototype[Symbol.replace]` owns seven variable native
containers after observable input, replacement, and flags coercion: the
non-ASCII input cache, collected result List, each capture List,
functional-replacer arguments, static-substitution scratch, final UTF-16
output, and the UTF-8 decode buffer.
Each container consumes Fuel and uses checked `try_reserve_exact` growth before
mutation. A typed test-only failpoint follows actual capacity growth, so it
models production reservation rather than logical append count.

After the required global `lastIndex = 0` write, input indexing uses one
`RegExpUtf16Source`. ASCII input remains borrowed; non-ASCII input is encoded
once into a fallibly reserved `Vec<u16>`. Empty-match advancement reads this
source with checked `u64` arithmetic, while `$`` and `$'` slicing and final
source copying use bounded ranges from the same representation. Replacement
template parsing streams directly into reusable match-local UTF-16 scratch.
A failed `$<name>` close-delimiter search is remembered, and successful
searches advance past the delimiter, keeping parser search work linear in the
template. Output is committed only after callback or named-capture observation,
so backward-position matches retain their required effects without changing
the result.

Collected results reserve, pin, and append immediately after each successful
`exec`, before global empty-match processing. All results remain rooted until
replacement ends, preserving the specification's collect-before-callback
contract. Existing string-valued input, match, capture, and callback returns
remain shared as `Arc<str>` instead of being copied into temporary Rust
`String` values, including every repeated `exec` argument. `ToString` of a
non-string value may still allocate a new Arc payload. The final UTF-16 output
uses an exact fallible UTF-8 byte reservation,
including lone-surrogate sentinel parity, before publication.

```text
[Decision Log]
- 목적과 의도: Make builtin replacement's owned native storage catchable and metered, while removing repeated UTF-16 scans and preserving every observable RegExp replacement ordering rule.
- 기존 구현 및 제약 조건: Results, captures, callback arguments, substitutions, and output grew infallibly; source ranges rescanned UTF-8 from the beginning; empty-match advancement allocated a fresh UTF-16 vector and narrowed ToLength to usize; final UTF-16 decoding and callback string preparation copied through infallible buffers.
- 검토한 주요 대안: Pre-reserve all storage at entry, retain per-slice conversion, build every string as owned UTF-16, publish partial output before callbacks, redesign all JS strings/property keys in the same patch, or isolate the containers directly owned by @@replace.
- 선택한 방식: Preserve observable coercions and global lastIndex setup; borrow ASCII or fallibly cache non-ASCII input once; reserve each phase at its specification boundary; append substitutions and output as UTF-16; retain existing Arc strings; decode through one exact fallible String buffer; inject failures only at real growth.
- 다른 대안 대신 이 방식을 선택한 이유: Entry reservation changes trap order and over-reserves paths never taken; repeated conversion is superlinear; universal UTF-16 ownership penalizes ASCII; partial output complicates abrupt completion; a runtime-wide JS-string/key allocator is too broad for one auditable unit. Phase ownership gives deterministic failure and retry evidence without claiming unrelated allocations.
- 장점, 단점 및 영향: Replacement container failures are Realm-correct RangeErrors, Fuel has exact boundaries, pins and native temporaries unwind on every abrupt path, Unicode slicing is linear in copied output, and large lastIndex cannot overflow wasm32 usize. ToString-created and final-result Arc<str> publication, dynamic named-group PropertyKey allocation, error-message strings, compiler/backend allocation, and legacy String builtin paths remain runtime-wide follow-ups rather than covered OOM guarantees.
```

### RegExp named-group conformance ownership

Named capture syntax and runtime behavior span parser early errors, matcher
capture metadata, result-object publication, and replacement substitution.
Those implementation paths already serve more RegExp features, so supported
Test262 policy owns the completed named-group surface through one exact
path-to-feature map rather than a directory or feature-wide exception. The
runner and analyzer consume the same immutable map. Existing match-indices and
duplicate-name maps remain separate owners, preventing one feature unit from
silently broadening another.

```text
[Decision Log]
- 목적과 의도: Publish the already conforming named-capture surface as measurable support without admitting unaudited future files or unrelated Symbol behavior.
- 기존 구현 및 제약 조건: Named groups already execute across literals, exec, indices, and replacement, but regexp-named-groups remained a global skip except for match-indices and duplicate-name exact maps; one adjacent replacement test also requires Symbol.iterator.
- 검토한 주요 대안: Remove the feature gate globally, admit directory prefixes, merge every RegExp admission, keep informational forced runs only, or freeze the independently passing paths under a shared exact map.
- 선택한 방식: Freeze 86 disjoint paths, remove only regexp-named-groups for those paths in both policy tools, retain poisoned-stdlib.js as one explicit scope skip, and hard-gate exact and related-scope counts in CI.
- 다른 대안 대신 이 방식을 선택한 이유: Global and prefix admission accept future behavior without review; merging ownership obscures dependencies; forced runs do not improve supported accounting; lifting Symbol.iterator here would claim unrelated semantics.
- 장점, 단점 및 영향: Supported accounting gains 86 passes with no runtime semantic change, future siblings remain closed, and one unrelated dependency stays visible. Native compiler-allocation fallibility remains a separate runtime-hardening unit.
```

### VM-local compiled RegExp matcher cache

RegExp construction and builtin execution share immutable compiled matchers
through one VM-local LRU. A key owns the canonical source `Arc<str>`, the five
compiler-semantic flag bits `i/m/s/u/v`, and one input domain:
scalar-preferred, UTF-16 code units, or logical UTF-16 required. The state and
result flags `d/g/y` share a matcher, and Realm identity is deliberately absent
because compiled programs contain no JavaScript heap values. Cache entries are
therefore outside GC tracing and remain valid across collection and GcIdx reuse.

`RegExpBuiltinExec` still performs receiver validation, input `ToString`,
`lastIndex` coercion, and the global/sticky out-of-range reset before lookup.
A miss completes backend selection and fallback before the test-only terminal
failure hook; only a successful terminal matcher is offered to the cache. A hit
leaves that hook armed. Constructor seeding and internal String paths use the
same key rules. Cache publication reserves the native LRU best-effort: failure
drops only the cache candidate and returns the successfully compiled matcher.

Every `CompiledRegex` backend handle is `Arc`-owned at RuJa's enum boundary, so
LRU hits and active-call clones allocate no new backend state and eviction cannot
invalidate a matcher already in use. Vendored `Clone` implementations are left
unchanged. Retention permits at most 16 entries, 256 KiB of source, 64 KiB per
source, and a 128 MiB conservative matcher charge. Rust compilation pins its
existing 10 MiB NFA and 2 MiB lazy-DFA limits explicitly. Capture-free regular
patterns use `regex-automata` directly with one mutex-protected execution cache,
instead of the high-level backend's hidden scratch pool. Their charge includes
the immutable program, initial cache allocation, wrapper structures, and a
conservative four-times overhead over each forward and reverse 512 KiB lazy-DFA
cache capacity for vector slack and hash-table buckets. After three inefficient
cache clears, the matcher permanently switches to its retained, finitely
charged PikeVM program. PikeVM scratch is created and dropped per fallback call
rather than retained. This finite charge lets ordinary Rust and logical UTF-16
programs coexist under the same LRU budget. The logical backend reports its own
deliberately overestimated compiled-storage charge. Fancy,
prefiltered/capture-corrected
composite, captured Rust, overflowing, and oversized matchers execute normally
but are not cached because a finite total retained bound is unavailable at
their public API boundary.

```text
[Decision Log]
- 목적과 의도: Reuse terminal RegExp compilation without changing ECMAScript observation order, GC ownership, Realm error identity, or the sandbox's native-memory bound.
- 기존 구현 및 제약 조건: Every constructor/exec/internal String route rebuilt a matcher; backend Clone could duplicate programs or scratch pools; regex-automata retains mutable execution caches; fancy/composite backends do not expose a finite total retained-memory bound; cache allocation itself must not turn successful compilation into JavaScript failure.
- 검토한 주요 대안: Store a matcher on every RegExp heap object, use a process-global cache, key by all flags or Realm, cache every high-level backend by source length, change vendor Clone semantics, select a backend from current cache contents, or keep a VM-local semantic LRU with an explicit finite Rust cache.
- 선택한 방식: Keep one VM-owned LRU keyed by source, i/m/s/u/v, and input domain; Arc-wrap backend values only in RuJa; publish after terminal success on a best-effort basis; enforce checked entry/source/matcher accounting; use one explicitly owned regex-automata hybrid cache plus a retained PikeVM program for capture-free regular patterns; charge four times both 512 KiB cache capacities and all three retained NFAs; permanently switch to PikeVM with per-call scratch after repeated inefficient clears; admit only finitely charged Rust and logical programs while running all other variants uncached.
- 다른 대안 대신 이 방식을 선택한 이유: Per-object storage scales with live heap objects and needs GC slot ownership; a global cache crosses VM policy boundaries; d/g/y and Realm do not alter compilation; source size does not bound a hidden scratch pool; cache-state backend selection can change observable resource-limit behavior; vendor Arc conversion changes public no_std and clone behavior. Explicit cache ownership preserves stable matcher semantics and a defensible memory ceiling.
- 장점, 단점 및 영향: Repeated common matchers and logical fallback programs coexist and reuse allocation-free handles across Realms and GC, publication failure is invisible, failures remain uncached, and reentrant eviction is safe. Explicit-cache searches serialize per matcher, while different matchers remain independent. Saturated matchers avoid repeated hybrid work and recompilation but pay a fresh bounded PikeVM scratch allocation per call. Fancy, composite, and captured Rust patterns still recompile; broader reuse requires finite retained-memory APIs for those backends.
```

### Residual built-in subclass ownership

The broad `class/subclass-builtins` policy owns only its established constructor
families. Later constructor families use exact path-to-feature rows rather than
expanding that prefix allow-list. `SharedArrayBuffer` and `WeakRef` therefore
contribute exactly four admitted files: declaration and expression forms for
each constructor. Runner and analyzer consume the same immutable map, while
tooling verifies live metadata and rejects future siblings, outside paths, and
extra unsupported dependencies.

```text
[Decision Log]
- 목적과 의도: Keep built-in subclass conformance ownership closed as newly implemented constructors are added.
- 기존 구현 및 제약 조건: The existing prefix policy predates SharedArrayBuffer and WeakRef; adding their feature names there would automatically own unknown future files under the directory.
- 검토한 주요 대안: Expand the prefix feature set, create separate prefix rules, leave the tests skipped, or use one shared exact map.
- 선택한 방식: Add four exact path-to-feature rows and apply them after the established prefix policy in both tools.
- 다른 대안 대신 이 방식을 선택한 이유: Exact rows express the complete current residual surface and preserve review of future tests without duplicating runtime logic.
- 장점, 단점 및 영향: Current declaration/expression coverage is complete and symmetric; unrelated feature gates do not move. Each new subclass test requires explicit metadata review.
```

### AsyncIteratorClose and guarded control transfers

Async iteration lowers abrupt close into three bytecode phases around the
ordinary Await opcode: start/get/call, awaited-result validation, and caught
close-error precedence. This keeps async function, Module, and async-generator
suspension on the existing continuation machinery. Async-from-sync iterators
delegate close to their wrapper operation so the underlying iterator-result
value is unwrapped before the wrapper result settles.

Finally guards snapshot the environment and clean operand-stack depth at entry.
All return, throw, break, and continue diversions restore that snapshot before
running finally code. A for-of value is moved to a compiler temporary before
the guard is installed and the temporary is cleared immediately after reload,
preventing partial LHS references and retained last values from surviving an
awaited close.

Guarded bytecode ranges also define break/continue ownership. The runtime
follows the compiler's linear cleanup trampoline through scope/catch cleanup to
the first Jump. If that logical destination remains inside an outer guard, the
completion resumes without entering that outer finally. Destinations outside
the range continue propagating guard by guard. This distinction prevents both
premature outer finally execution and skipped outer finally blocks.

```text
[Decision Log]
- 목적과 의도: AsyncIteratorClose의 suspension과 finally control transfer를 operand, environment, GC 관점에서 손실 없이 표현한다.
- 기존 구현 및 제약 조건: finally guard는 target/sequence만 저장했고 diversion은 stack/env를 복원하지 않았다. inner iterator-finally completion은 실제 break/continue 목표가 outer try 내부인지와 무관하게 모든 outer guard로 전파됐다.
- 검토한 주요 대안: close 전용 Rust state machine, frame 전체 clone, compiler가 모든 transfer마다 outer-finally 개수를 인코딩, 또는 guard snapshot과 protected bytecode range를 사용하기.
- 선택한 방식: 기존 Await bytecode와 completion stack을 재사용하고 guard에 start/target/env/stack depth를 저장한다. compiler cleanup trampoline의 logical destination으로 outer guard 포함 여부를 판정한다.
- 다른 대안 대신 이 방식을 선택한 이유: 별도 state machine과 frame clone은 catch/finally 상태를 중복하고 GC surface를 늘린다. 개수 인코딩은 각 transfer opcode와 patching 계약을 넓힌다. bytecode range는 현재 compiler가 이미 보장하는 선형 cleanup 형식을 직접 사용한다.
- 장점, 단점 및 영향: partial Reference가 await continuation에 남지 않고 saved env가 모든 root visitor에 포함된다. same-loop/outer-loop continue와 nested finally가 정확히 분리된다. cleanup opcode 형식이 trampoline 또는 다중 Jump로 바뀌면 destination resolver와 회귀 테스트도 함께 변경해야 한다.
```

**Next:** [Features](features.md) · [Known limitations](limitations.md) · [Back to README](../README.md)
