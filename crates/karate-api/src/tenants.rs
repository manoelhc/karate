//! Explicit operator grants and tenant lifecycle; no first-login ownership.
use crate::{
    membership::storage,
    service::{ProjectService, member_principal, set_context},
};
use deadpool_postgres::Transaction;
use karate_core::{
    Error,
    authorization::{Action, Identity, Principal},
    tenant::{self, TenantState},
};
use serde::{Deserialize, Serialize};
use tokio_postgres::{IsolationLevel, Row};
use uuid::Uuid;
#[derive(Debug, Serialize)]
pub struct Tenant {
    pub tenant_id: Uuid,
    pub name: String,
    pub state: TenantState,
    pub revision: i64,
    pub can_manage: bool,
    pub deletion: Option<Deletion>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TenantChange {
    pub name: String,
    pub state: TenantState,
    pub expected_revision: i64,
}
fn row(r: &Row) -> Result<Tenant, Error> {
    Ok(Tenant {
        tenant_id: r.try_get("tenant_id").map_err(storage)?,
        name: r.try_get("name").map_err(storage)?,
        state: r.try_get::<_, String>("state").map_err(storage)?.parse()?,
        revision: r.try_get("revision").map_err(storage)?,
        can_manage: false,
        deletion: None,
    })
}
pub(crate) async fn locked(
    tx: &Transaction<'_>,
    tenant: Uuid,
    exclusive: bool,
) -> Result<Tenant, Error> {
    row(&tx
        .query_opt(
            "SELECT * FROM karate.lock_tenant($1,$2)",
            &[&tenant, &exclusive],
        )
        .await
        .map_err(storage)?
        .ok_or(Error::Forbidden)?)
}
pub(crate) async fn require_active(tx: &Transaction<'_>, tenant: Uuid) -> Result<(), Error> {
    locked(tx, tenant, false).await?.state.require_active()
}
/// Recovery is limited to current owner when suspended; no data authority is granted.
pub(crate) async fn authorize_recovery(
    tx: &Transaction<'_>,
    identity: &Identity,
    tenant: Uuid,
) -> Result<Principal, Error> {
    if identity.key_restriction.is_some() {
        return Err(Error::Forbidden);
    }
    member_principal(tx, identity, tenant).await?;
    let info = locked(tx, tenant, false).await?;
    let principal = member_principal(tx, identity, tenant).await?;
    tenant::inspect(&principal, tenant, info.state)?;
    let state: Option<String> = tx
        .query_one("SELECT karate.deletion_state($1)", &[&tenant])
        .await
        .map_err(storage)?
        .get(0);
    if state.as_deref() == Some("erasing") {
        return Err(Error::Forbidden);
    }
    Ok(principal)
}
impl ProjectService {
    pub async fn provision_tenant(
        &self,
        credential: &str,
        tenant: Uuid,
        grant: Uuid,
    ) -> Result<Tenant, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        tenant::bootstrap_identity(&identity)?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(storage)?;
        set_context(&tx, &identity, tenant).await?;
        let offer = tx
            .query_opt(
                "SELECT * FROM karate.lock_tenant_grant($1,$2)",
                &[&tenant, &grant],
            )
            .await
            .map_err(storage)?
            .ok_or(Error::Forbidden)?;
        tenant::claim_grant(
            &identity,
            &offer.try_get::<_, String>("issuer").map_err(storage)?,
            &offer.try_get::<_, String>("subject").map_err(storage)?,
            offer.try_get("expires_at").map_err(storage)?,
            crate::identity::unix_now()?,
            !offer.try_get::<_, bool>("consumed").map_err(storage)?
                && !offer.try_get::<_, bool>("revoked").map_err(storage)?,
        )?;
        let mut result = row(&tx
            .query_one(
                "SELECT * FROM karate.provision_tenant($1,$2)",
                &[&tenant, &grant],
            )
            .await
            .map_err(storage)?)?;
        member_principal(&tx, &identity, tenant)
            .await?
            .authorize(tenant, Action::DeleteTenant)?;
        result.can_manage = true;
        result.deletion = deletion_info(&tx, tenant).await?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
    pub async fn tenant(&self, credential: &str, tenant: Uuid) -> Result<Tenant, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(storage)?;
        let principal = authorize_recovery(&tx, &identity, tenant).await?;
        let mut result = locked(&tx, tenant, false).await?;
        result.can_manage = principal.authorize(tenant, Action::DeleteTenant).is_ok();
        if result.can_manage {
            result.deletion = deletion_info(&tx, tenant).await?;
        }
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
    pub async fn change_tenant(
        &self,
        credential: &str,
        tenant: Uuid,
        input: &TenantChange,
    ) -> Result<Tenant, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(storage)?;
        // Check before locking; read authority afresh after the exclusive lock too.
        member_principal(&tx, &identity, tenant)
            .await?
            .authorize(tenant, Action::DeleteTenant)?;
        let current = locked(&tx, tenant, true).await?;
        let principal = member_principal(&tx, &identity, tenant).await?;
        tenant::change(
            &principal,
            tenant,
            &input.name,
            input.expected_revision,
            current.revision,
        )?;
        let mut result = row(&tx
            .query_one(
                "SELECT * FROM karate.change_tenant($1,$2,$3,$4)",
                &[
                    &tenant,
                    &input.name,
                    &input.state.as_str(),
                    &input.expected_revision,
                ],
            )
            .await
            .map_err(storage)?)?;
        result.can_manage = true;
        result.deletion = deletion_info(&tx, tenant).await?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
}

#[derive(Debug, Serialize)]
pub struct Deletion {
    pub request_id: Uuid,
    pub state: String,
    pub requested_at: i64,
    pub erase_after: i64,
    pub retention_days: i32,
}
async fn deletion_info(tx: &Transaction<'_>, tenant: Uuid) -> Result<Option<Deletion>, Error> {
    tx.query_opt("SELECT * FROM karate.deletion_info($1)", &[&tenant])
        .await
        .map_err(storage)?
        .map(|r| {
            Ok(Deletion {
                request_id: r.try_get("request_id").map_err(storage)?,
                state: r.try_get("state").map_err(storage)?,
                requested_at: r.try_get("requested_at").map_err(storage)?,
                erase_after: r.try_get("erase_after").map_err(storage)?,
                retention_days: r.try_get("retention_days").map_err(storage)?,
            })
        })
        .transpose()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeletionInput {
    pub confirmation: Uuid,
    pub retention_days: i32,
    pub expected_revision: i64,
}
impl ProjectService {
    pub async fn request_deletion(
        &self,
        credential: &str,
        tenant: Uuid,
        input: &DeletionInput,
        cancel: bool,
    ) -> Result<Tenant, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(storage)?;
        member_principal(&tx, &identity, tenant)
            .await?
            .authorize(tenant, Action::DeleteTenant)?;
        let current = locked(&tx, tenant, true).await?;
        let principal = member_principal(&tx, &identity, tenant).await?;
        tenant::request_deletion(
            &principal,
            tenant,
            input.confirmation,
            input.retention_days,
            input.expected_revision,
            current.revision,
        )?;
        tx.execute(
            "SELECT karate.request_deletion($1,$2,$3,$4)",
            &[
                &tenant,
                &input.expected_revision,
                &input.retention_days,
                &cancel,
            ],
        )
        .await
        .map_err(storage)?;
        let mut result = locked(&tx, tenant, false).await?;
        result.can_manage = true;
        result.deletion = deletion_info(&tx, tenant).await?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
}
