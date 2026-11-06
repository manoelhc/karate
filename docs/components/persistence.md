# COMP-PERSISTENCE — tenant and project storage

Owner: backend implementer. Current implementation is migrations 0001–0007 plus
the Rust `karate-api::service` adapter, documented in [API component](api.md).
Requirements: PLAT-TENANT/RBAC and REQ-001. PostgreSQL 14.20 was available for
local verification. This is not selection of an aging major for production;
the deployment version and container digest must be selected and tested later.

## Data and authority

Tables: tenants, memberships and projects. Every table includes tenant_id.
Projects use `(tenant_id, project_id)` as their primary key; downstream tables
must use composite foreign keys. Membership keys are tenant+issuer+subject.
Names and role strings are constrained; timestamps are UTC-capable timestamptz.
Project revisions support compare-and-set updates, which the adapter must use.
The adapter bounds project lists to 100 records with UUID cursors and parameterized
queries. Migration 0002 adds append-only project audit records with tenant/project
composite references, actor context, action and server time. Create/update and
audit commit together; audit failure rolls back the mutation.

All three tables ENABLE and FORCE row-level security. Membership visibility
requires transaction-local tenant, authenticated issuer and subject, plus current
nonrevoked/unexpired membership. Project writes require owner/admin/member.
Viewer reads work, viewer writes do not. A client-supplied tenant alone fails.
No cache is used for current authority. Long operations must reauthorize between
bounded work units; a SQL transaction snapshot is not a revocation push channel.

The runtime gets SELECT on tenants/memberships and CRUD on projects, never
TRUNCATE, schema ownership, SUPERUSER or BYPASSRLS. Tenant provisioning and
membership changes now use narrow definer functions with owner safeguards;
see [membership administration](membership.md). Direct runtime writes stay denied.
GUC settings do not authenticate a caller: only a trusted adapter may set them
after verifying credentials. SQL injection or possession of runtime credentials
can forge GUCs; parameterized queries, bounded pools and secret custody remain
required. RLS is defense in depth, not a substitute for shared service policy.

## Runtime, durability and failure

Current fixture runs with PostgreSQL 14.20 binaries, private mode-0700 directory,
Unix socket only, ten connections, 16 MiB shared buffers and disposable data.
Fixture trust auth is never a production configuration. It uses no actual
customer identities or credentials. It creates no shared/external infrastructure.

The migration is atomic and additive on a fresh database. Rollback before use
is deletion of the disposable fixture. For persisted deployments, restore or
forward migration is required; no destructive down migration is supplied.
`make test-db` verifies bootstrap, policy behavior, transaction rollback and
restart durability; backup/restore, upgrades and concurrent service transactions
are separate unverified gates. No Docker/container verification is claimed.

Production deployment must supply TLS, secret injection, bounded connection pool,
statement/lock timeouts, backup retention, deletion propagation, OTel spans and
RPO/RTO measurements. No production values or SLOs are claimed by this slice.
Schema SQL has no executable language coverage denominator; actual SQL behavior
is tested against PostgreSQL. A future Rust adapter needs its own >90% line and
branch reports (now measured for karate-api). MCP/voice and UI consume that adapter through shared services,
not direct SQL. The schema contains no localized UI strings.

## Acceptance evidence

`tests/postgres/tenant_isolation.sql` covers two tenants with identical names and
project IDs, missing/forged context, wrong issuer, viewer mutation attempts,
membership escalation denial, revoked/expired membership, cross-tenant writes,
optimistic update preconditions, transaction context reset and rollback.
`tests/postgres/restart.sql` verifies durable acknowledged writes, fresh-connection
authority and tenant B's retained rows. See the run verification report for status.

Migration 0004 adds identity-bound offers and invitation events, with forced RLS
and named capabilities owned by the existing NOLOGIN guard. Offer issue/revoke/
acceptance, enrollment/re-enrollment and audit are atomic. Captured inviter and
recipient revisions prevent stale grants after later membership changes. Runtime
has no direct offer writes; see [invitation component](invitations.md).

Migration 0005 adds forced-RLS service_keys/key_events and named credential/
management functions. Only the NOLOGIN guard can read/write verifier rows; runtime
gets bounded function execution and scoped audit reads. SHA-256 verifiers, never
plaintext keys, are stored. No new privileged login role is provisioned.

Migration 0006 adds forced-RLS login_flows, browser_sessions and session_events.
Only named authentication/lifecycle capabilities are granted to runtime. State and
binding cookies are hashed; transient nonce/PKCE are readable only by the guard.
Durable sessions store SHA-256 verifiers and membership revision, with atomic
issuance/revocation audit. See [browser sessions](browser-sessions.md).

Migration 0007 adds operator-only provisioning grants, atomic permanent-owner
bootstrap, tenant events and active/suspended revisioned metadata. KEY SHARE
normal-operation guards serialize with exclusive lifecycle changes; existing
membership locks stay compatible. Restrictive project/audit RLS and capability
state checks add defense in depth. See [tenant component](tenants.md).
