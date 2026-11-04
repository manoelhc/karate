.PHONY: help install run lint format test build licenses audit secrets test-db coverage-lines coverage clean

help:
	@printf '%s\n' 'install: fetch locked Rust dependencies' 'lint: formatting and strict clippy' 'format: format Rust' 'test: isolated signed-token, HTTP, PostgreSQL and domain tests' 'build: build library' 'licenses: dependency license/source checks' 'audit: RustSec dependency audit' 'secrets: scan working tree with Gitleaks' 'coverage-lines: measure per-component line coverage' 'coverage: enforce >90% line AND branch per component' 'test-db: isolated real PostgreSQL tenant tests' 'run: start configured project API' 'clean: remove this workspace build artifacts'

install:
	./scripts/cargo.sh fetch --locked

lint:
	./scripts/cargo.sh fmt --all -- --check
	./scripts/cargo.sh clippy --workspace --all-targets --locked --offline -- -D warnings

format:
	./scripts/cargo.sh fmt --all

test:
	./scripts/test-postgres.sh ./scripts/cargo.sh test --workspace --locked --offline

build:
	./scripts/cargo.sh build --workspace --locked --offline

licenses:
	./scripts/cargo.sh deny --offline check licenses bans sources

audit:
	./scripts/cargo.sh audit

secrets:
	gitleaks dir . --redact --no-banner --report-format json --report-path docs/specker/runs/2026-11-04-karate/evidence/projects/gitleaks.json

coverage-lines:
	python3 tools/coverage.py

coverage:
	python3 tools/coverage.py --require-branches

test-db:
	./scripts/test-postgres.sh

run:
	./scripts/cargo.sh run --locked --offline -p karate-api

clean:
	./scripts/cargo.sh clean
