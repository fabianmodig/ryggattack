#!/usr/bin/env python3
"""Bind local compiler/browser evidence to exact source and artifact bytes."""
import hashlib
import json
import pathlib
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parents[1]
manifest = root / 'verification/prediction-frozen-source.json'

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def sources():
    paths = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '--', 'src', 'web', 'assets', 'Cargo.toml', 'Cargo.lock', 'scripts/build-web.sh', 'scripts/wasm-bindgen-version.sh'], cwd=root, text=True).splitlines()
    return {name: digest(root / name) for name in sorted(set(paths))}

if sys.argv[1:] == ['record']:
    data = {'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(), 'source_sha256': sources(), 'state': 'exact hashes include current uncommitted changes; head alone is not provenance'}
    if manifest.exists():
        raise SystemExit('refuse to overwrite frozen source evidence')
    manifest.write_text(json.dumps(data, indent=2) + '\n')
    print('Recorded', len(data['source_sha256']), 'source paths')
else:
    data = json.loads(manifest.read_text())
    assert data['source_sha256'] == sources(), 'source changed since freeze'
    artifact = {str(p.relative_to(root / 'dist/web')): digest(p) for p in sorted((root / 'dist/web').rglob('*')) if p.is_file()}
    assert artifact and 'ryggattack_bg.wasm' in artifact
    path = root / 'verification/prediction-artifact.json'
    if sys.argv[1:] == ['artifact']:
        assert not path.exists(), 'do not relabel an existing artifact'
        path.write_text(json.dumps({'source_manifest_sha256': digest(manifest), 'artifact_sha256': artifact}, indent=2) + '\n')
    else:
        expected = json.loads(path.read_text())
        assert expected['source_manifest_sha256'] == digest(manifest)
        assert expected['artifact_sha256'] == artifact, 'artifact changed after build'
    print('Verified', len(data['source_sha256']), 'source paths and', len(artifact), 'artifact paths')
