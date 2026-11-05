use axum::{Router, body::Body, http::Request};
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use karate_api::{
    http::router,
    identity::{TokenVerifier, unix_now},
    keys::KeyInput,
    service::ProjectService,
};
use karate_core::{
    Error,
    authorization::{Action, Role},
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

fn input(scopes: Vec<Action>) -> KeyInput {
    KeyInput {
        key_id: Uuid::new_v4(),
        name: "fixture automation".into(),
        scopes,
        expires_at: unix_now().unwrap() + 3600,
    }
}
#[tokio::test]
async fn api_keys_verify_scopes_tenants_revisions_rotation_and_atomic_audit() {
    let admin = db("KARATE_TEST_ADMIN_URL").await;
    let runtime = db("KARATE_TEST_DATABASE_URL").await;
    let tenant = Uuid::from_u128(0x501);
    let foreign = Uuid::from_u128(0x502);
    for t in [tenant, foreign] {
        admin
            .execute(
                "INSERT INTO karate.tenants(tenant_id,name) VALUES($1,'Key fixture')",
                &[&t],
            )
            .await
            .unwrap();
    }
    let owner = add(&admin, tenant, "key-owner", "owner").await;
    let manager = add(&admin, tenant, "key-admin", "admin").await;
    add(&admin, tenant, "key-viewer", "viewer").await;
    add(&admin, foreign, "key-owner", "owner").await;
    let jwks = std::fs::read_to_string(env("KARATE_TEST_JWKS")).unwrap();
    let service = ProjectService::connect(
        TokenVerifier::new(
            "https://issuer.example",
            "karate-api",
            &jwks,
            unix_now().unwrap() + 3600,
        )
        .unwrap(),
        &env("KARATE_TEST_DATABASE_URL"),
    )
    .await
    .unwrap();
    let app = router(service.clone());
    let ot = token("key-owner");
    let at = token("key-admin");
    let vt = token("key-viewer");
    let project = Uuid::new_v4();
    service
        .create(&ot, tenant, project, "Key test project")
        .await
        .unwrap();
    let read_input = input(vec![Action::Read]);
    let issued = service.create_key(&ot, tenant, &read_input).await.unwrap();
    assert!(issued.secret.starts_with("kk1."));
    assert_eq!(issued.secret.len(), 134);
    let stored: Vec<u8> = admin
        .query_one(
            "SELECT verifier FROM karate.service_keys WHERE tenant_id=$1 AND key_id=$2",
            &[&tenant, &issued.key.key_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(stored.len(), 32);
    assert!(
        stored.as_slice()
            == aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, issued.secret.as_bytes())
                .as_ref()
    );
    assert!(service.get(&issued.secret, tenant, project).await.is_ok());
    assert!(matches!(
        service
            .create(&issued.secret, tenant, Uuid::new_v4(), "denied")
            .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service.get(&issued.secret, foreign, project).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service.create_key(&ot, tenant, &read_input).await,
        Err(Error::Conflict)
    ));
    for scopes in [
        vec![],
        vec![Action::Read, Action::Read],
        vec![Action::Read; 5],
        vec![Action::ManageKeys],
        vec![Action::ManageMembers],
        vec![Action::Billing],
        vec![Action::DeleteTenant],
    ] {
        assert!(matches!(
            service.create_key(&ot, tenant, &input(scopes)).await,
            Err(Error::Forbidden)
        ));
    }
    for bad in [
        KeyInput {
            name: "".into(),
            ..input(vec![Action::Read])
        },
        KeyInput {
            expires_at: 0,
            ..input(vec![Action::Read])
        },
        KeyInput {
            expires_at: i64::MAX,
            ..input(vec![Action::Read])
        },
    ] {
        assert!(matches!(
            service.create_key(&ot, tenant, &bad).await,
            Err(Error::Validation)
        ));
    }
    assert!(matches!(
        service
            .create_key(&vt, tenant, &input(vec![Action::Read]))
            .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service
            .create_key("bad", tenant, &input(vec![Action::Read]))
            .await,
        Err(Error::Unauthenticated)
    ));
    // Keys cannot perform control-plane operations, including invitation acceptance.
    assert!(matches!(
        service
            .create_key(&issued.secret, tenant, &input(vec![Action::Read]))
            .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service.list_keys(&issued.secret, tenant, 25, None).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service.list_members(&issued.secret, tenant, 25, None).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service
            .accept_invitation(&issued.secret, tenant, Uuid::new_v4())
            .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service
            .list_invitations(&issued.secret, tenant, 25, None)
            .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service
            .rotate_key(&issued.secret, tenant, issued.key.key_id, 1)
            .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service
            .revoke_key(&issued.secret, tenant, issued.key.key_id, 1)
            .await,
        Err(Error::Forbidden)
    ));
    // Canonical parser, wrong secret and unknown locators never authenticate.
    let mut forged = issued.secret.clone();
    forged.replace_range(
        133..134,
        if &issued.secret[133..134] == "0" {
            "1"
        } else {
            "0"
        },
    );
    let unknown = format!(
        "kk1.{}.{}.{}",
        tenant.simple(),
        Uuid::new_v4().simple(),
        "a".repeat(64)
    );
    let mut malformed = vec![
        "kk1.".into(),
        "kk1.bad".into(),
        "kk1.".to_owned() + &"x".repeat(130),
        issued.secret.to_uppercase(),
        forged.clone(),
        unknown,
    ];
    malformed.push(format!(
        "kk1.{}.{}.{}",
        "z".repeat(32),
        issued.key.key_id.simple(),
        "a".repeat(64)
    ));
    malformed.push(format!(
        "kk1.{}.{}.{}",
        "a".repeat(31),
        "b".repeat(33),
        "c".repeat(64)
    ));
    malformed.push(format!(
        "kk1.{}.{}.{}",
        "a".repeat(32),
        "b".repeat(31),
        "c".repeat(65)
    ));
    malformed.push(format!(
        "kk1.{}.{}.{}.{}",
        "a".repeat(32),
        "b".repeat(32),
        "c".repeat(30),
        "d".repeat(33)
    ));
    for bad in malformed {
        assert!(matches!(
            service.get(&bad, tenant, project).await,
            Err(Error::Unauthenticated)
        ));
    }
    // Invalid verifiers fail before the membership SELECT; storage outage is closed.
    admin
        .batch_execute("REVOKE SELECT ON karate.memberships FROM karate_api_test")
        .await
        .unwrap();
    assert!(matches!(
        service.get(&forged, tenant, project).await,
        Err(Error::Unauthenticated)
    ));
    assert!(matches!(
        service.get(&issued.secret, tenant, project).await,
        Err(Error::Unavailable)
    ));
    admin
        .batch_execute("GRANT SELECT ON karate.memberships TO karate_api_test")
        .await
        .unwrap();
    admin
        .batch_execute(
            "REVOKE EXECUTE ON FUNCTION karate.key_verifier(uuid,uuid) FROM karate_api_test",
        )
        .await
        .unwrap();
    assert!(matches!(
        service.get(&issued.secret, tenant, project).await,
        Err(Error::Unavailable)
    ));
    admin
        .batch_execute(
            "GRANT EXECUTE ON FUNCTION karate.key_verifier(uuid,uuid) TO karate_api_test",
        )
        .await
        .unwrap();
    let writer = service
        .create_key(
            &at,
            tenant,
            &input(vec![
                Action::Read,
                Action::Write,
                Action::Run,
                Action::Export,
            ]),
        )
        .await
        .unwrap();
    assert!(
        service
            .create(&writer.secret, tenant, Uuid::new_v4(), "scoped write")
            .await
            .is_ok()
    );
    assert!(service.list(&writer.secret, tenant, 25, None).await.is_ok());
    let own = service.list_keys(&at, tenant, 100, None).await.unwrap();
    assert_eq!(own.items.len(), 1);
    assert_eq!(own.items[0].key_id, writer.key.key_id);
    assert!(matches!(
        service.revoke_key(&at, tenant, issued.key.key_id, 1).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service.rotate_key(&ot, tenant, writer.key.key_id, 1).await,
        Err(Error::Forbidden)
    ));
    let all = service.list_keys(&ot, tenant, 100, None).await.unwrap();
    assert_eq!(all.items.len(), 2);
    assert!(all.next_cursor.is_none());
    let first = service.list_keys(&ot, tenant, 1, None).await.unwrap();
    assert_eq!(
        service
            .list_keys(&ot, tenant, 100, first.next_cursor)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    for bad in [0, 101] {
        assert!(matches!(
            service.list_keys(&ot, tenant, bad, None).await,
            Err(Error::Validation)
        ));
    }
    assert!(service.list_keys(&vt, tenant, 25, None).await.is_err());
    assert!(service.list_keys("bad", tenant, 25, None).await.is_err());
    for bad in [0, i64::MAX] {
        assert!(matches!(
            service
                .rotate_key(&ot, tenant, issued.key.key_id, bad)
                .await,
            Err(Error::Validation)
        ));
    }
    assert!(matches!(
        service.revoke_key(&ot, tenant, Uuid::new_v4(), 1).await,
        Err(Error::NotFound)
    ));
    assert!(matches!(
        service
            .revoke_key("bad", tenant, issued.key.key_id, 1)
            .await,
        Err(Error::Unauthenticated)
    ));
    // Concurrent rotations admit one expected revision; old secret stops working.
    let (a, b) = tokio::join!(
        service.rotate_key(&ot, tenant, issued.key.key_id, 1),
        service.rotate_key(&ot, tenant, issued.key.key_id, 1)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let rotated = if let Ok(key) = a {
        assert!(matches!(b, Err(Error::Conflict)));
        key
    } else {
        b.unwrap()
    };
    assert_eq!(rotated.key.revision, 2);
    assert!(rotated.secret != issued.secret);
    assert!(matches!(
        service.get(&issued.secret, tenant, project).await,
        Err(Error::Unauthenticated)
    ));
    assert!(service.get(&rotated.secret, tenant, project).await.is_ok());
    // Atomic failures leave the original verifier/revision usable.
    let pending = input(vec![Action::Read]);
    admin
        .batch_execute("REVOKE INSERT ON karate.key_events FROM karate_membership_guard")
        .await
        .unwrap();
    assert!(matches!(
        service.create_key(&ot, tenant, &pending).await,
        Err(Error::Unavailable)
    ));
    assert!(matches!(
        service.rotate_key(&ot, tenant, rotated.key.key_id, 2).await,
        Err(Error::Unavailable)
    ));
    assert!(matches!(
        service.revoke_key(&ot, tenant, rotated.key.key_id, 2).await,
        Err(Error::Unavailable)
    ));
    admin
        .batch_execute("GRANT INSERT ON karate.key_events TO karate_membership_guard")
        .await
        .unwrap();
    assert!(service.get(&rotated.secret, tenant, project).await.is_ok());
    let recovered = service.create_key(&ot, tenant, &pending).await.unwrap();
    service
        .revoke_key(&ot, tenant, rotated.key.key_id, 2)
        .await
        .unwrap();
    assert!(matches!(
        service.get(&rotated.secret, tenant, project).await,
        Err(Error::Unauthenticated)
    ));
    assert!(matches!(
        service.rotate_key(&ot, tenant, rotated.key.key_id, 3).await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        service.revoke_key(&ot, tenant, rotated.key.key_id, 3).await,
        Err(Error::Conflict)
    ));
    // Expiry and membership change invalidate independently; restoration is explicit.
    admin
        .execute(
            "UPDATE karate.service_keys SET expires_at=0 WHERE key_id=$1",
            &[&recovered.key.key_id],
        )
        .await
        .unwrap();
    assert!(matches!(
        service.get(&recovered.secret, tenant, project).await,
        Err(Error::Unauthenticated)
    ));
    assert!(matches!(
        service
            .rotate_key(&ot, tenant, recovered.key.key_id, 1)
            .await,
        Err(Error::Conflict)
    ));
    service
        .revoke_key(&ot, tenant, recovered.key.key_id, 1)
        .await
        .unwrap();
    service
        .change_member(&ot, tenant, manager, 1, Some(Role::Member))
        .await
        .unwrap();
    assert!(matches!(
        service.get(&writer.secret, tenant, project).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service.rotate_key(&at, tenant, writer.key.key_id, 1).await,
        Err(Error::Forbidden)
    ));
    service
        .change_member(&ot, tenant, manager, 2, Some(Role::Admin))
        .await
        .unwrap();
    assert!(matches!(
        service.get(&writer.secret, tenant, project).await,
        Err(Error::Forbidden)
    ));
    let rebound = service
        .rotate_key(&at, tenant, writer.key.key_id, 1)
        .await
        .unwrap();
    assert!(service.get(&rebound.secret, tenant, project).await.is_ok());
    admin.execute("UPDATE karate.memberships SET expires_at=clock_timestamp()-interval '1 second' WHERE membership_id=$1",&[&manager]).await.unwrap();
    assert!(matches!(
        service.get(&rebound.secret, tenant, project).await,
        Err(Error::Forbidden)
    ));
    admin
        .execute(
            "UPDATE karate.memberships SET expires_at=NULL WHERE membership_id=$1",
            &[&manager],
        )
        .await
        .unwrap();
    service
        .change_member(&ot, tenant, manager, 3, None)
        .await
        .unwrap();
    assert!(matches!(
        service.get(&rebound.secret, tenant, project).await,
        Err(Error::Forbidden)
    ));
    service
        .revoke_key(&ot, tenant, writer.key.key_id, 2)
        .await
        .unwrap();
    // Key authentication is independent of a now-expired JWT key snapshot.
    let outage_key = service
        .create_key(&ot, tenant, &input(vec![Action::Read]))
        .await
        .unwrap();
    let expiry = unix_now().unwrap() + 1;
    let outage = ProjectService::connect(
        TokenVerifier::new("https://issuer.example", "karate-api", &jwks, expiry).unwrap(),
        &env("KARATE_TEST_DATABASE_URL"),
    )
    .await
    .unwrap();
    while unix_now().unwrap() < expiry {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(
        service
            .get(&outage_key.secret, tenant, project)
            .await
            .is_ok()
    );
    assert!(
        outage
            .get(&outage_key.secret, tenant, project)
            .await
            .is_ok()
    );
    assert!(matches!(
        outage.get(&ot, tenant, project).await,
        Err(Error::Unavailable)
    ));
    // HTTP one-time disclosure, no verifier in metadata, localized errors.
    let root = format!("/v1/tenants/{tenant}/keys");
    let http_input = input(vec![Action::Read]);
    let payload = json!({"key_id":http_input.key_id,"name":http_input.name,"scopes":["read"],"expires_at":http_input.expires_at});
    let (status, created) = request(&app, "POST", &root, Some(&ot), payload.clone()).await;
    assert_eq!(status, 200);
    assert!(created["secret"].as_str().unwrap().starts_with("kk1."));
    assert!(created["key"].get("verifier").is_none());
    let (status, page) = request(&app, "GET", &root, Some(&ot), Value::Null).await;
    assert_eq!(status, 200);
    assert!(!page.to_string().contains("kk1."));
    assert!(!page.to_string().contains("verifier"));
    assert!(!page.to_string().contains("secret"));
    for (method, path, body) in [
        ("POST", root.clone(), payload.clone()),
        ("GET", root.clone(), Value::Null),
        (
            "POST",
            format!("{root}/{}/rotate", http_input.key_id),
            json!({"expected_revision":1}),
        ),
        (
            "POST",
            format!("{root}/{}/revoke", http_input.key_id),
            json!({"expected_revision":1}),
        ),
    ] {
        assert_eq!(request(&app, method, &path, None, body).await.0, 401);
    }
    for (method, path, body) in [
        ("POST", "/v1/tenants/bad/keys".into(), payload),
        ("POST", root.clone(), json!({"extra":true})),
        ("GET", format!("{root}?limit=bad"), Value::Null),
        ("GET", "/v1/tenants/bad/keys".into(), Value::Null),
        (
            "POST",
            format!("{root}/bad/rotate"),
            json!({"expected_revision":1}),
        ),
        (
            "POST",
            format!("{root}/{}/rotate", http_input.key_id),
            json!({"extra":true}),
        ),
        (
            "POST",
            format!("{root}/bad/revoke"),
            json!({"expected_revision":1}),
        ),
        (
            "POST",
            format!("{root}/{}/revoke", http_input.key_id),
            json!({"extra":true}),
        ),
    ] {
        assert_eq!(request(&app, method, &path, Some(&ot), body).await.0, 400);
    }
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{root}/{}/rotate", http_input.key_id),
            Some(&ot),
            json!({"expected_revision":1})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{root}/{}/revoke", http_input.key_id),
            Some(&ot),
            json!({"expected_revision":2})
        )
        .await
        .0,
        200
    );
    let (status, _) = request(
        &app,
        "GET",
        &format!("/v1/tenants/{tenant}/projects/{project}"),
        Some(&outage_key.secret),
        Value::Null,
    )
    .await;
    assert_eq!(status, 200);
    // Raw runtime SQL has no credential-table access or audit write privilege.
    for sql in [
        "SELECT * FROM karate.service_keys",
        "UPDATE karate.service_keys SET revoked=false",
        "INSERT INTO karate.service_keys DEFAULT VALUES",
        "DELETE FROM karate.service_keys",
        "INSERT INTO karate.key_events DEFAULT VALUES",
        "UPDATE karate.key_events SET action='key.created'",
        "DELETE FROM karate.key_events",
    ] {
        assert!(runtime.batch_execute(sql).await.is_err());
    }
    assert!(
        runtime
            .query("SELECT * FROM karate.key_events", &[])
            .await
            .unwrap()
            .is_empty()
    );
    runtime.execute("SELECT set_config('karate.tenant_id',$1,false),set_config('karate.issuer','https://issuer.example',false),set_config('karate.subject','key-owner',false)",&[&tenant.to_string()]).await.unwrap();
    assert!(
        !runtime
            .query("SELECT * FROM karate.key_events", &[])
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        runtime
            .query("SELECT * FROM karate.list_keys($1,102,NULL)", &[&tenant])
            .await
            .is_err()
    );
    assert!(
        runtime
            .query("SELECT * FROM karate.list_keys($1,25,NULL)", &[&foreign])
            .await
            .is_err()
    );
    let verifier = vec![0u8; 32];
    let expiry = unix_now().unwrap() + 3600;
    for scopes in [vec!["manage_keys"], vec!["read", "read"], vec![]] {
        let failure = runtime
            .query(
                "SELECT * FROM karate.create_key($1,$2,'raw fixture',$3,$4,$5,$6)",
                &[
                    &tenant,
                    &Uuid::new_v4(),
                    &scopes,
                    &expiry,
                    &verifier,
                    &Uuid::new_v4(),
                ],
            )
            .await
            .unwrap_err();
        assert_eq!(failure.code().unwrap().code(), "K0001");
    }
    let bad_verifier = vec![0u8; 31];
    let failure = runtime
        .query(
            "SELECT * FROM karate.create_key($1,$2,'raw fixture',ARRAY['read'],$3,$4,$5)",
            &[
                &tenant,
                &Uuid::new_v4(),
                &expiry,
                &bad_verifier,
                &Uuid::new_v4(),
            ],
        )
        .await
        .unwrap_err();
    assert_eq!(failure.code().unwrap().code(), "K0004");
    runtime
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ")
        .await
        .unwrap();
    assert!(
        runtime
            .query(
                "SELECT * FROM karate.change_key($1,$2,1,NULL,$3)",
                &[&tenant, &outage_key.key.key_id, &Uuid::new_v4()]
            )
            .await
            .is_err()
    );
    runtime.batch_execute("ROLLBACK").await.unwrap();
    let row = admin
        .query_one(
            "SELECT count(*) FROM karate.memberships WHERE membership_id=$1 AND role='owner'",
            &[&owner],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, i64>(0), 1);
}
