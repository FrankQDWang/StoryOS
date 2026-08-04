.PHONY: contracts generate-contracts verify

contracts:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace --all-targets --all-features
	PYTHONDONTWRITEBYTECODE=1 python3 docs/foundation/verify-versioned-protocol-route-catalog.py --self-test
	PYTHONDONTWRITEBYTECODE=1 python3 docs/foundation/verify-postgresql-release-1-persistence-catalog.py --self-test
	cargo run --quiet -p storyos-contracts -- check
generate-contracts:
	cargo run --quiet -p storyos-contracts -- generate
verify: contracts
	@cargo metadata --no-deps --format-version 1 | python3 scripts/verify-workspace-boundaries.py
