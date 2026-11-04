# Project API verification — TASK-003b1

This record supersedes historical foundation measurements in
[verification.md](verification.md). Delivery base: `972ae0cb741f6d1832898dc045db2c3131dc5b07`.
Local checks ran on 2026-11-04 on aarch64 macOS, Rust 1.99.1 and isolated PostgreSQL
14.20. Migration 0002 followed 0001 in disposable databases only.
No production or shared database migration was performed.

## Executed checks

| Check | Result | Evidence |
| --- | --- | --- |
| Format, strict Clippy, workspace tests, build, licenses/bans/sources | Pass; 24 named Rust tests | [log](evidence/projects/rust-checks.log) |
| Real PostgreSQL isolation, revocation, rollback, restart, context reset | Pass | [log](evidence/projects/postgres.log) |
| Strict per-component line and branch coverage | Pass | [log](evidence/projects/coverage.log), [raw summary](evidence/projects/coverage-summary.json) |
| RustSec locked dependency graph | Pass; no findings or warnings | [report](evidence/projects/audit.json) |

[Command records](evidence/projects/commands.json) contain timestamps and exit
codes. [Snapshot](evidence/projects/snapshot.json) identifies source/docs/config
files by SHA-256, excluding evidence itself to avoid recursive hashes. Coverage
records source and lockfile hashes independently.

| Component | Lines | Branches |
| --- | --- | --- |
| karate-core | 304/304 (100%) | 194/200 (97%) |
| karate-api | 509/523 (97.323%) | 107/110 (97.273%) |

Instrumentation uses `nightly-2026-11-04`, rustc 1.101.0-nightly `708c94ef0`, LLVM
23.1.5, with actual branch counters. Production remains Rust 1.99.1. All production
source files and the API executable are inventoried. Only test/third-party code
and the nonexecutable API module index are excluded; a source check rejects added
logic in that index. Percentages use unrounded raw counts, separately per component.
FIND-006 (unmeasured branches) is resolved for these components.

## Behavior and security evidence

Ephemeral RSA keys sign real fixture JWTs. Integration checks reject forged,
expired, wrong issuer/audience/algorithm, missing-claim and unknown-key tokens and
expired/invalid JWKS configuration. Authentication precedes request DB access;
the runtime refuses superuser, BYPASSRLS, table-owner and TCP connection settings.
No bypass, token-directed key fetch, arbitrary shell executor or assessor is added.

Real service/HTTP tests cover two tenants, current membership revocation/expiry,
viewer write denial, malformed inputs, pagination, idempotency and conflicting
revisions. Mutation audit insertion failure rolls back the project mutation.
Direct nonsuperuser SQL checks deny unscoped/viewer audit reads, actor spoofing and
audit update/delete. Process tests exercise startup failure/redaction, loopback HTTP
and graceful SIGINT shutdown. Locale tests check error parity and negotiation.

Initial RustCrypto dependency selection failed audit (RUSTSEC-2026-0387 and
RUSTSEC-2023-0071); the supported AWS-LC backend removes those packages. No advisory
ignore was added, and the failing graph was never committed. License/source checks
pass with compatible documented licenses and narrowly pinned duplicate allowances.

## Remaining gates

Signed fixtures are not evidence of complete OIDC or commercial-provider
integration. PKCE/login/logout, automated JWKS refresh, member/key lifecycle, TLS,
global rate limits, OTel, containers, recovery/performance gates, UI, worker,
assessment execution, MCP, voice, billing and the complete platform baseline remain
open. Docker is absent locally. Full reference research remains incomplete.
No deployment/release claim follows from component verification. CI status is
recorded separately after publication; local results alone do not claim remote CI.
