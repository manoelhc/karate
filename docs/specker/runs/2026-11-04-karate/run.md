# Karate build run

- Mode: build; target: /Users/manoelhc/Documents/GitHub/manoelhc/karate.
- Method: https://github.com/manoelhc/specker at f71612bb25bfb1951b3d0df4abb3de9bf914e9d3.
- Reference: https://www.aikido.dev/attack/aipentest (research only; no assessment authorized).
- Product: independent MIT-licensed AI security-assessment SaaS; copyright Manoel Carvalho preserved.
- Current delivery base: 972ae0cb741f6d1832898dc045db2c3131dc5b07. User requires direct commits/pushes to main without PRs. No production deployment or paid resources authorized.
- Current stage: **partial implementation**. TASK-003b1 project API implemented; complete platform and reference parity still open.
- Toolchains: production Rust 1.99.1; separate coverage-only nightly-2026-11-04 (1.101.0-nightly, 708c94ef0), matching LLVM tools. PostgreSQL 14.20 local fixture; production major version not selected. Docker absent.
- Capacity: disk recovered to several GiB during this continuation, permitting the isolated coverage toolchain. Recheck before large installs; workspace build outputs are removed at handoff.
- Owner: single backend implementer using backend-engineer workflow. No sub-agents, cloud/account provisioning or provider credentials.

## Latest implemented slice

`karate-api` is a runnable Rust resource server and shared project service:
RS256 at+jwt verification with configured issuer/audience/expiring public JWKS,
current database membership before domain policy, and bounded async PostgreSQL
queries. Tenant/project RLS is retained. POST is idempotent by tenant/project UUID;
PATCH uses optimistic revisions; list uses bounded UUID pagination. Migration
0002 records attributable create/update audit in the same transaction. Errors
are localized en/pt-BR with stable codes and no raw credential/provider output.

Real fixtures cover signed-token rejection, forged tenant/issuer access, viewer
restrictions, current revocation/expiry, duplicate creation, concurrent updates,
audit failure rollback, audit-reader isolation, loopback HTTP, binary startup
failure and graceful shutdown. No authentication bypass or network assessor exists.

Coverage now measures branches with a date-pinned instrumentation toolchain and
passes >90% line AND branch separately for core/API. Production compiler is
unchanged. The former missing-branch blocker is resolved for current components;
it does not pass the remaining platform gates. Latest exact counts, commands,
source hashes and test evidence live in [project verification](projects-verification.md).

An initial RustCrypto dependency selection failed RustSec checks; switching the
supported JWT backend to AWS-LC removed the affected packages with no advisory
suppression. License/source/duplicate checks use exact documented dependency seams.
No vulnerable initial graph was committed.

## Scope and evidence boundaries

Research remains partial: nine of 55 inventoried pages substantively read, with
an open frontier and uninspected media. Preserve provisional source IDs and newly
advertised iOS parity until detailed research resolves it. English/pt-BR and the
web/API SaaS buyer remain unvalidated market assumptions.

The access-token verifier is not complete OIDC login: browser PKCE, live provider
sign-in/logout, automatic JWKS discovery/rotation and session revocation remain
pending. The local issuer fixture generates real signatures but is not evidence
of a commercial IdP integration. DB runtime is Unix-socket only; non-loopback API
binds and unencrypted DB TCP are rejected. No frontend/worker/assessment is built.

## Exact next actions

1. TASK-003b2: design and implement audited tenant/membership administration with
   invitation expiry/replay and last-owner protections, service API-key creation/
   hashing/scopes/expiry/rotation/revocation, then real OIDC login/PKCE and bounded
   JWKS discovery/refresh with a maintained test IdP. Preserve shared policy and
   extend real integration and component coverage rather than adding a bypass.
2. TASK-004a after its access prerequisites: expiring target ownership, immutable
   scope, atomic budget/job publication, lease fencing and per-request authority/
   cancellation/DNS pinning. Only owned HTTP fixtures are authorized.
3. TASK-001 alongside independent build work: finish documentation reading and
   the mobile/iOS/report frontier; reconcile provisional PRD/business evidence.
4. Continue full baseline: React/localization/a11y, MCP/voice/providers, billing,
   orchestration, OTel, container/recovery/security/performance gates. None waived.

No renewed approval is required for local work or direct-to-main delivery. Paid
resources, production provisioning/deployment and outreach remain unauthorized.
