#!/usr/bin/env python3
"""Reject integration entrypoints and native test plugins in a built app."""
import argparse
import os
from pathlib import Path
import stat
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('artifact', type=Path)
parser.add_argument('--native-source-commit')
args = parser.parse_args()
markers = [b'identity_acceptance_generated_test', b'local_modules_generated_test',
           b'NDS_TEST_SERVER', b'NDS_TEST_MAILPIT', b'NDS isolated archive:',
           b'IntegrationTestPlugin', b'dev/flutter/plugins/integration_test',
           b'IntegrationTestWidgetsFlutterBinding', b'plugins.flutter.io/integration_test',
           b'package:integration_test/', b'NDS_DIAGNOSTICS_PRIVATE_SENTINEL']
checked = 0
native_source_found = False
native_record_count = 0
expected = args.native_source_commit
if expected is not None:
    import re
    if not re.fullmatch('[a-f0-9]{40}', expected):
        raise SystemExit('Expected an immutable native source SHA')

def native_source(data):
    global native_source_found, native_record_count
    if expected is None: return
    prefix = b'NDS-SOURCE-1\0'
    offset = 0
    while (offset := data.find(prefix, offset)) >= 0:
        body = data[offset+len(prefix):offset+len(prefix)+41]
        offset += len(prefix)
        if len(body) != 41 or body[-1] != 0:
            raise SystemExit('Malformed native source record')
        value = body.rstrip(b'\0')
        if value != b'unknown' and not re.fullmatch(b'[a-f0-9]{40}', value):
            raise SystemExit('Malformed native source identity')
        if value.decode() != expected:
            raise SystemExit('Native source record differs: ' + value.decode())
        native_record_count += 1
        if native_record_count > 32:
            raise SystemExit('Unexpected native source record count')
        native_source_found = True


def regular_file(path):
    if path.is_symlink():
        raise OSError('Regular file required')
    flags = os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0) | getattr(os, 'O_NONBLOCK', 0)
    descriptor = os.open(path, flags)
    if not stat.S_ISREG(os.fstat(descriptor).st_mode):
        os.close(descriptor)
        raise OSError('Regular file required')
    return os.fdopen(descriptor, 'rb')


def linux_bundle(root):
    native = root / 'lib/libnds_native.so'
    if native.exists() or native.is_symlink():
        return True
    parts = root.parts
    if len(parts) >= 5 and parts[-5:-3] == ('build', 'linux') and parts[-1] == 'bundle':
        return True
    # Missing native assets must not turn Linux runners/workers/AOT into an
    # alternative source witness. Do not reclassify nested other-OS resources.
    for path in [root / 'lib/libapp.so', *root.iterdir()]:
        try:
            with regular_file(path) as candidate:
                if candidate.read(4) == b'\x7fELF':
                    return True
        except OSError:
            continue
    return False


def linux_native_source(root):
    path = root / 'lib/libnds_native.so'
    prefix = b'NDS-SOURCE-1\0'
    record_size = len(prefix) + 41
    try:
        if not path.resolve(strict=True).is_relative_to(root.resolve()):
            raise SystemExit('Linux native library must remain inside its bundle')
        with regular_file(path) as candidate:
            before = os.fstat(candidate.fileno())
            if not 0 < before.st_size <= 1024 * 1024 * 1024:
                raise SystemExit('Linux native library is outside its 1 GiB inspection bound')
            if candidate.read(4) != b'\x7fELF':
                raise SystemExit('Expected ELF native asset lib/libnds_native.so')
            candidate.seek(0)
            tail, total = b'', 0
            while chunk := candidate.read(64 * 1024):
                total += len(chunk)
                if total > before.st_size:
                    raise SystemExit('Linux native library changed during inspection')
                data, offset = tail + chunk, 0
                while (found := data.find(prefix, offset)) >= 0:
                    if found + record_size > len(data):
                        tail = data[found:]
                        break
                    native_source(data[found:found + record_size])
                    offset = found + len(prefix)
                else:
                    tail = data[offset:][-(len(prefix) - 1):]
            # A full prefix with a truncated body must retain the same refusal
            # as the whole-file parser; processed records are never recounted.
            native_source(tail)
            after = os.fstat(candidate.fileno())
            if (total != before.st_size or
                    (before.st_size, before.st_mtime_ns, before.st_ctime_ns) !=
                    (after.st_size, after.st_mtime_ns, after.st_ctime_ns)):
                raise SystemExit('Linux native library changed during inspection')
    except OSError:
        raise SystemExit('Expected regular contained Linux native asset lib/libnds_native.so') from None


def check(name, data):
    global checked
    if any(marker in data for marker in markers):
        raise SystemExit('Test harness found in app artifact: ' + name)
    checked += 1

if args.artifact.suffix == '.apk':
    with zipfile.ZipFile(args.artifact) as archive:
        for item in archive.infolist():
            if expected and item.filename.endswith('/libnds_native.so'):
                native_source(archive.read(item))
            if item.filename.endswith(('.dex', 'kernel_blob.bin')):
                if item.file_size > 128 * 1024 * 1024:
                    raise SystemExit('Unexpectedly large app inspection input')
                check(item.filename, archive.read(item))
elif args.artifact.is_dir():
    if any('integration_test' in path.name.lower() for path in args.artifact.rglob('*')):
        raise SystemExit('Native integration test plugin found in app bundle')
    if expected and linux_bundle(args.artifact):
        linux_native_source(args.artifact)
    elif expected:
        for path in args.artifact.rglob('*'):
            if path.is_file() and 0 < path.stat().st_size <= 128 * 1024 * 1024:
                with path.open('rb') as candidate:
                    magic = candidate.read(4)
                    if magic[:2]==b'MZ' or magic in [b'\x7fELF',b'\xcf\xfa\xed\xfe',b'\xfe\xed\xfa\xcf',b'\xca\xfe\xba\xbe',b'\xbe\xba\xfe\xca',b'\xca\xfe\xba\xbf',b'\xbf\xba\xfe\xca']:
                        native_source(magic + candidate.read())
    payloads = [*args.artifact.rglob('kernel_blob.bin'), *args.artifact.rglob('libapp.so')]
    for path in payloads:
        if path.stat().st_size > 128 * 1024 * 1024:
            raise SystemExit('Unexpectedly large Dart app payload')
        check(str(path.relative_to(args.artifact)), path.read_bytes())
else:
    raise SystemExit('Expected an APK or app bundle directory')
if not checked:
    raise SystemExit('No expected Flutter JIT or Linux AOT app payload was found')
if expected and not native_source_found:
    raise SystemExit('Expected native source SHA is absent from built native code')
print(f'Checked {checked} built app payloads; integration harness absent')
if expected: print(f'Verified {native_record_count} retained native source records')
