EVIDENCE_DIR ?= docs/specker/runs/2026-11-04-karate/evidence/erasure

.PHONY: help install run lint format test build licenses audit secrets test-oidc test-db coverage-lines coverage clean install-rust lint-rust format-rust test-rust build-rust licenses-rust audit-rust coverage-rust

help:
	@printf '%s\n' 'install: fetch locked Rust and web dependencies' 'lint: Rust and web static checks' 'format: format Rust and web' 'test: isolated signed-token, HTTP, PostgreSQL, Dex and domain tests (Docker/Podman required)' 'test-oidc: maintained Dex protocol fixture' 'build: build Rust workspace and static web artifact' 'licenses: dependency license/source checks' 'audit: RustSec dependency audit' 'secrets: scan working tree with Gitleaks' 'coverage-lines: measure per-component line coverage' 'coverage: enforce >90% line AND branch per component' 'test-db: isolated real PostgreSQL tenant tests' 'run: start configured project API' 'erasure-batch: run one configured primary-database erasure batch' 'clean: remove this workspace build artifacts' 'web-install: pinned local Node and npm dependencies' 'web-e2e: real local HTTPS/API/PostgreSQL/Dex browser journey' 'web-dev: loopback presentation preview (sign-in needs HTTPS gateway)' 'web-test: strict whole-web coverage' 'web-lint/web-build/web-licenses/web-audit: frontend gates' 'storybook: report component catalog status'

install-rust:
	./scripts/cargo.sh fetch --locked

lint-rust:
	./scripts/cargo.sh fmt --all -- --check
	./scripts/cargo.sh clippy --workspace --all-targets --locked --offline -- -D warnings

format-rust:
	./scripts/cargo.sh fmt --all

test-rust:
	./scripts/test-postgres.sh ./scripts/cargo.sh test --workspace --locked --offline

test-oidc:
	./scripts/test-postgres.sh ./scripts/cargo.sh test --locked --offline -p karate-api --test dex

build-rust:
	./scripts/cargo.sh build --workspace --locked --offline

licenses-rust:
	./scripts/cargo.sh deny --offline check licenses bans sources

audit-rust:
	./scripts/cargo.sh audit

secrets:
	@mkdir -p $(EVIDENCE_DIR)
	gitleaks dir . --redact --no-banner --report-format json --report-path $(EVIDENCE_DIR)/gitleaks.json

coverage-lines:
	python3 tools/coverage.py --output-dir $(EVIDENCE_DIR)

coverage-rust:
	python3 tools/coverage.py --require-branches --output-dir $(EVIDENCE_DIR)

test-db:
	./scripts/test-postgres.sh

run:
	./scripts/cargo.sh run --locked --offline -p karate-api

clean:
	./scripts/cargo.sh clean

# Complete implemented-component entry points; platform gates remain in run.md.
.PHONY: install lint format test build licenses audit coverage web-install web-lint web-test web-build web-e2e web-licenses web-audit web-dev web-clean storybook
install: install-rust web-install
lint: lint-rust web-lint
format: format-rust
	./scripts/node.sh npm --prefix web run format
test: test-rust web-test
build: build-rust web-build
licenses: licenses-rust web-licenses
audit: audit-rust web-audit
coverage: coverage-rust web-test

web-install:
	python3 tools/web/install-node.py
	./scripts/node.sh npm --prefix web ci --ignore-scripts --no-fund
web-lint:
	./scripts/node.sh npm --prefix web run lint
web-test:
	./scripts/node.sh npm --prefix web test
web-build:
	./scripts/node.sh npm --prefix web run build
web-e2e: build-rust web-build
	./scripts/test-web.sh
web-licenses:
	./scripts/node.sh npm --prefix web run licenses
web-audit:
	./scripts/node.sh npm --prefix web run audit
web-dev: web-build
	./scripts/node.sh npm --prefix web run dev
web-clean:
	rm -rf web/dist web/coverage web/test-results web/playwright-report
storybook:
	@printf '%s\n' 'No reusable component catalog yet; run make web-e2e for the real workspace preview/tests.'

.PHONY: erasure-batch
erasure-batch:
	./scripts/cargo.sh run --locked --offline -p karate-erasure
