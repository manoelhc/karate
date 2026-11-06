#![forbid(unsafe_code)]
//! One durable database batch. No tenant input, shell, network assessor or API identity.
use karate_core::Error;
use std::time::Duration;
use tokio_postgres::{NoTls, config::Host};

pub async fn run(connection: &str) -> Result<(String, i64), Error> {
    let mut config: tokio_postgres::Config = connection.parse().map_err(|_| Error::Validation)?;
    if !config.get_hostaddrs().is_empty()
        || config.get_hosts().is_empty()
        || config
            .get_hosts()
            .iter()
            .any(|h| !matches!(h, Host::Unix(_)))
    {
        return Err(Error::Validation);
    }
    config.options("-c statement_timeout=3000 -c lock_timeout=1000 -c idle_in_transaction_session_timeout=5000");
    config.connect_timeout(Duration::from_secs(2));
    let (client, connection) = config
        .connect(NoTls)
        .await
        .map_err(|_| Error::Unavailable)?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let role=client.query_one("SELECT pg_has_role(current_user,'karate_erasure_runner','MEMBER') AND NOT rolsuper AND NOT rolbypassrls AND NOT pg_has_role(current_user,'karate_erasure_guard','MEMBER') AND NOT pg_has_role(current_user,'karate_membership_guard','MEMBER') AND NOT EXISTS(SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='karate' AND c.relowner=r.oid) FROM pg_roles r WHERE rolname=current_user",&[]).await.map_err(|_|Error::Unavailable)?;
    if !role.get::<_, bool>(0) {
        return Err(Error::Forbidden);
    }
    let result = client
        .query_one("SELECT * FROM karate.erase_next_batch()", &[])
        .await
        .map_err(|_| Error::Unavailable)?;
    Ok((result.get(0), result.get(1)))
}
