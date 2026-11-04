# Membership administration verification — TASK-003b2a

Owner: backend implementer. Requirements: PLAT-TENANT/RBAC/AUDIT/I18N.
Delivery base: e530e906bf0329dd9533d7c78991e3071dd1d8b3. Local checks on 2026-11-04,
aarch64 macOS, production Rust 1.99.1, PostgreSQL 14.20. Migrations 0001–0003 ran
only in disposable private Unix-socket clusters. No production/shared migration.
Full TASK-003b2 and the full platform remain incomplete.

## Executed gates

| Check | Result | Evidence |
| --- | --- | --- |
| Formatting, strict Clippy, workspace tests and build | Pass; 26 named tests, zero failed/skipped | [log](evidence/membership/rust-checks.log) |
| Licenses, dependency bans and sources | Pass; no new dependencies, unchanged Cargo.lock | [log](evidence/membership/rust-checks.log) |
| Real SQL isolation/revocation/rollback/restart | Pass | [log](evidence/membership/postgres.log) |
| Per-component >90% lines AND branches | Pass | [summary](evidence/membership/coverage-summary.json), [LLVM](evidence/membership/coverage-llvm.json) |
| Refreshed RustSec audit | Pass; zero vulnerabilities/warnings, no ignores | [report](evidence/membership/audit.json) |
| Working-tree secret scan | Pass after hash-record formatting correction | [log](evidence/membership/secrets.log), [report](evidence/membership/gitleaks.json) |

[Commands](evidence/membership/commands.json) record timestamps/exit codes;
[snapshot](evidence/membership/snapshot.json) binds source/docs/config files to
SHA-256 digests, excluding evidence itself. Test fixtures include ephemeral signing
keys and fictional identities only, cleaned up by the fixture trap.

| Component | Line numerator/denominator | Branch numerator/denominator |
| --- | --- | --- |
| karate-core | 324/324 (100%) | 206/212 (97.170%) |
| karate-api | 678/696 (97.414%) | 119/122 (97.541%) |

Coverage uses nightly-2026-11-04, rustc 1.101.0-nightly 708c94ef0, LLVM 23.1.5;
production stays 1.99.1. Inventory includes the new membership module, every other
executable product file and API subprocess startup. Only test/third-party code and
the checked nonexecutable API module index are excluded. Raw counts are compared
strictly without rounding. SQL behavior is exercised on PostgreSQL, not falsely
assigned a Rust line/branch denominator.

## Security and failure cases

The domain role matrix covers owner/admin/member/viewer actors against every
target/desired role and revoke, foreign tenants and service-key restrictions.
Real signed-token, HTTP and DB tests prove directory permissions, bounded paging,
active-target/revision checks, owner promotion restrictions, last-owner protection,
revocation, self-revocation, and no-op behavior. Audit insertion failure rolls back
the mutation. Audit reads remain tenant/admin scoped and direct writes are denied.

Two concurrent self-demotions start with two owners; exactly one commits. A queued
administrator actually waits on a tenant row lock, is revoked before release and
is then refused. Membership transactions explicitly select READ COMMITTED even
when the connection default is serializable. Raw repeatable-read mutation calls
are rejected, preventing stale post-lock authority and owner counts.

Raw SQL tests check foreign tenant/issuer denial, stale/invalid/last-owner errors,
admin escalation refusal, direct membership/event write denial, revoked PUBLIC
function execution, inability to assume the definer role and temporary-table
shadowing resistance. Startup rejects runtime membership in the privileged role.
Existing own-membership RLS and project regressions still pass unchanged.

OWNER_REQUIRED is an additive closed 409 code with en/pt-BR instructions. Unknown
fields/roles, invalid UUIDs, missing credentials and stale requests have bounded
localized responses. No private token, email/profile or actual customer capture
enters events or committed reports.

## Scan correction and evidence integrity

The initial Gitleaks run flagged four SHA-256 values adjacent to API-related paths
in the historical project snapshot. Every digest was verified against the exact
e530e90 file bytes. The historical representation changed from a path-to-hash map
to explicit path/sha256 records, retaining all digest values and provenance. The
new snapshot uses the same unambiguous format. No scanner rule/path was suppressed
and no credential was found. [Initial redacted report](evidence/membership/gitleaks-initial.json)
and [failed scan record](evidence/membership/commands-initial.json) remain available.

## Remaining gates and handoff

This is an existing-member administrative API, not a complete access lifecycle.
Invitations, acceptance identity binding, expiry/replay, re-enrollment, tenant
creation/deletion, API-key hashing/rotation/revocation and full OIDC/PKCE remain
open. UI, MCP/voice, worker/assessments, billing, OTel, distributed rate limits,
TLS, audit retention/erasure, container/recovery/performance and full research are
also open. Docker is absent locally. No paid resources, shared accounts or
production deployment were created. The new NOLOGIN database role existed only
inside disposable fixtures. MIT licensing is unchanged.

Foundation CI passed at the prior e530e90 checkpoint. The new exact-commit CI result
is checked after pushing; local evidence alone does not claim remote CI success.
