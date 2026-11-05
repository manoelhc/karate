//! Exercise the actual binary with isolated configuration, including shutdown.
#[path = "support/provider.rs"]
mod provider;
use karate_api::identity::unix_now;
use std::{
    process::{Child, Command, Stdio},
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn env(name: &str) -> String {
    std::env::var(name).expect("make test supplies isolated fixture")
}
fn command() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_karate-api"));
    for key in [
        "KARATE_ISSUER",
        "KARATE_AUDIENCE",
        "KARATE_JWKS_FILE",
        "KARATE_JWKS_EXPIRES_AT",
        "KARATE_DATABASE_URL",
        "KARATE_BIND",
        "KARATE_OIDC_DISCOVERY",
        "KARATE_OIDC_KEY_TTL",
        "KARATE_OIDC_CA_FILE",
        "KARATE_BROWSER_CLIENT_ID",
        "KARATE_PUBLIC_ORIGIN",
    ] {
        cmd.env_remove(key);
    }
    // Keep LLVM_PROFILE_FILE for instrumented child-process coverage.
    cmd
}
fn configured() -> Command {
    let mut cmd = command();
    cmd.env("KARATE_ISSUER", "https://issuer.example")
        .env("KARATE_AUDIENCE", "karate-api")
        .env("KARATE_JWKS_FILE", env("KARATE_TEST_JWKS"))
        .env(
            "KARATE_JWKS_EXPIRES_AT",
            (unix_now().unwrap() + 3600).to_string(),
        )
        .env("KARATE_DATABASE_URL", env("KARATE_TEST_DATABASE_URL"));
    cmd
}

#[test]
fn startup_fails_closed_without_exposing_configuration() {
    assert!(!command().output().unwrap().status.success());
    for (name, value) in [
        ("KARATE_ISSUER", "invalid-private-config"),
        ("KARATE_AUDIENCE", ""),
        ("KARATE_JWKS_FILE", "/missing-private-key-location"),
        ("KARATE_JWKS_EXPIRES_AT", "wrong"),
        ("KARATE_DATABASE_URL", "private-invalid-dsn"),
        ("KARATE_BROWSER_CLIENT_ID", "unpaired"),
        ("KARATE_PUBLIC_ORIGIN", "https://karate.example"),
        ("KARATE_BIND", "not-an-address"),
        ("KARATE_BIND", "0.0.0.0:8080"),
    ] {
        let output = configured().env(name, value).output().unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("could not start"));
        if !value.is_empty() {
            assert!(!error.contains(value));
        }
    }
    for key in [
        "KARATE_AUDIENCE",
        "KARATE_JWKS_FILE",
        "KARATE_JWKS_EXPIRES_AT",
        "KARATE_DATABASE_URL",
    ] {
        assert!(
            !configured()
                .env_remove(key)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    assert!(
        !configured()
            .env("KARATE_BROWSER_CLIENT_ID", "karate-browser")
            .env("KARATE_PUBLIC_ORIGIN", "https://karate.example")
            .output()
            .unwrap()
            .status
            .success()
    );
    // Already-bound socket must produce a bounded startup failure.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    assert!(
        !configured()
            .env("KARATE_BIND", listener.local_addr().unwrap().to_string())
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn discovery_process_configuration_is_explicit_bounded_and_fail_closed() {
    use std::os::unix::ffi::OsStringExt;
    let remote = || {
        let mut cmd = configured();
        cmd.env_remove("KARATE_JWKS_FILE")
            .env_remove("KARATE_JWKS_EXPIRES_AT")
            .env("KARATE_OIDC_DISCOVERY", "true")
            .env("KARATE_ISSUER", "https://127.0.0.1:9/issuer");
        cmd
    };
    for (key, value) in [
        ("KARATE_OIDC_DISCOVERY", "yes"),
        ("KARATE_OIDC_CA_FILE", "unexpected"),
        ("KARATE_OIDC_KEY_TTL", "600"),
    ] {
        assert!(
            !configured()
                .env(key, value)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    for (key, value) in [
        ("KARATE_JWKS_FILE", "conflict"),
        ("KARATE_JWKS_EXPIRES_AT", "123"),
        ("KARATE_OIDC_KEY_TTL", "invalid"),
        ("KARATE_OIDC_KEY_TTL", "0"),
        ("KARATE_OIDC_CA_FILE", "/nonexistent-fixture-ca"),
    ] {
        let result = remote().env(key, value).output().unwrap();
        assert!(!result.status.success());
        assert!(!String::from_utf8(result.stderr).unwrap().contains(value));
    }
    // No custom CA and default TTL still validate TLS; inaccessible owned endpoint fails.
    assert!(!remote().output().unwrap().status.success());
    for key in [
        "KARATE_OIDC_DISCOVERY",
        "KARATE_OIDC_KEY_TTL",
        "KARATE_OIDC_CA_FILE",
    ] {
        assert!(
            !remote()
                .env(key, std::ffi::OsString::from_vec(vec![255]))
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    let root = std::path::PathBuf::from(env("KARATE_TEST_TLS_DIR"));
    for bytes in [vec![b'x'; 65537], vec![255]] {
        let file = root.join(uuid::Uuid::new_v4().to_string());
        std::fs::write(&file, bytes).unwrap();
        assert!(
            !configured()
                .env("KARATE_JWKS_FILE", &file)
                .output()
                .unwrap()
                .status
                .success()
        );
        std::fs::remove_file(file).unwrap();
    }
}

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[tokio::test]
async fn real_process_serves_liveness_and_drains_on_interrupt() {
    serve(configured(), None).await;
}

#[tokio::test]
async fn real_process_discovers_https_keys_and_checks_membership() {
    let p = provider::Provider::start().await;
    std::fs::write(p.root.join("trusted-ca.pem"), &p.ca).unwrap();
    let mut cmd = configured();
    cmd.env_remove("KARATE_JWKS_FILE")
        .env_remove("KARATE_JWKS_EXPIRES_AT")
        .env("KARATE_OIDC_DISCOVERY", "true")
        .env("KARATE_OIDC_KEY_TTL", "600")
        .env("KARATE_ISSUER", &p.issuer)
        .env("KARATE_OIDC_CA_FILE", p.root.join("trusted-ca.pem"))
        .env("KARATE_BROWSER_CLIENT_ID", "karate-browser")
        .env("KARATE_PUBLIC_ORIGIN", "https://karate.example")
        .env("HTTPS_PROXY", "http://127.0.0.1:9")
        .env("ALL_PROXY", "http://127.0.0.1:9");
    serve(cmd, Some(p.token("fixture"))).await;
    assert_eq!(p.count(), 2);
}

async fn serve(mut cmd: Command, credential: Option<String>) {
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reservation.local_addr().unwrap();
    drop(reservation);
    let mut process = Process(
        cmd.env("KARATE_BIND", address.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut socket = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(socket) = tokio::net::TcpStream::connect(address).await {
                break socket;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("API startup deadline");
    socket
        .write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).await.unwrap();
    assert!(response.starts_with("HTTP/1.1 204"));
    if let Some(credential) = credential {
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        socket.write_all(format!("GET /v1/tenants/00000000-0000-0000-0000-000000000101/projects HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {credential}\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).await.unwrap();
        assert!(response.starts_with("HTTP/1.1 403"));
        assert!(!response.contains(&credential));
    }
    assert!(
        Command::new("kill")
            .args(["-INT", &process.0.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let status = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(status) = process.0.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("shutdown deadline");
    assert!(status.success());
}
