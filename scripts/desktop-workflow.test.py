#!/usr/bin/env python3
"""Parse actual workflow semantics without invoking any compiler."""
import pathlib
import unittest
import yaml

ROOT = pathlib.Path(__file__).resolve().parents[1]


class DesktopWorkflow(unittest.TestCase):
    def setUp(self):
        path = ROOT / ".forgejo/workflows/desktop-release.yml"
        self.assertTrue(path.is_file(), "missing independent desktop workflow")
        self.workflow = yaml.safe_load(path.read_text())

    def test_all_tags_and_manual_only_publish_push_tag_after_both_lanes(self):
        events = self.workflow.get("on", self.workflow.get(True))
        self.assertEqual(events["push"], {"tags": ["**"]})
        self.assertIn("workflow_dispatch", events)
        jobs = self.workflow["jobs"]
        build = jobs["build"]
        self.assertEqual(build["runs-on"], "ubuntu-latest")
        self.assertNotIn("container", build)
        self.assertEqual(build["env"]["CARGO_BUILD_JOBS"], "1")
        lanes = build["strategy"]["matrix"]["include"]
        self.assertEqual({x["platform"] for x in lanes}, {"linux", "windows"})
        self.assertEqual({x["target"] for x in lanes},
                         {"x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"})
        publish = jobs["publish"]
        self.assertEqual(publish["needs"], ["build"])
        self.assertEqual(publish["if"], "github.event_name == 'push' && startsWith(github.ref, 'refs/tags/')")
        self.assertEqual(publish["env"]["FORGEJO_RELEASE_TOKEN"], "${{ github.token }}")
        for step in build["steps"]:
            self.assertNotIn("github.token", str(step))
            self.assertNotIn("secrets.", str(step))
            self.assertNotIn("github.ref_name", step.get("run", ""))
        self.assertFalse(any("secrets." in str(v) for v in self.workflow.values()))

    def test_prerequisites_cache_build_and_artifact_contract(self):
        steps = self.workflow["jobs"]["build"]["steps"]
        cache_index, cache = next((i, s) for i, s in enumerate(steps) if "rust-cache@" in s.get("uses", ""))
        before = "\n".join(s.get("run", "") for s in steps[:cache_index])
        for dependency in ("clang", "lld", "llvm", "cmake", "ninja-build", "python3-venv", "zstd", "libudev-dev", "libwayland-dev"):
            self.assertIn(dependency, before)
        self.assertIn("zstd --version", before)
        self.assertEqual(cache["with"]["save-if"], "${{ github.ref == 'refs/heads/main' }}")
        self.assertEqual(cache["with"]["shared-key"], "desktop-${{ matrix.platform }}")
        self.assertIn("desktop", cache["with"]["prefix-key"])
        run = "\n".join(s.get("run", "") for s in steps)
        self.assertIn("pip install --only-binary=:all: cargo-xwin==0.23.1", run)
        self.assertIn("python3 -m venv", run)
        self.assertIn("cargo build --locked --release --bin ryggattack --target x86_64-unknown-linux-gnu", run)
        self.assertIn("cargo xwin build --locked --release --bin ryggattack --target x86_64-pc-windows-msvc", run)
        windows = next(s for s in steps if "cargo xwin build" in s.get("run", ""))
        self.assertIn("target-feature=+crt-static", windows["env"]["RUSTFLAGS"])
        self.assertEqual(next(s for s in steps if "checkout@" in s.get("uses", ""))["with"]["persist-credentials"], False)
        upload = next(s for s in steps if "upload-artifact@" in s.get("uses", ""))
        self.assertEqual(upload["uses"], "https://code.forgejo.org/actions/upload-artifact@v3")
        self.assertEqual(upload["with"]["if-no-files-found"], "error")
        publish_steps = self.workflow["jobs"]["publish"]["steps"]
        downloads = [s for s in publish_steps if "download-artifact@" in s.get("uses", "")]
        self.assertEqual({s["with"]["name"] for s in downloads}, {"desktop-linux", "desktop-windows"})
        self.assertTrue(all(s["uses"] == "https://code.forgejo.org/actions/download-artifact@v3" for s in downloads))
        ci = yaml.safe_load((ROOT / ".forgejo/workflows/ci.yml").read_text())
        runs = "\n".join(s.get("run", "") for s in ci["jobs"]["chart"]["steps"])
        for test in ("desktop-workflow.test.py", "desktop-release.test.py", "desktop-publish.test.py"):
            self.assertIn("python3 scripts/" + test, runs)


if __name__ == "__main__":
    unittest.main()
