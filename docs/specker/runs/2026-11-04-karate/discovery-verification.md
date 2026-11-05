# Discovery verification — TASK-003b2d1

Owner: backend implementer; PLAT-OAUTH/TENANT/RBAC. Base commit:
`0d577b40673ffa7e80a59789412b981342f6e359`. Checked 2026-11-05 on aarch64 macOS,
Rust 1.99.1, PostgreSQL 14.20, Python 3.14.6 and OpenSSL 3. The PostgreSQL fixture
uses migrations 0001–0005 unchanged; no new or shared migration. The public
[component contract](../../../components/identity-refresh.md) and
[ADR](../../../adr/0007-bounded-provider-key-refresh.md) define supported behavior.

## Executed checks

| Check | Result | Evidence |
| --- | --- | --- |
| Format, strict Clippy, tests, build | Pass: 36 named Rust tests, no failed/skipped | [log](evidence/discovery/rust-checks.log) |
| License/bans/sources | Pass; narrow documented license/version seams | [log](evidence/discovery/rust-checks.log) |
| PostgreSQL isolation/restart | Pass | [log](evidence/discovery/postgres.log) |
| Strict component coverage | Pass | [summary](evidence/discovery/coverage-summary.json), [LLVM](evidence/discovery/coverage-llvm.json) |
| Refreshed RustSec audit | Pass; 219 packages, zero vulnerabilities/warnings, no ignores | [report](evidence/discovery/audit.json) |
| Gitleaks | Pass, no leaks | [log](evidence/discovery/secrets.log), [report](evidence/discovery/gitleaks.json) |
| Artifact/source checks | Pass | [log](evidence/discovery/artifact-checks.log), [snapshot](evidence/discovery/snapshot.json) |
| Workspace cleanup | Pass | [log](evidence/discovery/cleanup.log) |

[Command records](evidence/discovery/commands.json) contain timestamps and exit
codes. Snapshot hashes source/docs/config/lock files excluding evidence itself.
Coverage separately hashes production sources and the current lockfile. Existing
resolved package versions and MIT license text are preserved; the lock adds the
HTTP/TLS graph and feature edges. This is not an unchanged lockfile claim.

| Component | Lines | Branches |
| --- | --- | --- |
| karate-core | 382/382 (100%) | 234/240 (97.5%) |
| karate-api | 1448/1472 (98.370%) | 228/238 (95.798%) |

Coverage uses nightly-2026-11-04 with matching LLVM 23.1.5, separately from product
Rust 1.99.1. Discovery is part of the API runtime denominator. All executable
product Rust sources, including subprocess startup, are included; only test,
third-party/compiler code and the source-checked nonexecutable module index are
excluded. Python is solely an owned test fault server, not product logic. No new
coverage exclusion was added to meet the threshold.

## Behavioral evidence

The fixture generates a private CA, a localhost server certificate and two RSA
signing keys for each isolated run. Tests perform real HTTPS and verify actual
JWT signatures. They reject untrusted roots, hostname mismatch, malformed/oversized
CA input, invalid launch settings, conflicting modes, issuer mismatch, cross-origin
and non-HTTPS JWKS, redirects, non-JSON/compressed/oversized/truncated/invalid UTF-8
responses and invalid key sets. Fault cases exercise header count, read timeout,
per-document total timeout and whole-refresh deadline. The binary test sets
unusable ambient proxies yet successfully retrieves keys directly.

Concurrent requests use the still-valid snapshot while one producer fetches.
Successful replacement uses a distinct signing key and removes the old key ID.
Unknown key IDs do not cause extra retrieval. Invalid refresh preserves the prior
set only until its original deadline; it cannot extend trust. Cooldown bounds
repeated failures and allows recovery. Cancellation reserves the attempt delay
before I/O. Current DB membership, viewer restrictions, foreign tenants and
revocation are checked through the real shared service. Binary tests exercise
file/discovery modes, generic error redaction and graceful shutdown.

The HTTP library defers PEM validation; an initial test exposed this and the loader
was tightened to require exactly one parsed certificate before building its
client. Final tests and coverage ran against the corrected code. License checks
initially required explicit review of the public-root dataset and two upstream
major-version seams; the final graph passes with the redistribution text retained.
The initial secret scan matched the prose phrase describing fault/concurrency/expiry
tests in traceability.md. It contained no credential; the sentence was clarified
without scanner suppression. The [redacted initial finding](evidence/discovery/gitleaks-initial.json)
and [initial log](evidence/discovery/secrets-initial.log) are retained.
No vulnerability advisory is suppressed. Candidate OIDC clients were researched
but not installed or claimed integrated.

All ephemeral private keys, TLS configuration and fault-server processes are
removed with their fixture. No system trust store, shared account, customer record,
production environment or paid resource was changed. Error logs contain no raw
provider data or credential. Provider metadata is fetched only from owned local
fixtures during tests; public documentation/registry reads are passive research.

## Remaining gates

This is a verified discovery/refresh prerequisite, not full OIDC or release
readiness. Browser authorization-code/PKCE, maintained-client selection, maintained
IdP sign-in, sessions/logout/revocation, UI/tenant lifecycle, assessment workers,
MCP/voice, billing, distributed rate controls, full TLS deployment, retention,
OTel, containers, recovery/performance and research remain open. Docker is absent;
the owned HTTPS fault fixture does not substitute for maintained-provider or
container acceptance. Same-origin-only JWKS is an intentional provider restriction.

Previous 0d577b4 remote CI passed. The exact new commit's CI is checked after push;
local results alone do not claim remote CI success. Next: TASK-003b2d2 maintained
OIDC client/provider evaluation and complete PKCE/session implementation.
