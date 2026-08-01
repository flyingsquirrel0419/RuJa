"""Frozen legacy RegExp constructor-accessor Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_regexp_legacy_accessors_admission.txt"
)
REGEXP_LEGACY_ACCESSOR_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)


def _features(path):
    features = {"legacy-regexp"}
    if path.endswith("/this-subclass-constructor.js"):
        features.add("class")
    if path.endswith("/this-cross-realm-constructor.js"):
        features.update({"cross-realm", "Reflect"})
        if "/input/" in path:
            features.add("Reflect.set")
    return frozenset(features)


REGEXP_LEGACY_ACCESSOR_FEATURES = {
    path: _features(path) for path in REGEXP_LEGACY_ACCESSOR_FILES
}

if len(REGEXP_LEGACY_ACCESSOR_FILES) != 24:
    raise RuntimeError("legacy RegExp accessor admission must contain 24 files")
if frozenset(REGEXP_LEGACY_ACCESSOR_FEATURES) != REGEXP_LEGACY_ACCESSOR_FILES:
    raise RuntimeError("legacy RegExp accessor admission manifest is out of sync")
if any(
    not path.startswith("annexB/built-ins/RegExp/legacy-accessors/")
    for path in REGEXP_LEGACY_ACCESSOR_FILES
):
    raise RuntimeError("legacy RegExp accessor admission contains an unrelated path")
