#[cfg(feature = "flutter")]
use flutter_rust_bridge::frb;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use nddev_device_sync_agent::NativeAgent;
use nddev_device_sync_domain::modules::{ModuleError, ModuleObservation};

#[cfg_attr(feature = "flutter", frb(opaque))]
pub struct LocalAgent {
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    inner: NativeAgent,
}

#[cfg_attr(feature = "flutter", frb(sync))]
pub fn supports_local_agent() -> bool {
    cfg!(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    ))
}

impl LocalAgent {
    pub async fn discover() -> Result<Self, ModuleError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            Ok(Self {
                inner: tokio::task::spawn_blocking(NativeAgent::discover)
                    .await
                    .map_err(|_| ModuleError::Unavailable)??,
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(ModuleError::UnsupportedPlatform)
        }
    }
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn close(&self) {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        self.inner.close();
    }
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn modules(&self) -> Result<Vec<ModuleObservation>, ModuleError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            Ok(self.inner.modules())
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(ModuleError::UnsupportedPlatform)
        }
    }
    pub async fn system_info(
        &self,
    ) -> Result<nddev_device_sync_agent::view::sysinfo::SystemSnapshot, ModuleError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.inner.system_info().await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(ModuleError::UnsupportedPlatform)
        }
    }
    pub async fn clipboard_entries(
        &self,
        before: Option<u64>,
    ) -> Result<nddev_device_sync_agent::view::clipboard::Page, ModuleError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.inner.clipboard_entries(before).await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = before;
            Err(ModuleError::UnsupportedPlatform)
        }
    }
    pub async fn clipboard_content(
        &self,
        entry: u64,
        mime: String,
    ) -> Result<Vec<u8>, ModuleError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.inner.clipboard_content(entry, mime).await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = (entry, mime);
            Err(ModuleError::UnsupportedPlatform)
        }
    }
    pub async fn cleaner_diagnostics(
        &self,
    ) -> Result<nddev_device_sync_agent::view::cleaner::Diagnostics, ModuleError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.inner.cleaner_diagnostics().await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(ModuleError::UnsupportedPlatform)
        }
    }
    pub async fn cleaner_preview(
        &self,
    ) -> Result<nddev_device_sync_agent::view::cleaner::CleanerRunSummary, ModuleError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.inner.cleaner_preview().await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(ModuleError::UnsupportedPlatform)
        }
    }
    pub async fn cleaner_last_run(
        &self,
    ) -> Result<nddev_device_sync_agent::view::cleaner::CleanerRunSummary, ModuleError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.inner.cleaner_last_run().await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(ModuleError::UnsupportedPlatform)
        }
    }
    pub async fn update_plan(
        &self,
    ) -> Result<nddev_device_sync_agent::view::updater::Plan, ModuleError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.inner.update_plan().await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(ModuleError::UnsupportedPlatform)
        }
    }
    pub async fn update_last_run(
        &self,
    ) -> Result<nddev_device_sync_agent::view::updater::UpdateRunSummary, ModuleError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.inner.update_last_run().await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(ModuleError::UnsupportedPlatform)
        }
    }
    pub async fn repository_status(
        &self,
        path: String,
    ) -> Result<nddev_device_sync_agent::view::gds::Status, ModuleError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.inner
                .repository_status(std::path::Path::new(&path))
                .await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = path;
            Err(ModuleError::UnsupportedPlatform)
        }
    }
    pub async fn remote_sessions(
        &self,
    ) -> Result<nddev_device_sync_agent::view::rds::RemoteSnapshot, ModuleError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.inner.remote_sessions().await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(ModuleError::UnsupportedPlatform)
        }
    }
}

/// Presentation access to the provider's opaque data-carrying enum. The
/// canonical enum remains in updater; no DTO is copied or serialized to JSON.
#[cfg_attr(feature = "flutter", frb(sync))]
pub fn update_action_kind(action: &nddev_device_sync_updater::view::SystemUpdateAction) -> String {
    use nddev_device_sync_updater::view::SystemUpdateAction;
    match action {
        SystemUpdateAction::GdsInstall { .. } => "gds_install",
        SystemUpdateAction::VerifiedApplications => "verified_applications",
        SystemUpdateAction::AptObserve => "apt_observe",
        SystemUpdateAction::ToolchainObserve => "toolchain_observe",
        SystemUpdateAction::Homebrew => "homebrew",
        SystemUpdateAction::Flatpak => "flatpak",
    }
    .into()
}

#[cfg_attr(feature = "flutter", frb(sync))]
pub fn update_action_platform(
    action: &nddev_device_sync_updater::view::SystemUpdateAction,
) -> Option<String> {
    match action {
        nddev_device_sync_updater::view::SystemUpdateAction::GdsInstall { platform } => {
            Some(platform.clone())
        }
        _ => None,
    }
}
