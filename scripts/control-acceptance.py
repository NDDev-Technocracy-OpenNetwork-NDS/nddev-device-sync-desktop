#!/usr/bin/env python3
"""Actual native device-control acceptance (ADR 0018) on Linux.

Runs the prebuilt `api::control_acceptance` test ELF against an explicitly owned
running loopback PG/SMTP fixture, an isolated private Secret Service and the
installed `rldyour-sysinfo` tool. The controller only delivers the real OTP from
the owned mailbox and ages the owned session's sign-in proof in the owned
fixture database; every admission decision is made by the product APIs.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
TEST = "api::control_acceptance::device_control_admits_narrows_and_widens_a_real_system_tool"
spec = importlib.util.spec_from_file_location("nds_screen_acceptance", ROOT / "scripts/screen-acceptance.py")
helpers = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helpers)


def events(path):
    values = []
    for line in path.read_text(errors="replace").splitlines():
        start = line.find("{")
        if start < 0:
            continue
        try:
            value = json.loads(line[start:])
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and "event.name" in value:
            values.append(value)
    return values


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--native-test", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    fixture = json.loads(args.fixture.read_text())
    assert fixture["status"] == "running" and fixture["base_url"].startswith("http://127.0.0.1:")
    assert fixture["database_container"].startswith("nds-db-check-")
    assert args.native_test.is_absolute() and args.native_test.is_file()
    socket = Path(os.environ["XDG_RUNTIME_DIR"]) / "rldyour-sysinfo.sock"
    assert socket.is_socket(), "the installed rldyour-sysinfo socket is required"
    args.output.mkdir(mode=0o700)
    root = args.output / "scope"
    for relative in ["data", "state", "config", "cache", "runtime", "control", "markers"]:
        (root / relative).mkdir(mode=0o700, parents=True)
    # The installed read-only metrics daemon keeps its own socket; only its path
    # is exposed inside the private runtime directory.
    (root / "runtime/rldyour-sysinfo.sock").symlink_to(socket)
    markers = root / "markers"
    email = fixture["owner_email"]
    authority = helpers.Authority(fixture, root)
    prior = {row["ID"] for row in helpers.get(authority.mailbox + "/api/v1/messages")["messages"]}
    config = root / "control.json"
    helpers.write(config, {"endpoint": fixture["base_url"], "email": email, "directory": str(markers)})
    worker = root / "worker.json"
    helpers.write(worker, {"root": str(root), "environment": {"NDS_CONTROL_ACCEPTANCE": str(config)},
                           "command": [str(args.native_test), "--exact", TEST, "--ignored", "--nocapture",
                                       "--test-threads=1"]})
    env = {k: os.environ[k] for k in ["HOME", "PATH", "LANG", "LC_ALL"] if k in os.environ}
    env |= {name: str(root / relative) for name, relative in [
        ("XDG_DATA_HOME", "data"), ("XDG_STATE_HOME", "state"), ("XDG_CONFIG_HOME", "config"),
        ("XDG_CACHE_HOME", "cache"), ("XDG_RUNTIME_DIR", "runtime")]}
    env |= {"PYTHONDONTWRITEBYTECODE": "1", "RUST_MIN_STACK": "8388608"}
    log = args.output / "test.log"
    record = {"status": "RUNNING", "fixture_source": fixture.get("source_commit"), "test": TEST,
              "scope": "Actual Linux device control through public client APIs, owned PG/SMTP, private "
                       "Secret Service and the installed sysinfo tool; no Flutter UI or installed NDS app claim."}
    started = time.monotonic()
    with log.open("wb") as output:
        child = subprocess.Popen(["dbus-run-session", "--", sys.executable, "-c",
                                  "import importlib.util,sys;from pathlib import Path;"
                                  "s=importlib.util.spec_from_file_location('w',sys.argv[1]);"
                                  "m=importlib.util.module_from_spec(s);s.loader.exec_module(m);m.worker(Path(sys.argv[2]))",
                                  str(ROOT / "scripts/screen-acceptance.py"), str(worker)],
                                 env=env, stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
        delivered = aged = False
        try:
            while child.poll() is None:
                assert time.monotonic() - started < 900, "bounded control acceptance timeout"
                if not delivered and (markers / "otp-requested").exists():
                    code = authority.message(email, prior, r"(?:code|код входа): (\d{8})")[1]
                    helpers.write(markers / "otp.json", {"code": code})
                    delivered = True
                if not aged and (markers / "age-requested").exists():
                    # Only the owned fixture's sessions exist; shift their sign-in
                    # proof outside the recent window without changing validity.
                    subprocess.run(["docker", "exec", "-i", fixture["database_container"], "psql", "-XAt",
                                    "-v", "ON_ERROR_STOP=1", "-U", "postgres", "-d", "nds", "-c",
                                    "UPDATE nds_sessions SET authenticated_at_ms=authenticated_at_ms-600000"],
                                   check=True, capture_output=True, timeout=10)
                    helpers.write(markers / "aged", {"aged_ms": 600000})
                    aged = True
                time.sleep(0.1)
        finally:
            helpers.stop(child)
    record["exit"] = child.returncode
    record["seconds"] = time.monotonic() - started
    proof_path = markers / "proof.json"
    record["proof"] = json.loads(proof_path.read_text()) if proof_path.exists() else None
    observed = events(log)
    admissions = [e for e in observed if str(e.get("event.name", "")).startswith("module.admission.")]
    actions = [e for e in observed if str(e.get("event.name", "")).startswith("module.action.")]
    device = (record["proof"] or {}).get("device")
    correlated = {e["operation.id"] for e in admissions if e.get("outcome") == "accepted"} & \
                 {e["operation.id"] for e in actions if e.get("outcome") == "ok"}
    record["diagnostics"] = {
        "admission_events": len(admissions), "action_events": len(actions),
        "fullauto_accepted": sum(e["event.name"] == "module.admission.fullauto" and e.get("outcome") == "accepted" for e in admissions),
        "manual_denied_disabled": sum(e["event.name"] == "module.admission.manual" and e.get("outcome") == "denied"
                                      and e.get("error.type") == "module_disabled" for e in admissions),
        "manual_accepted": sum(e["event.name"] == "module.admission.manual" and e.get("outcome") == "accepted" for e in admissions),
        "correlated_operations": len(correlated),
        "same_device": bool(device) and all(e.get("device.id") == device for e in admissions + actions),
        "principal_present": all(re.fullmatch(r"[A-Za-z0-9_.:-]+", str(e.get("user.id", ""))) for e in admissions + actions),
    }
    d = record["diagnostics"]
    ok = (record["exit"] == 0 and record["proof"] and d["fullauto_accepted"] >= 2 and d["manual_denied_disabled"] >= 3
          and d["manual_accepted"] >= 1 and d["correlated_operations"] >= 3 and d["same_device"] and d["principal_present"])
    record["status"] = "PASS" if ok else "FAIL"
    helpers.write(args.output / "result.json", record)
    print(json.dumps({k: v for k, v in record.items() if k != "proof"}, indent=2))
    raise SystemExit(0 if ok else 1)


if __name__ == "__main__":
    main()
