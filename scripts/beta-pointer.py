#!/usr/bin/env python3
"""Advance beta-deploy with a non-force, globally serialized Git CAS update.

Call ONLY after both unique beta images were published and all CI gates passed.
No cluster credentials are used. Conflicts retry against the new branch tip.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
import urllib.request

import yaml


def git(source, *args, check=True):
    return subprocess.run(['git', '-C', str(source), *args], check=check,
                          capture_output=True, text=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=Path.cwd())
    parser.add_argument('--event', type=Path, default=os.environ.get('GITHUB_EVENT_PATH'))
    parser.add_argument('--tag', required=True)
    args = parser.parse_args()
    if not re.fullmatch(r'sha-[0-9a-f]{40}', args.tag):
        raise ValueError('beta tag must be a unique 40-hex artifact ID')
    source = args.source.resolve()
    sha = git(source, 'rev-parse', 'HEAD').stdout.strip()
    event = json.loads(args.event.read_text())
    repository = os.environ['GITHUB_REPOSITORY']
    if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository):
        raise ValueError('invalid repository')
    number = int(event['number'])
    pr = event['pull_request']
    if (os.environ['GITHUB_EVENT_NAME'] != 'pull_request' or
            pr['head']['repo']['full_name'] != repository or
            pr['base']['repo']['full_name'] != repository or pr['head']['sha'] != sha):
        raise ValueError('only checked-out same-repository PR heads may deploy')

    def eligible():
        request = urllib.request.Request(
            f"{os.environ['GITHUB_API_URL']}/repos/{repository}/pulls/{number}",
            headers={'Authorization': 'token ' + os.environ['BETA_DEPLOY_TOKEN']})
        with urllib.request.urlopen(request, timeout=20) as response:
            live = json.load(response)
        return (live['state'] == 'open' and live['head']['sha'] == sha and
                live['head']['repo']['full_name'] == repository and
                live['base']['repo']['full_name'] == repository)

    for attempt in range(8):
        exists = git(source, 'ls-remote', '--exit-code', '--heads', 'origin', 'beta-deploy', check=False)
        if exists.returncode not in (0, 2):
            raise RuntimeError('cannot query delivery branch')
        if exists.returncode == 0:
            git(source, 'fetch', '--no-tags', 'origin', 'refs/heads/beta-deploy')
            parent = git(source, 'rev-parse', 'FETCH_HEAD').stdout.strip()
        else:
            parent = sha
        if not eligible():
            print('Skipped: PR closed, untrusted, or head superseded')
            return
        with tempfile.TemporaryDirectory(prefix='ryggattack-beta-') as temp:
            tree = Path(temp) / 'delivery'
            git(source, 'worktree', 'add', '--detach', str(tree), parent)
            try:
                target = tree / 'charts/ryggattack'
                shutil.rmtree(target)
                shutil.copytree(source / 'charts/ryggattack', target)
                chart = target / 'Chart.yaml'
                text, count = re.subn(r'^appVersion:.*$', 'appVersion: ' + args.tag,
                                     chart.read_text(), flags=re.MULTILINE)
                if count != 1:
                    raise ValueError('expected exactly one appVersion')
                chart.write_text(text)
                values_path = target / 'values.yaml'
                values = yaml.safe_load(values_path.read_text())
                image = f'git.modig.online/{repository.lower()}/beta'
                values['image'] = {'web': image + '/web', 'server': image + '/server', 'tag': args.tag}
                values['publicHost'] = 'ryggattack-beta.modig.online'
                values['allowedOrigins'] = 'https://ryggattack-beta.modig.online'
                values['deployment'] = {'sourceSHA': sha, 'pullRequest': str(number),
                                        'runID': os.environ['GITHUB_RUN_ID']}
                values_path.write_text(yaml.safe_dump(values, sort_keys=False))
                git(tree, 'add', '--all', 'charts/ryggattack')
                if git(tree, 'diff', '--cached', '--quiet', check=False).returncode == 0:
                    print('Beta desired state already current')
                    return
                git(tree, '-c', 'user.name=Ryggattack beta', '-c',
                    'user.email=ryggattack-beta@modig.online', 'commit', '-m',
                    f'beta: PR #{number} {sha} {args.tag} [skip ci]')
                if not eligible():
                    print('Skipped: PR head superseded before pointer update')
                    return
                # Never force. Successful pushes form one linear global history,
                # including the initial branch-creation race.
                pushed = git(tree, 'push', 'origin', 'HEAD:refs/heads/beta-deploy', check=False)
                if pushed.returncode == 0:
                    print(f'Beta desired state: PR #{number} {sha} {args.tag}')
                    return
            finally:
                git(source, 'worktree', 'remove', '--force', str(tree))
        time.sleep(min(attempt + 1, 5))
    raise RuntimeError('beta pointer not updated after 8 non-force attempts')


if __name__ == '__main__':
    main()
