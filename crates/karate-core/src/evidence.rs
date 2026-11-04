//! Observations never imply an exploit or a verified vulnerability.
use crate::Error;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Observation {
    pub check: &'static str,
    pub header: &'static str,
    pub classification: Classification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    HardeningObservation,
}

/// Presence-only observations. Values and response bodies are never echoed.
/// Remediation/explanation copy is localized by the consuming presentation.
pub fn inspect_headers(
    headers: &[(String, String)],
    https: bool,
) -> Result<Vec<Observation>, Error> {
    if headers.len() > 128
        || headers
            .iter()
            .any(|(key, value)| key.len() > 256 || value.len() > 8192)
    {
        return Err(Error::LimitReached);
    }
    // Individual and count bounds above make this sum safe on supported targets.
    if headers
        .iter()
        .map(|(key, value)| key.len() + value.len())
        .sum::<usize>()
        > 65_536
    {
        return Err(Error::LimitReached);
    }
    let has = |name: &str| {
        headers
            .iter()
            .any(|(key, value)| key.eq_ignore_ascii_case(name) && !value.trim().is_empty())
    };
    let mut observations = Vec::new();
    for (header, check, applies) in [
        ("content-security-policy", "missing_csp", true),
        ("x-content-type-options", "missing_nosniff", true),
        ("strict-transport-security", "missing_hsts", https),
        ("referrer-policy", "missing_referrer_policy", true),
    ] {
        if applies && !has(header) {
            observations.push(Observation {
                check,
                header,
                classification: Classification::HardeningObservation,
            });
        }
    }
    Ok(observations)
}

/// A known inventory gives an endpoint ratio, never a security/quality score.
pub fn coverage(tested: usize, discovered: usize) -> Option<f64> {
    if discovered == 0 || tested > discovered {
        None
    } else {
        Some(tested as f64 / discovered as f64 * 100.0)
    }
}
