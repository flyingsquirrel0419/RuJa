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
    "module", "import-assertions", "top-level-await", "arraybuffer",
    "sharedarraybuffer", "atomics", "DataView", "TypedArray",
    "Intl", "WeakRef", "FinalizationRegistry", "AggregateError",
    "resizable-arraybuffer", "regexp-v-flag", "regexp-duplicate-named-groups",
    "json-modules", "import-attributes", "hashbang",
    "regexp-named-groups", "regexp-unicode-property-escapes",
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
    # negative: { phase: <parse|runtime|resolution>, type: <ErrorName> }
    mn = re.search(r'^negative:\s*\n(  phase:\s*(\w+)\n  type:\s*(\w+)|  type:\s*(\w+)\n  phase:\s*(\w+))', block, re.MULTILINE)
    if mn:
        phase = mn.group(2) or mn.group(5)
        typ = mn.group(3) or mn.group(4)
        meta['negative'] = {'phase': phase, 'type': typ}
    return meta

def should_skip(meta):
    feats = set(meta.get('features', []))
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
    parts = []
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
    if should_skip(meta):
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
            return 'fail', out or f'expected {want}, no error raised'
        if r.returncode == 0 and not out:
            return 'pass', ''
        return 'fail', out
    except subprocess.TimeoutExpired:
        return 'timeout', ''
    except Exception as e:
        return 'error', str(e)

def normalize_err(err):
    e = err
    if 'ParseError' in e or 'SyntaxError' in e and 'parse' in e.lower():
        if 'Unexpected token' in e:
            return 'PARSE: unexpected token'
        if 'Expected' in e:
            return 'PARSE: expected something'
        return 'PARSE: other'
    if 'not defined' in e.lower() or 'is not defined' in e.lower():
        return 'RUNTIME: not defined'
    if 'not a function' in e.lower():
        return 'RUNTIME: not a function'
    if 'cannot read propert' in e.lower() or ('undefined' in e.lower() and 'read' in e.lower()):
        return 'RUNTIME: cannot read property of undefined'
    if 'TypeError' in e:
        return 'RUNTIME: TypeError'
    if 'RangeError' in e:
        return 'RUNTIME: RangeError'
    if 'ReferenceError' in e:
        return 'RUNTIME: ReferenceError'
    if 'stack' in e.lower() and 'overflow' in e.lower():
        return 'RUNTIME: stack overflow'
    if 'panic' in e.lower():
        return 'PANIC'
    if 'Test262Error' in e:
        return 'ASSERT: Test262 assertion failed'
    if 'Assertion' in e:
        return 'ASSERT: failed'
    if not e:
        return 'FAIL: no output (nonzero exit)'
    first = e.splitlines()[0][:120] if e else ''
    return f'OTHER: {first}'

def main():
    dirs = sys.argv[1:] if len(sys.argv) > 1 else ['language/identifiers']
    fails = []
    counts = Counter()
    total = 0
    for d in dirs:
        base = Path(TEST262) / 'test' / d
        if not base.exists():
            continue
        for f in sorted(base.rglob('*.js')):
            if '_FIXTURE' in f.name:
                continue
            total += 1
            status, err = run_test(f)
            counts[status] += 1
            if status == 'fail':
                bucket = normalize_err(err)
                fails.append((str(f.relative_to(Path(TEST262)/'test')), bucket, err[:300]))
    ran = counts['pass'] + counts['fail']
    rate = 100 * counts['pass'] / ran if ran else 0
    print(f"\n=== SUMMARY over {total} tests (ran {ran}) ===")
    print(f"pass rate: {rate:.1f}%  pass={counts['pass']} fail={counts['fail']} skip={counts['skip']}")
    print("\n=== FAIL BUCKETS ===")
    bucket_counts = Counter(b for _,b,_ in fails)
    for b, c in bucket_counts.most_common():
        print(f"  {c:4d}  {b}")
    print("\n=== SAMPLE FAILS PER BUCKET ===")
    by_bucket = defaultdict(list)
    for path, bucket, err in fails:
        by_bucket[bucket].append((path, err))
    for bucket, items in sorted(by_bucket.items(), key=lambda x:-len(x[1]))[:8]:
        print(f"\n--- {bucket} ({len(items)}) ---")
        for path, err in items[:4]:
            print(f"  {path}")
            for line in err.splitlines()[:3]:
                print(f"      {line}")
    out = Path('/tmp/ruja_test262_fails.json')
    out.write_text(json.dumps({'rate': rate, 'counts': dict(counts), 'fails': fails}, indent=2))
    print(f"\nfull dump -> {out}")

if __name__ == '__main__':
    main()
