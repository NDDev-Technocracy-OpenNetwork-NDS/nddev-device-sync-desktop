#!/usr/bin/env python3
"""Drive the existing Linux identity UI with real signed staging and an owned Vault.

The caller supplies an existing isolated PG/SMTP receipt and an independently
approved assembly policy. The controller serves exact signed fixture bytes,
reads only its fresh client journal, and never substitutes authentication,
cryptography, a credential store, or operation receipts.
"""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import signal
import sqlite3
import subprocess
import sys
import threading
import time
from urllib.parse import urlsplit
import uuid


def load(path, limit=65536):
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise RuntimeError('Expected a bounded regular fixture file')
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result: raise RuntimeError('Duplicate fixture JSON field')
            result[key] = value
        return result
    return json.loads(path.read_text(), object_pairs_hook=unique)


def sha(path):
    with path.open('rb') as stream: return hashlib.file_digest(stream, 'sha256').hexdigest()


def atomic_json(path, data):
    temporary = path.with_name(path.name + '.' + uuid.uuid4().hex + '.tmp')
    temporary.write_text(json.dumps(data, sort_keys=True) + '\n')
    temporary.replace(path)


class Downloads:
    def __init__(self, root, policy, manifest):
        self.mode = 'failure'
        self.epoch = 0
        self.requests = 0
        self.guard = threading.Lock()
        self.stop = threading.Event()
        paths = [urlsplit(policy[key]) for key in ['manifest_url', 'bundle_url']]
        artifact = urlsplit(manifest['artifact']['url'])
        for url in [*paths, artifact]:
            if (url.scheme != 'http' or url.hostname != '127.0.0.1' or
                    url.port is None or not 1024 <= url.port <= 65535 or
                    url.username or url.password or url.fragment or url.query or
                    url.netloc != f'127.0.0.1:{url.port}' or url.port != artifact.port):
                raise RuntimeError('UI qualification requires one approved literal loopback origin')
        origin = f'http://127.0.0.1:{artifact.port}'
        if origin not in policy['allowed_origins']: raise RuntimeError('Fixture origin is not approved')
        payload = root / 'payload'
        if payload.is_symlink() or not payload.is_file() or not 65536 <= payload.stat().st_size <= 4194304:
            raise RuntimeError('Use a bounded 64KiB..4MiB opaque qualification payload')
        if payload.stat().st_size != manifest['artifact']['bytes'] or sha(payload) != manifest['artifact']['sha256']:
            raise RuntimeError('Opaque fixture bytes do not match the signed description')
        bundle_path = root / 'bundle.json'
        if bundle_path.is_symlink() or not bundle_path.is_file() or not 1 <= bundle_path.stat().st_size <= 262144:
            raise RuntimeError('Expected a bounded regular Sigstore bundle')
        self.bodies = {paths[0].path: (root / 'app-release.json').read_bytes(),
                       paths[1].path: (root / 'bundle.json').read_bytes(), artifact.path: payload.read_bytes()}
        if len(self.bodies) != 3: raise RuntimeError('Fixture routes must be distinct')
        owner = self
        class Handler(BaseHTTPRequestHandler):
            def setup(self):
                super().setup()
                self.connection.settimeout(3)
            def log_message(self, *_): pass
            def do_GET(self):
                with owner.guard:
                    owner.requests += 1
                    mode, epoch = owner.mode, owner.epoch
                    exhausted = owner.requests > 128
                body = owner.bodies.get(self.path)
                if exhausted or mode in ['failure', 'offline'] or body is None:
                    self.send_response(503); self.send_header('Content-Length', '0'); self.end_headers(); return
                self.send_response(200); self.send_header('Content-Length', str(len(body)))
                self.send_header('Connection', 'close'); self.end_headers()
                try:
                    if self.path == artifact.path and mode == 'slow':
                        self.wfile.write(body[:32768]); self.wfile.flush()
                        deadline = time.monotonic() + 90
                        while not owner.stop.wait(.05) and time.monotonic() < deadline:
                            with owner.guard:
                                if owner.epoch != epoch: break
                    else: self.wfile.write(body)
                except (BrokenPipeError, ConnectionResetError, TimeoutError): pass
                self.close_connection = True
        class Server(ThreadingHTTPServer):
            daemon_threads = False
            allow_reuse_address = False
            def __init__(self, *args):
                self.capacity = threading.BoundedSemaphore(4)
                super().__init__(*args)
            def process_request(self, request, address):
                if not self.capacity.acquire(blocking=False):
                    self.shutdown_request(request); return
                try: super().process_request(request, address)
                except BaseException:
                    self.capacity.release(); raise
            def process_request_thread(self, request, address):
                try: super().process_request_thread(request, address)
                finally: self.capacity.release()
        self.server = Server(('127.0.0.1', artifact.port), Handler)
        self.worker = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.worker.start()
    def select(self, mode):
        with self.guard: self.mode = mode; self.epoch += 1; return self.requests
    def close(self):
        self.stop.set(); self.server.shutdown(); self.server.server_close(); self.worker.join(timeout=5)
        if self.worker.is_alive(): raise RuntimeError('Owned download server did not stop')


def fixture_mode(receipt, original, mode):
    current = load(receipt)
    control = receipt.resolve().with_suffix('.control.json')
    if (Path(original['control_file']).resolve() != control or
            current['base_url'] != original['base_url'] or current['owner_email'] != original['owner_email']):
        raise RuntimeError('Owned fixture identity/control changed')
    atomic_json(control, {'mode': mode})
    wanted = 'offline' if mode == 'offline' else 'running'
    deadline = time.monotonic() + 20
    while load(receipt).get('status') != wanted:
        if time.monotonic() > deadline: raise RuntimeError('Owned backend did not reach requested mode')
        time.sleep(.1)


def pending_snapshot(state, identity, expected_schema):
    databases = list((state / 'nddev-device-sync/sync').glob('*.db'))
    if not 1 <= len(databases) <= 8: raise RuntimeError('Unexpected owned journal inventory')
    found = []
    for path in databases:
        if path.is_symlink(): raise RuntimeError('Refuse a symlinked journal')
        with sqlite3.connect(path.as_uri() + '?mode=ro', uri=True, timeout=.5) as db:
            db.execute('PRAGMA query_only=ON')
            version = db.execute('SELECT schema_version FROM v3_meta WHERE singleton=1').fetchone()
            if version != (expected_schema,): raise RuntimeError('Actual owned journal format differs')
            row = db.execute("SELECT id,kind,entity_id,body,state FROM v3_outbox WHERE id=?", (identity['pending_id'],)).fetchone()
            if row:
                if row[1:3] != ('vault_record', identity['record_id']) or row[4] != 'pending':
                    raise RuntimeError('Original operation scope/state differs')
                body = bytes(row[3])
                if not 1 <= len(body) <= 24576: raise RuntimeError('Unexpected original operation size')
                wire = json.loads(body)
                if (wire['operation_id'] != identity['pending_id'] or wire['device_id'] != identity['device_id'] or
                        wire['entity_id'] != identity['record_id'] or wire['entity_type'] != 'vault_record' or
                        wire['payload']['key_id'] != identity['key_id'] or
                        b'SYNTHETIC_APP_UPDATE_PENDING' in body):
                    raise RuntimeError('Actual encrypted pending original differs from UI scope')
                found.append({'database': path, 'body': body, 'wire': wire, 'sha256': hashlib.sha256(body).hexdigest()})
    if len(found) != 1: raise RuntimeError('Expected exactly one actual pending original')
    return found[0]


def receipt_snapshot(original, identity):
    with sqlite3.connect(original['database'].as_uri() + '?mode=ro', uri=True, timeout=.5) as db:
        db.execute('PRAGMA query_only=ON')
        if db.execute('SELECT count(*) FROM v3_outbox WHERE id=?', (identity['pending_id'],)).fetchone() != (0,):
            raise RuntimeError('Original operation is still pending')
        row = db.execute("SELECT body,receipt_body,receipt_status,server_seq FROM v3_entities WHERE kind='vault_record' AND id=?", (identity['record_id'],)).fetchone()
        if row is None or row[2] != 201 or row[3] <= 0: raise RuntimeError('Actual committed server receipt is absent')
        wire, receipt = json.loads(bytes(row[0])), json.loads(bytes(row[1]))
        if (wire != original['wire'] or receipt['operation_id'] != identity['pending_id'] or
                wire['device_id'] != identity['device_id'] or receipt['server_seq'] != row[3]):
            raise RuntimeError('Server receipt did not retain the original operation')
        return {'server_seq': row[3], 'receipt_sha256': hashlib.sha256(bytes(row[1])).hexdigest()}


def bundle_snapshot(app):
    root = app / 'build/linux/x64/debug/bundle'
    files = [p for p in root.rglob('*') if p.is_file()]
    if not files or len(files) > 1024: raise RuntimeError('Expected the actual owned Linux debug app bundle')
    return {str(p.relative_to(root)): sha(p) for p in sorted(files)}


def terminate_owned(process, executable, command):
    if process.poll() is not None: return
    try:
        actual = Path(f'/proc/{process.pid}/exe').resolve(strict=True)
        args = Path(f'/proc/{process.pid}/cmdline').read_bytes().split(b'\0')
    except FileNotFoundError: return
    if actual != executable or os.fsencode(str(command)) not in args:
        raise RuntimeError('Refuse termination: disposable child PID/executable differs')
    process.terminate()
    try: process.wait(timeout=150)
    except subprocess.TimeoutExpired:
        if Path(f'/proc/{process.pid}/exe').resolve(strict=True) != executable:
            raise RuntimeError('Refuse changed child executable')
        process.kill(); process.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--signed-fixture', type=Path, required=True)
    parser.add_argument('--policy', type=Path, required=True, help='independently approved COMPILE-TIME qualification policy')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--app', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--offline-pub', action='store_true')
    args = parser.parse_args()
    if sys.platform != 'linux' or os.environ.get('NDS_ISOLATED_PROVIDER') != '1' or os.environ.get('NDS_OWNED_XVFB') != '1':
        parser.error('Use an explicitly owned Linux Xvfb/native-provider fixture')
    if not all(p.is_absolute() for p in [args.fixture, args.signed_fixture, args.policy, args.output, args.app]):
        parser.error('All fixture/build/evidence paths must be absolute')
    app, output = args.app.resolve(), args.output.resolve()
    if output.exists() or output.is_relative_to(app): parser.error('Evidence needs a fresh private directory outside the app')
    backend = load(args.fixture)
    for key in ['base_url', 'mailpit_url']:
        url = urlsplit(backend[key])
        if url.scheme != 'http' or url.hostname != '127.0.0.1' or not url.port or url.username or url.password:
            parser.error('Only the existing owned loopback backend is allowed')
    if backend.get('status') != 'running' or not backend['owner_email'].endswith('@example.invalid'):
        parser.error('Expected a running synthetic owned identity fixture')
    if args.policy.resolve().is_relative_to(args.signed_fixture.resolve()):
        parser.error('Approved assembly policy must be supplied independently of downloaded fixture files')
    policy = load(args.policy)
    manifest = load(args.signed_fixture / 'app-release.json')
    # This is transport/fixture setup, not a signature/compatibility verifier.
    # The actual Rust client must authenticate the document and reject failures.
    if subprocess.check_output(['git', 'status', '--porcelain'], cwd=app, text=True).strip():
        parser.error('Use a clean signed qualification source')
    protected = [app / name for name in ['pubspec.yaml', 'pubspec.lock', 'packages/nds_native/rust/Cargo.toml', 'packages/nds_native/rust/Cargo.lock']]
    source_files = {str(p.relative_to(app)): sha(p) for p in protected}
    output.mkdir(mode=0o700, parents=True)
    handoff = output / ('nds-app-update-ui-' + uuid.uuid4().hex); handoff.mkdir(mode=0o700)
    state = output / 'state'; state.mkdir(mode=0o700)
    atomic_json(handoff / 'expectation.json', {'version': manifest['version']})
    environment = os.environ.copy()
    environment['XDG_STATE_HOME'] = str(state)
    environment.pop('NDS_APP_UPDATE_POLICY', None)
    environment['CARGO_BUILD_JOBS'] = '2'
    command = app / 'scripts/identity-acceptance.py'
    argv = [sys.executable, str(command), '--fixture', str(args.fixture), '--app', str(app),
            '--device', 'linux', '--vault', '--app-update-handoff', str(handoff), '--app-update-policy', str(args.policy)]
    if args.offline_pub: argv.append('--offline-pub')
    downloads = Downloads(args.signed_fixture, policy, manifest)
    process = None; initial_bundle = None; original = None; identity = None; checks = []; restore_requests = None; result = None
    events = ['online-ready', 'offline-requested', 'pending-created', 'discovery-failure', 'after-discovery',
              'partial-cancel', 'download-partial', 'after-cancel', 'verified-stage', 'after-stage',
              'local-restore', 'after-restore', 'reconnect-requested', 'after-reconnect', 'complete']
    receipt_data = None
    try:
        with (output / 'controller-child.log').open('w') as log:
            process = subprocess.Popen(argv, cwd=app, env=environment, stdout=log, stderr=subprocess.STDOUT)
        for event in events:
            fact = handoff / (event + '.json'); deadline = time.monotonic() + (900 if event == 'online-ready' else 120)
            while not fact.exists():
                if process.poll() is not None: raise RuntimeError('Owned UI child exited without the required phase receipt')
                if time.monotonic() > deadline: raise RuntimeError('Owned UI phase timed out: ' + event)
                time.sleep(.1)
            data = load(fact, 8192)
            if event == 'online-ready':
                initial_bundle = bundle_snapshot(app)
            elif event == 'offline-requested': fixture_mode(args.fixture, backend, 'offline')
            elif event == 'pending-created':
                identity = data; original = pending_snapshot(state, identity, manifest['data']['vault_journal'])
                if original['wire']['schema_version'] != manifest['data']['sync_wire']:
                    raise RuntimeError('Actual queued wire version differs from the candidate contract')
            elif event == 'discovery-failure': downloads.select('failure')
            elif event == 'partial-cancel': downloads.select('slow')
            elif event == 'verified-stage': downloads.select('good')
            elif event == 'local-restore': restore_requests = downloads.select('offline')
            if original is not None and event not in ['after-reconnect', 'complete']:
                observed = pending_snapshot(state, identity, manifest['data']['vault_journal'])
                if observed['sha256'] != original['sha256']: raise RuntimeError('Original encrypted pending bytes changed')
                checks.append(event)
            staging = state / 'nds/app-updates'
            if event == 'download-partial':
                if not any((p / 'payload').is_file() and 0 < (p / 'payload').stat().st_size < manifest['artifact']['bytes'] for p in staging.glob('work-*')):
                    raise RuntimeError('Expected actual partial bytes before UI cancellation')
            if event in ['after-discovery', 'after-cancel']:
                if (staging / 'current').exists() or list(staging.glob('work-*')):
                    raise RuntimeError('Failed/cancelled staging published or retained partial bytes')
            if event in ['after-stage', 'after-restore', 'after-reconnect']:
                pointer = (staging / 'current').read_text()
                if pointer != sha(args.signed_fixture / 'app-release.json') or sha(staging / pointer / 'payload') != manifest['artifact']['sha256'] or list(staging.glob('work-*')):
                    raise RuntimeError('Actual staged candidate differs or partial work remains')
                if bundle_snapshot(app) != initial_bundle: raise RuntimeError('Installed debug application files changed')
            if event == 'after-restore':
                with downloads.guard:
                    if downloads.requests != restore_requests: raise RuntimeError('Local restore unexpectedly used the download network')
                if not data.get('fresh_native_store_and_vault_read') or data['key_id'] != identity['key_id'] or data['device_id'] != identity['device_id']:
                    raise RuntimeError('Fresh protected-store/key availability was not confirmed')
            if event == 'reconnect-requested': fixture_mode(args.fixture, backend, 'online')
            if event == 'after-reconnect':
                result = receipt_snapshot(original, identity)
                if data.get('manual_sync') is not False or data['key_id'] != identity['key_id'] or data['device_id'] != identity['device_id']:
                    raise RuntimeError('Original client/scope changed during replay')
            if event == 'complete' and not (data.get('passed') and data.get('fixture_signed_out')):
                raise RuntimeError('Synthetic fixture cleanup was not confirmed')
            atomic_json(handoff / (event + '.ready'), {'ok': True})
        status = process.wait(timeout=120)
        text = (handoff / 'flutter-driver.log').read_text()
        if status != 0 or 'NDS_APP_UPDATE_PRESERVATION_PASS' not in text or 'All tests passed.' not in text:
            raise RuntimeError('No completed passing identity/UI runner receipt')
        receipt_data = {'scope': 'Actual Linux Flutter + native protected-store reads + encrypted original + original background replay; opaque signed staging fixture, no activation/distribution claim',
            'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=app, text=True).strip(),
            'manifest_sha256': sha(args.signed_fixture / 'app-release.json'), 'original_operation_id': identity['pending_id'],
            'original_encrypted_body_sha256': original['sha256'], 'preservation_checkpoints': checks,
            'actual_server_receipt': result, 'fresh_native_store_and_vault_read': True, 'installed_bundle_unchanged': True,
            'manual_sync': False, 'ui_child_exit': status, 'download_network_unused_for_restore': True,
            'cleanup': {'ui_child_joined': True, 'download_server_joined': True, 'backend_restored': True, 'isolated_native_provider': 'caller must independently verify cleanup'}}
    finally:
        if process is not None: terminate_owned(process, Path(sys.executable).resolve(), command)
        downloads.close()
        if load(args.fixture).get('status') == 'offline': fixture_mode(args.fixture, backend, 'online')
    if {str(p.relative_to(app)): sha(p) for p in protected} != source_files:
        raise RuntimeError('Owned integration configuration was not restored exactly')
    if receipt_data is not None:
        receipt_data['source_inputs_restored'] = source_files
        atomic_json(output / 'preservation-receipt.json', receipt_data)
        print('Actual application staging/UI preservation completed; private receipt:', output / 'preservation-receipt.json')


if __name__ == '__main__': main()
