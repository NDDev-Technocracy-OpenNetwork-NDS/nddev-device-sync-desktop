"""Own the temporary Flutter integration-test dependency and debug hook scope."""
from contextlib import contextmanager
from pathlib import Path
import json
import os
import sys
import re
import signal
import subprocess
from urllib.parse import unquote, urlparse


def pub_get(app, *, offline=False, locked=False):
    command = ['flutter', 'pub', 'get', *(['--enforce-lockfile'] if locked else []),
               *(['--offline'] if offline else [])]
    process = subprocess.Popen(command, cwd=app, stdout=subprocess.DEVNULL,
                               start_new_session=os.name != 'nt')
    try:
        status = process.wait(timeout=120)
    except (subprocess.TimeoutExpired, KeyboardInterrupt):
        if os.name == 'nt':
            subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
        else:
            os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            if os.name == 'nt':
                process.kill()
            else:
                os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=10)
        raise
    if status:
        raise subprocess.CalledProcessError(status, command)


def native_source_commit(app: Path):
    config = app / '.dart_tool/package_config.json'
    packages = json.loads(config.read_text())['packages']
    native = [p for p in packages if p['name'] == 'nds_native']
    if len(native) != 1:
        raise RuntimeError('Expected one selected native package')
    uri = urlparse(native[0]['rootUri'])
    package = (Path(unquote(uri.path)) if uri.scheme == 'file'
               else config.parent / unquote(uri.path)).resolve()
    root = Path(subprocess.check_output(
        ['git', 'rev-parse', '--show-toplevel'], cwd=package, text=True, timeout=10).strip())
    commit = subprocess.check_output(
        ['git', 'rev-parse', 'HEAD'], cwd=root, text=True, timeout=10).strip()
    if not re.fullmatch('[a-f0-9]{40}', commit):
        raise RuntimeError('Expected an immutable native source')
    changed = subprocess.check_output(
        ['git', 'diff', '--name-only', '-z', 'HEAD'], cwd=root, timeout=10).decode().split('\0')
    allowed = {(app / 'pubspec.yaml').resolve(), (app / 'pubspec.lock').resolve()}
    if any((root / name).resolve() not in allowed for name in changed if name):
        raise RuntimeError('Native source has changes outside the owned fixture configuration')
    return commit

@contextmanager
def integration_runner(app: Path, *, development_http=False, worker_digest=None, offline_pub=False):
    spec, lock = app / 'pubspec.yaml', app / 'pubspec.lock'
    original, original_lock = spec.read_bytes(), lock.read_bytes()
    source = original.decode()
    if '  integration_test:' in source or 'hooks:' in source:
        raise RuntimeError('Refuse to replace existing integration/hook configuration')
    if source.count('dev_dependencies:\n') != 1:
        raise RuntimeError('Expected one development-dependency section')
    source = source.replace('dev_dependencies:\n', 'dev_dependencies:\n  integration_test:\n    sdk: flutter\n')
    source += '\nhooks:\n  user_defines:\n    nds_native:\n'
    if worker_digest is not None:
        if not re.fullmatch('[a-f0-9]{64}', worker_digest):
            raise RuntimeError('An immutable worker digest is required')
        source += f'      gds_worker_sha256: "{worker_digest}"\n'
    if development_http:
        source += '      development_http: true\n'
    # The same explicit, receipt-checked cache contract as scripts/build.py.
    # Hooks still verify the selected RDS source, config and build receipt.
    cache = os.environ.get('NDS_NATIVE_BUILD_HOME')
    if cache is not None:
        if not Path(cache).is_absolute():
            raise RuntimeError('Prepared native cache must be an absolute path')
        source += '      native_build_cache: ' + json.dumps(cache) + '\n'
        source += '      native_build_python: ' + json.dumps(sys.executable) + '\n'
    def interrupted(_signal, _frame):
        raise KeyboardInterrupt
    handlers = {sig: signal.signal(sig, interrupted) for sig in [signal.SIGTERM, signal.SIGINT]}
    try:
        spec.write_text(source)
        pub_get(app, offline=offline_pub)
        # Resolve the package actually selected by this app, not the checkout
        # containing the acceptance runner. Only our temporary app files differ.
        commit = native_source_commit(app)
        spec.write_text(source + f'      source_commit: "{commit}"\n')
        yield
    finally:
        spec.write_bytes(original)
        lock.write_bytes(original_lock)
        try:
            pub_get(app, offline=offline_pub, locked=True)
        finally:
            for sig, handler in handlers.items():
                signal.signal(sig, handler)
