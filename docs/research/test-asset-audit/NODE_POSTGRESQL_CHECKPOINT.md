# Node PostgreSQL checkpoint

Source review is partial: 65 tests in 20 files have individual verdicts. Remaining files are listed below. No parameterized file has been adjudicated yet; expand its actual cases manually before using a declaration count.

| Reviewed file | Cases | Verdict counts |
|---|---|---|
| apps/web/test/node-postgresql/activity-stream-cross-table-http.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/node-postgresql/activity-stream-duplicate-http.integration.test.ts | 1 | {'MERGE': 1} |
| apps/web/test/node-postgresql/archive-project-http.integration.test.ts | 3 | {'MERGE': 1, 'KEEP': 2} |
| apps/web/test/node-postgresql/create-chapter-http.integration.test.ts | 6 | {'KEEP': 5, 'DELETE': 1} |
| apps/web/test/node-postgresql/create-project-challenge-http.integration.test.ts | 2 | {'KEEP': 2} |
| apps/web/test/node-postgresql/create-project-http.integration.test.ts | 2 | {'KEEP': 2} |
| apps/web/test/node-postgresql/create-volume-http.integration.test.ts | 4 | {'KEEP': 3, 'DELETE': 1} |
| apps/web/test/node-postgresql/delete-chapter-http.integration.test.ts | 4 | {'KEEP': 3, 'DELETE': 1} |
| apps/web/test/node-postgresql/delete-volume-http.integration.test.ts | 4 | {'KEEP': 3, 'DELETE': 1} |
| apps/web/test/node-postgresql/list-projects-http.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/node-postgresql/manuscript-search-http.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/node-postgresql/manuscript-tree-http.integration.test.ts | 2 | {'KEEP': 2} |
| apps/web/test/node-postgresql/project-http.integration.test.ts | 8 | {'MERGE': 1, 'KEEP': 7} |
| apps/web/test/node-postgresql/protocol-http-host.integration.test.ts | 5 | {'DELETE': 1, 'KEEP': 4} |
| apps/web/test/node-postgresql/set-current-chapter-http.integration.test.ts | 5 | {'KEEP': 5} |
| apps/web/test/node-postgresql/snapshot-replay-http.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/node-postgresql/undo-latest-author-action-http.integration.test.ts | 3 | {'KEEP': 2, 'DELETE': 1} |
| apps/web/test/node-postgresql/update-chapter-http.integration.test.ts | 4 | {'KEEP': 2, 'DELETE': 2} |
| apps/web/test/node-postgresql/update-project-http.integration.test.ts | 5 | {'MERGE': 1, 'KEEP': 4} |
| apps/web/test/node-postgresql/update-volume-http.integration.test.ts | 3 | {'KEEP': 1, 'DELETE': 2} |

## Next files

Continue Proposal, Run, context and export command families. Structural commands, Current Chapter and Undo are now reviewed.

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
- apps/web/test/node-postgresql/settle-multi-operation-selections-http.integration.test.ts
- apps/web/test/node-postgresql/stream-proposal-generation-http.integration.test.ts
- apps/web/test/node-postgresql/takeover-http.integration.test.ts
- apps/web/test/node-postgresql/takeover-late-result-http.integration.test.ts
- apps/web/test/node-postgresql/undo-acceptance-http.integration.test.ts
- apps/web/test/node-postgresql/unknown-create-successor-http.integration.test.ts
- apps/web/test/node-postgresql/update-project-assistance-http.integration.test.ts
- apps/web/test/node-postgresql/withdraw-proposal-http.integration.test.ts

## Structural and navigation checkpoint

- NP033-NP057 cover six complete Volume/Chapter command files (25 tests). All six commands share structure_command.rs replay_structure, CommandReplay::response_project and StructureRoute::problem. Retain create-volume-http:760 for common missing-history/damaged-projection responses; delete the five repeated negative cases. Retain each distinct live domain input and command-specific legacy effect reconstruction.
- Keep delete-volume-http:517 for four outcome projections (including Refused), create-chapter-http:1002 for nonempty Current Chapter A becoming B, and delete-chapter-http:571 for captured empty Project becoming nonempty. The Delete Volume test does not actually change Current Chapter despite its title. Delete repeated Project freeze cases in create-volume:597, update-volume:497 and update-chapter:632.
- The named Changed Retry subcases in create-volume:148 and create-chapter:361 use a new unissued key with an old nonce. They do not prove changed canonical bytes against an existing idempotency binding.
- Keep create-chapter:262: its old Session starts at Activity position zero while a new Session captures the actual nonzero position. Generic creation tests have no old/new Session comparison.
- update-chapter:252 receives the earlier Adapter MERGE of exact unchanged Head Revision identities. Existing empty prose equality cannot replace that assertion.
- NP058-NP064 cover navigation and Undo projection cases. These commands each own a separate SQL replay reader, response column offsets, error conversion and HTTP problem match. Their projection-error cases cannot be removed by analogy with the shared structural sequence.
- NP065 deletes the HTTP Undo rate-budget case. Inspection corrected an initial assumption: the Adapter helper and Server dispatcher use the same ChallengeRateClass::for_command_kind. Adapter project_command_challenge_tests:282 already pins combined capacity, free exact retry and Project separation with a fixed clock; the HTTP case relies on an unmeasured two-window timing assumption.
- Reconcile older Rust and Contracts covering-test citations that point to the newly deleted structural HTTP cases. Redirect to the retained owner for the exact behavior, not merely another command with a similar name. This reconciliation remains pending before sample selection.
- No product, test or generated file changed. No mutation experiment or targeted execution ran during this source-review checkpoint.
