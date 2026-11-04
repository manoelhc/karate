# Karate product requirements

Status: initial specification, derived from public documentation; authenticated reference behavior remains unobserved. Sources are in the research ledger. Implementation and verification are tracked separately.

Karate is an open-source, independently implemented AI-assisted security-assessment SaaS. The initial buyer hypothesis is an engineering or application-security lead operating a web/API SaaS, with a staging environment and a need for repeatable evidence before releases. This is not validated demand.

## Customer outcome

Create a workspace and project, prove control of the target, configure the authorized scope and test identities, inspect a preflight and estimate, run a bounded assessment, review evidence and limitations, remediate, retest and export an audience-appropriate report. No clean report is proof that an application is secure.

## Requirements

| ID | Behavior and acceptance | Source |
| --- | --- | --- |
| REQ-001 | Projects group assessments and reusable configuration; tenant A cannot read tenant B's project. | PAGE-044; PLAT-TENANT |
| REQ-002 | Exact-origin target scope, blocked paths and separate visit-only dependencies; verified ownership expires and is rechecked before network work. Out-of-scope redirects and private/metadata destinations are rejected. | PAGE-004,010,011 |
| REQ-003 | Assessments use a durable draft → queued → preflight → discovering → testing → validating → completed lifecycle, with pause/resume/cancel/failure and explicit partial results. | PAGE-024,010 |
| REQ-004 | Isolated test identities per role/tenant, authentication preflight and safe secret storage. Refresh coordination must not require weakening the target's refresh rotation. | PAGE-012,014; proposed improvement |
| REQ-005 | Seed discovery from OpenAPI, URLs and provided source context. Show discovered/tested/skipped endpoints and reasons; unknown denominator stays unknown. | PAGE-007,032,033 |
| REQ-006 | Bounded Rust agents propose and execute approved security checks. Record evidence, reproducibility, check coverage, model cost, timeouts and cancellation. A model claim alone never creates a verified finding. | PAGE-034,035 |
| REQ-007 | Findings include severity, check/CWE, affected endpoint, evidence, reproduction and remediation; separate confirmed observations from unverified hypotheses. Independent recheck precedes verification. | PAGE-001,034 |
| REQ-008 | Retests preserve original evidence, compare against the deployed fix and distinguish fixed, persists, inconclusive and failed access. | PAGE-031 |
| REQ-009 | Technical, management and customer-safe reports derive from the same authorized data, redact secrets and disclose limitations. Export JSON/HTML/PDF; do not claim certification. | PAGE-030 |
| REQ-010 | Continuous assessments respond to explicit deployment events or schedules, deduplicate triggers and honor concurrency/spending limits. | PAGE-027 |
| REQ-011 | Source context, remediation proposals and ticket/PR integration use bounded repository access; no autonomous merge or publishing. | PAGE-007,047 |
| REQ-012 | Network, mobile (Android and iOS), AI-specific and MCP-target assessment capability remain in the reference-parity backlog with dedicated isolation and acceptance fixtures; unsupported types are visibly unavailable, never simulated. iOS is newly advertised on the reference entry page; detailed behavior is still unresearched. | PAGE-003,008,025,026; PAGE-052 partial |
| REQ-013 | Private-target connector binds authorized resources to one tenant with outbound authenticated communication and explicit network policy. | PAGE-009,050 |
| REQ-014 | API keys, OAuth2/OIDC, RBAC, audit, localization, MCP interface, voice, usage/billing, OTel, containers, component docs and strict coverage are mandatory. | All Specker PLAT IDs |

## Deliberate differences

Original Karate branding. Open-source operation and portable providers. Explicit check-by-endpoint evidence instead of a misleading universal safety score. Isolated credentials and refresh coordination. Spending budgets visible before and during runs. Accepted risk is an authenticated, audited decision; a target repository's prompt cannot suppress a finding automatically.

## Scope discipline

Build web/API assessment foundations first, then broader agent, mobile, network and provider integrations. Sequencing does not declare those features complete or waive them. Full parity is not currently implemented. The unrelated Aikido SCA, endpoint protection, firewall and general cloud posture product suites are outside this reference-product boundary.

## UX

Dark, restrained security workspace with warm coral accents, readable typography and clear status language. Navigation: Overview, Assessments, Findings, Projects, Reports, Settings. Primary CTA: New assessment. Empty states explain the next real action. No seeded fake vulnerability counts in the production interface. Locale defaults are en and pt-BR; keyboard and text alternatives for voice are mandatory.
