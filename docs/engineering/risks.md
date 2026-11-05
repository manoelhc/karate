# Risk register

| ID | Risk | Impact | Mitigation / owner | State |
| --- | --- | --- | --- | --- |
| R-01 | Only 2.5 GiB free at start | Builds/images may not fit | Lean profiles and capacity check before installs; operator | Open |
| R-02 | Full product container stack unverified | Release readiness blocked | Podman discovered and Dex fixture verified; build/test full Karate stack separately; operator | Open |
| R-03 | No authenticated reference session | UI parity unverified | Human assistance request; preserve public-source provenance; researcher | Open |
| R-04 | No configured AI/STT/TTS provider credentials | Live integration gate blocked | Complete adapters and deterministic tests, then provider smoke; implementer | Open |
| R-05 | Autonomous scanner false claims/scope escape | Customer harm and misleading reports | Policy outside model; strict evidence separation; security owner | Open |
| R-06 | Mobile/network/advanced agent parity is substantial | Scope exceeds a scaffold | Preserve full backlog and checkpoints; never claim implemented from specs; implementer | Open |
| R-07 | Rust branch instrumentation compatibility | Must preserve measured component gates | nightly-2026-11-04 measures current components; quality owner | Resolved for current Rust components |
| R-08 | Hosted unit economics and market demand unvalidated | Unsustainable business | Design-partner experiments, cost meters, bounded spending; product owner | Open |

Continuation observations (2026-11-04): free disk fell below 1 GiB; Rust 1.99.1
and matching LLVM tools are now verified. The system nightly is from 2024 and
cannot compile this workspace. Measure capacity before installing a compatible
dated instrumentation toolchain. PostgreSQL 14.20 exists locally and passed
isolated tests, but does not select a production version. No Docker binary was
found. Evidence ledger recovery fixed lost source IDs, while reading coverage
and authenticated reference observation remain incomplete. Revisit these open
risks before their dependent slices or release; none is a waived requirement.

TASK-003b1 follow-up: disk recovered to several GiB. A separate pinned coverage
compiler now measures branches, resolving R-07 for current Rust components. Docker
remains unavailable. API JWT/JWKS snapshots expire and require operator rotation;
automatic provider refresh, full browser login, TLS deployment and token/session
revocation remain launch blockers. The initial cryptography backend failed audit
and was replaced before commit; the final dependency graph passes the audit.

2026-11-05 browser continuation: Docker remains absent but an existing Podman
machine was found and used for the disposable Dex fixture. No machine provisioning
or shared container changes were performed. Client selection, code/PKCE and durable
sessions are implemented. Human/provider connector integration, provider logout,
React, distributed login rate limits, retention and full Karate containers remain
open. See the current sessions verification for actual checks and source hashes.
