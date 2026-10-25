"""Verify the archived proof and its exact runtime source in disk/index/HEAD.

Run from any directory: python3 docs/visual-fidelity-evidence/verify.py [disk|index|HEAD]
No build, renderer, network, extraction, or production action is performed.
"""
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[2]
PREFIX = 'docs/visual-fidelity-evidence/'
MODE = sys.argv[1] if len(sys.argv) > 1 else 'disk'
assert MODE in ('disk', 'index', 'HEAD'), MODE


def read(path):
    if MODE == 'disk':
        return (ROOT / path).read_bytes()
    ref = ':' + path if MODE == 'index' else 'HEAD:' + path
    return subprocess.check_output(['git', 'show', ref], cwd=ROOT)


def sha(data):
    return hashlib.sha256(data).hexdigest()


scanned = 0
key_header = re.compile(rb'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----')


def member_bytes(archive, member):
    stream = archive.extractfile(member)
    assert stream is not None, member
    return stream.read()


def scan(name, data, depth=0):
    global scanned
    scanned += 1
    assert not key_header.search(data), 'private-key signature: ' + name
    if name.endswith('.tar.gz'):
        assert depth < 3, name
        with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as archive:
            names = archive.getnames()
            assert len(names) == len(set(names)), 'duplicate archive path'
            for member in archive.getmembers():
                assert member.isfile(), member.name
                assert not member.name.startswith('/') and '..' not in Path(member.name).parts
                scan(name + '!' + member.name, member_bytes(archive, member), depth + 1)


payload = json.loads(read(PREFIX + 'payload-sha256.json'))
for path, expected in payload.items():
    data = read(PREFIX + path)
    assert sha(data) == expected, path
    scan(path, data)
receipt = json.loads(read(PREFIX + 'build-receipt.json'))
for path, expected in receipt['sources'].items():
    assert sha(read(path)) == expected, 'tested source mismatch: ' + path
archive_bytes = read(PREFIX + 'final-acceptance-evidence.tar.gz')
assert sha(archive_bytes) == '669c38249e8623c0857eeee1667fea1b02fb322df5dba242372125be55ebe901'
with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode='r:gz') as archive:
    manifest_path = 'verification/final-acceptance/package-manifest.json'
    manifest = json.loads(member_bytes(archive, manifest_path))
    assert set(archive.getnames()) == set(manifest) | {manifest_path}
    for path, expected in manifest.items():
        assert sha(member_bytes(archive, path)) == expected, path
    embedded = json.loads(member_bytes(archive, 'verification/final-acceptance/build-20261025T124108Z/result.json'))
    assert embedded == receipt
review = json.loads(read(PREFIX + 'reviewer-execution/result.json'))
assert review['status'] == 'passed' and review['source_native_artifact_hashes_unchanged']
assert review['archive_sha256'] == sha(archive_bytes)
assert review['native_passed'] == 216 and review['node_passed'] == 168
print(json.dumps({'status': 'passed', 'mode': MODE, 'source_hashes': len(receipt['sources']),
                  'payload_hashes': len(payload), 'archive_members': len(manifest) + 1,
                  'signature_scanned_members': scanned, 'archive_sha256': sha(archive_bytes)}, indent=2))
