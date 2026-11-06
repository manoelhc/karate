# Karate

An independently implemented, MIT-licensed AI security-assessment platform in
development. Rust owns backend/domain logic; TypeScript/React now provides the sign-in and workspace settings interface. No third-party assessment is authorized by building this project.

The project API now runs with signed access-token verification, current tenant
membership, PostgreSQL row-level security, paginated project reads, idempotent
creation, revision-checked updates and atomic audit events. Errors support English
and Brazilian Portuguese. Owners/admins can now list existing memberships, change
allowed roles and revoke access, with atomic audit and concurrent last-owner
protection. Identity-bound invitations now support expiry, single-use acceptance
and revision-safe re-enrollment. Scoped API keys support hashed verification,
expiry, atomic rotation/revocation and one-time secret disclosure. Trusted HTTPS
discovery now refreshes signing keys with bounded requests and fail-closed expiry. Browser code/PKCE sign-in uses secure tenant-bound sessions and local revocation.
Explicit operator grants now bootstrap permanent owners; current owners can rename,
suspend and resume tenants with atomic audit. The [React workspace](web/README.md) supports sign-in, session logout and owner
recovery with English/pt-BR copy. No assessment worker exists yet.

```sh
make install
make lint test build licenses
make test-db
make coverage
make web-e2e
```

Prerequisites: Rust 1.99.1 with rustfmt/clippy, PostgreSQL initdb/pg_ctl/psql,
OpenSSL 3, Python >=3.11, a C compiler/CMake for AWS-LC, cargo-deny 0.20.2,
and Docker or a running Podman machine for digest-pinned Dex. `make install`
installs checksum-pinned workspace-local Node 26.13.0 and the locked frontend graph.
Python >=3.12 is needed for the safe Node archive extraction. CI installs the
pinned Playwright browser; local cache use records its actual version.
Coverage uses a separate **nightly-2026-11-04** toolchain with llvm-tools-preview;
production remains on Rust 1.99.1. `make test` provisions only an isolated
Unix-socket database, signing keys, private HTTPS fixture and synthetic Dex container,
then removes them. No human/provider connector journey is claimed by that fixture.

To start the API, supply trusted issuer/audience, an expiring public JWKS file or opt-in HTTPS discovery,
and a nonsuperuser Unix-socket PostgreSQL connection, then run `make run`.
See [API setup](docs/operations/project-api.md) for exact configuration and grants.
There is no anonymous mode, first-login administrator or committed credential.

Latest local checks: **43 Rust tests passed**, including real PostgreSQL/HTTP,
provider/session recovery and tenant lifecycle races. All three components exceed 90%
line and branch coverage: core 462/462 lines, 266/272 branches; API 2292/2339 lines,
351/388 branches; web 190/191 lines, 193/197 branches. Retention/erasure, human-provider integration, provider SSO logout, full Karate
containers and the complete platform remain unfinished. This is partial
implementation, not release readiness.

- [Current run and next actions](docs/specker/runs/2026-11-04-karate/run.md)
- [Latest web verification](docs/specker/runs/2026-11-04-karate/web-verification.md)
- [Prior tenant verification](docs/specker/runs/2026-11-04-karate/tenants-verification.md)
- [Prior session verification](docs/specker/runs/2026-11-04-karate/sessions-verification.md)
- [Prior discovery verification](docs/specker/runs/2026-11-04-karate/discovery-verification.md)
- [Prior API-key verification](docs/specker/runs/2026-11-04-karate/keys-verification.md)
- [Prior invitation verification](docs/specker/runs/2026-11-04-karate/invitations-verification.md)
- [Prior membership verification](docs/specker/runs/2026-11-04-karate/membership-verification.md)
- [Prior project API verification](docs/specker/runs/2026-11-04-karate/projects-verification.md)
- [API contract](docs/contracts/projects-v1.openapi.json) and [component](docs/components/api.md)
- [Architecture](docs/architecture/overview.md), [plan](docs/engineering/plan.md), [traceability](docs/product/traceability.md)
- [Pinned Specker method](https://github.com/manoelhc/specker/tree/f71612bb25bfb1951b3d0df4abb3de9bf914e9d3)

Delivery uses direct commits/pushes to main without PRs. No production deployment
or paid resource has been created. Existing MIT copyright is preserved.
