"""Frozen Test262 class files admitted for default parameters."""

from pathlib import Path, PurePosixPath


_MANIFEST = Path(__file__).with_name("test262_class_default_parameter_admission.txt")
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
                f"invalid class default-parameter path at line {line_number}: {path}"
            )
        if path in entries:
            raise ValueError(
                f"duplicate class default-parameter path at line {line_number}: {path}"
            )
        entries.add(path)
    return frozenset(entries)


CLASS_DEFAULT_PARAMETER_FILES = _read_manifest()
