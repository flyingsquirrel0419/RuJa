# RuJa Session Handoff

Date: 2026-06-30  
Last turn: test262 test count check + handoff creation

## Project state

- Project root: `/root/RuJa`
- GitHub repo: `https://github.com/flyingsquirrel0419/RuJa`
- Current HEAD: `5d5f8a3`
  ```
  5d5f8a3 fix: shortest round-trip precision for Number.prototype.toString(radix)
  ```
- Worktree status: **clean** (`git status --short` empty)
- Author/Committer preserved as `flyingsquirrel0419`

## Recently completed fixes

1. GC `with_obj` reentrancy use-after-free — `db4bb5c`
2. `Array.from` iterable support — `db4bb5c`
3. `Number.prototype.toString(radix)` shortest round-trip precision — `5d5f8a3`

New dependency added this turn:

- `num-rational = "0.4"` (already pulled in via `Cargo.lock`)

## test262 harness status

- test262 clone path: `/root/test262` (external to this repo, not committed)
- Total test files under `/root/test262/test`: **21,031**
- Harness files under `/root/test262/harness`: **42**
- Runner/analyzer scripts: `tools/test262_runner.py`, `tools/test262_analyze.py`
- Default runner invocation: `python3 tools/test262_runner.py [subdirectory]`
  - Default subdirectory: `language/expressions`
  - Skips `_FIXTURE` files automatically
  - Recognized environment variable: `TEST262=/root/test262`

Currently skipped features (hard-coded in runner):

```text
module, import-assertions, top-level-await, arraybuffer, sharedarraybuffer,
atomics, DataView, TypedArray, Intl, WeakRef, FinalizationRegistry,
AggregateError, resizable-arraybuffer, regexp-v-flag,
regexp-duplicate-named-groups, json-modules, import-attributes, hashbang,
regexp-named-groups, regexp-unicode-property-escapes
```

> Note: Running the full `test262` tree over 21k tests is currently expensive
> and not part of CI; use targeted subsets for regression checks.

## Known remaining gaps

- UTF-16 string model migration (`Value::String` as `Rc<[u16]>`) — large refactor, previously deferred
- `RegExp` stateful behavior (`lastIndex`, global match) — incomplete
- `JSON.stringify` replacer / space / reviver / `toJSON` options — incomplete
- `Map` / `Set` iterators beyond basic support — partial
- Full test262 coverage for builtins, statements, and annex-B behavior

## Baseline verification commands

```bash
CARGO_BUILD_JOBS=1 cargo test --release
cargo clippy --all-targets
cargo fmt -- --check
```

Use `TEST262=/root/test262 python3 tools/test262_runner.py <subdir>` for
targeted conformance runs.

## Helpful context paths

- Rollout context:
  `/root/.codex/sessions/2026/06/29/rollout-2026-06-29T00-51-18-019f0eed-92cf-7d72-ad0a-7c7b532026ce.jsonl`
- Memory registry: `/root/.codex/memories/MEMORY.md` (search `RuJa`)
