"""Frozen base `Intl.Locale` Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_intl_locale_admission.txt")
INTL_LOCALE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

INTL_LOCALE_FEATURES = {
    relative: frozenset({"Intl.Locale"}) for relative in INTL_LOCALE_FILES
}
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
