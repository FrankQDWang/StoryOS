# browser-source test verdicts

Reviewed: 19 cases in 15 files. See PROGRESS.md for directory completion.

Reason codes: [METHOD.md](METHOD.md). Locations use the fixed audit baseline.

## apps/web/test/browser-source/accept-block-proposal.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS013 | 12 — retries the same protected Acceptance after an unknown delivery | KEEP | K2 | Acceptance delivery must retain exact body/key/nonce across unknown and unreadable responses, recover a persisted in-flight Attempt, and refuse a competing decision. A terminal challenge problem must clear unresolved status while a 401 after earlier unknown delivery remains unresolved; known 403 must block further automatic submission. Real HTTP Acceptance tests do not run IndexedDB transport classification. Receive the valid-flight pending-projection checks from acceptance-journal before removing that direct helper case. | apps/web/test/browser-source/acceptance-journal.integration.test.ts:9; apps/web/test/node-postgresql/acceptance-refusal-http.integration.test.ts:14 |

## apps/web/test/browser-source/acceptance-journal.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS014 | 9 — rejects a foreign Acceptance flight before it can disappear from Journal recovery | MERGE | M1 | Move the valid flight's one intent/group, sequence and needs_attention/unsettled-count assertions into the first unresolved stage of the public Acceptance delivery case. Its createFlight callers construct key, partition and frozen command fields from the same workspace; this test's hidden key and foreign flight argument are fabricated internal tuples, not stored-partition drift. Do not keep those negative inputs as proof of realistic recovery corruption. Preserve the useful public pending-projection observations before deleting this fixture. | apps/web/test/browser-source/accept-block-proposal.integration.test.ts:12 |

## apps/web/test/browser-source/archive-historical-acknowledgement.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS001 | 11 — explains a historical Archive Project acknowledgement and does not retry it | KEEP | K2 | Archive's browser command wrapper must propagate historical acknowledgement unavailability after exactly one PUT/Challenge instead of entering its own automatic retry. Other command wrappers have separate catch/retry bodies, and real HTTP historical evidence tests call the generated client directly. Despite its title this case does not mount a UI or verify explanatory text; retain only the no-retry/protocol-classification claim. | apps/web/test/node-postgresql/archive-project-http.integration.test.ts:431; apps/web/test/browser-source/rename-historical-acknowledgement.integration.test.ts:15 |

## apps/web/test/browser-source/author-edit-outcome.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS012 | 14 — keeps the generated outcome Query proof header-only | KEEP | K2 | The executed generated outcome client must require proof and send it only in the anti-forgery header on an exact bodyless GET with same-origin credentials. Server tests exercise actual protected outcomes but do not reject an extra proof copy in the URL; static OpenAPI/schema shape does not execute the browser request construction. | apps/web/test/node-postgresql/project-http.integration.test.ts:253; crates/storyos-contracts/src/release1_artifacts_tests.rs:57 |

## apps/web/test/browser-source/browser-command-foundation.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS015 | 18 — uses typed input, clipboard, and Client Session commands in Google Chrome | DELETE | D4 | Trusted insertion, IME and clipboard behavior already execute through the packaged manuscript input journey using the same commands. The cookie set action is used only by this foundation test; real journeys establish sessions another way and use this command only to clear them. Its foundation-cookie probe, result-kind literals and Chrome user-agent text add no independent product regression. The named journey covers actual editing, not a mutation confined to the unused cookie-set helper branch. | apps/web/test/browser-exact-dist/s2-input.integration.test.ts:284 |
| BS016 | 59 — refuses navigation and code inputs at the production command boundary | MOVE | L1 | Move the five malformed/unsupported/extra-field production command inputs into a Node contract test for parseProductionHostRequest (proposed node-contract/browser-command-contract.test.ts). This exact-object validator is called at the privileged browser-command dispatch boundary, but the test invokes a pure parser with no browser or transport. Existing successful production journeys cannot prove rejection of arbitrary navigation/code inputs; preserve the guard coverage in the cheaper owning layer. | apps/web/test/browser-exact-dist/production-host.integration.test.ts:5 |

## apps/web/test/browser-source/chapter-navigation.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS018 | 38 — selects a tree Chapter through getChapter and keeps pending on the current Chapter | KEEP | K2 | Read-only inspection of another Chapter must leave the current Chapter's unsent text intact; returning must show that pending text, and a later expired Chapter query must keep it while showing a user-facing error without raw protocol text. Packaged navigation opens empty Chapters and does not combine a pending edit with failed inspection. Local current_chapter override rejection is overlapping coverage, not the unique reason. | apps/web/test/browser-exact-dist/s2-09-navigate-reopen.integration.test.ts:95; apps/web/test/browser-source/current-chapter-historical-acknowledgement.integration.test.ts:26 |

## apps/web/test/browser-source/create-historical-acknowledgement.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS003 | 74 — explains a historical Create Volume acknowledgement and does not retry it | KEEP | K2 | Create Volume must render its historical error in the inline form without retrying the POST or allocating another Challenge. Create Chapter has a different wrapper/form component and error state; server structural replay coverage does not execute either browser catch path. | apps/web/test/browser-source/create-historical-acknowledgement.integration.test.ts:125; apps/web/test/node-postgresql/create-volume-http.integration.test.ts:760 |
| BS004 | 125 — keeps the pending inline title when another creation is requested and explains the historical acknowledgement | KEEP | K2 | A held Chapter creation must keep the original connected read-only input, title and target Volume when another creation is requested; after historical failure it must show the specific error and send only the original POST/Challenge. Create Volume and completed creation journeys do not force this pending-form interleaving. | apps/web/test/browser-source/create-historical-acknowledgement.integration.test.ts:74; apps/web/test/browser-exact-dist/s2-08-create-chapter.integration.test.ts:43 |

## apps/web/test/browser-source/current-chapter-historical-acknowledgement.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS009 | 26 — explains a historical Set Current Chapter acknowledgement and does not retry it | KEEP | K2 | Requesting another Current Chapter and receiving historical unavailability must leave Chapter A displayed, show the alert and make only one PUT. This guards the switch recovery state plus set-current-chapter's own retry guard; rename/delete error forms do not execute navigation state changes. | apps/web/test/node-postgresql/set-current-chapter-http.integration.test.ts:1043; apps/web/test/browser-source/undo-historical-acknowledgement.integration.test.ts:22 |

## apps/web/test/browser-source/delete-historical-acknowledgement.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS005 | 74 — explains a historical Delete Volume acknowledgement and does not retry it | KEEP | K2 | Confirmed Volume deletion must display the historical acknowledgement alert and avoid the delete-volume wrapper's automatic retry. Chapter deletion has its own wrapper and view handler. This case checks the Project title but does not explicitly assert that the Volume row remains. | apps/web/test/browser-source/delete-historical-acknowledgement.integration.test.ts:131; apps/web/test/browser-source/create-historical-acknowledgement.integration.test.ts:74 |
| BS006 | 131 — explains a historical Delete Chapter acknowledgement and does not retry it | KEEP | K2 | Confirmed Chapter deletion must preserve its visible Chapter and show the historical alert after exactly one DELETE/Challenge. Volume deletion executes different command and view catch paths; HTTP only establishes the received problem, not whether the browser removes the item or retries it. | apps/web/test/browser-source/delete-historical-acknowledgement.integration.test.ts:74; apps/web/test/node-postgresql/create-volume-http.integration.test.ts:760 |

## apps/web/test/browser-source/list-open.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS017 | 11 — opens a listed empty Project from getProject, not from the list payload | KEEP | K1 | Opening an empty Project must replace a stale/different library title with getProject's current title and load the canonical empty tree. The packaged library test uses matching list/query titles and cannot detect reusing the stale list payload as the opened Project. This test requires the actual query plus visible replacement. | apps/web/test/browser-exact-dist/s2-03-list-open-library.integration.test.ts:34; apps/web/test/browser-source/project-entry.integration.test.ts:18 |

## apps/web/test/browser-source/project-entry.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS019 | 18 — opens the URL-selected Project and fails closed before invalid entry requests | KEEP | K2 | Entry must use the URL Project rather than stale document metadata, and malformed/extra-segment Project URLs must render a blocked state without any request or stale page content. The Node Project opener receives an already selected ID; library opening has a valid ID and no invalid URL. Keep these entry/DOM cases; the normal query order/credentials and protected-home wording repeat other owners. | apps/web/test/node-contract/project-open.test.ts:31; apps/web/test/browser-source/list-open.integration.test.ts:11 |

## apps/web/test/browser-source/readable-export-historical-acknowledgement.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS011 | 22 — explains a historical human-readable export acknowledgement and does not retry it | KEEP | K2 | The readable export control must display historical unavailability and retain the visible Chapter after one admission POST. Its export-human-readable retry map and manuscript-readable-export error state are separate from rename/navigation; HTTP historical evidence cannot prove this button state or avoid browser retry. | apps/web/test/node-postgresql/readable-export-admission-http.integration.test.ts:715; apps/web/test/browser-source/rename-historical-acknowledgement.integration.test.ts:15 |

## apps/web/test/browser-source/rename-historical-acknowledgement.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS002 | 15 — explains a historical Update Project acknowledgement and does not retry it | KEEP | K2 | Project rename must show the historical explanation while retaining the author's unsaved title and the listed Project, with exactly one command and Challenge. Volume/Chapter rename use different React components and wrapper functions; real HTTP proves the error response but not form preservation or command resubmission. | apps/web/test/node-postgresql/update-project-http.integration.test.ts:575; apps/web/test/browser-source/update-historical-acknowledgement.integration.test.ts:74 |

## apps/web/test/browser-source/undo-historical-acknowledgement.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS010 | 22 — explains a historical Undo Latest Author Action acknowledgement and does not retry it | KEEP | K2 | The editor Undo shortcut must report historical unavailability while retaining saved state and no editor failure, with one Undo POST. Undo's command catch and editor-state cleanup differ from structural form errors; real Undo historical replay does not dispatch a keyboard event or inspect browser failure state. | apps/web/test/node-postgresql/undo-latest-author-action-http.integration.test.ts:603; apps/web/test/browser-source/current-chapter-historical-acknowledgement.integration.test.ts:26 |

## apps/web/test/browser-source/update-historical-acknowledgement.integration.test.ts

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| BS007 | 74 — explains a historical Update Volume acknowledgement and does not retry it | KEEP | K2 | Volume rename must keep the typed title and show its historical error without a second PATCH or Challenge. Its volume-tree-actions component and update-volume retry wrapper are independent of the Project and Chapter forms; server-side shared structural replay cannot observe discarded form text. | apps/web/test/browser-source/update-historical-acknowledgement.integration.test.ts:138; apps/web/test/browser-source/rename-historical-acknowledgement.integration.test.ts:15 |
| BS008 | 138 — explains a historical Update Chapter acknowledgement and does not retry it | KEEP | K2 | Chapter rename must retain the author's input and surrounding Volume while presenting the historical error after one PATCH/Challenge. Updating a Volume uses a separate component and wrapper; successful packaged rename does not execute this catch branch. | apps/web/test/browser-source/update-historical-acknowledgement.integration.test.ts:74; apps/web/test/node-postgresql/create-volume-http.integration.test.ts:760 |
