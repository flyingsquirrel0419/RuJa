"""Frozen base and Locale-info `Intl.Locale` Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_intl_locale_admission.txt")
_INFO_MANIFEST = Path(__file__).with_name("test262_intl_locale_info_admission.txt")
INTL_LOCALE_BASE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
INTL_LOCALE_INFO_FILES = frozenset(
    line
    for raw_line in _INFO_MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
INTL_LOCALE_FILES = INTL_LOCALE_BASE_FILES | INTL_LOCALE_INFO_FILES

INTL_LOCALE_FEATURES = {
    relative: frozenset({"Intl.Locale"}) for relative in INTL_LOCALE_BASE_FILES
}
INTL_LOCALE_FEATURES.update(
    {
        relative: frozenset({"Intl.Locale", "Intl.Locale-info"})
        for relative in INTL_LOCALE_INFO_FILES
    }
)
for relative in (
    "intl402/Locale/prototype/getCollations/output-array-values.js",
    "intl402/Locale/prototype/getHourCycles/output-array-values.js",
):
    INTL_LOCALE_FEATURES[relative] = INTL_LOCALE_FEATURES[relative] | {
        "Array.prototype.includes"
    }
for relative in (
    "intl402/Locale/prototype/getWeekInfo/firstDay-by-id.js",
    "intl402/Locale/prototype/getWeekInfo/firstDay-by-option.js",
    "intl402/Locale/prototype/getWeekInfo/output-object-keys.js",
):
    INTL_LOCALE_FEATURES[relative] = INTL_LOCALE_FEATURES[relative] | {"Reflect"}
INTL_LOCALE_FEATURES.update(
    {
        "intl402/Locale/invalid-tag-throws-symbol.js": frozenset(
            {"Intl.Locale", "Symbol"}
        ),
        "intl402/Locale/proto-from-ctor-realm.js": frozenset(
            {"Intl.Locale", "Reflect", "Symbol", "cross-realm"}
        ),
        "intl402/Locale/prototype/toStringTag/toString-removed-tag.js": frozenset(
            {"Intl.Locale", "Symbol.toStringTag"}
        ),
        "intl402/Locale/prototype/toStringTag/toString.js": frozenset(
            {"Intl.Locale", "Symbol.toStringTag"}
        ),
        "intl402/Locale/prototype/toStringTag/toStringTag.js": frozenset(
            {"Intl.Locale", "Symbol.toStringTag"}
        ),
    }
)


def intl_locale_features(relative_path):
    """Return the complete audited feature set for one admitted Locale file."""
    return INTL_LOCALE_FEATURES.get(relative_path, frozenset())
