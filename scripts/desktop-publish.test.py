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
        self.tmp = tempfile.TemporaryDirectory(prefix="desktop-test-")
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
                    assets = [dict(a) for a in state["assets"]]
                    if assets and state.get("metadata_fault") == "missing":
                        assets.pop()
                    if assets and state.get("metadata_fault") == "size":
                        assets[0]["size"] += 1
                    if assets and state.get("metadata_fault") == "origin":
                        assets[0]["browser_download_url"] = "http://example.invalid/steal-token"
                    self.response(200, assets)
                elif self.path.startswith("/attachments/"):
                    if state.get("draft_download_404") and state["release"]["draft"]:
                        self.response(404, {})
                        return
                    if state.get("redirect_download"):
                        self.send_response(302)
                        self.send_header("Location", state["redirect_download"])
                        self.end_headers()
                        return
                    if state.get("published_download_404"):
                        self.response(404, {})
                        return
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
                    if state["fail_upload"] or (state.get("fail_upload_after") is not None
                                                and len(state["assets"]) >= state["fail_upload_after"]):
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
                change = json.loads(body)
                state.setdefault("patches", []).append(change)
                if change["draft"] and state.get("rollback_failure"):
                    self.response(500, {})
                    return
                state["release"].update(change)
                if change["draft"] and state.get("rollback_response_failure"):
                    self.response(500, {})
                    return
                if not change["draft"]:
                    state["release"].update(state.get("publish_mutation", {}))
                    if state.get("publish_failure"):
                        self.response(500, {})  # Applied, but response was lost/failed.
                        return
                self.response(200, state["release"])

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        self.url = f"http://127.0.0.1:{server.server_port}"

    def publish(self):
        return self.pub.publish(self.url, "owner/repo", "synthetic-token", TAG, SHA, self.out)

    def test_automatic_token_draft_download_404_publishes_then_verifies(self):
        self.state["draft_download_404"] = True
        self.assertEqual(self.publish(), 7)
        self.assertIs(self.state["release"]["draft"], False)

    def test_draft_upload_verify_then_publish_all_assets(self):
        self.publish()
        self.assertIs(self.state["release"]["draft"], False)
        self.assertEqual(self.state["release"]["tag_name"], TAG)
        self.assertEqual(self.state["release"]["target_commitish"], SHA)
        self.assertEqual({a["name"] for a in self.state["assets"]},
                         {"ryggattack-linux-x86_64.tar.gz", "ryggattack-windows-x86_64.zip", "SHA256SUMS"})
        methods = [m for m, p in self.state["requests"]]
        self.assertEqual(methods.count("PATCH"), 1)
        self.assertEqual(self.state["patches"], [{"draft": False}])
        before_publish = self.state["requests"][:methods.index("PATCH")]
        self.assertEqual([p for m, p in before_publish if p.startswith("/attachments/")], [])
        after_publish = self.state["requests"][methods.index("PATCH") + 1:]
        self.assertEqual([p for m, p in after_publish if m == "GET" and p.startswith("/attachments/")],
                         ["/attachments/1", "/attachments/2", "/attachments/3"])
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

    def test_failed_upload_leaves_only_draft(self):
        self.state["fail_upload_after"] = 1
        with self.assertRaisesRegex(RuntimeError, "HTTP 500"):
            self.publish()
        self.assertEqual(len(self.state["assets"]), 1)
        self.assertIs(self.state["release"]["draft"], True)
        self.assertFalse(any(m == "PATCH" for m, p in self.state["requests"]))

    def test_bad_prepublish_metadata_never_publishes(self):
        for fault in ("missing", "size", "origin"):
            with self.subTest(fault=fault):
                self.state.update(release=None, assets=[], bytes={}, requests=[], metadata_fault=fault)
                with self.assertRaises(ValueError):
                    self.publish()
                self.assertIs(self.state["release"]["draft"], True)
                self.assertFalse(any(m == "PATCH" or p.startswith("/attachments/")
                                     for m, p in self.state["requests"]))

    def test_bad_published_download_restores_draft_and_reads_it_back(self):
        for fault, error in (("corrupt_download", "bytes mismatch"),
                             ("published_download_404", "HTTP 404")):
            with self.subTest(fault=fault):
                self.state.update(release=None, assets=[], bytes={}, requests=[], patches=[],
                                  corrupt_download=False, published_download_404=False)
                self.state[fault] = True
                with self.assertRaisesRegex((ValueError, RuntimeError), error):
                    self.publish()
                self.assertIs(self.state["release"]["draft"], True)
                self.assertEqual(self.state["patches"], [{"draft": False}, {"draft": True}])
                self.assertEqual(self.state["requests"][-1], ("GET", "/api/v1/repos/owner/repo/releases/7"))
                self.assertEqual(len(self.state["assets"]), 3)

    def test_failed_publication_response_still_rolls_back(self):
        self.state["publish_failure"] = True
        with self.assertRaisesRegex(RuntimeError, "HTTP 500"):
            self.publish()
        self.assertEqual(self.state["patches"], [{"draft": False}, {"draft": True}])
        self.assertIs(self.state["release"]["draft"], True)
        self.assertEqual(self.state["requests"][-1], ("GET", "/api/v1/repos/owner/repo/releases/7"))

    def test_mismatched_release_is_not_mutated_by_rollback(self):
        for mutation in ({"id": 8}, {"tag_name": "other"}, {"target_commitish": "b" * 40}):
            with self.subTest(mutation=mutation):
                self.state.update(release=None, assets=[], bytes={}, requests=[], patches=[],
                                  publish_mutation=mutation)
                with self.assertRaisesRegex(ValueError, "release source/tag/draft mismatch") as raised:
                    self.publish()
                self.assertIn("ROLLBACK FAILED", " ".join(raised.exception.__notes__))
                self.assertEqual(self.state["patches"], [{"draft": False}])
                self.assertIs(self.state["release"]["draft"], False)

    def test_failed_rollback_response_is_resolved_by_draft_readback(self):
        self.state.update(corrupt_download=True, rollback_response_failure=True)
        with self.assertRaisesRegex(ValueError, "uploaded asset bytes mismatch") as raised:
            self.publish()
        self.assertEqual(getattr(raised.exception, "__notes__", []), [])
        self.assertIs(self.state["release"]["draft"], True)
        self.assertEqual(self.state["requests"][-1], ("GET", "/api/v1/repos/owner/repo/releases/7"))

    def test_failed_rollback_preserves_original_error_and_reports_failure(self):
        self.state.update(corrupt_download=True, rollback_failure=True)
        with self.assertRaisesRegex(ValueError, "uploaded asset bytes mismatch") as raised:
            self.publish()
        self.assertIn("ROLLBACK FAILED: cannot confirm release 7 is draft: Forgejo PATCH failed: HTTP 500",
                      " ".join(raised.exception.__notes__))
        self.assertIs(self.state["release"]["draft"], False)

    def test_redirect_never_forwards_token_and_restores_draft(self):
        seen = []

        class Sink(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                seen.append(dict(self.headers))
                self.send_response(200)
                self.end_headers()

            def log_message(self, *args):
                pass

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Sink)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        self.state["redirect_download"] = f"http://127.0.0.1:{server.server_port}/token-sink"
        with self.assertRaisesRegex(RuntimeError, "unexpected Forgejo redirect"):
            self.publish()
        self.assertEqual(seen, [])
        self.assertIs(self.state["release"]["draft"], True)
        with self.assertRaisesRegex(ValueError, "invalid asset readback URL/size"):
            self.state.update(release=None, assets=[], bytes={}, metadata_fault="origin")
            self.publish()
        self.assertEqual(seen, [])

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
