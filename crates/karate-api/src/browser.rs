//! OIDC library primitives, bounded transport and durable browser/session binding.
use crate::{
    discovery::{document, https_url, response_document, transport},
    identity::{TokenVerifier, unix_now},
    membership::storage,
    service::{ProjectService, member_principal},
};
use aws_lc_rs::{
    constant_time::verify_slices_are_equal,
    digest::{SHA256, digest},
    rand,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use deadpool_postgres::Object;
use karate_core::{
    Error,
    authorization::{Action, Identity, SessionRestriction},
};
use openid::{Claims, Client, Discovered, IdToken, Options, StandardClaims};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

pub struct BrowserProvider {
    issuer: String,
    client_id: String,
    pub(crate) origin: String,
    http: reqwest::Client,
    permits: Semaphore,
}
pub struct LoginStart {
    pub authorization_url: String,
    pub binding: String,
}
pub struct LoginSession {
    pub credential: String,
    pub expires_at: i64,
}
#[derive(Serialize)]
pub struct SessionInfo {
    pub tenant_id: Uuid,
    pub session_id: Uuid,
    pub expires_at: i64,
}
#[derive(Deserialize)]
struct TokenResponse {
    id_token: String,
    access_token: String,
    token_type: String,
}
fn secret() -> Result<String, Error> {
    let mut bytes = [0; 32];
    rand::fill(&mut bytes).map_err(|_| Error::Unavailable)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn hash(value: &str) -> Vec<u8> {
    digest(&SHA256, value.as_bytes()).as_ref().to_vec()
}
fn hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn session_locator(value: &str) -> Result<(Uuid, Uuid), Error> {
    if value.len() != 134 {
        return Err(Error::Unauthenticated);
    }
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 4
        || parts[0] != "ks1"
        || !hex(parts[1], 32)
        || !hex(parts[2], 32)
        || !hex(parts[3], 64)
    {
        return Err(Error::Unauthenticated);
    }
    Ok((
        Uuid::parse_str(parts[1]).map_err(|_| Error::Unauthenticated)?,
        Uuid::parse_str(parts[2]).map_err(|_| Error::Unauthenticated)?,
    ))
}
impl BrowserProvider {
    pub fn new(
        issuer: &str,
        client_id: &str,
        origin: &str,
        ca: Option<&[u8]>,
    ) -> Result<Self, Error> {
        https_url(issuer)?;
        let public = https_url(origin)?;
        if public.path() != "/" || client_id.is_empty() || client_id.len() > 255 {
            return Err(Error::Validation);
        }
        Ok(Self {
            issuer: issuer.into(),
            client_id: client_id.into(),
            origin: public.origin().ascii_serialization(),
            http: transport(ca)?,
            permits: Semaphore::new(4),
        })
    }
    async fn client(&self) -> Result<Client<Discovered>, Error> {
        tokio::time::timeout(Duration::from_secs(8), async {
            let discovery = https_url(&format!(
                "{}/.well-known/openid-configuration",
                self.issuer.trim_end_matches('/')
            ))?;
            let raw = document(&self.http, discovery.clone()).await?;
            let value: Value = serde_json::from_str(&raw).map_err(|_| Error::Unavailable)?;
            if value["issuer"] != self.issuer {
                return Err(Error::Unavailable);
            }
            let config: openid::Config =
                serde_json::from_str(&raw).map_err(|_| Error::Unavailable)?;
            for url in [
                &config.authorization_endpoint,
                &config.token_endpoint,
                &config.jwks_uri,
            ] {
                if https_url(url.as_str()).is_err() || url.origin() != discovery.origin() {
                    return Err(Error::Unavailable);
                }
            }
            if !config.response_types_supported.iter().any(|s| s == "code")
                || !config
                    .id_token_signing_alg_values_supported
                    .iter()
                    .any(|s| s == "RS256")
                || !value["code_challenge_methods_supported"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|s| s == "S256"))
            {
                return Err(Error::Unavailable);
            }
            let jwks = document(&self.http, config.jwks_uri.clone()).await?;
            // Enforce the existing bounded RSA/RS256/signature-only key contract too.
            TokenVerifier::new(&self.issuer, &self.client_id, &jwks, unix_now()? + 60)
                .map_err(|_| Error::Unavailable)?;
            let jwks = serde_json::from_str(&jwks).map_err(|_| Error::Unavailable)?;
            Ok(Client::new(
                config.into(),
                self.client_id.clone(),
                None::<String>,
                Some(format!("{}/v1/auth/callback", self.origin)),
                self.http.clone(),
                Some(jwks),
            ))
        })
        .await
        .map_err(|_| Error::Unavailable)?
    }
    async fn exchange(
        &self,
        code: &str,
        nonce: &str,
        pkce: &str,
    ) -> Result<(Identity, i64), Error> {
        let _permit = self
            .permits
            .try_acquire()
            .map_err(|_| Error::LimitReached)?;
        let client = self.client().await?;
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "authorization_code")
            .append_pair("code", code)
            .append_pair("code_verifier", pkce)
            .append_pair("client_id", &self.client_id)
            .append_pair(
                "redirect_uri",
                client.redirect_url().ok_or(Error::Unavailable)?,
            )
            .finish();
        // No retry: a lost response may have consumed this authorization code.
        let response = self
            .http
            .post(client.config().token_endpoint.clone())
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|_| Error::Unavailable)?;
        let raw = response_document(response).await?;
        let response: TokenResponse =
            serde_json::from_str(&raw).map_err(|_| Error::Unauthenticated)?;
        if response.id_token.len() > 8192
            || response.access_token.len() > 8192
            || response.access_token.is_empty()
            || !response.token_type.eq_ignore_ascii_case("bearer")
        {
            return Err(Error::Unauthenticated);
        }
        let header =
            jsonwebtoken::decode_header(&response.id_token).map_err(|_| Error::Unauthenticated)?;
        if header.alg != jsonwebtoken::Algorithm::RS256
            || header.typ.as_deref().is_some_and(|v| v != "JWT")
            || header.crit.is_some()
            || header.enc.is_some()
            || header.zip.is_some()
        {
            return Err(Error::Unauthenticated);
        }
        let kid = header.kid.ok_or(Error::Unauthenticated)?;
        if !client.jwks.as_ref().is_some_and(|s| s.find(&kid).is_some()) {
            return Err(Error::Unauthenticated);
        }
        let mut token: IdToken<StandardClaims> = IdToken::new_encoded(&response.id_token);
        client
            .decode_token(&mut token)
            .map_err(|_| Error::Unauthenticated)?;
        client
            .validate_token(&token, Some(nonce), None)
            .map_err(|_| Error::Unauthenticated)?;
        let claims = token.payload().map_err(|_| Error::Unauthenticated)?;
        let payload = response
            .id_token
            .split('.')
            .nth(1)
            .ok_or(Error::Unauthenticated)?;
        let payload: Value = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(payload)
                .map_err(|_| Error::Unauthenticated)?,
        )
        .map_err(|_| Error::Unauthenticated)?;
        let now = unix_now()?;
        if payload["iss"] != self.issuer
            || claims.sub().is_empty()
            || !claims.sub().is_ascii()
            || claims.sub().len() > 255
            || claims.iat() < 0
            // Small issuer clock skew is not a not-before or expiry extension.
            || claims.iat() > now + 5
            || claims.exp() <= claims.iat()
            || claims.exp().saturating_sub(claims.iat()) > 3600
            || payload
                .get("nbf")
                .is_some_and(|v| v.as_i64().is_none_or(|n| n > now))
        {
            return Err(Error::Unauthenticated);
        }
        for (name, value) in [
            ("at_hash", response.access_token.as_str()),
            ("c_hash", code),
        ] {
            if let Some(encoded) = payload.get(name) {
                let received = URL_SAFE_NO_PAD
                    .decode(encoded.as_str().ok_or(Error::Unauthenticated)?)
                    .map_err(|_| Error::Unauthenticated)?;
                verify_slices_are_equal(&received, &hash(value)[..16])
                    .map_err(|_| Error::Unauthenticated)?;
            }
        }
        Ok((
            Identity {
                issuer: self.issuer.clone(),
                subject: claims.sub().into(),
                key_restriction: None,
                session_restriction: None,
            },
            claims.exp().min(now + 3600),
        ))
    }
}
impl ProjectService {
    pub async fn connect_browser(
        verifier: impl Into<crate::discovery::AccessVerifier>,
        connection: &str,
        browser: BrowserProvider,
    ) -> Result<Arc<Self>, Error> {
        let mut service = Self::connect(verifier, connection).await?;
        Arc::get_mut(&mut service)
            .ok_or(Error::Unavailable)?
            .browser = Some(browser);
        Ok(service)
    }
    pub(crate) fn browser_origin(&self) -> Result<&str, Error> {
        Ok(&self.browser.as_ref().ok_or(Error::Unavailable)?.origin)
    }
    pub async fn begin_login(&self, tenant: Uuid) -> Result<LoginStart, Error> {
        let browser = self.browser.as_ref().ok_or(Error::Unavailable)?;
        let _permit = browser
            .permits
            .try_acquire()
            .map_err(|_| Error::LimitReached)?;
        let state = format!("kl1.{}.{}", tenant.simple(), secret()?);
        let binding = secret()?;
        let nonce = secret()?;
        let pkce = openid::generate_s256_pkce();
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        client
            .execute(
                "SELECT karate.begin_login($1,$2,$3,$4,$5)",
                &[
                    &tenant,
                    &hash(&state),
                    &hash(&binding),
                    &nonce,
                    &pkce.code_verifier(),
                ],
            )
            .await
            .map_err(storage)?;
        let mut provider = browser.client().await?;
        provider.pkce = Some(pkce);
        let authorization_url = provider
            .auth_url(&Options {
                state: Some(state),
                nonce: Some(nonce),
                ..Options::default()
            })
            .to_string();
        Ok(LoginStart {
            authorization_url,
            binding,
        })
    }
    pub async fn complete_login(
        &self,
        state: &str,
        binding: &str,
        code: Option<&str>,
        response_issuer: Option<&str>,
    ) -> Result<LoginSession, Error> {
        let browser = self.browser.as_ref().ok_or(Error::Unavailable)?;
        if state.len() != 101 || binding.len() != 64 {
            return Err(Error::Unauthenticated);
        }
        let parts: Vec<_> = state.split('.').collect();
        if parts.len() != 3
            || parts[0] != "kl1"
            || !hex(parts[1], 32)
            || !hex(parts[2], 64)
            || !hex(binding, 64)
        {
            return Err(Error::Unauthenticated);
        }
        let tenant = Uuid::parse_str(parts[1]).map_err(|_| Error::Unauthenticated)?;
        let mut client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let flow = client
            .query_opt(
                "SELECT * FROM karate.consume_login($1,$2,$3)",
                &[&tenant, &hash(state), &hash(binding)],
            )
            .await
            .map_err(storage)?
            .ok_or(Error::Unauthenticated)?;
        let code = code
            .filter(|c| !c.is_empty() && c.len() <= 4096)
            .ok_or(Error::Unauthenticated)?;
        if response_issuer.is_some_and(|s| s != browser.issuer) {
            return Err(Error::Unauthenticated);
        }
        let (identity, expires) = browser
            .exchange(
                code,
                &flow.try_get::<_, String>("nonce").map_err(storage)?,
                &flow.try_get::<_, String>("pkce").map_err(storage)?,
            )
            .await?;
        let id = Uuid::new_v4();
        let credential = format!("ks1.{}.{}.{}", tenant.simple(), id.simple(), secret()?);
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(storage)?;
        crate::tenants::authorize_recovery(&tx, &identity, tenant).await?;
        tx.query_one(
            "SELECT * FROM karate.open_session($1,$2,$3,$4)",
            &[&tenant, &id, &hash(&credential), &expires],
        )
        .await
        .map_err(storage)?;
        crate::tenants::authorize_recovery(&tx, &identity, tenant).await?;
        tx.commit().await.map_err(storage)?;
        Ok(LoginSession {
            credential,
            expires_at: expires,
        })
    }
    pub(crate) async fn session_connection(
        &self,
        credential: &str,
    ) -> Result<(Identity, Object), Error> {
        let (tenant, id) = session_locator(credential)?;
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let row = client
            .query_opt(
                "SELECT * FROM karate.session_verifier($1,$2)",
                &[&tenant, &id],
            )
            .await
            .map_err(storage)?;
        let expected = match &row {
            Some(r) => r.try_get::<_, Vec<u8>>("verifier").map_err(storage)?,
            None => vec![0; 32],
        };
        let verifier = hash(credential);
        verify_slices_are_equal(&expected, &verifier).map_err(|_| Error::Unauthenticated)?;
        let row = client
            .query_opt(
                "SELECT * FROM karate.touch_session($1,$2,$3)",
                &[&tenant, &id, &verifier],
            )
            .await
            .map_err(storage)?
            .ok_or(Error::Unauthenticated)?;
        Ok((
            Identity {
                issuer: row.try_get("issuer").map_err(storage)?,
                subject: row.try_get("subject").map_err(storage)?,
                key_restriction: None,
                session_restriction: Some(SessionRestriction {
                    tenant_id: tenant,
                    membership_revision: row.try_get("membership_revision").map_err(storage)?,
                }),
            },
            client,
        ))
    }
    pub async fn session_info(&self, credential: &str) -> Result<SessionInfo, Error> {
        let (tenant, id) = session_locator(credential)?;
        let (identity, mut client) = self.session_connection(credential).await?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(storage)?;
        crate::tenants::authorize_recovery(&tx, &identity, tenant).await?;
        let row = tx
            .query_one(
                "SELECT * FROM karate.session_verifier($1,$2)",
                &[&tenant, &id],
            )
            .await
            .map_err(storage)?;
        let expires_at = row.try_get("expires_at").map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(SessionInfo {
            tenant_id: tenant,
            session_id: id,
            expires_at,
        })
    }
    pub async fn logout(&self, credential: &str, all: bool) -> Result<(), Error> {
        let (tenant, id) = session_locator(credential)?;
        let (identity, mut client) = self.session_connection(credential).await?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(storage)?;
        member_principal(&tx, &identity, tenant)
            .await?
            .authorize(tenant, Action::Read)?;
        tx.execute(
            "SELECT karate.close_sessions($1,$2,$3)",
            &[&tenant, &id, &all],
        )
        .await
        .map_err(storage)?;
        tx.commit().await.map_err(storage)
    }
}
