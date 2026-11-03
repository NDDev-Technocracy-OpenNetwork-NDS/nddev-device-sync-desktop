//! Process configuration and FFI only; event policy and diagnostics belong to SDK.
#[cfg(feature = "flutter")]
use flutter_rust_bridge::frb;
use nddev_device_sync_telemetry::{
    Config, DebugWindow, Logger,
    client_traces::ClientTraces,
    view::{ClientLifecycle, DebugScope, DiagnosticsError, FailureKind, LocalDiagnostics},
};
use std::{
    sync::{Mutex, MutexGuard, OnceLock},
    time::Duration,
};

struct ProcessTelemetry {
    logger: Logger,
    traces: ClientTraces,
}
static LOGGER: OnceLock<Option<Mutex<ProcessTelemetry>>> = OnceLock::new();
fn logger() -> Option<MutexGuard<'static, ProcessTelemetry>> {
    LOGGER
        .get()?
        .as_ref()
        .map(|logger| logger.lock().unwrap_or_else(|e| e.into_inner()))
}

#[cfg_attr(feature = "flutter", frb(init))]
pub fn init_app() -> Result<(), DiagnosticsError> {
    #[cfg(target_arch = "wasm32")]
    nddev_device_sync_accounts::invitation::capture_browser_invitation()
        .map_err(|_| DiagnosticsError::Unavailable)?;
    #[cfg(target_arch = "wasm32")]
    crate::browser::configure().map_err(|_| DiagnosticsError::Unavailable)?;
    LOGGER.get_or_init(|| {
        let config = Config {
            service: "nddev-device-sync-client".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            repository: "NDDev-Technocracy-OpenNetwork-NDS/nddev-device-sync-desktop".into(),
            commit: crate::provenance::source_commit().into(),
            channel: "alpha".into(),
            standards: crate::provenance::standards_release().into(),
            environment: if cfg!(debug_assertions)
                || crate::provenance::source_commit() == "unknown"
            {
                "development"
            } else {
                "client"
            }
            .into(),
            target_prefixes: vec![
                "nds_accounts".into(),
                "nddev_device_sync_organizations".into(),
                "nds_native".into(),
                "nddev_device_sync_vault".into(),
                "nddev_device_sync_agent".into(),
                "nddev_device_sync_adapter_io".into(),
                "nddev_device_sync_sysinfo".into(),
                "nddev_device_sync_clipboard".into(),
                "nddev_device_sync_cleaner".into(),
                "nddev_device_sync_updater".into(),
                "nddev_device_sync_github".into(),
                "nddev_device_sync_remote".into(),
            ],
            default_module: "process".into(),
            debug: None,
        };
        let traces = ClientTraces::new(&config).ok()?;
        let logger = nddev_device_sync_telemetry::install_client_with_tracer_to(
            config,
            traces.tracer(),
            nddev_device_sync_telemetry::LocalOutput::Stdout,
        )
        .ok()?;
        Some(Mutex::new(ProcessTelemetry { logger, traces }))
    });
    // An unavailable OS sink is explicitly degraded; identity may still work.
    Ok(())
}

#[cfg_attr(feature = "flutter", frb(sync))]
pub fn client_diagnostics() -> LocalDiagnostics {
    logger().map_or_else(LocalDiagnostics::unavailable, |logger| {
        logger.logger.control.diagnostics()
    })
}

#[cfg_attr(feature = "flutter", frb(sync))]
pub fn configure_debug(scope: DebugScope, seconds: u16) -> Result<(), DiagnosticsError> {
    if seconds > 300 || (seconds > 0 && !matches!(scope, DebugScope::Http | DebugScope::Io)) {
        return Err(DiagnosticsError::InvalidInput);
    }
    let logger = logger().ok_or(DiagnosticsError::Unavailable)?;
    // A window the user starts or stops here is theirs; the server-policy follower
    // never closes it.
    #[cfg(not(target_arch = "wasm32"))]
    FOLLOWER_OPENED.store(false, std::sync::atomic::Ordering::Release);
    logger
        .logger
        .control
        .set_debug((seconds > 0).then(|| DebugWindow {
            scope: scope.as_str().into(),
            duration: Duration::from_secs(u64::from(seconds)),
            events: 100,
        }))
        .map_err(|_| DiagnosticsError::Unavailable)
}

/// True means the SDK accepted the call, not that an external collector received it.
#[cfg_attr(feature = "flutter", frb(sync))]
pub fn record_client_failure(kind: FailureKind) -> bool {
    let Some(logger) = logger() else {
        return false;
    };
    if logger.logger.control.output_health().terminating {
        return false;
    }
    logger.logger.control.report_failure(kind);
    true
}

#[cfg_attr(feature = "flutter", frb(sync))]
pub fn record_client_lifecycle(state: ClientLifecycle) -> bool {
    let Some(logger) = logger() else {
        return false;
    };
    if logger.logger.control.output_health().terminating {
        return false;
    }
    logger.logger.control.report_client_lifecycle(state);
    true
}

/// Native flushing runs outside Flutter's frame thread; browser timers and
/// console output are owned by the browser runtime.
pub async fn shutdown_telemetry() -> Result<LocalDiagnostics, DiagnosticsError> {
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    if let Err(error) =
        nddev_device_sync_vault::replication::native::NativeReplica::shutdown().await
    {
        tracing::error!(event.name = "sync.runtime.stop_failed", module = "vault", outcome = "error", error.type = error.code());
        return Err(DiagnosticsError::Unavailable);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Ok(tokio::task::spawn_blocking(shutdown_local)
            .await
            .unwrap_or_else(|_| LocalDiagnostics::unavailable()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        Ok(shutdown_local())
    }
}
fn shutdown_local() -> LocalDiagnostics {
    logger().map_or_else(LocalDiagnostics::unavailable, |mut logger| {
        logger.traces.shutdown();
        logger.logger.shutdown();
        logger.logger.control.diagnostics()
    })
}

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn actual_sdk_events_keep_the_compiled_producer_identity() {
        use std::io::Read;

        const CHILD: &str = "NDS_TEST_TELEMETRY_PRODUCER_CHILD";
        const RUNTIME_POISON: &str = "runtime-standards-must-not-override-build";
        const OUTPUT_BYTES: u64 = 65_536;
        if std::env::var_os(CHILD).is_some() {
            super::init_app().unwrap();
            assert!(super::client_diagnostics().sink_initialized);
            assert!(super::record_client_lifecycle(
                super::ClientLifecycle::Ready
            ));
            let stopped = super::shutdown_local();
            assert_eq!(stopped.terminating, Some(true));
            assert_eq!(stopped.shutdown_incomplete, Some(false));
            std::process::exit(0);
        }

        struct Child {
            process: std::process::Child,
            output: Option<std::thread::JoinHandle<std::io::Result<Vec<u8>>>>,
        }
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.process.kill();
                let _ = self.process.wait();
                if let Some(output) = self.output.take() {
                    let _ = output.join();
                }
            }
        }
        let mut child = Child {
            process: std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "api::telemetry::tests::actual_sdk_events_keep_the_compiled_producer_identity",
                    // No pretty `test ...` prefix may join a real SDK JSON line.
                    "--quiet",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .env("NDS_STANDARDS_RELEASE", RUNTIME_POISON)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap(),
            output: None,
        };
        let stdout = child.process.stdout.take().unwrap();
        child.output = Some(std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stdout.take(OUTPUT_BYTES + 1).read_to_end(&mut bytes)?;
            Ok(bytes)
        }));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let status = loop {
            if let Some(status) = child.process.try_wait().unwrap() {
                break status;
            }
            assert!(std::time::Instant::now() < deadline, "SDK child deadline");
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        let output = child.output.take().unwrap().join().unwrap().unwrap();
        assert!(status.success(), "isolated SDK producer failed ({status})");
        assert!(output.len() as u64 <= OUTPUT_BYTES);
        let events: Vec<_> = output
            .split(|byte| *byte == b'\n')
            .filter_map(|line| serde_json::from_slice::<serde_json::Value>(line).ok())
            .map(|event| nddev_device_sync_telemetry::decode_event(event).unwrap())
            .collect();
        // A configured build catches the old literal `unreleased`; an unset
        // build keeps that honest fallback. Runtime settings cannot choose it.
        for name in ["client.ready", "process.stopped"] {
            let records: Vec<_> = events
                .iter()
                .filter(|event| event.event_name == name)
                .collect();
            assert_eq!(records.len(), 1, "missing or duplicate SDK lifecycle event");
            let event = records[0];
            assert_eq!(
                event.standards_release,
                crate::provenance::standards_release()
            );
            assert_ne!(event.standards_release, RUNTIME_POISON);
            assert_eq!(event.source_commit, crate::provenance::source_commit());
            assert_eq!(
                event.source_repository,
                "NDDev-Technocracy-OpenNetwork-NDS/nddev-device-sync-desktop"
            );
            assert_eq!(event.service_name, "nddev-device-sync-client");
        }
    }

    #[test]
    fn missing_console_preserves_identity_and_reports_diagnostic_unavailability() {
        const CHILD: &str = "NDS_TEST_CLOSED_STDOUT_CHILD";
        if std::env::var_os(CHILD).is_some() {
            use std::os::fd::{FromRawFd, OwnedFd};
            unsafe {
                drop(OwnedFd::from_raw_fd(1));
            }
            super::init_app().unwrap();
            let status = super::client_diagnostics();
            assert!(!status.sink_initialized && !status.debug_enabled);
            assert!(status.write_failures.is_none() && status.dropped_events.is_none());
            crate::api::identity::NdsClient::new("https://example.invalid".into()).unwrap();
            assert_eq!(
                super::configure_debug(super::DebugScope::Http, 30),
                Err(super::DiagnosticsError::Unavailable)
            );
            assert!(!super::record_client_failure(
                super::FailureKind::NativeBridge
            ));
            std::process::exit(0);
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "api::telemetry::tests::missing_console_preserves_identity_and_reports_diagnostic_unavailability", "--nocapture"])
            .env(CHILD, "1").output().unwrap();
        assert!(
            output.status.success(),
            "isolated missing-console startup failed"
        );
    }
}

pub(crate) use nddev_device_sync_accounts::telemetry::Delivery;
#[cfg(not(target_arch = "wasm32"))]
use nddev_device_sync_accounts::view::DeliveryCollection;
pub use nddev_device_sync_accounts::view::DeliveryView;

#[cfg(not(target_arch = "wasm32"))]
/// Whether the active local debug window was opened by the server-policy follower.
static FOLLOWER_OPENED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(not(target_arch = "wasm32"))]
/// The delivery the follower currently watches; one follower per delivery.
static FOLLOWING: Mutex<std::sync::Weak<Delivery>> = Mutex::new(std::sync::Weak::new());
#[cfg(not(target_arch = "wasm32"))]
/// One local window never exceeds this; it is re-armed while the server window lasts.
const FOLLOW_WINDOW_SECONDS: u64 = 300;
#[cfg(not(target_arch = "wasm32"))]
const FOLLOW_INTERVAL: Duration = Duration::from_secs(15);

#[cfg(not(target_arch = "wasm32"))]
/// What the follower does for one observed delivery policy (desktop#172).
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Follow {
    /// Open an I/O debug window for this many seconds.
    Open(u64),
    /// Close the window the follower opened.
    Close,
    Keep,
}

#[cfg(not(target_arch = "wasm32"))]
/// The server states a finite debug window per device; the app follows it with its own
/// local SDK window that never outlives the server's end. A window the user opened is
/// never closed by the follower.
pub(crate) fn follow_decision(
    collection: DeliveryCollection,
    debug_expires_at: u64,
    now: u64,
    active: bool,
    opened_by_follower: bool,
) -> Follow {
    match collection {
        DeliveryCollection::Debug if debug_expires_at > now => {
            if active {
                Follow::Keep
            } else {
                Follow::Open((debug_expires_at - now).min(FOLLOW_WINDOW_SECONDS))
            }
        }
        _ if active && opened_by_follower => Follow::Close,
        _ => Follow::Keep,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn follow_once(view: &DeliveryView) {
    use std::sync::atomic::Ordering;
    let Some(process) = logger() else {
        return;
    };
    let control = &process.logger.control;
    let active = control.debug_active();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |time| time.as_secs());
    match follow_decision(
        view.collection,
        view.debug_expires_at,
        now,
        active,
        FOLLOWER_OPENED.load(Ordering::Acquire),
    ) {
        Follow::Open(seconds) if seconds > 0 => {
            let window = DebugWindow {
                scope: DebugScope::Io.as_str().into(),
                duration: Duration::from_secs(seconds),
                events: 100,
            };
            if control.set_debug(Some(window)).is_ok() {
                FOLLOWER_OPENED.store(true, Ordering::Release);
            }
        }
        Follow::Close => {
            if control.set_debug(None).is_ok() {
                FOLLOWER_OPENED.store(false, Ordering::Release);
            }
        }
        _ if !active => FOLLOWER_OPENED.store(false, Ordering::Release),
        _ => {}
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Starts the follower for this delivery unless it already runs. It ends with the delivery.
pub(crate) fn follow_collection(delivery: &std::sync::Arc<Delivery>) {
    {
        let mut current = FOLLOWING.lock().unwrap_or_else(|error| error.into_inner());
        if current
            .upgrade()
            .is_some_and(|watched| std::sync::Arc::ptr_eq(&watched, delivery))
        {
            return;
        }
        *current = std::sync::Arc::downgrade(delivery);
    }
    let watched = std::sync::Arc::downgrade(delivery);
    drop(nddev_device_sync_accounts::runtime::spawn(async move {
        loop {
            let Some(delivery) = watched.upgrade() else {
                break;
            };
            let view = delivery.snapshot();
            drop(delivery);
            follow_once(&view);
            nddev_device_sync_accounts::runtime::sleep(FOLLOW_INTERVAL).await;
        }
    }));
}

pub(crate) fn start_delivery(
    identity: std::sync::Arc<nddev_device_sync_accounts::IdentityClient>,
    foreground: bool,
) -> Option<Delivery> {
    let process = logger()?;
    Some(Delivery::start(
        identity,
        process.logger.control.subscribe().ok()?,
        process.traces.subscribe().ok()?,
        foreground,
    ))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod follow_tests {
    use super::{Follow, follow_decision};
    use nddev_device_sync_accounts::view::DeliveryCollection::{Debug, Off, Standard, Unknown};

    #[test]
    fn the_local_window_follows_a_finite_server_debug_window() {
        // Seven minutes of server debug: windows of at most five minutes, ending with it.
        assert_eq!(
            follow_decision(Debug, 1_420, 1_000, false, false),
            Follow::Open(300)
        );
        assert_eq!(
            follow_decision(Debug, 1_420, 1_300, true, true),
            Follow::Keep
        );
        assert_eq!(
            follow_decision(Debug, 1_420, 1_301, false, true),
            Follow::Open(119)
        );
        // An ended or contradictory window opens nothing.
        assert_eq!(
            follow_decision(Debug, 1_000, 1_000, false, false),
            Follow::Keep
        );
        assert_eq!(follow_decision(Debug, 0, 1_000, false, false), Follow::Keep);
    }

    #[test]
    fn leaving_debug_closes_only_the_followers_window() {
        for mode in [Standard, Off, Unknown] {
            assert_eq!(follow_decision(mode, 0, 1_000, true, true), Follow::Close);
            // A window the user started from Utilities stays theirs.
            assert_eq!(follow_decision(mode, 0, 1_000, true, false), Follow::Keep);
            assert_eq!(follow_decision(mode, 0, 1_000, false, false), Follow::Keep);
        }
    }
}
