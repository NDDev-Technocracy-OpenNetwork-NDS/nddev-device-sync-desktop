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
            standards: "unreleased".into(),
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
                "nddev_device_sync_gds".into(),
                "nddev_device_sync_rds".into(),
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
pub async fn shutdown_telemetry() -> LocalDiagnostics {
    #[cfg(not(target_arch = "wasm32"))]
    {
        tokio::task::spawn_blocking(shutdown_local)
            .await
            .unwrap_or_else(|_| LocalDiagnostics::unavailable())
    }
    #[cfg(target_arch = "wasm32")]
    {
        shutdown_local()
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
pub use nddev_device_sync_accounts::view::DeliveryView;

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
