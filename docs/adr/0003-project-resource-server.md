# ADR-0003: bounded JWT resource server and Unix-socket PostgreSQL

Accepted for TASK-003b1, 2026-11-04; owner: backend implementer.

Use Axum, Tokio, tokio-postgres/deadpool-postgres and jsonwebtoken. All domain
permission decisions remain in karate-core and the shared Rust service. Add a
real project API before agent/transport integrations. A bounded public JWKS file
with an absolute freshness deadline permits offline startup and explicit key
rotation while automated discovery/refresh is developed. This is not full OIDC
login and must not be represented as such. Only access-token audience/type is
accepted; ID tokens are not API authorization credentials.

Initially restrict DB connections to Unix sockets, rejecting unencrypted TCP.
This supports the owned local integration fixture and a same-host API/database
operator setup; distributed deployment requires a maintained TLS connector and
its verification. Local ephemeral test state is not product persistence: the
production PostgreSQL service owns durable tables and backup lifecycle.

No product cache is needed for the small current membership/transaction paths:
authority and revision reads require current state. JWKS is immutable bounded
configuration, not an unbounded cache. No auto-created owner or anonymous test
mode exists. All new dependencies have MIT/Apache-compatible declarations;
transitive licenses/advisories must pass the recorded tooling before delivery.

Consequences: automatic JWKS rotation, browser PKCE, key/session revocation,
rate-limiting, OTel, TLS/container verification and the complete platform baseline
stay open. Real signed-token and PostgreSQL tests prove this slice only.
