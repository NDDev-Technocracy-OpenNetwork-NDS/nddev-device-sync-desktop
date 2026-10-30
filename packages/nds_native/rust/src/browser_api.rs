//! Thin browser boundary over the same Rust composition used by Flutter.
//! Public values are generated metadata projections; commands stay opaque.
use crate::api::{
    catalog_types::{
        GithubCatalogError, GithubCredentialReferencePage, GithubRepositoryPageView,
        GithubSavedRepositoryPageView, GithubSourceView,
    },
    identity::{IncidentPage, IncidentView, NdsClient},
    module_control::DeviceModuleView,
    organizations::{CompanyCommand, LocalOrganization},
    telemetry,
};
use nddev_device_sync_accounts::{
    ClientError,
    view::{DeliveryView, PasskeyView, View},
};
use nddev_device_sync_organizations::view::*;
use nddev_device_sync_telemetry::view::{
    ClientLifecycle, DebugScope, FailureKind, LocalDiagnostics,
};
use nddev_device_sync_vault::view::{ClipboardView, RecordPage, StatePage, VaultError, VaultView};
use serde::{Serialize, de::DeserializeOwned};
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

fn present<T: Tsify + Serialize>(value: T) -> Result<Ts<T>, JsError> {
    value
        .into_ts()
        .map_err(|_| JsError::new("invalid_response"))
}
fn client_error(error: ClientError) -> JsError {
    JsError::new(&error.code())
}
fn vault_error(error: VaultError) -> JsError {
    JsError::new(&error.code())
}
fn catalog_error(error: GithubCatalogError) -> JsError {
    JsError::new(&error.to_string())
}
fn unsigned_number(value: js_sys::Number) -> Result<u32, JsError> {
    JsValue::from(value)
        .as_f64()
        .filter(|n| n.is_finite() && n.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(n))
        .map(|n| n as u32)
        .ok_or_else(|| JsError::new("invalid_input"))
}
fn boolean_input(value: js_sys::Boolean) -> Result<bool, JsError> {
    JsValue::from(value)
        .as_bool()
        .ok_or_else(|| JsError::new("invalid_input"))
}
fn revision(value: js_sys::BigInt) -> Result<u64, JsError> {
    value.try_into().map_err(|_| JsError::new("invalid_input"))
}
fn enum_input<T: Tsify + DeserializeOwned>(value: Ts<T>) -> Result<T, JsError>
where
    T::JsType: Clone,
{
    if !value.js_value().is_string() {
        return Err(JsError::new("invalid_input"));
    }
    value.to_rust().map_err(|_| JsError::new("invalid_input"))
}
#[wasm_bindgen]
pub fn initialize_app() -> Result<(), JsError> {
    telemetry::init_app().map_err(|error| JsError::new(&error.to_string()))
}
#[wasm_bindgen]
pub fn capture_invitation_link(route: String) -> Result<(), JsError> {
    crate::api::organizations::capture_invitation_link(route).map_err(client_error)
}
#[wasm_bindgen]
pub fn new_vault_record_id() -> String {
    crate::api::identity::new_vault_record_id()
}
#[wasm_bindgen]
pub fn supports_clipboard() -> bool {
    crate::api::identity::supports_native_clipboard()
}
#[wasm_bindgen]
pub fn client_diagnostics() -> Result<Ts<LocalDiagnostics>, JsError> {
    present(telemetry::client_diagnostics())
}
#[wasm_bindgen]
pub fn record_client_failure(kind: Ts<FailureKind>) -> Result<bool, JsError> {
    Ok(telemetry::record_client_failure(enum_input(kind)?))
}
#[wasm_bindgen]
pub fn record_client_lifecycle(state: Ts<ClientLifecycle>) -> Result<bool, JsError> {
    Ok(telemetry::record_client_lifecycle(enum_input(state)?))
}
#[wasm_bindgen]
pub async fn shutdown_telemetry() -> Result<Ts<LocalDiagnostics>, JsError> {
    present(
        telemetry::shutdown_telemetry()
            .await
            .map_err(|error| JsError::new(&error.to_string()))?,
    )
}
#[wasm_bindgen]
pub fn configure_debug(scope: Ts<DebugScope>, seconds: js_sys::Number) -> Result<(), JsError> {
    // Preserve the number at the ABI; the owning configuration enforces its
    // policy after this checked representation conversion (never integer wrap).
    let seconds =
        u16::try_from(unsigned_number(seconds)?).map_err(|_| JsError::new("invalid_input"))?;
    telemetry::configure_debug(enum_input(scope)?, seconds)
        .map_err(|error| JsError::new(&error.to_string()))
}
#[wasm_bindgen(js_name=NdsClient)]
pub struct BrowserClient {
    inner: NdsClient,
}
impl Drop for BrowserClient {
    fn drop(&mut self) {
        self.inner.cancel();
    }
}
#[wasm_bindgen(js_class=NdsClient)]
impl BrowserClient {
    #[wasm_bindgen(constructor)]
    pub fn new(endpoint: String) -> Result<Self, JsError> {
        Ok(Self {
            inner: NdsClient::new(endpoint).map_err(client_error)?,
        })
    }
    pub fn snapshot(&self) -> Result<Ts<View>, JsError> {
        present(self.inner.snapshot())
    }
    pub fn delivery_snapshot(&self) -> Result<Ts<DeliveryView>, JsError> {
        present(self.inner.delivery_snapshot())
    }
    pub fn lifecycle(&self, state: Ts<ClientLifecycle>) -> Result<bool, JsError> {
        let state = enum_input(state)?;
        self.inner.set_lifecycle(state);
        telemetry::record_client_lifecycle(state);
        Ok(crate::api::identity::client_work_permitted(state))
    }
    pub fn cancel(&self) {
        self.inner.cancel();
    }
    pub async fn refresh(&self) -> Result<Ts<View>, JsError> {
        present(self.inner.refresh().await.map_err(client_error)?)
    }
    pub async fn reconcile(&self) -> Result<Ts<View>, JsError> {
        present(self.inner.reconcile().await.map_err(client_error)?)
    }
    pub async fn poll_passkey(&self) -> Result<Ts<View>, JsError> {
        present(self.inner.poll_passkey().await.map_err(client_error)?)
    }
    pub async fn poll_github(&self) -> Result<Ts<View>, JsError> {
        present(self.inner.poll_github().await.map_err(client_error)?)
    }
    pub async fn logout(&self) -> Result<Ts<View>, JsError> {
        present(self.inner.logout().await.map_err(client_error)?)
    }
    pub async fn refresh_devices(&self) -> Result<Ts<View>, JsError> {
        present(self.inner.refresh_devices().await.map_err(client_error)?)
    }
    pub async fn import_browser_invitation(&self) -> Result<Ts<View>, JsError> {
        present(
            self.inner
                .import_browser_invitation()
                .await
                .map_err(client_error)?,
        )
    }
    pub async fn request_email(&self, email: String, locale: String) -> Result<Ts<View>, JsError> {
        present(
            self.inner
                .request_email(email, locale)
                .await
                .map_err(client_error)?,
        )
    }
    pub async fn verify_email(&self, code: String) -> Result<Ts<View>, JsError> {
        present(self.inner.verify_email(code).await.map_err(client_error)?)
    }
    pub async fn start_github(&self, locale: String) -> Result<Ts<View>, JsError> {
        present(
            self.inner
                .start_github(locale)
                .await
                .map_err(client_error)?,
        )
    }
    pub async fn start_passkey(
        &self,
        register: js_sys::Boolean,
        email: Option<String>,
        locale: String,
    ) -> Result<Ts<View>, JsError> {
        present(
            self.inner
                .start_passkey(boolean_input(register)?, email, locale)
                .await
                .map_err(client_error)?,
        )
    }
    pub async fn revoke_passkey(&self, id: String) -> Result<Ts<View>, JsError> {
        present(self.inner.revoke_passkey(id).await.map_err(client_error)?)
    }
    pub async fn prepare_device(&self, name: String) -> Result<Ts<View>, JsError> {
        present(
            self.inner
                .prepare_device(name)
                .await
                .map_err(client_error)?,
        )
    }
    pub async fn revoke_device(&self, id: String) -> Result<Ts<View>, JsError> {
        present(self.inner.revoke_device(id).await.map_err(client_error)?)
    }
    pub async fn passkeys(&self) -> Result<Vec<Ts<PasskeyView>>, JsError> {
        self.inner
            .passkeys()
            .await
            .map_err(client_error)?
            .into_iter()
            .map(present)
            .collect()
    }
    pub async fn organizations(&self) -> Result<Vec<Ts<OrganizationOverviewView>>, JsError> {
        self.inner
            .organizations()
            .await
            .map_err(client_error)?
            .into_iter()
            .map(present)
            .collect()
    }
    pub async fn select_organization(
        &self,
        company: String,
        membership_revision: js_sys::BigInt,
    ) -> Result<BrowserOrganization, JsError> {
        Ok(BrowserOrganization {
            inner: self
                .inner
                .select_organization(company, revision(membership_revision)?)
                .await
                .map_err(client_error)?,
        })
    }
    pub async fn create_organization(
        &self,
        name: String,
    ) -> Result<BrowserCompanyCommand, JsError> {
        Ok(BrowserCompanyCommand {
            inner: self
                .inner
                .create_organization(name)
                .await
                .map_err(client_error)?,
        })
    }
    pub async fn incidents(&self, after: String) -> Result<Ts<IncidentPage>, JsError> {
        present(self.inner.incidents(after).await.map_err(client_error)?)
    }
    pub async fn incident_action(
        &self,
        id: String,
        expected: js_sys::BigInt,
        silence_seconds: js_sys::Number,
    ) -> Result<Ts<IncidentView>, JsError> {
        present(
            self.inner
                .incident_action(id, revision(expected)?, unsigned_number(silence_seconds)?)
                .await
                .map_err(client_error)?,
        )
    }
    pub fn cancel_github_catalog(&self) {
        self.inner.cancel_github_catalog();
    }
    pub async fn github_catalog_source(&self) -> Result<Ts<GithubSourceView>, JsError> {
        present(
            self.inner
                .github_catalog_source()
                .await
                .map_err(catalog_error)?,
        )
    }
    pub async fn clear_github_catalog_source(
        &self,
        expected_revision: String,
        expected_pending: Option<String>,
    ) -> Result<(), JsError> {
        self.inner
            .clear_github_catalog_source(expected_revision, expected_pending)
            .await
            .map_err(catalog_error)
    }
    pub async fn select_github_credential(
        &self,
        record_ref: String,
        revision: String,
        pending_id: Option<String>,
        expected_source_revision: String,
        expected_source_pending: Option<String>,
    ) -> Result<(), JsError> {
        self.inner
            .select_github_credential(
                record_ref,
                revision,
                pending_id,
                expected_source_revision,
                expected_source_pending,
            )
            .await
            .map_err(catalog_error)
    }
    pub async fn github_catalog_credentials(
        &self,
        after: String,
    ) -> Result<Ts<GithubCredentialReferencePage>, JsError> {
        present(
            self.inner
                .github_catalog_credentials(after)
                .await
                .map_err(catalog_error)?,
        )
    }
    pub async fn saved_github_repository_page(
        &self,
        next_page: js_sys::Boolean,
    ) -> Result<Ts<GithubSavedRepositoryPageView>, JsError> {
        present(
            self.inner
                .saved_github_repository_page(boolean_input(next_page)?)
                .await
                .map_err(catalog_error)?,
        )
    }
    pub async fn github_repository_page(
        &self,
        next_page: js_sys::Boolean,
    ) -> Result<Ts<GithubRepositoryPageView>, JsError> {
        present(
            self.inner
                .github_repository_page(boolean_input(next_page)?)
                .await
                .map_err(catalog_error)?,
        )
    }
    pub async fn vault_snapshot(&self) -> Result<Ts<VaultView>, JsError> {
        present(self.inner.vault_snapshot().await.map_err(vault_error)?)
    }
    pub async fn vault_refresh(&self) -> Result<Ts<VaultView>, JsError> {
        present(self.inner.vault_refresh().await.map_err(vault_error)?)
    }
    pub async fn vault_sync(&self) -> Result<Ts<VaultView>, JsError> {
        present(self.inner.vault_sync().await.map_err(vault_error)?)
    }
    pub async fn publish_device_state(&self) -> Result<Ts<VaultView>, JsError> {
        present(
            self.inner
                .publish_device_state()
                .await
                .map_err(vault_error)?,
        )
    }
    pub async fn vault_records(&self, after: String) -> Result<Ts<RecordPage>, JsError> {
        present(self.inner.vault_records(after).await.map_err(vault_error)?)
    }
    pub async fn sync_states(&self, after: String) -> Result<Ts<StatePage>, JsError> {
        present(self.inner.sync_states(after).await.map_err(vault_error)?)
    }
    pub async fn vault_save(
        &self,
        id: Option<String>,
        expected: js_sys::BigInt,
        pending: Option<String>,
        title: String,
        username: String,
        secret: String,
    ) -> Result<Ts<VaultView>, JsError> {
        let mut secret = zeroize::Zeroizing::new(secret);
        let expected = revision(expected)?;
        let value = std::mem::take(&mut *secret);
        present(
            self.inner
                .vault_save(id, expected, pending, title, username, value)
                .await
                .map_err(vault_error)?,
        )
    }
    pub async fn vault_delete(
        &self,
        id: String,
        expected: js_sys::BigInt,
        pending: Option<String>,
    ) -> Result<Ts<VaultView>, JsError> {
        present(
            self.inner
                .vault_delete(id, revision(expected)?, pending)
                .await
                .map_err(vault_error)?,
        )
    }
    pub async fn vault_resolve(
        &self,
        id: String,
        expected: js_sys::BigInt,
        pending: Option<String>,
        cursor: js_sys::BigInt,
        use_conflict: js_sys::Boolean,
    ) -> Result<Ts<VaultView>, JsError> {
        present(
            self.inner
                .vault_resolve(
                    id,
                    revision(expected)?,
                    pending,
                    revision(cursor)?,
                    boolean_input(use_conflict)?,
                )
                .await
                .map_err(vault_error)?,
        )
    }
    pub async fn device_modules(&self) -> Result<Vec<Ts<DeviceModuleView>>, JsError> {
        self.inner
            .device_modules()
            .await
            .map_err(client_error)?
            .into_iter()
            .map(present)
            .collect()
    }
    pub async fn configure_module(
        &self,
        module: String,
        enabled: js_sys::Boolean,
        permissions: js_sys::Array<js_sys::JsString>,
        expected_revision: js_sys::BigInt,
        pending_id: Option<String>,
    ) -> Result<(), JsError> {
        if permissions.length() > 16 {
            return Err(JsError::new("invalid_input"));
        }
        let permissions = permissions
            .iter()
            .map(|v| v.as_string().ok_or_else(|| JsError::new("invalid_input")))
            .collect::<Result<Vec<_>, _>>()?;
        self.inner
            .configure_module(
                module,
                boolean_input(enabled)?,
                permissions,
                revision(expected_revision)?,
                pending_id,
            )
            .await
            .map_err(client_error)
    }
    pub async fn clipboard_snapshot(&self) -> Result<Ts<ClipboardView>, JsError> {
        present(
            self.inner
                .clipboard_snapshot()
                .await
                .map_err(client_error)?,
        )
    }
    pub async fn clipboard_publish_text(&self, text: String) -> Result<String, JsError> {
        self.inner
            .clipboard_publish_text(text)
            .await
            .map_err(client_error)
    }
    pub fn reserve_clipboard_copy(&self) -> Result<Option<String>, JsError> {
        self.inner.reserve_clipboard_copy().map_err(vault_error)
    }
    pub async fn clipboard_copy(&self, ticket: Option<String>) -> Result<(), JsError> {
        self.inner
            .clipboard_copy(ticket)
            .await
            .map_err(client_error)
    }
    pub async fn vault_copy_secret(
        &self,
        id: String,
        ticket: Option<String>,
    ) -> Result<(), JsError> {
        self.inner
            .vault_copy_secret(id, ticket)
            .await
            .map_err(vault_error)
    }
}
#[wasm_bindgen(js_name=Organization)]
pub struct BrowserOrganization {
    inner: LocalOrganization,
}
#[wasm_bindgen(js_class=Organization)]
impl BrowserOrganization {
    pub async fn overview(&self) -> Result<Ts<OrganizationOverviewView>, JsError> {
        present(self.inner.overview().await.map_err(client_error)?)
    }
    pub async fn members(
        &self,
        after: Option<String>,
    ) -> Result<Ts<OrganizationMemberPageView>, JsError> {
        present(self.inner.members(after).await.map_err(client_error)?)
    }
    pub async fn invitations(
        &self,
        after: Option<String>,
    ) -> Result<Ts<OrganizationInvitationPageView>, JsError> {
        present(self.inner.invitations(after).await.map_err(client_error)?)
    }
    pub async fn invite(
        &self,
        email: String,
        role: Ts<OrganizationRoleView>,
    ) -> Result<BrowserCompanyCommand, JsError> {
        Ok(BrowserCompanyCommand {
            inner: self
                .inner
                .invite(email, enum_input(role)?)
                .await
                .map_err(client_error)?,
        })
    }
    pub async fn revoke(&self, id: String) -> Result<BrowserCompanyCommand, JsError> {
        Ok(BrowserCompanyCommand {
            inner: self.inner.revoke(id).await.map_err(client_error)?,
        })
    }
    pub async fn change_member(
        &self,
        user: String,
        expected: js_sys::BigInt,
        role: Option<Ts<OrganizationRoleView>>,
    ) -> Result<BrowserCompanyCommand, JsError> {
        Ok(BrowserCompanyCommand {
            inner: self
                .inner
                .change_member(user, revision(expected)?, role.map(enum_input).transpose()?)
                .await
                .map_err(client_error)?,
        })
    }
    pub fn close(&self) {
        self.inner.close();
    }
}
#[wasm_bindgen(js_name=CompanyCommand)]
pub struct BrowserCompanyCommand {
    inner: CompanyCommand,
}
#[wasm_bindgen(js_class=CompanyCommand)]
impl BrowserCompanyCommand {
    pub fn is_current(&self) -> bool {
        self.inner.is_current()
    }
    pub async fn execute(&self) -> Result<(), JsError> {
        self.inner.execute().await.map_err(client_error)
    }
}
