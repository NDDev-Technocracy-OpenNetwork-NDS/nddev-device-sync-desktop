//! Thin presentation adapter over the retained GDS/Vault runtime.
use super::{catalog_types::*, identity::NdsClient};
#[cfg(feature = "flutter")]
use flutter_rust_bridge::frb;
use nddev_device_sync_domain::repositories::catalog::CredentialSelection;
use nddev_device_sync_gds::catalog_runtime::{CatalogClient, CatalogRuntime};
use std::sync::Arc;

fn catalog_revision(text: &str) -> Result<u64, GithubCatalogError> {
    let value = text
        .parse::<u64>()
        .map_err(|_| GithubCatalogError::InvalidInput)?;
    if value.to_string() != text || value > nddev_device_sync_domain::sync::MAX_REVISION {
        return Err(GithubCatalogError::InvalidInput);
    }
    Ok(value)
}

impl NdsClient {
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(crate) fn invalidate_catalog(&self) {
        if let Some(client) = self
            .catalog
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            client.set_active(false);
        }
    }
    pub fn cancel_github_catalog(&self) {
        if let Some(client) = self
            .catalog
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            client.cancel_github_catalog();
        }
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(crate) async fn catalog_runtime(&self) -> Result<Arc<CatalogRuntime>, GithubCatalogError> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.native_runtime()
                .ok_or(GithubCatalogError::AuthorizationRequired)?
                .catalog(CatalogRuntime::new)
                .map_err(Into::into)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let vault = self.vault().await?;
            let mut slot = self.catalog_owner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(owner) = &*slot {
                let owner: Arc<dyn std::any::Any + Send + Sync> = owner.clone();
                return owner
                    .downcast()
                    .map_err(|_| GithubCatalogError::Unavailable);
            }
            let owner = Arc::new(CatalogRuntime::new(self.shared_identity().clone(), vault));
            *slot = Some(owner.clone());
            Ok(owner)
        }
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(crate) async fn catalog_observation(
        &self,
    ) -> Option<nddev_device_sync_domain::modules::ModuleObservation> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            self.native_runtime()?
                .catalog(CatalogRuntime::new)
                .ok()?
                .observation()
                .await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let owner = self
                .catalog_owner
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()?;
            let owner: Arc<dyn std::any::Any + Send + Sync> = owner;
            owner.downcast::<CatalogRuntime>().ok()?.observation().await
        }
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    async fn catalog_session(&self) -> Result<Arc<CatalogClient>, GithubCatalogError> {
        if !self.catalog_active() {
            return Err(GithubCatalogError::Cancelled);
        }
        if let Some(client) = self
            .catalog
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            return Ok(client.clone());
        }
        let lifetime = self.shared_identity().local_use_lifetime();
        let owner = self.catalog_runtime().await?;
        let fence = self
            .shared_identity()
            .local_use_fence_from(&lifetime)
            .await?;
        let client = owner.foreground(self.shared_identity().clone());
        let mut slot = self.catalog.lock().unwrap_or_else(|e| e.into_inner());
        if !self.catalog_active() {
            return Err(GithubCatalogError::Cancelled);
        }
        self.shared_identity().check_local_use_lifetime(&fence)?;
        Ok(slot.get_or_insert(client).clone())
    }
    pub async fn github_catalog_source(&self) -> Result<GithubSourceView, GithubCatalogError> {
        super::identity::observe_operation(
            "gds",
            "gds.catalog.source.read",
            async {
                let source = self.vault().await?.github_source().await?;
                Ok(GithubSourceView {
                    credential: source.selection.map(|selection| GithubCredentialReference {
                        record_ref: selection.record_ref().into(),
                        title: String::new(),
                        username: String::new(),
                        revision: selection.revision().to_string(),
                        pending_id: selection.pending_id().map(Into::into),
                    }),
                    revision: source.revision.to_string(),
                    pending_id: source.pending_id,
                    local_revision: source.local_revision.to_string(),
                    conflicted: source.conflicted,
                })
            },
            GithubCatalogError::Unavailable,
            nddev_device_sync_gds::catalog_runtime::classify_error,
        )
        .await
    }
    pub async fn select_github_credential(
        &self,
        record_ref: String,
        revision: String,
        pending_id: Option<String>,
        expected_source_revision: String,
        expected_source_pending: Option<String>,
    ) -> Result<(), GithubCatalogError> {
        let selection =
            CredentialSelection::new(record_ref, catalog_revision(&revision)?, pending_id)?;
        self.set_catalog_source(
            Some(selection),
            expected_source_revision,
            expected_source_pending,
        )
        .await
    }
    pub async fn clear_github_catalog_source(
        &self,
        expected_revision: String,
        expected_pending: Option<String>,
    ) -> Result<(), GithubCatalogError> {
        self.set_catalog_source(None, expected_revision, expected_pending)
            .await
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    async fn set_catalog_source(
        &self,
        selection: Option<CredentialSelection>,
        revision: String,
        pending: Option<String>,
    ) -> Result<(), GithubCatalogError> {
        super::identity::observe_operation(
            "gds",
            "gds.catalog.source.write",
            async {
                let revision = catalog_revision(&revision)?;
                self.invalidate_catalog();
                self.vault()
                    .await?
                    .set_github_source(selection, revision, pending)
                    .await?;
                self.wake_replica();
                Ok(())
            },
            GithubCatalogError::Unavailable,
            nddev_device_sync_gds::catalog_runtime::classify_error,
        )
        .await
    }
    pub async fn github_catalog_credentials(
        &self,
        after: String,
    ) -> Result<GithubCredentialReferencePage, GithubCatalogError> {
        self.catalog_session()
            .await?
            .github_catalog_credentials(after)
            .await
    }
    pub async fn saved_github_repository_page(
        &self,
        next_page: bool,
    ) -> Result<GithubSavedRepositoryPageView, GithubCatalogError> {
        self.catalog_session()
            .await?
            .saved_github_repository_page(next_page)
            .await
    }
    pub async fn github_repository_page(
        &self,
        next_page: bool,
    ) -> Result<GithubRepositoryPageView, GithubCatalogError> {
        self.catalog_session()
            .await?
            .github_repository_page(next_page)
            .await
    }
}
