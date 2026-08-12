"""Pinned admission and ownership audit for PlainDateTime until/since."""

from hashlib import sha256
from pathlib import Path
import subprocess
import tarfile

from test262_temporal_plain_date_time_conversions_admission import (
    js_executable_tokens,
    property_call_indices,
)


# [Decision Log]
# - Purpose and intent: admit all 193 supported ISO direct files
#   and one true downstream caller while retaining every known dependency and
#   non-ISO blocker.
# - Existing implementation and constraints: `until` and `since` are common
#   method names, eight files passed before either method existed, and sparse
#   CI checkouts cannot establish full-tree ownership by themselves.
# - Main alternatives considered: directory admission, grep totals, or exact
#   manifests plus a token-aware audit of the pinned Git archive.
# - Selected approach: freeze paths, metadata, direct/computed references and
#   calls, candidate ownership, results, blocker errors, and line locations.
# - Why this approach: it rejects absent-method false positives, homonyms,
#   hidden downstream calls, and future corpus drift instead of inflating the
#   supported-subset result.
# - Benefits, drawbacks, and impact: all 322 owner files and 328 method rows
#   fail closed; each live audit streams the full pinned corpus once.
PINNED_TEST262_REVISION = "9e61c12835c5e4a3bdba93850427e6742c4f64c4"
METHODS = ("until", "since")
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
    if lines != tuple(sorted(lines)) or len(lines) != len(set(lines)):
        raise RuntimeError(f"{name} must be sorted and duplicate-free")
    return frozenset(lines)


TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_FILES = _read_manifest(
    "test262_temporal_plain_date_time_difference_direct.txt"
)
TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_TRANSITIONS = _read_manifest(
    "test262_temporal_plain_date_time_difference_direct_transitions.txt"
)
TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DOWNSTREAM_FILES = _read_manifest(
    "test262_temporal_plain_date_time_difference_downstream.txt"
)
TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_INTL_BLOCKERS = _read_manifest(
    "test262_temporal_plain_date_time_difference_intl_blockers.txt"
)
TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_HOMONYMS = _read_manifest(
    "test262_temporal_plain_date_time_difference_homonyms.txt"
)
TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_FILES = (
    TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_FILES
    | TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_TRANSITIONS
    | TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DOWNSTREAM_FILES
)
TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_COMPLETE_FILES = (
    TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_FILES
    | TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_INTL_BLOCKERS
)
TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_PREIMPLEMENTATION_FALSE_POSITIVES = frozenset(
    f"built-ins/Temporal/PlainDateTime/prototype/{method}/{name}"
    for method in METHODS
    for name in (
        "argument-number.js",
        "argument-propertybag-calendar-wrong-type.js",
        "options-invalid.js",
        "options-wrong-type.js",
    )
)


def _read_metadata():
    rows = []
    metadata = {}
    path = Path(__file__).with_name(
        "test262_temporal_plain_date_time_difference_metadata.txt"
    )
    for raw_line in path.read_text().splitlines():
        if not (line := raw_line.strip()) or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 5:
            raise RuntimeError(f"invalid PlainDateTime difference metadata row: {line}")
        relative, features, includes, flags, negative = fields
        if negative != "-":
            raise RuntimeError(
                f"unexpected PlainDateTime difference negative metadata: {relative}"
            )

        def values(field):
            return frozenset() if field == "-" else frozenset(field.split(","))

        rows.append(relative)
        metadata[relative] = (
            values(features),
            values(includes),
            values(flags),
            None,
        )
    if tuple(rows) != tuple(sorted(rows)) or len(rows) != len(set(rows)):
        raise RuntimeError(
            "PlainDateTime difference metadata must be sorted and duplicate-free"
        )
    return metadata


TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_METADATA = _read_metadata()
TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_FEATURES = {
    path: TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_METADATA[path][0]
    for path in TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_FILES
}
TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_INCLUDES = {
    path: TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_METADATA[path][1]
    for path in TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_FILES
}
TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_FLAGS = {
    path: TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_METADATA[path][2]
    for path in TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_FILES
}
TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_NEGATIVE = {
    path: TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_METADATA[path][3]
    for path in TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_FILES
}

_HOMONYM_OWNERS = {
    path: next(
        owner
        for owner in ("PlainDate", "PlainTime", "ZonedDateTime")
        if f"/Temporal/{owner}/" in path
    )
    for path in TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_HOMONYMS
}
AUDIT_TREE_ROOTS = (
    "harness",
    *("test/" + DIRECT_PREFIXES[method].rstrip("/") for method in METHODS),
    *("test/" + INTL_PREFIXES[method].rstrip("/") for method in METHODS),
)
AUDIT_TREE_FILES = tuple(
    "test/" + path
    for path in sorted(TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DOWNSTREAM_FILES)
)
AUDIT_TREE_PATHS = AUDIT_TREE_ROOTS + AUDIT_TREE_FILES

_EXPECTED_CANDIDATE_COUNT = 322
_EXPECTED_CANDIDATE_TOTALS = (3351, 631, 618, 0, 609, 0, 586, 573, 0, 564, 0)
_EXPECTED_CANDIDATE_DIGEST = (
    "97b13fea06dc78ae2e91bfbbefdec41258909f9664565c18b21d156a8061525f"
)
_EXPECTED_OWNERSHIP_ROWS = 328
_EXPECTED_OWNERSHIP_TOTALS = {
    "direct": (260, 0, 251, 0, 215, 0, 206, 0),
    "direct_transition": (1, 0, 1, 0, 1, 0, 1, 0),
    "intl": (328, 0, 328, 0, 325, 0, 325, 0),
    "downstream": (1, 0, 1, 0, 2, 0, 2, 0),
    "homonym": (28, 0, 28, 0, 30, 0, 30, 0),
}
_EXPECTED_OWNERSHIP_DIGEST = (
    "b9086a888da6effcdb92a18f563a9cdcb657e568a574e646023c53592e17672b"
)
_EXPECTED_METADATA_DIGEST = (
    "3f826152e2e7ac81d3ec575a8dc399fa2b5e81c9968f331e5f252f5a925dd467"
)
_EXPECTED_COMPUTED_AUDIT_FILES = 1341
_EXPECTED_COMPUTED_AUDIT_TOTALS = (6848, 9472, 9472, 11)
_EXPECTED_COMPUTED_AUDIT_DIGEST = (
    "dfebda4bcd36cafa174639d58f9ee968cd055d244562f067697773214556331a"
)

if (
    len(TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_FILES) != 191
    or len(TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_TRANSITIONS) != 2
    or len(TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DOWNSTREAM_FILES) != 1
    or len(TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_INTL_BLOCKERS) != 117
    or len(TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_HOMONYMS) != 11
    or len(TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_FILES) != 194
    or len(TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_COMPLETE_FILES) != 311
    or len(TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_METADATA) != 311
    or set(TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_METADATA)
    != TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_COMPLETE_FILES
    or len(TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_PREIMPLEMENTATION_FALSE_POSITIVES)
    != 8
    or not TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_PREIMPLEMENTATION_FALSE_POSITIVES
    <= TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_FILES
):
    raise RuntimeError("PlainDateTime difference manifests have invalid cardinality")


def _member_counts(tokens, method):
    calls = set(property_call_indices(tokens, method))
    direct = tuple(
        index
        for index, token in enumerate(tokens)
        if token in (".", "?.")
        and index + 1 < len(tokens)
        and tokens[index + 1] == method
    )
    computed = tuple(
        index
        for index, token in enumerate(tokens)
        if token == "["
        and index + 2 < len(tokens)
        and tokens[index + 1] == ("string", method)
        and tokens[index + 2] == "]"
    )
    return (
        len(direct),
        len(computed),
        sum(index in calls for index in direct),
        sum(index in calls for index in computed),
    )


def _candidate_stats(sources):
    expected = (
        TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_COMPLETE_FILES
        | TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_HOMONYMS
    )
    candidates = {}
    for relative, source in sources.items():
        tokens = js_executable_tokens(source)
        counts = (tokens.count("PlainDateTime"),)
        for method in METHODS:
            counts += (
                sum(
                    token == method or token == ("string", method)
                    for token in tokens
                ),
                *_member_counts(tokens, method),
            )
        references = counts[2] + counts[3] + counts[7] + counts[8]
        has_plain_date_time = (
            "PlainDateTime" in tokens or "/PlainDateTime/" in relative
        )
        if relative not in expected and not (references and has_plain_date_time):
            continue
        candidates[relative] = (counts, tokens)
    return candidates


def _candidate_digest(candidates):
    serialized = "".join(
        f"{path}\t" + "\t".join(map(str, candidates[path][0])) + "\n"
        for path in sorted(candidates)
    )
    return sha256(serialized.encode()).hexdigest()


def _category(relative):
    if relative in TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_FILES:
        return "PlainDateTime", "direct"
    if relative in TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_TRANSITIONS:
        return "PlainDateTime", "direct_transition"
    if relative in TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_INTL_BLOCKERS:
        return "PlainDateTime", "intl"
    if relative in TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DOWNSTREAM_FILES:
        return "PlainDateTime", "downstream"
    if relative in _HOMONYM_OWNERS:
        return _HOMONYM_OWNERS[relative], "homonym"
    raise RuntimeError(f"PlainDateTime difference candidate has no owner: {relative}")


def _ownership_rows(candidates):
    rows = []
    for relative in sorted(candidates):
        owner, category = _category(relative)
        _, tokens = candidates[relative]
        for method in METHODS:
            counts = _member_counts(tokens, method)
            if counts[0] or counts[1]:
                rows.append((relative, method, *counts, owner, category))
    return tuple(rows)


def verify_candidate_contract(candidates):
    expected = (
        TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_COMPLETE_FILES
        | TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_HOMONYMS
    )
    if set(candidates) != expected or len(candidates) != _EXPECTED_CANDIDATE_COUNT:
        raise RuntimeError(
            "PlainDateTime difference candidate surface drifted: "
            f"missing={sorted(expected - set(candidates))} "
            f"outside={sorted(set(candidates) - expected)}"
        )
    totals = tuple(
        sum(counts[index] for counts, _ in candidates.values())
        for index in range(11)
    )
    digest = _candidate_digest(candidates)
    if totals != _EXPECTED_CANDIDATE_TOTALS or digest != _EXPECTED_CANDIDATE_DIGEST:
        raise RuntimeError(
            "PlainDateTime difference candidate counts drifted: "
            f"totals={totals} digest={digest}"
        )
    rows = _ownership_rows(candidates)
    ownership_totals = {
        category: tuple(
            sum(
                row[2 + index]
                for row in rows
                if row[7] == category and row[1] == method
            )
            for method in METHODS
            for index in range(4)
        )
        for category in _EXPECTED_OWNERSHIP_TOTALS
    }
    serialized = "".join("\t".join(map(str, row)) + "\n" for row in rows)
    ownership_digest = sha256(serialized.encode()).hexdigest()
    if (
        len(rows) != _EXPECTED_OWNERSHIP_ROWS
        or ownership_totals != _EXPECTED_OWNERSHIP_TOTALS
        or ownership_digest != _EXPECTED_OWNERSHIP_DIGEST
    ):
        raise RuntimeError(
            "PlainDateTime difference ownership drifted: "
            f"rows={len(rows)} totals={ownership_totals} "
            f"digest={ownership_digest}"
        )


def verify_computed_ambiguity_contract(sources):
    rows = {}
    for relative, source in sources.items():
        tokens = js_executable_tokens(source)
        if "PlainDateTime" not in tokens:
            continue
        rows[relative] = (
            tokens.count("PlainDateTime"),
            tokens.count("["),
            tokens.count("]"),
            sum(
                1
                for index, token in enumerate(tokens)
                if token == "]"
                and index + 1 < len(tokens)
                and (
                    tokens[index + 1] == "("
                    or (
                        tokens[index + 1] == "?."
                        and index + 2 < len(tokens)
                        and tokens[index + 2] == "("
                    )
                )
            ),
            sha256(repr(tokens).encode()).hexdigest(),
        )
    totals = tuple(sum(counts[index] for counts in rows.values()) for index in range(4))
    serialized = "".join(
        f"{relative}\t" + "\t".join(map(str, rows[relative])) + "\n"
        for relative in sorted(rows)
    )
    digest = sha256(serialized.encode()).hexdigest()
    if (
        len(rows) != _EXPECTED_COMPUTED_AUDIT_FILES
        or totals != _EXPECTED_COMPUTED_AUDIT_TOTALS
        or digest != _EXPECTED_COMPUTED_AUDIT_DIGEST
    ):
        raise RuntimeError(
            "PlainDateTime difference computed-reference ambiguity drifted: "
            f"files={len(rows)} totals={totals} digest={digest}"
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
                    "until" not in source
                    and "since" not in source
                    and "PlainDateTime" not in source
                    and "/PlainDateTime/" not in member.name
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
    for relative in sorted(TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_COMPLETE_FILES):
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
    return sha256("".join(repr(row) + "\n" for row in rows).encode()).hexdigest()


def audit_corpus(corpus_root, parse_meta):
    """Fail closed unless paths, metadata, and ownership match the pin."""

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
    if live_direct != (
        TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_FILES
        | TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_DIRECT_TRANSITIONS
    ):
        raise RuntimeError("PlainDateTime difference direct directories drifted")
    if live_intl != TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_INTL_BLOCKERS:
        raise RuntimeError("PlainDateTime difference Intl directories drifted")
    for relative, expected in TEMPORAL_PLAIN_DATE_TIME_DIFFERENCE_METADATA.items():
        path = test_root / relative
        if not path.is_file():
            raise FileNotFoundError(path)
        metadata = parse_meta(path.read_text())
        actual = (
            frozenset(metadata.get("features", [])),
            frozenset(metadata.get("includes", [])),
            frozenset(metadata.get("flags", [])),
            metadata.get("negative"),
        )
        if actual != expected:
            raise RuntimeError(
                f"PlainDateTime difference metadata fields drifted for {relative}: "
                f"actual={actual} expected={expected}"
            )
    metadata_digest = _metadata_digest(test_root, parse_meta)
    if metadata_digest != _EXPECTED_METADATA_DIGEST:
        raise RuntimeError(
            f"PlainDateTime difference metadata drifted: {metadata_digest}"
        )
    sources = _pinned_sources(corpus_root)
    verify_computed_ambiguity_contract(sources)
    candidates = _candidate_stats(sources)
    verify_candidate_contract(candidates)
    return candidates
