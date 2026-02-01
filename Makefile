# Makefile for OTC RFQ Engine
# Common development commands

# Detect current branch
CURRENT_BRANCH := $(shell git rev-parse --abbrev-ref HEAD)

# Default target
.PHONY: all
all: test fmt lint build

# Build the project
.PHONY: build
build:
	cargo build

.PHONY: release
release:
	cargo build --release

# Run unit tests
.PHONY: test
test:
	RUST_LOG=warn cargo test --lib

# Run integration tests
.PHONY: test-integration
test-integration:
	RUST_LOG=warn cargo test --test '*'

# Run all tests
.PHONY: test-all
test-all: test test-integration

# Format the code
.PHONY: fmt
fmt:
	cargo +stable fmt --all

# Check formatting
.PHONY: fmt-check
fmt-check:
	cargo +stable fmt --check

# Run Clippy for linting
.PHONY: lint
lint:
	cargo clippy --all-targets --all-features -- -D warnings

.PHONY: lint-fix
lint-fix:
	cargo clippy --fix --all-targets --all-features --allow-dirty --allow-staged -- -D warnings

# Clean the project
.PHONY: clean
clean:
	cargo clean

# Pre-push checks (run before pushing)
.PHONY: pre-push
pre-push: fix fmt lint-fix test doc
	@echo "All pre-push checks passed!"

# Quick check (formatting and linting only)
.PHONY: check
check: fmt-check lint

# Run the project
.PHONY: run
run:
	cargo run

# Run in release mode
.PHONY: run-release
run-release:
	cargo run --release

# Apply cargo fix
.PHONY: fix
fix:
	cargo fix --allow-staged --allow-dirty

# Generate documentation
.PHONY: doc
doc:
	cargo doc --no-deps --document-private-items

# Open documentation in browser
.PHONY: doc-open
doc-open:
	cargo doc --no-deps --open

# Check for missing documentation
.PHONY: doc-check
doc-check:
	cargo clippy -- -W missing-docs

# Run database migrations
.PHONY: migrate
migrate:
	sqlx migrate run

# Create a new migration
.PHONY: migrate-new
migrate-new:
	@read -p "Migration name: " name; \
	sqlx migrate add $$name

# Coverage report
.PHONY: coverage
coverage:
	@command -v cargo-tarpaulin > /dev/null || cargo install cargo-tarpaulin
	mkdir -p coverage
	RUST_LOG=warn cargo tarpaulin --verbose --all-features --timeout 120 --out Xml --output-dir coverage

.PHONY: coverage-html
coverage-html:
	@command -v cargo-tarpaulin > /dev/null || cargo install cargo-tarpaulin
	mkdir -p coverage
	RUST_LOG=warn cargo tarpaulin --all-features --timeout 120 --out Html --output-dir coverage

.PHONY: open-coverage
open-coverage:
	open coverage/tarpaulin-report.html

# Benchmarks
.PHONY: check-cargo-criterion
check-cargo-criterion:
	@command -v cargo-criterion > /dev/null || cargo install cargo-criterion

.PHONY: bench
bench: check-cargo-criterion
	cargo criterion --output-format=quiet

.PHONY: bench-show
bench-show:
	open target/criterion/report/index.html

.PHONY: bench-clean
bench-clean:
	rm -rf target/criterion

# Git log helper
.PHONY: git-log
git-log:
	@if [ "$(CURRENT_BRANCH)" = "HEAD" ]; then \
		echo "You are in a detached HEAD state. Please check out a branch."; \
		exit 1; \
	fi; \
	echo "Showing git log for branch $(CURRENT_BRANCH) against main:"; \
	git log main..$(CURRENT_BRANCH) --pretty=full

# Check for Spanish comments (should be English only)
.PHONY: check-spanish
check-spanish:
	@rg -n --pcre2 -e '^\s*(//|///|//!|#|/\*|\*).*?[áéíóúÁÉÍÓÚñÑ¿¡]' \
		--glob '!target/*' \
		--glob '!**/*.png' \
		. && (echo "Spanish comments found"; exit 1) || echo "No Spanish comments found"

# Docker compose for local development
.PHONY: docker-up
docker-up:
	docker-compose up -d postgres redis

.PHONY: docker-down
docker-down:
	docker-compose down

# GitHub workflow simulation with act
.PHONY: workflow-build
workflow-build:
	DOCKER_HOST="$${DOCKER_HOST}" act push --job build \
		-P ubuntu-latest=catthehacker/ubuntu:latest

.PHONY: workflow-lint
workflow-lint:
	DOCKER_HOST="$${DOCKER_HOST}" act push --job lint

.PHONY: workflow-test
workflow-test:
	DOCKER_HOST="$${DOCKER_HOST}" act push --job test

.PHONY: workflow
workflow: workflow-build workflow-lint workflow-test

# Help
.PHONY: help
help:
	@echo "OTC RFQ Engine - Development Commands"
	@echo ""
	@echo "Build:"
	@echo "  make build        - Build debug version"
	@echo "  make release      - Build release version"
	@echo "  make clean        - Clean build artifacts"
	@echo ""
	@echo "Test:"
	@echo "  make test         - Run unit tests"
	@echo "  make test-all     - Run all tests"
	@echo "  make coverage     - Generate coverage report"
	@echo ""
	@echo "Code Quality:"
	@echo "  make fmt          - Format code"
	@echo "  make lint         - Run clippy lints"
	@echo "  make lint-fix     - Auto-fix lint issues"
	@echo "  make pre-push     - Run all pre-push checks"
	@echo ""
	@echo "Documentation:"
	@echo "  make doc          - Generate documentation"
	@echo "  make doc-open     - Open docs in browser"
	@echo ""
	@echo "Database:"
	@echo "  make migrate      - Run migrations"
	@echo "  make docker-up    - Start local services"
	@echo ""
	@echo "Benchmarks:"
	@echo "  make bench        - Run benchmarks"
	@echo "  make bench-show   - View benchmark results"
