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
| TASK-003b | Backend implementer / large | 003a; write identity/persistence/API component contracts and choose maintained Rust libraries | Real test-IdP credential validation, async current membership lookup, tenant-safe project CRUD with pagination/revision, key lifecycle and durable audit; transport integration tests | **Next implementation slice.** No production auth bypass; fixtures use an isolated issuer/database. Migrate additively and test upgrade/rollback. |
| TASK-004a | Backend implementer / large | 003b plus target-verification/worker contracts | Expiring ownership proof, immutable scope, atomic budget+job publication, lease fencing, per-request cancellation/authority and pinned DNS; owned HTTP fixture | Pending. No third-party network assessment. |
| TASK-012a | Quality owner / medium | Current domain; sufficient capacity for compatible dated nightly instrumentation | >90% line and branch per component; make coverage | Line measured; branch gate blocked. Production compiler remains 1.99.1. |

Backend checklist: model/internal contract and initial threat review complete for
003a; no shared/production migration approval applies to disposable fixtures;
lint/tests/build/license/audit checks recorded. CI workflow authored but unrun;
remote checks, release controls and the full compliance/access lifecycle remain
open. Delivery uses direct commits and pushes to main, without PRs, per the user's
2026-11-04 instruction. Publication does not change the verification gate status.

Migration strategy: initial schema is additive; future updates use expand/backfill/verify/contract with snapshots before destructive changes. Production provisioning, billing activation and deployment require their own authority. No production environment has been selected.
