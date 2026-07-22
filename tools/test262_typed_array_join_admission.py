"""Frozen Test262 TypedArray.prototype.join metadata."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_typed_array_join_admission.txt")
TYPED_ARRAY_JOIN_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_BASE = "built-ins/TypedArray/prototype/join/"
_RAB_ONLY = frozenset(
    {
        f"{_BASE}coerced-separator-grow.js",
        f"{_BASE}coerced-separator-shrink.js",
        f"{_BASE}resizable-buffer.js",
    }
)


def _features(relative: str) -> frozenset[str]:
    if relative in _RAB_ONLY:
        return frozenset({"resizable-arraybuffer"})

    features = {"TypedArray"}
    is_bigint = relative.startswith(f"{_BASE}BigInt/")
    if is_bigint:
        features.add("BigInt")
    if relative.endswith("detached-buffer-during-fromIndex-returns-single-comma.js"):
        features.add("align-detached-buffer-semantics-with-web-reality")
    if relative.endswith("return-abrupt-from-separator-symbol.js"):
        features.add("Symbol")
    if relative == f"{_BASE}this-is-not-object.js":
        features.add("Symbol")
    if relative.endswith("return-abrupt-from-this-out-of-bounds.js"):
        features.add("resizable-arraybuffer")
        if is_bigint:
            features.update({"ArrayBuffer", "arrow-function"})
    if relative == f"{_BASE}separator-tostring-once-after-resized.js":
        features.add("resizable-arraybuffer")
    if relative == f"{_BASE}not-a-constructor.js":
        features.update({"Reflect.construct", "arrow-function"})
    return frozenset(features)


TYPED_ARRAY_JOIN_FEATURES = {
    relative: _features(relative) for relative in TYPED_ARRAY_JOIN_FILES
}

if frozenset(TYPED_ARRAY_JOIN_FEATURES) != TYPED_ARRAY_JOIN_FILES:
    raise RuntimeError("TypedArray join admission manifest and feature map differ")
