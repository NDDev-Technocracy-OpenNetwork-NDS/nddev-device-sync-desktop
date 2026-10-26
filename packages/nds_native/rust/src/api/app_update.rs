//! Application discovery and verified staging; separate from tools/updater.
//! There is deliberately no installation, activation or user-service control API.
#[cfg(feature = "flutter")]
use flutter_rust_bridge::frb;

#[cfg(target_os = "linux")]
mod linux;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppUpdateError {
    Unsupported,
    NotConfigured,
    Busy,
    Cancelled,
    Timeout,
    Network,
    TooLarge,
    Signature,
    Manifest,
    Source,
    Compatibility,
    NotNewer,
    Storage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppUpdateStatus {
    Unsupported,
    NotConfigured,
    Idle,
    Checking,
    Available,
    Downloading,
    Staged,
    Failed,
}

#[derive(Clone)]
pub struct AppUpdateView {
    pub status: AppUpdateStatus,
    pub version: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub error: Option<AppUpdateError>,
}

impl AppUpdateView {
    fn empty(status: AppUpdateStatus) -> Self {
        Self {
            status,
            version: String::new(),
            downloaded_bytes: 0,
            total_bytes: 0,
            error: None,
        }
    }
}

/// Policy is embedded by the owning assembly. No trust, URL, path or publisher
/// arguments are exposed to Flutter, an untrusted manifest, or runtime env vars.
#[cfg_attr(feature = "flutter", frb(opaque))]
pub struct AppUpdates {
    #[cfg(target_os = "linux")]
    inner: std::sync::Arc<linux::Updater>,
}

impl AppUpdates {
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            inner: std::sync::Arc::new(linux::Updater::embedded()),
        }
    }

    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn snapshot(&self) -> AppUpdateView {
        #[cfg(target_os = "linux")]
        {
            self.inner.snapshot()
        }
        #[cfg(not(target_os = "linux"))]
        {
            AppUpdateView::empty(AppUpdateStatus::Unsupported)
        }
    }

    pub async fn restore(&self) -> Result<AppUpdateView, AppUpdateError> {
        #[cfg(target_os = "linux")]
        {
            super::identity::observe_operation(
                "desktop",
                "app.update.restore",
                self.inner.restore(),
                AppUpdateError::Storage,
                classify,
            )
            .await
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(AppUpdateError::Unsupported)
        }
    }

    pub async fn check(&self) -> Result<AppUpdateView, AppUpdateError> {
        #[cfg(target_os = "linux")]
        {
            super::identity::observe_operation(
                "desktop",
                "app.update.check",
                self.inner.check(),
                AppUpdateError::Network,
                classify,
            )
            .await
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(AppUpdateError::Unsupported)
        }
    }

    pub async fn stage(&self) -> Result<AppUpdateView, AppUpdateError> {
        #[cfg(target_os = "linux")]
        {
            super::identity::observe_operation(
                "desktop",
                "app.update.stage",
                self.inner.stage(),
                AppUpdateError::Storage,
                classify,
            )
            .await
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(AppUpdateError::Unsupported)
        }
    }

    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn cancel(&self) {
        #[cfg(target_os = "linux")]
        self.inner.cancel();
    }

    pub async fn close(&self) {
        #[cfg(target_os = "linux")]
        self.inner.close().await;
    }
}

impl Default for AppUpdates {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "linux")]
fn classify(error: &AppUpdateError) -> (String, &'static str, bool) {
    let code = match error {
        AppUpdateError::Unsupported => "unsupported",
        AppUpdateError::NotConfigured => "not_configured",
        AppUpdateError::Busy => "busy",
        AppUpdateError::Cancelled => "cancelled",
        AppUpdateError::Timeout => "timeout",
        AppUpdateError::Network => "network",
        AppUpdateError::TooLarge => "too_large",
        AppUpdateError::Signature => "signature",
        AppUpdateError::Manifest => "manifest",
        AppUpdateError::Source => "source",
        AppUpdateError::Compatibility => "compatibility",
        AppUpdateError::NotNewer => "not_newer",
        AppUpdateError::Storage => "storage",
    };
    let retryable = matches!(
        error,
        AppUpdateError::Busy
            | AppUpdateError::Timeout
            | AppUpdateError::Network
            | AppUpdateError::Storage
    );
    (
        code.into(),
        if retryable { "error" } else { "refused" },
        retryable,
    )
}
