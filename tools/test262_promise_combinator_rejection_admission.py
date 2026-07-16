"""Frozen Test262 Promise combinator setup-rejection admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_promise_combinator_rejection_admission.txt"
)
PROMISE_COMBINATOR_REJECTION_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_NO_FEATURE_FILES = {
    "built-ins/Promise/all/S25.4.4.1_A3.1_T1.js",
    "built-ins/Promise/all/S25.4.4.1_A3.1_T2.js",
    "built-ins/Promise/race/S25.4.4.3_A2.2_T1.js",
    "built-ins/Promise/race/S25.4.4.3_A2.2_T2.js",
    "built-ins/Promise/race/invoke-resolve-get-error-reject.js",
}

PROMISE_COMBINATOR_REJECTION_FEATURES = {}
for relative in PROMISE_COMBINATOR_REJECTION_FILES:
    if relative in _NO_FEATURE_FILES:
        features = set()
    elif relative.endswith("resolve-not-callable-reject-with-typeerror.js"):
        if "/allSettled/" in relative:
            features = {"Promise.allSettled", "arrow-function"}
        elif "/any/" in relative:
            features = {"Promise.any", "arrow-function"}
        else:
            features = {"arrow-function"}
    elif "/allSettled/" in relative:
        features = {"Promise.allSettled", "Symbol.iterator"}
    elif "/any/" in relative:
        if "/iter-assigned-" in relative:
            features = {
                "Promise.any",
                "Symbol",
                "Symbol.iterator",
                "computed-property-names",
            }
        elif relative.endswith("iter-arg-is-symbol-reject.js"):
            features = {"Promise.any", "Symbol"}
        elif "/iter-arg-is-" in relative:
            features = {"Promise.any"}
        else:
            features = {"Promise.any", "Symbol.iterator"}
    else:
        features = {"Symbol.iterator"}
    PROMISE_COMBINATOR_REJECTION_FEATURES[relative] = frozenset(features)
