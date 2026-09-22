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

## Repository-local storage

Keep observation source and deterministic configuration in this repository and
track them in Git. Keep generated runtime output, logs, screenshots, health data,
and SQLite databases with their sidecars under Git-ignored `target/observation/`.
The existing read model is `target/observation/data/runs.sqlite`. Original execution
reports remain under Git-ignored `target/verification/` and are not replaced by
the read model. Container mounts use these checkout paths for persistent
observation data, not an external directory or a Docker named volume. This layout
does not move the product database or select a cloud service.

The approved supervision interface and read-only API must be built from tracked
source through repository-owned commands. The ignored interaction prototype is a
design reference, not a runtime dependency. Health probes and evidence queries
remain read-only; periodic probing does not authorize autonomous Agent diagnosis,
test execution, or corrective actions.
