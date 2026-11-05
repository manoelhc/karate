//! Maintained Dex protocol fixture. Its synthetic connector is not human sign-in evidence.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use karate_api::{
    browser::BrowserProvider,
    identity::{TokenVerifier, unix_now},
    service::ProjectService,
};
use karate_core::Error;
use std::{collections::HashMap, time::Duration};
use uuid::Uuid;
fn env(name: &str) -> String {
    std::env::var(name).expect("make test supplies isolated Dex fixture")
}
fn params(url: &url::Url) -> HashMap<String, String> {
    url.query_pairs().into_owned().collect()
}
async fn authorization(http: &reqwest::Client, start: &str, issuer: &str) -> url::Url {
    let origin = url::Url::parse(issuer).unwrap().origin();
    let mut next = url::Url::parse(start).unwrap();
    for _ in 0..8 {
        assert_eq!(
            next.origin(),
            origin,
            "fixture never visits an external provider"
        );
        let r = http.get(next.clone()).send().await.unwrap();
        assert!(
            r.status().is_redirection(),
            "synthetic connector must redirect"
        );
        let location = r.headers().get("location").unwrap().to_str().unwrap();
        next = next.join(location).unwrap();
        if next.origin().ascii_serialization() == "https://karate.example" {
            assert_eq!(next.path(), "/v1/auth/callback");
            return next;
        }
    }
    panic!("bounded authorization redirect limit")
}
#[tokio::test]
async fn maintained_dex_code_pkce_nonce_replay_and_session_lifecycle() {
    let issuer = env("KARATE_TEST_DEX_ISSUER");
    let ca = std::fs::read(format!("{}/ca.pem", env("KARATE_TEST_TLS_DIR"))).unwrap();
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .add_root_certificate(reqwest::Certificate::from_pem(&ca).unwrap())
        .build()
        .unwrap();
    let verifier = TokenVerifier::new(
        "https://issuer.example",
        "karate-api",
        &std::fs::read_to_string(env("KARATE_TEST_JWKS")).unwrap(),
        unix_now().unwrap() + 3600,
    )
    .unwrap();
    let service = ProjectService::connect_browser(
        verifier,
        &env("KARATE_TEST_DATABASE_URL"),
        BrowserProvider::new(
            &issuer,
            "karate-browser",
            "https://karate.example",
            Some(&ca),
        )
        .unwrap(),
    )
    .await
    .unwrap();
    let (admin, f) = tokio_postgres::connect(&env("KARATE_TEST_ADMIN_URL"), tokio_postgres::NoTls)
        .await
        .unwrap();
    tokio::spawn(async move { f.await.unwrap() });
    let tenant = Uuid::new_v4();
    // Dex v2.46.1 mockCallback fixed UserID, protobuf IDTokenSubject(user_id,conn_id).
    // Explicit fixture bootstrap only; application code never provisions membership on login.
    let subject = URL_SAFE_NO_PAD.encode(b"\x0a\x0d0-385-28089-0\x12\x07fixture");
    admin
        .execute(
            "INSERT INTO karate.tenants(tenant_id,name) VALUES($1,'Dex fixture')",
            &[&tenant],
        )
        .await
        .unwrap();
    admin.execute("INSERT INTO karate.memberships(tenant_id,issuer,subject,role) VALUES($1,$2,$3,'owner')",&[&tenant,&issuer,&subject]).await.unwrap();
    let flow = service.begin_login(tenant).await.unwrap();
    let callback = authorization(&http, &flow.authorization_url, &issuer).await;
    let query = params(&callback);
    let session = service
        .complete_login(
            &query["state"],
            &flow.binding,
            query.get("code").map(String::as_str),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .session_info(&session.credential)
            .await
            .unwrap()
            .tenant_id,
        tenant
    );
    assert!(
        service
            .list(&session.credential, tenant, 1, None)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(matches!(
        service
            .complete_login(
                &query["state"],
                &flow.binding,
                query.get("code").map(String::as_str),
                None
            )
            .await,
        Err(Error::Unauthenticated)
    ));
    // A fresh local state cannot make Dex reuse an already redeemed code.
    let another = service.begin_login(tenant).await.unwrap();
    let state = params(&url::Url::parse(&another.authorization_url).unwrap())["state"].clone();
    assert!(
        service
            .complete_login(
                &state,
                &another.binding,
                query.get("code").map(String::as_str),
                None
            )
            .await
            .is_err()
    );
    // Altering the authorization challenge makes the real provider reject our stored verifier.
    let flow = service.begin_login(tenant).await.unwrap();
    let mut url = url::Url::parse(&flow.authorization_url).unwrap();
    let mut q = params(&url);
    q.insert("code_challenge".into(), "A".repeat(43));
    url.set_query(None);
    url.query_pairs_mut().extend_pairs(q.iter());
    let callback = authorization(&http, url.as_str(), &issuer).await;
    let q = params(&callback);
    assert!(
        service
            .complete_login(
                &q["state"],
                &flow.binding,
                q.get("code").map(String::as_str),
                None
            )
            .await
            .is_err()
    );
    // A signed token from the provider still cannot substitute a different nonce.
    let flow = service.begin_login(tenant).await.unwrap();
    let mut url = url::Url::parse(&flow.authorization_url).unwrap();
    let mut q = params(&url);
    q.insert("nonce".into(), "wrong".into());
    url.set_query(None);
    url.query_pairs_mut().extend_pairs(q.iter());
    let q = params(&authorization(&http, url.as_str(), &issuer).await);
    assert!(matches!(
        service
            .complete_login(
                &q["state"],
                &flow.binding,
                q.get("code").map(String::as_str),
                None
            )
            .await,
        Err(Error::Unauthenticated)
    ));
    service.logout(&session.credential, true).await.unwrap();
    assert!(matches!(
        service.session_info(&session.credential).await,
        Err(Error::Unauthenticated)
    ));
}
