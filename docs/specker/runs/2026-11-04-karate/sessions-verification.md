# Browser sessions verification — TASK-003b2d2

Base: e68bdbc0948505d2dfcd309601293217a31d82b8. Checked 2026-11-05 with
Rust 1.99.1, PostgreSQL 14.20, Python 3.14.6, OpenSSL 3 and the existing Podman
machine on arm64 macOS. [Component](../../../components/browser-sessions.md),
[ADR-0008](../../../adr/0008-browser-oidc-sessions.md),
[API contract](../../../contracts/projects-v1.md) and
[operations](../../../operations/project-api.md) describe supported behavior.

## Executed checks

| Check | Result | Evidence |
| --- | --- | --- |
| Format, strict Clippy, tests, build, licenses/bans/sources | Pass: 40 named Rust tests, none skipped | [log](evidence/sessions/rust-checks.log) |
| Real PostgreSQL isolation/restart, migrations 0001–0006 | Pass | [log](evidence/sessions/postgres.log) |
| Maintained Dex protocol fixture | Pass | [log](evidence/sessions/oidc.log) |
| Strict per-component line and branch coverage | Pass | [summary](evidence/sessions/coverage-summary.json), [LLVM](evidence/sessions/coverage-llvm.json) |
| RustSec refreshed audit | Pass: 253 packages, no vulnerability/warning/ignore | [report](evidence/sessions/audit.json) |
| Gitleaks | Pass; zero findings after research cleanup | [report](evidence/sessions/gitleaks.json), [log](evidence/sessions/secrets.log) |
| Artifacts/source hashes and workspace cleanup | Pass | [checks](evidence/sessions/artifact-checks.log), [snapshot](evidence/sessions/snapshot.json), [cleanup](evidence/sessions/cleanup.log) |

[Commands](evidence/sessions/commands.json) record timestamps/exit codes. Initial
commands/logs remain separate from the final migration regression rerun. Coverage
hashes every product source and the lock; the final snapshot hashes source,
configuration, docs and tests excluding evidence. No existing resolved dependency
version was removed/upgraded; 34 packages were added. Product MIT text unchanged.

| Component | Lines | Branches |
| --- | --- | --- |
| karate-core | 397/397 (100%) | 242/248 (97.581%) |
| karate-api | 2027/2069 (97.970%) | 342/374 (91.444%) |

Both components independently exceed 90% line AND branch coverage. The denominator includes
browser.rs and every other executable API/core source, including real process
startup. Test/dependency/compiler sources and the checked module-only lib.rs are
the only existing exclusions. SQL is exercised against real PostgreSQL rather
than assigned a fabricated Rust coverage denominator. Nightly-2026-11-04 is used
only for instrumentation; product compilation remains Rust 1.99.1.

## Behavioral evidence and corrections

The isolated fault provider signs real RS256 ID tokens and checks the posted
S256 verifier. Tests reject wrong browser binding, expired/replayed state, bad
issuer/audience/authorized-party/nonce/subject/time/hash/header/key claims,
malformed metadata, unsupported algorithms/PKCE and cross-origin endpoints.
Matching state is atomically consumed before exchange, including cancellation or
provider denial; a concurrent replay has exactly one successful claimant.
The shared transport retains real TLS, response-size/read/total/header limits,
no redirects, no proxies and no retries, covered by existing discovery tests.

Session tests use real PostgreSQL and the shared service: missing/foreign/current
membership, captured revision, durable service recreation, unknown/forged secret,
idle/absolute expiry, current/all revocation and issuance/revocation audit rollback.
Runtime cannot directly read state/verifier tables, write audit or call the helper.
128-flow and 32-current-session capacities are exercised. Full obsolete member
revisions cannot block fresh sign-in or crowd active sessions out of revoke-all.
That review finding was fixed by including current membership revision in the
index, count and bounded revocation query, then adding a 32-obsolete-session case.

HTTP router tests verify secure cookie attributes, fixed redirect, no-referrer,
metadata without credentials, exact-origin/header CSRF, duplicate/mixed cookies
and bearer rejection, localized errors and cookie clearing. Existing real socket
and process tests remain in the suite; they cover launch configuration and graceful
shutdown. No React/browser automation or human login journey is claimed here.

Digest-pinned Dex v2.46.1 runs as a restricted local container with its maintained
upstream mockCallback synthetic identity. Real TLS discovery, authorization code,
PKCE enforcement, nonce, RS256 signature, access-token hash, code replay and local
session revocation pass. The fixture follows at most eight same-provider redirects
and stops before the configured Karate callback; it never visits a third-party
reference. Membership is explicitly bootstrapped in the private fixture only.

An initial TLS fixture failure was traced to a missing explicit CA key-usage
extension under Python's strict certificate verification. The fixture was corrected;
TLS verification remains enabled and no TLS version downgrade remains. Repeated
Dex tests then exposed a one-second provider issued-at lead from the VM/host clocks.
[Sanitized temporary diagnostics](evidence/sessions/clock-diagnostic.log) show the
failure. Production diagnostics were removed; a maximum five-second issued-at
lead is allowed, with strict not-before/expiry and API-clock session cap. Positive
bounded-skew and negative future-token cases were added. Twenty repeated real
Dex runs passed after that correction ([regression log](evidence/sessions/clock-regression.log)).
No provider token or cookie was printed or retained in evidence.

## Infrastructure actions and remaining gates

Only local owned fixture state changed: migration 0006 in disposable clusters;
official Dex image in the existing local cache; uniquely named containers removed
after each fixture. Private CA/signing keys, PKCE/nonce/configuration and test DBs
are deleted by cleanup. No VM provisioning, system trust change, shared account,
external client registration, production migration, paid resource or deployment.

This closes the local backend code/PKCE/session slice. It does not close full
OIDC/product readiness: provider SSO/backchannel logout, real human/upstream
connector, React same-origin integration, invitation onboarding and tenant
provisioning/lifecycle remain open. Only existing members can use browser login;
invitation acceptance requires the addressed access JWT. Retention/deletion,
distributed abuse controls, HTTP/DB deployment TLS, OTel, full Karate containers,
recovery/performance/security/a11y, assessment workers, MCP/voice/billing and
research parity remain mandatory gates. Dex container verification supplies
provider evidence only. Podman is available; claiming no container runtime would
be inaccurate, even though Docker is absent locally.

The previous e68bdbc exact-commit CI passed. New exact-commit CI is checked after
the direct push to main; local checks alone do not claim that result. No PR or
production deployment is part of this delivery. Next: tenant provisioning/lifecycle
and then the React sign-in/session journey against this API.

The first working-tree secret scan matched two public upstream sample token
strings in the ignored, temporary openid source evaluation directory. That owned
research directory was removed during cleanup, with no scanner exception. The
redacted [initial report](evidence/sessions/gitleaks-initial.json) is retained;
these were upstream examples, not customer/provider credentials. Final scan
results are recorded separately.
