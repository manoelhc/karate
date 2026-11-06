# ADR-0010: bounded primary-database tenant erasure

Accepted for local implementation, 2026-11-06. No production rollout authority.

Use a separate Rust 1.99.1 one-batch executable and a narrow NOLOGIN PostgreSQL
capability. Current interactive owner confirmation and revision schedule a 7–30
day recovery period, suspend ordinary access and write attributable audit. Any
current owner may cancel before the first batch; cancellation stays suspended.
The original requester must still be an owner at the first destructive batch.

One tenant lock and SQL transaction contain at most 100 customer-row deletions
and phase progress. SKIP LOCKED permits independent runners, rollback preserves
rows/checkpoints together, and each fresh process continues committed progress.
No network action or external side effect occurs, so a separate leased job with
fencing is unnecessary here. The assessment worker will still require leases.
The runner cannot read customer tables or start the API. Definer search paths and
fixed table identifiers are explicit. Unknown tenant stores fail closed.

A minimal restricted UUID/request/time receipt outlives the customer rows and
blocks UUID re-provisioning. It proves only logical deletion in the primary
PostgreSQL database. Backups, WAL, replicas, restored copies, external providers,
legal/support holds and general age retention require their own policies and real
verification before release. No compliance or physical erasure assertion follows
from this receipt. No automatic production schedule is configured.

Additive migration 0008 precedes API/runner deployment. The new JSON property is
additive in this unreleased API; the paired UI requires it. Before shared rollout,
review compatibility, take/verify snapshots and design a forward rollback: after
first erasure, an application rollback cannot restore the deleted records.
