//! Composition and generated presentation boundary only.
use super::identity::NdsClient;
#[cfg(feature = "flutter")]
use flutter_rust_bridge::frb;
use nddev_device_sync_accounts::{ClientError, View};
use nddev_device_sync_organizations::{OrganizationsClient, PreparedCommand, view::*};

#[cfg_attr(feature = "flutter", frb(opaque))]
pub struct CompanyCommand {
    client: OrganizationsClient,
    prepared: PreparedCommand,
}
impl CompanyCommand {
    /// Every retry sends the original immutable command with fresh device proof.
    pub async fn execute(&self) -> Result<(), ClientError> {
        self.client.execute(&self.prepared).await.map(|_| ())
    }
}
impl NdsClient {
    pub async fn company(&self) -> Result<OrganizationOverviewView, ClientError> {
        OrganizationsClient::new(self.shared_identity().clone())
            .overview()
            .await
    }
    pub async fn company_members(
        &self,
        after: Option<String>,
    ) -> Result<OrganizationMemberPageView, ClientError> {
        OrganizationsClient::new(self.shared_identity().clone())
            .members(after)
            .await
    }
    pub async fn company_invitations(
        &self,
        after: Option<String>,
    ) -> Result<OrganizationInvitationPageView, ClientError> {
        OrganizationsClient::new(self.shared_identity().clone())
            .invitations(after)
            .await
    }
    pub async fn company_invite(
        &self,
        email: String,
        role: OrganizationRoleView,
    ) -> Result<CompanyCommand, ClientError> {
        let client = OrganizationsClient::new(self.shared_identity().clone());
        let prepared = client.invite(email, role).await?;
        Ok(CompanyCommand { client, prepared })
    }
    pub async fn company_revoke(&self, invitation: String) -> Result<CompanyCommand, ClientError> {
        let client = OrganizationsClient::new(self.shared_identity().clone());
        let prepared = client.revoke(invitation).await?;
        Ok(CompanyCommand { client, prepared })
    }
    pub async fn company_change_member(
        &self,
        user: String,
        revision: u64,
        role: Option<OrganizationRoleView>,
    ) -> Result<CompanyCommand, ClientError> {
        let client = OrganizationsClient::new(self.shared_identity().clone());
        let prepared = client.change_member(user, revision, role).await?;
        Ok(CompanyCommand { client, prepared })
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
