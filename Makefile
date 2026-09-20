.PHONY: contracts generate-contracts project-scope release-package verify verify-local verify-local-steps verify-policy verify-plan verify-changed verify-pr verify-tracker web web-foundation web-typecheck

VERIFY_STEP = PYTHONDONTWRITEBYTECODE=1 python3 scripts/verification.py step
BASE ?= origin/main
VERIFY_ARGS ?=
PR ?=
REPORT ?=

.PHONY: verify-evidence
verify-evidence:
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/verification_evidence.py publish --pr "$(PR)" --report "$(REPORT)" $(VERIFY_ARGS)

verify-plan:
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/verification_plan.py plan --base "$(BASE)" $(VERIFY_ARGS)

verify-changed:
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/verification_plan.py run --base "$(BASE)" $(VERIFY_ARGS)

verify-policy:
	$(VERIFY_STEP) input-ownership -- python3 scripts/verification.py inventory --check
	$(VERIFY_STEP) project-inputs -- scripts/verify-project-scope.sh --check-inputs
	$(VERIFY_STEP) verification-tests -- python3 -m unittest discover -s scripts -p '*_tests.py'

contracts: verify-policy
	$(VERIFY_STEP) protocol-self-test -- python3 docs/foundation/verify-versioned-protocol-route-catalog.py --self-test
	$(VERIFY_STEP) persistence-self-test -- python3 docs/foundation/verify-postgresql-release-1-persistence-catalog.py --self-test
	$(VERIFY_STEP) author-edit-self-test -- python3 docs/foundation/verify-manuscript-author-edit-batch-policy.py --self-test
	$(VERIFY_STEP) tracker-self-test -- python3 scripts/verify-stage1-ticket-bindings.py --self-test
	$(VERIFY_STEP) transaction-self-test -- python3 scripts/verify-transaction-control-receivers.py --self-test
	$(VERIFY_STEP) rust-format -- cargo fmt --all -- --check
	$(VERIFY_STEP) rust-clippy -- cargo clippy --workspace --all-targets --all-features -- -D warnings
	$(VERIFY_STEP) rust-tests -- python3 scripts/verification.py rust-tests
	$(VERIFY_STEP) rust-doc-tests -- cargo test --workspace --doc --all-features
	$(VERIFY_STEP) generated-contracts -- cargo run --quiet -p storyos-contracts -- check
	$(MAKE) web
web-typecheck:
	@python3 scripts/verification_shared.py plan >/dev/null
	$(VERIFY_STEP) node-install -- pnpm install --frozen-lockfile
	$(VERIFY_STEP) web-typecheck -- pnpm --dir apps/web run typecheck

release-package: web-typecheck
	$(VERIFY_STEP) release-package -- python3 scripts/package-release.py

web-foundation: release-package
	$(VERIFY_STEP) foundation-tests -- pnpm --dir apps/web exec vitest run --project node-contract --project browser-source

web: web-foundation
	STORYOS_WEB_TYPECHECKED=1 $(VERIFY_STEP) project-scope -- scripts/verify-project-scope.sh

project-scope: release-package
	STORYOS_WEB_TYPECHECKED=1 $(VERIFY_STEP) project-scope -- scripts/verify-project-scope.sh
generate-contracts:
	cargo run --quiet -p storyos-contracts -- generate
verify-local:
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/verification.py run -- make verify-local-steps

verify-local-steps: contracts
	@$(VERIFY_STEP) workspace-boundaries -- sh -c 'cargo metadata --no-deps --format-version 1 | python3 scripts/verify-workspace-boundaries.py'
	@$(VERIFY_STEP) author-edit-policy -- python3 docs/foundation/verify-manuscript-author-edit-batch-policy.py
	@$(VERIFY_STEP) transaction-guards -- python3 scripts/verify-transaction-control-receivers.py

verify-tracker:
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/verify-stage1-ticket-bindings.py

verify-pr: verify-policy
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
