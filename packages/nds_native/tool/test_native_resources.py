"""Check resource evidence against actual owned Linux pidfd lifetimes."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "scripts"))
from native_resources import sample_resources


@unittest.skipUnless(sys.platform.startswith("linux") and hasattr(os, "pidfd_open"), "Linux pidfd evidence")
class NativeResourcesTest(unittest.TestCase):
    def test_live_dead_and_closed_pidfds_remain_distinct(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory) / "resources.jsonl"
            owner = types.SimpleNamespace(pid=os.getpid(), poll=lambda: None)
            child = subprocess.Popen(
                [sys.executable, "-c", "import sys; sys.stdin.buffer.read(1)"],
                stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            )
            descriptors = []
            try:
                before = sample_resources(owner, evidence, force=True, tracked_pid=child.pid)
                descriptors.append(os.pidfd_open(child.pid, 0))
                descriptors.append(os.dup(descriptors[0]))
                live = sample_resources(owner, evidence, force=True, tracked_pid=child.pid)
                self.assertEqual(
                    live["pidfd_targets"].get("tracked_matches", 0),
                    before["pidfd_targets"].get("tracked_matches", 0) + 2,
                )
                self.assertGreaterEqual(live["pidfd_targets"].get("live", 0), 2)
                self.assertGreaterEqual(live["pidfd_targets"]["unique_inodes"], 1)
                child.stdin.write(b"x")
                child.stdin.flush()
                child.stdin.close()
                self.assertEqual(child.wait(timeout=3), 0)
                dead = sample_resources(owner, evidence, force=True, tracked_pid=child.pid)
                self.assertGreaterEqual(
                    dead["pidfd_targets"].get("dead", 0),
                    before["pidfd_targets"].get("dead", 0) + 2,
                )
                self.assertEqual(dead["pidfd_targets"].get("tracked_matches", 0), 0)
                for descriptor in descriptors:
                    os.close(descriptor)
                descriptors.clear()
                closed = sample_resources(owner, evidence, force=True)
                self.assertEqual(
                    closed["fd_types"].get("anon_inode:[pidfd]", 0),
                    before["fd_types"].get("anon_inode:[pidfd]", 0),
                )
                self.assertEqual(len([json.loads(line) for line in evidence.read_text().splitlines()]), 4)
            finally:
                for descriptor in descriptors:
                    os.close(descriptor)
                if child.poll() is None:
                    child.terminate()
                    child.wait(timeout=3)
                if child.stdin is not None:
                    child.stdin.close()


if __name__ == "__main__":
    unittest.main()
