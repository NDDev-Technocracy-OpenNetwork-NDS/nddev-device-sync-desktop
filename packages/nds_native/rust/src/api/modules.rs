use super::{identity::NdsClient, module_control::Adapter};
#[cfg(feature = "flutter")]
use flutter_rust_bridge::frb;
use nddev_device_sync_accounts::ClientError;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use nddev_device_sync_agent::NativeAgent;
use nddev_device_sync_domain::modules::{ModuleError, ModuleObservation};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use std::sync::Arc;

#[cfg_attr(feature = "flutter", frb(opaque))]
pub struct LocalAgent {
    owner: std::sync::Weak<super::module_control::ModuleControl>,
    identity_lifetime: nddev_device_sync_accounts::LocalUseLifetime,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    inner: Arc<NativeAgent>,
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
    #[cfg_attr(feature = "flutter", frb(ignore))]
    async fn discover(
        owner: std::sync::Weak<super::module_control::ModuleControl>,
        identity_lifetime: nddev_device_sync_accounts::LocalUseLifetime,
    ) -> Result<Self, ModuleError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            Ok(Self {
                owner,
                identity_lifetime,
                inner: Arc::new(
                    tokio::task::spawn_blocking(NativeAgent::discover)
                        .await
                        .map_err(|_| ModuleError::Unavailable)??,
                ),
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = (owner, identity_lifetime);
            Err(ModuleError::UnsupportedPlatform)
        }
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    fn check_client(&self, client: &NdsClient) -> Result<(), ClientError> {
        if self
            .owner
            .upgrade()
            .is_none_or(|owner| !std::sync::Arc::ptr_eq(&owner, &client.module_control))
        {
            return Err(ClientError::RevalidationRequired);
        }
        client
            .shared_identity()
            .check_captured_local_use_lifetime(&self.identity_lifetime)
    }
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn close(&self) {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.inner.close();
            if let Some(owner) = self.owner.upgrade() {
                owner.detach(&self.inner);
            }
        }
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
        client: &NdsClient,
    ) -> Result<nddev_device_sync_agent::view::sysinfo::SystemSnapshot, ClientError> {
        self.check_client(client)?;
        let guard = client.module_access("sysinfo", Adapter::Native).await?;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let result = self.inner.system_info().await;
            guard.validate().await?;
            guard.observe(&result);
            result.map_err(|error| ClientError::Server(error.to_string()))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = guard;
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
    pub async fn clipboard_entries(
        &self,
        client: &NdsClient,
        before: Option<u64>,
    ) -> Result<nddev_device_sync_agent::view::clipboard::Page, ClientError> {
        self.check_client(client)?;
        let guard = client.module_access("clipboard", Adapter::Native).await?;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let result = self.inner.clipboard_entries(before).await;
            guard.validate().await?;
            guard.observe(&result);
            result.map_err(|error| ClientError::Server(error.to_string()))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = (before, guard);
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
    pub async fn clipboard_content(
        &self,
        client: &NdsClient,
        entry: u64,
        mime: String,
    ) -> Result<Vec<u8>, ClientError> {
        self.check_client(client)?;
        let guard = client.module_access("clipboard", Adapter::Native).await?;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let result = self.inner.clipboard_content(entry, mime).await;
            guard.validate().await?;
            guard.observe(&result);
            result.map_err(|error| ClientError::Server(error.to_string()))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = (entry, mime, guard);
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
    pub async fn cleaner_diagnostics(
        &self,
        client: &NdsClient,
    ) -> Result<nddev_device_sync_agent::view::cleaner::Diagnostics, ClientError> {
        self.check_client(client)?;
        let guard = client.module_access("cleaner", Adapter::Native).await?;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let result = self.inner.cleaner_diagnostics().await;
            guard.validate().await?;
            guard.observe(&result);
            result.map_err(|error| ClientError::Server(error.to_string()))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = guard;
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
    pub async fn cleaner_preview(
        &self,
        client: &NdsClient,
    ) -> Result<nddev_device_sync_agent::view::cleaner::CleanerRunSummary, ClientError> {
        self.check_client(client)?;
        let guard = client.module_access("cleaner", Adapter::Native).await?;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let result = self.inner.cleaner_preview().await;
            guard.validate().await?;
            guard.observe(&result);
            result.map_err(|error| ClientError::Server(error.to_string()))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = guard;
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
    pub async fn cleaner_last_run(
        &self,
        client: &NdsClient,
    ) -> Result<nddev_device_sync_agent::view::cleaner::CleanerRunSummary, ClientError> {
        self.check_client(client)?;
        let guard = client.module_access("cleaner", Adapter::Native).await?;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let result = self.inner.cleaner_last_run().await;
            guard.validate().await?;
            guard.observe(&result);
            result.map_err(|error| ClientError::Server(error.to_string()))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = guard;
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
    pub async fn update_plan(
        &self,
        client: &NdsClient,
    ) -> Result<nddev_device_sync_agent::view::updater::Plan, ClientError> {
        self.check_client(client)?;
        let guard = client.module_access("updater", Adapter::Native).await?;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let result = self.inner.update_plan().await;
            guard.validate().await?;
            guard.observe(&result);
            result.map_err(|error| ClientError::Server(error.to_string()))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = guard;
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
    pub async fn update_last_run(
        &self,
        client: &NdsClient,
    ) -> Result<nddev_device_sync_agent::view::updater::UpdateRunSummary, ClientError> {
        self.check_client(client)?;
        let guard = client.module_access("updater", Adapter::Native).await?;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let result = self.inner.update_last_run().await;
            guard.validate().await?;
            guard.observe(&result);
            result.map_err(|error| ClientError::Server(error.to_string()))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = guard;
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
    pub async fn repository_status(
        &self,
        client: &NdsClient,
        path: String,
    ) -> Result<nddev_device_sync_agent::view::gds::Status, ClientError> {
        self.check_client(client)?;
        let guard = client.module_access("gds", Adapter::Native).await?;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let result = self
                .inner
                .repository_status(std::path::Path::new(&path))
                .await;
            guard.validate().await?;
            guard.observe(&result);
            result.map_err(|error| ClientError::Server(error.to_string()))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = (path, guard);
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
    pub async fn remote_sessions(
        &self,
        client: &NdsClient,
    ) -> Result<nddev_device_sync_agent::view::rds::RemoteSnapshot, ClientError> {
        self.check_client(client)?;
        let guard = client.module_access("rds", Adapter::Native).await?;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let result = self.inner.remote_sessions().await;
            guard.validate().await?;
            guard.observe(&result);
            result.map_err(|error| ClientError::Server(error.to_string()))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = guard;
            Err(ClientError::Server("module_unsupported".into()))
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

impl NdsClient {
    pub async fn discover_local_agent(&self) -> Result<LocalAgent, ClientError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        let _discovery = self
            .module_control
            .discovery
            .try_lock()
            .map_err(|_| ClientError::Busy)?;
        self.shared_identity().authorize_local().await?;
        let agent = LocalAgent::discover(
            std::sync::Arc::downgrade(&self.module_control),
            self.shared_identity().local_use_lifetime(),
        )
        .await
        .map_err(|error| ClientError::Server(error.to_string()))?;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        if let Err(error) = self.module_control.attach(&agent.inner) {
            agent.close();
            return Err(error);
        }
        if let Err(error) = self
            .shared_identity()
            .check_captured_local_use_lifetime(&agent.identity_lifetime)
        {
            agent.close();
            return Err(error);
        }
        Ok(agent)
    }
}
