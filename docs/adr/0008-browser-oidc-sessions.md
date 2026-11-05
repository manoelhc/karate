# ADR-0008: bounded OIDC adapter and durable browser sessions

Accepted for TASK-003b2d2, 2026-11-05. Rust 1.99.1; MIT product license unchanged.

Use pinned openid 0.25.0 for S256 PKCE, authorization URL construction, JOSE
signature decoding and standard ID-token claims. Read the exact downloaded source,
MIT text and [upstream crate](https://crates.io/crates/openid/0.25.0).
Its network convenience methods check body length after collection, so Karate
calls only pure protocol/verification APIs. Our existing bounded reqwest transport
owns discovery and token exchange. No redirect, ambient proxy, automatic retry,
unbounded response or endpoint selected by a token is permitted.

openidconnect 4.1.0 and oidc-client 4.1.1 require rsa 0.9 with the unresolved
RUSTSEC-2023-0071 advisory. Rauthy-client 0.15.1 uses an internal global client,
redirect-following and unbounded JSON, without a custom transport seam; its claim
shape also couples it to Rauthy. These candidates were inspected, not adopted.
openid uses biscuit 0.7.0/ring 0.17.14 for this RS256 flow. This is a dated audit
and targeted source review, not a complete vendor security audit.

The earlier architecture's browser in-memory provider token storage is superseded
by a backend-for-frontend session boundary: browser JavaScript receives no provider
token. The Rust callback verifies code/PKCE/nonce/state/browser binding and current
membership, then issues a random opaque HttpOnly cookie. Only its SHA-256 verifier
is durable. A fixed configured HTTPS origin controls redirect and CSRF checks;
there is no caller-selected return URL. Public client registration needs no client
secret. Request only openid; do not request offline access or retain refresh tokens.

Karate additionally enforces exact raw issuer, RS256, a known key ID, supported
header types, bounded subject/token sizes, issued-at/expiry/optional not-before,
and optional access-token/code hashes. Discovery authorization/token/key endpoints
must share the trusted issuer's HTTPS origin. Providers with endpoint CDNs,
confidential-client-only registrations, other signing algorithms or ID lifetimes
longer than an hour need a separately reviewed adapter/configuration change.

Membership must already exist. First-login provisioning, email matching and
browser invitation acceptance are not implemented. A session is bound to one
tenant and member revision, intersected with current role on every request.
Interactive session owners/admins can use the existing control plane. API keys
remain restricted. Invitation acceptance still requires the addressed access JWT.

A five-minute one-use flow, four concurrent provider operations per process,
128 pending flows per tenant, 32 live sessions per tenant/identity, fifteen-minute
idle expiry and at-most-one-hour absolute expiry bound work and authority. Local
logout/revoke-all is atomic with audit; already admitted bounded requests are not
recalled. It does not terminate the provider's own session. Provider/backchannel
logout, distributed abuse controls, retention and production cookie/TLS deployment
remain explicit release work.

Maintained-provider verification uses digest-pinned Dex v2.46.1 with its upstream
mockCallback synthetic connector. Real discovery, TLS, code/PKCE, ID signatures,
nonce/hash checks and code replay are exercised. This is not evidence of a human
password journey, external upstream connector or React integration. Dex's
[synthetic connector](https://github.com/dexidp/dex/blob/v2.46.1/connector/mock/connectortest.go)
and [token issuer](https://github.com/dexidp/dex/blob/v2.46.1/server/tokens/issuer.go)
were inspected. Only disposable owned local fixtures receive requests.

ID-token issued-at allows at most five seconds of provider clock lead. Not-before
and expiry remain strict; absolute session expiry is capped at API time + one hour
even when the issuer leads. This handles the observed subsecond host/Podman VM
clock offset, whose Unix-second rounding intermittently produced iat = now + 1.
Tests reject larger future issued-at and verify the cap for accepted small skew.
