"""Frozen WeakRef and FinalizationRegistry Test262 admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_weak_reference_admission.data")
WEAK_REFERENCE_FEATURES = {}
for raw_line in _MANIFEST.read_text().splitlines():
    line = raw_line.strip()
    if not line or line.startswith("#"):
        continue
    relative, separator, raw_features = line.partition("|")
    if separator != "|" or relative in WEAK_REFERENCE_FEATURES:
        raise ValueError(f"invalid WeakRef admission row: {raw_line}")
    WEAK_REFERENCE_FEATURES[relative] = frozenset(
        feature for feature in raw_features.split(",") if feature
    )

WEAK_REFERENCE_FILES = frozenset(WEAK_REFERENCE_FEATURES)


def weak_reference_features(relative_path):
    """Return the complete audited feature set for one admitted file."""
    return WEAK_REFERENCE_FEATURES.get(relative_path, frozenset())
