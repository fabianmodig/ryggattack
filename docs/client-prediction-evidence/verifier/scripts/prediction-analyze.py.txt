#!/usr/bin/env python3
"""Recompute temporal prediction evidence from retained browser observations."""
import json
import math
import pathlib
import sys

folder = pathlib.Path(sys.argv[1])
summary = []
for path in sorted(folder.glob('*-Predict*.json')):
    if path.name.endswith('-failure-data.json'):
        continue
    data = json.loads(path.read_text())
    if not data.get('samples'):
        continue
    seat = data['samples'][0]['seat']
    inputs = data['inputs']
    wires = data['wire']
    samples = data['samples']
    observations = []
    for control in inputs:
        if not (control['horizontal'] or control['vertical']):
            continue
        confirmed = next((f for f in wires if f['motion'] and f['motion']['applied'][seat]['sequence'] >= control['sequence']), None)
        if not confirmed:
            continue
        for s in samples:
            if control['at'] <= s['at'] < confirmed['at'] and s['local_position'] and s['poses'][seat]:
                distance = math.dist(s['poses'][seat][0], s['authority'][seat]['position'])
                if distance > 1e-5 and s['prediction_clock'][0] > s['simulation_tick']:
                    observations.append({'sequence': control['sequence'], 'input_at_ms': control['at'], 'render_at_ms': s['at'], 'confirmation_at_ms': confirmed['at'], 'render_before_confirmation_ms': confirmed['at'] - s['at'], 'render_distance_from_last_authority': distance, 'prediction_tick': s['prediction_clock'][0], 'authority_tick': s['simulation_tick']})
                    break
    paths = {json.dumps(f['motion']['rails'][seat]['path'], sort_keys=True) for f in wires if f['motion']}
    corrections = []
    common = {f['simulation_tick']: f for f in wires}
    for index, sample in enumerate(samples):
        actual = common.get((sample.get('prediction_clock') or [None])[0])
        if not actual or not sample['local_position']:
            continue
        error = math.dist(sample['local_position'], actual['carts'][seat]['position'])
        if error <= 1e-3:
            continue
        recovered = None
        for later in samples[index + 1:]:
            future = common.get((later.get('prediction_clock') or [None])[0])
            if later['simulation_tick'] >= actual['simulation_tick'] and later['local_position'] and future and math.dist(later['local_position'], future['carts'][seat]['position']) <= 1e-3:
                recovered = later
                break
        corrections.append({'predicted_tick': sample['prediction_clock'][0], 'error_units':error, 'later_authority_matching_sample_after_ms': recovered['at'] - sample['at'] if recovered else None})
    summary.append({'file': path.name, 'seat': seat, 'preconfirmation_observations': observations, 'corrections': corrections, 'unique_authoritative_routes': len(paths), 'junction_observed': any('Junction' in p for p in paths), 'speed_observed': any(f['carts'][seat]['speed_seconds'] > 0 for f in wires), 'death_observed': any(not f['carts'][seat]['alive'] for f in wires)})
assert summary, 'no browser observations'
print(json.dumps(summary, indent=2))
