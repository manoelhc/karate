# Continuation handoff

State: **partial implementation**. TASK-003b2b adds identity-bound invitation
issue/list/revoke/acceptance and revision-safe re-enrollment. Signed identity,
current inviter authority, expiry, tenant locking and captured revisions prevent
replay and stale restoration. Enrollment and audit commit atomically. UUIDs only
locate offers; no mail or bearer-link authentication is introduced.

All 28 Rust tests pass, including real signed-token, HTTP, PostgreSQL, process,
concurrency and rollback cases. Core: 360/360 lines, 224/230 branches. API: 926/947
lines, 133/138 branches. Both exceed 90% separately. Dependency/license/source,
RustSec and secret checks pass. See [invitation verification](invitations-verification.md).

Use `make lint test build licenses test-db coverage`, `make audit`, `make secrets`.
Production remains Rust 1.99.1; coverage uses nightly-2026-11-04. Migration 0004 has
only run in disposable fixtures. `make run` still requires explicit identity and
database configuration; see the [runbook](../../../operations/project-api.md).

Next: tenant-scoped API keys with hashed verifiers, limited scopes, expiry,
rotation/revocation; retain tenant lifecycle and full OIDC work. Provider onboarding,
UI, assessments, containers and the complete platform/research baseline remain
open. No shared migration, production deployment, paid resource or third-party
assessment. Direct commits/pushes to main without PRs remain authorized. Resume
[run.md](run.md).
