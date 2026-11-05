# ADR-0007: bounded public provider-key snapshots

Accepted for TASK-003b2d1, 2026-11-05. Owner: backend implementer.

The expiring immutable JWKS file currently requires restart for each rotation.
Add opt-in discovery and atomic refresh using reqwest/rustls and the existing
AWS-LC-backed JWT verifier. Keep file mode compatible. Pin a single trusted issuer
origin, bound both network documents and elapsed work, and reject redirects. Read
only public metadata/keys from the explicitly configured provider. Provider errors
never appear in user responses or logs.

Architecture exception: a process-local snapshot is appropriate for public signing
keys, not customer state or authorization decisions. Key includes issuer/audience
and exact configured transport policy; maximum retained set is 32 keys/64KiB.
Lifetime is bounded independently of provider cache headers. Successful replacement
invalidates removed keys; failed refresh cannot extend the prior lifetime. Member
roles/revocation stay live in PostgreSQL. A single refresh guard coalesces work;
a cancelled producer cannot extend trust. An expiring cooldown bounds failures.
Multiple API processes each have their own bounded public cache; fleet-wide rate
coordination and provider capacity evidence remain release work.

Keep full browser OIDC/PKCE and durable session lifecycle as required follow-ups.
The crates.io dependency metadata observed 2026-11-05 shows openidconnect 4.1.0 and
oidc-client 4.1.1 still require rsa 0.9; the unresolved RUSTSEC-2023-0071 advisory
prevents a clean default adoption under this repository's no-ignore audit policy.
This does not claim a public-key-only verifier is exploitable by that private-key
attack. Rauthy-client 0.15.1 has optional RSA support and remains a candidate needing
protocol, license and crypto-backend review. Do not hand-roll browser OIDC to avoid
the selection work. No vulnerable dependency or advisory exception is introduced.

Primary evidence (accessed 2026-11-05):
- [OpenID discovery](https://openid.net/specs/openid-connect-discovery-1_0.html)
- [reqwest 0.13.5](https://docs.rs/reqwest/0.13.5/reqwest/struct.ClientBuilder.html)
- [openidconnect dependency metadata](https://crates.io/api/v1/crates/openidconnect/4.1.0/dependencies)
- [oidc-client dependency metadata](https://crates.io/api/v1/crates/oidc-client/4.1.1/dependencies)
- [Rauthy dependency metadata](https://crates.io/api/v1/crates/rauthy-client/0.15.1/dependencies)
- [RustSec advisory](https://rustsec.org/advisories/RUSTSEC-2023-0071.html)

Verification must exercise real TLS and signatures with isolated trust material;
that fixture is not a maintained IdP or full provider integration. No schema,
shared account, customer data, paid resource or deployment action is required.
