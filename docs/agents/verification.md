# Repository verification

This guide is the single owner of verification commands for every agent client.
The main flow comes first. The reference sections are after it.

## Daily loop

1. At task start, run `make verify-status BASE=origin/main`. Then do its `nextAction`.
   To refuse a commit message with a text guard finding before the commit exists, run
   `make install-hooks` one time per clone. The hook is optional. The hook refuses the
   message, but `make verify-policy` only reports commit message findings as advisory.

   `make verify-policy` runs the ASD-STE100 text guard on added prose with the
   [rejected-word list](ste-rejected-words.json). It also runs the whitespace check and the
   Rust literal guard on added Rust lines with the [exemption list](rust-literal-exemptions.json).
   `make verify-status` shows the change size and the module size as advisory. It also
   shows two prerequisites. `cleanTree` needs a clean worktree for a check that requires
   it. `policyFresh` needs a current passed `verify-policy` result for a complete run.
2. After each product or test edit, run the smallest check that can fail on that edit.
   For one check, use `make verify-targeted CHECK=<check>`. For the full selected scope,
   use `make verify-changed BASE=<base>`. Set `BASE` to the actual comparison commit.
3. After a test lifecycle change, run `make verify-policy`. A test lifecycle change
   adds, renames, or deletes a test, or changes the runner or the
   [input policy](verification-policy.json).
4. After `make verify-policy`, inspect a new plan with `make verify-plan BASE=<base>`.
5. Start a long command in the background with the client's own mechanism. The
   command is complete when its process exits and its structured result is
   available. A printed stage line is not the end signal.

The loop is complete when every check that the current sources select has a
current PASS. A pending check is not a PASS.

## Candidate review and admission

The implementation session is the executor. It opens the PR and runs the steps
below. It does not merge. A coordinator session examines the evidence. Then it merges
the PR with an ordinary merge commit.

1. Open the PR and wait for current `verify` success. On the clean candidate, run `python3 scripts/verification_reviews.py request --pr <pr> --executor-context <context>`.
2. Start two independent read-only reviews, one for each axis. Each review runs in
   a new context of an agent tool that is different from the implementer's tool.
   Give each reviewer the printed request and the scoped diff `git diff <base>...HEAD`.
   The Standards reviewer compares the diff with `AGENTS.md`, `CODING_STANDARDS.md`,
   `GLOSSARY.md`, and ASD-STE100. The Spec reviewer compares the diff with the
   ticket or task contract.
   Each reviewer returns `PASS` or `FAIL` with `file:line` evidence.

   Each reviewer prompt states these review rules. The request `guards` field gives the
   `ste-text-guard`, `rust-literal-guard`, and `diff-whitespace` results of the candidate.
   A finding for a guard-owned rule is non-blocking: ASD-STE100 words and sentence
   length, positional-literal comments, whitespace, and size. The guards use the
   [rejected-word list](ste-rejected-words.json) and the
   [exemption list](rust-literal-exemptions.json).

   The reviewer reports blocking and non-blocking findings in two separate lists, and
   returns `FAIL` only for a blocking finding. A PR gets at most three review rounds.
   Only a blocking finding starts a new round.

   - Current example when Claude Code implements: the Codex plugin. Use one new
     thread for each axis. Do not use `--write`. The first command finds the newest
     installed plugin version:

     ```bash
     codex_root=$(ls -d ~/.claude/plugins/cache/openai-codex/codex/*/ | sort -V | tail -n 1)
     node "${codex_root}scripts/codex-companion.mjs" task --fresh "<axis prompt>"
     ```

   - When Codex implements, use a different agent tool or a separate Claude Code
     session for each axis.
3. Post each verdict as a PR comment. If a verdict is `FAIL`, fix the findings.
   Commit and push the fix. Then do steps 1 and 2 again. Continue until the two axes
   PASS. After the third round, send the open findings to the coordinator.
4. Write one review record for each axis as JSON with `request_sha256` (the request digest), `axis` (`standards` or `spec`), `reviewer_context` (for example `codex-standards-pr<pr>` or `codex-spec-pr<pr>`), `result` (`PASS` or `FAIL`), and `evidence`. The executor context and the two reviewer contexts must differ. IDs assert consistency, not authenticated identity.
5. Import each record with `python3 scripts/verification_reviews.py import --request <path> --record <review-json>`. The newest retained import per axis governs admission. After review fixes or policy drift, commit and obtain a current request and independent imports.
6. Run the policy-required targeted checks on current sources. A ticket that requires a complete local run uses `make verify-local BASE=<base-sha> VERIFY_ARGS='--issue <issue> --pr <pr> --executor-context <context> --review-request <path>'` after the imports.

   `make verify-local` is the pre-merge evidence command. Start it after the Standards
   and Spec reviews of the candidate tree, and only one time for that tree. Fix a red
   targeted check with another targeted check.
7. Send the PR link and the verdict comment links to the coordinator.

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

For details about the daily loop, a status `nextAction`, or a review, read the
sections below.

## Input inventory and complete-run reports

Use `make verify-policy` to check file ownership and the verification command.
Use `python3 scripts/verification.py inventory` to inspect the input list as JSON.
The [input policy](verification-policy.json) includes tracked files and new files that Git does not ignore. Its
ordered path rules classify each input. Test files require an explicit test rule.
A Cargo group names the existing owning crate. Historical and prototype tests
retain separate classifications; the inventory does not add them to product tests.

`make verify-local` runs the complete existing gate on a clean source tree. It
writes a unique report below `target/verification/` and prints its path. The report
contains the source commit and tree, input inventory, host identity, child commands,
stage results, and elapsed time. Its total is measured directly. Nested stage
durations overlap their parent and must not be added to that total.

A nonzero child result, interruption, incomplete stage, or changed source identity
prevents a successful report. A failed child keeps its failure even if another
command succeeds. Input write stamps detect ordinary writes even when the original
bytes are restored; they are run observations, not reusable cache keys. A report describes local execution within the existing trust
boundary; it is not an independent attestation or a domain Verification Evidence
Bundle. Keep secrets in environment variables, not recorded command arguments.

## Current status and targeted checks

`make verify-status BASE=origin/main` reads the current daily plan and retained
results. It does not run tests or write observations. Use
`python3 scripts/verification_plan.py status --base origin/main` for JSON, or
`make verify-plan` for a readable plan summary. An empty
change set remains pending. A prior result can be passed, failed, or stale.
Package-dependent work on dirty sources has unmet prerequisites.

Status JSON has `prerequisites.cleanTree` and `prerequisites.policyFresh`. Each
one has a `status` (`met`, `unmet`, or `not-required`) and a `reason`. A selected
check that the policy marks `clean`, or that requires the package, needs a clean
tracked and untracked worktree. A complete run needs a current passed
`verify-policy` result. When a prerequisite is unmet, `nextAction.prerequisite`
names it and `agentHint` tells how to meet it. The daily, targeted, and complete
runs check the same prerequisites before the first step. An unmet prerequisite
stops the run with the dirty paths or the refresh command. It writes no step record.

`make verify-targeted CHECK=verify-policy VERIFY_ARGS='--issue 744'` runs a check
registered in the policy. Query it with
`python3 scripts/verification.py status --check verify-policy --json`.
Source bytes and write stamps, policy, test membership, command, execution input
digest, and plan identity bind targeted results. Only a current passed result
can satisfy a current prerequisite. They do not satisfy a separately requested complete run.
The Make test targets use this same entry when called outside a managed run.
Public step, Rust, shared database, package, and recovery script entries create a
root record or inherit the existing run. Direct Cargo, Vitest, or other shell
commands outside these entries remain outside managed observation.

Use `VERIFY_ARGS='--issue 744 --pr 123'` on daily or targeted Make entries for
explicit attribution. Missing attribution stays null. Nested steps retain their
parent step ID and do not create another root. Reports retain actual child start,
UTC and monotonic intervals, process birth identity, and five-second heartbeats.
Request records distinguish refused requests from execution. A cached daily result
has no actual child start. Records remain below ignored `target/verification/`.
Keep secrets in environment variables; do not put them in command arguments,
purpose, trigger, or recovery reason fields.

`python3 scripts/verification.py status --attempt <run-id> --json` reads the
existing complete attempt and prints its next recovery command. Recovery uses the
original failed boundary and its policy-owned preparation. Each recovery has a
separate retained report linked by `recovery_of`; the attempt component still owns
retry admission. A failed recovery cannot clear the original failure. Status
reports distinguish active and lost process identity without changing admission.

## Status output

Status commands return version 2 summaries by default. JSON consumers that need
`plan` or full prerequisites must add `--details`. Text summaries also omit file
reasons and graph membership. Plan JSON and retained reports keep their full format.

Use `decision`, `reasonCode`, `nextAction.argv`, and `agentHint` to select the next
step. `changedInputs` lists changed identity fields, never environment values.
An active process points to observation. A lost process needs cleanup confirmation.
A changed identity requires a fresh plan. Recovery commands still enforce current
admission and require an accurate reason. A current PASS covers only the selected
verification scope; merge checks and reviews retain their separate authority.

Daily status shows `prerequisites.policyFresh`. A stale `verify-policy` result
does not change the daily `nextAction`, because `make verify-changed` runs the
policy group. Only the complete-run status and the `verify-local` preflight make
`policyFresh` the next action.

Daily status also gives two size fields for information only. They do not change
`decision` or `nextAction`. `changeSize` counts the added and changed lines of the
worktree against the merge base with `BASE`. It counts the deleted files and their
lines separately. Its `limits` list gives the limits, and its `above` list gives each
limit that the count is more than.

`moduleSize` lists each changed Rust module that is larger than the lower limit,
without its `#[cfg(test)]` modules and its `_tests.rs` file. Each item has an `above`
list. [CODING_STANDARDS.md](../../CODING_STANDARDS.md#change-size) owns the size rules. The text summary shows these fields after the decision
and shows at most eight modules.

### Bounded daily queries

`make verify-plan` now calls `verification_plan.py summary --format text`.
`VERIFY_ARGS='--format json'` returns the same summary as JSON. Daily status and
plan summaries have a 16 KiB and 80-line limit, eight checks per page, and clipped
text fields with original character counts. Failures and pending checks come first.
Counts cover all checks; equal blocking reasons share a count. Query errors also
use a bounded summary; `--details` retains the full error. Partial readiness
does not satisfy pending checks. Guidance does not authorize execution.

Use `make verify-plan VERIFY_ARGS='--select blocked --page 2'` for another page.
Use `VERIFY_ARGS='--index 3 --details'` for the exact current check, including all
reasons and members. Indexes belong to the displayed plan digest; refresh after
source changes. `--details` gives full status/plan facts without a size limit.
`python3 scripts/verification_plan.py plan --format json --base origin/main`
retains the full version 1 plan for program consumers. Redirect this explicit
export to `target/plan.json` when a saved plan is needed. Full graph exports use
`plan --profile complete`. The runner still recomputes and rejects stale inputs.
Queries do not create files, run tests, or change retained evidence.

## Daily file selection

Use `make verify-plan BASE=origin/main` to inspect a bounded daily summary.
Use `make verify-changed BASE=origin/main` to execute that scope. Set BASE to the
actual comparison commit or ref; `BASE=HEAD` checks current working changes only.
The daily entry never dispatches complete verification. Each check is ready or pending.
Ready checks execute on dirty sources. When a selected check requires the package,
a dirty tree stops the run before the first step. A pending result returns exit code 2
and cannot publish a cache entry.
Exact-dist, recovery, and unresolved scopes retain explicit pending obligations.
Save plans under ignored `target/` and run a saved plan with
`python3 scripts/verification_plan.py run --base origin/main --plan target/plan.json`.
The runner recomputes the plan and refuses stale or edited plans. Reports bind
current file bytes, index entries, source write stamps, test membership and plan digest.

New tests in supported locations join discovery automatically. File execution is
an explicit opt-in: copy the applicable `file_profiles` declaration from the input
policy to the first line only after reviewing all imports, file reads and environment
needs. This profile permits repository inputs and the locked test toolchain only;
live services, mutable shared fixtures, release packages and ignored build outputs
require the complete group. A changed dependency invalidates that declaration.
Rust selections batch current owners and reverse consumers with all features. Prior
ownership and dependencies retain affected groups after moves or deletions. Database
consumers retain the workspace feature profile. Compiler records must cover selected
Rust test files. Unknown ownership fails with a policy action.

The policy declares cross-language consumers. Plans list effective groups, files,
preparation, shared phases, reasons, and an unknown estimate when no comparable
sample exists. Shared database execution prepares one controlled fixture, then runs
only selected groups with existing phase order and resets. It does not start the
SQL activation oracles, exact-dist journeys, or recovery drills. Those complete
proof obligations remain unchanged. Locked Web preparation and package creation each
run at most once. Package consumers do not extend the isolated Node cache profile.

When adding, renaming or deleting tests, run `make verify-policy` and inspect a fresh
plan. Renames and deletions retain prior groups and remove obsolete
files from current test membership. Unknown locations or execution profiles fail.
A new framework or shared resource requires a reviewed policy and runner extension,
with a public command regression. Review declarations with the same independent
Standards and Spec process as code. Test names and counts are discovered, not fixed.


## Retained workflow graphs

Daily and targeted plans include a version 1 `graph`. Complete reports retain the
same graph beside `plan` for local diagnosis. Use
`python3 scripts/verification_plan.py plan --profile complete` to read the committed HEAD
workflow without execution. Daily exports read current working inputs.

The policy owns operation types, preparation requirements, and check membership.
The exporter combines these definitions with discovered files and shared phases.
Each graph binds source, policy, file membership, and plan identity. Stable file
IDs contain the check profile and repository-relative path. A rename removes the
old ID and adds a new ID; snapshots do not infer a rename from similar bytes.
Missing historical graphs remain unknown. No old report is changed.
Local reports retain full graph snapshots. They are diagnostic records, not a PR
publication gate.

`dependencies` run from prerequisite to consumer. Edges marked `both-selected`
retain phase order without selecting an excluded phase. `relations` distinguish
containment from file membership. A file is a whole selection unit, never a test
name filter. Membership does not claim separate execution, success, or duration.
Nodes mark selection and distinguish checks, aggregates, files, builds, setup,
resets, and cleanup. Existing plan checks retain pending reasons. Graph export
validates identities, edges, cycles, and file coverage before execution. It does
not schedule work or change the existing execution order.

## Shared Web test phases

Shared HTTP and process-cut tests require one first-line JSON declaration:

```typescript
// Verification: {"phase":"http-main","after":[]}
```

`after` contains repository-relative paths in the same phase. Add only required
dependencies. Independent ready files run in sorted path order. The policy owns
the serial phase order, test project, report stage, and preparation action. File
declarations cannot reorder phases or request resources. The current phases share
one prepared PostgreSQL fixture; challenge resets and fixture reloads stay at their
declared boundaries. The exact-dist preparation remains outside these phases.

Use `python3 scripts/verification_shared.py plan` to inspect ordered members.
The complete runner consumes this discovery through the `run` action after its
existing package and database preparation. New supported shared tests join their
declared phase automatically. Remove or update dependent declarations when moving,
renaming or deleting a file. Missing or duplicate declarations, unknown phases,
dangling or cross-phase dependencies, cycles, and empty phases fail policy checks
before expensive children start. `make verify-policy` and the project input check
validate this structure. Independent policy self-tests also precede Rust compilation.

## Verification-tool self-tests

`verify-policy` discovers each current `scripts/*_tests.py` file and runs the
whole file once. An uncommitted or untracked test file stops this step before
the self-tests run, and the message tells the author to commit it. The policy lists files approved for overlap and caps workers at
two. A new or undeclared file runs serially. A missing file, invalid declaration,
or empty selection fails. The public serial diagnostic command is
`STORYOS_VERIFICATION_TEST_WORKERS=1 make verify-targeted CHECK=verification-tests`.
Use an unset worker override for candidate targeted and complete verification so
their execution input digests match.

The approved files use disposable Git repositories or temporary databases and
paths. Local HTTP fixtures bind port zero. Mock Docker and package commands write
inside their fixture repository. The runner gives each file a separate temporary
directory and process group, removes inherited `CARGO_TARGET_DIR` and cache root
from file tests, and records one node attempt per file under the root run. A file
with shared product database state or a fixed port stays serial. Interrupted
workers are signalled and reaped before the root attempt ends. The serialized
diagnostic command does not replace the normal targeted admission result.

## Daily result reuse and host budget

The reviewed Node profile caches passed policy checks, Web preparation and type
checks, and selected Node tests as one group. A hit records a `cached` step and its
producer report. Cargo groups always execute. Complete candidate reuse follows the admission rules below. Selected Vitest runs disable result-cache writes.

The key binds current non-ignored input bytes, modes and link targets; test-file membership;
checks and workers; runners and toolchains; host identity; and an environment digest.
An unrelated file edit can miss, but a new Git SHA alone does not. Environment values stay private.
Reuse requires the original complete successful report, its digest and Vitest output,
and equal installed Node dependency trees, including modes and write stamps. Dependency
identity must stay equal from prepared test start through report completion. The
identity skips the directory names in the policy `dependency_ignore` list, for example
the Vite cache. The `.pnpm` store stays in the identity. Missing,
changed or corrupt output causes execution. The Web workspace link binds to repository
inputs; other external links disable reuse. Failed, interrupted, incomplete or
source-changing runs cannot publish reusable results.

Use `make verify-changed BASE=HEAD VERIFY_ARGS=--no-cache` to force execution without
reading or publishing a result-cache entry. Local entries in `target/verification-cache/`
need their referenced reports. A cache hit is daily feedback, not a PR check.

The complete and daily run commands admit one root run per checkout at a time. A busy
budget fails with a retry reason. The lock covers process-group cleanup; overdue
descendants are terminated and fail the run. The input policy caps daily Cargo build
jobs, Rust test threads and Vitest workers. Use `VERIFY_ARGS='--workers 1'` to lower
that cap. Daily groups stay serial. A requested complete run admits one bounded pair:
`foundation-tests` and `project-scope` after `release-package`. The policy declares
their read and write resources, two-worker limit, six of eight allowed CPU slots, and
12 of 16 GiB allowed memory. The host needs at least eight CPUs and 16 GiB physical
memory. A dependency between the pair, resource conflict, or insufficient host
budget restores the serial order. An unhandled prerequisite refuses the pair before
either stage starts. The release package is read only during both stages. Foundation
Vitest uses its own Chrome profile and scratch state; Project Scope owns PostgreSQL,
Cargo output, exact-dist Chrome, and recovery copies. Project Scope retains its
internal PostgreSQL phase order, fixture resets, migration, exact-dist, and both
recovery proofs. Failure lets an already-started independent stage finish and record
its cleanup; cancellation signals both owned process groups. Reports retain stage
attempt intervals, so concurrent cost must use their union rather than their sum.
`make verify-targeted CHECK=web-overlap` runs the same pair with overlap for a
scope-matched diagnostic comparison; it does not start a complete run. The scheduler
calls the existing Make stage targets and skips their already-passed package prerequisite.
For an explicitly requested complete serial comparison on the same source tree,
set `STORYOS_VERIFICATION_COMPARE=serial` on `make verify-local`; the default
complete entry still uses bounded overlap when the resource budget allows it.

## Rust build-cache generations

Repository verification commands and `make generate-contracts` use one owned Cargo
target directory at a time. The first managed directory is the measured
`target/issue-763-workset`; new directories live under `target/rust-cache/`.
An existing measured directory needs its owner marker before adoption.
`python3 scripts/verification_rust_cache.py status` shows the active generation,
logical and allocated bytes, task scratch, high-water mark, and admission limit.
The policy sets 12 GiB for the active high-water mark and 20 GiB for the active
plus task-scratch limit. These are measured engineering limits, not a promise
about future workspace size. A new generation stays in warmup until a passing
complete run establishes its full build set. An over-limit set refuses the next
managed run with a budget action instead of rotating repeatedly.

The host budget covers managed builds and retirement. The runner records the
generation and target path in targeted and complete execution identity. Only an
inactive generation with a matching owner marker can enter the persisted retired
list. Cleanup moves it to a fixed quarantine path and removes that whole
directory under the budget lock. A stopped cleanup resumes from that list.
Unknown top-level content or links stop cleanup. The active directory is never
cleaned in place. `cargo clean` is not used because it can remove Cargo lock-file
paths while another process still holds those file descriptors.

Direct shell `cargo` commands are outside managed admission. Do not point an
external `CARGO_TARGET_DIR` at an owned generation. The one-time historical
`target/debug` migration completed in #791 and its automatic code is retired.
Managed entries now leave any later default `target/debug` untouched. The local
`target/verification/legacy-rust-cache.json` record remains historical evidence;
managed entries do not rewrite or remove it. Managed generation cleanup still
uses owned whole-directory boundaries and does not remove source, user data,
release packages, verification reports, observation data, or unrelated target output.

When tests or dependencies change, inspect the new plan and report. Extend a cache
profile only after specifying its inputs, required outputs and resource ownership,
adding public CLI invalidation tests, and obtaining independent Standards and Spec
review. New frameworks remain ineligible until the policy and runner support them.

## Complete attempt admission

`make verify-local BASE=origin/main VERIFY_ARGS='--issue 746 --pr 123'` records
explicit attribution. Omit unknown identities. Optional `--purpose` and `--trigger`
record invocation context. Source, base, policy, membership, command, tool bytes
and versions (including Chrome), runners, host, and an environment digest bind
admission. Environment values and inherited Make orchestration flags are excluded.
An active duplicate returns its run identity. A valid matching success returns
its original report without another child. Older reports lack reuse authority.

An unchanged failure refuses a complete retry. Run
`python3 scripts/verification.py recover --attempt <run-id> --reason '<reason>'`.
Recovery executes the failed owning stage with policy-declared preparation.
Interruption or process loss requires child cleanup; a known launch infrastructure
fault also requires an available executable. Successful recovery binds the original
report and unchanged source. Invalid evidence needs correction at its owner.

Local records under `target/verification/` retain requests, refusals, attempts,
reuse, recovery reasons, process identity, and UTC/monotonic timing. Preflight
refusal consumes no attempt. These are not product domain records. The readiness
boundary checks clean source, input ownership, current targeted results, and independent review imports.
Independent Standards and Spec reviews remain required before merging a PR.

## Execution time and resources

The runner applies `stage_budgets_seconds` from the input policy. A stage over its
budget stops its child process group, waits for cleanup, and returns 124 even if
the child handles termination with exit zero. Its record retains `budget_seconds`
and `budget_exceeded`; observation reports `stage-budget-exceeded`. The HTTP
stage has a 15-minute ceiling; each recovery drill has a five-minute ceiling.
These are stop limits, not expected durations or performance acceptance evidence.

Ordinary business tests use `withChallengeBudget(projectId, action)` to prepare
only that Project's operational Challenge quota before a command. The helper does
not sleep or retry errors. Dedicated admission tests use the original interfaces
and controlled clocks. Production limits and nonce checks do not change.
Recovery Acceptance preparation selects only the seven scenarios that create its
six retained conditions and refusal evidence. Ordinary verification still runs
all business cases. Shared phases retain per-file Vitest JSON results in complete
and daily runs. Use the command-owned database described in
[Parallel resources](parallel-resources.md) for targeted measurements.

Managed execution and status output include `supervision`. It separates collector
health from exact report receipt (`current`, `behind`, or `pending`). The query
service returns the collected source digest and runtime comparison facts. A PASS
with unavailable supervision remains a test PASS, with an explicit observation
repair action. Observation cannot admit, reject, retry, or rewrite verification.
Missing build state, candidate facts, and comparable samples remain explicit;
unknown timing never means no regression. Grafana availability does not gate tests.

## Local run observation

The optional Grafana and SQLite observation service is described in
[Verification observation](verification-observation.md). Observation never admits,
rejects, retries, or rewrites verification, and Grafana availability does not gate
tests. Read that guide before you start, stop, or rebuild the service, or change
the collector or dashboards.
