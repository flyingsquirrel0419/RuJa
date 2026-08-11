"""Pinned admission and ownership audit for PlainDateTime conversions."""

from hashlib import sha256
from pathlib import Path
import re
import subprocess
import tarfile


# [Decision Log]
# - Purpose and intent: Admit exactly 52 direct conversion tests and prove that
#   no additional caller is eligible outside those directories.
# - Existing implementation and constraints: CI uses sparse Test262 checkouts,
#   while same-named methods on other Temporal types create lexical homonyms.
# - Main alternatives considered: scan only checked-out paths, use text regexes,
#   or inspect the complete pinned Git tree with a JavaScript-aware lexer.
# - Selected approach: stream test/ and harness/ from the pinned Git commit,
#   freeze token/call ownership, and verify direct live paths and metadata.
# - Why this approach: the Git archive removes sparse-checkout blind spots and
#   token ownership separates target calls from same-named methods on other types.
# - Benefits, drawbacks, and impact: drift fails closed with no new dependency;
#   each admission run pays a small full-corpus archive and tokenization cost.
PINNED_TEST262_REVISION = "9e61c12835c5e4a3bdba93850427e6742c4f64c4"
METHODS = ("toPlainDate", "toPlainTime", "withPlainTime")
DIRECT_PREFIXES = {
    method: f"built-ins/Temporal/PlainDateTime/prototype/{method}/"
    for method in METHODS
}
AUDIT_TREE_ROOTS = (
    "harness",
    "test/built-ins/Temporal/PlainDateTime/prototype/toPlainDate",
    "test/built-ins/Temporal/PlainDateTime/prototype/toPlainTime",
    "test/built-ins/Temporal/PlainDateTime/prototype/withPlainTime",
    "test/intl402/Temporal/PlainDate/prototype/withPlainTime",
)
AUDIT_TREE_PATHS = AUDIT_TREE_ROOTS


def _read_manifest():
    lines = tuple(
        line
        for raw_line in Path(__file__).with_name(
            "test262_temporal_plain_date_time_conversions_admission.txt"
        ).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )
    if tuple(sorted(lines)) != lines or len(set(lines)) != len(lines):
        raise RuntimeError(
            "PlainDateTime conversion manifest must be sorted and duplicate-free"
        )
    return frozenset(lines)


def _read_named_manifest(name):
    lines = tuple(
        line
        for raw_line in Path(__file__).with_name(name).read_text().splitlines()
        if (line := raw_line.strip()) and not line.startswith("#")
    )
    if tuple(sorted(lines)) != lines or len(set(lines)) != len(lines):
        raise RuntimeError(f"{name} must be sorted and duplicate-free")
    return frozenset(lines)


TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES = _read_manifest()
TEMPORAL_PLAIN_DATE_TIME_TO_PLAIN_DATE_FILES = frozenset(
    path
    for path in TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES
    if path.startswith(DIRECT_PREFIXES["toPlainDate"])
)
TEMPORAL_PLAIN_DATE_TIME_TO_PLAIN_TIME_FILES = frozenset(
    path
    for path in TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES
    if path.startswith(DIRECT_PREFIXES["toPlainTime"])
)
TEMPORAL_PLAIN_DATE_TIME_WITH_PLAIN_TIME_FILES = frozenset(
    path
    for path in TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES
    if path.startswith(DIRECT_PREFIXES["withPlainTime"])
)
TEMPORAL_PLAIN_DATE_TIME_CONVERSION_PREIMPLEMENTATION_FALSE_POSITIVES = (
    frozenset(
        {
            "built-ins/Temporal/PlainDateTime/prototype/withPlainTime/argument-number.js"
        }
    )
)

_ARROW_FILES = frozenset(
    {
        "argument-string-no-implicit-midnight.js",
        "argument-string-time-designator-required-for-disambiguation.js",
        "argument-string-with-time-designator.js",
        "argument-string-with-utc-designator.js",
        "year-zero.js",
    }
)
_WITH_PLAIN_TIME_TEMPORAL_HELPERS = frozenset(
    {
        "argument-object-insufficient-data.js",
        "argument-string-calendar-annotation.js",
        "argument-string-date-with-utc-offset.js",
        "argument-string-time-designator-required-for-disambiguation.js",
        "argument-string-time-separators.js",
        "argument-string-time-zone-annotation.js",
        "argument-string-unknown-annotation.js",
        "argument-string-with-time-designator.js",
        "argument-string-without-time-designator.js",
        "argument-time.js",
        "argument-zoneddatetime-balance-negative-time-units.js",
        "argument-zoneddatetime-negative-epochnanoseconds.js",
        "leap-second.js",
        "no-argument-default-to-midnight.js",
        "plaintime-propertybag-no-time-units.js",
        "subclassing-ignored.js",
    }
)


def _features(path):
    name = Path(path).name
    features = {"Temporal"}
    if name == "argument-wrong-type.js":
        features.update(("BigInt", "Symbol"))
    elif name == "branding.js":
        features.add("Symbol")
    elif name == "not-a-constructor.js":
        features.add("Reflect.construct")
    if name in _ARROW_FILES and "/withPlainTime/" in path:
        features.add("arrow-function")
    return frozenset(features)


def _includes(path):
    name = Path(path).name
    includes = set()
    if (
        "/toPlainDate/" in path
        and name in {"basic.js", "limits.js"}
        or "/toPlainTime/" in path
        and name == "basic.js"
        or "/withPlainTime/" in path
        and name in _WITH_PLAIN_TIME_TEMPORAL_HELPERS
    ):
        includes.add("temporalHelpers.js")
    if name == "order-of-operations.js":
        includes.update(("compareArray.js", "temporalHelpers.js"))
    if name in {"length.js", "name.js", "prop-desc.js"}:
        includes.add("propertyHelper.js")
    if name == "not-a-constructor.js":
        includes.add("isConstructor.js")
    return frozenset(includes)


TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FEATURES = {
    path: _features(path) for path in TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES
}
TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INCLUDES = {
    path: _includes(path) for path in TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES
}
TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES
}
TEMPORAL_PLAIN_DATE_TIME_CONVERSION_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES
}

_EXPECTED_DIRECT_COUNTS = (8, 7, 37)
TEMPORAL_PLAIN_DATE_TIME_CONVERSION_DOWNSTREAM_FILES = frozenset()
TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_FILES = _read_named_manifest(
    "test262_temporal_plain_date_time_conversions_intl_blockers.txt"
)
TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_FEATURES = {
    path: frozenset({"Temporal", "Intl.Era-monthcode"})
    for path in TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_FILES
}
TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_INCLUDES = {
    path: frozenset({"temporalHelpers.js"})
    for path in TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_FILES
}
TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_FLAGS = {
    path: frozenset() for path in TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_FILES
}
TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_NEGATIVE = {
    path: None for path in TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_FILES
}

_EXPECTED_CANDIDATE_TOTALS = (131, 53, 136, 79, 26, 108, 43, 8, 90)
_EXPECTED_CANDIDATE_DIGEST = (
    "d5b604de88ca4b5e86401bf0d0f04934c808bc04bde10d0bf5a15908521eafb7"
)
_EXPECTED_CANDIDATE_COUNT = 135
_EXPECTED_CALL_OWNERSHIP_DIGEST = (
    "37b45683a21b12df36eed44bdc54ab2453552bd48b3ee3dd99dbc8db9f0d40b6"
)
_EXPECTED_CALL_CATEGORY_TOTALS = {"direct": 49, "intl": 1, "homonym": 91}
_METHOD_OWNER_PATTERN = re.compile(
    r"^(?:built-ins|intl402)/Temporal/([^/]+)/prototype/"
    r"(?:toPlainDate|toPlainTime|withPlainTime)/"
)
_START_OF_DAY_HOMONYM = (
    "intl402/Temporal/ZonedDateTime/prototype/startOfDay/dst-basic.js"
)
_PATH_OWNED_HOMONYMS = frozenset({_START_OF_DAY_HOMONYM})

if (
    (
        len(TEMPORAL_PLAIN_DATE_TIME_TO_PLAIN_DATE_FILES),
        len(TEMPORAL_PLAIN_DATE_TIME_TO_PLAIN_TIME_FILES),
        len(TEMPORAL_PLAIN_DATE_TIME_WITH_PLAIN_TIME_FILES),
    )
    != _EXPECTED_DIRECT_COUNTS
    or len(TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES) != 52
    or TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES
    != (
        TEMPORAL_PLAIN_DATE_TIME_TO_PLAIN_DATE_FILES
        | TEMPORAL_PLAIN_DATE_TIME_TO_PLAIN_TIME_FILES
        | TEMPORAL_PLAIN_DATE_TIME_WITH_PLAIN_TIME_FILES
    )
    or not TEMPORAL_PLAIN_DATE_TIME_CONVERSION_PREIMPLEMENTATION_FALSE_POSITIVES
    < TEMPORAL_PLAIN_DATE_TIME_WITH_PLAIN_TIME_FILES
    or TEMPORAL_PLAIN_DATE_TIME_CONVERSION_DOWNSTREAM_FILES
    or len(TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_FILES) != 1
):
    raise RuntimeError(
        "PlainDateTime conversion admission must contain exact 8 + 7 + 37 files"
    )


_JS_REGEX_PREFIX_TOKENS = frozenset(
    {
        "(",
        "[",
        "{",
        ",",
        ";",
        ":",
        "=",
        "+=",
        "-=",
        "*=",
        "/=",
        "%=",
        "**=",
        "<<=",
        ">>=",
        ">>>=",
        "&=",
        "|=",
        "^=",
        "&&=",
        "||=",
        "??=",
        "=>",
        "!",
        "?",
        "+",
        "-",
        "*",
        "%",
        "&",
        "|",
        "^",
        "~",
        "<",
        ">",
        "<=",
        ">=",
        "==",
        "!=",
        "===",
        "!==",
        "<<",
        ">>",
        ">>>",
        "&&",
        "||",
        "??",
        "return",
        "throw",
        "case",
        "delete",
        "void",
        "typeof",
        "new",
        "in",
        "instanceof",
        "yield",
        "await",
        "else",
        "do",
    }
)
_JS_MULTI_CHAR_PUNCTUATORS = tuple(
    sorted(
        {
            ">>>=",
            "===",
            "!==",
            "**=",
            "<<=",
            ">>=",
            "&&=",
            "||=",
            "??=",
            ">>>",
            "=>",
            "++",
            "--",
            "**",
            "<<",
            ">>",
            "<=",
            ">=",
            "==",
            "!=",
            "&&",
            "||",
            "??",
            "?.",
            "+=",
            "-=",
            "*=",
            "/=",
            "%=",
            "&=",
            "|=",
            "^=",
            "...",
        },
        key=len,
        reverse=True,
    )
)


def js_executable_tokens(source):
    """Tokenize executable JS while excluding comments, regexps, and literal text."""

    tokens = []
    length = len(source)

    def regex_allowed():
        if not tokens:
            return True
        previous = tokens[-1]
        if previous == ")":
            depth = 0
            for token_index in range(len(tokens) - 1, -1, -1):
                token = tokens[token_index]
                if token == ")":
                    depth += 1
                elif token == "(":
                    depth -= 1
                    if depth == 0:
                        head = tokens[token_index - 1] if token_index else None
                        if head in ("if", "while", "for", "with", "switch", "catch"):
                            return True
                        return (
                            head == "await"
                            and token_index > 1
                            and tokens[token_index - 2] == "for"
                        )
            return False
        if previous == "of":
            depth = 0
            for token_index in range(len(tokens) - 2, -1, -1):
                token = tokens[token_index]
                if token == ")":
                    depth += 1
                elif token == "(":
                    if depth:
                        depth -= 1
                    else:
                        return token_index > 0 and tokens[token_index - 1] == "for"
            return False
        if previous not in _JS_REGEX_PREFIX_TOKENS:
            return False
        return len(tokens) < 2 or tokens[-2] not in (".", "?.")

    def scan_quoted(index, quote):
        value = []
        index += 1
        while index < length:
            char = source[index]
            if char == "\\":
                index += 1
                if index >= length:
                    break
                escaped = source[index]
                escapes = {
                    "b": "\b",
                    "f": "\f",
                    "n": "\n",
                    "r": "\r",
                    "t": "\t",
                    "v": "\v",
                    "0": "\0",
                }
                if escaped in "\r\n":
                    if (
                        escaped == "\r"
                        and index + 1 < length
                        and source[index + 1] == "\n"
                    ):
                        index += 1
                elif escaped == "x" and index + 2 < length:
                    value.append(chr(int(source[index + 1 : index + 3], 16)))
                    index += 2
                elif escaped == "u":
                    if index + 1 < length and source[index + 1] == "{":
                        close = source.find("}", index + 2)
                        if close == -1:
                            raise RuntimeError(
                                "invalid JavaScript string escape in corpus audit"
                            )
                        value.append(chr(int(source[index + 2 : close], 16)))
                        index = close
                    elif index + 4 < length:
                        value.append(chr(int(source[index + 1 : index + 5], 16)))
                        index += 4
                    else:
                        raise RuntimeError(
                            "invalid JavaScript string escape in corpus audit"
                        )
                else:
                    value.append(escapes.get(escaped, escaped))
                index += 1
            elif char == quote:
                return index + 1, "".join(value)
            elif char in "\r\n":
                raise RuntimeError("unterminated JavaScript string in corpus audit")
            else:
                value.append(char)
                index += 1
        raise RuntimeError("unterminated JavaScript string in corpus audit")

    def scan_regex(index):
        index += 1
        in_class = False
        while index < length:
            char = source[index]
            if char == "\\":
                index += 2
                continue
            if char in "\r\n":
                raise RuntimeError("unterminated JavaScript regexp in corpus audit")
            if char == "[":
                in_class = True
            elif char == "]":
                in_class = False
            elif char == "/" and not in_class:
                index += 1
                while index < length and (
                    source[index].isalnum() or source[index] in "_$"
                ):
                    index += 1
                return index
            index += 1
        raise RuntimeError("unterminated JavaScript regexp in corpus audit")

    def scan_template(index):
        index += 1
        while index < length:
            char = source[index]
            if char == "\\":
                index += 2
            elif char == "`":
                return index + 1
            elif source.startswith("${", index):
                index = scan_code(index + 2, stop_at_closing_brace=True)
            else:
                index += 1
        raise RuntimeError("unterminated JavaScript template in corpus audit")

    def scan_code(index, stop_at_closing_brace=False):
        brace_depth = 0
        while index < length:
            char = source[index]
            if char.isspace():
                index += 1
                continue
            if source.startswith("//", index):
                newline = source.find("\n", index + 2)
                index = length if newline == -1 else newline + 1
                continue
            if source.startswith("/*", index):
                close = source.find("*/", index + 2)
                if close == -1:
                    raise RuntimeError("unterminated JavaScript comment in corpus audit")
                index = close + 2
                continue
            if char in "'\"":
                index, value = scan_quoted(index, char)
                tokens.append(("string", value))
                continue
            if char == "`":
                index = scan_template(index)
                continue
            if char == "}" and stop_at_closing_brace and brace_depth == 0:
                return index + 1
            if char == "{":
                brace_depth += 1
            elif char == "}":
                brace_depth -= 1
            if char == "/" and regex_allowed():
                index = scan_regex(index)
                continue
            if char.isalpha() or char in "_$":
                end = index + 1
                while end < length and (
                    source[end].isalnum() or source[end] in "_$"
                ):
                    end += 1
                tokens.append(source[index:end])
                index = end
                continue
            if char.isdigit():
                end = index + 1
                while end < length and (
                    source[end].isalnum() or source[end] in "._$"
                ):
                    end += 1
                tokens.append(source[index:end])
                index = end
                continue
            punctuator = next(
                (
                    candidate
                    for candidate in _JS_MULTI_CHAR_PUNCTUATORS
                    if source.startswith(candidate, index)
                ),
                None,
            )
            if punctuator is not None:
                tokens.append(punctuator)
                index += len(punctuator)
                continue
            tokens.append(char)
            index += 1
        if stop_at_closing_brace:
            raise RuntimeError(
                "unterminated JavaScript template expression in corpus audit"
            )
        return index

    scan_code(0)
    return tuple(tokens)


def _matching_token(tokens, start, opening, closing):
    depth = 0
    for index in range(start, len(tokens)):
        if tokens[index] == opening:
            depth += 1
        elif tokens[index] == closing:
            depth -= 1
            if depth == 0:
                return index
    raise RuntimeError(f"unmatched JavaScript {opening} in corpus audit")


def property_reference_indices(tokens, property_name):
    references = []
    for index, token in enumerate(tokens):
        if (
            token in (".", "?.")
            and index + 1 < len(tokens)
            and tokens[index + 1] == property_name
        ):
            references.append(index)
        elif (
            token == "["
            and index + 2 < len(tokens)
            and tokens[index + 1] == ("string", property_name)
            and tokens[index + 2] == "]"
        ):
            references.append(index)
    return tuple(references)


def property_call_indices(tokens, property_name):
    calls = []
    for index in property_reference_indices(tokens, property_name):
        if tokens[index] in (".", "?."):
            end = index + 2
        else:
            receiver = index - 1
            if receiver >= 0 and tokens[receiver] == "?.":
                receiver -= 1
            if receiver < 0 or tokens[receiver] in _JS_REGEX_PREFIX_TOKENS:
                continue
            end = index + 3
        if end < len(tokens) and tokens[end] == "?.":
            end += 1
        if end >= len(tokens) or tokens[end] != "(":
            continue
        parameters_end = _matching_token(tokens, end, "(", ")")
        if (
            index < len(tokens)
            and tokens[index] == "["
            and parameters_end + 1 < len(tokens)
            and tokens[parameters_end + 1] == "{"
        ):
            continue
        calls.append(index)
    return tuple(calls)


def candidate_stats(sources):
    """Return exact token/reference/call counts for conversion-name candidates."""

    candidates = {}
    for relative, source in sources.items():
        if not any(method in source for method in METHODS):
            continue
        tokens = js_executable_tokens(source)
        names = tuple(
            sum(token == method or token == ("string", method) for token in tokens)
            for method in METHODS
        )
        if not any(names):
            continue
        references = tuple(
            len(property_reference_indices(tokens, method)) for method in METHODS
        )
        calls = tuple(len(property_call_indices(tokens, method)) for method in METHODS)
        candidates[relative] = (names + references + calls, tokens)
    return candidates


def _candidate_digest(candidates):
    serialized = "".join(
        f"{path}\t" + "\t".join(map(str, candidates[path][0])) + "\n"
        for path in sorted(candidates)
    )
    return sha256(serialized.encode()).hexdigest()


def _receiver_for_call(tokens, call_index):
    receiver_index = call_index - 1
    if receiver_index >= 0 and tokens[receiver_index] == "?.":
        receiver_index -= 1
    if receiver_index < 0:
        return None
    receiver = tokens[receiver_index]
    if isinstance(receiver, str) and re.fullmatch(
        r"[A-Za-z_$][A-Za-z0-9_$]*", receiver
    ):
        return receiver
    return None


def _receiver_temporal_types(tokens, receiver):
    types = set()
    for index, token in enumerate(tokens):
        if token != receiver:
            continue
        suffix = tokens[index + 1 : index + 6]
        for prefix in (
            ("instanceof", "Temporal", "."),
            ("=", "new", "Temporal", "."),
            ("=", "Temporal", "."),
        ):
            if suffix[: len(prefix)] == prefix:
                temporal_type = suffix[len(prefix)]
                if isinstance(temporal_type, str):
                    types.add(temporal_type)
    return tuple(sorted(types))


def _call_ownership_rows(candidates):
    rows = []
    for relative in sorted(candidates):
        counts, tokens = candidates[relative]
        for method_index, method in enumerate(METHODS):
            call_indices = property_call_indices(tokens, method)
            if len(call_indices) != counts[6 + method_index]:
                raise RuntimeError(
                    f"PlainDateTime conversion call recount drifted: {relative}: {method}"
                )
            if not call_indices:
                continue
            if relative in TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES:
                owner, category = "PlainDateTime", "direct"
            elif relative in TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_FILES:
                owner, category = "PlainDateTime", "intl"
            elif relative == _START_OF_DAY_HOMONYM:
                owner, category = "ZonedDateTime", "homonym"
            elif relative == "harness/temporalHelpers.js":
                owner, category = "PlainYearMonth", "homonym"
            else:
                match = _METHOD_OWNER_PATTERN.match(relative)
                if match is None:
                    raise RuntimeError(
                        f"PlainDateTime conversion call has no pinned owner: {relative}"
                    )
                owner, category = match.group(1), "homonym"

            if relative not in _PATH_OWNED_HOMONYMS:
                for call_index in call_indices:
                    receiver = _receiver_for_call(tokens, call_index)
                    inferred = (
                        _receiver_temporal_types(tokens, receiver)
                        if receiver is not None
                        else ()
                    )
                    if inferred and owner not in inferred:
                        raise RuntimeError(
                            "PlainDateTime conversion receiver ownership drifted: "
                            f"{relative}: {method}: {receiver}: {inferred}"
                        )
            rows.append((relative, method, len(call_indices), owner, category))
    return tuple(rows)


def verify_candidate_contract(candidates):
    if (
        len(candidates) != _EXPECTED_CANDIDATE_COUNT
        or not TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES <= set(candidates)
    ):
        raise RuntimeError(
            "PlainDateTime conversion candidate surface drifted: "
            f"count={len(candidates)} direct_missing="
            f"{sorted(TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES - set(candidates))}"
        )
    totals = tuple(
        sum(counts[index] for counts, _ in candidates.values())
        for index in range(9)
    )
    digest = _candidate_digest(candidates)
    if totals != _EXPECTED_CANDIDATE_TOTALS or digest != _EXPECTED_CANDIDATE_DIGEST:
        raise RuntimeError(
            "PlainDateTime conversion candidate/call counts drifted: "
            f"totals={totals} digest={digest}"
        )

    ownership = _call_ownership_rows(candidates)
    serialized_ownership = "".join(
        "\t".join(map(str, row)) + "\n" for row in ownership
    )
    ownership_digest = sha256(serialized_ownership.encode()).hexdigest()
    category_totals = {
        category: sum(row[2] for row in ownership if row[4] == category)
        for category in ("direct", "intl", "homonym")
    }
    if (
        ownership_digest != _EXPECTED_CALL_OWNERSHIP_DIGEST
        or category_totals != _EXPECTED_CALL_CATEGORY_TOTALS
    ):
        raise RuntimeError(
            "PlainDateTime conversion outside call ownership drifted: "
            f"totals={category_totals} digest={ownership_digest}"
        )
    outside_plain_date_time = {
        relative
        for relative, _, _, owner, _ in ownership
        if owner == "PlainDateTime"
        and relative
        not in (
            TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES
            | TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_FILES
        )
    }
    if outside_plain_date_time:
        raise RuntimeError(
            "PlainDateTime conversion gained an unaccounted downstream call outside "
            f"direct directories: {sorted(outside_plain_date_time)}"
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
        raise RuntimeError(
            "configured Test262 corpus verification failed closed"
        ) from error
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
    if live_files != tree_files:
        raise RuntimeError(
            "configured Test262 sparse corpus/harness is incomplete: "
            f"missing={sorted(tree_files - live_files)} "
            f"outside={sorted(live_files - tree_files)}"
        )
    return live_files


def _pinned_candidate_sources(corpus_root):
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
        process = subprocess.Popen(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as error:
        raise RuntimeError(
            "configured Test262 corpus archive failed closed"
        ) from error

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
                    raise RuntimeError(
                        f"configured Test262 corpus archive omitted {member.name}"
                    )
                source = archived.read().decode("utf-8")
                if not any(method in source for method in METHODS):
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
        raise RuntimeError(
            "configured Test262 corpus archive failed closed"
        ) from error
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


def audit_corpus(corpus_root, parse_meta):
    """Fail closed unless the pinned direct and no-downstream contracts are exact."""

    corpus_root = Path(corpus_root)
    if not corpus_root.is_dir():
        raise FileNotFoundError(corpus_root)
    _verify_pinned_tree(corpus_root)
    test_root = corpus_root / "test"

    live_direct = set()
    for method in METHODS:
        directory = test_root / DIRECT_PREFIXES[method]
        if not directory.is_dir():
            raise FileNotFoundError(directory)
        live_direct.update(
            path.relative_to(test_root).as_posix() for path in directory.glob("*.js")
        )
    if live_direct != TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES:
        raise RuntimeError(
            "PlainDateTime conversion live directories drifted: "
            f"expected={sorted(TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES)} "
            f"actual={sorted(live_direct)}"
        )

    for relative in sorted(TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FILES):
        path = test_root / relative
        metadata = parse_meta(path.read_text())
        actual = (
            frozenset(metadata.get("features", [])),
            frozenset(metadata.get("includes", [])),
            frozenset(metadata.get("flags", [])),
            metadata.get("negative"),
        )
        expected = (
            TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FEATURES[relative],
            TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INCLUDES[relative],
            TEMPORAL_PLAIN_DATE_TIME_CONVERSION_FLAGS[relative],
            TEMPORAL_PLAIN_DATE_TIME_CONVERSION_NEGATIVE[relative],
        )
        if actual != expected:
            raise RuntimeError(
                f"PlainDateTime conversion metadata drifted: {relative}: {actual}"
            )

    for relative in sorted(TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_FILES):
        path = test_root / relative
        metadata = parse_meta(path.read_text())
        actual = (
            frozenset(metadata.get("features", [])),
            frozenset(metadata.get("includes", [])),
            frozenset(metadata.get("flags", [])),
            metadata.get("negative"),
        )
        expected = (
            TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_FEATURES[relative],
            TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_INCLUDES[relative],
            TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_FLAGS[relative],
            TEMPORAL_PLAIN_DATE_TIME_CONVERSION_INTL_NEGATIVE[relative],
        )
        if actual != expected:
            raise RuntimeError(
                f"PlainDateTime conversion Intl metadata drifted: {relative}: {actual}"
            )

    candidates = candidate_stats(_pinned_candidate_sources(corpus_root))
    verify_candidate_contract(candidates)
    return candidates
