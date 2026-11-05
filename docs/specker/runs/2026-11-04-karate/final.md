# Continuation handoff

State: **partial implementation**. TASK-003b2c adds scoped API-key creation,
metadata listing, rotation and revocation. AWS-LC supplies secure random bytes,
SHA-256 verifiers and constant-time comparison. Authentication precedes current
membership resolution; tenant/scopes/member revision restrict authority. Keys
cannot invoke control-plane operations. Rotation and audit are atomic; plaintext
is returned once and never stored or logged.

All 30 Rust tests pass, including real DB/HTTP/process, cryptographic verification,
tenant/scope restrictions, expiry, concurrent rotation and audit rollback. Core:
382/382 lines, 234/240 branches. API: 1244/1267 lines, 167/176 branches. Both exceed
90% separately. Dependency/license/source, RustSec and secret checks pass. See
[key verification](keys-verification.md) for exact records and limitations.

Use `make lint test build licenses test-db coverage`, `make audit`, `make secrets`.
Production remains Rust 1.99.1; coverage uses nightly-2026-11-04. Migration 0005 has
only run in disposable fixtures. The [runbook](../../../operations/project-api.md)
describes custody, grants, lost-response recovery and explicit rotation.

Next: full OIDC authorization-code/PKCE, bounded discovery/JWKS refresh and session
lifecycle with maintained IdP verification. Tenant lifecycle, UI, assessment,
providers, containers and the full platform/research baseline remain open. No
shared migration, deployment, paid resource or third-party assessment. Continue
direct commits/pushes to main without PRs. Resume [run.md](run.md).
