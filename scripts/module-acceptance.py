#!/usr/bin/env python3
"""Exercise real local modules with a separate native clipboard archive."""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
from integration_runner import integration_runner
from native_worker import package_linux_worker, deliver_linux_worker
from contextlib import ExitStack

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--fixture', type=Path, required=True)
parser.add_argument('--clipboard-daemon', type=Path, required=True)
args = parser.parse_args()
if not args.clipboard_daemon.is_absolute() or not args.clipboard_daemon.is_file():
    raise SystemExit('An absolute installed native daemon path is required')
fixture_identity = json.loads(args.fixture.read_text())
from urllib.parse import urlparse
if fixture_identity.get('status') != 'running' or any(urlparse(fixture_identity[key]).scheme != 'http' or urlparse(fixture_identity[key]).hostname != '127.0.0.1' for key in ['base_url','mailpit_url']):
    raise SystemExit('An explicitly owned running loopback identity fixture is required')
root = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(root / 'packages/nds_native/tool'))
from prepare_native import build_environment
os.environ.update(build_environment())
helper = root / 'integration_test/fixture_identity.dart'
if helper.exists(): raise SystemExit('Refuse to overwrite existing fixture helper')
target = root / 'integration_test/local_modules_generated_test.dart'
driver = root / 'integration_test/driver_generated.dart'
if target.exists() or driver.exists():
    raise SystemExit('Refuse to overwrite an existing generated test target')
target.parent.mkdir(exist_ok=True)
log_fd, log_name = tempfile.mkstemp(prefix='nds-native-ui-', suffix='.log')
os.close(log_fd)
log = Path(log_name)
with tempfile.TemporaryDirectory(prefix='nds-ui-providers.', dir='/tmp') as directory:
    fixture = Path(directory)
    archive = fixture / 'clipboard'
    archive.mkdir(mode=0o700)
    repository = fixture / 'repository'
    subprocess.run(['git', 'init', '--quiet', '--initial-branch=fixture', str(repository)], check=True, timeout=10)
    (repository / 'fixture.txt').write_text('Isolated repository status input.\n')
    environment = os.environ.copy()
    environment.update(RLDYOUR_CLIPBOARD_HOME=str(archive), RLDYOUR_CLIPBOARD_CAPTURE='0',
                       RLDYOUR_CLIPBOARD_BUDGET='8388608', CARGO_BUILD_JOBS='4')
    daemon = subprocess.Popen([str(args.clipboard_daemon)], env=environment, stdin=subprocess.DEVNULL,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    runner = ExitStack()
    try:
        staging = fixture / 'worker'
        digest = package_linux_worker(root, staging)
        runner.enter_context(integration_runner(root, development_http=True, worker_digest=digest))
        deadline = time.monotonic() + 10
        while True:
            try:
                connection = socket.socket(socket.AF_UNIX)
                connection.settimeout(2)
                connection.connect(str(archive / 'rldyour-clipboard.sock'))
                break
            except OSError:
                connection.close()
                if time.monotonic() >= deadline or daemon.poll() is not None:
                    raise RuntimeError('Owned clipboard daemon startup failed')
                time.sleep(.025)
        with connection, connection.makefile('rb') as reader:
            def response():
                line = reader.readline(8193)
                if len(line) > 8192 or not line.endswith(b'\n'):
                    raise RuntimeError('Invalid bounded clipboard frame')
                return json.loads(line)
            def request(value, payload=b''):
                connection.sendall(json.dumps(value).encode() + b'\n' + payload)
                return response()
            assert request({'op':'hello','v':1,'role':'capture'})['ev'] == 'hello'
            draft = request({'op':'begin','req':1})['draft']
            payload = 'NDS isolated archive: Русский / English'.encode()
            assert request({'op':'part','req':2,'draft':draft,'mime':'text/plain;charset=utf-8','bytes':len(payload)}, payload)['ev'] == 'ok'
            assert request({'op':'commit','req':3,'draft':draft})['entry'] > 0
        shutil.copyfile(root / 'packages/nds_ui/integration_test/fixture_identity.dart', helper)
        shutil.copyfile(root / 'packages/nds_ui/integration_test/local_modules_test.dart', target)
        shutil.copyfile(root / 'packages/nds_ui/integration_test/driver.dart', driver)
        with log.open('w') as output:
            defines = [
                '--dart-define=NDS_TEST_SERVER='+fixture_identity['base_url'],
                '--dart-define=NDS_TEST_MAILPIT='+fixture_identity['mailpit_url'],
                '--dart-define=NDS_TEST_OWNER='+fixture_identity['owner_email'],
                '--dart-define=NDS_TEST_REPOSITORY='+str(repository),
                '--dart-define=NDS_TEST_RDS_UNAVAILABLE='+str(shutil.which('rds') is None).lower()]
            bundle = root / 'build/linux/x64/debug/bundle'
            build = ['flutter','build','linux','--debug','--no-pub',
                     '--target='+str(target),*defines]
            drive = ['flutter','drive','--target='+str(target),'--driver='+str(driver),
                       '-d','linux','--no-pub','--verbose',
                       '--use-application-binary='+str(bundle / 'nddev_device_sync_desktop'),*defines]
            for command in [build, drive]:
                process = subprocess.Popen(command, cwd=root, env=environment, stdout=output,
                                           stderr=subprocess.STDOUT, start_new_session=True)
                try:
                    status = process.wait(timeout=900)
                except (subprocess.TimeoutExpired, KeyboardInterrupt):
                    os.killpg(process.pid, signal.SIGTERM)
                    try: process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait()
                    raise
                if status != 0:
                    break
                if command is build:
                    deliver_linux_worker(staging, bundle, digest)
        if status == 0:
            subprocess.run(['python3', str(root / 'scripts/check-native-events.py'), str(log)], check=True, timeout=10)
        print(f'Actual native module UI acceptance exit={status}; private log: {log}')
        raise SystemExit(status)
    finally:
        target.unlink(missing_ok=True)
        helper.unlink(missing_ok=True)
        driver.unlink(missing_ok=True)
        try:
            runner.close()
        finally:
            daemon.terminate()
            try: daemon.wait(timeout=5)
            except subprocess.TimeoutExpired:
                daemon.kill()
                daemon.wait()
