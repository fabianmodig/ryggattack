#!/usr/bin/env python3
import json
import pathlib
import re

root = pathlib.Path(__file__).resolve().parents[1]
v = root / 'verification'
pre = json.loads((v / 'prediction-preconfirmation-final.json').read_text())
runs = {}
for folder in sorted(v.glob('prediction-browser-*')):
    if folder.is_dir() and (folder / 'results.json').exists():
        data = json.loads((folder / 'results.json').read_text())
        runs[folder.name] = {'status': data['status'], 'failure': data.get('failure'), 'checks': data.get('checks'), 'cases': data.get('cases'), 'viewport': data.get('game_viewport')}
log = (v / 'prediction-frozen-native.log').read_text()
counts = [int(n) for n in re.findall(r'test result: ok\. (\d+) passed;', log)]
summary = {'rust_tests_passed': sum(counts), 'rust_groups': counts, 'native_graphics': False, 'browser_runs': runs, 'preconfirmation': [], 'timings': []}
for result in pre:
    obs = result['preconfirmation_observations']
    summary['preconfirmation'].append({k:val for k,val in result.items() if k != 'preconfirmation_observations'} | {'observations':len(obs), 'render_before_confirmation_ms': {'min':min(o['render_before_confirmation_ms'] for o in obs), 'max':max(o['render_before_confirmation_ms'] for o in obs)} if obs else None})
for line in (v / 'timings.tsv').read_text().splitlines():
    name, seconds, code = line.split('\t')
    summary['timings'].append({'stage':name,'seconds':float(seconds),'exit':int(code)})
(v / 'prediction-summary.json').write_text(json.dumps(summary, indent=2)+'\n')
print(json.dumps({'rust_tests_passed':sum(counts),'run_statuses':{k:x['status']for k,x in runs.items()},'preconfirmation':summary['preconfirmation']},indent=2))
