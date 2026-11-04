# Continuation handoff

State: **partial implementation**. TASK-003b2a adds existing-member directory,
role changes and revocation to the shared Rust service/API, with revision checks,
atomic audit and concurrent permanent-owner protection. Admins manage member/viewer
roles; owners manage all roles. Authority is rechecked after waiting for the tenant
lock, under explicit READ COMMITTED isolation. Runtime SQL cannot write membership
rows directly or assume the narrow NOLOGIN capability role.

All 26 named Rust tests pass, including real PostgreSQL/HTTP/process checks and
concurrency/privilege cases. Core: 324/324 lines, 206/212 branches. API: 678/696
lines, 119/122 branches. Both exceed 90% separately. Dependency license/source,
RustSec and secret scans pass. Snapshot hash formatting was corrected without
scanner suppression; prior digest values were preserved and verified. See
[membership verification](membership-verification.md) for commands and evidence.

Production Rust remains 1.99.1; coverage uses nightly-2026-11-04. `make lint test
build licenses test-db coverage`, `make audit`, `make secrets` verify the slice.
`make run` requires explicit issuer/JWKS/database configuration. Migration 0003 is
additive and has only run in disposable local fixtures; read the
[runbook](../../../operations/project-api.md) before an authorized environment change.

Next: invitation acceptance bound to verified identity, expiry/replay and audited
re-enrollment, then tenant lifecycle, scoped API keys and full OIDC login. UI,
worker, containers, providers, operations and the full platform/research baseline
remain open. No production deployment, paid resources or third-party assessment.
The user requires commits/pushes directly to main without PRs. Resume [run.md](run.md).
