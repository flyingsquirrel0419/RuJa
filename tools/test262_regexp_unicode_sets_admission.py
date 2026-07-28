"""Frozen Test262 RegExp v-mode generated Unicode set admission."""

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
    "property-of-strings-escape",
    "string-literal",
)
_OPERATIONS = ("union", "intersection", "difference")
_EXPECTED_MATRIX_FILES = frozenset(
    f"built-ins/RegExp/unicodeSets/generated/{left}-{operation}-{right}.js"
    for left in _OPERANDS
    for operation in _OPERATIONS
    for right in _OPERANDS
)
_EXPECTED_RGI_FILES = frozenset(
    f"built-ins/RegExp/unicodeSets/generated/rgi-emoji-{version}.js"
    for version in ("13.1", "14.0", "15.0", "15.1", "16.0", "17.0")
)
_STRING_PROPERTIES = (
    "Basic_Emoji",
    "Emoji_Keycap_Sequence",
    "RGI_Emoji",
    "RGI_Emoji_Flag_Sequence",
    "RGI_Emoji_Modifier_Sequence",
    "RGI_Emoji_Tag_Sequence",
    "RGI_Emoji_ZWJ_Sequence",
)
_STRING_PROPERTY_SUFFIXES = (
    "",
    "-negative-CharacterClass",
    "-negative-P",
    "-negative-u",
)
_EXPECTED_STRING_PROPERTY_FILES = frozenset(
    f"built-ins/RegExp/property-escapes/generated/strings/{property_name}{suffix}.js"
    for property_name in _STRING_PROPERTIES
    for suffix in _STRING_PROPERTY_SUFFIXES
)
_EXPECTED_FILES = (
    _EXPECTED_MATRIX_FILES | _EXPECTED_RGI_FILES | _EXPECTED_STRING_PROPERTY_FILES
)
REGEXP_UNICODE_SETS_FEATURES = {
    relative: frozenset(
        {"regexp-v-flag", "regexp-unicode-property-escapes"}
        if "/property-escapes/generated/strings/" in relative
        else {"regexp-v-flag"}
        | (
            {"regexp-unicode-property-escapes"}
            if (
                "character-property-escape" in relative
                or "property-of-strings-escape" in relative
                or "/rgi-emoji-" in relative
            )
            else set()
        )
    )
    for relative in REGEXP_UNICODE_SETS_FILES
}

if frozenset(REGEXP_UNICODE_SETS_FEATURES) != REGEXP_UNICODE_SETS_FILES:
    raise RuntimeError("RegExp Unicode sets admission manifest is out of sync")
if REGEXP_UNICODE_SETS_FILES != _EXPECTED_FILES:
    raise RuntimeError("RegExp Unicode sets admission is not the generated string-set corpus")
