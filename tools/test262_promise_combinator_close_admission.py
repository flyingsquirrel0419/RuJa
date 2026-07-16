"""Frozen Test262 Promise combinator IteratorClose admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_promise_combinator_close_admission.txt"
)
PROMISE_COMBINATOR_CLOSE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

PROMISE_COMBINATOR_CLOSE_FEATURES = {}
for relative in PROMISE_COMBINATOR_CLOSE_FILES:
    if "/allSettled/" in relative:
        features = {"Promise.allSettled", "Symbol.iterator"}
    elif "/any/" in relative:
        features = {
            "Promise.any",
            "Symbol.iterator",
            "computed-property-names",
            "Symbol",
            "arrow-function",
        }
    else:
        features = {"Symbol.iterator"}
    PROMISE_COMBINATOR_CLOSE_FEATURES[relative] = frozenset(features)
