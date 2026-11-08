#!/usr/bin/env python3
"""Package only the desktop client and runtime assets; no compiler invocation."""
import argparse
import hashlib
import io
import json
import pathlib
import re
import stat
import struct
import tarfile
import zipfile

TARGETS = {"linux": "x86_64-unknown-linux-gnu", "windows": "x86_64-pc-windows-msvc"}
LAUNCHER = b'''#!/bin/sh
set -eu
cd -- "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
exec ./ryggattack "$@"
'''
INSTRUCTIONS = """Ryggattack desktop (unsigned development build)
Extract the entire archive before launching; keep assets beside the executable.
Linux: run ./launch.sh (it changes to the package directory). Requires x86_64,
Debian 12 or newer / compatible glibc >= 2.36, a graphical session, GPU drivers,
Vulkan or OpenGL support, libudev, Wayland/X11 and libxkbcommon runtime libraries.
Windows x64: double-click ryggattack.exe inside the extracted directory.
Windows DLL dependencies must be checked on the actual build; the archive does
not bundle the Microsoft Visual C++ runtime. Install the Microsoft Visual C++
2015-2022 x64 Redistributable if required by the executable's imports.
Neither a native Windows graphical play-test nor code signing is implied.
Online CLI arguments can be passed to the executable / Linux launcher.
source-sha.txt and metadata.json identify this build; SHA256SUMS verifies downloads.
"""


def verify_binary(data, platform):
    if platform == "linux":
        valid = (len(data) >= 64 and data[:7] == b"\x7fELF\x02\x01\x01"
                 and struct.unpack_from("<H", data, 18)[0] == 62
                 and struct.unpack_from("<H", data, 16)[0] in (2, 3))
    else:
        valid = False
        if len(data) >= 64 and data[:2] == b"MZ":
            offset = struct.unpack_from("<I", data, 60)[0]
            valid = (offset + 26 <= len(data) and data[offset:offset + 4] == b"PE\0\0"
                     and struct.unpack_from("<H", data, offset + 4)[0] == 0x8664
                     and struct.unpack_from("<H", data, offset + 24)[0] == 0x20b)
    if not valid:
        raise ValueError(f"not a {platform} x86_64 executable (ELF64 / PE32+ required)")


def package(root, binary, platform, source_sha, tag, output):
    root, binary, output = pathlib.Path(root), pathlib.Path(binary), pathlib.Path(output)
    if platform not in TARGETS:
        raise ValueError("unsupported platform")
    if not re.fullmatch(r"[0-9a-f]{40}", source_sha):
        raise ValueError("source SHA must be a full lowercase Git SHA")
    if not isinstance(tag, str) or any(ord(c) < 32 for c in tag):
        raise ValueError("tag must not contain control characters")
    if binary.is_symlink() or not binary.is_file():
        raise ValueError("missing executable or symlink executable")
    data = binary.read_bytes()
    verify_binary(data, platform)
    assets = root / "assets"
    if assets.is_symlink() or not assets.is_dir():
        raise ValueError("missing assets directory")
    files = {}
    for path in sorted(assets.rglob("*")):
        if path.is_symlink() or (not path.is_file() and not path.is_dir()):
            raise ValueError("assets must contain only regular files/directories")
        if path.is_file():
            files[path.relative_to(root).as_posix()] = (path.read_bytes(), 0o644)
    if not files:
        raise ValueError("assets directory is empty")
    exe = "ryggattack" + (".exe" if platform == "windows" else "")
    files.update({exe: (data, 0o755), "README.txt": (INSTRUCTIONS.encode(), 0o644),
                  "source-sha.txt": ((source_sha + "\n").encode(), 0o644),
                  "metadata.json": ((json.dumps({"source_sha": source_sha, "tag": tag,
                     "platform": platform, "target": TARGETS[platform], "unsigned": True},
                     indent=2, ensure_ascii=True) + "\n").encode(), 0o644)})
    if platform == "linux":
        files["launch.sh"] = (LAUNCHER, 0o755)
    output.mkdir(parents=True, exist_ok=True)
    prefix = f"ryggattack-{platform}-x86_64"
    archive = output / (prefix + (".zip" if platform == "windows" else ".tar.gz"))
    if platform == "windows":
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as stream:
            for name, (content, mode) in sorted(files.items()):
                info = zipfile.ZipInfo(prefix + "/" + name)
                info.create_system = 3
                info.external_attr = (stat.S_IFREG | mode) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                stream.writestr(info, content)
    else:
        with tarfile.open(archive, "w:gz") as stream:
            for name, (content, mode) in sorted(files.items()):
                info = tarfile.TarInfo(prefix + "/" + name)
                info.mode, info.size = mode, len(content)
                stream.addfile(info, io.BytesIO(content))
    (output / "SHA256SUMS").write_text(hashlib.sha256(archive.read_bytes()).hexdigest()
                                       + "  " + archive.name + "\n")
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", required=True, choices=TARGETS)
    parser.add_argument("--binary", required=True, type=pathlib.Path)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--tag", default="")
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args()
    package(pathlib.Path(__file__).resolve().parents[1], args.binary, args.platform,
            args.source_sha, args.tag, args.output)


if __name__ == "__main__":
    main()
