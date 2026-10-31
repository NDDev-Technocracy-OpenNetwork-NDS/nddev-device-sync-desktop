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
use nddev_device_sync_domain::Platform;
use nddev_device_sync_telemetry::view::ClientLifecycle;
#[cfg(any(
    not(any(target_os = "linux", target_os = "macos", target_os = "windows")),
    test
))]
use nddev_device_sync_vault::replication::ReplicaContext;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
use nddev_device_sync_vault::replication::batch_outcome;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use nddev_device_sync_vault::replication::native::NativeReplica;
use nddev_device_sync_vault::replication::{
    ReplicaTask as Replica, device_observations, replica_auth_error,
};
use nddev_device_sync_vault::{
    VaultClient,
    view::{RecordPage, StatePage, VaultError, VaultView},
};
#[cfg(any(
    not(any(target_os = "linux", target_os = "macos", target_os = "windows")),
    test
))]
use std::sync::Weak;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tracing::Instrument;
use zeroize::Zeroizing;

#[cfg_attr(feature = "flutter", frb(opaque))]
pub struct NdsClient {
    inner: Arc<IdentityClient>,
    // Ephemeral per-host assembly input, never persisted or replicated by Vault.
    cache_base: Option<u16>,
    pub(crate) catalog: Mutex<Option<Arc<nddev_device_sync_gds::catalog_runtime::CatalogClient>>>,
    pub(super) catalog_operations: super::catalog_operation::CatalogOperations,
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    pub(crate) catalog_owner: nddev_device_sync_vault::replication::CatalogSlot,
    pub(crate) module_control: Arc<super::module_control::ModuleControl>,
    vault: tokio::sync::Mutex<Option<Arc<VaultClient>>>,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    native_runtime: Mutex<Option<Arc<NativeReplica>>>,
    replica: Mutex<Option<Replica>>,
    replica_control: tokio::sync::Mutex<()>,
    pub(super) publication: Arc<Mutex<()>>,
    pub(super) organizations: Arc<Mutex<super::organizations::OrganizationSlot>>,
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
        Self::new_with_cache_base(endpoint, None)
    }
    /// The host assembly may supply its existing RDS-owned cache forward base.
    /// This neither discovers a port nor binds a scope or starts a forward.
    pub fn new_with_cache_base(
        endpoint: String,
        cache_base: Option<u16>,
    ) -> Result<Self, ClientError> {
        if cache_base == Some(0) {
            return Err(ClientError::InvalidInput);
        }
        #[cfg(not(target_os = "linux"))]
        if cache_base.is_some() {
            return Err(ClientError::Server("module_unsupported".into()));
        }
        Ok(Self {
            inner: Arc::new(IdentityClient::new(&endpoint)?),
            cache_base,
            catalog: Mutex::default(),
            catalog_operations: Default::default(),
            #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
            catalog_owner: Arc::default(),
            module_control: Arc::default(),
            vault: tokio::sync::Mutex::new(None),
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            native_runtime: Mutex::default(),
            replica: Mutex::default(),
            replica_control: tokio::sync::Mutex::default(),
            publication: Arc::default(),
            organizations: Arc::default(),
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
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(super) fn native_cache_base(&self) -> Option<u16> {
        self.cache_base
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub(crate) fn native_runtime(&self) -> Option<Arc<NativeReplica>> {
        self.native_runtime
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    async fn stop_native_owner(&self, owner: Arc<NativeReplica>) -> Result<(), ClientError> {
        owner.stop().await.map_err(replica_start_error)?;
        let mut slot = self
            .native_runtime
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if slot
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &owner))
        {
            slot.take();
        }
        Ok(())
    }
    pub async fn refresh(&self) -> Result<View, ClientError> {
        let generation = self.seal_identity_modules();
        self.shutdown_owned_modules(&generation).await?;
        if let Some(task) = self.cancel_work(false) {
            task.join().await;
        }
        let view = self.inner.refresh().await?;
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        self.module_control.new_identity_generation(&generation)?;
        self.open_organizations_locked();
        Ok(view)
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
                let _ = self.clear_replica_fenced(&lifetime).await;
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
                    let _ = self.clear_replica_fenced(&lifetime).await;
                    return Err(error);
                }
            }
        }
        // A known device revocation closes device capabilities while the NDS
        // session remains valid for inventory and explicit sign-out.
        if !view.authenticated || revoked {
            let _ = self.clear_replica_fenced(&lifetime).await;
        }
        Ok(self.inner.view())
    }
    pub async fn request_email(&self, email: String, locale: String) -> Result<View, ClientError> {
        self.seal_organizations();
        self.shutdown_owned_modules_current().await?;
        self.cancel_screen();
        self.invalidate_catalog();
        self.inner.request_email(email, &locale).await
    }
    pub async fn verify_email(&self, code: String) -> Result<View, ClientError> {
        self.seal_organizations();
        self.shutdown_owned_modules_current().await?;
        let view = self.inner.verify_email(code).await?;
        // Verification is an owning Accounts fact even if later local cleanup
        // fails. Do not replace local resources until the old owners drained.
        self.reset_replica_session().await?;
        Ok(view)
    }
    pub async fn start_github(&self, locale: String) -> Result<View, ClientError> {
        self.seal_organizations();
        self.shutdown_owned_modules_current().await?;
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
        // Registration adds a credential to the retained session; it does not
        // replace identity and poll does not create a new module generation.
        if !registration {
            self.seal_organizations();
            self.shutdown_owned_modules_current().await?;
        }
        self.invalidate_catalog();
        self.inner.start_passkey(registration, email, &locale).await
    }
    pub async fn passkeys(&self) -> Result<Vec<PasskeyView>, ClientError> {
        self.inner.passkeys().await
    }
    pub async fn revoke_passkey(&self, id: String) -> Result<View, ClientError> {
        let organization_generation = self.organization_generation();
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        let native_owner = self.native_runtime();
        let view = self.inner.revoke_passkey(id).await?;
        if !view.authenticated {
            self.seal_organizations_if(&organization_generation);
            if self.shutdown_owned_modules_current().await.is_err() {
                // Preserve the verified revocation result; cleanup remains HOLD
                // in ModuleControl and cannot grant a replacement capability.
                return Ok(view);
            }
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            if let Some(owner) = native_owner {
                self.stop_native_owner(owner).await?;
            }
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
        if sign_in {
            self.seal_organizations();
            self.shutdown_owned_modules_current().await?;
        }
        let view = self.inner.poll_passkey().await?;
        if sign_in && view.authenticated && view.flow.is_empty() {
            self.reset_replica_session().await?;
        }
        Ok(view)
    }
    pub async fn poll_github(&self) -> Result<View, ClientError> {
        let sign_in = self.inner.view().flow == "github";
        if sign_in {
            self.seal_organizations();
            self.shutdown_owned_modules_current().await?;
        }
        let view = self.inner.poll_github().await?;
        if sign_in && view.authenticated && view.flow.is_empty() {
            self.reset_replica_session().await?;
        }
        Ok(view)
    }
    pub async fn logout(&self) -> Result<View, ClientError> {
        let lifetime = self.inner.local_use_lifetime();
        let generation = self.seal_identity_modules();
        self.shutdown_owned_modules(&generation).await?;
        self.retire_local_resources(&generation, &lifetime, false)
            .await?;
        self.inner.check_captured_local_use_lifetime(&lifetime)?;
        let view = self.inner.logout().await?;
        let mut vault = self.vault.lock().await;
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        if self.module_control.is_generation(&generation) {
            vault.take();
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
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        if let Some(owner) = self.native_runtime() {
            owner.ensure_delivery(|identity| super::telemetry::start_delivery(identity, true));
            return;
        }
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
        let organization_generation = self.organization_generation();
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        let native_owner = self.native_runtime();
        self.cancel_screen();
        let view = self.inner.revoke_device(id).await?;
        if view
            .devices
            .iter()
            .any(|device| device.is_current && device.revoked)
        {
            self.seal_organizations_if(&organization_generation);
            if self.shutdown_owned_modules_current().await.is_err() {
                return Ok(view);
            }
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            if let Some(owner) = native_owner {
                self.stop_native_owner(owner).await?;
            }
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
    async fn retire_local_resources(
        &self,
        generation: &super::module_control::ControlGeneration,
        lifetime: &nddev_device_sync_accounts::LocalUseLifetime,
        clear_vault: bool,
    ) -> Result<(), ClientError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        let native_owner;
        let task = {
            let mut vault = self.vault.lock().await;
            let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
            self.inner.check_captured_local_use_lifetime(lifetime)?;
            if !self.module_control.is_generation(generation) {
                return Err(ClientError::Cancelled);
            }
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            {
                native_owner = self.native_runtime();
            }
            self.cancel_screen();
            self.invalidate_catalog();
            self.delivery
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            if clear_vault {
                vault.take();
            }
            self.replica
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
                .and_then(|mut replica| replica.cancel())
        };
        if let Some(task) = task {
            task.join().await;
        }
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        if let Some(owner) = native_owner {
            self.stop_native_owner(owner).await?;
        }
        Ok(())
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    fn seal_identity_modules(&self) -> super::module_control::ControlGeneration {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        self.seal_organizations_locked();
        self.invalidate_catalog();
        self.module_control.seal()
    }
    async fn shutdown_owned_modules(
        &self,
        generation: &super::module_control::ControlGeneration,
    ) -> Result<(), ClientError> {
        // All exact owners are attempted, including when one drain fails.
        // A dropped waiter leaves the registered work/join futures retained.
        let (modules, catalog, screen) = tokio::join!(
            self.module_control.shutdown_generation(generation),
            self.catalog_operations.drain(),
            self.screen_close(),
        );
        modules.and(catalog).and(screen.map_err(|error| {
            use super::screen::ScreenFailure;
            match error {
                ScreenFailure::Busy => ClientError::Busy,
                ScreenFailure::Timeout => ClientError::Timeout,
                ScreenFailure::Expired => ClientError::Expired,
                ScreenFailure::Closed => ClientError::Cancelled,
                other => ClientError::Server(other.code().into()),
            }
        }))
    }
    async fn shutdown_owned_modules_current(&self) -> Result<(), ClientError> {
        let generation = {
            let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
            self.invalidate_catalog();
            self.module_control.seal()
        };
        self.shutdown_owned_modules(&generation).await
    }
    async fn reset_replica_session(&self) -> Result<(), ClientError> {
        let lifetime = self.inner.local_use_lifetime();
        let generation = self.seal_identity_modules();
        self.shutdown_owned_modules(&generation).await?;
        self.retire_local_resources(&generation, &lifetime, true)
            .await?;
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        self.inner.check_captured_local_use_lifetime(&lifetime)?;
        self.module_control.new_identity_generation(&generation)?;
        self.open_organizations_locked();
        self.reset_replica_schedule();
        Ok(())
    }
    async fn clear_replica_fenced(
        &self,
        lifetime: &nddev_device_sync_accounts::LocalUseLifetime,
    ) -> Result<(), ClientError> {
        let generation = {
            let _vault = self.vault.lock().await;
            let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
            self.inner.check_captured_local_use_lifetime(lifetime)?;
            self.cancel_screen();
            self.invalidate_catalog();
            self.seal_organizations_locked();
            self.module_control.seal()
        };
        // Neither Vault, publication nor any module/owner mutex crosses this
        // await. The old resources remain attached on unresolved cleanup.
        self.shutdown_owned_modules(&generation).await?;
        self.retire_local_resources(&generation, lifetime, true)
            .await
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
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let _ = (initial_delay, schedule);
            let owner = NativeReplica::attach(
                &self.inner,
                data_directory().map_err(replica_start_error)?,
                Some(env!("CARGO_PKG_VERSION").into()),
            )
            .await
            .map_err(replica_start_error)?;
            self.inner.validate_local_use(fence).await?;
            owner
                .catalog(nddev_device_sync_gds::catalog_runtime::CatalogRuntime::new)
                .map_err(replica_start_error)?;
            {
                let _publication = self
                    .publication
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                self.inner.check_local_use_lifetime(fence)?;
                *self
                    .native_runtime
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(owner);
            }
            self.ensure_delivery();
            Ok(())
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let vault = self.vault().await.map_err(replica_start_error)?;
            self.catalog_runtime()
                .await
                .map_err(|_| ClientError::Busy)?;
            self.inner.validate_local_use(fence).await?;
            // Protected I/O is complete before this brief cancellation/publication boundary.
            let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
            self.inner.check_local_use_lifetime(fence)?;
            if self.spawn_replica(initial_delay, Arc::downgrade(&vault), schedule) {
                self.ensure_delivery();
            }
            Ok(())
        }
    }
    // Caller holds publication: policy replacement and task publication cannot
    // interleave after this check. The task retains this exact policy identity.
    #[cfg(any(
        not(any(target_os = "linux", target_os = "macos", target_os = "windows")),
        test
    ))]
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
                #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
                catalog: self.catalog_owner.clone(),
                #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
                catalog: Arc::default(),
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
        // Polling is one protected read. Its credential checks share this trace
        // instead of filling the export queue with unrelated standalone spans.
        let span = nddev_device_sync_telemetry::operation_span("vault", "process")
            .map_err(|_| VaultError::Unavailable)?;
        Box::pin(async {
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            let mut view = if let Some(owner) = self.native_runtime() {
                let fence = self
                    .inner
                    .local_use_fence()
                    .await
                    .map_err(replica_auth_error)?;
                let view = owner.snapshot().await?;
                self.inner
                    .validate_local_use(&fence)
                    .await
                    .map_err(replica_auth_error)?;
                view
            } else {
                self.vault().await?.refresh_local().await?
            };
            #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
            let mut view = self.vault().await?.refresh_local().await?;
            nddev_device_sync_vault::replication::project_policy(
                &mut view,
                &self
                    .schedule()
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()),
            );
            Ok(view)
        })
        .instrument(span)
        .await
    }
    pub async fn vault_sync(&self) -> Result<VaultView, VaultError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let owner = self.native_runtime().ok_or(VaultError::Busy)?;
            let fence = self
                .inner
                .local_use_fence()
                .await
                .map_err(replica_auth_error)?;
            let result = observe("vault.sync", owner.synchronize()).await;
            self.inner
                .validate_local_use(&fence)
                .await
                .map_err(replica_auth_error)?;
            result
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
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
                    let attempt = vault.retry_synchronize_attempt().await?;
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
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        if let Some(owner) = self.native_runtime() {
            owner.wake();
            return;
        }
        if self.replication_active.load(Ordering::Relaxed)
            && let Some(replica) = &*self
                .replica
                .lock()
                .unwrap_or_else(|error| error.into_inner())
        {
            replica.set_active(true);
        }
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    async fn clipboard_vault(&self) -> Result<Arc<VaultClient>, VaultError> {
        let vault = self.vault().await?;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let view = vault.snapshot();
            if self.native_runtime().is_some() && (view.catching_up || view.last_sync_at == 0) {
                // The retained desktop owner reconciles a separate VaultClient.
                // Observe its progress under this reader's own protected fence;
                // the existing follower path never takes the writer lease.
                vault.observe_replica_once().await?;
            }
        }
        Ok(vault)
    }
    pub async fn clipboard_snapshot(
        &self,
    ) -> Result<nddev_device_sync_vault::view::ClipboardView, ClientError> {
        let guard = self
            .module_access("clipboard", super::module_control::Adapter::Shared)
            .await?;
        let value = self
            .clipboard_vault()
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
                .clipboard_vault()
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
        self.publish_state_inner().await
    }
    pub async fn publish_local_state(
        &self,
        agent: &super::modules::LocalAgent,
    ) -> Result<VaultView, VaultError> {
        agent.check_client(self).map_err(|_| VaultError::Locked)?;
        self.publish_state_inner().await
    }
    async fn publish_state_inner(&self) -> Result<VaultView, VaultError> {
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
                .map_err(replica_auth_error)?;
            let modules = self
                .module_observations()
                .await
                .map_err(replica_auth_error)?;
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
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            if self.native_runtime().is_some() {
                let result = vault.stage_states(values).await;
                if result.is_ok() {
                    self.wake_replica();
                }
                return result;
            }
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
    fn schedule(&self) -> Arc<Mutex<ReplicaSchedule>> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        if let Some(owner) = self.native_runtime() {
            return owner.schedule();
        }
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
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        if let Some(owner) = self.native_runtime() {
            return owner.delivery_snapshot();
        }
        self.delivery
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .map_or_else(Default::default, |delivery| delivery.snapshot())
    }
    /// Application teardown, preserving the Accounts/native-store identity.
    /// An unresolved module or screen owner prevents resource replacement.
    pub async fn shutdown(&self) -> Result<(), ClientError> {
        let lifetime = self.inner.local_use_lifetime();
        let generation = self.seal_identity_modules();
        self.shutdown_owned_modules(&generation).await?;
        self.retire_local_resources(&generation, &lifetime, true)
            .await?;
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        self.inner.check_captured_local_use_lifetime(&lifetime)?;
        if !self.module_control.is_generation(&generation) {
            return Err(ClientError::Cancelled);
        }
        self.inner.cancel();
        Ok(())
    }
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn cancel(&self) {
        self.seal_organizations();
        self.module_control.stop();
        self.cancel_work(true);
    }
    fn cancel_work(&self, clear_sign_in: bool) -> Option<runtime::Task> {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        self.seal_organizations_locked();
        self.cancel_screen();
        self.invalidate_catalog();
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        if let Some(catalog) = self
            .catalog_owner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            catalog.cancel();
        }
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

impl Drop for NdsClient {
    fn drop(&mut self) {
        // Cancellation intent only. UI/application owners must await shutdown;
        // Drop cannot claim browser drain or erase retained unknown ACK state.
        self.module_control.stop();
        self.cancel_work(true);
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

#[cfg(test)]
mod cache_assembly_tests {
    use super::*;

    #[test]
    fn zero_cache_base_refuses_before_endpoint_construction() {
        assert!(matches!(
            NdsClient::new_with_cache_base("not-an-endpoint".into(), Some(0)),
            Err(ClientError::InvalidInput)
        ));
        assert!(matches!(
            NdsClient::new("not-an-endpoint".into()),
            Err(ClientError::InvalidEndpoint)
        ));
    }

    #[test]
    fn existing_constructor_keeps_cache_assembly_unconfigured() {
        let client = NdsClient::new("https://example.invalid".into()).unwrap();
        assert_eq!(client.native_cache_base(), None);
        let explicit =
            NdsClient::new_with_cache_base("https://example.invalid".into(), None).unwrap();
        assert_eq!(explicit.native_cache_base(), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn explicit_cache_base_is_retained_only_as_local_assembly_input() {
        let client =
            NdsClient::new_with_cache_base("https://example.invalid".into(), Some(u16::MAX))
                .unwrap();
        assert_eq!(client.native_cache_base(), Some(u16::MAX));
        assert!(client.vault.try_lock().unwrap().is_none());
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn explicit_cache_base_is_unsupported_before_endpoint_construction() {
        assert!(matches!(
            NdsClient::new_with_cache_base("not-an-endpoint".into(), Some(u16::MAX)),
            Err(ClientError::Server(code)) if code == "module_unsupported"
        ));
    }
}

#[cfg(all(
    test,
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
mod activity_tests {
    use super::*;
    use nddev_device_sync_application::replica::CapacityProbeOutcome;
    use nddev_device_sync_vault::replication::recovered_policy;

    #[tokio::test]
    async fn native_shutdown_refusal_still_joins_exact_catalog_work() {
        let client = NdsClient::new("https://example.invalid".into()).unwrap();
        let (send, receive) = tokio::sync::oneshot::channel();
        let waiter = client
            .catalog_operations
            .start(
                &client.catalog_operations.generation(),
                Arc::new(()),
                async move {
                    receive.await.unwrap();
                    Err(super::super::catalog_types::GithubCatalogError::CredentialChanged)
                },
            )
            .unwrap();
        let stale = client.seal_identity_modules();
        client
            .module_control
            .new_identity_generation(&stale)
            .unwrap();
        // Native drain refuses this old epoch immediately. Catalog work must
        // still be attempted, and dropping this waiter must not detach it.
        assert!(
            runtime::timeout(
                std::time::Duration::from_millis(5),
                client.shutdown_owned_modules(&stale),
            )
            .await
            .is_err()
        );
        send.send(()).unwrap();
        assert!(matches!(
            client.shutdown_owned_modules(&stale).await,
            Err(ClientError::Cancelled)
        ));
        assert!(matches!(
            waiter.result().await.unwrap().as_ref(),
            Err(super::super::catalog_types::GithubCatalogError::CredentialChanged)
        ));
        client.catalog_operations.drain().await.unwrap();
    }

    #[tokio::test]
    async fn module_publication_rejects_foreign_and_cancelled_agent_before_capacity_preflight() {
        let client = NdsClient::new("https://example.invalid".into()).unwrap();
        let other = NdsClient::new("https://example.invalid".into()).unwrap();
        let epoch = client.module_control.generation().unwrap();
        let agent = super::super::modules::LocalAgent::discover(
            Arc::downgrade(&client.module_control),
            client.inner.local_use_lifetime(),
            epoch,
            None,
        )
        .await
        .unwrap();
        for owner in [&client, &other] {
            owner
                .schedule()
                .lock()
                .unwrap()
                .after(BatchOutcome::CapacityExhausted);
        }
        assert!(matches!(
            client.publish_local_state(&agent).await,
            Err(VaultError::CapacityExhausted)
        ));
        assert!(matches!(
            other.publish_local_state(&agent).await,
            Err(VaultError::Locked)
        ));
        client.cancel();
        assert!(matches!(
            client.publish_local_state(&agent).await,
            Err(VaultError::Locked)
        ));
        agent.close();
    }

    impl NdsClient {
        fn capacity_blocked(&self) -> bool {
            self.schedule()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_capacity_blocked()
        }
    }

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
            (
                "organization list",
                std::mem::size_of_val(&client.organizations()),
            ),
            (
                "organization selection",
                std::mem::size_of_val(&client.select_organization(String::new(), 1)),
            ),
            (
                "organization creation",
                std::mem::size_of_val(&client.create_organization(String::new())),
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
                client.reset_replica_session().await.unwrap();
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
