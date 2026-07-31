"""Frozen Test262 primitive-base Reference files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_reference_primitive_admission.txt")
REFERENCE_PRIMITIVE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)

REFERENCE_PRIMITIVE_FEATURES = {
    "language/types/reference/get-value-prop-base-primitive.js": frozenset(
        {"Symbol"}
    ),
    "language/types/reference/get-value-prop-base-primitive-realm.js": frozenset(
        {"cross-realm", "Symbol"}
    ),
    "language/types/reference/put-value-prop-base-primitive.js": frozenset(
        {"Symbol", "Proxy"}
    ),
    "language/types/reference/put-value-prop-base-primitive-realm.js": frozenset(
        {"cross-realm", "Symbol", "Proxy"}
    ),
}

if frozenset(REFERENCE_PRIMITIVE_FEATURES) != REFERENCE_PRIMITIVE_FILES:
    raise RuntimeError("primitive Reference admission manifest is out of sync")
