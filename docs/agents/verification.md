# Repository verification

This guide and its reference sections are the single owner of verification commands for
every agent client. The main flow is in this guide. The reference sections are in
[Verification observation](verification-observation.md).

## Daily loop

1. At task start, run `make verify-status BASE=origin/main`. Then do its `nextAction`.
   To refuse a commit message with a text guard finding before the commit exists, run
   `make install-hooks` one time per clone. The hook is optional. The hook refuses the
   message, but `make verify-policy` only reports commit message findings as advisory.

   `make verify-policy` runs the ASD-STE100 text guard on added prose with the
   [rejected-word list](ste-rejected-words.json). It also runs the whitespace check and the
   Rust literal guard on added Rust lines with the [exemption list](rust-literal-exemptions.json).
   The same guard refuses each `tracing` form in `crates/` that can record author text (ADR 0047).

   `make verify-status` shows the change size and the module size as advisory. It also
   shows two prerequisites. `cleanTree` needs a clean worktree for a check that requires
   it. `policyFresh` needs a current passed `verify-policy` result for a complete run.
2. After each product or test edit, run the smallest check that can fail on that edit.
   For one check, use `make verify-targeted CHECK=<check>`. For the full selected scope,
   use `make verify-changed BASE=<base>`. Set `BASE` to the actual comparison commit.
   When a failed test is in the [flake register](verification-observation.md#flake-register), the final line names its
   issue and `main` pass rate. A known flake still fails the run.

   To run one exact-dist journey file with the `project-scope` procedure, use
   `make verify-journey FILE=<file> RUNS=<n> LOAD=<processes>`. `RUNS` and `LOAD` are optional.
3. After a test lifecycle change, run `make verify-policy`. A test lifecycle change
   adds, renames, or deletes a test, or changes the runner or the
   [input policy](verification-policy.json).
4. After `make verify-policy`, inspect a new plan with `make verify-plan BASE=<base>`.
5. Start a long command in the background with the client's own mechanism. The
   command is complete when its process exits and its structured result is
   available. A printed stage line is not the end signal. Heavy stages wait in one host
   queue for all worktrees. `hostQueue` in `verify-status` shows the holder.

The loop is complete when every check that the current sources select has a
current PASS. A pending check is not a PASS.

## Candidate review and admission

The implementation session is the executor. It opens the PR and runs the steps
below. It does not merge. A coordinator session examines the evidence. Then it merges
the PR with an ordinary merge commit.

1. Open the PR. On the clean candidate, run `make review-round PR=<pr> [CONTEXT=<executor context>]`. After the current `verify` succeeds, it writes the request and starts one new read-only Codex plugin thread for each axis with the [review prompt](review-prompt.md). Then it posts the two verdict comments, imports the two records, and prints the next action. When Codex implements, a different agent tool or a separate Claude Code session reviews each axis with the same prompt. The executor posts and imports the same verdicts and records.

   After the two review jobs, the command stops each Codex broker of the worktree and its app server, also when a job fails.

   The command and `verification_reviews.py` retry a transient GitHub API failure. A retry does not post a verdict comment two times. Do not put a `gh` wrapper on `PATH`, because `PATH` is a verification input.
2. If a verdict is `FAIL`, fix the blocking findings, commit, push, and run the command again. A PR gets at most three rounds. The command refuses a fourth round: send the open findings to the coordinator.
   After a round with `PASS` on the two axes, the command refuses a new round while the candidate tree does not change. The verdict comments of the round record its tree. A source fix after that round, for example after a failed `make verify-local`, changes the tree and starts the next round.
3. Run the policy-required targeted checks on current sources. A ticket that requires a complete local run uses `make verify-local BASE=<base-sha> VERIFY_ARGS='--issue <issue> --pr <pr> --executor-context <context> --review-request <path>'` after the imports.

   `make verify-local` is the pre-merge evidence command. Start it after the Standards
   and Spec reviews of the candidate tree, and only one time for that tree. Fix a red
   targeted check with another targeted check.
4. Send the PR link and the verdict comment links to the coordinator.

For a failed complete run, use `python3 scripts/verification.py status --attempt <id> --json` and its recovery command. Recovery needs current reviews and targeted results.
Source fixes return to targeted checks and a new candidate. Retain every attempt.

After merge, synchronize `main` and run `make verify-tracker`. A ticket that requires a post-merge complete run uses a fresh request and `make verify` with `--purpose post-merge-different-tree` and the request's base.
Manual Linux uses `--purpose manual-linux` in request and execution. The workflow accepts
JSON `{"request": <request>, "reviews": {"standards": <record>, "spec": <record>}}` for the selected Git tree.
It imports actual independent reviews and runs fresh targeted checks on Linux. Local source stamps belong to admission; candidate-bound reviews remain portable.

## PR verification and optional complete run

The required GitHub `verify` check validates the pull-request synthetic merge.
Wait for the required GitHub `verify` sentinel and independent Standards and Spec
reviews. Resolve findings with targeted checks and push the corrected candidate.
The `candidate-evidence` status and complete-report publication command are retired.
Do not start a complete local run only to satisfy a PR status. When a ticket requires
complete verification, keep its local report and use the recovery procedure above.

Extend the policy and runner together for a new framework or execution group.
Complete local verification and manual Linux verification retain their stage obligations
when requested.

Selected dirty-tree runs are daily feedback. Complete candidate verification needs
a clean tree because release packaging binds Git identity. An empty change set or
empty test discovery cannot report success. When a complete run is requested,
PostgreSQL fixtures, ordered HTTP groups, exact-dist oracles and both recovery
drills remain mandatory. The PR `verify` sentinel checks the policy and runner.

The `verify` workflow does not run on `edited`.
A review request accepts any successful `verify` run on the head that matches the request base, head, and tree.
`verify-pr` accepts the event base or the current remote base tip, and prints which one.

In a pull-request run, the `ste-text-guard` step of `verify-pr` runs the text guard file mode.
The range starts at the base parent of the synthetic merge and stops at the synthetic merge.
A finding fails the `verify` check. The commit message mode stays local, because the GitHub checkout has a depth of two.

## Parallel implementation

Before starting database or observation commands in parallel worktrees, read
[Parallel resources](parallel-resources.md) for checkout isolation and lifecycle rules.

Use [Issue tracker](issue-tracker.md) for ticket readiness, integration ownership,
and final acceptance. Run each ticket's commands in its own worktree with its own
`target/` outputs and checkout resources. Set `BASE` to the exact integration
commit used by that ticket when selecting its changes. Attribute targeted runs
to the child Issue; attribute aggregate candidate runs to the specification PR.

Evidence belongs to its recorded source identity. A child PASS does not prove the
combined integration tree. After integration, plan against the PR base and run
applicable combined checks. Independent Standards and Spec reviews cover the full
specification diff. Complete local runs remain required when the specification,
ticket, or user requests them; parallel delivery does not create a new full-run gate.
Do not share mutable build directories or reuse another worktree's local admission
records. Coordinate host capacity before simultaneous resource-heavy commands;
checkout locks do not enforce a host-wide budget.

## Agent clients

These commands and rules are the same for every agent client, for example Claude
Code and Codex. Codex loads `AGENTS.md` directly. Claude Code loads it through
`CLAUDE.md`. If a client does not load `AGENTS.md` or `CLAUDE.md`, include this document in its project
instructions. The checked policy is the common owner. Client instructions link
here and do not copy its rules. Each client can own an implementation, review, or
coordinator role. The Standards and Spec reviewers use an agent tool that is
different from the implementer's tool, as
[Candidate review and admission](#candidate-review-and-admission) specifies.

## Reference

The reference sections for the daily loop, a status `nextAction`, a complete run, and local run
observation are in [Verification observation](verification-observation.md#input-inventory-and-complete-run-reports).
