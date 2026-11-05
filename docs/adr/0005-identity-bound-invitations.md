# ADR-0005: identity-addressed offers with revision-bound delegation

Accepted 2026-11-05 for TASK-003b2b; owner: backend implementer.

Use exact configured issuer + provider subject as the invitation recipient. A UUID
locates the offer but grants no authority. Acceptance always requires the addressed
interactive signed token. Avoid introducing a bearer-link secret or treating an
email address/claim as verified enrollment authority. No mail integration is implied.

Capture inviter and recipient membership revisions at creation. Recheck current
roles, revocation, expiry and both revisions under the existing tenant lock before
acceptance. This conservative rule invalidates old offers after any inviter
membership change, even if its authority later returns. Two offers for an absent
recipient cannot both enroll it; an old offer cannot undo a later revocation.
Allow re-enrollment only when the captured recipient record is still inactive.
Owners offer admin/member/viewer; admins offer only member/viewer and cannot
restore previous owners/admins. Ownership is assigned after enrollment separately.

The shared Rust service enforces credential, scope and role policy. Narrow SQL
capabilities repeat storage guards and atomically persist offer state, enrollment
and audit under READ COMMITTED. The runtime receives no direct writes. No new
third-party dependency or license change. No production/shared migration.

Tradeoff: administrators need a trusted source for exact provider subject IDs.
Provider search/email onboarding and delivery are separate work requiring their
own verification. Full OIDC, API keys, tenant lifecycle, UI, retention, telemetry
and container gates remain open. This is a real resource-server enrollment path,
not evidence of complete browser login or a verified end-to-end SaaS.
