"""Frozen Test262 TypedArray.prototype.toString metadata."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_typed_array_to_string_admission.txt")
TYPED_ARRAY_TO_STRING_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(relative: str) -> frozenset[str]:
    features = {"TypedArray"}
    if relative.endswith("/BigInt/detached-buffer.js"):
        features.add("BigInt")
    if relative.endswith("/not-a-constructor.js"):
        features.update({"Reflect.construct", "arrow-function"})
    return frozenset(features)


TYPED_ARRAY_TO_STRING_FEATURES = {
    relative: _features(relative) for relative in TYPED_ARRAY_TO_STRING_FILES
}

if frozenset(TYPED_ARRAY_TO_STRING_FEATURES) != TYPED_ARRAY_TO_STRING_FILES:
    raise RuntimeError("TypedArray toString admission manifest and feature map differ")
