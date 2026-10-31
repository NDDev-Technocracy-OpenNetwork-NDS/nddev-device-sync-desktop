#!/usr/bin/env python3
"""Verify the locked native graph, including dependencies of other platforms."""
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1] / 'packages/nds_native/rust'
metadata = json.loads(subprocess.check_output(
    ['cargo', 'metadata', '--locked', '--all-features', '--format-version', '1'],
    cwd=root, text=True, timeout=180,
))
# Two source URLs or revisions can create separate Rust types and independent
# accounts/SDK state even when their names and versions match. Android-only
# dependencies must participate in this check on Linux CI too.
for name in (
    'nddev-device-sync-application',
    'nddev-device-sync-adapter-io',
    'nddev-device-sync-domain',
    'nddev-device-sync-accounts',
    'nddev-device-sync-organizations',
    'nddev-device-sync-telemetry',
    'nddev-device-sync-vault',
    'nddev-device-sync-github',
    'nddev-device-sync-clipboard',
):
    packages = [package for package in metadata['packages'] if package['name'] == name]
    if len(packages) != 1:
        raise SystemExit(f'Expected one {name} package across all platforms, found {len(packages)}')
print('Locked native graph has one application, domain, adapter I/O, accounts, telemetry, vault, GitHub module and clipboard package')
