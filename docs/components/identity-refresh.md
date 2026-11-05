# Trusted provider discovery and key refresh

Owner: backend implementer. TASK-003b2d1 / PLAT-OAUTH. Implementation and measured
verification are recorded at the run checkpoint; this specification is not a pass.

The API accepts either the existing expiring JWKS file or opt-in HTTPS discovery.
Issuer/audience remain trusted launch configuration; neither a token header nor
network metadata selects an issuer. Discovery appends
`/.well-known/openid-configuration` after removing the issuer's trailing slash.
Returned issuer must match the configured string exactly. JWKS URI must use the
same origin, HTTPS, no userinfo/query/fragment, and at most 2048 bytes. This is an
intentional supported-provider restriction, not general OIDC conformance.

Public provider data is held only in one bounded process snapshot, never persisted
or logged. A complete valid response replaces the snapshot atomically. The existing
RS256 at+jwt verifier still validates signature, type, claims and current key
freshness before any membership access. API keys keep their independent path.
Unknown key IDs never cause an extra refresh. Refresh is demand-driven after half
the configured lifetime; one request refreshes while concurrent requests use the
still-valid snapshot or fail closed. On failure, the original expiry is retained.
Exponential cooldown with jitter bounds repeat attempts. Restart requires a fresh
successful discovery; no disk cache or stale fallback is loaded.

Fixed transport ceilings: no redirects/proxies/automatic retries, HTTPS with
certificate and hostname validation, 2s connect/read and 5s total per GET, 8s total
refresh, 64KiB per document, 32 HTTP/1 response headers, one idle connection per
host. Response must be HTTP 200 JSON without compression. TTL launch setting is
4–3600 seconds (default 600); short values are for owned verification and increase
provider traffic. Cooldown is 1,2,4,8,16,32,60 seconds plus <250ms jitter.
An optional public PEM CA file adds explicitly configured trust for private IdPs.
It never disables certificate/hostname verification. Provider host resolution is
allowed only for this operator-selected origin; no assessment capability exists.

Acceptance: real owned HTTPS fixture, actual signatures, issuer/URI/redirect/TLS/
body/timeout failures, atomic replacement, concurrent refresh, expiry and recovery,
unknown-kid flood resistance, shared service/current membership and process config.
Run `make lint test build licenses test-db coverage`, audit and secret checks.
Maintained IdP, PKCE, session lifecycle, containers and live provider gates remain
open. No authorization callback, ID-token API acceptance, session cookie or browser
UI is implied by this prerequisite.
