# Karate build run

- Mode: build (research → business → specifications → implementation → verification).
- Target: /Users/manoelhc/Documents/GitHub/manoelhc/karate.
- Method: https://github.com/manoelhc/specker at f71612bb25bfb1951b3d0df4abb3de9bf914e9d3.
- Reference: https://www.aikido.dev/attack/aipentest.
- Product: Karate, an independently implemented open-source AI pentesting SaaS.
- License: existing MIT, copyright Manoel Carvalho, preserved.
- Current stage: independent foundation implementation while discovery/specification gaps remain open. **Partial implementation**, not plan ready, fully verified locally, release ready or deployed.
- Foundation base: d8ccc5764529c574c1052c090aacbc5704f912a9. Delivery workflow: commit completed work and push directly to main, without PRs, as instructed by the user on 2026-11-04. Git history records publication; no deployment is authorized.
- Tools: web research, local commands, browser automation available. Docker absent. Rust 1.99.1 and matching LLVM 23.1.1 now verified. PostgreSQL 14.20 used for isolated fixtures; no production version selected.
- Environment: free disk fell below 1 GiB during continuation. Recheck capacity before larger installs. Build outputs removed at handoff; no unrelated files removed. No cloud resources authorized or created.
- Assumptions: English and pt-BR; initial buyer engineering/security teams operating web/API SaaS; local development before choosing cloud deployment. These are proposed, not customer-validated.
- Access: no authenticated Aikido workflow observed; AI/STT/TTS provider credentials not requested or inspected yet.
- Test scope: local owned fixtures; reference vendor is research only.
- Current owner: backend implementer (single agent; backend-engineer skill applied, no delegation).
- Outstanding: full TASK-001/002 and 003b–012, provider/container/experience and operational gates. No mandatory baseline or parity scope waived.

## This continuation

Recovered the 51-page prior research inventory without copying raw vendor
captures. Nine pages have attested substantive reading; four newly fetched public
entry pages remain partial. Current reading coverage: 9/55. Discovery frontier
is open. Restored source IDs cited by the earlier PRD, recorded mobile/iOS scope
discovery, and kept vendor claims separate from observed behavior.

TASK-003a now has a Rust 1.99.1 workspace and locked dependencies. Domain code
enforces explicit role permissions, authentication-before-membership resolution,
tenant/issuer/subject binding, key scope restrictions, strict URL boundaries,
special-address rejection, validated lifecycle restoration, budget arithmetic
and bounded presence-only observations. It contains no network executor.

Migration 0001 creates tenant, membership and project tables with forced RLS and
current authenticated membership predicates. Real isolated PostgreSQL tests
exercise cross-tenant/role denials, revocation, transaction context, rollback and
restart durability. There is no Rust database or identity adapter yet.

Verification: 17 Rust tests pass; lint/build/licenses pass; PostgreSQL fixture
passes; refreshed advisory and secret scans pass. Domain line coverage is
297/297; branches are unmeasured. `make coverage` exits nonzero intentionally.
See [verification](verification.md), [commands](evidence/commands.json) and the
working-tree snapshot for exact evidence. Foundation CI and pinned pre-commit
configuration are written but unrun remotely/uninstalled respectively.

Homebrew Rust 1.96.1 precedes rustup on PATH; scripts/cargo.sh explicitly selects
the pinned compiler/rustdoc. The old nightly is 1.78.0 from 2024 and unusable for
this workspace. No global configuration was changed. The GitHub connector could
not fetch the method tree URL; its local checkout was verified at the exact pin.

No authenticated reference workflow, real OIDC/AI/STT/TTS integration or provider
credential setup has been verified. No password/token was requested or saved.
All testing used isolated fictional identities and owned code/database fixtures.
No reference-vendor scan, outreach, purchase, paid resource or deployment.

## Exact next actions

1. TASK-003b: write identity, Rust PostgreSQL adapter and project API component
   contracts; resolve maintained Rust dependencies and latest supported test DB
   version. Implement real test-IdP verification before membership lookup and
   tenant-safe project create/list/update through the shared service layer, with
   bounded pagination, revision conflicts and durable audit. Keep production
   authentication mandatory and add real transport/DB tests.
2. TASK-001 alongside independent build work: read remaining source pages,
   enumerate the index/mobile-iOS/report frontier, and reconcile provisional PRD
   citations and business claims. No authenticated parity can be claimed yet.
3. TASK-012a: assess space for a compatible date-pinned branch instrumentation
   toolchain; keep production Rust 1.99.1 and fail the strict gate until actual
   branch counters exceed 90%. All future components need separate denominators.

No pending user approval is needed for the next local implementation slice.
