"""Assembly metadata uses Flutter's real encoded input, without exposing it."""
import base64
import hashlib
from pathlib import Path
import tempfile
import unittest

from cache_assembly import cache_port, linux_cache_assembly


class LinuxCacheAssembly(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.config = self.root / "linux/flutter/ephemeral/generated_config.cmake"
        self.config.parent.mkdir(parents=True)

    def write(self, *defines):
        encoded = ",".join(base64.b64encode(value.encode()).decode() for value in defines)
        self.config.write_text('# Generated code do not commit.\nlist(APPEND FLUTTER_TOOL_ENVIRONMENT\n'
                               f'  "DART_DEFINES={encoded}"\n)\n')

    def test_port_is_only_explicit_and_bounded(self):
        for value in ["1", "65535"]:
            self.assertEqual(cache_port(value), int(value))
        for value in [None, 0, True, "", "0", "65536", "-1", "1.0", " 80", "８０"]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                cache_port(value)

    def test_absent_define_stays_unconfigured(self):
        for body in ["# Generated code do not commit.\n", '  "DART_DEFINES="\n']:
            self.config.write_text(body)
            self.assertIsNone(linux_cache_assembly(self.root)["cache_port"])

    def test_actual_encoded_port_and_configuration_hash(self):
        self.write("OTHER=private-sentinel", "NDS_CACHE_PORT=65535")
        self.assertEqual(linux_cache_assembly(self.root), {
            "cache_port": 65535,
            "flutter_configuration_sha256": hashlib.sha256(self.config.read_bytes()).hexdigest()})

    def test_duplicate_or_invalid_port_refuses_without_private_values(self):
        for defines in [("NDS_CACHE_PORT=0",), ("NDS_CACHE_PORT=",),
                        ("NDS_CACHE_PORT=80", "NDS_CACHE_PORT=81")]:
            self.write("OTHER=private-sentinel", *defines)
            with self.subTest(defines=defines), self.assertRaises(ValueError) as caught:
                linux_cache_assembly(self.root)
            self.assertNotIn("private-sentinel", str(caught.exception))

    def test_malformed_encoded_configuration_does_not_mean_unconfigured(self):
        for body in ['  "DART_DEFINES=@@@"\n', '  "DART_DEFINES=/w=="\n',
                     '  "DART_DEFINES="\n  "DART_DEFINES="\n']:
            self.config.write_text(body)
            with self.subTest(body=body), self.assertRaises(ValueError):
                linux_cache_assembly(self.root)

    def test_missing_link_and_oversized_configuration_refuse(self):
        with self.assertRaises(ValueError):
            linux_cache_assembly(self.root)
        target = self.root / "target"
        target.write_text("")
        self.config.symlink_to(target)
        with self.assertRaises(ValueError):
            linux_cache_assembly(self.root)
        self.config.unlink()
        self.config.write_bytes(b"x" * (1024 * 1024 + 1))
        with self.assertRaises(ValueError):
            linux_cache_assembly(self.root)


if __name__ == "__main__":
    unittest.main()
