# Core tests — partial review

Completed: nine Project, Volume and Chapter test files, 49 tests. The directory is not complete. Reason codes: [METHOD.md](METHOD.md).

D5 does not claim that the named public test executes an unreachable guard. Record an uncovered mutation if selected; do not present it as a successful coverage experiment.

## crates/storyos-core/src/archive_project_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|
| CO003 | 16 — a_matching_revision_and_active_lifecycle_classifies_as_applied | DELETE | D1 | The HTTP test asserts an Applied archive, revision 2 and archived library state. The Core success tuple adds no input or effect. | apps/web/test/node-postgresql/archive-project-http.integration.test.ts:158 |
| CO004 | 24 — a_stale_revision_classifies_as_conflicted_with_zero_lifecycle_effect | DELETE | D1 | The HTTP test archives at revision 1, then submits a new key at stale revision 1 and expects Conflicted. The current Project is already archived, so this also observes stale-revision precedence. | apps/web/test/node-postgresql/archive-project-http.integration.test.ts:158 |
| CO005 | 37 — an_already_archived_project_classifies_as_no_effect | DELETE | D1 | The HTTP test sends a new key at revision 2 to an already archived Project and expects NoEffect. It exercises the same Core lifecycle condition. | apps/web/test/node-postgresql/archive-project-http.integration.test.ts:158 |
| CO006 | 49 — a_missing_project_classifies_as_refused_with_zero_lifecycle_effect | DELETE | D5 | Only persist_archive_project calls this classifier in product code. It returns MissingProject before the call and always passes ProjectPresence::Present (archive_project.rs:83,101 in the adapter). Absent is an internal synthetic input here. HTTP Scope refusal remains covered, but that test cannot kill a mutation confined to this unreachable Core guard. | apps/web/test/node-postgresql/archive-project-http.integration.test.ts:158 |

## crates/storyos-core/src/create_chapter_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|
| CO023 | 21 — the_first_chapter_on_an_empty_active_project_becomes_current | DELETE | D1 | The HTTP test creates the first Chapter and checks its identity becomes Current Chapter, order 1 and tree revision 3. | apps/web/test/node-postgresql/create-chapter-http.integration.test.ts:361 |
| CO024 | 33 — a_later_chapter_preserves_the_existing_current_chapter | DELETE | D1 | Later Chapter creation checks the first Chapter remains current and the tree revision advances. The pure fixture uses an empty ordered list with an existing Current Chapter, but that synthetic combination adds no observable production case. | apps/web/test/node-postgresql/create-chapter-http.integration.test.ts:361 |
| CO025 | 50 — a_stale_tree_revision_classifies_as_conflicted_with_zero_authority_effect | DELETE | D1 | The stale Create Chapter request checks conflicted/stale_tree_revision and no Commit or Author Action. | apps/web/test/node-postgresql/create-chapter-http.integration.test.ts:361 |
| CO026 | 61 — an_invalid_volume_join_classifies_as_refused_with_zero_authority_effect | DELETE | D1 | The missing Volume request checks refused/invalid_volume_join despite a stale expected tree; it observes join refusal precedence as well. | apps/web/test/node-postgresql/create-chapter-http.integration.test.ts:361 |
| CO027 | 71 — an_archived_project_classifies_as_refused_with_zero_authority_effect | DELETE | D1 | After archive, a new Chapter request checks refused/archived_project and no authority allocation. | apps/web/test/node-postgresql/create-chapter-http.integration.test.ts:361 |
| CO028 | 81 — an_invalid_title_classifies_as_refused_with_zero_authority_effect | DELETE | D5 | HTTP structure_title rejects invalid title lengths before calling the sole current product adapter entry. The empty title is asserted as HTTP 400. This named test does not exercise the Core invalid-title branch or the overlong HTTP case. | apps/web/test/node-postgresql/create-chapter-http.integration.test.ts:361 |

## crates/storyos-core/src/create_project_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|
| CO001 | 4 — an_absent_project_classifies_as_the_closed_empty_open_state | DELETE | D1 | The HTTP create case asserts an empty opened Project, zero Chapters, and one project row. It reaches the Core classifier through persist_create_project. Returning ExistingProject for Absent would reject the same observed operation. | apps/web/test/node-postgresql/create-project-http.integration.test.ts:75 |
| CO002 | 12 — an_existing_project_classifies_as_a_closed_refusal | DELETE | D1 | The HTTP test creates the prospective Project before command submission and expects 409. Bypassing the Core ExistingProject result would reach the INSERT and return a different error, so this is not merely an idempotency-replay assertion. | apps/web/test/node-postgresql/create-project-http.integration.test.ts:75 |

## crates/storyos-core/src/create_volume_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|
| CO012 | 16 — a_matching_tree_revision_and_active_project_classifies_as_applied | DELETE | D1 | HTTP creation asserts tree revision 2, Volume title/order, one Authoritative Commit, and the queried tree. The pure Applied result repeats its revision increment. | apps/web/test/node-postgresql/create-volume-http.integration.test.ts:148 |
| CO013 | 24 — a_stale_tree_revision_classifies_as_conflicted_with_zero_authority_effect | DELETE | D1 | HTTP creation with the prior tree revision must produce stale_tree_revision and keep the tree and Volume count unchanged. | apps/web/test/node-postgresql/create-volume-http.integration.test.ts:148 |
| CO014 | 35 — an_archived_project_classifies_as_refused_with_zero_authority_effect | DELETE | D1 | The HTTP test archives the Project, then creates a Volume and checks refused/archived_project. This reaches the Core lifecycle rule. | apps/web/test/node-postgresql/create-volume-http.integration.test.ts:148 |
| CO015 | 45 — an_invalid_title_classifies_as_refused_with_zero_authority_effect | DELETE | D5 | Both invalid lengths are refused by structure_title at the current HTTP ingress before Core. No other production caller of the adapter create_volume method was found. The HTTP empty-title 400 covers the observable refusal, not a mutation confined to the duplicate Core guard. The overlong public request case is not covered by this named test. | apps/web/test/node-postgresql/create-volume-http.integration.test.ts:148 |

## crates/storyos-core/src/delete_chapter_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|
| CO036 | 21 — removing_a_non_current_chapter_preserves_the_current_chapter | DELETE | D1 | Deleting non-current Chapter B retains A as Current Chapter and removes B from the queried tree. | apps/web/test/node-postgresql/delete-chapter-http.integration.test.ts:246 |
| CO037 | 32 — removing_the_current_chapter_selects_the_next_remaining_chapter | DELETE | D1 | Deleting current A with C remaining selects C. The same forward-successor rule is observed through the public command and Project response. | apps/web/test/node-postgresql/delete-chapter-http.integration.test.ts:246 |
| CO038 | 48 — removing_the_last_current_chapter_selects_the_previous_remaining_chapter | KEEP | K1 | Deleting the last current Chapter while earlier Chapters remain must select the preceding Chapter, not Empty. The HTTP test name says next then previous but its actual sequence deletes non-current B, then A, then sole remaining C; it never selects a previous sibling. The exact-dist test removes A/B/C in forward order, and Adapter tests cover non-current or forward deletion. This Core case is the only reviewed previous-sibling input. | apps/web/test/node-postgresql/delete-chapter-http.integration.test.ts:246; apps/web/test/browser-exact-dist/s2-13-delete-chapter.integration.test.ts:181; crates/storyos-adapter-postgres/src/delete_chapter_tests.rs:354 |
| CO039 | 64 — removing_the_only_current_chapter_opens_an_explicit_empty_state | DELETE | D1 | The HTTP deletion of sole remaining C checks an explicit empty Project and empty queried tree. | apps/web/test/node-postgresql/delete-chapter-http.integration.test.ts:246 |
| CO040 | 81 — an_already_removed_chapter_classifies_as_no_effect | DELETE | D1 | A new-key delete of already removed C checks no_effect/already_removed, not merely idempotent replay. | apps/web/test/node-postgresql/delete-chapter-http.integration.test.ts:246 |
| CO041 | 92 — a_stale_tree_revision_classifies_as_conflicted_with_zero_authority_effect | DELETE | D1 | A stale tree request for removed A checks Conflicted, proving stale revision is checked before removed-state NoEffect. | apps/web/test/node-postgresql/delete-chapter-http.integration.test.ts:246 |
| CO042 | 102 — an_invalid_chapter_join_classifies_as_refused_with_zero_authority_effect | DELETE | D1 | The public missing Chapter case checks refused/invalid_chapter_join. | apps/web/test/node-postgresql/delete-chapter-http.integration.test.ts:417 |
| CO043 | 112 — an_archived_project_classifies_as_refused_with_zero_authority_effect | DELETE | D1 | The same HTTP case archives the Project before deletion and checks refused/archived_project. | apps/web/test/node-postgresql/delete-chapter-http.integration.test.ts:417 |

## crates/storyos-core/src/delete_volume_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|
| CO044 | 20 — removing_an_empty_volume_applies_the_next_tree_revision | DELETE | D1 | Deleting an empty Volume checks Applied, an Authoritative Commit and a queried tree without that Volume. The frozen-acknowledgement case also uses the next revision for NoEffect. | apps/web/test/node-postgresql/delete-volume-http.integration.test.ts:238 |
| CO045 | 28 — a_nonempty_volume_is_refused_with_zero_authority_effect | DELETE | D1 | The nonempty Volume case checks nonempty_volume, no Commit/Author Action, and unchanged tree revision and sibling order. | apps/web/test/node-postgresql/delete-volume-http.integration.test.ts:238 |
| CO046 | 40 — an_already_removed_volume_is_a_no_effect_retry | DELETE | D1 | A new-key delete of the removed Volume checks no_effect/already_removed with no authority allocation. | apps/web/test/node-postgresql/delete-volume-http.integration.test.ts:238 |
| CO047 | 53 — a_stale_tree_revision_is_conflicted_with_zero_authority_effect | DELETE | D1 | The frozen-acknowledgement case sends stale tree revision 4 after deletion advances it to 5, and checks Conflicted. | apps/web/test/node-postgresql/delete-volume-http.integration.test.ts:517 |
| CO048 | 65 — an_invalid_volume_join_is_refused_with_zero_authority_effect | KEEP | K1 | A syntactically valid missing or foreign Volume identity in the owned Project must refuse InvalidVolumeJoin instead of returning Applied/NoEffect. The HTTP delete test rejects a foreign Project and tests createChapter into a removed Volume, which are different calls. Neither it nor the Adapter deletion test supplies a missing Volume to delete_volume. | apps/web/test/node-postgresql/delete-volume-http.integration.test.ts:238; crates/storyos-adapter-postgres/src/delete_volume_tests.rs:282 |
| CO049 | 77 — an_archived_project_is_refused_with_zero_authority_effect | DELETE | D1 | The archive-specific HTTP test creates a Volume, archives its Project and checks refused/archived_project on deletion. | apps/web/test/node-postgresql/delete-volume-http.integration.test.ts:384 |

## crates/storyos-core/src/update_chapter_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|
| CO029 | 21 — a_matching_revision_rename_and_reorder_classifies_as_applied | DELETE | D1 | The HTTP rename/reorder checks title, order, tree revision, unchanged prose and queried placement. | apps/web/test/node-postgresql/update-chapter-http.integration.test.ts:252 |
| CO030 | 33 — a_stale_tree_revision_classifies_as_conflicted_with_zero_authority_effect | DELETE | D1 | A stale tree request produces conflicted/stale_tree_revision and leaves tree revision and sibling count unchanged. | apps/web/test/node-postgresql/update-chapter-http.integration.test.ts:252 |
| CO031 | 43 — an_unchanged_title_and_order_classifies_as_no_effect | DELETE | D1 | The unchanged title/order request checks no_effect/unchanged and allocates neither Commit nor Author Action. | apps/web/test/node-postgresql/update-chapter-http.integration.test.ts:252 |
| CO032 | 54 — a_wrong_scope_chapter_classifies_as_refused_with_zero_authority_effect | DELETE | D1 | The second HTTP case requests a missing Chapter and checks refused/invalid_chapter_join with no Commit or Author Action. | apps/web/test/node-postgresql/update-chapter-http.integration.test.ts:470 |
| CO033 | 64 — an_invalid_order_classifies_as_refused_with_zero_authority_effect | DELETE | D1 | The HTTP case checks order beyond sibling count as invalid_order. Zero is rejected by structure_admission::positive before Core and is not claimed as covered inside Core. | apps/web/test/node-postgresql/update-chapter-http.integration.test.ts:252 |
| CO034 | 80 — an_archived_project_classifies_as_refused_with_zero_authority_effect | DELETE | D1 | The second HTTP case archives before Update Chapter and asserts archived_project even when title and order are unchanged. | apps/web/test/node-postgresql/update-chapter-http.integration.test.ts:470 |
| CO035 | 90 — an_invalid_title_classifies_as_refused_with_zero_authority_effect | DELETE | D5 | Invalid title lengths are rejected by structure_title at the sole current product ingress. HTTP checks the empty title as 400; it does not execute this internal Core guard or test the overlong HTTP variant. | apps/web/test/node-postgresql/update-chapter-http.integration.test.ts:252 |

## crates/storyos-core/src/update_project_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|
| CO007 | 17 — a_matching_revision_and_new_title_classifies_as_applied | DELETE | D1 | The HTTP test renames and then reads both Project and library, including revision 2. It observes the same Core title/revision effect. | apps/web/test/node-postgresql/update-project-http.integration.test.ts:210 |
| CO008 | 28 — a_stale_revision_classifies_as_conflicted_with_zero_title_effect | DELETE | D1 | The HTTP test sends Stale Title with revision 1 after a successful rename and expects Conflicted without a title change. | apps/web/test/node-postgresql/update-project-http.integration.test.ts:210 |
| CO009 | 41 — an_unchanged_title_classifies_as_no_effect | DELETE | D1 | The HTTP test sends the same title at the current revision and expects a NoEffect Receipt. The pure classifier repeats that condition. | apps/web/test/node-postgresql/update-project-http.integration.test.ts:210 |
| CO010 | 53 — an_invalid_title_classifies_as_refused_with_zero_title_effect | DELETE | D5 | Application admission rejects empty and over-1024-byte titles before invoking the store. Thus the Core invalid-title branch is not an independent reachable product protection. Application tests explicitly cover both lengths; the HTTP test covers the empty-title public refusal. Those tests do not execute a mutation confined to this Core guard. | crates/storyos-application/src/update_project_tests.rs:98; apps/web/test/node-postgresql/update-project-http.integration.test.ts:294 |
| CO011 | 73 — a_missing_project_classifies_as_refused_with_zero_title_effect | DELETE | D5 | persist_update_project returns MissingProject before classification and always passes Present (adapter update_project.rs:83,95). The synthetic Absent branch has no current product caller. HTTP foreign-Scope refusal is semantic coverage, not coverage of this unreachable Core guard. | apps/web/test/node-postgresql/update-project-http.integration.test.ts:210 |

## crates/storyos-core/src/update_volume_tests.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|
| CO016 | 21 — a_matching_revision_rename_and_reorder_classifies_as_applied | DELETE | D1 | HTTP rename/reorder asserts title, order 2, incremented tree revision and queried canonical placement. This includes all fields in the pure Applied assertion. | apps/web/test/node-postgresql/update-volume-http.integration.test.ts:196 |
| CO017 | 33 — a_stale_tree_revision_classifies_as_conflicted_with_zero_authority_effect | DELETE | D1 | The HTTP test submits a stale tree revision and checks conflicted/stale_tree_revision, no Authoritative Commit and unchanged tree. | apps/web/test/node-postgresql/update-volume-http.integration.test.ts:196 |
| CO018 | 43 — an_unchanged_title_and_order_classifies_as_no_effect | DELETE | D1 | The HTTP unchanged title/order case asserts no_effect/unchanged with no Commit or Author Action. | apps/web/test/node-postgresql/update-volume-http.integration.test.ts:196 |
| CO019 | 54 — a_wrong_scope_volume_classifies_as_refused_with_zero_authority_effect | DELETE | D1 | The HTTP missing-Volume case reaches InvalidVolumeJoin and checks refused/invalid_volume_join, no Commit and no Author Action. The pure test uses the same Invalid join state. | apps/web/test/node-postgresql/update-volume-http.integration.test.ts:196 |
| CO020 | 64 — an_invalid_order_classifies_as_refused_with_zero_authority_effect | DELETE | D1 | Order above the Volume count is checked as refused/invalid_order in the HTTP test. The zero-order case is rejected by structure_admission::positive before this Core call; it adds no reachable Core input. Do not claim a Core zero-only mutation is covered. | apps/web/test/node-postgresql/update-volume-http.integration.test.ts:196 |
| CO021 | 80 — an_archived_project_classifies_as_refused_with_zero_authority_effect | DELETE | D1 | The HTTP test archives the Project and verifies the update refuses as archived_project. This is not the unchanged outcome although the values equal the current values. | apps/web/test/node-postgresql/update-volume-http.integration.test.ts:196 |
| CO022 | 90 — an_invalid_title_classifies_as_refused_with_zero_authority_effect | DELETE | D5 | structure_title rejects empty and overlong titles before the only current production entry to update_volume. The HTTP empty-title assertion covers the public refusal, not this duplicate Core guard. A new internal consumer would require revisiting this verdict. | apps/web/test/node-postgresql/update-volume-http.integration.test.ts:196 |

