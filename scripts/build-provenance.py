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
parser.add_argument('--build-kind', choices=['debug','profile','release'],
                    help='Must agree with the actual Linux CMake build and bundle')
parser.add_argument('--product-version',
                    help='Optional approved product candidate, separate from the wrapper version')
args = parser.parse_args()
args.app_root = args.app_root.resolve()
args.native_root = args.native_root.resolve()
if args.product_version is not None:
    candidate = re.fullmatch(r'1\.0\.0-alpha\.([1-9][0-9]{0,9})', args.product_version)
    if candidate is None or int(candidate[1]) > 4294967295:
        parser.error('Product version must be 1.0.0-alpha.<1..4294967295>')

def sha(path):
    with path.open('rb') as source: return hashlib.file_digest(source, 'sha256').hexdigest()

def identity(root):
    repository = next(line.removeprefix('repository: ').strip() for line in (root/'module.yaml').read_text().splitlines() if line.startswith('repository: '))
    commit = subprocess.run(['git','rev-parse','HEAD'],cwd=root,capture_output=True,text=True,check=False)
    dirty = subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True).strip()
    return {'repository':repository, 'commit':commit.stdout.strip() if commit.returncode==0 else 'unknown',
            'state':'uncommitted' if dirty or commit.returncode else 'committed'}

def elf_header(path):
    with path.open('rb') as binary:
        return binary.read(64)

def linux_elf(path):
    header = elf_header(path)
    if (len(header) != 64 or header[:7] != b'\x7fELF\x02\x01\x01' or
            int.from_bytes(header[16:18], 'little') not in (2, 3) or
            int.from_bytes(header[18:20], 'little') != 62 or
            int.from_bytes(header[20:24], 'little') != 1):
        raise SystemExit('Expected a Linux x86_64 ELF artifact')

def linux_bundle(artifact, paths):
    # This is the existing Linux packager's x64 output, not a new archive or
    # installation format. Other platform provenance callers retain their mode.
    modes = [mode for mode in ('debug', 'profile', 'release')
             if artifact == args.app_root / 'build/linux/x64' / mode / 'bundle']
    elfs = [path for path in paths if elf_header(path).startswith(b'\x7fELF')]
    if not modes and not elfs:
        return None
    if len(modes) != 1 or not elfs:
        raise SystemExit('Expected the canonical Linux x64 build bundle')
    mode = modes[0]
    if args.build_kind is not None and args.build_kind != mode:
        raise SystemExit('Declared build kind differs from the Linux bundle')
    cache = artifact.parent / 'CMakeCache.txt'
    if (not cache.is_file() or cache.is_symlink() or cache.stat().st_size > 1024 * 1024):
        raise SystemExit('Expected the actual Linux CMake build configuration')
    configured = re.findall(r'^CMAKE_BUILD_TYPE:(?:STRING|UNINITIALIZED)=(Debug|Profile|Release)$',
                            cache.read_text(), re.MULTILINE)
    if configured != [mode.capitalize()]:
        raise SystemExit('Linux CMake build kind differs from its bundle')
    for path in paths:
        if not path.resolve().is_relative_to(artifact):
            raise SystemExit('Linux bundle contains an external file reference')
    if any(path.is_symlink() and not path.is_file() for path in artifact.rglob('*')):
        raise SystemExit('Linux bundle must not omit linked directories or dangling files')
    for path in elfs:
        linux_elf(path)
    runners = [path for path in elfs if path.parent == artifact and
               path.name != 'nds-gds-readonly-worker']
    if len(runners) != 1 or not runners[0].stat().st_mode & 0o111:
        raise SystemExit('Expected one executable Linux application runner')
    kernels = list(artifact.rglob('kernel_blob.bin'))
    aot = artifact / 'lib/libapp.so'
    if mode == 'debug':
        if len(kernels) != 1 or kernels[0].stat().st_size == 0 or aot.exists():
            raise SystemExit('Debug Linux bundle must contain its JIT payload only')
    elif kernels or not aot.is_file():
        raise SystemExit('Release/profile Linux bundle requires AOT without JIT payloads')
    else:
        linux_elf(aot)

    # Consume the existing native_worker.py receipt contract, without rebuilding,
    # copying or changing its independently owned worker graph.
    worker = artifact / 'nds-gds-readonly-worker'
    receipt_path = artifact / 'nds-gds-worker.build.json'
    if (worker.is_symlink() or not worker.is_file() or
            not 0 < worker.stat().st_size <= 128 * 1024 * 1024 or
            not worker.stat().st_mode & 0o111 or receipt_path.is_symlink() or
            not receipt_path.is_file() or receipt_path.stat().st_size > 16384):
        raise SystemExit('Expected the existing bounded Linux worker package and receipt')
    try:
        receipt = json.loads(receipt_path.read_bytes())
        lock = tomllib.loads((args.native_root / 'packages/nds_native/rust/Cargo.lock').read_text())
        gds = [package for package in lock['package'] if package['name'] == 'nddev-device-sync-gds']
        digest = receipt['sha256']
        if (len(gds) != 1 or receipt['schema_version'] != 1 or
                receipt['worker'] != worker.name or
                receipt['target'] != 'x86_64-unknown-linux-gnu' or
                receipt['source'] != gds[0]['source'] or
                receipt['host_build_environment'] != {'NDS_GDS_WORKER_SHA256': digest} or
                not isinstance(digest, str) or not re.fullmatch('[a-f0-9]{64}', digest) or
                sha(worker) != digest):
            raise ValueError('Worker differs from the locked source or bytes')
    except (KeyError, TypeError, ValueError):
        raise SystemExit('Linux worker receipt differs from the locked source or bytes') from None
    linux_elf(worker)
    return mode, {'name':'linux', 'architecture':'x86_64'}, {
        'path':worker.name, 'sha256':digest, 'receipt_sha256':sha(receipt_path),
        'source':receipt['source'], 'target':receipt['target']}

artifact = args.artifact.resolve()
linux = None
if artifact.is_file(): files = {artifact.name: sha(artifact)}
elif artifact.is_dir():
    paths = sorted(path for path in artifact.rglob('*') if path.is_file())
    if not paths or len(paths)>32768: raise SystemExit('Unexpected artifact file count')
    linux = linux_bundle(artifact, paths)
    if linux is not None and args.output.resolve().is_relative_to(artifact):
        raise SystemExit('Provenance output must remain outside the Linux bundle')
    files = {str(path.relative_to(artifact)):sha(path) for path in paths}
    if linux is not None and (files.get(linux[2]['path']) != linux[2]['sha256'] or
                              files.get('nds-gds-worker.build.json') != linux[2]['receipt_sha256']):
        raise SystemExit('Linux worker changed while recording the bundle')
else: raise SystemExit('Built artifact does not exist')
app_source = identity(args.app_root)
native_source = identity(args.native_root)
if any(source['state'] != 'committed' for source in (app_source, native_source)):
    raise SystemExit('Committed wrapper and native sources are required')
versions = re.findall(r'^version: (\S+)\s*$', (args.app_root/'pubspec.yaml').read_text(), re.MULTILINE)
if len(versions) != 1:
    raise SystemExit('Expected one actual wrapper pubspec version')
subprocess.run([sys.executable, str(Path(__file__).with_name('check-artifact.py')),
                str(artifact), '--native-source-commit', native_source['commit']],
               check=True, timeout=120)
record = {'build_kind':linux[0] if linux is not None else args.build_kind or 'debug',
          'wrapper_version':versions[0], 'app_source':app_source,
          'native_source':native_source,
          'standards':tomllib.loads((args.app_root/'standarts.lock').read_text()),
          'toolchains':json.loads((args.native_root/'toolchains.lock.json').read_text()),
          'rust_toolchain':tomllib.loads((args.native_root/'packages/nds_native/rust/rust-toolchain.toml').read_text())['toolchain'],
          'locks_sha256':{'app_pubspec':sha(args.app_root/'pubspec.lock'),
                         'native_cargo':sha(args.native_root/'packages/nds_native/rust/Cargo.lock')},
          'artifact_files_sha256':files}
if args.product_version is not None:
    # A product label is not compatibility, signing or release authorization.
    record['product_version'] = args.product_version
if linux is not None:
    record['platform'], record['worker'] = linux[1], linux[2]
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
