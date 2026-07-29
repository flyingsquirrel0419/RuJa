"""Frozen `%Intl%` and `Intl.getCanonicalLocales` Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_intl_canonical_locales_admission.txt")
INTL_CANONICAL_LOCALES_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

INTL_CANONICAL_LOCALES_FEATURES = {
    "intl402/Intl/getCanonicalLocales/error-cases.js": frozenset({"Symbol"}),
    "intl402/Intl/getCanonicalLocales/has-property.js": frozenset({"Proxy"}),
    "intl402/Intl/getCanonicalLocales/locales-is-not-a-string.js": frozenset({"Symbol"}),
    "intl402/Intl/getCanonicalLocales/overriden-arg-length.js": frozenset({"Symbol"}),
    "intl402/Intl/toStringTag/toString.js": frozenset({"Symbol.toStringTag"}),
    "intl402/Intl/toStringTag/toStringTag.js": frozenset({"Symbol.toStringTag"}),
}


def intl_canonical_locales_features(relative_path):
    """Return only broad gates audited for one frozen Intl file."""
    if relative_path not in INTL_CANONICAL_LOCALES_FILES:
        return frozenset()
    return INTL_CANONICAL_LOCALES_FEATURES.get(relative_path, frozenset())
