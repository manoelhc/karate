use axum::{Router, body::Body, http::Request};
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use karate_api::{
    http::router,
    identity::{TokenVerifier, unix_now},
    invitations::InviteInput,
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

fn offer(subject: &str, role: Role) -> InviteInput {
    InviteInput {
        invitation_id: Uuid::new_v4(),
        subject: subject.into(),
        role,
        expires_at: unix_now().unwrap() + 3600,
    }
}
#[tokio::test]
async fn invitations_require_bound_identity_current_grants_and_single_atomic_acceptance() {
    let admin = db("KARATE_TEST_ADMIN_URL").await;
    let runtime = db("KARATE_TEST_DATABASE_URL").await;
    let tenant = Uuid::from_u128(0x401);
    let foreign = Uuid::from_u128(0x402);
    for t in [tenant, foreign] {
        admin
            .execute(
                "INSERT INTO karate.tenants(tenant_id,name) VALUES($1,'Invitation fixture')",
                &[&t],
            )
            .await
            .unwrap();
    }
    let owner = add(&admin, tenant, "inviter", "owner").await;
    let manager = add(&admin, tenant, "manager", "admin").await;
    add(&admin, tenant, "viewer", "viewer").await;
    add(&admin, foreign, "foreign-owner", "owner").await;
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
    let app = router(service.clone());
    let ot = token("inviter");
    let at = token("manager");
    let vt = token("viewer");
    let input = offer("new-member", Role::Member);
    let invite = service
        .create_invitation(&ot, tenant, &input)
        .await
        .unwrap();
    assert_eq!(invite.state, "pending");
    assert_eq!(
        service
            .create_invitation(&ot, tenant, &input)
            .await
            .unwrap()
            .invitation_id,
        invite.invitation_id
    );
    let changed = InviteInput {
        subject: "someone-else".into(),
        ..offer("", Role::Viewer)
    };
    let changed = InviteInput {
        invitation_id: input.invitation_id,
        ..changed
    };
    assert!(matches!(
        service.create_invitation(&ot, tenant, &changed).await,
        Err(Error::Conflict)
    ));
    for bad in [
        InviteInput {
            subject: "".into(),
            ..offer("", Role::Member)
        },
        InviteInput {
            subject: "x".repeat(256),
            ..offer("", Role::Member)
        },
        InviteInput {
            expires_at: unix_now().unwrap(),
            ..offer("", Role::Member)
        },
        InviteInput {
            expires_at: i64::MAX,
            ..offer("", Role::Member)
        },
    ] {
        assert!(matches!(
            service.create_invitation(&ot, tenant, &bad).await,
            Err(Error::Validation)
        ));
    }
    for c in [&vt, "invalid"] {
        assert!(
            service
                .create_invitation(c, tenant, &offer("blocked", Role::Viewer))
                .await
                .is_err()
        );
    }
    assert!(matches!(
        service
            .create_invitation(&ot, tenant, &offer("blocked", Role::Owner))
            .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service
            .create_invitation(&at, tenant, &offer("blocked", Role::Admin))
            .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service
            .create_invitation(&ot, tenant, &offer("viewer", Role::Member))
            .await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        service
            .create_invitation(&ot, foreign, &offer("blocked", Role::Viewer))
            .await,
        Err(Error::Forbidden)
    ));
    for c in [&ot, &at, &token("other"), "invalid"] {
        assert!(
            service
                .accept_invitation(c, tenant, input.invitation_id)
                .await
                .is_err()
        );
    }
    assert!(matches!(
        service
            .accept_invitation(&token("new-member"), foreign, input.invitation_id)
            .await,
        Err(Error::Forbidden)
    ));
    let accepted = service
        .accept_invitation(&token("new-member"), tenant, input.invitation_id)
        .await
        .unwrap();
    assert_eq!(accepted.subject, "new-member");
    assert_eq!(accepted.role, Role::Member);
    assert_eq!(accepted.revision, 1);
    assert!(matches!(
        service
            .accept_invitation(&token("new-member"), tenant, input.invitation_id)
            .await,
        Err(Error::Conflict)
    ));
    assert!(
        service
            .create(
                &token("new-member"),
                tenant,
                Uuid::new_v4(),
                "accepted access"
            )
            .await
            .is_ok()
    );
    // Two offers for an absent identity capture absence; only one can enroll it.
    let first = offer("parallel", Role::Member);
    let second = offer("parallel", Role::Viewer);
    service
        .create_invitation(&at, tenant, &first)
        .await
        .unwrap();
    service
        .create_invitation(&at, tenant, &second)
        .await
        .unwrap();
    let pt = token("parallel");
    let (one, two) = tokio::join!(
        service.accept_invitation(&pt, tenant, first.invitation_id),
        service.accept_invitation(&pt, tenant, second.invitation_id)
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert!(matches!(one, Err(Error::Conflict)) || matches!(two, Err(Error::Conflict)));
    // Same offer is single-use even with concurrent acceptance.
    let once = offer("once", Role::Viewer);
    service.create_invitation(&ot, tenant, &once).await.unwrap();
    let once_token = token("once");
    let (a, b) = tokio::join!(
        service.accept_invitation(&once_token, tenant, once.invitation_id),
        service.accept_invitation(&once_token, tenant, once.invitation_id)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    // Revocation / expiry cannot be replayed; no membership is introduced.
    let cancelled = offer("cancelled", Role::Viewer);
    service
        .create_invitation(&at, tenant, &cancelled)
        .await
        .unwrap();
    assert_eq!(
        service
            .revoke_invitation(&at, tenant, cancelled.invitation_id)
            .await
            .unwrap()
            .state,
        "revoked"
    );
    assert!(matches!(
        service
            .accept_invitation(&token("cancelled"), tenant, cancelled.invitation_id)
            .await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        service
            .revoke_invitation(&at, tenant, cancelled.invitation_id)
            .await,
        Err(Error::Conflict)
    ));
    let expired = offer("expired", Role::Viewer);
    service
        .create_invitation(&ot, tenant, &expired)
        .await
        .unwrap();
    admin
        .execute(
            "UPDATE karate.invitations SET expires_at=0 WHERE invitation_id=$1",
            &[&expired.invitation_id],
        )
        .await
        .unwrap();
    assert!(matches!(
        service
            .accept_invitation(&token("expired"), tenant, expired.invitation_id)
            .await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        service
            .revoke_invitation(&ot, tenant, expired.invitation_id)
            .await,
        Err(Error::Conflict)
    ));
    // Re-enrollment preserves membership identity and invalidates competing offers.
    service
        .change_member(&ot, tenant, accepted.membership_id, 1, None)
        .await
        .unwrap();
    let restore = offer("new-member", Role::Viewer);
    let stale = offer("new-member", Role::Member);
    service
        .create_invitation(&at, tenant, &restore)
        .await
        .unwrap();
    service
        .create_invitation(&at, tenant, &stale)
        .await
        .unwrap();
    let restored = service
        .accept_invitation(&token("new-member"), tenant, restore.invitation_id)
        .await
        .unwrap();
    assert_eq!(restored.membership_id, accepted.membership_id);
    assert_eq!(restored.revision, 3);
    assert!(!restored.revoked);
    service
        .change_member(&ot, tenant, restored.membership_id, 3, None)
        .await
        .unwrap();
    assert!(matches!(
        service
            .accept_invitation(&token("new-member"), tenant, stale.invitation_id)
            .await,
        Err(Error::Conflict)
    ));
    let former = add(&admin, tenant, "former-admin", "admin").await;
    service
        .change_member(&ot, tenant, former, 1, None)
        .await
        .unwrap();
    assert!(matches!(
        service
            .create_invitation(&at, tenant, &offer("former-admin", Role::Viewer))
            .await,
        Err(Error::Forbidden)
    ));
    let restore_admin = offer("former-admin", Role::Admin);
    service
        .create_invitation(&ot, tenant, &restore_admin)
        .await
        .unwrap();
    assert!(matches!(
        service
            .revoke_invitation(&at, tenant, restore_admin.invitation_id)
            .await,
        Err(Error::Forbidden)
    ));
    assert_eq!(
        service
            .accept_invitation(&token("former-admin"), tenant, restore_admin.invitation_id)
            .await
            .unwrap()
            .role,
        Role::Admin
    );
    // A changed/revoked inviter cannot leave live delegation behind, even after restoration.
    let sponsored = offer("sponsored", Role::Member);
    service
        .create_invitation(&at, tenant, &sponsored)
        .await
        .unwrap();
    service
        .change_member(&ot, tenant, manager, 1, Some(Role::Member))
        .await
        .unwrap();
    assert!(matches!(
        service
            .accept_invitation(&token("sponsored"), tenant, sponsored.invitation_id)
            .await,
        Err(Error::Forbidden)
    ));
    service
        .change_member(&ot, tenant, manager, 2, Some(Role::Admin))
        .await
        .unwrap();
    assert!(matches!(
        service
            .accept_invitation(&token("sponsored"), tenant, sponsored.invitation_id)
            .await,
        Err(Error::Forbidden)
    ));
    let revoked_sponsor = offer("sponsor-revoked", Role::Member);
    service
        .create_invitation(&at, tenant, &revoked_sponsor)
        .await
        .unwrap();
    service
        .change_member(&ot, tenant, manager, 3, None)
        .await
        .unwrap();
    assert!(matches!(
        service
            .accept_invitation(
                &token("sponsor-revoked"),
                tenant,
                revoked_sponsor.invitation_id
            )
            .await,
        Err(Error::Forbidden)
    ));
    let expiring_sponsor = add(&admin, tenant, "expiring-inviter", "admin").await;
    let short_offer = offer("expired-sponsor-recipient", Role::Viewer);
    service
        .create_invitation(&token("expiring-inviter"), tenant, &short_offer)
        .await
        .unwrap();
    admin.execute("UPDATE karate.memberships SET expires_at=clock_timestamp()-interval '1 second' WHERE membership_id=$1",&[&expiring_sponsor]).await.unwrap();
    assert!(matches!(
        service
            .accept_invitation(
                &token("expired-sponsor-recipient"),
                tenant,
                short_offer.invitation_id
            )
            .await,
        Err(Error::Forbidden)
    ));
    // Audit failure rolls back both initial issue and acceptance; retry remains possible.
    let rollback = offer("rollback", Role::Viewer);
    admin
        .batch_execute("REVOKE INSERT ON karate.invitation_events FROM karate_membership_guard")
        .await
        .unwrap();
    assert!(matches!(
        service.create_invitation(&ot, tenant, &rollback).await,
        Err(Error::Unavailable)
    ));
    admin
        .batch_execute("GRANT INSERT ON karate.invitation_events TO karate_membership_guard")
        .await
        .unwrap();
    service
        .create_invitation(&ot, tenant, &rollback)
        .await
        .unwrap();
    admin
        .batch_execute("REVOKE INSERT ON karate.invitation_events FROM karate_membership_guard")
        .await
        .unwrap();
    assert!(matches!(
        service
            .accept_invitation(&token("rollback"), tenant, rollback.invitation_id)
            .await,
        Err(Error::Unavailable)
    ));
    assert!(matches!(
        service
            .revoke_invitation(&ot, tenant, rollback.invitation_id)
            .await,
        Err(Error::Unavailable)
    ));
    admin
        .batch_execute("GRANT INSERT ON karate.invitation_events TO karate_membership_guard")
        .await
        .unwrap();
    assert_eq!(
        service
            .accept_invitation(&token("rollback"), tenant, rollback.invitation_id)
            .await
            .unwrap()
            .revision,
        1
    );
    let all = service
        .list_invitations(&ot, tenant, 100, None)
        .await
        .unwrap();
    assert!(all.items.iter().any(|i| i.state == "expired"));
    assert!(all.next_cursor.is_none());
    let page = service
        .list_invitations(&ot, tenant, 1, None)
        .await
        .unwrap();
    let rest = service
        .list_invitations(&ot, tenant, 100, page.next_cursor)
        .await
        .unwrap();
    assert_eq!(rest.items.len() + 1, all.items.len());
    for bad in [0, 101] {
        assert!(matches!(
            service.list_invitations(&ot, tenant, bad, None).await,
            Err(Error::Validation)
        ));
    }
    assert!(
        service
            .list_invitations(&vt, tenant, 25, None)
            .await
            .is_err()
    );
    assert!(
        service
            .list_invitations("bad", tenant, 25, None)
            .await
            .is_err()
    );
    assert!(
        service
            .revoke_invitation(&vt, tenant, input.invitation_id)
            .await
            .is_err()
    );
    assert!(
        service
            .revoke_invitation("bad", tenant, input.invitation_id)
            .await
            .is_err()
    );
    assert!(matches!(
        service.revoke_invitation(&ot, tenant, Uuid::new_v4()).await,
        Err(Error::NotFound)
    ));
    // Real HTTP handlers, localization and malformed input contracts.
    let root = format!("/v1/tenants/{tenant}/invitations");
    let http = offer("http-member", Role::Member);
    let payload = json!({"invitation_id":http.invitation_id,"subject":http.subject,"role":"member","expires_at":http.expires_at});
    assert_eq!(
        request(&app, "POST", &root, Some(&ot), payload.clone())
            .await
            .0,
        200
    );
    assert_eq!(
        request(&app, "POST", &root, None, payload.clone()).await.0,
        401
    );
    assert_eq!(
        request(&app, "POST", &root, Some(&ot), json!({"extra":true}))
            .await
            .0,
        400
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/tenants/bad/invitations",
            Some(&ot),
            payload
        )
        .await
        .0,
        400
    );
    assert_eq!(
        request(&app, "GET", &root, Some(&ot), Value::Null).await.0,
        200
    );
    assert_eq!(request(&app, "GET", &root, None, Value::Null).await.0, 401);
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("{root}?limit=bad"),
            Some(&ot),
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
            "/v1/tenants/bad/invitations",
            Some(&ot),
            Value::Null
        )
        .await
        .0,
        400
    );
    let accept_path = format!("{root}/{}/accept", http.invitation_id);
    assert_eq!(
        request(&app, "POST", &accept_path, None, Value::Null)
            .await
            .0,
        401
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{root}/bad/accept"),
            Some(&ot),
            Value::Null
        )
        .await
        .0,
        400
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &accept_path,
            Some(&token("http-member")),
            Value::Null
        )
        .await
        .0,
        200
    );
    let revoke_http = offer("http-revoke", Role::Viewer);
    service
        .create_invitation(&ot, tenant, &revoke_http)
        .await
        .unwrap();
    let revoke_path = format!("{root}/{}/revoke", revoke_http.invitation_id);
    assert_eq!(
        request(&app, "POST", &revoke_path, None, Value::Null)
            .await
            .0,
        401
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("{root}/bad/revoke"),
            Some(&ot),
            Value::Null
        )
        .await
        .0,
        400
    );
    assert_eq!(
        request(&app, "POST", &revoke_path, Some(&ot), Value::Null)
            .await
            .0,
        200
    );
    // No raw writes, unscoped events, wrong-issuer acceptance or function exposure.
    for sql in [
        "INSERT INTO karate.invitations DEFAULT VALUES",
        "UPDATE karate.invitations SET state='pending'",
        "DELETE FROM karate.invitations",
        "INSERT INTO karate.invitation_events DEFAULT VALUES",
        "UPDATE karate.invitation_events SET action='invitation.accepted'",
        "DELETE FROM karate.invitation_events",
    ] {
        assert!(runtime.batch_execute(sql).await.is_err());
    }
    assert_eq!(
        runtime
            .query("SELECT * FROM karate.invitation_events", &[])
            .await
            .unwrap()
            .len(),
        0
    );
    runtime.execute("SELECT set_config('karate.tenant_id',$1,false),set_config('karate.issuer','https://wrong.example',false),set_config('karate.subject','new-member',false)",&[&tenant.to_string()]).await.unwrap();
    assert!(
        runtime
            .query(
                "SELECT * FROM karate.accept_invitation($1,$2,$3)",
                &[&tenant, &input.invitation_id, &Uuid::new_v4()]
            )
            .await
            .is_err()
    );
    runtime.batch_execute("SELECT set_config('karate.issuer','https://issuer.example',false),set_config('karate.subject','inviter',false)").await.unwrap();
    assert!(
        !runtime
            .query("SELECT * FROM karate.invitation_events", &[])
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        runtime
            .query(
                "SELECT * FROM karate.list_invitations($1,102,NULL)",
                &[&tenant]
            )
            .await
            .is_err()
    );
    assert!(
        runtime
            .query(
                "SELECT * FROM karate.list_invitations($1,25,NULL)",
                &[&foreign]
            )
            .await
            .is_err()
    );
    let raw = offer("raw-bound", Role::Viewer);
    service.create_invitation(&ot, tenant, &raw).await.unwrap();
    runtime
        .batch_execute("SELECT set_config('karate.subject','raw-bound',false)")
        .await
        .unwrap();
    runtime
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ")
        .await
        .unwrap();
    let failure = runtime
        .query(
            "SELECT * FROM karate.accept_invitation($1,$2,$3)",
            &[&tenant, &raw.invitation_id, &Uuid::new_v4()],
        )
        .await
        .unwrap_err();
    assert_eq!(failure.code().unwrap().code(), "K0004");
    runtime.batch_execute("ROLLBACK").await.unwrap();
    assert!(
        runtime
            .query("SELECT karate.invitation_role('owner',NULL,'owner')", &[])
            .await
            .is_err()
    );
    let unsafe_functions=admin.query_one("SELECT EXISTS(SELECT 1 FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace CROSS JOIN LATERAL aclexplode(p.proacl) a WHERE n.nspname='karate' AND a.grantee=0)",&[]).await.unwrap();
    assert!(!unsafe_functions.get::<_, bool>(0));
    assert_eq!(admin.query_one("SELECT count(*) FROM karate.memberships WHERE tenant_id=$1 AND membership_id=$2 AND role='owner'",&[&tenant,&owner]).await.unwrap().get::<_,i64>(0),1);
}
