#!/usr/bin/env python3
"""Build or test with verified native inputs; restore the app configuration."""
import argparse
import os
import json
import tempfile
from pathlib import Path
import re
import signal
import shutil
import subprocess
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('flutter_arguments', nargs=argparse.REMAINDER)
args = parser.parse_args()
if not args.flutter_arguments:
    parser.error('a Flutter build target is required')
source = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
sys.path.insert(0, str(source / 'packages/nds_native/tool'))
from prepare_native import build_environment
from native_worker import package_linux_worker, deliver_linux_worker
app = Path.cwd()
spec = app / 'pubspec.yaml'
original = spec.read_bytes()
arguments = list(args.flutter_arguments)
is_test = arguments[0] == 'test'
# Apple bundle versions are numeric. Keep the candidate/build number in the
# owning pubspec instead of duplicating alpha build numbers in CI commands.
if arguments[0] in ('ios', 'macos') and not any(arg.startswith('--build-name') for arg in arguments):
    version = re.search(r'^version: (\d+\.\d+\.\d+)(?:-[0-9A-Za-z.-]+)?(?:\+\d+)?\s*$', original.decode(), re.MULTILINE)
    if version is None:
        raise SystemExit('Expected an explicit native application version')
    arguments.append('--build-name=' + version[1])
if re.search(r'^hooks:', original.decode(), re.MULTILINE):
    raise SystemExit('Refuse to replace existing hook configuration')
commit = subprocess.check_output(['git','rev-parse','HEAD'], cwd=source, text=True).strip()
if not re.fullmatch('[a-f0-9]{40}', commit):
    raise SystemExit('Native source requires a Git commit')
dirty = subprocess.check_output(['git','status','--porcelain','--untracked-files=no'], cwd=source, text=True).strip()
if dirty and not is_test:
    raise SystemExit('Committed native source is required; tracked drift:\n' + '\n'.join(dirty.splitlines()[:32]))
if (app != source and not (is_test and app == source / 'packages/nds_ui')
        and f'ref: {commit}' not in original.decode()):
    raise SystemExit('The app must consume this exact shared native source')
print('Building native source: ' + commit + (' (uncommitted)' if dirty else ''), flush=True)
os.environ.update(build_environment())
# Resolve from the actual app directory: a Git/Pub native dependency may live
# outside the installation's parent Cargo configuration scope.
metadata = json.loads(subprocess.check_output(
    ['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1',
     '--manifest-path', str(source / 'packages/nds_native/rust/Cargo.toml')],
    cwd=app, text=True, timeout=30))
os.environ.setdefault('CARGO_TARGET_DIR', metadata['target_directory'])
os.environ.setdefault('CARGO_BUILD_BUILD_DIR', metadata.get('build_directory', metadata['target_directory']))
if is_test:
    os.environ.setdefault('FRB_DART_LOAD_EXTERNAL_LIBRARY_NATIVE_LIB_DIR',
                          str(Path(metadata['target_directory']) / 'debug'))
def interrupted(_signal, _frame):
    raise KeyboardInterrupt
handlers = {sig: signal.signal(sig, interrupted) for sig in [signal.SIGTERM, signal.SIGINT]}
worker_staging = None
try:
    worker_digest = None
    if arguments[0] == 'linux':
        worker_staging = tempfile.TemporaryDirectory(prefix='nds-gds-worker-build-')
        worker_digest = package_linux_worker(source, Path(worker_staging.name))
    worker_define = '' if worker_digest is None else f'      gds_worker_sha256: "{worker_digest}"\n'
    cache_define = '      native_build_cache: ' + json.dumps(os.environ['NDS_NATIVE_BUILD_HOME']) + '\n'
    python_define = '      native_build_python: ' + json.dumps(sys.executable) + '\n'
    source_define = '' if dirty else f'      source_commit: "{commit}"\n'
    spec.write_bytes(original + f'\nhooks:\n  user_defines:\n    nds_native:\n{source_define}{worker_define}{cache_define}{python_define}'.encode())
    environment = os.environ.copy()
    environment['CARGO_BUILD_JOBS'] = '4'
    executable = shutil.which('flutter')
    if executable is None:
        raise SystemExit('Flutter is unavailable')
    process = subprocess.Popen([executable,*([] if is_test else ['build']),*arguments],
        env=environment, start_new_session=os.name != 'nt')
    try:
        status = process.wait(timeout=1800)
    except (subprocess.TimeoutExpired, KeyboardInterrupt):
        if os.name == 'nt':
            subprocess.run(['taskkill','/PID',str(process.pid),'/T','/F'],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
        else:
            os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            if os.name != 'nt': os.killpg(process.pid, signal.SIGKILL)
            else: process.kill()
            process.wait(timeout=10)
        raise
    if status == 0 and worker_staging is not None:
        mode = 'debug' if '--debug' in arguments else 'profile' if '--profile' in arguments else 'release'
        bundle = app / 'build/linux/x64' / mode / 'bundle'
        deliver_linux_worker(Path(worker_staging.name), bundle, worker_digest)
    raise SystemExit(status)
finally:
    spec.write_bytes(original)
    for sig, handler in handlers.items(): signal.signal(sig, handler)
    if worker_staging is not None: worker_staging.cleanup()
