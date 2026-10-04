# browser-exact-dist test verdicts

Reviewed: 32 cases in 31 files. See PROGRESS.md for directory completion.

Reason codes: [METHOD.md](METHOD.md). Locations use the fixed audit baseline.

## apps/web/test/browser-exact-dist/exact-dist-foundation.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD001 | 22 — serves untransformed dist bytes in a reloadable same-origin child frame | DELETE | D4 | The product-host byte and resource contract is covered by the real HTTP host test, and actual editor reload is exercised by the Stage 1 journey. This case instead checks the test-only exactDistPlugin plus native iframe realm/sessionStorage semantics. Root/Project byte equality and Content-Length do not compare either response with a built artifact, so the title overstates untransformed-byte proof. No product covering test is claimed for a mutation confined to the Vite test plugin or the artificial realm marker. | apps/web/test/node-postgresql/protocol-http-host.integration.test.ts:106; apps/web/test/browser-exact-dist/s1-jrn-001.integration.test.ts:307 |

## apps/web/test/browser-exact-dist/production-page.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD002 | 34 — loads the exact Vite production page in Google Chrome and shows the Stage 1 surface | MERGE | M1 | Move protected-ready heading/message, no alert and no premature editor/AI controls into the Create Chapter journey before it creates the Project. Its iframe already loads the same packaged root. The explicit Chrome user-agent check tests runner selection; byte/header smoke belongs to the retained real-host HTTP test. Remove this standalone page fixture only after the initial-surface assertions transfer. | apps/web/test/browser-exact-dist/s2-08-create-chapter.integration.test.ts:43; apps/web/test/node-postgresql/protocol-http-host.integration.test.ts:106 |

## apps/web/test/browser-exact-dist/s2-01-bootstrap-challenge.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD003 | 48 — a new local browser session reaches protected-ready without AI and acquires createProjectChallenge | MERGE | M1 | The Create Chapter journey starts through the same root bootstrap and obtains a real Create Project Challenge on form submission. Transfer its root no-alert/no-AI surface checks, including MCP absence, into that fixture before creation; the production-page transfer supplies the rest of the initial surface. The extra direct generated-client Challenge call and static nonce/profile shape checks add no separate browser interaction. Removal depends on the surface transfer. | apps/web/test/browser-exact-dist/s2-08-create-chapter.integration.test.ts:43; apps/web/test/browser-exact-dist/production-page.integration.test.ts:34 |

## apps/web/test/browser-exact-dist/s2-02-create-empty-project.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD004 | 34 — the protected browser creates one empty workspace without a starter Chapter | MERGE | M1 | The Create Chapter journey already creates an empty Project before any Volume. Add its exact Project heading, empty tree and empty-workspace/no-starter content checks at that point, then remove this duplicate bootstrap/create fixture. Its textarea absence is obsolete as a general editor absence check because production uses Tiptap; preserve the actual empty-tree/current-state behavior. | apps/web/test/browser-exact-dist/s2-08-create-chapter.integration.test.ts:43 |

## apps/web/test/browser-exact-dist/s2-03-list-open-library.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD005 | 34 — the current User library lists owned Projects and opens an empty Project from getProject | MERGE | M1 | Rename Project already creates an empty Project, reloads the real library, checks scoped/no-secret results and opens the same Project with the current title. Transfer the final empty-tree assertion into that case before removal; this case has no divergent list/query title (the retained source test does). Do not retain a second complete fixture only for unchanged title versus renamed title. | apps/web/test/browser-exact-dist/s2-04-rename-project.integration.test.ts:46; apps/web/test/browser-source/list-open.integration.test.ts:11 |

## apps/web/test/browser-exact-dist/s2-04-rename-project.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD006 | 46 — the author renames one exact Project and the library plus opened title converge | KEEP | K2 | Submitting the real Project rename form must update the opened heading and the newly loaded library, then open the renamed Project correctly. HTTP rename observes durable query values but does not execute menu/form wiring or library refresh; the source historical-acknowledgement test returns an error and never proves successful packaged rename. Receive the empty-tree check from the library-only case. | apps/web/test/node-postgresql/update-project-http.integration.test.ts:294; apps/web/test/browser-source/rename-historical-acknowledgement.integration.test.ts:15; apps/web/test/browser-exact-dist/s2-03-list-open-library.integration.test.ts:34 |

## apps/web/test/browser-exact-dist/s2-05-archive-project.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD007 | 47 — the library fails closed on open and write for an archived Project | KEEP | K2 | The reloaded library must render an archived Project as a disabled open button and stay protected-ready when clicked. HTTP Archive refuses commands but cannot detect an enabled stale library entry. The fixture archives through an imported command wrapper, not the packaged Archive UI; absent form[data-rename]/form[data-archive] and textarea selectors do not prove all current menu/write controls are disabled. Retain only the actual disabled-open regression. | apps/web/test/node-postgresql/archive-project-http.integration.test.ts:431; apps/web/test/browser-source/archive-historical-acknowledgement.integration.test.ts:11 |

## apps/web/test/browser-exact-dist/s2-07-create-volume.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD008 | 35 — the author creates one named Volume from the empty manuscript workspace | MERGE | M1 | Volume rename/reorder already creates the same first named Volume and checks the exact single-item title list. Transfer the first-Volume empty-project state and zero child-Chapter assertion there before creating its second Volume, then remove this repeated Project/Volume creation fixture. Repeated textarea and AI-text absences do not justify a separate scenario. | apps/web/test/browser-exact-dist/s2-10-rename-reorder-volumes.integration.test.ts:65 |

## apps/web/test/browser-exact-dist/s2-08-create-chapter.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD009 | 43 — the author creates Chapters at inline menu positions and keeps the first current Chapter | KEEP | K2 | Pointer versus overflow Chapter menus must offer the correct before/after placements, put the title editor between exact siblings, cancel on Escape without insertion, choose the default last Volume versus an explicit Volume, and retain the first Current Chapter and order after reload. HTTP ordering never executes this menu position/selection wiring; ordinary journey setup only appends. Keep those behaviors, not the fixed 12px styling or screenshot creation as independent regressions. Receive the root/empty-Project assertion transfers. | apps/web/test/node-postgresql/create-chapter-http.integration.test.ts:1002; apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts:130 |

## apps/web/test/browser-exact-dist/s2-09-navigate-reopen.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD011 | 95 — the author opens each Chapter from the tree and reopens the current Chapter | MERGE | M1 | Its distinct input is conflicting localStorage, query and hash Chapter hints on reopen: the server Current Chapter must win. Transfer those hints into save-truth before its final reopen, pointing them to inspected Chapter A while B is current. That receiver already inspects non-current A read-only, retains both Chapters' prose and reopens B. Three empty-Chapter clicks add no distinct ownership rule; removal depends on preserving the conflicting-hint input. | apps/web/test/browser-exact-dist/s2-save-truth.integration.test.ts:77; apps/web/test/browser-source/chapter-navigation.integration.test.ts:38 |

## apps/web/test/browser-exact-dist/s2-10-rename-reorder-volumes.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD010 | 65 — the author renames and reorders Volumes from the canonical tree and they survive reopen | KEEP | K2 | The packaged Volume menu must rename one exact Volume, reopen its persisted title, move the first Volume down and reopen the exact new order. Core/HTTP ordering cannot detect stale menu target/form wiring or a display order that diverges after reload; Chapter menu updates have separate component/command paths. Receive first-Volume empty-workspace/zero-Chapter checks before creating Volume B. | apps/web/test/node-postgresql/update-volume-http.integration.test.ts:196; apps/web/test/browser-exact-dist/s2-07-create-volume.integration.test.ts:35 |

## apps/web/test/browser-exact-dist/s2-11-rename-reorder-chapters.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD012 | 91 — the author renames and reorders Chapters from the canonical tree and they survive reopen | KEEP | K2 | The actual Chapter menu/form must rename the selected Current Chapter, preserve that identity while moving it down, and reopen with the renamed Chapter still current although it is no longer first. Volume reorder has a separate menu/command and no current-editor binding; HTTP Chapter update cannot detect incorrect rendered heading/current selection after reorder. | apps/web/test/browser-exact-dist/s2-10-rename-reorder-volumes.integration.test.ts:65; apps/web/test/node-postgresql/update-chapter-http.integration.test.ts:252 |

## apps/web/test/browser-exact-dist/s2-12-current-chapter.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD013 | 84 — writes in two Chapters, switches current Chapter, and reopens the current Chapter | MERGE | M1 | The save-truth case already types Alpha/Beta in two Chapters, makes B current, inspects A read-only and reopens B. Transfer the exact second Chapter GET, reopened editability and current-selection menu observations into that case. Check current-menu visibility only while the relevant menu is open; current absence checks can otherwise pass because a menu is closed. Keep the transfer prerequisite rather than maintaining the same create/type/switch fixture twice. | apps/web/test/browser-exact-dist/s2-save-truth.integration.test.ts:77; apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts:130 |

## apps/web/test/browser-exact-dist/s2-13-delete-chapter.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD014 | 181 — the author confirms Chapter removal, keeps the next current Chapter, then opens empty | KEEP | K2 | After confirming removal of a written Current Chapter, the packaged editor must bind the next Chapter, become writable with no old pending input, repeat that transition, and disappear after the last Chapter is deleted. HTTP deletion checks tree/head rules but cannot catch stale editor/controller attachment after the delete form completes. Statistics deletion does not by itself establish this repeated writable-successor-to-empty transition. | apps/web/test/node-postgresql/delete-chapter-http.integration.test.ts:571; apps/web/test/browser-exact-dist/s2-14-delete-volume.integration.test.ts:66 |

## apps/web/test/browser-exact-dist/s2-14-delete-volume.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD015 | 66 — the author cannot remove a nonempty Volume, then removes an empty Volume | KEEP | K2 | A confirmed nonempty Volume removal must show the refusal while retaining both rows; a following empty-Volume removal must succeed and clear that prior error. Chapter deletion uses another form and command, while HTTP Volume refusal cannot detect a stale error left in the mounted UI after a later success. | apps/web/test/node-postgresql/delete-volume-http.integration.test.ts:517; apps/web/test/browser-exact-dist/s2-13-delete-chapter.integration.test.ts:181 |

## apps/web/test/browser-exact-dist/s2-editor.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD017 | 77 — hydrates production Tiptap for one paragraph Block without a textarea write path | MERGE | M1 | Move initial no-editor state and first paragraph/Block identity, editability and zero-pending hydration checks into Create Chapter:43, which already transitions from an empty Project to its first packaged editor. The Ctrl+B check runs on an empty document with no subsequent typing, so enabled stored marks could still produce no strong/b element and no text change; it does not establish unsupported formatting rejection. Do not preserve a separate fixture for that masked assertion. | apps/web/test/browser-exact-dist/s2-08-create-chapter.integration.test.ts:43; apps/web/test/browser-exact-dist/s2-input.integration.test.ts:284 |

## apps/web/test/browser-exact-dist/s2-frequent-undo.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD019 | 69 — keeps the editor writable through frequent Author Undo between Author Edit batches | KEEP | K2 | Repeated real Edit/Edit/Undo cycles must keep the packaged editor writable and create exactly 24 Author Edit and 12 Undo requests with only successful Challenge responses, without resetting quota windows. Single Undo covers one cycle, the long-session journey performs one Undo and explicitly resets quotas, and source tests mock network admission. Retain continued-cycle/frontier behavior; elapsed time is logged but has no threshold, so this does not prove 12 Undos occurred inside one minute. Receive exact Block/frontier/reload assertions from the single-Undo case. | apps/web/test/browser-exact-dist/s2-undo.integration.test.ts:162; apps/web/test/browser-exact-dist/s2-long-session.integration.test.ts:123; apps/web/test/browser-source/author-undo-rate-limit.integration.test.ts:143 |

## apps/web/test/browser-exact-dist/s2-input.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD031 | 188 — consumes real assistance and Run Activity before the author continues writing | KEEP | K2 | The packaged Activity consumer must advance its rendered cursor through real assistance_updated, agent_run_created and Author Edit events, then allow another saved edit. Mock SSE tests cannot detect drift between real Server event bytes and this mounted consumer; enabled composer journeys do not assert this exact stream/cursor sequence. The optional database teardown also expects these two fixed-key assistance/Run receipts. Preserve that fixture dependency during any future consolidation. | apps/web/test/browser-source/activity-stream-consume.integration.test.ts:105; apps/web/test/browser-source/activity-stream-consume.integration.test.ts:322; apps/web/test/browser-exact-dist/production-host.integration.test.ts:10 |
| BD032 | 284 — settles IME, clipboard, drop, and contiguous Block replacement without reusing identity | KEEP | K2 | Two complete multiline replacements must allocate distinct new right Block identities; cross-Block cut must remove the old right identity; IME cancellation/confirmation and drop must persist the resulting text across reload. Single paste in move/retype cannot detect reusing an earlier replacement identity, and repeated composition/split writing never performs a cross-Block selected cut. Drop is a synthesized DragEvent with DataTransfer, while clipboard commands drive the actual browser; do not label it native drag gesture coverage. | apps/web/test/browser-exact-dist/s2-move-retype.integration.test.ts:184; apps/web/test/browser-exact-dist/s2-sustained-writing.integration.test.ts:74; apps/web/test/browser-source/local-edit-journal-append-projection.integration.test.ts:156 |

## apps/web/test/browser-exact-dist/s2-interruption.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD018 | 95 — recovers Local Edit Journal text after reload without a second Author Edit | MERGE | M1 | Transfer the repeated reload and complete authoritative Revision equality check into Stage 1 journey after its recovered saved state. That journey already observes a retained pending Journal intent before reloading and settles the same visible text. This test accepts either pending or already saved before its supposed interruption, so it can exercise only ordinary reload. It counts no POSTs; unchanged Revision on the final reload proves no new authoritative revision, not necessarily no duplicate request. | apps/web/test/browser-exact-dist/s1-jrn-001.integration.test.ts:307; apps/web/test/browser-source/reload-recovery.integration.test.ts:68 |

## apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD028 | 130 — runs the AI-disabled production journey without losing Chapter work | MERGE | M1 | Its two-Chapter write/reopen, single positive search, ready statistics and one export are covered more precisely by the focused save/search/statistics/export tests. Carry its exact final GET equality for both Chapters into save-truth and the unavailable-assistant/no-AI presentation checks into readable export (workspace already checks the unavailable assistant). Then retire this second aggregate setup or point its release evidence at those retained cases. Preserve the product release gate; no gate change is part of this audit. | apps/web/test/browser-exact-dist/s2-save-truth.integration.test.ts:77; apps/web/test/browser-exact-dist/s2-search.integration.test.ts:127; apps/web/test/browser-exact-dist/s2-statistics.integration.test.ts:120; apps/web/test/browser-exact-dist/s2-readable-export.integration.test.ts:115; apps/web/test/browser-exact-dist/s2-workspace.integration.test.ts:65 |

## apps/web/test/browser-exact-dist/s2-long-session.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD020 | 123 — repeats Chapter switching, Undo, search, and reload without losing work | KEEP | K2 | After 2401 individual trusted inputs, the packaged editor must complete automatic batching/collection, switch Chapters, reload and save another input without losing either Chapter. The source working-set test calls persist/collect directly and cannot detect broken mounted-controller collection or post-reload continued input. Search/statistics smoke is incidental. This journey resets Challenge windows and logs timings with no performance thresholds; it proves neither sustained quota behavior nor a latency/RPO/RTO bound. | apps/web/test/browser-source/journal-working-set.integration.test.ts:16; apps/web/test/browser-exact-dist/s2-sustained-writing.integration.test.ts:74; apps/web/test/browser-exact-dist/s2-save-truth.integration.test.ts:77 |

## apps/web/test/browser-exact-dist/s2-move-retype.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD029 | 184 — moves and retypes Blocks with stable identity and refuses copy as a move | KEEP | K2 | A real move shortcut must reorder the selected Block without changing either identity, while later multiline paste must mint a third identity and retype must change only the selected moved Block's kind. The final reload checks all identities/kinds/text together. Core primitives and split/join do not execute selection-to-move/retype key binding. Despite the title, no copy-as-move request is refused; the actual copy protection is a newly allocated pasted Block identity. | apps/web/test/browser-exact-dist/s2-split-join.integration.test.ts:179; apps/web/test/browser-exact-dist/s2-input.integration.test.ts:284 |

## apps/web/test/browser-exact-dist/s2-physical-drill.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD026 | 89 — writes Chinese and English after restore, settles, reloads, and keeps isolation | KEEP | K2 | The physical-restore runner replays a post-backup WAL title, opens that restored Project through the real library, continues writing, reloads and checks isolation from foreign/stale fixture content. Ordinary new-Project journeys cannot detect inability to resume with restored Session/base/storage state. vitest.config selects this file only in the restore suite, and verify-recovery-hold.sh checks the WAL marker before invoking it. This source audit did not execute the physical drill. | apps/web/test/browser-exact-dist/s2-save-truth.integration.test.ts:77; apps/web/test/browser-exact-dist/s2-long-session.integration.test.ts:123 |

## apps/web/test/browser-exact-dist/s2-readable-export.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD025 | 115 — exports a durable human-readable manuscript through the Worker | KEEP | K2 | After one ready export, creating and editing a second Chapter then requesting another must render a new export identity and exact updated manuscript bytes with a download control. This catches a panel reusing the earlier ready result. HTTP pinned-source tests own durable capture/output but not browser polling/render identity; the aggregate journey requests only one export. A visible download button is not proof that downloading works. | apps/web/test/node-postgresql/readable-export-pinned-source-http.integration.test.ts:124; apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts:130 |

## apps/web/test/browser-exact-dist/s2-save-truth.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD016 | 77 — shows pending, saving, and saved without calling local input saved, across Chapters | KEEP | K2 | The first sampled state with newly visible local text must be pending/saving rather than saved, and later saved labels must follow settled prose through current-Chapter change, read-only inspection and reopen. The ordinary current-Chapter case waits for saving eventually but does not reject saved at this first sample. This is a sampled observation with no held server response; it does not prove that every frame remains unsaved until acknowledgement. Receive conflicting reopen hints and remaining two-Chapter query/editability checks from navigation/current-Chapter duplicates. | apps/web/test/browser-exact-dist/s2-12-current-chapter.integration.test.ts:84; apps/web/test/browser-exact-dist/s1-jrn-001.integration.test.ts:307 |

## apps/web/test/browser-exact-dist/s2-search.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD023 | 127 — searches the current Chapter and manuscript with bounded Snapshot identity | KEEP | K2 | The packaged search radio/form must distinguish current-Chapter misses from manuscript hits, render exact Chapter/Block/range identities, clear prior hits for no-match, and exclude a deleted Chapter on later search while preserving another Chapter's hit. HTTP query logic cannot detect a radio value ignored by the client or stale result DOM; the aggregate journey checks only one positive hit. | apps/web/test/node-postgresql/manuscript-search-http.integration.test.ts:78; apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts:130 |

## apps/web/test/browser-exact-dist/s2-split-join.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD030 | 179 — splits and joins adjacent Blocks and reopens the same identities | KEEP | K2 | Splitting, reloading the two stored Blocks, then Backspace-joining a nonempty right Block must retain the left identity and complete concatenated text through another reload. Sustained writing joins an empty newly split Block; source adapter join does not persist/reload and is DELETE. This case protects actual hydrated Block identity plus the right text after join; middle-of-text splitting remains a distinct source input. | apps/web/test/browser-exact-dist/s2-sustained-writing.integration.test.ts:74; apps/web/test/browser-source/manuscript-tiptap-adapter.integration.test.ts:55; apps/web/test/browser-source/manuscript-tiptap-adapter.integration.test.ts:110 |

## apps/web/test/browser-exact-dist/s2-statistics.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD024 | 120 — rebuilds Chapter and manuscript statistics after edit, switch, and deletion | KEEP | K2 | The mounted statistics panel must refresh exact Chapter versus manuscript counts after editing a different Chapter and deleting that Current Chapter. Distinct word/character totals expose swapped fields or stale selection/aggregate display. Core Unicode golden cases own counting semantics but do not execute panel refresh, and the aggregate/long journeys check only lag and Snapshot shape. | crates/storyos-core/src/statistics_profile_tests.rs:4; apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts:130; apps/web/test/browser-exact-dist/s2-long-session.integration.test.ts:123 |

## apps/web/test/browser-exact-dist/s2-sustained-writing.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD022 | 74 — saves sustained Chinese composition, idle pauses, and Block boundaries without a quota reset | KEEP | K2 | Repeated Chinese composition, punctuation, split and join must yield exactly 50 Author Edit requests, successful Challenge responses and exact final text without quota-window resets. Packaged input exercises only a few such operations; long-session explicitly resets quota windows and uses plain characters, while frequent Undo exercises a different command family. This case detects additional composition submissions or boundary edits that exhaust real admission during sustained writing. Runtime measurements are logged, not a declared latency target. | apps/web/test/browser-exact-dist/s2-input.integration.test.ts:284; apps/web/test/browser-exact-dist/s2-long-session.integration.test.ts:123; apps/web/test/browser-exact-dist/s2-frequent-undo.integration.test.ts:69 |

## apps/web/test/browser-exact-dist/s2-undo.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD021 | 162 — undoes one exact admitted Author Action and restores identity after reload | MERGE | M1 | Frequent Undo already types two distinct settled edits, reverses only the latest and continues editing. Transfer the exact stable Block identity, before/after undo frontier and final reload/restored frontier checks into a selected cycle and final reload of that case. The current receiver checks text/request counts but not these identities; complete the transfer before removing this duplicate setup. | apps/web/test/browser-exact-dist/s2-frequent-undo.integration.test.ts:69 |

## apps/web/test/browser-exact-dist/s2-workspace.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD027 | 65 — the production page uses the approved workspace without losing writing state | KEEP | K2 | Collapsing and reopening the unavailable assistant beside newly typed text must preserve the same writable editor, Chapter and writer generation; disabled composer submission must stay idle and further typing must work. Other journeys use the composer enabled or never collapse it. Fixed width/font and repeated label/selector inventories are not separate KEEP reasons. The test observes DOM/state continuity, not a held unsaved interval or final server persistence. | apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts:130; apps/web/test/browser-exact-dist/s2-save-truth.integration.test.ts:77 |
