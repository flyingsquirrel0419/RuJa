"""Frozen Test262 Await Dictionary Promise keyed admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_promise_keyed_admission.txt")
PROMISE_KEYED_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_EXTRA_FEATURES_BY_NAME = {
    "arg-not-object-reject-bigint.js": {"BigInt"},
    "arg-not-object-reject.js": {"Symbol"},
    "ctx-ctor-constructed.js": {"new.target"},
    "getownproperty-not-enumerable.js": {"Reflect"},
    "getownproperty-returns-undefined.js": {"Proxy", "Reflect"},
    "getownproperty-throws.js": {"Proxy"},
    "not-a-constructor.js": {"Reflect.construct", "arrow-function"},
    "ownkeys-throws.js": {"Proxy"},
    "resolve-before-loop-exit.js": {"Reflect"},
    "symbol-keys.js": {"Symbol"},
}

PROMISE_KEYED_FEATURES = {
    relative: frozenset(
        {"await-dictionary"}
        | _EXTRA_FEATURES_BY_NAME.get(relative.rsplit("/", 1)[-1], set())
    )
    for relative in PROMISE_KEYED_FILES
}

if frozenset(PROMISE_KEYED_FEATURES) != PROMISE_KEYED_FILES:
    raise RuntimeError("Promise keyed admission manifest is out of sync")
if any(
    not relative.startswith(
        (
            "built-ins/Promise/allKeyed/",
            "built-ins/Promise/allSettledKeyed/",
        )
    )
    for relative in PROMISE_KEYED_FILES
):
    raise RuntimeError("Promise keyed admission contains an unrelated path")
