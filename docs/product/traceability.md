# Requirement traceability

Each row remains open until the complete acceptance is verified. Partial helper
coverage does not pass a platform capability. Owner of this checkpoint: backend
implementer; all future runtime/component owners are the implementation agent.

| Requirements | Source | Component/task | Current evidence | Remaining acceptance |
| --- | --- | --- | --- | --- |
| REQ-001, PLAT-TENANT | PAGE-044, POLICY | COMP-PERSISTENCE, TASK-003a/003b | Real SQL two-tenant project isolation/restart tests | Rust service, create/list/change endpoints, lifecycle/invites/export/delete, UI |
| PLAT-RBAC | POLICY | COMP-DOMAIN/PERSISTENCE, TASK-003a | All role/action matrix; key scope reduction; current membership/RLS tests | Every transport, owner transfer/last-owner guards, concurrent management |
| PLAT-OAUTH | POLICY | Identity adapter, TASK-003b | Authentication-before-membership policy tested with fixtures only | Maintained OIDC verifier, real test-IdP login/logout, PKCE/rotation/outages |
| PLAT-KEYS/AUDIT | POLICY | Access/audit adapters, TASK-003b | Scope policy only | Secure key creation/hash/expiry/rotation/revocation, durable attributable audit |
| REQ-002 | PAGE-004/010/011 | COMP-DOMAIN, TASK-004 | Strict origin/path and special-IP/mixed-DNS policy tests | Ownership proof, expiration, pinned network transport, redirect/rebinding tests |
| REQ-003, PLAT-AGENTS | PAGE-024, POLICY | COMP-DOMAIN, TASK-004/005 | Complete lifecycle table; checked budget arithmetic | Durable jobs, leases/fencing, cancellation at dispatch, spending reservations |
| REQ-005/006/007 | PAGE-001/033; PAGE-007/034/035 provisional | COMP-DOMAIN, TASK-005 | Bounded header observations, honest endpoint ratio | Discovery, allowlisted checks, model adapters, independent reproduction |
| REQ-004/008/009 | PAGE-012/014/030/031 provisional | TASK-005/009/010 | Specification only | Test identities, retests and audience-safe JSON/HTML/PDF reports |
| REQ-010/011 | PAGE-027/047 provisional | TASK-009 | Specification only | Scheduling, deploy triggers, repository/ticket integrations |
| REQ-012/013 | PAGE-003/008/009/025/026/050 provisional; PAGE-052 partial | TASK-010 | Specification/backlog only | Android/iOS/network/AI/MCP target capabilities, private connector |
| PLAT-UX/I18N | POLICY | Frontend, TASK-006 | Proposed locales/layout only | React journeys, full en/pt-BR catalogs, RTL/a11y and real APIs |
| PLAT-MCP/VOICE | POLICY | TASK-007 | Architecture only | Real protocol/client and voice providers with shared authorization |
| PLAT-BILLING | POLICY | TASK-008 | Budget arithmetic only | Entitlements, meters, invoices/subscriptions, reconciliation/concurrency |
| PLAT-OTEL/CONTAINER | POLICY | TASK-011 | None | Traces/metrics/logs, images, full restart/recovery and provider failures |
| Strict >90% line/branch per component | POLICY | TASK-012 | Domain line report; branch gate blocked | Compatible pinned branch instrumentation; all future components included |

Current checks and hashes: [verification](../specker/runs/2026-11-04-karate/verification.md).

## TASK-003b1 update (supersedes initial status cells above)

REQ-001 now has real authenticated create/list/get/update project APIs and a Rust
PostgreSQL service. PLAT-RBAC is enforced there and in RLS. PLAT-OAUTH now has
signed access-token resource-server validation; full login/PKCE/live-provider
integration remains pending. PLAT-AUDIT now has atomic append-only project
mutation events with restricted readers; retention/tamper detection and other
event families remain pending. PLAT-I18N has localized API errors, not complete
UI/report/email catalogs. Per-component line/branch gates pass for both current
Rust components. See [current evidence](../specker/runs/2026-11-04-karate/projects-verification.md).

## TASK-003b2a update

PLAT-TENANT/RBAC/AUDIT now include an authorized existing-member directory,
role changes, revocation, last-permanent-owner safeguards under concurrency and
atomic membership events. Real DB/HTTP/domain tests cover escalation, stale
revisions, post-lock revocation, direct SQL privileges and audit rollback. UI,
invitations/re-enrollment, tenant lifecycle, API keys, complete OIDC and retention
remain open. Latest evidence belongs in the membership verification record; the
project checkpoint is retained as historical evidence rather than overwritten.

## TASK-003b2b update

PLAT-TENANT/RBAC/AUDIT includes identity-addressed invitation issue/list/revoke/
acceptance and revision-safe re-enrollment. Offers never grant ownership, require
current inviter authority, and are single-use under tenant locking. No mail
delivery or full OIDC claim is made. Tenant lifecycle, API keys, UI and remaining
platform gates stay open; see the invitation verification checkpoint.

## TASK-003b2c update

PLAT-KEYS now has real tenant-scoped random credentials, nonrecoverable verifiers,
limited delegation, expiry, revision-safe rotation/revocation and atomic audit.
Actual DB/HTTP tests cover data access and control-plane denial. Full OIDC, tenant
lifecycle, UI, rate coordination and the remaining platform gates stay open; see
the API-key verification checkpoint for current evidence.

TASK-003b2d1 advances PLAT-OAUTH with trusted HTTPS discovery and bounded JWKS
refresh. Implementation: discovery.rs, shared AccessVerifier and binary config.
Evidence: real TLS, signing-key rotation, concurrency, expiry and fault tests; strict
component coverage. PLAT-OAUTH is still partial: full PKCE/session and maintained
IdP integration remain open; no overall requirement is marked complete here.
