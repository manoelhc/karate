# Historical foundation verification — partial implementation

This records the foundation delivered in 972ae0c. The current API/coverage
checkpoint is [project verification](projects-verification.md).

Date: 2026-11-04. Owner: backend implementer/reviewer, distinct review passes in
one agent. Base commit: d8ccc5764529c574c1052c090aacbc5704f912a9 on main. Evidence
applies to the captured [working-tree snapshot](evidence/snapshot.json), not to
the old commit alone. Migration version: 0001. Dependency inputs: Cargo.lock and
tools/uv.lock. No production deployment or remote CI run is claimed.

Environment: macOS aarch64; Rust/Cargo 1.99.1; LLVM 23.1.1; PostgreSQL 14.20;
Python 3.13.5; cargo-deny 0.20.2; cargo-audit 0.22.1; Gitleaks 8.30.1. Evidence
commands and timestamps are recorded in [commands.json](evidence/commands.json).

## Executed checks

| Check | Command and actual result | Evidence |
| --- | --- | --- |
| CHECK-001 | make lint test build licenses: exit 0. rustfmt/clippy clean; **17 passed, 0 failed, 0 ignored**, no quarantines; build/license/bans/source checks pass. | [Rust log](evidence/rust-checks.log) |
| CHECK-002 | make test-db: exit 0 after OS permission for shared memory. Real migration/RLS/rollback/restart checks pass on a nonsuperuser runtime role with two tenants sharing names/IDs. | [PostgreSQL log](evidence/postgres.log) |
| CHECK-003 | make coverage: exit 2 from Make (helper exits 1). **297/297 lines (100%)** pass the strict line threshold; **0/0 branches are unmeasured**, not 100%. Full coverage gate blocked. | [Raw LLVM](evidence/coverage-llvm.json), [summary with hashes](evidence/coverage-summary.json), [gate log](evidence/coverage-gate.log) |
| CHECK-004 | Refreshed cargo audit: exit 0, 1,403 advisories, 56 locked packages, 0 matched vulnerabilities, no warnings in JSON. RustSec checkout observed at 965bf9ad083b58e6340d65829d85217b18be9290. | [Audit JSON](evidence/audit.json) |
| CHECK-005 | make secrets: exit 0; no leaks found in working-tree scan. Generated build/VCS/virtualenv paths are excluded, not product code. | [Gitleaks JSON](evidence/gitleaks.json), [log](evidence/secrets.log) |

Coverage merges all test executables without counting test source as product
logic. Every current first-party Rust source file appears in the denominator.
No security code is excluded. SQL has no Rust/TS executable coverage denominator;
its behavior is covered by real SQL assertions. No TypeScript product exists yet,
so its mandatory coverage and experience gates are pending, not waived.

Initial checks exposed wrong compiler selection and a clippy warning; both were
fixed and rerun. Installing matching LLVM tools resolved rust-objcopy warnings.
Sandbox PostgreSQL startup failed before tests; the subsequently authorized local
fixture ran successfully and was removed. These earlier failures are not passes.

## Review findings

| Finding | Severity / disposition | Evidence, affected requirement and next action |
| --- | --- | --- |
| FIND-001 | High / ledger repaired, reading gap open | PRD source IDs lacked repository ledger. Recovered IDs/hashes/status; TASK-001 must finish discovery/reading. |
| FIND-002 | High / fixed | Staged scope validated after URL normalization. Raw ambiguous paths now fail first; regression suite covers dot/encoded/backslash/control escapes (REQ-002). |
| FIND-003 | High / fixed | Public lifecycle fields/deserialization allowed inconsistent paused states. Validated restoration/private fields and exhaustive transition tests enforce invariants (REQ-003). |
| FIND-004 | High / fixed for library | Principal no longer constructed from public client fields; current membership checks bind tenant+issuer+subject after authentication. Keys cannot gain privileged authority. Real adapters pending (PLAT-RBAC/OAUTH/KEYS). |
| FIND-005 | Medium / fixed | Homebrew compiler shadowed rustup; wrapper selects exact compiler/rustdoc and checks now pass. |
| FIND-006 | High / open blocker | Branch data absent. Evaluate compatible date-pinned instrumentation toolchain; no full coverage pass (TASK-012a). |
| FIND-007 | High / open scope | API/worker/frontend/providers and most baseline features absent. Continue TASK-003b–012; no full product verification. |
| FIND-008 | Medium / resolved product choice | Pause in preflight/discovery differs from reference; documented in internal contract and research. |
| FIND-009 | Medium / open research | Current marketing includes iOS; preserved in mobile parity backlog pending detailed research. |
| FIND-010 | Medium / fixed | Header helper had unbounded inputs. Added count/name/value/total caps and boundary tests without echoing values. |

## Full Specker gates

| Gate | State at this checkpoint |
| --- | --- |
| Research integrity | Pending: reading 9/55; media/authenticated workflow/frontier incomplete. |
| Plan completeness | Pending: baseline retained; runtime contracts, diagrams and many component docs required. |
| Static checks | Pass for current Rust library; full platform checks pending. |
| Functional | Pass for named domain/storage checks; real adapters and full acceptance pending. |
| Coverage | Blocked: line pass, branch denominator missing; future components pending. |
| Isolation/security | Partial local policy/RLS/secret/audit checks; transports, SAST/fuzz and complete threat coverage pending. |
| Supply chain | Locked graph/license/advisory checks pass; SBOM/provenance/container scans and release verification pending. |
| API/events | Pending: internal contract only; no public API/events or upgrade compatibility evidence. |
| AI/MCP/voice | Pending; live integration prerequisites unconfigured. |
| Experience | Pending: no frontend, locale/a11y or keyboard/screen-reader evidence. |
| Performance/resilience | Pending beyond database rollback/restart; no measured latency/RPO/RTO/SLO claims. |
| Operations | Blocked for container checks (Docker absent); full stack, restore, alerts and runbooks pending. |
| Documentation | Component docs and evidence written; complete diagrams/rendering/clean full-stack start pending. |
| CI/release | Pending: authored CI, strict branch gate fails; remote run/protection/release controls unverified. |

The complete product is not locally verified or release ready. A passing domain
test, RLS fixture or line percentage cannot substitute for the remaining gates.
