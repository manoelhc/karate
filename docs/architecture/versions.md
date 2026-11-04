# Verified toolchain and dependency record

Checked 2026-11-04. Production code language remains Rust; presentation remains
TypeScript/React. MIT copyright and license text are unchanged.

| Item | Exact observed selection | Evidence and decision |
| --- | --- | --- |
| Rust | 1.99.1, edition 2024, aarch64-apple-darwin | Installed compiler 58cf8f9c3; official [stable manifest](https://static.rust-lang.org/dist/channel-rust-stable.toml) dated 2026-10-15 resolves 1.99.1. Pinned in rust-toolchain.toml and CI. |
| LLVM coverage | 23.1.1-rust-1.99.1-stable | Matching llvm-tools component installed and executed; Homebrew LLVM 22 is incompatible with this compiler. |
| PostgreSQL fixture | 14.20 (Homebrew) | Existing local binary, real isolated bootstrap/RLS/restart tests passed. Not a production major-version choice; latest supported deployment version and digest remain unresolved. |
| cargo-deny | 0.20.2 | License, duplicate dependency and source checks pass. |
| cargo-audit | 0.22.1 | Refreshed RustSec database, 1,403 advisories, no matched vulnerability/warning in locked graph. |
| Gitleaks | 8.30.1 | Installed binary; hook pinned to upstream commit 83d9cd684c87d95d656c1458ef04895a7f1cbd8e. Working-tree scan passed. |
| CI checkout | v6 commit d23441a48e516b6c34aea4fa41551a30e30af803 | Resolved using the official GitHub ref API; read-only workflow permission, credentials not persisted. Workflow has not run remotely. |
| Verification helpers | Python 3.13.5 observed; >=3.11 required | Standard library only, uv lockfile; these are engineering tools, not application logic. |

The wrapper selects rustc/rustdoc explicitly because the host's Homebrew Rust
1.96.1 precedes rustup in PATH. No global PATH/config was changed. Initial checks
through cargo without this override failed honestly; wrapper checks succeeded.
Installing matching LLVM tools resolved temporary rust-objcopy dynamic-library
warnings. The latest preserved check log is free of that warning.

## Material Rust dependencies

Exact application and transitive resolutions are in Cargo.lock. Versions were
resolved from the installed registry and checked by cargo-deny and refreshed
RustSec audit. This records compatible pinned versions, not an unsupported claim
that each crate is the latest published release.

| Crate | Version | License | Purpose / primary documentation | Instrumentation and replacement |
| --- | --- | --- | --- | --- |
| serde | 1.0.229 | MIT OR Apache-2.0 | Typed serialization; [upstream](https://github.com/serde-rs/serde) | No I/O; caller spans. Replacement touches contracts. |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | Test-only JSON contract checks; [upstream](https://github.com/serde-rs/json) | No production adapter yet; low replacement cost. |
| uuid | 1.29.0 | MIT OR Apache-2.0 | Typed tenant IDs; [upstream](https://github.com/uuid-rs/uuid) | No UUID generation enabled; storage/API boundary impact if replaced. |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | Closed Rust errors; [upstream](https://github.com/dtolnay/thiserror) | Internal Display text is not localized user copy. Low replacement cost. |
| url | 2.5.8 | MIT OR Apache-2.0 | WHATWG parsing; [versioned docs](https://docs.rs/url/2.5.8/url/) | Validation surrounds normalization. Security-sensitive replacement requires all scope regressions. |
| ICU4X normalizer/properties | 2.4.0 (transitive via IDNA) | Unicode-3.0 | International domain normalization; [upstream](https://github.com/unicode-org/icu4x) | Compatible license checked; coupled to URL parsing. |

Maintenance/security posture is limited to upstream provenance and the dated
advisory/license reports; no vendor support SLA or full source audit is claimed.
Recheck before release. OTel SDK, Rust HTTP/DB/identity/provider libraries, React,
TypeScript, frontend runtime, MCP revision and container digests are deliberately
unselected until their respective implementation slices; those version gates
remain pending. No non-Rust backend language exception has been accepted.

## Coverage tooling gap

Stable instrumentation produces real line data but no branch denominator in this
run. The official [coverage-options documentation](https://doc.rust-lang.org/unstable-book/compiler-flags/coverage-options.html)
places branch instrumentation behind an unstable compiler option. The installed
nightly is 1.78.0-nightly from 2024-02-11 and cannot compile this edition/MSRV.
A compatible, date-pinned instrumentation-only toolchain must be provisioned and
tested after checking disk capacity; production stays on 1.99.1. No bootstrap
override or fabricated branch percentage is used. `make coverage` fails closed.
