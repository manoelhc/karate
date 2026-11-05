# Karate threat model

Assets: tenant membership and credentials; target test secrets; authorized scope; findings and evidence; model/voice credentials; usage balance; audit history.

Trust boundaries: browser↔API, OIDC provider↔token verifier, API↔tenant database, worker↔untrusted target, worker↔model, MCP client↔shared domain services, microphone/transcript↔confirmed action, operator↔private connector.

| Threat | Required mitigation | Test |
| --- | --- | --- |
| Cross-tenant access/foreign-key attachment | Verified membership, scoped queries, composite keys and RLS | Same IDs/names across two tenants and all interfaces |
| SSRF and DNS rebinding | Exact origins, IP validation and connection pinning, no proxy inheritance or redirects | Private/metadata/mixed DNS, redirect and IPv4-mapped IPv6 cases |
| Prompt injection from target/context | Untrusted data separation; typed check proposals; no prompt-granted authority | Target instructs export/secrets/scope expansion; reject |
| Leaked API/target/provider secrets | Non-recoverable key verification; external secret storage/encrypted target credentials; redacted telemetry | Log/export/UI inspection and key lifecycle |
| OIDC token misuse | Maintained protocol library, signature/algorithm/issuer/audience/expiry validation; membership in DB | Forged/expired/wrong-audience/revoked membership |
| Replayed or duplicate jobs | Atomic publication, leased claims, fencing, idempotency key | Crash/retry/expired lease and concurrent requests |
| False confirmed findings | Independent deterministic evidence/retest; explicit hypotheses | Clean and intentionally weak owned fixtures |
| Voice unintended mutation | Review/confirm exact actor+tenant+action+target; short expiry and single-use nonce | Altered/replayed confirmation and wrong role |
| Denial of wallet | Atomic budget reservation, concurrency, rate and provider token limits | Concurrent dispatch and exhausted-budget tests |
| Export injection | Escape HTML/CSV; redaction; scoped report snapshots | Malicious target-supplied strings |

No real customer PII or production target credentials are required for development. Hosting jurisdiction, production retention period, processor contracts, support access and applicable regulation are unresolved launch decisions. This document is not a certification claim.

TASK-003b2a adds concurrent owner-removal, queued-actor revocation, admin role
escalation and privileged-function misuse cases. Tenant row locking, post-lock
authorization, permanent-owner requirements, column-specific capability grants,
fixed function search paths and atomic audit are verified against real PostgreSQL.
The NOLOGIN capability owner bypasses RLS only inside narrow functions; runtime
membership in that role is refused at startup. Existing GUC trust assumptions
remain unchanged. Production retention and privileged-operator governance stay open.

TASK-003b2b binds invitation acceptance to the exact signed issuer/subject. IDs
are non-authorizing locators; no email/bearer-link identity inference. Inviter and
recipient revision snapshots, current role/expiry checks, tenant locking and
single-use state stop stale re-enrollment and concurrent acceptance. Enrollment
audit failure rolls back offer and membership together. No message delivery or
actual customer identity data is required for the fixtures.
