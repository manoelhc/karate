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
        let result = row(&tx
            .query_one(
                "SELECT * FROM karate.provision_tenant($1,$2)",
                &[&tenant, &grant],
            )
            .await
            .map_err(storage)?)?;
        member_principal(&tx, &identity, tenant)
            .await?
            .authorize(tenant, Action::DeleteTenant)?;
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
        authorize_recovery(&tx, &identity, tenant).await?;
        let result = locked(&tx, tenant, false).await?;
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
        let result = row(&tx
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
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
}
