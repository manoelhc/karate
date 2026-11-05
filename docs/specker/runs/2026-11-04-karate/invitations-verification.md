# Invitation verification — TASK-003b2b

Owner: backend implementer; PLAT-TENANT/RBAC/AUDIT. Delivery base:
`e83ee45dc13b84db89922cf1f631c8dd5aa05867`. Checked 2026-11-05 on aarch64 macOS,
Rust 1.99.1, PostgreSQL 14.20 and Python 3.14.6. Migration 0004 followed 0001–0003
only inside disposable private Unix-socket fixtures. No shared/production change.

## Executed gates

| Check | Result | Evidence |
| --- | --- | --- |
| Format, strict Clippy, workspace tests, build | Pass: 28 named tests, zero failed/skipped | [log](evidence/invitations/rust-checks.log) |
| License/source/dependency bans | Pass; no new dependencies or lockfile changes | [log](evidence/invitations/rust-checks.log) |
| Real database isolation, rollback and restart suite | Pass | [log](evidence/invitations/postgres.log) |
| Strict per-component line/branch coverage | Pass | [summary](evidence/invitations/coverage-summary.json), [LLVM report](evidence/invitations/coverage-llvm.json) |
| Refreshed RustSec audit | Pass; zero findings/warnings, no ignored advisories | [report](evidence/invitations/audit.json) |
| Secret scan | Pass | [log](evidence/invitations/secrets.log), [report](evidence/invitations/gitleaks.json) |

[Commands](evidence/invitations/commands.json) record timestamp and exit code.
[Snapshot](evidence/invitations/snapshot.json) hashes source/docs/config/lock files;
evidence is excluded to avoid recursive hashes. Coverage independently hashes
product source and Cargo.lock. MIT license and locked dependency graph are unchanged.

| Component | Lines | Branches |
| --- | --- | --- |
| karate-core | 360/360 (100%) | 224/230 (97.391%) |
| karate-api | 926/947 (97.782%) | 133/138 (96.377%) |

Branch instrumentation uses nightly-2026-11-04 (rustc 1.101.0-nightly 708c94ef0,
LLVM 23.1.5), not a production compiler change. Every executable product file,
including invitation logic and API subprocess startup, is inventoried. Existing
exclusions remain test/third-party code and the source-checked nonexecutable API
module index. Raw counts, not rounded percentages, enforce strictly >90% per
component. SQL behavior has real PostgreSQL tests, not an invented Rust denominator.

## Behavior and failure evidence

Real signed-token/DB/HTTP tests exercise exact identity and tenant binding,
role escalation refusal, duplicate IDs, creation retries, expiry/revocation/replay,
current inviter role/revision/expiry and recipient revision drift. Concurrent
acceptance of the same offer or two competing offers enrolls exactly once.
Re-enrollment preserves member ID and increments revision; an earlier offer cannot
undo a later revocation. Expired/revoked/changed inviters cannot leave valid old
delegation behind, even after regaining their role. Owner offers never grant owner.

Audit failures roll back issue, revoke and acceptance; acceptance can subsequently
succeed after dependency recovery. The new invitation audit family captures
enrollment and re-enrollment alongside immutable grant/prior-revision metadata.
Raw SQL tests deny unscoped event reads, direct mutation, foreign issuer/tenant,
unbounded lists, helper execution and repeatable-read acceptance. PUBLIC function
execution stays revoked. Existing project/membership/owner guards still pass.

One initial coverage run failed in the added raw-isolation test: a combined SQL
batch queried before BEGIN ISOLATION LEVEL. Separate calls corrected the fixture;
the complete suite and coverage were rerun. [Initial failure](evidence/invitations/coverage-initial.log)
and [command record](evidence/invitations/commands-initial.json) are retained.
No product security check was relaxed or skipped. SQL subject bounds now match
the service's 255-byte limit. No private signing key or actual customer capture
is committed; all fixture keys/clusters are removed after execution.

## Outstanding acceptance

This implements resource-server invitations, not full provider login or email
onboarding. Operators need a trusted exact provider-subject source. No messages
are sent. Full OIDC/PKCE/JWKS automation, provider directory/delivery, tenant
lifecycle, scoped API keys, UI, worker/assessment, MCP/voice, billing, OTel, rate
coordination, TLS, audit retention/erasure, containers, recovery/performance and
complete research remain open. Docker is absent. No production deployment or
paid resources. Release readiness is not claimed.

Prior e83ee45 passed remote CI. The new exact-commit run is checked after pushing;
local results alone do not claim remote CI success. Next: API-key lifecycle with
hashed verifiers, bounded scopes, expiry, rotation and revocation.
