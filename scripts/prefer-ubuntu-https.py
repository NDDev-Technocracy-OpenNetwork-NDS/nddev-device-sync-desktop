#!/usr/bin/env python3
"""Prefer the existing Ubuntu HTTPS archive on the hosted Linux runner."""

import os
from pathlib import Path
import re
import stat
import sys


MIRRORS = Path("/etc/apt/apt-mirrors.txt")
MAX_BYTES = 65536
# actions/runner-images@7e7dc1bcc0ef464e5e315a2b4c33609590c6cffe,
# images/ubuntu/scripts/build/configure-apt-sources.sh.
ORIGINS = (
    b"http://azure.archive.ubuntu.com/ubuntu/",
    b"https://archive.ubuntu.com/ubuntu/",
    b"https://security.ubuntu.com/ubuntu/",
)
RECORD = re.compile(
    rb"[ \t]*(?P<origin>[^ \t\r\n]+)[ \t]+priority:(?P<priority>[0-9]+)"
    rb"[ \t]*(?:#[^\r\n]*)?(?:\r?\n)?"
)


def preferred_mirrors(data: bytes) -> bytes:
    if len(data) > MAX_BYTES:
        raise ValueError("Mirror list exceeds its bound")
    records = {}
    offset = 0
    for line in data.splitlines(keepends=True):
        if not re.fullmatch(rb"[ \t]*(?:#[^\r\n]*)?(?:\r?\n)?", line):
            record = RECORD.fullmatch(line)
            if record is None or record["origin"] not in ORIGINS:
                raise ValueError("Unknown mirror record")
            origin = record["origin"]
            if origin in records:
                raise ValueError("Duplicate mirror record")
            records[origin] = (offset + record.start("priority"), record["priority"])
        offset += len(line)
    if set(records) != set(ORIGINS):
        raise ValueError("Missing mirror record")
    priorities = tuple(records[origin][1] for origin in ORIGINS)
    if priorities not in ((b"1", b"2", b"3"), (b"2", b"1", b"3")):
        raise ValueError("Unknown mirror priorities")
    updated = bytearray(data)
    for origin, priority in zip(ORIGINS, (b"2", b"1", b"3")):
        updated[records[origin][0]] = priority[0]
    return bytes(updated)


def prefer_file(path: Path) -> bool:
    flags = os.O_RDWR | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK
    with os.fdopen(os.open(path, flags), "r+b", buffering=0) as source:
        metadata = os.fstat(source.fileno())
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_size > MAX_BYTES:
            raise ValueError("A bounded regular mirror list is required")
        original = source.read(MAX_BYTES + 1)
        updated = preferred_mirrors(original)
        if updated == original:
            return False
        source.seek(0)
        if source.write(updated) != len(updated):
            raise OSError("Incomplete mirror preference write")
        source.flush()
        os.fsync(source.fileno())
    return True


def main() -> None:
    if (
        len(sys.argv) != 1
        or sys.platform != "linux"
        or os.geteuid() != 0
        or os.environ.get("GITHUB_ACTIONS") != "true"
        or os.environ.get("RUNNER_OS") != "Linux"
    ):
        raise SystemExit("Requires the Linux GitHub Actions runner as root")
    try:
        changed = prefer_file(MIRRORS)
    except (OSError, ValueError):
        raise SystemExit("Ubuntu mirror preference refused") from None
    print("Ubuntu HTTPS archive preferred" if changed else "Ubuntu HTTPS archive already preferred")


if __name__ == "__main__":
    main()
