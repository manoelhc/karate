# ADR-0006: random API keys with revision-bound delegation

Accepted 2026-11-05, TASK-003b2c; owner: backend implementer.

Use 32 secure random bytes from the existing AWS-LC implementation. Store SHA-256
of the canonical credential (version + tenant locator + key locator + secret),
never the secret. Compare fixed-size verifier bytes using AWS-LC constant-time
equality. This is high-entropy credential verification, not password hashing.
The library is pinned at the existing 1.19.0; its packaged rand/digest/constant_time
source and ISC-compatible license were inspected. The docs.rs fetch failed, so
version-specific installed upstream source supplied the API evidence. Cargo.lock
adds only the existing crate as a direct karate-api dependency; no versions change.

Credential lookup reads only a single key record before membership resolution.
After verification, a typed key restriction binds tenant/scopes/member revision.
Core authorization intersects it with current membership and denies privileged
operations. Membership changes invalidate old keys even if authority later returns.
Interactive rotation explicitly rebinds current membership; no automatic revival.

Create/rotation disclose the secret once. Duplicate IDs conflict because a stored
verifier cannot recover plaintext. Rotation is compare-and-set without overlap;
creator only. Owners may revoke others' keys but cannot rotate and impersonate
another identity. Atomic audit protects create/rotate/revoke under tenant locks.
No new auth bypass, arbitrary execution, production resource or language exception.

Tradeoffs: callers must use a secret manager and recover a lost response via an
explicit new rotation/revocation. A 90-day maximum lifetime is a product limit,
not a measured security SLA. Online key attempts require the database; distributed
rate limits remain a release blocker. A request already authenticated can finish
its bounded transaction after revocation; future long-running jobs must recheck
at their documented boundaries. Full OIDC, tenant lifecycle, UI/operations and
container gates remain open.
