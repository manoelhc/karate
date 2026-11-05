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
| CI checkout | v6 commit d23441a48e516b6c34aea4fa41551a30e30af803 | Resolved using the official GitHub ref API; read-only workflow permission, credentials not persisted. Foundation workflow passed remotely at e530e90. |
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
| serde_json | 1.0.154 | MIT OR Apache-2.0 | JSON transport, locale catalogs and contract checks; [upstream](https://github.com/serde-rs/json) | Used by the API; contract-sensitive replacement. |
| uuid | 1.29.0 | MIT OR Apache-2.0 | Typed tenant IDs; [upstream](https://github.com/uuid-rs/uuid) | API enables v4 generation for events; storage/API boundary impact if replaced. |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | Closed Rust errors; [upstream](https://github.com/dtolnay/thiserror) | Internal Display text is not localized user copy. Low replacement cost. |
| url | 2.5.8 | MIT OR Apache-2.0 | WHATWG parsing; [versioned docs](https://docs.rs/url/2.5.8/url/) | Validation surrounds normalization. Security-sensitive replacement requires all scope regressions. |
| ICU4X normalizer/properties | 2.4.0 (transitive via IDNA) | Unicode-3.0 | International domain normalization; [upstream](https://github.com/unicode-org/icu4x) | Compatible license checked; coupled to URL parsing. |

Maintenance/security posture is limited to upstream provenance and the dated
advisory/license reports; no vendor support SLA or full source audit is claimed.
Recheck before release. OTel SDK, full OIDC/provider clients, React,
TypeScript, frontend runtime, MCP revision and container digests are deliberately
unselected until their respective implementation slices; those version gates
remain pending. No non-Rust backend language exception has been accepted.

## Historical coverage tooling gap (resolved for current components below)

Stable instrumentation produces real line data but no branch denominator in this
run. The official [coverage-options documentation](https://doc.rust-lang.org/unstable-book/compiler-flags/coverage-options.html)
places branch instrumentation behind an unstable compiler option. The installed
nightly is 1.78.0-nightly from 2024-02-11 and cannot compile this edition/MSRV.
A compatible, date-pinned instrumentation-only toolchain must be provisioned and
tested after checking disk capacity; production stays on 1.99.1. No bootstrap
override or fabricated branch percentage is used. `make coverage` fails closed.

## Project API continuation

Latest stable direct crate versions were fetched from the official crates.io API
on 2026-11-04 before implementation and pinned exactly. The RustCrypto backend of
jsonwebtoken initially pulled crypto-bigint/rsa versions with active advisories;
AWS-LC removed these dependencies. No advisory exception is configured.

| Dependency | Pin | License / purpose / primary docs |
| --- | --- | --- |
| axum | 0.8.9 | MIT; HTTP routing/extractors; https://docs.rs/axum/0.8.9/axum/ |
| tokio | 1.54.1 | MIT; async I/O, timers and signals; https://docs.rs/tokio/1.54.1/tokio/ |
| tokio-postgres | 0.7.18 | MIT OR Apache-2.0; parameterized async PostgreSQL; https://docs.rs/tokio-postgres/0.7.18/tokio_postgres/ |
| deadpool-postgres | 0.14.2 | MIT OR Apache-2.0; bounded connection pool; https://docs.rs/deadpool-postgres/0.14.2/deadpool_postgres/ |
| jsonwebtoken | 11.1.0 | MIT; signed JWT validation with aws_lc_rs backend; https://docs.rs/jsonwebtoken/11.1.0/jsonwebtoken/ |
| aws-lc-rs / aws-lc-sys | 1.19.0 / 0.47.0 | ISC and upstream compatible combined licenses; crypto backend, C/CMake build; https://github.com/aws/aws-lc-rs |
| tower / http-body-util | 0.5.3 / 0.1.5 | MIT; transport tests and bounded body collection |
| coverage-only Rust | nightly-2026-11-04 | rustc 1.101.0-nightly (708c94ef0 2026-11-03), matching LLVM; branch instrumentation only |

BSD-3-Clause and ISC are allowed after reviewing the transitive crypto/router
licenses; MIT remains the product license. Exact duplicate-version allowances
cover getrandom 0.2.17 (JWT vs newer platform RNG), syn 2.0.119 (macro ecosystem
migration) and wasi 0.11.1 (target generations). Other duplicate versions still
fail. Maintenance/advisory status is a dated tool result, not a vendor support
promise. Provider OIDC/MCP/voice SDKs remain unselected.

HTTP/SQL libraries can be instrumented at shared service/transaction boundaries;
OTel SDK/exporter integration and end-to-end traces remain pending. Crypto code
has no customer-data logging. Replacing the HTTP/pool layer is bounded by public
project contracts; replacing token validation requires all signature/claim and
provider tests. Automated upstream key refresh remains separate work.

Both Rust components now exceed 90% line AND branch coverage with actual counters;
production compiler still 1.99.1. The old nightly/branch-data blocker no longer
applies to current components. See the latest project verification record for
counts and the scope of remaining release gates.

## API-key continuation (2026-11-05)

aws-lc-rs 1.19.0 is now also a direct API dependency for secure random fill,
SHA-256 digest and constant-time verifier equality. Exact installed upstream
source was inspected; the versioned docs.rs fetch was unavailable. This reuses the
existing crypto implementation and compatible license. No resolved package version
changed; the lockfile adds only the direct dependency edge. Updated license,
source and RustSec reports remain required for this checkpoint.

## Provider discovery continuation (2026-11-05)

reqwest 0.13.5 (MIT OR Apache-2.0) is pinned with defaults disabled and only the
rustls feature. TLS uses rustls 0.23.46 / platform-verifier 0.7.1, with AWS-LC;
HTTP/2, HTTP/3, decompression and ambient proxy use are disabled in this client.
The lock includes cross-target optional dependencies; cargo-deny audits the whole
resolved graph. No prior package versions were upgraded. Primary APIs were read
in [versioned docs](https://docs.rs/reqwest/0.13.5/reqwest/struct.ClientBuilder.html)
and the exact installed upstream source. RustSec refresh found no advisory/warning;
final dated evidence belongs in the discovery verification report.

The root-certificate dataset webpki-root-certs 1.0.11 uses CDLA-Permissive-2.0.
Its text permits use/modification/sharing with license-text redistribution and does
not relicense Karate code; a package/version-specific license allowance and a
[copy of that text](../licenses/webpki-root-certs-CDLA-2.0.txt) preserve the
obligation. Include this notice with future distributions that contain the data.
The product MIT license remains unchanged. Exact duplicate seams are base64 0.22.1
(existing JWT/Postgres versus newer HTTP) and untrusted 0.7.1 (AWS-LC versus
rustls-webpki). No blanket duplicate or advisory suppression was added.

The official crates.io API reports openidconnect 4.1.0, oidc-client 4.1.1 and
rauthy-client 0.15.1. Some indexed docs still show older versions. The first two
require rsa 0.9 with RUSTSEC-2023-0071; neither was adopted. Rauthy's optional RSA
and supported-provider behavior need further review. The [decision](../adr/0007-bounded-provider-key-refresh.md)
records primary evidence and leaves browser client selection explicitly open.

## Browser-session continuation (2026-11-05)

openid 0.25.0 is pinned (MIT selected from MIT OR Unlicense), with defaults disabled
and rustls enabled. Upstream exact source/license were inspected; only pure PKCE,
authorization-URL and ID-token verification APIs are called. Network helpers are
not used because they collect before checking response size. The version uses
biscuit 0.7.0/ring 0.17.14; it does not add the previously rejected rsa package.
base64 0.23.1 becomes a direct dependency at the already locked version. The lock
adds 34 packages; existing resolved versions are preserved. [ADR-0008](../adr/0008-browser-oidc-sessions.md)
records rejected alternatives and replacement boundaries. Latest audit checks the
253-package graph without advisory ignores; see dated session evidence.

foldhash 0.2.0 carries Zlib. Its installed license was read: preserve origin,
mark modified sources, retain notice in source distributions. No upstream source
was modified; an exact package/version allowance and [notice](../licenses/foldhash-0.2.0-Zlib.txt)
are retained. windows-sys 0.52.0 is an exact duplicate seam for ring's older target
API versus current platform libraries. No blanket license or duplicate exception.

Dex v2.46.1 is the maintained test provider, with upstream Apache-2.0 LICENSE read.
The fixture uses official ghcr.io/dexidp/dex architecture manifests:

- arm64: sha256:331e0a33d66b182302fd906c435cfcdcd2fe065cee5bd5346668e06379449e8c
- amd64: sha256:ca5daa7da6be22465702e0d93ef74e064231460681093ac253e1479960c829b3

These are fixture pins, not selected product images. Local arm64 runs use the
existing Podman machine; Linux CI uses Docker and the amd64 manifest. Only the
synthetic upstream connector is tested; human/upstream-provider journeys and
Karate's full container stack remain unverified. No system runtime provisioning.
