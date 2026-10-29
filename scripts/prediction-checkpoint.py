#!/usr/bin/env python3
"""Preserve sanitized, incomplete native verification for a continuation run."""
import hashlib
import json
import pathlib
import re
import subprocess

root = pathlib.Path(__file__).resolve().parents[1]
files = sorted((root / 'verification').glob('prediction*.log'))
files += [root / 'verification/timings.tsv', root / 'verification/prediction-baseline.json']
for file in files:
    text = file.read_text()
    if re.search(r'-----BEGIN .*PRIVATE KEY-----|(?:credential|password|authorization)[\"\s:=]+[a-fA-F0-9]{64}', text, re.I):
        raise SystemExit(f'potential private material in {file.name}; do not publish')

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

runtime = subprocess.check_output(['git', 'ls-files', 'src', 'web', 'assets', 'Cargo.toml', 'Cargo.lock', 'scripts/build-web.sh'], cwd=root, text=True).splitlines()
log = (root / 'verification/prediction-current-cargo-test.log').read_text()
counts = [int(n) for n in re.findall(r'test result: ok\. (\d+) passed;', log)]
result = {
    'task': 't_4de42a24',
    'base': 'a48fbf5',
    'status': 'INCOMPLETE_NATIVE_CHECKPOINT_NOT_BROWSER_ACCEPTANCE_OR_PR_READY',
    'source_parent': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
    'source_state': 'runtime hashes below identify the tested dirty source; a later commit does not imply a new execution',
    'rust_tests_passed': sum(counts),
    'rust_test_groups': counts,
    'node_tests_passed': 163,
    'runtime_sha256': {name: digest(root / name) for name in runtime},
    'evidence_sha256': {str(file.relative_to(root)): digest(file) for file in files},
    'privacy_scan': 'selected textual logs scanned for private-key headers and raw 64-hex credential assignments; no screenshots produced',
    'remaining': [
        'Full prediction acceptance matrix, exact timing/ack alignment under asymmetric delays and batching',
        'Harden missing-history/catch-up and lifecycle/route/collision/speed edge cases beyond initial tests',
        'Baseline-aware Clippy comparison: current pass has warnings, including one new test style warning',
        'WASM build and exact-artifact two-rendered-client zero/delay/jitter evidence using real menus/keys',
        'Actual shared adapter scheduling and full browser lifecycle/collision/junction/respawn coverage',
        'Offline graphical replay on same artifact; native graphical testing remains deferred',
        'Helm/chart Python CI gates and final sanitized evidence package',
        'Forgejo PR, exact-head CI, independent native reviewer handoff; no merge/deployment authorized'
    ]
}
path = root / 'verification/prediction-checkpoint.json'
path.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'status': result['status'], 'rust_tests_passed': sum(counts), 'native_evidence_files': len(files), 'checkpoint': str(path)}, indent=2))
print('SAFE_EVIDENCE_PATHS')
for file in files:
    print(file.relative_to(root))
