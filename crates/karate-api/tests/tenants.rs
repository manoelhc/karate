use axum::{Router, body::Body, http::Request};
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use karate_api::{
    http::router,
    identity::{TokenVerifier, unix_now},
    invitations::InviteInput,
    keys::KeyInput,
    service::ProjectService,
    tenants::TenantChange,
};
use karate_core::{
    Error,
    authorization::{Action, Role},
    tenant::TenantState,
};
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

async fn issue(a: &Client, t: Uuid, subject: &str) -> Uuid {
    let id = Uuid::new_v4();
    a.execute("SELECT karate.issue_tenant_grant($1,$2,'New workspace','https://issuer.example',$3,$4,'local fixture authorization')",&[&t,&id,&subject,&(unix_now().unwrap()+600)]).await.unwrap();
    id
}
async fn wait_for_locks(admin: &Client, minimum: i64) {
    tokio::time::timeout(std::time::Duration::from_secs(2),async {
        loop {
            let count:i64=admin.query_one("SELECT count(*) FROM pg_stat_activity WHERE application_name='tenant_lifecycle' AND wait_event_type='Lock'",&[]).await.unwrap().get(0);
            if count>=minimum {break;}
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }).await.expect("operation must reach its lock wait before releasing the fixture");
}
fn change(revision: i64, state: TenantState) -> TenantChange {
    TenantChange {
        name: "Workspace renamed".into(),
        state,
        expected_revision: revision,
    }
}
#[tokio::test]
async fn provisioning_lifecycle_replay_authority_atomicity_and_serialized_recovery() {
    let a = db("KARATE_TEST_ADMIN_URL").await;
    let runtime = db("KARATE_TEST_DATABASE_URL").await;
    let verifier = TokenVerifier::new(
        "https://issuer.example",
        "karate-api",
        &std::fs::read_to_string(env("KARATE_TEST_JWKS")).unwrap(),
        unix_now().unwrap() + 3600,
    )
    .unwrap();
    let s = ProjectService::connect(
        verifier,
        &format!(
            "{} application_name=tenant_lifecycle",
            env("KARATE_TEST_DATABASE_URL")
        ),
    )
    .await
    .unwrap();
    let app = router(s.clone());
    let t = Uuid::new_v4();
    let grant = issue(&a, t, "new-owner").await;
    let owner = token("new-owner");
    assert!(matches!(
        s.provision_tenant("forged", t, grant).await,
        Err(Error::Unauthenticated)
    ));
    assert!(matches!(
        s.provision_tenant(&token("alice"), t, grant).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        s.provision_tenant(&owner, Uuid::new_v4(), grant).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        s.provision_tenant(&owner, t, Uuid::new_v4()).await,
        Err(Error::Forbidden)
    ));
    // Failed creation audit leaves neither owner nor tenant nor consumed grant.
    a.batch_execute("REVOKE INSERT ON karate.tenant_events FROM karate_membership_guard")
        .await
        .unwrap();
    assert!(matches!(
        s.provision_tenant(&owner, t, grant).await,
        Err(Error::Unavailable)
    ));
    assert_eq!(
        a.query_one(
            "SELECT count(*) FROM karate.tenants WHERE tenant_id=$1",
            &[&t]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        0
    );
    assert!(
        !a.query_one(
            "SELECT consumed FROM karate.provisioning_grants WHERE tenant_id=$1",
            &[&t]
        )
        .await
        .unwrap()
        .get::<_, bool>(0)
    );
    a.batch_execute("GRANT INSERT ON karate.tenant_events TO karate_membership_guard")
        .await
        .unwrap();
    let (left, right) = tokio::join!(
        s.provision_tenant(&owner, t, grant),
        s.provision_tenant(&owner, t, grant)
    );
    assert_ne!(left.is_ok(), right.is_ok());
    assert!(matches!(
        if left.is_err() { left } else { right },
        Err(Error::Conflict)
    ));
    let info = s.tenant(&owner, t).await.unwrap();
    assert_eq!(info.revision, 1);
    assert!(info.can_manage);
    assert_eq!(info.state, TenantState::Active);
    let member=a.query_one("SELECT role,expires_at IS NULL AS permanent FROM karate.memberships WHERE tenant_id=$1",&[&t]).await.unwrap();
    assert_eq!(member.get::<_, String>("role"), "owner");
    assert!(member.get::<_, bool>("permanent"));
    for subject in ["administrator", "reader"] {
        add(
            &a,
            t,
            subject,
            if subject == "reader" {
                "viewer"
            } else {
                "admin"
            },
        )
        .await;
        assert!(!s.tenant(&token(subject), t).await.unwrap().can_manage);
        assert!(matches!(
            s.change_tenant(&token(subject), t, &change(1, TenantState::Suspended))
                .await,
            Err(Error::Forbidden)
        ));
    }
    assert!(matches!(
        s.tenant(&token("bob"), t).await,
        Err(Error::Forbidden)
    ));
    let key = s
        .create_key(
            &owner,
            t,
            &KeyInput {
                key_id: Uuid::new_v4(),
                name: "Tenant fixture".into(),
                scopes: vec![Action::Read],
                expires_at: unix_now().unwrap() + 600,
            },
        )
        .await
        .unwrap()
        .secret;
    assert!(matches!(s.tenant(&key, t).await, Err(Error::Forbidden)));
    assert!(matches!(
        s.provision_tenant(&key, t, grant).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        s.change_tenant(&key, t, &change(1, TenantState::Suspended))
            .await,
        Err(Error::Forbidden)
    ));
    for n in [0, i64::MAX] {
        assert!(matches!(
            s.change_tenant(&owner, t, &change(n, TenantState::Active))
                .await,
            Err(Error::Validation)
        ));
    }
    let updated = s
        .change_tenant(&owner, t, &change(1, TenantState::Active))
        .await
        .unwrap();
    assert_eq!(updated.revision, 2);
    assert!(matches!(
        s.change_tenant(&owner, t, &change(1, TenantState::Active))
            .await,
        Err(Error::Conflict)
    ));
    assert_eq!(
        s.change_tenant(&owner, t, &change(2, TenantState::Active))
            .await
            .unwrap()
            .revision,
        2
    );
    let project = Uuid::new_v4();
    s.create(&owner, t, project, "Stored project")
        .await
        .unwrap();
    let invite = Uuid::new_v4();
    s.create_invitation(
        &owner,
        t,
        &InviteInput {
            invitation_id: invite,
            subject: "incoming".into(),
            role: Role::Viewer,
            expires_at: unix_now().unwrap() + 600,
        },
    )
    .await
    .unwrap();
    a.batch_execute("REVOKE INSERT ON karate.tenant_events FROM karate_membership_guard")
        .await
        .unwrap();
    assert!(matches!(
        s.change_tenant(&owner, t, &change(2, TenantState::Suspended))
            .await,
        Err(Error::Unavailable)
    ));
    assert_eq!(
        s.tenant(&owner, t).await.unwrap().state,
        TenantState::Active
    );
    a.batch_execute("GRANT INSERT ON karate.tenant_events TO karate_membership_guard")
        .await
        .unwrap();
    s.change_tenant(&owner, t, &change(2, TenantState::Suspended))
        .await
        .unwrap();
    for credential in [&owner, &key] {
        assert!(matches!(
            s.list(credential, t, 1, None).await,
            Err(Error::Forbidden)
        ));
    }
    assert!(matches!(
        s.list_members(&owner, t, 1, None).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        s.list_keys(&owner, t, 1, None).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        s.accept_invitation(&token("incoming"), t, invite).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        s.tenant(&token("reader"), t).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        s.provision_tenant(&owner, t, grant).await,
        Err(Error::Conflict)
    ));
    s.change_tenant(&owner, t, &change(3, TenantState::Active))
        .await
        .unwrap();
    assert!(s.get(&owner, t, project).await.is_ok());
    assert!(s.list(&key, t, 1, None).await.is_ok());
    // HTTP contract, localized errors and optimistic revision responses.
    let path = format!("/v1/tenants/{t}");
    assert_eq!(
        request(&app, "GET", &path, Some(&owner), json!({})).await.0,
        200
    );
    assert_eq!(request(&app, "GET", &path, None, json!({})).await.0, 401);
    assert_eq!(
        request(
            &app,
            "PATCH",
            &path,
            Some(&owner),
            json!({"name":"HTTP renamed","state":"active","expected_revision":4})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(
            &app,
            "PATCH",
            &path,
            Some(&owner),
            json!({"name":"HTTP renamed","state":"deleted","expected_revision":5})
        )
        .await
        .0,
        400
    );
    assert_eq!(
        request(
            &app,
            "PATCH",
            &path,
            Some(&owner),
            json!({"name":"","state":"active","expected_revision":5})
        )
        .await
        .0,
        400
    );
    let new = Uuid::new_v4();
    let offer = issue(&a, new, "new-owner").await;
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/v1/tenants/{new}/provision"),
            Some(&owner),
            json!({"grant_id":offer})
        )
        .await
        .0,
        201
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/v1/tenants/{new}/provision"),
            Some(&owner),
            json!({"grant_id":offer})
        )
        .await
        .0,
        409
    );
    for kind in ["expired", "revoked"] {
        let t = Uuid::new_v4();
        let g = issue(&a, t, "new-owner").await;
        if kind == "expired" {
            a.execute(
                "UPDATE karate.provisioning_grants SET expires_at=0 WHERE tenant_id=$1",
                &[&t],
            )
            .await
            .unwrap();
        } else {
            a.execute("SELECT karate.revoke_tenant_grant($1,$2)", &[&t, &g])
                .await
                .unwrap();
        }
        assert!(matches!(
            s.provision_tenant(&owner, t, g).await,
            Err(Error::Conflict)
        ));
    }
    // Runtime never issues grants, reads the grant table, writes tenants or events.
    for sql in [
        "SELECT * FROM karate.provisioning_grants",
        "INSERT INTO karate.tenants DEFAULT VALUES",
        "UPDATE karate.tenants SET state='suspended'",
        "INSERT INTO karate.tenant_events DEFAULT VALUES",
        "SELECT karate.issue_tenant_grant(null,null,null,null,null,null,null)",
        "SELECT karate.revoke_tenant_grant(null,null)",
    ] {
        assert!(runtime.batch_execute(sql).await.is_err());
    }
    // Suspension waits for an admitted project transaction to finish its atomic audit.
    let mut blocker = db("KARATE_TEST_ADMIN_URL").await;
    let tx = blocker.transaction().await.unwrap();
    tx.batch_execute("LOCK TABLE karate.audit_events IN ACCESS EXCLUSIVE MODE")
        .await
        .unwrap();
    let service = s.clone();
    let credential = owner.clone();
    let write = tokio::spawn(async move {
        service
            .create(&credential, t, Uuid::new_v4(), "Before suspension")
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2),async{loop{let n:i64=a.query_one("SELECT count(*) FROM pg_stat_activity WHERE application_name='tenant_lifecycle' AND wait_event_type='Lock'",&[]).await.unwrap().get(0);if n>0{break;}tokio::time::sleep(std::time::Duration::from_millis(10)).await;}}).await.unwrap();
    let service = s.clone();
    let credential = owner.clone();
    let suspension = tokio::spawn(async move {
        service
            .change_tenant(&credential, t, &change(5, TenantState::Suspended))
            .await
    });
    wait_for_locks(&a, 2).await;
    assert!(!suspension.is_finished());
    tx.commit().await.unwrap();
    write.await.unwrap().unwrap();
    suspension.await.unwrap().unwrap();
    assert!(matches!(
        s.create(&owner, t, Uuid::new_v4(), "After suspension")
            .await,
        Err(Error::Forbidden)
    ));
    s.change_tenant(&owner, t, &change(6, TenantState::Active))
        .await
        .unwrap();
    // An owner revoked while waiting cannot resume or alter metadata using an old snapshot.
    let tx = blocker.transaction().await.unwrap();
    tx.execute(
        "SELECT 1 FROM karate.tenants WHERE tenant_id=$1 FOR NO KEY UPDATE",
        &[&t],
    )
    .await
    .unwrap();
    tx.execute("UPDATE karate.memberships SET revoked_at=clock_timestamp(),revision=revision+1 WHERE tenant_id=$1 AND subject='new-owner'",&[&t]).await.unwrap();
    let service = s.clone();
    let credential = owner.clone();
    let pending = tokio::spawn(async move {
        service
            .change_tenant(&credential, t, &change(7, TenantState::Suspended))
            .await
    });
    wait_for_locks(&a, 1).await;
    assert!(!pending.is_finished());
    tx.commit().await.unwrap();
    assert!(matches!(pending.await.unwrap(), Err(Error::Forbidden)));
    assert_eq!(
        a.query_one("SELECT state FROM karate.tenants WHERE tenant_id=$1", &[&t])
            .await
            .unwrap()
            .get::<_, String>(0),
        "active"
    );
}
