#!/usr/bin/env python3
"""Set the paired web/server tag only after both images have been published."""
import pathlib, re, sys
version=sys.argv[1]
if not re.fullmatch(r'sha-[0-9a-f]{7,40}',version):
    raise SystemExit('expected immutable sha-<commit> image tag')
path=pathlib.Path(sys.argv[2] if len(sys.argv)>2 else 'charts/ryggattack/Chart.yaml')
text=path.read_text()
updated,count=re.subn(r'^appVersion:.*$',f'appVersion: {version}',text,flags=re.MULTILINE)
if count!=1:
    raise SystemExit('expected exactly one appVersion')
path.write_text(updated)
