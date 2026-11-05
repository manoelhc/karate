# Membership administration — TASK-003b2a

Owner: backend implementer. Requirements: PLAT-TENANT/RBAC/AUDIT. This slice
manages existing memberships. [Invitations](invitations.md) now supply explicit
re-enrollment; tenant provisioning, API keys and full OIDC remain in TASK-003b2. No new dependency or license change.

Contract: GET /v1/tenants/{tenant}/memberships with limit 1–100 and after UUID;
PATCH /v1/tenants/{tenant}/memberships/{membership} with role and expected_revision;
POST /v1/tenants/{tenant}/memberships/{membership}/revoke with expected_revision.
Responses carry membership_id, issuer, subject, role, revision, revoked and expiry
as Unix seconds (null for permanent). Lists include inactive members for review;
mutations require an active target and matching revision. No-op role changes do
not advance revision or audit. Lost responses require a list/read-state check
before retry; stale mutations return CONFLICT. Reactivation requires a new identity-bound invitation with a matching revision.

Authenticate before database access. Shared Rust policy permits owner/admin reads;
admins may change/revoke only members/viewers and assign only member/viewer.
Owners may manage all roles. Service keys cannot manage memberships. Removing an
owner requires another current, nonexpiring owner (otherwise OWNER_REQUIRED). Assigning owner requires a
nonexpiring target. Serialize changes per tenant, then recheck actor and target
under that lock. Expiring owners do not count as durable ownership successors.

Keep existing membership RLS unchanged (own active row only). Migration 0003 adds
stable tenant-scoped membership IDs/revisions and a separate append-only audit
family. Narrow SECURITY DEFINER functions use a dedicated NOLOGIN BYPASSRLS role
with only table-specific grants, fixed pg_catalog/pg_temp search_path, qualified
relations, revoked PUBLIC execute and no dynamic SQL. The runtime is never a
member of this role and gets only named function execute grants; direct membership
writes stay denied. Functions repeat policy as storage defense. GUCs still rely
on trusted credential verification; SQL credentials themselves are privileged.
The migration requires a privileged local/bootstrap identity and creates this
role only in isolated fixtures during development. No shared database change.

Audit captures actor issuer/subject, target membership ID, old/new role, action,
revision and server time, in the same transaction as the change. No token, email,
profile or customer capture is collected. Owner/admin audit reads are tenant
scoped. Retention, erasure and external tamper detection remain launch gates.

Verification: real signed JWT + PostgreSQL + HTTP; role matrix; last-owner and
concurrent owner removal; revocation while waiting for the tenant lock; stale
revision; expiry; issuer/tenant collisions; audit-failure rollback; function grants
and search-path shadowing; paging; localized validation. All Rust source remains
inside per-component >90% line/branch checks. No container/provider/UI claim.

Design references: PostgreSQL [safe definer functions](https://www.postgresql.org/docs/14/sql-createfunction.html)
and [row locking](https://www.postgresql.org/docs/14/explicit-locking.html).

Management transactions explicitly use READ COMMITTED so post-lock queries see
newly committed authority. Raw SQL mutation capabilities reject other isolation
levels; a stale repeatable-read snapshot must never count a removed owner. Tests
cover a serializable connection default and a rejected raw repeatable-read call.
