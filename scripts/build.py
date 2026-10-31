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
import stat
import subprocess
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--app-update-policy', type=Path, metavar='FILE',
                    help='Trusted assembly JSON policy; specify before the Flutter target')
parser.add_argument('flutter_arguments', nargs=argparse.REMAINDER)
args = parser.parse_args()
if not args.flutter_arguments:
    parser.error('a Flutter build target is required')

def invalid_json_constant(_value):
    raise ValueError('Invalid JSON constant')

def read_policy(path):
    if path is None:
        return ''
    try:
        if path.is_symlink():
            raise ValueError('Policy must be a regular file')
        flags = os.O_RDONLY | getattr(os, 'O_NONBLOCK', 0) | getattr(os, 'O_NOFOLLOW', 0)
        with os.fdopen(os.open(path, flags), 'rb') as policy_file:
            if not stat.S_ISREG(os.fstat(policy_file.fileno()).st_mode):
                raise ValueError('Policy must be a regular file')
            payload = policy_file.read(65537)
        if len(payload) > 65536:
            raise ValueError('Policy exceeds its bound')
        policy = payload.decode('utf-8')
        if not isinstance(json.loads(policy, parse_constant=invalid_json_constant), dict):
            raise ValueError('Policy must be an object')
        return policy
    except (OSError, UnicodeError, ValueError, RecursionError):
        # Never include policy contents in parser, helper or compiler diagnostics.
        raise SystemExit('App-update policy requires a regular UTF-8 JSON object file of at most 65536 bytes') from None

def checked_build_directory(value):
    try:
        if (not isinstance(value, str) or not Path(value).is_absolute()
                or len(value) > 32768
                or len(value.encode('utf-8')) > 32768
                or re.search(r'[\x00-\x1f\x7f-\x9f]', value)):
            raise ValueError
    except (ValueError, UnicodeError):
        raise SystemExit('Prepared Cargo build directory requires an absolute UTF-8 path without controls of at most 32768 bytes') from None
    return value

app_update_policy = read_policy(args.app_update_policy)
# Only the tracked hook input may configure the native library. Preparation,
# worker packaging and Flutter must not inherit an ambient policy authority.
os.environ.pop('NDS_APP_UPDATE_POLICY', None)
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
managed = bool(os.environ.get('NDS_CACHE_FRONTENDS'))
rust = source / 'packages/nds_native/rust'
# Match the managed build's invocation identity when resolving its final library.
manifest = [] if managed else ['--manifest-path', str(rust / 'Cargo.toml')]
metadata = json.loads(subprocess.check_output(
    ['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1', *manifest],
    cwd=rust if managed else app, text=True, timeout=30))
native_build_directory = checked_build_directory(metadata.get('build_directory', metadata['target_directory']))
os.environ.setdefault('CARGO_TARGET_DIR', metadata['target_directory'])
os.environ.setdefault('CARGO_BUILD_BUILD_DIR', native_build_directory)
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
        worker_staging = tempfile.TemporaryDirectory(prefix='nds-github-worker-build-')
        worker_digest = package_linux_worker(source, Path(worker_staging.name))
    worker_define = '' if worker_digest is None else f'      github_worker_sha256: "{worker_digest}"\n'
    cache_define = '      native_build_cache: ' + json.dumps(os.environ['NDS_NATIVE_BUILD_HOME']) + '\n'
    directory_define = '      native_build_directory: ' + json.dumps(native_build_directory) + '\n'
    python_define = '      native_build_python: ' + json.dumps(sys.executable) + '\n'
    source_define = '' if dirty else f'      source_commit: "{commit}"\n'
    policy_define = '      app_update_policy: ' + json.dumps(app_update_policy) + '\n'
    spec.write_bytes(original + f'\nhooks:\n  user_defines:\n    nds_native:\n{source_define}{worker_define}{cache_define}{directory_define}{python_define}{policy_define}'.encode())
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
