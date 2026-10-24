#!/usr/bin/env python3
"""Run the real shared UI against an explicitly owned local identity fixture."""
import argparse
import json
import os
import re
from pathlib import Path
import signal
import shutil
import subprocess
import time
import uuid
from urllib.parse import urlparse
from integration_runner import integration_runner
from contextlib import ExitStack

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--fixture", type=Path, required=True)
parser.add_argument("--app", type=Path, default=Path(__file__).resolve().parents[1])
parser.add_argument("--device", default="linux")
parser.add_argument("--cold-restart", action="store_true",
                    help="Verify encrypted persistence across two Android app processes")
parser.add_argument("--isolated-android", action="store_true",
                    help="Use a unique test applicationId on an explicitly selected Android device")
parser.add_argument("--company", action="store_true", help="Exercise company invitation and revocation through the real UI")
parser.add_argument("--vault", action="store_true", help="Exercise real native vault creation and encrypted sync")
parser.add_argument("--catalog-source", action="store_true", help="Also qualify saved GitHub choice through the real UI (requires --vault and --fixture-sync3)")
parser.add_argument("--modules", action="store_true", help="Qualify Android module consent across online/offline app processes")
parser.add_argument("--offline-pub", action="store_true", help="Resolve temporary test dependencies from a prewarmed cache; retain lockfile verification")
parser.add_argument("--sync-role", choices=["owner", "recipient"])
parser.add_argument("--resume", action="store_true", help="Verify a retained desktop session after another device changed the server")
args = parser.parse_args()
if args.catalog_source and (not args.vault or args.cold_restart or args.resume or args.modules or args.company or args.sync_role):
    parser.error("Catalog source acceptance requires a standalone --vault setup without other scenarios")
app = args.app.resolve()
app_id = None
gradle_path = app / "android/app/build.gradle.kts"
original_gradle = None
temporary_gradle = None
android = args.device.startswith("emulator-") or args.isolated_android
if args.modules and (not android or not args.isolated_android or not args.cold_restart
                     or args.company or args.vault or args.sync_role):
    parser.error("Module acceptance requires isolated Android cold restart without other scenario flags")
if args.isolated_android:
    if args.device in ["linux", "macos", "windows"]:
        raise SystemExit("Select the explicit ADB device id for isolated Android acceptance")
    original_gradle = gradle_path.read_bytes()
    match = re.search(rb'applicationId = "(ai[.]nddev[.]nds[.][a-zA-Z0-9_.]+)"', original_gradle)
    if not match or len(re.findall(rb'applicationId = ', original_gradle)) != 1:
        raise SystemExit("Expected one NDS applicationId; refuse ambiguous replacement")
    app_id = "ai.nddev.nds.acceptance.t" + uuid.uuid4().hex
    temporary_gradle = original_gradle.replace(match[0], f'applicationId = "{app_id}"'.encode())
if args.cold_restart:
    if not android:
        raise SystemExit("Cold-process acceptance requires an owned emulator or isolated Android namespace")
    gradle = (app / "android/app/build.gradle.kts").read_text()
    match = re.search(r'applicationId = "(ai[.]nddev[.]nds[.][a-zA-Z0-9_.]+)"', gradle)
    if not match:
        raise SystemExit("Expected an NDS-owned Android application id")
    if app_id is None:
        app_id = match.group(1)
    installed = subprocess.run(["adb", "-s", args.device, "shell", "pm", "path", app_id],
                               capture_output=True, timeout=10)
    if installed.stdout.strip():
        raise SystemExit("Cold acceptance requires an emulator without an existing NDS app")
fixture = json.loads(args.fixture.read_text())
for key in ["base_url", "mailpit_url"]:
    parsed = urlparse(fixture[key])
    if parsed.scheme != "http" or parsed.hostname != "127.0.0.1":
        raise SystemExit("Only the isolated loopback fixture is accepted")
if fixture.get("status") != "running":
    raise SystemExit("Fixture is not running")
source = Path(__file__).resolve().parents[1] / "packages/nds_ui/integration_test" / (
    "device_modules_test.dart" if args.modules else
    "server_replica_flow_test.dart" if args.sync_role else "identity_flow_test.dart")
helper = app / "integration_test/fixture_identity.dart"
module_helper = app / "integration_test/local_modules_test.dart" if not args.sync_role else None
# The general identity flow imports these controls even when the optional
# catalog branch is disabled. Its Dart import must resolve before tree shaking.
background_helper = app / "integration_test/device_modules_test.dart" if not args.modules and not args.sync_role else None
if background_helper is not None and background_helper.exists():
    raise SystemExit("Refuse to overwrite existing background fixture helpers")
if module_helper is not None and module_helper.exists():
    raise SystemExit("Refuse to overwrite existing module fixture helpers")
if helper.exists(): raise SystemExit("Refuse to overwrite existing fixture helper")
target = app / "integration_test/identity_acceptance_generated_test.dart"
# flutter test forwards Dart test events, not the complete native stdout stream.
# Keep the existing mobile and multi-replica runners; capture this ordinary Linux
# identity flow through the same driver already used by module acceptance.
driver = app / "integration_test/identity_acceptance_driver_generated.dart" if args.device == "linux" and not args.sync_role else None
if target.exists() or (driver is not None and driver.exists()):
    raise SystemExit("Refuse to overwrite existing generated test target or driver")
target.parent.mkdir(exist_ok=True)
ports = [str(urlparse(fixture[key]).port) for key in ["base_url", "mailpit_url"]]
log = args.fixture.with_name(args.fixture.stem + "-flutter-" + args.device + ".log")
if args.sync_role:
    log = log.with_stem(log.stem + "-sync-" + args.sync_role)
runner = ExitStack()

def fixture_mode(mode):
    control = args.fixture.resolve().with_suffix(".control.json")
    if Path(fixture["control_file"]).resolve() != control:
        raise RuntimeError("Expected the receipt's exact owned control file")
    temporary = control.with_name(control.name + "." + uuid.uuid4().hex + ".tmp")
    temporary.write_text(json.dumps({"mode": mode}))
    temporary.replace(control)
    deadline = time.monotonic() + 15
    wanted = "offline" if mode == "offline" else "running"
    while json.loads(args.fixture.read_text())["status"] != wanted:
        if time.monotonic() >= deadline:
            raise RuntimeError("Owned fixture did not reach requested mode")
        time.sleep(.1)

try:
    if temporary_gradle is not None:
        gradle_path.write_bytes(temporary_gradle)
    runner.enter_context(integration_runner(app, development_http=True, offline_pub=args.offline_pub))
    shutil.copyfile(source.with_name("fixture_identity.dart"), helper)
    shutil.copyfile(source, target)
    if module_helper is not None:
        shutil.copyfile(source.with_name("local_modules_test.dart"), module_helper)
    if background_helper is not None:
        shutil.copyfile(source.with_name("device_modules_test.dart"), background_helper)
    if driver is not None:
        shutil.copyfile(source.with_name("driver.dart"), driver)
    if android:
        for port in ports:
            subprocess.run(["adb", "-s", args.device, "reverse", "tcp:" + port, "tcp:" + port], check=True, timeout=10)
    command = (["flutter", "drive", "--target=" + str(target), "--driver=" + str(driver)]
        if driver is not None else ["flutter", "test", str(target),
        "--timeout=5m" if args.sync_role else "--timeout=3m"])
    command += ["-d", args.device, "--no-pub",
        "--dart-define=NDS_TEST_SERVER=" + fixture["base_url"],
        "--dart-define=NDS_TEST_MAILPIT=" + fixture["mailpit_url"],
        "--dart-define=NDS_TEST_OWNER=" + fixture["owner_email"],
        "--dart-define=NDS_DEVICE_NAME=" + ("Sync " + args.sync_role if args.sync_role else "Acceptance device")]
    if fixture.get("screen_peer_email") is not None:
        handoff = Path(fixture["screen_handoff"])
        if (args.device != "linux" or not fixture["screen_peer_email"].endswith("@example.invalid")
                or not handoff.is_absolute() or not handoff.is_dir()
                or not handoff.name.startswith("nds-native-screen-")
                or os.environ.get("NDS_OWNED_XVFB") != "1"):
            raise SystemExit("Screen UI acceptance requires an owned Linux display and native-pair handoff")
        command += ["--dart-define=NDS_TEST_SCREEN_PEER=" + fixture["screen_peer_email"],
                    "--dart-define=NDS_TEST_SCREEN_HANDOFF=" + str(handoff)]
    if args.company:
        command.append("--dart-define=NDS_TEST_COMPANY=true")
    if args.vault:
        command.append("--dart-define=NDS_TEST_VAULT=true")
    if args.catalog_source:
        command.append("--dart-define=NDS_TEST_CATALOG_SOURCE=true")
    if args.sync_role:
        command.append("--dart-define=NDS_TEST_SYNC_ROLE=" + args.sync_role)
    environment = os.environ.copy()
    environment["CARGO_BUILD_JOBS"] = "4"
    status = 0
    phases = (["persist", "offline", "restore"] if args.modules else
              ["persist", "restore"] if args.cold_restart else
              ["restore"] if args.resume else ["complete"])
    for phase in phases:
        if args.modules and phase in ["offline", "restore"]:
            fixture_mode("offline" if phase == "offline" else "online")
        phase_log = log.with_stem(log.stem + "-" + phase) if args.cold_restart else log
        with phase_log.open("w") as output:
            phase_command = command + ["--dart-define=NDS_TEST_PHASE=" + phase]
            if phase != phases[-1]:
                phase_command.append("--no-uninstall")
            process = subprocess.Popen(phase_command,
                cwd=app, env=environment, stdout=output,
                stderr=subprocess.STDOUT, start_new_session=True)
            try:
                status = process.wait(timeout=900)
            except (subprocess.TimeoutExpired, KeyboardInterrupt):
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                raise
        passed = "All tests passed." if driver is not None else "All tests passed!"
        if status == 0 and passed not in phase_log.read_text():
            status = 1
            print("Flutter exited without a completed passing test receipt")
        if args.modules and status == 0 and f"NDS_ANDROID_MODULE_PHASE_PASS phase={phase}" not in phase_log.read_text():
            status = 1
            print("Flutter did not confirm the selected module scenario")
        print(f"Actual identity UI acceptance phase={phase} exit={status}; private log: {phase_log}")
        if status:
            break
        if app_id is not None:
            subprocess.run(["adb", "-s", args.device, "shell", "am", "force-stop", app_id],
                           check=True, timeout=10)
            alive = subprocess.run(["adb", "-s", args.device, "shell", "pidof", app_id],
                                   capture_output=True, timeout=10)
            if alive.returncode != 1 or alive.stdout.strip():
                raise SystemExit("Owned Android application did not stop between phases")
    raise SystemExit(status)
finally:
    target.unlink(missing_ok=True)
    helper.unlink(missing_ok=True)
    if module_helper is not None:
        module_helper.unlink(missing_ok=True)
    if background_helper is not None:
        background_helper.unlink(missing_ok=True)
    if driver is not None:
        driver.unlink(missing_ok=True)
    try:
        runner.close()
    finally:
        if temporary_gradle is not None:
            if gradle_path.read_bytes() != temporary_gradle:
                raise RuntimeError("Android configuration changed outside acceptance; preserve it for review")
            gradle_path.write_bytes(original_gradle)
        if app_id is not None:
            subprocess.run(["adb", "-s", args.device, "uninstall", app_id],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15)
        if android:
            for port in ports:
                subprocess.run(["adb", "-s", args.device, "reverse", "--remove", "tcp:" + port],
                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
        if args.modules and json.loads(args.fixture.read_text())["status"] == "offline":
            fixture_mode("online")
