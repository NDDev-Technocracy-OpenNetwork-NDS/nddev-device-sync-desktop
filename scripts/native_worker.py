"""The shared Linux worker packaging contract for builds and actual UI acceptance."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess


def package_linux_worker(source: Path, staging: Path):
    manifest = source / 'packages/nds_native/rust/Cargo.toml'
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--format-version', '1', '--manifest-path', str(manifest)],
        cwd=source, text=True, timeout=120))
    agent = [p for p in metadata['packages'] if p['name'] == 'nddev-device-sync-agent']
    gds = [p for p in metadata['packages'] if p['name'] == 'nddev-device-sync-github']
    if len(agent) != 1 or len(gds) != 1:
        raise SystemExit('One locked Agent and GitHub module package required')
    # The standalone worker retains its owning graph. Shared's native
    # provider overrides must not rewrite the Agent/GitHub module lockfiles.
    worker_environment = dict(os.environ, CARGO_HOME=os.environ['NDS_NATIVE_SOURCE_HOME'])
    subprocess.run(
        ['cargo', 'run', '--locked', '--manifest-path', agent[0]['manifest_path'],
         '--bin', 'nds-package-github-worker', '--', '--bundle', str(staging)],
        cwd=source, env=worker_environment, check=True, timeout=600)
    receipt_path = staging / 'nds-github-worker.build.json'
    if receipt_path.stat().st_size > 16384:
        raise SystemExit('Worker receipt exceeds its bound')
    receipt = json.loads(receipt_path.read_text())
    worker_digest = receipt['sha256']
    worker = staging / 'nds-github-readonly-worker'
    if (receipt['schema_version'] != 1 or receipt['worker'] != worker.name or
            receipt['target'] != 'x86_64-unknown-linux-gnu' or
            receipt['source'] != gds[0]['source'] or
            receipt['host_build_environment'] != {'NDS_GITHUB_WORKER_SHA256': worker_digest} or
            not re.fullmatch('[a-f0-9]{64}', worker_digest) or
            worker.is_symlink() or not worker.is_file() or
            not 0 < worker.stat().st_size <= 128 * 1024 * 1024):
        raise SystemExit('Worker package differs from locked shared source')
    with worker.open('rb') as payload:
        if hashlib.file_digest(payload, 'sha256').hexdigest() != worker_digest:
            raise SystemExit('Worker package digest differs')
    return worker_digest


def deliver_linux_worker(staging: Path, bundle: Path, digest: str):
    if not bundle.is_dir():
        raise RuntimeError('Expected Linux bundle for worker delivery')
    for name in ['nds-github-readonly-worker', 'nds-github-worker.build.json']:
        shutil.copy2(staging / name, bundle / name)
    with (bundle / 'nds-github-readonly-worker').open('rb') as payload:
        if hashlib.file_digest(payload, 'sha256').hexdigest() != digest:
            raise RuntimeError('Delivered worker digest differs')
