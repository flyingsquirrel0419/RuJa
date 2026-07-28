"""Frozen Test262 admission for collision-sensitive Unicode RegExp properties."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_regexp_logical_utf16_admission.txt")
REGEXP_LOGICAL_UTF16_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
_EXPECTED_FILES = frozenset(
    {
        "built-ins/RegExp/property-escapes/generated/General_Category_-_Private_Use.js",
        "built-ins/RegExp/property-escapes/generated/General_Category_-_Surrogate.js",
    }
)
REGEXP_LOGICAL_UTF16_FEATURES = {
    relative: frozenset({"regexp-unicode-property-escapes"})
    for relative in REGEXP_LOGICAL_UTF16_FILES
}

if REGEXP_LOGICAL_UTF16_FILES != _EXPECTED_FILES:
    raise RuntimeError("logical UTF-16 RegExp admission contains an unexpected path")
if frozenset(REGEXP_LOGICAL_UTF16_FEATURES) != REGEXP_LOGICAL_UTF16_FILES:
    raise RuntimeError("logical UTF-16 RegExp admission manifest is out of sync")
