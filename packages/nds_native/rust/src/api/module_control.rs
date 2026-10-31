//! Device module composition over Core policy and Vault's encrypted settings.
//! Descriptors, consent, actual adapter presence and observations stay distinct.
use super::identity::NdsClient;
#[cfg(feature = "flutter")]
use flutter_rust_bridge::frb;
use nddev_device_sync_accounts::{ClientError, IdentityClient, LocalUseFence};
use nddev_device_sync_domain::{
    ModuleDescriptor, ModuleId, ModuleKind, Permission, Platform,
    devices::{ControlMode, DevicePolicy, DevicePurpose},
    modules::{ModuleError, ModuleObservation, control::Configuration},
    state::Reachability,
};
use nddev_device_sync_vault::VaultClient;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
#[cfg_attr(
    all(target_arch = "wasm32", feature = "browser-bindings"),
    derive(tsify::Tsify, serde::Serialize)
)]
#[cfg_attr(
    all(target_arch = "wasm32", feature = "browser-bindings"),
    tsify(large_number_types_as_bigints, missing_as_null)
)]
pub struct DeviceModuleView {
    pub id: String,
    pub version: String,
    pub supported: bool,
    pub installed: Option<bool>,
    pub local_tool_supported: bool,
    pub local_tool_installed: Option<bool>,
    pub enabled: bool,
    pub permissions: Vec<String>,
    pub required_permissions: Vec<String>,
    pub optional_permissions: Vec<String>,
    pub readiness: String,
    pub checked_at_ms: Option<u64>,
    pub error_class: Option<String>,
    pub revision: u64,
    pub pending_id: Option<String>,
    pub conflicted: bool,
}

/// Authenticated device control as observed by this client (ADR 0018).
/// Presentation only: Rust admission decides with the same values.
#[derive(Clone)]
#[cfg_attr(
    all(target_arch = "wasm32", feature = "browser-bindings"),
    derive(tsify::Tsify, serde::Serialize)
)]
#[cfg_attr(
    all(target_arch = "wasm32", feature = "browser-bindings"),
    tsify(large_number_types_as_bigints, missing_as_null)
)]
pub struct DeviceControlView {
    /// `fullauto` or `manual`.
    pub mode: String,
    pub purposes: Vec<String>,
    /// Zero means no authenticated policy was observed; manual applies.
    pub revision: u64,
    /// `server`, `cached` (server unreachable) or `default` (never observed).
    pub source: String,
}

/// The policy an admission decision used. Revision zero is the unverified
/// manual default; it never widens beyond the existing module checklist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ControlBasis {
    pub(crate) mode: ControlMode,
    pub(crate) revision: u64,
}

#[derive(Default)]
#[cfg_attr(feature = "flutter", frb(ignore))]
pub(crate) struct ModuleControl {
    slots: Mutex<BTreeMap<String, Slot>>,
    /// Last authenticated policy of this identity generation. Cleared on seal.
    policy: Mutex<Option<DevicePolicy>>,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub(crate) discovery: tokio::sync::Mutex<()>,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    native: Mutex<native_ownership::Owners>,
}
#[derive(Clone)]
pub(crate) struct ControlGeneration {
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    native: Arc<native_ownership::Epoch>,
}

struct Slot {
    configuration: Configuration,
    operation_id: Option<String>,
    local_revision: u64,
    conflicted: bool,
    lifetime: Arc<CancellationToken>,
    observation: Option<ModuleObservation>,
}
impl ModuleControl {
    /// Acknowledging the same configuration does not invalidate useful work.
    /// A distinct edit or conflict cancels the affected module's lifetime.
    fn observe_configuration(
        &self,
        module: &str,
        configuration: &Configuration,
        conflicted: bool,
        operation_id: Option<&str>,
        local_revision: u64,
    ) -> (Arc<CancellationToken>, bool) {
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        let mut changed = false;
        let slot = slots.entry(module.into()).or_insert_with(|| Slot {
            configuration: configuration.clone(),
            operation_id: operation_id.map(str::to_owned),
            local_revision,
            conflicted,
            lifetime: Arc::new(CancellationToken::new()),
            observation: None,
        });
        if local_revision < slot.local_revision {
            let stale = CancellationToken::new();
            stale.cancel();
            return (Arc::new(stale), false);
        }
        slot.local_revision = local_revision;
        if slot.configuration != *configuration
            || slot.conflicted != conflicted
            || slot.operation_id.as_deref() != operation_id
        {
            slot.lifetime.cancel();
            slot.lifetime = Arc::new(CancellationToken::new());
            slot.configuration = configuration.clone();
            slot.operation_id = operation_id.map(str::to_owned);
            slot.conflicted = conflicted;
            slot.observation = None;
            changed = true;
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            for agent in self
                .native
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .owners
                .iter()
            {
                if module == "browser-use" {
                    agent.cancel_browser();
                } else {
                    agent.invalidate_module(module);
                }
            }
        }
        (slot.lifetime.clone(), changed)
    }
    /// A changed authenticated policy closes every system-tool admission
    /// lifetime, so a late result cannot acquire the replacement's authority.
    fn observe_policy(&self, policy: &DevicePolicy) {
        let mut current = self.policy.lock().unwrap_or_else(|e| e.into_inner());
        if current.as_ref() == Some(policy) {
            return;
        }
        let previous = current.replace(policy.clone());
        drop(current);
        let basis = |value: Option<&DevicePolicy>| {
            value.map_or((ControlMode::Manual, 0), |value| {
                (value.mode(), value.revision())
            })
        };
        if basis(previous.as_ref()) == basis(Some(policy)) {
            return;
        }
        let tools: Vec<String> = nddev_device_sync_agent::module_graph()
            .map(|graph| {
                graph
                    .iter()
                    .filter(|descriptor| descriptor.kind == ModuleKind::Tool)
                    .map(|descriptor| descriptor.id.as_str().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        for module in &tools {
            if let Some(slot) = slots.get_mut(module) {
                slot.lifetime.cancel();
                slot.lifetime = Arc::new(CancellationToken::new());
                slot.observation = None;
            }
        }
        drop(slots);
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        for agent in self
            .native
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .owners
            .iter()
        {
            for module in &tools {
                agent.invalidate_module(module);
            }
        }
    }
    fn cached_basis(&self) -> ControlBasis {
        self.policy
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map_or(
                ControlBasis {
                    mode: ControlMode::Manual,
                    revision: 0,
                },
                |value| ControlBasis {
                    mode: value.mode(),
                    revision: value.revision(),
                },
            )
    }
    /// Read the authenticated policy under the caller's original fence. An
    /// unreachable or older server keeps the last observed policy, or manual
    /// when none was observed; it never widens authority.
    async fn effective_policy(
        &self,
        identity: &IdentityClient,
        fence: &LocalUseFence,
    ) -> Result<(Option<DevicePolicy>, &'static str), ClientError> {
        match identity.device_policy(fence).await {
            Ok(policy) => {
                self.observe_policy(&policy);
                Ok((Some(policy), "server"))
            }
            Err(error) if policy_unreachable(&error) => {
                let cached = self
                    .policy
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let source = if cached.is_some() {
                    "cached"
                } else {
                    "default"
                };
                Ok((cached, source))
            }
            Err(error) => Err(error),
        }
    }
    pub(crate) fn stop(&self) {
        let _ = self.seal();
    }
    pub(crate) fn seal(&self) -> ControlGeneration {
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        for slot in slots.values() {
            slot.lifetime.cancel();
        }
        slots.clear();
        // A replacement identity never inherits the old device control policy.
        *self.policy.lock().unwrap_or_else(|e| e.into_inner()) = None;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let owners = self.native.lock().unwrap_or_else(|e| e.into_inner());
            owners.stop();
            ControlGeneration {
                native: owners.epoch.clone(),
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            ControlGeneration {}
        }
    }
    pub(crate) fn is_generation(&self, generation: &ControlGeneration) -> bool {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            Arc::ptr_eq(
                &self.native.lock().unwrap_or_else(|e| e.into_inner()).epoch,
                &generation.native,
            )
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = generation;
            true
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub(crate) fn attach(
        &self,
        epoch: &Arc<native_ownership::Epoch>,
        agent: Arc<nddev_device_sync_agent::NativeAgent>,
    ) -> Result<Arc<native_ownership::NativeOwner>, ClientError> {
        let mut hosts = self.native.lock().unwrap_or_else(|e| e.into_inner());
        // Closed/HOLD owners still consume capacity; they are not weak slots.
        hosts.admit(epoch)?;
        let owner = native_ownership::NativeOwner::new(agent, epoch.clone());
        hosts.owners.push(owner.clone());
        Ok(owner)
    }
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub(crate) fn generation(&self) -> Result<Arc<native_ownership::Epoch>, ClientError> {
        let hosts = self.native.lock().unwrap_or_else(|e| e.into_inner());
        if hosts.epoch.is_stopped() {
            return Err(ClientError::Busy);
        }
        Ok(hosts.epoch.clone())
    }
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub(crate) fn detach_drained(
        &self,
        owner: &Arc<native_ownership::NativeOwner>,
        drain: &Arc<native_ownership::DrainState>,
    ) -> Result<(), ClientError> {
        let mut hosts = self.native.lock().unwrap_or_else(|e| e.into_inner());
        if !Arc::ptr_eq(&hosts.epoch, &owner.epoch) || !owner.drained_by(drain) {
            return Err(ClientError::Cancelled);
        }
        hosts.owners.retain(|current| !Arc::ptr_eq(current, owner));
        Ok(())
    }
    /// Every owner starts its retained drain before this caller can suspend.
    /// Cancellation of a shutdown waiter never owns or aborts those tasks.
    #[cfg(all(
        test,
        any(target_os = "linux", target_os = "macos", target_os = "windows")
    ))]
    pub(crate) async fn shutdown(&self) -> Result<(), ClientError> {
        let generation = self.seal();
        self.shutdown_generation(&generation).await
    }
    pub(crate) async fn shutdown_generation(
        &self,
        generation: &ControlGeneration,
    ) -> Result<(), ClientError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let owners = {
                let owners = self.native.lock().unwrap_or_else(|e| e.into_inner());
                if !Arc::ptr_eq(&owners.epoch, &generation.native) || !owners.epoch.is_stopped() {
                    return Err(ClientError::Cancelled);
                }
                owners.owners.clone()
            };
            let drains: Vec<_> = owners.iter().map(|owner| owner.begin_shutdown()).collect();
            let mut failure = None;
            for (owner, drain) in owners.iter().zip(drains) {
                match drain.wait().await {
                    Ok(()) => {
                        if let Err(error) = self.detach_drained(owner, &drain) {
                            failure.get_or_insert(error);
                        }
                    }
                    Err(error) => {
                        failure.get_or_insert(error);
                    }
                }
            }
            if let Some(error) = failure {
                return Err(error);
            }
            if !self.is_generation(generation) {
                return Err(ClientError::Cancelled);
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let _ = generation;
        Ok(())
    }
    /// Called only after the old owners have drained and identity adoption has
    /// completed. A stale discovery retains its old epoch and cannot attach.
    pub(crate) fn new_identity_generation(
        &self,
        generation: &ControlGeneration,
    ) -> Result<(), ClientError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let mut hosts = self.native.lock().unwrap_or_else(|e| e.into_inner());
            if !Arc::ptr_eq(&hosts.epoch, &generation.native) || !hosts.epoch.is_stopped() {
                return Err(ClientError::Cancelled);
            }
            if !hosts.owners.is_empty() {
                return Err(ClientError::Busy);
            }
            hosts.epoch = Arc::default();
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let _ = generation;
        Ok(())
    }
}

pub(crate) enum Adapter {
    Native,
    Shared,
}

pub(crate) fn descriptor(module: &str) -> Result<ModuleDescriptor, ClientError> {
    let graph =
        nddev_device_sync_agent::module_graph().map_err(|_| ClientError::InvalidResponse)?;
    let id = ModuleId::new(module).map_err(|_| ClientError::InvalidInput)?;
    graph.get(&id).cloned().ok_or(ClientError::InvalidInput)
}
fn tag(value: impl Serialize) -> Result<String, ClientError> {
    match serde_json::to_value(value).map_err(|_| ClientError::InvalidResponse)? {
        serde_json::Value::String(value) => Ok(value),
        _ => Err(ClientError::InvalidResponse),
    }
}
pub(crate) fn vault_error(error: nddev_device_sync_vault::Error) -> ClientError {
    match error {
        nddev_device_sync_vault::Error::Busy => ClientError::Busy,
        nddev_device_sync_vault::Error::Locked => ClientError::RevalidationRequired,
        _ => ClientError::Server(error.code()),
    }
}
pub(super) fn shared_permissions(module: &str) -> Option<BTreeSet<Permission>> {
    use Permission::*;
    match module {
        "clipboard" => Some(BTreeSet::from([Clipboard, LocalNetwork])),
        "gds" => Some(nddev_device_sync_gds::catalog_runtime::required_permissions()),
        "rds" if cfg!(target_os = "linux") => Some(BTreeSet::from([ReadOnly, LocalNetwork])),
        _ => None,
    }
}
fn optional_permissions(module: &str) -> BTreeSet<Permission> {
    if module == "gds" && nddev_device_sync_gds::catalog_runtime::workspace_supported() {
        BTreeSet::from([Permission::GitWrite])
    } else {
        BTreeSet::new()
    }
}
fn capabilities(descriptor: &ModuleDescriptor) -> (bool, bool, BTreeSet<Permission>) {
    let native = super::modules::supports_local_agent()
        && platform().is_some_and(|p| descriptor.platforms.contains(&p));
    let mut required = if native {
        descriptor.permissions.clone()
    } else {
        BTreeSet::new()
    };
    let shared = shared_permissions(descriptor.id.as_str());
    if let Some(extra) = &shared {
        required.extend(extra);
    }
    (native, native || shared.is_some(), required)
}

fn edit_basis_matches(
    current: &nddev_device_sync_vault::ModuleSetting,
    revision: u64,
    pending: Option<&str>,
) -> bool {
    !current.conflicted
        && ((current.revision == revision && current.pending_id.as_deref() == pending)
            || (current.revision > revision
                && current.pending_id.is_none()
                && pending.is_some()
                && current.operation_id.as_deref() == pending))
}
fn platform() -> Option<Platform> {
    if cfg!(target_arch = "wasm32") {
        Some(Platform::Web)
    } else if cfg!(target_os = "linux") {
        Some(Platform::Linux)
    } else if cfg!(target_os = "macos") {
        Some(Platform::Macos)
    } else if cfg!(target_os = "windows") {
        Some(Platform::Windows)
    } else if cfg!(target_os = "android") {
        Some(Platform::Android)
    } else if cfg!(target_os = "ios") {
        Some(Platform::Ios)
    } else {
        None
    }
}

pub(crate) struct ModuleUseGuard {
    identity: Arc<IdentityClient>,
    vault: Arc<VaultClient>,
    control: Arc<ModuleControl>,
    module: String,
    version: String,
    configuration: Configuration,
    operation_id: Option<String>,
    pub(crate) lifetime: Arc<CancellationToken>,
    fence: Arc<LocalUseFence>,
    /// Device control used for a system-tool admission; None for modules that
    /// keep their explicit configuration.
    basis: Option<ControlBasis>,
    /// Correlates admission, action and result diagnostics; never a grant.
    pub(crate) operation: String,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    publication: Arc<Mutex<()>>,
}
impl ModuleUseGuard {
    /// The caller holds NdsClient publication after validating this original
    /// guard. Configuration replacement and delivery share this short boundary.
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(crate) fn publish<T, E: From<ClientError>>(
        &self,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let slots = self.control.slots.lock().unwrap_or_else(|e| e.into_inner());
        if self.lifetime.is_cancelled()
            || slots.get(&self.module).is_none_or(|slot| {
                !Arc::ptr_eq(&slot.lifetime, &self.lifetime)
                    || slot.conflicted
                    || slot.configuration != self.configuration
                    || slot.operation_id != self.operation_id
            })
            || self
                .basis
                .is_some_and(|basis| basis != self.control.cached_basis())
        {
            return Err(ClientError::Cancelled.into());
        }
        self.identity.check_local_use_lifetime(&self.fence)?;
        publish()
    }
    /// Only an actual adapter result in this still-current lifetime can refresh
    /// readiness. Consent and successful settings persistence are not probes.
    pub(crate) fn observe<T>(&self, result: &Result<T, ModuleError>) {
        let view = self.identity.view();
        if self.basis.is_some() && !view.current_device_id.is_empty() && !view.user_id.is_empty() {
            // The action result carries the same operation as its admission.
            let module = self.module.as_str();
            match result {
                Ok(_) => tracing::event!(target: "nds_native", tracing::Level::INFO, {
                    event.name = "module.action.completed", module,
                    device.id = %view.current_device_id, user.id = %view.user_id,
                    operation.id = %self.operation, outcome = "ok"
                }),
                Err(error) => tracing::event!(target: "nds_native", tracing::Level::WARN, {
                    event.name = "module.action.failed", module,
                    device.id = %view.current_device_id, user.id = %view.user_id,
                    operation.id = %self.operation, error.type = %error, outcome = "error"
                }),
            }
        }
        let at = (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
            .try_into()
            .ok();
        let mut slots = self.control.slots.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(slot) = slots.get_mut(&self.module)
            && Arc::ptr_eq(&slot.lifetime, &self.lifetime)
            && !self.lifetime.is_cancelled()
        {
            let value = slot.observation.get_or_insert_with(|| ModuleObservation {
                id: self.module.clone(),
                version: self.version.clone(),
                configured: true,
                reachability: Reachability::NotChecked,
                observed_at_ms: None,
                error: None,
            });
            value.observe(result, at);
        }
    }
    pub(crate) async fn validate(&self) -> Result<(), ClientError> {
        self.validate_with(true).await
    }
    async fn validate_with(&self, reread_policy: bool) -> Result<(), ClientError> {
        if self.lifetime.is_cancelled() {
            return Err(ClientError::Cancelled);
        }
        self.identity.validate_local_use(&self.fence).await?;
        if let Some(basis) = self.basis
            && !reread_policy
            && basis != self.control.cached_basis()
        {
            return Err(ClientError::Cancelled);
        }
        if let Some(basis) = self.basis.filter(|_| reread_policy) {
            // A policy replaced during the work closes this admission.
            let (policy, _) = self
                .control
                .effective_policy(&self.identity, &self.fence)
                .await?;
            if policy.map_or(basis.revision != 0, |value| {
                value.mode() != basis.mode || value.revision() != basis.revision
            }) {
                return Err(ClientError::Cancelled);
            }
        }
        let current = self
            .vault
            .module_configuration(&self.module)
            .await
            .map_err(vault_error)?;
        let (observed, _) = self.control.observe_configuration(
            &self.module,
            &current.configuration,
            current.conflicted,
            current.operation_id.as_deref(),
            current.local_revision,
        );
        if observed.is_cancelled() {
            return Err(ClientError::Busy);
        }
        if current.conflicted
            || current.configuration != self.configuration
            || current.operation_id != self.operation_id
            || self.lifetime.is_cancelled()
        {
            return Err(ClientError::Cancelled);
        }
        self.identity.validate_local_use(&self.fence).await
    }
}

impl NdsClient {
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(crate) async fn module_access(
        &self,
        module: &'static str,
        adapter: Adapter,
    ) -> Result<ModuleUseGuard, ClientError> {
        observe(
            module,
            "module.access.checked",
            self.module_access_inner(module, adapter),
        )
        .await
    }
    async fn module_access_inner(
        &self,
        module: &str,
        adapter: Adapter,
    ) -> Result<ModuleUseGuard, ClientError> {
        let descriptor = descriptor(module)?;
        let vault = self.vault().await.map_err(vault_error)?;
        let identity = self.shared_identity().clone();
        let fence = Arc::new(identity.local_use_fence().await?);
        // Read the device policy before capturing the module lifetime, so an
        // observed replacement closes earlier work rather than this admission.
        let basis = if descriptor.kind == ModuleKind::Tool {
            let (policy, _) = self
                .module_control
                .effective_policy(&identity, &fence)
                .await?;
            Some(policy.map_or(
                ControlBasis {
                    mode: ControlMode::Manual,
                    revision: 0,
                },
                |value| ControlBasis {
                    mode: value.mode(),
                    revision: value.revision(),
                },
            ))
        } else {
            None
        };
        let setting = vault
            .module_configuration(module)
            .await
            .map_err(vault_error)?;
        let (lifetime, changed) = self.module_control.observe_configuration(
            module,
            &setting.configuration,
            setting.conflicted,
            setting.operation_id.as_deref(),
            setting.local_revision,
        );
        if lifetime.is_cancelled() {
            return Err(ClientError::Busy);
        }
        if changed {
            self.module_changed(module).await;
        }
        if setting.conflicted {
            return Err(ClientError::Server("module_configuration_conflict".into()));
        }
        let (supported, installed, required) = match adapter {
            Adapter::Native => {
                let supported = platform().is_some_and(|p| descriptor.platforms.contains(&p))
                    && super::modules::supports_local_agent();
                let installed = self
                    .native_module_observations()
                    .await?
                    .iter()
                    .any(|v| v.id == module && v.configured);
                (supported, installed, descriptor.permissions)
            }
            Adapter::Shared => match shared_permissions(module) {
                Some(permissions) => (true, true, permissions),
                None => (false, false, BTreeSet::new()),
            },
        };
        let mode = basis.map_or(ControlMode::Manual, |basis| basis.mode);
        let admission =
            setting
                .configuration
                .admit_in(mode, descriptor.kind, supported, installed, &required);
        let operation = uuid::Uuid::now_v7().to_string();
        if basis.is_some() {
            admission_event(&identity, module, mode, &operation, admission.err());
        }
        if let Err(reason) = admission {
            return Err(ClientError::Server(format!("module_{}", tag(reason)?)));
        }
        let guard = ModuleUseGuard {
            identity,
            vault,
            control: self.module_control.clone(),
            module: module.into(),
            version: descriptor.version,
            configuration: setting.configuration,
            operation_id: setting.operation_id,
            lifetime,
            fence,
            basis,
            operation,
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            publication: self.publication.clone(),
        };
        guard.validate_with(false).await?;
        Ok(guard)
    }
    async fn module_changed(&self, module: &str) {
        if module == "gds" {
            self.cancel_github_catalog();
        }
        if module == "rds" {
            self.cancel_screen();
        }
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        if module == "browser-use" {
            let owners = self
                .module_control
                .native
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .owners
                .clone();
            for owner in owners {
                owner.begin_drain();
            }
        }
        #[cfg(target_os = "linux")]
        if module == "cache" {
            let owners = self
                .module_control
                .native
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .owners
                .clone();
            // Settings persistence already succeeded. Await cleanup separately;
            // its retained row/HOLD must not rewrite durable user intent.
            for owner in owners {
                let _ = owner.join_cache().await;
            }
        }
    }
    async fn native_module_observations(&self) -> Result<Vec<ModuleObservation>, ClientError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let owners = self
                .module_control
                .native
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .owners
                .clone();
            let mut values = owners
                .last()
                .map_or_else(Vec::new, |owner| owner.observations());
            // A newer view must not hide another retained owner's unresolved
            // cleanup by publishing its own successful provider observation.
            let mut blocked = None;
            for owner in owners {
                if let Some(error) = owner.cleanup_blocked()
                    && (error == ModuleError::Unavailable || blocked.is_none())
                {
                    blocked = Some(error);
                }
            }
            if let Some(error) = blocked
                && let Some(value) = values.iter_mut().find(|value| value.id == "browser-use")
            {
                value.reachability = Reachability::NotChecked;
                value.observed_at_ms = None;
                value.error = Some(error);
            }
            #[cfg(target_os = "linux")]
            {
                let cache_blocked = self
                    .module_control
                    .native
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .owners
                    .iter()
                    .filter_map(|owner| owner.cache_blocked())
                    .max_by_key(|error| u8::from(*error == ModuleError::Unavailable));
                if let Some(error) = cache_blocked
                    && let Some(value) = values.iter_mut().find(|value| value.id == "cache")
                {
                    value.reachability = Reachability::NotChecked;
                    value.observed_at_ms = None;
                    value.error = Some(error);
                }
            }
            Ok(values)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Ok(Vec::new())
        }
    }
    /// Current device control (ADR 0018) under a fresh local-use fence.
    pub async fn device_control(&self) -> Result<DeviceControlView, ClientError> {
        observe("agent", "device.control.read", async {
            let identity = self.shared_identity().clone();
            let fence = identity.local_use_fence().await?;
            let (policy, source) = self
                .module_control
                .effective_policy(&identity, &fence)
                .await?;
            identity.validate_local_use(&fence).await?;
            Ok(control_view(policy.as_ref(), source))
        })
        .await
    }
    /// Owner compare-and-set replacement. Widening manual into fullauto needs
    /// a recent sign-in (`recent_authentication_required`); a stale revision
    /// returns `operation_conflict`. Success closes system-tool admissions.
    pub async fn set_device_control(
        &self,
        expected_revision: u64,
        mode: String,
        purposes: Vec<String>,
    ) -> Result<DeviceControlView, ClientError> {
        let mode = ControlMode::parse(&mode).map_err(|_| ClientError::InvalidInput)?;
        let count = purposes.len();
        let purposes = purposes
            .iter()
            .map(|value| DevicePurpose::parse(value).map_err(|_| ClientError::InvalidInput))
            .collect::<Result<BTreeSet<_>, _>>()?;
        if purposes.is_empty() || purposes.len() != count {
            return Err(ClientError::InvalidInput);
        }
        observe("agent", "device.control.changed", async {
            let identity = self.shared_identity().clone();
            let fence = identity.local_use_fence().await?;
            let policy = identity
                .replace_device_policy(&fence, expected_revision, mode, purposes)
                .await?;
            self.module_control.observe_policy(&policy);
            identity.validate_local_use(&fence).await?;
            Ok(control_view(Some(&policy), "server"))
        })
        .await
    }
    pub async fn device_modules(&self) -> Result<Vec<DeviceModuleView>, ClientError> {
        observe("agent", "module.readiness.read", self.read_device_modules()).await
    }
    async fn read_device_modules(&self) -> Result<Vec<DeviceModuleView>, ClientError> {
        self.read_module_state().await.map(|(views, _)| views)
    }
    pub(super) async fn module_observations(&self) -> Result<Vec<ModuleObservation>, ClientError> {
        self.read_module_state().await.map(|(_, values)| values)
    }
    async fn read_module_state(
        &self,
    ) -> Result<(Vec<DeviceModuleView>, Vec<ModuleObservation>), ClientError> {
        let vault = self.vault().await.map_err(vault_error)?;
        let identity = self.shared_identity();
        let fence = identity.local_use_fence().await?;
        let native = self.native_module_observations().await?;
        let graph =
            nddev_device_sync_agent::module_graph().map_err(|_| ClientError::InvalidResponse)?;
        // Presentation uses the last authenticated policy; admission rereads it.
        let mode = self.module_control.cached_basis().mode;
        let mut result = Vec::new();
        let mut observations = Vec::new();
        for descriptor in graph.iter() {
            let id = descriptor.id.as_str();
            let setting = vault.module_configuration(id).await.map_err(vault_error)?;
            let (lifetime, changed) = self.module_control.observe_configuration(
                id,
                &setting.configuration,
                setting.conflicted,
                setting.operation_id.as_deref(),
                setting.local_revision,
            );
            if lifetime.is_cancelled() {
                return Err(ClientError::Busy);
            }
            if changed {
                self.module_changed(id).await;
            }
            let (native_supported, supported, required) = capabilities(descriptor);
            let shared = shared_permissions(id);
            let native_observation = native.iter().find(|v| v.id == id);
            let shared_observation = if id == "gds" {
                self.catalog_observation().await
            } else {
                self.module_control
                    .slots
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(id)
                    .and_then(|slot| slot.observation.clone())
            };
            let installed = if shared.is_some() {
                Some(true)
            } else if native_supported {
                native_observation.map(|v| v.configured)
            } else {
                Some(false)
            };
            // Permissions belong to the adapter actually used. A working local
            // Git worker does not require the browser catalog's credential grant.
            let admitted = [
                (
                    native_supported,
                    native_observation.is_none_or(|v| v.configured),
                    &descriptor.permissions,
                    if changed { None } else { native_observation },
                ),
                (
                    shared.is_some(),
                    true,
                    shared.as_ref().unwrap_or(&required),
                    if changed {
                        None
                    } else {
                        shared_observation.as_ref()
                    },
                ),
            ]
            .into_iter()
            .filter(|(supported, installed, required, _)| {
                setting
                    .configuration
                    .admit_in(mode, descriptor.kind, *supported, *installed, required)
                    .is_ok()
            })
            .max_by_key(|(_, _, _, value)| value.and_then(|v| v.observed_at_ms));
            let observation = admitted.and_then(|(_, _, _, value)| value);
            observations.push(observation.cloned().unwrap_or_else(|| ModuleObservation {
                id: id.into(),
                version: descriptor.version.clone(),
                configured: installed.unwrap_or(false),
                reachability: Reachability::NotChecked,
                observed_at_ms: None,
                error: None,
            }));
            let readiness = if setting.conflicted {
                "conflicted".into()
            } else {
                tag(match admitted {
                    Some((supported, installed, required, value)) => setting
                        .configuration
                        .readiness_in(mode, descriptor.kind, supported, installed, required, value),
                    None => setting.configuration.readiness_in(
                        mode,
                        descriptor.kind,
                        supported,
                        installed.unwrap_or(true),
                        &required,
                        None,
                    ),
                })?
            };
            result.push(DeviceModuleView {
                id: id.into(),
                version: descriptor.version.clone(),
                supported,
                installed,
                local_tool_supported: native_supported,
                local_tool_installed: native_observation.map(|v| v.configured),
                enabled: setting.configuration.enabled,
                permissions: setting
                    .configuration
                    .permissions
                    .into_iter()
                    .map(tag)
                    .collect::<Result<_, _>>()?,
                required_permissions: required.into_iter().map(tag).collect::<Result<_, _>>()?,
                optional_permissions: optional_permissions(id)
                    .into_iter()
                    .map(tag)
                    .collect::<Result<_, _>>()?,
                readiness,
                checked_at_ms: observation.and_then(|v| v.observed_at_ms),
                error_class: observation.and_then(|v| v.error).map(tag).transpose()?,
                revision: setting.revision,
                pending_id: setting.pending_id,
                conflicted: setting.conflicted,
            });
        }
        identity.validate_local_use(&fence).await?;
        Ok((result, observations))
    }
    pub async fn configure_module(
        &self,
        module: String,
        enabled: bool,
        permissions: Vec<String>,
        revision: u64,
        pending_id: Option<String>,
    ) -> Result<(), ClientError> {
        let definition = descriptor(&module)?;
        let telemetry_module = definition.id.as_str().to_owned();
        super::identity::observe_operation(
            &telemetry_module,
            "module.configuration.changed",
            async {
                let context = self.shared_identity().local_use_lifetime();
                let fence = self
                    .shared_identity()
                    .local_use_fence_from(&context)
                    .await?;
                let (_, supported, required) = capabilities(&definition);
                let mut allowed = required.clone();
                allowed.extend(optional_permissions(&module));
                if enabled && !supported {
                    return Err(ClientError::Server("module_unsupported".into()));
                }
                if permissions.len() > allowed.len() {
                    return Err(ClientError::InvalidInput);
                }
                let parsed = permissions
                    .iter()
                    .map(|value| {
                        serde_json::from_value(serde_json::Value::String(value.clone()))
                            .map_err(|_| ClientError::InvalidInput)
                    })
                    .collect::<Result<BTreeSet<Permission>, _>>()?;
                if parsed.len() != permissions.len() || !parsed.is_subset(&allowed) {
                    return Err(ClientError::InvalidInput);
                }
                let vault = self.vault().await.map_err(vault_error)?;
                let current = vault
                    .module_configuration(&module)
                    .await
                    .map_err(vault_error)?;
                if current.conflicted {
                    return Err(ClientError::Server("module_configuration_conflict".into()));
                }
                let configuration = Configuration {
                    enabled,
                    permissions: parsed,
                };
                if !edit_basis_matches(&current, revision, pending_id.as_deref()) {
                    return Err(ClientError::Server("module_configuration_changed".into()));
                }
                self.shared_identity().validate_local_use(&fence).await?;
                if current.configuration == configuration {
                    return Ok(());
                }

                let mutation = vault
                    .set_module_configuration_attempt(
                        module.clone(),
                        current.revision,
                        current.pending_id,
                        configuration.clone(),
                    )
                    .await
                    .map_err(vault_error)?;
                // Durability already succeeded. Invalidation must not mask it with a
                // fallible follow-up read or claim a server acknowledgement.
                if self
                    .shared_identity()
                    .check_captured_local_use_lifetime(&context)
                    .is_ok()
                {
                    let (current, _) = self.module_control.observe_configuration(
                        &module,
                        &configuration,
                        false,
                        Some(&mutation.operation_id),
                        mutation.local_revision,
                    );
                    if !current.is_cancelled() {
                        self.module_changed(&module).await;
                    }
                }
                self.wake_replica();
                Ok(())
            },
            ClientError::TelemetryUnavailable,
            |error| (error.code(), "rejected", false),
        )
        .await
    }
}

// Reuse the existing common SDK owner for terminal admission/action outcomes.
pub(crate) fn observe<T>(
    module: &'static str,
    event: &'static str,
    work: impl std::future::Future<Output = Result<T, ClientError>>,
) -> impl std::future::Future<Output = Result<T, ClientError>> {
    super::identity::observe_operation(
        module,
        event,
        work,
        ClientError::TelemetryUnavailable,
        |error| {
            let terminal = matches!(
                error,
                ClientError::Transport
                    | ClientError::Timeout
                    | ClientError::InvalidResponse
                    | ClientError::CredentialStore
                    | ClientError::TelemetryUnavailable
            ) || matches!(error, ClientError::Server(code) if matches!(code.as_str(), "provider_failed" | "invalid_response" | "vault_dependency_unavailable" | "vault_storage_full"));
            (
                error.code(),
                if terminal { "error" } else { "rejected" },
                terminal,
            )
        },
    )
}

// Concrete native lifetime ownership stays in Shared composition. Accounts owns
// scheduling, Agent owns opaque browser handles and the actual drain authority.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
/// flutter_rust_bridge:ignore
pub(crate) mod native_ownership {
    use super::*;
    #[cfg(target_os = "linux")]
    use cache_ownership::CacheState;
    #[cfg(target_os = "linux")]
    pub(crate) use cache_ownership::checked_cache;
    use nddev_device_sync_accounts::runtime::{self, Task};
    #[cfg(target_os = "linux")]
    use nddev_device_sync_agent::view::{CacheError, cache};
    use nddev_device_sync_agent::{
        NativeAgent,
        view::{BrowserError, browser},
    };
    use std::{
        future::Future,
        sync::atomic::{AtomicBool, Ordering},
    };
    use tokio::sync::watch;

    #[derive(Default)]
    pub(crate) struct Epoch(AtomicBool);
    impl Epoch {
        pub(crate) fn is_stopped(&self) -> bool {
            self.0.load(Ordering::Acquire)
        }
    }
    #[derive(Default)]
    pub(crate) struct Owners {
        pub(crate) epoch: Arc<Epoch>,
        pub(crate) owners: Vec<Arc<NativeOwner>>,
    }
    impl Owners {
        pub(super) fn admit(&self, epoch: &Arc<Epoch>) -> Result<(), ClientError> {
            if !Arc::ptr_eq(&self.epoch, epoch) || epoch.is_stopped() {
                return Err(ClientError::Cancelled);
            }
            if self.owners.len() >= 8 {
                return Err(ClientError::Busy);
            }
            Ok(())
        }
        pub(crate) fn stop(&self) {
            self.epoch.0.store(true, Ordering::Release);
            for owner in &self.owners {
                owner.close();
            }
        }
    }

    // No Debug: these values can contain private browser output and URLs.
    #[derive(Clone)]
    pub(crate) enum BrowserValue {
        Status(browser::Status),
        Sessions(Vec<browser::Session>),
        Session(browser::Session),
        Outcome(browser::Outcome),
        Closed,
    }
    pub(crate) enum BrowserFailure {
        Admission(ClientError),
        Provider(BrowserError),
        // Keep the actual provider outcome (including script failure) private
        // even when a later authorization fence prevents returning it to UI.
        Invalidated {
            _provider_outcome: Box<BrowserResult>,
            error: ClientError,
        },
        Interrupted,
    }
    impl BrowserFailure {
        pub(crate) fn client_error(&self) -> ClientError {
            match self {
                Self::Admission(error) => error.clone(),
                Self::Invalidated { error, .. } => error.clone(),
                Self::Interrupted => ClientError::Server("browser_cleanup_unresolved".into()),
                Self::Provider(BrowserError::Refused(refusal)) => {
                    ClientError::Server(format!("browser_{}", refusal.code()))
                }
                Self::Provider(BrowserError::Io(error)) => match error.code() {
                    "cancelled" => ClientError::Cancelled,
                    "timeout" => ClientError::Timeout,
                    "busy" => ClientError::Busy,
                    code => ClientError::Server(format!("browser_io_{code}")),
                },
                Self::Provider(BrowserError::InvalidResponse) => ClientError::InvalidResponse,
                Self::Provider(BrowserError::UnsupportedPlatform) => {
                    ClientError::Server("module_unsupported".into())
                }
            }
        }
        fn needs_drain(&self) -> bool {
            match self {
                Self::Interrupted => true,
                Self::Admission(_) => false,
                Self::Invalidated { .. } => true,
                Self::Provider(BrowserError::Io(_) | BrowserError::InvalidResponse) => true,
                Self::Provider(BrowserError::Refused(refusal)) => matches!(
                    refusal.code(),
                    "timeout"
                        | "extension_unavailable"
                        | "executor_unavailable"
                        | "session_not_found"
                ),
                Self::Provider(BrowserError::UnsupportedPlatform) => false,
            }
        }
    }
    pub(crate) type BrowserResult = Result<BrowserValue, BrowserFailure>;
    type SharedResult = Arc<BrowserResult>;
    type Completion = watch::Sender<Option<SharedResult>>;
    pub(crate) async fn checked<F>(guard: Arc<ModuleUseGuard>, work: F) -> BrowserResult
    where
        F: Future<Output = Result<BrowserValue, BrowserError>>,
    {
        guard.validate().await.map_err(BrowserFailure::Admission)?;
        let result = work.await.map_err(BrowserFailure::Provider);
        let validation = guard.validate().await;
        // NativeAgent's existing observe_mapped owns native observations and
        // telemetry. Shared validates publication without duplicating its
        // provider/refusal projection; Outcome.failure stays in the result.
        match validation {
            Ok(()) => result,
            Err(error) => Err(BrowserFailure::Invalidated {
                _provider_outcome: Box::new(result),
                error,
            }),
        }
    }
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Interest {
        Waiting,
        Delivered,
        Abandoned,
    }
    #[derive(Default)]
    struct BrowserLife {
        stop: CancellationToken,
    }
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Phase {
        Cold,
        Active,
        Draining,
        Drained,
        Hold,
    }
    struct BrowserState {
        life: Arc<BrowserLife>,
        phase: Phase,
        closing: bool,
        cancel_sent: bool,
        rows: Vec<Arc<RequestRow>>,
        drain: Option<Arc<DrainState>>,
    }
    // The production arm is always the same concrete NativeAgent. The inert
    // arm exists only to exercise Shared task/ownership races without discovery,
    // sockets, Chrome or a host login in unit tests.
    enum NativePort {
        Agent(Arc<NativeAgent>),
        #[cfg(test)]
        Inert(Arc<InertPort>),
    }
    impl NativePort {
        fn cancel_browser(&self) {
            match self {
                Self::Agent(agent) => agent.cancel_browser(),
                #[cfg(test)]
                Self::Inert(port) => {
                    port.cancelled.fetch_add(1, Ordering::AcqRel);
                }
            }
        }
        fn close(&self) {
            match self {
                Self::Agent(agent) => agent.close(),
                #[cfg(test)]
                Self::Inert(port) => {
                    port.cancelled.fetch_add(1, Ordering::AcqRel);
                }
            }
        }
        fn modules(&self) -> Vec<ModuleObservation> {
            match self {
                Self::Agent(agent) => agent.modules(),
                #[cfg(test)]
                Self::Inert(_) => vec![ModuleObservation {
                    id: "browser-use".into(),
                    version: "fixture".into(),
                    configured: true,
                    reachability: Reachability::NotChecked,
                    observed_at_ms: Some(1),
                    error: None,
                }],
            }
        }
        async fn drain_browser(&self) -> Result<(), ClientError> {
            match self {
                Self::Agent(agent) => agent
                    .drain_browser()
                    .await
                    .map_err(|error| BrowserFailure::Provider(error).client_error()),
                #[cfg(test)]
                Self::Inert(port) => {
                    port.drains.fetch_add(1, Ordering::AcqRel);
                    port.drain_started.cancel();
                    port.release.cancelled().await;
                    port.result.clone()
                }
            }
        }
        fn resume_browser(&self) -> Result<(), ClientError> {
            match self {
                Self::Agent(agent) => agent
                    .resume_browser()
                    .map_err(|error| BrowserFailure::Provider(error).client_error()),
                #[cfg(test)]
                Self::Inert(port) => {
                    port.resumed.fetch_add(1, Ordering::AcqRel);
                    Ok(())
                }
            }
        }
    }
    #[cfg(test)]
    struct InertPort {
        cancelled: std::sync::atomic::AtomicUsize,
        drains: std::sync::atomic::AtomicUsize,
        resumed: std::sync::atomic::AtomicUsize,
        drain_started: CancellationToken,
        release: CancellationToken,
        result: Result<(), ClientError>,
    }
    pub(crate) struct NativeOwner {
        agent: NativePort,
        pub(crate) epoch: Arc<Epoch>,
        browser: Mutex<BrowserState>,
        #[cfg(target_os = "linux")]
        cache: Mutex<CacheState>,
    }
    impl NativeOwner {
        pub(super) fn new(agent: Arc<NativeAgent>, epoch: Arc<Epoch>) -> Arc<Self> {
            Arc::new(Self {
                agent: NativePort::Agent(agent),
                epoch,
                browser: Mutex::new(BrowserState {
                    life: Arc::default(),
                    phase: Phase::Cold,
                    closing: false,
                    cancel_sent: false,
                    rows: Vec::new(),
                    drain: None,
                }),
                #[cfg(target_os = "linux")]
                cache: Mutex::default(),
            })
        }
        pub(crate) fn close(&self) {
            #[cfg(target_os = "linux")]
            self.close_cache();
            let mut state = self.browser.lock().unwrap_or_else(|e| e.into_inner());
            state.life.stop.cancel();
            if !state.closing {
                state.closing = true;
                state.cancel_sent = true;
                self.agent.close();
            }
        }
        pub(super) fn invalidate_module(&self, module: &str) {
            #[cfg(target_os = "linux")]
            if module == "cache" {
                self.seal_cache();
            }
            match &self.agent {
                NativePort::Agent(agent) => {
                    let _ = agent.invalidate_module(module);
                }
                #[cfg(test)]
                NativePort::Inert(_) => {}
            }
        }
        pub(crate) fn cancel_browser(&self) {
            let mut state = self.browser.lock().unwrap_or_else(|e| e.into_inner());
            self.cancel_lifetime(&mut state);
        }
        fn cancel_lifetime(&self, state: &mut BrowserState) {
            state.life.stop.cancel();
            // Agent cancellation invalidates its successful empty-drain flag.
            // Do it once per life, never again merely because another waiter
            // subscribes to the same already-completed drain.
            if !state.cancel_sent {
                state.cancel_sent = true;
                self.agent.cancel_browser();
            }
        }
        fn abandon(&self, life: &Arc<BrowserLife>) {
            let mut state = self.browser.lock().unwrap_or_else(|e| e.into_inner());
            if Arc::ptr_eq(&state.life, life) {
                self.cancel_lifetime(&mut state);
            }
        }
        pub(crate) fn observations(&self) -> Vec<ModuleObservation> {
            let state = self.browser.lock().unwrap_or_else(|e| e.into_inner());
            let mut values = self.agent.modules();
            if (state.phase != Phase::Active || state.life.stop.is_cancelled())
                && let Some(value) = values.iter_mut().find(|value| value.id == "browser-use")
            {
                value.reachability = Reachability::NotChecked;
                value.observed_at_ms = None;
                value.error = Some(
                    if state.phase == Phase::Hold
                        || state.drain.as_ref().is_some_and(|drain| {
                            drain.result.borrow().as_ref().is_some_and(Result::is_err)
                        })
                    {
                        ModuleError::Unavailable
                    } else {
                        ModuleError::Busy
                    },
                );
            }
            drop(state);
            #[cfg(target_os = "linux")]
            if let Some(error) = self.cache_blocked()
                && let Some(value) = values.iter_mut().find(|value| value.id == "cache")
            {
                value.reachability = Reachability::NotChecked;
                value.observed_at_ms = None;
                value.error = Some(error);
            }
            values
        }
        pub(super) fn cleanup_blocked(&self) -> Option<ModuleError> {
            let state = self.browser.lock().unwrap_or_else(|e| e.into_inner());
            if state.phase == Phase::Hold
                || state
                    .drain
                    .as_ref()
                    .is_some_and(|drain| drain.result.borrow().as_ref().is_some_and(Result::is_err))
            {
                Some(ModuleError::Unavailable)
            } else if state.phase == Phase::Draining {
                Some(ModuleError::Busy)
            } else {
                None
            }
        }
        pub(crate) fn start_request<F>(
            self: &Arc<Self>,
            mutation: bool,
            close: bool,
            work: F,
        ) -> Result<RequestWaiter, ClientError>
        where
            F: Future<Output = BrowserResult> + Send + 'static,
        {
            let mut state = self.browser.lock().unwrap_or_else(|e| e.into_inner());
            // Only genuinely terminal bookkeeping may be reclaimed. Dropping a
            // finished completion handle is not used as proof that work joined.
            if self.epoch.is_stopped()
                || state.closing
                || state.phase != Phase::Active
                || state.life.stop.is_cancelled()
            {
                return Err(ClientError::Cancelled);
            }
            state.rows.retain(|row| !row.reclaimable());
            let ordinary = state.rows.iter().filter(|row| !row.close).count();
            let closing = state.rows.iter().filter(|row| row.close).count();
            if (!close && ordinary >= 8) || (close && closing >= 8) {
                return Err(ClientError::Busy);
            }
            let row = RequestRow::new(state.life.clone(), mutation, close);
            state.rows.push(row.clone());
            let owned = self.clone();
            // Registration, both pre-spawn guards, and both handles are installed
            // before releasing this owner lock. No RPC is polled under the lock.
            row.launch(work, move |row| async move {
                let interest = row.wait_interest().await;
                let uncertain = row.joined.borrow().as_ref().is_some_and(|result| {
                    result
                        .as_ref()
                        .as_ref()
                        .err()
                        .is_some_and(BrowserFailure::needs_drain)
                });
                if row.life.stop.is_cancelled()
                    || (row.mutation && (interest == Interest::Abandoned || uncertain))
                {
                    owned.abandon(&row.life);
                    // Never await this drainer: it joins this completion task.
                    let _ = owned.begin_drain_if(&row.life);
                }
            });
            drop(state);
            row.admitted.cancel();
            Ok(RequestWaiter {
                owner: self.clone(),
                row,
                delivered: false,
            })
        }
        pub(crate) fn begin_drain(self: &Arc<Self>) -> Arc<DrainState> {
            let mut state = self.browser.lock().unwrap_or_else(|e| e.into_inner());
            let drain = self.install_drain(&mut state);
            drop(state);
            drain.admitted.cancel();
            drain
        }
        fn begin_drain_if(self: &Arc<Self>, life: &Arc<BrowserLife>) -> Option<Arc<DrainState>> {
            let mut state = self.browser.lock().unwrap_or_else(|e| e.into_inner());
            if !Arc::ptr_eq(&state.life, life) {
                return None;
            }
            let drain = self.install_drain(&mut state);
            drop(state);
            drain.admitted.cancel();
            Some(drain)
        }
        fn install_drain(self: &Arc<Self>, state: &mut BrowserState) -> Arc<DrainState> {
            // Seal FIRST: completion tasks awaiting caller interest must wake
            // before the drainer attempts to join them.
            self.cancel_lifetime(state);
            if let Some(drain) = &state.drain {
                return drain.clone();
            }
            state.phase = Phase::Draining;
            let rows = state.rows.clone();
            let drain = DrainState::new();
            state.drain = Some(drain.clone());
            let owner = self.clone();
            let captured = state.life.clone();
            let exact = drain.clone();
            drain.launch(async move {
                let mut failure = None;
                for row in &rows {
                    let task = row
                        .completion_task
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .take();
                    if let Some(task) = task {
                        task.join().await;
                    }
                    // A panicked/unpolled completion may not have taken work.
                    let work = row
                        .work_task
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .take();
                    if let Some(work) = work {
                        work.join().await;
                    }
                    if row.failed.load(Ordering::Acquire)
                        || !row.bookkeeping.load(Ordering::Acquire)
                        || row.joined.borrow().is_none()
                    {
                        failure.get_or_insert(ClientError::Server(
                            "browser_cleanup_unresolved".into(),
                        ));
                    }
                }
                // Attempt the owning drain even after task failure; it cannot
                // turn missing/panicked terminal evidence into success.
                if let Err(error) = owner.agent.drain_browser().await {
                    failure.get_or_insert(error);
                }
                let mut state = owner.browser.lock().unwrap_or_else(|e| e.into_inner());
                if !Arc::ptr_eq(&state.life, &captured)
                    || state
                        .drain
                        .as_ref()
                        .is_none_or(|value| !Arc::ptr_eq(value, &exact))
                {
                    return Err(ClientError::Cancelled);
                }
                state.phase = if failure.is_none() {
                    Phase::Drained
                } else {
                    Phase::Hold
                };
                if failure.is_none() {
                    state.rows.clear();
                }
                failure.map_or(Ok(()), Err)
            });
            drain
        }
        pub(crate) fn drained_by(&self, drain: &Arc<DrainState>) -> bool {
            #[cfg(target_os = "linux")]
            if let Some(result) = self.cache_drained_by(drain) {
                return result;
            }
            self.browser_drained_by(drain)
        }
        fn browser_drained_by(&self, drain: &Arc<DrainState>) -> bool {
            let state = self.browser.lock().unwrap_or_else(|e| e.into_inner());
            state.phase == Phase::Drained
                && state.rows.is_empty()
                && state
                    .drain
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, drain))
                && drain.result.borrow().as_ref().is_some_and(Result::is_ok)
        }
        pub(crate) fn resume(&self, guard: &ModuleUseGuard) -> Result<(), ClientError> {
            // Identity/Vault validation has already awaited outside all locks.
            // This final slot/lifetime CAS is synchronous with invalidation.
            let _publication = guard.publication.lock().unwrap_or_else(|e| e.into_inner());
            let slots = guard
                .control
                .slots
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            guard.identity.check_local_use_lifetime(&guard.fence)?;
            let current = slots.get("browser-use").is_some_and(|slot| {
                Arc::ptr_eq(&slot.lifetime, &guard.lifetime) && !slot.lifetime.is_cancelled()
            });
            let mut state = self.browser.lock().unwrap_or_else(|e| e.into_inner());
            if !current
                || self.epoch.is_stopped()
                || state.closing
                || state.phase != Phase::Drained
                || !state.rows.is_empty()
                || state
                    .drain
                    .as_ref()
                    .is_none_or(|drain| !drain.result.borrow().as_ref().is_some_and(Result::is_ok))
            {
                return Err(ClientError::Cancelled);
            }
            self.agent.resume_browser()?;
            state.life = Arc::default();
            state.cancel_sent = false;
            state.drain = None;
            state.phase = Phase::Active;
            Ok(())
        }
        pub(crate) fn can_resume(&self) -> bool {
            let state = self.browser.lock().unwrap_or_else(|e| e.into_inner());
            !state.closing && (state.phase != Phase::Active || state.life.stop.is_cancelled())
        }
        #[cfg(not(target_os = "linux"))]
        pub(crate) fn begin_shutdown(self: &Arc<Self>) -> Arc<DrainState> {
            self.close();
            self.begin_drain()
        }
    }

    #[cfg(target_os = "linux")]
    mod cache_ownership {
        use super::*;

        // Private provider evidence, never a DTO/log field or a new authority.
        pub(crate) enum CacheFailure {
            Admission(ClientError),
            Provider(CacheError),
            Invalidated {
                effect: Box<CacheResult>,
                error: ClientError,
            },
            Interrupted,
        }
        pub(crate) type CacheResult = Result<cache::Status, CacheFailure>;
        type ResultCell = watch::Sender<Option<Arc<CacheResult>>>;
        impl CacheFailure {
            fn client_error(&self) -> ClientError {
                match self {
                    Self::Admission(error) | Self::Invalidated { error, .. } => error.clone(),
                    Self::Provider(error) => super::super::super::modules::cache_error(error),
                    Self::Interrupted => unresolved(),
                }
            }
            fn unresolved(&self) -> bool {
                match self {
                    Self::Interrupted => true,
                    Self::Provider(CacheError::Io(error)) => error.code() == "transport",
                    Self::Invalidated { effect, .. } => {
                        effect.as_ref().as_ref().err().is_some_and(Self::unresolved)
                    }
                    _ => false,
                }
            }
        }
        fn unresolved() -> ClientError {
            ClientError::Server("cache_cleanup_unresolved".into())
        }
        pub(crate) async fn checked_cache<F>(guard: Arc<ModuleUseGuard>, work: F) -> CacheResult
        where
            F: Future<Output = Result<cache::Status, CacheError>>,
        {
            guard.validate().await.map_err(CacheFailure::Admission)?;
            let effect = work.await.map_err(CacheFailure::Provider);
            match guard.validate().await {
                Ok(()) => effect,
                Err(error) => Err(CacheFailure::Invalidated {
                    effect: Box::new(effect),
                    error,
                }),
            }
        }

        #[derive(Default)]
        pub(super) struct CacheState {
            generation: Arc<()>,
            lifetime: Option<Arc<CancellationToken>>,
            sealed: bool,
            closing: bool,
            row: Option<Arc<CacheRow>>,
            shutdown: Option<Arc<DrainState>>,
        }
        pub(crate) struct CacheSnapshot(Arc<()>);
        impl NativeOwner {
            pub(super) fn close_cache(&self) {
                let mut state = self.cache.lock().unwrap_or_else(|e| e.into_inner());
                state.closing = true;
                state.sealed = true;
                state.generation = Arc::new(());
            }
            pub(super) fn seal_cache(&self) {
                let mut state = self.cache.lock().unwrap_or_else(|e| e.into_inner());
                state.sealed = true;
                state.generation = Arc::new(());
            }
            fn with_cache_guard<T>(
                &self,
                guard: &ModuleUseGuard,
                apply: impl FnOnce(&mut CacheState) -> Result<T, ClientError>,
            ) -> Result<T, ClientError> {
                let _publication = guard.publication.lock().unwrap_or_else(|e| e.into_inner());
                let slots = guard
                    .control
                    .slots
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                guard.identity.check_local_use_lifetime(&guard.fence)?;
                let current = slots.get("cache").is_some_and(|slot| {
                    Arc::ptr_eq(&slot.lifetime, &guard.lifetime)
                        && !slot.lifetime.is_cancelled()
                        && !slot.conflicted
                        && slot.configuration == guard.configuration
                        && slot.operation_id == guard.operation_id
                });
                if !current || self.epoch.is_stopped() {
                    return Err(ClientError::Cancelled);
                }
                let mut state = self.cache.lock().unwrap_or_else(|e| e.into_inner());
                if state.closing {
                    return Err(ClientError::Cancelled);
                }
                apply(&mut state)
            }
            fn prepare_cache(
                state: &mut CacheState,
                lifetime: &Arc<CancellationToken>,
            ) -> Result<(), ClientError> {
                if state.closing {
                    return Err(ClientError::Cancelled);
                }
                if state.sealed
                    && state
                        .lifetime
                        .as_ref()
                        .is_some_and(|old| Arc::ptr_eq(old, lifetime))
                {
                    return Err(ClientError::Cancelled);
                }
                if let Some(row) = &state.row {
                    if row.unresolved() {
                        return Err(unresolved());
                    }
                    if !row.reclaimable() {
                        return Err(ClientError::Busy);
                    }
                    state.row = None;
                }
                if state
                    .lifetime
                    .as_ref()
                    .is_none_or(|old| !Arc::ptr_eq(old, lifetime))
                {
                    state.generation = Arc::new(());
                    state.lifetime = Some(lifetime.clone());
                }
                state.sealed = false;
                Ok(())
            }
            fn snapshot_cache_state(
                state: &mut CacheState,
                lifetime: &Arc<CancellationToken>,
            ) -> Result<CacheSnapshot, ClientError> {
                Self::prepare_cache(state, lifetime)?;
                Ok(CacheSnapshot(state.generation.clone()))
            }
            fn publish_cache_state<T>(
                state: &CacheState,
                snapshot: &CacheSnapshot,
                publish: impl FnOnce() -> Result<T, ClientError>,
            ) -> Result<T, ClientError> {
                if state.closing
                    || state.sealed
                    || !Arc::ptr_eq(&state.generation, &snapshot.0)
                    || state.row.is_some()
                {
                    return Err(ClientError::Busy);
                }
                publish()
            }
            pub(crate) fn cache_snapshot(
                &self,
                guard: &ModuleUseGuard,
            ) -> Result<CacheSnapshot, ClientError> {
                self.with_cache_guard(guard, |state| {
                    Self::snapshot_cache_state(state, &guard.lifetime)
                })
            }
            pub(crate) fn publish_cache<T>(
                &self,
                guard: &ModuleUseGuard,
                snapshot: &CacheSnapshot,
                publish: impl FnOnce() -> Result<T, ClientError>,
            ) -> Result<T, ClientError> {
                self.with_cache_guard(guard, |state| {
                    Self::publish_cache_state(state, snapshot, publish)
                })
            }
            pub(crate) fn start_cache_request<F>(
                self: &Arc<Self>,
                guard: &ModuleUseGuard,
                work: F,
            ) -> Result<CacheWaiter, ClientError>
            where
                F: Future<Output = CacheResult> + Send + 'static,
            {
                let waiter = self.with_cache_guard(guard, |state| {
                    Self::prepare_cache(state, &guard.lifetime)?;
                    Ok(self.install_cache_row(state, work))
                })?;
                waiter.row.admitted.cancel();
                Ok(waiter)
            }
            fn install_cache_row<F>(
                self: &Arc<Self>,
                state: &mut CacheState,
                work: F,
            ) -> CacheWaiter
            where
                F: Future<Output = CacheResult> + Send + 'static,
            {
                state.generation = Arc::new(());
                let row = CacheRow::new(state.generation.clone());
                state.row = Some(row.clone());
                // Both guards and task handles are registered under the short
                // owner lock before admission. Neither task is polled here.
                row.launch(self.clone(), work);
                CacheWaiter {
                    owner: self.clone(),
                    row,
                    delivered: false,
                }
            }
            pub(crate) async fn join_cache(&self) -> Result<(), ClientError> {
                let row = self
                    .cache
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .row
                    .clone();
                if let Some(row) = row {
                    row.joined_result().await;
                    if row.unresolved() {
                        return Err(unresolved());
                    }
                }
                Ok(())
            }
            pub(crate) fn cache_blocked(&self) -> Option<ModuleError> {
                let state = self.cache.lock().unwrap_or_else(|e| e.into_inner());
                if state.row.as_ref().is_some_and(|row| row.unresolved())
                    || state.shutdown.as_ref().is_some_and(|drain| {
                        drain.result.borrow().as_ref().is_some_and(Result::is_err)
                    })
                {
                    Some(ModuleError::Unavailable)
                } else if state.sealed
                    || state.closing
                    || state
                        .row
                        .as_ref()
                        .is_some_and(|row| row.joined.borrow().is_none())
                {
                    Some(ModuleError::Busy)
                } else {
                    None
                }
            }
            pub(crate) fn begin_shutdown(self: &Arc<Self>) -> Arc<DrainState> {
                self.close();
                let mut state = self.cache.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(drain) = &state.shutdown {
                    return drain.clone();
                }
                let row = state.row.clone();
                let drain = DrainState::new();
                state.shutdown = Some(drain.clone());
                let owner = self.clone();
                let exact = drain.clone();
                drain.launch(async move {
                    let mut failure = None;
                    // Start the existing retained browser drain before waiting
                    // on cache; neither failure may skip the other cleanup.
                    let browser = owner.begin_drain();
                    if let Some(row) = &row
                        && let Err(error) = row.join_shutdown().await
                    {
                        failure.get_or_insert(error);
                    }
                    if let Err(error) = browser.wait().await {
                        failure.get_or_insert(error);
                    }
                    let mut state = owner.cache.lock().unwrap_or_else(|e| e.into_inner());
                    if state
                        .shutdown
                        .as_ref()
                        .is_none_or(|value| !Arc::ptr_eq(value, &exact))
                    {
                        return Err(ClientError::Cancelled);
                    }
                    if failure.is_none() {
                        state.row = None;
                    }
                    failure.map_or(Ok(()), Err)
                });
                drop(state);
                drain.admitted.cancel();
                drain
            }
            pub(super) fn cache_drained_by(&self, drain: &Arc<DrainState>) -> Option<bool> {
                let state = self.cache.lock().unwrap_or_else(|e| e.into_inner());
                let Some(full) = &state.shutdown else {
                    return state.row.is_some().then_some(false);
                };
                let cache_drained = Arc::ptr_eq(full, drain)
                    && state.closing
                    && state.row.is_none()
                    && drain.result.borrow().as_ref().is_some_and(Result::is_ok);
                drop(state);
                let browser = self
                    .browser
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .drain
                    .clone();
                Some(
                    cache_drained
                        && browser
                            .as_ref()
                            .is_some_and(|browser| self.browser_drained_by(browser)),
                )
            }
        }

        struct CacheRow {
            generation: Arc<()>,
            work: ResultCell,
            joined: ResultCell,
            interest: watch::Sender<Interest>,
            work_task: Mutex<Option<Task>>,
            completion_task: Mutex<Option<Task>>,
            admitted: CancellationToken,
            failed: AtomicBool,
            bookkeeping: AtomicBool,
        }
        impl CacheRow {
            fn new(generation: Arc<()>) -> Arc<Self> {
                Arc::new(Self {
                    generation,
                    work: watch::channel(None).0,
                    joined: watch::channel(None).0,
                    interest: watch::channel(Interest::Waiting).0,
                    work_task: Mutex::default(),
                    completion_task: Mutex::default(),
                    admitted: CancellationToken::new(),
                    failed: AtomicBool::new(false),
                    bookkeeping: AtomicBool::new(false),
                })
            }
            fn launch<F>(self: &Arc<Self>, owner: Arc<NativeOwner>, work: F)
            where
                F: Future<Output = CacheResult> + Send + 'static,
            {
                let terminal = CacheWorkTerminal(self.clone());
                let joined = CacheJoinTerminal {
                    row: self.clone(),
                    finished: false,
                };
                let admitted = self.clone();
                *self.work_task.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(runtime::spawn(async move {
                        admitted.admitted.cancelled().await;
                        let result = if admitted.failed.load(Ordering::Acquire) {
                            Err(CacheFailure::Interrupted)
                        } else {
                            work.await
                        };
                        terminal.finish(result);
                    }));
                let row = self.clone();
                *self
                    .completion_task
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some(runtime::spawn(async move {
                    // Keep the exact owner, not only its I/O future, through
                    // actual join and terminal publication after caller loss.
                    let _owner = owner;
                    row.admitted.cancelled().await;
                    let work = row
                        .work_task
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .take();
                    if let Some(work) = work {
                        work.join().await;
                    } else {
                        row.failed.store(true, Ordering::Release);
                    }
                    let result = row.work.borrow().clone().unwrap_or_else(|| {
                        row.failed.store(true, Ordering::Release);
                        Arc::new(Err(CacheFailure::Interrupted))
                    });
                    joined.finish(result);
                }));
            }
            async fn joined_result(&self) -> Arc<CacheResult> {
                let mut result = self.joined.subscribe();
                loop {
                    if let Some(result) = result.borrow_and_update().clone() {
                        return result;
                    }
                    if result.changed().await.is_err() {
                        self.failed.store(true, Ordering::Release);
                        return Arc::new(Err(CacheFailure::Interrupted));
                    }
                }
            }
            fn unresolved(&self) -> bool {
                self.failed.load(Ordering::Acquire)
                    || self.joined.borrow().as_ref().is_some_and(|result| {
                        result
                            .as_ref()
                            .as_ref()
                            .err()
                            .is_some_and(CacheFailure::unresolved)
                    })
            }
            fn reclaimable(&self) -> bool {
                self.bookkeeping.load(Ordering::Acquire)
                    && !self.unresolved()
                    && *self.interest.borrow() != Interest::Waiting
                    && self
                        .completion_task
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .as_ref()
                        .is_some_and(Task::is_finished)
            }
            async fn join_shutdown(&self) -> Result<(), ClientError> {
                let completion = self
                    .completion_task
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take();
                if let Some(task) = completion {
                    task.join().await;
                }
                let work = self
                    .work_task
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take();
                if let Some(task) = work {
                    task.join().await;
                }
                if self.unresolved()
                    || !self.bookkeeping.load(Ordering::Acquire)
                    || self.joined.borrow().is_none()
                {
                    Err(unresolved())
                } else {
                    Ok(())
                }
            }
        }
        struct CacheWorkTerminal(Arc<CacheRow>);
        impl CacheWorkTerminal {
            fn finish(self, result: CacheResult) {
                self.0.work.send_replace(Some(Arc::new(result)));
            }
        }
        impl Drop for CacheWorkTerminal {
            fn drop(&mut self) {
                if self.0.work.borrow().is_none() {
                    self.0.failed.store(true, Ordering::Release);
                    self.0.admitted.cancel();
                    self.0
                        .work
                        .send_replace(Some(Arc::new(Err(CacheFailure::Interrupted))));
                }
            }
        }
        struct CacheJoinTerminal {
            row: Arc<CacheRow>,
            finished: bool,
        }
        impl CacheJoinTerminal {
            fn finish(mut self, result: Arc<CacheResult>) {
                self.row.bookkeeping.store(true, Ordering::Release);
                self.row.joined.send_replace(Some(result));
                self.finished = true;
            }
        }
        impl Drop for CacheJoinTerminal {
            fn drop(&mut self) {
                if !self.finished {
                    self.row.failed.store(true, Ordering::Release);
                    self.row.admitted.cancel();
                    // Preserve a provider effect already recorded; panic is
                    // additional cleanup failure, never an erased result.
                    if self.row.joined.borrow().is_none() {
                        self.row
                            .joined
                            .send_replace(Some(Arc::new(Err(CacheFailure::Interrupted))));
                    }
                }
            }
        }
        pub(crate) struct CacheWaiter {
            owner: Arc<NativeOwner>,
            row: Arc<CacheRow>,
            delivered: bool,
        }
        impl CacheWaiter {
            pub(crate) async fn wait<P>(
                self,
                guard: Arc<ModuleUseGuard>,
                caller_publication: P,
            ) -> Result<cache::Status, ClientError>
            where
                P: FnOnce() -> Result<(), ClientError>,
            {
                let captured = guard.clone();
                self.wait_validated(guard.validate(), move |waiter| {
                    waiter.owner.with_cache_guard(&captured, |state| {
                        waiter.publish_caller(state, caller_publication)
                    })
                })
                .await
            }
            fn publish_caller(
                &self,
                state: &CacheState,
                caller: impl FnOnce() -> Result<(), ClientError>,
            ) -> Result<(), ClientError> {
                self.check_publication(state)?;
                caller()
            }
            fn check_publication(&self, state: &CacheState) -> Result<(), ClientError> {
                if self.owner.epoch.is_stopped()
                    || state.closing
                    || state.sealed
                    || !Arc::ptr_eq(&state.generation, &self.row.generation)
                    || !state
                        .row
                        .as_ref()
                        .is_some_and(|row| Arc::ptr_eq(row, &self.row))
                {
                    return Err(ClientError::Cancelled);
                }
                Ok(())
            }
            #[cfg(test)]
            async fn wait_inert(self) -> Result<cache::Status, ClientError> {
                // Only ownership/publication is exercised. Identity/Vault
                // admission stays in the existing real owning integration gate.
                self.wait_validated(async { Ok(()) }, |waiter| {
                    let state = waiter.owner.cache.lock().unwrap();
                    waiter.check_publication(&state)
                })
                .await
            }
            async fn wait_validated<V, P>(
                mut self,
                validate: V,
                publish: P,
            ) -> Result<cache::Status, ClientError>
            where
                V: Future<Output = Result<(), ClientError>>,
                P: FnOnce(&Self) -> Result<(), ClientError>,
            {
                let result = self.row.joined_result().await;
                validate.await?;
                publish(&self)?;
                // Interest/Drop lives through validation; no await after the
                // final original-fence publication check. Never replay work.
                self.delivered = true;
                self.row.interest.send_replace(Interest::Delivered);
                match result.as_ref() {
                    Ok(status) => Ok(status.clone()),
                    Err(error) => Err(error.client_error()),
                }
            }
        }
        impl Drop for CacheWaiter {
            fn drop(&mut self) {
                if !self.delivered {
                    self.row.interest.send_replace(Interest::Abandoned);
                }
                // Cache-only caller loss NEVER cancels any browser lifetime.
            }
        }

        #[cfg(test)]
        mod tests {
            use super::super::ownership_tests::{owner, run};
            use super::*;
            use tokio::sync::oneshot;

            fn status() -> cache::Status {
                // Opaque inert result, never an actual port/route/readiness proof.
                cache::Status {
                    port: 1,
                    ready: false,
                    bindings: Vec::new(),
                }
            }
            fn start<F>(
                owner: &Arc<NativeOwner>,
                lifetime: Arc<CancellationToken>,
                work: F,
            ) -> Result<CacheWaiter, ClientError>
            where
                F: Future<Output = CacheResult> + Send + 'static,
            {
                let waiter = {
                    let mut state = owner.cache.lock().unwrap();
                    // Reuse production row admission/registration. Real
                    // identity/Vault checks are outside this inert fixture.
                    NativeOwner::prepare_cache(&mut state, &lifetime)?;
                    owner.install_cache_row(&mut state, work)
                };
                waiter.row.admitted.cancel();
                Ok(waiter)
            }

            #[test]
            fn dropped_cache_waiter_retains_work_and_revoke_never_cancels_browser() {
                run(async {
                    let (owner, port) = owner(Arc::default(), false, Ok(()));
                    let (entered, receiving) = oneshot::channel();
                    let (release, held) = oneshot::channel();
                    let waiter = start(&owner, Arc::default(), async move {
                        entered.send(()).unwrap();
                        held.await.unwrap();
                        Ok(status())
                    })
                    .unwrap();
                    let row = waiter.row.clone();
                    receiving.await.unwrap();
                    drop(waiter);
                    owner.seal_cache();
                    let competing = start(&owner, Arc::default(), async { Ok(status()) }).err();
                    let before = (
                        port.cancelled.load(Ordering::Acquire),
                        port.drains.load(Ordering::Acquire),
                    );
                    let pending = row.joined.borrow().is_none();
                    release.send(()).unwrap();
                    let joined = owner.join_cache().await;
                    let work_known = matches!(row.joined.borrow().as_deref(), Some(Ok(_)));
                    // A new explicitly validated configuration lifetime may
                    // attempt again only after the prior row actually joined.
                    let next = start(&owner, Arc::default(), async { Ok(status()) }).unwrap();
                    let next_result = next.wait_inert().await;
                    let drain = owner.begin_shutdown();
                    let shutdown = drain.wait().await;
                    assert_eq!(competing, Some(ClientError::Busy));
                    assert_eq!(before, (0, 0));
                    assert!(pending && work_known);
                    assert_eq!(joined, Ok(()));
                    assert!(next_result.is_ok());
                    assert_eq!(shutdown, Ok(()));
                    assert!(row.bookkeeping.load(Ordering::Acquire));
                    assert!(owner.drained_by(&drain));
                });
            }

            #[test]
            fn joined_uncertain_is_not_a_missing_or_panicked_terminal() {
                run(async {
                    for kind in 0..4 {
                        let (owner, port) = owner(Arc::default(), false, Ok(()));
                        let waiter = start(&owner, Arc::default(), async move {
                            match kind {
                                0 => Err(CacheFailure::Provider(CacheError::Uncertain)),
                                // Existing RDS re-exports the SAME singleton F0
                                // IoError. No new dep/provider is introduced.
                                1 => Err(CacheFailure::Provider(CacheError::Io(
                                    nddev_device_sync_rds::IoError::Transport,
                                ))),
                                3 => Err(CacheFailure::Invalidated {
                                    effect: Box::new(Err(CacheFailure::Provider(
                                        CacheError::Uncertain,
                                    ))),
                                    error: ClientError::Cancelled,
                                }),
                                _ => panic!("inert cache task failure"),
                            }
                        })
                        .unwrap();
                        let row = waiter.row.clone();
                        let delivered = waiter.wait_inert().await;
                        let drain = owner.begin_shutdown();
                        let shutdown = drain.wait().await;
                        assert!(delivered.is_err());
                        assert_eq!(shutdown.is_ok(), kind == 0 || kind == 3);
                        assert_eq!(owner.drained_by(&drain), kind == 0 || kind == 3);
                        assert_eq!(
                            port.drains.load(Ordering::Acquire),
                            1,
                            "browser drain attempted after cache failure"
                        );
                        let effect = row.joined.borrow();
                        match effect.as_deref() {
                            Some(Err(CacheFailure::Provider(CacheError::Uncertain))) => {
                                assert_eq!(kind, 0)
                            }
                            Some(Err(CacheFailure::Provider(CacheError::Io(error)))) => {
                                assert_eq!(kind, 1);
                                assert_eq!(error.code(), "transport");
                            }
                            Some(Err(CacheFailure::Interrupted)) => assert_eq!(kind, 2),
                            Some(Err(CacheFailure::Invalidated { effect, error })) => {
                                assert_eq!(kind, 3);
                                assert_eq!(*error, ClientError::Cancelled);
                                assert!(matches!(
                                    effect.as_ref(),
                                    Err(CacheFailure::Provider(CacheError::Uncertain))
                                ));
                            }
                            _ => panic!("typed effect was lost"),
                        }
                    }
                });
            }

            #[test]
            fn stale_cache_delivery_is_denied_without_erasing_joined_result() {
                run(async {
                    let (owner, port) = owner(Arc::default(), false, Ok(()));
                    let waiter = start(&owner, Arc::default(), async { Ok(status()) }).unwrap();
                    let row = waiter.row.clone();
                    row.joined_result().await;
                    let (entered, receiving) = oneshot::channel();
                    let (release, held) = oneshot::channel();
                    let caller = tokio::spawn(async move {
                        waiter
                            .wait_validated(
                                async move {
                                    entered.send(()).unwrap();
                                    held.await.unwrap();
                                    Ok(())
                                },
                                |waiter| {
                                    waiter.check_publication(&waiter.owner.cache.lock().unwrap())
                                },
                            )
                            .await
                    });
                    receiving.await.unwrap();
                    owner.seal_cache();
                    release.send(()).unwrap();
                    let delivered = caller.await.unwrap();
                    let before = port.cancelled.load(Ordering::Acquire);
                    owner.begin_shutdown().wait().await.unwrap();
                    assert_eq!(delivered, Err(ClientError::Cancelled));
                    assert_eq!(before, 0);
                    assert!(matches!(row.joined.borrow().as_deref(), Some(Ok(_))));
                });
            }

            #[test]
            fn abandoned_final_cache_validation_never_cancels_browser() {
                run(async {
                    let (owner, port) = owner(Arc::default(), false, Ok(()));
                    let waiter = start(&owner, Arc::default(), async { Ok(status()) }).unwrap();
                    let row = waiter.row.clone();
                    row.joined_result().await;
                    let (entered, receiving) = oneshot::channel();
                    let caller = tokio::spawn(async move {
                        waiter
                            .wait_validated(
                                async move {
                                    entered.send(()).unwrap();
                                    std::future::pending::<()>().await;
                                    Ok(())
                                },
                                |_| Ok(()),
                            )
                            .await
                    });
                    receiving.await.unwrap();
                    caller.abort();
                    let _ = caller.await;
                    let before = port.cancelled.load(Ordering::Acquire);
                    let interest = *row.interest.borrow();
                    owner.begin_shutdown().wait().await.unwrap();
                    assert_eq!(before, 0);
                    assert!(interest == Interest::Abandoned);
                    assert!(matches!(row.joined.borrow().as_deref(), Some(Ok(_))));
                });
            }

            #[test]
            fn held_captured_caller_denial_preserves_effect_and_abandons_delivery() {
                run(async {
                    let (owner, port) = owner(Arc::default(), false, Ok(()));
                    let waiter = start(&owner, Arc::default(), async {
                        Err(CacheFailure::Provider(CacheError::Uncertain))
                    })
                    .unwrap();
                    let row = waiter.row.clone();
                    row.joined_result().await;
                    let (entered, receiving) = oneshot::channel();
                    let (release, held) = oneshot::channel();
                    let checked = Arc::new(AtomicBool::new(false));
                    let observed = checked.clone();
                    let caller = tokio::spawn(async move {
                        waiter
                            .wait_validated(
                                async move {
                                    entered.send(()).unwrap();
                                    held.await.unwrap();
                                    Ok(())
                                },
                                |waiter| {
                                    let state = waiter.owner.cache.lock().unwrap();
                                    // Invoke the production publication seam after the
                                    // held validation; this is no real identity fixture.
                                    waiter.publish_caller(&state, || {
                                        observed.store(true, Ordering::Release);
                                        Err(ClientError::RevalidationRequired)
                                    })
                                },
                            )
                            .await
                    });
                    receiving.await.unwrap();
                    release.send(()).unwrap();
                    let delivered = caller.await.unwrap();
                    let interest = *row.interest.borrow();
                    let before = port.cancelled.load(Ordering::Acquire);
                    let joined = owner.begin_shutdown().wait().await;
                    assert_eq!(delivered, Err(ClientError::RevalidationRequired));
                    assert!(checked.load(Ordering::Acquire));
                    assert!(interest == Interest::Abandoned);
                    assert_eq!(before, 0);
                    assert_eq!(joined, Ok(()));
                    assert!(matches!(
                        row.joined.borrow().as_deref(),
                        Some(Err(CacheFailure::Provider(CacheError::Uncertain)))
                    ));
                });
            }

            #[test]
            fn status_generation_survives_a_later_validation_await() {
                run(async {
                    let (owner, _) = owner(Arc::default(), false, Ok(()));
                    let life = Arc::default();
                    let snapshot =
                        NativeOwner::snapshot_cache_state(&mut owner.cache.lock().unwrap(), &life)
                            .unwrap();
                    let old_generation = snapshot.0.clone();
                    let (entered, receiving) = oneshot::channel();
                    let (release, held) = oneshot::channel();
                    let captured = owner.clone();
                    let caller = tokio::spawn(async move {
                        // Native read completed; final original-guard validation
                        // is still pending. No status may cross this mutation.
                        entered.send(()).unwrap();
                        held.await.unwrap();
                        NativeOwner::publish_cache_state(
                            &captured.cache.lock().unwrap(),
                            &snapshot,
                            || Ok(status()),
                        )
                    });
                    receiving.await.unwrap();
                    let waiter = start(&owner, life.clone(), async {
                        Err(CacheFailure::Provider(CacheError::Uncertain))
                    })
                    .unwrap();
                    let effect = waiter.wait_inert().await;
                    // Same production reclaim/snapshot path: the known joined
                    // row is gone, so generation alone must reject the old read.
                    let new_snapshot =
                        NativeOwner::snapshot_cache_state(&mut owner.cache.lock().unwrap(), &life)
                            .unwrap();
                    release.send(()).unwrap();
                    let published = caller.await.unwrap();
                    owner.begin_shutdown().wait().await.unwrap();
                    assert!(effect.is_err());
                    assert_eq!(published, Err(ClientError::Busy));
                    assert!(!Arc::ptr_eq(&old_generation, &new_snapshot.0));
                });
            }

            #[test]
            fn concurrent_full_drains_still_attempt_browser_after_cache_failure() {
                run(async {
                    let (owner, port) = owner(Arc::default(), true, Ok(()));
                    let (entered, receiving) = oneshot::channel();
                    let (release, held) = oneshot::channel();
                    let waiter = start(&owner, Arc::default(), async move {
                        entered.send(()).unwrap();
                        held.await.unwrap();
                        Err(CacheFailure::Interrupted)
                    })
                    .unwrap();
                    receiving.await.unwrap();
                    drop(waiter);
                    let first = owner.begin_shutdown();
                    let second = owner.begin_shutdown();
                    let same = Arc::ptr_eq(&first, &second);
                    let abandoned = {
                        let first = first.clone();
                        tokio::spawn(async move { first.wait().await })
                    };
                    abandoned.abort();
                    let _ = abandoned.await;
                    release.send(()).unwrap();
                    port.drain_started.cancelled().await;
                    port.release.cancel();
                    let result = second.wait().await;
                    assert!(same);
                    assert!(result.is_err());
                    assert_eq!(first.wait().await, result);
                    assert_eq!(port.drains.load(Ordering::Acquire), 1);
                    assert!(!owner.drained_by(&second));
                });
            }

            #[test]
            fn unpolled_cache_terminals_are_hold_not_uncertain_or_success() {
                let row = CacheRow::new(Arc::default());
                drop(CacheWorkTerminal(row.clone()));
                drop(CacheJoinTerminal {
                    row: row.clone(),
                    finished: false,
                });
                assert!(row.unresolved());
                assert!(row.admitted.is_cancelled());
                assert!(!row.reclaimable());
                assert!(matches!(
                    row.work.borrow().as_deref(),
                    Some(Err(CacheFailure::Interrupted))
                ));
                assert!(matches!(
                    row.joined.borrow().as_deref(),
                    Some(Err(CacheFailure::Interrupted))
                ));
            }
        }
    }

    struct RequestRow {
        life: Arc<BrowserLife>,
        mutation: bool,
        close: bool,
        work: Completion,
        joined: Completion,
        interest: watch::Sender<Interest>,
        work_task: Mutex<Option<Task>>,
        completion_task: Mutex<Option<Task>>,
        admitted: CancellationToken,
        failed: AtomicBool,
        bookkeeping: AtomicBool,
    }
    impl RequestRow {
        fn new(life: Arc<BrowserLife>, mutation: bool, close: bool) -> Arc<Self> {
            Arc::new(Self {
                life,
                mutation,
                close,
                work: watch::channel(None).0,
                joined: watch::channel(None).0,
                interest: watch::channel(Interest::Waiting).0,
                work_task: Mutex::default(),
                completion_task: Mutex::default(),
                admitted: CancellationToken::new(),
                failed: AtomicBool::new(false),
                bookkeeping: AtomicBool::new(false),
            })
        }
        fn launch<F, C, J>(self: &Arc<Self>, work: F, completed: C)
        where
            F: Future<Output = BrowserResult> + Send + 'static,
            C: FnOnce(Arc<Self>) -> J + Send + 'static,
            J: Future<Output = ()> + Send + 'static,
        {
            let terminal = WorkTerminal { row: self.clone() };
            let joined = JoinTerminal {
                row: self.clone(),
                finished: false,
            };
            let admitted = self.clone();
            *self.work_task.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(runtime::spawn(async move {
                    admitted.admitted.cancelled().await;
                    let result = if admitted.life.stop.is_cancelled() {
                        Err(BrowserFailure::Admission(ClientError::Cancelled))
                    } else {
                        work.await
                    };
                    terminal.finish(result);
                }));
            let row = self.clone();
            *self
                .completion_task
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(runtime::spawn(async move {
                row.admitted.cancelled().await;
                let work = row
                    .work_task
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take();
                if let Some(work) = work {
                    work.join().await;
                } else {
                    row.failed.store(true, Ordering::Release);
                }
                let result = row
                    .work
                    .borrow()
                    .clone()
                    .unwrap_or_else(|| Arc::new(Err(BrowserFailure::Interrupted)));
                // Close admission before publishing an uncertain mutation
                // result; a newly woken caller cannot race another RPC into it.
                if row.mutation
                    && result
                        .as_ref()
                        .as_ref()
                        .err()
                        .is_some_and(BrowserFailure::needs_drain)
                {
                    row.life.stop.cancel();
                }
                row.joined.send_replace(Some(result));
                completed(row).await;
                joined.finish();
            }));
        }
        async fn wait_interest(&self) -> Interest {
            let mut interest = self.interest.subscribe();
            loop {
                let value = *interest.borrow_and_update();
                if value != Interest::Waiting {
                    return value;
                }
                tokio::select! {
                    _ = self.life.stop.cancelled() => return Interest::Abandoned,
                    value = interest.changed() => if value.is_err() { return Interest::Abandoned; },
                }
            }
        }
        fn reclaimable(&self) -> bool {
            self.bookkeeping.load(Ordering::Acquire)
                && !self.failed.load(Ordering::Acquire)
                && *self.interest.borrow() != Interest::Waiting
                && self
                    .completion_task
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .as_ref()
                    .is_some_and(Task::is_finished)
        }
    }
    // Captured before either spawn: unpolled/aborted/panicked tasks cannot leave
    // a completion looking pending forever or masquerade as an ACK.
    struct WorkTerminal {
        row: Arc<RequestRow>,
    }
    impl WorkTerminal {
        fn finish(self, result: BrowserResult) {
            self.row.work.send_replace(Some(Arc::new(result)));
        }
    }
    impl Drop for WorkTerminal {
        fn drop(&mut self) {
            let missing = self.row.work.borrow().is_none();
            if missing {
                self.row.failed.store(true, Ordering::Release);
                self.row.life.stop.cancel();
                self.row.admitted.cancel();
                self.row
                    .work
                    .send_replace(Some(Arc::new(Err(BrowserFailure::Interrupted))));
            }
        }
    }
    struct JoinTerminal {
        row: Arc<RequestRow>,
        finished: bool,
    }
    impl JoinTerminal {
        fn finish(mut self) {
            self.row.bookkeeping.store(true, Ordering::Release);
            self.finished = true;
        }
    }
    impl Drop for JoinTerminal {
        fn drop(&mut self) {
            if !self.finished {
                self.row.failed.store(true, Ordering::Release);
                self.row.life.stop.cancel();
                self.row.admitted.cancel();
                self.row
                    .joined
                    .send_replace(Some(Arc::new(Err(BrowserFailure::Interrupted))));
            }
        }
    }
    pub(crate) struct RequestWaiter {
        owner: Arc<NativeOwner>,
        row: Arc<RequestRow>,
        delivered: bool,
    }
    impl RequestWaiter {
        pub(crate) async fn wait(
            self,
            guard: Arc<ModuleUseGuard>,
        ) -> Result<BrowserValue, ClientError> {
            let captured = guard.clone();
            self.wait_validated(guard.validate(), move |waiter| {
                // Reuse the ORIGINAL fence. This is validation/publication,
                // never a new module_access/auth attempt, retry or resume.
                let _publication = captured
                    .publication
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let slots = captured
                    .control
                    .slots
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                captured
                    .identity
                    .check_local_use_lifetime(&captured.fence)?;
                let current = slots.get("browser-use").is_some_and(|slot| {
                    Arc::ptr_eq(&slot.lifetime, &captured.lifetime)
                        && !slot.lifetime.is_cancelled()
                        && !slot.conflicted
                        && slot.configuration == captured.configuration
                        && slot.operation_id == captured.operation_id
                });
                if !current {
                    return Err(ClientError::Cancelled);
                }
                waiter.check_owner_publication()
            })
            .await
        }
        fn check_owner_publication(&self) -> Result<(), ClientError> {
            let state = self.owner.browser.lock().unwrap_or_else(|e| e.into_inner());
            if self.owner.epoch.is_stopped()
                || state.closing
                || state.phase != Phase::Active
                || !Arc::ptr_eq(&state.life, &self.row.life)
                || self.row.life.stop.is_cancelled()
                || !state.rows.iter().any(|row| Arc::ptr_eq(row, &self.row))
            {
                return Err(ClientError::Cancelled);
            }
            Ok(())
        }
        async fn wait_validated<V, P>(
            mut self,
            validate: V,
            publication: P,
        ) -> Result<BrowserValue, ClientError>
        where
            V: Future<Output = Result<(), ClientError>>,
            P: FnOnce(&Self) -> Result<(), ClientError>,
        {
            let mut completion = self.row.joined.subscribe();
            let result = loop {
                if let Some(result) = completion.borrow_and_update().clone() {
                    break result;
                }
                completion
                    .changed()
                    .await
                    .map_err(|_| ClientError::InvalidResponse)?;
            };
            // Keep Self/interest/Drop alive through this caller-side await.
            // Joined provider evidence stays private and unchanged on denial.
            validate.await?;
            publication(&self)?;
            // No await after this exact-life final publication boundary.
            self.delivered = true;
            self.row.interest.send_replace(Interest::Delivered);
            match result.as_ref() {
                Ok(value) => Ok(value.clone()),
                Err(error) => Err(error.client_error()),
            }
        }
        #[cfg(test)]
        async fn wait_inert(self) -> Result<BrowserValue, ClientError> {
            // Only the Shared ownership fence is exercised here; real captured
            // identity/Vault validation belongs to the owning integration gate.
            self.wait_validated(async { Ok(()) }, Self::check_owner_publication)
                .await
        }
    }
    impl Drop for RequestWaiter {
        fn drop(&mut self) {
            if !self.delivered {
                if self.row.mutation {
                    self.owner.abandon(&self.row.life);
                }
                self.row.interest.send_replace(Interest::Abandoned);
            }
        }
    }
    pub(crate) struct DrainState {
        result: watch::Sender<Option<Result<(), ClientError>>>,
        task: Mutex<Option<Task>>,
        admitted: CancellationToken,
    }
    impl DrainState {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                result: watch::channel(None).0,
                task: Mutex::default(),
                admitted: CancellationToken::new(),
            })
        }
        fn launch(
            self: &Arc<Self>,
            work: impl Future<Output = Result<(), ClientError>> + Send + 'static,
        ) {
            let terminal = DrainTerminal {
                state: self.clone(),
                finished: false,
            };
            let admitted = self.admitted.clone();
            *self.task.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(runtime::spawn(async move {
                    admitted.cancelled().await;
                    let result = work.await;
                    terminal.finish(result);
                }));
        }
        pub(crate) async fn wait(&self) -> Result<(), ClientError> {
            let mut completion = self.result.subscribe();
            loop {
                if let Some(result) = completion.borrow_and_update().clone() {
                    return result;
                }
                completion
                    .changed()
                    .await
                    .map_err(|_| ClientError::InvalidResponse)?;
            }
        }
    }
    struct DrainTerminal {
        state: Arc<DrainState>,
        finished: bool,
    }
    impl DrainTerminal {
        fn finish(mut self, result: Result<(), ClientError>) {
            self.state.result.send_replace(Some(result));
            self.finished = true;
        }
    }
    impl Drop for DrainTerminal {
        fn drop(&mut self) {
            if !self.finished {
                self.state.result.send_replace(Some(Err(ClientError::Server(
                    "browser_cleanup_unresolved".into(),
                ))));
            }
        }
    }

    #[cfg(test)]
    mod ownership_tests {
        use super::*;
        use std::sync::atomic::AtomicUsize;
        use tokio::sync::oneshot;

        pub(super) fn run(work: impl Future<Output = ()>) {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    runtime::timeout(std::time::Duration::from_secs(5), work)
                        .await
                        .expect("bounded inert ownership fixture");
                });
        }
        pub(super) fn owner(
            epoch: Arc<Epoch>,
            held: bool,
            result: Result<(), ClientError>,
        ) -> (Arc<NativeOwner>, Arc<InertPort>) {
            let port = Arc::new(InertPort {
                cancelled: AtomicUsize::new(0),
                drains: AtomicUsize::new(0),
                resumed: AtomicUsize::new(0),
                drain_started: CancellationToken::new(),
                release: CancellationToken::new(),
                result,
            });
            if !held {
                port.release.cancel();
            }
            (
                Arc::new(NativeOwner {
                    agent: NativePort::Inert(port.clone()),
                    epoch,
                    browser: Mutex::new(BrowserState {
                        life: Arc::default(),
                        phase: Phase::Active,
                        closing: false,
                        cancel_sent: false,
                        rows: Vec::new(),
                        drain: None,
                    }),
                    #[cfg(target_os = "linux")]
                    cache: Mutex::default(),
                }),
                port,
            )
        }
        async fn published(row: &RequestRow) {
            let mut result = row.joined.subscribe();
            while result.borrow_and_update().is_none() {
                result.changed().await.unwrap();
            }
        }

        #[test]
        fn dropped_mutation_waiter_retains_held_work_until_actual_join_then_drain() {
            run(async {
                let (owner, port) = owner(Arc::default(), false, Ok(()));
                let (entered, receiving) = oneshot::channel();
                let (release, held) = oneshot::channel();
                let released = Arc::new(AtomicBool::new(false));
                struct Released(Arc<AtomicBool>);
                impl Drop for Released {
                    fn drop(&mut self) {
                        self.0.store(true, Ordering::Release);
                    }
                }
                let finished = released.clone();
                let waiter = owner
                    .start_request(true, false, async move {
                        let _actual_work_lifetime = Released(finished);
                        entered.send(()).unwrap();
                        held.await.unwrap();
                        Ok(BrowserValue::Closed)
                    })
                    .unwrap();
                let row = waiter.row.clone();
                receiving.await.unwrap();
                let caller = tokio::spawn(async move { waiter.wait_inert().await });
                caller.abort();
                let _ = caller.await;
                assert!(row.life.stop.is_cancelled());
                assert!(!released.load(Ordering::Acquire));
                assert!(row.joined.borrow().is_none());
                assert_eq!(port.drains.load(Ordering::Acquire), 0);
                let drain = owner.begin_drain();
                release.send(()).unwrap();
                drain.wait().await.unwrap();
                assert!(released.load(Ordering::Acquire));
                assert!(row.bookkeeping.load(Ordering::Acquire));
                assert!(matches!(
                    row.joined.borrow().as_deref(),
                    Some(Ok(BrowserValue::Closed))
                ));
                assert_eq!(port.drains.load(Ordering::Acquire), 1);
            });
        }
        #[test]
        fn drop_after_joined_result_publication_still_triggers_retained_drain() {
            run(async {
                let (owner, port) = owner(Arc::default(), false, Ok(()));
                let waiter = owner
                    .start_request(true, true, async { Ok(BrowserValue::Closed) })
                    .unwrap();
                let row = waiter.row.clone();
                published(&row).await;
                assert!(!row.bookkeeping.load(Ordering::Acquire));
                drop(waiter);
                port.drain_started.cancelled().await;
                owner.begin_drain().wait().await.unwrap();
                assert!(row.bookkeeping.load(Ordering::Acquire));
                assert_eq!(port.drains.load(Ordering::Acquire), 1);
            });
        }
        #[test]
        fn held_delivery_rechecks_lifetime_and_retains_private_provider_outcome() {
            run(async {
                let (owner, _) = owner(Arc::default(), false, Ok(()));
                let waiter = owner
                    .start_request(true, false, async {
                        Ok(BrowserValue::Outcome(browser::Outcome {
                            failure: Some(browser::ScriptFailure::ScriptError),
                            output: "PRIVATE-OLD-OUTPUT".into(),
                            truncated: false,
                            tabs: Vec::new(),
                        }))
                    })
                    .unwrap();
                let row = waiter.row.clone();
                published(&row).await;
                let (entered, checking) = oneshot::channel();
                let (release, held) = oneshot::channel();
                let caller = tokio::spawn(async move {
                    waiter
                        .wait_validated(
                            async move {
                                entered.send(()).unwrap();
                                held.await.unwrap();
                                Ok(())
                            },
                            RequestWaiter::check_owner_publication,
                        )
                        .await
                });
                checking.await.unwrap();
                owner.cancel_browser();
                release.send(()).unwrap();
                assert!(matches!(caller.await.unwrap(), Err(ClientError::Cancelled)));
                owner.begin_drain().wait().await.unwrap();
                assert!(matches!(row.joined.borrow().as_deref(),
                    Some(Ok(BrowserValue::Outcome(value)))
                        if value.output == "PRIVATE-OLD-OUTPUT"
                        && value.failure == Some(browser::ScriptFailure::ScriptError)));
            });
        }
        #[test]
        fn held_delivery_cannot_publish_or_cancel_a_replacement_lifetime() {
            run(async {
                let (owner, port) = owner(Arc::default(), false, Ok(()));
                let waiter = owner
                    .start_request(true, false, async { Ok(BrowserValue::Closed) })
                    .unwrap();
                let row = waiter.row.clone();
                published(&row).await;
                let (entered, checking) = oneshot::channel();
                let (release, held) = oneshot::channel();
                let caller = tokio::spawn(async move {
                    waiter
                        .wait_validated(
                            async move {
                                entered.send(()).unwrap();
                                held.await.unwrap();
                                Ok(())
                            },
                            RequestWaiter::check_owner_publication,
                        )
                        .await
                });
                checking.await.unwrap();
                owner.begin_drain().wait().await.unwrap();
                // Isolate only the same-Arc publication/drop fence, not the
                // real identity/Vault authorization needed for explicit resume.
                let replacement = Arc::new(BrowserLife::default());
                {
                    let mut state = owner.browser.lock().unwrap();
                    state.life = replacement.clone();
                    state.phase = Phase::Active;
                    state.cancel_sent = false;
                    state.drain = None;
                }
                let before = port.cancelled.load(Ordering::Acquire);
                release.send(()).unwrap();
                assert!(matches!(caller.await.unwrap(), Err(ClientError::Cancelled)));
                assert!(!replacement.stop.is_cancelled());
                assert_eq!(port.cancelled.load(Ordering::Acquire), before);
                assert!(matches!(
                    row.joined.borrow().as_deref(),
                    Some(Ok(BrowserValue::Closed))
                ));
            });
        }
        #[test]
        fn dropping_mutation_during_final_validation_keeps_result_and_owned_drain() {
            run(async {
                let (owner, port) = owner(Arc::default(), false, Ok(()));
                let waiter = owner
                    .start_request(true, true, async { Ok(BrowserValue::Closed) })
                    .unwrap();
                let row = waiter.row.clone();
                published(&row).await;
                let (entered, checking) = oneshot::channel();
                let (_release, held) = oneshot::channel::<()>();
                let caller = tokio::spawn(async move {
                    waiter
                        .wait_validated(
                            async move {
                                entered.send(()).unwrap();
                                held.await.unwrap();
                                Ok(())
                            },
                            RequestWaiter::check_owner_publication,
                        )
                        .await
                });
                checking.await.unwrap();
                caller.abort();
                let _ = caller.await;
                port.drain_started.cancelled().await;
                owner.begin_drain().wait().await.unwrap();
                assert!(row.bookkeeping.load(Ordering::Acquire));
                assert!(matches!(
                    row.joined.borrow().as_deref(),
                    Some(Ok(BrowserValue::Closed))
                ));
                assert_eq!(port.drains.load(Ordering::Acquire), 1);
            });
        }
        #[test]
        fn dropping_metadata_during_final_validation_does_not_cancel_owner() {
            run(async {
                let (owner, port) = owner(Arc::default(), false, Ok(()));
                let waiter = owner
                    .start_request(false, false, async { Ok(BrowserValue::Closed) })
                    .unwrap();
                let row = waiter.row.clone();
                let (entered, checking) = oneshot::channel();
                let (_release, held) = oneshot::channel::<()>();
                let caller = tokio::spawn(async move {
                    waiter
                        .wait_validated(
                            async move {
                                entered.send(()).unwrap();
                                held.await.unwrap();
                                Ok(())
                            },
                            RequestWaiter::check_owner_publication,
                        )
                        .await
                });
                checking.await.unwrap();
                caller.abort();
                let _ = caller.await;
                let completion = row.completion_task.lock().unwrap().take().unwrap();
                completion.join().await;
                assert!(!row.life.stop.is_cancelled());
                assert_eq!(port.cancelled.load(Ordering::Acquire), 0);
                assert_eq!(port.drains.load(Ordering::Acquire), 0);
            });
        }
        #[test]
        fn owner_observations_expose_draining_and_hold_without_new_dto() {
            run(async {
                let (owner, port) = owner(Arc::default(), true, Err(ClientError::Timeout));
                assert!(owner.observations()[0].error.is_none());
                let drain = owner.begin_drain();
                assert_eq!(owner.observations()[0].error, Some(ModuleError::Busy));
                assert!(owner.observations()[0].observed_at_ms.is_none());
                port.release.cancel();
                assert_eq!(drain.wait().await, Err(ClientError::Timeout));
                assert_eq!(
                    owner.observations()[0].error,
                    Some(ModuleError::Unavailable)
                );
            });
        }
        #[test]
        fn metadata_waiter_drop_does_not_cancel_owner_or_browser_siblings() {
            run(async {
                let (owner, port) = owner(Arc::default(), false, Ok(()));
                let waiter = owner
                    .start_request(false, false, async { Ok(BrowserValue::Closed) })
                    .unwrap();
                let row = waiter.row.clone();
                drop(waiter);
                published(&row).await;
                let completion = row.completion_task.lock().unwrap().take().unwrap();
                completion.join().await;
                assert!(!row.life.stop.is_cancelled());
                assert_eq!(port.cancelled.load(Ordering::Acquire), 0);
                assert_eq!(port.drains.load(Ordering::Acquire), 0);
            });
        }
        #[test]
        fn concurrent_drain_waiters_share_result_even_if_first_waiter_disappears() {
            run(async {
                let (owner, port) = owner(Arc::default(), true, Err(ClientError::Timeout));
                let drain = owner.begin_drain();
                let first = {
                    let drain = drain.clone();
                    tokio::spawn(async move { drain.wait().await })
                };
                let second = {
                    let drain = owner.begin_drain();
                    tokio::spawn(async move { drain.wait().await })
                };
                port.drain_started.cancelled().await;
                first.abort();
                let _ = first.await;
                port.release.cancel();
                assert_eq!(second.await.unwrap(), Err(ClientError::Timeout));
                assert_eq!(drain.wait().await, Err(ClientError::Timeout));
                assert_eq!(port.drains.load(Ordering::Acquire), 1);
                assert!(!owner.drained_by(&drain));
            });
        }
        #[test]
        fn repeated_waiters_do_not_cancel_again_after_positive_empty_drain() {
            run(async {
                let (owner, port) = owner(Arc::default(), false, Ok(()));
                let first = owner.begin_drain();
                first.wait().await.unwrap();
                let cancellations = port.cancelled.load(Ordering::Acquire);
                owner.cancel_browser();
                let second = owner.begin_drain();
                assert!(Arc::ptr_eq(&first, &second));
                second.wait().await.unwrap();
                assert_eq!(port.cancelled.load(Ordering::Acquire), cancellations);
                assert_eq!(port.drains.load(Ordering::Acquire), 1);
                assert!(owner.drained_by(&first));
            });
        }
        #[test]
        fn panic_terminal_remains_hold_even_after_physical_drain_attempt() {
            run(async {
                let (owner, port) = owner(Arc::default(), false, Ok(()));
                let waiter = owner
                    .start_request(true, false, async { panic!("inert work failure") })
                    .unwrap();
                assert!(waiter.wait_inert().await.is_err());
                let drain = owner.begin_drain();
                assert!(drain.wait().await.is_err());
                assert_eq!(port.drains.load(Ordering::Acquire), 1);
                assert!(!owner.browser.lock().unwrap().rows.is_empty());
                assert!(!owner.drained_by(&drain));
            });
        }
        #[test]
        fn unpolled_terminal_guards_publish_hold_instead_of_pending_or_success() {
            let row = RequestRow::new(Arc::default(), true, false);
            drop(WorkTerminal { row: row.clone() });
            drop(JoinTerminal {
                row: row.clone(),
                finished: false,
            });
            assert!(row.life.stop.is_cancelled());
            assert!(row.failed.load(Ordering::Acquire));
            assert!(matches!(
                row.joined.borrow().as_deref(),
                Some(Err(BrowserFailure::Interrupted))
            ));
            assert!(!row.reclaimable());
        }
        #[test]
        fn both_task_handles_exist_before_admission_allows_work_to_poll() {
            run(async {
                let row = RequestRow::new(Arc::default(), false, false);
                let entered = Arc::new(AtomicBool::new(false));
                let observed = entered.clone();
                row.launch(
                    async move {
                        observed.store(true, Ordering::Release);
                        Ok(BrowserValue::Closed)
                    },
                    |_| async {},
                );
                assert!(row.work_task.lock().unwrap().is_some());
                assert!(row.completion_task.lock().unwrap().is_some());
                tokio::task::yield_now().await;
                assert!(!entered.load(Ordering::Acquire));
                row.admitted.cancel();
                let completion = row.completion_task.lock().unwrap().take().unwrap();
                completion.join().await;
                assert!(entered.load(Ordering::Acquire));
                assert!(row.bookkeeping.load(Ordering::Acquire));
                assert!(row.work_task.lock().unwrap().is_none());
            });
        }
        #[test]
        fn eight_strong_owners_include_hold_and_stale_epoch_is_refused() {
            let mut owners = Owners::default();
            for _ in 0..8 {
                let (owner, _) = owner(owners.epoch.clone(), false, Ok(()));
                owner.browser.lock().unwrap().phase = Phase::Hold;
                owners.owners.push(owner);
            }
            assert_eq!(owners.admit(&owners.epoch), Err(ClientError::Busy));
            assert_eq!(owners.admit(&Arc::default()), Err(ClientError::Cancelled));
            assert_eq!(owners.owners.len(), 8);
        }
        #[test]
        fn ordinary_and_reserved_close_rows_are_independently_bounded() {
            run(async {
                let (owner, _) = owner(Arc::default(), false, Ok(()));
                let release = CancellationToken::new();
                let mut callers = Vec::new();
                for close in [false, true] {
                    for _ in 0..8 {
                        let release = release.clone();
                        callers.push(
                            owner
                                .start_request(true, close, async move {
                                    release.cancelled().await;
                                    Ok(BrowserValue::Closed)
                                })
                                .unwrap(),
                        );
                    }
                    assert_eq!(
                        owner
                            .start_request(true, close, async { Ok(BrowserValue::Closed) })
                            .err(),
                        Some(ClientError::Busy)
                    );
                }
                assert_eq!(owner.browser.lock().unwrap().rows.len(), 16);
                drop(callers);
                release.cancel();
                owner.begin_drain().wait().await.unwrap();
            });
        }
        #[test]
        fn stopped_admission_does_not_reclaim_hold_records_or_poll_new_work() {
            run(async {
                let (owner, _) = owner(Arc::default(), false, Ok(()));
                owner.cancel_browser();
                let entered = Arc::new(AtomicBool::new(false));
                let observed = entered.clone();
                assert_eq!(
                    owner
                        .start_request(true, false, async move {
                            observed.store(true, Ordering::Release);
                            Ok(BrowserValue::Closed)
                        })
                        .err(),
                    Some(ClientError::Cancelled)
                );
                assert!(!entered.load(Ordering::Acquire));
                assert!(owner.browser.lock().unwrap().rows.is_empty());
            });
        }
        #[test]
        fn late_old_waiter_drop_cannot_cancel_a_replacement_browser_lifetime() {
            run(async {
                let (owner, port) = owner(Arc::default(), false, Ok(()));
                let waiter = owner
                    .start_request(true, false, async { Ok(BrowserValue::Closed) })
                    .unwrap();
                published(&waiter.row).await;
                owner.begin_drain().wait().await.unwrap();
                // Authorization/resume itself belongs to the identity/Vault
                // fixture; this isolates the exact-Arc stale-drop fence.
                let replacement = Arc::new(BrowserLife::default());
                {
                    let mut state = owner.browser.lock().unwrap();
                    state.life = replacement.clone();
                    state.phase = Phase::Active;
                    state.cancel_sent = false;
                    state.drain = None;
                }
                let before = port.cancelled.load(Ordering::Acquire);
                drop(waiter);
                assert!(!replacement.stop.is_cancelled());
                assert_eq!(port.cancelled.load(Ordering::Acquire), before);
            });
        }
        #[test]
        fn shutdown_attempts_all_owners_and_retains_failed_owner_capacity() {
            run(async {
                let control = ModuleControl::default();
                let epoch = control.native.lock().unwrap().epoch.clone();
                let (failed, first) = owner(epoch.clone(), false, Err(ClientError::Timeout));
                let (later, second) = owner(epoch, false, Ok(()));
                control
                    .native
                    .lock()
                    .unwrap()
                    .owners
                    .extend([failed.clone(), later]);
                assert_eq!(control.shutdown().await, Err(ClientError::Timeout));
                assert_eq!(first.drains.load(Ordering::Acquire), 1);
                assert_eq!(second.drains.load(Ordering::Acquire), 1);
                let owners = control.native.lock().unwrap();
                assert_eq!(owners.owners.len(), 1);
                assert!(Arc::ptr_eq(&owners.owners[0], &failed));
            });
        }
        #[test]
        fn stale_composition_generation_cannot_advance_a_new_identity() {
            let control = ModuleControl::default();
            let old = control.seal();
            control.new_identity_generation(&old).unwrap();
            assert!(!control.is_generation(&old));
            assert_eq!(
                control.new_identity_generation(&old),
                Err(ClientError::Cancelled)
            );
        }
    }
}

fn control_view(policy: Option<&DevicePolicy>, source: &str) -> DeviceControlView {
    match policy {
        Some(policy) => DeviceControlView {
            mode: policy.mode().as_str().into(),
            purposes: policy
                .purposes()
                .iter()
                .map(|value| value.as_str().to_owned())
                .collect(),
            revision: policy.revision(),
            source: source.into(),
        },
        None => DeviceControlView {
            mode: ControlMode::Manual.as_str().into(),
            purposes: Vec::new(),
            revision: 0,
            source: "default".into(),
        },
    }
}

/// Unreachable or older servers keep the last observed policy; refusals,
/// cancellation and malformed replies propagate unchanged.
fn policy_unreachable(error: &ClientError) -> bool {
    match error {
        ClientError::Transport
        | ClientError::Timeout
        | ClientError::Busy
        | ClientError::OnlineVerificationRequired => true,
        ClientError::Server(code) => matches!(
            code.as_str(),
            "device_policy_unavailable" | "dependency_unavailable" | "server_busy" | "rate_limited"
        ),
        _ => false,
    }
}

/// One correlated admission record: original principal, device, operation and
/// the mode that decided it. No configuration or secret content is recorded.
fn admission_event(
    identity: &IdentityClient,
    module: &str,
    mode: ControlMode,
    operation: &str,
    refusal: Option<nddev_device_sync_domain::modules::control::Readiness>,
) {
    let view = identity.view();
    // Records need the original identifiers; unauthenticated states emit none.
    if view.current_device_id.is_empty() || view.user_id.is_empty() {
        return;
    }
    let event = match mode {
        ControlMode::Fullauto => "module.admission.fullauto",
        ControlMode::Manual => "module.admission.manual",
    };
    match refusal {
        None => tracing::event!(target: "nds_native", tracing::Level::INFO, {
            event.name = event, module, device.id = %view.current_device_id,
            user.id = %view.user_id, operation.id = %operation, outcome = "accepted"
        }),
        Some(reason) => {
            let error = tag(reason).unwrap_or_default();
            tracing::event!(target: "nds_native", tracing::Level::WARN, {
                event.name = event, module, device.id = %view.current_device_id,
                user.id = %view.user_id, operation.id = %operation,
                error.type = %format!("module_{error}"), outcome = "denied"
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edit_basis_follows_only_its_own_acknowledgement() {
        let mut current = nddev_device_sync_vault::ModuleSetting {
            revision: 7,
            operation_id: Some("original".into()),
            ..Default::default()
        };
        assert!(edit_basis_matches(&current, 6, Some("original")));
        assert!(edit_basis_matches(&current, 7, None));
        assert!(!edit_basis_matches(&current, 6, None));
        assert!(!edit_basis_matches(&current, 6, Some("other")));
        // Equal values from a different edit cannot revive an earlier draft.
        current.operation_id = Some("replacement".into());
        assert!(!edit_basis_matches(&current, 6, Some("original")));
        current.pending_id = Some("replacement".into());
        assert!(!edit_basis_matches(&current, 7, None));
        assert!(edit_basis_matches(&current, 7, Some("replacement")));
        current.conflicted = true;
        assert!(!edit_basis_matches(&current, 7, Some("replacement")));
    }
    #[test]
    fn acknowledgement_preserves_lifetime_but_an_identical_new_edit_cannot_revive_it() {
        let control = ModuleControl::default();
        let configuration = Configuration {
            enabled: true,
            permissions: BTreeSet::from([Permission::ReadOnly]),
        };
        let (original, _) =
            control.observe_configuration("gds", &configuration, false, Some("original"), 10);
        let (acknowledged, changed) =
            control.observe_configuration("gds", &configuration, false, Some("original"), 10);
        assert!(!changed && Arc::ptr_eq(&original, &acknowledged) && !original.is_cancelled());
        // Withdrawal and reapproval may both occur between local observations.
        let (fresh, changed) =
            control.observe_configuration("gds", &configuration, false, Some("new-choice"), 12);
        assert!(changed && original.is_cancelled() && !fresh.is_cancelled());
        assert!(!Arc::ptr_eq(&original, &fresh));
        let (stale, changed) =
            control.observe_configuration("gds", &configuration, false, Some("original"), 10);
        assert!(stale.is_cancelled() && !changed && !fresh.is_cancelled());
        let (retained, changed) =
            control.observe_configuration("gds", &configuration, false, Some("new-choice"), 13);
        assert!(!changed && Arc::ptr_eq(&retained, &fresh));
        control.stop();
        assert!(fresh.is_cancelled());
        let (new_context, _) =
            control.observe_configuration("gds", &configuration, false, Some("other-store"), 0);
        assert!(!new_context.is_cancelled());
    }
    #[test]
    fn device_policy_replacement_closes_only_system_tool_admissions() {
        let control = ModuleControl::default();
        let configuration = Configuration::default();
        let (tool, _) = control.observe_configuration("sysinfo", &configuration, false, None, 1);
        let (gds, _) = control.observe_configuration("gds", &configuration, false, None, 1);
        assert_eq!(
            control.cached_basis(),
            ControlBasis {
                mode: ControlMode::Manual,
                revision: 0
            }
        );
        let enrolled = DevicePolicy::enrolled();
        control.observe_policy(&enrolled);
        assert!(
            tool.is_cancelled(),
            "admission under the unverified default survived"
        );
        assert!(
            !gds.is_cancelled(),
            "explicit-consent modules keep their lifetime"
        );
        assert_eq!(
            control.cached_basis(),
            ControlBasis {
                mode: ControlMode::Fullauto,
                revision: 1
            }
        );
        let (current, _) = control.observe_configuration("sysinfo", &configuration, false, None, 1);
        assert!(!current.is_cancelled());
        // Observing the same authenticated policy again keeps useful work.
        control.observe_policy(&enrolled);
        assert!(!current.is_cancelled());
        let narrowed = DevicePolicy::new(
            ControlMode::Manual,
            BTreeSet::from([DevicePurpose::Desktop]),
            2,
        )
        .unwrap();
        control.observe_policy(&narrowed);
        assert!(current.is_cancelled() && !gds.is_cancelled());
        // A replacement identity never inherits the previous policy.
        control.stop();
        assert_eq!(control.cached_basis().revision, 0);
    }
    #[test]
    fn only_unreachable_policy_reads_fall_back_and_none_widens() {
        for error in [
            ClientError::Transport,
            ClientError::Timeout,
            ClientError::Busy,
            ClientError::OnlineVerificationRequired,
            ClientError::Server("device_policy_unavailable".into()),
            ClientError::Server("dependency_unavailable".into()),
        ] {
            assert!(policy_unreachable(&error), "{error}");
        }
        for error in [
            ClientError::Cancelled,
            ClientError::InvalidResponse,
            ClientError::RevalidationRequired,
            ClientError::DeviceRevoked,
            ClientError::Server("operation_conflict".into()),
            ClientError::Server("invalid_request".into()),
        ] {
            assert!(!policy_unreachable(&error), "{error}");
        }
        let default = control_view(None, "default");
        assert_eq!(
            (
                default.mode.as_str(),
                default.revision,
                default.source.as_str()
            ),
            ("manual", 0, "default")
        );
        let observed = control_view(Some(&DevicePolicy::enrolled()), "cached");
        assert_eq!(
            (
                observed.mode.as_str(),
                observed.purposes,
                observed.source.as_str()
            ),
            ("fullauto", vec!["desktop".to_owned()], "cached")
        );
    }
}
