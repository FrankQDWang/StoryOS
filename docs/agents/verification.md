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

The inventory is a prerequisite for future file selection. It currently neither
selects nor caches tests. Keep the complete candidate gate, PostgreSQL fixtures,
ordered HTTP groups, exact-dist oracle, and both recovery drills. The PR sentinel
checks the inventory and harness, but does not yet validate full-run reports.
