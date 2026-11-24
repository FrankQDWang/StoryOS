.DEFAULT_GOAL := check

# The three check tiers of ADR 0070. The make check-fast steps run in one shell, which stops at the first failure.
# Then scripts/check-lanes.py runs the heavy steps as lanes at the same time, and a failed lane stops the other
# lanes. A lane with names after the colon starts when those lanes passed. scripts/check-evidence.sh prints the
# Check Evidence Line at the end.
.PHONY: check-fast check check-full
BASE ?= origin/main
# These node-contract files need the release package, so make check runs them after it builds the package.
CHECK_PACKAGE_CONTRACT_FILES = test/node-contract/protocol-http.integration.test.ts test/node-contract/production-build.test.ts
CHECK_FAST_STEPS = scripts/verify-web-source-guards.sh; \
	cargo fmt --all -- --check; \
	cargo clippy --workspace --all-targets --all-features -- -D warnings; \
	pnpm install --frozen-lockfile; \
	pnpm --dir apps/web run typecheck; \
	pnpm --dir apps/web exec vitest run --project node-contract $(foreach file,$(CHECK_PACKAGE_CONTRACT_FILES),--exclude $(file)); \
	cargo run --quiet --locked -p storyos-contracts -- check; \
	cargo metadata --no-deps --format-version 1 | python3 scripts/verify-workspace-boundaries.py; \
	python3 scripts/verify-transaction-control-receivers.py; \
	cargo run --quiet --locked -p storyos-literal-guard -- --tracing; \
	git diff --check "$$(git merge-base $(BASE) HEAD)" --
CHECK_LANES = \
	--lane "lanes-tests=python3 scripts/check-lanes-tests.py" \
	--lane "package=python3 scripts/package-release.py; \
		pnpm --dir apps/web exec vitest run --project node-contract $(CHECK_PACKAGE_CONTRACT_FILES)" \
	--lane "build=cargo test --workspace --all-targets --all-features; cargo test --workspace --doc --all-features" \
	--browser-source \
	--lane "phases:package=scripts/dev-postgres.sh with python3 scripts/check-phases.py" \
	--lane "postgres-project-scope:build,package=scripts/dev-postgres.sh with \
		cargo test --workspace --all-features --test project_scope -- --ignored --nocapture" \
	--lane "postgres-command-challenge:build,package=scripts/dev-postgres.sh with \
		cargo test --workspace --all-features --test project_command_challenge -- --ignored --nocapture" \
	--postgres-shards build,package
# The GitHub verify check runs only the make check-fast steps, and ADR 0070 keeps tooling self-tests out of it.
CHECK_STEPS = $(CHECK_FAST_STEPS); \
	python3 scripts/review-tests.py; \
	python3 scripts/check-linux-package-tests.py; \
	python3 scripts/check-lanes.py $(CHECK_LANES)
# make check-full runs the lanes of make check and the project-scope lanes in one run of the lane runner.
CHECK_FULL_STEPS = $(CHECK_FAST_STEPS); \
	python3 scripts/review-tests.py; \
	python3 scripts/check-linux-package-tests.py; \
	python3 scripts/check-lanes.py $(CHECK_LANES) --project-scope

check-fast:
	@scripts/check-evidence.sh $@ sh -exc '$(CHECK_FAST_STEPS)'

# A change that has only documents that no code names runs only the check-fast steps (scripts/check-scope.py).
check:
	@if [ "$$(python3 scripts/check-scope.py $(BASE))" = documents ]; then \
		echo "The change has only documents that no code names, so make check runs only the make check-fast steps."; \
		scripts/check-evidence.sh $@ --scope documents sh -exc '$(CHECK_FAST_STEPS)'; \
	else \
		scripts/check-evidence.sh $@ sh -exc '$(CHECK_STEPS)'; \
	fi

check-full:
	@scripts/check-evidence.sh $@ sh -exc '$(CHECK_FULL_STEPS)'

.PHONY: release-package release-package-linux generate-contracts
# A fresh checkout has no Web dependencies, so the goal installs them and checks the types first.
release-package:
	pnpm install --frozen-lockfile
	pnpm --dir apps/web run typecheck
	python3 scripts/package-release.py

# The same package for Linux x86_64 (ADR 0104 section 6). The Rust build and the checks run in containers.
# It is not a step of make check or make check-full, because the build needs many minutes.
release-package-linux:
	pnpm install --frozen-lockfile
	pnpm --dir apps/web run typecheck
	python3 scripts/package-release.py --target linux-x86_64

generate-contracts:
	cargo run --quiet -p storyos-contracts -- generate

.PHONY: verify-personal-instance
# Runs the scripts of deploy/personal-instance/ in an Ubuntu 24.04 container with the Linux package of the commit
# (issue 1855). It needs the network for the packages of Caddy and of the PostgreSQL client, so it is not a step of
# make check-full.
verify-personal-instance: release-package-linux
	sh scripts/verify-personal-instance.sh

.PHONY: generate-persistence-catalog
# Rewrites generated/persistence-catalog/ from a new database that the committed migrations activate (ADR 0079).
# The goal packages the release first, so commit the migration change before you run it.
generate-persistence-catalog: release-package
	scripts/dev-postgres.sh with env STORYOS_UPDATE_GENERATED=1 cargo test --locked -p storyos-adapter-postgres --all-features --lib -- --ignored --exact persistence_catalog_tests::the_committed_persistence_catalog_matches_the_activated_schema

.PHONY: review
# One Codex review round on a pull request (ADR 0070). WAIT is the seconds to wait for the GitHub verify check.
WAIT ?= 1200
review:
	@python3 scripts/review.py --pr "$(PR)" --wait "$(WAIT)"
