# Repository verification

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
Rust opt-in files must appear in compiler dependency records for a test executable,
then run their existing crate test targets. Unlinked files fail. Crates with ignored
tests or conditional attributes retain the complete group. Rust production changes
include current reverse consumers in the plan and require complete verification.
No function filter is inferred.

When adding, renaming or deleting tests, run `make verify-policy` and inspect a fresh
plan. Renames and deletions expand to complete verification and remove obsolete
files from current test membership. Unknown locations or execution profiles fail.
A new framework or shared resource requires a reviewed policy and runner extension,
with a public command regression. Review declarations with the same independent
Standards and Spec process as code. Test names and counts are discovered, not fixed.

Use these commands from Codex, Cursor and Grok Build. If a client does not load
AGENTS.md, include this document in its project instructions. The checked policy
is the common owner; client instructions link here. Selected dirty-tree runs are
daily feedback. A complete group needs a clean tree because release packaging binds
Git identity. An empty change set or empty test discovery cannot report success.
Complete candidate verification, PostgreSQL fixtures, ordered HTTP groups, exact-dist
oracles and both recovery drills remain mandatory. There is no result cache yet.
The PR sentinel checks the policy and runner, but does not yet validate full reports.
