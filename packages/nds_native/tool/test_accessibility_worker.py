"""Protocol tests exercise process ownership, not a simulated GUI."""
import json
import os
from pathlib import Path
import sys
import tempfile
import signal
import subprocess
import time
import unittest
import types

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "scripts"))
from accessibility_worker import AccessibilityWorker, query_tree

FAKE = """import json,os,sys,time
if sys.argv[1:]==["stop_reading"]:
 import fcntl
 fcntl.fcntl(sys.stdin.fileno(),fcntl.F_SETPIPE_SZ,4096)
print(json.dumps({"ready":True,"worker_pid":os.getpid()}),flush=True)
if sys.argv[1:]==["stop_reading"]:time.sleep(30)
for line in sys.stdin:
 request=json.loads(line)
 if request["op"]=="close":
  print(json.dumps({"id":request["id"],"closed":True,"worker_pid":os.getpid()}),flush=True)
  break
 if request["labels"]==["hang"]:time.sleep(30)
 if request["labels"]==["partial"]:
  sys.stdout.write('{"worker_pid":');sys.stdout.flush();time.sleep(30)
 if request["labels"]==["exit"]:raise SystemExit(9)
 print(json.dumps({"id":request["id"],"found":request["labels"]==["visible"],"visited":1,"worker_pid":os.getpid()}),flush=True)
"""


FILLED_PIPE_RUNNER = 'import json,os,sys,time\nfrom pathlib import Path\nsys.path.insert(0,sys.argv[1])\nfrom accessibility_worker import AccessibilityWorker,MAX_BYTES\nroot=Path(sys.argv[2])\nworker=AccessibilityWorker(root/"worker.log",root/"proof.json",command=[sys.executable,"-u",str(root/"worker.py"),"stop_reading"],timeout=0.2)\nlabels=[""]*8\nwire=lambda:(json.dumps({"op":"query","pid":os.getpid(),"labels":labels,"activate":False,"id":1})+"\\n").encode()\nremaining=MAX_BYTES-len(wire())\nfor index in range(8):\n count=min(256,remaining//6)\n labels[index]="ж"*count\n remaining-=count*6\n if remaining<6 and len(labels[index])+remaining<=256:\n  labels[index]+="a"*remaining\n  remaining=0\nassert remaining==0 and len(wire())==MAX_BYTES\nstarted=time.monotonic()\ntry:\n try:worker.query(os.getpid(),labels)\n except TimeoutError:pass\n else:raise AssertionError("Unresponsive peer was accepted")\n assert time.monotonic()-started<3\n assert worker.process.poll() is not None\n try:worker.query(os.getpid(),["visible"])\n except RuntimeError:pass\n else:raise AssertionError("Failed worker restarted")\nfinally:worker.close()\nprint("bounded filled-pipe failure")\n'

@unittest.skipUnless(sys.platform.startswith("linux"), "Linux acceptance pipe protocol")
class AccessibilityWorkerTest(unittest.TestCase):
    def make_worker(self, directory, timeout=1):
        root = Path(directory)
        fake = root / "worker.py"
        fake.write_text(FAKE)
        return AccessibilityWorker(
            root / "worker.log", root / "proof.json",
            command=[sys.executable, "-u", str(fake)], timeout=timeout,
        )

    def test_many_queries_keep_one_process_and_close_it(self):
        with tempfile.TemporaryDirectory() as directory:
            worker = self.make_worker(directory)
            pid = worker.process.pid
            try:
                for index in range(300):
                    label = "visible" if index % 2 else "absent"
                    self.assertEqual(worker.query(os.getpid(), [label]), label == "visible")
                    self.assertEqual(worker.process.pid, pid)
                    self.assertIsNone(worker.process.poll())
            finally:
                worker.close()
            proof = json.loads((Path(directory) / "proof.json").read_text())
            self.assertEqual(proof["queries_completed"], 300)
            self.assertEqual(proof["worker_restarts"], 0)
            self.assertEqual(proof["exit"], 0)
            self.assertTrue(proof["graceful_close"])

    def test_stalled_query_closes_owned_child_without_restart(self):
        with tempfile.TemporaryDirectory() as directory:
            worker = self.make_worker(directory, timeout=0.2)
            pid = worker.process.pid
            started = time.monotonic()
            with self.assertRaises(TimeoutError):
                worker.query(os.getpid(), ["hang"])
            self.assertLess(time.monotonic() - started, 3)
            self.assertIsNotNone(worker.process.poll())
            self.assertEqual(worker.process.pid, pid)
            with self.assertRaises(RuntimeError):
                worker.query(os.getpid(), ["visible"])
            proof = json.loads((Path(directory) / "proof.json").read_text())
            self.assertEqual(proof["queries_completed"], 0)
            self.assertEqual(proof["worker_restarts"], 0)

    def test_worker_exit_is_a_failure_and_is_not_retried(self):
        with tempfile.TemporaryDirectory() as directory:
            worker = self.make_worker(directory)
            with self.assertRaises(RuntimeError):
                worker.query(os.getpid(), ["exit"])
            self.assertEqual(worker.process.returncode, 9)
            with self.assertRaises(RuntimeError):
                worker.query(os.getpid(), ["visible"])

    def test_partial_response_keeps_one_response_deadline(self):
        with tempfile.TemporaryDirectory() as directory:
            worker = self.make_worker(directory, timeout=0.2)
            started = time.monotonic()
            with self.assertRaises(TimeoutError):
                worker.query(os.getpid(), ["partial"])
            self.assertLess(time.monotonic() - started, 3)
            self.assertIsNotNone(worker.process.poll())

    def test_filled_input_pipe_cannot_block_failure_cleanup(self):
        # The outer owned process group also bounds this regression on an
        # implementation that accidentally returns to blocking writes.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fake = root / "worker.py"
            fake.write_text(FAKE)
            runner = root / "runner.py"
            runner.write_text(FILLED_PIPE_RUNNER)
            process = subprocess.Popen(
                [sys.executable, "-u", str(runner),
                 str(Path(__file__).resolve().parents[3] / "scripts"), str(root)],
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                start_new_session=True,
            )
            try:
                output, _ = process.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.communicate(timeout=3)
                self.fail("Filled accessibility pipe exceeded the owned watchdog")
            self.assertEqual(process.returncode, 0, output.decode(errors="replace"))
            proof = json.loads((root / "proof.json").read_text())
            self.assertEqual(proof["queries_completed"], 0)
            self.assertEqual(proof["worker_restarts"], 0)
            self.assertFalse(proof["graceful_close"])


class CachedAccessible:
    """A cache contract fixture; this is not a native accessibility provider."""
    def __init__(self, name, pid=0, children=()):
        self.remote_name = name
        self.pid = pid
        self.children = list(children)
        self.cached_name = None
        self.cache_mask = 1

    def get_name(self):
        if self.cache_mask == 0:
            return self.remote_name
        if self.cached_name is None:
            self.cached_name = self.remote_name
        return self.cached_name

    def clear_cache(self):
        self.cached_name = None
        for child in self.children:
            child.clear_cache()

    def set_cache_mask(self, mask):
        self.cache_mask = mask
        for child in self.children:
            child.set_cache_mask(mask)

    def get_child_count(self):
        return len(self.children)

    def get_child_at_index(self, index):
        return self.children[index]

    def get_process_id(self):
        return self.pid


class AccessibilityFreshnessTest(unittest.TestCase):
    def test_same_cached_tree_observes_changed_labels(self):
        status = CachedAccessible("Off")
        application = CachedAccessible("application", pid=123, children=[status])
        desktop = CachedAccessible("desktop", children=[application])
        api = types.SimpleNamespace(
            Cache=types.SimpleNamespace(NONE=0), get_desktop=lambda _: desktop,
        )
        self.assertTrue(query_tree(api, 123, ["Off"], False)["found"])
        status.remote_name = "Unknown"
        self.assertTrue(query_tree(api, 123, ["Unknown"], False)["found"])
        self.assertFalse(query_tree(api, 123, ["Off"], False)["found"])
        status.remote_name = "On"
        self.assertTrue(query_tree(api, 123, ["On"], False)["found"])
        self.assertFalse(query_tree(api, 123, ["Unknown"], False)["found"])


if __name__ == "__main__":
    unittest.main()
