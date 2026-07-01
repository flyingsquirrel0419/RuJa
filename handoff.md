# RuJa Session Handoff

Date: 2026-07-01
Last turn: full test262 workflow parallelization + skip-list expansion

## Project state

- Project root: `/root/RuJa`
- GitHub repo: `https://github.com/flyingsquirrel0419/RuJa`
- Current HEAD: `537e30a`
- Worktree status: clean
- Author/Committer preserved as `flyingsquirrel0419`

## Recent work

1. GC `with_obj` reentrancy use-after-free
2. `Array.from` iterable support
3. `Number.prototype.toString(radix)` shortest round-trip precision
4. Full test262 CI workflow + expanded skip features

New dependency: `num-rational = "0.4"`

## test262 status

- Local test262 path: `/root/test262`
- Local test files: 21,031
- Harness files: 42
- CI workflow: `.github/workflows/test262-full.yml`
  - build: compile release ruja and upload artifact
  - setup: dynamic directory matrix
  - test262-full: parallel run per directory
  - summary: aggregate pass rate

### First full-run baseline (run 28461793818)

- Total: 76,397
- Ran: 60,178
- Pass: 19,987
- Fail: 40,191
- Skip: 15,481
- Pass rate: 33.2%
- Longest jobs: language/expressions (22m21s), language/statements (25m34s)

### Expanded skip features

class / static / fields / decorators, async-functions / async-iteration /
generators, Symbol family, Proxy/Reflect, TypedArray variants, ArrayBuffer /
SharedArrayBuffer, Map / Set / WeakMap / WeakSet / Promise / AggregateError,
Intl, iterator-helpers, explicit-resource-management, import-assertions /
import-attributes / import.meta / dynamic-import, module / top-level-await /
hashbang / json-modules, object-rest / object-spread, rest-parameters,
destructuring-*, optional-chaining, logical-assignment-operators,
regexp advanced flags, tail-call-optimization, u180e, shadowrealm.

## Known gaps

- UTF-16 string model migration
- RegExp stateful lastIndex / global match
- JSON.stringify options (replacer, space, reviver, toJSON)
- Map / Set iterators beyond basics
- Live progress flushing in CI runner

## Verification commands

```bash
CARGO_BUILD_JOBS=1 cargo test --release
cargo clippy --all-targets
cargo fmt -- --check
```

Targeted test262:

```bash
TEST262=/root/test262 python3 tools/test262_runner.py <subdir>
TEST262=/root/test262 python3 tools/test262_analyze.py <subdir>
```

Workflow links:
- CI subset: https://github.com/flyingsquirrel0419/RuJa/actions/workflows/ci.yml
- Full: https://github.com/flyingsquirrel0419/RuJa/actions/workflows/test262-full.yml

## Context paths

- Rollout context: `/root/.codex/sessions/2026/06/29/rollout-2026-06-29T00-51-18-019f0eed-92cf-7d72-ad0a-7c7b532026ce.jsonl`
- Memory registry: `/root/.codex/memories/MEMORY.md` (search `RuJa`)
