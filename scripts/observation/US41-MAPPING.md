# US41 requirement and source map

| Requirement | Human view | API | Retained source or projection |
| --- | --- | --- | --- |
| Six rule categories and evidence | Overview counts; collapsed needs-review groups; run drawer | `/violations`, `/runs/<run>` | Existing `violations` view over valid run and request records and `timing_comparison`; evidence paths point to retained report or request files. |
| Refused, reused, and actual starts | Review disposition and history activity filters; run summary and cost drawer | `/violations`, `/runs?activity=`, `/runs/<run>`, `/requests?run=`, `/runs/<run>/cost` | Request outcome, run `attempt_started`, `request_cost`, and `run_cost`. A refused or reused request adds no root cost. |
| Exclusive Issue cost and wait quality | Cost drawer | `/runs/<run>/cost` | `run_cost`, `stage_cost`, `issue_cost`, `request_cost`, `issue_wait`. Concurrent and unclassified intervals stay separate. One Issue wait total is shown once. |
| Graph, node, and UTC timeline | Direct summary entry; graph, node, and timeline views; full dashboard links | `/runs/<run>/graph`, `/runs/<run>/attempts` | `run_graphs`, `node_states`, `node_attempts`; dependency and member edges stay distinct. Missing ends or file attempts remain unknown. |
| Two-run differences and quality | Comparison search, scope summary, changed-node list, full comparison link | `/runs`, `/compare?left=&right=` | The same comparison SQL as the dashboard over retained run payloads, graphs, states, and attempts. |
| Reading continuity | Hash route, session state, visible Back and new-tab links | No write API | Browser state only; polling keeps displayed membership until Update. |

Omissions: Remote CI, shell work, unmeasured build and wait, grouped file duration, and missing legacy facts remain unknown; the App cannot schedule, admit, retry, or change results.
