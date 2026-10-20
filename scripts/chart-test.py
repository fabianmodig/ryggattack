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
print('Helm public/private, hardening, probes and paired immutable-image contract passed')
