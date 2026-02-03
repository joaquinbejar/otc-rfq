# =============================================================================
# Makefile for OTC RFQ Engine
# High-performance OTC Request-for-Quote engine
# =============================================================================

# Detect current branch
CURRENT_BRANCH := $(shell git rev-parse --abbrev-ref HEAD)

# Project name for packaging
PROJECT_NAME := otc-rfq

# =============================================================================
# Default target
# =============================================================================
.PHONY: all
all: fmt lint test build

# =============================================================================
# 🔧 Build & Run
# =============================================================================

.PHONY: build
build:
	@echo "🔨 Building debug version..."
	cargo build

.PHONY: release
release:
	@echo "🚀 Building release version..."
	cargo build --release

.PHONY: run
run:
	@echo "▶️  Running application..."
	cargo run

.PHONY: run-release
run-release:
	@echo "▶️  Running application (release mode)..."
	cargo run --release

.PHONY: clean
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean

# =============================================================================
# 🧪 Test & Quality
# =============================================================================

.PHONY: test
test:
	@echo "🧪 Running all tests..."
	RUST_LOG=warn cargo test --all-features

.PHONY: test-lib
test-lib:
	@echo "🧪 Running library tests..."
	RUST_LOG=warn cargo test --lib

.PHONY: test-integration
test-integration:
	@echo "🧪 Running integration tests..."
	RUST_LOG=warn cargo test --test '*'

.PHONY: test-doc
test-doc:
	@echo "🧪 Running documentation tests..."
	cargo test --doc

.PHONY: fmt
fmt:
	@echo "✨ Formatting code..."
	cargo +stable fmt --all

.PHONY: fmt-check
fmt-check:
	@echo "🔍 Checking code formatting..."
	cargo +stable fmt --all --check

.PHONY: lint
lint:
	@echo "🔍 Running clippy lints..."
	cargo clippy --all-targets --all-features -- -D warnings

.PHONY: lint-fix
lint-fix:
	@echo "🔧 Auto-fixing lint issues..."
	cargo clippy --fix --all-targets --all-features --allow-dirty --allow-staged -- -D warnings

.PHONY: fix
fix:
	@echo "🔧 Applying cargo fix suggestions..."
	cargo fix --allow-staged --allow-dirty

.PHONY: check
check: fmt-check lint test
	@echo "✅ All checks passed!"

.PHONY: pre-push
pre-push: fix fmt lint-fix test doc
	@echo "✅ All pre-push checks passed!"

# =============================================================================
# 📦 Packaging & Docs
# =============================================================================

.PHONY: doc
doc:
	@echo "📚 Generating documentation..."
	cargo doc --no-deps --document-private-items

.PHONY: doc-open
doc-open:
	@echo "📚 Opening documentation in browser..."
	cargo doc --no-deps --open

.PHONY: doc-check
doc-check:
	@echo "🔍 Checking for missing documentation..."
	cargo clippy -- -W missing-docs

.PHONY: create-doc
create-doc:
	@echo "📝 Generating internal documentation..."
	@mkdir -p doc
	cargo doc --no-deps --document-private-items
	@echo "Documentation generated in target/doc/"

.PHONY: readme
readme:
	@echo "📝 Regenerating README..."
	@command -v cargo-readme > /dev/null || cargo install cargo-readme
	cargo readme > README.md.new
	@echo "New README generated as README.md.new"

.PHONY: publish
publish:
	@echo "📦 Publishing to crates.io..."
	cargo publish --dry-run
	@echo "Dry run complete. Run 'cargo publish' to actually publish."

.PHONY: package
package:
	@echo "📦 Creating package..."
	cargo package --list

# =============================================================================
# 📈 Coverage & Benchmarks
# =============================================================================

.PHONY: coverage
coverage:
	@echo "📊 Generating code coverage report (XML)..."
	@command -v cargo-tarpaulin > /dev/null || cargo install cargo-tarpaulin
	@mkdir -p coverage
	RUST_LOG=warn cargo tarpaulin --verbose --all-features --timeout 120 --out Xml --output-dir coverage

.PHONY: coverage-html
coverage-html:
	@echo "📊 Generating HTML coverage report..."
	@command -v cargo-tarpaulin > /dev/null || cargo install cargo-tarpaulin
	@mkdir -p coverage
	RUST_LOG=warn cargo tarpaulin --all-features --timeout 120 --out Html --output-dir coverage

.PHONY: open-coverage
open-coverage:
	@echo "📊 Opening coverage report..."
	open coverage/tarpaulin-report.html

.PHONY: check-cargo-criterion
check-cargo-criterion:
	@command -v cargo-criterion > /dev/null || cargo install cargo-criterion

.PHONY: bench
bench: check-cargo-criterion
	@echo "⚡ Running benchmarks..."
	cargo criterion --output-format=quiet

.PHONY: bench-show
bench-show:
	@echo "📊 Opening benchmark report..."
	open target/criterion/report/index.html

.PHONY: bench-save
bench-save:
	@echo "💾 Saving benchmark baseline..."
	cargo criterion --save-baseline main

.PHONY: bench-compare
bench-compare:
	@echo "📊 Comparing benchmarks against baseline..."
	cargo criterion --baseline main

.PHONY: bench-json
bench-json: check-cargo-criterion
	@echo "📊 Running benchmarks (JSON output)..."
	cargo criterion --message-format=json

.PHONY: bench-clean
bench-clean:
	@echo "🧹 Cleaning benchmark data..."
	rm -rf target/criterion

# =============================================================================
# 🗄️ Database
# =============================================================================

.PHONY: migrate
migrate:
	@echo "🗄️  Running database migrations..."
	sqlx migrate run

.PHONY: migrate-new
migrate-new:
	@echo "🗄️  Creating new migration..."
	@read -p "Migration name: " name; \
	sqlx migrate add $$name

.PHONY: migrate-revert
migrate-revert:
	@echo "🗄️  Reverting last migration..."
	sqlx migrate revert

.PHONY: db-reset
db-reset:
	@echo "🗄️  Resetting database..."
	sqlx database drop -y || true
	sqlx database create
	sqlx migrate run

# =============================================================================
# 🐳 Docker
# =============================================================================

.PHONY: docker-up
docker-up:
	@echo "🐳 Starting Docker services..."
	docker-compose up -d postgres redis

.PHONY: docker-down
docker-down:
	@echo "🐳 Stopping Docker services..."
	docker-compose down

.PHONY: docker-logs
docker-logs:
	@echo "🐳 Showing Docker logs..."
	docker-compose logs -f

.PHONY: docker-build
docker-build:
	@echo "🐳 Building Docker image..."
	docker build -t $(PROJECT_NAME):latest .

.PHONY: docker-run
docker-run:
	@echo "🐳 Running Docker container..."
	docker run -p 50051:50051 -p 8080:8080 $(PROJECT_NAME):latest

# =============================================================================
# 🧹 Git & Workflow Helpers
# =============================================================================

.PHONY: git-log
git-log:
	@if [ "$(CURRENT_BRANCH)" = "HEAD" ]; then \
		echo "You are in a detached HEAD state. Please check out a branch."; \
		exit 1; \
	fi; \
	echo "📋 Showing git log for branch $(CURRENT_BRANCH) against main:"; \
	git log main..$(CURRENT_BRANCH) --pretty=full

.PHONY: check-spanish
check-spanish:
	@echo "🔍 Checking for Spanish words in code..."
	@rg -n --pcre2 -e '^\s*(//|///|//!|#|/\*|\*).*?[áéíóúÁÉÍÓÚñÑ¿¡]' \
		--glob '!target/*' \
		--glob '!**/*.png' \
		. && (echo "❌ Spanish comments found"; exit 1) || echo "✅ No Spanish comments found"

.PHONY: zip
zip:
	@echo "📦 Creating project zip..."
	@mkdir -p dist
	zip -r dist/$(PROJECT_NAME)-$(shell date +%Y%m%d).zip . \
		-x "target/*" \
		-x ".git/*" \
		-x "*.DS_Store" \
		-x "coverage/*" \
		-x "dist/*"
	@echo "✅ Created dist/$(PROJECT_NAME)-$(shell date +%Y%m%d).zip"

.PHONY: tree
tree:
	@echo "🌳 Project structure:"
	@tree -I 'target|.git|node_modules|coverage|dist' -L 3

.PHONY: loc
loc:
	@echo "📊 Lines of code:"
	@tokei --exclude target --exclude .git

.PHONY: deps
deps:
	@echo "📦 Dependency tree:"
	cargo tree --depth 1

.PHONY: outdated
outdated:
	@echo "📦 Checking for outdated dependencies..."
	@command -v cargo-outdated > /dev/null || cargo install cargo-outdated
	cargo outdated

.PHONY: audit
audit:
	@echo "🔒 Security audit..."
	@command -v cargo-audit > /dev/null || cargo install cargo-audit
	cargo audit

# =============================================================================
# 🤖 GitHub Actions (via act)
# =============================================================================

.PHONY: workflow-build
workflow-build:
	@echo "🤖 Simulating build workflow..."
	DOCKER_HOST="$${DOCKER_HOST}" act push --job build \
		-P ubuntu-latest=catthehacker/ubuntu:latest

.PHONY: workflow-lint
workflow-lint:
	@echo "🤖 Simulating lint workflow..."
	DOCKER_HOST="$${DOCKER_HOST}" act push --job lint

.PHONY: workflow-test
workflow-test:
	@echo "🤖 Simulating test workflow..."
	DOCKER_HOST="$${DOCKER_HOST}" act push --job test

.PHONY: workflow-coverage
workflow-coverage:
	@echo "🤖 Simulating coverage workflow..."
	DOCKER_HOST="$${DOCKER_HOST}" act push --job coverage

.PHONY: workflow
workflow: workflow-build workflow-lint workflow-test
	@echo "✅ All workflows completed!"

# =============================================================================
# 🚀 Release
# =============================================================================

.PHONY: version
version:
	@echo "📋 Current version:"
	@grep '^version' Cargo.toml | head -1

.PHONY: tag
tag:
	@echo "🏷️  Creating git tag..."
	@version=$$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'); \
	git tag -a "v$$version" -m "Release v$$version"; \
	echo "Created tag v$$version"

# =============================================================================
# ❓ Help
# =============================================================================

.PHONY: help
help:
	@echo ""
	@echo "╔══════════════════════════════════════════════════════════════════════╗"
	@echo "║              OTC RFQ Engine - Development Commands                    ║"
	@echo "╚══════════════════════════════════════════════════════════════════════╝"
	@echo ""
	@echo "🔧 Build & Run:"
	@echo "  make build           Compile the project (debug)"
	@echo "  make release         Build in release mode"
	@echo "  make run             Run the main binary"
	@echo "  make run-release     Run in release mode"
	@echo "  make clean           Clean build artifacts"
	@echo ""
	@echo "🧪 Test & Quality:"
	@echo "  make test            Run all tests"
	@echo "  make test-lib        Run library tests only"
	@echo "  make test-integration Run integration tests"
	@echo "  make fmt             Format code"
	@echo "  make fmt-check       Check formatting without applying"
	@echo "  make lint            Run clippy with warnings as errors"
	@echo "  make lint-fix        Auto-fix lint issues"
	@echo "  make fix             Auto-fix Rust compiler suggestions"
	@echo "  make check           Run fmt-check + lint + test"
	@echo "  make pre-push        Run all pre-push checks"
	@echo ""
	@echo "📦 Packaging & Docs:"
	@echo "  make doc             Generate documentation"
	@echo "  make doc-open        Build and open Rust documentation"
	@echo "  make doc-check       Check for missing docs via clippy"
	@echo "  make create-doc      Generate internal docs"
	@echo "  make readme          Regenerate README using cargo-readme"
	@echo "  make publish         Prepare and publish crate to crates.io"
	@echo ""
	@echo "📈 Coverage & Benchmarks:"
	@echo "  make coverage        Generate code coverage report (XML)"
	@echo "  make coverage-html   Generate HTML coverage report"
	@echo "  make open-coverage   Open HTML report"
	@echo "  make bench           Run benchmarks using Criterion"
	@echo "  make bench-show      Open benchmark report"
	@echo "  make bench-save      Save benchmark history snapshot"
	@echo "  make bench-compare   Compare benchmark runs"
	@echo "  make bench-json      Output benchmarks in JSON"
	@echo "  make bench-clean     Remove benchmark data"
	@echo ""
	@echo "🗄️  Database:"
	@echo "  make migrate         Run database migrations"
	@echo "  make migrate-new     Create a new migration"
	@echo "  make migrate-revert  Revert last migration"
	@echo "  make db-reset        Reset database (drop, create, migrate)"
	@echo ""
	@echo "🐳 Docker:"
	@echo "  make docker-up       Start Docker services"
	@echo "  make docker-down     Stop Docker services"
	@echo "  make docker-logs     Show Docker logs"
	@echo "  make docker-build    Build Docker image"
	@echo "  make docker-run      Run Docker container"
	@echo ""
	@echo "🧹 Git & Workflow Helpers:"
	@echo "  make git-log         Show commits on current branch vs main"
	@echo "  make check-spanish   Check for Spanish words in code"
	@echo "  make zip             Create zip without target/ and temp files"
	@echo "  make tree            Visualize project tree"
	@echo "  make loc             Count lines of code"
	@echo "  make deps            Show dependency tree"
	@echo "  make outdated        Check for outdated dependencies"
	@echo "  make audit           Run security audit"
	@echo ""
	@echo "🤖 GitHub Actions (via act):"
	@echo "  make workflow-build  Simulate build workflow"
	@echo "  make workflow-lint   Simulate lint workflow"
	@echo "  make workflow-test   Simulate test workflow"
	@echo "  make workflow-coverage Simulate coverage workflow"
	@echo "  make workflow        Run all workflows"
	@echo ""
	@echo "🚀 Release:"
	@echo "  make version         Show current version"
	@echo "  make tag             Create git tag from Cargo.toml version"
	@echo ""
