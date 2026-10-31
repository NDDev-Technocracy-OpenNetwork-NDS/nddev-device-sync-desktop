#!/usr/bin/env python3
"""Bind an existing Linux qualification bundle; never build, install or sign it."""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tomllib
import traceback
from urllib.parse import parse_qs, urlsplit


ROOT = Path(__file__).resolve().parents[1]
DESKTOP = 'https://github.com/NDDev-Technocracy-OpenNetwork-NDS/nddev-device-sync-desktop'
GITHUB_MODULE = 'https://github.com/NDDev-Technocracy-OpenNetwork-NDS/nddev-device-sync-github'
DESIGN_NAME = 'NDDev-Technocracy-OpenNetwork-NDS/nddev-opennetwork-design-system'
DESIGN_HISTORY = 'NDDev-OpenNetwork/nddev-opennetwork-design-system'
MAX_PAYLOAD = 1024 * 1024 * 1024
MAX_JSON = 8 * 1024 * 1024
MAX_SBOM = 64 * 1024 * 1024
MAX_FILES = 32768
SYFT_CONFIG = '''check-for-app-update: false
parallelism: 2
enrich: []
package:
  search-indexed-archives: false
  search-unindexed-archives: false
file:
  metadata:
    selection: all
    digests: [sha256]
golang:
  use-packages-lib: false
  search-local-mod-cache-licenses: false
  search-local-vendor-licenses: false
  search-remote-licenses: false
java:
  use-network: false
  use-maven-local-repository: false
  resolve-transitive-dependencies: false
javascript:
  search-remote-licenses: false
python:
  search-remote-licenses: false
cpp:
  vcpkg-allow-git-clone: false
'''


def require(condition):
    if not condition:
        raise ValueError('Native qualification input refused')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def hex_value(value, length):
    return isinstance(value, str) and re.fullmatch('[a-f0-9]{' + str(length) + '}', value) is not None


def relative_name(name):
    require(isinstance(name, str) and name and '\\' not in name and '\0' not in name)
    require(not name.startswith('/') and all(part not in ('', '.', '..') for part in name.split('/')))
    require(str(PurePosixPath(name)) == name)
    return name


def regular(path):
    flags = os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0) | getattr(os, 'O_NONBLOCK', 0)
    require(not path.is_symlink())
    handle = os.fdopen(os.open(path, flags), 'rb')
    if not stat.S_ISREG(os.fstat(handle.fileno()).st_mode):
        handle.close()
        raise ValueError('Regular input required')
    return handle


def read_bytes(path, limit=MAX_JSON):
    with regular(path) as handle:
        require(os.fstat(handle.fileno()).st_size <= limit)
        data = handle.read(limit + 1)
    require(len(data) <= limit)
    return data


def file_digest(path, limit=MAX_PAYLOAD):
    value, count = hashlib.sha256(), 0
    with regular(path) as handle:
        before = os.fstat(handle.fileno())
        require(before.st_size <= limit)
        while chunk := handle.read(64 * 1024):
            count += len(chunk)
            require(count <= limit)
            value.update(chunk)
        after = os.fstat(handle.fileno())
    require((before.st_size, before.st_mtime_ns) == (after.st_size, after.st_mtime_ns))
    require(count == before.st_size)
    return value.hexdigest(), count


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result)
        result[key] = value
    return result


def bad_constant(_value):
    raise ValueError('Non-JSON number')


def json_object(data):
    result = json.loads(data.decode('utf-8'), object_pairs_hook=unique_object,
                        parse_constant=bad_constant)
    require(isinstance(result, dict))
    return result


def json_bytes(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False) + '\n').encode()


def write_new(path, data):
    with path.open('xb') as handle:
        handle.write(data)
    path.chmod(0o600)


def command_environment():
    # The prepared tool is selected by A's PATH. No ambient scanner options,
    # credentials, agents, provider configuration or proxy settings reach it.
    return {'PATH': os.environ.get('PATH', os.defpath), 'LANG': 'C.UTF-8',
            'LC_ALL': 'C.UTF-8', 'NO_COLOR': '1', 'GIT_CONFIG_GLOBAL': os.devnull,
            'GIT_CONFIG_NOSYSTEM': '1', 'GIT_TERMINAL_PROMPT': '0',
            'GIT_OPTIONAL_LOCKS': '0'}


def git(directory, *arguments):
    return subprocess.check_output(
        ['git', '-c', 'core.fsmonitor=false', '--no-optional-locks', *arguments],
        cwd=directory, env=command_environment(), text=True, timeout=15).strip()


def source_identity(directory, *, allow_cargo_marker=False):
    require(Path(git(directory, 'rev-parse', '--show-toplevel')).resolve() == directory)
    commit = git(directory, 'rev-parse', 'HEAD')
    require(hex_value(commit, 40))
    status = git(directory, 'status', '--porcelain')
    require(not status or (allow_cargo_marker and status == '?? .cargo-ok'))
    if allow_cargo_marker:
        # Cargo's checkout sentinel is the only permitted untracked input.
        # Inspect an existing marker even when a Git ignore rule hides it.
        marker = directory / '.cargo-ok'
        try:
            marker.lstat()
        except FileNotFoundError:
            require(not status)
        else:
            require(read_bytes(marker, 0) == b'')
    return commit


def inventory(bundle):
    require(bundle.is_dir() and not bundle.is_symlink())
    result, total, entries = {}, 0, 0
    for parent, directories, files in os.walk(bundle, followlinks=False):
        for name in directories + files:
            entries += 1
            require(entries <= 2 * MAX_FILES)
            path = Path(parent) / name
            relative = relative_name(path.relative_to(bundle).as_posix())
            metadata = path.lstat()
            require(not path.is_symlink() and (stat.S_ISDIR(metadata.st_mode) or stat.S_ISREG(metadata.st_mode)))
            if stat.S_ISREG(metadata.st_mode):
                checksum, size = file_digest(path)
                total += size
                require(total <= MAX_PAYLOAD and len(result) < MAX_FILES)
                result[relative] = checksum
    require(result)
    return result


def git_source(source):
    require(isinstance(source, str) and source.startswith('git+'))
    url = urlsplit(source[4:])
    require(url.scheme == 'https' and url.netloc == 'github.com' and hex_value(url.fragment, 40))
    require(parse_qs(url.query, strict_parsing=True) == {'rev': [url.fragment]})
    require(url.query == 'rev=' + url.fragment)
    repository = 'https://github.com' + url.path.removesuffix('.git')
    require(re.fullmatch(r'https://github\.com/[A-Za-z0-9_.-]{1,100}/[A-Za-z0-9_.-]{1,100}', repository))
    require(len(repository) <= 222)
    return repository, url.fragment


def source_graph(commit, shared, worker, standards, design, worker_commit):
    sources = {}

    def add(role, repository, revision):
        require(re.fullmatch('[a-z0-9_.-]{1,64}', role) and hex_value(revision, 40))
        require(re.fullmatch(r'https://github\.com/[A-Za-z0-9_.-]{1,100}/[A-Za-z0-9_.-]{1,100}', repository))
        require(len(repository) <= 222 and role not in sources)
        sources[role] = {'role': role, 'repository': repository, 'commit': revision}

    add('desktop', DESKTOP, commit)
    add('worker', GITHUB_MODULE, worker_commit)
    for lock, prefix in [(shared, 'crate.'), (worker, 'worker.crate.')]:
        require(isinstance(lock.get('package'), list))
        for package in lock['package']:
            require(isinstance(package, dict))
            source = package.get('source', '')
            require(isinstance(source, str))
            if source.startswith('git+'):
                repository, revision = git_source(source)
                add(prefix + package['name'], repository, revision)
    add('standards', 'https://github.com/' + standards['source_repo'], standards['source_commit'])
    require(design['source']['repository'] in (DESIGN_NAME, DESIGN_HISTORY))
    add('design-system', 'https://github.com/' + DESIGN_NAME, design['source']['commit'])
    require(len(sources) <= 64)
    return [sources[key] for key in sorted(sources)]


def run_logged(command, scratch, name, timeout=180, *, environment=None):
    with (scratch / (name + '.log')).open('xb') as log:
        subprocess.run(command, cwd=scratch, env=command_environment() if environment is None else environment, stdin=subprocess.DEVNULL,
                       stdout=log, stderr=subprocess.STDOUT, check=True, timeout=timeout)


def spdx_inventory(document, scan_root):
    require(scan_root.is_dir() and not scan_root.is_symlink())
    resolved_root = scan_root.resolve()
    directories, entries = {''}, 0
    for parent, children, files in os.walk(scan_root, followlinks=False):
        entries += len(children) + len(files)
        require(entries <= 2 * MAX_FILES)
        for child in children:
            path = Path(parent) / child
            require(not path.is_symlink() and stat.S_ISDIR(path.lstat().st_mode))
            require(path.resolve().is_relative_to(resolved_root))
            directories.add(relative_name(path.relative_to(scan_root).as_posix()))
    require(document.get('spdxVersion') == 'SPDX-2.3' and isinstance(document.get('files'), list))
    require(len(document['files']) <= MAX_FILES + len(directories))
    result, seen = {}, set()
    for entry in document['files']:
        require(isinstance(entry, dict))
        name = entry['fileName']
        require(isinstance(name, str))
        original_name = name
        if name.startswith('./'):
            name = name[2:]
        elif name.startswith('/'):
            name = name[1:]
        if name:
            relative_name(name)
        else:
            require(original_name == '')
        require(name not in seen)
        seen.add(name)
        if name in directories:
            # Syft 1.54.1 emits only this sentinel shape for real directories.
            # A regular/unrecognized file without SHA256 is never skipped.
            require(entry.get('fileTypes') == ['OTHER'] and entry.get('checksums') == [
                {'algorithm': 'SHA1', 'checksumValue': '0' * 40}])
            continue
        require(isinstance(entry['checksums'], list) and all(isinstance(item, dict) for item in entry['checksums']))
        checksums = [item['checksumValue'] for item in entry['checksums'] if item['algorithm'] == 'SHA256']
        require(len(result) < MAX_FILES and len(checksums) == 1 and hex_value(checksums[0], 64))
        result[name] = checksums[0]
    return result


def scan(syft, target, output, config, scratch, name, version, declared=False):
    command = [str(syft), 'scan', 'dir:' + str(target), '--config', str(config),
               '--base-path', str(target), '--source-name', name, '--source-version', version,
               '--output', 'spdx-json=' + str(output), '--parallelism', '2']
    if declared:
        command += ['--override-default-catalogers', 'rust-cargo-lock-cataloger,dart-pubspec-lock-cataloger']
    # Isolate only the scanner's profile. Git/H11 keep their existing environment;
    # no ambient HOME, cache, authentication or proxy settings reach Syft.
    profile = scratch / (name + '.profile')
    profile.mkdir(mode=0o700)
    environment = command_environment()
    for variable, directory in [('HOME', 'home'), ('XDG_CACHE_HOME', 'cache'), ('TMPDIR', 'tmp')]:
        path = profile / directory
        path.mkdir(mode=0o700)
        environment[variable] = str(path)
    run_logged(command, scratch, name, environment=environment)
    return json_object(read_bytes(output, MAX_SBOM))


class BoundedArchive:
    def __init__(self, handle):
        self.handle, self.hash, self.size = handle, hashlib.sha256(), 0

    def write(self, data):
        require(self.size + len(data) <= MAX_PAYLOAD)
        written = self.handle.write(data)
        require(written == len(data))
        self.hash.update(data)
        self.size += written
        return written

    def flush(self):
        self.handle.flush()


class CheckedInput:
    def __init__(self, handle):
        self.handle, self.hash, self.size = handle, hashlib.sha256(), 0

    def read(self, size):
        data = self.handle.read(size)
        self.hash.update(data)
        self.size += len(data)
        return data


def archive(bundle, files, sidecars, output):
    require(sum(len(value) for value in sidecars.values()) <= MAX_SBOM * 2)
    partial = output / '.payload.partial'
    with partial.open('xb') as raw:
        bounded = BoundedArchive(raw)
        with gzip.GzipFile(filename='', mode='wb', fileobj=bounded, mtime=0) as compressed:
            with tarfile.open(mode='w|', fileobj=compressed, format=tarfile.PAX_FORMAT) as package:
                file_bytes = 0
                for relative, expected in sorted(files.items()):
                    with regular(bundle / relative) as handle:
                        before = os.fstat(handle.fileno())
                        file_bytes += before.st_size
                        require(file_bytes <= MAX_PAYLOAD)
                        info = tarfile.TarInfo('app/' + relative)
                        info.size, info.mode = before.st_size, before.st_mode & 0o777
                        checked = CheckedInput(handle)
                        package.addfile(info, checked)
                        after = os.fstat(handle.fileno())
                    require(checked.hash.hexdigest() == expected and checked.size == before.st_size)
                    require((before.st_size, before.st_mtime_ns) == (after.st_size, after.st_mtime_ns))
                for name, data in sorted(sidecars.items()):
                    relative_name(name)
                    info = tarfile.TarInfo('metadata/' + name)
                    info.size, info.mode = len(data), 0o644
                    package.addfile(info, io.BytesIO(data))
        raw.flush()
        os.fsync(raw.fileno())
    require(bounded.size > 0)
    partial.rename(output / 'payload')
    return bounded.hash.hexdigest(), bounded.size


def produce(args):
    candidate = re.fullmatch(r'1\.0\.0-alpha\.([1-9][0-9]{0,9})', args.version)
    require(candidate is not None and int(candidate[1]) <= 4294967295)
    require(1024 <= args.port <= 65535)
    require(not args.output.is_symlink() and not args.output.exists())
    bundle, worker_source, output = args.bundle.resolve(), args.worker_source.resolve(), args.output.resolve()
    require(not args.bundle.is_symlink() and not args.worker_source.is_symlink())
    require(not output.exists() and not output.is_relative_to(bundle) and not output.is_relative_to(worker_source))
    commit = source_identity(ROOT)
    worker_commit = source_identity(worker_source, allow_cargo_marker=True)
    provenance_bytes, formats_bytes = read_bytes(args.provenance), read_bytes(args.formats, 65536)
    provenance, formats = json_object(provenance_bytes), json_object(formats_bytes)
    require(set(formats) == {'schema', 'source_commit', 'component_version', 'platform', 'architecture', 'data'})
    require(type(formats['schema']) is int and formats['schema'] == 1)
    require(formats['source_commit'] == commit and formats['platform'] == 'linux' and formats['architecture'] == 'x86_64')
    native = ROOT / 'packages/nds_native/rust'
    require(formats['component_version'] == tomllib.loads(read_bytes(native / 'Cargo.toml').decode())['package']['version'])
    data = formats['data']
    require(isinstance(data, dict) and set(data) == {'vault_journal', 'sync_wire', 'accounts_fence'})
    require(all(type(value) is int and 1 <= value <= 4294967295 for value in data.values()))
    require(provenance['build_kind'] == 'release' and provenance['platform'] == {'name': 'linux', 'architecture': 'x86_64'})
    for key in ('app_source', 'native_source'):
        require(provenance[key] == {'repository': DESKTOP.removeprefix('https://github.com/'),
                                    'commit': commit, 'state': 'committed'})
    require('product_version' not in provenance or provenance['product_version'] == args.version)
    files = inventory(bundle)
    require(provenance['artifact_files_sha256'] == files)
    lock_bytes = {'shared/Cargo.lock': read_bytes(native / 'Cargo.lock'),
                  'shared/pubspec.lock': read_bytes(ROOT / 'pubspec.lock'),
                  'worker/Cargo.lock': read_bytes(worker_source / 'Cargo.lock')}
    require(provenance['locks_sha256'] == {
        'native_cargo': digest(lock_bytes['shared/Cargo.lock']),
        'app_pubspec': digest(lock_bytes['shared/pubspec.lock'])})
    shared = tomllib.loads(lock_bytes['shared/Cargo.lock'].decode())
    worker = tomllib.loads(lock_bytes['worker/Cargo.lock'].decode())
    github = [package for package in shared['package'] if package['name'] == 'nddev-device-sync-github']
    require(len(github) == 1 and git_source(github[0]['source']) == (GITHUB_MODULE, worker_commit))
    worker_root = [package for package in worker['package'] if package['name'] == 'nddev-device-sync-github' and 'source' not in package]
    require(len(worker_root) == 1 and worker_root[0]['version'] == github[0]['version'])
    standards_bytes = read_bytes(ROOT / 'standarts.lock')
    design_bytes = read_bytes(ROOT / 'generated/launcher-provenance.json')
    sources = source_graph(commit, shared, worker, tomllib.loads(standards_bytes.decode()),
                           json_object(design_bytes), worker_commit)
    output.mkdir(mode=0o700)
    scratch = output / '.work'
    scratch.mkdir(mode=0o700)
    regenerated = scratch / 'verified-provenance.json'
    command = [sys.executable, str(ROOT / 'scripts/build-provenance.py'), str(bundle),
               '--app-root', str(ROOT), '--native-root', str(ROOT), '--build-kind', 'release',
               '--output', str(regenerated)]
    if 'product_version' in provenance:
        command += ['--product-version', args.version]
    run_logged(command, scratch, 'provenance')
    require(json_object(read_bytes(regenerated)) == provenance)
    declared_input = scratch / 'declared-locks'
    for name, value in lock_bytes.items():
        target = declared_input / name
        target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        write_new(target, value)
    config = scratch / 'syft.yaml'
    write_new(config, SYFT_CONFIG.encode())
    prepared_syft = shutil.which('syft')
    require(prepared_syft is not None)
    syft = Path(prepared_syft).resolve()
    syft_digest, _ = file_digest(syft, 512 * 1024 * 1024)
    native_sbom, declared_sbom = scratch / 'native.spdx.json', scratch / 'declared-dependencies.spdx.json'
    native_document = scan(syft, bundle, native_sbom, config, scratch, 'nds-linux-bundle', args.version)
    require(spdx_inventory(native_document, bundle) == files)
    declared_document = scan(syft, declared_input, declared_sbom, config, scratch,
                             'nds-declared-application-locks', args.version, declared=True)
    require(spdx_inventory(declared_document, declared_input) == {name: digest(value) for name, value in lock_bytes.items()})
    require(isinstance(declared_document.get('packages'), list) and declared_document['packages'])
    facts = {'schema': 1, 'source_commit': commit, 'worker_source_commit': worker_commit,
             'scope': 'Declared application lockfile inputs, possibly including dev and other targets; not linked Rust or build-tool dependency completeness',
             'locks_sha256': {name: digest(value) for name, value in lock_bytes.items()},
             'standards_sha256': digest(standards_bytes), 'design_provenance_sha256': digest(design_bytes),
             'syft_executable_sha256': syft_digest, 'syft_config_sha256': digest(SYFT_CONFIG.encode()),
             'declared_package_count': len(declared_document['packages'])}
    sidecars = {'provenance.json': provenance_bytes, 'formats.json': formats_bytes,
                'native.spdx.json': read_bytes(native_sbom, MAX_SBOM),
                'declared-dependencies.spdx.json': read_bytes(declared_sbom, MAX_SBOM),
                'dependency-lock-facts.json': json_bytes(facts),
                **{'locks/' + name: value for name, value in lock_bytes.items()}}
    require(source_identity(ROOT) == commit and
            source_identity(worker_source, allow_cargo_marker=True) == worker_commit)
    require(read_bytes(native / 'Cargo.lock') == lock_bytes['shared/Cargo.lock'] and
            read_bytes(ROOT / 'pubspec.lock') == lock_bytes['shared/pubspec.lock'] and
            read_bytes(worker_source / 'Cargo.lock') == lock_bytes['worker/Cargo.lock'])
    require(inventory(bundle) == files)
    archive_digest, archive_size = archive(bundle, files, sidecars, output)
    require(inventory(bundle) == files)
    identity = {'schema': 1, 'source_commit': commit, 'sha256': archive_digest, 'bytes': archive_size}
    manifest = {'schema': 1, 'application': 'nds', 'version': args.version,
                'platform': 'linux', 'architecture': 'x86_64', 'channel': 'alpha',
                'sources': sources, 'data': data,
                'artifact': {'url': f'http://127.0.0.1:{args.port}/payload', 'bytes': archive_size,
                             'sha256': archive_digest, 'app_source': commit, 'native_source': commit}}
    manifest_bytes = json_bytes(manifest)
    require(len(manifest_bytes) <= 65536)
    write_new(output / 'artifact-identity.json', json_bytes(identity))
    write_new(output / 'app-release.json', manifest_bytes)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('bundle', 'provenance', 'formats', 'worker-source', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--port', type=int, required=True)
    args = parser.parse_args()
    try:
        produce(args)
    except (OSError, ValueError, KeyError, TypeError, RecursionError,
            subprocess.SubprocessError, tarfile.TarError) as error:
        frames = traceback.extract_tb(error.__traceback__)
        frame = frames[-2] if len(frames) > 1 and frames[-1].name == 'require' else frames[-1]
        print('Native qualification producer refused invalid or incomplete inputs '
              f'({type(error).__name__}: {frame.name}:{frame.lineno})', file=sys.stderr)
        return 1
    print('Native qualification payload produced; no signing, distribution or activation')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
