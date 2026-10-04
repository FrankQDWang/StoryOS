# node-process-cut test verdicts

Reviewed: 5 cases in 3 files. See PROGRESS.md for directory completion.

Reason codes: [METHOD.md](METHOD.md). Locations use the fixed audit baseline.

## apps/web/test/node-process-cut/apply-author-edit-process-cut.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| PC001 | 179 — Server restart and bounded PostgreSQL interruption keep GET-first ApplyAuthorEdit recovery | KEEP | K2 | An issued Challenge and later committed Author Edit must remain queryable after actual Server restarts without extra authority rows. Pausing PostgreSQL must not fabricate a committed result, and unpausing must restore the same result/counts. Browser recovery mocks transport and cannot catch process-local evidence or stale outcome caching. The test explicitly calls GET and throws delivery loss after reading the full response; it does not prove an automatic client recovery policy or an in-flight crash. Restarts use the controlled stop helper and database unavailability uses a two-second client timeout. | crates/storyos-adapter-postgres/src/author_edit_outcome_tests.rs:59; apps/web/test/browser-source/reload-recovery.integration.test.ts:68; apps/web/test/node-postgresql/takeover-late-result-http.integration.test.ts:119 |

## apps/web/test/node-process-cut/project-export-admission-process-cut.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| PC002 | 104 — an admitted Project Export Archive stays in progress across a Server process cut | MERGE | M1 | Move the pre-claim Server restart, retained export identity, in_progress status and absent immutable_root assertions into the same-file Worker-claim case before claim-only. It already admits a real Archive and stops/restarts Server around durable work. Checking the unclaimed state is distinct from outcome_unknown, so transfer it before deleting this separate empty-Project fixture. Worker --check is startup smoke, not unique export behavior. | apps/web/test/node-process-cut/project-export-admission-process-cut.integration.test.ts:180 |
| PC003 | 180 — a Worker claim without settlement is outcome_unknown, then takeover settles ready | KEEP | K2 | A durable Archive claim that exits without settlement must be outcome_unknown with no published root, then become downloadable ready output after lease expiry and a new Worker process. Readable export has a separate claim/settlement/output family; ordinary pinned Archive completion does not leave a claim without settlement. This uses --claim-only plus TTL zero, not an unexpected process kill. The Receipt count regex /0$/ is weaker than exact zero and ZIP magic alone is not full archive validation. Receive the pre-claim in-progress restart assertions from :104. | apps/web/test/node-postgresql/project-export-pinned-source-http.integration.test.ts:148; apps/web/test/node-process-cut/readable-export-admission-process-cut.integration.test.ts:182 |

## apps/web/test/node-process-cut/readable-export-admission-process-cut.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| PC004 | 106 — an admitted human-readable export stays in progress across a Server process cut | MERGE | M1 | Move the pre-claim Server restart, retained export identity, in_progress status and absent manuscript_utf8 assertions into the same-file Worker-claim case before claim-only. Its real admission and process lifecycle can check this earlier state without a second empty-Project fixture. Keep the assertion transfer as a prerequisite; outcome_unknown alone does not cover an unclaimed operation. | apps/web/test/node-process-cut/readable-export-admission-process-cut.integration.test.ts:182 |
| PC005 | 182 — a Worker claim without settlement is outcome_unknown, then takeover settles ready | KEEP | K2 | A readable export claim left unsettled must survive process exit as outcome_unknown with no manuscript, then be reclaimed by the next Worker and expose exact empty manuscript bytes. Archive claims follow a separate family and ordinary readable pinning has no expired claim. --claim-only and TTL zero establish a durable cut, not an unexpected crash. The /0$/ Receipt assertion does not prove exact zero. Receive the unclaimed restart assertions from :106. | apps/web/test/node-postgresql/readable-export-pinned-source-http.integration.test.ts:124; apps/web/test/node-process-cut/project-export-admission-process-cut.integration.test.ts:180 |
