"""Pinned admission and ownership audit for PlainDateTime arithmetic."""

from hashlib import sha256
from pathlib import Path
import subprocess
import tarfile

from test262_temporal_plain_date_time_conversions_admission import (
    js_executable_tokens,
    property_call_indices,
)


# [Decision Log]
# - Purpose and intent: Admit the complete 84-file ISO add/subtract surface
#   while retaining all known non-ISO and downstream blockers and recording
#   the later datetime-math admission without changing ownership.
# - Existing implementation and constraints: add/subtract are common method
#   names, CI uses sparse Test262 checkouts, and Intl files call each method in
#   large loops that aggregate-only accounting could hide.
# - Main alternatives considered: directory prefixes, grep-only ownership, or
#   exact manifests plus a token-aware complete pinned Git-tree audit.
# - Selected approach: freeze four disjoint manifests, metadata, results,
#   method call counts, homonyms, and a complete archive-derived candidate set.
# - Why this approach: it separates real PlainDateTime calls from Duration and
#   ZonedDateTime homonyms and fails closed when corpus ownership changes.
# - Benefits, drawbacks, and impact: exact accounting prevents false support;
#   each live audit streams the pinned Temporal corpus and harness from Git.
PINNED_TEST262_REVISION = "9e61c12835c5e4a3bdba93850427e6742c4f64c4"
METHODS = ("add", "subtract")
DIRECT_PREFIXES = {
    method: f"built-ins/Temporal/PlainDateTime/prototype/{method}/"
    for method in METHODS
}
INTL_PREFIXES = {
    method: f"intl402/Temporal/PlainDateTime/prototype/{method}/"
    for method in METHODS
}


def _read_manifest(name):
    lines = tuple(
        line
        for raw_line in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )
    if tuple(sorted(lines)) != lines or len(set(lines)) != len(lines):
        raise RuntimeError(f"{name} must be sorted and duplicate-free")
    return frozenset(lines)


TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FILES = _read_manifest(
    "test262_temporal_plain_date_time_arithmetic_admission.txt"
)
TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_INTL_FILES = _read_manifest(
    "test262_temporal_plain_date_time_arithmetic_intl_blockers.txt"
)
TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_DOWNSTREAM_ADMISSION_FILES = _read_manifest(
    "test262_temporal_plain_date_time_arithmetic_downstream_admission.txt"
)
TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_INTL_DOWNSTREAM_FILES = _read_manifest(
    "test262_temporal_plain_date_time_arithmetic_intl_downstream_blockers.txt"
)
TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_COMPLETE_FILES = (
    TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FILES
    | TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_INTL_FILES
    | TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_DOWNSTREAM_ADMISSION_FILES
    | TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_INTL_DOWNSTREAM_FILES
)
TEMPORAL_PLAIN_DATE_TIME_ADD_FILES = frozenset(
    path
    for path in TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FILES
    if path.startswith(DIRECT_PREFIXES["add"])
)
TEMPORAL_PLAIN_DATE_TIME_SUBTRACT_FILES = frozenset(
    path
    for path in TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FILES
    if path.startswith(DIRECT_PREFIXES["subtract"])
)

TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_PREIMPLEMENTATION_FALSE_POSITIVES = frozenset(
    f"built-ins/Temporal/PlainDateTime/prototype/{method}/{name}"
    for method in METHODS
    for name in (
        "argument-invalid-property.js",
        "argument-singular-properties.js",
        "options-invalid.js",
        "options-wrong-type.js",
    )
)

_SYMBOL_FILES = frozenset(
    {"argument-not-object.js", "branding.js", "options-invalid.js"}
)
_TEMPORAL_HELPER_FILES = frozenset(
    {
        "add-large-subseconds.js",
        "ambiguous-date.js",
        "argument-duration-max-plus-min-date.js",
        "argument-duration-max.js",
        "argument-duration.js",
        "argument-string-fractional-units-rounding-mode.js",
        "argument-string-negative-fractional-units.js",
        "argument-string.js",
        "balance-negative-time-units.js",
        "basic-arithmetic.js",
        "blank-duration.js",
        "hour-overflow.js",
        "month-boundary.js",
        "negative-duration.js",
        "options-empty.js",
        "overflow-undefined.js",
        "subclassing-ignored.js",
        "subtract-large-subseconds.js",
    }
)
_COMPARE_HELPER_FILES = frozenset(
    {
        "options-read-before-algorithmic-validation.js",
        "order-of-operations.js",
        "overflow-wrong-type.js",
    }
)


def _direct_features(path):
    name = Path(path).name
    features = {"Temporal"}
    if name in _SYMBOL_FILES:
        features.add("Symbol")
    elif name == "options-wrong-type.js":
        features.update(("BigInt", "Symbol"))
    elif name == "not-a-constructor.js":
        features.add("Reflect.construct")
    return frozenset(features)


def _direct_includes(path):
    name = Path(path).name
    if name in _TEMPORAL_HELPER_FILES:
        return frozenset({"temporalHelpers.js"})
    if name in _COMPARE_HELPER_FILES:
        return frozenset({"compareArray.js", "temporalHelpers.js"})
    if name in {"length.js", "name.js", "prop-desc.js"}:
        return frozenset({"propertyHelper.js"})
    if name == "not-a-constructor.js":
        return frozenset({"isConstructor.js"})
    return frozenset()


TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FEATURES = {
    path: _direct_features(path) for path in TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FILES
}
TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_INCLUDES = {
    path: _direct_includes(path) for path in TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FILES
}
TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FILES
}
TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FILES
}

_HOMONYM_OWNERS = {
    "built-ins/Temporal/PlainDateTime/prototype/since/float64-representable-integer.js": "Duration",
    "built-ins/Temporal/PlainDateTime/prototype/until/float64-representable-integer.js": "Duration",
    "built-ins/Temporal/ZonedDateTime/prototype/subtract/overflow-subtracting-months-from-min-year.js": "ZonedDateTime",
    "intl402/Temporal/ZonedDateTime/prototype/add/dst.js": "ZonedDateTime",
    "intl402/Temporal/ZonedDateTime/prototype/subtract/dst.js": "ZonedDateTime",
    "intl402/Temporal/ZonedDateTime/prototype/getTimeZoneTransition/subtract-second-and-nanosecond-from-last-transition.js": "ZonedDateTime",
}

AUDIT_TREE_ROOTS = (
    "harness",
    "test/built-ins/Temporal/PlainDateTime/prototype/add",
    "test/built-ins/Temporal/PlainDateTime/prototype/subtract",
    "test/intl402/Temporal/PlainDateTime/prototype/add",
    "test/intl402/Temporal/PlainDateTime/prototype/subtract",
)
AUDIT_TREE_FILES = tuple(
    "test/" + relative
    for relative in sorted(
        TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_DOWNSTREAM_ADMISSION_FILES
        | TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_INTL_DOWNSTREAM_FILES
    )
)
AUDIT_TREE_PATHS = AUDIT_TREE_ROOTS + AUDIT_TREE_FILES

_EXPECTED_CANDIDATE_COUNT = 249
_EXPECTED_CANDIDATE_TOTALS = (1861, 2487, 2469, 2467, 2449)
_EXPECTED_CANDIDATE_DIGEST = (
    "b4ee8f1fb690e6a972393e1b31016e083b349292c1f514dcb487e0665b26994e"
)
_EXPECTED_OWNERSHIP_ROWS = 239
_EXPECTED_OWNERSHIP_TOTALS = {
    "direct": (166, 165),
    "intl": (2271, 2271),
    "downstream": (7, 2),
    "intl_downstream": (9, 0),
    "homonym": (14, 11),
}
_EXPECTED_OWNERSHIP_DIGEST = (
    "090b63806cbb802becbdbff5593eb728d7958d9a2003ee441eb8da6911317264"
)
_EXPECTED_METADATA_DIGEST = (
    "e12b076139fc5a91b6d6474824d7b80bb5f21609fce298e6cf638fdee1ec127f"
)

if (
    len(TEMPORAL_PLAIN_DATE_TIME_ADD_FILES) != 42
    or len(TEMPORAL_PLAIN_DATE_TIME_SUBTRACT_FILES) != 42
    or len(TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FILES) != 84
    or len(TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_INTL_FILES) != 148
    or len(TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_DOWNSTREAM_ADMISSION_FILES) != 4
    or len(TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_INTL_DOWNSTREAM_FILES) != 7
    or len(TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_COMPLETE_FILES) != 243
    or len(TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_PREIMPLEMENTATION_FALSE_POSITIVES)
    != 8
):
    raise RuntimeError("PlainDateTime arithmetic manifests have invalid cardinality")


def _candidate_stats(sources):
    candidates = {}
    expected = TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_COMPLETE_FILES | set(
        _HOMONYM_OWNERS
    )
    for relative, source in sources.items():
        tokens = js_executable_tokens(source)
        add_calls = len(property_call_indices(tokens, "add"))
        subtract_calls = len(property_call_indices(tokens, "subtract"))
        has_method = add_calls != 0 or subtract_calls != 0
        has_plain_date_time = "PlainDateTime" in tokens or "/PlainDateTime/" in relative
        if relative not in expected and not (has_method and has_plain_date_time):
            continue
        counts = (
            tokens.count("PlainDateTime"),
            tokens.count("add"),
            tokens.count("subtract"),
            add_calls,
            subtract_calls,
        )
        candidates[relative] = (counts, tokens)
    return candidates


def _candidate_digest(candidates):
    serialized = "".join(
        f"{relative}\t" + "\t".join(map(str, candidates[relative][0])) + "\n"
        for relative in sorted(candidates)
    )
    return sha256(serialized.encode()).hexdigest()


def _category(relative):
    if relative in TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FILES:
        return "PlainDateTime", "direct"
    if relative in TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_INTL_FILES:
        return "PlainDateTime", "intl"
    if relative in TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_DOWNSTREAM_ADMISSION_FILES:
        return "PlainDateTime", "downstream"
    if relative in TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_INTL_DOWNSTREAM_FILES:
        return "PlainDateTime", "intl_downstream"
    if relative in _HOMONYM_OWNERS:
        return _HOMONYM_OWNERS[relative], "homonym"
    raise RuntimeError(f"PlainDateTime arithmetic candidate has no owner: {relative}")


def _ownership_rows(candidates):
    rows = []
    for relative in sorted(candidates):
        owner, category = _category(relative)
        _, tokens = candidates[relative]
        for method in METHODS:
            calls = len(property_call_indices(tokens, method))
            if calls:
                rows.append((relative, method, calls, owner, category))
    return tuple(rows)


def verify_candidate_contract(candidates):
    expected = TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_COMPLETE_FILES | set(
        _HOMONYM_OWNERS
    )
    if set(candidates) != expected or len(candidates) != _EXPECTED_CANDIDATE_COUNT:
        raise RuntimeError(
            "PlainDateTime arithmetic candidate surface drifted: "
            f"missing={sorted(expected - set(candidates))} "
            f"outside={sorted(set(candidates) - expected)}"
        )
    totals = tuple(
        sum(counts[index] for counts, _ in candidates.values()) for index in range(5)
    )
    digest = _candidate_digest(candidates)
    if totals != _EXPECTED_CANDIDATE_TOTALS or digest != _EXPECTED_CANDIDATE_DIGEST:
        raise RuntimeError(
            "PlainDateTime arithmetic candidate counts drifted: "
            f"totals={totals} digest={digest}"
        )
    rows = _ownership_rows(candidates)
    serialized = "".join("\t".join(map(str, row)) + "\n" for row in rows)
    ownership_digest = sha256(serialized.encode()).hexdigest()
    ownership_totals = {
        category: tuple(
            sum(row[2] for row in rows if row[4] == category and row[1] == method)
            for method in METHODS
        )
        for category in _EXPECTED_OWNERSHIP_TOTALS
    }
    if (
        len(rows) != _EXPECTED_OWNERSHIP_ROWS
        or ownership_totals != _EXPECTED_OWNERSHIP_TOTALS
        or ownership_digest != _EXPECTED_OWNERSHIP_DIGEST
    ):
        raise RuntimeError(
            "PlainDateTime arithmetic ownership drifted: "
            f"rows={len(rows)} totals={ownership_totals} digest={ownership_digest}"
        )


def _run_git(corpus_root, *arguments):
    try:
        result = subprocess.run(
            ("git", "-C", str(corpus_root), *arguments),
            check=True,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise RuntimeError("configured Test262 corpus verification failed closed") from error
    return result.stdout


def _verify_pinned_tree(corpus_root):
    revision = _run_git(corpus_root, "rev-parse", "--verify", "HEAD^{commit}").strip()
    if revision != PINNED_TEST262_REVISION:
        raise RuntimeError(
            f"Test262 revision drifted: expected {PINNED_TEST262_REVISION}, got {revision}"
        )
    dirty = _run_git(
        corpus_root,
        "status",
        "--porcelain",
        "--untracked-files=all",
        "--",
        *AUDIT_TREE_PATHS,
    )
    if dirty:
        raise RuntimeError(f"pinned Test262 corpus/harness is dirty: {dirty.strip()}")
    tree_files = {
        line
        for line in _run_git(
            corpus_root,
            "ls-tree",
            "-r",
            "--name-only",
            PINNED_TEST262_REVISION,
            "--",
            *AUDIT_TREE_PATHS,
        ).splitlines()
        if line.endswith(".js")
    }
    live_files = set()
    for relative in AUDIT_TREE_ROOTS:
        root = corpus_root / relative
        if not root.is_dir():
            raise FileNotFoundError(root)
        live_files.update(
            path.relative_to(corpus_root).as_posix() for path in root.rglob("*.js")
        )
    for relative in AUDIT_TREE_FILES:
        path = corpus_root / relative
        if not path.is_file():
            raise FileNotFoundError(path)
        live_files.add(relative)
    if live_files != tree_files:
        raise RuntimeError(
            "configured Test262 sparse corpus/harness is incomplete: "
            f"missing={sorted(tree_files - live_files)} "
            f"outside={sorted(live_files - tree_files)}"
        )


def _pinned_sources(corpus_root):
    command = (
        "git",
        "-c",
        "gc.auto=0",
        "-C",
        str(corpus_root),
        "archive",
        "--format=tar",
        PINNED_TEST262_REVISION,
        "--",
        "test",
        "harness",
    )
    try:
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    except OSError as error:
        raise RuntimeError("configured Test262 corpus archive failed closed") from error
    sources = {}
    try:
        if process.stdout is None:
            raise RuntimeError("configured Test262 corpus archive has no output")
        with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
            for member in archive:
                if not member.isfile() or not member.name.endswith(".js"):
                    continue
                archived = archive.extractfile(member)
                if archived is None:
                    raise RuntimeError(f"configured Test262 archive omitted {member.name}")
                source = archived.read().decode("utf-8")
                if (
                    ("add" not in source and "subtract" not in source)
                    or (
                        "PlainDateTime" not in source
                        and "/PlainDateTime/" not in member.name
                    )
                ):
                    continue
                relative = (
                    member.name.removeprefix("test/")
                    if member.name.startswith("test/")
                    else member.name
                )
                sources[relative] = source
        stderr = process.stderr.read().decode("utf-8") if process.stderr else ""
        return_code = process.wait(timeout=30)
    except (
        OSError,
        RuntimeError,
        subprocess.SubprocessError,
        tarfile.TarError,
        UnicodeError,
    ) as error:
        process.kill()
        process.wait()
        raise RuntimeError("configured Test262 corpus archive failed closed") from error
    finally:
        if process.stdout is not None:
            process.stdout.close()
        if process.stderr is not None:
            process.stderr.close()
    if return_code != 0:
        raise RuntimeError(
            "configured Test262 corpus archive failed closed: " + stderr.strip()
        )
    return sources


def _metadata_digest(test_root, parse_meta):
    rows = []
    for relative in sorted(TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_COMPLETE_FILES):
        path = test_root / relative
        if not path.is_file():
            raise FileNotFoundError(path)
        metadata = parse_meta(path.read_text())
        rows.append(
            (
                relative,
                tuple(sorted(metadata.get("features", []))),
                tuple(sorted(metadata.get("includes", []))),
                tuple(sorted(metadata.get("flags", []))),
                metadata.get("negative"),
            )
        )
    serialized = "".join(repr(row) + "\n" for row in rows)
    return sha256(serialized.encode()).hexdigest()


def audit_corpus(corpus_root, parse_meta):
    """Fail closed unless direct, blocker, metadata, and ownership sets are exact."""

    corpus_root = Path(corpus_root)
    if not corpus_root.is_dir():
        raise FileNotFoundError(corpus_root)
    _verify_pinned_tree(corpus_root)
    test_root = corpus_root / "test"
    live_direct = set()
    live_intl = set()
    for method in METHODS:
        direct_dir = test_root / DIRECT_PREFIXES[method]
        intl_dir = test_root / INTL_PREFIXES[method]
        if not direct_dir.is_dir() or not intl_dir.is_dir():
            raise FileNotFoundError(direct_dir if not direct_dir.is_dir() else intl_dir)
        live_direct.update(
            path.relative_to(test_root).as_posix() for path in direct_dir.glob("*.js")
        )
        live_intl.update(
            path.relative_to(test_root).as_posix() for path in intl_dir.glob("*.js")
        )
    if live_direct != TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FILES:
        raise RuntimeError("PlainDateTime arithmetic direct directories drifted")
    if live_intl != TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_INTL_FILES:
        raise RuntimeError("PlainDateTime arithmetic Intl directories drifted")

    for relative in sorted(TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FILES):
        metadata = parse_meta((test_root / relative).read_text())
        actual = (
            frozenset(metadata.get("features", [])),
            frozenset(metadata.get("includes", [])),
            frozenset(metadata.get("flags", [])),
            metadata.get("negative"),
        )
        expected = (
            TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FEATURES[relative],
            TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_INCLUDES[relative],
            TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_FLAGS[relative],
            TEMPORAL_PLAIN_DATE_TIME_ARITHMETIC_NEGATIVE[relative],
        )
        if actual != expected:
            raise RuntimeError(
                f"PlainDateTime arithmetic direct metadata drifted: {relative}: {actual}"
            )
    metadata_digest = _metadata_digest(test_root, parse_meta)
    if metadata_digest != _EXPECTED_METADATA_DIGEST:
        raise RuntimeError(
            f"PlainDateTime arithmetic complete metadata drifted: {metadata_digest}"
        )
    candidates = _candidate_stats(_pinned_sources(corpus_root))
    verify_candidate_contract(candidates)
    return candidates
