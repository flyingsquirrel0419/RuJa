"""Exact Test262 coverage for the shared Instant string parser expansion."""

from pathlib import Path


_MANIFEST = Path(__file__).with_name(
    "test262_temporal_instant_string_parser_admission.txt"
)
TEMPORAL_INSTANT_STRING_PARSER_FILES = frozenset(
    line
    for raw_line in _MANIFEST.read_text().splitlines()
    if (line := raw_line.strip()) and not line.startswith("#")
)
TEMPORAL_INSTANT_STRING_PARSER_FEATURES = {
    path: frozenset(
        {"Temporal"}
        | (
            {"arrow-function"}
            if path.endswith(("/argument-string-invalid.js", "/year-zero.js"))
            else set()
        )
    )
    for path in TEMPORAL_INSTANT_STRING_PARSER_FILES
}

if len(TEMPORAL_INSTANT_STRING_PARSER_FILES) != 36:
    raise RuntimeError("Temporal.Instant string parser admission must contain 36 files")
