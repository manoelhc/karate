# Primary-database tenant erasure

Local implementation/runbook; no shared infrastructure action taken. Production
use is blocked by backup/restore, provider propagation, legal/support hold,
container and full platform prerequisites in the current run.

Migration 0008 must follow 0001–0007 before the current API and runner start.
API runtime needs EXECUTE on `deletion_state(uuid)`, `deletion_info(uuid)` and
`request_deletion(uuid,bigint,integer,boolean)` through its existing narrow grant
list. Never give API credentials erasure_runner/guard membership. Only an
operator-created, nonsuperuser, non-owner, NOBYPASSRLS login receives membership
in NOLOGIN `karate_erasure_runner`. It receives no raw tenant table privileges.
These are deployment instructions, not commands executed on a shared account.

Supply that dedicated Unix-socket DSN as `KARATE_ERASURE_DATABASE_URL`, then run
`make erasure-batch`. Do not store or print the DSN in evidence or shell history.
One invocation performs one eligible tenant batch and exits. Output JSON contains
only `outcome` and `deleted_rows` (0–100); no identities or tenant IDs. Values are
`idle`, `blocked_authority`, `progress`, `primary_database_erased`. Nonzero exit
means invalid configuration/role or unavailable operation; inspect restricted DB
health, privileges and migration inventory without logging credentials. Do not
blindly loop on errors. Connection/statement/lock/idle transaction limits are
2/3/1/5 seconds. No persistent schedule or daemon is installed.

Scheduling and cancellation are available to current owners in the React workspace
and REST contract. They require exact tenant confirmation, 7–30 days and current
revision. Never shorten a real request's recovery deadline using operator SQL.
A lost HTTP response is recovered by GET metadata, not automatic mutation replay.
Cancel pending/blocked work before resuming the suspended workspace explicitly.
Erasing cannot be cancelled or resumed. New sign-in/session/audit writes are
interlocked; an in-flight transaction is ordered by the same tenant lock.

Each committed batch is a checkpoint. If a worker dies before commit, the batch
rolls back; another invocation can continue. SQL failures leave the phase and rows
unchanged. Unknown tenant stores refuse erasure until the allowlist, dependency
order, grants, write barriers and tests are updated. Never weaken this inventory
check to make a new schema appear supported.

Only restricted operators can inspect `erasure_receipts`; its three fields are
tenant UUID, request UUID and completion time. Keep receipts across database
restores and reconcile them before opening traffic—this restore procedure is not
yet implemented/verified and therefore blocks release. Receipt visibility is not
customer evidence of backup/media/provider erasure. Operator support receives no
authentication bypass or substitute authority in this implementation.
