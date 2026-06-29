#!/usr/bin/env python3
"""Minimal test262 runner for RuJa.

Runs a subset of test262 language tests through the RuJa binary and reports
pass/fail counts. Uses the real test262 harness files (assert.js, sta.js, and
any per-test `includes:`) rather than a hand-rolled stub, so tests relying on
`verifyProperty`, `compareArray`, etc. are exercised correctly.
"""
import os, re, subprocess, sys
from pathlib import Path

RUJA = str(Path(__file__).resolve().parent.parent / "target/debug/ruja")
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
    """Parse the /*--- ... ---*/ metadata block, handling multi-line lists."""
    m = re.search(r'/\*---\n(.*?)\n---\*/', src, re.DOTALL)
    if not m:
        return {}
    meta = {}
    block = m.group(1)
    # YAML-ish: we capture flags/features/includes as bracketed or bare lists.
    for key in ('flags', 'features', 'includes'):
        # match `key: [a, b]` or `key: [a]`
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

# Harness files always loaded (the minimum test262 requires).
BASE_HARNESS = ['sta.js', 'assert.js']

def build_source(path):
    """Build the full source: harness files + the test."""
    src = Path(path).read_text()
    meta = parse_meta(src)
    parts = []
    # Base harness (sta.js defines Test262Error; assert.js needs it).
    for inc in BASE_HARNESS:
        p = HARNESS / inc
        if p.exists():
            parts.append(p.read_text())
    # Per-test includes (propertyHelper.js, compareArray.js, etc.).
    for inc in meta.get('includes', []):
        p = HARNESS / inc
        if p.exists():
            parts.append(p.read_text())
    parts.append(src)
    return "\n".join(parts), meta

def run_test(path):
    full, meta = build_source(path)
    if should_skip(meta):
        return 'skip'
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
                return 'pass'
            return 'fail'
        return 'pass' if (r.returncode == 0 and not out) else 'fail'
    except subprocess.TimeoutExpired:
        return 'timeout'
    except Exception:
        return 'error'

def main():
    dirs = sys.argv[1:] if len(sys.argv) > 1 else ['language/expressions']
    counts = {'pass': 0, 'fail': 0, 'skip': 0, 'timeout': 0, 'error': 0}
    total = 0
    for d in dirs:
        base = Path(TEST262) / 'test' / d
        if not base.exists():
            continue
        for f in sorted(base.rglob('*.js')):
            if '_FIXTURE' in f.name:
                continue
            total += 1
            if total % 100 == 0:
                sys.stderr.write(f"  ...{total} tests, {counts['pass']} pass, {counts['fail']} fail\n")
            counts[run_test(f)] += 1
    ran = counts['pass'] + counts['fail']
    print(f"\nResults over {total} tests (ran {ran}):")
    for k in ['pass', 'fail', 'skip', 'timeout', 'error']:
        print(f"  {k}: {counts[k]}")
    if ran > 0:
        rate = 100 * counts['pass'] / ran
        print(f"  pass rate (of run): {rate:.1f}%")
        print(f"RATE={rate:.1f} PASS={counts['pass']} FAIL={counts['fail']} "
              f"SKIP={counts['skip']} TOTAL={total} RAN={ran}")
    else:
        print("RATE=0.0 PASS=0 FAIL=0 SKIP=0 TOTAL=0 RAN=0")

if __name__ == '__main__':
    main()
