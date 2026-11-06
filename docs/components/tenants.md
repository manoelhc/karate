# Tenant provisioning and lifecycle — TASK-003b2e1

Owner: backend implementer; PLAT-TENANT/RBAC/AUDIT. Status: backend slice verified locally; [evidence](../specker/runs/2026-11-04-karate/tenants-verification.md).
No new dependency or backend language. Product remains partial; no deployment.

An operator issues a tenant-scoped provisioning grant for an exact verified issuer/
subject, a tenant UUID and name, with expiry at most seven days. Runtime cannot
issue grants. The authenticated addressed access JWT may redeem it once; neither
its UUID nor a provider claim alone grants ownership. API keys and tenant sessions
cannot bootstrap tenants. Creation, permanent owner, consumed grant and tenant
creation audit are one READ COMMITTED transaction. Missing/wrong/revoked/expired/
consumed grants fail closed; retry after lost response uses GET tenant, not reissue.
The operator records a nonsecret approval reference; no customer email is needed.

GET /v1/tenants/{tenant} returns metadata for current interactive members when
active; only current owners can inspect suspended tenants. PATCH on that path
requires a current owner and expected_revision, name and state (active/suspended).
It is compare-and-set; even no-op requires current revision, and no-op creates no
audit/revision. Only active and suspended states exist. Primary-database deletion is implemented separately; see [tenant erasure](tenant-erasure.md).

Shared authorization checks current tenant state on every normal operation. A
KEY SHARE tenant lock holds through bounded work, while lifecycle changes take
UPDATE before re-resolving current owner authority. Existing membership mutations
use NO KEY UPDATE, compatible with KEY SHARE. This prevents a request from
committing work after suspension and avoids shared-lock upgrade deadlocks.
Invitation acceptance also takes the active-tenant guard. SQL storage guards/RLS
protect inactive project access and control-plane mutations in addition to Rust.

Owner browser sign-in and tenant metadata remain usable while suspended, allowing
recovery; ordinary data/membership/key/invitation operations stay blocked. Logout
may still revoke one's own sessions. Suspension does not revoke membership or
credentials; resume restores still-current, unexpired permissions. Credentials
revoked independently never revive. No claim of cancellation of future workers:
workers must integrate this shared policy when implemented.

Verification: isolated real PostgreSQL, tenant/identity/role/revision/replay/audit
failures, concurrent grant redemption, suspension/operation serialization,
post-lock owner revocation, raw-runtime restrictions, browser recovery, HTTP error
localization, strict >90% component coverage and maintained Dex regression.
Commands: make lint test build licenses test-db coverage; audit/secrets; exact CI.

Retention boundary: suspension retains all tenant data and audit. Grants contain
sensitive identity references and operator approval metadata; encrypt storage and
restrict operator access. Owner-confirmed delayed deletion and primary-database logical receipts now exist.
General retention schedules, external propagation, backups and full erasure proof
remain release gates. No shared/production migration
is authorized; migration 0007 runs only in private disposable fixtures.

TASK-006a adds `can_manage` to tenant responses, derived from the current service
principal's owner permission. It is a presentation hint only; all writes continue
to enforce current membership and authorization inside the transaction.

Pending deletion blocks rename/resume, and erasing closes owner browser recovery.
Tenant responses now include owner-only nullable deletion metadata. Migration
0008 is required before the current API/erasure binary; apply only to isolated
fixtures in this run.
