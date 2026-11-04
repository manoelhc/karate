# Project HTTP contract v1

All routes below require `Authorization: Bearer <RS256 access JWT>` and current
tenant membership. Tenant selection is a UUID path parameter, not authority.

| Method and path | Input | Success | Permission |
| --- | --- | --- | --- |
| POST /v1/tenants/{tenant}/projects | JSON project_id UUID, name string | 201 project; 200 for identical retry | Write |
| GET /v1/tenants/{tenant}/projects | limit 1–100 (default 25), optional after UUID | 200 {items, next_cursor} | Read |
| GET /v1/tenants/{tenant}/projects/{project} | UUIDs | 200 project | Read |
| PATCH /v1/tenants/{tenant}/projects/{project} | JSON name, expected_revision positive int64 below max | 200 updated project | Write |

Project: `{project_id, name, revision}`. Tenant context is already explicit in
the authenticated route. Names obey the shared domain validator. Unknown JSON
fields fail validation. Lists order by UUID, with exclusive cursors; traversal
is not a snapshot under concurrent insertions. A foreign resource ID returns
NOT_FOUND; absent tenant membership returns FORBIDDEN before resource lookup.

Errors: `{code, category, message, retryable, remediation}` and no success data.
VALIDATION=400, UNAUTHENTICATED=401 (Bearer challenge), FORBIDDEN=403,
NOT_FOUND=404, CONFLICT=409, LIMIT_REACHED=429, UNAVAILABLE=503. No raw errors,
credentials or request body are echoed. Accept-Language selects en/pt-BR using
q-values; unknown locales fall back to en. Every response has Content-Language.
User-facing translations are provisional until human review.

`GET /healthz` is public liveness (204); no customer data. Unsupported methods and
routes have bounded localized errors. Idempotent create replay does not repeat
the audit event. A lost PATCH response can be resolved by GET and revision;
never automatically replay a mutation with a changed expected revision.

The machine-readable contract is [OpenAPI 3.1](projects-v1.openapi.json). Tenant
rate limiting, API keys and pagination signing remain unimplemented. UUID cursors reveal only identifiers already
visible in authorized responses and never grant access.

## Existing membership administration

| Method and path | Input | Success |
| --- | --- | --- |
| GET /v1/tenants/{tenant}/memberships | limit 1–100 (default 25), optional after UUID | 200 {items, next_cursor} |
| PATCH /v1/tenants/{tenant}/memberships/{membership} | role, expected_revision | 200 membership |
| POST /v1/tenants/{tenant}/memberships/{membership}/revoke | expected_revision | 200 revoked membership |

A membership contains membership_id, issuer, subject, role, revision, revoked and
expires_at (Unix seconds or null). Directory reads require ManageMembers. Lists
include revoked/expired entries for administrative review, ordered by UUID; they
are not snapshots during concurrent changes. Role updates/revocation require an
active target and matching revision, or return CONFLICT. Replaying a no-op role
change at the current revision is harmless and emits no audit. After an ambiguous
response, inspect directory state before deciding whether to send another change.

Owners can manage any role. Admins can manage only members/viewers and assign only
member/viewer. Service keys cannot manage memberships. Removing an owner requires
another active, nonexpiring owner; assigning owner requires a nonexpiring target.
Concurrent management is serialized per tenant and current authority is checked
after waiting. Revoke never deletes history or grants a restoration path. Stale,
inactive refusals use CONFLICT. Last-owner/expiring-owner assignment refusals use
OWNER_REQUIRED (409) with localized instructions to preserve permanent ownership.
No invitations, auto-enrollment or tenant creation are exposed. Existing 8 KiB
body, pool and SQL timeouts apply; distributed tenant rate limits remain pending.
