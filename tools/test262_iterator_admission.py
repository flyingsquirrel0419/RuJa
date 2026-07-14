"""Frozen Test262 global Iterator and Iterator prototype core boundary."""

from pathlib import Path, PurePosixPath


_MANIFEST = Path(__file__).with_name("test262_iterator_admission.txt")
_PREFIX = "built-ins/Iterator/"

ITERATOR_CORE_FEATURES = frozenset(
    {
        "Reflect",
        "Symbol",
        "Symbol.iterator",
        "explicit-resource-management",
        "iterator-helpers",
    }
)


def _read_manifest():
    entries = set()
    for line_number, raw_line in enumerate(_MANIFEST.read_text().splitlines(), start=1):
        path = raw_line.strip()
        if not path or path.startswith("#"):
            continue
        pure = PurePosixPath(path)
        if (
            not path.startswith(_PREFIX)
            or not path.endswith(".js")
            or pure.is_absolute()
            or ".." in pure.parts
        ):
            raise ValueError(
                f"invalid Iterator admission path at line {line_number}: {path}"
            )
        if path in entries:
            raise ValueError(
                f"duplicate Iterator admission path at line {line_number}: {path}"
            )
        entries.add(path)
    return frozenset(entries)


ITERATOR_CORE_FILES = _read_manifest()
