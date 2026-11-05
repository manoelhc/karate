//! Identity-addressed offers: IDs locate an offer, never authenticate acceptance.
use crate::{
    identity::unix_now,
    membership::{Member, member, storage},
    service::{ProjectService, authorize, set_context},
};
use karate_core::{
    Error,
    authorization::{Action, Role, validate_invitation_acceptor, validate_invitation_grant},
};
use serde::{Deserialize, Serialize};
use tokio_postgres::{IsolationLevel, Row};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InviteInput {
    pub invitation_id: Uuid,
    pub subject: String,
    pub role: Role,
    pub expires_at: i64,
}
#[derive(Debug, Serialize)]
pub struct Invitation {
    pub invitation_id: Uuid,
    pub issuer: String,
    pub subject: String,
    pub role: Role,
    pub expires_at: i64,
    pub state: String,
    #[serde(skip_serializing)]
    previous_role: Option<Role>,
}
#[derive(Debug, Serialize)]
pub struct InvitationPage {
    pub items: Vec<Invitation>,
    pub next_cursor: Option<Uuid>,
}
fn invitation(row: &Row) -> Result<Invitation, Error> {
    let expires_at = row.try_get("expires_at").map_err(storage)?;
    let mut state: String = row.try_get("state").map_err(storage)?;
    if state == "pending" && expires_at <= unix_now()? {
        state = "expired".into();
    }
    Ok(Invitation {
        invitation_id: row.try_get("invitation_id").map_err(storage)?,
        issuer: row.try_get("issuer").map_err(storage)?,
        subject: row.try_get("subject").map_err(storage)?,
        role: row.try_get::<_, String>("role").map_err(storage)?.parse()?,
        expires_at,
        state,
        previous_role: row
            .try_get::<_, Option<String>>("previous_role")
            .map_err(storage)?
            .map(|r| r.parse())
            .transpose()?,
    })
}
impl ProjectService {
    pub async fn create_invitation(
        &self,
        credential: &str,
        tenant: Uuid,
        input: &InviteInput,
    ) -> Result<Invitation, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        let now = unix_now()?;
        if input.subject.is_empty()
            || input.subject.len() > 255
            || input.expires_at <= now
            || input.expires_at > now + 604800
        {
            return Err(Error::Validation);
        }
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(storage)?;
        authorize(&tx, &identity, tenant, Action::ManageMembers)
            .await?
            .authorize_invitation(tenant, None, input.role)?;
        let role = match input.role {
            Role::Owner => "owner",
            Role::Admin => "admin",
            Role::Member => "member",
            Role::Viewer => "viewer",
        };
        let row = tx
            .query_one(
                "SELECT * FROM karate.create_invitation($1,$2,$3,$4,$5,$6)",
                &[
                    &tenant,
                    &input.invitation_id,
                    &input.subject,
                    &role,
                    &input.expires_at,
                    &Uuid::new_v4(),
                ],
            )
            .await
            .map_err(storage)?;
        let result = invitation(&row)?;
        // Validate locked recipient facts before making the offer durable.
        authorize(&tx, &identity, tenant, Action::ManageMembers)
            .await?
            .authorize_invitation(tenant, result.previous_role, result.role)?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
    pub async fn list_invitations(
        &self,
        credential: &str,
        tenant: Uuid,
        limit: i64,
        after: Option<Uuid>,
    ) -> Result<InvitationPage, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        if !(1..=100).contains(&limit) {
            return Err(Error::Validation);
        }
        let tx = client.transaction().await.map_err(storage)?;
        authorize(&tx, &identity, tenant, Action::ManageMembers).await?;
        let rows = tx
            .query(
                "SELECT * FROM karate.list_invitations($1,$2,$3)",
                &[&tenant, &(limit + 1), &after],
            )
            .await
            .map_err(storage)?;
        let items: Vec<Invitation> = rows
            .iter()
            .take(limit as usize)
            .map(invitation)
            .collect::<Result<_, _>>()?;
        let next_cursor = if rows.len() > limit as usize {
            items.last().map(|i| i.invitation_id)
        } else {
            None
        };
        tx.commit().await.map_err(storage)?;
        Ok(InvitationPage { items, next_cursor })
    }
    pub async fn revoke_invitation(
        &self,
        credential: &str,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Invitation, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(storage)?;
        authorize(&tx, &identity, tenant, Action::ManageMembers).await?;
        let row = tx
            .query_one(
                "SELECT * FROM karate.revoke_invitation($1,$2,$3)",
                &[&tenant, &id, &Uuid::new_v4()],
            )
            .await
            .map_err(storage)?;
        let result = invitation(&row)?;
        authorize(&tx, &identity, tenant, Action::ManageMembers)
            .await?
            .authorize_invitation(tenant, result.previous_role, result.role)?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
    pub async fn accept_invitation(
        &self,
        credential: &str,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Member, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        // Service keys cannot enroll, even if bound to an invited subject.
        validate_invitation_acceptor(&identity, &identity.issuer, &identity.subject)?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(storage)?;
        set_context(&tx, &identity, tenant).await?;
        let row=tx.query_one("SELECT (r.enrolled).membership_id,(r.enrolled).issuer,(r.enrolled).subject,(r.enrolled).role,(r.enrolled).revision,(r.enrolled).revoked_at IS NOT NULL AS revoked,floor(extract(epoch FROM (r.enrolled).expires_at))::bigint AS expires_at,r.sponsor_role,r.previous_role FROM karate.accept_invitation($1,$2,$3) r",&[&tenant,&id,&Uuid::new_v4()]).await.map_err(storage)?;
        let result = member(&row)?;
        validate_invitation_acceptor(&identity, &result.issuer, &result.subject)?;
        validate_invitation_grant(
            row.try_get::<_, String>("sponsor_role")
                .map_err(storage)?
                .parse()?,
            row.try_get::<_, Option<String>>("previous_role")
                .map_err(storage)?
                .map(|r| r.parse())
                .transpose()?,
            result.role,
        )?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
}
