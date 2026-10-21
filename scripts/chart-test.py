#!/usr/bin/env python3
"""Render real Helm and assert the public/private and single-replica contract."""
import subprocess, yaml

def render(*args):
    output=subprocess.check_output(['helm','template','ryggattack','charts/ryggattack',*args],text=True)
    return list(yaml.safe_load_all(output))
items=render()
deps=[i for i in items if i['kind']=='Deployment']
assert len(deps)==2
for d in deps:
    assert d['spec']['replicas']==1
    assert d['spec']['strategy']['type']=='Recreate'
    pod=d['spec']['template']['spec']
    assert pod['imagePullSecrets']==[{'name':'forgejo-pull-secret'}]
    assert pod['automountServiceAccountToken'] is False
    c=pod['containers'][0]
    assert c['securityContext']['readOnlyRootFilesystem']
    assert c['securityContext']['allowPrivilegeEscalation'] is False
    assert c['securityContext']['capabilities']['drop']==['ALL']
    assert pod['securityContext']['runAsNonRoot']
    assert c['resources']['requests'] and c['resources']['limits']
    assert c['readinessProbe'] and c['livenessProbe']
server=next(d for d in deps if d['metadata']['name'].endswith('-server'))
c=server['spec']['template']['spec']['containers'][0]
env={e['name']:e['value'] for e in c['env']}
assert env['RYGGATTACK_MODE']=='production'
assert env['RYGGATTACK_ALLOWED_ORIGINS']=='https://ryggattack.modig.online'
assert c['readinessProbe']['httpGet']=={'path':'/readyz','port':'management'}
assert c['livenessProbe']['httpGet']=={'path':'/healthz','port':'management'}
ingress=next(i for i in items if i['kind']=='Ingress')
assert ingress['spec']['ingressClassName']=='external'
assert ingress['metadata']['annotations']['cert-manager.io/cluster-issuer']=='letsencrypt-cloudflare'
assert ingress['spec']['tls'][0]['hosts']==['ryggattack.modig.online']
paths=ingress['spec']['rules'][0]['http']['paths']
assert {(p['path'],p['pathType']) for p in paths}=={('/','Prefix'),('/ws','Exact')}
assert next(p for p in paths if p['path']=='/ws')['backend']['service']['port']['name']=='websocket'
assert all(p['backend']['service']['port']['name']!='management' for p in paths)
assert all(s['spec']['type']=='ClusterIP' for s in items if s['kind']=='Service')
policies=[i for i in items if i['kind']=='NetworkPolicy']
assert policies, 'management port must be namespace-private under NetworkPolicy'
images=[d['spec']['template']['spec']['containers'][0]['image'] for d in render('--set','image.tag=sha-1234567') if d['kind']=='Deployment']
assert len(images)==2 and all(i.endswith(':sha-1234567') for i in images)
for value in ['latest','sha-bad','']:
    if not value: continue
    assert subprocess.run(['helm','template','bad','charts/ryggattack','--set','image.tag='+value],capture_output=True).returncode!=0
# Publication and migration are configuration contracts too: no branch may
# accidentally gain package/repository writes, and the chart follows BOTH images.
with open('.forgejo/workflows/release.yml') as file:
    release=yaml.safe_load(file)
assert 'permissions' not in release
assert release['env']['DOCKER_BUILDKIT']=='0'
assert 'docker-buildx' not in str(release['jobs']['publish']['steps'])
steps=release['jobs']['publish']['steps']
for name in ['Log in to Forgejo container registry','Publish paired images before changing desired deployment','Commit paired chart appVersion after successful publication']:
    step=next(s for s in steps if s.get('name')==name)
    assert "github.ref == 'refs/heads/main'" in step['if']
    assert "env.PACKAGE_USER != ''" in step['if'] and "env.PACKAGE_TOKEN != ''" in step['if']
publish=next(i for i,s in enumerate(steps) if s.get('name')=='Publish paired images before changing desired deployment')
commit=next(i for i,s in enumerate(steps) if s.get('name')=='Commit paired chart appVersion after successful publication')
assert publish<commit
assert 'for component in web server' in steps[publish]['run']
assert '[skip ci]' in steps[commit]['run']
assert 'git push origin HEAD:main' in steps[commit]['run']
with open('.github/workflows/pages.yml') as file:
    pages=yaml.safe_load(file)
assert any(s.get('with',{}).get('path')=='dist/pages' for s in pages['jobs']['web']['steps'])
assert not any('scripts/build-web.sh' in s.get('run','') for s in pages['jobs']['web']['steps'])
print('Helm public/private, hardening, probes, paired immutable publication and Pages migration contracts passed')
