# Project API setup and limitations

The API binary is `karate-api`; `make run` starts it after configuration validation.
It serves only loopback addresses. Run `make test` for a fully automated owned
fixture, including ephemeral RSA signing keys, real PostgreSQL, real loopback HTTP,
valid/invalid signed tokens and binary startup/shutdown. Nothing is deployed.

## Configuration

| Variable | Required value / bounds |
| --- | --- |
| KARATE_ISSUER | Exact trusted HTTPS issuer identifier; <=2048 bytes; no credentials/query/fragment |
| KARATE_AUDIENCE | Dedicated API audience, 1–255 bytes; do not use a browser ID-token audience |
| KARATE_JWKS_FILE | Readable public JWKS JSON snapshot, <=64 KiB, 1–32 unique nonempty key IDs; RSA/RS256/signature keys only |
| KARATE_JWKS_EXPIRES_AT | Absolute Unix timestamp strictly in the future and at most 24h from startup; expiry fails requests closed |
| KARATE_DATABASE_URL | tokio-postgres connection string with Unix socket host and dedicated runtime role; TCP/hostaddr rejected |
| KARATE_BIND | Optional loopback IP:port, default 127.0.0.1:8080; non-loopback rejected |

Supply secrets through the environment or an external supervisor/secret facility,
never a committed .env. Errors omit connection strings and raw provider errors.
The public JWKS snapshot must come from the configured issuer through a trusted
operator process. Tokens cannot supply or redirect key retrieval. Replace the
snapshot and restart for key rotation; a missing key ID fails authentication.
Automatic discovery/refresh, external-provider login/logout and PKCE are pending.

Accepted access tokens: signed RS256 JWTs with typ `at+jwt`, matching iss/aud,
nonempty sub, iat and exp, optional nbf, maximum one-hour lifetime. No clock skew
leeway. Critical/compressed/encrypted headers are unsupported and rejected.
A normal `JWT` ID token is rejected. Raw token material never enters audit/logs.

## Database bootstrap

Use a dedicated migration identity to apply 0001, 0002 then 0003 to a new database.
These are additive local schema migrations; do not run them on a shared database
without its own change authority. No production environment has been provisioned.
Create the runtime role externally with appropriate authentication and grant:

```sql
GRANT USAGE ON SCHEMA karate TO karate_runtime;
GRANT SELECT ON karate.tenants, karate.memberships TO karate_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON karate.projects TO karate_runtime;
GRANT SELECT, INSERT ON karate.audit_events TO karate_runtime;
GRANT SELECT ON karate.membership_events TO karate_runtime;
GRANT EXECUTE ON FUNCTION karate.list_memberships(uuid,bigint,uuid),
    karate.membership_snapshot(uuid,uuid),
    karate.change_membership(uuid,uuid,bigint,text,uuid) TO karate_runtime;
```

The role must not own customer tables or have SUPERUSER/BYPASSRLS. Startup checks
these properties. Never grant UPDATE/DELETE/TRUNCATE on audit records. Tenant and
initial membership provisioning requires an explicit privileged operator action:
insert a tenant and a membership binding that tenant to the exact issuer+subject
of an authorized account. Role values are owner/admin/member/viewer. No identity
is granted access merely because it supplies a tenant UUID or role claim.

The app uses at most eight pooled connections, a two-second pool acquisition/
creation/recycle timeout, three-second statement timeout, one-second lock timeout,
and five-second idle transaction timeout. Statement-local RLS and service policy
both enforce access. Long-lived transactions and cache-based authorization are
not used. There is no automatic retry of ambiguous mutations.

Create requests use explicit project UUIDs for idempotency; equal retries return
200 and do not duplicate audit. PATCH requires the expected revision and returns
409 when stale. After a lost response, GET the project before deciding whether to
retry. UUID pagination is ordered but not a snapshot under concurrent insertions.

## Recovery and outstanding production gates

SIGINT drains accepted HTTP work; process restart reloads immutable configuration.
Database transactions roll back if the audit insert fails. The fixture verifies
rollback and PostgreSQL restart; it does not prove production backup restoration.
Migration 0002 can be applied after existing 0001 projects without rewriting them.
No destructive down migration is supplied: use forward fixes or a verified restore
for persistent environments. Audit retention/deletion and tamper detection are
still required before customer operation.

TCP TLS, reverse-proxy deployment, tenant rate limits, OTel, containers, key/session
revocation, tenant lifecycle and all assessment/provider interfaces remain
open. Do not expose this partial service as a production-ready pentesting platform.

## Membership migration and recovery

Migration 0003 requires a privileged bootstrap identity capable of creating the
NOLOGIN BYPASSRLS `karate_membership_guard` role. It adds member UUIDs/revisions,
a separate membership audit table, and four narrow definer functions. Existing
RLS policies and primary keys remain unchanged. The role has only specific table/
column grants; PUBLIC execution and schema creation are revoked. Never grant this
role to the runtime or a user; startup rejects runtime membership in it. Only the
three functions listed above are callable by the runtime. Do not grant raw member
UPDATE/INSERT/DELETE, event INSERT/UPDATE/DELETE, or helper-function execution.

The migration has only been applied to disposable local databases. Existing
persistent deployments need their own migration authority, backup and assessment
of the UUID backfill/index lock duration. A role name collision aborts rather than
silently trusting a preexisting role. There is no destructive down migration.

For existing members, owners/admins can list, change allowed roles and revoke
through the documented API. Promote a nonexpiring member to owner before demoting
or revoking the last permanent owner. No-op role changes emit no extra audit;
mutations require matching revisions. Revocation is irreversible through this API;
re-enrollment requires a new identity-bound invitation. Tenant provisioning/repair remains an explicit operator
operation. Such operator changes must acquire the same tenant row lock before
changing membership; privileged out-of-band SQL is outside runtime guarantees.
Do not bootstrap a tenant solely with expiring owners. There is no automated
recovery bypass when ownership is missing.

Membership writes serialize on the tenant row using FOR NO KEY UPDATE, bounded by
the existing lock/statement/idle timeouts. Actor permission is read again after
waiting. The Rust policy runs before mutation; the SQL function repeats guards and
writes audit atomically. A lost response requires inspecting the current directory
and revision before another request. Timeouts/audit failures roll back changes and
return a closed error. Customer-facing UI, invitation lifecycle, key lifecycle,
retention/erasure, external audit tamper detection and full OIDC are still pending.

## Invitation migration and workflow

Apply migration 0004 after 0003, then grant the runtime:

```sql
GRANT SELECT ON karate.invitation_events TO karate_runtime;
GRANT EXECUTE ON FUNCTION karate.create_invitation(uuid,uuid,text,text,bigint,uuid),
    karate.list_invitations(uuid,bigint,uuid),
    karate.revoke_invitation(uuid,uuid,uuid),
    karate.accept_invitation(uuid,uuid,uuid) TO karate_runtime;
```

No direct invitation-table privileges, event writes, helper execution or guard-role
membership are needed. Migration 0004 extends the existing NOLOGIN capability
role with membership INSERT/expiry-update and invitation/event privileges; it does
not grant them to the runtime. All new functions revoke PUBLIC execution and use
fixed search paths. RLS stays forced. Only local disposable clusters were migrated.

A manager supplies the recipient's exact subject at the configured issuer, a UUID,
an allowed role and an expiry at most seven days ahead. Share the resulting locator
through an independently authorized channel; this API sends no messages. The
recipient signs in with the addressed identity and accepts. The locator alone
cannot enroll anyone. Owners can grant admin, while admins can grant member/viewer.
After enrollment, use existing owner safeguards for any ownership transfer.

Revoked/expired memberships can be re-enrolled with a new offer that captures
their current revision. Older offers cannot undo subsequent membership changes.
An inviter role change, revocation, expiry or re-enrollment invalidates its prior
offers, even if it later regains authority. Issue a new reviewed offer as needed.
Accepted/revoked offers are terminal. Lost responses require inspecting offer and
membership state before retrying; no automatic retry of ambiguous acceptance.
Audit failure rolls back the whole transition. Invitation events include enrollment
and re-enrollment evidence; membership_events retains role/revoke evidence.
Retention/erasure and external tamper detection remain unimplemented launch gates.

## API-key migration, custody and recovery

Apply migration 0005 after 0004 in an independently authorized environment. Local
verification only uses disposable clusters. Grant runtime:

```sql
GRANT SELECT ON karate.key_events TO karate_runtime;
GRANT EXECUTE ON FUNCTION karate.key_verifier(uuid,uuid),
    karate.list_keys(uuid,bigint,uuid),
    karate.create_key(uuid,uuid,text,text[],bigint,bytea,uuid),
    karate.change_key(uuid,uuid,bigint,bytea,uuid) TO karate_runtime;
```

The verifier function is an internal authentication capability, not an HTTP
endpoint. Do not grant direct service_keys SELECT/writes, audit writes, guard-role
membership or PUBLIC execution. Schema RLS remains forced; management functions
use tenant locking and READ COMMITTED. Only the existing NOLOGIN role gets
credential-table privileges. Never log raw Authorization headers or response
secrets; neither raw secret nor verifier enters the event family.

Interactive owner/admin creates keys for self, with explicit UUID/name/scopes and
expiry within 90 days. Store the one-time secret in an external secret manager;
Karate cannot recover it. A lost create response leaves metadata: rotate at its
current revision or revoke. Rotation replaces the verifier without overlap, keeps
scope/expiry and requires the original creator. For expiry extension, create a
separate reviewed key and revoke the old key; there is no implicit renewal.
Owners can revoke any tenant key, admins only own. Revoked keys stay terminal.

Membership revision changes invalidate keys. After authorized role restoration,
interactive creator rotation can rebind a still-unexpired, unrevoked key; keys
never reactivate automatically. JWT/JWKS outage blocks interactive management but
independent valid keys can serve scoped data requests with current membership.
Database failure blocks authentication. Key revocation takes effect on subsequent
authentication; it does not recall already admitted bounded operations. No cache.

Secrets are never persisted in fixtures or artifacts. Future deployments still
need HTTP/DB TLS, request-rate controls, monitoring, retention/erasure, provider
login and container/recovery verification. This is not a production deployment.
