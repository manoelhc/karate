# Karate build run

- Mode: build; target: /Users/manoelhc/Documents/GitHub/manoelhc/karate.
- Method: https://github.com/manoelhc/specker at f71612bb25bfb1951b3d0df4abb3de9bf914e9d3.
- Reference: https://www.aikido.dev/attack/aipentest (research only; no assessment authorized).
- Product: independent MIT-licensed AI security-assessment SaaS; copyright Manoel Carvalho preserved.
- Current delivery base: 256852839805b5dcbb46a6ba5a61f1b5a1911b19. User requires direct commits/pushes to main without PRs. No production deployment or paid resources authorized.
- Current stage: **partial implementation**. TASK-003b1 project API, TASK-003b2a existing-member administration TASK-003b2b invitations and TASK-003b2c API keys and TASK-003b2d1 discovery/key refresh plus TASK-003b2d2 browser sessions and TASK-003b2e1 explicit provisioning/lifecycle and TASK-006a React access/recovery implemented; complete platform and reference parity still open.
- Toolchains: production Rust 1.99.1; separate coverage-only nightly-2026-11-04 (1.101.0-nightly, 708c94ef0), matching LLVM tools. PostgreSQL 14.20 local fixture; production major version not selected. Docker absent; existing Podman machine supports the pinned Dex fixture.
- Capacity was constrained (under 2 GiB); run rebuildable Rust/frontend artifacts sequentially. Node/npm were installed only in the workspace; see web verification for cleanup.
- Owner: single implementer using frontend-engineer plus existing backend workflow; Sites guidance routes this repository to local-only work. No sub-agents, cloud/account provisioning or provider credentials.

## Latest continuation (2026-11-06): React workspace access and recovery

TASK-006a implements the localized React session/tenant workspace, server-derived
owner control visibility, safe browser callback failure redirect, bounded API
requests and session-scoped cache cleanup. 18 frontend tests and the actual local
HTTPS browser/Rust/PostgreSQL/Dex journey pass, with axe, keyboard and responsive
checks. See [web verification](web-verification.md) for evidence and remaining gates.
Rust 1.99.1 retains all authorization/domain logic. No deployment or account action.
Prior tenant checkpoint 2568528 passed exact-commit CI; this checkpoint's exact CI
is checked after its direct push. First CI at 7728fae passed Rust/PostgreSQL/coverage
but failed a zero LCP sample before keyboard input. The test now waits for an actual
paint entry; thresholds stay unchanged and the follow-up requires the full CI
browser journey. See web verification. The human-provider connector gate remains open.

## Latest continuation (2026-11-06): explicit tenant provisioning and lifecycle

TASK-003b2e1 adds operator-issued identity-bound provisioning grants, atomic
permanent owner/bootstrap/audit, interactive metadata and owner-only rename/
suspend/resume. Active-tenant guards serialize ordinary work with suspension,
with current member checks after lock waits. Suspended owners can sign in and
recover; data/control-plane operations stay blocked. Grants are not recycled.
43 Rust tests, real SQL lifecycle/locking/RLS and strict component coverage pass;
see [tenant verification](tenants-verification.md). Migration 0007 runs only in
private fixtures. No dependency, MIT license or shared infrastructure change.

## Prior implemented slice (2026-11-05): discovery/key refresh

TASK-003b2d1 adds opt-in trusted HTTPS discovery and automatic bounded JWKS refresh.
Metadata cannot select a new issuer or origin. TLS certificates/hostnames, response
sizes, deadlines and headers are checked; redirects/proxies/retries are disabled.
Concurrent callers use only a still-valid prior snapshot; failed/cancelled refresh
cannot extend trust. Current membership and API-key behavior remain unchanged.
36 tests, real TLS/DB/process fixtures, strict component coverage and local checks
pass; see [discovery verification](discovery-verification.md). Previous 0d577b4 CI
passed. New exact-commit CI is checked after push. No schema/shared account action.

## Prior continuation: browser code/PKCE and sessions

TASK-003b2d2 implements a maintained openid 0.25.0 adapter with bounded transport,
browser-bound one-use state, verified ID claims and current membership, then opaque
HttpOnly tenant sessions with idle/absolute expiry and atomic local revocation.
Real PostgreSQL, signed fault fixtures and maintained Dex protocol tests run locally.
See [sessions verification](sessions-verification.md) for final gate evidence.
No human-provider/React or IdP SSO logout claim is made. Migration 0006 runs only
in disposable clusters; the existing Podman VM hosts disposable Dex containers.


## Prior API-key slice (2026-11-05)

TASK-003b2c adds real scoped API-key creation/list/rotation/revocation. Credentials
contain 32 random bytes; only an AWS-LC SHA-256 verifier is stored. Verification
precedes current membership lookup. Tenant/scopes/expiry/revocation/member revision
bound authority; no key can access the control plane. Rotation is creator-only,
revision-checked and replaces the verifier atomically with audit. Plaintext is
returned only on creation/rotation. Owner can revoke another creator's key.
Migration 0005 runs only in disposable fixtures. The direct AWS-LC dependency
reuses the existing locked version. See [key verification](keys-verification.md).
Prior invitation checkpoint e9ce5d8 passed remote CI.

## Prior invitation slice (2026-11-05)

TASK-003b2b adds invitation issue/list/revoke and identity-bound single-use
acceptance. Offers expire within seven days, grant admin/member/viewer only, and
bind the inviter/recipient revisions. Re-enrollment preserves the member ID and
cannot replay older authority after revocation. Tenant locking and READ COMMITTED
protect acceptance races; audit failure rolls back both membership and offer.
The locator alone never authorizes acceptance, and no email/message is sent.
Migration 0004 uses the existing NOLOGIN capability boundary in disposable fixtures.
See [invitation verification](invitations-verification.md) for current evidence.
Previous membership checkpoint e83ee45 passed remote CI.

## Prior membership slice

TASK-003b2a adds a bounded owner/admin membership directory, granular role changes,
revisioned revocation and attributable atomic membership events. Admins can manage
only member/viewer roles; owners can manage all. Concurrent removal preserves a
permanent owner. The service rechecks current actor authority after obtaining the
tenant lock. OWNER_REQUIRED provides a localized recovery instruction. Existing
membership RLS remains own-row only; narrow NOLOGIN-role definer capabilities
supply the directory and guarded updates. Runtime membership in that role is
refused at startup. Migration 0003 has only run in disposable local databases.

The latest evidence is [membership verification](membership-verification.md).
Project checkpoint evidence remains historical; foundation CI passed at e530e90.
Full 003b2 remains unfinished: tenant lifecycle and full OIDC.

## Prior implemented foundation

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

The Rust API now provides browser code/PKCE/session endpoints, but only for
existing members. Human/provider connector integration, provider logout,
tenant deletion/retention and full platform gates remain open. The synthetic Dex connector
is real protocol evidence, not a human sign-in journey. DB runtime is Unix-socket
only; non-loopback API binds and unencrypted DB TCP are rejected. React access/recovery is now built; no worker or assessment executor is built.

## Exact next actions

1. TASK-003b2e2: define and implement tenant retention/deletion and operator-support
   boundaries. React access/recovery is implemented. Human IdP connector integration,
   SSO/backchannel logout, manual screen-reader and field performance remain open.
2. TASK-004a after its access prerequisites: expiring target ownership, immutable
   scope, atomic budget/job publication, lease fencing and per-request authority/
   cancellation/DNS pinning. Only owned HTTP fixtures are authorized.
3. TASK-001 alongside independent build work: finish documentation reading and
   the mobile/iOS/report frontier; reconcile provisional PRD/business evidence.
4. Continue full baseline: complete React journeys/localization/a11y, MCP/voice/providers, billing,
   orchestration, OTel, container/recovery/security/performance gates. None waived.

No renewed approval is required for local work or direct-to-main delivery. Paid
resources, production provisioning/deployment and outreach remain unauthorized.
