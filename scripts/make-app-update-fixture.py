#!/usr/bin/env python3
"""Make bounded opaque staging inputs; these are not native release artifacts."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tomllib
from urllib.parse import urlsplit


REPOSITORY = "NDDev-Technocracy-OpenNetwork-NDS/nddev-device-sync-desktop"
REPOSITORY_ID = "1412610357"
URI = "https://github.com/" + REPOSITORY
WORKFLOW = URI + "/.github/workflows/app-update-qualification.yml@refs/heads/main"


def inputs(root, port):
    if not 1024 <= port <= 65535:
        raise ValueError("qualification port must be 1024..65535")
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    if not re.fullmatch("[a-f0-9]{40}", commit):
        raise ValueError("committed source required")
    if subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=root):
        raise ValueError("tracked source drift")
    lock_bytes = (root / "packages/nds_native/rust/Cargo.lock").read_bytes()
    lock = tomllib.loads(lock_bytes.decode())
    sources = [{"role": "desktop", "repository": URI, "commit": commit}]
    for package in lock["package"]:
        source = package.get("source", "")
        if not source.startswith("git+"):
            continue
        url = urlsplit(source.removeprefix("git+"))
        repository = "https://github.com" + url.path.removesuffix(".git")
        role = "crate." + package["name"]
        if (url.scheme != "https" or url.netloc != "github.com"
                or not re.fullmatch("[a-f0-9]{40}", url.fragment)
                or not re.fullmatch("[a-z0-9_.-]{1,64}", role)):
            raise ValueError("fixture requires canonical immutable public Git sources")
        sources.append({"role": role, "repository": repository, "commit": url.fragment})
    standards = tomllib.loads((root / "standarts.lock").read_text())
    sources.append({"role": "standards", "repository": "https://github.com/" + standards["source_repo"],
                    "commit": standards["source_commit"]})
    sources.sort(key=lambda value: value["role"])
    if len(sources) > 64 or len({s["role"] for s in sources}) != len(sources):
        raise ValueError("ambiguous or oversized fixture source graph")
    seed = hashlib.sha256(lock_bytes + commit.encode()).digest()
    payload = seed * (1024 * 1024 // len(seed))
    origin = f"http://127.0.0.1:{port}"
    manifest = {
        "schema": 1, "application": "nds", "version": "1.0.0-alpha.2",
        "platform": "linux", "architecture": "x86_64", "channel": "alpha",
        "sources": sources,
        # Deliberate qualification case, independently compared with owning Rust
        # exports by the real client. This is not a production migration grant.
        "data": {"vault_journal": 8, "sync_wire": 3, "accounts_fence": 1},
        "artifact": {"url": origin + "/payload", "bytes": len(payload),
                     "sha256": hashlib.sha256(payload).hexdigest(),
                     "app_source": commit, "native_source": commit},
    }
    policy = {
        "manifest_url": origin + "/app-release.json",
        "bundle_url": origin + "/app-release.sigstore.json",
        "issuer": "https://token.actions.githubusercontent.com",
        "workflow_identity": WORKFLOW, "repository_uri": URI,
        "repository_id": REPOSITORY_ID, "allowed_origins": [origin],
        "approved_sources": sources, "installed_version": "1.0.0-alpha.1",
    }
    return manifest, policy, payload


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    manifest, policy, payload = inputs(root, args.port)
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    for name, value in [("app-release.json", manifest), ("qualification-policy.json", policy)]:
        (args.output / name).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    (args.output / "payload").write_bytes(payload)
    print("Opaque staging fixture generated; no application packaging or activation")


if __name__ == "__main__":
    main()
