# Scoped API keys — TASK-003b2c

Owner: backend implementer; PLAT-KEYS/TENANT/RBAC/AUDIT. Shared Rust service;
PostgreSQL durable credential store; reuse pinned AWS-LC 1.19.0 for secure random
bytes, SHA-256 and constant-time equality. This adds a direct dependency on the
already locked ISC-compatible implementation, not a new cryptography scheme.

Interactive owners/admins create keys for their own identity. Scope is a unique,
nonempty subset of read/write/run/export (max four), intersected with current role.
Keys cannot manage keys, memberships, invitations, billing or tenants. Bind the
key to a tenant and creator membership revision; any membership change invalidates
old keys. No automatic revival after re-enrollment. Owners list/revoke all tenant
keys; admins list/revoke only their own. Rotation requires the original creator's
current interactive management authority; owners cannot rotate someone else's key.

Credential format: kk1.<tenant UUID hex>.<key UUID hex>.<32 random bytes as hex>.
Public locators grant nothing. Store only SHA-256 of the whole canonical credential;
compare with AWS-LC constant-time equality before resolving membership. SHA-256 is
appropriate for 256-bit random credentials, not user-chosen passwords. No raw key
is persisted, logged, audited or recoverable. Database credential lookup is bounded
to one key and does not resolve memberships before verification. JWT remains the
interactive path; API keys remain usable during JWKS outage while their own
verifier/expiry/current membership are valid. No authentication bypass is added.

Create: caller UUID, name, scopes and future expiry <=90 days. Duplicate UUID
conflicts; it never reveals/replaces an existing secret. List uses UUID pagination,
1–100 rows. Rotate/revoke require expected_revision. Rotation replaces the secret,
increments revision, retains scopes/expiry and explicitly rebinds to the creator's
current membership revision. Revoked/expired keys cannot rotate. Revoke increments
revision and is terminal. Each mutation and attributable audit commit atomically.

Secret is returned once on create/rotation with Cache-Control no-store; metadata
never includes verifier or secret. A lost response requires inspecting metadata
and explicitly rotating at the current revision, or revoking; no automatic replay.
Owner can revoke another creator's key but cannot obtain a replacement for it.
No overlapping-secret grace period. Successful authentication snapshots authority
for a bounded request; revocation affects subsequent authentication, not retroactive
cancellation of an already admitted transaction. No authorization cache.

Migration 0005 extends existing NOLOGIN capabilities and forced RLS. Runtime gets
named function execution and audit SELECT only, no raw credential-table access.
All management uses tenant locking and READ COMMITTED; SQL repeats privilege/
revision/bounds checks. Authentication function reads credentials only and is not
exposed through HTTP. GUCs still rely on trusted adapters; protect runtime SQL creds.

Acceptance: real key generation, hashed storage, malformed/forged/unknown keys,
tenant/scope isolation, current membership revision/expiry, expiry/revocation,
concurrent rotation, atomic rollback, single secret disclosure, SQL permissions,
HTTP/locales and >90% line/branch per component. Provider login, full tenant
lifecycle, UI, secret delivery, distributed rate limits, OTel, retention and
containers remain pending. Only disposable fixtures are migrated; no deployment.
