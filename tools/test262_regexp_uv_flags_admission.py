"""Frozen Test262 RegExp u/v mutual-exclusion admission."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name("test262_regexp_uv_flags_admission.txt")
REGEXP_UV_FLAGS_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
_EXPECTED_FILES = frozenset(
    {
        "built-ins/RegExp/prototype/unicodeSets/uv-flags-constructor.js",
        "built-ins/RegExp/prototype/unicodeSets/uv-flags.js",
    }
)
REGEXP_UV_FLAGS_FEATURES = {
    relative: frozenset({"regexp-v-flag"}) for relative in REGEXP_UV_FLAGS_FILES
}

if REGEXP_UV_FLAGS_FILES != _EXPECTED_FILES:
    raise RuntimeError("RegExp u/v flag admission contains an unexpected path")
if frozenset(REGEXP_UV_FLAGS_FEATURES) != REGEXP_UV_FLAGS_FILES:
    raise RuntimeError("RegExp u/v flag admission manifest is out of sync")
