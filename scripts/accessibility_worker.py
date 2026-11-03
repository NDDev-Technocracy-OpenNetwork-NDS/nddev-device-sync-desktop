"""One bounded AT-SPI client for an owned Linux acceptance case.

Some AT-SPI bridges retain peer credentials after disconnect. A client per poll
can therefore exhaust the GUI's pidfds. Keep one client, refresh cached state,
and fail the case if its protocol stalls; never silently restart it.
"""

import collections
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import time

MAX_BYTES = 4096
MAX_QUERIES = 4096
MAX_LIFETIME_SECONDS = 900


def write_pipe(descriptor, raw, deadline):
    pending = memoryview(raw)
    while pending:
        remaining = deadline - time.monotonic()
        if remaining <= 0 or not select.select([], [descriptor], [], remaining)[1]:
            raise TimeoutError("Accessibility worker write deadline")
        try:
            count = os.write(descriptor, pending)
        except BlockingIOError:
            continue
        if count <= 0:
            raise RuntimeError("Accessibility worker pipe closed")
        pending = pending[count:]


class AccessibilityWorker:
    def __init__(self, log, proof, *, command=None, timeout=5):
        self.proof = proof
        self.timeout = timeout
        self.sequence = 0
        self.attempted = 0
        self.completed = 0
        self.closed = False
        self.buffer = bytearray()
        self.started = time.time()
        command = command or ["/usr/bin/python3", str(Path(__file__).resolve()), "--worker"]
        with log.open("ab") as output:
            self.process = subprocess.Popen(
                command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=output, bufsize=0,
            )
        os.set_blocking(self.process.stdin.fileno(), False)
        os.set_blocking(self.process.stdout.fileno(), False)
        try:
            ready = self._read(time.monotonic() + timeout)
            if ready != {"ready": True, "worker_pid": self.process.pid}:
                raise RuntimeError("Accessibility worker did not initialize")
        except Exception:
            self.close(graceful=False)
            raise

    def _read(self, deadline):
        while b"\n" not in self.buffer:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not select.select([self.process.stdout], [], [], remaining)[0]:
                raise TimeoutError("Accessibility worker response deadline")
            try:
                chunk = os.read(self.process.stdout.fileno(), MAX_BYTES + 1)
            except BlockingIOError:
                continue
            if not chunk:
                raise RuntimeError("Accessibility worker stopped")
            self.buffer.extend(chunk)
            if len(self.buffer) > MAX_BYTES:
                raise RuntimeError("Accessibility worker response byte budget")
        line, _, rest = self.buffer.partition(b"\n")
        self.buffer = bytearray(rest)
        value = json.loads(line)
        if type(value) is not dict or value.get("worker_pid") != self.process.pid:
            raise RuntimeError("Accessibility worker response identity")
        return value

    def _exchange(self, value, timeout):
        self.sequence += 1
        value["id"] = self.sequence
        raw = (json.dumps(value) + "\n").encode()
        if len(raw) > MAX_BYTES:
            raise ValueError("Accessibility request byte budget")
        deadline = time.monotonic() + timeout
        write_pipe(self.process.stdin.fileno(), raw, deadline)
        response = self._read(deadline)
        if response.get("id") != self.sequence or "error" in response:
            raise RuntimeError("Accessibility worker request failed")
        return response

    def query(self, pid, labels, activate=False):
        if self.closed:
            raise RuntimeError("Accessibility worker is closed")
        if (type(pid) is not int or pid <= 0 or type(activate) is not bool
                or not isinstance(labels, list) or not 1 <= len(labels) <= 8
                or any(type(label) is not str or len(label) > 256 for label in labels)):
            raise ValueError("Invalid owned accessibility query")
        if self.attempted >= MAX_QUERIES:
            raise RuntimeError("Accessibility request budget")
        self.attempted += 1
        try:
            result = self._exchange(
                {"op": "query", "pid": pid, "labels": labels, "activate": activate},
                self.timeout,
            )
            if type(result.get("found")) is not bool or type(result.get("visited")) is not int:
                raise RuntimeError("Accessibility worker result shape")
            if not 0 <= result["visited"] <= 1024:
                raise RuntimeError("Accessibility traversal budget")
            self.completed += 1
            return result["found"]
        except Exception:
            self.close(graceful=False)
            raise

    def close(self, *, graceful=True):
        if self.closed:
            return
        self.closed = True
        request_close = graceful
        graceful = False
        if self.process.poll() is None:
            try:
                if not request_close:
                    raise RuntimeError("Accessibility transport failed")
                response = self._exchange({"op": "close"}, min(self.timeout, 2))
                graceful = response.get("closed") is True
                self.process.wait(timeout=3)
            except (OSError, ValueError, RuntimeError, TimeoutError, subprocess.TimeoutExpired):
                if self.process.poll() is None:
                    self.process.terminate()
                    try:
                        self.process.wait(timeout=2)
                    except subprocess.TimeoutExpired:
                        self.process.kill()
                        self.process.wait(timeout=2)
        for stream in [self.process.stdin, self.process.stdout]:
            if stream is not None:
                stream.close()
        self.proof.write_text(json.dumps({
            "worker_pid": self.process.pid, "started_at_unix": self.started,
            "finished_at_unix": time.time(), "queries_attempted": self.attempted,
            "queries_completed": self.completed, "worker_restarts": 0,
            "max_inflight": 1, "exit": self.process.returncode,
            "graceful_close": graceful, "request_timeout_seconds": self.timeout,
            "worker_lifetime_seconds": MAX_LIFETIME_SECONDS,
            "scope": "Owned accessibility client lifetime; native GUI state is qualified separately",
        }, indent=2) + "\n")


def pump_events(context):
    for _ in range(64):
        if not context.pending():
            break
        context.iteration(False)


def query_tree(Atspi, pid, labels, activate):
    deadline = time.monotonic() + 2
    desktop = Atspi.get_desktop(0)
    desktop.clear_cache()
    queue = collections.deque()
    for index in range(min(desktop.get_child_count(), 32)):
        item = desktop.get_child_at_index(index)
        if item and item.get_process_id() == pid:
            item.set_cache_mask(Atspi.Cache.NONE)
            item.clear_cache()
            queue.append((item, 0))
    seen = 0
    found = False
    while queue and seen < 1024 and time.monotonic() < deadline:
        item, depth = queue.popleft()
        seen += 1
        try:
            if item.get_name() in labels:
                if not activate:
                    found = True
                    break
                candidate = item
                for _ in range(3):
                    action = candidate.get_action_iface()
                    if action and action.get_n_actions() > 0:
                        found = bool(action.do_action(0))
                        break
                    candidate = candidate.get_parent()
                    if candidate is None:
                        break
                if found:
                    break
            if depth < 24:
                for index in range(min(item.get_child_count(), 256, 1024 - len(queue))):
                    child = item.get_child_at_index(index)
                    if child:
                        queue.append((child, depth + 1))
        except Exception:
            continue
    return {"found": found, "visited": seen}


def serve():
    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi, GLib

    Atspi.init()
    Atspi.set_timeout(250, 500)
    context = GLib.MainContext.default()
    pending = bytearray()
    deadline = time.monotonic() + MAX_LIFETIME_SECONDS
    queries = 0
    os.set_blocking(sys.stdin.fileno(), False)
    os.set_blocking(sys.stdout.fileno(), False)

    def respond(value):
        value["worker_pid"] = os.getpid()
        raw = (json.dumps(value) + "\n").encode()
        if len(raw) > MAX_BYTES:
            raise ValueError("Accessibility response byte budget")
        write_pipe(sys.stdout.fileno(), raw, min(deadline, time.monotonic() + 2))

    try:
        respond({"ready": True})
        while time.monotonic() < deadline:
            pump_events(context)
            if not select.select([sys.stdin], [], [], 0.1)[0]:
                continue
            try:
                chunk = os.read(sys.stdin.fileno(), MAX_BYTES + 1)
            except BlockingIOError:
                continue
            if not chunk:
                break
            pending.extend(chunk)
            if len(pending) > MAX_BYTES:
                raise ValueError("Accessibility request byte budget")
            if b"\n" not in pending:
                continue
            line, _, rest = pending.partition(b"\n")
            pending = bytearray(rest)
            if rest:
                raise ValueError("Only one accessibility request may be in flight")
            request = json.loads(line)
            identifier = request.get("id")
            if type(identifier) is not int or identifier <= 0:
                raise ValueError("Accessibility request identity")
            if request.get("op") == "close":
                respond({"id": identifier, "closed": True})
                break
            pid, labels, activate = request.get("pid"), request.get("labels"), request.get("activate")
            if (request.get("op") != "query" or type(pid) is not int or pid <= 0
                    or type(activate) is not bool or not isinstance(labels, list)
                    or not 1 <= len(labels) <= 8
                    or any(type(label) is not str or len(label) > 256 for label in labels)):
                raise ValueError("Invalid owned accessibility query")
            queries += 1
            if queries > MAX_QUERIES:
                raise ValueError("Accessibility request budget")
            # Events can arrive while waiting for the request pipe.
            pump_events(context)
            try:
                result = query_tree(Atspi, pid, labels, activate)
            except Exception as error:
                result = {"error": "tree_query_failed", "error_type": type(error).__name__}
            respond(dict(result, id=identifier))
        else:
            raise TimeoutError("Accessibility worker case deadline")
    finally:
        Atspi.exit()


if __name__ == "__main__":
    if sys.argv[1:] != ["--worker"]:
        raise SystemExit("Use the owned acceptance driver")
    serve()
