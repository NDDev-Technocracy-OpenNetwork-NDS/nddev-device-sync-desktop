#[cfg(feature = "flutter")]
use flutter_rust_bridge::frb;
use nddev_device_sync_accounts::runtime::{self, Instant};
use nddev_device_sync_accounts::{
    IdentityClient,
    view::{ClientError, PasskeyView, View},
};
use nddev_device_sync_application::lifecycle::{
    BatchOutcome, ClientLifecycle as Activity, ReplicaSchedule,
};
#[cfg(test)]
use nddev_device_sync_application::replica::CapacityProbeOutcome;
use nddev_device_sync_domain::Platform;
use nddev_device_sync_telemetry::view::ClientLifecycle;
#[cfg(test)]
use nddev_device_sync_vault::replication::recovered_policy;
use nddev_device_sync_vault::replication::{
    ReplicaContext, ReplicaTask as Replica, batch_outcome, device_observations,
};
use nddev_device_sync_vault::{
    VaultClient,
    view::{RecordPage, StatePage, VaultError, VaultView},
};
use std::{
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tracing::Instrument;
use zeroize::Zeroizing;

#[cfg_attr(feature = "flutter", frb(opaque))]
pub struct NdsClient {
    inner: Arc<IdentityClient>,
    pub(crate) catalog: Mutex<super::catalog::CatalogState>,
    pub(crate) module_control: Arc<super::module_control::ModuleControl>,
    vault: tokio::sync::Mutex<Option<Arc<VaultClient>>>,
    replica: Mutex<Option<Replica>>,
    replica_control: tokio::sync::Mutex<()>,
    pub(super) publication: Arc<Mutex<()>>,
    replica_schedule: Arc<Mutex<Arc<Mutex<ReplicaSchedule>>>>,
    active: AtomicBool,
    replication_active: AtomicBool,
    pub(crate) screen: Mutex<Option<super::screen::ScreenSlot>>,
    // Raw lifecycle fact; admission and publication share this lifetime owner.
    pub(crate) screen_lifecycle: Mutex<ClientLifecycle>,
    delivery: Mutex<Option<super::telemetry::Delivery>>,
    #[cfg(not(target_arch = "wasm32"))]
    clipboard: nddev_device_sync_clipboard::os::TextClipboard,
}

struct ClipboardReservation(Option<String>);
impl Drop for ClipboardReservation {
    fn drop(&mut self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(id) = &self.0 {
            nddev_device_sync_clipboard::browser::cancel_write(id);
        }
    }
}

impl NdsClient {
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(crate) fn shared_identity(&self) -> &Arc<IdentityClient> {
        &self.inner
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(crate) fn catalog_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }
    pub fn new(endpoint: String) -> Result<Self, ClientError> {
        Ok(Self {
            inner: Arc::new(IdentityClient::new(&endpoint)?),
            catalog: Mutex::default(),
            module_control: Arc::default(),
            vault: tokio::sync::Mutex::new(None),
            replica: Mutex::default(),
            replica_control: tokio::sync::Mutex::default(),
            publication: Arc::default(),
            replica_schedule: Arc::default(),
            active: AtomicBool::new(true),
            replication_active: AtomicBool::new(true),
            screen: Mutex::default(),
            screen_lifecycle: Mutex::new(ClientLifecycle::Detached),
            delivery: Mutex::default(),
            #[cfg(not(target_arch = "wasm32"))]
            clipboard: Default::default(),
        })
    }
    pub async fn refresh(&self) -> Result<View, ClientError> {
        if let Some(task) = self.cancel_work(false) {
            task.join().await;
        }
        self.inner.refresh().await
    }
    /// Reconcile shared local authorization on foreground ticks/resume. Pending
    /// logout retries through the identity owner; no presentation auth rules.
    pub async fn reconcile(&self) -> Result<View, ClientError> {
        let view = self.inner.view();
        let lifetime = self.inner.local_use_lifetime();
        if !self.replication_active.load(Ordering::Relaxed) || !view.flow.is_empty() {
            return Ok(view);
        }
        let offline = view.offline_authorized;
        let view = match self.inner.maintain_authorization().await {
            Ok(view) => view,
            Err(ClientError::Busy | ClientError::OnlineVerificationRequired) => {
                return Ok(self.inner.view());
            }
            Err(error) => {
                self.clear_replica_fenced(&lifetime).await;
                return Err(error);
            }
        };
        if offline && view.session_verified {
            self.wake_replica();
        }
        let revoked = view
            .devices
            .iter()
            .any(|device| device.is_current && device.revoked);
        if view.authenticated && !view.current_device_id.is_empty() && !revoked {
            let start = async {
                let fence = self.inner.local_use_fence_from(&lifetime).await?;
                self.start_replica_fenced(Duration::ZERO, &fence).await
            }
            .await;
            match start {
                Ok(()) => {
                    self.reconcile_screen().await;
                    return Ok(self.inner.view());
                }
                Err(ClientError::Busy) => return Ok(self.inner.view()),
                Err(error) => {
                    self.clear_replica_fenced(&lifetime).await;
                    return Err(error);
                }
            }
        }
        // A known device revocation closes device capabilities while the NDS
        // session remains valid for inventory and explicit sign-out.
        if !view.authenticated || revoked {
            self.clear_replica_fenced(&lifetime).await;
        }
        Ok(self.inner.view())
    }
    pub async fn request_email(&self, email: String, locale: String) -> Result<View, ClientError> {
        self.cancel_screen();
        self.invalidate_catalog();
        self.inner.request_email(email, &locale).await
    }
    pub async fn verify_email(&self, code: String) -> Result<View, ClientError> {
        let view = self.inner.verify_email(code).await?;
        self.reset_replica_session().await;
        Ok(view)
    }
    pub async fn start_github(&self, locale: String) -> Result<View, ClientError> {
        self.cancel_screen();
        self.invalidate_catalog();
        self.inner.start_github(&locale).await
    }
    pub async fn start_passkey(
        &self,
        registration: bool,
        email: Option<String>,
        locale: String,
    ) -> Result<View, ClientError> {
        self.invalidate_catalog();
        self.inner.start_passkey(registration, email, &locale).await
    }
    pub async fn passkeys(&self) -> Result<Vec<PasskeyView>, ClientError> {
        self.inner.passkeys().await
    }
    pub async fn revoke_passkey(&self, id: String) -> Result<View, ClientError> {
        let view = self.inner.revoke_passkey(id).await?;
        if !view.authenticated {
            self.invalidate_catalog();
            self.delivery
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take();
            self.stop_replica().await;
            self.vault.lock().await.take();
        }
        Ok(view)
    }
    pub async fn poll_passkey(&self) -> Result<View, ClientError> {
        let sign_in = self.inner.view().flow == "passkey";
        let view = self.inner.poll_passkey().await?;
        if sign_in && view.authenticated && view.flow.is_empty() {
            self.reset_replica_session().await;
        }
        Ok(view)
    }
    pub async fn poll_github(&self) -> Result<View, ClientError> {
        let sign_in = self.inner.view().flow == "github";
        let view = self.inner.poll_github().await?;
        if sign_in && view.authenticated && view.flow.is_empty() {
            self.reset_replica_session().await;
        }
        Ok(view)
    }
    pub async fn logout(&self) -> Result<View, ClientError> {
        self.module_control.stop();
        self.cancel_screen();
        self.invalidate_catalog();
        self.delivery
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        self.stop_replica().await;
        let view = self.inner.logout().await?;
        self.vault.lock().await.take();
        {
            let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
            self.reset_replica_schedule();
        }
        Ok(view)
    }
    pub async fn refresh_devices(&self) -> Result<View, ClientError> {
        self.inner.refresh_devices().await
    }
    pub async fn prepare_device(&self, name: String) -> Result<View, ClientError> {
        let lifetime = self.inner.local_use_lifetime();
        self.stop_replica().await;
        let view = if self.inner.view().offline_authorized {
            self.inner.view()
        } else {
            self.inner.prepare_device(name).await?
        };
        let fence = self.inner.local_use_fence_from(&lifetime).await?;
        self.start_replica_fenced(Duration::ZERO, &fence).await?;
        Ok(view)
    }
    fn ensure_delivery(&self) {
        {
            let mut delivery = self
                .delivery
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if delivery.as_ref().is_none_or(|delivery| delivery.finished()) {
                delivery.take();
                *delivery = super::telemetry::start_delivery(
                    self.inner.clone(),
                    self.replication_active.load(Ordering::Relaxed),
                );
            }
        }
    }
    pub async fn revoke_device(&self, id: String) -> Result<View, ClientError> {
        self.cancel_screen();
        let view = self.inner.revoke_device(id).await?;
        if view
            .devices
            .iter()
            .any(|device| device.is_current && device.revoked)
        {
            self.delivery
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take();
            self.cancel_screen();
            self.invalidate_catalog();
            self.stop_replica().await;
        }
        Ok(view)
    }
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn set_lifecycle(&self, state: ClientLifecycle) {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        *self
            .screen_lifecycle
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = state;
        let active = client_work_permitted(state);
        let replication = client_replication_permitted(state);
        let interactive_changed = self.active.swap(active, Ordering::Relaxed) != active;
        let replication_changed =
            self.replication_active.swap(replication, Ordering::Relaxed) != replication;
        if !interactive_changed && !replication_changed {
            return;
        }
        if (interactive_changed && active) || (replication_changed && replication) {
            // Wake existing owners only after closing stale online admission.
            // Offline local use remains bounded by the retained protected grant.
            self.inner.require_online_verification();
        }
        if interactive_changed && !active {
            self.cancel_screen();
            self.invalidate_catalog();
        }
        if !replication_changed {
            return;
        }
        if let Some(delivery) = &*self
            .delivery
            .lock()
            .unwrap_or_else(|error| error.into_inner())
        {
            delivery.foreground(replication);
        }
        if let Some(replica) = &*self.replica.lock().unwrap_or_else(|e| e.into_inner()) {
            replica.set_active(replication);
        }
    }
    async fn stop_replica(&self) {
        let replica = self
            .replica
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(mut replica) = replica
            && let Some(task) = replica.cancel()
        {
            task.join().await;
        }
    }
    async fn reset_replica_session(&self) {
        self.module_control.stop();
        self.cancel_screen();
        self.invalidate_catalog();
        self.stop_replica().await;
        self.vault.lock().await.take();
        // Detach the old session policy: a late result cannot block the new one.
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        self.reset_replica_schedule();
    }
    async fn clear_replica_fenced(&self, lifetime: &nddev_device_sync_accounts::LocalUseLifetime) {
        let task = {
            let mut vault = self.vault.lock().await;
            let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
            if self
                .inner
                .check_captured_local_use_lifetime(lifetime)
                .is_err()
            {
                return;
            }
            self.module_control.stop();
            self.cancel_screen();
            self.invalidate_catalog();
            self.delivery
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            vault.take();
            self.replica
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
                .and_then(|mut replica| replica.cancel())
        };
        if let Some(task) = task {
            task.join().await;
        }
    }
    async fn start_replica_fenced(
        &self,
        initial_delay: Duration,
        fence: &nddev_device_sync_accounts::LocalUseFence,
    ) -> Result<(), ClientError> {
        // A foreground tick must not replace a replica while manual sync owns
        // its stop/retry/restart sequence. That caller restarts it on completion.
        let Ok(_control) = self.replica_control.try_lock() else {
            return Ok(());
        };
        self.start_replica_for_schedule(initial_delay, fence, self.schedule())
            .await
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    async fn start_replica_for_schedule(
        &self,
        initial_delay: Duration,
        fence: &nddev_device_sync_accounts::LocalUseFence,
        schedule: Arc<Mutex<ReplicaSchedule>>,
    ) -> Result<(), ClientError> {
        let vault = self.vault().await.map_err(replica_start_error)?;
        self.inner.validate_local_use(fence).await?;
        // Protected I/O is complete before this brief cancellation/publication boundary.
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        self.inner.check_local_use_lifetime(fence)?;
        if self.spawn_replica(initial_delay, Arc::downgrade(&vault), schedule) {
            self.ensure_delivery();
        }
        Ok(())
    }
    // Caller holds publication: policy replacement and task publication cannot
    // interleave after this check. The task retains this exact policy identity.
    fn spawn_replica(
        &self,
        initial_delay: Duration,
        weak: Weak<VaultClient>,
        schedule: Arc<Mutex<ReplicaSchedule>>,
    ) -> bool {
        if !Arc::ptr_eq(&schedule, &self.schedule()) {
            return false;
        }
        let mut slot = self.replica.lock().unwrap_or_else(|e| e.into_inner());
        if slot.as_ref().is_some_and(Replica::is_running) {
            return true;
        }
        slot.take();
        *slot = Some(Replica::spawn(
            ReplicaContext {
                identity: self.inner.clone(),
                vault: weak,
                publication: self.publication.clone(),
                policy_slot: self.replica_schedule.clone(),
                schedule,
                client_version: Some(env!("CARGO_PKG_VERSION").into()),
            },
            initial_delay,
            self.replication_active.load(Ordering::Relaxed),
        ));
        true
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(crate) async fn vault(&self) -> Result<Arc<VaultClient>, VaultError> {
        let mut slot = self.vault.lock().await;
        if let Some(vault) = &*slot {
            return Ok(vault.clone());
        }
        let vault = Arc::new(VaultClient::new(self.inner.clone(), data_directory()?).await?);
        *slot = Some(vault.clone());
        Ok(vault)
    }
    pub async fn vault_refresh(&self) -> Result<VaultView, VaultError> {
        observe("vault.refresh", self.vault_snapshot()).await
    }
    pub async fn vault_snapshot(&self) -> Result<VaultView, VaultError> {
        let mut view = self.vault().await?.refresh_local().await?;
        if self.capacity_blocked() {
            // Preserve a real current read/auth error; healthy inbox reads still
            // display the retained upload-capacity reason.
            if view.sync_error.is_empty() {
                view.sync_error = VaultError::CapacityExhausted.code();
            }
            view.catching_up = true;
        }
        Ok(view)
    }
    pub async fn vault_sync(&self) -> Result<VaultView, VaultError> {
        let lifetime = self.inner.local_use_lifetime();
        observe("vault.sync", async {
            let _control = self
                .replica_control
                .try_lock()
                .map_err(|_| VaultError::Busy)?;
            self.stop_replica().await;
            let fence = self
                .inner
                .local_use_fence_from(&lifetime)
                .await
                .map_err(replica_auth_error)?;
            let schedule = {
                let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
                self.inner
                    .check_local_use_lifetime(&fence)
                    .map_err(replica_auth_error)?;
                // An explicit retry owns fresh policy. Already running state
                // publishers may finish, but can only update their old policy.
                self.reset_replica_schedule()
            };
            let mut history = None;
            let result = async {
                let vault = self.vault().await?;
                let attempt = vault.synchronize_attempt(false).await?;
                history = attempt.history;
                attempt.result.map(|progress| progress.view)
            }
            .await;
            let next = {
                let mut policy = schedule.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(history) = history {
                    policy.observe_history(history);
                }
                policy.after(batch_outcome(&result))
            };
            if let Some(delay) = next {
                self.start_replica_for_schedule(delay, &fence, schedule)
                    .await
                    .map_err(replica_auth_error)?;
            }
            result
        })
        .await
    }
    pub async fn vault_records(&self, after: String) -> Result<RecordPage, VaultError> {
        observe("vault.records", async {
            self.vault().await?.records(after).await
        })
        .await
    }
    pub async fn vault_save(
        &self,
        id: Option<String>,
        expected_revision: u64,
        expected_pending: Option<String>,
        title: String,
        username: String,
        secret: String,
    ) -> Result<VaultView, VaultError> {
        // Own and clear supplied secret bytes even if authorization/storage fails.
        let secret = Zeroizing::new(secret);
        let result = observe("vault.record.save", async {
            let vault = self.vault().await?;
            vault
                .save_record(
                    id,
                    expected_revision,
                    expected_pending,
                    title,
                    username,
                    secret,
                )
                .await?;
            Ok(vault.snapshot())
        })
        .await?;
        self.wake_replica();
        Ok(result)
    }
    pub async fn vault_delete(
        &self,
        id: String,
        expected_revision: u64,
        expected_pending: Option<String>,
    ) -> Result<VaultView, VaultError> {
        let result = observe("vault.record.delete", async {
            let vault = self.vault().await?;
            vault
                .delete_record(id, expected_revision, expected_pending)
                .await?;
            Ok(vault.snapshot())
        })
        .await?;
        self.wake_replica();
        Ok(result)
    }
    pub async fn vault_resolve(
        &self,
        id: String,
        expected_revision: u64,
        expected_pending: Option<String>,
        expected_cursor: u64,
        use_conflict: bool,
    ) -> Result<VaultView, VaultError> {
        let result = observe("vault.record.resolve", async {
            let vault = self.vault().await?;
            vault
                .resolve_record(
                    id,
                    expected_revision,
                    expected_cursor,
                    expected_pending,
                    use_conflict,
                )
                .await?;
            Ok(vault.snapshot())
        })
        .await?;
        self.wake_replica();
        Ok(result)
    }
    pub(crate) fn wake_replica(&self) {
        if self.replication_active.load(Ordering::Relaxed)
            && let Some(replica) = &*self
                .replica
                .lock()
                .unwrap_or_else(|error| error.into_inner())
        {
            replica.set_active(true);
        }
    }
    pub async fn clipboard_snapshot(
        &self,
    ) -> Result<nddev_device_sync_vault::view::ClipboardView, ClientError> {
        let guard = self
            .module_access("clipboard", super::module_control::Adapter::Shared)
            .await?;
        let value = self
            .vault()
            .await
            .map_err(super::module_control::vault_error)?
            .clipboard()
            .await
            .map_err(super::module_control::vault_error)?;
        guard.validate().await?;
        Ok(value.map_or_else(Default::default, |(metadata, _)| metadata))
    }
    pub async fn clipboard_publish_text(&self, text: String) -> Result<String, ClientError> {
        let text = Zeroizing::new(text);
        let result = super::module_control::observe("clipboard", "clipboard.publish", async {
            self.module_access("clipboard", super::module_control::Adapter::Shared)
                .await?;
            self.vault()
                .await
                .map_err(super::module_control::vault_error)?
                .share_clipboard(text)
                .await
                .map_err(super::module_control::vault_error)
        })
        .await?;
        self.wake_replica();
        Ok(result)
    }
    pub async fn clipboard_publish_archive(
        &self,
        agent: &super::modules::LocalAgent,
        entry: u64,
        mime: String,
    ) -> Result<String, ClientError> {
        if !matches!(
            mime.as_str(),
            "text/plain" | "text/plain;charset=utf-8" | "text/plain;charset=UTF-8"
        ) {
            return Err(ClientError::InvalidInput);
        }
        let result =
            super::module_control::observe("clipboard", "clipboard.archive.publish", async {
                let guard = self
                    .module_access("clipboard", super::module_control::Adapter::Shared)
                    .await?;
                let bytes = Zeroizing::new(agent.clipboard_content(self, entry, mime).await?);
                if bytes.len() > 8192 {
                    return Err(ClientError::InvalidInput);
                }
                let text = Zeroizing::new(
                    String::from_utf8(bytes.to_vec()).map_err(|_| ClientError::InvalidResponse)?,
                );
                guard.validate().await?;
                self.vault()
                    .await
                    .map_err(super::module_control::vault_error)?
                    .share_clipboard(text)
                    .await
                    .map_err(super::module_control::vault_error)
            })
            .await?;
        self.wake_replica();
        Ok(result)
    }
    pub async fn clipboard_publish_native(&self) -> Result<String, ClientError> {
        #[cfg(target_os = "android")]
        {
            let guard = self
                .module_access("clipboard", super::module_control::Adapter::Shared)
                .await?;
            if !self.active.load(Ordering::Relaxed) {
                return Err(ClientError::Busy);
            }
            // Preserve the native allowlisted refusal reason, never its content.
            let text = self
                .clipboard
                .read()
                .await
                .map_err(|error| ClientError::Server(error.to_string()))?;
            guard.validate().await?;
            let vault = self
                .vault()
                .await
                .map_err(|error| ClientError::Server(error.code()))?;
            let result = vault
                .share_clipboard(text)
                .await
                .map_err(|error| ClientError::Server(error.code()))?;
            self.wake_replica();
            Ok(result)
        }
        #[cfg(not(target_os = "android"))]
        {
            Err(ClientError::Server("clipboard_unsupported".into()))
        }
    }
    /// Called synchronously in the explicit user gesture, before async decryption.
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn reserve_clipboard_copy(&self) -> Result<Option<String>, VaultError> {
        #[cfg(target_arch = "wasm32")]
        {
            nddev_device_sync_clipboard::browser::reserve_write()
                .map(Some)
                .map_err(clipboard_error)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Ok(None)
        }
    }
    async fn write_clipboard(
        &self,
        text: Zeroizing<String>,
        sensitive: bool,
        reserved: &ClipboardReservation,
    ) -> Result<(), VaultError> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = sensitive;
            nddev_device_sync_clipboard::browser::write_reserved(
                reserved.0.clone().ok_or(VaultError::Invalid)?,
                text,
            )
            .await
            .map_err(clipboard_error)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if reserved.0.is_some() {
                return Err(VaultError::Invalid);
            }
            self.clipboard
                .write(text, sensitive)
                .await
                .map_err(clipboard_error)
        }
    }
    pub async fn clipboard_copy(&self, ticket: Option<String>) -> Result<(), ClientError> {
        let reserved = ClipboardReservation(ticket);
        super::module_control::observe("clipboard", "clipboard.copy", async {
            let guard = self
                .module_access("clipboard", super::module_control::Adapter::Shared)
                .await?;
            let (_, text) = self
                .vault()
                .await
                .map_err(super::module_control::vault_error)?
                .clipboard()
                .await
                .map_err(super::module_control::vault_error)?
                .ok_or(ClientError::InvalidInput)?;
            guard.validate().await?;
            let result = self.write_clipboard(text, false, &reserved).await;
            guard.observe(&result.as_ref().map(|_| ()).map_err(|error| match error {
                VaultError::Busy => nddev_device_sync_domain::modules::ModuleError::Busy,
                VaultError::Cancelled => nddev_device_sync_domain::modules::ModuleError::Cancelled,
                VaultError::Invalid => nddev_device_sync_domain::modules::ModuleError::InvalidInput,
                _ => nddev_device_sync_domain::modules::ModuleError::Unavailable,
            }));
            result.map_err(super::module_control::vault_error)
        })
        .await
    }
    pub async fn vault_copy_secret(
        &self,
        id: String,
        ticket: Option<String>,
    ) -> Result<(), VaultError> {
        let reserved = ClipboardReservation(ticket);
        observe("vault.secret.copy", async {
            let text = self.vault().await?.native_secret(id).await?;
            self.write_clipboard(text, true, &reserved).await
        })
        .await
    }
    pub async fn incidents(&self, after: String) -> Result<IncidentPage, ClientError> {
        observe_incident("observability.incidents.read", async {
            let reply = self
                .inner
                .sync_request(nddev_device_sync_accounts::SyncRequest::Incidents { after })
                .await?;
            let page: nddev_device_sync_accounts::observability_wire::IncidentPage =
                serde_json::from_slice(&reply.body).map_err(|_| ClientError::InvalidResponse)?;
            if page.incidents.len() > 50 {
                return Err(ClientError::InvalidResponse);
            }
            let mut incidents = Vec::new();
            for incident in page.incidents {
                incidents.push(incident_view(incident)?);
            }
            Ok(IncidentPage {
                incidents,
                next_cursor: page.next_cursor.unwrap_or_default(),
                generated_at: page.generated_at,
            })
        })
        .await
    }
    pub async fn incident_action(
        &self,
        id: String,
        revision: u64,
        silence_seconds: u32,
    ) -> Result<IncidentView, ClientError> {
        let event = if silence_seconds == 0 {
            "observability.incident.acknowledge"
        } else {
            "observability.incident.silence"
        };
        observe_incident(event, async {
            if !(1..=9_007_199_254_740_991).contains(&revision) || silence_seconds > 86_400 {
                return Err(ClientError::InvalidInput);
            }
            let operation_id = uuid::Uuid::now_v7().to_string();
            let expected_revision = revision as i64;
            let body = if silence_seconds == 0 {
                serde_json::to_vec(
                    &nddev_device_sync_accounts::observability_wire::IncidentAction {
                        operation_id,
                        expected_revision,
                    },
                )
            } else {
                let until = (time::OffsetDateTime::now_utc()
                    + time::Duration::seconds(i64::from(silence_seconds)))
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(|_| ClientError::InvalidInput)?;
                serde_json::to_vec(
                    &nddev_device_sync_accounts::observability_wire::SilenceAction {
                        operation_id,
                        expected_revision,
                        until,
                    },
                )
            }
            .map_err(|_| ClientError::InvalidInput)?;
            let reply = self
                .inner
                .sync_request(nddev_device_sync_accounts::SyncRequest::IncidentAction {
                    id,
                    silence: silence_seconds > 0,
                    body,
                })
                .await?;
            incident_view(
                serde_json::from_slice(&reply.body).map_err(|_| ClientError::InvalidResponse)?,
            )
        })
        .await
    }
    pub async fn sync_states(&self, after: String) -> Result<StatePage, VaultError> {
        observe("sync.states.read", async {
            self.vault().await?.states(after).await
        })
        .await
    }
    pub async fn publish_device_state(&self) -> Result<VaultView, VaultError> {
        self.publish_state_inner(Vec::new()).await
    }
    pub async fn publish_local_state(
        &self,
        agent: &super::modules::LocalAgent,
    ) -> Result<VaultView, VaultError> {
        self.publish_state_inner(agent.modules().map_err(|_| VaultError::Unavailable)?)
            .await
    }
    async fn publish_state_inner(
        &self,
        modules: Vec<nddev_device_sync_domain::modules::ModuleObservation>,
    ) -> Result<VaultView, VaultError> {
        use nddev_device_sync_domain::state::Snapshot;
        let schedule = self.schedule();
        let mut attempted = false;
        let mut history = None;
        let result = observe("sync.states.publish", async {
            if schedule
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_capacity_blocked()
            {
                return Err(VaultError::CapacityExhausted);
            }
            let vault = self.vault().await?;
            self.inner
                .refresh_devices()
                .await
                .map_err(|_| VaultError::Unavailable)?;
            let mut values = device_observations(&self.inner, env!("CARGO_PKG_VERSION"))?;
            let observed_at_ms = values[0].observed_at_ms;
            let capture = |data| Snapshot {
                schema_version: 1,
                observed_at_ms,
                data,
            };
            for module in modules {
                values.push(capture(module.state()));
            }
            attempted = true;
            let attempt = vault.publish_states_attempt(values).await?;
            history = attempt.history;
            attempt.result
        })
        .await;
        if attempted && matches!(&result, Err(VaultError::CapacityExhausted)) {
            let mut policy = schedule.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(history) = history {
                policy.observe_history(history);
            }
            policy.after(BatchOutcome::CapacityExhausted);
        }
        result
    }
    fn capacity_blocked(&self) -> bool {
        self.schedule()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_capacity_blocked()
    }
    fn schedule(&self) -> Arc<Mutex<ReplicaSchedule>> {
        self.replica_schedule
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    // Caller holds publication, including when replacing an identity session.
    fn reset_replica_schedule(&self) -> Arc<Mutex<ReplicaSchedule>> {
        let schedule = Arc::default();
        *self
            .replica_schedule
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Arc::clone(&schedule);
        schedule
    }
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn snapshot(&self) -> View {
        self.inner.view()
    }
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn delivery_snapshot(&self) -> super::telemetry::DeliveryView {
        self.delivery
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .map_or_else(Default::default, |delivery| delivery.snapshot())
    }
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn cancel(&self) {
        self.module_control.stop();
        self.cancel_work(true);
    }
    fn cancel_work(&self, clear_sign_in: bool) -> Option<runtime::Task> {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        self.cancel_screen();
        self.invalidate_catalog();
        self.delivery
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let task = self
            .replica
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .and_then(|mut replica| replica.cancel());
        if clear_sign_in {
            self.inner.cancel_sign_in();
        } else {
            self.inner.cancel();
        }
        task
    }
}

/// Presentation reports raw lifecycle once; Core retains interactive admission.
#[cfg_attr(feature = "flutter", frb(sync))]
pub fn client_work_permitted(state: ClientLifecycle) -> bool {
    client_activity(state).is_some_and(|(activity, platform)| activity.permits_work(platform))
}

fn client_replication_permitted(state: ClientLifecycle) -> bool {
    client_activity(state)
        .is_some_and(|(activity, platform)| activity.permits_replication(platform))
}

fn client_activity(state: ClientLifecycle) -> Option<(Activity, Platform)> {
    let activity = match state {
        ClientLifecycle::Detached => Activity::Detached,
        ClientLifecycle::Resumed => Activity::Resumed,
        ClientLifecycle::Inactive => Activity::Inactive,
        ClientLifecycle::Hidden => Activity::Hidden,
        ClientLifecycle::Paused => Activity::Paused,
        ClientLifecycle::Ready => return None,
    };
    let platform = if cfg!(target_arch = "wasm32") {
        Platform::Web
    } else if cfg!(target_os = "linux") {
        Platform::Linux
    } else if cfg!(target_os = "macos") {
        Platform::Macos
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "android") {
        Platform::Android
    } else if cfg!(target_os = "ios") {
        Platform::Ios
    } else {
        return None;
    };
    Some((activity, platform))
}

fn replica_start_error(error: VaultError) -> ClientError {
    match error {
        VaultError::Busy => ClientError::Busy,
        VaultError::Locked => ClientError::RevalidationRequired,
        // Constructing a local replica performs no server request.
        VaultError::Unavailable => ClientError::Server("vault_storage_unavailable".into()),
        other => ClientError::Server(other.code()),
    }
}

fn replica_auth_error(error: ClientError) -> VaultError {
    match error {
        ClientError::Busy | ClientError::OnlineVerificationRequired => VaultError::Busy,
        ClientError::Server(code) if code == "server_busy" => VaultError::Busy,
        ClientError::Server(code) if code == "sync_capacity_exhausted" => {
            VaultError::CapacityExhausted
        }
        ClientError::DeviceRevoked | ClientError::RevalidationRequired => VaultError::Locked,
        _ => VaultError::Unavailable,
    }
}

fn clipboard_error(error: nddev_device_sync_clipboard::os::Error) -> VaultError {
    tracing::event!(target:"nds_native",tracing::Level::WARN,{event.name="clipboard.delivery.failed",module="clipboard",scope="io",error.type=%error,outcome="error"});
    VaultError::Unavailable
}

// User actions produce one terminal event outside delivery-suppressed I/O.
fn observe_incident<T>(
    event: &'static str,
    work: impl std::future::Future<Output = Result<T, ClientError>>,
) -> impl std::future::Future<Output = Result<T, ClientError>> {
    observe_operation(
        "observability",
        event,
        work,
        ClientError::TelemetryUnavailable,
        |error| {
            (
                error.code(),
                "error",
                matches!(
                    error,
                    ClientError::InvalidResponse
                        | ClientError::Transport
                        | ClientError::Timeout
                        | ClientError::CredentialStore
                        | ClientError::TelemetryUnavailable
                ),
            )
        },
    )
}

fn observe<T>(
    event: &'static str,
    work: impl std::future::Future<Output = Result<T, VaultError>>,
) -> impl std::future::Future<Output = Result<T, VaultError>> {
    observe_operation("vault", event, work, VaultError::Unavailable, |error| {
        (
            error.code(),
            "error",
            matches!(
                error,
                VaultError::Unavailable
                    | VaultError::Authentication
                    | VaultError::StorageFull
                    | VaultError::CapacityExhausted
            ),
        )
    })
}

// One terminal caller event and process span; transport owners retain their I/O spans.
#[cfg_attr(feature = "flutter", frb(ignore))]
pub(crate) fn observe_operation<T, E>(
    module: &str,
    event: &'static str,
    work: impl std::future::Future<Output = Result<T, E>>,
    unavailable: E,
    classify: fn(&E) -> (String, &'static str, bool),
) -> impl std::future::Future<Output = Result<T, E>> {
    // Own the potentially large operation once. Keeping it inline in each
    // observation wrapper exhausts mobile stacks when nested futures are polled.
    let work = Box::pin(work);
    async move {
        let span = nddev_device_sync_telemetry::operation_span(module, "process")
            .map_err(|_| unavailable)?;
        observe_in_span(module, event, work, span, classify).await
    }
}

// Once resources are owned, telemetry availability must not skip their teardown.
#[cfg_attr(feature = "flutter", frb(ignore))]
pub(crate) fn observe_cleanup<T, E>(
    module: &'static str,
    event: &'static str,
    work: impl std::future::Future<Output = Result<T, E>>,
    classify: fn(&E) -> (String, &'static str, bool),
) -> impl std::future::Future<Output = Result<T, E>> {
    let work = Box::pin(work);
    async move {
        let span = nddev_device_sync_telemetry::operation_span(module, "process")
            .unwrap_or_else(|_| tracing::Span::none());
        observe_in_span(module, event, work, span, classify).await
    }
}

async fn observe_in_span<T, E>(
    module: &str,
    event: &'static str,
    work: impl std::future::Future<Output = Result<T, E>>,
    span: tracing::Span,
    classify: fn(&E) -> (String, &'static str, bool),
) -> Result<T, E> {
    let completion = span.clone();
    async {
        let start = Instant::now();
        let result = work.await;
        match &result {
            Ok(_) => tracing::event!(target:"nds_native",tracing::Level::INFO,{event.name=event,module,duration_ms=start.elapsed().as_millis() as u64,outcome="ok"}),
            Err(error) => {
                let (error_type, outcome, terminal_failure) = classify(error);
                nddev_device_sync_telemetry::finish_operation_span(&completion, terminal_failure);
                tracing::event!(target:"nds_native",tracing::Level::WARN,{event.name=event,module,duration_ms=start.elapsed().as_millis() as u64,error.type=%error_type,outcome});
            }
        }
        result
    }.instrument(span).await
}
fn data_directory() -> Result<std::path::PathBuf, VaultError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        nddev_device_sync_accounts::private_data_directory().map_err(|_| VaultError::Unavailable)
    }
    #[cfg(target_arch = "wasm32")]
    {
        Ok("/browser".into())
    }
}

#[derive(Clone)]
#[cfg_attr(
    all(target_arch = "wasm32", feature = "browser-bindings"),
    derive(tsify::Tsify, serde::Serialize)
)]
#[cfg_attr(
    all(target_arch = "wasm32", feature = "browser-bindings"),
    tsify(large_number_types_as_bigints, missing_as_null)
)]
pub struct IncidentView {
    pub id: String,
    pub service: String,
    pub kind: String,
    pub severity: String,
    pub state: String,
    pub revision: u64,
    pub occurrences: u64,
    pub first_seen: String,
    pub last_seen: String,
    pub silenced_until: String,
}
#[derive(Clone)]
#[cfg_attr(
    all(target_arch = "wasm32", feature = "browser-bindings"),
    derive(tsify::Tsify, serde::Serialize)
)]
#[cfg_attr(
    all(target_arch = "wasm32", feature = "browser-bindings"),
    tsify(large_number_types_as_bigints, missing_as_null)
)]
pub struct IncidentPage {
    pub incidents: Vec<IncidentView>,
    pub next_cursor: String,
    pub generated_at: String,
}
fn incident_view(
    value: nddev_device_sync_accounts::observability_wire::Incident,
) -> Result<IncidentView, ClientError> {
    fn tag(value: impl serde::Serialize) -> Result<String, ClientError> {
        serde_json::to_value(value)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .ok_or(ClientError::InvalidResponse)
    }
    if value.revision < 1 || value.occurrences < 1 {
        return Err(ClientError::InvalidResponse);
    }
    Ok(IncidentView {
        id: value.incident_id,
        service: value.service_name,
        kind: tag(value.kind)?,
        severity: tag(value.severity)?,
        state: tag(value.state)?,
        revision: value.revision as u64,
        occurrences: value.occurrences as u64,
        first_seen: value.first_seen,
        last_seen: value.last_seen,
        silenced_until: value.silenced_until.unwrap_or_default(),
    })
}

/// Native read preserves Android's sensitivity/URI refusal contract. Browser
/// copy support does not imply a native clipboard read adapter exists.
#[cfg_attr(feature = "flutter", frb(sync))]
pub fn supports_native_clipboard_read() -> bool {
    cfg!(target_os = "android")
}

#[cfg_attr(feature = "flutter", frb(sync))]
pub fn supports_native_clipboard() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        nddev_device_sync_clipboard::browser::supported()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "windows",
            target_os = "android"
        ))
    }
}

#[cfg_attr(feature = "flutter", frb(sync))]
pub fn new_vault_record_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(all(
    test,
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
mod activity_tests {
    use super::*;

    #[test]
    fn public_operation_futures_fit_the_mobile_stack_budget() {
        let client = NdsClient::new("https://example.invalid".into()).unwrap();
        let sizes = [
            (
                "device state",
                std::mem::size_of_val(&client.publish_device_state()),
            ),
            (
                "clipboard",
                std::mem::size_of_val(&client.clipboard_publish_text(String::new())),
            ),
            ("manual sync", std::mem::size_of_val(&client.vault_sync())),
            ("company overview", std::mem::size_of_val(&client.company())),
            (
                "company members",
                std::mem::size_of_val(&client.company_members(None)),
            ),
            (
                "company invitations",
                std::mem::size_of_val(&client.company_invitations(None)),
            ),
            (
                "company invite",
                std::mem::size_of_val(&client.company_invite(
                    String::new(),
                    nddev_device_sync_organizations::view::OrganizationRoleView::User,
                )),
            ),
            (
                "company revoke",
                std::mem::size_of_val(&client.company_revoke(String::new())),
            ),
            (
                "company role",
                std::mem::size_of_val(&client.company_change_member(String::new(), 1, None)),
            ),
            (
                "invitation import",
                std::mem::size_of_val(&client.import_invitation(String::new())),
            ),
        ];
        for (operation, bytes) in sizes {
            assert!(
                bytes <= 16 * 1024,
                "{operation} future requires {bytes} bytes"
            );
        }
    }

    #[test]
    fn blocked_state_preflight_does_not_create_a_new_server_refusal() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let client = NdsClient::new("https://example.invalid".into()).unwrap();
                let policy = client.schedule();
                policy
                    .lock()
                    .unwrap()
                    .after(BatchOutcome::CapacityExhausted);
                let check = policy.lock().unwrap().capacity_check().unwrap();
                assert!(matches!(
                    client.publish_device_state().await,
                    Err(VaultError::CapacityExhausted)
                ));
                assert!(
                    policy
                        .lock()
                        .unwrap()
                        .recovered_capacity(&check, CapacityProbeOutcome::Receipt)
                        .is_some()
                );
                assert!(client.vault.try_lock().unwrap().is_none());
            });
    }

    #[test]
    fn verified_capacity_completion_replaces_only_its_current_refusal() {
        let client = NdsClient::new("https://example.invalid".into()).unwrap();
        let old = client.schedule();
        old.lock().unwrap().after(BatchOutcome::CapacityExhausted);
        let check = old.lock().unwrap().capacity_check().unwrap();
        let replacement = {
            let _publication = client.publication.lock().unwrap();
            recovered_policy(
                &client.replica_schedule,
                &old,
                &check,
                CapacityProbeOutcome::Receipt,
            )
            .unwrap()
        };
        assert!(Arc::ptr_eq(&replacement, &client.schedule()));
        assert!(!client.capacity_blocked());
        old.lock().unwrap().after(BatchOutcome::CapacityExhausted);
        assert!(!client.capacity_blocked());
        {
            let _publication = client.publication.lock().unwrap();
            assert!(
                recovered_policy(
                    &client.replica_schedule,
                    &old,
                    &check,
                    CapacityProbeOutcome::Receipt
                )
                .is_none()
            );
        }
        replacement
            .lock()
            .unwrap()
            .after(BatchOutcome::CapacityExhausted);
        let stale = replacement.lock().unwrap().capacity_check().unwrap();
        replacement
            .lock()
            .unwrap()
            .after(BatchOutcome::CapacityExhausted);
        let _publication = client.publication.lock().unwrap();
        assert!(
            recovered_policy(
                &client.replica_schedule,
                &replacement,
                &stale,
                CapacityProbeOutcome::NoEligibleWork
            )
            .is_none()
        );
        assert!(client.capacity_blocked());
    }

    #[test]
    fn late_capacity_result_cannot_reblock_an_explicit_retry() {
        let client = NdsClient::new("https://example.invalid".into()).unwrap();
        let old = client.schedule();
        let _ = old.lock().unwrap().after(BatchOutcome::CapacityExhausted);
        let retry = {
            let _publication = client.publication.lock().unwrap();
            client.reset_replica_schedule()
        };
        let _ = retry.lock().unwrap().after(BatchOutcome::CaughtUp);

        // A state publication admitted before the retry completes afterwards.
        let _ = old.lock().unwrap().after(BatchOutcome::CapacityExhausted);
        assert!(old.lock().unwrap().is_capacity_blocked());
        assert!(!client.capacity_blocked());
        assert!(Arc::ptr_eq(&retry, &client.schedule()));

        // A real refusal from the new attempt still blocks its own policy,
        // and healthy inbox reads cannot remove it.
        let _ = retry.lock().unwrap().after(BatchOutcome::CapacityExhausted);
        let _ = retry.lock().unwrap().after(BatchOutcome::InboxHead);
        assert!(client.capacity_blocked());
    }

    #[test]
    fn obsolete_policy_cannot_publish_or_replace_a_replica() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let client = NdsClient::new("https://example.invalid".into()).unwrap();
                let obsolete = client.schedule();
                let updates = {
                    let _publication = client.publication.lock().unwrap();
                    let current = client.reset_replica_schedule();
                    assert!(!client.spawn_replica(Duration::ZERO, Weak::new(), obsolete.clone(),));
                    assert!(client.replica.lock().unwrap().is_none());
                    assert!(client.spawn_replica(Duration::from_secs(3600), Weak::new(), current,));
                    client.replica.lock().unwrap().as_ref().unwrap().updates()
                };
                {
                    let _publication = client.publication.lock().unwrap();
                    assert!(!client.spawn_replica(Duration::ZERO, Weak::new(), obsolete));
                }
                client.set_lifecycle(ClientLifecycle::Hidden);
                assert_eq!(
                    client.replica.lock().unwrap().as_ref().unwrap().is_active(),
                    client_replication_permitted(ClientLifecycle::Hidden)
                );
                assert!(!client.catalog_active());
                client.set_lifecycle(ClientLifecycle::Detached);
                assert!(!client.replica.lock().unwrap().as_ref().unwrap().is_active());
                client.wake_replica();
                assert!(!client.replica.lock().unwrap().as_ref().unwrap().is_active());
                client.set_lifecycle(ClientLifecycle::Resumed);
                assert!(client.replica.lock().unwrap().as_ref().unwrap().is_active());
                client.stop_replica().await;
                assert!(updates.has_changed().is_err());
            });
    }

    #[test]
    fn overlapping_manual_sync_does_not_stop_the_owned_replica() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let client = NdsClient::new("https://example.invalid".into()).unwrap();
                let _updates = {
                    let _publication = client.publication.lock().unwrap();
                    assert!(client.spawn_replica(
                        Duration::from_secs(3600),
                        Weak::new(),
                        client.schedule(),
                    ));
                    client.replica.lock().unwrap().as_ref().unwrap().updates()
                };
                let control = client.replica_control.lock().await;
                assert!(matches!(client.vault_sync().await, Err(VaultError::Busy)));
                assert!(client.replica.lock().unwrap().is_some());
                client.set_lifecycle(ClientLifecycle::Hidden);
                assert_eq!(
                    client.replica.lock().unwrap().as_ref().unwrap().is_active(),
                    client_replication_permitted(ClientLifecycle::Hidden)
                );
                assert!(!client.catalog_active());
                drop(control);
                assert!(client.replica_control.try_lock().is_ok());
                client.stop_replica().await;
            });
    }

    #[test]
    fn capacity_blocks_uploads_and_late_old_results_stay_in_the_old_session() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let client = NdsClient::new("https://example.invalid".into()).unwrap();
                let old = client.schedule();
                assert_eq!(
                    old.lock().unwrap().after(BatchOutcome::CapacityExhausted),
                    Some(Duration::from_secs(15))
                );
                for lifecycle in [ClientLifecycle::Hidden, ClientLifecycle::Resumed] {
                    client.set_lifecycle(lifecycle);
                    client.stop_replica().await;
                    assert!(Arc::ptr_eq(&old, &client.schedule()));
                    assert!(client.capacity_blocked());
                    // Authenticated task/inbox eligibility is covered by the owned peer fixture.
                }
                client.reset_replica_session().await;
                assert!(!Arc::ptr_eq(&old, &client.schedule()));
                let _ = old.lock().unwrap().after(BatchOutcome::CapacityExhausted);
                assert!(!client.capacity_blocked());
            });
    }

    #[test]
    fn visible_inactive_keeps_admission_and_hidden_resumed_transitions_change_it() {
        let client = NdsClient::new("https://example.invalid".into()).unwrap();
        client.set_lifecycle(ClientLifecycle::Inactive);
        assert!(client.active.load(Ordering::Relaxed));
        client.set_lifecycle(ClientLifecycle::Hidden);
        assert!(!client.active.load(Ordering::Relaxed));
        client.set_lifecycle(ClientLifecycle::Resumed);
        assert!(client.active.load(Ordering::Relaxed));
        assert!(!client_work_permitted(ClientLifecycle::Ready));
    }
}

#[cfg(test)]
mod cleanup_observation_tests {
    #[test]
    fn cleanup_executes_when_span_configuration_is_rejected() {
        use std::{
            cell::Cell,
            future::Future,
            task::{Context, Poll, Waker},
        };
        assert!(nddev_device_sync_telemetry::operation_span("invalid/module", "process").is_err());
        let called = Cell::new(false);
        let work = async {
            called.set(true);
            Ok::<(), ()>(())
        };
        let mut future = Box::pin(super::observe_cleanup(
            "invalid/module",
            "cleanup.test",
            work,
            |_| ("test_error".into(), "error", true),
        ));
        let mut context = Context::from_waker(Waker::noop());
        assert!(matches!(
            future.as_mut().poll(&mut context),
            Poll::Ready(Ok(()))
        ));
        assert!(called.get());
    }
}
