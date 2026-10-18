#!/usr/bin/env python3
"""Prepare canonical RDS build inputs without changing the user's Cargo config."""
import argparse
from contextlib import contextmanager
import errno
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time
import tomllib


PACKAGE = Path(__file__).resolve().parents[1]
RDS_REPOSITORY = "https://github.com/NDDev-Technocracy-OpenNetwork-NDS/nddev-device-sync-rds"


def run_command(arguments, *, cwd=None, env=None, timeout=30, capture=False):
    process = subprocess.Popen(
        arguments, cwd=cwd, env=env, text=True, encoding="utf-8",
        stdout=subprocess.PIPE if capture else sys.stderr,
        start_new_session=os.name != "nt",
    )
    try:
        output, _ = process.communicate(timeout=timeout)
    except (subprocess.TimeoutExpired, KeyboardInterrupt):
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
        else:
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        try:
            process.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            if os.name == "nt":
                process.kill()
            else:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            process.communicate(timeout=10)
        raise
    if process.returncode:
        raise subprocess.CalledProcessError(process.returncode, arguments, output)
    return output or ""


@contextmanager
def preparation_lock(path):
    with path.open("a+b") as lock:
        lock.seek(0)
        if not lock.read(1):
            lock.write(b"\0")
            lock.flush()
        deadline = time.monotonic() + 300
        while True:
            try:
                if os.name == "nt":
                    import msvcrt
                    lock.seek(0)
                    msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
                else:
                    import fcntl
                    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except OSError as error:
                if error.errno not in (errno.EACCES, errno.EAGAIN):
                    raise
                if time.monotonic() >= deadline:
                    raise TimeoutError("native input preparation is busy")
                time.sleep(0.1)
        try:
            yield
        finally:
            if os.name == "nt":
                lock.seek(0)
                msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(lock, fcntl.LOCK_UN)


def prepare(cache, *, configured=False):
    manifest = tomllib.loads((PACKAGE / "rust/Cargo.toml").read_text())
    dependency = manifest["dependencies"]["nddev-device-sync-rds"]
    revision = dependency["rev"]
    if dependency["git"] != RDS_REPOSITORY or not re.fullmatch(r"[a-f0-9]{40}", revision):
        raise ValueError("RDS preparation requires its canonical immutable source")
    channel = tomllib.loads((PACKAGE / "rust/rust-toolchain.toml").read_text())["toolchain"]["channel"]
    cache = cache.resolve()
    destination = cache / revision
    destination.mkdir(mode=0o700, parents=True, exist_ok=True)
    cargo_home = destination / "cargo-home"
    cargo_home.mkdir(exist_ok=True)
    source_home = destination / "sdk-cargo-home"
    source_home.mkdir(exist_ok=True)
    rust_bin = Path(run_command(
        ["rustup", "which", "--toolchain", channel, "cargo"], capture=True,
    ).strip()).parent
    environment = dict(os.environ, CARGO_HOME=str(source_home),
                       GIT_TERMINAL_PROMPT="0", RUSTUP_AUTO_INSTALL="0",
                       GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1",
                       CARGO_NET_GIT_FETCH_WITH_CLI="true")
    with preparation_lock(destination / ".prepare.lock"):
        # Resolve only the default SDK first. Its owned-screen feature needs the
        # checked provider config we are about to generate; no code is built.
        probe = destination / "sdk-source"
        probe.mkdir(exist_ok=True)
        (probe / "lib.rs").write_text("")
        (probe / "Cargo.toml").write_text(
            '[package]\nname="nds-build-input"\nversion="0.0.0"\nedition="2024"\n'
            '[lib]\npath="lib.rs"\n[workspace]\n[dependencies]\n'
            'nddev-device-sync-rds={git=' + json.dumps(RDS_REPOSITORY) +
            ',rev=' + json.dumps(revision) + ',default-features=false}\n'
        )
        command = ["rustup", "run", channel, "cargo", "metadata", "--format-version", "1"]
        if configured:
            command.append("--offline")
        # Keep source discovery separate from the product's provider patches.
        # Actual product builds always use their checked --locked graph.
        metadata = json.loads(run_command(
            command, cwd=probe, env=environment, timeout=180, capture=True,
        ))
        packages = [p for p in metadata["packages"] if p["name"] == "nddev-device-sync-rds"]
        expected = f"git+{RDS_REPOSITORY}?rev={revision}#{revision}"
        if len(packages) != 1 or packages[0]["source"] != expected:
            raise ValueError("RDS preparation source was overridden")
        sdk = Path(packages[0]["manifest_path"]).parent
        actual = run_command(["git", "rev-parse", "HEAD"], cwd=sdk, env=environment, capture=True).strip()
        dirty = run_command(
            ["git", "status", "--porcelain", "--untracked-files=no"], cwd=sdk, env=environment, capture=True,
        ).strip()
        if actual != revision or dirty:
            raise ValueError("RDS preparation source has tracked changes")
        if configured:
            # The outer build already performed the owning source verification.
            # Hooks consume those inputs without receiving CI credentials.
            provider = destination / "provider"
            receipt = json.loads((provider / "library-provider-verification.json").read_text())
            source = json.loads((sdk / "providers/screen-source.lock.json").read_text())
            digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
            if (receipt["source"] != source
                    or receipt["config_sha256"] != digest(cargo_home / "config.toml")
                    or receipt["build_receipt_sha256"] != digest(provider / "screen-build-verification.json")):
                raise ValueError("prepared native inputs differ from the owning receipt")
        else:
            run_command(
                [sys.executable, str(sdk / "scripts/prepare-library-provider.py"),
                 "--destination", str(destination / "provider"),
                 "--config", str(cargo_home / "config.toml")],
                env=environment, timeout=240,
            )
    return {"cargo_home": str(cargo_home), "source_home": str(source_home),
            "rust_bin": str(rust_bin), "cache": str(cache)}


def build_environment(cache=None):
    cache = Path(cache or os.environ.get("NDS_NATIVE_BUILD_HOME", Path.cwd() / "build/nds-native"))
    result = prepare(cache)
    return dict(os.environ, CARGO_HOME=result["cargo_home"],
                PATH=result["rust_bin"] + os.pathsep + os.environ.get("PATH", ""),
                NDS_NATIVE_BUILD_HOME=result["cache"], RUSTUP_AUTO_INSTALL="0",
                NDS_NATIVE_SOURCE_HOME=result["source_home"],
                GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1",
                GIT_TERMINAL_PROMPT="0", CARGO_NET_GIT_FETCH_WITH_CLI="true")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", type=Path)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--configured", action="store_true",
                        help="consume inputs verified by the outer build; do not refresh source verification")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    cache = args.cache or Path(os.environ.get("NDS_NATIVE_BUILD_HOME", Path.cwd() / "build/nds-native"))
    if args.json:
        if args.command:
            parser.error("--json cannot wrap a command")
        print(json.dumps(prepare(cache, configured=args.configured)))
    else:
        if args.configured:
            parser.error("--configured is only for the hook's --json handoff")
        command = args.command[1:] if args.command[:1] == ["--"] else args.command
        if not command:
            parser.error("a command or --json is required")
        raise SystemExit(subprocess.run(command, env=build_environment(cache)).returncode)
