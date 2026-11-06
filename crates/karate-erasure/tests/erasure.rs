use karate_core::Error;
use std::process::Command;
use tokio_postgres::{Client, NoTls};
fn env(key: &str) -> String {
    std::env::var(key).unwrap()
}
async fn db(key: &str) -> Client {
    let (c, f) = tokio_postgres::connect(&env(key), NoTls).await.unwrap();
    tokio::spawn(async move {
        f.await.unwrap();
    });
    c
}
fn binary() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_karate-erasure"));
    c.env_remove("KARATE_ERASURE_DATABASE_URL");
    c
}
#[tokio::test]
async fn durable_bounded_erasure_isolated_authority_rollback_and_binary() {
    let mut a = db("KARATE_TEST_ADMIN_URL").await;
    let runtime = db("KARATE_TEST_DATABASE_URL").await;
    let mut runner = db("KARATE_TEST_ERASURE_URL").await;
    assert!(!binary().output().unwrap().status.success());
    for config in [
        "invalid-private-config",
        "dbname=postgres",
        "host=127.0.0.1",
        "hostaddr=127.0.0.1",
        "host=/missing-private-socket",
    ] {
        let output = binary()
            .env("KARATE_ERASURE_DATABASE_URL", config)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8(output.stderr).unwrap().contains(config));
    }
    for name in ["KARATE_TEST_ADMIN_URL", "KARATE_TEST_DATABASE_URL"] {
        assert_eq!(karate_erasure::run(&env(name)).await, Err(Error::Forbidden));
    }
    assert_eq!(
        karate_erasure::run(&env("KARATE_TEST_ERASURE_URL"))
            .await
            .unwrap(),
        ("idle".into(), 0)
    );
    assert!(
        runtime
            .query("SELECT * FROM karate.erase_next_batch()", &[])
            .await
            .is_err()
    );
    for table in [
        "tenant_deletions",
        "erasure_receipts",
        "projects",
        "memberships",
    ] {
        assert!(
            runner
                .query(&format!("SELECT * FROM karate.{table}"), &[])
                .await
                .is_err()
        );
    }
    let t:String=a.query_one("INSERT INTO karate.tenants(tenant_id,name) VALUES(gen_random_uuid(),'Erase fixture') RETURNING tenant_id::text",&[]).await.unwrap().get(0);
    let foreign:String=a.query_one("INSERT INTO karate.tenants(tenant_id,name) VALUES(gen_random_uuid(),'Keep fixture') RETURNING tenant_id::text",&[]).await.unwrap().get(0);
    a.execute("INSERT INTO karate.memberships(tenant_id,issuer,subject,role) VALUES($1::text::uuid,'https://issuer.example','eraser','owner'),($1::text::uuid,'https://issuer.example','backup','owner')",&[&t]).await.unwrap();
    a.execute("INSERT INTO karate.projects(tenant_id,project_id,name) SELECT $1::text::uuid,gen_random_uuid(),'Fixture' FROM generate_series(1,205)",&[&t]).await.unwrap();
    a.execute("INSERT INTO karate.projects(tenant_id,project_id,name) VALUES($1::text::uuid,gen_random_uuid(),'Keep')",&[&foreign]).await.unwrap();
    // Seed every dependent store, with real FK chains and identity-scoped rows.
    a.execute("INSERT INTO karate.audit_events SELECT tenant_id,gen_random_uuid(),'https://issuer.example','eraser','project.created',project_id,now() FROM karate.projects WHERE tenant_id=$1::text::uuid",&[&t]).await.unwrap();
    a.execute("INSERT INTO karate.browser_sessions(tenant_id,session_id,issuer,subject,membership_revision,verifier,expires_at) VALUES($1::text::uuid,gen_random_uuid(),'https://issuer.example','eraser',1,decode(repeat('aa',32),'hex'),2000000000)",&[&t]).await.unwrap();
    a.execute("INSERT INTO karate.session_events SELECT tenant_id,gen_random_uuid(),session_id,issuer,subject,'session.created',now() FROM karate.browser_sessions WHERE tenant_id=$1::text::uuid",&[&t]).await.unwrap();
    a.execute("INSERT INTO karate.service_keys(tenant_id,key_id,name,issuer,subject,membership_revision,scopes,expires_at,verifier) VALUES($1::text::uuid,gen_random_uuid(),'Fixture','https://issuer.example','eraser',1,ARRAY['read'],2000000000,decode(repeat('aa',32),'hex'))",&[&t]).await.unwrap();
    a.execute("INSERT INTO karate.key_events SELECT tenant_id,gen_random_uuid(),key_id,issuer,subject,'key.created',1,now() FROM karate.service_keys WHERE tenant_id=$1::text::uuid",&[&t]).await.unwrap();
    a.execute("INSERT INTO karate.invitations(tenant_id,invitation_id,issuer,subject,role,expires_at,sponsor_subject,sponsor_revision) VALUES($1::text::uuid,gen_random_uuid(),'https://issuer.example','invited','member',2000000000,'eraser',1)",&[&t]).await.unwrap();
    a.execute("INSERT INTO karate.invitation_events(tenant_id,event_id,invitation_id,actor_issuer,actor_subject,action) SELECT tenant_id,gen_random_uuid(),invitation_id,issuer,subject,'invitation.created' FROM karate.invitations WHERE tenant_id=$1::text::uuid",&[&t]).await.unwrap();
    a.execute("INSERT INTO karate.membership_events(tenant_id,event_id,membership_id,actor_issuer,actor_subject,action,old_role,new_role,revision) SELECT tenant_id,gen_random_uuid(),membership_id,issuer,subject,'membership.role_changed','admin','owner',2 FROM karate.memberships WHERE tenant_id=$1::text::uuid",&[&t]).await.unwrap();
    a.execute("INSERT INTO karate.login_flows(tenant_id,state_hash,binding_hash,nonce,pkce) VALUES($1::text::uuid,decode(repeat('aa',32),'hex'),decode(repeat('bb',32),'hex'),repeat('a',64),repeat('b',43))",&[&t]).await.unwrap();
    a.execute("INSERT INTO karate.provisioning_grants(tenant_id,grant_id,name,issuer,subject,expires_at,consumed,approval_reference) VALUES($1::text::uuid,gen_random_uuid(),'Old name','https://issuer.example','eraser',2000000000,true,'fixture')",&[&t]).await.unwrap();
    a.query_one("SELECT set_config('karate.tenant_id',$1,false),set_config('karate.issuer','https://issuer.example',false),set_config('karate.subject','eraser',false)",&[&t]).await.unwrap();
    a.execute(
        "SELECT karate.request_deletion($1::text::uuid,1,7,false)",
        &[&t],
    )
    .await
    .unwrap();
    assert_eq!(
        karate_erasure::run(&env("KARATE_TEST_ERASURE_URL"))
            .await
            .unwrap()
            .0,
        "idle"
    );
    a.execute(
        "UPDATE karate.tenant_deletions SET erase_after=0 WHERE tenant_id=$1::text::uuid",
        &[&t],
    )
    .await
    .unwrap();
    a.execute("UPDATE karate.memberships SET revoked_at=now() WHERE tenant_id=$1::text::uuid AND subject='eraser'",&[&t]).await.unwrap();
    assert_eq!(
        karate_erasure::run(&env("KARATE_TEST_ERASURE_URL"))
            .await
            .unwrap()
            .0,
        "blocked_authority"
    );
    a.query_one("SELECT set_config('karate.subject','backup',false)", &[])
        .await
        .unwrap();
    a.execute(
        "SELECT karate.request_deletion($1::text::uuid,2,7,true)",
        &[&t],
    )
    .await
    .unwrap();
    a.execute(
        "SELECT karate.request_deletion($1::text::uuid,3,7,false)",
        &[&t],
    )
    .await
    .unwrap();
    a.execute(
        "UPDATE karate.tenant_deletions SET erase_after=0 WHERE tenant_id=$1::text::uuid",
        &[&t],
    )
    .await
    .unwrap();
    // Cancellation owns the same lock: a runner skips the uncommitted change,
    // then sees the committed cancellation rather than starting destruction.
    let cancel = a.transaction().await.unwrap();
    cancel
        .execute(
            "SELECT karate.request_deletion($1::text::uuid,4,7,true)",
            &[&t],
        )
        .await
        .unwrap();
    assert_eq!(
        karate_erasure::run(&env("KARATE_TEST_ERASURE_URL"))
            .await
            .unwrap()
            .0,
        "idle"
    );
    cancel.commit().await.unwrap();
    assert_eq!(
        karate_erasure::run(&env("KARATE_TEST_ERASURE_URL"))
            .await
            .unwrap()
            .0,
        "idle"
    );
    a.execute(
        "SELECT karate.request_deletion($1::text::uuid,5,7,false)",
        &[&t],
    )
    .await
    .unwrap();
    a.execute(
        "UPDATE karate.tenant_deletions SET erase_after=0 WHERE tenant_id=$1::text::uuid",
        &[&t],
    )
    .await
    .unwrap();
    // Unknown future stores fail closed, including those invisible to the runner role.
    a.batch_execute("CREATE TABLE karate.unregistered(tenant_id uuid)")
        .await
        .unwrap();
    assert_eq!(
        karate_erasure::run(&env("KARATE_TEST_ERASURE_URL")).await,
        Err(Error::Unavailable)
    );
    a.batch_execute("DROP TABLE karate.unregistered")
        .await
        .unwrap();
    // Rollback/restart: progress and row deletions commit together, never a stale lease.
    let tx = runner.transaction().await.unwrap();
    let first = tx
        .query_one("SELECT * FROM karate.erase_next_batch()", &[])
        .await
        .unwrap();
    assert_eq!(first.get::<_, i64>(1), 100);
    let other = karate_erasure::run(&env("KARATE_TEST_ERASURE_URL"))
        .await
        .unwrap();
    assert_eq!(other.0, "idle");
    tx.rollback().await.unwrap();
    assert_eq!(
        a.query_one(
            "SELECT count(*) FROM karate.audit_events WHERE tenant_id=$1::text::uuid",
            &[&t]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        205
    );
    let output = binary()
        .env(
            "KARATE_ERASURE_DATABASE_URL",
            env("KARATE_TEST_ERASURE_URL"),
        )
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("progress")
    );
    assert!(
        a.execute(
            "SELECT karate.request_deletion($1::text::uuid,6,7,true)",
            &[&t]
        )
        .await
        .is_err()
    );
    assert!(a.execute("INSERT INTO karate.login_flows(tenant_id,state_hash,binding_hash,nonce,pkce) VALUES($1::text::uuid,decode(repeat('cc',32),'hex'),decode(repeat('bb',32),'hex'),repeat('a',64),repeat('b',43))",&[&t]).await.is_err());
    let mut completed = false;
    for _ in 0..40 {
        let (state, n) = karate_erasure::run(&env("KARATE_TEST_ERASURE_URL"))
            .await
            .unwrap();
        assert!((0..=100).contains(&n));
        if state == "primary_database_erased" {
            completed = true;
            break;
        }
    }
    assert!(completed);
    for table in [
        "tenants",
        "memberships",
        "projects",
        "audit_events",
        "session_events",
        "key_events",
        "invitation_events",
        "membership_events",
        "tenant_events",
        "login_flows",
        "browser_sessions",
        "service_keys",
        "invitations",
        "provisioning_grants",
        "tenant_deletions",
    ] {
        assert_eq!(
            a.query_one(
                &format!("SELECT count(*) FROM karate.{table} WHERE tenant_id=$1::text::uuid"),
                &[&t]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
            0,
            "{table}"
        );
    }
    assert_eq!(
        a.query_one(
            "SELECT count(*) FROM karate.projects WHERE tenant_id=$1::text::uuid",
            &[&foreign]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        1
    );
    assert_eq!(
        a.query_one(
            "SELECT count(*) FROM karate.erasure_receipts WHERE tenant_id=$1::text::uuid",
            &[&t]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        1
    );
    assert!(
        a.execute(
            "INSERT INTO karate.tenants(tenant_id,name) VALUES($1::text::uuid,'Resurrection')",
            &[&t]
        )
        .await
        .is_err()
    );
    assert!(a.execute("SELECT karate.issue_tenant_grant($1::text::uuid,gen_random_uuid(),'Replay','https://issuer.example','backup',floor(extract(epoch FROM now()))::bigint+60,'fixture')",&[&t]).await.is_err());
    let tx = runner
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    assert!(
        tx.query("SELECT * FROM karate.erase_next_batch()", &[])
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    a.batch_execute(
        "REVOKE EXECUTE ON FUNCTION karate.erase_next_batch() FROM karate_erasure_runner",
    )
    .await
    .unwrap();
    assert!(
        !binary()
            .env(
                "KARATE_ERASURE_DATABASE_URL",
                env("KARATE_TEST_ERASURE_URL")
            )
            .output()
            .unwrap()
            .status
            .success()
    );
    a.batch_execute("GRANT EXECUTE ON FUNCTION karate.erase_next_batch() TO karate_erasure_runner")
        .await
        .unwrap();
    assert!(
        binary()
            .env(
                "KARATE_ERASURE_DATABASE_URL",
                env("KARATE_TEST_ERASURE_URL")
            )
            .output()
            .unwrap()
            .status
            .success()
    );
}
