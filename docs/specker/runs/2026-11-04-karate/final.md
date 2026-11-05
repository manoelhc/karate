# Continuation handoff

State: **partial implementation**. TASK-003b2d1 adds opt-in trusted HTTPS discovery
and bounded automatic JWKS refresh to the Rust API. Exact issuer/same-origin keys,
TLS, response/time/header limits and no redirects/proxies constrain retrieval.
Single-producer replacement and cooldown preserve original-expiry enforcement;
current tenant membership is never cached. File-mode configuration remains usable.

All 36 Rust tests pass, including real HTTPS/signatures, fault/time limits,
rotation/concurrency/cancellation, current PostgreSQL membership and actual binary
startup/shutdown. Core: 382/382 lines, 234/240 branches. API: 1448/1472 lines,
228/238 branches. Both independently exceed 90%. License/source/bans, refreshed
RustSec and secrets checks pass. See [discovery verification](discovery-verification.md)
for exact command records, source hashes and limitations.

Run `make lint test build licenses test-db coverage`, `make audit`, `make secrets`.
Production stays Rust 1.99.1; coverage uses nightly-2026-11-04. No schema migration
was added. Configuration, overlap/recovery and supported-provider restrictions are
in the [runbook](../../../operations/project-api.md). Temporary fixture state and
workspace build artifacts are removed at handoff.

Next: TASK-003b2d2 audited maintained OIDC client/provider, authorization-code/PKCE
and session lifecycle. Client selection is open; common candidates still pull an
unresolved RSA advisory, while Rauthy's optional algorithm support needs review.
No hand-written browser auth was substituted. Maintained IdP, full platform,
container/research and deployment gates remain open. Continue direct commits and
pushes to main without PRs. Resume [run.md](run.md).
