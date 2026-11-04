#!/usr/bin/env python3
"""Guard exact-head backend artifact transfer without rebuilding in beta."""
from pathlib import Path
import subprocess
import tempfile
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[1]
TRUSTED_PR = "github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name == github.repository"


def workflow():
    return yaml.load((ROOT / '.forgejo/workflows/ci.yml').read_text(), Loader=yaml.BaseLoader)


class BackendArtifactContract(unittest.TestCase):
    def test_web_builds_backend_once_for_trusted_prs(self):
        steps = workflow()['jobs']['web']['steps']
        producers = [s for s in steps if s.get('name') == 'Build beta backend artifact']
        self.assertEqual(len(producers), 1, 'web must produce the exact-head backend artifact')
        producer = producers[0]
        self.assertEqual(producer.get('if'), TRUSTED_PR)
        self.assertIn('cargo build --locked --release --bin ryggattack-server', producer['run'])
        self.assertIn('cp target/release/ryggattack-server dist/server/', producer['run'])
        self.assertIn('git rev-parse HEAD > dist/server/source-sha.txt', producer['run'])
        self.assertIn('sha256sum ryggattack-server > SHA256SUMS', producer['run'])
        dependencies = next(s for s in steps if s.get('name') == 'Install beta backend build dependencies')
        self.assertEqual(dependencies.get('if'), TRUSTED_PR)
        self.assertIn('libudev-dev libwayland-dev', dependencies['run'])
        self.assertLess(steps.index(dependencies), steps.index(producer))

    def test_current_run_artifact_is_verified_before_packaging(self):
        jobs = workflow()['jobs']
        uploads = [s for s in jobs['web']['steps']
                   if 'upload-artifact@v3' in s.get('uses', '')
                   and s.get('with', {}).get('name') == 'beta-server']
        downloads = [s for s in jobs['beta']['steps']
                     if 'download-artifact@v3' in s.get('uses', '')
                     and s.get('with', {}).get('name') == 'beta-server']
        self.assertEqual(len(uploads), 1, 'backend artifact must be uploaded')
        self.assertEqual(len(downloads), 1, 'beta must download the backend from this run')
        self.assertEqual(uploads[0].get('if'), TRUSTED_PR)
        self.assertEqual(uploads[0]['with']['path'], 'dist/server')
        self.assertEqual(uploads[0]['with']['if-no-files-found'], 'error')
        self.assertEqual(uploads[0]['with']['retention-days'], '2')
        producer = next(s for s in jobs['web']['steps'] if s.get('name') == 'Build beta backend artifact')
        self.assertLess(jobs['web']['steps'].index(producer), jobs['web']['steps'].index(uploads[0]))
        self.assertEqual(downloads[0]['with'], {'name': 'beta-server', 'path': 'dist/server'})
        steps = jobs['beta']['steps']
        verify = next(s for s in steps if s.get('name') == 'Verify backend artifact')
        self.assertIn('test "$(cat dist/server/source-sha.txt)" = "$(git rev-parse HEAD)"', verify['run'])
        self.assertIn('sha256sum --check SHA256SUMS', verify['run'])
        packaging = next(s for s in steps if 'docker build' in s.get('run', ''))
        self.assertLess(steps.index(downloads[0]), steps.index(verify))
        self.assertLess(steps.index(verify), steps.index(packaging))
        self.assertFalse(any('rust-toolchain' in s.get('uses', '') or
                             'rust-cache' in s.get('uses', '') for s in steps))
        commands = '\n'.join(s.get('run', '') for s in steps)
        self.assertNotIn('cargo ', commands)
        self.assertNotIn('target/release', commands)

    def test_exact_workflow_verifier_rejects_stale_tampered_and_missing_artifacts(self):
        steps = workflow()['jobs']['beta']['steps']
        script = next(s['run'] for s in steps if s.get('name') == 'Verify backend artifact')
        for case in ('valid', 'stale-head', 'tampered-binary', 'missing-binary', 'missing-checksum'):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                def run(command):
                    return subprocess.run(command, cwd=root, check=True,
                                          capture_output=True, text=True)
                run(['git', 'init', '-q'])
                run(['git', '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                     'commit', '--allow-empty', '-qm', 'artifact verifier fixture'])
                artifact = root / 'dist/server'
                artifact.mkdir(parents=True)
                # Deliberate test bytes: this test verifies transfer integrity,
                # not compilation or execution of a real server binary.
                (artifact / 'ryggattack-server').write_bytes(b'backend integrity fixture\n')
                head = run(['git', 'rev-parse', 'HEAD']).stdout
                (artifact / 'source-sha.txt').write_text(head)
                run(['bash', '-e', '-c', '(cd dist/server && sha256sum ryggattack-server > SHA256SUMS)'])
                if case == 'stale-head':
                    (artifact / 'source-sha.txt').write_text('0' * 40 + '\n')
                elif case == 'tampered-binary':
                    (artifact / 'ryggattack-server').write_bytes(b'changed fixture\n')
                elif case == 'missing-binary':
                    (artifact / 'ryggattack-server').unlink()
                elif case == 'missing-checksum':
                    (artifact / 'SHA256SUMS').unlink()
                result = subprocess.run(['bash', '-e', '-o', 'pipefail', '-c', script],
                                        cwd=root, capture_output=True, text=True)
                if case == 'valid':
                    self.assertEqual(result.returncode, 0, result.stderr)
                else:
                    self.assertNotEqual(result.returncode, 0, 'bad artifact must fail closed')


if __name__ == '__main__':
    unittest.main()
