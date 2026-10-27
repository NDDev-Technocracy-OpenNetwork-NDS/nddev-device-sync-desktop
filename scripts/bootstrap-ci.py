#!/usr/bin/env python3
"""Install checksum-pinned Flutter only inside an ephemeral CI runner."""
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tarfile
import zipfile

root = Path(__file__).resolve().parents[1]
lock = json.loads((root / 'toolchains.lock.json').read_text())
if os.environ.get('CI') != 'true' or not os.environ.get('RUNNER_TEMP') or not os.environ.get('GITHUB_PATH'):
    raise SystemExit('This installer is restricted to an ephemeral GitHub CI runner')
key = platform.system() + '-' + platform.machine()
entry = lock['archives'].get(key)
if entry is None:
    raise SystemExit('No reviewed Flutter SDK archive for this host')
destination = Path(os.environ['RUNNER_TEMP']) / 'nds-flutter'
destination.mkdir(exist_ok=False)
archive = destination / Path(entry['archive']).name
url = 'https://storage.googleapis.com/flutter_infra_release/releases/' + entry['archive']
# This pinned GET writes a regular file, so curl can reset a partial transfer
# before retrying receive failures; the outer 650-second bound remains strict.
subprocess.run(['curl','--fail','--location','--show-error','--retry','2','--retry-all-errors','--max-time','600',url,'--output',str(archive)],check=True,timeout=650)
with archive.open('rb') as source:
    actual = hashlib.file_digest(source, 'sha256').hexdigest()
if actual != entry['sha256']:
    raise SystemExit('Flutter SDK checksum mismatch')
if archive.name.endswith('.tar.xz'):
    with tarfile.open(archive) as source: source.extractall(destination,filter='data')
elif platform.system() == 'Darwin':
    # The system extractor retains executable and symlink attributes in the
    # verified official macOS archive; Python ZipFile does not preserve them.
    # Allow the multi-gigabyte SDK to finish on hosted Intel Macs, with a finite budget.
    subprocess.run(['unzip','-q',str(archive),'-d',str(destination)],check=True,timeout=300)
else:
    with zipfile.ZipFile(archive) as source: source.extractall(destination)
with Path(os.environ['GITHUB_PATH']).open('a') as output:
    output.write(str(destination / 'flutter/bin')+'\n')
