#!/usr/bin/env python3
"""Production defaults stay unchanged; beta hosts/origins/metadata are isolated."""
import subprocess
import yaml


def render(*args):
    return list(yaml.safe_load_all(subprocess.check_output([
        'helm', 'template', 'ryggattack-beta', 'charts/ryggattack', *args], text=True)))


host = 'ryggattack-beta.modig.online'
items = render('--set', f'publicHost={host}', '--set', f'allowedOrigins=https://{host}',
               '--set-string', 'deployment.sourceSHA=' + 'a' * 40,
               '--set-string', 'deployment.pullRequest=42')
for item in items:
    if item['kind'] == 'Ingress':
        assert item['spec']['rules'][0]['host'] == host, 'beta ingress host must be configurable'
        assert item['spec']['tls'][0]['hosts'] == [host]
    if item['kind'] == 'Deployment':
        annotations = item['spec']['template']['metadata']['annotations']
        assert annotations['ryggattack.modig.online/source-sha'] == 'a' * 40
        assert annotations['ryggattack.modig.online/pull-request'] == '42'
        if item['metadata']['name'].endswith('-server'):
            env = {e['name']: e['value'] for e in item['spec']['template']['spec']['containers'][0]['env']}
            assert env['RYGGATTACK_ALLOWED_ORIGINS'] == f'https://{host}'
print('Beta chart host, origin and source metadata passed')
