# Project HTTP contract v1

Project routes require `Authorization: Bearer <credential>` and current tenant
membership. Credentials can be RS256 access JWTs or tenant-bound API keys with the
needed read/write scope. Control-plane routes require interactive JWT authority.
Tenant selection is a UUID path parameter, not authority. JWT keys come from a
trusted expiring file or opt-in bounded HTTPS discovery; stale key trust fails
with UNAVAILABLE. Unknown key IDs do not trigger extra retrieval. Browser
authorization-code/PKCE and sessions remain pending.

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
rate limiting and pagination signing remain unimplemented. UUID cursors reveal only identifiers already
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

## Identity-bound invitations

| Method and path | Input | Success |
| --- | --- | --- |
| POST /v1/tenants/{tenant}/invitations | invitation_id, subject, role, expires_at | 200 invitation, including identical retry |
| GET /v1/tenants/{tenant}/invitations | limit 1–100, after UUID | 200 {items,next_cursor} |
| POST /v1/tenants/{tenant}/invitations/{invitation}/accept | No payload; addressed identity's bearer token | 200 membership |
| POST /v1/tenants/{tenant}/invitations/{invitation}/revoke | No payload; manager's bearer token | 200 invitation |

An invitation contains invitation_id, issuer, subject, role, expires_at (Unix
seconds) and state (pending/expired/revoked/accepted). IDs locate offers and are
not bearer secrets. Subject is the exact identity-provider subject, not an email
address. Issuer is taken from the verified creator token; it cannot be supplied
in the request. Expiry must be in the future and within seven days. Subject is
bounded to 255 bytes. Creation/revocation/directory require ManageMembers; owners
may offer admin/member/viewer, admins only member/viewer and cannot restore former
owners/admins. Owner grants use the existing membership management endpoint.

Acceptance requires the addressed signed identity and rejects service keys. An
existing membership is not required. Both inviter and recipient state are checked
under the tenant lock. Any inviter revision change/revocation/expiry or recipient
revision drift invalidates the grant. Re-enrollment preserves the member ID,
increments revision and clears expired/revoked status; accepted memberships are
nonexpiring until revoked. Concurrent acceptance consumes an offer once. Failed
audit insertion rolls back enrollment and leaves a pending offer usable.

Expired/consumed/revoked offers and stale recipient state return CONFLICT (409).
Wrong identity/tenant or lost inviter authority returns FORBIDDEN (403). After an
ambiguous response, the inviter can inspect the directory/membership revision;
never issue another offer automatically. Identical creation retries require a
still-valid input expiry and do not duplicate audit. No invitation delivery email,
automatic enrollment or claim-based administrator provisioning exists.

## Scoped API keys

| Method and path | Input | Success |
| --- | --- | --- |
| POST /v1/tenants/{tenant}/keys | key_id, name, scopes, expires_at | 200 {key,secret} once |
| GET /v1/tenants/{tenant}/keys | limit 1–100, after UUID | 200 {items,next_cursor} metadata |
| POST /v1/tenants/{tenant}/keys/{key}/rotate | expected_revision | 200 {key,secret} once |
| POST /v1/tenants/{tenant}/keys/{key}/revoke | expected_revision | 200 key metadata |

These routes require an interactive JWT and ManageKeys. Create binds the current
issuer/subject/tenant and membership revision. Scopes are a nonempty, unique
subset of read/write/run/export, at most four; no control-plane delegation.
Names obey the shared 120-character name validator. Expiry is future Unix seconds
at most 90 days ahead. UUID collisions conflict; retries never recover a secret.

Key metadata: key_id, name, issuer, subject, scopes, expires_at, revision, revoked.
Owner lists/revokes any tenant key; admin only own. Rotation requires the original
creator's current interactive authority. It replaces the verifier atomically,
keeps scopes/expiry, increments revision and rebinds the current member revision.
Revoked/expired keys cannot rotate. Revoke is terminal and also works on expired
keys. Stale revisions return CONFLICT. No grace period retains an old verifier.

Only create/rotation returns the secret; store it in an external secret manager.
List/audit/other responses never return it or the verifier. Responses use no-store.
If a response is lost, inspect metadata and explicitly rotate at the current
revision or revoke; never retry with a new ID automatically.

Use `Authorization: Bearer <kk1 credential>` for delegated project operations.
Public UUIDs in the key are locators, not tenant authority. Verification precedes
membership lookup; scopes intersect current role, and membership revision changes
invalidate old keys. Keys cannot accept invitations or manage the control plane.
Run/export scopes do not imply that assessment/export endpoints are implemented.
JWT/JWKS failure does not invalidate separately verified API keys. Revocation is
checked at each new authentication; admitted bounded transactions are not recalled.

## Browser sign-in and sessions

| Method and path | Input | Success |
| --- | --- | --- |
| POST /v1/tenants/{tenant}/login | Origin and X-Karate-CSRF: 1; no body | 200 {authorization_url}, binding cookie |
| GET /v1/auth/callback | state, code or error, optional iss; binding cookie | 303 to fixed public origin; session cookie |
| GET /v1/session | session cookie or opaque session bearer | 200 {tenant_id,session_id,expires_at} |
| POST /v1/session/logout | session; Origin and X-Karate-CSRF: 1 | 204; current session revoked, cookie cleared |
| POST /v1/session/revoke-all | session; Origin and X-Karate-CSRF: 1 | 204; identity's live sessions in this tenant revoked |

Browser routes require configured browser support and existing tenant membership.
Start requests must originate from the exact configured HTTPS public origin. The
caller navigates to authorization_url; callback redirect is fixed, never supplied
by the caller. Cookies are __Host-karate-login (5 minutes) and
__Host-karate-session (at most 1 hour), Secure/HttpOnly/SameSite=Lax/Path=/, no Domain.
The callback clears the binding after a matched success/failure and uses
Referrer-Policy: no-referrer. Missing/malformed query or missing binding fails
without exchanging a code. State/nonce/code/token details are never error copy.

Cookie-authenticated mutations on all existing endpoints require exact Origin and
X-Karate-CSRF: 1; no CORS credential sharing is enabled. Cookie and Authorization
together, repeated credential cookies/headers and invalid credentials fail closed.
A valid current session can access the existing interactive control plane within
its tenant/current role, including keys and member management. Invitation acceptance
requires an addressed access JWT because browser login requires existing membership.
Provider ID tokens are never resource API credentials. Responses are no-store and
errors retain stable codes and en/pt-BR recovery copy.

Invalid/replayed/expired state or claims return UNAUTHENTICATED (401), unavailable
provider/DB returns UNAVAILABLE (503), CSRF or lost membership returns FORBIDDEN
(403), capacity returns LIMIT_REACHED (429). An ambiguous token exchange consumes
the flow; start a new login instead of replaying. Local session revocation does
not promise IdP SSO logout. A changed member revision requires a fresh login.

## Tenant provisioning and lifecycle

| Method and path | Input | Success |
| --- | --- | --- |
| POST /v1/tenants/{tenant}/provision | {grant_id}; addressed access JWT only | 201 {tenant_id,name,state,revision} |
| GET /v1/tenants/{tenant} | Current interactive member | 200 tenant metadata; suspended tenants require owner |
| PATCH /v1/tenants/{tenant} | {name,state,expected_revision}; current owner | 200 tenant metadata |

state is active or suspended. Names follow the existing 120-character validator.
Expected revision is 1 through i64::MAX-1. Wrong revision returns CONFLICT (409),
invalid input VALIDATION (400), missing/wrong grant or permission FORBIDDEN (403),
invalid credentials UNAUTHENTICATED (401), and database/audit failure UNAVAILABLE
(503). A consumed/revoked/expired addressed grant returns CONFLICT. No operator
approval reference, grant identity fields or persistence internals are returned.
Unknown fields are rejected; existing en/pt-BR/no-store error projection applies.

Only a separately issued operator grant permits bootstrap. No public grant issuance
endpoint, automatic owner, caller-selected owner or cross-tenant session bootstrap
exists. Tenant and permanent owner are created with audit in one transaction.
Provision is single-use: inspect GET after an ambiguous response. PATCH is
compare-and-set; identical state/name with the current revision is a no-op without
a new event. After an ambiguous PATCH, GET current metadata before any retry.

Session-cookie PATCH requires existing exact-Origin and X-Karate-CSRF controls.
API keys cannot inspect/change tenant metadata or bootstrap. Suspension blocks
ordinary work and preserves owner recovery; resume does not undo independent
credential/membership revocation. Data remains retained, with no deletion or
backup-erasure claim. Pool/SQL/body bounds apply; distributed rate control remains
unimplemented. These endpoints do not configure or launch an assessment.
