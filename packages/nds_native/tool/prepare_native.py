#!/usr/bin/env python3
"""Prepare canonical remote-module build inputs without changing the user's Cargo config."""
import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tomllib


PACKAGE = Path(__file__).resolve().parents[1]
REMOTE_REPOSITORY = "https://github.com/NDDev-Technocracy-OpenNetwork-NDS/nddev-device-sync-remote"


def standards_release():
    # The native producer belongs to Shared, including when another app uses
    # this package. Never substitute the SDK edition or an ambient variable.
    value = tomllib.loads((PACKAGE.parents[1] / "contracts.lock").read_text())["source_release"]
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9_.:-]{1,128}", value):
        raise ValueError("native standards release requires its owning lock identity")
    return value


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


def prepare(cache, *, configured=False):
    release = standards_release()
    rust = PACKAGE / "rust"
    manifest = tomllib.loads((rust / "Cargo.toml").read_text())
    dependency = manifest["dependencies"]["nddev-device-sync-remote"]
    revision = dependency["rev"]
    if dependency["git"] != REMOTE_REPOSITORY or not re.fullmatch(r"[a-f0-9]{40}", revision):
        raise ValueError("Remote module preparation requires its canonical immutable source")

    channel = tomllib.loads((rust / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    cache = Path(cache).resolve()
    cargo_home = cache / revision / "cargo-home"
    cargo_home.mkdir(mode=0o700, parents=True, exist_ok=True)
    rust_bin = Path(run_command(
        ["rustup", "which", "--toolchain", channel, "cargo"], capture=True,
    ).strip()).parent
    environment = dict(
        os.environ, CARGO_HOME=str(cargo_home), GIT_TERMINAL_PROMPT="0",
        RUSTUP_AUTO_INSTALL="0", GIT_CONFIG_GLOBAL=os.devnull,
        GIT_CONFIG_NOSYSTEM="1", CARGO_NET_GIT_FETCH_WITH_CLI="true",
    )
    command = ["rustup", "run", channel, "cargo", "metadata", "--locked", "--format-version", "1"]
    if configured:
        command.append("--offline")
    # Read the exact consumer lock and its Git patches instead of preparing an
    # independent provider source tree. The hook rechecks the same inputs offline.
    metadata = json.loads(run_command(
        command, cwd=rust, env=environment, timeout=180, capture=True,
    ))
    expected = f"git+{REMOTE_REPOSITORY}?rev={revision}#{revision}"
    packages = [p for p in metadata["packages"] if p["name"] == "nddev-device-sync-remote"]
    if len(packages) != 1 or packages[0]["source"] != expected:
        raise ValueError("Remote module preparation source was overridden")
    sdk = Path(packages[0]["manifest_path"]).parent
    actual = run_command(["git", "rev-parse", "HEAD"], cwd=sdk, env=environment, capture=True).strip()
    dirty = run_command(
        ["git", "status", "--porcelain", "--untracked-files=no"], cwd=sdk, env=environment, capture=True,
    ).strip()
    if actual != revision or dirty:
        raise ValueError("Remote module preparation source has tracked changes")
    return {"cargo_home": str(cargo_home), "rust_bin": str(rust_bin), "cache": str(cache), "standards_release": release}


def build_environment(cache=None, *, configured=False):
    cache = Path(cache or os.environ.get("NDS_NATIVE_BUILD_HOME", Path.cwd() / "build/nds-native"))
    result = prepare(cache, configured=configured)
    paths = [result["rust_bin"], os.environ.get("PATH", "")]
    compiler = {}
    frontends = os.environ.get("NDS_CACHE_FRONTENDS")
    if frontends:
        prefix = Path(frontends)
        if not prefix.is_absolute() or not (prefix / "cargo").is_file():
            raise ValueError("NDS cache frontend is not configured")
        paths.insert(0, str(prefix))
        compiler = {name: str(Path(result["rust_bin"]) / (tool + (".exe" if os.name == "nt" else "")))
                    for name, tool in [("RUSTC", "rustc"), ("RUSTDOC", "rustdoc")]}
    return dict(os.environ, CARGO_HOME=result["cargo_home"],
                PATH=os.pathsep.join(paths),
                **compiler,
                NDS_STANDARDS_RELEASE=result["standards_release"],
                NDS_NATIVE_BUILD_HOME=result["cache"], RUSTUP_AUTO_INSTALL="0",
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
