use axum::{Router, body::Body, http::Request};
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use karate_api::{
    http::router,
    identity::{TokenVerifier, unix_now},
    service::ProjectService,
};
use karate_core::{Error, authorization::Role};
use serde_json::{Value, json};
use tokio_postgres::{Client, NoTls};
use tower::ServiceExt;
use uuid::Uuid;

fn env(key: &str) -> String {
    std::env::var(key).expect("use make test with isolated fixture")
}
fn token(subject: &str) -> String {
    let mut h = Header::new(Algorithm::RS256);
    h.typ = Some("at+jwt".into());
    h.kid = Some("fixture".into());
    let now = unix_now().unwrap();
    encode(&h,&json!({"iss":"https://issuer.example","sub":subject,"aud":"karate-api","iat":now-1,"exp":now+900}),&EncodingKey::from_rsa_der(&std::fs::read(env("KARATE_TEST_KEY")).unwrap())).unwrap()
}
async fn db(name: &str) -> Client {
    let (c, connection) = tokio_postgres::connect(&env(name), NoTls).await.unwrap();
    tokio::spawn(async move {
        connection.await.unwrap();
    });
    c
}
async fn add(admin: &Client, tenant: Uuid, subject: &str, role: &str) -> Uuid {
    let id = Uuid::new_v4();
    admin.execute("INSERT INTO karate.memberships(tenant_id,membership_id,issuer,subject,role) VALUES($1,$2,'https://issuer.example',$3,$4)",&[&tenant,&id,&subject,&role]).await.unwrap();
    id
}
async fn request(
    app: &Router,
    method: &str,
    path: &str,
    credential: Option<&str>,
    body: Value,
) -> (u16, Value) {
    let mut r = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .header("accept-language", "pt-BR");
    if let Some(c) = credential {
        r = r.header("authorization", format!("Bearer {c}"));
    }
    let response = app
        .clone()
        .oneshot(r.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    assert_eq!(response.headers()["content-language"], "pt-BR");
    assert_eq!(response.headers()["cache-control"], "no-store");
    let status = response.status().as_u16();
    let body =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    (status, body)
}

#[tokio::test]
async fn membership_management_enforces_roles_revisions_ownership_and_atomic_audit() {
    let mut admin = db("KARATE_TEST_ADMIN_URL").await;
    let runtime = db("KARATE_TEST_DATABASE_URL").await;
    let tenant = Uuid::from_u128(0x301);
    let foreign = Uuid::from_u128(0x302);
    for t in [tenant, foreign] {
        admin
            .execute(
                "INSERT INTO karate.tenants(tenant_id,name) VALUES($1,'Membership fixture')",
                &[&t],
            )
            .await
            .unwrap();
    }
    let owner = add(&admin, tenant, "owner", "owner").await;
    let manager = add(&admin, tenant, "manager", "admin").await;
    let member = add(&admin, tenant, "person", "member").await;
    let viewer = add(&admin, tenant, "watcher", "viewer").await;
    let outside = add(&admin, foreign, "outsider", "owner").await;
    let keys = std::fs::read_to_string(env("KARATE_TEST_JWKS")).unwrap();
    let service = ProjectService::connect(
        TokenVerifier::new(
            "https://issuer.example",
            "karate-api",
            &keys,
            unix_now().unwrap() + 3600,
        )
        .unwrap(),
        &env("KARATE_TEST_DATABASE_URL"),
    )
    .await
    .unwrap();
    admin
        .batch_execute("GRANT karate_membership_guard TO karate_api_test")
        .await
        .unwrap();
    assert!(matches!(
        ProjectService::connect(
            TokenVerifier::new(
                "https://issuer.example",
                "karate-api",
                &keys,
                unix_now().unwrap() + 3600
            )
            .unwrap(),
            &env("KARATE_TEST_DATABASE_URL")
        )
        .await,
        Err(Error::Forbidden)
    ));
    admin
        .batch_execute("REVOKE karate_membership_guard FROM karate_api_test")
        .await
        .unwrap();
    let public_execute=admin.query_one("SELECT EXISTS(SELECT 1 FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace CROSS JOIN LATERAL aclexplode(p.proacl) a WHERE n.nspname='karate' AND p.prosecdef AND a.grantee=0)",&[]).await.unwrap();
    assert!(!public_execute.get::<_, bool>(0));
    let app = router(service.clone());
    let root = format!("/v1/tenants/{tenant}/memberships");
    let ot = token("owner");
    let at = token("manager");
    let mt = token("person");
    let vt = token("watcher");
    let departing = add(&admin, foreign, "departing", "owner").await;
    let retired = service
        .change_member(&token("departing"), foreign, departing, 1, None)
        .await
        .unwrap();
    assert!(retired.revoked);
    assert!(matches!(
        service
            .list_members(&token("departing"), foreign, 25, None)
            .await,
        Err(Error::Forbidden)
    ));
    assert_eq!(
        service
            .list_members(&token("outsider"), foreign, 25, None)
            .await
            .unwrap()
            .items
            .len(),
        2
    );
    // Operator defaults cannot silently change the fresh-snapshot lock protocol.
    let serial_default = ProjectService::connect(
        TokenVerifier::new(
            "https://issuer.example",
            "karate-api",
            &keys,
            unix_now().unwrap() + 3600,
        )
        .unwrap(),
        &format!(
            "{} options='-c default_transaction_isolation=serializable'",
            env("KARATE_TEST_DATABASE_URL")
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        serial_default
            .change_member(&ot, tenant, owner, 1, Some(Role::Owner))
            .await
            .unwrap()
            .revision,
        1
    );
    drop(serial_default);
    let page = service.list_members(&ot, tenant, 100, None).await.unwrap();
    assert_eq!(page.items.len(), 4);
    assert!(page.next_cursor.is_none());
    let page = service.list_members(&at, tenant, 1, None).await.unwrap();
    assert_eq!(page.items.len(), 1);
    let rest = service
        .list_members(&at, tenant, 100, page.next_cursor)
        .await
        .unwrap();
    assert_eq!(rest.items.len(), 3);
    for invalid in [0, 101, -1] {
        assert!(matches!(
            service.list_members(&ot, tenant, invalid, None).await,
            Err(Error::Validation)
        ));
    }
    for c in [&mt, &vt] {
        assert!(matches!(
            service.list_members(c, tenant, 25, None).await,
            Err(Error::Forbidden)
        ));
        assert!(matches!(
            service
                .change_member(c, tenant, member, 1, Some(Role::Admin))
                .await,
            Err(Error::Forbidden)
        ));
    }
    assert!(matches!(
        service.list_members("invalid", tenant, 25, None).await,
        Err(Error::Unauthenticated)
    ));
    assert!(matches!(
        service
            .change_member("invalid", tenant, member, 1, None)
            .await,
        Err(Error::Unauthenticated)
    ));
    assert!(matches!(
        service.list_members(&ot, foreign, 25, None).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service.change_member(&ot, tenant, outside, 1, None).await,
        Err(Error::NotFound)
    ));
    for r in [0, i64::MAX] {
        assert!(matches!(
            service.change_member(&ot, tenant, member, r, None).await,
            Err(Error::Validation)
        ));
    }
    for (target, desired) in [
        (owner, None),
        (manager, Some(Role::Viewer)),
        (member, Some(Role::Owner)),
        (member, Some(Role::Admin)),
    ] {
        assert!(matches!(
            service.change_member(&at, tenant, target, 1, desired).await,
            Err(Error::Forbidden)
        ));
    }
    assert!(matches!(
        service.change_member(&ot, tenant, owner, 1, None).await,
        Err(Error::OwnerRequired)
    ));
    assert!(matches!(
        service
            .change_member(&ot, tenant, owner, 1, Some(Role::Admin))
            .await,
        Err(Error::OwnerRequired)
    ));
    let unchanged = service
        .change_member(&ot, tenant, owner, 1, Some(Role::Owner))
        .await
        .unwrap();
    assert_eq!(unchanged.revision, 1);
    let changed = service
        .change_member(&at, tenant, member, 1, Some(Role::Viewer))
        .await
        .unwrap();
    assert_eq!(changed.role, Role::Viewer);
    assert_eq!(changed.revision, 2);
    assert!(matches!(
        service.change_member(&at, tenant, member, 1, None).await,
        Err(Error::Conflict)
    ));
    assert_eq!(
        service
            .change_member(&at, tenant, member, 2, Some(Role::Member))
            .await
            .unwrap()
            .revision,
        3
    );
    let retired = service
        .change_member(&at, tenant, viewer, 1, None)
        .await
        .unwrap();
    assert!(retired.revoked);
    assert!(matches!(
        service.get(&vt, tenant, Uuid::new_v4()).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service
            .change_member(&ot, tenant, viewer, 2, Some(Role::Member))
            .await,
        Err(Error::Conflict)
    ));
    let expiring = add(&admin, tenant, "temporary", "member").await;
    admin.execute("UPDATE karate.memberships SET expires_at=clock_timestamp()+interval '1 hour' WHERE membership_id=$1",&[&expiring]).await.unwrap();
    assert!(matches!(
        service
            .change_member(&ot, tenant, expiring, 1, Some(Role::Owner))
            .await,
        Err(Error::OwnerRequired)
    ));
    admin.execute("UPDATE karate.memberships SET expires_at=clock_timestamp()-interval '1 second' WHERE membership_id=$1",&[&expiring]).await.unwrap();
    assert!(matches!(
        service.change_member(&ot, tenant, expiring, 1, None).await,
        Err(Error::Conflict)
    ));
    let second = service
        .change_member(&ot, tenant, member, 3, Some(Role::Owner))
        .await
        .unwrap();
    assert_eq!(second.revision, 4);
    // Both self-demotions start with two owners. Only one can commit.
    let (first, second) = tokio::join!(
        service.change_member(&ot, tenant, owner, 1, Some(Role::Admin)),
        service.change_member(&mt, tenant, member, 4, Some(Role::Admin))
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(
        matches!(first, Err(Error::OwnerRequired)) || matches!(second, Err(Error::OwnerRequired))
    );
    let (active_token, active_id, active_revision) = if first.is_ok() {
        (&mt, member, 4)
    } else {
        (&ot, owner, 1)
    };
    assert!(matches!(
        service
            .change_member(active_token, tenant, active_id, active_revision, None)
            .await,
        Err(Error::OwnerRequired)
    ));
    // An expiring owner is not a valid last-owner successor.
    admin.execute("UPDATE karate.memberships SET role='owner',expires_at=clock_timestamp()+interval '1 hour' WHERE membership_id=$1",&[&expiring]).await.unwrap();
    assert!(matches!(
        service
            .change_member(active_token, tenant, active_id, active_revision, None)
            .await,
        Err(Error::OwnerRequired)
    ));
    admin
        .execute(
            "UPDATE karate.memberships SET role='member' WHERE membership_id=$1",
            &[&expiring],
        )
        .await
        .unwrap();
    // Audit storage failure must roll back the membership mutation.
    admin
        .batch_execute("REVOKE INSERT ON karate.membership_events FROM karate_membership_guard")
        .await
        .unwrap();
    assert!(matches!(
        service
            .change_member(active_token, tenant, manager, 1, Some(Role::Member))
            .await,
        Err(Error::Unavailable)
    ));
    admin
        .batch_execute("GRANT INSERT ON karate.membership_events TO karate_membership_guard")
        .await
        .unwrap();
    let row = admin
        .query_one(
            "SELECT role,revision FROM karate.memberships WHERE membership_id=$1",
            &[&manager],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "admin");
    assert_eq!(row.get::<_, i64>(1), 1);
    // The actor loses authority while its request waits on the tenant lock.
    let queued = add(&admin, tenant, "queued-manager", "admin").await;
    let tx = admin.transaction().await.unwrap();
    tx.query_one(
        "SELECT tenant_id FROM karate.tenants WHERE tenant_id=$1 FOR NO KEY UPDATE",
        &[&tenant],
    )
    .await
    .unwrap();
    let waiting_service = service.clone();
    let waiting = tokio::spawn(async move {
        waiting_service
            .change_member(
                &token("queued-manager"),
                tenant,
                expiring,
                1,
                Some(Role::Viewer),
            )
            .await
    });
    let mut observed = false;
    for _ in 0..60 {
        let row=tx.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE usename='karate_api_test' AND wait_event_type='Lock')", &[]).await.unwrap();
        if row.get::<_, bool>(0) {
            observed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        observed,
        "management request must actually wait on the lock"
    );
    tx.execute(
        "UPDATE karate.memberships SET revoked_at=clock_timestamp() WHERE membership_id=$1",
        &[&queued],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert!(matches!(waiting.await.unwrap(), Err(Error::Forbidden)));
    assert!(matches!(
        service
            .list_members(&token("queued-manager"), tenant, 25, None)
            .await,
        Err(Error::Forbidden)
    ));
    runtime.execute("SELECT set_config('karate.tenant_id',$1,false),set_config('karate.issuer','https://issuer.example',false),set_config('karate.subject','manager',false)",&[&tenant.to_string()]).await.unwrap();
    for (target, role) in [
        (active_id, None),
        (expiring, Some("owner")),
        (manager, None),
    ] {
        let failure = runtime
            .query(
                "SELECT * FROM karate.change_membership($1,$2,1,$3,$4)",
                &[&tenant, &target, &role, &Uuid::new_v4()],
            )
            .await
            .unwrap_err();
        assert_eq!(failure.code().unwrap().code(), "K0001");
    }
    runtime
        .batch_execute("RESET karate.tenant_id; RESET karate.issuer; RESET karate.subject")
        .await
        .unwrap();
    // HTTP contracts and localized closed errors, including unknown/invalid fields.
    let (status, body) = request(&app, "GET", &root, Some(active_token), Value::Null).await;
    assert_eq!(status, 200);
    assert_eq!(body["items"].as_array().unwrap().len(), 6);
    assert_eq!(request(&app, "GET", &root, None, Value::Null).await.0, 401);
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("{root}?limit=bad"),
            Some(active_token),
            Value::Null
        )
        .await
        .0,
        400
    );
    assert_eq!(
        request(
            &app,
            "GET",
            "/v1/tenants/bad/memberships",
            Some(active_token),
            Value::Null
        )
        .await
        .0,
        400
    );
    for (method, path, payload) in [
        (
            "PATCH",
            format!("{root}/{manager}"),
            json!({"role":"invalid","expected_revision":1}),
        ),
        (
            "PATCH",
            format!("{root}/bad"),
            json!({"role":"viewer","expected_revision":1}),
        ),
        (
            "POST",
            format!("{root}/{manager}/revoke"),
            json!({"expected_revision":1,"extra":true}),
        ),
        (
            "POST",
            format!("{root}/bad/revoke"),
            json!({"expected_revision":1}),
        ),
    ] {
        assert_eq!(
            request(&app, method, &path, Some(active_token), payload)
                .await
                .0,
            400
        );
    }
    let (status, body) = request(
        &app,
        "POST",
        &format!("{root}/{active_id}/revoke"),
        Some(active_token),
        json!({"expected_revision":active_revision}),
    )
    .await;
    assert_eq!(status, 409);
    assert_eq!(body["code"], "OWNER_REQUIRED");
    assert!(
        body["remediation"]
            .as_str()
            .unwrap()
            .contains("proprietário")
    );
    let path = format!("{root}/{manager}");
    let input = json!({"role":"viewer","expected_revision":1});
    assert_eq!(
        request(&app, "PATCH", &path, None, input.clone()).await.0,
        401
    );
    assert_eq!(
        request(&app, "PATCH", &path, Some(active_token), input)
            .await
            .0,
        200
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{path}/revoke"),
            None,
            json!({"expected_revision":2})
        )
        .await
        .0,
        401
    );
    let (status, body) = request(
        &app,
        "POST",
        &format!("{path}/revoke"),
        Some(active_token),
        json!({"expected_revision":2}),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["revoked"], true);
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{path}/revoke"),
            Some(active_token),
            json!({"expected_revision":2})
        )
        .await
        .0,
        409
    );
    // Direct SQL cannot bypass function permissions, audit immutability or tenant checks.
    assert!(
        runtime
            .execute("UPDATE karate.memberships SET role='owner'", &[])
            .await
            .is_err()
    );
    assert!(
        runtime
            .batch_execute("SET ROLE karate_membership_guard")
            .await
            .is_err()
    );
    assert!(
        runtime
            .query(
                "SELECT * FROM karate.list_memberships($1,25,NULL)",
                &[&tenant]
            )
            .await
            .is_err()
    );
    assert_eq!(
        runtime
            .query("SELECT * FROM karate.membership_events", &[])
            .await
            .unwrap()
            .len(),
        0
    );
    for action in [
        "DELETE FROM karate.membership_events",
        "UPDATE karate.membership_events SET old_role='owner'",
        "INSERT INTO karate.membership_events DEFAULT VALUES",
    ] {
        assert!(runtime.batch_execute(action).await.is_err());
    }
    let active_subject = if active_id == owner {
        "owner"
    } else {
        "person"
    };
    runtime.execute("SELECT set_config('karate.tenant_id',$1,false),set_config('karate.issuer','https://issuer.example',false),set_config('karate.subject',$2,false)",&[&tenant.to_string(),&active_subject]).await.unwrap();
    let events = runtime
        .query(
            "SELECT action,actor_issuer,revision FROM karate.membership_events",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(events.len(), 7);
    for row in events {
        assert_eq!(row.get::<_, String>(1), "https://issuer.example");
        assert!(row.get::<_, i64>(2) > 1);
    }
    assert!(
        runtime
            .query(
                "SELECT * FROM karate.list_memberships($1,25,NULL)",
                &[&foreign]
            )
            .await
            .is_err()
    );
    assert!(
        runtime
            .query(
                "SELECT * FROM karate.list_memberships($1,102,NULL)",
                &[&tenant]
            )
            .await
            .is_err()
    );
    runtime
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ")
        .await
        .unwrap();
    let failure = runtime
        .query(
            "SELECT * FROM karate.membership_snapshot($1,$2)",
            &[&tenant, &active_id],
        )
        .await
        .unwrap_err();
    assert_eq!(failure.code().unwrap().code(), "K0004");
    runtime.batch_execute("ROLLBACK").await.unwrap();
    for (target, rev, role, code) in [
        (active_id, active_revision, None, "K0005"),
        (Uuid::new_v4(), 1, None, "K0002"),
        (active_id, 0, None, "K0004"),
        (active_id, active_revision, Some("invalid"), "K0004"),
        (manager, 2, None, "K0003"),
    ] {
        let failure = runtime
            .query(
                "SELECT * FROM karate.change_membership($1,$2,$3,$4,$5)",
                &[&tenant, &target, &rev, &role, &Uuid::new_v4()],
            )
            .await
            .unwrap_err();
        assert_eq!(failure.code().unwrap().code(), code);
    }
    runtime.batch_execute("CREATE TEMP TABLE memberships(role text); INSERT INTO memberships VALUES('owner'); SET search_path=pg_temp,public").await.unwrap();
    assert_eq!(
        runtime
            .query(
                "SELECT * FROM karate.list_memberships($1,25,NULL)",
                &[&tenant]
            )
            .await
            .unwrap()
            .len(),
        6
    );
    // Foreign issuer with same subject never gains directory access.
    runtime
        .batch_execute("SELECT set_config('karate.issuer','https://other.example',false)")
        .await
        .unwrap();
    assert!(
        runtime
            .query(
                "SELECT * FROM karate.list_memberships($1,25,NULL)",
                &[&tenant]
            )
            .await
            .is_err()
    );
}
