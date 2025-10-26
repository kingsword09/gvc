.PHONY: help build test fmt lint clean install release

help:
	@echo "Available targets:"
	@echo "  build    - Build the project in debug mode"
	@echo "  release  - Build the project in release mode"
	@echo "  test     - Run all tests"
	@echo "  fmt      - Format code with rustfmt"
	@echo "  lint     - Run clippy linter"
	@echo "  clean    - Clean build artifacts"
	@echo "  install  - Install the binary locally"
	@echo "  check    - Run all checks (fmt, lint, test)"

build:
	cargo build

release:
	cargo build --release

test:
	cargo test --all-features

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

clean:
	cargo clean

install:
	cargo install --path .

check: fmt-check lint test
	@echo "All checks passed!"

# Development helpers
watch:
	cargo watch -x build

watch-test:
	cargo watch -x test

run-check:
	cargo run -- check

run-list:
	cargo run -- list

run-update:
	cargo run -- update --no-git

# Documentation
doc:
	cargo doc --no-deps --open

doc-all:
	cargo doc --all-features --open

# Benchmarks (if added in the future)
bench:
	cargo bench

# Coverage (requires cargo-tarpaulin)
coverage:
	cargo tarpaulin --all-features --workspace --timeout 120 --out Html

# Update dependencies
update-deps:
	cargo update
