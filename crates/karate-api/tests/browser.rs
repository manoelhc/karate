#[path = "support/provider.rs"]
mod provider;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use karate_api::{
    browser::{BrowserProvider, LoginStart},
    http::router,
    identity::{TokenVerifier, unix_now},
    service::ProjectService,
};
use karate_core::Error;
use provider::Provider;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc};
use tower::ServiceExt;
use uuid::Uuid;
const ORIGIN: &str = "https://karate.example";
fn env(n: &str) -> String {
    std::env::var(n).expect("make test supplies isolated fixture")
}
fn verifier() -> TokenVerifier {
    TokenVerifier::new(
        "https://issuer.example",
        "karate-api",
        &std::fs::read_to_string(env("KARATE_TEST_JWKS")).unwrap(),
        unix_now().unwrap() + 3600,
    )
    .unwrap()
}
async fn service(p: &Provider) -> Arc<ProjectService> {
    ProjectService::connect_browser(
        verifier(),
        &env("KARATE_TEST_DATABASE_URL"),
        BrowserProvider::new(&p.issuer, "karate-browser", ORIGIN, Some(&p.ca)).unwrap(),
    )
    .await
    .unwrap()
}
fn params(url: &str) -> HashMap<String, String> {
    url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}
fn signed(p: &Provider, nonce: &str, change: impl FnOnce(&mut Value, &mut Header)) -> String {
    let now = unix_now().unwrap();
    let mut value = json!({"iss":p.issuer,"aud":"karate-browser","sub":"browser-user","iat":now-1,"exp":now+900,"nonce":nonce});
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("fixture".into());
    header.typ = Some("JWT".into());
    change(&mut value, &mut header);
    encode(
        &header,
        &value,
        &EncodingKey::from_rsa_der(&std::fs::read(env("KARATE_TEST_KEY")).unwrap()),
    )
    .unwrap()
}
async fn prepare(
    p: &Provider,
    s: &ProjectService,
    t: Uuid,
    change: impl FnOnce(&mut Value, &mut Header),
) -> (LoginStart, String) {
    p.configure(p.normal("fixture"));
    let login = s.begin_login(t).await.unwrap();
    let q = params(&login.authorization_url);
    assert_eq!(q["code_challenge_method"], "S256");
    assert_eq!(q["scope"], "openid");
    assert_eq!(q["redirect_uri"], format!("{ORIGIN}/v1/auth/callback"));
    let token = signed(p, &q["nonce"], change);
    let mut plan = p.normal("fixture");
    plan["/issuer/token"] = json!({"challenge":q["code_challenge"],"body":json!({"id_token":token,"access_token":Uuid::new_v4().to_string(),"token_type":"Bearer"}).to_string()});
    p.configure(plan);
    (login, q["state"].clone())
}
async fn login(p: &Provider, s: &ProjectService, t: Uuid) -> String {
    let (l, state) = prepare(p, s, t, |_, _| {}).await;
    s.complete_login(&state, &l.binding, Some("fixture-code"), Some(&p.issuer))
        .await
        .unwrap()
        .credential
}
async fn admin() -> tokio_postgres::Client {
    let (c, f) = tokio_postgres::connect(&env("KARATE_TEST_ADMIN_URL"), tokio_postgres::NoTls)
        .await
        .unwrap();
    tokio::spawn(async move { f.await.unwrap() });
    c
}
async fn seed(a: &tokio_postgres::Client, p: &Provider, t: Uuid) {
    a.execute(
        "INSERT INTO karate.tenants(tenant_id,name) VALUES($1,'Browser fixture')",
        &[&t],
    )
    .await
    .unwrap();
    a.execute("INSERT INTO karate.memberships(tenant_id,issuer,subject,role) VALUES($1,$2,'browser-user','owner')",&[&t,&p.issuer]).await.unwrap();
}

#[tokio::test]
async fn browser_binding_claims_durable_sessions_authority_cookies_and_atomic_revocation() {
    let p = Provider::start().await;
    let s = service(&p).await;
    let a = admin().await;
    let t = Uuid::new_v4();
    seed(&a, &p, t).await;
    let (first, state) = prepare(&p, &s, t, |_, _| {}).await;
    let before = p.count();
    assert!(matches!(
        s.complete_login(&state, &"0".repeat(64), Some("fixture-code"), None)
            .await,
        Err(Error::Unauthenticated)
    ));
    assert_eq!(p.count(), before);
    let initial = s
        .complete_login(
            &state,
            &first.binding,
            Some("fixture-code"),
            Some(&p.issuer),
        )
        .await
        .unwrap()
        .credential;
    assert!(s.session_info(&initial).await.is_ok());
    let before = p.count();
    assert!(matches!(
        s.complete_login(&state, &first.binding, Some("fixture-code"), None)
            .await,
        Err(Error::Unauthenticated)
    ));
    assert_eq!(p.count(), before);
    for (field, value) in [
        ("nonce", json!("wrong")),
        ("iss", json!("https://wrong.example")),
        ("aud", json!("wrong")),
        ("azp", json!("wrong")),
        ("aud", json!(["karate-browser", "other"])),
        ("exp", json!(unix_now().unwrap() - 1)),
        ("iat", json!(-1)),
        ("iat", json!(unix_now().unwrap() + 300)),
        ("exp", json!(unix_now().unwrap() + 7200)),
        ("sub", json!("")),
        ("sub", json!("a".repeat(256))),
        ("sub", json!("não-ascii")),
        ("nbf", json!(unix_now().unwrap() + 300)),
        ("nbf", json!("wrong")),
        ("at_hash", json!("wrong")),
        ("c_hash", json!(5)),
    ] {
        let (l, state) = prepare(&p, &s, t, |c, _| c[field] = value).await;
        assert!(
            matches!(
                s.complete_login(&state, &l.binding, Some("fixture-code"), None)
                    .await,
                Err(Error::Unauthenticated)
            ),
            "field {field}"
        );
    }
    for field in ["nonce", "sub", "iss", "aud", "iat", "exp"] {
        let (l, state) = prepare(&p, &s, t, |c, _| {
            c.as_object_mut().unwrap().remove(field);
        })
        .await;
        assert!(matches!(
            s.complete_login(&state, &l.binding, Some("fixture-code"), None)
                .await,
            Err(Error::Unauthenticated)
        ));
    }
    for n in 0..4 {
        let (l, state) = prepare(&p, &s, t, |_, h| match n {
            0 => h.kid = None,
            1 => h.kid = Some("unknown".into()),
            2 => h.typ = Some("at+jwt".into()),
            _ => h.crit = Some(vec!["unsupported".into()]),
        })
        .await;
        assert!(matches!(
            s.complete_login(&state, &l.binding, Some("fixture-code"), None)
                .await,
            Err(Error::Unauthenticated)
        ));
    }
    let (l, state) = prepare(&p, &s, t, |_, _| {}).await;
    assert!(matches!(
        s.complete_login(
            &state,
            &l.binding,
            Some("fixture-code"),
            Some("https://other.example")
        )
        .await,
        Err(Error::Unauthenticated)
    ));
    let (l, state) = prepare(&p, &s, t, |_, _| {}).await;
    assert!(matches!(
        s.complete_login(&state, &l.binding, None, None).await,
        Err(Error::Unauthenticated)
    ));
    let (l, state) = prepare(&p, &s, t, |_, _| {}).await;
    a.execute("UPDATE karate.login_flows SET expires_at=clock_timestamp()-interval '1 second' WHERE tenant_id=$1",&[&t]).await.unwrap();
    assert!(matches!(
        s.complete_login(&state, &l.binding, Some("fixture-code"), None)
            .await,
        Err(Error::Unauthenticated)
    ));
    for state in [
        "".into(),
        "x".repeat(101),
        format!("kl1.{}.{}", t.simple(), "Z".repeat(64)),
    ] {
        assert!(matches!(
            s.complete_login(&state, &"0".repeat(64), Some("fixture-code"), None)
                .await,
            Err(Error::Unauthenticated)
        ));
    }
    // Bounded issuer clock skew does not extend absolute session expiry.
    let (l, state) = prepare(&p, &s, t, |c, _| {
        c["iat"] = json!(unix_now().unwrap() + 3);
        c["exp"] = json!(unix_now().unwrap() + 3603);
    })
    .await;
    let skewed = s
        .complete_login(&state, &l.binding, Some("fixture-code"), None)
        .await
        .unwrap();
    assert!(skewed.expires_at <= unix_now().unwrap() + 3600);
    s.logout(&skewed.credential, false).await.unwrap();
    let other = Uuid::new_v4();
    a.execute(
        "INSERT INTO karate.tenants(tenant_id,name) VALUES($1,'No membership')",
        &[&other],
    )
    .await
    .unwrap();
    let (l, state) = prepare(&p, &s, other, |_, _| {}).await;
    assert!(matches!(
        s.complete_login(&state, &l.binding, Some("fixture-code"), None)
            .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        s.list(&initial, other, 1, None).await,
        Err(Error::Forbidden)
    ));
    let second = login(&p, &s, t).await;
    let restored = service(&p).await;
    assert!(restored.session_info(&second).await.is_ok()); // durable across service restart
    a.execute(
        "UPDATE karate.memberships SET revision=revision+1 WHERE tenant_id=$1",
        &[&t],
    )
    .await
    .unwrap();
    assert!(matches!(
        s.list(&second, t, 1, None).await,
        Err(Error::Forbidden)
    ));
    // An obsolete full generation cannot block fresh login or crowd out revoke-all.
    a.execute("INSERT INTO karate.browser_sessions(tenant_id,session_id,issuer,subject,membership_revision,verifier,expires_at) SELECT $1,gen_random_uuid(),$2,'browser-user',1,decode(repeat('00',32),'hex'),extract(epoch FROM clock_timestamp())::bigint+600 FROM generate_series(1,32)",&[&t,&p.issuer]).await.unwrap();
    let active = login(&p, &s, t).await;
    for bad in [
        "ks1.bad".into(),
        "ks1.".to_string() + &"a".repeat(130),
        format!(
            "ks1.{}.{}.{}",
            t.simple(),
            Uuid::new_v4().simple(),
            "0".repeat(64)
        ),
        active[..133].to_owned() + if active.ends_with('0') { "1" } else { "0" },
    ] {
        assert!(matches!(
            s.session_info(&bad).await,
            Err(Error::Unauthenticated)
        ));
    }
    a.batch_execute("REVOKE INSERT ON karate.session_events FROM karate_membership_guard")
        .await
        .unwrap();
    assert!(matches!(
        s.logout(&active, false).await,
        Err(Error::Unavailable)
    ));
    assert!(s.session_info(&active).await.is_ok());
    let (l, state) = prepare(&p, &s, t, |_, _| {}).await;
    assert!(matches!(
        s.complete_login(&state, &l.binding, Some("fixture-code"), None)
            .await,
        Err(Error::Unavailable)
    ));
    a.batch_execute("GRANT INSERT ON karate.session_events TO karate_membership_guard")
        .await
        .unwrap();
    let another = login(&p, &s, t).await;
    s.logout(&active, false).await.unwrap();
    assert!(matches!(
        s.session_info(&active).await,
        Err(Error::Unauthenticated)
    ));
    assert!(s.session_info(&another).await.is_ok());
    s.logout(&another, true).await.unwrap();
    assert!(matches!(
        s.session_info(&another).await,
        Err(Error::Unauthenticated)
    ));
    let expired = login(&p, &s, t).await;
    let id = s.session_info(&expired).await.unwrap().session_id;
    a.execute("UPDATE karate.browser_sessions SET idle_until=clock_timestamp()-interval '1 second' WHERE tenant_id=$1 AND session_id=$2",&[&t,&id]).await.unwrap();
    assert!(matches!(
        s.session_info(&expired).await,
        Err(Error::Unauthenticated)
    ));
    let expired = login(&p, &s, t).await;
    let id = s.session_info(&expired).await.unwrap().session_id;
    a.execute(
        "UPDATE karate.browser_sessions SET expires_at=0 WHERE tenant_id=$1 AND session_id=$2",
        &[&t, &id],
    )
    .await
    .unwrap();
    assert!(matches!(
        s.session_info(&expired).await,
        Err(Error::Unauthenticated)
    ));
    // Session capacity is atomic and bounded, including concurrent issuance.
    a.execute("INSERT INTO karate.browser_sessions(tenant_id,session_id,issuer,subject,membership_revision,verifier,expires_at) SELECT $1,gen_random_uuid(),$2,'browser-user',2,decode(repeat('00',32),'hex'),extract(epoch FROM clock_timestamp())::bigint+600 FROM generate_series(1,32)",&[&t,&p.issuer]).await.unwrap();
    let (l, state) = prepare(&p, &s, t, |_, _| {}).await;
    assert!(matches!(
        s.complete_login(&state, &l.binding, Some("fixture-code"), None)
            .await,
        Err(Error::LimitReached)
    ));
    a.execute(
        "UPDATE karate.browser_sessions SET revoked=true WHERE tenant_id=$1",
        &[&t],
    )
    .await
    .unwrap();
    let (l, state) = prepare(&p, &s, t, |_, _| {}).await;
    let (left, right) = tokio::join!(
        s.complete_login(&state, &l.binding, Some("fixture-code"), None),
        s.complete_login(&state, &l.binding, Some("fixture-code"), None)
    );
    assert_ne!(left.is_ok(), right.is_ok());
    assert!(matches!(
        if left.is_err() { left } else { right },
        Err(Error::Unauthenticated)
    ));
    // Raw runtime credentials cannot read state/verifiers, create audit or call the guard helper.
    let (runtime, f) =
        tokio_postgres::connect(&env("KARATE_TEST_DATABASE_URL"), tokio_postgres::NoTls)
            .await
            .unwrap();
    tokio::spawn(async move { f.await.unwrap() });
    for sql in [
        "SELECT * FROM karate.login_flows",
        "SELECT * FROM karate.browser_sessions",
        "SELECT karate.session_member('00000000-0000-0000-0000-000000000101')",
        "INSERT INTO karate.session_events DEFAULT VALUES",
    ] {
        assert!(runtime.batch_execute(sql).await.is_err());
    }
    // HTTP state binding, secure cookies, CSRF, localized errors, current-session metadata and logout.
    let app = router(s.clone());
    let request = || {
        Request::builder()
            .method("POST")
            .uri(format!("/v1/tenants/{t}/login"))
            .header("origin", ORIGIN)
            .header("x-karate-csrf", "1")
            .body(Body::empty())
            .unwrap()
    };
    p.configure(p.normal("fixture"));
    let response = app.clone().oneshot(request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let binding = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .to_string();
    assert!(binding.contains("Secure; HttpOnly; SameSite=Lax"));
    assert!(!binding.contains("Domain="));
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let q = params(body["authorization_url"].as_str().unwrap());
    let mut plan = p.normal("fixture");
    plan["/issuer/token"] = json!({"challenge":q["code_challenge"],"body":json!({"id_token":signed(&p,&q["nonce"],|_,_|{}),"access_token":Uuid::new_v4().to_string(),"token_type":"Bearer"}).to_string()});
    p.configure(plan);
    let callback = Request::builder()
        .uri(format!(
            "/v1/auth/callback?state={}&code=fixture-code",
            q["state"]
        ))
        .header("cookie", binding.split(';').next().unwrap())
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(callback).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
    assert_eq!(response.headers()["location"], format!("{ORIGIN}/"));
    let session = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap())
        .find(|v| v.starts_with("__Host-karate-session="))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let request = |method: &str, path: String, origin: Option<&str>, csrf: bool| {
        let mut r = Request::builder()
            .method(method)
            .uri(path)
            .header("cookie", &session)
            .header("accept-language", "pt-BR");
        if let Some(o) = origin {
            r = r.header("origin", o);
        }
        if csrf {
            r = r.header("x-karate-csrf", "1");
        }
        r.body(Body::empty()).unwrap()
    };
    let response = app
        .clone()
        .oneshot(request("GET", "/v1/session".into(), None, false))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["tenant_id"], t.to_string());
    assert!(body.get("credential").is_none());
    for (origin, csrf) in [
        (None, false),
        (Some("https://evil.example"), true),
        (Some(ORIGIN), false),
    ] {
        let r = app
            .clone()
            .oneshot(request("POST", "/v1/session/logout".into(), origin, csrf))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
        assert_eq!(r.headers()["content-language"], "pt-BR");
    }
    let mut ambiguous = request("GET", "/v1/session".into(), None, false);
    ambiguous.headers_mut().insert(
        "authorization",
        format!("Bearer {}", p.token("fixture")).parse().unwrap(),
    );
    assert_eq!(
        app.clone().oneshot(ambiguous).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let r = app
        .clone()
        .oneshot(request(
            "POST",
            "/v1/session/revoke-all".into(),
            Some(ORIGIN),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
    assert!(
        r.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        app.clone()
            .oneshot(request("GET", "/v1/session".into(), None, false))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    // A suspended owner can sign in and recover, without gaining project access.
    let owner = login(&p, &s, t).await;
    let update = karate_api::tenants::TenantChange {
        name: "Suspended fixture".into(),
        state: karate_core::tenant::TenantState::Suspended,
        expected_revision: 1,
    };
    s.change_tenant(&owner, t, &update).await.unwrap();
    assert!(matches!(
        s.list(&owner, t, 1, None).await,
        Err(Error::Forbidden)
    ));
    let recovery = login(&p, &s, t).await;
    assert!(s.session_info(&recovery).await.is_ok());
    assert_eq!(
        s.tenant(&recovery, t).await.unwrap().state,
        karate_core::tenant::TenantState::Suspended
    );
    assert!(matches!(
        s.provision_tenant(&recovery, Uuid::new_v4(), Uuid::new_v4())
            .await,
        Err(Error::Forbidden)
    ));
    s.change_tenant(
        &recovery,
        t,
        &karate_api::tenants::TenantChange {
            name: "Resumed fixture".into(),
            state: karate_core::tenant::TenantState::Active,
            expected_revision: 2,
        },
    )
    .await
    .unwrap();
    assert!(s.list(&recovery, t, 1, None).await.is_ok());
    s.logout(&recovery, true).await.unwrap();
}

#[tokio::test]
async fn browser_provider_configuration_metadata_and_cookie_boundaries_fail_closed() {
    let p = Provider::start().await;
    for (issuer, client, origin) in [
        ("http://localhost", "karate-browser", ORIGIN),
        (p.issuer.as_str(), "", ORIGIN),
        (
            p.issuer.as_str(),
            "karate-browser",
            "https://karate.example/path",
        ),
        (
            p.issuer.as_str(),
            "karate-browser",
            "https://karate.example?query",
        ),
    ] {
        assert!(BrowserProvider::new(issuer, client, origin, Some(&p.ca)).is_err());
    }
    assert!(BrowserProvider::new(&p.issuer, &"x".repeat(256), ORIGIN, None).is_err());
    let s = service(&p).await;
    let a = admin().await;
    let t = Uuid::new_v4();
    seed(&a, &p, t).await;
    for (field, value) in [
        ("issuer", json!("https://foreign.example")),
        (
            "authorization_endpoint",
            json!("https://foreign.example/auth"),
        ),
        ("token_endpoint", json!("http://localhost/token")),
        ("jwks_uri", json!(format!("{}/jwks?extra", p.issuer))),
        ("response_types_supported", json!(["id_token"])),
        ("id_token_signing_alg_values_supported", json!(["HS256"])),
        ("code_challenge_methods_supported", json!(["plain"])),
    ] {
        let mut plan = p.normal("fixture");
        let entry = &mut plan["/issuer/.well-known/openid-configuration"];
        let mut doc: Value = serde_json::from_str(entry["body"].as_str().unwrap()).unwrap();
        doc[field] = value;
        entry["body"] = json!(doc.to_string());
        p.configure(plan);
        assert!(
            matches!(s.begin_login(t).await, Err(Error::Unavailable)),
            "metadata {field}"
        );
    }
    p.configure(p.normal("fixture"));
    assert!(s.begin_login(Uuid::new_v4()).await.is_err());
    let app = router(s.clone());
    for cookie in [
        "__Host-karate-session=bad".to_string(),
        format!("__Host-karate-session={}", "x".repeat(134)),
        "x".repeat(8193),
        "__Host-karate-session=a; __Host-karate-session=b".into(),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/session")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/session")
                .header("cookie", "other=1")
                .header("cookie", "other=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    for path in ["/v1/auth/callback", "/v1/auth/callback?state=bad"] {
        let r = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(r.status().is_client_error());
    }
    for (origin, csrf) in [
        (Some(ORIGIN), Some("0")),
        (None, Some("1")),
        (Some(ORIGIN), None),
    ] {
        let mut req = Request::builder()
            .method("POST")
            .uri(format!("/v1/tenants/{t}/login"));
        if let Some(v) = origin {
            req = req.header("origin", v);
        }
        if let Some(v) = csrf {
            req = req.header("x-karate-csrf", v);
        }
        assert_eq!(
            app.clone()
                .oneshot(req.body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    // Tenant capacity is enforced before any provider call and recovers after expiry.
    a.execute("DELETE FROM karate.login_flows WHERE tenant_id=$1", &[&t])
        .await
        .unwrap();
    a.execute("INSERT INTO karate.login_flows(tenant_id,state_hash,binding_hash,nonce,pkce) SELECT $1,decode(lpad(to_hex(n),64,'0'),'hex'),decode(repeat('00',32),'hex'),repeat('0',64),repeat('a',43) FROM generate_series(1,128) n",&[&t]).await.unwrap();
    let before = p.count();
    assert!(matches!(s.begin_login(t).await, Err(Error::LimitReached)));
    assert_eq!(p.count(), before);
    a.execute("UPDATE karate.login_flows SET expires_at=clock_timestamp()-interval '1 second' WHERE tenant_id=$1",&[&t]).await.unwrap();
    let flow = s.begin_login(t).await.unwrap();
    let state = params(&flow.authorization_url)["state"].clone();
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/auth/callback?state={state}&error=access_denied"
                ))
                .header("cookie", format!("__Host-karate-login={}", flow.binding))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    assert!(
        r.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(r.headers()["referrer-policy"], "no-referrer");
    // Feature disabled does not acquire an alternate or bypass identity path.
    let disabled = ProjectService::connect(verifier(), &env("KARATE_TEST_DATABASE_URL"))
        .await
        .unwrap();
    assert!(matches!(
        disabled.begin_login(t).await,
        Err(Error::Unavailable)
    ));
    assert!(matches!(
        disabled.complete_login("", "", None, None).await,
        Err(Error::Unavailable)
    ));
    let response = router(disabled)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/tenants/{t}/login"))
                .header("origin", ORIGIN)
                .header("x-karate-csrf", "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
