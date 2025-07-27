# PolyTorus Blockchain Platform Makefile

.PHONY: help build test clean lint fmt docs

# Default target
help: ## Show this help message
	@echo "🚀 PolyTorus Blockchain Platform"
	@echo ""
	@echo "Available targets:"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

# Development targets
build: ## Build the project
	@echo "🔨 Building PolyTorus..."
	cargo build

build-release: ## Build the project in release mode
	@echo "🔨 Building PolyTorus (Release)..."
	cargo build --release

test: ## Run all tests
	@echo "🧪 Running tests..."
	cargo test --workspace

test-execution: ## Run execution layer tests
	@echo "🧪 Running execution layer tests..."
	cargo test -p execution

test-consensus: ## Run consensus layer tests
	@echo "🧪 Running consensus layer tests..."
	cargo test -p consensus

test-settlement: ## Run settlement layer tests
	@echo "🧪 Running settlement layer tests..."
	cargo test -p settlement

test-data-availability: ## Run data availability layer tests
	@echo "🧪 Running data availability layer tests..."
	cargo test -p data-availability

test-p2p: ## Run P2P network tests
	@echo "🧪 Running P2P network tests..."
	cargo test -p p2p-network

test-wallet: ## Run wallet tests
	@echo "🧪 Running wallet tests..."
	cargo test -p polytorus-wallet

# Code quality targets
lint: ## Run clippy linter
	@echo "🔍 Running clippy..."
	cargo clippy --all-targets --all-features -- -D warnings

fmt: ## Format code
	@echo "🎨 Formatting code..."
	cargo fmt --all

fmt-check: ## Check if code is formatted
	@echo "🎨 Checking code formatting..."
	cargo fmt --all -- --check

check: ## Run cargo check
	@echo "🔍 Running cargo check..."
	cargo check --workspace

clean: ## Clean build artifacts
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	rm -rf target/
	rm -rf logs/

# Documentation targets
docs: ## Generate documentation
	@echo "📚 Generating documentation..."
	cargo doc --no-deps --open

docs-all: ## Generate documentation for all dependencies
	@echo "📚 Generating documentation (with dependencies)..."
	cargo doc --open

# Docker targets
docker-build: ## Build Docker image
	@echo "🐳 Building Docker image..."
	docker build -t polytorus:latest .

# Development environment
dev-setup: ## Setup development environment
	@echo "🛠️ Setting up development environment..."
	rustup update
	rustup component add clippy rustfmt
	cargo install cargo-watch cargo-edit

dev-watch: ## Watch for changes and rebuild
	@echo "👀 Watching for changes..."
	cargo watch -x check -x test

# Release targets
release-check: fmt-check lint test ## Run all checks for release
	@echo "✅ Release checks passed!"

release-build: clean release-check build-release docs ## Build release version
	@echo "🎉 Release build completed!"

# Installation target
install: build-release ## Install PolyTorus binary
	@echo "📦 Installing PolyTorus..."
	cargo install --path .

# All-in-one targets
all: clean build test lint fmt docs ## Run all development tasks

ci: fmt-check lint test ## Run CI checks

# Version and info
version: ## Show version information
	@echo "PolyTorus Blockchain Platform"
	@echo "Version: $(shell cargo pkgid | cut -d'#' -f2 | cut -d':' -f2)"
	@echo "Rust: $(shell rustc --version)"
	@echo "Cargo: $(shell cargo --version)"

info: version ## Show project information
	@echo ""
	@echo "🏗️ Architecture: 4-Layer Modular Blockchain"
	@echo "  - Execution Layer: WASM smart contracts"
	@echo "  - Settlement Layer: Optimistic rollups"
	@echo "  - Consensus Layer: Pluggable consensus"
	@echo "  - Data Availability: Distributed storage"
	@echo ""
	@echo "🔐 Security: Quantum-resistant cryptography"
	@echo "🌐 Network: WebRTC P2P with advanced features"
	@echo "💼 Wallets: HD wallets with multiple crypto backends"
	@echo ""
	@echo "📊 Test Coverage: $(shell cargo test --workspace 2>&1 | grep -o '[0-9]\+ passed' | head -1 || echo 'Run tests first') tests"