"""Frozen `Intl.Collator` and `String.prototype.localeCompare` admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_intl_collator_admission.txt")
INTL_COLLATOR_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

INTL_COLLATOR_FEATURES = {
    relative: frozenset() for relative in INTL_COLLATOR_FILES
}
for relative in (
    "intl402/Collator/prototype/compare/builtin.js",
    "intl402/Collator/prototype/compare/compare-function-builtin.js",
    "intl402/Collator/prototype/resolvedOptions/builtin.js",
    "intl402/Collator/supportedLocalesOf/builtin.js",
    "intl402/String/prototype/localeCompare/builtin.js",
):
    INTL_COLLATOR_FEATURES[relative] = frozenset({"Reflect.construct"})
INTL_COLLATOR_FEATURES["intl402/Collator/proto-from-ctor-realm.js"] = frozenset(
    {"cross-realm", "Reflect", "Symbol"}
)
for name in (
    "toString-changed-tag.js",
    "toString-removed-tag.js",
    "toString.js",
    "toStringTag.js",
):
    INTL_COLLATOR_FEATURES[
        f"intl402/Collator/prototype/toStringTag/{name}"
    ] = frozenset({"Symbol.toStringTag"})


def intl_collator_features(relative_path):
    """Return the complete audited feature set for one admitted file."""
    return INTL_COLLATOR_FEATURES.get(relative_path, frozenset())
