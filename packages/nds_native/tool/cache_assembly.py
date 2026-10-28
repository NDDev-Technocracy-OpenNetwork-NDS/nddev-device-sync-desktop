"""Read only the cache assembly input from Flutter's generated Linux config."""
import base64
import hashlib
import os
from pathlib import Path
import re
import stat


def cache_port(value):
    if not isinstance(value, str) or not re.fullmatch(r"[0-9]{1,5}", value):
        raise ValueError("Cache port must be an explicit integer from 1 to 65535")
    port = int(value)
    if not 1 <= port <= 65535:
        raise ValueError("Cache port must be an explicit integer from 1 to 65535")
    return port


def linux_cache_assembly(app_root):
    path = Path(app_root) / "linux/flutter/ephemeral/generated_config.cmake"
    try:
        flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
        if path.is_symlink():
            raise ValueError
        with os.fdopen(os.open(path, flags), "rb") as source:
            before = os.fstat(source.fileno())
            if not stat.S_ISREG(before.st_mode) or before.st_size > 1024 * 1024:
                raise ValueError
            payload = source.read(1024 * 1024 + 1)
            after = os.fstat(source.fileno())
        if (len(payload) != before.st_size or
                (before.st_size, before.st_mtime_ns, before.st_ctime_ns) !=
                (after.st_size, after.st_mtime_ns, after.st_ctime_ns)):
            raise ValueError
        lines = [line for line in payload.decode("utf-8").splitlines()
                 if "DART_DEFINES=" in line]
        if len(lines) > 1:
            raise ValueError
        ports = []
        if lines:
            match = re.fullmatch(r'  "DART_DEFINES=([A-Za-z0-9+/=,]*)"', lines[0])
            if match is None:
                raise ValueError
            for encoded in match[1].split(",") if match[1] else []:
                define = base64.b64decode(encoded, validate=True).decode("utf-8")
                key, _, value = define.partition("=")
                if key == "NDS_CACHE_PORT":
                    ports.append(cache_port(value))
        if len(ports) > 1:
            raise ValueError
    except (OSError, UnicodeError, ValueError):
        # Other Dart defines can contain private inputs. Never include them or
        # the generated configuration body in provenance or diagnostics.
        raise ValueError("Expected the bounded generated Linux Flutter cache configuration") from None
    return {"cache_port": ports[0] if ports else None,
            "flutter_configuration_sha256": hashlib.sha256(payload).hexdigest()}
