#!/usr/bin/env python3
"""Analyze test262 failures: dump failing test paths grouped by directory."""
import json
import sys
from collections import defaultdict
from pathlib import Path

# Reuse the analyzer's canonical execution path so variants, modules, and async
# completion are classified identically in both reports.
sys.path.insert(0, str(Path(__file__).parent))
from test262_analyze import TEST262, run_test as run_test_capture

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
                failures.append((rel, result, out[:2000]))

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
