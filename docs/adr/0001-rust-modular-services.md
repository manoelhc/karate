# ADR-0001: Rust application core with separate workers

Status: Accepted. Owner: implementation agent. Date: 2026-11-04.

Context: a security assessment is a long-running workload with strict scope and tenant boundaries; a sprawling microservice graph would multiply authorization and operational risk.

Decision: use a modular Rust application with a separate worker process and TypeScript/React presentation, sharing domain and policy code across REST, MCP and voice.

Consequences: fewer network boundaries and consistent authorization; careful module ownership remains necessary. Scale worker pools independently. PostgreSQL-backed durable queue messages provide atomic job publication; external broker migration is possible when measured throughput justifies it. This is not a language exception.

Alternatives: TypeScript backend violates the selected stack without a demonstrated constraint; many microservices add complexity without observed need; process-local persistence cannot meet restart/HA requirements.

Verification: tenant/role contract tests across transports, worker lease/restart tests, import-boundary checks, database transactions and container smoke tests. All are pending implementation evidence.
