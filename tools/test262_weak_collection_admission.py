"""Frozen feature-gated Test262 WeakMap and WeakSet files."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_weak_collection_admission.data")
WEAK_COLLECTION_FEATURES = {}
for raw_line in _MANIFEST.read_text().splitlines():
    line = raw_line.strip()
    if not line or line.startswith("#"):
        continue
    path, separator, raw_features = line.partition("|")
    if not separator or not path or not raw_features:
        raise RuntimeError(f"invalid weak collection admission line: {raw_line!r}")
    features = frozenset(feature for feature in raw_features.split(",") if feature)
    if path in WEAK_COLLECTION_FEATURES or not features:
        raise RuntimeError(f"invalid weak collection admission entry: {path!r}")
    WEAK_COLLECTION_FEATURES[path] = features

WEAK_COLLECTION_FILES = frozenset(WEAK_COLLECTION_FEATURES)
