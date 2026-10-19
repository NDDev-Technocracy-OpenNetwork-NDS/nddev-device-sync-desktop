"""Real GTK/Flutter hidden-window replay in a private X11/DBus/credential scope."""

import argparse, ctypes as C, hashlib, json, os, re, secrets, sqlite3, subprocess, sys, tempfile, time, urllib.request
from pathlib import Path
from urllib.parse import urlparse

P = argparse.ArgumentParser(description=__doc__)
P.add_argument("--fixture", type=Path, required=True)
P.add_argument("--bundle", type=Path, required=True)
P.add_argument("--helper", type=Path, required=True)
P.add_argument("--evidence", type=Path, required=True)
P.add_argument("--source", required=True)
P.add_argument("--worker", type=Path, help=argparse.SUPPRESS)
OPTIONS = P.parse_args()
ROOT = Path(__file__).resolve().parents[1]
for path in [OPTIONS.fixture, OPTIONS.bundle, OPTIONS.helper, OPTIONS.evidence]:
    if not path.is_absolute() or path.resolve().is_relative_to(ROOT):
        P.error(
            "Use explicit owned fixture/artifact/evidence paths outside this repository"
        )
if not re.fullmatch("[a-f0-9]{40}", OPTIONS.source):
    P.error("An immutable source SHA is required")
W = OPTIONS.evidence
FIXTURE = OPTIONS.fixture
BUNDLE = OPTIONS.bundle
HELPER = OPTIONS.helper

APP = "ai.nddev.nds.nddev_device_sync_desktop"
NAME = "^NDDev OpenNetwork$"
PREFIX = "api::replica_acceptance::"
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def run(args, **kw):
    return subprocess.run(
        args,
        check=True,
        capture_output=True,
        text=True,
        timeout=kw.pop("timeout", 15),
        **kw
    ).stdout.strip()


def wait(test, seconds=30):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        value = test()
        if value:
            return value
        time.sleep(0.1)
    raise AssertionError("Owned native acceptance condition exceeded its deadline")


def get(url):
    with HTTP.open(url, timeout=3) as r:
        return json.load(r)


def events(path):
    out = []
    if not path.exists():
        return out
    for line in path.read_text(errors="replace").splitlines():
        try:
            x = json.loads(line)
        except ValueError:
            continue
        if isinstance(x, dict) and "event.name" in x:
            out.append(x)
    return out


def windows(visible=False):
    args = (
        ["xdotool", "search"]
        + (["--onlyvisible"] if visible else [])
        + ["--name", NAME]
    )
    p = subprocess.run(args, capture_output=True, text=True, timeout=3)
    assert p.returncode in [0, 1]
    return set(p.stdout.split())


def close_window(window):
    # Actual ICCCM WM_DELETE_WINDOW, without requiring/installing a window manager.
    class Data(C.Union):
        _fields_ = [("b", C.c_char * 20), ("s", C.c_short * 10), ("l", C.c_long * 5)]

    class Client(C.Structure):
        _fields_ = [
            ("type", C.c_int),
            ("serial", C.c_ulong),
            ("send_event", C.c_int),
            ("display", C.c_void_p),
            ("window", C.c_ulong),
            ("message_type", C.c_ulong),
            ("format", C.c_int),
            ("data", Data),
        ]

    class Event(C.Union):
        _fields_ = [("client", Client), ("pad", C.c_long * 24)]

    x = C.CDLL("libX11.so.6")
    x.XOpenDisplay.argtypes = [C.c_char_p]
    x.XOpenDisplay.restype = C.c_void_p
    x.XInternAtom.argtypes = [C.c_void_p, C.c_char_p, C.c_int]
    x.XInternAtom.restype = C.c_ulong
    x.XSendEvent.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_long, C.POINTER(Event)]
    x.XSendEvent.restype = C.c_int
    x.XFlush.argtypes = [C.c_void_p]
    x.XCloseDisplay.argtypes = [C.c_void_p]
    display = x.XOpenDisplay(None)
    assert display
    try:
        event = Event()
        event.client.type = 33
        event.client.display = display
        event.client.window = int(window)
        event.client.message_type = x.XInternAtom(display, b"WM_PROTOCOLS", 0)
        event.client.format = 32
        event.client.data.l[0] = x.XInternAtom(display, b"WM_DELETE_WINDOW", 0)
        assert x.XSendEvent(display, int(window), 0, 0, C.byref(event)) != 0
        x.XFlush(display)
    finally:
        x.XCloseDisplay(display)


def control(mode):
    p = FIXTURE
    f = json.loads(p.read_text())
    target = Path(f["control_file"])
    assert target.parent == p.parent
    pending = target.with_suffix(".pending")
    pending.write_text(json.dumps({"mode": mode}))
    pending.replace(target)
    wanted = "running" if mode == "online" else "offline"
    wait(lambda: json.loads(p.read_text()).get("status") == wanted, 15)


def worker(root):
    root = Path(root)
    for key, relative in [
        ("XDG_DATA_HOME", "data"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_RUNTIME_DIR", "runtime"),
    ]:
        assert (
            Path(os.environ[key]).resolve() == root / relative
        ), "A private XDG profile is required"
    f = json.loads((FIXTURE).read_text())
    assert f["status"] == "running"
    helper = str(HELPER)
    assert HELPER.is_file()
    binary = BUNDLE / "nddev_device_sync_desktop"
    assert binary.is_file()
    config = root / "case.json"
    config.write_text(json.dumps({"directory": str(root), "receipt": str(FIXTURE)}))
    config.chmod(0o600)
    env = os.environ | {
        "NDS_NATIVE_WINDOW_ACCEPTANCE": str(config),
        "RLDYOUR_CLIPBOARD_HOME": str(root / "unused-clipboard"),
        "RLDYOUR_CLIPBOARD_CAPTURE": "0",
    }
    for k in [
        "GH_TOKEN",
        "GITHUB_TOKEN",
        "SSH_AUTH_SOCK",
        "NDS_TEST_ISOLATED_GITHUB_TOKEN_FILE",
    ]:
        env.pop(k, None)
    daemon = None
    gui = None
    with (W / "keyring.log").open("wb") as keylog:
        try:
            daemon = subprocess.Popen(
                [
                    "gnome-keyring-daemon",
                    "--foreground",
                    "--components=secrets",
                    "--unlock",
                    "--control-directory",
                    str(root / "control"),
                ],
                stdin=subprocess.PIPE,
                stdout=keylog,
                stderr=keylog,
                env=env,
            )
            daemon.stdin.write(secrets.token_urlsafe(32).encode())
            daemon.stdin.close()

            def secret_service():
                assert daemon.poll() is None
                p = subprocess.run(
                    [
                        "gdbus",
                        "call",
                        "--session",
                        "--dest",
                        "org.freedesktop.DBus",
                        "--object-path",
                        "/org/freedesktop/DBus",
                        "--method",
                        "org.freedesktop.DBus.GetConnectionUnixProcessID",
                        "org.freedesktop.secrets",
                    ],
                    capture_output=True,
                    timeout=3,
                )
                if p.returncode:
                    return False
                assert int(re.findall(rb"\d+", p.stdout)[-1]) == daemon.pid
                return True

            wait(secret_service, 10)
            before = {
                x["ID"] for x in get(f["mailpit_url"] + "/api/v1/messages")["messages"]
            }
            with (W / "seed.log").open("wb") as log:
                seed = subprocess.Popen(
                    [
                        helper,
                        "--ignored",
                        "--exact",
                        PREFIX + "seed_native_window_profile",
                        "--nocapture",
                    ],
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    env=env,
                )
                try:

                    def otp():
                        assert (
                            seed.poll() is None
                        ), "Seed helper stopped before OTP; inspect owned log"
                        if not (root / "otp-requested").exists():
                            return False
                        for row in get(f["mailpit_url"] + "/api/v1/messages")[
                            "messages"
                        ]:
                            if row["ID"] in before:
                                continue
                            mail = get(
                                f["mailpit_url"] + "/api/v1/message/" + row["ID"]
                            )
                            if not any(
                                v["Address"] == f["owner_email"] for v in mail["To"]
                            ):
                                continue
                            match = re.search(
                                r"(?:code|код входа): (\d{8})", mail["Text"]
                            )
                            if match:
                                p = root / "otp.tmp"
                                p.write_text(json.dumps({"code": match[1]}))
                                p.chmod(0o600)
                                p.replace(root / "otp.json")
                                return True
                        return False

                    wait(otp, 25)
                    assert seed.wait(timeout=90) == 0
                finally:
                    if seed.poll() is None:
                        seed.kill()
                        seed.wait(timeout=5)
            assert (root / "seeded").is_file()
            dbs = list((root / "state/nddev-device-sync/sync").glob("*.db"))
            assert len(dbs) == 1

            def pending():
                with sqlite3.connect(
                    "file:" + str(dbs[0]) + "?mode=ro", uri=True, timeout=2
                ) as db:
                    return db.execute(
                        "SELECT count(*) FROM v3_outbox WHERE state='pending'"
                    ).fetchone()[0]

            gui_log = W / "gui-runtime.log"
            with gui_log.open("wb") as log:
                gui = subprocess.Popen(
                    [str(binary)],
                    cwd=binary.parent,
                    env=env,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                )

                def running_batch():
                    assert gui.poll() is None, "GUI stopped; inspect owned log"
                    return any(
                        e["event.name"] == "sync.replica.batch"
                        and e.get("outcome") == "ok"
                        for e in events(gui_log)
                    )

                wait(running_batch, 60)
                wait(lambda: pending() == 0, 30)
                ids = wait(lambda: windows(True), 10)
                assert len(ids) == 1
                window = next(iter(ids))
                pid = gui.pid
                control("offline")
                with (W / "stage.log").open("wb") as stage:
                    subprocess.run(
                        [
                            helper,
                            "--ignored",
                            "--exact",
                            PREFIX + "stage_native_window_offline_work",
                            "--nocapture",
                        ],
                        stdout=stage,
                        stderr=subprocess.STDOUT,
                        env=env,
                        check=True,
                        timeout=60,
                    )
                assert pending() == 1
                close_window(window)
                wait(lambda: not windows(True), 10)
                assert gui.poll() is None and windows() == {window}
                hidden_at = time.time()
                start = len(events(gui_log))
                control("online")
                wait(lambda: pending() == 0, 90)
                assert gui.poll() is None and not windows(True)
                wait(
                    lambda: any(
                        x["event.name"] == "sync.replica.batch"
                        and x.get("outcome") == "ok"
                        for x in events(gui_log)[start:]
                    ),
                    30,
                )
                with (W / "secondary.log").open("wb") as second:
                    result = subprocess.run(
                        [str(binary)],
                        cwd=binary.parent,
                        env=env,
                        stdout=second,
                        stderr=subprocess.STDOUT,
                        timeout=15,
                    )
                assert result.returncode == 0
                wait(lambda: windows(True) == {window}, 10)
                assert gui.pid == pid and gui.poll() is None
                assert not any(
                    x["event.name"] == "process.started"
                    for x in events(W / "secondary.log")
                )
                run(["xdotool", "windowfocus", "--sync", window])
                run(["xdotool", "key", "--clearmodifiers", "ctrl+q"])
                assert gui.wait(timeout=15) == 0
            assert "SYNTHETIC_WINDOW_SECRET" not in gui_log.read_text(errors="replace")
            with (W / "restarted-gui-runtime.log").open("wb") as log:
                gui = subprocess.Popen(
                    [str(binary)],
                    cwd=binary.parent,
                    env=env,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                )

                def restarted_ready():
                    assert gui.poll() is None
                    return any(
                        x["event.name"] == "sync.replica.batch"
                        and x.get("outcome") == "ok"
                        for x in events(W / "restarted-gui-runtime.log")
                    )

                wait(restarted_ready, 60)
                assert pending() == 0 and len(windows(True)) == 1
                run(["gapplication", "action", APP, "quit"])
                assert gui.wait(timeout=15) == 0
            assert "SYNTHETIC_WINDOW_SECRET" not in (
                W / "restarted-gui-runtime.log"
            ).read_text(errors="replace")
            proof = {
                "source": OPTIONS.source,
                "gui_binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                "pending_offline": 1,
                "pending_after_hidden_reconnect": 0,
                "same_primary_pid_on_reopen": True,
                "same_native_window_on_reopen": True,
                "secondary_engine_started": False,
                "ctrl_q_exit": 0,
                "native_quit_action_exit": 0,
                "session_restored_after_explicit_quit": True,
                "manual_sync_during_hidden_replay": False,
                "private_x11": True,
                "private_credentials": True,
                "helper_binary_sha256": hashlib.sha256(
                    Path(helper).read_bytes()
                ).hexdigest(),
                "scope": "Linux window/process integration only; retained Rust owner after UI-engine disposal and CLI duplicate-owner arbitration remain unqualified.",
            }
            for name in [
                "seed.log",
                "stage.log",
                "gui-runtime.log",
                "restarted-gui-runtime.log",
            ]:
                rows = events(W / name)
                assert rows and {x["source.commit"] for x in rows} == {OPTIONS.source}
                assert "SYNTHETIC_WINDOW_SECRET" not in (W / name).read_text(
                    errors="replace"
                )
            (W / "window-proof.json").write_text(json.dumps(proof, indent=2) + "\n")
            print(
                "PASS: actual hidden GUI alone replayed encrypted pending; reopen reused PID/window and Ctrl+Q quit normally."
            )
        finally:
            for child in [gui, daemon]:
                if child is not None and child.poll() is None:
                    child.terminate()
                    try:
                        child.wait(timeout=8)
                    except subprocess.TimeoutExpired:
                        child.kill()
                        child.wait(timeout=5)


def main():
    assert FIXTURE.is_file() and HELPER.is_file() and BUNDLE.is_dir()
    f = json.loads(FIXTURE.read_text())
    assert f["status"] == "running" and f["owner_email"].endswith("@example.invalid")
    for key in ["base_url", "mailpit_url"]:
        origin = urlparse(f[key])
        assert (
            origin.scheme == "http" and origin.hostname == "127.0.0.1" and origin.port
        )
        assert origin.username is None and origin.password is None
        assert origin.path in ["", "/"] and not origin.query and not origin.fragment
    assert time.time() < f["expires_at_unix"]
    W.mkdir(mode=0o700, parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="nds-window-") as directory:
        root = Path(directory)
        for p in ["data/keyrings", "state", "config", "cache", "runtime", "control"]:
            (root / p).mkdir(parents=True, mode=0o700)
        env = os.environ | {
            key: str(root / value)
            for key, value in [
                ("XDG_DATA_HOME", "data"),
                ("XDG_STATE_HOME", "state"),
                ("XDG_CONFIG_HOME", "config"),
                ("XDG_CACHE_HOME", "cache"),
                ("XDG_RUNTIME_DIR", "runtime"),
            ]
        }
        subprocess.run(
            [
                "xvfb-run",
                "-a",
                "-s",
                "-screen 0 1440x1000x24 -nolisten tcp",
                "dbus-run-session",
                "--",
                sys.executable,
                __file__,
                *sys.argv[1:],
                "--worker",
                directory,
            ],
            env=env,
            check=True,
            timeout=330,
        )


if __name__ == "__main__":
    if OPTIONS.worker is not None:
        worker(OPTIONS.worker)
    else:
        main()
