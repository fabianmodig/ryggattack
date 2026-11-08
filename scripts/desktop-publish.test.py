#!/usr/bin/env python3
"""Real archives + loopback fake Forgejo HTTP API; no external writes."""
import hashlib
import http.server
import importlib.util
import json
import pathlib
import tempfile
import threading
import unittest
import urllib.parse

ROOT = pathlib.Path(__file__).resolve().parents[1]
SHA = "a" * 40
TAG = "test/unsafe-$()"


def load(name):
    path = ROOT / "scripts" / (name + ".py")
    if not path.exists():
        raise AssertionError(f"missing desktop implementation: {name}")
    spec = importlib.util.spec_from_file_location(name, path)
    obj = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(obj)
    return obj


class Publisher(unittest.TestCase):
    def setUp(self):
        self.pub = load("publish-desktop")
        self.tmp = tempfile.TemporaryDirectory(dir=ROOT, prefix=".desktop-test-")
        self.addCleanup(self.tmp.cleanup)
        root = pathlib.Path(self.tmp.name)
        (root / "assets").mkdir()
        (root / "assets/a").write_bytes(b"synthetic asset")
        self.out = root / "downloads"
        pack, tests = load("package-desktop"), load("desktop-release.test")
        for platform in ("linux", "windows"):
            binary = root / platform
            binary.write_bytes(tests.fixture(platform))
            pack.package(root, binary, platform, SHA, TAG, self.out / platform)
        self.state = {"release": None, "assets": [], "bytes": {}, "requests": [],
                      "sha": SHA, "fail_upload": False, "corrupt_download": False}
        state = self.state

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def response(self, code, data):
                self.send_response(code)
                self.end_headers()
                self.wfile.write(json.dumps(data).encode())

            def do_GET(self):
                state["requests"].append(("GET", self.path))
                if "/tags/" in self.path and "/releases/" not in self.path:
                    self.response(200, {"name": TAG, "commit": {"sha": state["sha"]}})
                elif "/releases/tags/" in self.path:
                    self.response(200 if state["release"] else 404, state["release"] or {})
                elif self.path.endswith("/assets"):
                    self.response(200, state["assets"])
                elif self.path.startswith("/attachments/"):
                    self.send_response(200)
                    self.end_headers()
                    data = state["bytes"][int(self.path.rsplit("/", 1)[1])]
                    self.wfile.write(b"bad" if state["corrupt_download"] else data)
                else:
                    self.response(200, state["release"])

            def do_POST(self):
                state["requests"].append(("POST", self.path))
                body = self.rfile.read(int(self.headers["Content-Length"]))
                if self.path.endswith("/releases"):
                    state["release"] = dict(json.loads(body), id=7)
                    self.response(201, state["release"])
                else:
                    if state["fail_upload"]:
                        self.response(500, {})
                        return
                    name = urllib.parse.parse_qs(urllib.parse.urlsplit(self.path).query)["name"][0]
                    data = body.split(b"\r\n\r\n", 1)[1].rsplit(b"\r\n--", 1)[0]
                    identity = len(state["assets"]) + 1
                    asset = {"id": identity, "name": name, "size": len(data),
                             "browser_download_url": f"http://127.0.0.1:{self.server.server_port}/attachments/{identity}"}
                    state["assets"].append(asset)
                    state["bytes"][identity] = data
                    self.response(201, asset)

            def do_PATCH(self):
                state["requests"].append(("PATCH", self.path))
                body = self.rfile.read(int(self.headers["Content-Length"]))
                state["release"].update(json.loads(body))
                self.response(200, state["release"])

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        self.url = f"http://127.0.0.1:{server.server_port}"

    def publish(self):
        return self.pub.publish(self.url, "owner/repo", "synthetic-token", TAG, SHA, self.out)

    def test_draft_upload_verify_then_publish_all_assets(self):
        self.publish()
        self.assertIs(self.state["release"]["draft"], False)
        self.assertEqual(self.state["release"]["tag_name"], TAG)
        self.assertEqual(self.state["release"]["target_commitish"], SHA)
        self.assertEqual({a["name"] for a in self.state["assets"]},
                         {"ryggattack-linux-x86_64.tar.gz", "ryggattack-windows-x86_64.zip", "SHA256SUMS"})
        methods = [m for m, p in self.state["requests"]]
        self.assertEqual(methods.count("PATCH"), 1)
        before_publish = self.state["requests"][:methods.index("PATCH")]
        self.assertEqual({p for m, p in before_publish if m == "GET" and p.startswith("/attachments/")},
                         {"/attachments/1", "/attachments/2", "/attachments/3"})
        sums = self.state["bytes"][3].decode()
        for a in self.state["assets"][:2]:
            self.assertIn(hashlib.sha256(self.state["bytes"][a["id"]]).hexdigest() + "  " + a["name"], sums)

    def test_manual_branch_and_manual_tag_cli_cannot_publish(self):
        import os
        import subprocess
        for event, ref in (("workflow_dispatch", "refs/heads/main"),
                           ("workflow_dispatch", "refs/tags/" + TAG),
                           ("push", "refs/heads/main")):
            env = dict(os.environ, GITHUB_EVENT_NAME=event, GITHUB_REF=ref, GITHUB_REF_NAME=TAG,
                       GITHUB_SERVER_URL=self.url, GITHUB_REPOSITORY="owner/repo",
                       FORGEJO_RELEASE_TOKEN="synthetic-token")
            result = subprocess.run(["python3", str(ROOT / "scripts/publish-desktop.py"),
                                     "--directory", str(self.out)], env=env,
                                    cwd=ROOT, text=True, capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("restricted to push tag events", result.stderr)
        self.assertEqual(self.state["requests"], [])

    def test_rebound_checksums_do_not_hide_wrong_source_or_tag(self):
        package = load("package-desktop")
        root = self.out.parent
        for sha, tag in (("b" * 40, TAG), (SHA, "different-tag")):
            package.package(root, root / "windows", "windows", sha, tag, self.out / "windows")
            with self.assertRaisesRegex(ValueError, "source/tag/platform mismatch"):
                self.publish()
            self.assertEqual(self.state["requests"], [])

    def test_existing_release_or_tag_mismatch_causes_no_write(self):
        for existing in (False, True):
            self.state["release"] = {"id": 7, "draft": True} if existing else None
            self.state["sha"] = SHA if existing else "b" * 40
            self.state["requests"].clear()
            with self.assertRaises((ValueError, RuntimeError)):
                self.publish()
            self.assertFalse(any(m != "GET" for m, p in self.state["requests"]))

    def test_failed_upload_or_corrupt_readback_leaves_only_draft(self):
        for mode in ("fail_upload", "corrupt_download"):
            self.state.update(release=None, assets=[], bytes={}, requests=[], fail_upload=False,
                              corrupt_download=False)
            self.state[mode] = True
            with self.assertRaises((ValueError, RuntimeError)):
                self.publish()
            self.assertIs(self.state["release"]["draft"], True)
            self.assertFalse(any(m == "PATCH" for m, p in self.state["requests"]))

    def test_corrupt_or_missing_artifact_causes_no_api_writes(self):
        archive = next(self.out.rglob("*.zip"))
        archive.write_bytes(b"corrupted")
        with self.assertRaises((ValueError, RuntimeError)):
            self.publish()
        self.assertFalse(any(m != "GET" for m, p in self.state["requests"]))
        archive.unlink()
        with self.assertRaises((ValueError, RuntimeError)):
            self.publish()
        self.assertFalse(any(m != "GET" for m, p in self.state["requests"]))


if __name__ == "__main__":
    unittest.main()
