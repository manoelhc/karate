# Web workspace — TASK-006a

Owner: frontend implementer. Status: implemented local browser journey; broader release gates remain open.
React/TypeScript owns presentation only; domain and authorization remain Rust.
This checkout is local-only with direct Git commits/pushes, not Sites hosting.

Journey: signed-out workspace-ID entry → existing trusted OIDC sign-in → session
and tenant metadata → owner rename/suspend/resume → current/all-session logout.
Only already enrolled members can sign in. No browser token entry, storage or
first-login owner. The only persistent browser preference is locale. All customer
metadata stays in a bounded in-memory query cache, cleared on logout/expiry.

Server metadata supplies can_manage for control visibility; every mutation still
reauthorizes in Rust. Browser navigation failures from the callback redirect to a
fixed same-origin generic failure state without reflecting code/state/provider
errors. Non-browser API errors retain the existing JSON/status contract.

Use a client-rendered React application, TanStack Query with no automatic mutation
or query retries, bounded request bodies and responses, and typed validation of
unknown JSON. Native same-origin cookies remain HttpOnly/Secure; unsafe requests
carry X-Karate-CSRF: 1 and the browser supplies Origin. No Authorization header,
third-party script, arbitrary HTML or client-side authority inference. CSP and
security headers apply at the HTTPS static/proxy boundary.

Both en and pt-BR cover all copy and recovery. A calm cream/forest workspace
layout, semantic landmarks, visible focus, labeled fields, native confirmation
dialog and 320px–4K layouts support the primary task. Loading, signed-out, failed
sign-in, unavailable, forbidden, expired, stale revision, mutation-in-flight,
suspended-owner recovery and successful states are explicit. Ambiguous mutations
require refresh before retry; no automatic replay. Read-only members never see
owner controls. Suspension copy states data is retained.

Verification: TypeScript strict/noUncheckedIndexedAccess, ESLint/accessibility,
Prettier/stylelint, locale parity, dependency license/audit, component/API tests,
strict >90% line and branch coverage over every executable frontend source, and
real Chromium → HTTPS → Rust → PostgreSQL → Dex navigation. Include keyboard,
axe, mobile overflow and screenshots. Initial bundle budgets: <=150 KiB gzip JS,
<=10 KiB gzip CSS; measure gzip/brotli. CWV targets LCP<=2.5s, CLS<=0.1, INP<=200ms
are goals, not field measurements. Screen-reader/field metrics remain explicit
unless actually observed. Human/upstream-provider login is not proved by Dex's
synthetic connector. Full product containers, production TLS, erasure and platform
requirements remain pending; no production deployment or paid resource.


Component structure: `App` accepts a QueryClient (required) and optional browser
navigation callback (defaults to location.assign); SignIn owns only ID validation
and pending navigation; SignedIn owns session/tenant queries; TenantPanel owns
local edits and confirmation. Server results remain typed/validated at api.ts.
Small fixed locale catalogs are bundled together (58 keys each) so switching
language works during network failures; their bytes count toward the JS budget.
No additional customer fields or analytics are collected. Erasure is not offered
because the Rust retention/deletion workflow is not yet implemented.

Data minimization review: locale-only persistence, no provider tokens, no session
or customer capture in logs/traces. Logout/expiry clears both query and mutation
caches. Late mutation responses are scoped to the initiating session and cannot
repopulate a signed-out workspace. Native dialog/Escape/focus return, skip link,
semantic labels, axe and 320/1440/3840px checks pass locally; manual screen-reader
verification remains outstanding. Success is announced outside remounting forms.
