use karate_api::{http::router, identity::TokenVerifier, service::ProjectService};
use karate_core::Error;
use std::{env, io::Read, net::SocketAddr};

async fn run() -> Result<(), Error> {
    let required = |key| env::var(key).map_err(|_| Error::Validation);
    let issuer = required("KARATE_ISSUER")?;
    let audience = required("KARATE_AUDIENCE")?;
    let keys_path = required("KARATE_JWKS_FILE")?;
    let expiry = required("KARATE_JWKS_EXPIRES_AT")?
        .parse()
        .map_err(|_| Error::Validation)?;
    let connection = required("KARATE_DATABASE_URL")?;
    let mut keys = String::new();
    std::fs::File::open(keys_path)
        .map_err(|_| Error::Validation)?
        .take(65_537)
        .read_to_string(&mut keys)
        .map_err(|_| Error::Validation)?;
    let verifier = TokenVerifier::new(&issuer, &audience, &keys, expiry)?;
    let address: SocketAddr = env::var("KARATE_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()
        .map_err(|_| Error::Validation)?;
    if !address.ip().is_loopback() {
        return Err(Error::Validation);
    }
    let service = ProjectService::connect(verifier, &connection).await?;
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|_| Error::Unavailable)?;
    axum::serve(listener, router(service))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|_| Error::Unavailable)
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    if run().await.is_err() {
        eprintln!(
            "Karate API could not start or serve. Check the documented configuration and dependencies; credentials are never printed."
        );
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}
