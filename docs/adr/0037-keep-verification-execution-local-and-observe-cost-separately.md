# Keep verification execution local and observe cost separately

Keep Python and Make as the verification executors. First repair daily selection,
complete-run admission, and recovery. Evaluate moon or cargo-nextest only after
measurements show a remaining need. A new runner does not by itself prevent an
Agent from repeatedly starting complete verification.

Use Grafana OSS and SQLite for local observation of existing execution records.
SQLite is a rebuildable read model. Observation cannot execute tests, change a
result, or cause a retry. The first deployment stays on the developer machine;
dashboard availability is not a verification prerequisite. This decision does not
change the production topology in ADR 0022 or the domain Verification Evidence
Bundle. The verification specification owns the staged delivery and acceptance.
