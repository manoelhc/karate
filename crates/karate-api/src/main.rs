use karate_api::{http::router, identity::TokenVerifier, service::ProjectService};
use karate_core::Error;
use std::{env, io::Read, net::SocketAddr};

fn public_file(path: &str) -> Result<String, Error> {
    let mut value = String::new();
    std::fs::File::open(path)
        .map_err(|_| Error::Validation)?
        .take(65_537)
        .read_to_string(&mut value)
        .map_err(|_| Error::Validation)?;
    if value.len() > 65_536 {
        return Err(Error::Validation);
    }
    Ok(value)
}

async fn run() -> Result<(), Error> {
    let required = |key| env::var(key).map_err(|_| Error::Validation);
    let issuer = required("KARATE_ISSUER")?;
    let audience = required("KARATE_AUDIENCE")?;
    let connection = required("KARATE_DATABASE_URL")?;
    let verifier: karate_api::discovery::AccessVerifier =
        match env::var("KARATE_OIDC_DISCOVERY").as_deref() {
            Err(env::VarError::NotPresent) | Ok("false") => {
                if env::var_os("KARATE_OIDC_CA_FILE").is_some()
                    || env::var_os("KARATE_OIDC_KEY_TTL").is_some()
                {
                    return Err(Error::Validation);
                }
                let keys = public_file(&required("KARATE_JWKS_FILE")?)?;
                let expiry = required("KARATE_JWKS_EXPIRES_AT")?
                    .parse()
                    .map_err(|_| Error::Validation)?;
                TokenVerifier::new(&issuer, &audience, &keys, expiry)?.into()
            }
            Ok("true") => {
                if env::var_os("KARATE_JWKS_FILE").is_some()
                    || env::var_os("KARATE_JWKS_EXPIRES_AT").is_some()
                {
                    return Err(Error::Validation);
                }
                let ttl = match env::var("KARATE_OIDC_KEY_TTL") {
                    Ok(value) => value.parse().map_err(|_| Error::Validation)?,
                    Err(env::VarError::NotPresent) => 600,
                    Err(_) => return Err(Error::Validation),
                };
                let ca = match env::var("KARATE_OIDC_CA_FILE") {
                    Ok(path) => Some(public_file(&path)?),
                    Err(env::VarError::NotPresent) => None,
                    Err(_) => return Err(Error::Validation),
                };
                karate_api::discovery::RefreshingVerifier::discover(
                    &issuer,
                    &audience,
                    ttl,
                    ca.as_ref().map(|s| s.as_bytes()),
                )
                .await?
                .into()
            }
            _ => return Err(Error::Validation),
        };
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
