#!/usr/bin/env python3
"""Analyze test262 failures: collect failing test paths + RuJa stderr,
then bucket by error message pattern to find high-frequency real bugs."""
import os, re, subprocess, sys, json
from pathlib import Path
from collections import Counter, defaultdict

RUJA = str(Path(__file__).resolve().parent.parent / "target/release/ruja")
TEST262 = os.environ.get("TEST262", "/root/test262")
HARNESS = Path(TEST262) / "harness"

SKIP_FEATURES = {
    "AggregateError", "ArrayBuffer", "DataView", "FinalizationRegistry",
    "Float16Array", "Float32Array", "Float64Array", "Int8Array", "Int16Array",
    "Int32Array", "Intl", "Map", "Promise", "Set", "SharedArrayBuffer",
    "Symbol", "Symbol.asyncIterator", "Symbol.hasInstance", "Symbol.iterator",
    "Symbol.toPrimitive", "Symbol.toStringTag",
    "TypedArray", "Uint8Array", "Uint8Array-base64", "Uint8Array-hex",
    "Uint8ClampedArray", "Uint16Array", "Uint32Array", "WeakMap", "WeakRef",
    "WeakSet", "arraybuffer", "async-functions", "async-iteration", "atomics",
    "class-fields-private", "class-fields-private-in",
    "class-fields-public", "class-methods-private",
    "class-static-fields-private", "class-static-fields-public",
    "class-static-methods-private", "decorators",
    "default-parameters", "destructuring-binding",
    "dynamic-import", "error-cause", "explicit-resource-management",
    "export-star-as-namespace-from-module",
    "generators", "hashbang", "import-assertions",
    "import-attributes", "import-defer", "import.meta", "iterator-helpers",
    "json-modules", "module",
    "object-rest", "optional-chaining",
    "proxy-missing-checks", "Proxy", "Reflect",
    "Reflect.construct", "regexp-duplicate-named-groups",
    "regexp-named-groups", "regexp-unicode-property-escapes", "regexp-v-flag",
    "resizable-arraybuffer", "shadowrealm",
    "sharedarraybuffer", "source-phase-imports",
    "source-phase-imports-module-source", "tail-call-optimization",
    "top-level-await", "u180e",
}

EXPLICIT_RESOURCE_MANAGEMENT_SYMBOL_PREFIXES = (
    "built-ins/Symbol/asyncDispose/",
    "built-ins/Symbol/dispose/",
)

SYMBOL_FUNCTION_NAME_PREFIXES = (
    "language/expressions/object/fn-name-",
    "language/expressions/object/method-definition/fn-name-",
    "language/statements/class/definition/fn-name-",
)

FOR_OF_SYMBOL_ITERATOR_PREFIXES = (
    "language/statements/for-of/",
)

TYPED_ARRAY_CONSTRUCTORS_PREFIXES = (
    "built-ins/TypedArrayConstructors/",
)

TYPED_ARRAY_CONSTRUCTORS_FEATURES = {
    "TypedArray",
    "ArrayBuffer",
    "DataView",
    "Reflect",
    "Reflect.construct",
    "Symbol.toPrimitive",
    "Symbol.toStringTag",
}

DATA_VIEW_PREFIXES = (
    "built-ins/DataView/",
)

DATA_VIEW_FEATURES = {
    "ArrayBuffer",
    "DataView",
    "Float16Array",
    "Int8Array",
    "Reflect",
    "Reflect.construct",
    "Symbol.toPrimitive",
    "Symbol.toStringTag",
    "Uint8Array",
}

def parse_meta(src):
    m = re.search(r'/\*---\n(.*?)\n---\*/', src, re.DOTALL)
    if not m:
        return {}
    meta = {}
    block = m.group(1)
    for key in ('flags', 'features', 'includes'):
        m2 = re.search(rf'^{key}:\s*\[(.*?)\]', block, re.MULTILINE | re.DOTALL)
        if m2:
            meta[key] = [x.strip() for x in m2.group(1).split(',') if x.strip()]
    mn = re.search(
        r'^negative:\s*\n(\s+phase:\s*(\w+)\n\s+type:\s*(\w+)|\s+type:\s*(\w+)\n\s+phase:\s*(\w+))',
        block,
        re.MULTILINE,
    )
    if mn:
        phase = mn.group(2) or mn.group(5)
        typ = mn.group(3) or mn.group(4)
        meta['negative'] = {'phase': phase, 'type': typ}
    return meta

def explicit_resource_management_symbols_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(EXPLICIT_RESOURCE_MANAGEMENT_SYMBOL_PREFIXES)

def symbol_function_name_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(SYMBOL_FUNCTION_NAME_PREFIXES)

def for_of_symbol_iterator_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(FOR_OF_SYMBOL_ITERATOR_PREFIXES)

def typed_array_constructors_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(TYPED_ARRAY_CONSTRUCTORS_PREFIXES)

def data_view_path(path):
    try:
        rel = Path(path).resolve().relative_to((Path(TEST262) / "test").resolve())
    except ValueError:
        return False
    rel_text = rel.as_posix()
    return rel_text.startswith(DATA_VIEW_PREFIXES)

def should_skip(meta, path=None):
    feats = set(meta.get('features', []))
    if path is not None and explicit_resource_management_symbols_path(path):
        feats.discard("explicit-resource-management")
    if path is not None and symbol_function_name_path(path):
        feats.discard("Symbol")
    if path is not None and for_of_symbol_iterator_path(path):
        feats.discard("Symbol.iterator")
    if path is not None and typed_array_constructors_path(path):
        feats.difference_update(TYPED_ARRAY_CONSTRUCTORS_FEATURES)
    if path is not None and data_view_path(path):
        feats.difference_update(DATA_VIEW_FEATURES)
    if feats & SKIP_FEATURES:
        return True
    flags = meta.get('flags', [])
    if 'module' in flags or 'async' in flags:
        return True
    return False

BASE_HARNESS = ['sta.js', 'assert.js']

def build_source(path):
    src = Path(path).read_text()
    meta = parse_meta(src)
    flags = meta.get('flags', [])
    if 'raw' in flags:
        return src, meta
    parts = []
    # onlyStrict must be a directive prologue before any harness code, matching
    # test262_runner.py. Otherwise strict-mode negative tests are misbucketed.
    if 'onlyStrict' in flags:
        parts.append("'use strict';")
    for inc in BASE_HARNESS:
        p = HARNESS / inc
        if p.exists():
            parts.append(p.read_text())
    for inc in meta.get('includes', []):
        p = HARNESS / inc
        if p.exists():
            parts.append(p.read_text())
    parts.append(src)
    return "\n".join(parts), meta

def run_test(path):
    """Return (status, err). For negative tests a thrown error of the
    expected type counts as pass. RuJa reports errors via stderr/stdout and
    may exit 0 or nonzero, so we judge by error text, not exit code."""
    full, meta = build_source(path)
    if should_skip(meta, path):
        return 'skip', ''
    try:
        import tempfile
        with tempfile.NamedTemporaryFile('w', suffix='.js', delete=False) as tf:
            tf.write(full)
            tmpname = tf.name
        try:
            r = subprocess.run([RUJA, tmpname], capture_output=True, text=True, timeout=8)
        finally:
            os.unlink(tmpname)
        out = (r.stderr + r.stdout).strip()
        neg = meta.get('negative')
        if neg:
            want = neg.get('type', '')
            if want and want in out:
                return 'pass', ''
            return 'fail', out
        if r.returncode == 0 and not out:
            return 'pass', ''
        return 'fail', out
    except subprocess.TimeoutExpired:
        return 'timeout', ''
    except Exception as e:
        return 'error', str(e)

def bucket(err):
    if not err:
        return 'OTHER: (no output)'
    # normalize paths/ids
    err = re.sub(r"'[^']{5,}'", "'<value>'", err)
    err = re.sub(r'\([^)]*\)', '()', err)
    err = re.sub(r'at line \d+', 'at line <n>', err)
    err = re.sub(r'\[[^\]]+\]', '[]', err)
    err = err.strip().split('\n')[0]
    return err[:200]

def main():
    dirs = sys.argv[1:] if len(sys.argv) > 1 else ['language/expressions']
    fails = defaultdict(list)
    counts = Counter()
    for d in dirs:
        base = Path(TEST262) / 'test' / d
        if not base.exists():
            print(f"SKIP missing: {base}", file=sys.stderr)
            continue
        files = sorted(base.rglob('*.js'))
        print(f"Scanning {len(files)} files under {d} ...", file=sys.stderr)
        for f in files:
            if '_FIXTURE' in f.name:
                continue
            status, err = run_test(f)
            if status == 'fail':
                b = bucket(err)
                fails[b].append((str(f.relative_to(Path(TEST262) / 'test')), err))
                counts[b] += 1

    # Sort buckets by frequency
    print("\n=== SUMMARY ===")
    for b, c in counts.most_common():
        print(f"{c:>4} {b}")
    print("\n=== SAMPLE FAILS PER BUCKET ===")
    for b, items in fails.items():
        print(f"\n--- {b} ({len(items)}) ---")
        for p, e in items[:3]:
            print(f"  {p}")
            print(f"      {e[:200]}")

    out = '/tmp/ruja_test262_fails.json'
    with open(out, 'w') as f:
        json.dump({b: items for b, items in fails.items()}, f, indent=2)
    print(f"\nfull dump -> {out}")

if __name__ == '__main__':
    main()
