# Browser exact-dist checkpoint

Source review is partial: 10 cases in 10 test files; four KEEP, one DELETE, five MERGE. Direct candidate lines: 67.

| Reviewed file | Verdict |
|---|---|
| apps/web/test/browser-exact-dist/exact-dist-foundation.test.ts | DELETE |
| apps/web/test/browser-exact-dist/production-page.integration.test.ts | MERGE |
| apps/web/test/browser-exact-dist/s2-01-bootstrap-challenge.integration.test.ts | MERGE |
| apps/web/test/browser-exact-dist/s2-02-create-empty-project.integration.test.ts | MERGE |
| apps/web/test/browser-exact-dist/s2-03-list-open-library.integration.test.ts | MERGE |
| apps/web/test/browser-exact-dist/s2-04-rename-project.integration.test.ts | KEEP |
| apps/web/test/browser-exact-dist/s2-05-archive-project.integration.test.ts | KEEP |
| apps/web/test/browser-exact-dist/s2-07-create-volume.integration.test.ts | MERGE |
| apps/web/test/browser-exact-dist/s2-08-create-chapter.integration.test.ts | KEEP |
| apps/web/test/browser-exact-dist/s2-10-rename-reorder-volumes.integration.test.ts | KEEP |

## Pending files

- apps/web/test/browser-exact-dist/inline-proposal.integration.test.ts
- apps/web/test/browser-exact-dist/production-captured-memory.integration.test.ts
- apps/web/test/browser-exact-dist/production-host.integration.test.ts
- apps/web/test/browser-exact-dist/production-run-evidence.integration.test.ts
- apps/web/test/browser-exact-dist/restored-discard.integration.test.ts
- apps/web/test/browser-exact-dist/s1-jrn-001.integration.test.ts
- apps/web/test/browser-exact-dist/s2-09-navigate-reopen.integration.test.ts
- apps/web/test/browser-exact-dist/s2-11-rename-reorder-chapters.integration.test.ts
- apps/web/test/browser-exact-dist/s2-12-current-chapter.integration.test.ts
- apps/web/test/browser-exact-dist/s2-13-delete-chapter.integration.test.ts
- apps/web/test/browser-exact-dist/s2-14-delete-volume.integration.test.ts
- apps/web/test/browser-exact-dist/s2-editor.integration.test.ts
- apps/web/test/browser-exact-dist/s2-frequent-undo.integration.test.ts
- apps/web/test/browser-exact-dist/s2-input.integration.test.ts
- apps/web/test/browser-exact-dist/s2-interruption.integration.test.ts
- apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts
- apps/web/test/browser-exact-dist/s2-long-session.integration.test.ts
- apps/web/test/browser-exact-dist/s2-move-retype.integration.test.ts
- apps/web/test/browser-exact-dist/s2-physical-drill.integration.test.ts
- apps/web/test/browser-exact-dist/s2-readable-export.integration.test.ts
- apps/web/test/browser-exact-dist/s2-save-truth.integration.test.ts
- apps/web/test/browser-exact-dist/s2-search.integration.test.ts
- apps/web/test/browser-exact-dist/s2-split-join.integration.test.ts
- apps/web/test/browser-exact-dist/s2-statistics.integration.test.ts
- apps/web/test/browser-exact-dist/s2-sustained-writing.integration.test.ts
- apps/web/test/browser-exact-dist/s2-undo.integration.test.ts
- apps/web/test/browser-exact-dist/s2-workspace.integration.test.ts
- apps/web/test/browser-exact-dist/stage1-journey-expectation.ts

## Decisions and evidence

- DELETE exact-dist-foundation:22: it tests the Vite-only serving plugin and browser-native realm/storage behavior. Actual product-host bytes belong to protocol-http-host:106. Equal root/Project responses and correct lengths do not establish equality with a build artifact. The Stage 1 journey also observes real editor reload; no covering product test is claimed for an isolated plugin or synthetic realm-marker mutation.
- MERGE production-page, bootstrap-Challenge and empty-Project surface assertions into Create Chapter:43 before creation/Volume creation. It already loads the packaged root, creates a Project and obtains a real protected Challenge through the form. Keep the transfer prerequisite; do not count the files as directly removable yet.
- MERGE empty library reopen's tree assertion into Project rename:46, and first-Volume empty/zero-Chapter checks into Volume rename/reorder:65. Both receiving cases already perform the relevant real setup.
- KEEP Project rename, archived disabled-open, relative Chapter creation and Volume rename/reorder at their real UI boundary. HTTP persistence cannot detect wrong menu targets, stale library rendering or inline placement/cancel behavior.
- Archive is performed with a directly imported wrapper; its retained observation is the disabled archived library button. Old form/textarea absence selectors do not establish that all current write menus are disabled. Chapter menu fixed 12px font and screenshot output are not separate KEEP grounds.
- exact-dist-plugin.ts and inline-chapter-creation.ts are fully read and have support dispositions. exact-dist-global-setup.ts has been read only through line 145; finish it before marking support complete. Its optional authority oracle has dependencies on production-host/prose/Inline/Memory/Run/composer/multi-location fixtures; do not remove those tests before checking all counters.
- Read-ahead: s2-jrn-001.integration.test.ts is fully read but not adjudicated; compare its aggregate workflow to focused search/statistics/export/Chapter tests. s1-jrn-001 has only imports and the test body (307-439) reviewed; helpers remain. stage1-journey-expectation.ts has only lines 1-160 reviewed; remaining normalization and expected objects remain.
- All completed declarations and cited starts checked. No runtime test, mutation or product/test change occurred.
