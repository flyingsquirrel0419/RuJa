"""Frozen Test262 TypedArray.prototype.toLocaleString metadata."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_typed_array_to_locale_string_admission.txt"
)
TYPED_ARRAY_TO_LOCALE_STRING_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_BASE = "built-ins/TypedArray/prototype/toLocaleString/"
_RAB_ONLY = frozenset(
    {
        f"{_BASE}resizable-buffer.js",
        f"{_BASE}user-provided-tolocalestring-grow.js",
        f"{_BASE}user-provided-tolocalestring-shrink.js",
    }
)


def _features(relative: str) -> frozenset[str]:
    if relative in _RAB_ONLY:
        return frozenset({"resizable-arraybuffer"})

    features = {"TypedArray"}
    if relative.startswith(f"{_BASE}BigInt/"):
        features.add("BigInt")
    if relative.endswith("return-abrupt-from-this-out-of-bounds.js"):
        features.update(
            {"ArrayBuffer", "arrow-function", "resizable-arraybuffer"}
        )
    if relative == f"{_BASE}not-a-constructor.js":
        features.update({"Reflect.construct", "arrow-function"})
    if relative == f"{_BASE}this-is-not-object.js":
        features.add("Symbol")
    return frozenset(features)


TYPED_ARRAY_TO_LOCALE_STRING_FEATURES = {
    relative: _features(relative) for relative in TYPED_ARRAY_TO_LOCALE_STRING_FILES
}

if (
    frozenset(TYPED_ARRAY_TO_LOCALE_STRING_FEATURES)
    != TYPED_ARRAY_TO_LOCALE_STRING_FILES
):
    raise RuntimeError(
        "TypedArray toLocaleString admission manifest and feature map differ"
    )
