---
status: accepted
---

# Verify Locally With One Check Command and Keep GitHub Checks Cheap

The repository cleanup specification of 2026-10-22 owns this ADR. Its first ticket adds the commands that this ADR names.

## Context

On 2026-10-22 the author examined the verification layers of the repository. The required GitHub check `verify` runs no product build and no product test. It runs the text guard, the self-tests of the verification tools, and approximately 20 GitHub API calls. The product checks run only on the author's machine.

A Python layer of approximately 5,700 lines decides if a local result counts. It hashes each tracked file and the environment, caches results, and admits candidates. Its own tests add 5,100 lines. An observation stack of approximately 10,000 lines records the cost of verification. It has recorded no run since 2026-10-06.

Two constraints stay. The GitHub plan is free, and its runner is slow. The author will not rent a server or host a runner. Heavy product tests stay on the author's machine.

## Decision

- **One check command in three tiers.** No recipe calls Make a second time, and no recipe removes environment variables.
  - `make check-fast` runs `cargo fmt --check`, Clippy, the web typecheck, the `node-contract` Vitest project, and the generated-contract drift check. It runs no contract verifier from the foundation documents. [ADR 0074](0074-generate-the-route-catalog-from-the-operation-registry.md) removed the route catalog verifier. Repository cleanup phase 5 ([#1344](https://github.com/FrankQDWang/StoryOS/issues/1344)) removed the persistence catalog verifier and the Author Edit batch policy verifier. The migration digest check of the PostgreSQL adapter build script is the proof of the persistence catalog. Then it runs the workspace boundary verifier, the transaction receiver verifier, and the tracing check of the literal guard. It ends with `git diff --check` against the base.
  - Two `node-contract` files need the release package: `protocol-http.integration.test.ts` and `production-build.test.ts`. `make check-fast` does not run them. `make check` runs them after it builds the package.
  - `make check` runs the `check-fast` steps, then `cargo test`, the doc tests, the `browser-source` Vitest project, and the release package. With the development PostgreSQL, it then runs the ignored PostgreSQL tests and the `node-postgresql` and `node-process-cut` phases. It is the default Make goal.
  - `make check-full` runs the `check` steps and then the project-scope run: the packaged Server and Worker checks, exact-dist, and the recovery drill.
- **Clean tree.** `make check` and `make check-full` stop at the start when the working tree has changes, and tell the implementer to commit first. `make check-fast` runs on a working tree with changes.
- **Check Evidence Line.** Each tier prints one line at the end: the tier, the HEAD commit, the tree hash, the result, and the time. Only the line of `make check` or `make check-full` is merge evidence. The line of `make check-fast` is not. The implementer puts the line in the pull request. The reviewer compares its commit with the pull request head. No other local record exists.
- **GitHub check.** The `verify` workflow runs `make check-fast`. Clippy stays in it only while the run stays under 10 minutes with the Actions cache. The workflow runs no tooling self-test and no tracker API call.
- **Merge gate.** A pull request merges when three conditions are true. The GitHub check passes. A Check Evidence Line of `make check` or `make check-full` names the pull request head. The Codex review passes.
- **Codex review.** The review stays, with two axes and at most three rounds. One script of approximately 300 lines sends the request, waits, posts the verdict, counts the rounds, and retries GitHub network errors. Only a blocking finding fails a round.
- **Removed layers.** The verification Python family and its tests are removed. With them go the result cache, the candidate admission, the daily result reuse, and the host queue. The review record import, the flake register, and the tracker SHA binding are removed. The observation stack and the Grafana configuration are removed. The tag `evidence/pre-cleanup-2026-10` on the last `main` before the removal keeps them.
- **Text guard.** The ASD-STE100 rule stays as one sentence in `AGENTS.md`. The text guard script, its word list, the commit hook, and the CI check are removed. A reviewer does not block on wording.
- **Literal guard.** The guard keeps only its tracing check: a `tracing` call must not record author text. It runs in `make check-fast`. The `/*name*/` comment on a positional literal is no longer required.
- **Change size.** The limits stay at 800 added lines, and at 500 added lines for complex logic. Deleted lines and mechanical changes do not count. A ticket that must add more than 800 lines declares the count and the reason in its body. The author approves it with the breakdown. A reviewer judges the size, and no script reports it.
- **Delivery document.** One document, `docs/agents/delivery.md`, holds the delivery rules in at most 3,000 words. It replaces `executor-protocol.md`, the delivery part of `issue-tracker.md`, `verification.md`, `verification-observation.md`, `verification-policy.json`, `flaky-tests.json`, and `parallel-resources.md`.

## Replaced rules

- ADR 0037 kept Python and Make as the verification executors with input fingerprints, complete-run admission, and a Grafana observation stack. This ADR supersedes ADR 0037.
- ADR 0012 does not change. The Deterministic Verification Oracle, the Contract-Faithful Fake Destination, the Deterministic Test Scheduler, and the Verification Evidence Bundle are product concepts. The removed tooling did not implement them.

## Considered options

- **Run the product tests on GitHub.** Rejected: the free runner is slow, and a PostgreSQL job for the full suite exceeds its budget. The author will not host a runner.
- **Keep the fingerprint layer and remove only the observation stack.** Rejected: the layer defends against an agent that reuses a stale result. The Check Evidence Line gives the same binding to a tree with approximately 30 lines.
- **Remove the Codex review with its tooling.** Rejected: the author finds the review useful. Only its tooling is heavy.

## Consequences

- Approximately 29,000 lines of agent-supervision tooling go away. Approximately 2,000 lines of product check commands stay.
- The GitHub check cannot catch a product test failure. The Check Evidence Line and the review are the only evidence of a local run. This is the same trust as before, with less machinery.
- The first ticket of the cleanup adds this ADR, the three check tiers, the evidence script, and the new workflow. The removal tickets start after it merges into the integration branch.
