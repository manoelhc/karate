# Continuation handoff

State: **partial implementation**. TASK-003b2d2 adds browser authorization-code/
PKCE login, browser-bound one-use state, strict ID-token verification, current
membership and durable opaque HttpOnly sessions. Current/all local revocation is
atomic with audit. Session expiry and membership revision are checked on every
request; CSRF checks protect cookie mutations. Existing access JWT/API-key paths
remain available. No provider token reaches browser JavaScript.

All 40 named Rust tests pass. Core: 397/397 lines, 242/248 branches. API:
2027/2069 lines, 342/374 branches. Both independently exceed 90%. Real PostgreSQL,
signed HTTPS fault fixtures, maintained digest-pinned Dex protocol tests and
process startup/shutdown are included. License/source/bans and refreshed RustSec
pass; final secrets/artifact/cleanup evidence is in [sessions verification](sessions-verification.md).

Run `make lint test build licenses test-db coverage`, `make test-oidc`, `make audit`
and `make secrets`. Production stays Rust 1.99.1; coverage uses nightly-2026-11-04.
Docker or a running Podman machine is required for Dex. Migration 0006 has only
run in disposable databases; no shared migration/deployment occurred. See the
[runbook](../../../operations/project-api.md) for configuration and boundaries.

Next: tenant provisioning/lifecycle and explicit owner bootstrap, then React
sign-in/session integration with localized recovery/a11y and a human-provider
journey. The Dex synthetic connector is maintained protocol evidence, not human
login or a production integration. Provider SSO/backchannel logout, retention,
distributed abuse controls, full Karate containers/recovery/telemetry and the
complete assessment/MCP/voice/billing/research baseline remain open. Continue
committing/pushing directly to main without PRs. Resume [run.md](run.md).
