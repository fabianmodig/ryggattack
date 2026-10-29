#!/usr/bin/env python3
"""Check that retained evidence rejects changed, missing and extra members."""
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

root = pathlib.Path(__file__).resolve().parents[1]
source = root / 'docs/client-prediction-evidence'
with tempfile.TemporaryDirectory(prefix='prediction-package-negative-', dir=root / 'verification') as directory:
    package = pathlib.Path(directory) / 'package'
    def run():
        result = subprocess.run([sys.executable, str(root / 'scripts/prediction-evidence.py')], cwd=root, env=os.environ | {'PREDICTION_EVIDENCE_PACKAGE':str(package)}, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        return result.returncode
    shutil.copytree(source, package)
    assert run() == 0, 'unmodified package fails'
    extra = package / 'records/manifest.json'
    extra.write_text('{}')
    assert run() != 0, 'unexpected nested manifest escapes inventory'
    extra.unlink()
    target = next((package / 'source').rglob('*.txt'))
    original = target.read_bytes()
    target.write_bytes(original + b'\nmodified\n')
    assert run() != 0, 'modified source accepted'
    target.write_bytes(original)
    target.unlink()
    assert run() != 0, 'missing source accepted'
    target.write_bytes(original)
    manifest = package / 'manifest.json'
    data = json.loads(manifest.read_text())
    first = next(iter(data['members_sha256']))
    data['members_sha256'][first] = '0' * 64
    manifest.write_text(json.dumps(data))
    assert run() != 0, 'modified manifest checksum accepted'
print('PASS: valid package; reject extra, changed, missing members and modified manifest checksum')
