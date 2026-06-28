# Contributing to RuJa

First off, thanks for taking the time to contribute — that's genuinely
appreciated. RuJa is a small project, so the bar is care and correctness
rather than scale.

This guide covers the workflow for getting a change merged into `main`.
For the engine's internal design, see [docs/architecture.md](docs/architecture.md);
for what is and isn't implemented, see [docs/features.md](docs/features.md)
and [docs/limitations.md](docs/limitations.md).

## Before you start

- RuJa is pre-1.0 alpha software. Breaking changes are acceptable, but
  must be called out in the PR description and `CHANGELOG.md`.
- The engine targets ES2015-class features plus a growing slice of later
  spec. When in doubt about whether something is in scope, open an issue
  first.
- Security reports go through [SECURITY.md](SECURITY.md), not a public
  issue or PR.

## Development setup

RuJa builds on stable Rust (1.96.0 or newer).

```bash
git clone https://github.com/flyingsquirrel0419/RuJa.git
cd RuJa
cargo build
cargo test
```

There are no hidden dependencies or build scripts — `cargo` is all you
need.

## Repo layout

| Path | Contents |
| --- | --- |
| `src/lexer.rs` | Tokenizer |
| `src/parser.rs`, `src/ast.rs`, `src/token.rs` | Parser and AST |
| `src/compiler.rs`, `src/bytecode.rs` | Bytecode compiler |
| `src/vm.rs` | Bytecode VM (the hot path) |
| `src/gc.rs` | Mark-and-sweep garbage collector |
| `src/value.rs` | Runtime values |
| `src/builtins.rs` | Built-in globals (`Object`, `Array`, `Promise`, …) |
| `src/function.rs`, `src/environment.rs` | Function/closure machinery |
| `src/error.rs` | Error model |
| `src/bin/ruja.rs` | The `ruja` CLI binary |
| `tests/` | Integration tests |
| `examples/` | Runnable `.js` samples |
| `docs/` | Architecture, features, limitations docs |

## Workflow

1. Fork the repo and create a branch off `main`:
   ```bash
   git checkout -b fix/my-change
   ```
2. Make your change. Keep diffs scoped to the module or behavior you're
   touching; avoid unrelated refactors in the same PR.
3. Add or update tests under `tests/` covering the new or fixed behavior.
   Integration tests are the norm here — there is no separate `src`-level
   unit test convention to match.
4. Update `CHANGELOG.md` under the `[Unreleased]` section, following the
   existing entry style (one line per change, grouped by Added / Changed /
   Fixed / Removed).
5. Run the full local check before pushing:
   ```bash
   cargo test
   cargo fmt --all -- --check
   cargo clippy --all-targets -- -D warnings
   ```
6. Commit with a clear message. We don't enforce a strict format, but a
   short imperative summary plus an optional body works well, e.g.:
   ```
   fix(vm): enforce writable:false in set_property
   ```
7. Push and open a pull request. Fill in the PR template; the checklist
   mirrors what CI will run.

## Code style

- `cargo fmt` is authoritative for formatting. Don't hand-format.
- Clippy runs with `-D warnings` in CI — fix or justify every lint.
- Prefer the engine's existing patterns over introducing new abstractions.
  In particular, reach for `Op` handlers and the existing `Value` /
  `Property` shapes before adding new top-level machinery.
- Keep `interpret_inner` readable: new ops belong as a match arm, and
  sizeable handlers can be split into helper methods on the VM.
- Comments are welcome where behavior is non-obvious, but skip narration
  that just restates the code.

## Tests

- Integration tests live in `tests/` and use the `run` / `run_err`
  helpers from `tests/common`. `run` panics on a runtime error, so use
  `run_err` when you need to assert that a specific error is raised.
- Deep-nesting and large-stack tests run on a dedicated worker thread
  (the VM is not `Send`). Follow the pattern in `tests/bugfixes.rs` when
  a test needs a bigger stack.
- Every behavior change should come with at least one test that would
  fail before the change and pass after.

## CHANGELOG

Add your entry under `[Unreleased]`, grouped by:

- **Added** — new features or capabilities
- **Changed** — changes to existing behavior
- **Fixed** — bug fixes
- **Removed** — removed features

Keep entries user-facing: a reader of the changelog should understand
what changed without reading the diff.

## Commit messages

No rigid convention, but please:

- Use the imperative mood in the summary ("add" not "added").
- Reference the issue or PR number in the body when relevant (`Closes #123`).
- Keep the summary under ~72 characters.

## Pull requests

- One logical change per PR. If a PR grows into several, split it.
- Fill in the PR template checklist. CI runs the same checks locally, so
  if `cargo test` / `fmt` / `clippy` pass on your machine, CI should pass
  too.
- Be patient with review. This is a small project maintained in spare
  time.

## Reporting issues

Use the issue templates (bug report / feature request). For bugs,
include the JS input that reproduces the problem, the RuJa version or
commit, expected vs. actual behavior, and your platform.

## License

By contributing, you agree that your contributions are licensed under the
[Apache-2.0](LICENSE) license that covers the project.
