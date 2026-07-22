"""Frozen feature-gated Test262 Array.prototype.toLocaleString files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_array_to_locale_string_admission.txt")
ARRAY_TO_LOCALE_STRING_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

ARRAY_TO_LOCALE_STRING_FEATURES = {
    "built-ins/Array/prototype/toLocaleString/not-a-constructor.js": frozenset(
        {"Reflect.construct", "arrow-function"}
    ),
    "built-ins/Array/prototype/toLocaleString/resizable-buffer.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/toLocaleString/user-provided-tolocalestring-grow.js": frozenset(
        {"resizable-arraybuffer"}
    ),
    "built-ins/Array/prototype/toLocaleString/user-provided-tolocalestring-shrink.js": frozenset(
        {"resizable-arraybuffer"}
    ),
}

if frozenset(ARRAY_TO_LOCALE_STRING_FEATURES) != ARRAY_TO_LOCALE_STRING_FILES:
    raise RuntimeError("Array toLocaleString admission manifest and feature map differ")
