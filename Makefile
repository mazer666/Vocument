.PHONY: help check validate bindings test fmt lint hooks clean

help:
	@echo "Vocument Developer Commands:"
	@echo "  make check     - Run all repo, Rust, UniFFI, Swift (macOS), and Kotlin (Android) checks"
	@echo "  make bindings  - Regenerate Swift and Kotlin UniFFI bindings from Rust DocumentCore"
	@echo "  make validate  - Validate repository structure, docs, and workflow policies"
	@echo "  make test      - Run Rust workspace tests + Swift & Kotlin UniFFI handshakes"
	@echo "  make fmt       - Format Rust code with cargo fmt"
	@echo "  make lint      - Run cargo clippy with strict warnings"
	@echo "  make hooks     - Install local Git hooks (DCO + pre-push CI)"
	@echo "  make clean     - Clean Cargo, Swift, and Kotlin build artifacts"

check:
	./scripts/local-ci.sh

bindings:
	./scripts/generate-bindings.sh

validate:
	./scripts/validate-repo.sh

test:
	cargo test --workspace
	swift run --package-path apps/macos vocument-mac-handshake
	./scripts/verify-kotlin-handshake.sh

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace --all-targets -- -D warnings

hooks:
	./scripts/install-hooks.sh

clean:
	cargo clean
	rm -rf apps/macos/.build
	rm -rf apps/android/.gradle apps/android/app/build
