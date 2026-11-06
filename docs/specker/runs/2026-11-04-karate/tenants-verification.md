# Tenant verification — TASK-003b2e1

Base: 4225dc9b1852c2b6b896848088a4fd1461ab6287. Verified 2026-11-06 on arm64 macOS
with Rust 1.99.1, PostgreSQL 14.20 and the existing Podman/Dex fixture. Owner: backend
implementer; PLAT-TENANT/RBAC/AUDIT. [Component](../../../components/tenants.md),
[ADR-0009](../../../adr/0009-explicit-tenant-provisioning.md),
[contract](../../../contracts/projects-v1.md) and [runbook](../../../operations/project-api.md)
define the supported behavior. No dependency or product license changed.

## Executed checks

| Check | Result | Evidence |
| --- | --- | --- |
| Formatting, strict Clippy, tests, build, licenses/bans/sources | Pass: 43 named Rust tests, none failed/skipped | [log](evidence/tenants/rust-checks.log) |
| PostgreSQL migrations 0001–0007, isolation, lifecycle, restart | Pass | [log](evidence/tenants/postgres.log) |
| Strict per-component coverage | Pass | [summary](evidence/tenants/coverage-summary.json), [LLVM](evidence/tenants/coverage-llvm.json) |
| Refreshed RustSec audit | Pass: 253 packages, zero vulnerabilities/warnings, no ignores | [report](evidence/tenants/audit.json) |
| Gitleaks | Pass, zero findings | [report](evidence/tenants/gitleaks.json), [log](evidence/tenants/secrets.log) |
| Artifact/source checks and cleanup | Pass | [checks](evidence/tenants/artifact-checks.log), [snapshot](evidence/tenants/snapshot.json), [cleanup](evidence/tenants/cleanup.log) |

| Component | Lines | Branches |
| --- | --- | --- |
| karate-core | 462/462 (100%) | 266/272 (97.794%) |
| karate-api | 2262/2308 (98.007%) | 346/378 (91.534%) |

Both components independently exceed 90% for both metrics. Coverage uses the
separate pinned nightly-2026-11-04 instrumentation compiler and matching LLVM;
production stays Rust 1.99.1. New tenant modules are included, with no new exclusion.
Existing exclusions are tests, compiler/dependency code and the source-checked
module-only API lib.rs. SQL behavior is tested in PostgreSQL, not assigned a
fabricated Rust denominator. Source/lock hashes accompany the report. The final
snapshot covers code, tests, migration, contracts and docs, excluding historical
evidence itself. Commands include actual timestamps and exit status. Terminal trailing whitespace
was removed from captured logs; check results and substantive output are unchanged.

## Behavior verified

Grant tests reject forged identity, wrong addressed owner, foreign tenant,
unknown grant, API key/session bootstrap, expiry, revocation and replay. Concurrent
redemption succeeds exactly once. Issuance is operator-only: runtime cannot issue
or revoke grants, read their table, write tenant rows or forge audit. Creation
produces one permanent owner; audit insertion failure leaves neither tenant,
owner nor consumed grant. Operator grant lifetime/duplicate validation and stale
raw transaction isolation fail closed.

Lifecycle tests cover active metadata for members, owner-only mutation, suspended
metadata denial for nonowners, no delegated-key control plane, invalid state/name/
revision, stale writers, no-op without duplicate audit, and mutation audit rollback.
Suspension blocks projects, keys, member administration and invitation acceptance.
Raw project RLS hides suspended rows; capability-level administration also denies.
Resume retains data and restores independently current credentials. Prior grants
cannot restore/reset ownership or bypass suspension. Tenant audit stays scoped.

Concurrency is exercised against real locks, with pg_stat_activity observations
before releasing the fixture: suspension waits for an admitted project write and
its atomic audit to commit. Work started after suspension is denied. An owner
revoked while a lifecycle call waits cannot mutate after the lock is acquired.
Post-wait membership checks use READ COMMITTED; stronger raw isolation is refused.
Existing membership concurrency and permanent-owner regression tests also pass.

The browser regression signs in an owner, suspends the tenant, verifies data denial,
signs in again for recovery, resumes through the session service and revokes
sessions. HTTP router checks cover tenant GET/PATCH/provision status/body validation,
localized errors and no-store. Existing cookie CSRF and real socket/process tests
remain in the suite. Digest-pinned maintained Dex protocol tests pass as part of
normal tests/coverage; this is synthetic-connector protocol evidence, not a human
provider/React journey or the full Karate container gate.

## Operational boundary and remaining gates

Only synthetic fixtures were provisioned: operator grants and migration 0007 in
private disposable PostgreSQL clusters, and temporary Dex containers in the
existing local Podman machine. No shared operator/runtime role, system trust,
external account, production migration, paid resource or deployment changed.
All fixture DBs, generated keys/configuration and named containers are removed;
workspace build outputs are cleaned at handoff. MIT and Cargo.lock remain unchanged.

Suspension retains data; it is not deletion or a retention policy. Grant UUID
reservations are deliberately not recycled. Deletion request/propagation, backup
expiry, erasure proof, operator support tooling, distributed rate controls,
production TLS/secrets, OTel, full Karate containers/recovery/performance/security,
React/a11y, human provider/SSO logout, assessment/MCP/voice/billing and remaining
research are still mandatory gates. No scope or coverage prerequisite is waived.

Previous 4225dc9 exact-commit CI passed. New exact-commit CI is checked after the
direct push to main. No PR or production deployment is part of this checkpoint.
Next: TASK-006a React sign-in/session and tenant recovery UI with real browser
verification; tenant erasure remains a separate explicitly open lifecycle slice.
