use karate_core::{Error, authorization::*};
use std::cell::Cell;
use uuid::Uuid;

const TENANT: Uuid = Uuid::from_u128(1);
const OTHER: Uuid = Uuid::from_u128(2);
const ACTIONS: [Action; 8] = [
    Action::Read,
    Action::Write,
    Action::Run,
    Action::Export,
    Action::ManageKeys,
    Action::ManageMembers,
    Action::Billing,
    Action::DeleteTenant,
];

struct Auth {
    error: Option<Error>,
    issuer: &'static str,
    subject: &'static str,
    key: Option<(Uuid, Vec<Action>)>,
    calls: Cell<usize>,
}

impl Default for Auth {
    fn default() -> Self {
        Self {
            error: None,
            issuer: "https://id.example",
            subject: "alice",
            key: None,
            calls: Cell::new(0),
        }
    }
}

impl Authenticator for Auth {
    fn authenticate(&self, _: &str, _: i64) -> Result<Identity, Error> {
        self.calls.set(self.calls.get() + 1);
        if let Some(error) = self.error {
            return Err(error);
        }
        Ok(Identity {
            issuer: self.issuer.into(),
            subject: self.subject.into(),
            key_restriction: self.key.as_ref().map(|(tenant_id, scopes)| KeyRestriction {
                tenant_id: *tenant_id,
                scopes: scopes.clone(),
            }),
        })
    }
}

struct Store {
    role: Role,
    tenant: Uuid,
    issuer: &'static str,
    subject: &'static str,
    revoked: bool,
    expiry: Option<i64>,
    missing: bool,
    error: Option<Error>,
    calls: Cell<usize>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            role: Role::Owner,
            tenant: TENANT,
            issuer: "https://id.example",
            subject: "alice",
            revoked: false,
            expiry: None,
            missing: false,
            error: None,
            calls: Cell::new(0),
        }
    }
}

impl MembershipStore for Store {
    fn membership(&self, _: &Identity, _: Uuid) -> Result<Option<Membership>, Error> {
        self.calls.set(self.calls.get() + 1);
        if let Some(error) = self.error {
            return Err(error);
        }
        if self.missing {
            return Ok(None);
        }
        Ok(Some(Membership {
            issuer: self.issuer.into(),
            subject: self.subject.into(),
            tenant_id: self.tenant,
            role: self.role,
            revoked: self.revoked,
            expires_at: self.expiry,
        }))
    }
}

#[test]
fn all_roles_and_actions_obey_the_documented_matrix() {
    for (name, role, expected) in [
        ("owner", Role::Owner, [true; 8]),
        (
            "admin",
            Role::Admin,
            [true, true, true, true, true, true, true, false],
        ),
        (
            "member",
            Role::Member,
            [true, true, true, true, false, false, false, false],
        ),
        (
            "viewer",
            Role::Viewer,
            [true, false, false, false, false, false, false, false],
        ),
    ] {
        assert_eq!(name.parse::<Role>(), Ok(role));
        let actor = resolve(
            &Auth::default(),
            &Store {
                role,
                ..Store::default()
            },
            "test-only",
            TENANT,
            10,
        )
        .unwrap();
        for (action, allowed) in ACTIONS.into_iter().zip(expected) {
            assert_eq!(
                actor.authorize(TENANT, action).is_ok(),
                allowed,
                "{role:?} {action:?}"
            );
            assert_eq!(actor.authorize(OTHER, action), Err(Error::Forbidden));
        }
    }
    for invalid in ["", "Owner", "superadmin"] {
        assert_eq!(invalid.parse::<Role>(), Err(Error::Forbidden));
    }
}

#[test]
fn credentials_are_verified_before_any_membership_lookup() {
    let auth = Auth {
        error: Some(Error::Unauthenticated),
        ..Auth::default()
    };
    let store = Store::default();
    for credential in ["", &"x".repeat(8193), "forged"] {
        assert!(matches!(
            resolve(&auth, &store, credential, TENANT, 10),
            Err(Error::Unauthenticated)
        ));
    }
    assert_eq!(store.calls.get(), 0);
    assert_eq!(auth.calls.get(), 1);
    let unavailable = Auth {
        error: Some(Error::Unavailable),
        ..Auth::default()
    };
    assert!(matches!(
        resolve(&unavailable, &store, "test-only", TENANT, 10),
        Err(Error::Unavailable)
    ));
    for auth in [
        Auth {
            issuer: "",
            ..Auth::default()
        },
        Auth {
            subject: "",
            ..Auth::default()
        },
    ] {
        assert!(matches!(
            resolve(&auth, &store, "test-only", TENANT, 10),
            Err(Error::Unauthenticated)
        ));
    }
    assert_eq!(store.calls.get(), 0);
}

#[test]
fn membership_is_bound_to_tenant_issuer_subject_and_current_authority() {
    for store in [
        Store {
            tenant: OTHER,
            ..Store::default()
        },
        Store {
            issuer: "https://attacker.example",
            ..Store::default()
        },
        Store {
            subject: "bob",
            ..Store::default()
        },
        Store {
            revoked: true,
            ..Store::default()
        },
        Store {
            expiry: Some(10),
            ..Store::default()
        },
        Store {
            expiry: Some(9),
            ..Store::default()
        },
        Store {
            missing: true,
            ..Store::default()
        },
    ] {
        assert!(matches!(
            resolve(&Auth::default(), &store, "test-only", TENANT, 10),
            Err(Error::Forbidden)
        ));
    }
    let mut store = Store {
        expiry: Some(11),
        ..Store::default()
    };
    assert!(resolve(&Auth::default(), &store, "test-only", TENANT, 10).is_ok());
    store.revoked = true;
    assert!(matches!(
        resolve(&Auth::default(), &store, "test-only", TENANT, 10),
        Err(Error::Forbidden)
    ));
    assert_eq!(
        store.calls.get(),
        2,
        "membership cannot be cached between resolutions"
    );
    let failed = Store {
        error: Some(Error::Unavailable),
        ..Store::default()
    };
    assert!(matches!(
        resolve(&Auth::default(), &failed, "test-only", TENANT, 10),
        Err(Error::Unavailable)
    ));
}

#[test]
fn keys_cannot_cross_tenants_create_other_keys_or_elevate_roles() {
    let store = Store::default();
    let foreign = Auth {
        key: Some((OTHER, ACTIONS.to_vec())),
        ..Auth::default()
    };
    assert!(matches!(
        resolve(&foreign, &store, "test-only", TENANT, 10),
        Err(Error::Forbidden)
    ));
    assert_eq!(store.calls.get(), 0);
    for scopes in [vec![], vec![Action::Read], ACTIONS.to_vec()] {
        let auth = Auth {
            key: Some((TENANT, scopes.clone())),
            ..Auth::default()
        };
        let actor = resolve(&auth, &store, "test-only", TENANT, 10).unwrap();
        for action in ACTIONS {
            let allowed = [Action::Read, Action::Write, Action::Run, Action::Export]
                .contains(&action)
                && scopes.contains(&action);
            assert_eq!(actor.authorize(TENANT, action).is_ok(), allowed);
        }
        assert_eq!(
            actor.validate_delegation(&[Action::Read]),
            Err(Error::Forbidden)
        );
    }
    let auth = Auth {
        key: Some((TENANT, vec![Action::Run])),
        ..Auth::default()
    };
    let viewer = Store {
        role: Role::Viewer,
        ..Store::default()
    };
    let actor = resolve(&auth, &viewer, "test-only", TENANT, 10).unwrap();
    assert_eq!(actor.authorize(TENANT, Action::Run), Err(Error::Forbidden));
}

#[test]
fn delegation_is_bounded_and_never_grants_privileged_operations() {
    for role in [Role::Owner, Role::Admin] {
        let actor = resolve(
            &Auth::default(),
            &Store {
                role,
                ..Store::default()
            },
            "test-only",
            TENANT,
            10,
        )
        .unwrap();
        assert_eq!(
            actor.validate_delegation(&[Action::Read, Action::Run]),
            Ok(())
        );
        assert_eq!(actor.validate_delegation(&[]), Err(Error::Forbidden));
        assert_eq!(
            actor.validate_delegation(&[Action::Read; 5]),
            Err(Error::Forbidden)
        );
        for action in [
            Action::ManageKeys,
            Action::ManageMembers,
            Action::Billing,
            Action::DeleteTenant,
        ] {
            assert_eq!(actor.validate_delegation(&[action]), Err(Error::Forbidden));
        }
    }
    let member = resolve(
        &Auth::default(),
        &Store {
            role: Role::Member,
            ..Store::default()
        },
        "test-only",
        TENANT,
        10,
    )
    .unwrap();
    assert_eq!(
        member.validate_delegation(&[Action::Read]),
        Err(Error::Forbidden)
    );
}
