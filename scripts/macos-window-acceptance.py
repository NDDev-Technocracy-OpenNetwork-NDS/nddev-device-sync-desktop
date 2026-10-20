"""Real AppKit close/reopen/quit on an owned CI app, without runtime test hooks."""
import argparse
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import signal
import subprocess
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('artifact', type=Path)
parser.add_argument('--source', required=True)
parser.add_argument('--evidence', type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
if (platform.system() != 'Darwin' or platform.machine() != 'arm64'
        or os.environ.get('GITHUB_ACTIONS') != 'true'):
    parser.error('Use the explicitly owned macOS ARM GitHub runner, not a personal profile')
if not re.fullmatch('[a-f0-9]{40}', args.source):
    parser.error('An immutable source SHA is required')
evidence = args.evidence.resolve()
if not args.evidence.is_absolute() or evidence.is_relative_to(root):
    parser.error('Evidence belongs outside the repository')
evidence.mkdir(mode=0o700, parents=True, exist_ok=False)
bundles = list(args.artifact.resolve().glob('*.app'))
assert len(bundles) == 1, 'Expected one built app'
bundle = bundles[0]
info = plistlib.loads((bundle / 'Contents/Info.plist').read_bytes())
binary = bundle / 'Contents/MacOS' / info['CFBundleExecutable']


def run(argv, **kw):
    return subprocess.run(argv, check=True, capture_output=True, text=True,
                          timeout=kw.pop('timeout', 25), **kw).stdout


def matching_pids():
    result = subprocess.run(['pgrep', '-f', '^' + re.escape(str(binary)) + '$'],
                            capture_output=True, text=True, timeout=5)
    assert result.returncode in (0, 1)
    return {int(pid) for pid in result.stdout.split()}


assert not matching_pids(), 'Never attach to an existing app'
log = evidence / 'sdk.log'


def events():
    out = []
    for line in log.read_text(errors='replace').splitlines():
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if isinstance(value, dict) and 'event.name' in value:
            out.append(value)
    return out


def wait(predicate, seconds=20):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        assert app.poll() is None, 'Owned app exited before completing the window scenario'
        value = predicate()
        if value:
            return value
        time.sleep(.2)
    raise AssertionError('Native window condition exceeded its deadline')


snapshot_expression = '''({
  NSWindow *w = (NSWindow *)[(id)[NSApp delegate] valueForKey:@"mainFlutterWindow"];
  id c = [w contentViewController];
  id e = [c valueForKey:@"engine"];
  [[NSString alloc] initWithData:[NSJSONSerialization dataWithJSONObject:@{
    @"window": @((uintptr_t)(__bridge void *)w),
    @"controller": @((uintptr_t)(__bridge void *)c),
    @"engine": @((uintptr_t)(__bridge void *)e),
    @"visible": @([w isVisible]), @"mini": @([w isMiniaturized])
  } options:0 error:NULL] encoding:NSUTF8StringEncoding];
})'''.replace('\n', ' ')
appkit_controller = '''
import lldb
def command(text):
    result = lldb.SBCommandReturnObject()
    lldb.debugger.GetCommandInterpreter().HandleCommand(text, result)
    print(result.GetOutput() or '', end='', flush=True)
    if not result.Succeeded():
        print(result.GetError() or '', end='', flush=True)
        raise RuntimeError('Owned debugger command failed')

thread = lldb.debugger.GetSelectedTarget().GetProcess().GetSelectedThread()
names = [thread.GetFrameAtIndex(i).GetFunctionName() or '' for i in range(12)]
idle = names[0].startswith('mach_msg') and any('__CFRunLoopServiceMachPort' in name for name in names)
outcome = 'NDS_MAIN_BUSY'
try:
    if idle:
        command('expression --language objective-c++ -- @import AppKit;')
        command('expression --language objective-c++ --object-description -- ' + expression)
        outcome = 'NDS_APPKIT_DONE'
    else:
        command('thread backtrace --count 12')
finally:
    command('process detach')
print(outcome, flush=True)
'''
number = 0


def appkit(expression):
    global number
    assert app.poll() is None
    number += 1
    transcript = evidence / f'appkit-{number}.log'
    deadline = time.monotonic() + 90
    control = 'script exec(' + json.dumps('expression = ' + repr(expression) + '\n' + appkit_controller) + ')'
    with transcript.open('w') as output:
        # Each fresh debugger resolves symbols and imports the AppKit module.
        # Keep its setup budget separate from the actual window/Quit deadlines,
        # and retain partial diagnostics even when the debugger times out.
        for _ in range(3):
            offset = transcript.stat().st_size
            result = subprocess.run([
                'xcrun', 'lldb', '--batch', '--no-lldbinit',
                '-o', f'process attach --pid {app.pid}',
                '-o', 'thread select 1',
                '-o', control,
            ], stdout=output, stderr=subprocess.STDOUT, text=True,
                timeout=max(.1, deadline - time.monotonic()))
            observed = transcript.read_bytes()[offset:].decode().splitlines()
            if result.returncode != 0 or 'NDS_MAIN_BUSY' not in observed:
                break
            # A busy observation detached without running the action/query.
            # Never retry an executed expression or a failed lifecycle action.
            time.sleep(.2)
    assert result.returncode == 0 and 'NDS_APPKIT_DONE' in observed, 'AppKit debugger command failed; inspect owned evidence'
    return transcript.read_text()


def snapshot():
    output = appkit(snapshot_expression)
    values = [json.loads(line) for line in output.splitlines() if line.startswith('{')]
    assert len(values) == 1
    return values[0]


def same_window():
    observed = snapshot()
    assert all(observed[key] == original[key] for key in ('window', 'controller', 'engine')), 'Window/engine identity changed'
    assert matching_pids() == {app.pid}, 'Reopen created another app process'
    return observed


env = {key: value for key, value in os.environ.items()
       if key not in ('GH_TOKEN', 'GITHUB_TOKEN', 'SSH_AUTH_SOCK')}
with log.open('w') as output:
    app = subprocess.Popen([str(binary)], stdout=output, stderr=output, env=env)
    try:
        wait(lambda: any(e['event.name'] == 'client.ready' for e in events()), 60)
        original = snapshot()
        assert original['visible'] and all(original[key] for key in ('window', 'controller', 'engine'))
        # Run lifecycle actions on the normal main run loop after detaching;
        # Flutter's other threads must be free to handle the window callbacks.
        for _ in range(2):
            appkit('(void)[(NSWindow *)[(id)[NSApp delegate] valueForKey:@"mainFlutterWindow"] performSelector:@selector(performClose:) withObject:nil afterDelay:0];')
            time.sleep(1)
            assert not same_window()['visible']
            assert not any(e['event.name'] == 'process.stopped' for e in events()), 'Closing the window shut down the Rust SDK'
            run(['open', '-a', str(bundle)])
            wait(lambda: same_window()['visible'])
        appkit('(void)[(NSWindow *)[(id)[NSApp delegate] valueForKey:@"mainFlutterWindow"] performSelector:@selector(performMiniaturize:) withObject:nil afterDelay:0];')
        wait(lambda: same_window()['mini'])
        run(['open', '-a', str(bundle)])
        wait(lambda: (state := same_window())['visible'] and not state['mini'])
        appkit('(void)[NSApp performSelector:@selector(terminate:) withObject:nil afterDelay:0];')
        assert app.wait(timeout=20) == 0, 'Explicit Quit did not exit normally'
        rows = events()
        assert sum(e['event.name'] == 'process.started' for e in rows) == 1
        assert sum(e['event.name'] == 'process.stopped' for e in rows) == 1
        assert all(e['source.commit'] == args.source for e in rows)
        assert len({e['producer.instance_id'] for e in rows}) == 1
        assert [e['producer.sequence'] for e in rows] == list(range(1, len(rows) + 1))
        result = {'source': args.source, 'close_reopen_cycles': 2,
                  'same_process_window_controller_engine': True,
                  'minimized_window_restored': True, 'explicit_quit_exit': 0,
                  'sdk_start_stop': [1, 1], 'sdk_events': len(rows),
                  'sdk_sequence_contiguous': True,
                  'scope': 'Actual macOS ARM window/process/SDK lifecycle; no authentication, Keychain or server-sync qualification'}
        (evidence / 'qualification.json').write_text(json.dumps(result, indent=2) + '\n')
        print('Actual macOS ARM close/reopen/minimize/Quit passed; one window/engine/SDK;', len(rows), 'SDK events')
    finally:
        for pid in matching_pids():
            os.kill(pid, signal.SIGCONT)
            os.kill(pid, signal.SIGTERM)
        if app.poll() is None:
            try:
                app.wait(timeout=5)
            except subprocess.TimeoutExpired:
                app.kill()
                app.wait(timeout=5)
