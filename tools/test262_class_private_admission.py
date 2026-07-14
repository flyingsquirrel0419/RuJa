"""Frozen Test262 private class boundary files and their feature gates."""

from pathlib import Path, PurePosixPath


PRIVATE_CLASS_FEATURES = frozenset(
    {
        "class-fields-private",
        "class-fields-private-in",
        "class-methods-private",
        "class-static-fields-private",
        "class-static-methods-private",
    }
)
_MANIFEST = Path(__file__).with_name("test262_class_private_admission.txt")
_CLASS_PREFIXES = (
    "language/expressions/class/",
    "language/statements/class/",
)


def _read_manifest():
    entries = {}
    for line_number, raw_line in enumerate(_MANIFEST.read_text().splitlines(), start=1):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        path, separator, raw_features = line.partition(" | ")
        features = frozenset(
            feature.strip() for feature in raw_features.split(",") if feature.strip()
        )
        if not separator or not path or not features:
            raise ValueError(f"invalid private class admission at line {line_number}")
        if (
            not path.startswith(_CLASS_PREFIXES)
            or not path.endswith(".js")
            or PurePosixPath(path).is_absolute()
            or ".." in PurePosixPath(path).parts
        ):
            raise ValueError(f"invalid private class path at line {line_number}: {path}")
        if not features <= PRIVATE_CLASS_FEATURES:
            raise ValueError(
                f"invalid private class feature at line {line_number}: {raw_features}"
            )
        if path in entries:
            raise ValueError(f"duplicate private class path at line {line_number}: {path}")
        entries[path] = features
    return entries


CLASS_PRIVATE_FEATURES_BY_FILE = _read_manifest()
CLASS_PRIVATE_FILES = frozenset(CLASS_PRIVATE_FEATURES_BY_FILE)
