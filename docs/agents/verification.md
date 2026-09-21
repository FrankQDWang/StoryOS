# Repository verification

Start with `make verify-status BASE=origin/main`; use `make verify-changed` after edits.
For test lifecycle changes, run `make verify-policy` and inspect a new plan.

## Candidate review and admission

1. Open the PR and wait for current `verify` success. On the clean candidate, run `python3 scripts/verification_reviews.py request --pr <pr> --executor-context <context>`.
2. Give the printed request and scoped diff to separate Standards and Spec reviewers. Each returns JSON with `request_sha256` (the request digest), `axis` (`standards` or `spec`), `reviewer_context`, `result` (`PASS` or `FAIL`), and `evidence`. All three contexts must differ; IDs assert consistency, not authenticated identity.
3. Import each record with `python3 scripts/verification_reviews.py import --request <path> --record <review-json>`. The newest retained import per axis governs admission. After review fixes or policy drift, commit and obtain a current request and independent imports.
4. Run the policy-required targeted checks on current sources in the same environment. Run `make verify-local BASE=<base-sha> VERIFY_ARGS='--issue <issue> --pr <pr> --executor-context <context> --review-request <path>'` once, then follow evidence publication below.

For a failed complete run, use `python3 scripts/verification.py status --attempt <id> --json` and its recovery command. Recovery needs current reviews and targeted results.
Source fixes return to targeted checks and a new candidate. Retain every attempt.

Equal merged trees use `make verify-tracker` only. Different trees need a fresh request and `make verify` with `--purpose post-merge-different-tree` and the request's base.
Manual Linux uses `--purpose manual-linux` in request and execution. The workflow accepts
JSON `{"request": <request>, "reviews": {"standards": <record>, "spec": <record>}}` for the selected Git tree.
It imports actual independent reviews and runs fresh targeted checks on Linux. Local source stamps belong to admission; candidate-bound reviews remain portable.

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

## Daily file selection

Use `make verify-plan BASE=origin/main` to inspect the changed files and reasons.
Use `make verify-changed BASE=origin/main` to execute that scope. Set BASE to the
actual comparison commit or ref; `BASE=HEAD` checks current working changes only.
The daily entry never dispatches complete verification. Each check is ready or pending.
Ready checks execute on dirty sources. Package consumers stay pending until sources
are clean. A pending result returns exit code 2 and cannot publish a cache entry.
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

Use these commands from Codex, Cursor and Grok Build. If a client does not load
AGENTS.md, include this document in its project instructions. The checked policy
is the common owner; client instructions link here. Selected dirty-tree runs are
daily feedback. Complete candidate verification needs a clean tree because release packaging binds
Git identity. An empty change set or empty test discovery cannot report success.
Complete candidate verification, PostgreSQL fixtures, ordered HTTP groups, exact-dist
oracles and both recovery drills remain mandatory.
The PR sentinel checks the policy and runner; a separate gate validates complete reports.

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

## Daily result reuse and host budget

The reviewed Node profile caches passed policy checks, Web preparation and type
checks, and selected Node tests as one group. A hit records a `cached` step and its
producer report. Cargo groups always execute. Complete candidate reuse follows the admission rules below. Selected Vitest runs disable result-cache writes.

The key binds current non-ignored input bytes, modes and link targets; test-file membership;
checks and workers; runners and toolchains; host identity; and an environment digest.
An unrelated file edit can miss, but a new Git SHA alone does not. Environment values stay private.
Reuse requires the original complete successful report, its digest and Vitest output,
and equal installed Node dependency trees, including modes and write stamps. Dependency
identity must stay equal from prepared test start through report completion. Missing,
changed or corrupt output causes execution. The Web workspace link binds to repository
inputs; other external links disable reuse. Failed, interrupted, incomplete or
source-changing runs cannot publish reusable results.

Use `make verify-changed BASE=HEAD VERIFY_ARGS=--no-cache` to force execution without
reading or publishing a result-cache entry. Local entries in `target/verification-cache/`
need their referenced reports. A cache hit is daily feedback, not candidate evidence.

The complete and daily run commands admit one run per checkout at a time. A busy
budget fails with a retry reason. The lock covers process-group cleanup; overdue
descendants are terminated and fail the run. The input policy caps daily Cargo build
jobs, Rust test threads and Vitest workers. Use `VERIFY_ARGS='--workers 1'` to lower
that cap. Groups stay serial. Complete runs retain their existing worker configuration.

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
Existing independent reviews and protected evidence publication remain required.

## Candidate evidence publication

1. Wait for the `verify` sentinel and independent Standards and Spec reviews. Resolve
   findings with targeted checks. Fetch current main and the PR synthetic merge.
2. Run `make verify-local` once on the clean candidate tree that equals that merge tree.
3. Publish with `make verify-evidence PR=<number> REPORT=<report-path>`. The activated
   contract requires current candidate review imports. Use `VERIFY_ARGS=--policy-reviewed`
   only for the compatible installation against the earlier protected reader.
4. Wait for `candidate-evidence` success, then merge. A changed head, base, tree, policy
   revision or discovered membership requires fresh evidence. Preserve strict branch protection.

The evidence workflow runs protected main code and reads candidate Git objects as data.
It checks the newest authorized structured report; an invalid newer report cannot fall
back to an older success. Publication through a PR comment triggers validation without
a source commit. The report includes its command, clean source identity and a complete
plan bound to the base, tree, policy digest and current test files. The policy declares
mandatory stages and group ownership, not historical test names or counts. Web records
must cover each current file through its whole project or explicit file selection.

Extend the policy and runner together for a new framework or execution group. Changes
to verification commands, selectors, manifests, workflows or guidance require the explicit
policy-review declaration. The protected validator still checks the candidate report.
Reports and review declarations remain Agent-writable: they establish consistency within
the local trust boundary, not independent proof of execution or reviewer identity.
Complete local verification and manual Linux verification retain all existing obligations.

## Current status and targeted checks

`make verify-status BASE=origin/main` reads the current daily plan and retained
results. It does not run tests or write observations. Use
`python3 scripts/verification_plan.py status --base origin/main` for JSON, or
`make verify-plan VERIFY_ARGS='--format text'` for a readable plan. An empty
change set remains pending. A prior result can be passed, failed, or stale;
package-dependent work on dirty sources has unmet prerequisites.

`make verify-targeted CHECK=verify-policy VERIFY_ARGS='--issue 744'` runs a check
registered in the policy. Query it with
`python3 scripts/verification.py status --check verify-policy --json`.
Source bytes and write stamps, policy, test membership, command, execution input
digest, and plan identity bind targeted results. Only a current passed result
can satisfy a current prerequisite. These results do not replace candidate evidence.
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

## Local run observation

Use Docker Engine with Compose 2.24.4 or later and Python 3.13 or later.
`make observe-start` builds the pinned Grafana OSS image and SQLite plugin, imports
retained records, and starts a five-second collector. Open
<http://127.0.0.1:3749/d/storyos-verification>. `make observe-status` shows container
state and current records. `make observe-stop` removes the observation containers;
records and the read model remain. `make observe-rebuild` rebuilds the read model
from retained files. Collection catches up after a restart without test execution.

The collector reads only `target/verification/*/report.json`, step files, and
request files. It writes `target/observation/data/runs.sqlite`. These directories
are ignored source and cache inputs. SQLite is disposable observation, not
admission or test evidence. No executor imports this database. The dashboard shows
root Issue attribution (or unknown), live stages, scope, reason, elapsed time to
the last heartbeat, and heartbeat age. Display elapsed time uses UTC and is an
estimate; recorded monotonic durations remain in the read model. A stale heartbeat
means unknown liveness, not completion. It does not change the reported status.
Legacy and malformed inputs stay in the diagnostics view. Raw records are never
changed; missing historical attribution remains unknown. Deleted input records
stay in the read model until a rebuild. Rebuild does not preserve derived facts.

The public collector accepts `collect`, `watch`, `status`, and `rebuild`, with
`--records` and `--database` for disposable inputs. `watch --interval` accepts
1 to 3600 seconds. Each collection transaction writes at most 200 changed records;
watch catches up on later polls and one-shot commands drain the backlog. Each
input is limited to 16 MiB. Containers have CPU and memory limits. Collection scans
retained paths; volume overhead measurement belongs to the Issue-cost follow-up.

Grafana binds only to loopback and permits anonymous Viewer access. Login and basic
authentication are disabled; its unused administrator password is random on each
start. The provisioned data source is not editable, opens SQLite with `mode=ro`,
and has `attachLimit: 0`. Only the observation directory is mounted, read-only.
The plugin's default internal-database block list remains enabled. The collector
has no network. Grafana has no configured external notification destination, and
analytics is disabled; advisory alert evaluation stays local. Setup needs Internet access to pull images
and the plugin; this does not change product hosting.

Run `make verify-targeted CHECK=verification-observation-tests` for collector
regressions. Run `make observe-smoke` after installation changes. This bounded
check starts an actual disposable managed command and queries the provisioned
Grafana dashboard through the real SQLite plugin on a temporary loopback port.
It removes its containers and temporary database when complete. It does not run
the product suite. `make verify-policy` discovers the collector tests normally.

### Issue cost and advisory rules

The same dashboard shows `issue_cost`, `request_cost`, `stage_cost`, and
`violations`. Root totals include only recorded actual starts. They retain the
profile and final state, including failures and interruptions. Reused reports
and refused requests stay separate. `observed_runs` retains historical and
incomplete observations; missing attribution and duration remain unknown.

Stage intervals partition each root duration. Child intervals replace parent
intervals; concurrent leaf intervals have one `concurrent` cost. Remaining time
is `unclassified`. These are elapsed costs, not CPU measurements. An optional
`blocked_intervals` list contains explicit monotonic start/end pairs. An empty
list records zero; an absent list means unknown. Multiple roots need one explicit
`blocked_clock` identity before their intervals can form an Issue-wide union.
The Issue blocked total repeats across its profile rows; do not sum those cells.
Current executors do not measure user blocking, so their value remains unknown.

Advisory rules detect daily complete dispatch, duplicate candidate starts without
recorded recovery, unassigned starts, overlapping checkout resource use, stale
heartbeats, and comparable runtime growth. Requests do not become executed
violations. A stale heartbeat means unknown liveness. Runtime comparison requires
an explicit recorded `build_state` (`cold` or `warm`), equal effective scope,
policy, tools, execution inputs, runners, host and repository. Missing facts or
samples stay unknown. Current reports omit build state and do not establish a
runtime benchmark. Never edit original reports to add these observations.

`scripts/observation/settings.json` sets the heartbeat and comparison thresholds,
record batch cap, and database size guard. Compose caps CPU and memory. Grafana
refreshes every five seconds and evaluates provisioned rules every ten seconds.
There is no external notification destination. Grafana rules query the same
`violations` view as the dashboard and cannot authorize execution or retries.

Collection prints wall and CPU seconds, peak process RSS and database bytes.
`make observe-smoke` measures 1,000 synthetic retained roots plus one actual
bounded managed command, queries the real dashboard, waits for a firing rule,
restarts observation without restarting that command, and records container CPU,
memory and block I/O. This is synthetic overhead evidence, not a product speedup.
The original records survive observation downtime and rebuild. The database
size guard stops further collection above 256 MiB; it never removes raw records.
