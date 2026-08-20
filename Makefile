.PHONY: contracts generate-contracts project-scope verify web

contracts:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace --all-targets --all-features
	cargo test --workspace --doc --all-features
	PYTHONDONTWRITEBYTECODE=1 python3 docs/foundation/verify-versioned-protocol-route-catalog.py --self-test
	PYTHONDONTWRITEBYTECODE=1 python3 docs/foundation/verify-postgresql-release-1-persistence-catalog.py --self-test
	PYTHONDONTWRITEBYTECODE=1 python3 docs/foundation/verify-manuscript-author-edit-batch-policy.py --self-test
	node --test docs/research/author-edit-batch-browser-process.test.mjs
	node docs/research/author-edit-batch-prerelease-browser-harness.mjs
	PYTHONDONTWRITEBYTECODE=1 python3 scripts/verify-stage1-ticket-bindings.py --self-test
	cargo run --quiet -p storyos-contracts -- check
	pnpm --package=typescript@5.9.3 dlx tsc --noEmit --skipLibCheck false --lib es2022,dom --module nodenext --moduleResolution nodenext generated/typescript/storyos-public-release-1/client.d.mts
	$(MAKE) web
web:
	node --test apps/web/test/protocol-boot.test.mjs
	node --test apps/web/test/project-open.test.mjs
	node --test apps/web/test/author-edit-outcome-browser.integration.test.mjs
	node --test apps/web/test/editor-session-browser.integration.test.mjs
	node --test apps/web/test/manual-input-browser.integration.test.mjs
	node --test apps/web/test/acknowledgement-loss-browser.integration.test.mjs
	node --test apps/web/test/takeover-late-result-browser.integration.test.mjs
	node --test apps/web/test/activity-reorder-browser.integration.test.mjs
	node --test apps/web/test/activity-resync-browser.integration.test.mjs
	node --test apps/web/test/reload-recovery-browser.integration.test.mjs
	cargo build --quiet -p storyos-server
	node --test apps/web/test/protocol-http.integration.test.mjs
	$(MAKE) project-scope
project-scope:
	scripts/verify-project-scope.sh
generate-contracts:
	cargo run --quiet -p storyos-contracts -- generate
verify: contracts
	@cargo metadata --no-deps --format-version 1 | python3 scripts/verify-workspace-boundaries.py
	@PYTHONDONTWRITEBYTECODE=1 python3 docs/foundation/verify-manuscript-author-edit-batch-policy.py
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/verify-stage1-ticket-bindings.py
