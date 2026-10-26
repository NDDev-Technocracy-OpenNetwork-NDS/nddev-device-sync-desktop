"""Build identity follows the native owner before SDK or compiler execution."""
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import prepare_native


class StandardsIdentity(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.package = self.root / "packages/nds_native"
        self.package.mkdir(parents=True)
        self.lock = self.root / "standarts.lock"
        self.owner = patch.object(prepare_native, "PACKAGE", self.package)
        self.owner.start()
        self.addCleanup(self.owner.stop)

    def write_release(self, value):
        self.lock.write_text("source_release = " + json.dumps(value) + "\n")

    def test_owning_identity_ignores_ambient_value(self):
        self.write_release("v0.0.1-alpha.10")
        with patch.dict(os.environ, {"NDS_STANDARDS_RELEASE": "foreign-sdk-edition"}):
            self.assertEqual(prepare_native.standards_release(), "v0.0.1-alpha.10")

    def test_sdk_identity_format_is_not_restricted_to_semver(self):
        for value in ["unreleased", "edition:alpha_1", "a" * 128]:
            with self.subTest(value=value):
                self.write_release(value)
                self.assertEqual(prepare_native.standards_release(), value)

    def test_invalid_lock_refuses_before_sdk_or_cache_mutation(self):
        for value in ["", "a" * 129, "alpha 1", "alpha\n", "é", True, 7]:
            with self.subTest(value=value):
                self.write_release(value)
                cache = self.root / "unused-cache"
                with patch.object(prepare_native, "run_command") as command:
                    with self.assertRaises(ValueError):
                        prepare_native.prepare(cache, configured=True)
                    command.assert_not_called()
                self.assertFalse(cache.exists())

    def test_missing_lock_does_not_inherit_runtime_identity(self):
        with patch.dict(os.environ, {"NDS_STANDARDS_RELEASE": "runtime"}):
            with self.assertRaises(FileNotFoundError):
                prepare_native.standards_release()

    def test_build_environment_always_replaces_ambient_identity(self):
        prepared = {"rust_bin": "/fixture/rust", "cargo_home": "/fixture/cargo",
                    "source_home": "/fixture/source", "cache": "/fixture/cache",
                    "standards_release": "v0.0.1-alpha.10"}
        with patch.dict(os.environ, {"PATH": "/usr/bin", "NDS_STANDARDS_RELEASE": "runtime"}, clear=True):
            with patch.object(prepare_native, "prepare", return_value=prepared) as command:
                result = prepare_native.build_environment("/fixture/cache", configured=True)
            command.assert_called_once_with(Path("/fixture/cache"), configured=True)
            self.assertEqual(result["NDS_STANDARDS_RELEASE"], "v0.0.1-alpha.10")
            self.assertEqual(os.environ["NDS_STANDARDS_RELEASE"], "runtime")


if __name__ == "__main__":
    unittest.main()
