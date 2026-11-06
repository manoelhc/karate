//! Shared application service. HTTP/MCP/voice must never query customer tables directly.
use crate::{discovery::AccessVerifier, identity::unix_now};
use deadpool_postgres::{Manager, Object, Pool, Runtime, Timeouts, Transaction};
use karate_core::{
    Error,
    authorization::{Action, Identity, Membership, Principal, authorize_membership},
    validate_name,
};
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio_postgres::{NoTls, Row, config::Host};
use uuid::Uuid;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Project {
    pub project_id: Uuid,
    pub name: String,
    pub revision: i64,
}

#[derive(Debug, Serialize)]
pub struct ProjectPage {
    pub items: Vec<Project>,
    pub next_cursor: Option<Uuid>,
}

pub struct ProjectService {
    pub(crate) browser: Option<crate::browser::BrowserProvider>,
    verifier: AccessVerifier,
    pub(crate) pool: Pool,
}

fn unavailable<T>(_: T) -> Error {
    Error::Unavailable
}

fn project(row: &Row) -> Result<Project, Error> {
    Ok(Project {
        project_id: row.try_get("project_id").map_err(unavailable)?,
        name: row.try_get("name").map_err(unavailable)?,
        revision: row.try_get("revision").map_err(unavailable)?,
    })
}

impl ProjectService {
    pub async fn connect(
        verifier: impl Into<AccessVerifier>,
        connection: &str,
    ) -> Result<Arc<Self>, Error> {
        let mut config: tokio_postgres::Config = connection.parse().map_err(unavailable)?;
        if !config.get_hostaddrs().is_empty()
            || config.get_hosts().is_empty()
            || config
                .get_hosts()
                .iter()
                .any(|host| !matches!(host, Host::Unix(_)))
        {
            return Err(Error::Validation);
        }
        config.options("-c statement_timeout=3000 -c lock_timeout=1000 -c idle_in_transaction_session_timeout=5000");
        config.connect_timeout(Duration::from_secs(2));
        let pool = Pool::builder(Manager::new(config, NoTls))
            .max_size(8)
            .runtime(Runtime::Tokio1)
            .timeouts(Timeouts {
                wait: Some(Duration::from_secs(2)),
                create: Some(Duration::from_secs(2)),
                recycle: Some(Duration::from_secs(2)),
            })
            .build()
            .map_err(unavailable)?;
        let client = pool.get().await.map_err(unavailable)?;
        let role = client.query_one("SELECT rolsuper OR rolbypassrls OR pg_has_role(current_user, 'karate_membership_guard', 'MEMBER') OR EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='karate' AND c.relowner=r.oid) AS unsafe_role FROM pg_roles r WHERE rolname=current_user", &[]).await.map_err(unavailable)?;
        if role.get::<_, bool>("unsafe_role") {
            return Err(Error::Forbidden);
        }
        drop(client);
        Ok(Arc::new(Self {
            browser: None,
            verifier: verifier.into(),
            pool,
        }))
    }

    pub(crate) async fn connection(&self, credential: &str) -> Result<(Identity, Object), Error> {
        // No DB access can occur before the cryptographic check.
        if credential.starts_with("kk1.") {
            return self.key_connection(credential).await;
        }
        if credential.starts_with("ks1.") {
            return self.session_connection(credential).await;
        }
        let identity = self.verifier.verify(credential).await?;
        let client = self.pool.get().await.map_err(unavailable)?;
        Ok((identity, client))
    }

    pub async fn create(
        &self,
        credential: &str,
        tenant: Uuid,
        id: Uuid,
        name: &str,
    ) -> Result<(Project, bool), Error> {
        let (identity, mut client) = self.connection(credential).await?;
        validate_name(name)?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(unavailable)?;
        authorize(&tx, &identity, tenant, Action::Write).await?;
        let inserted = tx.query_opt("INSERT INTO karate.projects (tenant_id,project_id,name) VALUES ($1,$2,$3) ON CONFLICT (tenant_id,project_id) DO NOTHING RETURNING project_id,name,revision", &[&tenant,&id,&name]).await.map_err(unavailable)?;
        let (result, created) = match inserted {
            Some(row) => {
                audit(&tx, tenant, &identity, id, "project.created").await?;
                (project(&row)?, true)
            }
            None => {
                let row = tx.query_opt("SELECT project_id,name,revision FROM karate.projects WHERE tenant_id=$1 AND project_id=$2", &[&tenant,&id]).await.map_err(unavailable)?.ok_or(Error::Forbidden)?;
                let existing = project(&row)?;
                if existing.name != name {
                    return Err(Error::Conflict);
                }
                (existing, false)
            }
        };
        tx.commit().await.map_err(unavailable)?;
        Ok((result, created))
    }

    pub async fn get(&self, credential: &str, tenant: Uuid, id: Uuid) -> Result<Project, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(unavailable)?;
        authorize(&tx, &identity, tenant, Action::Read).await?;
        let row = tx.query_opt("SELECT project_id,name,revision FROM karate.projects WHERE tenant_id=$1 AND project_id=$2", &[&tenant,&id]).await.map_err(unavailable)?.ok_or(Error::NotFound)?;
        let result = project(&row)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(result)
    }

    pub async fn list(
        &self,
        credential: &str,
        tenant: Uuid,
        limit: i64,
        after: Option<Uuid>,
    ) -> Result<ProjectPage, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        if !(1..=100).contains(&limit) {
            return Err(Error::Validation);
        }
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(unavailable)?;
        authorize(&tx, &identity, tenant, Action::Read).await?;
        let rows = tx.query("SELECT project_id,name,revision FROM karate.projects WHERE tenant_id=$1 AND ($2::uuid IS NULL OR project_id>$2) ORDER BY project_id LIMIT $3", &[&tenant,&after,&(limit+1)]).await.map_err(unavailable)?;
        let more = rows.len() > limit as usize;
        let items: Vec<Project> = rows
            .iter()
            .take(limit as usize)
            .map(project)
            .collect::<Result<_, _>>()?;
        let next_cursor = if more {
            items.last().map(|p| p.project_id)
        } else {
            None
        };
        tx.commit().await.map_err(unavailable)?;
        Ok(ProjectPage { items, next_cursor })
    }

    pub async fn update(
        &self,
        credential: &str,
        tenant: Uuid,
        id: Uuid,
        name: &str,
        revision: i64,
    ) -> Result<Project, Error> {
        let (identity, mut client) = self.connection(credential).await?;
        validate_name(name)?;
        if !(1..i64::MAX).contains(&revision) {
            return Err(Error::Validation);
        }
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(unavailable)?;
        authorize(&tx, &identity, tenant, Action::Write).await?;
        let row = tx.query_opt("UPDATE karate.projects SET name=$3,revision=revision+1 WHERE tenant_id=$1 AND project_id=$2 AND revision=$4 RETURNING project_id,name,revision", &[&tenant,&id,&name,&revision]).await.map_err(unavailable)?;
        let Some(row) = row else {
            let exists = tx
                .query_opt(
                    "SELECT 1 FROM karate.projects WHERE tenant_id=$1 AND project_id=$2",
                    &[&tenant, &id],
                )
                .await
                .map_err(unavailable)?
                .is_some();
            return Err(if exists {
                Error::Conflict
            } else {
                Error::NotFound
            });
        };
        audit(&tx, tenant, &identity, id, "project.updated").await?;
        let result = project(&row)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(result)
    }
}

pub(crate) async fn authorize(
    tx: &Transaction<'_>,
    identity: &Identity,
    tenant: Uuid,
    action: Action,
) -> Result<Principal, Error> {
    let principal = member_principal(tx, identity, tenant).await?;
    principal.authorize(tenant, action)?;
    crate::tenants::require_active(tx, tenant).await?;
    let principal = member_principal(tx, identity, tenant).await?;
    principal.authorize(tenant, action)?;
    Ok(principal)
}
pub(crate) async fn member_principal(
    tx: &Transaction<'_>,
    identity: &Identity,
    tenant: Uuid,
) -> Result<Principal, Error> {
    set_context(tx, identity, tenant).await?;
    let row = tx.query_opt("SELECT role,revision FROM karate.memberships WHERE tenant_id=$1 AND issuer=$2 AND subject=$3 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>statement_timestamp())", &[&tenant,&identity.issuer,&identity.subject]).await.map_err(unavailable)?.ok_or(Error::Forbidden)?;
    let role: String = row.try_get("role").map_err(unavailable)?;
    let membership = Membership {
        revision: row.try_get("revision").map_err(unavailable)?,
        issuer: identity.issuer.clone(),
        subject: identity.subject.clone(),
        tenant_id: tenant,
        role: role.parse()?,
        revoked: false,
        expires_at: None,
    };
    let principal = authorize_membership(identity.clone(), membership, tenant, unix_now()?)?;
    Ok(principal)
}

async fn audit(
    tx: &Transaction<'_>,
    tenant: Uuid,
    identity: &Identity,
    project: Uuid,
    action: &str,
) -> Result<(), Error> {
    tx.execute("INSERT INTO karate.audit_events (tenant_id,event_id,actor_issuer,actor_subject,action,project_id) VALUES ($1,$2,$3,$4,$5,$6)",
        &[&tenant,&Uuid::new_v4(),&identity.issuer,&identity.subject,&action,&project]).await.map_err(unavailable)?;
    Ok(())
}

pub(crate) async fn set_context(
    tx: &Transaction<'_>,
    identity: &Identity,
    tenant: Uuid,
) -> Result<(), Error> {
    tx.batch_execute("SET LOCAL statement_timeout='3s'; SET LOCAL lock_timeout='1s'; SET LOCAL idle_in_transaction_session_timeout='5s'").await.map_err(unavailable)?;
    tx.execute("SELECT set_config('karate.tenant_id',$1,true),set_config('karate.issuer',$2,true),set_config('karate.subject',$3,true)", &[&tenant.to_string(),&identity.issuer,&identity.subject]).await.map_err(unavailable)?;
    Ok(())
}
