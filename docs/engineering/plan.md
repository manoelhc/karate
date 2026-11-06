# Karate implementation plan

The full Specker platform baseline remains mandatory. Status starts pending; a document is not implementation evidence.

| Task | Requirements | Dependency | Outcome and verification |
| --- | --- | --- | --- |
| TASK-001 | Research | None | Read indexed pentest docs; inventory media/authenticated gaps and support frontier. |
| TASK-002 | Business/architecture | 001 | Sourced business hypothesis, PRD, ADRs, component specs and traceability. |
| TASK-003 | PLAT-TENANT/RBAC/OAUTH/KEYS/AUDIT | 002 | Tenant-safe persistence, OIDC, key lifecycle and consistent authority; negative API tests. |
| TASK-004 | REQ-001/002/003 | 003 | Create project, verify target, queue/cancel assessment; owned fixture end-to-end. |
| TASK-005 | REQ-005/006/007/008 | 004 | Discovery, bounded check execution, independent verification and retesting with durable evidence. |
| TASK-006 | PLAT-UX/I18N | 003/004 | Accessible localized React workspace wired to real APIs, complete states and integration tests. |
| TASK-007 | PLAT-MCP/VOICE | 003/005 | Real MCP tools and voice read/confirmed mutation through shared policies. |
| TASK-008 | PLAT-BILLING/AGENTS | 004 | Concurrency/spending reservations, usage ledger, invoice/entitlement lifecycle and provider integration. |
| TASK-009 | REQ-009/010/011 | 005 | Audience-safe reports, scheduling/deploy triggers, source context and reviewable remediation integration. |
| TASK-010 | REQ-004/012/013 | 005 | Isolated browser authentication, private connector, mobile/network/AI-specific assessment parity with evaluated fixtures. |
| TASK-011 | PLAT-OTEL/CONTAINER | 003–010 | Trace propagation, images/compose, operating runbooks, recovery drills. |
| TASK-012 | All | All | >90% line/branch per component, security/a11y/performance review, real provider/container gates and final report. |

No task is verified by being listed here. Each implementation checkpoint records actual changed files, checks and blockers. Exact commands belong in the Makefile and verification report after tool support is established. Do not silently replace TASK-010 with a mock and claim parity.

## Current execution slices

TASK-001/002 remain partial: the recovered research ledger distinguishes 9 read
pages from 46 discovered pages. Critical policy decisions used read sources and
explicit user policy, allowing the independent foundation work below. Complete
plan readiness has not been claimed; runtime-specific component docs/contracts
are prerequisites to their coding slices.

| Slice | Owner / size | Dependencies and files | Acceptance / command | Status and rollback |
| --- | --- | --- | --- | --- |
| TASK-003a | Backend implementer / medium | Existing architecture, POLICY, PAGE-004/010/011/024/044; crates/karate-core, migration 0001, SQL fixtures | Explicit role/lifecycle matrix, strict scope, real two-tenant persistence and restart; make lint test build licenses test-db | Implemented foundation with recorded local checks; full task/coverage remain open. Additive schema used only in disposable fixture; source changes reversible. |
| TASK-003b | Backend implementer / large | 003a; identity/persistence/API component contracts | Signed access-token verification, async current membership, paginated project API, revision/idempotency and atomic audit | **003b1 implemented; remote CI passed at e530e90.** 003b2a adds existing membership administration; 003b2b adds identity-bound invitations/re-enrollment. 003b2c adds scoped key lifecycle; full OIDC login/refresh, tenant lifecycle and audit retention remain open. |
| TASK-003b2a | Backend implementer / medium | 003b1; membership component; migration 0003; core/API policy and real fixtures | Bounded directory, granular roles, revisioned revocation, serialized permanent-owner safeguards, atomic audit; make lint test build licenses test-db coverage | Local checks pass: 26 tests and >90% component line/branch coverage; exact-commit remote CI checked after push. Complete 003b2 remains open. Additive local migration only; no shared environment action. |
| TASK-003b2b | Backend implementer / medium | 003b2a; invitation component; migration 0004; shared Rust service and domain | Identity-bound expiry/replay, current inviter authority, revision-safe re-enrollment, atomic audit and real DB/HTTP tests | Implemented with 28 passing tests and >90% line/branch coverage per component; no shared migration or message delivery. Complete access/platform baseline stays open. |
| TASK-003b2c | Backend implementer / medium | 003b2a/b; API-key component; migration 0005; shared credential/policy paths | Random keys, hashed verifiers, scoped access, expiry, current membership revision, atomic rotation/revoke/audit; real DB/HTTP and component coverage | Implemented with 30 tests and strict coverage locally; no deployment. Complete platform gates remain open. |
| TASK-003b2d1 | Backend implementer / medium | 003b1/c; identity-refresh component and ADR-0007; reqwest/rustls | Trusted HTTPS discovery, bounded single-producer key rotation, original-expiry enforcement, real TLS/signature/process tests; make lint test build licenses test-db coverage | Implemented with 36 passing tests and >90% line/branch coverage per component. Full PKCE/session and maintained-provider gates remain open. No schema/shared environment change. |
| TASK-004a | Backend implementer / large | 003b plus target-verification/worker contracts | Expiring ownership proof, immutable scope, atomic budget+job publication, lease fencing, per-request cancellation/authority and pinned DNS; owned HTTP fixture | Pending. No third-party network assessment. |
| TASK-012a | Quality owner / medium | Current Rust components | >90% line and branch per component; make coverage | Pass locally with nightly-2026-11-04 instrumentation; production remains 1.99.1. Full platform gates still open. |

Backend checklist: model/internal contract and initial threat review complete for
003a; no shared/production migration approval applies to disposable fixtures;
lint/tests/build/license/audit checks recorded. Foundation CI passed at 0d577b4; the current continuation must pass again.
Release controls and the full compliance/access lifecycle remain open. Delivery uses direct commits and pushes to main, without PRs, per the user's
2026-11-04 instruction. Publication does not change the verification gate status.

Migration strategy: initial schema is additive; future updates use expand/backfill/verify/contract with snapshots before destructive changes. Production provisioning, billing activation and deployment require their own authority. No production environment has been selected.

TASK-003b2d2 implementation: browser-sessions component, openid 0.25.0 bounded adapter,
additive local session migration, PKCE/nonce/state/browser binding, durable
session logout/revocation, current membership and maintained Dex verification.
Acceptance: make lint test build licenses test-db coverage; make test-oidc;
audit/secrets and exact-commit CI. No shared infrastructure or migration action.

Next access slice TASK-003b2e: explicit tenant provisioning/lifecycle and owner
bootstrap, with deletion/retention boundaries; define its component contract first.
TASK-006a can then wire the real browser login/session API to the React shell,
including localized recovery, accessibility and a real browser/provider journey.
The synthetic Dex connector does not close that human integration gate.

TASK-003b2d2 final local verification: 40 named tests; core 397/397 lines,
242/248 branches; API 2027/2069 lines, 342/374 branches. Maintained Dex code/PKCE
and local session lifecycle pass. No skipped prerequisite is counted as complete.
See the sessions verification record; exact-commit CI is checked after push.

Implemented TASK-003b2e1: tenant component contract, identity-addressed operator grants,
atomic tenant/permanent-owner/audit bootstrap, owner-only metadata and suspension/
resume with serialized current-authority checks. No automatic owner or destructive
delete. Run complete backend checks and strict coverage; maintain full erasure,
React, provider and platform gates as pending. Single implementer; no delegation.

Backend checklist for this slice: [x] model/contract and impact scan; [x] identity/
retention and threat review; [x] local-only migration boundary (no shared action);
[x] implementation/test/lint/build; [ ] exact-commit CI. Bounded state, no new
libraries, no cache and no external account/customer-data action are planned.

TASK-003b2e1 final local checks: 43 tests, real SQL lifecycle/RLS and lock ordering;
core 462/462 lines and 266/272 branches, API 2262/2308 lines and 346/378 branches.
Strict component thresholds pass. Next TASK-006a React session/tenant recovery
journey; 003b2e2 erasure/retention remains explicit. See tenant verification.

Active TASK-006a: React session/tenant-recovery workspace with same-origin cookies,
server-derived control visibility, complete en/pt-BR states and bounded requests.
Small additive Rust metadata/browser-error changes support the real UI. Local
HTTPS Rust/PostgreSQL/Dex browser journey, UI coverage >90% lines/branches, a11y,
responsive screenshots, bundle/license/audit/static checks and exact CI required.
No frontend domain logic, hosting registration, deployment or paid resource.

TASK-006a implementation checkpoint: React/TypeScript CSR, 58 en/pt-BR keys,
validated bounded API adapter, session-bound mutation callbacks, complete recovery
states and native accessible confirmation. Additive Rust owner visibility and
safe HTML callback failure redirect. 18 frontend tests and real HTTPS browser
journey pass locally; bundle/audit/licenses/static checks recorded with final
Rust regression/coverage below. Manual screen-reader, human IdP, field CWV/INP,
full containers and platform gates remain open. No hosting/account action.

Frontend checklist: [x] design/states/data flow; [x] impact/license review;
[x] minimization/current authority/expiry; [x] CSP and failure hardening;
[x] local implementation and real browser verification; [ ] exact-commit CI.
Necessary dependencies follow the user's continuing implementation authorization.
No sub-agents. Direct main commits; no PR or deployment.

TASK-006a final local gate: 43 Rust tests, 18 frontend tests and the real browser
journey pass. Core 462/462 lines and 266/272 branches; API 2292/2339 and 351/388;
web 190/191 and 193/197. All strictly exceed 90% independently. Audits, licenses,
static checks, PostgreSQL isolation/restart, bundle, a11y and responsive checks pass.
See web verification for unresolved human-provider/screen-reader/field-performance,
retention and full platform gates. Exact CI is checked after the direct main push.
