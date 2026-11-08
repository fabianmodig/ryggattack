#!/usr/bin/env python3
"""Desktop contract tests; executable fixtures are SYNTHETIC, not game builds."""
import hashlib
import importlib.util
import json
import pathlib
import struct
import subprocess
import tarfile
import tempfile
import unittest
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
SHA = "a" * 40


def module(name):
    path = ROOT / "scripts" / (name + ".py")
    if not path.exists():
        raise AssertionError(f"missing desktop implementation: {path.name}")
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), path)
    obj = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(obj)
    return obj


def fixture(platform):
    # Explicit synthetic ELF/PE headers; never presented as compiled Ryggattack.
    if platform == "linux":
        data = bytearray(64)
        data[:7] = b"\x7fELF\x02\x01\x01"
        struct.pack_into("<HH", data, 16, 2, 62)
    else:
        data = bytearray(256)
        data[:2] = b"MZ"
        struct.pack_into("<I", data, 60, 128)
        data[128:132] = b"PE\0\0"
        struct.pack_into("<H", data, 132, 0x8664)
        struct.pack_into("<H", data, 152, 0x20b)
    return bytes(data)


class Packaging(unittest.TestCase):
    def test_real_archives_complete_selected_inputs_and_metadata(self):
        pack = module("package-desktop")
        with tempfile.TemporaryDirectory(dir=ROOT, prefix=".desktop-test-") as tmp:
            root = pathlib.Path(tmp)
            (root / "assets/players").mkdir(parents=True)
            (root / "assets/players/rider.png").write_bytes(b"synthetic asset")
            (root / "private-evidence.txt").write_text("DO NOT PACKAGE")
            for platform in ("linux", "windows"):
                binary = root / ("fixture-" + platform)
                binary.write_bytes(fixture(platform))
                out = root / platform
                archive = pack.package(root, binary, platform, SHA, "release/weird $() tag", out)
                if platform == "linux":
                    with tarfile.open(archive) as f:
                        files = {m.name: f.extractfile(m).read() for m in f if m.isfile()}
                        self.assertEqual(f.getmember("ryggattack-linux-x86_64/ryggattack").mode, 0o755)
                else:
                    with zipfile.ZipFile(archive) as f:
                        files = {n: f.read(n) for n in f.namelist()}
                prefix = f"ryggattack-{platform}-x86_64/"
                exe = "ryggattack" + (".exe" if platform == "windows" else "")
                expected = {exe, "assets/players/rider.png", "README.txt", "source-sha.txt", "metadata.json"}
                if platform == "linux":
                    expected.add("launch.sh")
                self.assertEqual({n.removeprefix(prefix) for n in files}, expected)
                self.assertEqual(files[prefix + exe], fixture(platform))
                self.assertEqual(files[prefix + "source-sha.txt"].decode().strip(), SHA)
                meta = json.loads(files[prefix + "metadata.json"])
                self.assertEqual(meta["source_sha"], SHA)
                self.assertEqual(meta["tag"], "release/weird $() tag")
                self.assertEqual(meta["platform"], platform)
                self.assertEqual((out / "SHA256SUMS").read_text(),
                                 hashlib.sha256(archive.read_bytes()).hexdigest() + "  " + archive.name + "\n")

    def test_rejects_invalid_platform_sha_binary_and_assets(self):
        pack = module("package-desktop")
        with tempfile.TemporaryDirectory(dir=ROOT, prefix=".desktop-test-") as tmp:
            root = pathlib.Path(tmp)
            binary = root / "binary"
            binary.write_bytes(fixture("linux"))
            (root / "assets").mkdir()
            (root / "assets/a").write_text("asset")
            for platform, sha, tag in (("mac", SHA, "v1"), ("linux", "short", "v1"),
                                       ("linux", SHA, "bad\nvalue")):
                with self.subTest(platform=platform, sha=sha, tag=tag):
                    with self.assertRaises(ValueError):
                        pack.package(root, binary, platform, sha, tag, root / "out")
            for content in (b"", b"#!/bin/sh\n", fixture("windows")):
                binary.write_bytes(content)
                with self.assertRaises(ValueError):
                    pack.package(root, binary, "linux", SHA, "v1", root / "out")
            binary.write_bytes(fixture("linux"))
            (root / "assets/a").unlink()
            with self.assertRaises(ValueError):
                pack.package(root, binary, "linux", SHA, "v1", root / "out")
            (root / "assets/a").symlink_to(binary)
            with self.assertRaises(ValueError):
                pack.package(root, binary, "linux", SHA, "v1", root / "out")
            (root / "assets/a").unlink()
            (root / "assets").rmdir()
            with self.assertRaises(ValueError):
                pack.package(root, binary, "linux", SHA, "v1", root / "out")
            binary.unlink()
            with self.assertRaises(ValueError):
                pack.package(root, binary, "linux", SHA, "v1", root / "out")

    def test_launcher_changes_directory_with_spaces_and_preserves_arguments(self):
        pack = module("package-desktop")
        with tempfile.TemporaryDirectory(dir=ROOT, prefix=".desktop-test-") as tmp:
            root = pathlib.Path(tmp)
            package = root / "game with spaces"
            package.mkdir()
            launcher = package / "launch.sh"
            launcher.write_bytes(pack.LAUNCHER)
            launcher.chmod(0o755)
            # Synthetic script solely for exercising the actual launcher.
            exe = package / "ryggattack"
            exe.write_text('#!/bin/sh\nprintf "%s\\n" "$PWD" "$@"\n')
            exe.chmod(0o755)
            result = subprocess.run([str(launcher), "two words", "$(unsafe)"], cwd=root,
                                    text=True, capture_output=True, check=True)
            self.assertEqual(result.stdout.splitlines(), [str(package), "two words", "$(unsafe)"])


if __name__ == "__main__":
    unittest.main()
