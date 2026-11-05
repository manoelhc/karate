# Internal domain contract v1

Owner: COMP-DOMAIN. This contract is internal Rust, not a public REST API.

Role permissions, in action order Read, Write, Run, Export, ManageKeys,
ManageMembers, Billing, DeleteTenant:

| Role | Allowed actions |
| --- | --- |
| Owner | All eight |
| Admin | All except DeleteTenant |
| Member | Read, Write, Run, Export |
| Viewer | Read |

API-key scopes intersect role permissions. Empty scopes grant nothing; keys
cannot create keys, manage memberships, bill or delete tenants. Identity is
issuer+subject, never subject alone. Service keys bind the current membership
revision as well as tenant and scopes; revision drift denies authority. Membership expiry uses an exclusive bound:
`now >= expires_at` is denied. Invalid/expired credentials must fail before a
membership lookup. Dependency failure never produces a principal.

Assessments move draft → queued → preflight → discovering → testing → validating
→ completed. Active work may pause/resume; cancellation is final from every
nonterminal phase. Queued, active and paused work may fail. All terminal states
reject subsequent events. Pause in preflight/discovery is a proposed Karate
behavior; the reference only documents manual pause during attacks.

Names are nonblank, at most 120 Unicode scalar values, without control characters.
Scopes allow at most 100 blocked prefixes, 2,048 bytes per path, and 8,192 bytes
per URL. Prefix `/api` includes `/api` and `/api/x`, never `/apiculture`.
Blocked prefixes override allowed prefixes, including `/` blocking everything.
Normalization must not turn an ambiguous raw URL into an authorized request.
Scope JSON is not accepted as a trusted authority object; rebuild through `new`.

Header observations accept at most 128 headers, 256 bytes per name, 8,192 bytes
per value and 65,536 bytes total. Limit refusal produces LIMIT_REACHED with no
response values. These are library limits; the network adapter must also enforce
streaming response limits before allocating a full response.

Errors: VALIDATION, UNAUTHENTICATED, FORBIDDEN, NOT_FOUND, CONFLICT,
OUT_OF_SCOPE, UNVERIFIED, LIMIT_REACHED, UNAVAILABLE, OWNER_REQUIRED. No error contains a
credential, submitted URL or raw provider response. The API supplies localized closed error envelopes and OpenAPI schemas;
other transports remain pending.

Membership change policy refines ManageMembers: admins may manage only members
and viewers, without assigning admin/owner. Owners may manage all roles. Removing
an owner requires another active, nonexpiring owner; assigning owner requires a
nonexpiring target. Violations of these ownership invariants return OWNER_REQUIRED.
Adapters must hold a tenant management lock across current facts, policy and commit.

Invitation grants exclude owner. Current owner/admin authority is combined with
the inactive recipient's previous role; admins cannot restore previous owners or
admins. Acceptance binds exact issuer/subject and excludes service keys. Locked
storage must supply current sponsor facts, revisions and expiry before commit.

Key-management access refines ManageKeys: owner may list/revoke tenant keys, admin
only own, and rotation always requires the original creator. Delegated scopes are
unique, nonempty, at most four and cannot include privileged actions.
