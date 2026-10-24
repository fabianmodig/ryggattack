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
# Stream to disk during execution: an interrupted long build must retain evidence.
with (log_dir / f'{label}.log').open('w') as log:
    result = subprocess.Popen(command, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    assert result.stdout is not None
    for line in result.stdout:
        log.write(line)
        log.flush()
        print(line, end='', flush=True)
    result.wait()
    elapsed = time.monotonic() - start
    summary = f'\nTIMING {label}: {elapsed:.3f} s; exit={result.returncode}\n'
    log.write(summary)
    print(summary, end='', flush=True)
with (log_dir / 'timings.tsv').open('a') as file:
    file.write(f'{label}\t{elapsed:.3f}\t{result.returncode}\n')
sys.exit(result.returncode)
