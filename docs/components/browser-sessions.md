# Browser login and tenant-bound sessions

TASK-003b2d2, PLAT-OAUTH/TENANT/RBAC/AUDIT. Owner: backend implementer.
Status: backend slice implemented and locally verified; see [evidence](../specker/runs/2026-11-04-karate/sessions-verification.md). Full platform remains partial.

Use openid 0.25.0 (MIT selection) for S256 PKCE, authorization URLs, ID-token
signature/issuer/audience/authorized-party/nonce/expiry validation. Karate owns
bounded transport, exact raw issuer comparison, stricter algorithm/claim limits,
browser binding, one-use state, and current tenant membership. Do not call the
library's network convenience methods: their size check occurs after allocation.
Rauthy's hidden global HTTP transport does not preserve Karate's redirect/body
bounds; it is not adopted. No vulnerability ignore or custom OIDC crypto.

The selected journey is a public OIDC client with authorization code + PKCE.
Launch configuration enables browser login with a client ID and exact HTTPS public
origin. Authorization/token/JWKS endpoints stay on the configured issuer origin.
No dynamic registration, arbitrary redirect, email-based membership or first-login
owner. Request only openid scope. Provider access/refresh/ID tokens are never
returned to browser JavaScript or retained after verification.

POST /v1/tenants/{tenant}/login requires exact Origin plus X-Karate-CSRF: 1.
A fresh random state, nonce, PKCE verifier and browser-binding cookie define a
five-minute flow. SQL stores transient verifier/nonce and only state/binding
hashes; raw table access is denied. Each tenant has at most 128 pending flows. Each identity/tenant has at most 32
live sessions at its current membership revision; obsolete generations cannot
block new login or crowd current sessions out of bounded revocation.
Expired flow cleanup is bounded. GET /v1/auth/callback consumes the matching flow
atomically before exchanging the code, so ambiguous exchanges are never retried.
Wrong browser, expired/replayed state, provider denial or invalid claims cannot
mint a session. A new login is the recovery path after a failed exchange.

Sessions are random tenant-bound opaque credentials; only SHA-256 verifiers are
stored. Cookie is Secure, HttpOnly, SameSite=Lax, Path=/ and __Host-prefixed, with
no Domain. Current credential expiry/revocation and membership revision are checked
on every request. Absolute lifetime is one hour, idle lifetime fifteen minutes;
provider ID-token expiry can shorten absolute lifetime. Role changes/re-enrollment
invalidate prior sessions. Session credentials remain bound to one tenant while
allowing interactive control-plane policy. Cookie mutation requests require Origin
and the CSRF header; bearer APIs remain unchanged and ambiguous cookie+header
credentials fail closed. No browser token is accepted as an API bearer ID token.

GET /v1/session returns only current session metadata. POST /v1/session/logout
revokes current session; POST /v1/session/revoke-all revokes all of that identity's
sessions in the selected tenant. Audit is atomic with issuance/revocation. Local
logout does not claim to terminate the provider's own login session. Restart does
not lose durable sessions or revive consumed state. No provider refresh token is
stored; absolute expiry requires sign-in again. Expired credentials fail closed.

All persistence is tenant-scoped, forced RLS and narrow NOLOGIN capabilities;
plaintext PKCE state is short-lived sensitive protocol state, not a long-lived
provider secret. Deletion/retention and encrypted storage deployment remain release
controls. No shared/production migration is authorized or applied by local tests.
Verify with real PostgreSQL, hostile signed-token/fault fixtures, actual HTTP and
a maintained Dex instance with isolated synthetic credentials. Container smoke
for Dex is provider evidence, not complete Karate platform/container readiness.

ID-token issued-at allows at most five seconds of provider clock lead. Not-before
and expiry remain strict; absolute session expiry is capped at API time + one hour
even when the issuer leads. This handles the observed subsecond host/Podman VM
clock offset, whose Unix-second rounding intermittently produced iat = now + 1.
Tests reject larger future issued-at and verify the cap for accepted small skew.

TASK-006a browser callbacks accepting `text/html` redirect failed sign-in to the
fixed public origin `/?signin=failed`, clear binding and set no-referrer. No provider
error/code/state is reflected. JSON clients retain existing error/status behavior.
