"""Frozen Test262 AsyncFromSyncIteratorPrototype admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_async_from_sync_iterator_admission.txt"
)
ASYNC_FROM_SYNC_ITERATOR_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
ASYNC_FROM_SYNC_ITERATOR_FEATURES = {
    relative: frozenset({"async-iteration"})
    for relative in ASYNC_FROM_SYNC_ITERATOR_FILES
}
