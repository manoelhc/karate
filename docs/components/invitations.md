# Identity-bound invitations — TASK-003b2b

Owner: backend implementer; PLAT-TENANT/RBAC/AUDIT. Extend the existing shared
Rust service and NOLOGIN database capabilities; preserve own-row membership RLS.
No dependency changes, email delivery, paid resources or production provisioning.

Owners/admins create a time-limited offer to an exact subject at the configured
issuer. Owner offers may grant admin/member/viewer; admin offers only member/viewer
and cannot restore an old owner/admin. Ownership is assigned separately after
acceptance using the existing protected membership path. The invite UUID is a
locator, never a bearer credential; acceptance requires the addressed identity's
valid signed access token. There is no email-address matching or email token.

POST /v1/tenants/{tenant}/invitations: invitation_id, subject, role, expires_at
(Unix seconds, future and at most seven days). GET uses limit 1–100, after UUID.
POST /{invitation}/accept needs authentication but no existing tenant membership;
POST /{invitation}/revoke needs current management authority. Accept/revoke have
no request payload. Directory entries include lifecycle state and identity binding.
Creation with the same ID and identical issuer/subject/role/expiry/creator is an
idempotent retry; a changed payload conflicts. Accepted/revoked/expired offers
cannot be reused. Lost acceptance responses require inspecting membership state
through an administrator; do not create a new offer automatically.

Serialize all changes with the tenant membership lock under READ COMMITTED.
At issue time capture the inviter's membership revision and the recipient's
previous membership revision (or absence). At acceptance require the inviter to
still be active, at the same revision and allowed to grant the offered role.
Require the recipient record to still match its captured revision and be inactive
or absent. Concurrent offers/acceptances, subsequent revocation/demotion or a
re-enrolled inviter cannot resurrect stale authority. Acceptance creates or
reactivates the addressed membership, preserving its ID when re-enrolling and
incrementing its revision. Membership becomes nonexpiring until explicit revocation.

Invitation create/revoke/accept and enrollment audit commit atomically. Events
record actor, offer, resulting member ID/revision and server time; the immutable
offer retains prior role/revision and grant details. No credentials are stored.
Runtime gets named function execution only; raw invite/member/event writes are
not granted. Read event policy remains tenant/admin scoped. No public function
execution or untrusted search path. SQL repeats Rust role rules as storage defense.

Tests must cover identity/issuer/tenant mismatch, expiry, replay, duplicate IDs,
revocation, role escalation, inviter/recipient revision drift, parallel acceptance,
audit failure rollback, raw SQL permissions and HTTP error/localization contracts.
All new Rust source participates in strict per-component line/branch coverage.
Full OIDC, tenant lifecycle, API keys, UI, retention, OTel and containers stay open.
