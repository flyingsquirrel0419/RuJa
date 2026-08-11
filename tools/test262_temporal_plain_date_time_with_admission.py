"""Pinned admission and ownership audit for PlainDateTime.prototype.with."""

from hashlib import sha256
from pathlib import Path

from test262_temporal_plain_date_time_conversions_admission import (
    js_executable_tokens,
    property_call_indices,
)
from test262_temporal_plain_date_time_round_admission import (
    _pinned_sources,
    _run_git,
    verify_computed_ambiguity_contract,
)


# [Decision Log]
# - Purpose and intent: Admit the 29 independently executable direct files
#   while retaining the one Temporal.Now dependency and all 70 non-ISO files.
# - Existing implementation and constraints: `with` is shared by six Temporal
#   prototypes, one direct fixture fails before the method, and sparse checkout
#   cannot prove downstream ownership.
# - Main alternatives considered: directory admission, aggregate results, raw
#   grep, or exact manifests plus full pinned executable-token ownership.
# - Selected approach: freeze direct/blocker/Intl/homonym manifests, metadata,
#   exact results/errors, and full-archive property references and calls.
# - Why this approach: it prevents absent-method false positives, homonyms, or
#   future files from silently becoming PlainDateTime support.
# - Benefits, drawbacks, and impact: all known ownership is fail-closed; the
#   full PlainDateTime token-identity audit is intentionally conservative.
PINNED_TEST262_REVISION = "9e61c12835c5e4a3bdba93850427e6742c4f64c4"
DIRECT_PREFIX = "built-ins/Temporal/PlainDateTime/prototype/with/"
INTL_PREFIX = "intl402/Temporal/PlainDateTime/prototype/with/"


def _read_manifest(name):
    lines = tuple(
        line
        for raw in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw.strip()) and not line.startswith("#")
    )
    if lines != tuple(sorted(lines)) or len(lines) != len(set(lines)):
        raise RuntimeError(f"{name} must be sorted and duplicate-free")
    return frozenset(lines)


TEMPORAL_PLAIN_DATE_TIME_WITH_FILES = _read_manifest(
    "test262_temporal_plain_date_time_with_admission.txt"
)
TEMPORAL_PLAIN_DATE_TIME_WITH_DIRECT_BLOCKERS = _read_manifest(
    "test262_temporal_plain_date_time_with_direct_blockers.txt"
)
TEMPORAL_PLAIN_DATE_TIME_WITH_INTL_BLOCKERS = _read_manifest(
    "test262_temporal_plain_date_time_with_intl_blockers.txt"
)
TEMPORAL_PLAIN_DATE_TIME_WITH_HOMONYMS = _read_manifest(
    "test262_temporal_plain_date_time_with_homonyms.txt"
)
TEMPORAL_PLAIN_DATE_TIME_WITH_COMPLETE_FILES = (
    TEMPORAL_PLAIN_DATE_TIME_WITH_FILES
    | TEMPORAL_PLAIN_DATE_TIME_WITH_DIRECT_BLOCKERS
    | TEMPORAL_PLAIN_DATE_TIME_WITH_INTL_BLOCKERS
)
TEMPORAL_PLAIN_DATE_TIME_WITH_PREIMPLEMENTATION_FALSE_POSITIVES = frozenset(
    DIRECT_PREFIX + name
    for name in (
        "argument-not-object.js",
        "calendar-throws.js",
        "options-invalid.js",
        "string-throws.js",
        "timezone-throws.js",
    )
)

if (
    len(TEMPORAL_PLAIN_DATE_TIME_WITH_FILES) != 29
    or len(TEMPORAL_PLAIN_DATE_TIME_WITH_DIRECT_BLOCKERS) != 1
    or len(TEMPORAL_PLAIN_DATE_TIME_WITH_INTL_BLOCKERS) != 70
    or len(TEMPORAL_PLAIN_DATE_TIME_WITH_HOMONYMS) != 75
    or len(TEMPORAL_PLAIN_DATE_TIME_WITH_COMPLETE_FILES) != 100
):
    raise RuntimeError("PlainDateTime with manifests have invalid cardinality")

_SYMBOL_FILES = frozenset({"branding.js", "options-invalid.js"})
_BIGINT_SYMBOL_FILES = frozenset({"options-wrong-type.js"})
_INTL_ERA_FILES = frozenset({"constrain-day.js"})
_REFLECT_FILES = frozenset({"not-a-constructor.js"})
_TEMPORAL_HELPER_FILES = frozenset(
    {
        "argument-object-insufficient-data.js",
        "basic-year-month-day.js",
        "basic.js",
        "constrain-day.js",
        "copy-properties-not-undefined.js",
        "multiple-unrecognized-properties-ignored.js",
        "options-empty.js",
        "overflow-undefined.js",
        "subclassing-ignored.js",
    }
)
_COMPARE_HELPER_FILES = frozenset(
    {
        "infinity-throws-rangeerror.js",
        "options-read-before-algorithmic-validation.js",
        "order-of-operations.js",
        "overflow-wrong-type.js",
    }
)


def _features(path):
    name = Path(path).name
    features = {"Temporal"}
    if name in _SYMBOL_FILES:
        features.add("Symbol")
    if name in _BIGINT_SYMBOL_FILES:
        features.update(("BigInt", "Symbol"))
    if name in _INTL_ERA_FILES:
        features.add("Intl.Era-monthcode")
    if name in _REFLECT_FILES:
        features.add("Reflect.construct")
    return frozenset(features)


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


TEMPORAL_PLAIN_DATE_TIME_WITH_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_DATE_TIME_WITH_FILES
}
TEMPORAL_PLAIN_DATE_TIME_WITH_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_DATE_TIME_WITH_FILES
}
TEMPORAL_PLAIN_DATE_TIME_WITH_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_TIME_WITH_FILES
}
TEMPORAL_PLAIN_DATE_TIME_WITH_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_TIME_WITH_FILES
}

_HARNESS_OWNERS = {"harness/temporalHelpers.js": "GenericTemporalHelper"}
_EXPECTED_CANDIDATE_COUNT = 176
_EXPECTED_CANDIDATE_TOTALS = (184, 1_419, 1_419, 1_407, 11)
_EXPECTED_CANDIDATE_DIGEST = (
    "06161973ab82fac830ede92e8e9f9fa1d051ff9eb3b53c054aa08675512119f4"
)
_EXPECTED_OWNERSHIP_ROWS = 176
_EXPECTED_OWNERSHIP_TOTALS = {
    "direct": (99, 87, 0),
    "direct_blocker": (1, 1, 0),
    "intl": (631, 631, 0),
    "homonym": (688, 688, 0),
    "harness": (0, 0, 11),
}
_EXPECTED_OWNERSHIP_DIGEST = (
    "e79854a4dcb773d64528527c26c726eb6822904677773d9a01707469ae03dbfa"
)
_EXPECTED_METADATA_DIGEST = (
    "928ca2756fc273444309e70f6afa292a2c453e2618ee831557c2f8eadee4b028"
)


def property_reference_indices(tokens):
    return tuple(
        index
        for index, token in enumerate(tokens)
        if (token == "with" and index != 0 and tokens[index - 1] in (".", "?."))
        or token == ("string", "with")
    )


def _generic_computed_calls(tokens):
    return sum(
        1
        for index, token in enumerate(tokens)
        if token == "]" and index + 1 < len(tokens) and tokens[index + 1] == "("
    )


def _candidate_stats(sources):
    expected = (
        TEMPORAL_PLAIN_DATE_TIME_WITH_COMPLETE_FILES
        | TEMPORAL_PLAIN_DATE_TIME_WITH_HOMONYMS
        | set(_HARNESS_OWNERS)
    )
    candidates = {}
    for relative, source in sources.items():
        if relative not in expected and (
            "PlainDateTime" not in source and "/PlainDateTime/" not in relative
        ):
            continue
        tokens = js_executable_tokens(source)
        references = len(property_reference_indices(tokens))
        calls = len(property_call_indices(tokens, "with"))
        computed_calls = _generic_computed_calls(tokens) if relative in _HARNESS_OWNERS else 0
        if relative not in expected and not (
            references and ("PlainDateTime" in tokens or "/PlainDateTime/" in relative)
        ):
            continue
        candidates[relative] = (
            (
                tokens.count("PlainDateTime"),
                sum(token == "with" or token == ("string", "with") for token in tokens),
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


def _homonym_owner(relative):
    if "/PlainDate/prototype/with/" in relative or relative.endswith(
        "Temporal/PlainDateTime/prototype/year/epoch-year.js"
    ):
        return "PlainDate"
    for owner in (
        "PlainMonthDay",
        "PlainTime",
        "PlainYearMonth",
        "ZonedDateTime",
    ):
        if f"/{owner}/prototype/with/" in relative:
            return owner
    raise RuntimeError(f"PlainDateTime with homonym has no owner: {relative}")


def _category(relative):
    if relative in TEMPORAL_PLAIN_DATE_TIME_WITH_FILES:
        return "PlainDateTime", "direct"
    if relative in TEMPORAL_PLAIN_DATE_TIME_WITH_DIRECT_BLOCKERS:
        return "PlainDateTime", "direct_blocker"
    if relative in TEMPORAL_PLAIN_DATE_TIME_WITH_INTL_BLOCKERS:
        return "PlainDateTime", "intl"
    if relative in TEMPORAL_PLAIN_DATE_TIME_WITH_HOMONYMS:
        return _homonym_owner(relative), "homonym"
    if relative in _HARNESS_OWNERS:
        return _HARNESS_OWNERS[relative], "harness"
    raise RuntimeError(f"PlainDateTime with candidate has no owner: {relative}")


def _ownership_rows(candidates):
    rows = []
    for relative in sorted(candidates):
        owner, category = _category(relative)
        counts, _ = candidates[relative]
        references, calls, computed_calls = counts[2:]
        if references or computed_calls:
            rows.append((relative, references, calls, computed_calls, owner, category))
    return tuple(rows)


def verify_candidate_contract(candidates):
    expected = (
        TEMPORAL_PLAIN_DATE_TIME_WITH_COMPLETE_FILES
        | TEMPORAL_PLAIN_DATE_TIME_WITH_HOMONYMS
        | set(_HARNESS_OWNERS)
    )
    if set(candidates) != expected or len(candidates) != _EXPECTED_CANDIDATE_COUNT:
        raise RuntimeError(
            "PlainDateTime with candidate surface drifted: "
            f"missing={sorted(expected - set(candidates))} "
            f"outside={sorted(set(candidates) - expected)}"
        )
    totals = tuple(
        sum(counts[index] for counts, _ in candidates.values()) for index in range(5)
    )
    digest = _candidate_digest(candidates)
    if totals != _EXPECTED_CANDIDATE_TOTALS or digest != _EXPECTED_CANDIDATE_DIGEST:
        raise RuntimeError(
            f"PlainDateTime with candidate counts drifted: totals={totals} digest={digest}"
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
            "PlainDateTime with ownership drifted: "
            f"rows={len(rows)} totals={ownership_totals} digest={ownership_digest}"
        )


def _metadata_digest(test_root, parse_meta):
    rows = []
    for relative in sorted(TEMPORAL_PLAIN_DATE_TIME_WITH_COMPLETE_FILES):
        metadata = parse_meta((test_root / relative).read_text())
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
    revision = _run_git(corpus_root, "rev-parse", "--verify", "HEAD^{commit}").strip()
    if revision != PINNED_TEST262_REVISION:
        raise RuntimeError(f"Test262 revision drifted: {revision}")
    test_root = corpus_root / "test"
    direct_dir = test_root / DIRECT_PREFIX
    intl_dir = test_root / INTL_PREFIX
    if not direct_dir.is_dir() or not intl_dir.is_dir():
        raise FileNotFoundError(direct_dir if not direct_dir.is_dir() else intl_dir)
    live_direct = {
        path.relative_to(test_root).as_posix() for path in direct_dir.glob("*.js")
    }
    expected_direct = (
        TEMPORAL_PLAIN_DATE_TIME_WITH_FILES
        | TEMPORAL_PLAIN_DATE_TIME_WITH_DIRECT_BLOCKERS
    )
    live_intl = {path.relative_to(test_root).as_posix() for path in intl_dir.glob("*.js")}
    if live_direct != expected_direct or live_intl != TEMPORAL_PLAIN_DATE_TIME_WITH_INTL_BLOCKERS:
        raise RuntimeError("PlainDateTime with direct or Intl directory drifted")
    audit_paths = (
        "harness",
        "test/" + DIRECT_PREFIX.rstrip("/"),
        "test/" + INTL_PREFIX.rstrip("/"),
        *("test/" + path for path in sorted(TEMPORAL_PLAIN_DATE_TIME_WITH_HOMONYMS)),
    )
    dirty = _run_git(
        corpus_root,
        "status",
        "--porcelain",
        "--untracked-files=all",
        "--",
        *audit_paths,
    )
    if dirty:
        raise RuntimeError(f"pinned Test262 corpus/harness is dirty: {dirty.strip()}")
    for relative in TEMPORAL_PLAIN_DATE_TIME_WITH_HOMONYMS:
        if not (test_root / relative).is_file():
            raise FileNotFoundError(test_root / relative)
    for relative in sorted(TEMPORAL_PLAIN_DATE_TIME_WITH_FILES):
        metadata = parse_meta((test_root / relative).read_text())
        actual = (
            frozenset(metadata.get("features", [])),
            frozenset(metadata.get("includes", [])),
            frozenset(metadata.get("flags", [])),
            metadata.get("negative"),
        )
        expected = (
            TEMPORAL_PLAIN_DATE_TIME_WITH_FEATURES[relative],
            TEMPORAL_PLAIN_DATE_TIME_WITH_INCLUDES[relative],
            TEMPORAL_PLAIN_DATE_TIME_WITH_FLAGS[relative],
            TEMPORAL_PLAIN_DATE_TIME_WITH_NEGATIVE[relative],
        )
        if actual != expected:
            raise RuntimeError(f"PlainDateTime with metadata fields drifted for {relative}")
    metadata_digest = _metadata_digest(test_root, parse_meta)
    if metadata_digest != _EXPECTED_METADATA_DIGEST:
        raise RuntimeError(f"PlainDateTime with metadata drifted: {metadata_digest}")
    sources = _pinned_sources(corpus_root)
    verify_computed_ambiguity_contract(sources)
    candidates = _candidate_stats(sources)
    verify_candidate_contract(candidates)
    return candidates
