//! Transport-independent policy and domain invariants for Karate.
#![forbid(unsafe_code)]

pub mod assessment;
pub mod authorization;
pub mod evidence;
pub mod scope;
pub mod tenant;

use serde::{Deserialize, Serialize};

/// Closed, stable error codes; transports localize their messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Error {
    #[error("validation failed")]
    Validation,
    #[error("authentication required")]
    Unauthenticated,
    #[error("permission denied")]
    Forbidden,
    #[error("resource not found")]
    NotFound,
    #[error("operation conflicts with current state")]
    Conflict,
    #[error("permanent workspace ownership required")]
    OwnerRequired,
    #[error("target is outside the authorized scope")]
    OutOfScope,
    #[error("target ownership is not verified")]
    Unverified,
    #[error("usage limit reached")]
    LimitReached,
    #[error("dependency unavailable")]
    Unavailable,
}

/// Validate names without silently trimming or truncating user input.
pub fn validate_name(value: &str) -> Result<(), Error> {
    if value.trim().is_empty() || value.chars().count() > 120 || value.chars().any(char::is_control)
    {
        Err(Error::Validation)
    } else {
        Ok(())
    }
}
