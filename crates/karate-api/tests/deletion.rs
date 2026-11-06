use axum::{Router, body::Body, http::Request};
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use karate_api::{
    http::router,
    identity::{TokenVerifier, unix_now},
    service::ProjectService,
    tenants::{DeletionInput, TenantChange},
};
use karate_core::{Error, tenant::TenantState};
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
async fn deletion_auth_revision_cancel_interlocks_atomicity_and_http_contract() {
    let a = db("KARATE_TEST_ADMIN_URL").await;
    let verifier = || {
        TokenVerifier::new(
            "https://issuer.example",
            "karate-api",
            &std::fs::read_to_string(env("KARATE_TEST_JWKS")).unwrap(),
            unix_now().unwrap() + 3600,
        )
        .unwrap()
    };
    assert!(matches!(
        ProjectService::connect(verifier(), &env("KARATE_TEST_ERASURE_URL")).await,
        Err(Error::Forbidden)
    ));
    let s = ProjectService::connect(verifier(), &env("KARATE_TEST_DATABASE_URL"))
        .await
        .unwrap();
    let app = router(s.clone());
    let t = Uuid::new_v4();
    a.execute(
        "INSERT INTO karate.tenants VALUES($1,'Deletion workspace',now(),'active',1)",
        &[&t],
    )
    .await
    .unwrap();
    add(&a, t, "deletion-owner", "owner").await;
    add(&a, t, "deletion-viewer", "viewer").await;
    let owner = token("deletion-owner");
    let viewer = token("deletion-viewer");
    let input = |revision| DeletionInput {
        confirmation: t,
        retention_days: 7,
        expected_revision: revision,
    };
    assert!(s.tenant(&owner, t).await.unwrap().deletion.is_none());
    let key = s
        .create_key(
            &owner,
            t,
            &karate_api::keys::KeyInput {
                key_id: Uuid::new_v4(),
                name: "Read fixture".into(),
                scopes: vec![karate_core::authorization::Action::Read],
                expires_at: unix_now().unwrap() + 60,
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        s.request_deletion(&key.secret, t, &input(1), false).await,
        Err(Error::Forbidden)
    ));

    for credential in ["bad", &viewer] {
        assert!(
            s.request_deletion(credential, t, &input(1), false)
                .await
                .is_err()
        );
    }
    assert!(matches!(
        s.request_deletion(&owner, Uuid::new_v4(), &input(1), false)
            .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        s.request_deletion(&owner, t, &input(1), true).await,
        Err(Error::Conflict)
    ));
    a.batch_execute("REVOKE INSERT ON karate.tenant_events FROM karate_membership_guard")
        .await
        .unwrap();
    assert!(matches!(
        s.request_deletion(&owner, t, &input(1), false).await,
        Err(Error::Unavailable)
    ));
    a.batch_execute("GRANT INSERT ON karate.tenant_events TO karate_membership_guard")
        .await
        .unwrap();
    assert_eq!(s.tenant(&owner, t).await.unwrap().revision, 1);
    assert!(s.tenant(&owner, t).await.unwrap().deletion.is_none());
    let pending = s
        .request_deletion(&owner, t, &input(1), false)
        .await
        .unwrap();
    let job = pending.deletion.unwrap();
    let event = a.query_one("SELECT action,deletion_request_id,retention_days,erase_after FROM karate.tenant_events WHERE tenant_id=$1 ORDER BY revision DESC LIMIT 1", &[&t]).await.unwrap();
    assert_eq!(event.get::<_, String>(0), "tenant.deletion_requested");
    assert_eq!(event.get::<_, Uuid>(1), job.request_id);
    assert_eq!(event.get::<_, i32>(2), 7);
    assert_eq!(event.get::<_, i64>(3), job.erase_after);
    assert_eq!(pending.state, TenantState::Suspended);
    assert_eq!(job.state, "pending");
    assert_eq!(job.erase_after - job.requested_at, 7 * 86400);
    assert_eq!(
        s.tenant(&owner, t)
            .await
            .unwrap()
            .deletion
            .unwrap()
            .request_id,
        job.request_id
    );
    assert!(matches!(s.tenant(&viewer, t).await, Err(Error::Forbidden)));
    assert!(matches!(
        s.request_deletion(&owner, t, &input(1), false).await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        s.request_deletion(&owner, t, &input(2), false).await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        s.change_tenant(
            &owner,
            t,
            &TenantChange {
                name: "Resume".into(),
                state: TenantState::Active,
                expected_revision: 2
            }
        )
        .await,
        Err(Error::Conflict)
    ));
    a.execute(
        "UPDATE karate.tenant_deletions SET state='erasing' WHERE tenant_id=$1",
        &[&t],
    )
    .await
    .unwrap();
    assert!(matches!(s.tenant(&owner, t).await, Err(Error::Forbidden)));
    a.execute(
        "UPDATE karate.tenant_deletions SET state='pending' WHERE tenant_id=$1",
        &[&t],
    )
    .await
    .unwrap();
    let r = s
        .request_deletion(&owner, t, &input(2), true)
        .await
        .unwrap();
    assert_eq!(r.revision, 3);
    assert_eq!(r.state, TenantState::Suspended);
    assert_eq!(r.deletion.unwrap().state, "cancelled");
    let r = s
        .change_tenant(
            &owner,
            t,
            &TenantChange {
                name: "Resumed".into(),
                state: TenantState::Active,
                expected_revision: 3,
            },
        )
        .await
        .unwrap();
    assert_eq!(r.revision, 4);
    let path = format!("/v1/tenants/{t}/deletion");
    for (route, credential, body, status) in [
        (path.clone(), None, json!({}), 401),
        (
            path.clone(),
            Some(viewer.as_str()),
            json!({"confirmation":t,"retention_days":7,"expected_revision":4}),
            403,
        ),
        (
            path.clone(),
            Some(owner.as_str()),
            json!({"confirmation":Uuid::new_v4(),"retention_days":7,"expected_revision":4}),
            400,
        ),
        (
            path.clone(),
            Some(owner.as_str()),
            json!({"confirmation":t,"retention_days":31,"expected_revision":4}),
            400,
        ),
        (
            path.clone(),
            Some(owner.as_str()),
            json!({"confirmation":t,"retention_days":7,"expected_revision":0}),
            400,
        ),
        (
            path.clone(),
            Some(owner.as_str()),
            json!({"extra":true}),
            400,
        ),
        (
            "/v1/tenants/invalid/deletion".into(),
            Some(owner.as_str()),
            json!({}),
            400,
        ),
        (format!("{path}/cancel"), None, json!({}), 401),
        (
            format!("{path}/cancel"),
            Some(owner.as_str()),
            json!({}),
            400,
        ),
        (
            "/v1/tenants/invalid/deletion/cancel".into(),
            Some(owner.as_str()),
            json!({}),
            400,
        ),
    ] {
        assert_eq!(
            request(&app, "POST", &route, credential, body).await.0,
            status
        );
    }
    let body = json!({"confirmation":t,"retention_days":30,"expected_revision":4});
    assert_eq!(
        request(&app, "POST", &path, Some(&owner), body).await.0,
        200
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{path}/cancel"),
            Some(&owner),
            json!({"confirmation":t,"retention_days":7,"expected_revision":5})
        )
        .await
        .0,
        200
    );
    // Actual lock wait: a revoked requester loses permission after obtaining the lock.
    let mut locking = db("KARATE_TEST_ADMIN_URL").await;
    let tx = locking.transaction().await.unwrap();
    tx.query_one(
        "SELECT tenant_id FROM karate.tenants WHERE tenant_id=$1 FOR UPDATE",
        &[&t],
    )
    .await
    .unwrap();
    let service = s.clone();
    let credential = owner.clone();
    let task = tokio::spawn(async move {
        service
            .request_deletion(
                &credential,
                t,
                &DeletionInput {
                    confirmation: t,
                    retention_days: 7,
                    expected_revision: 6,
                },
                false,
            )
            .await
    });
    let mut observed_wait = false;
    for _ in 0..100 {
        let n: i64 = a
            .query_one(
                "SELECT count(*) FROM pg_stat_activity WHERE wait_event_type='Lock'",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        if n > 0 {
            observed_wait = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        observed_wait,
        "request must actually wait on the tenant lock"
    );
    tx.execute("UPDATE karate.memberships SET revoked_at=now(),revision=revision+1 WHERE tenant_id=$1 AND subject='deletion-owner'",&[&t]).await.unwrap();
    tx.commit().await.unwrap();
    assert!(matches!(task.await.unwrap(), Err(Error::Forbidden)));
}
