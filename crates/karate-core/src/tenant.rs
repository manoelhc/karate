//! Tenant bootstrap and lifecycle policy, shared by every transport.
use crate::{
    Error,
    authorization::{Action, Identity, Principal},
    validate_name,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TenantState {
    Active,
    Suspended,
}
impl TenantState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Suspended => "suspended",
        }
    }
    pub fn require_active(self) -> Result<(), Error> {
        if self == Self::Active {
            Ok(())
        } else {
            Err(Error::Forbidden)
        }
    }
}
impl std::str::FromStr for TenantState {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self, Error> {
        match value {
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            _ => Err(Error::Unavailable),
        }
    }
}
pub fn bootstrap_identity(identity: &Identity) -> Result<(), Error> {
    if identity.issuer.is_empty()
        || identity.subject.is_empty()
        || identity.key_restriction.is_some()
        || identity.session_restriction.is_some()
    {
        Err(Error::Forbidden)
    } else {
        Ok(())
    }
}
pub fn claim_grant(
    identity: &Identity,
    issuer: &str,
    subject: &str,
    expires: i64,
    now: i64,
    available: bool,
) -> Result<(), Error> {
    bootstrap_identity(identity)?;
    if identity.issuer != issuer || identity.subject != subject {
        return Err(Error::Forbidden);
    }
    if !available || expires <= now {
        return Err(Error::Conflict);
    }
    Ok(())
}
pub fn inspect(principal: &Principal, tenant: Uuid, state: TenantState) -> Result<(), Error> {
    principal.authorize(
        tenant,
        if state == TenantState::Active {
            Action::Read
        } else {
            Action::DeleteTenant
        },
    )
}
pub fn change(
    principal: &Principal,
    tenant: Uuid,
    name: &str,
    expected: i64,
    current: i64,
) -> Result<(), Error> {
    principal.authorize(tenant, Action::DeleteTenant)?;
    validate_name(name)?;
    if !(1..i64::MAX).contains(&expected) {
        return Err(Error::Validation);
    }
    if expected != current {
        return Err(Error::Conflict);
    }
    Ok(())
}
