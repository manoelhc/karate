# Local development

The domain library, PostgreSQL migrations and project API are implemented.
See [project API setup](project-api.md) for runtime configuration and limitations.
Production uses Rust 1.99.1; scripts/cargo.sh selects its compiler/rustdoc explicitly
when Homebrew binaries precede rustup. No global shell configuration is changed.

Run `make install`, then `make lint test build licenses`. The suite requires local
PostgreSQL binaries, OpenSSL 3 and Python >=3.11. It creates a fresh private
Unix-socket PostgreSQL cluster and ephemeral signing keys, exercises domain,
real API/database and process behavior, then removes the fixture. It ignores
operator DATABASE_URL settings and never migrates an existing database.

`make test-db` additionally exercises SQL policies, rollback and restart. Trust
authentication exists only inside mode-0700 temporary fixture directories with
TCP disabled. Production runtime roles must not own tables or bypass RLS.

`make coverage` needs nightly-2026-11-04 with llvm-tools-preview, used only for
instrumentation. It enforces strict >90% line and branch coverage separately for
every Rust component, including executable startup code. `make audit` requires
cargo-audit; `make licenses` requires cargo-deny 0.20.2; `make secrets` uses
Gitleaks 8.30.1. No credentials or raw private fixtures are saved in reports.

`make clean` removes Cargo outputs in this workspace, retaining run evidence.
No Docker runtime or complete production/container verification is available yet.
