#!/usr/bin/env python3
import pathlib, subprocess, tempfile
script=pathlib.Path('scripts/update-chart-version.py').resolve()
with tempfile.TemporaryDirectory() as directory:
    chart=pathlib.Path(directory)/'Chart.yaml'
    chart.write_text('apiVersion: v2\nappVersion: sha-abcdef0\nversion: 0.1.0\n')
    for tag in ['latest','sha-nope','sha-abcdef0\nappVersion: bad','']:
        before=chart.read_bytes()
        assert subprocess.run(['python3',str(script),tag,str(chart)],capture_output=True).returncode!=0
        assert chart.read_bytes()==before
    subprocess.run(['python3',str(script),'sha-1234567',str(chart)],check=True)
    assert chart.read_text()=='apiVersion: v2\nappVersion: sha-1234567\nversion: 0.1.0\n'
print('Chart update validates paired immutable tag and preserves non-version fields')
