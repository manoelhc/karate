//! Shared authorization. Only trusted credential adapters implement Authenticator.
use crate::Error;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Admin,
    Member,
    Viewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Read,
    Write,
    Run,
    Export,
    ManageKeys,
    ManageMembers,
    Billing,
    DeleteTenant,
}

impl Role {
    pub fn allows(self, action: Action) -> bool {
        matches!(
            (self, action),
            (
                Self::Owner,
                Action::Read
                    | Action::Write
                    | Action::Run
                    | Action::Export
                    | Action::ManageKeys
                    | Action::ManageMembers
                    | Action::Billing
                    | Action::DeleteTenant
            ) | (Self::Admin | Self::Member | Self::Viewer, Action::Read)
                | (
                    Self::Admin,
                    Action::Write
                        | Action::Run
                        | Action::Export
                        | Action::ManageKeys
                        | Action::ManageMembers
                        | Action::Billing
                )
                | (Self::Member, Action::Write | Action::Run | Action::Export)
        )
    }
}

impl std::str::FromStr for Role {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "member" => Ok(Self::Member),
            "viewer" => Ok(Self::Viewer),
            _ => Err(Error::Forbidden),
        }
    }
}

/// Trusted adapter output, never deserialize this from requests or model output.
/// `None` grants interactive membership authority; service keys MUST use Some.
pub struct Identity {
    pub issuer: String,
    pub subject: String,
    pub key_restriction: Option<KeyRestriction>,
}

pub struct KeyRestriction {
    pub tenant_id: Uuid,
    pub scopes: Vec<Action>,
}

/// Trusted membership-store output, fetched afresh after authentication.
pub struct Membership {
    pub issuer: String,
    pub subject: String,
    pub tenant_id: Uuid,
    pub role: Role,
    pub revoked: bool,
    pub expires_at: Option<i64>,
}

pub trait Authenticator {
    /// Validate signature/verifier, issuer, audience, expiry and revocation.
    /// Implementations must not log `credential` or include it in errors.
    fn authenticate(&self, credential: &str, now: i64) -> Result<Identity, Error>;
}

pub trait MembershipStore {
    fn membership(&self, identity: &Identity, tenant: Uuid) -> Result<Option<Membership>, Error>;
}

/// An operation snapshot, not a session cache. Resolve again for every operation.
pub struct Principal {
    tenant_id: Uuid,
    role: Role,
    scopes: Option<Vec<Action>>,
}

pub fn resolve(
    authenticator: &impl Authenticator,
    memberships: &impl MembershipStore,
    credential: &str,
    tenant: Uuid,
    now: i64,
) -> Result<Principal, Error> {
    if credential.is_empty() || credential.len() > 8192 {
        return Err(Error::Unauthenticated);
    }
    let identity = authenticator.authenticate(credential, now)?;
    if identity.subject.is_empty() || identity.issuer.is_empty() {
        return Err(Error::Unauthenticated);
    }
    if identity
        .key_restriction
        .as_ref()
        .is_some_and(|key| key.tenant_id != tenant)
    {
        return Err(Error::Forbidden);
    }
    let membership = memberships
        .membership(&identity, tenant)?
        .ok_or(Error::Forbidden)?;
    if membership.tenant_id != tenant
        || membership.subject != identity.subject
        || membership.issuer != identity.issuer
        || membership.revoked
        || membership.expires_at.is_some_and(|expiry| now >= expiry)
    {
        return Err(Error::Forbidden);
    }
    Ok(Principal {
        tenant_id: tenant,
        role: membership.role,
        scopes: identity.key_restriction.map(|key| key.scopes),
    })
}

impl Principal {
    pub fn authorize(&self, tenant: Uuid, action: Action) -> Result<(), Error> {
        if self.tenant_id != tenant || !self.role.allows(action) {
            return Err(Error::Forbidden);
        }
        if let Some(scopes) = &self.scopes
            && (privileged(action) || !scopes.contains(&action))
        {
            return Err(Error::Forbidden);
        }
        Ok(())
    }

    pub fn validate_delegation(&self, scopes: &[Action]) -> Result<(), Error> {
        self.authorize(self.tenant_id, Action::ManageKeys)?;
        if scopes.is_empty()
            || scopes.len() > 4
            || scopes
                .iter()
                .any(|scope| privileged(*scope) || self.authorize(self.tenant_id, *scope).is_err())
        {
            return Err(Error::Forbidden);
        }
        Ok(())
    }
}

fn privileged(action: Action) -> bool {
    matches!(
        action,
        Action::ManageKeys | Action::ManageMembers | Action::Billing | Action::DeleteTenant
    )
}
