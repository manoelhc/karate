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
#[derive(Clone)]
pub struct Identity {
    pub issuer: String,
    pub subject: String,
    pub key_restriction: Option<KeyRestriction>,
    pub session_restriction: Option<SessionRestriction>,
}

#[derive(Clone)]
pub struct SessionRestriction {
    pub tenant_id: Uuid,
    pub membership_revision: i64,
}

#[derive(Clone)]
pub struct KeyRestriction {
    pub membership_revision: i64,
    pub tenant_id: Uuid,
    pub scopes: Vec<Action>,
}

/// Trusted membership-store output, fetched afresh after authentication.
pub struct Membership {
    pub revision: i64,
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
        .session_restriction
        .as_ref()
        .is_some_and(|session| session.tenant_id != tenant)
    {
        return Err(Error::Forbidden);
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
    authorize_membership(identity, membership, tenant, now)
}

/// Shared policy for async adapters: identity MUST come from credential verification,
/// membership from a fresh authorized store lookup, never request/model fields.
pub fn authorize_membership(
    identity: Identity,
    membership: Membership,
    tenant: Uuid,
    now: i64,
) -> Result<Principal, Error> {
    if identity.subject.is_empty() || identity.issuer.is_empty() {
        return Err(Error::Unauthenticated);
    }
    if identity
        .session_restriction
        .as_ref()
        .is_some_and(|session| {
            session.tenant_id != tenant || session.membership_revision != membership.revision
        })
    {
        return Err(Error::Forbidden);
    }
    if identity
        .key_restriction
        .as_ref()
        .is_some_and(|key| key.tenant_id != tenant)
    {
        return Err(Error::Forbidden);
    }
    if identity
        .key_restriction
        .as_ref()
        .is_some_and(|key| key.membership_revision != membership.revision)
    {
        return Err(Error::Forbidden);
    }
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

/// Current sponsor/recipient facts must come from the locked trusted store.
pub fn validate_invitation_grant(
    sponsor: Role,
    previous: Option<Role>,
    offered: Role,
) -> Result<(), Error> {
    if offered == Role::Owner
        || !sponsor.allows(Action::ManageMembers)
        || (sponsor == Role::Admin
            && (offered == Role::Admin || matches!(previous, Some(Role::Owner | Role::Admin))))
    {
        return Err(Error::Forbidden);
    }
    Ok(())
}

pub fn validate_invitation_acceptor(
    identity: &Identity,
    issuer: &str,
    subject: &str,
) -> Result<(), Error> {
    if identity.key_restriction.is_some()
        || identity.session_restriction.is_some()
        || identity.issuer != issuer
        || identity.subject != subject
        || issuer.is_empty()
        || subject.is_empty()
    {
        return Err(Error::Forbidden);
    }
    Ok(())
}

impl Principal {
    pub fn authorize_key_access(
        &self,
        tenant: Uuid,
        own: bool,
        rotating: bool,
    ) -> Result<(), Error> {
        self.authorize(tenant, Action::ManageKeys)?;
        if !own && (rotating || self.role != Role::Owner) {
            return Err(Error::Forbidden);
        }
        Ok(())
    }

    pub fn authorize_invitation(
        &self,
        tenant: Uuid,
        previous: Option<Role>,
        offered: Role,
    ) -> Result<(), Error> {
        self.authorize(tenant, Action::ManageMembers)?;
        validate_invitation_grant(self.role, previous, offered)
    }

    /// The adapter must hold the tenant management lock while obtaining these
    /// current target facts and committing the resulting mutation plus audit.
    pub fn authorize_member_change(
        &self,
        tenant: Uuid,
        current: Role,
        desired: Option<Role>,
        other_permanent_owner: bool,
        target_permanent: bool,
    ) -> Result<(), Error> {
        self.authorize(tenant, Action::ManageMembers)?;
        if self.role == Role::Admin
            && (matches!(current, Role::Owner | Role::Admin)
                || matches!(desired, Some(Role::Owner | Role::Admin)))
        {
            return Err(Error::Forbidden);
        }
        if (desired == Some(Role::Owner) && !target_permanent)
            || (current == Role::Owner && desired != Some(Role::Owner) && !other_permanent_owner)
        {
            return Err(Error::OwnerRequired);
        }
        Ok(())
    }

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
                .enumerate()
                .any(|(i, scope)| scopes[..i].contains(scope))
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
