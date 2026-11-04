//! Exercise the actual binary with isolated configuration, including shutdown.
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
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reservation.local_addr().unwrap();
    drop(reservation);
    let mut process = Process(
        configured()
            .env("KARATE_BIND", address.to_string())
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
