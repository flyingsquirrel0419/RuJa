"""Frozen static import-attribute and typed-module Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_static_import_attributes_admission.txt")
STATIC_IMPORT_ATTRIBUTES_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def static_import_attributes_features(relative_path):
    """Return only the broad gates audited for one frozen file."""
    if relative_path.startswith("language/module-code/import-attributes/"):
        return frozenset({"import-attributes"})
    name = relative_path.rsplit("/", 1)[-1]
    if name.startswith("json-"):
        features = {"import-attributes", "json-modules"}
        if name == "json-idempotency.js":
            features.add("dynamic-import")
        return frozenset(features)
    if name.startswith("text-"):
        return frozenset({"import-attributes", "import-text"})
    return frozenset()
