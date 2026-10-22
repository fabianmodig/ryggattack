#!/usr/bin/env python3
"""Offline proof-package integrity checks; never runs a game or acceptance test."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent
EXCLUDED = {'manifest.json', 'SHA256SUMS'}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_digest(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def target(relative):
    candidate = ROOT / relative
    assert not candidate.is_symlink(), f'Symlink forbidden: {relative}'
    assert candidate.resolve().is_relative_to(ROOT), relative
    return candidate


def original(relative):
    path = target(relative)
    if path.is_file():
        return path.read_bytes()
    compressed = target(relative + '.gz')
    assert compressed.is_file(), f'Missing original: {relative}'
    return gzip.decompress(compressed.read_bytes())


def load(relative):
    return json.loads(original(relative))


def refresh():
    entries = []
    for path in sorted(ROOT.rglob('*')):
        if not path.is_file():
            continue
        relative = path.relative_to(ROOT).as_posix()
        if relative in EXCLUDED:
            continue
        assert not path.is_symlink(), relative
        entries.append({'file': relative, 'bytes': path.stat().st_size,
                        'sha256': file_digest(path)})
    manifest = {'schema': 1,
                'scope': 'Packaging integrity only; not acceptance execution metadata',
                'excludes': sorted(EXCLUDED),
                'file_count': len(entries),
                'bytes': sum(item['bytes'] for item in entries),
                'files': entries}
    (ROOT / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    paths = sorted([item['file'] for item in entries] + ['manifest.json'])
    (ROOT / 'SHA256SUMS').write_text(''.join(
        f'{file_digest(target(name))}  {name}\n' for name in paths))


def verify(repo=None):
    manifest = load('manifest.json')
    files = manifest['files']
    assert len(files) == manifest['file_count']
    expected_names = {item['file'] for item in files}
    assert len(expected_names) == len(files)
    actual_names = {p.relative_to(ROOT).as_posix() for p in ROOT.rglob('*')
                    if p.is_file()} - EXCLUDED
    assert actual_names == expected_names, {
        'unexpected': sorted(actual_names - expected_names),
        'missing': sorted(expected_names - actual_names)}
    for item in files:
        path = target(item['file'])
        assert path.stat().st_size == item['bytes'], item['file']
        assert file_digest(path) == item['sha256'], item['file']
    assert sum(item['bytes'] for item in files) == manifest['bytes']
    checksum_names = set()
    for line in target('SHA256SUMS').read_text().splitlines():
        expected, name = line.split('  ', 1)
        assert file_digest(target(name)) == expected, name
        assert name not in checksum_names, name
        checksum_names.add(name)
    assert checksum_names == expected_names | {'manifest.json'}
    retained = load('retained-originals.json')
    for item in retained:
        stored = target(item['file']).read_bytes()
        data = gzip.decompress(stored) if item['encoding'] == 'gzip' else stored
        assert len(data) == item['original_bytes'], item['file']
        assert digest(data) == item['original_sha256'], item['file']
    runs = load('runs/index.json')
    assert len(runs) == 6
    assert [r['status'] for r in runs].count('failed') == 5
    assert [r['status'] for r in runs].count('passed') == 1
    passing = None
    for run in runs:
        prefix = 'runs/' + run['run'] + '/'
        result = load(prefix + 'results.json')
        assert result['status'] == run['status'], prefix
        assert result['total_ms'] == run['report_total_ms'], prefix
        for item in load(prefix + 'sha256.json'):
            assert digest(original(prefix + item['file'])) == item['sha256'], prefix + item['file']
        archived = target(prefix + 'executed-source-manifest.json')
        if archived.exists():
            for name, expected in json.loads(archived.read_text()).items():
                archive_name = prefix + 'executed-source/' + Path(name).name
                assert digest(original(archive_name)) == expected, archive_name
        if result['status'] == 'passed':
            passing = result
            assert result['errors'] == []
            assert result['private_material_negative_scan']['passed'] is True
            assert result['preflight']['completed'] and result['natural']['completed']
            assert result['preflight']['target'] == 7 and result['natural']['target'] == 1
            for shot in result['screenshots']:
                assert digest(original(prefix + shot['file'])) == shot['sha256']
    assert passing is not None
    source_checks = load('pass-source-verification.json')
    assert len(source_checks['checks']) == len(passing['hashes']) == 104
    for item in source_checks['checks']:
        name = item['file']
        assert item['expected_sha256'] == item['actual_sha256'] == passing['hashes'][name]['sha256'], name
        assert item['bytes'] == passing['hashes'][name]['bytes'], name
        if name.startswith(('scripts/', 'src/', 'web/')) or name in ('Cargo.lock', 'Cargo.toml'):
            data = original('source-at-pass/' + name)
            assert digest(data) == item['expected_sha256'] and len(data) == item['bytes'], name
        if repo is not None:
            path = repo / name
            assert not Path(name).is_absolute() and '..' not in Path(name).parts, name
            # The original worktree uses a shared target/ symlink for native outputs.
            assert (path.resolve().is_relative_to(repo.resolve())
                    or name.startswith('target/')), name
            assert file_digest(path) == item['expected_sha256'], name
            assert path.stat().st_size == item['bytes'], name
    visited = set()
    pending = ['scripts/online-hud-smoke.mjs']
    while pending:
        name = pending.pop()
        if name in visited:
            continue
        visited.add(name)
        source = original('source-at-pass/' + name).decode()
        for spec in re.findall(r'''(?:from\s*|import\s*)['"]([^'"]+)['"]''', source):
            if spec.startswith('.'):
                path = (ROOT / 'source-at-pass' / name).parent / spec
                local = path.resolve().relative_to(ROOT / 'source-at-pass').as_posix()
                assert local in passing['hashes'], local
                pending.append(local)
    assert sorted(visited) == source_checks['local_harness_import_closure']
    before = load('provenance/hud-runtime-source-before-build.json')
    built = load('provenance/hud-built-artifact.json')
    assert before == built['source_state'] and built['runtime_sources_unchanged'] is True
    for name, expected in before['files'].items():
        recorded = passing['hashes'].get(name)
        if recorded is None and name.startswith('assets/'):
            recorded = passing['hashes']['dist/web/' + name]
        assert recorded and recorded['sha256'] == expected, name
    for name, item in built['artifacts'].items():
        assert passing['hashes']['dist/web/' + name] == item, name
    native = original('logs/hud-native-final.log').decode()
    native_passed = sum(map(int, re.findall(r'test result: ok\. (\d+) passed;', native)))
    assert native_passed == 201, native_passed
    node = original('logs/hud-all-node-startup-fixed.log').decode()
    assert '# tests 122\n' in node and '# pass 122\n' in node and '# fail 0\n' in node
    whitespace = load('whitespace-findings.json')
    assert whitespace['new_authored_file_findings'] == []
    all_paths = [p for p in ROOT.rglob('*') if p.is_file()]
    summary = {'status': 'verified', 'package_file_count': len(all_paths),
               'package_bytes': sum(p.stat().st_size for p in all_paths),
               'manifest_entries': len(files), 'retained_originals': len(retained),
               'actual_runs': len(runs), 'failed_actual_runs': 5,
               'passing_run_hashes_checked': len(source_checks['checks']),
               'local_harness_sources': len(visited), 'native_tests': native_passed,
               'node_tests': 122, 'current_repo_rechecked': repo is not None,
               'manifest_sha256': file_digest(ROOT / 'manifest.json'),
               'sha256sums_sha256': file_digest(ROOT / 'SHA256SUMS')}
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path)
    parser.add_argument('--refresh-manifest', action='store_true',
                        help='Deliberately refresh package hashes; not acceptance execution')
    args = parser.parse_args()
    if args.refresh_manifest:
        refresh()
    verify(args.repo)
