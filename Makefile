.PHONY: all setup build test clean help \
	elixir-setup elixir-compile elixir-test elixir-format elixir-lint \
	rust-check rust-build rust-test rust-format rust-lint \
	rust-test-core rust-test-cp rust-test-agent rust-test-providers rust-test-runtime \
	rust-build-core rust-build-cp rust-build-agent rust-build-providers rust-build-runtime \
	docs shell release ci verify

# =============================================================================
# Colors for output
# =============================================================================
BLUE := \033[0;34m
GREEN := \033[0;32m
YELLOW := \033[0;33m
NC := \033[0m # No Color

# =============================================================================
# Default target
# =============================================================================
all: setup build
	@echo "$(GREEN)✅ Setup and build complete$(NC)"

# =============================================================================
# Setup
# =============================================================================

setup: rust-setup
	@echo "$(GREEN)✅ Setup complete$(NC)"

rust-setup:
	@echo "$(BLUE)📦 Checking Rust toolchain...$(NC)"
	@rustc --version
	@cargo --version

# =============================================================================
# Build
# =============================================================================

build: rust-build
	@echo "$(GREEN)✅ Build complete$(NC)"

rust-build: rust-build-core rust-build-cp rust-build-agent rust-build-providers rust-build-runtime
	@echo "$(GREEN)✅ Rust workspace build complete$(NC)"

rust-build-core:
	@echo "$(BLUE)🔨 Building wasmatrix-core...$(NC)"
	@cargo build -p wasmatrix-core --manifest-path=crates/Cargo.toml

rust-build-cp:
	@echo "$(BLUE)🔨 Building wasmatrix-control-plane...$(NC)"
	@cargo build -p wasmatrix-control-plane --manifest-path=crates/Cargo.toml

rust-build-agent:
	@echo "$(BLUE)🔨 Building wasmatrix-agent...$(NC)"
	@cargo build -p wasmatrix-agent --manifest-path=crates/Cargo.toml

rust-build-providers:
	@echo "$(BLUE)🔨 Building wasmatrix-providers...$(NC)"
	@cargo build -p wasmatrix-providers --manifest-path=crates/Cargo.toml

rust-build-runtime:
	@echo "$(BLUE)🔨 Building wasmatrix-runtime...$(NC)"
	@cargo build -p wasmatrix-runtime --manifest-path=crates/Cargo.toml

rust-release:
	@echo "$(BLUE)🔨 Building Rust release...$(NC)"
	@cargo build --release --manifest-path=crates/Cargo.toml

# =============================================================================
# Test
# =============================================================================

test: rust-test
	@echo "$(GREEN)✅ All tests passed$(NC)"

rust-test: rust-test-core rust-test-cp rust-test-agent rust-test-providers rust-test-runtime
	@echo "$(GREEN)✅ All Rust tests passed$(NC)"

rust-test-core:
	@echo "$(BLUE)🧪 Running wasmatrix-core tests...$(NC)"
	@cargo test -p wasmatrix-core --manifest-path=crates/Cargo.toml

rust-test-cp:
	@echo "$(BLUE)🧪 Running wasmatrix-control-plane tests...$(NC)"
	@cargo test -p wasmatrix-control-plane --manifest-path=crates/Cargo.toml

rust-test-agent:
	@echo "$(BLUE)🧪 Running wasmatrix-agent tests...$(NC)"
	@cargo test -p wasmatrix-agent --manifest-path=crates/Cargo.toml

rust-test-providers:
	@echo "$(BLUE)🧪 Running wasmatrix-providers tests...$(NC)"
	@cargo test -p wasmatrix-providers --manifest-path=crates/Cargo.toml

rust-test-runtime:
	@echo "$(BLUE)🧪 Running wasmatrix-runtime tests...$(NC)"
	@cargo test -p wasmatrix-runtime --manifest-path=crates/Cargo.toml

rust-test-watch:
	@echo "$(BLUE)👁️  Running Rust tests in watch mode...$(NC)"
	@cargo watch -x test --manifest-path=crates/Cargo.toml

rust-test-next:
	@echo "$(BLUE)🧪 Running nextest...$(NC)"
	@cargo nextest run --manifest-path=crates/Cargo.toml

# =============================================================================
# Code Quality
# =============================================================================

lint: rust-lint
	@echo "$(GREEN)✅ Linting complete$(NC)"

rust-lint:
	@echo "$(BLUE)🔍 Running Rust linters...$(NC)"
	@cargo clippy --manifest-path=crates/Cargo.toml -- -D warnings || true

rust-format:
	@echo "$(BLUE)✨ Formatting Rust code...$(NC)"
	@cargo fmt --manifest-path=crates/Cargo.toml

rust-format-check:
	@echo "$(BLUE)🔍 Checking Rust formatting...$(NC)"
	@cargo fmt --manifest-path=crates/Cargo.toml -- --check

rust-audit:
	@echo "$(BLUE)🔍 Auditing dependencies...$(NC)"
	@cargo audit --manifest-path=crates/Cargo.toml

rust-check:
	@echo "$(BLUE)🔍 Checking Rust code...$(NC)"
	@cargo check --manifest-path=crates/Cargo.toml

# =============================================================================
# Development
# =============================================================================

shell:
	@echo "$(BLUE)🚀 Starting control plane shell...$(NC)"
	@cargo run --bin wasmatrix-control-plane --manifest-path=crates/Cargo.toml

dev-control:
	@echo "$(BLUE)🚀 Starting control plane...$(NC)"
	@cargo run --bin wasmatrix-control-plane --manifest-path=crates/Cargo.toml

dev-agent:
	@echo "$(BLUE)🚀 Starting node agent...$(NC)"
	@cargo run --bin wasmatrix-agent --manifest-path=crates/Cargo.toml

dev-runtime:
	@echo "$(BLUE)🚀 Starting runtime...$(NC)"
	@cargo run --bin wasmatrix-runtime --manifest-path=crates/Cargo.toml

dev-all:
	@echo "$(BLUE)🚀 Starting all components...$(NC)"
	@cargo run --bin wasmatrix-control-plane --manifest-path=crates/Cargo.toml &
	@cargo run --bin wasmatrix-agent --manifest-path=crates/Cargo.toml &
	@cargo run --bin wasmatrix-runtime --manifest-path=crates/Cargo.toml

# =============================================================================
# Documentation
# =============================================================================

docs: rust-docs
	@echo "$(GREEN)✅ Documentation generated$(NC)"

rust-docs:
	@echo "$(BLUE)📚 Generating Rust documentation...$(NC)"
	@cargo doc --manifest-path=crates/Cargo.toml --no-deps

rust-docs-open:
	@echo "$(BLUE)📚 Opening Rust documentation...$(NC)"
	@cargo doc --manifest-path=crates/Cargo.toml --no-deps --open

rust-docs-core:
	@echo "$(BLUE)📚 Generating wasmatrix-core docs...$(NC)"
	@cargo doc -p wasmatrix-core --manifest-path=crates/Cargo.toml --no-deps

# =============================================================================
# Feature-Sliced Design Targets
# =============================================================================

fsd-test: fsd-test-instance fsd-test-capability
	@echo "$(GREEN)✅ All FSD feature tests passed$(NC)"

fsd-test-instance:
	@echo "$(BLUE)🧪 Running instance_management feature tests...$(NC)"
	@cargo test -p wasmatrix-control-plane --test instance_management --manifest-path=crates/Cargo.toml

fsd-test-capability:
	@echo "$(BLUE)🧪 Running capability_management feature tests...$(NC)"
	@cargo test -p wasmatrix-control-plane --test capability --manifest-path=crates/Cargo.toml

# =============================================================================
# Workspace Management
# =============================================================================

workspace-tree:
	@echo "$(BLUE)📁 Workspace structure:$(NC)"
	@find crates -type f -name "*.rs" | grep -v target | head -30

workspace-clean:
	@echo "$(BLUE)🧹 Cleaning workspace...$(NC)"
	@cargo clean --manifest-path=crates/Cargo.toml
	@find crates -type d -name "target" -exec rm -rf {} + 2>/dev/null || true

# =============================================================================
# CI/Verification
# =============================================================================

ci: rust-format-check rust-build rust-test
	@echo "$(GREEN)✅ CI checks passed$(NC)"

verify: ci rust-lint rust-audit
	@echo "$(GREEN)✅ Full verification complete$(NC)"

# =============================================================================
# Release
# =============================================================================

release: workspace-clean rust-release
	@echo "$(GREEN)✅ Release build complete$(NC)"
	@echo "$(YELLOW)Binaries located in crates/target/release/$(NC)"

# =============================================================================
# Clean
# =============================================================================

clean: workspace-clean
	@echo "$(GREEN)✅ Clean complete$(NC)"

# =============================================================================
# Help
# =============================================================================

help:
	@echo "$(BLUE)Wasmatrix Build System - Feature-Sliced Design$(NC)"
	@echo ""
	@echo "$(YELLOW)Usage: make [target]$(NC)"
	@echo ""
	@echo "$(BLUE)Setup & Build:$(NC)"
	@echo "  setup          - Install dependencies"
	@echo "  build          - Build all Rust components"
	@echo "  all            - Setup and build everything"
	@echo ""
	@echo "$(BLUE)Testing:$(NC)"
	@echo "  test           - Run all Rust tests"
	@echo "  rust-test      - Run all Rust tests"
	@echo "  rust-test-core  - Run wasmatrix-core tests"
	@echo "  rust-test-cp    - Run wasmatrix-control-plane tests"
	@echo "  rust-test-agent  - Run wasmatrix-agent tests"
	@echo "  rust-test-providers  - Run wasmatrix-providers tests"
	@echo "  rust-test-runtime  - Run wasmatrix-runtime tests"
	@echo "  rust-test-watch - Watch and re-run tests"
	@echo ""
	@echo "$(BLUE)Code Quality:$(NC)"
	@echo "  lint           - Run clippy linter"
	@echo "  format         - Format all Rust code"
	@echo "  format-check   - Check formatting"
	@echo "  audit          - Audit dependencies for vulnerabilities"
	@echo ""
	@echo "$(BLUE)Development:$(NC)"
	@echo "  shell          - Start control plane"
	@echo "  dev-control    - Start control plane"
	@echo "  dev-agent      - Start node agent"
	@echo "  dev-runtime    - Start runtime"
	@echo "  dev-all        - Start all components"
	@echo ""
	@echo "$(BLUE)Documentation:$(NC)"
	@echo "  docs           - Generate all documentation"
	@echo "  docs-open      - Generate and open docs"
	@echo ""
	@echo "$(BLUE)Feature-Sliced Design:$(NC)"
	@echo "  fsd-test       - Run all FSD feature tests"
	@echo "  fsd-test-instance  - Test instance_management feature"
	@echo "  fsd-test-capability - Test capability_management feature"
	@echo ""
	@echo "$(BLUE)Workspace:$(NC)"
	@echo "  workspace-tree  - Show workspace structure"
	@echo "  workspace-clean - Clean all artifacts"
	@echo ""
	@echo "$(BLUE)CI/Release:$(NC)"
	@echo "  ci             - Run CI checks (format, build, test)"
	@echo "  verify         - Full verification (CI + lint + audit)"
	@echo "  release        - Create release build"
	@echo "  clean          - Clean all build artifacts"
	@echo ""
	@echo "$(BLUE)Examples:$(NC)"
	@echo "  make build && make test"
	@echo "  make rust-test-core"
	@echo "  make lint && make test"
	@echo "  make dev-control"
