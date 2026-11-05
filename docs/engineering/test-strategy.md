# Verification strategy

Unit and Rust integration tests exercise the domain's actual public interfaces.
Authentication and membership doubles are explicitly fixtures, not real OIDC or
API-key evidence. Every state/event combination and every role/action pair is
checked against independent expected tables; no network is used for scope tests.
Boundary inputs cover URL normalization, identity collisions, key restriction,
revocation/expiry, invalid persisted state, budget overflow and bounded headers.

The SQL suite runs an actual PostgreSQL server under a temporary Unix socket.
It tests the bootstrap migration with a runtime role that neither owns tables nor
bypasses RLS. Two tenants intentionally reuse names and IDs. Assertions exercise
forged context, issuer collisions, missing/expired/revoked membership, viewer
and cross-tenant writes, context reset, rollback, compare-and-set preconditions
and acknowledged data after restart. This is storage integration, not a Rust
database-adapter or end-to-end application test. PostgreSQL-version CI evidence
and production backup/restore/concurrency checks remain separate gates.

`make test` now runs all workspace tests with isolated PostgreSQL and ephemeral
RSA keys. These are signed access-token resource-server tests, not full external
OIDC sign-in tests. Project transport and service tests verify atomic audit,
concurrent updates, idempotency, revoked membership and real loopback HTTP. The
actual API process is exercised for invalid config, serving and graceful shutdown.

`make coverage` uses nightly-2026-11-04 only for branch instrumentation, merging
all test and child-binary counters. Production stays on 1.99.1. It checks every
source file and separate component denominators. The API lib.rs module index has
no executable statements; a source validator enforces this exemption. No
executable first-party code is excluded. Raw line AND branch counts must exceed
90%; absent/empty reports fail. Latest reports exceed both thresholds locally.

CI installs the separately pinned instrumentation compiler and runs the same real
fixtures. No branch protection has been verified. No release workflow exists, and this pipeline cannot establish the
complete platform baseline. React coverage, real OIDC/MCP/voice/provider clients,
SAST/fuzzing, container scans/smoke, resilience/performance, accessibility, i18n,
backup/restore and full end-to-end journeys remain required in TASK-003b–012.

Membership tests use real signed tokens and nonsuperuser PostgreSQL operations.
They exercise the complete actor/target/desired-role policy matrix, permanent-owner
succession, concurrent self-demotion, self-revocation, revocation while actually
waiting on the tenant lock, revision conflicts, expiry, atomic audit failure and
raw SQL privilege/search-path attacks. New executable modules are automatically
inventoried by coverage. The audit test temporarily changes privileges only in
the isolated fixture and restores them before completing.

Invitation tests use the existing signed-token and real PostgreSQL fixtures.
Exercise duplicate IDs, active versus inactive recipients, wrong subject/issuer/
tenant, expiration, terminal replay, concurrent same/different-offer acceptance,
current sponsor role/revision/expiry, re-enrollment after revocation, audit rollback,
raw SQL privileges/isolation and localized HTTP failures. No mocked enrollment
is accepted as a provider-login test.

API-key tests use actual AWS-LC randomness, SHA-256 and constant-time verification
against PostgreSQL. Cover malformed/forged/unknown credentials, verification before
membership reads, tenant/scope/current-revision restrictions, credential and member
expiry/revocation, concurrent rotation, audit rollback, disclosure boundaries,
JWKS outage independence and raw SQL privileges/bounds/isolation. Current key
material is never printed into committed logs.

Discovery tests use an owned HTTPS fault fixture with a fresh private CA/server
certificate and two generated signing keys. Python only provides test transport;
all product authentication/network behavior remains Rust. Tests check real TLS
trust/hostname rejection, issuer/origin restrictions, no redirects, body/header/
read/total time limits, key replacement/removal, concurrency, cancellation cooldown,
failed refresh without lifetime extension, recovery, current DB membership and the
actual API binary in both launch modes. No real customer identity/credential is
used. This is deliberately not claimed as a maintained IdP/browser login test.
