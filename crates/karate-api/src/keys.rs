//! High-entropy credentials: never serialize the stored verifier or log secrets.
use crate::{
    identity::unix_now,
    membership::storage,
    service::{ProjectService, authorize},
};
use aws_lc_rs::{
    constant_time::verify_slices_are_equal,
    digest::{SHA256, digest},
    rand,
};
use deadpool_postgres::Object;
use karate_core::{
    Error,
    authorization::{Action, Identity, KeyRestriction},
    validate_name,
};
use serde::{Deserialize, Serialize};
use tokio_postgres::{IsolationLevel, Row};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyInput {
    pub key_id: Uuid,
    pub name: String,
    pub scopes: Vec<Action>,
    pub expires_at: i64,
}
#[derive(Debug, Serialize)]
pub struct ApiKey {
    pub key_id: Uuid,
    pub name: String,
    pub issuer: String,
    pub subject: String,
    pub scopes: Vec<Action>,
    pub expires_at: i64,
    pub revision: i64,
    pub revoked: bool,
}
// Intentionally no Debug implementation: the one-time response contains a credential.
#[derive(Serialize)]
pub struct IssuedKey {
    pub key: ApiKey,
    pub secret: String,
}
#[derive(Debug, Serialize)]
pub struct KeyPage {
    pub items: Vec<ApiKey>,
    pub next_cursor: Option<Uuid>,
}
fn scopes(row: &Row) -> Result<Vec<Action>, Error> {
    row.try_get::<_, Vec<String>>("scopes")
        .map_err(storage)?
        .into_iter()
        .map(|s| {
            serde_json::from_value(serde_json::Value::String(s)).map_err(|_| Error::Unavailable)
        })
        .collect()
}
fn metadata(row: &Row) -> Result<ApiKey, Error> {
    Ok(ApiKey {
        key_id: row.try_get("key_id").map_err(storage)?,
        name: row.try_get("name").map_err(storage)?,
        issuer: row.try_get("issuer").map_err(storage)?,
        subject: row.try_get("subject").map_err(storage)?,
        scopes: scopes(row)?,
        expires_at: row.try_get("expires_at").map_err(storage)?,
        revision: row.try_get("revision").map_err(storage)?,
        revoked: row.try_get("revoked").map_err(storage)?,
    })
}
fn generate(tenant: Uuid, id: Uuid) -> Result<(String, Vec<u8>), Error> {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes).map_err(|_| Error::Unavailable)?;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let secret = format!("kk1.{}.{}.{hex}", tenant.simple(), id.simple());
    let verifier = digest(&SHA256, secret.as_bytes()).as_ref().to_vec();
    Ok((secret, verifier))
}
fn locator(credential: &str) -> Result<(Uuid, Uuid), Error> {
    if credential.len() != 134 {
        return Err(Error::Unauthenticated);
    }
    let parts: Vec<_> = credential.split('.').collect();
    if parts.len() != 4
        || parts[0] != "kk1"
        || parts[1].len() != 32
        || parts[2].len() != 32
        || parts[3].len() != 64
        || parts[1..].iter().any(|p| {
            !p.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        })
    {
        return Err(Error::Unauthenticated);
    }
    Ok((
        Uuid::parse_str(parts[1]).map_err(|_| Error::Unauthenticated)?,
        Uuid::parse_str(parts[2]).map_err(|_| Error::Unauthenticated)?,
    ))
}
impl ProjectService {
    pub(crate) async fn key_connection(
        &self,
        credential: &str,
    ) -> Result<(Identity, Object), Error> {
        let (tenant, id) = locator(credential)?;
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let row = client
            .query_opt("SELECT * FROM karate.key_verifier($1,$2)", &[&tenant, &id])
            .await
            .map_err(storage)?;
        let expected = match &row {
            Some(row) => row.try_get::<_, Vec<u8>>("verifier").map_err(storage)?,
            None => vec![0; 32],
        };
        verify_slices_are_equal(digest(&SHA256, credential.as_bytes()).as_ref(), &expected)
            .map_err(|_| Error::Unauthenticated)?;
        let row = row.ok_or(Error::Unauthenticated)?;
        if row.try_get::<_, bool>("revoked").map_err(storage)?
            || row.try_get::<_, i64>("expires_at").map_err(storage)? <= unix_now()?
        {
            return Err(Error::Unauthenticated);
        }
        // Membership is deliberately resolved only after credential verification.
        Ok((
            Identity {
                issuer: row.try_get("issuer").map_err(storage)?,
                subject: row.try_get("subject").map_err(storage)?,
                key_restriction: Some(KeyRestriction {
                    tenant_id: tenant,
                    scopes: scopes(&row)?,
                    membership_revision: row.try_get("membership_revision").map_err(storage)?,
                }),
            },
            client,
        ))
    }
    pub async fn create_key(
        &self,
        credential: &str,
        tenant: Uuid,
        input: &KeyInput,
    ) -> Result<IssuedKey, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        validate_name(&input.name)?;
        let now = unix_now()?;
        if input.expires_at <= now || input.expires_at > now + 7776000 {
            return Err(Error::Validation);
        }
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(storage)?;
        authorize(&tx, &identity, tenant, Action::ManageKeys)
            .await?
            .validate_delegation(&input.scopes)?;
        let (secret, verifier) = generate(tenant, input.key_id)?;
        let names: Vec<String> = input
            .scopes
            .iter()
            .map(|s| {
                serde_json::to_value(s)
                    .expect("closed action enum")
                    .as_str()
                    .expect("string action enum")
                    .into()
            })
            .collect();
        let row = tx
            .query_one(
                "SELECT * FROM karate.create_key($1,$2,$3,$4,$5,$6,$7)",
                &[
                    &tenant,
                    &input.key_id,
                    &input.name,
                    &names,
                    &input.expires_at,
                    &verifier,
                    &Uuid::new_v4(),
                ],
            )
            .await
            .map_err(storage)?;
        let key = metadata(&row)?;
        authorize(&tx, &identity, tenant, Action::ManageKeys)
            .await?
            .validate_delegation(&key.scopes)?;
        tx.commit().await.map_err(storage)?;
        Ok(IssuedKey { key, secret })
    }
    pub async fn list_keys(
        &self,
        credential: &str,
        tenant: Uuid,
        limit: i64,
        after: Option<Uuid>,
    ) -> Result<KeyPage, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        if !(1..=100).contains(&limit) {
            return Err(Error::Validation);
        }
        let tx = client.transaction().await.map_err(storage)?;
        let principal = authorize(&tx, &identity, tenant, Action::ManageKeys).await?;
        let rows = tx
            .query(
                "SELECT * FROM karate.list_keys($1,$2,$3)",
                &[&tenant, &(limit + 1), &after],
            )
            .await
            .map_err(storage)?;
        let items: Vec<ApiKey> = rows
            .iter()
            .take(limit as usize)
            .map(metadata)
            .collect::<Result<_, _>>()?;
        for key in &items {
            principal.authorize_key_access(
                tenant,
                key.issuer == identity.issuer && key.subject == identity.subject,
                false,
            )?;
        }
        let next_cursor = if rows.len() > limit as usize {
            items.last().map(|k| k.key_id)
        } else {
            None
        };
        tx.commit().await.map_err(storage)?;
        Ok(KeyPage { items, next_cursor })
    }
    pub async fn rotate_key(
        &self,
        credential: &str,
        tenant: Uuid,
        id: Uuid,
        revision: i64,
    ) -> Result<IssuedKey, Error> {
        let (secret, verifier) = generate(tenant, id)?;
        let key = self
            .mutate_key(credential, tenant, id, revision, Some(verifier))
            .await?;
        Ok(IssuedKey { key, secret })
    }
    pub async fn revoke_key(
        &self,
        credential: &str,
        tenant: Uuid,
        id: Uuid,
        revision: i64,
    ) -> Result<ApiKey, Error> {
        self.mutate_key(credential, tenant, id, revision, None)
            .await
    }
    async fn mutate_key(
        &self,
        credential: &str,
        tenant: Uuid,
        id: Uuid,
        revision: i64,
        verifier: Option<Vec<u8>>,
    ) -> Result<ApiKey, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        if !(1..i64::MAX).contains(&revision) {
            return Err(Error::Validation);
        }
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(storage)?;
        authorize(&tx, &identity, tenant, Action::ManageKeys).await?;
        let row = tx
            .query_one(
                "SELECT * FROM karate.change_key($1,$2,$3,$4,$5)",
                &[&tenant, &id, &revision, &verifier, &Uuid::new_v4()],
            )
            .await
            .map_err(storage)?;
        let key = metadata(&row)?;
        let principal = authorize(&tx, &identity, tenant, Action::ManageKeys).await?;
        principal.authorize_key_access(
            tenant,
            key.issuer == identity.issuer && key.subject == identity.subject,
            verifier.is_some(),
        )?;
        if verifier.is_some() {
            principal.validate_delegation(&key.scopes)?;
        }
        tx.commit().await.map_err(storage)?;
        Ok(key)
    }
}
