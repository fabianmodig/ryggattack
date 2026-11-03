#!/usr/bin/env python3
"""Advance beta-deploy with a non-force, globally serialized Git CAS update.

Call ONLY after both unique beta images were published and all CI gates passed.
No cluster credentials are used. Conflicts retry against the new branch tip.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
import sys
import urllib.request

import yaml


DESTINATION = 'https://git.modig.online/fabianmodig/ryggattack-beta-deploy.git'
REF = 'refs/heads/beta-deploy'


def git(source, *args, check=True, env=None):
    result = subprocess.run(['git', '-C', str(source), *args], env=env,
                            capture_output=True, text=True)
    if check and result.returncode:
        # Never expose Git diagnostics: remote servers can echo credentials.
        raise RuntimeError('Git operation failed (diagnostics suppressed)')
    return result


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
        sys.exit('Only checked-out same-repository PR heads may deploy')

    def eligible():
        request = urllib.request.Request(
            f"{os.environ['GITHUB_API_URL']}/repos/{repository}/pulls/{number}",
            headers={'Authorization': 'token ' + os.environ['PR_READ_TOKEN']})
        with urllib.request.urlopen(request, timeout=20) as response:
            live = json.load(response)
        return (live['state'] == 'open' and live['head']['sha'] == sha and
                live['head']['repo']['full_name'] == repository and
                live['base']['repo']['full_name'] == repository)

    # The credential lives only in subprocess environments, never argv, URLs,
    # checkout config, or disk. Do not copy source checkout auth to this repo.
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(('GIT_CONFIG_', 'GIT_TRACE', 'GIT_CURL_'))}
    credential = base64.b64encode(
        ('ryggattack-beta:' + os.environ['BETA_DEPLOY_TOKEN']).encode()).decode()
    env.update({'GIT_CONFIG_COUNT': '1',
                'GIT_CONFIG_KEY_0': 'http.https://git.modig.online/.extraHeader',
                'GIT_CONFIG_VALUE_0': 'Authorization: Basic ' + credential,
                'GIT_TERMINAL_PROMPT': '0'})
    destination = os.environ.get('BETA_DEPLOY_REPOSITORY', DESTINATION)
    # Only the exact isolated destination or a local regression fixture is valid.
    if destination != DESTINATION and not Path(destination).is_absolute():
        raise ValueError('invalid isolated delivery repository')

    for attempt in range(8):
        with tempfile.TemporaryDirectory(prefix='ryggattack-beta-') as temp:
            tree = Path(temp)
            git(tree, 'init', '-b', 'beta-deploy')
            git(tree, 'remote', 'add', 'beta', destination)

            def delivery(*command, check=True):
                return git(tree, *command, check=check, env=env)

            def remote_tip():
                result = delivery('ls-remote', '--exit-code', '--heads', 'beta', REF,
                                  check=False)
                if result.returncode == 2:
                    return None
                if result.returncode:
                    raise RuntimeError('cannot query delivery branch')
                lines = result.stdout.splitlines()
                if len(lines) != 1 or lines[0].split()[1] != REF:
                    raise RuntimeError('invalid delivery ref response')
                tip = lines[0].split()[0]
                if not re.fullmatch(r'[0-9a-f]{40}', tip):
                    raise RuntimeError('invalid delivery commit')
                return tip

            parent = remote_tip()
            if parent:
                delivery('fetch', '--no-tags', 'beta', REF)
                delivery('checkout', '--detach', 'FETCH_HEAD')
            # With no parent this fresh init has an unborn/orphan branch: app
            # source commits and other files can never enter destination history.
            if not eligible():
                print('Skipped: PR closed, untrusted, or head superseded')
                return
            target = tree / 'charts/ryggattack'
            shutil.rmtree(target, ignore_errors=True)
            shutil.copytree(source / 'charts/ryggattack', target)
            chart = target / 'Chart.yaml'
            text, count = re.subn(r'^appVersion:.*$', 'appVersion: ' + args.tag,
                                 chart.read_text(), flags=re.MULTILINE)
            if count != 1:
                raise ValueError('expected exactly one appVersion')
            chart.write_text(text)
            values_path = target / 'values.yaml'
            values = yaml.safe_load(values_path.read_text())
            image = 'git.modig.online/ryggattack-beta/ryggattack'
            values['image'] = {'web': image + '/web', 'server': image + '/server', 'tag': args.tag}
            values['publicHost'] = 'ryggattack-beta.modig.online'
            values['allowedOrigins'] = 'https://ryggattack-beta.modig.online'
            values['deployment'] = {'sourceSHA': sha, 'pullRequest': str(number),
                                    'runID': os.environ['GITHUB_RUN_ID']}
            values_path.write_text(yaml.safe_dump(values, sort_keys=False))
            git(tree, 'add', '--all', 'charts/ryggattack')
            if git(tree, 'diff', '--cached', '--quiet', check=False).returncode == 0:
                if remote_tip() != git(tree, 'rev-parse', 'HEAD').stdout.strip():
                    raise RuntimeError('delivery ref changed during read-back')
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
            pushed = delivery('push', 'beta', 'HEAD:' + REF, check=False)
            if pushed.returncode == 0:
                if remote_tip() != git(tree, 'rev-parse', 'HEAD').stdout.strip():
                    raise RuntimeError('delivery ref changed during read-back')
                print(f'Beta desired state: PR #{number} {sha} {args.tag}')
                return
        time.sleep(min(attempt + 1, 5))
    raise RuntimeError('beta pointer not updated after 8 non-force attempts')


if __name__ == '__main__':
    try:
        main()
    except Exception:
        sys.exit('Beta publication failed (private diagnostics suppressed)')
