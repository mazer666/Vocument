.PHONY: help check validate test fmt lint hooks clean

help:
	@echo "Vocument Developer Commands:"
	@echo "  make check     - Run all repository, formatting, lint, and test checks (local-ci.sh)"
	@echo "  make validate  - Validate repository structure, docs, and workflow policies"
	@echo "  make test      - Run Rust workspace unit & integration tests + Swift bridge check"
	@echo "  make fmt       - Format Rust code with cargo fmt"
	@echo "  make lint      - Run cargo clippy with strict warnings"
	@echo "  make hooks     - Install local Git hooks (DCO + pre-push CI)"
	@echo "  make clean     - Clean Cargo and Swift build artifacts"

check:
	./scripts/local-ci.sh

validate:
	./scripts/validate-repo.sh

test:
	cargo test --workspace
	swift test --package-path apps/macos

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace --all-targets -- -D warnings

hooks:
	./scripts/install-hooks.sh

clean:
	cargo clean
	rm -rf apps/macos/.build
