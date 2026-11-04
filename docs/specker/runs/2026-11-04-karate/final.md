# Continuation handoff

State: **partial implementation**. The runnable Rust project API now verifies
signed access tokens before resolving current tenant membership, supports bounded
project reads and idempotent creation/revision updates, and commits attributable
audit records atomically. English and pt-BR errors share a closed contract.

All 24 named Rust tests pass, including real PostgreSQL, HTTP and process tests.
Core coverage is 304/304 lines and 194/200 branches; API coverage is 509/523 lines
and 107/110 branches. Both components exceed 90% separately. RustSec, dependency
license/source checks and local database isolation/restart checks pass. See
[project verification](projects-verification.md) for commands and evidence.

Production Rust remains 1.99.1; branch instrumentation uses the separately pinned
nightly-2026-11-04. Public JWKS snapshots expire and require operator rotation.
Complete OIDC login, member/key administration, frontend, worker, assessment,
containers, operational readiness and the full platform baseline remain open.
Research remains nine of 55 inventoried pages substantively read. These gaps
prevent a release-readiness claim; implemented component gates do not waive them.

Use `make lint test build licenses`, `make test-db`, `make coverage`, `make audit`
and `make secrets`; see [local setup](../../../operations/local-development.md)
and the [API runbook](../../../operations/project-api.md). `make run` requires
explicit identity/database configuration and starts the loopback resource server.

Next: TASK-003b2 audited tenant/membership administration, invitation expiry/replay
and last-owner protections, scoped API-key lifecycle, and complete OIDC integration.
Resume from [run.md](run.md). The user authorized direct commits and pushes to main
without PRs. No deployment, paid resources or third-party assessment occurred;
MIT licensing is preserved. Git history records publication of this checkpoint.
