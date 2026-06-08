# dbsnap — developer task runner.
# Run `make` or `make help` for the list of targets.
#
# Targets are grouped to mirror the CI pipeline (.github/workflows/ci.yml) so
# you can reproduce each gate locally before pushing.

CARGO        ?= cargo
DATABASE_URL ?= postgres://dbsnap:dbsnap@localhost:5433/dbsnap
ARGS         ?=

export DATABASE_URL

.DEFAULT_GOAL := help

# ---------------------------------------------------------------------------
##@ General
# ---------------------------------------------------------------------------

.PHONY: help
help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage: make \033[36m<target>\033[0m\n"} \
		/^[a-zA-Z0-9_-]+:.*?##/ { printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2 } \
		/^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) }' $(MAKEFILE_LIST)
	@echo

# ---------------------------------------------------------------------------
##@ Quality (→ quality-gate)
# ---------------------------------------------------------------------------

.PHONY: fmt
fmt: ## Format the whole workspace
	$(CARGO) fmt --all

.PHONY: fmt-check
fmt-check: ## Check formatting (CI: Static Quality)
	$(CARGO) fmt --all --check

.PHONY: clippy
clippy: ## Lint with clippy, warnings as errors (CI: Clippy Lint)
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

.PHONY: check
check: ## Type-check the workspace (CI: Build Check)
	$(CARGO) check --workspace --all-targets --locked

.PHONY: doc
doc: ## Build docs, warnings as errors (CI: Static Quality)
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --workspace --no-deps

.PHONY: quality
quality: fmt-check clippy check doc ## Run every quality gate

# ---------------------------------------------------------------------------
##@ Security (→ security-gate)
# ---------------------------------------------------------------------------

.PHONY: deny
deny: ## Advisories + bans + licenses + sources (CI: Cargo Deny)
	$(CARGO) deny check

.PHONY: security
security: deny ## Run every security gate

# ---------------------------------------------------------------------------
##@ Tests
# ---------------------------------------------------------------------------

.PHONY: test
test: ## Run the test suite (nextest if available, else cargo test)
	@if command -v cargo-nextest >/dev/null 2>&1; then \
		$(CARGO) nextest run --workspace; \
	else \
		$(CARGO) test --workspace; \
	fi

.PHONY: test-junit
test-junit: ## Run tests with the CI profile (emits JUnit report)
	$(CARGO) nextest run --workspace --profile ci

.PHONY: doctest
doctest: ## Run documentation tests
	$(CARGO) test --workspace --doc

# ---------------------------------------------------------------------------
##@ CI
# ---------------------------------------------------------------------------

.PHONY: ci
ci: quality security test doctest ## Run the full pipeline locally (all gates)
	@echo "✅ All local CI gates passed."

# ---------------------------------------------------------------------------
##@ Build & run
# ---------------------------------------------------------------------------

.PHONY: build
build: ## Debug build of the whole workspace
	$(CARGO) build --workspace

.PHONY: release
release: ## Optimized release build of the dbsnap binary
	$(CARGO) build --release --locked -p dbsnap-cli
	@echo "binary: target/release/dbsnap"

.PHONY: install
install: ## Install the dbsnap binary into ~/.cargo/bin
	$(CARGO) install --path crates/dbsnap-cli --locked

.PHONY: run
run: ## Run the CLI (pass args via ARGS, e.g. make run ARGS="diff --verbose")
	$(CARGO) run -p dbsnap-cli -- $(ARGS)

.PHONY: docker
docker: ## Build the Docker image (tag: dbsnap:dev)
	docker build -t dbsnap:dev .

# ---------------------------------------------------------------------------
##@ Local database
# ---------------------------------------------------------------------------

.PHONY: db-up
db-up: ## Start the throwaway PostgreSQL (docker compose)
	docker compose up -d

.PHONY: db-down
db-down: ## Stop and remove the PostgreSQL container + volume
	docker compose down -v

# ---------------------------------------------------------------------------
##@ Housekeeping
# ---------------------------------------------------------------------------

.PHONY: clean
clean: ## Remove build artifacts
	$(CARGO) clean
