"""List the exact Git revisions represented by the locked native sources."""
import re
from urllib.parse import parse_qs, urlsplit


def valid_repository(repository):
    return len(repository) <= 222 and re.fullmatch(
        r'https://github\.com/[A-Za-z0-9_.-]{1,100}/[A-Za-z0-9_.-]{1,100}', repository
    )


def git_source(source):
    if not isinstance(source, str) or not source.startswith('git+'):
        raise ValueError('Git source required')
    url = urlsplit(source[4:])
    repository = 'https://github.com' + url.path.removesuffix('.git')
    if (url.scheme != 'https' or url.netloc != 'github.com'
            or not re.fullmatch('[a-f0-9]{40}', url.fragment)
            or parse_qs(url.query, strict_parsing=True) != {'rev': [url.fragment]}
            or url.query != 'rev=' + url.fragment or not valid_repository(repository)):
        raise ValueError('Expected an immutable public GitHub source')
    return repository, url.fragment


def release_sources(named, *locks):
    sources = {}
    revisions = set()

    def add(role, repository, commit):
        if (not re.fullmatch('[a-z0-9_.-]{1,64}', role) or role in sources
                or not valid_repository(repository) or not re.fullmatch('[a-f0-9]{40}', commit)):
            raise ValueError('Invalid or ambiguous native release source')
        sources[role] = {'role': role, 'repository': repository, 'commit': commit}
        revisions.add((repository.lower(), commit))

    for source in named:
        add(*source)

    cargo_sources = []
    for prefix, lock in locks:
        if not isinstance(lock.get('package'), list):
            raise ValueError('Invalid Cargo lock')
        for package in lock['package']:
            source = package.get('source', '')
            if not isinstance(source, str):
                raise ValueError('Invalid Cargo package source')
            if source.startswith('git+'):
                cargo_sources.append((prefix + package['name'], *git_source(source)))

    # Multiple crates and both lockfiles can refer to one source revision.
    for role, repository, commit in sorted(cargo_sources):
        if (repository.lower(), commit) not in revisions:
            add(role, repository, commit)

    if len(sources) > 64:
        raise ValueError('Native release source graph exceeds the manifest contract')
    return [sources[role] for role in sorted(sources)]
