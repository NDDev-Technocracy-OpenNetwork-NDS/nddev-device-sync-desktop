#!/usr/bin/env python3
"""Actual Accounts-authorized screen viewing in disposable Linux native stores.

The supplied loopback PG/SMTP fixture is exclusively owned by this run. Two
private X11 displays and Secret Services keep capture and credentials separate
from the user's desktop. Administrative changes use signed production routes.
"""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from urllib.parse import urlsplit
import uuid

ROOT = Path(__file__).resolve().parents[1]
TEST = "api::screen_acceptance::actual_company_screen_revalidation_closes_native_stream"
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def wait(test, seconds=30):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = test()
        if value:
            return value
        time.sleep(.1)
    raise TimeoutError("Owned screen acceptance deadline exceeded")


def stop(process):
    if process is not None and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def get(url):
    with HTTP.open(url, timeout=5) as response:
        return json.load(response)


def write(path, value):
    pending = path.with_suffix(".pending")
    pending.write_text(json.dumps(value) + "\n")
    pending.chmod(0o600)
    pending.replace(path)


def worker(config):
    c = json.loads(config.read_text())
    root = Path(c["root"])
    for name, relative in [("XDG_DATA_HOME", "data"), ("XDG_STATE_HOME", "state"),
                           ("XDG_CONFIG_HOME", "config"), ("XDG_CACHE_HOME", "cache"),
                           ("XDG_RUNTIME_DIR", "runtime")]:
        assert Path(os.environ[name]).resolve() == root / relative
    daemon = child = None
    try:
        with (root / "keyring.log").open("xb") as log:
            daemon = subprocess.Popen(
                ["gnome-keyring-daemon", "--foreground", "--components=secrets", "--unlock",
                 "--control-directory", str(root / "control")],
                stdin=subprocess.PIPE, stdout=log, stderr=log)
            daemon.stdin.write(secrets.token_urlsafe(32).encode())
            daemon.stdin.close()

            def owned_service():
                assert daemon.poll() is None
                p = subprocess.run(
                    ["gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
                     "--object-path", "/org/freedesktop/DBus", "--method",
                     "org.freedesktop.DBus.GetConnectionUnixProcessID", "org.freedesktop.secrets"],
                    capture_output=True, timeout=3)
                if p.returncode:
                    return False
                assert int(re.findall(rb"\d+", p.stdout)[-1]) == daemon.pid
                return True

            wait(owned_service, 10)
            child = subprocess.Popen(c["command"], env=os.environ | c["environment"])
            result = child.wait(timeout=2400)
            if result:
                raise RuntimeError("Owned screen member failed; inspect its private log")
    finally:
        stop(child)
        stop(daemon)


class Authority:
    def __init__(self, fixture, root):
        self.origin = fixture["base_url"]
        self.mailbox = fixture["mailpit_url"]
        self.root = root
        self.challenge_at = {}

    def request(self, path, body=None, token=None, headers=None, method=None):
        raw = body if isinstance(body, bytes) else json.dumps(body, separators=(",", ":")).encode() if body is not None else None
        h = {"Content-Type": "application/json"} | (headers or {})
        if token is not None:
            h["Authorization"] = "Bearer " + token
        req = urllib.request.Request(self.origin + path, data=raw, headers=h, method=method)
        try:
            response = HTTP.open(req, timeout=8)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read(65537)
            assert len(raw) <= 65536
            return response.status, json.loads(raw) if raw else None

    def message(self, email, prior, pattern):
        def find():
            for row in get(self.mailbox + "/api/v1/messages")["messages"]:
                if row["ID"] in prior:
                    continue
                message = get(self.mailbox + "/api/v1/message/" + row["ID"])
                if any(to["Address"] == email for to in message["To"]):
                    match = re.search(pattern, message["Text"])
                    if match:
                        return match
            return None
        return wait(find, 15)

    def authenticate(self, email, invitation=None):
        prior = {row["ID"] for row in get(self.mailbox + "/api/v1/messages")["messages"]}
        path = "/v2/auth/invitations/challenges" if invitation else "/v2/auth/email/challenges"
        status, challenge = self.request(path, {"email": email} | (invitation or {}))
        assert status == 202
        self.challenge_at[email] = time.monotonic() + challenge["resend_after_seconds"] + 1
        otp = self.message(email, prior, r"(?:code|код входа): (\d{8})")[1]
        status, session = self.request("/v2/auth/email/verify", {"challenge_id": challenge["challenge_id"], "code": otp})
        assert status == 200
        return session["session_token"], session["session"]["user_id"]

    def sign(self, key, data):
        path = self.root / "signing-input"
        path.write_bytes(data)
        try:
            return subprocess.check_output(["openssl", "pkeyutl", "-sign", "-inkey", str(key),
                                            "-rawin", "-in", str(path)], timeout=5, stderr=subprocess.DEVNULL)
        finally:
            path.unlink()

    def enroll(self, session):
        token, user = session
        key = self.root / (uuid.uuid4().hex + ".pem")
        subprocess.run(["openssl", "genpkey", "-algorithm", "ED25519", "-out", str(key)],
                       check=True, timeout=5, stderr=subprocess.DEVNULL)
        key.chmod(0o600)
        public = subprocess.check_output(["openssl", "pkey", "-in", str(key), "-pubout", "-outform", "DER"], timeout=5)
        assert len(public) == 44 and public[:12] == bytes.fromhex("302a300506032b6570032100")
        enc = lambda b: base64.urlsafe_b64encode(b).decode().rstrip("=")
        status, challenge = self.request("/v2/devices/challenges", {"platform": "linux",
            "display_name": "Owned screen controller", "public_key": enc(public[12:])}, token)
        assert status == 201
        proof = self.sign(key, b"NDS-ENROLLMENT-V2\0" + base64.urlsafe_b64decode(challenge["challenge"] + "="))
        status, device = self.request("/v2/devices/enrollments", {"challenge_id": challenge["challenge_id"], "signature": enc(proof)}, token)
        assert status == 201
        return {"token": token, "user": user, "key": key, "device": device["device_id"]}

    def signed(self, actor, path, body=None):
        raw = b"" if body is None else json.dumps(body, separators=(",", ":")).encode()
        method = "GET" if body is None else "POST"
        now = int(time.time())
        nonce = base64.urlsafe_b64encode(secrets.token_bytes(24)).decode().rstrip("=")
        digest = "sha-256=:" + base64.b64encode(hashlib.sha256(raw).digest()).decode() + ":"
        params = f'("@method" "@target-uri" "content-digest");created={now};expires={now+60};nonce="{nonce}";alg="ed25519";keyid="{actor["device"]}"'
        proof = self.sign(actor["key"], f'"@method": {method}\n"@target-uri": {self.origin+path}\n"content-digest": {digest}\n"@signature-params": {params}'.encode())
        return self.request(path, raw if body is not None else None, actor["token"],
                            {"Signature-Input": "nds=" + params, "Signature": "nds=:" + base64.b64encode(proof).decode() + ":", "Content-Digest": digest}, method)

    def invite(self, actor, role):
        email = uuid.uuid4().hex + "@example.invalid"
        prior = {row["ID"] for row in get(self.mailbox + "/api/v1/messages")["messages"]}
        status, invitation = self.signed(actor, "/v2/company/invitations", {"operation_id": operation(), "email": email, "role": role})
        assert status == 201
        match = self.message(email, prior, r"#invite/([A-Za-z0-9_-]{43})/([A-Za-z0-9_-]{43})")
        assert match[1] == invitation["invitation_id"]
        session = self.authenticate(email, {"invitation_id": match[1], "invitation_token": match[2]})
        return email, session


def operation():
    return str(uuid.UUID(int=(int(time.time()*1000) << 80) | (7 << 76) |
                         (secrets.randbits(12) << 64) | (2 << 62) | secrets.randbits(62)))


def qualify(args, fixture):
    processes, displays, handles = [], [], []
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    root = Path(tempfile.mkdtemp(prefix="nds-native-screen-", dir=Path.home() / ".local/state"))
    try:
        authority = Authority(fixture, root)
        owner = authority.enroll(authority.authenticate(fixture["owner_email"]))
        host_email, _ = authority.invite(owner, "user")
        _, admin_session = authority.invite(owner, "admin")
        controller = authority.enroll(admin_session)
        # Clients obtain their own real OTP. Respect the public resend window.
        wait(lambda: time.monotonic() >= max(authority.challenge_at.values()), 300)

        prior_messages = {row["ID"] for row in get(authority.mailbox + "/api/v1/messages")["messages"]}
        delivered_otps = set()

        def check():
            for child in processes:
                if child.poll() is not None and child.returncode != 0:
                    raise RuntimeError("Owned screen process failed; inspect private logs")
            assert all(p.stat().st_size <= 128 * 1024**2 for p in args.output.glob("*.log"))
            for role in (["host", "viewer"] if args.viewer == "native" else ["host"]):
                if role not in delivered_otps and (root / f"{role}-otp-requested.json").exists():
                    email = host_email if role == "host" else fixture["owner_email"]
                    code = authority.message(email, prior_messages, r"(?:code|код входа): (\d{8})")[1]
                    write(root / f"{role}-otp.json", {"code": code})
                    delivered_otps.add(role)

        def fact(name, seconds=120):
            def read():
                check()
                p = root / name
                if p.exists():
                    assert p.stat().st_size <= 4096
                    return json.loads(p.read_text())
                return None
            return wait(read, seconds)

        def start(role, flutter=False):
            scope = root / role
            for relative in ["data/keyrings", "state", "config", "cache", "runtime", "control"]:
                (scope / relative).mkdir(mode=0o700, parents=True, exist_ok=True)
            import select
            for _ in range(32):
                number = str(secrets.randbelow(20000) + 100)
                if Path(f"/tmp/.X{number}-lock").exists() or Path(f"/tmp/.X11-unix/X{number}").exists():
                    continue
                auth = scope / "xauthority"
                auth.touch(mode=0o600, exist_ok=True)
                subprocess.run(["xauth", "-f", str(auth), "add", ":" + number, ".", secrets.token_hex(16)],
                               check=True, capture_output=True, timeout=5)
                display = subprocess.Popen(["Xvfb", ":" + number, "-displayfd", "1", "-screen", "0",
                    "1440x1000x24" if flutter else "640x480x24", "-nolisten", "tcp", "-noreset", "-auth", str(auth)],
                    stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
                if select.select([display.stdout], [], [], 10)[0]:
                    actual = display.stdout.readline().decode().strip()
                    if actual == number and display.poll() is None:
                        displays.append(display)
                        break
                stop(display)
            else:
                raise RuntimeError("No owned X11 display could be allocated")
            env = {k: v for k, v in os.environ.items() if not k.startswith("NDS_") and k not in
                   ["GH_TOKEN", "GITHUB_TOKEN", "SSH_AUTH_SOCK", "DBUS_SESSION_BUS_ADDRESS", "WAYLAND_DISPLAY"]}
            env.update({k: str(scope / v) for k, v in [("XDG_DATA_HOME", "data"), ("XDG_STATE_HOME", "state"),
                ("XDG_CONFIG_HOME", "config"), ("XDG_CACHE_HOME", "cache"), ("XDG_RUNTIME_DIR", "runtime")]})
            env.update(DISPLAY=":" + number, XAUTHORITY=str(auth), GDK_BACKEND="x11", LIBGL_ALWAYS_SOFTWARE="1")
            # Build inputs are explicitly verified by prepare_native, not state.
            for name in ["CARGO_HOME", "CARGO_TARGET_DIR", "NDS_NATIVE_BUILD_HOME", "NDS_NATIVE_BUILD_PREPARED", "NDS_NATIVE_SOURCE_HOME"]:
                if name in os.environ:
                    env[name] = os.environ[name]
            env.update({k: v for k, v in os.environ.items() if k.startswith("NDS_CACHE_")})
            if flutter:
                config = scope / "fixture.json"
                write(config, fixture | {"screen_peer_email": host_email, "screen_handoff": str(root)})
                command = [sys.executable, str(ROOT / "scripts/identity-acceptance.py"), "--fixture", str(config), "--offline-pub"]
                extra = {"NDS_OWNED_XVFB": "1", "NDS_OWNED_XVFB_PID": str(display.pid)}
            else:
                config = scope / "native.json"
                write(config, {"endpoint": fixture["base_url"], "email": host_email if role == "host" else fixture["owner_email"],
                               "role": role, "directory": str(root)})
                command = [str(args.native_test), "--exact", TEST, "--ignored", "--nocapture"]
                extra = {"NDS_SCREEN_ACCEPTANCE": str(config), "NDS_OWNED_XVFB": "1", "NDS_OWNED_XVFB_PID": str(display.pid)}
            config = scope / "worker.json"
            write(config, {"root": str(scope), "command": command, "environment": extra})
            log = (args.output / f"{role}.log").open("xb")
            handles.append(log)
            child = subprocess.Popen(["dbus-run-session", "--", sys.executable, __file__, "--worker", str(config)],
                                     env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            processes.append(child)

        if args.viewer == "flutter":
            start("viewer", flutter=True)
            fact("viewer-ui-ready.json", 1800)
            start("host")
        else:
            start("host")
            start("viewer")
        fact("host-streaming.json", 180)
        pixels = fact("viewer-frame.json")
        status, page = authority.signed(controller, "/v2/company/members")
        assert status == 200
        member = next(row for row in page["members"] if row["user_id"] == owner["user"])
        status, _ = authority.signed(controller, "/v2/company/members/" + owner["user"],
            {"operation_id": operation(), "expected_revision": member["revision"], "action": "set_role", "role": "user"})
        assert status == 200
        write(root / "demoted.json", {"demoted": True})
        host_proof, viewer_proof = fact("host-proof.json", 15), fact("viewer-proof.json", 15)
        for child in processes:
            assert child.wait(timeout=30) == 0
        write(args.output / "receipt.json", {"source": args.source, "viewer": args.viewer,
            "actual_smtp_otp": True, "actual_signed_role_change": True, "actual_frame": pixels,
            "host": host_proof, "viewer_proof": viewer_proof,
            "scope": "Owned Linux X11 view-only stream and Accounts company-role revocation; other platforms and source-loss/update acceptance remain separate."})
    finally:
        for child in processes:
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                child.wait(timeout=20)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=5)
        for log in root.glob("*/*.log"):
            assert log.stat().st_size <= 128 * 1024**2
            shutil.copyfile(log, args.output / (log.parent.name + "-" + log.name))
        for display in displays:
            stop(display)
        for handle in handles:
            handle.close()
        shutil.rmtree(root)
        write(args.output / "cleanup.json", {"owned_children_joined": all(p.poll() is not None for p in processes + displays),
              "owned_native_stores_removed": not root.exists()})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--native-test", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source", required=True)
    parser.add_argument("--viewer", choices=["native", "flutter"], default="flutter")
    args = parser.parse_args()
    assert sys.platform == "linux" and re.fullmatch("[a-f0-9]{40}", args.source)
    for p in [args.fixture, args.native_test, args.output]:
        assert p.is_absolute() and not p.resolve().is_relative_to(ROOT)
    assert args.fixture.is_file() and args.native_test.is_file() and not args.output.exists()
    assert subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip() == args.source
    assert not subprocess.check_output(["git", "diff", "--name-only", "HEAD"], cwd=ROOT)
    fixture = json.loads(args.fixture.read_text())
    assert fixture["status"] == "running" and fixture["owner_email"].endswith("@example.invalid")
    assert fixture["expires_at_unix"] - time.time() > 2700
    for name in ["base_url", "mailpit_url"]:
        u = urlsplit(fixture[name])
        assert u.scheme == "http" and u.hostname == "127.0.0.1" and u.port
        assert not u.username and not u.password and u.path in ["", "/"] and not u.query and not u.fragment
    marker = Path(fixture["stop_marker"])
    assert marker.parent.resolve() == args.fixture.parent.resolve()
    try:
        qualify(args, fixture)
    finally:
        marker.touch()
        wait(lambda: json.loads(args.fixture.read_text())["status"] == "stopped", 30)
        cleanup = json.loads((args.output / "cleanup.json").read_text())
        write(args.output / "cleanup.json", cleanup | {"fixture_stopped": True})


if __name__ == "__main__":
    def interrupted(_signal, _frame):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, interrupted)
    if len(sys.argv) == 3 and sys.argv[1] == "--worker":
        worker(Path(sys.argv[2]))
    else:
        main()
