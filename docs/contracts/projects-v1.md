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
