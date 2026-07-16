"""Frozen Test262 AsyncIterator async-disposal admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_async_iterator_dispose_admission.txt")
ASYNC_ITERATOR_DISPOSE_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
ASYNC_ITERATOR_DISPOSE_FEATURES = {
    relative: frozenset({"explicit-resource-management"})
    for relative in ASYNC_ITERATOR_DISPOSE_FILES
}
