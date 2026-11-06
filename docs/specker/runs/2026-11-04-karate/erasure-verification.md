# Tenant deletion checkpoint — 2026-11-06

Base e075ec520ab971b4acaf4988d66b04e16ab65d3f. Pinned Specker method unchanged.
TASK-003b2e2 is implemented for logical primary-database deletion. This checkpoint
is partial platform implementation, not a deployment or release-readiness claim.
Prior exact CI passed [42242274685](https://github.com/manoelhc/karate/actions/runs/42242274685);
see [receipt](evidence/erasure/prior-ci.json). Current exact CI is checked after the
authorized direct push to main; no PR is created.

## Implemented behavior

Current interactive owners confirm the tenant UUID and revision, choose 7–30 days,
and atomically schedule deletion/suspension/audit. Cancellation stays suspended;
resume is explicit. Pending intent blocks rename/resume. First erasure revalidates
the requester; lost ownership blocks the job. Any current owner can cancel/review
before erasure starts. Erasing closes recovery and inserts into tenant stores.
Deadline and job state are re-read after locking, including rescheduling races.
The API never gives API keys deletion authority or its runtime erasure privileges.

The separate Rust process runs one transaction/batch, at most 100 customer rows,
under a restricted runner role. Tenant locks/SKIP LOCKED serialize destruction
against owner changes and other runners. Checkpoints commit with deletions; failed
batches roll back. Unknown tenant stores fail closed. All 15 existing tenant stores
are emptied, preserving a different tenant. The restricted three-field receipt
prevents tenant UUID re-provisioning and contains no name, identity or credential.
No external effects, arbitrary shell, network assessor or automatic schedule.

## Executed checks

- `make lint test build licenses audit test-db`: pass, [log](evidence/erasure/checks.log).
- Final Rust strengthening: `make lint-rust test-rust web-e2e coverage-rust`: pass,
  [log](evidence/erasure/final.log). 46 Rust tests include real DB/HTTP/TLS/Dex and
  actual binary paths. New tests cover API-key refusal, owner loss after an observed
  lock wait, typed audit, rollback, owner cancellation before a competing runner,
  role isolation, >100 rows, first-batch authority, checkpoint continuation from a
  fresh worker process, future stores, write barriers and anti-resurrection.
- After screenshot wording correction, `make web-lint web-test web-build`: pass,
  [log](evidence/erasure/web-final.log), 22 tests, all five application sources,
  75 translated en/pt-BR keys. No threshold or exclusion reduction.
- Final `make web-e2e`: pass, [log](evidence/erasure/browser-final.log), actual local
  HTTPS/Rust/PostgreSQL/Dex code-PKCE sign-in, schedule/reload/cancel/resume and final
  erasure. Five worker batches; foreign tenant preserved; erased session denied.
  axe reports zero violations, native-dialog Escape/focus and keyboard journey
  pass; 320/1440/3840px checks pass. Reviewed rendered screenshots include the
  [Portuguese pending state](evidence/erasure/workspace-mobile-pt.png) and
  [mobile confirmation](evidence/erasure/deletion-confirmation-mobile.png).
- `make secrets` and `git diff --check`: pass. No new third-party dependency; MIT
  unchanged. Lockfile adds only the internal erasure workspace package. RustSec,
  npm audit and license/source checks pass with no advisory suppression.

| Runtime component | Lines | Branches |
| --- | --- | --- |
| karate-core | 479/479 (100%) | 274/280 (97.86%) |
| karate-api | 2408/2455 (98.09%) | 355/392 (90.56%) |
| karate-erasure | 59/59 (100%) | 8/8 (100%) |
| web | 232/233 (99.57%) | 271/278 (97.48%) |

All four independently exceed 90%, including the new runtime. Production Rust
remains 1.99.1; instrumentation is nightly-2026-11-04. [Rust summary](evidence/erasure/coverage-summary.json),
[LLVM detail](evidence/erasure/coverage-llvm.json), [web summary](evidence/erasure/web-coverage.json),
and [source snapshot](evidence/erasure/snapshot.json) preserve denominators/hashes.
SQL is verified through real integration assertions, not counted as Rust coverage.
PostgreSQL baseline restart passes; erasure evidence specifically proves transaction
rollback and fresh-process continuation, not an erasure-specific DB crash recovery.

Final compressed bundle: JS 102831 bytes gzip (limit 153600), CSS 1764 (limit 10240).
[Browser metadata](evidence/erasure/browser.json) records local Chromium
153.0.8010.12, LCP 152ms and CLS 0 for this unthrottled local sample. These are not
field distributions, INP evidence or a human-provider journey. CI installs its
pinned Playwright Chromium independently and must pass the same assertions.

## Infrastructure/account action record

Only disposable local PostgreSQL clusters/roles/migrations 0001–0008, private TLS
fixtures, existing Podman machine's temporary pinned Dex containers, local build
outputs and cached package downloads were used. Fixtures/processes/keys clean up
on exit. No system CA, shared migration, real customer dataset, external provider
credential, account, production runner/schedule, deployment or paid resource was
created. Git commits/pushes and GitHub CI are authorized delivery actions.
Task-owned Rust artifacts are cleaned after local verification; Node dependencies
and portable web build remain workspace-local and ignored.

## Open gates and next work

Backup/WAL/replica and provider erasure, restore-time tombstone reconciliation,
legal/support holds and general age retention remain release blockers; the receipt
is not physical-media or compliance proof. No product API/erasure container matrix
has run; the Dex fixture container does not satisfy that gate. Complete containers,
restore/recovery, human IdP/SSO, manual screen-reader, field performance, distributed
abuse controls, research frontier, assessments, MCP, voice, billing, observability,
SBOM/provenance and all unfinished Specker baseline items remain open. No gate is
waived by pushing. Next bounded work: tombstone-aware restore/recovery verification
and product container execution before assessing broader deletion readiness.
