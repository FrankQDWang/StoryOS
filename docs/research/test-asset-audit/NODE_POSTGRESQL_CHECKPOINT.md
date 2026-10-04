# Node PostgreSQL checkpoint

Source review is partial: 32 tests in 12 files have individual verdicts. Remaining files are listed below. No parameterized file has been adjudicated yet; expand its actual cases manually before using a declaration count.

| Reviewed file | Cases | Verdict counts |
|---|---|---|
| apps/web/test/node-postgresql/activity-stream-cross-table-http.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/node-postgresql/activity-stream-duplicate-http.integration.test.ts | 1 | {'MERGE': 1} |
| apps/web/test/node-postgresql/archive-project-http.integration.test.ts | 3 | {'MERGE': 1, 'KEEP': 2} |
| apps/web/test/node-postgresql/create-project-challenge-http.integration.test.ts | 2 | {'KEEP': 2} |
| apps/web/test/node-postgresql/create-project-http.integration.test.ts | 2 | {'KEEP': 2} |
| apps/web/test/node-postgresql/list-projects-http.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/node-postgresql/manuscript-search-http.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/node-postgresql/manuscript-tree-http.integration.test.ts | 2 | {'KEEP': 2} |
| apps/web/test/node-postgresql/project-http.integration.test.ts | 8 | {'MERGE': 1, 'KEEP': 7} |
| apps/web/test/node-postgresql/protocol-http-host.integration.test.ts | 5 | {'DELETE': 1, 'KEEP': 4} |
| apps/web/test/node-postgresql/snapshot-replay-http.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/node-postgresql/update-project-http.integration.test.ts | 5 | {'MERGE': 1, 'KEEP': 4} |

## Next files

Continue structural command families: create-volume, create-chapter, update-volume, update-chapter, delete-volume, delete-chapter, set-current-chapter, and undo-latest-author-action. Then Proposal/Run/export families. All tests in project-http and protocol-http-host have now been read and adjudicated.

## Evidence corrections and execution dependencies

- The Author Edit unit-count and primitive-count cases in project-http:253 use no issued Challenge and an all-zero nonce. Both only assert 422 plus unchanged authority. A removed count guard can still produce the expected result at Challenge refusal. NP008 records the limitation; CT007 no longer describes these as proved count-limit coverage. No mutation experiment has been run.
- NC022 was corrected: the User UUID is checked in packaged_session_mappings and again in main after Storage Activation. Its broad process-exit oracle remains masked by missing later prerequisites. NP012 retains the real-database startup rejection case.
- NP006 MERGE: move both Users positive Project/Chapter reads into project-http:1080. Existing coverage citations to :132 remain conditional on that merge during final reconciliation.
- NP023 MERGE: add byte-identical repeated SSE response and unchanged authority assertions to activity-stream-cross-table-http:168, then remove activity-stream-duplicate-http:133. Complete repeated frame data is already covered; complete response bytes and authority counts are not.
- NP025 MERGE: move changed rename retry and list revision into update-project-http:294. Redirect the earlier Adapter update_project Activity-count MERGE from :210 to this retained owner.
- NP030 MERGE: move changed archival retry, library revision and blocked valid rename into archive-project-http:308. The earlier Adapter archival Activity-count MERGE already targets that retained owner.
- Foreign rename/archive helper failures occur while acquiring a Challenge, before the command request is sent. They must not be described as command-handler scope coverage.
- Keep first-ack publication race separate from sequential replay, and Current Chapter replay separate from empty-Project title replay. These have different discriminating inputs.
- append_web_tables.py handles only plain top-level declarations and rejects parameterized/nested declarations. It records manually supplied verdicts and source spans; it does not infer decisions. The installed TypeScript 7 package has no importable legacy JS parser, so no dependency was added.

## Pending files

- apps/web/test/node-postgresql/accept-proposal-http.integration.test.ts
- apps/web/test/node-postgresql/acceptance-refusal-http.integration.test.ts
- apps/web/test/node-postgresql/admitted-proposal-target-http.integration.test.ts
- apps/web/test/node-postgresql/compact-active-context-http.integration.test.ts
- apps/web/test/node-postgresql/complete-fake-model-decision-http.integration.test.ts
- apps/web/test/node-postgresql/complete-ready-partial-proposal-http.integration.test.ts
- apps/web/test/node-postgresql/continue-conversation-input-http.integration.test.ts
- apps/web/test/node-postgresql/continue-proposal-generation-claim-http.integration.test.ts
- apps/web/test/node-postgresql/continue-proposal-generation-http.integration.test.ts
- apps/web/test/node-postgresql/create-agent-run-http.integration.test.ts
- apps/web/test/node-postgresql/create-chapter-http.integration.test.ts
- apps/web/test/node-postgresql/create-volume-http.integration.test.ts
- apps/web/test/node-postgresql/delete-chapter-http.integration.test.ts
- apps/web/test/node-postgresql/delete-volume-http.integration.test.ts
- apps/web/test/node-postgresql/edit-inline-proposal-http.integration.test.ts
- apps/web/test/node-postgresql/edit-proposal-candidate-http.integration.test.ts
- apps/web/test/node-postgresql/export-acknowledgement-support.ts
- apps/web/test/node-postgresql/open-block-proposal-http.integration.test.ts
- apps/web/test/node-postgresql/project-export-admission-http.integration.test.ts
- apps/web/test/node-postgresql/project-export-pinned-source-http.integration.test.ts
- apps/web/test/node-postgresql/readable-export-admission-http.integration.test.ts
- apps/web/test/node-postgresql/readable-export-pinned-source-http.integration.test.ts
- apps/web/test/node-postgresql/rebuild-expired-reference-http.integration.test.ts
- apps/web/test/node-postgresql/recover-or-cancel-agent-run-http.integration.test.ts
- apps/web/test/node-postgresql/recovery-archived-exports-http.integration.test.ts
- apps/web/test/node-postgresql/reject-proposal-operations-http.integration.test.ts
- apps/web/test/node-postgresql/reopen-rejected-operations-http.integration.test.ts
- apps/web/test/node-postgresql/replan-proposal-http.integration.test.ts
- apps/web/test/node-postgresql/retrieve-original-result-http.integration.test.ts
- apps/web/test/node-postgresql/set-current-chapter-http.integration.test.ts
- apps/web/test/node-postgresql/settle-multi-operation-selections-http.integration.test.ts
- apps/web/test/node-postgresql/stream-proposal-generation-http.integration.test.ts
- apps/web/test/node-postgresql/takeover-http.integration.test.ts
- apps/web/test/node-postgresql/takeover-late-result-http.integration.test.ts
- apps/web/test/node-postgresql/undo-acceptance-http.integration.test.ts
- apps/web/test/node-postgresql/undo-latest-author-action-http.integration.test.ts
- apps/web/test/node-postgresql/unknown-create-successor-http.integration.test.ts
- apps/web/test/node-postgresql/update-chapter-http.integration.test.ts
- apps/web/test/node-postgresql/update-project-assistance-http.integration.test.ts
- apps/web/test/node-postgresql/update-volume-http.integration.test.ts
- apps/web/test/node-postgresql/withdraw-proposal-http.integration.test.ts
