# Browser exact-dist checkpoint

Source review is partial: 35 cases in 34 test files; 21 KEEP, three DELETE, 11 MERGE. Direct test candidate lines: 83. Exclusive support candidate lines: 434 (support-removals.json).

| Reviewed file | Verdict |
|---|---|
| apps/web/test/browser-exact-dist/exact-dist-foundation.test.ts:22 | DELETE |
| apps/web/test/browser-exact-dist/production-page.integration.test.ts:34 | MERGE |
| apps/web/test/browser-exact-dist/s2-01-bootstrap-challenge.integration.test.ts:48 | MERGE |
| apps/web/test/browser-exact-dist/s2-02-create-empty-project.integration.test.ts:34 | MERGE |
| apps/web/test/browser-exact-dist/s2-03-list-open-library.integration.test.ts:34 | MERGE |
| apps/web/test/browser-exact-dist/s2-04-rename-project.integration.test.ts:46 | KEEP |
| apps/web/test/browser-exact-dist/s2-05-archive-project.integration.test.ts:47 | KEEP |
| apps/web/test/browser-exact-dist/s2-07-create-volume.integration.test.ts:35 | MERGE |
| apps/web/test/browser-exact-dist/s2-08-create-chapter.integration.test.ts:43 | KEEP |
| apps/web/test/browser-exact-dist/s2-10-rename-reorder-volumes.integration.test.ts:65 | KEEP |
| apps/web/test/browser-exact-dist/s2-09-navigate-reopen.integration.test.ts:95 | MERGE |
| apps/web/test/browser-exact-dist/s2-11-rename-reorder-chapters.integration.test.ts:91 | KEEP |
| apps/web/test/browser-exact-dist/s2-12-current-chapter.integration.test.ts:84 | MERGE |
| apps/web/test/browser-exact-dist/s2-13-delete-chapter.integration.test.ts:181 | KEEP |
| apps/web/test/browser-exact-dist/s2-14-delete-volume.integration.test.ts:66 | KEEP |
| apps/web/test/browser-exact-dist/s2-save-truth.integration.test.ts:77 | KEEP |
| apps/web/test/browser-exact-dist/s2-editor.integration.test.ts:77 | MERGE |
| apps/web/test/browser-exact-dist/s2-interruption.integration.test.ts:95 | MERGE |
| apps/web/test/browser-exact-dist/s2-frequent-undo.integration.test.ts:69 | KEEP |
| apps/web/test/browser-exact-dist/s2-long-session.integration.test.ts:123 | KEEP |
| apps/web/test/browser-exact-dist/s2-undo.integration.test.ts:162 | MERGE |
| apps/web/test/browser-exact-dist/s2-sustained-writing.integration.test.ts:74 | KEEP |
| apps/web/test/browser-exact-dist/s2-search.integration.test.ts:127 | KEEP |
| apps/web/test/browser-exact-dist/s2-statistics.integration.test.ts:120 | KEEP |
| apps/web/test/browser-exact-dist/s2-readable-export.integration.test.ts:115 | KEEP |
| apps/web/test/browser-exact-dist/s2-physical-drill.integration.test.ts:89 | KEEP |
| apps/web/test/browser-exact-dist/s2-workspace.integration.test.ts:65 | KEEP |
| apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts:130 | MERGE |
| apps/web/test/browser-exact-dist/s2-move-retype.integration.test.ts:184 | KEEP |
| apps/web/test/browser-exact-dist/s2-split-join.integration.test.ts:179 | KEEP |
| apps/web/test/browser-exact-dist/s2-input.integration.test.ts:188 | KEEP |
| apps/web/test/browser-exact-dist/s2-input.integration.test.ts:284 | KEEP |
| apps/web/test/browser-exact-dist/s1-jrn-001.integration.test.ts:307 | KEEP |

## Pending files

- apps/web/test/browser-exact-dist/inline-proposal.integration.test.ts
- apps/web/test/browser-exact-dist/production-captured-memory.integration.test.ts
- apps/web/test/browser-exact-dist/production-host.integration.test.ts
- apps/web/test/browser-exact-dist/production-run-evidence.integration.test.ts
- apps/web/test/browser-exact-dist/restored-discard.integration.test.ts

## Decisions and evidence

- DELETE exact-dist-foundation:22: it tests the Vite-only serving plugin and browser-native realm/storage behavior. Actual product-host bytes belong to protocol-http-host:106. Equal root/Project responses and correct lengths do not establish equality with a build artifact. The Stage 1 journey also observes real editor reload; no covering product test is claimed for an isolated plugin or synthetic realm-marker mutation.
- MERGE production-page, bootstrap-Challenge and empty-Project surface assertions into Create Chapter:43 before creation/Volume creation. It already loads the packaged root, creates a Project and obtains a real protected Challenge through the form. Keep the transfer prerequisite; do not count the files as directly removable yet.
- MERGE empty library reopen's tree assertion into Project rename:46, and first-Volume empty/zero-Chapter checks into Volume rename/reorder:65. Both receiving cases already perform the relevant real setup.
- KEEP Project rename, archived disabled-open, relative Chapter creation and Volume rename/reorder at their real UI boundary. HTTP persistence cannot detect wrong menu targets, stale library rendering or inline placement/cancel behavior.
- Archive is performed with a directly imported wrapper; its retained observation is the disabled archived library button. Old form/textarea absence selectors do not establish that all current write menus are disabled. Chapter menu fixed 12px font and screenshot output are not separate KEEP grounds.
- exact-dist-plugin.ts and inline-chapter-creation.ts are fully read and have support dispositions. exact-dist-global-setup.ts is fully reviewed and recorded in SUPPORT.md. Its optional authority oracle has exact dependencies on production-host/prose/Inline/Memory/Run/composer/multi-location fixtures; do not remove those tests before reconciling the counters.
- Read-ahead: s2-jrn-001 is now adjudicated as MERGE (BD028). s1-jrn-001 and stage1-journey-expectation.ts are now fully reviewed (BD033 and SUPPORT.md).
- All completed declarations and cited starts checked. No runtime test, mutation or product/test change occurred.

## Chapter operations and save display

- Added BD011-BD016: navigation, Chapter rename/reorder, Current Chapter, Chapter deletion, Volume deletion and save truth. All six full source files reviewed.
- MERGE navigation's conflicting localStorage/query/hash hints into save-truth before final reopen. Point them to inspected A while B is current. The receiver already checks actual prose and non-current inspection, so three extra empty Chapters are not required.
- MERGE Current Chapter's remaining exact B query/editability/menu assertions into save-truth. Its two-Chapter create/type/switch/inspect/reload sequence is otherwise the same. Require an open menu before asserting that its current-selection action is absent; closed menus can mask that check.
- KEEP Chapter rename/reorder for current identity and heading after the renamed Chapter moves away from first position; KEEP repeated current deletion for fresh writable successor and final no-editor state; KEEP Volume refusal then success for retained rows and error clearing.
- KEEP save-truth for the first sampled local-text state. It has no held response and is not evidence that every frame stays unsaved until acknowledgement. This limitation must remain when its receiving fixture is consolidated.
- Current totals: 16 cases/files, eight KEEP, one DELETE, seven MERGE; 67 direct lines. No runtime execution or source mutation.

## Editor hydration, reload and sustained input

- BD017-BD022 add editor hydration, interruption, frequent Undo, long session, single Undo and sustained writing. All six full files reviewed.
- MERGE hydration into first Chapter creation; the empty-document Ctrl+B check cannot detect an enabled stored bold mark and is not a KEEP reason.
- MERGE repeated-reload exact Revision equality into Stage 1's recovery journey. Interruption:95 permits either pending or saved before reload, and never counts POSTs. It can pass without an actual unsettled cut; unchanged final Revision is weaker than no repeated request.
- KEEP frequent Undo for real repeated Edit/Edit/Undo and exact request counts without quota resets. Transfer single-Undo Block identity/frontier and final reload observations into that receiver before removing the separate test. Twelve rounds do not prove all Undos happened in one rate-limit minute: elapsed time is logged but not bounded to that window.
- KEEP long-session for 2401 trusted inputs through mounted automatic collection, Chapter switching and continued writing after reload. Source working-set directly invokes collection. Long-session explicitly resets rate windows and has no asserted latency/RPO/RTO target.
- KEEP sustained writing for 50 real submissions through repeated IME, punctuation, split and join without quota reset. It complements Undo's different command family and the long-session fixture's reset windows. Do not count printed timings as performance assertions.
- Current totals: 22 cases/files, 11 KEEP, one DELETE, 10 MERGE; 67 direct candidate lines. No runtime test or source mutation.

## Derived panels, physical restore and aggregate journey

- BD023-BD028 add search, statistics, readable export, physical drill, workspace and the Stage 2 aggregate journey. All six full files reviewed.
- KEEP search scope selection, precise result identity and deletion refresh; statistics field/Chapter/aggregate refresh; and a second export with new identity and newly rendered bytes. HTTP/Core tests cannot detect stale panel state or wrong client selection. Visible download control is not executed download coverage.
- KEEP unavailable-assistant collapse/reopen with current text and continued typing. Its pixel width/font and label inventories are not independent regressions; no held pre-settlement interval is proved.
- KEEP physical drill: verify-recovery-hold.sh validates the post-backup WAL title before running the restore-only browser selection. The test consumes that actual restored context and continues writing. No physical recovery was executed during this audit.
- MERGE the Stage 2 aggregate journey into the focused retained cases, transferring final two-Chapter GET equality to save-truth and unavailable/no-AI presentation to readable export. Preserve release-gate evidence when implementing consolidation; this audit changes no gate.
- exact-dist-global-setup.ts is fully reviewed. With STORYOS_STAGE1_AUTHORITY_ORACLE=1, its required teardown compares exact production-host and assistance scenario counters, Stage 1's four edits/activities/actions and no foreign or unexpected receipts. Production tests/support remain pending; their fixture counts cannot be silently dropped.
- Current totals: 28 cases/files, 16 KEEP, one DELETE, 11 MERGE; 67 direct candidate lines. No runtime execution or source mutation.

## Block identity and real Activity consumer

- BD029-BD032 add move/retype, split/join and both input cases. All three files fully reviewed (32 cases in 31 files total).
- KEEP move/retype selection wiring and stable identities across reload. Its title says copy is refused as a move, but the body actually performs a paste and checks a new identity; no explicit refusal occurs.
- KEEP nonempty right-Block join after rehydration. Sustained writing joins an empty new Block; source adapter duplicate does not persist. Middle split remains a separate source KEEP input.
- KEEP repeated multiline replacement identity allocation, cross-Block cut, composition and reload. Drop uses a synthetic DragEvent, not a native drag gesture.
- KEEP real assistance/Run/Author Edit cursor convergence followed by another saved edit. The optional database oracle requires the fixed f802/f804 receipts from this case; mocked stream tests cannot replace the actual wire/consumer pair.
- Current exact-dist counts: 32 cases in 31 files, 20 KEEP, one DELETE, 11 MERGE; 67 direct candidate lines. No runtime execution, product edit or temporary mutation.

## Stage 1 and production command routing

- BD033 keeps Stage 1's real cross-store/Activity/collection relationships across retained-input reload. Chinese input is direct insertion, not IME. Authority Receipts/effects in the in-browser object come from Journal settlements; the optional teardown supplies separate database counts. Normalization removes timestamp and command/coverage digest values; do not claim independent validation of those hashes.
- Stage 1 expected/normalization support, browser command client, privileged dispatcher and production-host-command.ts are fully read and recorded in SUPPORT.md. Wrappers are not empty assertions: they dispatch to these executing helpers.
- Read-ahead complete: all five remaining wrapper files (production-host, inline-proposal, captured-memory, run-evidence, restored-discard) are read; underlying scenario helpers remain except production-host-command.ts, which is fully reviewed. That helper's cold real-origin context checks issued HttpOnly/SameSite cookie, enforced Trusted Types/frame-ancestors, real replay-generation resync, held old-writer POST across Takeover, retained old local Journal and winner continued writing. The eventual production-host:5 verdict is KEEP.
- Reconcile manual-input:579 validated stale refusal against production-host:5: the latter already holds an actual old-writer POST across Takeover and checks the rendered read-only editor/local text. The other manual parameters have distinct unknown schema and earlier challenge/admission/terminal evidence. Do not change those verdicts until the covering production file is fully entered.
- Remaining helper work includes production-prose-request-command (routes prose/refused/conflict/restore) and its Draft helpers, Inline, captured Memory, Run evidence, composer and multi-location. No partial helper read is treated as a finished disposition.
- Current totals: 33 cases in 32 files, 21 KEEP, one DELETE, 11 MERGE; 67 direct candidate lines. No active process or temporary mutation.

## Direct API assertions behind browser titles

- BD034/BD035 DELETE captured Memory and Run evidence wrappers. Both full helper bodies are read (the truncated retrieval/cancel section was separately read). Their titles claim display/selection, but the evidence checks call generated getAgentRun directly after ordinary composer operations and reloads. No Memory/evidence DOM content is asserted.
- Memory's exact use=false/contribution=true current revision, old capture and withheld evidence are already in create-agent-run:554. Current versus historical withheld lookup follows the same exact revision join in create_agent_run_read.rs:146-150 and optional booleans :237-245. The real prose flow already checks same Conversation after reload/lost acknowledgement.
- Run evidence repeats retained HTTP cases for nonempty superseded Context, oversized exact input, compaction installed/refused, reference rebuilt/blocked/unknown, retained-result settled/missing/cancelled, successor dispatched/budget/fence-cancelled. Each corresponding public test is cited in BD035. Reload before a direct GET does not create an additional browser reader boundary.
- These deletions require removing exclusive command routes/imports and the corresponding exact-dist-global-setup expected Project/Receipt/action counter blocks. No product behavior is represented by keeping now-unused fixture counts. Preserve the other authority oracle groups.
- support-removals.json records 434 additional exclusive helper lines (177 Memory + 257 Run evidence). Include these spans, unioned by path, in future directory/top-file totals; they are not additional runtime tests or separate sample rows. Do not count shared dispatcher/teardown cleanup yet.
- acceptance.ts and production-composer-controls.ts are fully reviewed. Composer has real UI controls, held dispatch, lost steering acknowledgement plus exact reload retry, and read-only writer refusal; eventual production-host:30 verdict is KEEP. Other production-host scenarios still require the prose/mixed/conflict/multi helpers.
- Current source totals: 35 exact-dist cases / 34 files, 21 KEEP, three DELETE, 11 MERGE. Test spans 83 lines plus 434 support lines. No runtime mutation or active process.
