//! Resource-server validation, not browser OIDC sign-in. Never trust token URLs.
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use karate_core::{Error, authorization::Identity};
use serde::Deserialize;
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn unix_now() -> Result<i64, Error> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Unavailable)?
            .as_secs(),
    )
    .map_err(|_| Error::Unavailable)
}

pub struct TokenVerifier {
    issuer: String,
    validation: Validation,
    keys: HashMap<String, DecodingKey>,
    keys_expire_at: i64,
}

#[derive(Clone, Deserialize)]
struct Claims {
    sub: String,
    iat: i64,
    exp: i64,
}

impl TokenVerifier {
    /// The issuer, audience and JWKS are trusted deployment configuration.
    /// Keys are public and expire no later than 24h after loading.
    pub fn new(
        issuer: &str,
        audience: &str,
        jwks: &str,
        keys_expire_at: i64,
    ) -> Result<Self, Error> {
        let origin = url::Url::parse(issuer).map_err(|_| Error::Validation)?;
        let now = unix_now()?;
        if origin.scheme() != "https"
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.query().is_some()
            || origin.fragment().is_some()
            || issuer.len() > 2048
            || audience.is_empty()
            || audience.len() > 255
            || jwks.len() > 65_536
            || keys_expire_at <= now
            || keys_expire_at > now + 86_400
        {
            return Err(Error::Validation);
        }
        let set: JwkSet = serde_json::from_str(jwks).map_err(|_| Error::Validation)?;
        if set.keys.is_empty() || set.keys.len() > 32 {
            return Err(Error::Validation);
        }
        let mut keys = HashMap::new();
        for key in set.keys {
            let value = serde_json::to_value(&key).map_err(|_| Error::Validation)?;
            if value["kty"] != "RSA" || value["alg"] != "RS256" || value["use"] != "sig" {
                return Err(Error::Validation);
            }
            let kid = key
                .common
                .key_id
                .as_ref()
                .filter(|kid| !kid.is_empty() && kid.len() <= 128)
                .ok_or(Error::Validation)?;
            let decoding = DecodingKey::from_jwk(&key).map_err(|_| Error::Validation)?;
            if keys.insert(kid.clone(), decoding).is_some() {
                return Err(Error::Validation);
            }
        }
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[issuer]);
        validation.set_audience(&[audience]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.leeway = 0;
        validation.validate_nbf = true;
        Ok(Self {
            issuer: issuer.into(),
            validation,
            keys,
            keys_expire_at,
        })
    }

    pub fn verify(&self, token: &str) -> Result<Identity, Error> {
        let now = unix_now()?;
        if now >= self.keys_expire_at {
            return Err(Error::Unavailable);
        }
        if token.is_empty() || token.len() > 8192 {
            return Err(Error::Unauthenticated);
        }
        let header = decode_header(token).map_err(|_| Error::Unauthenticated)?;
        if header.alg != Algorithm::RS256
            || header.typ.as_deref() != Some("at+jwt")
            || header.crit.is_some()
            || header.enc.is_some()
            || header.zip.is_some()
        {
            return Err(Error::Unauthenticated);
        }
        let kid = header.kid.as_ref().ok_or(Error::Unauthenticated)?;
        let key = self.keys.get(kid).ok_or(Error::Unauthenticated)?;
        let claims = decode::<Claims>(token, key, &self.validation)
            .map_err(|_| Error::Unauthenticated)?
            .claims;
        if claims.sub.is_empty()
            || claims.sub.len() > 255
            || claims.iat > now
            || claims.exp <= now
            || claims.iat < 0
            || claims.exp <= claims.iat
            || claims.exp.saturating_sub(claims.iat) > 3600
        {
            return Err(Error::Unauthenticated);
        }
        Ok(Identity {
            issuer: self.issuer.clone(),
            subject: claims.sub,
            session_restriction: None,
            key_restriction: None,
        })
    }
}
