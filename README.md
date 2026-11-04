# Karate

An independently implemented, MIT-licensed AI security-assessment platform in
development. Rust owns domain logic and backend services; TypeScript/React is
planned for presentation. Assessments require verified ownership and explicit
authorization. No third-party assessment is authorized by building this project.

Current implementation: shared authorization, scope, lifecycle, budget and
observation rules, plus a tenant/project PostgreSQL migration. **There is no
runnable application server, web UI or assessment worker yet.** The complete
Specker platform scope remains required.

With Rust 1.99.1 (rustup, rustfmt, clippy) installed:

```sh
make install
make lint test build
make licenses       # requires cargo-deny 0.20.2
make test-db        # requires local initdb, pg_ctl and psql
make coverage-lines # requires the matching llvm-tools-preview component
```

The database command uses only an isolated disposable Unix-socket fixture.
It does not connect to an existing database. `make coverage` enforces both
strict >90% thresholds and currently fails because branch data is unavailable.
`make run` reports the missing application runtime instead of starting a demo.

Latest checks: 17 Rust tests passed; real PostgreSQL isolation and restart checks
passed; domain line coverage 297/297 (100%); branch coverage blocked. Dependency
license, RustSec and working-tree secret checks passed. CI is authored but has
not run remotely. Container and full platform gates remain open.

- [Current run and exact next action](docs/specker/runs/2026-11-04-karate/run.md)
- [Verification evidence and unmet gates](docs/specker/runs/2026-11-04-karate/verification.md)
- [Implementation plan](docs/engineering/plan.md) and [traceability](docs/product/traceability.md)
- [Architecture](docs/architecture/overview.md), [domain component](docs/components/domain.md), [persistence component](docs/components/persistence.md)
- [Local development](docs/operations/local-development.md) and [version record](docs/architecture/versions.md)

The pinned [Specker method](https://github.com/manoelhc/specker/tree/f71612bb25bfb1951b3d0df4abb3de9bf914e9d3)
governs this build. This repository is the target product. No deployment, paid
resource or production authentication bypass has been created.
