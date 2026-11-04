use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use karate_api::{
    http::{Locale, router},
    identity::{TokenVerifier, unix_now},
    service::ProjectService,
};
use karate_core::Error;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

const A: Uuid = Uuid::from_u128(0x101);
const B: Uuid = Uuid::from_u128(0x102);
fn env(name: &str) -> String {
    std::env::var(name)
        .expect("run via make test (isolated signed-token/PostgreSQL fixture required)")
}
fn jwks() -> String {
    std::fs::read_to_string(env("KARATE_TEST_JWKS")).unwrap()
}
fn verifier() -> TokenVerifier {
    TokenVerifier::new(
        "https://issuer.example",
        "karate-api",
        &jwks(),
        unix_now().unwrap() + 3600,
    )
    .unwrap()
}
fn claims(subject: &str) -> Value {
    let now = unix_now().unwrap();
    json!({"iss":"https://issuer.example","sub":subject,"aud":"karate-api","iat":now-1,"exp":now+900})
}
fn token_with(payload: Value, header: Header) -> String {
    let key = EncodingKey::from_rsa_der(&std::fs::read(env("KARATE_TEST_KEY")).unwrap());
    encode(&header, &payload, &key).unwrap()
}
fn header() -> Header {
    let mut h = Header::new(Algorithm::RS256);
    h.kid = Some("fixture".into());
    h.typ = Some("at+jwt".into());
    h
}
fn token(subject: &str) -> String {
    token_with(claims(subject), header())
}

#[test]
fn signed_tokens_reject_forgery_expiry_audience_issuer_type_and_claim_errors() {
    let verifier = verifier();
    assert_eq!(verifier.verify(&token("alice")).unwrap().subject, "alice");
    let now = unix_now().unwrap();
    for (field, value) in [
        ("iss", json!("https://attacker.example")),
        ("aud", json!("id-token-client")),
        ("exp", json!(now - 1)),
        ("iat", json!(now + 100)),
        ("iat", json!(-1)),
        ("iat", json!(now - 7200)),
        ("exp", json!(0)),
        ("nbf", json!(now + 100)),
        ("sub", json!("")),
        ("sub", json!("x".repeat(256))),
    ] {
        let mut c = claims("alice");
        c[field] = value;
        assert!(
            matches!(
                verifier.verify(&token_with(c, header())),
                Err(Error::Unauthenticated)
            ),
            "{field}"
        );
    }
    for field in ["iss", "aud", "exp", "iat", "sub"] {
        let mut c = claims("alice");
        c.as_object_mut().unwrap().remove(field);
        assert!(matches!(
            verifier.verify(&token_with(c, header())),
            Err(Error::Unauthenticated)
        ));
    }
    for change in 0..4 {
        let mut h = header();
        match change {
            0 => h.kid = None,
            1 => h.kid = Some("unknown".into()),
            2 => h.typ = Some("JWT".into()),
            _ => h.typ = None,
        };
        assert!(matches!(
            verifier.verify(&token_with(claims("alice"), h)),
            Err(Error::Unauthenticated)
        ));
    }
    let mut h = Header::new(Algorithm::HS256);
    h.typ = Some("at+jwt".into());
    h.kid = Some("fixture".into());
    let wrong = encode(
        &h,
        &claims("alice"),
        &EncodingKey::from_secret(b"isolated-test-signature"),
    )
    .unwrap();
    assert!(matches!(
        verifier.verify(&wrong),
        Err(Error::Unauthenticated)
    ));
    let mut critical = header();
    critical.crit = Some(vec!["unrecognized".into()]);
    assert!(matches!(
        verifier.verify(&token_with(claims("alice"), critical)),
        Err(Error::Unauthenticated)
    ));
    let mut encrypted = header();
    encrypted.enc = Some(serde_json::from_value(json!("A128GCM")).unwrap());
    assert!(matches!(
        verifier.verify(&token_with(claims("alice"), encrypted)),
        Err(Error::Unauthenticated)
    ));
    let mut compressed = header();
    compressed.zip = Some(serde_json::from_value(json!("DEF")).unwrap());
    assert!(matches!(
        verifier.verify(&token_with(claims("alice"), compressed)),
        Err(Error::Unauthenticated)
    ));
    let valid = token("alice");
    let mut parts: Vec<_> = valid.split('.').map(str::to_owned).collect();
    let replacement = if &parts[2][0..1] == "A" { "B" } else { "A" };
    parts[2].replace_range(0..1, replacement);
    for invalid in [
        String::new(),
        "not.jwt".into(),
        "x".repeat(8193),
        parts.join("."),
    ] {
        assert!(matches!(
            verifier.verify(&invalid),
            Err(Error::Unauthenticated)
        ));
    }
}

#[tokio::test]
async fn key_configuration_is_bounded_explicit_and_expires() {
    let now = unix_now().unwrap();
    for issuer in [
        "invalid",
        "http://issuer.example",
        "https://u@issuer.example",
        "https://:password@issuer.example",
        "https://issuer.example?q=1",
        "https://issuer.example#x",
    ] {
        assert!(TokenVerifier::new(issuer, "api", &jwks(), now + 100).is_err());
    }
    for keys in ["bad".into(), "x".repeat(65537), r#"{"keys":[]}"#.into()] {
        assert!(TokenVerifier::new("https://issuer.example", "api", &keys, now + 100).is_err());
    }
    let original: Value = serde_json::from_str(&jwks()).unwrap();
    assert!(
        TokenVerifier::new(
            &format!("https://issuer.example/{}", "x".repeat(2048)),
            "api",
            &jwks(),
            now + 100
        )
        .is_err()
    );
    let ec = json!({"keys":[{"kty":"EC","crv":"P-256","x":"foo","y":"bar","alg":"RS256","kid":"fixture","use":"sig"}]});
    assert!(
        TokenVerifier::new("https://issuer.example", "api", &ec.to_string(), now + 100).is_err()
    );
    for (field, value) in [
        ("alg", json!("RS512")),
        ("use", json!("enc")),
        ("kid", json!("")),
        ("kid", json!("x".repeat(129))),
        ("n", json!("!")),
    ] {
        let mut k = original.clone();
        k["keys"][0][field] = value;
        assert!(
            TokenVerifier::new("https://issuer.example", "api", &k.to_string(), now + 100).is_err()
        );
    }
    for count in [2, 33] {
        let duplicate = json!({"keys":vec![original["keys"][0].clone();count]});
        assert!(
            TokenVerifier::new(
                "https://issuer.example",
                "api",
                &duplicate.to_string(),
                now + 100
            )
            .is_err()
        );
    }
    for expiry in [now, now + 86401] {
        assert!(TokenVerifier::new("https://issuer.example", "api", &jwks(), expiry).is_err());
    }
    for aud in ["".to_string(), "x".repeat(256)] {
        assert!(TokenVerifier::new("https://issuer.example", &aud, &jwks(), now + 100).is_err());
    }
    let expiring = TokenVerifier::new("https://issuer.example", "api", &jwks(), now + 1).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert!(matches!(
        expiring.verify("anything"),
        Err(Error::Unavailable)
    ));
}

#[test]
fn language_negotiation_and_catalogs_are_complete() {
    for value in ["pt-BR", "pt-PT", "en;q=0.2,pt-BR;q=0.9", "pt;q=1,en;q=0"] {
        assert_eq!(Locale::negotiate(value), Locale::PtBr);
    }
    for value in [
        "",
        "fr",
        "en-US",
        "pt;q=0",
        "pt;q=NaN",
        "pt;q=2",
        "pt;q=-1",
        "pt;x=1",
        "pt;q=1;x=2",
        "*",
        "pt;q=oops",
        &"x".repeat(1025),
    ] {
        assert_eq!(Locale::negotiate(value), Locale::En);
    }
    let en: Value = serde_json::from_str(include_str!("../locales/en.json")).unwrap();
    let pt: Value = serde_json::from_str(include_str!("../locales/pt-BR.json")).unwrap();
    assert_eq!(
        en.as_object().unwrap().keys().collect::<Vec<_>>(),
        pt.as_object().unwrap().keys().collect::<Vec<_>>()
    );
    for catalog in [en, pt] {
        for pair in catalog.as_object().unwrap().values() {
            assert_eq!(pair.as_array().unwrap().len(), 2);
            assert!(
                pair.as_array()
                    .unwrap()
                    .iter()
                    .all(|v| !v.as_str().unwrap().is_empty())
            );
        }
    }
}

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    credential: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .header("accept-language", "pt-BR");
    if let Some(value) = credential {
        builder = builder.header("authorization", format!("Bearer {value}"));
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    assert_eq!(response.headers()["content-language"], "pt-BR");
    assert_eq!(response.headers()["cache-control"], "no-store");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, body)
}

#[tokio::test]
async fn real_postgres_http_and_shared_service_enforce_tenants_revisions_and_atomic_audit() {
    let service = ProjectService::connect(verifier(), &env("KARATE_TEST_DATABASE_URL"))
        .await
        .unwrap();
    let app = router(service.clone());
    let alice = token("alice");
    let bob = token("bob");
    let viewer = token("viewer");
    let id = Uuid::from_u128(10);
    let base = format!("/v1/tenants/{A}/projects");
    let path = format!("{base}/{id}");
    let (admin, connection) =
        tokio_postgres::connect(&env("KARATE_TEST_ADMIN_URL"), tokio_postgres::NoTls)
            .await
            .unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    assert!(matches!(
        ProjectService::connect(verifier(), &env("KARATE_TEST_ADMIN_URL")).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        ProjectService::connect(verifier(), "host=localhost user=any").await,
        Err(Error::Validation)
    ));
    assert!(matches!(
        ProjectService::connect(verifier(), "host=/tmp hostaddr=127.0.0.1 user=any").await,
        Err(Error::Validation)
    ));
    assert!(matches!(
        ProjectService::connect(verifier(), "bad-config").await,
        Err(Error::Unavailable)
    ));
    assert!(matches!(
        ProjectService::connect(verifier(), "user=any").await,
        Err(Error::Validation)
    ));
    assert!(matches!(
        ProjectService::connect(verifier(), "host=/missing-karate-test-socket user=any").await,
        Err(Error::Unavailable)
    ));
    let payload = json!({"project_id":id,"name":"Shared name"});
    for auth in [
        "Bearer ",
        "Basic rejected",
        "Bearer forged",
        &format!("Bearer {}", "x".repeat(8193)),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&base)
                    .header("authorization", auth)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
        assert_eq!(response.headers()["www-authenticate"], "Bearer");
    }
    let duplicate = Request::builder()
        .uri(&base)
        .header("authorization", format!("Bearer {alice}"))
        .header("authorization", format!("Bearer {bob}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(duplicate).await.unwrap().status(), 401);
    let lower = Request::builder()
        .uri(&base)
        .header("authorization", format!("bearer {alice}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(lower).await.unwrap().status(), 200);
    assert_eq!(
        request(&app, "GET", "/healthz", None, Value::Null).await.0,
        204
    );
    assert_eq!(request(&app, "GET", &base, None, Value::Null).await.0, 401);
    assert_eq!(
        request(&app, "POST", &base, Some("forged"), payload.clone())
            .await
            .0,
        401
    );
    let (status, created) = request(&app, "POST", &base, Some(&alice), payload.clone()).await;
    assert_eq!(status, 201);
    assert_eq!(created["revision"], 1);
    assert_eq!(
        request(&app, "POST", &base, Some(&alice), payload.clone())
            .await
            .0,
        200
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &base,
            Some(&alice),
            json!({"project_id":id,"name":"other"})
        )
        .await
        .0,
        409
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/v1/tenants/{B}/projects"),
            Some(&bob),
            payload.clone()
        )
        .await
        .0,
        201
    );
    for method in ["GET", "PATCH"] {
        assert_eq!(
            request(
                &app,
                method,
                &path,
                Some(&bob),
                json!({"name":"attack","expected_revision":1})
            )
            .await
            .0,
            403
        );
    }
    assert_eq!(
        request(&app, "GET", &path, Some(&viewer), Value::Null)
            .await
            .0,
        200
    );
    assert_eq!(
        request(&app, "POST", &base, Some(&viewer), payload.clone())
            .await
            .0,
        403
    );
    assert!(matches!(
        service.update(&viewer, A, id, "attack", 1).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service.get(&alice, B, id).await,
        Err(Error::Forbidden)
    ));
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("{base}/{}", Uuid::from_u128(999)),
            Some(&alice),
            Value::Null
        )
        .await
        .0,
        404
    );
    for invalid in [
        json!({"project_id":id,"name":""}),
        json!({"project_id":id,"name":"x","role":"owner"}),
        json!({"project_id":"bad","name":"x"}),
    ] {
        let (status, error) = request(&app, "POST", &base, Some(&alice), invalid).await;
        assert_eq!(status, 400);
        assert_eq!(error["code"], "VALIDATION");
        assert_eq!(error.as_object().unwrap().len(), 5);
    }
    let (status, _) = request(
        &app,
        "POST",
        &base,
        Some(&alice),
        json!({"project_id":id,"name":"x".repeat(9000)}),
    )
    .await;
    assert_eq!(status, 400);
    for suffix in [
        "?limit=0",
        "?limit=101",
        "?limit=x",
        "?after=bad",
        "?extra=x",
    ] {
        assert_eq!(
            request(
                &app,
                "GET",
                &format!("{base}{suffix}"),
                Some(&alice),
                Value::Null
            )
            .await
            .0,
            400
        );
    }
    assert_eq!(
        request(
            &app,
            "GET",
            "/v1/tenants/bad/projects",
            Some(&alice),
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
            &format!("{base}/bad"),
            Some(&alice),
            Value::Null
        )
        .await
        .0,
        400
    );
    assert_eq!(
        request(
            &app,
            "PATCH",
            &format!("{base}/bad"),
            Some(&alice),
            json!({"name":"x","expected_revision":1})
        )
        .await
        .0,
        400
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/v1/tenants/bad/projects",
            Some(&alice),
            payload.clone()
        )
        .await
        .0,
        400
    );
    assert_eq!(
        request(&app, "GET", "/missing", None, Value::Null).await.0,
        404
    );
    assert_eq!(
        request(&app, "DELETE", &path, Some(&alice), Value::Null)
            .await
            .0,
        400
    );
    service
        .create(&alice, A, Uuid::from_u128(20), "Second")
        .await
        .unwrap();
    let first = service.list(&alice, A, 1, None).await.unwrap();
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.next_cursor, Some(id));
    let second = service.list(&alice, A, 1, first.next_cursor).await.unwrap();
    assert_eq!(second.items[0].project_id, Uuid::from_u128(20));
    assert_eq!(second.next_cursor, None);
    assert!(
        service
            .list(&alice, A, 10, Some(Uuid::from_u128(30)))
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        request(&app, "GET", &base, Some(&alice), Value::Null)
            .await
            .1["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    for revision in [0, -1, i64::MAX] {
        assert!(matches!(
            service.update(&alice, A, id, "x", revision).await,
            Err(Error::Validation)
        ));
    }
    let (one, two) = tokio::join!(
        service.update(&alice, A, id, "writer one", 1),
        service.update(&alice, A, id, "writer two", 1)
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert!(matches!(one, Err(Error::Conflict)) || matches!(two, Err(Error::Conflict)));
    assert_eq!(
        request(
            &app,
            "PATCH",
            &path,
            Some(&alice),
            json!({"name":"HTTP update","expected_revision":2})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(&app, "PATCH", &path, Some(&alice), json!({"name":"bad"}))
            .await
            .0,
        400
    );
    assert!(matches!(
        service
            .update(&alice, A, Uuid::from_u128(999), "missing", 1)
            .await,
        Err(Error::NotFound)
    ));
    // Failed audit must roll back the successful project update.
    admin
        .batch_execute("REVOKE INSERT ON karate.audit_events FROM karate_api_test")
        .await
        .unwrap();
    assert!(matches!(
        service.update(&alice, A, id, "must roll back", 3).await,
        Err(Error::Unavailable)
    ));
    assert_eq!(service.get(&alice, A, id).await.unwrap().revision, 3);
    admin
        .batch_execute("GRANT INSERT ON karate.audit_events TO karate_api_test")
        .await
        .unwrap();
    let count: i64 = admin
        .query_one(
            "SELECT count(*) FROM karate.audit_events WHERE tenant_id=$1",
            &[&A],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 4);
    // Audit permissions and RLS are exercised as the actual runtime DB role.
    let (mut runtime, runtime_connection) =
        tokio_postgres::connect(&env("KARATE_TEST_DATABASE_URL"), tokio_postgres::NoTls)
            .await
            .unwrap();
    tokio::spawn(async move {
        let _ = runtime_connection.await;
    });
    assert_eq!(
        runtime
            .query_one("SELECT count(*) FROM karate.audit_events", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    for subject in ["viewer", "bob"] {
        let tx = runtime.transaction().await.unwrap();
        tx.execute("SELECT set_config('karate.tenant_id',$1,true),set_config('karate.issuer','https://issuer.example',true),set_config('karate.subject',$2,true)",&[&A.to_string(),&subject]).await.unwrap();
        assert_eq!(
            tx.query_one("SELECT count(*) FROM karate.audit_events", &[])
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        tx.rollback().await.unwrap();
    }
    let tx = runtime.transaction().await.unwrap();
    tx.execute("SELECT set_config('karate.tenant_id',$1,true),set_config('karate.issuer','https://issuer.example',true),set_config('karate.subject','alice',true)",&[&A.to_string()]).await.unwrap();
    assert_eq!(
        tx.query_one("SELECT count(*) FROM karate.audit_events", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        4
    );
    assert!(tx.execute("INSERT INTO karate.audit_events (tenant_id,event_id,actor_issuer,actor_subject,action,project_id) VALUES ($1,$2,'https://issuer.example','bob','project.created',$3)",&[&A,&Uuid::new_v4(),&id]).await.is_err());
    tx.rollback().await.unwrap();
    assert!(
        runtime
            .execute("UPDATE karate.audit_events SET actor_subject='forged'", &[])
            .await
            .is_err()
    );
    assert!(
        runtime
            .execute("DELETE FROM karate.audit_events", &[])
            .await
            .is_err()
    );
    assert_eq!(service.get(&bob, B, id).await.unwrap().name, "Shared name");
    admin
        .execute(
            "UPDATE karate.memberships SET revoked_at=now() WHERE tenant_id=$1 AND subject='alice'",
            &[&A],
        )
        .await
        .unwrap();
    assert!(matches!(
        service.get(&alice, A, id).await,
        Err(Error::Forbidden)
    ));
    admin.execute("UPDATE karate.memberships SET expires_at=now()-interval '1 second' WHERE tenant_id=$1 AND subject='viewer'",&[&A]).await.unwrap();
    assert!(matches!(
        service.get(&viewer, A, id).await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service.get(&token("unknown"), A, id).await,
        Err(Error::Forbidden)
    ));
    // Transport test over a real loopback TCP listener, with signed token and DB read.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    socket.write_all(format!("GET /v1/tenants/{B}/projects/{id} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {bob}\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).await.unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains("Shared name"));
    server.abort();
    // Remove DB permissions: invalid credentials still fail before any DB access.
    admin
        .batch_execute("REVOKE SELECT ON karate.memberships FROM karate_api_test")
        .await
        .unwrap();
    assert!(matches!(
        service.get("forged", A, id).await,
        Err(Error::Unauthenticated)
    ));
    assert!(matches!(
        service.get(&bob, B, id).await,
        Err(Error::Unavailable)
    ));
    admin
        .batch_execute("GRANT SELECT ON karate.memberships TO karate_api_test")
        .await
        .unwrap();
}
