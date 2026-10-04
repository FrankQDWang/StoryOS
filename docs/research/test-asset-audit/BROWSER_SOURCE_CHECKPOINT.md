# Browser source checkpoint

Source review is partial: 19 runtime cases in 15 test files. No mutation samples selected.

| Reviewed file | Cases | Verdict counts |
|---|---|---|
| apps/web/test/browser-source/accept-block-proposal.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/acceptance-journal.integration.test.ts | 1 | {'MERGE': 1} |
| apps/web/test/browser-source/archive-historical-acknowledgement.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/author-edit-outcome.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/browser-command-foundation.test.ts | 2 | {'DELETE': 1, 'MOVE': 1} |
| apps/web/test/browser-source/chapter-navigation.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/create-historical-acknowledgement.integration.test.ts | 2 | {'KEEP': 2} |
| apps/web/test/browser-source/current-chapter-historical-acknowledgement.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/delete-historical-acknowledgement.integration.test.ts | 2 | {'KEEP': 2} |
| apps/web/test/browser-source/list-open.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/project-entry.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/readable-export-historical-acknowledgement.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/rename-historical-acknowledgement.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/undo-historical-acknowledgement.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/update-historical-acknowledgement.integration.test.ts | 2 | {'KEEP': 2} |

## Pending files

- apps/web/test/browser-source/acknowledgement-loss.integration.test.ts
- apps/web/test/browser-source/activity-reorder.integration.test.ts
- apps/web/test/browser-source/activity-resync.integration.test.ts
- apps/web/test/browser-source/activity-stream-consume.integration.test.ts
- apps/web/test/browser-source/author-edit-rate-limit.integration.test.ts
- apps/web/test/browser-source/author-undo-rate-limit.integration.test.ts
- apps/web/test/browser-source/draft-undo-lifetime.integration.test.ts
- apps/web/test/browser-source/editor-session.integration.test.ts
- apps/web/test/browser-source/journal-gc-fenced.integration.test.ts
- apps/web/test/browser-source/journal-gc.integration.test.ts
- apps/web/test/browser-source/journal-version-three.ts
- apps/web/test/browser-source/journal-working-set.integration.test.ts
- apps/web/test/browser-source/local-edit-journal-append-drift.integration.test.ts
- apps/web/test/browser-source/local-edit-journal-append-fixture.ts
- apps/web/test/browser-source/local-edit-journal-append-projection.integration.test.ts
- apps/web/test/browser-source/local-recovery-panel.integration.test.ts
- apps/web/test/browser-source/manual-input.integration.test.ts
- apps/web/test/browser-source/manuscript-tiptap-adapter.integration.test.ts
- apps/web/test/browser-source/refused-edit-discard.integration.test.ts
- apps/web/test/browser-source/reload-recovery.integration.test.ts
- apps/web/test/browser-source/scenario.ts
- apps/web/test/browser-source/takeover-late-result.integration.test.ts

## Evidence and execution dependencies

- BS001-BS011 cover eight historical-acknowledgement files. Unlike the shared Server structural replay reader, each browser command has its own retry map/catch and each form has a separate error-state owner. Keep actual UI/no-retry behaviors; mocked 409 responses do not prove database history handling.
- Archive historical acknowledgement calls the wrapper directly. Its title claims an explanation, but it does not mount UI or assert text. Retained protection is one PUT/Challenge and historical error classification. Delete Volume preserves the Project title but does not explicitly compare the Volume row.
- Chapter creation holds its request while another creation is attempted, then checks the same read-only input/title/Volume remains connected. Ordinary creation success lacks this controlled interleaving.
- BS012 keeps execution of the generated outcome proof-header request: no nonce in URL/body, exact GET, same-origin credentials and required proof. CT010 names it as the executable owner; static contract shape is not equivalent.
- BS013 keeps Acceptance delivery classification: exact retry after unknown/unreadable output, competing decision refusal, recovered persisted in-flight Attempt, terminal pre-admission refusal, and later authentication failure that cannot resolve an earlier unknown delivery. IndexedDB/mocked delivery differs from real HTTP refusal storage.
- BS014 MERGE moves valid flight sequence/record/group and pending-projection assertions into BS013 before successful retry. Fabricated foreign/key arguments to createFlight are not stored-partition drift: both product callers construct them from the same workspace. Reconcile accepted persistence/seam contracts before any broader D5 inference.
- BS015 DELETE removes generic input/clipboard smoke. Packaged input at s2-input:284 invokes those commands and observes saved manuscript changes. Cookie action=set is used only by the foundation test; actual journeys use another session setup and invoke this command only to clear it. A mutation confined to that unused helper action is not claimed caught by the packaged journey. This D4 limitation remains in the random sample population.
- BS016 MOVE keeps malformed production-command rejection at the pure Node parser layer; proposed destination: apps/web/test/node-contract/browser-command-contract.test.ts. No destination was created. The privileged browser-command dispatcher invokes this parser; valid journeys do not cover malformed inputs.
- BS017-BS019 keep stale library-title replacement, pending Chapter text across inspection/query failure, and invalid URL entry before requests. Packaged library/navigation use matching live titles and empty Chapters. Ordinary heading/query-order/credential checks can be trimmed while preserving unique assertions.
- Browser declaration and citation starts checked against fixed source. Current totals: 16 KEEP, one DELETE, one MERGE, one MOVE; 40 immediate candidate lines. No runtime test, product edit, source mutation or database command occurred.
