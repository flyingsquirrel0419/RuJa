#!/usr/bin/env python3
"""Analyze test262 failures: dump failing test paths grouped by directory."""
import os, re, subprocess, sys, json
from pathlib import Path
from collections import defaultdict

# Reuse runner functions
sys.path.insert(0, str(Path(__file__).parent))
from test262_runner import build_source, should_skip, RUJA, TEST262

def run_test_capture(path):
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
                return 'pass', out
            return 'fail', out
        return ('pass' if (r.returncode == 0 and not out) else 'fail'), out
    except subprocess.TimeoutExpired:
        return 'timeout', ''
    except Exception as e:
        return 'error', str(e)

def main():
    dirs = sys.argv[1:] if len(sys.argv) > 1 else ['language/statements', 'language/expressions']
    failures = []
    total = 0
    for d in dirs:
        base = Path(TEST262) / 'test' / d
        if not base.exists():
            continue
        for f in sorted(base.rglob('*.js')):
            if '_FIXTURE' in f.name:
                continue
            total += 1
            if total % 500 == 0:
                sys.stderr.write(f"  ...{total}\n")
            result, out = run_test_capture(f)
            if result in ('fail', 'timeout', 'error'):
                rel = str(f.relative_to(Path(TEST262) / 'test'))
                failures.append((rel, result, out[:500]))

    by_dir = defaultdict(list)
    for rel, result, out in failures:
        parts = rel.split('/')
        key = '/'.join(parts[:-1])
        by_dir[key].append((rel, result, out))

    print(f"\n=== {len(failures)} failures across {len(by_dir)} directories ===\n")
    for key, items in sorted(by_dir.items(), key=lambda x: -len(x[1])):
        print(f"[{len(items)}] {key}")

    out_file = '/tmp/test262_failures.json'
    with open(out_file, 'w') as f:
        json.dump(failures, f, indent=2)
    print(f"\nFull details: {out_file}")

if __name__ == '__main__':
    main()
