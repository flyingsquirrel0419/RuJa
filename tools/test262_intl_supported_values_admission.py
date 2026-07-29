"""Frozen standalone `Intl.supportedValuesOf` Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_intl_supported_values_admission.txt")
INTL_SUPPORTED_VALUES_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

INTL_SUPPORTED_VALUES_FEATURES = {
    relative: frozenset({"Intl-enumeration"})
    for relative in INTL_SUPPORTED_VALUES_FILES
}
INTL_SUPPORTED_VALUES_FEATURES[
    "intl402/Intl/supportedValuesOf/builtin.js"
] |= frozenset({"Reflect.construct"})
INTL_SUPPORTED_VALUES_FEATURES[
    "intl402/Intl/supportedValuesOf/calendars-required-by-intl-era-monthcode.js"
] |= frozenset({"Intl.Era-monthcode"})
for _name in ("calendars.js", "collations.js"):
    INTL_SUPPORTED_VALUES_FEATURES[
        f"intl402/Intl/supportedValuesOf/{_name}"
    ] |= frozenset({"Intl.Locale", "Array.prototype.includes"})
INTL_SUPPORTED_VALUES_FEATURES[
    "intl402/Intl/supportedValuesOf/numberingSystems.js"
] |= frozenset({"Intl.Locale"})
for _name in (
    "numberingSystems-with-simple-digit-mappings.js",
    "units.js",
):
    INTL_SUPPORTED_VALUES_FEATURES[
        f"intl402/Intl/supportedValuesOf/{_name}"
    ] |= frozenset({"Array.prototype.includes"})


def intl_supported_values_features(relative_path):
    """Return broad gates audited for one frozen supportedValuesOf file."""
    return INTL_SUPPORTED_VALUES_FEATURES.get(relative_path, frozenset())
