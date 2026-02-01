.PHONY: all setup build test clean help \
	elixir-setup elixir-compile elixir-test elixir-format elixir-lint \
	rust-check rust-build rust-test rust-format rust-lint \
	docs shell release

# Default target
all: setup build

# =============================================================================
# Setup
# =============================================================================

setup: elixir-setup rust-setup
	@echo "✅ Setup complete"

elixir-setup:
	@echo "📦 Installing Elixir dependencies..."
	@mix deps.get

rust-setup:
	@echo "📦 Checking Rust toolchain..."
	@rustc --version
	@cargo --version

# =============================================================================
# Build
# =============================================================================

build: elixir-compile rust-build
	@echo "✅ Build complete"

elixir-compile:
	@echo "🔨 Compiling Elixir..."
	@mix compile

rust-check:
	@echo "🔍 Checking Rust code..."
	@cargo check --manifest-path=crates/Cargo.toml

rust-build:
	@echo "🔨 Building Rust workspace..."
	@cargo build --manifest-path=crates/Cargo.toml

rust-release:
	@echo "🔨 Building Rust release..."
	@cargo build --release --manifest-path=crates/Cargo.toml

# =============================================================================
# Test
# =============================================================================

test: elixir-test rust-test
	@echo "✅ All tests passed"

elixir-test:
	@echo "🧪 Running Elixir tests..."
	@mix test

elixir-test-watch:
	@echo "👁️  Running Elixir tests in watch mode..."
	@mix test.watch

rust-test:
	@echo "🧪 Running Rust tests..."
	@cargo test --manifest-path=crates/Cargo.toml

rust-test-release:
	@echo "🧪 Running Rust tests (release mode)..."
	@cargo test --release --manifest-path=crates/Cargo.toml

# =============================================================================
# Code Quality
# =============================================================================

lint: elixir-lint rust-lint
	@echo "✅ Linting complete"

elixir-lint:
	@echo "🔍 Running Elixir linters..."
	@mix credo --strict || true
	@mix dialyzer || true

elixir-format:
	@echo "✨ Formatting Elixir code..."
	@mix format

elixir-format-check:
	@echo "🔍 Checking Elixir formatting..."
	@mix format --check-formatted

rust-lint:
	@echo "🔍 Running Rust linters..."
	@cargo clippy --manifest-path=crates/Cargo.toml -- -D warnings || true

rust-format:
	@echo "✨ Formatting Rust code..."
	@cargo fmt --manifest-path=crates/Cargo.toml

rust-format-check:
	@echo "🔍 Checking Rust formatting..."
	@cargo fmt --manifest-path=crates/Cargo.toml -- --check

# =============================================================================
# Development
# =============================================================================

shell:
	@echo "🚀 Starting Elixir shell..."
	@iex -S mix

dev-control:
	@echo "🚀 Starting control plane..."
	@iex -S mix

dev-agent:
	@echo "🚀 Starting node agent..."
	@cargo run --bin wasmatrix-agent --manifest-path=crates/Cargo.toml

dev-runtime:
	@echo "🚀 Starting runtime..."
	@cargo run --bin wasmatrix-runtime --manifest-path=crates/Cargo.toml

# =============================================================================
# Documentation
# =============================================================================

docs: elixir-docs rust-docs
	@echo "✅ Documentation generated"

elixir-docs:
	@echo "📚 Generating Elixir documentation..."
	@mix docs

rust-docs:
	@echo "📚 Generating Rust documentation..."
	@cargo doc --manifest-path=crates/Cargo.toml --no-deps

rust-docs-open:
	@echo "📚 Opening Rust documentation..."
	@cargo doc --manifest-path=crates/Cargo.toml --no-deps --open

# =============================================================================
# Release
# =============================================================================

release: clean elixir-setup rust-release
	@echo "✅ Release build complete"
	@echo "Binaries located in crates/target/release/"

# =============================================================================
# Clean
# =============================================================================

clean: elixir-clean rust-clean
	@echo "✅ Clean complete"

elixir-clean:
	@echo "🧹 Cleaning Elixir build artifacts..."
	@mix clean
	@rm -rf _build deps mix.lock

rust-clean:
	@echo "🧹 Cleaning Rust build artifacts..."
	@cargo clean --manifest-path=crates/Cargo.toml

# =============================================================================
# CI/Verification
# =============================================================================

ci: setup elixir-format-check elixir-compile elixir-test rust-format-check rust-build rust-test
	@echo "✅ CI checks passed"

verify: ci lint
	@echo "✅ Full verification complete"

# =============================================================================
# Help
# =============================================================================

help:
	@echo "Wasmatrix Build System"
	@echo ""
	@echo "Usage: make [target]"
	@echo ""
	@echo "Setup & Build:"
	@echo "  setup          - Install all dependencies (Elixir + Rust)"
	@echo "  build          - Build all components"
	@echo "  all            - Setup and build everything"
	@echo ""
	@echo "Testing:"
	@echo "  test           - Run all tests"
	@echo "  elixir-test    - Run Elixir tests only"
	@echo "  rust-test      - Run Rust tests only"
	@echo ""
	@echo "Code Quality:"
	@echo "  lint           - Run all linters"
	@echo "  format         - Format all code"
	@echo "  ci             - Run CI checks (format, compile, test)"
	@echo "  verify         - Full verification (CI + lint)"
	@echo ""
	@echo "Development:"
	@echo "  shell          - Start Elixir shell (iex -S mix)"
	@echo "  dev-control    - Start control plane"
	@echo "  dev-agent      - Start node agent"
	@echo "  dev-runtime    - Start runtime"
	@echo ""
	@echo "Documentation:"
	@echo "  docs           - Generate all documentation"
	@echo "  elixir-docs    - Generate Elixir docs"
	@echo "  rust-docs      - Generate Rust docs"
	@echo ""
	@echo "Release:"
	@echo "  release        - Create release build"
	@echo "  rust-release   - Build Rust in release mode"
	@echo ""
	@echo "Maintenance:"
	@echo "  clean          - Clean all build artifacts"
	@echo "  help           - Show this help message"
