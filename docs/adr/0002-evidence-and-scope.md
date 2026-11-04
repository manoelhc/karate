# ADR-0002: Evidence and scope are enforced outside model decisions

Status: Accepted. Owner: implementation agent. Date: 2026-11-04.

Context: target content and model output are untrusted. An assessment platform can become an SSRF proxy or produce false confidence if a model assertion is treated as proof.

Decision: enforce exact scope, target ownership, DNS/IP policy, permitted checks, costs and cancellation in Rust; store model suggestions separately from reproducible observations.

Consequences: limited checks must be labeled precisely. Unsupported agent actions fail explicitly. Local owned fixtures may use private addresses only through test configuration, never an implicit production exception. Full reference parity requires further evaluated check modules.

Alternatives: unrestricted shell/browser/network agent rejected because its authority cannot be bounded by a prompt alone; publishing inferred vulnerabilities rejected because evidence is insufficient.

Verification: redirect/DNS-rebinding/private-address tests, two-tenant fixtures, malformed model output, exhausted budgets, cancellation, and independent reproduction tests. No third-party assessment is authorized by the build request.
