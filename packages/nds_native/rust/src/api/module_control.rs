//! Device module composition over Core policy and Vault's encrypted settings.
//! Descriptors, consent, actual adapter presence and observations stay distinct.
use super::identity::NdsClient;
#[cfg(feature = "flutter")]
use flutter_rust_bridge::frb;
use nddev_device_sync_accounts::{ClientError, IdentityClient, LocalUseFence};
use nddev_device_sync_domain::{
    ModuleDescriptor, ModuleId, Permission, Platform,
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
    pub readiness: String,
    pub checked_at_ms: Option<u64>,
    pub error_class: Option<String>,
    pub revision: u64,
    pub pending_id: Option<String>,
    pub conflicted: bool,
}

#[derive(Default)]
#[cfg_attr(feature = "flutter", frb(ignore))]
pub(crate) struct ModuleControl {
    slots: Mutex<BTreeMap<String, Slot>>,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub(crate) discovery: tokio::sync::Mutex<()>,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    native: Mutex<Vec<std::sync::Weak<nddev_device_sync_agent::NativeAgent>>>,
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
                .iter()
                .filter_map(std::sync::Weak::upgrade)
            {
                let _ = agent.invalidate_module(module);
            }
        }
        (slot.lifetime.clone(), changed)
    }
    pub(crate) fn stop(&self) {
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        for slot in slots.values() {
            slot.lifetime.cancel();
        }
        slots.clear();
        drop(slots);
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        for agent in self
            .native
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .filter_map(|host| host.upgrade())
        {
            agent.close();
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub(crate) fn detach(&self, agent: &Arc<nddev_device_sync_agent::NativeAgent>) {
        self.native
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|host| {
                host.upgrade()
                    .is_some_and(|live| !Arc::ptr_eq(&live, agent))
            });
    }
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub(crate) fn attach(
        &self,
        agent: &Arc<nddev_device_sync_agent::NativeAgent>,
    ) -> Result<(), ClientError> {
        let mut hosts = self.native.lock().unwrap_or_else(|e| e.into_inner());
        hosts.retain(|host| host.strong_count() > 0);
        if hosts
            .iter()
            .filter_map(std::sync::Weak::upgrade)
            .any(|host| Arc::ptr_eq(&host, agent))
        {
            return Ok(());
        }
        // The module and clipboard views own separate bounded I/O lifetimes.
        if hosts.len() >= 8 {
            return Err(ClientError::Busy);
        }
        hosts.push(Arc::downgrade(agent));
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
fn shared_permissions(module: &str) -> Option<BTreeSet<Permission>> {
    use Permission::*;
    match module {
        "clipboard" => Some(BTreeSet::from([Clipboard, LocalNetwork])),
        "gds" => Some(BTreeSet::from([ReadOnly, CredentialStore, LocalNetwork])),
        "rds" if cfg!(target_os = "linux") => Some(BTreeSet::from([ReadOnly, LocalNetwork])),
        _ => None,
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
}
impl ModuleUseGuard {
    /// Only an actual adapter result in this still-current lifetime can refresh
    /// readiness. Consent and successful settings persistence are not probes.
    pub(crate) fn observe<T>(&self, result: &Result<T, ModuleError>) {
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
        if self.lifetime.is_cancelled() {
            return Err(ClientError::Cancelled);
        }
        self.identity.validate_local_use(&self.fence).await?;
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
        if let Err(reason) = setting.configuration.admit(supported, installed, &required) {
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
        };
        guard.validate().await?;
        Ok(guard)
    }
    async fn module_changed(&self, module: &str) {
        if module == "gds" {
            self.cancel_github_catalog();
        }
        if module == "rds" {
            self.cancel_screen();
        }
    }
    async fn native_module_observations(&self) -> Result<Vec<ModuleObservation>, ClientError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            let agent = self
                .module_control
                .native
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .rev()
                .find_map(std::sync::Weak::upgrade);
            Ok(agent.map_or_else(Vec::new, |agent| agent.modules()))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Ok(Vec::new())
        }
    }
    pub async fn device_modules(&self) -> Result<Vec<DeviceModuleView>, ClientError> {
        observe("agent", "module.readiness.read", self.read_device_modules()).await
    }
    async fn read_device_modules(&self) -> Result<Vec<DeviceModuleView>, ClientError> {
        let vault = self.vault().await.map_err(vault_error)?;
        let identity = self.shared_identity();
        let fence = identity.local_use_fence().await?;
        let native = self.native_module_observations().await?;
        let graph =
            nddev_device_sync_agent::module_graph().map_err(|_| ClientError::InvalidResponse)?;
        let mut result = Vec::new();
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
            let shared_observation = self
                .module_control
                .slots
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(id)
                .and_then(|slot| slot.observation.clone());
            let observation = if changed {
                None
            } else {
                [native_observation, shared_observation.as_ref()]
                    .into_iter()
                    .flatten()
                    .max_by_key(|value| value.observed_at_ms)
            };
            let installed = if shared.is_some() {
                Some(true)
            } else if native_supported {
                native_observation.map(|v| v.configured)
            } else {
                Some(false)
            };
            let readiness = if setting.conflicted {
                "conflicted".into()
            } else {
                tag(setting.configuration.readiness(
                    supported,
                    installed.unwrap_or(true),
                    &required,
                    observation,
                ))?
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
                readiness,
                checked_at_ms: observation.and_then(|v| v.observed_at_ms),
                error_class: observation.and_then(|v| v.error).map(tag).transpose()?,
                revision: setting.revision,
                pending_id: setting.pending_id,
                conflicted: setting.conflicted,
            });
        }
        identity.validate_local_use(&fence).await?;
        Ok(result)
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
                if enabled && !supported {
                    return Err(ClientError::Server("module_unsupported".into()));
                }
                if permissions.len() > required.len() {
                    return Err(ClientError::InvalidInput);
                }
                let parsed = permissions
                    .iter()
                    .map(|value| {
                        serde_json::from_value(serde_json::Value::String(value.clone()))
                            .map_err(|_| ClientError::InvalidInput)
                    })
                    .collect::<Result<BTreeSet<Permission>, _>>()?;
                if parsed.len() != permissions.len() || !parsed.is_subset(&required) {
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
}
