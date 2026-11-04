# Local development

Only the domain library and tenant/project schema exist. There is no runnable
web/API/worker service yet. `make run` intentionally exits with that explanation.

Install Rust 1.99.1 using rustup with rustfmt and clippy. Run `make install` once
to fetch locked dependencies, then `make lint test build licenses`. cargo-deny
0.20.2 is needed for licenses; cargo-audit is needed for `make audit`. The wrapper
resolves compiler and rustdoc from the pinned toolchain even when Homebrew binaries
precede rustup shims. No user-wide shell configuration is changed.

`make test-db` requires initdb, pg_ctl and psql on PATH. It creates a unique
directory under /tmp, a private Unix socket with no TCP listener, isolated
fictional identities, and a nonsuperuser runtime role. It runs the migration,
negative policy tests and a restart check; cleanup stops and removes that fixture.
It never reads DATABASE_URL or connects to a preexisting database. An OS sandbox
may require permission for PostgreSQL shared memory. Do not weaken permissions on
the fixture directory or repurpose this trust-auth setup for a shared environment.

`make clean` removes only Cargo build outputs in this workspace. Preserved test
reports live in the run evidence directory. The full stack and production
deployment commands remain pending; no cloud/account resource is required here.
