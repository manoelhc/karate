# Karate

An independently implemented, MIT-licensed AI security-assessment platform in
development. Rust owns backend/domain logic; TypeScript/React presentation is
still pending. No third-party assessment is authorized by building this project.

The project API now runs with signed access-token verification, current tenant
membership, PostgreSQL row-level security, paginated project reads, idempotent
creation, revision-checked updates and atomic audit events. Errors support English
and Brazilian Portuguese. No assessment worker or frontend exists yet.

```sh
make install
make lint test build licenses
make test-db
make coverage
```

Prerequisites: Rust 1.99.1 with rustfmt/clippy, PostgreSQL initdb/pg_ctl/psql,
OpenSSL 3, Python >=3.11, a C compiler/CMake for AWS-LC, and cargo-deny 0.20.2.
Coverage uses a separate **nightly-2026-11-04** toolchain with llvm-tools-preview;
production remains on Rust 1.99.1. `make test` provisions only an isolated
Unix-socket database and ephemeral signing keys, then removes them.

To start the API, supply trusted issuer/audience, an expiring public JWKS snapshot,
and a nonsuperuser Unix-socket PostgreSQL connection, then run `make run`.
See [API setup](docs/operations/project-api.md) for exact configuration and grants.
There is no anonymous mode, first-login administrator or committed credential.

Latest local checks: **24 Rust tests passed**, real HTTP/PostgreSQL and binary
startup/shutdown tests passed. Both components exceed 90% line and branch coverage:
core 304/304 lines, 194/200 branches; API 509/523 lines, 107/110 branches.
Full provider login, key management, containers and the complete platform remain
unverified/unfinished; this is a partial implementation, not a ready SaaS.

- [Current run and next actions](docs/specker/runs/2026-11-04-karate/run.md)
- [Project API verification](docs/specker/runs/2026-11-04-karate/projects-verification.md)
- [API contract](docs/contracts/projects-v1.openapi.json) and [component](docs/components/api.md)
- [Architecture](docs/architecture/overview.md), [plan](docs/engineering/plan.md), [traceability](docs/product/traceability.md)
- [Pinned Specker method](https://github.com/manoelhc/specker/tree/f71612bb25bfb1951b3d0df4abb3de9bf914e9d3)

Delivery uses direct commits/pushes to main without PRs. No production deployment
or paid resource has been created. Existing MIT copyright is preserved.
