//! Composition and generated presentation boundary. The original personal
//! identity and an explicitly selected organization own every late result.
use super::identity::NdsClient;
#[cfg(feature = "flutter")]
use flutter_rust_bridge::frb;
use nddev_device_sync_accounts::{ClientError, View};
use nddev_device_sync_organizations::{
    Organization, OrganizationsClient, PreparedCommand, view::*,
};
use std::sync::{Arc, Mutex};

type Slot = Arc<Mutex<OrganizationSlot>>;
#[derive(Default)]
#[cfg_attr(feature = "flutter", frb(ignore))]
pub(super) struct OrganizationSlot {
    generation: Arc<()>,
    selection: Arc<()>,
    closed: bool,
    current: Option<Arc<Selected>>,
    client: Option<Arc<OrganizationsClient>>,
}
struct Selected {
    client: Arc<OrganizationsClient>,
    organization: Organization,
    generation: Arc<()>,
    selection: Arc<()>,
}
/// A captured presentation context, never a membership grant. A personal
/// operation does not borrow a later selection; an organizational operation
/// keeps the exact selected owner even after an unrelated await.
#[cfg_attr(feature = "flutter", frb(ignore))]
#[cfg(target_os = "linux")]
pub(super) struct OrganizationObservation {
    slot: Slot,
    generation: Arc<()>,
    selected: Option<Arc<Selected>>,
}
#[cfg(target_os = "linux")]
impl OrganizationObservation {
    // The caller serializes this check with NdsClient publication.
    pub(super) fn check(&self) -> Result<(), ClientError> {
        self.slot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .check(&self.generation, self.selected.as_ref())
    }
}
impl OrganizationSlot {
    fn deselect(&mut self) {
        if let Some(owner) = self.current.take() {
            owner.organization.close();
        }
        self.selection = Arc::default();
    }
    pub(super) fn seal(&mut self) {
        self.closed = true;
        self.deselect();
        if let Some(client) = self.client.take() {
            client.close();
        }
        self.generation = Arc::default();
    }
    pub(super) fn open(&mut self) {
        self.seal();
        self.closed = false;
    }
    fn check(
        &self,
        generation: &Arc<()>,
        selected: Option<&Arc<Selected>>,
    ) -> Result<(), ClientError> {
        if self.closed || !Arc::ptr_eq(&self.generation, generation) {
            return Err(ClientError::RevalidationRequired);
        }
        if let Some(selected) = selected {
            if !Arc::ptr_eq(&self.selection, &selected.selection)
                || self
                    .current
                    .as_ref()
                    .is_none_or(|current| !Arc::ptr_eq(current, selected))
            {
                return Err(ClientError::RevalidationRequired);
            }
            selected.client.check_publication()?;
            selected.organization.check_publication()?;
        }
        Ok(())
    }
}
/// Selection handle only. close denies its captured work; it neither logs out
/// the personal account nor cancels/rolls back a submitted server command.
#[cfg_attr(feature = "flutter", frb(opaque))]
pub struct LocalOrganization {
    owner: Arc<Selected>,
    publication: Arc<Mutex<()>>,
    slot: Slot,
}
#[cfg_attr(feature = "flutter", frb(opaque))]
pub struct CompanyCommand {
    client: Arc<OrganizationsClient>,
    selected: Option<Arc<Selected>>,
    generation: Arc<()>,
    publication: Arc<Mutex<()>>,
    slot: Slot,
    prepared: PreparedCommand,
}
impl CompanyCommand {
    /// Local lifetime observation; execute still revalidates current authority.
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn is_current(&self) -> bool {
        self.check().is_ok()
    }
    /// Explicit retry always sends the original immutable command. A local
    /// publication denial never establishes absence of a remote effect.
    pub async fn execute(&self) -> Result<(), ClientError> {
        self.check()?;
        self.prepared.execute().await?;
        self.check()
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    fn check(&self) -> Result<(), ClientError> {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        let slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        slot.check(&self.generation, self.selected.as_ref())?;
        self.client.check_publication()?;
        self.prepared.check_publication()
    }
}
impl LocalOrganization {
    #[cfg_attr(feature = "flutter", frb(ignore))]
    #[cfg(target_os = "linux")]
    pub(super) fn observe_for(
        &self,
        client: &NdsClient,
    ) -> Result<OrganizationObservation, ClientError> {
        if !Arc::ptr_eq(&self.publication, &client.publication)
            || !Arc::ptr_eq(&self.slot, &client.organizations)
        {
            return Err(ClientError::RevalidationRequired);
        }
        self.publish(|| {
            Ok(OrganizationObservation {
                slot: self.slot.clone(),
                generation: self.owner.generation.clone(),
                selected: Some(self.owner.clone()),
            })
        })
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    fn publish<T>(&self, work: impl FnOnce() -> Result<T, ClientError>) -> Result<T, ClientError> {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        let slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        slot.check(&self.owner.generation, Some(&self.owner))?;
        work()
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    async fn read<T>(
        &self,
        work: impl std::future::Future<Output = Result<T, ClientError>>,
    ) -> Result<T, ClientError> {
        self.publish(|| Ok(()))?;
        let value = work.await?;
        self.publish(|| Ok(value))
    }
    /// Local lifetime observation only, never a fresh membership grant.
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn is_current(&self) -> bool {
        self.publish(|| Ok(())).is_ok()
    }
    pub async fn overview(&self) -> Result<OrganizationOverviewView, ClientError> {
        self.read(self.owner.organization.overview()).await
    }
    pub async fn members(
        &self,
        after: Option<String>,
    ) -> Result<OrganizationMemberPageView, ClientError> {
        self.read(self.owner.organization.members(after)).await
    }
    pub async fn invitations(
        &self,
        after: Option<String>,
    ) -> Result<OrganizationInvitationPageView, ClientError> {
        self.read(self.owner.organization.invitations(after)).await
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    fn command(&self, prepared: PreparedCommand) -> Result<CompanyCommand, ClientError> {
        self.publish(|| {
            prepared.check_publication()?;
            Ok(CompanyCommand {
                client: self.owner.client.clone(),
                selected: Some(self.owner.clone()),
                generation: self.owner.generation.clone(),
                publication: self.publication.clone(),
                slot: self.slot.clone(),
                prepared,
            })
        })
    }
    pub async fn invite(
        &self,
        email: String,
        role: OrganizationRoleView,
    ) -> Result<CompanyCommand, ClientError> {
        self.publish(|| Ok(()))?;
        self.command(self.owner.organization.invite(email, role).await?)
    }
    pub async fn revoke(&self, invitation: String) -> Result<CompanyCommand, ClientError> {
        self.publish(|| Ok(()))?;
        self.command(self.owner.organization.revoke(invitation).await?)
    }
    pub async fn change_member(
        &self,
        user: String,
        revision: u64,
        role: Option<OrganizationRoleView>,
    ) -> Result<CompanyCommand, ClientError> {
        self.publish(|| Ok(()))?;
        self.command(
            self.owner
                .organization
                .change_member(user, revision, role)
                .await?,
        )
    }
    #[cfg_attr(feature = "flutter", frb(sync))]
    pub fn close(&self) {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        self.owner.organization.close();
        if slot
            .current
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &self.owner))
        {
            slot.deselect();
        }
    }
}
impl Drop for LocalOrganization {
    fn drop(&mut self) {
        self.close();
    }
}
impl NdsClient {
    #[cfg_attr(feature = "flutter", frb(ignore))]
    #[cfg(target_os = "linux")]
    pub(super) fn observe_personal(&self) -> Result<OrganizationObservation, ClientError> {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        let slot = self.organizations.lock().unwrap_or_else(|e| e.into_inner());
        slot.check(&slot.generation, None)?;
        Ok(OrganizationObservation {
            slot: self.organizations.clone(),
            generation: slot.generation.clone(),
            selected: None,
        })
    }
    // The following locked helpers are called only under the existing identity
    // publication mutex. No organization/control mutex crosses any await.
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(super) fn seal_organizations_locked(&self) {
        self.organizations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .seal();
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(super) fn open_organizations_locked(&self) {
        self.organizations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .open();
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(super) fn seal_organizations(&self) {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        self.seal_organizations_locked();
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(super) fn organization_generation(&self) -> Arc<()> {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        self.organizations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .generation
            .clone()
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    pub(super) fn seal_organizations_if(&self, generation: &Arc<()>) {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        let mut slot = self.organizations.lock().unwrap_or_else(|e| e.into_inner());
        if Arc::ptr_eq(generation, &slot.generation) {
            slot.seal();
        }
    }
    #[cfg_attr(feature = "flutter", frb(ignore))]
    fn organization_client(&self) -> Result<(Arc<OrganizationsClient>, Arc<()>), ClientError> {
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        let mut slot = self.organizations.lock().unwrap_or_else(|e| e.into_inner());
        slot.check(&slot.generation, None)?;
        // A newly invoked collection/selection may capture current personal
        // authority. It never reopens an old selection or edits an old command.
        if slot
            .client
            .as_ref()
            .is_some_and(|client| client.check_publication().is_err())
        {
            slot.open();
        }
        let client = slot
            .client
            .get_or_insert_with(|| {
                Arc::new(OrganizationsClient::new(self.shared_identity().clone()))
            })
            .clone();
        Ok((client, slot.generation.clone()))
    }
    pub async fn organizations(&self) -> Result<Vec<OrganizationOverviewView>, ClientError> {
        let (client, generation) = self.organization_client()?;
        let value = client.list().await?;
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        let slot = self.organizations.lock().unwrap_or_else(|e| e.into_inner());
        slot.check(&generation, None)?;
        client.check_publication()?;
        Ok(value)
    }
    pub async fn select_organization(
        &self,
        company: String,
        membership_revision: u64,
    ) -> Result<LocalOrganization, ClientError> {
        let (client, generation) = self.organization_client()?;
        let selection = {
            let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
            let mut slot = self.organizations.lock().unwrap_or_else(|e| e.into_inner());
            slot.check(&generation, None)?;
            client.check_publication()?;
            slot.deselect();
            slot.selection.clone()
        };
        let organization = client.select(company, membership_revision).await?;
        let owner = Arc::new(Selected {
            client,
            organization,
            generation,
            selection,
        });
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        let mut slot = self.organizations.lock().unwrap_or_else(|e| e.into_inner());
        slot.check(&owner.generation, None)?;
        if !Arc::ptr_eq(&slot.selection, &owner.selection) {
            return Err(ClientError::RevalidationRequired);
        }
        owner.client.check_publication()?;
        owner.organization.check_publication()?;
        slot.current = Some(owner.clone());
        Ok(LocalOrganization {
            owner,
            publication: self.publication.clone(),
            slot: self.organizations.clone(),
        })
    }
    pub async fn create_organization(&self, name: String) -> Result<CompanyCommand, ClientError> {
        let (client, generation) = self.organization_client()?;
        let prepared = client.create(name).await?;
        let _publication = self.publication.lock().unwrap_or_else(|e| e.into_inner());
        let slot = self.organizations.lock().unwrap_or_else(|e| e.into_inner());
        slot.check(&generation, None)?;
        client.check_publication()?;
        prepared.check_publication()?;
        Ok(CompanyCommand {
            client,
            selected: None,
            generation,
            publication: self.publication.clone(),
            slot: self.organizations.clone(),
            prepared,
        })
    }
    pub async fn import_invitation(&self, link: String) -> Result<View, ClientError> {
        self.shared_identity().import_invitation(link).await
    }
    pub async fn import_browser_invitation(&self) -> Result<View, ClientError> {
        let view = self.shared_identity().view();
        if view.authenticated || view.logout_pending || !view.flow.is_empty() {
            return Ok(view);
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(link) = nddev_device_sync_accounts::invitation::take_browser_invitation() {
            return self
                .shared_identity()
                .import_invitation(link.to_string())
                .await;
        }
        Ok(self.shared_identity().view())
    }
}

/// Browser route I/O only. No URL or invitation token is returned to presentation.
#[cfg_attr(feature = "flutter", frb(sync))]
pub fn capture_invitation_link(route: String) -> Result<(), ClientError> {
    #[cfg(target_arch = "wasm32")]
    nddev_device_sync_accounts::invitation::capture_browser_invitation_route(route)?;
    #[cfg(not(target_arch = "wasm32"))]
    drop(route);
    Ok(())
}

#[cfg(test)]
mod lifetime_tests {
    use super::*;

    #[test]
    fn observed_loss_cannot_seal_a_replacement_generation() {
        // No authorization is created here. Exercise the actual local hooks
        // used to publish a replacement and to seal a captured observation.
        let client = NdsClient::new("https://example.invalid".into()).unwrap();
        let old = client.organization_generation();
        {
            let _publication = client.publication.lock().unwrap();
            client.open_organizations_locked();
        }
        let replacement = client.organization_generation();
        client.seal_organizations_if(&old);
        {
            let slot = client.organizations.lock().unwrap();
            assert!(!slot.closed);
            assert!(Arc::ptr_eq(&slot.generation, &replacement));
        }
        client.seal_organizations_if(&replacement);
        assert!(client.organizations.lock().unwrap().closed);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn captured_personal_presentation_cannot_adopt_a_reopened_generation() {
        let client = NdsClient::new("https://example.invalid".into()).unwrap();
        let captured = client.observe_personal().unwrap();
        let _publication = client.publication.lock().unwrap();
        assert!(captured.check().is_ok());
        client.organizations.lock().unwrap().deselect();
        // Closing an unrelated selected context does not change a personal
        // operation's origin, but an identity-generation replacement does.
        assert!(captured.check().is_ok());
        client.open_organizations_locked();
        assert!(matches!(
            captured.check(),
            Err(ClientError::RevalidationRequired)
        ));
    }
}

#[cfg(all(test, target_os = "linux", feature = "development-http"))]
mod acceptance {
    use super::*;
    use serde_json::Value;
    use std::time::Duration;

    async fn json(http: &reqwest::Client, url: String) -> Value {
        let bytes = http
            .get(url)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert!(bytes.len() <= 65_536);
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn sign_in(client: &NdsClient, http: &reqwest::Client, mailbox: &str, email: &str) {
        let prior = json(http, format!("{mailbox}/api/v1/messages")).await;
        client
            .request_email(email.into(), "en".into())
            .await
            .unwrap();
        let code = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let messages = json(http, format!("{mailbox}/api/v1/messages")).await;
                for row in messages["messages"].as_array().unwrap() {
                    if prior["messages"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|old| old["ID"] == row["ID"])
                    {
                        continue;
                    }
                    let message = json(
                        http,
                        format!("{mailbox}/api/v1/message/{}", row["ID"].as_str().unwrap()),
                    )
                    .await;
                    if !message["To"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|to| to["Address"] == email)
                    {
                        continue;
                    }
                    if let Some((_, code)) = message["Text"].as_str().unwrap().split_once("code: ")
                    {
                        return code.chars().take(8).collect::<String>();
                    }
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("owned SMTP did not deliver the sign-in code");
        assert!(code.len() == 8 && code.bytes().all(|b| b.is_ascii_digit()));
        assert!(client.verify_email(code).await.unwrap().authenticated);
        // Exercise the same Accounts enrollment proof without starting an
        // unrelated native replica, screen, browser or discovery owner.
        client
            .shared_identity()
            .prepare_device("Owned multispace composition".into())
            .await
            .unwrap();
    }

    #[test]
    #[ignore = "requires owned loopback PG/SMTP and PID-verified isolated native credentials"]
    fn actual_multispace_publication_preserves_personal_owner_and_key() {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                tokio::time::timeout(Duration::from_secs(240), async {
                    let test_panic_hook = std::panic::take_hook();
                    super::super::telemetry::init_app().unwrap();
                    // Keep the test harness's failure location. Key material is
                    // compared only with boolean assertions, never Debug output.
                    std::panic::set_hook(test_panic_hook);
                    let receipt = std::env::var("NDS_FIXTURE_RECEIPT").unwrap();
                    let fixture: Value =
                        serde_json::from_slice(&std::fs::read(receipt).unwrap()).unwrap();
                    let endpoint = fixture["base_url"].as_str().unwrap();
                    let mailbox = fixture["mailpit_url"].as_str().unwrap();
                    let email = fixture["owner_email"].as_str().unwrap();
                    assert!(endpoint.starts_with("http://127.0.0.1:"));
                    assert!(mailbox.starts_with("http://127.0.0.1:"));
                    assert!(email.ends_with("@example.invalid"));
                    assert!(
                        fixture["expires_at_unix"].as_f64().unwrap()
                            > time::OffsetDateTime::now_utc().unix_timestamp() as f64 + 120.0
                    );
                    // Accounts owns the selected native TLS provider. Follow
                    // application construction before this fixture-only client.
                    let client = NdsClient::new(endpoint.into()).unwrap();
                    let http = reqwest::Client::builder()
                        .no_proxy()
                        .timeout(Duration::from_secs(5))
                        .build()
                        .unwrap();
                    assert!(!client.refresh().await.unwrap().authenticated);
                    sign_in(&client, &http, mailbox, email).await;
                    let personal = client.shared_identity().view();
                    let key = client
                        .shared_identity()
                        .ensure_vault_key(None)
                        .await
                        .unwrap();
                    // One real owning Vault sync performs its protected key
                    // delivery. No replica loop is started, so the later local
                    // operation stays pending throughout this scope test.
                    assert!(
                        client
                            .vault()
                            .await
                            .unwrap()
                            .synchronize()
                            .await
                            .unwrap()
                            .unlocked
                    );
                    let staged = client
                        .vault_save(
                            None,
                            0,
                            None,
                            "Personal pending record".into(),
                            "Synthetic identity".into(),
                            "SYNTHETIC_MULTISPACE_SECRET".into(),
                        )
                        .await
                        .unwrap();
                    assert!(staged.unlocked && staged.pending == 1);
                    let retained_record = client
                        .vault_records(String::new())
                        .await
                        .unwrap()
                        .items
                        .into_iter()
                        .find(|row| row.title == "Personal pending record")
                        .unwrap();
                    assert!(retained_record.pending_id.is_some());
                    let first = client.organizations().await.unwrap();
                    assert_eq!(first.len(), 1);
                    let create = client
                        .create_organization("Second composition space".into())
                        .await
                        .unwrap();
                    create.execute().await.unwrap();
                    create.execute().await.unwrap();
                    let choices = client.organizations().await.unwrap();
                    assert_eq!(choices.len(), 2);
                    let second = choices
                        .iter()
                        .find(|row| row.company_id != first[0].company_id)
                        .unwrap();
                    let old = client
                        .select_organization(first[0].company_id.clone(), first[0].member.revision)
                        .await
                        .unwrap();
                    let original_presentation = old.observe_for(&client).unwrap();
                    let stale_command = old
                        .invite(
                            "never-sent@example.invalid".into(),
                            OrganizationRoleView::User,
                        )
                        .await
                        .unwrap();
                    let (joined, wait_joined) = tokio::sync::oneshot::channel();
                    let (release, wait_release) = tokio::sync::oneshot::channel();
                    // Hold an actual authorized provider value at the same
                    // production read boundary the public methods use.
                    let held = old.read(async {
                        let value = old.owner.organization.overview().await?;
                        joined.send(()).unwrap();
                        wait_release.await.unwrap();
                        Ok(value)
                    });
                    let replace = async {
                        wait_joined.await.unwrap();
                        let replacement = client
                            .select_organization(second.company_id.clone(), second.member.revision)
                            .await
                            .unwrap();
                        release.send(()).unwrap();
                        replacement
                    };
                    let (late, replacement) = tokio::join!(held, replace);
                    {
                        let _publication = client.publication.lock().unwrap();
                        assert!(matches!(
                            original_presentation.check(),
                            Err(ClientError::RevalidationRequired)
                        ));
                    }
                    assert!(matches!(late, Err(ClientError::RevalidationRequired)));
                    assert!(!old.is_current());
                    assert!(!stale_command.is_current());
                    assert!(matches!(
                        stale_command.execute().await,
                        Err(ClientError::RevalidationRequired)
                    ));
                    old.close();
                    drop(old);
                    assert!(replacement.is_current());
                    assert_eq!(
                        replacement.overview().await.unwrap().company_id,
                        second.company_id
                    );
                    assert!(
                        replacement
                            .invitations(None)
                            .await
                            .unwrap()
                            .invitations
                            .is_empty()
                    );
                    let after = client.shared_identity().refresh_devices().await.unwrap();
                    assert_eq!(after.user_id, personal.user_id);
                    assert_eq!(after.tenant_id, personal.tenant_id);
                    assert_eq!(
                        after
                            .devices
                            .iter()
                            .find(|d| d.is_current)
                            .unwrap()
                            .device_id,
                        personal
                            .devices
                            .iter()
                            .find(|d| d.is_current)
                            .unwrap()
                            .device_id
                    );
                    let retained = client
                        .shared_identity()
                        .ensure_vault_key(None)
                        .await
                        .unwrap();
                    assert_eq!(retained.id(), key.id());
                    assert!(retained.expose() == key.expose());
                    let after_switch = client
                        .vault_records(String::new())
                        .await
                        .unwrap()
                        .items
                        .into_iter()
                        .find(|row| row.id == retained_record.id)
                        .unwrap();
                    assert_eq!(after_switch.title, retained_record.title);
                    assert_eq!(after_switch.revision, retained_record.revision);
                    assert_eq!(after_switch.pending_id, retained_record.pending_id);
                    let captured = client
                        .create_organization("Cancelled composition command".into())
                        .await
                        .unwrap();
                    client.cancel();
                    assert!(!replacement.is_current());
                    assert!(!captured.is_current());
                    assert!(captured.execute().await.is_err());
                    assert!(client.organizations().await.is_err());
                    client.shutdown().await.unwrap();
                    let reopened = NdsClient::new(endpoint.into()).unwrap();
                    assert!(reopened.refresh().await.unwrap().authenticated);
                    reopened
                        .shared_identity()
                        .prepare_device("Owned multispace composition".into())
                        .await
                        .unwrap();
                    assert_eq!(reopened.organizations().await.unwrap().len(), 2);
                    let retained = reopened
                        .shared_identity()
                        .ensure_vault_key(None)
                        .await
                        .unwrap();
                    assert_eq!(retained.id(), key.id());
                    assert!(retained.expose() == key.expose());
                    let after_reopen = reopened
                        .vault_records(String::new())
                        .await
                        .unwrap()
                        .items
                        .into_iter()
                        .find(|row| row.id == retained_record.id)
                        .unwrap();
                    assert_eq!(after_reopen.title, retained_record.title);
                    assert_eq!(after_reopen.revision, retained_record.revision);
                    assert_eq!(after_reopen.pending_id, retained_record.pending_id);
                    let live = reopened
                        .select_organization(second.company_id.clone(), second.member.revision)
                        .await
                        .unwrap();
                    client.cancel();
                    assert!(live.overview().await.unwrap().member.active);
                    live.close();
                    reopened.shutdown().await.unwrap();
                    super::super::telemetry::shutdown_telemetry().await.unwrap();
                })
                .await
                .expect("owned multispace composition exceeded its finite deadline");
            });
    }
}
