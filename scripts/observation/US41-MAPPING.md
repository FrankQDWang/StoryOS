# US41 requirement and source map

The App reads a disposable SQLite projection. Original reports in
`target/verification/` remain the execution evidence. All paths below are GET
paths under `/api/v1`. A response can lag collection and is not one snapshot
across pages.

| Requirement | Human view | API | Retained source or projection |
| --- | --- | --- | --- |
| Six rule categories and evidence | Overview counts; collapsed needs-review groups; run drawer | `/violations`, `/runs/<run>` | Existing `violations` view over valid run and request records and `timing_comparison`; evidence paths point to retained report or request files. |
| Refused, reused, and actual starts | Run summary and cost drawer | `/runs/<run>`, `/requests?run=`, `/runs/<run>/cost` | Request outcome, run `attempt_started`, `request_cost`, and `run_cost`. A refused or reused request adds no root cost. |
| Exclusive Issue cost and wait quality | Cost drawer | `/runs/<run>/cost` | `run_cost`, `stage_cost`, `issue_cost`, `request_cost`, `issue_wait`. Concurrent and unclassified intervals stay separate. One Issue wait total is shown once. |
| Graph and node facts | Direct run-summary entry; graph and node views; full DAG link | `/runs/<run>/graph`, `/runs/<run>/attempts` | `run_graphs`, `node_states`, `node_attempts`; graph dependencies and relations keep distinct meanings. |
| UTC attempt timeline | Timeline view; full Grafana timeline link | `/runs/<run>/attempts` | Actual node attempts. A missing end or file attempt remains unknown. |
| Two-run differences and quality | Comparison search, scope summary, changed-node list, full comparison link | `/runs`, `/compare?left=&right=` | The same comparison SQL as the dashboard over retained run payloads, graphs, states, and attempts. |
| Reading continuity | Hash route, session state, visible Back and new-tab links | No write API | Browser state only; polling keeps displayed membership until Update. |

Current limits and omissions:

- Remote CI and arbitrary shell work are outside managed local coverage.
- Current executor reports do not measure user blocked wait or record build
  state. Issue wait and comparable runtime claims normally remain unknown.
- A grouped Cargo or shared test stage does not prove each member file ran or
  has a duration. Selected membership is not minimal-selection proof.
- Old reports can lack graph, attribution, request, or timing facts. The App
  displays unknown and does not fill gaps from later records.
- The App does not send alerts, admit a candidate, change a test result, retry,
  or schedule an Agent. Existing Grafana advisory rules remain provisioned.
