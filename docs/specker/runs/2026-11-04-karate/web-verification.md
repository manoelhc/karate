# Web workspace verification — TASK-006a

Base: 256852839805b5dcbb46a6ba5a61f1b5a1911b19. Verified locally on 2026-11-06,
arm64 macOS, Rust 1.99.1, Node 26.13.0, PostgreSQL 14.20 and existing Podman.
[Component](../../../components/web-workspace.md) and [web instructions](../../../../web/README.md)
define this local-only React sign-in/session/tenant-recovery slice. MIT is preserved.
No account, production deployment, Sites registration or paid resource was created.

## Executed checks

| Check | Result | Evidence |
| --- | --- | --- |
| TypeScript strict, ESLint/JSX a11y, stylelint, Prettier, locale parity | Pass, 58 en/pt-BR keys | [final log](evidence/web/final.log) |
| Frontend component/API tests | Pass, 18 tests; no skipped application source | [log](evidence/web/final.log), [coverage and hashes](evidence/web/web-coverage.json) |
| Build and bundle budgets | Pass, measured gzip/brotli delta from zero baseline | [log](evidence/web/final.log) |
| Dependency license allowlist and npm audit | Pass, zero vulnerabilities; MIT runtime notices shipped | [log](evidence/web/final.log) |
| Real HTTPS browser / Rust / PostgreSQL / Dex | Pass, one complete journey with real services | [log](evidence/web/e2e.log), [browser/a11y/lab](evidence/web/browser.json) |
| Rust formatting, strict Clippy and build | Pass, production compiler 1.99.1 | [lint](evidence/web/rust-lint.log), [test compilation](evidence/web/rust-tests.log) |
| Rust regression tests | Pass, 43 named tests | [log](evidence/web/rust-tests.log) |
| Rust licenses/bans/sources and refreshed RustSec audit | Pass, no vulnerability ignore | [log](evidence/web/rust-security.log) |
| Strict Rust component coverage | Pass: both metrics >90% for each Rust component | [summary](evidence/web/coverage-summary.json) |
| Secret scan | Pass, zero findings | [report](evidence/web/gitleaks.json) |

| Component | Lines | Branches |
| --- | --- | --- |
| karate-core | 462/462 (100%) | 266/272 (97.79%) |
| karate-api | 2292/2339 (97.99%) | 351/388 (90.46%) |
| web | 190/191 (99.48%) | 193/197 (97.97%) |

[Coverage execution](evidence/web/coverage.log), [PostgreSQL isolation/restart](evidence/web/postgres.log)
and [cleanup](evidence/web/cleanup.log) also pass. The 43 Rust tests and 18 frontend
tests have no ignored/skipped tests. The separate real-browser journey passes.
The frontend bundle is about 99 KiB gzip JS and 1.7 KiB gzip CSS, within budgets;
exact raw/gzip/brotli bytes and delta from zero are in the final log.

Rust coverage uses the pinned instrumentation-only nightly; production remains
1.99.1. All executable application sources are included. The enforced Rust
module-only index exception and third-party/test exclusions remain unchanged.
Frontend V8 thresholds and the raw-count verifier both require strictly >90%
lines and branches for the web component, including the entry point and locales'
initialization. Source/lock hashes accompany evidence; SQL is tested in PostgreSQL
without inventing a Rust coverage denominator. Tool orchestration is not product
domain logic. CI repeats the implemented component gates after the direct push;
exact-commit CI is checked separately from this pre-commit local record.

## Behavior and visual checks

Browser navigation uses genuine code/PKCE redirects, the real Rust callback and
Secure/HttpOnly session cookies. Tests verify origin/CSRF enforcement, owner rename,
CAS conflict from an independent SQL write, native confirmation/Escape/focus return,
suspension and a fresh owner sign-in for recovery, resume, all-session logout,
read-only control visibility and forged viewer mutation denial, membership loss,
and a safe generic callback failure. Component tests cover expiry, failures,
bounded malformed/oversized response handling and a late mutation finishing after
logout without restoring private cache data. A revoked member can enter another
workspace ID. Runtime authorization continues to live in the Rust service.

Axe reports zero violations for tested English/Portuguese screens and the native
confirmation dialog. Keyboard skip-link and modal focus behavior pass. No horizontal
overflow at 320px or 3840px. These synthetic screenshots were visually reviewed:
[desktop sign-in](evidence/web/signin-desktop.png),
[mobile sign-in](evidence/web/signin-mobile.png),
[desktop workspace](evidence/web/workspace-desktop.png),
[Portuguese mobile workspace](evidence/web/workspace-mobile-pt.png).
The local browser is cached headless Chromium 153.0.8010.12; CI installs Chromium
158.0.8127.18 paired with Playwright 1.65.0. Installed desktop Chrome timed out during
context setup; using the existing isolated headless cache resolved that prerequisite.
No user browser profile was used. LCP/CLS in browser.json are an unthrottled local
sample, not field Core Web Vitals or an INP measurement.

## Operational actions and open gates

Disposable local PostgreSQL clusters, fresh CA/leaf keys and named Dex containers
were created and removed by fixtures. Chromium trusts only the fresh test leaf
SPKI; Python and Rust verify the fixture CA. No system trust or shared DB changed.
The Node toolchain and npm graph were installed only in this workspace. Very low
disk capacity caused one initial fixture startup failure; removing this task's
rebuildable artifacts allowed actual verification. No unrelated files were deleted.
Normal and instrumented Rust builds run sequentially for the same reason. A first instrumented compile failed without a captured compiler diagnostic; its cause was not established. A one-job retry compiled successfully. The coverage runner now honors the build-job limit and retains rendered compiler diagnostics for future failures.

The initial npm graph had glob/minimatch advisories; fixed overrides are documented
in web/README.md and exercised by the full toolchain. ESLint 9 remains the newest
peer-compatible version for the JSX a11y plugin but is outside upstream maintenance;
a compatible ESLint 10 tooling migration remains open. There is no dependency
vulnerability suppression. Runtime notices include all applicable licenses.

Still open: human/upstream-provider login and SSO logout, manual screen-reader
verification, INP/field performance and throttled performance baselines, complete
product containers/production TLS/recovery, tenant erasure/retention, verified-target
assessment execution, MCP/voice/billing/observability, and the full Specker platform
baseline. No production readiness, full WCAG conformance or deployment is claimed.
Next bounded engineering slice is TASK-003b2e2 tenant retention/deletion contracts
and implementation, followed by TASK-004a verified-target scope/budget controls.


## CI paint-observer follow-up

The first push, 7728fae, passed remote Rust tests/licenses, PostgreSQL and strict
Rust coverage. The web job passed lint/tests/build/audit/licenses, then rejected
an LCP sample of zero in Chromium 158 before the keyboard journey. See
[initial CI](evidence/web/initial-ci.json). Zero is not a valid performance pass.
The browser test now waits for the first observed LCP entry before any keyboard
interaction can finalize LCP; it still enforces positive LCP, <=2.5s and CLS<=0.1.
No application behavior, thresholds, retry policy or coverage exclusions changed.

A local follow-up attempt was blocked by disk exhaustion during rebuilding the
unchanged Rust fixture binary; that attempt did not run the browser and is not
counted as a pass. Task-owned partial build output was removed. The amended test
is checked by the full CI browser journey after its follow-up direct push. Prior
local application/coverage evidence remains valid for the unchanged product code.


The second CI run at 4d46ec2 passed the positive LCP check and exposed real 320px
horizontal overflow under Linux font metrics. The header now allows wrapping
between the brand and language control. Viewport assertions retain their exact
bounds and now report the measured document/viewport widths. The full job is
required again after this CSS correction. Earlier macOS screenshots are identified
by their local browser version; they do not establish Linux layout behavior.
