#!/usr/bin/env python3
"""Exercise beta delivery with real temporary Git remotes and a local PR API."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import unittest

SCRIPT = Path(__file__).with_name('beta-pointer.py')


def git(path, *args):
    return subprocess.check_output(['git', '-C', str(path), *args], text=True, stderr=subprocess.DEVNULL).strip()


class PointerTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.remote = self.root / 'remote.git'
        self.source = self.root / 'source'
        self.remote.mkdir()
        git(self.remote, 'init', '--bare')
        self.source.mkdir()
        git(self.source, 'init', '-b', 'main')
        git(self.source, 'config', 'user.name', 'Test')
        git(self.source, 'config', 'user.email', 'test@example.invalid')
        chart = self.source / 'charts/ryggattack'
        chart.mkdir(parents=True)
        (chart / 'Chart.yaml').write_text('apiVersion: v2\nname: ryggattack\nversion: 0.1.0\nappVersion: sha-1234567\n')
        (chart / 'values.yaml').write_text('image: {web: prod/web, server: prod/server, tag: ""}\n')
        (chart / 'candidate.txt').write_text('chart from PR\n')
        git(self.source, 'add', '.')
        git(self.source, 'commit', '-m', 'source')
        self.sha = git(self.source, 'rev-parse', 'HEAD')
        git(self.source, 'remote', 'add', 'origin', str(self.remote))
        git(self.source, 'push', 'origin', 'main')
        self.pr = {'state': 'open', 'head': {'sha': self.sha, 'repo': {'full_name': 'owner/game'}},
                   'base': {'repo': {'full_name': 'owner/game'}}}
        self.event = self.root / 'event.json'
        self.event.write_text(json.dumps({'number': 42, 'pull_request': self.pr}))
        self.on_get = lambda: None
        outer = self
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                outer.on_get()
                self.send_response(200)
                self.end_headers()
                self.wfile.write(json.dumps(outer.pr).encode())
            def log_message(self, format, *args):
                pass
        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.addCleanup(self.server.server_close)
        self.addCleanup(self.server.shutdown)
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def run_pointer(self, tag='sha-' + 'b' * 40):
        self.assertTrue(SCRIPT.exists(), 'missing beta pointer publisher')
        return subprocess.run(['python3', str(SCRIPT), '--source', str(self.source),
                               '--event', str(self.event), '--tag', tag], text=True, capture_output=True,
                              env={**os.environ, 'GITHUB_REPOSITORY': 'owner/game', 'GITHUB_EVENT_NAME': 'pull_request',
                                   'GITHUB_API_URL': f'http://127.0.0.1:{self.server.server_port}',
                                   'BETA_DEPLOY_TOKEN': 'local-test', 'GITHUB_RUN_ID': '9'})

    def test_atomic_chart_and_pair_does_not_modify_main(self):
        result = self.run_pointer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(git(self.remote, 'rev-parse', 'main'), self.sha)
        values = git(self.remote, 'show', 'beta-deploy:charts/ryggattack/values.yaml')
        self.assertIn('git.modig.online/owner/game/beta/web', values)
        self.assertIn('git.modig.online/owner/game/beta/server', values)
        self.assertIn(self.sha, values)
        self.assertIn('ryggattack-beta.modig.online', values)
        self.assertIn('sha-' + 'b' * 40, git(self.remote, 'show', 'beta-deploy:charts/ryggattack/Chart.yaml'))
        self.assertEqual(git(self.remote, 'show', 'beta-deploy:charts/ryggattack/candidate.txt'), 'chart from PR')
        self.assertEqual(git(self.source, 'status', '--porcelain'), '')

    def test_stale_or_closed_pr_retains_previous_state(self):
        self.assertEqual(self.run_pointer().returncode, 0)
        before = git(self.remote, 'rev-parse', 'beta-deploy')
        self.pr['head']['sha'] = 'c' * 40
        result = self.run_pointer('sha-' + 'd' * 40)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('Skipped', result.stdout)
        self.assertEqual(git(self.remote, 'rev-parse', 'beta-deploy'), before)
        self.pr['head']['sha'] = self.sha
        self.pr['state'] = 'closed'
        self.assertEqual(self.run_pointer().returncode, 0)
        self.assertEqual(git(self.remote, 'rev-parse', 'beta-deploy'), before)

    def test_fork_event_never_creates_delivery_branch(self):
        event = json.loads(self.event.read_text())
        event['pull_request']['head']['repo']['full_name'] = 'fork/game'
        self.event.write_text(json.dumps(event))
        result = self.run_pointer()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('same-repository', result.stderr)
        self.assertEqual(git(self.remote, 'for-each-ref', '--format=%(refname)'), 'refs/heads/main')

    def test_head_rechecked_immediately_before_push(self):
        count = 0
        def supersede():
            nonlocal count
            count += 1
            if count == 2:
                self.pr['head']['sha'] = 'c' * 40
        self.on_get = supersede
        result = self.run_pointer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('superseded before', result.stdout)
        self.assertEqual(git(self.remote, 'for-each-ref', '--format=%(refname)'), 'refs/heads/main')

    def test_non_force_conflict_retries_on_global_branch_tip(self):
        self.assertEqual(self.run_pointer().returncode, 0)
        mutator = self.root / 'mutator'
        subprocess.run(['git', 'clone', '--branch', 'beta-deploy', str(self.remote), str(mutator)],
                       check=True, capture_output=True)
        git(mutator, 'config', 'user.name', 'Concurrent publisher')
        git(mutator, 'config', 'user.email', 'other@example.invalid')
        competitor = None
        def compete():
            nonlocal competitor
            if competitor is None:
                (mutator / 'charts/ryggattack/concurrent.txt').write_text('another successful candidate')
                git(mutator, 'add', '.')
                git(mutator, 'commit', '-m', 'concurrent pointer')
                competitor = git(mutator, 'rev-parse', 'HEAD')
                git(mutator, 'push', 'origin', 'HEAD:beta-deploy')
        self.on_get = compete
        result = self.run_pointer('sha-' + 'e' * 40)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(git(self.remote, 'rev-parse', 'beta-deploy^'), competitor)
        self.assertEqual(git(self.remote, 'rev-parse', 'main'), self.sha)
        self.assertIn('sha-' + 'e' * 40, git(self.remote, 'show', 'beta-deploy:charts/ryggattack/Chart.yaml'))

    def test_api_failure_does_not_change_desired_state(self):
        self.assertEqual(self.run_pointer().returncode, 0)
        before = git(self.remote, 'rev-parse', 'beta-deploy')
        # A malformed live response is not permission to deploy.
        self.pr = {}
        self.assertNotEqual(self.run_pointer('sha-' + 'f' * 40).returncode, 0)
        self.assertEqual(git(self.remote, 'rev-parse', 'beta-deploy'), before)


if __name__ == '__main__':
    unittest.main()
