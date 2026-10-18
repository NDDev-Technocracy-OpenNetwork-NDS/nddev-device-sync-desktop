#!/usr/bin/env python3
"""Record wrapper/native source identities and actual artifact digests separately."""
import argparse
import hashlib
import json
import re
import zipfile
from pathlib import Path
import subprocess
import sys
import tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('artifact', type=Path)
parser.add_argument('--app-root', type=Path, default=Path.cwd())
parser.add_argument('--native-root', type=Path, default=Path(__file__).resolve().parents[1])
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--build-kind', choices=['debug','profile','release'], default='debug')
args = parser.parse_args()

def sha(path):
    with path.open('rb') as source: return hashlib.file_digest(source, 'sha256').hexdigest()

def identity(root):
    repository = next(line.removeprefix('repository: ').strip() for line in (root/'module.yaml').read_text().splitlines() if line.startswith('repository: '))
    commit = subprocess.run(['git','rev-parse','HEAD'],cwd=root,capture_output=True,text=True,check=False)
    dirty = subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True).strip()
    return {'repository':repository, 'commit':commit.stdout.strip() if commit.returncode==0 else 'unknown',
            'state':'uncommitted' if dirty or commit.returncode else 'committed'}

artifact = args.artifact.resolve()
if artifact.is_file(): files = {artifact.name: sha(artifact)}
elif artifact.is_dir():
    paths = sorted(path for path in artifact.rglob('*') if path.is_file())
    if not paths or len(paths)>32768: raise SystemExit('Unexpected artifact file count')
    files = {str(path.relative_to(artifact)):sha(path) for path in paths}
else: raise SystemExit('Built artifact does not exist')
app_source = identity(args.app_root)
native_source = identity(args.native_root)
if any(source['state'] != 'committed' for source in (app_source, native_source)):
    raise SystemExit('Committed wrapper and native sources are required')
subprocess.run([sys.executable, str(Path(__file__).with_name('check-artifact.py')),
                str(artifact), '--native-source-commit', native_source['commit']],
               check=True, timeout=120)
record = {'build_kind':args.build_kind, 'app_source':app_source,
          'native_source':native_source,
          'standards':tomllib.loads((args.app_root/'standarts.lock').read_text()),
          'toolchains':json.loads((args.native_root/'toolchains.lock.json').read_text()),
          'rust_toolchain':tomllib.loads((args.native_root/'packages/nds_native/rust/rust-toolchain.toml').read_text())['toolchain'],
          'locks_sha256':{'app_pubspec':sha(args.app_root/'pubspec.lock'),
                         'native_cargo':sha(args.native_root/'packages/nds_native/rust/Cargo.lock')},
          'artifact_files_sha256':files}
if artifact.suffix == '.apk':
    gradle = (args.app_root/'android/app/build.gradle.kts').read_text()
    minima = re.findall(r'^\s*minSdk = (\d+)\s*$', gradle, re.MULTILINE)
    if len(minima) != 1: raise SystemExit('Android requires an explicit native-compatible minimum API')
    with zipfile.ZipFile(artifact) as archive:
        abis = sorted(name.split('/')[1] for name in archive.namelist()
                      if name.startswith('lib/') and name.endswith('/libnds_native.so'))
    record['platform'] = {'name':'android','minimum_api':int(minima[0]),'native_abis':abis}
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(json.dumps(record,indent=2,sort_keys=True)+'\n')
print('Build provenance recorded: wrapper and native source identities are separate')
