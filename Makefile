.PHONY: contracts generate-contracts verify web

contracts:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace --all-targets --all-features
	cargo test --workspace --doc --all-features
	PYTHONDONTWRITEBYTECODE=1 python3 docs/foundation/verify-versioned-protocol-route-catalog.py --self-test
	PYTHONDONTWRITEBYTECODE=1 python3 docs/foundation/verify-postgresql-release-1-persistence-catalog.py --self-test
	PYTHONDONTWRITEBYTECODE=1 python3 scripts/verify-stage1-ticket-bindings.py --self-test
	cargo run --quiet -p storyos-contracts -- check
	pnpm --package=typescript@5.9.3 dlx tsc --noEmit --skipLibCheck false --lib es2022,dom --module nodenext --moduleResolution nodenext generated/typescript/storyos-public-release-1/client.d.mts
	$(MAKE) web
web:
	node --test apps/web/test/protocol-boot.test.mjs
	cargo build --quiet -p storyos-server
	node --test apps/web/test/protocol-http.integration.test.mjs
generate-contracts:
	cargo run --quiet -p storyos-contracts -- generate
verify: contracts
	@cargo metadata --no-deps --format-version 1 | python3 scripts/verify-workspace-boundaries.py
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/verify-stage1-ticket-bindings.py
