use karate_core::{
    Error,
    authorization::{
        Action, Identity, KeyRestriction, Membership, Role, SessionRestriction,
        authorize_membership,
    },
    tenant::*,
};
use uuid::Uuid;
const TENANT: Uuid = Uuid::from_u128(77);
fn identity() -> Identity {
    Identity {
        issuer: "https://issuer.example".into(),
        subject: "owner".into(),
        key_restriction: None,
        session_restriction: None,
    }
}
#[test]
fn grants_bind_interactive_identity_expiry_and_single_use() {
    let id = identity();
    assert_eq!(
        claim_grant(&id, &id.issuer, &id.subject, 11, 10, true),
        Ok(())
    );
    for (iss, sub) in [
        ("wrong", id.subject.as_str()),
        (id.issuer.as_str(), "wrong"),
    ] {
        assert_eq!(
            claim_grant(&id, iss, sub, 11, 10, true),
            Err(Error::Forbidden)
        );
    }
    for (expires, available) in [(10, true), (9, true), (11, false)] {
        assert_eq!(
            claim_grant(&id, &id.issuer, &id.subject, expires, 10, available),
            Err(Error::Conflict)
        );
    }
    for n in 0..4 {
        let mut id = identity();
        match n {
            0 => id.issuer.clear(),
            1 => id.subject.clear(),
            2 => {
                id.key_restriction = Some(KeyRestriction {
                    tenant_id: TENANT,
                    membership_revision: 1,
                    scopes: vec![Action::Read],
                })
            }
            _ => {
                id.session_restriction = Some(SessionRestriction {
                    tenant_id: TENANT,
                    membership_revision: 1,
                })
            }
        };
        assert_eq!(bootstrap_identity(&id), Err(Error::Forbidden));
    }
}
#[test]
fn lifecycle_is_owner_only_revisioned_and_recoverable() {
    assert_eq!("invalid".parse::<TenantState>(), Err(Error::Unavailable));
    for state in [TenantState::Active, TenantState::Suspended] {
        assert_eq!(state.as_str().parse::<TenantState>(), Ok(state));
        assert_eq!(state.require_active().is_ok(), state == TenantState::Active);
    }
    for role in [Role::Owner, Role::Admin, Role::Member, Role::Viewer] {
        let id = identity();
        let m = Membership {
            tenant_id: TENANT,
            issuer: id.issuer.clone(),
            subject: id.subject.clone(),
            role,
            revision: 1,
            revoked: false,
            expires_at: None,
        };
        let p = authorize_membership(id, m, TENANT, 10).unwrap();
        assert_eq!(inspect(&p, TENANT, TenantState::Active), Ok(()));
        assert_eq!(
            inspect(&p, TENANT, TenantState::Suspended).is_ok(),
            role == Role::Owner
        );
        assert_eq!(
            change(&p, TENANT, "Workspace", 1, 1).is_ok(),
            role == Role::Owner
        );
        assert_eq!(
            change(&p, Uuid::new_v4(), "Workspace", 1, 1),
            Err(Error::Forbidden)
        );
        if role == Role::Owner {
            for r in [0, i64::MAX] {
                assert_eq!(
                    change(&p, TENANT, "Workspace", r, 1),
                    Err(Error::Validation)
                );
            }
            assert_eq!(change(&p, TENANT, "", 1, 1), Err(Error::Validation));
            assert_eq!(change(&p, TENANT, "Workspace", 1, 2), Err(Error::Conflict));
        }
    }
}
