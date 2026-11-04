# COMP-DOMAIN — shared policy and domain library

Owner: backend implementer. Path: `crates/karate-core`. Rust 1.99.1, MIT.
This is a library, not an assessment executor. The project REST service now
invokes `authorize_membership` after cryptographic access-token validation and
async current membership lookup. MCP, voice and workers must use the same shared
service/policy; those transports and durable scheduling remain pending.

## Contract

REQ-002/003/006 and PLAT-TENANT/RBAC/AGENTS drive this slice. Public Rust types
are the internal contract; the project API exposes its own versioned HTTP schema. Stable errors in
`lib.rs` are machine codes; localized presentation belongs to transports (the HTTP API now supplies it).
The project HTTP contract is separate. Scope construction validates before URL normalization. Only explicit HTTP(S)
origins and segment-bounded paths are accepted. Encoded paths, dot segments,
backslashes, duplicate slashes and credentials are refused conservatively.
Queries are allowed on requests but not scope definitions. A scope match alone
never authorizes traffic: current membership, proof, DNS/address pinning,
request budgets, cancellation and a configured transport are also necessary.

Lifecycle transitions preserve the phase when paused, clear it on resume or
termination, and reject all changes after completion, cancellation or failure.
Restoration validates paused-state invariants. Budget arithmetic is checked;
the persistence owner must serialize reservation updates in one transaction.
Header observations are not exploit findings. Coverage has an unknown state
when no endpoint denominator exists or counts are inconsistent.

## Authority and data

Role/action permissions are explicit and deny by default. Authentication runs
before membership resolution. Membership binds issuer+subject+tenant, includes
revocation and expiry, and is re-read for each resolution. Principals have no
deserializer or public fields. API-key restrictions can only reduce role
authority; privileged scope delegation is prohibited. Credential validation
is delegated to an authenticator, not implemented as homemade cryptography.

No database, cache, network, process, files, secrets or environment configuration
is accessed by this library. Its values are disposable operation snapshots,
not authoritative process-local persistence. Audit persistence, encryption,
retention, export/deletion and membership concurrency belong to adapters.
There is no telemetry exporter or container for a shared library: consumers
own spans, redaction, runtime configuration, graceful shutdown and deployment.
No latency SLO is measured; functions perform bounded in-process work.

## Verification and limitations

`make lint`, `make test`, `make build`, `make licenses` exercise this component.
Tests cover every role/action, forged tenant membership, authentication ordering,
revocation/expiry, lifecycle transitions, budget overflow and ambiguous URLs.
No test contacts an assessment target. Mock identity/store fixtures prove policy
only; they do not prove OIDC, API-key or provider integration.
Coverage includes every executable source file without security-code exclusions.
Line and branch counts must be reported separately; absent branch tooling blocks
the coverage gate. See the run verification record for executed evidence.
