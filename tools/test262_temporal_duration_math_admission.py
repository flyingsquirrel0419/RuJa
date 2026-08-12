"""Pinned exact admission for Duration.prototype.add and Duration.compare."""

from hashlib import sha256
from pathlib import Path
import subprocess


# [Decision Log]
# - Purpose and intent: admit the complete ISO/fixed-offset add/compare surface
#   while retaining named-zone, Intl, and later-method dependencies as exact blockers.
# - Existing implementation and constraints: four files passed before either
#   method existed, and two PlainDateTime files overlap an older ownership unit.
# - Main alternatives considered: broad Temporal admission, directory counts,
#   or exact manifests plus metadata/result/error contracts.
# - Selected approach: freeze five sorted manifests, all live metadata, runner
#   and analyzer parity, transition identity, and the complete 94-file result.
# - Why this approach: it rejects false positives and corpus drift without
#   claiming named-IANA/DST or later difference methods as supported.
# - Benefits, drawbacks, and impact: 84 direct and two released downstream
#   files are admitted; three Intl and five downstream blockers remain visible.
PINNED_TEST262_REVISION = "9e61c12835c5e4a3bdba93850427e6742c4f64c4"
_EXPECTED_METADATA_DIGEST = (
    "28cb57f073ed3474572d5befa50acf7f2f1e015783610c2db5bd7d075fd7b643"
)


def _read(name):
    lines = tuple(
        line
        for raw in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw.strip()) and not line.startswith("#")
    )
    if tuple(sorted(lines)) != lines or len(lines) != len(set(lines)):
        raise RuntimeError(f"{name} must be sorted and duplicate-free")
    return frozenset(lines)


TEMPORAL_DURATION_MATH_FILES = _read("test262_temporal_duration_math_admission.txt")
TEMPORAL_DURATION_MATH_INTL_BLOCKERS = _read(
    "test262_temporal_duration_math_intl_blockers.txt"
)
TEMPORAL_DURATION_MATH_DOWNSTREAM_ADMISSION = _read(
    "test262_temporal_duration_math_downstream_admission.txt"
)
TEMPORAL_DURATION_MATH_DOWNSTREAM_BLOCKERS = _read(
    "test262_temporal_duration_math_downstream_blockers.txt"
)
TEMPORAL_DURATION_MATH_FALSE_POSITIVES = _read(
    "test262_temporal_duration_math_false_positives.txt"
)
TEMPORAL_DURATION_MATH_ADMITTED = (
    TEMPORAL_DURATION_MATH_FILES | TEMPORAL_DURATION_MATH_DOWNSTREAM_ADMISSION
)
TEMPORAL_DURATION_MATH_COMPLETE = (
    TEMPORAL_DURATION_MATH_ADMITTED
    | TEMPORAL_DURATION_MATH_INTL_BLOCKERS
    | TEMPORAL_DURATION_MATH_DOWNSTREAM_BLOCKERS
)

_FEATURE_OVERRIDES = {
    "built-ins/Temporal/Duration/compare/not-a-constructor.js": {"Reflect.construct", "Temporal"},
    "built-ins/Temporal/Duration/compare/options-wrong-type.js": {"BigInt", "Symbol", "Temporal"},
    "built-ins/Temporal/Duration/compare/relativeto-propertybag-timezone-string-year-zero.js": {"Temporal", "arrow-function"},
    "built-ins/Temporal/Duration/compare/relativeto-propertybag-timezone-wrong-type.js": {"BigInt", "Symbol", "Temporal"},
    "built-ins/Temporal/Duration/prototype/add/argument-not-object.js": {"Symbol", "Temporal"},
    "built-ins/Temporal/Duration/prototype/add/branding.js": {"Symbol", "Temporal"},
    "built-ins/Temporal/Duration/prototype/add/not-a-constructor.js": {"Reflect.construct", "Temporal"},
}
TEMPORAL_DURATION_MATH_FEATURES = {
    path: frozenset(_FEATURE_OVERRIDES.get(path, {"Temporal"}))
    for path in TEMPORAL_DURATION_MATH_ADMITTED
}


def audit_metadata(test262_root, parse_meta):
    corpus_root = Path(test262_root)
    try:
        revision = subprocess.run(
            ("git", "-C", str(corpus_root), "rev-parse", "--verify", "HEAD^{commit}"),
            check=True,
            capture_output=True,
            text=True,
            timeout=30,
        ).stdout.strip()
        if revision != PINNED_TEST262_REVISION:
            raise RuntimeError(
                f"Test262 revision drifted: expected {PINNED_TEST262_REVISION}, got {revision}"
            )
        tracked = tuple("test/" + path for path in sorted(TEMPORAL_DURATION_MATH_COMPLETE))
        dirty = subprocess.run(
            (
                "git", "-C", str(corpus_root), "status", "--porcelain",
                "--untracked-files=all", "--", *tracked,
            ),
            check=True,
            capture_output=True,
            text=True,
            timeout=30,
        ).stdout
    except (OSError, subprocess.SubprocessError) as error:
        raise RuntimeError("configured Test262 corpus verification failed closed") from error
    if dirty:
        raise RuntimeError(f"pinned Test262 Duration math corpus is dirty: {dirty.strip()}")
    test_root = corpus_root / "test"
    rows = []
    for relative in sorted(TEMPORAL_DURATION_MATH_COMPLETE):
        source = (test_root / relative).read_text()
        metadata = parse_meta(source)
        rows.append(
            (
                relative,
                tuple(sorted(metadata.get("features", []))),
                tuple(sorted(metadata.get("includes", []))),
                tuple(sorted(metadata.get("flags", []))),
                metadata.get("negative"),
            )
        )
    digest = sha256("".join(repr(row) + "\n" for row in rows).encode()).hexdigest()
    if digest != _EXPECTED_METADATA_DIGEST:
        raise RuntimeError(f"Temporal Duration math metadata drifted: {digest}")
    return tuple(rows)

if (
    len(TEMPORAL_DURATION_MATH_FILES) != 84
    or len(TEMPORAL_DURATION_MATH_INTL_BLOCKERS) != 3
    or len(TEMPORAL_DURATION_MATH_DOWNSTREAM_ADMISSION) != 2
    or len(TEMPORAL_DURATION_MATH_DOWNSTREAM_BLOCKERS) != 5
    or len(TEMPORAL_DURATION_MATH_FALSE_POSITIVES) != 4
    or len(TEMPORAL_DURATION_MATH_COMPLETE) != 94
):
    raise RuntimeError("Temporal Duration math manifests have invalid cardinality")
