"""Pinned direct admission for Temporal.PlainDateTime.prototype.round."""

from hashlib import sha256
from pathlib import Path
import subprocess
import tarfile

from test262_temporal_plain_date_time_conversions_admission import (
    js_executable_tokens,
    property_call_indices,
)


# [Decision Log]
# - Purpose and intent: Admit only the complete pinned direct round surface.
# - Existing implementation and constraints: broad Temporal feature skipping
#   would otherwise hide all 45 files, including absent-method false positives.
# - Main alternatives considered: admit the directory prefix or freeze an exact
#   path and metadata inventory.
# - Selected approach: load a sorted duplicate-free manifest and derive each
#   file's exact feature set from the pinned metadata inventory.
# - Why this approach: future files remain skipped until their ownership and
#   behavior are audited instead of silently becoming supported.
# - Benefits, drawbacks, and impact: direct conformance and full-tree alias,
#   computed-call, harness, and homonym ownership all fail closed.
PINNED_TEST262_REVISION = "9e61c12835c5e4a3bdba93850427e6742c4f64c4"


def _read_manifest():
    lines = tuple(
        line
        for raw in Path(__file__)
        .with_name("test262_temporal_plain_date_time_round_admission.txt")
        .read_text()
        .splitlines()
        if (line := raw.strip()) and not line.startswith("#")
    )
    if lines != tuple(sorted(lines)) or len(lines) != len(set(lines)):
        raise RuntimeError("PlainDateTime round manifest must be sorted and unique")
    return frozenset(lines)


TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES = _read_manifest()
if len(TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES) != 45:
    raise RuntimeError("PlainDateTime round manifest must contain exactly 45 files")

_SYMBOL_FILES = frozenset({"branding.js"})
_BIGINT_SYMBOL_FILES = frozenset({"options-wrong-type.js"})
_REFLECT_FILES = frozenset({"not-a-constructor.js"})
_ARROW_FILES = frozenset(
    {"smallestunit-plurals-accepted.js", "smallestunit-string-shorthand.js"}
)
_TEMPORAL_HELPER_FILES = frozenset(
    {
        "balance.js",
        "negative-time.js",
        "rounding-direction.js",
        "roundingincrement-non-integer.js",
        "roundingincrement-one-day.js",
        "roundingincrement-undefined.js",
        "roundingmode-basic.js",
        "roundingmode-ceil.js",
        "roundingmode-expand.js",
        "roundingmode-floor.js",
        "roundingmode-halfCeil.js",
        "roundingmode-halfEven.js",
        "roundingmode-halfExpand.js",
        "roundingmode-halfFloor.js",
        "roundingmode-halfTrunc.js",
        "roundingmode-halfexpand-is-default.js",
        "roundingmode-trunc.js",
        "roundingmode-undefined.js",
        "smallestunit-plurals-accepted.js",
        "smallestunit-string-shorthand.js",
        "subclassing-ignored.js",
    }
)
_COMPARE_HELPER_FILES = frozenset(
    {
        "options-read-before-algorithmic-validation.js",
        "roundingincrement-wrong-type.js",
        "roundingmode-wrong-type.js",
        "smallestunit-wrong-type.js",
    }
)


def _features(path):
    name = Path(path).name
    features = {"Temporal"}
    if name in _SYMBOL_FILES:
        features.add("Symbol")
    if name in _BIGINT_SYMBOL_FILES:
        features.update(("BigInt", "Symbol"))
    if name in _REFLECT_FILES:
        features.add("Reflect.construct")
    if name in _ARROW_FILES:
        features.add("arrow-function")
    return frozenset(features)


TEMPORAL_PLAIN_DATE_TIME_ROUND_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES
}


def _includes(path):
    name = Path(path).name
    if name in _COMPARE_HELPER_FILES:
        return frozenset({"compareArray.js", "temporalHelpers.js"})
    if name in _TEMPORAL_HELPER_FILES:
        return frozenset({"temporalHelpers.js"})
    if name in {"length.js", "name.js", "prop-desc.js"}:
        return frozenset({"propertyHelper.js"})
    if name == "not-a-constructor.js":
        return frozenset({"isConstructor.js"})
    return frozenset()


TEMPORAL_PLAIN_DATE_TIME_ROUND_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES
}
TEMPORAL_PLAIN_DATE_TIME_ROUND_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES
}
TEMPORAL_PLAIN_DATE_TIME_ROUND_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES
}

_HOMONYM_OWNERS = {
    "built-ins/Temporal/Duration/prototype/round/bubble-time-unit.js": "Duration",
    "built-ins/Temporal/Duration/prototype/round/relativeto-date-limits.js": "Duration",
    "built-ins/Temporal/Duration/prototype/round/zero-duration.js": "Duration",
    "intl402/Temporal/Duration/prototype/round/dst-balancing-result.js": "Duration",
    "intl402/Temporal/Duration/prototype/round/dst-rounding-result.js": "Duration",
    "intl402/Temporal/Duration/prototype/round/relativeto-string-datetime.js": "Duration",
    "intl402/Temporal/ZonedDateTime/prototype/round/dst-skipped-cross-midnight.js": "ZonedDateTime",
    "intl402/Temporal/ZonedDateTime/prototype/round/round-dst-boundaries.js": "ZonedDateTime",
}
_HARNESS_OWNERS = {"harness/temporalHelpers.js": "GenericTemporalHelper"}

AUDIT_TREE_ROOTS = (
    "harness",
    "test/built-ins/Temporal/PlainDateTime/prototype/round",
)
AUDIT_TREE_FILES = tuple("test/" + path for path in sorted(_HOMONYM_OWNERS))
AUDIT_TREE_PATHS = AUDIT_TREE_ROOTS + AUDIT_TREE_FILES

_EXPECTED_CANDIDATE_COUNT = 54
_EXPECTED_CANDIDATE_TOTALS = (81, 155, 144, 132, 11)
_EXPECTED_CANDIDATE_DIGEST = (
    "47655cbe1cfb5e5ecb91e6f8c82f990b3226d43cdf8fb94db3ccda1609e24752"
)
_EXPECTED_OWNERSHIP_ROWS = 54
_EXPECTED_OWNERSHIP_TOTALS = {
    "direct": (85, 73, 0),
    "homonym": (59, 59, 0),
    "harness": (0, 0, 11),
}
_EXPECTED_OWNERSHIP_DIGEST = (
    "08c71ca8353f61e841e5aba3b5109568971c79cb844cf3f7de13df281121ec34"
)
_EXPECTED_METADATA_DIGEST = (
    "f0baecbe0247101ca12e8b6a19581f3af7d8b01082e0120cc8b0ee59b19e79a5"
)
_EXPECTED_COMPUTED_AUDIT_FILES = 1_341
_EXPECTED_COMPUTED_AUDIT_TOTALS = (6_848, 9_472, 9_472, 11)
_EXPECTED_COMPUTED_AUDIT_DIGEST = (
    "dfebda4bcd36cafa174639d58f9ee968cd055d244562f067697773214556331a"
)


def property_reference_indices(tokens, name):
    return tuple(
        index
        for index, token in enumerate(tokens)
        if (token == name and index != 0 and tokens[index - 1] in (".", "?."))
        or token == ("string", name)
    )


def _is_identifier(token):
    return isinstance(token, str) and bool(token) and (
        token[0].isalpha() or token[0] in "_$"
    ) and all(char.isalnum() or char in "_$" for char in token)


def plain_date_time_bindings(tokens):
    bindings = set()
    changed = True
    while changed:
        changed = False
        for index in range(len(tokens) - 2):
            name = tokens[index]
            if not _is_identifier(name) or tokens[index + 1] != "=":
                continue
            rhs = tokens[index + 2 : index + 7]
            constructs = rhs[:4] == ("new", "Temporal", ".", "PlainDateTime")
            factory = rhs[:5] == (
                "Temporal",
                ".",
                "PlainDateTime",
                ".",
                "from",
            )
            aliases = bool(rhs) and rhs[0] in bindings
            if (constructs or factory or aliases) and name not in bindings:
                bindings.add(name)
                changed = True
    return frozenset(bindings)


def computed_member_indices(tokens):
    bindings = plain_date_time_bindings(tokens)
    stack = []
    members = []
    for index, token in enumerate(tokens):
        if token == "[":
            stack.append(index)
        elif token == "]" and stack:
            opening = stack.pop()
            if opening == 0:
                continue
            base_index = opening - 1
            if tokens[base_index] == "?." and base_index != 0:
                base_index -= 1
            if tokens[base_index] in bindings:
                members.append(index)
    return tuple(members)


def computed_call_indices(tokens):
    return tuple(
        index
        for index in computed_member_indices(tokens)
        if index + 1 < len(tokens)
        and (
            tokens[index + 1] == "("
            or (
                tokens[index + 1] == "?."
                and index + 2 < len(tokens)
                and tokens[index + 2] == "("
            )
        )
    )


def _candidate_stats(sources):
    candidates = {}
    expected = (
        TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES
        | set(_HOMONYM_OWNERS)
        | set(_HARNESS_OWNERS)
    )
    for relative, source in sources.items():
        if relative not in expected and (
            "PlainDateTime" not in source and "/PlainDateTime/" not in relative
        ):
            continue
        tokens = js_executable_tokens(source)
        references = len(property_reference_indices(tokens, "round"))
        calls = len(property_call_indices(tokens, "round"))
        computed_calls = (
            len(
                tuple(
                    index
                    for index, token in enumerate(tokens)
                    if token == "]"
                    and index + 1 < len(tokens)
                    and tokens[index + 1] == "("
                )
            )
            if relative in _HARNESS_OWNERS
            else len(computed_member_indices(tokens))
        )
        round_tokens = sum(
            token == "round" or token == ("string", "round") for token in tokens
        )
        has_candidate = (round_tokens != 0 or computed_calls != 0) and (
            "PlainDateTime" in tokens or "/PlainDateTime/" in relative
        )
        if relative not in expected and not has_candidate:
            continue
        candidates[relative] = (
            (
                tokens.count("PlainDateTime"),
                round_tokens,
                references,
                calls,
                computed_calls,
            ),
            tokens,
        )
    return candidates


def _candidate_digest(candidates):
    serialized = "".join(
        f"{relative}\t" + "\t".join(map(str, candidates[relative][0])) + "\n"
        for relative in sorted(candidates)
    )
    return sha256(serialized.encode()).hexdigest()


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
            "PlainDateTime computed-reference ambiguity drifted: "
            f"files={len(rows)} totals={totals} digest={digest}"
        )


def _ownership_rows(candidates):
    rows = []
    for relative in sorted(candidates):
        tokens = candidates[relative][1]
        references = len(property_reference_indices(tokens, "round"))
        calls = len(property_call_indices(tokens, "round"))
        computed_calls = (
            len(
                tuple(
                    index
                    for index, token in enumerate(tokens)
                    if token == "]"
                    and index + 1 < len(tokens)
                    and tokens[index + 1] == "("
                )
            )
            if relative in _HARNESS_OWNERS
            else len(computed_member_indices(tokens))
        )
        if not references and not computed_calls:
            continue
        if relative in _HARNESS_OWNERS:
            owner = _HARNESS_OWNERS[relative]
            category = "harness"
        else:
            owner = _HOMONYM_OWNERS.get(relative, "PlainDateTime")
            category = "homonym" if relative in _HOMONYM_OWNERS else "direct"
        rows.append((relative, references, calls, computed_calls, owner, category))
    return tuple(rows)


def verify_candidate_contract(candidates):
    expected = (
        TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES
        | set(_HOMONYM_OWNERS)
        | set(_HARNESS_OWNERS)
    )
    if set(candidates) != expected or len(candidates) != _EXPECTED_CANDIDATE_COUNT:
        raise RuntimeError(
            "PlainDateTime round candidate surface drifted: "
            f"missing={sorted(expected - set(candidates))} "
            f"outside={sorted(set(candidates) - expected)}"
        )
    totals = tuple(
        sum(counts[index] for counts, _ in candidates.values()) for index in range(5)
    )
    digest = _candidate_digest(candidates)
    if totals != _EXPECTED_CANDIDATE_TOTALS or digest != _EXPECTED_CANDIDATE_DIGEST:
        raise RuntimeError(
            f"PlainDateTime round candidate counts drifted: totals={totals} digest={digest}"
        )
    rows = _ownership_rows(candidates)
    serialized = "".join("\t".join(map(str, row)) + "\n" for row in rows)
    ownership_digest = sha256(serialized.encode()).hexdigest()
    ownership_totals = {
        category: tuple(
            sum(row[index] for row in rows if row[5] == category)
            for index in (1, 2, 3)
        )
        for category in _EXPECTED_OWNERSHIP_TOTALS
    }
    if (
        len(rows) != _EXPECTED_OWNERSHIP_ROWS
        or ownership_totals != _EXPECTED_OWNERSHIP_TOTALS
        or ownership_digest != _EXPECTED_OWNERSHIP_DIGEST
    ):
        raise RuntimeError(
            "PlainDateTime round ownership drifted: "
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
        live_files.update(path.relative_to(corpus_root).as_posix() for path in root.rglob("*.js"))
    for relative in AUDIT_TREE_FILES:
        path = corpus_root / relative
        if not path.is_file():
            raise FileNotFoundError(path)
        live_files.add(relative)
    if live_files != tree_files:
        raise RuntimeError(
            "configured Test262 sparse corpus/harness is incomplete: "
            f"missing={sorted(tree_files - live_files)} outside={sorted(live_files - tree_files)}"
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
                relative = (
                    member.name.removeprefix("test/")
                    if member.name.startswith("test/")
                    else member.name
                )
                expected = (
                    TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES
                    | set(_HOMONYM_OWNERS)
                    | set(_HARNESS_OWNERS)
                )
                if relative not in expected and (
                    "PlainDateTime" not in source and "/PlainDateTime/" not in relative
                ):
                    continue
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
        raise RuntimeError("configured Test262 corpus archive failed closed: " + stderr.strip())
    return sources


def _metadata_digest(test_root, parse_meta):
    rows = []
    for relative in sorted(TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES):
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
    corpus_root = Path(corpus_root)
    if not corpus_root.is_dir():
        raise FileNotFoundError(corpus_root)
    _verify_pinned_tree(corpus_root)
    test_root = corpus_root / "test"
    direct_dir = test_root / "built-ins/Temporal/PlainDateTime/prototype/round"
    if not direct_dir.is_dir():
        raise FileNotFoundError(direct_dir)
    live_direct = {
        path.relative_to(test_root).as_posix() for path in direct_dir.glob("*.js")
    }
    if live_direct != TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES:
        raise RuntimeError(
            "PlainDateTime round direct surface drifted: "
            f"missing={sorted(TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES - live_direct)} "
            f"outside={sorted(live_direct - TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES)}"
        )
    intl_files = _run_git(
        corpus_root,
        "ls-tree",
        "-r",
        "--name-only",
        PINNED_TEST262_REVISION,
        "--",
        "test/intl402/Temporal/PlainDateTime/prototype/round",
    ).strip()
    if intl_files:
        raise RuntimeError(f"PlainDateTime round Intl surface drifted: {intl_files}")
    for relative in sorted(TEMPORAL_PLAIN_DATE_TIME_ROUND_FILES):
        metadata = parse_meta((test_root / relative).read_text())
        actual = (
            frozenset(metadata.get("features", [])),
            frozenset(metadata.get("includes", [])),
            frozenset(metadata.get("flags", [])),
            metadata.get("negative"),
        )
        expected = (
            TEMPORAL_PLAIN_DATE_TIME_ROUND_FEATURES[relative],
            TEMPORAL_PLAIN_DATE_TIME_ROUND_INCLUDES[relative],
            TEMPORAL_PLAIN_DATE_TIME_ROUND_FLAGS[relative],
            TEMPORAL_PLAIN_DATE_TIME_ROUND_NEGATIVE[relative],
        )
        if actual != expected:
            raise RuntimeError(
                f"PlainDateTime round metadata fields drifted for {relative}: "
                f"actual={actual} expected={expected}"
            )
    metadata_digest = _metadata_digest(test_root, parse_meta)
    if metadata_digest != _EXPECTED_METADATA_DIGEST:
        raise RuntimeError(f"PlainDateTime round metadata drifted: {metadata_digest}")
    sources = _pinned_sources(corpus_root)
    verify_computed_ambiguity_contract(sources)
    candidates = _candidate_stats(sources)
    verify_candidate_contract(candidates)
    return candidates
