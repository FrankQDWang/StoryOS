.PHONY: contracts generate-contracts project-scope verify verify-local verify-pr verify-tracker web

contracts:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace --all-targets --all-features
	cargo test --workspace --doc --all-features
	PYTHONDONTWRITEBYTECODE=1 python3 docs/foundation/verify-versioned-protocol-route-catalog.py --self-test
	PYTHONDONTWRITEBYTECODE=1 python3 docs/foundation/verify-postgresql-release-1-persistence-catalog.py --self-test
	PYTHONDONTWRITEBYTECODE=1 python3 docs/foundation/verify-manuscript-author-edit-batch-policy.py --self-test
	PYTHONDONTWRITEBYTECODE=1 python3 scripts/verify-stage1-ticket-bindings.py --self-test
	cargo run --quiet -p storyos-contracts -- check
	$(MAKE) web
web:
	pnpm install --frozen-lockfile
	pnpm --dir apps/web run typecheck
	pnpm --dir apps/web exec vite build
	cargo build --quiet -p storyos-server
	pnpm --dir apps/web exec vitest run --project node-contract --project browser-source --project browser-exact-dist
	node --test apps/web/test/production-page-browser.integration.test.mjs
	$(MAKE) project-scope
project-scope:
	scripts/verify-project-scope.sh
generate-contracts:
	cargo run --quiet -p storyos-contracts -- generate
verify-local: contracts
	@cargo metadata --no-deps --format-version 1 | python3 scripts/verify-workspace-boundaries.py
	@PYTHONDONTWRITEBYTECODE=1 python3 docs/foundation/verify-manuscript-author-edit-batch-policy.py

verify-tracker:
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/verify-stage1-ticket-bindings.py

verify-pr:
	@set -eu; \
		if [ -z "$${STORYOS_PR_BASE_SHA:-}" ]; then \
			printf '%s\n' "STORYOS_PR_BASE_SHA is required" >&2; \
			exit 1; \
		fi; \
		if [ -z "$${STORYOS_PR_HEAD_SHA:-}" ]; then \
			printf '%s\n' "STORYOS_PR_HEAD_SHA is required" >&2; \
			exit 1; \
		fi; \
		parents="$$(git rev-list --parents -n 1 HEAD)"; \
		set -- $$parents; \
		if [ "$$#" -ne 3 ]; then \
			printf '%s\n' "The pull request checkout must be a two-parent merge commit" >&2; \
			exit 1; \
		fi; \
		merge="$$1"; \
		base="$$2"; \
		head="$$3"; \
		if [ "$$base" != "$$STORYOS_PR_BASE_SHA" ]; then \
			printf 'Expected base %s but found %s\n' "$$STORYOS_PR_BASE_SHA" "$$base" >&2; \
			exit 1; \
		fi; \
		if [ "$$head" != "$$STORYOS_PR_HEAD_SHA" ]; then \
			printf 'Expected head %s but found %s\n' "$$STORYOS_PR_HEAD_SHA" "$$head" >&2; \
			exit 1; \
		fi; \
		tree="$$(git rev-parse "$$merge^{tree}")"; \
		printf 'Pull request base: %s\nPull request head: %s\nSynthetic merge: %s\nSynthetic merge tree: %s\n' \
			"$$base" "$$head" "$$merge" "$$tree"; \
		git diff --check "$$base" "$$merge" --
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/verify-stage1-ticket-bindings.py --self-test
	@$(MAKE) verify-tracker

verify: verify-local
	@$(MAKE) verify-tracker
