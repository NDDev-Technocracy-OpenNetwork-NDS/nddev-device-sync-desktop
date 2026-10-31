# nddev-device-sync-desktop

Desktop client and shared Flutter presentation for
[NDDev OpenNetwork](https://nddev.ai), AGPL-3.0-only. `packages/nds_ui` owns the
en/ru Flutter interface used by desktop and mobile; `packages/nds_native` generates
a thin bridge to shared Rust modules. NDS tokens and unwrapped keys never enter Dart.
The React browser assembly reuses this Rust composition and the canonical en/ru
message catalog through the optional `browser-bindings` feature. Desktop and
mobile retain the default Flutter bridge; the Flutter web baseline is retired.

Module management requires an authenticated, enrolled device. Desired enabled
state and NDS permissions use the existing encrypted per-device Vault journal
with displayed revision/pending-ID CAS. The original operation and frozen local
revision fence authorization across acknowledgement, delayed results and
withdrawal/reapproval. Provider observations cannot overwrite consent.
Device-state publication uses the same Rust observation selection as module
readiness on every platform, including shared adapters. Unchecked or disabled
adapters cannot publish fresh successful health. Local-agent publication also
checks the originating client and cancellation lifetime before reading its state.

Core owns admission and readiness. The composition reports actual supported
adapters, discovered local dependencies and last query results separately;
consent alone is not readiness and never grants OS/browser permissions. A
configuration change cancels affected owned work and stale results. Existing
replica/RDS lifetimes perform revalidation; no second scheduler or plugin
installer is introduced. Source delivery and platform acceptance remain separate.

Module presentation uses FRB's compile-checked mirrors of the three required
core types. The bridge exports their fields and enums; domain helper methods
remain Rust-only.

The same observability SDK used by the server and agent owns event validation,
redaction, trace context, debug windows and local diagnostics. Flutter framework
and root-isolate async failures pass only a fixed error class through the generated
bridge; raw exception text/stacks are omitted from both logs and error widgets.
Diagnostics work before sign-in. Network and local-I/O debug scopes expire
after five minutes or 100 debug events. Normal security/error logging continues.
Protected Vault snapshot reads correlate their credential checks under one Rust
operation span, keeping routine polling from multiplying standalone read traces.
The SDK emits process startup and controlled shutdown; a cancelable desktop exit
waits for bounded shutdown, while mobile background/detach never terminates it.
Forced termination cannot promise a final event. Missing native initialization
shows a safe localized startup screen; a missing sink remains explicitly degraded.

`just check` includes real framework/root-isolate failure tests using the native
library. The observability repository's isolated pipeline can additionally consume
that private test log through `NDS_CLIENT_EVENTS_FILE` and verify searchable
failure records in OpenObserve. Authenticated clients use the shared
accounts sender for signed event and OTLP span delivery; the server applies its
operator policy and binds tenant/user/device identity. Clipboard and alerts
shows per-signal acknowledgements, losses, policy and last successful receipt.
Pause/resume, bounded retention and cancellation belong to the shared sender.
Browser batches persist through page restart in the SDK spool; the UI reports
actual payload-write readiness separately from policy and remote acknowledgements.
Startup capture belongs only to the first verified client subscription.

One Flutter lifecycle adapter feeds the core activity policy. Visible desktop
and browser clients keep bounded synchronization and delivery when input focus
moves elsewhere. Hidden browser/mobile clients pause and reconcile on resume;
the retained desktop process continues replication. Screens consume the same
core activity decision for interactive work.
Manual synchronization owns one stop/retry/restart sequence; overlapping calls
report busy. An authorized manual retry replaces its capacity policy, so late
results from earlier work cannot block it again or restart an obsolete replica.
Healthy inbox reads and lifecycle changes retain a real capacity refusal.
After a completed inbox read observes newer retention, Core may request one
immutable-original capacity check. Ordinary uploads stay blocked until its
actual handled outcome (or proven empty inbox) is revalidated under the current
Accounts lifetime and policy identity. Recovery replaces policy so late old
publishers cannot reblock it. Repeated refusal at the same retention floor does
not create a probe loop; uncertain transport retries preserve the original.

Use Flutter/Dart from `toolchains.lock.json`, the compiler from
`packages/nds_native/rust/rust-toolchain.toml` and flutter_rust_bridge_codegen 2.13.0.
Install the Rust target matching the platform being built. `flutter pub get
--enforce-lockfile`, `just check`, `just generated-check` and `just build-linux`
cover dependencies, static checks, generated-source drift and the Linux build.
Run the same native build on each other supported host before claiming support.

Native preparation is shared by the Flutter hook, build wrapper, generator and
`just check`. It locates the immutable RDS SDK through Cargo, verifies its clean
Git source, and calls that module's checked provider preparer. Generated inputs
and a dedicated Cargo home stay in build caches; user Cargo/Git configuration is
not changed. Public source fetching is non-interactive and needs no SSH agent.
For direct Cargo checks, wrap the command with
`python packages/nds_native/tool/prepare_native.py --` and supply
`--manifest-path packages/nds_native/rust/Cargo.toml --locked` to Cargo.
The product lock remains authoritative; source discovery does not build a second
runtime. Concurrent preparation shares one bounded OS lock.
The wrapper verifies source signatures before invoking Flutter. Its isolated
hook consumes the checked configuration without receiving GitHub credentials;
direct hook preparation requires the owning preparer's normal GitHub CLI access.
Standalone worker packaging retains its own locked graph without the shared
client's native provider overrides.

Linux credentials use native libsecret through the Rust Accounts adapter. Builds
require `libsecret-1-dev`; runtime requires libsecret 0.21 or newer and an encrypted
Secret Service session. Existing credential attributes and binary values are preserved.

The Linux alpha uses Flutter's supported Skia GPU backend. Requalify Impeller
after an engine update passes native interaction acceptance under low VRAM;
this selection does not enable software rendering.

`flutter run -d linux` opens the client. Enter the HTTPS origin of an NDS server
with compatible control-plane and encrypted-sync endpoints. Native credential
storage must be available and unlocked.
On Linux, closing the window keeps NDS running in the desktop session. Launch
NDS again to reopen the existing window; use Ctrl+Q to quit, or the standard
`gapplication action ai.nddev.nds.nddev_device_sync_desktop quit` command.
On macOS, closing the window also keeps NDS running. Reopen it from the Dock or
Finder to restore the same window and engine, including a minimized window.
Use Quit (Cmd+Q) for the existing bounded Rust shutdown.
Desktop presentation attaches to the retained Rust runtime; cancelling or
disposing its facade leaves synchronization running. Native processes share the
journal's replica admission: one owner reconciles, while CLI followers verify
the authenticated history head and committed local state before reporting freshness.
GitHub sign-in requires the server's actual provider configuration and explicit
browser approval; an unavailable provider stays unavailable.

`just identity-acceptance /private/path/fixture.json linux` runs the real UI
against an explicitly started server PostgreSQL/Mailpit fixture. The driver
briefly enables debug-only loopback HTTP, uses a synthetic identity, and restores
the runner configuration afterward. Production builds require HTTPS. Runtime
logs and fixture receipts stay outside the repository.

For encrypted persistence across Android processes, pass `--cold-restart` to the
acceptance driver with an explicitly selected native device. Add
`--isolated-android` for a unique test application ID that preserves installed
applications. The two launches preserve test-app storage between phases, then
revoke the session and uninstall only the test-owned app.
ADB reverse admission refuses existing device-port bindings; teardown removes
only the mappings successfully admitted by this run.
When ADB runs on another computer, make both fixture loopback ports reachable on
that ADB host, and tunnel the owned VM service port back to the driver computer.
Pass `--host-vmservice-port PORT`. The existing integration driver reserves that
ADB-host port without replacing a binding, keeps DDS local, and removes only its
own forward and test app. The source and build remain on the driver computer.

The desktop Local modules tab reads native system metrics, clipboard history,
cleaner diagnostics/previews, update plans and repository/remote-session state.
Missing tools report unavailable. `just module-acceptance /absolute/path/to/clipboardd /private/fixture.json`
runs actual Flutter interactions with installed providers, a fresh clipboard
archive with capture disabled, and a temporary Git repository. It does not read
or change the personal clipboard archive or execute cleanup/updates.

The repository page captures local catalog baselines before its protected
GitHub read and publishes observations through the encrypted Vault writer.
Core retains a bounded page until each candidate is unchanged or durably queued;
backpressure and caller timeouts preserve its remaining work. The provider
continuation advances only after all page facts and any terminal scope summary
have a local receipt. Session or selection changes cancel the pending page.
The saved-catalog view reads Vault without resolving a GitHub credential and
shows observation dates, pending/conflict state and missing owner profiles.
These are retained observations, not current access or an atomic provider snapshot.
Known conflicts use a separate bounded pass: Vault captures the complete affected
families before a real fresh provider read, and only matching returned facts may
queue resolutions. That pass never advances traversal. An active queued or
unknown operation waits for its actual outcome. A conflicted scope is captured
before restarting the first page; its decision is made only from the genuine
terminal traversal summary. Changed scope proof restarts the traversal while
preserving saved observations. Conflicts remain visible until Applied receipts.
The explicit GitHub choice is encrypted owner-wide intent, restored when the
repository screen opens. It does not grant device consent or provider access.
Reads and durable publications retain the original saved source, selected
credential and this installation's consent; changing one rejects old work.
Source-setting conflicts remain blocked. The elected native Rust Vault owner
runs bounded catalog traversal and periodic existing-commit cycles for saved
local connections; it retains source, device consent and writer ownership until
Git has drained. Recursive initialization, child publication and changed-graph
reconciliation remain incomplete. End-to-end provider/client acceptance remains separate. Owned UI
acceptance uses `identity-acceptance.py --vault --catalog-source` with a fresh
`--fixture-sync3` fixture.

An admitted foreground repository connection retains its exact GDS client,
work and terminal result when the caller disappears. One connection runs at a
time; identity replacement waits for both this work and native module shutdown.
Final delivery validates the original caller and configuration. A refused
delivery does not undo Git effects. Missing completion retains the owner and
blocks replacement; a joined provider failure keeps its owning error. This
in-process retention is not a durable Git-operation journal.

Screen requests and viewing use the shared Rust owner, Accounts authorization
and Core contracts. The current viewer backend is Linux X11; other targets
report unsupported. Native subsystem checks and Accounts-authenticated Flutter
viewing are separate qualification scopes. The owned Linux X11 workflow below
exercises actual frames and termination; other target roles and final product
and release acceptance remain separate.

`scripts/screen-acceptance.py` qualifies that Linux path against an explicitly
owned, fresh 3600-second Server `--fixture-sync3` fixture. Supply `--fixture`,
`--native-test` (the prepared `development-http`, no-default-features Rust lib
test executable), `--output` (a new private directory), and the checkout's exact
`--source` SHA. Prewarm the Flutter integration dependencies before starting;
the UI run resolves them offline and retains the application's lockfile.
The default `--viewer flutter` pairs the real UI with a native host;
`--viewer native` isolates the native control/media path. Both use real SMTP
OTP, separate Secret Services and owned X11 displays, observe changed source
pixels. The default `--case role` revokes the viewer's company role through
signed production routes. With either viewer, `--case source-loss`, `cancel`,
and `local-disable` terminate the owned capture
display, cancel the actual host handle, or disable its local module consent.
`logout` uses the actual host logout path; `expiry` waits for the real
five-minute view lifetime, without changing clocks or production limits. Each
Flutter case checks removal of the actual displayed image; expiry also rejects
an early disconnect against the native host's expiry proof. Each invocation
qualifies only its selected case. Receipts and cleanup remain outside
the repository; other targets and release qualification remain separate.

`python scripts/build.py linux --release` builds the committed Linux AOT bundle
and its existing GDS worker with a tracked native source SHA. An explicitly
approved assembly policy may be passed before the target with
`--app-update-policy /path/to/approved-policy.json`; its exact bounded JSON
string becomes the tracked NativeAssets input. An unset policy stays empty and
does not inherit an ambient policy. The wrapper restores the app configuration
after success, failure or cancellation. Ordinary `flutter build` retains
development provenance.

Linux cache assembly takes only an explicitly approved local port:
`python scripts/build.py linux --release --dart-define=NDS_CACHE_PORT=PORT`.
There is no default or port discovery. The main workflow's optional
`linux_cache_port` dispatch input supplies the same define to Linux only;
ordinary pushes and other targets leave it unset. Build provenance records the
port (or `null`) and the digest of Flutter's generated Linux configuration,
without exporting other Dart defines. This records assembly intent, not cache
readiness, profile binding or RDS identity. Android builds must leave it unset.

Linux application updates provide verified discovery, bounded staging,
cancellation and authenticated local restore in a separate owned namespace.
The existing system Updater tool retains its independent plan/report scope.
Installation, process switching and activation are unavailable; other native
targets report unsupported. An unconfigured build reports that updates are not
configured. Native keys and encrypted pending operations remain with their
existing owners.

CI pins SDK archives by SHA-256, builds Linux release/AOT and macOS (ARM64/Intel)
and Windows debug bundles. These builds are separate from native
credential/provider interaction acceptance. `scripts/check-artifact.py` rejects
integration entrypoints and native test plugins. Integration dependencies exist
only inside the acceptance driver's temporary scope. The Linux qualification
artifact binds actual bundle bytes, worker inputs, owning format metadata,
provenance and separate native-file and declared-dependency SPDX inventories.
The main-only workflow authenticates its complete release manifest after the
quality gate; consumption also requires the entire exact-main run to succeed.
Qualification uses an explicit development loopback origin. It does not publish
a release, configure a production update feed, sign other platforms or install
the application. A transport or CI acknowledgement is not runtime acceptance.

Vault records use bounded cursor pages. The UI supports create/edit,
encrypted tombstones and explicit conflict choices guarded by the displayed
revision and observed sequence. A save confirms a durable local outbox receipt;
the UI separately shows pending changes and the last fully caught-up timestamp.
Background reads keep editors enabled and retain keyboard focus. A user action
waits for an already running read and supersedes its presentation result.
Secret values are copied through platform clipboard operations and are never
returned by the bridge. Shared text is explicit, bounded to 8 KiB and encrypted.
Android refuses private/unsupported input and marks delivered values sensitive
so they do not re-enter shared capture.

Passkey management exposes only public fingerprints and creation times.
Removal requires a recent sign-in and consumes the server's atomic revocation
receipt; revoking the current passkey session returns to sign-in. Creating or
using a passkey opens the server's browser ceremony with comparison-code approval.

`--vault` in identity acceptance also exercises encrypted NDS text sharing. The
Android identity/Vault scenario preserves the device user's system clipboard;
explicit Android native copy/capture and private-value refusal remain in
`--modules`, which requires an owned disposable clipboard context. Desktop copy
acceptance uses the actual OS clipboard. Use an owned X11 session on Linux, for example
`xvfb-run -a env -u WAYLAND_DISPLAY GDK_BACKEND=x11 python3 scripts/identity-acceptance.py --fixture /private/fixture.json --vault`.
The test must observe the native operation completing before reading the OS
clipboard. Production UI confirmation uses the same native completion receipt.

`python3 scripts/window-acceptance.py --help` describes the Linux window check.
Supply an owned PostgreSQL/SMTP `--fixture-sync3` fixture, its built loopback
test bundle, a native test executable built with `development-http`, the exact
artifact source SHA, the matching `development-http` CLI executable and its
immutable source SHA, and a new evidence directory outside the repository. It
uses a private X11/DBus/credential profile to check close, automatic encrypted
replay, authenticated CLI reads while the GUI owns replication, single-instance
reopen, bounded Rust shutdown on explicit quit and session restoration.

The browser assembly uses the same UI and a stable, single-threaded Rust/WASM
bundle. `initializeNdsBridge` supplies the preloaded ES module to FRB without
dynamic JavaScript evaluation; its default native loader remains unchanged.
The Flutter build also exports the vault worker's four journal entrypoints.
Browser storage, HTTP, credentials and clipboard are owned by their adapters;
native tools are not exposed as browser capabilities.

With default features disabled, `browser-bindings` exposes identity, company
administration, Vault, clipboard, catalog and incident operations over the same
Rust owners without linking the Flutter bridge. Public metadata types come from
their owning Rust views; 64-bit values remain `bigint`, inputs reject coercion and company commands
retain their immutable Rust handles. Catalog continuations stay in Rust and
explicit page actions are not automatically replayed by presentation caches.
Clipboard availability uses the same platform capability check as Flutter.
Secrets are copied by the Rust clipboard
adapter, never returned as presentation values. Fixed React/browser failure
classes use the shared telemetry SDK without exception text or stacks.
Install `wasm32-unknown-unknown` for the pinned compiler and run `just browser-check`.
The React application lives in `nddev-device-sync-web`; its source-matched
workflow and offline-upgrade acceptance are separate from this binding check.

The UI distinguishes cached offline authorization, local pending operation IDs,
committed revisions and complete sync time. Foreground reconciliation observes
shared logout decisions and retries a pending remote logout. Storage persistence
reports the browser's actual grant; it does not prevent explicit site-data
removal. Browser clipboard requests reserve user activation before asynchronous
secret access. Qualify the full browser assembly separately from native gates.

The same Rust reconciliation owner maintains Accounts' bounded seven-day
authorization and protected clock observations. Resume closes online admission
until signed device verification; valid local data stays available offline.
Reconnection wakes the existing replica, preserving pending operation IDs and
bytes. No Flutter/React timer owns authorization policy or extends a cached grant.

Company management uses the standalone Organizations Rust client, the existing
Accounts session/signing owner and Core policies. Shared presentation supports
company metadata, bounded member/invitation pages, invite/revoke, role changes
and membership removal. An uncertain command retains its original opaque Rust
handle for retry; there is no offline administrative queue or automatic resend.

Signed-out native clients can import an invitation link into Rust and complete
the existing email verification. Browser entry captures and removes invitation
fragments before presentation routing; incoming links do not switch an active
account. HTTPS app/universal-link association is deployment-specific and is not
claimed by manual native import. The existing browser assembly remains the
Flutter migration baseline; its React replacement is a separate ADR 0010 task.

Release journals live in [GitHub Releases](https://github.com/NDDev-Technocracy-OpenNetwork-NDS/nddev-device-sync-desktop/releases).
