#[path = "support/provider.rs"]
mod provider;
use karate_api::{discovery::RefreshingVerifier, service::ProjectService};
use karate_core::Error;
use provider::Provider;
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio_postgres::NoTls;

async fn discover(p: &Provider, ttl: u64) -> Result<RefreshingVerifier, Error> {
    RefreshingVerifier::discover(&p.issuer, "karate-api", ttl, Some(&p.ca)).await
}

#[tokio::test]
async fn discovery_rejects_untrusted_configuration_tls_metadata_and_responses() {
    let p = Provider::start().await;
    for issuer in [
        "invalid".into(),
        "http://localhost".into(),
        "https://user@localhost".into(),
        "https://:pass@localhost".into(),
        "https://localhost?q=x".into(),
        "https://localhost#fragment".into(),
        format!("https://localhost/{}", "a".repeat(2048)),
    ] {
        assert!(matches!(
            RefreshingVerifier::discover(&issuer, "karate-api", 600, None).await,
            Err(Error::Validation)
        ));
    }
    for (audience, ttl) in [
        ("".into(), 600),
        ("x".repeat(256), 600),
        ("karate-api".into(), 3),
        ("karate-api".into(), 3601),
    ] {
        assert!(matches!(
            RefreshingVerifier::discover(&p.issuer, &audience, ttl, None).await,
            Err(Error::Validation)
        ));
    }
    for ca in [b"bad certificate".to_vec(), vec![0; 65537]] {
        assert!(matches!(
            RefreshingVerifier::discover(&p.issuer, "karate-api", 600, Some(&ca)).await,
            Err(Error::Validation)
        ));
    }
    assert!(matches!(
        RefreshingVerifier::discover(&p.issuer, "karate-api", 600, None).await,
        Err(Error::Unavailable)
    ));
    assert!(matches!(
        RefreshingVerifier::discover(
            &p.issuer.replace("localhost", "127.0.0.1"),
            "karate-api",
            600,
            Some(&p.ca)
        )
        .await,
        Err(Error::Unavailable)
    ));
    for metadata in [
        json!({"issuer":"https://wrong.example","jwks_uri":format!("{}/keys",p.issuer)}),
        json!({"issuer":p.issuer,"jwks_uri":"https://untrusted.invalid/keys"}),
        json!({"issuer":p.issuer,"jwks_uri":"http://localhost/keys"}),
        json!({"issuer":p.issuer}),
        json!({"issuer":p.issuer,"jwks_uri":format!("{}/keys?secret=x",p.issuer)}),
    ] {
        let mut plan = p.normal("fixture");
        plan["/issuer/.well-known/openid-configuration"]["body"] = json!(metadata.to_string());
        p.configure(plan);
        let before = p.count();
        assert!(matches!(discover(&p, 600).await, Err(Error::Unavailable)));
        assert_eq!(p.count(), before + 1);
    }
    for path in ["/issuer/.well-known/openid-configuration", "/issuer/keys"] {
        for response in [
            json!({"status":302,"headers":{"Location":format!("{}/trap",p.issuer),"Content-Type":"application/json"}}),
            json!({"status":503}),
            json!({"body":"{}","headers":{"Content-Type":"text/html"}}),
            json!({"body":"{}","headers":{}}),
            json!({"body":"{}","headers":{"Content-Type":"application/json","Content-Encoding":"gzip"}}),
            json!({"body":"x".repeat(65537)}),
            json!({"body":"x".repeat(65537),"stream":true}),
            json!({"body":"{"}),
            json!({"invalid_utf8":true}),
            json!({"body":"x","extra_length":100}),
            json!({"body":"{\"keys\":[]}"}),
        ] {
            let mut plan = p.normal("fixture");
            plan[path] = response;
            p.configure(plan);
            assert!(
                matches!(discover(&p, 600).await, Err(Error::Unavailable)),
                "path={path}"
            );
        }
    }
    let mut plan = p.normal("fixture");
    plan["/issuer/keys"]["delay"] = json!(3);
    p.configure(plan);
    let start = std::time::Instant::now();
    assert!(matches!(discover(&p, 600).await, Err(Error::Unavailable)));
    assert!(start.elapsed() < Duration::from_secs(4));
    p.configure(p.normal("fixture"));
    let verifier = discover(&p, 600).await.unwrap();
    let before = p.count();
    for token in ["".into(), "x".repeat(8193), p.token("unknown")] {
        assert!(matches!(
            verifier.verify(&token).await,
            Err(Error::Unauthenticated)
        ));
    }
    for _ in 0..30 {
        assert!(matches!(
            verifier.verify(&p.token("unknown")).await,
            Err(Error::Unauthenticated)
        ));
    }
    assert_eq!(p.count(), before);
    assert_eq!(
        verifier.verify(&p.token("fixture")).await.unwrap().subject,
        "discovery-user"
    );
}

#[tokio::test]
async fn transport_bounds_headers_total_body_time_and_complete_refresh_time() {
    let p = Provider::start().await;
    let mut plan = p.normal("fixture");
    let mut headers = json!({"Content-Type":"application/json"});
    for n in 0..40 {
        headers[format!("X-Fixture-{n}")] = json!("bounded");
    }
    plan["/issuer/keys"]["headers"] = headers;
    p.configure(plan);
    assert!(matches!(discover(&p, 600).await, Err(Error::Unavailable)));
    let mut plan = p.normal("fixture");
    plan["/issuer/keys"]["pieces"] = json!(8);
    plan["/issuer/keys"]["piece_delay"] = json!(0.8);
    p.configure(plan);
    let start = std::time::Instant::now();
    assert!(matches!(discover(&p, 600).await, Err(Error::Unavailable)));
    assert!(start.elapsed() >= Duration::from_secs(4) && start.elapsed() < Duration::from_secs(7));
    let mut plan = p.normal("fixture");
    for path in ["/issuer/.well-known/openid-configuration", "/issuer/keys"] {
        plan[path]["pieces"] = json!(5);
        plan[path]["piece_delay"] = json!(0.85);
    }
    p.configure(plan);
    let start = std::time::Instant::now();
    assert!(matches!(discover(&p, 600).await, Err(Error::Unavailable)));
    assert!(start.elapsed() >= Duration::from_secs(7) && start.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
async fn refresh_is_atomic_coalesced_expires_without_extension_and_recovers() {
    let p = Provider::start().await;
    let verifier = Arc::new(discover(&p, 4).await.unwrap());
    let old = p.token("fixture");
    let new = p.token("rotated");
    let mut plan = p.normal("rotated");
    plan["/issuer/keys"]["delay"] = json!(0.4);
    p.configure(plan);
    tokio::time::sleep(Duration::from_millis(2100)).await;
    let before_refresh = p.count();
    let producer = {
        let v = verifier.clone();
        let t = new.clone();
        tokio::spawn(async move { v.verify(&t).await })
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        while p.count() < before_refresh + 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let before = p.count();
    for _ in 0..20 {
        assert!(verifier.verify(&old).await.is_ok());
    }
    assert_eq!(p.count(), before);
    assert!(producer.await.unwrap().is_ok());
    assert!(matches!(
        verifier.verify(&old).await,
        Err(Error::Unauthenticated)
    ));
    assert!(verifier.verify(&new).await.is_ok());
    let mut bad = p.normal("rotated");
    bad["/issuer/keys"]["body"] = json!("{\"keys\":[]}");
    p.configure(bad);
    tokio::time::sleep(Duration::from_millis(2100)).await;
    assert!(verifier.verify(&new).await.is_ok()); // Last good snapshot still within original expiry.
    let before = p.count();
    for _ in 0..20 {
        assert!(verifier.verify(&new).await.is_ok());
    }
    assert_eq!(p.count(), before);
    tokio::time::sleep(Duration::from_millis(2100)).await;
    assert!(matches!(
        verifier.verify(&new).await,
        Err(Error::Unavailable)
    ));
    p.configure(p.normal("recovered"));
    tokio::time::sleep(Duration::from_millis(2300)).await;
    assert!(verifier.verify(&p.token("recovered")).await.is_ok());
    assert!(matches!(
        verifier.verify(&new).await,
        Err(Error::Unauthenticated)
    ));
    // A cancelled producer reserves the cooldown before network work begins.
    let mut plan = p.normal("recovered");
    plan["/issuer/keys"]["delay"] = json!(1);
    p.configure(plan);
    tokio::time::sleep(Duration::from_millis(2100)).await;
    let before_refresh = p.count();
    let producer = {
        let v = verifier.clone();
        let t = p.token("recovered");
        tokio::spawn(async move { v.verify(&t).await })
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        while p.count() < before_refresh + 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    producer.abort();
    assert!(matches!(producer.await, Err(error) if error.is_cancelled()));
    let before = p.count();
    assert!(verifier.verify(&p.token("recovered")).await.is_ok());
    assert_eq!(p.count(), before);
}

#[tokio::test]
async fn discovered_credentials_use_current_membership_and_never_grant_tenant_authority() {
    let p = Provider::start().await;
    let verifier = discover(&p, 600).await.unwrap();
    let service = ProjectService::connect(
        verifier,
        &std::env::var("KARATE_TEST_DATABASE_URL").unwrap(),
    )
    .await
    .unwrap();
    let tenant = uuid::Uuid::new_v4();
    let token = p.token("fixture");
    assert!(matches!(
        service.list(&token, tenant, 1, None).await,
        Err(Error::Forbidden)
    ));
    let (admin, connection) =
        tokio_postgres::connect(&std::env::var("KARATE_TEST_ADMIN_URL").unwrap(), NoTls)
            .await
            .unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    admin
        .execute(
            "INSERT INTO karate.tenants(tenant_id,name) VALUES($1,'Discovery fixture')",
            &[&tenant],
        )
        .await
        .unwrap();
    admin.execute("INSERT INTO karate.memberships(tenant_id,issuer,subject,role) VALUES($1,$2,'discovery-user','viewer')",&[&tenant,&p.issuer]).await.unwrap();
    assert!(
        service
            .list(&token, tenant, 1, None)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(matches!(
        service
            .create(&token, tenant, uuid::Uuid::new_v4(), "Denied")
            .await,
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        service.list(&token, uuid::Uuid::new_v4(), 1, None).await,
        Err(Error::Forbidden)
    ));
    admin
        .execute(
            "UPDATE karate.memberships SET revoked_at=clock_timestamp() WHERE tenant_id=$1",
            &[&tenant],
        )
        .await
        .unwrap();
    assert!(matches!(
        service.list(&token, tenant, 1, None).await,
        Err(Error::Forbidden)
    ));
}
