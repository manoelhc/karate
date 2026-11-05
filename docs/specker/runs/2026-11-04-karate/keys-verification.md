# API-key verification — TASK-003b2c

Owner: backend implementer; PLAT-KEYS/TENANT/RBAC/AUDIT. Base commit:
`e9ce5d8da760c60f85f1991e58578762bda5597e`. Checked 2026-11-05 on aarch64 macOS,
production Rust 1.99.1, PostgreSQL 14.20 and Python 3.14.6. Migration 0005 followed
0001–0004 only in disposable private Unix-socket fixtures. No shared migration.

## Executed gates

| Check | Result | Evidence |
| --- | --- | --- |
| Format, strict Clippy, workspace tests, build | Pass: 30 named tests, zero failed/skipped | [log](evidence/keys/rust-checks.log) |
| Dependency licenses/bans/sources | Pass | [log](evidence/keys/rust-checks.log) |
| PostgreSQL isolation/rollback/restart suite | Pass | [log](evidence/keys/postgres.log) |
| >90% line AND branch per runtime/shared package | Pass | [summary](evidence/keys/coverage-summary.json), [LLVM report](evidence/keys/coverage-llvm.json) |
| Refreshed RustSec audit | Pass; zero vulnerabilities/warnings, no ignores | [report](evidence/keys/audit.json) |
| Secret scan | Pass after source-hash report clarification | [log](evidence/keys/secrets.log), [report](evidence/keys/gitleaks.json) |

[Command records](evidence/keys/commands.json) contain timestamps and exit codes.
[Snapshot](evidence/keys/snapshot.json) hashes source/docs/config/lock files,
excluding evidence itself. Coverage independently hashes all production Rust source
and Cargo.lock. Both use separate path/sha256 records to distinguish source hashes
from credentials. MIT license unchanged; the only lock change is the existing
AWS-LC package becoming a direct API dependency. No resolved versions changed.

| Component | Lines | Branches |
| --- | --- | --- |
| karate-core | 382/382 (100%) | 234/240 (97.5%) |
| karate-api | 1244/1267 (98.185%) | 167/176 (94.886%) |

Instrumentation remains nightly-2026-11-04, rustc 1.101.0-nightly 708c94ef0, LLVM
23.1.5. Production remains Rust 1.99.1. The key module is part of the API runtime
and is included with every other executable product source and subprocess startup.
Only test/third-party code and the source-checked nonexecutable API module index
are excluded. Raw counts enforce >90% independently per component. SQL is exercised
against PostgreSQL and is not assigned a fabricated Rust coverage denominator.

## Security and lifecycle evidence

Tests use real AWS-LC random generation, SHA-256 and constant-time verification.
They verify the stored digest matches the generated credential, metadata/event
responses omit secrets/verifiers, and invalid verifiers fail before membership
SELECT. Missing credential-store permission fails closed. Malformed formats,
unknown locators, forged secrets, foreign tenants and overbroad/duplicate/empty
scopes are denied. Keys cannot manage keys/memberships/invitations or accept offers.

Current membership role/revision, revocation and expiry reduce authority. Keys
remain invalid after role restoration until an interactive creator explicitly
rotates/rebinds them. Expired/revoked keys cannot rotate. Owner can revoke another
creator's key but cannot rotate it; admins see/manage only their own keys.
Independent key verification works during an expired JWT key snapshot without
weakening either credential path.

Concurrent rotations at one revision commit once. Old secrets fail immediately
on subsequent authentication. Audit failure rolls back create/rotate/revoke and
leaves the prior verifier/revision usable. Lost responses are not automatically
retried or used to recover a plaintext key. Direct runtime credential-table access
and event writes are denied; raw scope/duplicate/verifier-size/list/isolation
checks also pass. HTTP tests cover one-time disclosure, no-store, localized
errors, malformed requests, revisions and actual scoped project access.

The initial secret scan flagged the SHA-256 source hash next to keys.rs in the
coverage report. It was compared to the actual source bytes, then represented as
separate path/sha256 fields. Coverage was regenerated with unchanged measured
counts. No secret was found and no rule/path was suppressed. The
[initial redacted report](evidence/keys/gitleaks-initial.json) and
[scan log](evidence/keys/secrets-initial.log) are retained. All generated credentials
exist only in the isolated test process; no private key, customer capture or .env
is committed. Fixture cleanup removes clusters/signing keys; workspace cleanup is
recorded in [cleanup](evidence/keys/cleanup.log).

## Remaining gates

This is a verified local API-key lifecycle, not release readiness. Full OIDC/PKCE,
bounded discovery/JWKS refresh and session management, tenant lifecycle, UI,
assessment workers, MCP/voice, billing, distributed rate limits, TLS, audit
retention/erasure, OTel, containers, recovery/performance and complete research
remain open. Docker is absent. Existing admitted bounded requests can finish
after key revocation; future long-running work must reauthorize at its boundaries.
No paid resources, deployment or shared account changes were performed.

Prior e9ce5d8 passed remote CI. The new exact-commit CI run is checked after push;
local evidence alone does not claim remote CI success. Next: full OIDC login and
session lifecycle with maintained-provider evidence and honest prerequisite gates.
