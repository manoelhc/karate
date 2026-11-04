# ADR-0004: serialized membership administration with narrow database capabilities

Accepted for TASK-003b2a, 2026-11-04; owner: backend implementer.

Existing membership RLS exposes only the current caller's active row. Project and
audit policies depend on that visibility. Broadening it to an administrative
directory would change the meaning of their membership subqueries and risks
accidental privilege expansion. Keep those policies unchanged.

Add three callable SECURITY DEFINER capabilities for listing, a locked target
snapshot, and a guarded role/revocation change. A fourth helper is private. All
functions use a fixed trusted search path, qualified tables, parameterized inputs,
explicit tenant/current actor checks, and revoked PUBLIC execution. A dedicated
NOLOGIN role owns them, bypasses RLS, and has only necessary table/column grants.
The runtime has no membership in this role; startup rejects that misconfiguration.
No direct membership writes or audit append/update/delete grants go to runtime.
No new Rust dependencies are introduced; MIT licensing remains unchanged.

The Rust shared service authenticates, authorizes management, obtains the tenant
lock/target facts, reauthorizes the actor and applies core role/ownership policy.
The final SQL capability repeats storage guards and appends audit in the same
transaction. Locks cover a tenant rather than individual owners to serialize
cross-row ownership changes; fixed SQL timeouts bound contention. No automatic
mutation retry is added. Expiring owners cannot be the sole durable successor.

This deliberately introduces a privileged database capability boundary. Function
ownership/grants/search paths are security-sensitive and require real negative
SQL tests. GUCs identify a previously authenticated caller; they do not make
runtime SQL credentials safe for untrusted clients. Keep SQL credentials private.
All maintenance writes must take the same tenant lock. A privileged operator can
otherwise bypass invariants and is outside runtime policy guarantees.

Migration 0003 needs privileged role creation and backfills UUIDs on existing
memberships. Only disposable fixtures are migrated here. Shared environments need
authorized migration planning and backup/restore validation; production is not
provisioned. Invitations, tenant lifecycle, restoration, keys, full OIDC, UI,
OTel and container verification remain explicit follow-up gates.

References: PostgreSQL [definer-function safety](https://www.postgresql.org/docs/14/sql-createfunction.html)
and [row locks](https://www.postgresql.org/docs/14/explicit-locking.html).

Management transactions explicitly use READ COMMITTED so post-lock queries see
newly committed authority. Raw SQL mutation capabilities reject other isolation
levels; a stale repeatable-read snapshot must never count a removed owner. Tests
cover a serializable connection default and a rejected raw repeatable-read call.
