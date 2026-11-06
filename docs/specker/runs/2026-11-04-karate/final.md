# Continuation handoff

State: **partial implementation**. TASK-003b2e1 adds explicit operator-granted
creation of a tenant and permanent owner, atomic audit, tenant metadata and
owner-only rename/suspend/resume. Ordinary service work requires active state;
current owners can sign in and recover suspended tenants. Grants never reset
ownership. Suspension retains data and is not deletion/erasure.

All 43 Rust tests pass, plus real SQL boundary/isolation/restart checks. Coverage:
core 462/462 lines, 266/272 branches; API 2262/2308 lines, 346/378 branches. Both
independently exceed 90%. Formatting, Clippy, build, license/source/bans and refreshed
RustSec pass. Final secrets/artifact/cleanup evidence and exact command records are
in [tenant verification](tenants-verification.md). Cargo.lock and MIT unchanged.

Run `make lint test build licenses test-db coverage`, `make audit`, `make secrets`.
Rust 1.99.1 is production; nightly-2026-11-04 is coverage only. Docker or a running
Podman machine supplies the pinned Dex fixture. Migration 0007 only ran in private
clusters. See [operations](../../../operations/project-api.md) for exact runtime/
operator grants, rollout ordering and recovery. No shared infrastructure/deployment.

Next: TASK-006a React sign-in/session and tenant recovery journey, localization,
a11y and actual browser/provider checks. Deletion/retention, operator support,
provider SSO logout, full Karate containers/recovery/telemetry, assessment workers,
MCP/voice/billing and research parity remain open. Continue direct commits/pushes
to main without PRs; resume [run.md](run.md).
