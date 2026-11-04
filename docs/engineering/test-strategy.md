# Verification strategy

Unit and Rust integration tests exercise the domain's actual public interfaces.
Authentication and membership doubles are explicitly fixtures, not real OIDC or
API-key evidence. Every state/event combination and every role/action pair is
checked against independent expected tables; no network is used for scope tests.
Boundary inputs cover URL normalization, identity collisions, key restriction,
revocation/expiry, invalid persisted state, budget overflow and bounded headers.

The SQL suite runs an actual PostgreSQL server under a temporary Unix socket.
It tests the bootstrap migration with a runtime role that neither owns tables nor
bypasses RLS. Two tenants intentionally reuse names and IDs. Assertions exercise
forged context, issuer collisions, missing/expired/revoked membership, viewer
and cross-tenant writes, context reset, rollback, compare-and-set preconditions
and acknowledged data after restart. This is storage integration, not a Rust
database-adapter or end-to-end application test. PostgreSQL-version CI evidence
and production backup/restore/concurrency checks remain separate gates.

`make lint test build licenses secrets` checks existing product code. `make
coverage-lines` merges LLVM counters across all Rust test binaries and checks
the inventory against every source file, including unused files. Product source
contains no test code. No first-party executable code is excluded. Generated
dependency/compiler data and tests are outside the product denominator. The helper
refuses additional workspace components until separate component gates exist.
`make coverage` requires strict >90% raw line AND branch counts, rejecting missing
branches. A 0/0 branch report is blocked, not 100%. No rounding relaxes thresholds.

CI contains passing-scope foundation jobs and a strict coverage job that currently
fails on missing branch data. No remote CI run or branch protection has been
observed. No release workflow exists, and this pipeline cannot establish the
complete platform baseline. React coverage, real OIDC/MCP/voice/provider clients,
SAST/fuzzing, container scans/smoke, resilience/performance, accessibility, i18n,
backup/restore and full end-to-end journeys remain required in TASK-003b–012.
