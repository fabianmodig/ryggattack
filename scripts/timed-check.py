#!/usr/bin/env python3
"""Record exact verification output and wall-clock timings."""
import pathlib
import subprocess
import sys
import time

root = pathlib.Path(__file__).resolve().parents[1]
log_dir = root / 'verification'
log_dir.mkdir(exist_ok=True)
label, *command = sys.argv[1:]
start = time.monotonic()
result = subprocess.run(command, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
elapsed = time.monotonic() - start
output = result.stdout + f'\nTIMING {label}: {elapsed:.3f} s; exit={result.returncode}\n'
(log_dir / f'{label}.log').write_text(output)
with (log_dir / 'timings.tsv').open('a') as file:
    file.write(f'{label}\t{elapsed:.3f}\t{result.returncode}\n')
print(output, end='')
sys.exit(result.returncode)
