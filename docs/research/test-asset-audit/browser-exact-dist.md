# browser-exact-dist test verdicts

Reviewed: 10 cases in 10 files. See PROGRESS.md for directory completion.

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

## apps/web/test/browser-exact-dist/s2-10-rename-reorder-volumes.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BD010 | 65 — the author renames and reorders Volumes from the canonical tree and they survive reopen | KEEP | K2 | The packaged Volume menu must rename one exact Volume, reopen its persisted title, move the first Volume down and reopen the exact new order. Core/HTTP ordering cannot detect stale menu target/form wiring or a display order that diverges after reload; Chapter menu updates have separate component/command paths. Receive first-Volume empty-workspace/zero-Chapter checks before creating Volume B. | apps/web/test/node-postgresql/update-volume-http.integration.test.ts:196; apps/web/test/browser-exact-dist/s2-07-create-volume.integration.test.ts:35 |
