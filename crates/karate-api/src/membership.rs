//! Existing-member administration. No invitation or implicit provisioning path.
use crate::{
    identity::unix_now,
    service::{ProjectService, authorize},
};
use karate_core::{
    Error,
    authorization::{Action, Role},
};
use serde::Serialize;
use tokio_postgres::Row;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct Member {
    pub membership_id: Uuid,
    pub issuer: String,
    pub subject: String,
    pub role: Role,
    pub revision: i64,
    pub revoked: bool,
    pub expires_at: Option<i64>,
}
#[derive(Debug, Serialize)]
pub struct MemberPage {
    pub items: Vec<Member>,
    pub next_cursor: Option<Uuid>,
}

// Only deliberately raised storage contract codes are public. Unexpected errors
// (including audit failures) remain unavailable, without exposing SQL details.
pub(crate) fn storage(error: tokio_postgres::Error) -> Error {
    match error.code().map(|code| code.code()) {
        Some("K0001") => Error::Forbidden,
        Some("K0002") => Error::NotFound,
        Some("K0003") => Error::Conflict,
        Some("K0004") => Error::Validation,
        Some("K0005") => Error::OwnerRequired,
        _ => Error::Unavailable,
    }
}
pub(crate) fn member(row: &Row) -> Result<Member, Error> {
    Ok(Member {
        membership_id: row.try_get("membership_id").map_err(storage)?,
        issuer: row.try_get("issuer").map_err(storage)?,
        subject: row.try_get("subject").map_err(storage)?,
        role: row.try_get::<_, String>("role").map_err(storage)?.parse()?,
        revision: row.try_get("revision").map_err(storage)?,
        revoked: row.try_get("revoked").map_err(storage)?,
        expires_at: row.try_get("expires_at").map_err(storage)?,
    })
}

impl ProjectService {
    pub async fn list_members(
        &self,
        credential: &str,
        tenant: Uuid,
        limit: i64,
        after: Option<Uuid>,
    ) -> Result<MemberPage, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        if !(1..=100).contains(&limit) {
            return Err(Error::Validation);
        }
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(storage)?;
        authorize(&tx, &identity, tenant, Action::ManageMembers).await?;
        let rows=tx.query("SELECT membership_id,issuer,subject,role,revision,revoked_at IS NOT NULL AS revoked,floor(extract(epoch FROM expires_at))::bigint AS expires_at FROM karate.list_memberships($1,$2,$3)", &[&tenant,&(limit+1),&after]).await.map_err(storage)?;
        let items: Vec<Member> = rows
            .iter()
            .take(limit as usize)
            .map(member)
            .collect::<Result<_, _>>()?;
        let next_cursor = if rows.len() > limit as usize {
            items.last().map(|m| m.membership_id)
        } else {
            None
        };
        tx.commit().await.map_err(storage)?;
        Ok(MemberPage { items, next_cursor })
    }

    /// None revokes; Some changes role. Never revives an inactive membership.
    pub async fn change_member(
        &self,
        credential: &str,
        tenant: Uuid,
        id: Uuid,
        revision: i64,
        role: Option<Role>,
    ) -> Result<Member, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        if !(1..i64::MAX).contains(&revision) {
            return Err(Error::Validation);
        }
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(storage)?;
        authorize(&tx, &identity, tenant, Action::ManageMembers).await?;
        let row=tx.query_opt("SELECT (s.target).membership_id,(s.target).issuer,(s.target).subject,(s.target).role,(s.target).revision,(s.target).revoked_at IS NOT NULL AS revoked,floor(extract(epoch FROM (s.target).expires_at))::bigint AS expires_at,s.other_owner FROM karate.membership_snapshot($1,$2) s", &[&tenant,&id]).await.map_err(storage)?.ok_or(Error::NotFound)?;
        // Snapshot holds the tenant lock until commit. Resolve the actor again
        // after waiting: a concurrent demotion/revocation cannot retain power.
        let principal = authorize(&tx, &identity, tenant, Action::ManageMembers).await?;
        let target = member(&row)?;
        principal.authorize_member_change(
            tenant,
            target.role,
            role,
            row.try_get("other_owner").map_err(storage)?,
            target.expires_at.is_none(),
        )?;
        let now = unix_now()?;
        if target.revision != revision
            || target.revoked
            || target.expires_at.is_some_and(|expiry| expiry <= now)
        {
            return Err(Error::Conflict);
        }
        let role = role.map(|r| match r {
            Role::Owner => "owner",
            Role::Admin => "admin",
            Role::Member => "member",
            Role::Viewer => "viewer",
        });
        let row=tx.query_one("SELECT membership_id,issuer,subject,role,revision,revoked_at IS NOT NULL AS revoked,floor(extract(epoch FROM expires_at))::bigint AS expires_at FROM karate.change_membership($1,$2,$3,$4,$5)", &[&tenant,&id,&revision,&role,&Uuid::new_v4()]).await.map_err(storage)?;
        let result = member(&row)?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
}
