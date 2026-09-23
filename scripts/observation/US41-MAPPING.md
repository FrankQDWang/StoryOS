# US41 requirement and source map

The App reads a disposable SQLite projection. Original reports in `target/verification/` remain execution evidence. API paths below are GET under `/api/v1`; collection can lag and pages are separate snapshots.

| Requirement | Human view | API | Retained source or projection |
| --- | --- | --- | --- |
| Six rule categories and evidence | Overview counts; collapsed needs-review groups; run drawer | `/violations`, `/runs/<run>` | Existing `violations` view over valid run and request records and `timing_comparison`; evidence paths point to retained report or request files. |
| Refused, reused, and actual starts | Review disposition and history activity filters; run summary and cost drawer | `/violations`, `/runs?activity=`, `/runs/<run>`, `/requests?run=`, `/runs/<run>/cost` | Request outcome, run `attempt_started`, `request_cost`, and `run_cost`. A refused or reused request adds no root cost. |
| Exclusive Issue cost and wait quality | Cost drawer | `/runs/<run>/cost` | `run_cost`, `stage_cost`, `issue_cost`, `request_cost`, `issue_wait`. Concurrent and unclassified intervals stay separate. One Issue wait total is shown once. |
| Graph and node facts | Direct run-summary entry; graph and node views; full DAG link | `/runs/<run>/graph`, `/runs/<run>/attempts` | `run_graphs`, `node_states`, `node_attempts`; graph dependencies and relations keep distinct meanings. |
| UTC attempt timeline | Timeline view; full Grafana timeline link | `/runs/<run>/attempts` | Actual node attempts. A missing end or file attempt remains unknown. |
| Two-run differences and quality | Comparison search, scope summary, changed-node list, full comparison link | `/runs`, `/compare?left=&right=` | The same comparison SQL as the dashboard over retained run payloads, graphs, states, and attempts. |
| Reading continuity | Hash route, session state, visible Back and new-tab links | No write API | Browser state only; polling keeps displayed membership until Update. |

Current limits and omissions:

- Remote CI and arbitrary shell work are outside managed local coverage.
- Current executor reports do not measure user blocked wait or build state. Issue wait and comparable runtime claims normally remain unknown.
- Grouped Cargo or shared tests do not prove member file execution or duration. Selection does not prove a minimal plan.
- Old reports can lack graph, attribution, request, or timing facts. The App keeps those facts unknown.
- The App does not send alerts, admit, change results, retry, or schedule an Agent. Grafana advisory rules remain provisioned.
