#!/usr/bin/env python3
"""Beta is gated by all PR checks, artifact reuse, and same-repo trust."""
from pathlib import Path
import yaml

workflow = yaml.load(Path('.forgejo/workflows/ci.yml').read_text(), Loader=yaml.BaseLoader)
assert 'pull_request_target' not in workflow['on']
assert 'permissions' not in workflow
jobs = workflow['jobs']
assert 'beta' in jobs, 'missing gated beta publication job'
beta = jobs['beta']
assert set(beta['needs']) == {'test', 'chart', 'web'}
assert "github.event_name == 'pull_request'" in beta['if']
assert 'github.event.pull_request.head.repo.full_name == github.repository' in beta['if']
steps = beta['steps']
checkout = next(s for s in steps if s.get('uses', '').endswith('checkout@v4'))
assert checkout['with']['ref'] == '${{ github.event.pull_request.head.sha }}'
assert checkout['with']['token'] == '${{ github.token }}', 'source checkout must use ordinary read token'
assert beta['env']['PR_READ_TOKEN'] == '${{ github.token }}'
assert beta['env']['BETA_PACKAGE_USER'] == '${{ secrets.BETA_PACKAGE_USER }}'
assert beta['env']['BETA_PACKAGE_TOKEN'] == '${{ secrets.BETA_PACKAGE_TOKEN }}'
assert 'PACKAGE_USER' not in beta['env'] and 'PACKAGE_TOKEN' not in beta['env']
assert beta['env']['BETA_DEPLOY_TOKEN'] == '${{ secrets.BETA_DEPLOY_TOKEN }}'
assert any('download-artifact@v3' in s.get('uses', '') for s in steps)
assert any('upload-artifact@v3' in s.get('uses', '') for s in jobs['web']['steps'])
commands = '\n'.join(s.get('run', '') for s in steps)
assert 'scripts/build-web.sh' not in commands, 'reuse tested web artifact; do not rebuild wasm'
assert 'container-smoke.mjs' in commands
smoke = next(s for s in steps if 'container-smoke.mjs' in s.get('run', ''))
assert smoke.get('env', {}).get('PRODUCTION_ORIGIN') == 'https://ryggattack-beta.modig.online', 'beta images must boot with beta runtime origin, not the production default'
container_smoke = Path('scripts/container-smoke.mjs').read_text()
assert 'RYGGATTACK_ALLOWED_ORIGINS=${origin}' in container_smoke
assert 'PRODUCTION_ORIGIN:origin' in container_smoke
release = yaml.load(Path('.forgejo/workflows/release.yml').read_text(), Loader=yaml.BaseLoader)
assert 'PRODUCTION_ORIGIN' not in str(release), 'production release must retain its canonical smoke default'
assert commands.index('container-smoke.mjs') < commands.index('docker push') < commands.index('beta-pointer.py')
assert ':latest' not in commands
assert 'IMAGE=git.modig.online/ryggattack-beta/ryggattack' in commands
assert '$PACKAGE_' not in commands
assert 'test "$(cat dist/web/source-sha.txt)" = "$(git rev-parse HEAD)"' in commands
assert 'org.opencontainers.image.revision=$(git rev-parse HEAD)' in commands
assert 'uuid' in commands, 'reruns must use a fresh artifact ID, not overwrite SHA tags'
assert 'BETA_DEPLOY_TOKEN' in beta['env']
assert all('PACKAGE_TOKEN' not in j.get('env', {}) and 'BETA_DEPLOY_TOKEN' not in j.get('env', {})
           for k, j in jobs.items() if k != 'beta')
assert 'beta-chart.test.py' in str(jobs['chart'])
assert 'beta-pointer.test.py' in str(jobs['chart'])
print('Beta workflow trust, gates, artifact reuse and publication ordering passed')
