"""Frozen Test262 Promise finally and reaction admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_promise_finally_admission.txt")
PROMISE_FINALLY_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_SPECIAL_FINALLY_FEATURES = {
    "invokes-then-with-function.js": {"Reflect.construct", "arrow-function"},
    "not-a-constructor.js": {"Reflect.construct", "arrow-function"},
    "rejected-observable-then-calls-argument.js": {
        "Reflect.construct",
        "arrow-function",
        "class",
    },
    "resolved-observable-then-calls-argument.js": {
        "Reflect.construct",
        "arrow-function",
    },
    "species-constructor-throws.js": {"Promise"},
    "this-value-then-not-callable.js": {"Symbol"},
}

PROMISE_FINALLY_FEATURES = {}
for relative in PROMISE_FINALLY_FILES:
    if "/prototype/finally/" in relative:
        features = {"Promise.prototype.finally"}
        features.update(_SPECIAL_FINALLY_FEATURES.get(Path(relative).name, set()))
    elif "/allSettled/" in relative:
        features = {"Promise.allSettled"}
    elif "/resolve/" in relative:
        features = {"Symbol"}
    else:
        features = set()
    PROMISE_FINALLY_FEATURES[relative] = frozenset(features)
