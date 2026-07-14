"""Frozen Test262 class files admitted for destructuring bindings."""

from pathlib import Path, PurePosixPath


_MANIFEST = Path(__file__).with_name("test262_class_destructuring_admission.txt")
_CLASS_PREFIXES = (
    "language/expressions/class/",
    "language/statements/class/",
)


def _read_manifest():
    entries = set()
    for line_number, raw_line in enumerate(_MANIFEST.read_text().splitlines(), start=1):
        path = raw_line.strip()
        if not path or path.startswith("#"):
            continue
        if (
            not path.startswith(_CLASS_PREFIXES)
            or not path.endswith(".js")
            or PurePosixPath(path).is_absolute()
            or ".." in PurePosixPath(path).parts
        ):
            raise ValueError(
                f"invalid class destructuring path at line {line_number}: {path}"
            )
        if path in entries:
            raise ValueError(
                f"duplicate class destructuring path at line {line_number}: {path}"
            )
        entries.add(path)
    return frozenset(entries)


CLASS_DESTRUCTURING_FILES = _read_manifest()
