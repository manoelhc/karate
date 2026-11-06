# ADR-0009: explicit tenant grants and serialized lifecycle

Accepted 2026-11-06 for TASK-003b2e1. Rust 1.99.1 and MIT unchanged; no new library.

First login must not create an owner. An independently authorized operator issues
one tenant-scoped grant addressed to an exact issuer/subject and approved tenant
name/UUID. Only an authenticated access JWT can claim it. Grant UUIDs are locators,
not secrets or authority. Tenant sessions and API keys cannot claim grants. The
operator capabilities have no PUBLIC/runtime execution grant; the application
never receives a platform-admin bypass or migration-owner credentials.

A grant is immutable except consumption/revocation, expires within seven days and
reserves its tenant UUID permanently. Revoked/expired grants cannot be refreshed
or recycled; a separately approved new tenant UUID is required. This intentionally
avoids resurrecting prior ownership. SQL validates again while holding the grant
row lock. Tenant, permanent owner, grant consumption and creation event commit or
roll back together. A lost response is resolved with authenticated tenant GET.

Tenant metadata has active/suspended state and a compare-and-set revision. Current
interactive owner alone can rename/suspend/resume; the existing owner-only
DeleteTenant policy permission gates lifecycle management without adding a delete
endpoint. Normal service operations require active state and hold a KEY SHARE
row lock through commit. Lifecycle takes UPDATE, then reads current owner again.
Existing member changes use NO KEY UPDATE, compatible with the normal guard;
ordinary reads remain concurrent. The [PostgreSQL lock table](https://www.postgresql.org/docs/14/explicit-locking.html)
supports these conflicts; [READ COMMITTED](https://www.postgresql.org/docs/14/transaction-iso.html)
supplies post-wait statement visibility. Real tests exercise the ordering.

All service transactions touching this guard explicitly use READ COMMITTED;
stale raw isolation is rejected. Membership is resolved again after lock waits,
so a prior role/session revision cannot retain authority. Membership/invitation
storage capabilities reject inactive tenants, and restrictive project/audit RLS
adds defense in depth. Runtime credentials remain trusted adapter credentials;
GUCs are not a separate authentication boundary against a stolen DB login.

Suspended owners may sign in and inspect/change tenant metadata for recovery;
ordinary project/member/invitation/key operations stay denied. Current members
may revoke their own sessions. Suspension retains data and does not revoke or
extend credentials; resume restores only independently current authority. No
future worker cancellation or deletion propagation is claimed by this slice.

Deletion request/erasure workflows, retention schedules, backup expiry, distributed
abuse control, production TLS/secret custody, React and complete provider/platform
gates remain mandatory. Migration 0007 is additive and defaults existing tenants
to active revision 1, leaving existing identity/ownership unchanged. It has run
only in disposable fixtures. In an authorized environment apply schema/grants
before the new binary; old binaries cannot enforce lifecycle state through the
new Rust guard, so drain old instances before enabling suspension. No destructive
down migration or production rollout is performed here.
