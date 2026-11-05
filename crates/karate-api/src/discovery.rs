//! Operator-selected public metadata only. No token-supplied network locations.
use crate::identity::{TokenVerifier, unix_now};
use karate_core::{Error, authorization::Identity};
use reqwest::{Certificate, Client, Url};
use serde::Deserialize;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::Mutex as AsyncMutex, time::Instant};

pub enum AccessVerifier {
    File(Box<TokenVerifier>),
    Discovery(Box<RefreshingVerifier>),
}
impl From<TokenVerifier> for AccessVerifier {
    fn from(value: TokenVerifier) -> Self {
        Self::File(Box::new(value))
    }
}
impl From<RefreshingVerifier> for AccessVerifier {
    fn from(value: RefreshingVerifier) -> Self {
        Self::Discovery(Box::new(value))
    }
}
impl AccessVerifier {
    pub async fn verify(&self, token: &str) -> Result<Identity, Error> {
        match self {
            Self::File(verifier) => verifier.verify(token),
            Self::Discovery(verifier) => verifier.verify(token).await,
        }
    }
}

#[derive(Deserialize)]
struct Metadata {
    issuer: String,
    jwks_uri: String,
}
struct Snapshot {
    verifier: Arc<TokenVerifier>,
    next_attempt: Instant,
    failures: u32,
}
/// Cache contains public keys only; current tenant authority is never cached.
pub struct RefreshingVerifier {
    issuer: String,
    audience: String,
    discovery: Url,
    lifetime: u64,
    client: Client,
    snapshot: Mutex<Snapshot>,
    refresh: AsyncMutex<()>,
}
pub(crate) fn https_url(value: &str) -> Result<Url, Error> {
    let url = Url::parse(value).map_err(|_| Error::Validation)?;
    if value.len() > 2048
        || url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Validation);
    }
    Ok(url)
}
pub(crate) async fn document(client: &Client, url: Url) -> Result<String, Error> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| Error::Unavailable)?;
    response_document(response).await
}
pub(crate) async fn response_document(mut response: reqwest::Response) -> Result<String, Error> {
    let media = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .unwrap_or_default()
        .trim();
    if response.status() != reqwest::StatusCode::OK
        || !media.eq_ignore_ascii_case("application/json")
        || response.headers().contains_key("content-encoding")
        || response.content_length().is_some_and(|n| n > 65_536)
    {
        return Err(Error::Unavailable);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Error::Unavailable)? {
        if bytes.len() + chunk.len() > 65_536 {
            return Err(Error::Unavailable);
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|_| Error::Unavailable)
}
async fn load(
    client: &Client,
    discovery: &Url,
    issuer: &str,
    audience: &str,
    lifetime: u64,
) -> Result<TokenVerifier, Error> {
    tokio::time::timeout(Duration::from_secs(8), async {
        let metadata: Metadata = serde_json::from_str(&document(client, discovery.clone()).await?)
            .map_err(|_| Error::Unavailable)?;
        let keys = https_url(&metadata.jwks_uri).map_err(|_| Error::Unavailable)?;
        if metadata.issuer != issuer || keys.origin() != discovery.origin() {
            return Err(Error::Unavailable);
        }
        let jwks = document(client, keys).await?;
        TokenVerifier::new(issuer, audience, &jwks, unix_now()? + lifetime as i64)
            .map_err(|_| Error::Unavailable)
    })
    .await
    .map_err(|_| Error::Unavailable)?
}
impl RefreshingVerifier {
    /// The optional public CA extends trust without disabling TLS verification.
    pub async fn discover(
        issuer: &str,
        audience: &str,
        lifetime: u64,
        ca_pem: Option<&[u8]>,
    ) -> Result<Self, Error> {
        let origin = https_url(issuer)?;
        if audience.is_empty() || audience.len() > 255 || !(4..=3600).contains(&lifetime) {
            return Err(Error::Validation);
        }
        let discovery = Url::parse(&format!(
            "{}/.well-known/openid-configuration",
            origin.as_str().trim_end_matches('/')
        ))
        .map_err(|_| Error::Validation)?;
        let client = transport(ca_pem)?;
        let verifier = load(&client, &discovery, issuer, audience, lifetime).await?;
        Ok(Self {
            issuer: issuer.into(),
            audience: audience.into(),
            discovery,
            lifetime,
            client,
            snapshot: Mutex::new(Snapshot {
                verifier: Arc::new(verifier),
                next_attempt: Instant::now() + Duration::from_secs(lifetime / 2),
                failures: 0,
            }),
            refresh: AsyncMutex::new(()),
        })
    }
    pub async fn verify(&self, token: &str) -> Result<Identity, Error> {
        if token.is_empty() || token.len() > 8192 {
            return Err(Error::Unauthenticated);
        }
        // A token's kid never changes cadence. Concurrent callers do not queue on I/O.
        if let Ok(_producer) = self.refresh.try_lock() {
            let due = {
                let mut snapshot = self.snapshot.lock().map_err(|_| Error::Unavailable)?;
                if Instant::now() >= snapshot.next_attempt {
                    // Reserve before I/O: cancellation cannot create an immediate retry storm.
                    snapshot.next_attempt = Instant::now() + Duration::from_secs(8);
                    snapshot.failures = (snapshot.failures + 1).min(7);
                    true
                } else {
                    false
                }
            };
            if due {
                let result = load(
                    &self.client,
                    &self.discovery,
                    &self.issuer,
                    &self.audience,
                    self.lifetime,
                )
                .await;
                let mut snapshot = self.snapshot.lock().map_err(|_| Error::Unavailable)?;
                match result {
                    Ok(verifier) => {
                        snapshot.verifier = Arc::new(verifier);
                        snapshot.failures = 0;
                        snapshot.next_attempt =
                            Instant::now() + Duration::from_secs(self.lifetime / 2);
                    }
                    Err(_) => {
                        let seconds = (1_u64 << (snapshot.failures - 1)).min(60);
                        let jitter = (uuid::Uuid::new_v4().as_u128() % 250) as u64;
                        snapshot.next_attempt =
                            Instant::now() + Duration::from_millis(seconds * 1000 + jitter);
                    }
                }
            }
        }
        let verifier = self
            .snapshot
            .lock()
            .map_err(|_| Error::Unavailable)?
            .verifier
            .clone();
        verifier.verify(token)
    }
}

pub(crate) fn transport(ca_pem: Option<&[u8]>) -> Result<Client, Error> {
    let mut builder = Client::builder()
        .https_only(true)
        .http1_only()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .referer(false)
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(2))
        .read_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(5))
        .http1_max_headers(32)
        .pool_max_idle_per_host(1)
        .pool_idle_timeout(Duration::from_secs(30));
    if let Some(pem) = ca_pem {
        if pem.len() > 65_536 {
            return Err(Error::Validation);
        }
        let certs = Certificate::from_pem_bundle(pem).map_err(|_| Error::Validation)?;
        if certs.len() != 1 {
            return Err(Error::Validation);
        }
        builder = builder.tls_certs_merge(certs);
    }
    builder.build().map_err(|_| Error::Unavailable)
}
