.DEFAULT_GOAL := check

# The three check tiers of ADR 0070. Each tier runs its steps in one shell, which stops at the first failure,
# and scripts/check-evidence.sh prints the Check Evidence Line at the end.
.PHONY: check-fast check check-full
BASE ?= origin/main
# These node-contract files need the release package, so make check runs them after it builds the package.
CHECK_PACKAGE_CONTRACT_FILES = test/node-contract/protocol-http.integration.test.ts test/node-contract/production-build.test.ts
CHECK_FAST_STEPS = cargo fmt --all -- --check; \
	cargo clippy --workspace --all-targets --all-features -- -D warnings; \
	pnpm install --frozen-lockfile; \
	pnpm --dir apps/web run typecheck; \
	pnpm --dir apps/web exec vitest run --project node-contract $(foreach file,$(CHECK_PACKAGE_CONTRACT_FILES),--exclude $(file)); \
	cargo run --quiet --locked -p storyos-contracts -- check; \
	cargo metadata --no-deps --format-version 1 | python3 scripts/verify-workspace-boundaries.py; \
	python3 scripts/verify-transaction-control-receivers.py; \
	cargo run --quiet --locked -p storyos-literal-guard -- --tracing; \
	git diff --check "$$(git merge-base $(BASE) HEAD)" --
CHECK_STEPS = $(CHECK_FAST_STEPS); \
	cargo test --workspace --all-targets --all-features; \
	cargo test --workspace --doc --all-features; \
	pnpm --dir apps/web exec vitest run --project browser-source; \
	python3 scripts/package-release.py; \
	pnpm --dir apps/web exec vitest run --project node-contract $(CHECK_PACKAGE_CONTRACT_FILES); \
	scripts/dev-postgres.sh with sh -exc " \
		cargo test --workspace --all-features --test project_scope -- --ignored --nocapture; \
		cargo test --workspace --all-features --test project_command_challenge -- --ignored --nocapture; \
		cargo test --workspace --all-features --lib -- --ignored --nocapture; \
		python3 scripts/check-phases.py"
# STORYOS_WEB_TYPECHECKED=1 tells the project-scope script that the release package exists.
CHECK_FULL_STEPS = $(CHECK_STEPS); \
	STORYOS_WEB_TYPECHECKED=1 scripts/verify-project-scope.sh

check-fast:
	@scripts/check-evidence.sh $@ sh -exc '$(CHECK_FAST_STEPS)'

check:
	@scripts/check-evidence.sh $@ sh -exc '$(CHECK_STEPS)'

check-full:
	@scripts/check-evidence.sh $@ sh -exc '$(CHECK_FULL_STEPS)'

.PHONY: release-package generate-contracts
# A fresh checkout has no Web dependencies, so the goal installs them and checks the types first.
release-package:
	pnpm install --frozen-lockfile
	pnpm --dir apps/web run typecheck
	python3 scripts/package-release.py

generate-contracts:
	cargo run --quiet -p storyos-contracts -- generate

.PHONY: review
# One Codex review round on a pull request (ADR 0070). WAIT is the seconds to wait for the GitHub verify check.
WAIT ?= 1200
review:
	@python3 scripts/review.py --pr "$(PR)" --wait "$(WAIT)"
