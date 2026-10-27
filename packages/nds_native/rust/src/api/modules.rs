#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::module_control::native_ownership::{self, BrowserValue, NativeOwner};
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
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    native_owner: Arc<NativeOwner>,
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
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub(super) async fn discover(
        owner: std::sync::Weak<super::module_control::ModuleControl>,
        identity_lifetime: nddev_device_sync_accounts::LocalUseLifetime,
        epoch: Arc<native_ownership::Epoch>,
    ) -> Result<Self, ClientError> {
        let inner = Arc::new(
            tokio::task::spawn_blocking(NativeAgent::discover)
                .await
                .map_err(|_| ClientError::InvalidResponse)?
                .map_err(|error| ClientError::Server(error.to_string()))?,
        );
        let control = owner.upgrade().ok_or(ClientError::Cancelled)?;
        let native_owner = match control.attach(&epoch, inner.clone()) {
            Ok(value) => value,
            Err(error) => {
                inner.close();
                return Err(error);
            }
        };
        Ok(Self {
            owner,
            identity_lifetime,
            inner,
            native_owner,
        })
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(super) fn check_client(&self, client: &NdsClient) -> Result<(), ClientError> {
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
            self.native_owner.close();
        }
    }
    /// Await the actual owned work and exact browser drain. Sync close/Drop
    /// merely cancel; neither is a shutdown receipt.
    pub async fn shutdown(&self) -> Result<(), ClientError> {
        self.close();
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let drain = self.native_owner.begin_drain();
            drain.wait().await?;
            if let Some(owner) = self.owner.upgrade() {
                owner.detach_drained(&self.native_owner, &drain)?;
            }
        }
        Ok(())
    }
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn modules(&self) -> Result<Vec<ModuleObservation>, ModuleError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            Ok(self.native_owner.observations())
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(ModuleError::UnsupportedPlatform)
        }
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    async fn browser_request<F, W>(
        &self,
        client: &NdsClient,
        mutation: bool,
        close: bool,
        work: W,
    ) -> Result<BrowserValue, ClientError>
    where
        W: FnOnce(Arc<NativeAgent>) -> F,
        F: std::future::Future<
                Output = Result<BrowserValue, nddev_device_sync_agent::view::BrowserError>,
            > + Send
            + 'static,
    {
        self.check_client(client)?;
        let guard = Arc::new(client.module_access("browser-use", Adapter::Native).await?);
        self.native_owner
            .start_request(
                mutation,
                close,
                native_ownership::checked(guard.clone(), work(self.inner.clone())),
            )?
            .wait(guard)
            .await
    }
    /// Explicit composition boundary, never implied by readiness or a saved
    /// enabled setting. A failed/unknown drain cannot be retried into consent.
    pub async fn resume_browser(&self, client: &NdsClient) -> Result<(), ClientError> {
        self.check_client(client)?;
        let guard = client.module_access("browser-use", Adapter::Native).await?;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            if !self.native_owner.can_resume() {
                return Err(ClientError::Busy);
            }
            self.native_owner.begin_drain().wait().await?;
            self.check_client(client)?;
            guard.validate().await?;
            self.native_owner.resume(&guard)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = guard;
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
    pub async fn browser_status(
        &self,
        client: &NdsClient,
    ) -> Result<super::module_types::BrowserStatus, ClientError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            match self
                .browser_request(client, false, false, |agent| async move {
                    agent.browser_status().await.map(BrowserValue::Status)
                })
                .await?
            {
                BrowserValue::Status(value) => Ok(value),
                _ => Err(ClientError::InvalidResponse),
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = client;
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
    pub async fn browser_sessions(
        &self,
        client: &NdsClient,
    ) -> Result<Vec<super::module_types::BrowserSession>, ClientError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            match self
                .browser_request(client, false, false, |agent| async move {
                    agent.browser_sessions().await.map(BrowserValue::Sessions)
                })
                .await?
            {
                BrowserValue::Sessions(value) => Ok(value),
                _ => Err(ClientError::InvalidResponse),
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = client;
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
    /// Dropping a mutation waiter cancels this exact owner's browser lifetime,
    /// including its browser siblings, and retains work/ACK for owned drain.
    /// Other owners, other modules and foreign/legacy sessions are unaffected.
    pub async fn browser_open(
        &self,
        client: &NdsClient,
        project: String,
    ) -> Result<super::module_types::BrowserSession, ClientError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            match self
                .browser_request(client, true, false, |agent| async move {
                    agent
                        .browser_open(&project)
                        .await
                        .map(BrowserValue::Session)
                })
                .await?
            {
                BrowserValue::Session(value) => Ok(value),
                _ => Err(ClientError::InvalidResponse),
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = (client, project);
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
    pub async fn browser_execute(
        &self,
        client: &NdsClient,
        project: String,
        code: String,
        timeout_ms: u32,
        expect: Option<super::module_types::BrowserDocument>,
    ) -> Result<super::module_types::BrowserOutcome, ClientError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            match self
                .browser_request(client, true, false, |agent| async move {
                    agent
                        .browser_execute(
                            &project,
                            &code,
                            std::time::Duration::from_millis(u64::from(timeout_ms)),
                            expect,
                        )
                        .await
                        .map(BrowserValue::Outcome)
                })
                .await?
            {
                BrowserValue::Outcome(value) => Ok(value),
                _ => Err(ClientError::InvalidResponse),
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = (client, project, code, timeout_ms, expect);
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
    /// An awaited close remains target-only. Abandoning its waiter has the same
    /// owner-level cancellation semantics as open/execute, not global close.
    pub async fn browser_close(
        &self,
        client: &NdsClient,
        project: String,
        close_tabs: bool,
    ) -> Result<(), ClientError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            match self
                .browser_request(client, true, true, |agent| async move {
                    agent
                        .browser_close(&project, close_tabs)
                        .await
                        .map(|()| BrowserValue::Closed)
                })
                .await?
            {
                BrowserValue::Closed => Ok(()),
                _ => Err(ClientError::InvalidResponse),
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = (client, project, close_tabs);
            Err(ClientError::Server("module_unsupported".into()))
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

impl Drop for LocalAgent {
    fn drop(&mut self) {
        // Async owners remain in ModuleControl until an awaited positive drain.
        self.close();
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
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        let epoch = self.module_control.generation()?;
        self.shared_identity().authorize_local().await?;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let agent = LocalAgent::discover(
                std::sync::Arc::downgrade(&self.module_control),
                self.shared_identity().local_use_lifetime(),
                epoch,
            )
            .await?;
            if let Err(error) = self
                .shared_identity()
                .check_captured_local_use_lifetime(&agent.identity_lifetime)
            {
                agent.close();
                return Err(error);
            }
            Ok(agent)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(ClientError::Server("module_unsupported".into()))
        }
    }
}
