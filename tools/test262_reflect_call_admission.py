"""Frozen Test262 Reflect.apply and Reflect.construct admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_reflect_call_admission.txt")
REFLECT_CALL_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

_SPECIAL_FEATURES = {
    "apply/arguments-list-is-not-array-like-but-still-valid.js": {
        "arrow-function",
        "Symbol",
    },
    "apply/arguments-list-is-not-array-like.js": {
        "arrow-function",
        "Symbol",
    },
    "apply/not-a-constructor.js": {"Reflect.construct", "arrow-function"},
    "construct/not-a-constructor.js": {"arrow-function"},
}

REFLECT_CALL_FEATURES = {}
for relative in REFLECT_CALL_FILES:
    features = {"Reflect"}
    suffix = relative.removeprefix("built-ins/Reflect/")
    if suffix.startswith("construct/") and suffix != "construct/construct.js":
        features.add("Reflect.construct")
    features.update(_SPECIAL_FEATURES.get(suffix, set()))
    REFLECT_CALL_FEATURES[relative] = frozenset(features)
