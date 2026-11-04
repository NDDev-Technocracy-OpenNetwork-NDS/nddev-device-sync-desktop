"""Bounded resource evidence for an owned Linux acceptance process."""

import collections
import itertools
import json
import os
import time
from pathlib import Path

_LAST_SAMPLE = {}


def sample_resources(process, evidence, *, force=False, tracked_pid=None):
    """Record counts only; descriptor paths and process environment stay private."""
    now = time.time()
    tick = time.monotonic()
    if not force and tick - _LAST_SAMPLE.get(process.pid, 0) < 1:
        return
    _LAST_SAMPLE[process.pid] = tick
    row = {"timestamp_unix": now, "pid": process.pid, "exit": process.poll()}
    if row["exit"] is not None:
        row["process_gone"] = True
        with evidence.open("a") as output:
            output.write(json.dumps(row, sort_keys=True) + "\n")
        return row
    root = Path("/proc") / str(process.pid)
    try:
        counts = collections.Counter()
        pidfds = collections.Counter()
        inodes = set()
        with os.scandir(root / "fd") as descriptors:
            for index, descriptor in enumerate(itertools.islice(descriptors, 4097)):
                if index == 4096:
                    row["fd_sample_truncated"] = True
                    break
                try:
                    target = os.readlink(descriptor.path)
                except FileNotFoundError:
                    continue
                if target.startswith("socket:"):
                    kind = "socket"
                elif target.startswith("pipe:"):
                    kind = "pipe"
                elif target in {
                    "anon_inode:[eventfd]",
                    "anon_inode:[eventpoll]",
                    "anon_inode:[timerfd]",
                    "anon_inode:inotify",
                    "anon_inode:[pidfd]",
                    "anon_inode:sync_file",
                }:
                    kind = target
                elif target.startswith("anon_inode:"):
                    kind = "anon_inode:other"
                else:
                    kind = "file"
                counts[kind] += 1
                if kind == "anon_inode:[pidfd]":
                    try:
                        info = {}
                        for line in (root / "fdinfo" / descriptor.name).read_text().splitlines():
                            key, _, value = line.partition(":")
                            if key in {"Pid", "ino"}:
                                info[key] = int(value.strip())
                        target_pid = info.get("Pid", 0)
                        pidfds["live" if target_pid > 0 else "dead" if target_pid == -1 else "unknown"] += 1
                        if "ino" in info:
                            inodes.add(info["ino"])
                        if tracked_pid is not None and target_pid == tracked_pid:
                            pidfds["tracked_matches"] += 1
                    except (OSError, ValueError):
                        pidfds["unreadable"] += 1
        row["fd_types"] = dict(counts)
        row["fd_count"] = sum(counts.values())
        row["pidfd_targets"] = dict(pidfds, unique_inodes=len(inodes))
        if tracked_pid is not None:
            row["tracked_pid"] = tracked_pid
        for line in (root / "status").read_text().splitlines():
            key, _, value = line.partition(":")
            if key in {"Threads", "VmRSS", "FDSize"}:
                row[key] = int(value.split()[0])
        for line in (root / "limits").read_text().splitlines():
            if line.startswith("Max open files"):
                row["nofile_soft"], row["nofile_hard"] = line.split()[3:5]
    except (FileNotFoundError, ProcessLookupError):
        row["process_gone"] = True
    except OSError as error:
        row["sample_error"] = error.errno
    with evidence.open("a") as output:
        output.write(json.dumps(row, sort_keys=True) + "\n")
    return row
