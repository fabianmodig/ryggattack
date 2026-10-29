#!/usr/bin/env python3
"""Sanitized, independently checkable prediction evidence package (no binaries)."""
import gzip
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parents[1]
package = pathlib.Path(os.environ.get('PREDICTION_EVIDENCE_PACKAGE', root / 'docs/client-prediction-evidence'))

def sha(data):
    return hashlib.sha256(data).hexdigest()

def privacy(data, name):
    text = data.decode('utf-8', errors='strict')
    assert not re.search(r'-----BEGIN [^-]*PRIVATE KEY-----|(?:credential|password|authorization)[\"\s:=]+[a-fA-F0-9]{64}', text, re.I), 'private material: ' + name

if sys.argv[1:] == ['assemble']:
    assert not package.exists(), 'never overwrite historical proof; choose a new package explicitly'
    source = json.loads((root / 'verification/prediction-frozen-source.json').read_text())
    artifact = json.loads((root / 'verification/prediction-artifact.json').read_text())
    producers = sorted((root / 'verification').glob('prediction*.log'))
    producers += sorted((root / 'verification').glob('prediction*.json'))
    producers += [root / 'verification/timings.tsv']
    for directory in sorted((root / 'verification').glob('prediction-browser-*')):
        if directory.is_dir():
            assert (directory / 'results.json').exists(), 'cannot package an active browser run'
            producers += sorted(p for p in directory.rglob('*') if p.is_file())
    originals = {}
    for path in producers:
        data = path.read_bytes()
        if path.suffix != '.png':
            privacy(data, str(path))
        relative = path.relative_to(root / 'verification')
        dest = package / 'records' / relative
        if path.suffix == '.json' and len(data) > 100000:
            dest = dest.with_suffix(dest.suffix + '.gz')
            stored = gzip.compress(data, mtime=0)
        else:
            stored = data
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(stored)
        originals[str(relative)] = {'path': str(dest.relative_to(package)), 'original_sha256': sha(data), 'original_size': len(data), 'gzip': dest.suffix == '.gz'}
    # Archive exact sources as inert text; no Cargo/Node/runtime discovery.
    for name, expected in source['source_sha256'].items():
        data = (root / name).read_bytes()
        assert sha(data) == expected, 'runtime changed before packaging: ' + name
        dest = package / 'source' / (name + '.txt')
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
    helpers = ['scripts/prediction-evidence.py', 'scripts/prediction-evidence.test.py', 'scripts/prediction-provenance.py', 'scripts/prediction-analyze.py', 'scripts/prediction-report.py']
    for name in helpers:
        dest = package / 'verifier' / (name + '.txt')
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes((root / name).read_bytes())
    current = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
    manifest = {'status': 'verification records, not an automatic acceptance verdict', 'source_head_at_assembly': current, 'frozen_source': source, 'artifact': artifact, 'originals': originals, 'screenshots_visually_reviewed': False, 'privacy': 'text and decompressed text scanned for key headers and raw credential assignments; screenshot visual review separate', 'members_sha256': {str(p.relative_to(package)): sha(p.read_bytes()) for p in sorted(package.rglob('*')) if p.is_file()}}
    (package / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print('Assembled', len(manifest['members_sha256']), 'members from', len(originals), 'original records')
else:
    manifest = json.loads((package / 'manifest.json').read_text())
    actual = {str(p.relative_to(package)): sha(p.read_bytes()) for p in package.rglob('*') if p.is_file() and p != package / 'manifest.json'}
    assert actual == manifest['members_sha256'], 'package member missing, changed or unexpected'
    for name, entry in manifest['originals'].items():
        data = (package / entry['path']).read_bytes()
        if entry['gzip']:
            data = gzip.decompress(data)
        assert sha(data) == entry['original_sha256'] and len(data) == entry['original_size'], name
        if not name.endswith('.png'):
            privacy(data, name)
    for name, expected in manifest['frozen_source']['source_sha256'].items():
        assert sha((package / 'source' / (name + '.txt')).read_bytes()) == expected
        if sys.argv[1:] == ['checkout']:
            assert sha((root / name).read_bytes()) == expected, 'checkout source changed: ' + name
    if sys.argv[1:] == ['checkout']:
        for archived in (package / 'verifier/scripts').glob('*.txt'):
            assert archived.read_bytes() == (root / 'scripts' / archived.name.removesuffix('.txt')).read_bytes(), 'verifier changed after assembly'
        artifact = {str(p.relative_to(root / 'dist/web')): sha(p.read_bytes()) for p in (root / 'dist/web').rglob('*') if p.is_file()}
        assert artifact == manifest['artifact']['artifact_sha256'], 'checkout artifact changed'
    log = (package / 'records/prediction-frozen-native.log').read_text()
    count = sum(int(n) for n in re.findall(r'test result: ok\. (\d+) passed;', log))
    print(json.dumps({'package_members_verified': len(actual), 'rust_tests_passed': count, 'checkout_compared': sys.argv[1:] == ['checkout'], 'screen_visual_review_claim': manifest['screenshots_visually_reviewed']}, indent=2))
