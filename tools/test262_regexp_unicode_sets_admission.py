"""Frozen Test262 RegExp v-mode character-only set-operation admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_regexp_unicode_sets_admission.txt")
REGEXP_UNICODE_SETS_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
_OPERANDS = (
    "character",
    "character-class",
    "character-class-escape",
    "character-property-escape",
)
_OPERATIONS = ("union", "intersection", "difference")
_EXPECTED_FILES = frozenset(
    f"built-ins/RegExp/unicodeSets/generated/{left}-{operation}-{right}.js"
    for left in _OPERANDS
    for operation in _OPERATIONS
    for right in _OPERANDS
)
REGEXP_UNICODE_SETS_FEATURES = {
    relative: frozenset(
        {"regexp-v-flag"}
        | (
            {"regexp-unicode-property-escapes"}
            if "character-property-escape" in relative
            else set()
        )
    )
    for relative in REGEXP_UNICODE_SETS_FILES
}

if frozenset(REGEXP_UNICODE_SETS_FEATURES) != REGEXP_UNICODE_SETS_FILES:
    raise RuntimeError("RegExp Unicode sets admission manifest is out of sync")
if REGEXP_UNICODE_SETS_FILES != _EXPECTED_FILES:
    raise RuntimeError("RegExp Unicode sets admission is not the character-only matrix")
