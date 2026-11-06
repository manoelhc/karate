# Karate workspace

React/TypeScript presentation for the existing Rust browser-session and tenant
APIs. Existing members sign in through the configured OIDC provider. Owners can
rename, suspend, resume and schedule/cancel primary-database deletion of their workspace; other members see read-only settings.
English and Brazilian Portuguese are included. No token is stored in JavaScript.

From the repository root:

```sh
make web-install                 # checksum-pinned Node 26.13.0; locked npm graph
make web-lint web-test web-build web-licenses web-audit
make install-rust web-e2e         # real local PostgreSQL/Dex/Rust/HTTPS/Chromium
make web-dev                     # presentation preview only, bound to loopback
```

PostgreSQL binaries and Docker or Podman must already be available. The browser
fixture uses a disposable PostgreSQL cluster, digest-pinned Dex and fresh private
CA. Python and Rust validate the CA; Chromium trusts only the fresh leaf's SPKI
for this isolated run. It never installs a system CA or disables all certificate
checks. An existing macOS Chromium headless-shell cache is used locally when available
(its actual version is recorded); CI installs the pinned
Playwright Chromium with `./scripts/node.sh web/node_modules/.bin/playwright
install chromium`. All owned service processes, fixture databases and keys are
removed on completion. Synthetic identities are not a human-provider integration.

`web/dist` is a portable static artifact. An eventual production HTTPS gateway must
serve it and proxy `/v1` to the configured Rust service on the same origin, preserve
Origin, and apply the security headers in `tools/web/fixture.py`. The fixture is
verification infrastructure, not a production server. `KARATE_PUBLIC_ORIGIN` must
match the gateway and the provider's registered callback. No hosting registration,
deployment or public endpoint is part of this delivery. The HTTP preview serves the built artifact with its strict CSP (no inline HMR
exception) and cannot exercise Secure cookies; use the HTTPS fixture for that journey.

The application uses a bounded in-memory Query cache and 15-second, 64-KiB API
requests. No automatic query/mutation retry or polling. Only locale is persisted.
Stale or ambiguous writes require an explicit refresh before another attempt.
Every mutation is independently authorized in Rust; `can_manage` controls visibility
only. Ending Karate sessions does not end the provider's SSO session.

Local verification covers all executable application sources with strict >90%
line and branch coverage, key parity, TypeScript, JSX accessibility rules, CSS,
formatting, dependency audit/licenses, keyboard navigation, axe and responsive
screenshots. Bundle budgets are 150 KiB gzip JS and 10 KiB gzip CSS. Field Core Web
Vitals, manual screen-reader testing, human-provider login, production container/
TLS/recovery and the full platform baseline remain open gates. There is no Storybook
catalog yet; the app has one access/settings journey and its real browser fixture.

Dependencies are pinned. TypeScript 6.0.3 is the latest compatible 6.0 release for
typescript-eslint 8.72.1; ESLint 9.39.5 is required by the current JSX accessibility
plugin's supported peers. Its upstream maintenance window has ended; a compatible
ESLint 10 migration remains a tooling follow-up. Minimatch >=7 and glob >=9 are
resolved to fixed 11.0.1 and 14.0.2, respectively, for GHSA-g3hv-75wg-m249 and
GHSA-3g3m-2fjr-6pw4. Lint, build and coverage exercise those compatibility overrides.
Do not run `npm audit fix --force`. Runtime license notices ship in the static
artifact. The license checker labels private workspace roots UNLICENSED regardless
of their MIT field; only `karate-web@0.1.0` is excluded from dependency inspection.
Its copied MIT license is checked against the repository original.
