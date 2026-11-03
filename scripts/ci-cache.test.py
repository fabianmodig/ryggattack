#!/usr/bin/env python3
"""Guard compression prerequisites and native/web cache compatibility."""
import pathlib
import unittest

import yaml

ROOT = pathlib.Path(__file__).resolve().parents[1]


def load(relative):
    return yaml.safe_load((ROOT / relative).read_text())


def cache_step(steps):
    return next((i, step) for i, step in enumerate(steps)
                if step.get("uses", "").startswith("https://github.com/Swatinem/rust-cache@"))


class CacheContract(unittest.TestCase):
    def setUp(self):
        self.native = load(".forgejo/workflows/ci.yml")["jobs"]["test"]["steps"]
        self.web = load(".github/actions/setup-web-build/action.yml")["runs"]["steps"]

    def test_zstd_is_installed_before_each_cache_restore(self):
        for label, steps in (("native", self.native), ("web", self.web)):
            with self.subTest(job=label):
                index, _ = cache_step(steps)
                before = "\n".join(s.get("run", "") for s in steps[:index])
                self.assertRegex(before, r"apt-get install[^\n]*\bzstd\b")
                self.assertIn("zstd --version", before)

    def test_cache_epoch_is_shared_but_native_and_web_stay_separate(self):
        native = cache_step(self.native)[1]["with"]
        web = cache_step(self.web)[1]["with"]
        self.assertEqual(native.get("prefix-key"), "v1-rust-zstd")
        self.assertEqual(web.get("prefix-key"), native.get("prefix-key"))
        self.assertEqual(native["shared-key"], "test")
        self.assertEqual(web["shared-key"], "web")
        self.assertIs(native["cache-on-failure"], True)
        self.assertIs(web["cache-on-failure"], True)

    def test_only_main_can_save_caches(self):
        for label, steps in (("native", self.native), ("web", self.web)):
            with self.subTest(job=label):
                config = cache_step(steps)[1]["with"]
                self.assertEqual(config.get("save-if"),
                                 "${{ github.ref == 'refs/heads/main' }}")


if __name__ == "__main__":
    unittest.main()
