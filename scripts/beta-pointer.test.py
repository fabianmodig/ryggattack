#!/usr/bin/env python3
"""Exercise beta delivery with real temporary Git remotes and a local PR API."""
import json
import base64
import shutil
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
        self.source_remote = self.root / 'source.git'
        self.source_remote.mkdir()
        git(self.source_remote, 'init', '--bare')
        self.remote = self.root / 'destination.git'
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
        (self.source / 'private-source.txt').write_text('must never be delivered')
        git(self.source, 'add', '.')
        git(self.source, 'commit', '-m', 'private source history')
        self.sha = git(self.source, 'rev-parse', 'HEAD')
        git(self.source, 'remote', 'add', 'origin', str(self.source_remote))
        git(self.source, 'push', 'origin', 'main')
        self.pr = {'state': 'open', 'head': {'sha': self.sha, 'repo': {'full_name': 'owner/game'}},
                   'base': {'repo': {'full_name': 'owner/game'}}}
        self.event = self.root / 'event.json'
        self.event.write_text(json.dumps({'number': 42, 'pull_request': self.pr}))
        self.headers = []
        self.api_status = 200
        self.extra_env = {}
        self.on_get = lambda: None
        self.source_refs = git(self.source_remote, 'show-ref')
        self.source_history = git(self.source_remote, 'rev-list', '--all')
        self.source_config = (self.source / '.git/config').read_bytes()
        def source_unchanged():
            self.assertEqual(git(self.source_remote, 'show-ref'), self.source_refs)
            self.assertEqual(git(self.source_remote, 'rev-list', '--all'), self.source_history)
            self.assertEqual((self.source / '.git/config').read_bytes(), self.source_config)
            self.assertEqual(git(self.source, 'status', '--porcelain'), '')
        self.addCleanup(source_unchanged)
        outer = self
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                outer.headers.append((self.path, self.headers.get('Authorization')))
                outer.on_get()
                self.send_response(outer.api_status)
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
                                   'BETA_DEPLOY_TOKEN': 'destination-secret', 'PR_READ_TOKEN': 'source-read-secret',
                                   'BETA_DEPLOY_REPOSITORY': str(self.remote), 'GITHUB_RUN_ID': '9', **self.extra_env})

    def test_atomic_chart_and_pair_does_not_modify_main(self):
        result = self.run_pointer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(git(self.source_remote, 'rev-parse', 'main'), self.sha)
        self.assertEqual(git(self.remote, 'for-each-ref', '--format=%(refname)'), 'refs/heads/beta-deploy')
        values = git(self.remote, 'show', 'beta-deploy:charts/ryggattack/values.yaml')
        self.assertIn('git.modig.online/ryggattack-beta/ryggattack/web', values)
        self.assertIn('git.modig.online/ryggattack-beta/ryggattack/server', values)
        self.assertIn(self.sha, values)
        self.assertIn('ryggattack-beta.modig.online', values)
        self.assertIn('sha-' + 'b' * 40, git(self.remote, 'show', 'beta-deploy:charts/ryggattack/Chart.yaml'))
        self.assertEqual(git(self.remote, 'show', 'beta-deploy:charts/ryggattack/candidate.txt'), 'chart from PR')
        self.assertEqual(git(self.source, 'status', '--porcelain'), '')
        self.assertEqual(git(self.remote, 'rev-list', '--count', 'beta-deploy'), '1')
        files = git(self.remote, 'ls-tree', '-r', '--name-only', 'beta-deploy').splitlines()
        self.assertTrue(files)
        self.assertTrue(all(f.startswith('charts/ryggattack/') for f in files), files)
        self.assertEqual(git(self.source_remote, 'for-each-ref', '--format=%(refname)'), 'refs/heads/main')
        self.assertNotIn('destination-secret', (self.source / '.git/config').read_text())

    def test_api_uses_source_automatic_read_token_only(self):
        result = self.run_pointer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.headers, [('/repos/owner/game/pulls/42', 'token source-read-secret')] * 2)
        self.assertNotIn('source-read-secret', result.stdout + result.stderr)
        self.assertNotIn('destination-secret', result.stdout + result.stderr)

    def test_initial_concurrent_creation_retries_without_source_ancestry(self):
        competitor = None
        count = 0
        def compete():
            nonlocal competitor, count
            count += 1
            if count != 2:
                return
            other = self.root / 'other'
            other.mkdir()
            git(other, 'init', '-b', 'beta-deploy')
            git(other, 'config', 'user.name', 'Other')
            git(other, 'config', 'user.email', 'other@example.invalid')
            chart = other / 'charts/ryggattack'
            chart.mkdir(parents=True)
            (chart / 'competitor').write_text('initial winner')
            git(other, 'add', '.')
            git(other, 'commit', '-m', 'initial concurrent delivery')
            competitor = git(other, 'rev-parse', 'HEAD')
            git(other, 'push', str(self.remote), 'HEAD:beta-deploy')
        self.on_get = compete
        result = self.run_pointer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(git(self.remote, 'rev-parse', 'beta-deploy^'), competitor)
        self.assertEqual(git(self.remote, 'rev-list', '--count', 'beta-deploy'), '2')
        self.assertEqual(git(self.source_remote, 'rev-parse', 'main'), self.sha)
        self.assertEqual(len(self.headers), 4)

    def test_final_api_failure_keeps_destination_unchanged(self):
        self.assertEqual(self.run_pointer().returncode, 0)
        before = git(self.remote, 'rev-parse', 'beta-deploy')
        count = 0
        def fail_final():
            nonlocal count
            count += 1
            if count == 2:
                self.api_status = 403
        self.on_get = fail_final
        result = self.run_pointer('sha-' + 'f' * 40)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(count, 2)
        self.assertEqual(git(self.remote, 'rev-parse', 'beta-deploy'), before)

    def test_live_fork_rejected_before_push(self):
        self.pr['head']['repo']['full_name'] = 'fork/game'
        result = self.run_pointer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('Skipped', result.stdout)
        self.assertEqual(git(self.remote, 'for-each-ref', '--format=%(refname)'), '')

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
        self.assertEqual(git(self.remote, 'for-each-ref', '--format=%(refname)'), '')

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
        self.assertEqual(git(self.remote, 'for-each-ref', '--format=%(refname)'), '')

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
        self.assertEqual(git(self.source_remote, 'rev-parse', 'main'), self.sha)
        self.assertIn('sha-' + 'e' * 40, git(self.remote, 'show', 'beta-deploy:charts/ryggattack/Chart.yaml'))

    def test_secure_destination_auth_and_remote_readback(self):
        shim = self.root / 'bin'
        shim.mkdir()
        real_git = shutil.which('git')
        audit = self.root / 'audit.jsonl'
        executable = shim / 'git'
        executable.write_text('#!/usr/bin/env python3\n' +
            'import os,sys,json,subprocess\n' +
            'args=sys.argv[1:]\n' +
            "if any(x in args for x in ('ls-remote','fetch','push')):\n" +
            " assert 'destination-secret' not in ' '.join(args)\n" +
            " from pathlib import Path\n" +
            " config=(Path(args[1])/'.git/config').read_text()\n" +
            " assert 'destination-secret' not in config and 'extraheader' not in config.lower()\n" +
            " assert os.environ.get('GIT_CONFIG_KEY_0') == 'http.https://git.modig.online/.extraHeader'\n" +
            " assert os.environ.get('GIT_CONFIG_VALUE_0') == 'Authorization: Basic ' + __import__('base64').b64encode(b'ryggattack-beta:destination-secret').decode()\n" +
            " with open(" + repr(str(audit)) + ", 'a') as f: f.write(json.dumps(args)+'\\n')\n" +
            " if 'push' in args: sys.exit(0) # transport claims success but no ref moved\n" +
            'sys.exit(subprocess.call([' + repr(real_git) + ']+args))\n')
        executable.chmod(0o755)
        self.extra_env['PATH'] = str(shim) + os.pathsep + os.environ['PATH']
        result = self.run_pointer()
        self.assertNotEqual(result.returncode, 0, 'must verify actual remote ref after successful push')
        operations = [json.loads(line) for line in audit.read_text().splitlines()]
        self.assertEqual([a[2] for a in operations], ['ls-remote', 'push', 'ls-remote'])
        self.assertEqual(git(self.remote, 'for-each-ref', '--format=%(refname)'), '')
        self.assertNotIn('destination-secret', result.stdout + result.stderr)
        self.assertNotIn(base64.b64encode(b'ryggattack-beta:destination-secret').decode(), result.stdout + result.stderr)

    def test_api_failure_does_not_change_desired_state(self):
        self.assertEqual(self.run_pointer().returncode, 0)
        before = git(self.remote, 'rev-parse', 'beta-deploy')
        # A malformed live response is not permission to deploy.
        self.pr = {}
        self.assertNotEqual(self.run_pointer('sha-' + 'f' * 40).returncode, 0)
        self.assertEqual(git(self.remote, 'rev-parse', 'beta-deploy'), before)


if __name__ == '__main__':
    unittest.main()
