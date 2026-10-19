#!/usr/bin/env python3
"""Exercise the real shared-gameplay executable, without a window or display."""
import json
import os
import subprocess
import sys

command = ([sys.argv[1]] if len(sys.argv) > 1 else
           ['cargo', 'run', '--locked', '--bin', 'ryggattack-headless', '--'])
env = dict(os.environ)
for key in ('DISPLAY', 'WAYLAND_DISPLAY'):
    env.pop(key, None)
result = subprocess.run(command + ['--ticks', '2048', '--seed', '42', '--bots', '0', '--fire'],
                        env=env, capture_output=True, text=True)
print(result.stderr, file=sys.stderr, end="")
assert result.returncode == 0, result.stderr
frames = [json.loads(line) for line in result.stdout.splitlines()]
assert frames[0]['tick'] == 0
assert frames[-1]['tick'] == 2048
assert frames[-1]['seconds'] == 32.0
assert frames[0]['carts'][0]['position'] != frames[1]['carts'][0]['position']
assert any(frame['projectiles'] > 0 for frame in frames), 'held input must fire real missiles'
assert frames[-1]['until_drop'] < 20.0, 'the shared 30-second pickup clock must advance'
assert any(frame['pickups'] > 0 for frame in frames), 'real rail pickups must spawn'
assert any(frame['scenery'] < frames[0]['scenery'] for frame in frames), 'real blasts must wreck scenery'
assert all(frame['wins'] == [0, 0, 0, 0] for frame in frames), 'solo practice must not score'
match = subprocess.run(command + ['--ticks', '8192', '--seed', '42', '--bots', '1', '--fire'],
                       env=env, capture_output=True, text=True)
assert match.returncode == 0, match.stderr
match_frames = [json.loads(line) for line in match.stdout.splitlines()]
assert max(match_frames[-1]['wins']) == 3, 'a real two-cart match must reach its win target'
assert any(sum(p['alive'] for p in frame['carts']) == 1 for frame in match_frames), 'real elimination'
assert any(sum(p['alive'] for p in frame['carts']) == 2 and any(frame['wins'])
           for frame in match_frames), 'automatic next round must restore both carts'
for invalid in (['--bots', '4'], ['--ticks', '-1'], ['--seed'], ['--unknown']):
    rejected = subprocess.run(command + invalid, env=env, capture_output=True, text=True)
    assert rejected.returncode != 0, f'accepted invalid arguments: {invalid}'
print(result.stdout, end='')
print(json.dumps({'two_cart_match_final_tick': match_frames[-1]['tick'],
                  'wins': match_frames[-1]['wins'], 'automatic_round_restoration': True}))
print('PASS: no-display movement, held fire, fixed clocks, pickups/scenery, solo rules, complete two-cart match, invalid arguments')
