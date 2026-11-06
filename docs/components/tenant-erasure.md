# Tenant retention and primary-database erasure — TASK-003b2e2

Implementation contract, 2026-11-06. Rust owns policy, REST/shared service and a
separate one-batch erasure executable; React owns request/cancel/recovery states.
No production database, real customer data, external account or deployment action.

A current interactive owner must confirm the exact tenant UUID and current tenant
revision, choosing a 7–30 day recovery window (engineering bounds, not a legal
retention assertion). Scheduling atomically suspends the workspace, records intent
and audit, and advances its revision. Pending deletion prevents ordinary resume or
rename. Any current owner can cancel until erasure starts; cancellation leaves the
workspace suspended for explicit review/resume. Reads recover a lost POST response;
no mutation automatically retries. One current request per tenant bounds metadata.
A cancelled request can be replaced only using the current revision.

The separate Rust erasure process uses an explicitly granted database runner role,
not API credentials. One invocation performs at most 100 row deletions for one
eligible tenant, in a single bounded transaction with SKIP LOCKED and the same
exclusive tenant lock used by lifecycle changes. No long-lived lease is needed:
commit is the checkpoint, and process death rolls the entire batch back. Multiple
runners cannot overlap one tenant. Before the first destructive batch, the recorded
requester must still be a current owner; otherwise the request becomes blocked and
requires current-owner cancellation/review. Once erasing starts it cannot resume or
cancel; service/browser recovery also closes so no new tenant rows can be created.

Erase dependent audit rows before credentials/projects/memberships, then grants
and tenant metadata. The fixed table inventory fails closed when a future tenant
store has not been integrated. An FK-free, restricted receipt contains only tenant
UUID, request UUID and completion timestamp. It permanently prevents tenant-ID
reuse; it is a logical primary-database receipt, not physical-media/backup proof.
No identity, workspace name, credential hash or customer evidence survives in it.

Backups/WAL/replicas, external providers, support/legal holds, configurable general
data-age retention, and restore-time tombstone reconciliation remain explicit
release gates. No automatic production schedule is installed. The operator runs
bounded batches with a dedicated least-privilege role; API roles cannot run them,
and the erasure role cannot start the API or read customer rows directly.

Verification: domain rejection matrix; actual PostgreSQL row bounds, fresh-process checkpoint continuation and
transaction rollback, current-owner loss, cancellation/start races, denied raw privileges,
foreign-tenant preservation and anti-resurrection; real HTTP/cookie/UI journey;
actual worker binary invalid-config/success/error paths; >90% line AND branch
coverage separately for core, API, erasure runtime and web; lint/build/license/audit,
exact-commit CI. Full platform/container gates remain open until actually verified.
